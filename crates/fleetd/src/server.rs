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
use crate::reconcile::{reconcile, Action};
use crate::runner::{ExecOutput, Runner, UnitSpec};
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
        Self {
            units: Arc::new(Mutex::new(HashMap::new())),
            next_id: Arc::new(AtomicU64::new(1)),
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
    // Admission-only global cap: refuse a new mission once rolling-24h spend has
    // hit the ceiling. Read fresh each admission (low frequency; no shared atomic).
    let since = now_ms() - 24 * 3600 * 1000;
    let spent = st.store.lock().unwrap().spend_since(since).unwrap_or(0.0);
    if spent >= st.global_cap {
        return Err((StatusCode::TOO_MANY_REQUESTS, "global daily cost cap reached".into()));
    }

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

    // Validate the mode BEFORE registering a handle, so a bad request never
    // leaves a driverless unit in the map.
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

    // Register the per-unit handle atomically, then spawn its driver. A fresh
    // unit_id is always unique, so this returns Some.
    let (cmd_rx, evt_tx) =
        register_unit_if_absent(&st, &unit_id).expect("freshly-allocated unit_id is unique");

    match runner_mode.as_str() {
        "demo" => {
            let runner = FakeRunner::new(demo_script(&spec));
            tokio::spawn(run(runner, FakeForge::default(), spec, fresh_ctx(&st), cmd_rx, evt_tx));
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
            tokio::spawn(run(runner, forge, spec, fresh_ctx(&st), cmd_rx, evt_tx));
        }
        _ => unreachable!("mode validated above"),
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

/// Reconstruct a `UnitSpec` from its persisted row for rehydration. NOTE: the
/// review-gate floor isn't persisted in `units`, so resumed units default to 2
/// rounds. A unit originally launched with a higher floor could therefore open
/// its gate a round earlier after a restart-resume; persisting the floor is a
/// follow-up.
fn spec_from_row(r: &UnitRow) -> UnitSpec {
    UnitSpec {
        unit_id: r.unit_id.clone(),
        tier: parse_tier(&r.tier),
        task: r.task.clone(),
        usd_cap: r.usd_cap,
        wall_clock_secs: r.wall_clock_secs,
        gate: GateConfig { min_review_rounds: 2 },
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
        .filter(|r| !matches!(r.phase.as_str(), "done" | "no_change" | "failed"))
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
