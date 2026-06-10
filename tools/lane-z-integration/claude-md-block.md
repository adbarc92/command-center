Lane B's PROPOSED CLAUDE.md BLOCK — appended verbatim to `~/.claude/CLAUDE.md` by
`deploy_globals.py` (idempotent: skipped if the heading already exists).

---

## Budget-Discipline Standing Rules

Always-on cost-discipline rules. Each has a trigger; follow it by default, override only with a
stated one-line reason. (Full reference: command-center `docs/playbooks/budget-discipline.md`.)

1. **Exploration via subagent (6A).** Trigger: about to do a broad/open-ended search, read 3+ files
   to answer one question, read a file only for a conclusion, or onboard into an unfamiliar repo.
   → Dispatch an `Explore`/`Task` subagent with an explicit output contract ("return file:line +
   short summary; don't paste file bodies"); keep only its conclusion. Exception: a known file you
   are about to EDIT — read it directly.

2. **Cheapest-capable-model routing (6C).** Trigger: about to dispatch a `Task`/subagent or choose
   a model. → Cheap/fast model for mechanical work (1–2 files, complete spec, rote transform);
   standard model for multi-file integration and routine debugging; capable/top model for design,
   architecture, ambiguity, and ALL code/spec review. Unsure → start one tier down and escalate on
   failure. Use model *tiers*, not hard-coded IDs.

3. **Checkpoint at boundaries (6D).** Trigger: a phase/spike/milestone just completed, context is
   heavy at a clean cut point, or the user signals stop/handoff. → Proactively run `end-session`
   (resume as future-you) or `handoff` (resume as another agent) so the next session starts
   compact. Don't silently continue past a boundary; defer only with a stated reason. Skip mid-phase
   when next steps are tightly coupled.

4. **Workflow token budgets (6F).** Trigger: starting a multi-phase Workflow/plan or a fan-out. →
   State an explicit per-phase token budget (cap on context returned to the master) before each
   phase; scale fan-out to the budget (don't fan 10 agents into a 2k budget); `log()` what you drop
   to fit, with a pointer to recover it.

5. **Digest-first reading (6G).** Trigger: about to read source to UNDERSTAND a repo/sub-project. →
   Read its digest first (`docs/digests/` or `CLAUDE.md`/`AGENTS.md`); if none exists and the
   exploration is non-trivial, generate one with the `codebase-digest` skill at
   `docs/digests/<unit>.md`, then read that. Mark and re-verify if the digest looks stale.
