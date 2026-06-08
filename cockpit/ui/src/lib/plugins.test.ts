import { describe, it, expect } from 'vitest';
import { chipFor, type PluginState } from './plugins';

describe('chipFor', () => {
  it('maps each canonical state to a class + label, with no chip for stopped', () => {
    const cases: Record<PluginState, string | null> = {
      stopped: null,
      building: 'building',
      starting: 'starting',
      'health-probing': 'health-probing',
      'ready-probing': 'ready-probing',
      healthy: 'healthy',
      error: 'error',
    };
    for (const [state, expected] of Object.entries(cases)) {
      expect(chipFor(state as PluginState)?.cls ?? null).toBe(expected);
    }
  });

  it('marks error and healthy and busy tones correctly', () => {
    expect(chipFor('error')!.tone).toBe('bad');
    expect(chipFor('healthy')!.tone).toBe('ok');
    expect(chipFor('building')!.tone).toBe('busy');
  });
});
