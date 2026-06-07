//! Pure helpers for rate-limit resilience: classify an exec outcome, compute
//! backoff delays, and decide the wall-clock cap with rate-limit time exempt.
//! No async, no I/O — fully unit-tested.

use crate::runner::ExecOutput;

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
