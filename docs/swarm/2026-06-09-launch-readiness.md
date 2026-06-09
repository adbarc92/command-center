# Command Center — Launch-Readiness Swarm (2026-06-09)

> Companion to [`docs/ROADMAP.md`](../ROADMAP.md) and
> [`docs/command-center-vision.md`](../command-center-vision.md). The ROADMAP's items 1–6 are
> *agent-operation* features (already swarmed → PRs #7–#12). **This doc covers the work that gates
> actually launching the product**: the SP1 spine becoming daily-usable, the app-plugins embedding,
> and desktop packaging. Produced via the `swarm-handoff` method.

## Launch = two milestones

- **Usable (by you, daily):** the SP1 spine runs as an app — dispatch a mission → oracle → container
  → verified PR — without manual daemon babysitting, and a no-Docker demo path to try it cold.
- **Shippable (to others):** cross-platform signed bundles produced by CI, with a day-1 guide.

## Current reality (verified 2026-06-09)

SP1 is **functionally complete and merged** (PRs #1–#4): state machine, real `LocalDockerRunner`,
oracle phase, build/check/review loop, `GhForge`, SQLite persistence + resume, rate-limit
resilience. 29 + 43 unit tests green.

Two launch blockers turned out **already done**, verified this session:

- ✅ **`cc-agent:dev` Docker image exists** (built 3 days ago; Docker 28.3.3 healthy).
- ✅ **End-to-end spine proof PASSES (non-model path).** `local_docker_it::provision_commit_export_roundtrip`
  and `preflight_it::full_pipeline_opens_a_real_mergeable_pr` both green — the latter opened a **real
  mergeable PR** on the sandbox: `adbarc92/command-center-agent-sandbox#4`. The Docker→git→checks→GitHub
  plumbing is proven on real infra. (Only the *paid model* path remains unproven — see Serial S2.)

---

## Dependency analysis — swarmable vs serial vs blocked

| Item | Kind | Disposition |
|---|---|---|
| A2 Tauri sidecar supervisor | code, isolated | **Lane L1** |
| A4 Demo-mode FakeRunner path | code, isolated | **Lane L2** |
| C9 Bundle signing + updater config | config + doc, isolated | **Lane L3** |
| C8 Release CI/CD | new files, isolated | **Lane L4** |
| C10 Day-1 quick-start | new doc, isolated | **Lane L5** |
| A1 Build agent image | one-shot | **Serial S1** — *already done* |
| A3 Non-model e2e proof | one-shot validation | **Serial S2** — *done this session* ✅ |
| A3′ Live paid model run | one-shot, needs key + $ | **Serial S3** — needs you |
| B5 Phase-0 webview spike | exploratory, gates B | **Serial S4** — orchestrator, hands-on |
| B6/B7 App-plugins Phase-1 + embedding | code | **Blocked** on S4 |
| Code-signing certs | procurement | **External** — only you |

**The core independence test holds:** the five lanes below have **zero owned-file overlap**. The one
shared subtree, `cockpit/ui/src-tauri/`, is split *by file* — L1 owns the Rust + `Cargo.toml` +
`capabilities/`, L3 owns `tauri.conf.json` — so they never collide. **No out-of-repo global files
this time → no Lane Z needed.**

---

## The lanes (dispatch-ready — each self-contained)

### Lane L1 — Tauri sidecar supervisor   ·   ready   ·   (roadmap A2)
- **Scope:** Make the desktop app self-contained: on launch, spawn the bundled `fleetd-serve`
  sidecar, gate the UI until `/health` is green, restart it if it crashes, and kill it on window
  close. Today the user must run `cargo run --release --bin serve` by hand.
- **Owns (exclusive write):** `cockpit/ui/src-tauri/src/main.rs`, `cockpit/ui/src-tauri/src/lib.rs`,
  `cockpit/ui/src-tauri/Cargo.toml`, `cockpit/ui/src-tauri/capabilities/default.json`.
- **Reads (no write):** `cockpit/ui/src-tauri/tauri.conf.json` (externalBin already declares
  `binaries/fleetd-serve`), `cockpit/ui/scripts/build-sidecar.mjs`, `crates/fleetd/src/bin/serve.rs`,
  `cockpit/ui/src/lib/api.ts`.
- **Shared contract:** `tauri.conf.json` is **owned by L3** — do not edit it. If you need a config
  key there (you shouldn't — externalBin exists), file a request to L3.
- **Depends on / blocks:** none.
- **Done when:** `npm run tauri dev` launches the window, the UI shows a "starting daemon…" state
  until `/health` returns ok, killing the `fleetd-serve` process triggers an automatic restart, and
  closing the window leaves no orphaned `fleetd-serve`.
- **Verify:** `npm run sidecar && npm run tauri dev`; from another shell `taskkill /IM fleetd-serve.exe`
  and watch it respawn; close the app and confirm `tasklist | findstr fleetd-serve` is empty.
- **Notes:** Use Tauri v2 sidecar APIs (`tauri-plugin-shell` or `tauri::process::Command`); add the
  shell-execute permission to `capabilities/default.json`. Keep the supervisor in Rust, not JS, so it
  survives webview reloads.

### Lane L2 — Demo-mode FakeRunner path   ·   ready   ·   (roadmap A4)
- **Scope:** Let a mission dispatched with `mode: "demo"` run end-to-end with **no Docker and no API
  key** — the `FakeRunner` plays scripted oracle/build/review outputs, cost is zeroed, and the full
  event stream still emits so the cockpit renders identically. This is the "try it cold" story.
- **Owns (exclusive write):** `crates/fleetd/src/server.rs`, `crates/fleetd/src/driver.rs`, and a new
  `crates/fleetd/tests/demo_mode_it.rs`.
- **Reads (no write):** `crates/fleetd/src/fake.rs`, `crates/fleetd/src/runner.rs`,
  `crates/fleet-core/src/*`.
- **Shared contract:** none — `crates/fleetd/**` is touched by no other lane.
- **Depends on / blocks:** none.
- **Done when:** `create_mission` branches on `mode`; a `demo` unit drives the full phase progression
  (`SPEC → BUILDING → CHECKING → REVIEWING → … → PR_OPEN/DONE`) via `FakeRunner` with `$0` metered and
  no Docker call; a `real` unit is unchanged.
- **Verify:** add `demo_mode_it.rs` asserting a demo unit reaches a terminal phase with zero Docker
  invocations and zero cost; `cargo test -p fleetd` green; `cargo clippy --all-targets` clean.
- **Notes:** `server.rs::create_mission` already accepts `mode` but always constructs
  `LocalDockerRunner` — branch there. Don't open a real PR in demo (the forge is faked too).

### Lane L3 — Bundle signing + auto-updater config   ·   ready   ·   (roadmap C9)
- **Scope:** Author the *configuration and documentation* for signed, updatable cross-platform
  bundles. Cert **procurement is out of scope** (human/external) — this lane lays the wiring with
  clearly-named placeholders and documents exactly what secrets are needed.
- **Owns (exclusive write):** `cockpit/ui/src-tauri/tauri.conf.json`, `docs/release/signing-and-updates.md`.
- **Reads (no write):** Tauri v2 bundle/updater docs, `cockpit/ui/package.json`.
- **Shared contract:** **L4 (CI) will reference your secret names** — define them in the doc
  (`APPLE_CERTIFICATE`, `APPLE_ID`, `APPLE_TEAM_ID`, `WINDOWS_CERTIFICATE`, `TAURI_SIGNING_PRIVATE_KEY`,
  etc.) so L4 can wire them. That doc list IS the contract.
- **Depends on / blocks:** none (L4 reads your doc, doesn't block on your merge).
- **Done when:** `tauri.conf.json` has bundle targets for Win (`msi`/`nsis`), macOS (`app`/`dmg`),
  Linux (`appimage`/`deb`); an `updater` block (endpoint placeholder + pubkey field); signing identity
  fields wired to env; and `docs/release/signing-and-updates.md` lists every cert/secret + how to
  obtain it (Apple Developer ID + notarization; Windows Authenticode).
- **Verify:** `npm run tauri build` still succeeds locally **unsigned** (no cert present → dev bundle);
  `tauri.conf.json` validates; the doc enumerates the exact secret names L4 consumes.
- **Notes:** Do **not** commit any real cert/key. Keep `signingIdentity`/updater pubkey as
  env-substituted placeholders so unsigned local builds keep working.

### Lane L4 — Release CI/CD   ·   ready   ·   (roadmap C8)
- **Scope:** First CI for the repo: a cross-platform matrix that builds + tests on every push, and a
  tag-triggered release that produces bundles. Validates the "runs on Win/macOS/Linux" constraint
  that is currently unautomated.
- **Owns (exclusive write):** `.github/workflows/` (new — `ci.yml`, `release.yml`).
- **Reads (no write):** `cockpit/ui/package.json`, `cockpit/ui/scripts/build-sidecar.mjs`, `Cargo.toml`,
  `cockpit/ui/src-tauri/tauri.conf.json`, and L3's `docs/release/signing-and-updates.md` for secret names.
- **Shared contract:** reference L3's documented secret names by `${{ secrets.* }}` — do not invent
  signing config in the workflow that belongs in `tauri.conf.json`.
- **Depends on / blocks:** soft-reads L3's doc; can be authored in parallel.
- **Done when:** `ci.yml` runs `cargo test --workspace` (the two Docker ITs stay `--ignored` in
  hosted CI — note this explicitly), builds the frontend + sidecar, and runs `tauri build` on
  `windows-latest`, `macos-latest`, `ubuntu-latest`, uploading bundles as artifacts; `release.yml`
  fires on `v*` tags.
- **Verify:** workflows pass `actionlint` (or `python -c "import yaml,glob; [yaml.safe_load(open(f)) for f in glob.glob('.github/workflows/*.yml')]"`); document that the Docker ITs need a self-hosted/dind runner (out of scope) and are skipped in hosted CI.
- **Notes:** `log()` the coverage gap honestly — hosted CI cannot run the real-Docker integration
  tests; they remain a local/self-hosted gate.

### Lane L5 — Day-1 quick-start + ops guide   ·   ready   ·   (roadmap C10)
- **Scope:** The missing "from clone to first mission" guide. The specs assume deep familiarity; a new
  user (or future-you) has no on-ramp.
- **Owns (exclusive write):** `docs/quickstart.md`.
- **Reads (no write):** `docs/command-center-vision.md`, the SP1 design spec, `crates/fleetd/src/server.rs`
  (endpoints), `cockpit/ui/package.json` scripts, `.env.example`, `deploy/agent-image/Dockerfile`.
- **Shared contract:** none — pure new doc.
- **Depends on / blocks:** none.
- **Done when:** the doc walks: prerequisites (Docker, `gh` auth, optional `ANTHROPIC_API_KEY`) →
  build (`cargo build --release`, `npm install`, image already built or `docker build deploy/agent-image`)
  → launch the app → dispatch a **demo** mission (no key) → dispatch a **real** T1 mission → resume a
  halted unit → troubleshooting (Docker down, missing key, port in use).
- **Verify:** a reader following only this doc reaches a dispatched demo unit. Cross-check every command
  against the real `package.json` scripts and server endpoints.
- **Notes:** Reference the verified facts from this doc (image exists; demo mode from Lane L2 — note it
  as "after L2 merges"); don't document endpoints that don't exist in `server.rs`.

---

## Rules of the road (every dispatched agent)

1. **Stay in your lane** — write only your Owns paths; never edit another lane's files (esp. the
   `cockpit/ui/src-tauri/` file-split between L1 and L3). Need a change elsewhere → contract request.
2. **Branch/worktree per lane**, off `main`; never commit to `main`.
3. **Don't widen scope** — build only your item; report anything else you find.
4. **Verify before done** — run your Verify check, paste real output.
5. **Report for integration** — files changed, contract requests, verify output, cross-lane notes.

## Integration plan

1. Lanes merge in **any order** — zero owned-file overlap by construction.
2. L4 consumes L3's documented secret names; if L3 lands first the names are canonical, if not L4
   references the agreed list (no merge dependency — it's a string contract).
3. **Reconcile:** on the merged tree run `cargo test --workspace`, `npm run tauri build` (unsigned),
   and `npm run tauri dev` (confirm L1's sidecar supervision + L2's demo mode coexist).

---

## Serial track — NOT swarmable (orchestrator / human)

These are single-threaded, exploratory, or credential-gated — forcing them into lanes buys nothing.

- **S1 — Agent image:** ✅ **done** (`cc-agent:dev` present). *Optional hardening:* pin by digest +
  a `deploy/agent-image/build.ps1`; small enough to fold into a follow-up.
- **S2 — Non-model e2e proof:** ✅ **done this session.** Both ignored Docker ITs pass; real mergeable
  PR opened (`command-center-agent-sandbox#4`). The spine's Docker→git→checks→GitHub path is proven.
- **S3 — Live paid model run:** **needs you.** A real T1 mission with `ANTHROPIC_API_KEY` set, through
  the full oracle→build→review→PR loop on a throwaway repo. Costs real tokens; must be human-watched
  the first time. This is the last unproven slice of the spine.
- **S4 — Phase-0 webview embedding spike (app-plugins B5):** **orchestrator, hands-on.** The design's
  explicit go/no-go gate — enable Tauri `unstable`, prove a child webview renders a real app, hides
  under an overlay in <150ms with no flash, and tears down with no orphans; record the outcome in
  `spikes/SPIKE-RESULTS-app-plugins.md`. Single exploratory thread; it *gates* B6/B7, so it cannot be
  parallelized with them.
- **External — code-signing certs:** Apple Developer ID ($99/yr + notarization) and a Windows
  Authenticode cert. Procurement only you can do; L3 documents exactly what's needed.

## Blocked — unblock after S4 says "go"

- **App-plugins Phase-1:** make the spike's Tauri config permanent (`unstable` feature, exact pin,
  webview capabilities).
- **App-plugins Phase-6:** `embed.rs` + `plugin_show`/`plugin_hide`/`plugin_set_rect`, wire Audience
  end-to-end, run the smoke checklist. Becomes its own small serial build (or a 2-lane mini-swarm)
  once the spike de-risks it.
- **view-plugins:** designed/approved (`feat/view-plugins`); an alternate skin, not a launch gate.
