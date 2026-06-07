# Rate-Limit Resilience Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the daemon's containerized `claude` agents back off and retry through Anthropic rate-limits (429/529), holding their slot, and park at `NeedsHuman` (never `Failed`) if an outage outlasts a ~1h envelope.

**Architecture:** An outer retry loop inside `driver.rs::agent_exec`, scoped to claude steps only (not the test command). A pure `classify()` detects a rate-limit from exec output (text match on stderr+stdout primary); a pure `Backoff` yields exponential delays; a pure `wall_clock_exceeded()` exempts rate-limit time. Exhaustion routes through a new `Trigger::RetriesExhausted` on the existing guarded interrupt line. The cockpit shows a "rate limited" chip.

**Tech Stack:** Rust (tokio, axum), Svelte cockpit. Spec: [../specs/2026-06-07-rate-limit-resilience-design.md](../specs/2026-06-07-rate-limit-resilience-design.md).

**Branch:** `feat/rate-limit-resilience` (already created).

---

## Task 1: GATING spike — pin the rate-limit signal

**Files:**
- Modify: `spikes/SPIKE-RESULTS.md` (append a section)

This task confirms what `claude -p` actually emits on a sustained 429/529 and a hard usage cap, so the classifier patterns aren't a guess. It requires Docker + the `cc-agent:dev` image + a way to provoke a rate-limit. **If you cannot provoke a real rate-limit, do NOT block:** record that the signal is unconfirmed and proceed with the conservative text patterns below (the design commits to text-match-on-stderr as the standalone primary). The classifier is built and tested against synthetic fixtures regardless; the spike only refines the patterns.

- [ ] **Step 1: Try to capture a real rate-limit**

If you have a way to force throttling (e.g. a tiny `CLAUDE_CODE_MAX_RETRIES=0` run during a known-busy window, or a stub that returns HTTP 429), run a `claude -p --output-format stream-json` step inside the container and capture **both stdout and stderr** and the **exit code**.

- [ ] **Step 2: Record findings**

Append to `spikes/SPIKE-RESULTS.md` a `## Rate-limit signal (2026-06-07)` section noting, for each of {sustained 429/529, hard usage cap}: the exit code, whether a terminal `{"type":"result","is_error":true,...}` record appears (and its `subtype`/error fields), and the exact stderr/stdout text. If unconfirmed, write "UNCONFIRMED — proceeding with conservative text patterns: `rate limit`, `rate_limit`, `overloaded`, `429`, `529`, `usage limit`."

- [ ] **Step 3: Commit**

```bash
git add spikes/SPIKE-RESULTS.md
git commit -m "spike(fleetd): capture/record claude rate-limit signal for classifier"
```

---

## Task 2: `ExecOutput.stderr` (load-bearing for detection)

**Files:**
- Modify: `crates/fleetd/src/runner.rs` (the `ExecOutput` struct)
- Modify: `crates/fleetd/src/local_docker.rs:159-164` (stop discarding stderr)
- Modify: `crates/fleetd/src/fake.rs` (`ok`/`fail` defaults + a `rate_limited` helper)

- [ ] **Step 1: Add the field to `ExecOutput`**

In `runner.rs`, add `stderr` to the struct:
```rust
/// Result of one `exec` step.
#[derive(Clone, Debug)]
pub struct ExecOutput {
    pub exit_code: i32,
    pub stdout: Vec<String>,
    /// Captured stderr lines. Load-bearing for rate-limit detection: a hard
    /// usage cap surfaces only here, with no structured stdout record.
    pub stderr: Vec<String>,
    pub usage: Option<Usage>,
}
```

- [ ] **Step 2: Populate stderr in the real runner**

In `local_docker.rs::exec`, change the discard to capture:
```rust
        let (code, out, err) = docker(args).await?;
        Ok(ExecOutput {
            exit_code: code,
            stdout: out.lines().map(str::to_string).collect(),
            stderr: err.lines().map(str::to_string).collect(),
            usage: None, // claude stream-json usage parsing happens in the driver
        })
```

- [ ] **Step 3: Default stderr in the fakes + add a `rate_limited` helper**

In `fake.rs`, add `stderr: vec![]` to both `ok` and `fail`, and add a builder that carries a rate-limit on stderr only (with no usage — the hard-cap shape):
```rust
    /// Convenience: an exec output with exit code 0 and a given cost.
    pub fn ok(cost_usd: f64, stdout: &[&str]) -> ExecOutput {
        ExecOutput {
            exit_code: 0,
            stdout: stdout.iter().map(|s| s.to_string()).collect(),
            stderr: vec![],
            usage: Some(Usage { tokens_in: 100, tokens_out: 10, cost_usd }),
        }
    }

    /// Convenience: a failing exec output (non-zero exit).
    pub fn fail(cost_usd: f64) -> ExecOutput {
        ExecOutput {
            exit_code: 1,
            stdout: vec![],
            stderr: vec![],
            usage: Some(Usage { cost_usd, ..Default::default() }),
        }
    }

    /// An Anthropic rate-limit: non-zero exit, the signal ONLY on stderr, no usage
    /// (the hard-cap shape — proves the classifier reads stderr end to end).
    pub fn rate_limited() -> ExecOutput {
        ExecOutput {
            exit_code: 1,
            stdout: vec![],
            stderr: vec!["API Error: 429 rate limit exceeded".into()],
            usage: None,
        }
    }
```

- [ ] **Step 4: Build the workspace (compile-only check)**

Run: `cargo build --workspace`
Expected: FAILS — every other `ExecOutput { .. }` literal now misses `stderr`. There are none outside `fake.rs`/`local_docker.rs` (all tests build via `FakeRunner::ok`/`fail`), so this should actually PASS. If any literal errors, add `stderr: vec![]` to it.

- [ ] **Step 5: Run the suite (no behavior change yet)**

Run: `cargo test --workspace`
Expected: PASS (all existing tests; stderr is unused so far).

- [ ] **Step 6: Commit**

```bash
git add crates/fleetd/src/runner.rs crates/fleetd/src/local_docker.rs crates/fleetd/src/fake.rs
git commit -m "feat(fleetd): capture stderr in ExecOutput (rate-limit detection input)"
```

---

## Task 3: `retry.rs` — `StepOutcome` + `classify`

**Files:**
- Create: `crates/fleetd/src/retry.rs`
- Modify: `crates/fleetd/src/lib.rs` (register the module)

- [ ] **Step 1: Write the module with `classify` + tests**

Create `crates/fleetd/src/retry.rs`:
```rust
//! Pure helpers for rate-limit resilience: classify an exec outcome, compute
//! backoff delays, and decide the wall-clock cap with rate-limit time exempt.
//! No async, no I/O — fully unit-tested.

use crate::runner::ExecOutput;
use std::time::Duration;

/// The constant the cockpit chip exact-matches (keep in sync with `fleet.ts`).
pub const RL_REASON: &str = "rate limited";

/// Outcome of one agent exec, as far as retry logic cares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepOutcome {
    /// Proceed (default for any unrecognized signal — preserves today's behavior).
    Ok,
    /// A recognized Anthropic throttle: back off and retry the same step.
    RateLimited,
}

/// Case-insensitive substrings that mark an Anthropic throttle (Task-1 spike may
/// extend these). Only consulted on a NON-ZERO exit, so a clean run is never a
/// false positive.
const RL_PATTERNS: &[&str] =
    &["rate limit", "rate_limit", "overloaded", "429", "529", "usage limit"];

/// Classify an exec output. Conservative: `Ok` unless a non-zero exit carries a
/// recognized rate-limit signal on stdout or stderr.
pub fn classify(out: &ExecOutput) -> StepOutcome {
    if out.exit_code == 0 {
        return StepOutcome::Ok;
    }
    let hit = out
        .stdout
        .iter()
        .chain(out.stderr.iter())
        .any(|line| {
            let l = line.to_lowercase();
            RL_PATTERNS.iter().any(|p| l.contains(p))
        });
    if hit {
        StepOutcome::RateLimited
    } else {
        StepOutcome::Ok
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::Usage;

    fn out(code: i32, stdout: &[&str], stderr: &[&str]) -> ExecOutput {
        ExecOutput {
            exit_code: code,
            stdout: stdout.iter().map(|s| s.to_string()).collect(),
            stderr: stderr.iter().map(|s| s.to_string()).collect(),
            usage: Some(Usage::default()),
        }
    }

    #[test]
    fn success_is_ok() {
        assert_eq!(classify(&out(0, &["all good"], &[])), StepOutcome::Ok);
    }

    #[test]
    fn rate_limit_text_on_stderr_only_is_detected() {
        // The hard-cap shape: signal lives only on stderr.
        let e = out(1, &[], &["API Error: 429 rate limit exceeded"]);
        assert_eq!(classify(&e), StepOutcome::RateLimited);
    }

    #[test]
    fn overloaded_on_stdout_is_detected() {
        assert_eq!(classify(&out(1, &["Error: Overloaded (529)"], &[])), StepOutcome::RateLimited);
    }

    #[test]
    fn unrelated_nonzero_exit_is_ok() {
        // e.g. a timeout-kill (124) or a normal agent failure — preserved as today.
        assert_eq!(classify(&out(124, &["compilation failed"], &[])), StepOutcome::Ok);
    }
}
```

- [ ] **Step 2: Register the module**

In `crates/fleetd/src/lib.rs`, add `pub mod retry;` next to the other `pub mod` lines.

- [ ] **Step 3: Run the tests**

Run: `cargo test -p fleetd retry::tests`
Expected: PASS (4 tests).

- [ ] **Step 4: Commit**

```bash
git add crates/fleetd/src/retry.rs crates/fleetd/src/lib.rs
git commit -m "feat(fleetd): classify() rate-limit detection (conservative, stderr-aware)"
```

---

## Task 4: `retry.rs` — `Backoff`

**Files:**
- Modify: `crates/fleetd/src/retry.rs`

- [ ] **Step 1: Write the failing test**

Add to `retry.rs` (inside `mod tests`):
```rust
    #[test]
    fn backoff_grows_exponentially_then_caps() {
        let mut b = Backoff::new(2, 300); // base 2s, cap 300s
        assert_eq!(b.next_delay(), Duration::from_secs(2));
        assert_eq!(b.next_delay(), Duration::from_secs(4));
        assert_eq!(b.next_delay(), Duration::from_secs(8));
        assert_eq!(b.next_delay(), Duration::from_secs(16));
        // ... eventually saturates at the cap and stays there.
        let mut last = Duration::ZERO;
        for _ in 0..20 {
            last = b.next_delay();
        }
        assert_eq!(last, Duration::from_secs(300));
    }
```

- [ ] **Step 2: Run, expect failure**

Run: `cargo test -p fleetd backoff_grows`
Expected: FAIL (`Backoff` not defined).

- [ ] **Step 3: Implement `Backoff` + env-config helpers**

Add to `retry.rs` (above the tests):
```rust
/// Exponential backoff: `min(cap, base · 2^attempt)`. Deterministic (no jitter —
/// the permit is held during waits, so there is no re-acquire herd to scatter).
pub struct Backoff {
    attempt: u32,
    base_secs: u64,
    cap_secs: u64,
}

impl Backoff {
    pub fn new(base_secs: u64, cap_secs: u64) -> Self {
        Self { attempt: 0, base_secs, cap_secs }
    }

    pub fn next_delay(&mut self) -> Duration {
        let factor = 1u64.checked_shl(self.attempt).unwrap_or(u64::MAX);
        let secs = self.base_secs.saturating_mul(factor).min(self.cap_secs);
        self.attempt = self.attempt.saturating_add(1);
        Duration::from_secs(secs)
    }
}

fn env_secs(key: &str, default: u64) -> u64 {
    std::env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

/// Backoff/envelope knobs (env-tunable; defaults chosen for SP1).
pub fn rl_base_secs() -> u64 { env_secs("CC_RL_BASE_SECS", 2) }
pub fn rl_cap_secs() -> u64 { env_secs("CC_RL_CAP_SECS", 300) }
pub fn rl_max_wait_secs() -> u64 { env_secs("CC_RL_MAX_WAIT_SECS", 3600) }
```

- [ ] **Step 4: Run, expect pass**

Run: `cargo test -p fleetd backoff_grows`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/fleetd/src/retry.rs
git commit -m "feat(fleetd): exponential Backoff + env-tunable rate-limit knobs"
```

---

## Task 5: `retry.rs` — `wall_clock_exceeded` + wire into the driver

**Files:**
- Modify: `crates/fleetd/src/retry.rs`
- Modify: `crates/fleetd/src/driver.rs` (`Run` gains `rl_elapsed`; `over_wall_clock` delegates)

- [ ] **Step 1: Write the failing test**

Add to `retry.rs` tests:
```rust
    #[test]
    fn wall_clock_exempts_rate_limit_time() {
        let cap = 30; // 30s cap
        // 100s elapsed but 80s of it was rate-limit waiting => 20s effective <= 30s.
        assert!(!wall_clock_exceeded(Duration::from_secs(100), Duration::from_secs(80), cap));
        // Same elapsed, no exemption => 100s > 30s => exceeded.
        assert!(wall_clock_exceeded(Duration::from_secs(100), Duration::ZERO, cap));
        // cap 0 disables the check entirely.
        assert!(!wall_clock_exceeded(Duration::from_secs(9999), Duration::ZERO, 0));
    }
```

- [ ] **Step 2: Run, expect failure**

Run: `cargo test -p fleetd wall_clock_exempts`
Expected: FAIL (`wall_clock_exceeded` not defined).

- [ ] **Step 3: Implement the pure function**

Add to `retry.rs`:
```rust
/// Whether the agent-active wall-clock cap is exceeded, with rate-limit time
/// (`rl_elapsed`) exempt. `cap_secs == 0` disables the check.
pub fn wall_clock_exceeded(elapsed: Duration, rl_elapsed: Duration, cap_secs: u64) -> bool {
    cap_secs > 0 && elapsed.saturating_sub(rl_elapsed).as_secs() > cap_secs
}
```

- [ ] **Step 4: Add `rl_elapsed` to `Run` and delegate `over_wall_clock`**

In `driver.rs`, add the field to `struct Run` (next to `started`):
```rust
    started: std::time::Instant,
    /// Accumulated rate-limit time (failed-attempt exec + backoff sleeps); exempt
    /// from the wall-clock and the basis for the give-up envelope.
    rl_elapsed: std::time::Duration,
```
Initialize it in `run()` where the `Run { .. }` is built (next to `started: std::time::Instant::now(),`):
```rust
        started: std::time::Instant::now(),
        rl_elapsed: std::time::Duration::ZERO,
```
Replace `over_wall_clock`:
```rust
    fn over_wall_clock(&self) -> bool {
        crate::retry::wall_clock_exceeded(
            self.started.elapsed(),
            self.rl_elapsed,
            self.spec.wall_clock_secs,
        )
    }
```

- [ ] **Step 5: Run the suite**

Run: `cargo test --workspace`
Expected: PASS (`rl_elapsed` is `ZERO` everywhere so far → identical behavior).

- [ ] **Step 6: Commit**

```bash
git add crates/fleetd/src/retry.rs crates/fleetd/src/driver.rs
git commit -m "feat(fleetd): wall-clock cap exempts rate-limit time (pure, tested)"
```

---

## Task 6: `Trigger::RetriesExhausted` in fleet-core

**Files:**
- Modify: `crates/fleet-core/src/transition.rs`

- [ ] **Step 1: Write the failing test**

In `transition.rs` tests, add:
```rust
    #[test]
    fn retries_exhausted_parks_agent_phases_at_needs_human() {
        for p in [Spec, Building, Reviewing] {
            assert_eq!(transition(p, Tier::T1, Trigger::RetriesExhausted), Some(NeedsHuman));
        }
        // Not meaningful from a daemon-only phase → invalid.
        assert_eq!(transition(MergeCheck, Tier::T1, Trigger::RetriesExhausted), None);
    }
```

- [ ] **Step 2: Run, expect failure**

Run: `cargo test -p fleet-core retries_exhausted`
Expected: FAIL (`RetriesExhausted` not a variant).

- [ ] **Step 3: Add the trigger + guarded edge**

In `transition.rs`, add the variant in the interrupts section of `enum Trigger`:
```rust
    // --- interrupts (guarded by phase) ---
    CapBreach,
    Stall,
    OracleTampering,
    /// The rate-limit backoff envelope was exhausted (Anthropic still unavailable).
    RetriesExhausted,
    FatalError,
```
Add it to the **guarded** interrupt line (NOT the universal block) — extend line ~59:
```rust
        CapBreach | Stall | RetriesExhausted if phase.is_agent_active() => return Some(NeedsHuman),
```

- [ ] **Step 4: Run, expect pass**

Run: `cargo test -p fleet-core`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/fleet-core/src/transition.rs
git commit -m "feat(fleet-core): RetriesExhausted trigger parks agent phases at NeedsHuman"
```

---

## Task 7: The retry loop in `agent_exec`

**Files:**
- Modify: `crates/fleetd/src/driver.rs` (`agent_exec` + its 4 call sites + retry-config fields; tests)
- Modify: `crates/fleetd/src/fake.rs` (an "always rate-limited" mode for the exhaustion test)

- [ ] **Step 1: Add an "always" mode to `FakeRunner`**

In `fake.rs`, add a field + builder so a runner can return a fixed output forever (the finite script can't express "always rate-limited"):
```rust
pub struct FakeRunner {
    scripted: Mutex<VecDeque<ExecOutput>>,
    /// When set, `exec` ignores the script and returns this every time.
    always: Option<ExecOutput>,
    health: Liveness,
    // ... existing fields ...
}
```
In `FakeRunner::new`, init `always: None`. Add a builder:
```rust
    /// Make every `exec` return `out` (ignores the script). For retry tests.
    pub fn always(mut self, out: ExecOutput) -> Self {
        self.always = Some(out);
        self
    }
```
In the `exec` impl, short-circuit at the top:
```rust
    async fn exec(&self, _h: &Handle, _wd: &str, _argv: &[String]) -> Result<ExecOutput, RunnerError> {
        if let Some(out) = &self.always {
            return Ok(out.clone());
        }
        self.scripted
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| RunnerError::Failed("FakeRunner: script exhausted".into()))
    }
```

- [ ] **Step 2: Write the failing tests**

In `driver.rs` tests, add two tests. (`spec`, `cycle`, `run`, `RunCtx`, `drain`, `phases` helpers already exist in this module.)
```rust
    #[tokio::test(start_paused = true)]
    async fn rate_limited_step_retries_then_succeeds() {
        // Oracle rate-limits once (signal on stderr), then succeeds; floor-1 cycle.
        let mut script = vec![FakeRunner::rate_limited(), FakeRunner::ok(0.01, &["test_a.rs"])];
        script.extend(cycle(0));
        let (ctx, crx) = mpsc::unbounded_channel();
        let (etx, mut erx) = mpsc::unbounded_channel();
        drop(ctx);
        let final_phase = run(
            FakeRunner::new(script),
            FakeForge::default(),
            spec(Tier::T1, 100.0, 1),
            RunCtx::standalone(),
            crx,
            etx,
        )
        .await;
        assert_eq!(final_phase, Phase::Done, "a single rate-limit is retried, not fatal");
        let evs = drain(&mut erx);
        // The retry surfaced a "rate limited" Blocked.
        assert!(
            evs.iter().any(|e| matches!(&e.event,
                Event::Blocked { reason, .. } if reason == crate::retry::RL_REASON)),
            "a rate-limit retry emits a Blocked(\"rate limited\")"
        );
        // The oracle Iteration{Review,0} was emitted exactly once (not per retry).
        let oracle_iters = evs.iter().filter(|e|
            matches!(e.event, Event::Iteration { kind: IterationKind::Review, n: 0 })).count();
        assert_eq!(oracle_iters, 1, "Iteration emitted once, before the retry loop");
    }

    #[tokio::test(start_paused = true)]
    async fn persistent_rate_limit_parks_at_needs_human() {
        let permits = std::sync::Arc::new(tokio::sync::Semaphore::new(1));
        let (ctx, crx) = mpsc::unbounded_channel();
        let (etx, mut erx) = mpsc::unbounded_channel();
        let runner = FakeRunner::new(vec![]).always(FakeRunner::rate_limited());
        let h = tokio::spawn(run(
            runner,
            FakeForge::default(),
            spec(Tier::T1, 100.0, 1),
            RunCtx {
                start_seq: 0,
                start_cost: 0.0,
                resume: false,
                start_phase: Phase::Queued,
                permits: permits.clone(),
            },
            crx,
            etx,
        ));
        // Drive virtual time forward until it parks at NeedsHuman.
        loop {
            let e = erx.recv().await.expect("stream closed before NeedsHuman");
            if matches!(e.event, Event::PhaseChanged { to: Phase::NeedsHuman, .. }) {
                break;
            }
        }
        // Entry cleanup released the permit when parking.
        assert_eq!(permits.available_permits(), 1, "permit released at NeedsHuman");
        ctx.send(Command::Abandon { cmd_id: "a".into() }).unwrap();
        let _ = h.await;
    }
```

- [ ] **Step 3: Run, expect failure**

Run: `cargo test -p fleetd rate_limited_step_retries_then_succeeds`
Expected: FAIL — `agent_exec` doesn't retry yet (the first `rate_limited()` output, exit 1 with no usage, is currently returned as-is and the oracle proceeds on empty stdout / wrong path).

- [ ] **Step 4: Rewrite `agent_exec` with the retry loop + `claude_step` param**

In `driver.rs`, add the imports at the top of the file (next to the existing `use` lines):
```rust
use crate::retry::{classify, rl_base_secs, rl_cap_secs, rl_max_wait_secs, Backoff, StepOutcome, RL_REASON};
```
Replace the whole `agent_exec` method with:
```rust
    /// Run one agent step. For claude steps (`claude_step == true`) this wraps the
    /// exec in a rate-limit backoff loop: on a recognized throttle it accounts the
    /// attempt, emits a "rate limited" Blocked, waits (holding the slot, exempt from
    /// the wall-clock), and re-execs; after ~1h of accumulated rate-limit time it
    /// parks the unit at NeedsHuman. The test command (`claude_step == false`) is
    /// returned unchanged. Returns `None` if the unit was driven off this phase
    /// (fatal error, halt, cap breach, or retries exhausted).
    async fn agent_exec(
        &mut self,
        kind: IterationKind,
        n: u32,
        stream: LogStream,
        claude_step: bool,
        argv: &[String],
    ) -> Option<ExecOutput> {
        self.emit(Event::Iteration { kind, n });
        let handle = self.handle.clone().expect("agent_exec without a handle");
        let mut backoff = Backoff::new(rl_base_secs(), rl_cap_secs());
        let max_wait = std::time::Duration::from_secs(rl_max_wait_secs());
        loop {
            let t0 = std::time::Instant::now();
            let mut out = match self.runner.exec(&handle, steps::WORKDIR, argv).await {
                Ok(o) => o,
                Err(e) => {
                    self.emit(Event::Error {
                        scope: ErrorScope::Agent,
                        retryable: false,
                        detail: e.to_string(),
                    });
                    self.goto(Trigger::FatalError, Some("exec failed".into()), None);
                    return None;
                }
            };
            for line in &out.stdout {
                self.emit(Event::Log { stream, line: line.clone() });
            }
            if out.usage.is_none() {
                out.usage = claude_meter::parse_usage(&out.stdout);
            }
            if !claude_step {
                return Some(out); // the test command: no rate-limit handling
            }
            match classify(&out) {
                StepOutcome::Ok => return Some(out), // caller's phase arm accounts it
                StepOutcome::RateLimited => {
                    // Account this attempt's own cost (no-op if usage didn't parse);
                    // a trackable cap breach wins over waiting.
                    if self.account(&out) {
                        self.goto(Trigger::CapBreach, Some("usd cap".into()), None);
                        return None;
                    }
                    self.rl_elapsed += t0.elapsed();
                    if self.rl_elapsed >= max_wait {
                        self.goto(
                            Trigger::RetriesExhausted,
                            Some("rate-limit retries exhausted".into()),
                            None,
                        );
                        return None;
                    }
                    let delay = backoff.next_delay();
                    self.emit(Event::Blocked {
                        reason: RL_REASON.into(),
                        cap: None,
                        detail: format!("retrying in {}s", delay.as_secs()),
                    });
                    // Interruptible wait on a fixed deadline (a spurious non-Halt
                    // command consumes only the remaining delay, never restarts it).
                    // NOTE: `select!` only yields a value here — borrowing `self`
                    // (goto/emit) inside a `recv()` arm would conflict with the
                    // `&mut self.commands` the recv future holds, so handle AFTER.
                    let until = tokio::time::Instant::now() + delay;
                    loop {
                        let received = tokio::select! {
                            _ = tokio::time::sleep_until(until) => None,
                            cmd = self.commands.recv() => Some(cmd),
                        };
                        match received {
                            None => break, // deadline reached → re-exec the step
                            Some(Some(Command::Halt { cmd_id })) => {
                                self.goto(Trigger::Halt, Some("user halt".into()), Some(cmd_id));
                                return None;
                            }
                            Some(Some(other)) => self.emit(Event::Error {
                                scope: ErrorScope::System,
                                retryable: false,
                                detail: format!(
                                    "command {} not valid in {:?}",
                                    other.cmd_id(), self.phase
                                ),
                            }), // keep waiting on the same `until`
                            Some(None) => { self.fail_closed(); return None; }
                        }
                    }
                    self.rl_elapsed += delay;
                    // loop: re-exec the same step (slot held, container kept).
                }
            }
        }
    }
```

- [ ] **Step 5: Update the 4 call sites with the `claude_step` flag**

In `driver.rs`, find each `self.agent_exec(...)` call and insert the flag before `&argv`:
- Spec (oracle): `self.agent_exec(IterationKind::Review, 0, LogStream::Agent, true, &argv)`
- Building: `self.agent_exec(IterationKind::Build, n, LogStream::Agent, true, &argv)`
- Checking (test command — NOT claude): `self.agent_exec(IterationKind::Check, n, LogStream::Check, false, &argv)`
- Reviewing: `self.agent_exec(IterationKind::Review, round, LogStream::Agent, true, &argv)`

- [ ] **Step 6: Run the new tests + full suite**

Run: `cargo test -p fleetd rate_limited_step_retries_then_succeeds persistent_rate_limit_parks_at_needs_human`
Expected: PASS.
Run: `cargo test --workspace`
Expected: PASS (existing tests unaffected — `classify` returns `Ok` for all their exit-0 outputs).

- [ ] **Step 7: Clippy**

Run: `cargo clippy --workspace --all-targets`
Expected: clean (exit 0).

- [ ] **Step 8: Commit**

```bash
git add crates/fleetd/src/driver.rs crates/fleetd/src/fake.rs
git commit -m "feat(fleetd): rate-limit backoff/retry in agent_exec; park at NeedsHuman on exhaustion"
```

---

## Task 8: Cockpit "rate limited" chip

**Files:**
- Modify: `cockpit/ui/src/lib/fleet.ts` (`Unit` + `newUnit` + `fold`)
- Modify: `cockpit/ui/src/App.svelte` (tile chip)

- [ ] **Step 1: Add a `rateLimited` flag to the Unit model**

In `fleet.ts`, mirror the existing `awaitingSlot` pattern. Add to the `Unit` interface (next to `awaitingSlot`):
```ts
  /** True while the unit is backing off through an Anthropic rate-limit. */
  rateLimited: boolean;
```
In `newUnit`, initialize it (next to `awaitingSlot: false,`):
```ts
    rateLimited: false,
```
In `fold`, set it on the matching `Blocked`, and clear both flags on any phase change:
```ts
    case 'phase_changed':
      u.phase = ev.to;
      u.history = [...u.history, ev.to];
      u.awaitingSlot = false;
      u.rateLimited = false; // any phase change means the wait resolved
      break;
```
and in the `blocked` case:
```ts
    case 'blocked':
      u.blocked = ev.reason;
      if (ev.reason === 'awaiting concurrency slot') u.awaitingSlot = true;
      if (ev.reason === 'rate limited') u.rateLimited = true; // exact match to RL_REASON
      break;
```

- [ ] **Step 2: Add the chip to the tile**

In `App.svelte`, next to the existing `awaitingSlot` chip (`{#if u.awaitingSlot}...`), add:
```svelte
                {#if u.rateLimited}<span class="slot disp" title="backing off through an Anthropic rate limit">⏳ RATE-LIMIT</span>{/if}
```
(It reuses the existing `.slot` chip style — no new CSS needed.)

- [ ] **Step 3: Build + typecheck**

Run: `cd cockpit/ui && npm run build && npm run check`
Expected: build OK; 0 errors / 0 warnings.

- [ ] **Step 4: Commit**

```bash
git add cockpit/ui/src/lib/fleet.ts cockpit/ui/src/App.svelte
git commit -m "feat(cockpit): rate-limited chip (mirrors awaiting-slot flag)"
```

---

## Final verification

- [ ] `cargo test --workspace` green; `cargo clippy --workspace --all-targets` clean.
- [ ] `cd cockpit/ui && npm run build && npm run check` clean.
- [ ] Confirm `spikes/SPIKE-RESULTS.md` records the rate-limit signal (or an explicit UNCONFIRMED note); if a real signal was captured that differs from `RL_PATTERNS`, extend `RL_PATTERNS` in `retry.rs` and add a `classify` fixture test for it.
- [ ] Then **superpowers:requesting-code-review** before opening a PR; do not merge.
