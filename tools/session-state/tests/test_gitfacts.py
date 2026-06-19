from pathlib import Path
import session_state.gitfacts as g

CLEAN = "# branch.oid abc123\n# branch.head main\n# branch.upstream origin/main\n# branch.ab +0 -0\n"
DIRTY = (
    "# branch.oid abc123\n# branch.head main\n"
    "1 .M N... 100644 100644 100644 aaa bbb src/foo.rs\n"
    "2 R. N... 100644 100644 100644 ccc ddd R100 new.rs\told.rs\n"
    "? untracked.txt\n"
)
DETACHED = "# branch.oid abc123\n# branch.head (detached)\n1 .M N... 100644 100644 100644 a b x.rs\n"
CONFLICT = (
    "# branch.oid abc123\n# branch.head (detached)\n"
    "u UU N... 100644 100644 100644 100644 a b c conflicted.rs\n"
)


def test_parse_clean():
    r = g.parse_porcelain_v2(CLEAN)
    assert r == {"branch": "main", "detached": False, "dirty": []}


def test_parse_dirty_includes_renamed_and_untracked():
    r = g.parse_porcelain_v2(DIRTY)
    assert r["branch"] == "main" and r["detached"] is False
    assert "src/foo.rs" in r["dirty"]
    assert "new.rs" in r["dirty"]        # rename: new path
    assert "untracked.txt" in r["dirty"]


def test_parse_detached_branch_is_none():
    r = g.parse_porcelain_v2(DETACHED)
    assert r["branch"] is None and r["detached"] is True
    assert r["dirty"] == ["x.rs"]


def test_parse_conflict_unmerged_record():
    r = g.parse_porcelain_v2(CONFLICT)
    assert r["detached"] is True
    assert "conflicted.rs" in r["dirty"]


def test_in_progress_detects_rebase(tmp_path):
    (tmp_path / "rebase-merge").mkdir()
    assert g.in_progress(tmp_path) == "rebase"


def test_in_progress_none(tmp_path):
    assert g.in_progress(tmp_path) is None


def test_collect_git_facts_non_git(tmp_path):
    assert g.collect_git_facts(tmp_path) is None
