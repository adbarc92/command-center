//! `run-once` — drive a single real unit end-to-end (LocalDockerRunner + GhForge)
//! and stream its events to stdout. The minimal "live run" harness for SP1 until
//! the HTTP/WS server + cockpit land.
//!
//! Requires: Docker + the `cc-agent:dev` image, an authed `gh`, and
//! `ANTHROPIC_API_KEY` in the environment (forwarded into the container).
//!
//! Usage:
//!   ANTHROPIC_API_KEY=sk-... cargo run -p fleetd --bin run_once -- "TASK TEXT"
//!
//! Env knobs (with defaults for the public sandbox repo):
//!   CC_REPO_URL, CC_REPO_SLUG, CC_BASE_BRANCH, CC_TEST_CMD, CC_USD_CAP, CC_MIN_ROUNDS

use fleet_core::{Event, GateConfig, Tier};
use fleetd::driver::{run, EventEnvelope, RunCtx};
use fleetd::gh_forge::GhForge;
use fleetd::local_docker::LocalDockerRunner;
use fleetd::runner::UnitSpec;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

#[tokio::main]
async fn main() {
    // Load a .env from the cwd (or a parent) if present — see .env.example.
    let _ = dotenvy::dotenv();

    if std::env::var("ANTHROPIC_API_KEY").is_err() {
        eprintln!("error: ANTHROPIC_API_KEY is not set (copy .env.example to .env, or export it).");
        std::process::exit(2);
    }

    let task = std::env::args().nth(1).unwrap_or_else(|| {
        "Add a function `sum(a, b)` in src/index.js exported via \
         `module.exports.sum` that returns a + b."
            .to_string()
    });

    let millis = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis();
    let unit_id = format!("u{millis}");
    let branch = format!("agent/{unit_id}");

    let repo_url = env_or(
        "CC_REPO_URL",
        "https://github.com/adbarc92/command-center-agent-sandbox",
    );
    let repo_slug = env_or("CC_REPO_SLUG", "adbarc92/command-center-agent-sandbox");
    let base_branch = env_or("CC_BASE_BRANCH", "main");
    let test_cmd = env_or("CC_TEST_CMD", "node --test");
    // Conservative defaults for the first live runs.
    let usd_cap: f64 = env_or("CC_USD_CAP", "3.0").parse().unwrap_or(3.0);
    let wall_clock_secs: u64 = env_or("CC_WALL_SECS", "1800").parse().unwrap_or(1800);
    // First runs default to a single review round to keep cost down.
    let min_review_rounds: u32 = env_or("CC_MIN_ROUNDS", "1").parse().unwrap_or(1);

    let spec = UnitSpec {
        unit_id: unit_id.clone(),
        tier: Tier::T1, // autonomous for the first live run
        task,
        usd_cap,
        wall_clock_secs,
        gate: GateConfig { min_review_rounds },
        repo_url: repo_url.clone(),
        repo_slug: repo_slug.clone(),
        base_branch: base_branch.clone(),
        branch: branch.clone(),
        test_cmd,
    };

    let runner = LocalDockerRunner::new(env_or("CC_IMAGE", "cc-agent:dev"));
    let host_clone = std::env::temp_dir().join(format!("cc-host-{unit_id}"));
    let forge = GhForge::new(
        repo_url,
        repo_slug,
        base_branch,
        host_clone,
        format!("command-center SP1: {unit_id}"),
    );

    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
    let (evt_tx, mut evt_rx) = mpsc::unbounded_channel::<EventEnvelope>();
    drop(cmd_tx); // T1 run: no interactive commands

    println!("== run-once: unit {unit_id} on {branch} (cap ${usd_cap}) ==");
    let driver = tokio::spawn(run(runner, forge, spec, RunCtx::standalone(), cmd_rx, evt_tx));

    while let Some(env) = evt_rx.recv().await {
        print_event(&env);
    }

    match driver.await {
        Ok(phase) => println!("== terminal phase: {phase:?} =="),
        Err(e) => eprintln!("driver panicked: {e}"),
    }
}

fn print_event(env: &EventEnvelope) {
    match &env.event {
        Event::PhaseChanged { from, to, reason, .. } => {
            let why = reason.as_deref().map(|r| format!(" ({r})")).unwrap_or_default();
            println!("[{:>3}] {from:?} -> {to:?}{why}", env.seq);
        }
        Event::Iteration { kind, n } => println!("[{:>3}] iteration {kind:?} #{n}", env.seq),
        Event::Metric { cost_usd, tokens_in, tokens_out, .. } => {
            println!("[{:>3}] $ {cost_usd:.4}  (in {tokens_in} / out {tokens_out})", env.seq)
        }
        Event::Finding { round, title, .. } => println!("[{:>3}] review r{round}: {title}", env.seq),
        Event::Artifact { kind, reference } => println!("[{:>3}] artifact {kind:?}: {reference}", env.seq),
        Event::OracleProposed { test_files, .. } => {
            println!("[{:>3}] oracle proposed {} test line(s)", env.seq, test_files.len())
        }
        Event::Blocked { reason, .. } => println!("[{:>3}] BLOCKED: {reason}", env.seq),
        Event::Error { scope, detail, .. } => println!("[{:>3}] ERROR {scope:?}: {detail}", env.seq),
        Event::Done { result } => println!("[{:>3}] DONE: {result}", env.seq),
        Event::Log { stream, line } => println!("[{:>3}] {stream:?}| {line}", env.seq),
    }
}
