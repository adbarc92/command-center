//! `harness-conformance [--wall-clock-secs N] [--grace-secs N] -- <harness command> [args...]`
//!
//! Runs every conformance case against a harness and exits 0 only if none failed.

use harness_conformance::{run_all, CaseOutcome, KitConfig};
use std::process::ExitCode;
use std::time::Duration;

const USAGE: &str =
    "usage: harness-conformance [--wall-clock-secs N] [--grace-secs N] -- <harness command> [args...]";

fn parse(args: Vec<String>) -> Result<KitConfig, String> {
    let split = args
        .iter()
        .position(|a| a == "--")
        .ok_or_else(|| USAGE.to_string())?;
    let (flags, rest) = args.split_at(split);
    let command = rest[1..].to_vec();
    if command.is_empty() {
        return Err(USAGE.to_string());
    }
    let mut cfg = KitConfig::new(command);
    let mut it = flags.iter();
    while let Some(flag) = it.next() {
        let value = it
            .next()
            .ok_or_else(|| format!("{flag} needs a value\n{USAGE}"))?;
        let secs: u64 = value
            .parse()
            .map_err(|_| format!("{flag}: `{value}` is not a whole number of seconds"))?;
        match flag.as_str() {
            "--wall-clock-secs" => cfg.wall_clock = Duration::from_secs(secs),
            "--grace-secs" => cfg.grace = Duration::from_secs(secs),
            other => return Err(format!("unknown flag `{other}`\n{USAGE}")),
        }
    }
    if cfg.grace.is_zero() {
        return Err(format!(
            "--grace-secs must be at least 1: a zero grace fails every case\n{USAGE}"
        ));
    }
    Ok(cfg)
}

fn main() -> ExitCode {
    let cfg = match parse(std::env::args().skip(1).collect()) {
        Ok(cfg) => cfg,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::from(2);
        }
    };

    let reports = run_all(&cfg);
    let (mut passed, mut skipped, mut failed) = (0, 0, 0);
    for report in &reports {
        match &report.outcome {
            CaseOutcome::Pass => {
                passed += 1;
                println!("PASS  {}", report.name);
            }
            CaseOutcome::Skipped(why) => {
                skipped += 1;
                println!("SKIP  {}  ({why})", report.name);
            }
            CaseOutcome::Fail(violation) => {
                failed += 1;
                println!("FAIL  {}  {violation:?}", report.name);
            }
        }
    }
    println!("{passed} passed, {skipped} skipped, {failed} failed");

    if failed == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
