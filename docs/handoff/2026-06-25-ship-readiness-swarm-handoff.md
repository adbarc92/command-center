# Ship-Readiness Audit + Swarm Handoff (2026-06-25)

**Branch audited:** `spike/view-plugins-handshake` @ `9fbce0a` (P4 spike committed; working tree clean
except untracked `.claude/`, `.context-curator/`).
**Question this answers:** what must we *do or confirm* to ship the Command Center, and which of that
is swarmable vs. human-only.
**Verdict:** **engineering is done and verified green; the remaining gap is human-gated validation +
procurement.** The two feature builds behind the gates are decomposed below into non-colliding lanes,
dispatch-ready the moment their spikes say GO.

> **Do not dispatch the swarm yet.** Both feature lanes (A1, A2) are *blocked* on human-gated spikes
> (P3, P4) that an agent cannot run. Producing this handoff is safe; fanning out agents is opt-in and
> not even *possible* until the gates pass. Lane H below is the only thing dispatchable today.

---

## Part 1 — Audit: what's verified green (re-run this session, not trusted from docs)

| Gate | Command | Result |
|---|---|---|
| Rust engine (`fleet-core` + `fleetd`) | `cargo test --workspace` | **exit 0** — all green; 3 real-Docker/network ITs `#[ignore]`d (need a dind/network runner) |
| Session-state plugin | `node --test plugins/session-state/test/*.test.mjs` | **46/46 pass** (ROADMAP said 40; suite grew) |
| Cockpit type-check | `cd cockpit/ui && npm run check` | **0 errors, 0 warnings**, 335 files |
| Cockpit unit suite | `cd cockpit/ui && npm test` | **72/72 pass** (10 files) |

**Not re-run this session (slow; claimed green by the 2026-06-23 readiness handoff, command given to
re-verify):** the full packaged build — `cd cockpit/ui && npm ci && npm run sidecar && npm run tauri
build` → MSI + NSIS bundles, exit 0. Re-run before the signed release.

**What exists and runs (architecture, condensed):**
- **Rust engine** — multi-agent mission dispatch, USD budget cap (`--max-budget-usd`), rate-limit
  retry, swarm-decomposition engine, sqlite store.
- **Tauri cockpit** — fleetd sidecar supervisor, app-plugin lifecycle manager (`stopped→building→…
  →healthy`, fully tested), project dashboard (stage inference + halyard/Audience adapters), approval
  overlay, plugin switcher, live updater runtime.
- **Workflow layer** (North-Star "low cost"/"context hygiene") — cache-timer, rate-limit retry,
  budget-discipline rules, Tier-1 context offload (MEMORY.md + the hardened session-state plugin).

## Part 2 — Audit: the gap to ship (ranked by what it unblocks)

These are the **only** things standing between here and a shippable, feature-complete Command Center.
None is an engineering deficiency in merged code.

| # | Blocker | Type | Unblocks | Owner |
|---|---|---|---|---|
| **CI** | GitHub Actions **red on every branch** — *billing failure* (no runner allocated; jobs die ~3s, `runner=0`, `steps=0`). Confirmed unrelated to code (all suites green locally). | 💳 billing fix (~5 min) | Honest CI signal; merges stop needing `--admin` | **You** |
| **P3** | App-plugin **webview spike**, gates 2–5. Harness exists; `spike_show` **hangs** (sync `#[tauri::command]` calling `window.add_child`, which deadlocks on the main thread). Fix = make the command `async` (or wrap in `app.run_on_main_thread`), then visually confirm renders / resize ≤150ms / hide-on-overlay no-flash / no orphan container on quit. | 🔴 human-gated (visual judgment) | **Lane A1** (app-plugin embedding) | **You** |
| **P4** | View-plugin **handshake spike**. Harness **scaffolded this branch** (`ccplugin://` scheme + 100-reload driver). Awaiting the watched run: `plugin-hello→init` across **100 reloads, 0 drops**, dev **and** packaged; record gates a/b/c-CORS/c-CSP/d to `spikes/SPIKE-RESULTS.md`. | 🟠 human-gated (watched run) | **Lane A2** (view-plugin runtime) | **You** |
| **S3** | One **live paid T1 mission**. Set `ANTHROPIC_API_KEY`; watched oracle→build→review→PR on a throwaway repo. Validates the spine on real tokens (not a build gate). | 🟠 human-gated + spend | Confidence in the autonomous spine | **You** |
| **Certs** | **Code-signing certs** — Apple Developer ID ($99/yr + notarization) + Windows Authenticode. Wiring + secret names already done (`release.yml`, `tauri.conf.json`). | 🟣 procurement (~1wk lead) | The **signed cross-platform release** run | **You** |

**Recommended order:** CI billing → P3 → P4 → dispatch A1∥A2 → Lane S integration → S3 + Certs
(parallel, then the signed release run). Lane H (below) can run any time, today.

### How to run the human-gated spikes
- **P3:** `docs/handoff/2026-06-24-human-gated-spikes-runbook.md` + `2026-06-15-P3-spike-resume.md`
  (the `spike_show` bug + gate criteria). Record to `spikes/SPIKE-RESULTS-app-plugins.md`.
- **P4:** `spikes/SPIKE-RESULTS.md` already has the gate table + run commands wired
  (`npm run desktop` → "⌬ VP SPIKE" → "▶ RUN 100-RELOAD"; then `npm run bundle` for packaged).

---

## Part 3 — Swarm handoff: the feature build behind the gates

Two independent feature builds + one shell-integration owner + one ready-now hardening lane. The core
risk (false independence → two lanes editing one file) is resolved by giving the **shared shell files
a single owner (Lane S)** that integrates last; A1 and A2 keep their logic in *new, exclusively-owned
files* and file contract requests against the shell.

**Shared contracts (single-owner = Lane S):**
| File | Owner | A1 may request | A2 may request |
|---|---|---|---|
| `cockpit/ui/src/App.svelte` | **Lane S** | a switcher entry + mount its `AppPluginView` component | a switcher entry + mount the `ViewPluginSlot` |
| `cockpit/ui/src-tauri/src/lib.rs` | **Lane S** | register `plugin_show/hide/set_rect` in `invoke_handler!` | `register_uri_scheme_protocol("ccplugin", …)` |
| `cockpit/ui/src-tauri/tauri.conf.json` | **Lane S** | (webview config if needed) | add `frame-src http://ccplugin.localhost` to host CSP |

Everything else each lane writes is exclusively its own.

### Lane A1 — App-plugin embedding   ·   **blocked on P3 GO**
- **Scope:** make a running app-plugin (proving app: Audience) render inside the cockpit — backend
  webview commands + a self-contained Svelte view component. Carry the spike's proven `spike_show/
  hide/set_rect` logic forward as the production implementation.
- **Owns (exclusive write):**
  - `cockpit/ui/src-tauri/Cargo.toml` (add `tauri = { version = "=2.11.2", features = ["unstable"] }`)
  - `cockpit/ui/src-tauri/capabilities/default.json` (webview API perms for `app::<id>` labels)
  - `cockpit/ui/src-tauri/src/plugins/manager.rs` (add async `plugin_show/plugin_hide/plugin_set_rect`)
    — or a new `cockpit/ui/src-tauri/src/plugins/webview.rs`
  - `cockpit/ui/src/lib/app-plugin/AppPluginView.svelte` (**new** — placeholder rect + `ResizeObserver`
    → `plugin_set_rect`, state chips off `plugin://state`, `overlay-open/close` → `plugin_hide`)
  - `<config-dir>/app-plugins/audience/app-plugin.json` (Audience manifest: build args, health/ready
    probes, devAuth path)
- **Reads (no write):** `cockpit/ui/src-tauri/src/plugins/{manifest,discovery,state}.rs`; the spike's
  `spike_show` impl as the reference for the working webview API.
- **Shared contract:** files in the table above → owned by **Lane S**; A1 files the two requests.
- **Depends on / blocks:** depends on **P3 GO** (must know the exact webview API + that the deadlock
  fix holds); blocks nothing (Lane S waits on it).
- **Done when:** Audience launches→renders inside the rect; resize settles ≤150ms; hide-on-overlay
  round-trips ≤150ms with no stale-content flash over ≥10 trials; quitting the cockpit leaves **no
  orphaned containers** (`docker ps`).
- **Verify:** `cargo test --workspace` green; `cd cockpit/ui && npm run check && npm test` green; then
  a manual `npm run desktop` walk of the four observations above.
- **Notes / open Qs (from the spec):** child-webview CSP (does host `csp:null` cascade in 2.11
  `unstable`?); OAuth-popup session partition; external-nav (Stripe) stays in-app; adopt-check TOCTOU.
  If hide-on-overlay can't meet ≤150ms without flicker, the spec's fallback is a separate
  `WebviewWindow` per app (changes only the embedding surface; everything upstream is reusable).
- **Effort:** ~14–23h serial (Tauri config + webview commands + Svelte view + Audience E2E). Backend
  and Svelte can scaffold in parallel but integrate as one contract — keep them in **one lane**.

### Lane A2 — View-plugin runtime   ·   **blocked on P4 GO**
- **Scope:** the sandboxed-iframe view-plugin runtime — host↔plugin MessagePort bridge with command
  policy, a convenience SDK, a dev/packaged loader, and a reference plugin that dogfoods the full
  message surface. **Branch from `spike/view-plugins-handshake`, not `feat/view-plugins`.**
- **Owns (exclusive write):**
  - `cockpit/ui/src/lib/bridge.ts` (**new** — handshake, per-unit dirty-delta `state` msgs ~60ms
    debounce, `log-append` seq cursor, `log-reset` on reconnect, inbound flood-kill + outbound
    shape/authority/cost/rate policy + `command-ack`, 3s ready-timeout → ops-grid fallback)
  - `cockpit/ui/src/lib/loader.ts` (**new** — dev impl scans the vite plugin index; packaged impl
    scans `<config-dir>/plugins/` + bundled resources via the `ccplugin://` scheme)
  - `cockpit/plugin-sdk/` (**new** — `src/index.ts` `connect()` API, `package.json`, `tsconfig.json`,
    esbuild build, author types)
  - `plugins/reference/` (**new** — manifest + `index.html` + `index.ts` exercising state-apply,
    log-append, command launch + ack-rejection, oracle-approval `degraded` flag; ship **inlined**
    bundle so it's immune to the c-CORS risk)
  - the **real** scheme-handler module that replaces the spike's `spike_view_plugins.rs` (move its
    request/response shape + verbatim CSP header into the production handler)
  - `spikes/SPIKE-RESULTS.md` (record the P4 result)
- **Reads (no write):** `cockpit/ui/src/lib/store.svelte.ts` (dirty-set source — already extracted),
  `cockpit/ui/src/lib/ApprovalOverlay.svelte` (host overlay — already done), and the spike files as
  the working reference for the handshake + CSP.
- **Shared contract:** files in the table above → owned by **Lane S**; A2 files the scheme-registration
  + `frame-src` CSP + switcher-mount requests.
- **Depends on / blocks:** depends on **P4 GO** (the spike must prove handshake + CSP from the scheme
  handler); blocks nothing (Lane S waits on it).
- **Done when:** reference plugin handshakes 100× with 0 drops; full policy matrix tested
  (valid/shape/authority/rate/flood); liveness timeout → ops-grid fallback; dev **and** packaged
  smoke pass.
- **Verify:** `cd cockpit/ui && npm run check && npm test` green (incl. the new bridge/policy/loader
  tests); `npm run build` clean; reference-plugin handshake harness 100/100.
- **Notes / open Qs (from the spec):** ship **Spec-A (runtime + reference) this cycle**, defer Spec-B
  (the "Battlefield" RTS skin) to cycle 2 (spec's recommendation — the runtime alone is ~6–7
  security-critical workstreams). If gate c-CORS fails, the inlined reference bundle already sidesteps
  it; fallback #2 (loopback static server) is documented if modular loading is later needed. Reconnect
  replay-from-0 is inherited and out of scope (`Snapshot.last_seq` exists but unused).
- **Effort:** ~4–6 days serial (bridge ~2–3d security-critical; SDK + loader + reference ~2–3d).

### Lane S — Shell integration owner   ·   **blocked on A1 ∥ A2 complete**
- **Scope:** the single owner of the hot shared shell files. Integrates last: applies A1's and A2's
  contract requests, removes the throwaway spike scaffold, and de-stales `feat/view-plugins`.
- **Owns (exclusive write):** `cockpit/ui/src/App.svelte`, `cockpit/ui/src-tauri/src/lib.rs`,
  `cockpit/ui/src-tauri/tauri.conf.json`. **Deletes** the spike scaffold once A2 lands the real
  handler: `cockpit/ui/src-tauri/src/spike_view_plugins.rs`,
  `cockpit/ui/src/lib/spike/SpikeViewPlugins.svelte`,
  `cockpit/ui/src-tauri/src/spike-view-plugin/`, the spike's `App.svelte` "⌬ VP SPIKE" button, and the
  spike's scheme registration in `lib.rs`.
- **Reads (no write):** the merged A1 and A2 lanes; each lane's final report (the contract requests).
- **Depends on / blocks:** depends on **both A1 and A2**; this lane is the integration barrier.
- **Done when:** both an app-plugin and a view-plugin are reachable from the **one** switcher; the
  ops-grid default view is unchanged; the spike scaffold is gone; `feat/view-plugins` reconciled
  against current main (disjoint files — no conflicts expected; it is *not* the build baseline).
- **Verify:** full merged build — `cargo test --workspace` && `cd cockpit/ui && npm run check && npm
  test && npm run tauri build` all green; manual `npm run desktop` switch app→view→ops-grid.
- **Notes:** this is the reconciliation pass. Keep it thin — it writes only the three shared files and
  the deletions; it does not build features.

### Lane H — Hardening + housekeeping   ·   **READY NOW (independent, dispatchable today)**
- **Scope:** the one open session-state hardening item + doc/state reconciliation. No overlap with
  any other lane.
- **Owns (exclusive write):** `plugins/session-state/<keying module>.mjs` (H4 fix — normalize
  path separators before the repo-key compare so a backslash-vs-forwardslash difference between git
  output and a prior write stops triggering a spurious `COLLISION`); `docs/ROADMAP.md` (reconcile:
  H1–H3 shipped via PR #31 → strike from the backlog, leaving **H4** as the lone open item;
  refresh the "Requires your attention" status line).
- **Reads (no write):** the session-state test suite.
- **Depends on / blocks:** nothing.
- **Done when:** a path-separator-mismatched meta no longer produces a spurious collision; a
  regression test covers it; `node --test plugins/session-state/test/*.test.mjs` green; ROADMAP no
  longer lists already-shipped hardening as open.
- **Verify:** `node --test plugins/session-state/test/*.test.mjs` → all green incl. the new H4 case.
- **Notes:** loose ends to clear while here — delete
  `~/.claude/settings.json.pre-sessionstate-migration.bak`; `rm -rf` the lingering
  `.claude/worktrees/agent-…` dir once its handle releases; reinstall the session-state plugin at the
  hardened version (installed copy is still pre-H1/H2/H3).

---

## Rules of the road (paste into every dispatched agent)
1. **Stay in your lane.** Write only files your lane owns. Need a change in a shared shell file?
   Record a *contract request* in your final report — never edit Lane S's files.
2. **Worktree per lane.** One git worktree/branch per lane (A1, A2, H run concurrently → isolate).
   A2 branches from `spike/view-plugins-handshake`.
3. **Shared shell files are Lane S's, single-owner.** A1/A2 request; S writes.
4. **Don't widen scope.** Build only your lane's items; report anything else you find.
5. **Verify before claiming done.** Run your lane's Verify command; paste the real output.
6. **Report for integration.** End with: files changed, contract requests, verify output, anything
   affecting another lane.

## Integration order
1. **Today (human):** fix CI billing; (optional) dispatch **Lane H**.
2. **Human spikes:** run **P3** and **P4**; record GO/NO-GO to the spike-results files.
3. **On P3 GO → dispatch A1; on P4 GO → dispatch A2** (parallel, worktrees).
4. **Lane S** integrates once A1 ∥ A2 land: apply contract requests, delete spike scaffold, de-stale
   `feat/view-plugins`.
5. **Reconcile:** full merged build + test + packaged smoke (Lane S Verify).
6. **Human:** S3 live mission + Certs procurement → the **signed cross-platform release** run
   (CI is otherwise ready by name).

## Still-blocked after the swarm (and what unblocks them)
- **Signed release** ← Certs procurement (Apple Developer Program + Windows Authenticode).
- **Spine confidence** ← S3 one live paid mission (key + watched hour).
- **Spec-B "Battlefield" view-plugin** ← deliberately deferred to cycle 2 after the runtime hardens.
- **Tier-2 context offload (item 3)** ← the claude.ai MCP connector being reliably available headless.
- **6B budget hygiene** ← ContextCurator (the user's own product) shipping its `cc_*` API.
