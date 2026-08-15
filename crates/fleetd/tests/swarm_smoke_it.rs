//! Real-seam smoke: requires `claude` + git + ANTHROPIC_API_KEY. Ignored by default.

#[tokio::test]
#[ignore = "requires git network access; clones a public repo"]
async fn git_doc_source_clones_reads_and_cleans_up() {
    use fleetd::docsource::{DocSource, GitDocSource};
    let d = GitDocSource::new();
    let out = d
        .read(
            "https://github.com/adbarc92/command-center-agent-sandbox",
            "main",
            "README.md",
        )
        .await;
    assert!(out.is_ok(), "reads a known file: {out:?}");
}
