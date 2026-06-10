import { describe, it, expect } from 'vitest';
import { flushSync } from 'svelte';
import { FleetStore } from './store.svelte';
import { newUnit } from './fleet';
import type { Envelope } from './types';
import type { CreateReq } from './api';

// Lane O: the host-overlay trigger lives entirely in host-owned fleet state, so these
// tests drive the store directly (no view/plugin, no network) and assert the derived
// `awaitingApproval` + the staged-launch flow that App.svelte renders as modals.

function seed(store: FleetStore, id: string, task = 'build the thing') {
  store.units[id] = newUnit(id, task, 'T1');
  store.order = [...store.order, id];
}

function evt(unitId: string, seq: number, event: Envelope['event']): Envelope {
  return { unit_id: unitId, seq, event };
}

describe('FleetStore host-overlay trigger (awaitingApproval)', () => {
  it('is null until a unit enters awaiting_oracle_approval, regardless of selection', () => {
    const s = new FleetStore();
    const cleanup = $effect.root(() => {
      seed(s, 'a');
      seed(s, 'b');
      // Select a DIFFERENT unit than the one that will await approval — the trigger must
      // fire off fleet state, not the current selection / active view.
      s.select('a');
      flushSync();
      expect(s.awaitingApproval).toBeNull();

      s.onEvt('b', evt('b', 1, { type: 'phase_changed', from: 'spec', to: 'awaiting_oracle_approval' }));
      flushSync();
      expect(s.awaitingApproval?.id).toBe('b');
      expect(s.selectedId).toBe('a'); // selection untouched
    });
    cleanup();
  });

  it('surfaces the unit with its frozen test set (oracleFiles)', () => {
    const s = new FleetStore();
    const cleanup = $effect.root(() => {
      seed(s, 'a');
      s.onEvt('a', evt('a', 1, { type: 'oracle_proposed', test_files: ['t/a.test.js'], hash: 'h', summary: 's' }));
      s.onEvt('a', evt('a', 2, { type: 'phase_changed', from: 'spec', to: 'awaiting_oracle_approval' }));
      flushSync();
      expect(s.awaitingApproval?.oracleFiles).toEqual(['t/a.test.js']);
    });
    cleanup();
  });

  it('clears once the unit leaves the awaiting phase (approve/reject resolved)', () => {
    const s = new FleetStore();
    const cleanup = $effect.root(() => {
      seed(s, 'a');
      s.onEvt('a', evt('a', 1, { type: 'phase_changed', from: 'spec', to: 'awaiting_oracle_approval' }));
      flushSync();
      expect(s.awaitingApproval?.id).toBe('a');

      s.onEvt('a', evt('a', 2, { type: 'phase_changed', from: 'awaiting_oracle_approval', to: 'building' }));
      flushSync();
      expect(s.awaitingApproval).toBeNull();
    });
    cleanup();
  });
});

describe('FleetStore REAL-launch confirm flow', () => {
  const req: CreateReq = { task: 'ship it', tier: 't2', mode: 'real', min_review_rounds: 3 };

  it('requestRealLaunch stages a pending launch without hitting the daemon', () => {
    const s = new FleetStore();
    expect(s.pendingLaunch).toBeNull();
    s.requestRealLaunch(req);
    expect(s.pendingLaunch).toEqual(req);
    // No unit created yet — nothing was sent.
    expect(s.order).toHaveLength(0);
  });

  it('cancelLaunch clears the pending launch and sends nothing', () => {
    const s = new FleetStore();
    s.requestRealLaunch(req);
    s.cancelLaunch();
    expect(s.pendingLaunch).toBeNull();
    expect(s.order).toHaveLength(0);
  });

  it('confirmLaunch with nothing staged is a no-op returning null', async () => {
    const s = new FleetStore();
    await expect(s.confirmLaunch()).resolves.toBeNull();
  });
});
