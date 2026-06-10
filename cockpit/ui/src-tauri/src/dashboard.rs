//! Dashboard read seams that the browser can't reach directly (spec §7, §6.2):
//!  - Halyard CLI-spawn adapter: run `halyard status` / `halyard queue`, both of
//!    which print JSON to stdout, and pass that JSON through to the frontend.
//!  - Audience HTTP poll: `GET /health` + `GET /posts` from the Rust side so the
//!    desktop host (not a sandboxed webview) owns the devAuth/CORS call.
//!
//! Read-only by construction (locked decision #4): these commands only read. They
//! degrade gracefully — a missing CLI / unreachable backend returns a typed error
//! the adapters turn into a greyed `health: "unknown"` lane, never a wrong stage.
//!
//! Config is environment-driven so the shell can point at a real install:
//!  - HALYARD_BIN (default "halyard") — the CLI to spawn.
//!  - HALYARD_CONFIG_DIR (default ".") — CWD for the spawn; Halyard resolves config
//!    relative to CWD.
//!  - AUDIENCE_API_URL (default "http://localhost:8080").

use serde_json::Value;
use std::process::Command;

fn halyard_bin() -> String {
    std::env::var("HALYARD_BIN").unwrap_or_else(|_| "halyard".to_string())
}

fn halyard_cwd() -> String {
    std::env::var("HALYARD_CONFIG_DIR").unwrap_or_else(|_| ".".to_string())
}

fn audience_base() -> String {
    std::env::var("AUDIENCE_API_URL").unwrap_or_else(|_| "http://localhost:8080".to_string())
}

/// Spawn `halyard <subcommand>` and parse its stdout as JSON. The CLI prints the
/// machine result to stdout and human/log lines to stderr (Halyard digest), so we
/// only parse stdout. Any failure (binary absent, non-JSON, non-zero exit) is a
/// typed error string — the caller greys the lane.
fn run_halyard(subcommand: &str) -> Result<Value, String> {
    let out = Command::new(halyard_bin())
        .arg(subcommand)
        .current_dir(halyard_cwd())
        .output()
        .map_err(|e| format!("halyard spawn failed: {e}"))?;

    if !out.status.success() {
        let code = out.status.code().unwrap_or(-1);
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(format!("halyard {subcommand} exited {code}: {}", stderr.trim()));
    }

    let stdout = String::from_utf8_lossy(&out.stdout);
    serde_json::from_str::<Value>(stdout.trim())
        .map_err(|e| format!("halyard {subcommand} stdout was not JSON: {e}"))
}

/// `halyard status` → an array of release status objects (see HalyardReleaseStatus).
#[tauri::command]
pub fn halyard_status() -> Result<Value, String> {
    run_halyard("status")
}

/// `halyard queue` → an array of open proposals (see HalyardProposal).
#[tauri::command]
pub fn halyard_queue() -> Result<Value, String> {
    run_halyard("queue")
}

/// `GET {AUDIENCE_API_URL}/health` → true iff the backend answers 2xx (§6.2: a down
/// `/health` means the whole stack isn't running ⇒ Idle + unknown on the board).
#[tauri::command]
pub async fn audience_health() -> Result<bool, String> {
    let url = format!("{}/health", audience_base());
    match reqwest::Client::new().get(&url).send().await {
        Ok(resp) => Ok(resp.status().is_success()),
        // A connection error is "backend down", not a hard failure — report false.
        Err(_) => Ok(false),
    }
}

/// `GET {AUDIENCE_API_URL}/posts` → the posts array (passed through to the adapter).
/// Accepts either a bare array or a `{ posts: [...] }` / `{ data: [...] }` envelope.
#[tauri::command]
pub async fn audience_posts() -> Result<Value, String> {
    let url = format!("{}/posts", audience_base());
    let resp = reqwest::Client::new()
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("audience /posts request failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("audience /posts returned {}", resp.status()));
    }
    let body: Value = resp
        .json()
        .await
        .map_err(|e| format!("audience /posts body was not JSON: {e}"))?;

    Ok(unwrap_posts(body))
}

/// Normalize the `/posts` body to a JSON array — tolerate a bare array or a common
/// `{ posts | data | items: [...] }` envelope so the adapter always sees a list.
fn unwrap_posts(body: Value) -> Value {
    if body.is_array() {
        return body;
    }
    if let Value::Object(map) = &body {
        for key in ["posts", "data", "items"] {
            if let Some(arr @ Value::Array(_)) = map.get(key) {
                return arr.clone();
            }
        }
    }
    Value::Array(vec![])
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn unwrap_posts_passes_bare_array_through() {
        let arr = json!([{ "id": "1", "status": "draft" }]);
        assert_eq!(unwrap_posts(arr.clone()), arr);
    }

    #[test]
    fn unwrap_posts_extracts_posts_envelope() {
        let body = json!({ "posts": [{ "id": "1", "status": "published" }], "total": 1 });
        assert_eq!(
            unwrap_posts(body),
            json!([{ "id": "1", "status": "published" }])
        );
    }

    #[test]
    fn unwrap_posts_extracts_data_and_items_envelopes() {
        assert_eq!(
            unwrap_posts(json!({ "data": [{ "id": "a" }] })),
            json!([{ "id": "a" }])
        );
        assert_eq!(
            unwrap_posts(json!({ "items": [{ "id": "b" }] })),
            json!([{ "id": "b" }])
        );
    }

    #[test]
    fn unwrap_posts_falls_back_to_empty_array() {
        assert_eq!(unwrap_posts(json!({ "unexpected": true })), json!([]));
        assert_eq!(unwrap_posts(json!("nope")), json!([]));
    }

    #[test]
    fn env_defaults_are_sane() {
        // Defaults must not panic and must produce usable strings.
        assert!(!halyard_bin().is_empty());
        assert!(audience_base().starts_with("http"));
    }
}
