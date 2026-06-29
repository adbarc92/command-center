import { describe, test, expect, vi } from 'vitest';
import { connectPlugin } from './bridge';
import { FakeWindow, FakeMessageChannel, type FakePort } from './bridge.testkit';

// The view-plugin handshake (design: docs/superpowers/specs/2026-06-07-view-plugins-design.md
// §"Handshake — plugin-announces-ready"). These tests run the host side (bridge.ts) against
// the faithful start()-enforcing port model in bridge.testkit, so a deterministic
// "every handshake fails" bug (e.g. a forgotten port.start()) shows up as RED — which
// jsdom's lenient built-in port would not catch.

function setup() {
  const host = new FakeWindow('http://tauri.localhost');
  const pluginWin = new FakeWindow('null'); // sandboxed iframe = opaque "null" origin
  const iframe = { contentWindow: pluginWin } as unknown as HTMLIFrameElement;
  return { host, pluginWin, iframe };
}

/**
 * The plugin SDK's side of the handshake, minimal: announce `plugin-hello` to the host,
 * await `init`, grab the transferred port, post `ready`. Mirrors the real SDK's `connect()`.
 */
function runFakePlugin(
  pluginWin: FakeWindow,
  host: FakeWindow,
  opts: { apiVersion?: number } = {},
): { port: () => FakePort | null } {
  let gotPort: FakePort | null = null;
  pluginWin.addEventListener('message', (e) => {
    const m = e.data as { v?: number; type?: string };
    if (m?.type === 'init') {
      gotPort = e.ports[0] ?? null;
      gotPort?.postMessage({ v: 1, type: 'ready' });
    }
  });
  // Announce readiness first (the corrected, no-load-race direction).
  host.deliver({
    data: { v: 1, type: 'plugin-hello', apiVersion: opts.apiVersion ?? 1 },
    origin: 'null',
    source: pluginWin,
    ports: [],
  });
  return { port: () => gotPort };
}

describe('view-plugin handshake (host side)', () => {
  test('completes: plugin-hello -> init(+transferred port) -> ready resolves a handle', async () => {
    const { host, pluginWin, iframe } = setup();

    const connected = connectPlugin(iframe, {
      window: host,
      channelFactory: () => new FakeMessageChannel(),
    });
    runFakePlugin(pluginWin, host);

    const handle = await connected;
    expect(handle).toBeTruthy();
    expect(handle.port).toBeTruthy();
  });

  test('rejects on ready-timeout when the plugin never acks (liveness -> ops-grid fallback)', async () => {
    const { host, pluginWin, iframe } = setup();

    const connected = connectPlugin(iframe, {
      window: host,
      channelFactory: () => new FakeMessageChannel(),
      readyTimeoutMs: 20,
    });
    // Plugin says hello and receives init, but never posts `ready`.
    pluginWin.addEventListener('message', () => {
      /* deliberately silent — no ready */
    });
    host.deliver({
      data: { v: 1, type: 'plugin-hello', apiVersion: 1 },
      origin: 'null',
      source: pluginWin,
      ports: [],
    });

    await expect(connected).rejects.toMatchObject({ reason: 'ready-timeout' });
  }, 1000);

  test('refuses an unsupported apiVersion: rejects and transfers no port', async () => {
    const { host, pluginWin, iframe } = setup();

    let initSeen = false;
    pluginWin.addEventListener('message', (e) => {
      if ((e.data as { type?: string })?.type === 'init') initSeen = true;
    });

    const connected = connectPlugin(iframe, {
      window: host,
      channelFactory: () => new FakeMessageChannel(),
      supportedApiVersions: [1],
      readyTimeoutMs: 50,
    });
    runFakePlugin(pluginWin, host, { apiVersion: 99 });

    await expect(connected).rejects.toMatchObject({ reason: 'unsupported-api-version' });
    expect(initSeen).toBe(false); // a refused plugin never receives a port
  }, 1000);

  test('port identity: a `ready` spoofed onto the host window does not complete the handshake', async () => {
    const { host, pluginWin, iframe } = setup();

    const connected = connectPlugin(iframe, {
      window: host,
      channelFactory: () => new FakeMessageChannel(),
      readyTimeoutMs: 40,
    });
    // Real hello (so a port IS issued), but the plugin never acks over the port.
    pluginWin.addEventListener('message', () => {
      /* receives init; stays silent on the port */
    });
    host.deliver({
      data: { v: 1, type: 'plugin-hello', apiVersion: 1 },
      origin: 'null',
      source: pluginWin,
      ports: [],
    });
    // Attacker spoofs `ready` straight onto the host window (not over the port).
    host.deliver({ data: { v: 1, type: 'ready' }, origin: 'null', source: pluginWin, ports: [] });

    // The window-delivered `ready` must be ignored; only the private port counts.
    await expect(connected).rejects.toMatchObject({ reason: 'ready-timeout' });
  }, 1000);
});
