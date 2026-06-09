# Lane C — Context offload, Tier 1

> Paste this entire file as the prompt for a single agent. It is self-contained. Roadmap item:
> **3 (Tier 1 only)** — local Claude Code memory + ContextCurator recall/offload.

## Your worktree (set up first)

```bash
git worktree add .claude/worktrees/feat+context-offload-t1 -b feat/context-offload-t1 main
cd .claude/worktrees/feat+context-offload-t1
```

(If your harness creates the worktree for you, just confirm you are on `feat/context-offload-t1`, not `main`.)

## Goal

Automate **recall** of durable project facts at session start, and **offload** of new durable facts
during/after work, so stable context isn't re-derived at token cost. **Tier 1 only.**

## Scope boundaries (read carefully)

- **Tier 2 is OUT of scope** — the Claude.ai Project knowledge base is deferred (connector
  reliability). Do not design or build it here.
- **You use ContextCurator AS-IS.** Building/integrating ContextCurator (roadmap 6B) is a **separate,
  blocked** item — it is the **user's own product**, not ours to build. Use its existing `cc_*` MCP
  tools as they are; do not reimplement eviction/pinning.

## Owns (exclusive write)

- `docs/playbooks/context-offload.md` — the Tier-1 automation design + memory-write discipline
  (when to recall, when to offload, what counts as a durable fact). Create `docs/playbooks/` if absent.
- `tools/context-offload/**` — any helper script you add.

## Reads (no write)

- The project memory store: `~/.claude/projects/<project>/memory/` (`MEMORY.md` index + note files).
  Treat the store as **append-only** — written by normal operation, not by you wholesale.
- The ContextCurator MCP surface (`cc_*` tools).
- [`docs/ROADMAP.md`](../../ROADMAP.md) §3.

## Shared contract

- If your design adds a **`SessionStart` hook** (to auto-recall at session start), file a
  **`settings.json` contract request to Lane Z** (exact event + command). Do not edit `settings.json`.

## Done when

- Durable facts are **recalled at session start without manual prompting**.
- New durable facts are **written to memory automatically at natural boundaries** (phase/spike end).

## Verify (run, paste real output)

- Start a fresh session (or simulate the SessionStart path) → confirm relevant prior facts surface
  **without the user re-stating them**. Show the recalled facts.

## Notes / open questions

- Distinguish clearly: **memory** = agent-derived durable facts (decisions, gotchas, "why");
  **ContextCurator** = in-window eviction/pinning. They're complementary tiers — say which does what.
- Keep it **headless-safe**: the design must degrade gracefully if ContextCurator's MCP is absent
  (fall back to the local memory store). Never block on a tool that may not be connected.

---

## Rules of the Road (follow exactly)

1. **Stay in your lane.** Write only files under **Owns**. Never edit `~/.claude/settings.json` or
   another lane's files — file a contract request to Z instead. Don't rebuild ContextCurator.
2. **Worktree per lane.** Work on `feat/context-offload-t1`; never commit to `main`.
3. **Global/shared files are append-only + single-owner.** You own none — request entries.
4. **Don't widen scope.** Tier 1 only; no Tier 2, no building 6B.
5. **Verify before done.** Run the Verify check; paste the real recalled-facts output.
6. **Report for integration.** End with: files changed; any `settings.json` contract request to Z;
   your verify output; anything affecting another lane.
