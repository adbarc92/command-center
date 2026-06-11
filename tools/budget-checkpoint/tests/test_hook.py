"""Tests for the stdin->stdout hook envelope."""

from __future__ import annotations

import json

from budget_checkpoint.core import DEFAULT_TURN_THRESHOLD
from budget_checkpoint.hook import build_output, run


def _event(transcript_path, stop_hook_active=False):
    return json.dumps({
        "session_id": "sess-1",
        "transcript_path": transcript_path,
        "stop_hook_active": stop_hook_active,
        "hook_event_name": "Stop",
    })


def _write_transcript(tmp_path, n_turns):
    t = tmp_path / "transcript.jsonl"
    rows = []
    for i in range(n_turns):
        kind = "user" if i % 2 == 0 else "assistant"
        rows.append(json.dumps({"type": kind, "message": f"m{i}"}))
    t.write_text("\n".join(rows), encoding="utf-8")
    return t


def test_build_output_uses_additionalcontext_not_block():
    out = build_output("hello")
    # Gentlest mechanism: additionalContext, never decision/block.
    assert out == {
        "hookSpecificOutput": {
            "hookEventName": "Stop",
            "additionalContext": "hello",
        }
    }
    assert "decision" not in out


def test_run_emits_nudge_for_long_transcript(tmp_path):
    t = _write_transcript(tmp_path, DEFAULT_TURN_THRESHOLD)
    out = run(_event(str(t)))
    assert out is not None
    ctx = out["hookSpecificOutput"]["additionalContext"]
    assert "/end-session" in ctx
    assert "/handoff" in ctx
    assert str(DEFAULT_TURN_THRESHOLD) in ctx


def test_run_silent_for_short_transcript(tmp_path):
    t = _write_transcript(tmp_path, 3)
    assert run(_event(str(t))) is None


def test_run_silent_when_stop_hook_active(tmp_path):
    t = _write_transcript(tmp_path, DEFAULT_TURN_THRESHOLD + 50)
    assert run(_event(str(t), stop_hook_active=True)) is None


def test_run_silent_on_missing_transcript(tmp_path):
    assert run(_event(str(tmp_path / "nope.jsonl"))) is None


def test_run_silent_on_empty_stdin():
    assert run("") is None
    assert run("   \n") is None


def test_run_silent_on_malformed_json():
    assert run("{not valid json") is None


def test_run_silent_on_non_object_json():
    assert run("[1, 2, 3]") is None


def test_run_respects_custom_threshold(tmp_path):
    t = _write_transcript(tmp_path, 10)
    # default threshold (60) -> silent; custom threshold 10 -> nudge
    assert run(_event(str(t))) is None
    assert run(_event(str(t)), threshold=10) is not None
