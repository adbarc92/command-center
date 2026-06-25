# Handoff — Post-launch: spikes, overlays, and the road to a signed release

> ⚠️ **SUPERSEDED (2026-06-25)** — the overlay + packaging swarm shipped; the spike/cert track moved on.
> Current human-gated work: **[`2026-06-25-spikes-handoff.md`](2026-06-25-spikes-handoff.md)**; ship plan:
> [`2026-06-25-ship-readiness-swarm-handoff.md`](2026-06-25-ship-readiness-swarm-handoff.md). Kept for history.

**For:** the next agent (or future-you), 2026-06-11.
**Goal of the session:** with the launchable shell landed, take the product from *daily-usable* to
*shippable + feature-complete*. The remaining work is **mostly serial** — two human-gated go/no-go
spikes (P3, P4) gate the big feature builds, and a signed release needs certs you procure. There is
a **small 2-lane parallel swarm** (deferred overlays + packaging hardening) that can run anytime,
independent of the spikes.

**Repo:** `d:/MajorProjects/CURRENT/command-center` (Windows; PowerShell + Bash). `main` was
`affac36` at audit; **this plan assumes PRs #17–#21 have merged** (the launch swarm).

---

## Where things stand (audited 2026-06-10, assuming #17–#21 land)

Merged on `main` after the launch swarm:
- **SP1 spine** (e2e-proven, non-model path), **swarm-dispatch engine**, **roadmap tooling**.
- **Project Dashboard** + **app-plugins backend** (S0 / PR #16).
- **Lane A (#17):** top-level **Switcher** mounts Fleet ⇄ Projects; fleet state extracted to a shared
  rune store `cockpit/ui/src/lib/store.svelte.ts`. **The dashboard is now reachable.**
- **Lane B (#18):** Tauri **sidecar supervisor** (`src-tauri/src/sidecar.rs`) — self-launch +
  health-gate + restart-on-crash + kill-on-close, **live-proven** this session (killed → respawned in
  ~2s; graceful close → no orphan). Plus bundle signing/updater **config** +
  `docs/release/signing-and-updates.md` (the secret-name contract).
- **Lane C (#19):** `.github/workflows/{ci,release}.yml` — 3-OS matrix + tag-release.
- **Lane D (#20):** `docs/quickstart.md`.
- **Lane E (#21):** `crates/fleetd/tests/demo_mode_it.rs` — demo mode proven on `FakeRunner`, $0/no-Docker.

Worktrees: only `main` + `spike/app-plugins-webview` (kept — holds the Gate-1 webview result). Branches
`feat/app-plugins` (merged) and `feat/view-plugins` (unmerged, **stale**) remain.

### Five findings that shape this plan

1. **The two go/no-go spikes are interactive/visual — NOT swarmable.** They each need a *watched*
   session and gate the big feature builds:
   - **P3 (app-plugin embedding):** Gate 1 (build with Tauri `unstable`) PASS on
     `spike/app-plugins-webview` (`spikes/SPIKE-RESULTS-app-plugins.md`). **Gates 2–5** need Audience
     running at `:3000` + visual judgment (renders, resize ≤150ms, hide-on-overlay no-flash, lifecycle
     orphan check). Until this records an explicit **go**, no embedding production code starts.
   - **P4 (view-plugin runtime):** the sandboxed-iframe + **MessagePort** handshake spike — (a) iframe
     renders, (b) `plugin-hello → init` port round-trip across **100 reloads with zero dropped
     handshakes**, dev **and** packaged. Records to `spikes/SPIKE-RESULTS.md`.
2. **`feat/view-plugins` is STALE** (diff vs `main` `+906 / −18,667`) — exactly like `feat/app-plugins`
   was before S0. It predates the dashboard + roadmap tooling and touches the same manifests + dashboard
   files. Landing it needs a `main`-merge + manifest reconciliation — but that's premature until **P4
   says go** (it's an alternate skin, *not a launch gate*). De-stale only when you build on it.
3. **The human-authority overlays are a deferred, unblocked, swarmable-now gap.** Lane A deferred the
   **oracle-approval modal + real-mode launch confirm** (its STRETCH). **Both** plugin design docs
   require this exact "host overlay" (`inert`-ing the rest while open) as the trust boundary for human
   authority over an untrusted plugin surface. Building it now closes a real cockpit **safety gap** AND
   pre-builds shared infrastructure both plugin tracks need. → **Swarm Lane O.**
4. **Packaging has small, unblocked hardening gaps** (surfaced by the launch lanes):
   - `cockpit/ui/scripts/build-sidecar.mjs` builds + bundles a **debug-profile** sidecar
     (`cargo build -p fleetd --bin serve` → copies `target/debug/serve`); shipped release bundles carry
     a debug binary.
   - Lane B's updater **config** is wired but `tauri-plugin-updater` (crate + JS plugin) is **not
     registered** — no live update checks happen.
   - A cosmetic `failed to send message to the webview` line logs on clean exit (after the sidecar is
     already reaped — harmless, but noisy). → **Swarm Lane P.**
5. **Human/external gates (only you):** **S3** one live *paid* T1 mission (`ANTHROPIC_API_KEY`, real
   tokens, human-watched) — the last unproven slice of the spine; **code-signing certs** (Apple
   Developer ID + Windows Authenticode) — Lane B's `docs/release/signing-and-updates.md` says exactly
   what's needed and Lane C's `release.yml` consumes them by name.

Full lane/phase detail already exists — **do not re-derive**:
- App-plugin embedding build order: `docs/superpowers/specs/2026-06-07-app-plugins-design.md` §6.
- View-plugin runtime build order + spike: `docs/superpowers/specs/2026-06-07-view-plugins-design.md`
  §"Host overlay", §"Build order", §"Scope & phasing".

---

## Dependency analysis — swarmable now vs serial vs blocked

| Item | Kind | Disposition |
|---|---|---|
| Oracle-approval + real-mode overlays (cockpit safety gap) | frontend, isolated | **Swarm Lane O — now** |
| Packaging hardening (release sidecar · updater plugin · teardown log) | config/build, isolated | **Swarm Lane P — now** |
| P3 app-plugin embedding spike, gates 2–5 | interactive go/no-go | **Serial — you** |
| P4 view-plugin handshake spike | interactive go/no-go | **Serial — you** |
| S3 live paid T1 mission | credential + $ | **Serial — you** |
| Code-signing certs | procurement | **External — you** |
| App-plugin embedding (config→manifest→lifecycle→embed→Audience) | code | **Blocked on P3 go** → later swarm |
| View-plugin runtime (Spec-A) + branch de-stale | code | **Blocked on P4 go** → later swarm |
| Signed cross-platform release run | one-shot | **Blocked on certs** |

**The honest shape:** the bulk of remaining value is gated behind P3/P4 + human steps. Run the
2-lane swarm (O+P) anytime; drive the spikes hands-on; then the blocked feature swarms unblock.

---

## The swarmable-now mini-swarm — 2 lanes, ZERO owned-file overlap

Lane O is `src/` (frontend); Lane P is `src-tauri/` + `scripts/` + `package.json`. They never touch the
same file. Dispatch both off `main` (post-#17–#21), each in its own worktree.

### Lane O — Human-authority overlays   ·   ready
- **Scope:** Build the **oracle-approval modal** + **real-mode launch-confirm** overlay Lane A deferred.
  Triggered purely by the host's own `FleetState` (the `fleet` store), independent of any view/plugin.
  Focus-stealing modal at `z-index` above content; while open, the rest of the app is `inert` (so a
  future mounted plugin cannot receive input). This closes a safety gap and is the shared host-overlay
  infra both plugin tracks reuse.
- **Owns (exclusive write):** `cockpit/ui/src/App.svelte`, new `cockpit/ui/src/lib/ApprovalOverlay.svelte`,
  `cockpit/ui/src/lib/store.svelte.ts` (only if you must add a derived like `fleet.awaitingApproval`).
- **Reads (no write):** `cockpit/ui/src/lib/Switcher.svelte`, `cockpit/ui/src/views/Dashboard.svelte`,
  and the spec: `2026-06-07-view-plugins-design.md` §"Host overlay — human-authority actions" (the AC),
  `2026-06-07-app-plugins-design.md` §4.
- **Shared contract:** none in this swarm (Lane P never touches `src/`).
- **Done when:** a unit entering `awaiting_oracle_approval` pops a modal (frozen test set + APPROVE/
  REJECT) **regardless of the active view**; choosing REAL on a launch shows a confirm modal; while a
  modal is open, focus is the host's and the rest of the UI is `inert`; APPROVE/REJECT/confirm wire to
  the store's existing command sink (`fleet.cmd(id, 'approve_oracle'|'reject_oracle')`). The plugin/other
  views can show only a non-interactive "AWAITING APPROVAL" indicator — no approval verb.
- **Verify:** from `cockpit/ui`: `npm run check` (0 errors) · `npm run test` (existing **55** stay green
  + new overlay/trigger tests) · `npm run build`. Describe the modal trigger + `inert` behavior; note
  the manual GUI step (drive a unit to `awaiting_oracle_approval`, confirm the modal captures focus).
- **Notes:** `inert` on the *content subtree* (honored by WebView2/Chromium) is what actually blocks
  input — a backdrop + z-index alone does not. Keep the store the single source of fleet state.

### Lane P — Packaging & release hardening   ·   ready
- **Scope:** Three isolated fixes toward a clean shippable bundle: (1) ship a **release-profile** sidecar
  in bundles; (2) **activate** the updater runtime (`tauri-plugin-updater` crate + JS plugin) so Lane B's
  wired config actually checks for updates; (3) silence the cosmetic `failed to send message to the
  webview` teardown line on clean exit.
- **Owns (exclusive write):** `cockpit/ui/scripts/build-sidecar.mjs`,
  `cockpit/ui/src-tauri/{Cargo.toml, src/lib.rs, src/sidecar.rs}`, `cockpit/ui/package.json`,
  `cockpit/ui/src-tauri/capabilities/default.json`.
- **Reads (no write):** `cockpit/ui/src-tauri/tauri.conf.json` (the `updater` block Lane B wired —
  CI injects `pubkey`; don't fight it), `docs/release/signing-and-updates.md`.
- **Shared contract:** ⚠️ `lib.rs` already carries (preserve, additive only): `mod plugins`/`mod
  dashboard`/`mod sidecar`, `.manage(PluginManager)` + `.manage(SidecarSupervisor)`, the single
  6-command `invoke_handler`, and the `ExitRequested` handler (`sidecar.shutdown()` → `stop_all_owned`
  → `exit(0)`). Adding the updater plugin = one `.plugin(tauri_plugin_updater::Builder…)` + JS init.
- **Done when:** bundles carry a release sidecar (or you document why dev stays debug — see open
  question); `tauri-plugin-updater` registered, `cargo build` + `cargo clippy` + `npm run check` pass;
  the teardown line no longer logs on a clean close (or is documented as upstream-benign).
- **Verify:** from `cockpit/ui`: `npm run sidecar` (still works) · `cd src-tauri && cargo build` +
  `cargo clippy` · `cd .. && npm run tauri build` (unsigned, succeeds) · `npm run check`.
- **Open question (decide + record):** `build-sidecar.mjs` runs for BOTH `npm run desktop` (dev) and
  the bundle. A blanket switch to `--release` slows the dev inner loop. Prefer parametrizing — debug for
  dev, release only for `tauri build` (e.g. a `--release` flag / env the bundle's `beforeBuildCommand`
  passes). State what you chose.

**Build-ordering rule (both lanes that compile Rust):** `npm run sidecar` BEFORE any `cargo
build`/`tauri build` — the `externalBin` resource is checked at compile time.

**Dispatch & integration:** identical rules of the road to the prior swarms
([`2026-06-10-launch-swarm-handoff.md`](2026-06-10-launch-swarm-handoff.md) + the `swarm-handoff`
skill). Pre-create one worktree per lane off `main`; dispatch both in one message; each commits on its
branch (no push/PR); reports files + verify output. Then open 2 PRs. Integrate in any order (zero
overlap); reconcile with `npm run sidecar` → `cargo build`/`clippy` → `npm run check`/`test` →
`npm run tauri build` (unsigned).

---

## Serial / human-gated track (needs you — not swarmable)

Drive these hands-on, ideally in parallel with the O+P swarm:

- **P3 — app-plugin webview spike, gates 2–5.** Bring Audience up (`D:/MajorProjects/CURRENT/audience`,
  dev posture, `:3000`), then walk gates 2–5 on `spike/app-plugins-webview` and record the go/no-go +
  the exact webview API into `spikes/SPIKE-RESULTS-app-plugins.md` (§"Exact webview API that worked" —
  Phase 6 copies it verbatim). **Go → unblocks the app-plugin embedding swarm.**
- **P4 — view-plugin handshake spike.** Prove the sandboxed-iframe + MessagePort handshake (100 reloads,
  zero drops, dev + packaged); record to `spikes/SPIKE-RESULTS.md`. **Go → unblocks the view-plugin
  runtime swarm.**
- **S3 — one live paid T1 mission.** Set `ANTHROPIC_API_KEY`; dispatch a real T1 mission through
  oracle→build→review→PR on a throwaway repo; human-watched. The last unproven slice of the spine.
- **Code-signing certs.** Apple Developer ID ($99/yr + notarization) + Windows Authenticode. Lane B's
  `docs/release/signing-and-updates.md` enumerates exactly what's needed; Lane C's `release.yml`
  consumes them by name. → unblocks the **signed release run**.

---

## Blocked feature swarms (unblock after the spike says "go")

Do **not** start these until the gating spike records a go. The design docs already hold the
dispatch-ready detail — reference, don't re-derive.

- **App-plugin embedding (after P3 go).** Build order in `app-plugins-design.md` §6: (0) Tauri config —
  pin `unstable`, webview-label scheme + capabilities; (1) manifest/discovery *(already on main from
  S0)*; (2) lifecycle manager; (3) embedding + shell coordination (Rust webview create/position/
  show-hide **and** the Svelte switcher/rect/overlay-signal — one contract, built together, carrying the
  spike's glue forward); (4) wire Audience end-to-end. Lanes 2 and 3 split cleanly (Rust core vs
  embed+shell); the **Lane O overlay** is the human-authority layer they mount under.
- **View-plugin runtime (after P4 go).** First a **serial de-stale pre-step** (merge `main` into
  `feat/view-plugins`, reconcile the same manifests S0 did — see finding #2), then Spec-A build order in
  `view-plugins-design.md` §"Build order": loader/CSP → sandboxed iframe → MessagePort bridge → host
  command policy → view-switcher lifecycle. Reuses Lane O's overlay infra.
- **SHELL extension.** Once either runtime lands, extend the Switcher (already built, `ViewEntry`
  supports a `badge`) to mount the new view + register its commands.

---

## After this session

With O+P landed + the spikes driven: the cockpit has human-authority overlays, bundles ship a release
sidecar with live update checks, and you have explicit go/no-go on both plugin runtimes. Remaining to
**shippable** = certs + one signed release run (CI is ready). Remaining to **feature-complete** = the
two blocked feature swarms → app-plugin embedding + view-plugin runtime → SHELL mounts them.

## Suggested skills
- **`swarm-handoff`** — re-read before dispatching O+P (rules of the road, integration).
- **`dispatching-parallel-agents`** / **`using-git-worktrees`** — the fan-out machinery.
- **`frontend-design`** — for Lane O's overlay UI (matches the cockpit's existing HUD aesthetic).
- **`verification-before-completion`** — run the reconcile commands; report real output.
- **`subagent-driven-development`** — for the (later) blocked feature swarms once spikes clear.
