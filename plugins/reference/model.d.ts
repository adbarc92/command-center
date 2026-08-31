// Types for the reference plugin's pure model (runtime is model.js).

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

export interface LogLine {
  seq: number;
  stream: string;
  line: string;
}

export interface StateMessage {
  full: boolean;
  changed: UnitLite[];
  removed?: string[];
  order?: string[];
  degraded?: boolean;
}

export interface CommandAck {
  reqId: string;
  ok: boolean;
  reasonClass?: string;
}

export interface RefModel {
  units: Record<string, UnitLite>;
  order: string[];
  degraded: boolean;
  logs: Record<string, LogLine[]>;
  acks: CommandAck[];
}

export function createModel(): RefModel;
export function applyState(model: RefModel, msg: StateMessage): RefModel;
export function applyLog(model: RefModel, unitId: string, lines: LogLine[]): RefModel;
export function applyReset(model: RefModel): RefModel;
export function recordAck(model: RefModel, ack: CommandAck): RefModel;
export function list(model: RefModel): UnitLite[];
export function awaitingApproval(model: RefModel): UnitLite | null;
export function ticker(model: RefModel, unitId: string): LogLine | null;
