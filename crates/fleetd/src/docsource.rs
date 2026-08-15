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
    async fn read(
        &self,
        repo_url: &str,
        base_branch: &str,
        doc_path: &str,
    ) -> Result<String, DocError>;
}

/// Canned content keyed only by `doc_path`; a path of "missing.md" → NotFound.
pub struct FakeDocSource {
    pub content: String,
}
impl FakeDocSource {
    pub fn new(content: &str) -> Self {
        Self {
            content: content.into(),
        }
    }
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

/// Validate clone inputs before handing them to `git`. Rejects flag-smuggling
/// (leading `-`), non-https schemes, and unsafe doc paths (absolute or `..`).
fn validate_clone_inputs(
    repo_url: &str,
    base_branch: &str,
    doc_path: &str,
) -> Result<(), DocError> {
    if repo_url.starts_with('-') || base_branch.starts_with('-') {
        return Err(DocError::Failed(
            "repo_url/base_branch must not start with '-'".into(),
        ));
    }
    if !repo_url.starts_with("https://") {
        return Err(DocError::Failed("repo_url must be an https:// URL".into()));
    }
    if base_branch.is_empty() {
        return Err(DocError::Failed("base_branch is required".into()));
    }
    // doc_path must be a relative path with no parent-dir escapes.
    let p = std::path::Path::new(doc_path);
    if p.is_absolute()
        || doc_path.is_empty()
        || p.components().any(|c| {
            matches!(
                c,
                std::path::Component::ParentDir
                    | std::path::Component::Prefix(_)
                    | std::path::Component::RootDir
            )
        })
    {
        return Err(DocError::NotFound(doc_path.into()));
    }
    Ok(())
}

/// Shallow-clones the base branch to a temp dir, reads the file, and removes the
/// dir on drop regardless of outcome (a failed/empty swarm has no driver to clean up).
pub struct GitDocSource;
impl GitDocSource {
    pub fn new() -> Self {
        Self
    }
}
impl Default for GitDocSource {
    fn default() -> Self {
        Self::new()
    }
}

/// Per-call sequence so two concurrent real swarms never collide on the same temp
/// clone dir (which would make one's drop-guard delete the other's clone).
static CLONE_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

struct TempClone(std::path::PathBuf);
impl Drop for TempClone {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[async_trait]
impl DocSource for GitDocSource {
    async fn read(
        &self,
        repo_url: &str,
        base_branch: &str,
        doc_path: &str,
    ) -> Result<String, DocError> {
        validate_clone_inputs(repo_url, base_branch, doc_path)?;
        let dir = std::env::temp_dir().join(format!(
            "cc-plan-{}-{}",
            std::process::id(),
            CLONE_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let _guard = TempClone(dir.clone());
        let ok = tokio::process::Command::new("git")
            .args(["clone", "--depth", "1", "--branch"])
            .arg(base_branch)
            .arg("--")
            .arg(repo_url)
            .arg(&dir)
            .output()
            .await
            .map_err(|e| DocError::Failed(e.to_string()))?;
        if !ok.status.success() {
            return Err(DocError::Failed(String::from_utf8_lossy(&ok.stderr).into()));
        }
        let base = tokio::fs::canonicalize(&dir)
            .await
            .map_err(|e| DocError::Failed(e.to_string()))?;
        let target = tokio::fs::canonicalize(base.join(doc_path))
            .await
            .map_err(|_| DocError::NotFound(doc_path.into()))?;
        if !target.starts_with(&base) {
            return Err(DocError::NotFound(doc_path.into()));
        }
        tokio::fs::read_to_string(&target)
            .await
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
        assert!(d
            .read("u", "main", "spec.md")
            .await
            .unwrap()
            .contains("spec"));
        assert!(matches!(
            d.read("u", "main", "missing.md").await,
            Err(DocError::NotFound(_))
        ));
    }

    #[test]
    fn validate_clone_inputs_rejects_flag_and_traversal_and_scheme() {
        assert!(validate_clone_inputs("https://h/r", "main", "spec.md").is_ok());
        assert!(validate_clone_inputs("-x", "main", "spec.md").is_err()); // flag smuggle (url)
        assert!(validate_clone_inputs("https://h/r", "--upload-pack=x", "s").is_err()); // flag smuggle (branch)
        assert!(validate_clone_inputs("git@h:r", "main", "spec.md").is_err()); // non-https
        assert!(validate_clone_inputs("https://h/r", "main", "../etc/passwd").is_err()); // traversal
        assert!(validate_clone_inputs("https://h/r", "main", "/etc/passwd").is_err()); // absolute
        assert!(validate_clone_inputs("https://h/r", "main", "").is_err()); // empty
    }
}
