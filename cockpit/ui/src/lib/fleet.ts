// Folds the daemon event stream into per-unit view state.

import type { FleetEvent, Phase } from './types';

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
  tokensIn: number;
  tokensOut: number;
  iters: { build: number; check: number; review: number };
  log: LogLine[];
  findings: Finding[];
  oracleFiles: string[];
  branch?: string;
  pr?: string;
  blocked?: string;
  error?: string;
  result?: string;
}

export function newUnit(id: string, task: string, tier: string): Unit {
  return {
    id,
    task,
    tier,
    phase: 'queued',
    history: ['queued'],
    cost: 0,
    tokensIn: 0,
    tokensOut: 0,
    iters: { build: 0, check: 0, review: 0 },
    log: [],
    findings: [],
    oracleFiles: [],
  };
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
