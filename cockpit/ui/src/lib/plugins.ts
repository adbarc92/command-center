import { invoke } from '@tauri-apps/api/core';

export type PluginState =
  | 'stopped' | 'building' | 'starting'
  | 'health-probing' | 'ready-probing' | 'healthy' | 'error';

export interface PluginMeta { id: string; name: string; icon: string; url: string; }

export interface Chip { cls: PluginState; label: string; tone: 'ok' | 'bad' | 'busy'; }

const LABELS: Record<Exclude<PluginState, 'stopped'>, string> = {
  building: 'BUILDING', starting: 'STARTING',
  'health-probing': 'HEALTH', 'ready-probing': 'READY?', healthy: 'LIVE', error: 'ERROR',
};

export function chipFor(state: PluginState): Chip | null {
  if (state === 'stopped') return null;
  const tone: Chip['tone'] = state === 'error' ? 'bad' : state === 'healthy' ? 'ok' : 'busy';
  return { cls: state, label: LABELS[state], tone };
}

export const listPlugins = (): Promise<PluginMeta[]> => invoke('plugins_list');
export const launchPlugin = (id: string): Promise<void> => invoke('plugin_launch', { id });
