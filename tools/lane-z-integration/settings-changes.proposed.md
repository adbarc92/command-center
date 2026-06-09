# Proposed `~/.claude/settings.json` deltas

Eyeball-able view of exactly what `deploy_globals.py` changes. Everything not listed is left
**untouched** (your ~230 permission entries, plugins, statusLine, PostToolUse/StopFailure hooks).

## 1. Additive (default — always applied with `--apply`)

### `hooks.SessionStart` — NEW (Lane C, memory recall)

There is **no** `SessionStart` block today. Add:

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

Requires `~/.claude/tools/context-offload/recall.py` to exist — the script copies it from the repo
(merge PR #10 first). recall.py is stdlib-only and exits 0 even if the memory store is missing, so
this hook can never fail a session start.

## 2. Opt-in: `--adopt-new-cache-timer` (Lane A1, option 2 in README)

Repoints the **existing** Stop + UserPromptSubmit hooks from the old `claude-cache-countdown` path
to Lane A1's new `cache-countdown`. `statusLine` is **left on the old install** so it keeps working.

```diff
 "Stop": [{ "matcher": "", "hooks": [{ "type": "command",
-  "command": "pwsh.exe -NoProfile -File \"C:\\Users\\barclay\\.claude\\tools\\claude-cache-countdown\\hooks\\cache-timer-write.ps1\"",
+  "command": "pwsh.exe -NoProfile -File \"C:\\Users\\barclay\\.claude\\tools\\cache-countdown\\hooks\\cache-timer-write.ps1\"",
   "timeout": 5 }]}],
 "UserPromptSubmit": [{ "matcher": "", "hooks": [{ "type": "command",
-  "command": "pwsh.exe -NoProfile -File \"C:\\Users\\barclay\\.claude\\tools\\claude-cache-countdown\\hooks\\cache-timer-resume.ps1\"",
+  "command": "pwsh.exe -NoProfile -File \"C:\\Users\\barclay\\.claude\\tools\\cache-countdown\\hooks\\cache-timer-resume.ps1\"",
   "timeout": 5 }]}],
```

Without this flag, Stop/UserPromptSubmit/statusLine are **unchanged** (old install keeps running).

## 3. Opt-in: `--set-retries N` (Lane A2)

`env.CLAUDE_CODE_MAX_RETRIES` is `"20"` today. A2 *suggests* `"10"` for the widest cache-window
margin (worst-case ~55s ≪ 300s TTL). `"20"` also fits (~135s modeled). Only changed if you pass the
flag; default leaves `"20"`.

```diff
 "env": {
-  "CLAUDE_CODE_MAX_RETRIES": "20"
+  "CLAUDE_CODE_MAX_RETRIES": "10"   // only with --set-retries 10
 }
```
