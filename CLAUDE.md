# CLAUDE.md

<!-- BEGIN: ACTIVE-SESSION-PICKUP — remove this block when feat/view-plugin-bridge-handshake merges to main -->
## Active session pickup

If the current branch is `feat/view-plugin-bridge-handshake` (check with `git rev-parse --abbrev-ref HEAD`),
read [`docs/handoff/2026-06-29-view-plugin-bridge-handshake-tdd.md`](docs/handoff/2026-06-29-view-plugin-bridge-handshake-tdd.md)
before doing anything else. It documents:

- the **P4 handshake determination** (host held the transferred MessagePort but never called `port.start()`
  → 100% handshake failure; proven by mutation, hidden by jsdom's lenient ports),
- the host handshake built test-first (`cockpit/ui/src/lib/bridge.ts` + `bridge.test.ts` + the faithful
  `bridge.testkit.ts`; 76 tests green, `npm run check` clean),
- what's next (confirm the determination, then build the plugin SDK `connect()` — the mirror flaw), and
- the platform caveat (WebView2 MessagePort-into-sandbox is unprovable in jsdom; manual packaged gate still required).

If the branch has changed, this section and the linked doc are stale — skip them and delete this block.
<!-- END: ACTIVE-SESSION-PICKUP -->

Project conventions live in the global `~/.claude/CLAUDE.md` and this project's memory store
(`MEMORY.md` index, auto-loaded at session start).

Resumable per-session state is surfaced automatically at session start by the **session-state plugin**
(`plugins/session-state/`); run `/save-state` to record a narrative checkpoint for the next session.
