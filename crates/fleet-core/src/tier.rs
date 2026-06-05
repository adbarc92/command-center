//! The autonomy ladder. A mission's `Tier` gates the two human checkpoints:
//! the oracle/test-set approval and the final PR.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    /// Low stakes: autonomous oracle (no human approval), auto-open PR.
    T1,
    /// Medium: human approves the frozen test set; auto-open PR.
    T2,
    /// High: human approves the test set AND ships the PR (never auto-PR).
    T3,
}

impl Tier {
    /// Whether this tier requires a human to approve the frozen test set
    /// before the build loop may start.
    pub fn requires_oracle_approval(self) -> bool {
        matches!(self, Tier::T2 | Tier::T3)
    }

    /// Whether this tier opens the PR automatically once the gate is met and
    /// the branch is mergeable. T3 routes to a human instead.
    pub fn auto_opens_pr(self) -> bool {
        matches!(self, Tier::T1 | Tier::T2)
    }
}
