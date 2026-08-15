import { describe, it, expect } from 'vitest';
import {
  validateManifest,
  negotiateCapabilities,
  pluginSrc,
  discoverFrom,
  devPluginSource,
  packagedPluginSource,
  type PluginManifest,
} from './loader';
import { HOST_API_VERSION } from './bridge';

const good = {
  id: 'reference',
  name: 'Reference',
  version: '0.1.0',
  apiVersion: HOST_API_VERSION,
  capabilities: ['log-append'],
  entry: 'index.html',
};

describe('validateManifest', () => {
  it('accepts a well-formed manifest', () => {
    const r = validateManifest(good);
    expect(r.ok).toBe(true);
  });

  it('refuses an unsupported apiVersion (host will not load it)', () => {
    const r = validateManifest({ ...good, apiVersion: 99 });
    expect(r).toEqual({ ok: false, reason: 'unsupported-apiVersion' });
  });

  it('rejects a bad id, missing entry, path-traversal entry, and non-object', () => {
    expect(validateManifest({ ...good, id: 'Bad Id!' }).ok).toBe(false);
    expect(validateManifest({ ...good, entry: '' }).ok).toBe(false);
    expect(validateManifest({ ...good, entry: '../escape.html' })).toEqual({ ok: false, reason: 'bad-entry' });
    expect(validateManifest(null)).toEqual({ ok: false, reason: 'not-an-object' });
    expect(validateManifest({ ...good, capabilities: 'nope' }).ok).toBe(false);
  });
});

describe('negotiateCapabilities', () => {
  it('intersects the requested set with the host set (dropping unknowns)', () => {
    const m: PluginManifest = { ...good, capabilities: ['log-append', 'mystery-power'] };
    expect(negotiateCapabilities(m)).toEqual(['log-append']);
  });
});

describe('pluginSrc', () => {
  it('resolves the dev static route', () => {
    expect(pluginSrc('dev', 'reference', 'index.html')).toBe('/plugins/reference/index.html');
  });

  // Windows/WebView2 (the primary target) serves a custom scheme at `http://<scheme>.localhost`.
  // A literal `ccplugin://` URL is an EXTERNAL protocol there, and a sandboxed iframe refuses to
  // navigate to one ("Navigation to external protocol blocked by sandbox") — the frame stays blank.
  it('resolves the packaged URL to the http://ccplugin.localhost origin on Windows', () => {
    expect(pluginSrc('packaged', 'reference', 'index.html', true)).toBe(
      'http://ccplugin.localhost/reference/index.html',
    );
  });

  it('resolves the packaged URL to the ccplugin:// scheme off Windows', () => {
    expect(pluginSrc('packaged', 'reference', 'index.html', false)).toBe(
      'ccplugin://localhost/reference/index.html',
    );
  });
});

describe('discoverFrom', () => {
  it('partitions valid vs rejected and dedups ids', () => {
    const res = discoverFrom('dev', [good, { ...good, apiVersion: 5 }, good]);
    expect(res.plugins).toHaveLength(1);
    expect(res.plugins[0].src).toBe('/plugins/reference/index.html');
    expect(res.plugins[0].grantedCapabilities).toEqual(['log-append']);
    expect(res.rejected.map((r) => r.reason)).toEqual(['unsupported-apiVersion', 'duplicate-id']);
  });
});

describe('plugin sources (dev index | packaged scan)', () => {
  it('devPluginSource discovers from a build-time index', async () => {
    const src = devPluginSource({ manifests: [good] });
    const { plugins } = await src.discover();
    expect(plugins.map((p) => p.manifest.id)).toEqual(['reference']);
    expect(plugins[0].src.startsWith('/plugins/')).toBe(true);
  });

  it('packagedPluginSource discovers from an injected scan and resolves scheme URLs', async () => {
    const src = packagedPluginSource(async () => [good]);
    const { plugins } = await src.discover();
    expect(plugins[0].src).toBe('ccplugin://localhost/reference/index.html');
  });
});
