#!/usr/bin/env python3
"""Lane Z — apply the roadmap swarm's global-config contract requests.

DRY-RUN BY DEFAULT. Prints the plan and writes nothing unless you pass --apply.
Backs up settings.json and CLAUDE.md (timestamped) before any write.

Run from the repo root AFTER merging lane PRs #7 (cache-timer) and #10 (context-offload)
so the tools exist on `main`:

    python tools/lane-z-integration/deploy_globals.py                 # dry-run
    python tools/lane-z-integration/deploy_globals.py --apply
    python tools/lane-z-integration/deploy_globals.py --apply --adopt-new-cache-timer
    python tools/lane-z-integration/deploy_globals.py --apply --set-retries 10

See README.md in this directory for the full reconciliation (esp. the cache-timer conflict).
"""
from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
from datetime import datetime
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
DEFAULT_CONFIG_DIR = Path.home() / ".claude"

OLD_CACHE_DIR = "claude-cache-countdown"
NEW_CACHE_DIR = "cache-countdown"

# Tools to deploy into ~/.claude/tools/ : (repo path, dest name, run `uv sync`?)
TOOLS = [
    ("tools/cache-countdown", "cache-countdown", True),
    ("tools/context-offload", "context-offload", False),
]

CLAUDE_MD_HEADING = "## Budget-Discipline Standing Rules"
CLAUDE_MD_BLOCK_FILE = Path(__file__).resolve().parent / "claude-md-block.md"


def recall_command(config_dir: Path) -> str:
    recall = config_dir / "tools" / "context-offload" / "recall.py"
    return f'python "{recall}" --format hook'


class Planner:
    def __init__(self, apply: bool):
        self.apply = apply
        self.changes: list[str] = []
        self.warnings: list[str] = []

    def note(self, msg: str):
        self.changes.append(msg)

    def warn(self, msg: str):
        self.warnings.append(msg)


def backup(path: Path, p: Planner):
    if not path.exists():
        return
    stamp = datetime.now().strftime("%Y%m%d-%H%M%S")
    dest = path.with_suffix(path.suffix + f".bak-{stamp}")
    p.note(f"backup {path.name} -> {dest.name}")
    if p.apply:
        shutil.copy2(path, dest)


def deploy_tools(config_dir: Path, p: Planner):
    tools_root = config_dir / "tools"
    for rel, dest_name, do_sync in TOOLS:
        src = REPO / rel
        dest = tools_root / dest_name
        if not src.exists():
            p.warn(f"missing {rel} in repo — merge its PR first; skipping {dest_name}")
            continue
        p.note(f"copy {rel}  ->  {dest}")
        if p.apply:
            dest.parent.mkdir(parents=True, exist_ok=True)
            shutil.copytree(src, dest, dirs_exist_ok=True)
        if do_sync:
            p.note(f"uv sync in {dest}")
            if p.apply:
                if shutil.which("uv") is None:
                    p.warn(f"`uv` not found on PATH — run `uv sync` manually in {dest}")
                else:
                    subprocess.run(["uv", "sync"], cwd=dest, check=False)


def merge_settings(config_dir: Path, p: Planner, adopt_cache: bool, set_retries: int | None):
    path = config_dir / "settings.json"
    if not path.exists():
        p.warn(f"{path} does not exist — nothing to merge")
        return
    data = json.loads(path.read_text(encoding="utf-8"))
    hooks = data.setdefault("hooks", {})
    changed = False

    # --- Lane C: SessionStart memory-recall hook (additive, idempotent) ---
    cmd = recall_command(config_dir)
    sessionstart = hooks.get("SessionStart", [])
    already = any(
        h.get("command") == cmd
        for group in sessionstart
        for h in group.get("hooks", [])
    )
    if already:
        p.note("hooks.SessionStart: recall hook already present — no change")
    else:
        sessionstart.append({"matcher": "", "hooks": [{"type": "command", "command": cmd, "timeout": 5}]})
        hooks["SessionStart"] = sessionstart
        p.note(f"hooks.SessionStart += recall hook  ({cmd})")
        changed = True

    # --- Lane A1: opt-in repoint of Stop + UserPromptSubmit to the new cache-countdown ---
    if adopt_cache:
        for event in ("Stop", "UserPromptSubmit"):
            for group in hooks.get(event, []):
                for h in group.get("hooks", []):
                    c = h.get("command", "")
                    if OLD_CACHE_DIR in c:
                        h["command"] = c.replace(OLD_CACHE_DIR, NEW_CACHE_DIR)
                        p.note(f"hooks.{event}: repoint {OLD_CACHE_DIR} -> {NEW_CACHE_DIR}")
                        changed = True
        p.warn("statusLine still points at the OLD claude-cache-countdown install (kept on purpose) "
               "— do not delete that directory, or port a statusline.py first (README option 3).")
    else:
        p.note("cache-timer hooks: UNCHANGED (old install kept; pass --adopt-new-cache-timer to switch)")

    # --- Lane A2: opt-in retries value ---
    if set_retries is not None:
        env = data.setdefault("env", {})
        old = env.get("CLAUDE_CODE_MAX_RETRIES")
        if old == str(set_retries):
            p.note(f"env.CLAUDE_CODE_MAX_RETRIES already {set_retries} — no change")
        else:
            env["CLAUDE_CODE_MAX_RETRIES"] = str(set_retries)
            p.note(f"env.CLAUDE_CODE_MAX_RETRIES: {old} -> {set_retries}")
            changed = True
    else:
        cur = data.get("env", {}).get("CLAUDE_CODE_MAX_RETRIES", "(unset)")
        p.note(f"env.CLAUDE_CODE_MAX_RETRIES: UNCHANGED ({cur}; A2 suggests 10 via --set-retries 10)")

    if changed and p.apply:
        path.write_text(json.dumps(data, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
        # fail loudly if we somehow produced invalid JSON
        json.loads(path.read_text(encoding="utf-8"))


def append_claude_md(config_dir: Path, p: Planner):
    path = config_dir / "CLAUDE.md"
    block_doc = CLAUDE_MD_BLOCK_FILE.read_text(encoding="utf-8")
    # the file has a prose preamble then "---" then the real block; take everything after the first "---"
    block = block_doc.split("\n---\n", 1)[-1].strip() + "\n"
    if not path.exists():
        p.warn(f"{path} does not exist — would create it with the block")
        if p.apply:
            path.write_text(block, encoding="utf-8")
        return
    existing = path.read_text(encoding="utf-8")
    if CLAUDE_MD_HEADING in existing:
        p.note("CLAUDE.md: budget-discipline block already present — no change")
        return
    p.note(f"CLAUDE.md += '{CLAUDE_MD_HEADING}' block ({len(block.splitlines())} lines, appended)")
    if p.apply:
        sep = "" if existing.endswith("\n\n") else ("\n" if existing.endswith("\n") else "\n\n")
        path.write_text(existing + sep + block, encoding="utf-8")


def main():
    ap = argparse.ArgumentParser(description="Lane Z global-config deploy (dry-run by default).")
    ap.add_argument("--apply", action="store_true", help="actually write (otherwise dry-run)")
    ap.add_argument("--adopt-new-cache-timer", action="store_true",
                    help="repoint Stop/UserPromptSubmit to Lane A1's new cache-countdown (README option 2)")
    ap.add_argument("--set-retries", type=int, default=None, metavar="N",
                    help="set env.CLAUDE_CODE_MAX_RETRIES (A2 suggests 10)")
    ap.add_argument("--config-dir", type=Path, default=DEFAULT_CONFIG_DIR,
                    help="default: ~/.claude")
    args = ap.parse_args()

    p = Planner(apply=args.apply)
    cfg = args.config_dir.expanduser()

    print(f"{'APPLY' if args.apply else 'DRY-RUN'}  repo={REPO}  config={cfg}\n")

    backup(cfg / "settings.json", p)
    backup(cfg / "CLAUDE.md", p)
    deploy_tools(cfg, p)
    merge_settings(cfg, p, adopt_cache=args.adopt_new_cache_timer, set_retries=args.set_retries)
    append_claude_md(cfg, p)

    print("PLAN" + (" (applied)" if args.apply else " (nothing written — pass --apply)") + ":")
    for c in p.changes:
        print(f"  - {c}")
    if p.warnings:
        print("\nWARNINGS:")
        for w in p.warnings:
            print(f"  ! {w}")
    if not args.apply:
        print("\nRe-run with --apply to perform the above.")


if __name__ == "__main__":
    main()
