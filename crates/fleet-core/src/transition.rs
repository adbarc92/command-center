//! The pure unit state machine: `(phase, tier, trigger) -> Option<phase>`.
//!
//! Returning `None` means the trigger is not valid in that phase — the daemon
//! treats that as a programming error / ignored event, never a silent state jump.

use crate::{Phase, Tier};
use serde::{Deserialize, Serialize};

/// Everything that can drive a unit between phases. Forward-progress triggers
/// are daemon observations; interrupt/re-entry triggers come from caps, the
/// runner, or human commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "trigger", rename_all = "snake_case")]
pub enum Trigger {
    // --- forward progress ---
    Start,
    Provisioned,
    OracleFrozen,
    OracleApproved,
    OracleRejected,
    BuildFinished,
    ChecksPassed,
    ChecksFailed,
    /// Green checks but the branch diff is empty: a legitimate no-op.
    EmptyDiff,
    /// A review round finished; `gate_met` is the evidence-based gate verdict
    /// (green + no unresolved blockers + non-increasing + round floor).
    ReviewFinished { gate_met: bool },
    /// Trial merge into a fresh base succeeded.
    MergeClean,
    /// Trial merge hit conflicts.
    MergeConflict,
    /// GitHub reported the open PR mergeable.
    PrMergeable,
    /// GitHub reported the open PR's base moved (`dirty`).
    PrDirty,
    // --- interrupts (guarded by phase) ---
    CapBreach,
    Stall,
    OracleTampering,
    FatalError,
    Halt,
    // --- human re-entry ---
    Resume,
    Abandon,
    /// Human ships the PR from `NeedsHuman` (the T3 final gate).
    Ship,
}

/// Compute the next phase, or `None` if the trigger is invalid here.
pub fn transition(phase: Phase, tier: Tier, trigger: Trigger) -> Option<Phase> {
    use Phase::*;
    use Trigger::*;

    // Universal interrupts first (they win from any non-terminal phase).
    match trigger {
        FatalError if !phase.is_terminal() => return Some(Failed),
        Halt if !phase.is_terminal() => return Some(Halted),
        CapBreach | Stall if phase.is_agent_active() => return Some(NeedsHuman),
        OracleTampering if phase.is_agent_active() => return Some(NeedsHuman),
        _ => {}
    }

    // Phase-specific forward / re-entry transitions.
    Some(match (phase, trigger) {
        (Queued, Start) => Provisioning,
        (Provisioning, Provisioned) => Spec,

        (Spec, OracleFrozen) => {
            if tier.requires_oracle_approval() {
                AwaitingOracleApproval
            } else {
                Building
            }
        }
        (AwaitingOracleApproval, OracleApproved) => Building,
        (AwaitingOracleApproval, OracleRejected) => Spec,

        (Building, BuildFinished) => Checking,
        (Checking, ChecksPassed) => Reviewing,
        (Checking, ChecksFailed) => Building,
        (Checking, EmptyDiff) => NoChange,

        (Reviewing, ReviewFinished { gate_met: true }) => MergeCheck,
        (Reviewing, ReviewFinished { gate_met: false }) => Building,

        (MergeCheck, MergeClean) => {
            if tier.auto_opens_pr() {
                PrOpen
            } else {
                NeedsHuman // T3: human ships
            }
        }
        (MergeCheck, MergeConflict) => NeedsHuman,

        (PrOpen, PrMergeable) => Done,
        (PrOpen, PrDirty) => NeedsHuman,

        // Human re-entry. Resume re-provisions (the paused container was torn
        // down; provision reuses the persisted volume) before continuing.
        (NeedsHuman, Resume) => Provisioning,
        (NeedsHuman, Abandon) => Failed,
        (NeedsHuman, Ship) => PrOpen,
        (Halted, Resume) => Provisioning,
        (Halted, Abandon) => Failed,

        // Anything else is invalid in this phase.
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use Phase::*;

    /// Drive a sequence of triggers, asserting the resulting phase at each step.
    fn run(start: Phase, tier: Tier, steps: &[(Trigger, Phase)]) {
        let mut phase = start;
        for (i, (trigger, expected)) in steps.iter().enumerate() {
            phase = transition(phase, tier, *trigger)
                .unwrap_or_else(|| panic!("step {i}: {trigger:?} invalid in {phase:?}"));
            assert_eq!(phase, *expected, "step {i}: after {trigger:?}");
        }
    }

    #[test]
    fn t1_happy_path_autonomous_oracle_to_done() {
        run(
            Queued,
            Tier::T1,
            &[
                (Trigger::Start, Provisioning),
                (Trigger::Provisioned, Spec),
                (Trigger::OracleFrozen, Building), // T1 auto-freezes, no approval
                (Trigger::BuildFinished, Checking),
                (Trigger::ChecksPassed, Reviewing),
                (Trigger::ReviewFinished { gate_met: true }, MergeCheck),
                (Trigger::MergeClean, PrOpen), // T1 auto-opens PR
                (Trigger::PrMergeable, Done),
            ],
        );
    }

    #[test]
    fn t2_requires_oracle_approval_then_auto_pr() {
        run(
            Spec,
            Tier::T2,
            &[
                (Trigger::OracleFrozen, AwaitingOracleApproval),
                (Trigger::OracleApproved, Building),
                (Trigger::BuildFinished, Checking),
                (Trigger::ChecksPassed, Reviewing),
                (Trigger::ReviewFinished { gate_met: true }, MergeCheck),
                (Trigger::MergeClean, PrOpen), // T2 auto-opens PR
                (Trigger::PrMergeable, Done),
            ],
        );
    }

    #[test]
    fn t3_human_approves_oracle_and_ships_pr() {
        run(
            Spec,
            Tier::T3,
            &[
                (Trigger::OracleFrozen, AwaitingOracleApproval),
                (Trigger::OracleApproved, Building),
                (Trigger::BuildFinished, Checking),
                (Trigger::ChecksPassed, Reviewing),
                (Trigger::ReviewFinished { gate_met: true }, MergeCheck),
                (Trigger::MergeClean, NeedsHuman), // T3 never auto-opens PR
                (Trigger::Ship, PrOpen),           // human ships
                (Trigger::PrMergeable, Done),
            ],
        );
    }

    #[test]
    fn oracle_rejection_returns_to_spec() {
        assert_eq!(
            transition(AwaitingOracleApproval, Tier::T2, Trigger::OracleRejected),
            Some(Spec)
        );
    }

    #[test]
    fn checks_fail_loops_back_to_building() {
        assert_eq!(transition(Checking, Tier::T1, Trigger::ChecksFailed), Some(Building));
    }

    #[test]
    fn review_not_met_loops_back_to_building() {
        assert_eq!(
            transition(Reviewing, Tier::T1, Trigger::ReviewFinished { gate_met: false }),
            Some(Building)
        );
    }

    #[test]
    fn empty_diff_is_no_change_not_failure() {
        assert_eq!(transition(Checking, Tier::T1, Trigger::EmptyDiff), Some(NoChange));
    }

    #[test]
    fn merge_conflict_needs_human() {
        assert_eq!(transition(MergeCheck, Tier::T1, Trigger::MergeConflict), Some(NeedsHuman));
    }

    #[test]
    fn pr_dirty_after_open_needs_human() {
        assert_eq!(transition(PrOpen, Tier::T1, Trigger::PrDirty), Some(NeedsHuman));
    }

    #[test]
    fn cap_breach_only_interrupts_agent_active_phases() {
        // Active phase: interrupted.
        assert_eq!(transition(Building, Tier::T1, Trigger::CapBreach), Some(NeedsHuman));
        // Daemon-only phase: cap breach is not meaningful → invalid.
        assert_eq!(transition(MergeCheck, Tier::T1, Trigger::CapBreach), None);
    }

    #[test]
    fn stall_interrupts_oracle_phase() {
        assert_eq!(transition(Spec, Tier::T1, Trigger::Stall), Some(NeedsHuman));
    }

    #[test]
    fn fatal_error_from_any_active_phase_fails() {
        for p in [Provisioning, Spec, Building, Checking, Reviewing, MergeCheck, PrOpen] {
            assert_eq!(transition(p, Tier::T1, Trigger::FatalError), Some(Failed));
        }
    }

    #[test]
    fn halt_and_resume_round_trip() {
        assert_eq!(transition(Building, Tier::T1, Trigger::Halt), Some(Halted));
        // Resume re-provisions (reuses the volume) before re-entering the loop.
        assert_eq!(transition(Halted, Tier::T1, Trigger::Resume), Some(Provisioning));
    }

    #[test]
    fn resume_goes_to_provisioning_not_building() {
        assert_eq!(transition(NeedsHuman, Tier::T1, Trigger::Resume), Some(Provisioning));
        assert_eq!(transition(Halted, Tier::T1, Trigger::Resume), Some(Provisioning));
    }

    #[test]
    fn abandon_from_needs_human_fails() {
        assert_eq!(transition(NeedsHuman, Tier::T1, Trigger::Abandon), Some(Failed));
    }

    #[test]
    fn terminal_phases_reject_everything() {
        for p in [Done, NoChange, Failed] {
            for t in [Trigger::Resume, Trigger::Start, Trigger::Halt, Trigger::FatalError] {
                assert_eq!(transition(p, Tier::T1, t), None, "{p:?} should reject {t:?}");
            }
        }
    }

    #[test]
    fn invalid_trigger_in_phase_is_none() {
        // You cannot pass checks straight from Building.
        assert_eq!(transition(Building, Tier::T1, Trigger::ChecksPassed), None);
        // You cannot open a PR straight from Reviewing.
        assert_eq!(transition(Reviewing, Tier::T1, Trigger::MergeClean), None);
    }
}
