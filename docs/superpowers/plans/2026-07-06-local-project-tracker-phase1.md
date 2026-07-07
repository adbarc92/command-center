# Local Project Tracker — Phase 1 (Tracking) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a read-only `local` source to the Project Dashboard that shows each local project's stage (from `docs/STATUS.md` front-matter) and its roadmap work-items (from `ROADMAP.md` `cc-item` headers).

**Architecture:** A Rust Tauri command (`scan_local_projects`) does filesystem discovery + raw file reads (no parsing); a pure-TS adapter (`localCards`) parses front-matter (`js-yaml`) and `cc-item`s (`marked`, so code fences are skipped) and emits **fully-resolved** `ProjectCard`s (`stageSource:'declared'`, `override:null` — it never calls `applyOverride`, which would expire the marker at 72h). The store's `pollLocal` replaces the `local` source's cards each poll, exactly like `pollHalyard`.

**Tech Stack:** Rust (Tauri command, `sha2`, `walkdir` or std `fs`), TypeScript, `js-yaml`, `marked`, `vitest`, `node:test`-style Rust unit tests.

**Spec:** [`docs/superpowers/specs/2026-07-06-local-project-tracker-design.md`](../specs/2026-07-06-local-project-tracker-design.md). This plan implements **Phase 1 only** (U1–U4). Phase 2 (dispatch, U5) is a separate plan written after these interfaces settle.

## Global Constraints

- **No `applyOverride` for local cards.** Local cards are emitted fully-resolved: `stage = markerStage`, `stageSource = 'declared'`, `override = null`, `conflict = null` (spec §5). Verified: `Dashboard.svelte:184` renders the DECLARED chip null-safe on `override?.reason`.
- **`Source` union and `SOURCE_LABEL` must stay in sync** — adding `'local'` to the union (`model.ts`) requires a `SOURCE_LABEL['local']` entry (`Dashboard.svelte`).
- **Degrade, never invent a stage** (parent locked #4 / §8): a scan failure or unreadable project yields a `health:'unknown'` card, never a fabricated stage.
- **Windows-first:** paths use forward-slash normalization; front-matter parsing strips a leading UTF-8 BOM and tolerates CRLF (spec §3.1).
- **Canonical stage values** come from `model.ts` `PIPELINE`/`OFF_PIPELINE`; introduce no new stage strings.
- **Phase-1 board touches are limited to** `SOURCE_LABEL['local']` and the "declared Nd ago" footer hint (spec §5); no change to `stage.ts` or any existing adapter.

---

### Task 1: STATUS.md front-matter parser

**Files:**
- Create: `cockpit/ui/src/lib/dashboard/frontmatter.ts`
- Test: `cockpit/ui/src/lib/dashboard/frontmatter.test.ts`
- Modify: `cockpit/ui/package.json` (add `js-yaml` + `@types/js-yaml`)

**Interfaces:**
- Produces: `parseStatusFrontmatter(text: string): StatusMarker` where
  ```ts
  export interface StatusMarker {
    present: boolean;              // was there a byte-0 front-matter block at all
    stage?: string;                // raw stage string (may be non-canonical)
    readiness?: string;
    updated?: string;              // ISO string (coerced)
    blocked?: string;
    name?: string;
    baseBranch?: string;           // Phase-2 field, parsed now
    testCmd?: string;              // Phase-2 field, parsed now
  }
  ```

- [ ] **Step 1: Add the dependency**

Run: `cd cockpit/ui && npm install js-yaml && npm install -D @types/js-yaml`
Expected: `js-yaml` in `dependencies`, `@types/js-yaml` in `devDependencies`.

- [ ] **Step 2: Write the failing tests**

```ts
// cockpit/ui/src/lib/dashboard/frontmatter.test.ts
import { describe, it, expect } from 'vitest';
import { parseStatusFrontmatter } from './frontmatter';

describe('parseStatusFrontmatter', () => {
  it('parses a valid byte-0 block', () => {
    const m = parseStatusFrontmatter('---\nstage: Build\nreadiness: "85%"\nupdated: "2026-07-06"\n---\n# Title\n');
    expect(m.present).toBe(true);
    expect(m.stage).toBe('Build');
    expect(m.readiness).toBe('85%');
    expect(m.updated).toBe('2026-07-06');
  });

  it('strips a leading UTF-8 BOM', () => {
    const m = parseStatusFrontmatter('﻿---\nstage: Ship\n---\n');
    expect(m.present).toBe(true);
    expect(m.stage).toBe('Ship');
  });

  it('tolerates CRLF fences', () => {
    const m = parseStatusFrontmatter('---\r\nstage: Plan\r\n---\r\n# T\r\n');
    expect(m.stage).toBe('Plan');
  });

  it('is absent when line 1 is not a fence (no false front-matter from a Session-log rule)', () => {
    const m = parseStatusFrontmatter('# STATUS\n\nsome prose\n\n---\n\n## Session 1\n');
    expect(m.present).toBe(false);
    expect(m.stage).toBeUndefined();
  });

  it('coerces an unquoted date to a string (not a Date)', () => {
    const m = parseStatusFrontmatter('---\nstage: Build\nupdated: 2026-07-06\n---\n');
    expect(typeof m.updated).toBe('string');
    expect(m.updated).toBe('2026-07-06');
  });

  it('parses baseBranch and testCmd (Phase-2 fields) when present', () => {
    const m = parseStatusFrontmatter('---\nstage: Build\nbase_branch: "main"\ntest_cmd: "cargo test"\n---\n');
    expect(m.baseBranch).toBe('main');
    expect(m.testCmd).toBe('cargo test');
  });
});
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cd cockpit/ui && npx vitest run src/lib/dashboard/frontmatter.test.ts`
Expected: FAIL — `parseStatusFrontmatter` is not defined.

- [ ] **Step 4: Implement the parser**

```ts
// cockpit/ui/src/lib/dashboard/frontmatter.ts
// U1 (spec §3.1): parse a byte-0 YAML front-matter block from STATUS.md.
// Robust to BOM, CRLF, and the Session-log `---` horizontal-rule collision.
import { load } from 'js-yaml';

export interface StatusMarker {
  present: boolean;
  stage?: string;
  readiness?: string;
  updated?: string;
  blocked?: string;
  name?: string;
  baseBranch?: string;
  testCmd?: string;
}

const str = (v: unknown): string | undefined =>
  v == null ? undefined : v instanceof Date ? v.toISOString().slice(0, 10) : String(v);

export function parseStatusFrontmatter(text: string): StatusMarker {
  // Strip a leading UTF-8 BOM so the byte-0 fence check holds on Windows-written files.
  const body = text.charCodeAt(0) === 0xfeff ? text.slice(1) : text;
  const lines = body.split('\n');
  // Front-matter exists only if line 1 is exactly a fence (CRLF-tolerant).
  if (!/^---\r?$/.test(lines[0] ?? '')) return { present: false };
  // Closing fence = the next `---` line; a later `---` is a Session-log rule, not a fence.
  let end = -1;
  for (let i = 1; i < lines.length; i++) {
    if (/^---\r?$/.test(lines[i])) {
      end = i;
      break;
    }
  }
  if (end === -1) return { present: false };

  const yaml = lines.slice(1, end).join('\n');
  let doc: Record<string, unknown>;
  try {
    doc = (load(yaml) as Record<string, unknown>) ?? {};
  } catch {
    return { present: true }; // malformed YAML: block existed but yielded no fields
  }
  return {
    present: true,
    stage: str(doc.stage),
    readiness: str(doc.readiness),
    updated: str(doc.updated),
    blocked: str(doc.blocked),
    name: str(doc.name),
    baseBranch: str(doc.base_branch),
    testCmd: str(doc.test_cmd),
  };
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cd cockpit/ui && npx vitest run src/lib/dashboard/frontmatter.test.ts`
Expected: PASS (6 tests).

- [ ] **Step 6: Commit**

```bash
git add cockpit/ui/src/lib/dashboard/frontmatter.ts cockpit/ui/src/lib/dashboard/frontmatter.test.ts cockpit/ui/package.json cockpit/ui/package-lock.json
git commit -m "feat(dashboard): STATUS.md front-matter parser (U1)"
```

---

### Task 2: ROADMAP.md `cc-item` parser + model types

**Files:**
- Create: `cockpit/ui/src/lib/dashboard/roadmap.ts`
- Test: `cockpit/ui/src/lib/dashboard/roadmap.test.ts`
- Modify: `cockpit/ui/src/lib/dashboard/model.ts` (add `RoadmapItem` + `ProjectCard.dispatch`)
- Modify: `cockpit/ui/package.json` (add `marked`)

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `model.ts`: `RoadmapItem` interface + `dispatch?: { items: RoadmapItem[] }` on `ProjectCard`.
  - `parseRoadmapItems(text: string): { items: RoadmapItem[]; warnings: string[] }`.

- [ ] **Step 1: Add the dependency**

Run: `cd cockpit/ui && npm install marked`
Expected: `marked` in `dependencies`.

- [ ] **Step 2: Add the model types**

In `cockpit/ui/src/lib/dashboard/model.ts`, after the `ProjectCard` interface, add:

```ts
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
```

And add one optional field inside the `ProjectCard` interface (after `family?`):

```ts
  /** §5 — local source only: the project's roadmap work-queue (read by the Phase-2 UI). */
  dispatch?: { items: RoadmapItem[] };
```

- [ ] **Step 3: Write the failing tests**

```ts
// cockpit/ui/src/lib/dashboard/roadmap.test.ts
import { describe, it, expect } from 'vitest';
import { parseRoadmapItems } from './roadmap';

describe('parseRoadmapItems', () => {
  it('parses a tagged heading into an item, resolving the Dispatch body', () => {
    const md = [
      '## Cache timer',
      '<!-- cc-item id=cache-timer status=done tier=t2 lane=workflow -->',
      'Some prose.',
      '',
      '**Dispatch:** Do the thing. Acceptance: it works.',
    ].join('\n');
    const { items } = parseRoadmapItems(md);
    expect(items).toHaveLength(1);
    expect(items[0]).toMatchObject({
      id: 'cache-timer', title: 'Cache timer', status: 'done', tier: 't2',
      lane: 'workflow', dispatchable: true,
    });
    expect(items[0].task).toContain('Do the thing');
  });

  it('IGNORES a cc-item inside a fenced code block', () => {
    const md = [
      '## Docs',
      'How the format works:',
      '```',
      '## Example',
      '<!-- cc-item id=example status=open tier=t1 -->',
      '```',
    ].join('\n');
    expect(parseRoadmapItems(md).items).toHaveLength(0);
  });

  it('defaults tier to t1 and falls back task→title when no Dispatch/prose', () => {
    const md = '## Bare item\n<!-- cc-item id=bare status=open -->\n';
    const { items } = parseRoadmapItems(md);
    expect(items[0].tier).toBe('t1');
    expect(items[0].task).toBe('Bare item');
  });

  it('marks an item missing id as non-dispatchable and warns', () => {
    const md = '## No id\n<!-- cc-item status=open tier=t1 -->\n';
    const { items, warnings } = parseRoadmapItems(md);
    expect(items[0].dispatchable).toBe(false);
    expect(warnings.join(' ')).toMatch(/missing id/i);
  });

  it('marks duplicate ids non-dispatchable and warns', () => {
    const md = [
      '## A', '<!-- cc-item id=dup status=open -->',
      '## B', '<!-- cc-item id=dup status=open -->',
    ].join('\n');
    const { items, warnings } = parseRoadmapItems(md);
    expect(items.every((i) => !i.dispatchable)).toBe(true);
    expect(warnings.join(' ')).toMatch(/duplicate id/i);
  });
});
```

- [ ] **Step 4: Run the tests to verify they fail**

Run: `cd cockpit/ui && npx vitest run src/lib/dashboard/roadmap.test.ts`
Expected: FAIL — `parseRoadmapItems` is not defined.

- [ ] **Step 5: Implement the parser**

```ts
// cockpit/ui/src/lib/dashboard/roadmap.ts
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
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cd cockpit/ui && npx vitest run src/lib/dashboard/roadmap.test.ts`
Expected: PASS (5 tests).

- [ ] **Step 7: Commit**

```bash
git add cockpit/ui/src/lib/dashboard/roadmap.ts cockpit/ui/src/lib/dashboard/roadmap.test.ts cockpit/ui/src/lib/dashboard/model.ts cockpit/ui/package.json cockpit/ui/package-lock.json
git commit -m "feat(dashboard): ROADMAP.md cc-item parser + RoadmapItem model (U2)"
```

---

### Task 3: The `local` adapter

**Files:**
- Create: `cockpit/ui/src/lib/dashboard/adapters/local.ts`
- Test: `cockpit/ui/src/lib/dashboard/adapters/local.test.ts`
- Modify: `cockpit/ui/src/lib/dashboard/model.ts` (add `'local'` to `Source`)

**Interfaces:**
- Consumes: `parseStatusFrontmatter` (Task 1), `parseRoadmapItems` (Task 2), `ProjectCard`/`RoadmapItem` (model).
- Produces:
  ```ts
  export interface LocalProjectDoc {
    projectDir: string; name?: string;
    statusText?: string; roadmapText?: string; roadmapHash?: string;
    statusMtimeMs?: number; roadmapMtimeMs?: number; isPinned: boolean;
  }
  export interface LocalReader { scan(): Promise<LocalProjectDoc[]>; }
  export function localCards(reader: LocalReader, opts?: { now?: () => Date; staleAfterSec?: number }): Promise<ProjectCard[]>;
  export const LOCAL_SOURCE: Source;
  ```

- [ ] **Step 1: Add `'local'` to the Source union**

In `cockpit/ui/src/lib/dashboard/model.ts`, change:
```ts
export type Source = 'halyard' | 'audience' | 'fleet' | 'app-plugin' | 'manual';
```
to:
```ts
export type Source = 'halyard' | 'audience' | 'fleet' | 'app-plugin' | 'manual' | 'local';
```

- [ ] **Step 2: Write the failing tests**

```ts
// cockpit/ui/src/lib/dashboard/adapters/local.test.ts
import { describe, it, expect } from 'vitest';
import { localCards, type LocalProjectDoc, type LocalReader } from './local';

const NOW = () => new Date('2026-07-10T00:00:00Z');
const reader = (docs: LocalProjectDoc[]): LocalReader => ({ scan: async () => docs });

describe('localCards', () => {
  it('emits a declared card for a valid marker, fully resolved (no override)', async () => {
    const cards = await localCards(
      reader([{ projectDir: 'D:/proj/alpha', statusText: '---\nstage: Build\nreadiness: "85%"\nupdated: "2026-07-06"\n---\n', isPinned: false }]),
      { now: NOW },
    );
    expect(cards).toHaveLength(1);
    expect(cards[0]).toMatchObject({
      projectId: 'local:D--proj-alpha', source: 'local', stage: 'Build',
      stageSource: 'declared', override: null, conflict: null, health: 'ok',
    });
    expect(cards[0].detail).toContain('85%');
  });

  it('does NOT emit a card for an auto-discovered project with no marker', async () => {
    const cards = await localCards(reader([{ projectDir: 'D:/proj/beta', statusText: '# no frontmatter\n', isPinned: false }]), { now: NOW });
    expect(cards).toHaveLength(0);
  });

  it('emits a degraded card for a PINNED project with no marker', async () => {
    const cards = await localCards(reader([{ projectDir: 'D:/proj/beta', statusText: undefined, isPinned: true }]), { now: NOW });
    expect(cards).toHaveLength(1);
    expect(cards[0]).toMatchObject({ health: 'unknown', stage: 'Idle' });
    expect(cards[0].detail).toMatch(/no STATUS marker/i);
  });

  it('emits a degraded card for an invalid stage value', async () => {
    const cards = await localCards(reader([{ projectDir: 'D:/proj/g', statusText: '---\nstage: Bogus\n---\n', isPinned: false }]), { now: NOW });
    expect(cards[0]).toMatchObject({ health: 'unknown' });
    expect(cards[0].detail).toMatch(/invalid stage: Bogus/);
  });

  it('attaches parsed roadmap items and an item count detail', async () => {
    const cards = await localCards(
      reader([{
        projectDir: 'D:/proj/alpha',
        statusText: '---\nstage: Build\n---\n',
        roadmapText: '## A\n<!-- cc-item id=a status=open tier=t1 -->\n## B\n<!-- cc-item id=b status=done -->\n',
        isPinned: false,
      }]),
      { now: NOW },
    );
    expect(cards[0].dispatch?.items).toHaveLength(2);
    expect(cards[0].detail).toMatch(/2 tagged · 1 open/);
  });

  it('degrades every lane to unknown if the reader throws', async () => {
    const cards = await localCards({ scan: async () => { throw new Error('scan boom'); } }, { now: NOW });
    expect(cards).toHaveLength(1);
    expect(cards[0]).toMatchObject({ source: 'local', health: 'unknown' });
    expect(cards[0].detail).toMatch(/scan boom/);
  });
});
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cd cockpit/ui && npx vitest run src/lib/dashboard/adapters/local.test.ts`
Expected: FAIL — `./local` has no exports.

- [ ] **Step 4: Implement the adapter**

```ts
// cockpit/ui/src/lib/dashboard/adapters/local.ts
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
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cd cockpit/ui && npx vitest run src/lib/dashboard/adapters/local.test.ts`
Expected: PASS (6 tests).

- [ ] **Step 6: Commit**

```bash
git add cockpit/ui/src/lib/dashboard/adapters/local.ts cockpit/ui/src/lib/dashboard/adapters/local.test.ts cockpit/ui/src/lib/dashboard/model.ts
git commit -m "feat(dashboard): local source adapter — docs → ProjectCards (U4)"
```

---

### Task 4: Rust `scan_local_projects` command

**Files:**
- Create: `cockpit/ui/src-tauri/src/local_projects.rs`
- Modify: `cockpit/ui/src-tauri/src/lib.rs` (add `mod local_projects;` and register the command)
- Modify: `cockpit/ui/src-tauri/Cargo.toml` (add `sha2`, `walkdir`)

**Interfaces:**
- Produces: `#[tauri::command] scan_local_projects(config: ScanConfig) -> Result<Vec<LocalProjectDoc>, String>` where `ScanConfig = { scan_roots, max_depth, pins, excludes }` and `LocalProjectDoc` serializes to the TS shape in Task 3 (camelCase via serde rename).

- [ ] **Step 1: Add dependencies**

In `cockpit/ui/src-tauri/Cargo.toml` `[dependencies]`, add:
```toml
sha2 = "0.10"
walkdir = "2"
```
Run: `cd cockpit/ui/src-tauri && cargo build`
Expected: builds (new crates fetched).

- [ ] **Step 2: Write the failing tests + skeleton**

```rust
// cockpit/ui/src-tauri/src/local_projects.rs
//! U4 (spec §4, §6): filesystem discovery + raw reads for the `local` dashboard source.
//! No markdown parsing here — the TS adapter parses. Discovery is bounded-recursive,
//! prunes heavy dirs, does NOT follow symlinks, and treats every dir with docs/STATUS.md
//! as a project (nested markers allowed). `roadmap_hash` (SHA-256 over raw bytes) feeds
//! the Phase-2 write-back CAS.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ScanConfig {
    #[serde(default)]
    pub scan_roots: Vec<String>,
    #[serde(default = "default_depth")]
    pub max_depth: usize,
    #[serde(default)]
    pub pins: Vec<String>,
    #[serde(default)]
    pub excludes: Vec<String>,
}
fn default_depth() -> usize { 5 }

#[derive(Serialize, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LocalProjectDoc {
    pub project_dir: String,
    pub status_text: Option<String>,
    pub roadmap_text: Option<String>,
    pub roadmap_hash: Option<String>,
    pub is_pinned: bool,
}

const PRUNE: &[&str] = &[".git", "node_modules", "target", "dist"];

fn normalize(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

fn is_excluded(path: &str, excludes: &[String]) -> bool {
    let p = path.to_lowercase();
    excludes.iter().any(|e| p.contains(&e.replace('\\', "/").to_lowercase()))
}

/// Read a project dir's STATUS.md/ROADMAP.md into a doc (raw text; hash over raw bytes).
fn read_project(dir: &Path, is_pinned: bool) -> LocalProjectDoc {
    let status_text = std::fs::read_to_string(dir.join("docs/STATUS.md")).ok();
    let roadmap_bytes = std::fs::read(dir.join("ROADMAP.md")).ok();
    let roadmap_hash = roadmap_bytes.as_ref().map(|b| {
        let mut h = Sha256::new();
        h.update(b);
        format!("{:x}", h.finalize())
    });
    let roadmap_text = roadmap_bytes.and_then(|b| String::from_utf8(b).ok());
    LocalProjectDoc {
        project_dir: normalize(dir),
        status_text,
        roadmap_text,
        roadmap_hash,
        is_pinned,
    }
}

/// Bounded-recursive discovery: every dir containing docs/STATUS.md is a project
/// (including nested). Prunes PRUNE dirs, skips symlinks, respects excludes + depth.
fn discover(root: &Path, max_depth: usize, excludes: &[String], out: &mut Vec<PathBuf>) {
    let walker = walkdir::WalkDir::new(root)
        .max_depth(max_depth)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            !(e.file_type().is_dir() && PRUNE.contains(&name.as_ref()))
                && !is_excluded(&normalize(e.path()), excludes)
        });
    for entry in walker.flatten() {
        if entry.file_type().is_dir() && entry.path().join("docs/STATUS.md").is_file() {
            out.push(entry.path().to_path_buf());
        }
    }
}

#[tauri::command]
pub fn scan_local_projects(config: ScanConfig) -> Result<Vec<LocalProjectDoc>, String> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    for root in &config.scan_roots {
        discover(Path::new(root), config.max_depth, &config.excludes, &mut dirs);
    }
    let discovered: std::collections::HashSet<String> =
        dirs.iter().map(|p| normalize(p)).collect();

    let mut docs: Vec<LocalProjectDoc> = dirs.iter().map(|d| read_project(d, false)).collect();
    // Pins: included even without a marker; skip a pin already auto-discovered.
    for pin in &config.pins {
        let norm = pin.replace('\\', "/");
        if discovered.contains(&norm) {
            continue;
        }
        docs.push(read_project(Path::new(pin), true));
    }
    Ok(docs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp() -> PathBuf {
        let d = std::env::temp_dir().join(format!("cc-scan-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }
    fn make_project(root: &Path, rel: &str, status: &str) {
        let dir = root.join(rel);
        fs::create_dir_all(dir.join("docs")).unwrap();
        fs::write(dir.join("docs/STATUS.md"), status).unwrap();
    }

    #[test]
    fn discovers_nested_marked_projects_and_prunes() {
        let root = tmp();
        make_project(&root, "alpha", "---\nstage: Build\n---\n");
        make_project(&root, "mono/services/api", "---\nstage: Spec\n---\n"); // depth-3 nested
        fs::create_dir_all(root.join("node_modules/pkg/docs")).unwrap();
        fs::write(root.join("node_modules/pkg/docs/STATUS.md"), "x").unwrap(); // must be pruned
        let cfg = ScanConfig { scan_roots: vec![root.to_string_lossy().into()], max_depth: 5, pins: vec![], excludes: vec![] };
        let docs = scan_local_projects(cfg).unwrap();
        let dirs: Vec<&str> = docs.iter().map(|d| d.project_dir.as_str()).collect();
        assert!(dirs.iter().any(|d| d.ends_with("/alpha")));
        assert!(dirs.iter().any(|d| d.ends_with("/mono/services/api")));
        assert!(!dirs.iter().any(|d| d.contains("node_modules")));
    }

    #[test]
    fn hashes_roadmap_over_raw_bytes() {
        let root = tmp();
        make_project(&root, "beta", "---\nstage: Build\n---\n");
        fs::write(root.join("beta/ROADMAP.md"), "## X\n<!-- cc-item id=x status=open -->\n").unwrap();
        let cfg = ScanConfig { scan_roots: vec![root.to_string_lossy().into()], max_depth: 5, pins: vec![], excludes: vec![] };
        let docs = scan_local_projects(cfg).unwrap();
        let beta = docs.iter().find(|d| d.project_dir.ends_with("/beta")).unwrap();
        assert!(beta.roadmap_hash.as_ref().unwrap().len() == 64); // hex sha256
    }

    #[test]
    fn pinned_unmarked_dir_is_included() {
        let root = tmp();
        let pin = root.join("pinned-no-marker");
        fs::create_dir_all(&pin).unwrap();
        let cfg = ScanConfig { scan_roots: vec![], max_depth: 5, pins: vec![pin.to_string_lossy().into()], excludes: vec![] };
        let docs = scan_local_projects(cfg).unwrap();
        assert_eq!(docs.len(), 1);
        assert!(docs[0].is_pinned);
        assert!(docs[0].status_text.is_none());
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cd cockpit/ui/src-tauri && cargo test local_projects`
Expected: FAIL — module not declared in `lib.rs` (compile error) or tests fail before registration.

- [ ] **Step 4: Register the module + command**

In `cockpit/ui/src-tauri/src/lib.rs`: add near the other `mod` lines:
```rust
mod local_projects;
```
and inside `tauri::generate_handler![ … ]`, add `local_projects::scan_local_projects,` to the list.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cd cockpit/ui/src-tauri && cargo test local_projects`
Expected: PASS (3 tests). Then `cargo build` to confirm the handler registration compiles.

- [ ] **Step 6: Commit**

```bash
git add cockpit/ui/src-tauri/src/local_projects.rs cockpit/ui/src-tauri/src/lib.rs cockpit/ui/src-tauri/Cargo.toml cockpit/ui/src-tauri/Cargo.lock
git commit -m "feat(dashboard): scan_local_projects Tauri command (U4 Rust half)"
```

---

### Task 5: Wire the local source into the store + board

**Files:**
- Modify: `cockpit/ui/src/lib/dashboard/store.ts` (add `pollLocal`)
- Test: `cockpit/ui/src/lib/dashboard/store.test.ts` (add a `pollLocal` case)
- Modify: `cockpit/ui/src/views/Dashboard.svelte` (`SOURCE_LABEL['local']`, wire a `LocalReader`, poll it in `refresh`)
- Test: `cockpit/ui/src/lib/dashboard/adapters/local.test.ts` already covers the footer "declared Nd ago" detail; no new render test framework is introduced here.

**Interfaces:**
- Consumes: `localCards`, `LocalReader` (Task 3); `replaceSource` (store).
- Produces: `pollLocal(state, reader, now?) => Promise<BoardState>`.

- [ ] **Step 1: Write the failing store test**

Add to `cockpit/ui/src/lib/dashboard/store.test.ts`:
```ts
import { pollLocal, newBoard, cardList } from './store';
import type { LocalReader } from './adapters/local';

it('pollLocal replaces only the local source', async () => {
  const reader: LocalReader = { scan: async () => [
    { projectDir: 'D:/p/one', statusText: '---\nstage: Build\n---\n', isPinned: false },
  ] };
  const board = await pollLocal(newBoard(), reader, () => new Date('2026-07-10T00:00:00Z'));
  const cards = cardList(board);
  expect(cards).toHaveLength(1);
  expect(cards[0].source).toBe('local');
  expect(cards[0].projectId).toBe('local:D--p-one');
});
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cd cockpit/ui && npx vitest run src/lib/dashboard/store.test.ts`
Expected: FAIL — `pollLocal` is not exported.

- [ ] **Step 3: Implement `pollLocal`**

In `cockpit/ui/src/lib/dashboard/store.ts`, add the import and the function alongside `pollHalyard`:
```ts
import { localCards, type LocalReader } from './adapters/local';

export async function pollLocal(
  state: BoardState,
  reader: LocalReader,
  now: () => Date = () => new Date(),
): Promise<BoardState> {
  const cards = await localCards(reader, { now });
  return replaceSource(state, 'local', cards);
}
```

- [ ] **Step 4: Run the store test to verify it passes**

Run: `cd cockpit/ui && npx vitest run src/lib/dashboard/store.test.ts`
Expected: PASS.

- [ ] **Step 5: Wire the board (SOURCE_LABEL + reader + poll)**

In `cockpit/ui/src/views/Dashboard.svelte`:

(a) add the label to `SOURCE_LABEL`:
```ts
    manual: 'MANUAL',
    local: 'LOCAL',
```

(b) add a `localReader?: LocalReader` prop (beside `halyardReader`) and, in `refresh()`, after the audience poll:
```ts
      if (localReader) board = await pollLocal(board, localReader);
```
importing `pollLocal` from the store and `type LocalReader` from `../lib/dashboard/adapters/local`.

(c) In `App.svelte` (the real shell that wires readers), provide a `localReader` whose `scan()` invokes the Tauri command:
```ts
const localReader = {
  scan: () => import('@tauri-apps/api/core').then((m) =>
    m.invoke('scan_local_projects', { config: { scanRoots: ['D:/MajorProjects'], maxDepth: 5, pins: [], excludes: [] } })),
};
```
(The `detail` already renders the "declared Nd ago" hint via the adapter; no separate footer markup is required because `.detail` is shown at `Dashboard.svelte:195`.)

- [ ] **Step 6: Verify the whole suite is green**

Run: `cd cockpit/ui && npx vitest run && cd src-tauri && cargo test`
Expected: all TS + Rust tests pass. Then `npm run tauri build` (or `tauri dev`) to confirm the board shows local project cards from `D:/MajorProjects`.

- [ ] **Step 7: Commit**

```bash
git add cockpit/ui/src/lib/dashboard/store.ts cockpit/ui/src/lib/dashboard/store.test.ts cockpit/ui/src/views/Dashboard.svelte cockpit/ui/src/App.svelte
git commit -m "feat(dashboard): wire local source into store + board (U4)"
```

---

### Task 6: U3 — STATUS.md front-matter stamping in the session-wrap skills

**Files:**
- Modify: `plugins/session-state/` capture path (the in-repo `save-state` producer) to stamp/refresh the front-matter when it writes `docs/STATUS.md`.
- Modify (out-of-repo, documented as a manual step): the global `end-session` / `handoff` skills and the `~/.claude/CLAUDE.md` STATUS.md convention note.
- Create: `docs/superpowers/local-tracker-status-convention.md` (the authored convention the skills follow).

**Interfaces:**
- Consumes: the U1 front-matter shape (Task 1).
- Produces: `docs/STATUS.md` files that begin (byte 0, BOM-less) with the `stage`/`readiness`/`updated` front-matter the adapter reads.

- [ ] **Step 1: Author the convention doc**

Create `docs/superpowers/local-tracker-status-convention.md` documenting: the exact front-matter block (from spec §3.1), that it must be byte-0 and BOM-less, that a pre-existing H1 moves below it, and which skills maintain it. (Content = spec §3.1 verbatim + the "who writes it" note from §3.3.)

- [ ] **Step 2: Add front-matter stamping to the in-repo producer**

Locate where the `session-state` plugin writes the STATUS/narrative (grep `plugins/session-state/src` for the STATUS/`docs` writer). Add a helper that, before writing, ensures the file starts with the front-matter block — inserting it (with the current `stage`/`readiness`/`updated`) if absent, or updating `stage`/`updated` in place if present — and relocates any leading `# H1` below the block. Write BOM-less UTF-8.

- [ ] **Step 3: Add a test for the stamper**

In the plugin's `test/` dir, add a test: given a STATUS.md with no front-matter and a leading `# Title`, the stamper produces a byte-0 `---\nstage: <x>\n…\n---\n# Title\n`; given one that already has front-matter, it updates `stage`/`updated` without duplicating the block. Run the plugin's test command (`node --test` per its suite) and confirm PASS.

- [ ] **Step 4: Document the out-of-repo skill edits**

In the convention doc, add a "Manual wiring" section noting that `end-session` and `handoff` (global skills at `~/.claude/skills/`) and the `~/.claude/CLAUDE.md` STATUS.md convention must gain the same stamping step — these live outside this repo and are applied by hand. (This keeps the plan honest: the in-repo `save-state` path is tested here; the global skills are a documented follow-up.)

- [ ] **Step 5: Commit**

```bash
git add docs/superpowers/local-tracker-status-convention.md plugins/session-state/
git commit -m "feat(session-state): stamp STATUS.md stage front-matter (U3)"
```

---

## Self-Review

- **Spec coverage:** U1 (§3.1) → Task 1; U2 (§3.2) → Task 2; U4 adapter (§5/§6) → Task 3; U4 Rust scan/discovery (§4/§6) → Task 4; store+board wiring (§2/§6) → Task 5; U3 skill stamping (§3.3) → Task 6. Phase-2-only items (dispatch, write-back, auth, `family`, reconcile) are intentionally excluded and belong to the Phase-2 plan.
- **Additive-model check:** `Source += 'local'` (Task 3 Step 1) and `RoadmapItem`/`dispatch?` (Task 2 Step 2) are the only `model.ts` changes; no `stage.ts` change; `applyOverride` is never called for local cards (adapter emits resolved cards) — matches the Global Constraints.
- **Type consistency:** `LocalProjectDoc` fields align between the Rust serialize (`#[serde(rename_all="camelCase")]`, Task 4) and the TS interface (Task 3): `projectDir`, `statusText`, `roadmapText`, `roadmapHash`, `isPinned`. `pollLocal(state, reader, now?)` matches its call in Dashboard (Task 5). `parseStatusFrontmatter`/`parseRoadmapItems` signatures match their adapter consumption.
- **Placeholder scan:** no TBD/TODO; every code step shows complete code. Task 6 Step 2 references "grep for the writer" rather than a fixed line because the plugin's writer path is discovered at execution — the action and acceptance are concrete.
- **Known simplification:** `statusMtimeMs`/`roadmapMtimeMs` from the spec's §6.3 mtime-gating are declared in the TS `LocalProjectDoc` but the Rust command returns the always-read form in Phase 1; the incremental mtime-skip is a cheap follow-up and is not required for correctness. Flagged rather than hidden.
