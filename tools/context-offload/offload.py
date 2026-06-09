#!/usr/bin/env python
"""offload.py — Tier-1 context offload: write a durable project fact to local memory.

Roadmap item 3, Tier 1 (see docs/playbooks/context-offload.md).

Appends a single agent-derived durable fact (a decision, a gotcha, a "why") to the local
Claude Code project-memory store as a note file, and links it from MEMORY.md. This is the
*offload* half of the recall/offload pair: at a natural boundary (phase end, spike end) the
agent calls this to move stable context out of the active window so it isn't re-derived at
token cost next session — and is pulled back by `recall.py` at the next session start.

What counts as a durable fact (the memory-write discipline, enforced by convention here):
  - A DECISION and its rationale ("we chose X over Y because Z").
  - A GOTCHA / failure mode discovered the hard way (so it isn't rediscovered).
  - A "WHY" that isn't recoverable from the code/docs alone.
NOT durable: transient task state, anything already in the repo, doc duplication, secrets.
(See docs/playbooks/context-offload.md for the full rubric.)

Design constraints (lane brief):
  - Stdlib only, no network, headless-safe. ContextCurator (cc_* MCP) is a complementary
    *in-window* tier; this local store is always-available and independent of it.
  - Append-only & idempotent: re-running with the same --slug updates that one note and does
    not duplicate the index entry. Never rewrites unrelated notes.

Usage:
    python offload.py --title "..." --summary "..." [--slug ...] [--type project|reference]
                      [--body "..." | --body-file PATH] [--cwd PATH] [--session-id ID]
                      [--dry-run]
"""

from __future__ import annotations

import argparse
import datetime as _dt
import os
import re
import sys
from pathlib import Path

# Reuse the path/slug resolution from recall so both halves agree on where memory lives.
sys.path.insert(0, str(Path(__file__).resolve().parent))
from recall import claude_home, canonical_project_root, path_to_slug  # noqa: E402

for _stream in (sys.stdout, sys.stderr):
    try:
        _stream.reconfigure(encoding="utf-8")  # type: ignore[attr-defined]
    except Exception:
        pass


def slugify(title: str) -> str:
    s = title.lower().strip()
    s = re.sub(r"[^a-z0-9]+", "-", s).strip("-")
    return s or "note"


def resolve_memory_dir(cwd: Path) -> Path:
    """Return the canonical project's memory dir, matching an existing slug case-insensitively.

    Unlike recall (which only reads existing stores), offload may create the memory dir for a
    project that has none yet. We still collapse worktree paths to the canonical root so notes
    written from a worktree land in the shared project memory.
    """
    projects = claude_home() / "projects"
    canon = canonical_project_root(cwd)
    slug = path_to_slug(canon)
    if projects.is_dir():
        for p in projects.iterdir():
            if p.is_dir() and p.name.lower() == slug.lower():
                return p / "memory"
    return projects / slug / "memory"


def ensure_index(memory_dir: Path) -> Path:
    idx = memory_dir / "MEMORY.md"
    if not idx.is_file():
        idx.write_text("# Project Memory Index\n\n", encoding="utf-8")
    return idx


def write_note(memory_dir: Path, slug: str, title: str, summary: str, body: str,
               note_type: str, session_id: str) -> Path:
    note_path = memory_dir / f"{slug}.md"
    today = _dt.date.today().isoformat()
    if not body.strip():
        body = f"{summary}\n\nCaptured {today}."
    elif "captured" not in body.lower():
        body = body.rstrip() + f"\n\nCaptured {today}."
    front = (
        "---\n"
        f"name: {slug}\n"
        f'description: "{summary}"\n'
        "metadata: \n"
        "  node_type: memory\n"
        f"  type: {note_type}\n"
        f"  originSessionId: {session_id}\n"
        "---\n\n"
    )
    note_path.write_text(front + body.rstrip() + "\n", encoding="utf-8")
    return note_path


def upsert_index(idx: Path, slug: str, title: str, summary: str) -> str:
    text = idx.read_text(encoding="utf-8")
    entry = f"- [{title}]({slug}.md) — {summary}"
    line_re = re.compile(rf"^\s*[-*]\s*\[[^\]]+\]\(\s*{re.escape(slug)}\.md\s*\).*$", re.MULTILINE)
    if line_re.search(text):
        text = line_re.sub(entry, text)
        action = "updated"
    else:
        text = text.rstrip("\n") + "\n" + entry + "\n"
        action = "added"
    idx.write_text(text, encoding="utf-8")
    return action


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description="Offload a durable project fact to local memory.")
    ap.add_argument("--title", required=True)
    ap.add_argument("--summary", required=True, help="One-line index summary (the recall hook shows this).")
    ap.add_argument("--slug", default=None, help="Note filename stem (default: slugified title).")
    ap.add_argument("--type", choices=["project", "reference"], default="project")
    ap.add_argument("--body", default="")
    ap.add_argument("--body-file", default=None)
    ap.add_argument("--cwd", default=os.getcwd())
    ap.add_argument("--session-id", default=os.environ.get("CLAUDE_SESSION_ID", "unknown"))
    ap.add_argument("--dry-run", action="store_true")
    args = ap.parse_args(argv)

    body = args.body
    if args.body_file:
        body = Path(args.body_file).read_text(encoding="utf-8", errors="replace")

    slug = args.slug or slugify(args.title)
    memory_dir = resolve_memory_dir(Path(args.cwd).resolve())

    if args.dry_run:
        print(f"[dry-run] memory dir: {memory_dir}")
        print(f"[dry-run] note: {slug}.md")
        print(f"[dry-run] index entry: - [{args.title}]({slug}.md) — {args.summary}")
        return 0

    memory_dir.mkdir(parents=True, exist_ok=True)
    idx = ensure_index(memory_dir)
    note_path = write_note(memory_dir, slug, args.title, args.summary, body, args.type, args.session_id)
    action = upsert_index(idx, slug, args.title, args.summary)

    print(f"offloaded: {note_path}")
    print(f"index entry {action}: {idx}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
