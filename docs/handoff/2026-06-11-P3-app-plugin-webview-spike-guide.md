# P3 — App-plugin webview embedding spike (visual go/no-go)

> ℹ️ **STATUS (2026-06-25)** — to *run* P3, use the current
> **[`2026-06-25-spikes-handoff.md`](2026-06-25-spikes-handoff.md)** (the fix is committed; runbook is current).
> This doc is retained as the **reference for the gate definitions + the GO=overlay-B / NO-GO=windows-C decision.**

**You run this** — it needs visual judgment a tool can't give. Goal: decide **how** the cockpit hosts
a whole web app (proving app: **Audience**) as a child webview, before any embedding code is written.

- **Decision to produce:** **GO = overlay approach "B"** (Rust-positioned child webview under a Svelte
  rect) · **NO-GO = fallback "C"** (separate OS windows).
- **Why it gates everything:** App-plugin embedding (Phases 4–6, `app-plugins-design.md` §6) must **not**
  start until this records an explicit go/no-go. The backend lifecycle is already done & merged; this is
  the one missing proof.
- **Record results to:** `spikes/SPIKE-RESULTS-app-plugins.md` (Gate 1 = PASS already; fill gates 2–5,
  the "exact webview API that worked", and the decision).

---

## Prerequisites (have these running first)

1. **Audience up** at `http://localhost:3000` — `D:/MajorProjects/CURRENT/audience`, dev posture.
   Confirm it loads in a normal browser tab before embedding it.
2. **Docker** running (lifecycle gate 5 checks for orphaned containers).
3. **Sidecar built before any Rust build** — `node cockpit/ui/scripts/build-sidecar.mjs`.
   ⚠️ A bare `cargo build`/`tauri build` fails without it (`externalBin … fleetd-serve … doesn't exist`).
   This bit Gate 1; do it first every time.

## Step 0 — stand up the throwaway harness (the part that isn't built yet)

The existing `spike/app-plugins-webview` branch is **stale** (it only flips on the Tauri `unstable`
feature and predates the merged plugin backend). Recommended: **branch fresh off `main`**, then add a
*throwaway* harness — this is the spike, keep it ugly:

1. Enable unstable in `cockpit/ui/src-tauri/Cargo.toml`:
   `tauri = { version = "=2.11.2", features = ["unstable"] }`.
2. Add two throwaway Tauri commands using the 2.11 **`unstable` child-webview API**
   (`WebviewBuilder` + `Window::add_child`, then `Webview::set_position` / `set_size`):
   - `spike_show(rect)` — create/position a child webview at a screen rect, `url = http://localhost:3000`.
   - `spike_hide()` / `spike_set_rect(rect)` — hide and reposition it.
3. In `App.svelte`, drop a placeholder `<div>` with a `ResizeObserver` that reports its rect to
   `spike_set_rect`, plus buttons to show/hide and to pop the existing approval overlay.
4. `npm run sidecar && npm run tauri dev` to launch. (Repeat with `npm run tauri build` for the
   **packaged** check — gates 2 & 4 must hold in *both*.)

> An agent can scaffold this harness for you; the **gates below are yours** — they're visual/timed.

## The gates — observe each, mark PASS/NO-GO

| # | What to do | PASS criteria |
|---|---|---|
| **2 · Renders** | Show the child webview over the placeholder, **dev and packaged**. | Audience renders fully inside the rect; interactive (scroll, click, type). No blank/white webview in either build. |
| **3 · Real-origin behaviors** | Exercise Audience as a real site: log in / set a cookie & reload; trigger a `window.open` popup; follow a full-page redirect. | Cookies persist across reload; popup opens; redirect navigates in-webview. (If any fail, note which — informs the isolation/auth story, not necessarily a NO-GO.) |
| **4 · Positioning & hide** | Resize the host window ~10× and toggle the approval overlay open/close ~10×. Watch the seam. | Webview tracks the Svelte rect within **≤150ms / ≤10 frames** on resize; **hides ≤150ms with no stale-content flash** when the overlay opens; scroll + focus preserved on re-show. ≥10 trials, no failures. |
| **5 · Lifecycle, no orphans** | Full round-trip: launch plugin → health → ready → show; then **quit the app**. | After quit, `docker ps` shows **no orphaned** plugin containers (blocking teardown ran). |

**Decision rule:** all of 2, 4, 5 PASS → **GO (overlay "B")**. A hard failure in 2/4/5 (won't render in
packaged, can't hit the resize/hide budget, or orphans survive quit) → **NO-GO**, fall back to separate
windows "C" and record why. Gate 3 quirks are recorded as follow-ups, not automatic blockers.

## What to write down (so Phase 6 can copy it)

In `spikes/SPIKE-RESULTS-app-plugins.md`:
- Tick gates 2–5 with one line of evidence each (incl. the measured resize/hide latency).
- **The exact `unstable` calls that worked** — the precise `WebviewBuilder` / `add_child` /
  `set_position` / `set_size` signatures. Phase 6 copies these verbatim.
- The **decision + rationale** (GO-B or NO-GO-C).

**On GO:** the App-plugin embedding swarm (`app-plugins-design.md` §6) is dispatch-ready, and Lane A wires
`onPluginState` into the dashboard. **On NO-GO:** the embedding design switches to the windows fallback.

---

> Sibling spike (separate, **not** this doc): **P4** — the *view-plugin* iframe + MessagePort handshake
> (100 reloads, zero drops, dev + packaged) → `spikes/SPIKE-RESULTS.md`. Different runtime, different proof.
