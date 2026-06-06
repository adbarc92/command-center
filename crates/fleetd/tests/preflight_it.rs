//! $0 full-pipeline pre-flight: exercises the entire non-model path end-to-end
//! against the real sandbox repo and opens a REAL, mergeable PR — proving the
//! Docker + git + checks + GitHub plumbing before any paid live run.
//!
//! Requires Docker + cc-agent:dev + an authed `gh` with push rights to the
//! sandbox. Ignored by default; run with:
//!   cargo test -p fleetd --test preflight_it -- --ignored --nocapture
//!
//! The agent's work is STUBBED (we write the files a real agent would), so this
//! costs nothing. Leaves one PR open on the sandbox as proof.

use fleet_core::{GateConfig, Tier};
use fleetd::forge::{Forge, MergeResult, Mergeability};
use fleetd::gh_forge::GhForge;
use fleetd::local_docker::LocalDockerRunner;
use fleetd::runner::{Runner, UnitSpec};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const SLUG: &str = "adbarc92/command-center-agent-sandbox";
const URL: &str = "https://github.com/adbarc92/command-center-agent-sandbox";

#[tokio::test]
#[ignore = "requires Docker + cc-agent:dev + authed gh; opens a real PR"]
async fn full_pipeline_opens_a_real_mergeable_pr() {
    let millis = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis();
    let unit_id = format!("preflight-{millis}");
    let branch = format!("agent/{unit_id}");

    let spec = UnitSpec {
        unit_id: unit_id.clone(),
        tier: Tier::T1,
        task: "stub".into(),
        usd_cap: 1.0,
        wall_clock_secs: 0,
        gate: GateConfig::default(),
        repo_url: URL.into(),
        repo_slug: SLUG.into(),
        base_branch: "main".into(),
        branch: branch.clone(),
        test_cmd: "node --test".into(),
    };

    let runner = LocalDockerRunner::new("cc-agent:dev");
    let handle = runner.provision(&spec).await.expect("provision (clone sandbox + branch)");

    // ── stub the agent: write impl + its test (what oracle+build would do) ──
    let work = "set -e; cd /work/repo; \
        mkdir -p src; \
        printf 'module.exports.sum = (a, b) => a + b;\\n' > src/index.js; \
        printf 'const test=require(\"node:test\");const assert=require(\"node:assert\");\
const {sum}=require(\"./src/index.js\");\
test(\"sum adds\",()=>assert.strictEqual(sum(2,3),5));\\n' > sum.test.js";
    let w = runner.exec(&handle, "/work/repo", &["sh".into(), "-c".into(), work.into()]).await
        .expect("write stub files");
    assert_eq!(w.exit_code, 0, "stub write failed: {:?}", w.stdout);

    // ── daemon commits the work ──
    let committed = runner.commit_all(&handle, "feat: implement sum").await.expect("commit_all");
    assert!(committed, "expected a commit to be created");

    // ── checks (the objective signal) ──
    let check = runner.exec(&handle, "/work/repo", &["node".into(), "--test".into()]).await
        .expect("run checks");
    assert_eq!(check.exit_code, 0, "checks should pass: {:?}", check.stdout);

    // ── non-empty diff vs base ──
    let diff = runner.has_diff(&handle, "main", &branch).await.expect("has_diff");
    assert!(diff, "expected a non-empty diff vs main");

    // ── escape the branch + open a real PR via GhForge ──
    let bundle = runner.export_bundle(&handle, &branch).await.expect("export bundle");
    let host_clone = std::env::temp_dir().join(format!("cc-preflight-{millis}"));
    let forge = GhForge::new(URL, SLUG, "main", host_clone.clone(), format!("pre-flight: {unit_id}"));

    let merge = forge.trial_merge(&bundle, &branch).await.expect("trial merge");
    assert_eq!(merge, MergeResult::Clean, "branch should merge cleanly onto main");

    let pr = forge.open_pr(&branch).await.expect("open PR");
    println!("PR: {pr}");
    assert!(pr.contains("github.com"), "expected a PR url, got {pr:?}");

    // ── poll GitHub mergeability (async) ──
    let mut verdict = Mergeability::Pending;
    for _ in 0..15 {
        verdict = forge.poll_mergeable(&pr).await.expect("poll mergeable");
        if verdict != Mergeability::Pending {
            break;
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
    assert_eq!(verdict, Mergeability::Mergeable, "GitHub should report the PR mergeable");

    // ── cleanup the container + host clone (leave the PR as proof) ──
    runner.teardown(&handle).await.expect("teardown");
    let _ = std::fs::remove_dir_all(&host_clone);
    let _ = std::fs::remove_file(&bundle);

    println!("PRE-FLIGHT OK — real mergeable PR opened at {pr}");
}
