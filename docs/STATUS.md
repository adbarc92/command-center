---
stage: Build
readiness: "~85% to feature-complete"
updated: "2026-07-16"
name: "Command Center"
base_branch: "main"
test_cmd: "cargo test --workspace"
---

# Command Center — Status

_Canonical, living status. The **State summary** below is rewritten in place each session; the
**Session log** is appended newest-first. Supersedes the scattered `docs/handoff/*` snapshots (those
are frozen history). The front-matter above is the Local-Tracker U1 stage marker — the cockpit reads
its own `STATUS.md`, so the Command Center appears on its own board as a `local:` card._

## State summary

**TL;DR.** The **workflow layer is essentially shipped** (cache timer, rate-limit retry, budget
rules, and the whole session-state plugin — hardening H1–H4 all merged). The **product layer** is
mid-stride: the **Project Dashboard + Local-Tracker Phase 1 shipped (PR #36)**; two things are in
flight — the **cockpit design overhaul** (blocked on Claude Design output) and the **app/view-plugin
embedding spikes** (P3 effectively done, **P4 one debugging session from a verdict**).

**Vision (sharpened 2026-07-16):** the Command Center is the operator's **one-stop shop for agentic
engineering** — dispatch work, see every project's stage, act without alt-tabbing, host the other
tools inside it, and (future) **remote-control** it from away-from-desk. **Feature-complete before
launch.**

**Locked build order (auth-foundation-first):**
1. **Local-Tracker Phase 2 dispatch** — the keystone (viewer → command surface) + the loopback-auth
   foundation Remote Control reuses. Specced (Option A), 3 critique rounds, **not built**.
2. **Resolve P4 → dispatch the app-plugin + view-plugin embedding swarms.**
3. **Design overhaul** (needs Claude Design output).
4. **Remote Control** — named pillar; brainstorm→spec after Phase-2 auth lands.

**Open PRs.**
- **#37** (`docs/tracker-spec-and-hygiene` → main) — this session's docs reconcile + gitignore
  hygiene. Docs only.
- **#35** (`feat/view-plugin-bridge-handshake`, **draft**) — stale since 2026-06-30, overlaps the P4
  spike. **Decision pending:** close as superseded (recommended) or revive.

**Known gaps / blockers.**
- **P4 spike unresolved** — first watched run dropped all 100 handshakes; leading hypothesis (module
  scripts fetched CORS-mode, `sdk.js` never runs) + cheap fix (`Access-Control-Allow-Origin: *`)
  documented but unverified. Blocks the view-plugin runtime swarm.
- **P3 go/no-go never written up** — spike effectively done; `spikes/SPIKE-RESULTS.md` still blank.
- **Launch gates (all out-of-repo, yours):** code-signing certs (Apple $99/yr + Windows
  Authenticode), one signed release run, one live paid T1 mission (S3). No release tag exists yet.
- **CI billing** was blocking runner allocation; #36 merged 2026-07-12 *suggests* it's resolved —
  verify with a fresh `gh run` before trusting green CI.
- **Installed session-state plugin is v0.1.0** (pre-H1–H4); fixes on `main`, not re-released.
- **Untracked config awaiting a decision:** `.claude/settings.json`, `cockpit/ui/src-tauri/.taurignore`.

**Next steps.**
1. Decide PR #35's fate; merge #37 when reviewed.
2. **Re-run the P4 watched spike**, confirm the CORS fix, record 100/100 → write P3 + P4 verdicts to
   `SPIKE-RESULTS.md`.
3. Start **Local-Tracker Phase 2** (writing-plans → build) — keystone + auth foundation.
4. When ready, dispatch the embedding swarms; land the design overhaul; then spec Remote Control.

## Session log

### 2026-07-16 — Work-audit, vision reframe, repo hygiene
- Ran a full work-audit; found the local clone was **9 commits stale** (no fetch since 2026-06-25) —
  #34 + #36 were merged on GitHub but absent locally. Fetched + fast-forwarded `main`.
- **Vision sharpened** with the operator to "one-stop shop for agentic engineering," **feature-complete
  before launch**, with **Remote Control** as a new future pillar. Locked the auth-foundation-first
  build order (Phase-2 dispatch → P4/embedding → design → remote).
- **Reshaped `ROADMAP.md`** to reflect reality (#36 shipped, hardening H1–H4 all merged, P3/P4 real
  status) + added the Remote Control pillar and build order. **Created this `docs/STATUS.md`.**
- **Repo hygiene (PR #37):** discarded a redundant working-tree H4 draft (superseded by #34's better
  self-healing fix); moved P4 diag instrumentation to `spike/view-plugins-handshake`; tracked the
  design-overhaul + handoff docs; folded the P3 hide/show finding into the app-plugins spec;
  gitignore'd the real runtime-SQLite path, `.claude/worktrees`, and `.context-curator/`; retired the
  merged `local-tracker` worktree + branch.
