//! In-memory fakes for the `Runner` and `Forge` seams. These let the entire
//! lifecycle driver be tested without Docker, git, or Claude.

use crate::forge::{Forge, ForgeError, MergeResult, Mergeability};
use crate::runner::{ExecOutput, Handle, Liveness, Runner, RunnerError, UnitSpec, Usage};
use async_trait::async_trait;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// A `Runner` that replays scripted `ExecOutput`s in order, ignoring argv.
pub struct FakeRunner {
    scripted: Mutex<VecDeque<ExecOutput>>,
    /// When set, `exec` ignores the script and returns this every time.
    always: Option<ExecOutput>,
    health: Liveness,
    has_diff: bool,
    /// Shared call counters (clone before moving the runner into `run`).
    pub teardowns: Arc<AtomicUsize>,
    pub discards: Arc<AtomicUsize>,
    /// Unit-ids returned by `list_unit_containers` (for reconciliation tests).
    unit_containers: Vec<String>,
    /// Scripted `read_files` results (consumed front-to-back), same
    /// interior-mutability wrapper as `scripted`.
    oracle_reads: Mutex<Vec<Vec<String>>>,
}

impl FakeRunner {
    /// Build from a script of exec outputs (consumed front-to-back).
    pub fn new(script: Vec<ExecOutput>) -> Self {
        Self {
            scripted: Mutex::new(script.into()),
            always: None,
            health: Liveness::Alive,
            has_diff: true,
            teardowns: Arc::new(AtomicUsize::new(0)),
            discards: Arc::new(AtomicUsize::new(0)),
            unit_containers: Vec::new(),
            oracle_reads: Mutex::new(Vec::new()),
        }
    }

    /// Make every `exec` return `out` (ignores the script). For retry tests.
    pub fn always(mut self, out: ExecOutput) -> Self {
        self.always = Some(out);
        self
    }

    /// Make `has_diff` report no changes (to exercise the NO_CHANGE path).
    pub fn empty_diff(mut self) -> Self {
        self.has_diff = false;
        self
    }

    /// Script the results of successive `read_files` calls (consumed front-to-back).
    pub fn oracle_contents(mut self, reads: Vec<Vec<String>>) -> Self {
        self.oracle_reads = Mutex::new(reads);
        self
    }

    /// Convenience: an exec output with exit code 0 and a given cost.
    pub fn ok(cost_usd: f64, stdout: &[&str]) -> ExecOutput {
        ExecOutput {
            exit_code: 0,
            stdout: stdout.iter().map(|s| s.to_string()).collect(),
            stderr: vec![],
            usage: Some(Usage { tokens_in: 100, tokens_out: 10, cost_usd }),
        }
    }

    /// Convenience: a failing exec output (non-zero exit).
    pub fn fail(cost_usd: f64) -> ExecOutput {
        ExecOutput {
            exit_code: 1,
            stdout: vec![],
            stderr: vec![],
            usage: Some(Usage { cost_usd, ..Default::default() }),
        }
    }

    /// An Anthropic rate-limit: non-zero exit, the signal ONLY on stderr, no usage
    /// (the hard-cap shape — proves the classifier reads stderr end to end).
    pub fn rate_limited() -> ExecOutput {
        ExecOutput {
            exit_code: 1,
            stdout: vec![],
            stderr: vec!["API Error: 429 rate limit exceeded".into()],
            usage: None,
        }
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
        if let Some(out) = &self.always {
            return Ok(out.clone());
        }
        self.scripted
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| RunnerError::Failed("FakeRunner: script exhausted".into()))
    }

    async fn health(&self, _handle: &Handle) -> Result<Liveness, RunnerError> {
        Ok(self.health)
    }

    async fn list_unit_containers(&self) -> Result<Vec<String>, RunnerError> {
        Ok(self.unit_containers.clone())
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
        self.teardowns.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    async fn discard(&self, _handle: &Handle) -> Result<(), RunnerError> {
        self.discards.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    async fn reap_unit(&self, _unit_id: &str) -> Result<(), RunnerError> {
        self.teardowns.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    async fn read_files(&self, _handle: &Handle, _glob: &str) -> Result<Vec<String>, RunnerError> {
        // Pop the next scripted read; default to a STABLE constant so unscripted tests
        // see an unchanged oracle and never trip tamper detection.
        let mut q = self.oracle_reads.lock().unwrap();
        Ok(if q.is_empty() { vec!["<frozen>".to_string()] } else { q.remove(0) })
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
