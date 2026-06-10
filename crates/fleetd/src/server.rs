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

use crate::docsource::DocSource;
use crate::driver::{run, EventEnvelope, RunCtx};
use crate::fake::{FakeForge, FakeRunner};
use crate::planner::Planner;
use crate::reconcile::{reconcile, Action};
use crate::runner::{ExecOutput, Runner, UnitSpec};
use crate::swarm::{admit_lanes, slug, AdmissionConfig, LaneDecision};
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
use tokio::sync::{broadcast, mpsc, Semaphore};

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
    /// Monotonic swarm-id counter. Seeded from `max_swarm_seq` on startup so a
    /// daemon restart never re-mints a live swarm id (mirrors `next_id`).
    next_swarm: Arc<AtomicU64>,
    store: Arc<Mutex<Store>>,
    docker: Arc<Mutex<(Instant, bool)>>,
    /// Fleet-wide concurrency slots, shared by every driver (CC_MAX_CONCURRENT).
    permits: Arc<Semaphore>,
    /// Rolling-24h spend ceiling that admits/refuses new missions (CC_GLOBAL_USD_CAP).
    global_cap: f64,
}

/// Default concurrency / global-cap knobs, overridable via env.
const DEFAULT_MAX_CONCURRENT: usize = 3;
const DEFAULT_GLOBAL_USD_CAP: f64 = 20.0;

fn env_usize(key: &str, default: usize) -> usize {
    std::env::var(key).ok().and_then(|v| v.parse().ok()).filter(|&n| n > 0).unwrap_or(default)
}
fn env_f64(key: &str, default: f64) -> f64 {
    std::env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

impl AppState {
    pub fn new(store: Arc<Mutex<Store>>) -> Self {
        let max_concurrent = env_usize("CC_MAX_CONCURRENT", DEFAULT_MAX_CONCURRENT);
        let next = store.lock().unwrap().max_unit_seq().expect("seed next_id from max_unit_seq") + 1;
        let next_sw = store.lock().unwrap().max_swarm_seq().expect("seed next_swarm from max_swarm_seq") + 1;
        Self {
            units: Arc::new(Mutex::new(HashMap::new())),
            next_id: Arc::new(AtomicU64::new(next)),
            next_swarm: Arc::new(AtomicU64::new(next_sw)),
            store,
            // Start "stale" so the first /health does a real probe.
            docker: Arc::new(Mutex::new((Instant::now() - Duration::from_secs(60), false))),
            permits: Arc::new(Semaphore::new(max_concurrent)),
            global_cap: env_f64("CC_GLOBAL_USD_CAP", DEFAULT_GLOBAL_USD_CAP),
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
        .route("/swarms", post(create_swarm).get(list_swarms))
        .route("/swarms/:id", get(get_swarm))
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

/// A `RunCtx` for a freshly-created unit: starts at `Queued`, no prior seq/cost,
/// sharing the fleet-wide concurrency permits.
fn fresh_ctx(st: &AppState) -> RunCtx {
    RunCtx {
        start_seq: 0,
        start_cost: 0.0,
        resume: false,
        start_phase: Phase::Queued,
        permits: st.permits.clone(),
    }
}

/// Build the initial persisted row for a fresh unit. `mode` ("demo"|"real") is a
/// runner choice that isn't part of `UnitSpec`, so it's threaded in explicitly.
fn row_from_spec(spec: &UnitSpec, mode: &str) -> UnitRow {
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
        mode: mode.into(),
        min_review_rounds: spec.gate.min_review_rounds,
        swarm_id: None,
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

#[derive(Debug)]
#[non_exhaustive]
pub enum SpawnError {
    /// A driver already exists for this id (register lost the race / id reuse).
    AlreadyRegistered,
}

/// Register the per-unit handle and spawn its driver. The row must already be
/// persisted by the caller (create_mission, or fan-out via
/// commit_lane_unit). Returns Err if a driver already exists (no double-spawn).
fn spawn_driver_for(st: &AppState, spec: UnitSpec, mode: &str, unit_id: &str)
    -> Result<(), SpawnError> {
    let (cmd_rx, evt_tx) = match register_unit_if_absent(st, unit_id) {
        Some(ch) => ch,
        None => return Err(SpawnError::AlreadyRegistered),
    };
    match mode {
        "demo" => {
            let runner = FakeRunner::new(demo_script(&spec));
            tokio::spawn(run(runner, FakeForge::default(), spec, fresh_ctx(st), cmd_rx, evt_tx));
        }
        _ => {
            use crate::gh_forge::GhForge;
            use crate::local_docker::LocalDockerRunner;
            let host_clone = std::env::temp_dir().join(format!("cc-host-{unit_id}"));
            let forge = GhForge::new(spec.repo_url.clone(), spec.repo_slug.clone(),
                spec.base_branch.clone(), host_clone, format!("command-center SP1: {unit_id}"));
            tokio::spawn(run(LocalDockerRunner::new("cc-agent:dev"), forge, spec, fresh_ctx(st), cmd_rx, evt_tx));
        }
    }
    Ok(())
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

    // Validate the mode BEFORE persisting/registering, so a bad request never
    // leaves a driverless unit in the map or a junk row in the store.
    let runner_mode = req.mode.clone();
    match runner_mode.as_str() {
        "demo" => {}
        "real" => {
            if std::env::var("ANTHROPIC_API_KEY").is_err() {
                return Err((StatusCode::BAD_REQUEST, "ANTHROPIC_API_KEY not set".into()));
            }
        }
        other => return Err((StatusCode::BAD_REQUEST, format!("unknown mode: {other}"))),
    }

    // P2: admission check + row insert in ONE critical section.
    {
        let s = st.store.lock().unwrap();
        let since = now_ms() - 24 * 3600 * 1000;
        if s.committed_spend(since).unwrap_or(0.0) >= st.global_cap {
            return Err((StatusCode::TOO_MANY_REQUESTS, "global daily cost cap reached".into()));
        }
        let mut row = row_from_spec(&spec, &runner_mode);
        row.swarm_id = None;
        s.upsert_unit(&row, now_ms()).ok();
    }
    if spawn_driver_for(&st, spec, &runner_mode, &unit_id).is_err() {
        return Err((StatusCode::CONFLICT, "unit already exists".into()));
    }

    Ok(Json(CreateResp { unit_id }))
}

/// Atomically register a per-unit handle (command channel + event forwarder +
/// broadcast) in the units map, returning the driver-side channels **iff** this
/// call created it. `None` means a concurrent caller already registered the unit,
/// so the caller must NOT spawn a second driver. The check-and-insert runs under
/// the units-map lock, so two simultaneous Resume clicks yield exactly one driver.
fn register_unit_if_absent(
    st: &AppState,
    unit_id: &str,
) -> Option<(mpsc::UnboundedReceiver<Command>, mpsc::UnboundedSender<EventEnvelope>)> {
    let mut units = st.units.lock().unwrap();
    if units.contains_key(unit_id) {
        return None;
    }
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel::<Command>();
    let (evt_tx, evt_rx) = mpsc::unbounded_channel::<EventEnvelope>();
    let (bcast, _) = broadcast::channel::<EventEnvelope>(1024);
    spawn_forwarder(st.store.clone(), bcast.clone(), evt_rx, unit_id.to_string());
    units.insert(unit_id.to_string(), UnitHandle { cmd_tx, bcast });
    Some((cmd_rx, evt_tx))
}

/// Bring a store-only unit (e.g. one left `Halted` by startup reconciliation
/// after a daemon restart) back into memory so an inbound `Resume`/`Abandon`
/// command has a live driver. Atomic: a concurrent rehydration is a no-op.
/// Rehydrated units run in REAL mode (`LocalDockerRunner` + `GhForge`) — demo
/// units are throwaway and aren't meant to survive a restart.
fn rehydrate(st: &AppState, unit_id: &str) {
    // Only rehydrate units we actually have persisted.
    let Some(row) = st.store.lock().unwrap().get_unit(unit_id).ok().flatten() else {
        return;
    };
    // Atomic check-and-insert: if a concurrent caller won, don't spawn a 2nd driver.
    let Some((cmd_rx, evt_tx)) = register_unit_if_absent(st, unit_id) else {
        return;
    };
    let spec = spec_from_row(&row);
    let ctx = RunCtx {
        start_seq: row.last_seq,
        start_cost: row.cost,
        resume: true,
        // Park at Halted; the inbound Resume command drives it on to Provisioning
        // (which reuses the kept volume and skips the already-frozen oracle).
        start_phase: Phase::Halted,
        permits: st.permits.clone(),
    };
    // Rehydrate as the SAME runner the unit was launched with (persisted), so a
    // demo unit resumes as a demo and never attempts a real Docker provision.
    match row.mode.as_str() {
        "demo" => {
            let runner = FakeRunner::new(demo_script(&spec));
            tokio::spawn(run(runner, FakeForge::default(), spec, ctx, cmd_rx, evt_tx));
        }
        _ => {
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
            tokio::spawn(run(runner, forge, spec, ctx, cmd_rx, evt_tx));
        }
    }
}

/// Reconstruct a `UnitSpec` from its persisted row for rehydration, including the
/// review-gate floor it was launched with, so a resumed unit honors the same gate.
fn spec_from_row(r: &UnitRow) -> UnitSpec {
    UnitSpec {
        unit_id: r.unit_id.clone(),
        tier: parse_tier(&r.tier),
        task: r.task.clone(),
        usd_cap: r.usd_cap,
        wall_clock_secs: r.wall_clock_secs,
        gate: GateConfig { min_review_rounds: r.min_review_rounds.max(1) },
        repo_url: r.repo_url.clone(),
        repo_slug: r.repo_slug.clone(),
        base_branch: r.base_branch.clone(),
        branch: r.branch.clone(),
        test_cmd: r.test_cmd.clone(),
        oracle_frozen: r.oracle_frozen,
    }
}

fn parse_tier(s: &str) -> Tier {
    match s {
        "t2" => Tier::T2,
        "t3" => Tier::T3,
        _ => Tier::T1,
    }
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
    // If the unit isn't live in memory (e.g. restart-stranded), rehydrate it from
    // the store first — an atomic check-and-insert, so concurrent commands still
    // yield one driver. No-op if it's already present or absent from the store.
    if !st.units.lock().unwrap().contains_key(&id) {
        rehydrate(&st, &id);
    }
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

fn parse_phase(s: &str) -> Phase {
    serde_json::from_str::<Phase>(&format!("\"{s}\"")).unwrap_or(Phase::Queued)
}

/// Reap orphan containers and mark their units `Halted` with a coherent event,
/// after a daemon restart. Run BEFORE the server accepts connections (no live
/// forwarders exist yet, so the synthetic event is written directly to the store).
pub async fn reconcile_on_startup<R: Runner>(state: &AppState, runner: &R) {
    let rows = state.store.lock().unwrap().list_units().unwrap_or_default();
    let nonterminal: Vec<String> = rows
        .iter()
        .filter(|r| !fleet_core::TERMINAL_PHASE_STRS.contains(&r.phase.as_str()))
        .map(|r| r.unit_id.clone())
        .collect();
    let running = runner.list_unit_containers().await.unwrap_or_default();
    for action in reconcile(&nonterminal, &running) {
        match action {
            Action::HaltWithContainer(id) => {
                let _ = runner.reap_unit(&id).await;
                halt_in_store(state, &id);
            }
            Action::HaltNoContainer(id) => halt_in_store(state, &id),
            Action::ReapStray(id) => {
                let _ = runner.reap_unit(&id).await;
            }
        }
    }

    // Swarm reconcile: planning → failed; fanning_out → resume missing lanes.
    let swarms = state.store.lock().unwrap().list_swarms().unwrap_or_default();
    for sw in swarms {
        match sw.status.as_str() {
            "planning" => {
                state.store.lock().unwrap().update_swarm(&sw.swarm_id, "failed", sw.planner_cost,
                    sw.lanes_launched, sw.lanes_dropped, Some("daemon restarted during planning"), now_ms()).ok();
            }
            "fanning_out" => resume_fan_out(state, &sw),
            _ => {}
        }
    }
}

/// Re-commit any admitted lane whose unit doesn't yet exist, then mark running.
/// A lane counts as launched only if its `unit_id` is set AND a unit row actually
/// exists (defensive against a crash between minting the id and committing the row).
fn resume_fan_out(st: &AppState, sw: &crate::store::SwarmRow) {
    let lanes = st.store.lock().unwrap().lanes_for_swarm(&sw.swarm_id).unwrap_or_default();
    let mut launched = 0u32;
    for l in lanes.into_iter().filter(|l| l.decision == "admit") {
        let exists = l.unit_id.as_ref()
            .and_then(|id| st.store.lock().unwrap().get_unit(id).ok().flatten()).is_some();
        if exists { launched += 1; continue; }
        let n = st.next_id.fetch_add(1, Ordering::Relaxed);
        let unit_id = format!("u{n}");
        let lane = crate::swarm::Lane { title: l.title.clone(), task: l.task.clone(), rationale: l.rationale.clone() };
        let spec = lane_spec(sw, &unit_id, l.idx as usize, &lane);
        let mut row = row_from_spec(&spec, &sw.mode);
        row.swarm_id = Some(sw.swarm_id.clone());
        st.store.lock().unwrap().commit_lane_unit(&sw.swarm_id, l.idx, &row, now_ms()).ok();
        if spawn_driver_for(st, spec, &sw.mode, &unit_id).is_ok() { launched += 1; }
    }
    st.store.lock().unwrap().update_swarm(&sw.swarm_id, "running", sw.planner_cost,
        launched, sw.lanes_dropped, None, now_ms()).ok();
}

/// Append a synthetic `Halted` event + update the row (one store write, no await).
fn halt_in_store(state: &AppState, id: &str) {
    let s = state.store.lock().unwrap();
    if let Ok(Some(row)) = s.get_unit(id) {
        let seq = row.last_seq + 1;
        let env = EventEnvelope {
            unit_id: id.to_string(),
            seq,
            event: Event::PhaseChanged {
                from: parse_phase(&row.phase),
                to: Phase::Halted,
                reason: Some("daemon restarted".into()),
                cmd_id: None,
            },
        };
        let ts = now_ms();
        let _ = s.append_event(id, seq, ts, &serde_json::to_string(&env).unwrap_or_default());
        let _ = s.update_unit(id, "halted", row.cost, seq, Some("daemon restarted"), ts);
    }
}

/// A FakeRunner demo script: oracle, then one build/check/review cycle per review
/// round with blockers trending to zero so the gate opens on the floor.
fn demo_script(spec: &UnitSpec) -> Vec<ExecOutput> {
    let floor = spec.gate.min_review_rounds.max(1);
    let mut s = vec![];
    // On resume the oracle is already frozen and the Spec phase is skipped, so the
    // script must start at the first build — otherwise the build would consume the
    // oracle's scripted output.
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

// ---------------------------------------------------------------------------
// Swarm endpoints: POST /swarms, GET /swarms, GET /swarms/:id
// ---------------------------------------------------------------------------

#[derive(Deserialize, Default)]
struct CreateSwarmReq {
    doc_path: String,
    #[serde(default)] tier: TierReq,
    #[serde(default = "default_mode")] mode: String,
    #[serde(default)] max_lanes: Option<u32>,
    #[serde(default)] usd_budget: Option<f64>,
    #[serde(default)] per_lane_cap: Option<f64>,
    #[serde(default = "default_floor")] min_review_rounds: u32,
    #[serde(default)] repo_url: Option<String>,
    #[serde(default)] repo_slug: Option<String>,
    #[serde(default)] base_branch: Option<String>,
}

#[derive(Serialize)]
struct CreateSwarmResp { swarm_id: String }

async fn create_swarm(State(st): State<AppState>, Json(req): Json<CreateSwarmReq>)
    -> Result<Json<CreateSwarmResp>, (StatusCode, String)> {
    // Step 0 — synchronous validation (no 4xx can occur in the spawned task).
    if req.doc_path.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "doc_path is required".into()));
    }
    match req.mode.as_str() {
        "demo" => {}
        "real" => {
            if std::env::var("ANTHROPIC_API_KEY").is_err() {
                return Err((StatusCode::BAD_REQUEST, "ANTHROPIC_API_KEY not set".into()));
            }
            if !docker_ok(&st).await {
                return Err((StatusCode::SERVICE_UNAVAILABLE, "docker not available".into()));
            }
        }
        other => return Err((StatusCode::BAD_REQUEST, format!("unknown mode: {other}"))),
    }

    // Step 1 — committed-spend admission + persist 'planning'.
    let since = now_ms() - 24 * 3600 * 1000;
    let lane_cap = req.max_lanes.unwrap_or_else(|| env_usize("CC_MAX_LANES", 8) as u32).max(1);
    let per_lane_cap = req.per_lane_cap.unwrap_or(5.0);
    let n = st.next_swarm.fetch_add(1, Ordering::Relaxed);
    let swarm_id = format!("sw{n}");
    let repo_url = req.repo_url.unwrap_or_else(|| "https://github.com/adbarc92/command-center-agent-sandbox".into());
    let repo_slug = req.repo_slug.unwrap_or_else(|| "adbarc92/command-center-agent-sandbox".into());
    let base_branch = req.base_branch.unwrap_or_else(|| "main".into());
    let row = {
        let s = st.store.lock().unwrap();
        let committed = s.committed_spend(since).unwrap_or(0.0);
        if committed >= st.global_cap {
            return Err((StatusCode::TOO_MANY_REQUESTS, "global daily cost cap reached".into()));
        }
        let usd_budget = req.usd_budget.unwrap_or_else(|| (st.global_cap - committed).clamp(0.0, 15.0));
        let row = crate::store::SwarmRow {
            swarm_id: swarm_id.clone(), repo_url, repo_slug, base_branch,
            doc_path: req.doc_path.clone(), tier: phase_tier(req.tier.into()), mode: req.mode.clone(),
            lane_cap, usd_budget, per_lane_cap, status: "planning".into(), planner_cost: 0.0,
            lanes_launched: 0, lanes_dropped: 0, min_review_rounds: req.min_review_rounds.max(1),
            terminal_reason: None,
        };
        s.upsert_swarm(&row, now_ms()).ok();
        row
    };

    // Step 2+ — spawn the slow work with mode-selected seams (demo never pays).
    let st2 = st.clone();
    let id2 = swarm_id.clone();
    tokio::spawn(async move {
        match row.mode.as_str() {
            "demo" => {
                use crate::{docsource::FakeDocSource, planner::FakePlanner, swarm::Lane};
                let lanes = vec![
                    Lane { title: "Lane One".into(), task: "demo task 1".into(), rationale: "indep".into() },
                    Lane { title: "Lane Two".into(), task: "demo task 2".into(), rationale: "indep".into() },
                ];
                run_swarm(st2, id2, FakePlanner::ok(lanes, 0.01), FakeDocSource::new("# demo spec")).await;
            }
            _ => {
                use crate::{docsource::GitDocSource, planner::ClaudePlanner};
                run_swarm(st2, id2, ClaudePlanner::new(), GitDocSource::new()).await;
            }
        }
    });

    Ok(Json(CreateSwarmResp { swarm_id }))
}

#[derive(Serialize)]
struct SwarmSummary {
    swarm_id: String, status: String, lanes_launched: u32, lanes_dropped: u32,
    planner_cost: f64, doc_path: String,
}

async fn list_swarms(State(st): State<AppState>) -> Json<Vec<SwarmSummary>> {
    let rows = st.store.lock().unwrap().list_swarms().unwrap_or_default();
    Json(rows.into_iter().map(|r| SwarmSummary {
        swarm_id: r.swarm_id, status: r.status, lanes_launched: r.lanes_launched,
        lanes_dropped: r.lanes_dropped, planner_cost: r.planner_cost, doc_path: r.doc_path,
    }).collect())
}

#[derive(Serialize)]
struct SwarmDetail {
    swarm_id: String, status: String, planner_cost: f64,
    lanes_launched: u32, lanes_dropped: u32, awaiting_human: u64,
    spent_so_far: f64, lanes: Vec<LaneView>, units: Vec<String>,
}
#[derive(Serialize)]
struct LaneView { idx: u32, title: String, decision: String, unit_id: Option<String> }

async fn get_swarm(State(st): State<AppState>, Path(id): Path<String>)
    -> Result<Json<SwarmDetail>, StatusCode> {
    let s = st.store.lock().unwrap();
    let sw = s.get_swarm(&id).ok().flatten().ok_or(StatusCode::NOT_FOUND)?;
    let lanes = s.lanes_for_swarm(&id).unwrap_or_default();
    let (total, terminal, awaiting) = s.swarm_rollup(&id).unwrap_or((0, 0, 0));
    // Computed status: running→done only when every child unit is terminal.
    let status = if sw.status == "running" && total > 0 && terminal == total {
        "done".to_string()
    } else { sw.status.clone() };
    // "spent so far" = actual child cost + planner cost (NOT reservations).
    let unit_ids: Vec<String> = lanes.iter().filter_map(|l| l.unit_id.clone()).collect();
    let mut spent = sw.planner_cost;
    for uid in &unit_ids {
        if let Ok(Some(u)) = s.get_unit(uid) { spent += u.cost; }
    }
    Ok(Json(SwarmDetail {
        swarm_id: id, status, planner_cost: sw.planner_cost,
        lanes_launched: sw.lanes_launched, lanes_dropped: sw.lanes_dropped, awaiting_human: awaiting,
        spent_so_far: spent,
        lanes: lanes.iter().map(|l| LaneView {
            idx: l.idx, title: l.title.clone(), decision: l.decision.clone(), unit_id: l.unit_id.clone(),
        }).collect(),
        units: unit_ids,
    }))
}

/// Plan → admit → fan out, for an already-persisted `planning` swarm row.
/// Generic over the seams so tests inject fakes. Each store mutation is a short
/// sync critical section. Admission here mirrors the mission path: the
/// committed-spend check and the unit-row insert happen under ONE store-lock span
/// (per-lane re-check + commit_lane_unit), so the cap binds atomically (P2).
pub async fn run_swarm<P: Planner, D: DocSource>(st: AppState, swarm_id: String, planner: P, doc: D) {
    let Some(sw) = st.store.lock().unwrap().get_swarm(&swarm_id).ok().flatten() else { return };

    // 2. Plan (read doc, then decompose).
    let doc_text = match doc.read(&sw.repo_url, &sw.base_branch, &sw.doc_path).await {
        Ok(t) => t,
        Err(e) => return fail_swarm(&st, &swarm_id, sw.planner_cost, &format!("doc: {e}")),
    };
    let outcome = match planner.plan(&doc_text, sw.lane_cap as usize).await {
        Ok(o) => o,
        Err(e) => return fail_swarm(&st, &swarm_id, 0.0, &format!("planner: {e}")),
    };
    st.store.lock().unwrap()
        .update_swarm(&swarm_id, "planning", outcome.cost_usd, 0, 0, None, now_ms()).ok();
    if outcome.lanes.is_empty() {
        return fail_swarm(&st, &swarm_id, outcome.cost_usd, "planner returned zero lanes");
    }

    // 3. Admit (pure) + persist every lane's decision.
    let cfg = AdmissionConfig {
        lane_cap: sw.lane_cap as usize, usd_budget: sw.usd_budget,
        per_lane_cap: sw.per_lane_cap, planner_cost: outcome.cost_usd,
    };
    let decisions = admit_lanes(&outcome.lanes, &cfg);
    let mut dropped = 0u32;
    for (i, d) in &decisions {
        let lane = &outcome.lanes[*i];
        let dstr = match d {
            LaneDecision::Admit => "admit",
            LaneDecision::DropOverLaneCap => "drop_lane_cap",
            LaneDecision::DropOverBudget => "drop_budget",
        };
        if !matches!(d, LaneDecision::Admit) { dropped += 1; }
        st.store.lock().unwrap()
            .upsert_lane(&swarm_id, *i as u32, &lane.title, &lane.task, &lane.rationale, dstr, None).ok();
    }
    let admitted_idxs: Vec<usize> = decisions.iter()
        .filter(|(_, d)| matches!(d, LaneDecision::Admit)).map(|(i, _)| *i).collect();
    if admitted_idxs.is_empty() {
        st.store.lock().unwrap()
            .update_swarm(&swarm_id, "empty", outcome.cost_usd, 0, dropped, Some("no lanes admitted"), now_ms()).ok();
        return;
    }

    // 4. Fan out (idempotent per-lane).
    st.store.lock().unwrap()
        .update_swarm(&swarm_id, "fanning_out", outcome.cost_usd, 0, dropped, None, now_ms()).ok();
    let since = now_ms() - 24 * 3600 * 1000;
    let mut launched = 0u32;
    'fanout: for (pos, idx) in admitted_idxs.iter().copied().enumerate() {
        let lane = &outcome.lanes[idx];
        // Admission critical section: re-check committed spend, mint id, commit row+link.
        let (unit_id, spec) = {
            let s = st.store.lock().unwrap();
            if s.committed_spend(since).unwrap_or(0.0) >= st.global_cap {
                // Cap tripped: mark this lane and all remaining admitted lanes as
                // drop_global_cap so no admit row is left dangling with unit_id = NULL.
                for rem_idx in admitted_idxs[pos..].iter().copied() {
                    let rem_lane = &outcome.lanes[rem_idx];
                    s.upsert_lane(&swarm_id, rem_idx as u32, &rem_lane.title, &rem_lane.task,
                        &rem_lane.rationale, "drop_global_cap", None).ok();
                    dropped += 1;
                }
                break 'fanout;
            }
            let n = st.next_id.fetch_add(1, Ordering::Relaxed);
            let unit_id = format!("u{n}");
            let spec = lane_spec(&sw, &unit_id, idx, lane);
            let mut row = row_from_spec(&spec, &sw.mode);
            row.swarm_id = Some(swarm_id.clone());
            s.commit_lane_unit(&swarm_id, idx as u32, &row, now_ms()).ok();
            (unit_id, spec)
        };
        // Spawn the driver outside the lock; the row already exists.
        if spawn_driver_for(&st, spec, &sw.mode, &unit_id).is_ok() {
            launched += 1;
        }
    }
    st.store.lock().unwrap()
        .update_swarm(&swarm_id, "running", outcome.cost_usd, launched, dropped, None, now_ms()).ok();
}

fn fail_swarm(st: &AppState, swarm_id: &str, planner_cost: f64, reason: &str) {
    st.store.lock().unwrap()
        .update_swarm(swarm_id, "failed", planner_cost, 0, 0, Some(reason), now_ms()).ok();
}

/// Build a lane's UnitSpec from the swarm config.
fn lane_spec(sw: &crate::store::SwarmRow, unit_id: &str, idx: usize, lane: &crate::swarm::Lane) -> UnitSpec {
    UnitSpec {
        unit_id: unit_id.into(),
        tier: parse_tier(&sw.tier),
        task: lane.task.clone(),
        usd_cap: sw.per_lane_cap,
        wall_clock_secs: 1800,
        gate: GateConfig { min_review_rounds: sw.min_review_rounds.max(1) },
        repo_url: sw.repo_url.clone(),
        repo_slug: sw.repo_slug.clone(),
        base_branch: sw.base_branch.clone(),
        branch: format!("agent/{}/{}-{}", sw.swarm_id, idx, slug(&lane.title)),
        test_cmd: "node --test".into(),
        oracle_frozen: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn building_row(id: &str) -> UnitRow {
        UnitRow {
            unit_id: id.into(),
            tier: "t1".into(),
            task: "t".into(),
            repo_url: "u".into(),
            repo_slug: "s".into(),
            base_branch: "main".into(),
            branch: format!("agent/{id}"),
            test_cmd: "node --test".into(),
            usd_cap: 1.0,
            wall_clock_secs: 600,
            phase: "building".into(),
            cost: 0.2,
            last_seq: 7,
            oracle_frozen: true,
            terminal_reason: None,
            mode: "real".into(),
            min_review_rounds: 1,
            swarm_id: None,
        }
    }

    #[tokio::test]
    async fn reconcile_halts_stranded_unit_with_coherent_event() {
        let store = Arc::new(Mutex::new(Store::open_memory().unwrap()));
        store.lock().unwrap().upsert_unit(&building_row("u1"), 1).unwrap();
        let state = AppState::new(store.clone());
        // FakeRunner reports no running containers → unit is a stranded orphan.
        let runner = FakeRunner::new(vec![]);

        reconcile_on_startup(&state, &runner).await;

        let s = store.lock().unwrap();
        let row = s.get_unit("u1").unwrap().unwrap();
        assert_eq!(row.phase, "halted", "stranded unit marked halted");
        assert_eq!(row.last_seq, 8, "synthetic event bumped last_seq");
        assert_eq!(row.terminal_reason.as_deref(), Some("daemon restarted"));
        // The synthetic phase_changed event is in the log (coherent with the row).
        let evs = s.events_since("u1", 7).unwrap();
        assert_eq!(evs.len(), 1);
        assert!(evs[0].contains("halted"));
    }

    #[tokio::test]
    async fn create_mission_refused_over_global_cap() {
        // A pre-existing unit's spend is over the default $20 rolling-24h cap, so
        // admission of a new mission is refused (429) before any unit is built.
        let store = Arc::new(Mutex::new(Store::open_memory().unwrap()));
        {
            let s = store.lock().unwrap();
            let mut r = building_row("spent");
            r.cost = 999.0;
            s.upsert_unit(&r, now_ms()).unwrap();
        }
        let state = AppState::new(store);
        let resp = create_mission(
            State(state),
            Json(CreateReq {
                task: "t".into(),
                tier: TierReq::T1,
                mode: "demo".into(),
                min_review_rounds: 1,
            }),
        )
        .await;
        match resp {
            Err((code, _)) => assert_eq!(code, StatusCode::TOO_MANY_REQUESTS),
            Ok(_) => panic!("expected 429 over the global cap"),
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn register_unit_if_absent_is_atomic_under_races() {
        // Two registrations racing on the SAME id must yield exactly one handle:
        // the check-and-insert is one critical section under the map mutex, so a
        // concurrent caller sees the inserted handle and gets None. This is the
        // load-bearing guarantee behind "two simultaneous Resume clicks → one
        // driver" (tested without a Docker driver — per the rehydration design).
        let state = AppState::default();
        let a = state.clone();
        let b = state.clone();
        let h1 = tokio::spawn(async move { register_unit_if_absent(&a, "u1").is_some() });
        let h2 = tokio::spawn(async move { register_unit_if_absent(&b, "u1").is_some() });
        let won = [h1.await.unwrap(), h2.await.unwrap()];
        assert_eq!(won.iter().filter(|&&w| w).count(), 1, "exactly one registration wins");
        assert_eq!(state.units.lock().unwrap().len(), 1, "no duplicate handle");
    }

    #[tokio::test]
    async fn rehydrate_skips_units_absent_from_the_store() {
        // A command for a unit that was never persisted must not fabricate a handle.
        let state = AppState::default();
        rehydrate(&state, "ghost");
        assert!(!state.units.lock().unwrap().contains_key("ghost"));
        assert_eq!(state.units.lock().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn rehydrate_demo_unit_resumes_as_demo_to_done() {
        // A halted DEMO unit, rehydrated, resumes on its persisted FakeRunner (not a
        // real Docker provision) and drives to Done — proving mode survives a restart.
        let store = Arc::new(Mutex::new(Store::open_memory().unwrap()));
        {
            let s = store.lock().unwrap();
            let mut r = building_row("u1");
            r.mode = "demo".into();
            r.min_review_rounds = 1;
            r.phase = "halted".into();
            r.oracle_frozen = true; // frozen → resume skips the oracle
            r.last_seq = 3;
            r.cost = 0.1;
            s.upsert_unit(&r, now_ms()).unwrap();
        }
        let state = AppState::new(store);
        rehydrate(&state, "u1");

        // Grab the live handle, subscribe, then drive it on with Resume.
        let (mut rx, tx) = {
            let units = state.units.lock().unwrap();
            let h = units.get("u1").expect("rehydrated handle present");
            (h.bcast.subscribe(), h.cmd_tx.clone())
        };
        tx.send(Command::Resume { cmd_id: "r1".into() }).expect("driver alive");

        let mut saw_oracle = false;
        let mut done = false;
        for _ in 0..200 {
            let env = rx.recv().await.expect("event stream closed before Done");
            match env.event {
                Event::OracleProposed { .. } => saw_oracle = true,
                Event::Done { ref result } => {
                    assert_eq!(result, "done", "demo unit resumes to a successful Done");
                    done = true;
                    break;
                }
                _ => {}
            }
        }
        assert!(done, "rehydrated demo unit reached Done");
        assert!(!saw_oracle, "resume must not re-run the oracle");
    }

    #[tokio::test]
    async fn next_id_seeds_above_persisted_units() {
        let store = Arc::new(Mutex::new(Store::open_memory().unwrap()));
        store.lock().unwrap().upsert_unit(&building_row("u5"), 1).unwrap();
        let state = AppState::new(store);
        // Fresh allocation must not collide with u5.
        let n = state.next_id.fetch_add(1, Ordering::Relaxed);
        assert_eq!(n, 6, "next mint is u6, never an existing id");
    }

    fn swarm_row_srv(id: &str, status: &str) -> crate::store::SwarmRow {
        crate::store::SwarmRow {
            swarm_id: id.into(), repo_url: "u".into(), repo_slug: "s".into(), base_branch: "main".into(),
            doc_path: "spec.md".into(), tier: "t1".into(), mode: "demo".into(), lane_cap: 8, usd_budget: 15.0,
            per_lane_cap: 5.0, status: status.into(), planner_cost: 0.0, lanes_launched: 0,
            lanes_dropped: 0, min_review_rounds: 1, terminal_reason: None,
        }
    }

    #[tokio::test]
    async fn next_swarm_seeds_above_persisted_swarms() {
        let store = Arc::new(Mutex::new(Store::open_memory().unwrap()));
        store.lock().unwrap().upsert_swarm(&swarm_row_srv("sw5", "running"), 1).unwrap();
        let state = AppState::new(store);
        let n = state.next_swarm.fetch_add(1, Ordering::Relaxed);
        assert_eq!(n, 6, "next swarm mint is sw6, never an existing id");
    }

    #[tokio::test]
    async fn create_mission_refused_when_committed_reservations_breach_cap() {
        // A single NON-TERMINAL unit reserving > $20 must block a new mission, even
        // though its recorded cost is small — committed_spend counts the reservation.
        let store = Arc::new(Mutex::new(Store::open_memory().unwrap()));
        {
            let s = store.lock().unwrap();
            let mut r = building_row("rsv");
            r.phase = "building".into(); // non-terminal
            r.cost = 0.1;
            r.usd_cap = 25.0;            // reservation alone exceeds the $20 cap
            s.upsert_unit(&r, now_ms()).unwrap();
        }
        let state = AppState::new(store);
        let resp = create_mission(State(state), Json(CreateReq {
            task: "t".into(), tier: TierReq::T1, mode: "demo".into(), min_review_rounds: 1,
        })).await;
        match resp {
            Err((code, _)) => assert_eq!(code, StatusCode::TOO_MANY_REQUESTS),
            Ok(_) => panic!("expected 429 — committed reservation breaches the cap"),
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_missions_cannot_both_breach_the_cap() {
        // Global cap is $20. Pre-load committed spend so exactly ONE more default
        // $5-cap unit crosses the ceiling: seed reserves 16, first admit pushes
        // committed to 21 (>=20) so the second is refused — but ONLY if check+insert
        // are atomic. An open race would let both observe 16<20 and both admit.
        let store = std::sync::Arc::new(std::sync::Mutex::new(Store::open_memory().unwrap()));
        {
            let s = store.lock().unwrap();
            let mut r = building_row("seed"); r.phase = "building".into(); r.usd_cap = 16.0; r.cost = 0.0;
            s.upsert_unit(&r, now_ms()).unwrap();
        }
        let state = AppState::new(store.clone());
        let a = state.clone();
        let b = state.clone();
        let h1 = tokio::spawn(async move {
            create_mission(State(a), Json(CreateReq { task: "t".into(), tier: TierReq::T1, mode: "demo".into(), min_review_rounds: 1 })).await.is_ok()
        });
        let h2 = tokio::spawn(async move {
            create_mission(State(b), Json(CreateReq { task: "t".into(), tier: TierReq::T1, mode: "demo".into(), min_review_rounds: 1 })).await.is_ok()
        });
        let wins = [h1.await.unwrap(), h2.await.unwrap()].iter().filter(|&&w| w).count();
        assert_eq!(wins, 1, "exactly one mission admitted; the cap binds atomically");
    }

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

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn run_swarm_fans_out_admitted_lanes_to_done() {
        use crate::docsource::FakeDocSource;
        use crate::planner::FakePlanner;
        use crate::swarm::Lane;

        let state = AppState::default();
        let sw = crate::store::SwarmRow {
            swarm_id: "sw1".into(), repo_url: "https://github.com/x/y".into(), repo_slug: "x/y".into(),
            base_branch: "main".into(), doc_path: "spec.md".into(), tier: "t1".into(), mode: "demo".into(),
            lane_cap: 8, usd_budget: 100.0, per_lane_cap: 5.0, status: "planning".into(),
            planner_cost: 0.0, lanes_launched: 0, lanes_dropped: 0, min_review_rounds: 1,
            terminal_reason: None,
        };
        state.store.lock().unwrap().upsert_swarm(&sw, now_ms()).unwrap();

        let lanes = vec![
            Lane { title: "Add A".into(), task: "do A".into(), rationale: "indep".into() },
            Lane { title: "Add B".into(), task: "do B".into(), rationale: "indep".into() },
        ];
        let planner = FakePlanner::ok(lanes, 0.2);
        let doc = FakeDocSource::new("# spec");

        run_swarm(state.clone(), "sw1".into(), planner, doc).await;

        let units = state.store.lock().unwrap().list_units().unwrap();
        let mine: Vec<_> = units.iter().filter(|u| u.swarm_id.as_deref() == Some("sw1")).cloned().collect();
        assert_eq!(mine.len(), 2);
        assert!(mine.iter().all(|u| (u.usd_cap - 5.0).abs() < 1e-9));
        let branches: std::collections::HashSet<_> = mine.iter().map(|u| u.branch.clone()).collect();
        assert_eq!(branches.len(), 2, "unique branches");
        assert!(branches.iter().all(|b| b.starts_with("agent/sw1/")));

        let mut done = false;
        for _ in 0..500 {
            let (total, term, _) = state.store.lock().unwrap().swarm_rollup("sw1").unwrap();
            if total == 2 && term == 2 { done = true; break; }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        assert!(done, "all lanes reached terminal");
        assert_eq!(state.store.lock().unwrap().get_swarm("sw1").unwrap().unwrap().status, "running");
    }

    #[tokio::test]
    async fn run_swarm_zero_admitted_is_empty_not_done() {
        use crate::docsource::FakeDocSource;
        use crate::planner::FakePlanner;
        use crate::swarm::Lane;
        let state = AppState::default();
        let sw = crate::store::SwarmRow {
            swarm_id: "sw2".into(), repo_url: "u".into(), repo_slug: "s".into(), base_branch: "main".into(),
            doc_path: "spec.md".into(), tier: "t1".into(), mode: "demo".into(),
            lane_cap: 8, usd_budget: 1.0, per_lane_cap: 5.0, status: "planning".into(),
            planner_cost: 0.0, lanes_launched: 0, lanes_dropped: 0, min_review_rounds: 1,
            terminal_reason: None,
        };
        state.store.lock().unwrap().upsert_swarm(&sw, now_ms()).unwrap();
        let planner = FakePlanner::ok(vec![Lane { title: "A".into(), task: "a".into(), rationale: "r".into() }], 0.0);
        run_swarm(state.clone(), "sw2".into(), planner, FakeDocSource::new("# spec")).await;
        let got = state.store.lock().unwrap().get_swarm("sw2").unwrap().unwrap();
        assert_eq!(got.status, "empty", "lanes produced but none admitted ⇒ empty, never done");
    }

    #[tokio::test]
    async fn run_swarm_planner_error_is_failed() {
        use crate::docsource::FakeDocSource;
        use crate::planner::FakePlanner;
        let state = AppState::default();
        let sw = crate::store::SwarmRow {
            swarm_id: "sw3".into(), repo_url: "u".into(), repo_slug: "s".into(), base_branch: "main".into(),
            doc_path: "spec.md".into(), tier: "t1".into(), mode: "demo".into(), lane_cap: 8, usd_budget: 15.0,
            per_lane_cap: 5.0, status: "planning".into(), planner_cost: 0.0, lanes_launched: 0,
            lanes_dropped: 0, min_review_rounds: 1, terminal_reason: None,
        };
        state.store.lock().unwrap().upsert_swarm(&sw, now_ms()).unwrap();
        run_swarm(state.clone(), "sw3".into(), FakePlanner::err("boom"), FakeDocSource::new("x")).await;
        assert_eq!(state.store.lock().unwrap().get_swarm("sw3").unwrap().unwrap().status, "failed");
    }

    #[tokio::test]
    async fn post_swarms_validates_then_returns_id() {
        let state = AppState::default();
        // unknown mode → error, no row created
        let bad = create_swarm(State(state.clone()), Json(CreateSwarmReq {
            doc_path: "spec.md".into(), mode: "weird".into(), ..Default::default()
        })).await;
        assert!(bad.is_err());
        // demo → ok, returns an id
        let ok = create_swarm(State(state.clone()), Json(CreateSwarmReq {
            doc_path: "spec.md".into(), mode: "demo".into(), ..Default::default()
        })).await.expect("ok");
        assert!(ok.0.swarm_id.starts_with("sw"));
    }

    #[tokio::test]
    async fn reconcile_marks_planning_swarm_failed_and_resumes_fanning_out() {
        let store = Arc::new(Mutex::new(Store::open_memory().unwrap()));
        {
            let s = store.lock().unwrap();
            // A swarm stuck mid-planning at crash → must become failed.
            s.upsert_swarm(&swarm_row_srv("swP", "planning"), now_ms()).unwrap();
            // A swarm mid-fan-out: lane 0 launched (unit exists), lane 1 not.
            let mut f = swarm_row_srv("swF", "fanning_out"); f.mode = "demo".into(); f.min_review_rounds = 1;
            s.upsert_swarm(&f, now_ms()).unwrap();
            s.upsert_lane("swF", 0, "A", "ta", "r", "admit", Some("u1")).unwrap();
            let mut u = building_row("u1"); u.swarm_id = Some("swF".into()); u.phase = "building".into();
            s.upsert_unit(&u, now_ms()).unwrap();
            s.upsert_lane("swF", 1, "B", "tb", "r", "admit", None).unwrap();
        }
        let state = AppState::new(store.clone());
        let runner = FakeRunner::new(vec![]);
        reconcile_on_startup(&state, &runner).await;

        let s = store.lock().unwrap();
        assert_eq!(s.get_swarm("swP").unwrap().unwrap().status, "failed");
        // swF resumed: lane 1 now has a unit_id, status running.
        let lanes = s.lanes_for_swarm("swF").unwrap();
        assert!(lanes[1].unit_id.is_some(), "missing lane was committed on resume");
        assert_eq!(s.get_swarm("swF").unwrap().unwrap().status, "running");
    }

    #[tokio::test]
    async fn run_swarm_marks_remaining_lanes_drop_global_cap_when_cap_trips_midway() {
        use crate::docsource::FakeDocSource;
        use crate::planner::FakePlanner;
        use crate::swarm::Lane;
        let state = AppState::default();
        // Pre-load committed spend at/above the global cap via a non-terminal big-cap unit,
        // so the very first fan-out re-check trips the global cap.
        {
            let s = state.store.lock().unwrap();
            let mut r = building_row("rsv");
            r.phase = "building".into();
            r.usd_cap = 25.0;
            r.cost = 0.0;
            s.upsert_unit(&r, now_ms()).unwrap();
        }
        let sw = crate::store::SwarmRow {
            swarm_id: "swG".into(), repo_url: "u".into(), repo_slug: "s".into(), base_branch: "main".into(),
            doc_path: "spec.md".into(), tier: "t1".into(), mode: "demo".into(), lane_cap: 8, usd_budget: 100.0,
            per_lane_cap: 5.0, status: "planning".into(), planner_cost: 0.0, lanes_launched: 0,
            lanes_dropped: 0, min_review_rounds: 1, terminal_reason: None,
        };
        state.store.lock().unwrap().upsert_swarm(&sw, now_ms()).unwrap();
        let lanes = vec![
            Lane { title: "A".into(), task: "a".into(), rationale: "r".into() },
            Lane { title: "B".into(), task: "b".into(), rationale: "r".into() },
        ];
        run_swarm(state.clone(), "swG".into(), FakePlanner::ok(lanes, 0.0), FakeDocSource::new("# spec")).await;
        // Both lanes were admitted by admit_lanes (budget 100 allows them) but the global
        // cap trips at fan-out, so NEITHER launches and BOTH are recorded drop_global_cap.
        let lanes = state.store.lock().unwrap().lanes_for_swarm("swG").unwrap();
        assert!(lanes.iter().all(|l| l.decision == "drop_global_cap"), "no admit rows left dangling");
        assert!(lanes.iter().all(|l| l.unit_id.is_none()), "nothing launched");
    }
}
