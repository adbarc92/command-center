//! The pure, sync swarm admission core: lane admission against the dual
//! guardrail, and branch-slug sanitization. No async, no I/O — exhaustively
//! unit-testable, mirroring `fleet_core::gate`.

/// One independent sub-task the planner carved out of the doc.
#[derive(Clone, Debug, PartialEq)]
pub struct Lane {
    pub title: String,
    pub task: String,
    pub rationale: String,
}

/// The dual guardrail: a hard lane count and a worst-case dollar envelope.
#[derive(Clone, Copy, Debug)]
pub struct AdmissionConfig {
    pub lane_cap: usize,
    pub usd_budget: f64,
    pub per_lane_cap: f64,
    pub planner_cost: f64,
}

/// Per-lane admission verdict. `DropOverGlobalCap` is set later by the fan-out
/// loop (a runtime re-check), never by `admit_lanes`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LaneDecision { Admit, DropOverLaneCap, DropOverBudget }

/// Walk lanes in order; admit while BOTH the count cap and the
/// (usd_budget − planner_cost) envelope hold. Conservative: each admitted lane
/// is assumed to spend its full `per_lane_cap`.
pub fn admit_lanes(lanes: &[Lane], cfg: &AdmissionConfig) -> Vec<(usize, LaneDecision)> {
    let envelope = (cfg.usd_budget - cfg.planner_cost).max(0.0);
    let mut admitted = 0usize;
    lanes.iter().enumerate().map(|(i, _)| {
        let decision = if admitted >= cfg.lane_cap {
            LaneDecision::DropOverLaneCap
        } else if (admitted as f64 + 1.0) * cfg.per_lane_cap > envelope {
            LaneDecision::DropOverBudget
        } else {
            admitted += 1;
            LaneDecision::Admit
        };
        (i, decision)
    }).collect()
}

/// Sanitize a planner-chosen lane title into a git-ref-safe, length-bounded
/// slug. Uniqueness comes from the `{idx}-` prefix the caller adds, so this is
/// purely cosmetic and never the uniqueness key.
pub fn slug(title: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for c in title.chars() {
        let lc = c.to_ascii_lowercase();
        if lc.is_ascii_alphanumeric() {
            out.push(lc);
            prev_dash = false;
        } else if !prev_dash && !out.is_empty() {
            out.push('-');
            prev_dash = true;
        }
    }
    let trimmed: String = out.trim_matches('-').chars().take(32).collect();
    let trimmed = trimmed.trim_matches('-').to_string();
    if trimmed.is_empty() { "lane".into() } else { trimmed }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_sanitizes_charset_length_and_empty() {
        assert_eq!(slug("Add Auth!!"), "add-auth");
        assert_eq!(slug("  spaced  out  "), "spaced-out");
        assert_eq!(slug("🚀🚀🚀"), "lane");          // non-ascii → fallback
        assert_eq!(slug(""), "lane");
        assert_eq!(slug(&"x".repeat(100)).len(), 32); // truncated
    }

    fn lanes(n: usize) -> Vec<Lane> {
        (0..n).map(|i| Lane { title: format!("l{i}"), task: "t".into(), rationale: "r".into() }).collect()
    }
    fn cfg(lane_cap: usize, usd_budget: f64, per_lane_cap: f64, planner_cost: f64) -> AdmissionConfig {
        AdmissionConfig { lane_cap, usd_budget, per_lane_cap, planner_cost }
    }
    fn admits(d: &[(usize, LaneDecision)]) -> usize {
        d.iter().filter(|(_, x)| matches!(x, LaneDecision::Admit)).count()
    }

    #[test]
    fn lane_cap_binds_first() {
        // budget allows 10, cap allows 3.
        let d = admit_lanes(&lanes(5), &cfg(3, 100.0, 5.0, 0.0));
        assert_eq!(admits(&d), 3);
        assert!(matches!(d[3].1, LaneDecision::DropOverLaneCap));
    }

    #[test]
    fn budget_binds_first() {
        // cap allows 8, budget allows floor((15-0)/5)=3.
        let d = admit_lanes(&lanes(8), &cfg(8, 15.0, 5.0, 0.0));
        assert_eq!(admits(&d), 3);
        assert!(matches!(d[3].1, LaneDecision::DropOverBudget));
    }

    #[test]
    fn planner_cost_reserved_first() {
        // budget 15, planner spent 6 → floor((15-6)/5)=1.
        let d = admit_lanes(&lanes(4), &cfg(8, 15.0, 5.0, 6.0));
        assert_eq!(admits(&d), 1);
    }

    #[test]
    fn planner_over_budget_admits_zero() {
        let d = admit_lanes(&lanes(4), &cfg(8, 4.0, 5.0, 5.0));
        assert_eq!(admits(&d), 0);
        assert!(d.iter().all(|(_, x)| matches!(x, LaneDecision::DropOverBudget)));
    }

    #[test]
    fn empty_input_is_empty_output() {
        assert!(admit_lanes(&[], &cfg(8, 15.0, 5.0, 0.0)).is_empty());
    }
}
