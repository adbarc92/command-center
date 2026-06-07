# Handoff — finish SP1-Hardening (Phases 4b, 5, 6)

> For the agent picking this up. Everything referenced is in the repo or git; this
> doc adds only the state + gotchas that aren't already captured elsewhere.
> Date: 2026-06-06.

## Your mission

Finish the SP1 daily-use hardening by implementing the remaining plan phases:
- **Phase 4b — atomic rehydration** (resume a restart-stranded unit)
- **Phase 5 — concurrency semaphore + admission cost cap** (server wiring)
- **Phase 6 — cockpit reconnect + cost bar + badges**

The authoritative task breakdown (bite-sized, TDD) is here — **follow it**:
- Plan: [plans/2026-06-06-sp1-hardening.md](plans/2026-06-06-sp1-hardening.md)
- Design (approved, 3 critique rounds): [specs/2026-06-06-sp1-hardening-design.md](specs/2026-06-06-sp1-hardening-design.md)
- Parent SP1 design: [specs/2026-06-05-command-center-sp1-design.md](specs/2026-06-05-command-center-sp1-design.md)
- Project vision/decomposition: [../command-center-vision.md](../command-center-vision.md)

## Current state (all committed, all green)

- **Branch:** `feat/sp1-hardening` (off `feat/sp1-fleet-engine`, which is PR #1 → `main`).
  Work here; do **not** merge to `main` without asking the user.
- **Done:** Phase 1 (resume path), Phase 2 (SQLite persistence + endpoints, verified across
  restart), Phase 3 (volume lifecycle), Phase 4a (startup reconciliation). See `git log`.
- **Tests:** `cargo test --workspace` → 28 `fleet-core` + 26 `fleetd` green; 2 ignored
  real-Docker ITs (`local_docker_it`, `preflight_it`). `cargo clippy --workspace
  --all-targets` is clean. **Keep both green at every commit.**

## What already exists that 4b/5/6 build on (read before coding)

- **`RunCtx`** (`crates/fleetd/src/driver.rs`): `{ start_seq, start_cost, resume, permits:
  Arc<Semaphore> }`. The driver seeds seq+cost from it, skips the oracle on
  `resume && spec.oracle_frozen`, acquires a permit at the **top of the `Provisioning` arm**
  (emits `Blocked{"awaiting concurrency slot"}`), and **releases the permit + tears down the
  container on entering `NeedsHuman`/`Halted`/terminal**. `run()` currently hardcodes
  `phase: Phase::Queued`.
- **`Runner` trait** (`runner.rs`): `provision` (probe-based volume reuse), `exec(workdir)`,
  `commit_all`, `has_diff`, `export_bundle`, `teardown` (keeps volume), `discard`
  (removes volume), `list_unit_containers`, `reap_unit`. `FakeRunner` implements all with
  settable `unit_containers` and `teardowns`/`discards` counters.
- **`Store`** (`store.rs`): SQLite/WAL; `upsert_unit`, `update_unit`, `append_event`,
  `get_unit`, `list_units`, `events_since`, `spend_since(since_ts)`. `UnitRow` carries the
  full spec + `phase`/`cost`/`last_seq`/`oracle_frozen`/`terminal_reason`.
- **`reconcile()`** (`reconcile.rs`): pure; `reconcile_on_startup` + `halt_in_store` in
  `server.rs` apply it. Called from `serve.rs` before serving.
- **Server** (`server.rs`): `AppState { units, next_id, store: Arc<Mutex<Store>>, docker }`.
  Endpoints: `POST /missions`, `GET /units`, `GET /units/:id`, `GET /units/:id/events?since`,
  `POST /units/:id/commands`, `GET /units/:id/stream?since`, `GET /health`. The per-unit
  **forwarder** (`spawn_forwarder`) persists each event + projection then broadcasts.
- **Cockpit** (`cockpit/ui/`): `src/lib/{types,api,fleet}.ts`, `src/App.svelte`; Tauri shell
  in `src-tauri/`. Build: `cd cockpit/ui && npm run build && npm run check`.

## Gotchas that will bite you (learned the hard way)

1. **Rehydration needs a `start_phase`.** `run()` hardcodes `Phase::Queued`. Add
   `start_phase: Phase` to `RunCtx` (default `Queued` in `standalone()`); rehydrate with
   `start_phase: Halted` so the spawned driver parks at `Halted` and the `Resume` command
   drives it `→ Provisioning` (which reuses the volume). Update the existing explicit
   `RunCtx { .. }` literal in the `resume_skips_oracle_*` driver test.
2. **Rehydration must be atomic.** Two concurrent `Resume` POSTs must yield **one** driver.
   Do check-and-insert **under the `units`-map mutex**: while holding the lock, if absent,
   insert the `UnitHandle` (channels/forwarder/bcast), drop the lock, then `tokio::spawn`
   the driver. Factor the handle construction out of `create_mission` and reuse it.
3. **Mode isn't persisted.** Resume-after-restart is meaningful only for **real** units;
   rehydrate with `LocalDockerRunner` + `GhForge` (needs `ANTHROPIC_API_KEY`). Demo units
   are throwaway — fine if they don't resume post-restart (note it).
4. **Phase 5 semaphore injection.** `AppState` must own `Arc<Semaphore>`
   (`CC_MAX_CONCURRENT`, default 3) and pass *that* (clone) into every `RunCtx` built in
   `create_mission`/rehydrate — currently they use `RunCtx::standalone()` which makes its own
   `Semaphore(1)`. The driver-side permit logic is already done.
5. **Global cap is admission-only.** In `create_mission`, before building the unit:
   `if store.spend_since(now_ms() - 24*3600*1000) >= CC_GLOBAL_USD_CAP { 429 }`. No driver
   changes, no atomic (the mid-run global trip was deliberately cut — see design R3).
6. **Never hold the `Store` mutex across `.await`** (rusqlite guards aren't `Send`). Do all
   DB ops in a sync block; `await` runner/forge calls *outside* the lock.
7. **`FakeRunner` execs don't yield**, so you can't deterministically halt a unit mid-loop
   in a test. Test resume by constructing a `resume:true` run **directly** (see
   `driver.rs::tests::resume_skips_oracle_and_continues_cost_and_seq`). For rehydration,
   test the atomic register-if-absent logic without spawning a real Docker driver (factor it).
8. **Design-critique gate hook** only fires on `docs/superpowers/specs/*-design.md`. You are
   implementing an approved design — do **not** write new design docs; edit code freely.
9. **Commit hygiene (user global rules):** per-task commits; **no** `Co-Authored-By`; **no**
   "Generated with Claude Code" footer; don't commit unless the task is green; never push to
   `main` directly.

## Definition of done

- Phases 4b/5/6 implemented per the plan; `cargo test --workspace` + `cargo clippy
  --workspace --all-targets` clean; `cd cockpit/ui && npm run build && npm run check` clean.
- Manual acceptance (the headline daily-use win): start `serve`, launch a demo unit, **kill
  + restart** `serve`, reload the cockpit → the unit reappears (`Halted`); pressing RESUME on
  a real unit re-provisions reusing the volume and continues **without re-running the
  oracle**. A 4th concurrent unit waits while 3 run. `POST /missions` 429s past the daily cap.
- Then use **superpowers:requesting-code-review** and report back; do not merge.

## Suggested skills

- **superpowers:executing-plans** (or **superpowers:subagent-driven-development**) — execute
  the plan task-by-task.
- **superpowers:test-driven-development** — red/green per task.
- **superpowers:verification-before-completion** — run the commands, show output, before
  claiming done.
- **superpowers:requesting-code-review** — before handing back.
