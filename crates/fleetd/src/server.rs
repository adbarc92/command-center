//! The localhost HTTP/WS server: the daemon's public surface. Commands flow IN
//! over REST, events flow OUT over `/units/:id/stream` (WS). Per-unit live state
//! is in-memory (ring buffer + broadcast) **and** persisted to the SQLite `Store`
//! so units survive cockpit reloads and daemon restarts.
//!
//! Endpoints:
//!   POST /missions                -> { unit_id }
//!   GET  /units                   -> [ summary ]
//!   GET  /units/:id               -> snapshot (phase + events, from the store)
//!   GET  /units/:id/events?since  -> [ event-json ]
//!   POST /units/:id/commands      -> 202 (Command JSON)
//!   GET  /units/:id/stream?since  -> WebSocket (replay since, then live)
//!   GET  /health                  -> { docker, anthropic_key, version }
//!
//! NOTE: persistence uses one `Arc<Mutex<Store>>` (WAL). A dedicated writer task
//! is a future optimization; at single-user scale a brief sync lock per event is
//! fine (the lock is never held across `.await`).

use crate::driver::{run, EventEnvelope, RunCtx};
use crate::fake::{FakeForge, FakeRunner};
use crate::runner::{ExecOutput, UnitSpec};
use crate::store::{Store, UnitRow};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, Query, State,
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
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::{broadcast, mpsc};

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as i64
}

fn phase_str(p: &Phase) -> String {
    serde_json::to_value(p).unwrap().as_str().unwrap_or("unknown").to_string()
}

/// Per-unit live state held by the server (for broadcast/commands). Event
/// history lives in the store now, not an in-memory buffer.
struct UnitHandle {
    cmd_tx: mpsc::UnboundedSender<Command>,
    bcast: broadcast::Sender<EventEnvelope>,
}

#[derive(Clone)]
pub struct AppState {
    units: Arc<Mutex<HashMap<String, UnitHandle>>>,
    next_id: Arc<AtomicU64>,
    store: Arc<Mutex<Store>>,
    docker: Arc<Mutex<(Instant, bool)>>,
}

impl AppState {
    pub fn new(store: Arc<Mutex<Store>>) -> Self {
        Self {
            units: Arc::new(Mutex::new(HashMap::new())),
            next_id: Arc::new(AtomicU64::new(1)),
            store,
            // Start "stale" so the first /health does a real probe.
            docker: Arc::new(Mutex::new((Instant::now() - Duration::from_secs(60), false))),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new(Arc::new(Mutex::new(Store::open_memory().expect("memory store"))))
    }
}

/// Build the router.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/missions", post(create_mission))
        .route("/units", get(list_units))
        .route("/units/:id", get(get_unit))
        .route("/units/:id/events", get(get_events))
        .route("/units/:id/commands", post(post_command))
        .route("/units/:id/stream", get(ws_stream))
        .route("/health", get(health))
        .with_state(state)
}

#[derive(Deserialize)]
struct CreateReq {
    task: String,
    #[serde(default)]
    tier: TierReq,
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

/// Build the initial persisted row for a fresh unit.
fn row_from_spec(spec: &UnitSpec) -> UnitRow {
    UnitRow {
        unit_id: spec.unit_id.clone(),
        tier: phase_tier(spec.tier),
        task: spec.task.clone(),
        repo_url: spec.repo_url.clone(),
        repo_slug: spec.repo_slug.clone(),
        base_branch: spec.base_branch.clone(),
        branch: spec.branch.clone(),
        test_cmd: spec.test_cmd.clone(),
        usd_cap: spec.usd_cap,
        wall_clock_secs: spec.wall_clock_secs,
        phase: "queued".into(),
        cost: 0.0,
        last_seq: 0,
        oracle_frozen: spec.oracle_frozen,
        terminal_reason: None,
    }
}
fn phase_tier(t: Tier) -> String {
    match t {
        Tier::T1 => "t1",
        Tier::T2 => "t2",
        Tier::T3 => "t3",
    }
    .into()
}

async fn create_mission(
    State(st): State<AppState>,
    Json(req): Json<CreateReq>,
) -> Result<Json<CreateResp>, (StatusCode, String)> {
    let n = st.next_id.fetch_add(1, Ordering::Relaxed);
    let unit_id = format!("u{n}");
    let spec = UnitSpec {
        unit_id: unit_id.clone(),
        tier: req.tier.into(),
        task: req.task,
        usd_cap: 5.0,
        wall_clock_secs: 1800,
        gate: GateConfig { min_review_rounds: req.min_review_rounds.max(1) },
        repo_url: "https://github.com/adbarc92/command-center-agent-sandbox".into(),
        repo_slug: "adbarc92/command-center-agent-sandbox".into(),
        base_branch: "main".into(),
        branch: format!("agent/{unit_id}"),
        test_cmd: "node --test".into(),
        oracle_frozen: false,
    };

    // Persist the initial row.
    st.store.lock().unwrap().upsert_unit(&row_from_spec(&spec), now_ms()).ok();

    let runner_mode = req.mode.clone();
    if runner_mode == "real" && std::env::var("ANTHROPIC_API_KEY").is_err() {
        return Err((StatusCode::BAD_REQUEST, "ANTHROPIC_API_KEY not set".into()));
    }

    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel::<Command>();
    let (evt_tx, evt_rx) = mpsc::unbounded_channel::<EventEnvelope>();
    let (bcast, _) = broadcast::channel::<EventEnvelope>(1024);

    spawn_forwarder(st.store.clone(), bcast.clone(), evt_rx, unit_id.clone());

    match runner_mode.as_str() {
        "demo" => {
            let runner = FakeRunner::new(demo_script(&spec));
            tokio::spawn(run(runner, FakeForge::default(), spec, RunCtx::standalone(), cmd_rx, evt_tx));
        }
        "real" => {
            use crate::gh_forge::GhForge;
            use crate::local_docker::LocalDockerRunner;
            let host_clone = std::env::temp_dir().join(format!("cc-host-{unit_id}"));
            let forge = GhForge::new(
                spec.repo_url.clone(),
                spec.repo_slug.clone(),
                spec.base_branch.clone(),
                host_clone,
                format!("command-center SP1: {unit_id}"),
            );
            let runner = LocalDockerRunner::new("cc-agent:dev");
            tokio::spawn(run(runner, forge, spec, RunCtx::standalone(), cmd_rx, evt_tx));
        }
        other => return Err((StatusCode::BAD_REQUEST, format!("unknown mode: {other}"))),
    }

    st.units.lock().unwrap().insert(unit_id.clone(), UnitHandle { cmd_tx, bcast });
    Ok(Json(CreateResp { unit_id }))
}

/// Fan each driver event into the store (persist + projection), the in-memory
/// ring buffer, and the broadcast. The store lock is held only in a sync block.
fn spawn_forwarder(
    store: Arc<Mutex<Store>>,
    bcast: broadcast::Sender<EventEnvelope>,
    mut evt_rx: mpsc::UnboundedReceiver<EventEnvelope>,
    unit_id: String,
) {
    tokio::spawn(async move {
        let mut cur_phase = "queued".to_string();
        let mut cur_cost = 0.0_f64;
        let mut terminal_reason: Option<String> = None;
        while let Some(env) = evt_rx.recv().await {
            match &env.event {
                Event::PhaseChanged { to, .. } => cur_phase = phase_str(to),
                Event::Metric { cost_usd, .. } => cur_cost = *cost_usd,
                Event::Done { result } => terminal_reason = Some(result.clone()),
                _ => {}
            }
            let json = serde_json::to_string(&env).unwrap_or_default();
            let ts = now_ms();
            {
                let s = store.lock().unwrap();
                let _ = s.append_event(&unit_id, env.seq, ts, &json);
                let _ = s.update_unit(&unit_id, &cur_phase, cur_cost, env.seq, terminal_reason.as_deref(), ts);
            }
            let _ = bcast.send(env);
        }
    });
}

#[derive(Serialize)]
struct UnitSummary {
    unit_id: String,
    phase: String,
    cost: f64,
    usd_cap: f64,
    tier: String,
    task: String,
    last_seq: u64,
}
fn summary(r: &UnitRow) -> UnitSummary {
    UnitSummary {
        unit_id: r.unit_id.clone(),
        phase: r.phase.clone(),
        cost: r.cost,
        usd_cap: r.usd_cap,
        tier: r.tier.clone(),
        task: r.task.clone(),
        last_seq: r.last_seq,
    }
}

async fn list_units(State(st): State<AppState>) -> Json<Vec<UnitSummary>> {
    let rows = st.store.lock().unwrap().list_units().unwrap_or_default();
    Json(rows.iter().map(summary).collect())
}

#[derive(Serialize)]
struct Snapshot {
    unit_id: String,
    phase: String,
    usd_cap: f64,
    cost: f64,
    events: Vec<serde_json::Value>,
}

async fn get_unit(
    State(st): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Snapshot>, StatusCode> {
    let s = st.store.lock().unwrap();
    let row = s.get_unit(&id).ok().flatten().ok_or(StatusCode::NOT_FOUND)?;
    let events = s
        .events_since(&id, 0)
        .unwrap_or_default()
        .iter()
        .filter_map(|j| serde_json::from_str(j).ok())
        .collect();
    Ok(Json(Snapshot {
        unit_id: id,
        phase: row.phase,
        usd_cap: row.usd_cap,
        cost: row.cost,
        events,
    }))
}

#[derive(Deserialize)]
struct SinceQuery {
    #[serde(default)]
    since: u64,
}

async fn get_events(
    State(st): State<AppState>,
    Path(id): Path<String>,
    Query(q): Query<SinceQuery>,
) -> Json<Vec<serde_json::Value>> {
    let events = st
        .store
        .lock()
        .unwrap()
        .events_since(&id, q.since)
        .unwrap_or_default()
        .iter()
        .filter_map(|j| serde_json::from_str(j).ok())
        .collect();
    Json(events)
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
            Err(_) => StatusCode::GONE,
        },
        None => StatusCode::NOT_FOUND,
    }
}

async fn ws_stream(
    State(st): State<AppState>,
    Path(id): Path<String>,
    Query(q): Query<SinceQuery>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| stream_to_socket(socket, id, q.since, st))
}

async fn stream_to_socket(mut socket: WebSocket, id: String, since: u64, st: AppState) {
    // Subscribe BEFORE reading the store so nothing in between is missed; dedup by seq.
    let (replay, mut rx) = {
        let units = st.units.lock().unwrap();
        let rx = units.get(&id).map(|h| h.bcast.subscribe());
        let replay = st.store.lock().unwrap().events_since(&id, since).unwrap_or_default();
        (replay, rx)
    };

    let mut last_seq = since;
    for json in replay {
        if let Some(seq) = seq_of(&json) {
            last_seq = last_seq.max(seq);
        }
        if socket.send(Message::Text(json)).await.is_err() {
            return;
        }
    }
    // If the unit has no live driver (e.g. terminal/restarted), there's nothing to tail.
    let Some(mut rx) = rx.take() else { return };
    loop {
        match rx.recv().await {
            Ok(env) => {
                if env.seq <= last_seq {
                    continue;
                }
                last_seq = env.seq;
                match serde_json::to_string(&env) {
                    Ok(j) => {
                        if socket.send(Message::Text(j)).await.is_err() {
                            return;
                        }
                    }
                    Err(_) => return,
                }
            }
            Err(broadcast::error::RecvError::Lagged(_)) => continue,
            Err(broadcast::error::RecvError::Closed) => return,
        }
    }
}

fn seq_of(json: &str) -> Option<u64> {
    serde_json::from_str::<serde_json::Value>(json).ok()?.get("seq")?.as_u64()
}

#[derive(Serialize)]
struct Health {
    docker: bool,
    anthropic_key: bool,
    version: &'static str,
}

async fn health(State(st): State<AppState>) -> Json<Health> {
    Json(Health {
        docker: docker_ok(&st).await,
        anthropic_key: std::env::var("ANTHROPIC_API_KEY").is_ok(),
        version: env!("CARGO_PKG_VERSION"),
    })
}

/// Cached docker liveness (5s TTL) so the cockpit can poll cheaply.
async fn docker_ok(st: &AppState) -> bool {
    {
        let cache = st.docker.lock().unwrap();
        if cache.0.elapsed() < Duration::from_secs(5) {
            return cache.1;
        }
    }
    let ok = tokio::process::Command::new("docker")
        .args(["version", "--format", "{{.Server.Version}}"])
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false);
    *st.docker.lock().unwrap() = (Instant::now(), ok);
    ok
}

/// A FakeRunner demo script: oracle, then one build/check/review cycle per review
/// round with blockers trending to zero so the gate opens on the floor.
fn demo_script(spec: &UnitSpec) -> Vec<ExecOutput> {
    let floor = spec.gate.min_review_rounds.max(1);
    let mut s = vec![FakeRunner::ok(0.02, &["sum.test.js"])];
    for remaining in (0..floor).rev() {
        s.push(FakeRunner::ok(0.03, &["implementing the change"]));
        s.push(FakeRunner::ok(0.0, &["tests: 1 passing"]));
        s.push(FakeRunner::ok(0.04, &[&format!("review done\nBLOCKERS={remaining}")]));
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
            oracle_frozen: false,
        };
        assert_eq!(demo_script(&spec).len(), 7);
    }
}
