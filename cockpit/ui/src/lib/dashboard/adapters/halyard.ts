// §6.1 Halyard adapter — release/launch state.
//
// Reads via the CLI-spawn fallback (§7): a Tauri command spawns `halyard status`
// and `halyard queue`, each of which prints JSON to stdout (confirmed in the
// Halyard digest + cli.ts). The adapter is written against a small read interface
// (`HalyardReader`) so the future read-only-head read API swaps in with no change
// to the mapping below.

import type { ProjectCard, Source, StageOverride } from '../model';
import { resolveStage, applyOverride } from '../stage';

export const HALYARD_SOURCE: Source = 'halyard';

/** One element of `halyard status` stdout (Halyard's `ReleaseStatus`). */
export interface HalyardReleaseStatus {
  release_id: string;
  app: string;
  surface: string;
  version: string;
  state: string; // RELEASE_STATES + dead/rejected
  waiting_on: string;
  flag: string | null;
}

/** One element of `halyard queue` stdout (Halyard's `Proposal`, open by default). */
export interface HalyardProposal {
  proposal_id: string;
  kind: string; // flag_removal | social_post | …
  app: string;
  release_id?: string;
  title: string;
  status: string; // open | approved | …
}

/** The swappable read seam: CLI-spawn today, read-only-head API later (§7). */
export interface HalyardReader {
  status(): Promise<HalyardReleaseStatus[]>;
  queue(): Promise<HalyardProposal[]>;
}

// §6.1 mapping table — Halyard native release-state → canonical pipeline stage.
// (Human-gate / terminal precedence is layered on top via resolveStage.)
function pipelineFor(state: string): { stage: 'Build' | 'Review' | 'Ship' | 'Live' | 'Archived' | null; terminal: boolean } {
  switch (state) {
    case 'tagged':
    case 'built':
      return { stage: 'Build', terminal: false };
    case 'tested':
    case 'uploaded':
    case 'in_review':
      return { stage: 'Review', terminal: false };
    case 'shipped_dark':
      return { stage: 'Ship', terminal: false };
    case 'live':
      return { stage: 'Live', terminal: false };
    case 'rolled_back':
      return { stage: 'Archived', terminal: false };
    case 'dead':
    case 'rejected':
      return { stage: null, terminal: true };
    default:
      return { stage: null, terminal: false };
  }
}

/** A release native-state that is itself a human gate (the flip moment). */
function stateIsFlipGate(state: string): boolean {
  return state === 'shipped_dark';
}

export interface HalyardAdapterOpts {
  /** Override store (§4), keyed by projectId. */
  overrides?: Record<string, StageOverride>;
  now?: () => Date;
  staleAfterSec?: number;
}

/**
 * Build the current Halyard `ProjectCard`s from a reader. Degrades gracefully:
 * a reader that throws (Halyard config root missing / CLI absent) yields a single
 * `health: 'unknown'` card so the board greys Halyard's lane without inventing a
 * stage (§8).
 */
export async function halyardCards(
  reader: HalyardReader,
  opts: HalyardAdapterOpts = {},
): Promise<ProjectCard[]> {
  const now = opts.now ?? (() => new Date());
  const nowMs = now().getTime();
  const nowIso = now().toISOString();
  const staleAfterSec = opts.staleAfterSec ?? 15;
  const overrides = opts.overrides ?? {};

  let statuses: HalyardReleaseStatus[];
  let proposals: HalyardProposal[];
  try {
    [statuses, proposals] = await Promise.all([reader.status(), reader.queue()]);
  } catch (err) {
    return [
      {
        projectId: 'halyard:__source__',
        source: HALYARD_SOURCE,
        name: 'Halyard',
        stage: 'Idle',
        detail: `source unreachable: ${err instanceof Error ? err.message : String(err)}`,
        blocked: null,
        stageSource: 'inferred',
        override: null,
        conflict: null,
        updatedIso: nowIso,
        staleAfterSec,
        health: 'unknown',
      },
    ];
  }

  // Index open proposals by release so a release with an open queue item is Blocked
  // (the queue is the human-gate signal; `detail` keeps the pipeline position).
  const openByRelease = new Map<string, HalyardProposal[]>();
  for (const p of proposals) {
    if (p.status !== 'open') continue;
    const key = p.release_id ?? `app:${p.app}`;
    const arr = openByRelease.get(key) ?? [];
    arr.push(p);
    openByRelease.set(key, arr);
  }

  return statuses.map((s) => {
    const projectId = `halyard:${s.app}:${s.release_id}`;
    const { stage: pipeline, terminal } = pipelineFor(s.state);
    const openProps = openByRelease.get(s.release_id) ?? [];

    const flipGate = stateIsFlipGate(s.state);
    const queueGate = openProps.length > 0;
    const isHumanGate = flipGate || queueGate;

    const inferred = resolveStage({ pipeline, isHumanGate, isTerminalFailure: terminal });
    const { stage, stageSource, override, conflict } = applyOverride(
      inferred,
      overrides[projectId],
      nowMs,
    );

    const detail =
      stage === 'Blocked' || override
        ? `${s.state}${openProps.length ? ` · ${openProps.length} proposal${openProps.length > 1 ? 's' : ''} awaiting approval` : ''}`
        : s.state;

    const blocked =
      stage === 'Blocked'
        ? {
            gate: (flipGate && !queueGate ? 'flip' : 'approval') as 'flip' | 'approval',
            action: queueGate
              ? `Approve ${openProps.length} proposal${openProps.length > 1 ? 's' : ''} in the queue`
              : 'Flip the launch flag',
            deepLink: 'halyard://queue',
          }
        : null;

    return {
      projectId,
      source: HALYARD_SOURCE,
      name: `${s.app} ${s.version} (${s.surface})`,
      stage,
      detail,
      blocked,
      stageSource,
      override,
      conflict,
      updatedIso: nowIso,
      staleAfterSec,
      health: 'ok',
      family: s.app,
    };
  });
}
