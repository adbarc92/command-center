# Lane A2 — Agent-level API rate-limit auto-retry

> Paste this entire file as the prompt for a single agent. It is self-contained. Roadmap item: **5**.

## Your worktree (set up first)

```bash
git worktree add .claude/worktrees/feat+agent-rate-limit-retry -b feat/agent-rate-limit-retry main
cd .claude/worktrees/feat+agent-rate-limit-retry
```

(If your harness creates the worktree for you, just confirm you are on `feat/agent-rate-limit-retry`,
not `main`. **Branch name matters:** do not reuse `feat/rate-limit-resilience` — that's the already-merged
*fleetd-level* work; this lane is a different layer.)

## Goal

Automatic, periodic retries when the **Anthropic API** returns "Server is temporarily limiting
requests" (429s), at the **harness / agent level** — i.e. the agent's own API calls, **not** the
fleet's.

## Critical scoping — do NOT duplicate existing work

The repo **already has fleetd-level rate-limit resilience** (merged `feat/rate-limit-resilience`:
`fleetd` backoff/retry in `agent_exec`, the cockpit rate-limited chip, backoff tests). **That layer
is done — do not touch it or re-implement it.** This lane is the *agent's own* API retry, a distinct
layer above the fleet.

## Step 1 — Spike the retry surface BEFORE building (the real open question)

You do **not** yet know where the agent's own API retry is configured. **Investigate first; don't
assume.** Determine which of these it is:

- a `~/.claude/settings.json` **env var / setting**, or
- a **CLI flag** on the agent invocation, or
- a **wrapper** the agent calls through.

Find where the harness's own API retry/backoff actually lives, document your finding, *then* design
the minimal mechanism. The mechanism you build depends entirely on this answer.

## Coordinate with the cache window

Retry/backoff should stay **within** the ~5-minute prompt-cache TTL where possible, so a transient
429 doesn't force a cold, full-cost re-read. (This pairs with Lane A1's cache timer.) Document the
backoff schedule and how it respects the window.

## Owns (exclusive write)

- `tools/rate-limit-retry/**` — your implementation (wrapper/script/config) **and** a
  `tools/rate-limit-retry/FINDINGS.md` documenting the spike: where the retry surface lives, the
  mechanism chosen, and the backoff schedule.
- If the answer turns out to be "just a `settings.json` env knob," then this lane produces mainly
  `FINDINGS.md` + the exact contract request to Z — that's a fine, small outcome. Say so plainly.

## Reads (no write)

- [`docs/ROADMAP.md`](../../ROADMAP.md) §5.
- The existing fleetd rate-limit work (read to understand the boundary — do not modify).

## Shared contract — you do NOT write `settings.json`

If the mechanism is a `settings.json` **env/setting**, file a **contract request to Lane Z** with the
exact key/value to add. Do not edit `settings.json` yourself.

## Done when

- A **simulated API 429** auto-retries with backoff that stays inside the cache window.
- `FINDINGS.md` documents the retry surface, the mechanism, and the backoff schedule (no `TBD`s).

## Verify (run, paste real output)

- Demonstrate the retry against a **simulated 429** (mock or injected) and show the backoff timing
  in output — confirm it stays within ~5 min.
- If the mechanism is a `settings.json` entry, print the exact entry (this is your contract request to Z).

## Notes / open questions

- The spike (Step 1) is the crux — budget time for it. Report the answer even if it makes the build trivial.
- Don't conflate with fleetd. If you find yourself in `fleetd`/`fleet-core` code, you're in the wrong layer — stop and report.

---

## Rules of the Road (follow exactly)

1. **Stay in your lane.** Write only files under **Owns**. Never edit another lane's files or any
   global file — file a contract request instead. **Never touch the merged fleetd rate-limit code.**
2. **Worktree per lane.** Work on `feat/agent-rate-limit-retry`; never commit to `main`.
3. **Global/shared files are append-only + single-owner.** You own none — request entries.
4. **Don't widen scope.** Item 5 only. Anything else → report, don't do.
5. **Verify before done.** Run the Verify checks; paste real output.
6. **Report for integration.** End with: files changed; any **contract request to Z**; your verify
   output; the spike finding; anything affecting another lane (esp. Lane A1, the cache window).
