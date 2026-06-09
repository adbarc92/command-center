# Swarm Dispatch — Autonomous Decompose-and-Fan-Out

> Status: **brainstormed with user; passed 3 adversarial critique rounds; awaiting final
> user review of this spec.**
> Parent: [../../command-center-vision.md](../../command-center-vision.md)
> Builds on: [2026-06-05-command-center-sp1-design.md](./2026-06-05-command-center-sp1-design.md)
> Last updated: 2026-06-09

## Goal

Turn one multi-feature document into many mergeable PRs in parallel. A user points a
**swarm** at a doc/spec **path inside the target repo**; a **planner agent autonomously
decomposes** it into independent lanes; the daemon **fans out one agent unit per lane**,
each running the existing SP1 dispatch → oracle → build/check/review → verified-PR pipeline
**driver unchanged**, on its own branch and PR.

The swarm layer sits **on top of** the per-unit engine. It does not touch the unit
**driver or state machine**. It extends the unit **creation/persistence path**, the
**global-cap admission check**, and **daemon startup** — see "Required engine changes" and
"Blast radius" for the exact, bounded lists.

## Required engine changes (preconditions — must land first)

Pre-existing engine gaps the swarm forces into the light; the swarm is unsafe without them,
so they are part of this spec:

- **P1 — Seed `next_id` from the store inside `AppState::new` (R2 #1, R3 #4).** Today
  `next_id: AtomicU64::new(1)` is unconditional (`server.rs:87`); nothing re-mints ids on
  boot today (reconcile only *halts* — `server.rs:598`), so the latent `u{n}` collision never
  fires. The swarm's `fanning_out` resume is the **first** code to mint ids during startup; it
  would mint `u1…` over live rows and `upsert_unit`'s `ON CONFLICT DO UPDATE` would silently
  overwrite their phase/cost. Fix: add `Store::max_unit_seq()` and seed
  `next_id = max_unit_seq + 1` **inside `AppState::new`** (the single construction point,
  before `serve.rs` calls `reconcile_on_startup`).
- **P2 — Admission is one store-lock critical section + one transaction (R2 #2, R3 #2).**
  The original TOCTOU exists only because `create_mission` calls `spend_since` and
  `upsert_unit` in **two separate** `store.lock()` spans (`server.rs:207` then `:243`). The
  fix is **not** a second mutex (there is exactly one `Arc<Mutex<Store>>`; adding `admit`
  guards nothing it doesn't already guard and introduces a lock-ordering hazard against
  `spawn_forwarder`). The fix is: perform *read `committed_spend` → insert the unit's
  `queued` row → (swarm path) write the lane back-link* inside **one** `store.lock()` span
  wrapped in a single SQLite `IMMEDIATE` transaction. rusqlite is sync, so the lock is never
  held across `.await`. Driver spawn happens after the lock releases. One lock, no ordering
  question, race closed.
- **P3 — One authoritative terminal-phase list (R2 #4).** `fleet_core` has
  `Phase::is_terminal()` (`Done|NoChange|Failed`); the string set is also hardcoded in
  `reconcile_on_startup` (`server.rs:602`). Export `fleet_core::TERMINAL_PHASE_STRS:
  &[&str]`; `reconcile` and the swarm rollup both consume it. (Verified: the hardcoded set
  exactly matches `is_terminal()` today, so this is a faithful dedup.)
- **P4 — `committed_spend` replaces `spend_since` on BOTH admission paths (R3 #3).**
  `create_mission`'s existing `spend_since` call (`server.rs:207`) is **replaced** by
  `committed_spend`; otherwise the global-cap blindness Round-1 #3 claimed to fix remains
  live for standalone missions. The new `swarms`/`swarm_id` migrations go in `Store::init`
  (ordered before any query), like the existing `ALTER TABLE` migrations.

## What this proves (and what it doesn't)

**Proves:** the engine can take a coarse multi-feature input, split it autonomously, and run
N units within a **hard worst-case cost ceiling and a hard agent-count cap** — while the
**code-shipping decision per lane stays tier-gated** by that lane's oracle.

**Does not change:** the per-unit driver, the oracle/tier gate, container/credential
boundaries, or unit reconciliation of running containers.

## Key decisions (brainstorming, 2026-06-09)

| # | Decision | Choice |
|---|----------|--------|
| 1 | Swarm shape | **Decompose-and-fan-out** — one task → N independent lanes, one unit each, same repo, each its own branch/PR. |
| 2 | Who decomposes | **Fully autonomous** — no approval gate on the split. (Per-lane code-shipping stays oracle/tier-gated.) |
| 3 | Input | **A doc/spec path inside the target repo.** |
| 4 | Guardrails | **Both** a lane-count cap **and** a per-swarm USD ceiling on *committed worst-case* spend; whichever binds first stops admitting. Lanes also queue on `CC_MAX_CONCURRENT` and count against `CC_GLOBAL_USD_CAP` at commitment time. |
| 5 | Orchestration home | **Daemon-orchestrated.** State persisted for restart-safety. |

## Naming

The lane-count maximum is **one value** named `lane_cap` everywhere, sourced from request
param `max_lanes` (default env `CC_MAX_LANES`, default 8). `Planner::plan` receives this same
value. The per-lane dollar cap is **one value** `per_lane_cap`: `admit_lanes` projects with it
and `spawn_unit` receives that literal value as the unit's `usd_cap` — never parallel fields.

## How the budget actually binds (load-bearing)

The swarm budget is a **conservative commitment ceiling**, composed from two facts:

1. **Each admitted lane runs with `usd_cap = per_lane_cap`** — `spawn_unit` is called with
   exactly `cfg.per_lane_cap`.
2. **`admit_lanes` admits at most `floor((usd_budget − planner_cost) / per_lane_cap)`
   lanes.** Worst-case spend `= planner_cost + admitted × per_lane_cap`.

It is a ceiling, not a live meter (cheap lanes don't reclaim budget). Accepted by design.

### Committed-spend admission (R1 #3, R2 #2/#3, **R3 #1**)

The existing check compares `spend_since` (sum of live-updated `cost`) to the cap; at
fan-out time lane `cost` is `0`, so it waves through committed-but-unspent budget. Fix —
admission compares **committed spend**, partitioned by `TERMINAL_PHASE_STRS` (P3) so each
unit is counted **once**:

```
committed_spend(since) =
    Σ cost                of units WHERE created_ts ≥ since AND phase ∈ TERMINAL_PHASE_STRS
  + Σ MAX(usd_cap, cost)  of units WHERE created_ts ≥ since AND phase ∉ TERMINAL_PHASE_STRS
  + Σ planner_cost        of swarms WHERE created_ts ≥ since
```

**The non-terminal term is `MAX(usd_cap, cost)`, not `usd_cap` (R3 #1 — the most serious
surviving defect).** `driver.rs` `account()` adds a step's cost and emits the `Metric`
event (which the forwarder writes to `cost`) **before** checking `cost > usd_cap` and
parking — so a non-terminal unit's recorded `cost` can exceed its `usd_cap` by one step's
overshoot (exactly the `rate_limit_cost_breaching_cap_parks_via_cap_breach` scenario). Using
plain `usd_cap` would under-count real committed spend in the one direction that costs money;
`MAX` restores the never-under-count property. One `Store::committed_spend(since)` query.

Read under the **P2** single critical section, so no admitter observes a half-updated
transition (`update_unit` writes phase+cost in one statement).

Accepted trade-offs: a unit parked at `needs_human`/`halted` holds its reservation until it
ages out of the 24h window (`created_ts ≥ since`) — conservative, never unsafe, and **not** a
permanent leak (verified: the window drops it at 24h). `planner_cost` of `failed`/`empty`
swarms counts for 24h (real spend); it is written to the swarm row the moment the planner
returns, before any lane admission.

Within fan-out, each lane is admitted in its own P2 critical section with a fresh
`committed_spend` re-check; a lane stopped by the **global** cap (vs the **swarm** budget) is
recorded with a distinct decision `drop_global_cap` (R2 #11).

### Concurrency fairness & the dual-guardrail honesty note (R3 #7)

Lane units share the global `CC_MAX_CONCURRENT` semaphore (default 3); a large swarm delays
other work (R1 #4). No private pool/priority (YAGNI), modest `CC_MAX_LANES`, documented.

**At shipped defaults the budget binds before the lane cap:** `usd_budget≤$15`,
`per_lane_cap=$5` ⇒ ≤3 lanes, while `lane_cap=8`. So `lane_cap` is a **safety ceiling for
overridden budgets**, not a co-equal everyday limit. The "whichever binds first" framing is
honest only once a user raises `usd_budget` toward `lane_cap × per_lane_cap`. Documented
rather than pretending both bite at defaults.

## Architecture

Repo idiom: **pure sync core** + **seam traits with fakes**. Three new modules in
`crates/fleetd/src/`:

### `planner.rs`
```rust
pub struct Lane { pub title: String, pub task: String, pub rationale: String }
pub struct PlanOutcome { pub lanes: Vec<Lane>, pub cost_usd: f64 }
#[async_trait]
pub trait Planner: Send + Sync {
    async fn plan(&self, doc: &str, lane_cap: usize) -> Result<PlanOutcome, PlanError>;
}
```
`FakePlanner` (scripted) and `ClaudePlanner` (read-only Claude call; cost via `claude_meter`).

### `docsource.rs`
```rust
#[async_trait]
pub trait DocSource: Send + Sync {
    async fn read(&self, repo_url: &str, base_branch: &str, doc_path: &str)
        -> Result<String, DocError>;
}
```
`FakeDocSource` (canned + "not found") and `GitDocSource` (shallow clone + read). **Cleanup
(R3 #6):** `GitDocSource` clones to a temp dir and removes it via a drop/`finally` guard
**regardless of outcome** (success, error, empty), since a `failed`/`empty` swarm has no
driver lifecycle to hang cleanup off. (The pre-existing `cc-host-{unit_id}` clone leak in
`create_mission` is noted out-of-scope.)

### `swarm.rs` — pure admission core + types
```rust
pub struct AdmissionConfig { pub lane_cap: usize, pub usd_budget: f64,
                             pub per_lane_cap: f64, pub planner_cost: f64 }
pub enum LaneDecision { Admit, DropOverLaneCap, DropOverBudget }  // drop_global_cap set by
                                                                 // the fan-out loop, not here
pub fn admit_lanes(lanes: &[Lane], cfg: &AdmissionConfig) -> Vec<(usize, LaneDecision)>;
pub fn slug(title: &str) -> String;  // [a-z0-9-], trim/collapse dashes, truncate 32,
                                     // fallback "lane"; idx prefix guarantees uniqueness
```
Plus `SwarmSpec`, `SwarmRow`, `SwarmStatus`. No async/I-O.

### Seam-selection matrix (R2 #10) — `mode` selects all seams; a `demo` swarm never pays
| `mode` | Planner | DocSource | Runner | Forge |
|--------|---------|-----------|--------|-------|
| `demo` | `FakePlanner` | `FakeDocSource` | `FakeRunner` | `FakeForge` |
| `real` | `ClaudePlanner` | `GitDocSource` | `LocalDockerRunner` | `GhForge` |

## Data model

**`swarms`:**
```
swarms(swarm_id TEXT PRIMARY KEY, repo_url TEXT, repo_slug TEXT, base_branch TEXT,
  doc_path TEXT, tier TEXT, mode TEXT, lane_cap INTEGER, usd_budget REAL, per_lane_cap REAL,
  status TEXT,  -- planning | fanning_out | running | done | failed | empty
  planner_cost REAL, lanes_launched INTEGER, lanes_dropped INTEGER,
  min_review_rounds INTEGER, terminal_reason TEXT, created_ts INTEGER, updated_ts INTEGER)
```
**`swarm_lanes`** (idempotent/resumable fan-out — R1 #6, R2 #5):
```
swarm_lanes(swarm_id TEXT, idx INTEGER,  -- PRIMARY KEY(swarm_id, idx)
  title TEXT, task TEXT, rationale TEXT,
  decision TEXT,  -- admit | drop_lane_cap | drop_budget | drop_global_cap
  unit_id TEXT)   -- NULL until the lane's unit row is committed
```
**`units` gains nullable `swarm_id TEXT`** + `idx_units_swarm` index (R1 #13), via the
idempotent `ALTER TABLE … ADD COLUMN` trick **in `Store::init`** (P4 ordering). **Blast
radius (honest):** `UnitRow` gains a field; `SELECT_COLS_*`, `map_row`, `upsert_unit`'s param
list carry it (`ON CONFLICT` set untouched — set-once). Driver/state machine unchanged.

New store methods: `max_unit_seq` (P1), `committed_spend` (P4), `upsert_swarm`,
`update_swarm`, `get_swarm`, `list_swarms`, `upsert_lane`, `lanes_for_swarm`, `swarm_rollup`,
and `commit_lane_unit(swarm_id, idx, unit_row)` — **one transaction** inserting the unit's
`queued` row *and* setting `swarm_lanes.unit_id` (closes the lost-lane window, R2 #5). On
resume a lane counts as launched only if `unit_id` non-NULL **and** a unit row exists
(defensive re-check); else it is (re-)committed.

### Status model (R1 #8, R2 #4, R3 #6/#9)

`status` is authoritative through `planning → fanning_out → running`, plus terminal
`failed`/`empty`. Only `running → done` is derived, via one aggregate:
```
swarm_rollup(swarm_id) -> (total, terminal, awaiting_human)  -- one GROUP BY over units,
                                                            -- classified by TERMINAL_PHASE_STRS
```
- `planning` / `failed` (planner errored, zero lanes, or daemon restarted mid-planning) /
  `empty` (lanes produced but **zero admitted** — distinct terminal, never `done`).
- `fanning_out` → `running` (all admitted lanes committed).
- `done` — computed: `status==running && total>0 && terminal==total`.

**Parked children:** `needs_human`/`halted` are non-terminal, so a swarm with one stays
`running` — correct (it genuinely isn't done). `GET /swarms/:id` surfaces `awaiting_human`
from the rollup so it's observable, not silently stuck. A forever-`running` swarm (child
never resumed) is acceptable: its reservation and `planner_cost` **age out of the cap window
at 24h** (verified — no permanent headroom leak); only the status row persists.

## Lifecycle & fan-out

`POST /swarms` validates synchronously, returns `{ swarm_id }`, then runs the slow work in a
`tokio::spawn`ed task (R1 #5):

0. **Synchronous validation (R2 #6/#7, R3 #5).** All 4xx-able checks happen here, before the
   row or spawned task exists: unknown `mode` → 400; `mode==real` without `ANTHROPIC_API_KEY`
   → 400; **`mode==real` with Docker down → 400/503 via `docker_ok` (R3 #5)** — fail fast
   *before* the paid planner call rather than paying to plan and then fanning out N guaranteed
   provision failures; resolve `repo_url`/`repo_slug`/`base_branch` (explicit body fields,
   defaulting to the sandbox repo — no URL→slug parsing). The spawned task can never hit a
   4xx condition.
1. **Global-cap admission.** In a P2 critical section, refuse `429` if `committed_spend(24h)
   ≥ CC_GLOBAL_USD_CAP`. Else persist the `planning` row, return `{ swarm_id }`.
2. **Plan (spawned task).** `DocSource.read` → `Planner.plan(doc, lane_cap)`. Write
   `planner_cost` immediately. Planner error or zero lanes → `failed`.
3. **Admit (pure core).** `admit_lanes(...)`; persist each lane to `swarm_lanes` with its
   `decision`; set `lanes_dropped`. Drops are recorded **only** as columns
   (`swarm_lanes.decision` + `swarms.lanes_dropped`) — **no `SwarmLanesDropped` event**, since
   the events table is unit-keyed with no home for swarm events (R2 #8). Zero admitted →
   `empty`.
4. **Fan out (idempotent, per-lane isolated).** Set `fanning_out`. For each `admit` lane with
   NULL `unit_id`: open a P2 critical section, re-check `committed_spend`; if the global cap
   is now hit, set the lane `drop_global_cap` and stop; else `commit_lane_unit` (atomic
   row+back-link) with swarm-supplied repo coords, `usd_cap = per_lane_cap`, **`tier =
   swarm.tier`, `min_review_rounds = swarm.min_review_rounds` (R3 #10)**, `swarm_id` set,
   `branch = agent/{swarm_id}/{idx}-{slug(title)}`; close the section; then register + spawn
   the driver. A per-lane spawn failure (`register_unit_if_absent` → `None`) is mapped to that
   lane's `terminal_reason` and the loop **continues** — `spawn_unit` returns `Result` and
   **never `expect`s** (R2 #6, R3 #4). All admitted lanes committed → `running`.

`spawn_unit(spec, mode, swarm_id) -> Result<unit_id, SpawnError>` is extracted from
`create_mission` and used by both paths; it threads repo coords, `usd_cap`, `tier`,
`min_review_rounds`, `swarm_id`, and replaces the existing `.expect()` with `Result`.

**Default tier (R3 #10):** lanes inherit `swarm.tier`, default **T1** — which in this engine
is *autonomous oracle → review-gate → PR* (still objectivity-gated, no human approval step),
consistent with decision #2's "fully autonomous." Set `tier=T2/T3` for human oracle approval.

**Defaults.** `lane_cap` ← `max_lanes` (`CC_MAX_LANES=8`, a ceiling — see honesty note);
`per_lane_cap=5.0`; `usd_budget` ← `min(CC_GLOBAL_USD_CAP − committed_spend, $15)` (sized at
request time; the per-lane global re-check in step 4 is the real backstop, recording
`drop_global_cap` distinctly).

## HTTP surface (additive)

- `POST /swarms` → `{ swarm_id }` (immediate). Body: `{ doc_path, tier?, mode?, max_lanes?,
  usd_budget?, per_lane_cap?, min_review_rounds?, repo_url?, repo_slug?, base_branch? }`.
- `GET /swarms` → list (status via `swarm_rollup`).
- `GET /swarms/:id` → planner status + cost; per-lane decisions from `swarm_lanes`; child
  unit summaries (rolled up); `awaiting_human` count; computed status; and **"spent so far" =
  `Σ child.cost (actual) + planner_cost`** — defined explicitly and labeled distinct from the
  admission reservation, so reservations are never shown as spend (R3 #8).

No swarm-level WebSocket; the cockpit subscribes per lane via `/units/:id/stream`.

## Restart-safety

`reconcile_on_startup` (after P1 seeds `next_id`) gains:
- `planning` → `failed` (no units exist yet — created only in `fanning_out`).
- `fanning_out` → resume: per lane, launched iff `unit_id` non-NULL *and* row exists;
  (re-)commit the rest in P2 sections with committed-spend re-checks; then `running`.
- `running` → left as-is; `running → done` recomputed on read.

## Testing strategy (TDD)

- **Pure (`swarm.rs`):** `admit_lanes` (lane-cap binds, budget binds, both, planner-cost
  reservation, `planner_cost > usd_budget` ⇒ 0 ⇒ `empty`, all-dropped); `slug`
  (charset/length/empty-fallback).
- **Store:** swarm/lane CRUD; migration + index in `init`; `max_unit_seq`; `committed_spend`
  — including the **`MAX(usd_cap, cost)` over-cap case** (a non-terminal unit with
  `cost>usd_cap` contributes `cost`, R3 #1); planner_cost included; 24h window ages out
  parked units; `swarm_rollup` terminal vs awaiting_human.
- **Admission race (P2):** two concurrent admitters near a full cap → exactly one wins;
  committed total never exceeds the cap (single transaction, no second mutex).
- **`next_id` seeding (P1):** boot with persisted `u1..u5` ⇒ next mint `u6`; `fanning_out`
  resume mints no colliding id.
- **End-to-end (fakes):** N units with `swarm_id` + `usd_cap==per_lane_cap` + swarm `tier`
  + unique branches; drops recorded in `swarm_lanes`; zero-admitted ⇒ `empty`; `done` only
  when all children terminal; a `needs_human` child keeps swarm `running`, `awaiting_human=1`.
- **Resume idempotency (R2 #5):** crash after `commit_lane_unit` before spawn ⇒ no duplicate;
  `unit_id` set but row missing ⇒ re-committed (no lost lane).
- **Server:** `POST /swarms` validates synchronously (bad mode / missing key / Docker down ⇒
  4xx *before* `{swarm_id}`); committed-spend admission refuses `429` when N×cap would breach
  the global cap; standalone `create_mission` now also uses `committed_spend` (P4 regression).
- **Real `ClaudePlanner`+`GitDocSource`:** Docker/gh-gated smoke test only; asserts the temp
  clone is removed on success **and** on planner error.

## Out of scope (YAGNI)

Human approval/preview of the split; free-text/multi-repo decomposition; a live cross-unit
spend meter that reclaims budget; a per-swarm concurrency sub-pool/priority; a swarm-level
event stream; cross-lane dependency ordering; sibling-PR conflict resolution; a
swarm-abandon command for forever-`running` swarms (resources age out at 24h anyway); fixing
the pre-existing `cc-host-*` unit-clone leak.

## Open questions

None blocking. If a future planner needs more than one file of repo context, `DocSource`
widens without changing the orchestration.

## Design Critique Log

### Critique Round 1

Independent reviewer, grounded in the engine code. All resolved: (1) budget didn't bind at
runtime → worst-case commitment ceiling; (2) lane `usd_cap` not from `per_lane_cap` →
threaded literally; (3) global cap blind to committed spend → `committed_spend`; (4) fleet
starvation → accepted/documented; (5) synchronous handler stall → immediate return + spawned
task; (6) restart mid-fan-out orphaned children → `fanning_out` + `swarm_lanes` + idempotent
resume; (7) planner cost escaped the cap → included; (8) rollup contradictions / zero-admitted
vacuous `done` → authoritative status + `empty` + single aggregate; (9) branch collisions →
`{idx}-{slug}`; (10) hardcoded repo → `spawn_unit` parameterized; (11) `planner_cost >
budget` → `empty`; (12) "unchanged" overclaim → narrowed + blast radius; (13) unindexed
lookup → `idx_units_swarm`.

### Critique Round 2

A fresh reviewer pressure-tested the Round-1 fixes; several were themselves broken. Resolved,
mostly via the **Required engine changes**: (1) `next_id` never seeded → **P1**; (2)
committed-spend cross-task TOCTOU → a lock (later simplified in R3); (3) `committed_spend`
counting subtleties → exact once-per-unit partition + accepted reservations; (4) three
"terminal" definitions + parked-children wedge → **P3** + `awaiting_human` + honest `done`;
(5) lost-lane write-back → `commit_lane_unit` transaction + resume row-existence check; (6)
`spawn_unit` error contract / panic → all 4xx moved to sync handler, `Result` not `expect`;
(7) `repo_slug` had no source → explicit body field; (8) `SwarmLanesDropped` had nowhere to
live → removed, persisted as columns; (9) three names for the lane cap → unified; (10)
demo-mode seam wiring → explicit matrix; (11) stale budget default / conflated drops →
`drop_global_cap`; (12) unbounded slug → truncate/sanitize.

### Critique Round 3

A final reviewer verified the Round-1/2 fixes for correctness and mutual consistency and
found two were wrong/over-built plus several real gaps. All resolved:

1. **`committed_spend` under-counted** — `usd_cap` is *not* an upper bound on a non-terminal
   unit's live `cost`, because `account()` bills + emits a step before checking the cap
   (proven by the existing cap-breach test). Changed the non-terminal term to
   `MAX(usd_cap, cost)`. **The most serious surviving defect.**
2. **The Round-2 `admit` mutex was redundant and added a lock-ordering hazard** — the single
   `Arc<Mutex<Store>>` already serializes; the real fix is check+insert+back-link in **one**
   store-lock span + one SQLite transaction. Dropped the second mutex (**P2** rewritten).
3. **The committed-spend fix only landed on the swarm path** — `create_mission` still called
   the blind `spend_since`. Added **P4**: replace it on both paths; migrations in
   `Store::init`.
4. **P1 seeding had no concrete home / a retained `expect` could still panic** — seed inside
   `AppState::new`; `spawn_unit` maps `register → None` to `Err`, never `expect`.
5. **Docker not gated at real-swarm admission** — would pay to plan, then fan out N
   provision-failures. Added a `docker_ok` preflight to step 0 for `mode==real`.
6. **`GitDocSource` clone cleanup unspecified** — a `failed`/`empty` swarm leaked the temp
   dir. Specified a drop-guard cleanup regardless of outcome; smoke test asserts it.
7. **`lane_cap` dead weight at defaults** ($15/$5 ⇒ 3 lanes < cap 8) — reframed `lane_cap` as
   a ceiling for overridden budgets, not a co-equal everyday bound; documented honestly.
8. **`GET /swarms/:id` cost undefined / could show reservations as spend** — defined "spent so
   far" = `Σ child.cost + planner_cost`, distinct from the admission reservation.
9. **Forever-`running` swarm** — verified resources age out of the 24h cap window (no
   permanent leak); a swarm-abandon command is explicitly out-of-scope.
10. **`tier`/`min_review_rounds` not threaded to lanes** — `spawn_unit` threads both;
    documented the default swarm tier (**T1 = autonomous oracle**, still objectivity-gated),
    consistent with decision #2.

Verified sound by Round 3 (no change needed): P1's premise (reconcile mints no ids today),
P3's terminal-list match, `update_unit`'s atomic phase+cost write, and the 24h aging-out of
abandoned reservations.
