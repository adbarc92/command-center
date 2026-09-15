use std::process::Command;

fn kit() -> Command {
    Command::new(env!("CARGO_BIN_EXE_harness-conformance"))
}

fn run_against_fake(mode: &str) -> (Option<i32>, String) {
    let out = kit()
        .env("HARNESS_FAKE_MODE", mode)
        .env("HARNESS_FAKE_STEP_MS", "20")
        .args([
            "--wall-clock-secs",
            "10",
            "--grace-secs",
            "2",
            "--",
            env!("CARGO_BIN_EXE_harness-fake"),
        ])
        .output()
        .expect("harness-conformance runs");
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
    )
}

#[test]
fn exits_zero_against_the_conformant_fake() {
    let (code, stdout) = run_against_fake("conformant");
    assert_eq!(code, Some(0), "{stdout}");
    assert!(stdout.contains("6 passed, 0 skipped, 0 failed"), "{stdout}");
}

#[test]
fn exits_one_and_names_the_case_when_the_harness_breaks_the_protocol() {
    let (code, stdout) = run_against_fake("silent_metering");
    assert_eq!(code, Some(1), "{stdout}");
    assert!(
        stdout.contains("FAIL  happy_path_t1  MeteringDeclaredButSilent"),
        "{stdout}"
    );
}

#[test]
fn exits_two_on_a_usage_error() {
    let out = kit().output().expect("harness-conformance runs");
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("usage: harness-conformance"));
}
