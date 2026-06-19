import session_state.merge as m


def _rec(ts, type, branch, did=None, detached=False, in_progress=None):
    g = {"branch": branch, "detached": detached, "in_progress": in_progress,
         "head": "abc x", "dirty": [], "worktree": None, "git_unavailable": False}
    r = {"ts": ts, "type": type, "source": "t", "session_id": "s", "repo": "D:/r", "git": g}
    if type == "rich":
        r["did"] = did or "did"
        r["next"] = ["n1"]
        r["open_threads"] = ["t1"]
    return r


def test_freshest_git_from_scratch_over_older_timeline():
    timeline = [_rec("2026-01-01T00:00:00Z", "auto", "main")]
    scratch = [_rec("2026-02-01T00:00:00Z", "auto", "feat/x")]
    state = m.resolve_state(timeline, scratch)
    assert state["git"]["branch"] == "feat/x"


def test_narrative_is_newest_rich():
    timeline = [_rec("2026-01-01T00:00:00Z", "rich", "main", did="old"),
                _rec("2026-02-01T00:00:00Z", "rich", "main", did="new")]
    state = m.resolve_state(timeline, [])
    assert state["narrative"]["did"] == "new"


def test_branch_banner_when_branches_differ():
    timeline = [_rec("2026-01-01T00:00:00Z", "rich", "feat/x")]
    scratch = [_rec("2026-02-01T00:00:00Z", "auto", "main")]
    state = m.resolve_state(timeline, scratch)
    assert state["branch_banner"] is not None and "feat/x" in state["branch_banner"]


def test_no_banner_when_detached():
    timeline = [_rec("2026-01-01T00:00:00Z", "rich", "feat/x")]
    scratch = [_rec("2026-02-01T00:00:00Z", "auto", None, detached=True)]
    state = m.resolve_state(timeline, scratch)
    assert state["branch_banner"] is None


def test_resume_block_none_when_empty():
    assert m.render_resume_block([], []) is None


def test_resume_block_contains_next_and_threads():
    timeline = [_rec("2026-02-01T00:00:00Z", "rich", "main", did="shipped X")]
    block = m.render_resume_block(timeline, [])
    assert "shipped X" in block and "n1" in block and "t1" in block
