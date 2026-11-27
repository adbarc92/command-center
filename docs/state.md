<!-- graph:state -->
# State

What survives between sessions. Read at the start of a session, append at the end.
Everything here comes from the project's own docs; follow the links for detail.

## Current

- Control plane and workflow layer are feature-complete and tested; the repo is public and `main` is branch-protected with `cargo test (workspace)` required. See [STATUS.md](STATUS.md).
- Interactive smoke finished (run 3 packaged, 2026-08-16); PR #49 (plugin runtime) merged. See [STATUS.md](STATUS.md).
- Telltale integration built (#65-#67); Telltale itself moved to its own repo on 2026-08-30. See [the pivot handoff](handoffs/31f0a85d-8bcc-4d27-a849-e9e950749558.md).
- Locked build order (auth-foundation-first): Local-Tracker Phase 2 dispatch, embedding swarms (shipped), design overhaul, Remote Control. See [ROADMAP.md](ROADMAP.md).

## Decisions

- Membership test: a feature belongs only if it serves the north star. See [ROADMAP.md](ROADMAP.md).
- fleetd serves a CORS allowlist, never `Any` (#68). See [STATUS.md](STATUS.md).
- Embargo guard removed 2026-08-30 and dropped from required checks. See [STATUS.md](STATUS.md).
- Telltale cards are `Idle`, never `Build`; `Blocked` only for an unassigned open crash issue. See [STATUS.md](STATUS.md).

## Dead ends

- The fleetd spend-cap race was a test-scenario flake, not a broken cap; the swallowed `.ok()` was exonerated as the cause (#72). See [STATUS.md](STATUS.md).
- Telltale built inside this repo (PR #64) was the wrong repository; both #64 and #63 closed unmerged. See [STATUS.md](STATUS.md) and [the pivot handoff](handoffs/31f0a85d-8bcc-4d27-a849-e9e950749558.md).
- Gate 5 success judged by `docker ps` missed residue; use `docker ps -a`. See [STATUS.md](STATUS.md).

## Open

- Fix-or-accept decision on `stop_all_owned(30_000)` running synchronously in the exit callback. See [STATUS.md](STATUS.md).
- A GitHub Support ticket to garbage-collect unreachable old digest objects. See [STATUS.md](STATUS.md).
- Six testing-plan items await human ratification; a full testing-plan re-run is the top next action. See [testing/PLAN.md](testing/PLAN.md).
- Human-gated tasks: spikes, live paid run, signing certs. See [the spikes handoff](handoff/2026-06-25-spikes-handoff.md).
