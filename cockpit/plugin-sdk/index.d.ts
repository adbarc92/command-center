// Type declarations for the cockpit view-plugin SDK (hand-written; the runtime is
// plain browser ESM in index.js). Mirrors the host bridge protocol.

export type PluginTier = 't1' | 't2' | 't3';
export type PluginMode = 'demo' | 'real';
export type PluginUnitAction = 'halt' | 'resume' | 'abandon' | 'ship';

export interface LogDelta {
  seq: number;
  stream: string;
  line: string;
}

/** A unit projection minus its heavy log (history capped by the host). */
export interface UnitLite {
  id: string;
  task: string;
  tier: string;
  phase: string;
  history: string[];
  cost: number;
  usdCap: number;
  tokensIn: number;
  tokensOut: number;
  iters: { build: number; check: number; review: number };
  findings: { round: number; title: string; severity: string; resolved: boolean }[];
  oracleFiles: string[];
  branch?: string;
  pr?: string;
  blocked?: string;
  awaitingSlot: boolean;
  rateLimited: boolean;
  error?: string;
  result?: string;
  lastSeq: number;
}

export interface StateMessage {
  v: 1;
  type: 'state';
  full: boolean;
  changed: UnitLite[];
  removed: string[];
  order: string[];
  degraded: boolean;
}

export interface CommandAck {
  v: 1;
  type: 'command-ack';
  reqId: string;
  ok: boolean;
  reasonClass?: string;
}

export interface LaunchReq {
  task: string;
  tier: PluginTier;
  mode: PluginMode;
  min_review_rounds?: number;
}

export interface PluginClient {
  apiVersion?: number;
  capabilities: string[];
  onState(cb: (s: StateMessage) => void): () => void;
  onLog(cb: (unitId: string, lines: LogDelta[]) => void): () => void;
  onReset(cb: () => void): () => void;
  onAck(cb: (ack: CommandAck) => void): () => void;
  launch(req: LaunchReq): Promise<CommandAck>;
  command(unitId: string, action: PluginUnitAction): Promise<CommandAck>;
}

export interface ConnectOpts {
  scope?: unknown;
  parent?: unknown;
  timeoutMs?: number;
}

export function connect(opts?: ConnectOpts): Promise<PluginClient>;
export function attach(
  port: MessagePort,
  init?: { apiVersion?: number; capabilities?: string[] },
): PluginClient;
