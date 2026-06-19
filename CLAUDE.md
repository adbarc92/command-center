# CLAUDE.md

Project conventions live in the global `~/.claude/CLAUDE.md` and this project's memory store
(`MEMORY.md` index, auto-loaded at session start). This file currently holds only an active-session
pickup pointer.

<!-- BEGIN: ACTIVE-SESSION-PICKUP — remove this block when the P3 spike is decided (GO/NO-GO recorded) -->
## Active session pickup

We paused **mid-debug of the P3 app-plugin webview spike**. Before doing anything else, read
[`docs/handoff/2026-06-15-P3-spike-resume.md`](docs/handoff/2026-06-15-P3-spike-resume.md). It documents:

- the **active bug** (the harness `Show` button hangs — `spike_show` likely deadlocks as a sync Tauri
  command calling `window.add_child`; tracing is already committed; **one observation confirms it, then
  make the command `async`**),
- the **exact cold-start runbook** (Audience on `:3000`, the cockpit harness in the worktree),
- the **spike work is in a worktree** — `.claude/worktrees/agent-a709aaf1bcad07d41` on branch
  `spike/app-plugins-webview-v2` @ `59470d7` — **not** on `main`,
- Gate 2–5 status (none observed yet) and the Gate-5-live plan, and
- the pre-P3 product-audit results (build green on `main`; remaining work is all human-gated).

Docker infra (postgres/redis/minio) was left running. If the P3 decision has since been recorded to
`spikes/SPIKE-RESULTS-app-plugins.md`, this block is stale — delete it (and this file if otherwise empty).
<!-- END: ACTIVE-SESSION-PICKUP -->
