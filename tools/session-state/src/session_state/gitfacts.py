"""gitfacts.py — collect git facts via a single porcelain-v2 call + a filesystem probe.

porcelain v2 record prefixes parsed for `dirty`:
  '1' changed, '2' renamed/copied (path is field after the score), 'u' unmerged, '?' untracked.
'# branch.head' gives the branch or the literal '(detached)'. upstream/ab headers are absent
when detached and are intentionally NOT used (ahead/behind is not tracked).
"""
from __future__ import annotations

import shutil
import subprocess
from pathlib import Path


def parse_porcelain_v2(text: str) -> dict:
    branch: str | None = None
    detached = False
    dirty: list[str] = []
    for line in text.splitlines():
        if line.startswith("# branch.head "):
            head = line[len("# branch.head "):].strip()
            if head == "(detached)":
                detached = True
                branch = None
            else:
                branch = head
        elif line.startswith("1 ") or line.startswith("2 "):
            # ordinary/renamed: path is the last tab-or-space field; renames carry "\told".
            payload = line.split("\t")[0]
            dirty.append(payload.split(" ")[-1])
        elif line.startswith("u "):
            dirty.append(line.split("\t")[0].split(" ")[-1])
        elif line.startswith("? "):
            dirty.append(line[2:].strip())
    return {"branch": branch, "detached": detached, "dirty": dirty}


def in_progress(git_dir: Path) -> str | None:
    if (git_dir / "rebase-merge").exists() or (git_dir / "rebase-apply").exists():
        return "rebase"
    if (git_dir / "MERGE_HEAD").exists():
        return "merge"
    if (git_dir / "BISECT_LOG").exists():
        return "bisect"
    return None


def _git(cwd: Path, args: list[str], git_bin: str) -> subprocess.CompletedProcess:
    return subprocess.run(
        [git_bin, "-C", str(cwd), *args],
        capture_output=True, text=True, timeout=5,
    )


def collect_git_facts(cwd: Path, dirty_cap: int = 50) -> dict | None:
    git_bin = shutil.which("git")
    if git_bin is None:
        # binary missing — visible, not silent (Spec §9 #10)
        return {"branch": None, "detached": False, "in_progress": None,
                "head": None, "dirty": [], "worktree": None, "git_unavailable": True}
    try:
        top = _git(cwd, ["rev-parse", "--show-toplevel"], git_bin)
        if top.returncode != 0:
            return None  # not a git repo
        status = _git(cwd, ["--no-optional-locks", "status", "--porcelain=v2", "--branch"], git_bin)
        parsed = parse_porcelain_v2(status.stdout)
        head_subject = None
        head = _git(cwd, ["log", "-1", "--format=%h %s"], git_bin)
        if head.returncode == 0:
            head_subject = head.stdout.strip()
        git_common = _git(cwd, ["rev-parse", "--git-dir"], git_bin)
        git_dir = Path(cwd) / git_common.stdout.strip() if git_common.returncode == 0 else Path(cwd) / ".git"
        # worktree: linked-worktree subpath if cwd's toplevel differs from the main worktree
        wt = None
        common = _git(cwd, ["rev-parse", "--path-format=absolute", "--git-common-dir"], git_bin)
        if common.returncode == 0:
            common_root = Path(common.stdout.strip()).parent
            if common_root != Path(top.stdout.strip()):
                wt = str(Path(top.stdout.strip()).name)
        return {
            "branch": parsed["branch"],
            "detached": parsed["detached"],
            "in_progress": in_progress(git_dir),
            "head": head_subject,
            "dirty": parsed["dirty"][:dirty_cap],
            "worktree": wt,
            "git_unavailable": False,
        }
    except Exception:
        return None
