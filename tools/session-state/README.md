# session-state

Per-repo dev-session state capture + zero-friction resume for Claude Code. Spec:
`docs/superpowers/specs/2026-06-19-session-state-resume-design.md`.

## Install

```
pwsh -NoProfile -File tools/session-state/install.ps1
```

Wires three hooks into `~/.claude/settings.json` (SessionStart→resume, Stop→scratch,
SessionEnd→boundary), invoking `python.exe` directly. Re-running is idempotent.

## Uninstall

```
pwsh -NoProfile -File tools/session-state/install.ps1 -Uninstall          # remove hooks
pwsh -NoProfile -File tools/session-state/install.ps1 -Uninstall -Purge   # also delete state
```

## Disable temporarily

Set `CC_SESSION_STATE_DISABLE=1` in a shell to make all hooks no-op there.

## Inspect

```
python src/session_state/cli.py list
python src/session_state/cli.py show [<path-or-repo-key>]
python src/session_state/cli.py prune [<path-or-repo-key>]
```

## Rich records (the narrative)

Auto git facts are captured by hooks. The **narrative** (did/next/open_threads) is written by the
`/save-state` skill — run it at the end of a session or a phase boundary. When using the `end-session`
skill, run `/save-state` as a follow-up step before ending so the next session can resume with full
context. The resume block will remind you if no narrative was captured
("_(no narrative captured yet — run /save-state to record one)_").
