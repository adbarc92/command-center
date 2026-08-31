import { describe, it, expect } from 'vitest';
import {
  createModel,
  applyState,
  applyLog,
  applyReset,
  recordAck,
  list,
  awaitingApproval,
  ticker,
  type UnitLite,
} from '../../../../plugins/reference/model.js';

// Lane V: proves the reference plugin's model applies the FULL message surface correctly
// (dirty `state` deltas, `log-append`, `log-reset`, `command-ack`) — "trivial" = small UI,
// not narrow coverage. The Tauri/iframe dogfood is Lane S's integration step.

function unit(id: string, phase = 'building'): UnitLite {
  return {
    id,
    task: 't',
    tier: 'T1',
    phase,
    history: [phase],
    cost: 0.5,
    usdCap: 5,
    tokensIn: 0,
    tokensOut: 0,
    iters: { build: 0, check: 0, review: 0 },
    findings: [],
    oracleFiles: [],
    awaitingSlot: false,
    rateLimited: false,
    lastSeq: 1,
  };
}

describe('reference plugin model', () => {
  it('a full snapshot seeds units; a delta merges only changed ones', () => {
    const m = createModel();
    applyState(m, { full: true, changed: [unit('u1'), unit('u2')], order: ['u1', 'u2'], degraded: false });
    expect(list(m).map((u) => u.id)).toEqual(['u1', 'u2']);

    // A delta touching only u2 must not drop u1.
    applyState(m, { full: false, changed: [unit('u2', 'done')], order: ['u1', 'u2'], degraded: true });
    expect(m.units['u1'].phase).toBe('building');
    expect(m.units['u2'].phase).toBe('done');
    expect(m.degraded).toBe(true);
  });

  it('a full snapshot resets the baseline (mirrors the host lastEmitted reset)', () => {
    const m = createModel();
    applyState(m, { full: true, changed: [unit('u1')], order: ['u1'], degraded: false });
    applyState(m, { full: true, changed: [unit('u2')], order: ['u2'], degraded: false });
    expect(Object.keys(m.units)).toEqual(['u2']);
  });

  it('log-append accumulates and the ticker is the last line; log-reset clears', () => {
    const m = createModel();
    applyLog(m, 'u1', [{ seq: 1, stream: 'agent', line: 'a' }]);
    applyLog(m, 'u1', [{ seq: 2, stream: 'check', line: 'b' }]);
    expect(m.logs['u1']).toHaveLength(2);
    expect(ticker(m, 'u1')).toEqual({ seq: 2, stream: 'check', line: 'b' });

    applyReset(m);
    expect(m.logs['u1']).toBeUndefined();
    expect(ticker(m, 'u1')).toBeNull();
  });

  it('detects awaiting_oracle_approval presence (non-interactive indicator source)', () => {
    const m = createModel();
    applyState(m, { full: true, changed: [unit('u1', 'building'), unit('u2', 'awaiting_oracle_approval')], order: ['u1', 'u2'], degraded: false });
    expect(awaitingApproval(m)?.id).toBe('u2');
  });

  it('records command-acks including a rejection', () => {
    const m = createModel();
    recordAck(m, { reqId: 'p0', ok: true });
    recordAck(m, { reqId: 'p1', ok: false, reasonClass: 'real-requires-confirm' });
    expect(m.acks).toHaveLength(2);
    expect(m.acks[1]).toMatchObject({ ok: false, reasonClass: 'real-requires-confirm' });
  });
});
