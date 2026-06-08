use crate::plugins::seams::{Clock, EventSink, Probe, Spawner};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

pub struct HttpProbe;
impl Probe for HttpProbe {
    fn probe(&self, url: &str) -> Option<u16> {
        match ureq::get(url).timeout(Duration::from_millis(2000)).call() {
            Ok(resp) => Some(resp.status()),
            Err(ureq::Error::Status(code, _)) => Some(code), // 3xx/4xx still a status
            Err(_) => None,                                   // connection refused etc.
        }
    }
}

pub struct RealClock {
    start: Instant,
}
impl RealClock {
    pub fn new() -> Self {
        RealClock {
            start: Instant::now(),
        }
    }
}
impl Default for RealClock {
    fn default() -> Self {
        Self::new()
    }
}
impl Clock for RealClock {
    fn now_ms(&self) -> u64 {
        self.start.elapsed().as_millis() as u64
    }
    fn sleep_ms(&self, ms: u64) {
        std::thread::sleep(Duration::from_millis(ms));
    }
}

pub struct TauriEventSink {
    pub app: AppHandle,
}
impl EventSink for TauriEventSink {
    fn emit_state(&self, plugin_id: &str, state: &str) {
        let _ = self.app.emit(
            "plugin://state",
            serde_json::json!({ "id": plugin_id, "state": state }),
        );
    }
}

/// Spawner over std::process. Compose stacks are detached daemons; we track the
/// `up`/`build`/`down` invocation, not container PIDs. Child ids index a table.
#[derive(Default)]
pub struct ShellSpawner {
    children: Mutex<Vec<Option<std::process::Child>>>,
}
impl Spawner for ShellSpawner {
    /// Used for the BUILD step: `vars` are the manifest's `build.args` and are
    /// surfaced as Docker `--build-arg K=V` flags appended to the command, NOT
    /// as process env (Audience bakes auth/origin at build time; env would not
    /// reach the image). Validated end-to-end in the Phase-6 smoke.
    fn run_to_completion(
        &self,
        cmd: &str,
        cwd: &Path,
        vars: &BTreeMap<String, String>,
        _timeout: u64,
    ) -> i32 {
        let mut c = base_command(cmd, cwd);
        for (k, v) in vars {
            c.arg("--build-arg").arg(format!("{k}={v}"));
        }
        c.status()
            .map(|s| s.code().unwrap_or(-1))
            .unwrap_or(-1)
    }

    /// Used for the START step: `env` are runtime env vars applied to the process.
    fn spawn(&self, cmd: &str, cwd: &Path, env: &BTreeMap<String, String>) -> u64 {
        let mut c = base_command(cmd, cwd);
        for (k, v) in env {
            c.env(k, v);
        }
        let child = c.spawn().ok();
        let mut t = self.children.lock().unwrap();
        t.push(child);
        (t.len() - 1) as u64
    }

    fn has_exited(&self, id: u64) -> bool {
        let mut t = self.children.lock().unwrap();
        match t.get_mut(id as usize).and_then(|c| c.as_mut()) {
            Some(child) => matches!(child.try_wait(), Ok(Some(_))),
            None => true,
        }
    }

    fn kill(&self, id: u64) {
        let mut t = self.children.lock().unwrap();
        if let Some(Some(child)) = t.get_mut(id as usize) {
            let _ = child.kill();
        }
    }
}

/// Split a whitespace-delimited command string into program + args, set cwd.
/// (Our manifest commands are quote-free, e.g. `docker compose -f x.yml up`.)
fn base_command(cmd: &str, cwd: &Path) -> std::process::Command {
    let mut parts = cmd.split_whitespace();
    let prog = parts.next().unwrap_or("");
    let mut c = std::process::Command::new(prog);
    c.args(parts).current_dir(cwd);
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_command_splits_program_and_args() {
        let c = base_command(
            "docker compose -f docker-compose.prod.yml up",
            Path::new("/x"),
        );
        assert_eq!(c.get_program(), "docker");
        let args: Vec<_> = c
            .get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect();
        assert_eq!(args, vec!["compose", "-f", "docker-compose.prod.yml", "up"]);
    }
}
