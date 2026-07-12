// U2 (spec §3.2): parse dispatchable cc-item work-items from ROADMAP.md.
// Structural (via `marked` tokens) so cc-item comments inside code fences are skipped.
import { lexer } from 'marked';
import type { RoadmapItem } from './model';

const HEADER = /^<!--\s*cc-item\s+(.+?)\s*-->$/;

function parseHeader(inner: string): Record<string, string> {
  const out: Record<string, string> = {};
  for (const tok of inner.split(/\s+/)) {
    const eq = tok.indexOf('=');
    if (eq > 0) out[tok.slice(0, eq).toLowerCase()] = tok.slice(eq + 1);
  }
  return out;
}

function isStatus(s: string | undefined): s is RoadmapItem['status'] {
  return s === 'open' || s === 'active' || s === 'blocked' || s === 'done';
}
function isTier(s: string | undefined): s is RoadmapItem['tier'] {
  return s === 't1' || s === 't2' || s === 't3';
}

export function parseRoadmapItems(text: string): { items: RoadmapItem[]; warnings: string[] } {
  const tokens = lexer(text); // block tokens; `code` tokens are opaque → fences skipped
  const warnings: string[] = [];
  const raw: Array<Omit<RoadmapItem, 'dispatchable'>> = [];

  for (let i = 0; i < tokens.length; i++) {
    const t = tokens[i];
    if (t.type !== 'heading') continue;
    // Find the next non-space block; it must be an html comment matching the header.
    let j = i + 1;
    while (j < tokens.length && tokens[j].type === 'space') j++;
    const next = tokens[j];
    if (!next || next.type !== 'html') continue;
    const m = HEADER.exec((next.raw as string).trim());
    if (!m) continue;

    const h = parseHeader(m[1]);
    const title = (t.text as string).trim();
    const status = isStatus(h.status) ? h.status : 'open';
    const tier = isTier(h.tier) ? h.tier : 't1';

    // Resolve dispatch task: a following `**Dispatch:**` paragraph → prose → title.
    let task = title;
    let prose: string | undefined;
    for (let k = j + 1; k < tokens.length; k++) {
      const b = tokens[k];
      if (b.type === 'heading') break;
      if (b.type === 'paragraph') {
        const txt = (b.text as string).trim();
        if (/^\*\*Dispatch:\*\*/i.test(txt)) { task = txt.replace(/^\*\*Dispatch:\*\*\s*/i, ''); break; }
        if (prose === undefined) prose = txt;
      }
    }
    if (task === title && prose) task = prose;

    if (!h.id) warnings.push(`cc-item "${title}" missing id`);
    raw.push({ id: h.id ?? '', title, status, tier, lane: h.lane, task });
  }

  // Duplicate-id detection (unique-per-roadmap; §3.2/§7.3).
  const counts = new Map<string, number>();
  for (const r of raw) if (r.id) counts.set(r.id, (counts.get(r.id) ?? 0) + 1);
  for (const [id, n] of counts) if (n > 1) warnings.push(`duplicate id "${id}"`);

  const items: RoadmapItem[] = raw.map((r) => ({
    ...r,
    dispatchable: !!r.id && counts.get(r.id) === 1,
  }));
  return { items, warnings };
}
