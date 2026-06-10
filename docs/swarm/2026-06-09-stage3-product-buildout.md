# Command Center — Stage 3 Swarm: Product Buildout (SP3 dashboard + SP5 plugins)

> Third swarm in the sequence. Prior: the **agent-operation** roadmap swarm (PRs #7–#12, merged) and
> the **launch-readiness** swarm (`docs/swarm/2026-06-09-launch-readiness.md`, PR #13 — doc merged,
> lanes L1–L5 handed off via `docs/handoff/command-center-launch-swarm-handoff.md`). This stage turns
> the launchable single-view shell into a **multi-view product**: the project dashboard (SP3) and the
> plugin system (SP5 — app-plugin embedding + view-plugin runtime). Produced via the `swarm-handoff`
> method. Anchored to [`docs/command-center-vision.md`](../command-center-vision.md) (SP1–SP5) and the
> two merged design specs cited below.

## Audited current state (2026-06-09)

- ✅ **SP1 spine** — merged, non-model e2e proven (real PR `command-center-agent-sandbox#4`).
- ✅ **Swarm-dispatch (roadmap item 2) is BUILT and merged** — `crates/fleetd/src/{swarm,planner}.rs`,
  `swarms`/`swarm_lanes` store tables, `POST/GET /swarms`. The engine can already fan out + admit
  lanes under a budget. *(So this stage's swarms could eventually be auto-dispatched by the daemon.)*
- ✅ **Cockpit (main)** — ~95% complete single-view fleet console (mission form, unit grid, detail
  rail, cost/burn, reconnect). Gaps: human-authority overlays (oracle-approval modal, real-mode
  confirm), and no top-level view switcher.
- 🌿 **`feat/app-plugins` (UNMERGED)** — Phases 2–5 done: manifest, discovery, lifecycle state
  machine, real Tauri seams, manager, Svelte chip mapping. **17 Rust + 2 JS tests green, zero overlap
  with main.** Phase 6 (embedding) not started. Spec:
  [`specs/2026-06-07-app-plugins-design.md`](../superpowers/specs/2026-06-07-app-plugins-design.md).
- 🌿 **`feat/view-plugins` (UNMERGED)** — design only, no code; independent of app-plugins. Spec:
  [`specs/2026-06-07-view-plugins-design.md`](../superpowers/specs/2026-06-07-view-plugins-design.md).
- 📋 **Project dashboard (SP3 / item 4)** — design-approved, 0 TBDs:
  [`specs/2026-06-09-project-dashboard-design.md`](../superpowers/specs/2026-06-09-project-dashboard-design.md).
  Buildable now against a **CLI-spawn Halyard adapter** (no Halyard-repo change required).
- 🔬 **app-plugins webview spike (S4)** — Gate 1 PASS (`unstable` builds); gates 2–5 (render / latency
  / orphan-free) await a watched session. **Gates app-plugin embedding.**

## Prerequisites (serial — clear these BEFORE dispatching the lanes)

These are not lanes; they are ordering gates the orchestrator/human clears first.

- **P1 — Land launch-readiness L1–L5.** This stage assumes the sidecar supervisor (L1), demo-mode
  (L2), and CI (L4) are on `main`; the dashboard and shell rely on a runnable, self-launching app.
- **P2 — Merge `feat/app-plugins` Phases 2–5 to `main`** (zero conflict; Phase 6 stays stubbed). The
  plugin manager + `plugin://state` events are inputs to both Lane A (dashboard signal) and Lane B.
- **P3 — Record app-plugins webview spike S4 gates 2–5** (go/no-go + the exact `unstable` webview
  API). **Lane B cannot start until this records a go and the API.** Watched session; see
  `spikes/SPIKE-RESULTS-app-plugins.md`.
- **P4 — Run view-plugins Spike #1** (iframe `sandbox` + CSP + MessagePort `plugin-hello`→`init`
  handshake across reloads, no dropped init). **Lane C's Phase-A build is gated on this.**

## The coupling problem & how we carve around it

Four features all want the same two files — `cockpit/ui/src/App.svelte` (the shell: switcher +
overlays + which view is mounted) and `cockpit/ui/src-tauri/src/lib.rs` (Tauri command registration).
If split, they collide. So **one lane (SHELL) owns both**; the feature lanes own their own modules and
file *contract requests* (a switcher entry + command registration) back to SHELL, which integrates
last. This is the same single-owner pattern the roadmap swarm used for the global config files.

---

## The lanes

### Lane A — Project Dashboard (SP3 / item 4-build)   ·   ready (after P1–P2)
- **Scope:** Build the read-only project board from the approved spec — tells, at a glance, what stage
  every project is in. Four source adapters → one `ProjectCard` model → a board view.
- **Owns (exclusive write):** `cockpit/ui/src/lib/dashboard/**` (`ProjectCard`/`StageSignal` model,
  stage-precedence logic, the four adapters), `cockpit/ui/src/views/Dashboard.svelte`,
  `cockpit/ui/src-tauri/src/dashboard.rs` (Halyard CLI-spawn + Audience HTTP poll commands).
- **Reads (no write):** the dashboard spec (implement §3–§8 verbatim); fleet `phase_changed` events
  and `plugin://state` events (subscribe, read-only); `docs/digests/{halyard,audience}-digest.md`.
- **Shared contract → SHELL:** request (a) a **"Projects"** switcher entry mounting
  `views/Dashboard.svelte`, and (b) registration of your `dashboard.rs` commands in `lib.rs`.
- **Depends on / blocks:** needs P1–P2. Independent of B and C.
- **Done when:** the board renders `ProjectCard`s for Halyard releases + Audience posts + fleet
  missions + app-plugin lifecycle, mapped to the canonical stage pipeline, with stale-greying and
  deep-links out; degrades gracefully when a source is down.
- **Verify:** with Halyard + Audience reachable, the board shows correct stages; kill one source → its
  cards grey, others unaffected; a fleet mission's stage advances live as `phase_changed` arrives.
- **Notes:** start on the **CLI-spawn Halyard adapter** (spec §6.1/§7) — no Halyard-repo change. The
  HTTP "Halyard head" is a separate, non-blocking Halyard project; the adapter swaps later with zero
  board change. Largest lane — keep the four adapters behind one `ProjectCard` interface.

### Lane B — App-plugin embedding (SP5 / app-plugins Phase 6)   ·   blocked on P3
- **Scope:** Finish app-plugins: embed a running app (proving app: Audience) as a child webview inside
  the cockpit, positioned under a Svelte rect, shown/hidden by the switcher, torn down cleanly.
- **Owns (exclusive write):** `cockpit/ui/src-tauri/src/plugins/embed.rs` (create/position/show/hide/
  destroy using the **exact API recorded by the S4 spike**), additions to
  `cockpit/ui/src-tauri/src/plugins/manager.rs`, the Audience app-plugin manifest under
  `docs/app-plugins/audience/` (+ a smoke checklist).
- **Reads (no write):** `spikes/SPIKE-RESULTS-app-plugins.md` (the recorded webview API — do NOT guess
  it), the app-plugins spec §6, the merged `plugins/{state,seams,manager}.rs`.
- **Shared contract → SHELL:** request an **"Audience"** switcher entry + registration of the
  `plugin_show/hide/set_rect/overlay` commands in `lib.rs` + a reserved webview rect from the shell.
- **Depends on / blocks:** **blocked on P3 (spike go + API).** If the spike says no-go ("C" =
  separate OS windows), only `embed.rs`'s mechanism changes; everything else holds. Independent of A.
- **Done when:** cold-launch → lifecycle chips `building→…→healthy` → Audience renders in-cockpit →
  switching away hides it (<150ms, no flash) → quitting leaves no orphan container (`docker ps`).
- **Verify:** run the smoke checklist (dev + packaged); `docker ps` clean after quit; paste output.
- **Notes:** copy the spike's API verbatim. Don't widen into view-plugins (different trust model).

### Lane C — View-plugin runtime + reference skin (SP5)   ·   blocked on P4
- **Scope:** Stand up the sandboxed view-plugin runtime (untrusted iframe renderers over shared
  fleet-state via MessagePort) and ship one reference plugin proving the bridge end-to-end.
- **Owns (exclusive write):** `cockpit/ui/src-tauri/src/viewplugins/**` (the `ccplugin://` scheme
  handler / loader), `cockpit/ui/src/lib/viewhost/**` (MessagePort bridge + command policy +
  state-projection), `cockpit/ui/viewplugins/reference/**` (the trivial reference plugin).
- **Reads (no write):** the view-plugins spec (implement the manifest + handshake + policy verbatim);
  the shared fleet store SHELL owns (read-only projection).
- **Shared contract → SHELL:** request a **skin** switcher entry + read access to the extracted
  `lib/store.svelte.ts` (SHELL owns it); the host-overlay modals for oracle/real-mode are SHELL's.
- **Depends on / blocks:** **blocked on P4 (handshake spike).** Independent of A and B (distinct trust
  model, distinct loader, distinct files).
- **Done when:** the reference plugin loads in a sandboxed iframe, completes the `plugin-hello`→`init`
  handshake, receives live per-unit state deltas, and can issue a (policed) launch/command — with
  oracle-approve/real-mode still host-only.
- **Verify:** reference plugin echoes live fleet state across ≥100 iframe reloads with zero dropped
  handshakes; a plugin attempt to approve an oracle is rejected by host policy. Paste output.
- **Notes:** sandbox-first — the iframe gets null origin, no network, no daemon access. Don't reuse the
  app-plugin loader (opposite trust model).

### Lane SHELL — Cockpit shell integration owner (hotspot owner; integrates LAST)
- **Scope:** Own the two coupling files; assemble the multi-view shell; add the human-authority
  overlays; mount every feature lane's view and register its commands.
- **Owns (exclusive write):** `cockpit/ui/src/App.svelte`, `cockpit/ui/src/lib/store.svelte.ts`
  (extract the fleet-state store out of App.svelte so views/plugins share one source),
  `cockpit/ui/src-tauri/src/lib.rs`, a new `cockpit/ui/src/lib/Switcher.svelte`.
- **Reads (no write):** each feature lane's exported view/command surface; the view-plugins spec's
  host-overlay section.
- **Shared contract:** **you are the single writer of `App.svelte` + `lib.rs`.** Collect A/B/C's
  contract requests (switcher entries, command registrations, reserved rect, store access) and apply
  them in one pass. Build the **oracle-approval modal** + **real-mode confirm** overlays (the cockpit
  gap) here, `inert`-ing any mounted plugin while a modal is open.
- **Depends on / blocks:** integrates after A/B/C land their modules; B/C contributions arrive only
  once P3/P4 clear (SHELL can integrate A first, then B/C as they unblock).
- **Done when:** a top-level switcher toggles Fleet / Projects (A) / Audience (B, if go) / skin (C, if
  ready); the store is shared; oracle-approval + real-mode modals work and suppress input to mounted
  plugins.
- **Verify:** `npm run tauri dev` → switch across all available views without state loss; trigger an
  awaiting-oracle unit → the modal appears over any mounted plugin and the plugin is `inert`.
- **Notes:** keep `store.svelte.ts` the one source of fleet state. Don't implement feature internals —
  only mount + register + overlays.

---

## Rules of the road (every dispatched agent)

1. **Stay in your lane** — write only your Owns paths. **Never edit `App.svelte` or `lib.rs`** (SHELL
   owns them) — file a contract request instead.
2. **Branch/worktree per lane**, off `main`; never commit to `main`.
3. **Feature lanes request shell mounts; SHELL is the single writer** of the two hotspot files.
4. **Don't widen scope** — A≠B≠C trust models and files are distinct; report cross-lane discoveries.
5. **Verify before done** — run your Verify check, paste real output.
6. **Report for integration** — files changed, contract requests to SHELL, verify output, cross-lane notes.

## Integration plan

1. Merge **Lane A** (dashboard) once it lands — independent of the spikes.
2. Merge **Lane B** after P3 (go) and **Lane C** after P4; both are file-isolated from A and each other.
3. Merge **Lane SHELL last**, applying the collected switcher/command/rect/store contract requests in
   one pass, plus its own overlays.
4. **Reconcile:** `npm run sidecar` → `cargo test --workspace` → `npm run tauri dev`; confirm the
   switcher mounts every available view, the shared store has no duplication, and oracle/real-mode
   modals suppress input to mounted plugins.

## Serial / spike track (orchestrator / human — not lanes)

- **P3 — app-plugins webview spike gates 2–5** (watched GUI + Audience up). Gates Lane B.
- **P4 — view-plugins Spike #1** (iframe sandbox + MessagePort handshake). Gates Lane C.
- **S3 — live paid T1 mission** (needs `ANTHROPIC_API_KEY` + $; human-watched). The last unproven
  slice of the spine — orthogonal to this stage but worth doing alongside.
- **app-plugins Phases 2–5 merge (P2)** and **launch-readiness L1–L5 merge (P1)** — serial merges.

## Deferred / blocked (NOT this stage)

- **SP2 rich onboarding** (grill-me wired into the mission form + test-quality scoring) — its UI lives
  in `App.svelte` (SHELL) and its scoring touches the oracle (fleetd); fold into a follow-on once this
  stage's shell stabilizes, to avoid serializing SHELL further.
- **SP4 deployment pillar** — design-only; the dashboard is read-only this cycle (deep-links out).
- **Halyard head** — separate Halyard-repo project; the CLI-spawn adapter makes it non-blocking.
- **6B ContextCurator** (user's own product) and **3 Tier-2 claude.ai KB** (connector reliability) —
  external blockers; integrate when available.
- **Engine fleet-scaling** (durable command journal, dropped-event backpressure, multi-unit dashboard
  aggregates) — real candidates, but an *engine* track; spin a separate swarm if prioritized over
  product buildout.
