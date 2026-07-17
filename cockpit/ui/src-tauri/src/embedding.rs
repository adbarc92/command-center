// App-plugin child-webview embedding (Lane S integration of the Lane A contract bundle;
// P3 spike `spike/app-plugins-webview-v2` carry-forward).
//
// These three commands are the trusted Rust surface the Svelte shell drives to composite a
// child webview (a whole first-party web app) under a Svelte-owned content rect.
//
// TWO load-bearing findings from the P3 spike are baked in here:
//  1. The commands are `async`. A *synchronous* command runs on the main (event-loop)
//     thread, but `add_child` / `set_position` post a message to that same loop and block
//     on it — a sync command deadlocks itself (the `spike_show` hang). As `async` commands
//     they run on an async-runtime worker, leaving the loop free to pump webview creation.
//  2. Inactive webviews are PARKED off-screen, never `hide()`d. WebView2 `hide()`→`show()`
//     forces a repaint/reload (loses scroll/app state); moving the view off-screen keeps its
//     render tree warm. A small warm-pool LRU bounds how many kept-warm webviews live.
//
// Webview label scheme is `app::<id>` — stable across relaunch/adopt, and matched by the
// `webviews: ["app::*"]` capability glob in `capabilities/default.json`.

use crate::plugins::manager::PluginManager;
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::{
    webview::WebviewBuilder, AppHandle, LogicalPosition, LogicalSize, Manager, State, WebviewUrl,
};

/// The CSS/logical rect the Svelte placeholder reports (device-independent px, relative to
/// the host window's content area). Mirrors the `DOMRect` fields the shell emits.
#[derive(Debug, Clone, Copy, serde::Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// How many kept-warm (parked) webviews we keep before destroying the least-recently-used.
const WARM_CAP: usize = 3;
/// Park origin: far enough off-screen that no sliver paints, near enough to stay a valid
/// logical coordinate. The render tree stays ALIVE here (no hide()/show() repaint-reload).
const PARK_X: f64 = -32000.0;
const PARK_Y: f64 = -32000.0;
/// The host window created by tauri.conf.json (its only `windows[]` entry) gets the
/// default label "main".
const HOST_WINDOW_LABEL: &str = "main";

/// Webview label for an app plugin. Stable across relaunch/adopt; matches the `app::*`
/// capability glob.
pub fn app_label(id: &str) -> String {
    format!("app::{id}")
}

/// Warm-pool bookkeeping: MRU-ordered labels + each label's last on-screen rect. Kept as
/// host state via `.manage(WebviewPool::default())`.
#[derive(Default)]
pub struct WebviewPool {
    lru: Mutex<Vec<String>>,
    last_rect: Mutex<HashMap<String, Rect>>,
}

impl WebviewPool {
    /// Mark `label` most-recently-used and evict past the warm cap (destroying the LRU
    /// victim's webview so memory is bounded — the next show recreates it).
    fn touch_and_evict(&self, app: &AppHandle, label: &str, rect: Rect) {
        self.last_rect.lock().unwrap().insert(label.to_string(), rect);
        let mut lru = self.lru.lock().unwrap();
        lru.retain(|l| l != label);
        lru.insert(0, label.to_string());
        while lru.len() > WARM_CAP {
            if let Some(victim) = lru.pop() {
                if let Some(wv) = app.get_webview(&victim) {
                    let _ = wv.close();
                }
                self.last_rect.lock().unwrap().remove(&victim);
            }
        }
    }
}

/// First show → create the child webview over the reserved rect (call only once the plugin
/// is `healthy`); later shows → reposition + bring on-screen + focus. Async: webview
/// create/show MUST run off the main thread (see module docs — sync deadlocks).
#[tauri::command]
pub async fn plugin_show(
    app: AppHandle,
    mgr: State<'_, PluginManager>,
    pool: State<'_, WebviewPool>,
    id: String,
    rect: Rect,
) -> Result<(), String> {
    let label = app_label(&id);
    if let Some(wv) = app.get_webview(&label) {
        wv.set_position(LogicalPosition::new(rect.x, rect.y))
            .map_err(|e| format!("set_position: {e}"))?;
        wv.set_size(LogicalSize::new(rect.width, rect.height))
            .map_err(|e| format!("set_size: {e}"))?;
        wv.set_focus().map_err(|e| format!("set_focus: {e}"))?;
    } else {
        let url_str = mgr
            .url_for(&id)
            .ok_or_else(|| format!("no url for plugin {id} (launch it first?)"))?;
        let url: tauri::Url = url_str
            .parse()
            .map_err(|e| format!("bad plugin url {url_str}: {e}"))?;
        let window = app
            .get_window(HOST_WINDOW_LABEL)
            .ok_or_else(|| format!("host window '{HOST_WINDOW_LABEL}' not found"))?;
        // THE `unstable` call: attach a child webview to the existing host window.
        let wv = window
            .add_child(
                WebviewBuilder::new(&label, WebviewUrl::External(url)),
                LogicalPosition::new(rect.x, rect.y),
                LogicalSize::new(rect.width, rect.height),
            )
            .map_err(|e| format!("add_child: {e}"))?;
        wv.set_focus().map_err(|e| format!("set_focus: {e}"))?;
    }
    pool.touch_and_evict(&app, &label, rect);
    Ok(())
}

/// Switch-away / host-overlay-open → PARK off-screen (keep the render tree warm), do NOT
/// destroy and do NOT `hide()` (which would force a repaint/reload on the next show).
#[tauri::command]
pub async fn plugin_hide(app: AppHandle, id: String) -> Result<(), String> {
    if let Some(wv) = app.get_webview(&app_label(&id)) {
        wv.set_position(LogicalPosition::new(PARK_X, PARK_Y))
            .map_err(|e| format!("park: {e}"))?;
    }
    Ok(())
}

/// ResizeObserver-driven: keep the native webview glued to the reserved box on window
/// resize / layout change. Async for the same main-thread reason as `plugin_show`.
#[tauri::command]
pub async fn plugin_set_rect(
    app: AppHandle,
    pool: State<'_, WebviewPool>,
    id: String,
    rect: Rect,
) -> Result<(), String> {
    let label = app_label(&id);
    if let Some(wv) = app.get_webview(&label) {
        wv.set_position(LogicalPosition::new(rect.x, rect.y))
            .map_err(|e| format!("set_position: {e}"))?;
        wv.set_size(LogicalSize::new(rect.width, rect.height))
            .map_err(|e| format!("set_size: {e}"))?;
        pool.last_rect.lock().unwrap().insert(label, rect);
    }
    Ok(())
}
