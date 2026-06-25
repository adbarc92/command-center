# Handoff — Spikes, signed release, and the blocked feature swarms

> ⚠️ **SUPERSEDED (2026-06-25)** — the spike/cert status here is stale (both spikes are now committed &
> ready to run). Current human-gated work: **[`2026-06-25-spikes-handoff.md`](2026-06-25-spikes-handoff.md)**;
> ship plan: [`2026-06-25-ship-readiness-swarm-handoff.md`](2026-06-25-ship-readiness-swarm-handoff.md). Kept for history.

**For:** the next agent (or future-you), continuing from 2026-06-11.
**One-line state:** the swarmable-now work is **done and on `main`**; everything substantial that
remains is **human-gated** (two visual spikes, one paid mission, code-signing certs). This doc hands
off those gates + a short list of autonomous tidies that were offered but not yet done.

**Repo:** `d:/MajorProjects/CURRENT/command-center` (Windows; PowerShell + Bash). `main` is at
**`185ff20`** after this session.

---

## What landed this session (don't redo — see the PRs)

Executed the swarmable-now half of
[`2026-06-11-post-launch-swarm-handoff.md`](2026-06-11-post-launch-swarm-handoff.md) (its serial/
human-gated track and blocked feature swarms are **untouched** and still the source of truth for those):

- **PR #22 — Lane O (human-authority overlays).** `ApprovalOverlay.svelte` + store derivations
  (`awaitingApproval`, `pendingLaunch`/`requestRealLaunch`/`confirmLaunch`/`cancelLaunch`); oracle-
  approval + real-launch-confirm modals; `.hud` content subtree `inert` while open; verbs wired to
  `fleet.cmd(id, 'approve_oracle'|'reject_oracle')`. 17 new tests.
- **PR #23 — Lane P (packaging hardening).** Release-profile sidecar (`sidecar:release` / `--release`
  flag, debug stays default for dev); `tauri-plugin-updater` runtime activated (crate + JS + capability,
  additive in `lib.rs`); teardown log line documented as upstream/benign (not suppressed).
- **PR #24 — follow-ups.** `release.yml` now builds the sidecar via `npm run sidecar:release`;
  `docs/release/signing-and-updates.md` §3 un-staled (updater now wired); **`docs/ROADMAP.md` gained a
  top "⚠️ Requires your attention — human-gated" section** — that table is the canonical list of the
  blockers below.

Reconciliation before merge: merged O+P whole was green — `npm run check` 0 errors, `npm run test`
**72 passed** (55 original + 17 new). Worktrees cleaned; only `main` + `spike/app-plugins-webview`
remain.

---

## Two findings/corrections from this session (read before acting)

1. **`feat/view-plugins` holds NO unmerged work.** It is **0 ahead / 85 behind** `main`; its tip
   (`bf83d44`) is the *design doc*, already on `main`. The prior handoff's "unmerged, stale, de-stale
   it" framing (finding #2) is **moot** — there is nothing unique to preserve. When **P4 says go**,
   branch the view-plugin runtime work **fresh off `main`**; do not try to merge-forward this branch.
   (Left in place; deleting it is harmless but optional.)
2. **Cosmetic:** commit `78cf0b7` (the #24 follow-up) has stray `@` lines top/bottom — PowerShell
   here-string syntax leaked into a Bash commit. Content is intact and merged; not worth rewriting
   `main` history. Mentioned only so it isn't mistaken for corruption.

---

## What remains — all human-gated (you, not an agent)

These are mirrored in [`docs/ROADMAP.md`](../ROADMAP.md) → "⚠️ Requires your attention". An agent
**cannot** do them: each needs visual judgment, a real credential + spend, or out-of-repo procurement.

- **P3 — app-plugin webview spike, gates 2–5.** Bring Audience up (`D:/MajorProjects/CURRENT/audience`,
  dev, `:3000`); walk gates 2–5 on `spike/app-plugins-webview`; record go/no-go + the exact webview API
  to `spikes/SPIKE-RESULTS-app-plugins.md`. **Go → unblocks the app-plugin embedding swarm.**
- **P4 — view-plugin handshake spike.** Sandboxed-iframe + MessagePort handshake, 100 reloads zero
  drops, dev **and** packaged; record to `spikes/SPIKE-RESULTS.md`. **Go → unblocks the view-plugin
  runtime swarm** (then branch fresh off `main`, per finding #1).
- **S3 — one live paid T1 mission.** Set `ANTHROPIC_API_KEY`; dispatch a real T1 mission
  oracle→build→review→PR on a throwaway repo, human-watched. Last unproven slice of the SP1 spine.
- **Code-signing certs.** Apple Developer ID ($99/yr + notarization) + Windows Authenticode. Wiring +
  canonical secret names already done — `docs/release/signing-and-updates.md` §4; `release.yml`
  consumes them by name. **→ unblocks the signed cross-platform release run** (CI is otherwise ready).

---

## Autonomous tidies — offered this session, NOT yet done

The user chose to make this handoff instead of doing them. Any are safe to pick up:

- **A. Visually verify Lane O's overlay (highest value).** The modal/focus/`inert` behavior is the one
  thing tests can't prove. The **real-mode launch-confirm** path is easy to trigger with no live
  mission — launch the cockpit (`cd cockpit/ui && npm run desktop`, or demo mode per Lane E), select
  REAL in the new-mission form, confirm the modal captures focus and the content beneath is inert.
  Oracle-approval modal needs a unit driven to `awaiting_oracle_approval` (check whether demo mode
  reaches that state). Use the **`verify`** or **`run`** skill.
- **B. Delete 6 fully-merged local branches:** `feat/app-plugins`, `feat/lane-a-shell`,
  `feat/lane-b-tauri-host`, `feat/lane-c-ci`, `feat/lane-d-quickstart`, `feat/lane-e-demo-verify`.
  **Keep** `spike/app-plugins-webview` (holds the Gate-1 result) and `feat/view-plugins` (optional).
- **C. Delete the obsolete TEMP block in `~/.claude/CLAUDE.md`** ("resume tomorrow / delete once picked
  up") — it's now stale; this session is well past it.
- **D. Track `docs/handoff/`** — currently untracked (`git status` shows `?? docs/handoff/`). Commit
  these handoff docs if you want them versioned.

---

## Blocked feature swarms (only after the gating spike says "go")

Do **not** start before the gate. The design docs hold dispatch-ready detail — **reference, don't
re-derive** (this was the prior handoff's explicit instruction too):

- **App-plugin embedding** (after P3) — build order in
  `docs/superpowers/specs/2026-06-07-app-plugins-design.md` §6. Mounts under Lane O's overlay.
- **View-plugin runtime** (after P4) — build order in
  `docs/superpowers/specs/2026-06-07-view-plugins-design.md` §"Build order"; reuses Lane O's overlay.
  Branch fresh off `main` (finding #1), no de-stale needed.
- **SHELL** — extend the existing Switcher (`ViewEntry` supports a `badge`) to mount whichever runtime
  lands + register its commands.

---

## Suggested skills

- **`verify`** / **`run`** — for tidy A (drive the app, confirm the overlay).
- **`swarm-handoff`** — when a spike clears and you decompose the corresponding feature swarm.
- **`using-git-worktrees`** / **`subagent-driven-development`** — the fan-out machinery for those swarms.
- **`frontend-design`** — view-plugin runtime UI (matches the cockpit HUD aesthetic).
- **`verification-before-completion`** — run the real reconcile commands; report actual output.
