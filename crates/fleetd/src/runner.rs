//! The isolation seam. Everything container-specific lives behind `Runner`.
//! `FakeRunner` (tests) and `LocalDockerRunner` (Phase 2) are the impls.

use async_trait::async_trait;
use fleet_core::{GateConfig, Tier};
use std::path::PathBuf;

/// What the daemon needs to start a unit.
#[derive(Clone, Debug)]
pub struct UnitSpec {
    pub unit_id: String,
    pub tier: Tier,
    pub task: String,
    /// Hard USD ceiling for the whole unit (daemon-enforced; the in-container
    /// `--max-budget-usd` backstop is set from the remaining amount in Phase 2).
    pub usd_cap: f64,
    pub gate: GateConfig,
}

/// Opaque handle to a provisioned container.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Handle {
    pub id: String,
}

/// Token/cost usage parsed from a Claude Code `result` record (Spike 2).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Usage {
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub cost_usd: f64,
}

/// Result of one `exec` step.
#[derive(Clone, Debug)]
pub struct ExecOutput {
    pub exit_code: i32,
    pub stdout: Vec<String>,
    pub usage: Option<Usage>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Liveness {
    Alive,
    Stalled,
}

#[derive(Debug, thiserror::Error)]
pub enum RunnerError {
    #[error("runner failure: {0}")]
    Failed(String),
}

#[async_trait]
pub trait Runner: Send + Sync {
    async fn provision(&self, spec: &UnitSpec) -> Result<Handle, RunnerError>;
    /// Run one bounded command in the container. Phase 2 streams stdout live;
    /// Phase 1 returns the collected output.
    async fn exec(&self, handle: &Handle, argv: &[String]) -> Result<ExecOutput, RunnerError>;
    async fn health(&self, handle: &Handle) -> Result<Liveness, RunnerError>;
    /// `git bundle` the branch and `docker cp` it to a host path (Spike 1).
    async fn export_bundle(
        &self,
        handle: &Handle,
        branch: &str,
    ) -> Result<PathBuf, RunnerError>;
    async fn teardown(&self, handle: &Handle) -> Result<(), RunnerError>;
}
