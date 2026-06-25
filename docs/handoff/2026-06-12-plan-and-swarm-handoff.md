# Plan + Swarm Handoff — tomorrow's command-center work

> ⚠️ **SUPERSEDED (2026-06-25)** — first by `2026-06-12-remaining-work-handoff.md`, now by the current set.
> Current human-gated work: **[`2026-06-25-spikes-handoff.md`](2026-06-25-spikes-handoff.md)**; ship plan:
> [`2026-06-25-ship-readiness-swarm-handoff.md`](2026-06-25-ship-readiness-swarm-handoff.md). Kept for history.

**For:** the next session (you + an agent swarm). **From:** 2026-06-11.
**One-line state:** every agent-buildable feature is shipped; the two unbuilt features
(app-plugin embedding, view-plugin runtime) are blocked behind **human-only visual spikes**. So
tomorrow is a **serial human-gated track** (the spikes + a paid mission + certs) running alongside a
**small autonomous swarm** of genuinely-independent tidies, with the big feature swarms held until a
spike says "go."

**Repo:** `d:/MajorProjects/CURRENT/command-center`. `main` is the source of truth.
**Canonical status:** [`docs/ROADMAP.md`](../ROADMAP.md) (flags reconciled 2026-06-11).

---

## The plan — tomorrow's critical path

The gating insight: **nothing big can be swarmed until you clear a spike.** So the day has two
independent tracks that run in parallel:

```
YOU (serial, human-only)                 AGENTS (parallel, autonomous)
─────────────────────────                ─────────────────────────────
Lane A scaffolds P3 harness  ───────────▶ Lane A: P3 webview harness  ┐
        │                                 Lane B: 6D checkpoint hook   ├─ swarm now
        ▼                                 Lane C: globals owner        ┘  (zero overlap)
P3 spike (gates 2–5) ──GO──▶ unblocks ▶ App-plugin embedding swarm (§6)
P4 spike (100 reloads) ─GO──▶ unblocks ▶ View-plugin runtime swarm (fresh off main)
S3 paid T1 mission (proves the spine on real $)
Certs (procurement) ───────▶ unblocks ▶ signed cross-platform release
```

**Recommended order for you:** kick off the **autonomous swarm first** (it runs unattended), then
walk **P3** (Lane A's harness makes it possible), then **P4**, then **S3**, then start cert
procurement (out-of-repo, slow). Each spike that returns GO immediately unblocks a feature swarm you
can dispatch from its design doc.

---

## Part A — Human-gated serial track (you, not an agent)

Each needs visual judgment, a real credential + spend, or out-of-repo procurement. Detail lives in
[`docs/ROADMAP.md` §"Requires your attention"](../ROADMAP.md).

1. **P3 — app-plugin webview spike (gates 2–5).** Full guide:
   [`docs/handoff/2026-06-11-P3-app-plugin-webview-spike-guide.md`](2026-06-11-P3-app-plugin-webview-spike-guide.md).
   Needs **Lane A** to have scaffolded the throwaway harness first. Record go/no-go + the exact
   `unstable` webview API to `spikes/SPIKE-RESULTS-app-plugins.md`. **GO → app-plugin embedding swarm.**
2. **P4 — view-plugin handshake spike.** Sandboxed-iframe + MessagePort `plugin-hello → init` across
   **100 reloads, zero drops, dev AND packaged** → `spikes/SPIKE-RESULTS.md`. **GO → view-plugin
   runtime swarm.**
3. **S3 — one live paid T1 mission.** Set `ANTHROPIC_API_KEY`; dispatch a real T1 mission
   oracle→build→review→PR on a throwaway repo, human-watched. Last unproven slice of the spine.
4. **Certs — code-signing.** Apple Developer ID ($99/yr + notarization) + Windows Authenticode.
   Wiring + secret names done (`docs/release/signing-and-updates.md` §4). **→ signed release run.**

---

## Part B — Autonomous swarm (run now, unattended)

Three lanes with **zero write-overlap**. Dispatch each from its brief verbatim + the Rules of the
Road below. **This handoff is the deliverable — do not auto-dispatch; the user opts in.**

### Lane A — P3 webview spike harness   ·   ready
- **Scope:** Scaffold a *throwaway* child-webview harness so the human can walk P3 gates 2–5. Branch
  **fresh off `main`** (the existing `spike/app-plugins-webview` branch is stale — predates the
  merged plugin backend; do not reuse it).
- **Owns (exclusive write):** a new throwaway branch `spike/app-plugins-webview-v2`, containing:
  `cockpit/ui/src-tauri/Cargo.toml` (enable `tauri … features = ["unstable"]`),
  `cockpit/ui/src-tauri/src/spike_webview.rs` (new — `spike_show(rect)`/`spike_hide()`/`spike_set_rect(rect)`
  via the 2.11 `unstable` child-webview API), `cockpit/ui/src-tauri/src/lib.rs` (register the 3 commands),
  `cockpit/ui/src/App.svelte` (placeholder `<div>` + `ResizeObserver` reporting its rect, show/hide buttons).
- **Reads (no write):** the P3 spike guide, `docs/superpowers/specs/2026-06-07-app-plugins-design.md` §6,
  `spikes/SPIKE-RESULTS-app-plugins.md`.
- **Shared contract:** none — throwaway branch, never merges to `main`, so it owns its cockpit files
  outright with no collision risk.
- **Depends on / blocks:** **blocks the human's P3 spike** (they run gates 2–5 against this build).
- **Done when:** `cd cockpit/ui && npm run sidecar` then `cargo build --manifest-path src-tauri/Cargo.toml`
  exits 0, and `npm run tauri dev` launches a window where "show" renders Audience (`:3000`) inside the
  placeholder rect and "hide"/resize reposition it. (Agent proves it *builds + renders*; the timed/visual
  gates are the human's.)
- **Verify:** `npm run sidecar && cargo build --manifest-path src-tauri/Cargo.toml; echo "EXIT=$?"`
  → 0; then a manual `npm run tauri dev` smoke.
- **Notes / open questions:** Build the **sidecar first** every time (a bare `cargo build` fails on the
  `externalBin` resource — see the spike guide). Keep it ugly; it's throwaway. Record the exact
  `WebviewBuilder`/`add_child`/`set_position`/`set_size` calls that worked into SPIKE-RESULTS for Phase 6.

### Lane B — 6D budget-checkpoint Stop hook   ·   ready (optional feature — confirm wanted)
- **Scope:** A `Stop` hook that nudges the agent to run `/handoff` or `/end-session` at phase/spike
  boundaries so the next session starts compact (Roadmap item 6D).
- **Owns (exclusive write):** `tools/budget-checkpoint/` (new dir — hook script, README, tests). Match
  the existing tool conventions: **Python via UV**, mirroring `tools/cache-countdown/`.
- **Reads (no write):** `docs/playbooks/budget-discipline.md` (6D spec), `tools/cache-countdown/`
  (hook + UV layout to copy), `tools/context-offload/` (hook pattern).
- **Shared contract:** `~/.claude/settings.json` → **owned by Lane C** → request: "add a `Stop` hook
  entry invoking `tools/budget-checkpoint/<script>`." Lane B does **not** edit settings.json itself.
- **Depends on / blocks:** Lane C applies its settings entry at integration.
- **Done when:** the script runs standalone against a sample Stop-event payload and emits the nudge per
  its rule; its tests pass.
- **Verify:** `cd tools/budget-checkpoint && uv run pytest; echo "EXIT=$?"` → 0.
- **Notes / open questions:** 6D is marked **optional** in the roadmap — confirm the user wants it before
  building. **Open question:** how to detect a "phase/spike boundary" from the Stop hook's context
  (token count threshold? elapsed turns? explicit marker?). Pick a simple, documented heuristic; don't
  over-engineer.

### Lane C — Globals owner: `~/.claude/settings.json`   ·   ready (integrates LAST)
- **Scope:** Single owner of the global settings file. Deploy the **context-offload Tier-1**
  `SessionStart` recall hook (the `tools/context-offload/` code exists but is **not** wired into
  settings) and apply **Lane B's** `Stop` checkpoint entry.
- **Owns (exclusive write):** `~/.claude/settings.json` — **out-of-repo, NOT worktree-isolated.** Single
  ownership is the only protection; no other lane may write this file.
- **Reads (no write):** `tools/context-offload/recall.py`, `tools/lane-z-integration/deploy_globals.py`
  (the deployer that already knows the entries + backs up first), Lane B's contract request.
- **Shared contract:** it **is** the owner — collects (a) the context-offload `SessionStart` entry and
  (b) Lane B's `Stop` checkpoint entry, applies both in one write.
- **Depends on / blocks:** integrate **after** Lane B (needs its final script path).
- **Done when:** `~/.claude/settings.json` parses as valid JSON, contains the `SessionStart` recall hook
  + the `Stop` checkpoint hook, **preserves** the existing cache-timer + `CLAUDE_CODE_MAX_RETRIES`
  entries, and a fresh `claude` session launches with recalled memory injected and no duplicate hooks.
- **Verify:** back up first; then `python -c "import json;json.load(open(r'C:/Users/barclay/.claude/settings.json'))"`
  → no error; start a new session and confirm SessionStart recall fires.
- **Notes / open questions:** Prefer running `deploy_globals.py` (idempotent, backs up) over hand-editing.
  Run this lane **solo** against settings.json — never concurrently with anything else that writes it.

### Integration order + rules

**Order:** Lane A is standalone (throwaway branch, never merges). Lane B merges to `main` (new `tools/`
dir, no overlap). Lane C runs **last**, applying Lane B's settings request to the global file.
**Reconcile:** after B+C, run `npm run check && npm run test` in `cockpit/ui` (should stay green — these
lanes don't touch app source) and confirm `settings.json` holds the exact intended union.

**Rules of the road (put in every dispatched agent's instructions):**
1. Stay in your lane — write only files your lane owns; need a change elsewhere → file a contract request.
2. One branch/worktree per lane; never commit to `main`.
3. Shared/global files are append-only and single-owner (only Lane C writes `settings.json`).
4. Don't widen scope — report out-of-scope finds, don't fix them.
5. Verify before claiming done — paste real command output.
6. Report for integration: what changed, contract requests, verify output, cross-lane effects.

---

## Part C — Blocked feature swarms (dispatch ONLY after the gating spike says GO)

Reference the design docs — **do not re-derive**:

- **App-plugin embedding** (after **P3** GO) — build order in
  `docs/superpowers/specs/2026-06-07-app-plugins-design.md` §6. Backend lifecycle already merged;
  this adds `plugin_show/hide/set_rect` + child-webview positioning + the switcher, mounting under
  Lane O's overlay. Copy the exact webview API from the P3 SPIKE-RESULTS verbatim.
- **View-plugin runtime** (after **P4** GO) — build order in
  `docs/superpowers/specs/2026-06-07-view-plugins-design.md` §"Build order". **DESIGN-ONLY today**
  (zero runtime code). Branch **fresh off `main`**; reuses Lane O's overlay. Bridge/sandbox/SDK/loader/
  reference-plugin are all to-build.

When a spike clears, re-run **`swarm-handoff`** on that feature's design doc to carve its lanes.

---

## Deferred — needs your decision (not laned)

- **Item 2 — Swarm Handoff skill wrapper.** The fleetd engine is built (`crates/fleetd/src/swarm.rs`,
  `/swarms` endpoints, planner) and a generic `swarm-handoff` skill already exists. **Open scope
  question:** what should a command-center-specific wrapper do beyond those two — auto-invoke the
  fleetd `/swarms` engine from the orchestrator? The roadmap defers it "until the hand-made format
  proves out." Decide the scope, then it becomes a clean lane.

---

## Suggested skills
- **`using-git-worktrees`** + **`subagent-driven-development`** / **`dispatching-parallel-agents`** —
  fan-out machinery for Part B (worktree per repo-mutating lane).
- **`verify`** / **`run`** — drive the P3 harness (Lane A) and the spikes.
- **`swarm-handoff`** — re-run on each feature design doc once its spike clears (Part C).
- **`frontend-design`** — view-plugin runtime UI (match the cockpit HUD aesthetic).
- **`verification-before-completion`** — every lane reports real verify output, not assertions.
