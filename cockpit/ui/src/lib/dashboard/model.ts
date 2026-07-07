// The Project Dashboard's single render contract and stage model.
//
// Spec: docs/superpowers/specs/2026-06-09-project-dashboard-design.md (§3, §5).
// Every source adapter (§6) produces `ProjectCard`s through this one model; the
// board (`views/Dashboard.svelte`) renders nothing else. Adding a fifth source
// later is one new adapter, zero board change (locked decision #6).

// ── §3 canonical pipeline + off-pipeline states ─────────────────────────────
// Declared verbatim ONCE here (R3 fix #2): every §6 mapping maps INTO these exact
// strings. This is the single source of truth for stage names.

/** The fixed, cross-tool happy-path pipeline, in order. */
export const PIPELINE = [
  'Idea',
  'Spec',
  'Plan',
  'Build',
  'Review',
  'Ship',
  'Live',
  'Archived',
] as const;
export type PipelineStage = (typeof PIPELINE)[number];

/** A project is in exactly one of these *instead of* a pipeline stage. */
export const OFF_PIPELINE = ['Blocked', 'Failed', 'Idle'] as const;
export type OffPipelineStage = (typeof OFF_PIPELINE)[number];

export type Stage = PipelineStage | OffPipelineStage;

/** §3 fixed ordinal — sort/display only. Off-pipeline states are NOT on this axis. */
export const STAGE_ORDINAL: Record<PipelineStage, number> = {
  Idea: 0,
  Spec: 1,
  Plan: 2,
  Build: 3,
  Review: 4,
  Ship: 5,
  Live: 6,
  Archived: 7,
};

export function isPipelineStage(s: Stage): s is PipelineStage {
  return (PIPELINE as readonly string[]).includes(s);
}

export function isOffPipeline(s: Stage): s is OffPipelineStage {
  return (OFF_PIPELINE as readonly string[]).includes(s);
}

// ── §5.1 ProjectCard — the single render contract ───────────────────────────

export type Source = 'halyard' | 'audience' | 'fleet' | 'app-plugin' | 'manual';

export type Health = 'ok' | 'degraded' | 'unknown';

export type Gate = 'approval' | 'flip' | 'oracle-approval' | 'manual';

export interface BlockedInfo {
  gate: Gate;
  action: string;
  /** Where the operator acts — opens the owning tool (§8). */
  deepLink: string;
}

/** §4 override object — stamped, TTL-expiring, conflict-surfaced. */
export interface StageOverride {
  stage: Stage;
  reason: string;
  setBy: string;
  setAtIso: string;
  ttlHours?: number;
}

/** §4 "declared vs inferred" conflict chip payload. */
export interface ConflictInfo {
  declared: Stage;
  inferred: Stage;
}

export interface ProjectCard {
  /** Globally unique, source-prefixed: "<source>:<native id>" (§5.1, R2 fix #2). */
  projectId: string;
  source: Source;
  name: string;
  /** Canonical stage (§3) — the headline. */
  stage: Stage;
  /** Native sub-state, one level down (§3 glance/depth split). */
  detail: string;
  /** Present iff stage === 'Blocked' (§5.1 — the board's most valuable affordance). */
  blocked: BlockedInfo | null;
  stageSource: 'inferred' | 'declared';
  override: StageOverride | null;
  conflict: ConflictInfo | null;
  /** When this card was last refreshed from its source. */
  updatedIso: string;
  /** Adapter's freshness budget; the board greys the card past it (§8). */
  staleAfterSec: number;
  /** Source/probe liveness — separate from `stage` (§5.1 R2 fix #4). */
  health: Health;
  /** Optional launch/open target (e.g. an app-plugin head). */
  url?: string;
  /** §6.4 — groups two-axis cards (Audience app vs. Audience posts) without merging. */
  family?: string;
  /** §5 — local source only: the project's roadmap work-queue (read by the Phase-2 UI). */
  dispatch?: { items: RoadmapItem[] };
}

// §5 — a roadmap work-item parsed from a ROADMAP.md cc-item header (local source).
export interface RoadmapItem {
  id: string;
  title: string;
  status: 'open' | 'active' | 'blocked' | 'done';
  tier: 't1' | 't2' | 't3';
  lane?: string;
  task: string;          // resolved dispatch text: Dispatch block → prose → title
  missionId?: string;    // Phase-2, inert here
  dispatchable: boolean; // has a unique id (Phase-2 gate; computed now)
}

// ── §5.2 StageSignal — what adapters consume ────────────────────────────────

/**
 * A normalized event/poll-result an adapter turns into (or updates) a `ProjectCard`.
 * Sources emit native shapes; the adapter is the only thing that understands them.
 */
export interface StageSignal {
  projectId: string;
  source: Source;
  /** The source's own enum value (opaque to the board). */
  nativeState: string;
  /** Source-specific blob: proposal count, probe status, iteration n, … */
  nativeDetail?: Record<string, unknown>;
  observedIso: string;
  /** Adapter's read of "this native state is a human gate". */
  isHumanGate: boolean;
  isTerminalFailure: boolean;
}

/**
 * One adapter, one interface (§6): given my source, produce the current set of
 * `ProjectCard`s. The board composes their outputs.
 */
export interface SourceAdapter {
  readonly source: Source;
  /** Returns the current cards, degrading to `health: 'unknown'` cards on source-down (§8). */
  poll(): Promise<ProjectCard[]>;
}
