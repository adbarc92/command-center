---
name: save-state
description: Save the current dev-session's resumable state (what we did, next steps, open threads) to the per-repo session-state timeline so the next session can resume instantly. Use at the end of a work session, at a phase/spike boundary, or when the user says "save state", "checkpoint", or before ending a session.
---

# Save Session State

Append an agent-authored **rich** record to this repo's session-state timeline. Auto git facts are
already captured by hooks; this records the *meaning* — what got done, where we paused, what's next,
and which threads are open.

## Steps

1. Compose the narrative from the current session:
   - `did`: 1-3 sentences — what was accomplished and where work paused.
   - `next`: a list of concrete next actions.
   - `open_threads`: active bugs, blockers, pending decisions, things to watch.
2. Write it to a temp JSON file (use the OS temp dir; pick a unique name):
   ```json
   { "did": "...", "next": ["..."], "open_threads": ["..."] }
   ```
3. Run the capture script (direct python; path = the installed tool):
   ```
   python "%USERPROFILE%/.claude/tools/session-state/src/session_state/capture_rich.py" --input <tempfile>
   ```
   (Bash: `python "$HOME/.claude/tools/session-state/src/session_state/capture_rich.py" --input <tempfile>`)
4. Read the script's output:
   - "narrative saved." → done. The temp file was deleted for you.
   - "narrative NOT saved … Temp preserved at <path>" → tell the user; do **not** blind-retry. Surface
     the printed retry command.

## Notes
- This complements `end-session`/`handoff`; run it as the final step when ending a session.
- The next session's SessionStart hook will surface this automatically.
