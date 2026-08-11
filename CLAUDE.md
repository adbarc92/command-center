# CLAUDE.md

<!-- BEGIN: ACTIVE-SESSION-PICKUP — remove this block when feat/plugin-runtime (PR #49) merges to main -->
## Active session pickup

If the current branch is `feat/plugin-runtime` (check with `git rev-parse --abbrev-ref HEAD`), read
the **State summary** in [`docs/STATUS.md`](docs/STATUS.md) and the **"Smoke run 1"** section of
[`spikes/SPIKE-RESULTS.md`](spikes/SPIKE-RESULTS.md) before doing anything else.

The one-line version: **PR #49's interactive smoke is ~2 of 11 items done.** Item 1.5 failed with a
UI-freezing defect (`plugin_launch` ran the whole docker build on the main event-loop thread); it is
root-caused, fixed and regression-tested in `db74a47`, **but that fix has only ever been verified by
automated gates — never in a watched window.** Re-running 1.5 and finishing the rest of the checklist
is the next action, and #49 stays draft until it's done.

Two traps that cost time this session and will again:
- **A running dev app blocks any rebuild of the tauri crate** — `tauri-build` can't overwrite
  `target/debug/fleetd-serve.exe` while it's the running sidecar, and fails `PermissionDenied`. Quit
  the app before building.
- **Quit the cockpit gracefully, never `Stop-Process`,** when testing Gate 5 — a force-kill skips
  `stop_all_owned` and fabricates a teardown failure.

If the branch has changed, this section and its links are stale — delete this block.
<!-- END: ACTIVE-SESSION-PICKUP -->

Project conventions live in the global `~/.claude/CLAUDE.md` and this project's memory store
(`MEMORY.md` index, auto-loaded at session start).

Resumable per-session state is surfaced automatically at session start by the **session-state plugin**
(`plugins/session-state/`); run `/save-state` to record a narrative checkpoint for the next session.
