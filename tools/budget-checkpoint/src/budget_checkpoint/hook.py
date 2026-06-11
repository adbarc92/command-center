"""The Stop-hook entry point: read the event JSON on stdin, decide whether a
work boundary was crossed, and emit a gentle, NON-BLOCKING nudge on stdout.

Run via UV:

    uv run budget-checkpoint            # reads a Stop event from stdin
    echo '{"session_id":"x","transcript_path":"...","stop_hook_active":false}' \
        | uv run budget-checkpoint

Output contract (verified against the Claude Code hooks docs):
    A Stop hook may emit, on stdout, JSON with
    ``hookSpecificOutput.additionalContext`` — "non-error feedback that
    continues the conversation". We use exactly that, deliberately AVOIDING
    ``{"decision":"block"}`` (which would *prevent the turn from ending* and can
    force a nag loop). 6D wants a nudge, not an auto-stop, so additionalContext
    is the gentlest mechanism available.

    When no boundary is detected we emit nothing and exit 0 — a silent Stop hook
    is a no-op and never disturbs the turn.

Headless contract (mirrors context-offload/cache-countdown): stdlib only, no
network, and exit code is ALWAYS 0 so a misbehaving transcript can never fail
the user's turn.
"""

from __future__ import annotations

import json
import sys
from typing import Optional

# Windows consoles default to cp1252; a hook's stdout is captured as bytes, so
# force UTF-8 for deterministic encoding of the nudge text.
for _stream in (sys.stdout, sys.stderr):
    _reconf = getattr(_stream, "reconfigure", None)
    if _reconf is not None:
        try:
            _reconf(encoding="utf-8")
        except (ValueError, OSError):
            pass

from .core import DEFAULT_TURN_THRESHOLD, NUDGE_EVERY, evaluate, parse_event


def build_output(additional_context: str) -> dict:
    """Wrap nudge text in the Stop-hook additionalContext envelope."""
    return {
        "hookSpecificOutput": {
            "hookEventName": "Stop",
            "additionalContext": additional_context,
        }
    }


def run(
    raw_stdin: str,
    *,
    threshold: int = DEFAULT_TURN_THRESHOLD,
    nudge_every: int = NUDGE_EVERY,
) -> Optional[dict]:
    """Pure core of the hook: stdin text in, optional output dict out.

    Returns the JSON-serializable output dict when a nudge is warranted, or None
    when the hook should stay silent. Any parse failure yields None (no nudge).
    """
    raw = (raw_stdin or "").strip()
    if not raw:
        return None
    try:
        payload = json.loads(raw)
    except ValueError:
        return None
    if not isinstance(payload, dict):
        return None

    event = parse_event(payload)
    text = evaluate(event, threshold=threshold, nudge_every=nudge_every)
    if text is None:
        return None
    return build_output(text)


def main(argv: Optional[list[str]] = None) -> int:
    raw = sys.stdin.read()
    try:
        output = run(raw)
    except Exception:
        # Last-resort guard: a Stop hook must never crash the turn.
        return 0
    if output is not None:
        sys.stdout.write(json.dumps(output))
        sys.stdout.flush()
    return 0


if __name__ == "__main__":  # pragma: no cover
    raise SystemExit(main())
