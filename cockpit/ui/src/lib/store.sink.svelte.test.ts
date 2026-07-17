import { describe, it, expect, vi, beforeEach } from 'vitest';

// Lane V: the store is the SINGLE command sink. These tests exercise the paths that touch
// the network (`launch`/`reconnect`), so `./api` is mocked — jsdom has no fleetd. The
// load-bearing assertion is the single-sink regression: a bridge `launch` concurrent with
// a `reconnect()` must yield exactly ONE unit and ONE socket (no duplicate-unit /
// double-socket, which a second optimistic writer would otherwise cause).

const createMission = vi.fn<(req: unknown) => Promise<string>>();
const listUnits = vi.fn<() => Promise<unknown[]>>();
const health = vi.fn<() => Promise<unknown>>();
const openStream = vi.fn(() => ({ close: vi.fn() }));
const sendCommand = vi.fn(async () => 202);

vi.mock('./api', () => ({
  createMission: (req: unknown) => createMission(req),
  listUnits: () => listUnits(),
  health: () => health(),
  openStream: (...args: unknown[]) => openStream(...(args as [])),
  sendCommand: (...args: unknown[]) => sendCommand(...(args as [])),
}));

// Imported AFTER the mock is registered.
const { FleetStore } = await import('./store.svelte');
import type { Envelope } from './types';

const req = { task: 'do the thing', tier: 't1' as const, mode: 'demo' as const, min_review_rounds: 2 };

function evt(unitId: string, seq: number, event: Envelope['event']): Envelope {
  return { unit_id: unitId, seq, event };
}

beforeEach(() => {
  vi.clearAllMocks();
  openStream.mockImplementation(() => ({ close: vi.fn() }));
  health.mockResolvedValue({ docker: true, anthropic_key: true, version: 'test' });
});

describe('FleetStore single command sink', () => {
  it('a bridge launch concurrent with reconnect yields exactly one unit + one socket', async () => {
    const s = new FleetStore();
    createMission.mockResolvedValue('u-race');
    // The daemon snapshot lists the very unit `launch` is optimistically inserting.
    listUnits.mockResolvedValue([
      { unit_id: 'u-race', phase: 'building', cost: 0, usd_cap: 5, tier: 't1', task: 'do the thing', last_seq: 0 },
    ]);

    // Fire both writers at once — the exact interleaving must not matter.
    await Promise.all([s.launch(req), s.reconnect()]);

    expect(s.order.filter((id) => id === 'u-race')).toHaveLength(1);
    expect(Object.keys(s.units)).toEqual(['u-race']);
    expect(openStream).toHaveBeenCalledTimes(1);
  });

  it('reconnect and a later launch of the same id still open only one socket', async () => {
    const s = new FleetStore();
    listUnits.mockResolvedValue([
      { unit_id: 'u1', phase: 'queued', cost: 0, usd_cap: 5, tier: 't1', task: 't', last_seq: 0 },
    ]);
    await s.reconnect();
    expect(openStream).toHaveBeenCalledTimes(1);

    createMission.mockResolvedValue('u1'); // daemon hands back an already-known id
    await s.launch(req);
    expect(s.order.filter((id) => id === 'u1')).toHaveLength(1);
    expect(openStream).toHaveBeenCalledTimes(1); // no second socket
  });
});

describe('FleetStore dirty-set accumulator (bridge drain seam)', () => {
  it('records touched ids on fold and clears them on drain', async () => {
    const s = new FleetStore();
    createMission.mockResolvedValue('u1');
    await s.launch(req); // optimistic insert marks dirty
    expect(s.drainDirty()).toEqual(['u1']);
    expect(s.drainDirty()).toEqual([]); // drained set is now empty

    s.onEvt('u1', evt('u1', 1, { type: 'phase_changed', from: 'queued', to: 'building' }));
    s.onEvt('u1', evt('u1', 2, { type: 'metric', tokens_in: 1, tokens_out: 2, cost_usd: 0.1, elapsed_ms: 1 }));
    expect(s.drainDirty()).toEqual(['u1']); // deduped to the single touched id

    // A deduped/replayed frame (seq <= lastSeq) does not touch the unit.
    s.onEvt('u1', evt('u1', 2, { type: 'log', stream: 'agent', line: 'x' }));
    expect(s.drainDirty()).toEqual([]);
  });
});

describe('FleetStore seq-tagged log tail (log-append source)', () => {
  it('returns only lines after the cursor, with seq preserved', async () => {
    const s = new FleetStore();
    createMission.mockResolvedValue('u1');
    await s.launch(req);
    s.onEvt('u1', evt('u1', 1, { type: 'log', stream: 'agent', line: 'first' }));
    s.onEvt('u1', evt('u1', 2, { type: 'log', stream: 'check', line: 'second' }));

    expect(s.logsSince('u1', 0)).toEqual([
      { seq: 1, stream: 'agent', line: 'first' },
      { seq: 2, stream: 'check', line: 'second' },
    ]);
    expect(s.logsSince('u1', 1)).toEqual([{ seq: 2, stream: 'check', line: 'second' }]);
    expect(s.logsSince('u1', 2)).toEqual([]);
    expect(s.logsSince('unknown', 0)).toEqual([]);
  });
});
