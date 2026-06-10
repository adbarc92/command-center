"""Unit tests for the pure core: parsing, remaining time, stages, cost, bells."""

from __future__ import annotations

import json
from datetime import datetime, timedelta, timezone

import pytest

from cache_countdown.core import (
    CACHE_TTL_SECONDS,
    COST_PER_TOKEN_AT_STAKE,
    ColorStage,
    TimerSnapshot,
    bells_crossed,
    cost_at_stake,
    format_line,
    read_timer_file,
    remaining_seconds,
    stage_for,
)

BASE = datetime(2026, 1, 1, 12, 0, 0, tzinfo=timezone.utc)


def snap(stopped=True, cached_tokens=None, ts=BASE):
    return TimerSnapshot(
        session_id="s",
        timestamp=ts,
        stopped=stopped,
        project="proj",
        cached_tokens=cached_tokens,
    )


def at(elapsed_s):
    return BASE + timedelta(seconds=elapsed_s)


# --- timestamp parsing (PowerShell `Get-Date -Format o`) ---------------------

def test_parses_powershell_7digit_fractional_offset(tmp_path):
    p = tmp_path / "cache-timer-x.json"
    p.write_text(json.dumps({
        "session_id": "x",
        "timestamp": "2026-06-09T11:59:09.6795938-06:00",
        "stopped": True,
        "project": "Disruption",
        "cwd": "D:\\proj",
    }), encoding="utf-8")
    s = read_timer_file(p)
    assert s.session_id == "x"
    assert s.stopped is True
    assert s.project == "Disruption"
    # -06:00 offset normalises to 17:59 UTC
    assert s.timestamp.astimezone(timezone.utc).hour == 17


def test_parses_z_suffix(tmp_path):
    p = tmp_path / "cache-timer-z.json"
    p.write_text(json.dumps({
        "session_id": "z",
        "timestamp": "2026-03-14T10:35:00.000Z",
        "stopped": False,
    }), encoding="utf-8")
    s = read_timer_file(p)
    assert s.timestamp.tzinfo is not None
    assert s.stopped is False


def test_missing_required_fields_raises(tmp_path):
    p = tmp_path / "bad.json"
    p.write_text(json.dumps({"stopped": True}), encoding="utf-8")
    with pytest.raises(ValueError):
        read_timer_file(p)


# --- remaining seconds -------------------------------------------------------

def test_remaining_full_at_start():
    assert remaining_seconds(snap(), at(0)) == CACHE_TTL_SECONDS


def test_remaining_clamps_at_zero():
    assert remaining_seconds(snap(), at(CACHE_TTL_SECONDS + 50)) == 0


def test_remaining_midway():
    assert remaining_seconds(snap(), at(95)) == CACHE_TTL_SECONDS - 95


# --- color stages: HOT -> GREEN -> YELLOW -> RED -> COLD ---------------------

def test_stage_hot_when_not_stopped():
    # Even with low remaining, an active (not-stopped) session is HOT.
    assert stage_for(snap(stopped=False), 5) is ColorStage.HOT


def test_stage_green_above_60():
    assert stage_for(snap(), 120) is ColorStage.GREEN
    assert stage_for(snap(), 61) is ColorStage.GREEN


def test_stage_yellow_30_to_60():
    assert stage_for(snap(), 59) is ColorStage.YELLOW
    assert stage_for(snap(), 30) is ColorStage.YELLOW


def test_stage_red_under_30():
    assert stage_for(snap(), 29) is ColorStage.RED
    assert stage_for(snap(), 1) is ColorStage.RED


def test_stage_cold_at_zero():
    assert stage_for(snap(), 0) is ColorStage.COLD


def test_full_stage_progression():
    s = snap()
    stages = [stage_for(s, remaining_seconds(s, at(e)))
              for e in (0, 240, 266, 290, 300)]
    assert stages == [
        ColorStage.GREEN,   # 295
        ColorStage.YELLOW,  # 55
        ColorStage.RED,     # 29
        ColorStage.RED,     # 5
        ColorStage.COLD,    # 0
    ]


# --- cost at stake -----------------------------------------------------------

def test_cost_none_when_tokens_unknown():
    assert cost_at_stake(snap(cached_tokens=None)) is None


def test_cost_basis_is_4_50_per_million():
    # $4.50 per 1M cached tokens (full input minus cache-read).
    assert cost_at_stake(snap(cached_tokens=1_000_000)) == pytest.approx(4.50)
    assert COST_PER_TOKEN_AT_STAKE == pytest.approx(0.0000045)


def test_spec_example_575():
    # ~1.28M tokens -> ~$5.75 (the spec's worked example).
    cost = cost_at_stake(snap(cached_tokens=1_277_778))
    assert cost == pytest.approx(5.75, abs=0.01)


# --- display line ------------------------------------------------------------

def test_format_red_with_cost():
    # 45s remaining is YELLOW (RED is <30s); the spec's "🔴 0:45 $5.75" example
    # mixes a red glyph with 0:45, but per the documented thresholds 45s is
    # yellow. Assert the documented stage + the cost rendering.
    line = format_line(snap(cached_tokens=1_277_778), 45)
    assert line.startswith("\U0001F7E1")  # yellow circle (45s > 30s)
    assert "0:45" in line
    assert "$5.75" in line


def test_format_red_under_30_with_cost():
    line = format_line(snap(cached_tokens=1_277_778), 20)
    assert line.startswith("\U0001F534")  # red circle (20s < 30s)
    assert "0:20" in line
    assert "$5.75" in line


def test_format_cold_has_no_cost():
    line = format_line(snap(cached_tokens=1_277_778), 0)
    assert "COLD" in line
    assert "$" not in line


def test_format_hot():
    line = format_line(snap(stopped=False, cached_tokens=1_000_000), 295)
    assert line.startswith("\U0001F525")  # fire
    assert "HOT" in line


def test_format_without_cost_when_unknown():
    line = format_line(snap(cached_tokens=None), 100)
    assert "$" not in line
    assert "1:40" in line


# --- bells: escalate at 60 / 30 / 10s ---------------------------------------

def test_bell_at_60():
    assert bells_crossed(prev_remaining=61, remaining=59, stopped=True) == 1


def test_bell_at_30():
    assert bells_crossed(prev_remaining=31, remaining=29, stopped=True) == 2


def test_bell_at_10():
    assert bells_crossed(prev_remaining=11, remaining=9, stopped=True) == 3


def test_no_bell_when_not_crossing():
    assert bells_crossed(prev_remaining=120, remaining=119, stopped=True) == 0


def test_no_bell_when_not_stopped():
    assert bells_crossed(prev_remaining=61, remaining=59, stopped=False) == 0


def test_lowest_threshold_wins_on_big_jump():
    # A tick that jumps past several thresholds rings the lowest one's count.
    assert bells_crossed(prev_remaining=65, remaining=5, stopped=True) == 3
