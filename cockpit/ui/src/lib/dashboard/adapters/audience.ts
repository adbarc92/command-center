// §6.2 Audience adapter — post-run + backend status.
//
// Reads through the same HTTP API the web UI uses (Audience digest): `GET /health`
// for backend liveness and `GET /posts` for post status. When `/health` is down the
// whole stack is treated as not running ⇒ no live project state (Idle + unknown),
// per the §6.2 mapping. Reads go via a Tauri command (so the desktop host, not the
// browser, owns the cross-origin/devAuth call) behind the `AudienceReader` seam.

import type { ProjectCard, Source, StageOverride, BlockedInfo } from '../model';
import { resolveStage, applyOverride } from '../stage';

export const AUDIENCE_SOURCE: Source = 'audience';

/** One element of `GET /posts` (the fields the dashboard reads). */
export interface AudiencePost {
  id: string;
  status: string; // draft | generating | approval-pending | published | rejected | failed
  text?: string;
  platforms?: string[];
}

export interface AudienceReader {
  /** True iff `GET /health` answered ok. */
  health(): Promise<boolean>;
  posts(): Promise<AudiencePost[]>;
}

// §6.2 mapping table — Audience native post status → canonical pipeline stage.
function pipelineFor(status: string): {
  stage: 'Spec' | 'Build' | 'Live' | 'Archived' | null;
  gate: boolean;
  terminal: boolean;
} {
  switch (status) {
    case 'draft':
      return { stage: 'Spec', gate: false, terminal: false };
    case 'generating':
      return { stage: 'Build', gate: false, terminal: false };
    case 'approval-pending':
      return { stage: null, gate: true, terminal: false }; // → Blocked
    case 'published':
      return { stage: 'Live', gate: false, terminal: false };
    case 'rejected':
      return { stage: 'Archived', gate: false, terminal: false };
    case 'failed':
      return { stage: null, gate: false, terminal: true }; // → Failed
    default:
      return { stage: null, gate: false, terminal: false };
  }
}

export interface AudienceAdapterOpts {
  overrides?: Record<string, StageOverride>;
  now?: () => Date;
  staleAfterSec?: number;
  /** Roll dozens of posts into one "Audience: N awaiting approval" card past this count (§6.2, R3 #3). */
  rollupThreshold?: number;
}

function unknownCard(detail: string, nowIso: string, staleAfterSec: number): ProjectCard {
  return {
    projectId: 'audience:__source__',
    source: AUDIENCE_SOURCE,
    name: 'Audience',
    stage: 'Idle',
    detail,
    blocked: null,
    stageSource: 'inferred',
    override: null,
    conflict: null,
    updatedIso: nowIso,
    staleAfterSec,
    health: 'unknown',
    family: 'audience',
  };
}

/**
 * Build the current Audience `ProjectCard`s. Degrades: `/health` down ⇒ a single
 * Idle + `health: 'unknown'` card (stack not running, no live state, §6.2/§8); a
 * `/posts` fetch that throws ⇒ same. Above `rollupThreshold` posts, emits one
 * rolled-up "N awaiting approval" card so granularity is an adapter policy, not a
 * board concern (R3 fix #3).
 */
export async function audienceCards(
  reader: AudienceReader,
  opts: AudienceAdapterOpts = {},
): Promise<ProjectCard[]> {
  const now = opts.now ?? (() => new Date());
  const nowMs = now().getTime();
  const nowIso = now().toISOString();
  const staleAfterSec = opts.staleAfterSec ?? 15;
  const overrides = opts.overrides ?? {};
  const rollupThreshold = opts.rollupThreshold ?? 8;

  let alive: boolean;
  try {
    alive = await reader.health();
  } catch {
    return [unknownCard('source unreachable: /health', nowIso, staleAfterSec)];
  }
  if (!alive) {
    return [unknownCard('backend down · /health failing', nowIso, staleAfterSec)];
  }

  let posts: AudiencePost[];
  try {
    posts = await reader.posts();
  } catch (err) {
    return [unknownCard(`source unreachable: ${err instanceof Error ? err.message : String(err)}`, nowIso, staleAfterSec)];
  }

  // Rollup policy: if per-post granularity would swamp the board, emit one card.
  if (posts.length > rollupThreshold) {
    const awaiting = posts.filter((p) => p.status === 'approval-pending').length;
    const blocked: BlockedInfo | null =
      awaiting > 0
        ? { gate: 'approval', action: `Approve ${awaiting} post${awaiting > 1 ? 's' : ''}`, deepLink: 'audience:///queue' }
        : null;
    return [
      {
        projectId: 'audience:__rollup__',
        source: AUDIENCE_SOURCE,
        name: 'Audience',
        stage: blocked ? 'Blocked' : 'Live',
        detail: `${posts.length} posts · ${awaiting} awaiting approval`,
        blocked,
        stageSource: 'inferred',
        override: null,
        conflict: null,
        updatedIso: nowIso,
        staleAfterSec,
        health: 'ok',
        family: 'audience',
      },
    ];
  }

  return posts.map((p) => {
    const projectId = `audience:post:${p.id}`;
    const { stage: pipeline, gate, terminal } = pipelineFor(p.status);
    const inferred = resolveStage({ pipeline, isHumanGate: gate, isTerminalFailure: terminal });
    const { stage, stageSource, override, conflict } = applyOverride(inferred, overrides[projectId], nowMs);

    const blocked: BlockedInfo | null =
      stage === 'Blocked'
        ? { gate: 'approval', action: 'Approve before posting', deepLink: `audience:///posts/${p.id}` }
        : null;

    return {
      projectId,
      source: AUDIENCE_SOURCE,
      name: p.text ? p.text.slice(0, 48) : `post ${p.id}`,
      stage,
      detail: stage === 'Blocked' || override ? `${p.status} · ${(p.platforms ?? []).join(', ')}` : p.status,
      blocked,
      stageSource,
      override,
      conflict,
      updatedIso: nowIso,
      staleAfterSec,
      health: 'ok',
      family: 'audience',
    };
  });
}
