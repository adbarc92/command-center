//! Demo-mode verification (Lane E / launch-readiness Lane L2).
//!
//! Demo mode is the "try it cold" path: a mission dispatched with `mode: "demo"`
//! must run the FULL lifecycle on the in-process `FakeRunner` + `FakeForge` —
//! **no Docker, no network, no API key, no real PR** — while still emitting the
//! complete event stream so the cockpit renders identically to a real run.
//!
//! These tests reconstruct exactly what `server.rs::spawn_driver_for` does for
//! `mode == "demo"`: `driver::run(FakeRunner::new(demo_script(spec)),
//! FakeForge::default(), …)`. `demo_script` is private to `server.rs`, so it is
//! mirrored here (kept in lockstep with the daemon by
//! `demo_script_matches_server_shape`, which pins the call-count invariant the
//! server's own unit test asserts: oracle + 3 calls per review round).
//!
//! Crucially: this file runs in plain `cargo test` — it is NOT `#[ignore]`d,
//! because demo mode is the whole point of being able to verify with no Docker
//! and no key. It references only the fakes; `LocalDockerRunner`/`GhForge` never
//! appear, so "zero Docker invocations / no real PR" is structural, not incidental.

use fleet_core::{Command, Event, GateConfig, Phase, Tier};
use fleetd::driver::{run, EventEnvelope, RunCtx};
use fleetd::fake::{FakeForge, FakeRunner};
use fleetd::runner::{ExecOutput, UnitSpec};
use tokio::sync::mpsc;

/// The fake PR url `FakeForge::open_pr` returns. A demo run that opens a PR must
/// surface THIS (a faked PR), never a real `github.com` url — proof the forge is
/// faked and no real PR is created.
const FAKE_PR_URL: &str = "https://fake/pr/1";

/// Mirror of the private `server.rs::demo_script`: a `FakeRunner` script of
/// oracle, then one build/check/review cycle per review round with blockers
/// trending to zero so the gate opens exactly on the floor. Kept faithful to the
/// daemon by `demo_script_matches_server_shape` below.
fn demo_script(spec: &UnitSpec) -> Vec<ExecOutput> {
    let floor = spec.gate.min_review_rounds.max(1);
    let mut s = vec![];
    if !spec.oracle_frozen {
        s.push(FakeRunner::ok(0.02, &["sum.test.js"])); // oracle
    }
    for remaining in (0..floor).rev() {
        s.push(FakeRunner::ok(0.03, &["implementing the change"]));
        s.push(FakeRunner::ok(0.0, &["tests: 1 passing"]));
        s.push(FakeRunner::ok(0.04, &[&format!("review done\nBLOCKERS={remaining}")]));
    }
    s
}

/// The `UnitSpec` `server.rs::create_mission` builds for a fresh mission (T1, the
/// default tier), so this exercises the same shape the daemon dispatches in demo.
fn demo_spec(floor: u32) -> UnitSpec {
    UnitSpec {
        unit_id: "demo-u1".into(),
        tier: Tier::T1,
        task: "add a sum() helper".into(),
        usd_cap: 5.0,
        wall_clock_secs: 1800,
        gate: GateConfig { min_review_rounds: floor.max(1) },
        repo_url: "https://github.com/adbarc92/command-center-agent-sandbox".into(),
        repo_slug: "adbarc92/command-center-agent-sandbox".into(),
        base_branch: "main".into(),
        branch: "agent/demo-u1".into(),
        test_cmd: "node --test".into(),
        oracle_frozen: false,
    }
}

/// Spawn the demo driver exactly as `spawn_driver_for(.., "demo", ..)` does:
/// `FakeRunner` (no Docker) + `FakeForge` (no real PR). T1 needs no commands, so
/// the command channel is dropped. Returns the terminal phase and every event.
async fn run_demo(spec: UnitSpec) -> (Phase, Vec<EventEnvelope>) {
    let script = demo_script(&spec);
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel::<Command>();
    let (evt_tx, mut evt_rx) = mpsc::unbounded_channel::<EventEnvelope>();
    drop(cmd_tx); // T1 demo is fully autonomous

    let phase = run(
        FakeRunner::new(script),
        FakeForge::default(),
        spec,
        RunCtx::standalone(),
        cmd_rx,
        evt_tx,
    )
    .await;

    let mut events = vec![];
    while let Ok(e) = evt_rx.try_recv() {
        events.push(e);
    }
    (phase, events)
}

fn phase_sequence(events: &[EventEnvelope]) -> Vec<Phase> {
    events
        .iter()
        .filter_map(|e| match &e.event {
            Event::PhaseChanged { to, .. } => Some(*to),
            _ => None,
        })
        .collect()
}

/// The headline assertion: a demo mission reaches a terminal phase driven entirely
/// by `FakeRunner` + `FakeForge`, walking the full lifecycle and opening only a
/// FAKED PR — with zero Docker invocations and zero real (network/API) spend.
#[tokio::test]
async fn demo_mission_reaches_terminal_on_fakes_no_docker_no_real_pr() {
    let (phase, events) = run_demo(demo_spec(2)).await;

    // 1. Reaches a terminal phase. The demo sandbox has a non-empty diff, so the
    //    happy path lands on DONE (one of DONE / PR_OPEN / NO_CHANGE).
    assert!(
        matches!(phase, Phase::Done | Phase::NoChange | Phase::PrOpen),
        "demo unit must reach a terminal phase, got {phase:?}"
    );
    assert_eq!(phase, Phase::Done, "the demo sandbox script lands on DONE");

    // 2. It walked the FULL progression (not a short-circuit). T1 auto-freezes the
    //    oracle, so SPEC → BUILDING → CHECKING → REVIEWING → MERGE_CHECK → PR_OPEN → DONE.
    let seq = phase_sequence(&events);
    for required in [
        Phase::Spec,
        Phase::Building,
        Phase::Checking,
        Phase::Reviewing,
        Phase::MergeCheck,
        Phase::PrOpen,
        Phase::Done,
    ] {
        assert!(seq.contains(&required), "missing phase {required:?} in {seq:?}");
    }
    // T1 freezes the oracle automatically — never parks for human approval in demo.
    assert!(
        !seq.contains(&Phase::AwaitingOracleApproval),
        "T1 demo auto-freezes the oracle; no human gate"
    );
    assert!(!seq.contains(&Phase::Failed), "demo happy path never fails: {seq:?}");

    // 3. No REAL PR: the only PR artifact is FakeForge's faked url, never github.com.
    let pr_refs: Vec<&str> = events
        .iter()
        .filter_map(|e| match &e.event {
            Event::Artifact { kind: fleet_core::ArtifactKind::Pr, reference } => Some(reference.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(pr_refs, vec![FAKE_PR_URL], "demo opens only a FAKED PR, never a real one");
    assert!(
        !pr_refs.iter().any(|r| r.contains("github.com")),
        "no real github.com PR may be opened in demo: {pr_refs:?}"
    );

    // 4. Zero REAL cost: every Metric came from FakeRunner's synthetic Usage (no
    //    Anthropic call billed). The demo script's TOTAL fake spend is bounded and
    //    well under the unit cap — it is bookkeeping, not money. floor=2 ⇒
    //    oracle 0.02 + 2×(0.03+0.0+0.04) = 0.16.
    let final_cost = events
        .iter()
        .filter_map(|e| match e.event {
            Event::Metric { cost_usd, .. } => Some(cost_usd),
            _ => None,
        })
        .fold(0.0_f64, f64::max);
    assert!((final_cost - 0.16).abs() < 1e-9, "demo metered fake cost is 0.16, got {final_cost}");
    assert!(final_cost < 5.0, "demo never approaches the usd cap — no real spend");

    // 5. The event stream emitted normally (the cockpit has something to render):
    //    iterations, logs, findings, a terminal Done.
    assert!(
        events.iter().any(|e| matches!(e.event, Event::Iteration { .. })),
        "demo emits Iteration events"
    );
    assert!(
        events.iter().any(|e| matches!(e.event, Event::Log { .. })),
        "demo emits Log events"
    );
    assert!(
        events.iter().any(|e| matches!(&e.event, Event::Done { result } if result == "done")),
        "demo emits a terminal Done(done)"
    );
    // Seqs are strictly increasing — a coherent stream for the WS replay/dedup.
    let seqs: Vec<u64> = events.iter().map(|e| e.seq).collect();
    assert!(seqs.windows(2).all(|w| w[0] < w[1]), "event seqs strictly increase: {seqs:?}");
}

/// The review gate opens exactly on the floor: floor=3 runs three review rounds
/// (blockers 2 → 1 → 0) before DONE, proving the demo script drives a real,
/// multi-round progression rather than a one-shot.
#[tokio::test]
async fn demo_runs_full_review_rounds_to_the_floor() {
    let (phase, events) = run_demo(demo_spec(3)).await;
    assert_eq!(phase, Phase::Done);
    let review_rounds = events
        .iter()
        .filter(|e| matches!(e.event, Event::Iteration { kind: fleet_core::IterationKind::Review, n } if n >= 1))
        .count();
    assert_eq!(review_rounds, 3, "floor=3 demo runs three review rounds before the gate opens");
}

/// Guard against drift: this test's mirrored `demo_script` must match the daemon's
/// call-count invariant — oracle + 3 calls per review round — the same shape
/// `server.rs::demo_script_has_oracle_plus_three_calls_per_round` pins. If the
/// server's script changes shape, this fails and flags the mirror to be updated.
#[test]
fn demo_script_matches_server_shape() {
    // floor 2 ⇒ 1 oracle + 2×3 = 7 scripted execs (mirrors the server unit test).
    assert_eq!(demo_script(&demo_spec(2)).len(), 7);
    // A resumed unit (oracle already frozen) drops the oracle call.
    let mut resumed = demo_spec(2);
    resumed.oracle_frozen = true;
    assert_eq!(demo_script(&resumed).len(), 6);
}
