"""The ticker: reads a cache-timer state file once per second, renders the
countdown line, and rings escalating bells at the 60/30/10s thresholds.

Run via UV:

    uv run cache-countdown --session test
    uv run cache-countdown --file ~/.claude/state/cache-timer-<id>.json

Self-test mode (no real session needed — drives a synthetic clock so you can
watch every color stage and hear every bell in a few seconds):

    uv run cache-countdown --self-test
"""

from __future__ import annotations

import argparse
import sys
import time

# Windows consoles default to a legacy codepage (cp1252) that can't encode the
# countdown emoji. Force UTF-8 on the std streams so 🔥/🟢/🟡/🔴/❄️ render.
for _stream in (sys.stdout, sys.stderr):
    _reconf = getattr(_stream, "reconfigure", None)
    if _reconf is not None:
        try:
            _reconf(encoding="utf-8")
        except (ValueError, OSError):
            pass
from datetime import datetime, timezone
from pathlib import Path
from typing import Optional

from .core import (
    BELL_THRESHOLDS,
    CACHE_TTL_SECONDS,
    ColorStage,
    TimerSnapshot,
    bells_crossed,
    format_line,
    read_timer_file,
    remaining_seconds,
    stage_for,
)


def state_dir() -> Path:
    home = Path.home()
    return home / ".claude" / "state"


def timer_path_for_session(session: str) -> Path:
    return state_dir() / f"cache-timer-{session}.json"


def ring_bell(count: int, stream=sys.stderr) -> None:
    """Emit `count` terminal bells (BEL, 0x07). No-op for count <= 0."""
    if count <= 0:
        return
    stream.write("\a" * count)
    stream.flush()


def render(line: str, stream=sys.stdout) -> None:
    """Overwrite the current terminal line with `line`."""
    stream.write("\r\033[K" + line)
    stream.flush()


def run(
    load_snapshot,
    *,
    now_fn=lambda: datetime.now(timezone.utc),
    sleep_fn=time.sleep,
    interval: float = 1.0,
    max_ticks: Optional[int] = None,
    bell_fn=ring_bell,
    render_fn=render,
    stop_when_cold: bool = False,
) -> int:
    """Core tick loop. Injectable clock/sleep/bell/render for testing.

    `load_snapshot()` returns a fresh TimerSnapshot each tick (so a resumed
    session — stopped flips back to False — is picked up live), or None if the
    file vanished.
    """
    prev_remaining = CACHE_TTL_SECONDS + 1  # so the first 60s bell can fire
    ticks = 0
    while max_ticks is None or ticks < max_ticks:
        snap = load_snapshot()
        if snap is None:
            render_fn("(no active cache timer)")
            return 0
        now = now_fn()
        remaining = remaining_seconds(snap, now)
        stage = stage_for(snap, remaining)

        bells = bells_crossed(prev_remaining, remaining, snap.stopped)
        if bells:
            bell_fn(bells)

        render_fn(format_line(snap, remaining))

        prev_remaining = remaining
        ticks += 1

        if stop_when_cold and stage is ColorStage.COLD:
            return 0
        if max_ticks is not None and ticks >= max_ticks:
            break
        sleep_fn(interval)
    return 0


def _self_test(interval: float = 0.05) -> int:
    """Drive a synthetic clock from HOT through COLD so the operator can watch
    the color stages and hear the 60/30/10s bells without a real session."""
    base = datetime(2026, 1, 1, 12, 0, 0, tzinfo=timezone.utc)
    # Tokens chosen so cost-at-stake reads ~ $5.75 (matches the spec example):
    #   5.75 / 0.0000045 ≈ 1,277,778 cached tokens.
    cached = 1_277_778

    # A scripted sequence of (seconds_elapsed, stopped) the synthetic clock
    # steps through. First tick is HOT (not stopped), then it Stops and the
    # countdown runs through each threshold.
    script = [(0, False)]  # HOT
    script += [(e, True) for e in (
        0,    # 295 GREEN
        236,  # 59  YELLOW  (crosses 60 bell)
        266,  # 29  RED     (crosses 30 bell)
        286,  # 9   RED     (crosses 10 bell)
        300,  # 0   COLD
    )]

    idx = {"i": 0}

    def load():
        i = min(idx["i"], len(script) - 1)
        _, stopped = script[i]
        return TimerSnapshot(
            session_id="self-test",
            timestamp=base,  # fixed; elapsed comes from now_fn below
            stopped=stopped,
            project="self-test",
            cached_tokens=cached,
        )

    def now():
        i = min(idx["i"], len(script) - 1)
        elapsed, _ = script[i]
        return base.replace() + _seconds(elapsed)

    rung = {"total": 0}

    def bell(count):
        rung["total"] += count
        sys.stderr.write(f"  [BELL x{count}]\n")
        sys.stderr.flush()

    def render_fn(line):
        sys.stdout.write(line + "\n")
        sys.stdout.flush()

    def advance(_):
        idx["i"] += 1

    run(
        load,
        now_fn=now,
        sleep_fn=advance,
        interval=interval,
        max_ticks=len(script),
        bell_fn=bell,
        render_fn=render_fn,
    )
    print(f"\nself-test complete: {rung['total']} bells rung "
          f"(expected 1+2+3 = 6 across the 60/30/10s thresholds)")
    return 0


def _seconds(n: int):
    from datetime import timedelta

    return timedelta(seconds=n)


def main(argv: Optional[list[str]] = None) -> int:
    parser = argparse.ArgumentParser(
        prog="cache-countdown",
        description="Live cache-TTL countdown + cost-at-stake for awaiting-approval states.",
    )
    src = parser.add_mutually_exclusive_group()
    src.add_argument("--session", help="session id; reads ~/.claude/state/cache-timer-<id>.json")
    src.add_argument("--file", type=Path, help="explicit path to a cache-timer state file")
    parser.add_argument("--interval", type=float, default=1.0, help="tick interval seconds (default 1)")
    parser.add_argument("--self-test", action="store_true", help="run the synthetic self-test")
    parser.add_argument("--once", action="store_true", help="render a single tick and exit")
    parser.add_argument("--stop-when-cold", action="store_true", help="exit once the cache is COLD")
    args = parser.parse_args(argv)

    if args.self_test:
        return _self_test()

    if args.file:
        path = args.file
    elif args.session:
        path = timer_path_for_session(args.session)
    else:
        parser.error("one of --session, --file, or --self-test is required")
        return 2

    def load() -> Optional[TimerSnapshot]:
        if not path.exists():
            return None
        try:
            return read_timer_file(path)
        except (ValueError, OSError):
            return None

    return run(
        load,
        interval=args.interval,
        max_ticks=1 if args.once else None,
        stop_when_cold=args.stop_when_cold,
    )


if __name__ == "__main__":  # pragma: no cover
    raise SystemExit(main())
