//! Drives `harness-fake` by hand over the codec, before the kit exists.

use harness_protocol::{
    error_code, method, read_message, write_message, Caps, InitializeParams, InitializeResult,
    MessageKind, Outcome, ReadError, Repo, RpcMessage, Tier, UnitEvent, UnitResult, WorkItem,
    WorkItemKind, WorkOrder, PROTOCOL_VERSION,
};
use std::io::BufReader;
use std::process::{Command, Stdio};

fn order(tier: Tier) -> WorkOrder {
    WorkOrder {
        unit_id: "u1".into(),
        work_item: WorkItem {
            kind: WorkItemKind::RoadmapItem,
            reference: "sandbox#smoke".into(),
            fingerprint: None,
        },
        tier,
        task: "smoke".into(),
        repo: Repo {
            url: "https://example.invalid/sandbox.git".into(),
            slug: "example/sandbox".into(),
            base_branch: "main".into(),
        },
        branch: "agent/u1".into(),
        test_cmd: "node --test".into(),
        caps: Caps {
            usd: 1.0,
            wall_clock_secs: 60,
            min_review_rounds: 1,
        },
        resume: None,
    }
}

fn spawn_fake() -> std::process::Child {
    Command::new(env!("CARGO_BIN_EXE_harness-fake"))
        .env("HARNESS_FAKE_STEP_MS", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("harness-fake builds and starts")
}

#[test]
fn conformant_fake_runs_a_t1_unit_to_pr_open() {
    let mut child = spawn_fake();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    let init = RpcMessage::request(
        1,
        method::INITIALIZE,
        &InitializeParams {
            protocol_version: PROTOCOL_VERSION.into(),
        },
    );
    write_message(&mut stdin, &init).unwrap();
    let reply: InitializeResult = read_message(&mut stdout).unwrap().result_as().unwrap();
    assert_eq!(reply.harness.name, "harness-fake");

    write_message(
        &mut stdin,
        &RpcMessage::request(2, method::UNIT_START, &order(Tier::T1)),
    )
    .unwrap();
    assert_eq!(
        read_message(&mut stdout).unwrap().kind(),
        MessageKind::Response { id: 2 }
    );

    let mut saw_metric = false;
    let result = loop {
        let msg = read_message(&mut stdout).expect("harness keeps talking until its result");
        match msg.method.as_deref() {
            Some(method::UNIT_EVENT) => {
                if matches!(
                    msg.params_as::<UnitEvent>().unwrap(),
                    UnitEvent::Metric { .. }
                ) {
                    saw_metric = true;
                }
            }
            Some(method::UNIT_RESULT) => break msg.params_as::<UnitResult>().unwrap(),
            other => panic!("unexpected message: {other:?}"),
        }
    };
    assert!(
        saw_metric,
        "metering: usd was declared, so a metric must arrive"
    );
    assert_eq!(result.outcome, Outcome::PrOpen);
    assert_eq!(
        result.evidence.expect("pr_open carries evidence").branch,
        "agent/u1"
    );

    drop(stdin);
    assert!(matches!(read_message(&mut stdout), Err(ReadError::Eof)));
    assert!(child.wait().unwrap().success());
}

#[test]
fn fake_refuses_a_foreign_major_version() {
    let mut child = spawn_fake();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    let init = RpcMessage::request(
        1,
        method::INITIALIZE,
        &InitializeParams {
            protocol_version: "99.0".into(),
        },
    );
    write_message(&mut stdin, &init).unwrap();
    let reply = read_message(&mut stdout).unwrap();
    match reply.kind() {
        MessageKind::ErrorResponse { id, error } => {
            assert_eq!(id, 1);
            assert_eq!(error.code, error_code::PROTOCOL_VERSION_UNSUPPORTED);
        }
        other => panic!("expected an error response, got {other:?}"),
    }
    assert!(child.wait().unwrap().success());
}
