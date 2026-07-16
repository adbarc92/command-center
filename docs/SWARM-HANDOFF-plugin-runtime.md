# Swarm Handoff — Cockpit Plugin Runtime (view-plugins + app-plugins)

> **Companion to** [`docs/ROADMAP.md`](ROADMAP.md) and the two design specs
> ([view-plugins](superpowers/specs/2026-06-07-view-plugins-design.md),
> [app-plugins](superpowers/specs/2026-06-07-app-plugins-design.md)).
> **Date:** 2026-07-16. **Unblocked by:** P3 (app-plugin child-webview) **and** P4 (view-plugin
> handshake) spikes both **GO** — see [`spikes/SPIKE-RESULTS.md`](../spikes/SPIKE-RESULTS.md) and
> `spikes/SPIKE-RESULTS-app-plugins.md`.
>
> This is a *different* swarm from [`docs/SWARM-HANDOFF.md`](SWARM-HANDOFF.md) (that one was the
> roadmap-items swarm, which explicitly excluded the plugin build until the spikes passed).

## Kickoff for a fresh context (read this first)

You've been handed this cold to **dispatch the plugin-runtime swarm**. Every lane brief is
self-contained.

1. **Read the "Honest dependency analysis" below before dispatching.** This is *not* a clean
   3-way fan-out — it's two internally-serial feature cores plus a shell/config owner that
   integrates last. Dispatching 3 agents that all edit `App.svelte` would collide hard.
2. Invoke the `swarm-handoff` skill (its Part-I method). **Dispatch Lanes V and A concurrently**
   (worktree-isolated), then **Lane S last** (it owns the shared shell + Tauri config and
   assembles the others' contract requests).
3. Give each agent its lane brief *verbatim* plus the Rules of the Road (in `swarm-handoff`).
4. Dispatching a swarm is **expensive / opt-in** — confirm the user wants to spend it first.
5. Then run the Integration plan at the bottom.

---

## ⚠️ Honest dependency analysis (why the shape is V + A + S, not 3 equal lanes)

The two features are **not cleanly independent**, and each is **internally serial**:

- **View-plugin runtime** build order is a chain: `store.svelte.ts` → `bridge.ts` → host overlay
  → loader + scheme + view-switcher → reference plugin. Each step needs the previous.
- **App-plugin embedding** build order is a chain: manifest → lifecycle manager → embedding +
  shell coordination → Audience wiring.
- **Three hot files are written by BOTH features:**
  `cockpit/ui/src/App.svelte` (both add a topbar switcher + swap the content region),
  `cockpit/ui/src-tauri/src/lib.rs` (view-plugins register the `ccplugin://` scheme; app-plugins
  register `plugin_show/hide/set_rect`), and
  `cockpit/ui/src-tauri/tauri.conf.json` (view-plugins set the host CSP; app-plugins add
  capabilities/unstable).

So the safe decomposition is **two feature-core lanes with zero owned-file overlap** (each
builds its own new files + unit tests, and **files contract requests** for the three shared
files) **plus one dedicated Shell/Tauri-integration owner lane (Lane S)** that writes those three
files once, unifying both plugin kinds, and integrates last. The feature lanes' cores are
**unit-testable in isolation** (that's what makes them parallelizable); the real end-to-end
embedding + dogfooding is a **spike-and-smoke** step done at integration by Lane S — which matches
both specs (the app-plugins spec calls embedding "not meaningfully unit-testable"; the view-plugins
reference-plugin dogfood needs the shell).

## Dependency graph

```
Run concurrently (worktree-isolated, zero owned-file overlap):
    Lane V — view-plugin runtime core   (src/lib/* new files, plugin-sdk, plugins/reference)
    Lane A — app-plugin embedding core  (src-tauri/src/plugins/*, capabilities, Cargo.toml)

Integrates last (single writer of the shared shell + Tauri config):
    Lane S — shell & Tauri-integration owner  (App.svelte, src-tauri/src/lib.rs, tauri.conf.json)

Excluded / out of scope this swarm:
    Battlefield game-skin (Spec-B) — depends on the runtime being done (follow-on cycle)
```

## Shared contracts (single-owner)

| Shared file | Owner | V requests | A requests |
|---|---|---|---|
| `cockpit/ui/src/App.svelte` | **Lane S** | view-switcher entry + in-DOM sandboxed-iframe content-swap | topbar switcher `[Fleet][Audience][+]` + reserved-rect placeholder + `ResizeObserver` rect-emit + `overlay-open/close` signal + Fleet-stays-in-DOM |
| `cockpit/ui/src-tauri/src/lib.rs` | **Lane S** | `ccplugin://` scheme registration + plugin-doc CSP **response header** (carry the proven `spike_view_plugins.rs` code — incl. `Access-Control-Allow-Origin: *` and `script-src … http://ccplugin.localhost`) | `plugin_show`/`plugin_hide`/`plugin_set_rect` command bodies + `app::<id>` webview-label scheme + registration |
| `cockpit/ui/src-tauri/tauri.conf.json` | **Lane S** | host CSP `frame-src http://ccplugin.localhost` | capabilities ref / any webview config |

`capabilities/default.json` and `src-tauri/Cargo.toml` are **app-plugin-only** → owned directly by
Lane A (not shared). Feature lanes submit contract requests as **exact snippets** (Rust bodies,
Svelte glue, config-key deltas) so Lane S can paste-and-reconcile in one write.

---

### Lane V — View-plugin runtime core   ·   ✅ ready
- **Spec:** [`view-plugins-design.md`](superpowers/specs/2026-06-07-view-plugins-design.md) build
  order steps **2–5** (step 1 spike is done; **skip step 6 Battlefield** — separate cycle).
- **Goal:** the sandboxed-iframe view-plugin runtime — store extraction (single command sink),
  MessagePort bridge + command policy, host overlay (oracle approval), plugin SDK, loader, and a
  reference plugin that exercises the full message surface.
- **Owns (exclusive write):** `cockpit/ui/src/lib/store.svelte.ts` (new), `cockpit/ui/src/lib/bridge.ts`
  (new), `cockpit/ui/src/lib/loader.ts` (new), `cockpit/ui/src/lib/overlay/**` (new),
  `cockpit/plugin-sdk/**`, `plugins/reference/**`, plus their tests.
- **Reads (no write):** `cockpit/ui/src/lib/{api.ts,fleet.ts,types.ts}`, `cockpit/ui/src/App.svelte`,
  `spikes/SPIKE-RESULTS.md` (P4 findings).
- **Shared contract:** files requests to **Lane S** for the App.svelte / lib.rs / tauri.conf.json
  entries in the table above. Does **not** write those files.
- **Internal order (one agent, serial):** `store.svelte.ts` (+ regression: a bridge `launch`
  concurrent with `reconnect()` yields **exactly one unit + one socket**) → `bridge.ts`
  (plugin-hello→init→ready 100× no-drop; dirty-delta `state`; `log-append`/`log-reset`;
  **command policy** — shape/authority/cost/rate + inbound-flood-kill; `command-ack`) → `overlay/`
  (oracle-approval modal, focus-stealing) → `plugin-sdk` + `loader` + **reference plugin**.
- **Done when:** unit tests green for store (fold/single-sink), bridge (handshake/policy/ack),
  policy (reject unknown-type/unknown-id/over-bound `task`/`approve_oracle`; flood→kill);
  reference plugin exercises the full surface (dirty-`state`, `log-append`, a policed `launch`, a
  `command-ack` rejection, presence during `awaiting_oracle_approval`); `npm run check` + `npm test`
  green. **End-to-end dogfood is deferred to integration (needs Lane S shell).**
- **Verify:** `cd cockpit/ui && npm run check && npm test` → all green; new store/bridge/policy tests present.
- **Notes / open questions:** carry the **P4 findings** into the production scheme handler you hand
  to Lane S — it MUST serve plugin assets with `Access-Control-Allow-Origin: *` (opaque-origin
  module scripts fetch in CORS mode) and name `http://ccplugin.localhost` in `script-src` (opaque
  origin ⇒ `'self'` matches nothing). Store extraction touches the **live reconnect/socket path** +
  the Svelte-5 `.svelte.ts` reactivity rule — regression-test it before the bridge. The spike files
  (`spike_view_plugins.rs`, `SpikeViewPlugins.svelte`, `spike-view-plugin/*`) are **throwaway
  references**, not code to merge.

### Lane A — App-plugin embedding core   ·   ✅ ready
- **Spec:** [`app-plugins-design.md`](superpowers/specs/2026-06-07-app-plugins-design.md) build
  order **0–2** owned here; step **3** (embedding + shell coordination) delivered as a contract
  request to Lane S; step **4** (Audience e2e) at integration.
- **Goal:** trusted child-webview app plugins — manifest + discovery, the lifecycle manager
  (spawn/probe/stop/adopt-don't-respawn), the Rust embedding commands, wired to Audience.
- **Owns (exclusive write):** `cockpit/ui/src-tauri/src/plugins/**` (extend the existing
  `manifest.rs`/`state.rs`/`manager.rs`/`seams.rs` scaffolds), `cockpit/ui/src-tauri/capabilities/default.json`
  (webview perms — app-plugin-only), `cockpit/ui/src-tauri/Cargo.toml` (`tauri = { features =
  ["unstable"] }` + **pin the exact Tauri version**), app-plugin manifest files, Audience dev-auth wiring.
- **Reads (no write):** `cockpit/ui/src-tauri/src/lib.rs` (sidecar-babysitter precedent),
  `tauri.conf.json`, `App.svelte`, `docs/digests/audience-digest.md`, `spikes/SPIKE-RESULTS-app-plugins.md`.
- **Shared contract:** files requests to **Lane S** for the App.svelte / lib.rs / tauri.conf.json
  entries in the table above — supplying the **exact Rust command bodies + Svelte glue snippets**
  (the spike proved the Rust+shell pair; carry that glue forward rather than re-deriving it).
- **Internal order (one agent, serial):** manifest + discovery (pure unit) → **lifecycle manager**
  with injectable seams **Clock / Probe / Spawner / EventSink** (pure-unit: transitions,
  timeout→error, partial-stack adopt, crash→error) → embedding command bodies (from the P3 spike) as
  the contract-request bundle → Audience manifest + credential-free dev path.
- **Done when:** `cargo test` green for manifest (parse valid/invalid, `apiVersion` refusal,
  discovery union/precedence) + lifecycle (state machine via the four fake seams); the embedding
  command bodies + shell glue are delivered as a contract-request bundle to Lane S. **Real embedding
  smoke is deferred to integration.**
- **Verify:** `cd cockpit/ui/src-tauri && cargo test` → green; manifest + lifecycle unit tests present.
- **Notes / open questions:** **P3 finding** — webview create/show MUST run **off the main thread**
  (async commands) or it deadlocks (`spike_show` hang, fixed by async); `hide()`/`show()` forces a
  repaint/reload, so **park inactive webviews off-screen + warm-pool LRU**, don't destroy. The
  `unstable` Tauri feature is **not semver-stable** — pin the version + note in upgrade docs.
  Fallback **"C"** (a separate `WebviewWindow` per app) is a *likely* outcome — the build order
  isolates the embedding surface so choosing C changes only that layer. Webview-label scheme:
  `app::<id>`, stable across relaunch/adopt; the capability glob must match it.

### Lane S — Shell & Tauri-integration owner   ·   ⏳ integrates last
- **Goal:** be the **single writer** of the three shared files; unify the topbar switcher across
  **both** plugin kinds, register both Rust surfaces, set both config keys, and run the dev +
  packaged smoke gates.
- **Owns (exclusive write):** `cockpit/ui/src/App.svelte`, `cockpit/ui/src-tauri/src/lib.rs`,
  `cockpit/ui/src-tauri/tauri.conf.json`.
- **Reads (no write):** both design specs; Lane V's + Lane A's contract-request bundles; the P4
  spike harness (`SpikeViewPlugins.svelte`, `spike_view_plugins.rs`) + P3 spike glue as **reference
  implementations** to carry forward.
- **Shared contract:** *is* the owner of all three; no other lane writes them.
- **Depends on / blocks:** depends on V + A returning their cores + bundles → **S integrates last.**
  Can start early: scaffold the unified switcher skeleton + publish the request schema (each lane
  submits: switcher entry, Rust registration snippet, config-key delta).
- **Done when:** topbar switcher shows **Fleet** (in-DOM, default, untouched ops grid) + view-plugins
  (sandboxed iframe) + app-plugins (child webview) + `[+ …]` (discovered-not-launched); `lib.rs`
  registers the `ccplugin://` scheme (with ACAO + CSP header) **and** `plugin_show/hide/set_rect`;
  `tauri.conf.json` holds the host CSP (`frame-src http://ccplugin.localhost`) + capabilities; the
  Fleet ops grid behaves **unchanged** (regression canary); **dev AND packaged smoke pass** (switcher
  Fleet→app→Fleet with no leak, hide-on-overlay works across the native boundary, the reference
  view-plugin dogfoods end-to-end).
- **Verify:** `cd cockpit/ui && npm run build && npm run check` → clean; manual smoke checklist in
  dev (`npm run desktop`) **and** packaged (`npm run bundle` → run `target/release/app.exe`) — record
  results; existing ops-grid tests still green.
- **Notes / open questions:** a Tauri child webview **composites over** the host webview as a native
  OS view — **`z-index` does NOT cross the boundary.** Host overlays (modals, command palette, the
  switcher's own dropdown) must trigger a **Rust hide-on-overlay** of the active app webview, not
  stacking. If V and A both request a switcher entry, **unify** them into one switcher component
  that handles both plugin kinds (this unification is the main design judgement in this lane).

---

## Integration plan

1. **Lanes V and A run concurrently** (worktree-isolated; zero owned-file overlap). Each produces
   its unit-tested core **plus a contract-request bundle** for Lane S (exact snippets).
2. **Lane S integrates last:** builds the unified topbar switcher, registers both Rust surfaces in
   `lib.rs`, sets both `tauri.conf.json` keys — assembling V's and A's bundles in single writes.
3. **Reconcile (orchestrator):** `npm run build && npm run check` clean; `cargo test` green;
   dev + packaged smoke with **both** plugin kinds live and the ops grid unchanged; record the
   result (append to `spikes/SPIKE-RESULTS.md`). Note any lane still blocked and what unblocks it.

## Pre-steps & ground rules

- **Base each lane off `main`.** The stale `feat/view-plugins` branch referenced in older notes was
  **merged + deleted during the 2026-07-16 cleanup** — there is nothing to de-stale; start fresh.
- **Spike code is throwaway.** `spike_view_plugins.rs`, `SpikeViewPlugins.svelte`, the `App.svelte`
  `⌬ VP SPIKE` launcher block, and `spike-view-plugin/*` are **reference material** — lanes carry
  the *findings* into production code; they do **not** merge the spike files. (The `.taurignore`
  fleet.db-ignore lives on `main` already and stays.)
- **Worktree-isolate all three lanes** (they mutate repo files).

## Blocked / out of scope

- **Battlefield game-skin (Spec-B, `plugins/battlefield/`)** — depends on the runtime being done;
  a follow-on cycle. The "one cycle vs two" boundary (Spec-A alone vs A+B) is a **review-time**
  call, not a lane.
- **`fleetd` has no API auth** — a broader hardening item; sandboxed view-plugins have no network
  so it does not block this swarm (app-plugins are trusted first-party by design).
