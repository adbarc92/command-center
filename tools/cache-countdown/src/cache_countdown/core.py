"""Pure, side-effect-free core of the cache countdown.

Everything here is deterministic and unit-testable: parse the state file,
compute remaining seconds, map seconds -> color stage, compute cost-at-stake,
and render the display line. The ticker (`ticker.py`) wires this to a clock,
a terminal, and the bell.
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from datetime import datetime, timezone
from enum import Enum
from pathlib import Path
from typing import Optional

# The Anthropic prompt-cache TTL is ~5 minutes (300s). The reference impl
# (KatsuJinCode/claude-cache-countdown) counts down 295s — 300 minus a 5s
# safety buffer so a "0:00" display means the cache is genuinely gone rather
# than racing the server.
CACHE_TTL_SECONDS = 295

# --- Cost-at-stake basis -----------------------------------------------------
# "Cost at stake" = the extra dollars you pay if the cache goes COLD and the
# whole session prefix has to be re-read at full input price instead of being
# served cheaply from cache.
#
# Per-token rates (Claude Opus 4.8, the Command Center's default model — see
# the claude-api skill model table, 2026-06):
#   * full input          $5.00 / 1M tokens   -> $0.000005 / token
#   * cache READ (~0.1x)   $0.50 / 1M tokens   -> $0.0000005 / token
#   * cache WRITE (~1.25x) $6.25 / 1M tokens   (paid once when the cache is
#                                               (re)written; not the at-stake
#                                               delta)
#
# When the cache is HOT, the session prefix is served at the cache-READ rate.
# When it goes COLD, that same prefix is re-read at the FULL INPUT rate. The
# money "at stake" while the countdown runs is therefore:
#
#     at_stake = tokens * (FULL_INPUT_PER_TOKEN - CACHE_READ_PER_TOKEN)
#              = tokens * $0.0000045
#
# i.e. $4.50 per 1M cached tokens. Change MODEL_NAME / the two rates below if
# the default model changes; do not silently hard-code a dollar figure.
MODEL_NAME = "claude-opus-4-8"
FULL_INPUT_PER_TOKEN = 5.00 / 1_000_000
CACHE_READ_PER_TOKEN = 0.50 / 1_000_000
COST_PER_TOKEN_AT_STAKE = FULL_INPUT_PER_TOKEN - CACHE_READ_PER_TOKEN  # $4.50 / 1M


class ColorStage(str, Enum):
    """Visual stages of the countdown, hottest -> coldest."""

    HOT = "HOT"        # session active (Stop hook hasn't fired) — cache refreshing
    GREEN = "GREEN"    # plenty of time left
    YELLOW = "YELLOW"  # getting warm
    RED = "RED"        # respond NOW
    COLD = "COLD"      # cache expired

    @property
    def emoji(self) -> str:
        return {
            ColorStage.HOT: "\U0001F525",    # fire
            ColorStage.GREEN: "\U0001F7E2",   # green circle
            ColorStage.YELLOW: "\U0001F7E1",  # yellow circle
            ColorStage.RED: "\U0001F534",     # red circle
            ColorStage.COLD: "❄️",  # snowflake
        }[self]

    @property
    def label(self) -> str:
        if self is ColorStage.HOT:
            return "HOT"
        if self is ColorStage.COLD:
            return "COLD"
        return ""


# Stage thresholds, in seconds remaining. Mirrors the reference impl:
#   >60s  -> GREEN   30-60s -> YELLOW   <30s -> RED   <=0 -> COLD
# HOT is special: it's keyed off `stopped == False`, not off a time threshold.
GREEN_THRESHOLD = 60
YELLOW_THRESHOLD = 30

# Bell thresholds (seconds remaining). When the countdown crosses each of these
# downward, the ticker rings an escalating number of bells.
BELL_THRESHOLDS = (60, 30, 10)


@dataclass(frozen=True)
class TimerSnapshot:
    """Parsed contents of ~/.claude/state/cache-timer-{session}.json."""

    session_id: str
    timestamp: datetime
    stopped: bool
    project: str = "unknown"
    cwd: Optional[str] = None
    # Optional: number of cached input tokens at stake. The PowerShell hooks
    # don't currently capture this; if absent, cost-at-stake is omitted from
    # the display rather than fabricated.
    cached_tokens: Optional[int] = None


def read_timer_file(path: Path) -> TimerSnapshot:
    """Parse a cache-timer state file. Raises ValueError on malformed input."""
    raw = json.loads(Path(path).read_text(encoding="utf-8"))
    return _snapshot_from_dict(raw)


def _snapshot_from_dict(raw: dict) -> TimerSnapshot:
    if "session_id" not in raw or "timestamp" not in raw:
        raise ValueError("timer file missing session_id/timestamp")
    ts = _parse_timestamp(raw["timestamp"])
    cached = raw.get("cached_tokens")
    return TimerSnapshot(
        session_id=str(raw["session_id"]),
        timestamp=ts,
        stopped=bool(raw.get("stopped", False)),
        project=str(raw.get("project", "unknown")),
        cwd=raw.get("cwd"),
        cached_tokens=int(cached) if cached is not None else None,
    )


def _parse_timestamp(value: str) -> datetime:
    """Parse the PowerShell `Get-Date -Format o` round-trip timestamp.

    Example: ``2026-06-09T11:59:09.6795938-06:00``. Python's fromisoformat
    only accepts up to 6 fractional digits, so trim any extra precision.
    """
    text = str(value).strip()
    # Normalise a trailing Z to an explicit UTC offset.
    if text.endswith("Z"):
        text = text[:-1] + "+00:00"
    # Trim fractional seconds to 6 digits if longer (PowerShell emits 7).
    if "." in text:
        head, frac = text.split(".", 1)
        # frac may carry a timezone suffix, e.g. "6795938-06:00"
        tz = ""
        for i, ch in enumerate(frac):
            if ch in "+-":
                tz = frac[i:]
                frac = frac[:i]
                break
        frac = frac[:6]
        text = f"{head}.{frac}{tz}"
    dt = datetime.fromisoformat(text)
    if dt.tzinfo is None:
        dt = dt.replace(tzinfo=timezone.utc)
    return dt


def remaining_seconds(snap: TimerSnapshot, now: datetime) -> int:
    """Seconds left on the cache window. Clamped at 0 (never negative)."""
    elapsed = (now - snap.timestamp).total_seconds()
    return max(0, int(round(CACHE_TTL_SECONDS - elapsed)))


def stage_for(snap: TimerSnapshot, remaining: int) -> ColorStage:
    """Map (stopped-state, remaining seconds) -> color stage."""
    if not snap.stopped:
        return ColorStage.HOT
    if remaining <= 0:
        return ColorStage.COLD
    if remaining < YELLOW_THRESHOLD:
        return ColorStage.RED
    if remaining < GREEN_THRESHOLD:
        return ColorStage.YELLOW
    return ColorStage.GREEN


def cost_at_stake(snap: TimerSnapshot) -> Optional[float]:
    """Dollars re-spent if the cache goes cold, or None if token count unknown."""
    if snap.cached_tokens is None:
        return None
    return snap.cached_tokens * COST_PER_TOKEN_AT_STAKE


def format_mmss(remaining: int) -> str:
    minutes, seconds = divmod(max(0, remaining), 60)
    return f"{minutes}:{seconds:02d}"


def format_line(snap: TimerSnapshot, remaining: int) -> str:
    """Render the one-line display, e.g. ``🔴 0:45 $5.75`` or ``❄️ COLD``."""
    stage = stage_for(snap, remaining)
    parts = [stage.emoji]
    if stage is ColorStage.COLD:
        parts.append("COLD")
    elif stage is ColorStage.HOT:
        parts.append("HOT")
        parts.append(format_mmss(remaining))
    else:
        parts.append(format_mmss(remaining))
    cost = cost_at_stake(snap)
    if cost is not None and stage is not ColorStage.COLD:
        parts.append(f"${cost:.2f}")
    return " ".join(parts)


def bells_crossed(prev_remaining: int, remaining: int, stopped: bool) -> int:
    """How many bells to ring this tick.

    Returns the escalating bell count for the *lowest* threshold crossed
    downward since the previous tick (60s -> 1, 30s -> 2, 10s -> 3). Returns 0
    if no threshold was crossed or the session isn't stopped.
    """
    if not stopped:
        return 0
    bell_count = 0
    for level, threshold in enumerate(BELL_THRESHOLDS, start=1):
        # Crossed downward through this threshold this tick.
        if prev_remaining > threshold >= remaining:
            bell_count = level
    return bell_count
