# Swarm Handoff — remaining work (post-swarm, 2026-06-11)

**For:** the next session. **Supersedes:** `2026-06-12-plan-and-swarm-handoff.md` (its Part B
3-lane swarm was **executed and merged** this session).
**One-line, honest state:** the **swarmable-now backlog is essentially cleared**. Everything
substantial that remains is **gated** — behind a human visual spike, a paid mission, cert
procurement, or a design decision. So tomorrow is primarily a **human spike day**, not a fan-out
morning. The two feature builds are **pre-carved + gated** below; the *one genuine multi-lane
swarm* only exists once **both** spikes clear (Part C).

**Repo:** `d:/MajorProjects/CURRENT/command-center`. `main` is source of truth.

---

## What landed this session (don't redo)

- **PR #25/#26/#27 (merged):** handoff docs tracked · ROADMAP status reconciled · **6D budget-checkpoint
  hook** shipped.
- **PR #11 (mcp-browser-bridge, merged):** MV3 service-worker disconnect fix.
- **3-lane swarm executed:** Lane A scaffolded the **P3 webview harness** (branch
  `spike/app-plugins-webview-v2` @ `4f36d85`, builds clean, exact `unstable` API recorded) · Lane B
  6D hook · Lane C wired **context-offload Tier-1 recall** + the **6D Stop hook** into live
  `~/.claude/settings.json` (both verified running; backup at `settings.json.bak-2026-06-11T01-11-01-993Z`).

**Net effect on remaining work:** view-plugin **store extraction** and **host overlay** are already
done (Lane O + the store extraction), and the P3 harness now exists — so the remaining feature work is
smaller than the design docs imply, and what's left is sequenced/gated rather than parallel.

---

## Part A — Human-gated serial track (you, not an agent) — **this is tomorrow's main work**

1. **P3 — app-plugin webview spike (gates 2–5). ▶ NOW RUNNABLE.** Lane A built the throwaway harness.
   - Open the worktree `.claude/worktrees/agent-a709aaf1bcad07d41` (branch `spike/app-plugins-webview-v2`),
     bring **Audience** up at `:3000`, `npm run sidecar && npm run tauri dev`, click **show/hide**, resize.
   - Walk gates 2–5 per `docs/handoff/2026-06-11-P3-app-plugin-webview-spike-guide.md`; record go/no-go +
     the exact API to `spikes/SPIKE-RESULTS-app-plugins.md`. **GO → unblocks B1.**
   - *Note from Lane A:* the harness drives the webview from **Rust** (not gated by the JS capability
     allowlist), so no `capabilities/default.json` change was needed for the spike. Production (B1 step 0)
     still adds webview capabilities for the dynamic label scheme — flagged, not blocking the spike.
2. **P4 — view-plugin handshake spike.** Still needs a harness (design-only). Sandboxed-iframe + MessagePort
   `plugin-hello → init`, **100 reloads, zero drops, dev AND packaged** → `spikes/SPIKE-RESULTS.md`.
   **GO → unblocks B2.** (Pre-step: scaffold a P4 harness — a small agent lane, mirror Lane A's pattern.)
3. **S3 — one live paid T1 mission.** `ANTHROPIC_API_KEY`; real T1 oracle→build→review→PR on a throwaway repo.
4. **Certs — code-signing.** Apple Developer ID + Windows Authenticode (procurement) → signed release.

---

## Part B — Pre-carved feature builds (GATED; each is a focused SERIAL build, not a swarm)

The design docs are explicit that these are single-contract sequences — do **not** force them into parallel
lanes. Dispatch each as one focused build (worktree, `frontend-design` for shell UI) the moment its spike GOes.

### B1 — App-plugin embedding   ·   blocked on **P3 GO**
- **Why serial:** `app-plugins-design.md` §6 — the Rust webview commands and the Svelte switcher/placeholder
  that drive them are **"two halves of one contract, built together"** (Critique R2 merged them). Steps 1–2
  (manifest + lifecycle) are **already merged**.
- **Remaining = §6 steps 0, 3, 4:** (0) pin Tauri + add `unstable` + webview capabilities for the
  `app::<id>` dynamic-label scheme in `capabilities/default.json`; (3) the embedding+shell slice —
  `plugin_show/hide/set_rect`, hide-on-overlay via `plugin://overlay-open/close`, topbar switcher entries +
  state chips + reserved-rect `ResizeObserver`, Fleet-stays-in-DOM; (4) wire Audience end-to-end (manifest +
  devAuth path, launch→use→quit, `docker ps` no-orphans).
- **Reuse:** Lane A's exact `WebviewBuilder`/`add_child`/`set_position`/`set_size` calls (in SPIKE-RESULTS).
  Carry the spike's throwaway Svelte glue forward — don't re-derive against a mock.
- **Owns:** `cockpit/ui/src-tauri/src/plugins/*` (embedding cmds), `cockpit/ui/src-tauri/Cargo.toml`,
  `capabilities/default.json`, the **App.svelte switcher region**. **Done when:** Audience launches in the
  cockpit, hides under an overlay within the §6 budget, quits with no orphaned containers.

### B2 — View-plugin runtime   ·   blocked on **P4 GO**
- **Why mostly serial:** `view-plugins-design.md` §"Build order" is a dependency chain. **Steps 2 (store
  extraction) and 4 (host overlay) are ALREADY DONE.** Remaining: **3 → 5 → 6**, sequenced.
- **Remaining:** (3) **Bridge** — MessagePort `plugin-hello` handshake + dirty-delta `state` +
  `log-append`/`log-reset` + **command policy** (shape/authority/cost/rate/flood-kill) + `command-ack`;
  (5) **SDK + loader** (dev-index | packaged scan) + `ccplugin://` scheme + both CSPs + **view-switcher** +
  liveness + the **reference plugin** (must exercise the full message surface); (6) **Battlefield** skin.
- **Open decision (yours):** ship **Spec-A (runtime) then Spec-B (Battlefield) as two cycles** (recommended)
  or one combined cycle. The build order is identical; only the merge/review boundary moves.
- **Owns:** `cockpit/ui/src/lib/{bridge.ts,loader.ts}` (new), `cockpit/ui/src-tauri/src/lib.rs` (scheme +
  plugin CSP header), `tauri.conf.json` (host CSP), `cockpit/plugin-sdk/`, `plugins/{reference,battlefield}/`,
  the **App.svelte switcher region**.

---

## Part C — The ONE genuine multi-lane swarm (only when **P3 GO _and_ P4 GO**)

B1 and B2 are independent features but **both mount into the same shell** — they collide on `App.svelte`'s
switcher and on `src-tauri/src/lib.rs` (command/scheme registry). That collision is exactly what a swarm
decomposition resolves: two feature lanes + a thin **shell-owner** lane holding the shared contracts.

### Lane A1 — App-plugin embedding   ·   ready when P3 GO
- **Scope/Owns/Done:** = **B1** above, MINUS the switcher mount + lib.rs registration (those go to the SHELL lane).
- **Shared contract:** `App.svelte` switcher + `src-tauri/src/lib.rs` → owned by **Lane S** → request: "add an
  App-plugin switcher entry + register `plugin_show/hide/set_rect`."
- **Verify:** Audience launches/hides/quits-clean in the cockpit.

### Lane A2 — View-plugin runtime   ·   ready when P4 GO
- **Scope/Owns/Done:** = **B2** above, MINUS the view-switcher mount + lib.rs scheme registration.
- **Shared contract:** `App.svelte` switcher + `src-tauri/src/lib.rs` → owned by **Lane S** → request: "add a
  view-plugin switcher entry + register the `ccplugin://` scheme + plugin-CSP header."
- **Verify:** reference plugin handshakes (100×, no drop), policy rejects forbidden commands, switcher A→B→A no leak.

### Lane S — SHELL owner (the shared-contract lane)   ·   integrates LAST
- **Owns (exclusive write):** `cockpit/ui/src/App.svelte` (the `[Fleet][Audience][View…]` switcher + which
  content kind mounts) and `cockpit/ui/src-tauri/src/lib.rs` (the one `invoke_handler` / scheme registry).
- **Collects:** Lane A1's + Lane A2's registration requests; wires both runtimes into one switcher; keeps the
  Fleet ops-grid the default in-DOM tab (regression canary).
- **Done when:** `npm run build && npm run check` clean, ops-grid unchanged, both a hosted app and a view-plugin
  reachable from the one switcher.

**Integration order:** A1 ∥ A2 in worktrees (disjoint except the contracts) → **Lane S last**, applying both
requests in one write → reconcile (`npm run check && npm run test`; ops-grid canary green).
**Rules of the road:** stay in lane; one worktree/branch per lane; only Lane S writes `App.svelte`/`lib.rs`;
verify with real output before "done".

---

## Deferred — needs your decision / external dependency (not laned)

- **Item 2 — Swarm Handoff skill wrapper.** Fleetd `/swarms` engine built; generic `swarm-handoff` skill
  exists. **Open scope question:** what should a CC-specific wrapper add (auto-invoke the engine from the
  orchestrator?). Decide → becomes a clean lane.
- **Item 3 Tier 2** 🔗 claude.ai connector reliability. **Item 6B** 🔗 ContextCurator ships ([[contextcurator-is-users-own-product]]).

---

## Suggested skills
- **`verify`** / **`run`** — drive the P3 harness + spikes (Part A).
- **`swarm-handoff`** — re-run on each feature doc to refine its lanes once its spike clears.
- **`using-git-worktrees`** / **`subagent-driven-development`** — fan-out for Part C (worktree per lane).
- **`frontend-design`** — the switcher + view-plugin UI (cockpit HUD aesthetic).
- **`verification-before-completion`** — real verify output per lane, not assertions.
