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
