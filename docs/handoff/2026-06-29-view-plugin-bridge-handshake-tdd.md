# Session Pickup — 2026-06-29 · View-plugin handshake (P4) — TDD determination

**Branch:** `feat/view-plugin-bridge-handshake`
**Active spec:** [`docs/superpowers/specs/2026-06-07-view-plugins-design.md`](../superpowers/specs/2026-06-07-view-plugins-design.md) (§"The plugin contract" → handshake)
**Lane context:** Lane A2 (view-plugin runtime) from [`docs/handoff/2026-06-25-ship-readiness-swarm-handoff.md`](2026-06-25-ship-readiness-swarm-handoff.md)

## Why this session existed

P3 (app-plugin webview) **passed**. P4 (view-plugin handshake) was **failing 100% of handshakes** —
the owner believed it was "a fundamental flaw with the implementation." Goal: scaffold tests from the
spec and **use TDD to determine the problem**. Owner's read (to confirm next session): the flaw was
**host-side** — *the host held the transferred MessagePort but never started it* (the "former" of two
candidates). This session built the host handshake fresh and reproduced/guarded exactly that flaw.

## Determination (the answer to "what was the problem")

**Root cause (host side, high confidence):** the host mints a `MessageChannel`, transfers `port2` to the
plugin iframe, and holds `port1` to receive `ready`. **If `port1.start()` is never called, nothing the
plugin posts — including `ready` — is ever delivered, so every handshake hangs and times out.** That is
precisely the "all handshakes failed" symptom.

**Proven** by mutation: removing `port.start()` in `bridge.ts` flips the happy-path test from pass →
**timeout/RED**; restoring it → GREEN. (Toggle `bridge.ts:` `port1.start?.()` to re-verify.)

**Why a spike could "look correct" yet fail every time:** a probe showed **jsdom's `MessagePort` does NOT
enforce `start()`** — it delivers messages regardless. Real WebView2 does enforce it. So a naive jsdom
unit test goes green with the bug present (false negative). The fix was to build a **faithful,
`start()`-enforcing** port/window model (`bridge.testkit.ts`) as the executable spec of real browser
behavior — that harness is what makes the flaw observable.

## Where we are in the plan

Host side of the handshake built test-first. **76 tests passing** on this branch (72 baseline + 4 new).
`npm run check` clean (0 errors, 337 files).

| Slice / Task | Status | Notes |
|---|---|---|
| Faithful browser-semantics test harness (`bridge.testkit.ts`) | done | start()-enforcing FakePort/FakeMessageChannel/FakeWindow |
| Handshake happy path: hello→init(+port)→ready resolves | done | the `port.start()` guard lives here |
| Liveness timeout (3s → reject → ops-grid fallback) | done | bounds both "never says hello" and "never readies" |
| apiVersion refusal (no port to an unspeakable plugin) | done | `supportedApiVersions`, default `[1]` |
| Port identity (window-spoofed `ready` ignored) | done | host trusts only the private port |
| **Plugin SDK `connect()`** (`cockpit/plugin-sdk/`) | **pending** | the mirror flaw — see below |
| Bridge runtime: dirty-delta `state`, `log-append`/`log-reset`, `command-ack` | pending | spec §Messages |
| Command **policy** (shape/authority/cost/rate/flood-kill) | pending | spec §"Host command policy" — the trust boundary |
| `ccplugin://` scheme handler + two CSPs (lib.rs / tauri.conf.json) | pending | Lane S owns the shared shell files |

## What to pick up next (in order)

1. **Confirm the determination** against the original spike (owner to check) — does "held the port but
   never started it" match? If it was instead the **SDK side**, build #2 first.
2. **Build the plugin SDK `connect()` with TDD** (`cockpit/plugin-sdk/`, does not exist yet). The *other*
   deterministic 100%-failure mode is the SDK posting `plugin-hello` **before** attaching its `init`
   listener → the port-bearing reply is missed every time. Mirror-image of this session's flaw; reuse
   `bridge.testkit.ts`. An end-to-end test wiring the fake SDK to `connectPlugin` proves both ends.
3. Then the bridge runtime + command policy (security-critical) per the spec's build order.

## Known limitations

- **Platform transfer is NOT proven.** Whether WebView2 actually delivers a transferred `MessagePort`
  into a `sandbox="allow-scripts"` null-origin iframe — gate (b)'s true platform risk — **cannot be
  proven in jsdom**. This suite proves the *protocol logic*, not the platform transfer. The manual
  packaged run (gates a/b/c-CORS/c-CSP/d → `spikes/SPIKE-RESULTS.md`) is still required.
- The original P4 spike harness is **not in this repo** (no `spike/view-plugins-handshake` branch,
  commit `9fbce0a` unknown). This work was built fresh from the spec, not by debugging that harness.
- `bridge.ts` is host-handshake only — no state/log/command surface yet.

## Servers / commands used

- `cd cockpit/ui && npm ci` — deps were not installed; required before tests.
- `cd cockpit/ui && npx vitest run src/lib/bridge.test.ts` — handshake suite.
- `cd cockpit/ui && npm test` — full suite (vitest run). `npm run check` — svelte-check + tsc.
- Re-prove the flaw: comment out `port1.start?.()` in `src/lib/bridge.ts` → handshake test times out.
