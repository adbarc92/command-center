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

# Make `src/` importable when this script is run directly by path (the hooks/CLI do this),
# since running a script by path only puts its own dir on sys.path, not the package parent.
_SRC = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if _SRC not in sys.path:
    sys.path.insert(0, _SRC)


def main(argv: list[str] | None = None) -> int:
    if os.environ.get("CC_SESSION_STATE_DISABLE"):
        return 0
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
