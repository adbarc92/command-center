# cache-countdown — cache-aware approval timer

Command Center roadmap **item 1** (cache-aware approval timer) + **6E** (cache-window pacing).

A live countdown of the ~5-minute Anthropic prompt-cache TTL, shown whenever a
task is **awaiting user approval**, so you respond before the cache goes cold and
a large session is re-read at full cost. Shows `🔥 HOT → 🟢 → 🟡 → 🔴 → ❄️ COLD`
plus **cost-at-stake** (e.g. `🔴 0:45 $5.75`), with escalating bell alerts at
60 / 30 / 10 seconds.

Reference implementation: [`KatsuJinCode/claude-cache-countdown`](https://github.com/KatsuJinCode/claude-cache-countdown).

## How it works

```
Stop hook ─────────────► writes cache-timer-<session>.json  (stopped=true,  ts=now)
UserPromptSubmit hook ─► writes cache-timer-<session>.json  (stopped=false, ts=now)
                                   │
                                   ▼
        ticker (uv run cache-countdown) reads it every second,
        counts down 295 − elapsed, renders the line, rings the bells
```

- **`stopped=true`** (Stop hook) → task is awaiting approval → countdown is live.
- **`stopped=false`** (UserPromptSubmit) → session active → cache re-warmed → `🔥 HOT`.

State file path: `~/.claude/state/cache-timer-<session_id>.json`.

## Layout

| Path | What |
|---|---|
| `hooks/cache-timer-write.ps1`  | Stop hook — marks the cache draining |
| `hooks/cache-timer-resume.ps1` | UserPromptSubmit hook — marks the cache warm again |
| `src/cache_countdown/core.py`  | Pure logic: parse, remaining, stage, cost, bells |
| `src/cache_countdown/ticker.py`| The loop: clock + terminal + bell; `cache-countdown` entry point |
| `pyproject.toml`               | UV package (`uv run cache-countdown`) |
| `tests/`                       | pytest unit tests |
| `install.ps1`                  | Installer; prints the two settings.json hook entries |
| `PACING.md`                    | Roadmap 6E — the warm/cool/retry-within-window rule |

## Install

```powershell
pwsh -NoProfile -File install.ps1
```

Copies the scripts + ticker to `~/.claude/tools/cache-countdown/`, runs `uv sync`,
and **prints** the two hook entries for `settings.json`. It does **not** edit
`settings.json` — the orchestrator (Lane Z) pastes the printed entries.

## Run the ticker

```powershell
uv run --project ~/.claude/tools/cache-countdown cache-countdown --session <session-id>
# or against a specific state file:
uv run cache-countdown --file ~/.claude/state/cache-timer-<id>.json
# self-test (synthetic clock — walks every stage, rings every bell):
uv run cache-countdown --self-test
```

## Test

```powershell
uv run pytest
```

## Cost-at-stake basis

Cost-at-stake = the dollars re-spent if the cache goes cold and the session
prefix is re-read at full input price instead of the cheap cache-read price.
At the Command Center default model (Claude Opus 4.8): `$5.00/1M` full input −
`$0.50/1M` cache read = **`$4.50` per 1M cached tokens**. See `PACING.md` and
`COST_PER_TOKEN_AT_STAKE` in `core.py`. The token count comes from an optional
`cached_tokens` field in the state file; when absent, cost is omitted rather
than fabricated.
