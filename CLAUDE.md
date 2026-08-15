# CLAUDE.md

<!-- BEGIN: ACTIVE-SESSION-PICKUP — remove this block when feat/plugin-runtime (PR #49) merges to main -->
## Active session pickup

If the current branch is `feat/plugin-runtime` (check with `git rev-parse --abbrev-ref HEAD`), read
the **State summary** in [`docs/STATUS.md`](docs/STATUS.md) and the **"Smoke run 2"** section of
[`spikes/SPIKE-RESULTS.md`](spikes/SPIKE-RESULTS.md) before doing anything else.

The one-line version: **Smoke run 2 finished Part 1, and `db74a47` is CONFIRMED** (responsiveness
measured: 1,127 samples at 1 Hz, zero unresponsive). The smoke found **four defects that every
automated gate had passed**; two are fixed and verified in a watched window (`55b0a5b` view-plugin
Windows URL, `2ab1b49` shutdown exit loop). **#49 is still draft**, now blocked on **D-7**
(view-plugins receive no state — `DataCloneError` posting Svelte 5 `$state` proxies; **agreed as the
next action**), **D-2** (capability negotiation is inert — every plugin is granted every host
capability), and **the packaged Part 2, which has still never been run**.

Traps, updated:
- **Items 1.2, 1.4a and 1.7 are BLOCKED, not broken**, behind **D-3**: fleetd serves no CORS headers,
  so no browser `fetch` from the cockpit reaches the daemon. Pre-existing on `main` — the FLEET ops
  grid renders nothing because of this. Don't chase it as a #49 regression.
- **Assert Gate 5 with `docker ps -a`, not `docker ps`.** `docker ps` cannot see the `Created` /
  `Exited` residue teardown leaves, and that residue breaks the *next* launch with a name conflict.
- **"Images are prebuilt so there's no build" is false** — `compose build` runs regardless.
- **Quit the cockpit gracefully, never `Stop-Process`,** when testing Gate 5 — a force-kill skips
  `stop_all_owned` and fabricates a teardown failure.
- The old "a running dev app blocks any rebuild of the tauri crate" trap was a *symptom* of the
  exit-loop defect and should be largely gone since `2ab1b49`. Quit the app before building anyway.

If the branch has changed, this section and its links are stale — delete this block.
<!-- END: ACTIVE-SESSION-PICKUP -->

Project conventions live in the global `~/.claude/CLAUDE.md` and this project's memory store
(`MEMORY.md` index, auto-loaded at session start).

Resumable per-session state is surfaced automatically at session start by the **session-state plugin**
(`plugins/session-state/`); run `/save-state` to record a narrative checkpoint for the next session.
