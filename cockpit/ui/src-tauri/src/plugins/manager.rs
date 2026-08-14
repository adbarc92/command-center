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
    // Recorded at launch so the Phase-6 crash watcher can poll this child; teardown
    // itself goes through `lifecycle.stop`, so nothing reads it back yet.
    #[allow(dead_code)]
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
    // That layer is the only caller and has not landed yet.
    #[allow(dead_code)]
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
