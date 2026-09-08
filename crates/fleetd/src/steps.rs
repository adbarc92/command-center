//! Builders for the per-phase container commands. Each returns an argv vector
//! (no shell) run by the `Runner` in `WORKDIR`. The oracle/build/review steps
//! invoke `claude`; the check step runs the project's test command.
//!
//! NOTE: the in-container `claude` is a stock install without the host's
//! `code-review` skill, so the review step approximates it with a review prompt
//! that emits a `BLOCKERS=N` line. Installing the real skill in the image is
//! **weave item W5**, still unbuilt: writer and grader are the same model in the
//! same container, which is `WORKFLOW` B1 unmet at the engine level. No wording in
//! the review prompt can fix that; only running the grader elsewhere can.
//!
//! ## W2 - what ported, and what could not
//!
//! ADR 0001 retired `reqdrive` as a tool and kept its prompts as the thing worth
//! transplanting. What ports is the *discipline*: implement exactly one thing, never
//! touch the frozen tests, run the real checks, report in a shape a machine can
//! parse, and review against named categories rather than "look for problems".
//!
//! What does not port is everything welded to reqdrive's own artifacts - `prd.json`,
//! `progress.txt`, `.reqdrive/runs/`, per-story ids, and the `iteration-summary`
//! block. This harness has none of them: it owns state in event-sourced SQLite, and
//! it has no story concept at all.
//!
//! **So W2 as the plan worded it depends on W6 (per-story decomposition), which the
//! plan schedules later.** That dependency was never stated. Ported here is the
//! transferable half; the story-shaped half waits on a story model.

use crate::runner::UnitSpec;

/// All agent/check steps run in the cloned repo inside the volume.
pub const WORKDIR: &str = "/work/repo";

fn claude_argv(prompt: String, remaining_usd: f64, wall_secs: u64) -> Vec<String> {
    let mut v = Vec::new();
    // In-container wall-clock bound (daemon-independent): prefix `timeout <secs>`.
    if wall_secs > 0 {
        v.push("timeout".into());
        v.push(wall_secs.to_string());
    }
    v.extend([
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
    ]);
    v
}

pub fn oracle(spec: &UnitSpec, remaining_usd: f64) -> Vec<String> {
    // The oracle is written BEFORE any implementation, then content-hashed and
    // frozen (W3), so everything downstream is measured against it. A vague oracle
    // is not a weak test - it is a wrong definition of done that nothing later can
    // correct, because tampering with it halts the unit.
    let prompt = format!(
        "# Test oracle\n\
         \n\
         You define when this task is done. You do NOT implement it.\n\
         \n\
         ## Task\n\
         \n\
         {}\n\
         \n\
         ## What to produce\n\
         \n\
         A single `*.test.js` file runnable by `node --test`, containing a minimal but \
         meaningful set of automated tests.\n\
         \n\
         ## Rules\n\
         \n\
         1. Write ONLY the test file. Do not implement the solution, and do not stub \
            it so the tests pass vacuously.\n\
         2. The tests MUST fail against the tree as it stands. A test that already \
            passes defines nothing.\n\
         3. Assert on observable behaviour - return values, thrown errors, written \
            files. Never on internal names you are also about to invent.\n\
         4. Cover the ordinary case, at least one boundary, and at least one failure \
            the task implies. Prefer four sharp cases to twenty shallow ones.\n\
         5. Where the task is ambiguous, choose the reading a careful reviewer would, \
            encode that choice as a test, and say so in a comment at the top.\n\
         \n\
         This file is frozen and hashed when you finish. Later phases may add files \
         but may never edit it - a change is detected and halts the run for a human.",
        spec.task
    );
    claude_argv(prompt, remaining_usd, spec.wall_clock_secs)
}

pub fn build(spec: &UnitSpec, findings: &str, remaining_usd: f64) -> Vec<String> {
    // reqdrive's implementation discipline, minus its artifacts: one thing at a
    // time, real checks, never touch the frozen tests. The harness owns commits and
    // state, so - unlike reqdrive - the agent is told NOT to manage either.
    let prompt = format!(
        "# Implement\n\
         \n\
         ## Task\n\
         \n\
         {}\n\
         \n\
         ## Outstanding review findings\n\
         \n\
         {}\n\
         \n\
         ## Rules\n\
         \n\
         1. Make the existing test suite pass. It is the definition of done and it \
            was written before you; do not negotiate with it.\n\
         2. You may ADD files. You must NOT modify or delete any existing test file. \
            Doing so is detected by content hash and halts the run - it does not \
            merely fail.\n\
         3. Address every outstanding finding above before adding anything new.\n\
         4. Smallest change that earns a green suite. No speculative abstraction, no \
            unrelated refactor, no scope the task did not ask for - scope creep is a \
            review finding in its own right.\n\
         5. Run the project test command yourself before you finish, and fix what it \
            reports. Do not claim success you have not observed.\n\
         6. Do NOT commit, branch, or tag. The harness owns version control and \
            records state itself; leave your work in the tree.\n\
         \n\
         If the task cannot be done without editing a frozen test, stop and say so \
         plainly rather than working around it.",
        spec.task, findings
    );
    claude_argv(prompt, remaining_usd, spec.wall_clock_secs)
}

pub fn review(remaining_usd: f64, wall_secs: u64) -> Vec<String> {
    // reqdrive's four review criteria, which are the part worth keeping: it reviewed
    // against named categories rather than 'look for problems'. The BLOCKERS=N line
    // is this harness's parse contract (driver::parse_blockers) and must survive any
    // edit here.
    //
    // W5 caveat, unfixable from inside this prompt: the reviewer is the same model in
    // the same container as the writer. Asking it to be adversarial is not the same as
    // it being independent.
    let prompt = 
        "# Review\n\
         \n\
         Review the current working-tree diff against the base branch. You did not \
         write it; do not defend it.\n\
         \n\
         ## Look for, in this order\n\
         \n\
         1. Security - injection, auth bypass, secrets committed, unsafe operations.\n\
         2. Correctness - logic errors, unhandled edge cases, off-by-one, null and \
            error paths.\n\
         3. Scope - anything changed that the task did not ask for.\n\
         4. Quality - dead code, needless complexity, missing error handling.\n\
         \n\
         A passing test suite is not evidence of correctness: the tests were written \
         before the code and only cover what they cover.\n\
         \n\
         ## Output\n\
         \n\
         For each must-fix issue, one line: `SEVERITY file:line - what is wrong`, \
         where SEVERITY is CRITICAL or WARNING. Only issues you can point at in the \
         diff. Do not pad the list, and do not raise style preferences.\n\
         \n\
         Then finish your reply with exactly one line:\n\
         \n\
         BLOCKERS=N\n\
         \n\
         where N is the count of must-fix issues, 0 if none. This line is parsed by \
         machine. Emit it even when N is 0, and emit it last."
            .to_string();
    claude_argv(prompt, remaining_usd, wall_secs)
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
            wall_clock_secs: 0,
            gate: GateConfig::default(),
            repo_url: "https://github.com/x/y".into(),
            repo_slug: "x/y".into(),
            base_branch: "main".into(),
            branch: "agent/u".into(),
            test_cmd: "npm test".into(),
            oracle_frozen: false,
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

    // ── W2 prompt invariants ────────────────────────────────────────────────
    // These pin the DISCIPLINE, not the wording. A prompt is the least reviewable
    // thing in the engine - it has no type and no compiler - so the rules that make
    // it safe are asserted rather than trusted. Reword freely; drop a rule and this
    // fails.

    fn prompt_of(argv: &[String]) -> String {
        // The prompt is the argument right after `-p`.
        let i = argv.iter().position(|a| a == "-p").expect("a -p flag");
        argv[i + 1].clone()
    }

    #[test]
    fn the_oracle_is_told_not_to_implement_and_to_fail_first() {
        let p = prompt_of(&oracle(&spec(), 1.0));
        assert!(p.contains("do NOT implement") || p.contains("You do NOT implement"));
        assert!(p.contains("MUST fail"), "a test that already passes defines nothing");
        assert!(p.contains("add sum(a,b)"), "the task must reach the agent");
    }

    #[test]
    fn the_builder_may_not_edit_frozen_tests_or_touch_version_control() {
        let p = prompt_of(&build(&spec(), "none", 1.0));
        assert!(p.contains("must NOT modify or delete any existing test file"));
        // The harness owns commits. An agent that commits corrupts the diff the
        // review and merge steps read.
        assert!(p.contains("Do NOT commit"));
        assert!(p.contains("add sum(a,b)"), "the task must reach the agent");
    }

    #[test]
    fn the_builder_is_given_the_outstanding_findings() {
        // Review findings that never reach the next build round make the review loop
        // decorative - it would re-raise the same blockers forever.
        let p = prompt_of(&build(&spec(), "CRITICAL src/x.js:12 - unchecked null", 1.0));
        assert!(p.contains("CRITICAL src/x.js:12 - unchecked null"));
    }

    #[test]
    fn the_review_prompt_states_the_parse_contract_the_driver_relies_on() {
        // `driver::parse_blockers` scans for a line starting `BLOCKERS=`. If this
        // instruction is ever dropped, every review silently reads as 0 blockers
        // (see `parse_blockers_treats_an_absent_marker_as_clean`).
        let p = prompt_of(&review(1.0, 0));
        assert!(p.contains("BLOCKERS=N"));
        assert!(p.contains("Emit it even when N is 0"));
        for criterion in ["Security", "Correctness", "Scope", "Quality"] {
            assert!(p.contains(criterion), "missing review criterion {criterion}");
        }
    }

    #[test]
    fn check_splits_test_command() {
        assert_eq!(check(&spec()), vec!["npm".to_string(), "test".to_string()]);
    }

    #[test]
    fn timeout_prefix_when_wall_secs_set() {
        let mut s = spec();
        s.wall_clock_secs = 30;
        let argv = oracle(&s, 1.0);
        assert_eq!(argv[0], "timeout");
        assert_eq!(argv[1], "30");
        assert!(argv.contains(&"claude".to_string()));
    }
}
