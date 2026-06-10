"""Tests for the tick loop: the synthetic clock should walk every color stage
and fire bells at the 60/30/10s thresholds."""

from __future__ import annotations

from datetime import datetime, timedelta, timezone

from cache_countdown.core import TimerSnapshot
from cache_countdown.ticker import run

BASE = datetime(2026, 1, 1, 12, 0, 0, tzinfo=timezone.utc)


def make_loop(script):
    """script: list of (elapsed_seconds, stopped). Returns (load, now, advance)."""
    idx = {"i": 0}

    def load():
        i = min(idx["i"], len(script) - 1)
        _, stopped = script[i]
        return TimerSnapshot(
            session_id="t", timestamp=BASE, stopped=stopped,
            project="t", cached_tokens=1_000_000,
        )

    def now():
        i = min(idx["i"], len(script) - 1)
        elapsed, _ = script[i]
        return BASE + timedelta(seconds=elapsed)

    def advance(_):
        idx["i"] += 1

    return load, now, advance


def test_full_run_walks_all_stages_and_rings_six_bells():
    # HOT, then GREEN -> YELLOW(60) -> RED(30) -> RED(10) -> COLD.
    script = [
        (0, False),    # HOT
        (0, True),     # 295 GREEN
        (236, True),   # 59  YELLOW, crosses 60 -> 1 bell
        (266, True),   # 29  RED,    crosses 30 -> 2 bells
        (286, True),   # 9   RED,    crosses 10 -> 3 bells
        (300, True),   # 0   COLD
    ]
    load, now, advance = make_loop(script)

    lines = []
    bells = []

    run(
        load,
        now_fn=now,
        sleep_fn=advance,
        interval=0,
        max_ticks=len(script),
        bell_fn=lambda c: bells.append(c),
        render_fn=lambda line: lines.append(line),
    )

    # Six lines rendered, one per scripted tick.
    assert len(lines) == 6
    # Stages visible in order.
    assert "\U0001F525" in lines[0]   # HOT fire
    assert "\U0001F7E2" in lines[1]   # GREEN
    assert "\U0001F7E1" in lines[2]   # YELLOW
    assert "\U0001F534" in lines[3]   # RED
    assert "\U0001F534" in lines[4]   # RED
    assert "COLD" in lines[5]
    # Bells: 1, 2, 3 — escalating at each threshold.
    assert bells == [1, 2, 3]


def test_no_timer_file_renders_idle_message():
    rendered = []
    rc = run(
        lambda: None,
        max_ticks=5,
        render_fn=lambda line: rendered.append(line),
        bell_fn=lambda c: None,
        sleep_fn=lambda _: None,
    )
    assert rc == 0
    assert rendered == ["(no active cache timer)"]


def test_resume_flips_hot_again():
    # stopped True -> the user resumes -> stopped False again => HOT.
    script = [(100, True), (0, False)]
    load, now, advance = make_loop(script)
    lines = []
    run(load, now_fn=now, sleep_fn=advance, interval=0, max_ticks=2,
        bell_fn=lambda c: None, render_fn=lambda l: lines.append(l))
    assert "\U0001F7E2" in lines[0] or "\U0001F7E1" in lines[0]  # was counting down
    assert "\U0001F525" in lines[1]  # HOT after resume
