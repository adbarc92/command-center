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
