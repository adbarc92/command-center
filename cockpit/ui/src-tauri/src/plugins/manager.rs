use crate::plugins::discovery::{discover, DiscoveredPlugin};
use crate::plugins::seams::Probe;
use crate::plugins::seams_impl::{HttpProbe, RealClock, ShellSpawner, TauriEventSink};
use crate::plugins::state::{run_start_sequence, StartOutcome};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, State};

/// One launched plugin's runtime record.
pub struct Running {
    pub child_id: Option<u64>,
    pub owned: bool,
}

#[derive(Default)]
pub struct PluginManager {
    pub discovered: Mutex<Vec<DiscoveredPlugin>>,
    pub running: Mutex<HashMap<String, Running>>,
    pub spawner: ShellSpawner,
}

impl PluginManager {
    /// Discovery roots: the per-user plugins dir. (A dev-list root can be added later.)
    pub fn roots() -> Vec<PathBuf> {
        let mut v = Vec::new();
        if let Some(home) = home_dir() {
            v.push(home.join(".command-center/app-plugins"));
        }
        v
    }

    /// The head URL for a discovered plugin (used by the Phase-6 embedding layer).
    pub fn url_for(&self, id: &str) -> Option<String> {
        self.discovered
            .lock()
            .unwrap()
            .iter()
            .find(|d| d.manifest.id == id)
            .map(|d| d.manifest.url.clone())
    }
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
}

#[tauri::command]
pub fn plugins_list(mgr: State<'_, PluginManager>) -> Vec<serde_json::Value> {
    let roots = PluginManager::roots();
    let root_refs: Vec<&std::path::Path> = roots.iter().map(|p| p.as_path()).collect();
    let found = discover(&root_refs);
    let out = found
        .iter()
        .map(|d| {
            serde_json::json!({
                "id": d.manifest.id,
                "name": d.manifest.name,
                "icon": d.manifest.icon,
                "url": d.manifest.url,
            })
        })
        .collect();
    *mgr.discovered.lock().unwrap() = found;
    out
}

#[tauri::command]
pub fn plugin_launch(
    app: AppHandle,
    mgr: State<'_, PluginManager>,
    id: String,
) -> Result<(), String> {
    let disc = {
        mgr.discovered
            .lock()
            .unwrap()
            .iter()
            .find(|d| d.manifest.id == id)
            .cloned()
    };
    let Some(disc) = disc else {
        return Err(format!("unknown plugin {id}"));
    };

    let probe = HttpProbe;
    let clock = RealClock::new();
    let sink = TauriEventSink { app: app.clone() };
    let images_present = probe.probe(&disc.manifest.lifecycle.health.url).is_some();

    // NOTE: this call is synchronous and may block up to the probe timeout (~180 s).
    // If that proves problematic in the Phase-6 smoke it can move to a background task.
    let outcome = run_start_sequence(
        &disc.manifest,
        &disc.dir,
        &probe,
        &mgr.spawner,
        &clock,
        &sink,
        images_present,
    );
    match outcome {
        StartOutcome::Healthy { owned, child_id } => {
            mgr.running
                .lock()
                .unwrap()
                .insert(id, Running { child_id, owned });
            // Phase 6 shows the webview here (once healthy).
            Ok(())
        }
        StartOutcome::Error(e) => Err(e),
    }
}
