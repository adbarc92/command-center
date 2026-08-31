import { describe, it, expect } from 'vitest';
import { PluginSession, makeFleetHost, type HostMessage } from './bridge';
import { FleetStore } from './store.svelte';
import { newUnit } from './fleet';

// D-7 regression lane. `bridge.test.ts` drives a hand-rolled `BridgeHost` fake whose units
// are plain objects, so it can never reproduce the production failure: `makeFleetHost` hands
// the session `store.units[id]` and `store.order`, which are Svelte 5 `$state` PROXIES, and
// a proxy is not structured-cloneable. `postMessage` therefore threw `DataCloneError` inside
// `onReady` — before the tick timer was ever installed — so the plugin got no state AND no
// ticks. These tests use the real store and a real MessagePort for exactly that reason.

function seed(store: FleetStore, id: string, task = 'build the thing') {
  store.units[id] = newUnit(id, task, 'T1');
  store.order = [...store.order, id];
}

describe('PluginSession over a real MessagePort, backed by real $state units', () => {
  it('posts a full snapshot without throwing DataCloneError', () => {
    const chan = new MessageChannel();
    const cleanup = $effect.root(() => {
      const store = new FleetStore();
      seed(store, 'u1');
      // Populate the proxy-backed array fields that `toUnitLite` passes through by
      // reference - these are what actually fail to clone.
      store.units['u1'].history.push('spec');
      store.units['u1'].findings.push({
        round: 1,
        title: 'a finding',
        severity: 'minor',
        resolved: false,
      });
      store.units['u1'].oracleFiles.push('t/a.test.js');

      const session = new PluginSession(chan.port1, makeFleetHost(store), { autoTick: false });
      expect(() => session.sendFullState()).not.toThrow();
    });
    cleanup();
    chan.port1.close();
    chan.port2.close();
  });

  it('actually delivers the unit to the plugin side of the port', async () => {
    const chan = new MessageChannel();
    const received: HostMessage[] = [];
    // Wait on the CONDITION (a message arrived), never on a fixed delay. Port delivery is
    // async and a `setTimeout(0)` races it - that made this test intermittently red.
    let seen: () => void;
    const delivered = new Promise<void>((r) => (seen = r));
    chan.port2.onmessage = (e: MessageEvent) => {
      received.push(e.data as HostMessage);
      seen();
    };
    chan.port2.start();

    const cleanup = $effect.root(() => {
      const store = new FleetStore();
      seed(store, 'u1');
      store.units['u1'].oracleFiles.push('t/a.test.js');
      const session = new PluginSession(chan.port1, makeFleetHost(store), { autoTick: false });
      session.sendFullState();
    });

    await Promise.race([
      delivered,
      new Promise((_, reject) =>
        setTimeout(() => reject(new Error('no host message arrived on the port within 5s')), 5000),
      ),
    ]);
    cleanup();

    const state = received.find((m) => m.type === 'state');
    expect(state, 'plugin received no state message at all').toBeDefined();
    const msg = state as Extract<HostMessage, { type: 'state' }>;
    expect(msg.full).toBe(true);
    expect(msg.order).toEqual(['u1']);
    expect(msg.changed[0]?.id).toBe('u1');
    expect(msg.changed[0]?.oracleFiles).toEqual(['t/a.test.js']);

    chan.port1.close();
    chan.port2.close();
  });
});
