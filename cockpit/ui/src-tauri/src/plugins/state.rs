use crate::plugins::manifest::{Manifest, Probe as ProbeCfg};
use crate::plugins::seams::{Clock, EventSink, Probe, Spawner};
use std::path::Path;

pub const STOPPED: &str = "stopped";
pub const BUILDING: &str = "building";
pub const STARTING: &str = "starting";
pub const HEALTH_PROBING: &str = "health-probing";
pub const READY_PROBING: &str = "ready-probing";
pub const HEALTHY: &str = "healthy";
pub const ERROR: &str = "error";

#[derive(Debug, PartialEq, Eq)]
pub enum StartOutcome {
    Healthy { owned: bool },
    Error(String),
}

/// Poll one probe until a status in `ok_status` appears or `timeout` elapses.
/// Returns true on success, false on timeout. Advances the injected clock.
/// Probe-first: at least one attempt is always made before the timeout is
/// checked, so even `timeout == 0` fires exactly one probe before giving up.
fn poll_until_ok(cfg: &ProbeCfg, probe: &dyn Probe, clock: &dyn Clock) -> bool {
    let start = clock.now_ms();
    loop {
        if let Some(code) = probe.probe(&cfg.url) {
            if cfg.ok_status.contains(&code) { return true; }
        }
        if clock.now_ms().saturating_sub(start) >= cfg.timeout { return false; }
        clock.sleep_ms(cfg.interval);
    }
}

fn both_probes_pass(m: &Manifest, probe: &dyn Probe) -> bool {
    let h = probe.probe(&m.lifecycle.health.url)
        .map(|c| m.lifecycle.health.ok_status.contains(&c)).unwrap_or(false);
    if !h { return false; }
    probe.probe(&m.lifecycle.ready.url)
        .map(|c| m.lifecycle.ready.ok_status.contains(&c)).unwrap_or(false)
}

#[allow(clippy::too_many_arguments)]
pub fn run_start_sequence(
    m: &Manifest, manifest_dir: &Path,
    probe: &dyn Probe, spawner: &dyn Spawner, clock: &dyn Clock, sink: &dyn EventSink,
    images_present: bool,
) -> StartOutcome {
    let cwd = m.resolved_cwd(manifest_dir);

    // Step 0: build (only if a build step exists and images are absent)
    if let Some(build) = &m.lifecycle.build {
        if !images_present {
            sink.emit_state(&m.id, BUILDING);
            let code = spawner.run_to_completion(&build.cmd, &cwd, &build.args, build.timeout);
            if code != 0 { sink.emit_state(&m.id, ERROR); return StartOutcome::Error(format!("build exited {code}")); }
        }
    }

    // Step 1: adopt check — both probes already up → adopt (not owned)
    if both_probes_pass(m, probe) {
        sink.emit_state(&m.id, HEALTHY);
        return StartOutcome::Healthy { owned: false };
    }

    // Step 2: spawn start (owned)
    sink.emit_state(&m.id, STARTING);
    let _child = spawner.spawn(&m.lifecycle.start, &cwd, &m.lifecycle.env);

    // Step 3: health then ready
    sink.emit_state(&m.id, HEALTH_PROBING);
    if !poll_until_ok(&m.lifecycle.health, probe, clock) {
        sink.emit_state(&m.id, ERROR); return StartOutcome::Error("health probe timed out".into());
    }
    sink.emit_state(&m.id, READY_PROBING);
    if !poll_until_ok(&m.lifecycle.ready, probe, clock) {
        sink.emit_state(&m.id, ERROR); return StartOutcome::Error("ready probe timed out".into());
    }

    // Step 4: healthy
    sink.emit_state(&m.id, HEALTHY);
    StartOutcome::Healthy { owned: true }
}

/// If the owned child has exited, emit `error` and return true. The caller is
/// responsible for destroying the kept-alive webview on a true return (§4).
pub fn check_crash(plugin_id: &str, child_id: u64, spawner: &dyn Spawner, sink: &dyn EventSink) -> bool {
    if spawner.has_exited(child_id) {
        sink.emit_state(plugin_id, ERROR);
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::manifest::Manifest;
    use crate::plugins::seams::fakes::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    fn manifest() -> Manifest {
        Manifest::from_json(r#"{"id":"audience","name":"Audience","apiVersion":1,
          "url":"http://localhost:3000",
          "lifecycle":{"cwd":"/x","build":{"cmd":"build","timeout":1000},
            "start":"up","stop":"down","env":{},
            "health":{"url":"h","okStatus":[200],"timeout":5000,"interval":1000},
            "ready":{"url":"r","okStatus":[200,302],"timeout":5000,"interval":1000}}}"#).unwrap()
    }

    #[test]
    fn cold_start_walks_building_to_healthy_and_owns_the_stack() {
        let probe = ScriptedProbe { responses: Mutex::new(HashMap::from([
            // adopt check: both down at first
            ("h".to_string(), vec![None, None, Some(200), Some(200)]),
            ("r".to_string(), vec![None, None, Some(302)]),
        ])) };
        let spawner = FakeSpawner { start_child_id: 7, build_exit: 0, exited: Mutex::new(false) };
        let clock = FakeClock::default();
        let sink = RecordingSink::default();

        let outcome = run_start_sequence(&manifest(), std::path::Path::new("/x"),
            &probe, &spawner, &clock, &sink, /*images_present=*/false);

        assert_eq!(outcome, StartOutcome::Healthy { owned: true });
        let states: Vec<String> = sink.states.lock().unwrap().iter().map(|(_, s)| s.clone()).collect();
        assert_eq!(states, vec!["building","starting","health-probing","ready-probing","healthy"]);
    }

    #[test]
    fn adopts_when_both_probes_already_pass_and_marks_not_owned() {
        let probe = ScriptedProbe { responses: Mutex::new(HashMap::from([
            ("h".to_string(), vec![Some(200)]),
            ("r".to_string(), vec![Some(200)]),
        ])) };
        let spawner = FakeSpawner { start_child_id: 7, build_exit: 0, exited: Mutex::new(false) };
        let out = run_start_sequence(&manifest(), std::path::Path::new("/x"),
            &probe, &spawner, &FakeClock::default(), &RecordingSink::default(), /*images_present=*/true);
        assert_eq!(out, StartOutcome::Healthy { owned: false });
    }

    #[test]
    fn partial_stack_health_only_falls_through_to_spawn() {
        // health up, ready down at adopt → must NOT adopt; spawn then both come up
        let probe = ScriptedProbe { responses: Mutex::new(HashMap::from([
            ("h".to_string(), vec![Some(200), Some(200)]),       // adopt(h) ok, later health-poll ok
            ("r".to_string(), vec![None, Some(200)]),            // adopt(r) down → fall through; ready-poll ok
        ])) };
        let spawner = FakeSpawner { start_child_id: 7, build_exit: 0, exited: Mutex::new(false) };
        let sink = RecordingSink::default();
        let out = run_start_sequence(&manifest(), std::path::Path::new("/x"),
            &probe, &spawner, &FakeClock::default(), &sink, true);
        assert_eq!(out, StartOutcome::Healthy { owned: true }); // spawned → owned
        let states: Vec<String> = sink.states.lock().unwrap().iter().map(|(_, s)| s.clone()).collect();
        assert!(states.contains(&"starting".to_string()));
    }

    #[test]
    fn health_timeout_yields_error() {
        let probe = ScriptedProbe { responses: Mutex::new(HashMap::from([
            ("h".to_string(), vec![None]),  // never comes up
            ("r".to_string(), vec![None]),
        ])) };
        let spawner = FakeSpawner { start_child_id: 7, build_exit: 0, exited: Mutex::new(false) };
        let sink = RecordingSink::default();
        let out = run_start_sequence(&manifest(), std::path::Path::new("/x"),
            &probe, &spawner, &FakeClock::default(), &sink, true);
        assert!(matches!(out, StartOutcome::Error(_)));
        assert_eq!(sink.states.lock().unwrap().last().unwrap().1, "error");
    }

    #[test]
    fn build_failure_yields_error_before_spawn() {
        let probe = ScriptedProbe { responses: Mutex::new(HashMap::new()) };
        let spawner = FakeSpawner { start_child_id: 7, build_exit: 2, exited: Mutex::new(false) };
        let sink = RecordingSink::default();
        let out = run_start_sequence(&manifest(), std::path::Path::new("/x"),
            &probe, &spawner, &FakeClock::default(), &sink, /*images_present=*/false);
        assert!(matches!(out, StartOutcome::Error(_)));
        assert_eq!(sink.states.lock().unwrap()[0].1, "building");
    }

    #[test]
    fn crash_while_healthy_transitions_to_error() {
        let spawner = FakeSpawner { start_child_id: 7, build_exit: 0, exited: Mutex::new(true) };
        let sink = RecordingSink::default();
        // Given an owned, healthy plugin whose child has exited, the watcher flips to error.
        let flipped = check_crash(&"audience".to_string(), 7, &spawner, &sink);
        assert!(flipped);
        assert_eq!(sink.states.lock().unwrap().last().unwrap().1, "error");
    }

    #[test]
    fn no_crash_when_child_alive() {
        let spawner = FakeSpawner { start_child_id: 7, build_exit: 0, exited: Mutex::new(false) };
        let sink = RecordingSink::default();
        let flipped = check_crash(&"audience".to_string(), 7, &spawner, &sink);
        assert!(!flipped);
        assert!(sink.states.lock().unwrap().is_empty());
    }
}
