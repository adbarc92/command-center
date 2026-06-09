# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
"""Simulated Anthropic 429 retry — models the agent/harness-level auto-retry.

This is NOT a re-implementation of the retry that ships in Claude Code; it is a
faithful *model* of it, used to (a) demonstrate that a transient
"Server is temporarily limiting requests" 429 auto-retries with backoff and
(b) prove the backoff envelope stays within the ~5-minute prompt-cache TTL
(coordinates with Lane A1's cache timer).

The real retry surface is a settings.json env knob — `CLAUDE_CODE_MAX_RETRIES`
(default 10) — handled by the Anthropic SDK underneath the Claude Code harness.
The SDK retries 429/5xx with exponential backoff and **honors the `retry-after`
header**. See FINDINGS.md for the spike write-up. This script reproduces that
contract so the behavior can be verified without burning real API calls.

Run:
    uv run tools/rate-limit-retry/simulate_429.py
    uv run tools/rate-limit-retry/simulate_429.py --retries 10 --sustained
"""

from __future__ import annotations

import argparse
import sys
from dataclasses import dataclass

# ---------------------------------------------------------------------------
# The cache window we must stay inside. Anthropic prompt-cache (ephemeral) TTL
# is ~5 min / 300s. We target a working budget BELOW that so a transient 429
# never forces a cold, full-cost re-read. (Pairs with Lane A1.)
# ---------------------------------------------------------------------------
CACHE_TTL_SECS = 300.0
# Keep a safety margin so the LAST retry still lands inside a warm cache.
CACHE_BUDGET_SECS = 270.0

# The exact harness error text this layer targets (from the Claude Code error
# reference). Matching this is informational only — the SDK classifies on the
# 429 status, not the string.
RL_MESSAGE = "Server is temporarily limiting requests (not your usage limit)"


@dataclass
class BackoffPolicy:
    """Exponential backoff with a per-attempt cap, mirroring the SDK contract.

    `retry_after` (server header, seconds) takes precedence when present — the
    SDK waits exactly that long instead of its computed backoff. This is the
    key reason the schedule reliably stays inside the cache window: Anthropic's
    transient 429s carry small retry-after values (single-digit seconds).
    """

    base_secs: float = 0.5      # initial backoff
    multiplier: float = 2.0     # exponential factor
    cap_secs: float = 8.0       # per-attempt ceiling (SDK clamps; not minutes)

    def delay_for(self, attempt: int, retry_after: float | None = None) -> float:
        if retry_after is not None:
            # Honor the server header verbatim (still clamp insane values).
            return min(retry_after, self.cap_secs * 4)
        raw = self.base_secs * (self.multiplier ** attempt)
        return min(raw, self.cap_secs)


@dataclass
class FakeResponse:
    """One simulated API outcome."""

    status: int                 # 429 == throttled, 200 == ok
    retry_after: float | None = None
    message: str = ""


class Fake429Server:
    """Returns N 429s (a transient throttle), then a 200 — unless --sustained,
    in which case it 429s forever to exercise the exhaustion path."""

    def __init__(self, fail_times: int, retry_after: float | None, sustained: bool):
        self.remaining = fail_times
        self.retry_after = retry_after
        self.sustained = sustained
        self.calls = 0

    def call(self) -> FakeResponse:
        self.calls += 1
        if self.sustained or self.remaining > 0:
            self.remaining -= 1
            return FakeResponse(429, self.retry_after, RL_MESSAGE)
        return FakeResponse(200, None, "ok")


def run_with_retry(
    server: Fake429Server,
    policy: BackoffPolicy,
    max_retries: int,
) -> tuple[bool, float, list[str]]:
    """Drive the call through the modeled retry loop.

    Returns (succeeded, total_backoff_secs, log_lines). No real sleeping — we
    accumulate the *modeled* wall-clock so the demo is instant and deterministic.
    """
    log: list[str] = []
    total_backoff = 0.0

    # attempt 0 is the initial try; up to `max_retries` retries follow.
    for attempt in range(max_retries + 1):
        resp = server.call()
        if resp.status == 200:
            log.append(f"  attempt {attempt}: 200 OK  (succeeded)")
            return True, total_backoff, log

        # 429 throttle — classify and back off (unless we're out of retries).
        if attempt == max_retries:
            log.append(
                f"  attempt {attempt}: 429 \"{resp.message}\" "
                f"-> retries exhausted ({max_retries})"
            )
            return False, total_backoff, log

        delay = policy.delay_for(attempt, resp.retry_after)
        total_backoff += delay
        ra = "" if resp.retry_after is None else f" [retry-after={resp.retry_after}s]"
        log.append(
            f"  attempt {attempt}: 429 throttle{ra} "
            f"-> Retrying in {delay:.1f}s (attempt {attempt + 1}/{max_retries}); "
            f"cumulative backoff={total_backoff:.1f}s"
        )
    return False, total_backoff, log


def scenario(name: str, server: Fake429Server, policy: BackoffPolicy, max_retries: int) -> bool:
    print(f"\n=== {name} ===")
    print(f"    policy: base={policy.base_secs}s x{policy.multiplier} cap={policy.cap_secs}s, "
          f"max_retries={max_retries}")
    ok, total, log = run_with_retry(server, policy, max_retries)
    for line in log:
        print(line)

    within = total <= CACHE_BUDGET_SECS
    print(f"    result: {'RECOVERED' if ok else 'exhausted -> surfaced to user'}; "
          f"total modeled backoff = {total:.1f}s")
    print(f"    cache window: {total:.1f}s {'<=' if within else '>'} "
          f"{CACHE_BUDGET_SECS:.0f}s budget (TTL {CACHE_TTL_SECS:.0f}s) -> "
          f"{'WITHIN WINDOW (cache stays warm)' if within else 'EXCEEDS WINDOW'}")
    return within


def main() -> int:
    p = argparse.ArgumentParser(description="Simulate an Anthropic 429 harness-level auto-retry.")
    p.add_argument("--retries", type=int, default=10,
                   help="max retries (models CLAUDE_CODE_MAX_RETRIES; default 10)")
    p.add_argument("--sustained", action="store_true",
                   help="429 forever (exercise the exhaustion path)")
    args = p.parse_args()

    policy = BackoffPolicy()
    all_within = True

    # Scenario 1: a single transient 429 with no retry-after header.
    all_within &= scenario(
        "Scenario 1 - single transient 429 (no retry-after), then 200",
        Fake429Server(fail_times=1, retry_after=None, sustained=False),
        policy, args.retries,
    )

    # Scenario 2: 429 carrying a server retry-after header (the common case).
    all_within &= scenario(
        "Scenario 2 - 429 with retry-after=3s header, then 200 (SDK honors header)",
        Fake429Server(fail_times=1, retry_after=3.0, sustained=False),
        policy, args.retries,
    )

    # Scenario 3: worst case — every retry 429s. Proves the FULL envelope (all
    # `--retries` backoffs at the cap) still fits inside the cache window.
    all_within &= scenario(
        f"Scenario 3 - WORST CASE: {args.retries} consecutive 429s at the cap",
        Fake429Server(fail_times=10**9, retry_after=None, sustained=True),
        policy, args.retries,
    )

    print("\n" + "=" * 64)
    if all_within:
        print("PASS: every scenario's backoff envelope stays within the "
              f"~{CACHE_TTL_SECS:.0f}s cache window.")
        return 0
    print("FAIL: a backoff envelope exceeded the cache window budget.")
    return 1


if __name__ == "__main__":
    sys.exit(main())
