# budget-checkpoint — 6D boundary-checkpoint Stop hook

Command Center roadmap **item 6D** ("Proactive checkpoint at boundaries", see
`docs/playbooks/budget-discipline.md`).

A minimal Claude Code **`Stop` hook** that, at a natural work boundary, surfaces a
**gentle, non-blocking nudge** to run `/end-session` or `/handoff` — so the next
session starts **compact** instead of re-loading a bloated conversation at full
cost. It never auto-ends a session and never blocks the turn.

This is an **optional** convenience feature. With the hook off, 6D still holds as
a behavioral rule the agent follows by hand; the hook just makes the
context-size trigger automatic.

## How it works

```
Stop event (JSON on stdin) ──► budget-checkpoint.ps1 ──► uv run budget-checkpoint
                                                              │
                                  reads transcript_path, counts turns
                                                              │
                       turns ≥ threshold (and stop_hook_active false)?
                                          │ yes                 │ no
                                          ▼                     ▼
              { "hookSpecificOutput": {              (emit nothing, exit 0)
                  "hookEventName": "Stop",
                  "additionalContext": "...nudge..." } }
```

The nudge is delivered via **`hookSpecificOutput.additionalContext`** — the
Claude Code hooks docs describe this as "non-error feedback that continues the
conversation". We deliberately do **not** use `{"decision":"block"}`: blocking a
Stop *prevents the turn from ending* and can force a nag loop, which is the
opposite of a gentle checkpoint reminder.

## Boundary heuristic (v1 — conservative, documented assumption)

> **The question 6D leaves open:** how do you detect a "boundary"? The playbook
> lists phase-complete, context-heavy, and explicit stop/handoff signals. Of
> these, the only one a Stop hook can read **cheaply and reliably** from the
> event itself is **context size** — the others need semantic understanding of
> the work the harness doesn't hand us.

So v1 uses a **transcript turn-count threshold** as a proxy for "context is
heavy, and this Stop is a clean cut point" (6D's "context-size threshold"
trigger):

- **Count turns** by reading the `transcript_path` JSONL the Stop event provides
  and counting `user`/`assistant` entries.
- **Nudge** once turns reach `DEFAULT_TURN_THRESHOLD` (default **60**), then only
  every `NUDGE_EVERY` turns (default **20**) thereafter — so a long session is
  reminded periodically, **not nagged on every Stop**.
- **Never nudge** when `stop_hook_active` is true (a previous Stop hook already
  acted — re-nudging there would be the exact loop 6D warns against), and never
  below the threshold (short sessions are cheap to resume).

**Why turn-count and not token-count?** The Stop payload doesn't include a token
total, and re-reading the full transcript to estimate tokens on every Stop would
itself cost time. Turn-count is a stable, zero-dependency proxy that's good
enough to flag "this conversation is getting long". Tune the two constants in
`core.py` (or via the `threshold` / `nudge_every` params) to taste.

**Assumptions, stated honestly:**
- The transcript is JSON Lines with one object per entry and a `type` field of
  `user`/`assistant` for real turns. If that schema shifts, `count_turns`
  returns a low/zero count and the hook simply **stays silent** (fails safe — it
  never errors the turn).
- If `transcript_path` is missing/unreadable, the hook is a no-op.

## Layout

| Path | What |
|---|---|
| `hooks/budget-checkpoint.ps1`    | Stop hook shim — pipes the event into the UV/Python hook |
| `src/budget_checkpoint/core.py`  | Pure logic: parse event, count turns, boundary decision, nudge text |
| `src/budget_checkpoint/hook.py`  | stdin→stdout envelope; `budget-checkpoint` entry point |
| `pyproject.toml`                 | UV package (`uv run budget-checkpoint`) |
| `tests/`                         | pytest unit tests (core + hook) |
| `install.ps1`                    | Installer; prints the settings.json Stop-hook entry |

## Install

```powershell
pwsh -NoProfile -File install.ps1
```

Copies the shim + package to `~/.claude/tools/budget-checkpoint/`, runs
`uv sync`, and **prints** the Stop-hook entry for `settings.json`. It does
**not** edit `settings.json` — the orchestrator (Lane C) pastes the printed
entry.

## Run / test the hook manually

```powershell
# Pipe a sample Stop event into the hook:
'{"session_id":"x","transcript_path":"C:\path\to\transcript.jsonl","stop_hook_active":false}' `
  | uv run budget-checkpoint
```

A short transcript prints nothing; a transcript past the threshold prints the
`additionalContext` nudge JSON.

## Test

```powershell
uv run pytest
```

## Stdlib-only / headless contract

No dependencies, no network, no MCP. Exit code is **always 0** so a malformed
event or transcript can never fail the user's turn — mirrors the
`context-offload` and `cache-countdown` hooks.
