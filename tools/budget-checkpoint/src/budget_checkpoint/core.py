"""Pure boundary-detection logic for the budget-checkpoint Stop hook.

No I/O of its own beyond reading the transcript file it is handed a path to.
Everything here is deterministic and unit-testable: given a parsed Stop event
(and the transcript it points at), decide whether a *work boundary* was crossed
and, if so, what nudge text to surface.

Roadmap item 6D — "Proactive checkpoint at boundaries"
(docs/playbooks/budget-discipline.md). The playbook's automation note asks for a
"lightweight check that, at a turn-end, evaluates whether a boundary was crossed
... and, if so, surfaces a reminder to run end-session/handoff (a nudge, not an
auto-run)." This module is that check.

Heuristic (v1 — deliberately conservative; see README "Boundary heuristic"):
  The single signal we can read cheaply and reliably from a Stop event is the
  *size of the conversation so far*, via the transcript JSONL the event points
  at. When the turn count crosses a threshold the working context is "heavy" and
  a Stop is a natural clean cut point — exactly 6D's "context-size threshold"
  trigger. We nudge once the count crosses the threshold and then only every
  `NUDGE_EVERY` turns after, so the agent isn't nagged on every single Stop.

We never fire when `stop_hook_active` is true: that flag means a previous Stop
hook already blocked/continued the turn, and re-nudging into that would be the
exact nagging loop 6D warns against.
"""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Optional

# --- Tunable heuristic constants ---------------------------------------------

#: Nudge once the transcript reaches this many turns (user+assistant entries).
#: Conservative on purpose: short sessions never get nudged. ~60 turns is a
#: "this is getting long" signal without being trigger-happy.
DEFAULT_TURN_THRESHOLD = 60

#: After the threshold is crossed, only re-nudge every Nth additional turn, so a
#: long session that keeps stopping isn't nagged on every Stop. 20 ≈ "every
#: little while", not "every turn".
NUDGE_EVERY = 20

#: The advisory text surfaced to the agent. Mentions both routes (6D says
#: end-session for resume-as-future-you, handoff for resume-as-another-agent).
NUDGE_TEMPLATE = (
    "[budget-checkpoint] This session is {turns} turns long — a natural "
    "boundary to checkpoint (roadmap 6D). If the current sub-goal is done, "
    "consider running /end-session (to resume as future-you) or /handoff (to "
    "hand to another agent) so the next session starts compact instead of "
    "re-loading this whole conversation at full cost. Skip if the next step is "
    "tightly coupled to this one."
)


@dataclass(frozen=True)
class StopEvent:
    """The fields of a Stop-hook stdin payload that we care about."""

    session_id: str
    transcript_path: Optional[str]
    stop_hook_active: bool = False
    cwd: Optional[str] = None


def parse_event(payload: dict) -> StopEvent:
    """Build a StopEvent from a decoded Stop-hook JSON payload.

    Tolerant of missing keys — a malformed/partial payload yields an event with
    empties rather than raising, so the hook degrades to "no nudge" instead of
    failing the turn.
    """
    return StopEvent(
        session_id=str(payload.get("session_id", "") or ""),
        transcript_path=payload.get("transcript_path") or None,
        stop_hook_active=bool(payload.get("stop_hook_active", False)),
        cwd=payload.get("cwd") or None,
    )


def count_turns(transcript_path: Optional[str]) -> int:
    """Count conversation turns in a transcript JSONL file.

    Claude Code transcripts are JSON Lines: one JSON object per line, one per
    conversation entry. We count lines that look like a real turn (objects with
    a ``type`` of ``user`` or ``assistant``); lines we can't parse or that are
    summaries/meta are skipped. Returns 0 for a missing/unreadable file so the
    hook simply doesn't nudge rather than erroring.
    """
    if not transcript_path:
        return 0
    path = Path(transcript_path)
    if not path.exists():
        return 0
    turns = 0
    try:
        import json

        with path.open("r", encoding="utf-8") as fh:
            for line in fh:
                line = line.strip()
                if not line:
                    continue
                try:
                    obj = json.loads(line)
                except ValueError:
                    continue
                if isinstance(obj, dict) and obj.get("type") in ("user", "assistant"):
                    turns += 1
    except OSError:
        return 0
    return turns


def should_nudge(
    turns: int,
    stop_hook_active: bool,
    *,
    threshold: int = DEFAULT_TURN_THRESHOLD,
    nudge_every: int = NUDGE_EVERY,
) -> bool:
    """Decide whether this Stop is a boundary worth nudging at.

    Rules:
      * Never nudge while a Stop hook is already active (avoids a nag loop).
      * Never nudge below the threshold (short sessions are cheap to resume).
      * At/after the threshold, nudge on the turn that first crosses it, then
        only every ``nudge_every`` turns thereafter (so a long, chatty session
        is reminded periodically, not constantly).
    """
    if stop_hook_active:
        return False
    if turns < threshold:
        return False
    return (turns - threshold) % max(nudge_every, 1) == 0


def nudge_text(turns: int) -> str:
    """Render the advisory nudge for a transcript of ``turns`` turns."""
    return NUDGE_TEMPLATE.format(turns=turns)


def evaluate(
    event: StopEvent,
    *,
    threshold: int = DEFAULT_TURN_THRESHOLD,
    nudge_every: int = NUDGE_EVERY,
    turn_counter=count_turns,
) -> Optional[str]:
    """Top-level decision: return nudge text, or None if no nudge is warranted.

    ``turn_counter`` is injectable so tests can supply a turn count without a
    real transcript file on disk.
    """
    turns = turn_counter(event.transcript_path)
    if should_nudge(
        turns,
        event.stop_hook_active,
        threshold=threshold,
        nudge_every=nudge_every,
    ):
        return nudge_text(turns)
    return None
