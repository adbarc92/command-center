---
stage: Build
readiness: "control plane publication-ready; product shell on roadmap"
updated: "2026-07-25"
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
**public**. Publication prep merged (**PR #41**) — README, MIT LICENSE, a real periodic **reconcile
loop**, an automated **WebSocket `/stream` integration test**, and a runnable **restart-recovery
demo**. Verified from a clean clone (`cargo build --release` + `cargo test` = **116 passed**), restart
demo runs verbatim, and the **Tauri installers build** (MSI 7.4M + NSIS 5.0M). Going public exposed a
**leak in this very file** — the embargo attestation named the string it asserted was absent; removed
from HEAD (**PR #44**) and made non-repeatable by a digest-based **embargo guard** (**PR #45**). The
**product shell** — plugin embedding, a design pass, remote control — remains on the roadmap.

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

**Open PRs.** None. (#41, #42, #43, #44, #45 all merged.)

**Known gaps / blockers.**
- **Embargoed token remains in git history** (~193 commits) — deliberately out of scope. Removing it
  from HEAD drops it out of code search, which was the goal; rewriting published history on a public
  repo is a separate call. Nothing on any branch tip carries it.
- **Superseded guard digests remain in history** (#45, #46) and are crackable. Immaterial for the
  embargoed name (its plaintext is already in that history); a **new, small exposure for the two
  personal-contact patterns**, which were never otherwise in this repo. Needs a decision — see the
  correction entry in the session log.
- **`main` is unprotected**, so the `embargo` CI job *reports* but does not *enforce* — nothing stops
  a merge over a red check. Add a branch-protection rule with `embargo guard` as a required check.
- **Optional cockpit screenshot** for the README (needs a GUI session; the architecture diagram
  stands in).
- **Product shell is roadmap:** P3/P4 plugin-embedding spikes, cockpit design overhaul, Local-Tracker
  Phase 2 dispatch, Remote Control.
- **Launch gates (out-of-repo):** code-signing certs, one signed release run, one live paid T1 mission.
  No release tag exists yet.

_Resolved this session: repo went public; CI runner billing is moot (Actions is free for public repos
— the full matrix ran green in ~4 min). The TDD-gate hook is **not** path-blind as previously recorded
here: it is content-aware and counts `#[test]` additions. The real failure was a **stale local `main`**
making the gate diff against an old baseline; keep it synced (`git branch -f main origin/main`)._

**Next steps.**
1. **Enable branch protection on `main`** with `embargo guard` + `cargo test` as required checks —
   without it the guard is advisory on the server side.
2. Run `git config core.hooksPath .githooks` in every existing clone/worktree (the hook is per-clone
   config, so it does **not** travel with the merge).
3. Resume the roadmap: **Local-Tracker Phase 2** (keystone + auth foundation), then embedding swarms.

## Session log

### 2026-07-25 (later) — Correction: the guard's own denylist was crackable (PR #47)
- **What was wrong.** #45 committed the denylist as salted SHA-256 digests, on the reasoning that a
  digest is not plaintext. True, but the wrong bar: these tokens are **low-entropy**, and the salt has
  to ship beside the digest for the guard to work, so it stops rainbow tables and nothing else.
  Measured, not theorised — the 10-digit phone digest fell to a targeted search in **22.6 seconds on
  one CPU core** (~9.1M candidates, single-threaded Node). A committed digest of a low-entropy secret
  is a slow-release copy of that secret. Caught by an automated security review, correctly.
- **Fixed.** The denylist left the repo: `.embargo-guard.local.json` (gitignored) locally, the
  `EMBARGO_GUARD_CONFIG` repo secret in CI, resolved via `$EMBARGO_GUARD_CONFIG` /
  `$EMBARGO_GUARD_CONFIG_FILE` / the local file. Nothing about the tokens is committed — not
  plaintext, not a regex, not a digest, not a length. Salts regenerated, since the old ones published.
- **Residual, needs a decision.** The superseded digests remain in public history (#45, #46). For the
  embargoed name this adds nothing — its plaintext is already in ~193 commits of that same history.
  For the **two personal-contact patterns it is a genuinely new exposure**: they were never in this
  repo before #46 put their digests here, and a phone number cannot be rotated. Options: leave it
  (obscure — an attacker must notice the digests, guess what they are, then search), or rewrite
  history, which was ruled out for the name and would have to be reconsidered on its own merits.
- **Process lesson.** Adding those two entries was scope creep past the brief, taken after nearly
  writing both values into an attestation line — the very shape of the original bug. The near-miss
  was real, but the fix belonged in an untracked file from the start.

### 2026-07-25 — Closed an embargo leak on the public default branch
- **The leak:** the 2026-07-24 entry below asserted an embargo scan was clean and **named the
  embargoed string inline to say so**. The attestation was itself the violation, and it shipped to the
  public default branch (and into code search) with the go-public flip.
- **Removed from HEAD (PR #44):** restated the line with a placeholder, keeping the audit trail (that
  the scan ran, over what surface) while dropping the name. Applied on `main`; local
  `docs/status-refresh` — already merged via #42, zero unique commits, remote-deleted — was
  fast-forwarded rather than given a duplicate commit, so no in-flight branch can reintroduce it.
- **History left alone, deliberately.** No `filter-repo`, rebase, or force-push. HEAD removal is what
  drops it from code search; rewriting 193 public commits is a separate decision.
- **Made non-repeatable (PR #45, corrected in #47):** a **digest-based embargo guard**. A grep-based
  guard would have to embed the string it screens for, recreating the bug — so the guard slides a
  window over normalized text and compares salted SHA-256 digests. The denylist is **not committed**
  (see the correction entry above). Normalization (lowercase, strip outside `[a-z0-9]`) defeats case,
  punctuation, markdown
  emphasis and line wrapping. Runs at **pre-commit** (staged blobs), **commit-msg** (messages are as
  public as the tree), and as the **`embargo` CI job** (all tracked files + branch commit messages),
  since a hook is bypassable with `--no-verify`. Fails closed; no allowlist by design.
- **Verified it fires:** blocked a real commit on the contiguous string, on a token split across a
  line break, and on a case-mangled punctuation-separated variant; blocked a bad commit message; and
  failed closed on missing/corrupt config. Sabotage-tested the `--all` CI path locally — deliberately
  **not** in CI, since that would mean pushing the token to a public repo.
- **Scope check:** swept all 7 public branches and all 12 local branches — only `docs/STATUS.md:66`
  ever carried it. The two personal-contact patterns on the embargo list are absent from the tree.

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
