//! The evidence-based review gate. The only *objective* signal is `checks_green`;
//! the review conditions add confidence, not truth. See SP1 spec, Section 2.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct GateConfig {
    /// Minimum number of review rounds before auto-advance is allowed. This is a
    /// floor against "one shallow pass = done", not the criterion itself.
    pub min_review_rounds: u32,
}

impl Default for GateConfig {
    fn default() -> Self {
        Self { min_review_rounds: 3 }
    }
}

/// A snapshot taken at the end of a review round, used to evaluate the gate.
#[derive(Debug, Clone, Copy)]
pub struct ReviewSnapshot {
    /// 1-based index of the review round just completed.
    pub round: u32,
    /// Unresolved blocker findings after this round.
    pub unresolved_blockers: u32,
    /// Unresolved blockers after the *previous* round, if any.
    pub prev_unresolved_blockers: Option<u32>,
    /// Whether the objective checks (tests) are currently green.
    pub checks_green: bool,
}

/// True iff the unit may auto-advance from `Reviewing` to `MergeCheck`.
///
/// Requires ALL of: green checks (objective), zero unresolved blockers, the
/// round floor reached, and a non-increasing blocker count vs. the prior round
/// (no oscillation in this round).
pub fn gate_met(cfg: GateConfig, s: ReviewSnapshot) -> bool {
    let non_increasing = match s.prev_unresolved_blockers {
        Some(prev) => s.unresolved_blockers <= prev,
        None => true,
    };
    s.checks_green
        && s.unresolved_blockers == 0
        && s.round >= cfg.min_review_rounds
        && non_increasing
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(round: u32, blockers: u32, prev: Option<u32>, green: bool) -> ReviewSnapshot {
        ReviewSnapshot {
            round,
            unresolved_blockers: blockers,
            prev_unresolved_blockers: prev,
            checks_green: green,
        }
    }

    #[test]
    fn met_when_green_zero_blockers_and_floor_reached() {
        assert!(gate_met(GateConfig::default(), snap(3, 0, Some(1), true)));
    }

    #[test]
    fn not_met_before_round_floor() {
        assert!(!gate_met(GateConfig::default(), snap(2, 0, Some(0), true)));
    }

    #[test]
    fn not_met_when_checks_red_even_if_no_blockers() {
        // The objective signal dominates: red tests can never auto-advance.
        assert!(!gate_met(GateConfig::default(), snap(5, 0, Some(0), false)));
    }

    #[test]
    fn not_met_with_unresolved_blockers() {
        assert!(!gate_met(GateConfig::default(), snap(5, 2, Some(3), true)));
    }

    #[test]
    fn first_round_has_no_prior_to_compare() {
        // round floor of 1 to isolate the prev=None branch
        let cfg = GateConfig { min_review_rounds: 1 };
        assert!(gate_met(cfg, snap(1, 0, None, true)));
    }

    #[test]
    fn respects_custom_floor() {
        let cfg = GateConfig { min_review_rounds: 1 };
        assert!(gate_met(cfg, snap(1, 0, Some(0), true)));
    }
}
