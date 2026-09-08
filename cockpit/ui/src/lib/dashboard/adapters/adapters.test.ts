import { describe, it, expect } from 'vitest';
import { halyardCards, type HalyardReader } from './halyard';
import { audienceCards, type AudienceReader } from './audience';
import { fleetCard, fleetCardsFromSnapshots } from './fleet';
import { appPluginCard, appPluginCards } from './appPlugin';
import type { Snapshot } from '../../types';

const NOW = () => new Date('2026-06-09T12:00:00Z');

// ── §6.1 Halyard ────────────────────────────────────────────────────────────
describe('halyard adapter', () => {
  function reader(statuses: any[], queue: any[] = []): HalyardReader {
    return { status: async () => statuses, queue: async () => queue };
  }

  it('maps the release-state enum onto canonical stages', async () => {
    const cases: Array<[string, string]> = [
      ['tagged', 'Build'],
      ['built', 'Build'],
      ['tested', 'Review'],
      ['uploaded', 'Review'],
      ['in_review', 'Review'],
      ['live', 'Live'],
      ['rolled_back', 'Archived'],
    ];
    for (const [state, expected] of cases) {
      const [card] = await halyardCards(
        reader([{ release_id: 'r1', app: 'aurora', surface: 'web', version: '4.2', state, flag: null, waiting_on: '' }]),
        { now: NOW },
      );
      expect(card.stage, state).toBe(expected);
    }
  });

  it('dead/rejected → Failed (terminal precedence)', async () => {
    for (const state of ['dead', 'rejected']) {
      const [card] = await halyardCards(
        reader([{ release_id: 'r1', app: 'aurora', surface: 'web', version: '4.2', state, flag: null, waiting_on: '' }]),
        { now: NOW },
      );
      expect(card.stage).toBe('Failed');
    }
  });

  it('shipped_dark is Blocked on the flip gate, pipeline kept in detail', async () => {
    const [card] = await halyardCards(
      reader([{ release_id: 'r1', app: 'aurora', surface: 'web', version: '4.2', state: 'shipped_dark', flag: 'aurora_42', waiting_on: '' }]),
      { now: NOW },
    );
    expect(card.stage).toBe('Blocked');
    expect(card.blocked?.gate).toBe('flip');
    expect(card.detail).toContain('shipped_dark');
  });

  it('an open proposal in the queue blocks the release (gate: approval)', async () => {
    const [card] = await halyardCards(
      reader(
        [{ release_id: 'r1', app: 'aurora', surface: 'web', version: '4.2', state: 'in_review', flag: null, waiting_on: '' }],
        [{ proposal_id: 'p1', kind: 'social_post', app: 'aurora', release_id: 'r1', title: 'X post', status: 'open' }],
      ),
      { now: NOW },
    );
    expect(card.stage).toBe('Blocked');
    expect(card.blocked?.gate).toBe('approval');
    expect(card.detail).toContain('awaiting approval');
    expect(card.projectId).toBe('halyard:aurora:r1');
  });

  it('degrades to a single unknown card when the CLI read throws', async () => {
    const broken: HalyardReader = {
      status: async () => { throw new Error('halyard not found'); },
      queue: async () => [],
    };
    const cards = await halyardCards(broken, { now: NOW });
    expect(cards).toHaveLength(1);
    expect(cards[0].health).toBe('unknown');
    expect(cards[0].stage).toBe('Idle');
    expect(cards[0].detail).toContain('unreachable');
  });
});

// ── §6.2 Audience ─────────────────────────────────────────────────────────────
describe('audience adapter', () => {
  function reader(alive: boolean, posts: any[] = []): AudienceReader {
    return { health: async () => alive, posts: async () => posts };
  }

  // Every status below is one Audience actually emits, taken from its generated
  // PostStatusSchema and pinned in audience.contract.test.ts. The previous version of
  // this block asserted on 'published', 'rejected' and 'approval-pending' — three
  // strings Audience has never emitted — so it was green against an invention.
  it('maps every real post status onto a canonical stage', async () => {
    const cases: Array<[string, string]> = [
      ['draft', 'Spec'],
      ['generating', 'Build'],
      ['ready_for_review', 'Blocked'],
      ['awaiting_approval', 'Blocked'],
      ['approved', 'Build'],
      ['publishing', 'Build'],
      ['fully_published', 'Live'],
      ['partially_published', 'Blocked'],
      ['failed', 'Failed'],
    ];
    for (const [status, expected] of cases) {
      const cards = await audienceCards(reader(true, [{ id: '1', status }]), { now: NOW });
      expect(cards[0].stage, status).toBe(expected);
    }
  });

  it('awaiting_approval → Blocked (the approve-before-post gate)', async () => {
    // The defect this replaces: Audience emits `awaiting_approval`, the adapter
    // looked for `approval-pending`, so the one gate that must stop for a human
    // fell through to `default` and never fired.
    const cards = await audienceCards(reader(true, [{ id: '1', status: 'awaiting_approval' }]), { now: NOW });
    expect(cards[0].stage).toBe('Blocked');
    expect(cards[0].blocked?.gate).toBe('approval');
  });

  it('a status absent from the contract classifies as nothing, and does not throw', async () => {
    const cards = await audienceCards(reader(true, [{ id: '1', status: 'a_status_from_the_future' }]), { now: NOW });
    expect(cards[0].stage).not.toBe('Blocked');
    expect(cards[0].stage).not.toBe('Live');
  });

  it('failed → Failed', async () => {
    const cards = await audienceCards(reader(true, [{ id: '1', status: 'failed' }]), { now: NOW });
    expect(cards[0].stage).toBe('Failed');
  });

  it('backend /health down → one Idle + unknown card (§6.2)', async () => {
    const cards = await audienceCards(reader(false), { now: NOW });
    expect(cards).toHaveLength(1);
    expect(cards[0].stage).toBe('Idle');
    expect(cards[0].health).toBe('unknown');
  });

  it('rolls up past the threshold into one card', async () => {
    const posts = Array.from({ length: 12 }, (_, i) => ({ id: `${i}`, status: i < 3 ? 'awaiting_approval' : 'fully_published' }));
    const cards = await audienceCards(reader(true, posts), { now: NOW, rollupThreshold: 8 });
    expect(cards).toHaveLength(1);
    expect(cards[0].stage).toBe('Blocked');
    expect(cards[0].detail).toContain('3 awaiting approval');
  });
});

// ── §6.3 Fleet ────────────────────────────────────────────────────────────────
describe('fleet adapter', () => {
  it('maps mission phases onto canonical stages', () => {
    const cases: Array<[any, string]> = [
      ['queued', 'Plan'],
      ['provisioning', 'Plan'],
      ['spec', 'Spec'],
      ['building', 'Build'],
      ['checking', 'Build'],
      ['reviewing', 'Review'],
      ['merge_check', 'Review'],
      ['pr_open', 'Ship'],
      ['done', 'Ship'],
      ['no_change', 'Archived'],
    ];
    for (const [phase, expected] of cases) {
      const card = fleetCard({ id: 'u1', phase, task: 't', tier: 'T1' }, { now: NOW });
      expect(card.stage, phase).toBe(expected);
    }
  });

  it('awaiting_oracle_approval → Blocked (oracle-approval gate)', () => {
    const card = fleetCard({ id: 'u1', phase: 'awaiting_oracle_approval', task: 't', tier: 'T2' }, { now: NOW });
    expect(card.stage).toBe('Blocked');
    expect(card.blocked?.gate).toBe('oracle-approval');
    expect(card.blocked?.deepLink).toContain('u1');
  });

  it('needs_human / halted → Blocked (manual gate)', () => {
    for (const phase of ['needs_human', 'halted'] as const) {
      const card = fleetCard({ id: 'u1', phase, task: 't', tier: 'T3' }, { now: NOW });
      expect(card.stage).toBe('Blocked');
      expect(card.blocked?.gate).toBe('manual');
    }
  });

  it('failed → Failed', () => {
    expect(fleetCard({ id: 'u1', phase: 'failed', task: 't', tier: 'T1' }, { now: NOW }).stage).toBe('Failed');
  });

  it('builds cards from snapshots', () => {
    const snaps: Snapshot[] = [
      { unit_id: 'u1', phase: 'building', cost: 0, usd_cap: 5, tier: 't1', task: 'build x', last_seq: 0 },
    ];
    const cards = fleetCardsFromSnapshots(snaps, { now: NOW });
    expect(cards[0].projectId).toBe('fleet:u1');
    expect(cards[0].stage).toBe('Build');
  });
});

// ── §6.4 App-plugin ───────────────────────────────────────────────────────────
describe('app-plugin adapter', () => {
  it('maps plugin://state onto the operational axis', () => {
    const cases: Array<[any, string]> = [
      ['stopped', 'Idle'],
      ['building', 'Build'],
      ['starting', 'Build'],
      ['health-probing', 'Build'],
      ['ready-probing', 'Build'],
      ['healthy', 'Live'],
    ];
    for (const [state, expected] of cases) {
      const card = appPluginCard({ id: 'audience', name: 'Audience', state }, { now: NOW });
      expect(card.stage, state).toBe(expected);
    }
  });

  it('error → Failed with degraded health and the stderr line in detail', () => {
    const card = appPluginCard({ id: 'audience', name: 'Audience', state: 'error', errorLine: 'port in use' }, { now: NOW });
    expect(card.stage).toBe('Failed');
    expect(card.health).toBe('degraded');
    expect(card.detail).toContain('port in use');
  });

  it('carries a family tag so it groups with the work-axis card', () => {
    const card = appPluginCard({ id: 'audience', name: 'Audience', state: 'healthy', url: 'http://localhost:3000' }, { now: NOW });
    expect(card.family).toBe('audience');
    expect(card.url).toBe('http://localhost:3000');
  });

  it('empty list → no cards (graceful absence; awaits feat/app-plugins)', () => {
    expect(appPluginCards([], { now: NOW })).toEqual([]);
  });
});
