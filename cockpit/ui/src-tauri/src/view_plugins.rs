// View-plugin asset scheme `ccplugin://` (Lane S integration of the Lane V contract bundle;
// P4 spike `spike/view-plugins-handshake` carry-forward).
//
// A sandboxed view-plugin runs in an `<iframe sandbox="allow-scripts">` whose `src` is
// `ccplugin://localhost/<id>/<path>`. On Windows/WebView2 (primary target) that resolves to
// the distinct origin `http://ccplugin.localhost/...`; every plugin shares that origin
// (`<id>` is a path). This handler serves the plugin's static assets from disk with the
// plugin-document CSP + CORS header.
//
// THREE load-bearing findings from the P4 spike are baked in here (each cost a dropped
// handshake run to discover — do not "simplify" them away):
//  1. `script-src` names the concrete origin `http://ccplugin.localhost`, NOT `'self'`. A
//     `sandbox="allow-scripts"` iframe has an OPAQUE ("null") origin, and CSP `'self'`
//     matches the document origin — which, being opaque, matches nothing.
//  2. Every response carries `Access-Control-Allow-Origin: *`. Opaque-origin module scripts
//     (the `<script type=module src=sdk.js>` entry) are ALWAYS fetched in CORS mode; without
//     ACAO the fetch is rejected, `sdk.js` never runs, no `plugin-hello` posts, and every
//     handshake times out.
//  3. The plugin-document CSP is delivered as a RESPONSE HEADER (never a `<meta>` the plugin
//     could rewrite). `connect-src 'none'` is what makes the no-network guarantee provable.

use std::path::PathBuf;
use tauri::http::{Request, Response};
use tauri::{AppHandle, Manager};

/// The custom URI scheme the host iframe points at. Registered on the Tauri builder.
pub const SCHEME: &str = "ccplugin";

/// The plugin-document Content-Security-Policy (see finding #1/#3 above).
const PLUGIN_CSP: &str = "default-src 'none'; script-src 'self' http://ccplugin.localhost; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'none'; frame-src 'none'; base-uri 'none'; form-action 'none'";

fn mime_for(sub: &str) -> &'static str {
    match sub.rsplit('.').next() {
        Some("html") | Some("htm") => "text/html",
        Some("js") | Some("mjs") => "text/javascript",
        Some("css") => "text/css",
        Some("json") => "application/json",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("woff2") => "font/woff2",
        Some("wasm") => "application/wasm",
        _ => "application/octet-stream",
    }
}

/// Roots that hold view-plugin static assets; each `<id>` is a subdir. Searched in order,
/// first hit wins:
///  - `CC_VIEW_PLUGINS_DEV` (dev seam — point at the repo `plugins/` dir),
///  - the packaged resource dir `plugins/`,
///  - the per-user `~/.command-center/plugins`.
fn plugin_roots(app: &AppHandle) -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(dev) = std::env::var_os("CC_VIEW_PLUGINS_DEV") {
        v.push(PathBuf::from(dev));
    }
    if let Ok(res) = app.path().resource_dir() {
        v.push(res.join("plugins"));
    }
    if let Some(home) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")) {
        v.push(PathBuf::from(home).join(".command-center/plugins"));
    }
    v
}

/// The shared plugin SDK, served same-origin as `<id>/sdk.js` so every plugin can
/// `import './sdk.js'`. Dev: `CC_PLUGIN_SDK` (repo `cockpit/plugin-sdk/index.js`);
/// packaged: the resource dir `plugin-sdk/index.js`.
fn sdk_bytes(app: &AppHandle) -> Option<Vec<u8>> {
    if let Some(p) = std::env::var_os("CC_PLUGIN_SDK") {
        if let Ok(b) = std::fs::read(PathBuf::from(p)) {
            return Some(b);
        }
    }
    if let Ok(res) = app.path().resource_dir() {
        if let Ok(b) = std::fs::read(res.join("plugin-sdk/index.js")) {
            return Some(b);
        }
    }
    None
}

fn built(status: u16, mime: &str, body: Vec<u8>) -> Response<Vec<u8>> {
    Response::builder()
        .status(status)
        .header("Content-Type", mime)
        .header("Content-Security-Policy", PLUGIN_CSP)
        // Opaque-origin (sandboxed) iframe fetches its module graph in CORS mode (finding #2).
        .header("Access-Control-Allow-Origin", "*")
        .body(body)
        .expect("static response is always well-formed")
}

fn not_found() -> Response<Vec<u8>> {
    built(404, "text/plain", b"cc view-plugin: not found".to_vec())
}

/// Serve one asset for `ccplugin://localhost/<id>/<path>`. Registered via
/// `.register_uri_scheme_protocol(SCHEME, |ctx, req| respond(ctx.app_handle(), req))`.
pub fn respond(app: &AppHandle, request: Request<Vec<u8>>) -> Response<Vec<u8>> {
    // uri().path() == "/<id>/<path...>"
    let rel = request.uri().path().trim_start_matches('/');
    let mut parts = rel.splitn(2, '/');
    let id = parts.next().unwrap_or("");
    let mut sub = parts.next().unwrap_or("").to_string();
    if id.is_empty() {
        return not_found();
    }
    if sub.is_empty() {
        sub = "index.html".to_string();
    }
    // Reject path traversal / odd components — the id and every sub segment must be plain.
    if id == ".." || id.contains('/') || id.contains('\\') {
        return not_found();
    }
    if sub.split('/').any(|c| c.is_empty() || c == "." || c == "..") || sub.contains('\\') {
        return not_found();
    }

    // The SDK is a special same-origin asset shared across all plugins.
    if sub == "sdk.js" {
        return match sdk_bytes(app) {
            Some(body) => built(200, "text/javascript", body),
            None => not_found(),
        };
    }

    for root in plugin_roots(app) {
        let candidate = root.join(id).join(&sub);
        if let Ok(body) = std::fs::read(&candidate) {
            return built(200, mime_for(&sub), body);
        }
    }
    not_found()
}
