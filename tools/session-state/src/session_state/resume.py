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

# Make `src/` importable when this script is run directly by path (the hooks/CLI do this),
# since running a script by path only puts its own dir on sys.path, not the package parent.
_SRC = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if _SRC not in sys.path:
    sys.path.insert(0, _SRC)

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
        dir = keying.state_dir(cwd, create=False)  # read-only path; never writes
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
