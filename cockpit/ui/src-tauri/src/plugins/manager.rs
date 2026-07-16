use crate::plugins::discovery::{discover, DiscoveredPlugin};
use crate::plugins::manifest::Manifest;
use crate::plugins::seams::Probe;
use crate::plugins::seams_impl::{HttpProbe, RealClock, ShellSpawner, TauriEventSink};
use crate::plugins::state::{run_start_sequence, StartOutcome};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};
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
    /// Discovery roots (spec §2 seam: dev list ∪ user dir). Ordered so LATER roots
    /// win on `id` collision (discovery dedupes with later-wins): the dev-list root is
    /// placed first and the per-user dir last, so a user-installed plugin overrides a
    /// dev-checkout one of the same id. The dev-list root is opt-in via the
    /// `CC_APP_PLUGINS_DEV` env var (points at e.g. this repo's
    /// `cockpit/ui/src-tauri/app-plugins/`), keeping machine paths out of the binary.
    pub fn roots() -> Vec<PathBuf> {
        let mut v = Vec::new();
        if let Some(dev) = std::env::var_os("CC_APP_PLUGINS_DEV") {
            v.push(PathBuf::from(dev));
        }
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

    /// Blocking teardown of every OWNED plugin, run concurrently (one thread each),
    /// bounded by a single TOTAL deadline (kept under the OS force-kill ceiling for a
    /// graceful exit). Adopted (not-owned) stacks are left running — the user started
    /// those by hand. Threads still running at the deadline are abandoned (best-effort
    /// fallback; documented known gap for force-quit).
    pub fn stop_all_owned(&self, total_deadline_ms: u64) {
        let running = std::mem::take(&mut *self.running.lock().unwrap());
        let discovered = self.discovered.lock().unwrap().clone();
        let deadline = Instant::now() + Duration::from_millis(total_deadline_ms);

        let handles: Vec<_> = running
            .into_iter()
            .filter(|(_, r)| r.owned)
            .filter_map(|(id, _r)| {
                let found = discovered
                    .iter()
                    .find(|d| d.manifest.id == id)
                    .map(|d| (d.manifest.clone(), d.dir.clone()))?;
                Some(std::thread::spawn(move || stop_one(&found.0, &found.1)))
            })
            .collect();

        for h in handles {
            if Instant::now() >= deadline {
                break; // total budget exhausted
            }
            let _ = h.join(); // best-effort within the budget
        }
    }
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
}

/// Run a plugin's `lifecycle.stop` command (if any) to completion in its resolved cwd.
/// Compose stacks are detached processes, so `docker compose down` is the real teardown —
/// shelling out directly here rather than going through `spawner.kill`, which only reaches
/// child PIDs that were captured at launch time.
fn stop_one(m: &Manifest, dir: &std::path::Path) {
    if let Some(stop) = &m.lifecycle.stop {
        let cwd = m.resolved_cwd(dir);
        let mut parts = stop.split_whitespace();
        if let Some(prog) = parts.next() {
            let _ = std::process::Command::new(prog)
                .args(parts)
                .current_dir(&cwd)
                .status();
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::discovery::discover;
    use crate::plugins::manifest::Popups;
    use std::path::Path;

    /// The shipped Audience proving manifest lives in the repo (dev-list root) and must
    /// parse, validate, and carry the credential-free dev posture (spec §2 + audience
    /// digest): fake providers baked as BUILD args, and devAuth selected at runtime env.
    #[test]
    fn shipped_audience_manifest_is_credential_free_dev() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("app-plugins/audience/app-plugin.json");
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        let m = Manifest::from_json(&text).expect("audience manifest parses");
        m.validate().expect("audience manifest validates");

        assert_eq!(m.id, "audience");
        let lc = &m.lifecycle;

        // Fake providers are BUILD args (baked into the image — a runtime env can't flip a
        // prod-built Next image to devAuth/fake; spec §2 "build vs runtime env").
        let build = lc.build.as_ref().expect("audience has a build step");
        assert_eq!(build.args.get("NODE_ENV").map(String::as_str), Some("development"));
        assert_eq!(build.args.get("AI_PROVIDER").map(String::as_str), Some("fake"));
        assert_eq!(build.args.get("MEDIA_PROVIDER").map(String::as_str), Some("fake"));

        // devAuth is selected at runtime (NODE_ENV) and fabricates an identity from
        // DEV_WORKSPACE_ID/DEV_USER_ID (audience digest) — so no Clerk cookie is needed.
        assert_eq!(lc.env.get("NODE_ENV").map(String::as_str), Some("development"));
        assert!(lc.env.contains_key("DEV_WORKSPACE_ID"));
        assert!(lc.env.contains_key("DEV_USER_ID"));

        // Audience root `/` 302-redirects to /dashboard → the ready probe must accept 3xx
        // or a perfectly healthy Next server is marked `error` (spec §2, critique R1 #3).
        assert!(lc.ready.ok_status.contains(&302));
        assert_eq!(lc.health.ok_status, vec![200]);

        // OAuth popups must share the app's session partition (spec §4) → popups allowed.
        assert_eq!(m.webview.popups, Popups::Allow);
    }

    /// `CC_APP_PLUGINS_DEV` adds a dev-list discovery root; discovery finds a manifest
    /// placed under it. (Verifies the dev seam wired into `roots()` end-to-end.)
    #[test]
    fn dev_list_root_is_discovered() {
        let tmp = std::env::temp_dir().join("appplugins_devroot_test");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("audience")).unwrap();
        let text = std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("app-plugins/audience/app-plugin.json"),
        )
        .unwrap();
        std::fs::write(tmp.join("audience/app-plugin.json"), text).unwrap();

        let found = discover(&[tmp.as_path()]);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].manifest.id, "audience");
    }
}
