---
name: save-state
description: Save the current dev-session's resumable state (what we did, next steps, open threads) to the per-repo session-state timeline so the next session resumes instantly. Use at the end of a work session, at a phase/spike boundary, or when the user says "save state", "checkpoint", or before ending a session.
---

# Save Session State

Append an agent-authored **rich** record to this repo's session-state timeline (auto git facts are
captured by hooks; this records the *meaning*).

## Steps

1. Compose the narrative from this session:
   - `did`: 1-3 sentences — what was accomplished and where work paused.
   - `next`: list of concrete next actions.
   - `open_threads`: active bugs, blockers, pending decisions, things to watch.
2. Write it to a uniquely-named temp JSON file in the OS temp dir:
   ```json
   { "did": "...", "next": ["..."], "open_threads": ["..."] }
   ```
3. Resolve the plugin's script path from the registry (the plugin's install dir is version-stamped, so
   read it rather than hardcode). Read `~/.claude/plugins/installed_plugins.json` (honor
   `CLAUDE_CONFIG_DIR`), key `"session-state@command-center"`, take the first entry's `installPath`.
   If that path doesn't exist, scan `~/.claude/plugins/cache/command-center/session-state/` for the
   newest version dir containing `src/capture_rich.mjs`.
4. Run: `node "<installPath>/src/capture_rich.mjs" --input "<tempfile>"`
5. Read the output:
   - "session-state: narrative saved." → done (the temp file was deleted for you).
   - "narrative NOT saved … Temp preserved at <path>" → tell the user; do NOT blind-retry; surface the
     printed retry command.

## Notes
- This is invoked by you (the model) via this skill's description — it is not auto-called by
  `end-session`. Run it at the end of a session or a phase boundary.
- The next session's SessionStart hook surfaces this automatically.
