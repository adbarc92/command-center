"""test_direct_invocation.py — regression: scripts invoked BY ABSOLUTE PATH must work.

When the hooks/CLI run scripts as `python.exe .../src/session_state/resume.py`, Python
puts only the script's own directory on sys.path (not the parent `src/`), so
`from session_state import ...` raises ModuleNotFoundError without a bootstrap.
These tests exercise the exact same invocation pattern.
"""
from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parents[1] / "src" / "session_state"


def _init_git_repo(d: Path) -> None:
    for args in (
        ["git", "init", "-q"],
        ["git", "config", "user.email", "t@t"],
        ["git", "config", "user.name", "t"],
    ):
        subprocess.run(args, cwd=d, check=False)
    (d / "f.txt").write_text("x", encoding="utf-8")
    subprocess.run(["git", "add", "."], cwd=d, check=False)
    subprocess.run(["git", "commit", "-qm", "init"], cwd=d, check=False)


def _env(claude_home: Path) -> dict:
    e = dict(os.environ)
    e["CLAUDE_CONFIG_DIR"] = str(claude_home)
    e.pop("CC_SESSION_STATE_DISABLE", None)
    return e


def test_capture_rich_runs_by_path(tmp_path: Path) -> None:
    repo = tmp_path / "repo"
    repo.mkdir()
    _init_git_repo(repo)
    payload = tmp_path / "rich.json"
    payload.write_text(
        json.dumps({"did": "by-path test", "next": [], "open_threads": []}),
        encoding="utf-8",
    )
    r = subprocess.run(
        [sys.executable, str(SCRIPTS / "capture_rich.py"), "--input", str(payload)],
        cwd=repo,
        env=_env(tmp_path / ".claude"),
        capture_output=True,
        text=True,
    )
    assert r.returncode == 0, f"stderr={r.stderr!r} stdout={r.stdout!r}"
    assert "narrative saved" in r.stdout.lower(), (
        f"expected 'narrative saved' in stdout; got: {r.stdout!r}\nstderr: {r.stderr!r}"
    )


def test_resume_emits_by_path_when_state_exists(tmp_path: Path) -> None:
    repo = tmp_path / "repo"
    repo.mkdir()
    _init_git_repo(repo)
    payload = tmp_path / "rich.json"
    payload.write_text(
        json.dumps({"did": "hello-resume", "next": ["n"], "open_threads": []}),
        encoding="utf-8",
    )
    subprocess.run(
        [sys.executable, str(SCRIPTS / "capture_rich.py"), "--input", str(payload)],
        cwd=repo,
        env=_env(tmp_path / ".claude"),
        capture_output=True,
        text=True,
    )
    r = subprocess.run(
        [sys.executable, str(SCRIPTS / "resume.py")],
        input=json.dumps({"source": "startup"}),
        cwd=repo,
        env=_env(tmp_path / ".claude"),
        capture_output=True,
        text=True,
    )
    assert r.returncode == 0, f"stderr={r.stderr!r} stdout={r.stdout!r}"
    assert "hello-resume" in r.stdout, (
        f"expected 'hello-resume' in stdout; got: {r.stdout!r}\nstderr: {r.stderr!r}"
    )
    assert "hookSpecificOutput" in r.stdout, (
        f"expected 'hookSpecificOutput' in stdout; got: {r.stdout!r}\nstderr: {r.stderr!r}"
    )
