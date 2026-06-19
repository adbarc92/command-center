"""keying.py — repo→state-dir keying.

canonical_project_root / path_to_slug are VENDORED from
~/.claude/tools/context-offload/recall.py (do not import across tools — separate install
roots). test_keying.py asserts parity. If recall.py changes, re-vendor.
"""
from __future__ import annotations

import json
import os
import subprocess
from pathlib import Path


def claude_home() -> Path:
    override = os.environ.get("CLAUDE_CONFIG_DIR")
    return Path(override) if override else Path.home() / ".claude"


def canonical_project_root(cwd: Path) -> Path:
    """Collapse `<root>/.claude/worktrees/<name>` back to `<root>` (vendored from recall.py)."""
    parts = cwd.parts
    for i in range(len(parts) - 1):
        if parts[i] == ".claude" and i + 1 < len(parts) and parts[i + 1] == "worktrees":
            return Path(*parts[:i])
    return cwd


def path_to_slug(path: Path) -> str:
    """Claude Code's project slug: replace separators and drive colon with '-' (vendored)."""
    s = str(path)
    return s.replace("\\", "-").replace("/", "-").replace(":", "-")


def repo_root(cwd: Path) -> Path | None:
    try:
        out = subprocess.run(
            ["git", "-C", str(cwd), "rev-parse", "--show-toplevel"],
            capture_output=True, text=True, timeout=5,
        )
        if out.returncode != 0:
            return None
        return canonical_project_root(Path(out.stdout.strip()))
    except Exception:
        return None


def repo_key(cwd: Path) -> str:
    root = repo_root(cwd)
    return path_to_slug(root if root is not None else cwd)


def state_dir(cwd: Path) -> Path:
    d = claude_home() / "state" / "sessions" / repo_key(cwd)
    (d / "scratch").mkdir(parents=True, exist_ok=True)
    return d


def check_meta(dir: Path, canonical_repo: str) -> bool:
    """Write meta.json on first use; return False (and drop a COLLISION marker) on mismatch."""
    meta = dir / "meta.json"
    if not meta.exists():
        meta.write_text(json.dumps({"repo": canonical_repo}), encoding="utf-8")
        return True
    try:
        existing = json.loads(meta.read_text(encoding="utf-8")).get("repo")
    except Exception:
        existing = None
    if existing == canonical_repo:
        return True
    (dir / "COLLISION").write_text(
        f"expected {existing!r} got {canonical_repo!r}", encoding="utf-8"
    )
    return False
