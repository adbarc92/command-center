//! `LocalDockerRunner` — the real `Runner`, implemented over the `docker` CLI
//! (the exact command sequence proven in Spike 1). Container per unit, isolated
//! clone in a named volume, branch escape via `git bundle` + `docker cp`.
//!
//! We shell to the `docker` CLI rather than `bollard` for SP1: it's the sequence
//! the spike validated, it's cross-platform, and the `Runner` trait means a
//! bollard impl can replace it later with zero changes elsewhere.

use crate::runner::{ExecOutput, Handle, Liveness, Runner, RunnerError, UnitSpec};
use async_trait::async_trait;
use std::path::PathBuf;
use tokio::process::Command;

pub struct LocalDockerRunner {
    image: String,
}

impl LocalDockerRunner {
    pub fn new(image: impl Into<String>) -> Self {
        Self { image: image.into() }
    }

    fn container_name(unit_id: &str) -> String {
        format!("cc_{}", sanitize(unit_id))
    }

    fn volume_name(unit_id: &str) -> String {
        format!("ccvol_{}", sanitize(unit_id))
    }
}

/// Keep only docker-name-safe characters.
fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-') { c } else { '_' })
        .collect()
}

/// Run `docker <args>`; return (exit_code, stdout, stderr).
async fn docker(args: Vec<String>) -> Result<(i32, String, String), RunnerError> {
    let out = Command::new("docker")
        .args(&args)
        .output()
        .await
        .map_err(|e| RunnerError::Failed(format!("spawn docker {args:?}: {e}")))?;
    Ok((
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    ))
}

/// Run `docker <args>` and fail if the exit code is non-zero.
async fn docker_ok(args: Vec<String>) -> Result<String, RunnerError> {
    let (code, out, err) = docker(args.clone()).await?;
    if code != 0 {
        return Err(RunnerError::Failed(format!("docker {args:?} exited {code}: {err}")));
    }
    Ok(out)
}

#[async_trait]
impl Runner for LocalDockerRunner {
    async fn provision(&self, spec: &UnitSpec) -> Result<Handle, RunnerError> {
        let name = Self::container_name(&spec.unit_id);
        let vol = Self::volume_name(&spec.unit_id);

        docker_ok(vec!["volume".into(), "create".into(), vol.clone()]).await?;
        docker_ok(vec![
            "run".into(),
            "-d".into(),
            "--name".into(),
            name.clone(),
            "--label".into(),
            format!("cc.unit_id={}", spec.unit_id),
            "-v".into(),
            format!("{vol}:/work"),
            // Forward the daemon's key without putting its value on the command line.
            "-e".into(),
            "ANTHROPIC_API_KEY".into(),
            "-w".into(),
            "/work".into(),
            self.image.clone(),
            "sleep".into(),
            "infinity".into(),
        ])
        .await?;

        Ok(Handle { id: name })
    }

    async fn exec(&self, handle: &Handle, argv: &[String]) -> Result<ExecOutput, RunnerError> {
        let mut args = vec!["exec".to_string(), handle.id.clone()];
        args.extend(argv.iter().cloned());
        let (code, out, _err) = docker(args).await?;
        Ok(ExecOutput {
            exit_code: code,
            stdout: out.lines().map(str::to_string).collect(),
            usage: None, // claude stream-json usage parsing lands in Phase 2b
        })
    }

    async fn health(&self, handle: &Handle) -> Result<Liveness, RunnerError> {
        let (code, out, _) = docker(vec![
            "inspect".into(),
            "-f".into(),
            "{{.State.Running}}".into(),
            handle.id.clone(),
        ])
        .await?;
        Ok(if code == 0 && out.trim() == "true" { Liveness::Alive } else { Liveness::Stalled })
    }

    async fn export_bundle(&self, handle: &Handle, branch: &str) -> Result<PathBuf, RunnerError> {
        // Complete, self-contained bundle (Spike 1: no prerequisites).
        docker_ok(vec![
            "exec".into(),
            handle.id.clone(),
            "sh".into(),
            "-c".into(),
            format!("cd /work/repo && git bundle create /work/out.bundle {branch}"),
        ])
        .await?;

        let host_path = std::env::temp_dir().join(format!("{}.bundle", handle.id));
        docker_ok(vec![
            "cp".into(),
            format!("{}:/work/out.bundle", handle.id),
            host_path.to_string_lossy().into_owned(),
        ])
        .await?;
        Ok(host_path)
    }

    async fn teardown(&self, handle: &Handle) -> Result<(), RunnerError> {
        // Best-effort: remove container then its volume; ignore "already gone".
        let _ = docker(vec!["rm".into(), "-f".into(), handle.id.clone()]).await;
        let vol = handle.id.replacen("cc_", "ccvol_", 1);
        let _ = docker(vec!["volume".into(), "rm".into(), vol]).await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_sanitized_and_prefixed() {
        assert_eq!(LocalDockerRunner::container_name("a/b 1"), "cc_a_b_1");
        assert_eq!(LocalDockerRunner::volume_name("a/b 1"), "ccvol_a_b_1");
    }
}
