// §6.4 App-plugin lifecycle adapter — hosted apps (is-it-up axis).
//
// Native vocabulary is the canonical `plugin://state` enum from the app-plugins spec:
//   stopped → building → starting → health-probing → ready-probing → healthy → (error | stopped)
// The plugin manager emits `plugin://state` events `{ id, state }` (push-style, §8),
// so this feeds the board live. This answers "is the hosted app up", not "what stage
// is the work in" — deliberately a second card on a second axis, grouped by `family`
// with §6.2's post-run card (the Audience double-count is by design, not a bug).
//
// NOTE: this source has NO DATA until `feat/app-plugins` merges (the plugin manager
// that emits these events is on that branch). The adapter is coded to the documented
// shape so it lights up the moment that branch lands — until then it simply produces
// no cards (degrades to nothing, never a wrong stage).

import type { ProjectCard, Source, StageOverride } from '../model';
import { resolveStage, applyOverride } from '../stage';

export const APP_PLUGIN_SOURCE: Source = 'app-plugin';

/** The seven canonical `plugin://state` values (app-plugins spec §3). */
export type PluginState =
  | 'stopped'
  | 'building'
  | 'starting'
  | 'health-probing'
  | 'ready-probing'
  | 'healthy'
  | 'error';

/** A registered hosted app + its current lifecycle state (from `plugins_list` + events). */
export interface PluginUnit {
  id: string;
  name: string;
  state: PluginState;
  url?: string;
  /** Last stderr line on error (`detail`), per §6.4. */
  errorLine?: string;
}

// §6.4 mapping table — plugin://state → canonical stage (operational axis).
function pipelineFor(state: PluginState): {
  stage: 'Build' | 'Live' | null;
  terminal: boolean;
  detail: string;
} {
  switch (state) {
    case 'stopped':
      return { stage: null, terminal: false, detail: 'stopped' }; // → Idle
    case 'building':
      return { stage: 'Build', terminal: false, detail: 'building images' };
    case 'starting':
      return { stage: 'Build', terminal: false, detail: 'starting' };
    case 'health-probing':
      return { stage: 'Build', terminal: false, detail: 'health probe' };
    case 'ready-probing':
      return { stage: 'Build', terminal: false, detail: 'ready probe' };
    case 'healthy':
      return { stage: 'Live', terminal: false, detail: 'serving' };
    case 'error':
      return { stage: null, terminal: true, detail: 'error' }; // → Failed
  }
}

export interface AppPluginAdapterOpts {
  overrides?: Record<string, StageOverride>;
  now?: () => Date;
  staleAfterSec?: number;
}

/** Derive one hosted app's `ProjectCard` from its lifecycle state. */
export function appPluginCard(u: PluginUnit, opts: AppPluginAdapterOpts = {}): ProjectCard {
  const now = opts.now ?? (() => new Date());
  const nowMs = now().getTime();
  const nowIso = now().toISOString();
  const staleAfterSec = opts.staleAfterSec ?? 120; // pushed
  const overrides = opts.overrides ?? {};

  const projectId = `app-plugin:${u.id}`;
  const { stage: pipeline, terminal, detail: mapDetail } = pipelineFor(u.state);
  const inferred = resolveStage({ pipeline, isHumanGate: false, isTerminalFailure: terminal });
  const { stage, stageSource, override, conflict } = applyOverride(inferred, overrides[projectId], nowMs);

  const detail =
    u.state === 'error' && u.errorLine ? `error: ${u.errorLine}` : `${u.name} (app) · ${mapDetail}`;

  return {
    projectId,
    source: APP_PLUGIN_SOURCE,
    name: `${u.name} (app)`,
    stage,
    detail,
    blocked: null, // an app being down is never a human *gate* — it's Failed/Idle, not Blocked
    stageSource,
    override,
    conflict,
    updatedIso: nowIso,
    staleAfterSec,
    health: u.state === 'error' ? 'degraded' : 'ok',
    url: u.url,
    family: u.id, // groups with the same-id work-axis card (e.g. "audience")
  };
}

/** Build cards for all registered hosted apps. Empty list ⇒ no cards (graceful absence). */
export function appPluginCards(units: PluginUnit[], opts: AppPluginAdapterOpts = {}): ProjectCard[] {
  return units.map((u) => appPluginCard(u, opts));
}
