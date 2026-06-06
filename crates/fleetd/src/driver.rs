//! The lifecycle driver: runs one unit through the `fleet-core` state machine by
//! calling the `Runner` (container) and `Forge` (host git/PR), emitting the event
//! contract and honoring inbound commands. Fully exercisable against the fakes.

use crate::forge::{Forge, MergeResult, Mergeability};
use crate::runner::{ExecOutput, Handle, Runner, UnitSpec};
use crate::{claude_meter, steps};
use fleet_core::{
    gate_met, transition, ArtifactKind, Command, ErrorScope, Event, IterationKind, LogStream,
    Phase, ReviewSnapshot, Trigger,
};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

/// An event with its ordering metadata. `ts` is added at the server layer.
#[derive(Debug, Clone, serde::Serialize)]
pub struct EventEnvelope {
    pub unit_id: String,
    pub seq: u64,
    pub event: Event,
}

/// Drive a unit to a terminal phase, returning it. `commands` carries inbound
/// control; `events` receives the outbound stream.
pub async fn run<R: Runner, F: Forge>(
    runner: R,
    forge: F,
    spec: UnitSpec,
    commands: UnboundedReceiver<Command>,
    events: UnboundedSender<EventEnvelope>,
) -> Phase {
    Run {
        runner,
        forge,
        phase: Phase::Queued,
        seq: 0,
        handle: None,
        cost_usd: 0.0,
        n_build: 0,
        n_check: 0,
        n_review: 0,
        prev_blockers: None,
        pr_url: None,
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

    /// Run one step in `steps::WORKDIR`, streaming stdout as `Log` events and
    /// back-filling usage from claude's stream-json if the runner didn't supply
    /// it. On runner failure, fail the unit and return `None`.
    async fn agent_exec(
        &mut self,
        kind: IterationKind,
        n: u32,
        stream: LogStream,
        argv: &[String],
    ) -> Option<ExecOutput> {
        self.emit(Event::Iteration { kind, n });
        let handle = self.handle.clone().expect("agent_exec without a handle");
        match self.runner.exec(&handle, steps::WORKDIR, argv).await {
            Ok(mut out) => {
                for line in &out.stdout {
                    self.emit(Event::Log { stream, line: line.clone() });
                }
                if out.usage.is_none() {
                    out.usage = claude_meter::parse_usage(&out.stdout);
                }
                Some(out)
            }
            Err(e) => {
                self.emit(Event::Error {
                    scope: ErrorScope::Agent,
                    retryable: false,
                    detail: e.to_string(),
                });
                self.goto(Trigger::FatalError, Some("exec failed".into()), None);
                None
            }
        }
    }

    async fn drive(mut self) -> Phase {
        loop {
            match self.phase {
                Phase::Queued => self.goto(Trigger::Start, None, None),

                Phase::Provisioning => match self.runner.provision(&self.spec).await {
                    Ok(h) => {
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
                },

                Phase::Spec => {
                    if self.check_halt() {
                        continue;
                    }
                    let argv = steps::oracle(&self.spec, self.remaining());
                    let Some(out) =
                        self.agent_exec(IterationKind::Review, 0, LogStream::Agent, &argv).await
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
                        self.agent_exec(IterationKind::Build, n, LogStream::Agent, &argv).await
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
                        self.agent_exec(IterationKind::Check, n, LogStream::Check, &argv).await
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
                    let argv = steps::review(self.remaining());
                    let Some(out) =
                        self.agent_exec(IterationKind::Review, round, LogStream::Agent, &argv).await
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
                            Command::Abandon { .. } => self.goto(Trigger::Abandon, None, Some(cid)),
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

                Phase::Done | Phase::NoChange | Phase::Failed => {
                    if let Some(h) = self.handle.clone() {
                        let _ = self.runner.teardown(&h).await;
                    }
                    let result = match self.phase {
                        Phase::Done => "done",
                        Phase::NoChange => "no_change",
                        _ => "failed",
                    };
                    self.emit(Event::Done { result: result.into() });
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
            gate: GateConfig { min_review_rounds: floor },
            repo_url: "https://github.com/x/y".into(),
            repo_slug: "x/y".into(),
            base_branch: "main".into(),
            branch: "agent/u1".into(),
            test_cmd: "npm test".into(),
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
}
