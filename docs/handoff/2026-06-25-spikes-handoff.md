# Human-Gated Master Handoff — Spikes P3/P4 + S3 + Certs (2026-06-25)

> **Purpose.** The **single** doc for everything only *you* can do on the road to a shippable,
> feature-complete Command Center — two go/no-go spikes that need your eyes, one live paid run, and the
> code-signing procurement. The engineering backlog is cleared and verified green (see
> [the ship-readiness audit](2026-06-25-ship-readiness-swarm-handoff.md)); each item below is a short
> watched session or an out-of-repo purchase, with copy-paste runbooks and exact pass criteria.
>
> **Both spikes are now committed and ready for a watched run** — no debugging or scaffolding remains.
> **What changed since the [2026-06-24 runbook](2026-06-24-human-gated-spikes-runbook.md)** (now stale on
> status, superseded by this doc): the P3 `spike_show` async fix is **committed** (was "uncommitted in the
> worktree"), and P4's harness is **committed on `spike/view-plugins-handshake`** (its glance table said
> "not yet scaffolded"). State below is verified against the repo this session.

## At a glance

| # | Task | Kind | What it proves / unblocks | State | Your effort |
|---|---|---|---|---|---|
| **CI** | GitHub billing fix | 💳 billing | Honest CI signal on every branch; merges stop needing `--admin` | red across all branches (no runner allocated) | ~5 min |
| **P3** | App-plugin webview spike | 🔴 watched GUI | Child webview renders a whole web app in the cockpit → **Lane A1** (app-plugin embedding) | ✅ fix committed; needs your go/no-go | ~30–45 min |
| **P4** | View-plugin handshake spike | 🟠 watched run (dev + packaged) | Sandboxed-iframe `plugin-hello→init` ×100/0-drop → **Lane A2** (view-plugin runtime) | ✅ harness committed; needs your run | ~1–2 hr |
| **S3** | One live paid T1 mission | 🟠 credential + spend | The autonomous spine on real tokens (oracle→build→review→PR) | needs `ANTHROPIC_API_KEY` + a few $ | ~1 watched hour |
| **Certs** | Code-signing certs | 🟣 procurement | The signed cross-platform release run | wiring done; needs purchase | ~1 wk lead, ~30 min wiring |

**Recommended order:** **CI billing first** (5 min, highest leverage — GitHub → Settings → Billing &
plans, then `gh run rerun <id>`) → **P3** (largest downstream unlock) → **P4** → then **S3** and **Certs**
in parallel whenever (kick Certs off early; it has a ~1-week procurement lead). The spikes are independent
of each other. Downstream feature lanes are carved in
[2026-06-25-ship-readiness-swarm-handoff.md](2026-06-25-ship-readiness-swarm-handoff.md).

---

## P3 — App-plugin webview spike  🔴 (visual go/no-go)

**Where it lives** (a dedicated worktree, **not** `main` and not your current `spike/view-plugins-handshake` checkout):
- Worktree: `.claude/worktrees/agent-a709aaf1bcad07d41`
- Branch: `spike/app-plugins-webview-v2` @ `a41a573` — *"fix spike_show hang — async commands so webview creation runs off the main thread"*
- The fix is **committed** (`spike_webview.rs:63` is now `pub async fn spike_show`). `cargo check` was clean when committed.

**What was wrong + the fix (already done):** the three spike commands (`spike_show/hide/set_rect`) were
synchronous `#[tauri::command]`s; `window.add_child(...)` (Tauri 2.11 `unstable` child-webview API)
**deadlocks on the main thread** when called from a sync command. They are now `async fn`, so creation
runs off the main thread. All that remains is **your eyes on the window.**

### Runbook (cold start)

```bash
# 1) Audience web on :3000 — dev posture, deterministic fake providers.
#    If docker infra (postgres/redis/minio) is down: `docker compose up -d` first.
cd /d/MajorProjects/CURRENT/audience
AI_PROVIDER=fake MEDIA_PROVIDER=fake pnpm --filter @audience/web dev      # -> http://localhost:3000
#    Renderable (200): /compose (the harness points here), /onboarding, /settings.
#    500 without the api service: /dashboard, /queue, /calendar.

# 2) Cockpit harness — the P3 WORKTREE, not the main checkout.
cd "/d/MajorProjects/CURRENT/command-center/.claude/worktrees/agent-a709aaf1bcad07d41/cockpit/ui"
npm run sidecar          # builds the fleetd-serve sidecar (required before tauri)
npm run tauri dev        # ~1.5 min cached; opens the GUI; fleetd on :8787
```

In the GUI click **"SPIKE: show webview"** and watch the `tauri dev` **stderr**.

### Pass criteria (record each)

| Gate | Pass condition |
|---|---|
| **Hang gone** | stderr prints `[SPIKE] add_child RETURNED ok` (it never did pre-fix). Confirms the async fix held. |
| **Gate 2 — renders** | Audience `/compose` paints over the purple placeholder rect. |
| **Gate 3 — real origin** | It's the real `localhost:3000` page (forms/scroll interactive), not a stub. (Lighter here — Audience has no `/login`.) |
| **Gate 4 — resize ≤150ms** | Drag-resize the host window; the child rect tracks the Svelte `<div>` within ~150ms (the `ResizeObserver` streams `spike_set_rect`). |
| **Hide-on-overlay** | "SPIKE: hide" hides it cleanly — no flash, no orphan paint. |
| **Gate 5 — no orphans** | Quit the app, then `docker ps -a`. **Baseline (pre-existing, NOT spike leakage):** the `audience-*` **Exited** containers from a prior full-stack run. You're checking the spike left nothing *new*. |

**If `add_child RETURNED ok` prints but nothing renders** → the secondary **z-order** hypothesis (child
webview behind the opaque main webview). Don't redesign; flag it — the remedy is `set_focus`/raise or a
transparent slot region, a small follow-up.

### Record + decide
Write go/no-go **plus the exact webview API that worked** (the `add_child`/`set_position`/`set_size`/
`hide`/`show`/`set_focus` calls) to **`spikes/SPIKE-RESULTS-app-plugins.md`**. A **GO** unblocks **Lane A1**
(app-plugin embedding, ~14–23h, pre-carved in the swarm handoff). A NO-GO on Gate 4 → the spec's fallback
is a separate `WebviewWindow` per app (changes only the embedding surface; everything upstream is reusable).

> Two P3 worktrees exist — use **`agent-a709aaf1bcad07d41`** (v2, has the fix). The older
> `.claude/worktrees/spike+app-plugins-webview` (v1, `2d5c125`) predates the fix; ignore it.

---

## P4 — View-plugin handshake spike  🟠 (watched run, dev + packaged)

**Where it lives:** your **current branch** `spike/view-plugins-handshake` @ `9fbce0a` (the main checkout —
no worktree switch needed). `cargo check` clean · `npm run check` 0/0 · `npm test` 72/72 (verified this session).
The `ccplugin://` scheme handler + embedded plugin + 100-reload host driver are wired; the ops grid is
untouched (a `⌬ VP SPIKE` button bottom-right opens the harness).

**Why only you:** the handshake timing race (gate b) only surfaces in a live run, and "packaged" means a
real `tauri build` artifact — both must pass.

### Runbook

```bash
cd /d/MajorProjects/CURRENT/command-center/cockpit/ui
# DEV — click "⌬ VP SPIKE" (bottom-right) → "▶ RUN 100-RELOAD HANDSHAKE", watch the scoreboard.
npm run desktop
# PACKAGED — same buttons, in an installed bundle (must ALSO pass).
npm run bundle          # then install the MSI/NSIS from src-tauri/target/release/bundle/
```

The harness shows a live gate scoreboard (a / b / c-CORS / c-CSP) and a log. The fill-in results table
already lives in **[`spikes/SPIKE-RESULTS.md`](../../spikes/SPIKE-RESULTS.md)** ("P4 — View-plugin handshake spike").

### Gates (record each — dev AND packaged)

| Gate | Pass condition |
|---|---|
| **(a) renders** | The sandboxed iframe served from `ccplugin://` paints its marker; origin shows `null`. |
| **(b) handshake ×100** | `plugin-hello→init` round-trip succeeds across **100 reloads, 0 dropped** — the headline number; catches the timing race. |
| **(c-CORS) self-code-load** | Opaque-origin `import()` of a 2nd file works **OR** the single inlined bundle (fallback #1) runs. |
| **(c-CSP) network blocked** | The plugin-doc CSP (`default-src 'none'; script-src 'self'; connect-src 'none'; …`) permits self-scripts and **blocks network** (fetch rejected). |
| **(d) host CSP / HMR** | *Manual* (left out of the harness): set `app.security.csp` in `tauri.conf.json` to the candidate in `SPIKE-RESULTS.md`, relaunch `npm run desktop`, confirm the iframe still loads **and** Vite HMR still works (edit a `.svelte` file → hot update, no full reload, no CSP error). Dev-only. |

**Pre-committed fallbacks (use, don't redesign):** **#1** single inlined bundle if opaque-origin module
load fails — the harness already supports it; **#2** loopback static server on a random port serving only
plugin assets (iframe still sandboxed) — ⚠️ it **must emit the same plugin-doc CSP header** (`connect-src
'none'`) or the no-network trust boundary dissolves.

### Record + decide
Write to **`spikes/SPIKE-RESULTS.md`** (the section + table are already there). A **GO** (all gates PASS,
0/100 drops dev **and** packaged) unblocks **Lane A2** (view-plugin runtime) — note the exact scheme + CSP
used. **c-CSP FAIL** (fetch succeeded) is a hard NO-GO: the "no network" guarantee is the trust boundary.
Any handshake drop is a real finding worth a NO-GO until understood (the announce-first design targets 0).

---

### After both spikes
**Both GO** → dispatch the feature swarm in [2026-06-25-ship-readiness-swarm-handoff.md](2026-06-25-ship-readiness-swarm-handoff.md):
Lane A1 (on P3 GO) ∥ Lane A2 (on P4 GO; branch from `spike/view-plugins-handshake`, do the
`feat/view-plugins` de-stale), then Lane S integrates and deletes the P4 spike scaffold.

---

## S3 — One live paid T1 mission  🟠 (real credential + real spend)

**What it proves:** the end-to-end autonomous spine on **real tokens** — oracle → build → review → PR —
on a throwaway repo. The last unproven slice of SP1; everything else runs green on synthetic fixtures.
**This is a validation run, not a build gate.**

**Why only you:** needs a real `ANTHROPIC_API_KEY`, real token spend (a few dollars), and a live human
watching the run.

### Runbook (sketch — confirm exact dispatch flags against `fleetd --help`)

```bash
# 1) Provide the key (PowerShell):  $env:ANTHROPIC_API_KEY = "sk-ant-…"
# 2) Point at a THROWAWAY repo (so a real PR is harmless).
# 3) Dispatch a T1 mission with the dollar ceiling on. The cost cap is enforceable —
#    --max-budget-usd is a hard ceiling that holds even if fleetd dies (SPIKE-RESULTS.md §2).
#    Watch: oracle test-set → build loop → review gate → PR creation, with the live $ counter.
```

**Watch for:** the `total_cost_usd` accounting matches the cap; the review gate actually gates; a real PR
lands on the throwaway repo. **Record** the run + final cost in the next handoff (no mandated file — it's
confidence, not a gate).

> `--max-turns` is **not** a flag in this Claude Code version — the budget control is `--max-budget-usd`
> plus a wall-clock `timeout` watchdog.

---

## Certs — Code-signing certificates  🟣 (procurement, out of repo)

**What it unblocks:** the **signed cross-platform release run**. CI wiring + the exact secret names are
**already done** (`release.yml` consumes them by name) — nothing in the repo blocks. Full reference:
[`docs/release/signing-and-updates.md`](../release/signing-and-updates.md) §4.

**Why only you:** buying certs requires money + identity verification (Apple Developer Program, a CA) —
~1 week procurement lead. Wiring them after is ~30 min. Local unsigned `npm run tauri build` keeps working
with no certs present (all identities are `null` placeholders), so none of this blocks development.

### Shopping list

| Platform | Obtain | Cost / lead |
|---|---|---|
| **macOS** | Apple Developer Program membership → **Developer ID Application** cert (export as password-protected `.p12`) + Team ID + an app-specific password for notarization. | $99/yr; instant–days |
| **Windows** | An Authenticode **code-signing cert** from a CA (DigiCert/Sectigo/…) as a password-protected `.pfx`. EV gives best SmartScreen reputation; OV also works. | varies; **days–1 wk** (identity vetting) |
| **Updater keypair** | Generate locally — no purchase: `cd cockpit/ui && npm run tauri signer generate -- -w ~/.tauri/cc-updater.key`. Put the printed **public** key in `tauri.conf.json → plugins.updater.pubkey`; keep the private half secret. | free; minutes |

### Then set these CI secrets (canonical names — `${{ secrets.<NAME> }}`)

- **macOS:** `APPLE_CERTIFICATE` (base64 `.p12`), `APPLE_CERTIFICATE_PASSWORD`, `APPLE_ID`,
  `APPLE_PASSWORD` (app-specific), `APPLE_TEAM_ID`.
- **Windows:** `WINDOWS_CERTIFICATE` (base64 `.pfx`), `WINDOWS_CERTIFICATE_PASSWORD`.
- **Updater (all platforms):** `TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.

`base64 -i cert.p12` / `base64 -i cert.pfx` produces the base64 values. The matching **public** updater
key is not a secret — it lives in `tauri.conf.json`.

---

## Quick verify reference
- P3 worktree compiles: `cargo check` in `.claude/worktrees/agent-a709aaf1bcad07d41/cockpit/ui/src-tauri`.
- P4 / main gates: `cargo test --workspace` (green / 3 ignored Docker ITs) · `cd cockpit/ui && npm run check && npm test` (0/0 · 72/72).
- Cockpit dev: `cd cockpit/ui && npm run desktop` · packaged: `npm run bundle`.
