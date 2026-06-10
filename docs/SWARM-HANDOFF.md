# Swarm Handoff — Command Center Roadmap

> **Companion to [`docs/ROADMAP.md`](ROADMAP.md).** This document lets a *team of agents work the
> roadmap in parallel* without colliding. It has two halves:
> **Part I — the reusable Swarm Handoff protocol** (the part that becomes the `swarm-handoff` skill,
> roadmap Item 2) and **Part II — this handoff** (the roadmap carved into dispatch-ready lanes).
> Date: 2026-06-08.

Serves the North Star (ship autonomously & fast): one orchestrator decomposes a multi-feature doc
into independent lanes and fans out a swarm, instead of doing it serially.

## Kickoff for a fresh context (read this first)

You've been handed this doc cold to **dispatch the swarm**. You need no prior conversation — every
lane brief in Part II is self-contained.

1. The `swarm-handoff` skill (global, `~/.claude/skills/swarm-handoff/`) encodes the method in
   Part I — invoke it.
2. **Dispatch Lanes A, B, C, D concurrently**, then **Lane Z last** (it owns `~/.claude/settings.json`
   and integrates the others' hook requests). **Skip Lane E — already done this session** (it built
   the `swarm-handoff` skill itself).
3. Give each agent its lane brief *verbatim* plus the Rules of the Road. Worktree-isolate any lane
   that mutates repo files. Then run the Integration plan at the bottom.
4. Dispatching a swarm is **expensive/opt-in** — confirm the user wants to spend it before fanning out.
5. Blocked items (6B, 3-Tier2, 4-build) are not part of this swarm — see the bottom of Part II.

---

# Part I — The Swarm Handoff Protocol (reusable)

## When to use

A document (spec, plan, roadmap) contains **two or more features that are genuinely independent** —
they can be built without shared state or sequential dependency. If the work is one coherent
feature, or each step needs the previous one's output, **do not swarm it** — that's a normal plan,
executed serially.

## The core risk: false independence

The failure mode is two lanes editing the same file and producing a merge conflict (or worse, a
silent semantic clash). The entire protocol exists to make independence *real*: every lane has
**exclusive ownership** of the files it writes, and anything two lanes both need is pulled out into
a **shared contract** with a single owner. If you cannot cleanly assign ownership, the features are
not independent — merge them into one lane.

## The method (what the orchestrator does)

1. **Dependency-analyze the doc.** List the features. For each pair, ask: does B need A's output,
   or do they write the same files? Build the dependency graph. Mark each feature
   ✅ *ready* / 🔗 *blocked* (on another lane, or an external dependency).
2. **Carve lanes.** One lane ≈ one independent feature (or a *cluster* of features that share a
   file/domain and would collide if split — co-locate those in one lane). Aim for the fewest lanes
   that have zero write-overlap.
3. **Assign ownership.** Each lane declares **files it OWNS** (exclusive write), **files it reads**
   (read-only), and any **shared contract** it participates in.
4. **Name the shared contracts.** For every file ≥2 lanes need to touch, designate **one owning
   lane**; other lanes file an append-only *contract request* resolved at integration. Common
   hotspots: global config (`settings.json`), global instructions (`CLAUDE.md`), shared type/IPC
   modules, route registries.
   - ⚠️ **Global / out-of-repo files are not worktree-isolated.** Worktrees isolate *repo* files
     only. Files in `~/.claude/` (`settings.json`, `CLAUDE.md`, the memory store) are shared across
     every worktree and process, so two lanes writing one collide at the filesystem level
     (last-write-wins — not even a git merge). For these, **single-owner serialization is the only
     protection** — give each its own owning lane and never let another lane write it. Prefer a
     thin dedicated owner lane (one file, integrates last) for hot global config.
5. **Set integration order + checkpoints.** Usually: independent lanes merge in any order; the
   contract-owning lane merges first; a final reconciliation pass verifies the whole.
6. **Write dispatch-ready briefs** (schema below) and fan out — `Workflow` /
   `dispatching-parallel-agents` / `subagent-driven-development`, **worktree isolation** for any
   lane that mutates repo files concurrently.

## Lane brief schema (each lane is self-contained — an agent starts from it cold)

```
### Lane <X> — <name>   ·   <✅ ready | 🔗 blocked on …>
- Roadmap items: <which>
- Goal: <one sentence>
- Owns (exclusive write): <exact paths>
- Reads (no write): <paths>
- Shared contract: <file → owning lane → what you may request>
- Depends on / blocks: <lanes or external deps>
- Done when: <observable acceptance>
- Verify: <exact command or check>
- Notes / open questions: <what to investigate>
```

## Rules of the road (every swarm agent follows these)

1. **Stay in your lane.** Write only files your lane OWNS. Never edit another lane's owned files —
   if you need a change there, file a contract request in your final report.
2. **Branch/worktree per lane.** One feature branch (or worktree) per lane; never commit to `main`.
   Remember worktrees isolate *repo* files only — global files (`~/.claude/**`) are shared, so the
   single-owner rule (3) is what protects them, not the worktree.
3. **Shared files are append-only + single-owner.** Touch a shared-contract file only if you own it;
   otherwise request the entry.
4. **Don't widen scope.** Build only your lane's items. Discoveries outside scope → report, don't do.
5. **Verify before done.** Run your lane's verify check; report real output, not assertions.
6. **Report for integration.** End with: what you changed, any contract requests, your verify
   output, and anything that affects another lane.

## Integration (orchestrator, after lanes return)

Merge the contract-owning lane(s) first, then the rest; apply each lane's contract requests against
the owned shared files; run a reconciliation pass (full build/test) over the merged whole.

---

# Part II — This Handoff: the Roadmap

**Scope:** the six roadmap items. The app-plugins build is *not* in this swarm — it's gated on the
hands-on Phase-0 webview spike (serial, not parallelizable yet); it stays on its own track.

## Dependency graph

```
Ready now (parallel):   A ─ cache economics        D ─ dashboard spec (design only)
                        B ─ budget rules           Z ─ config owner (settings.json; integrates last)
                        C ─ context offload T1      E ─ swarm-handoff skill — done this session (see below)
Blocked (not swarmed):  6B  → user's ContextCurator must ship
                        3-Tier2 → claude.ai connector reliability
                        4-build → needs Lane D's spec + a Halyard head
Shared contracts:       ~/.claude/settings.json  (owner: Lane Z — dedicated; not worktree-isolated)
                        ~/.claude/CLAUDE.md       (owner: Lane B  — not worktree-isolated)
```

Lanes A–D + Z have **zero owned-file overlap**; the only cross-lane touch points are the two shared
global files above, each single-owned (and, being in `~/.claude/`, protected *only* by that single
ownership — see Part I). Lane A/B/C file their hook entries to **Lane Z** rather than writing
`settings.json` themselves; Z assembles them in one write at integration. (Lane E — the
swarm-handoff skill — is handled by the orchestrator this session, not dispatched here.)

---

### Lane A — Cache economics   ·   ✅ ready
- **Roadmap items:** 1 (cache-aware approval timer), 5 (API rate-limit auto-retry), 6E (cache-window pacing).
- **Goal:** make the ~5-minute prompt-cache window visible and economical — show a countdown while
  awaiting approval, auto-retry API 429s within the window, and pace work to keep cache warm.
- **Owns (exclusive write):** `tools/cache-countdown/**` (PowerShell hook scripts + Python ticker,
  packaged with **UV** per global rules); any new `~/.claude/state/` timer schema docs.
- **Reads:** [`docs/ROADMAP.md`](ROADMAP.md) §1/§5/§6E; the reference impl
  `KatsuJinCode/claude-cache-countdown`.
- **Shared contract:** `~/.claude/settings.json` is owned by **Lane Z**. Lane A does **not** edit it —
  A files a contract request to Z for the `Stop` + `UserPromptSubmit` hook entries (giving the exact
  command/script paths A produced). A owns only its own scripts.
- **Depends on / blocks:** A's hook entries land via Z at integration (Z integrates last).
- **Done when:** awaiting-approval shows a live countdown + cost-at-stake in this terminal; a
  simulated API 429 auto-retries with backoff that stays inside the cache window; the pacing rule
  (warm during work / cool when idle) is documented.
- **Verify:** trigger a `Stop` (finish a turn) → countdown appears and ticks `🔥→🟢→🟡→🔴→❄️`;
  install adds exactly two hook entries to `settings.json` (diff it).
- **Notes:** investigate the harness retry surface for Item 5 — is it a `settings.json` env, a CLI
  flag, or a wrapper? Don't assume; find where the agent's own API retry is configured.

### Lane B — Budget-discipline rules   ·   ✅ ready
- **Roadmap items:** 6A (exploration-via-subagent), 6C (cheapest-capable-model routing), 6D
  (checkpoint at boundaries), 6F (workflow token budgets), 6G (digest-first reading).
- **Goal:** turn existing capabilities into *standing, automatic* budget discipline.
- **Owns (exclusive write):** `~/.claude/CLAUDE.md` standing-rules additions; a new
  `docs/playbooks/budget-discipline.md` capturing the rules + when each applies.
- **Reads:** [`docs/ROADMAP.md`](ROADMAP.md) §6; the `codebase-digest` / `dispatching-parallel-agents`
  / `subagent-driven-development` skills.
- **Shared contract:** `~/.claude/CLAUDE.md` — **Lane B is the OWNER.** If 6D's checkpoint becomes a
  hook, file a `settings.json` contract request to **Lane Z** (do not edit settings.json directly).
- **Depends on / blocks:** none (CLAUDE.md owner).
- **Done when:** the five rules are written as unambiguous standing instructions an agent can follow
  without interpretation, each with a trigger and an example.
- **Verify:** a fresh agent reading only the new CLAUDE.md section can state, for a sample task,
  which model tier to use and whether to fan exploration to a subagent.
- **Notes:** 6C/6G are behavioral; 6D/6F may want a hook or skill — keep those as contract requests
  to Lane A, not direct edits.

### Lane C — Context offload, Tier 1   ·   ✅ ready
- **Roadmap items:** 3 (Tier 1 only — local Claude Code memory + ContextCurator recall/offload).
- **Goal:** automate recall of durable project facts at session start and offload of new durable
  facts during/after work, so stable context isn't re-derived at token cost. Tier 2 (Claude.ai
  Project KB) is **out of scope here** (deferred — connector reliability).
- **Owns (exclusive write):** `docs/playbooks/context-offload.md` (the Tier-1 automation design +
  the memory-write discipline); any helper script under `tools/context-offload/**`.
- **Reads:** the project memory store (`~/.claude/projects/<proj>/memory/`), the ContextCurator MCP
  surface (`cc_*` tools).
- **Shared contract:** if it adds a `SessionStart` hook, file a request to **Lane Z** (settings.json
  owner). The memory store is written by normal operation — treat as append-only.
- **Depends on / blocks:** none. **Note:** the *ContextCurator integration* (6B) is a separate
  blocked item (user's product) — Lane C uses ContextCurator as-is, does not build it.
- **Done when:** durable facts are recalled at session start without manual prompting, and new
  durable facts are written to memory automatically at natural boundaries.
- **Verify:** start a fresh session → relevant prior facts surface without the user re-stating them.

### Lane D — Project-dashboard spec   ·   ✅ ready (design only)
- **Roadmap items:** 4 (Central Project Manager) — **brainstorm → spec only**, no build.
- **Goal:** produce an approved design spec for the project-tracking dashboard.
- **Owns (exclusive write):** `docs/superpowers/specs/2026-06-08-project-dashboard-design.md`.
- **Reads:** the Halyard digest (`docs/digests/halyard-digest.md`), the Audience digest, the
  app-plugins spec (for the lifecycle-state signals to aggregate), the Halyard-head brief.
- **Shared contract:** none — pure design lane, touches no code and no shared config.
- **Depends on / blocks:** **blocks** the dashboard *build* (future) and informs whether to graduate
  Halyard from headless. Independent of A/B/C/E.
- **Done when:** the spec resolves the open questions (what is a "stage"; inferred vs declared; how
  Halyard/Audience feed it) and passes the design-critique gate (3 rounds, like the app-plugins spec).
- **Verify:** spec file exists with a Design Critique Log of three rounds; no `TBD`s.
- **Notes:** use the `brainstorming` skill; expect the design-critique gate hook to fire on creation.

### Lane E — Swarm Handoff skill   ·   🛠️ handled by the orchestrator THIS session (not swarm-dispatched)
> Because we extract the skill directly from Part I right after this doc is reviewed, Lane E is
> **not** part of any future swarm dispatch — it's listed for completeness and to record where the
> skill came from. If the swarm runs later, skip this lane.
- **Roadmap items:** 2 (Swarm Handoff as a master-agent capability).
- **Goal:** package **Part I of this document** as a reusable `swarm-handoff` skill the orchestrator
  invokes on any multi-feature doc.
- **Owns (exclusive write):** `~/.claude/skills/swarm-handoff/SKILL.md` (+ any `references/`).
- **Reads:** **Part I of this file** (the protocol — it's the skill's body) and this Part II as the
  worked example to reference.
- **Shared contract:** none.
- **Depends on / blocks:** soft-depends on this doc existing (the proven format); otherwise independent.
- **Done when:** the skill triggers on "swarm handoff" / a doc with independent features, and walks
  the Part-I method to emit a dispatch-ready handoff like Part II.
- **Verify:** invoke the skill against a sample 2-feature doc → it produces correct lanes with
  ownership + contracts. Use `skill-creator` to build and eval it.

### Lane Z — Config owner (`settings.json`)   ·   ✅ ready (integrates last)
- **Roadmap items:** none of its own — it exists to make the global-config shared contract safe.
- **Goal:** be the **single writer** of `~/.claude/settings.json`, assembling every hook/env entry
  the other lanes request into one coherent file (the global file no worktree can isolate).
- **Owns (exclusive write):** `~/.claude/settings.json`.
- **Reads:** the contract requests from Lanes A (Stop + UserPromptSubmit timer hooks), B (6D
  checkpoint hook, if any), C (SessionStart hook, if any) — each supplies an exact command/path.
- **Shared contract:** *is* the owner; no other lane writes this file.
- **Depends on / blocks:** depends on A/B/C having produced their hook scripts + declared their
  entries → **Z integrates last.** Can start early by scaffolding the file + publishing the
  request schema (what each lane must submit: event, matcher, command).
- **Done when:** `settings.json` contains exactly the union of requested entries, valid JSON,
  no clobbered pre-existing keys.
- **Verify:** `settings.json` parses; diff shows only the intended additive hook entries; each
  referenced script path exists (produced by its owning lane).
- **Notes:** additive only — never rewrite unrelated keys. If two lanes request the same event,
  reconcile into a single hook list.

---

## Integration plan

1. **Lanes A, B, C, D run concurrently.** A/B/C produce their scripts and **file `settings.json`
   contract requests to Z** (exact event + command); they do not touch `settings.json`.
2. **Merge the isolated owners:** Lane B (`CLAUDE.md`), Lane C (`playbooks/`, memory automation),
   Lane D (the spec) — no overlap, any order.
3. **Merge Lane Z last:** Z writes the union of all requested hook entries into `settings.json` in
   one pass (additive, valid JSON).
4. **Reconcile:** `settings.json` parses and holds exactly the intended hooks pointing at scripts
   that exist; a fresh session exercises the cache timer (A), the budget rules (B), and memory
   recall (C) together without conflict.
   *(Lane E — the skill — was completed by the orchestrator this session; nothing to merge here.)*

## After the swarm

- **6B, 3-Tier2, 4-build** remain blocked — revisit when their dependencies clear (ContextCurator
  ships / connector reliable / dashboard spec approved + Halyard head).
- **Lane E's output (the skill) generalizes this whole document** — the next roadmap can be swarmed
  by invoking it instead of hand-writing Part II.
