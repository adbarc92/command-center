import { describe, it, expect, vi } from 'vitest';
import {
  PROTOCOL_VERSION,
  HOST_API_VERSION,
  HOST_CAPABILITIES,
  TokenBucket,
  FloodMeter,
  policeCommand,
  toUnitLite,
  PluginSession,
  PluginBridge,
  type BridgeHost,
  type HostMessage,
  type UnitLite,
} from './bridge';
import { newUnit, type Unit } from './fleet';
import type { CreateReq } from './api';
import type { CommandName } from './types';

// ── a minimal in-memory BridgeHost fake ───────────────────────────────────────
interface HostBag {
  host: BridgeHost;
  units: Record<string, Unit>;
  state: { dirty: string[]; degraded: boolean };
  logs: Record<string, { seq: number; stream: string; line: string }[]>;
  launches: CreateReq[];
  commands: [string, CommandName][];
  staged: CreateReq[];
}

function makeHost(): HostBag {
  const units: Record<string, Unit> = {};
  const state = { dirty: [] as string[], degraded: false };
  const logs: Record<string, { seq: number; stream: string; line: string }[]> = {};
  const launches: CreateReq[] = [];
  const commands: [string, CommandName][] = [];
  const staged: CreateReq[] = [];
  const host: BridgeHost = {
    order: () => Object.keys(units),
    unit: (id) => units[id],
    drainDirty: () => {
      const d = state.dirty;
      state.dirty = [];
      return d;
    },
    logsSince: (id, since) => (logs[id] ?? []).filter((l) => l.seq > since),
    degraded: () => state.degraded,
    launch: async (req) => {
      launches.push(req);
      return 'new-unit';
    },
    command: async (id, name) => {
      commands.push([id, name]);
    },
    requestRealLaunch: (req) => {
      staged.push(req);
    },
  };
  return { host, units, state, logs, launches, commands, staged };
}

const wait = () => new Promise((r) => setTimeout(r, 0));

// A MessagePort round-trip can span more than one macrotask in jsdom, so poll for the
// condition rather than assuming a fixed number of ticks.
async function waitFor(pred: () => boolean, tries = 100): Promise<boolean> {
  for (let i = 0; i < tries; i++) {
    if (pred()) return true;
    await wait();
  }
  return pred();
}

// Collect messages the host sends to the plugin end of a channel.
function pluginEnd(port: MessagePort): { received: HostMessage[]; port: MessagePort } {
  const received: HostMessage[] = [];
  port.onmessage = (e: MessageEvent) => received.push(e.data as HostMessage);
  port.start?.();
  return { received, port };
}

const hasFull = (r: HostMessage[]) => r.some((m) => m.type === 'state' && m.full);
const findAck = (r: HostMessage[]) => r.find((m) => m.type === 'command-ack') as
  | Extract<HostMessage, { type: 'command-ack' }>
  | undefined;

describe('toUnitLite', () => {
  it('drops the heavy log and caps history', () => {
    const u = newUnit('u1', 'task', 'T1');
    u.log = [{ stream: 'agent', line: 'x' }];
    u.history = Array.from({ length: 100 }, () => 'building' as const);
    const lite = toUnitLite(u, 10) as UnitLite & { log?: unknown };
    expect(lite.log).toBeUndefined();
    expect(lite.history).toHaveLength(10);
    expect(lite.id).toBe('u1');
  });
});

describe('TokenBucket', () => {
  it('allows up to capacity, then denies until refill', () => {
    const b = new TokenBucket(2, 1, 0); // 2 tokens, 1/sec
    expect(b.take(0)).toBe(true);
    expect(b.take(0)).toBe(true);
    expect(b.take(0)).toBe(false); // dry
    expect(b.take(1000)).toBe(true); // +1 token after 1s
    expect(b.take(1000)).toBe(false);
  });
});

describe('FloodMeter', () => {
  it('fires only once the per-second ceiling is exceeded', () => {
    const f = new FloodMeter(3);
    expect(f.hit(0)).toBe(false);
    expect(f.hit(0)).toBe(false);
    expect(f.hit(0)).toBe(false);
    expect(f.hit(0)).toBe(true); // 4th within the window > ceiling 3
  });

  it('slides the window so old hits expire', () => {
    const f = new FloodMeter(2);
    f.hit(0);
    f.hit(0);
    expect(f.hit(2000)).toBe(false); // earlier hits aged out
  });
});

describe('policeCommand — the trust boundary', () => {
  const ctx = { hasUnit: (id: string) => id === 'known' };

  it('accepts a well-formed demo launch', () => {
    const r = policeCommand(
      { v: 1, type: 'command', reqId: 'r1', launch: { task: 'do it', tier: 't1', mode: 'demo' } },
      ctx,
    );
    expect(r).toEqual({ ok: true, kind: 'launch', req: { task: 'do it', tier: 't1', mode: 'demo', min_review_rounds: 2 } });
  });

  it('accepts a valid unit command on a known id', () => {
    const r = policeCommand({ v: 1, type: 'command', reqId: 'r1', unit: { id: 'known', action: 'halt' } }, ctx);
    expect(r).toEqual({ ok: true, kind: 'unit', unitId: 'known', action: 'halt' });
  });

  it('rejects a host-only verb (approve_oracle) as an authority violation', () => {
    const r = policeCommand({ v: 1, type: 'command', reqId: 'r1', unit: { id: 'known', action: 'approve_oracle' } }, ctx);
    expect(r).toEqual({ ok: false, reasonClass: 'authority' });
  });

  it('rejects an unknown unit action as unknown-type', () => {
    const r = policeCommand({ v: 1, type: 'command', reqId: 'r1', unit: { id: 'known', action: 'nuke' } }, ctx);
    expect(r).toEqual({ ok: false, reasonClass: 'unknown-type' });
  });

  it('rejects a command against an unknown unit id', () => {
    const r = policeCommand({ v: 1, type: 'command', reqId: 'r1', unit: { id: 'ghost', action: 'halt' } }, ctx);
    expect(r).toEqual({ ok: false, reasonClass: 'unknown-id' });
  });

  it('rejects an over-bound task', () => {
    const r = policeCommand(
      { v: 1, type: 'command', reqId: 'r1', launch: { task: 'x'.repeat(3000), tier: 't1', mode: 'demo' } },
      ctx,
    );
    expect(r).toEqual({ ok: false, reasonClass: 'over-bound' });
  });

  it('rejects malformed shapes (bad version, both/neither payload, empty task, bad tier)', () => {
    expect(policeCommand({ v: 2, type: 'command', reqId: 'r' }, ctx).ok).toBe(false);
    expect(policeCommand({ v: 1, type: 'command', reqId: 'r' }, ctx)).toEqual({ ok: false, reasonClass: 'malformed' });
    expect(
      policeCommand({ v: 1, type: 'command', reqId: 'r', launch: { task: 'a', tier: 't1', mode: 'demo' }, unit: { id: 'known', action: 'halt' } }, ctx).ok,
    ).toBe(false);
    expect(policeCommand({ v: 1, type: 'command', reqId: 'r', launch: { task: '   ', tier: 't1', mode: 'demo' } }, ctx)).toEqual({ ok: false, reasonClass: 'malformed' });
    expect(policeCommand({ v: 1, type: 'command', reqId: 'r', launch: { task: 'a', tier: 'gold', mode: 'demo' } }, ctx).ok).toBe(false);
    expect(policeCommand(null, ctx)).toEqual({ ok: false, reasonClass: 'malformed' });
  });

  it('rejects a task carrying control characters', () => {
    const task = `bad${String.fromCharCode(0)}null`; // embedded NUL
    const r = policeCommand({ v: 1, type: 'command', reqId: 'r1', launch: { task, tier: 't1', mode: 'demo' } }, ctx);
    expect(r).toEqual({ ok: false, reasonClass: 'malformed' });
  });
});

describe('PluginSession over a real MessageChannel', () => {
  it('sends a full snapshot on ready and echoes host capabilities in init separately', async () => {
    const { host, units } = makeHost();
    units['u1'] = newUnit('u1', 'task one', 'T1');
    const ch = new MessageChannel();
    const plugin = pluginEnd(ch.port2);
    const session = new PluginSession(ch.port1, host, { autoTick: false });
    session.start();

    ch.port2.postMessage({ v: 1, type: 'ready' });
    await waitFor(() => hasFull(plugin.received));

    const full = plugin.received.find((m) => m.type === 'state' && m.full);
    expect(full).toBeTruthy();
    expect((full as Extract<HostMessage, { type: 'state' }>).changed.map((u) => u.id)).toEqual(['u1']);
    session.destroy();
  });

  it('handshake→ready→full-snapshot succeeds 100× with zero drops', async () => {
    let delivered = 0;
    for (let i = 0; i < 100; i++) {
      const { host, units } = makeHost();
      units['u1'] = newUnit('u1', 't', 'T1');
      const ch = new MessageChannel();
      const plugin = pluginEnd(ch.port2);
      const session = new PluginSession(ch.port1, host, { autoTick: false });
      session.start();
      ch.port2.postMessage({ v: 1, type: 'ready' });
      if (await waitFor(() => hasFull(plugin.received))) delivered++;
      session.destroy();
    }
    expect(delivered).toBe(100);
  });

  it('pushes a dirty-delta state on tick containing only changed units', async () => {
    const bag = makeHost();
    const { host, units } = bag;
    units['u1'] = newUnit('u1', 't', 'T1');
    units['u2'] = newUnit('u2', 't', 'T1');
    const ch = new MessageChannel();
    const plugin = pluginEnd(ch.port2);
    const session = new PluginSession(ch.port1, host, { autoTick: false });
    session.start();
    ch.port2.postMessage({ v: 1, type: 'ready' });
    await waitFor(() => hasFull(plugin.received));
    plugin.received.length = 0;

    // Only u1 changed since the snapshot.
    units['u1'].phase = 'building';
    bag.state.dirty = ['u1'];
    session.tick();
    await waitFor(() => plugin.received.some((m) => m.type === 'state'));

    const delta = plugin.received.find((m) => m.type === 'state') as Extract<HostMessage, { type: 'state' }>;
    expect(delta.full).toBe(false);
    expect(delta.changed.map((u) => u.id)).toEqual(['u1']);
    session.destroy();
  });

  it('emits log-append for new lines and log-reset on reset()', async () => {
    const bag = makeHost();
    const { host, units, logs } = bag;
    units['u1'] = newUnit('u1', 't', 'T1');
    logs['u1'] = [
      { seq: 1, stream: 'agent', line: 'first' },
      { seq: 2, stream: 'agent', line: 'second' },
    ];
    const ch = new MessageChannel();
    const plugin = pluginEnd(ch.port2);
    const session = new PluginSession(ch.port1, host, { autoTick: false });
    session.start();
    ch.port2.postMessage({ v: 1, type: 'ready' });
    await waitFor(() => hasFull(plugin.received));

    bag.state.dirty = ['u1'];
    session.tick();
    await waitFor(() => plugin.received.some((m) => m.type === 'log-append'));
    const append = plugin.received.find((m) => m.type === 'log-append') as Extract<HostMessage, { type: 'log-append' }>;
    expect(append.lines.map((l) => l.seq)).toEqual([1, 2]);

    plugin.received.length = 0;
    session.reset();
    await waitFor(() => plugin.received.some((m) => m.type === 'log-reset') && hasFull(plugin.received));
    expect(plugin.received.some((m) => m.type === 'log-reset')).toBe(true);
    expect(hasFull(plugin.received)).toBe(true);
    session.destroy();
  });

  it('policed demo launch reaches the sink and is acked ok', async () => {
    const bag = makeHost();
    const ch = new MessageChannel();
    const plugin = pluginEnd(ch.port2);
    const session = new PluginSession(ch.port1, bag.host, { autoTick: false });
    session.start();
    ch.port2.postMessage({ v: 1, type: 'ready' });
    await waitFor(() => hasFull(plugin.received));

    ch.port2.postMessage({ v: 1, type: 'command', reqId: 'q1', launch: { task: 'go', tier: 't1', mode: 'demo' } });
    await waitFor(() => findAck(plugin.received) !== undefined);
    expect(findAck(plugin.received)).toMatchObject({ reqId: 'q1', ok: true });
    expect(bag.launches).toHaveLength(1);
    session.destroy();
  });

  it('a real launch is staged for host confirm and acked rejected (not executed)', async () => {
    const bag = makeHost();
    const ch = new MessageChannel();
    const plugin = pluginEnd(ch.port2);
    const session = new PluginSession(ch.port1, bag.host, { autoTick: false });
    session.start();
    ch.port2.postMessage({ v: 1, type: 'ready' });
    await waitFor(() => hasFull(plugin.received));

    ch.port2.postMessage({ v: 1, type: 'command', reqId: 'q2', launch: { task: 'go', tier: 't1', mode: 'real' } });
    await waitFor(() => findAck(plugin.received) !== undefined);
    expect(findAck(plugin.received)).toMatchObject({ reqId: 'q2', ok: false, reasonClass: 'real-requires-confirm' });
    expect(bag.launches).toHaveLength(0); // NOT executed
    expect(bag.staged).toHaveLength(1); // staged for the host overlay
    session.destroy();
  });

  it('acks a rejection with the policy reasonClass (unknown-id)', async () => {
    const bag = makeHost();
    const ch = new MessageChannel();
    const plugin = pluginEnd(ch.port2);
    const session = new PluginSession(ch.port1, bag.host, { autoTick: false });
    session.start();
    ch.port2.postMessage({ v: 1, type: 'ready' });
    await waitFor(() => hasFull(plugin.received));

    ch.port2.postMessage({ v: 1, type: 'command', reqId: 'q3', unit: { id: 'ghost', action: 'halt' } });
    await waitFor(() => findAck(plugin.received) !== undefined);
    expect(findAck(plugin.received)).toMatchObject({ reqId: 'q3', ok: false, reasonClass: 'unknown-id' });
    expect(bag.commands).toHaveLength(0);
    session.destroy();
  });

  it('an inbound flood kills the plugin (port close + onKill)', async () => {
    const bag = makeHost();
    const ch = new MessageChannel();
    pluginEnd(ch.port2);
    const onKill = vi.fn();
    const session = new PluginSession(ch.port1, bag.host, { autoTick: false, floodCeiling: 5, onKill });
    session.start();
    ch.port2.postMessage({ v: 1, type: 'ready' });
    await wait();

    for (let i = 0; i < 50; i++) {
      ch.port2.postMessage({ v: 1, type: 'command', reqId: `f${i}`, unit: { id: 'x', action: 'halt' } });
    }
    await waitFor(() => onKill.mock.calls.length > 0);
    expect(onKill).toHaveBeenCalledWith('flood');
    expect(session.isAlive).toBe(false);
  });
});

describe('PluginBridge window handshake (hello → init + port)', () => {
  it('replies init only to a hello from OUR frame, transferring a port', async () => {
    const bag = makeHost();
    let initMsg: HostMessage | null = null;
    let transferred: MessagePort | null = null;
    const contentWindow = {
      postMessage: (msg: unknown, _o: string, transfer?: Transferable[]) => {
        initMsg = msg as HostMessage;
        transferred = (transfer?.[0] as MessagePort) ?? null;
      },
    };
    const bridge = new PluginBridge({ contentWindow }, bag.host, { autoTick: false });

    // A hello from a DIFFERENT source is ignored.
    bridge.onWindowMessage({ data: { v: 1, type: 'plugin-hello' }, source: {} });
    expect(bridge.active).toBeNull();

    // A hello from our frame completes the handshake.
    bridge.onWindowMessage({ data: { v: 1, type: 'plugin-hello' }, source: contentWindow });
    expect(bridge.active).not.toBeNull();
    expect(initMsg).toMatchObject({ type: 'init', apiVersion: HOST_API_VERSION, capabilities: [...HOST_CAPABILITIES] });
    expect(transferred).toBeTruthy();

    // The transferred port is live: driving ready over it yields a snapshot.
    const plugin = pluginEnd(transferred!);
    transferred!.postMessage({ v: PROTOCOL_VERSION, type: 'ready' });
    await waitFor(() => hasFull(plugin.received));
    expect(hasFull(plugin.received)).toBe(true);

    bridge.destroy();
  });

  it('ignores a second hello (handshake is once-only)', () => {
    const bag = makeHost();
    let posts = 0;
    const contentWindow = { postMessage: () => { posts++; } };
    const bridge = new PluginBridge({ contentWindow }, bag.host, { autoTick: false });
    bridge.onWindowMessage({ data: { v: 1, type: 'plugin-hello' }, source: contentWindow });
    bridge.onWindowMessage({ data: { v: 1, type: 'plugin-hello' }, source: contentWindow });
    expect(posts).toBe(1);
    bridge.destroy();
  });
});
