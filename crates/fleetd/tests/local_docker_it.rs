//! Real-Docker integration test for `LocalDockerRunner`. Ignored by default
//! (needs Docker + the `cc-agent:dev` image); run with:
//!
//!   cargo test -p fleetd --test local_docker_it -- --ignored
//!
//! It exercises the full Spike-1 escape through the `Runner` API: provision a
//! container, commit inside the named volume, export a complete bundle to the
//! host, and verify the host-reconstructed branch SHA matches the container's.

use fleet_core::{GateConfig, Tier};
use fleetd::local_docker::LocalDockerRunner;
use fleetd::runner::{Runner, UnitSpec};
use std::process::Command as StdCommand;

fn spec(unit_id: &str) -> UnitSpec {
    UnitSpec {
        unit_id: unit_id.into(),
        tier: Tier::T1,
        task: "integration".into(),
        usd_cap: 1.0,
        wall_clock_secs: 0,
        gate: GateConfig::default(),
        // provision clones this; the public sandbox repo exists.
        repo_url: "https://github.com/adbarc92/command-center-agent-sandbox".into(),
        repo_slug: "adbarc92/command-center-agent-sandbox".into(),
        base_branch: "main".into(),
        branch: "agent/it".into(),
        test_cmd: "node --test".into(),
    }
}

#[tokio::test]
#[ignore = "requires Docker and the cc-agent:dev image"]
async fn provision_commit_export_roundtrip() {
    let runner = LocalDockerRunner::new("cc-agent:dev");
    let spec = spec("it-roundtrip");
    let handle = runner.provision(&spec).await.expect("provision");

    // Create a repo + feature branch inside the named volume (Spike 1).
    let script = "set -e; cd /work; rm -rf repo; git init -q repo; cd repo; \
         git config core.autocrlf false; git config user.email it@local; \
         git config user.name IT; echo base > README.md; git add README.md; \
         git commit -q -m base; git checkout -q -b agent/it; \
         printf 'x\\ny\\n' > f.txt; git add f.txt; git commit -q -m feat; \
         git rev-parse agent/it";
    let exec = runner.exec(&handle, "/work", &["sh".into(), "-c".into(), script.into()]).await;
    let bundle = match &exec {
        Ok(o) if o.exit_code == 0 => runner.export_bundle(&handle, "agent/it").await.ok(),
        _ => None,
    };

    // Reconstruct on the host and read the SHA back (the daemon's host clone).
    let dir = std::env::temp_dir().join("cc-it-clone");
    let _ = std::fs::remove_dir_all(&dir);
    let host_sha = bundle.as_ref().and_then(|b| {
        StdCommand::new("git")
            .args(["clone", "-q", &b.to_string_lossy(), &dir.to_string_lossy()])
            .status()
            .ok()?;
        let o = StdCommand::new("git")
            .args(["-C", &dir.to_string_lossy(), "rev-parse", "refs/remotes/origin/agent/it"])
            .output()
            .ok()?;
        Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
    });

    // Tear down BEFORE asserting so a failed assert never leaks the container.
    runner.teardown(&handle).await.expect("teardown");
    let _ = std::fs::remove_dir_all(&dir);
    if let Some(b) = &bundle {
        let _ = std::fs::remove_file(b);
    }

    let out = exec.expect("exec git script");
    assert_eq!(out.exit_code, 0, "git script failed: {:?}", out.stdout);
    let container_sha = out.stdout.last().expect("a sha line").trim().to_string();
    assert_eq!(container_sha.len(), 40, "expected a full sha, got {container_sha:?}");
    assert_eq!(host_sha.as_deref(), Some(container_sha.as_str()), "host SHA must match container");
}
