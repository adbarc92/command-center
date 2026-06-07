// Mirrors the daemon's event/command contract (fleet-core::event).

export type Phase =
  | 'queued'
  | 'provisioning'
  | 'spec'
  | 'awaiting_oracle_approval'
  | 'building'
  | 'checking'
  | 'reviewing'
  | 'merge_check'
  | 'pr_open'
  | 'done'
  | 'no_change'
  | 'failed'
  | 'needs_human'
  | 'halted';

export type IterationKind = 'build' | 'check' | 'review';
export type LogStream = 'agent' | 'check' | 'system';
export type Severity = 'info' | 'minor' | 'blocker';
export type ArtifactKind = 'branch' | 'pr' | 'diff';

export type FleetEvent =
  | { type: 'phase_changed'; from: Phase; to: Phase; reason?: string; cmd_id?: string }
  | { type: 'oracle_proposed'; test_files: string[]; hash: string; summary: string }
  | { type: 'iteration'; kind: IterationKind; n: number }
  | { type: 'log'; stream: LogStream; line: string }
  | { type: 'metric'; tokens_in: number; tokens_out: number; cost_usd: number; elapsed_ms: number }
  | { type: 'finding'; round: number; severity: Severity; title: string; file?: string; resolved: boolean }
  | { type: 'artifact'; kind: ArtifactKind; ref: string }
  | { type: 'blocked'; reason: string; cap?: string; detail: string }
  | { type: 'error'; scope: string; retryable: boolean; detail: string }
  | { type: 'done'; result: string };

export interface Envelope {
  unit_id: string;
  seq: number;
  event: FleetEvent;
}

/** A unit summary from `GET /units` — used to repopulate the fleet on load. */
export interface Snapshot {
  unit_id: string;
  phase: Phase;
  cost: number;
  usd_cap: number;
  tier: string;
  task: string;
  last_seq: number;
}

/** Daemon liveness from `GET /health` — drives the header badges. */
export interface Health {
  docker: boolean;
  anthropic_key: boolean;
  version: string;
}

export type CommandName =
  | 'halt'
  | 'resume'
  | 'abandon'
  | 'ship'
  | 'approve_oracle'
  | 'reject_oracle';
