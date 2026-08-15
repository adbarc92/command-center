---
plan-format: 1
next_id: 132

# RATIFICATION PENDING (proposed 2026-08-13, first run) — the Impact axis, 1-5 per module.
# Nothing here was derived from call-graph fan-in; these are judgments about blast radius.
spine_weight:
  tauri_host: 5           # the desktop shell: a defect here freezes or kills the whole product
  ui_shell: 5             # App.svelte is the only surface the operator touches
  fleetd_driver: 5        # the autonomous execution spine; defects burn real money and real agent time
  build_ci_gate: 5        # the gate itself: if it does not run, no other module's coverage is real
  app_plugin_runtime: 5   # CARVE-OUT, see Trust verdicts
  fleetd_server: 4        # the control plane every UI surface reads from
  fleetd_store: 4         # persistence; loss or corruption is unrecoverable
  fleetd_forge: 4         # container + GitHub lifecycle; orphaned resources and lost work
  ui_plugin_bridge: 4     # the trust boundary for untrusted plugin code
  fleet_core: 3           # small pure state machine, the best-covered thing in the repo
  ui_dashboard: 3         # a read-only viewer today; a wrong card misleads, it does not destroy
  dev_scripts: 3          # build/demo scripts; failure is loud and local
  session_state: 2        # developer-experience plugin; failure loses a session note
  py_tools: 2             # operator hooks; failure degrades a Claude session, not the product

core_entry_points:        # ratified traversal roots for reachability
  - cockpit/ui/src/main.ts
  - cockpit/ui/src-tauri/src/main.rs
  - crates/fleetd/src/bin/serve.rs
  - crates/fleetd/src/bin/run_once.rs

tier_map:
  rust-workspace:
    runners: ["cargo test --workspace"]
    globs:   [crates/fleet-core/**, crates/fleetd/**]
    trust:   reports-dne
    in_ci:   true
    parse_spec:
      ok: '^test (?<test>\S+) \.\.\. ok$'
      summary: '^test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out'
  tauri-host:
    runners: ["cargo test --manifest-path cockpit/ui/src-tauri/Cargo.toml"]
    globs:   [cockpit/ui/src-tauri/**]
    trust:   reports-dne
    in_ci:   false          # standalone [workspace]; see GAP for the CI hole
    parse_spec:
      ok: '^test (?<test>\S+) \.\.\. ok$'
      summary: '^test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out'
  vitest:
    runners: ["npm test (cwd cockpit/ui)"]
    globs:   [cockpit/ui/src/**]
    trust:   reports-dne
    in_ci:   false
    parse_spec:
      ok: '^ . (?<file>src/\S+) \((?<n>\d+) tests?\)'
      summary: '^\s*Tests\s+(\d+) passed \((\d+)\)$'
  node-embargo:
    runners: ["node --test scripts/embargo-guard.test.mjs"]
    globs:   [scripts/**]
    trust:   reports-dne
    in_ci:   true
    parse_spec:
      ok: '^ok (?<n>\d+) - (?<test>.+)$'
      summary: '^# pass (\d+)$'
  node-session-state:
    runners: ['node --test "plugins/session-state/test/*.test.mjs"']
    globs:   [plugins/session-state/**]
    trust:   reports-dne
    in_ci:   false
    parse_spec:
      ok: '^ok (?<n>\d+) - (?<test>.+)$'
      summary: '^# pass (\d+)$'
  pytest:
    runners: ["uv run pytest (cwd tools/budget-checkpoint)", "uv run pytest (cwd tools/cache-countdown)"]
    globs:   [tools/**]
    trust:   reports-dne
    in_ci:   false
    parse_spec:
      ok: '^\.+\s+\[\s*\d+%\]$'
      summary: '^(\d+) passed(?:, (\d+) skipped)?(?:, (\d+) failed)? in [\d.]+s$'
  manual:
    runners: ["a watched GUI session on the target machine"]
    globs:   [spikes/SPIKE-RESULTS.md]
    trust:   unverified
    in_ci:   false
    parse_spec:
      ok: 'PASS'
      summary: 'n/a - a human writes the result row by hand'
---

# Testing Plan — Command Center

> Risk-ranked map of testing gaps, automated and human-QA in one ranking. Produced by the
> `testing-plan` skill. **This file is the register; it writes no tests and changes no code.**
> Entries are append-only and IDs are never reused. `decision` / `rationale` / `owner` are
> human-owned fields — a scan may propose, never overwrite.

## 1. Run stamp

| | |
|---|---|
| **Run** | 2026-08-13 (first run — bootstrap) |
| **Commit** | `a3edc78` on `feat/plugin-runtime` |
| **Working tree at scan time** | `M cockpit/ui/src-tauri/src/plugins/manager.rs`, `?? cockpit/ui/src-tauri/tests/` — **concurrent work by another agent**, see the carve-out in §2 |
| **Repo age** | 223 commits, first commit 2026-06-04 (70 days) |

### Per-tier results this run

| Tier | Runner | Runs in CI? | This run |
|---|---|---|---|
| `rust-workspace` | `cargo test --workspace` | **yes** | **not run** — the cargo build was held by a concurrent agent; stamped `(static-only)` |
| `tauri-host` | `cargo test` in `cockpit/ui/src-tauri` | **NO** | **not run** — same reason; stamped `(static-only)` |
| `vitest` | `npm test` in `cockpit/ui` | **NO** | **GREEN** — 19 files, 135 tests, 135 passed, 0 failed, 0 skipped, 0 todo (209 s) |
| `node-embargo` | `node --test scripts/embargo-guard.test.mjs` | **yes** | **GREEN** — 13 tests, 13 pass, 0 fail, 0 skipped, 0 todo |
| `node-session-state` | `node --test "plugins/session-state/test/*.test.mjs"` | **NO** | **GREEN** — 52 tests, 52 pass, 0 fail, 0 skipped, 0 todo |
| `pytest` (budget-checkpoint) | `uv run pytest` | **NO** | **GREEN** — 24 passed |
| `pytest` (cache-countdown) | `uv run pytest` | **NO** | **GREEN** — 29 passed |
| `manual` | watched GUI session | n/a | **1 of 12 rows PASS**; 1 FAIL-then-fixed-but-never-re-watched; 1 undiagnosed ANOMALY; 9 never run |

### Coverage holes — read these before trusting any number above

1. **Five of the seven automated tiers never run in CI.** `.github/workflows/ci.yml` has exactly
   three jobs: `embargo`, `test` (`cargo test --workspace`), and `build` (`tauri build` ×3 OS).
   Only `rust-workspace` and `node-embargo` gate a pull request. 135 vitest tests, 28 Tauri-host
   Rust tests, 52 session-state tests and 53 pytest tests are **advisory**, not gating.
2. **`cargo test --workspace` does not reach the Tauri host crate.** `cockpit/ui/src-tauri/Cargo.toml`
   declares a bare `[workspace]`, so the root workspace (`crates/fleet-core`, `crates/fleetd`) excludes
   it. CI compiles that crate via `tauri build` and never executes one of its tests.
3. **CI has no Docker daemon.** `crates/fleetd/tests/{local_docker_it,preflight_it,swarm_smoke_it}.rs`
   are `#[ignore]`d (`swarm_smoke_it` for git network access, the other two for Docker) and run
   nowhere automatically. Everything whose only coverage is one of those files is **human-QA-only
   today**; entries say so individually.
4. **No lint, format, or static-analysis gate exists.** `clippy`, `rustfmt`, `cargo fmt`, eslint and
   prettier appear in zero workflow files.
5. **`release.yml` runs no tests at all** before signing and publishing. `releaseDraft: true` — a
   human clicking publish — is the only thing between an untested build and users.
6. **`npm run check`** (svelte-check + tsc, 353 files) is not in CI either.
7. **`churn_90d` is lifetime churn on this repo.** The first commit is 70 days old, so the 90-day
   window covers all 223 commits. Churn points therefore skew high across the board; treat them as a
   relative ordering signal, not an absolute rate.
8. **No captured per-test inventory exists for the two Rust tiers this run.** Every Rust claim below
   is static-analysis only. No `open → covered` transition would be licensed for those tiers.

## 2. Trust verdicts

| Tier · runner | Trust | Basis |
|---|---|---|
| `rust-workspace` · `cargo test --workspace` | `reports-dne` | libtest's summary carries a real `ignored` count and `#[ignore]` is a first-class primitive this repo actually uses. **But not run this session** — `(static-only)`. |
| `tauri-host` · `cargo test` | `reports-dne` | same runner grammar. **Not run this session**, and never run by CI at all. |
| `vitest` · `npm test` | `reports-dne` | vitest's summary distinguishes `passed` / `skipped` / `todo`; this run reported 0 of the latter two. Verified against a real captured run. |
| `node-embargo` · `node --test` | `reports-dne` | TAP emits `# skipped` and `# todo` separately from `# pass`. Captured: both 0. |
| `node-session-state` · `node --test` | `reports-dne` | same. Captured: both 0. |
| `pytest` · `uv run pytest` | `reports-dne` | pytest's summary line reports `skipped` separately. Captured: neither suite skipped. |
| `manual` · watched GUI | `unverified` | A human writes PASS/FAIL prose into `spikes/SPIKE-RESULTS.md` by hand. Nothing distinguishes "ran and passed" from "was not reached" except the author's discipline. |

**`manual-baseline: partial.`** One of the twelve manual rows (Gate 5 container teardown) carries a
recorded pass at `2026-08-10 @ 725b630`. Ten have never carried a result. One carries a FAIL that was
fixed in `db74a47` and has never been re-run in a watched window.

### Scope carve-out — the app-plugin runtime

`cockpit/ui/src-tauri/src/plugins/**` (`manager.rs`, `state.rs`, `manifest.rs`, `discovery.rs`,
`seams.rs`, `seams_impl.rs`, `mod.rs`) was **deliberately not scanned** on this run. Targeted tests for
the `plugin_launch` main-thread-blocking defect and the `stop_all_owned` / container-teardown
lifecycle (Gate 5) were being written by another agent while this plan was produced — the working tree
showed `M .../plugins/manager.rs` and a new `cockpit/ui/src-tauri/tests/tauri_command_threading.rs`.
Those two areas are **in progress, covered separately** and are recorded here only as context, never
as gaps to act on: entries `GAP-005` (1.5 AUDIENCE activation) and `GAP-009`/`GAP-010` (Gate 5) carry
`ratification_pending` for that reason and are excluded from the ranked index's call to action.

Neighbouring code that is *not* carved out was scanned normally — `embedding.rs`, `view_plugins.rs`,
`sidecar.rs`, `dashboard.rs`, `local_projects.rs`, `lib.rs` and the whole UI side all appear below.
Note that the new `tests/tauri_command_threading.rs` guard is a **signature-level** ratchet: it
inspects whether a `#[tauri::command]` is declared `async`, so blocking work one level down inside a
callee is invisible to it (see `GAP` entries for `WebviewPool::touch_and_evict` and `run_halyard`).

## 3. Needs ratification

Durable — carried across runs until a human answers. Nothing below has been applied.

| # | Awaiting | Detail |
|---|---|---|
| R1 | `spine_weight` | The whole table in the front matter is a first-run **proposal**. It is the Impact axis of every score in this file. Confirm or amend before the next run treats it as ratified. |
| R2 | `core_entry_points` | Four roots proposed. `plugins/session-state/**` and `tools/**` are reachable from **none** of them — their real entry points are the Claude Code hook contract (`hooks.json`, the `.ps1` wrappers) and the `/save-state` skill. Ratify whether those count as additional roots. |
| R3 | Phase-3 dispatch shape | On this bootstrap run the scanners returned ~200 candidates. Risk-tiered solo refutation of every one was not affordable, so refuters were dispatched **grouped by module** over the highest-risk and live-defect-asserting claims; the remainder are written `(unverified)`. Ratify this as the standing policy for large bootstrap runs, or require a second pass. |
| R4 | Manual checklist home | `spikes/SPIKE-RESULTS.md` was seeded as the single manual-QA source. `docs/handoff/2026-06-24-human-gated-spikes-runbook.md` and `2026-06-25-spikes-handoff.md` also contain human-gated procedures but are marked SUPERSEDED. Confirm SPIKE-RESULTS is canonical, or name a real checklist file. |
| R5 | Carve-out release | `GAP-005`, `GAP-009`, `GAP-010` are parked as "covered separately". Release them back into the ranking once the concurrent app-plugin-runtime test work lands, or mark them `covered` with the covering test. |
| R6 | Tier `in_ci` key | `tier_map` here carries a non-standard `in_ci` boolean per tier. It is the single most load-bearing fact this plan discovered and there was nowhere else in the schema to put it. Ratify the key or move it. |

## 4. Index

Machine-owned and fully regenerated each run. Ranks are re-densified `1..N` every time;
tie-break is `(impact desc, likelihood desc, GAP id asc)`.

### 4.1 Ranked open gaps

| rank | id | risk | title |
|---:|---|---:|---|
| 1 | `GAP-006` | **25** (L5×I5) | Smoke 1.6: native webview stays glued to its rect on resize (manual) |
| 2 | `GAP-008` | **25** (L5×I5) | Smoke 1.8: no leak or orphaned webview when switching away and back (manual) |
| 3 | `GAP-010` | **25** (L5×I5) | Smoke 1.9b: the app process survives window close (manual) — PARKED, undiagnosed |
| 4 | `GAP-013` | **25** (L5×I5) | Overlay input-block over a LIVE view-plugin iframe is unverified (new manual row) |
| 5 | `GAP-014` | **25** (L5×I5) | Rect glue under DPI, monitor, and window-move changes (new manual row) |
| 6 | `GAP-015` | **25** (L5×I5) | Cockpit behaviour after fleetd restarts or the socket drops (new manual row) |
| 7 | `GAP-017` | **25** (L5×I5) | `agent_exec` awaits the agent with no timeout and no cancellation |
| 8 | `GAP-033` | **25** (L5×I5) | Driver-plus-real-Docker resume has never been verified by machine or human |
| 9 | `GAP-002` | **20** (L4×I5) | Smoke 1.2: Fleet ops-grid regression canary (manual) |
| 10 | `GAP-007` | **20** (L4×I5) | Smoke 1.7: native webview parks off-screen while a host overlay is open (manual) |
| 11 | `GAP-011` | **20** (L4×I5) | Smoke 1.10: Vite HMR still works under the host CSP (manual) |
| 12 | `GAP-012` | **20** (L4×I5) | Smoke Part 2: the packaged build has never been launched (manual) |
| 13 | `GAP-018` | **20** (L4×I5) | `Runner::health` is implemented twice and called from nowhere, so `Trigger::Stall` is unreachable |
| 14 | `GAP-019` | **20** (L4×I5) | Resumed T2/T3: rejecting the oracle is a silent no-op |
| 15 | `GAP-021` | **20** (L4×I5) | A successful T3 ship orphans the unit's named volume |
| 16 | `GAP-022` | **20** (L4×I5) | `poll_mergeability` fires ten `gh` calls back to back with no delay |
| 17 | `GAP-023` | **20** (L4×I5) | The whole host-side git/GitHub failure surface is unexecuted because `FakeForge` cannot fail |
| 18 | `GAP-024` | **20** (L4×I5) | Every command-validity decision is written out four times |
| 19 | `GAP-057` | **20** (L4×I5) | The Tauri host crate is a standalone workspace, so CI never runs one of its tests |
| 20 | `GAP-058` | **20** (L4×I5) | The sidecar supervisor's restart loop has no test, no attempt cap, and no deadline |
| 21 | `GAP-059` | **20** (L4×I5) | `health_gate` does not restart on timeout, contradicting its own doc, and wedges the app in Starting |
| 22 | `GAP-062` | **20** (L4×I5) | `view_plugins::respond` is the only guard between plugin URLs and `fs::read`, with 14 untested branches |
| 23 | `GAP-063` | **20** (L4×I5) | Dev/packaged plugin-root precedence is the seam every remaining smoke row stands on, untested |
| 24 | `GAP-064` | **20** (L4×I5) | `WebviewPool::touch_and_evict` is the whole "no leak on switch" guarantee and is pure arithmetic nobody tests |
| 25 | `GAP-065` | **20** (L4×I5) | The `app::<id>` webview-label scheme is encoded in three places with a "MUST" nobody enforces |
| 26 | `GAP-066` | **20** (L4×I5) | The `ccplugin://` origin is written three ways, and the CSP form is Windows-only |
| 27 | `GAP-067` | **20** (L4×I5) | `127.0.0.1:8787` is hand-mirrored in four places and only one of them honours `CC_ADDR` |
| 28 | `GAP-068` | **20** (L4×I5) | The updater is registered against an empty pubkey and a `.example` endpoint |
| 29 | `GAP-069` | **20** (L4×I5) | `lib.rs:run`'s ExitRequested ordering is load-bearing and enforced only by statement order |
| 30 | `GAP-078` | **20** (L4×I5) | `api.ts` has no test file at all, and `openStream` wires no close or error handler |
| 31 | `GAP-081` | **20** (L4×I5) | `phaseClass` and `progress` drive every tile's colour and rail and have no direct assertion |
| 32 | `GAP-111` | **20** (L4×I5) | `npm run check` (353 files) is not in CI, and two source files are typechecked by nothing |
| 33 | `GAP-113` | **20** (L4×I5) | No lint, format, or static-analysis gate exists anywhere |
| 34 | `GAP-122` | **20** (L4×I5) | The embargo guard's `--all` mode, its skip paths, and its only write path are untested |
| 35 | `GAP-123` | **20** (L4×I5) | The git hooks and CI's inline commit-message range are shell nothing executes |
| 36 | `GAP-016` | **20** (L5×I4) | Starting a real mission with Docker stopped or the agent image absent (new manual row) |
| 37 | `GAP-043` | **20** (L5×I4) | No `busy_timeout` anywhere: a second writer loses events silently |
| 38 | `GAP-044` | **20** (L5×I4) | Every driver event does two synchronous SQLite writes inside a global mutex on a tokio worker |
| 39 | `GAP-045` | **20** (L5×I4) | `events_since` replays from 0 with no retention, pagination, or bound |
| 40 | `GAP-049` | **20** (L5×I4) | `router()` mounts nine routes with no auth, no origin check, and no CORS layer |
| 41 | `GAP-092` | **20** (L5×I4) | The hostile-plugin kill-and-revert path is covered by neither a test nor a checklist row |
| 42 | `GAP-119` | **20** (L5×I4) | Failed and halted units keep their volumes forever, and nothing has ever looked |
| 43 | `GAP-047` | **16** (L4×I4) | `env_f64` accepts a zero cap, bricking every mission with a 429 |
| 44 | `GAP-048` | **16** (L4×I4) | `stream_to_socket` drops events permanently on broadcast lag and never notices a dead peer |
| 45 | `GAP-050` | **16** (L4×I4) | `post_command` is the entire inbound control surface and no test drives it over HTTP |
| 46 | `GAP-053` | **16** (L4×I4) | `spawn_driver_for` and `rehydrate` duplicate the real-mode construction and both dispatch on `_` |
| 47 | `GAP-055` | **16** (L4×I4) | `bin/serve.rs:main` has no tests, no graceful shutdown, and panics on a held port |
| 48 | `GAP-056` | **16** (L4×I4) | `get_swarm` computes the swarm "done" verdict at read time with no test and no consumer |
| 49 | `GAP-087` | **16** (L4×I4) | `policeCommand`'s only numeric bound, `min_review_rounds`, is untested |
| 50 | `GAP-088` | **16** (L4×I4) | `loader.ts`'s traversal guard has one test case and disagrees with the Rust guard |
| 51 | `GAP-089` | **16** (L4×I4) | The shipped SDK has no lifetime story: unsubscribes untested, no `close()`, and a killed session hangs every pending promise |
| 52 | `GAP-090` | **16** (L4×I4) | The reference plugin's `esc`/`render` are structurally untestable, and `esc` guards `innerHTML` |
| 53 | `GAP-116` | **16** (L4×I4) | `FakeRunner` cannot fail, so the Docker error arms are unreachable from CI |
| 54 | `GAP-117` | **16** (L4×I4) | `trial_merge`'s Conflict half and cleanup are testable with git alone and are tested nowhere |
| 55 | `GAP-118` | **16** (L4×I4) | `local_docker`'s pure validators and exit-code mappings are untested, and two write paths swallow failure |
| 56 | `GAP-001` | **15** (L3×I5) | Smoke 1.1: switcher shows all four destinations (manual) |
| 57 | `GAP-005` | **15** (L3×I5) | Smoke 1.5: AUDIENCE app-plugin activation stays responsive (manual) — PARKED, covered separately |
| 58 | `GAP-009` | **15** (L3×I5) | Smoke 1.9a: Gate 5 container teardown on quit (manual) — PARKED, covered separately |
| 59 | `GAP-020` | **15** (L3×I5) | A `Ship` delivered to a `Halted` unit destroys it |
| 60 | `GAP-025` | **15** (L3×I5) | The red-checks feedback loop has no test and no iteration ceiling |
| 61 | `GAP-026` | **15** (L3×I5) | The wall-clock cap's driver-side use is dead code in the entire suite |
| 62 | `GAP-027` | **15** (L3×I5) | `fail_closed` bypasses the state machine and is never executed |
| 63 | `GAP-028` | **15** (L3×I5) | `ClaudePlanner::plan` spawns a real `claude` process with no timeout and no test |
| 64 | `GAP-029` | **15** (L3×I5) | The USD-cap check is copy-pasted at five call sites, three of them unexercised |
| 65 | `GAP-030` | **15** (L3×I5) | `steps::build` and `steps::review` are untested, and `review`'s prompt is half a contract |
| 66 | `GAP-032` | **15** (L3×I5) | `retry.rs:env_secs` and its three wrappers are untested |
| 67 | `GAP-060` | **15** (L3×I5) | `fleetd://status` is emitted to nobody |
| 68 | `GAP-061` | **15** (L3×I5) | The `ccplugin://` response headers are three load-bearing security invariants with no assertion |
| 69 | `GAP-070` | **15** (L3×I5) | `run_halyard` shells out with no timeout from a synchronous Tauri command |
| 70 | `GAP-071` | **15** (L3×I5) | The Audience HTTP commands have no client timeout and an unasserted error-policy asymmetry |
| 71 | `GAP-073` | **15** (L3×I5) | `App.svelte`'s app-plugin compositing effect never exercises the overlay park/restore pair |
| 72 | `GAP-074` | **15** (L3×I5) | The ResizeObserver rect-glue effect is asserted nowhere, teardown included |
| 73 | `GAP-075` | **15** (L3×I5) | No test in the repo ever mounts a view-plugin iframe from `App.svelte` |
| 74 | `GAP-076` | **15** (L3×I5) | `onKill` — the plugin-misbehaviour escape hatch — has no App-level test |
| 75 | `GAP-077` | **15** (L3×I5) | `selectApp` has no in-flight guard, so a double-click starts two docker builds |
| 76 | `GAP-079` | **15** (L3×I5) | `FleetStore.dispose` leaves the store un-restartable |
| 77 | `GAP-080` | **15** (L3×I5) | `fleet.ts:fold` matches Rust-side reason strings by exact equality, with no test on either side |
| 78 | `GAP-082` | **15** (L3×I5) | Phase-eligibility policy for the action buttons lives in three places with no assertion on any |
| 79 | `GAP-110` | **15** (L3×I5) | `npm test` — 135 tests over the whole cockpit UI — is not in CI |
| 80 | `GAP-112` | **15** (L3×I5) | Three whole test suites outside the Rust workspace are invoked by no gate |
| 81 | `GAP-114` | **15** (L3×I5) | `release.yml` signs and publishes without running a single test |
| 82 | `GAP-115` | **15** (L3×I5) | The Docker integration tests run nowhere, on any schedule |
| 83 | `GAP-121` | **15** (L3×I5) | The load-bearing sidecar-before-bundle order is written four times and CI does not reuse it |
| 84 | `GAP-093` | **15** (L5×I3) | The dashboard's local scan root is a hardcoded developer drive letter |
| 85 | `GAP-041` | **12** (L3×I4) | `Store::open` is the only constructor any real process uses and no test calls it |
| 86 | `GAP-042` | **12** (L3×I4) | `Store::init`'s migration ALTERs swallow every error, so a failed upgrade reads as an empty fleet |
| 87 | `GAP-046` | **12** (L3×I4) | `docker_ok` has no timeout and no single-flight guard, and `create_swarm` awaits it in-handler |
| 88 | `GAP-051` | **12** (L3×I4) | The `/units`, `/health` and `/units/:id` JSON shapes are a hand-mirrored contract nothing gates |
| 89 | `GAP-052` | **12** (L3×I4) | `create_mission` and `create_swarm`'s real-mode money guards are unexecuted |
| 90 | `GAP-054` | **12** (L3×I4) | `spawn_forwarder` discards store write errors, then broadcasts the event as if durable |
| 91 | `GAP-083` | **12** (L3×I4) | Capabilities are negotiated, thrown away, and never enforced |
| 92 | `GAP-084` | **12** (L3×I4) | The bridge's rate/flood buckets are never driven end to end |
| 93 | `GAP-085` | **12** (L3×I4) | The shipped `autoTick: true` default path executes in zero tests |
| 94 | `GAP-086` | **12** (L3×I4) | Hostile-input handling on the port is exercised only through pure-function tests |
| 95 | `GAP-091` | **12** (L3×I4) | The host duplicates the reference manifest inline, so the loader's tested code paths are unreachable in the app |
| 96 | `GAP-120` | **12** (L3×I4) | The fake and the real runner disagree on what a valid `UnitSpec` is |
| 97 | `GAP-039` | **12** (L4×I3) | `Provisioning` is excluded from `is_agent_active`, so nothing bounds a hung provision |
| 98 | `GAP-097` | **12** (L4×I3) | Both dashboard adapters map an unrecognised upstream state to a confident "Idle" |
| 99 | `GAP-101` | **12** (L4×I3) | `model.ts:isOffPipeline` is exported, unreferenced, untested — and `sortedCards` re-derives it inline |
| 100 | `GAP-124` | **12** (L4×I3) | `demo-restart-recovery.mjs` is the only end-to-end durability check and nothing runs it |
| 101 | `GAP-125` | **12** (L4×I3) | `index.html` loads Google Fonts against a CSP that has no `font-src` and no such origin |
| 102 | `GAP-031` | **10** (L2×I5) | `reconcile` and `reconcile_live` re-derive the same decision independently |
| 103 | `GAP-072` | **10** (L2×I5) | `local_projects`' exclusion list and depth bound are the only brakes on a whole-disk walk, and neither is tested |
| 104 | `GAP-040` | **9** (L3×I3) | `Phase::is_interruptible` is exported, uncalled, untested, and duplicated inline |
| 105 | `GAP-094` | **9** (L3×I3) | `Dashboard.svelte`'s entire live-wiring path is unexecuted while looking well tested |
| 106 | `GAP-095` | **9** (L3×I3) | Every dashboard adapter's degradation contract is unenforced at its edges |
| 107 | `GAP-096` | **9** (L3×I3) | App-scoped Halyard proposals are written into the map under a key nothing reads |
| 108 | `GAP-098` | **9** (L3×I3) | `dashboard/api.ts` has no test file and is the only place the four IPC command names appear |
| 109 | `GAP-099` | **9** (L3×I3) | The dashboard's user-facing affordances — deep links, chips, footers, empty state — are asserted nowhere |
| 110 | `GAP-100` | **9** (L3×I3) | App-plugin cards can never reach the dashboard board because the prop is never passed |
| 111 | `GAP-102` | **9** (L3×I3) | The dashboard's "source unreachable" card is hand-copied three times with divergent fields |
| 112 | `GAP-003` | **8** (L2×I4) | Smoke 1.3: REFERENCE view-plugin renders, handshakes, and cannot reach the network (manual) |
| 113 | `GAP-004` | **8** (L2×I4) | Smoke 1.4: command policy round-trip and command-ack rejection (manual) |
| 114 | `GAP-103` | **8** (L4×I2) | The session-state SessionEnd hook has never been observed firing |
| 115 | `GAP-104` | **8** (L4×I2) | The Stop hook spawns eight sequential git subprocesses against a 5-second budget |
| 116 | `GAP-106` | **8** (L4×I2) | `withLock` steals a lock from a demonstrably live holder, and `sleep` busy-spins |
| 117 | `GAP-126` | **8** (L4×I2) | The PowerShell hooks Claude Code actually executes are tested by nothing, in any repo language |
| 118 | `GAP-127` | **8** (L4×I2) | `deploy_globals.py` mutates the user's real `settings.json` and `CLAUDE.md` with no tests at all |
| 119 | `GAP-128` | **8** (L4×I2) | `context-offload` has no test infrastructure, and its update path corrupts on any Windows path |
| 120 | `GAP-129` | **8** (L4×I2) | cache-countdown's headline feature is inert, and its self-test cannot fail |
| 121 | `GAP-131` | **8** (L4×I2) | Both `install.ps1` scripts are the same 85 lines twice, with an untested `Copy-Item` nesting hazard |
| 122 | `GAP-034` | **6** (L2×I3) | `gate_met`'s anti-oscillation conjunct is unreachable dead logic |
| 123 | `GAP-035` | **6** (L2×I3) | `Event`'s wire shape is the cockpit's contract and seven of ten variants are unasserted |
| 124 | `GAP-036` | **6** (L2×I3) | The snake_case phase vocabulary is hand-duplicated across Rust, SQL and TypeScript |
| 125 | `GAP-037` | **6** (L2×I3) | `Command::to_trigger` has no production caller while the driver re-implements it twice |
| 126 | `GAP-038` | **6** (L2×I3) | `OracleTampering`'s transition arm is the trust gate and has no direct test |
| 127 | `GAP-105` | **6** (L3×I2) | `capture_end` drops a timeline record and deletes the only backup in the same breath |
| 128 | `GAP-107` | **6** (L3×I2) | Two `repoRoot` spawns per hook, and a torn `git status` renders as a real branch called `null` |
| 129 | `GAP-108` | **6** (L3×I2) | The session-state hook contract is validated only for file existence |
| 130 | `GAP-109` | **6** (L3×I2) | `capture_rich`'s four failure arms are the plugin's only user-visible errors and none is tested |
| 131 | `GAP-130` | **6** (L3×I2) | Both Python tools' console-script entry points are the untested side of the process boundary |

### 4.2 By status

| status | count | ids |
|---|---:|---|
| `open` | 131 | all entries |

_Three entries are parked as **covered separately** pending the concurrent app-plugin-runtime
test work and are excluded from the call to action even though they appear in the ranking above:
`GAP-005`, `GAP-009`, `GAP-010` (see §2 and R5)._

## 5. Entries

One flat list, `GAP-###` ascending, append-only. **Status is a field, not a section.** Ordering and
grouping live in the Index (§4), never here.

### GAP-001 — Smoke 1.1: switcher shows all four destinations (manual)

| field | value |
|---|---|
| **status** | `open` <!-- human --> |
| **claim_type** | `manual-unverified` |
| **layer** | manual |
| **verified_by** | manual |
| **anchors** | `spikes/SPIKE-RESULTS.md#smoke-run-1--2026-08-10`, `1.1` |
| **governs** | `cockpit/ui/src/App.svelte`, `cockpit/ui/src/lib/Switcher.svelte` |
| **last_manual_pass** | — (never_verified) |
| **risk** | L3 × I5 = **15** |
| **observations** | manual_coverage_pts=3, churn_90d=8 (churn_pts=4), never_verified=true |
| **anchor_sites** | 1 |
| **first_seen** | 2026-08-10 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** The row asks a human to eyeball that FLEET + PROJECTS + REFERENCE + AUDIENCE all
appear. `Switcher.test.ts` tests the presentational component against hand-written props; App's
*composition* of host views + discovered view-plugins + `plugins_list` app-plugins into that prop is
asserted nowhere, and neither is `activeSwitcherId`'s nested-ternary precedence. Scored 3 rather than
5 because the leaf component is genuinely covered.

**Concrete test.** **Automatable.** A jsdom render of `App` asserting the four
`data-testid` tabs in order with their labels, plus the aria-pressed precedence, replaces everything
this row asks a human to look at except "the segmented control looks right".

### GAP-002 — Smoke 1.2: Fleet ops-grid regression canary (manual)

| field | value |
|---|---|
| **status** | `open` <!-- human --> |
| **claim_type** | `manual-unverified` |
| **layer** | manual |
| **verified_by** | manual |
| **anchors** | `spikes/SPIKE-RESULTS.md#smoke-run-1--2026-08-10`, `1.2` |
| **governs** | `cockpit/ui/src/App.svelte`, `cockpit/ui/src/lib/fleet.ts`, `cockpit/ui/src/lib/store.svelte.ts` |
| **last_manual_pass** | — (never_verified) |
| **risk** | L4 × I5 = **20** |
| **observations** | manual_coverage_pts=4, churn_90d=8 (churn_pts=4), never_verified=true |
| **anchor_sites** | 1 |
| **first_seen** | 2026-08-10 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** This is the "did bolting the plugin runtime on break the thing the product is
for" canary, and it is the largest unasserted surface in the UI: not one test in `src/` renders or
inspects the ops grid. `App.overlay.test.ts` seeds a unit only to pop a modal and asserts nothing
about tiles; the grid markup carries no `data-testid` at all.

**Concrete test.** **Automatable.** Seed three units with distinct phases, assert one
tile each plus the ACTIVE/UNITS/BURN stat values, then switch away and back and assert the same tiles
return with selection intact — which also covers the automatable core of row 1.8.

### GAP-003 — Smoke 1.3: REFERENCE view-plugin renders, handshakes, and cannot reach the network (manual)

| field | value |
|---|---|
| **status** | `open` <!-- human --> |
| **claim_type** | `manual-unverified` |
| **layer** | manual |
| **verified_by** | manual |
| **anchors** | `spikes/SPIKE-RESULTS.md#smoke-run-1--2026-08-10`, `1.3` |
| **governs** | `cockpit/ui/src/lib/bridge.ts`, `cockpit/ui/src/lib/loader.ts`, `cockpit/plugin-sdk/**`, `plugins/reference/**`, `cockpit/ui/src-tauri/src/view_plugins.rs` |
| **last_manual_pass** | — (never_verified) |
| **risk** | L2 × I4 = **8** |
| **observations** | manual_coverage_pts=2, churn_90d=1 (churn_pts=2), never_verified=true |
| **anchor_sites** | 1 |
| **first_seen** | 2026-08-10 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** Scores low *because it is the best-defended manual row*: `bridge.test.ts` drives
the `plugin-hello`→`ready`→full-snapshot handshake 100× over a real `MessageChannel` with zero drops.
What stays genuinely human is the part jsdom cannot model — that a `sandbox="allow-scripts"` iframe
yields an opaque origin, that `connect-src 'none'` really blocks network, and that the CORS/module
fetch of `sdk.js` succeeds under the real `ccplugin://` handler.

**Concrete test.** **Partially automatable — see `GAP-075` and `GAP-086`.** The iframe-mount,
`sandbox` attribute, bridge construction and destroy-on-switch are jsdom-assertable; the opaque-origin
and CSP halves are not and must stay in this row.

### GAP-004 — Smoke 1.4: command policy round-trip and command-ack rejection (manual)

| field | value |
|---|---|
| **status** | `open` <!-- human --> |
| **claim_type** | `manual-unverified` |
| **layer** | manual |
| **verified_by** | manual |
| **anchors** | `spikes/SPIKE-RESULTS.md#smoke-run-1--2026-08-10`, `1.4` |
| **governs** | `cockpit/ui/src/lib/bridge.ts`, `cockpit/plugin-sdk/index.js` |
| **last_manual_pass** | — (never_verified) |
| **risk** | L2 × I4 = **8** |
| **observations** | manual_coverage_pts=2, churn_90d=1 (churn_pts=2), never_verified=true |
| **anchor_sites** | 1 |
| **first_seen** | 2026-08-10 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** `policeCommand` has eight direct tests, so the happy round-trip and the
`real-requires-confirm` rejection are already pinned. The residual holes are the ones no human step
would find either: the rate/flood buckets are never driven end to end, the `sink-error` ack arms are
dead in the suite, and a version-skewed message is dropped silently with no ack.

**Concrete test.** **Fully automatable — see `GAP-084`, `GAP-086`, `GAP-087`, `GAP-089`.** Everything
this row checks happens over a `MessageChannel`, which jsdom provides natively. This row can be
retired from the human gate once those land.

### GAP-005 — Smoke 1.5: AUDIENCE app-plugin activation stays responsive (manual) — PARKED, covered separately

| field | value |
|---|---|
| **status** | `open` <!-- human --> |
| **claim_type** | `manual-unverified` |
| **layer** | manual |
| **verified_by** | manual |
| **anchors** | `spikes/SPIKE-RESULTS.md#smoke-run-1--2026-08-10`, `1.5` |
| **governs** | `cockpit/ui/src-tauri/src/plugins/**`, `cockpit/ui/src/App.svelte` |
| **last_manual_pass** | — (FAIL 2026-08-10 @ `725b630`; fixed in `db74a47`, never re-run) |
| **risk** | L3 × I5 = **15** |
| **observations** | manual_coverage_pts=2, churn_90d=8 (churn_pts=4), never_verified=true |
| **anchor_sites** | 1 |
| **first_seen** | 2026-08-10 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **ratification_pending** | `accepted` — in progress, covered separately (see §2 carve-out and R5) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** The row that earned the whole smoke its keep: clicking AUDIENCE froze the UI
because `plugin_launch` was a synchronous `#[tauri::command]` running `docker compose build` on the
main event-loop thread. Root-caused and fixed in `db74a47`, pinned by two tests in
`src/App.appPlugin.test.ts` — but **verified only by automated gates, never in a watched window**.
Excluded from this plan's call to action: another agent is writing targeted tests for exactly this
defect concurrently.

**Concrete test.** Owned elsewhere. The residual human step after that work is narrow: watch the chip
walk `starting → health-probing → healthy` with the window responsive throughout.

### GAP-006 — Smoke 1.6: native webview stays glued to its rect on resize (manual)

| field | value |
|---|---|
| **status** | `open` <!-- human --> |
| **claim_type** | `manual-unverified` |
| **layer** | manual |
| **verified_by** | manual |
| **anchors** | `spikes/SPIKE-RESULTS.md#smoke-run-1--2026-08-10`, `1.6` |
| **governs** | `cockpit/ui/src/App.svelte`, `cockpit/ui/src-tauri/src/embedding.rs` |
| **last_manual_pass** | — (never_verified) |
| **risk** | L5 × I5 = **25** |
| **observations** | manual_coverage_pts=5, churn_90d=8 (churn_pts=4), never_verified=true |
| **anchor_sites** | 1 |
| **first_seen** | 2026-08-10 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** Nothing in the suite asserts one byte of the rect-glue effect —
`App.appPlugin.test.ts` explicitly stubs `ResizeObserver` to a no-op and its own comment defers this
to "smoke checklist 1.6". `embedding.rs` has zero tests. So the highest-churn interactive surface in
the app is defended by a checklist row that has never been run.

**Concrete test.** **Two thirds automatable — see `GAP-074`.** That an observer is attached to the
reserved rect, that the callback marshals a well-formed four-key rect through `toRect`, and that the
observer is disconnected on switch-away are all jsdom-assertable. Only the geometric truth needs a
real window — and see `GAP-014` for the DPI/multi-monitor case this row does *not* cover.

### GAP-007 — Smoke 1.7: native webview parks off-screen while a host overlay is open (manual)

| field | value |
|---|---|
| **status** | `open` <!-- human --> |
| **claim_type** | `manual-unverified` |
| **layer** | manual |
| **verified_by** | manual |
| **anchors** | `spikes/SPIKE-RESULTS.md#smoke-run-1--2026-08-10`, `1.7` |
| **governs** | `cockpit/ui/src/App.svelte`, `cockpit/ui/src-tauri/src/embedding.rs` |
| **last_manual_pass** | — (never_verified) |
| **risk** | L4 × I5 = **20** |
| **observations** | manual_coverage_pts=4, churn_90d=8 (churn_pts=4), never_verified=true |
| **anchor_sites** | 1 |
| **first_seen** | 2026-08-10 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** `z-index` and `inert` cannot cross the native-webview boundary, so parking is the
*only* mechanism keeping an app-plugin from painting over a REAL-launch confirm dialog. The two
existing app-plugin tests cover only the healthy/not-healthy compositing gate; neither ever opens an
overlay, so the entire park/restore branch pair is dead in the suite.

**Concrete test.** **Signal half automatable — see `GAP-073`.** Assert `plugin_hide` fires exactly
once on overlay open and `plugin_show` on close, with no re-issue on an unrelated state write. The
residual human step shrinks to confirming the pixels once. See also `GAP-013`, an input-block case
over a *view*-plugin that this row does not cover at all.

### GAP-008 — Smoke 1.8: no leak or orphaned webview when switching away and back (manual)

| field | value |
|---|---|
| **status** | `open` <!-- human --> |
| **claim_type** | `manual-unverified` |
| **layer** | manual |
| **verified_by** | manual |
| **anchors** | `spikes/SPIKE-RESULTS.md#smoke-run-1--2026-08-10`, `1.8` |
| **governs** | `cockpit/ui/src/App.svelte`, `cockpit/ui/src/lib/bridge.ts`, `cockpit/ui/src-tauri/src/embedding.rs` |
| **last_manual_pass** | — (never_verified) |
| **risk** | L5 × I5 = **25** |
| **observations** | manual_coverage_pts=5, churn_90d=8 (churn_pts=4), never_verified=true |
| **anchor_sites** | 1 |
| **first_seen** | 2026-08-10 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** A leak is the one defect class a human watching a window *cannot* see. Every test
calls `bridge.destroy()`/`session.destroy()` as cleanup and none asserts anything about it; the
window `message` listener, the `setInterval` tick, the transferred port, the `ResizeObserver`, and
the `WebviewPool` LRU entry are all released only by teardown paths nothing checks.

**Concrete test.** **Largely automatable — see `GAP-074`, `GAP-075`, `GAP-064`, `GAP-085`.** Construct
and destroy fifty times and assert the live window-listener count, timer count, and LRU length are
unchanged. Only "Task Manager shows no orphaned webview process" stays human.

### GAP-009 — Smoke 1.9a: Gate 5 container teardown on quit (manual) — PARKED, covered separately

| field | value |
|---|---|
| **status** | `open` <!-- human --> |
| **claim_type** | `manual-unverified` |
| **layer** | manual |
| **verified_by** | manual |
| **anchors** | `spikes/SPIKE-RESULTS.md#smoke-run-1--2026-08-10`, `1.9a` |
| **governs** | `cockpit/ui/src-tauri/src/plugins/manager.rs`, `crates/fleetd/src/local_docker.rs` |
| **last_manual_pass** | 2026-08-10 @ `725b630` (PASS — `docker ps` empty against a verified 0-container baseline) |
| **risk** | L3 × I5 = **15** |
| **observations** | manual_coverage_pts=2, churn_90d=9 (churn_pts=4), never_verified=false |
| **anchor_sites** | 1 |
| **first_seen** | 2026-08-10 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **ratification_pending** | `accepted` — in progress, covered separately (see §2 carve-out and R5) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** The only manual row in the register carrying a real PASS. Excluded from this
plan's call to action — `stop_all_owned` / container teardown is being covered by targeted tests
concurrently. Recorded here so the ranking is honest about what is and is not already defended.

**Concrete test.** Owned elsewhere. One gap this row does *not* close, filed separately as `GAP-119`:
it checks `docker ps` only, never `docker volume ls`, and volumes are deliberately kept on teardown.

### GAP-010 — Smoke 1.9b: the app process survives window close (manual) — PARKED, undiagnosed

| field | value |
|---|---|
| **status** | `open` <!-- human --> |
| **claim_type** | `manual-unverified` |
| **layer** | manual |
| **verified_by** | manual |
| **anchors** | `spikes/SPIKE-RESULTS.md#smoke-run-1--2026-08-10`, `1.9b` |
| **governs** | `cockpit/ui/src-tauri/src/lib.rs`, `cockpit/ui/src-tauri/src/sidecar.rs` |
| **last_manual_pass** | — (ANOMALY 2026-08-10 @ `725b630`) |
| **risk** | L5 × I5 = **25** |
| **observations** | manual_coverage_pts=5, churn_90d=10 (churn_pts=4), never_verified=true |
| **anchor_sites** | 1 |
| **first_seen** | 2026-08-10 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **ratification_pending** | `accepted` — adjacent to the carve-out (see §2 and R5) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** Teardown demonstrably ran — the containers came down — but the `app` process
survived the window close (pid 13396, no window, 23 threads, still responding 15 s later, 41 MB).
Undiagnosed: dev-only artifact of `tauri dev` supervision, or a real shutdown defect. `lib.rs` has
zero tests and its `ExitRequested` handler's ordering (reap the sidecar *before* `stop_all_owned`, so
the supervisor cannot respawn it mid-teardown) is enforced by nothing but statement order.

**Concrete test.** **The ordering half is automatable — see `GAP-069`.** A source-structure guard
asserting `SidecarSupervisor::shutdown` precedes `stop_all_owned`, which precedes `app_handle.exit(0)`.
Whether the process actually exits stays human until someone diagnoses the anomaly.

### GAP-011 — Smoke 1.10: Vite HMR still works under the host CSP (manual)

| field | value |
|---|---|
| **status** | `open` <!-- human --> |
| **claim_type** | `manual-unverified` |
| **layer** | manual |
| **verified_by** | manual |
| **anchors** | `spikes/SPIKE-RESULTS.md#smoke-run-1--2026-08-10`, `1.10` |
| **governs** | `cockpit/ui/src-tauri/tauri.conf.json`, `cockpit/ui/vite.config.ts` |
| **last_manual_pass** | — (never_verified) |
| **risk** | L4 × I5 = **20** |
| **observations** | manual_coverage_pts=5, churn_90d=3 (churn_pts=3), never_verified=true |
| **anchor_sites** | 1 |
| **first_seen** | 2026-08-10 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** Nothing in the repo reads `tauri.conf.json`'s CSP and checks it against what the
app actually loads — not the 19-file vitest suite, not either workflow. Two concrete drifts are
already visible and unguarded (`GAP-066` frame-src origin form, `GAP-125` index.html loading Google
Fonts against a `style-src 'self'` policy). HMR is only the most visible symptom of that class.

**Concrete test.** **The static half is automatable — see `GAP-066`.** Parse `tauri.conf.json` and
assert every origin any code path can request is admitted by the directive that governs it. Whether
the WebView2 HMR socket actually connects stays human.

### GAP-012 — Smoke Part 2: the packaged build has never been launched (manual)

| field | value |
|---|---|
| **status** | `open` <!-- human --> |
| **claim_type** | `manual-unverified` |
| **layer** | manual |
| **verified_by** | manual |
| **anchors** | `spikes/SPIKE-RESULTS.md#remaining-human-gate--interactive-dev--packaged-smoke-not-run-headlessly`, `Part 2` |
| **governs** | `cockpit/ui/src-tauri/tauri.conf.json`, `.github/workflows/ci.yml`, `.github/workflows/release.yml`, `cockpit/ui/scripts/build-sidecar.mjs` |
| **last_manual_pass** | — (never_verified) |
| **risk** | L4 × I5 = **20** |
| **observations** | manual_coverage_pts=5, churn_90d=4 (churn_pts=3), never_verified=true |
| **anchor_sites** | 1 |
| **first_seen** | 2026-07-17 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** CI produces `.msi`/`.exe`/`.dmg`/`.app`/`.deb`/`.AppImage` on three OSes with
`if-no-files-found: warn` — so a bundle producing zero files does not even fail the job — and never
launches one. `release.yml` signs and publishes without launching one either. **No packaged build of
this app has ever been confirmed to start.** Every packaged-only concern is unexercised by every
automated gate: the `ccplugin://` scheme without a dev server, `externalBin` resolution, the updater.

**Concrete test.** **Partially automatable — see `GAP-121`.** A CI step that unpacks the produced
bundle and asserts `binaries/fleetd-serve*` is inside it and answers `--version` would catch the
build-order class cheaply. Launching the GUI stays human, but it should become a per-release row that
must be PASS before a draft release is published.

### GAP-013 — Overlay input-block over a LIVE view-plugin iframe is unverified (new manual row)

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `manual-uncovered` |
| **layer** | manual |
| **verified_by** | manual |
| **anchors** | `cockpit/ui/src/App.svelte:overlayOpen` |
| **governs** | `cockpit/ui/src/App.svelte`, `cockpit/ui/src/lib/ApprovalOverlay.svelte` |
| **last_manual_pass** | — (never_verified) |
| **risk** | L5 × I5 = **25** |
| **observations** | manual_coverage_pts=5, churn_90d=8 (churn_pts=4), never_verified=true |
| **anchor_sites** | 1 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** `App.svelte`'s own comment asserts that `inert` on the content subtree is
"honored by WebView2/Chromium" and is "the real input block, not the backdrop" — a claim about a
native runtime that nobody has verified. jsdom implements no `inert` semantics at all: it reports the
property as `true` while delivering every event, so `App.overlay.test.ts`'s assertion is a spelling
check on an attribute, not evidence of a block. No existing row covers it — 1.7 is about parking a
*native* webview, an entirely different mechanism.

**Concrete test.** Not automatable in this harness. With REFERENCE live and focused, stage a REAL
launch and confirm keystrokes and clicks no longer reach the iframe, Tab cannot move focus into it,
the plugin cannot `.focus()` its way back, and Enter/Escape still hit the modal. Repeat packaged.

### GAP-014 — Rect glue under DPI, monitor, and window-move changes (new manual row)

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `manual-uncovered` |
| **layer** | manual |
| **verified_by** | manual |
| **anchors** | `cockpit/ui/src/App.svelte:$effect#rect-glue-resizeobserver` |
| **governs** | `cockpit/ui/src/App.svelte`, `cockpit/ui/src-tauri/src/embedding.rs` |
| **last_manual_pass** | — (never_verified) |
| **risk** | L5 × I5 = **25** |
| **observations** | manual_coverage_pts=5, churn_90d=8 (churn_pts=4), never_verified=true |
| **anchor_sites** | 1 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** The glue is driven solely by a `ResizeObserver` on the reserved div, which fires
on element *size* changes. A window **move** produces no size change, and `plugin_show`/
`plugin_set_rect` take `LogicalPosition` — so a scale-factor change is the most likely break. Row 1.6
says only "resize the window" on a single display; mixed-DPI multi-monitor is the ordinary case on
the target machine and the failure is a plugin rendered halfway off the window with no error.

**Concrete test.** Not automatable — jsdom has no layout engine and `getBoundingClientRect` returns
zeros. Drag the window between monitors with different scale factors, change display scaling while
running, move without resizing, minimise/restore, maximise/unmaximise; after each confirm the child
webview is still exactly over the reserved rect.

### GAP-015 — Cockpit behaviour after fleetd restarts or the socket drops (new manual row)

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `manual-uncovered` |
| **layer** | manual |
| **verified_by** | manual |
| **anchors** | `cockpit/ui/src/lib/store.svelte.ts:start`, `crates/fleetd/src/server.rs:stream_to_socket` |
| **governs** | `cockpit/ui/src/lib/api.ts`, `cockpit/ui/src/lib/store.svelte.ts`, `crates/fleetd/src/server.rs`, `cockpit/ui/src-tauri/src/sidecar.rs` |
| **last_manual_pass** | — (never_verified) |
| **risk** | L5 × I5 = **25** |
| **observations** | manual_coverage_pts=5, churn_90d=26 (churn_pts=5), never_verified=true |
| **anchor_sites** | 2 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** Nothing — automated or manual — observes what the cockpit does after the daemon
goes away, and the sidecar supervisor restarts fleetd on every crash, so this is a normal path, not an
exotic one. `openStream` wires no `onclose`/`onerror`, `start()` latches `started = true`, and daemon
health is fetched only inside that single `reconnect()`. The result is a cockpit that looks completely
healthy and is completely dead: frozen phases, a **green** DOCKER badge from minutes ago, launch
buttons that 404. The stale-green header is the dangerous part — an affirmative false signal, not
merely a missing one. All twelve existing rows assume a live daemon for the whole run.

**Concrete test.** Launch a DEMO unit, confirm tiles are streaming, then kill and restart the fleetd
sidecar. Record whether tiles keep updating, whether the header badge goes stale-but-green, whether
there is any user-visible indication, and whether anything recovers without an app restart. The
jsdom-testable half is filed as `GAP-078`.

### GAP-016 — Starting a real mission with Docker stopped or the agent image absent (new manual row)

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `manual-uncovered` |
| **layer** | manual |
| **verified_by** | manual |
| **anchors** | `crates/fleetd/src/local_docker.rs:provision`, `crates/fleetd/src/server.rs:docker_ok` |
| **governs** | `crates/fleetd/src/local_docker.rs`, `crates/fleetd/src/server.rs` |
| **last_manual_pass** | — (never_verified) |
| **risk** | L5 × I4 = **20** |
| **observations** | manual_coverage_pts=5, churn_90d=26 (churn_pts=5), never_verified=true |
| **anchor_sites** | 2 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** The most common first-run user error has never been performed by anyone. Both
`#[ignore]`d Docker ITs assume a healthy daemon *and* a prebuilt `cc-agent:dev` image, CI has no
daemon at all, and no smoke row starts a mission. Worse, `docker_ok` has no timeout and no
single-flight guard, so a wedged Docker Desktop makes every `/health` poll spawn its own `docker
version` — a subprocess pileup under exactly the condition the probe exists to detect — while
`create_swarm` awaits it *inside* the request handler.

**Concrete test.** With Docker stopped, and separately with the image absent, dispatch a real mission
from the cockpit and record what the operator sees and how long it takes. Much of this is
reclassifiable to CI — see `GAP-116`, `GAP-046` and `GAP-118`.

### GAP-017 — `agent_exec` awaits the agent with no timeout and no cancellation

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `manual-uncovered` |
| **layer** | manual |
| **verified_by** | manual |
| **anchors** | `crates/fleetd/src/driver.rs:Run::agent_exec`, `crates/fleetd/src/steps.rs:check` |
| **governs** | `crates/fleetd/src/driver.rs`, `crates/fleetd/src/steps.rs`, `crates/fleetd/src/local_docker.rs` |
| **last_manual_pass** | — (never_verified) |
| **risk** | L5 × I5 = **25** |
| **observations** | manual_coverage_pts=5, churn_90d=19 (churn_pts=5), never_verified=true |
| **anchor_sites** | 2 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** `agent_exec` awaits `runner.exec(...)` with no timeout; `local_docker.rs:exec` is a
bare `docker exec` with no bound either. The only wall-clock guard runs at the **top of the `drive`
loop**, i.e. between phases, so it can never interrupt an exec already in flight. `steps::check` — the
project's own test command — gets no in-container `timeout` prefix at all. A hung agent or a test
suite that never returns pins the driver task forever while holding an `OwnedSemaphorePermit`,
silently consuming a fleet concurrency slot. `Runner::health` exists and would notice, but nothing
calls it (`GAP-018`). No checklist row covers a stalled exec.

**Concrete test.** `crates/fleetd/tests/exec_watchdog_it.rs`, fakes only so it runs in CI: a
`HangingRunner` whose `exec` sleeps 86 400 s, driven under `#[tokio::test(start_paused = true)]` with
`wall_clock_secs: 60`; assert `Blocked{cap: Some("wall_clock")}`, `NeedsHuman`, and that the semaphore
returns to full `available_permits()`.

### GAP-018 — `Runner::health` is implemented twice and called from nowhere, so `Trigger::Stall` is unreachable

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-symbol` |
| **layer** | rust-workspace |
| **verified_by** | automated |
| **anchors** | `crates/fleetd/src/runner.rs:Runner::health`, `crates/fleetd/src/local_docker.rs:health`, `crates/fleetd/src/fake.rs:health` |
| **risk** | L4 × I5 = **20** |
| **observations** | coverage_pts=5 (no test and no caller), branches=0 (branch_pts=1), churn_90d=9 (churn_pts=4) |
| **anchor_sites** | 3 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** **Phase-3 confirmed.** An exhaustive workspace search for `.health(` returns one
hit and it is unrelated TypeScript. `Liveness::Stalled` is constructed only inside
`LocalDockerRunner::health` and consumed nowhere; `Trigger::Stall` appears only at its declaration, in
the universal-interrupt match pattern, and in one `fleet-core` unit test — the daemon never constructs
it. So container liveness detection is **built but not wired**: a container that dies, OOMs, or is
`docker kill`ed mid-run is never noticed, and the `Stall → NeedsHuman` path is dead. The dead wiring
is itself the defect; the missing test is secondary.

**Concrete test.** Add `FakeRunner::stalled()` (the `health` field exists but has no builder, so
`Liveness::Stalled` is currently unconstructible from the fake), then a `driver.rs` test asserting a
unit whose container reports `Stalled` is routed off the agent-active phases. It will fail to observe
anything today, which is the point.

### GAP-019 — Resumed T2/T3: rejecting the oracle is a silent no-op

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-branch` |
| **layer** | rust-workspace |
| **verified_by** | automated |
| **anchors** | `crates/fleetd/src/driver.rs:Run::drive#awaiting-oracle-reject-on-resume`, `crates/fleetd/src/driver.rs:Run::drive#spec-oracle-frozen-guard` |
| **risk** | L4 × I5 = **20** |
| **observations** | coverage_pts=4, branches=5 (branch_pts=2), churn_90d=19 (churn_pts=5) |
| **anchor_sites** | 2 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** **Phase-3 confirmed.** `Phase::Spec` is guarded by `if self.resume &&
self.spec.oracle_frozen { goto(OracleFrozen); continue; }`, and neither flag is ever cleared — `resume`
appears once outside construction, and `oracle_frozen` is only ever latched *on* (the store even has a
test asserting `None` must not un-freeze it). `server.rs:rehydrate` sets `resume: true` for every
restart-recovered unit. So a human clicking REJECT on a resumed T2/T3 unit drives
`Spec → OracleFrozen → AwaitingOracleApproval` with no oracle re-run and is asked the identical
question forever. A human-in-the-loop gate that silently does nothing, catchable only by a person
clicking the button, and no checklist row covers it.

**Concrete test.** In `driver.rs`'s inline `mod tests`, a `RunCtx{resume: true}` T2 unit with
`oracle_frozen: true`: send `RejectOracle` and assert a **second** oracle exec runs and a second
`OracleProposed` with a different hash is emitted. Red today.

### GAP-020 — A `Ship` delivered to a `Halted` unit destroys it

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-branch` |
| **layer** | rust-workspace |
| **verified_by** | automated |
| **anchors** | `crates/fleet-core/src/transition.rs:transition#ship-from-halted`, `crates/fleetd/src/driver.rs:Run::goto#none-arm`, `crates/fleetd/src/server.rs:post_command` |
| **risk** | L3 × I5 = **15** |
| **observations** | coverage_pts=2, branches=2 (branch_pts=1), churn_90d=26 (churn_pts=5) |
| **anchor_sites** | 3 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** **Phase-3 confirmed, every link.** `(NeedsHuman, Ship)` is defined;
`(Halted, Ship)` falls to `_ => None`. `Run::goto`'s `None` arm does not ignore the trigger — it
emits an `Event::Error` and then unconditionally sets `Phase::Failed` with reason
`invalid transition`. The driver's `Phase::NeedsHuman | Phase::Halted` arm routes `Command::Ship` for
**both** phases with no discrimination. And it is reachable from real input: `post_command` forwards
any deserialized `Command` with zero phase validation (returning 202), and `bridge.ts` exposes `ship`
to plugins with only a shape/`hasUnit` check. Only the host's own button gates on
`phase === 'needs_human'` — precisely the guard the HTTP and plugin paths lack. A Ship that lands one
tick after a Halt permanently `Failed`s a unit that had already produced a clean trial merge.

**Concrete test.** `driver.rs` test `ship_while_halted_must_not_destroy_the_unit`: pre-queue `Halt`,
let it park, send `Ship`, assert the unit stays `Halted` with a rejection `Event::Error` rather than
`Failed`. Write it as the desired behaviour so it fails until `goto`'s `None` arm stops force-failing
on human-supplied triggers.

### GAP-021 — A successful T3 ship orphans the unit's named volume

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-branch` |
| **layer** | rust-workspace |
| **verified_by** | automated |
| **anchors** | `crates/fleetd/src/driver.rs:Run::drive#pause-cleanup-takes-handle`, `crates/fleetd/src/driver.rs:Run::drive#done-arm-discard` |
| **risk** | L4 × I5 = **20** |
| **observations** | coverage_pts=4, branches=3 (branch_pts=2), churn_90d=19 (churn_pts=5) |
| **anchor_sites** | 2 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** **Phase-3 confirmed.** Entering `NeedsHuman`/`Halted` runs a pause cleanup that
does `self.handle.take()`. T3 lands at `NeedsHuman` by design, so by the time a human Ships it to
`Done` the terminal arm's `if let Some(h) = self.handle.clone()` sees `None` and **skips `discard`** —
and `discard` is the only thing that runs `docker volume rm`. `handle_id` survives but is consumed
only by the `Abandon` arm. No sweeper exists: `reap_unit` and reconcile only remove containers. So
every successful T3 ship, and any Resume→…→Done that passed through a pause, permanently orphans a
`ccvol_<unit>` volume holding a full repo clone. Gate 5 (`GAP-009`) checks `docker ps`, never
`docker volume ls`, so no human would see it either.

**Concrete test.** `driver.rs` test `t3_human_ships_from_needs_human_to_done`: drive a `Tier::T3` spec
(no test in the crate constructs T3) to `NeedsHuman`, send `Command::Ship`, assert `Done` **and**
`runner.discards == 1`. It is 0 today while `teardowns == 1`.

### GAP-022 — `poll_mergeability` fires ten `gh` calls back to back with no delay

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-branch` |
| **layer** | rust-workspace |
| **verified_by** | automated |
| **anchors** | `crates/fleetd/src/driver.rs:Run::poll_mergeability#pending-dirty-and-error-arms`, `crates/fleetd/src/gh_forge.rs:poll_mergeable` |
| **risk** | L4 × I5 = **20** |
| **observations** | coverage_pts=4, branches=4 (branch_pts=2), churn_90d=19 (churn_pts=5) |
| **anchor_sites** | 2 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** `Ok(Mergeability::Pending) => continue` loops `for _ in 0..MAX_MERGEABLE_POLLS`
with **no sleep**, and `gh_forge` maps GitHub's `"UNKNOWN"` to `Pending`. GitHub returns UNKNOWN for
several seconds after every fresh PR, so on a real run the happy path plausibly *always* declares
"mergeability poll timed out" within milliseconds and routes to `NeedsHuman` — and a `gh` rate limit
is amplified tenfold. Tellingly, `preflight_it.rs` compensates with its own 15-iteration loop and a 2 s
sleep: the IT proves the daemon's loop is wrong and hides it at the same time, and it is `#[ignore]`d
so CI never sees any of it. Every driver test uses `FakeForge::default()` (`Mergeable`), so the
Pending, Dirty and Err arms are unexecuted.

**Concrete test.** With `FakeForge { mergeable: Pending }` under `tokio::time::pause()`, assert exactly
`MAX_MERGEABLE_POLLS` polls, that **virtual elapsed time is greater than zero** (i.e. a backoff
exists), and that it ends in `Blocked` + `PrDirty`.

### GAP-023 — The whole host-side git/GitHub failure surface is unexecuted because `FakeForge` cannot fail

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-branch` |
| **layer** | rust-workspace |
| **verified_by** | automated |
| **anchors** | `crates/fleetd/src/fake.rs:FakeForge#no-failure-knobs`, `crates/fleetd/src/driver.rs:Run::drive#forge-and-export-failure-arms` |
| **risk** | L4 × I5 = **20** |
| **observations** | coverage_pts=4, branches=5 (branch_pts=2), churn_90d=19 (churn_pts=5) |
| **anchor_sites** | 2 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** `FakeForge` is only ever built via `::default()` (Clean + Mergeable) — grep finds
no struct-literal construction anywhere — and its three methods return `Ok(..)` unconditionally. So
five driver arms can never fire in an automated test: `export_bundle` Err, `MergeResult::Conflict`,
`trial_merge` Err, `open_pr` Err, and the poll Err arm. These are the routine real-world failures — a
base that moved, an expired `gh` token, a full temp dir, a secondary rate limit. Two contract
mismatches are visible and unasserted: `trial_merge` Err is emitted `retryable: true` but routed to a
non-retryable `Failed`, and "a PR already exists for this branch" (the normal outcome of any resume) is
treated as fatal. **This is the single cheapest reclassification in the repo** — the struct's fields
are already `pub` and already model Conflict/Dirty/Pending.

**Concrete test.** Add `trial_merge_fails`/`open_pr_fails`/`poll_fails` builders plus struct-literal
construction in driver tests, and assert each arm's scope, retryability, terminal phase, and whether
the container was torn down versus discarded.

### GAP-024 — Every command-validity decision is written out four times

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `duplicated-logic` |
| **layer** | rust-workspace |
| **verified_by** | automated |
| **anchors** | `crates/fleetd/src/driver.rs:Run::poll_halt`, `crates/fleetd/src/driver.rs:Run::drive#awaiting-oracle-recv`, `crates/fleetd/src/driver.rs:Run::drive#paused-recv`, `crates/fleetd/src/driver.rs:Run::agent_exec#backoff-select` |
| **risk** | L4 × I5 = **20** |
| **observations** | coverage_pts=3, branches=21 (branch_pts=5), churn_90d=19 (churn_pts=5) |
| **anchor_sites** | 4 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** Four hand-written copies of "which commands are valid here", each with its own
reject message. Adding a `Command` variant means editing four places and nothing enforces it. The
drift is already load-bearing: during a rate-limit backoff only `Halt` is honoured, so a user hitting
ABANDON on a throttled unit gets "not valid" and the unit keeps waiting — up to the full
`CC_RL_MAX_WAIT_SECS` (3600 s default) — holding its permit and its container.
`non_halt_command_during_backoff_errors_and_keeps_retrying` actually *pins* that behaviour with
`Resume`, and nothing flags that `Abandon` falls in the same bucket.

**Concrete test.** `crates/fleetd/tests/command_dispatch_matrix_it.rs`, fakes only: a table over every
`Command` × every waiting state, asserting each cell is either accepted-and-transitions or
rejected-with-an-Error-and-stays-parked — never silently dropped, never a hard `Failed`.

### GAP-025 — The red-checks feedback loop has no test and no iteration ceiling

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-branch` |
| **layer** | rust-workspace |
| **verified_by** | automated |
| **anchors** | `crates/fleetd/src/driver.rs:Run::drive#checking-red-checks`, `crates/fleetd/src/driver.rs:Run::drive#has-diff-error` |
| **risk** | L3 × I5 = **15** |
| **observations** | coverage_pts=4, branches=2 (branch_pts=1), churn_90d=19 (churn_pts=5) |
| **anchor_sites** | 2 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** `if out.exit_code != 0 { goto(ChecksFailed) }` is the entire red-test feedback
loop and no test anywhere supplies a non-zero check exit — `demo_script` in both `server.rs` and the
mirrored `demo_mode_it.rs` always scripts passing checks. The `Checking → Building → Checking` loop has
no iteration ceiling of its own; its only bounds are the USD cap and the between-phase wall-clock
check, itself untested (`GAP-026`). An agent that can never go green burns the full budget in a loop
nothing asserts. Separately, the `has_diff` Err arm fails **open** (proceeds to `ChecksPassed`),
opening a PR for a possibly-empty branch — a real decision no test pins.

**Concrete test.** Script oracle → build → check with `exit_code: 1` → build/check/review passing;
assert the phase sequence contains `Checking → Building`, that two `Iteration{Build}` events fired,
and that a tiny USD cap bounds the loop at `NeedsHuman`.

### GAP-026 — The wall-clock cap's driver-side use is dead code in the entire suite

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-branch` |
| **layer** | rust-workspace |
| **verified_by** | automated |
| **anchors** | `crates/fleetd/src/driver.rs:Run::drive#wall-clock-backstop`, `crates/fleetd/src/driver.rs:Run::over_wall_clock` |
| **risk** | L3 × I5 = **15** |
| **observations** | coverage_pts=4, branches=2 (branch_pts=1), churn_90d=19 (churn_pts=5) |
| **anchor_sites** | 2 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** Every driver test — and both `demo_mode_it.rs` scenarios, which set
`wall_clock_secs: 1800` — finishes long before `over_wall_clock()` can return true, so the
`is_agent_active() && over_wall_clock()` guard never executes. `retry.rs:wall_clock_exceeded` is
tested as a pure function, but the driver's *use* of it — the `Blocked` event the cockpit renders, the
`CapBreach` routing, and the rate-limit exemption fed by `self.rl_elapsed` — is not. This is the
daemon's only defence against an agent that loops without tripping the per-step USD check, so a
regression is a silent budget hole.

**Concrete test.** `#[tokio::test(start_paused = true)]` with `wall_clock_secs: 5` and a `FakeRunner`
whose exec sleeps 10 virtual seconds: assert `Blocked{cap: Some("wall_clock")}`, `NeedsHuman`, and
permit release. Companion: a rate-limited run whose `elapsed - rl_elapsed` stays under the cap must
**not** trip it.

### GAP-027 — `fail_closed` bypasses the state machine and is never executed

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-symbol` |
| **layer** | rust-workspace |
| **verified_by** | automated |
| **anchors** | `crates/fleetd/src/driver.rs:Run::fail_closed` |
| **risk** | L3 × I5 = **15** |
| **observations** | coverage_pts=4, branches=0 (branch_pts=1), churn_90d=19 (churn_pts=5) |
| **anchor_sites** | 1 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** The only path in the driver that sets `self.phase` directly, bypassing
`fleet_core::transition` — so it can move a unit to `Failed` from a state the machine might not allow.
It fires whenever the last `cmd_tx` is dropped while a unit is parked, and
`crates/fleetd/src/bin/run_once.rs` (a ratified entry point) drops `cmd_tx` immediately. So in a live
`run_once`, **any** cap breach, oracle-tamper detection, retries-exhausted park, merge conflict, or
PR-dirty verdict converts instantly to a hard `Failed` where the design intends a recoverable human
gate. No test closes the channel while a unit is parked.

**Concrete test.** `closed_command_channel_at_needs_human_fails_closed`: park via a cap breach with the
sender already dropped; assert the error + `Failed{reason: "control channel closed"}`, that the
container was torn down but the volume kept, and that the permit is released.

### GAP-028 — `ClaudePlanner::plan` spawns a real `claude` process with no timeout and no test

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-symbol` |
| **layer** | rust-workspace |
| **verified_by** | automated |
| **anchors** | `crates/fleetd/src/planner.rs:ClaudePlanner::plan` |
| **risk** | L3 × I5 = **15** |
| **observations** | coverage_pts=4, branches=0 (branch_pts=1), churn_90d=2 (churn_pts=2) |
| **anchor_sites** | 1 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** Reached from `serve.rs → run_swarm(ClaudePlanner::new())` for any non-demo swarm,
it awaits `Command::output()` with no timeout, no cancellation and no kill-on-drop. If the CLI hangs
or waits on stdin the swarm sits in status `planning` forever; `reconcile_on_startup` only rescues that
state on a *daemon restart*, so nothing recovers it while the process lives. Two further unpinned
contracts: `--max-budget-usd 1.0` is hardcoded rather than derived from the swarm's `usd_budget`, and
`parse_usage(..).unwrap_or(0.0)` means a format change silently bills planning at $0. Every test uses
`FakePlanner`, so not one line executes in CI.

**Concrete test.** Split out a pure `parse_plan_output(stdout, lane_cap)` and table-test it against
captured `stream-json` fixtures (mixed narration, a `result` record, malformed JSON, no array, an
over-cap array). Then assert `plan()` is wrapped in a bounded `tokio::time::timeout`.

### GAP-029 — The USD-cap check is copy-pasted at five call sites, three of them unexercised

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `duplicated-logic` |
| **layer** | rust-workspace |
| **verified_by** | automated |
| **anchors** | `crates/fleetd/src/driver.rs:Run::account`, `crates/fleetd/src/driver.rs:Run::remaining` |
| **risk** | L3 × I5 = **15** |
| **observations** | coverage_pts=2, branches=6 (branch_pts=3), churn_90d=19 (churn_pts=5) |
| **anchor_sites** | 2 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** `if self.account(&out) { goto(CapBreach, "usd cap") ; continue; }` is hand-copied
at `Spec`, `Building`, `Checking`, `Reviewing`, and inside `agent_exec`'s rate-limit arm. There is no
choke point, so a new billable step added without the copy spends money with no ceiling and emits no
`Metric` for the cockpit's cost chip. Enforcement is *post-hoc* (`cost_usd > usd_cap` after the spend),
so the only pre-hoc bound is the `--max-budget-usd` value `remaining()` feeds to `claude_argv` —
making the correctness of the copies the difference between a bounded and an unbounded overrun. Two of
the five copies have direct assertions; the Spec, Checking and Reviewing breach sub-paths do not.

**Concrete test.** A table over the four billable phases scripting a cheap run to the target phase then
one exec that blows the cap; assert each lands at `NeedsHuman{reason: "usd cap"}` and that no further
exec is issued. Plus `remaining_never_goes_negative_and_is_passed_to_claude`.

### GAP-030 — `steps::build` and `steps::review` are untested, and `review`'s prompt is half a contract

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-symbol` |
| **layer** | rust-workspace |
| **verified_by** | automated |
| **anchors** | `crates/fleetd/src/steps.rs:build`, `crates/fleetd/src/steps.rs:review`, `crates/fleetd/src/driver.rs:parse_blockers` |
| **risk** | L3 × I5 = **15** |
| **observations** | coverage_pts=4, branches=0 (branch_pts=1), churn_90d=4 (churn_pts=3) |
| **anchor_sites** | 3 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** `steps.rs` has three tests and they only call `oracle` and `check`. `review`
matters disproportionately: its prompt asks the agent to emit `BLOCKERS=N`, and `parse_blockers`
**defaults to 0 when the marker is absent**. So any drift in the prompt wording — or a model that
stops complying — silently produces zero blockers, `gate_met` opens on the round floor, and a unit with
unresolved must-fix findings sails into an auto-opened PR. `parse_blockers` is tested against a literal
string, which is exactly the kind of test that cannot catch producer/consumer drift.

**Concrete test.** `review_prompt_demands_the_blockers_marker`: assert the prompt contains
`BLOCKERS=N` and round-trip it through `parse_blockers` in the same assertion, so producer and consumer
are pinned together. Plus `build_prompt_carries_task_and_findings`.

### GAP-031 — `reconcile` and `reconcile_live` re-derive the same decision independently

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `duplicated-logic` |
| **layer** | rust-workspace |
| **verified_by** | automated |
| **anchors** | `crates/fleetd/src/reconcile.rs:reconcile`, `crates/fleetd/src/reconcile.rs:reconcile_live` |
| **risk** | L2 × I5 = **10** |
| **observations** | coverage_pts=2, branches=9 (branch_pts=3), churn_90d=2 (churn_pts=2) |
| **anchor_sites** | 2 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** `reconcile_live` is `reconcile` plus a `live` filter; both independently re-derive
`HaltWithContainer` vs `HaltNoContainer` and both re-derive stray detection from a second pass. They
run in different lifecycles (startup vs a 30 s timer) and both drive destructive `reap_unit` calls plus
store writes that force units to `Halted`. Divergence means the timer pass could start halting healthy
in-flight work, or stop reaping genuine orphans. Both functions are individually well tested — the only
untested sub-path is their **agreement**, which is the whole risk of the duplication.

**Concrete test.** `startup_is_steady_state_with_no_live_drivers`: assert
`reconcile(&p, &r) == reconcile_live(&p, &[], &r)` over an exhaustive enumeration on a 3-id universe.

### GAP-032 — `retry.rs:env_secs` and its three wrappers are untested

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-symbol` |
| **layer** | rust-workspace |
| **verified_by** | automated |
| **anchors** | `crates/fleetd/src/retry.rs:env_secs`, `crates/fleetd/src/retry.rs:rl_max_wait_secs` |
| **risk** | L3 × I5 = **15** |
| **observations** | coverage_pts=4, branches=0 (branch_pts=1), churn_90d=3 (churn_pts=3) |
| **anchor_sites** | 2 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** These govern the rate-limit retry budget, including the ~1 h envelope after which
`agent_exec` gives up and parks. `.parse().ok().unwrap_or(default)` swallows every malformed value
silently, so `CC_RL_MAX_WAIT_SECS=1h` yields 3600 by luck of the default and a typo'd variable name is
equally silent. During that envelope the unit holds its container and its concurrency permit, so this
is a real fleet-throughput knob. `Backoff::next_delay` is well tested but always with hand-passed
literals; the env plumbing that supplies them in production has no test.

**Concrete test.** One serialized test asserting the three documented defaults (2 / 300 / 3600) with the
vars unset, that a valid value is honoured, and that a malformed value falls back to the default rather
than to 0.

### GAP-033 — Driver-plus-real-Docker resume has never been verified by machine or human

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `manual-uncovered` |
| **layer** | manual |
| **verified_by** | manual |
| **anchors** | `crates/fleetd/src/driver.rs:run#resume-contract`, `crates/fleetd/src/local_docker.rs:provision#reused-volume-resume-path` |
| **governs** | `crates/fleetd/src/driver.rs`, `crates/fleetd/src/local_docker.rs`, `crates/fleetd/src/server.rs` |
| **last_manual_pass** | — (never_verified) |
| **risk** | L5 × I5 = **25** |
| **observations** | manual_coverage_pts=5, churn_90d=19 (churn_pts=5), never_verified=true |
| **anchor_sites** | 2 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** **No automated test anywhere runs `driver::run` against `LocalDockerRunner`** —
`demo_mode_it.rs` is fakes-only by design, and `preflight_it.rs`/`local_docker_it.rs` are `#[ignore]`d
and drive `Runner` methods by hand, deliberately bypassing the driver. So the crash-restart recovery
contract (reuse the persisted volume, skip the frozen oracle, re-checkout the agent branch over a
possibly dirty tree, re-arm the tamper gate from the reloaded hash) is only ever verified against
`FakeRunner`, whose `provision` returns a constant handle. The riskiest half lives in
`provision`'s `reused` branch (`rm -f .git/index.lock`, `git checkout -B`), which the fake cannot model
at all — and `reused` is inferred from an `exec` that can fail for unrelated reasons, in which case a
genuinely resumable volume is silently re-cloned over, losing all agent work. No checklist row covers
daemon-restart resume.

**Concrete test.** CI-runnable half: a `ResumingFakeRunner` recording the ordered `Runner` call
sequence, asserting a `RunCtx{resume: true, start_phase: Halted}` run issues `provision` → *no oracle
exec* → `read_files` → build/check, and never `discard`. Real half `#[ignore]`d: provision, commit,
teardown, re-provision the same id, assert no re-clone and that a planted `.git/index.lock` is removed.

### GAP-034 — `gate_met`'s anti-oscillation conjunct is unreachable dead logic

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-branch` |
| **layer** | rust-workspace |
| **verified_by** | automated |
| **anchors** | `crates/fleet-core/src/gate.rs:gate_met#non-increasing-conjunct` |
| **risk** | L2 × I3 = **6** |
| **observations** | coverage_pts=2, branches=3 (branch_pts=2), churn_90d=1 (churn_pts=2) |
| **anchor_sites** | 1 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** **Phase-3 confirmed.** `unresolved_blockers` and `prev_unresolved_blockers` are
both `u32`, and the verdict already requires `unresolved_blockers == 0`, so `0 <= prev` holds for every
possible prev — `non_increasing` can never change the answer. The documented intent ("no oscillation
in this round", restated in `transition.rs`) is unenforced: a unit that oscillates 0 → 3 → 0 across
rounds auto-advances identically to one that converged. This is the gate that lets the driver fire
`ReviewFinished{gate_met: true}` and push toward MergeCheck without a human. Low risk score only
because `fleet_core` is a spine-weight-3 module; the finding is structural, not cosmetic.

**Concrete test.** `non_increasing_is_subsumed_by_zero_blockers` documenting the unreachability, then a
decision: either delete the conjunct or move the anti-oscillation check somewhere it can bind (e.g. on
`round`-over-`round` blocker history rather than the current count).

### GAP-035 — `Event`'s wire shape is the cockpit's contract and seven of ten variants are unasserted

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-symbol` |
| **layer** | rust-workspace |
| **verified_by** | automated |
| **anchors** | `crates/fleet-core/src/event.rs:Event`, `cockpit/ui/src/lib/types.ts:FleetEvent` |
| **risk** | L2 × I3 = **6** |
| **observations** | coverage_pts=2, branches=0 (branch_pts=1), churn_90d=2 (churn_pts=2) |
| **anchor_sites** | 2 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** `Event` is the outbound `/stream` contract consumed across a process boundary by a
**hand-maintained** TypeScript mirror with nothing enforcing agreement. Only 3 of 10 variants have any
serialization assertion. The untested ones are the ones the operator acts on: `Blocked` (drives the
"why is this stuck" surface and the rate-limit notice), `Finding` (severity/round feed the review gate
display), `Error`, `Done`. The `skip_serializing_if` behaviour on `Finding.file` and `Blocked.cap` —
which `types.ts` encodes as optional properties — is asserted for zero of them, so a dropped attribute
compiles, passes `cargo test --workspace`, and shows up as a blank cell in a running cockpit.

**Concrete test.** `every_event_variant_wire_shape`: serialize one instance of each variant, assert the
`type` tag and every field key including the `None`-omission and `Some`-presence cases, then round-trip
each back through `from_str`.

### GAP-036 — The snake_case phase vocabulary is hand-duplicated across Rust, SQL and TypeScript

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `duplicated-logic` |
| **layer** | rust-workspace |
| **verified_by** | automated |
| **anchors** | `crates/fleet-core/src/phase.rs:Phase`, `crates/fleet-core/src/phase.rs:TERMINAL_PHASE_STRS`, `crates/fleetd/src/store.rs:swarm_rollup`, `cockpit/ui/src/lib/types.ts:Phase` |
| **risk** | L2 × I3 = **6** |
| **observations** | coverage_pts=2, branches=0 (branch_pts=1), churn_90d=14 (churn_pts=5) |
| **anchor_sites** | 4 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** Four independent copies, only the terminal subset pinned: the serde derive, the
`TERMINAL_PHASE_STRS` const (one test), raw SQL literals in `swarm_rollup` (`phase IN
('needs_human','halted')`) plus the interpolated queries in `committed_spend` and the server's
admission filters, and the TS union consumed by `store.svelte.ts`, `App.svelte` (`canShip`/`canResume`/
`PHASE_LABEL`) and `adapters/fleet.ts`. Phase is persisted as a bare `String` column, so nothing
type-checks the SQL against the enum and nothing at all type-checks TS against Rust. Renaming or adding
a phase silently breaks the parked-unit rollup, the spend partition, and the cockpit's attention
highlighting — all across boundaries `cargo test --workspace` cannot see. Widest drift surface in the
repo. See also `GAP-081` and `GAP-082` (the UI-side half).

**Concrete test.** Extend `terminal_strs_match_is_terminal` into
`phase_wire_strings_are_exhaustive_and_stable`: serialize all 14 variants and assert the set equals a
literal list, so adding or renaming one fails CI and forces the mirrors to be updated.

### GAP-037 — `Command::to_trigger` has no production caller while the driver re-implements it twice

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `duplicated-logic` |
| **layer** | rust-workspace |
| **verified_by** | automated |
| **anchors** | `crates/fleet-core/src/event.rs:Command::to_trigger`, `crates/fleetd/src/driver.rs:Run::drive#awaiting-oracle-recv`, `crates/fleetd/src/driver.rs:Run::drive#paused-recv` |
| **risk** | L2 × I3 = **6** |
| **observations** | coverage_pts=2, branches=6 (branch_pts=3), churn_90d=19 (churn_pts=5) |
| **anchor_sites** | 3 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** Three copies of one mapping, and **only the unused copy is under test** — and only
2 of its 6 arms at that. A new command verb ships correct in `fleet-core` and wrong in the daemon; the
`_ => other` catch-alls in both driver arms mean a mismatch degrades to a silent "not valid"
`Event::Error` rather than a compile failure. Overlaps `GAP-024`, which covers the four-way duplication
of the *validity* decision; this entry is specifically the command→trigger mapping.

**Concrete test.** `every_command_maps_to_its_trigger` covering all six arms, then replace the two
inline mappings with `cmd.to_trigger()` (or assert the inline copies agree with it for every variant).

### GAP-038 — `OracleTampering`'s transition arm is the trust gate and has no direct test

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-branch` |
| **layer** | rust-workspace |
| **verified_by** | automated |
| **anchors** | `crates/fleet-core/src/transition.rs:transition#oracle-tampering-arm` |
| **risk** | L2 × I3 = **6** |
| **observations** | coverage_pts=3, branches=2 (branch_pts=1), churn_90d=3 (churn_pts=3) |
| **anchor_sites** | 1 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** `OracleTampering if phase.is_agent_active() => NeedsHuman` is its own arm and the
trust gate the whole T1-autonomy story rests on; the driver fires it from three sites (unreadable
frozen files, an empty frozen set, a hash mismatch). If the guard or target ever changes, tamper
detection degrades to `None`, which `goto` turns into a hard `Failed` rather than the intended human
review — a silent downgrade of a security-shaped gate into a crash. Exercised only indirectly by two
fleetd driver tests; the guard-false side is never exercised for this trigger at all.

**Concrete test.** `oracle_tampering_parks_agent_phases_at_needs_human` mirroring the existing
`retries_exhausted_*` test over `[Spec, Building, Checking, Reviewing]`, plus `MergeCheck` and
`AwaitingOracleApproval` asserting `None`.

### GAP-039 — `Provisioning` is excluded from `is_agent_active`, so nothing bounds a hung provision

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `manual-uncovered` |
| **layer** | manual |
| **verified_by** | manual |
| **anchors** | `crates/fleet-core/src/phase.rs:Phase::is_agent_active` |
| **governs** | `crates/fleet-core/src/phase.rs`, `crates/fleetd/src/driver.rs`, `crates/fleetd/src/local_docker.rs` |
| **last_manual_pass** | — (never_verified) |
| **risk** | L4 × I3 = **12** |
| **observations** | manual_coverage_pts=4, churn_90d=3 (churn_pts=3), never_verified=true |
| **anchor_sites** | 1 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** `is_agent_active` returns true only for Spec/Building/Checking/Reviewing, and the
driver's only stall backstop gates on exactly that predicate. But `Provisioning` is where the long
blocking work happens: an unbounded `acquire_owned().await` on the concurrency semaphore, then
`local_docker.rs:provision`, which shells out to docker with no timeout. So a hung image build or a
never-freed slot leaves a unit pinned in `Provisioning` with no cap, no stall trigger, and no escape
hatch — the cockpit just shows PROVISIONING and a human is the only detector. Same failure shape as the
1.5 freeze.

**Concrete test.** A driver-level test with a fake `provision()` that sleeps past `wall_clock_secs`,
asserting the unit leaves `Provisioning`; plus a `fleet-core` unit test pinning the exclusion as a
deliberate decision rather than an accident.

### GAP-040 — `Phase::is_interruptible` is exported, uncalled, untested, and duplicated inline

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-symbol` |
| **layer** | rust-workspace |
| **verified_by** | automated |
| **anchors** | `crates/fleet-core/src/phase.rs:Phase::is_interruptible` |
| **risk** | L3 × I3 = **9** |
| **observations** | coverage_pts=4, branches=0 (branch_pts=1), churn_90d=3 (churn_pts=3) |
| **anchor_sites** | 1 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** Zero callers repo-wide and zero tests, while `transition` open-codes the identical
condition twice as `!phase.is_terminal()` on the `FatalError` and `Halt` arms. The crate ships a named
concept the state machine does not use, and an edit to one will not track the other. Drift/dead-API
rather than a live defect — ranked accordingly.

**Concrete test.** Either delete it, or add `interruptible_is_the_complement_of_terminal` over all 14
variants and wire it into the two arms that currently open-code it.

### GAP-041 — `Store::open` is the only constructor any real process uses and no test calls it

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-symbol` |
| **layer** | rust-workspace |
| **verified_by** | automated |
| **anchors** | `crates/fleetd/src/store.rs:Store::open` |
| **risk** | L3 × I4 = **12** |
| **observations** | coverage_pts=4, branches=0 (branch_pts=1), churn_90d=14 (churn_pts=5) |
| **anchor_sites** | 1 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** All 13 inline store tests and every `server.rs` test call `Store::open_memory()`,
which dies with the process and cannot exercise WAL, the `-wal`/`-shm` sidecars, or reopen.
`Store::open` is what `bin/serve.rs` actually uses. The whole reason the schema carries set-once
columns is resume-after-restart, and the cockpit sidecar auto-restarts fleetd on every exit — so
restart is the normal path, not the exceptional one. A durability regression (rows not committed, the
seq allocator re-seeding to 0 and re-minting a live unit id) would be invisible to CI and would surface
only as a human noticing their fleet history vanished.

**Concrete test.** Open a `Store` on a real file in a temp dir, write a unit + swarm + lanes + events,
drop it, reopen with `Store::open`, and assert every row and that `max_unit_seq`/`max_swarm_seq`
re-seed to the persisted maxima rather than 0.

### GAP-042 — `Store::init`'s migration ALTERs swallow every error, so a failed upgrade reads as an empty fleet

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-branch` |
| **layer** | rust-workspace |
| **verified_by** | automated |
| **anchors** | `crates/fleetd/src/store.rs:Store::init#alter-migration-on-preexisting-db` |
| **risk** | L3 × I4 = **12** |
| **observations** | coverage_pts=4, branches=1 (branch_pts=1), churn_90d=14 (churn_pts=5) |
| **anchor_sites** | 1 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** The migration loop is `let _ = conn.execute(stmt, [])` and the comment concedes it
relies on the ALTERs being "a no-op failure we ignore". On `open_memory` the CREATE TABLE always wins,
so the ALTER path never supplies the columns in any test — the sole migration test proves only the
fresh-db case. The fresh CREATE for `units` does **not** declare `swarm_id`, so even a new db depends
on an ALTER that cannot fail loudly. If any ALTER fails for a real reason on a user's existing
`fleet.db`, `init` still returns `Ok`, and every downstream query dies on "no such column" — which
`server.rs` converts to silence via `unwrap_or_default()`/`.ok()`. Silent total history loss on
upgrade, caught by no gate and no checklist row.

**Concrete test.** Hand-build a temp db with an OLD `units` table plus one legacy row, then
`Store::open` it and assert the row is readable with the new columns defaulted. Add a negative case
where an ALTER genuinely fails and assert `open()` reports it rather than returning `Ok`.

### GAP-043 — No `busy_timeout` anywhere: a second writer loses events silently

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `manual-uncovered` |
| **layer** | manual |
| **verified_by** | manual |
| **anchors** | `crates/fleetd/src/store.rs:Store::init#pragma-set` |
| **governs** | `crates/fleetd/src/store.rs`, `crates/fleetd/src/bin/serve.rs`, `cockpit/ui/src-tauri/src/sidecar.rs` |
| **last_manual_pass** | — (never_verified) |
| **risk** | L5 × I4 = **20** |
| **observations** | manual_coverage_pts=5, churn_90d=14 (churn_pts=5), never_verified=true |
| **anchor_sites** | 1 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** `init` sets `journal_mode=WAL` and `synchronous=NORMAL` but no `busy_timeout` —
grep finds no `busy_timeout`/`busy_handler` call in the whole repo — so a second writer gets
`SQLITE_BUSY` immediately with no retry. Two writers are reachable in normal use: the cockpit
supervisor respawns `fleetd-serve` the instant the old one exits, with no wait for the WAL lock; and a
developer running `cargo run --bin serve` alongside the packaged app opens the same default
`./fleet.db`. Every store write on the event hot path is `let _ = ...`, so a BUSY loses the event
silently; and if `Store::open` itself fails, `serve.rs` panics and the supervisor restart-loops. No
db/persistence/restart row exists in the manual checklist either.

**Concrete test.** Hold a write transaction from a second connection, then drive a unit through the
serve binary against the same `CC_DB`: assert writes either block-and-succeed or surface a visible
error, never silently drop. Pair with a supervisor check that a `Store::open` failure does not produce
an unbounded respawn loop.

### GAP-044 — Every driver event does two synchronous SQLite writes inside a global mutex on a tokio worker

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `manual-uncovered` |
| **layer** | manual |
| **verified_by** | manual |
| **anchors** | `crates/fleetd/src/server.rs:spawn_forwarder`, `crates/fleetd/src/store.rs:Store::append_event`, `crates/fleetd/src/store.rs:Store::update_unit` |
| **governs** | `crates/fleetd/src/server.rs`, `crates/fleetd/src/store.rs` |
| **last_manual_pass** | — (never_verified) |
| **risk** | L5 × I4 = **20** |
| **observations** | manual_coverage_pts=5, churn_90d=26 (churn_pts=5), never_verified=true |
| **anchor_sites** | 3 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** Two blocking `rusqlite` writes per event, inside a `std::sync::Mutex` critical
section, on a tokio worker thread — against a WAL file on disk in production but only against
`:memory:` in every existing test, so the tests **structurally cannot observe the real IO cost**. This
is the exact defect class the repo has already been burned by: the 1.5 FAIL was blocking work on the
wrong thread, passed every automated gate, and was caught only by a human watching a frozen window.
Here the cost scales with event volume and fleet size (one global lock shared by all drivers and every
HTTP handler) and the failure is a laggy cockpit rather than a red test. `evt_rx` is also an unbounded
mpsc, so a chatty driver grows memory with no backpressure.

**Concrete test.** Drive a few thousand events through `spawn_forwarder` against a **file-backed**
store and assert per-event forwarder latency stays under a budget, that a concurrent `/health` is
served within a budget while the burst is in flight, and that no events are dropped.

### GAP-045 — `events_since` replays from 0 with no retention, pagination, or bound

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `manual-uncovered` |
| **layer** | manual |
| **verified_by** | manual |
| **anchors** | `crates/fleetd/src/store.rs:Store::events_since`, `crates/fleetd/src/server.rs:get_unit` |
| **governs** | `crates/fleetd/src/store.rs`, `crates/fleetd/src/server.rs`, `cockpit/ui/src/lib/store.svelte.ts` |
| **last_manual_pass** | — (never_verified) |
| **risk** | L5 × I4 = **20** |
| **observations** | manual_coverage_pts=5, churn_90d=26 (churn_pts=5), never_verified=true |
| **anchor_sites** | 2 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** No `DELETE`, no `VACUUM`, no retention policy and no `LIMIT` anywhere in
`crates/fleetd/src` — units, swarms, lanes and events grow forever in the long-lived `./fleet.db` the
sidecar keeps reopening. `events_since` collects the entire matching set into a `Vec<String>` in
memory, and both the snapshot and WS-attach replay paths call it with `since = 0`, on the same global
store mutex. `FleetStore.ensureStream` also always passes `sinceSeq = 0`, so a cockpit reload makes the
daemon re-serialize every unit's entire log — a startup thundering herd. Unbounded growth plus full
replay is a slow-motion UI hang no unit test will ever see (existing tests replay one or two events).

**Concrete test.** Append 100 k events for one unit and assert the `/events` and WS-attach replay paths
stay under latency and memory budgets; assert some retention or pagination bound exists; assert
repeated daemon restarts do not monotonically grow the db.

### GAP-046 — `docker_ok` has no timeout and no single-flight guard, and `create_swarm` awaits it in-handler

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-symbol` |
| **layer** | rust-workspace |
| **verified_by** | automated |
| **anchors** | `crates/fleetd/src/server.rs:docker_ok`, `crates/fleetd/src/server.rs:create_swarm#docker-preflight` |
| **risk** | L3 × I4 = **12** |
| **observations** | coverage_pts=4, branches=1 (branch_pts=1), churn_90d=26 (churn_pts=5) |
| **anchor_sites** | 2 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** `Command::new("docker").args(["version",...]).output().await` with no timeout, and
the 5 s TTL cache is written only *after* the await returns — so a Docker Desktop that is starting or
wedged makes every unserved `/health` poll spawn its own docker process. The Tauri supervisor polls
every 300 ms for 20 s and `App.svelte` polls on an interval, so this is a subprocess pileup under
exactly the condition the probe exists to detect. Worse, `create_swarm` awaits `docker_ok` **inside**
the request handler for `mode: "real"`, so `POST /swarms` hangs unboundedly with the human staring at a
spinner — the same "slow work on the interactive path" shape as the 1.5 freeze. CI has no Docker, so
the false branch is all a CI machine could ever see; the true and hung branches are human-QA-only.

**Concrete test.** Behind a probe seam: assert the 5 s TTL is honoured (two calls inside the window
spawn one subprocess), that an injected 60 s probe returns `false` within a stated deadline, and that
`/health` answers within that deadline while the probe is stuck.

### GAP-047 — `env_f64` accepts a zero cap, bricking every mission with a 429

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-symbol` |
| **layer** | rust-workspace |
| **verified_by** | automated |
| **anchors** | `crates/fleetd/src/server.rs:env_f64`, `crates/fleetd/src/server.rs:env_usize` |
| **risk** | L4 × I4 = **16** |
| **observations** | coverage_pts=5, branches=0 (branch_pts=1), churn_90d=26 (churn_pts=5) |
| **anchor_sites** | 2 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** **Phase-3 confirmed.** `env_usize` guards with `.filter(|&n| n > 0)`; `env_f64`
does not. So `CC_GLOBAL_USD_CAP=0` (or a negative) is accepted, and the admission check
`committed_spend(...).unwrap_or(0.0) >= st.global_cap` evaluates `0.0 >= 0.0` — every `POST /missions`
**and** `POST /swarms` is refused forever on a fresh daemon with no spend. `api.ts:createMission`
surfaces that as "global daily cost cap reached", so the human sees a cap error on an empty fleet with
no way to tell it is a config typo. The asymmetry between the two helpers is the tell that the missing
filter is an oversight. `.env.example` and `docs/quickstart.md` document the knob as user-settable.
(Refuter notes `NaN` produces the opposite failure — the cap never binds.)

**Concrete test.** Assert `env_f64` rejects `"0"`, `"-5"` and `NaN` the way `env_usize` rejects `"0"`,
plus a companion asserting an `AppState` built with `CC_GLOBAL_USD_CAP=0` does not turn
`create_mission` into a permanent 429.

### GAP-048 — `stream_to_socket` drops events permanently on broadcast lag and never notices a dead peer

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-branch` |
| **layer** | rust-workspace |
| **verified_by** | automated |
| **anchors** | `crates/fleetd/src/server.rs:stream_to_socket#lagged-recv-arm`, `crates/fleetd/src/server.rs:stream_to_socket#no-recv-loop` |
| **risk** | L4 × I4 = **16** |
| **observations** | coverage_pts=4, branches=11 (branch_pts=4), churn_90d=26 (churn_pts=5) |
| **anchor_sites** | 2 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** `broadcast::channel(1024)` plus `Err(Lagged(_)) => continue` is silent, permanent
event loss on the seam the cockpit renders from — and the UI dedups on `seq` but never re-fetches, so a
burst of log lines leaves a tile stuck on a stale phase with no error anywhere. Separately the function
is write-only: it never calls `socket.recv()`, so it never processes Close frames and never learns the
peer is gone while the unit is live-but-quiet — the task and its receiver leak for the daemon's
lifetime (nothing ever removes a `UnitHandle`). The one existing WS test connects *after* the unit is
terminal, so it exercises only the replay half.

**Concrete test.** Flood a unit's sender past 1024 envelopes faster than the socket drains and assert
the client's received `seq` set has **no gap** — i.e. after a lag the server backfills from
`events_since(last_seq)` instead of `continue`-ing. Plus a peer-disappears test asserting the task
terminates.

### GAP-049 — `router()` mounts nine routes with no auth, no origin check, and no CORS layer

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `manual-uncovered` |
| **layer** | manual |
| **verified_by** | manual |
| **anchors** | `crates/fleetd/src/server.rs:router` |
| **governs** | `crates/fleetd/src/server.rs`, `crates/fleetd/src/bin/serve.rs`, `cockpit/ui/src-tauri/tauri.conf.json` |
| **last_manual_pass** | — (never_verified) |
| **risk** | L5 × I4 = **20** |
| **observations** | manual_coverage_pts=5, churn_90d=26 (churn_pts=5), never_verified=true |
| **anchor_sites** | 1 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** No auth layer, no origin check, no CORS layer — `tower-http` is not even a
dependency. Two unverified consequences pull in opposite directions and **neither is gated**.
*Security:* `POST /missions` and `POST /swarms` spend real money and start containers; any local
process can drive them, and `CC_ADDR` accepts any bind address with no loopback assertion, so one env
var exposes the fleet to the LAN. *Function:* the Tauri webview origin is `tauri://localhost`, not
`127.0.0.1:8787`; `tauri.conf.json` grants `connect-src` (CSP), but **CSP is not CORS**, and `api.ts`
uses plain `fetch`. Whether the read succeeds depends entirely on WebView2/WKWebView behaviour for the
custom scheme — unverified. Every vitest suite stubs `fetch`, so no test at any layer crosses the real
origin boundary. The only evidence this works is a human having watched a window.

**Concrete test.** A `crates/fleetd/tests/http_contract_it.rs` case that issues requests carrying
`Origin: http://tauri.localhost` and `Origin: https://evil.example` and asserts the response's
`Access-Control-Allow-Origin` against a **decided** policy, plus a preflight `OPTIONS`. Whichever policy
is chosen, the test freezes it. Add the corresponding human row.

### GAP-050 — `post_command` is the entire inbound control surface and no test drives it over HTTP

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-symbol` |
| **layer** | rust-workspace |
| **verified_by** | automated |
| **anchors** | `crates/fleetd/src/server.rs:post_command` |
| **risk** | L4 × I4 = **16** |
| **observations** | coverage_pts=4, branches=5 (branch_pts=2), churn_90d=26 (churn_pts=5) |
| **anchor_sites** | 1 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** Halt/Resume/Abandon/Ship/ApproveOracle all arrive here, and every existing test
pokes `h.cmd_tx` directly — bypassing the handler, the `Json<Command>` deserializer, and the
rehydrate-then-dispatch ordering. All three status outcomes (202/404/410) are unasserted, and axum's
422 on a deserialize failure never appears in the handler at all. The failure is invisible by
construction: `api.ts:sendCommand` returns the status but `FleetStore.cmd` discards it, so a
404/410/422 renders as a button click that does nothing. This handler also carries the `rehydrate` side
effect, so an untested path can **start a Docker container as a side effect of an HTTP POST**. See
`GAP-020` for the destructive case this lack of validation enables.

**Concrete test.** `crates/fleetd/tests/http_contract_it.rs` (demo mode, no Docker): POST to a ghost id
→ 404; to a unit whose driver exited → 410; a valid Halt → 202 **and** a `phase_changed` carrying the
same `cmd_id` on that unit's stream.

### GAP-051 — The `/units`, `/health` and `/units/:id` JSON shapes are a hand-mirrored contract nothing gates

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-symbol` |
| **layer** | rust-workspace |
| **verified_by** | automated |
| **anchors** | `crates/fleetd/src/server.rs:list_units`, `crates/fleetd/src/server.rs:health`, `crates/fleetd/src/server.rs:get_unit`, `cockpit/ui/src/lib/types.ts:Snapshot` |
| **risk** | L3 × I4 = **12** |
| **observations** | coverage_pts=4, branches=0 (branch_pts=1), churn_90d=26 (churn_pts=5) |
| **anchor_sites** | 4 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** These produce the only JSON the fleet view is built from, and no test anywhere
serializes them. `types.ts` is hand-maintained with no generator, so renaming `usd_cap` or dropping
`task` compiles clean, passes `cargo test --workspace`, passes `npm run check` (TS sees only its own
stale interface), passes `tauri build` — and the cockpit renders `undefined`, or throws outright in
`fromSnapshot`, which dereferences `s.tier.toUpperCase()`. **Drift is already present and
unverified:** the Rust `Snapshot` returned by `GET /units/:id` has no `tier` or `task`, while the TS
interface of the same name requires both. Discovered today only by a human seeing a blank tile.

**Concrete test.** `crates/fleetd/tests/ui_contract_it.rs`: assert the exact top-level key set of each
payload against a literal list, failing on missing **and** extra keys, and that `phase` is one of the
14 strings in `types.ts`. Emit the same payloads as golden fixtures under
`cockpit/ui/src/lib/__fixtures__/` and type-assert them in vitest so both sides fail together.

### GAP-052 — `create_mission` and `create_swarm`'s real-mode money guards are unexecuted

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-branch` |
| **layer** | rust-workspace |
| **verified_by** | automated |
| **anchors** | `crates/fleetd/src/server.rs:create_mission#real-mode-preflight`, `crates/fleetd/src/server.rs:create_swarm#real-mode-preflight` |
| **risk** | L3 × I4 = **12** |
| **observations** | coverage_pts=2, branches=9 (branch_pts=3), churn_90d=26 (churn_pts=5) |
| **anchor_sites** | 2 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** All existing tests pass `mode: "demo"`. The `ANTHROPIC_API_KEY` check is the only
thing between a cockpit click and a real Docker + GitHub + paid-Anthropic run, and no test proves it
fires — nor that it fires **before** the row insert and driver spawn, an ordering the code comment
claims is load-bearing precisely so a bad request "never leaves a driverless unit in the map or a junk
row in the store". The 409 path from `spawn_driver_for` returning `AlreadyRegistered` is likewise never
executed. For swarms the blast radius multiplies by `CC_MAX_LANES`. The 503 docker path cannot run in
CI at all, but the 400-before-any-side-effect assertion needs no Docker.

**Concrete test.** With `ANTHROPIC_API_KEY` removed, call with `mode: "real"` and assert 400 **and**
that `list_units()` is still empty and no handle was registered. Repeat for `create_swarm`, and assert
a rejected request does not burn a swarm id.

### GAP-053 — `spawn_driver_for` and `rehydrate` duplicate the real-mode construction and both dispatch on `_`

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `duplicated-logic` |
| **layer** | rust-workspace |
| **verified_by** | automated |
| **anchors** | `crates/fleetd/src/server.rs:spawn_driver_for#real-arm`, `crates/fleetd/src/server.rs:rehydrate#real-arm`, `crates/fleetd/src/server.rs:resume_fan_out` |
| **risk** | L4 × I4 = **16** |
| **observations** | coverage_pts=4, branches=8 (branch_pts=3), churn_90d=26 (churn_pts=5) |
| **anchor_sites** | 3 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** The runner/forge construction is copy-pasted verbatim in two `_` arms, each
independently building the temp host-clone path, the `GhForge` title, and a **hardcoded**
`cc-agent:dev` image — note `bin/serve.rs` honours `CC_IMAGE` for reconciliation but these two sites do
not, so the drift already exists. Neither copy is executed by any test. The dangerous shared property:
both dispatch on `_`, not on `"real"`. `create_mission` validates the mode string, but `rehydrate`
(`row.mode`) and `resume_fan_out` (`sw.mode`) feed **persisted** values straight through — so any
unexpected mode value in SQLite becomes a real, billable Docker run at daemon startup.

**Concrete test.** Extract one `real_driver(spec, unit_id)` and assert mode dispatch is a whitelist, not
a fallthrough: persist a unit row with `mode: ""` or `"legacy"`, call `rehydrate`, and assert no driver
is spawned rather than a silent promotion to a real run.

### GAP-054 — `spawn_forwarder` discards store write errors, then broadcasts the event as if durable

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-branch` |
| **layer** | rust-workspace |
| **verified_by** | automated |
| **anchors** | `crates/fleetd/src/server.rs:spawn_forwarder#ignored-store-write-error` |
| **risk** | L3 × I4 = **12** |
| **observations** | coverage_pts=2, branches=6 (branch_pts=3), churn_90d=26 (churn_pts=5) |
| **anchor_sites** | 1 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** `let _ = s.append_event(...)` and `let _ = s.update_unit(...)`, then
`bcast.send(env)` regardless. The result is a live UI that has folded events which do not exist in the
store: refresh the cockpit or restart the daemon and they vanish, `last_seq` regresses, and the WS
replay disagrees with what the human just watched — with no log line, no error event, and no health
signal. The existing test covers only the happy-path projection fold, so the enclosing symbol looks
covered while the durability guarantee it exists to provide is unasserted.

**Concrete test.** Drive `spawn_forwarder` with a store whose `append_event` fails and assert the
failure is **observable** — the event is not broadcast as durable, or an `Event::Error{scope: System}`
is emitted.

### GAP-055 — `bin/serve.rs:main` has no tests, no graceful shutdown, and panics on a held port

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-symbol` |
| **layer** | rust-workspace |
| **verified_by** | automated |
| **anchors** | `crates/fleetd/src/bin/serve.rs:main` |
| **risk** | L4 × I4 = **16** |
| **observations** | coverage_pts=5, branches=1 (branch_pts=1), churn_90d=5 (churn_pts=3) |
| **anchor_sites** | 1 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** No inline tests, no integration file, and `cargo test --workspace` never links its
`main`. Two behaviours bite in the packaged product. **Port-in-use:** `TcpListener::bind(...).unwrap_or_else(|e| panic!(...))`
aborts, and `sidecar.rs:supervise` respawns every 2 s forever — while `health_gate` polls
`127.0.0.1:8787/health`, which the *other* already-running fleetd cheerfully answers, so the supervisor
emits `Ready` and the cockpit believes it is talking to its own sidecar while that sidecar crash-loops
against a different `CC_DB`. **Shutdown:** there is no `with_graceful_shutdown` and no signal handler
at all; the process is torn down by `CommandChild::kill()`, severing in-flight WS clients mid-write —
and Gate 5 already recorded an undiagnosed "process did not exit" anomaly (`GAP-010`). Also
unasserted: `reconcile_on_startup` runs against a real `LocalDockerRunner` **before** the bind, so a
hung `docker ps` delays listening past the supervisor's 20 s health gate.

**Concrete test.** `crates/fleetd/tests/serve_bootstrap_it.rs`: hold the port, start `serve`, assert a
non-zero exit with a diagnosable message rather than a bare panic; and assert a terminate signal
mid-mission leaves a coherent `last_seq` when a second process reads the same db.

### GAP-056 — `get_swarm` computes the swarm "done" verdict at read time with no test and no consumer

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-symbol` |
| **layer** | rust-workspace |
| **verified_by** | automated |
| **anchors** | `crates/fleetd/src/server.rs:get_swarm` |
| **risk** | L4 × I4 = **16** |
| **observations** | coverage_pts=4, branches=6 (branch_pts=3), churn_90d=26 (churn_pts=5) |
| **anchor_sites** | 1 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** The only place the swarm "done" verdict exists, computed rather than persisted, as
a three-way conjunction with zero direct coverage — the `run_swarm` tests assert the *stored* status and
call `swarm_rollup` directly, never this handler. A wrong verdict is how a human ends up believing a
swarm finished while lanes are still burning budget, or the reverse. `spent_so_far` — the number a
human would use to decide whether to keep going — is likewise computed only here, by iterating
`get_unit` per lane inside the held store lock, and asserted nowhere. And `grep -rn 'swarm'
cockpit/ui/src` returns nothing: the entire `/swarms` surface is a fully-implemented,
contract-unpinned API with no client to notice drift.

**Concrete test.** Build a swarm with two child units; assert `running` while one child is
non-terminal, `done` only when both are terminal, never `done` when `total == 0`, and
`spent_so_far == planner_cost + sum(child.cost)`. Pin the exact `SwarmDetail`/`LaneView` key sets.

### GAP-057 — The Tauri host crate is a standalone workspace, so CI never runs one of its tests

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-symbol` |
| **layer** | build_ci_gate |
| **verified_by** | automated |
| **anchors** | `cockpit/ui/src-tauri/Cargo.toml:[workspace]`, `Cargo.toml:workspace.members`, `.github/workflows/ci.yml:jobs.test` |
| **risk** | L4 × I5 = **20** |
| **observations** | coverage_pts=5, branches=0 (branch_pts=1), churn_90d=10 (churn_pts=4) |
| **anchor_sites** | 3 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** Verified from source: line 2 of `cockpit/ui/src-tauri/Cargo.toml` is a bare
`[workspace]` with the comment "Standalone workspace so the parent Cargo workspace doesn't claim this
crate", and the root members are only `crates/fleet-core` and `crates/fleetd`. So `cargo test
--workspace` in CI's `test` job never reaches the crate that contains the sidecar supervisor, the
`ccplugin://` handler, the webview embedding layer, the dashboard commands and the app-plugin runtime.
CI's only contact with it is compilation inside `tauri build`. **This entry is the multiplier on every
other `tauri_host` and `app_plugin_runtime` finding in this plan** — including the tests the concurrent
carve-out work is writing right now, which will not gate a PR either.

**Concrete test.** Add `cargo test --manifest-path cockpit/ui/src-tauri/Cargo.toml` to the `test` job.
Pair it with a repo-root guard that walks every `Cargo.toml`, collects those declaring their own
`[workspace]`, and asserts each is named in an allowlist CI is known to invoke — so the next standalone
crate cannot silently drop out.

### GAP-058 — The sidecar supervisor's restart loop has no test, no attempt cap, and no deadline

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `manual-uncovered` |
| **layer** | manual |
| **verified_by** | manual |
| **anchors** | `cockpit/ui/src-tauri/src/sidecar.rs:supervise` |
| **governs** | `cockpit/ui/src-tauri/src/sidecar.rs` |
| **last_manual_pass** | — (never_verified) |
| **risk** | L4 × I5 = **20** |
| **observations** | manual_coverage_pts=5, churn_90d=1 (churn_pts=2), never_verified=true |
| **anchor_sites** | 1 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** `sidecar.rs` contains no `#[cfg(test)]` module at all, and no smoke row exercises a
mid-session sidecar crash — the twelve rows cover teardown at quit and nothing else. The restart loop
has no attempt cap and no cumulative deadline: on a hard-crashing or unspawnable binary it cycles
`emit_status(Down)` → 2 s backoff → respawn forever, and the two `Err` arms (sidecar resolve failure,
spawn failure) each `continue` into that same uncapped loop. Combined with `GAP-055`, a port already
held by a stale fleetd produces a permanent crash-loop the operator cannot see.

**Concrete test.** Extract the decision as a pure `should_restart(shutting_down, exit_code, attempt) ->
Restart` and unit-test crash→restart, shutdown→stop, and attempt-cap behaviour with no real process.
Manual row: kill `fleetd-serve` from outside the app and confirm exactly one listener on 8787 afterwards.

### GAP-059 — `health_gate` does not restart on timeout, contradicting its own doc, and wedges the app in Starting

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-branch` |
| **layer** | tauri-host |
| **verified_by** | automated |
| **anchors** | `cockpit/ui/src-tauri/src/sidecar.rs:health_gate`, `cockpit/ui/src-tauri/src/sidecar.rs:pump_events#gate-spawn` |
| **risk** | L4 × I5 = **20** |
| **observations** | coverage_pts=5, branches=5 (branch_pts=2), churn_90d=1 (churn_pts=2) |
| **anchor_sites** | 2 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** **Phase-3 confirmed.** `HEALTH_GATE_TIMEOUT`'s doc says the gate waits "before
giving up on this attempt and restarting". The code does not restart: the gate runs on a detached
`spawn` whose failure arm is a lone `log::warn!` and whose result is never joined. `supervise` advances
only when the child's event stream closes, so a fleetd that starts but never binds — port 8787 already
held by a stale sidecar, a real scenario — leaves the app in `Starting` indefinitely with no `Down`
event and no restart. The success arm, the `_` fallthrough and the deadline arm are all unexercised.

**Concrete test.** Thread the timeout, poll interval and base URL through as parameters, then run
`health_gate` against a stub that never binds: assert it returns false at the deadline rather than
hanging, and assert the supervisor reacts with a Down/restart transition — which is what the constant's
doc promises.

### GAP-060 — `fleetd://status` is emitted to nobody

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-symbol` |
| **layer** | tauri-host |
| **verified_by** | automated |
| **anchors** | `cockpit/ui/src-tauri/src/sidecar.rs:emit_status`, `cockpit/ui/src-tauri/src/sidecar.rs:STATUS_EVENT` |
| **risk** | L3 × I5 = **15** |
| **observations** | coverage_pts=5, branches=0 (branch_pts=1), churn_90d=1 (churn_pts=2) |
| **anchor_sites** | 2 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** **Phase-3 confirmed.** Grepping the whole `cockpit/` tree for `fleetd://` returns
exactly two hits, both inside `sidecar.rs` itself. The only `listen(` subscriber in the frontend is
`plugin://state`. So the supervisor emits Starting/Ready/Down on a channel with zero subscribers, and
the only live liveness path is `api.ts:health` polling `/health`. In fairness the module docstring
hedges ("the event is an optimisation, not a hard dependency") — but nothing would notice if the event
name, the `rename_all = "lowercase"` serialisation, or the `skip_serializing_if` on `code` changed.

**Concrete test.** A serde round-trip on `StatusPayload`, plus a source-level contract test asserting
the `"fleetd://status"` literal appears in at least one frontend listener — which fails today and
documents the dead channel rather than leaving it to be rediscovered.

### GAP-061 — The `ccplugin://` response headers are three load-bearing security invariants with no assertion

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-symbol` |
| **layer** | tauri-host |
| **verified_by** | automated |
| **anchors** | `cockpit/ui/src-tauri/src/view_plugins.rs:built`, `cockpit/ui/src-tauri/src/view_plugins.rs:PLUGIN_CSP` |
| **risk** | L3 × I5 = **15** |
| **observations** | coverage_pts=5, branches=0 (branch_pts=1), churn_90d=1 (churn_pts=2) |
| **anchor_sites** | 2 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** `view_plugins.rs` has zero tests. Its module header names three findings that each
"cost a dropped handshake run to discover": the concrete-origin `script-src` (a
`sandbox="allow-scripts"` iframe has an opaque origin, so `'self'` matches nothing), the
`Access-Control-Allow-Origin: *` (opaque-origin module scripts are always fetched CORS-mode, so without
it `sdk.js` never runs and every handshake times out), and CSP as a **response header** rather than a
`<meta>`. All three live in two string literals with no assertion anywhere, and the failure mode is a
silent handshake timeout only a human in a watched window sees. **Cheapest high-value test in the
module** — `built` takes no `AppHandle`, so it is a plain `#[test]`.

**Concrete test.** Assert `built(200, "text/javascript", vec![])` and `not_found()` both carry
`Content-Security-Policy == PLUGIN_CSP`, that it contains `script-src 'self' http://ccplugin.localhost`
(not bare `'self'`) and `connect-src 'none'`, and that ACAO is `*` on **both** the 200 and 404 paths.

### GAP-062 — `view_plugins::respond` is the only guard between plugin URLs and `fs::read`, with 14 untested branches

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-branch` |
| **layer** | tauri-host |
| **verified_by** | automated |
| **anchors** | `cockpit/ui/src-tauri/src/view_plugins.rs:respond#path-traversal-and-id-validation` |
| **risk** | L4 × I5 = **20** |
| **observations** | coverage_pts=5, branches=14 (branch_pts=4), churn_90d=1 (churn_pts=2) |
| **anchor_sites** | 1 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** Fourteen branches of hand-rolled string validation guard `root.join(id).join(sub)`
before a filesystem read, and none is tested. Whether `request.uri().path()` arrives percent-decoded
(making `%2e%2e` a bypass) is **unverified** — precisely why a test should pin it. A Windows `id` such
as a drive prefix or a device name also reaches `Path::join` unchecked. And this Rust guard **disagrees
with the JavaScript one** (`GAP-088`): the JS side rejects any `..` substring while Rust rejects only
`..` *components*, and the JS side accepts a bare backslash the Rust side 404s. A manifest is
attacker-supplied in the `~/.command-center/plugins` drop-in case.

**Concrete test.** Extract `resolve(path) -> Option<(String, String)>` and table-drive it over `/`,
`/../etc/passwd`, `/id/../../secret`, `/id/a//b`, `/id/.`, `/id/a\b`, and percent-encoded `%2e%2e`;
assert `/id` defaults to `index.html` and `/id/sdk.js` takes the SDK branch.

### GAP-063 — Dev/packaged plugin-root precedence is the seam every remaining smoke row stands on, untested

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-branch` |
| **layer** | tauri-host |
| **verified_by** | automated |
| **anchors** | `cockpit/ui/src-tauri/src/view_plugins.rs:plugin_roots#dev-env-precedence`, `cockpit/ui/src-tauri/src/view_plugins.rs:sdk_bytes` |
| **risk** | L4 × I5 = **20** |
| **observations** | coverage_pts=5, branches=3 (branch_pts=2), churn_90d=1 (churn_pts=2) |
| **anchor_sites** | 2 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** `SPIKE-RESULTS.md` step 1 instructs the operator to set `CC_VIEW_PLUGINS_DEV` and
`CC_PLUGIN_SDK` before running the smoke, and rows 1.3, 1.4 and 1.10 are **unmeasurable** if resolution
silently falls through to the wrong root. First-hit-wins ordering across three optional sources is
exactly the logic that rots when a fourth root is added, and a wrong-root hit fails as "the plugin
rendered stale content", not as an error. Zero tests; the `USERPROFILE`/`HOME` fallback in particular is
only ever exercised on one OS at a time.

**Concrete test.** Refactor to `roots_from(dev, resource, home) -> Vec<PathBuf>` and assert the exact
ordering dev → `<resource>/plugins` → `<home>/.command-center/plugins`, that an absent env var drops
only its own entry, and that `USERPROFILE` wins over `HOME`. Same shape for `sdk_bytes`.

### GAP-064 — `WebviewPool::touch_and_evict` is the whole "no leak on switch" guarantee and is pure arithmetic nobody tests

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-symbol` |
| **layer** | tauri-host |
| **verified_by** | automated |
| **anchors** | `cockpit/ui/src-tauri/src/embedding.rs:WebviewPool::touch_and_evict` |
| **risk** | L4 × I5 = **20** |
| **observations** | coverage_pts=5, branches=3 (branch_pts=2), churn_90d=1 (churn_pts=2) |
| **anchor_sites** | 1 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** `embedding.rs` has zero tests. This function is the only thing bounding
native-webview memory and it is list arithmetic that unit-tests trivially — yet today the only way to
learn it is wrong is Task Manager. Two specific unasserted behaviours: `plugin_hide` parks a webview
**without touching the LRU**, so a parked-but-still-MRU plugin can be destroyed by three subsequent
shows and the user's next switch-back gets a cold reload instead of the promised warm render tree; and
nothing removes a label from `lru`/`last_rect` when a plugin is stopped, so stale labels occupy
warm-cap slots and evict live webviews.
*(A related claim — that the `lru` mutex is held across a blocking main-thread `close()` — was **Phase-3
refuted**: `close()` compiles to a non-blocking `send_event`, so the critical section is two channel
sends. Recorded so it is not re-flagged.)*

**Concrete test.** Split bookkeeping from destruction — `touch(label, rect) -> Vec<String>` returning
evicted labels — then assert with no Tauri runtime: A,B,C,D evicts [A]; re-showing A before D evicts
[B]; re-showing the MRU evicts nothing and does not duplicate; `last_rect` has no entry for an evicted
label; `lru.len()` never exceeds `WARM_CAP`.

### GAP-065 — The `app::<id>` webview-label scheme is encoded in three places with a "MUST" nobody enforces

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `duplicated-logic` |
| **layer** | tauri-host |
| **verified_by** | automated |
| **anchors** | `cockpit/ui/src-tauri/src/embedding.rs:app_label`, `cockpit/ui/src-tauri/capabilities/default.json:app-plugins`, `cockpit/ui/src-tauri/src/embedding.rs:HOST_WINDOW_LABEL` |
| **risk** | L4 × I5 = **20** |
| **observations** | coverage_pts=5, branches=0 (branch_pts=1), churn_90d=4 (churn_pts=3) |
| **anchor_sites** | 3 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** `format!("app::{id}")`, the `"app::*"` glob in `capabilities/default.json`, and the
module header's contract are three independent copies, and the capability comment literally says the
glob "MUST match the webview-label scheme" — a MUST with no enforcement. Renaming the prefix compiles,
bundles, and passes every gate; it surfaces only as an app-plugin webview silently losing permissions
at runtime. `HOST_WINDOW_LABEL = "main"` has the same shape: a guess about a label `tauri.conf.json`
never states, degrading to a user-visible error string when `get_window` returns `None`.
*(A related claim — that the `app-plugins` capability over-grants webview-mutating permissions to
third-party app content — was **Phase-3 refuted**: the capability omits `remote`, and the child webview
loads an external URL, so Tauri classifies it `Origin::Remote` and the Local-only grants never resolve.
The capability is inert rather than dangerous. Recorded so it is not re-flagged.)*

**Concrete test.** `include_str!` the capability file and assert every `webviews` glob, with its
trailing `*` stripped, is a prefix of `app_label("x")`; assert `HOST_WINDOW_LABEL` appears in the
`default` capability's `windows` array and that `tauri.conf.json` declares exactly one window.

### GAP-066 — The `ccplugin://` origin is written three ways, and the CSP form is Windows-only

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `duplicated-logic` |
| **layer** | tauri-host |
| **verified_by** | automated |
| **anchors** | `cockpit/ui/src-tauri/tauri.conf.json:app.security.csp`, `cockpit/ui/src-tauri/src/view_plugins.rs:PLUGIN_CSP`, `cockpit/ui/src/lib/loader.ts:pluginSrc` |
| **risk** | L4 × I5 = **20** |
| **observations** | coverage_pts=5, branches=0 (branch_pts=1), churn_90d=3 (churn_pts=3) |
| **anchor_sites** | 3 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** **Phase-3 confirmed against the vendored Tauri source.** `pluginSrc` emits
`ccplugin://localhost/${id}/${entry}` and `App.svelte` uses that literal as the iframe `src` in both
dev and packaged; the host CSP is a single static string ending `frame-src http://ccplugin.localhost`
with no platform variants and no rewrite anywhere; and Tauri's own docs state the URL form is
`http://<scheme>.localhost` on **Windows/Android** but `<scheme>://localhost` on **macOS/iOS/Linux**.
Tauri's runtime CSP augmentation only injects nonces into `script-src`/`style-src`, never adds custom
schemes to `frame-src`. `release.yml` builds all three OSes. So view-plugins would be CSP-blocked on
macOS and Linux, discovered by a user. CI's three-OS matrix only runs `tauri build` and never launches
the app; smoke rows 1.3/1.10 were Windows-only and are recorded "not run".

**Concrete test.** Parse `tauri.conf.json` and assert its `frame-src` admits **every** origin form
`pluginSrc` can emit — including the raw `ccplugin://localhost` form — and that the origin token in
`PLUGIN_CSP`'s `script-src` is byte-identical to the one in `frame-src`. Mirror it in `loader.test.ts`.

### GAP-067 — `127.0.0.1:8787` is hand-mirrored in four places and only one of them honours `CC_ADDR`

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `duplicated-logic` |
| **layer** | tauri-host |
| **verified_by** | automated |
| **anchors** | `cockpit/ui/src-tauri/src/sidecar.rs:FLEETD_ADDR`, `cockpit/ui/src-tauri/tauri.conf.json:app.security.csp`, `cockpit/ui/src/lib/api.ts:BASE`, `crates/fleetd/src/bin/serve.rs:main#cc-addr` |
| **risk** | L4 × I5 = **20** |
| **observations** | coverage_pts=5, branches=0 (branch_pts=1), churn_90d=5 (churn_pts=3) |
| **anchor_sites** | 4 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** The `sidecar.rs` comment says it "mirrors serve.rs" — a mirror maintained by hand.
`serve.rs` honours `CC_ADDR`; the host does not. So setting `CC_ADDR` produces a running daemon the
supervisor health-gates against forever on the old port (never `Ready`, and per `GAP-059` never
restarted) while the CSP simultaneously blocks the frontend from the new one. Silent, three-symptom,
asserted nowhere.

**Concrete test.** Read `tauri.conf.json` and assert its `connect-src` contains both
`http://{FLEETD_ADDR}` and `ws://{FLEETD_ADDR}` built from the sidecar constant. Better: make
`FLEETD_ADDR` read `CC_ADDR` with the same default as `serve.rs` and test that override end to end.

### GAP-068 — The updater is registered against an empty pubkey and a `.example` endpoint

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-symbol` |
| **layer** | tauri-host |
| **verified_by** | automated |
| **anchors** | `cockpit/ui/src-tauri/tauri.conf.json:plugins.updater`, `cockpit/ui/src-tauri/src/lib.rs:run#updater-registration` |
| **risk** | L4 × I5 = **20** |
| **observations** | coverage_pts=5, branches=2 (branch_pts=1), churn_90d=10 (churn_pts=4) |
| **anchor_sites** | 2 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** **Phase-3 confirmed.** `pubkey` is the empty string and the single endpoint host is
a reserved `.example` name; `lib.rs` registers the plugin unconditionally with no `Builder::pubkey`
override, and no build step patches the config. Meanwhile `release.yml` injects
`TAURI_SIGNING_PRIVATE_KEY` for all three OSes — so a tagged build produces **signed artifacts whose
signature can never verify**, pointed at a nonexistent host. `tauri build` validates bundling, not
updater semantics, and no smoke row mentions the updater. `lib.rs` has zero tests.

**Concrete test.** A config-coherence test asserting `plugins.updater.pubkey` is non-empty, that no
endpoint host ends in `.example`/`.invalid`, and that every endpoint retains its
`{{target}}/{{arch}}/{{current_version}}` placeholders. Pair with a `release.yml` assertion that signing
key and verifying pubkey cannot be configured one-sidedly.

### GAP-069 — `lib.rs:run`'s ExitRequested ordering is load-bearing and enforced only by statement order

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-symbol` |
| **layer** | tauri-host |
| **verified_by** | automated |
| **anchors** | `cockpit/ui/src-tauri/src/lib.rs:run#exit-requested`, `cockpit/ui/src-tauri/src/lib.rs:run#generate-handler` |
| **risk** | L4 × I5 = **20** |
| **observations** | coverage_pts=5, branches=2 (branch_pts=1), churn_90d=10 (churn_pts=4) |
| **anchor_sites** | 2 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** `lib.rs` has zero tests and is a ratified core entry point. The handler's comment
states the sidecar must be reaped first "so the supervisor doesn't respawn it as we tear down" — an
ordering nothing enforces. Smoke row 1.9b (`GAP-010`) recorded a live, undiagnosed anomaly here, which
is precisely the signal this contract deserves a mechanical guard. The registration block is the other
silent-drift surface: adding a command and forgetting the `generate_handler!` line compiles clean and
fails only as a runtime "command not found" in the webview.

**Concrete test.** A source-structure guard (same technique the concurrent
`tests/tauri_command_threading.rs` uses): assert `SidecarSupervisor::shutdown` appears at an earlier
byte offset than `stop_all_owned`, which precedes `app_handle.exit(0)`; and assert every name in
`generate_handler!` resolves to a real `#[tauri::command]` in the tree.

### GAP-070 — `run_halyard` shells out with no timeout from a synchronous Tauri command

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-symbol` |
| **layer** | tauri-host |
| **verified_by** | automated |
| **anchors** | `cockpit/ui/src-tauri/src/dashboard.rs:run_halyard`, `cockpit/ui/src-tauri/src/dashboard.rs:halyard_status`, `cockpit/ui/src-tauri/src/local_projects.rs:scan_local_projects` |
| **risk** | L3 × I5 = **15** |
| **observations** | coverage_pts=4, branches=1 (branch_pts=1), churn_90d=2 (churn_pts=2) |
| **anchor_sites** | 3 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** **This is the same defect class as the 1.5 freeze, in commands nobody has looked
at.** `halyard_status`/`halyard_queue` are synchronous `#[tauri::command] pub fn` calling
`Command::output()` with no timeout; `scan_local_projects` is synchronous and does a bounded-recursive
`walkdir` over whole scan roots plus a full read of every STATUS.md/ROADMAP.md. Sync Tauri commands run
on the main event-loop thread. `Dashboard.svelte` fires all three on mount **and again every 15 s**, so
the freeze recurs. The concurrent `tests/tauri_command_threading.rs` guard already names this as
tolerated debt — it asserts signatures, so it can never go red when the hang happens. Nothing else
gates it: this crate's tests do not run in CI (`GAP-057`).

**Concrete test.** Point `HALYARD_BIN` at a stub and assert the three failure shapes (binary absent,
non-zero exit with stderr, non-JSON stdout); then the one that matters — a `sleep 30` stub must return
within a bounded time, which fails today and forces a timeout to exist.

### GAP-071 — The Audience HTTP commands have no client timeout and an unasserted error-policy asymmetry

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-symbol` |
| **layer** | tauri-host |
| **verified_by** | automated |
| **anchors** | `cockpit/ui/src-tauri/src/dashboard.rs:audience_health`, `cockpit/ui/src-tauri/src/dashboard.rs:audience_posts` |
| **risk** | L3 × I5 = **15** |
| **observations** | coverage_pts=4, branches=3 (branch_pts=2), churn_90d=1 (churn_pts=2) |
| **anchor_sites** | 2 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** `reqwest::Client::new()` with no `.timeout()`, so a backend that accepts the socket
and stalls leaves the invoke promise pending forever — and `Dashboard.svelte:refresh()` awaits
sequentially, so `pulling` never clears, REFRESH stays disabled, and the unrelated LOCAL lane never
polls. Only `unwrap_posts` is tested; neither transport is. The two commands also take **opposite**
error policies (`audience_health` swallows every transport error into `Ok(false)`, `audience_posts`
propagates) and nothing pins that as deliberate, so a refactor could flip a down backend into a hard
failure the adapter is not written to handle.

**Concrete test.** Stand a `tokio` test HTTP server and assert: `/health` 200 → `Ok(true)`; 500 →
`Ok(false)`; connection refused → `Ok(false)` not `Err`; `/posts` 503 → `Err` with the status; HTML body
→ `Err` "not JSON"; `{"posts":[…]}` → the unwrapped array.

### GAP-072 — `local_projects`' exclusion list and depth bound are the only brakes on a whole-disk walk, and neither is tested

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-branch` |
| **layer** | tauri-host |
| **verified_by** | automated |
| **anchors** | `cockpit/ui/src-tauri/src/local_projects.rs:discover#excludes-and-max-depth`, `cockpit/ui/src-tauri/src/local_projects.rs:scan_local_projects#pin-dedup` |
| **risk** | L2 × I5 = **10** |
| **observations** | coverage_pts=2, branches=5 (branch_pts=2), churn_90d=2 (churn_pts=2) |
| **anchor_sites** | 2 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** All four tests pass `max_depth: 5, excludes: vec![]`, so `is_excluded` is never
exercised with a non-empty list and the depth bound is never observed truncating anything — while
`scan_local_projects` is itself flagged unbounded main-thread debt whose cost scales with the
operator's disk. Separately the pin dedup arm (`if discovered.contains(&norm) { continue }`) is never
taken, and its correctness rests on a case-**sensitive** comparison on Windows, where `D:/proj` and
`d:/proj` are the same directory — so the same project can appear twice on the board, and a pinned
directory that is also discovered silently keeps `is_pinned: false`.

**Concrete test.** Build `root/deep/a/b/c/d/docs/STATUS.md` and assert it is found at depth 8 and absent
at 3; build `root/vendor/thing/docs/STATUS.md` and assert `excludes: ["vendor"]` drops it; pass a pin
with backslashes against forward-slash discovery and assert dedup still holds.

### GAP-073 — `App.svelte`'s app-plugin compositing effect never exercises the overlay park/restore pair

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-branch` |
| **layer** | vitest |
| **verified_by** | automated |
| **anchors** | `cockpit/ui/src/App.svelte:$effect#app-plugin-compositing` |
| **risk** | L3 × I5 = **15** |
| **observations** | coverage_pts=4, branches=4 (branch_pts=2), churn_90d=8 (churn_pts=4) |
| **anchor_sites** | 1 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** The jsdom half of smoke row 1.7 (`GAP-007`) and it is fully automatable: the
overlay-open branch (`if (overlayOpen) void invoke('plugin_hide')`) and the restore branch are pure
signal emission across the Tauri boundary, observable through the invoke mock this test file already
installs. The two existing tests cover only the healthy/not-healthy gate; neither ever opens an
overlay. Same class as the defect that froze the UI — a native-boundary contract the UI half must
honour. Note the pin would still be **local-only** until `npm test` reaches CI (`GAP-110`).

**Concrete test.** Activate audience, emit `healthy`, assert one `plugin_show`; call
`fleet.requestRealLaunch(...)`, assert one `plugin_hide` and no `plugin_show` while open; cancel and
assert `plugin_show` count is 2. Plus: no re-issue of `plugin_hide` on an unrelated `pluginState` write.

### GAP-074 — The ResizeObserver rect-glue effect is asserted nowhere, teardown included

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-symbol` |
| **layer** | vitest |
| **verified_by** | automated |
| **anchors** | `cockpit/ui/src/App.svelte:$effect#rect-glue-resizeobserver` |
| **risk** | L3 × I5 = **15** |
| **observations** | coverage_pts=4, branches=2 (branch_pts=1), churn_90d=8 (churn_pts=4) |
| **anchor_sites** | 1 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** Two thirds of smoke row 1.6 and the leak half of 1.8. `App.appPlugin.test.ts`
explicitly stubs `ResizeObserver` to a no-op and defers this to the checklist, so nothing asserts that
an observer is attached, that the callback marshals a well-formed rect, or — the high-value one — that
the effect's teardown **disconnects** when `activeApp` goes null. An undisconnected observer holding a
detached rect element is exactly the leak 1.8 is looking for, and a human eyeballing a window will
never see it.

**Concrete test.** Stub `ResizeObserver` with a class capturing the callback and recording
`observe`/`disconnect`; assert `observe` on the rect element, that firing the callback emits
`plugin_set_rect` with all four keys, and that switching to FLEET calls `disconnect` exactly once and
stops further emissions.

### GAP-075 — No test in the repo ever mounts a view-plugin iframe from `App.svelte`

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-symbol` |
| **layer** | vitest |
| **verified_by** | automated |
| **anchors** | `cockpit/ui/src/App.svelte:$effect#view-plugin-bridge`, `cockpit/ui/src/App.svelte:onSwitch#view-prefix-arm` |
| **risk** | L3 × I5 = **15** |
| **observations** | coverage_pts=4, branches=2 (branch_pts=1), churn_90d=8 (churn_pts=4) |
| **anchor_sites** | 2 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** `bridge.test.ts` unit-tests `PluginBridge` in isolation and no App-level test ever
activates a view-plugin — no test in `src/` references `view-plugin-frame` or `switch-view:` at all. So
the `view:` arm of `onSwitch`, the entry point to the entire sandboxed-plugin runtime, is unexecuted.
The bridge registers a window `message` listener and a 60 ms interval released only via `destroy()`, so
if the effect teardown ever stops firing, every FLEET↔REFERENCE round trip leaks a listener plus a
timer draining a store for an unmounted frame. The `sandbox="allow-scripts"` assertion is cheap
insurance on a security-shaped invariant with zero automated protection today.

**Concrete test.** `vi.mock('./lib/bridge')` so `PluginBridge` records constructor args and a `destroy`
spy; assert the iframe mounts with `sandbox === 'allow-scripts'` (and **not** `allow-same-origin`) and a
`ccplugin://` src, that exactly one bridge is constructed, and that switching away calls `destroy` once
and unmounts. Re-enter and assert no accumulation.

### GAP-076 — `onKill` — the plugin-misbehaviour escape hatch — has no App-level test

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-branch` |
| **layer** | vitest |
| **verified_by** | automated |
| **anchors** | `cockpit/ui/src/App.svelte:$effect#view-plugin-bridge#onkill-fallback` |
| **risk** | L3 × I5 = **15** |
| **observations** | coverage_pts=4, branches=2 (branch_pts=1), churn_90d=8 (churn_pts=4) |
| **anchor_sites** | 1 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** An inbound message flood must dump the untrusted iframe and fall back to the
trusted ops grid. `onKill` sets two pieces of state from inside an effect keyed on the first of them —
a self-invalidating write whose correctness depends on Svelte 5 effect re-entrancy doing the teardown
before the re-run. `bridge.test.ts` can only prove the callback fires, not what the shell does with it,
and the hostile-plugin kill path falls between every manual row (1.3/1.4 are the happy path, 1.7 is
app-plugin parking, 1.8 is leak-on-switch). A human smoke run would essentially never trigger it.

**Concrete test.** Capture the options passed to the mocked `PluginBridge`, invoke `options.onKill()`,
`await tick()`, then assert the iframe is gone, the Fleet ops grid is rendered, and
`switch-fleet` has `aria-pressed === "true"`.

### GAP-077 — `selectApp` has no in-flight guard, so a double-click starts two docker builds

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-branch` |
| **layer** | vitest |
| **verified_by** | automated |
| **anchors** | `cockpit/ui/src/App.svelte:selectApp#no-in-flight-guard` |
| **risk** | L3 × I5 = **15** |
| **observations** | coverage_pts=4, branches=3 (branch_pts=2), churn_90d=8 (churn_pts=4) |
| **anchor_sites** | 1 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** **Phase-3 confirmed on both sides.** UI: `activeApp` is set synchronously, then
`await tick()`, then a `pluginState[id] === 'healthy'` check — and `pluginState` is only ever written by
the async `plugin://state` listener, so a second click during `starting`/`building` fails the check and
dispatches again. The tab is a plain `<button>` with no `disabled` and no same-id short-circuit. Rust:
`plugin_launch` looks the id up and immediately `thread::spawn`s the start sequence; the `running` map
is consulted nowhere on the launch path and is only inserted into *after* `Healthy`. So both threads
run `docker compose build` against the same project. Direct descendant of the 1.5 freeze, and exactly
what a careful human smoke tester (one deliberate click) will never reproduce. *(The Rust half was read
while another agent had `manager.rs` modified in the working tree.)*

**Concrete test.** Fire two clicks on the app tab back to back without awaiting between them, then
`waitFor` and assert `calls('plugin_launch').length === 1`. Also add the app→app hand-off case (two app
tabs: assert `plugin_hide` on the outgoing before any `plugin_launch` on the incoming) and the
already-healthy re-entry short-circuit.

### GAP-078 — `api.ts` has no test file at all, and `openStream` wires no close or error handler

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-symbol` |
| **layer** | vitest |
| **verified_by** | automated |
| **anchors** | `cockpit/ui/src/lib/api.ts:openStream`, `cockpit/ui/src/lib/api.ts:createMission`, `cockpit/ui/src/lib/api.ts:sendCommand` |
| **risk** | L4 × I5 = **20** |
| **observations** | coverage_pts=5, branches=3 (branch_pts=2), churn_90d=2 (churn_pts=2) |
| **anchor_sites** | 3 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** **Phase-3 confirmed.** `api.ts` is mocked away wholesale by every test, so its real
code is executed by nothing. `openStream` sets only `onmessage` — a repo-wide grep for
`onclose|onerror` returns nothing — and `FleetStore.start()` latches `started = true` with `reconnect()`
called exactly once, while `ensureStream` refuses to reopen a socket already in its map (a closed
socket is still truthy). Daemon health is fetched only inside that one `reconnect`, so the ◉ DOCKER / ◉
KEY badges keep their last green value. This is a **missing feature**, not just a missing test: the
cockpit becomes silently, confidently dead when the sidecar restarts. Also unpinned: `sendCommand`
returns the raw status while `FleetStore.cmd` discards it, so a daemon-rejected halt looks identical to
a successful one.

**Concrete test.** New `cockpit/ui/src/lib/api.test.ts` with a stubbed `WebSocket`/`fetch`: assert the
`http→ws` URL rewrite and `since` param, that a malformed frame is swallowed, that the three `!res.ok`
guards throw with status and body, that `sendCommand` does **not** throw on 409 — and then pin the
socket-lifecycle contract, which is where the intended behaviour gets decided.

### GAP-079 — `FleetStore.dispose` leaves the store un-restartable

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-symbol` |
| **layer** | vitest |
| **verified_by** | automated |
| **anchors** | `cockpit/ui/src/lib/store.svelte.ts:dispose`, `cockpit/ui/src/lib/store.svelte.ts:start#started-latch` |
| **risk** | L3 × I5 = **15** |
| **observations** | coverage_pts=4, branches=0 (branch_pts=1), churn_90d=3 (churn_pts=3) |
| **anchor_sites** | 2 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** **Phase-3 confirmed.** `dispose()` closes the sockets but does not clear
`this.sockets` and does not reset `this.started`; it is `App.svelte`'s `onMount` teardown, and `fleet`
is a module-level singleton whose `units`/`order` survive the unmount. So a remount — Vite HMR of
`App.svelte`, or any second mount — takes the `if (this.started) return` early-out and `ensureStream`'s
truthy-dead-socket guard: the cockpit comes back fully populated and permanently frozen, no error, no
reconnect. Only a human doing an HMR reload would see it, and smoke row 1.10 only checks that HMR loads
at all.

**Concrete test.** `reconnect()` with two units, `dispose()`, assert both socket `close` spies fired;
then `start()` again and assert `openStream` was called twice more. Red today — pin the intended
contract.

### GAP-080 — `fleet.ts:fold` matches Rust-side reason strings by exact equality, with no test on either side

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-branch` |
| **layer** | vitest |
| **verified_by** | automated |
| **anchors** | `cockpit/ui/src/lib/fleet.ts:fold#blocked-reason-literals`, `crates/fleetd/src/retry.rs:RL_REASON`, `crates/fleetd/src/driver.rs:drive#awaiting-slot-reason` |
| **risk** | L3 × I5 = **15** |
| **observations** | coverage_pts=4, branches=3 (branch_pts=2), churn_90d=19 (churn_pts=5) |
| **anchor_sites** | 3 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** `fleet.ts` has no test file and the `blocked` arm is never folded by any test. Its
two flags are set by **exact string equality** against literals produced on the Rust side —
`RL_REASON = "rate limited"` and an inline `"awaiting concurrency slot"`. Nothing on either side
enforces the match: `cargo test --workspace`, the one logic gate CI actually has, can rename the reason
and stay green while the ◷ SLOT / ⏳ RATE-LIMIT chips silently stop appearing and a unit looks hung with
no explanation. Cross-language stringly-typed coupling with a green build on both sides is the worst
shape a gap can have here.

**Concrete test.** Fold a `Blocked` with each exact literal and assert the corresponding flag; fold an
unrecognised reason and assert both stay false while `blocked` is still set; fold any `phase_changed`
and assert both clear. Add a comment naming the two Rust producers, and ideally a Rust-side assertion
that those literals are the ones emitted.

### GAP-081 — `phaseClass` and `progress` drive every tile's colour and rail and have no direct assertion

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-symbol` |
| **layer** | vitest |
| **verified_by** | automated |
| **anchors** | `cockpit/ui/src/lib/fleet.ts:phaseClass`, `cockpit/ui/src/lib/fleet.ts:progress` |
| **risk** | L4 × I5 = **20** |
| **observations** | coverage_pts=4, branches=6 (branch_pts=3), churn_90d=3 (churn_pts=3) |
| **anchor_sites** | 2 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** Both run on every tile render; neither has a direct assertion anywhere, executing
only incidentally when a test seeds a unit and renders. `progress()` returns `i/(order.length-1)` from
an `indexOf` over a hand-maintained array that **omits** `needs_human`/`halted`/`no_change` entirely —
those fall to the `i < 0 ? 0` arm and render a zero-width rail, which may or may not be intended and is
currently undocumented and unpinned. Adding a phase to `types.ts` or the Rust enum silently lands it in
the idle bucket. Cheap, high-fanout end of the row-1.2 canary.

**Concrete test.** Table-drive every member of the `Phase` union through `phaseClass` asserting the
expected bucket; assert `progress` is monotonic non-decreasing along the happy path, that
`progress('failed') === 1`, and that an unknown phase yields 0 rather than a negative width.

### GAP-082 — Phase-eligibility policy for the action buttons lives in three places with no assertion on any

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `duplicated-logic` |
| **layer** | vitest |
| **verified_by** | automated |
| **anchors** | `cockpit/ui/src/App.svelte:canHalt`, `cockpit/ui/src/App.svelte:canResume`, `cockpit/ui/src/lib/fleet.ts:ATTENTION` |
| **risk** | L3 × I5 = **15** |
| **observations** | coverage_pts=4, branches=3 (branch_pts=2), churn_90d=8 (churn_pts=4) |
| **anchor_sites** | 3 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** `canHalt` inlines `['needs_human','halted','awaiting_oracle_approval']` —
byte-for-byte the `ATTENTION` array in `fleet.ts` — and `canResume` re-derives two thirds of it with an
`||`. The same knowledge exists a third time in the daemon, which is the actual authority, and
`sendCommand`'s status is discarded (`GAP-078`), so a daemon rule change leaves the UI offering a verb
that is silently rejected. Zero assertions on any copy. `GAP-020` is the destructive case this enables.

**Concrete test.** Drive one selected unit through every `Phase` and assert the enabled/disabled state
of RESUME/SHIP/HALT/ABANDON as a table; then assert the set `canHalt` excludes equals `fleet.ts`'s
`ATTENTION` set, so changing one without the other goes red.

### GAP-083 — Capabilities are negotiated, thrown away, and never enforced

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `duplicated-logic` |
| **layer** | vitest |
| **verified_by** | automated |
| **anchors** | `cockpit/ui/src/lib/loader.ts:negotiateCapabilities`, `cockpit/ui/src/lib/bridge.ts:PluginBridge.onWindowMessage#capabilities-fallback`, `cockpit/ui/src/App.svelte:$effect#view-plugin-bridge` |
| **risk** | L3 × I4 = **12** |
| **observations** | coverage_pts=4, branches=1 (branch_pts=1), churn_90d=8 (churn_pts=4) |
| **anchor_sites** | 3 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** **Phase-3 confirmed.** `negotiateCapabilities` intersects manifest ∩ host into
`DiscoveredPlugin.grantedCapabilities`, and a repo-wide grep shows that field has **no production
consumer**. `App.svelte` builds the bridge with only `{ onKill }`, so
`this.opts.capabilities ?? [...HOST_CAPABILITIES]` advertises the **full** host set: the reference
plugin requests only `log-append` and is told it also holds `real-launch-confirm`. And
`PluginSession` never consults capabilities for anything except the `advertisedCapabilities` getter —
they are advisory strings with no enforcement point anywhere. `loader.test.ts`'s green negotiation
tests actively create false confidence that negotiation is in effect.

**Concrete test.** Run the reference manifest through `discoverFrom` and construct the bridge the way
`App.svelte` does (no `capabilities`), then assert the captured `init.capabilities` equals the
negotiated `['log-append']`. Red today, and that is the point.

### GAP-084 — The bridge's rate/flood buckets are never driven end to end

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-branch` |
| **layer** | vitest |
| **verified_by** | automated |
| **anchors** | `cockpit/ui/src/lib/bridge.ts:PluginSession.onCommand#rate-limited`, `cockpit/ui/src/lib/bridge.ts:PluginSession.onCommand#sink-error` |
| **risk** | L3 × I4 = **12** |
| **observations** | coverage_pts=4, branches=7 (branch_pts=3), churn_90d=1 (churn_pts=2) |
| **anchor_sites** | 2 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** `TokenBucket` is unit-tested in isolation but no test drives a session past a
bucket, so the **wiring** is unproven: which bucket is charged for which verb, the capacities, and the
`!launchBucket.take() || !bucket.take()` short-circuit that burns a launch token when the general
bucket is dry. Swap the operands and a plugin's launch verb is silently un-rate-limited with no test
failing. Both `sink-error` catch arms are equally dead — and that is the *realistic* path, since the
sinks hit a live HTTP daemon: if the ack were dropped instead of sent, the SDK's `pending` map never
resolves and the plugin's `await fleet.launch(...)` hangs forever with no visible failure. Manual row
1.4 (`GAP-004`) is marked "not run", so rate limiting is verified by neither machine nor human.

**Concrete test.** Construct a session with tiny bucket capacities and `autoTick: false`, post three
launches, assert acks 1–2 `{ok:true}` and ack 3 `{ok:false, reasonClass:'rate-limited'}`. Separately
give the host a rejecting `launch`/`command` and assert both acks are `sink-error` with no unhandled
rejection escaping the `void this.onCommand(m)` call site.

### GAP-085 — The shipped `autoTick: true` default path executes in zero tests

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-branch` |
| **layer** | vitest |
| **verified_by** | automated |
| **anchors** | `cockpit/ui/src/lib/bridge.ts:PluginSession.onReady#autotick-timer`, `cockpit/ui/src/lib/bridge.ts:PluginSession.onReady#duplicate-ready` |
| **risk** | L3 × I4 = **12** |
| **observations** | coverage_pts=4, branches=2 (branch_pts=1), churn_90d=1 (churn_pts=2) |
| **anchor_sites** | 2 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** Every existing session test passes `{ autoTick: false }`, so the default
`App.svelte` actually uses is unexecuted. Two things are unproven: that the 60 ms tick timer starts at
all (if it silently didn't, the plugin would render only the initial snapshot and look frozen —
precisely what a human would have to catch in row 1.3), and that `clearInterval` in `kill()` releases
it. Worse, `if (this.ready) return` is the only thing stopping a hostile plugin from starting **one
interval per `ready`** — an unbounded timer leak that survives `kill()` because only the last timer is
cleared, and 50 readys/sec sits under the 200 flood ceiling.

**Concrete test.** With fake timers and defaults: drive `ready`, mark a unit dirty, advance 35 ms and
assert 3+ delta pushes; `destroy()`, advance, and assert no further `state` and
`vi.getTimerCount()` back to baseline. Then post `ready` 50 more times and assert zero additional
`state` messages and no timer growth.

### GAP-086 — Hostile-input handling on the port is exercised only through pure-function tests

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-branch` |
| **layer** | vitest |
| **verified_by** | automated |
| **anchors** | `cockpit/ui/src/lib/bridge.ts:PluginSession.handleMessage#version-skew-drop`, `cockpit/ui/src/lib/bridge.ts:PluginBridge.onWindowMessage#hostile-hello` |
| **risk** | L3 × I4 = **12** |
| **observations** | coverage_pts=4, branches=7 (branch_pts=3), churn_90d=1 (churn_pts=2) |
| **anchor_sites** | 2 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** `if (!m || m.v !== PROTOCOL_VERSION) return` is exercised only through
`policeCommand`'s pure test, never over the port — and the difference matters: over the port a
v-mismatched message is dropped **silently** with no ack, so a v2 plugin's every command hangs its
promise forever with no diagnostic. Nobody has chosen that behaviour deliberately. On the bridge side,
the `window.addEventListener` wiring in the constructor is executed by no assertion at all: if the
constructor stopped registering the listener, every existing test still passes and the handshake dies
only in the real app — a row-1.3 failure. The `e.source === frame.contentWindow` check is the *only*
auth available, since a sandboxed iframe's origin is `"null"`.

**Concrete test.** Dispatch a real `MessageEvent` on `window` (source is null in jsdom) and assert no
session is created and nothing was posted — proving the constructor's listener is wired **and** that a
non-frame sender is rejected. Then table-drive `{v:2,...}`, `{v:1,type:'wat'}`, `null`, a string, `{}`
over the port and assert no ack, no sink call, `isAlive` still true, and no exception.

### GAP-087 — `policeCommand`'s only numeric bound, `min_review_rounds`, is untested

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-branch` |
| **layer** | vitest |
| **verified_by** | automated |
| **anchors** | `cockpit/ui/src/lib/bridge.ts:policeCommand#min-review-rounds-bounds`, `cockpit/ui/src/lib/bridge.ts:policeCommand#reqid-validation` |
| **risk** | L4 × I4 = **16** |
| **observations** | coverage_pts=4, branches=27 (branch_pts=5), churn_90d=1 (churn_pts=2) |
| **anchor_sites** | 2 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** `policeCommand` is the trust boundary and the best-covered thing in the module —
which is exactly why the residual holes are the dangerous ones. `min_review_rounds` is the only numeric
bound in the whole policy and **no test touches it**, so the four-way guard
(`typeof`/`isInteger`/`<1`/`>6`) is unexecuted: a plugin passing `1e9` or `0` reaches `store.launch`
and then the daemon with an out-of-range review budget — a cost lever an untrusted frame should not
hold. `reqId` validation, which is what makes ack correlation forgery-resistant, is likewise untested.
Both are pure-function assertions and both are sub-assertions of manual row 1.4 that can be retired
from the human gate.

**Concrete test.** Table-drive `min_review_rounds` over `0, -1, 7, 1.5, NaN, Infinity, '3', null, 6, 1`
expecting `over-bound` for the first eight; assert `undefined` still defaults to 2. Separately assert
`reqId: ''`, a number, absent, and a 100 KB string all reject `malformed`, and that a malformed command
with a non-string `reqId` still produces an ack with `reqId: ''` rather than throwing.

### GAP-088 — `loader.ts`'s traversal guard has one test case and disagrees with the Rust guard

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-branch` |
| **layer** | vitest |
| **verified_by** | automated |
| **anchors** | `cockpit/ui/src/lib/loader.ts:validateManifest#entry-traversal-variants`, `cockpit/ui/src-tauri/src/view_plugins.rs:respond#component-check` |
| **risk** | L4 × I4 = **16** |
| **observations** | coverage_pts=4, branches=15 (branch_pts=4), churn_90d=1 (churn_pts=2) |
| **anchor_sites** | 2 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** The guard is a single line (`entry.includes('..') || entry.startsWith('/')`) with
exactly one test case. The untested variants matter because **the JS and Rust gates accept different
strings**: JS rejects any `..` substring while Rust rejects only `..` *components*; JS accepts a bare
backslash entry that Rust 404s. Nothing tests that they agree, and the Rust half has no tests at all
(`GAP-062`). A manifest is attacker-supplied in the `~/.command-center/plugins` drop-in case.

**Concrete test.** Table-drive `entry` over `'..\\escape.html'`, `'a/../../etc/passwd'`, `'%2e%2e/x'`,
`'.'`, `'sub\\index.html'`, `'http://evil/x'`, `'//evil/x'`, `'javascript:alert(1)'`,
`'index.html?../../x'`; for anything accepted, feed it through `pluginSrc` and assert the URL still has
a literal `ccplugin://localhost/<id>/` prefix with no `..` segment. Mirror the corpus in the Rust test.

### GAP-089 — The shipped SDK has no lifetime story: unsubscribes untested, no `close()`, and a killed session hangs every pending promise

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-symbol` |
| **layer** | vitest |
| **verified_by** | automated |
| **anchors** | `cockpit/plugin-sdk/index.js:attach`, `cockpit/plugin-sdk/index.js:connect#handshake-timeout` |
| **risk** | L4 × I4 = **16** |
| **observations** | coverage_pts=4, branches=18 (branch_pts=4), churn_90d=1 (churn_pts=2) |
| **anchor_sites** | 2 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** `plugin-sdk.test.ts` does import the shipped artifact (good, and worth recording)
but walks only three happy paths. Everything about lifetime is untested: the four unsubscribe closures
are never invoked and there is no `close()`/dispose at all, so the client can never release
`port.onmessage` or the `pending` map. The consequence chains with the host: when
`PluginSession.kill('flood')` fires, `post()` short-circuits on `!this.alive`, so the flood-triggering
command is **never acked**, its `pending` entry is never deleted, and the plugin's
`await fleet.launch(...)` hangs forever. `connect`'s `if (opts.timeoutMs)` also means the documented
default `connect()` never times out and never removes its window listener. And `attach` accepts
`init.apiVersion` blindly while `validateManifest` enforces it strictly on the host side —
asymmetric and untested. Note `cockpit/plugin-sdk/**` is outside vitest's `src/**` include pattern, so
this one importing test file is the only reachable coverage.

**Concrete test.** Over a real `MessageChannel`: register two `onState` callbacks, dispose the first,
push state, assert only the second fired; fire an in-flight `launch()` then `kill('flood')` and assert
(with a race-vs-timeout) that the promise never settles — documenting the hang. With fake timers,
assert `connect({timeoutMs:3000})` rejects at the deadline **and** removes its listener, and that
`connect()` with no `timeoutMs` stays pending forever (pinning today's behaviour).

### GAP-090 — The reference plugin's `esc`/`render` are structurally untestable, and `esc` guards `innerHTML`

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-symbol` |
| **layer** | vitest |
| **verified_by** | automated |
| **anchors** | `plugins/reference/app.js:esc`, `plugins/reference/app.js:render` |
| **risk** | L4 × I4 = **16** |
| **observations** | coverage_pts=5, branches=6 (branch_pts=3), churn_90d=1 (churn_pts=2) |
| **anchor_sites** | 2 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** `plugins/reference/app.js` has zero tests and **cannot get any in its current
shape**: it is outside vitest's include pattern, and at module scope it calls `document.getElementById`,
`connect(...)`, and imports `./sdk.js` — a file that does not exist on disk and is synthesized at
runtime by the Rust scheme handler's `sub == "sdk.js"` special case. So the one importable piece
(`model.js`) is well tested while the piece that touches untrusted data — a hand-rolled `esc` feeding
`innerHTML` — is untestable by construction. The sandbox contains the blast radius but does not make
injected script harmless inside the plugin's own document, and defence-in-depth is the file's own
stated intent.

**Concrete test.** Extract `esc` and make `render(model, doc)` take its root so a test under
`cockpit/ui/src/lib/` can import the shipped file the way `reference-plugin.test.ts` already imports
`model.js`. Then assert a unit id of `<img src=x onerror=...>` and a log line containing `</li><script>`
produce zero `script`/`img` nodes while the raw text survives as `textContent`, and that the
awaiting-approval banner renders with no clickable control.

### GAP-091 — The host duplicates the reference manifest inline, so the loader's tested code paths are unreachable in the app

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `duplicated-logic` |
| **layer** | vitest |
| **verified_by** | automated |
| **anchors** | `cockpit/ui/src/App.svelte:VIEW_PLUGIN_INDEX`, `cockpit/ui/src/lib/loader.ts:devPluginSource`, `plugins/reference/manifest.json` |
| **risk** | L3 × I4 = **12** |
| **observations** | coverage_pts=4, branches=3 (branch_pts=2), churn_90d=8 (churn_pts=4) |
| **anchor_sites** | 3 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** `App.svelte` inlines a copy of the manifest and calls `discoverFrom('packaged', …)`
directly, so `devPluginSource`, `packagedPluginSource` and the whole `dev` arm of `pluginSrc` are
unreachable from `main.ts` — `loader.test.ts`'s four green tests certify code the app never runs, which
is **worse than no coverage** because it reads as coverage. Meanwhile the copy that *is* shipped is
unpinned against `plugins/reference/manifest.json`: change the manifest's `entry`, `id` or
`capabilities` and the host keeps loading the stale copy, silently producing a 404 iframe (a row-1.3
failure) or the wrong grant (`GAP-083`). Separately **unverified**: `tauri.conf.json` declares no
`bundle.resources`, so a packaged build may ship neither `plugins/` nor `plugin-sdk/index.js` — which
would 404 both `index.html` and `sdk.js` in Part 2.

**Concrete test.** Static-import `plugins/reference/manifest.json` and assert it deep-equals the entry
the host actually loads; run it through `discoverFrom('packaged', …)` asserting one plugin, zero
rejections, the exact `src`, and `grantedCapabilities === ['log-append']`. Add a `tauri.conf.json`
assertion that the plugin and SDK paths are declared as bundle resources.

### GAP-092 — The hostile-plugin kill-and-revert path is covered by neither a test nor a checklist row

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `manual-uncovered` |
| **layer** | manual |
| **verified_by** | manual |
| **anchors** | `cockpit/ui/src/App.svelte:selectViewPlugin`, `cockpit/ui/src/lib/bridge.ts:PluginSession.kill` |
| **governs** | `cockpit/ui/src/App.svelte`, `cockpit/ui/src/lib/bridge.ts` |
| **last_manual_pass** | — (never_verified) |
| **risk** | L5 × I4 = **20** |
| **observations** | manual_coverage_pts=5, churn_90d=8 (churn_pts=4), never_verified=true |
| **anchor_sites** | 2 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** The flood/teardown response is the only plugin behaviour with a **host-visible**
consequence, and it is verified by nobody. `bridge.test.ts` asserts only that a `vi.fn()` `onKill`
receives `'flood'` against a fake host; no test renders `App`, so revert-to-the-trusted-ops-grid is
unproven. It is absent from the manual gate too: 1.3/1.4 are the happy path, 1.7 is app-plugin parking,
1.8 is leak-on-switch. If the revert misfires the user stares at a dead iframe with a live-looking
switcher, or a second bridge is built over a destroyed frame and leaks. Both smoke runs leave 1.3 and
1.4 "not run", so **the module with the repo's largest untrusted-input surface has never been executed
against a real plugin by a human or by CI.**

**Concrete test.** Dev build with a temporarily hostile reference plugin: flood the port and verify the
port closes, the iframe unmounts, the switcher reverts within one frame, five activate/flood cycles
leave the window-listener and detached-frame counts unchanged, and re-activating performs a fresh
handshake. Pre-automate the shell half (`GAP-076`).

### GAP-093 — The dashboard's local scan root is a hardcoded developer drive letter

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `manual-uncovered` |
| **layer** | manual |
| **verified_by** | manual |
| **anchors** | `cockpit/ui/src/App.svelte:localReader`, `cockpit/ui/src/lib/dashboard/adapters/local.ts:localCards#zero-docs` |
| **governs** | `cockpit/ui/src/App.svelte`, `cockpit/ui/src/lib/dashboard/adapters/local.ts`, `cockpit/ui/src-tauri/src/local_projects.rs` |
| **last_manual_pass** | — (never_verified) |
| **risk** | L5 × I3 = **15** |
| **observations** | manual_coverage_pts=5, churn_90d=8 (churn_pts=4), never_verified=true |
| **anchor_sites** | 2 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** **Phase-3 confirmed.** `App.svelte` hardcodes
`scanRoots: ['D:/MajorProjects'], maxDepth: 5, pins: [], excludes: []` as a literal — not from
`import.meta.env`, a settings file, or a command — and the Rust side has no default/env/home fallback
either. On any other machine `walkdir` swallows the io error, `scan_local_projects` returns `Ok([])`,
and `localCards` returns `[]` with health `ok`: the LOCAL lane renders as **absence indistinguishable
from "nothing to show"**. `localCards`' tests all inject docs, so the zero-docs path is never asserted,
and the checklist has no PROJECTS-content row at all. The codebase already has an env-seam pattern
(`VITE_FLEET_URL`); this reader simply doesn't use one.

**Concrete test.** Automated: `localCards(reader([]))` returns `[]` and the board renders an explicit
empty affordance rather than silence. Manual: on a machine with no `D:` drive, open PROJECTS and
confirm the LOCAL lane says something.

### GAP-094 — `Dashboard.svelte`'s entire live-wiring path is unexecuted while looking well tested

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-branch` |
| **layer** | vitest |
| **verified_by** | automated |
| **anchors** | `cockpit/ui/src/views/Dashboard.svelte:onMount#initialboard-absent`, `cockpit/ui/src/views/Dashboard.svelte:refresh#overlapping-polls` |
| **risk** | L3 × I3 = **9** |
| **observations** | coverage_pts=4, branches=4 (branch_pts=2), churn_90d=2 (churn_pts=2) |
| **anchor_sites** | 2 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** All five existing tests pass `initialBoard`, and `onMount` returns at its first line
when that prop is set — so the seed, the first refresh, both intervals, both subscriptions and the
teardown closure have **zero executed coverage** while the file reads as covered. That is the only path
that runs in the real app. PROJECTS is mounted and unmounted on every tab switch, so a broken cleanup
leaks two intervals and two listeners per visit. `refresh()` also has `try/finally` with **no catch**,
so any adapter rejection becomes an unhandled promise rejection on every tick, silently, with `pulling`
reset so the UI looks idle; and the 15 s interval has no in-flight guard, so slow sources stack up and
cards can flicker backwards.

**Concrete test.** Render without `initialBoard` using fake readers, fake timers, and stub subscribe
functions: assert seeded cards, the first refresh, that a captured fleet callback advances a card, that
timers re-poll, and that after `cleanup()` both unsubscribes ran and `vi.getTimerCount()` is 0. Then
hold a reader's promise across three intervals and assert `status()` was called **once**.

### GAP-095 — Every dashboard adapter's degradation contract is unenforced at its edges

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-branch` |
| **layer** | vitest |
| **verified_by** | automated |
| **anchors** | `cockpit/ui/src/lib/dashboard/adapters/halyard.ts:halyardCards#non-array-payload`, `cockpit/ui/src/lib/dashboard/adapters/local.ts:localCards#parse-throw-escapes-try` |
| **risk** | L3 × I3 = **9** |
| **observations** | coverage_pts=4, branches=2 (branch_pts=1), churn_90d=2 (churn_pts=2) |
| **anchor_sites** | 2 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** Both adapters promise to "degrade to `health: unknown`, never a fake stage", and
both promises hold only for the call inside the `try`. In `halyardCards` the `statuses.map(...)` sits
**outside** it, and the Rust side gives no shape guarantee — `run_halyard` returns whatever
`from_str::<Value>` yields from arbitrary CLI stdout, while `api.ts` asserts
`invoke<HalyardReleaseStatus[]>` as a compile-time lie. In `localCards`, `parseStatusFrontmatter` and
`parseRoadmapItems` run outside the `try`, and the latter runs `marked`'s lexer on unvalidated
third-party markdown read verbatim off disk — so **one malformed ROADMAP.md anywhere in a scanned tree
takes down the whole board's refresh loop**, including the healthy lanes, as an unhandled rejection
(`refresh` has no catch). Note the asymmetry: the Audience command normalizes via a tested
`unwrap_posts`; Halyard has no equivalent.

**Concrete test.** `halyardCards` with `status()` resolving to an object, `null`, and a JSON string —
assert `health: 'unknown'` rather than a throw. `localCards` with pathological `roadmapText`/`statusText`
— assert it resolves to cards. Plus a `store.test.ts` case asserting `pollLocal` never rejects.

### GAP-096 — App-scoped Halyard proposals are written into the map under a key nothing reads

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-branch` |
| **layer** | vitest |
| **verified_by** | automated |
| **anchors** | `cockpit/ui/src/lib/dashboard/adapters/halyard.ts:halyardCards#app-scoped-proposal-key` |
| **risk** | L3 × I3 = **9** |
| **observations** | coverage_pts=4, branches=4 (branch_pts=2), churn_90d=1 (churn_pts=2) |
| **anchor_sites** | 1 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** **Phase-3 confirmed.** The map is keyed `p.release_id ?? \`app:${p.app}\`` and the
only external read is `openByRelease.get(s.release_id)` — no `app:`-prefixed lookup exists anywhere. So
every release-less proposal (the `release_id?` field is explicitly optional and `social_post` is the
documented example) can never set a card Blocked and never contributes to the NEEDS YOU count. The
board's single most valuable number under-reports: a real human gate exists in Halyard's queue and the
operator is never told. The existing test only covers a proposal *with* a `release_id`.

**Concrete test.** One status and one OPEN proposal for the same app with `release_id` undefined;
assert the card is Blocked with gate `approval`. It is `Live` with no blocked affordance today.

### GAP-097 — Both dashboard adapters map an unrecognised upstream state to a confident "Idle"

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-branch` |
| **layer** | vitest |
| **verified_by** | automated |
| **anchors** | `cockpit/ui/src/lib/dashboard/adapters/audience.ts:pipelineFor#default-arm`, `cockpit/ui/src/lib/dashboard/adapters/halyard.ts:pipelineFor#default-arm` |
| **risk** | L4 × I3 = **12** |
| **observations** | coverage_pts=4, branches=18 (branch_pts=4), churn_90d=1 (churn_pts=2) |
| **anchor_sites** | 2 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** Both `default:` arms map an unknown native state to `{ stage: null }` → `Idle` with
`health: 'ok'` — indistinguishable from a genuinely idle project. So contract drift in either upstream
(an Audience backend adding `scheduled`, a Halyard release-state rename) silently mis-stages real work
as idle rather than surfacing it: the exact class of defect the whole declared-vs-inferred machinery
exists to prevent. The tests enumerate only states that already map, so the drift arm is the one arm
with no coverage, and nothing checks either mapping against its producer.

**Concrete test.** Table-drive both adapters with absent states — audience: `scheduled`, `queued`, `''`,
`APPROVAL-PENDING` (case drift); halyard: `canary`, `staged`, `shipped`, `null` — asserting either an
explicit "unrecognized state: X" in `detail` with `health: 'degraded'`, or at minimum pinning today's
behaviour so a future mapping change is visible.

### GAP-098 — `dashboard/api.ts` has no test file and is the only place the four IPC command names appear

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-symbol` |
| **layer** | vitest |
| **verified_by** | automated |
| **anchors** | `cockpit/ui/src/lib/dashboard/api.ts:tauriHalyardReader`, `cockpit/ui/src/lib/dashboard/api.ts:tauriAudienceReader` |
| **risk** | L3 × I3 = **9** |
| **observations** | coverage_pts=5, branches=0 (branch_pts=1), churn_90d=1 (churn_pts=2) |
| **anchor_sites** | 2 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** The one dashboard file with literally zero test execution, and the only place the
four IPC name strings exist on the TS side. A rename on the Rust side produces no compile error, no
type error and no test failure — it produces a runtime invoke rejection the adapters' catch blocks turn
into a permanently greyed lane that looks exactly like "Halyard isn't installed". The generic
parameters are erased at runtime and assert nothing. Both sides are ungated: vitest is not in CI and
the src-tauri crate is a standalone workspace (`GAP-057`). This is also the module's only live-I/O seam
with no injectable boundary beneath it.

**Concrete test.** `api.test.ts` with `vi.mock('@tauri-apps/api/core')`: assert each method invokes the
exact command name and returns its payload untransformed, and cross-check those four literals against
the `generate_handler![]` list in `src-tauri/src/lib.rs` (read as a fixture, or share a generated
constant).

### GAP-099 — The dashboard's user-facing affordances — deep links, chips, footers, empty state — are asserted nowhere

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-branch` |
| **layer** | vitest |
| **verified_by** | automated |
| **anchors** | `cockpit/ui/src/views/Dashboard.svelte:openLink`, `cockpit/ui/src/views/Dashboard.svelte:template#health-footer-precedence`, `cockpit/ui/src/views/Dashboard.svelte:template#declared-and-conflict-chips`, `cockpit/ui/src/views/Dashboard.svelte:template#empty-state-vs-first-poll` |
| **risk** | L3 × I3 = **9** |
| **observations** | coverage_pts=4, branches=5 (branch_pts=2), churn_90d=2 (churn_pts=2) |
| **anchor_sites** | 4 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** Four separate holes in one template, all user-visible. `openLink` — the entire
realisation of "every act-here affordance deep-links OUT" — is invoked by no test; both fallback layers
are pure error-path code that has never executed, and a wrong command name or unhandleable scheme
produces a click that silently does nothing. The four-arm health footer has no assertion, and the tests
are **structurally incapable** of reaching three arms: fixtures use a frozen 2026-06-09 clock while the
component reads `Date.now()`, so every card in every test is stale and greyed — the "renders the
canonical stage" test passes on `data-stage` while the card is visually inert. The DECLARED and DRIFT
chips — the visible half of the whole override/conflict mechanism — have never rendered. And there is
**no loading state at all**: between mount and the first (unbounded) Halyard poll the user is told, in
display type, "NO PROJECTS · every source is idle or unreachable", which is simply false.

**Concrete test.** Use `vi.setSystemTime` so `updatedIso` is relative to a frozen clock, then assert all
four footer arms and their precedence; render a board with an override and a conflict and assert both
chips with their tooltips; click `act` with a mocked `invoke` and assert the command and both fallbacks;
render with unresolved readers and assert the pre-first-poll state is not the empty state.

### GAP-100 — App-plugin cards can never reach the dashboard board because the prop is never passed

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-branch` |
| **layer** | vitest |
| **verified_by** | automated |
| **anchors** | `cockpit/ui/src/views/Dashboard.svelte:onMount#onpluginstate-guard`, `cockpit/ui/src/App.svelte:dashboard-mount` |
| **risk** | L3 × I3 = **9** |
| **observations** | coverage_pts=4, branches=1 (branch_pts=1), churn_90d=8 (churn_pts=4) |
| **anchor_sites** | 2 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** **Phase-3 confirmed.** `App.svelte` mounts `<Dashboard>` with five props and
`onPluginState` is not among them, so the guard is false in production and
`store.ts:applyPluginState` plus the whole `appPlugin.ts` adapter — six passing unit tests across two
files — never place a card on the real board. **This is the failure mode unit tests are worst at:
every piece passes and the composition is unwired.** Only a wiring assertion at the App level or a
human looking at the tab would reveal it, and neither exists (the app-plugin smoke rows are all about
the native webview, not the board).

**Concrete test.** An App-level assertion that the `<Dashboard>` instance receives an `onPluginState`
prop, plus a live-render Dashboard case invoking the captured callback and asserting an
`app-plugin:<id>` card appears with the mapped stage.

### GAP-101 — `model.ts:isOffPipeline` is exported, unreferenced, untested — and `sortedCards` re-derives it inline

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-symbol` |
| **layer** | vitest |
| **verified_by** | automated |
| **anchors** | `cockpit/ui/src/lib/dashboard/model.ts:isOffPipeline`, `cockpit/ui/src/lib/dashboard/store.ts:sortedCards` |
| **risk** | L4 × I3 = **12** |
| **observations** | coverage_pts=5, branches=0 (branch_pts=1), churn_90d=3 (churn_pts=3) |
| **anchor_sites** | 2 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** Zero references of any kind, and `model.ts` has no test file at all. Low individual
severity, but it points at a real hole: `sortedCards` hardcodes `if (s === 'Failed')` /
`if (s === 'Blocked')` / `else 99` instead of using the exported predicate, so the two representations
of "which stages are off-pipeline" can drift silently. Add a fourth off-pipeline state and
`sortedCards` buckets it at 99 alongside Idle with no type error and no failing test.

**Concrete test.** Either delete it, or add `model.test.ts` asserting `isOffPipeline` and
`isPipelineStage` exhaustively partition the `Stage` union — then make `sortedCards` use it.

### GAP-102 — The dashboard's "source unreachable" card is hand-copied three times with divergent fields

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `duplicated-logic` |
| **layer** | vitest |
| **verified_by** | automated |
| **anchors** | `cockpit/ui/src/lib/dashboard/adapters/halyard.ts:halyardCards#catch-card`, `cockpit/ui/src/lib/dashboard/adapters/audience.ts:unknownCard`, `cockpit/ui/src/lib/dashboard/adapters/local.ts:localCards#catch-card` |
| **risk** | L3 × I3 = **9** |
| **observations** | coverage_pts=3, branches=3 (branch_pts=2), churn_90d=2 (churn_pts=2) |
| **anchor_sites** | 3 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** Three copies of the degraded `ProjectCard` literal with real divergences: local and
halyard omit `family` while audience sets it; the `projectId` conventions differ; the `detail` prefixes
differ. `ProjectCard`'s stated purpose is being the **one** render contract, so adding a required field
means finding all three by hand — and the `downSources` banner keys off `health === 'unknown'` in all
of them. Only two are asserted in any depth. A fourth adapter would copy it a fourth time. Related:
every adapter also repeats a five-line options preamble that calls the injected `now()` **twice**, so a
card's `updatedIso` and the instant used to evaluate its override TTL are different — invisible to
tests because they all inject a frozen clock where the two coincide.

**Concrete test.** Extract `sourceDownCard(source, name, detail, nowIso, staleAfterSec)` and
`resolveAdapterOpts(opts, defaultStaleSec)` (one `now()` call), then one table-driven test across all
sources asserting the invariant degraded-card contract and that `Date.parse(nowIso) === nowMs` exactly.

### GAP-103 — The session-state SessionEnd hook has never been observed firing

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `manual-uncovered` |
| **layer** | manual |
| **verified_by** | manual |
| **anchors** | `plugins/session-state/hooks/hooks.json:SessionEnd`, `plugins/session-state/src/capture_end.mjs:module` |
| **governs** | `plugins/session-state/hooks/hooks.json`, `plugins/session-state/src/capture_end.mjs` |
| **last_manual_pass** | — (never_verified) |
| **risk** | L4 × I2 = **8** |
| **observations** | manual_coverage_pts=5, churn_90d=1 (churn_pts=2), never_verified=true |
| **anchor_sites** | 2 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** The module's own spike record verified exactly two events ("Both hooks fired —
SessionStart and Stop") and `claude plugin details` reported "Hooks (2)". `hooks.json` declares three.
So SessionEnd — the **only** path that writes a durable timeline record from a hook, the only path that
unlinks a finished session's scratch file, and the only caller of `prune` outside the CLI — has never
been observed firing in a real session on any OS. `entries.test.mjs` invokes the script directly but
asserts only the `reason === 'clear'` skip. The whole body is wrapped in a bare `try {} catch {}`
ending in `process.exit(0)`, so if the event never arrives with the expected payload, every session's
state is silently lost, scratch files accumulate forever, and the store never prunes — with no error
surface anywhere.

**Concrete test.** Extend the spike protocol with a SessionEnd leg: install into a throwaway
`CLAUDE_CONFIG_DIR`, run a session, exit normally, and assert `claude plugin details` lists three hooks,
that `timeline.jsonl` gained a `SessionEnd:<reason>` record, that the scratch file was unlinked, and
that prune ran. Then repeat with `reason=clear` and assert nothing is appended.

### GAP-104 — The Stop hook spawns eight sequential git subprocesses against a 5-second budget

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `manual-uncovered` |
| **layer** | manual |
| **verified_by** | manual |
| **anchors** | `plugins/session-state/hooks/hooks.json:Stop`, `plugins/session-state/src/capture_scratch.mjs:module`, `plugins/session-state/src/keying.mjs:repoRoot` |
| **governs** | `plugins/session-state/src/capture_scratch.mjs`, `plugins/session-state/src/keying.mjs`, `plugins/session-state/src/gitfacts.mjs` |
| **last_manual_pass** | — (never_verified) |
| **risk** | L4 × I2 = **8** |
| **observations** | manual_coverage_pts=5, churn_90d=3 (churn_pts=3), never_verified=true |
| **anchor_sites** | 3 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** **Phase-3 confirmed, count exact.** Eight `execFileSync` git spawns on the Stop
path, each carrying its own 5000 ms timeout, against a `"timeout": 5` (seconds) hook budget — an
internal worst case of ~40 s. Two of the eight are pure waste: `repoRoot(cwd)` is called, then
`stateDir(cwd)` re-enters `repoKey → repoRoot` with no memoization (`GAP-107`). This is the
highest-frequency path in the plugin — it runs at the end of **every** assistant turn — and nothing
anywhere bounds its latency; the spec even asserts the opposite as settled fact. The 30 s throttle is
also confirmed to sit **after** `collectGitFacts`, because the skip condition compares the
already-collected facts, so it saves one `writeFileSync` and no subprocess at all.

**Concrete test.** Time `capture_scratch.mjs` against a large synthetic repo with a dirty worktree and
assert wall time stays well inside 2 s. Then reorder so the throttle short-circuits before
`collectGitFacts`, and add the missing throttle unit tests (second run does not rewrite; a real git
change does rewrite inside the window; a corrupt scratch recovers).

### GAP-105 — `capture_end` drops a timeline record and deletes the only backup in the same breath

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-branch` |
| **layer** | node-session-state |
| **verified_by** | automated |
| **anchors** | `plugins/session-state/src/store.mjs:appendRecord#locktimeout-false`, `plugins/session-state/src/capture_end.mjs:module#ignores-return` |
| **risk** | L3 × I2 = **6** |
| **observations** | coverage_pts=4, branches=3 (branch_pts=2), churn_90d=1 (churn_pts=2) |
| **anchor_sites** | 2 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** **Phase-3 confirmed.** `appendRecord` returns `false` specifically on
`LockTimeout` — a checked-return contract — and `capture_end.mjs` calls it as a bare expression
statement, then unconditionally unlinks its own scratch file and prunes, all inside a bare
`try {} catch {}` with `exit(0)` and no output. So under contention the record is dropped **and** the
scratch copy, the only other place that session's git facts survive, is deleted. `capture_rich.mjs`
handles the identical return correctly (preserves the temp file, prints a retry), which proves the
contract was understood and then not honoured on the auto path. `lock.test.mjs` shows `LockTimeout` is
reachable in ~1 s of contention, and two Claude sessions in one repo is a routine setup.

**Concrete test.** Pre-create `timeline.lock` holding a live-pid fresh token, run `capture_end.mjs`,
and assert `timeline.jsonl` is unchanged **and** `scratch/<sid>.json` still exists.

### GAP-106 — `withLock` steals a lock from a demonstrably live holder, and `sleep` busy-spins

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-branch` |
| **layer** | node-session-state |
| **verified_by** | automated |
| **anchors** | `plugins/session-state/src/lock.mjs:withLock#age-steal-with-live-pid`, `plugins/session-state/src/lock.mjs:sleep` |
| **risk** | L4 × I2 = **8** |
| **observations** | coverage_pts=4, branches=11 (branch_pts=4), churn_90d=2 (churn_pts=2) |
| **anchor_sites** | 2 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** Every lock test is single-process and hand-fabricates lockfiles; no test ever has
two real processes contend. Two hazards follow from the source. The age backstop is **unconditional**:
it steals whenever mtime exceeds `maxAgeMs` even when the recorded pid is alive — so a holder that
stalls past 60 s (a paused debugger, a suspended laptop, a network-mounted `~/.claude`) has its lock
taken while still inside `fn()`, producing two concurrent writers doing `appendFileSync` plus a full
`latest.md` rewrite. The ownership-checked release stops the loser deleting the winner's lock but does
nothing about the two bodies running at once — that is the actual data-loss window. Second, `sleep` is
a hard busy-spin, so contention burns a core for up to `tries * backoffMs` inside a hook that fires
every turn; no test has ever paid that cost.

**Concrete test.** Spawn eight child processes each calling `appendRecord` against one shared temp dir;
assert `readTimeline(dir).length === 8`, that the raw line count matches (`readTimeline` swallows torn
lines), and that `latest.md` is complete rather than a truncated interleave. Add a holder-stalls-past-
`maxAgeMs` case asserting the two appends do not interleave.

### GAP-107 — Two `repoRoot` spawns per hook, and a torn `git status` renders as a real branch called `null`

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `duplicated-logic` |
| **layer** | node-session-state |
| **verified_by** | automated |
| **anchors** | `plugins/session-state/src/keying.mjs:stateDir`, `plugins/session-state/src/keying.mjs:repoKey`, `plugins/session-state/src/gitfacts.mjs:collectGitFacts#status-failure` |
| **risk** | L3 × I2 = **6** |
| **observations** | coverage_pts=4, branches=9 (branch_pts=3), churn_90d=3 (churn_pts=3) |
| **anchor_sites** | 3 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** All three capture entries do `repoRoot(cwd)` then `stateDir(cwd)`, and `stateDir`
re-enters `repoRoot` internally — the same subprocess twice, on the timeout-constrained path
(`GAP-104`). It is also a correctness hazard: `repoRoot` is evaluated at two different instants, so a
transient failure yields `repo` and `dir` derived from different roots, which `checkMeta` then reports
as a spurious COLLISION. Separately, `collectGitFacts`' status IIFE does `catch { return "" }`, and
`parsePorcelainV2("")` returns `{branch: null, detached: false, dirty: []}` — indistinguishable from a
clean attached repo. `head` is still populated from the separate `git log`, so `fmtGit` renders the
literal ``branch `null` @ abc1234``, and that garbage record becomes the freshest entry the next
session's resume block reads.

**Concrete test.** A spawn-counting test (PATH-shadow `git` with a counter) asserting at most one
`rev-parse --show-toplevel` per capture. Plus a PATH-shadowed git that fails only on `status`, asserting
the result is rejected or clearly marked degraded rather than reporting `branch: null`.

### GAP-108 — The session-state hook contract is validated only for file existence

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `duplicated-logic` |
| **layer** | node-session-state |
| **verified_by** | automated |
| **anchors** | `plugins/session-state/hooks/hooks.json:SessionStart`, `plugins/session-state/src/resume.mjs:module#source-gate`, `plugins/session-state/test/manifest.test.mjs:manifest-validity` |
| **risk** | L3 × I2 = **6** |
| **observations** | coverage_pts=4, branches=4 (branch_pts=2), churn_90d=1 (churn_pts=2) |
| **anchor_sites** | 3 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** The source gate is written twice — declaratively as `"matcher": "startup|resume"`
and imperatively as `["startup","resume"].includes(data.source)` — and the spec calls the alignment
deliberate. Nothing checks it. `manifest.test.mjs` asserts exactly one thing per hook: that the script
file exists. It never looks at the matcher, at `type`/`command`, or at the Stop entry's `timeout: 5` —
so widening the matcher produces hooks that fire and silently no-op, narrowing it kills resume with no
failing test, and dropping the timeout field (the only thing bounding the every-turn hook) passes
everything. The hook contract **is** this module's real entry point.

**Concrete test.** Read `hooks.json`, split the SessionStart matcher on `|`, and assert the set equals
the array `resume.mjs` gates on (export it and import it in both). Same for `capture_end.mjs`'s SKIP
set. Assert every entry has `type: 'command'`, `command: 'node'`, an `args[0]` under
`${CLAUDE_PLUGIN_ROOT}/`, and that Stop still carries a numeric timeout.

### GAP-109 — `capture_rich`'s four failure arms are the plugin's only user-visible errors and none is tested

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-branch` |
| **layer** | node-session-state |
| **verified_by** | automated |
| **anchors** | `plugins/session-state/src/capture_rich.mjs:module#failure-arms`, `plugins/session-state/src/store.mjs:prune#locktimeout-skip` |
| **risk** | L3 × I2 = **6** |
| **observations** | coverage_pts=4, branches=7 (branch_pts=3), churn_90d=1 (churn_pts=2) |
| **anchor_sites** | 2 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** `entries.test.mjs` exercises only the success path, and the `/save-state` skill
tells the model to read and relay these arms. Two concrete defects sit in that dead text: the retry
command it prints is `node capture_rich.mjs --input ${input}` — a bare relative script name that cannot
run from the user's cwd, handed to a user the skill instructs not to blind-retry — and `${input}` is
interpolated **unquoted**, so any temp path containing a space (routine on Windows) produces a broken
command anyway. The spec mandated "a fixture path containing a space" and no test in the suite uses
one. Bundled here: `prune`'s `LockTimeout` arm silently abandons truncation with no retry and no
signal, and its only automatic caller is the SessionEnd hook that may never fire (`GAP-103`).

**Concrete test.** Table-drive the four arms (no input; malformed JSON; repo-key collision; lock held)
asserting exit code, the message, and that the temp file is preserved — with a fixture dir whose name
contains a space, and asserting the printed retry command is absolute and shell-quoted. Plus a held-lock
`prune` asserting the file is untouched and a later uncontended prune truncates.

### GAP-110 — `npm test` — 135 tests over the whole cockpit UI — is not in CI

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-symbol` |
| **layer** | build_ci_gate |
| **verified_by** | automated |
| **anchors** | `.github/workflows/ci.yml:jobs`, `cockpit/ui/package.json:scripts.test` |
| **risk** | L3 × I5 = **15** |
| **observations** | coverage_pts=4, branches=2 (branch_pts=1), churn_90d=4 (churn_pts=3) |
| **anchor_sites** | 2 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** Verified: the only npm invocations anywhere in `.github/workflows` are `npm ci`,
`npm run sidecar`, `npm run tauri build`, and `npm run sidecar:release`. The 19 vitest files gate
nothing on push or PR; the only thing CI proves about the UI is that `vite build` compiled it.
**Every UI entry in this plan inherits this** — including every test the automatable smoke items below
would add. Orchestrator captured a real run this session: 135/135 green, so the suite is real and
healthy and simply ungated.

**Concrete test.** `scripts/ci-shape.test.mjs` (node:test, added to the `embargo` job which already runs
`node --test`): parse `ci.yml` with js-yaml and assert some step's `run` matches `/npm (run )?test|vitest/`.
Fails today; passes once the step is added.

### GAP-111 — `npm run check` (353 files) is not in CI, and two source files are typechecked by nothing

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-symbol` |
| **layer** | build_ci_gate |
| **verified_by** | automated |
| **anchors** | `cockpit/ui/package.json:scripts.check`, `cockpit/ui/tsconfig.node.json:include` |
| **risk** | L4 × I5 = **20** |
| **observations** | coverage_pts=5, branches=0 (branch_pts=1), churn_90d=4 (churn_pts=3) |
| **anchor_sites** | 2 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** `check` = `svelte-check --tsconfig ./tsconfig.app.json && tsc -p tsconfig.node.json`,
invoked by no workflow step. `vite build` (the `beforeBuildCommand`) strips types without checking them,
so a type error in a `.svelte`/`.ts` file is invisible to CI end to end. `SPIKE-RESULTS.md` records that
`npm run check` was not even *runnable* at session start (empty `node_modules`) — exactly how a
never-CI'd gate rots. Compounding it: `tsconfig.node.json:include` is `["vite.config.ts"]` only and
`tsconfig.app.json` is `src/**`, so `vitest.config.ts` and `cockpit/ui/scripts/build-sidecar.mjs` are
typechecked by nothing even if `check` were wired in — and `tsconfig.node.json` is the only project
enabling `noUnusedLocals`/`noFallthroughCasesInSwitch`, applied to exactly one 9-line file.

**Concrete test.** Assert a `ci.yml` step runs `npm run check`; separately assert every tracked
`.ts`/`.mjs` under `cockpit/ui` (excluding `node_modules`/`dist`) is matched by exactly one project's
`include`.

### GAP-112 — Three whole test suites outside the Rust workspace are invoked by no gate

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-symbol` |
| **layer** | build_ci_gate |
| **verified_by** | automated |
| **anchors** | `.github/workflows/ci.yml:jobs.test`, `plugins/session-state/test`, `tools/budget-checkpoint/pyproject.toml:[tool.pytest.ini_options]`, `tools/cache-countdown/pyproject.toml:[tool.pytest.ini_options]` |
| **risk** | L3 × I5 = **15** |
| **observations** | coverage_pts=4, branches=0 (branch_pts=1), churn_90d=4 (churn_pts=3) |
| **anchor_sites** | 4 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** Verified and captured green this session: `node --test
"plugins/session-state/test/*.test.mjs"` → 52/52, `uv run pytest` in `budget-checkpoint` → 24, in
`cache-countdown` → 29. **None of the three is invoked by any workflow step.** There is no
`package.json` under `plugins/session-state` and the strings `python`, `pytest` and `uv` appear nowhere
in `.github/workflows`. All three back things that run on the maintainer's machine on every session —
Stop hooks, SessionStart hooks, the session timeline — so a regression merged to `main` silently
degrades every future Claude Code session, and 105 passing tests would never notice.

**Concrete test.** Add the two steps (`node --test` needs no `package.json`; both pyprojects already set
`testpaths` and a pytest dev group, so `uv run pytest` is one line each) and assert their presence in
`scripts/ci-shape.test.mjs` so they cannot be quietly dropped again.

### GAP-113 — No lint, format, or static-analysis gate exists anywhere

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-symbol` |
| **layer** | build_ci_gate |
| **verified_by** | automated |
| **anchors** | `.github/workflows/ci.yml:jobs` |
| **risk** | L4 × I5 = **20** |
| **observations** | coverage_pts=5, branches=0 (branch_pts=1), churn_90d=4 (churn_pts=3) |
| **anchor_sites** | 1 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** Exhaustive grep: `clippy`, `rustfmt` and `cargo fmt` appear in zero files under
`.github/`, `.githooks/`, and every `Cargo.toml`; there is no eslint or prettier either. Clippy catches
several correctness-adjacent classes this codebase is actively exposed to for free — `await` while
holding a lock, blocking calls in async contexts, needless clones in the supervisor's threads — and it
has no automated reader. Given the repo's history (`GAP-005`), a lint that flags blocking-in-async is
disproportionately valuable here.

**Concrete test.** Add a `lint` job running `cargo clippy -- -D warnings` and `cargo fmt --check` over
the root workspace **and** over `cockpit/ui/src-tauri` (which needs its own `--manifest-path`, per
`GAP-057`), then assert both in `scripts/ci-shape.test.mjs`.

### GAP-114 — `release.yml` signs and publishes without running a single test

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-symbol` |
| **layer** | build_ci_gate |
| **verified_by** | automated |
| **anchors** | `.github/workflows/release.yml:jobs.release` |
| **risk** | L3 × I5 = **15** |
| **observations** | coverage_pts=5, branches=2 (branch_pts=1), churn_90d=2 (churn_pts=2) |
| **anchor_sites** | 1 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** **Phase-3 confirmed.** One job, no `needs:`, no test command: checkout → toolchain
→ cache → node → apt → `npm ci` → `npm run sidecar:release` → `tauri-action` with `releaseDraft: true`.
And the refuter found the sharper point: `ci.yml` triggers on `push: branches: ["**"]` and
`pull_request` — **a tag ref is not a branch**, so `cargo test --workspace` does not even run on a `v*`
tag push. A tag from a red branch produces a signed, notarized, published release; a human clicking
publish is a review step, not a gate. `ci.yml`'s own `build` job has `needs: test`, so the pattern
exists and was omitted here.

**Concrete test.** Assert the `release` job either contains a test step or a `needs:` on a job that
does, before the `tauri-action` step. Cheapest real fix: `uses: ./.github/workflows/ci.yml`.

### GAP-115 — The Docker integration tests run nowhere, on any schedule

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-symbol` |
| **layer** | build_ci_gate |
| **verified_by** | automated |
| **anchors** | `.github/workflows/ci.yml:jobs.test#ignored-its`, `crates/fleetd/tests/local_docker_it.rs`, `crates/fleetd/tests/preflight_it.rs` |
| **risk** | L3 × I5 = **15** |
| **observations** | coverage_pts=4, branches=0 (branch_pts=1), churn_90d=4 (churn_pts=3) |
| **anchor_sites** | 3 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** Documented as deliberate at `ci.yml` lines 9–14 and 93–94, and the documentation is
honest — but "documented" is not "covered". The real container lifecycle, the thing the product does, is
verified only when a human remembers to run it locally, and nothing records whether anyone ever has.
One correction to the header's framing: `swarm_smoke_it.rs` is ignored for **git network access**, not
Docker, and `demo_mode_it.rs` is not ignored and does run. Note also that many behaviours currently
guarded only by these files are reclassifiable to plain `cargo test` via fake seams — see `GAP-023`,
`GAP-116`, `GAP-117`.

**Concrete test.** A scheduled `.github/workflows/nightly-it.yml` on a self-hosted/dind runner running
`cargo test --workspace -- --ignored`, plus an assertion in `scripts/ci-shape.test.mjs` that such a
workflow exists so the gap cannot be quietly forgotten.

### GAP-116 — `FakeRunner` cannot fail, so the Docker error arms are unreachable from CI

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-branch` |
| **layer** | rust-workspace |
| **verified_by** | automated |
| **anchors** | `crates/fleetd/src/fake.rs:provision#no-failure-knob`, `crates/fleetd/src/fake.rs:list_unit_containers#no-failure-knob`, `crates/fleetd/src/driver.rs:drive#provisioning-err-arm` |
| **risk** | L4 × I4 = **16** |
| **observations** | coverage_pts=4, branches=0 (branch_pts=1), churn_90d=10 (churn_pts=4) |
| **anchor_sites** | 3 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** `FakeRunner::provision` returns `Ok(Handle)` unconditionally with no failure knob,
so the driver's `Provisioning` Err arm (`ErrorScope::Docker` → `FatalError`) is structurally
unreachable from any non-ignored test — error scope, permit release, terminal phase and whether the
cockpit sees `Done(failed)` are all unverified. The fake already models one failure
(`oracle_read_fails`), so the pattern exists. Worse, `list_unit_containers` on the real runner returns
`Ok(vec![])` when `docker ps` exits non-zero — "no containers running", a lie that propagates straight
into reconciliation, so a stopped daemon makes every non-terminal unit `HaltNoContainer` (halted while
its container may be alive and spending) and every genuine orphan invisible. `reconcile.rs`'s decision
function is densely tested, which makes this **look** covered; the input-corruption step is what is
untested. **Both are fully reclassifiable from human-QA to plain `cargo test`.**

**Concrete test.** Add `FakeRunner::provision_fails()` and `list_fails()`. Assert the Provisioning Err
arm's full contract, and assert `reconcile_on_startup`/`reconcile_tick` emit no `ReapStray`/
`HaltNoContainer` for units whose container state is merely *unknown*.

### GAP-117 — `trial_merge`'s Conflict half and cleanup are testable with git alone and are tested nowhere

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-symbol` |
| **layer** | rust-workspace |
| **verified_by** | automated |
| **anchors** | `crates/fleetd/src/gh_forge.rs:trial_merge`, `crates/fleetd/src/gh_forge.rs:ensure_clone`, `crates/fleetd/src/gh_forge.rs:open_pr` |
| **risk** | L4 × I4 = **16** |
| **observations** | coverage_pts=4, branches=1 (branch_pts=1), churn_90d=1 (churn_pts=2) |
| **anchor_sites** | 3 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** `trial_merge` is the gate that decides whether a paid agent run becomes a PR, and
its Conflict half has zero coverage anywhere. Its cleanup is unverified too: `git merge --abort` is
fired with `let _ =` after both outcomes, but abort **errors** when the merge was a fast-forward or
already up to date — which would leave the shared host clone in a detached mid-merge state poisoning
the next unit that reuses the path. `ensure_clone` full-clones into `temp_dir()` per unit with no
timeout and no size bound, and **nothing ever deletes it** (only the ignored ITs clean up after
themselves), so every real mission permanently leaks a repo clone — as does `export_bundle`'s
`.bundle` file. `open_pr` collapses every outcome into `ForgeError::Failed(stderr)`: no rate-limit
handling at all, in stark contrast to `retry.rs` on the Anthropic side, and "a PR already exists for
this branch" (the normal outcome of any resume) is fatal rather than idempotent.

**Concrete test.** **Needs only `git` and a `file://` fixture — no Docker, no GitHub, so it runs in
CI.** Build a bare origin plus a `git bundle`; assert Clean and Conflict outcomes, and that after
*either* the host clone is clean (`git status --porcelain` empty, no `MERGE_HEAD`). Make the `git`/`gh`
program names injectable and drive `open_pr` against stub scripts emitting canned stderr and exit codes.

### GAP-118 — `local_docker`'s pure validators and exit-code mappings are untested, and two write paths swallow failure

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-symbol` |
| **layer** | rust-workspace |
| **verified_by** | automated |
| **anchors** | `crates/fleetd/src/local_docker.rs:valid_repo_url`, `crates/fleetd/src/local_docker.rs:has_diff#exit-code-polarity`, `crates/fleetd/src/local_docker.rs:commit_all#nothing-to-commit`, `crates/fleetd/src/local_docker.rs:discard#swallowed-result` |
| **risk** | L4 × I4 = **16** |
| **observations** | coverage_pts=4, branches=5 (branch_pts=2), churn_90d=9 (churn_pts=4) |
| **anchor_sites** | 4 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** `valid_git_branch` has a dedicated injection test; its sibling `valid_repo_url` —
which guards the value handed to `git clone` inside the container, fed from an API-supplied
`UnitSpec.repo_url` and from `CC_REPO_URL` — has none at all: five boolean terms, zero assertions, and
nothing pins that `file://`, `ext::` and `http://` stay rejected. `has_diff` maps **any** non-zero exit
to "there is a diff", including git's 128 for a bad ref, routing a broken unit into bundle export and a
real PR instead of NO_CHANGE; only the `true` side is ever observed. `commit_all` collapses "clean
tree" together with every genuine git failure into `Ok(false)`. And `discard`/`teardown` swallow both
docker calls with `let _ =` and return `Ok(())`, so a container that refuses to die or a volume that is
not removed is indistinguishable from success.
*(A related claim — that `discard`'s `replacen("cc_", "ccvol_", 1)` mis-targets — was Phase-3 downgraded
to **uncertain**: every reachable `Handle.id` today starts with `cc_`, so the rewrite is correct. The
unenforced prefix coupling and the swallowed result remain real and are what this entry covers.)*

**Concrete test.** Table-test `valid_repo_url` mirroring the existing branch-validator test; extract and
table-test the two exit-code mappings over `0`, `1`, `128`; and assert `derived_volume(handle) ==
volume_name(unit_id)` over hostile ids including ones containing `cc_` internally.

### GAP-119 — Failed and halted units keep their volumes forever, and nothing has ever looked

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `manual-uncovered` |
| **layer** | manual |
| **verified_by** | manual |
| **anchors** | `crates/fleetd/src/local_docker.rs:teardown`, `crates/fleetd/src/local_docker.rs:reap_unit` |
| **governs** | `crates/fleetd/src/local_docker.rs`, `crates/fleetd/src/reconcile.rs` |
| **last_manual_pass** | — (never_verified) |
| **risk** | L5 × I4 = **20** |
| **observations** | manual_coverage_pts=5, churn_90d=9 (churn_pts=4), never_verified=true |
| **anchor_sites** | 2 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** By design `teardown` and `reap_unit` remove the container and **keep** the
`ccvol_<unit>` volume; `discard` — the only thing that removes one — runs solely on Done/NoChange/
Abandon, and `GAP-021` shows even the Done path can skip it. So every failed unit, every
halted-then-forgotten unit, and every unit whose driver died leaves a permanent named volume holding a
full repo clone, reachable directly from `serve.rs → reconcile_*  → reap_unit`. Automated coverage is
zero in the sense that matters (the fake models no volumes; the ignored ITs never inspect
`docker volume ls`). **Manual coverage is also zero**: Gate 5's recorded PASS is `docker ps` — containers
only. Unbounded disk growth verified by neither machine nor human. Add the abnormal-exit case too:
nothing reaps a container at process exit, so a `kill -9` leaves the agent running with the API key
until someone restarts the daemon.

**Concrete test.** Add `docker volume ls --filter name=ccvol_ -q` to the Gate 5 row and an abnormal-exit
row. Automate the decision half now: assert a unit terminating in `Failed` bumps `teardowns` and not
`discards`, then add a tested, bounded reaping policy with a pure "which volumes are eligible" function.

### GAP-120 — The fake and the real runner disagree on what a valid `UnitSpec` is

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-branch` |
| **layer** | rust-workspace |
| **verified_by** | automated |
| **anchors** | `crates/fleetd/src/fake.rs:provision#skips-spec-validation`, `crates/fleetd/src/local_docker.rs:provision#spec-validation`, `crates/fleetd/src/gh_forge.rs:guard_branch` |
| **risk** | L3 × I4 = **12** |
| **observations** | coverage_pts=2, branches=4 (branch_pts=2), churn_90d=10 (churn_pts=4) |
| **anchor_sites** | 3 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** `FakeRunner::provision` accepts any spec; `LocalDockerRunner::provision` rejects it
on `valid_repo_url`/`valid_git_branch`. Demo mode is the advertised "try it cold" path and
`demo_mode_it.rs` runs entirely on the fake — so a mission that passes cleanly in demo can fail at
provision in real mode with "unsafe branch name", **after** the user has committed to a paid run. The
exposure is real: swarm branches are built from planner-generated text
(`agent/{swarm_id}/{idx}-{slug}`) and only the slug half is sanitized. Compounding it, the identical
four-clause branch allowlist is implemented twice — `valid_git_branch` (bool) and `guard_branch`
(Result) — each with its own test corpus, and nothing asserts they agree; loosen one and a name accepted
at provision is rejected at PR time, killing a unit after it has paid for the whole run.

**Concrete test.** Move the checks into a shared `validate_spec(&UnitSpec)` called by **both** runners,
then assert a demo swarm whose planner-chosen title yields a hostile branch fails identically to real.
Separately assert `valid_git_branch(s) == guard_branch(s).is_ok()` over one shared corpus.

### GAP-121 — The load-bearing sidecar-before-bundle order is written four times and CI does not reuse it

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `duplicated-logic` |
| **layer** | build_ci_gate |
| **verified_by** | automated |
| **anchors** | `cockpit/ui/package.json:scripts.bundle`, `.github/workflows/ci.yml:jobs.build`, `cockpit/ui/scripts/build-sidecar.mjs:dest` |
| **risk** | L3 × I5 = **15** |
| **observations** | coverage_pts=4, branches=2 (branch_pts=1), churn_90d=7 (churn_pts=4) |
| **anchor_sites** | 3 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** The ordering `ci.yml` calls "LOAD-BEARING" is spelled out in four places
(`scripts.desktop`, `scripts.bundle`, `ci.yml`, `release.yml`) and asserted only by prose. CI does
**not** reuse `npm run bundle`; it re-implements the order with the **debug** sidecar, so the CI bundle
is not the bundle `release.yml` produces — which matters because Part 2 (`GAP-012`) has never launched
either. `build-sidecar.mjs` itself has no test at all: not the triple regex `/host:\s*(\S+)/` (silently
throws on a rustc output change), not the win32 `.exe` suffix, not the release/debug fork, not the
`../..` relative cwd. A silent rename here surfaces as an opaque Tauri `externalBin` compile failure on
three OSes at once. Also worth fixing: `build-sidecar.mjs`'s header claims `tauri build` wires the
release sidecar via `beforeBuildCommand`, but `tauri.conf.json` sets that to `npm run build` (vite
only) — the comment is wrong and no test would notice.

**Concrete test.** Assert any job invoking `tauri build` is immediately preceded by a
`sidecar`/`sidecar:release` step, and that CI uses the same composite script `package.json` defines.
Plus `build-sidecar.test.mjs` stubbing `rustc`/`cargo` on PATH and asserting the copied filename is
exactly what `externalBin` resolves.

### GAP-122 — The embargo guard's `--all` mode, its skip paths, and its only write path are untested

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-branch` |
| **layer** | node-embargo |
| **verified_by** | automated |
| **anchors** | `scripts/embargo-guard.mjs:modeAll`, `scripts/embargo-guard.mjs:runOverFiles#skip-paths`, `scripts/embargo-guard.mjs:modeAddEntry` |
| **risk** | L4 × I5 = **20** |
| **observations** | coverage_pts=4, branches=6 (branch_pts=3), churn_90d=3 (churn_pts=3) |
| **anchor_sites** | 3 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** The guard is the repo's one real security gate and one of only two things CI runs —
yet `embargo-guard.test.mjs` exercises only `--staged` and `--message`. **`--all` is the mode CI
actually invokes** and has zero direct coverage, and it differs materially: it reads the working tree
while `--staged` reads the staged blob. The header states "we report every skip rather than passing
silently — a guard that quietly declines to look at something reads as clean when it never checked" —
and nothing tests that: `MAX_BYTES`, the unreadable catch, and especially `isBinary` (a NUL in the first
8000 bytes) are all exit-0 paths, and `isBinary` skips with **no warning at all**, so a token in a
UTF-16 doc passes as clean. Worst of all, `modeAddEntry` — the only write path and the only way a
denylist is ever created — is invoked by no test; every test hand-rolls its denylist via a helper that
duplicates the salt+digest derivation. If the two ever disagree, every test still passes and every real
denylist silently matches nothing.

**Concrete test.** Add `--all` cases (clean and leaking, plus an `ls-files` entry missing on disk); a
>8 MiB file, a NUL-containing file, and an unreadable file each asserting a **reported** skip; and a
`--add-entry` case asserting the written digest/salt, that the plaintext token appears nowhere in the
file, that re-adding exits 1, and that a second add preserves the first entry.

### GAP-123 — The git hooks and CI's inline commit-message range are shell nothing executes

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-symbol` |
| **layer** | build_ci_gate |
| **verified_by** | automated |
| **anchors** | `.githooks/pre-commit:GUARD_ROOT`, `.githooks/commit-msg:GUARD_ROOT`, `.github/workflows/ci.yml:jobs.embargo#base-ref-fallback` |
| **risk** | L4 × I5 = **20** |
| **observations** | coverage_pts=5, branches=2 (branch_pts=1), churn_90d=4 (churn_pts=3) |
| **anchor_sites** | 3 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** The guard-side half is tested; the **hook** side is not tested at all.
`GUARD_ROOT="$(cd "$(dirname "$0")/.." && pwd)"` and the node-missing fail-closed branch are shell no
test executes — and the pre-commit comment records the exact incident it prevents (a relative
`core.hooksPath` makes git find no hook in a worktree and silently commit), which is precisely the
condition nothing verifies. Bundled here: `ci.yml`'s embargo job contains bespoke inline shell that
exists only in the YAML and runs only on GitHub, with two untested branches — the base-ref fallback and
the `git rev-parse --verify` guard that degrades to `git log -50`. If the range resolves wrong the guard
scans the wrong commits and reports clean; the fallback also silently caps at 50, so on a long branch
older commits are never scanned and the failure mode is a **pass**.

**Concrete test.** `scripts/githooks.test.mjs`: a scratch repo with an absolute `core.hooksPath`,
asserting a leaking commit is rejected and no commit object created; a `git worktree` case asserting
`GUARD_ROOT` resolves back to the guard-bearing checkout; and a no-node PATH case asserting exit 1.
Extract the CI shell into `scripts/collect-commit-messages.sh` and cover both branches.

### GAP-124 — `demo-restart-recovery.mjs` is the only end-to-end durability check and nothing runs it

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-symbol` |
| **layer** | dev_scripts |
| **verified_by** | automated |
| **anchors** | `scripts/demo-restart-recovery.mjs:main` |
| **risk** | L4 × I3 = **12** |
| **observations** | coverage_pts=5, branches=8 (branch_pts=3), churn_90d=1 (churn_pts=2) |
| **anchor_sites** | 1 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** Nothing references it: no workflow step, no test, no other script. It is the only
artifact that exercises the headline "recovers durably across restarts" claim end to end — SIGKILL, cold
process, same SQLite file — and it runs only when a human types it, against a `target/debug/serve`
binary that must already exist (it exits 2 otherwise, so a stale build makes it a silent no-op). Its
PASS condition is real; it is simply never evaluated. Closely related to `GAP-041`, which is the same
durability claim at the unit level.

**Concrete test.** Port the invariant into `crates/fleetd/tests/restart_recovery_it.rs` (or extend the
non-ignored `demo_mode_it.rs`): dispatch a demo unit against a temp SQLite path, abort the server task,
reopen the store, and assert the reconstructed event log length equals the pre-kill length and is > 0 —
the same check the script prints, but as an assertion inside `cargo test --workspace`.

### GAP-125 — `index.html` loads Google Fonts against a CSP that has no `font-src` and no such origin

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `manual-uncovered` |
| **layer** | manual |
| **verified_by** | manual |
| **anchors** | `cockpit/ui/index.html:head` |
| **governs** | `cockpit/ui/index.html`, `cockpit/ui/src-tauri/tauri.conf.json` |
| **last_manual_pass** | — (never_verified) |
| **risk** | L4 × I3 = **12** |
| **observations** | manual_coverage_pts=5, churn_90d=1 (churn_pts=2), never_verified=true |
| **anchor_sites** | 1 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** `index.html` loads Oxanium and IBM Plex Mono from `fonts.googleapis.com` /
`fonts.gstatic.com` while `tauri.conf.json` sets `default-src 'self'; style-src 'self' 'unsafe-inline'`
with **no `font-src`** and no googleapis origin — so the packaged webview should refuse the stylesheet
and silently fall back to system fonts. Whether this already manifests in dev or only packaged is
**unverified** (the app could not be launched during this run). Either way the app depends on network
reachability at startup for its typography, unverified offline. Nothing in the 19-file vitest suite or
in either workflow reads `index.html` at all.

**Concrete test.** A vitest case reading `index.html` and `tauri.conf.json` and asserting every external
origin referenced by a `<link>`/`<script>` is permitted by the directive that governs it — which fails
today. Pair with a human check: launch the packaged app offline and confirm the typography is
intentional rather than accidental fallback.

### GAP-126 — The PowerShell hooks Claude Code actually executes are tested by nothing, in any repo language

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-symbol` |
| **layer** | pytest |
| **verified_by** | automated |
| **anchors** | `tools/budget-checkpoint/hooks/budget-checkpoint.ps1:script`, `tools/cache-countdown/hooks/cache-timer-write.ps1:script`, `tools/cache-countdown/hooks/cache-timer-resume.ps1:script` |
| **risk** | L4 × I2 = **8** |
| **observations** | coverage_pts=5, branches=9 (branch_pts=3), churn_90d=1 (churn_pts=2) |
| **anchor_sites** | 3 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** These are the processes Claude Code spawns on **every Stop event on the user's
machine**, and nothing tests PowerShell anywhere in this repo. Concrete hazards visible in 31 lines:
the shim pipes `uv run`'s stdout straight through to Claude Code, whose hook-output contract is strict
JSON — so any uv resolution notice on a cold `--project` dir yields malformed hook output; the
`try{}catch{}` is dead cover, because with `$ErrorActionPreference='Continue'` a native non-zero exit
does not raise a terminating error; and `uv run` cold start can exceed the 10 s hook timeout
`install.ps1` writes into settings.json, stalling the end of every turn. On the cache-countdown side,
`cache-timer-write.ps1` is the **sole producer** of the state file the feature consumes and its format
is only ever approximated in tests by a hand-written timestamp literal; it does an unlocked
read-modify-write of a file the resume hook also writes, and `$sid` is interpolated straight into a
path with no sanitization. The two hooks are also ~40 identical lines differing only in one boolean.

**Concrete test.** A pwsh harness with a stub `uv` on PATH: assert stdout is byte-exact (a stub printing
chatter before the JSON must FAIL), exit code 0 on stub exits 1/2/127 and when `uv` is absent, and
completion inside 10 s cold. For the timer hooks, pipe a realistic Stop payload with `$env:USERPROFILE`
at a temp dir and feed the result straight into `core.read_timer_file`, then repeat for empty stdin,
non-JSON, a missing `session_id`, a `session_id` containing path separators, a corrupt existing file,
and a drive-root `cwd`. Extract the shared body into one `Write-CacheTimer -Stopped <bool>` first.

### GAP-127 — `deploy_globals.py` mutates the user's real `settings.json` and `CLAUDE.md` with no tests at all

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-symbol` |
| **layer** | pytest |
| **verified_by** | automated |
| **anchors** | `tools/lane-z-integration/deploy_globals.py:merge_settings`, `tools/lane-z-integration/deploy_globals.py:append_claude_md`, `tools/lane-z-integration/deploy_globals.py:deploy_tools`, `tools/lane-z-integration/deploy_globals.py:recall_command` |
| **risk** | L4 × I2 = **8** |
| **observations** | coverage_pts=5, branches=13 (branch_pts=4), churn_90d=1 (churn_pts=2) |
| **anchor_sites** | 4 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** The only code in the module that mutates files the README itself flags as having
"no git rollback", and `tools/lane-z-integration/` has no pyproject and no tests directory. Four
distinct defects are visible: the idempotency guard is an exact string compare against a freshly
rendered absolute path, so any separator or quoting difference **appends a duplicate SessionStart hook
on every run**; `json.loads` on the existing settings is unguarded and raises *after* `backup()` and
`deploy_tools()` have already run, leaving a partially-applied deploy with no unwind;
`append_claude_md` warns "would create it" and then, under `--apply`, actually writes — the one case
with no backup, since `backup()` returns early for a nonexistent file; and `recall_command` hardcodes
the bare interpreter name `python`, which on a stock Windows box is frequently the Store
app-execution alias, silently making the recall hook dead at every session start (indistinguishable
from "no memory yet", because `recall.py` prints nothing in that case).

**Concrete test.** pytest against a temp `--config-dir`: run `--apply` twice and assert exactly one
recall entry, including when the same hook is spelled with different separators; a malformed existing
settings.json asserting a clear failure with CLAUDE.md untouched; the create path asserting the message
matches what actually happens; and an execution test that the emitted `recall_command` runs, with a
config dir containing a space.

### GAP-128 — `context-offload` has no test infrastructure, and its update path corrupts on any Windows path

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-branch` |
| **layer** | pytest |
| **verified_by** | automated |
| **anchors** | `tools/context-offload/offload.py:upsert_index#regex-replacement-escape`, `tools/context-offload/offload.py:resolve_memory_dir#unsanitized-slug`, `tools/context-offload/recall.py:main` |
| **risk** | L4 × I2 = **8** |
| **observations** | coverage_pts=5, branches=7 (branch_pts=3), churn_90d=1 (churn_pts=2) |
| **anchor_sites** | 3 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** No pyproject, no tests directory — not one line executes under any harness — and
`recall.py` is a **live SessionStart hook** with a 5 s timeout whose stdout is injected verbatim into
the model's context. Three concrete defects: `upsert_index` passes the caller-supplied entry as a regex
**replacement template**, so a summary containing `D:\proj\...` raises `re.error: bad escape \p` — and
only on the *second* offload of the same slug, exactly the idempotent re-run the docstring advertises
as safe; an explicit `--slug` bypasses `slugify` entirely, so a traversal writes outside the memory
store, and the caller composing that argument is an LLM; and `render_hook` does no escaping of note
titles or summaries, making the memory store an unescaped prompt-injection surface into every future
session, with `--max-notes` defaulting to unbounded so the tool built to reduce token cost injects the
whole index. Exit code is hardcoded 0, so every failure is silent.

**Concrete test.** Stand up a pyproject and tests dir, then: `upsert_index` with a backslash path, `\1`,
and `\g<0>` in the summary asserting literal insertion; `--slug '../../evil'` and `'a/b'` asserting
rejection; and `main()` against a temp `CLAUDE_CONFIG_DIR` asserting `--format hook` prints **nothing**
for a zero-entry index, that a summary containing `</session-memory>` cannot break the envelope, and
that a 500-entry index stays inside the 5 s budget.

### GAP-129 — cache-countdown's headline feature is inert, and its self-test cannot fail

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `manual-uncovered` |
| **layer** | manual |
| **verified_by** | manual |
| **anchors** | `tools/cache-countdown/src/cache_countdown/core.py:cost_at_stake`, `tools/cache-countdown/src/cache_countdown/ticker.py:_self_test` |
| **governs** | `tools/cache-countdown/**` |
| **last_manual_pass** | — (never_verified) |
| **risk** | L4 × I2 = **8** |
| **observations** | manual_coverage_pts=5, churn_90d=1 (churn_pts=2), never_verified=true |
| **anchor_sites** | 2 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** **Phase-3 confirmed on both halves.** cost-at-stake leads the pyproject description
and the README, but no writer in the repo ever populates `cached_tokens` — the two PowerShell hooks set
only `session_id`, `stopped`, `timestamp`, `project`, `cwd`, and merely copy pre-existing keys forward.
So `cost_at_stake` returns `None` on every real tick and the dollar figure never renders. Every test
that exercises it hand-injects the value, and `_self_test` injects the same constant — so **the one
human-facing check actively conceals the gap** by always displaying a cost the production path cannot
produce. And `_self_test` asserts nothing at all: it prints "N bells rung (expected … 6)" and
unconditionally `return 0`, so an operator seeing "4 bells rung" still gets a zero exit. (In fairness,
`core.py` and the README do disclose the missing writer in prose.)
*(A related claim — that `ticker.state_dir` and the hooks resolve `~/.claude` to different directories
under `CLAUDE_CONFIG_DIR` — was **Phase-3 refuted**: on Windows `Path.home()` resolves via `USERPROFILE`,
the same directory the hooks write to, and `recall.py` shares no file with the ticker. A latent
inconsistency, not a live defect. Recorded so it is not re-flagged.)*

**Concrete test.** Make `_self_test` compare against the expected bell count and stage sequence and
return non-zero on mismatch, then add a pytest that monkeypatches `bells_crossed` to 0 and asserts it
fails. Separately drive the real producer end to end: run `cache-timer-write.ps1`, then
`cache-countdown --file <that file> --once`, and assert explicitly whether a cost figure is present —
either wire a `cached_tokens` writer or document the feature as inert.

### GAP-130 — Both Python tools' console-script entry points are the untested side of the process boundary

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `untested-symbol` |
| **layer** | pytest |
| **verified_by** | automated |
| **anchors** | `tools/budget-checkpoint/src/budget_checkpoint/hook.py:main`, `tools/cache-countdown/src/cache_countdown/ticker.py:main`, `tools/budget-checkpoint/src/budget_checkpoint/core.py:count_turns` |
| **risk** | L3 × I2 = **6** |
| **observations** | coverage_pts=4, branches=5 (branch_pts=2), churn_90d=1 (churn_pts=2) |
| **anchor_sites** | 3 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** The tests cover `build_output`/`run` and the injected-dependency `run()` loop, but
never `main` — so everything unique to the real entry point is unexecuted: `sys.stdin.read()` (which
blocks until EOF; if Claude Code ever spawns the hook without closing stdin the process hangs to the
10 s timeout on every Stop), the stdout write+flush, the blanket `except Exception: return 0`, and the
module-level UTF-8 `reconfigure` that exists only because Windows consoles default to cp1252. On the
ticker side, argument parsing, the `--file`/`--session`/`--self-test` dispatch and the `load()` closure
are all untested, and `--interval` is unvalidated — `--interval 0` busy-spins at 100 % of a core, a
negative raises mid-loop. The unreachable `return 2` after `parser.error(...)` is a tell that this path
has never been run. Related and cheap: `count_turns` json-parses **every line of the full transcript**
on every Stop, and the tests feed it 2–60 line files, so the cost characteristic that matters is
asserted nowhere.

**Concrete test.** Subprocess-level tests invoking the installed console scripts: assert exit 0 and
empty stdout for garbage input, byte-exact JSON on a threshold crossing, UTF-8-decodable output with
non-ASCII content, and that the process **exits** rather than hanging on an open-but-idle stdin. For the
ticker, assert each `--file`/`--session` dispatch, the corrupt-file swallow, and `--interval 0`. Add a
scaled perf assertion on `count_turns` against a 40 k-line transcript.

### GAP-131 — Both `install.ps1` scripts are the same 85 lines twice, with an untested `Copy-Item` nesting hazard

| field | value |
|---|---|
| **status** | `open` |
| **claim_type** | `duplicated-logic` |
| **layer** | pytest |
| **verified_by** | automated |
| **anchors** | `tools/budget-checkpoint/install.ps1:script`, `tools/cache-countdown/install.ps1:script`, `tools/cache-countdown/install.ps1:Get-HookEntries` |
| **risk** | L4 × I2 = **8** |
| **observations** | coverage_pts=5, branches=3 (branch_pts=2), churn_90d=1 (churn_pts=2) |
| **anchor_sites** | 3 |
| **first_seen** | 2026-08-13 |
| **last_verified** | 2026-08-13 @ `a3edc78` (static-only) |
| **decision** | — |
| **rationale** | — |

**Risk rationale.** Identical param blocks, encoding line, four `Copy-Item` calls, uv block and trailing
`ConvertTo-Json`, differing only in names, a timeout value, and a comment. Every defect in one is a
defect in the other, and with zero PowerShell tests, fixing one and forgetting the other is the likely
outcome. Two shared hazards: `Copy-Item -Path <src>\src -Destination $InstallDir -Recurse -Force`
creates `<InstallDir>\src` on a first run but copies **into** it on a re-run (classic footgun,
unverified for this exact invocation), leaving a stale package at the path `uv run` actually imports —
so the user "reinstalls", sees success, and keeps running old code. And `Get-HookEntries`' output **is**
the install contract, pasted into a real settings.json, with nothing validating its shape: neither
script declares `#requires -Version 7.0`, and under Windows PowerShell 5.1 `ConvertTo-Json` unwraps a
single-element array into a bare object, producing an entry Claude Code silently ignores — the hook
simply never fires, with no error anywhere.

**Concrete test.** Run each installer twice into the same temp `-InstallDir` and assert the tree is
identical after the second run — specifically no `<InstallDir>\src\src` — and that `uv run --project`
still resolves. Run `-PrintHooksOnly` under **both** pwsh 7 and `powershell.exe` and assert the parsed
JSON has an array of matcher objects. Then factor the shared body into one parameterised script.

## 6. Change log

Append-only, newest first. One dated, commit-stamped block per run.

### 2026-08-13 @ `a3edc78` — first run (bootstrap)

**Freshness.** Scanned at `a3edc78` on `feat/plugin-runtime`. Per-tier: `vitest` GREEN
(19 files / 135 tests / 0 skipped), `node-embargo` GREEN (13/13), `node-session-state` GREEN (52/52),
`pytest` GREEN (24 + 29). `rust-workspace` and `tauri-host` **not run** — the cargo build was held by
a concurrent agent, so every Rust claim is stamped `(static-only)` and no `open → covered` transition
would have been licensed for those tiers. `manual`: `manual-baseline: partial` (1 of 12 rows has a
recorded pass).

**Bootstrap seeding (Phase 0b).** Twelve entries (`GAP-001`…`GAP-012`) seeded from
`spikes/SPIKE-RESULTS.md` — the "REMAINING HUMAN GATE" procedure and the "Smoke run 1 — 2026-08-10"
results table — one per checklist row, each marked `&lt;!-- human --&gt;` (escaped here on purpose: the
parser treats a bare marker outside a table row as a fatal error) and carrying its recorded result
(`1.9a` PASS, `1.5` FAIL-then-fixed, `1.9b` ANOMALY, the rest never run). Two superseded human-gate
docs (`docs/handoff/2026-06-24-human-gated-spikes-runbook.md`,
`docs/handoff/2026-06-25-spikes-handoff.md`) were **not** seeded from; see R4.

**Scan (Phase 2).** Twelve read-only Lane-A scanners over a `check-globs`-validated disjoint partition
of all 274 tracked files (14 modules + an anchorless bucket; 0 overlaps, 0 unmapped). No Lane B — there
were no existing entries to re-verify. Scanners returned **≈215 candidates**.

**Dedup (Phase 2b).** `churn_90d` swept once, whole-repo, over every flagged anchor. Consolidation by
any-anchor overlap reduced ≈215 candidates to **131 entries** (`GAP-001`…`GAP-131`, `next_id` now 132).
The material merges, recorded so the next run does not re-mint them:
- The five separate "tier X is not in CI" candidates plus their per-module restatements collapsed to
  `GAP-057`, `GAP-110`, `GAP-111`, `GAP-112`, `GAP-113`, `GAP-115`.
- `Runner::health` was raised independently by `fleetd_driver` and `fleetd_forge` → `GAP-018`.
- The `poll_mergeable` no-delay loop was raised by both → `GAP-022`.
- `FakeForge`'s three missing failure knobs (three candidates) → `GAP-023`; `FakeRunner`'s two →
  `GAP-116`.
- The blocking synchronous dashboard commands were raised by both `tauri_host` and `ui_dashboard` →
  `GAP-070` and `GAP-071`.
- The Phase-vocabulary duplication was raised by `fleet_core`, `fleetd_server` and `ui_shell` →
  `GAP-036` (Rust/SQL side) and `GAP-081`/`GAP-082` (UI side).
- `local_docker`'s pure-validator and exit-code-mapping candidates (four) → `GAP-118`;
  `gh_forge`'s three → `GAP-117`.
- Roughly thirty low-severity single-symbol candidates in `ui_dashboard`, `session_state` and
  `py_tools` were folded into thematically-anchored entries (`GAP-095`, `GAP-099`, `GAP-102`,
  `GAP-109`, `GAP-126`…`GAP-131`) rather than minted separately.

**Adversarial verification (Phase 3).** Four refuters over **26 targeted claims** — every claim
asserting a *live defect* or a *dead contract* rather than merely absent coverage. Verdicts:
**21 confirmed, 4 refuted, 1 uncertain.** False-positive rate **4/26 ≈ 15 %**, which is the number to
watch on the next run.

*Refuted and dropped (recorded so they are never re-flagged):*
- "The `app-plugins` capability over-grants webview-mutating permissions to third-party app content."
  The capability omits `remote`; the child webview loads an external URL, so Tauri classifies it
  `Origin::Remote` and the Local-only grants never resolve. **Inert, not dangerous.** Noted in `GAP-065`.
- "`WebviewPool::touch_and_evict` holds the LRU mutex across a blocking main-thread `close()`."
  `close()` compiles to a non-blocking `send_event`; the critical section is two channel sends. Also
  `plugin_set_rect` never touches `lru`, so the named contention pair is wrong. Noted in `GAP-064`.
- "`ticker.state_dir` and the PowerShell hooks resolve `~/.claude` to different directories under
  `CLAUDE_CONFIG_DIR`." On Windows `Path.home()` resolves via `USERPROFILE` — the same directory the
  hooks write to — and `recall.py` shares no file with the ticker. A latent inconsistency, not a live
  defect. Noted in `GAP-129`.
- "`resolve.mjs` and `SKILL.md` implement different plugin-resolution algorithms." SKILL.md step 3 does
  contain the existence check plus the same cache-scan fallback. The dead-code half (zero production
  importers) is real and is retained inside `GAP-109`'s neighbourhood, but the claimed failure mode is
  wrong.

*Uncertain, written `(unverified)`:*
- "`discard`'s `replacen("cc_", "ccvol_", 1)` mis-targets the volume." Every reachable `Handle.id`
  today starts with `cc_`, so the rewrite is currently correct. The unenforced prefix coupling and the
  swallowed `let _ =` result remain real; both are covered by `GAP-118`.

*Not refuted this run.* The remaining ≈105 entries assert absent coverage rather than a live defect and
were **not** put through a refuter — risk-tiered solo dispatch over 215 candidates was not affordable
on a bootstrap run. They are honest static findings, not adversarially verified ones. See **R3**.

**Transitions.** None — every entry is new. No `covered`, `accepted`, or `intentionally-red` entries
exist yet, so no promotion or demotion was possible.

**Needs ratification.** Six items open (R1–R6), the load-bearing ones being the whole `spine_weight`
table (R1) and the Phase-3 dispatch policy (R3).

**Concurrent work observed.** The working tree carried `M cockpit/ui/src-tauri/src/plugins/manager.rs`
and an untracked `cockpit/ui/src-tauri/tests/tauri_command_threading.rs` throughout the run — another
agent writing targeted tests for the `plugin_launch` main-thread defect and the `stop_all_owned`
teardown lifecycle. `cockpit/ui/src-tauri/src/plugins/**` was deliberately not scanned; `GAP-005`,
`GAP-009` and `GAP-010` are parked accordingly. Note the new guard test is a **signature-level**
ratchet, which is why `GAP-064` and `GAP-070` — blocking work one call below a correctly-`async`
command — are still open findings.
