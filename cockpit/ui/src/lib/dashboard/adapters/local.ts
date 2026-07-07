// U4 (spec §5, §6): the `local` source adapter. Turns scanned project docs into
// fully-resolved ProjectCards WITHOUT applyOverride (a synthesized override would
// expire at 72h and flip stage — spec §5). Degrades to health:'unknown', never a fake stage.
import type { ProjectCard, Source } from '../model';
import { PIPELINE, OFF_PIPELINE } from '../model';
import { parseStatusFrontmatter } from '../frontmatter';
import { parseRoadmapItems } from '../roadmap';

export const LOCAL_SOURCE: Source = 'local';

export interface LocalProjectDoc {
  projectDir: string;
  name?: string;
  statusText?: string;
  roadmapText?: string;
  roadmapHash?: string;
  statusMtimeMs?: number;
  roadmapMtimeMs?: number;
  isPinned: boolean;
}
export interface LocalReader {
  scan(): Promise<LocalProjectDoc[]>;
}

const CANON = new Map<string, string>(
  [...PIPELINE, ...OFF_PIPELINE].map((s) => [s.toLowerCase(), s]),
);
const slug = (p: string): string => p.replace(/[\\/:]/g, '-').replace(/^-+|-+$/g, '');
const basename = (p: string): string => p.replace(/[\\/]+$/, '').split(/[\\/]/).pop() ?? p;

function daysAgo(iso: string | undefined, nowMs: number): string | null {
  if (!iso) return null;
  const t = Date.parse(iso);
  if (Number.isNaN(t)) return null;
  const d = Math.floor((nowMs - t) / 86_400_000);
  return d >= 1 ? `declared ${d}d ago` : null;
}

export async function localCards(
  reader: LocalReader,
  opts: { now?: () => Date; staleAfterSec?: number } = {},
): Promise<ProjectCard[]> {
  const now = opts.now ?? (() => new Date());
  const nowIso = now().toISOString();
  const nowMs = now().getTime();
  const staleAfterSec = opts.staleAfterSec ?? 90;

  let docs: LocalProjectDoc[];
  try {
    docs = await reader.scan();
  } catch (err) {
    return [{
      projectId: 'local:__source__', source: LOCAL_SOURCE, name: 'Local projects',
      stage: 'Idle', detail: `scan failed: ${err instanceof Error ? err.message : String(err)}`,
      blocked: null, stageSource: 'inferred', override: null, conflict: null,
      updatedIso: nowIso, staleAfterSec, health: 'unknown',
    }];
  }

  const cards: ProjectCard[] = [];
  for (const d of docs) {
    const projectId = `local:${slug(d.projectDir)}`;
    const marker = parseStatusFrontmatter(d.statusText ?? '');
    const nameFallback = d.name ?? basename(d.projectDir);

    // Pinned-but-unmarked → degraded card (honor the explicit pin). Auto-discovered
    // unmarked → skip (must not appear; spec §6.5 / locked #2).
    if (!marker.present || !marker.stage) {
      if (d.isPinned) {
        cards.push({
          projectId, source: LOCAL_SOURCE, name: nameFallback, stage: 'Idle',
          detail: 'no STATUS marker', blocked: null, stageSource: 'inferred',
          override: null, conflict: null, updatedIso: nowIso, staleAfterSec, health: 'unknown',
        });
      }
      continue;
    }

    const canon = CANON.get(marker.stage.toLowerCase());
    if (!canon) {
      cards.push({
        projectId, source: LOCAL_SOURCE, name: marker.name ?? nameFallback, stage: 'Idle',
        detail: `invalid stage: ${marker.stage}`, blocked: null, stageSource: 'inferred',
        override: null, conflict: null, updatedIso: nowIso, staleAfterSec, health: 'unknown',
      });
      continue;
    }

    const { items } = parseRoadmapItems(d.roadmapText ?? '');
    const openCount = items.filter((i) => i.status === 'open').length;
    const rot = daysAgo(marker.updated, nowMs);
    const countDetail = items.length ? `${items.length} tagged · ${openCount} open` : 'no roadmap items';
    const detail = marker.readiness
      ? `${marker.readiness}${rot ? ` · ${rot}` : ''}`
      : `${countDetail}${rot ? ` · ${rot}` : ''}`;

    const blocked = canon === 'Blocked'
      ? { gate: 'manual' as const, action: marker.blocked ?? 'Review STATUS', deepLink: `${d.projectDir}/docs/STATUS.md` }
      : null;

    cards.push({
      projectId, source: LOCAL_SOURCE, name: marker.name ?? nameFallback,
      stage: canon as ProjectCard['stage'], detail, blocked,
      stageSource: 'declared', override: null, conflict: null,
      updatedIso: marker.updated ?? nowIso, staleAfterSec, health: 'ok',
      dispatch: { items },
    });
  }
  return cards;
}
