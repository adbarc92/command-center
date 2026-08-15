use crate::plugins::discovery::{discover, DiscoveredPlugin};
use crate::plugins::manifest::Manifest;
use crate::plugins::seams::Probe;
use crate::plugins::seams_impl::{HttpProbe, RealClock, ShellSpawner, TauriEventSink};
use crate::plugins::state::{run_start_sequence, StartOutcome};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager, State};

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

        let handles: Vec<_> = teardown_targets(&running, &discovered)
            .into_iter()
            .map(|(m, dir)| std::thread::spawn(move || stop_one(&m, &dir)))
            .collect();

        for h in handles {
            if Instant::now() >= deadline {
                break; // total budget exhausted
            }
            let _ = h.join(); // best-effort within the budget
        }
    }
}

/// Which plugins a teardown pass must stop: OWNED entries resolved against discovery.
/// Adopted (not-owned) stacks are excluded — the user started those by hand — and a running
/// id with no surviving discovery record is skipped rather than panicking.
///
/// Split out of `stop_all_owned` so the *selection* half of Gate 5 is unit-testable. The
/// *execution* half (`stop_one`, which shells out to the manifest's `docker compose down`)
/// needs a Docker daemon; CI has none, so that half stays a human smoke gate.
fn teardown_targets(
    running: &HashMap<String, Running>,
    discovered: &[DiscoveredPlugin],
) -> Vec<(Manifest, PathBuf)> {
    running
        .iter()
        .filter(|(_, r)| r.owned)
        .filter_map(|(id, _r)| {
            discovered
                .iter()
                .find(|d| &d.manifest.id == id)
                .map(|d| (d.manifest.clone(), d.dir.clone()))
        })
        .collect()
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

/// Dispatch a plugin's start sequence onto a background thread and return immediately.
///
/// PHASE-6 SMOKE FINDING (checklist 1.5). This used to call `run_start_sequence` inline, with
/// a standing note that it "may block up to the probe timeout (~180 s)". The smoke proved it
/// out: a *synchronous* Tauri command runs on the main event-loop thread — the same P3 finding
/// that forced the embedding commands to be `async` (see `embedding.rs`) — and the sequence
/// blocks on `docker compose build` (a 20-minute budget for Audience) plus the health and
/// ready probe budgets. The entire UI froze from the tab click until the stack came up.
///
/// A dedicated OS thread, deliberately not an async-runtime worker: every seam in the sequence
/// is blocking (`Command::status`, `ureq`, `thread::sleep`), so handing it to the runtime would
/// starve a worker and merely relocate the stall.
///
/// **Ok means "dispatched", not "healthy".** Lifecycle truth reaches the shell only through the
/// `plugin://state` events the sink emits, which `App.svelte` already subscribes to; it must not
/// treat this returning as readiness. Pinned by `src/App.appPlugin.test.ts`.
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

    // `mgr` is borrowed from this invocation and cannot cross the thread boundary; the handle
    // can, so the worker re-acquires the manager from it.
    std::thread::spawn(move || {
        let mgr = app.state::<PluginManager>();
        let probe = HttpProbe;
        let clock = RealClock::new();
        let sink = TauriEventSink { app: app.clone() };
        let images_present = probe.probe(&disc.manifest.lifecycle.health.url).is_some();

        let outcome = run_start_sequence(
            &disc.manifest,
            &disc.dir,
            &probe,
            &mgr.spawner,
            &clock,
            &sink,
            images_present,
        );
        if let StartOutcome::Healthy { owned, child_id } = outcome {
            mgr.running
                .lock()
                .unwrap()
                .insert(disc.manifest.id.clone(), Running { child_id, owned });
        }
        // On Error the sink has already emitted `error`; the shell reacts to that event. There
        // is no caller left to return it to.
    });

    Ok(())
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

    // ---- Gate 5: teardown selection -------------------------------------------------
    //
    // Gate 5 ("quit the app, confirm `docker ps` is clean") is the one merge blocker on the
    // plugin runtime that had no automated coverage at all. It splits in two: WHICH stacks a
    // teardown pass picks, and WHETHER `docker compose down` actually brings them down. Only
    // the first half can be tested without a Docker daemon — CI has none — so that is what
    // these pin. The execution half stays a human smoke item.

    /// A manifest with no `stop` command, so `stop_one` is a no-op and these tests never
    /// shell out to anything.
    fn manifest(id: &str) -> Manifest {
        let text = serde_json::json!({
            "id": id, "name": id, "apiVersion": 1,
            "url": "http://localhost:3000",
            "lifecycle": {
                "cwd": "/x", "start": "up", "env": {},
                "health": { "url": "h", "okStatus": [200], "timeout": 5000, "interval": 1000 },
                "ready":  { "url": "r", "okStatus": [200], "timeout": 5000, "interval": 1000 }
            }
        })
        .to_string();
        Manifest::from_json(&text).expect("fixture manifest parses")
    }

    fn discovered(ids: &[&str]) -> Vec<DiscoveredPlugin> {
        ids.iter()
            .map(|id| DiscoveredPlugin {
                dir: PathBuf::from(format!("/plugins/{id}")),
                manifest: manifest(id),
            })
            .collect()
    }

    fn running(entries: &[(&str, bool)]) -> HashMap<String, Running> {
        entries
            .iter()
            .map(|(id, owned)| {
                (
                    (*id).to_string(),
                    Running { child_id: Some(1), owned: *owned },
                )
            })
            .collect()
    }

    /// Only OWNED stacks are torn down. An adopted stack — one that was already up when the
    /// start sequence ran, so the user owns it — must be left running on quit.
    #[test]
    fn teardown_selects_owned_and_leaves_adopted_running() {
        let targets = teardown_targets(
            &running(&[("audience", true), ("adopted", false)]),
            &discovered(&["audience", "adopted"]),
        );
        let ids: Vec<&str> = targets.iter().map(|(m, _)| m.id.as_str()).collect();
        assert_eq!(ids, vec!["audience"]);
    }

    /// A running id whose discovery record has vanished (plugin dir deleted or renamed while
    /// the app was up) is skipped, not panicked on. Teardown runs inside the shutdown handler,
    /// where a panic would strand every container that had not been reached yet.
    #[test]
    fn teardown_skips_running_id_with_no_discovery_record() {
        let targets = teardown_targets(
            &running(&[("audience", true), ("vanished", true)]),
            &discovered(&["audience"]),
        );
        let ids: Vec<&str> = targets.iter().map(|(m, _)| m.id.as_str()).collect();
        assert_eq!(ids, vec!["audience"]);
    }

    /// EVERY owned stack is selected, not just the first — a partial teardown is precisely the
    /// orphaned-container failure Gate 5 exists to catch.
    #[test]
    fn teardown_selects_every_owned_stack() {
        let targets = teardown_targets(
            &running(&[("a", true), ("b", true), ("c", true)]),
            &discovered(&["a", "b", "c"]),
        );
        let mut ids: Vec<&str> = targets.iter().map(|(m, _)| m.id.as_str()).collect();
        ids.sort_unstable();
        assert_eq!(ids, vec!["a", "b", "c"]);
    }

    /// The resolved teardown cwd is the manifest's, not the process's — `docker compose down`
    /// only finds the right stack if it runs where the compose file lives.
    #[test]
    fn teardown_target_carries_the_plugins_own_directory() {
        let targets = teardown_targets(&running(&[("audience", true)]), &discovered(&["audience"]));
        let (m, dir) = &targets[0];
        assert_eq!(dir, &PathBuf::from("/plugins/audience"));
        assert_eq!(m.resolved_cwd(dir), PathBuf::from("/x")); // absolute cwd wins over the dir
    }

    /// `stop_all_owned` empties the running map — including adopted entries, which are
    /// forgotten rather than stopped. Documents the `mem::take`: after one pass the manager
    /// tracks nothing.
    #[test]
    fn stop_all_owned_clears_the_running_map_including_adopted() {
        let mgr = PluginManager::default();
        *mgr.discovered.lock().unwrap() = discovered(&["audience", "adopted"]);
        *mgr.running.lock().unwrap() = running(&[("audience", true), ("adopted", false)]);

        mgr.stop_all_owned(5_000);

        assert!(mgr.running.lock().unwrap().is_empty());
    }

    /// Shutdown re-entrancy. `stop_all_owned` is called from the `ExitRequested` handler on the
    /// main event-loop thread (`lib.rs`) with a 30 s budget, so a second pass must not re-spend
    /// it. Bears on the open Gate-5 process-exit anomaly (the `app` process outliving the
    /// window): whatever holds the process open, this test rules out a teardown pass blocking
    /// the loop a second time.
    #[test]
    fn stop_all_owned_is_idempotent_and_the_second_pass_is_immediate() {
        let mgr = PluginManager::default();
        *mgr.discovered.lock().unwrap() = discovered(&["audience"]);
        *mgr.running.lock().unwrap() = running(&[("audience", true)]);

        mgr.stop_all_owned(5_000);
        let t = Instant::now();
        mgr.stop_all_owned(5_000);

        assert!(
            t.elapsed() < Duration::from_millis(250),
            "second teardown pass took {:?}; it should find nothing to do",
            t.elapsed()
        );
        assert!(mgr.running.lock().unwrap().is_empty());
    }
}
