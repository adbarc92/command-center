//! Guard test: the packaged bundle must actually contain the view-plugin runtime's assets.
//!
//! `view_plugins::plugin_roots` searches, in order:
//!
//!   1. `CC_VIEW_PLUGINS_DEV` — the dev seam, an env var pointing at the repo `plugins/` dir,
//!   2. `resource_dir()/plugins` — the packaged path, i.e. whatever `bundle.resources` ships,
//!   3. `~/.command-center/plugins` — a per-user override.
//!
//! In `tauri dev` the env var is set, so the runtime works and every automated gate is green.
//! A packaged build sets no `CC_*` vars, so it falls through to the resource dir — and
//! `bundle.resources` did not exist at all. `plugins/` and `plugin-sdk/index.js` were never
//! bundled, so `ccplugin://localhost/reference/index.html` 404'd, the frame stayed blank, and
//! `sdk_bytes()` found no SDK, so `plugin-hello` never posted and every handshake timed out.
//!
//! That is D-8, and it is the fourth instance of one pattern in this branch (see D-1, D-2, D-7
//! in `spikes/SPIKE-RESULTS.md`): something is exercised only on the dev path and was never
//! wired for the path that ships. Nothing in CI bundles the app, so no suite could see it.
//!
//! This test makes the class mechanical: delete either mapping from `tauri.conf.json` and this
//! goes red immediately, instead of surfacing as a blank pane in a packaged smoke weeks later.

use std::fs;
use std::path::Path;

/// `bundle.resources` entries required for the view-plugin runtime to work when packaged.
/// Key = repo-relative source (as written in `tauri.conf.json`), value = destination under
/// the app's `resource_dir()`. The destinations are what `plugin_roots` / `sdk_bytes` read.
const REQUIRED_RESOURCES: &[(&str, &str)] = &[
    ("../../../plugins/reference/", "plugins/reference/"),
    ("../../plugin-sdk/index.js", "plugin-sdk/index.js"),
];

fn config() -> serde_json::Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tauri.conf.json");
    let raw = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("{} is not valid JSON: {e}", path.display()))
}

#[test]
fn packaged_bundle_ships_the_view_plugin_root_and_sdk() {
    let cfg = config();
    let resources = cfg
        .get("bundle")
        .and_then(|b| b.get("resources"))
        .unwrap_or_else(|| {
            panic!(
                "tauri.conf.json has no `bundle.resources`. A packaged build then has no \
                 `resource_dir()/plugins` and no `plugin-sdk/index.js`, so every view-plugin \
                 404s and every handshake times out (D-8). Required: {REQUIRED_RESOURCES:?}"
            )
        });
    let map = resources
        .as_object()
        .expect("`bundle.resources` must be an object mapping source -> destination");

    for (src, dest) in REQUIRED_RESOURCES {
        let got = map.get(*src).and_then(|v| v.as_str()).unwrap_or_else(|| {
            panic!(
                "`bundle.resources` is missing the mapping {src:?} -> {dest:?}. Without it the \
                 packaged view-plugin runtime cannot load. Present mappings: {:?}",
                map.keys().collect::<Vec<_>>()
            )
        });
        assert_eq!(
            got, *dest,
            "`bundle.resources[{src:?}]` must land at {dest:?} - that is the path \
             `plugin_roots`/`sdk_bytes` read at runtime"
        );
    }
}

/// The sources named above must exist in the repo, or the bundler silently ships nothing.
#[test]
fn declared_resource_sources_exist_on_disk() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    for (src, _) in REQUIRED_RESOURCES {
        let p = manifest.join(src);
        assert!(
            p.exists(),
            "`bundle.resources` names {src:?}, which resolves to {} - but nothing is there, so \
             the packaged app would ship an empty plugin root",
            p.display()
        );
    }
}

/// The reference plugin's entry point and the SDK it imports must both be present. These are
/// the two exact files a packaged handshake needs: the document, and the module it loads.
#[test]
fn reference_plugin_entry_and_sdk_are_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    for rel in ["../../../plugins/reference/index.html", "../../plugin-sdk/index.js"] {
        let p = manifest.join(rel);
        assert!(p.exists(), "missing {} ({})", rel, p.display());
    }
}
