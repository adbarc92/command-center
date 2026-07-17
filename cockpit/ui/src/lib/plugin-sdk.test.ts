import { describe, it, expect } from 'vitest';
import { connect, attach } from '../../../plugin-sdk/index.js';
import { PluginSession, type BridgeHost } from './bridge';
import { newUnit, type Unit } from './fleet';
import type { CreateReq } from './api';

// Lane V: the SDK is the plugin-side of the same MessagePort protocol the host bridge
// speaks. These tests wire the real SDK client to a real `PluginSession` over a
// MessageChannel — a protocol-level proof of both ends (NOT the Tauri/iframe e2e, which
// Lane S owns). jsdom delivers port messages across macrotasks, so assertions poll.

const wait = () => new Promise((r) => setTimeout(r, 0));
async function waitFor(pred: () => boolean, tries = 100): Promise<boolean> {
  for (let i = 0; i < tries; i++) {
    if (pred()) return true;
    await wait();
  }
  return pred();
}

function hostWith(units: Record<string, Unit>): { host: BridgeHost; launches: CreateReq[] } {
  const launches: CreateReq[] = [];
  const host: BridgeHost = {
    order: () => Object.keys(units),
    unit: (id) => units[id],
    drainDirty: () => [],
    logsSince: () => [],
    degraded: () => false,
    launch: async (req) => {
      launches.push(req);
      return 'new-unit';
    },
    command: async () => {},
    requestRealLaunch: () => {},
  };
  return { host, launches };
}

describe('SDK attach() ↔ PluginSession over a MessageChannel', () => {
  it('receives the full snapshot after ready and resolves a demo launch ack', async () => {
    const units: Record<string, Unit> = { u1: newUnit('u1', 'task', 'T1') };
    const { host, launches } = hostWith(units);
    const ch = new MessageChannel();
    const session = new PluginSession(ch.port1, host, { autoTick: false });
    session.start();

    const client = attach(ch.port2, { apiVersion: 1, capabilities: ['log-append'] });
    const states: { full: boolean; changed: { id: string }[] }[] = [];
    client.onState((s) => states.push(s));

    await waitFor(() => states.length > 0);
    expect(states[0].full).toBe(true);
    expect(states[0].changed.map((u) => u.id)).toEqual(['u1']);

    const ack = await client.launch({ task: 'go', tier: 't1', mode: 'demo' });
    expect(ack.ok).toBe(true);
    expect(launches).toHaveLength(1);

    session.destroy();
  });

  it('surfaces a rejection ack (real launch blocked pending host confirm)', async () => {
    const { host } = hostWith({});
    const ch = new MessageChannel();
    const session = new PluginSession(ch.port1, host, { autoTick: false });
    session.start();
    const client = attach(ch.port2, { apiVersion: 1, capabilities: [] });

    const ack = await client.launch({ task: 'go', tier: 't1', mode: 'real' });
    expect(ack.ok).toBe(false);
    expect(ack.reasonClass).toBe('real-requires-confirm');
    session.destroy();
  });
});

describe('SDK connect() handshake', () => {
  it('posts plugin-hello and resolves a client when init transfers a port', async () => {
    const ch = new MessageChannel();
    const units: Record<string, Unit> = { u1: newUnit('u1', 'task', 'T1') };
    const { host } = hostWith(units);
    const session = new PluginSession(ch.port1, host, { autoTick: false });
    session.start();

    // Fake the window/parent: the parent replies to `plugin-hello` with an `init` event
    // that transfers port2 (exactly what the host bridge would do).
    let handler: ((e: unknown) => void) | null = null;
    let helloSeen = false;
    const scope = {
      addEventListener: (_t: string, h: (e: unknown) => void) => {
        handler = h;
      },
      removeEventListener: () => {},
    };
    const parent = {
      postMessage: (msg: { type: string }) => {
        if (msg.type === 'plugin-hello') {
          helloSeen = true;
          handler?.({ data: { v: 1, type: 'init', apiVersion: 1, capabilities: ['log-append'] }, ports: [ch.port2] });
        }
      },
    };

    const client = await connect({ scope, parent });
    expect(helloSeen).toBe(true);
    expect(client.capabilities).toEqual(['log-append']);

    const states: { full: boolean }[] = [];
    client.onState((s) => states.push(s));
    await waitFor(() => states.length > 0);
    expect(states[0].full).toBe(true);
    session.destroy();
  });
});
