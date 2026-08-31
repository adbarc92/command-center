// §6 Telltale feedback adapter — user bug/crash reports as board state.
//
// Reads `GET /v1/issues` from the Telltale ingest Worker (spec §6.3) via a Tauri
// command, so the desktop host — not the browser — owns the cross-origin call and
// holds the operator read token. Written against `FeedbackReader` for the same
// reason `halyard.ts` is written against `HalyardReader`: environment-agnostic and
// unit-testable with fakes.
//
// DEVIATION from spec §6.2, deliberate. §6.2 types the seam as
// `issues(): Promise<TelltaleIssue[]>`, but the Worker returns `{ issues, errors }`
// and §6.3 makes per-repo `errors` REQUIRED behaviour — "one bad repo would blank
// the entire feedback lane" is the failure it exists to prevent. A seam that
// returns only the array throws that list away and makes §6.3 unimplementable, so
// the seam returns the whole response. §6.2's `TelltaleIssue` shape is unchanged.

import type { ProjectCard, Source, StageOverride, BlockedInfo } from '../model';
import { resolveStage, applyOverride } from '../stage';

export const FEEDBACK_SOURCE: Source = 'feedback';

/** §6.2 — one issue as the Worker's `/v1/issues` reports it. */
export interface TelltaleIssue {
  repo: string;
  number: number;
  title: string;
  body: string;
  /** Explicit whitelist over `telltale:*` — `telltale:muted` matches that prefix too. */
  kind: 'bug' | 'crash' | 'unknown';
  /** Resolved by the Worker from its registry, never parsed from a label. */
  project: string;
  isOpen: boolean;
  /** The triage signal (§6.4) — native, free in the list response, self-clearing. */
  hasAssignee: boolean;
  createdIso: string;
  updatedIso: string;
  labels: string[];
  url: string;
}

/** §6.3 — the repos that answered, plus the ones that did not. */
export interface FeedbackIssuesResponse {
  issues: TelltaleIssue[];
  errors: Array<{ project: string; message: string }>;
}

/** The swappable read seam — mirrors `HalyardReader` / `AudienceReader`. */
export interface FeedbackReader {
  issues(): Promise<FeedbackIssuesResponse>;
}

export interface FeedbackAdapterOpts {
  overrides?: Record<string, StageOverride>;
  now?: () => Date;
  /** §6.3 — the Worker caches 60s upstream, so the card tolerates a long budget. */
  staleAfterSec?: number;
}

const WEEK_MS = 7 * 24 * 3_600_000;

function sourceCard(detail: string, nowIso: string, staleAfterSec: number): ProjectCard {
  return {
    projectId: 'feedback:__source__',
    source: FEEDBACK_SOURCE,
    name: 'Feedback',
    stage: 'Idle',
    detail,
    blocked: null,
    stageSource: 'inferred',
    override: null,
    conflict: null,
    updatedIso: nowIso,
    staleAfterSec,
    health: 'unknown',
    family: 'feedback',
  };
}

/**
 * Build the current feedback `ProjectCard`s (§6.4).
 *
 * One card per registry project **with at least one open issue** — empty projects
 * are deliberately omitted: `sortedCards` ranks `Idle` at 99, so ~10 permanently
 * grey "no open reports" cards would pile at the bottom of the grid, inflate the
 * header's project total, and duplicate projects already on the board via `local`.
 *
 * A project in `errors` degrades to its own `health: 'unknown'` card and nothing
 * else — §6.3's whole point is that one 403ing repo must not blank the lane.
 * A throw from the reader itself is different in kind: nothing is known, so it
 * yields the single synthetic `__source__` card the other adapters use.
 */
export async function feedbackCards(
  reader: FeedbackReader,
  opts: FeedbackAdapterOpts = {},
): Promise<ProjectCard[]> {
  const now = opts.now ?? (() => new Date());
  const nowMs = now().getTime();
  const nowIso = now().toISOString();
  const staleAfterSec = opts.staleAfterSec ?? 600;
  const overrides = opts.overrides ?? {};

  let res: FeedbackIssuesResponse;
  try {
    res = await reader.issues();
  } catch (err) {
    return [sourceCard(
      `source unreachable: ${err instanceof Error ? err.message : String(err)}`,
      nowIso,
      staleAfterSec,
    )];
  }

  const byProject = new Map<string, TelltaleIssue[]>();
  for (const issue of res.issues) {
    if (!issue.isOpen) continue; // closed issues carry no board state
    const list = byProject.get(issue.project);
    if (list) list.push(issue);
    else byProject.set(issue.project, [issue]);
  }

  const cards: ProjectCard[] = [];

  for (const [project, open] of byProject) {
    const projectId = `feedback:${project}`;

    // §6.4 precedence, top row wins: an open crash nobody has picked up is the
    // only thing here that is allowed to reach the board's "NEEDS YOU" number.
    const untriaged = open.find((i) => i.kind === 'crash' && !i.hasAssignee);

    const inferred = resolveStage({
      pipeline: null,
      isHumanGate: untriaged !== undefined,
      isTerminalFailure: false,
    });
    const { stage, stageSource, override, conflict } = applyOverride(
      inferred,
      overrides[projectId],
      nowMs,
    );

    const blocked: BlockedInfo | null =
      stage === 'Blocked' && untriaged
        ? { gate: 'manual', action: 'triage crash report', deepLink: untriaged.url }
        : null;

    const newThisWeek = open.filter(
      (i) => nowMs - Date.parse(i.createdIso) < WEEK_MS,
    ).length;

    cards.push({
      projectId,
      source: FEEDBACK_SOURCE,
      name: project,
      stage,
      detail: untriaged
        ? `${open.length} open · untriaged crash`
        : `${open.length} open · ${newThisWeek} new this week`,
      blocked,
      stageSource,
      override,
      conflict,
      updatedIso: nowIso,
      staleAfterSec,
      health: 'ok',
      // §6.4 — set for parity with the other adapters, but verified inert: nothing
      // reads `family` today.
      family: project,
    });
  }

  // §6.3 — degrade only the projects that actually failed.
  for (const e of res.errors) {
    cards.push({
      ...sourceCard(`unreachable: ${e.message}`, nowIso, staleAfterSec),
      projectId: `feedback:${e.project}`,
      name: e.project,
      family: e.project,
    });
  }

  return cards;
}
