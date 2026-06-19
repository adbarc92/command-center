import json
import pytest
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


@pytest.mark.xfail(reason="needs Task 6 merge.render_latest_md", strict=False)
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
