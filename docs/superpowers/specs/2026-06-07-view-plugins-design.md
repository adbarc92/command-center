# Cockpit View Plugins + Battlefield Skin — Design

> Status: **passed 3 adversarial critique rounds; READY TO IMPLEMENT (recommend A→B split).** See Design Critique Log.
> Parent: [../../command-center-vision.md](../../command-center-vision.md) (SP5 · view-plugin slice)
> Date: 2026-06-07

## Goal

Make the cockpit's view layer **pluggable**: the daemon's fleet-state stream feeds a
shared state model, and **untrusted, sandboxed renderer plugins** present it however
they like — proving the vision's "own the engine, design for the skin." This cycle
ships the **sandboxed view-plugin runtime** plus, on top of it, an RTS **Battlefield
game-skin** at *view-layer* parity with AgentCraft. The ops grid stays the trusted
built-in default. Non-goal: the other SP5 slices (app-plugins; mobile/remote).

## Grounding

- AgentCraft's edge is its **view layer**; our engine already exceeds it (it has no PR
  creation, review gate, or $ tracking). "Parity" = *view parity*, bounded by our
  stream + command API.
- Cockpit today = one `App.svelte` (ops grid) over `lib/{api,fleet,types}.ts`. The
  shared store must be **extracted** (Svelte-5 `.svelte.ts` caveat below — not trivial).

---

## Architecture

```
fleetd ──WS/REST──► Cockpit HOST (trusted; Tauri v2.11 + Svelte 5)
                      ├─ Transport     api.ts (stream, commands) — exists
                      ├─ Store         store.svelte.ts: folds events → FleetState;
                      │                 SINGLE command sink (createMission/command)
                      ├─ Built-in view ops grid (default; trusted; calls store directly)
                      ├─ Host overlay  human-authority modals (oracle approval, real-launch
                      │                 confirm) — host-rendered, focus-stealing, above iframe
                      └─ Plugin HOST
                           ├─ Loader    discover plugins (dev index | packaged scan); apiVersion+caps
                           ├─ Sandbox   <iframe sandbox="allow-scripts">  (opaque null origin)
                           └─ Bridge    MessagePort (identity-bound): host→plugin state/log deltas;
                                        plugin→host command → POLICY → store command sink
                 view-switcher: [Ops grid] [Reference] [Battlefield] [⋯discovered]
```

**Boundary.** A plugin is **untrusted UI in a sandboxed iframe** (`allow-scripts`,
**never** `allow-same-origin` → opaque `"null"` origin; no host DOM/storage; no network;
no daemon URL; no creds). Only channel = a **MessagePort** transferred at handshake.
**Isolation comes from the sandbox opaque origin, not the URL** (all plugins share the
`http://ccplugin.localhost` origin; `<id>` is only a path). The host is the sole party
that talks to the daemon and **polices** every command.

**Decisions.** Ops grid stays a trusted built-in; the contract is **dogfooded by a
trivial reference plugin** before the big Battlefield skin. **Human-authority actions
(oracle approval, real-mode launch confirmation) are rendered by the HOST**, never
issuable by a plugin (see Host overlay).

---

## The plugin contract

### Handshake — plugin-announces-ready (corrected; no `load`-race)
A `load`-fired host post can race the plugin's listener (dropped init). So the **plugin
announces readiness first**:
1. Host attaches its `message` listener, then creates the sandboxed iframe.
2. The plugin SDK's first action posts a window-level `plugin-hello` to `parent`
   (`postMessage(hello, "*")` — allowed from a null-origin child).
3. The host replies with `init` **transferring `port2`**:
   `iframe.contentWindow.postMessage(init, "*", [port2])`. (`"*"` is safe — payload is
   non-sensitive.) Thereafter **all traffic flows over the private port**; the host
   identifies the plugin by **holding the port**, not by `event.origin` (which is
   `"null"` and unusable for auth). MessagePort transfer into an `allow-scripts`
   sandbox is the canonical secure pattern and works on WebView2.

### Manifest
```json
{ "id":"battlefield","name":"Battlefield","version":"0.1.0",
  "apiVersion":1,"capabilities":["log-append"],"entry":"index.html" }
```
Host refuses an unsupported `apiVersion`. **`capabilities`** is an explicit named set
(e.g. `log-append`, `real-launch-confirm`) so features can grow without lockstep
apiVersion breakage; `init` echoes the host's supported capability set.

### Messages (all `{ v:1, type, … }`, over the port)
- **host → plugin:**
  - `init` — apiVersion + capabilities (+ transferred port). Plugin → `ready`.
  - `state` — **per-unit dirty deltas**: `{ changed: UnitLite[], removed: string[], order }`,
    debounced ~60ms; a **full** snapshot only on `ready`/reconnect. A full snapshot **resets
    the bridge's per-unit `lastEmitted` baseline**; subsequent deltas diff against it (so the
    snapshot and the first delta never double-send a unit). `UnitLite` is the unit **minus
    `log`** with `history` capped to a generous N (≥ a worst-case bounded phase walk; loop
    counts come from `iters`, not `history` length). `removed` is **reserved-always-empty
    this cycle** (the store never deletes units; a skin treats a terminal `phase` as
    "destroyed").
  - `log-append` — `{ unitId, lines:[{seq,stream,line}] }` append-only deltas (new since the
    plugin's per-unit seq cursor). The operation-ticker = last line.
  - `log-reset` — on a daemon-stream **reconnect**, tells the plugin to discard its per-unit
    seq cursors (a fresh full `state` follows), preventing gaps/floods.
  - `command-ack` — `{ reqId, ok | rejected, reasonClass }` — echoes the plugin's `reqId` so
    the SDK can resolve the originating `launch()`/`command()` promise (a plugin firing two
    commands can tell which was rejected; "launch denied" instead of a hang).
- **plugin → host:**
  - `ready` — ack.
  - `command` — `{ reqId, launch{task,tier,mode} | unit{id, halt|resume|abandon|ship} }`. The
    plugin-supplied **`reqId`** is opaque + plugin-scoped (distinct from the trusted
    host-generated `cmd_id`) and is echoed in `command-ack`. **Oracle approval/rejection are
    NOT plugin-issuable.**

`health` is **not** sent raw; the plugin gets a coarse `degraded: boolean` (the literal
`anthropic_key`/`version` stay host-side — no recon for untrusted code).

### Host command policy = the trust boundary (a policy, not a schema check)
- **Shape:** known type; `unit.id` exists in `FleetState`; fields typed + bounded
  (`task` length/charset).
- **Authority:** `approve_oracle`/`reject_oracle` rejected from plugins (host-only, via
  the overlay).
- **Cost/`real`:** plugin `launch` is **demo-only** unless the **host real-launch confirm
  overlay** approves it.
- **Rate (two layers):** (1) **inbound port-message ceiling** (msgs/sec) measured *before*
  policy — exceeding it `port.close()`s the plugin (a flood can't be deserialized into a
  host-thread stall); (2) a **per-plugin token-bucket** on accepted commands, stricter on
  `launch`.
- `cmd_id` stays host-generated. All commands (built-in + policed plugin) flow through the
  **store command sink**.

### Plugin SDK (bundled convenience, still sandboxed)
```js
const fleet = await connect();        // posts plugin-hello, awaits init port
fleet.onState(s => apply(s));         // dirty deltas {changed,removed,order}
fleet.onLog((unitId, lines) => …);    // append-only; onReset(() => …) clears cursors
fleet.onAck(a => …);                  // command-ack
fleet.launch({task,tier,mode}); fleet.command(unitId,'halt');
```

---

## Host overlay — human-authority actions (resolves the oracle-approval gap)

A full-frame untrusted iframe can't host trusted approval UI. So the host renders a
**focus-stealing modal layer** in its own Svelte shell, at `z-index` above the iframe,
**triggered by the host's own `FleetState`** (which the host owns), independent of plugin
cooperation:
- When any unit enters `awaiting_oracle_approval`, the host shows an **Oracle Approval
  modal** (the frozen test set + APPROVE/REJECT). While open it **captures pointer +
  keyboard via the `inert` attribute on the `<iframe>` element** — a backdrop + z-index alone
  does NOT stop a focused element *inside* the frame from receiving keystrokes; `inert`
  (honored by WebView2/Chromium) removes the whole frame subtree from focus/hit-testing —
  plus moving focus to the modal. The plugin may render only a
  non-interactive "AWAITING APPROVAL" indicator from its `state` flag; it has no approval
  verb and cannot dismiss or satisfy the modal.
- The same overlay infrastructure renders the **real-mode launch confirmation** when a
  plugin requests `launch{mode:'real'}`.
- **AC:** with a unit in `awaiting_oracle_approval`, the modal appears regardless of the
  plugin; the plugin cannot approve, dismiss, or satisfy it; keyboard focus is the host's
  while open (plugin hotkeys suppressed).

---

## Store extraction — single command sink (resolves two-writers)

`store.svelte.ts` (Svelte-5 runes require a `.svelte.ts` module — a plain `.ts` `$state`
is **not** reactive) becomes the **single owner** of: the `units/order/health`
`FleetState`, the WebSocket lifecycle (`openStream`, `sockets`, reconnect/dedup
`seq<=lastSeq`), **and the only callers of `api.ts`** via `store.createMission()` /
`store.command(unitId,name)` — which own `cmd_id`, the optimistic new-unit insert, and
socket-open. The ops grid and the bridge **both** call these store methods; the bridge's
policy sits **in front of** `store.command` for plugin-originated requests only (built-in
calls are trusted and skip policy). This removes the duplicate-unit/double-socket bug a
second optimistic writer would cause. **Regression test:** a bridge `launch` concurrent
with a `reconnect()` yields exactly one unit + one socket.

**Dirty-set mechanism (not a `$derived`).** Svelte-5 runes tell *components* what to
re-render; they do **not** hand a plain consumer like the bridge a "which units changed"
set. So the store's fold/command path (which already mutates one `unit.id` at a time —
cf. today's `onEvt`) pushes touched ids into a plain `dirty: Set<string>` accumulator that
the **bridge drains on its ~60ms tick** to build the delta. (Diffing 50 full units every
tick is the lazy fallback the payload-size AC guards against — don't reach for a `$derived`
that can't deliver per-key deltas cheaply.)

---

## Sandbox, CSP, loader & the Tauri reality

**Tauri v2 / WebView2 facts (Windows is primary):**
- A `register_uri_scheme_protocol` scheme resolves to `http://<scheme>.localhost/…` — a
  real distinct origin from the app, **same** for all plugins (`<id>` = path).
- `connect-src` governs only fetch/XHR/WS/EventSource/beacon — it **never** blocks
  `<script src>`/`<link>`/module imports, so a plugin loads its own code freely. The real
  self-code-load risk is **CORS on cross-origin module graphs**, a *separate* mechanism.

**Two distinct CSPs, specified:**
- **Host app CSP** (`tauri.conf.json`, currently `"csp": null`) — author from scratch; must
  include the plugin scheme in `frame-src` (the WebView2 origin form `http://ccplugin.localhost`,
  not the bare scheme) and not break Vite HMR.
- **Plugin-document CSP** — delivered as a **response header from the `ccplugin://` scheme
  handler in `lib.rs`** (not a `<meta>` the plugin controls):
  `default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self'
  data:; connect-src 'none'; frame-src 'none'; base-uri 'none'; form-action 'none'`.
  (Fallback #1 swaps `script-src 'self'` → hashed-inline.)

**Spike #1 — hard go/no-go, separated gates, dev AND packaged:**
- (a) sandboxed iframe from the scheme renders;
- (b) plugin-hello → `init` **MessagePort** round-trip succeeds **across 100 reloads, no
  dropped handshake** (catches the timing race);
- (c-CORS) a `<script type=module>` imports a second file from the same scheme **OR** the
  single inlined bundle (fallback #1) runs;
- (c-CSP) the plugin-doc CSP above permits self scripts and blocks network;
- (d) host CSP authored without breaking Vite HMR.
Record in `spikes/SPIKE-RESULTS.md`. **Pre-committed fallbacks:** #1 single inlined bundle
(if cross-origin module load fails); #2 loopback static server on a random port serving
*only* plugin assets, iframe still sandboxed. ⚠️ **The loopback server MUST emit the same
plugin-doc CSP header** (`connect-src 'none'`, etc.) the scheme handler would — a stock
static dev server ships no CSP, which would dissolve the "no network" guarantee. (⚠️ also:
`fleetd` has **no API auth**, so a loopback origin is only safe because the plugin frame
has no network — a daemon loopback token is a broader item this design flags but doesn't
require).

**Loader / install — dev vs packaged seam (made concrete):** discovery is a single
injected interface with two impls. **Dev (`vite dev`):** a Vite static/middleware route
serves `plugins/*`; the loader discovers them from a **build-time-generated dev index**
(no `~/.command-center` scan, no Tauri scheme — those don't exist under plain Vite).
**Packaged:** first-party plugins are bundled as Tauri resources and the `ccplugin://`
handler serves from **bundled-resources ∪ `~/.command-center/plugins/`**; the loader scans
those dirs. A smoke test loads the reference plugin end-to-end in **both** environments.

**Liveness / crash contract:** `ready` **timeout** (3s → "plugin failed to start," fall
back to ops grid); optional `heartbeat` ping/pong; on crash/timeout/flood → `port.close()`
+ remove the iframe + host `systemMessage` + **auto-revert the view-switcher to the trusted
ops grid**. View-switcher lifecycle: unmount → `port.close()` + iframe removal → mount next
(built-in views skip the bridge); a rapid A→B→A toggle leaks no sockets/ports (tested).

---

## The Battlefield game-skin (Spec-B; on the proven runtime)

From `state`/`log-append` + the real command set — **no new daemon features**; each
feature has an AC.

1. **Units = entities, position = pipeline progress** (Dispatch→Provision→[Build·Check·
   Review **arena**]→Merge·PR→Done; colour=phase, ring=cost, T# pip=tier, chip=needs-you/
   rate-limit/slot). **AC:** x-position matches phase zone; updates within one debounce tick.
2. **Selection:** click; hotkeys `1–8`; **state-cycle** `Tab` (needs-you→working→queued→
   failed); **control groups** `Ctrl+1–9` (lowest-value — droppable for v1). **AC:** `Tab`
   follows the documented order; `Ctrl+1` then `1` recalls the set.
3. **Command panel:** task, cost gauge, tier, iterations, oracle/findings, **live log +
   operation ticker**, PR link, action buttons = real commands (resume/halt/abandon/ship)
   **per-phase enabled**; oracle approval surfaces via the **host overlay**, not a plugin
   button; a rejected command shows via `command-ack`. **AC:** buttons enabled exactly for
   valid phases; a command round-trips and its ack is observable.
4. **Mission launcher** (task+tier+mode → `launch`; real → host confirm). **AC:** demo
   launch creates a unit; real launch prompts the host overlay first.
5. **Resource bar + alerts** (active/units/burn; docker/key from coarse `degraded`;
   needs-you pulse; jump-to-next-needs-you). **AC:** burn = sum of unit costs.

**Deferred:** race-skins/achievements/music; voice/terminal/file-explorer; minimap/fog;
multi-backend spawn / in-view git / free-form prompting.

---

## Scope & phasing

Runtime (security-critical) + RTS skin (large UI) are **separable**, and after three
critique rounds the **recommended plan is two cycles**: **Cycle 1 = Spec-A** (spike → store
→ bridge+policy → SDK → loader/scheme/CSP → reference plugin), shipped and hardened on its
own — its value (a proven, sandboxed plugin contract) stands without any skin; **Cycle 2 =
Spec-B** (Battlefield), built on the now-proven contract with its own per-AC test budget.
Spec-A alone is ~6–7 security-critical workstreams; bundling the full RTS skin into one
cycle risks the skin's size pressuring A's hardening. **The project owner asked for one
cycle** — this is flagged as the one open decision for the spec review: ship A→B as two
cycles (recommended) or one combined cycle. Either way the build order is the same; only the
merge/review boundary moves.

## Testing

- **Spike harness** (gated go/no-go, dev+packaged) → `SPIKE-RESULTS.md`.
- **Store:** carry-over `fold` tests; **reactivity when consumed from a component**
  (`.svelte.ts`); reconnect/dedup after extraction; **single-sink** test (bridge launch +
  concurrent reconnect → one unit/one socket).
- **Bridge:** plugin-hello→init→ready handshake (100×, no drop); port identity (stray
  window message ignored); dirty-delta `state` (only changed units; payload-size AC on a
  50-unit fleet); `log-append` since-seq; `log-reset` on reconnect; `command-ack`.
- **Policy:** valid forward; unknown type / unknown id / malformed / **`approve_oracle`** /
  over-bound `task` rejected; plugin `real` blocked pending host confirm; token-bucket +
  **inbound-flood kill** (10k msgs/s doesn't stall a host rAF > X ms).
- **Host overlay:** approval modal appears on `awaiting_oracle_approval` regardless of
  plugin; plugin can't satisfy/dismiss; focus captured.
- **Reference plugin** must exercise the **full message surface** (not just a unit list):
  dirty-`state` apply, `log-append`, a policed `launch`, a `command-ack` rejection path, and
  presence during `awaiting_oracle_approval` — "trivial" = small UI, not narrow coverage, so
  the contract is *proven* (not merely compiled) before Battlefield.
- **SDK / Loader / Battlefield (per AC)**; switcher A→B→A leak test;
  liveness (no-ready timeout, mid-session silence, flood→kill→ops-grid fallback).
- `npm run build && npm run check` clean; ops-grid behavior unchanged.

## Build order

1. **SPIKE** (gates above; pick fallback if needed).
2. Extract **`store.svelte.ts`** (sockets/reconnect + single command sink) + regression tests.
3. **Bridge**: MessagePort handshake (plugin-hello) + dirty-delta `state` + `log-append`/
   `log-reset` + **command policy** (shape/authority/cost/rate/flood-kill) + `command-ack`.
4. **Host overlay** (oracle approval + real-launch confirm; focus-stealing).
5. **Plugin SDK**; **loader** (dev index | packaged scan) + custom scheme + both CSPs +
   **view-switcher** + liveness; ship the **reference plugin** and dogfood end-to-end.
6. **Battlefield plugin** (Spec-B), per the ACs.

## File structure

Host: `cockpit/ui/src/lib/{store.svelte.ts (new), bridge.ts (new), loader.ts (new),
api.ts, fleet.ts, types.ts}`; `cockpit/ui/src/{App.svelte (shell+switcher), lib/overlay/*}`;
`cockpit/ui/src-tauri/{src/lib.rs (scheme+plugin CSP header), tauri.conf.json (host CSP)}`.
SDK: `cockpit/plugin-sdk/`. Plugins: `plugins/{reference,battlefield}/`. `spikes/SPIKE-RESULTS.md`.

## Open risks

- **Sandbox + scheme + two CSPs + opaque-origin self-code-load + MessagePort handshake** —
  the make-or-break unknown; Spike #1 with separated gates + two pre-committed fallbacks.
- **`fleetd` has no API auth** — out of scope (sandboxed plugin has no network); a
  prerequisite only for the loopback fallback; flagged as broader hardening.
- **Store extraction** touches the live reconnect/socket path + the Svelte-5 `.svelte.ts`
  rule; regression-tested before the bridge.
- **Per-tick clone** reduced to dirty-deltas + capped `history`; revisit only if it bites.
- **Reconnect replay-from-0** is inherited from today's cockpit (`openStream(..,0,..)`; the
  `Snapshot.last_seq` field exists but is unused); the `log-reset`/full-snapshot path
  amplifies it. Out of scope this cycle; revisit with `since=last_seq` if it bites.
- **Scope** runtime + skin; recommended split A→B across two cycles (owner's call).

## Design Critique Log

Three independent adversarial critique rounds, each a fresh agent grounded in the actual
cockpit code + Tauri config + verified browser/WebView2 platform facts, each seeing the
prior round's revision.

### Critique Round 1
Found three **Critical** platform-rooted flaws plus important gaps:
- **"Origin-checked both ways" is impossible** — a sandboxed iframe (no `allow-same-origin`)
  has a `"null"` origin, so origin-auth and a non-`"*"` targetOrigin can't exist. → Rebuilt
  the bridge on a **`MessageChannel`/`MessagePort`** transferred at handshake, identified by
  `event.source`/port identity, not origin.
- **Opaque-origin iframe may not load its own ES-module SDK** (cross-origin module/CORS). →
  Made the spike a **hard go/no-go** with **two pre-committed fallbacks** (single inlined
  bundle; loopback static server).
- **Loopback fallback silently re-opens the daemon** (fleetd has no auth). → Hardened it
  (sandbox + `connect-src 'none'`) and flagged the daemon-auth dependency.
- **Validator was shape-only** though it's "the entire trust boundary." → Specified a real
  **policy**: reject plugin-issued `approve_oracle`/`reject_oracle`, demo-only launches
  (real needs host confirm), token-bucket rate limits, task bounds.
- Also: **log-laden full snapshots** too heavy (→ split `state` vs `log-append` deltas);
  **store must be `.svelte.ts`** (a `.ts` `$state` isn't reactive); added a **trivial
  reference plugin** to dogfood the contract.
Verdict: needs rethinking on bridge/asset/validator — resolved in the revision.

### Critique Round 2
Confirmed the bridge is viable (MessagePort-into-sandbox works; `connect-src` doesn't block
script loading), but found two genuine blockers + gaps:
- **Host-rendered oracle approval over a full-frame iframe was hand-waved (Critical).** →
  Added a concrete **host overlay**: a focus-stealing modal triggered by the host's own
  `FleetState`, plugin gets only a non-interactive flag; same overlay for real-launch confirm.
- **Two-writers command-path ambiguity (Critical)** — after store extraction, who calls
  `createMission`/`sendCommand`? → Made the store the **single command sink**; ops grid +
  bridge both call it; policy sits in front of plugin-originated calls only.
- **`load`-event handshake race** → inverted to **plugin-announces-ready** (`plugin-hello`).
- **CSP under-spec / self-contradiction** → specified **two distinct CSPs** (host app +
  plugin-doc-as-response-header) explicitly.
- **`state` still deep-cloned all units each tick** → **per-unit dirty deltas**.
- **Inbound port-flood can wedge the host thread** regardless of command rate-limit → added
  an **inbound-message ceiling + `port.close()` kill**; plus a **liveness/crash contract**
  (`ready` timeout → ops-grid fallback) and **dev/packaged loader seam** (dev index vs
  packaged scan). Also: coarse `health`, named `capabilities`, `log-reset`, `command-ack`.
Verdict: resolve the two blockers + concretize — resolved in the revision.

### Critique Round 3
Traced a full plugin lifecycle end-to-end and found **no Critical** issues; the architecture
hangs together. Remaining items were **surgical**, now folded in:
- **`command-ack` lacked correlation** → added a plugin-supplied **`reqId`** echoed in the ack
  (SDK resolves the originating promise).
- **`removed` is uncomputable** (the store never deletes units) → documented **reserved-empty
  this cycle**; skins treat terminal `phase` as "destroyed."
- **Snapshot vs delta baseline** → a full snapshot **resets `lastEmitted`**; deltas diff
  against it.
- **Dirty-set isn't free on runes** → named the **`dirty: Set` accumulator** the bridge drains
  (not a `$derived`).
- **Loopback fallback must emit the plugin-doc CSP header** (a stock static server ships none).
- **Focus-stealing needs `inert` on the `<iframe>`** (backdrop+z-index don't stop in-frame
  keystrokes).
- **Reference plugin must exercise the full message surface** (not a narrow list).
- **Recommended the Spec-A/Spec-B split as the default plan** (two cycles), flagged as the one
  open decision for the owner.
Verdict: **READY TO IMPLEMENT** with those surgical edits (now applied); no further round needed.
