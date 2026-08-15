# SP1 Phase 0 — Validation Spike Results

> Run 2026-06-05 on the target machine (Windows 11, Docker 28.3.3 / Linux-WSL2, git 2.45,
> Claude Code 2.1.163, Rust 1.93). Feeds the Phase 1 plan.
> Plan: [docs/superpowers/plans/2026-06-05-sp1-phase0-validation-spikes.md](../docs/superpowers/plans/2026-06-05-sp1-phase0-validation-spikes.md)

## Spike 1 — cross-platform git escape  → **PASS** ✅

**Claim proven:** an agent committing inside a Linux container, with the clone in a **named
Docker volume** (no NTFS bind-mount of `.git`), can have its branch exported as a complete
self-contained bundle, pulled to the Windows host, and reconstructed byte-identically.

**Evidence:**
- Repo init + 2 commits inside the volume (`alpine/git`, `--entrypoint sh`) with
  `core.autocrlf=false`, `core.fileMode=false`, `core.symlinks=false` — **no `index.lock`,
  fileMode, or autocrlf errors.**
- `git bundle create out.bundle agent/spike` → `git bundle verify`: *"The bundle records a
  complete history."* — **no prerequisites** (not incremental, as required).
- Export via temp container: `docker create … -v cc_spike_vol:/work` → `docker cp
  tmp:/work/out.bundle <host>` → `docker rm`. Bundle landed on host (551 B).
- Host (NTFS) `git clone out.bundle host_clone` → `git fsck --full`: **clean** (only a
  benign "unborn branch main" notice because the bundle carried just `agent/spike`; in
  production the host clone already holds the base and we `fetch` the branch into it).
- **SHA match:** container `81877030b00adfb9e536c08af57498ede8f72478` ==
  host `81877030b00adfb9e536c08af57498ede8f72478`.

**Reusable command sequence → becomes `Runner::export_bundle` (Phase 2):**
1. in-container: `git bundle create /work/out.bundle <branch>` (+ `git bundle verify`)
2. host: `docker create --name <tmp> -v <vol>:/work <img>`; `docker cp <tmp>:/work/out.bundle <host>`; `docker rm <tmp>`
3. host clone: `git fetch <bundle> <branch>` (non-bare clone; avoid `--bare` due to host
   `safe.bareRepository=explicit`).

**Notes for Phase 2:** set the three `core.*` configs at clone time; the daemon's host clone
is **non-bare**; provision the in-volume clone from the base (network is open, so the
container can clone the origin URL directly, or the daemon seeds the volume).

## Spike 2 — Claude Code cost/token metering  → **PASS (best case)** ✅

**Claim proven:** `claude --print --output-format stream-json --verbose` emits a terminal
`result` record with parseable per-invocation **cost AND tokens**; summable across calls.

**Evidence — fields in the `type:"result"` record:**
- `total_cost_usd`: `0.20697875` (top-level, real dollars — the ~$0.20 is Opus-1M
  cache-creation on a trivial prompt; field accuracy is the point)
- `usage.input_tokens` `9457`, `usage.output_tokens` `4`,
  `usage.cache_creation_input_tokens` `25535`, `usage.cache_read_input_tokens` `0`
- `modelUsage["claude-opus-4-8[1m]"].costUSD` `0.20697875` (per-model breakdown)
- `num_turns` `1`, `duration_ms` `6734`

**Cap decision (supersedes the spec's hedge):** USD cost is **directly enforceable**:
- **Daemon-side:** parse `total_cost_usd` from each `exec`'s `result` record; sum across
  the unit's runs → enforce the USD cap. No price table needed (cost is reported).
- **Daemon-independent backstop:** pass **`--max-budget-usd <remaining_cap>`** into the
  in-container agent invocation. This is a built-in CLI dollar ceiling that holds **even if
  `fleetd` dies** — strictly better than the `--max-turns`/token proxy the spec assumed.
- Relevant flags confirmed present: `--print`, `--output-format stream-json`, `--verbose`,
  `--max-budget-usd`, `--dangerously-skip-permissions` (and `--allow-dangerously-skip-permissions`),
  `--model`, `--fallback-model`, `--agents`, `--json-schema`, `--permission-mode`.
- `--max-turns` is **not** a flag in 2.1.163; use `--max-budget-usd` + wall-clock
  (`timeout`) for the watchdog instead.

## Net effect on the design

Both load-bearing assumptions hold. One improvement to fold into the spec: the
daemon-independent cost cap is **`--max-budget-usd`** (a real dollar ceiling), not a token
proxy — so the "honest cost caveat" in Section 4 is largely resolved.

## Rate-limit signal (2026-06-07)

**Goal:** Pin what `claude -p --output-format stream-json` actually emits on a sustained
429/529 and on a hard usage-cap breach, so the classifier patterns are not a guess.

**Environment:** Windows 11, Docker 28.3.3, `cc-agent:dev` image present (`eb9794e3eba3`,
644 MB). `ANTHROPIC_API_KEY` was **not set** in the environment at spike time — live API
calls were therefore impossible and no real 429/529 or usage-cap response could be
provoked.

**Findings — UNCONFIRMED**

| Scenario | Exit code | Terminal `result` record? | stderr/stdout text |
|---|---|---|---|
| Sustained 429 / 529 | unconfirmed | unconfirmed | unconfirmed |
| Hard usage cap | unconfirmed | unconfirmed | unconfirmed |

**Proceeding with conservative text patterns (as specified by the design):**

The classifier will match on the following substrings found anywhere in stderr or stdout:

- `rate limit`
- `rate_limit`
- `overloaded`
- `429`
- `529`
- `usage limit`

These patterns cover the known Anthropic HTTP status codes (429 = too many requests,
529 = overloaded) and the human-readable phrases Claude Code has historically emitted for
rate-limit and overload conditions. The classifier is built and tested against synthetic
fixtures; this spike only would have refined the patterns — the absence of a confirmed
signal does not block implementation.

---

## Plugin Runtime — Swarm Integration (Lane S)  → **CODE-COMPLETE; interactive smoke pending** ⏳

> Run 2026-07-17 on the target machine (Windows 11, Tauri 2.11.2 `unstable`). Integrates the
> view-plugin runtime (Lane V) + app-plugin embedding (Lane A) into the shared shell.
> Dispatch doc: [docs/SWARM-HANDOFF-plugin-runtime.md](../docs/SWARM-HANDOFF-plugin-runtime.md).

**What landed (branch `feat/plugin-runtime`):**
- **Lane V** (view-plugin core): `store.svelte.ts` command-sink extraction, `bridge.ts`
  (MessagePort `PluginBridge`/`PluginSession` + command policy + flood/rate kill), `loader.ts`,
  `cockpit/plugin-sdk/`, `plugins/reference/`. Merged conflict-free.
- **Lane A** (app-plugin core): `src-tauri/src/plugins/*` (manifest/discovery/lifecycle already
  on main, extended), Tauri `unstable` pinned `=2.11.2`, `capabilities/default.json` `app::*`
  capability, Audience proving manifest + dev-list discovery. Merged conflict-free.
- **Lane S** (this integration, single writer of the three shared files):
  - `tauri.conf.json`: host CSP — `frame-src http://ccplugin.localhost` + `connect-src` for
    fleetd (`127.0.0.1:8787` http/ws).
  - `src-tauri/src/view_plugins.rs` (new): production `ccplugin://` asset handler serving from
    dev roots (`CC_VIEW_PLUGINS_DEV`) ∪ packaged resources ∪ `~/.command-center/plugins`, with
    the **P4 findings** baked in — `script-src … http://ccplugin.localhost` (opaque origin ⇒
    `'self'` matches nothing), `Access-Control-Allow-Origin: *` (opaque-origin module fetch is
    CORS-mode), CSP as a **response header**, `connect-src 'none'`.
  - `src-tauri/src/embedding.rs` (new): `plugin_show`/`plugin_hide`/`plugin_set_rect` — **async**
    (P3: sync deadlocks webview creation), `plugin_hide` **parks off-screen** (never `hide()`,
    which forces a repaint/reload), warm-pool LRU (cap 3), label scheme `app::<id>`.
  - `lib.rs`: registers the `ccplugin://` scheme + the three embedding commands + `WebviewPool`.
  - `App.svelte`: ONE topbar switcher across Fleet (in-DOM, default) + Projects + view-plugins
    (sandboxed iframe) + app-plugins (native webview); `PluginBridge` handshake wiring;
    hide-on-overlay compositing (native webview parked when a host overlay opens); ResizeObserver
    rect tracking. Fleet ops grid behaviour unchanged (regression canary).

**Automated gates — all GREEN (real output):**
- `cd cockpit/ui/src-tauri && cargo test` → **28 passed; 0 failed** (clean compile against 2.11.2
  `unstable`; the `add_child`/`get_webview`/`get_window`/`register_uri_scheme_protocol` surface).
- `cd cockpit/ui && npm run check` → **0 errors / 0 warnings** across 352 files.
- `cd cockpit/ui && npm test` → **133 passed** (18 files), incl. bridge handshake 100×-zero-drop,
  command policy/flood-kill, store single-sink, and the `App.overlay`/`Switcher`/`ApprovalOverlay`
  ops-grid regression canaries.
- `cd cockpit/ui && npm run build` → clean (one benign `INEFFECTIVE_DYNAMIC_IMPORT` warning).

**Provenance note:** the P3/P4 GO verdicts + throwaway spike harnesses live on unmerged branches
`spike/app-plugins-webview-v2` (P3: `spike_show` async fix) and `spike/view-plugins-handshake`
(P4 full GO). Their load-bearing *findings* are carried into `embedding.rs` / `view_plugins.rs`
above; the spike files themselves were **not** merged (throwaway, per the handoff).

### REMAINING HUMAN GATE — interactive dev + packaged smoke (not run headlessly)

This is the spec's "spike-and-smoke" step: it needs a watched window and cannot be asserted from
an automated run. Steps:

1. **Dev seam env** (so `ccplugin://` + app discovery resolve to the repo in dev):
   - `CC_VIEW_PLUGINS_DEV=<repo>/plugins`
   - `CC_PLUGIN_SDK=<repo>/cockpit/plugin-sdk/index.js`  (served as `<id>/sdk.js`)
   - `CC_APP_PLUGINS_DEV=<repo>/cockpit/ui/src-tauri/app-plugins`  (for the Audience app tab)
2. **Dev:** `cd cockpit/ui && npm run desktop` (or `tauri dev`). Verify:
   - Switcher shows FLEET (default, ops grid unchanged) + PROJECTS + REFERENCE (view-plugin) +
     AUDIENCE (app-plugin).
   - REFERENCE → sandboxed iframe renders, `plugin-hello`→`ready` completes, a policed `launch`
     round-trips, `command-ack` rejection path fires; **no network** from the plugin.
   - AUDIENCE → native webview appears over the reserved rect; resize the window and confirm the
     webview stays glued; trigger a host overlay (stage a REAL launch) and confirm the webview
     **parks off-screen** (hide-on-overlay across the native boundary), then restores on close.
   - Switch AUDIENCE→FLEET→AUDIENCE with no leak / no orphaned webview; Fleet state preserved.
   - Verify Vite HMR still works under the new host CSP (P4 spike gate d).
3. **Packaged:** `npm run bundle` → run `target/release/*app*.exe`; repeat the above. Confirm the
   `ccplugin://` scheme + child webview both work in the packaged build (no dev server).

Record PASS/FAIL here once run on the target machine.

### Smoke run 1 — 2026-08-10 (dev, partial) → **1.5 FAIL, fixed; rest not yet run**

Run on the target machine against `feat/plugin-runtime` merged up to `main` (`725b630`). Ended
early: the first app-plugin activation exposed a blocking defect that made the rest of the
app-plugin checklist unmeasurable until fixed.

**Environment note.** `cockpit/ui/node_modules` was empty at session start (collateral from the
2026-08-09 cleanup), so `npm run check` / `npm test` failed with `'vitest' is not recognized` —
a toolchain failure, not a code failure. `npm ci` restored it. Port 8080 was held by an unrelated
`purposefull` Spring Boot process which exited on its own before launch.

| Item | Result |
|---|---|
| 1.1 switcher renders | not run |
| 1.2 fleet regression canary | not run |
| 1.3 REFERENCE view-plugin | not run |
| 1.4 command policy round-trip | not run |
| **1.5 AUDIENCE activation** | **FAIL — UI froze on tab click (root-caused + fixed, see below)** |
| 1.6 rect glue on resize | not run |
| 1.7 park-on-overlay | not run |
| 1.8 no leak on switch | not run |
| **1.9 Gate 5 — container teardown** | **PASS — `docker ps` empty after quit (baseline was 0)** |
| 1.9 Gate 5 — process exit | **ANOMALY — see below** |
| 1.10 HMR under host CSP | not run |
| Part 2 packaged | not run |

**1.5 FAIL — root cause.** `plugin_launch` was a *synchronous* `#[tauri::command]`, so it ran on
the main event-loop thread (the same P3 finding that forced the embedding commands to be `async`)
and blocked there on `docker compose build` plus the health/ready probe budgets. The whole UI was
frozen from the tab click until the stack came up. The code carried a standing note predicting
exactly this ("may block up to the probe timeout (~180 s) … can move to a background task") — the
smoke is what came due.

**Fix (this session).** `plugin_launch` now dispatches `run_start_sequence` to a dedicated OS
thread and returns immediately; a plain thread rather than an async-runtime worker because every
seam in the sequence is blocking (`Command::status`, `ureq`, `thread::sleep`), so a runtime worker
would only relocate the stall. `Ok` now means *dispatched*, not *healthy* — so `App.svelte`
stopped fabricating `pluginState[id]='healthy'` and stopped calling `plugin_show` directly; the
existing compositing `$effect` composites on the `plugin://state` `healthy` event instead. That
second half is load-bearing: with launch returning early, the old code would have pointed the
child webview at a URL that is not serving yet. Pinned by `src/App.appPlugin.test.ts` (2 tests,
verified red→green).

**Gate 5 — split result.** Container teardown **PASS**: `docker ps` empty after a graceful quit,
against a verified 0-container baseline; `fleetd-serve` exited and 8787/5173/8080 all released.
**Process exit ANOMALY**: the `app` process survived the window close (pid 13396, no window, 23
threads, still responding 15 s later, 41 MB). Teardown clearly ran — the containers came down —
but the process did not exit after it. Not diagnosed; **carry into the next smoke** and decide
whether it is a dev-only artifact of `tauri dev` supervision or a real shutdown defect.

**Automated gates re-verified after the fix:** `cargo test` 28 passed · `npm test` **135 passed**
(19 files, +2 new) · `npm run check` **353 files, 0 errors / 0 warnings**.

**Still merge-blocking:** everything marked "not run" above, plus the whole packaged pass. The fix
is verified only at the automated level — it has **not** been confirmed in a watched window.

### Smoke run 2 — 2026-08-15 (dev) → **db74a47 CONFIRMED; 4 new defects, 2 fixed**

Run on the target machine against `feat/plugin-runtime` @ `26cbd24`, operator-driven with
checkpoint-per-item. Nine of eleven items had never been executed before this run.

**Environment notes — two of these invalidate assumptions the handoff was written on.**

- `docker ps` was empty at preflight, but `docker ps -a` was **not**: `audience-minio-1`,
  `audience-postgres-1` and `audience-redis-1` sat in **`Created`** state, left behind by Smoke
  run 1. They broke the first AUDIENCE launch outright:
  `Conflict. The container name "/audience-redis-1" is already in use`.
- **The handoff's "images are prebuilt, so no 20-min build" is wrong.** `compose build` runs
  regardless; it rebuilt `audience-video` and `audience-ai_content` before `up`.
- The pre-warmed Tauri build was stale after `main` merged up — the `app` crate rebuilt cold
  (16m22s). Budget that for the packaged pass too.

| Item | Result |
|---|---|
| 1.1 switcher renders | **PASS** (limited — see note) |
| 1.2 fleet regression canary | **NOT RUN** — ops grid renders nothing; blocked by **D-3** |
| 1.3a REFERENCE iframe renders | **FAIL → fixed → PASS** (**D-1**) |
| 1.3b `plugin-hello`→`ready` | **PASS** — `connected · caps: log-append, real-launch-confirm` |
| 1.3c no network from plugin | **PASS** — `connect-src 'none'` delivered on the plugin document |
| 1.4a policed `launch` round-trip | **PARTIAL** — bridge round-trip works; terminal ack is `REJECTED (sink-error)` from **D-3** |
| 1.4b `command-ack` rejection path | **PASS** — `REJECTED (real-requires-confirm)`, correct reasonClass |
| **1.5 AUDIENCE activation** | **PASS — 1,127 samples, 0 unresponsive** |
| 1.6 rect glue on resize | **PASS** |
| 1.7 park-on-overlay | **BLOCKED** by **D-3** — overlay unreachable, mechanism untested |
| 1.8 no leak on switch | **PASS** |
| 1.9a Gate 5 — container teardown | **PASS** — 11→1 running, 27→17 total (10 fully removed) |
| 1.9b Gate 5 — process exit | **FAIL → root-caused → fixed; re-verification PENDING** (**D-4**) |
| 1.10 HMR under host CSP | **NOT RIGOROUSLY VERIFIED** — worked incidentally, operator did not observe |
| Part 2 packaged | **NOT RUN** |

**1.1 covers less than it appears to.** `VIEW_PLUGIN_INDEX` (`App.svelte:45`) is a hard-coded
build-time constant, not runtime discovery. A REFERENCE tab appears whether or not the plugin
runtime works at all — as D-1 proved. Do not read 1.1 as evidence about the runtime.

**1.5 — `db74a47` is CONFIRMED.** The pivotal item. Responsiveness was *measured*, not judged:
`Process.Responding` sampled at 1 Hz across three traces totalling **1,127 samples with zero
unresponsive**, spanning a full `compose build`, a failed `up`, and a clean 0→3→10 container ramp.
The window stayed interactive through exactly the workload that froze it in run 1.

#### D-1 — view-plugins could not load on Windows at all *(FIXED this session)*

`pluginSrc()` emitted the literal `ccplugin://localhost/<id>/<entry>`. On Windows/WebView2 — which
`view_plugins.rs:5` itself calls the primary target — a custom scheme is reachable only as
`http://<scheme>.localhost/…`; the literal form is an *external protocol*, and
`sandbox="allow-scripts"` forbids navigating to one. The frame never navigated and stayed blank:

> `Navigation to external protocol blocked by sandbox, because it doesn't contain any of:
> 'allow-top-navigation-to-custom-protocols', …`

The codebase already contradicted itself here: the host CSP (`frame-src http://ccplugin.localhost`)
and `pluginSrc`'s own docstring both name the Windows origin the code never produced.

**Why every gate missed it:** `loader.test.ts:52` asserted the broken string as correct. CI has been
green on a view-plugin runtime that cannot load on the primary target platform. `tauri build` only
compiles; nothing in CI ever navigates the iframe.

Fixed test-first: `pluginSrc` takes an injected `isWindows` (defaulting to a UA check) and returns
the `http://ccplugin.localhost` form there. Verified live — REFERENCE rendered and completed its
handshake without an app restart.

#### D-2 — capability negotiation is dead code at runtime *(NOT fixed — needs a decision)*

The operator's status line read `caps: log-append, real-launch-confirm` for a plugin whose manifest
requests **only** `log-append`. Cause: `grantedCapabilities` is computed at `loader.ts:140` and read
in exactly one place in the whole codebase — a test (`loader.test.ts:75`). `App.svelte:237`
constructs `PluginBridge` without passing `capabilities`, so `bridge.ts:594` falls back to
`this.opts.capabilities ?? [...HOST_CAPABILITIES]` and ships the **full host set** in `init`.

Every view-plugin is granted every host capability regardless of its manifest. The manifest's
capability declaration is decorative. This is security-relevant in a sandbox/capability system.

#### D-3 — fleetd serves no CORS headers *(PRE-EXISTING on `main`, not a #49 regression)*

`OPTIONS /missions` → **405**; `GET /health` with an `Origin` returns **no**
`Access-Control-Allow-Origin`; and `crates/fleetd/src/*.rs` contains no CORS handling anywhere. So
every browser `fetch` from the cockpit to the daemon fails. One cause, three blocked items:

- **1.2** — `listUnits` is CORS-blocked, so the ops grid stays empty. Confirmed directly: three real
  units (`u1` failed, `u2` done, `u3` awaiting_oracle_approval) existed in the daemon and the grid
  still showed nothing.
- **1.4a** — the plugin's policed launch reaches the sink correctly, then `createMission`
  (`api.ts:16`) fails preflight → `REJECTED (sink-error)`.
- **1.7** — the overlay is `$derived` from a unit in `awaiting_oracle_approval`. `u3` was parked
  there on the daemon, but the cockpit cannot discover units, so the overlay never opened.

`git diff origin/main...HEAD` is **empty** for `cockpit/ui/src/lib/api.ts` and `crates/fleetd/`, so
the failing code is untouched by this branch. Whether this seam ever worked end-to-end is unverified
— note that WebSocket state streaming is CORS-exempt, so only the HTTP verbs are affected.

#### D-4 — the app never exits: an infinite exit loop *(FIXED; re-verification PENDING)*

Run 1's undiagnosed anomaly, reproduced and root-caused. `lib.rs` called `api.prevent_exit()`
unconditionally, ran teardown, then `app_handle.exit(0)` — which **re-emits `ExitRequested`**, so the
handler prevented it again, forever.

Measured: after a graceful close the process held **20 threads, no window, `Responding=True`,
spinning at 93.9% of one core**, having burned **309 s of CPU** by the time it was killed.

This **answers the handoff's open question 1 definitively: a real shutdown defect, not a `tauri dev`
supervision artifact.** The process tree showed `cargo` *blocked on* the app, not holding it open,
and a hot spin loop is not what supervision looks like.

**`0d05f55` made this worse while appearing to rule it out.** The handoff states its
`stop_all_owned_is_idempotent` test "already eliminated" the second-teardown-pass hypothesis. Making
teardown idempotent removed the 30 s cost per iteration and converted a slow loop into a hot one.

It also explains the project's own **trap #2** (`tauri-build` cannot overwrite `fleetd-serve.exe`
while the app runs) — that trap exists *because* the app never exits.

Fixed test-first with a `ShutdownGuard` (`lib.rs`): the first `ExitRequested` is prevented so
teardown runs; every later one is allowed through. Pinned by
`shutdown_guard_tests::only_the_first_exit_request_is_prevented`, verified red→green.

**Not fixed, deliberately (one change at a time):** `stop_all_owned(30_000)` still runs
*synchronously inside the `RunEvent` callback*, i.e. on the main event-loop thread — the same
architectural mistake `db74a47` fixed for `plugin_launch`. The trace shows the loop blocked ~2.5 min
(window closed ~13:15:30, containers reaped 13:18:00). The guard makes the process exit; it does not
make it exit *promptly*.

#### D-5 — a failed app-plugin launch is invisible in the UI *(NOT fixed)*

The first AUDIENCE launch failed on the container-name conflict and the UI showed **nothing** — no
chip, no error state. `plugin_launch` returns "dispatched", so there is no rejected promise, and the
background failure never reached the shell. Operator comment: *"It's empty, and I see no UX to
indicate anything is loading, which is a problem in itself."*

#### D-6 — Gate 5's success criterion uses the wrong instrument

Run 1 recorded teardown as PASS on "`docker ps` empty". `docker ps` lists only *running* containers
and cannot see the `Created`/`Exited` residue that teardown actually left — residue which then broke
the next run's launch. Gate 5 must assert on `docker ps -a` scoped to the project. Run 2's teardown
genuinely does better: total containers went 27→17, i.e. ten were **removed**, not merely stopped.

#### The systemic pattern

D-1 and D-2 are the same shape: **a unit test passes on a function whose output is never wired to
anything.** `pluginSrc` was tested against the broken string; `negotiateCapabilities` is tested in
isolation while its result is discarded. D-4 is the third instance of the main-event-loop-blocking
family already documented in `tests/tauri_command_threading.rs`. All three passed every automated
gate. Feed this into `docs/testing/PLAN.md`: the gap is integration-boundary coverage, not unit
coverage.

**Automated gates after this session's fixes:** `cargo test` **37 passed** · `cargo fmt` clean ·
`cargo clippy --all-targets -D warnings` clean · `npm test` **137 passed** (19 files) ·
`npm run check` **353 files, 0 errors / 0 warnings**.

**Still merge-blocking #49:** D-4's fix is unverified in a watched window (re-verification in
progress); D-2 needs a decision; 1.2/1.4a/1.7 are blocked behind D-3; 1.10 is unverified; and the
**entire packaged pass (Part 2) has still never been run**.
