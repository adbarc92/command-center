import { describe, it, expect } from 'vitest';
import {
  newBoard,
  pollHalyard,
  pollAudience,
  seedFleet,
  applyFleetPhase,
  applyPluginState,
  cardList,
  sortedCards,
  blockedCount,
  isStale,
  replaceSource,
} from './store';
import type { HalyardReader } from './adapters/halyard';
import type { AudienceReader } from './adapters/audience';
import type { Snapshot } from '../types';

const NOW = () => new Date('2026-06-09T12:00:00Z');
const NOW_MS = Date.parse('2026-06-09T12:00:00Z');

const halyardReader = (statuses: any[], queue: any[] = []): HalyardReader => ({
  status: async () => statuses,
  queue: async () => queue,
});
const audienceReader = (alive: boolean, posts: any[] = []): AudienceReader => ({
  health: async () => alive,
  posts: async () => posts,
});

describe('board composition', () => {
  it('composes cards from multiple sources, keyed by projectId', async () => {
    let board = newBoard();
    board = await pollHalyard(
      board,
      halyardReader([{ release_id: 'r1', app: 'aurora', surface: 'web', version: '4.2', state: 'live', flag: null, waiting_on: '' }]),
      {},
      NOW,
    );
    board = seedFleet(
      board,
      [{ unit_id: 'u1', phase: 'building', cost: 0, usd_cap: 5, tier: 't1', task: 'x', last_seq: 0 }],
      {},
      NOW,
    );
    const ids = cardList(board).map((c) => c.projectId);
    expect(ids).toContain('halyard:aurora:r1');
    expect(ids).toContain('fleet:u1');
  });

  it('a down source greys ONLY its lane; other sources are unaffected', async () => {
    let board = newBoard();
    // Halyard healthy
    board = await pollHalyard(
      board,
      halyardReader([{ release_id: 'r1', app: 'aurora', surface: 'web', version: '4.2', state: 'live', flag: null, waiting_on: '' }]),
      {},
      NOW,
    );
    // Audience backend DOWN
    board = await pollAudience(board, audienceReader(false), {}, NOW);

    const halyard = cardList(board).find((c) => c.source === 'halyard')!;
    const audience = cardList(board).find((c) => c.source === 'audience')!;
    expect(halyard.health).toBe('ok'); // unaffected
    expect(halyard.stage).toBe('Live');
    expect(audience.health).toBe('unknown'); // greyed
  });

  it('re-polling a source replaces only its cards', async () => {
    let board = newBoard();
    board = await pollHalyard(
      board,
      halyardReader([{ release_id: 'r1', app: 'aurora', surface: 'web', version: '4.2', state: 'built', flag: null, waiting_on: '' }]),
      {},
      NOW,
    );
    board = seedFleet(board, [{ unit_id: 'u1', phase: 'building', cost: 0, usd_cap: 5, tier: 't1', task: 'x', last_seq: 0 }], {}, NOW);
    // second halyard poll with a different release set
    board = await pollHalyard(
      board,
      halyardReader([{ release_id: 'r2', app: 'beta', surface: 'web', version: '1.0', state: 'live', flag: null, waiting_on: '' }]),
      {},
      NOW,
    );
    const ids = cardList(board).map((c) => c.projectId);
    expect(ids).not.toContain('halyard:aurora:r1'); // replaced
    expect(ids).toContain('halyard:beta:r2');
    expect(ids).toContain('fleet:u1'); // untouched
  });
});

describe('live fleet phase advance (§6.3 push)', () => {
  it('a phase_changed event advances a mission card live', () => {
    let board = newBoard();
    const snaps: Snapshot[] = [{ unit_id: 'u1', phase: 'building', cost: 0, usd_cap: 5, tier: 't1', task: 'ship it', last_seq: 0 }];
    board = seedFleet(board, snaps, {}, NOW);
    expect(board.cards['fleet:u1'].stage).toBe('Build');

    // event: building → reviewing
    board = applyFleetPhase(board, 'u1', 'reviewing', { task: 'ship it', tier: 'T1' }, {}, NOW);
    expect(board.cards['fleet:u1'].stage).toBe('Review');

    // event: reviewing → awaiting_oracle_approval (blocks)
    board = applyFleetPhase(board, 'u1', 'needs_human', { task: 'ship it', tier: 'T1' }, {}, NOW);
    expect(board.cards['fleet:u1'].stage).toBe('Blocked');
  });
});

describe('live plugin state (§6.4 push)', () => {
  it('a plugin://state event updates the hosted-app card', () => {
    let board = newBoard();
    board = applyPluginState(board, { id: 'audience', name: 'Audience', state: 'building' }, {}, NOW);
    expect(board.cards['app-plugin:audience'].stage).toBe('Build');
    board = applyPluginState(board, { id: 'audience', name: 'Audience', state: 'healthy', url: 'http://localhost:3000' }, {}, NOW);
    expect(board.cards['app-plugin:audience'].stage).toBe('Live');
  });
});

describe('selectors', () => {
  it('blockedCount is the "needs me" number', async () => {
    let board = newBoard();
    board = await pollHalyard(
      board,
      halyardReader(
        [{ release_id: 'r1', app: 'aurora', surface: 'web', version: '4.2', state: 'shipped_dark', flag: 'f', waiting_on: '' }],
      ),
      {},
      NOW,
    );
    board = applyFleetPhase(board, 'u1', 'needs_human', { task: 't', tier: 'T1' }, {}, NOW);
    expect(blockedCount(board)).toBe(2);
  });

  it('isStale greys a card past its freshness budget', () => {
    const card = {
      projectId: 'x', source: 'fleet' as const, name: 'n', stage: 'Build' as const, detail: '',
      blocked: null, stageSource: 'inferred' as const, override: null, conflict: null,
      updatedIso: '2026-06-09T11:00:00Z', staleAfterSec: 60, health: 'ok' as const,
    };
    expect(isStale(card, NOW_MS)).toBe(true); // an hour old, 60s budget
    expect(isStale({ ...card, updatedIso: '2026-06-09T11:59:30Z' }, NOW_MS)).toBe(false);
  });

  it('sortedCards puts Failed and Blocked first', async () => {
    let board = newBoard();
    board = replaceSource(board, 'fleet', [
      fleetCardStub('a', 'Live'),
      fleetCardStub('b', 'Blocked'),
      fleetCardStub('c', 'Failed'),
      fleetCardStub('d', 'Build'),
    ]);
    const stages = sortedCards(board).map((c) => c.stage);
    expect(stages[0]).toBe('Failed');
    expect(stages[1]).toBe('Blocked');
  });
});

function fleetCardStub(id: string, stage: any) {
  return {
    projectId: `fleet:${id}`, source: 'fleet' as const, name: id, stage, detail: '',
    blocked: null, stageSource: 'inferred' as const, override: null, conflict: null,
    updatedIso: '2026-06-09T12:00:00Z', staleAfterSec: 120, health: 'ok' as const,
  };
}
