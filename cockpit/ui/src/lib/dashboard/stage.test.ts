import { describe, it, expect } from 'vitest';
import { resolveStage, applyOverride, isOverrideExpired, DEFAULT_OVERRIDE_TTL_HOURS } from './stage';
import type { StageOverride } from './model';

describe('resolveStage — §3 precedence (Failed > Blocked > pipeline > Idle)', () => {
  it('Failed beats everything', () => {
    expect(resolveStage({ pipeline: 'Review', isHumanGate: true, isTerminalFailure: true })).toBe('Failed');
  });

  it('Blocked beats a pipeline position (a human gate wins over Review)', () => {
    expect(resolveStage({ pipeline: 'Review', isHumanGate: true, isTerminalFailure: false })).toBe('Blocked');
  });

  it('a pipeline position survives when not gated/failed', () => {
    expect(resolveStage({ pipeline: 'Build', isHumanGate: false, isTerminalFailure: false })).toBe('Build');
  });

  it('falls back to Idle when there is no pipeline position', () => {
    expect(resolveStage({ pipeline: null, isHumanGate: false, isTerminalFailure: false })).toBe('Idle');
  });
});

describe('override TTL — §4 anti-rot', () => {
  const now = Date.parse('2026-06-09T12:00:00Z');

  it('an unexpired override is live', () => {
    const o: StageOverride = { stage: 'Live', reason: 'r', setBy: 'op', setAtIso: '2026-06-09T11:00:00Z', ttlHours: 72 };
    expect(isOverrideExpired(o, now)).toBe(false);
  });

  it('an override past its TTL is expired', () => {
    const o: StageOverride = { stage: 'Live', reason: 'r', setBy: 'op', setAtIso: '2026-06-01T00:00:00Z', ttlHours: 72 };
    expect(isOverrideExpired(o, now)).toBe(true);
  });

  it('defaults to a 72h TTL when none is set', () => {
    const justUnder: StageOverride = { stage: 'Live', reason: 'r', setBy: 'op', setAtIso: '2026-06-06T13:00:00Z' };
    const justOver: StageOverride = { stage: 'Live', reason: 'r', setBy: 'op', setAtIso: '2026-06-06T11:00:00Z' };
    expect(DEFAULT_OVERRIDE_TTL_HOURS).toBe(72);
    expect(isOverrideExpired(justUnder, now)).toBe(false);
    expect(isOverrideExpired(justOver, now)).toBe(true);
  });

  it('treats an unparseable stamp as expired', () => {
    const o: StageOverride = { stage: 'Live', reason: 'r', setBy: 'op', setAtIso: 'not-a-date' };
    expect(isOverrideExpired(o, now)).toBe(true);
  });
});

describe('applyOverride — §4 inferred-with-declared-override', () => {
  const now = Date.parse('2026-06-09T12:00:00Z');

  it('with no override, returns the inferred stage', () => {
    const r = applyOverride('Build', null, now);
    expect(r).toEqual({ stage: 'Build', stageSource: 'inferred', override: null, conflict: null });
  });

  it('a live override wins and is stamped declared', () => {
    const o: StageOverride = { stage: 'Archived', reason: 'manual rollback', setBy: 'op', setAtIso: '2026-06-09T11:00:00Z' };
    const r = applyOverride('Live', o, now);
    expect(r.stage).toBe('Archived');
    expect(r.stageSource).toBe('declared');
    expect(r.override).toBe(o);
  });

  it('surfaces a conflict chip when declared and inferred diverge', () => {
    const o: StageOverride = { stage: 'Archived', reason: 'r', setBy: 'op', setAtIso: '2026-06-09T11:00:00Z' };
    const r = applyOverride('Live', o, now);
    expect(r.conflict).toEqual({ declared: 'Archived', inferred: 'Live' });
  });

  it('no conflict when declared matches inferred', () => {
    const o: StageOverride = { stage: 'Live', reason: 'r', setBy: 'op', setAtIso: '2026-06-09T11:00:00Z' };
    expect(applyOverride('Live', o, now).conflict).toBeNull();
  });

  it('an expired override is dropped and reverts to inferred (R2 fix #6)', () => {
    const o: StageOverride = { stage: 'Archived', reason: 'r', setBy: 'op', setAtIso: '2026-06-01T00:00:00Z' };
    const r = applyOverride('Live', o, now);
    expect(r.stage).toBe('Live');
    expect(r.stageSource).toBe('inferred');
  });
});
