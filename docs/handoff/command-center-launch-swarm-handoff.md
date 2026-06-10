# Handoff — Dispatch & manage the Command Center launch-readiness swarm

**For:** a fresh agent taking over orchestration.
**Your job:** dispatch the **5 launch-readiness lanes**, collect their work, open PRs, and run the
integration/reconcile. You are the orchestrator — you do NOT write lane code yourself; you fan it out
and integrate.

**Repo:** `d:/MajorProjects/CURRENT/command-center` (Windows, PowerShell + Bash available).
**Date context:** planned 2026-06-09.

---

## 1. The single source of truth — read this first

The full carve (lane scopes, exclusive file ownership, shared contracts, rules of the road, the
integration + reconcile plan) lives in:

- **`docs/swarm/2026-06-09-launch-readiness.md`** — **merged to `main`** (PR #13, merged 2026-06-09).
  Read it directly from your checkout; no prerequisite merge needed.

Lanes branch off `main`, which already contains the doc. (The lanes don't need to read the doc —
*you* do.)

Do not re-derive the carve. It already passed the swarm-handoff method. Your value is in execution.

## 2. Verified context (already done — do NOT redo)

- ✅ `cc-agent:dev` Docker image exists; Docker 28.3.3 healthy.
- ✅ Non-model spine proof passed: both `--ignored` Docker ITs (`local_docker_it`, `preflight_it`)
  green; real mergeable PR at `adbarc92/command-center-agent-sandbox#4`.
- ✅ App-plugins Phase-0 spike **Gate 1 PASS** — Tauri `unstable` builds (branch
  `spike/app-plugins-webview`, local/throwaway; see `spikes/SPIKE-RESULTS-app-plugins.md`).
- ✅ The agent-operation roadmap swarm is **merged**: PRs **#7–#12** (separate effort; don't conflate).

**Critical build-ordering gotcha (from the spike):** a bare `cargo build` in `cockpit/ui/src-tauri/`
FAILS until the sidecar binary exists (`externalBin` is checked at build time). Always run
`node cockpit/ui/scripts/build-sidecar.mjs` (or `npm run sidecar`) BEFORE any `cargo build` /
`tauri build` / `tauri dev`. Make sure lanes L1 and L4 honor this.

## 3. The 5 lanes (zero owned-file overlap by construction)

Full briefs are in the swarm doc §"The lanes". Summary + suggested branch names:

| Lane | Branch | Owns (exclusive write) |
|---|---|---|
| L1 Tauri sidecar supervisor | `feat/tauri-sidecar` | `cockpit/ui/src-tauri/src/{main,lib}.rs`, `Cargo.toml`, `capabilities/default.json` |
| L2 Demo-mode FakeRunner | `feat/demo-mode` | `crates/fleetd/src/{server,driver}.rs`, new `crates/fleetd/tests/demo_mode_it.rs` |
| L3 Bundle signing + updater | `feat/bundle-signing` | `cockpit/ui/src-tauri/tauri.conf.json`, `docs/release/signing-and-updates.md` |
| L4 Release CI/CD | `feat/release-ci` | `.github/workflows/**` |
| L5 Day-1 quick-start | `docs/quickstart` | `docs/quickstart.md` |

**The one shared contract:** L4 (CI) references L3's documented secret names (`APPLE_CERTIFICATE`,
`TAURI_SIGNING_PRIVATE_KEY`, etc.). No merge dependency — it's a string contract; L3's
`docs/release/signing-and-updates.md` is the canonical list. No out-of-repo global files this time,
so **no Lane Z / no global-config step** (unlike the roadmap swarm).

## 4. How to dispatch (the procedure that worked last time)

For **each** lane, do NOT rely on the Agent tool's auto-worktree (it branches off the wrong base).
Instead pre-create the worktree off `main`, then dispatch one agent per lane with a header that pins
it to that worktree + the verbatim lane brief from the swarm doc + the rules of the road.

1. Pre-create worktrees sequentially (avoids a parallel `git worktree add` lock race):
   ```bash
   git branch -f main origin/main        # ensure local main is current (PR #13 already merged)
   git worktree add .claude/worktrees/feat+tauri-sidecar  -b feat/tauri-sidecar  main
   git worktree add .claude/worktrees/feat+demo-mode      -b feat/demo-mode      main
   git worktree add .claude/worktrees/feat+bundle-signing -b feat/bundle-signing main
   git worktree add .claude/worktrees/feat+release-ci     -b feat/release-ci     main
   git worktree add .claude/worktrees/docs+quickstart     -b docs/quickstart     main
   ```
2. Dispatch all 5 agents **in one message** (parallel). Each prompt = this header, then the lane's
   brief copied verbatim from `docs/swarm/2026-06-09-launch-readiness.md`, then the rules of the road:

   > You are dispatched as **Lane <X>** of the launch-readiness swarm. Your worktree ALREADY EXISTS at
   > `D:/MajorProjects/CURRENT/command-center/.claude/worktrees/<dir>` on branch `<branch>` (off
   > `main`). FIRST: `cd` there. Skip any `git worktree add`. Never `cd` out, never edit the repo root
   > or other worktrees, never commit to `main`. **Build-ordering rule:** run `npm run sidecar` before
   > any `cargo build`/`tauri build`/`tauri dev`. When done, COMMIT on your branch (no push, no PR);
   > return a final report: files changed, contract requests, real verify output, cross-lane notes.
   > --- LANE BRIEF (follow exactly) --- <paste the lane's section + the doc's "Rules of the road"> ---

3. Dispatching a swarm is the expensive, opt-in step — confirm the user wants it fanned out before
   spawning all five.

## 5. Integration (you run this after lanes return)

Per the swarm doc §"Integration plan":

1. Open **5 separate PRs** to `main` (the user's established preference from the roadmap swarm — they
   chose separate over one combined). Push each branch, `gh pr create --base main`.
2. Lanes have zero overlap → mergeable in any order. Confirm L4 references L3's secret names.
3. **Reconcile on the merged tree:** `npm run sidecar` → `cargo test --workspace` →
   `npm run tauri build` (unsigned dev bundle should succeed) → `npm run tauri dev` (confirm L1's
   sidecar supervision and L2's demo mode coexist). Report real output.

## 6. Do NOT touch — serial / blocked / human-only

- **S3 live paid model run** — needs `ANTHROPIC_API_KEY` (the user provides it) + real tokens;
  human-watched. Not a lane.
- **S4 webview spike Gates 2–5** — needs Audience running + a watched GUI session (visual go/no-go).
  The user does this. Gate 1 is already cleared.
- **App-plugins Phase-1/Phase-6** — blocked until the S4 spike records a go.
- **Code-signing certs** — procurement only the user can do (L3 only documents what's needed).

## 7. User preferences (observed — follow them)

- Never push to `main` directly; always via PR. Feature branch per unit of work.
- **No** `Co-Authored-By` lines and **no** "Generated with Claude Code" footer in commits or PRs.
- Don't commit unless the work is the deliverable; don't bundle unrelated changes.
- Python tooling uses **UV** (no pip / requirements.txt).
- The user opts into expensive fan-outs explicitly — ask before spawning the swarm.

## 8. Housekeeping

- Existing worktrees from prior sessions are under `.claude/worktrees/` (roadmap lanes, lane-z,
  launch-swarm, spike). Prune merged ones with `git worktree remove`.
- The e2e test leaves sandbox PR #4 open by design; the user can close it.

---

## Suggested skills

- **`swarm-handoff`** — re-read the dispatch/rules-of-the-road/integration protocol before fanning out.
- **`dispatching-parallel-agents`** (or **`subagent-driven-development`**, or the `Workflow` tool) —
  the machinery to run the 5 lanes concurrently with worktree isolation.
- **`using-git-worktrees`** — for clean per-lane worktree setup/teardown.
- **`requesting-code-review`** / **`verification-before-completion`** — before declaring integration
  done; run the reconcile commands and report real output, not assertions.
- **`receiving-code-review`** — when handling each lane agent's contract requests/findings.
