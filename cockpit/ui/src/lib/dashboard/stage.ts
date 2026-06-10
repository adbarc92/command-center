// Stage derivation + precedence, applied uniformly by every adapter (§3, §4).
//
// Spec: docs/superpowers/specs/2026-06-09-project-dashboard-design.md.
// This is the one place the §3 precedence rule lives so adapters can't drift:
//   Failed > Blocked > pipeline stage > Idle
// and the §4 inferred-with-declared-override resolution.

import type { Stage, StageOverride, ConflictInfo } from './model';

/** Default override TTL when none is set (§4: 72h). */
export const DEFAULT_OVERRIDE_TTL_HOURS = 72;

/**
 * §3 precedence: pick the headline stage from a pipeline position and the two
 * off-pipeline conditions an adapter detected. Off-pipeline wins over pipeline —
 * `Blocked` is more operationally urgent than `Review` — and `Failed` beats all.
 * The pipeline position is preserved by the caller in `detail` so nothing is lost.
 */
export function resolveStage(opts: {
  pipeline: Stage | null;
  isHumanGate: boolean;
  isTerminalFailure: boolean;
  /** When the source maps to nothing active (no pipeline position, not gated/failed). */
  idleFallback?: boolean;
}): Stage {
  if (opts.isTerminalFailure) return 'Failed';
  if (opts.isHumanGate) return 'Blocked';
  if (opts.pipeline) return opts.pipeline;
  return opts.idleFallback === false ? 'Idle' : 'Idle';
}

/** True once a stamped override has outlived its TTL (§4 anti-rot guard). */
export function isOverrideExpired(o: StageOverride, nowMs: number): boolean {
  const ttlH = o.ttlHours ?? DEFAULT_OVERRIDE_TTL_HOURS;
  const setMs = Date.parse(o.setAtIso);
  if (Number.isNaN(setMs)) return true; // unparseable stamp ⇒ treat as expired
  return nowMs - setMs >= ttlH * 3_600_000;
}

export interface OverrideResolution {
  stage: Stage;
  stageSource: 'inferred' | 'declared';
  override: StageOverride | null;
  conflict: ConflictInfo | null;
}

/**
 * §4 inferred-with-declared-override. The override wins when present and unexpired;
 * an expired override is dropped and the card reverts to inferred (R2 fix #6). When a
 * live override and the inferred stage diverge, a "declared vs inferred" conflict chip
 * is surfaced rather than silently hiding the drift.
 */
export function applyOverride(
  inferred: Stage,
  override: StageOverride | null | undefined,
  nowMs: number,
): OverrideResolution {
  if (!override || isOverrideExpired(override, nowMs)) {
    return { stage: inferred, stageSource: 'inferred', override: null, conflict: null };
  }
  const conflict: ConflictInfo | null =
    override.stage !== inferred ? { declared: override.stage, inferred } : null;
  return { stage: override.stage, stageSource: 'declared', override, conflict };
}
