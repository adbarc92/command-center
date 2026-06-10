# Budget-Discipline Standing Rules

> **What this is.** Five always-on rules that turn capabilities the agent *already has* into
> **automatic** budget discipline instead of manual practice. They implement **Roadmap §6**
> (Proactive budget discipline) items **6A, 6C, 6D, 6F, 6G**. Each rule is written so a fresh
> agent can follow it **without interpretation**: it has a **trigger** (when it fires), a
> **concrete example**, and a **when-it-applies** scope.
>
> **Why standing, not manual.** The North Star is to keep cost low through intelligent resource
> use. Manual best-practice decays under load; a standing rule with an unambiguous trigger does
> not. These five are the cheap, behavioral wins — no new product code required for 6A/6C/6G;
> 6D/6F may want a harness hook (see the contract request to Lane Z at the end).
>
> **Where these live.** These rules are **global** — they belong in `~/.claude/CLAUDE.md` so they
> apply to *every* project, not just the Command Center. (Justification: budget discipline is not
> Command-Center-specific; re-reading source, fanning exploration, and model routing are universal
> agent behaviors. The carve assumed global, and that is correct.) The paste-ready text for the
> orchestrator is in the **PROPOSED CLAUDE.md BLOCK** section at the bottom. This playbook is the
> authoritative long-form reference; the CLAUDE.md block is the compressed standing-rule digest of
> it.

---

## How to read a rule

Each rule below has four parts:

- **Trigger** — the observable condition that makes the rule fire. If you can't name the trigger,
  the rule isn't standing; it's advice. Triggers here are phrased as "when you are about to …".
- **Do** — the mandated action.
- **Example** — one concrete worked instance.
- **When it applies / does not** — scope, so you don't over- or under-apply it.

The rules are **defaults, not absolutes.** A rule may be overridden when its own "does not apply"
clause is met, or when the user explicitly directs otherwise. Overriding silently is a violation;
overriding with a one-line stated reason is fine.

---

## 6A — Exploration-always-via-subagent

**Standing rule:** Broad, open-ended, or multi-file *exploration* never enters the master agent's
context. Fan it out to an `Explore`/subagent (via the `Task` tool); keep only the **conclusions**
the subagent returns.

- **Trigger** — You are about to do any of:
  - a broad search whose result set you can't bound in advance (e.g. "where is X handled?",
    "how does Y work across the repo?", "find all callers of Z");
  - reading **3 or more files** to answer a single question;
  - reading a file you only need a *conclusion* from (not one you're about to edit);
  - onboarding into an unfamiliar repo/sub-project.
- **Do** — Dispatch a subagent with a focused prompt and an explicit *output contract* ("return:
  the file:line of the handler and a 3-line summary of the flow; do not paste file bodies"). Read
  only its summary. The subagent's large intermediate reads stay in *its* context and are
  discarded; your master context grows by a few lines, not a few thousand.
- **Example**
  - **Bad (master context bloats):** In the master, run `Grep "rate.?limit"`, open the 6 files it
    returns, read each fully to understand the backoff path. → ~4k tokens of source now
    permanently in the master window.
  - **Good (conclusion only):** `Task("Explore: trace the rate-limit retry path. Return the
    file:line where 429s are caught, the backoff formula, and whether it respects the cache TTL.
    Do not paste file bodies.")` → master gains ~6 lines; the 4k tokens died with the subagent.
- **When it applies** — Any exploration, search, mapping, or "read to understand" task. Especially
  in long sessions, where master-context growth is the dominant cost driver.
- **When it does NOT apply** — (a) You already know the exact file *and* you're about to **edit**
  it — read it directly (you need it in context to edit). (b) A single, bounded read of one known
  file to get one fact. (c) The exploration's conclusion is itself large and must be carried
  verbatim (rare) — even then, prefer a subagent that returns a *digest* of it.
- **Cross-link** — Pairs with **6G**: before exploring at all, check for an existing digest.

---

## 6C — Cheapest-capable-model routing

**Standing rule:** Route every delegable unit of work to the **cheapest model that can do it
correctly**. Mechanical work → cheap/fast model. Judgment, design, and review → capable model.
This applies **everywhere** work is dispatched to a subagent — not only inside a `Workflow`.

- **Trigger** — You are about to dispatch a `Task`/subagent, or choose a model for a delegable
  unit of work.
- **Do** — Classify the task by the signals below and pick the tier. When genuinely unsure between
  two tiers, start one tier *down* and escalate on a `BLOCKED`/poor result (escalation is cheap;
  over-provisioning every task is not).

| Tier | Use for | Signals |
|---|---|---|
| **Cheap / fast** (e.g. Haiku-class) | Mechanical implementation | Touches 1–2 files; complete, unambiguous spec; isolated function; rote transform; mechanical refactor; running/formatting/listing |
| **Standard / mid** (e.g. Sonnet-class) | Integration & moderate judgment | Multi-file coordination; pattern-matching to existing code; routine debugging; tracing a known flow |
| **Capable / top** (e.g. Opus-class) | Judgment, design, review | Architecture/design decisions; broad codebase understanding; **any code review or spec-compliance review**; ambiguous or under-specified tasks; security-sensitive reasoning |

- **Example**
  - "Add a `--force` flag to this CLI command (spec is complete, one file)." → **cheap.**
  - "Wire the new publishing service into the orchestrator and the API routes." → **standard.**
  - "Review this PR for spec compliance and code quality." / "Design the dashboard's stage model."
    → **capable.**
- **When it applies** — Every dispatch decision, in any skill or ad-hoc, in any project.
- **When it does NOT apply** — The current (master) turn itself; you don't re-route your own
  in-context reasoning. Also: if a cheap model already failed this exact task (`BLOCKED`), escalate
  rather than re-dispatching the same tier.
- **Note** — Use *model tiers*, not hard-coded model IDs, in instructions — the cheapest-capable
  model in each tier changes over time. When you do need a current model ID/price, consult the
  `claude-api` skill rather than guessing.

---

## 6D — Proactive checkpoint at boundaries

**Standing rule:** At a natural **work boundary**, proactively checkpoint the session — run
`handoff` (if another agent/session will pick up) or `end-session` (if future-you resumes) — so the
next session starts **compact** instead of re-loading a bloated conversation at full cost.

- **Trigger** — You have just **crossed a boundary**, defined as any of:
  - a **phase** completed (a plan's phase done, a spike/gate reached, a milestone shipped);
  - a **context-size** threshold — the working context is large and the *current* sub-goal is
    done (a clean cut point), so continuing would carry stale bulk forward;
  - the user signals a **stop** ("let's stop here", "wrap up", "I have to log off") — route to
    `end-session`; or a **handoff** to another agent — route to `handoff`.
- **Do** — Don't silently continue past the boundary. Either checkpoint now, or state in one line
  why you're deferring (e.g. "next phase is 5 lines; deferring checkpoint until it's done").
  Prefer `end-session` for resume-as-future-you; `handoff` for resume-as-another-agent.
- **Example** — You finish the "backend lifecycle" phase of a build; the next phase is the webview
  embedding spike (a different mental context). **Boundary crossed →** trigger `end-session` so the
  embedding session opens with a tight handoff doc, not the full backend-phase transcript.
- **When it applies** — Phase/spike/milestone completions; explicit stop/handoff signals; clean
  cut points when context is heavy.
- **When it does NOT apply** — Mid-phase with tightly-coupled next steps (checkpointing there would
  fragment a single unit of work and the next session would just re-load it anyway). Tiny remaining
  work after the boundary (finish it first, then checkpoint).
- **Cross-link** — Feeds Roadmap §3 (context offload): the checkpoint is where durable facts get
  written down and the live window gets dropped.
- **Possible automation** — The "user signals stop" trigger is detectable by the harness. See the
  **`settings.json` contract request to Lane Z** at the end of this file. Until/unless that hook
  lands, this rule is followed **behaviorally** by the agent.

---

## 6F — Workflow token-budget directives

**Standing rule:** When you run a multi-phase `Workflow` (or any multi-phase dispatched plan),
assign each phase an **explicit token budget up front**, **scale fan-out to that budget**, and
**`log()` what you dropped** to stay within it. A phase without a stated budget is a budget
violation waiting to happen.

- **Trigger** — You are about to start a `Workflow` / multi-phase dispatched plan with two or more
  phases, or a fan-out step (parallel subagents).
- **Do**
  1. **State a per-phase budget** before dispatching that phase (e.g. "Explore phase: ≤ 8k tokens
     of returned context"). The budget is the cap on what comes *back into the master*, not what
     subagents spend internally.
  2. **Scale fan-out to the budget.** More budget → more parallel explorers / wider sweep; tight
     budget → fewer agents, narrower scope, ask each to return less. Don't fan out 10 agents into a
     2k-token budget.
  3. **`log()` what's dropped.** When you trim returned context to fit, record *what* you dropped
     and *where to find it* (file:line or "see subagent N's full output"), so the drop is
     recoverable, not silent.
- **Example** — A 3-phase workflow: *Explore* (budget 8k, fan out 3 explorers, each returns ≤ 2k
  summary), *Implement* (budget 4k, single cheap-model implementer per task), *Review* (budget 6k,
  one capable-model reviewer). Explore returns 11k of summaries → trim to the 8k budget, and
  `log("dropped explorer-3's full dependency dump; recover at services/api/src/routes/*.ts")`.
- **When it applies** — Any multi-phase `Workflow` or multi-phase dispatched plan; any fan-out.
- **When it does NOT apply** — A single-shot task with no phases and no fan-out (there's nothing to
  budget across). A genuinely tiny workflow where the whole thing fits well under one phase's worth
  of context — still cheap to state a one-line budget, but don't over-engineer it.
- **Cross-link** — The budget *drives* **6C** (tight budget favors cheaper models) and **6A** (fan
  exploration out so its cost stays off the master's budget line).

---

## 6G — Digest-first reading

**Standing rule:** Before reading source to *understand* a codebase (or sub-project), read its
**digest** if one exists; if one doesn't exist and you're about to do non-trivial exploration,
**produce one** (via the `codebase-digest` skill) and read that. Maintain digests so re-spend on
re-reading source drops to near zero.

- **Trigger** — You are about to read source files to **understand** a repo/sub-project (its
  architecture, contracts, data flow, or "where is X") — as opposed to reading a specific file to
  edit it.
- **Do**
  1. **Check for a digest first.** Look in the target repo's `docs/digests/` (canonical location)
     and its `CLAUDE.md`/`AGENTS.md`. If a current digest exists, read it instead of the source.
  2. **If none exists and the exploration is non-trivial** (more than a couple of files), run the
     `codebase-digest` skill to produce one at `docs/digests/<unit>.md`, then read it. This pays
     for itself the second time anyone (including future-you) needs the same understanding.
  3. **Keep it honest.** A digest is stamped with a commit/date; if it's stale relative to current
     HEAD and you relied on it, note `(digest may be stale)` and verify the specific claim you
     depend on against source.
- **Example** — Task touches Halyard. Instead of opening 15 Halyard files to learn its JSON-store
  model, read `halyard/docs/digests/halyard.md` (or generate it once with `codebase-digest`). Next
  time Halyard comes up, the digest is already there — zero re-read.
- **When it applies** — Understanding/onboarding/mapping any codebase you don't already hold in
  context; recurring work against Halyard, Audience, or any external repo.
- **When it does NOT apply** — Editing a specific known file (read it directly — see 6A's exception).
  A repo small enough that the digest *is* the source (a handful of files). One-off throwaway
  questions where no one will revisit.
- **Cross-link** — Combines with **6A**: the *digest-generation* itself is exploration → run it in a
  subagent (`codebase-digest` does heavy reading; keep that out of the master). Feeds Roadmap §3.

---

## Worked verification sample (fresh-agent decidability test)

A fresh agent reading **only** this playbook + the PROPOSED block must be able to answer, for a
sample task: **(a) which model tier**, and **(b) whether to fan exploration to a subagent.**

**Sample task:** *"Figure out how the Audience publishing service validates payloads, then add a
missing required-field check to one validator file."*

**Expected answers (derivable from the rules above):**

| Question | Answer | Rule applied |
|---|---|---|
| (a) Model tier? | **Two tiers.** The *"figure out how validation works"* exploration → dispatch to a **standard**-tier explorer subagent (tracing a known flow). The *"add one required-field check to one known validator file"* implementation → **cheap** tier (1 file, complete spec, mechanical). | 6C |
| (b) Fan exploration to a subagent? | **Yes for the "figure out how it works" part** — it's a multi-file "how does X work" exploration → 6A fires; dispatch an explorer, keep only its file:line + summary. **No for the edit** — once the explorer returns the validator's path, read *that one file* directly in the master and edit it (6A exception: known file you're about to edit). | 6A |
| Bonus: digest first? | **Yes** — before exploring Audience, check `audience-*/docs/digests/` for a publishing digest; if absent and this is recurring, generate one. | 6G |

This demonstrates the rules are decidable without interpretation: the task splits cleanly into an
*exploration* unit (standard model, fanned out) and an *implementation* unit (cheap model, direct
read+edit), exactly as 6A and 6C prescribe.

---

## PROPOSED CLAUDE.md BLOCK (for orchestrator to apply — Lane Z)

> Lane Z: append the block below **verbatim** to `~/.claude/CLAUDE.md`. It is self-contained
> (heading + rules) and global by intent (these behaviors are not project-specific). No edits
> needed. Full reference lives at `docs/playbooks/budget-discipline.md` in the Command Center repo.

```markdown
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
```

---

## Contract request to Lane Z — `settings.json` hook (6D automation, optional)

6D's *"user signals stop"* trigger is harness-detectable, so it **may** become a hook rather than
relying on agent behavior. This is a **request to Lane Z** (owner of `settings.json`) — not a
direct edit, and **not** routed to Lane A.

- **Event:** `Stop` (fires when the agent finishes responding / the turn ends — the harness's
  natural "session is pausing" signal).
- **Matcher:** `*` (all turns; the hook itself decides whether a checkpoint is warranted).
- **Command (intent):** a lightweight check that, at a turn-end, evaluates whether a **boundary**
  was crossed (phase complete / heavy context) and, if so, **surfaces a reminder** to run
  `end-session`/`handoff` (a nudge, not an auto-run — auto-ending a session unprompted is too
  aggressive). Concretely, a PowerShell/UV ticker-style script that writes a
  `~/.claude/state/checkpoint-nudge-{session}.json` flag the UI/agent can read, mirroring the
  cache-timer mechanism in Roadmap §1.
- **Status:** **Optional / proposed.** If Z declines or defers, 6D still works as a behavioral rule
  (above). Lane Z owns the final event/matcher/command wording; this entry states the intent so Z
  can decide. Do **not** treat this as blocking.
```
settings.json hook request (for Lane Z to evaluate):
  event:   Stop
  matcher: "*"
  command: <boundary-detector that writes ~/.claude/state/checkpoint-nudge-{session}.json
            when a phase/spike boundary is detected; surfaces an end-session/handoff nudge>
```
