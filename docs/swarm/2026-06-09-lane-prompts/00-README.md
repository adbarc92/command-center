# Swarm Lane Prompts — Command Center Roadmap (2026-06-09)

Dispatch-ready prompts derived from [`docs/SWARM-HANDOFF.md`](../../SWARM-HANDOFF.md) (Part II) and
[`docs/ROADMAP.md`](../../ROADMAP.md), with the 2026-06-09 planning refinements applied:

- **Lane A split** into **A1 (cache timer)** and **A2 (rate-limit retry)** for an extra concurrent lane.
- **Lane Z absorbed both global files** — it owns *and* applies `~/.claude/settings.json` **and**
  `~/.claude/CLAUDE.md`, last, in one orchestrator-controlled step. No dispatched agent writes a global file.
- **Lane B routes its hook request to Z** (the earlier A-vs-Z contradiction is fixed).
- **Step 0 prerequisite added**: land the roadmap docs on `main` before any lane branches.

## Files

| File | Lane | Dispatch? |
|---|---|---|
| [`_orchestrator-integration.md`](_orchestrator-integration.md) | Step 0 + Lane Z + integration | **You** (orchestrator), not dispatched |
| [`lane-A1-cache-timer.md`](lane-A1-cache-timer.md) | A1 — cache-aware approval timer + pacing (items 1, 6E) | concurrent worktree |
| [`lane-A2-rate-limit-retry.md`](lane-A2-rate-limit-retry.md) | A2 — agent-level API 429 retry (item 5) | concurrent worktree |
| [`lane-B-budget-discipline.md`](lane-B-budget-discipline.md) | B — standing budget rules (6A/C/D/F/G) | concurrent worktree |
| [`lane-C-context-offload.md`](lane-C-context-offload.md) | C — context offload Tier 1 (item 3 T1) | concurrent worktree |
| [`lane-D-dashboard-spec.md`](lane-D-dashboard-spec.md) | D — project-dashboard spec (item 4, design only) | concurrent worktree |

## How to use

1. Run **Step 0** from `_orchestrator-integration.md` first (PR the roadmap docs to `main`).
2. Dispatch **A1, A2, B, C, D concurrently** — paste each lane file verbatim into its own agent.
   Each is self-contained (worktree setup + brief + Rules of the Road + verify all inline).
3. When lanes return, run the **integration plan** in `_orchestrator-integration.md` (merge repo
   lanes in any order, then *you* apply the two global files from collected contract requests).

Dispatching the swarm is the expensive/opt-in step — these files are the safe deliverable that
makes it firable cold.

## Not in this swarm (still blocked)

- **6B** ContextCurator integration — waits on the user's ContextCurator shipping.
- **3 Tier 2** Claude.ai Project KB — waits on connector reliability.
- **4 build** — needs Lane D's spec approved + a Halyard head.
- **app-plugins** — on its own serial track, gated on the Phase-0 webview spike.
