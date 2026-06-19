from pathlib import Path, PureWindowsPath
import session_state.keying as k


def test_path_to_slug_windows_drive():
    s = k.path_to_slug(PureWindowsPath(r"D:\MajorProjects\CURRENT\command-center"))
    assert s == "D--MajorProjects-CURRENT-command-center"


def test_canonical_project_root_collapses_worktree():
    p = Path("/repo/.claude/worktrees/agent-x/crates")
    # only collapses the .claude/worktrees/<name> tail; deeper subpath is kept by repo_root via git,
    # canonical_project_root itself collapses to the parent of .claude
    assert k.canonical_project_root(Path("/repo/.claude/worktrees/agent-x")) == Path("/repo")
    # non-worktree path returned unchanged
    assert k.canonical_project_root(Path("/repo/src")) == Path("/repo/src")


def test_repo_key_non_git_uses_cwd(tmp_path):
    key = k.repo_key(tmp_path)
    assert key == k.path_to_slug(tmp_path)


def test_state_dir_created_under_claude_home(tmp_path, monkeypatch):
    monkeypatch.setenv("CLAUDE_CONFIG_DIR", str(tmp_path / ".claude"))
    d = k.state_dir(tmp_path)
    assert d.is_dir()
    assert d.parent.name == "sessions"


def test_check_meta_detects_collision(tmp_path):
    d = tmp_path / "key"
    d.mkdir()
    assert k.check_meta(d, "D:/repo-a") is True       # first write
    assert k.check_meta(d, "D:/repo-a") is True       # same repo, ok
    assert k.check_meta(d, "D:/other-repo") is False  # collision
    assert (d / "COLLISION").exists()


import importlib.util


def _load_recall():
    from session_state.keying import claude_home
    recall_path = claude_home() / "tools" / "context-offload" / "recall.py"
    if not recall_path.exists():
        return None
    spec = importlib.util.spec_from_file_location("_recall_vendor_src", recall_path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def test_vendoring_parity_with_recall():
    recall = _load_recall()
    if recall is None:
        import pytest
        pytest.skip("recall.py not installed on this machine")
    from pathlib import PureWindowsPath
    import session_state.keying as k
    samples = [
        PureWindowsPath(r"D:\MajorProjects\CURRENT\command-center"),
        Path("/repo/.claude/worktrees/agent-x"),
        Path("/plain/project"),
    ]
    for s in samples:
        assert k.path_to_slug(s) == recall.path_to_slug(s)
        assert k.canonical_project_root(s) == recall.canonical_project_root(s)
