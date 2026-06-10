"""Cache-aware approval timer for Claude Code.

Counts down the ~5-minute Anthropic prompt-cache TTL whenever a task is
awaiting user approval (the Stop hook fired), so the user responds before the
cache goes cold and a large session has to be re-read at full cost.
"""

from .core import (
    CACHE_TTL_SECONDS,
    ColorStage,
    TimerSnapshot,
    cost_at_stake,
    format_line,
    read_timer_file,
    remaining_seconds,
    stage_for,
)

__all__ = [
    "CACHE_TTL_SECONDS",
    "ColorStage",
    "TimerSnapshot",
    "cost_at_stake",
    "format_line",
    "read_timer_file",
    "remaining_seconds",
    "stage_for",
]
