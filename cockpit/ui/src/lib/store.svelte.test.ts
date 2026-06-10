import { describe, it, expect } from 'vitest';
import { flushSync } from 'svelte';
import { FleetStore } from './store.svelte';
import { newUnit } from './fleet';
import type { Envelope } from './types';

// Seed a unit directly into a fresh store (bypasses the network `reconnect`/`launch`
// paths, which we don't exercise here — jsdom has no fleetd).
function seed(store: FleetStore, id = 'u1') {
  store.units[id] = newUnit(id, 'build the thing', 'T1');
  store.order = [...store.order, id];
  store.selectedId ??= id;
}

function evt(unitId: string, seq: number, event: Envelope['event']): Envelope {
  return { unit_id: unitId, seq, event };
}

describe('FleetStore', () => {
  it('folds stream events into the addressed unit and dedups replays', () => {
    const s = new FleetStore();
    seed(s);
    s.onEvt('u1', evt('u1', 1, { type: 'phase_changed', from: 'queued', to: 'building' }));
    expect(s.units['u1'].phase).toBe('building');
    expect(s.units['u1'].lastSeq).toBe(1);

    // A frame at/below lastSeq is ignored (replayed/overlapping).
    s.onEvt('u1', evt('u1', 1, { type: 'phase_changed', from: 'building', to: 'failed' }));
    expect(s.units['u1'].phase).toBe('building');
  });

  it('broadcasts phase_changed to onPhase subscribers (the Dashboard seam)', () => {
    const s = new FleetStore();
    seed(s);
    const seen: Array<[string, string, string, string]> = [];
    const off = s.onPhase((id, to, task, tier) => seen.push([id, to, task, tier]));

    s.onEvt('u1', evt('u1', 1, { type: 'phase_changed', from: 'queued', to: 'spec' }));
    expect(seen).toEqual([['u1', 'spec', 'build the thing', 'T1']]);

    // A non-phase event does not notify.
    s.onEvt('u1', evt('u1', 2, { type: 'log', stream: 'agent', line: 'hi' }));
    expect(seen.length).toBe(1);

    // Unsubscribe stops delivery.
    off();
    s.onEvt('u1', evt('u1', 3, { type: 'phase_changed', from: 'spec', to: 'building' }));
    expect(seen.length).toBe(1);
  });

  it('projects current units as GET /units-shaped snapshots (Dashboard fleet seed)', () => {
    const s = new FleetStore();
    seed(s);
    s.onEvt('u1', evt('u1', 1, { type: 'metric', tokens_in: 10, tokens_out: 20, cost_usd: 0.5, elapsed_ms: 1 }));
    const snaps = s.snapshots();
    expect(snaps).toHaveLength(1);
    expect(snaps[0]).toMatchObject({ unit_id: 'u1', task: 'build the thing', tier: 'T1', cost: 0.5, last_seq: 1 });
  });

  it('keeps derived projections in sync (single source of fleet truth)', () => {
    const s = new FleetStore();
    const cleanup = $effect.root(() => {
      seed(s, 'a');
      seed(s, 'b');
      flushSync();
      expect(s.list.map((u) => u.id)).toEqual(['a', 'b']);
      expect(s.activeCount).toBe(2); // both non-terminal

      s.onEvt('a', evt('a', 1, { type: 'phase_changed', from: 'queued', to: 'done' }));
      flushSync();
      expect(s.activeCount).toBe(1); // 'a' is now terminal

      s.select('b');
      flushSync();
      expect(s.selected?.id).toBe('b');
    });
    cleanup();
  });

  it('cmd no-ops when there is no selection and never throws', async () => {
    const s = new FleetStore();
    // sendCommand would hit the network; with no selection it must short-circuit first.
    await expect(s.cmd('halt', null)).resolves.toBeUndefined();
  });
});
