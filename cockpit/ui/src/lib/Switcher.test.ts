import { describe, it, expect, afterEach, vi } from 'vitest';
import { render, cleanup, fireEvent } from '@testing-library/svelte';
import Switcher from './Switcher.svelte';

afterEach(cleanup);

const VIEWS = [
  { id: 'fleet', label: 'FLEET' },
  { id: 'projects', label: 'PROJECTS', badge: 3 },
];

describe('Switcher.svelte', () => {
  it('renders one segment per view and marks the active one pressed', () => {
    const { getByTestId } = render(Switcher, { props: { views: VIEWS, active: 'fleet', onselect: () => {} } });
    expect(getByTestId('switch-fleet').getAttribute('aria-pressed')).toBe('true');
    expect(getByTestId('switch-projects').getAttribute('aria-pressed')).toBe('false');
  });

  it('renders a badge only when a positive count is supplied', () => {
    const { getByTestId, queryByTestId } = render(Switcher, {
      props: { views: VIEWS, active: 'fleet', onselect: () => {} },
    });
    expect(getByTestId('switch-badge-projects').textContent).toContain('3');
    expect(queryByTestId('switch-badge-fleet')).toBeNull();
  });

  it('fires onselect with the clicked view id', async () => {
    const onselect = vi.fn();
    const { getByTestId } = render(Switcher, { props: { views: VIEWS, active: 'fleet', onselect } });
    await fireEvent.click(getByTestId('switch-projects'));
    expect(onselect).toHaveBeenCalledWith('projects');
  });
});
