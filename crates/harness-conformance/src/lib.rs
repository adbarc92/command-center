//! Conformance kit for `harness-protocol`. It spawns a harness as a child process and checks, from
//! outside, that it speaks the protocol correctly. It never inspects a harness's internals.

mod cases;
mod fixtures;
mod session;

pub use cases::run_all;
pub use fixtures::work_order;
pub use session::{Recv, Session};

use std::time::Duration;

#[derive(Debug, Clone)]
pub struct KitConfig {
    /// Program and arguments that start the harness.
    pub command: Vec<String>,
    /// Extra environment for the harness process.
    pub env: Vec<(String, String)>,
    /// Longest one case may run before the harness is killed.
    pub wall_clock: Duration,
    /// How long a harness has to answer a message or exit.
    pub grace: Duration,
}

impl KitConfig {
    pub fn new(command: Vec<String>) -> Self {
        Self {
            command,
            env: Vec::new(),
            wall_clock: Duration::from_secs(30),
            grace: Duration::from_secs(5),
        }
    }
}

/// One way a harness broke the protocol. Every variant is observable from outside the process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Violation {
    SpawnFailed(String),
    NoInitializeResponse,
    InitializeRejected(String),
    VersionMismatchAccepted,
    ExitedWithoutResult,
    Malformed { line: String },
    InvalidMessage,
    UnknownMethod { method: String },
    UnexpectedGateRequest,
    MessageAfterResult { method: String },
    DidNotExitAfterResult,
    MeteringDeclaredButSilent,
    WallClockExceeded,
    InvalidResult(String),
    GateRejectionIgnored,
    InterruptNotHonored { method: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaseOutcome {
    Pass,
    /// The harness declared it lacks the capability this case needs.
    Skipped(String),
    Fail(Violation),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseReport {
    pub name: &'static str,
    pub outcome: CaseOutcome,
}

impl CaseReport {
    pub fn passed(&self) -> bool {
        !matches!(self.outcome, CaseOutcome::Fail(_))
    }
}
