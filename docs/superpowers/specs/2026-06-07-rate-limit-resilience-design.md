# Rate-Limit Resilience — Design

> Status: **passed 3 adversarial critique rounds; approved-pending-spike.** See Design Critique Log.
> Parent: [./2026-06-05-command-center-sp1-design.md](./2026-06-05-command-center-sp1-design.md),
> builds on [./2026-06-06-sp1-hardening-design.md](./2026-06-06-sp1-hardening-design.md)
> Date: 2026-06-07

## Goal

Make the daemon's containerized `claude` agents survive **Anthropic-side rate
limiting / overload** (429 / 529 / sustained throttling) without failing the unit.
When Anthropic is the problem — not the agent — the daemon should **back off and
retry the same step with exponential backoff**, and only if the outage outlasts a
generous envelope, **park the unit at `NeedsHuman`** (resumable later), never
`Failed`. This serves the north star (*useful autonomy*): a transient provider
outage shouldn't strand a half-built unit or burn a human's attention.

## The bug today (motivating context)

An agent step runs `claude -p --output-format stream-json` inside the container
([steps.rs](../../../crates/fleetd/src/steps.rs)). The driver's `agent_exec`
([driver.rs](../../../crates/fleetd/src/driver.rs)) calls `runner.exec`, and for the
oracle/build/review steps it **ignores the exit code** (only `Checking` — the test
command — checks it). `LocalDockerRunner::exec` also **discards stderr**
(`local_docker.rs:159`). There is **no detection, retry, or backoff** for a throttled
call, so a rate-limited `claude` (non-zero exit after its own internal retries) is
silently swallowed and the phase proceeds on empty output (e.g. a review with no
`BLOCKERS=N` line defaults to `0` and opens the gate).

**Layering note.** `claude` already retries 429/529 internally (~10×, seconds-scale,
`CLAUDE_CODE_MAX_RETRIES`). Our retry is a deliberate **outer layer** that rides out
throttling lasting **minutes**, which claude's internal retries cannot.

---

## Design

### 1. Scope — claude steps only

`agent_exec` is **not** claude-only: **four** phases call it — `Spec`, `Building`,
`Reviewing` (claude) **and `Checking`**, which runs the project *test command*
(`steps::check`, e.g. `npm test`; `driver.rs:391`, `steps.rs:66`) and deliberately
relies on `exit_code != 0 → ChecksFailed` (`driver.rs:390-401`). So the retry/classify
path must run for **claude steps only**. `agent_exec` gains an explicit
`claude_step: bool` parameter (set `true` at the Spec/Building/Reviewing call sites,
`false` at `Checking`) — explicit rather than inferring from `IterationKind` so a
future step can't silently opt in/out. The classify+backoff runs only when
`claude_step`. `Checking` keeps today's exact behavior. This prevents a test suite
that prints "429"/"rate limit" from being mistaken for a throttle.

### 2. Detection — the spike is a GATE, and stderr is load-bearing

A pure `classify(&ExecOutput) -> StepOutcome { Ok, RateLimited }`. **`Ok` is the
default for any unrecognized signal**, so no unit that passes today begins to fail
(a non-rate-limit non-zero exit is still swallowed-and-proceeds, exactly as today).

Signals — **pinned by a gating spike before implementation** (see Build order). The
spike must capture, into `spikes/SPIKE-RESULTS.md`, what `claude -p` actually emits on
a sustained 429/529 and on a hard usage cap, with explicit pass/fail. Two realistic
shapes:
- a `stream-json` terminal `result` record with `is_error: true` and a rate-limit /
  overloaded error category, **or**
- (the likely hard-cap case) a non-zero exit with the error text **only on stderr** and
  no structured `result` record.

Because the structured record **may not exist**, the design commits to the **text
match standing alone as the primary signal**: case-insensitive
`rate limit | overloaded | 429 | 529 | usage limit` over **stderr and stdout**, on a
non-zero exit. The structured record, if the spike confirms it, is an additional
high-confidence path. **If the spike finds no reliable signal at all, the feature is
re-scoped, not shipped on a guess.**

Consequently **`ExecOutput` gains `stderr: Vec<String>`** (today discarded at
`local_docker.rs:159`) — this is **load-bearing, not cosmetic**: for a hard cap the
*only* signal is on stderr. The change touches the `Runner` trait shape,
`LocalDockerRunner::exec`, and `FakeRunner` (which gains **settable stderr**, so a test
can present a rate-limit on stderr alone — `fake.rs:53`'s `fail()` produces none
today). At least one retry/exhaustion test must carry its signal **only on stderr**.

We deliberately **do not** add `Timeout`/exit-124 handling (a per-attempt
in-container `timeout` kill): it conflicts with the daemon's own wall-clock backstop,
misclassifies when `wall_clock_secs == 0`, and is scope creep. A 124 stays `Ok`
(today's behavior); the daemon's existing wall-clock check governs aggregate time.

### 3. The retry loop (inside `agent_exec`, `claude_step` only)

`Event::Iteration { kind, n }` is emitted **once**, before the loop (retries don't
re-emit it, so iteration/round counts stay correct). Then:

```
loop {
    let t0 = Instant::now();
    out = runner.exec(argv).await            // Err (docker down) -> today's fatal path
    stream out.stdout as Log
    match classify(&out) {
        Ok => return Some(out)               // caller's phase arm calls account() as today
        RateLimited => {
            // Count this attempt's own cost if it parsed (independent session => sum is
            // correct; no-op when usage is None). A trackable cap breach wins over waiting.
            if self.account(&out) { goto(CapBreach, "usd cap"); return None }
            let delay = backoff.next();
            // rl_elapsed = the failed attempt's exec time + the backoff sleep.
            self.rl_elapsed += t0.elapsed();
            if self.rl_elapsed >= envelope { goto(RetriesExhausted, "rate-limit retries exhausted"); return None }
            self.emit(Blocked { reason: RL_REASON, detail: "retrying in {delay}s (attempt {k})" });
            let until = Instant::now() + delay;
            loop {
                select! {
                    _ = tokio::time::sleep_until(until) => break,
                    cmd = self.commands.recv() => {
                        if is_halt(cmd) { goto(Halt); return None }
                        else { emit "not valid mid-run" error; /* re-loop on the SAME `until` */ }
                    }
                }
            }
            self.rl_elapsed += delay;
            continue
        }
    }
}
```

**Cost.** Each re-exec is a fresh `claude -p` session reporting its *own*
`total_cost_usd`, so summing per attempt is correct. `account()` is a no-op when usage
didn't parse (`driver.rs:183` guards on `Some`), so on the hard-cap shape we may not be
able to track spend — therefore the **envelope (not cost) is the universal backstop**
that guarantees termination. When cost *is* trackable and breaches `usd_cap`, that wins
over retrying (don't burn budget waiting out an outage).

**Interruption.** Single-consumer channel: during backoff we `select!`
`sleep_until(deadline)` against `commands.recv()`, computing the deadline **once** so a
spurious non-Halt command consumes only the *remaining* delay (not a fresh full one —
otherwise a polling client could reset the timer forever). A received command is
handled with `poll_halt` semantics: `Halt` → `goto(Halt)` → Halted; **any non-Halt
command emits the existing "not valid mid-run" error and we re-enter the wait on the
same deadline** (never silently dropped; we never attempt an invalid `Abandon` from an
agent phase — to abandon, the user halts first, then abandons from `Halted`, as the
existing `halt_then_abandon` flow already does).

**Backoff scope + test determinism.** `Backoff` is constructed **fresh per
`agent_exec` call**, so its exponent resets per phase (a unit rate-limited in Building
then again in Reviewing restarts at `base·2⁰`; harmless — the envelope still bounds
total wait). Only `rl_elapsed` persists on `Run` as the cross-phase outage budget — do
**not** hang `Backoff` on `Run`, or a later phase would jump straight to 5-min sleeps.
Jitter is an **injectable/zeroable** component (off in tests) so attempt counts are
deterministic; exhaustion tests assert terminal phase + reason, never an exact attempt
count.

### 3a. Concurrency — hold the slot (revised)

The unit **keeps its concurrency permit through the wait.** Anthropic throttling is
**account-wide**, so a freed slot would only let another unit immediately re-throttle.
Holding wastes **at most one backoff interval (≤ `CC_RL_CAP_SECS`) of recovered
capacity per held slot** — bounded and acceptable, not zero — and it avoids the
starvation / thundering-herd / un-interruptible re-acquire failure modes that
releasing-and-re-acquiring introduces. (Lowering `CAP_SECS` would detect recovery
sooner at the cost of more attempts into an ongoing outage — the wrong trade.) The maximum hold is bounded: when `rl_elapsed` reaches the envelope the
unit parks at `NeedsHuman`, which (via existing entry cleanup) releases the permit and
tears down the container (volume kept). A user can `Halt`/`Abandon` to free a slot
sooner. (Per-unit slot-release during waits is a deliberate non-goal — see Out of
scope.)

### 4. Wall-clock interplay + exhaustion → NeedsHuman

**`rl_elapsed: Duration`** accumulates *all* rate-limit time (each failed attempt's
exec time **plus** its backoff sleep). It serves two roles:
- **Wall-clock exemption:** `over_wall_clock` subtracts `rl_elapsed`. Note the
  wall-clock check runs only at the **top of the `drive()` loop** (`driver.rs:252`),
  while the retry loop is parked *inside* `agent_exec` — so the wall-clock physically
  **cannot** fire mid-backoff regardless. The exemption's real job is the **mixed**
  case: genuine agent work *plus* an outage, where the outage time folded into
  `started.elapsed()` would otherwise trip the cap on a *subsequent* phase's top-of-loop
  check. The exemption arithmetic is extracted into a **pure function**
  `wall_clock_exceeded(elapsed, rl_elapsed, cap_secs) -> bool`, unit-tested directly (no
  clock dependency — the driver stays on `std::time::Instant`, changing no production
  timing; `over_wall_clock` becomes a one-line delegation).
- **Envelope:** when `rl_elapsed >= CC_RL_MAX_WAIT_SECS` (~1h), `agent_exec` parks the
  unit at `NeedsHuman` via a new `Trigger::RetriesExhausted`, added to the **existing
  `is_agent_active`-guarded interrupt line** in `fleet-core/transition.rs`
  (`CapBreach | Stall | RetriesExhausted if phase.is_agent_active() => NeedsHuman`) —
  one line, and it must go on the **guarded arm only, never the universal
  interrupt block** (else it could fire from `MergeCheck`/`PrOpen`). Its raise sites
  (Spec/Building/Reviewing) are a subset of `is_agent_active`, so the transition is
  always valid and can never hit the `goto → Failed` path. Reason:
  `"rate-limit retries exhausted"`.

Entry-side cleanup tears the container down keeping the volume, so a later `Resume`
re-provisions, reuses the volume, skips the frozen oracle, and continues — reusing
SP1-hardening machinery with no new work.

Backoff/envelope defaults (env-tunable): `CC_RL_BASE_SECS`=2, `CC_RL_CAP_SECS`=300,
`CC_RL_MAX_WAIT_SECS`=3600. Delay = `min(cap, base·2ⁿ) + jitter`.

**Idempotency of re-exec.** Safe by construction: `oracle_frozen` is set only after a
successful `Spec` (`driver.rs:322`); the WIP `commit_all` runs only after a successful
build (`:372`); `n` is bumped at phase entry, before the loop. **Oracle-exhaustion
corner:** if the `Spec` step itself exhausts, the unit parks with `oracle_frozen=false`,
so `Resume` re-runs the oracle from scratch against *remaining* budget (the failed
attempts' cost was already counted per §3). This is correct — no test set was ever
frozen — but note it can immediately `CapBreach` if the outage consumed most of the cap.
Acceptable: there is genuinely no oracle to skip.

### 5. Events + cockpit

Reuse `Event::Blocked` — no new event type. Two **shared reason constants** (defined
once, referenced by the cockpit's exact-match): `RL_REASON = "rate limited"` (retry
wait) and the exhaustion `PhaseChanged` reason `"rate-limit retries exhausted"`. The
cockpit gets a dedicated **`rateLimited` flag** on the unit (set when a
`Blocked{reason: "rate limited"}` arrives, cleared on the next `phase_changed`) and a
chip — mirroring the existing `awaitingSlot` flag/chip rather than rendering the
clobber-prone free-text `u.blocked`. A test asserts the exact reason bytes the cockpit
matches.

### 6. Testing

All against `FakeRunner`, **no Docker**, made instant with
`#[tokio::test(start_paused = true)]` (virtual time makes sleeps free):

- **`classify`** over spike-captured fixtures: rate-limit on a `result` record →
  `RateLimited`; rate-limit text **on stderr only** → `RateLimited`; success / empty /
  unrelated non-zero exit → `Ok`.
- **`wall_clock_exceeded`** pure-function unit tests: `rl_elapsed` fully covering a
  large `elapsed` ⇒ not exceeded; without the subtraction ⇒ exceeded (proves the
  exemption, no clock needed).
- **Retry-then-succeed:** attempt 1 rate-limit (signal on stderr), attempt 2 ok →
  one `Iteration`, a `Blocked{"rate limited"}`, continuation (seq continuity).
- **Exhaustion via envelope, untrackable cost:** runner always rate-limits with
  `usage: None` → still parks at `NeedsHuman` (`"rate-limit retries exhausted"`),
  permit released by entry cleanup. (Proves the envelope, not cost, is the backstop.)
- **Cap-breach during rate-limit (trackable cost):** rate-limited attempts whose summed
  cost exceeds `usd_cap` → `NeedsHuman` via `CapBreach`, not endless retry.
- **Halt mid-backoff** → `Halted`; **non-Halt mid-backoff** → "not valid" error and the
  wait resumes on the *same* deadline (does not restart, does not livelock).

---

## Decisions

1. Envelope: **wait it out** — give up after **`rl_elapsed` ≥ ~1h** (failed-exec time +
   backoff sleeps), each backoff ≤ 5 min, then `NeedsHuman` (resumable) — never
   `Failed`.
2. Retry/classify **scoped to claude steps** via an explicit `claude_step` flag;
   `Checking` excluded.
3. Detection: **text match on stderr+stdout is the primary, standalone signal**
   (structured `result` record is a bonus if the **gating spike** confirms it). `Ok` is
   the conservative default. **No 124/Timeout handling** (scope creep).
4. `account()` each rate-limited attempt when usage parses; a trackable cap breach wins
   over retrying; the **envelope is the universal termination backstop** regardless.
5. **Hold the concurrency permit** through the wait (account-wide throttle makes
   releasing pointless and starvation-prone); the envelope bounds the hold.
6. `rl_elapsed` (failed-exec + sleep time) is **exempt from the wall-clock** and is the
   envelope basis; exemption math is a **pure, unit-tested function** (driver stays on
   `std::time::Instant` — no production-timing change).
7. Reuse `Event::Blocked` with **shared reason constants**; cockpit gets a dedicated
   `rateLimited` flag/chip; exhaustion via `Trigger::RetriesExhausted` on the guarded
   interrupt line.

## Out of scope (YAGNI)

- No new event type; no per-error-type backoff tuning.
- No `Fatal` reclassification of non-rate-limit agent-step failures, and **no exit-124
  / in-container-timeout handling** (today's behavior preserved).
- No per-unit slot-release during waits (the permit is held; see §3a).
- Path B (auto-retry in the user's *interactive* Claude Code sessions) is **not**
  buildable as a true hook — hooks observe but cannot re-submit a turn — handled
  separately via `CLAUDE_CODE_MAX_RETRIES` + an optional notification hook.

## Build order

1. **GATING spike:** capture `claude -p`'s real output on a sustained 429/529 and a
   hard usage cap into `spikes/SPIKE-RESULTS.md` with explicit pass/fail; pin the exact
   classifier patterns (and the stderr-only case). Implementation does not proceed on an
   unconfirmed signal.
2. `ExecOutput.stderr` across trait + `local_docker` + `FakeRunner` (settable).
3. `classify` + `Backoff` + the pure `wall_clock_exceeded` (all unit-tested) —
   `backoff.rs` (or in `driver.rs`).
4. `Trigger::RetriesExhausted` on the guarded interrupt line in `fleet-core`
   (+ transition tests).
5. Retry loop in `agent_exec` (`claude_step` scoping, `account`-each-attempt,
   `sleep_until` deadline, halt handling, `rl_elapsed` exemption + envelope, permit
   held, exhaustion → NeedsHuman) + driver tests.
6. Cockpit `rateLimited` flag/chip keyed on the shared `"rate limited"` constant.

## File structure

`crates/fleetd/src/{driver.rs, runner.rs, fake.rs, local_docker.rs}` and maybe a new
`backoff.rs`; `crates/fleet-core/src/transition.rs`;
`cockpit/ui/src/{lib/fleet.ts, App.svelte}`; `spikes/SPIKE-RESULTS.md`.

## Design Critique Log

Three independent adversarial critique rounds, each a fresh agent grounded in the
actual code, each seeing the prior round's revision.

### Critique Round 1
Found three **Critical** code-rooted bugs and several Important gaps:
- **`agent_exec` is not claude-only** — the `Checking` step (`npm test`) flows through
  it, so an in-`agent_exec` retry loop would retry test output that prints "429" and
  break the `exit_code → ChecksFailed` loop. → Scoped retry/classify to **claude steps
  only** (explicit flag; `Checking` excluded).
- **`select!` on `commands.recv()`** conflicts with the single-consumer channel and the
  state machine (`Abandon` isn't valid mid-agent-phase). → Only `Halt` interrupts;
  non-Halt emits the existing "not valid" error; abandon requires halting first.
- **"Cost untouched" is unsafe** — a partial run bills > $0. → `account()` each
  rate-limited attempt; a trackable cap breach wins over retrying.
- **In-container `timeout`/exit-124** unhandled; **permit held** could freeze the fleet;
  **`start_paused` wall-clock test proves nothing** (real `std::time::Instant`). →
  Addressed in R1 (later refined): added 124→wall-clock, permit *release*, clock switch.
Verdict: needs rethinking on placement/cost — resolved in the revision.

### Critique Round 2
Pressure-tested the R1 revision; found the R1 fixes themselves were flawed:
- **Permit *release*-and-re-acquire** introduces thundering-herd starvation,
  un-interruptible re-acquire, and an envelope timer that doesn't count acquire-block
  time (units livelock for the full hour). → **Reverted to holding the permit**, with an
  account-wide-throttle justification; envelope bounds the hold.
- **124 → CapBreach** collides with the daemon's own wall-clock backstop and
  misclassifies when `wall_clock_secs==0`. → **Cut 124 handling entirely** (scope creep).
- **Clock switch to `tokio::time::Instant`** changes *production* timing. → Replaced with
  a **pure `wall_clock_exceeded()` function**, unit-tested directly; driver stays on
  `std::time::Instant`.
- **`usage:None` loses spend** (hard-cap case) and **non-Halt command restarts the full
  sleep** (livelock). → Envelope is the **universal backstop** regardless of cost; use
  `sleep_until(deadline)`. Also: **detection signal is unproven** → made the spike a
  **gate**, committed to **stderr text-match as the standalone primary**, and reclassed
  `stderr` as load-bearing. `StepKind` enum demoted to an explicit bool.
Verdict: still needed rethinking on permit/cost/signal — resolved in the revision.

### Critique Round 3
Pressure-tested the R2 revision. **No Critical or Important redesign issues** — the
state-machine wiring (`RetriesExhausted` on the guarded `is_agent_active` arm), cost
accounting (no double-emit), `ExecOutput.stderr` blast radius (2 literals, zero
call-site churn via defaulted `ok`/`fail`), and termination guarantee were all confirmed
sound. Remaining items were **documentation guardrails**, now folded in:
- Re-justified the wall-clock exemption on the **mixed-work** case (control flow already
  prevents a mid-backoff trip).
- Documented the **oracle-exhaustion → Resume re-runs the oracle against remaining
  budget** consequence.
- Pinned **`Backoff` per-phase vs `rl_elapsed` per-unit** and **injectable/zeroable
  jitter** so exhaustion tests assert phase+reason, not attempt count.
- Softened the "wastes no useful capacity" claim to "≤ one backoff interval".
- Noted `RetriesExhausted` belongs on the **guarded arm only, never universal**.
Verdict: **READY TO IMPLEMENT**, approved-pending-spike (the one load-bearing unknown —
the exact rate-limit signal — is correctly gated as build-order step 1; if the spike
finds no reliable signal, re-scope rather than ship on a guess).
