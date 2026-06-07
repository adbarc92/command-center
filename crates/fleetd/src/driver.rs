//! The lifecycle driver: runs one unit through the `fleet-core` state machine by
//! calling the `Runner` (container) and `Forge` (host git/PR), emitting the event
//! contract and honoring inbound commands. Fully exercisable against the fakes.

use crate::forge::{Forge, MergeResult, Mergeability};
use crate::retry::{classify, rl_base_secs, rl_cap_secs, rl_max_wait_secs, Backoff, StepOutcome, RL_REASON};
use crate::runner::{ExecOutput, Handle, Runner, UnitSpec};
use crate::{claude_meter, steps};
use fleet_core::{
    gate_met, transition, ArtifactKind, Command, ErrorScope, Event, IterationKind, LogStream,
    Phase, ReviewSnapshot, Trigger,
};
use std::sync::Arc;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

/// An event with its ordering metadata. `ts` is added at the server layer.
#[derive(Debug, Clone, serde::Serialize)]
pub struct EventEnvelope {
    pub unit_id: String,
    pub seq: u64,
    pub event: Event,
}

/// Per-run dependencies, kept in one struct to avoid signature sprawl across the
/// driver's many call sites.
pub struct RunCtx {
    /// Seq to continue from (= units.last_seq on resume, else 0).
    pub start_seq: u64,
    /// Cost already spent by prior runs of this unit (continues the budget).
    pub start_cost: f64,
    /// True when this is a resumed run (skips the already-frozen oracle).
    pub resume: bool,
    /// Phase the driver starts at. `Queued` for a fresh run; `Halted` when
    /// rehydrating a restart-stranded unit so it parks until a `Resume` command.
    pub start_phase: Phase,
    /// Fleet concurrency permits.
    pub permits: Arc<Semaphore>,
}

impl RunCtx {
    /// A standalone context for `run_once` and tests: a fresh run with its own
    /// single permit.
    pub fn standalone() -> Self {
        Self {
            start_seq: 0,
            start_cost: 0.0,
            resume: false,
            start_phase: Phase::Queued,
            permits: Arc::new(Semaphore::new(1)),
        }
    }
}

/// Drive a unit to a terminal phase, returning it. `commands` carries inbound
/// control; `events` receives the outbound stream.
pub async fn run<R: Runner, F: Forge>(
    runner: R,
    forge: F,
    spec: UnitSpec,
    ctx: RunCtx,
    commands: UnboundedReceiver<Command>,
    events: UnboundedSender<EventEnvelope>,
) -> Phase {
    Run {
        runner,
        forge,
        phase: ctx.start_phase,
        seq: ctx.start_seq,
        handle: None,
        cost_usd: ctx.start_cost,
        n_build: 0,
        n_check: 0,
        n_review: 0,
        prev_blockers: None,
        pr_url: None,
        started: std::time::Instant::now(),
        rl_elapsed: std::time::Duration::ZERO,
        permits: ctx.permits,
        permit: None,
        resume: ctx.resume,
        handle_id: None,
        spec,
        commands,
        events,
    }
    .drive()
    .await
}

struct Run<R: Runner, F: Forge> {
    runner: R,
    forge: F,
    spec: UnitSpec,
    commands: UnboundedReceiver<Command>,
    events: UnboundedSender<EventEnvelope>,
    phase: Phase,
    seq: u64,
    handle: Option<Handle>,
    cost_usd: f64,
    n_build: u32,
    n_check: u32,
    n_review: u32,
    prev_blockers: Option<u32>,
    pr_url: Option<String>,
    started: std::time::Instant,
    /// Accumulated rate-limit time (failed-attempt exec + backoff sleeps); exempt
    /// from the wall-clock and the basis for the give-up envelope.
    rl_elapsed: std::time::Duration,
    permits: Arc<Semaphore>,
    permit: Option<OwnedSemaphorePermit>,
    resume: bool,
    /// Container id retained across pause (when `handle` is cleared) so `abandon`
    /// can still discard the volume.
    handle_id: Option<String>,
}

/// Max mergeability polls before treating the PR as unverifiable.
const MAX_MERGEABLE_POLLS: u32 = 10;

impl<R: Runner, F: Forge> Run<R, F> {
    fn emit(&mut self, event: Event) {
        self.seq += 1;
        let _ = self.events.send(EventEnvelope {
            unit_id: self.spec.unit_id.clone(),
            seq: self.seq,
            event,
        });
    }

    /// Apply a trigger; emit `PhaseChanged`. An invalid trigger is a driver bug
    /// and fails the unit loudly rather than silently jumping state.
    fn goto(&mut self, trigger: Trigger, reason: Option<String>, cmd_id: Option<String>) {
        let from = self.phase;
        match transition(from, self.spec.tier, trigger) {
            Some(next) => {
                self.phase = next;
                self.emit(Event::PhaseChanged { from, to: next, reason, cmd_id });
            }
            None => {
                self.emit(Event::Error {
                    scope: ErrorScope::System,
                    retryable: false,
                    detail: format!("invalid trigger {trigger:?} in {from:?}"),
                });
                self.phase = Phase::Failed;
                self.emit(Event::PhaseChanged {
                    from,
                    to: Phase::Failed,
                    reason: Some("invalid transition".into()),
                    cmd_id: None,
                });
            }
        }
    }

    /// Drain currently-available commands. Returns a `Halt` if one arrived;
    /// other commands are invalid mid-run and surface as errors.
    fn poll_halt(&mut self) -> Option<Command> {
        let mut halt = None;
        while let Ok(cmd) = self.commands.try_recv() {
            match cmd {
                Command::Halt { .. } => halt = Some(cmd),
                other => self.emit(Event::Error {
                    scope: ErrorScope::System,
                    retryable: false,
                    detail: format!("command {} not valid in {:?}", other.cmd_id(), self.phase),
                }),
            }
        }
        halt
    }

    /// If a halt is pending, transition to `Halted` and signal the caller to loop.
    fn check_halt(&mut self) -> bool {
        if let Some(cmd) = self.poll_halt() {
            let cid = cmd.cmd_id().to_string();
            self.goto(Trigger::Halt, Some("user halt".into()), Some(cid));
            true
        } else {
            false
        }
    }

    /// Account an exec's usage toward the USD cap; emit a `Metric`. Returns true
    /// if the cumulative cost has breached the cap.
    fn account(&mut self, out: &ExecOutput) -> bool {
        if let Some(u) = out.usage {
            self.cost_usd += u.cost_usd;
            self.emit(Event::Metric {
                tokens_in: u.tokens_in,
                tokens_out: u.tokens_out,
                cost_usd: self.cost_usd,
                elapsed_ms: 0,
            });
        }
        self.cost_usd > self.spec.usd_cap
    }

    /// Remaining USD budget (never negative) — passed to claude as `--max-budget-usd`.
    fn remaining(&self) -> f64 {
        (self.spec.usd_cap - self.cost_usd).max(0.0)
    }

    /// Whether the wall-clock ceiling has been exceeded (0 = disabled).
    fn over_wall_clock(&self) -> bool {
        crate::retry::wall_clock_exceeded(
            self.started.elapsed(),
            self.rl_elapsed,
            self.spec.wall_clock_secs,
        )
    }

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
                            // Channel closed (no interactive commands, e.g. run_once):
                            // no command can interrupt, so just wait out the backoff.
                            Some(None) => {
                                tokio::time::sleep_until(until).await;
                                break;
                            }
                        }
                    }
                    self.rl_elapsed += delay;
                    // loop: re-exec the same step (slot held, container kept).
                }
            }
        }
    }

    async fn drive(mut self) -> Phase {
        loop {
            // Entry-side cleanup for paused states: stop the container (keeping the
            // volume for resume) and free the concurrency slot before parking on a
            // human. Idempotent — runs once per entry since handle/permit are cleared.
            if matches!(self.phase, Phase::NeedsHuman | Phase::Halted) {
                if let Some(h) = self.handle.take() {
                    let _ = self.runner.teardown(&h).await;
                }
                self.permit = None;
            }
            // Wall-clock backstop: an agent that loops/stalls burns time (money)
            // without tripping the per-step USD check. Cap it.
            if self.phase.is_agent_active() && self.over_wall_clock() {
                self.emit(Event::Blocked {
                    reason: "wall-clock cap".into(),
                    cap: Some("wall_clock".into()),
                    detail: format!("exceeded {}s", self.spec.wall_clock_secs),
                });
                self.goto(Trigger::CapBreach, Some("wall-clock cap".into()), None);
                continue;
            }
            match self.phase {
                Phase::Queued => self.goto(Trigger::Start, None, None),

                Phase::Provisioning => {
                    // Acquire a concurrency slot before any container/cost exists.
                    // The wait is visible (Blocked) so the cockpit doesn't show a
                    // silent hang. Covers both fresh (Queued→) and resume (→) paths.
                    if self.permit.is_none() {
                        self.emit(Event::Blocked {
                            reason: "awaiting concurrency slot".into(),
                            cap: None,
                            detail: String::new(),
                        });
                        self.permit =
                            Some(self.permits.clone().acquire_owned().await.expect("semaphore"));
                    }
                    match self.runner.provision(&self.spec).await {
                        Ok(h) => {
                            self.handle_id = Some(h.id.clone());
                            self.handle = Some(h);
                            self.goto(Trigger::Provisioned, None, None);
                        }
                        Err(e) => {
                            self.emit(Event::Error {
                                scope: ErrorScope::Docker,
                                retryable: false,
                                detail: e.to_string(),
                            });
                            self.goto(Trigger::FatalError, Some("provision failed".into()), None);
                        }
                    }
                }

                Phase::Spec => {
                    // On resume the test set is already frozen in the reused volume;
                    // don't re-run/re-charge the oracle or re-trigger approval.
                    if self.resume && self.spec.oracle_frozen {
                        self.goto(Trigger::OracleFrozen, Some("oracle already frozen".into()), None);
                        continue;
                    }
                    if self.check_halt() {
                        continue;
                    }
                    let argv = steps::oracle(&self.spec, self.remaining());
                    let Some(out) =
                        self.agent_exec(IterationKind::Review, 0, LogStream::Agent, true, &argv).await
                    else {
                        continue;
                    };
                    let test_files = out.stdout.clone();
                    // A content hash stands in for the frozen test-set fingerprint.
                    let hash = format!("h{}", test_files.len());
                    self.emit(Event::OracleProposed {
                        test_files,
                        hash,
                        summary: "generated test set".into(),
                    });
                    if self.account(&out) {
                        self.goto(Trigger::CapBreach, Some("usd cap".into()), None);
                        continue;
                    }
                    self.spec.oracle_frozen = true; // frozen; resume will skip Spec
                    self.goto(Trigger::OracleFrozen, None, None);
                }

                Phase::AwaitingOracleApproval => match self.commands.recv().await {
                    Some(cmd) => {
                        let cid = cmd.cmd_id().to_string();
                        match cmd {
                            Command::ApproveOracle { .. } => {
                                self.goto(Trigger::OracleApproved, None, Some(cid))
                            }
                            Command::RejectOracle { .. } => {
                                self.goto(Trigger::OracleRejected, None, Some(cid))
                            }
                            Command::Abandon { .. } => {
                                self.goto(Trigger::Abandon, None, Some(cid))
                            }
                            Command::Halt { .. } => self.goto(Trigger::Halt, None, Some(cid)),
                            other => self.emit(Event::Error {
                                scope: ErrorScope::System,
                                retryable: false,
                                detail: format!("{} not valid while awaiting oracle", other.cmd_id()),
                            }),
                        }
                    }
                    None => self.fail_closed(),
                },

                Phase::Building => {
                    if self.check_halt() {
                        continue;
                    }
                    self.n_build += 1;
                    let n = self.n_build;
                    let findings = match self.prev_blockers {
                        Some(b) if b > 0 => format!("{b} blocker(s) from the last review"),
                        _ => "none".into(),
                    };
                    let argv = steps::build(&self.spec, &findings, self.remaining());
                    let Some(out) =
                        self.agent_exec(IterationKind::Build, n, LogStream::Agent, true, &argv).await
                    else {
                        continue;
                    };
                    if self.account(&out) {
                        self.goto(Trigger::CapBreach, Some("usd cap".into()), None);
                        continue;
                    }
                    // The daemon commits the agent's work (agents edit but don't commit),
                    // so the branch carries the change into the bundle/PR.
                    let handle = self.handle.clone().expect("commit without a handle");
                    if let Err(e) = self.runner.commit_all(&handle, &format!("wip: build {n}")).await {
                        self.emit(Event::Error {
                            scope: ErrorScope::System,
                            retryable: false,
                            detail: format!("wip commit: {e}"),
                        });
                    }
                    self.goto(Trigger::BuildFinished, None, None);
                }

                Phase::Checking => {
                    if self.check_halt() {
                        continue;
                    }
                    self.n_check += 1;
                    let n = self.n_check;
                    let argv = steps::check(&self.spec);
                    let Some(out) =
                        self.agent_exec(IterationKind::Check, n, LogStream::Check, false, &argv).await
                    else {
                        continue;
                    };
                    if self.account(&out) {
                        self.goto(Trigger::CapBreach, Some("usd cap".into()), None);
                        continue;
                    }
                    if out.exit_code != 0 {
                        self.goto(Trigger::ChecksFailed, Some("checks red".into()), None);
                        continue;
                    }
                    // Green checks: route a no-op (empty diff vs base) to NO_CHANGE
                    // rather than opening an empty PR.
                    let handle = self.handle.clone().expect("diff without a handle");
                    let base = self.spec.base_branch.clone();
                    let branch = self.spec.branch.clone();
                    match self.runner.has_diff(&handle, &base, &branch).await {
                        Ok(false) => self.goto(Trigger::EmptyDiff, Some("no changes vs base".into()), None),
                        Ok(true) => self.goto(Trigger::ChecksPassed, None, None),
                        Err(e) => {
                            // On a diff-check error, proceed rather than stall.
                            self.emit(Event::Error {
                                scope: ErrorScope::System,
                                retryable: true,
                                detail: format!("diff check: {e}"),
                            });
                            self.goto(Trigger::ChecksPassed, None, None);
                        }
                    }
                }

                Phase::Reviewing => {
                    if self.check_halt() {
                        continue;
                    }
                    self.n_review += 1;
                    let round = self.n_review;
                    let argv = steps::review(self.remaining(), self.spec.wall_clock_secs);
                    let Some(out) =
                        self.agent_exec(IterationKind::Review, round, LogStream::Agent, true, &argv).await
                    else {
                        continue;
                    };
                    if self.account(&out) {
                        self.goto(Trigger::CapBreach, Some("usd cap".into()), None);
                        continue;
                    }
                    let blockers = parse_blockers(&out.stdout);
                    self.emit(Event::Finding {
                        round,
                        severity: fleet_core::Severity::Blocker,
                        title: format!("{blockers} unresolved blocker(s)"),
                        file: None,
                        resolved: blockers == 0,
                    });
                    let met = gate_met(
                        self.spec.gate,
                        ReviewSnapshot {
                            round,
                            unresolved_blockers: blockers,
                            prev_unresolved_blockers: self.prev_blockers,
                            checks_green: true, // we only enter Reviewing after ChecksPassed
                        },
                    );
                    self.prev_blockers = Some(blockers);
                    self.goto(Trigger::ReviewFinished { gate_met: met }, None, None);
                }

                Phase::MergeCheck => {
                    if self.check_halt() {
                        continue;
                    }
                    let handle = self.handle.clone().expect("merge_check without a handle");
                    let bundle = match self.runner.export_bundle(&handle, &self.spec.branch).await {
                        Ok(p) => p,
                        Err(e) => {
                            self.emit(Event::Error {
                                scope: ErrorScope::System,
                                retryable: false,
                                detail: e.to_string(),
                            });
                            self.goto(Trigger::FatalError, Some("export failed".into()), None);
                            continue;
                        }
                    };
                    let branch = self.spec.branch.clone();
                    self.emit(Event::Artifact {
                        kind: ArtifactKind::Branch,
                        reference: branch.clone(),
                    });
                    match self.forge.trial_merge(&bundle, &branch).await {
                        Ok(MergeResult::Clean) => self.goto(Trigger::MergeClean, None, None),
                        Ok(MergeResult::Conflict) => {
                            self.goto(Trigger::MergeConflict, Some("base conflict".into()), None)
                        }
                        Err(e) => {
                            self.emit(Event::Error {
                                scope: ErrorScope::Github,
                                retryable: true,
                                detail: e.to_string(),
                            });
                            self.goto(Trigger::FatalError, Some("merge failed".into()), None);
                        }
                    }
                }

                Phase::PrOpen => {
                    if self.pr_url.is_none() {
                        let branch = self.spec.branch.clone();
                        match self.forge.open_pr(&branch).await {
                            Ok(url) => {
                                self.emit(Event::Artifact {
                                    kind: ArtifactKind::Pr,
                                    reference: url.clone(),
                                });
                                self.pr_url = Some(url);
                            }
                            Err(e) => {
                                self.emit(Event::Error {
                                    scope: ErrorScope::Github,
                                    retryable: true,
                                    detail: e.to_string(),
                                });
                                self.goto(Trigger::FatalError, Some("open pr failed".into()), None);
                                continue;
                            }
                        }
                    }
                    let url = self.pr_url.clone().unwrap();
                    self.poll_mergeability(&url).await;
                }

                Phase::NeedsHuman | Phase::Halted => match self.commands.recv().await {
                    Some(cmd) => {
                        let cid = cmd.cmd_id().to_string();
                        match cmd {
                            Command::Resume { .. } => self.goto(Trigger::Resume, None, Some(cid)),
                            Command::Abandon { .. } => {
                                // Abandon drops the persisted volume (container is
                                // already torn down from the pause-entry cleanup).
                                if let Some(id) = self.handle_id.take() {
                                    let _ = self.runner.discard(&Handle { id }).await;
                                }
                                self.goto(Trigger::Abandon, None, Some(cid));
                            }
                            Command::Ship { .. } => self.goto(Trigger::Ship, None, Some(cid)),
                            other => self.emit(Event::Error {
                                scope: ErrorScope::System,
                                retryable: false,
                                detail: format!("{} not valid while paused", other.cmd_id()),
                            }),
                        }
                    }
                    None => self.fail_closed(),
                },

                Phase::Done | Phase::NoChange => {
                    // Success: the result is on the host (PR/bundle); drop the volume.
                    if let Some(h) = self.handle.clone() {
                        let _ = self.runner.discard(&h).await;
                    }
                    self.permit = None;
                    let result = if self.phase == Phase::Done { "done" } else { "no_change" };
                    self.emit(Event::Done { result: result.into() });
                    return self.phase;
                }
                Phase::Failed => {
                    // Keep the volume for forensics (a failure rarely produced a PR).
                    if let Some(h) = self.handle.clone() {
                        let _ = self.runner.teardown(&h).await;
                    }
                    self.permit = None;
                    self.emit(Event::Done { result: "failed".into() });
                    return self.phase;
                }
            }
        }
    }

    async fn poll_mergeability(&mut self, url: &str) {
        for _ in 0..MAX_MERGEABLE_POLLS {
            match self.forge.poll_mergeable(url).await {
                Ok(Mergeability::Mergeable) => {
                    self.goto(Trigger::PrMergeable, None, None);
                    return;
                }
                Ok(Mergeability::Dirty) => {
                    self.goto(Trigger::PrDirty, Some("base moved".into()), None);
                    return;
                }
                Ok(Mergeability::Pending) => continue,
                Err(e) => {
                    self.emit(Event::Error {
                        scope: ErrorScope::Github,
                        retryable: true,
                        detail: e.to_string(),
                    });
                }
            }
        }
        // Couldn't confirm mergeability — route to a human (modeled as PrDirty).
        self.emit(Event::Blocked {
            reason: "mergeability poll timed out".into(),
            cap: None,
            detail: format!("after {MAX_MERGEABLE_POLLS} polls"),
        });
        self.goto(Trigger::PrDirty, Some("mergeable poll timeout".into()), None);
    }

    fn fail_closed(&mut self) {
        self.emit(Event::Error {
            scope: ErrorScope::System,
            retryable: false,
            detail: "command channel closed".into(),
        });
        let from = self.phase;
        self.phase = Phase::Failed;
        self.emit(Event::PhaseChanged {
            from,
            to: Phase::Failed,
            reason: Some("control channel closed".into()),
            cmd_id: None,
        });
    }
}

/// Parse a `BLOCKERS=N` line from reviewer output; default 0 if absent.
fn parse_blockers(stdout: &[String]) -> u32 {
    for line in stdout {
        if let Some(rest) = line.strip_prefix("BLOCKERS=") {
            if let Ok(n) = rest.trim().parse() {
                return n;
            }
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::{FakeForge, FakeRunner};
    use fleet_core::{GateConfig, Tier};
    use tokio::sync::mpsc;

    fn spec(tier: Tier, usd_cap: f64, floor: u32) -> UnitSpec {
        UnitSpec {
            unit_id: "u1".into(),
            tier,
            task: "do a thing".into(),
            usd_cap,
            wall_clock_secs: 0,
            gate: GateConfig { min_review_rounds: floor },
            repo_url: "https://github.com/x/y".into(),
            repo_slug: "x/y".into(),
            base_branch: "main".into(),
            branch: "agent/u1".into(),
            test_cmd: "npm test".into(),
            oracle_frozen: false,
        }
    }

    fn phases(events: &[EventEnvelope]) -> Vec<Phase> {
        events
            .iter()
            .filter_map(|e| match &e.event {
                Event::PhaseChanged { to, .. } => Some(*to),
                _ => None,
            })
            .collect()
    }

    fn drain(rx: &mut mpsc::UnboundedReceiver<EventEnvelope>) -> Vec<EventEnvelope> {
        let mut v = vec![];
        while let Ok(e) = rx.try_recv() {
            v.push(e);
        }
        v
    }

    /// One cycle is build + check(pass) + review(blockers).
    fn cycle(blockers: u32) -> Vec<ExecOutput> {
        vec![
            FakeRunner::ok(0.01, &["built"]),
            FakeRunner::ok(0.01, &["checked"]),
            FakeRunner::ok(0.01, &[&format!("BLOCKERS={blockers}")]),
        ]
    }

    #[tokio::test]
    async fn t1_autonomous_runs_to_done() {
        // oracle, then 3 cycles trending 2 -> 1 -> 0 blockers; floor 3.
        let mut script = vec![FakeRunner::ok(0.01, &["test_a.rs", "test_b.rs"])];
        script.extend(cycle(2));
        script.extend(cycle(1));
        script.extend(cycle(0));

        let (ctx, crx) = mpsc::unbounded_channel();
        let (etx, mut erx) = mpsc::unbounded_channel();
        drop(ctx); // T1 needs no commands

        let final_phase = run(
            FakeRunner::new(script),
            FakeForge::default(),
            spec(Tier::T1, 100.0, 3),
            RunCtx::standalone(),
            crx,
            etx,
        )
        .await;

        assert_eq!(final_phase, Phase::Done);
        let evs = drain(&mut erx);
        let seq = phases(&evs);
        // T1 auto-freezes the oracle (no AwaitingOracleApproval) and reaches Done.
        assert!(!seq.contains(&Phase::AwaitingOracleApproval));
        assert!(seq.contains(&Phase::MergeCheck));
        assert!(seq.contains(&Phase::PrOpen));
        assert_eq!(seq.last(), Some(&Phase::Done));
        // It really looped three review rounds before the gate opened.
        let reviews = evs
            .iter()
            .filter(|e| matches!(e.event, Event::Iteration { kind: IterationKind::Review, n } if n >= 1))
            .count();
        assert_eq!(reviews, 3);
    }

    #[tokio::test]
    async fn discard_on_done_not_teardown() {
        let mut script = vec![FakeRunner::ok(0.01, &["test_a.rs"])]; // oracle
        script.extend(cycle(0)); // build, check(pass), review(BLOCKERS=0); floor 1
        let runner = FakeRunner::new(script);
        let discards = runner.discards.clone();
        let teardowns = runner.teardowns.clone();
        let (ctx, crx) = mpsc::unbounded_channel();
        let (etx, _erx) = mpsc::unbounded_channel();
        drop(ctx);
        let phase = run(
            runner,
            FakeForge::default(),
            spec(Tier::T1, 100.0, 1),
            RunCtx::standalone(),
            crx,
            etx,
        )
        .await;
        assert_eq!(phase, Phase::Done);
        assert_eq!(discards.load(std::sync::atomic::Ordering::Relaxed), 1, "Done discards the volume");
        assert_eq!(teardowns.load(std::sync::atomic::Ordering::Relaxed), 0, "Done keeps no volume");
    }

    #[tokio::test]
    async fn resume_skips_oracle_and_continues_cost_and_seq() {
        // A resumed unit: the oracle is already frozen, so the script has NO
        // oracle call — just one build/check/review cycle (floor 1 opens the gate).
        let script = cycle(0);
        let (ctx_tx, crx) = mpsc::unbounded_channel();
        let (etx, mut erx) = mpsc::unbounded_channel();
        drop(ctx_tx);
        let permits = std::sync::Arc::new(tokio::sync::Semaphore::new(1));
        let mut s = spec(Tier::T1, 100.0, 1);
        s.oracle_frozen = true;

        let final_phase = run(
            FakeRunner::new(script),
            FakeForge::default(),
            s,
            RunCtx {
                start_seq: 5,
                start_cost: 0.5,
                resume: true,
                start_phase: Phase::Queued,
                permits: permits.clone(),
            },
            crx,
            etx,
        )
        .await;

        assert_eq!(final_phase, Phase::Done);
        let evs = drain(&mut erx);
        // (a) the oracle was NOT re-run on resume
        assert!(
            !evs.iter().any(|e| matches!(e.event, Event::OracleProposed { .. })),
            "resume must not re-run the oracle"
        );
        // (b) seq continues from start_seq (first emitted event has seq > 5)
        assert!(evs.first().unwrap().seq > 5, "seq must continue from start_seq");
        // (c) cost continues from start_cost (a metric reflects >= 0.5)
        let max_cost = evs
            .iter()
            .filter_map(|e| match e.event {
                Event::Metric { cost_usd, .. } => Some(cost_usd),
                _ => None,
            })
            .fold(0.0, f64::max);
        assert!(max_cost >= 0.5, "cost must continue from start_cost, got {max_cost}");
        // (d) the permit is released once the unit finishes
        assert_eq!(permits.available_permits(), 1, "permit released at terminal");
    }

    #[tokio::test]
    async fn driver_waits_for_a_concurrency_slot() {
        // One slot, already taken: the driver must announce the wait (Blocked) and
        // must NOT provision until the slot frees — the concurrency guarantee.
        let permits = std::sync::Arc::new(tokio::sync::Semaphore::new(1));
        let held = permits.clone().acquire_owned().await.unwrap();

        let mut s = spec(Tier::T1, 100.0, 1);
        s.oracle_frozen = true; // resumed unit → script has no oracle call
        let (ctx_tx, crx) = mpsc::unbounded_channel();
        let (etx, mut erx) = mpsc::unbounded_channel();
        drop(ctx_tx);
        let h = tokio::spawn(run(
            FakeRunner::new(cycle(0)),
            FakeForge::default(),
            s,
            RunCtx {
                start_seq: 0,
                start_cost: 0.0,
                resume: true,
                start_phase: Phase::Queued,
                permits: permits.clone(),
            },
            crx,
            etx,
        ));

        // Drain to the "awaiting concurrency slot" signal; it must not have left
        // Provisioning (no container/cost) while the only slot is held.
        loop {
            let e = erx.recv().await.unwrap();
            if matches!(&e.event, Event::Blocked { reason, .. } if reason == "awaiting concurrency slot") {
                break;
            }
            assert!(
                !matches!(e.event, Event::PhaseChanged { from: Phase::Provisioning, .. }),
                "must not leave Provisioning before acquiring a slot"
            );
        }
        assert_eq!(permits.available_permits(), 0, "the only slot is held");

        // Free the slot → the driver acquires it and runs to completion.
        drop(held);
        assert_eq!(h.await.unwrap(), Phase::Done);
        assert_eq!(permits.available_permits(), 1, "permit released at terminal");
    }

    #[tokio::test]
    async fn empty_diff_routes_to_no_change() {
        // oracle + build + green check, but the branch has no diff vs base.
        let script = vec![
            FakeRunner::ok(0.01, &["test_a.rs"]), // oracle
            FakeRunner::ok(0.01, &["built"]),     // build
            FakeRunner::ok(0.01, &["checked"]),   // check passes
        ];
        let (ctx, crx) = mpsc::unbounded_channel();
        let (etx, _erx) = mpsc::unbounded_channel();
        drop(ctx);
        let final_phase = run(
            FakeRunner::new(script).empty_diff(),
            FakeForge::default(),
            spec(Tier::T1, 100.0, 3),
            RunCtx::standalone(),
            crx,
            etx,
        )
        .await;
        assert_eq!(final_phase, Phase::NoChange);
    }

    #[tokio::test]
    async fn t2_waits_for_oracle_approval_then_finishes() {
        let mut script = vec![FakeRunner::ok(0.01, &["test_a.rs"])];
        script.extend(cycle(0)); // floor 1 → first clean round opens the gate

        let (ctx, crx) = mpsc::unbounded_channel();
        let (etx, mut erx) = mpsc::unbounded_channel();

        let handle = tokio::spawn(run(
            FakeRunner::new(script),
            FakeForge::default(),
            spec(Tier::T2, 100.0, 1),
            RunCtx::standalone(),
            crx,
            etx,
        ));

        // Wait until the unit parks at AwaitingOracleApproval, accumulating events.
        let mut all = vec![];
        loop {
            let e = erx.recv().await.expect("stream closed before approval gate");
            let park = matches!(e.event, Event::PhaseChanged { to: Phase::AwaitingOracleApproval, .. });
            all.push(e);
            if park {
                break;
            }
        }
        ctx.send(Command::ApproveOracle { cmd_id: "c1".into(), edited_test_files: None }).unwrap();

        let final_phase = handle.await.unwrap();
        assert_eq!(final_phase, Phase::Done);
        all.extend(drain(&mut erx));
        let seq = phases(&all);
        assert!(seq.contains(&Phase::AwaitingOracleApproval));
        assert!(seq.contains(&Phase::Building));
    }

    #[tokio::test]
    async fn cap_breach_routes_to_needs_human() {
        // oracle is cheap; the first build blows the $0.5 cap.
        let script = vec![FakeRunner::ok(0.1, &["test_a.rs"]), FakeRunner::ok(1.0, &["built"])];

        let (ctx, crx) = mpsc::unbounded_channel();
        let (etx, mut erx) = mpsc::unbounded_channel();

        let handle = tokio::spawn(run(
            FakeRunner::new(script),
            FakeForge::default(),
            spec(Tier::T1, 0.5, 3),
            RunCtx::standalone(),
            crx,
            etx,
        ));

        // It should park at NeedsHuman; then we abandon to terminate.
        loop {
            let e = erx.recv().await.expect("stream closed before cap breach");
            if matches!(e.event, Event::PhaseChanged { to: Phase::NeedsHuman, .. }) {
                break;
            }
        }
        ctx.send(Command::Abandon { cmd_id: "c2".into() }).unwrap();

        let final_phase = handle.await.unwrap();
        assert_eq!(final_phase, Phase::Failed);
    }

    #[tokio::test]
    async fn halt_then_abandon() {
        let script = vec![FakeRunner::ok(0.01, &["test_a.rs"])]; // may not all be used
        let (ctx, crx) = mpsc::unbounded_channel();
        let (etx, mut erx) = mpsc::unbounded_channel();

        // Pre-queue a halt; the first agent-active phase (Spec) will pick it up.
        ctx.send(Command::Halt { cmd_id: "h1".into() }).unwrap();

        let handle = tokio::spawn(run(
            FakeRunner::new(script),
            FakeForge::default(),
            spec(Tier::T1, 100.0, 3),
            RunCtx::standalone(),
            crx,
            etx,
        ));

        loop {
            let e = erx.recv().await.expect("stream closed before halt");
            if matches!(e.event, Event::PhaseChanged { to: Phase::Halted, .. }) {
                break;
            }
        }
        ctx.send(Command::Abandon { cmd_id: "a1".into() }).unwrap();
        assert_eq!(handle.await.unwrap(), Phase::Failed);
    }

    #[test]
    fn parse_blockers_reads_the_marker() {
        assert_eq!(parse_blockers(&["noise".into(), "BLOCKERS=3".into()]), 3);
        assert_eq!(parse_blockers(&["nothing here".into()]), 0);
    }

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

    #[tokio::test(start_paused = true)]
    async fn rate_limit_cost_breaching_cap_parks_via_cap_breach() {
        // A rate-limited attempt that itself bills over the cap must route to
        // NeedsHuman via CapBreach (checked before backoff) — not loop for an hour.
        let rl_costly = ExecOutput {
            exit_code: 1,
            stdout: vec![],
            stderr: vec!["API Error: 429 rate limit exceeded".into()],
            usage: Some(crate::runner::Usage { tokens_in: 0, tokens_out: 0, cost_usd: 1.0 }),
        };
        let (ctx, crx) = mpsc::unbounded_channel();
        let (etx, mut erx) = mpsc::unbounded_channel();
        let h = tokio::spawn(run(
            FakeRunner::new(vec![]).always(rl_costly),
            FakeForge::default(),
            spec(Tier::T1, 0.5, 1), // cap $0.5; first attempt bills $1.0
            RunCtx::standalone(),
            crx,
            etx,
        ));
        let mut saw_rl_blocked = false;
        loop {
            let e = erx.recv().await.expect("stream closed before NeedsHuman");
            if matches!(&e.event, Event::Blocked { reason, .. } if reason == crate::retry::RL_REASON) {
                saw_rl_blocked = true;
            }
            if let Event::PhaseChanged { to: Phase::NeedsHuman, reason, .. } = &e.event {
                assert_eq!(reason.as_deref(), Some("usd cap"), "parked via CapBreach, not RetriesExhausted");
                break;
            }
        }
        assert!(!saw_rl_blocked, "cap breach short-circuits before any backoff Blocked");
        ctx.send(Command::Abandon { cmd_id: "a".into() }).unwrap();
        let _ = h.await;
    }

    #[tokio::test(start_paused = true)]
    async fn halt_during_backoff_interrupts_to_halted() {
        let (ctx, crx) = mpsc::unbounded_channel();
        let (etx, mut erx) = mpsc::unbounded_channel();
        let h = tokio::spawn(run(
            FakeRunner::new(vec![]).always(FakeRunner::rate_limited()),
            FakeForge::default(),
            spec(Tier::T1, 100.0, 1),
            RunCtx::standalone(),
            crx,
            etx,
        ));
        // Wait until it's backing off, then halt mid-wait.
        loop {
            let e = erx.recv().await.expect("closed before backoff");
            if matches!(&e.event, Event::Blocked { reason, .. } if reason == crate::retry::RL_REASON) {
                break;
            }
        }
        ctx.send(Command::Halt { cmd_id: "h".into() }).unwrap();
        loop {
            let e = erx.recv().await.expect("closed before Halted");
            if matches!(e.event, Event::PhaseChanged { to: Phase::Halted, .. }) {
                break;
            }
        }
        ctx.send(Command::Abandon { cmd_id: "a".into() }).unwrap(); // Halted -> Failed
        let _ = h.await;
    }

    #[tokio::test(start_paused = true)]
    async fn non_halt_command_during_backoff_errors_and_keeps_retrying() {
        let (ctx, crx) = mpsc::unbounded_channel();
        let (etx, mut erx) = mpsc::unbounded_channel();
        let h = tokio::spawn(run(
            FakeRunner::new(vec![]).always(FakeRunner::rate_limited()),
            FakeForge::default(),
            spec(Tier::T1, 100.0, 1),
            RunCtx::standalone(),
            crx,
            etx,
        ));
        // Wait for the first backoff, then send a NON-Halt command into the wait.
        loop {
            let e = erx.recv().await.expect("closed before backoff");
            if matches!(&e.event, Event::Blocked { reason, .. } if reason == crate::retry::RL_REASON) {
                break;
            }
        }
        ctx.send(Command::Resume { cmd_id: "r".into() }).unwrap();
        // It emits a "not valid" error and keeps retrying until the envelope -> NeedsHuman.
        let mut saw_invalid = false;
        loop {
            let e = erx.recv().await.expect("closed before NeedsHuman");
            if matches!(&e.event, Event::Error { detail, .. } if detail.contains("not valid")) {
                saw_invalid = true;
            }
            if matches!(e.event, Event::PhaseChanged { to: Phase::NeedsHuman, .. }) {
                break;
            }
        }
        assert!(saw_invalid, "a non-Halt command during backoff emits a 'not valid' error");
        ctx.send(Command::Abandon { cmd_id: "a".into() }).unwrap();
        let _ = h.await;
    }
}
