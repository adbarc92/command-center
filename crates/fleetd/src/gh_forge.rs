//! `GhForge` — the real `Forge`, implemented over host `git` + the `gh` CLI.
//! Push credentials stay here (host-side), never in the container. The branch
//! arrives as a bundle exported by the `Runner`; we import it into a host clone,
//! trial-merge a fresh base, push, and open the PR.

use crate::forge::{Forge, ForgeError, MergeResult, Mergeability};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use tokio::process::Command;

pub struct GhForge {
    repo_url: String,
    repo_slug: String,
    base_branch: String,
    host_clone: PathBuf,
    title: String,
}

impl GhForge {
    pub fn new(
        repo_url: impl Into<String>,
        repo_slug: impl Into<String>,
        base_branch: impl Into<String>,
        host_clone: PathBuf,
        title: impl Into<String>,
    ) -> Self {
        Self {
            repo_url: repo_url.into(),
            repo_slug: repo_slug.into(),
            base_branch: base_branch.into(),
            host_clone,
            title: title.into(),
        }
    }

    async fn ensure_clone(&self) -> Result<(), ForgeError> {
        if self.host_clone.join(".git").is_dir() {
            return Ok(());
        }
        run("git", &["clone", &self.repo_url, &self.host_clone.to_string_lossy()]).await?;
        Ok(())
    }

    fn git_dir(&self) -> String {
        self.host_clone.to_string_lossy().into_owned()
    }
}

#[async_trait]
impl Forge for GhForge {
    async fn trial_merge(&self, bundle: &Path, branch: &str) -> Result<MergeResult, ForgeError> {
        guard_branch(branch)?;
        self.ensure_clone().await?;
        let dir = self.git_dir();
        let bundle = bundle.to_string_lossy().into_owned();

        // Import the agent branch from the bundle and refresh the base.
        run("git", &["-C", &dir, "fetch", &bundle, &format!("{branch}:{branch}")]).await?;
        run("git", &["-C", &dir, "fetch", "origin", &self.base_branch]).await?;

        // Trial-merge the agent branch onto a fresh base; clean exit == mergeable.
        run("git", &["-C", &dir, "checkout", "-f", &format!("origin/{}", self.base_branch)]).await?;
        let merged = run_status(
            "git",
            &["-C", &dir, "merge", "--no-commit", "--no-ff", branch],
        )
        .await?;
        // Always abort/clean the trial, regardless of outcome.
        let _ = run_status("git", &["-C", &dir, "merge", "--abort"]).await;

        Ok(if merged { MergeResult::Clean } else { MergeResult::Conflict })
    }

    async fn open_pr(&self, branch: &str) -> Result<String, ForgeError> {
        guard_branch(branch)?;
        let dir = self.git_dir();
        run("git", &["-C", &dir, "push", "origin", &format!("{branch}:{branch}")]).await?;
        let url = run(
            "gh",
            &[
                "pr",
                "create",
                "--repo",
                &self.repo_slug,
                "--base",
                &self.base_branch,
                "--head",
                branch,
                "--title",
                &self.title,
                "--body",
                "Opened autonomously by command-center SP1.",
            ],
        )
        .await?;
        Ok(url.trim().to_string())
    }

    async fn poll_mergeable(&self, pr_url: &str) -> Result<Mergeability, ForgeError> {
        let out = run("gh", &["pr", "view", pr_url, "--json", "mergeable", "-q", ".mergeable"]).await?;
        Ok(map_mergeable(out.trim()))
    }
}

/// Map GitHub's `mergeable` enum to our tri-state.
fn map_mergeable(s: &str) -> Mergeability {
    match s {
        "MERGEABLE" => Mergeability::Mergeable,
        "CONFLICTING" => Mergeability::Dirty,
        _ => Mergeability::Pending, // "UNKNOWN" or anything unexpected → poll again
    }
}

/// Reject branch names that could inject options/args into git/gh.
fn guard_branch(b: &str) -> Result<(), ForgeError> {
    let ok = !b.is_empty()
        && !b.starts_with('-')
        && !b.contains("..")
        && b.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '/' | '-'));
    if ok {
        Ok(())
    } else {
        Err(ForgeError::Failed(format!("unsafe branch name: {b:?}")))
    }
}

/// Run a command, returning stdout on success or an error including stderr.
async fn run(program: &str, args: &[&str]) -> Result<String, ForgeError> {
    let out = Command::new(program)
        .args(args)
        .output()
        .await
        .map_err(|e| ForgeError::Failed(format!("spawn {program}: {e}")))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(ForgeError::Failed(format!(
            "{program} {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        )))
    }
}

/// Run a command and return only whether it succeeded (for trial merges).
async fn run_status(program: &str, args: &[&str]) -> Result<bool, ForgeError> {
    let status = Command::new(program)
        .args(args)
        .output()
        .await
        .map_err(|e| ForgeError::Failed(format!("spawn {program}: {e}")))?
        .status;
    Ok(status.success())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mergeable_mapping() {
        assert_eq!(map_mergeable("MERGEABLE"), Mergeability::Mergeable);
        assert_eq!(map_mergeable("CONFLICTING"), Mergeability::Dirty);
        assert_eq!(map_mergeable("UNKNOWN"), Mergeability::Pending);
        assert_eq!(map_mergeable(""), Mergeability::Pending);
    }

    #[test]
    fn branch_guard_blocks_injection() {
        assert!(guard_branch("agent/u1").is_ok());
        assert!(guard_branch("--upload-pack=x").is_err());
        assert!(guard_branch("a; rm -rf /").is_err());
    }
}
