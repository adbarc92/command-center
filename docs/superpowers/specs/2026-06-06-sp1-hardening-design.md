# SP1-Hardening — Daily-Use Design

> Status: **passed 3 adversarial critique rounds; ready for plan.** See Design Critique Log.
> Parent: [./2026-06-05-command-center-sp1-design.md](./2026-06-05-command-center-sp1-design.md)
> Last updated: 2026-06-06

## Goal

Make the SP1 fleet engine robust enough to live in daily: survive cockpit reloads and
daemon restarts, never strand a spending container, bound concurrency and total spend, make
**resume real and budget/oracle-correct**, and make the desktop REAL-mode flow pleasant.
Four axes on `fleetd` + the cockpit; also fixes latent SP1 bugs the critique surfaced.

### Pre-existing SP1 bugs this corrects
- `teardown` removes the volume on every call; driver only tears down on terminal states;
  paused states never teardown. Realigned (Axis 2).
- `Trigger::Resume` routes `NeedsHuman/Halted → Building`, which execs against a torn-down
  container. Changed to **`→ Provisioning`** (Axis 4).
- `run()` hardcodes `cost_usd: 0.0`; a resumed (fresh) driver would re-open the full budget.
  Fixed via `RunCtx.start_cost` (Axis 3).

---

## Axis 1 — Persistence + reconnect (`fleetd`)

**Store:** SQLite via `rusqlite`, **WAL mode + `synchronous=NORMAL`**. DB at `CC_DB`
(default `./fleet.db`).
- `units(unit_id PK, tier, task, repo_url, repo_slug, base_branch, branch, test_cmd,
  usd_cap, wall_clock_secs, phase, cost, last_seq, oracle_frozen INT, created_ts, updated_ts,
  terminal_reason)` — stores the **full `UnitSpec`** (for rehydration) plus `oracle_frozen`.
- `events(unit_id, seq, ts, json, PRIMARY KEY(unit_id, seq))`

**One writer task, not a shared mutex (Round 3).** A single dedicated task owns the
`Connection` and consumes a `mpsc` of `(write-op)`; per-unit forwarders *send* to it. It
**batch-commits** (drain the channel, one txn per ~50ms tick) — so the per-line `Log` storm
doesn't serialize on a lock held across fsync, and no `.await` ever holds the connection.
Reads (`GET`/WS replay) use a separate read connection (WAL allows concurrent readers).

**`seq` — one authority (the driver).** It assigns monotonic per-unit `seq`. For cross-run
monotonicity, `run()` takes `RunCtx.start_seq` (= `units.last_seq` on resume). The writer
persists/broadcasts the driver envelope unchanged. The `units` row is a projection.

**Reconciliation is the only other writer**, and runs **before the server accepts
connections** (no forwarder concurrency), going through the same writer API.

**Endpoints:** `GET /units` (list), `GET /units/:id` (incl. `usd_cap`), `GET
/units/:id/events?since=N`, `GET /health` (`{docker, anthropic_key, version}`; docker
liveness **cached 5s TTL** via `docker version --format`), WS `/stream?since=N`
(**subscribe-first, then read store `seq>N`, then tail live, dedup by seq**).

---

## Axis 2 — Volume lifecycle, reconciliation, per-exec timeout

**Teardown split:** `teardown(handle)` removes the container, **keeps the volume**;
`discard(handle)` also removes the volume.

**Driver wiring (Round 3 forensics fix):**
- `NeedsHuman`/`Halted` → `teardown` (keep volume — resumable).
- `Failed` → `teardown` (**keep volume — forensics**; failures rarely reach `MergeCheck`, so
  the volume is the only copy of partial work/logs).
- `Done`/`NoChange` → `discard` (result is on the host: PR + bundle).
- `abandon` → `discard`.

**Volume retention:** startup reconciliation **discards volumes of terminal `Failed` units
older than `CC_KEEP_FAILED_HOURS` (default 24h)**, bounding disk to {paused units + recent
failures}. (Paused units are kept until resumed/abandoned.)

**Startup reconciliation — drives off the STORE, covers all quadrants (Round 3):**
`reconcile(persisted_nonterminal: &[UnitRow], running: &[unit_id]) -> Vec<Action>`, pure:
- non-terminal **in `running`** → `teardown` + synthetic `Halted` event.
- non-terminal **not in `running`** (died Queued/awaiting-permit/pre-provision) → synthetic
  `Halted` event, **no container to reap**.
- terminal in store **but still running** (a prior `discard` failed) → `teardown`/`discard`
  only, no event.
Synthetic `Halted` events are written **directly** to the store at `last_seq+1` (no
broadcast — no subscribers at boot), keeping row + log coherent.

**Per-exec timeout:** `steps` prefixes the **in-container** command with `timeout <wall>`
(daemon-independent), not a daemon-side wrapper.

---

## Axis 3 — Concurrency + (admission-only) global cost

**`RunCtx` (one struct, kills signature sprawl):**
```
struct RunCtx { start_seq: u64, start_cost: f64, resume: bool, permits: Arc<Semaphore> }
```
`run(runner, forge, spec, ctx, cmd_rx, evt_tx)`. `self.cost_usd` is seeded from
`ctx.start_cost` (fixes the budget-reset bug); `self.seq` from `ctx.start_seq`. `run_once`
builds `RunCtx{0, 0.0, false, Semaphore::new(1)}`; tests use `RunCtx::test()`.

**Concurrency:** `AppState` owns `Arc<Semaphore>` (`CC_MAX_CONCURRENT`, default 3). The
driver holds `Option<OwnedSemaphorePermit>`:
- **Acquired at the top of the `Provisioning` arm** (the single funnel for both
  `Queued→Provisioning` and `Resume→Provisioning`): emit `Blocked{"awaiting concurrency
  slot"}`, `acquire_owned().await`, then `provision`. So the wait is visible and the permit
  precedes any cost.
- **Released at *entry* to `NeedsHuman`/`Halted`/terminal** — handled at the **top of those
  arms** (idempotent: `if self.handle.is_some() { teardown; handle=None } self.permit=None`)
  *before* parking on `recv`. This is the entry-side cleanup Round 3 required; it also means
  the container is gone (volume kept) so the resume probe works. No self-deadlock: the permit
  is dropped before the unit can re-enter `Provisioning`.

**Global cost — admission-only (scoped down per Round 3).** The painful mid-run global trip
is **dropped** (it forced shared cost state into every driver). Instead: `POST /missions` is
refused `429` when `SELECT SUM(cost) WHERE created_ts >= now()-24h ≥ CC_GLOBAL_USD_CAP`
(default $20) — a true rolling-24h window read fresh each admission (low frequency; no
grow-only atomic, no brick). Spend is otherwise bounded per unit by `usd_cap` +
`--max-budget-usd` + the wall-clock cap, and externally by the Anthropic Workspace limit. A
single unit can't exceed the global alone (per-unit cap ≪ global).

`account()` is unchanged: returns the per-unit-cap bool → `CapBreach` → `NeedsHuman`.

---

## Axis 4 — True resume (oracle-skip, volume reuse, rehydration) + cockpit UX

**`Resume → Provisioning`** (fleet-core change; update its transition tests).

**Oracle-skip on resume (Round 3 — critical).** `Spec` is currently unconditional and would
regenerate + re-charge for the test set (and re-trigger T2/T3 approval) on every resume. Fix:
the `units.oracle_frozen` flag is set true when the oracle first freezes. On resume
(`ctx.resume && spec.oracle_frozen`), the `Spec` arm **short-circuits**: emit nothing, no
oracle exec, `goto(OracleFrozen)` → straight to `Building`. The frozen tests already live in
the reused volume. So resume continues mid-build, paying nothing for the oracle and skipping
the cleared approval gate.

**Volume-reuse probe (Round 2/3).** `provision`, after the container is up, runs `exec test
-d /work/repo/.git`:
- exit 0 (**reuse**): skip clone; idempotent `git config`; **`git checkout -B <branch>`**;
  clear stale `.git/index.lock`; keep the working tree.
- exit ≠ 0 (**fresh/empty volume**): clone + branch as today.

**Rehydration — atomic (Round 3).** After reconciliation a `Halted` unit is in the store but
absent from the in-memory map; a `Resume` POST would `404`. Fix: a `rehydrate(unit_id)`
helper does **check-and-insert under the `units`-map mutex** — while holding the lock, if
absent, insert the `UnitHandle` (channels/forwarder/broadcast/buffer) built from the loaded
`UnitRow`, then release and spawn `run(ctx{start_seq:last_seq, start_cost:cost, resume:true})`
parked in `Halted`; if a concurrent caller already inserted, fall through to the normal
`cmd_tx.send`. So two simultaneous Resume clicks yield **one** driver. `create_mission` and
rehydration share the handle-construction code.

**Two resume paths, reconciled:** (a) **in-process** (daemon alive, handle present): the
parked driver receives `Resume` on its existing `cmd_tx` and continues with its own
seq/cost — `start_seq`/`start_cost` irrelevant (same driver). (b) **rehydrated** (after
restart): a fresh driver seeded from the row. Both reach `Provisioning` and behave
identically thereafter.

**Cockpit:** on load `GET /health` + `GET /units` → repopulate, WS `/stream?since=<lastSeq>`
per unit; live `$ cost / cap` bar; key/docker badges; an `awaiting slot` indicator; tidy
controls + oracle panel.

---

## Runner trait additions / changes
- `list_unit_containers() -> Vec<String>`; `discard(handle)`; `teardown` keeps the volume;
  `provision` does probe-based reuse. `FakeRunner` gets settable container list + records
  teardown/discard calls.

## Testing (build the riskiest first — Round 3)
**First, a FakeRunner resume integration test** (the de-risking gate): launch → halt at
Building → resume, asserting (a) **no second `OracleProposed`/no `AwaitingOracleApproval`**,
(b) seq continues from `start_seq`, (c) exactly **one** permit across the cycle, (d)
`cost_usd` is **not** reset (continues from `start_cost`). If this can't go green, stop.
Then: `Store` (`:memory:`, WAL, append/seq/list/since/windowed-SUM); `reconcile()` four
quadrants; concurrency (4th waits, paused frees permit); rehydration atomicity (concurrent
Resume → one driver); WS `?since` dedup; one real-Docker test (halt→resume reuses volume,
opens PR). Existing 44 tests stay green (RunCtx; teardown semantics; transition test update).

## Decisions (approved 2026-06-06)
1. `rusqlite` + WAL, single writer task. 2. Restart = reap + coherent HALT, user RESUME
(volume reuse, oracle-skip, cost/seq continued). 3. Defaults: 3 concurrent, $20 rolling-24h
**admission** cap; keep Failed volumes 24h.

## Build order
1. Resume FakeRunner test scaffold + the driver changes it forces (`RunCtx` w/
start_seq/start_cost/resume, oracle-skip, permit lifecycle, `Resume→Provisioning`) — prove
green. 2. `Store` (WAL, writer task) + forwarder persistence + endpoints. 3. Teardown
split + volume-reuse probe + driver teardown/discard wiring + Failed-keep + retention. 4.
`list_unit_containers` + `reconcile()` + startup reconciliation + rehydration. 5. Semaphore
in AppState + admission `429`. 6. Cockpit.

## File structure
`crates/fleetd/src/{store.rs,reconcile.rs}` (new); `server.rs`, `driver.rs`,
`{runner,fake,local_docker}.rs`; `crates/fleet-core/src/transition.rs`;
`cockpit/ui/src/{lib/api.ts,App.svelte}`.

## Design Critique Log

### Critique Round 1
Code-grounded; 10 findings, two critical in actual code. Resolutions: teardown removed the
volume & pause never tore down → **teardown(keep)/discard(remove)** split; semaphore deadlock
& QUEUED-not-a-wait → released `OwnedSemaphorePermit`; global cap unwired → (initially)
driver atomic; rusqlite-across-await → sync block; two seq authorities → driver sole +
`start_seq`; reconciliation incoherent → synthetic event; timeout → in-container; volume
reuse → `checkout -B`; scope → build order + pure `reconcile()`.

### Critique Round 2
Round-1 policies lacked an implementation site in the real loop/object graph. Resolutions:
`Resume → Provisioning`; permit wait at top of Provisioning with a `Blocked` signal; cost
delta pinned per-invocation; atomic as sole gate seeded from rolling-24h SUM; reconciliation
writes directly to the store pre-serving; **rehydration** for store-only units; volume-reuse
**probe**; terminal `discard` (then refined in R3); **`RunCtx`** vs signature sprawl;
`/health` docker probe cached.

### Critique Round 3
Caught three critical code-rooted bugs the R2 fixes would have shipped, plus sharper gaps:
- **Resume re-runs/re-charges the oracle (#1):** `Spec` is unconditional → added
  `oracle_frozen` (persisted) and a **Spec short-circuit on resume**.
- **`account()` had no global return path (#2)** + **grow-only atomic bricks (#8):**
  **dropped the mid-run global trip entirely**; global cap is now **admission-only** on a
  fresh rolling-24h `SUM` — simpler and correct, no driver-global state.
- **`run()` resets `cost_usd:0.0` (#9 bombshell):** `RunCtx.start_cost` seeds the resumed
  driver so budget continues.
- **Rehydration race (#3):** **check-and-insert under the units-map mutex** → one driver.
- **`discard` destroys failed forensics (#4):** `Failed` now **keeps** the volume; only
  `Done/NoChange/abandon` discard; + a 24h retention sweep bounds disk.
- **reconcile missed never-provisioned units (#5):** reconcile drives off
  **persisted_nonterminal** across all four quadrants.
- **Permit/teardown timing on pause (#6):** cleanup moved to **arm entry** (teardown + drop
  permit before `recv`).
- **SQLite single-mutex contention + log storm (#7):** switched to a **dedicated writer task
  (mpsc) + WAL + batch commit**.
- **Sequencing (#10):** build the **resume path + its FakeRunner test first** (riskiest),
  Store second.

**Status:** sound and grounded; remaining risk is execution, gated by the resume test built
first.
