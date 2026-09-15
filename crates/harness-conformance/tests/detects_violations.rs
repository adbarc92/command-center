//! Each misbehaving `harness-fake` mode must be caught by exactly the case that owns that rule.

use harness_conformance::{run_all, CaseOutcome, CaseReport, KitConfig, Violation};
use std::time::Duration;

fn fake_config(mode: &str) -> KitConfig {
    let mut cfg = KitConfig::new(vec![env!("CARGO_BIN_EXE_harness-fake").to_string()]);
    cfg.env = vec![
        ("HARNESS_FAKE_MODE".into(), mode.into()),
        ("HARNESS_FAKE_STEP_MS".into(), "20".into()),
    ];
    // Short, because `hang` makes every driving case wait out the whole wall clock.
    cfg.wall_clock = Duration::from_secs(2);
    cfg.grace = Duration::from_secs(1);
    cfg
}

fn outcome_of<'a>(reports: &'a [CaseReport], case: &str) -> &'a CaseOutcome {
    &reports
        .iter()
        .find(|r| r.name == case)
        .expect("case exists in run_all")
        .outcome
}

fn assert_detects(mode: &str, case: &str, want: Violation) {
    let reports = run_all(&fake_config(mode));
    assert_eq!(
        *outcome_of(&reports, case),
        CaseOutcome::Fail(want),
        "mode `{mode}`, case `{case}`; all reports: {reports:#?}"
    );
}

#[test]
fn crash_is_exited_without_result() {
    assert_detects("crash", "happy_path_t1", Violation::ExitedWithoutResult);
}

#[test]
fn malformed_line_is_reported_verbatim() {
    assert_detects(
        "malformed",
        "happy_path_t1",
        Violation::Malformed {
            line: "this is not json".into(),
        },
    );
}

#[test]
fn a_message_after_the_result_is_caught() {
    assert_detects(
        "event_after_result",
        "happy_path_t1",
        Violation::MessageAfterResult {
            method: "unit/event".into(),
        },
    );
}

#[test]
fn declared_metering_that_never_reports_is_caught() {
    assert_detects(
        "silent_metering",
        "happy_path_t1",
        Violation::MeteringDeclaredButSilent,
    );
}

#[test]
fn a_hung_harness_is_killed_at_the_wall_clock() {
    assert_detects("hang", "happy_path_t1", Violation::WallClockExceeded);
}

#[test]
fn an_unknown_method_is_caught() {
    assert_detects(
        "unknown_method",
        "happy_path_t1",
        Violation::UnknownMethod {
            method: "unit/whatever".into(),
        },
    );
}

#[test]
fn accepting_a_foreign_major_is_caught() {
    assert_detects(
        "accept_any_version",
        "version_mismatch_refused",
        Violation::VersionMismatchAccepted,
    );
}

#[test]
fn skipping_the_oracle_gate_is_caught() {
    assert_detects("skip_gate", "gate_approved_t2", Violation::GateNotRequested);
}

#[test]
fn ignoring_a_rejected_gate_is_caught() {
    assert_detects(
        "ignore_gate_rejection",
        "gate_rejected_t2",
        Violation::GateRejectionIgnored,
    );
}

#[test]
fn ignoring_halt_is_caught() {
    assert_detects(
        "ignore_halt",
        "halt",
        Violation::InterruptNotHonored {
            method: "unit/halt".into(),
        },
    );
}
