# Session-State Resume Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A global Claude Code tool that auto-captures per-repo dev-session state (git facts + agent-authored narrative) into an append-only timeline and auto-surfaces the latest state on session start, so resuming any repo is zero-friction.

**Architecture:** Stdlib-only Python invoked by three hooks (`Stop` → throttled per-session scratch; `SessionEnd` → `auto` boundary record; `SessionStart` → resume injection) plus a `save-state` skill that appends agent-authored `rich` records via a temp-file contract. Durable records live in `~/.claude/state/sessions/<repo-key>/timeline.jsonl` (locked appends); the freshest scratch + latest narrative are merged, branch-scoped, at read time. A PowerShell installer wires the hooks into `~/.claude/settings.json`.

**Tech Stack:** Python 3.11+ (stdlib only at runtime), pytest + UV (dev/test only), PowerShell 7 (installer), Git.

**Spec:** `docs/superpowers/specs/2026-06-19-session-state-resume-design.md` (read it; this plan implements it).

## Global Constraints

- **Runtime: Python standard library only.** No third-party imports in any shipped `src/session_state/*.py`. `uv`/`pytest` are dev/test only. (Spec §6, §10)
- **Python floor:** `requires-python = ">=3.11"`. (Matches `tools/cache-countdown` convention.)
- **Hooks must never block or crash a session:** every hook entry wraps all logic in try/except and **always exits 0**; emits nothing on the empty/error path. (Spec §6, §9)
- **Encoding:** every entry script reconfigures `sys.stdout`/`sys.stderr` to UTF-8 before output (Windows console is cp1252). (Spec §9)
- **State root:** `~/.claude/state/sessions/<repo-key>/`, honoring `CLAUDE_CONFIG_DIR` if set (as `recall.py` does). (Spec §3)
- **Kill-switch:** every hook entry checks env `CC_SESSION_STATE_DISABLE` first and no-ops (exit 0) if set. (Spec §4, §7)
- **Repo keying is vendored, not imported,** from `~/.claude/tools/context-offload/recall.py`; a parity test asserts behavior matches. (Spec §3)
- **Install target & invocation:** code is copied to `~/.claude/tools/session-state/`; hooks invoke `python.exe` **directly** (not `uv run`) for cold-start speed. (Spec §7)
- **Tooling location:** built in the command-center checkout under `tools/session-state/`, mirroring `tools/cache-countdown` and `tools/budget-checkpoint`.

---

## File Structure

```
tools/session-state/
  pyproject.toml                 # project metadata; requires-python >=3.11; no runtime deps
  README.md                      # what it is, install, uninstall, kill-switch
  install.ps1                    # copy to ~/.claude/tools, wire 3 hooks, -Uninstall, -Purge
  src/session_state/
    __init__.py
    keying.py                    # canonical_project_root, path_to_slug, repo_key, state_dir, meta guard
    gitfacts.py                  # resolve_git, collect_git_facts (porcelain v2 parse + in_progress probe)
    lock.py                      # file_lock() context manager (msvcrt/fcntl, bounded retry, LockTimeout)
    store.py                     # path helpers, append_record (locked+render), read_timeline, scratch r/w, prune
    merge.py                     # resolve_state(...), render_latest_md(...), render_resume_block(...)
    capture_scratch.py           # Stop-hook entry (thin)
    capture_end.py               # SessionEnd-hook entry (thin)
    capture_rich.py              # --input <tempfile> entry (thin)
    resume.py                    # SessionStart-hook entry (thin)
    cli.py                       # list / show / prune / uninstall
  tests/
    test_stdlib_only.py
    test_keying.py
    test_gitfacts.py
    test_lock.py
    test_store.py
    test_merge.py
    test_entries.py
    test_install_json.py
.claude/skills/save-state/SKILL.md   # project skill: write temp JSON + run capture_rich.py
```

Tasks build the testable core first (keying → gitfacts → lock → store → merge), then the thin entry scripts, then the installer, the skill, and the rich-record producer guarantee.

---

### Task 1: Project scaffold + stdlib-only guard

**Files:**
- Create: `tools/session-state/pyproject.toml`
- Create: `tools/session-state/src/session_state/__init__.py`
- Test: `tools/session-state/tests/test_stdlib_only.py`

**Interfaces:**
- Produces: the `session_state` package importable under `uv run`; the constant `session_state.__all_modules__` (list of shipped module names) used by the stdlib guard.

- [ ] **Step 1: Write `pyproject.toml`**

```toml
[project]
name = "session-state"
version = "0.1.0"
description = "Per-repo dev-session state capture + resume for Claude Code."
requires-python = ">=3.11"
dependencies = []

[dependency-groups]
dev = ["pytest>=8.0.0"]

[tool.pytest.ini_options]
testpaths = ["tests"]
pythonpath = ["src"]
```

- [ ] **Step 2: Write `src/session_state/__init__.py`**

```python
"""session_state — per-repo dev-session state capture + resume (stdlib only at runtime)."""

# Shipped runtime modules (no third-party imports allowed in these).
__all_modules__ = [
    "keying",
    "gitfacts",
    "lock",
    "store",
    "merge",
    "capture_scratch",
    "capture_end",
    "capture_rich",
    "resume",
    "cli",
]
```

- [ ] **Step 3: Write the failing stdlib-only test** in `tests/test_stdlib_only.py`

```python
import ast
import importlib.util
import sys
from pathlib import Path

SRC = Path(__file__).resolve().parents[1] / "src" / "session_state"

# Everything importable without installing anything (stdlib + our own package).
_STDLIB = set(sys.stdlib_module_names) | {"session_state"}


def _imported_top_levels(py_file: Path) -> set[str]:
    tree = ast.parse(py_file.read_text(encoding="utf-8"))
    tops: set[str] = set()
    for node in ast.walk(tree):
        if isinstance(node, ast.Import):
            for n in node.names:
                tops.add(n.name.split(".")[0])
        elif isinstance(node, ast.ImportFrom) and node.level == 0 and node.module:
            tops.add(node.module.split(".")[0])
    return tops


def test_runtime_modules_import_stdlib_only():
    offenders = {}
    for py_file in SRC.glob("*.py"):
        bad = {t for t in _imported_top_levels(py_file) if t not in _STDLIB}
        if bad:
            offenders[py_file.name] = sorted(bad)
    assert not offenders, f"non-stdlib imports found: {offenders}"
```

- [ ] **Step 4: Run it to verify it passes (only `__init__.py` exists, no bad imports)**

Run: `cd tools/session-state && uv run pytest tests/test_stdlib_only.py -v`
Expected: PASS (glob finds `__init__.py`, no offenders). This test is a standing guard re-run as later modules are added.

- [ ] **Step 5: Commit**

```bash
git add tools/session-state/pyproject.toml tools/session-state/src tools/session-state/tests/test_stdlib_only.py
git commit -m "feat(session-state): scaffold package + stdlib-only guard"
```

---

### Task 2: Repo keying (vendored from recall.py)

**Files:**
- Create: `tools/session-state/src/session_state/keying.py`
- Test: `tools/session-state/tests/test_keying.py`
- Reference: `~/.claude/tools/context-offload/recall.py` (vendor source for `canonical_project_root`, `path_to_slug`)

**Interfaces:**
- Produces:
  - `claude_home() -> Path`
  - `canonical_project_root(cwd: Path) -> Path`
  - `path_to_slug(path: Path) -> str`
  - `repo_root(cwd: Path) -> Path | None`  (git toplevel, canonicalized; None if not a git repo)
  - `repo_key(cwd: Path) -> str`  (slug of canonical repo root, or slug of cwd if non-git)
  - `state_dir(cwd: Path) -> Path`  (`<claude_home>/state/sessions/<repo_key>/`, created)
  - `check_meta(dir: Path, canonical_repo: str) -> bool`  (write meta.json if absent; True if matches, False on collision — writes `COLLISION` marker)

- [ ] **Step 1: Write failing tests** in `tests/test_keying.py`

```python
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
```

- [ ] **Step 2: Run to verify failure**

Run: `cd tools/session-state && uv run pytest tests/test_keying.py -v`
Expected: FAIL with `ModuleNotFoundError: session_state.keying`.

- [ ] **Step 3: Implement `keying.py`** (vendor the two functions verbatim from recall.py; add the new helpers)

```python
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
```

- [ ] **Step 4: Run tests to verify pass**

Run: `cd tools/session-state && uv run pytest tests/test_keying.py tests/test_stdlib_only.py -v`
Expected: PASS (all keying tests + stdlib guard still green).

- [ ] **Step 5: Add the vendoring-parity test** (append to `tests/test_keying.py`)

```python
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
```

- [ ] **Step 6: Run + commit**

Run: `cd tools/session-state && uv run pytest tests/test_keying.py -v`
Expected: PASS (parity test passes or skips cleanly).

```bash
git add tools/session-state/src/session_state/keying.py tools/session-state/tests/test_keying.py
git commit -m "feat(session-state): repo keying vendored from recall.py + parity test"
```

---

### Task 3: Git facts (porcelain v2 parse + in-progress probe)

**Files:**
- Create: `tools/session-state/src/session_state/gitfacts.py`
- Test: `tools/session-state/tests/test_gitfacts.py`

**Interfaces:**
- Consumes: nothing from prior tasks.
- Produces:
  - `parse_porcelain_v2(text: str) -> dict` → `{"branch": str|None, "detached": bool, "dirty": list[str]}`
  - `in_progress(git_dir: Path) -> str | None` → `"rebase"|"merge"|"bisect"|None`
  - `collect_git_facts(cwd: Path, dirty_cap: int = 50) -> dict | None` → the full `git` dict (Spec §2) or `None` when cwd is not a git repo. Sets `git_unavailable: True` when the git binary can't be resolved.

- [ ] **Step 1: Write failing tests** in `tests/test_gitfacts.py`

```python
from pathlib import Path
import session_state.gitfacts as g

CLEAN = "# branch.oid abc123\n# branch.head main\n# branch.upstream origin/main\n# branch.ab +0 -0\n"
DIRTY = (
    "# branch.oid abc123\n# branch.head main\n"
    "1 .M N... 100644 100644 100644 aaa bbb src/foo.rs\n"
    "2 R. N... 100644 100644 100644 ccc ddd R100 new.rs\told.rs\n"
    "? untracked.txt\n"
)
DETACHED = "# branch.oid abc123\n# branch.head (detached)\n1 .M N... 100644 100644 100644 a b x.rs\n"
CONFLICT = (
    "# branch.oid abc123\n# branch.head (detached)\n"
    "u UU N... 100644 100644 100644 100644 a b c conflicted.rs\n"
)


def test_parse_clean():
    r = g.parse_porcelain_v2(CLEAN)
    assert r == {"branch": "main", "detached": False, "dirty": []}


def test_parse_dirty_includes_renamed_and_untracked():
    r = g.parse_porcelain_v2(DIRTY)
    assert r["branch"] == "main" and r["detached"] is False
    assert "src/foo.rs" in r["dirty"]
    assert "new.rs" in r["dirty"]        # rename: new path
    assert "untracked.txt" in r["dirty"]


def test_parse_detached_branch_is_none():
    r = g.parse_porcelain_v2(DETACHED)
    assert r["branch"] is None and r["detached"] is True
    assert r["dirty"] == ["x.rs"]


def test_parse_conflict_unmerged_record():
    r = g.parse_porcelain_v2(CONFLICT)
    assert r["detached"] is True
    assert "conflicted.rs" in r["dirty"]


def test_in_progress_detects_rebase(tmp_path):
    (tmp_path / "rebase-merge").mkdir()
    assert g.in_progress(tmp_path) == "rebase"


def test_in_progress_none(tmp_path):
    assert g.in_progress(tmp_path) is None


def test_collect_git_facts_non_git(tmp_path):
    assert g.collect_git_facts(tmp_path) is None
```

- [ ] **Step 2: Run to verify failure**

Run: `cd tools/session-state && uv run pytest tests/test_gitfacts.py -v`
Expected: FAIL with `ModuleNotFoundError`.

- [ ] **Step 3: Implement `gitfacts.py`**

```python
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
```

- [ ] **Step 4: Run tests to verify pass**

Run: `cd tools/session-state && uv run pytest tests/test_gitfacts.py tests/test_stdlib_only.py -v`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add tools/session-state/src/session_state/gitfacts.py tools/session-state/tests/test_gitfacts.py
git commit -m "feat(session-state): git facts via porcelain v2 + in-progress probe"
```

---

### Task 4: Cross-platform advisory lock

**Files:**
- Create: `tools/session-state/src/session_state/lock.py`
- Test: `tools/session-state/tests/test_lock.py`

**Interfaces:**
- Produces:
  - `class LockTimeout(Exception)`
  - `file_lock(lock_path: Path, tries: int = 10, backoff: float = 0.2)` — a context manager that acquires an exclusive advisory lock on `lock_path` (a dedicated `.lock` file), retrying with backoff; raises `LockTimeout` if it can't. Uses `msvcrt` on Windows, `fcntl` on POSIX.

- [ ] **Step 1: Write failing tests** in `tests/test_lock.py`

```python
import threading
import time
from pathlib import Path
import session_state.lock as L


def test_lock_serializes_writers(tmp_path):
    lock = tmp_path / "t.lock"
    target = tmp_path / "out.txt"
    target.write_text("", encoding="utf-8")
    order = []

    def worker(tag):
        with L.file_lock(lock):
            cur = target.read_text(encoding="utf-8")
            time.sleep(0.05)  # widen the race window
            target.write_text(cur + tag + "\n", encoding="utf-8")
            order.append(tag)

    threads = [threading.Thread(target=worker, args=(str(i),)) for i in range(5)]
    for t in threads:
        t.start()
    for t in threads:
        t.join()

    lines = [l for l in target.read_text(encoding="utf-8").splitlines() if l]
    assert len(lines) == 5  # no lost updates → all writers serialized


def test_lock_timeout_raises(tmp_path):
    lock = tmp_path / "t.lock"
    with L.file_lock(lock):
        raised = []

        def contender():
            try:
                with L.file_lock(lock, tries=2, backoff=0.01):
                    pass
            except L.LockTimeout:
                raised.append(True)

        t = threading.Thread(target=contender)
        t.start()
        t.join()
        assert raised == [True]
```

- [ ] **Step 2: Run to verify failure**

Run: `cd tools/session-state && uv run pytest tests/test_lock.py -v`
Expected: FAIL with `ModuleNotFoundError`.

- [ ] **Step 3: Implement `lock.py`**

```python
"""lock.py — cross-platform exclusive advisory lock on a dedicated .lock file.

Windows: msvcrt.locking on a 1-byte region. POSIX: fcntl.flock. Bounded retry/backoff so an
append never hangs a session; on exhaustion raises LockTimeout (callers decide skip vs preserve).
"""
from __future__ import annotations

import contextlib
import time
from pathlib import Path

try:
    import msvcrt  # Windows
    _WINDOWS = True
except ImportError:  # POSIX
    import fcntl
    _WINDOWS = False


class LockTimeout(Exception):
    pass


@contextlib.contextmanager
def file_lock(lock_path: Path, tries: int = 10, backoff: float = 0.2):
    lock_path.parent.mkdir(parents=True, exist_ok=True)
    fh = open(lock_path, "a+b")
    try:
        acquired = False
        for attempt in range(tries):
            try:
                if _WINDOWS:
                    fh.seek(0)
                    msvcrt.locking(fh.fileno(), msvcrt.LK_NBLCK, 1)
                else:
                    fcntl.flock(fh.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
                acquired = True
                break
            except OSError:
                time.sleep(backoff)
        if not acquired:
            raise LockTimeout(f"could not lock {lock_path} after {tries} tries")
        yield
    finally:
        try:
            if _WINDOWS:
                fh.seek(0)
                msvcrt.locking(fh.fileno(), msvcrt.LK_UNLCK, 1)
            else:
                fcntl.flock(fh.fileno(), fcntl.LOCK_UN)
        except OSError:
            pass
        fh.close()
```

- [ ] **Step 4: Run tests to verify pass**

Run: `cd tools/session-state && uv run pytest tests/test_lock.py tests/test_stdlib_only.py -v`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add tools/session-state/src/session_state/lock.py tools/session-state/tests/test_lock.py
git commit -m "feat(session-state): cross-platform advisory file lock"
```

---

### Task 5: Store (timestamps, records, locked append, scratch, prune)

**Files:**
- Create: `tools/session-state/src/session_state/store.py`
- Test: `tools/session-state/tests/test_store.py`

**Interfaces:**
- Consumes: `keying.state_dir`, `lock.file_lock`, `merge.render_latest_md` (forward ref — Task 6 supplies it; Task 5 calls it through a module-level import that exists after Task 6, so Task 6 must land before `append_record`'s render is exercised — see Step 3 note).
- Produces:
  - `now_iso() -> str` (UTC, tz-aware, seconds precision)
  - `make_record(type, source, session_id, repo, git, **narrative) -> dict`
  - `scratch_path(dir, session_id) -> Path`; `write_scratch(dir, record)`; `read_scratches(dir) -> list[dict]`
  - `append_record(dir, record) -> bool` (locked append to `timeline.jsonl` + regen `latest.md` in the same lock hold; returns False on `LockTimeout` for `auto`, re-raises for callers that must not lose data — see Step 3)
  - `read_timeline(dir, tail=None) -> list[dict]` (skips corrupt lines)
  - `prune(dir, max_records=1000, orphan_days=7)`

- [ ] **Step 1: Write failing tests** in `tests/test_store.py`

```python
import json
from pathlib import Path
import session_state.store as s


def test_now_iso_is_utc():
    assert s.now_iso().endswith("Z") or "+00:00" in s.now_iso()


def test_make_record_auto_has_no_narrative():
    r = s.make_record("auto", "SessionEnd:other", "sid", "D:/r", {"branch": "main"})
    assert r["type"] == "auto" and "did" not in r


def test_scratch_roundtrip(tmp_path):
    rec = s.make_record("auto", "Stop", "sid1", "D:/r", {"branch": "main"})
    s.write_scratch(tmp_path, rec)
    got = s.read_scratches(tmp_path)
    assert len(got) == 1 and got[0]["session_id"] == "sid1"


def test_read_timeline_skips_corrupt(tmp_path):
    tl = tmp_path / "timeline.jsonl"
    tl.write_text('{"ts":"1","type":"auto"}\nNOT JSON\n{"ts":"2","type":"rich"}\n', encoding="utf-8")
    recs = s.read_timeline(tmp_path)
    assert [r["ts"] for r in recs] == ["1", "2"]


def test_append_record_writes_line_and_latest_md(tmp_path):
    rec = s.make_record("rich", "save-state", "sid", "D:/r", {"branch": "main"},
                        did="x", next=["a"], open_threads=[])
    assert s.append_record(tmp_path, rec) is True
    lines = (tmp_path / "timeline.jsonl").read_text(encoding="utf-8").splitlines()
    assert len(lines) == 1 and json.loads(lines[0])["did"] == "x"
    assert (tmp_path / "latest.md").exists()


def test_prune_truncates_oldest(tmp_path):
    tl = tmp_path / "timeline.jsonl"
    tl.write_text("".join(f'{{"ts":"{i}","type":"auto"}}\n' for i in range(10)), encoding="utf-8")
    s.prune(tmp_path, max_records=4)
    kept = [json.loads(l)["ts"] for l in tl.read_text(encoding="utf-8").splitlines()]
    assert kept == ["6", "7", "8", "9"]


def test_prune_removes_orphan_scratch(tmp_path, monkeypatch):
    import time
    sc = tmp_path / "scratch"
    sc.mkdir()
    old = sc / "old.json"
    old.write_text("{}", encoding="utf-8")
    import os
    eight_days = time.time() - 8 * 86400
    os.utime(old, (eight_days, eight_days))
    s.prune(tmp_path, orphan_days=7)
    assert not old.exists()
```

- [ ] **Step 2: Run to verify failure**

Run: `cd tools/session-state && uv run pytest tests/test_store.py -v`
Expected: FAIL (`ModuleNotFoundError`).

- [ ] **Step 3: Implement `store.py`**

> Note: `append_record` imports `merge.render_latest_md` lazily (inside the function) so Task 5's
> tests that don't hit the render still pass before Task 6 lands; `test_append_record_writes_latest_md`
> requires Task 6's `merge.render_latest_md` to exist — keep that one test `xfail` until Task 6, then
> remove the marker. (Simpler: implement a minimal `merge.render_latest_md` stub in Task 6 Step 1.)

```python
"""store.py — record construction, locked append, scratch I/O, retention."""
from __future__ import annotations

import json
import os
import time
from datetime import datetime, timezone
from pathlib import Path

from .lock import file_lock, LockTimeout


def now_iso() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def make_record(type: str, source: str, session_id: str | None, repo: str,
                git: dict | None, **narrative) -> dict:
    rec = {"ts": now_iso(), "type": type, "source": source,
           "session_id": session_id, "repo": repo, "git": git}
    if type == "rich":
        rec["did"] = narrative.get("did", "")
        rec["next"] = narrative.get("next", [])
        rec["open_threads"] = narrative.get("open_threads", [])
    return rec


def scratch_path(dir: Path, session_id: str) -> Path:
    return dir / "scratch" / f"{session_id}.json"


def write_scratch(dir: Path, record: dict) -> None:
    (dir / "scratch").mkdir(parents=True, exist_ok=True)
    p = scratch_path(dir, record.get("session_id") or "unknown")
    p.write_text(json.dumps(record), encoding="utf-8")


def read_scratches(dir: Path) -> list[dict]:
    out = []
    sc = dir / "scratch"
    if not sc.is_dir():
        return out
    for f in sc.glob("*.json"):
        try:
            out.append(json.loads(f.read_text(encoding="utf-8")))
        except Exception:
            continue
    return out


def read_timeline(dir: Path, tail: int | None = None) -> list[dict]:
    tl = dir / "timeline.jsonl"
    if not tl.exists():
        return []
    lines = tl.read_text(encoding="utf-8").splitlines()
    if tail is not None:
        lines = lines[-tail:]
    recs = []
    for ln in lines:
        try:
            recs.append(json.loads(ln))
        except Exception:
            continue
    return recs


def append_record(dir: Path, record: dict, tries: int = 10) -> bool:
    """Append + regenerate latest.md under one lock hold. Returns True on success,
    False on LockTimeout (caller decides). Never partially writes."""
    from .merge import render_latest_md  # lazy: avoid import cycle
    dir.mkdir(parents=True, exist_ok=True)
    try:
        with file_lock(dir / "timeline.lock", tries=tries):
            with open(dir / "timeline.jsonl", "a", encoding="utf-8") as fh:
                fh.write(json.dumps(record) + "\n")
                fh.flush()
            scratches = read_scratches(dir)
            timeline = read_timeline(dir, tail=50)
            (dir / "latest.md").write_text(render_latest_md(timeline, scratches), encoding="utf-8")
        return True
    except LockTimeout:
        return False


def prune(dir: Path, max_records: int = 1000, orphan_days: int = 7) -> None:
    tl = dir / "timeline.jsonl"
    if tl.exists():
        lines = tl.read_text(encoding="utf-8").splitlines()
        if len(lines) > max_records:
            tl.write_text("\n".join(lines[-max_records:]) + "\n", encoding="utf-8")
    sc = dir / "scratch"
    if sc.is_dir():
        cutoff = time.time() - orphan_days * 86400
        for f in sc.glob("*.json"):
            try:
                if f.stat().st_mtime < cutoff:
                    f.unlink()
            except OSError:
                continue
```

- [ ] **Step 4: Run tests** (mark `test_append_record_writes_line_and_latest_md` xfail until Task 6, or do Task 6 Step 1 first)

Run: `cd tools/session-state && uv run pytest tests/test_store.py -v`
Expected: PASS for all except the latest.md render test (pending Task 6's `render_latest_md`).

- [ ] **Step 5: Commit**

```bash
git add tools/session-state/src/session_state/store.py tools/session-state/tests/test_store.py
git commit -m "feat(session-state): store — records, locked append, scratch, prune"
```

---

### Task 6: Merge + render (the branch-scoped read model)

**Files:**
- Create: `tools/session-state/src/session_state/merge.py`
- Test: `tools/session-state/tests/test_merge.py`

**Interfaces:**
- Consumes: records produced by `store.make_record` (shape per Spec §2).
- Produces:
  - `resolve_state(timeline: list[dict], scratches: list[dict]) -> dict` → `{"git": dict|None, "git_source": dict|None, "narrative": dict|None, "branch_banner": str|None}` where `git` is freshest-by-ts across scratches+timeline, `narrative` is newest rich record, `branch_banner` is set when git/narrative branches differ (and neither is detached/in_progress).
  - `render_latest_md(timeline, scratches) -> str`
  - `render_resume_block(timeline, scratches) -> str | None` (terse; None if no state)

- [ ] **Step 1: Write failing tests** in `tests/test_merge.py`

```python
import session_state.merge as m


def _rec(ts, type, branch, did=None, detached=False, in_progress=None):
    g = {"branch": branch, "detached": detached, "in_progress": in_progress,
         "head": "abc x", "dirty": [], "worktree": None, "git_unavailable": False}
    r = {"ts": ts, "type": type, "source": "t", "session_id": "s", "repo": "D:/r", "git": g}
    if type == "rich":
        r["did"] = did or "did"
        r["next"] = ["n1"]
        r["open_threads"] = ["t1"]
    return r


def test_freshest_git_from_scratch_over_older_timeline():
    timeline = [_rec("2026-01-01T00:00:00Z", "auto", "main")]
    scratch = [_rec("2026-02-01T00:00:00Z", "auto", "feat/x")]
    state = m.resolve_state(timeline, scratch)
    assert state["git"]["branch"] == "feat/x"


def test_narrative_is_newest_rich():
    timeline = [_rec("2026-01-01T00:00:00Z", "rich", "main", did="old"),
                _rec("2026-02-01T00:00:00Z", "rich", "main", did="new")]
    state = m.resolve_state(timeline, [])
    assert state["narrative"]["did"] == "new"


def test_branch_banner_when_branches_differ():
    timeline = [_rec("2026-01-01T00:00:00Z", "rich", "feat/x")]
    scratch = [_rec("2026-02-01T00:00:00Z", "auto", "main")]
    state = m.resolve_state(timeline, scratch)
    assert state["branch_banner"] is not None and "feat/x" in state["branch_banner"]


def test_no_banner_when_detached():
    timeline = [_rec("2026-01-01T00:00:00Z", "rich", "feat/x")]
    scratch = [_rec("2026-02-01T00:00:00Z", "auto", None, detached=True)]
    state = m.resolve_state(timeline, scratch)
    assert state["branch_banner"] is None


def test_resume_block_none_when_empty():
    assert m.render_resume_block([], []) is None


def test_resume_block_contains_next_and_threads():
    timeline = [_rec("2026-02-01T00:00:00Z", "rich", "main", did="shipped X")]
    block = m.render_resume_block(timeline, [])
    assert "shipped X" in block and "n1" in block and "t1" in block
```

- [ ] **Step 2: Run to verify failure**

Run: `cd tools/session-state && uv run pytest tests/test_merge.py -v`
Expected: FAIL (`ModuleNotFoundError`).

- [ ] **Step 3: Implement `merge.py`**

```python
"""merge.py — branch-scoped read model: freshest git facts + latest narrative (Spec §5)."""
from __future__ import annotations


def _newest(records: list[dict]) -> dict | None:
    return max(records, key=lambda r: r.get("ts", ""), default=None)


def resolve_state(timeline: list[dict], scratches: list[dict]) -> dict:
    git_bearing = [r for r in (timeline + scratches) if r.get("git")]
    freshest = _newest(git_bearing)
    rich = _newest([r for r in timeline if r.get("type") == "rich"])

    banner = None
    if freshest and rich and freshest.get("git") and rich.get("git"):
        fg, rg = freshest["git"], rich["git"]
        suppress = fg.get("detached") or fg.get("in_progress")
        if not suppress and fg.get("branch") and rg.get("branch") and fg["branch"] != rg["branch"]:
            wt = f" (worktree {rg['worktree']})" if rg.get("worktree") else ""
            banner = (f"narrative captured on `{rg['branch']}`{wt} — "
                      f"newest activity is on `{fg['branch']}`")
    return {
        "git": freshest["git"] if freshest else None,
        "git_source": freshest,
        "narrative": rich,
        "branch_banner": banner,
    }


def _fmt_git(g: dict | None) -> str:
    if not g:
        return "_no git facts_"
    if g.get("git_unavailable"):
        return "_git binary unavailable_"
    if g.get("detached") or g.get("in_progress"):
        op = g.get("in_progress") or "detached"
        return f"HEAD {g.get('head', '?')} ({op})"
    line = f"branch `{g.get('branch')}` @ {g.get('head', '?')}"
    if g.get("dirty"):
        line += f" — {len(g['dirty'])} changed"
    return line


def render_resume_block(timeline: list[dict], scratches: list[dict]) -> str | None:
    if not timeline and not scratches:
        return None
    st = resolve_state(timeline, scratches)
    lines = ["<session-state>", "Resuming this repo — last known state:", _fmt_git(st["git"])]
    if st["branch_banner"]:
        lines.append(f"⚠ {st['branch_banner']}")
    n = st["narrative"]
    if n:
        if n.get("did"):
            lines.append(f"Last: {n['did']}")
        for item in (n.get("next") or [])[:5]:
            lines.append(f"Next: {item}")
        for item in (n.get("open_threads") or [])[:5]:
            lines.append(f"Open: {item}")
    else:
        lines.append("(no narrative captured yet — run /save-state to record one)")
    lines.append("</session-state>")
    return "\n".join(lines)


def render_latest_md(timeline: list[dict], scratches: list[dict]) -> str:
    st = resolve_state(timeline, scratches)
    out = ["# Latest session state", "", f"**Git:** {_fmt_git(st['git'])}", ""]
    if st["branch_banner"]:
        out += [f"> ⚠ {st['branch_banner']}", ""]
    n = st["narrative"]
    if n:
        out += [f"**Last:** {n.get('did','')}", "", "**Next:**"]
        out += [f"- {i}" for i in (n.get("next") or [])]
        out += ["", "**Open threads:**"]
        out += [f"- {i}" for i in (n.get("open_threads") or [])]
    else:
        out.append("_No narrative captured yet._")
    out += ["", "## Recent history", ""]
    for r in reversed(timeline[-5:]):
        out.append(f"- `{r.get('ts')}` {r.get('type')} ({r.get('source')})")
    return "\n".join(out) + "\n"
```

- [ ] **Step 4: Run all core tests to verify pass** (and clear the Task 5 xfail)

Run: `cd tools/session-state && uv run pytest tests/ -v`
Expected: PASS, including `test_append_record_writes_line_and_latest_md`. Remove any xfail marker added in Task 5.

- [ ] **Step 5: Commit**

```bash
git add tools/session-state/src/session_state/merge.py tools/session-state/tests/test_merge.py
git commit -m "feat(session-state): branch-scoped merge + markdown/resume rendering"
```

---

### Task 7: Hook entry scripts (capture_scratch, capture_end, capture_rich, resume)

**Files:**
- Create: `tools/session-state/src/session_state/capture_scratch.py`
- Create: `tools/session-state/src/session_state/capture_end.py`
- Create: `tools/session-state/src/session_state/capture_rich.py`
- Create: `tools/session-state/src/session_state/resume.py`
- Test: `tools/session-state/tests/test_entries.py`

**Interfaces:**
- Consumes: `keying`, `gitfacts`, `store`, `merge`.
- Produces (each as a `main()` returning an int exit code, run as `python -m session_state.<x>` or `python <path>/<x>.py`):
  - `capture_scratch.main()` reads stdin `{session_id}`, throttles (30s unless git facts changed), writes scratch.
  - `capture_end.main()` reads stdin `{session_id, reason}`, skips `reason ∈ {clear,resume}`, appends `auto`, deletes own scratch, prunes.
  - `capture_rich.main(argv)` reads `--input <file>`, appends `rich`; on `LockTimeout` preserves the temp file and prints retry; else deletes the temp file in `finally`.
  - `resume.main()` reads stdin `{source}`, emits the JSON envelope only for `source ∈ {startup,resume}`.

- [ ] **Step 1: Write failing tests** in `tests/test_entries.py`

```python
import io
import json
import subprocess
import sys
from pathlib import Path

import session_state.capture_scratch as cs
import session_state.capture_end as ce
import session_state.capture_rich as cr
import session_state.resume as rz


def _init_git_repo(tmp_path):
    subprocess.run(["git", "init", "-q"], cwd=tmp_path)
    subprocess.run(["git", "config", "user.email", "t@t"], cwd=tmp_path)
    subprocess.run(["git", "config", "user.name", "t"], cwd=tmp_path)
    (tmp_path / "f.txt").write_text("x", encoding="utf-8")
    subprocess.run(["git", "add", "."], cwd=tmp_path)
    subprocess.run(["git", "commit", "-qm", "init"], cwd=tmp_path)


def test_capture_scratch_writes(tmp_path, monkeypatch, capsys):
    monkeypatch.setenv("CLAUDE_CONFIG_DIR", str(tmp_path / ".claude"))
    _init_git_repo(tmp_path)
    monkeypatch.chdir(tmp_path)
    monkeypatch.setattr("sys.stdin", io.StringIO(json.dumps({"session_id": "sid1"})))
    assert cs.main() == 0
    import session_state.keying as k
    import session_state.store as s
    scratches = s.read_scratches(k.state_dir(tmp_path))
    assert len(scratches) == 1


def test_kill_switch_noops(tmp_path, monkeypatch):
    monkeypatch.setenv("CC_SESSION_STATE_DISABLE", "1")
    monkeypatch.setattr("sys.stdin", io.StringIO("{}"))
    assert cs.main() == 0  # and writes nothing


def test_capture_end_skips_clear(tmp_path, monkeypatch):
    monkeypatch.setenv("CLAUDE_CONFIG_DIR", str(tmp_path / ".claude"))
    _init_git_repo(tmp_path)
    monkeypatch.chdir(tmp_path)
    monkeypatch.setattr("sys.stdin", io.StringIO(json.dumps({"session_id": "s", "reason": "clear"})))
    assert ce.main() == 0
    import session_state.keying as k, session_state.store as st
    assert st.read_timeline(k.state_dir(tmp_path)) == []


def test_resume_silent_on_compact(tmp_path, monkeypatch, capsys):
    monkeypatch.setattr("sys.stdin", io.StringIO(json.dumps({"source": "compact"})))
    assert rz.main() == 0
    assert capsys.readouterr().out.strip() == ""


def test_capture_rich_appends_and_deletes_tempfile(tmp_path, monkeypatch):
    monkeypatch.setenv("CLAUDE_CONFIG_DIR", str(tmp_path / ".claude"))
    _init_git_repo(tmp_path)
    monkeypatch.chdir(tmp_path)
    payload = tmp_path / "rich.json"
    payload.write_text(json.dumps({"did": "shipped", "next": ["a"], "open_threads": []}), encoding="utf-8")
    assert cr.main(["--input", str(payload)]) == 0
    assert not payload.exists()
    import session_state.keying as k, session_state.store as st
    recs = st.read_timeline(k.state_dir(tmp_path))
    assert recs and recs[-1]["did"] == "shipped"
```

- [ ] **Step 2: Run to verify failure**

Run: `cd tools/session-state && uv run pytest tests/test_entries.py -v`
Expected: FAIL (`ModuleNotFoundError`).

- [ ] **Step 3a: Implement `capture_scratch.py`**

```python
"""capture_scratch.py — Stop-hook entry: overwrite this session's scratch with freshest git facts."""
from __future__ import annotations

import json
import os
import sys
from datetime import datetime, timezone
from pathlib import Path

for _s in (sys.stdout, sys.stderr):
    try:
        _s.reconfigure(encoding="utf-8")  # type: ignore[attr-defined]
    except Exception:
        pass

THROTTLE_SECONDS = 30


def main() -> int:
    try:
        if os.environ.get("CC_SESSION_STATE_DISABLE"):
            return 0
        raw = sys.stdin.read() or "{}"
        data = json.loads(raw) if raw.strip() else {}
        session_id = data.get("session_id") or "unknown"

        from session_state import keying, gitfacts, store
        cwd = Path(os.getcwd())
        root = keying.repo_root(cwd)
        repo = str(root) if root else str(cwd)
        dir = keying.state_dir(cwd)
        if not keying.check_meta(dir, repo):
            return 0  # collision: refuse loudly (marker written), don't guess

        git = gitfacts.collect_git_facts(cwd)
        # throttle: skip if recent AND git facts unchanged
        prev = store.scratch_path(dir, session_id)
        if prev.exists():
            try:
                old = json.loads(prev.read_text(encoding="utf-8"))
                age = (datetime.now(timezone.utc)
                       - datetime.fromisoformat(old["ts"].replace("Z", "+00:00"))).total_seconds()
                if age < THROTTLE_SECONDS and old.get("git") == git:
                    return 0
            except Exception:
                pass
        rec = store.make_record("auto", "Stop", session_id, repo, git)
        store.write_scratch(dir, rec)
    except Exception:
        pass
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
```

- [ ] **Step 3b: Implement `capture_end.py`**

```python
"""capture_end.py — SessionEnd-hook entry: append an auto boundary, delete own scratch, prune."""
from __future__ import annotations

import json
import os
import sys
from pathlib import Path

for _s in (sys.stdout, sys.stderr):
    try:
        _s.reconfigure(encoding="utf-8")  # type: ignore[attr-defined]
    except Exception:
        pass

SKIP_REASONS = {"clear", "resume"}


def main() -> int:
    try:
        if os.environ.get("CC_SESSION_STATE_DISABLE"):
            return 0
        raw = sys.stdin.read() or "{}"
        data = json.loads(raw) if raw.strip() else {}
        reason = data.get("reason", "other")
        if reason in SKIP_REASONS:
            return 0
        session_id = data.get("session_id") or "unknown"

        from session_state import keying, gitfacts, store
        cwd = Path(os.getcwd())
        root = keying.repo_root(cwd)
        repo = str(root) if root else str(cwd)
        dir = keying.state_dir(cwd)
        if not keying.check_meta(dir, repo):
            return 0
        git = gitfacts.collect_git_facts(cwd)
        rec = store.make_record("auto", f"SessionEnd:{reason}", session_id, repo, git)
        store.append_record(dir, rec)              # auto: ok to skip on lock timeout
        own = store.scratch_path(dir, session_id)
        if own.exists():
            try:
                own.unlink()
            except OSError:
                pass
        store.prune(dir)
    except Exception:
        pass
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
```

- [ ] **Step 3c: Implement `capture_rich.py`**

```python
"""capture_rich.py — append an agent-authored rich record from a temp JSON file.

Contract (Spec §6.3): agent writes {did,next,open_threads} to a temp file, runs
`python capture_rich.py --input <file>`. Temp file deleted on success; PRESERVED on
lock failure with a printed retry command (never silently lose hand-authored narrative).
"""
from __future__ import annotations

import argparse
import json
import os
import sys
from pathlib import Path

for _s in (sys.stdout, sys.stderr):
    try:
        _s.reconfigure(encoding="utf-8")  # type: ignore[attr-defined]
    except Exception:
        pass


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--input", required=True)
    args = ap.parse_args(argv)
    payload = Path(args.input)
    delete_temp = True
    try:
        data = json.loads(payload.read_text(encoding="utf-8"))
        from session_state import keying, gitfacts, store
        cwd = Path(os.getcwd())
        root = keying.repo_root(cwd)
        repo = str(root) if root else str(cwd)
        dir = keying.state_dir(cwd)
        if not keying.check_meta(dir, repo):
            print("session-state: repo-key collision; narrative NOT saved.", file=sys.stderr)
            delete_temp = False
            return 1
        git = gitfacts.collect_git_facts(cwd)
        rec = store.make_record("rich", "save-state", data.get("session_id"), repo, git,
                                did=data.get("did", ""), next=data.get("next", []),
                                open_threads=data.get("open_threads", []))
        if store.append_record(dir, rec):
            print("session-state: narrative saved.")
            return 0
        delete_temp = False
        print(f"session-state: could not acquire lock; narrative NOT saved. "
              f"Temp preserved at {payload}. Retry: python capture_rich.py --input {payload}",
              file=sys.stderr)
        return 1
    except Exception as exc:
        delete_temp = False
        print(f"session-state: error saving narrative: {exc}. Temp preserved at {payload}.",
              file=sys.stderr)
        return 1
    finally:
        if delete_temp:
            try:
                payload.unlink()
            except OSError:
                pass


if __name__ == "__main__":
    raise SystemExit(main())
```

- [ ] **Step 3d: Implement `resume.py`**

```python
"""resume.py — SessionStart-hook entry: emit the merged, branch-scoped resume block."""
from __future__ import annotations

import json
import os
import sys
from pathlib import Path

for _s in (sys.stdout, sys.stderr):
    try:
        _s.reconfigure(encoding="utf-8")  # type: ignore[attr-defined]
    except Exception:
        pass

EMIT_SOURCES = {"startup", "resume"}


def main() -> int:
    try:
        if os.environ.get("CC_SESSION_STATE_DISABLE"):
            return 0
        raw = sys.stdin.read() or "{}"
        data = json.loads(raw) if raw.strip() else {}
        if data.get("source") not in EMIT_SOURCES:
            return 0

        from session_state import keying, store, merge
        cwd = Path(os.getcwd())
        dir = keying.state_dir(cwd)  # read-only path; never writes
        block = merge.render_resume_block(store.read_timeline(dir, tail=50), store.read_scratches(dir))
        if not block:
            return 0
        print(json.dumps({"hookSpecificOutput": {
            "hookEventName": "SessionStart", "additionalContext": block}}))
    except Exception:
        pass
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
```

- [ ] **Step 4: Run tests to verify pass**

Run: `cd tools/session-state && uv run pytest tests/test_entries.py -v`
Expected: PASS (requires `git` on PATH for the repo-init helper).

- [ ] **Step 5: Commit**

```bash
git add tools/session-state/src/session_state/capture_scratch.py tools/session-state/src/session_state/capture_end.py tools/session-state/src/session_state/capture_rich.py tools/session-state/src/session_state/resume.py tools/session-state/tests/test_entries.py
git commit -m "feat(session-state): hook entry scripts (scratch/end/rich/resume)"
```

---

### Task 8: CLI (list / show / prune)

**Files:**
- Create: `tools/session-state/src/session_state/cli.py`
- Test: append to `tools/session-state/tests/test_entries.py`

**Interfaces:**
- Consumes: `keying`, `store`, `merge`.
- Produces: `cli.main(argv) -> int` with subcommands `list`, `show [SELECTOR]`, `prune [SELECTOR]`. `SELECTOR` = canonical path or repo-key; default = cwd's canonical repo. (`uninstall` delegates to the installer and is documented, not implemented in Python.)

- [ ] **Step 1: Write failing test** (append to `tests/test_entries.py`)

```python
import session_state.cli as cli


def test_cli_show_renders_state(tmp_path, monkeypatch, capsys):
    monkeypatch.setenv("CLAUDE_CONFIG_DIR", str(tmp_path / ".claude"))
    _init_git_repo(tmp_path)
    monkeypatch.chdir(tmp_path)
    import session_state.keying as k, session_state.store as st
    dir = k.state_dir(tmp_path)
    st.append_record(dir, st.make_record("rich", "save-state", "s", str(tmp_path),
                                         {"branch": "main", "head": "abc x", "dirty": []},
                                         did="hello", next=[], open_threads=[]))
    assert cli.main(["show"]) == 0
    assert "hello" in capsys.readouterr().out


def test_cli_list_shows_repo(tmp_path, monkeypatch, capsys):
    monkeypatch.setenv("CLAUDE_CONFIG_DIR", str(tmp_path / ".claude"))
    _init_git_repo(tmp_path)
    monkeypatch.chdir(tmp_path)
    import session_state.keying as k
    k.state_dir(tmp_path)  # create the dir
    assert cli.main(["list"]) == 0
```

- [ ] **Step 2: Run to verify failure**

Run: `cd tools/session-state && uv run pytest tests/test_entries.py -k cli -v`
Expected: FAIL (`ModuleNotFoundError`).

- [ ] **Step 3: Implement `cli.py`**

```python
"""cli.py — inspect/maintain session-state timelines."""
from __future__ import annotations

import argparse
import os
import sys
from pathlib import Path

for _s in (sys.stdout, sys.stderr):
    try:
        _s.reconfigure(encoding="utf-8")  # type: ignore[attr-defined]
    except Exception:
        pass


def _resolve_dir(selector: str | None):
    from session_state import keying
    if selector is None:
        return keying.state_dir(Path(os.getcwd()))
    p = Path(selector)
    if p.exists():
        return keying.state_dir(p)
    # treat as a repo-key under the sessions root
    return keying.claude_home() / "state" / "sessions" / selector


def main(argv: list[str] | None = None) -> int:
    from session_state import store, merge, keying
    ap = argparse.ArgumentParser(prog="session-state")
    sub = ap.add_subparsers(dest="cmd", required=True)
    sub.add_parser("list")
    sp_show = sub.add_parser("show"); sp_show.add_argument("selector", nargs="?")
    sp_prune = sub.add_parser("prune"); sp_prune.add_argument("selector", nargs="?")
    args = ap.parse_args(argv)

    if args.cmd == "list":
        root = keying.claude_home() / "state" / "sessions"
        if root.is_dir():
            for d in sorted(root.iterdir()):
                flag = " [COLLISION]" if (d / "COLLISION").exists() else ""
                print(f"{d.name}{flag}")
        return 0
    if args.cmd == "show":
        dir = _resolve_dir(args.selector)
        print(merge.render_latest_md(store.read_timeline(dir, tail=50), store.read_scratches(dir)))
        return 0
    if args.cmd == "prune":
        store.prune(_resolve_dir(args.selector))
        print("pruned.")
        return 0
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
```

- [ ] **Step 4: Run tests to verify pass**

Run: `cd tools/session-state && uv run pytest tests/ -v`
Expected: PASS (full suite green).

- [ ] **Step 5: Commit**

```bash
git add tools/session-state/src/session_state/cli.py tools/session-state/tests/test_entries.py
git commit -m "feat(session-state): cli (list/show/prune)"
```

---

### Task 9: PowerShell installer (+ uninstall) and JSON-shape test

**Files:**
- Create: `tools/session-state/install.ps1`
- Test: `tools/session-state/tests/test_install_json.py` (validates the hook-entry JSON shape the installer produces, against a fixture settings file — never the real one)

**Interfaces:**
- Produces: `install.ps1` that (a) copies `src`/`pyproject.toml`/`tests` to `~/.claude/tools/session-state/`, (b) resolves an absolute `python.exe`, (c) inserts/updates three hook entries in `~/.claude/settings.json` keyed by basename marker, (d) supports `-Uninstall` and `-Purge`, `-PrintHooksOnly`, `-DryRun`.

- [ ] **Step 1: Write `install.ps1`**

```powershell
# install.ps1 — installer for the session-state hooks (Windows / PowerShell 7).
# Wires SessionStart(resume) + Stop(scratch) + SessionEnd(boundary) into ~/.claude/settings.json,
# invoking python.exe DIRECTLY (not uv run) for cold-start speed. Idempotent by basename marker.
#
#   pwsh -NoProfile -File install.ps1                  # install + wire hooks
#   pwsh -NoProfile -File install.ps1 -PrintHooksOnly  # print the hook JSON only
#   pwsh -NoProfile -File install.ps1 -Uninstall       # remove our 3 hook entries
#   pwsh -NoProfile -File install.ps1 -Uninstall -Purge# also delete ~/.claude/state/sessions
param(
    [switch]$PrintHooksOnly,
    [switch]$Uninstall,
    [switch]$Purge,
    [switch]$DryRun,
    [string]$InstallDir = (Join-Path $env:USERPROFILE ".claude\tools\session-state"),
    [string]$SettingsPath = (Join-Path $env:USERPROFILE ".claude\settings.json")
)
$ErrorActionPreference = "Stop"
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8

$markers = @("session-state/resume.py", "session-state/capture_scratch.py", "session-state/capture_end.py")

function Resolve-Python {
    $py = Get-Command python.exe -ErrorAction SilentlyContinue
    if (-not $py) { throw "python.exe not found on PATH." }
    return $py.Source
}

function New-Entries {
    param([string]$Py, [string]$Dir)
    $d = $Dir -replace '\\','/'
    return @{
        SessionStart = @(@{ hooks = @(@{ type="command"; command="`"$Py`" `"$d/src/session_state/resume.py`"" }) })
        Stop         = @(@{ hooks = @(@{ type="command"; command="`"$Py`" `"$d/src/session_state/capture_scratch.py`""; timeout=5 }) })
        SessionEnd   = @(@{ hooks = @(@{ type="command"; command="`"$Py`" `"$d/src/session_state/capture_end.py`"" }) })
    }
}

function Remove-OurEntries {
    param($Hooks)
    foreach ($evt in @("SessionStart","Stop","SessionEnd")) {
        if ($Hooks.$evt) {
            $Hooks.$evt = @($Hooks.$evt | Where-Object {
                $cmd = ($_.hooks | ForEach-Object { $_.command }) -join " "
                -not ($markers | Where-Object { $cmd -like "*$_*" })
            })
        }
    }
    return $Hooks
}

if ($PrintHooksOnly) {
    (New-Entries -Py (Resolve-Python) -Dir $InstallDir) | ConvertTo-Json -Depth 8
    return
}

# 1. copy files (skip on uninstall)
if (-not $Uninstall -and -not $DryRun) {
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    Copy-Item (Join-Path $PSScriptRoot "src") $InstallDir -Recurse -Force
    Copy-Item (Join-Path $PSScriptRoot "pyproject.toml") $InstallDir -Force
    if (Test-Path (Join-Path $PSScriptRoot "tests")) { Copy-Item (Join-Path $PSScriptRoot "tests") $InstallDir -Recurse -Force }
}

# 2. load settings
$settings = if (Test-Path $SettingsPath) { Get-Content $SettingsPath -Raw | ConvertFrom-Json -AsHashtable } else { @{} }
if (-not $settings.hooks) { $settings.hooks = @{} }

# 3. always strip our prior entries first (idempotent; replaces path-drifted ones)
$settings.hooks = Remove-OurEntries $settings.hooks

# 4. add fresh entries unless uninstalling
if (-not $Uninstall) {
    $entries = New-Entries -Py (Resolve-Python) -Dir $InstallDir
    foreach ($evt in $entries.Keys) {
        if (-not $settings.hooks.$evt) { $settings.hooks.$evt = @() }
        $settings.hooks.$evt = @($settings.hooks.$evt) + $entries.$evt
    }
}

$json = $settings | ConvertTo-Json -Depth 12
if ($DryRun) { Write-Host $json; return }
Set-Content -Path $SettingsPath -Value $json -Encoding UTF8
Write-Host "session-state hooks $((if($Uninstall){'removed'}else{'installed'}))."

if ($Uninstall -and $Purge) {
    $state = Join-Path $env:USERPROFILE ".claude\state\sessions"
    if (Test-Path $state) { Remove-Item $state -Recurse -Force; Write-Host "purged $state" }
}
```

- [ ] **Step 2: Write the failing JSON-shape test** in `tests/test_install_json.py`

```python
import json
import shutil
import subprocess
from pathlib import Path

import pytest

TOOL = Path(__file__).resolve().parents[1]


def _pwsh():
    return shutil.which("pwsh") or shutil.which("pwsh.exe")


@pytest.mark.skipif(_pwsh() is None, reason="pwsh not available")
def test_print_hooks_only_shape():
    out = subprocess.run([_pwsh(), "-NoProfile", "-File", str(TOOL / "install.ps1"), "-PrintHooksOnly"],
                         capture_output=True, text=True)
    data = json.loads(out.stdout)
    assert set(data) == {"SessionStart", "Stop", "SessionEnd"}
    stop = data["Stop"][0]["hooks"][0]
    assert stop["timeout"] == 5
    assert "capture_scratch.py" in stop["command"]


@pytest.mark.skipif(_pwsh() is None, reason="pwsh not available")
def test_idempotent_against_recall_fixture(tmp_path):
    # fixture settings.json already containing a recall.py SessionStart hook
    settings = tmp_path / "settings.json"
    settings.write_text(json.dumps({"hooks": {"SessionStart": [
        {"hooks": [{"type": "command", "command": '"python.exe" "C:/x/recall.py"'}]}]}}), encoding="utf-8")
    install = [_pwsh(), "-NoProfile", "-File", str(TOOL / "install.ps1"),
               "-SettingsPath", str(settings), "-InstallDir", str(tmp_path / "tool")]
    subprocess.run(install, capture_output=True, text=True)
    subprocess.run(install, capture_output=True, text=True)  # run twice
    data = json.loads(settings.read_text(encoding="utf-8"))
    starts = data["hooks"]["SessionStart"]
    # recall.py entry preserved; exactly ONE of ours (no duplicate on re-run)
    cmds = [h["command"] for e in starts for h in e["hooks"]]
    assert any("recall.py" in c for c in cmds)
    assert sum("resume.py" in c for c in cmds) == 1
```

- [ ] **Step 3: Run to verify failure**

Run: `cd tools/session-state && uv run pytest tests/test_install_json.py -v`
Expected: FAIL until `install.ps1` exists / behaves (or SKIP if no pwsh — but this is Windows, pwsh is present).

- [ ] **Step 4: Run to verify pass** (after Step 1 is in place)

Run: `cd tools/session-state && uv run pytest tests/test_install_json.py -v`
Expected: PASS (both shape + idempotency).

- [ ] **Step 5: Commit**

```bash
git add tools/session-state/install.ps1 tools/session-state/tests/test_install_json.py
git commit -m "feat(session-state): PowerShell installer (+uninstall) and JSON-shape tests"
```

---

### Task 10: `save-state` skill + rich-record producer guarantee

**Files:**
- Create: `.claude/skills/save-state/SKILL.md`
- Create: `tools/session-state/README.md`

**Interfaces:**
- Consumes: `capture_rich.py` (the `--input <tempfile>` contract).
- Produces: a `/save-state` skill that the agent invokes (and that `end-session` should call as its final step).

> **Producer guarantee (Spec §4/§7, round-3 #7):** `end-session` is a third-party plugin skill in the
> superpowers cache; forking it is fragile (clobbered on plugin update). So the guarantee is delivered
> two ways that don't depend on editing the plugin: (1) the `save-state` skill is the canonical
> producer the agent runs at a boundary; (2) the resume block already nudges
> "_(no narrative captured yet — run /save-state…)_" when a session ended without one. **If**
> `end-session` is found to be user-editable at execution time, add a final "invoke save-state" step
> to it; otherwise record the convention in this README and the project `CLAUDE.md`. Decide at
> execution and note which path was taken.

- [ ] **Step 1: Write `.claude/skills/save-state/SKILL.md`**

```markdown
---
name: save-state
description: Save the current dev-session's resumable state (what we did, next steps, open threads) to the per-repo session-state timeline so the next session can resume instantly. Use at the end of a work session, at a phase/spike boundary, or when the user says "save state", "checkpoint", or before ending a session.
---

# Save Session State

Append an agent-authored **rich** record to this repo's session-state timeline. Auto git facts are
already captured by hooks; this records the *meaning* — what got done, where we paused, what's next,
and which threads are open.

## Steps

1. Compose the narrative from the current session:
   - `did`: 1-3 sentences — what was accomplished and where work paused.
   - `next`: a list of concrete next actions.
   - `open_threads`: active bugs, blockers, pending decisions, things to watch.
2. Write it to a temp JSON file (use the OS temp dir; pick a unique name):
   ```json
   { "did": "...", "next": ["..."], "open_threads": ["..."] }
   ```
3. Run the capture script (direct python; path = the installed tool):
   ```
   python "%USERPROFILE%/.claude/tools/session-state/src/session_state/capture_rich.py" --input <tempfile>
   ```
   (Bash: `python "$HOME/.claude/tools/session-state/src/session_state/capture_rich.py" --input <tempfile>`)
4. Read the script's output:
   - "narrative saved." → done. The temp file was deleted for you.
   - "narrative NOT saved … Temp preserved at <path>" → tell the user; do **not** blind-retry. Surface
     the printed retry command.

## Notes
- This complements `end-session`/`handoff`; run it as the final step when ending a session.
- The next session's SessionStart hook will surface this automatically.
```

- [ ] **Step 2: Write `README.md`** (install, uninstall, kill-switch, producer convention)

```markdown
# session-state

Per-repo dev-session state capture + zero-friction resume for Claude Code. Spec:
`docs/superpowers/specs/2026-06-19-session-state-resume-design.md`.

## Install
```
pwsh -NoProfile -File tools/session-state/install.ps1
```
Wires three hooks into `~/.claude/settings.json` (SessionStart→resume, Stop→scratch,
SessionEnd→boundary), invoking `python.exe` directly. Re-running is idempotent.

## Uninstall
```
pwsh -NoProfile -File tools/session-state/install.ps1 -Uninstall          # remove hooks
pwsh -NoProfile -File tools/session-state/install.ps1 -Uninstall -Purge   # also delete state
```

## Disable temporarily
Set `CC_SESSION_STATE_DISABLE=1` in a shell to make all hooks no-op there.

## Inspect
```
python src/session_state/cli.py list
python src/session_state/cli.py show [<path-or-repo-key>]
python src/session_state/cli.py prune [<path-or-repo-key>]
```

## Rich records (the narrative)
Auto git facts are captured by hooks. The **narrative** (did/next/open_threads) is written by the
`/save-state` skill — run it at the end of a session or a phase boundary. `end-session` should call
it as its final step (see the skill).
```

- [ ] **Step 3: Manual verification (no unit test — this is docs + a skill)**

Run (from a temp file you create):
```
echo '{"did":"manual test","next":["x"],"open_threads":[]}' > "$TMP/rs.json"
python tools/session-state/src/session_state/capture_rich.py --input "$TMP/rs.json"
python tools/session-state/src/session_state/cli.py show
```
Expected: "narrative saved." then `show` renders "manual test". The temp file is gone.

- [ ] **Step 4: Commit**

```bash
git add .claude/skills/save-state/SKILL.md tools/session-state/README.md
git commit -m "feat(session-state): save-state skill + README + producer convention"
```

---

### Task 11: Full-suite green + install + manual acceptance

**Files:** none new (verification task).

- [ ] **Step 1: Run the entire test suite**

Run: `cd tools/session-state && uv run pytest tests/ -v`
Expected: ALL PASS (stdlib-guard, keying+parity, gitfacts, lock, store, merge, entries, cli, install-json).

- [ ] **Step 2: Install for real**

Run: `pwsh -NoProfile -File tools/session-state/install.ps1`
Expected: "session-state hooks installed." `~/.claude/settings.json` now has the three entries; the existing `recall.py` SessionStart hook is preserved.

- [ ] **Step 3: Manual acceptance — capture + resume**

In this repo: end a session (or simulate by piping `{"session_id":"t","reason":"other"}` to `capture_end.py`), then simulate a startup:
```
echo '{"source":"startup"}' | python "%USERPROFILE%/.claude/tools/session-state/src/session_state/resume.py"
```
Expected: a JSON envelope with a `<session-state>` block reflecting the latest merged state.

- [ ] **Step 4: Manual acceptance — concurrent worktree (Spec §10)**

Create a throwaway worktree of a test repo, write a scratch from a "session" on `main` and another on a feature branch, run `cli.py show`. Expected: freshest wins; a branch banner appears when the narrative and freshest facts differ; no cross-stomp (two scratch files coexist).

- [ ] **Step 5: Final commit (if any doc tweaks) + open PR**

```bash
git add -A
git commit -m "test(session-state): full-suite green + acceptance notes" || echo "nothing to commit"
```
Then open a PR from `feat/session-state-resume` into `main` (do not push to main directly).

---

## Self-Review

**1. Spec coverage:**
- §2 data model → Tasks 3 (git dict), 5 (`make_record`). ✓
- §3 storage layout, keying, meta guard → Tasks 2, 5. ✓
- §4 three triggers, throttle, kill-switch, single git call, producer → Tasks 7 (entries+throttle+kill-switch), 3 (single call), 10 (producer). ✓
- §5 branch-scoped merge + detached suppression → Task 6. ✓
- §6 components (all five + skill) → Tasks 6–8, 10. ✓
- §7 install/uninstall/kill-switch/coexistence → Task 9. ✓
- §8 retention/orphan cleanup → Task 5 (`prune`), wired in Task 7 (`capture_end`). ✓
- §9 concurrency/lock/detached/collision/encoding/git-unavailable → Tasks 4, 3, 2, 7. ✓
- §10 testing matrix → every task's tests + Task 11. ✓
- §11 relationships (vendoring, coexistence) → Tasks 2, 9. ✓

**2. Placeholder scan:** No TBD/TODO; every code step has complete code; the one cross-task ordering nuance (store→merge render) is called out explicitly with the resolution.

**3. Type consistency:** record shape from `store.make_record` is consumed unchanged by `merge.resolve_state`, `render_*`, the entries, and the cli; `state_dir`/`repo_root`/`repo_key` signatures are stable across keying consumers; `file_lock`/`LockTimeout` names match between `lock.py`, `store.append_record`, and `capture_rich`. `render_latest_md(timeline, scratches)` and `render_resume_block(timeline, scratches)` signatures match all call sites.

**Note on branch:** the spec was committed on `docs/session-state-resume-spec`. Implement on a `feat/session-state-resume` branch (Task 11 opens the PR).
