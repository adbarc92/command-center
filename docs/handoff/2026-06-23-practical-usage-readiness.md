# Handoff — Practical-usage readiness (2026-06-23)

> ⚠️ **SUPERSEDED (2026-06-25)** — status here is stale (P4 is now scaffolded & committed; the P3
> `spike_show` hang is fixed & committed). Current readiness audit + swarm plan:
> **[`2026-06-25-ship-readiness-swarm-handoff.md`](2026-06-25-ship-readiness-swarm-handoff.md)**. Kept for history.

**Branch:** `main` @ `b35c9e2` (everything below is merged; nothing in flight).
**Question this answers:** how far is the Command Center from **practical day-to-day usage** by its owner?
**Verdict:** **~85% — the gap is human-gated validation + procurement, not engineering.** The swarmable
build backlog is cleared; what remains is mostly things an agent *cannot* do alone.

> This is a project-readiness handoff, not a mid-task pickup — there is no half-done work to resume.
> It exists so the next session can decide *what to do next* without re-deriving project state.
> Canonical live list of blockers: [`docs/ROADMAP.md` → "⚠️ Requires your attention"](../ROADMAP.md).

## State of the build (what exists and runs)

- **Rust engine** — `crates/fleet-core` + `crates/fleetd`: multi-agent mission dispatch, cost/budget
  cap (`--max-budget-usd`), rate-limit retry, swarm-decomposition engine (`/swarms`), sqlite store.
  `cargo test --workspace` = **104 green** (3 real-Docker ITs stay `#[ignore]`d — need a dind runner).
- **Tauri cockpit** — `cockpit/ui` (+ `src-tauri`): fleetd sidecar supervisor (health-gate/restart/
  clean-shutdown), app-plugin lifecycle manager, project dashboard (stage inference + halyard/Audience
  adapters), approval overlay, plugin switcher. Svelte suite green (~72). Builds end-to-end — verified
  this session: `npm ci → npm run sidecar → npm run tauri build` produced MSI + NSIS bundles, exit 0.
- **Context-hygiene workflow layer** (North Star "low cost" pillar) — shipped: cache-timer, rate-limit
  retry, budget-discipline rules, Tier-1 context offload (MEMORY.md + the session-state plugin, now
  hardened — PRs #30/#31 this session).

So the *engine and the host shell are built and test-green*. What's missing is the **proof that the
two plugin-embedding features work in a real window**, plus the **signed-release procurement**.

## The gap to practical usage — ranked by what it unblocks

| # | Blocker | Type | Unblocks |
|---|---|---|---|
| **P3** | App-plugin **webview spike** (gates 2–5). Harness exists; `spike_show` currently **hangs** — hypothesised main-thread deadlock from a sync command calling child-webview create. Debug it, then visually confirm renders / resize ≤150ms / no orphan on quit; record go/no-go. | 🔴 human-gated (visual judgment) | App-plugin **embedding** build (Phase 6, ~2 days, pre-carved) |
| **P4** | View-plugin **handshake spike**. Design complete, harness **not yet scaffolded**. Prove sandboxed-iframe + MessagePort `plugin-hello→init` across **100 reloads, zero drops**, dev **and** packaged. | 🟠 human-gated (watched run) | View-plugin **runtime** build (~3 days, designed) |
| **S3** | One **live paid T1 mission**. Set `ANTHROPIC_API_KEY`; watched oracle→build→review→PR on a throwaway repo. Validates the end-to-end spine on real tokens (not a build gate). | 🟠 human-gated + spend | Confidence in the autonomous spine |
| **Certs** | **Code-signing certs** — Apple Developer ID ($99/yr + notarization) + Windows Authenticode. Wiring & secret names already done in `release.yml` / `tauri.conf.json`. | 🟣 procurement (out of repo) | The **signed cross-platform release** run |

**Behind the spikes, the building is pre-carved and dispatch-ready** (≈8 serial days, or a 3-lane
swarm): app-plugin embedding (Rust webview commands + Svelte switcher mount) ∥ view-plugin runtime
(bridge + SDK + reference plugin), then an integration lane that mounts both to the one switcher.
Sources: [`docs/handoff/2026-06-12-remaining-work-handoff.md`](2026-06-12-remaining-work-handoff.md)
(Parts A–C), [`docs/handoff/2026-06-15-P3-spike-resume.md`](2026-06-15-P3-spike-resume.md) (the
`spike_show` bug + gate criteria), and the app-plugins / view-plugins design specs under
`docs/superpowers/specs/`.

## Infra caveat (not a code blocker, but it gates the green checkmark)

GitHub Actions is **red across every branch** — a **billing failure** ("recent account payments have
failed or your spending limit needs to be increased"): no runner is ever allocated (jobs die in ~3s,
`runner=0`, `steps=0`). The repo is private, so Actions consumes paid minutes. **Confirmed unrelated to
any code** — `cargo test --workspace` and the Tauri build both pass locally. Until billing is fixed,
every PR's checks stay red and merges need `--admin`. **Fix:** GitHub → Settings → Billing & plans,
then `gh run rerun <id>`. This is the single cheapest unblock for honest CI signal.

## What to pick up next (recommended order)

1. **Fix GitHub billing** (5 min, highest leverage) — restores real CI signal for everything below.
2. **P3 webview spike** — debug the `spike_show` hang, walk gates 2–5, record the embedding go/no-go.
   This unblocks the largest downstream build.
3. **P4 handshake spike** — scaffold the harness, run the 100-reload zero-drop check.
4. Once P3/P4 say "go": dispatch the **2–3 lane feature swarm** (designs are ready).
5. **S3** (anytime, needs a key + watched hour) and **Certs** (procurement, ~1 week lead) in parallel.

## Loose ends from this session (non-blocking)

- `~/.claude/settings.json.pre-sessionstate-migration.bak` still present — safe to delete when satisfied.
- One agent worktree dir (`.claude/worktrees/agent-a63522…`) was held by a lingering process at cleanup;
  git no longer tracks it (pruned) — it'll clear on its own, or `rm -rf` it once the handle releases.
- H1 semver fix (PR #31) **cleared the release gate** — a `0.10.x` session-state plugin release is now safe.

## Commands worth remembering

- Rust gate: `cargo test --workspace` · Plugin suite: `node --test "plugins/session-state/test/*.test.mjs"`
- Cockpit build: `cd cockpit/ui && npm ci && npm run sidecar && npm run tauri build`
- Cockpit dev: `cd cockpit/ui && npm run desktop` (sidecar + `tauri dev`, connects to fleetd on :8787)
