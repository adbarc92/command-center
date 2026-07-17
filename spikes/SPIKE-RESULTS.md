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
