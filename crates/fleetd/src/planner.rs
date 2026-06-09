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

/// Real planner: a read-only Claude call that emits a JSON lane array. Cost is
/// parsed from the CLI `result` record via `crate::claude_meter`.
pub struct ClaudePlanner;
impl ClaudePlanner { pub fn new() -> Self { Self } }
impl Default for ClaudePlanner { fn default() -> Self { Self::new() } }

#[async_trait]
impl Planner for ClaudePlanner {
    async fn plan(&self, doc: &str, lane_cap: usize) -> Result<PlanOutcome, PlanError> {
        let prompt = format!(
            "Split the following spec into at most {lane_cap} INDEPENDENT lanes that can be \
             built in parallel without colliding. Reply ONLY with a JSON array of objects \
             {{\"title\":..,\"task\":..,\"rationale\":..}}. Spec:\n\n{doc}"
        );
        let out = tokio::process::Command::new("claude")
            .args(["-p", &prompt, "--output-format", "stream-json", "--max-budget-usd", "1.0"])
            .output().await.map_err(|e| PlanError::Failed(e.to_string()))?;
        let stdout = String::from_utf8_lossy(&out.stdout);
        // Reuse the existing meter: parse_usage(&[String]) -> Option<Usage>; Usage.cost_usd.
        let lines: Vec<String> = stdout.lines().map(|l| l.to_string()).collect();
        let cost = crate::claude_meter::parse_usage(&lines).map(|u| u.cost_usd).unwrap_or(0.0);
        let json_slice = extract_json_array(&stdout).ok_or_else(|| PlanError::Failed("no JSON array".into()))?;
        #[derive(serde::Deserialize)]
        struct RawLane { title: String, task: String, #[serde(default)] rationale: String }
        let raw: Vec<RawLane> = serde_json::from_str(json_slice)
            .map_err(|e| PlanError::Failed(format!("bad JSON: {e}")))?;
        let lanes = raw.into_iter().take(lane_cap)
            .map(|r| Lane { title: r.title, task: r.task, rationale: r.rationale }).collect();
        Ok(PlanOutcome { lanes, cost_usd: cost })
    }
}

/// Find the first top-level JSON array in mixed CLI output.
fn extract_json_array(s: &str) -> Option<&str> {
    let start = s.find('[')?;
    let end = s.rfind(']')?;
    if end > start { Some(&s[start..=end]) } else { None }
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

    #[test]
    fn extract_json_array_finds_the_array() {
        assert_eq!(extract_json_array("noise [\"a\"] tail"), Some("[\"a\"]"));
        assert_eq!(extract_json_array("no array here"), None);
    }
}
