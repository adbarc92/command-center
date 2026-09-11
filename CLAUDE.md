# CLAUDE.md

Project conventions live in the global `~/.claude/CLAUDE.md` and this project's memory store
(`MEMORY.md` index, auto-loaded at session start).

Resumable per-session state is surfaced automatically at session start by the **session-state plugin**
(`plugins/session-state/`); run `/save-state` to record a narrative checkpoint for the next session.
