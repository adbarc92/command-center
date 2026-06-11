"""Tests for the pure boundary-detection logic."""

from __future__ import annotations

import json

from budget_checkpoint.core import (
    DEFAULT_TURN_THRESHOLD,
    NUDGE_EVERY,
    StopEvent,
    count_turns,
    evaluate,
    nudge_text,
    parse_event,
    should_nudge,
)


# --- parse_event -------------------------------------------------------------

def test_parse_event_reads_all_fields():
    ev = parse_event({
        "session_id": "abc",
        "transcript_path": "/tmp/t.jsonl",
        "stop_hook_active": True,
        "cwd": "/work",
    })
    assert ev == StopEvent("abc", "/tmp/t.jsonl", True, "/work")


def test_parse_event_tolerates_missing_fields():
    ev = parse_event({})
    assert ev.session_id == ""
    assert ev.transcript_path is None
    assert ev.stop_hook_active is False
    assert ev.cwd is None


# --- count_turns -------------------------------------------------------------

def test_count_turns_counts_user_and_assistant_lines(tmp_path):
    t = tmp_path / "t.jsonl"
    lines = [
        {"type": "user", "message": "hi"},
        {"type": "assistant", "message": "hello"},
        {"type": "summary", "summary": "ignored"},   # not a turn
        {"type": "user", "message": "again"},
    ]
    t.write_text("\n".join(json.dumps(x) for x in lines), encoding="utf-8")
    assert count_turns(str(t)) == 3


def test_count_turns_skips_blank_and_malformed_lines(tmp_path):
    t = tmp_path / "t.jsonl"
    t.write_text(
        '{"type":"user"}\n'
        "\n"                       # blank
        "not json at all\n"        # malformed -> skipped, not fatal
        '{"type":"assistant"}\n',
        encoding="utf-8",
    )
    assert count_turns(str(t)) == 2


def test_count_turns_missing_file_returns_zero(tmp_path):
    assert count_turns(str(tmp_path / "nope.jsonl")) == 0


def test_count_turns_none_path_returns_zero():
    assert count_turns(None) == 0


# --- should_nudge ------------------------------------------------------------

def test_no_nudge_below_threshold():
    assert should_nudge(DEFAULT_TURN_THRESHOLD - 1, stop_hook_active=False) is False


def test_nudge_exactly_at_threshold():
    assert should_nudge(DEFAULT_TURN_THRESHOLD, stop_hook_active=False) is True


def test_no_nudge_when_stop_hook_already_active():
    # Even well past the threshold, an active Stop hook must not re-nudge.
    assert should_nudge(DEFAULT_TURN_THRESHOLD + 5, stop_hook_active=True) is False


def test_renudges_only_every_nth_turn_after_threshold():
    base = DEFAULT_TURN_THRESHOLD
    assert should_nudge(base, stop_hook_active=False) is True
    assert should_nudge(base + 1, stop_hook_active=False) is False
    assert should_nudge(base + NUDGE_EVERY, stop_hook_active=False) is True
    assert should_nudge(base + NUDGE_EVERY + 1, stop_hook_active=False) is False


def test_custom_threshold_and_interval():
    assert should_nudge(10, stop_hook_active=False, threshold=10, nudge_every=5) is True
    assert should_nudge(11, stop_hook_active=False, threshold=10, nudge_every=5) is False
    assert should_nudge(15, stop_hook_active=False, threshold=10, nudge_every=5) is True


# --- nudge_text --------------------------------------------------------------

def test_nudge_text_mentions_both_routes_and_turn_count():
    txt = nudge_text(72)
    assert "72" in txt
    assert "/end-session" in txt
    assert "/handoff" in txt


# --- evaluate (with injected counter) ----------------------------------------

def test_evaluate_returns_nudge_when_boundary_crossed():
    ev = StopEvent("s", "/anything", stop_hook_active=False)
    out = evaluate(ev, turn_counter=lambda _: DEFAULT_TURN_THRESHOLD)
    assert out is not None
    assert "/end-session" in out


def test_evaluate_returns_none_below_threshold():
    ev = StopEvent("s", "/anything", stop_hook_active=False)
    out = evaluate(ev, turn_counter=lambda _: 3)
    assert out is None


def test_evaluate_returns_none_when_stop_hook_active():
    ev = StopEvent("s", "/anything", stop_hook_active=True)
    out = evaluate(ev, turn_counter=lambda _: DEFAULT_TURN_THRESHOLD + 100)
    assert out is None
