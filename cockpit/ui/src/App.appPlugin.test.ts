import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';

// App.appPlugin integration: activating an app-plugin tab must never block the shell on the
// plugin's start sequence, and must never composite the child webview before the plugin is
// actually serving. Found by the Phase-6 interactive smoke (checklist 1.5): `plugin_launch`
// was a *synchronous* Tauri command, so it ran on the main event-loop thread and froze the
// whole UI for the duration of `docker compose build` + the health/ready probe budget.
//
// The fix makes `plugin_launch` return as soon as the sequence is spawned. That changes its
// contract: it is a *dispatch ack*, NOT a completion signal. These tests pin the consequence —
// lifecycle truth arrives only via `plugin://state` events.
//
// The Tauri mocks must be installed before App.svelte is imported (it imports invoke/listen at
// module scope), hence `vi.hoisted` for the state the factories close over.
const { invokeMock, listeners } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  listeners: new Map<string, (e: { payload: { id: string; state: string } }) => void>(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (cmd: string, args?: unknown) => invokeMock(cmd, args),
}));
vi.mock('@tauri-apps/api/event', () => ({
  listen: (name: string, cb: (e: { payload: { id: string; state: string } }) => void) => {
    listeners.set(name, cb);
    return Promise.resolve(() => listeners.delete(name));
  },
}));

import { tick } from 'svelte';
import { render, cleanup, waitFor, fireEvent } from '@testing-library/svelte';
import App from './App.svelte';
import { fleet } from './lib/store.svelte';
import * as api from './lib/api';

const AUDIENCE = { id: 'audience', name: 'Audience', icon: '', url: 'http://localhost:3000' };
const TAB = 'switch-app:audience';

/** Push a `plugin://state` event the way the Rust `TauriEventSink` does. */
function emitState(id: string, state: string) {
  listeners.get('plugin://state')?.({ payload: { id, state } });
}

/** invoke() calls recorded for one command name. */
const calls = (cmd: string) => invokeMock.mock.calls.filter((c) => c[0] === cmd);

beforeEach(() => {
  // jsdom has no ResizeObserver; the rect-glue $effect constructs one as soon as an app tab
  // is active. A no-op stub keeps that effect from throwing (rect glue is a GUI concern —
  // it is covered by smoke checklist 1.6, not here).
  vi.stubGlobal(
    'ResizeObserver',
    class {
      observe() {}
      unobserve() {}
      disconnect() {}
    },
  );

  // Stub the network so App.onMount's fleet.start() is inert in jsdom (as in App.overlay.test).
  vi.spyOn(api, 'health').mockRejectedValue(new Error('no daemon in test'));
  vi.spyOn(api, 'listUnits').mockResolvedValue([]);
  fleet.units = {};
  fleet.order = [];
  fleet.selectedId = null;
  fleet.pendingLaunch = null;

  listeners.clear();
  invokeMock.mockReset();
  invokeMock.mockImplementation((cmd: string) => {
    if (cmd === 'plugins_list') return Promise.resolve([AUDIENCE]);
    // Every other command (plugin_launch/show/hide/set_rect, scan_local_projects) acks.
    return Promise.resolve(undefined);
  });
});

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe('App app-plugin activation', () => {
  it('does not composite the webview until the plugin reports healthy', async () => {
    const { getByTestId } = render(App);
    await waitFor(() => expect(getByTestId(TAB)).not.toBeNull());

    await fireEvent.click(getByTestId(TAB));
    await waitFor(() => expect(calls('plugin_launch')).toHaveLength(1));

    // `plugin_launch` resolving means "sequence dispatched", not "app is up". Compositing
    // here would point the child webview at a URL that is not serving yet.
    expect(calls('plugin_show')).toHaveLength(0);

    // Intermediate lifecycle states must not composite either.
    emitState('audience', 'building');
    await tick();
    emitState('audience', 'starting');
    await tick();
    expect(calls('plugin_show')).toHaveLength(0);

    // Only `healthy` composites.
    emitState('audience', 'healthy');
    await waitFor(() => expect(calls('plugin_show')).toHaveLength(1));
  });

  it('never composites the webview when the start sequence fails', async () => {
    const { getByTestId } = render(App);
    await waitFor(() => expect(getByTestId(TAB)).not.toBeNull());

    await fireEvent.click(getByTestId(TAB));
    await waitFor(() => expect(calls('plugin_launch')).toHaveLength(1));

    emitState('audience', 'building');
    await tick();
    emitState('audience', 'error');
    await tick();

    expect(calls('plugin_show')).toHaveLength(0);
  });
});
