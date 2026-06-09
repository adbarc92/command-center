# Context offload — Tier 1 (local memory recall + offload)

> Playbook for **Roadmap [item 3](../ROADMAP.md#3-dual-tier-context-offload-for-budget), Tier 1 only.**
> Serves the North Star pillars **context hygiene** + **low cost**: stop re-deriving stable
> facts at token cost. Lane C (`feat/context-offload-t1`). Date: 2026-06-09.
>
> **Tier 2 (the Claude.ai Project knowledge base) is out of scope here** — deferred on connector
> reliability (ROADMAP §3). This playbook covers only the local, always-available tier.

## What this is

Two automated motions around the local Claude Code **project-memory store**:

- **Recall** — at **session start**, durable facts from prior sessions surface in context
  **without the user re-stating them**.
- **Offload** — at **natural boundaries** (phase end, spike end, handoff), new durable facts are
  **written to memory** so they leave the active window and are recalled next time instead of
  re-derived.

Both are headless-safe: pure stdlib Python, no network, no MCP dependency, exit 0 even when the
store is absent. They never block on a tool that may not be connected.

## The two tiers — who does what (don't conflate them)

| | **Memory (this playbook, Tier 1)** | **ContextCurator (`cc_*` MCP)** |
|---|---|---|
| Holds | agent-**derived durable facts**: decisions, gotchas, the "why" | **in-window** tool results & pins |
| Scope | **across sessions** (on disk) | **within the current window** |
| Action | recall at start / offload at boundaries | evict stale results / pin live ones |
| Availability | **always** (local files) | **when the MCP is connected** |
| Ownership | ours to operate (append-only store) | **the user's own product — use as-is, do not rebuild** ([[contextcurator-is-users-own-product]]) |

They are **complementary, not competing**. Memory is the durable, cross-session, always-available
substrate; ContextCurator is the in-window eviction/pinning layer. Tier-1 recall is the floor that
works even when ContextCurator is absent (cron/CI/headless). When ContextCurator *is* connected, it
handles in-window hygiene on top; this playbook does **not** drive `cc_evict`/`cc_pin` (that's
roadmap 6B, blocked on the product shipping) and does not duplicate its logic.

## Where memory lives

`~/.claude/projects/<slug>/memory/` — a `MEMORY.md` index plus one note file per fact. The `<slug>`
is the project path with separators/colons replaced by `-`
(`D:\…\command-center` → `D--MajorProjects-CURRENT-command-center`). Honour `CLAUDE_CONFIG_DIR` if set.

**Worktree note (important):** a git worktree under `<root>/.claude/worktrees/<name>` has its own
path *and its own slug*, but it is the **same logical project** and must share the parent's memory.
The tooling collapses that suffix back to `<root>` before computing the slug, so recall/offload
inside a worktree resolve to the **canonical project store** — not a stray per-worktree one. Slug
matching is case-insensitive (the same drive can surface as `D--` or `d--`).

## Memory-write discipline — what counts as a durable fact

Offload a fact **only** when it is durable and not otherwise recoverable:

- **DECISION + rationale** — "we chose X over Y because Z" (the trade-off, not just the choice).
- **GOTCHA / failure mode** found the hard way — so a future session doesn't rediscover it
  (e.g. *skill-creator's optimizer can't launch `claude.cmd` on Windows*).
- **"WHY"** that the code and docs don't explain on their own.

**Do NOT offload:**

- Transient task state (use `handoff`/`end-session` for that).
- Anything already in the repo — **repo git is the source of truth for documents** (ROADMAP §3
  guardrail 1). Memory holds agent-derived facts, **not doc copies** → no drift.
- Secrets, tokens, or anything sensitive.

Keep each note tight: a one-line **summary** (this is what the recall hook injects) plus a short
body with the reasoning. The store is **append-only under normal operation** — notes accrue, they
aren't bulk-rewritten.

### When to recall vs. offload

- **Recall:** automatically at **every session start** (the `SessionStart` hook below). The agent
  then pulls a *full* note on demand only when a task needs its detail — the index keeps the
  start-of-session injection small.
- **Offload:** at a **natural boundary** — end of a build phase, end of a spike, before a handoff,
  or the moment a non-obvious decision/gotcha is settled. This is roadmap **6D** ("proactive
  checkpoint at boundaries") realised for durable facts; it pairs with `handoff`/`end-session`,
  which capture *transient* state.

## Tooling

Both scripts live in [`tools/context-offload/`](../../tools/context-offload/). Stdlib only — run
with the system `python` (no `uv sync` needed; they have no dependencies).

### `recall.py` — emit durable facts at session start

```
python tools/context-offload/recall.py [--cwd PATH] [--format hook|plain|json] [--max-notes N]
```

- `--format hook` (default) wraps the index in `<session-memory>…</session-memory>` for context
  injection; `plain` is human-readable; `json` is machine-readable.
- Emits **nothing** (exit 0) when the project has no memory — a fresh project's session start stays
  clean and the hook never fails.
- Worktree-aware and case-insensitive slug match (see above).

### `offload.py` — write one durable fact

```
python tools/context-offload/offload.py --title "..." --summary "..." \
    [--slug ...] [--type project|reference] [--body "..." | --body-file PATH] \
    [--cwd PATH] [--session-id ID] [--dry-run]
```

- Creates `<slug>.md` with frontmatter matching the existing note format and **upserts** the
  `MEMORY.md` index entry. **Idempotent** — re-running with the same `--slug` updates that one note
  and does not duplicate the index line; unrelated notes are untouched.
- `--dry-run` prints what it would write without touching disk.
- Picks up `CLAUDE_SESSION_ID` for `originSessionId` provenance if present.

The agent can also just write a note by hand in the existing format — `offload.py` is the
discipline-enforcing, idempotent convenience, not the only path.

## Automation — the SessionStart hook (contract request to Lane Z)

Recall is automated by a **`SessionStart` hook** that runs `recall.py` and injects its stdout.
Lane C does **not** edit `~/.claude/settings.json` (single-owner: Lane Z). **Contract request filed
to Lane Z** — add this entry to the `hooks` block of `~/.claude/settings.json`:

```json
"SessionStart": [
  {
    "matcher": "",
    "hooks": [
      {
        "type": "command",
        "command": "python \"C:\\Users\\barclay\\.claude\\tools\\context-offload\\recall.py\" --format hook",
        "timeout": 5
      }
    ]
  }
]
```

- **Event:** `SessionStart`. **Matcher:** `""` (all session starts). **Timeout:** `5`s (the script
  is local file I/O; it self-degrades to empty output well within this).
- **Deploy step (Lane Z or install):** copy `tools/context-offload/` →
  `C:\Users\barclay\.claude\tools\context-offload\` so the hook's absolute path resolves. (The repo
  copy under `tools/` is the source of truth; the `~/.claude` copy is what the hook executes,
  mirroring the existing `claude-cache-countdown` hook layout in `~/.claude/tools/`.) Alternatively
  point the hook's path straight at the repo checkout.
- A `SessionStart` hook's stdout is added to the session context, so the `<session-memory>` block
  surfaces durable facts before the user's first prompt — recall **without manual prompting**.

Offload is **not** hooked to an event — it fires at *semantic* boundaries the agent recognises
(phase/spike end), not at a fixed harness event, so it stays an agent-invoked action.

## Headless / degradation behaviour

- **No ContextCurator MCP:** unaffected — Tier 1 is the always-available floor; recall/offload are
  local-file only.
- **No memory store yet:** `recall.py` emits nothing and exits 0; the session starts clean.
- **Unreadable store / parse error:** caught; recall exits 0 (never fails a session start).
- **Console codepage:** output is forced to UTF-8 so em-dashes in summaries aren't mangled.

## Verification (real output, 2026-06-09)

Run from inside the Lane C worktree
(`…/command-center/.claude/worktrees/feat+context-offload-t1`) with **no `--cwd`** — exercising the
worktree→canonical collapse the `SessionStart` hook will hit:

```
$ python tools/context-offload/recall.py
<session-memory>
Durable facts recalled from this project's local memory store (Tier-1 context offload). These are agent-derived decisions, gotchas, and "why" — treat as known; recall the full note only when a task needs its detail.
Store: C:\Users\barclay\.claude\projects\d--MajorProjects-CURRENT-command-center\memory

- Command Center North Star (command-center-north-star.md) — the mission + the membership test for what belongs in the Command Center
- ContextCurator is the user's own product (contextcurator-is-users-own-product.md) — integrate the cc_* MCP when it ships; don't rebuild eviction/pinning
- skill-creator optimizer broken on Windows (skill-creator-optimizer-broken-on-windows.md) — run_eval.py can't launch claude.cmd; don't retry the description optimizer here
</session-memory>
```

The three prior durable facts surface **from inside a worktree, with no manual prompting and no
`--cwd` argument** — confirming session-start recall resolves to the canonical project store. Done.

Graceful degradation, idempotency, and the offload→recall round-trip were verified against
throwaway `CLAUDE_CONFIG_DIR` dirs (the real store was never written to during development):

- `recall.py --cwd <no-memory-path>` → empty stdout, exit 0.
- `offload.py` run 3× (one repeated slug) → 2 index entries, the repeat **updated** in place.
- `offload.py` then `recall.py` on the same throwaway store → the written facts read back.
