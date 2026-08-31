import type { RegistryEntry } from './types'

/**
 * The project registry — spec §5.1, with one deliberate deviation: a typed TS
 * module rather than registry.yml (see the plan's "deliberate deviations").
 *
 * Entries are EXPLICIT. There is no slug-to-repo inference anywhere in this
 * Worker: a wrong guess writes a user's bug report into a stranger's repository.
 *
 * An entry with no shipping sender (e.g. pawsport) is crash-only — it receives
 * Sentry-created issues and appears on the board, but no app POSTs to it.
 */
export const REGISTRY: Record<string, RegistryEntry> = {
  tenzy:           { repo: 'adbarc92/tenzy',           account: 'primary',   labels: ['telltale'] },
  giftkeeper:      { repo: 'adbarc92/parcle',          account: 'primary',   labels: ['telltale'] },
  purposefull:     { repo: 'OpenBarclay/purposefull',  account: 'secondary', labels: ['telltale'] },
  ironsoul:        { repo: 'adbarc92/ironsoul',        account: 'primary',   labels: ['telltale'] },
  audience:        { repo: 'adbarc92/audience',        account: 'primary',   labels: ['telltale'] },
  lineage:         { repo: 'adbarc92/lineage',         account: 'primary',   labels: ['telltale'] },
  'robo.learn':    { repo: 'OpenBarclay/robo.learn',   account: 'secondary', labels: ['telltale'] },
  'prima-tactica': { repo: 'adbarc92/prima-tactica',   account: 'primary',   labels: ['telltale', 'game'] },
  hexy:            { repo: 'adbarc92/hexy',            account: 'primary',   labels: ['telltale', 'game'] },

  // Crash-only: archived on GitHub, so it cannot receive issue writes (spec §5.3).
  pawsport:        { repo: 'adbarc92/telltale-intake', account: 'primary',   labels: ['telltale'] },

  // The live grader's target (spec §9.1). NEVER a product repo: the grader
  // creates real issues, and pointing it at a shipped product would publish
  // synthetic reports into a public tracker.
  __probe__:       { repo: 'adbarc92/telltale-probe',  account: 'primary',   labels: ['telltale'] },
}

export function lookup(project: string): RegistryEntry | null {
  return Object.prototype.hasOwnProperty.call(REGISTRY, project)
    ? REGISTRY[project]!
    : null
}
