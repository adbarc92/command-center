# Lane B — Budget-discipline standing rules

> Paste this entire file as the prompt for a single agent. It is self-contained. Roadmap items:
> **6A** (exploration-via-subagent), **6C** (cheapest-capable-model routing), **6D** (checkpoint at
> boundaries), **6F** (workflow token budgets), **6G** (digest-first reading).

## Your worktree (set up first)

```bash
git worktree add .claude/worktrees/feat+budget-discipline -b feat/budget-discipline main
cd .claude/worktrees/feat+budget-discipline
```

(If your harness creates the worktree for you, just confirm you are on `feat/budget-discipline`, not `main`.)

## Goal

Turn capabilities the agent *already has* into **standing, automatic budget discipline** rather than
manual practice. Write each as an unambiguous standing instruction with a **trigger** and an **example**.

The five rules:

- **6A — Exploration-always-via-subagent.** Broad searches / multi-file reads never enter the
  master's context — fan to `Explore`/subagents; keep only conclusions.
- **6C — Cheapest-capable-model routing.** Mechanical work → cheap model; judgment/review → capable
  model. A standing rule applied everywhere, not just inside `Workflow`.
- **6D — Proactive checkpoint at boundaries.** Auto-trigger `handoff`/`end-session` at phase/spike
  boundaries so the next session starts compact.
- **6F — Workflow token-budget directives.** Explicit per-phase budgets; fan-out scaled to budget;
  `log()` what's dropped.
- **6G — Digest-first reading.** Maintain + read codebase digests (the `codebase-digest` skill)
  instead of re-reading source.

## Owns (exclusive write)

- `docs/playbooks/budget-discipline.md` — the five rules, each with trigger + example + "when it applies."
  Create `docs/playbooks/` (does not exist yet).

### The global `CLAUDE.md` block — you do NOT edit `CLAUDE.md`

`~/.claude/CLAUDE.md` is a **global, non-worktree-isolated, no-rollback** file owned by **Lane Z (the
orchestrator)**. Do **not** edit it. Instead, inside your playbook, add a clearly delimited section:

```
## PROPOSED CLAUDE.md BLOCK (for orchestrator to apply — Lane Z)
<the exact, paste-ready standing-rules text to append to ~/.claude/CLAUDE.md>
```

Write it append-ready (a self-contained heading + the rules) so Z can paste it verbatim with no edits.

## Reads (no write)

- [`docs/ROADMAP.md`](../../ROADMAP.md) §6.
- The `codebase-digest`, `dispatching-parallel-agents`, and `subagent-driven-development` skills (for accuracy of 6A/6F/6G).

## Shared contract

- **`~/.claude/CLAUDE.md` → Lane Z.** Deliver your additions as the PROPOSED CLAUDE.md BLOCK above;
  Z applies it. Never write the file.
- **If 6D's checkpoint becomes a hook**, file a **`settings.json` contract request to Lane Z** (give
  the exact event + matcher + command). Do **not** route this to Lane A; Z owns `settings.json`.

## Done when

- Five rules are written as standing instructions a fresh agent can follow **without interpretation**,
  each with a trigger and a concrete example.
- The PROPOSED CLAUDE.md BLOCK is paste-ready for Z.

## Verify (run, paste real output)

- A fresh agent reading **only** your new playbook + the PROPOSED block can correctly state, for a
  given sample task: (a) which model tier to use, and (b) whether to fan exploration to a subagent.
  Demonstrate this with one worked sample task and the expected answer.

## Notes / open questions

- 6C and 6G are **behavioral** (text rules only). 6D and 6F **may** want a hook or skill — keep those
  as **contract requests to Lane Z** (for any `settings.json` hook), never as direct edits.
- Decide and state whether these rules belong in **global** `CLAUDE.md` (apply across all projects) or
  a project `CLAUDE.md` — the carve assumes global; justify if you deviate.

---

## Rules of the Road (follow exactly)

1. **Stay in your lane.** Write only `docs/playbooks/budget-discipline.md`. Never edit `~/.claude/CLAUDE.md`
   or `~/.claude/settings.json` — deliver the proposed block / file a contract request to Z.
2. **Worktree per lane.** Work on `feat/budget-discipline`; never commit to `main`.
3. **Global/shared files are append-only + single-owner.** You own none — request entries / hand Z the block.
4. **Don't widen scope.** The five rules only. Anything else → report, don't do.
5. **Verify before done.** Run the Verify check; paste the real worked-sample output.
6. **Report for integration.** End with: files changed; the PROPOSED CLAUDE.md BLOCK; any
   `settings.json` contract request to Z; your verify output; anything affecting another lane.
