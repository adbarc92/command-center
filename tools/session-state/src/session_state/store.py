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
