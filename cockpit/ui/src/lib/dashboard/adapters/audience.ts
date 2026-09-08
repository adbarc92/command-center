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
/**
 * §6.2's status vocabulary, taken from Audience's own generated contract
 * (`packages/contracts/src/generated/enums.ts`, `PostStatusSchema`) and pinned in
 * `contracts/audience-post-status.contract.json`.
 *
 * ⚠ The spec's original list — `draft → generating → approval-pending → published`
 * plus `rejected`/`failed` — was derived from a written digest rather than from the
 * schema, and three of those six strings Audience never emits. The adapter
 * implemented the spec faithfully and was therefore wrong: `awaiting_approval`,
 * the approve-before-post gate, fell through to `default` and never blocked.
 */
export type AudiencePostStatus =
  | 'draft'
  | 'generating'
  | 'ready_for_review'
  | 'awaiting_approval'
  | 'approved'
  | 'publishing'
  | 'fully_published'
  | 'partially_published'
  | 'failed';

/** One item of `GET /posts`'s `{ items: [...] }` envelope, as the API returns it. */
export interface AudiencePost {
  id: string;
  /** Widened to `string` on purpose: the wire can carry a status this build has not
   *  heard of, and `pipelineFor` must classify it rather than crash. */
  status: string;
  text?: string;
  createdAt?: string;
  updatedAt?: string | null;
}

export interface AudienceReader {
  /** True iff `GET /health` answered ok. */
  health(): Promise<boolean>;
  posts(): Promise<AudiencePost[]>;
}

// §6.2 mapping table — Audience native post status → canonical pipeline stage.
type Classification = {
  stage: 'Spec' | 'Build' | 'Live' | 'Archived' | null;
  gate: boolean;
  terminal: boolean;
};

/**
 * Exhaustive by construction: `Record<AudiencePostStatus, …>` fails to compile if
 * Audience adds a status and this table does not grow with it. That is the whole
 * point — the previous `switch` had a silent `default`, so five of Audience's nine
 * statuses classified as "nothing" and no test noticed.
 */
const CLASSIFY: Record<AudiencePostStatus, Classification> = {
  // Straight from the spec's mapping table.
  draft: { stage: 'Spec', gate: false, terminal: false },
  generating: { stage: 'Build', gate: false, terminal: false },
  fully_published: { stage: 'Live', gate: false, terminal: false },
  failed: { stage: null, gate: false, terminal: true },

  // The spec's "approve-before-post human gate", under its real name.
  awaiting_approval: { stage: null, gate: true, terminal: false },

  // In flight, no human owed anything.
  approved: { stage: 'Build', gate: false, terminal: false },
  publishing: { stage: 'Build', gate: false, terminal: false },

  // ⚠ Two product judgments the spec never contemplated, called out rather than
  // buried. Both are gated on the principle that the board exists to surface what
  // needs a person; change them here if the intent differs.
  //   ready_for_review   — a human must look before it can advance to approval.
  //   partially_published — some targets published and some did not; someone has to
  //                         decide about the rest, and `failed` (terminal) is wrong
  //                         because part of it did ship.
  ready_for_review: { stage: null, gate: true, terminal: false },
  partially_published: { stage: null, gate: true, terminal: false },
};

function pipelineFor(status: string): Classification {
  // An unknown status is classified as nothing rather than guessed at — the same
  // posture as the old `default`, but now reachable only by a status genuinely
  // absent from the pinned contract, not by five of the nine real ones.
  return CLASSIFY[status as AudiencePostStatus] ?? { stage: null, gate: false, terminal: false };
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
    // Derived from the same classification the per-post path uses, never from a
    // literal. This line previously hardcoded 'approval-pending' independently of
    // `pipelineFor`, so it would have kept reporting 0 even after the mapping was
    // corrected — and the rollup is precisely the high-volume case an operator
    // relies on, where a missed gate hides the most work.
    const awaiting = posts.filter((p) => pipelineFor(p.status).gate).length;
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
      detail: p.status,
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
