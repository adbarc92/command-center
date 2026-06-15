# Session Pickup — 2026-06-15 — P3 webview spike (mid-debug)

**Main repo branch:** `main` @ `b36ec90` (synced to `origin/main`; local commits below are **not pushed**).
**Spike work lives in a worktree, not main:**
`.claude/worktrees/agent-a709aaf1bcad07d41` on branch `spike/app-plugins-webview-v2` @ `59470d7`.
**Guide (authoritative for the gates):** [`docs/handoff/2026-06-11-P3-app-plugin-webview-spike-guide.md`](2026-06-11-P3-app-plugin-webview-spike-guide.md)
**Spec the spike gates:** [`docs/superpowers/specs/2026-06-07-app-plugins-design.md`](../superpowers/specs/2026-06-07-app-plugins-design.md) §6

## Where we are

Doing **P3** — the human-gated app-plugin webview go/no-go (overlay "B" vs windows "C"). Full environment is
stood up; **blocked on a harness bug** (the `Show` button hangs — root cause hypothesised, instrumentation
committed, one observation away from confirming). Gates 2–4 not yet observed; Gate 5 not yet run.

| Step | Status |
|---|---|
| Final audit of the product (pre-P3) | ✅ done — see below |
| Audience `:3000` runnable | ✅ (pnpm store was corrupt; hard-repaired) |
| Cockpit harness built + running | ✅ (then stopped for this handoff) |
| **Bug: `spike_show` hangs** | 🔬 instrumented, **confirm + fix next** |
| Gate 2 (renders) / 3 (real-origin) / 4 (positioning+hide) | ⬜ blocked on the bug |
| Gate 5 (no orphans, **live** — user's choice) | ⬜ not started |
| Record decision → `spikes/SPIKE-RESULTS-app-plugins.md` | ⬜ |

## THE ACTIVE BUG — read this first

**Symptom:** click **"SPIKE: show webview"** → status word stays `hidden`, **no** `err:` shown, webview never
appears (only the purple placeholder). So `await invoke('spike_show')` **neither resolves nor rejects — it hangs.**

**Hypothesis (high confidence):** `spike_show` is a **synchronous** `#[tauri::command] pub fn` and
`window.add_child(...)` (Tauri 2.11 `unstable` child-webview API) **deadlocks when called from a sync command**
(webview creation needs the main thread; a blocking sync command starves it). Classic Tauri-v2 gotcha.

**Already done (committed `59470d7` in the worktree):** `eprintln!` tracing in `spike_webview.rs::spike_show`
at `ENTRY`, before `add_child` (`calling add_child`), after (`add_child RETURNED ok`), and `set_focus done`.

**Next action (one observation):** relaunch `tauri dev`, click `Show`, grep the tauri-dev stderr for `[SPIKE]`:
- `ENTRY` + `calling add_child` print, but **`add_child RETURNED ok` never does** → deadlock confirmed.

**Then fix:** make `spike_show` (and for symmetry `spike_hide`/`spike_set_rect`) **`async fn`** so it runs off
the main thread — OR wrap the `add_child`/`set_position`/`set_size` calls in `app.run_on_main_thread(move || …)`.
Re-test Show; if the webview now appears over the placeholder, proceed to gates 2 → 4.
(Secondary hypothesis if async doesn't fix it: z-order — child webview rendering *behind* the opaque main
webview. Then try `set_focus`/raise, or a transparent slot region.)

## Exact resume runbook (cold start)

All paths absolute. Docker infra (postgres/redis/minio) was **left running** — verify with `docker ps`.

1. **Audience web on :3000** (host dev server):
   ```bash
   cd /d/MajorProjects/CURRENT/audience
   AI_PROVIDER=fake MEDIA_PROVIDER=fake pnpm --filter @audience/web dev    # -> http://localhost:3000
   ```
   - If infra is down: `docker compose up -d` first (postgres :55432, redis, minio).
   - **Renderable routes (200):** `/compose` (harness points here), `/onboarding`, `/settings`.
     **500 routes (need the `api` service):** `/dashboard`, `/queue`, `/calendar`; `/` 307→`/dashboard`→500.
2. **Cockpit harness** (the worktree — NOT the main checkout):
   ```bash
   cd "/d/MajorProjects/CURRENT/command-center/.claude/worktrees/agent-a709aaf1bcad07d41/cockpit/ui"
   npm run sidecar        # builds fleetd-serve sidecar (required before tauri build)
   npm run tauri dev      # compiles (~1.5 min cached), opens the GUI, fleetd on :8787
   ```
   - Watch the `tauri dev` stderr for `[SPIKE]` lines after clicking Show.
   - `npm install` already done in this worktree; sidecar binary already built.

## Gate 5 plan (user chose "re-exercise live")

- Launch path: Tauri command **`plugin_launch(id)`** (`cockpit/ui/src-tauri/src/plugins/manager.rs:119`),
  reads manifest `~/.command-center/app-plugins/<id>/app-plugin.json` (schema in `manifest.rs`; Audience
  example template at `manifest.rs:118`). Teardown = `lifecycle.stop` via `stop_all_owned` on app quit
  (`lib.rs:57`). Orphan check after quit: `docker ps -a`.
- **Gotcha:** the throwaway spike `App.svelte` has **no plugin-launch UI** — Gate-5-live needs a temporary
  launch button (or another way to invoke `plugin_launch`). A *minimal* manifest (tiny container with a
  health endpoint) exercises the real lifecycle without Audience's heavy prod build.
- **Baseline orphans (NOT spike leakage):** `audience-web-1`, `audience-api-1`, `audience-orchestrator-1`,
  `audience-video-1`, `audience-publishing-1`, `audience-notifications-1`, `audience-ai_content-1` — all
  **Exited** from a prior full-stack run (images prebuilt, in `docker-compose.prod.yml`). Record these as
  pre-existing before judging Gate 5.

## Audit findings (pre-P3, this session)

- **Was on a stale branch 8 commits behind `origin/main`.** Fast-forwarded local `main` → `b36ec90`. The
  "missing docs" were just un-pulled (handoff set + complete budget-checkpoint tool). **Audit against `main`.**
- **Build green on `main`:** `cargo test --workspace` = **104 passed / 0 failed / 3 ignored** (Docker/network/
  live-PR integration); `cargo clippy --workspace --all-targets` clean; cockpit `vitest` = **72 passed** (10 files).
- **Item 4 dashboard is further along than docs claim** — real impl exists (`cockpit/ui/src/lib/dashboard/`
  + `src/views/Dashboard.test.ts`), not just the design-only spec.
- **Remaining work is all human-gated:** P3 (this), P4 (view-plugin handshake spike, unrun), S3 (one paid T1
  mission), Certs (procurement; `release.yml` already wires the secrets). No agent-doable backlog remains.

## Assumptions / deviations taken (revisit if wrong)

- Used **`/compose`** as the webview target instead of the literal root, because `/` 500s without the `api`
  service. Gates 2 & 4 are origin-agnostic so this is faithful; **Gate 3** is lighter (Audience has no `/login`).
  To make the root work, bring up the full stack (`docker compose -f docker-compose.prod.yml up -d`) and wire
  the web app's API base — deferred as an Audience-ops detour.
- Left **docker infra up** (detached, persists) to speed resume; stopped only the session-bound dev servers
  (cockpit `app.exe`, next dev `:3000`).
- `AI_PROVIDER=fake MEDIA_PROVIDER=fake` for deterministic no-network providers (per Audience CLAUDE.md).

## Servers / state worth remembering

- Audience pnpm store (`D:\.pnpm-store\v11`) had a **corrupt empty `next` package**; fixed by `rm -rf` all
  Audience `node_modules` + clean `pnpm install` (~5m25s). If `next/dist/bin/next` is missing again, repeat.
- Tauri `dev` **auto-recompiles on `src-tauri/*.rs` edits** (watcher) — no manual restart needed for Rust changes.
