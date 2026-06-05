//! The public view-contract: events flowing OUT (`/stream`) and commands flowing
//! IN (REST control). The ops grid is the first consumer; a later game-skin view
//! is a second consumer of the same shapes. See SP1 spec, Section 2.

use crate::Phase;
use serde::{Deserialize, Serialize};

/// One outbound event payload. On the wire it is tagged with `"type"` and wrapped
/// by the daemon in an envelope carrying `unit_id`, `seq`, and `ts`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    PhaseChanged {
        from: Phase,
        to: Phase,
        #[serde(skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        cmd_id: Option<String>,
    },
    /// T2/T3: the frozen test set proposed for human approval.
    OracleProposed {
        test_files: Vec<String>,
        hash: String,
        summary: String,
    },
    Iteration {
        kind: IterationKind,
        n: u32,
    },
    Log {
        stream: LogStream,
        line: String,
    },
    Metric {
        tokens_in: u64,
        tokens_out: u64,
        cost_usd: f64,
        elapsed_ms: u64,
    },
    Finding {
        round: u32,
        severity: Severity,
        title: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        file: Option<String>,
        resolved: bool,
    },
    Artifact {
        kind: ArtifactKind,
        /// `ref` is a Rust keyword, so the field is `reference` on the wire `ref`.
        #[serde(rename = "ref")]
        reference: String,
    },
    Blocked {
        reason: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        cap: Option<String>,
        detail: String,
    },
    Error {
        scope: ErrorScope,
        retryable: bool,
        detail: String,
    },
    Done {
        result: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IterationKind {
    Build,
    Check,
    Review,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogStream {
    Agent,
    Check,
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Minor,
    Blocker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    Branch,
    Pr,
    Diff,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorScope {
    Docker,
    Github,
    Agent,
    System,
}

/// One inbound control command. Each carries a client-generated `cmd_id` that the
/// daemon echoes back on the resulting `PhaseChanged`/`Error` event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum Command {
    Halt { cmd_id: String },
    Resume { cmd_id: String },
    Abandon { cmd_id: String },
    /// The T3 final gate: human ships the PR from `NeedsHuman`.
    Ship { cmd_id: String },
    /// Approve the frozen test set; may carry an edited set (T2/T3).
    ApproveOracle {
        cmd_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        edited_test_files: Option<Vec<String>>,
    },
    RejectOracle { cmd_id: String },
}

impl Command {
    pub fn cmd_id(&self) -> &str {
        match self {
            Command::Halt { cmd_id }
            | Command::Resume { cmd_id }
            | Command::Abandon { cmd_id }
            | Command::Ship { cmd_id }
            | Command::ApproveOracle { cmd_id, .. }
            | Command::RejectOracle { cmd_id } => cmd_id,
        }
    }

    /// Map a command to the state-machine trigger it requests.
    pub fn to_trigger(&self) -> crate::Trigger {
        use crate::Trigger;
        match self {
            Command::Halt { .. } => Trigger::Halt,
            Command::Resume { .. } => Trigger::Resume,
            Command::Abandon { .. } => Trigger::Abandon,
            Command::Ship { .. } => Trigger::Ship,
            Command::ApproveOracle { .. } => Trigger::OracleApproved,
            Command::RejectOracle { .. } => Trigger::OracleRejected,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase_changed_wire_shape_is_tagged_and_omits_none() {
        let e = Event::PhaseChanged {
            from: Phase::Building,
            to: Phase::Checking,
            reason: None,
            cmd_id: None,
        };
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["type"], "phase_changed");
        assert_eq!(v["from"], "building");
        assert_eq!(v["to"], "checking");
        assert!(v.get("reason").is_none(), "None reason must be omitted");
        assert!(v.get("cmd_id").is_none(), "None cmd_id must be omitted");
    }

    #[test]
    fn artifact_serializes_ref_key() {
        let e = Event::Artifact {
            kind: ArtifactKind::Pr,
            reference: "https://github.com/x/y/pull/1".into(),
        };
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["type"], "artifact");
        assert_eq!(v["kind"], "pr");
        assert_eq!(v["ref"], "https://github.com/x/y/pull/1");
    }

    #[test]
    fn metric_roundtrips() {
        let e = Event::Metric { tokens_in: 9457, tokens_out: 4, cost_usd: 0.207, elapsed_ms: 6734 };
        let json = serde_json::to_string(&e).unwrap();
        let back: Event = serde_json::from_str(&json).unwrap();
        assert_eq!(e, back);
    }

    #[test]
    fn command_parses_and_maps_to_trigger() {
        let c: Command = serde_json::from_str(r#"{"command":"halt","cmd_id":"abc"}"#).unwrap();
        assert_eq!(c.cmd_id(), "abc");
        assert_eq!(c.to_trigger(), crate::Trigger::Halt);
    }

    #[test]
    fn approve_oracle_optional_edits_omitted_when_none() {
        let c = Command::ApproveOracle { cmd_id: "x".into(), edited_test_files: None };
        let v = serde_json::to_value(&c).unwrap();
        assert_eq!(v["command"], "approve_oracle");
        assert!(v.get("edited_test_files").is_none());
        assert_eq!(c.to_trigger(), crate::Trigger::OracleApproved);
    }
}
