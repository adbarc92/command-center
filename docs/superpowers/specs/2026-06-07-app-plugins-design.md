# App Plugins — Design Spec

> Status: design approved (brainstorming complete), pre-implementation.
> Date: 2026-06-07. Branch context: `feat/view-plugins` (this is the **app-plugins** slice — a
> different SP5 feature from view-plugins; see "Relationship to view-plugins" below).
> Parent: `docs/command-center-vision.md` (SP5 = view plugins + **app plugins** + mobile/remote).
> Proving plugin this cycle: **Audience**.

## 1. Goal & scope

Host *other whole apps* (first-party web apps) inside the command-center dashboard so the
operator works from one window: the fleet ops grid plus launchable apps like Audience, switched
from a top-level switcher.

**In scope (this cycle):**
- A uniform plugin kind: **every app plugin is a web app served at a URL** (it must have a head).
- **Host-managed lifecycle:** the Rust side starts/stops each app's backend, health-checks it,
  then shows its UI.
- **Child-webview runtime:** each app renders in a Tauri v2 child webview (its own origin).
- **Top-level switcher:** `[Fleet] [Audience] [+ …]`; the existing ops grid is the default
  entry and stays in-DOM.
- **Audience** as the single proving plugin.

**Out of scope (roadmap — see §5):** headless apps, third-party plugins & isolation, a host↔app
data bridge, production auth (Clerk), secrets management, mobile/remote. **Halyard is deferred**
(headless today — no web head; a separate brief covers giving it one).

**Relationship to view-plugins.** View-plugins are sandboxed iframe *renderers* over fleet
state, fed by a MessagePort bridge. App-plugins are different: they host *entire external apps*
with their own backends and origins, and deliberately have **no** data bridge. Same SP5 vision,
separate slices. This spec does not touch view-plugins.

### Locked decisions (do not relitigate)
1. **App form:** first-party local apps now (we define the contracts); third-party later.
2. **One plugin kind:** every app plugin is a web app served at a URL. Headless host-rendered
   apps were considered and **cut** — all apps must have a head.
3. **Embedding primitive = Tauri v2 child webview** (approach "B"): a real top-level browsing
   context (own origin → cookies/popups/network work), positioned/shown by Rust over a content
   rect the Svelte shell reserves. **Fallback = separate `WebviewWindow` per app ("C")** if
   overlay/z-order proves too fiddly. Plain `<iframe>` ("A") rejected — fights Audience's
   cookie/popup/origin needs. ⚠️ Child webviews require Tauri's `unstable` Cargo feature.
4. **Host manages lifecycle:** Rust starts/stops each app's backend (via `tauri-plugin-shell`,
   already a dep), health-polls, then shows the webview.
5. **Trust:** trusted first-party (real origin, network, cookies, popups). Isolation is a
   roadmap item. Arbitrary shell in the manifest is the accepted trust surface.
6. **No host↔app data bridge:** apps are self-contained; the host only frames/launches/manages.
7. **Shell navigation:** top-level switcher; the ops grid is the default entry (in-DOM, trusted).
8. **Halyard deferred; Audience proves the slice.**

## 2. Plugin manifest & contract

Each plugin ships an `app-plugin.json`:

```jsonc
{
  "id": "audience",
  "name": "Audience",
  "apiVersion": 1,                  // host refuses unsupported versions
  "icon": "icon.svg",
  "url": "http://localhost:3000",   // the head the host loads in a webview

  "lifecycle": {
    "managed": true,                // host starts/stops the backend
    "cwd": "D:/MajorProjects/CURRENT/audience",
    // First-run build (may take minutes): bakes the dev auth/origin posture into the images.
    // Audience's auth + API origin are BUILD-TIME constants (see "Auth & origin" below), so
    // devAuth/fake-providers must be set as build args, NOT runtime env.
    "build": {
      "cmd": "docker compose -f docker-compose.prod.yml build",
      "args": { "NODE_ENV": "development", "AI_PROVIDER": "fake", "MEDIA_PROVIDER": "fake" },
      "timeout": 1200000              // 20 min cold-build ceiling
    },
    "start": "docker compose -f docker-compose.prod.yml up",
    "stop":  "docker compose -f docker-compose.prod.yml down",
    "env": { "NODE_ENV": "development" },   // runtime env for non-baked services (api devAuth)
    // health = real readiness gate; ready = liveness of the web server only.
    "health": { "url": "http://localhost:8080/health", "okStatus": [200], "timeout": 180000, "interval": 1000 },
    "ready":  { "url": "http://localhost:3000",        "okStatus": [200, 204, 301, 302, 307, 308], "timeout": 180000, "interval": 1000 }
  },

  "webview": {                      // optional; host defaults applied when omitted
    "popups": "allow",              // "allow" → OAuth window.open opens a child webview sharing
                                    //   this app's session partition (so it can postMessage/poll
                                    //   its opener, which Audience's OAuth does every 2s/120s)
    "externalLinks": "in-app",      // "in-app" | "system-browser": top-level navigations to an
                                    //   external HTTPS origin (Stripe checkout) and back stay in
                                    //   the app webview ("in-app"); use "system-browser" to punt
                                    //   genuinely-leaving links to the OS browser
    "title": "Audience"             // default = name; only consumed by the "C" fallback window
  }
}
```

**Two probes, two distinct meanings.** `health` (Audience api `:8080/health`) is the **real
readiness gate** — a 200 means the backend the web UI depends on is live. `ready` (Audience web
`:3000`) is **liveness of the web server only** — "the server is serving," not "every page
renders." The host gates "show the webview" on **both** probes passing. A probe passes when the
response status is in its `okStatus` set; this matters because Audience's root `/`
**302-redirects to `/dashboard`** (digest), so the `ready` probe must accept 3xx, not just 2xx,
or a perfectly healthy Next.js server would be marked `error`. We do **not** try to assert "every
page renders correctly" — SSR pages can still surface an error state if a downstream fetch lags;
`health` (api up) is the strongest gate we can cheaply assert, and it precedes a usable UI.

**Build vs. runtime env — the critical Audience constraint.** Audience bakes both its **auth
posture and its API origin at Docker *build* time** (`next.config.mjs` inlines
`NEXT_PUBLIC_API_URL`; the prod compose passes auth/origin as build *args*). A runtime
`NODE_ENV=development` on `docker compose up` does **not** retroactively flip a prod-built image
to devAuth. Therefore the manifest separates **`build.args`** (baked: `NODE_ENV=development`,
`AI_PROVIDER=fake`, `MEDIA_PROVIDER=fake`) from **`env`** (runtime, for services like the api
whose `devAuth` is selected at process start). The host runs `build.cmd` on first launch (or when
images are absent) before `start`; this is a multi-minute, one-time cost reflected in
`build.timeout` and surfaced as a distinct `building` state (§3).

**Auth & origin precondition (proving cycle).** The child webview having a real origin and a
working cookie jar does **not** by itself solve Audience auth: Audience has **no login UI**, and
its client 401s on every authed call when the Clerk `__session` cookie is absent (digest). A
fresh webview has no such cookie and no way to obtain one. The *only* auth path this cycle is the
**devAuth build** above (api fabricates an identity from `DEV_WORKSPACE_ID`/`DEV_USER_ID`, no
cookie needed). Real Clerk auth is roadmap (§5). Equally, browser→api calls work only because web
(`:3000`) and api (`:8080`) are **same-site localhost as built**; Audience's Hono api registers
**no CORS**, so any future origin shift (hosted backend, `tauri://`/`app://` scheme) breaks all
authed fetches — a same-origin-as-built precondition, called out so the build doesn't assume
otherwise.

**`webview` block** is optional. All three fields have host defaults (`popups: "allow"`,
`externalLinks: "in-app"`, `title: name`), so a minimal plugin omits the block entirely. It
exists to handle Audience's two webview-fighting behaviors (OAuth `window.open` popups; Stripe
full-page redirects) and to title the "C" fallback window.

**Path resolution.** `cwd` (and any relative `build.cmd`/`start` paths) resolve **relative to the
manifest file's own directory** so a discovered/packaged plugin is portable; an absolute path is
allowed only as an escape hatch. The proving-cycle Audience manifest happens to use a
machine-specific absolute `cwd` (`D:/MajorProjects/CURRENT/audience`) — that's the escape hatch,
not the norm; path-resolution is a defined part of the contract, not an accident of one machine.

**Discovery seam** (mirrors view-plugins): dev list | packaged scan ∪
`~/.command-center/app-plugins/`. Anything in those locations is trusted by being there.

**Versioning:** the host supports a known set of `apiVersion` values and **refuses** a manifest
whose `apiVersion` it doesn't support (surfaced as a plugin in `error` state), rather than
guessing.

## 3. Lifecycle manager (Rust)

A `PluginManager` in `src-tauri` owns one `PluginProcess` per managed app. Single purpose: take
an app `stopped → visible-and-healthy`, and tear it back down. It generalizes the existing
`fleetd-serve` sidecar babysitter (`cockpit/ui/src-tauri/src/lib.rs`) from one fixed bundled
binary to N manifest-driven commands. Lifecycle commands are spawned **from Rust** via
`app.shell().command(prog, args)` — the JS `shell:allow-execute` allowlist
(`capabilities/default.json`) does not constrain Rust-side spawning, which is the accepted trust
surface (§5).

**State machine** (emitted to the shell as a `plugin://state` event):

```
stopped → building → starting → health-probing → ready-probing → healthy → (error | stopped)
   │ (images          └────────────────── error ◄──────────────────┘
   │  present)
   └──────────────────► starting (skip build)
```

`building` is its own state because Audience's first launch is a multi-minute Docker image build
(7 images + infra); folding it into `starting` would make the UI look hung. Skipped when
`build.cmd` is absent or images already exist.

**Canonical `plugin://state` enum (the single source of truth — §4 chips must use these exact
strings):** `stopped`, `building`, `starting`, `health-probing`, `ready-probing`, `healthy`,
`error`.

**Start sequence (per app, on user "launch"):**
0. If `build.cmd` is present and images are absent → `building`: run `build.cmd` with `build.args`
   up to `build.timeout`; non-zero exit → `error`.
1. **Adopt check:** probe **both** `health.url` and `ready.url` once. Adopt-and-skip-spawn only
   if **both** already pass (a fully-up stack); mark **not-owned**, go to step 4. If only `health`
   passes (a *partial* stack — e.g. after a crash that killed some of the 7 containers), do **not**
   adopt-and-show; fall through to step 2. (`docker compose up` is idempotent — it brings up only
   the missing services and leaves running ones alone — so re-running `start` over a partial stack
   is safe and is the reconcile action.)
2. Else spawn `lifecycle.start` with resolved `cwd` + `env`; pipe stdout/stderr to
   `log::info!`/`warn!` tagged with the plugin id (mirrors the sidecar loop). Mark **owned**.
3. **Health-probe** then **ready-probe** every `interval` ms until each passes (`okStatus`) or its
   `timeout` → `error`.
4. Both green → `healthy`; only then does §4 show the webview. The "adopt" path (step 1) reaches
   here only after the **same both-probes gate**, never on `health` alone.

**Stop / cleanup:**
- User stop, or app quit: run `lifecycle.stop`; if absent, kill the child process tree.
- **On app quit / window close** (`RunEvent::ExitRequested`): teardown is **blocking, not
  fire-and-forget** — `docker compose down` takes seconds, and Docker's daemon outlives the app,
  so an un-awaited `stop` orphans containers. The host calls `api.prevent_exit()`, then runs every
  *owned* app's `stop` **concurrently** (join-all) under a **single total deadline** (e.g. 30s,
  kept well under the OS/window-manager force-kill ceiling for a prevented exit — a real risk on
  Windows), falls back to `docker compose kill` / process-tree kill for any still running at the
  deadline, then exits explicitly. The deadline is a total budget, not per-app, so N owned apps
  don't serialize past the ceiling. (This cycle has one owned app; the concurrent bounded teardown
  is the contract for N>1.) We do **not** run `stop` for adopted (not-owned) stacks — the user
  started those by hand.
- **Force-quit / crash / OS shutdown is a known gap:** no handler can guarantee cleanup. Mitigated
  by adopt-don't-respawn (re-launch reuses the live stack) plus an **orphan-reconcile sweep** on
  next launch (probe known ports; offer to adopt or tear down).

**Edge cases:**
- **Adopt TOCTOU:** the adopt probe is a check-then-act race — a user could hand-start the stack
  *during* our start sequence. Acceptable for a single-user proving cycle; the **owned** flag is
  the sole guard against the host tearing down a hand-started stack, so it is set only when *we*
  spawned `start`.
- **Port conflict / start failure:** probe never passes before `timeout`, or child exits non-zero
  → `error` with the last stderr line, surfaced on the switcher chip.
- **Fixed ports / single-instance:** Audience hardcodes `:3000`/`:8080` (build-baked), so two
  app plugins on the same ports cannot coexist. Acceptable this cycle (one proving plugin); a
  port-allocation/remap story is roadmap (§5).
- **Crash while healthy:** the stdout/stderr task watches for `Terminated` → flips back to
  `error` so the chip updates live. On `healthy → error` the host also **destroys** the app's
  kept-alive webview (it's now bound to a dead `:3000`), so the next launch takes §4's "first
  show: create" path against the restarted backend rather than unhiding a stale webview.

## 4. Embedding & switcher

**Switcher (Svelte, in the topbar).** A row `[Fleet] [Audience] [+ …]`, each entry showing the
manifest `icon` + `name` + a **state chip** reusing the existing chip pattern (rate-limited /
awaiting-slot) to render the canonical `plugin://state` values (§3): `building` / `starting` /
`health-probing` / `ready-probing` / `healthy` / `error` (`stopped` = no chip). `[+ …]`
lists discovered-but-not-launched plugins. `activeTab` is Svelte `$state`. The shell is
`<div class="hud">` → `<header class="topbar">` + `<main class="grid">` today (`App.svelte`);
the switcher slots into the topbar and the content region below it is swapped.

**Two content kinds, one reserved rect:**
- **`Fleet` tab → in-DOM.** The current `<main class="grid">` ops grid stays exactly as-is —
  trusted, default, never a webview. Selecting Fleet just shows it.
- **App tab → child webview.** The shell renders an empty positioned placeholder
  (`<div bind:this>`) filling the content area; Svelte measures its rect and hands
  `{x, y, width, height}` to Rust, which creates/positions a child webview **over** that rect.
  The webview is a real top-level browsing context (own origin → cookies/popups/redirects work).

**Rust ↔ shell coordination (the spike-de-risked part):**
- `plugin_show(id, rect)` — first show: create the child webview at `url` (only once `healthy`);
  later shows: reposition + unhide.
- `plugin_hide(id)` — on switch-away: hide (do not destroy).
- `plugin_set_rect(id, rect)` — fired by a Svelte `ResizeObserver` on the placeholder so the
  webview tracks the reserved box on window resize / layout change.

**Compositing reality (do not assume CSS z-index works across the boundary).** A Tauri child
webview is a **native OS view composited over the host webview**, not a DOM node. Shell-rendered
HTML modals, dropdowns, toasts, and tooltips therefore **cannot** paint on top of the child
webview via `z-index` — they live in different layers. "Below modals" is achieved by **Rust
hiding the active app webview whenever a shell overlay opens and restoring it on close**, via an
explicit `plugin://overlay-open` / `overlay-close` signal from the shell — not by stacking order.
Any host UI that must appear over an app (command palette, global modal, the switcher's own
dropdown) triggers a hide. Likewise, native-view repositioning **trails** the host's CSS repaint,
so on window resize the webview visibly lags its rect for a frame or two; this is inherent to
overlay compositing, not a bug, and the spike's pass bar (§6) is set with that in mind.

**Keep-alive:** webviews are **hidden, not destroyed**, on switch-away (Audience is expensive to
cold-start and holds session state). The backend also keeps running; stopping it is an explicit
user action or app-quit (§3), not a tab switch.

**`webview` manifest block applied here — navigation matrix for Audience's hardest flows:**
- **OAuth (`window.open`)** → `popups: "allow"` opens a **child webview in the same session
  partition** as the app. The invariant: Audience's OAuth popup must `postMessage`/poll its opener
  to complete the connect, so it **must share the opener's session partition** — a popup in a
  foreign partition wouldn't share the session. (The exact poll cadence is incidental and not
  something the host depends on.)
- **Stripe checkout** → a **top-level navigation of the app webview** to an external HTTPS origin
  (Stripe's domain) and back to `FRONTEND_URL` (`:3000`). `externalLinks: "in-app"` keeps this in
  the app webview. Note this means a *first-party* app navigates to a genuinely *third-party*
  origin (Stripe) — explicitly allowed; the "trusted first-party" trust model (§5) governs the
  *plugin*, not every origin it may navigate to.
- **Genuinely-leaving links** → set `externalLinks: "system-browser"` to hand off to the OS
  browser. Not needed for Audience's core flows this cycle.

**Fallback "C" (separate `WebviewWindow`) — a likely outcome, not a remote one.** Because the
hide-on-overlay coordination and resize-lag above are real friction, the spike (§6) may well land
on "C": each app becomes its own `WebviewWindow` (titled via `webview.title`), and the switcher
focuses/raises windows instead of repositioning an overlay. "C" sidesteps the entire
cross-layer-compositing problem (each app owns its own OS window, modals are per-window). Same
lifecycle, same manifest, same trust — only the embedding surface changes. The build order (§6)
is written so that choosing "C" after the spike changes only the embedding layer, nothing
upstream.

## 5. Trust model + roadmap

**In scope — trusted first-party:**
- App plugins are **fully trusted**: real origin with network, cookies, popups, redirects — no
  sandboxing, no capability gating.
- **CSP posture:** each app webview gets **its own origin's CSP** — the host does **not** impose
  its `csp: null` (from `tauri.conf.json`) on child webviews; that null applies to the host shell,
  not to apps. `externalLinks: "in-app"` (§4) means a trusted first-party app may top-level-navigate
  to a genuinely **third-party origin** (Stripe) and back; we **accept** that the destination origin
  runs in the app's session partition for the round-trip. In a real deploy `success_url` is built
  from `FRONTEND_URL` and is attacker-influenceable — hardening the return-target is a roadmap item
  (below); for the trusted single-user proving cycle it is an accepted risk, named here, not solved.
- **Arbitrary shell is the accepted trust surface.** `lifecycle.start`/`stop` run whatever the
  manifest says, from Rust, with manifest `cwd`/`env`. Acceptable because every plugin this
  cycle is first-party and hand-authored — the manifest is effectively code.
- **No host↔app data bridge:** the host passes no fleet data to apps and reads nothing back.
- **Discovery trust:** anything in the dev list / packaged scan / `~/.command-center/app-plugins/`
  is trusted by being there — same posture as a bundled sidecar binary.

**Roadmap (out of scope — named so we don't build them now):**
- **Third-party plugins + isolation** — sandboxed origins, permission prompts, signed manifests,
  a capability allowlist gating shell/network. The big one; the rest is downstream of it.
- **Production auth** — Audience's Clerk `__session` cookie path (we run `devAuth` this cycle).
- **Host↔app bridge** — a typed channel if a future app needs fleet data.
- **Secrets management** — manifest `env` is plaintext today; real keys (Stripe/Runway/LLM,
  `TOKEN_ENC_KEY` KMS) need a secrets store, not the manifest.
- **Per-app resource limits / crash supervision** beyond the basic state machine (restart
  backoff, etc.).
- **External-navigation hardening** — constraining/allowlisting the origins an app webview may
  navigate to (and validating redirect return-targets like Stripe's `success_url`), once apps are
  no longer all trusted first-party.

## 6. Spike-first plan, build order & testing

### Spike #1 — child-webview embedding (go/no-go; before any production code)

A throwaway branch proving the riskiest dependency end-to-end. Each gate is pass/fail:
1. **`unstable` feature builds** — add Tauri's `unstable` Cargo feature (pinned version) + webview
   capabilities; app compiles & runs in dev + packaged.
2. **Renders a real app at a URL** — child webview loads Audience's web head (`:3000`) in **both**
   `npm run dev` and a packaged bundle.
3. **Real-origin behaviors work** — cookies persist; an OAuth-style `window.open` popup opens; a
   full-page redirect navigates.
4. **Rust positions it under a Svelte rect** — create over a reserved `<div>`, hide/show on
   tab-switch, and — the make-or-break sub-gate — **hide-on-overlay**, judged by **concrete,
   falsifiable** conditions (not a feel-test), with the call owned by the implementer recording
   pass/fail in `SPIKE-RESULTS.md`:
   - **Resize tracking:** after a window-resize ends, the webview settles to the new rect within
     **≤150 ms / ≤10 frames**; no content from outside the rect is ever painted.
   - **Hide-on-overlay round-trip:** opening a shell overlay (command palette / modal) hides the
     webview and closing restores it at the correct rect, round-trip **≤150 ms**, with **no
     visible flash of stale content** and the app's **scroll + focus state preserved** across the
     hide/restore, over **≥10 trials**.
   - **No-go → "C"** if any condition fails to hold reliably, or if hide-on-overlay produces a
     visible flicker that survives reasonable tuning.
5. **Lifecycle round-trips** — (`build` →) `start` → health-probe → ready-probe → `show`;
   quit runs a **blocking** `stop` and leaves **no orphaned containers** (verify with
   `docker ps`).

**Outcome → `spikes/SPIKE-RESULTS.md`:** go (proceed with overlay "B") or no-go (fall to "C"
separate windows, which only changes §4's embedding surface — manifest, lifecycle, switcher,
trust survive unchanged).

### Build order (after a go)
0. **Build prerequisites (Tauri config).** Add `tauri = { features = ["unstable"] }` to
   `src-tauri/Cargo.toml` and **pin the exact Tauri version** — `unstable` is explicitly not
   semver-stable, so a patch bump can silently break the embedding layer (note this in CI/upgrade
   docs). Add the webview-API permissions to `capabilities/default.json` — creating, positioning,
   showing, and hiding child webviews from Rust still goes through Tauri's permission system, and
   the capability must apply to the **dynamically-created webview labels**, not just `"main"`.
   (The Rust-side `start`/`stop` *spawn* is unconstrained by the JS `shell:allow-execute`
   allowlist — but the webview APIs are not.) **Define the webview-label scheme** here too:
   labels are derived from the plugin `id` (e.g. `app::<id>`), unique and stable across
   relaunch/adopt, and the capability glob must match that scheme.
1. **Manifest + discovery** — types, parse, `apiVersion` refusal, `build`/`env`/`okStatus`
   fields, `cwd` resolution, discovery seam. *(Pure, unit-testable, no Tauri.)*
2. **Lifecycle manager** — state machine, spawn/probe/stop, adopt-don't-respawn, `ExitRequested`
   cleanup. *(Bulk of the Rust logic + risk.)*
3. **Embedding + shell coordination (one slice).** The Rust webview create/position/show-hide/
   hide-on-overlay commands **and** the Svelte side that drives them — topbar switcher entries,
   state chips, reserved-rect placeholder + `ResizeObserver`, the `overlay-open/close` signal,
   Fleet-stays-in-DOM — are **two halves of one contract** and built together (the spike already
   proved them as a pair). Carry the spike's throwaway Svelte glue forward as the starting point
   rather than stubbing the Rust side against a mock and re-deriving the seam. *(Use
   `frontend-design` for the shell UI.)*
4. **Wire Audience end-to-end** — its manifest, the credential-free dev (devAuth/fake) build path,
   launch→use→quit.

### Testing strategy (matched to each layer)
- **Manifest/discovery:** plain Rust unit tests — parse valid/invalid, version refusal, discovery
  union/precedence.
- **Lifecycle:** the state machine is testable **without real Docker or real time** only if its
  side-effecting dependencies are extracted as injectable seams up front — not just `Probe` +
  `Spawner`, but also a **`Clock`** (drives `interval`/`timeout`, so timeout→error is asserted by
  advancing a fake clock) and a **state sink / event emitter** abstraction (so `plugin://state`
  transitions are asserted without a live `AppHandle`), and the **process-exit signal** modeled as
  an injectable event source rather than a hardwired `tauri-plugin-shell` `CommandEvent` task (so
  crash→error is tested by emitting a fake `Terminated`). With those four seams, transitions,
  timeout→error, partial-stack adopt, and crash→error are all pure-unit-tested. One slow
  integration test (feature-flagged) does a real `build`→`start`→health→`stop` against Audience's
  fake-provider stack and asserts no orphaned containers via `docker ps`.
- **Embedding:** not meaningfully unit-testable (OS/webview seam) → covered by the spike + a
  manual smoke checklist for dev & packaged.
- **Shell:** component-level tests for switcher state→chip rendering, rect-emit-on-resize, and
  Fleet-tab-renders-grid.
- **Regression guard:** the existing ops grid stays untouched — its current tests are the canary.

**TDD** applies to layers 1–2 and the shell logic (pure cores); the embedding seam is
spike-and-smoke. `verification-before-completion` gates each "done" claim on real output;
`requesting-code-review` before merge.

## Artifacts & references
- Audience digest: `docs/digests/audience-digest.md` (embedding constraints).
- Halyard digest: `docs/digests/halyard-digest.md` (why it's deferred — headless).
- Halyard head brief: `docs/superpowers/HANDOFF-2026-06-07-halyard-head.md`.
- Precedent: `cockpit/ui/src-tauri/src/lib.rs` (sidecar babysitter),
  `cockpit/ui/src-tauri/tauri.conf.json` (`externalBin`, `"csp": null`),
  `cockpit/ui/src-tauri/capabilities/default.json` (shell allowlist),
  `cockpit/ui/src/App.svelte` (shell layout).

## Design Critique Log

### Critique Round 1
An independent reviewer found eight load-bearing flaws, several existential to the proving cycle:

1. **Build-time vs. runtime env (existential).** Audience bakes auth posture + API origin at
   Docker *build* time, so a runtime `NODE_ENV=development` on a prod-built stack does **not**
   enable devAuth. **Resolved:** split the manifest into `build` (with `args`, baked) vs. `env`
   (runtime); added a `building` state and `build.cmd` run on first launch; documented the
   contradiction in §2 ("Build vs. runtime env").
2. **"Cookies just work" doesn't solve auth (existential).** Audience has no login UI and 401s
   without a Clerk `__session` cookie a fresh webview can't get. **Resolved:** added an explicit
   "Auth & origin precondition" in §2 — the *only* auth path this cycle is the devAuth build;
   webview cookie-jar capability is irrelevant to Audience auth.
3. **Ready probe rejects healthy server.** Audience root `/` 302-redirects, so a 2xx-only probe
   marks a healthy Next.js server `error`. **Resolved:** added per-probe `okStatus` sets
   (`ready` accepts 3xx); reframed `health`=readiness gate, `ready`=liveness only.
4. **60s health timeout too short for a cold 7-image build.** **Resolved:** separated `build`
   (20-min) from probe timeouts (raised to 180s); distinct `building` state.
5. **`ExitRequested` teardown was fire-and-forget → orphaned containers.** **Resolved:** §3 now
   specifies `api.prevent_exit()` + blocking `stop` to completion under a deadline + kill
   fallback; force-quit named as a known gap mitigated by adopt + an orphan-reconcile sweep.
6. **CSS z-index can't stack DOM modals over a native child webview.** **Resolved:** §4 now
   specifies hide-on-overlay coordination (Rust hides the webview when a shell overlay opens),
   acknowledges resize-lag as inherent, and reframes "C" as a likely (not remote) spike outcome;
   spike gate 4 made the hide-on-overlay sub-gate make-or-break.
7. **Missing Tauri prerequisites.** **Resolved:** added build-order step 0 — pin Tauri +
   `unstable` feature, and webview-API capabilities applied to dynamically-created webview labels
   (the spawn is unconstrained by the JS allowlist; the webview APIs are not).
8. **CORS / fixed-port / adopt-TOCTOU.** **Resolved:** §2 states the same-origin-as-built
   precondition (Audience api has no CORS); §3 notes fixed `:3000`/`:8080` single-instance and
   the adopt check-then-act race, both accepted for a single-plugin single-user cycle.

### Critique Round 2
A fresh reviewer confirmed R1 fixes #1–#3 were correct, then found seven remaining flaws —
mostly in lifecycle failure/recovery semantics and spike falsifiability:

1. **Adopt path bypassed the ready-probe.** Adopting on `health` alone could show a webview
   against a dead `:3000` after a partial crash. **Resolved:** §3 step 1 now adopts only when
   **both** probes pass; a health-only (partial) stack falls through to `start`, which is
   idempotent (`compose up` brings up only missing services).
2. **Spike gate 4 was unfalsifiable** ("feels unacceptable"). **Resolved:** §6 gate 4 now has
   concrete thresholds (resize settle ≤150 ms/≤10 frames; hide-on-overlay round-trip ≤150 ms, no
   stale-content flash, scroll+focus preserved, ≥10 trials) with a named owner recording the call.
3. **`cwd` was a hardcoded absolute dev path in a "packageable" contract.** **Resolved:** §2 now
   defines path resolution as relative-to-manifest-dir, absolute as escape hatch (which the
   Audience proving manifest uses).
4. **`ExitRequested` teardown lacked a concurrency/deadline contract.** **Resolved:** §3 now
   specifies concurrent (join-all) `stop` under a single *total* deadline kept under the OS
   force-kill ceiling, with kill fallback.
5. **Test seams were under-specified** — only `Probe`/`Spawner`, but the state machine also uses
   wall-clock polling, event emission, and a `CommandEvent` stream. **Resolved:** §6 testing now
   names four seams (`Probe`, `Spawner`, `Clock`, state-sink/event-emitter) plus an injectable
   process-exit source, so timeout→error and crash→error are pure-unit-testable.
6. **Build order split embedding (3) and shell (4)** though they're one contract proven together
   in the spike. **Resolved:** §6 merges them into one "embedding + shell coordination" slice that
   carries the spike's Svelte glue forward; webview-label scheme (`app::<id>`) named in step 0.
7. **`externalLinks`/`popups` defaults asserted to "just work"** against Audience's hardest flows.
   **Resolved:** §2 adds a `"system-browser"` option; §4 spells out the navigation matrix (OAuth
   popup shares the session partition; Stripe is a top-level nav to a third-party origin and back,
   explicitly allowed despite first-party trust governing the plugin not every origin).

### Critique Round 3
A final reviewer confirmed the R1/R2 fixes are internally consistent and mutually compatible, the
scope is still single-plan-sized, and the "B"-vs-"C" isolation claim holds. Four remaining flaws,
all cheap, resolved:

1. **[HIGH] CSP / third-party-origin boundary was implicit.** With host `csp: null` and a
   first-party app navigating to a third-party origin (Stripe), nothing stated the child-webview
   CSP posture or the accepted risk. **Resolved:** §5 now states each app webview gets its own
   origin's CSP (host `csp: null` does not cascade to children), names the third-party-origin
   round-trip as an accepted single-user-cycle risk, and adds an "external-navigation hardening"
   roadmap item (validating redirect return-targets).
2. **[MED] State-name vocabulary drifted** — §3 said `ready-probing`, §4 chips said `ready`.
   **Resolved:** §3 now declares a canonical `plugin://state` enum and §4 chips reference it
   verbatim.
3. **[MED] Crash-under-kept-alive-webview seam was open** — `healthy → error` left a hidden
   webview bound to a dead `:3000` with no recreate path. **Resolved:** §3 now destroys the
   kept-alive webview on `healthy → error`, so the next launch takes §4's "first show: create".
4. **[LOW] OAuth justification rested on a poll number** from an undiffed Audience branch.
   **Resolved:** §4 reframes the requirement as the invariant (popup must share the opener's
   session partition to postMessage/poll), with the cadence noted as incidental.

**Outcome:** no remaining existential flaws, no scope creep, no unresolved contradictions. Design
approved through three independent adversarial rounds.
