//! Payload types for every params/result on the harness wire (spec §3).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    T1,
    T2,
    T3,
}

impl Tier {
    /// T2/T3 need a human-approved oracle before building (mirrors `fleet-core` `Tier`).
    pub fn requires_oracle(self) -> bool {
        matches!(self, Tier::T2 | Tier::T3)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Isolation {
    Container,
    Host,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Metering {
    Usd,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GateKind {
    Oracle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Delivery {
    Bundle,
    Push,
}

/// What a harness guarantees. `fleetd` decides eligibility from this (spec §3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Capabilities {
    pub isolation: Isolation,
    pub metering: Metering,
    pub gates: Vec<GateKind>,
    pub delivery: Delivery,
    pub resume: bool,
    pub halt: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct HarnessInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct InitializeParams {
    pub protocol_version: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct InitializeResult {
    pub protocol_version: String,
    pub harness: HarnessInfo,
    pub capabilities: Capabilities,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkItemKind {
    RoadmapItem,
    Issue,
}

/// The id that must travel end to end (SMOKE-LAP-01 item 14: nothing carried it).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct WorkItem {
    pub kind: WorkItemKind,
    #[serde(rename = "ref")]
    pub reference: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Repo {
    pub url: String,
    pub slug: String,
    pub base_branch: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Caps {
    pub usd: f64,
    pub wall_clock_secs: u64,
    pub min_review_rounds: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Resume {
    pub oracle_frozen: bool,
}

/// `unit/start` params.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct WorkOrder {
    pub unit_id: String,
    pub work_item: WorkItem,
    pub tier: Tier,
    pub task: String,
    pub repo: Repo,
    pub branch: String,
    pub test_cmd: String,
    pub caps: Caps,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resume: Option<Resume>,
}

/// What the harness saw. `fleetd` maps each to a `fleet-core` trigger (SP-2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Observation {
    Provisioned,
    OracleFrozen,
    BuildFinished,
    ChecksPassed,
    ChecksFailed,
    EmptyDiff,
    ReviewFinished {
        round: u32,
        unresolved_blockers: u32,
        checks_green: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LogStream {
    Agent,
    Check,
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Minor,
    Blocker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    Branch,
    Pr,
    Diff,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ErrorScope {
    Docker,
    Github,
    Agent,
    System,
    Harness,
}

/// `unit/event` params (a notification).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UnitEvent {
    Observed {
        observation: Observation,
    },
    /// Every figure is incremental since the previous `metric` for this unit, not a running total.
    /// The control plane sums them into the per-unit spend (spec §4).
    Metric {
        tokens_in: u64,
        tokens_out: u64,
        cost_usd: f64,
        elapsed_ms: u64,
    },
    Log {
        stream: LogStream,
        line: String,
    },
    Finding {
        round: u32,
        severity: Severity,
        title: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        file: Option<String>,
        resolved: bool,
    },
    Artifact {
        kind: ArtifactKind,
        #[serde(rename = "ref")]
        reference: String,
    },
    Error {
        scope: ErrorScope,
        retryable: bool,
        detail: String,
    },
}

/// `gate/request` params (a request the harness blocks on).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "gate", rename_all = "snake_case")]
pub enum GateRequest {
    Oracle {
        test_files: Vec<String>,
        hash: String,
        summary: String,
    },
}

/// The control plane's answer to `gate/request`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct GateReply {
    pub approved: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edited_test_files: Option<Vec<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    PrOpen,
    NoChange,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DeliveryEvidence {
    Bundle { bundle_path: String },
    Push,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PrRef {
    pub url: String,
    pub number: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TestRun {
    pub command: String,
    pub exit_code: i32,
}

/// Claims the control plane re-reads from the sinks; never trusted as reported (spec §4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Evidence {
    pub branch: String,
    pub head_sha: String,
    pub delivery: DeliveryEvidence,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pr: Option<PrRef>,
    pub test: TestRun,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oracle_hash: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Failure {
    pub scope: ErrorScope,
    pub detail: String,
}

/// `unit/result` params — the last message a harness sends.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct UnitResult {
    pub outcome: Outcome,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<Evidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure: Option<Failure>,
}

/// Params/result for messages that carry nothing (`unit/halt`, `unit/resume`, `unit/abandon`).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub struct Empty {}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, to_value};

    #[test]
    fn tier_and_enums_are_snake_case_on_the_wire() {
        assert_eq!(to_value(Tier::T2).unwrap(), json!("t2"));
        assert_eq!(to_value(Isolation::Container).unwrap(), json!("container"));
        assert_eq!(
            to_value(WorkItemKind::RoadmapItem).unwrap(),
            json!("roadmap_item")
        );
        assert_eq!(to_value(Outcome::PrOpen).unwrap(), json!("pr_open"));
        assert!(
            Tier::T2.requires_oracle() && Tier::T3.requires_oracle() && !Tier::T1.requires_oracle()
        );
    }

    #[test]
    fn work_item_reference_is_ref_and_optional_fields_are_omitted() {
        let item = WorkItem {
            kind: WorkItemKind::Issue,
            reference: "adbarc92/audience#82".into(),
            fingerprint: None,
        };
        assert_eq!(
            to_value(&item).unwrap(),
            json!({"kind": "issue", "ref": "adbarc92/audience#82"})
        );
    }

    #[test]
    fn unit_event_observed_nests_a_kind_tagged_observation() {
        let ev = UnitEvent::Observed {
            observation: Observation::ReviewFinished {
                round: 3,
                unresolved_blockers: 0,
                checks_green: true,
            },
        };
        assert_eq!(
            to_value(&ev).unwrap(),
            json!({"type": "observed", "observation": {
                "kind": "review_finished", "round": 3, "unresolved_blockers": 0, "checks_green": true
            }})
        );
        let metric = UnitEvent::Metric {
            tokens_in: 10,
            tokens_out: 2,
            cost_usd: 0.5,
            elapsed_ms: 7,
        };
        assert_eq!(to_value(&metric).unwrap()["type"], json!("metric"));
    }

    #[test]
    fn evidence_delivery_is_kind_tagged() {
        let ev = Evidence {
            branch: "agent/u1".into(),
            head_sha: "a".repeat(40),
            delivery: DeliveryEvidence::Bundle {
                bundle_path: "u1.bundle".into(),
            },
            pr: None,
            test: TestRun {
                command: "cargo test".into(),
                exit_code: 0,
            },
            oracle_hash: None,
        };
        let v = to_value(&ev).unwrap();
        assert_eq!(
            v["delivery"],
            json!({"kind": "bundle", "bundle_path": "u1.bundle"})
        );
        assert!(v.get("pr").is_none() && v.get("oracle_hash").is_none());
        assert_eq!(
            to_value(DeliveryEvidence::Push).unwrap(),
            json!({"kind": "push"})
        );
    }

    #[test]
    fn gate_request_is_gate_tagged_and_round_trips() {
        let req = GateRequest::Oracle {
            test_files: vec!["tests/a.test.js".into()],
            hash: "h".into(),
            summary: "s".into(),
        };
        let v = to_value(&req).unwrap();
        assert_eq!(v["gate"], json!("oracle"));
        let back: GateRequest = serde_json::from_value(v).unwrap();
        assert_eq!(back, req);
    }
}
