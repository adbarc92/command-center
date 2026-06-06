//! Host-side git + GitHub operations. The daemon — never the container — holds
//! push credentials and performs the merge/PR (SP1 spec, Section 5). `FakeForge`
//! (tests) and a real `gh`/octocrab-backed impl (Phase 2) are the implementors.

use async_trait::async_trait;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MergeResult {
    Clean,
    Conflict,
}

/// GitHub computes `mergeable` asynchronously; `Pending` means "poll again".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mergeability {
    Mergeable,
    Dirty,
    Pending,
}

#[derive(Debug, thiserror::Error)]
pub enum ForgeError {
    #[error("forge failure: {0}")]
    Failed(String),
}

#[async_trait]
pub trait Forge: Send + Sync {
    /// Import the bundle into the host clone, fetch a fresh base, and trial-merge.
    async fn trial_merge(&self, bundle: &Path, branch: &str) -> Result<MergeResult, ForgeError>;
    /// Push the branch with the scoped token and open a PR; returns the PR URL.
    async fn open_pr(&self, branch: &str) -> Result<String, ForgeError>;
    /// One poll of GitHub's mergeability for the open PR.
    async fn poll_mergeable(&self, pr_url: &str) -> Result<Mergeability, ForgeError>;
}
