import { describe, it, expect, afterEach } from 'vitest';
import { render, cleanup } from '@testing-library/svelte';
import Dashboard from './Dashboard.svelte';
import { newBoard, replaceSource, applyCard } from '../lib/dashboard/store';
import { fleetCard } from '../lib/dashboard/adapters/fleet';
import { appPluginCard } from '../lib/dashboard/adapters/appPlugin';
import { halyardCards } from '../lib/dashboard/adapters/halyard';
import { audienceCards } from '../lib/dashboard/adapters/audience';

afterEach(cleanup);

const NOW = () => new Date('2026-06-09T12:00:00Z');

async function fixtureBoard() {
  let board = newBoard();
  // Halyard: a release blocked on the flip
  board = replaceSource(
    board,
    'halyard',
    await halyardCards(
      {
        status: async () => [{ release_id: 'r1', app: 'aurora', surface: 'web', version: '4.2', state: 'shipped_dark', flag: 'f', waiting_on: '' }],
        queue: async () => [],
      },
      { now: NOW },
    ),
  );
  // Audience: backend DOWN (degraded lane)
  board = replaceSource(
    board,
    'audience',
    await audienceCards({ health: async () => false, posts: async () => [] }, { now: NOW }),
  );
  // Fleet: a healthy build
  board = applyCard(board, fleetCard({ id: 'u1', phase: 'building', task: 'build the thing', tier: 'T1' }, { now: NOW }));
  // App-plugin: healthy hosted app
  board = applyCard(board, appPluginCard({ id: 'audience', name: 'Audience', state: 'healthy', url: 'http://localhost:3000' }, { now: NOW }));
  return board;
}

describe('Dashboard.svelte', () => {
  it('renders a card per project across all four sources', async () => {
    const board = await fixtureBoard();
    const { getAllByTestId } = render(Dashboard, { props: { initialBoard: board } });
    const cards = getAllByTestId('card');
    // halyard(1) + audience-down(1) + fleet(1) + app-plugin(1)
    expect(cards.length).toBe(4);
  });

  it('shows the "needs you" count (the Halyard flip block)', async () => {
    const board = await fixtureBoard();
    const { getByTestId } = render(Dashboard, { props: { initialBoard: board } });
    expect(getByTestId('needs-me').textContent).toContain('1');
  });

  it('greys the down source and renders the degradation banner', async () => {
    const board = await fixtureBoard();
    const { getByTestId, getAllByTestId } = render(Dashboard, { props: { initialBoard: board } });
    expect(getByTestId('degraded').textContent).toContain('AUDIENCE');
    const audienceCardEl = getAllByTestId('card').find(
      (el) => el.getAttribute('data-project-id')?.startsWith('audience:'),
    )!;
    expect(audienceCardEl.getAttribute('data-health')).toBe('unknown');
    expect(audienceCardEl.className).toContain('unknown');
  });

  it('renders the blocked "act here" deep-link affordance', async () => {
    const board = await fixtureBoard();
    const { getAllByTestId } = render(Dashboard, { props: { initialBoard: board } });
    const acts = getAllByTestId('act');
    expect(acts.length).toBe(1);
    expect(acts[0].textContent).toContain('Flip');
  });

  it('renders the canonical stage on each card', async () => {
    const board = await fixtureBoard();
    const { getAllByTestId } = render(Dashboard, { props: { initialBoard: board } });
    const stages = getAllByTestId('card').map((el) => el.getAttribute('data-stage'));
    expect(stages).toContain('Blocked'); // halyard flip
    expect(stages).toContain('Build'); // fleet
    expect(stages).toContain('Live'); // app-plugin
    expect(stages).toContain('Idle'); // audience down
  });
});
