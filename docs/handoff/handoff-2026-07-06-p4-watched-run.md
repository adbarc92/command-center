# Handoff — 2026-07-06 — P4 view-plugin handshake, watched run pending

Branch: `spike/view-plugins-handshake` @ `9fbce0a`. Backup handoff written while
`tauri dev` is running for the P4 watched run.

## 🔀 SESSION PIVOTED — now brainstorming: Local Project Tracker + Roadmap Dispatch

The P4 debug is PARKED (user will re-run the spike later; `tauri dev` still running; the
diag instrumentation is in place awaiting log readings — see "ACTIVE DEBUG" section below).
Current work is a `superpowers:brainstorming` session designing a NEW feature = a **fifth
source for the existing Project Dashboard** (roadmap item 4, already designed+built in
[2026-06-09-project-dashboard-design.md](../superpowers/specs/2026-06-09-project-dashboard-design.md)
and [cockpit/ui/src/lib/dashboard/](../../cockpit/ui/src/lib/dashboard/)).

**Resolved decisions (via clarifying Qs) — do NOT relitigate:**
1. Discovery = **hybrid** (scan a root e.g. d:\MajorProjects + manual pin/exclude).
2. Stage source = **explicit marker only** in STATUS.md front-matter (`stage:` + `readiness:`
   + `updated:`); un-stamped projects don't appear. Modify the doc-writing skills to stamp it.
3. Marker home = **STATUS.md** front-matter for stage; **ROADMAP.md items** become structured,
   dispatchable "things to feed agents."
4. Feed-an-agent = **dispatch a live fleet mission** via `POST /missions {task,tier}`
   ([api.ts:16](../../cockpit/ui/src/lib/api.ts#L16)); dispatched missions already loop back
   onto the board through the Fleet adapter (`phase_changed` → fleet card). Loop closes on
   existing rails.
5. Structure = **one spec, phased build** (Phase 1 tracking U1–U4; Phase 2 dispatch U5).
6. Roadmap schema = **structured items in ROADMAP.md** — per-item HTML-comment machine header
   `<!-- cc-item id= status= tier= lane= -->` + optional `**Dispatch:**` body (the task text).

**Design units (U1 STATUS convention, U2 ROADMAP convention, U3 skill changes, U4 `local`
adapter, U5 dispatch+loopback).** Two decisions still OPEN, presented to user with recs:
(a) new `Source='local'` vs reuse `'manual'` (rec: new `local`); (b) U5 item-status write-back
INTO ROADMAP.md vs dashboard-store-only (rec: write back — it's the user's own doc; the one
place the "read-only projector" principle bends).

**Current position:** decomposition APPROVED. U1 (STATUS front-matter: `stage` required +
`readiness`/`updated`/`blocked`/`name` optional; invalid stage → degraded card not silent-drop),
U2 (ROADMAP `<!-- cc-item id= status=open|active|blocked|done tier= lane= -->` + `**Dispatch:**`
body; task resolution: Dispatch block → prose → title; validator flags dup ids), and U3 (STATUS
stamping in end-session/save-state/handoff; ROADMAP via convention+validator) APPROVED. U4
PRESENTED + awaiting OK: Rust half `scan_local_projects(config{scanRoots,pins,excludes})` reads
raw file text; TS half parses front-matter+cc-items → ProjectCard[] (pure/testable). Card:
id=`local:<slug>`, source=`'local'` (new union member — resolves open-decision-a), stageSource=
`'declared'`, staleAfterSec≈2×poll (doc-age≠staleness), poll ~30s. Two additive model changes:
`Source` gains `'local'`; optional `dispatch?:{items:RoadmapItem[]}` on ProjectCard (Phase-2 UI
only). Phase-1 does NOT force-merge local vs audience/fleet cards; `family` is the later clustering
seam. **NEW WORKSTREAM — cockpit design overhaul (2026-07-06):** user wants a full redesign of the cockpit
via **Claude Design** (Anthropic Labs browser product — NOT a CLI tool; I can't drive it from here).
Chosen path: user runs Claude Design in-browser, I assist + implement. I wrote the design brief at
`docs/design/claude-design-brief.md` (subject, surfaces, current "Fleet Command HUD" baseline in
app.css, anti-templated guardrails, files for it to read). AWAITING the user's Claude Design output
(token set / component specs / screenshots) → then I implement on branch `design/cockpit-overhaul`:
map tokens into cockpit/ui/src/app.css `:root`, restyle App.svelte/Dashboard.svelte/ApprovalOverlay.
svelte/Switcher.svelte, verify in tauri dev, PR. Back in MAIN tree now (exited local-tracker worktree,
kept for PR #36 iteration).

**✅ PHASE 1 BUILT & SHIPPED AS PR #36 (2026-07-06):** the Local Project Tracker Phase-1 (U1–U4 + U3)
was implemented via subagent-driven development in worktree `.claude/worktrees/local-tracker` on
branch `feat/local-project-tracker` (off `main`), 8 commits, TS 91/91 + Rust 4/4 green, per-task
reviews + an opus whole-branch review (one integration bug found & fixed: healthy local card
`updatedIso` used the declared date → always-stale; now poll time). Pushed to origin, **PR #36 open
(base main)**. Worktree KEPT for PR iteration. SDD ledger: `.superpowers/sdd/progress.md` in the
worktree. CI on #36 will be red due to the GitHub Actions BILLING issue (not code). Phase 2
(dispatch) is a separate future plan. Deferred minors logged in the ledger (e.g. App.svelte
hard-codes scanRoots ['D:/MajorProjects']).

**SPEC + PHASE-1 PLAN COMMITTED (2026-07-06):** on branch `docs/local-project-tracker-spec`
— spec `0ccdec3`, Phase-1 plan `48d57c5` (`docs/superpowers/plans/2026-07-06-local-project-tracker-phase1.md`,
6 TDD tasks: U1 frontmatter parser, U2 cc-item parser, U4 local adapter, scan_local_projects Rust
cmd, store/board wiring, U3 STATUS stamping). Option A chosen. **Awaiting user's execution choice:
subagent-driven vs inline (superpowers:executing-plans / subagent-driven-development), or stop.**
Phase 2 plan deferred until Phase-1 interfaces settle in code. Branch sits on the spike tip → rebase/
cherry-pick onto main before PR. Working tree still has uncommitted P4/spike diag changes + tauri dev
running. Earlier detail:

**(spec history):** full design at
`docs/superpowers/specs/2026-07-06-local-project-tracker-design.md` — passed 3 adversarial critique
rounds (Design Critique Log appended). Self-review clean (no placeholders). **Awaiting user review of
the file**, then: commit on a new branch (`docs/local-project-tracker-spec`; currently on the unrelated
spike branch, uncommitted), then `superpowers:writing-plans`. Two decisions for the user at review:
(1) Phase-2 Option A (write-back + daemon-wide loopback-auth migration) vs Option B (deep-link only,
defers the auth work) — §0/§7.4; (2) branch/commit go-ahead. Key critique-driven changes from the
brainstorm: dropped the synthesized StageOverride (it expired at 72h in real stage.ts, flipping stage)
→ adapter emits fully-resolved card + "declared Nd ago" anti-rot hint; discovered fleetd `/swarms`
ALREADY accepts arbitrary real repos unauthenticated, so responsible dispatch requires router-level
auth + repo allowlist across all mutating endpoints (bigger than "a button").

--- (earlier) U5 PRESENTED + awaiting OK: dispatch button on `local:` card items → `POST /missions{task,tier,REPO?}`;
loopback free via existing Fleet adapter; item↔mission link in dashboard store; fleet card gets
`family=<local projectId>` to cluster. Write-back decision(b) RESOLVED = **write back to ROADMAP.md**
(narrow in-place `status=` header rewrite via `#[tauri::command] set_roadmap_item_status`): dispatch→
active, ship→done, fail→open. Rationale: docs are source of truth; store-only would rot. OPEN INTEGRATION
FLAG: `POST /missions` in api.ts shows only {task,tier} but a mission needs a TARGET REPO — must verify
fleetd mission-create contract accepts a repo/path field before Phase-2 build. Next: verify that fleetd
field, then write the spec to
`docs/superpowers/specs/YYYY-MM-DD-local-project-tracker-design.md`, self-review, user review,
then `superpowers:writing-plans`. (Brainstorming HARD-GATE: no implementation until spec approved.)

## ⚡ ACTIVE DEBUG — all 100 handshakes DROPPING

The first watched run showed **every** round dropping (systematic, not a timing race). In
Phase 1 (root-cause evidence gathering) of `superpowers:systematic-debugging` — instrumenting,
NOT yet fixing. Added diagnostics (uncommitted, all throwaway spike code):
- **New** [diag.js](../../cockpit/ui/src-tauri/src/spike-view-plugin/diag.js) — a CLASSIC
  (no-cors, CSP-authorized) probe that runs even when the module entry sdk.js is blocked; it
  reports `securitypolicyviolation` / capturing `error` / `unhandledrejection` / `doc-alive`
  to the host via `parent.postMessage('*')`. Loaded first in
  [index.html](../../cockpit/ui/src-tauri/src/spike-view-plugin/index.html) `<head>`.
- Scheme handler serves `/diag.js` ([spike_view_plugins.rs](../../cockpit/ui/src-tauri/src/spike_view_plugins.rs)).
- Host harness ([SpikeViewPlugins.svelte](../../cockpit/ui/src/lib/spike/SpikeViewPlugins.svelte))
  now logs each handshake stage (first 3 rounds) + all `diag` messages.

**Leading hypothesis (unconfirmed):** module scripts are ALWAYS fetched in CORS mode — the
entry `<script type=module src=sdk.js>` included. The scheme handler sets no
`Access-Control-Allow-Origin`, doc origin is opaque `null`, so the fetch fails → sdk.js never
runs → no `plugin-hello` → every round times out. The index.html comment claiming the entry is
"not subject to the cross-origin module-graph CORS rule" is WRONG. The prior CSP fix moved the
failure from a CSP-block to this CORS-block.

**Next action:** `tauri dev` is rebuilding (Rust route added). When it relaunches, re-run the
spike and read the log. Diagnosis by log pattern:
- `✕ diag load-ERROR … src=…sdk.js` → CORS block (my hypothesis). Fix: add
  `Access-Control-Allow-Origin: *` to `respond()` in spike_view_plugins.rs (covers BOTH the
  entry sdk.js AND the gate-c-CORS `import('./probe.js')`).
- `✕ diag CSP-BLOCK: script-src …sdk.js` → CSP still wrong (script-src fix incomplete).
- `→ hello received` then drop → plugin loads; port/init/ready leg is broken.
- no diag lines at all → doc never loaded (scheme handler / iframe issue).

Background tasks in flight: `bwilultp3` (`tauri dev`), `bkra826ot` (watcher for rebuild/relaunch).

## State right now

- **`tauri dev` IS RUNNING** (background task `bwilultp3`; vite on :5173, `app.exe`
  launched, fleetd sidecar healthy on :8787). The WebView2 cockpit window is open and
  waiting for the human to drive the P4 spike. Do **not** relaunch — attach to that window.
- **H4 keying false-COLLISION bug: FIXED this session** in
  [plugins/session-state/src/keying.mjs](../../plugins/session-state/src/keying.mjs) —
  separator-normalized compare that heals a `\`-vs-`/` mismatch to git's canonical form
  instead of writing a spurious `COLLISION` marker. New regression test in
  [test/keying.test.mjs](../../plugins/session-state/test/keying.test.mjs). Full
  session-state suite **47/47 pass**. **Not committed** (per user convention).
- **Uncommitted spike findings** in the working tree:
  - [spike_view_plugins.rs](../../cockpit/ui/src-tauri/src/spike_view_plugins.rs) — CSP fix:
    `script-src 'self'` does NOT authorize scripts in an opaque-origin (`sandbox=
    allow-scripts`, no `allow-same-origin`) iframe; must name the concrete WebView2 origin
    `http://ccplugin.localhost` (or a per-response nonce). Design-spec correction.
  - [2026-06-07-app-plugins-design.md](../../docs/superpowers/specs/2026-06-07-app-plugins-design.md)
    — P3 finding: WebView2 `hide()`/`show()` forces a repaint/reload; park inactive webview
    off-screen + warm-pool LRU instead. (P3 spike is effectively done; see below.)

## The task in flight: P4 watched run

The harness is fully built/wired (Rust `ccplugin://` scheme handler in
[lib.rs](../../cockpit/ui/src-tauri/src/lib.rs), embedded plugin assets, Svelte host
harness [SpikeViewPlugins.svelte](../../cockpit/ui/src/lib/spike/SpikeViewPlugins.svelte),
launch button in [App.svelte:326](../../cockpit/ui/src/App.svelte#L326)).

Human steps in the open window: click **⌬ VP SPIKE** → **▶ RUN 100-RELOAD HANDSHAKE**.
Go criteria to record:
- **a · renders** = PASS
- **b · handshake ×100** = `100/100 ok · 0 dropped` (headline; any drop = FAIL, timing race)
- **c · self-code-load (CORS)** = PASS (confirms the `script-src` origin fix)
- **c · network blocked (CSP)** = PASS
- **d · host CSP** = manual/deferred (ignore)

If **c-CORS FAILs**: in-place remedy already documented at
[spike_view_plugins.rs:48-51](../../cockpit/ui/src-tauri/src/spike_view_plugins.rs#L48) —
add `Access-Control-Allow-Origin: *` to `respond()`, rebuild, re-run.

## After the run

1. Record the four gate readings as the P4 go/no-go in the view-plugins design spec.
2. Commit H4 fix + P4/P3 spike findings (branch off; user must OK the commit).
3. If P3 **and** P4 both say GO → dispatch the pre-carved feature swarm (app-plugin
   embedding + view-plugin runtime). See [docs/SWARM-HANDOFF.md](../../docs/SWARM-HANDOFF.md).

## Not mine to fix (user-gated)

- **CI is red = GitHub Actions billing**, not code (private repo, paid minutes). Confirmed
  live on PR #35: `cargo test` fails at ~3s (no runner), `tauri build` skips. Local
  `cargo test --workspace` + `tauri build` pass. Fix: Settings → Billing, then `gh run rerun`.
- Installed session-state plugin is still v0.1.0 (pre-H1/H2/H3/H4); fixes are on repo main
  but not re-released.

## Other context

- **P3 app-plugin webview spike is further along than older session notes claim**: the
  `spike_show` hang was already fixed (commit `a41a573` on `spike/app-plugins-webview-v2` —
  async commands so webview creation runs off the main thread) and produced the doc finding
  above. Effectively done pending its go/no-go writeup.
- Open draft PR **#35** (`feat/view-plugin-bridge-handshake`) — separate feat branch,
  blocked by the same billing.

## Suggested skills

- `superpowers:verification-before-completion` — before claiming P4 GO, confirm the actual
  scoreboard readings (evidence before assertion).
- `session-state:save-state` — checkpoint once the P4 run is recorded (now unblocked by H4).
- `superpowers:dispatching-parallel-agents` / `swarm-handoff` — if P3+P4 both GO, to fan out
  the feature swarm.
- `superpowers:finishing-a-development-branch` — to integrate the H4 fix / decide the spike
  branch's fate.
