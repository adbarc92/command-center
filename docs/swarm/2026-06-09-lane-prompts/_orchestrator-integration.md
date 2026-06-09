# Orchestrator — Step 0, Lane Z, and Integration

> This file is **for you (the orchestrator)**, not for a dispatched agent. It holds the parts of the
> swarm that are **not** parallel: the prerequisite PR, the two global-file writes (Lane Z), and the
> merge/reconcile order. Run Step 0 before dispatching; run Integration after the lanes return.

---

## Step 0 — Prerequisite (do BEFORE dispatching any lane)

Every lane reads `docs/ROADMAP.md`, which currently exists **only** on `docs/roadmap-and-swarm`, not
`main`. Land the roadmap docs on `main` first so lanes can branch off `main` and read them.

- Open a PR: `docs/roadmap-and-swarm` → `main` (per the no-direct-push-to-main rule). Include
  `ROADMAP.md`, `SWARM-HANDOFF.md`, and this `docs/swarm/2026-06-09-lane-prompts/` directory.
- Merge it. **Now** lanes A1/A2/B/C/D branch off `main`.

(If you'd rather not merge yet, lanes may branch off `docs/roadmap-and-swarm` directly — but the
chosen plan is merge-first for a clean canonical base.)

---

## Dispatch — 5 concurrent lanes

Paste each lane file verbatim into its own isolated-worktree agent, **all at once**:

| Lane | File | Branch |
|---|---|---|
| A1 | `lane-A1-cache-timer.md` | `feat/cache-timer` |
| A2 | `lane-A2-rate-limit-retry.md` | `feat/agent-rate-limit-retry` |
| B | `lane-B-budget-discipline.md` | `feat/budget-discipline` |
| C | `lane-C-context-offload.md` | `feat/context-offload-t1` |
| D | `lane-D-dashboard-spec.md` | `feat/dashboard-spec` |

These have **zero owned-file overlap** by construction. Each agent files contract requests for the
two global files back to you (Lane Z).

---

## Lane Z — Globals owner (YOU apply these, last)

Lane Z is **not dispatched** — you are Z. You are the **single writer** of both global,
non-worktree-isolated, no-rollback files. Collect every lane's contract requests, then apply
**additively, in one pass each**:

### `~/.claude/settings.json`
Assemble the union of requested hook/env entries:

- **From A1:** a `Stop` hook + a `UserPromptSubmit` hook (cache timer) — exact commands/paths A1 produced.
- **From A2:** a rate-limit env/setting — *only if* A2's spike found that to be the mechanism.
- **From B:** a checkpoint hook — *only if* 6D became a hook.
- **From C:** a `SessionStart` hook — *only if* C added one.

Rules: **additive only** — never rewrite unrelated keys. If two lanes request the same event,
reconcile into a single hook list. Confirm the file still parses and every referenced script path exists.

### `~/.claude/CLAUDE.md`
Append **Lane B's PROPOSED CLAUDE.md BLOCK** (delivered inside `docs/playbooks/budget-discipline.md`)
verbatim. Additive only — do not disturb existing rules (UV, git workflow, etc.).

---

## Integration plan

1. **Step 0 PR merges** → lanes branched off `main`.
2. **Merge the repo-file lanes in any order** — no overlap:
   - A1 → `tools/cache-countdown/**`
   - A2 → `tools/rate-limit-retry/**`
   - B → `docs/playbooks/budget-discipline.md`
   - C → `docs/playbooks/context-offload.md`, `tools/context-offload/**`
   - D → `docs/superpowers/specs/2026-06-09-project-dashboard-design.md`
3. **Apply the two global files last (Lane Z, above):** `settings.json` ← union of hook/env requests;
   `CLAUDE.md` ← B's proposed block.
4. **Reconcile:**
   - `settings.json` parses; every hook points at a script that exists in the merged tree.
   - A fresh session exercises the **cache timer (A1)**, a **simulated 429 retry (A2)**, the **budget
     rules (B)**, and **memory recall (C)** together, without conflict.
   - Lane D's spec exists with a 3-round critique log and no `TBD`s.

---

## After the swarm — still blocked

- **6B** (ContextCurator integration) — revisit when the user's ContextCurator ships.
- **3 Tier 2** (Claude.ai Project KB) — revisit when the connector is reliably available.
- **4 build** (dashboard) — unblocked once Lane D's spec is approved + a Halyard head exists.
- **app-plugins** — separate serial track, gated on the Phase-0 webview spike.

> Note: a separate agent is building **swarm-dispatch automation** (roadmap Item 2) in
> `.claude/worktrees/feat+swarm-dispatch` (`docs/superpowers/specs/2026-06-09-swarm-dispatch-design.md`).
> Once that lands, this whole hand-built dispatch can be fired by that capability instead.
