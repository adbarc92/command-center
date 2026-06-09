//! Fetch the doc's *content* from the target repo so the planner core never
//! touches git/fs. `FakeDocSource` (tests) returns canned content or a
//! not-found error; `GitDocSource` (real, Task 17) shallow-clones and reads.

use async_trait::async_trait;

#[derive(Debug, thiserror::Error)]
pub enum DocError {
    #[error("doc not found: {0}")]
    NotFound(String),
    #[error("doc source failure: {0}")]
    Failed(String),
}

#[async_trait]
pub trait DocSource: Send + Sync {
    async fn read(&self, repo_url: &str, base_branch: &str, doc_path: &str)
        -> Result<String, DocError>;
}

/// Canned content keyed only by `doc_path`; a path of "missing.md" → NotFound.
pub struct FakeDocSource {
    pub content: String,
}
impl FakeDocSource {
    pub fn new(content: &str) -> Self { Self { content: content.into() } }
}
#[async_trait]
impl DocSource for FakeDocSource {
    async fn read(&self, _repo: &str, _base: &str, doc_path: &str) -> Result<String, DocError> {
        if doc_path == "missing.md" {
            return Err(DocError::NotFound(doc_path.into()));
        }
        Ok(self.content.clone())
    }
}

/// Shallow-clones the base branch to a temp dir, reads the file, and removes the
/// dir on drop regardless of outcome (a failed/empty swarm has no driver to clean up).
pub struct GitDocSource;
impl GitDocSource { pub fn new() -> Self { Self } }
impl Default for GitDocSource { fn default() -> Self { Self::new() } }

struct TempClone(std::path::PathBuf);
impl Drop for TempClone {
    fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); }
}

#[async_trait]
impl DocSource for GitDocSource {
    async fn read(&self, repo_url: &str, base_branch: &str, doc_path: &str) -> Result<String, DocError> {
        let dir = std::env::temp_dir().join(format!("cc-plan-{}", std::process::id()));
        let _guard = TempClone(dir.clone());
        let ok = tokio::process::Command::new("git")
            .args(["clone", "--depth", "1", "--branch", base_branch, repo_url])
            .arg(&dir).output().await
            .map_err(|e| DocError::Failed(e.to_string()))?;
        if !ok.status.success() {
            return Err(DocError::Failed(String::from_utf8_lossy(&ok.stderr).into()));
        }
        let path = dir.join(doc_path);
        tokio::fs::read_to_string(&path).await
            .map_err(|_| DocError::NotFound(doc_path.into()))
        // _guard drops here → temp dir removed.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn fake_doc_source_returns_content_or_not_found() {
        let d = FakeDocSource::new("# spec\n- a\n- b\n");
        assert!(d.read("u", "main", "spec.md").await.unwrap().contains("spec"));
        assert!(matches!(d.read("u", "main", "missing.md").await, Err(DocError::NotFound(_))));
    }
}
