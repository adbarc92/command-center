# Handoff — Tomorrow's session: finish the launchable shell (organized for parallel dispatch)

**For:** the next agent (or future-you), 2026-06-10.
**Goal of the session:** take the merged-but-unreachable dashboard + the documented-but-unbuilt
launch lanes to a **daily-usable, self-launching desktop app**, by running one **serial pre-step**
then a **5-lane parallel swarm**. Then the only things left for launch are human-gated (spikes,
API-key run, certs).

**Repo:** `d:/MajorProjects/CURRENT/command-center` (Windows; PowerShell + Bash). `main` @ `afb782d`.

---

## Where things stand (audited 2026-06-09, post-cleanup)

Merged on `main`: SP1 spine (e2e-proven), swarm-dispatch engine, roadmap tooling (`tools/**`,
`docs/playbooks/**`), the **Project Dashboard build** (`cockpit/ui/src/lib/dashboard/**`,
`views/Dashboard.svelte`, `src-tauri/src/dashboard.rs`), and all three swarm planning docs.

Worktrees/branches were pruned: only `main` + `spike/app-plugins-webview` (kept — holds the webview
Gate-1 result) remain as worktrees; unmerged `feat/app-plugins` and `feat/view-plugins` are kept.

**Three findings that shape this plan:**
1. **The dashboard is merged but UNREACHABLE.** `lib.rs` registers its Tauri commands, but
   `App.svelte` has **zero** references to it — no switcher, no mount. It's dead code until the shell
   mounts it. (This is why Lane A below exists and is the top priority.)
2. **`feat/app-plugins` is STALE.** Diff vs `main` is `+4,393 / −12,266`; it predates the roadmap
   tooling + dashboard and touches the same manifests Lane A changed (`Cargo.toml`, `package.json`,
   `lib.rs`). So merging it is **no longer zero-conflict** — it needs `main` merged in + those
   manifest conflicts resolved. → **Serial pre-step S0.**
3. **Launch lane L2 (demo-mode) is already implemented** — `server.rs` routes `mode:"demo"` →
   `FakeRunner`. So L2 becomes *verify-and-harden*, not build (Lane E).

Full lane briefs already exist — **do not re-derive them**, reference:
- Launch lanes L1/L3/L4/L5: [`docs/swarm/2026-06-09-launch-readiness.md`](../swarm/2026-06-09-launch-readiness.md) §"The lanes".
- SHELL lane: [`docs/swarm/2026-06-09-stage3-product-buildout.md`](../swarm/2026-06-09-stage3-product-buildout.md) §"Lane SHELL".

---

## Serial pre-step S0 — de-stale & land `feat/app-plugins` (do FIRST, alone)

Not a lane — it reconciles a stale branch and must land before the swarm branches off `main`, or
every lane re-conflicts on the manifests.

1. `git switch feat/app-plugins` (or a worktree), `git merge main`.
2. Resolve conflicts — expected on `cockpit/ui/src-tauri/{Cargo.toml,lib.rs}` and
   `cockpit/ui/package.json`. These are **additive unions**: keep both the dashboard's entries (now
   on main: `reqwest`, `mod dashboard` + its `invoke_handler` lines, vitest `^3`) and app-plugins'
   entries (`mod plugins`, plugin deps, its `plugins.ts`/manifest). `App.svelte` should NOT conflict
   (Phase 5 never touched it).
3. Verify: `npm run sidecar` → `cargo build` + `cargo clippy` + `cargo test` (17 plugin tests) →
   `npm run check` + `npm run test` (dashboard 45 + plugin 2). Reconcile the single vitest config.
4. PR → `main`. **Phase 6 (embedding) stays stubbed** — it's spike-gated (P3 below). This just lands
   the plugin **backend** so its `plugin://state` events light up the dashboard's app-plugin adapter.

After S0 merges, `main` carries dashboard + plugin backend with coherent manifests. **Branch all
lanes off that `main`.**

---

## The parallel swarm — 5 lanes, ZERO owned-file overlap

By construction these never touch the same file. No global/out-of-repo files → **no Lane Z.**

| Lane | Builds | Owns (exclusive write) |
|---|---|---|
| **A — Shell: make dashboard reachable** | Top-level switcher in `App.svelte` mounting **Fleet + Projects**; extract the fleet store so views share one source. (Stretch: the oracle-approval + real-mode overlays.) | `cockpit/ui/src/App.svelte`, new `cockpit/ui/src/lib/Switcher.svelte`, new `cockpit/ui/src/lib/store.svelte.ts` |
| **B — Tauri host: sidecar + signing** | L1 sidecar supervisor (spawn `fleetd-serve` on launch, health-gate, restart, kill-on-close) **+** L3 bundle signing/updater config. Co-located because they share the src-tauri manifests. | `cockpit/ui/src-tauri/src/{main.rs,lib.rs}`, `Cargo.toml`, `capabilities/default.json`, `tauri.conf.json`, `docs/release/signing-and-updates.md` |
| **C — Release CI/CD** | L4: Win/macOS/Linux build+test matrix + tag-release. Refs Lane B's documented secret names. | `.github/workflows/**` |
| **D — Day-1 quickstart** | L5: clone → build → launch → demo mission → real mission → resume → troubleshooting. | `docs/quickstart.md` |
| **E — Demo-mode verify + harden** | Confirm `mode:"demo"` runs the full phase progression on `FakeRunner` with $0/no-Docker; add the missing integration test; fix any gap. | `crates/fleetd/tests/demo_mode_it.rs` (+ small fixes in `crates/fleetd/src/**` if a gap is found) |

**Shared contracts:**
- Lane A owns `App.svelte` + the store; Lane B owns `lib.rs` + manifests → **no overlap** (A is
  `src/`, B is `src-tauri/`). The dashboard's `lib.rs` command registration is already on main from S0.
- Lane C references Lane B's signing **secret names** (a string contract via
  `docs/release/signing-and-updates.md`) — no merge dependency.
- Build-ordering rule for B & C: **`npm run sidecar` before any `cargo build`/`tauri build`** (the
  `externalBin` resource is checked at compile time — see `spikes/SPIKE-RESULTS-app-plugins.md`).

**Dispatch procedure & rules of the road:** identical to the prior swarm handoff
([`docs/handoff/command-center-launch-swarm-handoff.md`](command-center-launch-swarm-handoff.md) §4) —
pre-create one worktree per lane off `main`, paste each lane's brief + a worktree-pinned header,
dispatch all in one message, each commits on its branch (no push/PR), reports files + contract
requests + real verify output. Then open **5 separate PRs** (user's established preference).

**Integration:** lanes merge in any order (zero overlap); reconcile on the merged tree with
`npm run sidecar` → `cargo test --workspace` → `npm run tauri dev` — confirm the switcher mounts
Fleet + Projects, the sidecar self-launches and survives a kill, and a demo mission runs with no
Docker.

---

## Serial / human-gated track (needs you, not swarmable)

- **P3 — app-plugins webview spike, gates 2–5** (watched GUI + Audience up at :3000). Gate 1 already
  PASS on `spike/app-plugins-webview`. Unblocks app-plugin **embedding** (a later lane).
- **P4 — view-plugin handshake spike** (iframe sandbox + MessagePort). Unblocks the view-plugin runtime.
- **S3 — one live paid T1 mission** (set `ANTHROPIC_API_KEY`; real tokens; human-watched). The last
  unproven slice of the spine.
- **Code-signing certs** — Apple Developer ID + Windows Authenticode (procurement). Lane B documents
  exactly what's needed; CI (Lane C) consumes them by name.

## After this session

With S0 + lanes A–E landed: the dashboard is visible, the app self-launches, demo mode is proven, CI
builds cross-platform, and there's a quickstart. Remaining to "shippable" = certs + a signed release
run; remaining to "feature-complete" = the two spikes (P3/P4) → app-plugin embedding + view-plugin
runtime → SHELL mounts those views too (extend Lane A's switcher).

## Suggested skills
- **`swarm-handoff`** — re-read before dispatching (rules of the road, integration).
- **`dispatching-parallel-agents`** / **`using-git-worktrees`** — the fan-out machinery.
- **`verification-before-completion`** — run the reconcile commands; report real output.
- **`receiving-code-review`** — when integrating each lane's contract requests.
