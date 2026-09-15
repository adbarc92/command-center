//! The conformance cases. Each spawns a fresh harness.

use crate::fixtures::work_order;
use crate::session::{Recv, Session};
use crate::{CaseOutcome, CaseReport, KitConfig, Violation};
use harness_protocol::{
    error_code, method, Capabilities, Empty, GateKind, GateReply, InitializeParams,
    InitializeResult, MessageKind, Metering, Outcome, RpcMessage, Tier, UnitEvent, UnitResult,
    WorkOrder, PROTOCOL_VERSION,
};
use std::time::Instant;

const INIT_ID: u64 = 1;
const START_ID: u64 = 2;
const INTERRUPT_ID: u64 = 3;

/// What the kit does when a `gate/request` arrives.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum GatePolicy {
    Unexpected,
    Approve,
    Reject,
}

/// What one unit run produced.
#[derive(Default)]
pub(crate) struct Transcript {
    pub metrics: usize,
    pub gate_requests: usize,
    /// `None` when the run ended by an honoured interrupt.
    pub result: Option<UnitResult>,
}

pub(crate) fn pass(name: &'static str) -> CaseReport {
    CaseReport {
        name,
        outcome: CaseOutcome::Pass,
    }
}

pub(crate) fn fail(name: &'static str, violation: Violation) -> CaseReport {
    CaseReport {
        name,
        outcome: CaseOutcome::Fail(violation),
    }
}

/// Spawn, initialize at our version, and return the declared capabilities.
pub(crate) fn start_session(cfg: &KitConfig) -> Result<(Session, Capabilities), Violation> {
    let mut session = Session::spawn(cfg).map_err(Violation::SpawnFailed)?;
    session.send(&RpcMessage::request(
        INIT_ID,
        method::INITIALIZE,
        &InitializeParams {
            protocol_version: PROTOCOL_VERSION.into(),
        },
    ));
    let reply = match session.recv(cfg.grace) {
        Recv::Message(msg) => msg,
        Recv::Malformed(line) => return Err(Violation::Malformed { line }),
        Recv::Eof | Recv::Timeout => return Err(Violation::NoInitializeResponse),
    };
    match reply.kind() {
        MessageKind::Response { id: INIT_ID } => {
            let result: InitializeResult = reply
                .result_as()
                .map_err(|e| Violation::InvalidResult(format!("initialize result: {e}")))?;
            Ok((session, result.capabilities))
        }
        MessageKind::ErrorResponse { id: INIT_ID, error } => {
            Err(Violation::InitializeRejected(error.message.clone()))
        }
        _ => Err(Violation::NoInitializeResponse),
    }
}

fn validate_result(result: &UnitResult) -> Result<(), Violation> {
    match result.outcome {
        Outcome::PrOpen if result.evidence.is_none() => {
            Err(Violation::InvalidResult("pr_open without evidence".into()))
        }
        Outcome::Failed if result.failure.is_none() => {
            Err(Violation::InvalidResult("failed without failure".into()))
        }
        _ => Ok(()),
    }
}

/// After `unit/result` nothing more may arrive, and the process must exit within grace.
fn finish(
    session: &mut Session,
    cfg: &KitConfig,
    transcript: Transcript,
) -> Result<Transcript, Violation> {
    session.close_stdin();
    match session.recv(cfg.grace) {
        Recv::Message(msg) => {
            let name = msg.method.clone().unwrap_or_else(|| "<response>".into());
            return Err(Violation::MessageAfterResult { method: name });
        }
        Recv::Malformed(line) => return Err(Violation::Malformed { line }),
        Recv::Eof | Recv::Timeout => {}
    }
    if session.wait_exit(cfg.grace) {
        Ok(transcript)
    } else {
        session.kill();
        Err(Violation::DidNotExitAfterResult)
    }
}

/// Start `order` and read until the unit ends. `gate` answers any `gate/request`. If `interrupt`
/// is a method name, it is sent as a request after the first `unit/event`; from then on the harness
/// has `cfg.grace` to answer it and exit, without sending `unit/result`.
pub(crate) fn drive(
    session: &mut Session,
    cfg: &KitConfig,
    caps: &Capabilities,
    order: &WorkOrder,
    gate: GatePolicy,
    interrupt: Option<&'static str>,
) -> Result<Transcript, Violation> {
    let mut deadline = Instant::now() + cfg.wall_clock;
    session.send(&RpcMessage::request(START_ID, method::UNIT_START, order));
    let mut transcript = Transcript::default();
    let mut interrupt_sent: Option<&'static str> = None;
    let mut interrupt_acked = false;

    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let received = if remaining.is_zero() {
            Recv::Timeout
        } else {
            session.recv(remaining)
        };
        let msg = match received {
            Recv::Message(msg) => msg,
            Recv::Malformed(line) => return Err(Violation::Malformed { line }),
            Recv::Timeout => {
                session.kill();
                return Err(match interrupt_sent {
                    Some(ctl) => Violation::InterruptNotHonored {
                        method: ctl.to_string(),
                    },
                    None => Violation::WallClockExceeded,
                });
            }
            Recv::Eof => {
                return match interrupt_sent {
                    Some(_) if interrupt_acked && session.wait_exit(cfg.grace) => Ok(transcript),
                    Some(ctl) => {
                        session.kill();
                        Err(Violation::InterruptNotHonored {
                            method: ctl.to_string(),
                        })
                    }
                    None => Err(Violation::ExitedWithoutResult),
                };
            }
        };

        match msg.kind() {
            MessageKind::Response { id: START_ID } => {}
            MessageKind::Response { id: INTERRUPT_ID } if interrupt_sent.is_some() => {
                interrupt_acked = true;
            }
            MessageKind::ErrorResponse {
                id: INTERRUPT_ID, ..
            } => {
                let ctl = interrupt_sent.unwrap_or("interrupt");
                return Err(Violation::InterruptNotHonored {
                    method: ctl.to_string(),
                });
            }
            MessageKind::Notification { method: m } if m == method::UNIT_EVENT => {
                let ev: UnitEvent = msg
                    .params_as()
                    .map_err(|e| Violation::InvalidResult(format!("unit/event params: {e}")))?;
                if matches!(ev, UnitEvent::Metric { .. }) {
                    transcript.metrics += 1;
                }
                if let (Some(ctl), None) = (interrupt, interrupt_sent) {
                    session.send(&RpcMessage::request(INTERRUPT_ID, ctl, &Empty {}));
                    interrupt_sent = Some(ctl);
                    deadline = deadline.min(Instant::now() + cfg.grace);
                }
            }
            MessageKind::Notification { method: m } if m == method::UNIT_RESULT => {
                if let Some(ctl) = interrupt_sent {
                    return Err(Violation::InterruptNotHonored {
                        method: ctl.to_string(),
                    });
                }
                let result: UnitResult = msg
                    .params_as()
                    .map_err(|e| Violation::InvalidResult(format!("unit/result params: {e}")))?;
                validate_result(&result)?;
                if caps.metering == Metering::Usd
                    && transcript.metrics == 0
                    && result.outcome != Outcome::Failed
                {
                    return Err(Violation::MeteringDeclaredButSilent);
                }
                transcript.result = Some(result);
                return finish(session, cfg, transcript);
            }
            MessageKind::Request { id, method: m } if m == method::GATE_REQUEST => {
                transcript.gate_requests += 1;
                let approved = match gate {
                    GatePolicy::Unexpected => return Err(Violation::UnexpectedGateRequest),
                    GatePolicy::Approve => true,
                    GatePolicy::Reject => false,
                };
                session.send(&RpcMessage::response(
                    id,
                    &GateReply {
                        approved,
                        edited_test_files: None,
                    },
                ));
            }
            MessageKind::Notification { method: m } | MessageKind::Request { method: m, .. } => {
                return Err(Violation::UnknownMethod {
                    method: m.to_string(),
                });
            }
            _ => return Err(Violation::InvalidMessage),
        }
    }
}

/// A harness must refuse `initialize` from a different protocol major with `-32001`.
pub(crate) fn version_mismatch_refused(cfg: &KitConfig) -> CaseReport {
    const NAME: &str = "version_mismatch_refused";
    let mut session = match Session::spawn(cfg) {
        Ok(s) => s,
        Err(e) => return fail(NAME, Violation::SpawnFailed(e)),
    };
    session.send(&RpcMessage::request(
        INIT_ID,
        method::INITIALIZE,
        &InitializeParams {
            protocol_version: "99.0".into(),
        },
    ));
    match session.recv(cfg.grace) {
        Recv::Message(msg) => match msg.kind() {
            MessageKind::ErrorResponse { id: INIT_ID, error }
                if error.code == error_code::PROTOCOL_VERSION_UNSUPPORTED =>
            {
                pass(NAME)
            }
            MessageKind::Response { id: INIT_ID } => fail(NAME, Violation::VersionMismatchAccepted),
            _ => fail(NAME, Violation::InvalidMessage),
        },
        Recv::Malformed(line) => fail(NAME, Violation::Malformed { line }),
        Recv::Eof | Recv::Timeout => fail(NAME, Violation::NoInitializeResponse),
    }
}

/// A T1 unit runs to a well-formed result with no gate request, then the harness exits.
pub(crate) fn happy_path_t1(cfg: &KitConfig) -> CaseReport {
    const NAME: &str = "happy_path_t1";
    let (mut session, caps) = match start_session(cfg) {
        Ok(started) => started,
        Err(v) => return fail(NAME, v),
    };
    match drive(
        &mut session,
        cfg,
        &caps,
        &work_order(Tier::T1),
        GatePolicy::Unexpected,
        None,
    ) {
        Ok(t) if t.result.is_some() => pass(NAME),
        Ok(_) => fail(NAME, Violation::ExitedWithoutResult),
        Err(v) => fail(NAME, v),
    }
}

fn skipped(name: &'static str, why: &str) -> CaseReport {
    CaseReport {
        name,
        outcome: CaseOutcome::Skipped(why.into()),
    }
}

fn has_oracle_gate(caps: &Capabilities) -> bool {
    caps.gates.contains(&GateKind::Oracle)
}

/// T2: the harness asks for oracle approval before building; approved, it ends well-formed.
pub(crate) fn gate_approved_t2(cfg: &KitConfig) -> CaseReport {
    const NAME: &str = "gate_approved_t2";
    let (mut session, caps) = match start_session(cfg) {
        Ok(started) => started,
        Err(v) => return fail(NAME, v),
    };
    if !has_oracle_gate(&caps) {
        return skipped(
            NAME,
            "harness declares no oracle gate, so it may only take T1 units",
        );
    }
    match drive(
        &mut session,
        cfg,
        &caps,
        &work_order(Tier::T2),
        GatePolicy::Approve,
        None,
    ) {
        Err(v) => fail(NAME, v),
        Ok(t) => match t.result {
            None => fail(NAME, Violation::ExitedWithoutResult),
            Some(ref r) if t.gate_requests == 0 && r.outcome == Outcome::PrOpen => {
                fail(NAME, Violation::GateNotRequested)
            }
            Some(_) if t.gate_requests == 0 => skipped(
                NAME,
                "harness ended before reaching the gate; the approval path was not exercised",
            ),
            Some(_) => pass(NAME),
        },
    }
}

/// T2: a rejected oracle must never lead to `pr_open`.
pub(crate) fn gate_rejected_t2(cfg: &KitConfig) -> CaseReport {
    const NAME: &str = "gate_rejected_t2";
    let (mut session, caps) = match start_session(cfg) {
        Ok(started) => started,
        Err(v) => return fail(NAME, v),
    };
    if !has_oracle_gate(&caps) {
        return skipped(
            NAME,
            "harness declares no oracle gate, so it may only take T1 units",
        );
    }
    match drive(
        &mut session,
        cfg,
        &caps,
        &work_order(Tier::T2),
        GatePolicy::Reject,
        None,
    ) {
        Err(v) => fail(NAME, v),
        Ok(t) => match t.result {
            None => fail(NAME, Violation::ExitedWithoutResult),
            Some(ref r) if t.gate_requests == 0 && r.outcome == Outcome::PrOpen => {
                fail(NAME, Violation::GateNotRequested)
            }
            Some(_) if t.gate_requests == 0 => skipped(
                NAME,
                "harness ended before reaching the gate; the rejection path was not exercised",
            ),
            Some(ref r) if r.outcome == Outcome::PrOpen => {
                fail(NAME, Violation::GateRejectionIgnored)
            }
            Some(_) => pass(NAME),
        },
    }
}

/// After the first `unit/event`, send `ctl`; the harness must answer it and exit without a result.
fn interrupt_case(
    cfg: &KitConfig,
    name: &'static str,
    ctl: &'static str,
    needs_halt: bool,
) -> CaseReport {
    let (mut session, caps) = match start_session(cfg) {
        Ok(started) => started,
        Err(v) => return fail(name, v),
    };
    if needs_halt && !caps.halt {
        return skipped(name, "harness declares halt: false");
    }
    match drive(
        &mut session,
        cfg,
        &caps,
        &work_order(Tier::T1),
        GatePolicy::Unexpected,
        Some(ctl),
    ) {
        Ok(t) if t.result.is_none() => pass(name),
        Ok(_) => fail(
            name,
            Violation::InterruptNotHonored {
                method: ctl.to_string(),
            },
        ),
        Err(v) => fail(name, v),
    }
}

pub(crate) fn halt(cfg: &KitConfig) -> CaseReport {
    interrupt_case(cfg, "halt", method::UNIT_HALT, true)
}

pub(crate) fn abandon(cfg: &KitConfig) -> CaseReport {
    interrupt_case(cfg, "abandon", method::UNIT_ABANDON, false)
}

pub fn run_all(cfg: &KitConfig) -> Vec<CaseReport> {
    vec![
        version_mismatch_refused(cfg),
        happy_path_t1(cfg),
        gate_approved_t2(cfg),
        gate_rejected_t2(cfg),
        halt(cfg),
        abandon(cfg),
    ]
}
