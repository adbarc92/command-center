//! In-memory fakes for the `Runner` and `Forge` seams. These let the entire
//! lifecycle driver be tested without Docker, git, or Claude.

use crate::forge::{Forge, ForgeError, MergeResult, Mergeability};
use crate::runner::{ExecOutput, Handle, Liveness, Runner, RunnerError, UnitSpec, Usage};
use async_trait::async_trait;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// A `Runner` that replays scripted `ExecOutput`s in order, ignoring argv.
pub struct FakeRunner {
    scripted: Mutex<VecDeque<ExecOutput>>,
    health: Liveness,
    has_diff: bool,
}

impl FakeRunner {
    /// Build from a script of exec outputs (consumed front-to-back).
    pub fn new(script: Vec<ExecOutput>) -> Self {
        Self {
            scripted: Mutex::new(script.into()),
            health: Liveness::Alive,
            has_diff: true,
        }
    }

    /// Make `has_diff` report no changes (to exercise the NO_CHANGE path).
    pub fn empty_diff(mut self) -> Self {
        self.has_diff = false;
        self
    }

    /// Convenience: an exec output with exit code 0 and a given cost.
    pub fn ok(cost_usd: f64, stdout: &[&str]) -> ExecOutput {
        ExecOutput {
            exit_code: 0,
            stdout: stdout.iter().map(|s| s.to_string()).collect(),
            usage: Some(Usage { tokens_in: 100, tokens_out: 10, cost_usd }),
        }
    }

    /// Convenience: a failing exec output (non-zero exit).
    pub fn fail(cost_usd: f64) -> ExecOutput {
        ExecOutput { exit_code: 1, stdout: vec![], usage: Some(Usage { cost_usd, ..Default::default() }) }
    }
}

#[async_trait]
impl Runner for FakeRunner {
    async fn provision(&self, _spec: &UnitSpec) -> Result<Handle, RunnerError> {
        Ok(Handle { id: "fake-container".into() })
    }

    async fn exec(
        &self,
        _handle: &Handle,
        _workdir: &str,
        _argv: &[String],
    ) -> Result<ExecOutput, RunnerError> {
        self.scripted
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| RunnerError::Failed("FakeRunner: script exhausted".into()))
    }

    async fn health(&self, _handle: &Handle) -> Result<Liveness, RunnerError> {
        Ok(self.health)
    }

    async fn commit_all(&self, _handle: &Handle, _message: &str) -> Result<bool, RunnerError> {
        Ok(true)
    }

    async fn has_diff(&self, _h: &Handle, _base: &str, _branch: &str) -> Result<bool, RunnerError> {
        Ok(self.has_diff)
    }

    async fn export_bundle(&self, _handle: &Handle, _branch: &str) -> Result<PathBuf, RunnerError> {
        Ok(PathBuf::from("/fake/out.bundle"))
    }

    async fn teardown(&self, _handle: &Handle) -> Result<(), RunnerError> {
        Ok(())
    }
}

/// A `Forge` with configurable merge + mergeability outcomes.
pub struct FakeForge {
    pub merge: MergeResult,
    pub mergeable: Mergeability,
}

impl Default for FakeForge {
    fn default() -> Self {
        Self { merge: MergeResult::Clean, mergeable: Mergeability::Mergeable }
    }
}

#[async_trait]
impl Forge for FakeForge {
    async fn trial_merge(&self, _bundle: &Path, _branch: &str) -> Result<MergeResult, ForgeError> {
        Ok(self.merge)
    }

    async fn open_pr(&self, _branch: &str) -> Result<String, ForgeError> {
        Ok("https://fake/pr/1".into())
    }

    async fn poll_mergeable(&self, _pr_url: &str) -> Result<Mergeability, ForgeError> {
        Ok(self.mergeable)
    }
}
