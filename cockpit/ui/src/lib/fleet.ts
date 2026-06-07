// Folds the daemon event stream into per-unit view state.

import type { FleetEvent, Phase, Snapshot } from './types';

export interface LogLine {
  stream: string;
  line: string;
}
export interface Finding {
  round: number;
  title: string;
  severity: string;
  resolved: boolean;
}

export interface Unit {
  id: string;
  task: string;
  tier: string;
  phase: Phase;
  history: Phase[];
  cost: number;
  /** Per-unit USD ceiling (from the snapshot); drives the cost bar. */
  usdCap: number;
  tokensIn: number;
  tokensOut: number;
  iters: { build: number; check: number; review: number };
  log: LogLine[];
  findings: Finding[];
  oracleFiles: string[];
  branch?: string;
  pr?: string;
  blocked?: string;
  /** True while the unit is parked waiting for a concurrency slot. */
  awaitingSlot: boolean;
  /** True while the unit is backing off through an Anthropic rate-limit. */
  rateLimited: boolean;
  error?: string;
  result?: string;
  /** Highest event seq folded in, so reconnects replay only what's missing. */
  lastSeq: number;
}

export function newUnit(id: string, task: string, tier: string, usdCap = 5): Unit {
  return {
    id,
    task,
    tier,
    phase: 'queued',
    history: ['queued'],
    cost: 0,
    usdCap,
    tokensIn: 0,
    tokensOut: 0,
    iters: { build: 0, check: 0, review: 0 },
    log: [],
    findings: [],
    oracleFiles: [],
    awaitingSlot: false,
    rateLimited: false,
    lastSeq: 0,
  };
}

/**
 * Seed a unit from a persisted snapshot (on cockpit load). `lastSeq` stays 0 so
 * the WS stream replays the full event log to rebuild the detail view; the
 * snapshot just gives the tile its phase/cost/cap immediately.
 */
export function fromSnapshot(s: Snapshot): Unit {
  const u = newUnit(s.unit_id, s.task, s.tier.toUpperCase(), s.usd_cap);
  u.phase = s.phase;
  u.history = [s.phase];
  u.cost = s.cost;
  return u;
}

const ACTIVE: Phase[] = ['provisioning', 'spec', 'building', 'checking', 'reviewing', 'merge_check', 'pr_open'];
const ATTENTION: Phase[] = ['awaiting_oracle_approval', 'needs_human', 'halted'];
const GOOD: Phase[] = ['done', 'no_change'];

export function phaseClass(p: Phase): 'active' | 'attention' | 'good' | 'bad' | 'idle' {
  if (p === 'failed') return 'bad';
  if (GOOD.includes(p)) return 'good';
  if (ATTENTION.includes(p)) return 'attention';
  if (ACTIVE.includes(p)) return 'active';
  return 'idle';
}

export function isTerminal(p: Phase): boolean {
  return p === 'done' || p === 'no_change' || p === 'failed';
}

/** Approximate progress (0..1) along the happy path, for the tile rail. */
export function progress(p: Phase): number {
  const order: Phase[] = [
    'queued', 'provisioning', 'spec', 'awaiting_oracle_approval',
    'building', 'checking', 'reviewing', 'merge_check', 'pr_open', 'done',
  ];
  const i = order.indexOf(p);
  if (p === 'failed') return 1;
  return i < 0 ? 0 : i / (order.length - 1);
}

/** Apply one event to a unit (mutates and returns it). */
export function fold(u: Unit, ev: FleetEvent): Unit {
  switch (ev.type) {
    case 'phase_changed':
      u.phase = ev.to;
      u.history = [...u.history, ev.to];
      u.awaitingSlot = false;
      u.rateLimited = false; // any phase change means the wait resolved
      break;
    case 'oracle_proposed':
      u.oracleFiles = ev.test_files;
      break;
    case 'iteration':
      u.iters = { ...u.iters, [ev.kind]: ev.n };
      break;
    case 'log':
      u.log = [...u.log.slice(-400), { stream: ev.stream, line: ev.line }];
      break;
    case 'metric':
      u.cost = ev.cost_usd;
      u.tokensIn = ev.tokens_in;
      u.tokensOut = ev.tokens_out;
      break;
    case 'finding':
      u.findings = [
        ...u.findings,
        { round: ev.round, title: ev.title, severity: ev.severity, resolved: ev.resolved },
      ];
      break;
    case 'artifact':
      if (ev.kind === 'pr') u.pr = ev.ref;
      if (ev.kind === 'branch') u.branch = ev.ref;
      break;
    case 'blocked':
      u.blocked = ev.reason;
      if (ev.reason === 'awaiting concurrency slot') u.awaitingSlot = true;
      if (ev.reason === 'rate limited') u.rateLimited = true; // exact match to RL_REASON
      break;
    case 'error':
      u.error = `${ev.scope}: ${ev.detail}`;
      break;
    case 'done':
      u.result = ev.result;
      break;
  }
  return u;
}
