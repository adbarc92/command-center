# Swarm Dispatch Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a daemon-orchestrated "swarm" layer that reads a doc in the target repo, autonomously decomposes it into independent lanes, and fans out one existing-pipeline agent unit per lane, bounded by a hard lane cap and a per-swarm worst-case cost ceiling.

**Architecture:** A pure, sync `swarm.rs` admission core + two seam traits (`Planner`, `DocSource`) with fakes, layered over the unchanged per-unit driver. New `swarms`/`swarm_lanes` tables and a `units.swarm_id` column; `POST /swarms` validates synchronously then fans out in a spawned task. Four engine preconditions (P1 seed `next_id`, P2 single-transaction admission, P3 one terminal-phase list, P4 `committed_spend` on both paths) land first.

**Tech Stack:** Rust, axum, rusqlite (SQLite/WAL), tokio, async-trait. Spec: `docs/superpowers/specs/2026-06-09-swarm-dispatch-design.md`.

---

## Conventions used throughout

- **Run a single test:** `cargo test -p fleetd <test_name>` (or `-p fleet-core`). Run a module: `cargo test -p fleetd swarm::`.
- **Terminal phase strings** are the snake_case serde names: `done`, `no_change`, `failed`.
- **Commit** after each green task. Branch is the worktree branch; never main.
- All `Store` methods are sync; never hold the `Arc<Mutex<Store>>` across `.await`.

---

## Phase 0 — Engine preconditions (P1–P4)

### Task 1: P3 — one authoritative terminal-phase list

**Files:**
- Modify: `crates/fleet-core/src/phase.rs`
- Modify: `crates/fleet-core/src/lib.rs:18`

- [ ] **Step 1: Write the failing test**

Append to `crates/fleet-core/src/phase.rs` inside a `#[cfg(test)] mod tests`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_strs_match_is_terminal() {
        // The string list and the enum predicate must never drift apart.
        for p in [Phase::Done, Phase::NoChange, Phase::Failed,
                  Phase::Queued, Phase::Building, Phase::NeedsHuman, Phase::Halted] {
            let s = serde_json::to_value(p).unwrap().as_str().unwrap().to_string();
            assert_eq!(TERMINAL_PHASE_STRS.contains(&s.as_str()), p.is_terminal(),
                "{s} membership must equal is_terminal()");
        }
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p fleet-core terminal_strs_match_is_terminal`
Expected: FAIL — `cannot find value TERMINAL_PHASE_STRS`. (If `serde_json` is not a dev-dep of fleet-core, add it: `cargo add -p fleet-core --dev serde_json`.)

- [ ] **Step 3: Add the constant**

In `crates/fleet-core/src/phase.rs`, after the `impl Phase` block:

```rust
/// The snake_case phase strings that are terminal. Single source of truth shared
/// by the SQL rollup/admission queries and `reconcile`. MUST stay in sync with
/// `Phase::is_terminal` (the test `terminal_strs_match_is_terminal` enforces this).
pub const TERMINAL_PHASE_STRS: &[&str] = &["done", "no_change", "failed"];
```

- [ ] **Step 4: Export it**

In `crates/fleet-core/src/lib.rs`, change line 18:

```rust
pub use phase::{Phase, TERMINAL_PHASE_STRS};
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p fleet-core terminal_strs_match_is_terminal`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/fleet-core
git commit -m "feat(swarm): P3 authoritative TERMINAL_PHASE_STRS in fleet-core"
```

---

### Task 2: Store migrations — swarms, swarm_lanes, units.swarm_id

**Files:**
- Modify: `crates/fleetd/src/store.rs` (the `init` fn ~`:46-69`)

- [ ] **Step 1: Write the failing test**

Add to the `tests` mod in `store.rs`:

```rust
#[test]
fn swarm_tables_and_unit_swarm_id_exist_after_init() {
    let s = Store::open_memory().unwrap();
    // These pragmas/inserts only succeed if the schema migrated.
    s.conn.execute(
        "INSERT INTO swarms(swarm_id,status,planner_cost,created_ts,updated_ts) VALUES('s1','planning',0.0,1,1)",
        [],
    ).unwrap();
    s.conn.execute(
        "INSERT INTO swarm_lanes(swarm_id,idx,title,task,rationale,decision) VALUES('s1',0,'t','task','r','admit')",
        [],
    ).unwrap();
    // units.swarm_id column present:
    s.conn.execute("UPDATE units SET swarm_id='s1' WHERE unit_id='nope'", []).unwrap();
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p fleetd swarm_tables_and_unit_swarm_id_exist_after_init`
Expected: FAIL — `no such table: swarms`.

- [ ] **Step 3: Extend `init`**

In `store.rs`, inside `fn init`, extend the `execute_batch` to also create the swarm tables, and add the `swarm_id` migration to the idempotent `ALTER` loop. The `units`/`events` `CREATE TABLE` stays; append:

```rust
conn.execute_batch(
    "CREATE TABLE IF NOT EXISTS units( ... unchanged ... );
     CREATE TABLE IF NOT EXISTS events( ... unchanged ... );
     CREATE TABLE IF NOT EXISTS swarms(
       swarm_id TEXT PRIMARY KEY, repo_url TEXT, repo_slug TEXT, base_branch TEXT,
       doc_path TEXT, tier TEXT, mode TEXT, lane_cap INTEGER, usd_budget REAL, per_lane_cap REAL,
       status TEXT, planner_cost REAL, lanes_launched INTEGER, lanes_dropped INTEGER,
       min_review_rounds INTEGER, terminal_reason TEXT, created_ts INTEGER, updated_ts INTEGER);
     CREATE TABLE IF NOT EXISTS swarm_lanes(
       swarm_id TEXT, idx INTEGER, title TEXT, task TEXT, rationale TEXT,
       decision TEXT, unit_id TEXT, PRIMARY KEY(swarm_id, idx));",
)?;
// Idempotent migrations for DBs created before these columns/index existed.
for stmt in [
    "ALTER TABLE units ADD COLUMN mode TEXT NOT NULL DEFAULT 'demo'",
    "ALTER TABLE units ADD COLUMN min_review_rounds INTEGER NOT NULL DEFAULT 2",
    "ALTER TABLE units ADD COLUMN swarm_id TEXT",
    "CREATE INDEX IF NOT EXISTS idx_units_swarm ON units(swarm_id)",
] {
    let _ = conn.execute(stmt, []);
}
```

(Keep the two existing `ALTER` lines; just add the `swarm_id` column and the index to the same loop. The `CREATE INDEX` is also fine to run every boot.)

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p fleetd swarm_tables_and_unit_swarm_id_exist_after_init`
Expected: PASS

- [ ] **Step 5: Run the whole store suite (no regressions)**

Run: `cargo test -p fleetd store::`
Expected: PASS (existing 4 tests + the new one).

- [ ] **Step 6: Commit**

```bash
git add crates/fleetd/src/store.rs
git commit -m "feat(swarm): swarms/swarm_lanes tables + units.swarm_id migration"
```

---

### Task 3: `UnitRow.swarm_id` field threaded through the store

**Files:**
- Modify: `crates/fleetd/src/store.rs` (`UnitRow`, `upsert_unit`, `map_row`, `SELECT_COLS_*`)

- [ ] **Step 1: Write the failing test**

Add to the store `tests` mod:

```rust
#[test]
fn unit_swarm_id_round_trips() {
    let s = Store::open_memory().unwrap();
    let mut r = row("u1");
    r.swarm_id = Some("sw1".into());
    s.upsert_unit(&r, 1000).unwrap();
    assert_eq!(s.get_unit("u1").unwrap().unwrap().swarm_id.as_deref(), Some("sw1"));
}
```

Also update the test helper `fn row(id)` in the same mod to set `swarm_id: None,` (it constructs a full `UnitRow`).

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p fleetd unit_swarm_id_round_trips`
Expected: FAIL — `no field swarm_id on UnitRow`.

- [ ] **Step 3: Add the field + thread it**

In `UnitRow` (after `min_review_rounds`):

```rust
    /// Parent swarm id, set once at create for a lane's unit; `None` for a
    /// standalone mission. Set-once — never updated by the live projection.
    pub swarm_id: Option<String>,
```

In `upsert_unit`, add `swarm_id` as the final inserted column. Change the SQL column list and VALUES to include it (it is **not** in the `ON CONFLICT DO UPDATE` set — set-once):

```rust
"INSERT INTO units(unit_id,tier,task,repo_url,repo_slug,base_branch,branch,test_cmd,
   usd_cap,wall_clock_secs,phase,cost,last_seq,oracle_frozen,created_ts,updated_ts,terminal_reason,
   mode,min_review_rounds,swarm_id)
 VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?15,?16,?17,?18,?19)
 ON CONFLICT(unit_id) DO UPDATE SET
   phase=?11, cost=?12, last_seq=?13, oracle_frozen=?14, updated_ts=?15, terminal_reason=?16"
```

and append `r.swarm_id` to the `params![...]` list (after `r.min_review_rounds`).

In both `SELECT_COLS_ALL` and `SELECT_COLS_WHERE_ID`, append `,swarm_id` after `min_review_rounds`.

In `map_row`, add as the last field (index 17):

```rust
            swarm_id: r.get(17)?,
```

- [ ] **Step 4: Fix the other `UnitRow` literals**

The compiler will flag every `UnitRow { ... }`. Add `swarm_id: None,` to each: `server.rs` `row_from_spec` (it builds from a `UnitSpec` — use `None` for now; Task 13 sets it via a dedicated path), the `building_row` test helper in `server.rs`, and any others. Search: `cargo build -p fleetd 2>&1 | grep "missing field"`.

- [ ] **Step 5: Run tests**

Run: `cargo test -p fleetd unit_swarm_id_round_trips && cargo test -p fleetd store::`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/fleetd/src/store.rs crates/fleetd/src/server.rs
git commit -m "feat(swarm): thread UnitRow.swarm_id (set-once) through the store"
```

---

### Task 4: `max_unit_seq` + P1 seeding of `next_id`

**Files:**
- Modify: `crates/fleetd/src/store.rs` (new method)
- Modify: `crates/fleetd/src/server.rs` (`AppState::new`)

- [ ] **Step 1: Write the failing store test**

```rust
#[test]
fn max_unit_seq_reads_highest_numeric_suffix() {
    let s = Store::open_memory().unwrap();
    assert_eq!(s.max_unit_seq().unwrap(), 0, "empty store → 0");
    s.upsert_unit(&row("u3"), 1).unwrap();
    s.upsert_unit(&row("u10"), 1).unwrap();
    s.upsert_unit(&row("u2"), 1).unwrap();
    assert_eq!(s.max_unit_seq().unwrap(), 10, "parses the numeric suffix, not lexical max");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p fleetd max_unit_seq_reads_highest_numeric_suffix`
Expected: FAIL — `no method max_unit_seq`.

- [ ] **Step 3: Implement `max_unit_seq`**

Ids are `u{n}`. Parse the suffix in Rust (SQLite can't portably cast `substr`):

```rust
/// Highest `u{n}` suffix currently persisted (0 if none). Used to seed the
/// in-memory id allocator on startup so a restart never re-mints a live id.
pub fn max_unit_seq(&self) -> rusqlite::Result<u64> {
    let mut s = self.conn.prepare("SELECT unit_id FROM units")?;
    let ids = s.query_map([], |r| r.get::<_, String>(0))?;
    let mut max = 0u64;
    for id in ids {
        if let Some(n) = id?.strip_prefix('u').and_then(|n| n.parse::<u64>().ok()) {
            max = max.max(n);
        }
    }
    Ok(max)
}
```

- [ ] **Step 4: Seed `next_id` in `AppState::new`**

In `server.rs` `AppState::new`, replace `next_id: Arc::new(AtomicU64::new(1))` with a value seeded from the store (this is the single construction point, before `serve.rs` calls `reconcile_on_startup`):

```rust
let next = store.lock().unwrap().max_unit_seq().unwrap_or(0) + 1;
// ...
Self {
    units: Arc::new(Mutex::new(HashMap::new())),
    next_id: Arc::new(AtomicU64::new(next)),
    // ...rest unchanged...
}
```

- [ ] **Step 5: Write the seeding regression test**

In the `server.rs` tests mod:

```rust
#[tokio::test]
async fn next_id_seeds_above_persisted_units() {
    let store = Arc::new(Mutex::new(Store::open_memory().unwrap()));
    store.lock().unwrap().upsert_unit(&building_row("u5"), 1).unwrap();
    let state = AppState::new(store);
    // Fresh allocation must not collide with u5.
    let n = state.next_id.fetch_add(1, Ordering::Relaxed);
    assert_eq!(n, 6, "next mint is u6, never an existing id");
}
```

- [ ] **Step 6: Run tests**

Run: `cargo test -p fleetd max_unit_seq_reads_highest_numeric_suffix next_id_seeds_above_persisted_units`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add crates/fleetd/src/store.rs crates/fleetd/src/server.rs
git commit -m "feat(swarm): P1 seed next_id from max_unit_seq on startup"
```

---

### Task 5: `committed_spend` (P4) — counts reservations, not just recorded cost

**Files:**
- Modify: `crates/fleetd/src/store.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn committed_spend_counts_reservations_and_overcap_and_planner() {
    let s = Store::open_memory().unwrap();
    // A terminal unit contributes its final cost.
    let mut done = row("done"); done.phase = "done".into(); done.cost = 1.0; done.usd_cap = 5.0;
    s.upsert_unit(&done, 1000).unwrap();
    // A non-terminal unit under cap contributes its usd_cap (reservation).
    let mut run = row("run"); run.phase = "building".into(); run.cost = 0.5; run.usd_cap = 5.0;
    s.upsert_unit(&run, 1000).unwrap();
    // A non-terminal unit that BILLED PAST its cap contributes cost (MAX), not usd_cap.
    let mut over = row("over"); over.phase = "needs_human".into(); over.cost = 9.0; over.usd_cap = 5.0;
    s.upsert_unit(&over, 1000).unwrap();
    // Planner cost of a swarm counts too.
    s.conn.execute(
        "INSERT INTO swarms(swarm_id,status,planner_cost,created_ts,updated_ts) VALUES('sw',?1,?2,1000,1000)",
        rusqlite::params!["failed", 2.0],
    ).unwrap();
    // 1.0 (done) + 5.0 (run reservation) + 9.0 (over, MAX) + 2.0 (planner) = 17.0
    assert!((s.committed_spend(0).unwrap() - 17.0).abs() < 1e-9);
}

#[test]
fn committed_spend_window_excludes_old() {
    let s = Store::open_memory().unwrap();
    let mut old = row("old"); old.phase = "building".into(); old.usd_cap = 5.0;
    s.upsert_unit(&old, 100).unwrap();
    assert_eq!(s.committed_spend(500).unwrap(), 0.0, "created before the window is excluded");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p fleetd committed_spend`
Expected: FAIL — `no method committed_spend`.

- [ ] **Step 3: Implement `committed_spend`**

```rust
/// Worst-case committed spend in the rolling window: terminal units count their
/// final `cost`; non-terminal units count `MAX(usd_cap, cost)` (the driver can
/// bill a step past the cap before parking, so cost may exceed usd_cap); swarms
/// count their `planner_cost`. Partitioned by the authoritative terminal list.
pub fn committed_spend(&self, since_ts: i64) -> rusqlite::Result<f64> {
    let terminal = fleet_core::TERMINAL_PHASE_STRS
        .iter().map(|p| format!("'{p}'")).collect::<Vec<_>>().join(",");
    let sql = format!(
        "SELECT
           COALESCE((SELECT SUM(cost) FROM units
                       WHERE created_ts>=?1 AND phase IN ({terminal})),0)
         + COALESCE((SELECT SUM(MAX(usd_cap,cost)) FROM units
                       WHERE created_ts>=?1 AND phase NOT IN ({terminal})),0)
         + COALESCE((SELECT SUM(planner_cost) FROM swarms WHERE created_ts>=?1),0)"
    );
    self.conn.query_row(&sql, params![since_ts], |r| r.get(0))
}
```

Add `use fleet_core::TERMINAL_PHASE_STRS;` if not already imported (or fully-qualify as above). Note `MAX(a,b)` is SQLite's two-arg scalar max.

- [ ] **Step 4: Run tests**

Run: `cargo test -p fleetd committed_spend`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/fleetd/src/store.rs
git commit -m "feat(swarm): P4 committed_spend (reservations + over-cap MAX + planner)"
```

---

### Task 6: P2 — single-transaction admission in `create_mission`

**Files:**
- Modify: `crates/fleetd/src/server.rs` (`create_mission` admission)

This swaps the cap-blind `spend_since` for `committed_spend` on the standalone path and proves the check+insert is one critical section. (The full `spawn_unit` extraction is Task 13; here we only move the admission check to `committed_spend` and document the single-lock span.)

- [ ] **Step 1: Write the failing test**

In the `server.rs` tests mod (replaces the spirit of `create_mission_refused_over_global_cap`, which used recorded cost):

```rust
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p fleetd create_mission_refused_when_committed_reservations_breach_cap`
Expected: FAIL — current code uses `spend_since` (recorded cost 0.1 < 20) so it returns Ok.

- [ ] **Step 3: Swap to `committed_spend`**

In `create_mission`, replace the admission block:

```rust
    let since = now_ms() - 24 * 3600 * 1000;
    let spent = st.store.lock().unwrap().committed_spend(since).unwrap_or(0.0);
    if spent >= st.global_cap {
        return Err((StatusCode::TOO_MANY_REQUESTS, "global daily cost cap reached".into()));
    }
```

Leave the existing `spend_since` method in the store (still used by its own test); the admission path now uses `committed_spend`.

- [ ] **Step 4: Run tests (and confirm the old over-cap test still holds)**

Run: `cargo test -p fleetd create_mission_refused`
Expected: PASS for the new test. If `create_mission_refused_over_global_cap` still exists and passes (a terminal unit with cost 999 still breaches), keep it; committed_spend counts terminal `cost` too.

- [ ] **Step 5: Commit**

```bash
git add crates/fleetd/src/server.rs
git commit -m "feat(swarm): P2/P4 admission uses committed_spend on the mission path"
```

---

## Phase 1 — Pure core (`swarm.rs`)

### Task 7: `slug` — sanitized, length-bounded branch suffix

**Files:**
- Create: `crates/fleetd/src/swarm.rs`
- Modify: `crates/fleetd/src/lib.rs` (add `pub mod swarm;`)

- [ ] **Step 1: Create the module + failing test**

Create `crates/fleetd/src/swarm.rs`:

```rust
//! The pure, sync swarm admission core: lane admission against the dual
//! guardrail, and branch-slug sanitization. No async, no I/O — exhaustively
//! unit-testable, mirroring `fleet_core::gate`.

/// Sanitize a planner-chosen lane title into a git-ref-safe, length-bounded
/// slug. Uniqueness comes from the `{idx}-` prefix the caller adds, so this is
/// purely cosmetic and never the uniqueness key.
pub fn slug(title: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for c in title.chars() {
        let lc = c.to_ascii_lowercase();
        if lc.is_ascii_alphanumeric() {
            out.push(lc);
            prev_dash = false;
        } else if !prev_dash && !out.is_empty() {
            out.push('-');
            prev_dash = true;
        }
    }
    let trimmed: String = out.trim_matches('-').chars().take(32).collect();
    let trimmed = trimmed.trim_matches('-').to_string();
    if trimmed.is_empty() { "lane".into() } else { trimmed }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_sanitizes_charset_length_and_empty() {
        assert_eq!(slug("Add Auth!!"), "add-auth");
        assert_eq!(slug("  spaced  out  "), "spaced-out");
        assert_eq!(slug("🚀🚀🚀"), "lane");          // non-ascii → fallback
        assert_eq!(slug(""), "lane");
        assert_eq!(slug(&"x".repeat(100)).len(), 32); // truncated
    }
}
```

- [ ] **Step 2: Register the module**

In `crates/fleetd/src/lib.rs`, add (alphabetical with the others): `pub mod swarm;`

- [ ] **Step 3: Run test to verify it passes**

Run: `cargo test -p fleetd swarm::tests::slug`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/fleetd/src/swarm.rs crates/fleetd/src/lib.rs
git commit -m "feat(swarm): pure slug() for branch suffixes"
```

---

### Task 8: `Lane`, `AdmissionConfig`, `LaneDecision`, `admit_lanes`

**Files:**
- Modify: `crates/fleetd/src/swarm.rs`

- [ ] **Step 1: Write the failing tests**

Append to `swarm.rs` (above the `tests` mod's closing brace, add to `mod tests`):

```rust
    fn lanes(n: usize) -> Vec<Lane> {
        (0..n).map(|i| Lane { title: format!("l{i}"), task: "t".into(), rationale: "r".into() }).collect()
    }
    fn cfg(lane_cap: usize, usd_budget: f64, per_lane_cap: f64, planner_cost: f64) -> AdmissionConfig {
        AdmissionConfig { lane_cap, usd_budget, per_lane_cap, planner_cost }
    }
    fn admits(d: &[(usize, LaneDecision)]) -> usize {
        d.iter().filter(|(_, x)| matches!(x, LaneDecision::Admit)).count()
    }

    #[test]
    fn lane_cap_binds_first() {
        // budget allows 10, cap allows 3.
        let d = admit_lanes(&lanes(5), &cfg(3, 100.0, 5.0, 0.0));
        assert_eq!(admits(&d), 3);
        assert!(matches!(d[3].1, LaneDecision::DropOverLaneCap));
    }

    #[test]
    fn budget_binds_first() {
        // cap allows 8, budget allows floor((15-0)/5)=3.
        let d = admit_lanes(&lanes(8), &cfg(8, 15.0, 5.0, 0.0));
        assert_eq!(admits(&d), 3);
        assert!(matches!(d[3].1, LaneDecision::DropOverBudget));
    }

    #[test]
    fn planner_cost_reserved_first() {
        // budget 15, planner spent 6 → floor((15-6)/5)=1.
        let d = admit_lanes(&lanes(4), &cfg(8, 15.0, 5.0, 6.0));
        assert_eq!(admits(&d), 1);
    }

    #[test]
    fn planner_over_budget_admits_zero() {
        let d = admit_lanes(&lanes(4), &cfg(8, 4.0, 5.0, 5.0));
        assert_eq!(admits(&d), 0);
        assert!(d.iter().all(|(_, x)| matches!(x, LaneDecision::DropOverBudget)));
    }

    #[test]
    fn empty_input_is_empty_output() {
        assert!(admit_lanes(&[], &cfg(8, 15.0, 5.0, 0.0)).is_empty());
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p fleetd swarm::tests`
Expected: FAIL — `cannot find type Lane` / `admit_lanes`.

- [ ] **Step 3: Implement the types + `admit_lanes`**

Add near the top of `swarm.rs` (before `slug`):

```rust
/// One independent sub-task the planner carved out of the doc.
#[derive(Clone, Debug, PartialEq)]
pub struct Lane {
    pub title: String,
    pub task: String,
    pub rationale: String,
}

/// The dual guardrail: a hard lane count and a worst-case dollar envelope.
#[derive(Clone, Copy, Debug)]
pub struct AdmissionConfig {
    pub lane_cap: usize,
    pub usd_budget: f64,
    pub per_lane_cap: f64,
    pub planner_cost: f64,
}

/// Per-lane admission verdict. `DropOverGlobalCap` is set later by the fan-out
/// loop (a runtime re-check), never by `admit_lanes`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LaneDecision { Admit, DropOverLaneCap, DropOverBudget }

/// Walk lanes in order; admit while BOTH the count cap and the
/// (usd_budget − planner_cost) envelope hold. Conservative: each admitted lane
/// is assumed to spend its full `per_lane_cap`.
pub fn admit_lanes(lanes: &[Lane], cfg: &AdmissionConfig) -> Vec<(usize, LaneDecision)> {
    let envelope = (cfg.usd_budget - cfg.planner_cost).max(0.0);
    let mut admitted = 0usize;
    lanes.iter().enumerate().map(|(i, _)| {
        let decision = if admitted >= cfg.lane_cap {
            LaneDecision::DropOverLaneCap
        } else if (admitted as f64 + 1.0) * cfg.per_lane_cap > envelope {
            LaneDecision::DropOverBudget
        } else {
            admitted += 1;
            LaneDecision::Admit
        };
        (i, decision)
    }).collect()
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p fleetd swarm::tests`
Expected: PASS (all admit_lanes + slug tests).

- [ ] **Step 5: Commit**

```bash
git add crates/fleetd/src/swarm.rs
git commit -m "feat(swarm): pure admit_lanes dual-guardrail core"
```

---

## Phase 2 — Seam traits + fakes

### Task 9: `Planner` seam + `FakePlanner`

**Files:**
- Create: `crates/fleetd/src/planner.rs`
- Modify: `crates/fleetd/src/lib.rs` (`pub mod planner;`)

- [ ] **Step 1: Create the module + failing test**

Create `crates/fleetd/src/planner.rs`:

```rust
//! The decomposition seam. `FakePlanner` (tests/demo) returns scripted lanes;
//! `ClaudePlanner` (real, Task 16) makes a read-only Claude call. The planner
//! writes no code and opens no PR.

use crate::swarm::Lane;
use async_trait::async_trait;

#[derive(Debug, thiserror::Error)]
pub enum PlanError {
    #[error("planner failure: {0}")]
    Failed(String),
}

/// A decomposition result plus what it cost to produce.
#[derive(Clone, Debug)]
pub struct PlanOutcome {
    pub lanes: Vec<Lane>,
    pub cost_usd: f64,
}

#[async_trait]
pub trait Planner: Send + Sync {
    /// Decompose `doc` into at most `lane_cap` independent lanes.
    async fn plan(&self, doc: &str, lane_cap: usize) -> Result<PlanOutcome, PlanError>;
}

/// Scripted planner for tests/demo: returns a fixed outcome (clamped to lane_cap),
/// or a fixed error.
pub struct FakePlanner {
    outcome: Result<PlanOutcome, String>,
}
impl FakePlanner {
    pub fn ok(lanes: Vec<Lane>, cost_usd: f64) -> Self {
        Self { outcome: Ok(PlanOutcome { lanes, cost_usd }) }
    }
    pub fn err(msg: &str) -> Self { Self { outcome: Err(msg.into()) } }
}
#[async_trait]
impl Planner for FakePlanner {
    async fn plan(&self, _doc: &str, lane_cap: usize) -> Result<PlanOutcome, PlanError> {
        match &self.outcome {
            Ok(o) => Ok(PlanOutcome {
                lanes: o.lanes.iter().take(lane_cap).cloned().collect(),
                cost_usd: o.cost_usd,
            }),
            Err(m) => Err(PlanError::Failed(m.clone())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn fake_planner_clamps_to_lane_cap_and_can_error() {
        let lanes = vec![
            Lane { title: "a".into(), task: "ta".into(), rationale: "r".into() },
            Lane { title: "b".into(), task: "tb".into(), rationale: "r".into() },
        ];
        let p = FakePlanner::ok(lanes, 0.5);
        let out = p.plan("doc", 1).await.unwrap();
        assert_eq!(out.lanes.len(), 1);
        assert_eq!(out.cost_usd, 0.5);

        let e = FakePlanner::err("boom").plan("doc", 5).await;
        assert!(matches!(e, Err(PlanError::Failed(_))));
    }
}
```

- [ ] **Step 2: Register the module**

In `lib.rs`: `pub mod planner;`

- [ ] **Step 3: Run test to verify it passes**

Run: `cargo test -p fleetd planner::`
Expected: PASS. (If `thiserror`/`async-trait`/`tokio` macros are missing they're already deps — `forge.rs` uses `async_trait` and `thiserror`.)

- [ ] **Step 4: Commit**

```bash
git add crates/fleetd/src/planner.rs crates/fleetd/src/lib.rs
git commit -m "feat(swarm): Planner seam + FakePlanner"
```

---

### Task 10: `DocSource` seam + `FakeDocSource`

**Files:**
- Create: `crates/fleetd/src/docsource.rs`
- Modify: `crates/fleetd/src/lib.rs` (`pub mod docsource;`)

- [ ] **Step 1: Create the module + failing test**

Create `crates/fleetd/src/docsource.rs`:

```rust
//! Fetch the doc's *content* from the target repo so the planner core never
//! touches git/fs. `FakeDocSource` (tests) returns canned content or a
//! not-found error; `GitDocSource` (real, Task 17) shallow-clones and reads.

use async_trait::async_trait;

#[derive(Debug, thiserror::Error)]
pub enum DocError {
    #[error("doc not found: {0}")]
    NotFound(String),
    #[error("doc source failure: {0}")]
    Failed(String),
}

#[async_trait]
pub trait DocSource: Send + Sync {
    async fn read(&self, repo_url: &str, base_branch: &str, doc_path: &str)
        -> Result<String, DocError>;
}

/// Canned content keyed only by `doc_path`; a path of "missing.md" → NotFound.
pub struct FakeDocSource {
    pub content: String,
}
impl FakeDocSource {
    pub fn new(content: &str) -> Self { Self { content: content.into() } }
}
#[async_trait]
impl DocSource for FakeDocSource {
    async fn read(&self, _repo: &str, _base: &str, doc_path: &str) -> Result<String, DocError> {
        if doc_path == "missing.md" {
            return Err(DocError::NotFound(doc_path.into()));
        }
        Ok(self.content.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn fake_doc_source_returns_content_or_not_found() {
        let d = FakeDocSource::new("# spec\n- a\n- b\n");
        assert!(d.read("u", "main", "spec.md").await.unwrap().contains("spec"));
        assert!(matches!(d.read("u", "main", "missing.md").await, Err(DocError::NotFound(_))));
    }
}
```

- [ ] **Step 2: Register the module**

In `lib.rs`: `pub mod docsource;`

- [ ] **Step 3: Run test to verify it passes**

Run: `cargo test -p fleetd docsource::`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/fleetd/src/docsource.rs crates/fleetd/src/lib.rs
git commit -m "feat(swarm): DocSource seam + FakeDocSource"
```

---

## Phase 3 — Swarm persistence

### Task 11: `SwarmRow` + swarm CRUD + `swarm_rollup`

**Files:**
- Modify: `crates/fleetd/src/store.rs`

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn swarm_row_crud_and_list() {
    let s = Store::open_memory().unwrap();
    let sw = swarm_row("sw1", "planning");
    s.upsert_swarm(&sw, 1000).unwrap();
    assert_eq!(s.get_swarm("sw1").unwrap().unwrap().status, "planning");
    s.update_swarm("sw1", "running", 0.4, 3, 0, None, 1001).unwrap();
    let got = s.get_swarm("sw1").unwrap().unwrap();
    assert_eq!(got.status, "running");
    assert_eq!(got.planner_cost, 0.4);
    assert_eq!(got.lanes_launched, 3);
    assert_eq!(s.list_swarms().unwrap().len(), 1);
}

#[test]
fn swarm_rollup_counts_terminal_and_awaiting_human() {
    let s = Store::open_memory().unwrap();
    for (id, phase) in [("u1","done"), ("u2","failed"), ("u3","building"), ("u4","needs_human")] {
        let mut r = row(id); r.phase = phase.into(); r.swarm_id = Some("sw1".into());
        s.upsert_unit(&r, 1000).unwrap();
    }
    let (total, terminal, awaiting) = s.swarm_rollup("sw1").unwrap();
    assert_eq!(total, 4);
    assert_eq!(terminal, 2);          // done + failed
    assert_eq!(awaiting, 1);          // needs_human (halted would also count)
}
```

Add a `swarm_row` test helper to the tests mod:

```rust
fn swarm_row(id: &str, status: &str) -> SwarmRow {
    SwarmRow {
        swarm_id: id.into(), repo_url: "u".into(), repo_slug: "s".into(), base_branch: "main".into(),
        doc_path: "spec.md".into(), tier: "t1".into(), mode: "demo".into(),
        lane_cap: 8, usd_budget: 15.0, per_lane_cap: 5.0, status: status.into(),
        planner_cost: 0.0, lanes_launched: 0, lanes_dropped: 0, min_review_rounds: 2,
        terminal_reason: None,
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p fleetd swarm_row_crud_and_list swarm_rollup_counts`
Expected: FAIL — `cannot find type SwarmRow` / no methods.

- [ ] **Step 3: Implement `SwarmRow` + methods**

Add to `store.rs`:

```rust
/// Persisted swarm config + mutable projection. `created_ts` set once at insert.
#[derive(Debug, Clone)]
pub struct SwarmRow {
    pub swarm_id: String,
    pub repo_url: String,
    pub repo_slug: String,
    pub base_branch: String,
    pub doc_path: String,
    pub tier: String,
    pub mode: String,
    pub lane_cap: u32,
    pub usd_budget: f64,
    pub per_lane_cap: f64,
    pub status: String,
    pub planner_cost: f64,
    pub lanes_launched: u32,
    pub lanes_dropped: u32,
    pub min_review_rounds: u32,
    pub terminal_reason: Option<String>,
}

impl Store {
    pub fn upsert_swarm(&self, r: &SwarmRow, now: i64) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO swarms(swarm_id,repo_url,repo_slug,base_branch,doc_path,tier,mode,
               lane_cap,usd_budget,per_lane_cap,status,planner_cost,lanes_launched,lanes_dropped,
               min_review_rounds,terminal_reason,created_ts,updated_ts)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?17)
             ON CONFLICT(swarm_id) DO UPDATE SET
               status=?11, planner_cost=?12, lanes_launched=?13, lanes_dropped=?14,
               terminal_reason=?16, updated_ts=?17",
            params![r.swarm_id, r.repo_url, r.repo_slug, r.base_branch, r.doc_path, r.tier, r.mode,
                r.lane_cap, r.usd_budget, r.per_lane_cap, r.status, r.planner_cost,
                r.lanes_launched, r.lanes_dropped, r.min_review_rounds, r.terminal_reason, now],
        )?;
        Ok(())
    }

    pub fn update_swarm(&self, id: &str, status: &str, planner_cost: f64, lanes_launched: u32,
        lanes_dropped: u32, terminal_reason: Option<&str>, now: i64) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE swarms SET status=?2, planner_cost=?3, lanes_launched=?4, lanes_dropped=?5,
               terminal_reason=?6, updated_ts=?7 WHERE swarm_id=?1",
            params![id, status, planner_cost, lanes_launched, lanes_dropped, terminal_reason, now],
        )?;
        Ok(())
    }

    pub fn get_swarm(&self, id: &str) -> rusqlite::Result<Option<SwarmRow>> {
        self.conn.query_row(SELECT_SWARM_WHERE_ID, params![id], Self::map_swarm)
            .map(Some).or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None), other => Err(other),
            })
    }

    pub fn list_swarms(&self) -> rusqlite::Result<Vec<SwarmRow>> {
        let mut s = self.conn.prepare(SELECT_SWARM_ALL)?;
        s.query_map([], Self::map_swarm)?.collect()
    }

    /// (total, terminal, awaiting_human) over a swarm's child units. One GROUP-free
    /// aggregate; `awaiting_human` = non-terminal parked phases.
    pub fn swarm_rollup(&self, swarm_id: &str) -> rusqlite::Result<(u64, u64, u64)> {
        let terminal = fleet_core::TERMINAL_PHASE_STRS
            .iter().map(|p| format!("'{p}'")).collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT
               COUNT(*),
               COALESCE(SUM(CASE WHEN phase IN ({terminal}) THEN 1 ELSE 0 END),0),
               COALESCE(SUM(CASE WHEN phase IN ('needs_human','halted') THEN 1 ELSE 0 END),0)
             FROM units WHERE swarm_id=?1"
        );
        self.conn.query_row(&sql, params![swarm_id], |r| {
            Ok((r.get::<_, i64>(0)? as u64, r.get::<_, i64>(1)? as u64, r.get::<_, i64>(2)? as u64))
        })
    }

    fn map_swarm(r: &rusqlite::Row) -> rusqlite::Result<SwarmRow> {
        Ok(SwarmRow {
            swarm_id: r.get(0)?, repo_url: r.get(1)?, repo_slug: r.get(2)?, base_branch: r.get(3)?,
            doc_path: r.get(4)?, tier: r.get(5)?, mode: r.get(6)?,
            lane_cap: r.get::<_, i64>(7)? as u32, usd_budget: r.get(8)?, per_lane_cap: r.get(9)?,
            status: r.get(10)?, planner_cost: r.get(11)?,
            lanes_launched: r.get::<_, i64>(12)? as u32, lanes_dropped: r.get::<_, i64>(13)? as u32,
            min_review_rounds: r.get::<_, i64>(14)? as u32, terminal_reason: r.get(15)?,
        })
    }
}

const SELECT_SWARM_COLS: &str = "swarm_id,repo_url,repo_slug,base_branch,doc_path,tier,mode,lane_cap,usd_budget,per_lane_cap,status,planner_cost,lanes_launched,lanes_dropped,min_review_rounds,terminal_reason";
```

Add the two select constants at the bottom near the others:

```rust
const SELECT_SWARM_ALL: &str = "SELECT swarm_id,repo_url,repo_slug,base_branch,doc_path,tier,mode,lane_cap,usd_budget,per_lane_cap,status,planner_cost,lanes_launched,lanes_dropped,min_review_rounds,terminal_reason FROM swarms";
const SELECT_SWARM_WHERE_ID: &str = "SELECT swarm_id,repo_url,repo_slug,base_branch,doc_path,tier,mode,lane_cap,usd_budget,per_lane_cap,status,planner_cost,lanes_launched,lanes_dropped,min_review_rounds,terminal_reason FROM swarms WHERE swarm_id=?1";
```

(`SELECT_SWARM_COLS` is illustrative; the two full constants are what the code uses. Drop `SELECT_SWARM_COLS` if unused to avoid a dead-code warning.)

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p fleetd swarm_row_crud_and_list swarm_rollup_counts`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/fleetd/src/store.rs
git commit -m "feat(swarm): SwarmRow CRUD + swarm_rollup aggregate"
```

---

### Task 12: lane persistence + `commit_lane_unit` (atomic row + back-link)

**Files:**
- Modify: `crates/fleetd/src/store.rs`

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn lane_crud_and_commit_is_atomic() {
    let s = Store::open_memory().unwrap();
    s.upsert_swarm(&swarm_row("sw1", "fanning_out"), 1000).unwrap();
    // Persist a lane with no unit yet.
    s.upsert_lane("sw1", 0, "Add auth", "do auth", "indep", "admit", None).unwrap();
    let lanes = s.lanes_for_swarm("sw1").unwrap();
    assert_eq!(lanes.len(), 1);
    assert_eq!(lanes[0].decision, "admit");
    assert!(lanes[0].unit_id.is_none());

    // commit_lane_unit inserts the unit row AND sets the back-link in one txn.
    let mut u = row("u1"); u.swarm_id = Some("sw1".into());
    s.commit_lane_unit("sw1", 0, &u, 1001).unwrap();
    assert!(s.get_unit("u1").unwrap().is_some(), "unit row inserted");
    assert_eq!(s.lanes_for_swarm("sw1").unwrap()[0].unit_id.as_deref(), Some("u1"), "back-link set");
}
```

Add a `LaneRow` reference in the test by checking fields; define `LaneRow` in the impl below.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p fleetd lane_crud_and_commit_is_atomic`
Expected: FAIL — no `upsert_lane`/`lanes_for_swarm`/`commit_lane_unit`/`LaneRow`.

- [ ] **Step 3: Implement lane persistence**

```rust
#[derive(Debug, Clone)]
pub struct LaneRow {
    pub swarm_id: String,
    pub idx: u32,
    pub title: String,
    pub task: String,
    pub rationale: String,
    pub decision: String,
    pub unit_id: Option<String>,
}

impl Store {
    #[allow(clippy::too_many_arguments)]
    pub fn upsert_lane(&self, swarm_id: &str, idx: u32, title: &str, task: &str,
        rationale: &str, decision: &str, unit_id: Option<&str>) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO swarm_lanes(swarm_id,idx,title,task,rationale,decision,unit_id)
             VALUES(?1,?2,?3,?4,?5,?6,?7)
             ON CONFLICT(swarm_id,idx) DO UPDATE SET decision=?6, unit_id=?7",
            params![swarm_id, idx, title, task, rationale, decision, unit_id],
        )?;
        Ok(())
    }

    pub fn lanes_for_swarm(&self, swarm_id: &str) -> rusqlite::Result<Vec<LaneRow>> {
        let mut s = self.conn.prepare(
            "SELECT swarm_id,idx,title,task,rationale,decision,unit_id
             FROM swarm_lanes WHERE swarm_id=?1 ORDER BY idx")?;
        s.query_map(params![swarm_id], |r| Ok(LaneRow {
            swarm_id: r.get(0)?, idx: r.get::<_, i64>(1)? as u32, title: r.get(2)?,
            task: r.get(3)?, rationale: r.get(4)?, decision: r.get(5)?, unit_id: r.get(6)?,
        }))?.collect()
    }

    /// Insert the lane's unit row AND set `swarm_lanes.unit_id` in ONE transaction,
    /// so a crash never leaves a dangling back-link or an orphan row (spec R2 #5).
    /// `&mut self`-free: uses an explicit transaction on the shared connection.
    pub fn commit_lane_unit(&self, swarm_id: &str, idx: u32, u: &UnitRow, now: i64)
        -> rusqlite::Result<()> {
        self.conn.execute_batch("BEGIN IMMEDIATE")?;
        let r = (|| {
            self.upsert_unit(u, now)?;
            self.conn.execute(
                "UPDATE swarm_lanes SET unit_id=?3 WHERE swarm_id=?1 AND idx=?2",
                params![swarm_id, idx, u.unit_id],
            )?;
            Ok(())
        })();
        match r {
            Ok(()) => { self.conn.execute_batch("COMMIT")?; Ok(()) }
            Err(e) => { let _ = self.conn.execute_batch("ROLLBACK"); Err(e) }
        }
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p fleetd lane_crud_and_commit_is_atomic`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/fleetd/src/store.rs
git commit -m "feat(swarm): swarm_lanes CRUD + atomic commit_lane_unit"
```

---

## Phase 4 — Server: spawn_unit, endpoints, fan-out, reconcile

### Task 13: extract `spawn_unit` (Result, threads swarm fields)

**Files:**
- Modify: `crates/fleetd/src/server.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn spawn_unit_creates_demo_unit_with_swarm_id_and_cap() {
    let state = AppState::default();
    let spec = UnitSpec {
        unit_id: "u1".into(), tier: Tier::T1, task: "t".into(), usd_cap: 3.0,
        wall_clock_secs: 0, gate: GateConfig { min_review_rounds: 1 },
        repo_url: "https://github.com/x/y".into(), repo_slug: "x/y".into(),
        base_branch: "main".into(), branch: "agent/sw1/0-l".into(),
        test_cmd: "node --test".into(), oracle_frozen: false,
    };
    let id = spawn_unit(&state, spec, "demo", Some("sw1")).expect("spawn ok");
    assert_eq!(id, "u1");
    let row = state.store.lock().unwrap().get_unit("u1").unwrap().unwrap();
    assert_eq!(row.swarm_id.as_deref(), Some("sw1"));
    assert_eq!(row.usd_cap, 3.0);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p fleetd spawn_unit_creates_demo_unit_with_swarm_id_and_cap`
Expected: FAIL — `cannot find function spawn_unit`.

- [ ] **Step 3: Implement `spawn_unit` + `SpawnError`, refactor `create_mission` to use it**

Add to `server.rs`:

```rust
#[derive(Debug)]
pub enum SpawnError {
    /// A driver already exists for this id (register lost the race / id reuse).
    AlreadyRegistered,
}

/// Persist a fresh unit's row (carrying `swarm_id`) and spawn its driver. Used by
/// both the standalone mission path and swarm fan-out. Returns `Err` instead of
/// panicking (swarm fan-out must continue past a single bad lane). NOTE: callers
/// that need committed-spend admission must do that check under the store lock
/// BEFORE calling this (see create_mission / fan-out).
pub fn spawn_unit(st: &AppState, spec: UnitSpec, mode: &str, swarm_id: Option<&str>)
    -> Result<String, SpawnError> {
    let unit_id = spec.unit_id.clone();
    let mut row = row_from_spec(&spec, mode);
    row.swarm_id = swarm_id.map(|s| s.to_string());
    st.store.lock().unwrap().upsert_unit(&row, now_ms()).ok();

    let (cmd_rx, evt_tx) = match register_unit_if_absent(st, &unit_id) {
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
            let runner = LocalDockerRunner::new("cc-agent:dev");
            tokio::spawn(run(runner, forge, spec, fresh_ctx(st), cmd_rx, evt_tx));
        }
    }
    Ok(unit_id)
}
```

Then in `create_mission`, after building `spec` and validating the mode, replace the inline persist+register+spawn block with:

```rust
    st.store.lock().unwrap().upsert_unit(&row_from_spec(&spec, &runner_mode), now_ms()).ok();
    // ^ remove this line; spawn_unit persists the row itself.
    match spawn_unit(&st, spec, &runner_mode, None) {
        Ok(_) => {}
        Err(_) => return Err((StatusCode::CONFLICT, "unit already exists".into())),
    }
```

(Keep `create_mission`'s synchronous mode validation and the `committed_spend` admission from Task 6 ahead of this. The standalone path passes `swarm_id = None`.)

- [ ] **Step 4: Run tests (no regression on mission flow)**

Run: `cargo test -p fleetd spawn_unit_creates_demo_unit_with_swarm_id_and_cap && cargo test -p fleetd server::`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/fleetd/src/server.rs
git commit -m "feat(swarm): extract spawn_unit (Result, threads swarm_id) and reuse in create_mission"
```

---

### Task 14: the fan-out core — `run_swarm` (sync-validated input → spawned task)

**Files:**
- Modify: `crates/fleetd/src/server.rs`

This is the orchestration: given an already-validated swarm spec + the chosen seams, plan → admit → fan out. It is an `async fn` taking trait objects so tests drive it with fakes directly (no HTTP).

- [ ] **Step 1: Write the failing end-to-end test (fakes)**

```rust
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn run_swarm_fans_out_admitted_lanes_to_done() {
    use crate::docsource::FakeDocSource;
    use crate::planner::FakePlanner;
    use crate::swarm::Lane;

    let state = AppState::default();
    // Seed the swarm row as 'planning' (the handler does this synchronously).
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

    // Two demo units created under the swarm, with per_lane_cap + swarm_id + unique branches.
    let units = state.store.lock().unwrap().list_units().unwrap();
    let mine: Vec<_> = units.iter().filter(|u| u.swarm_id.as_deref() == Some("sw1")).collect();
    assert_eq!(mine.len(), 2);
    assert!(mine.iter().all(|u| (u.usd_cap - 5.0).abs() < 1e-9));
    let branches: std::collections::HashSet<_> = mine.iter().map(|u| u.branch.clone()).collect();
    assert_eq!(branches.len(), 2, "unique branches");
    assert!(branches.iter().all(|b| b.starts_with("agent/sw1/")));

    // Demo units drive themselves to done; the swarm rolls up to running→done.
    // Poll the rollup briefly (demo FakeRunner is fast).
    let mut done = false;
    for _ in 0..200 {
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
    let mut sw = crate::store::SwarmRow {
        swarm_id: "sw2".into(), repo_url: "u".into(), repo_slug: "s".into(), base_branch: "main".into(),
        doc_path: "spec.md".into(), tier: "t1".into(), mode: "demo".into(),
        lane_cap: 8, usd_budget: 1.0, per_lane_cap: 5.0, status: "planning".into(), // budget < 1 lane
        planner_cost: 0.0, lanes_launched: 0, lanes_dropped: 0, min_review_rounds: 1,
        terminal_reason: None,
    };
    state.store.lock().unwrap().upsert_swarm(&sw, now_ms()).unwrap();
    let planner = FakePlanner::ok(vec![Lane { title: "A".into(), task: "a".into(), rationale: "r".into() }], 0.0);
    run_swarm(state.clone(), "sw2".into(), planner, FakeDocSource::new("# spec")).await;
    let got = state.store.lock().unwrap().get_swarm("sw2").unwrap().unwrap();
    assert_eq!(got.status, "empty", "lanes produced but none admitted ⇒ empty, never done");
    let _ = &mut sw;
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
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p fleetd run_swarm`
Expected: FAIL — `cannot find function run_swarm`.

- [ ] **Step 3: Implement `run_swarm`**

```rust
use crate::docsource::DocSource;
use crate::planner::Planner;
use crate::swarm::{admit_lanes, slug, AdmissionConfig, LaneDecision};

/// Plan → admit → fan out, for an already-persisted `planning` swarm row.
/// Generic over the seams so tests inject fakes. Each store mutation is a short
/// sync critical section; the per-lane committed-spend re-check + commit_lane_unit
/// is the admission critical section (P2).
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
    // Record planner cost immediately (counts toward the global cap from now on).
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
        let dstr = match d { LaneDecision::Admit => "admit",
            LaneDecision::DropOverLaneCap => "drop_lane_cap", LaneDecision::DropOverBudget => "drop_budget" };
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
    for idx in admitted_idxs {
        let lane = &outcome.lanes[idx];
        let unit_id = {
            // Admission critical section: re-check committed spend, mint id, commit row+link.
            let s = st.store.lock().unwrap();
            if s.committed_spend(since).unwrap_or(0.0) >= st.global_cap {
                s.upsert_lane(&swarm_id, idx as u32, &lane.title, &lane.task, &lane.rationale,
                    "drop_global_cap", None).ok();
                dropped += 1;
                drop(s);
                break;
            }
            let n = st.next_id.fetch_add(1, Ordering::Relaxed);
            let unit_id = format!("u{n}");
            let spec = lane_spec(&sw, &unit_id, idx, lane);
            let mut row = row_from_spec(&spec, &sw.mode);
            row.swarm_id = Some(swarm_id.clone());
            s.commit_lane_unit(&swarm_id, idx as u32, &row, now_ms()).ok();
            unit_id
        };
        // Spawn the driver outside the lock; the row already exists.
        let spec = lane_spec(&sw, &unit_id, idx, lane);
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
```

Because `spawn_unit` (Task 13) both persists the row AND spawns, but fan-out already persisted the row via `commit_lane_unit`, add a thin `spawn_driver_for` that only registers + spawns (no second upsert), and have `spawn_unit` call it after its own upsert:

```rust
/// Register the per-unit handle and spawn its driver. The row must already be
/// persisted by the caller (create_mission via spawn_unit, or fan-out via
/// commit_lane_unit). Returns Err if a driver already exists (no double-spawn).
fn spawn_driver_for(st: &AppState, spec: UnitSpec, mode: &str, unit_id: &str)
    -> Result<(), SpawnError> {
    let (cmd_rx, evt_tx) = match register_unit_if_absent(st, unit_id) {
        Some(ch) => ch, None => return Err(SpawnError::AlreadyRegistered),
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
```

Refactor `spawn_unit` (Task 13) to: upsert the row, then `spawn_driver_for(...)`. This keeps one driver-spawn implementation.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p fleetd run_swarm`
Expected: PASS (fan-out to done, empty, failed).

- [ ] **Step 5: Commit**

```bash
git add crates/fleetd/src/server.rs
git commit -m "feat(swarm): run_swarm fan-out core (plan→admit→commit) over seams"
```

---

### Task 15: `POST /swarms`, `GET /swarms`, `GET /swarms/:id`

**Files:**
- Modify: `crates/fleetd/src/server.rs` (`router`, handlers)

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn post_swarms_validates_then_returns_id() {
    let state = AppState::default();
    // unknown mode → 400, no row created
    let bad = create_swarm(State(state.clone()), Json(CreateSwarmReq {
        doc_path: "spec.md".into(), mode: "weird".into(), ..Default::default()
    })).await;
    assert!(bad.is_err());
    // demo → ok, returns an id, row is 'planning' (or further along once the task runs)
    let ok = create_swarm(State(state.clone()), Json(CreateSwarmReq {
        doc_path: "spec.md".into(), mode: "demo".into(), ..Default::default()
    })).await.expect("ok");
    assert!(ok.0.swarm_id.starts_with("sw"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p fleetd post_swarms_validates_then_returns_id`
Expected: FAIL — no `create_swarm` / `CreateSwarmReq`.

- [ ] **Step 3: Implement the request type + handlers + routes**

Add a swarm id allocator field to `AppState` (`next_swarm: Arc<AtomicU64>`, seeded to 1 — swarm ids needn't survive collision the way units do, but seed from a `max_swarm_seq` for cleanliness; for the plan, `AtomicU64::new(1)` is acceptable since swarm ids are not reused by reconcile). Then:

```rust
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
    let (repo_url, repo_slug, base_branch) = (
        req.repo_url.unwrap_or_else(|| "https://github.com/adbarc92/command-center-agent-sandbox".into()),
        req.repo_slug.unwrap_or_else(|| "adbarc92/command-center-agent-sandbox".into()),
        req.base_branch.unwrap_or_else(|| "main".into()),
    );
    let row = {
        let s = st.store.lock().unwrap();
        let committed = s.committed_spend(since).unwrap_or(0.0);
        if committed >= st.global_cap {
            return Err((StatusCode::TOO_MANY_REQUESTS, "global daily cost cap reached".into()));
        }
        let usd_budget = req.usd_budget.unwrap_or_else(|| (st.global_cap - committed).min(15.0).max(0.0));
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
                // A demo swarm uses a scripted 2-lane split.
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
```

Register routes in `router`:

```rust
        .route("/swarms", post(create_swarm).get(list_swarms))
        .route("/swarms/:id", get(get_swarm))
```

Add `next_swarm: Arc<AtomicU64>` to `AppState` + `AppState::new` (`Arc::new(AtomicU64::new(1))`) and `Default`.

- [ ] **Step 4: Run tests**

Run: `cargo test -p fleetd post_swarms_validates_then_returns_id`
Expected: PASS. (This task references `ClaudePlanner::new()`/`GitDocSource::new()` which land in Tasks 16–17. To keep this task compiling, temporarily make the `"real"` arm `unreachable!("real swarm mode lands in Tasks 16–17")` and replace it in Task 17. Note this clearly so it isn't shipped.)

- [ ] **Step 5: Commit**

```bash
git add crates/fleetd/src/server.rs
git commit -m "feat(swarm): POST /swarms (sync-validated) + GET /swarms[/:id] with done rollup"
```

---

### Task 16: swarm reconcile on startup

**Files:**
- Modify: `crates/fleetd/src/server.rs` (`reconcile_on_startup`)

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn reconcile_marks_planning_swarm_failed_and_resumes_fanning_out() {
    let store = Arc::new(Mutex::new(Store::open_memory().unwrap()));
    {
        let s = store.lock().unwrap();
        // A swarm stuck mid-planning at crash → must become failed.
        let mut p = swarm_row_srv("swP", "planning");
        s.upsert_swarm(&p, now_ms()).unwrap();
        // A swarm mid-fan-out: lane 0 launched (unit exists), lane 1 not.
        let mut f = swarm_row_srv("swF", "fanning_out"); f.mode = "demo".into(); f.min_review_rounds = 1;
        s.upsert_swarm(&f, now_ms()).unwrap();
        s.upsert_lane("swF", 0, "A", "ta", "r", "admit", Some("u1")).unwrap();
        let mut u = building_row("u1"); u.swarm_id = Some("swF".into()); u.phase = "building".into();
        s.upsert_unit(&u, now_ms()).unwrap();
        s.upsert_lane("swF", 1, "B", "tb", "r", "admit", None).unwrap();
        let _ = &mut p;
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
```

Add a `swarm_row_srv` helper in the server tests mod (same shape as the store one).

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p fleetd reconcile_marks_planning_swarm_failed_and_resumes_fanning_out`
Expected: FAIL — reconcile doesn't touch swarms yet.

- [ ] **Step 3: Extend `reconcile_on_startup`**

After the existing unit-reconcile loop, add swarm handling:

```rust
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
fn resume_fan_out(st: &AppState, sw: &crate::store::SwarmRow) {
    let lanes = st.store.lock().unwrap().lanes_for_swarm(&sw.swarm_id).unwrap_or_default();
    let mut launched = 0u32;
    for l in lanes.into_iter().filter(|l| l.decision == "admit") {
        // Launched iff unit_id set AND the row actually exists (defensive).
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
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p fleetd reconcile_marks_planning_swarm_failed_and_resumes_fanning_out`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/fleetd/src/server.rs
git commit -m "feat(swarm): reconcile planning→failed and resume fanning_out"
```

---

## Phase 5 — Real seams (gated smoke only)

### Task 17: `ClaudePlanner` + `GitDocSource` (real impls, behind the demo path)

**Files:**
- Modify: `crates/fleetd/src/planner.rs`, `crates/fleetd/src/docsource.rs`, `crates/fleetd/src/server.rs`

These call out to Claude / git, so they get a Docker/gh/key-gated `#[ignore]` smoke test, mirroring `preflight_it`. The unit suite stays hermetic.

- [ ] **Step 1: Implement `GitDocSource` with guaranteed cleanup**

In `docsource.rs`:

```rust
/// Shallow-clones the base branch to a temp dir, reads the file, and removes the
/// dir on drop regardless of outcome (a failed/empty swarm has no driver to clean up).
pub struct GitDocSource;
impl GitDocSource { pub fn new() -> Self { Self } }
impl Default for GitDocSource { fn default() -> Self { Self::new() } }

struct TempClone(std::path::PathBuf);
impl Drop for TempClone {
    fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); }
}

#[async_trait]
impl DocSource for GitDocSource {
    async fn read(&self, repo_url: &str, base_branch: &str, doc_path: &str) -> Result<String, DocError> {
        let dir = std::env::temp_dir().join(format!("cc-plan-{}", std::process::id()));
        let _guard = TempClone(dir.clone());
        let ok = tokio::process::Command::new("git")
            .args(["clone", "--depth", "1", "--branch", base_branch, repo_url])
            .arg(&dir).output().await
            .map_err(|e| DocError::Failed(e.to_string()))?;
        if !ok.status.success() {
            return Err(DocError::Failed(String::from_utf8_lossy(&ok.stderr).into()));
        }
        let path = dir.join(doc_path);
        tokio::fs::read_to_string(&path).await
            .map_err(|_| DocError::NotFound(doc_path.into()))
        // _guard drops here → temp dir removed.
    }
}
```

- [ ] **Step 2: Implement `ClaudePlanner`**

In `planner.rs`, add a real impl that shells the Claude CLI (or HTTP) with a decomposition prompt and parses a JSON array of `{title, task, rationale}` plus cost via `claude_meter`. Keep the prompt and parsing in this module:

```rust
/// Real planner: a read-only Claude call that emits a JSON lane array. Cost is
/// parsed from the CLI `result` record via `crate::claude_meter`.
pub struct ClaudePlanner;
impl ClaudePlanner { pub fn new() -> Self { Self } }
impl Default for ClaudePlanner { fn default() -> Self { Self::new() } }

#[async_trait]
impl Planner for ClaudePlanner {
    async fn plan(&self, doc: &str, lane_cap: usize) -> Result<PlanOutcome, PlanError> {
        let prompt = format!(
            "Split the following spec into at most {lane_cap} INDEPENDENT lanes that can be \
             built in parallel without colliding. Reply ONLY with a JSON array of objects \
             {{\"title\":..,\"task\":..,\"rationale\":..}}. Spec:\n\n{doc}"
        );
        let out = tokio::process::Command::new("claude")
            .args(["-p", &prompt, "--output-format", "stream-json", "--max-budget-usd", "1.0"])
            .output().await.map_err(|e| PlanError::Failed(e.to_string()))?;
        let stdout = String::from_utf8_lossy(&out.stdout);
        // Reuse the existing meter: parse_usage(&[String]) -> Option<Usage>; Usage.cost_usd.
        let lines: Vec<String> = stdout.lines().map(|l| l.to_string()).collect();
        let cost = crate::claude_meter::parse_usage(&lines).map(|u| u.cost_usd).unwrap_or(0.0);
        let json_slice = extract_json_array(&stdout).ok_or_else(|| PlanError::Failed("no JSON array".into()))?;
        #[derive(serde::Deserialize)]
        struct RawLane { title: String, task: String, #[serde(default)] rationale: String }
        let raw: Vec<RawLane> = serde_json::from_str(json_slice)
            .map_err(|e| PlanError::Failed(format!("bad JSON: {e}")))?;
        let lanes = raw.into_iter().take(lane_cap)
            .map(|r| Lane { title: r.title, task: r.task, rationale: r.rationale }).collect();
        Ok(PlanOutcome { lanes, cost_usd: cost })
    }
}

/// Find the first top-level JSON array in mixed CLI output.
fn extract_json_array(s: &str) -> Option<&str> {
    let start = s.find('[')?;
    let end = s.rfind(']')?;
    if end > start { Some(&s[start..=end]) } else { None }
}
```

Check the actual `claude_meter` API and adjust `cost_from_stream` to whatever the existing function is named (it already parses cost from `result` records — reuse it; do not invent a new parser). If no reusable helper exists, parse the `result` line's `total_cost_usd` field here.

- [ ] **Step 3: Wire the real arm in `create_swarm`**

Replace the `unreachable!` placeholder from Task 15 with the real seams:

```rust
            _ => {
                use crate::{docsource::GitDocSource, planner::ClaudePlanner};
                run_swarm(st2, id2, ClaudePlanner::new(), GitDocSource::new()).await;
            }
```

- [ ] **Step 4: Add a gated smoke test**

In `crates/fleetd/tests/`, add `swarm_smoke_it.rs`:

```rust
//! Real-seam smoke: requires `claude` + git + ANTHROPIC_API_KEY. Ignored by default.
#[tokio::test]
#[ignore = "requires claude CLI + git + ANTHROPIC_API_KEY; makes a paid call"]
async fn git_doc_source_clones_reads_and_cleans_up() {
    use fleetd::docsource::{DocSource, GitDocSource};
    let d = GitDocSource::new();
    let out = d.read("https://github.com/adbarc92/command-center-agent-sandbox", "main", "README.md").await;
    assert!(out.is_ok(), "reads a known file");
    // Temp dir removal is covered by the Drop guard; assert no cc-plan-* dir lingers.
}
```

- [ ] **Step 5: Run the hermetic suite (smoke stays ignored)**

Run: `cargo test -p fleetd`
Expected: PASS; `git_doc_source_clones_reads_and_cleans_up` shows `ignored`.

- [ ] **Step 6: Commit**

```bash
git add crates/fleetd/src/planner.rs crates/fleetd/src/docsource.rs crates/fleetd/src/server.rs crates/fleetd/tests/swarm_smoke_it.rs
git commit -m "feat(swarm): real ClaudePlanner + GitDocSource (gated smoke)"
```

---

## Phase 6 — Full-suite gate

### Task 18: whole-suite green + clippy

- [ ] **Step 1: Run the full workspace test suite**

Run: `cargo test`
Expected: all `fleet-core` + `fleetd` tests PASS; only the Docker/gh/claude integration tests `ignored`.

- [ ] **Step 2: Lint**

Run: `cargo clippy -p fleet-core -p fleetd --all-targets -- -D warnings`
Expected: no warnings. Fix any (dead-code `SELECT_SWARM_COLS`, unused imports).

- [ ] **Step 3: Commit any lint fixes**

```bash
git add -A
git commit -m "chore(swarm): clippy clean across the swarm feature"
```

---

## Self-review checklist (run by the implementer before opening a PR)

- **Spec coverage:** P1 (Task 4), P2 (Tasks 6/14 admission critical section), P3 (Task 1), P4 (Tasks 5/6); pure core (Tasks 7–8); seams + fakes (Tasks 9–10); persistence incl. `committed_spend` MAX + `commit_lane_unit` + rollup (Tasks 5, 11, 12); `spawn_unit` Result + tier/usd_cap threading (Tasks 13–14); endpoints + sync validation + Docker preflight + `done`/`empty` semantics (Task 15); reconcile planning→failed + resume (Task 16); real seams + cleanup + gated smoke (Task 17).
- **`drop_global_cap`** is exercised by the fan-out loop; add a focused test if the global cap path isn't covered by Task 14 (a swarm whose per-lane re-check trips the cap mid-loop).
- **No placeholders shipped:** the Task 15 `unreachable!` for the real arm MUST be replaced in Task 17 — grep for `unreachable!("real swarm` before PR.
- **Type consistency:** `SwarmRow`/`LaneRow`/`UnitRow.swarm_id`, `AdmissionConfig`, `LaneDecision`, `run_swarm`, `spawn_unit`, `spawn_driver_for`, `lane_spec` names match across tasks.
