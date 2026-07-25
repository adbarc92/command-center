---
stage: Build
readiness: "control plane publication-ready; product shell on roadmap"
updated: "2026-07-24"
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

**TL;DR.** The **control plane and workflow layer are feature-complete and tested**, and the repo is
now **publication-ready**: publication prep merged (**PR #41**) — README, MIT LICENSE, a real periodic
**reconcile loop**, an automated **WebSocket `/stream` integration test**, and a runnable
**restart-recovery demo**. Verified from a clean clone (`cargo build --release` + `cargo test` =
**116 passed**), restart demo runs verbatim, and the **Tauri installers build** (MSI 7.4M + NSIS 5.0M).
The **product shell** — plugin embedding, a design pass, remote control — remains on the roadmap. The
repo is still **private** pending the go-public decision.

**Vision (unchanged):** the Command Center is the operator's **one-stop shop for agentic
engineering** — dispatch work, see every project's stage, act without alt-tabbing, host the other
tools inside it, and (future) **remote-control** it from away-from-desk. **Feature-complete before
launch.**

**Locked build order (auth-foundation-first):**
1. **Local-Tracker Phase 2 dispatch** — the keystone (viewer → command surface) + the loopback-auth
   foundation Remote Control reuses. Specced (Option A), **not built**.
2. **Resolve P4 → dispatch the app-plugin + view-plugin embedding swarms.**
3. **Design overhaul** (needs Claude Design output).
4. **Remote Control** — brainstorm→spec after Phase-2 auth lands.

**Open PRs.**
- **`feat/oracle-hash-persist` → main** — the two orphaned oracle-hash follow-ups (persist +
  reload-on-resume so the tamper gate re-arms), rebased onto main; +4 tests (120 total green).

**Known gaps / blockers.**
- **Repo is private** — flip to public when ready (deliberate; not yet done).
- **CI runner allocation** still gated on GitHub Actions billing (out-of-repo).
- **Local TDD-gate hook false-positives** on inline-test Rust PRs (it detects tests by file path, so
  it can't see `#[cfg(test)]` modules) — set `requireTestChanges: false` (or a content-based check) in
  `.claude/tdd-gate.json`.
- **Optional cockpit screenshot** for the README (needs a GUI session; the architecture diagram
  stands in).
- **Product shell is roadmap:** P3/P4 plugin-embedding spikes, cockpit design overhaul, Local-Tracker
  Phase 2 dispatch, Remote Control.
- **Launch gates (out-of-repo):** code-signing certs, one signed release run, one live paid T1 mission.
  No release tag exists yet.

**Next steps.**
1. Merge the oracle-hash follow-up PR (`feat/oracle-hash-persist`).
2. **Flip the repo public** when ready.
3. Fix the TDD-gate config so inline-test Rust PRs stop false-flagging.
4. Resume the roadmap: **Local-Tracker Phase 2** (keystone + auth foundation), then embedding swarms.

## Session log

### 2026-07-24 — Publication prep (control plane → public-ready)
- **Audit + safety:** full work-audit; **secrets scan clean** (tree + full history — `.env` never
  tracked, live key absent); **embargoed-name scan clean** (tree, history, commit messages).
- **Closed the two soft resume claims (Red→Green):** added a genuine periodic **reconcile loop**
  (`reconcile_live` spares live-driver units + `reconcile_tick`, `CC_RECONCILE_SECS` default 30s) so
  "reconcile loop" is literally accurate, and an automated **WebSocket `/stream` integration test**
  (real `tokio-tungstenite` client, proven non-vacuous via a sabotage run).
- **Docs:** wrote the **README** (architecture + executed quickstart + honest roadmap), added
  **MIT LICENSE**, and a runnable **restart-recovery demo** (`scripts/demo-restart-recovery.mjs`).
- **Verified from a clean clone:** `cargo build --release` + `cargo test --workspace` = **116 passed**;
  restart demo verbatim; **Tauri installers built** (MSI + NSIS). Set repo description + 10 topics.
  Merged as **PR #41**.
- **Rescued orphaned work:** the two unmerged oracle-hash commits (persist + reload) had lost their
  remote when `feat/oracle-freeze` was pruned; rebased onto main and put up from
  `feat/oracle-hash-persist` (+4 tests, 120 total green).
- Repo left **private** pending the go-public decision.

### 2026-07-16 — Work-audit, vision reframe, repo hygiene
- Ran a full work-audit; found the local clone was **9 commits stale** — #34 + #36 merged on GitHub but
  absent locally. Fetched + fast-forwarded `main`.
- **Vision sharpened** to "one-stop shop for agentic engineering," **feature-complete before launch**,
  with **Remote Control** as a new future pillar. Locked the auth-foundation-first build order.
- **Reshaped `ROADMAP.md`** to reflect reality; **created this `docs/STATUS.md`.**
- **Repo hygiene (PR #37):** discarded a redundant working-tree H4 draft; moved P4 diag instrumentation
  to `spike/view-plugins-handshake`; tracked design/handoff docs; gitignore hygiene; retired the merged
  `local-tracker` worktree + branch.
