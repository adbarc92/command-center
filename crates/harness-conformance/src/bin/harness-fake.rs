//! `harness-fake` — a scripted harness. It passes the conformance kit in `conformant` mode, and
//! `HARNESS_FAKE_MODE` makes it break the protocol in one specific way so the kit's detection can
//! be proven. `HARNESS_FAKE_STEP_MS` paces events so a controller can interrupt mid-run.

use harness_protocol::{
    error_code, method, read_message, version_compatible, write_message, ArtifactKind,
    Capabilities, Delivery, DeliveryEvidence, Empty, ErrorScope, Evidence, Failure, GateKind,
    GateReply, GateRequest, HarnessInfo, InitializeParams, InitializeResult, Isolation,
    MessageKind, Metering, Observation, Outcome, RpcMessage, TestRun, UnitEvent, UnitResult,
    WorkOrder, PROTOCOL_VERSION,
};
use std::io::{self, BufReader, Write};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Conformant,
    Crash,
    Malformed,
    EventAfterResult,
    SilentMetering,
    Hang,
    UnknownMethod,
    AcceptAnyVersion,
    SkipGate,
    IgnoreGateRejection,
    IgnoreHalt,
    FailBeforeGate,
}

fn mode() -> Mode {
    match std::env::var("HARNESS_FAKE_MODE").as_deref() {
        Ok("fail_before_gate") => Mode::FailBeforeGate,
        Ok("crash") => Mode::Crash,
        Ok("malformed") => Mode::Malformed,
        Ok("event_after_result") => Mode::EventAfterResult,
        Ok("silent_metering") => Mode::SilentMetering,
        Ok("hang") => Mode::Hang,
        Ok("unknown_method") => Mode::UnknownMethod,
        Ok("accept_any_version") => Mode::AcceptAnyVersion,
        Ok("skip_gate") => Mode::SkipGate,
        Ok("ignore_gate_rejection") => Mode::IgnoreGateRejection,
        Ok("ignore_halt") => Mode::IgnoreHalt,
        _ => Mode::Conformant,
    }
}

fn pace() -> Duration {
    let ms = std::env::var("HARNESS_FAKE_STEP_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(50);
    Duration::from_millis(ms)
}

const GATE_REQUEST_ID: u64 = 1_000;
const FAKE_HEAD_SHA: &str = "0123456789abcdef0123456789abcdef01234567";

fn capabilities() -> Capabilities {
    Capabilities {
        isolation: Isolation::None,
        metering: Metering::Usd,
        gates: vec![GateKind::Oracle],
        delivery: Delivery::Bundle,
        resume: false,
        halt: true,
    }
}

fn send(msg: &RpcMessage) {
    let mut out = io::stdout().lock();
    if write_message(&mut out, msg).is_err() {
        // The controller went away; nothing left to do.
        std::process::exit(0);
    }
}

fn event(ev: UnitEvent) {
    send(&RpcMessage::notification(method::UNIT_EVENT, &ev));
}

fn observe(observation: Observation) {
    event(UnitEvent::Observed { observation });
}

fn spawn_reader() -> Receiver<RpcMessage> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut input = BufReader::new(io::stdin());
        while let Ok(msg) = read_message(&mut input) {
            if tx.send(msg).is_err() {
                break;
            }
        }
    });
    rx
}

enum Flow {
    Continue,
    Stop,
}

/// Answer a control request. `Some(Flow::Stop)` means the unit must end now, without a result.
fn handle_control(msg: &RpcMessage) -> Option<Flow> {
    match msg.kind() {
        MessageKind::Request { id, method: m }
            if m == method::UNIT_HALT && mode() == Mode::IgnoreHalt =>
        {
            send(&RpcMessage::error(
                id,
                error_code::METHOD_NOT_FOUND,
                "halt ignored (ignore_halt mode)",
            ));
            None
        }
        MessageKind::Request { id, method: m }
            if m == method::UNIT_HALT || m == method::UNIT_ABANDON =>
        {
            send(&RpcMessage::response(id, &Empty {}));
            Some(Flow::Stop)
        }
        MessageKind::Request { id, method: m } if m == method::UNIT_RESUME => {
            send(&RpcMessage::response(id, &Empty {}));
            None
        }
        MessageKind::Request { id, .. } => {
            send(&RpcMessage::error(
                id,
                error_code::METHOD_NOT_FOUND,
                "unsupported in harness-fake",
            ));
            None
        }
        _ => None,
    }
}

/// Wait `pace`, answering control requests as they arrive.
fn pause(rx: &Receiver<RpcMessage>, pace: Duration) -> Flow {
    let deadline = Instant::now() + pace;
    loop {
        let now = Instant::now();
        if now >= deadline {
            return Flow::Continue;
        }
        match rx.recv_timeout(deadline - now) {
            Ok(msg) => {
                if let Some(Flow::Stop) = handle_control(&msg) {
                    return Flow::Stop;
                }
            }
            Err(RecvTimeoutError::Timeout) => return Flow::Continue,
            Err(RecvTimeoutError::Disconnected) => return Flow::Stop,
        }
    }
}

/// Block on the gate reply. `None` means the unit must stop (interrupted or controller gone).
fn await_gate(rx: &Receiver<RpcMessage>) -> Option<bool> {
    loop {
        let msg = rx.recv().ok()?;
        match msg.kind() {
            MessageKind::Response { id } if id == GATE_REQUEST_ID => {
                return Some(
                    msg.result_as::<GateReply>()
                        .map(|r| r.approved)
                        .unwrap_or(false),
                );
            }
            MessageKind::ErrorResponse { id, .. } if id == GATE_REQUEST_ID => return Some(false),
            _ => {
                if let Some(Flow::Stop) = handle_control(&msg) {
                    return None;
                }
            }
        }
    }
}

macro_rules! step {
    ($rx:expr, $pace:expr) => {
        if let Flow::Stop = pause($rx, $pace) {
            return;
        }
    };
}

fn run_unit(mode: Mode, order: &WorkOrder, rx: &Receiver<RpcMessage>) {
    let pace = pace();

    observe(Observation::Provisioned);

    if mode == Mode::FailBeforeGate && order.tier.requires_oracle() {
        let result = UnitResult {
            outcome: Outcome::Failed,
            evidence: None,
            failure: Some(Failure {
                scope: ErrorScope::Agent,
                detail: "failed before gate".into(),
            }),
        };
        send(&RpcMessage::notification(method::UNIT_RESULT, &result));
        return;
    }

    step!(rx, pace);

    match mode {
        Mode::Crash => std::process::exit(3),
        Mode::Hang => loop {
            std::thread::sleep(Duration::from_secs(3_600));
        },
        Mode::Malformed => {
            let mut out = io::stdout().lock();
            let _ = out.write_all(b"this is not json\n");
            let _ = out.flush();
        }
        Mode::UnknownMethod => send(&RpcMessage::notification("unit/whatever", &Empty {})),
        _ => {}
    }

    if order.tier.requires_oracle() && mode != Mode::SkipGate {
        observe(Observation::OracleFrozen);
        let gate = GateRequest::Oracle {
            test_files: vec!["tests/fake.test.js".into()],
            hash: "fake-oracle-hash".into(),
            summary: "one scripted test".into(),
        };
        send(&RpcMessage::request(
            GATE_REQUEST_ID,
            method::GATE_REQUEST,
            &gate,
        ));
        match await_gate(rx) {
            None => return,
            Some(false) if mode != Mode::IgnoreGateRejection => {
                let result = UnitResult {
                    outcome: Outcome::Failed,
                    evidence: None,
                    failure: Some(Failure {
                        scope: ErrorScope::Agent,
                        detail: "oracle rejected".into(),
                    }),
                };
                send(&RpcMessage::notification(method::UNIT_RESULT, &result));
                return;
            }
            Some(_) => {}
        }
    }

    if mode != Mode::SilentMetering {
        event(UnitEvent::Metric {
            tokens_in: 1_200,
            tokens_out: 300,
            cost_usd: 0.02,
            elapsed_ms: pace.as_millis() as u64,
        });
    }
    observe(Observation::BuildFinished);
    step!(rx, pace);
    observe(Observation::ChecksPassed);
    step!(rx, pace);
    observe(Observation::ReviewFinished {
        round: order.caps.min_review_rounds.max(1),
        unresolved_blockers: 0,
        checks_green: true,
    });
    step!(rx, pace);
    event(UnitEvent::Artifact {
        kind: ArtifactKind::Branch,
        reference: order.branch.clone(),
    });

    let result = UnitResult {
        outcome: Outcome::PrOpen,
        evidence: Some(Evidence {
            branch: order.branch.clone(),
            head_sha: FAKE_HEAD_SHA.into(),
            delivery: DeliveryEvidence::Bundle {
                bundle_path: format!("{}.bundle", order.unit_id),
            },
            pr: None,
            test: TestRun {
                command: order.test_cmd.clone(),
                exit_code: 0,
            },
            oracle_hash: order
                .tier
                .requires_oracle()
                .then(|| "fake-oracle-hash".to_string()),
        }),
        failure: None,
    };
    send(&RpcMessage::notification(method::UNIT_RESULT, &result));

    if mode == Mode::EventAfterResult {
        event(UnitEvent::Log {
            stream: harness_protocol::LogStream::System,
            line: "after result".into(),
        });
    }
}

fn main() {
    let mode = mode();
    let rx = spawn_reader();

    let Ok(init) = rx.recv() else { return };
    let MessageKind::Request { id, method: m } = init.kind() else {
        return;
    };
    if m != method::INITIALIZE {
        send(&RpcMessage::error(
            id,
            error_code::METHOD_NOT_FOUND,
            "expected initialize",
        ));
        return;
    }
    let params: InitializeParams = match init.params_as() {
        Ok(p) => p,
        Err(e) => {
            send(&RpcMessage::error(
                id,
                error_code::INVALID_PARAMS,
                e.to_string(),
            ));
            return;
        }
    };
    if mode != Mode::AcceptAnyVersion && !version_compatible(&params.protocol_version) {
        let message = format!("harness-fake speaks protocol {PROTOCOL_VERSION}");
        send(&RpcMessage::error(
            id,
            error_code::PROTOCOL_VERSION_UNSUPPORTED,
            message,
        ));
        return;
    }
    send(&RpcMessage::response(
        id,
        &InitializeResult {
            protocol_version: PROTOCOL_VERSION.into(),
            harness: HarnessInfo {
                name: "harness-fake".into(),
                version: env!("CARGO_PKG_VERSION").into(),
            },
            capabilities: capabilities(),
        },
    ));

    let Ok(start) = rx.recv() else { return };
    let MessageKind::Request { id, method: m } = start.kind() else {
        return;
    };
    if m != method::UNIT_START {
        send(&RpcMessage::error(
            id,
            error_code::METHOD_NOT_FOUND,
            "expected unit/start",
        ));
        return;
    }
    let order: WorkOrder = match start.params_as() {
        Ok(o) => o,
        Err(e) => {
            send(&RpcMessage::error(
                id,
                error_code::INVALID_PARAMS,
                e.to_string(),
            ));
            return;
        }
    };
    send(&RpcMessage::response(id, &Empty {}));
    run_unit(mode, &order, &rx);
}
