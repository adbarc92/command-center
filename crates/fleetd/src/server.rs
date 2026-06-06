//! The localhost HTTP/WS server: the daemon's public surface. Commands flow IN
//! over REST, events flow OUT over `/units/:id/stream` (WS). For SP1 it keeps
//! per-unit state in memory (ring buffer + broadcast); durability is SP3.
//!
//! Endpoints:
//!   POST /missions                -> { unit_id }      (spawn a unit)
//!   GET  /units/:id               -> snapshot          (phase + buffered events)
//!   POST /units/:id/commands      -> 202               (Command JSON body)
//!   GET  /units/:id/stream        -> WebSocket         (snapshot then live)

use crate::driver::{run, EventEnvelope};
use crate::fake::{FakeForge, FakeRunner};
use crate::runner::{ExecOutput, UnitSpec};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, State,
    },
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use fleet_core::{Command, Event, GateConfig, Phase, Tier};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex,
};
use tokio::sync::{broadcast, mpsc};

/// Per-unit live state held by the server.
struct UnitHandle {
    cmd_tx: mpsc::UnboundedSender<Command>,
    buffer: Arc<Mutex<Vec<EventEnvelope>>>,
    bcast: broadcast::Sender<EventEnvelope>,
    phase: Arc<Mutex<Phase>>,
}

#[derive(Clone)]
pub struct AppState {
    units: Arc<Mutex<HashMap<String, UnitHandle>>>,
    next_id: Arc<AtomicU64>,
}

impl Default for AppState {
    fn default() -> Self {
        Self { units: Arc::new(Mutex::new(HashMap::new())), next_id: Arc::new(AtomicU64::new(1)) }
    }
}

/// Build the router. `AppState::default()` gives an empty registry.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/missions", post(create_mission))
        .route("/units/:id", get(get_unit))
        .route("/units/:id/commands", post(post_command))
        .route("/units/:id/stream", get(ws_stream))
        .with_state(state)
}

#[derive(Deserialize)]
struct CreateReq {
    task: String,
    #[serde(default)]
    tier: TierReq,
    /// "demo" (FakeRunner, no secrets) is the default for the cockpit.
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default = "default_floor")]
    min_review_rounds: u32,
}

#[derive(Deserialize, Default, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum TierReq {
    #[default]
    T1,
    T2,
    T3,
}

impl From<TierReq> for Tier {
    fn from(t: TierReq) -> Self {
        match t {
            TierReq::T1 => Tier::T1,
            TierReq::T2 => Tier::T2,
            TierReq::T3 => Tier::T3,
        }
    }
}

fn default_mode() -> String {
    "demo".into()
}
fn default_floor() -> u32 {
    2
}

#[derive(Serialize)]
struct CreateResp {
    unit_id: String,
}

async fn create_mission(
    State(st): State<AppState>,
    Json(req): Json<CreateReq>,
) -> Result<Json<CreateResp>, (StatusCode, String)> {
    let n = st.next_id.fetch_add(1, Ordering::Relaxed);
    let unit_id = format!("u{n}");
    let tier: Tier = req.tier.into();
    let spec = UnitSpec {
        unit_id: unit_id.clone(),
        tier,
        task: req.task,
        usd_cap: 5.0,
        wall_clock_secs: 1800,
        gate: GateConfig { min_review_rounds: req.min_review_rounds.max(1) },
        repo_url: "https://github.com/adbarc92/command-center-agent-sandbox".into(),
        repo_slug: "adbarc92/command-center-agent-sandbox".into(),
        base_branch: "main".into(),
        branch: format!("agent/{unit_id}"),
        test_cmd: "node --test".into(),
    };

    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel::<Command>();
    let (evt_tx, mut evt_rx) = mpsc::unbounded_channel::<EventEnvelope>();
    let (bcast, _) = broadcast::channel::<EventEnvelope>(1024);
    let buffer = Arc::new(Mutex::new(Vec::<EventEnvelope>::new()));
    let phase = Arc::new(Mutex::new(Phase::Queued));

    // Forwarder: fan each driver event into the buffer + broadcast, tracking phase.
    {
        let buffer = buffer.clone();
        let phase = phase.clone();
        let bcast = bcast.clone();
        tokio::spawn(async move {
            while let Some(env) = evt_rx.recv().await {
                if let Event::PhaseChanged { to, .. } = &env.event {
                    *phase.lock().unwrap() = *to;
                }
                buffer.lock().unwrap().push(env.clone());
                let _ = bcast.send(env);
            }
        });
    }

    // Spawn the driver with the chosen backend.
    match req.mode.as_str() {
        "demo" => {
            let runner = FakeRunner::new(demo_script(&spec));
            tokio::spawn(run(runner, FakeForge::default(), spec, cmd_rx, evt_tx));
        }
        "real" => {
            // Real Docker + GitHub run (requires ANTHROPIC_API_KEY + the image).
            use crate::gh_forge::GhForge;
            use crate::local_docker::LocalDockerRunner;
            if std::env::var("ANTHROPIC_API_KEY").is_err() {
                return Err((StatusCode::BAD_REQUEST, "ANTHROPIC_API_KEY not set".into()));
            }
            let host_clone = std::env::temp_dir().join(format!("cc-host-{unit_id}"));
            let forge = GhForge::new(
                spec.repo_url.clone(),
                spec.repo_slug.clone(),
                spec.base_branch.clone(),
                host_clone,
                format!("command-center SP1: {unit_id}"),
            );
            let runner = LocalDockerRunner::new("cc-agent:dev");
            tokio::spawn(run(runner, forge, spec, cmd_rx, evt_tx));
        }
        other => return Err((StatusCode::BAD_REQUEST, format!("unknown mode: {other}"))),
    }

    st.units.lock().unwrap().insert(
        unit_id.clone(),
        UnitHandle { cmd_tx, buffer, bcast, phase },
    );
    Ok(Json(CreateResp { unit_id }))
}

#[derive(Serialize)]
struct Snapshot {
    unit_id: String,
    phase: Phase,
    events: Vec<EventEnvelope>,
}

async fn get_unit(
    State(st): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Snapshot>, StatusCode> {
    let units = st.units.lock().unwrap();
    let h = units.get(&id).ok_or(StatusCode::NOT_FOUND)?;
    let phase = *h.phase.lock().unwrap();
    let events = h.buffer.lock().unwrap().clone();
    Ok(Json(Snapshot { unit_id: id, phase, events }))
}

async fn post_command(
    State(st): State<AppState>,
    Path(id): Path<String>,
    Json(cmd): Json<Command>,
) -> StatusCode {
    let units = st.units.lock().unwrap();
    match units.get(&id) {
        Some(h) => match h.cmd_tx.send(cmd) {
            Ok(()) => StatusCode::ACCEPTED,
            Err(_) => StatusCode::GONE, // driver finished
        },
        None => StatusCode::NOT_FOUND,
    }
}

async fn ws_stream(
    State(st): State<AppState>,
    Path(id): Path<String>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| stream_to_socket(socket, id, st))
}

async fn stream_to_socket(mut socket: WebSocket, id: String, st: AppState) {
    // Subscribe BEFORE snapshotting so nothing is missed; de-dup by seq.
    let (snapshot, mut rx) = {
        let units = st.units.lock().unwrap();
        let Some(h) = units.get(&id) else { return };
        let snapshot = h.buffer.lock().unwrap().clone();
        let rx = h.bcast.subscribe();
        (snapshot, rx)
    };

    let mut last_seq = 0u64;
    for env in snapshot {
        last_seq = env.seq;
        if send_json(&mut socket, &env).await.is_err() {
            return;
        }
    }
    loop {
        match rx.recv().await {
            Ok(env) => {
                if env.seq <= last_seq {
                    continue; // already sent from the snapshot
                }
                last_seq = env.seq;
                if send_json(&mut socket, &env).await.is_err() {
                    return;
                }
            }
            Err(broadcast::error::RecvError::Lagged(_)) => continue,
            Err(broadcast::error::RecvError::Closed) => return,
        }
    }
}

async fn send_json(socket: &mut WebSocket, env: &EventEnvelope) -> Result<(), ()> {
    let json = serde_json::to_string(env).map_err(|_| ())?;
    socket.send(Message::Text(json)).await.map_err(|_| ())
}

/// A FakeRunner script that runs a demo unit to Done: oracle then one cycle per
/// review round with blockers trending to zero so the gate opens on the floor.
fn demo_script(spec: &UnitSpec) -> Vec<ExecOutput> {
    let floor = spec.gate.min_review_rounds.max(1);
    let mut s = vec![FakeRunner::ok(0.02, &["sum.test.js"])]; // oracle proposes a test
    for remaining in (0..floor).rev() {
        s.push(FakeRunner::ok(0.03, &["implementing the change"])); // build
        s.push(FakeRunner::ok(0.0, &["tests: 1 passing"])); // check
        s.push(FakeRunner::ok(0.04, &[&format!("review done\nBLOCKERS={remaining}")])); // review
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_script_has_oracle_plus_three_calls_per_round() {
        let spec = UnitSpec {
            unit_id: "u".into(),
            tier: Tier::T1,
            task: "t".into(),
            usd_cap: 5.0,
            wall_clock_secs: 0,
            gate: GateConfig { min_review_rounds: 2 },
            repo_url: "https://github.com/x/y".into(),
            repo_slug: "x/y".into(),
            base_branch: "main".into(),
            branch: "agent/u".into(),
            test_cmd: "node --test".into(),
        };
        // 1 oracle + 2 rounds * 3 calls = 7
        assert_eq!(demo_script(&spec).len(), 7);
    }
}
