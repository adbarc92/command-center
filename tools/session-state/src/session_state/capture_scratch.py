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
