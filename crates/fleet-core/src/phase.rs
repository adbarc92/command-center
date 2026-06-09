//! The lifecycle phases of a unit. See the SP1 spec, Section 2.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Queued,
    Provisioning,
    /// Oracle agent generates the test set.
    Spec,
    /// T2/T3 only: waiting for a human to approve the frozen test set.
    AwaitingOracleApproval,
    Building,
    Checking,
    Reviewing,
    /// Daemon trial-merges a fresh base and (after PR) polls GitHub mergeability.
    MergeCheck,
    PrOpen,
    // --- terminal ---
    Done,
    /// Green checks with an empty diff: a correct no-op, not a failure.
    NoChange,
    Failed,
    // --- waiting-on-human (non-terminal) ---
    NeedsHuman,
    Halted,
}

impl Phase {
    /// Terminal phases have no outgoing transitions.
    pub fn is_terminal(self) -> bool {
        matches!(self, Phase::Done | Phase::NoChange | Phase::Failed)
    }

    /// Phases in which the agent/container is actively running, so cost/stall
    /// caps and oracle-tampering apply.
    pub fn is_agent_active(self) -> bool {
        matches!(self, Phase::Spec | Phase::Building | Phase::Checking | Phase::Reviewing)
    }

    /// Non-terminal phases can always be halted or fail fatally.
    pub fn is_interruptible(self) -> bool {
        !self.is_terminal()
    }
}

/// The snake_case phase strings that are terminal. Single source of truth shared
/// by the SQL rollup/admission queries and `reconcile`. MUST stay in sync with
/// `Phase::is_terminal` (the test `terminal_strs_match_is_terminal` enforces this).
pub const TERMINAL_PHASE_STRS: &[&str] = &["done", "no_change", "failed"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_strs_match_is_terminal() {
        // The string list and the enum predicate must never drift apart.
        for p in [
            Phase::Queued,
            Phase::Provisioning,
            Phase::Spec,
            Phase::AwaitingOracleApproval,
            Phase::Building,
            Phase::Checking,
            Phase::Reviewing,
            Phase::MergeCheck,
            Phase::PrOpen,
            Phase::Done,
            Phase::NoChange,
            Phase::Failed,
            Phase::NeedsHuman,
            Phase::Halted,
        ] {
            let s = serde_json::to_value(p).unwrap().as_str().unwrap().to_string();
            assert_eq!(TERMINAL_PHASE_STRS.contains(&s.as_str()), p.is_terminal(),
                "{s} membership must equal is_terminal()");
        }
    }
}
