"""budget-checkpoint — a Stop hook that nudges the agent to checkpoint at work
boundaries (Command Center roadmap item 6D).

At a natural work boundary (here: the conversation has grown past a turn-count
threshold, a "context is heavy" clean cut point), the hook surfaces a gentle,
non-blocking reminder to run ``/end-session`` or ``/handoff`` so the next
session starts compact instead of re-loading a bloated conversation at full
cost. It never auto-ends a session and never blocks the turn.
"""

from .core import (
    DEFAULT_TURN_THRESHOLD,
    NUDGE_EVERY,
    StopEvent,
    count_turns,
    evaluate,
    nudge_text,
    parse_event,
    should_nudge,
)

__all__ = [
    "DEFAULT_TURN_THRESHOLD",
    "NUDGE_EVERY",
    "StopEvent",
    "count_turns",
    "evaluate",
    "nudge_text",
    "parse_event",
    "should_nudge",
]
