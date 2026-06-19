import io
import json
import subprocess
import sys
from pathlib import Path

import session_state.capture_scratch as cs
import session_state.capture_end as ce
import session_state.capture_rich as cr
import session_state.resume as rz


def _init_git_repo(tmp_path):
    subprocess.run(["git", "init", "-q"], cwd=tmp_path)
    subprocess.run(["git", "config", "user.email", "t@t"], cwd=tmp_path)
    subprocess.run(["git", "config", "user.name", "t"], cwd=tmp_path)
    (tmp_path / "f.txt").write_text("x", encoding="utf-8")
    subprocess.run(["git", "add", "."], cwd=tmp_path)
    subprocess.run(["git", "commit", "-qm", "init"], cwd=tmp_path)


def test_capture_scratch_writes(tmp_path, monkeypatch, capsys):
    monkeypatch.setenv("CLAUDE_CONFIG_DIR", str(tmp_path / ".claude"))
    _init_git_repo(tmp_path)
    monkeypatch.chdir(tmp_path)
    monkeypatch.setattr("sys.stdin", io.StringIO(json.dumps({"session_id": "sid1"})))
    assert cs.main() == 0
    import session_state.keying as k
    import session_state.store as s
    scratches = s.read_scratches(k.state_dir(tmp_path))
    assert len(scratches) == 1


def test_kill_switch_noops(tmp_path, monkeypatch):
    monkeypatch.setenv("CC_SESSION_STATE_DISABLE", "1")
    monkeypatch.setattr("sys.stdin", io.StringIO("{}"))
    assert cs.main() == 0  # and writes nothing


def test_capture_end_skips_clear(tmp_path, monkeypatch):
    monkeypatch.setenv("CLAUDE_CONFIG_DIR", str(tmp_path / ".claude"))
    _init_git_repo(tmp_path)
    monkeypatch.chdir(tmp_path)
    monkeypatch.setattr("sys.stdin", io.StringIO(json.dumps({"session_id": "s", "reason": "clear"})))
    assert ce.main() == 0
    import session_state.keying as k, session_state.store as st
    assert st.read_timeline(k.state_dir(tmp_path)) == []


def test_resume_silent_on_compact(tmp_path, monkeypatch, capsys):
    monkeypatch.setattr("sys.stdin", io.StringIO(json.dumps({"source": "compact"})))
    assert rz.main() == 0
    assert capsys.readouterr().out.strip() == ""


def test_kill_switch_noops_all_entries(tmp_path, monkeypatch):
    import io, json
    monkeypatch.setenv("CC_SESSION_STATE_DISABLE", "1")
    monkeypatch.setattr("sys.stdin", io.StringIO(json.dumps({"session_id": "s", "reason": "other"})))
    assert ce.main() == 0
    monkeypatch.setattr("sys.stdin", io.StringIO(json.dumps({"source": "startup"})))
    assert rz.main() == 0
    # capture_rich no-ops without even requiring --input
    assert cr.main([]) == 0


def test_resume_does_not_create_state_dir(tmp_path, monkeypatch):
    import io, json
    monkeypatch.setenv("CLAUDE_CONFIG_DIR", str(tmp_path / ".claude"))
    monkeypatch.chdir(tmp_path)
    monkeypatch.setattr("sys.stdin", io.StringIO(json.dumps({"source": "startup"})))
    assert rz.main() == 0
    import session_state.keying as k
    # state dir should NOT have been created by the read path
    assert not (k.state_dir(tmp_path, create=False)).exists()


def test_capture_rich_appends_and_deletes_tempfile(tmp_path, monkeypatch):
    monkeypatch.setenv("CLAUDE_CONFIG_DIR", str(tmp_path / ".claude"))
    _init_git_repo(tmp_path)
    monkeypatch.chdir(tmp_path)
    payload = tmp_path / "rich.json"
    payload.write_text(json.dumps({"did": "shipped", "next": ["a"], "open_threads": []}), encoding="utf-8")
    assert cr.main(["--input", str(payload)]) == 0
    assert not payload.exists()
    import session_state.keying as k, session_state.store as st
    recs = st.read_timeline(k.state_dir(tmp_path))
    assert recs and recs[-1]["did"] == "shipped"


import session_state.cli as cli


def test_cli_show_renders_state(tmp_path, monkeypatch, capsys):
    monkeypatch.setenv("CLAUDE_CONFIG_DIR", str(tmp_path / ".claude"))
    _init_git_repo(tmp_path)
    monkeypatch.chdir(tmp_path)
    import session_state.keying as k, session_state.store as st
    dir = k.state_dir(tmp_path)
    st.append_record(dir, st.make_record("rich", "save-state", "s", str(tmp_path),
                                         {"branch": "main", "head": "abc x", "dirty": []},
                                         did="hello", next=[], open_threads=[]))
    assert cli.main(["show"]) == 0
    assert "hello" in capsys.readouterr().out


def test_cli_list_shows_repo(tmp_path, monkeypatch, capsys):
    monkeypatch.setenv("CLAUDE_CONFIG_DIR", str(tmp_path / ".claude"))
    _init_git_repo(tmp_path)
    monkeypatch.chdir(tmp_path)
    import session_state.keying as k
    k.state_dir(tmp_path)  # create the dir
    assert cli.main(["list"]) == 0
