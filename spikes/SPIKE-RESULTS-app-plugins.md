# SPIKE #1 — App-plugin child-webview embedding

Branch: `spike/app-plugins-webview-v2` (supersedes stale v1 `spike/app-plugins-webview`).
Tauri `=2.11.2` + `unstable`. Harness: `spike_webview.rs` (3 async commands) + `App.svelte`
(AUDIENCE tab in the top switcher hosts the child webview).
Decision: **LEANING GO (overlay "B")** on dev evidence — **final pending packaged build + Gate 5.**

> Phase 0 throwaway go/no-go for embedding whole web apps as child webviews. Phase 1+ must not
> start until this records an explicit go/no-go. Spec gates: `app-plugins-design.md` §4/§6.

## Gate results

- [x] **1. `unstable` feature builds — PASS (dev).** `cargo build` exit 0 (sidecar must be built
  first — see v1 gotcha). Packaged (`tauri build`) still pending.
- [x] **Hang fixed — PASS.** The original `spike_show` deadlock (sync `#[tauri::command]` blocking
  the main event-loop thread on `add_child`) is fixed by making the three commands `async fn`
  (commit `a41a573`). Trace confirms `[SPIKE] add_child RETURNED ok` + `set_focus done` (it never
  printed pre-fix). Watched run 2026-06-26.
- [x] **2. Renders (dev) — PASS.** A Rust-positioned child webview rendered the whole Audience web
  app (`http://localhost:3000/compose`) full-area under a Svelte rect, in the AUDIENCE tab.
  ⚠️ **Audience is a poor proving app:** with only the web dev server + `fake` providers up (no
  `api` backend), its frontend throws `TypeError: Failed to fetch` (lib/api-client.ts) and can
  degrade to a blank render. That is Audience's missing backend, **not** an embedding failure
  (the webview faithfully executes real JS + network). For a clean re-confirm, point the harness
  at a stable page (a healthy app, or bring up the Audience `api` service).
- [x] **4a. Resize tracking (dev) — PASS.** On host-window resize the child webview tracks the
  reserved rect correctly (ResizeObserver + window-resize → `spike_set_rect`, rAF-coalesced).
- [~] **4b. Hide-on-switch — WORKS but inefficient.** Switching tabs hides/reveals the webview
  cleanly (no orphan paint), BUT `Webview::hide()`→`show()` forces a **repaint/reload** each
  switch (loses scroll/form/session state, re-runs app startup). The spec's §4 "hide, not destroy"
  keep-alive is therefore **not free**. → Roadmap: park off-screen instead of `hide()` + warm-pool
  LRU cap. Recorded in `app-plugins-design.md` §5 ("Webview keep-warm without reload").
- [~] **3. Real-origin behaviors — PARTIAL.** Confirmed the webview is a true origin (executes
  real JS, makes real network calls). Cookies-persist / `window.open` popup / full-page redirect
  NOT exercised this run because Audience's backend was down — revisit with a healthy app.
- [ ] **2/4 PACKAGED — pending.** Gates 2 & 4 must also hold in a real `npm run bundle` artifact,
  not just `tauri dev`.
- [ ] **5. Lifecycle / no orphans — pending.** Launch → health → ready → show → quit → `docker ps`
  clean. This exercises the app-plugin **backend** lifecycle, which the webview harness does not
  drive; needs a separate launch path (see the 2026-06-15 resume doc's Gate-5 plan).

## Exact webview API that worked (Phase 6 copies verbatim)

Tauri 2.11.2 `unstable`, called from Rust commands (not gated by the JS webview capability allowlist):
- `tauri::webview::WebviewBuilder::new(label, WebviewUrl::External(url))`
- `Window::add_child(builder, LogicalPosition, LogicalSize) -> Webview`  ← the `unstable` call
- `Webview::set_position(LogicalPosition)` · `Webview::set_size(LogicalSize)`
- `Webview::set_focus()` · `Webview::hide()` / `Webview::show()` · `Webview::close()`
- `Manager::get_webview(label) -> Option<Webview>` (reuse-or-create)

## Harness gotchas (carry into the embedding build)

- **`.taurignore` required.** `fleetd` writes `fleet.db` + WAL sidecars (`-shm`/`-wal`) into
  `src-tauri/`; the `tauri dev` file-watcher rebuilds on every WAL write → endless "Rebuilding
  application" loop (window opens-and-dies). Fix: `src-tauri/.taurignore` listing `fleet.db*`.
  (The repo `.gitignore` entries are root-anchored `/fleet.db` and do NOT cover `src-tauri/`.)
- **Teardown:** kill the `tauri dev` watcher ROOT (not `app.exe` — the watcher respawns it). Also
  kill the standalone Vite (`:5173`) and the detached `fleetd-serve` sidecar (`:8787`); both can
  survive an `app.exe` kill and a stale Vite forces the next run onto `:5174` (wrong `devUrl`).

## Decision + rationale

Dev-side evidence supports **overlay "B"** (renders full web app under a Svelte rect, resize
tracks, deadlock fixed). Two gates remain before a final GO: **packaged build** (2 & 4) and
**Gate 5** (lifecycle orphans). Known refinement before relying on cheap tab-switching:
off-screen-park keep-warm (spec §5), not `hide()`.
