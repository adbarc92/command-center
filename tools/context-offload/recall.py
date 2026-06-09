#!/usr/bin/env python
"""recall.py — Tier-1 context offload: auto-recall durable project facts at session start.

Roadmap item 3, Tier 1 (see docs/playbooks/context-offload.md).

Reads the local Claude Code project-memory store
(`~/.claude/projects/<slug>/memory/MEMORY.md` + note files) and prints the durable-fact
index to stdout. A `SessionStart` hook pipes this stdout into the model's context so stable
facts surface *without the user re-stating them*.

Design constraints (from the lane brief):
- Headless-safe: NO network, NO MCP calls, NO dependencies beyond the stdlib. ContextCurator
  (the cc_* MCP) is a complementary in-window tier and may be absent; this script is the
  always-available local fallback and never blocks on a tool that may not be connected.
- Read-only: the memory store is append-only, written by normal operation, not by this script.
- Worktree-aware: a git worktree has its own path (and so its own naming slug), but its durable
  facts live in the *canonical* project's memory. We resolve `<root>/.claude/worktrees/<x>`
  back to `<root>` before computing the slug, so recall works identically inside a worktree.

Exit code is always 0 (even when no memory exists) so a SessionStart hook never fails the session.

Usage:
    python recall.py [--cwd PATH] [--format hook|plain|json] [--max-notes N]

    --cwd       Project directory to recall for. Defaults to the current working directory.
    --format    hook  (default) wrap output in <session-memory> for context injection;
                plain emit the raw index; json emit a machine-readable object.
    --max-notes Cap how many note summaries are emitted (default: all). Keeps the
                session-start injection small — full notes are recalled on demand by the agent.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
from pathlib import Path


# Emit UTF-8 regardless of the host console codepage (Windows defaults to cp1252, which
# mangles the em-dashes used in memory summaries). A SessionStart hook captures stdout as
# bytes, so encoding must be deterministic.
for _stream in (sys.stdout, sys.stderr):
    try:
        _stream.reconfigure(encoding="utf-8")  # type: ignore[attr-defined]
    except Exception:
        pass


def claude_home() -> Path:
    """Locate ~/.claude, honouring CLAUDE_CONFIG_DIR if set."""
    override = os.environ.get("CLAUDE_CONFIG_DIR")
    if override:
        return Path(override)
    return Path.home() / ".claude"


def canonical_project_root(cwd: Path) -> Path:
    """Map a path to the canonical project root.

    A git worktree created under `<root>/.claude/worktrees/<name>` is the same logical
    project as `<root>` and shares its memory. Collapse that suffix so recall inside a
    worktree resolves to the parent project's memory store rather than a per-worktree one.
    """
    parts = cwd.parts
    # Look for the '.claude/worktrees/<name>' tail and strip it.
    for i in range(len(parts) - 1):
        if parts[i] == ".claude" and i + 1 < len(parts) and parts[i + 1] == "worktrees":
            return Path(*parts[:i])
    return cwd


def path_to_slug(path: Path) -> str:
    """Reproduce Claude Code's project-directory slug.

    Claude names `~/.claude/projects/<slug>` by replacing every path separator and drive
    colon with '-'. Drive-letter case is preserved as the OS reports it. e.g.
    `D:\\MajorProjects\\CURRENT\\command-center` -> `D--MajorProjects-CURRENT-command-center`.
    """
    s = str(path)
    s = s.replace("\\", "-").replace("/", "-").replace(":", "-")
    return s


def find_memory_dir(cwd: Path) -> Path | None:
    """Return the memory dir for cwd's project, or None if absent.

    Tries the worktree-collapsed canonical slug first, then the raw cwd slug as a fallback
    (covers projects whose memory was created directly inside a worktree). The on-disk slug
    casing is matched case-insensitively because the same path can appear as both 'D--' and
    'd--' depending on how the drive letter was reported when the dir was first created.
    """
    projects = claude_home() / "projects"
    if not projects.is_dir():
        return None

    candidates = [path_to_slug(canonical_project_root(cwd)), path_to_slug(cwd)]
    existing = {p.name.lower(): p for p in projects.iterdir() if p.is_dir()}
    for slug in candidates:
        hit = existing.get(slug.lower())
        if hit and (hit / "memory" / "MEMORY.md").is_file():
            return hit / "memory"
    return None


def strip_frontmatter(text: str) -> tuple[dict, str]:
    """Split simple `--- ... ---` YAML-ish frontmatter from a note body.

    Only the `name` and `description` keys are read (no YAML dep). Returns (meta, body).
    """
    meta: dict[str, str] = {}
    if not text.startswith("---"):
        return meta, text
    end = text.find("\n---", 3)
    if end == -1:
        return meta, text
    header = text[3:end]
    body = text[end + 4 :].lstrip("\n")
    for line in header.splitlines():
        m = re.match(r"\s*(name|description)\s*:\s*(.+?)\s*$", line)
        if m:
            meta[m.group(1)] = m.group(2).strip().strip('"').strip("'")
    return meta, body


def parse_index(memory_dir: Path, max_notes: int | None) -> list[dict]:
    """Parse MEMORY.md's index list into note records, enriched from each note's frontmatter."""
    index = (memory_dir / "MEMORY.md").read_text(encoding="utf-8", errors="replace")
    notes: list[dict] = []
    # Index lines look like: - [Title](file.md) — one-line summary
    link_re = re.compile(r"^\s*[-*]\s*\[(?P<title>[^\]]+)\]\((?P<file>[^)]+)\)\s*(?:[—–-]\s*(?P<summary>.+))?$")
    for line in index.splitlines():
        m = link_re.match(line)
        if not m:
            continue
        rec = {
            "title": m.group("title").strip(),
            "file": m.group("file").strip(),
            "summary": (m.group("summary") or "").strip(),
        }
        note_path = memory_dir / rec["file"]
        if note_path.is_file():
            meta, _ = strip_frontmatter(note_path.read_text(encoding="utf-8", errors="replace"))
            if not rec["summary"] and meta.get("description"):
                rec["summary"] = meta["description"]
        notes.append(rec)
        if max_notes is not None and len(notes) >= max_notes:
            break
    return notes


def render_hook(memory_dir: Path, notes: list[dict]) -> str:
    lines = [
        "<session-memory>",
        "Durable facts recalled from this project's local memory store "
        "(Tier-1 context offload). These are agent-derived decisions, gotchas, and \"why\" "
        "— treat as known; recall the full note only when a task needs its detail.",
        f"Store: {memory_dir}",
        "",
    ]
    for n in notes:
        summary = f" — {n['summary']}" if n["summary"] else ""
        lines.append(f"- {n['title']} ({n['file']}){summary}")
    lines.append("</session-memory>")
    return "\n".join(lines)


def render_plain(memory_dir: Path, notes: list[dict]) -> str:
    out = [f"# Recalled durable facts ({len(notes)}) — {memory_dir}", ""]
    for n in notes:
        summary = f" — {n['summary']}" if n["summary"] else ""
        out.append(f"- {n['title']} ({n['file']}){summary}")
    return "\n".join(out)


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description="Recall Tier-1 durable project facts at session start.")
    ap.add_argument("--cwd", default=os.getcwd(), help="Project dir (default: current dir).")
    ap.add_argument("--format", choices=["hook", "plain", "json"], default="hook")
    ap.add_argument("--max-notes", type=int, default=None)
    args = ap.parse_args(argv)

    try:
        memory_dir = find_memory_dir(Path(args.cwd).resolve())
    except Exception as exc:  # never let recall fail a session
        print(f"<session-memory>recall unavailable: {exc}</session-memory>" if args.format == "hook" else f"recall unavailable: {exc}", file=sys.stderr)
        return 0

    if memory_dir is None:
        if args.format == "json":
            print(json.dumps({"memory_dir": None, "notes": []}))
        # hook/plain: emit nothing so a fresh project's session start stays clean.
        return 0

    try:
        notes = parse_index(memory_dir, args.max_notes)
    except Exception as exc:
        if args.format == "json":
            print(json.dumps({"memory_dir": str(memory_dir), "error": str(exc), "notes": []}))
        return 0

    if args.format == "json":
        print(json.dumps({"memory_dir": str(memory_dir), "notes": notes}, indent=2))
    elif args.format == "plain":
        print(render_plain(memory_dir, notes))
    else:
        if notes:
            print(render_hook(memory_dir, notes))
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
