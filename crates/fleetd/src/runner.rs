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
    /// `--max-budget-usd` backstop is set from the remaining amount).
    pub usd_cap: f64,
    /// Wall-clock ceiling in seconds (0 = disabled). A backstop against an agent
    /// that loops/stalls and burns time (hence money) without tripping the USD
    /// cap between steps.
    pub wall_clock_secs: u64,
    pub gate: GateConfig,
    /// Clone URL the container fetches the project from (network is open).
    pub repo_url: String,
    /// `owner/name` slug for `gh` operations.
    pub repo_slug: String,
    /// Base branch the agent branches from and the PR targets.
    pub base_branch: String,
    /// The agent's working branch (e.g. `agent/<unit_id>`).
    pub branch: String,
    /// The project's test command, e.g. `npm test` (the objective check).
    pub test_cmd: String,
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
    /// Run one bounded command in the container at `workdir`. Returns the
    /// collected output (live streaming is a later refinement).
    async fn exec(
        &self,
        handle: &Handle,
        workdir: &str,
        argv: &[String],
    ) -> Result<ExecOutput, RunnerError>;
    async fn health(&self, handle: &Handle) -> Result<Liveness, RunnerError>;
    /// Stage and commit all working-tree changes on the agent branch. Returns
    /// true if a commit was actually created (i.e. there were changes). The
    /// daemon does this — agents (e.g. `claude`) edit files but don't commit.
    async fn commit_all(&self, handle: &Handle, message: &str) -> Result<bool, RunnerError>;
    /// Whether `branch` has a non-empty diff against `base` (used to route a
    /// no-op task to `NO_CHANGE` instead of opening an empty PR).
    async fn has_diff(
        &self,
        handle: &Handle,
        base: &str,
        branch: &str,
    ) -> Result<bool, RunnerError>;
    /// `git bundle` the branch and `docker cp` it to a host path (Spike 1).
    async fn export_bundle(
        &self,
        handle: &Handle,
        branch: &str,
    ) -> Result<PathBuf, RunnerError>;
    async fn teardown(&self, handle: &Handle) -> Result<(), RunnerError>;
}
