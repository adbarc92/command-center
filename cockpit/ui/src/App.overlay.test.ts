import { describe, it, expect, afterEach, beforeEach, vi } from 'vitest';
import { render, cleanup, waitFor, fireEvent } from '@testing-library/svelte';
import App from './App.svelte';
import { fleet } from './lib/store.svelte';
import { newUnit } from './lib/fleet';
import * as api from './lib/api';

// App.overlay integration: the host overlay (Lane O) must pop off the shared `fleet`
// store regardless of the active view, and freeze the rest of the app with `inert`.
// We stub the network so App.onMount's fleet.start() is inert in jsdom.
beforeEach(() => {
  vi.spyOn(api, 'health').mockRejectedValue(new Error('no daemon in test'));
  vi.spyOn(api, 'listUnits').mockResolvedValue([]);
  vi.spyOn(api, 'sendCommand').mockResolvedValue(200);
  // Reset shared-store state between tests (it's a module singleton).
  fleet.units = {};
  fleet.order = [];
  fleet.selectedId = null;
  fleet.pendingLaunch = null;
});

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

function seedAwaiting(id = 'u1') {
  const u = newUnit(id, 'add sum', 'T1');
  u.phase = 'awaiting_oracle_approval';
  u.oracleFiles = ['test/sum.test.js'];
  fleet.units[id] = u;
  fleet.order = [...fleet.order, id];
}

describe('App host overlay', () => {
  it('does not show the overlay when no unit awaits approval', async () => {
    const { queryByTestId } = render(App);
    await waitFor(() => expect(queryByTestId('hud')).not.toBeNull());
    expect(queryByTestId('approval-modal')).toBeNull();
    // `inert` is reflected as a DOM property (jsdom does not mirror it to an attribute).
    expect((queryByTestId('hud') as HTMLElement & { inert: boolean }).inert).toBe(false);
  });

  it('pops the oracle modal and marks the content inert when a unit awaits approval', async () => {
    seedAwaiting('u1');
    const { getByTestId } = render(App);
    await waitFor(() => expect(getByTestId('approval-modal')).not.toBeNull());
    expect(getByTestId('approval-files').textContent).toContain('test/sum.test.js');
    // The whole content subtree is inert so nothing beneath can take input.
    expect((getByTestId('hud') as HTMLElement & { inert: boolean }).inert).toBe(true);
  });

  it('wires APPROVE to fleet.cmd(id, approve_oracle) for the awaiting unit', async () => {
    seedAwaiting('u7');
    const { getByTestId } = render(App);
    await waitFor(() => expect(getByTestId('approval-approve')).not.toBeNull());
    await fireEvent.click(getByTestId('approval-approve'));
    expect(api.sendCommand).toHaveBeenCalledWith('u7', 'approve_oracle');
  });

  it('wires REJECT to fleet.cmd(id, reject_oracle)', async () => {
    seedAwaiting('u8');
    const { getByTestId } = render(App);
    await waitFor(() => expect(getByTestId('approval-reject')).not.toBeNull());
    await fireEvent.click(getByTestId('approval-reject'));
    expect(api.sendCommand).toHaveBeenCalledWith('u8', 'reject_oracle');
  });

  it('shows the modal even while the Projects view is active (view-independent)', async () => {
    seedAwaiting('u1');
    const { getByTestId } = render(App);
    await waitFor(() => expect(getByTestId('switch-projects')).not.toBeNull());
    await fireEvent.click(getByTestId('switch-projects'));
    // Still present after switching away from Fleet.
    expect(getByTestId('approval-modal')).not.toBeNull();
    expect((getByTestId('hud') as HTMLElement & { inert: boolean }).inert).toBe(true);
  });
});
