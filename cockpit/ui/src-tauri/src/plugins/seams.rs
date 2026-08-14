use std::collections::BTreeMap;
use std::path::Path;

/// One HTTP probe attempt. Returns the status code, or None on connection error.
pub trait Probe: Send + Sync {
    fn probe(&self, url: &str) -> Option<u16>;
}

/// Spawns the build/start/stop commands. Returns a handle whose `is_alive`
/// the state machine polls and whose exit drives crash→error.
pub trait Spawner: Send + Sync {
    /// Run a command to completion (build/stop). Ok(code).
    /// `vars` are interpreted by the impl: for `start`/`stop` they are process
    /// env; for the build step they are the manifest's `build.args` (which the
    /// real impl must surface as Docker `--build-arg`, not env — see Phase 4).
    fn run_to_completion(
        &self,
        cmd: &str,
        cwd: &Path,
        vars: &BTreeMap<String, String>,
        timeout_ms: u64,
    ) -> i32;
    /// Spawn a long-running command (start). Returns a child id.
    fn spawn(&self, cmd: &str, cwd: &Path, env: &BTreeMap<String, String>) -> u64;
    /// Has the spawned child exited? (drives crash→error)
    fn has_exited(&self, child_id: u64) -> bool;
    /// Force-kill a child (teardown fallback).
    fn kill(&self, child_id: u64);
}

/// Monotonic time, injectable so timeouts test without sleeping.
pub trait Clock: Send + Sync {
    fn now_ms(&self) -> u64;
    fn sleep_ms(&self, ms: u64);
}

/// Where state transitions go (Tauri event in prod; a Vec in tests).
pub trait EventSink: Send + Sync {
    fn emit_state(&self, plugin_id: &str, state: &str);
}

#[cfg(test)]
pub mod fakes {
    use super::*;
    use std::sync::Mutex;

    /// Probe that returns a scripted sequence of statuses per URL.
    pub struct ScriptedProbe {
        pub responses: Mutex<std::collections::HashMap<String, Vec<Option<u16>>>>,
    }
    impl Probe for ScriptedProbe {
        fn probe(&self, url: &str) -> Option<u16> {
            let mut map = self.responses.lock().unwrap();
            let q = map.get_mut(url).expect("no script for url");
            if q.len() == 1 {
                q[0]
            } else {
                q.remove(0)
            }
        }
    }

    #[derive(Default)]
    pub struct FakeClock {
        pub t: Mutex<u64>,
    }
    impl Clock for FakeClock {
        fn now_ms(&self) -> u64 {
            *self.t.lock().unwrap()
        }
        fn sleep_ms(&self, ms: u64) {
            *self.t.lock().unwrap() += ms;
        } // advance, don't block
    }

    #[derive(Default)]
    pub struct RecordingSink {
        pub states: Mutex<Vec<(String, String)>>,
    }
    impl EventSink for RecordingSink {
        fn emit_state(&self, id: &str, s: &str) {
            self.states.lock().unwrap().push((id.into(), s.into()));
        }
    }

    pub struct FakeSpawner {
        pub start_child_id: u64,
        pub build_exit: i32,
        pub exited: Mutex<bool>,
    }
    impl Spawner for FakeSpawner {
        fn run_to_completion(
            &self,
            _c: &str,
            _w: &Path,
            _e: &BTreeMap<String, String>,
            _t: u64,
        ) -> i32 {
            self.build_exit
        }
        fn spawn(&self, _c: &str, _w: &Path, _e: &BTreeMap<String, String>) -> u64 {
            self.start_child_id
        }
        fn has_exited(&self, _id: u64) -> bool {
            *self.exited.lock().unwrap()
        }
        fn kill(&self, _id: u64) {}
    }
}
