// THROWAWAY P4 SPIKE HARNESS (Rust side) — do NOT merge to main.
//
// Proves the make-or-break dependency of the sandboxed view-plugin runtime
// (see docs/superpowers/specs/2026-06-07-view-plugins-design.md §"Sandbox, CSP, loader"):
// an untrusted `<iframe sandbox="allow-scripts">` (opaque "null" origin) served from a
// custom URI scheme can render, complete the plugin-hello→init MessagePort handshake, load
// its own ES-module graph, and be fully network-blocked by a strict CSP.
//
// This module registers the `ccplugin://` scheme handler. On Windows/WebView2 (primary
// target) the scheme resolves to the distinct origin `http://ccplugin.localhost/...`;
// every plugin shares that origin (`<id>` would be a path in the real runtime — the spike
// serves a single embedded plugin). The handler is registered on the Tauri builder, so it
// is available in BOTH `tauri dev` and a packaged `tauri build` (it is a wry-runtime
// concern, independent of where the host app HTML is served from).
//
// The plugin-document CSP is delivered as a RESPONSE HEADER here — NOT a <meta> the plugin
// could control — exactly as the real runtime would. The host-app CSP (frame-src) is a
// SEPARATE gate (d): left as `csp: null` so the harness runs out of the box; the candidate
// value to test gate (d) lives in the spike runbook, not in tauri.conf.json.

use tauri::http::{Request, Response};

/// The plugin-document Content-Security-Policy, verbatim from the design spec.
/// `connect-src 'none'` is what makes gate c-CSP (no network) provable; `script-src 'self'`
/// permits the plugin's own code and blocks inline/eval.
const PLUGIN_CSP: &str = "default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'none'; frame-src 'none'; base-uri 'none'; form-action 'none'";

// The single embedded plugin, baked into the binary so the spike has no filesystem or
// bundling dependency and behaves identically in dev and packaged builds.
const INDEX_HTML: &str = include_str!("spike-view-plugin/index.html");
const SDK_JS: &str = include_str!("spike-view-plugin/sdk.js");
const PROBE_JS: &str = include_str!("spike-view-plugin/probe.js");

/// Custom URI scheme the host iframe points at. Register on the Tauri builder via
/// `.register_uri_scheme_protocol(spike_view_plugins::SCHEME, |_ctx, req| respond(req))`.
pub const SCHEME: &str = "ccplugin";

/// Serve one of the embedded plugin assets with the strict plugin-doc CSP header.
///
/// NOTE on gate c-CORS: no `Access-Control-Allow-Origin` header is set on purpose, so the
/// spike measures the *default* opaque-origin module-import behavior. If the watched run
/// shows `corsOk=false`, the in-place remedy is to add `Access-Control-Allow-Origin: *`
/// here (or adopt spec fallback #1, a single inlined bundle).
pub fn respond(request: Request<Vec<u8>>) -> Response<Vec<u8>> {
    let path = request.uri().path();
    let (body, mime): (&str, &str) = match path {
        "" | "/" | "/index.html" => (INDEX_HTML, "text/html"),
        "/sdk.js" => (SDK_JS, "text/javascript"),
        "/probe.js" => (PROBE_JS, "text/javascript"),
        _ => {
            return Response::builder()
                .status(404)
                .header("Content-Type", "text/plain")
                .header("Content-Security-Policy", PLUGIN_CSP)
                .body(b"cc view-plugin spike: not found".to_vec())
                .expect("static 404 response is always well-formed");
        }
    };

    Response::builder()
        .status(200)
        .header("Content-Type", mime)
        .header("Content-Security-Policy", PLUGIN_CSP)
        .body(body.as_bytes().to_vec())
        .expect("static asset response is always well-formed")
}
