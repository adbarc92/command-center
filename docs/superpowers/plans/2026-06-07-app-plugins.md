# App Plugins Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Host whole first-party web apps (proving plugin: Audience) inside the Tauri + Svelte command center, via a host-managed lifecycle and a Rust-positioned child webview, switched from a top-level switcher.

**Architecture:** A `PluginManager` in the `app` Tauri crate reads `app-plugin.json` manifests, runs each app's backend through a `building → starting → health-probing → ready-probing → healthy` state machine (built over injected `Probe`/`Spawner`/`Clock`/`EventSink` seams so it unit-tests without Docker or real time), then shows the app's web head in a Tauri child webview positioned over a Svelte-reserved rect. A spike de-risks the webview embedding first; everything upstream of embedding (manifest, discovery, lifecycle) is plain TDD and independent of the spike outcome.

**Tech Stack:** Rust + Tauri v2.11.2 (`unstable` feature for child webviews), `tauri-plugin-shell`, `serde`/`serde_json`; Svelte 5 + TypeScript + Vite; Vitest (added here) for the shell's pure logic; Docker Compose (Audience's backend).

**Source spec:** `docs/superpowers/specs/2026-06-07-app-plugins-design.md` (read it; this plan implements it section-by-section).

---

## Orientation (read before Task 1)

**Crate layout.** The cockpit shell is the Tauri crate at `cockpit/ui/src-tauri/` — package `app`, lib `app_lib`, a *standalone* Cargo workspace (`[workspace]` with no members), Tauri `2.11.2`. Today it holds only `src/lib.rs` (the builder + the `fleetd-serve` sidecar babysitter) and `src/main.rs`. All new Rust lands as modules of this crate under `src/plugins/`.

**Test commands.**
- Rust (run from the crate dir): `cd cockpit/ui/src-tauri && cargo test`. A single test: `cargo test plugins::manifest::tests::rejects_unknown_api_version`.
- Front-end pure logic (added in Phase 5): `cd cockpit/ui && npm run test`.

**Two design rules that shape every task:**
1. **Keep Tauri out of the core.** `manifest`, `discovery`, and the `state` machine must not `use tauri::*` — they depend only on the trait seams. This is what lets them unit-test fast without an `AppHandle`. Only `manager.rs` and `embed.rs` touch Tauri.
2. **The spike is the source of truth for the exact child-webview API.** Tauri's `unstable` child-webview calls (create/position/show/hide) are discovered and recorded in `spikes/SPIKE-RESULTS.md` during Phase 0. Phase 6 uses *those exact calls*. Do not guess the webview API before the spike validates it.

**Canonical state strings** (used by Rust events and the Svelte chips — never re-spell): `stopped`, `building`, `starting`, `health-probing`, `ready-probing`, `healthy`, `error`.

---

## Phase 0 — Spike #1: child-webview embedding (go/no-go)

> This is a **throwaway** branch, not production code. Its only deliverable is `spikes/SPIKE-RESULTS.md` recording go/no-go and the exact `unstable` webview API that worked. Per the spec this layer is spike-and-smoke, not TDD — the tasks below are gated checks, not red-green steps. **Do not proceed to Phase 1 until this phase yields an explicit go (overlay "B") or no-go (fall to "C"), recorded in the results file.**

### Task 0.1: Spike branch + unstable feature

**Files:**
- Modify: `cockpit/ui/src-tauri/Cargo.toml`
- Create: `spikes/SPIKE-RESULTS.md`

- [ ] **Step 1: Create the spike branch**

```bash
git checkout -b spike/app-plugins-webview
```

- [ ] **Step 2: Pin Tauri and enable the `unstable` feature**

In `cockpit/ui/src-tauri/Cargo.toml`, change the `tauri` dependency line so the version is exact-pinned and `unstable` is on (child webviews require it):

```toml
tauri = { version = "=2.11.2", features = ["unstable"] }
```

- [ ] **Step 3: Verify it still builds (the first gate)**

Run: `cd cockpit/ui/src-tauri && cargo build`
Expected: builds clean. If `unstable` breaks the build, that is gate-1 failure — record it and stop.

- [ ] **Step 4: Seed the results file**

Create `spikes/SPIKE-RESULTS.md`:

```markdown
# SPIKE #1 — App-plugin child-webview embedding

Branch: `spike/app-plugins-webview`. Tauri 2.11.2 + `unstable`.
Decision: PENDING (go = overlay "B" / no-go = separate windows "C").

## Gate results
- [ ] 1. `unstable` feature builds (dev + packaged)
- [ ] 2. Child webview renders Audience at :3000 (dev AND packaged)
- [ ] 3. Real-origin behaviors: cookies persist, window.open popup opens, full-page redirect navigates
- [ ] 4. Rust positions webview under a Svelte rect (resize ≤150ms/≤10frames; hide-on-overlay ≤150ms, no stale flash, scroll+focus preserved, ≥10 trials)
- [ ] 5. Lifecycle round-trip: (build →) start → health → ready → show; quit → blocking stop → `docker ps` shows no orphans

## Exact webview API that worked
(record the precise Tauri 2.11 unstable calls used to create/position/show/hide the child webview — Phase 6 copies these verbatim)

## Decision + rationale
```

### Task 0.2: Render Audience in a child webview (gates 2–3)

**Files:**
- Modify: `cockpit/ui/src-tauri/src/lib.rs` (temporary spike code; reverted when the branch is abandoned)

- [ ] **Step 1: Build Audience's images with the dev posture** (so :3000 is reachable for the spike)

```bash
cd D:/MajorProjects/CURRENT/audience
docker compose -f docker-compose.prod.yml build \
  --build-arg NODE_ENV=development --build-arg AI_PROVIDER=fake --build-arg MEDIA_PROVIDER=fake
NODE_ENV=development docker compose -f docker-compose.prod.yml up -d
# wait until `curl -fsS localhost:8080/health` is 200 and `curl -sI localhost:3000` returns 200/3xx
```

- [ ] **Step 2: In the Tauri `setup` hook, create a child webview pointed at `http://localhost:3000`**

Use Tauri 2.11's `unstable` child-webview API (`WebviewBuilder` on the main window, an explicit position + size). Record the exact calls you used in `SPIKE-RESULTS.md` → "Exact webview API that worked".

- [ ] **Step 3: Run dev and confirm Audience renders (gate 2, dev)**

Run: `cd cockpit/ui && npm run desktop`
Expected: Audience's dashboard renders inside the cockpit window. Tick gate 2 (dev half).

- [ ] **Step 4: Exercise real-origin behaviors (gate 3)**

In the embedded Audience: confirm a cookie set by the app persists across a reload; trigger an action that does `window.open` (a popup opens); trigger a navigation that does a full-page redirect (it navigates). Tick gate 3.

- [ ] **Step 5: Confirm in a packaged build (gate 2, packaged)**

Run: `cd cockpit/ui && npm run tauri build` then launch the bundled app.
Expected: Audience still renders in the packaged bundle. Tick gate 2 (packaged half). If packaged fails where dev passed, record it — it is a strong no-go signal.

### Task 0.3: Rust-positioned overlay under a Svelte rect (gate 4 — make-or-break)

**Files:**
- Modify: `cockpit/ui/src-tauri/src/lib.rs`, `cockpit/ui/src/App.svelte` (temporary spike code)

- [ ] **Step 1: Reserve a rect in Svelte and send it to Rust**

In `App.svelte`, add a placeholder `<div bind:this={rectEl}>` filling the content area; on mount and on a `ResizeObserver` callback, read `rectEl.getBoundingClientRect()` and `invoke('spike_set_rect', {x, y, width, height})`.

- [ ] **Step 2: Position/show/hide the webview from Rust**

Implement `spike_set_rect`, `spike_show`, `spike_hide` commands that reposition / show / hide the child webview over the reserved rect. Record the exact positioning calls in the results file.

- [ ] **Step 3: Measure resize tracking against the threshold**

Resize the window repeatedly. Confirm the webview settles to the new rect within **≤150 ms / ≤10 frames** and never paints outside the rect. Record pass/fail.

- [ ] **Step 4: Implement and measure hide-on-overlay (the make-or-break sub-gate)**

Add a fake shell modal toggled by a key. On open → `spike_hide`; on close → `spike_show`. Over **≥10 trials**, confirm: round-trip **≤150 ms**, **no visible flash of stale content**, and the app's **scroll + focus state preserved** across hide/restore. Record pass/fail per the spec's gate-4 conditions.

- [ ] **Step 5: Make the go/no-go call**

If gate 4 holds reliably → lean **go ("B")**. If hide-on-overlay flickers past reasonable tuning, or resize tearing is unavoidable → **no-go ("C")**. Record the decision and rationale.

### Task 0.4: Lifecycle round-trip + teardown (gate 5) and finalize decision

- [ ] **Step 1: Prove the start→show→quit→stop round-trip with no orphans**

From a clean state (no Audience containers): launch the cockpit so it runs `build`(if needed)→`start`→waits for health+ready→shows the webview; then quit the cockpit and confirm teardown.

Run after quit: `docker ps`
Expected: **no Audience containers running** (gate 5). If quit leaves containers, record it — it confirms the blocking-teardown requirement in Phase 4.

- [ ] **Step 2: Write the final decision into `SPIKE-RESULTS.md`**

Set `Decision:` to `go (B)` or `no-go (C)`, tick the gate checkboxes, and ensure the "Exact webview API that worked" section is complete (Phase 6 depends on it).

- [ ] **Step 3: Abandon the spike branch and return to the feature branch**

```bash
git checkout feat/app-plugins
git branch -D spike/app-plugins-webview   # throwaway; results file is re-created in Phase 1
```

> Carry `spikes/SPIKE-RESULTS.md`'s content forward by hand (copy it into the feature branch in Task 1.1) — it's the one artifact that survives the spike.

---

## Phase 1 — Build prerequisites (after a go)

### Task 1.1: Land the spike results + Tauri config on the feature branch

**Files:**
- Create: `spikes/SPIKE-RESULTS.md` (copy of the spike's final content)
- Modify: `cockpit/ui/src-tauri/Cargo.toml`

- [ ] **Step 1: Recreate the results file on `feat/app-plugins`**

Copy the final `spikes/SPIKE-RESULTS.md` content from the spike into the feature branch.

- [ ] **Step 2: Pin Tauri + `unstable` on the feature branch**

In `cockpit/ui/src-tauri/Cargo.toml`:

```toml
tauri = { version = "=2.11.2", features = ["unstable"] }
```

- [ ] **Step 3: Verify build**

Run: `cd cockpit/ui/src-tauri && cargo build`
Expected: clean build.

- [ ] **Step 4: Commit**

```bash
git add spikes/SPIKE-RESULTS.md cockpit/ui/src-tauri/Cargo.toml
git commit -m "chore(app-plugins): pin Tauri + enable unstable; record spike #1 results"
```

### Task 1.2: Webview capabilities + label scheme

**Files:**
- Modify: `cockpit/ui/src-tauri/capabilities/default.json`

- [ ] **Step 1: Add webview-API permissions for dynamically-created app webviews**

App webviews use the label scheme `app::<plugin-id>` (unique, stable across relaunch/adopt). The capability must cover those labels, not just `"main"`. Edit `capabilities/default.json` to add the webview permissions and a window/webview entry matching `app::*`:

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "enables the default permissions",
  "windows": ["main"],
  "webviews": ["main", "app::*"],
  "permissions": [
    "core:default",
    "core:webview:default",
    "core:webview:allow-create-webview",
    "core:webview:allow-set-webview-position",
    "core:webview:allow-set-webview-size",
    "core:webview:allow-webview-show",
    "core:webview:allow-webview-hide",
    "core:webview:allow-webview-close",
    {
      "identifier": "shell:allow-execute",
      "allow": [{ "name": "binaries/fleetd-serve", "sidecar": true, "args": true }]
    }
  ]
}
```

> If the spike recorded a different/narrower permission set as the one that actually worked, use that exact set instead — the spike is authoritative for the webview API surface.

- [ ] **Step 2: Verify the config parses (build picks it up)**

Run: `cd cockpit/ui/src-tauri && cargo build`
Expected: clean build; no capability-schema errors.

- [ ] **Step 3: Commit**

```bash
git add cockpit/ui/src-tauri/capabilities/default.json
git commit -m "feat(app-plugins): grant child-webview capabilities for app::* labels"
```

---

## Phase 2 — Manifest + discovery (pure Rust, full TDD)

> No Tauri imports in this phase. These modules compile and test with `cargo test` and have no dependency on the spike outcome.

### Task 2.1: Manifest types + parsing

**Files:**
- Create: `cockpit/ui/src-tauri/src/plugins/mod.rs`
- Create: `cockpit/ui/src-tauri/src/plugins/manifest.rs`
- Modify: `cockpit/ui/src-tauri/src/lib.rs` (add `mod plugins;`)

- [ ] **Step 1: Write the failing test**

Create `cockpit/ui/src-tauri/src/plugins/manifest.rs` with only the test module first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const AUDIENCE_JSON: &str = r#"{
      "id": "audience",
      "name": "Audience",
      "apiVersion": 1,
      "icon": "icon.svg",
      "url": "http://localhost:3000",
      "lifecycle": {
        "managed": true,
        "cwd": "D:/MajorProjects/CURRENT/audience",
        "build": { "cmd": "docker compose -f docker-compose.prod.yml build",
                   "args": { "NODE_ENV": "development" }, "timeout": 1200000 },
        "start": "docker compose -f docker-compose.prod.yml up",
        "stop": "docker compose -f docker-compose.prod.yml down",
        "env": { "NODE_ENV": "development" },
        "health": { "url": "http://localhost:8080/health", "okStatus": [200], "timeout": 180000, "interval": 1000 },
        "ready": { "url": "http://localhost:3000", "okStatus": [200, 302], "timeout": 180000, "interval": 1000 }
      }
    }"#;

    #[test]
    fn parses_a_full_manifest() {
        let m = Manifest::from_json(AUDIENCE_JSON).expect("should parse");
        assert_eq!(m.id, "audience");
        assert_eq!(m.api_version, 1);
        assert_eq!(m.url, "http://localhost:3000");
        assert_eq!(m.lifecycle.health.ok_status, vec![200]);
        assert_eq!(m.lifecycle.ready.ok_status, vec![200, 302]);
        // webview block omitted → defaults
        assert_eq!(m.webview.popups, Popups::Allow);
        assert_eq!(m.webview.external_links, ExternalLinks::InApp);
        assert_eq!(m.webview.title, "Audience"); // defaults to name
    }
}
```

- [ ] **Step 2: Run it to confirm it fails**

Run: `cd cockpit/ui/src-tauri && cargo test plugins::manifest`
Expected: FAIL — `Manifest` undefined.

- [ ] **Step 3: Write the types + parser above the test module**

```rust
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Deserialize)]
pub struct Manifest {
    pub id: String,
    pub name: String,
    #[serde(rename = "apiVersion")]
    pub api_version: u32,
    #[serde(default)]
    pub icon: String,
    pub url: String,
    pub lifecycle: Lifecycle,
    #[serde(default)]
    pub webview: WebviewCfg,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Lifecycle {
    #[serde(default)]
    pub managed: bool,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub build: Option<BuildStep>,
    pub start: String,
    #[serde(default)]
    pub stop: Option<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    pub health: Probe,
    pub ready: Probe,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BuildStep {
    pub cmd: String,
    #[serde(default)]
    pub args: BTreeMap<String, String>,
    #[serde(default = "default_build_timeout")]
    pub timeout: u64,
}
fn default_build_timeout() -> u64 { 1_200_000 }

#[derive(Debug, Clone, Deserialize)]
pub struct Probe {
    pub url: String,
    #[serde(rename = "okStatus", default = "default_ok_status")]
    pub ok_status: Vec<u16>,
    #[serde(default = "default_probe_timeout")]
    pub timeout: u64,
    #[serde(default = "default_probe_interval")]
    pub interval: u64,
}
fn default_ok_status() -> Vec<u16> { vec![200] }
fn default_probe_timeout() -> u64 { 180_000 }
fn default_probe_interval() -> u64 { 1_000 }

#[derive(Debug, Clone, Deserialize)]
pub struct WebviewCfg {
    #[serde(default)]
    pub popups: Popups,
    #[serde(rename = "externalLinks", default)]
    pub external_links: ExternalLinks,
    #[serde(default)]
    pub title: Option<String>,
}
impl Default for WebviewCfg {
    fn default() -> Self {
        WebviewCfg { popups: Popups::default(), external_links: ExternalLinks::default(), title: None }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Popups { #[default] Allow, Block }

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ExternalLinks { #[default] InApp, SystemBrowser }

impl Manifest {
    pub fn from_json(s: &str) -> Result<Manifest, serde_json::Error> {
        serde_json::from_str(s)
    }
    /// Effective window title (defaults to `name`).
    pub fn window_title(&self) -> String {
        self.webview.title.clone().unwrap_or_else(|| self.name.clone())
    }
}
```

> Note: the test reads `m.webview.title` as a resolved `String`. Adjust the test to call `m.window_title()` instead (since the field is `Option<String>`), or keep the field optional and assert `m.window_title() == "Audience"`. Use `window_title()` in the assertion.

- [ ] **Step 4: Create the module root and wire it in**

`cockpit/ui/src-tauri/src/plugins/mod.rs`:

```rust
pub mod manifest;
```

In `cockpit/ui/src-tauri/src/lib.rs`, add near the top (after the existing `use` lines):

```rust
mod plugins;
```

- [ ] **Step 5: Run the test to confirm it passes**

Run: `cd cockpit/ui/src-tauri && cargo test plugins::manifest`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add cockpit/ui/src-tauri/src/plugins/ cockpit/ui/src-tauri/src/lib.rs
git commit -m "feat(app-plugins): manifest types + JSON parsing with defaults"
```

### Task 2.2: apiVersion refusal + cwd resolution

**Files:**
- Modify: `cockpit/ui/src-tauri/src/plugins/manifest.rs`

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module:

```rust
#[test]
fn rejects_unknown_api_version() {
    let json = AUDIENCE_JSON.replace("\"apiVersion\": 1", "\"apiVersion\": 99");
    let m = Manifest::from_json(&json).unwrap();
    assert!(matches!(m.validate(), Err(ManifestError::UnsupportedApiVersion(99))));
}

#[test]
fn accepts_supported_api_version() {
    let m = Manifest::from_json(AUDIENCE_JSON).unwrap();
    assert!(m.validate().is_ok());
}

#[test]
fn resolves_relative_cwd_against_manifest_dir() {
    use std::path::Path;
    let mut m = Manifest::from_json(AUDIENCE_JSON).unwrap();
    m.lifecycle.cwd = Some("backend".into());
    let resolved = m.resolved_cwd(Path::new("/plugins/audience"));
    assert_eq!(resolved, Path::new("/plugins/audience/backend"));
}

#[test]
fn keeps_absolute_cwd_as_is() {
    use std::path::Path;
    let m = Manifest::from_json(AUDIENCE_JSON).unwrap(); // cwd is absolute D:/...
    let resolved = m.resolved_cwd(Path::new("/plugins/audience"));
    assert_eq!(resolved, Path::new("D:/MajorProjects/CURRENT/audience"));
}
```

- [ ] **Step 2: Run to confirm failure**

Run: `cd cockpit/ui/src-tauri && cargo test plugins::manifest`
Expected: FAIL — `validate`, `resolved_cwd`, `ManifestError` undefined.

- [ ] **Step 3: Implement validation + cwd resolution**

Add to `manifest.rs`:

```rust
use std::path::{Path, PathBuf};

pub const SUPPORTED_API_VERSIONS: &[u32] = &[1];

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ManifestError {
    #[error("unsupported apiVersion: {0}")]
    UnsupportedApiVersion(u32),
}

impl Manifest {
    pub fn validate(&self) -> Result<(), ManifestError> {
        if !SUPPORTED_API_VERSIONS.contains(&self.api_version) {
            return Err(ManifestError::UnsupportedApiVersion(self.api_version));
        }
        Ok(())
    }

    /// Resolve `lifecycle.cwd` relative to the manifest's own directory;
    /// absolute paths pass through unchanged. No cwd → manifest dir.
    pub fn resolved_cwd(&self, manifest_dir: &Path) -> PathBuf {
        match &self.lifecycle.cwd {
            None => manifest_dir.to_path_buf(),
            Some(c) => {
                let p = Path::new(c);
                if p.is_absolute() { p.to_path_buf() } else { manifest_dir.join(p) }
            }
        }
    }
}
```

Add `thiserror = "1"` to `[dependencies]` in `cockpit/ui/src-tauri/Cargo.toml`:

```toml
thiserror = "1"
```

> On Windows, `Path::new("D:/...").is_absolute()` is true. The absolute-cwd test asserts exact equality with the forward-slash form as written in the manifest; if `PathBuf` normalizes separators on your platform and the assert fails, compare with `Path::new(...)` on both sides (already done) — they normalize identically.

- [ ] **Step 4: Run to confirm pass**

Run: `cd cockpit/ui/src-tauri && cargo test plugins::manifest`
Expected: PASS (all manifest tests).

- [ ] **Step 5: Commit**

```bash
git add cockpit/ui/src-tauri/src/plugins/manifest.rs cockpit/ui/src-tauri/Cargo.toml
git commit -m "feat(app-plugins): apiVersion refusal + manifest-relative cwd resolution"
```

### Task 2.3: Discovery seam

**Files:**
- Create: `cockpit/ui/src-tauri/src/plugins/discovery.rs`
- Modify: `cockpit/ui/src-tauri/src/plugins/mod.rs`

- [ ] **Step 1: Write the failing test**

Create `discovery.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn discovers_manifests_and_dedupes_by_id_with_user_dir_winning() {
        let tmp = std::env::temp_dir().join("appplugins_disc_test");
        let _ = fs::remove_dir_all(&tmp);
        let packaged = tmp.join("packaged");
        let user = tmp.join("user");
        fs::create_dir_all(packaged.join("audience")).unwrap();
        fs::create_dir_all(user.join("audience")).unwrap();
        // both define id "audience"; user dir should win
        let base = r#"{"id":"audience","name":"NAME","apiVersion":1,"url":"http://localhost:3000",
            "lifecycle":{"start":"x","health":{"url":"h"},"ready":{"url":"r"}}}"#;
        fs::write(packaged.join("audience/app-plugin.json"), base.replace("NAME", "Packaged")).unwrap();
        fs::write(user.join("audience/app-plugin.json"), base.replace("NAME", "User")).unwrap();

        let found = discover(&[packaged.as_path(), user.as_path()]);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].manifest.name, "User"); // later root wins on id collision
    }
}
```

- [ ] **Step 2: Run to confirm failure**

Run: `cd cockpit/ui/src-tauri && cargo test plugins::discovery`
Expected: FAIL — `discover` / `DiscoveredPlugin` undefined.

- [ ] **Step 3: Implement discovery**

```rust
use crate::plugins::manifest::Manifest;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct DiscoveredPlugin {
    pub dir: PathBuf,
    pub manifest: Manifest,
}

/// Scan each root for `<root>/<plugin>/app-plugin.json`. Later roots override
/// earlier ones on `id` collision (so the user dir wins over packaged).
/// Manifests that fail to parse or fail `validate()` are skipped (logged by caller).
pub fn discover(roots: &[&Path]) -> Vec<DiscoveredPlugin> {
    use std::collections::BTreeMap;
    let mut by_id: BTreeMap<String, DiscoveredPlugin> = BTreeMap::new();
    for root in roots {
        let entries = match std::fs::read_dir(root) {
            Ok(e) => e,
            Err(_) => continue, // missing root is fine
        };
        for entry in entries.flatten() {
            let mani_path = entry.path().join("app-plugin.json");
            let Ok(text) = std::fs::read_to_string(&mani_path) else { continue };
            let Ok(manifest) = Manifest::from_json(&text) else { continue };
            if manifest.validate().is_err() { continue }
            by_id.insert(
                manifest.id.clone(),
                DiscoveredPlugin { dir: entry.path(), manifest },
            );
        }
    }
    by_id.into_values().collect()
}
```

Add `pub mod discovery;` to `mod.rs`.

- [ ] **Step 4: Run to confirm pass**

Run: `cd cockpit/ui/src-tauri && cargo test plugins::discovery`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add cockpit/ui/src-tauri/src/plugins/discovery.rs cockpit/ui/src-tauri/src/plugins/mod.rs
git commit -m "feat(app-plugins): manifest discovery with id-collision precedence"
```

---

## Phase 3 — Lifecycle core (Rust, full TDD with injected seams)

> The state machine drives off four injected traits — `Probe`, `Spawner`, `Clock`, `EventSink` — so timeout→error, partial-stack adopt, and crash→error are all pure-unit-testable without Docker, real time, or a Tauri `AppHandle`. No `use tauri::*` in `state.rs`.

### Task 3.1: The seam traits + fakes

**Files:**
- Create: `cockpit/ui/src-tauri/src/plugins/seams.rs`
- Modify: `cockpit/ui/src-tauri/src/plugins/mod.rs`

- [ ] **Step 1: Define the traits (no test yet — these are pure interfaces)**

Create `seams.rs`:

```rust
use std::collections::BTreeMap;
use std::path::PathBuf;

/// One HTTP probe attempt. Returns the status code, or None on connection error.
pub trait Probe: Send + Sync {
    fn probe(&self, url: &str) -> Option<u16>;
}

/// Spawns the build/start/stop commands. Returns a handle whose `is_alive`
/// the state machine polls and whose exit drives crash→error.
pub trait Spawner: Send + Sync {
    /// Run a command to completion (build/stop). Ok(code).
    fn run_to_completion(&self, cmd: &str, cwd: &PathBuf, env: &BTreeMap<String, String>, timeout_ms: u64) -> i32;
    /// Spawn a long-running command (start). Returns a child id.
    fn spawn(&self, cmd: &str, cwd: &PathBuf, env: &BTreeMap<String, String>) -> u64;
    /// Has the spawned child exited? (drives crash→error)
    fn has_exited(&self, child_id: u64) -> bool;
    /// Force-kill a child (teardown fallback).
    fn kill(&self, child_id: u64);
}

/// Monotonic time, injectable so timeouts test without sleeping.
pub trait Clock: Send + Sync {
    fn now_ms(&self) -> u64;
    fn sleep_ms(&self, ms: u64);
}

/// Where state transitions go (Tauri event in prod; a Vec in tests).
pub trait EventSink: Send + Sync {
    fn emit_state(&self, plugin_id: &str, state: &str);
}
```

- [ ] **Step 2: Add fakes in a test-support module**

Append to `seams.rs`:

```rust
#[cfg(test)]
pub mod fakes {
    use super::*;
    use std::sync::Mutex;

    /// Probe that returns a scripted sequence of statuses per URL.
    pub struct ScriptedProbe { pub responses: Mutex<std::collections::HashMap<String, Vec<Option<u16>>>> }
    impl Probe for ScriptedProbe {
        fn probe(&self, url: &str) -> Option<u16> {
            let mut map = self.responses.lock().unwrap();
            let q = map.get_mut(url).expect("no script for url");
            if q.len() == 1 { q[0] } else { q.remove(0) }
        }
    }

    #[derive(Default)]
    pub struct FakeClock { pub t: Mutex<u64> }
    impl Clock for FakeClock {
        fn now_ms(&self) -> u64 { *self.t.lock().unwrap() }
        fn sleep_ms(&self, ms: u64) { *self.t.lock().unwrap() += ms; } // advance, don't block
    }

    #[derive(Default)]
    pub struct RecordingSink { pub states: Mutex<Vec<(String, String)>> }
    impl EventSink for RecordingSink {
        fn emit_state(&self, id: &str, s: &str) { self.states.lock().unwrap().push((id.into(), s.into())); }
    }

    pub struct FakeSpawner {
        pub start_child_id: u64,
        pub build_exit: i32,
        pub exited: Mutex<bool>,
    }
    impl Spawner for FakeSpawner {
        fn run_to_completion(&self, _c: &str, _w: &PathBuf, _e: &BTreeMap<String, String>, _t: u64) -> i32 { self.build_exit }
        fn spawn(&self, _c: &str, _w: &PathBuf, _e: &BTreeMap<String, String>) -> u64 { self.start_child_id }
        fn has_exited(&self, _id: u64) -> bool { *self.exited.lock().unwrap() }
        fn kill(&self, _id: u64) {}
    }
}
```

Add `pub mod seams;` to `mod.rs`.

- [ ] **Step 3: Verify it compiles**

Run: `cd cockpit/ui/src-tauri && cargo test plugins::seams`
Expected: PASS (0 tests run, compiles clean).

- [ ] **Step 4: Commit**

```bash
git add cockpit/ui/src-tauri/src/plugins/seams.rs cockpit/ui/src-tauri/src/plugins/mod.rs
git commit -m "feat(app-plugins): lifecycle seam traits (Probe/Spawner/Clock/EventSink) + fakes"
```

### Task 3.2: State enum + the happy-path start sequence

**Files:**
- Create: `cockpit/ui/src-tauri/src/plugins/state.rs`
- Modify: `cockpit/ui/src-tauri/src/plugins/mod.rs`

- [ ] **Step 1: Write the failing test (happy path: build → start → health → ready → healthy)**

Create `state.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::manifest::Manifest;
    use crate::plugins::seams::fakes::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    fn manifest() -> Manifest {
        Manifest::from_json(r#"{"id":"audience","name":"Audience","apiVersion":1,
          "url":"http://localhost:3000",
          "lifecycle":{"cwd":"/x","build":{"cmd":"build","timeout":1000},
            "start":"up","stop":"down","env":{},
            "health":{"url":"h","okStatus":[200],"timeout":5000,"interval":1000},
            "ready":{"url":"r","okStatus":[200,302],"timeout":5000,"interval":1000}}}"#).unwrap()
    }

    #[test]
    fn cold_start_walks_building_to_healthy_and_owns_the_stack() {
        let probe = ScriptedProbe { responses: Mutex::new(HashMap::from([
            // adopt check: both down at first
            ("h".to_string(), vec![None, None, Some(200), Some(200)]),
            ("r".to_string(), vec![None, None, Some(302)]),
        ])) };
        let spawner = FakeSpawner { start_child_id: 7, build_exit: 0, exited: Mutex::new(false) };
        let clock = FakeClock::default();
        let sink = RecordingSink::default();

        let outcome = run_start_sequence(&manifest(), std::path::Path::new("/x"),
            &probe, &spawner, &clock, &sink, /*images_present=*/false);

        assert_eq!(outcome, StartOutcome::Healthy { owned: true });
        let states: Vec<String> = sink.states.lock().unwrap().iter().map(|(_, s)| s.clone()).collect();
        assert_eq!(states, vec!["building","starting","health-probing","ready-probing","healthy"]);
    }
}
```

- [ ] **Step 2: Run to confirm failure**

Run: `cd cockpit/ui/src-tauri && cargo test plugins::state`
Expected: FAIL — `run_start_sequence`, `StartOutcome` undefined.

- [ ] **Step 3: Implement the state strings + start sequence**

Above the test module in `state.rs`:

```rust
use crate::plugins::manifest::{Manifest, Probe as ProbeCfg};
use crate::plugins::seams::{Clock, EventSink, Probe, Spawner};
use std::path::Path;

pub const STOPPED: &str = "stopped";
pub const BUILDING: &str = "building";
pub const STARTING: &str = "starting";
pub const HEALTH_PROBING: &str = "health-probing";
pub const READY_PROBING: &str = "ready-probing";
pub const HEALTHY: &str = "healthy";
pub const ERROR: &str = "error";

#[derive(Debug, PartialEq, Eq)]
pub enum StartOutcome {
    Healthy { owned: bool },
    Error(String),
}

/// Poll one probe until a status in `ok_status` appears or `timeout` elapses.
/// Returns true on success, false on timeout. Advances the injected clock.
fn poll_until_ok(cfg: &ProbeCfg, probe: &dyn Probe, clock: &dyn Clock) -> bool {
    let start = clock.now_ms();
    loop {
        if let Some(code) = probe.probe(&cfg.url) {
            if cfg.ok_status.contains(&code) { return true; }
        }
        if clock.now_ms().saturating_sub(start) >= cfg.timeout { return false; }
        clock.sleep_ms(cfg.interval);
    }
}

fn both_probes_pass(m: &Manifest, probe: &dyn Probe) -> bool {
    let h = probe.probe(&m.lifecycle.health.url)
        .map(|c| m.lifecycle.health.ok_status.contains(&c)).unwrap_or(false);
    if !h { return false; }
    probe.probe(&m.lifecycle.ready.url)
        .map(|c| m.lifecycle.ready.ok_status.contains(&c)).unwrap_or(false)
}

#[allow(clippy::too_many_arguments)]
pub fn run_start_sequence(
    m: &Manifest, manifest_dir: &Path,
    probe: &dyn Probe, spawner: &dyn Spawner, clock: &dyn Clock, sink: &dyn EventSink,
    images_present: bool,
) -> StartOutcome {
    let cwd = m.resolved_cwd(manifest_dir);

    // Step 0: build (only if a build step exists and images are absent)
    if let Some(build) = &m.lifecycle.build {
        if !images_present {
            sink.emit_state(&m.id, BUILDING);
            let code = spawner.run_to_completion(&build.cmd, &cwd, &build.args, build.timeout);
            if code != 0 { sink.emit_state(&m.id, ERROR); return StartOutcome::Error(format!("build exited {code}")); }
        }
    }

    // Step 1: adopt check — both probes already up → adopt (not owned)
    if both_probes_pass(m, probe) {
        sink.emit_state(&m.id, HEALTHY);
        return StartOutcome::Healthy { owned: false };
    }

    // Step 2: spawn start (owned)
    sink.emit_state(&m.id, STARTING);
    let _child = spawner.spawn(&m.lifecycle.start, &cwd, &m.lifecycle.env);

    // Step 3: health then ready
    sink.emit_state(&m.id, HEALTH_PROBING);
    if !poll_until_ok(&m.lifecycle.health, probe, clock) {
        sink.emit_state(&m.id, ERROR); return StartOutcome::Error("health probe timed out".into());
    }
    sink.emit_state(&m.id, READY_PROBING);
    if !poll_until_ok(&m.lifecycle.ready, probe, clock) {
        sink.emit_state(&m.id, ERROR); return StartOutcome::Error("ready probe timed out".into());
    }

    // Step 4: healthy
    sink.emit_state(&m.id, HEALTHY);
    StartOutcome::Healthy { owned: true }
}
```

Add `pub mod state;` to `mod.rs`.

> The happy-path test scripts the adopt check to fail first (`None, None`) so the sequence spawns; then health returns `Some(200)` and ready `Some(302)`. The `ScriptedProbe` consumes one scripted response per call and repeats the last — make sure the script has enough entries for: adopt(health), adopt(ready), health-poll, ready-poll.

- [ ] **Step 4: Run to confirm pass**

Run: `cd cockpit/ui/src-tauri && cargo test plugins::state`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add cockpit/ui/src-tauri/src/plugins/state.rs cockpit/ui/src-tauri/src/plugins/mod.rs
git commit -m "feat(app-plugins): lifecycle state machine happy path (building→healthy)"
```

### Task 3.3: Adopt-both-probes, partial-stack fall-through, and timeout→error

**Files:**
- Modify: `cockpit/ui/src-tauri/src/plugins/state.rs`

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module:

```rust
#[test]
fn adopts_when_both_probes_already_pass_and_marks_not_owned() {
    let probe = ScriptedProbe { responses: Mutex::new(HashMap::from([
        ("h".to_string(), vec![Some(200)]),
        ("r".to_string(), vec![Some(200)]),
    ])) };
    let spawner = FakeSpawner { start_child_id: 7, build_exit: 0, exited: Mutex::new(false) };
    let out = run_start_sequence(&manifest(), std::path::Path::new("/x"),
        &probe, &spawner, &FakeClock::default(), &RecordingSink::default(), /*images_present=*/true);
    assert_eq!(out, StartOutcome::Healthy { owned: false });
}

#[test]
fn partial_stack_health_only_falls_through_to_spawn() {
    // health up, ready down at adopt → must NOT adopt; spawn then both come up
    let probe = ScriptedProbe { responses: Mutex::new(HashMap::from([
        ("h".to_string(), vec![Some(200), Some(200)]),       // adopt(h) ok, later health-poll ok
        ("r".to_string(), vec![None, Some(200)]),            // adopt(r) down → fall through; ready-poll ok
    ])) };
    let spawner = FakeSpawner { start_child_id: 7, build_exit: 0, exited: Mutex::new(false) };
    let sink = RecordingSink::default();
    let out = run_start_sequence(&manifest(), std::path::Path::new("/x"),
        &probe, &spawner, &FakeClock::default(), &sink, true);
    assert_eq!(out, StartOutcome::Healthy { owned: true }); // spawned → owned
    let states: Vec<String> = sink.states.lock().unwrap().iter().map(|(_, s)| s.clone()).collect();
    assert!(states.contains(&"starting".to_string()));
}

#[test]
fn health_timeout_yields_error() {
    let probe = ScriptedProbe { responses: Mutex::new(HashMap::from([
        ("h".to_string(), vec![None]),  // never comes up
        ("r".to_string(), vec![None]),
    ])) };
    let spawner = FakeSpawner { start_child_id: 7, build_exit: 0, exited: Mutex::new(false) };
    let sink = RecordingSink::default();
    let out = run_start_sequence(&manifest(), std::path::Path::new("/x"),
        &probe, &spawner, &FakeClock::default(), &sink, true);
    assert!(matches!(out, StartOutcome::Error(_)));
    assert_eq!(sink.states.lock().unwrap().last().unwrap().1, "error");
}

#[test]
fn build_failure_yields_error_before_spawn() {
    let probe = ScriptedProbe { responses: Mutex::new(HashMap::new()) };
    let spawner = FakeSpawner { start_child_id: 7, build_exit: 2, exited: Mutex::new(false) };
    let sink = RecordingSink::default();
    let out = run_start_sequence(&manifest(), std::path::Path::new("/x"),
        &probe, &spawner, &FakeClock::default(), &sink, /*images_present=*/false);
    assert!(matches!(out, StartOutcome::Error(_)));
    assert_eq!(sink.states.lock().unwrap()[0].1, "building");
}
```

- [ ] **Step 2: Run to confirm failure/coverage**

Run: `cd cockpit/ui/src-tauri && cargo test plugins::state`
Expected: the new tests exercise existing code — they should mostly PASS already (the happy-path impl already handles these). If `partial_stack_health_only_falls_through_to_spawn` fails because `both_probes_pass` short-circuits incorrectly, fix `both_probes_pass` to return false as soon as health-then-ready isn't both ok (it already does). Confirm the `ScriptedProbe` scripts have the right call counts.

> This task is primarily *characterization* — it proves the Phase-3.2 implementation already satisfies adopt/partial/timeout/build-fail. If any assertion fails, the fix belongs in `state.rs`, not the test.

- [ ] **Step 3: Commit**

```bash
git add cockpit/ui/src-tauri/src/plugins/state.rs
git commit -m "test(app-plugins): cover adopt-both-probes, partial fall-through, timeout/build errors"
```

### Task 3.4: Crash-while-healthy detection

**Files:**
- Modify: `cockpit/ui/src-tauri/src/plugins/state.rs`

- [ ] **Step 1: Write the failing test**

Add to the `tests` module:

```rust
#[test]
fn crash_while_healthy_transitions_to_error() {
    let spawner = FakeSpawner { start_child_id: 7, build_exit: 0, exited: Mutex::new(true) };
    let sink = RecordingSink::default();
    // Given an owned, healthy plugin whose child has exited, the watcher flips to error.
    let flipped = check_crash(&"audience".to_string(), 7, &spawner, &sink);
    assert!(flipped);
    assert_eq!(sink.states.lock().unwrap().last().unwrap().1, "error");
}

#[test]
fn no_crash_when_child_alive() {
    let spawner = FakeSpawner { start_child_id: 7, build_exit: 0, exited: Mutex::new(false) };
    let sink = RecordingSink::default();
    let flipped = check_crash(&"audience".to_string(), 7, &spawner, &sink);
    assert!(!flipped);
    assert!(sink.states.lock().unwrap().is_empty());
}
```

- [ ] **Step 2: Run to confirm failure**

Run: `cd cockpit/ui/src-tauri && cargo test plugins::state`
Expected: FAIL — `check_crash` undefined.

- [ ] **Step 3: Implement the crash check**

Add to `state.rs`:

```rust
/// If the owned child has exited, emit `error` and return true. The caller is
/// responsible for destroying the kept-alive webview on a true return (§4).
pub fn check_crash(plugin_id: &str, child_id: u64, spawner: &dyn Spawner, sink: &dyn EventSink) -> bool {
    if spawner.has_exited(child_id) {
        sink.emit_state(plugin_id, ERROR);
        true
    } else {
        false
    }
}
```

- [ ] **Step 4: Run to confirm pass**

Run: `cd cockpit/ui/src-tauri && cargo test plugins::state`
Expected: PASS (all state tests).

- [ ] **Step 5: Commit**

```bash
git add cockpit/ui/src-tauri/src/plugins/state.rs
git commit -m "feat(app-plugins): crash-while-healthy → error transition"
```

---

## Phase 4 — Manager + Tauri wiring (real Probe/Spawner/EventSink, integration)

> This phase provides the *real* seam implementations (HTTP probe, shell spawner over `tauri-plugin-shell`, Tauri-event sink) and the `PluginManager` that owns processes and exposes Tauri commands. The pure core from Phase 3 is reused unchanged.

### Task 4.1: Real seam implementations

**Files:**
- Create: `cockpit/ui/src-tauri/src/plugins/seams_impl.rs`
- Modify: `cockpit/ui/src-tauri/src/plugins/mod.rs`, `cockpit/ui/src-tauri/Cargo.toml`

- [ ] **Step 1: Add an HTTP client dep for the probe**

In `cockpit/ui/src-tauri/Cargo.toml` `[dependencies]`:

```toml
ureq = "2"
```

- [ ] **Step 2: Implement the real seams**

Create `seams_impl.rs`:

```rust
use crate::plugins::seams::{Clock, EventSink, Probe, Spawner};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

pub struct HttpProbe;
impl Probe for HttpProbe {
    fn probe(&self, url: &str) -> Option<u16> {
        match ureq::get(url).timeout(Duration::from_millis(2000)).call() {
            Ok(resp) => Some(resp.status()),
            Err(ureq::Error::Status(code, _)) => Some(code), // 3xx/4xx still a status
            Err(_) => None, // connection refused etc.
        }
    }
}

pub struct RealClock { start: Instant }
impl RealClock { pub fn new() -> Self { RealClock { start: Instant::now() } } }
impl Clock for RealClock {
    fn now_ms(&self) -> u64 { self.start.elapsed().as_millis() as u64 }
    fn sleep_ms(&self, ms: u64) { std::thread::sleep(Duration::from_millis(ms)); }
}

pub struct TauriEventSink { pub app: AppHandle }
impl EventSink for TauriEventSink {
    fn emit_state(&self, plugin_id: &str, state: &str) {
        let _ = self.app.emit("plugin://state", serde_json::json!({ "id": plugin_id, "state": state }));
    }
}

/// Spawner over std::process (compose stacks are detached daemons; we track the
/// `up`/`down` invocations, not container PIDs). Child ids index into a table.
#[derive(Default)]
pub struct ShellSpawner { children: Mutex<Vec<Option<std::process::Child>>> }
impl Spawner for ShellSpawner {
    fn run_to_completion(&self, cmd: &str, cwd: &PathBuf, env: &BTreeMap<String, String>, _timeout: u64) -> i32 {
        build_command(cmd, cwd, env).status().map(|s| s.code().unwrap_or(-1)).unwrap_or(-1)
    }
    fn spawn(&self, cmd: &str, cwd: &PathBuf, env: &BTreeMap<String, String>) -> u64 {
        let child = build_command(cmd, cwd, env).spawn().ok();
        let mut t = self.children.lock().unwrap();
        t.push(child);
        (t.len() - 1) as u64
    }
    fn has_exited(&self, id: u64) -> bool {
        let mut t = self.children.lock().unwrap();
        match t.get_mut(id as usize).and_then(|c| c.as_mut()) {
            Some(child) => matches!(child.try_wait(), Ok(Some(_))),
            None => true,
        }
    }
    fn kill(&self, id: u64) {
        let mut t = self.children.lock().unwrap();
        if let Some(Some(child)) = t.get_mut(id as usize) { let _ = child.kill(); }
    }
}

/// Split a shell command string into program + args and apply cwd/env.
/// (Compose commands are space-delimited and quote-free in our manifests.)
fn build_command(cmd: &str, cwd: &PathBuf, env: &BTreeMap<String, String>) -> std::process::Command {
    let mut parts = cmd.split_whitespace();
    let prog = parts.next().unwrap_or("");
    let mut c = std::process::Command::new(prog);
    c.args(parts).current_dir(cwd);
    for (k, v) in env { c.env(k, v); }
    c
}
```

Add `pub mod seams_impl;` to `mod.rs`.

> **Why `std::process` not `app.shell()`:** the design names `tauri-plugin-shell`, but compose `up -d` detaches and the real lifetime we track is the `up`/`down` invocation. `std::process::Command` keeps the spawner in `seams_impl` testable-by-substitution and avoids threading an `AppHandle` into every spawn. If the spike found `app.shell()` necessary for env/PATH reasons on Windows, swap the impl here — the `Spawner` trait is unchanged.

- [ ] **Step 3: Verify it compiles**

Run: `cd cockpit/ui/src-tauri && cargo build`
Expected: clean build.

- [ ] **Step 4: Commit**

```bash
git add cockpit/ui/src-tauri/src/plugins/seams_impl.rs cockpit/ui/src-tauri/src/plugins/mod.rs cockpit/ui/src-tauri/Cargo.toml
git commit -m "feat(app-plugins): real HTTP probe, shell spawner, Tauri event sink"
```

### Task 4.2: PluginManager + Tauri commands (launch / stop / list)

**Files:**
- Create: `cockpit/ui/src-tauri/src/plugins/manager.rs`
- Modify: `cockpit/ui/src-tauri/src/plugins/mod.rs`, `cockpit/ui/src-tauri/src/lib.rs`

- [ ] **Step 1: Implement the manager + commands**

Create `manager.rs`:

```rust
use crate::plugins::discovery::{discover, DiscoveredPlugin};
use crate::plugins::seams_impl::{HttpProbe, RealClock, ShellSpawner, TauriEventSink};
use crate::plugins::state::{run_start_sequence, StartOutcome};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Manager, State};

/// One launched plugin's runtime record.
struct Running { child_id: u64, owned: bool }

#[derive(Default)]
pub struct PluginManager {
    discovered: Mutex<Vec<DiscoveredPlugin>>,
    running: Mutex<HashMap<String, Running>>,
    spawner: ShellSpawner,
}

impl PluginManager {
    pub fn roots() -> Vec<PathBuf> {
        let mut v = Vec::new();
        // dev list: a folder next to the repo's cockpit, plus the user dir
        if let Some(home) = dirs_home() { v.push(home.join(".command-center/app-plugins")); }
        v
    }
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from)
}

#[tauri::command]
pub fn plugins_list(mgr: State<'_, PluginManager>) -> Vec<serde_json::Value> {
    let roots = PluginManager::roots();
    let root_refs: Vec<&std::path::Path> = roots.iter().map(|p| p.as_path()).collect();
    let found = discover(&root_refs);
    let out = found.iter().map(|d| serde_json::json!({
        "id": d.manifest.id, "name": d.manifest.name, "icon": d.manifest.icon, "url": d.manifest.url,
    })).collect();
    *mgr.discovered.lock().unwrap() = found;
    out
}

#[tauri::command]
pub fn plugin_launch(app: AppHandle, mgr: State<'_, PluginManager>, id: String) -> Result<(), String> {
    let disc = { mgr.discovered.lock().unwrap().iter().find(|d| d.manifest.id == id).cloned() };
    let Some(disc) = disc else { return Err(format!("unknown plugin {id}")) };
    let probe = HttpProbe;
    let clock = RealClock::new();
    let sink = TauriEventSink { app: app.clone() };
    let images_present = images_present(&disc.manifest, &probe);
    let outcome = run_start_sequence(&disc.manifest, &disc.dir, &probe, &mgr.spawner, &clock, &sink, images_present);
    match outcome {
        StartOutcome::Healthy { owned } => {
            // record so we know what to tear down on quit (Task 4.3)
            let child_id = 0; // ShellSpawner returns ids; for adopted stacks there is no child
            mgr.running.lock().unwrap().insert(id, Running { child_id, owned });
            // Task 6 shows the webview here.
            Ok(())
        }
        StartOutcome::Error(e) => Err(e),
    }
}

fn images_present(m: &crate::plugins::manifest::Manifest, probe: &dyn crate::plugins::seams::Probe) -> bool {
    // Cheap heuristic: if health already answers, the stack/images exist.
    probe.probe(&m.lifecycle.health.url).is_some()
}
```

> **Note on `child_id`:** `run_start_sequence` spawns internally via the `Spawner`, but the current signature doesn't return the child id to the caller. Before wiring teardown (Task 4.3), extend `StartOutcome::Healthy` to carry `child_id: Option<u64>` (None for adopted/not-owned), thread it out of `run_start_sequence`, and update the Phase-3 tests' `StartOutcome::Healthy { owned: .. }` patterns to `StartOutcome::Healthy { owned: .., child_id: .. }`. Do this as the first edit in Task 4.3 (it's a small, mechanical signature change with the tests already in place to catch regressions).

Add `pub mod manager;` to `mod.rs`.

- [ ] **Step 2: Register the manager + commands in the builder**

In `cockpit/ui/src-tauri/src/lib.rs`, update the builder:

```rust
        .plugin(tauri_plugin_shell::init())
        .manage(plugins::manager::PluginManager::default())
        .invoke_handler(tauri::generate_handler![
            plugins::manager::plugins_list,
            plugins::manager::plugin_launch,
        ])
```

- [ ] **Step 3: Verify build**

Run: `cd cockpit/ui/src-tauri && cargo build`
Expected: clean build (after the Task-4.3 signature note is applied, or temporarily ignore the unused `child_id`).

- [ ] **Step 4: Commit**

```bash
git add cockpit/ui/src-tauri/src/plugins/manager.rs cockpit/ui/src-tauri/src/plugins/mod.rs cockpit/ui/src-tauri/src/lib.rs
git commit -m "feat(app-plugins): PluginManager + plugins_list/plugin_launch commands"
```

### Task 4.3: Blocking teardown on ExitRequested

**Files:**
- Modify: `cockpit/ui/src-tauri/src/plugins/manager.rs`, `cockpit/ui/src-tauri/src/plugins/state.rs` (signature change), `cockpit/ui/src-tauri/src/lib.rs`

- [ ] **Step 1: Thread `child_id` out of the start sequence**

In `state.rs`, change `StartOutcome::Healthy { owned: bool }` to `Healthy { owned: bool, child_id: Option<u64> }`. In `run_start_sequence`: capture `let child = spawner.spawn(...)` and return `child_id: Some(child)` on the spawned path; `child_id: None` on the adopted path. Update all Phase-3 test patterns accordingly. Run `cargo test plugins::state` — Expected: PASS (proves the mechanical change is safe).

- [ ] **Step 2: Add a `stop_all_owned` method with a total deadline**

In `manager.rs`:

```rust
use crate::plugins::manifest::Manifest;

impl PluginManager {
    /// Blocking teardown of every OWNED plugin, concurrently, under a single
    /// total deadline. Adopted (not-owned) stacks are left running.
    pub fn stop_all_owned(&self, total_deadline_ms: u64) {
        let running = { std::mem::take(&mut *self.running.lock().unwrap()) };
        let discovered = self.discovered.lock().unwrap().clone();
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(total_deadline_ms);

        let handles: Vec<_> = running.into_iter().filter(|(_, r)| r.owned).filter_map(|(id, r)| {
            let m = discovered.iter().find(|d| d.manifest.id == id).map(|d| (d.manifest.clone(), d.dir.clone()))?;
            Some(std::thread::spawn(move || stop_one(&m.0, &m.1, r.child_id)))
        }).collect();

        for h in handles {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() { break; }
            let _ = h.join(); // best-effort within the budget
        }
        // Any thread still running past the deadline is abandoned; its `stop`/kill
        // is fire-and-forget at that point (documented known-gap fallback).
    }
}

fn stop_one(m: &Manifest, dir: &std::path::PathBuf, _child_id: Option<u64>) {
    if let Some(stop) = &m.lifecycle.stop {
        let cwd = m.resolved_cwd(dir);
        let mut parts = stop.split_whitespace();
        if let Some(prog) = parts.next() {
            let _ = std::process::Command::new(prog).args(parts).current_dir(&cwd).status();
        }
    }
}
```

- [ ] **Step 3: Call it from `RunEvent::ExitRequested` in the builder**

Replace the `.run(...)` tail in `lib.rs` so the app builds a handle and intercepts exit:

```rust
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                api.prevent_exit();
                let mgr = app_handle.state::<plugins::manager::PluginManager>();
                mgr.stop_all_owned(30_000); // total budget; kept under the OS force-kill ceiling
                app_handle.exit(0);
            }
        });
```

- [ ] **Step 4: Verify build**

Run: `cd cockpit/ui/src-tauri && cargo build`
Expected: clean build.

- [ ] **Step 5: Commit**

```bash
git add cockpit/ui/src-tauri/src/plugins/ cockpit/ui/src-tauri/src/lib.rs
git commit -m "feat(app-plugins): blocking concurrent teardown of owned stacks on exit"
```

---

## Phase 5 — Switcher shell (Svelte) + pure-logic tests

> The switcher and its state→chip mapping are TDD-able pure logic; the reserved-rect placeholder is wired here but the webview attaches in Phase 6. Adds Vitest (the repo has no JS test runner yet).

### Task 5.1: Add Vitest + the pure plugins module

**Files:**
- Modify: `cockpit/ui/package.json`
- Create: `cockpit/ui/vitest.config.ts`
- Create: `cockpit/ui/src/lib/plugins.ts`
- Create: `cockpit/ui/src/lib/plugins.test.ts`

- [ ] **Step 1: Add Vitest + a `test` script**

In `cockpit/ui/package.json`, add to `devDependencies`: `"vitest": "^2.1.0"`, and to `scripts`: `"test": "vitest run"`. Then:

Run: `cd cockpit/ui && npm install`
Expected: vitest installed.

- [ ] **Step 2: Vitest config**

Create `cockpit/ui/vitest.config.ts`:

```ts
import { defineConfig } from 'vitest/config';
export default defineConfig({ test: { environment: 'node', include: ['src/**/*.test.ts'] } });
```

- [ ] **Step 3: Write the failing test (state→chip mapping + types)**

Create `cockpit/ui/src/lib/plugins.test.ts`:

```ts
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

  it('marks error and healthy as terminal-ish for styling', () => {
    expect(chipFor('error')!.tone).toBe('bad');
    expect(chipFor('healthy')!.tone).toBe('ok');
    expect(chipFor('building')!.tone).toBe('busy');
  });
});
```

- [ ] **Step 4: Run to confirm failure**

Run: `cd cockpit/ui && npm run test`
Expected: FAIL — `./plugins` not found.

- [ ] **Step 5: Implement the pure module**

Create `cockpit/ui/src/lib/plugins.ts`:

```ts
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
```

- [ ] **Step 6: Run to confirm pass**

Run: `cd cockpit/ui && npm run test`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add cockpit/ui/package.json cockpit/ui/package-lock.json cockpit/ui/vitest.config.ts cockpit/ui/src/lib/plugins.ts cockpit/ui/src/lib/plugins.test.ts
git commit -m "feat(app-plugins): pure plugins module + Vitest state→chip tests"
```

### Task 5.2: Switcher component + reserved rect in App.svelte

**Files:**
- Create: `cockpit/ui/src/lib/Switcher.svelte`
- Modify: `cockpit/ui/src/App.svelte`

- [ ] **Step 1: Build the switcher component**

Create `cockpit/ui/src/lib/Switcher.svelte`:

```svelte
<script lang="ts">
  import { onMount } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import { listPlugins, launchPlugin, chipFor, type PluginMeta, type PluginState } from './plugins';

  let { active = $bindable('fleet') }: { active: string } = $props();
  let plugins = $state<PluginMeta[]>([]);
  let states = $state<Record<string, PluginState>>({});

  onMount(async () => {
    plugins = await listPlugins();
    return listen<{ id: string; state: PluginState }>('plugin://state', (e) => {
      states[e.payload.id] = e.payload.state;
    });
  });

  async function select(id: string) {
    active = id;
    if (id !== 'fleet' && states[id] !== 'healthy') await launchPlugin(id);
  }
</script>

<nav class="switcher mono">
  <button class:on={active === 'fleet'} onclick={() => select('fleet')}>FLEET</button>
  {#each plugins as p (p.id)}
    {@const chip = chipFor(states[p.id] ?? 'stopped')}
    <button class:on={active === p.id} onclick={() => select(p.id)}>
      {p.name}
      {#if chip}<span class="chip" class:ok={chip.tone === 'ok'} class:bad={chip.tone === 'bad'} class:busy={chip.tone === 'busy'}>{chip.label}</span>{/if}
    </button>
  {/each}
</nav>

<style>
  .switcher { display: flex; gap: 6px; align-items: center; }
  .switcher button { background: transparent; border: 1px solid #2a2f3a; color: #aeb6c2; padding: 4px 10px; cursor: pointer; }
  .switcher button.on { color: #fff; border-color: #4a90d9; }
  .chip { margin-left: 6px; font-size: 10px; padding: 1px 4px; border-radius: 3px; }
  .chip.ok { color: #6ee7a8; } .chip.bad { color: #f08a8a; } .chip.busy { color: #e0c46e; }
</style>
```

- [ ] **Step 2: Mount the switcher + a reserved rect in `App.svelte`**

In `App.svelte`: import the switcher and a rect helper; add `let active = $state('fleet')` and `let rectEl: HTMLDivElement`. Put `<Switcher bind:active />` in the topbar (after the `.brand` block). Wrap the existing `<main class="grid">` so it only shows when `active === 'fleet'`, and add a sibling placeholder shown otherwise:

```svelte
  {#if active === 'fleet'}
    <main class="grid"> … existing grid unchanged … </main>
  {:else}
    <div class="app-rect" bind:this={rectEl}></div>
  {/if}
```

Add a `ResizeObserver` + `$effect` that, while `active !== 'fleet'`, reads `rectEl.getBoundingClientRect()` and calls `invoke('plugin_set_rect', { id: active, x, y, width, height })` (command stubbed in Phase 6) on mount, resize, and `active` change. Emit an `overlay-open`/`overlay-close` via `invoke('plugin_overlay', { open })` when any global modal toggles (wire to the existing modal state if present; otherwise leave a single call site for Phase 6).

```css
  .app-rect { position: absolute; inset: var(--topbar-h, 56px) 0 0 0; }
```

- [ ] **Step 3: Verify the shell compiles + type-checks**

Run: `cd cockpit/ui && npm run check`
Expected: no Svelte/TS errors. (The `plugin_set_rect`/`plugin_overlay` invokes resolve at runtime in Phase 6; `check` only type-checks the TS.)

- [ ] **Step 4: Commit**

```bash
git add cockpit/ui/src/lib/Switcher.svelte cockpit/ui/src/App.svelte
git commit -m "feat(app-plugins): top-level switcher + reserved app rect (Fleet stays in-DOM)"
```

---

## Phase 6 — Embedding (B path) + wire Audience end-to-end (smoke)

> Uses the **exact** child-webview API recorded in `spikes/SPIKE-RESULTS.md`. This layer is spike-and-smoke per the spec — verification is a manual checklist in dev and packaged, not unit tests. **If the spike decided no-go ("C"), implement Task 6.1 as separate `WebviewWindow`s instead** (create-on-launch, raise-on-select, per-window modals) — every other phase is unchanged.

### Task 6.1: Embedding commands (create/position/show/hide/destroy)

**Files:**
- Create: `cockpit/ui/src-tauri/src/plugins/embed.rs`
- Modify: `cockpit/ui/src-tauri/src/plugins/mod.rs`, `cockpit/ui/src-tauri/src/plugins/manager.rs`, `cockpit/ui/src-tauri/src/lib.rs`

- [ ] **Step 1: Implement the four commands using the spike's exact API**

Create `embed.rs` with `plugin_set_rect`, `plugin_show`, `plugin_hide`, `plugin_overlay`, and an internal `destroy(id)`. Use the precise Tauri 2.11 `unstable` `WebviewBuilder`/positioning calls from the results file. Skeleton (fill the webview calls from the spike):

```rust
use tauri::{AppHandle, Manager, State};
use crate::plugins::manager::PluginManager;

fn label(id: &str) -> String { format!("app::{id}") }

#[tauri::command]
pub fn plugin_show(app: AppHandle, mgr: State<'_, PluginManager>, id: String, x: f64, y: f64, width: f64, height: f64) -> Result<(), String> {
    let url = mgr.url_for(&id).ok_or("unknown plugin")?;
    // If the webview `label(&id)` exists → reposition + show; else create it at `url`
    //   over the rect (x,y,width,height) on the main window. EXACT calls per SPIKE-RESULTS.md.
    Ok(())
}

#[tauri::command]
pub fn plugin_hide(app: AppHandle, id: String) -> Result<(), String> { /* hide webview label(&id) */ Ok(()) }

#[tauri::command]
pub fn plugin_set_rect(app: AppHandle, id: String, x: f64, y: f64, width: f64, height: f64) -> Result<(), String> { /* reposition+resize */ Ok(()) }

/// Shell signals a global overlay opened/closed → hide/restore the active app webview
/// (CSS z-index can't stack a DOM modal over a native webview).
#[tauri::command]
pub fn plugin_overlay(app: AppHandle, active_id: Option<String>, open: bool) -> Result<(), String> {
    if let Some(id) = active_id { if open { plugin_hide(app, id)?; } else { /* re-show at last rect */ } }
    Ok(())
}
```

Add a `url_for(&self, id)` helper to `PluginManager` (look up `discovered`), `pub mod embed;` to `mod.rs`, and register the four commands in the `generate_handler!` list in `lib.rs`. On `healthy`, have `plugin_launch` (or the shell, via `plugin_show`) attach the webview; on crash→error, call `destroy(id)` so the next launch recreates it (per §3/§4).

- [ ] **Step 2: Verify build + type-check**

Run: `cd cockpit/ui/src-tauri && cargo build` and `cd cockpit/ui && npm run check`
Expected: both clean.

- [ ] **Step 3: Commit**

```bash
git add cockpit/ui/src-tauri/src/plugins/embed.rs cockpit/ui/src-tauri/src/plugins/mod.rs cockpit/ui/src-tauri/src/plugins/manager.rs cockpit/ui/src-tauri/src/lib.rs
git commit -m "feat(app-plugins): child-webview embedding commands (show/hide/set_rect/overlay)"
```

### Task 6.2: Audience manifest + end-to-end smoke

**Files:**
- Create: `~/.command-center/app-plugins/audience/app-plugin.json` (user dir; not committed)
- Create: `docs/superpowers/SMOKE-app-plugins.md` (the smoke checklist)

- [ ] **Step 1: Write Audience's manifest into the user plugins dir**

Create `~/.command-center/app-plugins/audience/app-plugin.json` exactly as in spec §2 (with the real local `cwd`, `build.args` = `NODE_ENV=development`/`AI_PROVIDER=fake`/`MEDIA_PROVIDER=fake`, health `:8080/health` okStatus [200], ready `:3000` okStatus [200,301,302,307,308]).

- [ ] **Step 2: Cold-launch smoke (dev)**

From a clean state (`docker ps` shows no Audience containers):

Run: `cd cockpit/ui && npm run desktop`, then click the **Audience** tab.
Expected, in order: chip shows `BUILDING` (first run) → `STARTING` → `HEALTH` → `READY?` → `LIVE`; then Audience renders in the reserved rect. Record pass/fail in `docs/superpowers/SMOKE-app-plugins.md`.

- [ ] **Step 3: Switch + keep-alive smoke**

Switch to **Fleet** (the ops grid shows, unchanged) and back to **Audience** (renders instantly, no re-cold-start). Open a global overlay if one exists → webview hides → close → restores at the right rect. Record pass/fail.

- [ ] **Step 4: Teardown smoke (the orphan gate)**

Quit the cockpit window.

Run: `docker ps`
Expected: **no Audience containers running** (blocking teardown worked). Record pass/fail.

- [ ] **Step 5: Adopt smoke**

Manually `docker compose -f docker-compose.prod.yml up -d` Audience by hand, then launch the cockpit and click Audience.
Expected: it **adopts** (no second stack; chip goes straight toward `LIVE`), and on quit the hand-started (not-owned) stack is **left running** (`docker ps` still shows it). Record pass/fail.

- [ ] **Step 6: Packaged smoke**

Run: `cd cockpit/ui && npm run tauri build`, launch the bundle, repeat steps 2–4.
Expected: same behavior packaged as in dev. Record pass/fail.

- [ ] **Step 7: Commit the smoke record**

```bash
git add docs/superpowers/SMOKE-app-plugins.md
git commit -m "test(app-plugins): end-to-end smoke checklist results (dev + packaged)"
```

### Task 6.3: Final verification + review

- [ ] **Step 1: Full Rust test suite**

Run: `cd cockpit/ui/src-tauri && cargo test`
Expected: all manifest / discovery / state tests PASS.

- [ ] **Step 2: Front-end tests + type-check**

Run: `cd cockpit/ui && npm run test && npm run check`
Expected: PASS, no type errors.

- [ ] **Step 3: Regression guard — ops grid untouched**

Confirm the Fleet tab renders the original ops grid and a mission can still be launched against fleetd (the canary).

- [ ] **Step 4: Request code review**

Use `superpowers:requesting-code-review` before merging `feat/app-plugins`.

---

## Self-Review (run by the plan author after writing)

**Spec coverage:**
- §2 Manifest (id/name/apiVersion/icon/url/lifecycle/webview, two probes, okStatus, build-vs-env, cwd resolution, discovery, versioning) → Tasks 2.1–2.3. ✓
- §3 Lifecycle (state machine, build state, adopt-both-probes, partial fall-through, timeout→error, crash→error, blocking teardown, fixed-ports caveat) → Tasks 3.2–3.4, 4.2–4.3. ✓ (port-conflict surfaces as the health/ready timeout→error path; no dedicated task needed.)
- §4 Embedding (switcher, chips, reserved rect, set_rect/show/hide, hide-on-overlay, keep-alive, navigation matrix, "C" fallback) → Tasks 5.2, 6.1; "C" called out in Phase 6 preamble. ✓
- §5 Trust (trusted first-party, arbitrary shell from Rust, CSP-per-app, no bridge) → realized by construction (Rust-side spawn in 4.1; per-app webview origin in 6.1); no isolation code by design. ✓
- §6 Spike + build order + testing (four seams, TDD cores, smoke for the seam) → Phase 0; build order maps to Phases 1–6; four seams in 3.1; smoke in 6.2. ✓

**Placeholder scan:** Phase 6 webview calls intentionally defer to `SPIKE-RESULTS.md` — this is *deferral to the spike that exists to discover the OS API*, not a TODO; every other step carries concrete code/commands. The `child_id` signature evolution is explicitly sequenced (note in 4.2, executed first in 4.3) rather than left vague.

**Type consistency:** State strings are the seven canonical values everywhere (Rust consts in `state.rs`, the `PluginState` union in `plugins.ts`). `StartOutcome::Healthy` gains `child_id` in 4.3 with the Phase-3 tests updated in the same step. `chipFor`/`PluginState`/`PluginMeta` names match between `plugins.ts` and `plugins.test.ts`/`Switcher.svelte`. Commands (`plugins_list`, `plugin_launch`, `plugin_show`, `plugin_hide`, `plugin_set_rect`, `plugin_overlay`) match between Rust `#[tauri::command]` and the TS `invoke` callers.
