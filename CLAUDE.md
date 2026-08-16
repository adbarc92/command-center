# CLAUDE.md

<!-- BEGIN: ACTIVE-SESSION-PICKUP — remove this block when feat/plugin-runtime (PR #49) merges to main -->
## Active session pickup

If the current branch is `feat/plugin-runtime` (check with `git rev-parse --abbrev-ref HEAD`), read
the **State summary** in [`docs/STATUS.md`](docs/STATUS.md) and the **"Smoke run 3"** section of
[`spikes/SPIKE-RESULTS.md`](spikes/SPIKE-RESULTS.md) before doing anything else.

The one-line version: **the smoke is finished. Part 1 and the packaged Part 2 have both been run,
and #49 is READY FOR REVIEW with all 18 CI checks green** — it is waiting on a human merge decision,
nothing else. Run 3 scored **9 PASS / 2 BLOCKED / 2 NOT RUN / 0 FAIL** and closed five defects:
**D-7** (view-plugins got no state — `DataCloneError` posting Svelte `$state` proxies), **D-8** (the
packaged bundle shipped **no plugin root at all**, so no shipped build could load a view-plugin),
**D-2** (every plugin was granted every capability; now fails closed), **D-4** (re-verified packaged:
exit in 0.23 s cold, 5.27 s with 10 containers), and **D-5** (investigated, **did not reproduce**).

Traps, updated:
- **Items 1.2 / 1.4a / 1.7 are BLOCKED, not broken**, behind **D-3**: fleetd serves no CORS headers,
  so no browser `fetch` from the cockpit reaches the daemon. **Pre-existing on `main`** — the FLEET
  ops grid renders nothing because of this. Don't chase it as a #49 regression; it needs its own issue.
- **Assert Gate 5 with `docker ps -a`, not `docker ps`.** `docker ps` cannot see the `Created` /
  `Exited` residue teardown leaves, and that residue breaks the *next* launch with a name conflict.
- **Quit the cockpit gracefully, never `Stop-Process`,** when testing Gate 5 — a force-kill skips
  `stop_all_owned` and fabricates a teardown failure.
- **A clean packaged run shows no AUDIENCE tab, and that is correct.** `PluginManager::roots()` has
  no packaged resource root by design; app-plugins come from `CC_APP_PLUGINS_DEV` or
  `~/.command-center/app-plugins`. Set `CC_APP_PLUGINS_DEV` to drive AUDIENCE, and record it as
  "packaged binary, dev discovery seam".
- **A release build has no devtools.** F12 does nothing, so any check whose criterion is a console
  reading is NOT RUN when packaged — decide that up front rather than mid-session.
- **"CI never builds the app" is FALSE.** `ci.yml:311` runs `tauri build` on all three OSes. What no
  gate does is look *inside* the bundle — which is exactly how D-8 passed a *successful* build. This
  is `GAP-132` in [`docs/testing/PLAN.md`](docs/testing/PLAN.md).
- **"Images are prebuilt so there's no build" is false** — `compose build` runs regardless. Though in
  run 3 the images already existed and the ramp took ~15 s, not 20 min.

If the branch has changed, this section and its links are stale — delete this block.
<!-- END: ACTIVE-SESSION-PICKUP -->

Project conventions live in the global `~/.claude/CLAUDE.md` and this project's memory store
(`MEMORY.md` index, auto-loaded at session start).

Resumable per-session state is surfaced automatically at session start by the **session-state plugin**
(`plugins/session-state/`); run `/save-state` to record a narrative checkpoint for the next session.
