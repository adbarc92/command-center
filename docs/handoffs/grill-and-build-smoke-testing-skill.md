# Handoff — grill the operator, then build the interactive-smoke-testing skill properly

**Written:** 2026-08-15  ·  **Branch:** `feat/plugin-runtime` @ `7a05cae`  ·  **Session:** `4503b448-49ac-4b8c-9cd8-495937693183`

## ⏳ Background operation in flight

**None.** Nothing is running: 0 `app`/`fleetd-serve`/`cargo` processes, 0 containers, clean tree.
This handoff exists because the work needs a **fresh agent with full budget**, not because anything
is pending.

## Goal

Produce a **robust, empirically-tested** skill for driving operator-in-the-loop smoke tests.
The operator's words: *"I want it to be robust — it's about to see a lot of usage."*

Two phases, in order:

1. **Grill the operator** (`grill-me`) to settle the design questions listed below. Do this
   **first** — do not start authoring from the existing draft.
2. **Author it** under `writing-skills` discipline, with a real RED phase.

## State

- **Existing draft:** `~/.claude/skills/driving-interactive-smoke-tests/SKILL.md` — written at the
  end of a long session, **never subagent-tested**. Treat it as *input*, not a baseline; expect to
  rewrite it. Its captured failure modes are worth mining; its structure is not sacred.
- **Where the evidence came from:** a full operator-driven smoke of PR #49
  ([`spikes/SPIKE-RESULTS.md`](../../spikes/SPIKE-RESULTS.md) → "Smoke run 2"). That run found four
  defects every automated gate had passed, using the method this skill is meant to encode.
- **Prior art to read:** `tests/tauri_command_threading.rs` in this repo is a good example of the
  operator's taste — a *ratchet* that makes a class of defect mechanical rather than a doc asking
  people to be careful.

## Successor autonomy

**Phase 1 (grilling): `discuss-first`.** The whole point is extracting the operator's real
requirements. Do not propose a design and seek assent — interrogate branch by branch.

**Phase 2 (authoring/testing): `autonomous`, and subagents are explicitly authorised.** The operator
confirmed: *"Yes — dispatch freely for testing."* This lifts the earlier session's restriction. Use
subagents for baseline scenarios, wording micro-tests, and post-skill compliance runs. **Without a
real RED phase this deliverable is not robust, it is merely confident** — the existing draft is
already the "merely confident" version.

**Scope: personal, all projects** (`~/.claude/skills/`). It must generalise well beyond this repo —
web apps, CLIs, mobile, hardware — not just Tauri desktop. Do not let cockpit specifics leak in.

## Successor's next action

1. Read the existing draft and the "Smoke run 2" section of `spikes/SPIKE-RESULTS.md`.
2. Invoke **`grill-me`** on the design questions below. Resolve them with the operator.
3. Then invoke **`writing-skills`** and follow RED-GREEN-REFACTOR:
   - **RED:** run pressure scenarios against subagents *without* the skill. Capture rationalisations
     verbatim. **A no-guidance control is mandatory** — if the control doesn't exhibit the failure,
     don't write guidance for it.
   - **GREEN:** write only what addresses observed failures.
   - **REFACTOR:** close the loopholes the tests surface; re-run until stable.

## The empirical corpus (real observed failures — use these, don't invent scenarios)

This is the most valuable part of this brief. Every item below actually happened.

**Result laundering — the core failure the skill must prevent.**
- Smoke run 1 recorded 9 of 11 items as untested, but the surrounding narrative read as "mostly
  fine", and #49 sat for weeks as though only one item remained. **Unmeasured items decay into
  assumed passes.**

**The agent nearly recorded a false pass (this is the sharpest scenario available).**
- Item "prove the plugin cannot reach the network": the operator ran the probe and reported it
  blocked. The report *looked* like a pass. But the error text named origin `http://localhost:5173`
  — the **host** frame, not the sandboxed plugin frame — and it was a **CORS** error, not the
  **CSP** error the test was designed to elicit. Correct scoring: **not run**, re-run in the right
  frame. A skill must make an agent check that the evidence matches the claim, not just the verdict.

**Wrong instrument produces a confident wrong answer.**
- Gate 5's criterion was "`docker ps` empty". `docker ps` shows only *running* containers; three
  containers sat in `Created` state, invisible to it, and that residue broke the *next* run's launch.
  The passing criterion was measuring the wrong thing.

**Operator impressions are unreliable in BOTH directions — and that's not a criticism, it's the design constraint.**
- A hard defect (a frame that never navigated) was read as *"It's blank, but I assume it's mostly
  taking a while to load."*
- Conversely their *"PASS — stayed responsive"* was **correct**, and corroborated by 1,127 background
  samples at 1 Hz showing zero unresponsive. Objective instrumentation is what let both be scored
  honestly. **Sampling in the background costs the operator nothing and should probably be a default,
  not an option** — test that claim in the grill.

**Badly phrased items waste the operator's time — the agent failed at this live.**
- One item was phrased in internal jargon ("what is the cockpit doing?", "did the webview park?").
  The operator replied: ***"I'm not sure what you're asking me to check, be more specific."*** Only
  after the agent read the component source and re-phrased concretely — *"a modal titled `ORACLE
  APPROVAL REQUIRED` with `✕ REJECT` and `✓ APPROVE`; does the content behind it disappear?"* — was
  it answerable. **This is a prime RED scenario: give a subagent an internal-jargon checklist item
  and see whether it translates to observable terms before asking.**

**Free-text answers carried the decisive evidence.**
- The operator's own additions — a devtools 405 preflight, a pasted `DataCloneError` stack — led
  directly to two root causes that the structured options alone would have missed entirely.
  **Options must never crowd out free-text.** Note this cuts against over-structuring.

**BLOCKED is not FAIL.**
- Three items were unreachable because of an unrelated pre-existing defect. Scoring them FAIL would
  have falsely indicted the feature under test; scoring them PASS would have been a lie. They need
  their own bucket, and the skill must stop agents collapsing four outcomes into two.

**Deriving criteria from source converted judgement calls into observations.**
- Reading the code first yielded exact expected strings (`connected · caps: …` vs
  `failed to connect: …` after 3 s) instead of "check it connects". Cheap, and it is what made most
  items scorable at all.

## Design questions the grill must resolve (do NOT pre-answer these)

1. **Batching vs strict one-at-a-time.** The skill draft says one item per question. In practice the
   agent asked **2–3 related items per round** and the operator explicitly praised the flow. Strict
   serialisation may be too slow for an 11-item checklist. Where is the real line — and is it about
   item count, or about whether the items share a setup?
2. **How much source-reading is warranted per item?** It clearly paid off, but it costs latency
   before the operator can act. Should it be mandatory, conditional, or judgement?
3. **Should background instrumentation be mandatory?** It was decisive here. But it is only possible
   when the agent can observe the system. What is the rule when it can't?
4. **What happens when the operator's report contradicts the instrument?** This didn't occur —
   they agreed every time — so there is no observed precedent. Needs a decided policy.
5. **Continue or halt after a failure?** This run continued past failures and banked 8 further
   results, which the operator seemed to want. But the prior handoff's standing instruction was
   "record precisely what failed and stop." Which is right, and does it depend on failure severity?
6. **Authority to mutate the environment.** The agent asked before removing containers and before
   spending real money, and stated blast radius each time. Should the skill mandate that pattern,
   and does it differ for reversible vs irreversible actions?
7. **Does the skill own the record format**, or only the interrogation method? Writing results to a
   durable artifact was load-bearing here, but it may be out of scope.
8. **Operator expertise assumptions.** This operator reads stack traces and devtools fluently.
   Should the skill adapt phrasing to expertise, and if so how does the agent detect it?
9. **Relationship to `verify` / `qa-runner`.** Both exist in the operator's skill set and drive
   browser automation. Where is the boundary, so the right one gets picked?

## Live decisions (settled)

- **Subagent dispatch is authorised** for testing this skill (operator, this session).
- **The existing draft is input, not a baseline.** Expect a rewrite.
- **Personal scope**, `~/.claude/skills/`, must generalise across project types.
- **The method itself is validated** — it found four real defects in one session, two of which are
  now fixed and verified. The question is how to *encode* it, not whether it works.

## ⚠️ Open questions (unresolved — do NOT settle implicitly)

1. **All nine design questions above.** They are the grill's agenda. None are pre-answered; several
   (batching, halt-vs-continue) have evidence pointing *against* what the current draft says.
2. **Does the skill keep its current name?** `driving-interactive-smoke-tests` was chosen without
   consultation. Naming affects discovery, which the operator cares about.
3. **Should the skill's RED corpus be committed somewhere reusable?** The scenarios above are
   valuable and were expensive to obtain; there may be no good home for them.
4. **PR #49's six commits are still unpushed** — carried over, still unanswered. The operator chose
   "commit" over "commit and push" but said to focus on PR'ing the in-flight work. Confirm before
   pushing; do not assume. See
   [`docs/handoffs/4503b448-49ac-4b8c-9cd8-495937693183.md`](4503b448-49ac-4b8c-9cd8-495937693183.md)
   for the rest of the #49 state, including the agreed-next **D-7** fix.

## On completion

1. Skill lives at `~/.claude/skills/<name>/SKILL.md`, replacing the draft.
2. Record what the RED phase actually found — the rationalisation table should come from observed
   subagent behaviour, not from imagination.
3. If a memory note is warranted, update
   `~/.claude/projects/d--MajorProjects-CURRENT-command-center/memory/smoke-testing-via-structured-dialog.md`
   and its `MEMORY.md` index line, which currently point at the draft's name.
