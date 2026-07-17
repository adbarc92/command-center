// View-plugin loader (Lane V): discovery, manifest validation, apiVersion gate,
// capability negotiation, and iframe-src resolution across the dev/packaged seam.
//
// Discovery is a single injected interface (`PluginSource`) with two impls:
//   • dev (`vite dev`): a build-time-generated dev index of raw manifests; assets are
//     served by a Vite static route at `/plugins/<id>/…` (no Tauri scheme exists here).
//   • packaged: raw manifests scanned from bundled resources ∪ `~/.command-center/plugins`;
//     assets are served by the `ccplugin://` scheme handler (shared opaque origin).
//
// The loader never trusts a manifest: it validates shape, refuses an unsupported
// apiVersion, and intersects the plugin's requested capabilities with the host's.

import { HOST_API_VERSION, HOST_CAPABILITIES } from './bridge';

export interface PluginManifest {
  id: string;
  name: string;
  version: string;
  apiVersion: number;
  capabilities: string[];
  /** Entry document (relative), e.g. `index.html`. */
  entry: string;
}

export type PluginEnv = 'dev' | 'packaged';

export interface DiscoveredPlugin {
  manifest: PluginManifest;
  /** Capabilities the host actually granted = manifest.capabilities ∩ host capabilities. */
  grantedCapabilities: string[];
  /** The sandboxed-iframe `src` for this plugin's entry document. */
  src: string;
}

export interface RejectedPlugin {
  raw: unknown;
  reason: string;
}

export interface DiscoveryResult {
  plugins: DiscoveredPlugin[];
  rejected: RejectedPlugin[];
}

export interface PluginSource {
  discover(): Promise<DiscoveryResult>;
}

export type ManifestResult =
  | { ok: true; manifest: PluginManifest }
  | { ok: false; reason: string };

const ID_RE = /^[a-z0-9][a-z0-9-]*$/;

/**
 * Validate a raw manifest for shape + apiVersion. A malformed manifest, or one asking
 * for an apiVersion the host doesn't implement, is refused (the host won't load it).
 */
export function validateManifest(raw: unknown): ManifestResult {
  if (typeof raw !== 'object' || raw === null) return { ok: false, reason: 'not-an-object' };
  const m = raw as Record<string, unknown>;
  if (typeof m.id !== 'string' || !ID_RE.test(m.id)) return { ok: false, reason: 'bad-id' };
  if (typeof m.name !== 'string' || m.name.length === 0) return { ok: false, reason: 'bad-name' };
  if (typeof m.version !== 'string') return { ok: false, reason: 'bad-version' };
  if (typeof m.apiVersion !== 'number') return { ok: false, reason: 'bad-apiVersion' };
  if (m.apiVersion !== HOST_API_VERSION) return { ok: false, reason: 'unsupported-apiVersion' };
  if (!Array.isArray(m.capabilities) || !m.capabilities.every((c) => typeof c === 'string')) {
    return { ok: false, reason: 'bad-capabilities' };
  }
  if (typeof m.entry !== 'string' || m.entry.length === 0) return { ok: false, reason: 'bad-entry' };
  // Reject path-traversal / absolute entries — the scheme handler serves under <id>/.
  if (m.entry.includes('..') || m.entry.startsWith('/')) return { ok: false, reason: 'bad-entry' };
  return {
    ok: true,
    manifest: {
      id: m.id,
      name: m.name,
      version: m.version,
      apiVersion: m.apiVersion,
      capabilities: m.capabilities as string[],
      entry: m.entry,
    },
  };
}

/** The capabilities the host will actually grant: manifest ∩ host set (order = host's). */
export function negotiateCapabilities(
  manifest: PluginManifest,
  hostCaps: readonly string[] = HOST_CAPABILITIES,
): string[] {
  return hostCaps.filter((c) => manifest.capabilities.includes(c));
}

/**
 * The sandboxed-iframe `src` for a plugin entry. All packaged plugins share the opaque
 * `http://ccplugin.localhost` origin (isolation is the sandbox, not the URL); `<id>` is
 * only a path. Under plain Vite there is no Tauri scheme, so a static route is used.
 */
export function pluginSrc(env: PluginEnv, id: string, entry: string): string {
  return env === 'packaged'
    ? `ccplugin://localhost/${id}/${entry}`
    : `/plugins/${id}/${entry}`;
}

/** Validate + negotiate + resolve a batch of raw manifests, partitioning valid/rejected. */
export function discoverFrom(env: PluginEnv, rawManifests: unknown[]): DiscoveryResult {
  const plugins: DiscoveredPlugin[] = [];
  const rejected: RejectedPlugin[] = [];
  const seen = new Set<string>();
  for (const raw of rawManifests) {
    const res = validateManifest(raw);
    if (!res.ok) {
      rejected.push({ raw, reason: res.reason });
      continue;
    }
    if (seen.has(res.manifest.id)) {
      rejected.push({ raw, reason: 'duplicate-id' });
      continue;
    }
    seen.add(res.manifest.id);
    plugins.push({
      manifest: res.manifest,
      grantedCapabilities: negotiateCapabilities(res.manifest),
      src: pluginSrc(env, res.manifest.id, res.manifest.entry),
    });
  }
  return { plugins, rejected };
}

/** Dev discovery from a build-time-generated index of raw manifests. */
export function devPluginSource(index: { manifests: unknown[] }): PluginSource {
  return { discover: async () => discoverFrom('dev', index.manifests) };
}

/**
 * Packaged discovery: `scan` yields raw manifests from bundled resources ∪
 * `~/.command-center/plugins` (a Tauri command, injected so the loader stays testable).
 */
export function packagedPluginSource(scan: () => Promise<unknown[]>): PluginSource {
  return {
    discover: async () => {
      const raw = await scan();
      return discoverFrom('packaged', raw);
    },
  };
}
