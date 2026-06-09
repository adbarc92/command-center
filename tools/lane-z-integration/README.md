# Lane Z — global-config integration (review before applying)

This directory is the **orchestrator's integration step** for the 2026-06-09 roadmap swarm. The five
lane PRs (#7–#11) own only *repo* files. Three of them also need entries in your **personal global
config** — files outside the repo with **no git rollback**:

- `~/.claude/settings.json` (hooks + env)
- `~/.claude/CLAUDE.md` (standing rules)

Per the swarm's single-owner rule, **no dispatched agent wrote these** — they filed contract
requests, collected here. Nothing in this PR touches your machine. `deploy_globals.py` is
**dry-run by default**; it only writes when you pass `--apply`.

## Order of operations

1. **Merge the lane PRs first** (#7 cache-timer, #10 context-offload at minimum) so the tools exist
   on `main`. The deploy copies `tools/cache-countdown/` and `tools/context-offload/` from the repo
   into `~/.claude/tools/`.
2. Check out `main`, then **dry-run**: `python tools/lane-z-integration/deploy_globals.py`
3. Review the printed plan, then **apply**: `python tools/lane-z-integration/deploy_globals.py --apply`

The script backs up `settings.json` and `CLAUDE.md` (timestamped `.bak`) before any write.

## The collected contract requests

| Source | Target | Change | Conflict? |
|---|---|---|---|
| Lane C | `settings.json` `hooks.SessionStart` | add memory-recall hook (`context-offload/recall.py`) | none — additive |
| Lane B | `~/.claude/CLAUDE.md` | append "Budget-Discipline Standing Rules" block | none — additive, idempotent |
| Lane A2 | `settings.json` `env.CLAUDE_CODE_MAX_RETRIES` | currently `"20"`; A2 *suggests* `"10"` | your call (opt-in flag) |
| Lane A1 | `settings.json` `hooks.Stop` + `UserPromptSubmit` | wire the new cache-timer | **YES — see below** |

## The one real conflict — cache timer (Lane A1)

Your live `settings.json` **already runs an older `claude-cache-countdown` install**:

- `hooks.Stop` + `hooks.UserPromptSubmit` → `~/.claude/tools/claude-cache-countdown/hooks/*.ps1`
- `statusLine` → `~/.claude/tools/claude-cache-countdown/statusline.py`

Lane A1 built a **new, separate** `cache-countdown` tool (note: no `claude-` prefix): a UV/Python
terminal ticker with **cost-at-stake** and 60/30/10s bells. It is **not** a drop-in replacement —
**A1 did not build a `statusline.py`**, so adopting A1's hooks without keeping the old install would
leave your status line pointing at a path you might remove.

**This is your decision. Three options:**

1. **Keep old, ignore A1 (default).** The script leaves Stop/UserPromptSubmit/statusLine untouched.
   A1's tool ships in the repo, unused, until you decide. *Safest; loses A1's cost-at-stake.*
2. **Adopt A1's hooks, keep old statusLine** (`--adopt-new-cache-timer`). Repoints Stop +
   UserPromptSubmit to the new `cache-countdown`; leaves `statusLine` on the old install (so it keeps
   working). You then have the new ticker's cost-at-stake **and** the old status line. *Recommended
   if you want A1's value with zero breakage.*
3. **Fully migrate.** Port a `statusline.py` into A1's tool, repoint everything, delete the old
   install. Not scripted here — a follow-up.

The default merge is **additive-only** (option 1 + Lane C's SessionStart + Lane B's CLAUDE.md). The
A1 adoption and the retries change are **opt-in flags** so you choose explicitly.

## Files here

- `settings-changes.proposed.md` — the exact JSON deltas, eyeball-able.
- `claude-md-block.md` — Lane B's block, verbatim, as it will be appended.
- `deploy_globals.py` — stdlib, dry-run by default; flags below.

```
python tools/lane-z-integration/deploy_globals.py            # dry-run, additive-only plan
python tools/lane-z-integration/deploy_globals.py --apply    # apply additive-only
python tools/lane-z-integration/deploy_globals.py --apply --adopt-new-cache-timer   # + option 2
python tools/lane-z-integration/deploy_globals.py --apply --set-retries 10          # + A2's suggestion
```
