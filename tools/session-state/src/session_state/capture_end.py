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

# Make `src/` importable when this script is run directly by path (the hooks/CLI do this),
# since running a script by path only puts its own dir on sys.path, not the package parent.
_SRC = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if _SRC not in sys.path:
    sys.path.insert(0, _SRC)

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
