// §6.3 Fleet adapter — mission phases.
//
// The cheapest source to wire: the cockpit already exposes `GET /units` snapshots
// and emits `phase_changed` events the dashboard subscribes to (push-style, §8),
// so a mission's stage advances live with no polling. A fleet "project" is a
// mission unit (R3 fix #1).

import type { ProjectCard, Source, StageOverride, BlockedInfo } from '../model';
import type { Phase, Snapshot } from '../../types';
import { resolveStage, applyOverride } from '../stage';

export const FLEET_SOURCE: Source = 'fleet';

// §6.3 mapping table — fleet mission phase → canonical pipeline stage.
function pipelineFor(phase: Phase): {
  stage: 'Plan' | 'Spec' | 'Build' | 'Review' | 'Ship' | 'Archived' | null;
  gate: boolean;
  terminal: boolean;
} {
  switch (phase) {
    case 'queued':
    case 'provisioning':
      return { stage: 'Plan', gate: false, terminal: false };
    case 'spec':
      return { stage: 'Spec', gate: false, terminal: false };
    case 'awaiting_oracle_approval':
      return { stage: null, gate: true, terminal: false }; // → Blocked (oracle-approval)
    case 'building':
    case 'checking':
      return { stage: 'Build', gate: false, terminal: false };
    case 'reviewing':
    case 'merge_check':
      return { stage: 'Review', gate: false, terminal: false };
    case 'pr_open':
    case 'done':
      return { stage: 'Ship', gate: false, terminal: false };
    case 'needs_human':
    case 'halted':
      return { stage: null, gate: true, terminal: false }; // → Blocked (manual)
    case 'no_change':
      return { stage: 'Archived', gate: false, terminal: false };
    case 'failed':
      return { stage: null, gate: false, terminal: true }; // → Failed
    default:
      return { stage: null, gate: false, terminal: false };
  }
}

/** Which human gate a blocked phase represents (drives `blocked.gate`/action). */
function gateFor(phase: Phase): { gate: 'oracle-approval' | 'manual'; action: string } {
  if (phase === 'awaiting_oracle_approval') {
    return { gate: 'oracle-approval', action: 'Approve the frozen oracle test set' };
  }
  return { gate: 'manual', action: 'Resume or ship the unit' };
}

export interface FleetUnitState {
  id: string;
  phase: Phase;
  task: string;
  tier: string;
}

export interface FleetAdapterOpts {
  overrides?: Record<string, StageOverride>;
  now?: () => Date;
  staleAfterSec?: number;
  /** Where the cockpit's unit view lives, for deep-links. */
  deepLinkBase?: string;
}

/** Derive one mission unit's `ProjectCard` from its current phase. */
export function fleetCard(u: FleetUnitState, opts: FleetAdapterOpts = {}): ProjectCard {
  const now = opts.now ?? (() => new Date());
  const nowMs = now().getTime();
  const nowIso = now().toISOString();
  const staleAfterSec = opts.staleAfterSec ?? 120; // pushed; freshness budget is generous
  const overrides = opts.overrides ?? {};
  const deepLinkBase = opts.deepLinkBase ?? 'cockpit://units';

  const projectId = `fleet:${u.id}`;
  const { stage: pipeline, gate, terminal } = pipelineFor(u.phase);
  const inferred = resolveStage({ pipeline, isHumanGate: gate, isTerminalFailure: terminal });
  const { stage, stageSource, override, conflict } = applyOverride(inferred, overrides[projectId], nowMs);

  let blocked: BlockedInfo | null = null;
  if (stage === 'Blocked') {
    const g = gateFor(u.phase);
    blocked = { gate: g.gate, action: g.action, deepLink: `${deepLinkBase}/${u.id}` };
  }

  return {
    projectId,
    source: FLEET_SOURCE,
    name: u.task.length > 56 ? `${u.task.slice(0, 56)}…` : u.task,
    stage,
    detail: stage === 'Blocked' || override ? `${u.phase} · ${u.tier}` : u.phase,
    blocked,
    stageSource,
    override,
    conflict,
    updatedIso: nowIso,
    staleAfterSec,
    health: 'ok',
  };
}

/** Build cards from a snapshot list (`GET /units`) on cockpit load. */
export function fleetCardsFromSnapshots(snaps: Snapshot[], opts: FleetAdapterOpts = {}): ProjectCard[] {
  return snaps.map((s) =>
    fleetCard({ id: s.unit_id, phase: s.phase, task: s.task, tier: (s.tier ?? '').toUpperCase() }, opts),
  );
}
