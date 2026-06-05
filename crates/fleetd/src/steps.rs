//! Builders for the per-phase container commands. Each returns an argv vector
//! (no shell) run by the `Runner` in `WORKDIR`. The oracle/build/review steps
//! invoke `claude`; the check step runs the project's test command.
//!
//! NOTE: the in-container `claude` is a stock install without the host's
//! `code-review` skill, so the review step approximates it with a review prompt
//! that emits a `BLOCKERS=N` line. Installing the real skill in the image is a
//! later refinement.

use crate::runner::UnitSpec;

/// All agent/check steps run in the cloned repo inside the volume.
pub const WORKDIR: &str = "/work/repo";

fn claude_argv(prompt: String, remaining_usd: f64) -> Vec<String> {
    vec![
        "claude".into(),
        "-p".into(),
        prompt,
        "--dangerously-skip-permissions".into(),
        "--output-format".into(),
        "stream-json".into(),
        "--verbose".into(),
        // Daemon-independent dollar backstop (Spike 2).
        "--max-budget-usd".into(),
        format!("{remaining_usd:.4}"),
    ]
}

pub fn oracle(spec: &UnitSpec, remaining_usd: f64) -> Vec<String> {
    let prompt = format!(
        "You are the test oracle. Write a minimal but meaningful automated test \
         (a `*.test.js` file runnable by `node --test`) that objectively defines \
         when this task is done. Do NOT implement the solution itself. Task: {}",
        spec.task
    );
    claude_argv(prompt, remaining_usd)
}

pub fn build(spec: &UnitSpec, findings: &str, remaining_usd: f64) -> Vec<String> {
    let prompt = format!(
        "Implement the task so the test suite passes. You may ADD files but must \
         NOT modify or delete existing test files. Task: {}. Outstanding review \
         findings to address: {}",
        spec.task, findings
    );
    claude_argv(prompt, remaining_usd)
}

pub fn review(remaining_usd: f64) -> Vec<String> {
    let prompt = "Review the current working-tree diff for correctness and quality. \
         Finish your reply with a single line `BLOCKERS=N` where N is the count of \
         must-fix issues (0 if none)."
        .to_string();
    claude_argv(prompt, remaining_usd)
}

/// The project's test command, split into argv (no shell). e.g. "npm test".
pub fn check(spec: &UnitSpec) -> Vec<String> {
    spec.test_cmd.split_whitespace().map(String::from).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use fleet_core::{GateConfig, Tier};

    fn spec() -> UnitSpec {
        UnitSpec {
            unit_id: "u".into(),
            tier: Tier::T1,
            task: "add sum(a,b)".into(),
            usd_cap: 5.0,
            gate: GateConfig::default(),
            repo_url: "https://github.com/x/y".into(),
            repo_slug: "x/y".into(),
            base_branch: "main".into(),
            branch: "agent/u".into(),
            test_cmd: "npm test".into(),
        }
    }

    #[test]
    fn claude_steps_carry_budget_and_skip_perms() {
        let argv = oracle(&spec(), 4.25);
        assert_eq!(argv[0], "claude");
        assert!(argv.contains(&"--dangerously-skip-permissions".to_string()));
        assert!(argv.contains(&"stream-json".to_string()));
        let i = argv.iter().position(|a| a == "--max-budget-usd").unwrap();
        assert_eq!(argv[i + 1], "4.2500");
    }

    #[test]
    fn check_splits_test_command() {
        assert_eq!(check(&spec()), vec!["npm".to_string(), "test".to_string()]);
    }
}
