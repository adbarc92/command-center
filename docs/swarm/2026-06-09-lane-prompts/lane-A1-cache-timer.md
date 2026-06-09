# Lane A1 — Cache-aware approval timer + pacing

> Paste this entire file as the prompt for a single agent. It is self-contained — you need no other
> context. Roadmap items: **1** (cache-aware approval timer) + **6E** (cache-window pacing).

## Your worktree (set up first)

You work in an **isolated git worktree** off `main`. Never commit to `main`.

```bash
git worktree add .claude/worktrees/feat+cache-timer -b feat/cache-timer main
cd .claude/worktrees/feat+cache-timer
```

(If your harness creates the worktree for you, skip the command and just confirm you are on a
`feat/cache-timer` branch, not `main`.)

## Goal

Make the ~5-minute Anthropic prompt-cache window **visible and economical**: show a live countdown
whenever a task is **awaiting user approval**, so the user responds before the cache goes cold and a
large session has to be re-read at full cost. Plus document the cache-window pacing rule (6E).

## Mechanism (reference impl: `KatsuJinCode/claude-cache-countdown`)

- A **`Stop` hook** and a **`UserPromptSubmit` hook** write `~/.claude/state/cache-timer-{session}.json`.
- A **ticker** reads it and counts down `295 − elapsed`, showing `🔥 HOT → 🟢 → 🟡 → 🔴 → ❄️ COLD`
  plus **cost-at-stake** (e.g. `🔴 0:45 $5.75`), with escalating bell alerts at 60/30/10s.
- **This box is Windows.** Write the installer + hook scripts in **PowerShell**; write the ticker in
  **Python, packaged and run via UV** (per the user's global rules — no `pip`, no `requirements.txt`;
  use `pyproject.toml` + `uv run`).
- **6E pacing:** document the rule — keep cache warm during active work, let it cool when idle,
  coordinate `ScheduleWakeup`/work cadence with the ~5-min TTL. Write this as
  `tools/cache-countdown/PACING.md`.

## Owns (exclusive write)

- `tools/cache-countdown/**` — PowerShell installer + hook scripts, the UV/Python ticker
  (`pyproject.toml`, ticker module), and `tools/cache-countdown/PACING.md`.

Create the directory; it does not exist yet.

## Reads (no write)

- [`docs/ROADMAP.md`](../../ROADMAP.md) §1 and §6E (available on `main` after Step 0).
- The reference impl `KatsuJinCode/claude-cache-countdown` (web).

## Shared contract — you do NOT write `settings.json`

`~/.claude/settings.json` is owned by **Lane Z (the orchestrator)**. Do **not** edit it. Instead,
in your final report, **file a contract request** giving Z the exact entries to add:

- a **`Stop`** hook entry (event, matcher, exact `command` invoking your installed script), and
- a **`UserPromptSubmit`** hook entry (same shape).

Provide the precise command strings and absolute/`~`-relative script paths you produced, so Z can
paste them in unchanged.

## Done when

- Triggering an awaiting-approval state shows a **live countdown + cost-at-stake** in the terminal,
  ticking through `🔥→🟢→🟡→🔴→❄️` with the 60/30/10s bells.
- The installer would add **exactly two** hook entries to `settings.json` (you don't apply them — Z does).
- `tools/cache-countdown/PACING.md` states the warm/cool/retry-within-window rule.

## Verify (run, paste real output)

- `uv run` the ticker against a hand-written `~/.claude/state/cache-timer-test.json` and confirm it
  counts down through the color stages and fires the bells at the thresholds.
- Print the **two hook entries** your installer would write (as JSON) — this doubles as your contract
  request to Z.

## Notes / open questions

- The exact `settings.json` hook schema (event names, matcher format, command quoting on Windows) —
  verify against the live `~/.claude/settings.json` shape; hand Z entries that match it exactly.
- "Cost-at-stake" needs a token→$ basis — document your assumption (model + per-token rate) rather
  than hard-coding a number silently.

---

## Rules of the Road (follow exactly)

1. **Stay in your lane.** Write only files under **Owns** above. Never edit another lane's files or
   any global file (`~/.claude/settings.json`, `~/.claude/CLAUDE.md`) — file a contract request instead.
2. **Worktree per lane.** Work on `feat/cache-timer`; never commit to `main`.
3. **Global/shared files are append-only + single-owner.** You own none — request entries.
4. **Don't widen scope.** Build only items 1 + 6E. Anything else you spot → report it, don't do it.
5. **Verify before done.** Run the Verify checks; paste the real output, not an assertion.
6. **Report for integration.** End with: files changed; your **contract request to Z** (the two hook
   entries, verbatim); your verify output; anything affecting another lane.
