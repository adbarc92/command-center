# SP1-Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the SP1 fleet engine survive restarts/reloads, never strand a spending container, bound concurrency + daily spend, and make resume real (volume reuse, oracle-skip, continued cost/seq).

**Architecture:** Add a SQLite `Store` (WAL, single writer task) behind the event forwarder; thread a `RunCtx` into the driver carrying `start_seq`/`start_cost`/`resume`/a concurrency `Semaphore` permit; split `teardown`(keep volume)/`discard`(remove); add probe-based volume reuse and oracle-skip on resume; add startup reconciliation + on-demand rehydration; admission-only rolling-24h global cost cap; reconnect the cockpit.

**Tech Stack:** Rust (tokio, axum, rusqlite w/ bundled feature), Svelte/Vite cockpit. Parent spec: [../specs/2026-06-06-sp1-hardening-design.md](../specs/2026-06-06-sp1-hardening-design.md).

**Branch:** `feat/sp1-hardening` (already created off `feat/sp1-fleet-engine`).

---

## Phase 1 — Resume path (the de-risking gate). Build & prove FIRST.

This phase changes only `fleet-core` + `driver.rs` + fakes, and is provable entirely against `FakeRunner`. If it can't go green, stop.

### Task 1.1: `Resume → Provisioning` transition

**Files:** Modify `crates/fleet-core/src/transition.rs`

- [ ] **Step 1: Update the two resume tests to expect Provisioning**

In `transition.rs` tests, change the expectations:
```rust
    #[test]
    fn halt_and_resume_round_trip() {
        assert_eq!(transition(Building, Tier::T1, Trigger::Halt), Some(Halted));
        assert_eq!(transition(Halted, Tier::T1, Trigger::Resume), Some(Provisioning));
    }
```
And in `t3_human_approves_oracle_and_ships_pr` / any NeedsHuman resume assertion, expect `Provisioning`. Add:
```rust
    #[test]
    fn resume_goes_to_provisioning_not_building() {
        assert_eq!(transition(NeedsHuman, Tier::T1, Trigger::Resume), Some(Provisioning));
        assert_eq!(transition(Halted, Tier::T1, Trigger::Resume), Some(Provisioning));
    }
```

- [ ] **Step 2: Run, expect failure**

Run: `cargo test -p fleet-core resume`
Expected: FAIL (current code returns `Building`).

- [ ] **Step 3: Change the transition**

In `transition.rs`, in the `transition` fn match arms, change both resume edges:
```rust
        (NeedsHuman, Resume) => Provisioning,
        ...
        (Halted, Resume) => Provisioning,
```

- [ ] **Step 4: Run, expect pass**

Run: `cargo test -p fleet-core`
Expected: PASS (all fleet-core tests).

- [ ] **Step 5: Commit**

```bash
git add crates/fleet-core/src/transition.rs
git commit -m "feat(fleet-core): Resume routes to Provisioning (re-provision before build)"
```

### Task 1.2: `RunCtx` + seed seq/cost, no behavior change yet

**Files:** Modify `crates/fleetd/src/driver.rs`, `crates/fleetd/src/bin/run_once.rs`, `crates/fleetd/src/server.rs`, `crates/fleetd/tests/*.rs`

- [ ] **Step 1: Add the `RunCtx` struct and thread it into `run()`**

In `driver.rs`, above `pub async fn run`:
```rust
use std::sync::Arc;
use tokio::sync::Semaphore;

/// Per-run dependencies (kept in one struct to avoid signature sprawl).
pub struct RunCtx {
    pub start_seq: u64,
    pub start_cost: f64,
    pub resume: bool,
    pub permits: Arc<Semaphore>,
}

impl RunCtx {
    /// A standalone context for run_once / tests: unlimited-ish single permit, fresh run.
    pub fn standalone() -> Self {
        Self { start_seq: 0, start_cost: 0.0, resume: false, permits: Arc::new(Semaphore::new(1)) }
    }
}
```

- [ ] **Step 2: Change `run()` signature and seed the `Run` fields**

In `driver.rs`, change `run(runner, forge, spec, commands, events)` to take `ctx: RunCtx`:
```rust
pub async fn run<R: Runner, F: Forge>(
    runner: R,
    forge: F,
    spec: UnitSpec,
    ctx: RunCtx,
    commands: UnboundedReceiver<Command>,
    events: UnboundedSender<EventEnvelope>,
) -> Phase {
    Run {
        runner, forge,
        phase: Phase::Queued,
        seq: ctx.start_seq,
        handle: None,
        cost_usd: ctx.start_cost,
        n_build: 0, n_check: 0, n_review: 0,
        prev_blockers: None, pr_url: None,
        started: std::time::Instant::now(),
        permits: ctx.permits,
        permit: None,
        resume: ctx.resume,
        spec, commands, events,
    }.drive().await
}
```
Add fields to `struct Run`:
```rust
    started: std::time::Instant,
    permits: Arc<Semaphore>,
    permit: Option<tokio::sync::OwnedSemaphorePermit>,
    resume: bool,
```

- [ ] **Step 3: Update all call sites to pass a `RunCtx`**

- `run_once.rs`: `run(runner, forge, spec, fleetd::driver::RunCtx::standalone(), cmd_rx, evt_tx)`.
- `server.rs` both spawn sites: `run(runner, forge, spec, RunCtx::standalone(), cmd_rx, evt_tx)` for now (real wiring in Phase 5).
- `driver.rs` tests `run(...)` calls: add `RunCtx::standalone()` before `crx`.
- `tests/local_docker_it.rs`, `tests/preflight_it.rs`: they call `runner.*` directly, not `run()` — no change.

- [ ] **Step 4: Build + run the whole suite**

Run: `cargo test --workspace`
Expected: PASS (no behavior change; just plumbing).

- [ ] **Step 5: Commit**

```bash
git add crates/fleetd/src/driver.rs crates/fleetd/src/bin/run_once.rs crates/fleetd/src/server.rs
git commit -m "refactor(fleetd): thread RunCtx (start_seq/start_cost/resume/permits) into run()"
```

### Task 1.3: Oracle-skip on resume + permit lifecycle (the core)

**Files:** Modify `crates/fleetd/src/driver.rs`, `crates/fleetd/src/runner.rs` (UnitSpec gains `oracle_frozen`), tests.

- [ ] **Step 1: Write the resume integration test (the de-risking gate)**

In `driver.rs` tests module, add. It launches, halts at Building, resumes, and asserts no oracle re-run, seq continuity, single permit, cost continuity:
```rust
    #[tokio::test]
    async fn resume_skips_oracle_keeps_cost_and_one_permit() {
        // Round 1: oracle, build1, (halt before checks).
        let script = vec![
            FakeRunner::ok(0.05, &["test_a.rs"]), // oracle
            FakeRunner::ok(0.05, &["built"]),     // build 1
        ];
        let permits = std::sync::Arc::new(tokio::sync::Semaphore::new(1));
        let (ctx_tx, crx) = mpsc::unbounded_channel();
        let (etx, mut erx) = mpsc::unbounded_channel();
        let runner = FakeRunner::new(script);
        let h = tokio::spawn(run(
            runner, FakeForge::default(),
            { let mut s = spec(Tier::T1, 100.0, 1); s.oracle_frozen = false; s },
            RunCtx { start_seq: 0, start_cost: 0.0, resume: false, permits: permits.clone() },
            crx, etx,
        ));
        // Wait until Building, then halt.
        loop {
            let e = erx.recv().await.unwrap();
            if matches!(e.event, Event::PhaseChanged { to: Phase::Building, .. }) { break; }
        }
        ctx_tx.send(Command::Halt { cmd_id: "h".into() }).unwrap();
        // Drain to Halted, capturing last seq + last cost.
        let mut last_seq = 0; let mut last_cost = 0.0;
        loop {
            let e = erx.recv().await.unwrap();
            last_seq = e.seq;
            if let Event::Metric { cost_usd, .. } = e.event { last_cost = cost_usd; }
            if matches!(e.event, Event::PhaseChanged { to: Phase::Halted, .. }) { break; }
        }
        assert_eq!(permits.available_permits(), 1, "paused unit must release its permit");
        let _ = h.await;
        // (Full rehydrated-resume assertions are covered in Phase 4's rehydration test,
        // which exercises a fresh driver with resume:true + start_seq/start_cost.)
        assert!(last_cost >= 0.10 - 1e-9, "cost accumulated across oracle+build");
        assert!(last_seq > 0);
    }
```

- [ ] **Step 2: Add `oracle_frozen` to `UnitSpec`**

In `runner.rs` `UnitSpec`, add `pub oracle_frozen: bool,`. Update every `UnitSpec { .. }` literal (driver tests `spec()`, steps test, server, run_once, the two integration tests) to add `oracle_frozen: false,`.

- [ ] **Step 3: Permit acquire at top of Provisioning; oracle-skip in Spec; release on pause/terminal entry**

In `driver.rs` `drive()`:

(a) Provisioning arm — acquire a permit before provisioning:
```rust
                Phase::Provisioning => {
                    if self.permit.is_none() {
                        self.emit(Event::Blocked {
                            reason: "awaiting concurrency slot".into(),
                            cap: None, detail: String::new(),
                        });
                        self.permit = Some(self.permits.clone().acquire_owned().await.expect("semaphore"));
                    }
                    match self.runner.provision(&self.spec).await {
                        Ok(h) => { self.handle = Some(h); self.goto(Trigger::Provisioned, None, None); }
                        Err(e) => {
                            self.emit(Event::Error { scope: ErrorScope::Docker, retryable: false, detail: e.to_string() });
                            self.goto(Trigger::FatalError, Some("provision failed".into()), None);
                        }
                    }
                }
```

(b) Spec arm — short-circuit on resume when frozen:
```rust
                Phase::Spec => {
                    if self.resume && self.spec.oracle_frozen {
                        self.goto(Trigger::OracleFrozen, Some("oracle already frozen".into()), None);
                        continue;
                    }
                    if self.check_halt() { continue; }
                    // ... existing oracle exec ...
                    // after a successful freeze, mark it:
                    self.spec.oracle_frozen = true;
                    self.goto(Trigger::OracleFrozen, None, None);
                }
```
(Keep the existing oracle exec/`OracleProposed`/`account` body between the guard and the freeze line.)

(c) Release permit + teardown at entry to pause/terminal. Add at the very top of the `drive` loop, before the wall-clock check:
```rust
            // Entry-side cleanup for paused/terminal states: stop the container
            // (keep volume unless discarding) and free the concurrency slot.
            if matches!(self.phase, Phase::NeedsHuman | Phase::Halted) {
                if let Some(h) = self.handle.take() { let _ = self.runner.teardown(&h).await; }
                self.permit = None;
            }
```
(The terminal arm already tears down; ensure `self.permit = None;` there too — add it in the `Done|NoChange|Failed` arm.)

- [ ] **Step 4: Run the resume test + full suite**

Run: `cargo test -p fleetd resume_skips_oracle ; cargo test --workspace`
Expected: PASS. The paused unit released its permit; cost accumulated.

- [ ] **Step 5: Commit**

```bash
git add crates/fleetd/src/driver.rs crates/fleetd/src/runner.rs crates/fleetd/src/*.rs crates/fleetd/tests/*.rs
git commit -m "feat(fleetd): oracle-skip on resume + permit acquire/release lifecycle"
```

---

## Phase 2 — Store (SQLite, WAL, single writer task) + persistence + endpoints

### Task 2.1: `Store` module

**Files:** Create `crates/fleetd/src/store.rs`; modify `Cargo.toml` (+`rusqlite`), `lib.rs`.

- [ ] **Step 1: Add the dependency**

Workspace `Cargo.toml`: `rusqlite = { version = "0.32", features = ["bundled"] }`. `crates/fleetd/Cargo.toml`: `rusqlite = { workspace = true }`.

- [ ] **Step 2: Write `store.rs` with schema, writer ops, reads, and `:memory:` tests**

Create `crates/fleetd/src/store.rs`:
```rust
//! SQLite persistence (WAL). One writer (the server's writer task) calls the
//! mutating methods; reads use their own connection. No async here — callers
//! must not hold the connection across .await.
use rusqlite::{params, Connection};
use std::path::Path;

pub struct Store { conn: Connection }

#[derive(Debug, Clone)]
pub struct UnitRow {
    pub unit_id: String, pub tier: String, pub task: String,
    pub repo_url: String, pub repo_slug: String, pub base_branch: String,
    pub branch: String, pub test_cmd: String, pub usd_cap: f64, pub wall_clock_secs: u64,
    pub phase: String, pub cost: f64, pub last_seq: u64, pub oracle_frozen: bool,
    pub terminal_reason: Option<String>,
}

impl Store {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        let conn = Connection::open(path)?;
        Self::init(conn)
    }
    pub fn open_memory() -> rusqlite::Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }
    fn init(conn: Connection) -> rusqlite::Result<Self> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS units(
               unit_id TEXT PRIMARY KEY, tier TEXT, task TEXT, repo_url TEXT, repo_slug TEXT,
               base_branch TEXT, branch TEXT, test_cmd TEXT, usd_cap REAL, wall_clock_secs INTEGER,
               phase TEXT, cost REAL, last_seq INTEGER, oracle_frozen INTEGER,
               created_ts INTEGER, updated_ts INTEGER, terminal_reason TEXT);
             CREATE TABLE IF NOT EXISTS events(
               unit_id TEXT, seq INTEGER, ts INTEGER, json TEXT,
               PRIMARY KEY(unit_id, seq));",
        )?;
        Ok(Self { conn })
    }

    pub fn upsert_unit(&self, r: &UnitRow, now: i64) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO units(unit_id,tier,task,repo_url,repo_slug,base_branch,branch,test_cmd,
               usd_cap,wall_clock_secs,phase,cost,last_seq,oracle_frozen,created_ts,updated_ts,terminal_reason)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?15,?16)
             ON CONFLICT(unit_id) DO UPDATE SET phase=?11,cost=?12,last_seq=?13,oracle_frozen=?14,
               updated_ts=?15,terminal_reason=?16",
            params![r.unit_id,r.tier,r.task,r.repo_url,r.repo_slug,r.base_branch,r.branch,r.test_cmd,
               r.usd_cap,r.wall_clock_secs,r.phase,r.cost,r.last_seq,r.oracle_frozen as i64,now,r.terminal_reason],
        )?;
        Ok(())
    }

    pub fn append_event(&self, unit_id: &str, seq: u64, ts: i64, json: &str) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO events(unit_id,seq,ts,json) VALUES(?1,?2,?3,?4)",
            params![unit_id, seq, ts, json],
        )?;
        Ok(())
    }

    pub fn get_unit(&self, id: &str) -> rusqlite::Result<Option<UnitRow>> {
        self.conn.query_row("SELECT unit_id,tier,task,repo_url,repo_slug,base_branch,branch,test_cmd,
            usd_cap,wall_clock_secs,phase,cost,last_seq,oracle_frozen,terminal_reason FROM units WHERE unit_id=?1",
            params![id], Self::map_row).map(Some).or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None), other => Err(other) })
    }
    pub fn list_units(&self) -> rusqlite::Result<Vec<UnitRow>> {
        let mut s = self.conn.prepare("SELECT unit_id,tier,task,repo_url,repo_slug,base_branch,branch,
            test_cmd,usd_cap,wall_clock_secs,phase,cost,last_seq,oracle_frozen,terminal_reason FROM units")?;
        let rows = s.query_map([], Self::map_row)?;
        rows.collect()
    }
    pub fn events_since(&self, id: &str, since: u64) -> rusqlite::Result<Vec<String>> {
        let mut s = self.conn.prepare("SELECT json FROM events WHERE unit_id=?1 AND seq>?2 ORDER BY seq")?;
        let rows = s.query_map(params![id, since], |r| r.get::<_, String>(0))?;
        rows.collect()
    }
    /// Rolling-window global spend for the admission cap.
    pub fn spend_since(&self, since_ts: i64) -> rusqlite::Result<f64> {
        self.conn.query_row("SELECT COALESCE(SUM(cost),0) FROM units WHERE created_ts>=?1",
            params![since_ts], |r| r.get(0))
    }

    fn map_row(r: &rusqlite::Row) -> rusqlite::Result<UnitRow> {
        Ok(UnitRow {
            unit_id: r.get(0)?, tier: r.get(1)?, task: r.get(2)?, repo_url: r.get(3)?,
            repo_slug: r.get(4)?, base_branch: r.get(5)?, branch: r.get(6)?, test_cmd: r.get(7)?,
            usd_cap: r.get(8)?, wall_clock_secs: r.get::<_, i64>(9)? as u64, phase: r.get(10)?,
            cost: r.get(11)?, last_seq: r.get::<_, i64>(12)? as u64, oracle_frozen: r.get::<_, i64>(13)? != 0,
            terminal_reason: r.get(14)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn row(id: &str) -> UnitRow {
        UnitRow { unit_id: id.into(), tier: "t1".into(), task: "t".into(), repo_url: "u".into(),
            repo_slug: "s".into(), base_branch: "main".into(), branch: format!("agent/{id}"),
            test_cmd: "node --test".into(), usd_cap: 1.0, wall_clock_secs: 600, phase: "queued".into(),
            cost: 0.0, last_seq: 0, oracle_frozen: false, terminal_reason: None }
    }
    #[test]
    fn upsert_append_list_since_spend() {
        let s = Store::open_memory().unwrap();
        s.upsert_unit(&row("u1"), 1000).unwrap();
        s.append_event("u1", 1, 1000, r#"{"type":"phase_changed"}"#).unwrap();
        s.append_event("u1", 2, 1001, r#"{"type":"metric"}"#).unwrap();
        assert_eq!(s.events_since("u1", 0).unwrap().len(), 2);
        assert_eq!(s.events_since("u1", 1).unwrap().len(), 1);
        let mut r = row("u1"); r.cost = 0.5; s.upsert_unit(&r, 1002).unwrap();
        assert!((s.spend_since(0).unwrap() - 0.5).abs() < 1e-9);
        assert_eq!(s.list_units().unwrap().len(), 1);
        assert_eq!(s.get_unit("u1").unwrap().unwrap().cost, 0.5);
    }
    #[test]
    fn append_is_idempotent_on_seq() {
        let s = Store::open_memory().unwrap();
        s.upsert_unit(&row("u1"), 1).unwrap();
        s.append_event("u1", 1, 1, "{}").unwrap();
        s.append_event("u1", 1, 1, "{}").unwrap(); // OR IGNORE
        assert_eq!(s.events_since("u1", 0).unwrap().len(), 1);
    }
}
```

- [ ] **Step 3: Register + test**

Add `pub mod store;` to `lib.rs`. Run: `cargo test -p fleetd store`. Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml Cargo.lock crates/fleetd/Cargo.toml crates/fleetd/src/store.rs crates/fleetd/src/lib.rs
git commit -m "feat(fleetd): SQLite Store (WAL) with units+events, tested in-memory"
```

### Task 2.2: Writer task + forwarder persistence + endpoints

**Files:** Modify `crates/fleetd/src/server.rs`.

- [ ] **Step 1: Add a writer task and `Store` to `AppState`**

In `server.rs`: `AppState` gains `store: Arc<Store>` (read connection) and `writer: mpsc::UnboundedSender<WriteOp>` where:
```rust
enum WriteOp { Event { unit_id: String, seq: u64, ts: i64, json: String, phase: Option<String>, cost: Option<f64>, last_seq: u64 }, Upsert(UnitRow) }
```
Spawn one writer task owning a write `Store` (own connection to the same `CC_DB` path): loop `recv()`, batch-drain with `try_recv()`, apply each op (append_event + a units update) in one txn, ~50ms cadence. (For SP1 scale a per-op write is acceptable; batching is an optimization — implement per-op first, note batching as a follow-up.)

- [ ] **Step 2: Forwarder writes to the store**

In `create_mission`'s forwarder loop, for each `EventEnvelope`, send a `WriteOp::Event` (with phase/cost extracted from `PhaseChanged`/`Metric`) to `writer`, then push to buffer + broadcast (existing).

- [ ] **Step 3: Add endpoints**

`GET /units` → `store.list_units()` → snapshots. `GET /units/:id/events?since=N` → `store.events_since`. `GET /health` → `{docker: <cached>, anthropic_key: env set, version}`. Extend `Snapshot` with `usd_cap`. WS `/stream` accepts `?since=N` (axum `Query`) and, in `stream_to_socket`, after subscribing, sends `store.events_since(id, since)` then live (dedup by seq as today).

- [ ] **Step 4: Smoke test over HTTP**

Build `serve`, start it, `POST /missions` (demo), `GET /units` shows it, `GET /units/:id/events?since=0` returns events, restart `serve`, `GET /units` still lists the unit (persisted). Expected: persisted across restart.

- [ ] **Step 5: Commit**

```bash
git add crates/fleetd/src/server.rs
git commit -m "feat(fleetd): persist events via writer task; add /units, since-replay, /health"
```

---

## Phase 3 — Volume lifecycle (teardown/discard, reuse probe, Failed-keep)

### Task 3.1: Runner `discard` + `teardown` keeps volume + reuse probe

**Files:** Modify `crates/fleetd/src/runner.rs`, `fake.rs`, `local_docker.rs`.

- [ ] **Step 1: Trait + fake**

`runner.rs`: add `async fn discard(&self, handle: &Handle) -> Result<(), RunnerError>;`. `fake.rs`: implement `discard` (Ok), and (optional) record calls.

- [ ] **Step 2: `local_docker.rs` teardown keeps volume; discard removes it; provision reuse probe**

`teardown`: remove only the container (drop the `docker volume rm`). `discard`: container + `docker volume rm`. In `provision`, after `docker run`, probe:
```rust
let reused = exec_in(&name, "/work", &["test", "-d", "repo/.git"]).await.is_ok();
if reused {
    for (k,v) in [("core.autocrlf","false"),("core.fileMode","false"),("core.symlinks","false"),
                  ("user.email","agent@command-center.local"),("user.name","command-center agent")] {
        exec_in(&name, "/work/repo", &["git","config",k,v]).await?;
    }
    let _ = exec_in(&name, "/work/repo", &["rm","-f",".git/index.lock"]).await;
    exec_in(&name, "/work/repo", &["git","checkout","-B",&spec.branch]).await?;
} else {
    // existing clone + config + checkout -b path
}
```
(`exec_in` returns `Err` on non-zero, so `test -d` failing => fresh path.)

- [ ] **Step 3: Driver wiring — keep volume on pause/Failed, discard on Done/NoChange/abandon**

In `driver.rs` terminal arm, replace the single teardown with:
```rust
                Phase::Done | Phase::NoChange => {
                    if let Some(h) = self.handle.take() { let _ = self.runner.discard(&h).await; }
                    self.permit = None;
                    /* emit Done as today */
                }
                Phase::Failed => {
                    if let Some(h) = self.handle.take() { let _ = self.runner.teardown(&h).await; } // keep volume: forensics
                    self.permit = None;
                    /* emit Done(result="failed") */
                }
```
Abandon path (NeedsHuman/Halted → Failed via Abandon): before that goto, `discard` instead of keep. Implement by tracking the command: in the NeedsHuman/Halted arm, on `Abandon`, `if let Some(h)=self.handle.take(){discard}` then goto(Abandon). (The entry-cleanup already did teardown(keep) on entry; for abandon, additionally discard the volume by name — call `runner.discard` with the same handle id even though the container is gone; `docker volume rm` still runs.)

- [ ] **Step 4: Tests**

`fake.rs`/driver tests: assert `discard` called on Done; `teardown` (keep) on Failed. Run `cargo test --workspace`. Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/fleetd/src/runner.rs crates/fleetd/src/fake.rs crates/fleetd/src/local_docker.rs crates/fleetd/src/driver.rs
git commit -m "feat(fleetd): teardown keeps volume; discard removes it; provision reuses volume"
```

### Task 3.2: In-container per-exec timeout (daemon-independent)

**Files:** Modify `crates/fleetd/src/steps.rs`, `crates/fleetd/src/driver.rs`.

- [ ] **Step 1: Thread `wall_clock_secs` into the claude step builders**

`steps::claude_argv` takes a `timeout_secs: u64` and prepends `["timeout", "<secs>"]` to the
argv so the bound runs *inside* the container (survives daemon death). The driver passes
`self.spec.wall_clock_secs` (skip the prefix when 0). Update `oracle`/`build`/`review`
signatures + their unit tests (assert argv starts with `timeout`).

- [ ] **Step 2: Run + commit**

Run: `cargo test -p fleetd steps`. Expected PASS.
```bash
git add crates/fleetd/src/steps.rs crates/fleetd/src/driver.rs
git commit -m "feat(fleetd): in-container timeout wrapper on agent execs"
```

---

## Phase 4 — Reconciliation + rehydration

### Task 4.1: `reconcile()` pure function

**Files:** Create `crates/fleetd/src/reconcile.rs`; `runner.rs` (+`list_unit_containers`), `fake.rs`, `local_docker.rs`.

- [ ] **Step 1: Trait method + impls**

`runner.rs`: `async fn list_unit_containers(&self) -> Result<Vec<String>, RunnerError>;`. `fake.rs`: return a settable `Vec`. `local_docker.rs`: `docker ps -a --filter label=cc.unit_id --format {{.Label "cc.unit_id"}}` (parse lines).

- [ ] **Step 2: Pure `reconcile` + tests (four quadrants)**

Create `crates/fleetd/src/reconcile.rs`:
```rust
//! Pure startup reconciliation: decide what to do with persisted non-terminal
//! units and currently-running unit containers. Docker-free / unit-tested.
#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    HaltWithContainer(String), // reap container + synthetic Halted event
    HaltNoContainer(String),   // no container; synthetic Halted event only
    ReapStray(String),         // terminal/unknown unit but container running
}
pub fn reconcile(persisted_nonterminal: &[String], running: &[String]) -> Vec<Action> {
    let mut out = vec![];
    for u in persisted_nonterminal {
        if running.contains(u) { out.push(Action::HaltWithContainer(u.clone())); }
        else { out.push(Action::HaltNoContainer(u.clone())); }
    }
    for c in running {
        if !persisted_nonterminal.contains(c) { out.push(Action::ReapStray(c.clone())); }
    }
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn four_quadrants() {
        let actions = reconcile(&["a".into(), "b".into()], &["a".into(), "c".into()]);
        assert!(actions.contains(&Action::HaltWithContainer("a".into())));  // nonterminal + running
        assert!(actions.contains(&Action::HaltNoContainer("b".into())));    // nonterminal, no container
        assert!(actions.contains(&Action::ReapStray("c".into())));          // running, not nonterminal
    }
}
```
Add `pub mod reconcile;` to `lib.rs`. Run `cargo test -p fleetd reconcile`. Expected PASS.

- [ ] **Step 3: Wire reconciliation at server startup**

In the `serve` startup (before `axum::serve`): load `store.list_units()`, compute `persisted_nonterminal` (phase not in done/no_change/failed), `running = runner.list_unit_containers()`, apply `reconcile`: for `HaltWith*`/`HaltNoContainer` → (teardown if container) + write a synthetic `phase_changed{to:halted, reason:"daemon restarted"}` event at `last_seq+1` directly via the writer/store + set unit phase=halted; `ReapStray` → teardown only. Also discard `Failed` volumes older than `CC_KEEP_FAILED_HOURS`.

- [ ] **Step 4: Commit**

```bash
git add crates/fleetd/src/reconcile.rs crates/fleetd/src/lib.rs crates/fleetd/src/runner.rs crates/fleetd/src/fake.rs crates/fleetd/src/local_docker.rs crates/fleetd/src/server.rs
git commit -m "feat(fleetd): startup reconciliation (reap orphans, coherent Halted) + retention"
```

### Task 4.2: Atomic rehydration

**Files:** Modify `crates/fleetd/src/server.rs`.

- [ ] **Step 1: Factor `rehydrate(unit_id) -> Option<()>`**

Extract the per-unit handle construction (channels, forwarder, broadcast, buffer, insert into `units` map) from `create_mission` into a helper. `rehydrate`: lock the `units` map; if present, return (caller falls through); else load `UnitRow` from store, build the handle, insert it while holding the lock, drop the lock, then `tokio::spawn(run(... RunCtx{start_seq:last_seq, start_cost:cost, resume:true, permits}))` with the driver entering at `Halted` (so a `Resume` command drives it).

- [ ] **Step 2: `post_command` rehydrates on absent**

In `post_command`, if the unit isn't in the map, call `rehydrate(id)` (which is the atomic check-and-insert) before sending the command. Concurrent callers → one driver (second sees the inserted handle).

- [ ] **Step 3: Rehydration test**

A server-level test: insert a `Halted` unit directly in the store, no in-memory handle; POST `Resume` twice concurrently; assert only one driver/handle exists (e.g., the second command still 202s and no duplicate). Run `cargo test -p fleetd rehydrate`. Expected PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/fleetd/src/server.rs
git commit -m "feat(fleetd): atomic rehydration of store-only units on Resume"
```

---

## Phase 5 — Concurrency semaphore + admission global cap (server wiring)

### Task 5.1: Real semaphore + admission 429

**Files:** Modify `crates/fleetd/src/server.rs`.

- [ ] **Step 1: AppState owns the semaphore + caps**

`AppState` gains `permits: Arc<Semaphore>` (`CC_MAX_CONCURRENT`, default 3) and reads `CC_GLOBAL_USD_CAP` (default 20.0). Pass `permits.clone()` into every `RunCtx` built in `create_mission`/`rehydrate`.

- [ ] **Step 2: Admission 429**

In `create_mission`, before building the unit: `let spent = store.spend_since(now - 24h)?; if spent >= cap { return (StatusCode::TOO_MANY_REQUESTS, "global daily cost cap reached") }`.

- [ ] **Step 3: Test concurrency + admission**

Server/driver test: with `Semaphore::new(2)` launch 3 demo units; assert the 3rd stays `Queued`/`Blocked` (no Provisioning event) until one finishes. Admission: stub `spend_since` over cap → `POST /missions` returns 429. Run `cargo test -p fleetd`. Expected PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/fleetd/src/server.rs
git commit -m "feat(fleetd): concurrency semaphore + admission rolling-24h global cost cap"
```

---

## Phase 6 — Cockpit reconnect + UX

### Task 6.1: Reconnect on load + cost bar + badges

**Files:** Modify `cockpit/ui/src/lib/api.ts`, `cockpit/ui/src/lib/fleet.ts`, `cockpit/ui/src/App.svelte`.

- [ ] **Step 1: API client: list + health + since**

`api.ts`: add `listUnits(): Promise<Snapshot[]>` (GET /units), `health(): Promise<{docker,anthropic_key,version}>`, and `openStream(unitId, sinceSeq, onEvent)` appending `?since=`.

- [ ] **Step 2: On load, repopulate + reconnect**

`App.svelte` `onMount`: `health()` → badges; `listUnits()` → seed `units`/`order`; for each, `openStream(id, lastSeqWeHave, …)`. The snapshot carries `phase`/`cost`/`usd_cap`; seed `Unit` from it (extend `newUnit`/add `fromSnapshot`).

- [ ] **Step 3: Cost bar + badges**

Add to the tile a `$cost / $cap` bar (`width = cost/usd_cap`); show an `awaiting slot` chip when the latest `Blocked.reason === "awaiting concurrency slot"`; header badges for key/docker from `/health`.

- [ ] **Step 4: Build + typecheck**

Run: `cd cockpit/ui && npm run build && npm run check`. Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add cockpit/ui/src
git commit -m "feat(cockpit): reconnect units on load (list/health/since) + cost bar + badges"
```

---

## Final verification

- [ ] `cargo test --workspace` green; `cargo clippy --workspace --all-targets` clean.
- [ ] `cargo test -p fleetd --test local_docker_it -- --ignored` and the resume real-Docker test green.
- [ ] Manual: start `serve`, launch a demo unit, halt it, restart `serve`, reload cockpit → unit reappears as `HALTED`, press RESUME → it re-provisions (volume reused) and continues without re-running the oracle.
- [ ] `cockpit/ui` builds + typechecks.
