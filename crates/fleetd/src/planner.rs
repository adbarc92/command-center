//! The decomposition seam. `FakePlanner` (tests/demo) returns scripted lanes;
//! `ClaudePlanner` (real, Task 16) makes a read-only Claude call. The planner
//! writes no code and opens no PR.

use crate::swarm::Lane;
use async_trait::async_trait;

#[derive(Debug, thiserror::Error)]
pub enum PlanError {
    #[error("planner failure: {0}")]
    Failed(String),
}

/// A decomposition result plus what it cost to produce.
#[derive(Clone, Debug)]
pub struct PlanOutcome {
    pub lanes: Vec<Lane>,
    pub cost_usd: f64,
}

#[async_trait]
pub trait Planner: Send + Sync {
    /// Decompose `doc` into at most `lane_cap` independent lanes.
    async fn plan(&self, doc: &str, lane_cap: usize) -> Result<PlanOutcome, PlanError>;
}

/// Scripted planner for tests/demo: returns a fixed outcome (clamped to lane_cap),
/// or a fixed error.
pub struct FakePlanner {
    outcome: Result<PlanOutcome, String>,
}
impl FakePlanner {
    pub fn ok(lanes: Vec<Lane>, cost_usd: f64) -> Self {
        Self { outcome: Ok(PlanOutcome { lanes, cost_usd }) }
    }
    pub fn err(msg: &str) -> Self { Self { outcome: Err(msg.into()) } }
}
#[async_trait]
impl Planner for FakePlanner {
    async fn plan(&self, _doc: &str, lane_cap: usize) -> Result<PlanOutcome, PlanError> {
        match &self.outcome {
            Ok(o) => Ok(PlanOutcome {
                lanes: o.lanes.iter().take(lane_cap).cloned().collect(),
                cost_usd: o.cost_usd,
            }),
            Err(m) => Err(PlanError::Failed(m.clone())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn fake_planner_clamps_to_lane_cap_and_can_error() {
        let lanes = vec![
            Lane { title: "a".into(), task: "ta".into(), rationale: "r".into() },
            Lane { title: "b".into(), task: "tb".into(), rationale: "r".into() },
        ];
        let p = FakePlanner::ok(lanes, 0.5);
        let out = p.plan("doc", 1).await.unwrap();
        assert_eq!(out.lanes.len(), 1);
        assert_eq!(out.cost_usd, 0.5);

        let e = FakePlanner::err("boom").plan("doc", 5).await;
        assert!(matches!(e, Err(PlanError::Failed(_))));
    }
}
