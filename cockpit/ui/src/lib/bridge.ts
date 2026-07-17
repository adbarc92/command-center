// View-plugin MessagePort bridge + command policy (Lane V).
//
// A view plugin is UNTRUSTED UI in a sandboxed iframe (`allow-scripts`, never
// `allow-same-origin` → opaque "null" origin). Its only channel to the host is a
// MessagePort transferred at handshake. The host identifies the plugin by HOLDING the
// port (not by `event.origin`, which is "null" and unusable for auth), and it POLICES
// every command before it reaches the store's single command sink.
//
// Handshake (plugin-announces-ready — avoids the `load`-event post race):
//   1. Host attaches a window `message` listener, then creates the sandboxed iframe.
//   2. The plugin posts a window-level `plugin-hello` to `parent`.
//   3. The host replies `init` TRANSFERRING port2; thereafter all traffic is on the port.
//
// This module has three testable layers:
//   • pure policy + rate/flood primitives (`policeCommand`, `TokenBucket`, `FloodMeter`),
//   • `PluginSession` — everything that happens over an established MessagePort,
//   • `PluginBridge` — the window `plugin-hello` → `init`+port dance around a `PluginSession`.

import type { CreateReq } from './api';
import type { Unit } from './fleet';
import type { CommandName } from './types';
import type { FleetStore, LogDelta } from './store.svelte';

export const PROTOCOL_VERSION = 1 as const;
/** The apiVersion the host implements; a manifest asking for anything else is refused. */
export const HOST_API_VERSION = 1;
/** The named capability set the host advertises in `init` (grows without apiVersion breaks). */
export const HOST_CAPABILITIES = ['log-append', 'real-launch-confirm'] as const;

export type PluginTier = 't1' | 't2' | 't3';
export type PluginMode = 'demo' | 'real';
export type PluginUnitAction = 'halt' | 'resume' | 'abandon' | 'ship';

/** Host-only command verbs a plugin may NEVER issue (the overlay owns these). */
export const HOST_ONLY_ACTIONS = ['approve_oracle', 'reject_oracle'];
/** Unit verbs a plugin MAY request (still policed for shape/id/rate). */
export const PLUGIN_UNIT_ACTIONS: PluginUnitAction[] = ['halt', 'resume', 'abandon', 'ship'];
/** Upper bound on a plugin-supplied `task` string. */
export const MAX_TASK_LEN = 2000;
/** How many phase entries `UnitLite.history` carries (a generous bounded phase walk). */
export const HISTORY_CAP = 64;

// ── wire messages ────────────────────────────────────────────────────────────
// host → plugin
export interface InitMsg {
  v: 1;
  type: 'init';
  apiVersion: number;
  capabilities: string[];
}
export interface StateMsg {
  v: 1;
  type: 'state';
  /** true = full snapshot (resets the plugin's per-unit baseline); false = dirty delta. */
  full: boolean;
  changed: UnitLite[];
  /** Reserved-always-empty this cycle (the store never deletes units). */
  removed: string[];
  order: string[];
  /** Coarse daemon health — the literal key/version stay host-side (no recon). */
  degraded: boolean;
}
export interface LogAppendMsg {
  v: 1;
  type: 'log-append';
  unitId: string;
  lines: LogDelta[];
}
export interface LogResetMsg {
  v: 1;
  type: 'log-reset';
}
export interface CommandAckMsg {
  v: 1;
  type: 'command-ack';
  reqId: string;
  ok: boolean;
  reasonClass?: string;
}
export type HostMessage = InitMsg | StateMsg | LogAppendMsg | LogResetMsg | CommandAckMsg;

// plugin → host
export interface HelloMsg {
  v: 1;
  type: 'plugin-hello';
}
export interface ReadyMsg {
  v: 1;
  type: 'ready';
}
export interface CommandMsg {
  v: 1;
  type: 'command';
  /** Opaque, plugin-scoped correlation id — echoed in `command-ack`. NOT the host cmd_id. */
  reqId: string;
  launch?: { task: string; tier: PluginTier; mode: PluginMode; min_review_rounds?: number };
  unit?: { id: string; action: PluginUnitAction };
}

/** `UnitLite` = a unit minus its (heavy, untagged) `log`; `history` is capped by the bridge. */
export type UnitLite = Omit<Unit, 'log'>;

export function toUnitLite(u: Unit, historyCap = HISTORY_CAP): UnitLite {
  return {
    id: u.id,
    task: u.task,
    tier: u.tier,
    phase: u.phase,
    history: u.history.slice(-historyCap),
    cost: u.cost,
    usdCap: u.usdCap,
    tokensIn: u.tokensIn,
    tokensOut: u.tokensOut,
    iters: u.iters,
    findings: u.findings,
    oracleFiles: u.oracleFiles,
    branch: u.branch,
    pr: u.pr,
    blocked: u.blocked,
    awaitingSlot: u.awaitingSlot,
    rateLimited: u.rateLimited,
    error: u.error,
    result: u.result,
    lastSeq: u.lastSeq,
  };
}

// ── rate / flood primitives ──────────────────────────────────────────────────

/** Token bucket for ACCEPTED commands (a second, stricter bucket guards `launch`). */
export class TokenBucket {
  private tokens: number;
  private last: number;
  constructor(
    private readonly capacity: number,
    private readonly refillPerSec: number,
    now: number = Date.now(),
  ) {
    this.tokens = capacity;
    this.last = now;
  }

  /** Consume `cost` tokens; returns false (and consumes nothing) if the bucket is dry. */
  take(now: number = Date.now(), cost = 1): boolean {
    const elapsed = Math.max(0, now - this.last) / 1000;
    this.tokens = Math.min(this.capacity, this.tokens + elapsed * this.refillPerSec);
    this.last = now;
    if (this.tokens >= cost) {
      this.tokens -= cost;
      return true;
    }
    return false;
  }
}

/**
 * Inbound port-message ceiling measured BEFORE policy — a flood can't be deserialized
 * into a host-thread stall. Sliding one-second window; `hit()` returns true when the
 * ceiling is exceeded (the session then `port.close()`s the plugin).
 */
export class FloodMeter {
  private times: number[] = [];
  constructor(private readonly ceilingPerSec: number) {}

  hit(now: number = Date.now()): boolean {
    this.times.push(now);
    const cutoff = now - 1000;
    let drop = 0;
    while (drop < this.times.length && this.times[drop] < cutoff) drop++;
    if (drop) this.times.splice(0, drop);
    return this.times.length > this.ceilingPerSec;
  }
}

// ── command policy (the trust boundary — a policy, not just a schema check) ────

export type PolicyResult =
  | { ok: true; kind: 'launch'; req: CreateReq }
  | { ok: true; kind: 'unit'; unitId: string; action: PluginUnitAction }
  | { ok: false; reasonClass: string };

export interface PolicyContext {
  /** Does this unit id exist in the host's FleetState? */
  hasUnit(id: string): boolean;
}

function isPrintable(s: string): boolean {
  // Reject control chars (except tab 0x09, newline 0x0a, carriage-return 0x0d) so a
  // plugin cannot smuggle escapes/nulls into a task string.
  for (let i = 0; i < s.length; i++) {
    const c = s.charCodeAt(i);
    if (c === 0x09 || c === 0x0a || c === 0x0d) continue;
    if (c < 0x20 || c === 0x7f) return false;
  }
  return true;
}

/**
 * Validate a plugin `command` for SHAPE, AUTHORITY, and BOUNDS. Stateless and pure —
 * cost/`real` gating and rate limiting are stateful and enforced by the session. Returns
 * a discriminated result the session turns into a store call + `command-ack`.
 *
 * reasonClass vocabulary: `malformed` · `authority` (host-only verb) · `unknown-type`
 * (unknown unit action) · `unknown-id` · `over-bound` (task too long).
 */
export function policeCommand(msg: unknown, ctx: PolicyContext): PolicyResult {
  if (typeof msg !== 'object' || msg === null) return { ok: false, reasonClass: 'malformed' };
  const m = msg as Record<string, unknown>;
  if (m.v !== PROTOCOL_VERSION || m.type !== 'command') return { ok: false, reasonClass: 'malformed' };
  if (typeof m.reqId !== 'string' || m.reqId.length === 0) return { ok: false, reasonClass: 'malformed' };

  const hasLaunch = m.launch !== undefined;
  const hasUnit = m.unit !== undefined;
  if (hasLaunch === hasUnit) return { ok: false, reasonClass: 'malformed' }; // exactly one

  if (hasUnit) {
    const u = m.unit as Record<string, unknown>;
    if (typeof u?.id !== 'string' || typeof u?.action !== 'string') {
      return { ok: false, reasonClass: 'malformed' };
    }
    if (HOST_ONLY_ACTIONS.includes(u.action)) return { ok: false, reasonClass: 'authority' };
    if (!PLUGIN_UNIT_ACTIONS.includes(u.action as PluginUnitAction)) {
      return { ok: false, reasonClass: 'unknown-type' };
    }
    if (!ctx.hasUnit(u.id)) return { ok: false, reasonClass: 'unknown-id' };
    return { ok: true, kind: 'unit', unitId: u.id, action: u.action as PluginUnitAction };
  }

  const l = m.launch as Record<string, unknown>;
  if (typeof l?.task !== 'string') return { ok: false, reasonClass: 'malformed' };
  if (l.task.trim().length === 0 || !isPrintable(l.task)) return { ok: false, reasonClass: 'malformed' };
  if (l.task.length > MAX_TASK_LEN) return { ok: false, reasonClass: 'over-bound' };
  if (l.tier !== 't1' && l.tier !== 't2' && l.tier !== 't3') return { ok: false, reasonClass: 'malformed' };
  if (l.mode !== 'demo' && l.mode !== 'real') return { ok: false, reasonClass: 'malformed' };
  let rounds = 2;
  if (l.min_review_rounds !== undefined) {
    if (typeof l.min_review_rounds !== 'number' || !Number.isInteger(l.min_review_rounds) || l.min_review_rounds < 1 || l.min_review_rounds > 6) {
      return { ok: false, reasonClass: 'over-bound' };
    }
    rounds = l.min_review_rounds;
  }
  return {
    ok: true,
    kind: 'launch',
    req: { task: l.task, tier: l.tier, mode: l.mode, min_review_rounds: rounds },
  };
}

// ── host adapter ─────────────────────────────────────────────────────────────

/**
 * The narrow surface the bridge needs from the host store — so the session is testable
 * against a fake and the store stays the single owner of fleet state + command sink.
 */
export interface BridgeHost {
  order(): string[];
  unit(id: string): Unit | undefined;
  drainDirty(): string[];
  logsSince(id: string, sinceSeq: number): LogDelta[];
  /** Coarse daemon health only — never the literal key/version. */
  degraded(): boolean;
  launch(req: CreateReq): Promise<string>;
  command(unitId: string, name: CommandName): Promise<void>;
  requestRealLaunch(req: CreateReq): void;
}

/** Adapt the real `FleetStore` singleton into a `BridgeHost`. */
export function makeFleetHost(store: FleetStore): BridgeHost {
  return {
    order: () => store.order,
    unit: (id) => store.units[id],
    drainDirty: () => store.drainDirty(),
    logsSince: (id, since) => store.logsSince(id, since),
    degraded: () => {
      const d = store.daemon;
      return !d || !d.docker || !d.anthropic_key;
    },
    launch: (req) => store.launch(req),
    command: (id, name) => store.cmd(name, id),
    requestRealLaunch: (req) => store.requestRealLaunch(req),
  };
}

// ── session ──────────────────────────────────────────────────────────────────

export interface SessionOpts {
  /** State/log push cadence (ms). */
  tickMs?: number;
  /** Start the internal tick timer on `ready` (false = drive `tick()` manually in tests). */
  autoTick?: boolean;
  /** Inbound msgs/sec ceiling before `port.close()` kill. */
  floodCeiling?: number;
  /** Accepted-command token bucket size + refill/sec. */
  bucketCapacity?: number;
  bucketRefillPerSec?: number;
  /** Stricter bucket for `launch`. */
  launchBucketCapacity?: number;
  launchBucketRefillPerSec?: number;
  /** Host capabilities to echo in `init` (defaults to `HOST_CAPABILITIES`). */
  capabilities?: string[];
  /** Called when the plugin is killed (flood/destroy) so the host can revert to ops grid. */
  onKill?: (reason: string) => void;
  now?: () => number;
}

const DEFAULTS = {
  tickMs: 60,
  floodCeiling: 200,
  bucketCapacity: 20,
  bucketRefillPerSec: 10,
  launchBucketCapacity: 3,
  launchBucketRefillPerSec: 0.5,
};

/**
 * Everything that happens over an established MessagePort: the `ready` handshake tail,
 * dirty-delta `state` pushes, `log-append`/`log-reset`, and policed `command` → sink →
 * `command-ack`. Constructed with `port1`; the plugin holds `port2`.
 */
export class PluginSession {
  private ready = false;
  private alive = true;
  private timer: ReturnType<typeof setInterval> | null = null;
  private readonly flood: FloodMeter;
  private readonly bucket: TokenBucket;
  private readonly launchBucket: TokenBucket;
  private readonly now: () => number;
  private readonly tickMs: number;
  private readonly autoTick: boolean;
  private readonly capabilities: string[];
  // Per-unit last-emitted `UnitLite` JSON — suppresses no-op deltas (the "only changed
  // units" payload AC). A full snapshot resets this baseline.
  private emitted: Record<string, string> = {};
  // Per-unit log seq cursor: the highest seq already delivered via `log-append`.
  private logCursor: Record<string, number> = {};

  constructor(
    private readonly port: MessagePort,
    private readonly host: BridgeHost,
    opts: SessionOpts = {},
  ) {
    this.now = opts.now ?? Date.now;
    this.tickMs = opts.tickMs ?? DEFAULTS.tickMs;
    this.autoTick = opts.autoTick ?? true;
    this.capabilities = opts.capabilities ?? [...HOST_CAPABILITIES];
    this.flood = new FloodMeter(opts.floodCeiling ?? DEFAULTS.floodCeiling);
    this.bucket = new TokenBucket(
      opts.bucketCapacity ?? DEFAULTS.bucketCapacity,
      opts.bucketRefillPerSec ?? DEFAULTS.bucketRefillPerSec,
      this.now(),
    );
    this.launchBucket = new TokenBucket(
      opts.launchBucketCapacity ?? DEFAULTS.launchBucketCapacity,
      opts.launchBucketRefillPerSec ?? DEFAULTS.launchBucketRefillPerSec,
      this.now(),
    );
    this.onKill = opts.onKill;
  }

  private onKill?: (reason: string) => void;

  /** Begin receiving on the port. The plugin replies `ready`; then we snapshot + tick. */
  start(): void {
    this.port.onmessage = (e: MessageEvent) => this.handleMessage(e.data);
    this.port.start?.();
  }

  private handleMessage(data: unknown): void {
    if (!this.alive) return;
    // Flood ceiling is measured BEFORE any parsing/policy — the whole point is that a
    // flood can't even be deserialized into a host-thread stall.
    if (this.flood.hit(this.now())) {
      this.kill('flood');
      return;
    }
    const m = data as Record<string, unknown> | null;
    if (!m || m.v !== PROTOCOL_VERSION) return;
    if (m.type === 'ready') {
      this.onReady();
    } else if (m.type === 'command') {
      void this.onCommand(m);
    }
    // unknown types are ignored (forward-compatible)
  }

  private onReady(): void {
    if (this.ready) return;
    this.ready = true;
    this.sendFullState();
    if (this.autoTick) {
      this.timer = setInterval(() => this.tick(), this.tickMs);
    }
  }

  /** Post a FULL snapshot and reset the per-unit delta baseline. */
  sendFullState(): void {
    const changed: UnitLite[] = [];
    for (const id of this.host.order()) {
      const u = this.host.unit(id);
      if (!u) continue;
      const lite = toUnitLite(u);
      this.emitted[id] = JSON.stringify(lite);
      changed.push(lite);
    }
    this.post({
      v: 1,
      type: 'state',
      full: true,
      changed,
      removed: [],
      order: this.host.order(),
      degraded: this.host.degraded(),
    });
  }

  /**
   * Drain the store's dirty set and push a per-unit `state` delta (only units whose
   * `UnitLite` actually changed) plus `log-append` for any new seq-tagged log lines.
   */
  tick(): void {
    if (!this.ready || !this.alive) return;
    const ids = this.host.drainDirty();
    const changed: UnitLite[] = [];
    for (const id of ids) {
      const u = this.host.unit(id);
      if (!u) continue;
      const lite = toUnitLite(u);
      const json = JSON.stringify(lite);
      if (json !== this.emitted[id]) {
        this.emitted[id] = json;
        changed.push(lite);
      }
      // Log deltas are independent of the state-delta suppression above.
      const cursor = this.logCursor[id] ?? 0;
      const lines = this.host.logsSince(id, cursor);
      if (lines.length) {
        this.logCursor[id] = lines[lines.length - 1].seq;
        this.post({ v: 1, type: 'log-append', unitId: id, lines });
      }
    }
    if (changed.length) {
      this.post({
        v: 1,
        type: 'state',
        full: false,
        changed,
        removed: [],
        order: this.host.order(),
        degraded: this.host.degraded(),
      });
    }
  }

  /** On a daemon-stream reconnect: tell the plugin to discard cursors, then full-snapshot. */
  reset(): void {
    this.logCursor = {};
    this.post({ v: 1, type: 'log-reset' });
    this.emitted = {};
    this.sendFullState();
  }

  private async onCommand(m: Record<string, unknown>): Promise<void> {
    const result = policeCommand(m, { hasUnit: (id) => this.host.unit(id) !== undefined });
    const reqId = typeof m.reqId === 'string' ? m.reqId : '';
    if (!result.ok) {
      this.ack(reqId, false, result.reasonClass);
      return;
    }
    if (result.kind === 'launch') {
      // Stricter launch bucket + the general bucket.
      if (!this.launchBucket.take(this.now()) || !this.bucket.take(this.now())) {
        this.ack(reqId, false, 'rate-limited');
        return;
      }
      // Cost/`real`: a plugin `launch` is demo-only. A `real` request is STAGED for the
      // host's real-launch confirm overlay (host-owned) and acked rejected so the
      // plugin's promise resolves ("blocked pending confirm") instead of hanging.
      if (result.req.mode === 'real') {
        this.host.requestRealLaunch(result.req);
        this.ack(reqId, false, 'real-requires-confirm');
        return;
      }
      try {
        await this.host.launch(result.req);
        this.ack(reqId, true);
      } catch {
        this.ack(reqId, false, 'sink-error');
      }
      return;
    }
    // unit command
    if (!this.bucket.take(this.now())) {
      this.ack(reqId, false, 'rate-limited');
      return;
    }
    try {
      await this.host.command(result.unitId, result.action);
      this.ack(reqId, true);
    } catch {
      this.ack(reqId, false, 'sink-error');
    }
  }

  private ack(reqId: string, ok: boolean, reasonClass?: string): void {
    const msg: CommandAckMsg = { v: 1, type: 'command-ack', reqId, ok };
    if (reasonClass !== undefined) msg.reasonClass = reasonClass;
    this.post(msg);
  }

  private post(msg: HostMessage): void {
    if (!this.alive) return;
    this.port.postMessage(msg);
  }

  /** Kill the plugin: stop the tick, close the port, notify the host (→ ops-grid revert). */
  kill(reason: string): void {
    if (!this.alive) return;
    this.alive = false;
    if (this.timer !== null) {
      clearInterval(this.timer);
      this.timer = null;
    }
    try {
      this.port.close();
    } catch {
      /* already closed */
    }
    this.onKill?.(reason);
  }

  /** Clean unmount (view switch): identical teardown, reason `destroy`. */
  destroy(): void {
    this.kill('destroy');
  }

  get isReady(): boolean {
    return this.ready;
  }
  get isAlive(): boolean {
    return this.alive;
  }
  /** The host capability set echoed to the plugin (for tests/introspection). */
  get advertisedCapabilities(): string[] {
    return this.capabilities;
  }
}

// ── window-level bridge (hello → init+port → session) ─────────────────────────

/** The subset of an `<iframe>` the bridge needs (so it's fakeable in tests). */
export interface FrameLike {
  contentWindow:
    | { postMessage(msg: unknown, targetOrigin: string, transfer?: Transferable[]): void }
    | null;
}

/**
 * Wires the window-level `plugin-hello` → `init` (with transferred port) handshake around
 * a `PluginSession`. Identity is bound to `frame.contentWindow` (`event.source`) — the
 * only trustworthy handle, since the sandboxed frame's origin is the unusable "null".
 */
export class PluginBridge {
  private session: PluginSession | null = null;
  private readonly listener: (e: MessageEvent) => void;

  constructor(
    private readonly frame: FrameLike,
    private readonly host: BridgeHost,
    private readonly opts: SessionOpts = {},
  ) {
    this.listener = (e) => this.onWindowMessage(e);
    if (typeof window !== 'undefined') {
      window.addEventListener('message', this.listener as EventListener);
    }
  }

  /**
   * Handle a window `message`. Exposed (not just wired to `window`) so tests can drive the
   * handshake without constructing a real cross-window `MessageEvent` (whose `source` is
   * not settable in jsdom). Only a `plugin-hello` from OUR frame is honored, exactly once.
   */
  onWindowMessage(e: { data: unknown; source: unknown }): void {
    if (this.session) return; // handshake already completed — ignore extras
    if (e.source !== this.frame.contentWindow) return; // identity: must be OUR frame
    const d = e.data as Record<string, unknown> | null;
    if (!d || d.v !== PROTOCOL_VERSION || d.type !== 'plugin-hello') return;
    const cw = this.frame.contentWindow;
    if (!cw) return;
    const channel = new MessageChannel();
    const init: InitMsg = {
      v: 1,
      type: 'init',
      apiVersion: HOST_API_VERSION,
      capabilities: this.opts.capabilities ?? [...HOST_CAPABILITIES],
    };
    // `"*"` targetOrigin is safe — the init payload is non-sensitive and the sandboxed
    // frame's real origin is "null" (can't be named). Trust comes from holding port1.
    cw.postMessage(init, '*', [channel.port2]);
    this.session = new PluginSession(channel.port1, this.host, this.opts);
    this.session.start();
  }

  get active(): PluginSession | null {
    return this.session;
  }

  /** Full teardown: stop listening, kill the session (closes the port). No leaks. */
  destroy(): void {
    if (typeof window !== 'undefined') {
      window.removeEventListener('message', this.listener as EventListener);
    }
    this.session?.destroy();
    this.session = null;
  }
}
