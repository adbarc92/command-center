# Halyard — Codebase Digest (for agents)

> Audience: an agent integrating Halyard as an *app plugin* inside a Tauri v2 + Svelte 5 "command center" desktop dashboard.
> Source: commit `d0b3987` · 2026-06-07 · digested by reading ~12 files (package.json, README, INTEGRATION.md, index.ts barrel, cli.ts, web surface adapter, http flag client, state contract, org config) + grep/glob over the full tree for UI/server code (none found).
> Purpose of this digest: **integration** (embed as a plugin in another app's shell).

## TL;DR

Halyard is a **headless, event-driven release + publicity coordinator** for a multi-app shop (iOS / Android / web / desktop). It is a **Node ≥ 20 / TypeScript / ESM** package that ships as **(a) a CLI (`halyard`) and (b) an importable library** — there is **no UI, no web app, no HTTP/WS server, no `index.html`, and no dev server**. State is **git-backed JSON files** under `state/`; it runs **fully offline by default**, polling external systems (App Store Connect, a flag provider, Sentry, Stripe) only when their secrets are present. The single most important integration fact: **you cannot load Halyard in an iframe/webview — it has no front end to display.** To surface it in the command center you must either shell out to the `halyard` CLI and render its JSON output, or `import` the library functions and build your own Svelte UI on top of them.

## Where to look (navigation index)

| I need to… | Go to |
|------------|-------|
| Understand the whole design/rationale | `design.md` (declared source of truth) |
| Embed it as a library (the relevant path for this plugin) | `docs/INTEGRATION.md` |
| See the full public export surface | `src/halyard/index.ts` |
| Drive the CLI programmatically | `dispatch(args)` in `src/halyard/cli.ts:697` |
| Add/understand a CLI command | `src/halyard/cli.ts` (`dispatch` at `:697`, usage at `:735`) |
| Read release/launch/proposal state | `src/halyard/coordinator/record-store.ts`, `launch-store.ts`, `proposals.ts` |
| Understand the state machine | `src/halyard/contracts/state.ts`, `coordinator/state-machine.ts` |
| Operator "why is this stuck?" view | `summarizeRelease` in `src/halyard/coordinator/status.ts` (CLI `status` at `cli.ts:410`) |
| The approval queue | CLI `queue`/`approve` (`cli.ts:563`, `:573`); `coordinator/proposals.ts`, `coordinator/approve.ts` |
| Flip a flag (the "launch" action) | CLI `flip` (`cli.ts:667`); `flags/file-client.ts`, `flags/http-client.ts` |
| Config schemas | `src/halyard/config/org-config.schema.ts`, `config/app-config.schema.ts` |
| Where secrets resolve | `src/halyard/secrets/resolve.ts` (`setSecretStore`, `envSecretStore`) |
| Notifications (the "push to phone" surface) | `src/halyard/publicity/notify.ts` (`FileNotifier`, `WebhookNotifier`) |
| Org/channel config example | `halyard.config.yml`; per-app `apps/aurora/app.yml` |

## Architecture

**Shape:** single TypeScript package (not a monorepo). Two delivery surfaces — a CLI and an importable ESM library — over one dependency-injected engine. No client/server split; no network listener of its own. Persistence is the local filesystem (git-backed JSON). All external integrations sit behind **ports** with safe offline defaults that auto-upgrade to live clients when secrets are present.

| Unit | Path | Purpose |
|------|------|---------|
| Public barrel | `src/halyard/index.ts` | The whole feature matrix is exported from the package root `"halyard"`; deep imports are discouraged |
| CLI | `src/halyard/cli.ts` | `halyard` command; `dispatch(args)` exported, side-effect-free on import |
| Config | `src/halyard/config/` | Zod schemas + loaders; `SecretRef`; app discovery; backend guard |
| Contracts | `src/halyard/contracts/` | Launch / Release / Proposal Zod schemas + the release-state enum |
| Coordinator | `src/halyard/coordinator/` | record store, state machine, reconcile engine, launch store, proposals queue, graduation, changelog, approve, preflight, `sources/` (ASC review + flag polls) |
| Surfaces | `src/halyard/surfaces/` | shared adapter interface + web / ios / android build-test-deploy adapters (desktop is a stub) — note: "web" = a *deploy target* (Cloudflare Pages), NOT a UI |
| Flags | `src/halyard/flags/` | `FlagClient` port + git-backed `FlagFileClient` and `HttpFlagClient` |
| Publicity | `src/halyard/publicity/` | drafters, channel gate, publishers, notifier, announce policy, fan-out, voice canon |
| Agents | `src/halyard/agents/` | Sentry triage classifier, rejection drafter, narrative-seed drafter — **all propose-only, never act** |
| Maintenance | `src/halyard/maintenance/` | cert-expiry, platform-deadline, Renovate watchers |
| Secrets | `src/halyard/secrets/` | `SECRET:NAME` → value resolution, default from `process.env`, overridable via `setSecretStore` |
| Payments | `src/halyard/payments/` | Stripe verify-only port (read-only; never moves money) |
| Licensing | `src/halyard/licensing/` | offline open-core entitlement (Ed25519-signed `HALYARD_LICENSE_KEY`, fail-safe to free) |
| State (data) | `state/` | git-backed JSON records: `launches/ releases/ proposals/ flags/ publicity/ notifications/` |
| Voice canon (data) | `canon/voice/` | accreting corpus of approved posts |
| CI/CD | `.github/workflows/` | ci, release, reconcile (cron), maintenance (cron), sentry-alert |

## Key flows

### Release run (build → test → deploy → record)
CLI `release run` → `releaseRun` (`cli.ts:119`) loads org + app config → calls `runRelease(...)` (`coordinator/release-runner.ts`) → selects a surface adapter (`surfaces/web.ts|ios.ts|android.ts`) → runs configured shell **build/test** commands via `ShellCommandRunner` (`surfaces/command-runner.ts`) → **deploy** (web = `npx wrangler pages deploy`, or copy to `local_dir`) → writes a `Release` JSON record to `stateDir`. The adapter never decides pass/fail; a deterministic gate (`coordinator/gates.ts`) does (invariant #2). Exits non-zero on a dead/failed release.

### Reconcile (the heartbeat — external truth → state transitions → side effects)
CLI `reconcile` → `reconcileRun` (`cli.ts:171`) → `buildReconcileSources(org, apps, {flagClient})` builds pollers (ASC review poll, flag poll) → `reconcile(...)` (`coordinator/reconcile.ts`) applies state transitions idempotently (every transition carries a `release_id + transition` dedup key) → then fires **graduation proposals**, **publicity fan-out** (`firePublicity`), **Sentry triage**, and **rejection drafts**. Persistently-failing pollers raise a `coordinator_error` proposal to the notifier and auto-resolve on recovery (`cli.ts:242-266`). Designed to be invoked by a cron workflow (`reconcile.yml`, `*/20`), not a long-running daemon.

### The "launch" moment (flag flip)
CLI `flip --flag <key> --state on` → `flipCmd` (`cli.ts:667`) → `FlagClient.setState(key, on)` (git-backed file client by default, `HttpFlagClient` when `HALYARD_LIVE_FLAGS` + per-app `flags.api_url` + token). The flip is a **human gate**; the next reconcile projects the release to `live`, which is what fires publicity. The system never flips a flag on its own.

### Publicity fan-out + approval queue
On the `live` transition, `firePublicity` (`publicity/trigger.ts`) drafts copy (`TemplateDrafter` default / `AnthropicDrafter` when keyed + Pro). **Owned** channels (blog, waitlist email) may auto-publish on a light gate; **third-party** channels (X, LinkedIn, HN) only **draft + stage** as `Proposal` records — the actual post stays a human action via CLI `approve` (`cli.ts:573`). Approving a `social_post` feeds the final copy into the voice canon but still does not auto-post.

## Contracts (integration surface)

### Public library exports (import from `"halyard"`)
Everything is re-exported from `src/halyard/index.ts`. High-value entry points (signatures per `docs/INTEGRATION.md`):

| Function | Purpose |
|----------|---------|
| `runRelease({app, surface, version, commit, stateDir, workdir, runner, now})` | Run a release end-to-end |
| `reconcile({stateDir, sources, now})` + `buildReconcileSources(org, apps, {flagClient})` | Pull external truth into state |
| `firePublicity({org, apps, drafter, publisher, notifier, voiceCanon, stateDir, now})` | Fan out publicity on `live` |
| `newLaunch(...)`, `writeLaunch`, `linkRelease`, `bindReleaseToLaunch` | Create/link a launch |
| `readRelease`, `readLaunch`, `listProposals`, `scanReleaseIds`, `scanLaunchIds` | Read state (validated) |
| `summarizeRelease(release, nowIso)` | Operator "why stuck?" view |
| `approveProposal({stateDir, canonDir, proposalId, finalText?, now})` | Approve a queued proposal |
| `loadOrgConfig(path)`, `loadAppConfig(path)`, `validateOrgConfig`, `validateAppConfig` | Config (take **absolute** paths) |
| `setSecretStore({get})`, `resolveSecret`, `tryResolveSecret`, `envSecretStore` | Inject a secret store |
| `dispatch(args)` | Drive the CLI surface programmatically (args = argv after node/script) |
| Ports / defaults | `FlagClient`/`FlagFileClient`/`HttpFlagClient`, `Drafter`/`TemplateDrafter`/`AnthropicDrafter`, `Publisher`/`FilePublisher`/`HttpPublisher`, `Notifier`/`FileNotifier`/`WebhookNotifier` |
| Schemas / types | `ReleaseSchema`, `LaunchSchema`, `ProposalSchema`, `ReleaseStateSchema`, `dedupKey`, and inferred TS types |

### CLI commands (`halyard <cmd>`; see `cli.ts:735` usage block)
| Command | Purpose |
|---------|---------|
| `release run --app --surface --version [--commit]` | Build → test → deploy → record (exit ≠0 on dead) |
| `reconcile [--apps]` | Poll external truth, apply transitions, fire publicity/agents |
| `launch create --app --feature --title [--narrative --tier --announce]` | Create a launch (drafts a narrative seed if omitted) |
| `launch link --launch --release` | Bind a release to a launch |
| `flip --flag --state on\|off [--app]` | The human launch/rollback gate |
| `maintenance [--apps]` | cert / deadline / Renovate watchers → queue |
| `triage [--apps]` | Out-of-band Sentry crash triage |
| `status [--stuck] [--release]` | Why each release is where it is (JSON) |
| `queue [--all]` | The approval queue (JSON; open by default) |
| `approve --proposal [--text]` | Record approval; never auto-posts |
| `payments verify [--apps]` | Read-only payment-config check |
| `preflight [--apps] [--probe off]` | Production-readiness across integrations (exit ≠0 if not ready) |
| `license` | Show resolved entitlement/tier |

**All commands print a JSON result to stdout** and human/log lines to stderr — friendly for a host that shells out and parses stdout. Exit codes are meaningful (0 ok; 1 errors/not-ready; 2 usage).

### Outbound network shapes (only the ones Halyard *calls* — it exposes none itself)
| Client | Shape | File |
|--------|-------|------|
| `HttpFlagClient` | `GET/PUT {flags.api_url}/flags/{key}` Bearer-token, body `{on:boolean}` | `flags/http-client.ts` |
| ASC review poll | App Store Connect API | `coordinator/sources/asc-client.ts`, `asc-review.ts` |
| Sentry | Sentry API | `agents/triage/sentry-client.ts` |
| Stripe | balance read (verify-only) | `payments/stripe-client.ts` |
| `WebhookNotifier` | POSTs proposals to the approval webhook URL | `publicity/notify.ts` |
| `HttpPublisher` | POSTs to owned-channel publish endpoints | `publicity/publishers.ts` |
| Anthropic | `@anthropic-ai/sdk`, model `claude-opus-4-8` | `publicity/anthropic-drafter.ts`, agents |
| web deploy | `npx wrangler pages deploy` (Cloudflare) | `surfaces/web.ts:60` |

### Data shapes (Zod schemas; read via exported readers, do not parse files yourself)
| Name | Path | Notes |
|------|------|-------|
| `ReleaseStateSchema` / `RELEASE_STATES` | `contracts/state.ts` | `tagged→built→tested→uploaded→in_review→shipped_dark→live→rolled_back`; plus `dead`, `rejected` |
| `ReleaseSchema` | `contracts/release.schema.ts` | a release record (`state/releases/*.json`) |
| `LaunchSchema` | `contracts/launch.schema.ts` | a launch record (`state/launches/*.json`) |
| `ProposalSchema` | `contracts/proposal.schema.ts` | queue items: `social_post`, `coordinator_error`, triage, rejection, graduation |
| Org config | `config/org-config.schema.ts` | coordinator backend/state_dir/cron, notifications, drafting, channel registry |
| App config | `config/app-config.schema.ts` | per-app version scheme, flags naming, surfaces, triage, channels, maintenance |

### Config & environment
| Var / port / service | Required? | Notes |
|----------------------|-----------|-------|
| **(no listening port)** | — | Halyard binds nothing; nothing to expose/iframe |
| `halyard.config.yml` (org) | yes (CLI) | resolved relative to **process CWD** by the CLI; library loaders take absolute paths |
| `apps/<slug>/app.yml` (per app) | yes (CLI) | discovered under `apps/` relative to CWD |
| `state/` dir | yes | git-backed JSON; `coordinator.state_dir` (default `./state`) |
| `ANTHROPIC_API_KEY` | optional | enables AI drafting/agents (Pro); else deterministic templates |
| `HALYARD_APPROVAL_WEBHOOK` | optional | the mobile approval surface; else `FileNotifier` |
| `HALYARD_LIVE_FLAGS` / `HALYARD_LIVE_PUBLISH` / `HALYARD_LIVE_MERGE` | optional toggles | arm live flag provider / owned-channel publish / auto-merge; unset → safe local defaults |
| iOS: `MATCH_REPO`, `MATCH_PASSWORD`, `ASC_KEY_ID`, `ASC_ISSUER_ID`, `ASC_PRIVATE_KEY` | optional | only for iOS releases |
| Android: `SUPPLY_JSON_KEY_DATA` | optional | only for Android releases |
| Web: `CLOUDFLARE_API_TOKEN`, `CLOUDFLARE_ACCOUNT_ID` | optional | only for web deploy |
| `SENTRY_AUTH_TOKEN`; owned publish `BLOG_PUBLISH_URL`, `EMAIL_SEND_URL` | optional | triage / owned channels |
| per-app flag provider token (e.g. `AURORA_FLAG_PROVIDER_KEY`) | optional | named by app's `flags.api_key_ref`; base URL is `flags.api_url` config |
| `HALYARD_LICENSE_KEY` | optional | Ed25519-signed; Pro unlocks AI agents, auto-merge, multi-app; fail-safe to free |

All secrets are `SECRET:NAME` **references** in config; raw values are rejected at load time. They resolve from `process.env` by default or a `setSecretStore({get})` you install.

## Build · run · test

(package manager: **npm** — `package-lock.json` present; no pnpm/yarn/uv lockfile)

- Install: `npm install` (the `prepare` script also runs `npm run build` → `dist/`)
- Build: `npm run build` (`tsc -p tsconfig.build.json` → ESM `dist/` + `.d.ts`) — this is what library consumers import
- Typecheck: `npm run typecheck` (`tsc -p tsconfig.json --noEmit`)
- Run CLI (dev): `npm run halyard -- <args>` or `tsx src/halyard/cli.ts <args>`
- Run CLI (prod/installed): `halyard <args>` (bin → `dist/cli.js`)
- Local end-to-end demo (no accounts): `npm run demo` (`scripts/demo.ts`, runs the spine in a temp dir)
- Test: `npm test` (`vitest run`) · `npm run test:watch` · `npm run test:coverage` — large offline suite under `tests/` (~70 specs). *(commands read from package.json; not executed in this digest — `(unverified)` that they pass on this machine)*

There is **no `dev` / `start` / `serve` script and no dev-server port** — re-confirming this is not a web app.

## Gotchas & invariants

- **No UI / no server = cannot be iframed or webview-loaded.** This is the headline integration constraint. Halyard produces JSON and writes files; it renders nothing. A command-center plugin must (a) `import` the library and build a Svelte view over `readRelease`/`listProposals`/`summarizeRelease`/`approveProposal`, or (b) spawn the `halyard` CLI from the Tauri (Rust) side and parse stdout JSON. Option (a) is cleaner: the engine is fully DI'd and side-effect-free on import.
- **CLI resolves config paths relative to `process.cwd()`** (`cli.ts` uses `resolve("apps")`, `resolve("halyard.config.yml")`). If you spawn the CLI, set CWD to the Halyard config root. If you import the library, pass **absolute** paths to `loadOrgConfig`/`loadAppConfig` (per `INTEGRATION.md` "Known limitations").
- **State is git-backed JSON on the local filesystem**, not a database or service. The plugin and Halyard must agree on a `stateDir`. Concurrent writers rely on git rebase-retry (`scripts/commit-state.sh`) — a desktop host writing the same dir as a CI workflow could conflict. Treat the host as the reader/operator, let the engine own writes.
- **No auth layer, no multi-tenant model, no origin assumptions.** It trusts its caller completely (it is a local tool). That makes it *easy* to embed trust-wise, but it means **you must not expose its functions to untrusted/sandboxed plugin code** — there is no permission boundary inside Halyard. Any sandboxing must be the host's responsibility.
- **`zod` is a regular dependency.** If the command center also uses zod and passes schemas across the boundary, dedupe to one `zod` instance to avoid dual-instance type mismatches (`INTEGRATION.md`).
- **Deterministic gates are non-negotiable invariants, not adapter behavior:** no model ever decides ship/promote/flip/post (#2); third-party posts never auto-publish (#5). A host cannot wire around these — agents only draft/classify into the queue; humans approve; deterministic code executes.
- **Safe-by-default degradation:** when a live toggle/secret is unset, the client falls back to a local default (git flags, file notifier, template drafter, dry-run merge). So an embedded instance with no secrets is harmless and fully functional offline — good for a default plugin experience.
- **Open-core gating:** AI agents, auto-merge, and multi-app are Pro features gated on `HALYARD_LICENSE_KEY` (fail-safe to free). Multi-app *acting* commands (reconcile/maintenance/triage) hard-gate on >1 app; read-only diagnostics (status/preflight/payments) stay free. A free-tier embed silently uses templates instead of LLM drafts.
- **`design.md` is the declared source of truth**; the README is the operator manual. Read `design.md` before changing engine semantics.

## Open questions / unverified

- Did **not** execute `npm test` / `npm run build` / `npm run demo` on this machine — commands are pulled from `package.json`, pass status `(unverified)`.
- Did not read `design.md` in full, nor the ios/android surface adapters, the individual schema field shapes, or the agent/maintenance internals — out of scope for an *integration* digest. Their exported names and roles are captured above; field-level shapes live in the cited `*.schema.ts` files.
- `desktop` surface is described as a "stub" in the README; not verified what it does today (likely a no-op adapter).
- Whether the command center should embed via library import vs. CLI spawn is a host design decision; both are supported (`dispatch(args)` is exported and import is side-effect-free). No recommendation is binding here.
