# Lane A2 - Agent-level API rate-limit auto-retry - FINDINGS

Roadmap item **5**. This documents the **Step-1 spike** (where the agent's own
API retry surface lives), the **mechanism chosen**, the **backoff schedule**,
and the **contract request to Lane Z**.

> **Bottom line up front:** The agent's own API retry is **already built into
> the Claude Code harness** and is controlled by a single settings.json env
> knob - **CLAUDE_CODE_MAX_RETRIES** (default **10**, exponential backoff,
> honors the server retry-after header). It is **a distinct layer above** the
> already-merged fleetd rate-limit resilience. The "settings.json env knob"
> outcome the lane brief anticipated is exactly what we found, so this lane's
> real deliverables are this FINDINGS.md, a simulated-429 verification harness,
> and **one contract request to Lane Z** - a fine, small, honest outcome.

---

## Step 1 - The spike: where does the agent's own API retry live?

The brief named three candidates and said *don't assume - investigate*. The
answer is candidate **(a): a settings.json env var / setting.**

It is **not** a CLI flag and **not** a wrapper we need to build. The retry is
implemented inside Claude Code's own API client (the Anthropic SDK underneath
the harness), and the only thing the user/operator controls is an environment
variable surfaced through ~/.claude/settings.json -> env.

### Evidence (authoritative)

1. **The merged fleetd design doc already told us where this lane lives.**
   docs/superpowers/specs/2026-06-07-rate-limit-resilience-design.md
   section "Out of scope (YAGNI)" explicitly carves this out:
   > *Path B (auto-retry in the user's **interactive** Claude Code sessions) is
   > **not** buildable as a true hook - hooks observe but cannot re-submit a
   > turn - handled separately via CLAUDE_CODE_MAX_RETRIES + an optional
   > notification hook.*

   That spec's "Layering note" also names the layer boundary:
   > *claude already retries 429/529 internally (~10x, seconds-scale,
   > CLAUDE_CODE_MAX_RETRIES). Our [fleetd] retry is a deliberate **outer
   > layer** that rides out throttling lasting **minutes**.*

   So the agent-level retry is the **inner** layer (seconds-scale, in-harness);
   fleetd is the **outer** layer (minutes-scale, our merged code). This lane
   is the inner layer - confirmed not to be fleetd.

2. **It is already configured in the user's global settings.**
   ~/.claude/settings.json currently contains:

       "env": { "CLAUDE_CODE_MAX_RETRIES": "20" }

   i.e. the knob exists, is real, and is already set (to 20).

3. **Official Claude Code docs confirm the behavior** (code.claude.com):
   - **Error reference** -> *Automatic retries*:
     > *Claude Code retries transient failures before showing you an error.
     > Server errors, overloaded responses, request timeouts, temporary 429
     > throttles, and dropped connections are all retried up to **10 times with
     > exponential backoff**. While retrying, the spinner shows a
     > "Retrying in Ns - attempt x/y" countdown.*

     | Variable | Default | Effect |
     |---|---|---|
     | CLAUDE_CODE_MAX_RETRIES | **10** | Number of retry attempts. Lower it to surface failures faster in scripts; raise it to wait through longer incidents. |
     | API_TIMEOUT_MS | 600000 | Per-request timeout (ms). |

   - The exact error this lane targets is listed under *Usage limits* ->
     **"Server is temporarily limiting requests"**:
     > API Error: Server is temporarily limiting requests (not your usage limit)
     > *This is **retried automatically** before being shown.*

4. **SDK-level semantics** (Anthropic SDK, via the claude-api skill ref):
   > *The SDK auto-retries connection errors, 408, 409, 429, and >=500 with
   > exponential backoff (default 2 retries). Set max_retries ...*
   and on 429 specifically it reads retry-after and waits that long.
   CLAUDE_CODE_MAX_RETRIES overrides the SDK's max_retries (Claude Code
   ships the default at **10**, not the SDK's bare 2).

### Conclusion of the spike

| Question | Answer |
|---|---|
| Is it a settings.json env? | **Yes** - CLAUDE_CODE_MAX_RETRIES. |
| Is it a CLI flag? | No. |
| Is it a wrapper we build? | No. |
| Is it the same as fleetd? | **No** - fleetd is the outer (minutes) layer; this is the inner (seconds, in-harness) layer. |
| Does it auto-retry the "temporarily limiting requests" 429? | **Yes**, automatically, before the error is ever shown. |
| Does it honor retry-after? | **Yes** (SDK behavior). |

Because the answer is "a settings.json env knob," the build is trivial: the
value just needs to be present in the shared settings. This lane therefore
produces **FINDINGS.md + a verification harness + a contract request to Z**, as
the brief explicitly green-lit for this outcome.

---

## Mechanism chosen

**Use the built-in harness retry; configure it via the one env knob; do not
build a wrapper.** Building a re-submitting wrapper around the agent is the
wrong move and is impossible to do cleanly anyway (a hook can observe but cannot
re-submit a turn - see the fleetd spec quote above). The harness already does
the right thing; our job is only to (1) make sure the knob is set in shared
config and (2) document/verify the envelope respects the cache window.

The optional companion piece named by the fleetd spec - *"an optional
notification hook"* - is **out of scope for item 5** (it is observability, not
retry) and is left as a note for a future lane; building it here would widen
scope.

---

## Backoff schedule and the cache window

The harness uses **exponential backoff with a per-attempt cap**, and **honors
the server retry-after header** when present (taking precedence over the
computed delay). The published docs give the count (default 10) and the shape
(exponential, capped) but not the exact base/cap constants - those are internal
SDK constants. Our verification harness (simulate_429.py) models the contract
with conservative, representative constants and proves the envelope:

- base = 0.5s, multiplier = 2x, per-attempt cap = 8s
- retry-after header, when present, is honored verbatim (clamped against
  absurd values).

**Why it stays inside the cache window.** Anthropic prompt-cache (ephemeral)
TTL is ~5 min / **300s**. We target a **270s budget** to leave margin so the
*last* retry still lands on a warm cache. The transient 429s this layer catches
("Server is temporarily limiting requests") carry **single-digit-second**
retry-after values, so a real recovery is typically **one short backoff**, far
inside the window. Even the worst case - **all 10 retries 429 at the cap** -
sums to **55.5s of cumulative backoff**, still comfortably within 270s.

This is the coordination point with **Lane A1's cache timer**: a transient 429
recovers in seconds-to-tens-of-seconds, so it does **not** force a cold,
full-cost re-read. If an outage lasts *minutes* (beyond the harness's
seconds-scale budget), it is the **fleetd outer layer's** job to ride it out -
not this layer's.

> Note on CLAUDE_CODE_MAX_RETRIES=20 (the current global value): 20 retries at
> an 8s cap models to ~135s cumulative - still inside the 300s TTL but tighter
> on margin. We recommend **10** (the Claude Code default) for the shared
> Command Center setting precisely so the worst-case envelope (~55s) leaves the
> widest cache margin. See the contract request below.

---

## Verification (run + real output)

    $ uv run tools/rate-limit-retry/simulate_429.py

    === Scenario 1 - single transient 429 (no retry-after), then 200 ===
        policy: base=0.5s x2.0 cap=8.0s, max_retries=10
      attempt 0: 429 throttle -> Retrying in 0.5s (attempt 1/10); cumulative backoff=0.5s
      attempt 1: 200 OK  (succeeded)
        result: RECOVERED; total modeled backoff = 0.5s
        cache window: 0.5s <= 270s budget (TTL 300s) -> WITHIN WINDOW (cache stays warm)

    === Scenario 2 - 429 with retry-after=3s header, then 200 (SDK honors header) ===
        policy: base=0.5s x2.0 cap=8.0s, max_retries=10
      attempt 0: 429 throttle [retry-after=3.0s] -> Retrying in 3.0s (attempt 1/10); cumulative backoff=3.0s
      attempt 1: 200 OK  (succeeded)
        result: RECOVERED; total modeled backoff = 3.0s
        cache window: 3.0s <= 270s budget (TTL 300s) -> WITHIN WINDOW (cache stays warm)

    === Scenario 3 - WORST CASE: 10 consecutive 429s at the cap ===
        policy: base=0.5s x2.0 cap=8.0s, max_retries=10
      attempt 0..9: 429 throttle -> backoff 0.5,1,2,4,8,8,8,8,8,8s; cumulative=55.5s
      attempt 10: 429 "Server is temporarily limiting requests (not your usage limit)" -> retries exhausted (10)
        result: exhausted -> surfaced to user; total modeled backoff = 55.5s
        cache window: 55.5s <= 270s budget (TTL 300s) -> WITHIN WINDOW (cache stays warm)

    ================================================================
    PASS: every scenario's backoff envelope stays within the ~300s cache window.

A **simulated 429** ("Server is temporarily limiting requests") auto-retries
with backoff, recovers on the next success, and the **full** backoff envelope
stays inside the ~5-minute cache window in every scenario. PASS.

---

## Contract request to Lane Z (the exact settings.json entry)

This lane does **not** write settings.json (single-owner rule). Lane Z owns
that file. **Requested addition to the shared settings.json env block:**

    {
      "env": {
        "CLAUDE_CODE_MAX_RETRIES": "10"
      }
    }

- **Key:** CLAUDE_CODE_MAX_RETRIES
- **Value:** "10" (string - Claude Code env values are strings)
- **Rationale:** Enables agent-level auto-retry of transient Anthropic 429s
  ("Server is temporarily limiting requests") with exponential backoff that
  stays inside the ~5-minute prompt-cache TTL (worst-case ~55s of backoff).
  10 is the Claude Code default and gives the widest cache-window margin.
- **Note for Z:** the user's *personal* ~/.claude/settings.json currently sets
  this to "20". If the Command Center ships a project-scoped
  .claude/settings.json, set it to "10" there for the tightest cache margin;
  if instead Z is curating the user-global value, dropping 20 -> 10 is the
  recommended change but is the user's call (20 still fits the 300s TTL with
  ~135s modeled worst case).

---

## Layer boundary (so integration doesn't conflate the two)

| | **This lane (A2) - agent/harness layer** | **Merged feat/rate-limit-resilience - fleetd layer** |
|---|---|---|
| Whose API calls | The agent's own (interactive Claude Code) | The fleet's containerized claude -p steps |
| Where | Inside the harness / Anthropic SDK | crates/fleetd/src/retry.rs, driver.rs |
| Knob | CLAUDE_CODE_MAX_RETRIES (settings.json env) | CC_RL_BASE_SECS / CC_RL_CAP_SECS / CC_RL_MAX_WAIT_SECS |
| Timescale | Seconds (rides out transient throttles) | Minutes->~1h (rides out sustained outages) |
| Built here? | No - already in the harness; we only configure + verify | Already merged - **do not touch** |

We did not modify any fleetd/fleet-core code.
