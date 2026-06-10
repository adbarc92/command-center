# Handoff — Build a web "head" for Halyard (to make it a Command Center app plugin)

> For the agent picking this up. Self-contained brief.
> Date: 2026-06-07. Author: app-plugins design session.

## Why you're here

The **Command Center** (`d:\MajorProjects\CURRENT\command-center`) is gaining an
**app-plugin runtime**: it hosts other first-party web apps as plugins by launching their
backend, embedding their live UI as a **Tauri child webview** pointed at a URL, and
switching between them from one shell. See the app-plugins design spec (being written) at
`docs/superpowers/specs/2026-06-07-app-plugins-design.md`.

The runtime requires that **every app plugin has a "head": a web app served at a URL.**
**Halyard is headless today** — it's a Node/TS CLI + importable library over git-backed
JSON state, with **no UI, no web server, no port, no `index.html`**. So Halyard cannot be a
plugin until it grows a web head. **That is your task: build that head.**

## Read these first

- **Halyard codebase digest** (authoritative, agent-oriented):
  `d:\MajorProjects\CURRENT\command-center\docs\digests\halyard-digest.md`. Read it in full —
  it maps Halyard's library exports, CLI, state model, config, and integration constraints.
- **Halyard repo:** `D:\MajorProjects\INFRASTRUCTURE\halyard` (its `design.md` is the
  declared source of truth; `docs/INTEGRATION.md` documents the library surface).
- **App-plugins design spec** (the contract your head must satisfy):
  `docs/superpowers/specs/2026-06-07-app-plugins-design.md` in the command-center repo.

## What "a head" must be (the app-plugin contract)

Your head must satisfy the Command Center app-plugin contract:

1. **A web app reachable at a URL** (e.g. `http://localhost:<port>`), renderable in a
   webview — a normal SPA/SSR app is fine. It is **trusted first-party** code (real origin,
   network, cookies allowed) — no sandbox constraints to fight.
2. **A start command** the host can run to bring it up (the host uses `tauri-plugin-shell`
   to run it), e.g. an `npm run` script or a small binary. Startup must be non-interactive.
3. **A health check** the host can poll to know it's ready (an HTTP endpoint returning 200,
   e.g. `GET /health`), so the host shows a loading state until healthy, then loads the URL.
4. **A clean stop** (process terminates on signal; no orphaned children).
5. It **owns its own viewport** (the host gives it the full content rect) and should route
   from its own root — no assumption about being mounted under a sub-path.

(The exact manifest field names will be in the app-plugins spec; build the head so these are
straightforward to declare. Don't block on the manifest — these five properties are stable.)

## What the head should actually do (scope)

Surface Halyard's **operator workflow** — the same things its CLI exposes, as a UI. Prioritize
the human-gated actions, since Halyard's whole point is human approval gates:

- **Release status board** — "why is each release where it is." Back it with
  `summarizeRelease` / `status` (digest: `coordinator/status.ts`, CLI `status`).
- **Approval queue** — list queued proposals and **approve** them (CLI `queue` / `approve`;
  `coordinator/proposals.ts`, `coordinator/approve.ts`). This is the highest-value screen.
- **Flag flip** — the launch/rollback human gate (CLI `flip`; `flags/*`).
- **Launches / releases browse** — read-only views over the state records.
- Surface notifications/errors that Halyard raises (the `coordinator_error` proposals).

Keep it lean and operator-focused; this is a control surface, not a redesign of Halyard.
Defer anything not in the CLI surface.

## How to build it (recommended, not mandatory)

The digest's "recommended embedding strategy" applies. Two clean options:

- **Library-backed (preferred):** a small Node service that `import`s Halyard
  (`import { ... } from "halyard"` — the library is **side-effect-free on import and fully
  dependency-injected**), exposes a thin **JSON/REST API** over the readers/actions
  (`readRelease`, `listProposals`, `summarizeRelease`, `approveProposal`, flag clients,
  `loadOrgConfig`/`loadAppConfig`), and serves a frontend. Cleanest; type-safe; no stdout
  parsing.
- **CLI-backed (fallback):** spawn the `halyard` CLI (`dispatch(args)` is exported) and parse
  its **stdout JSON** (every command prints a JSON result; exit codes are meaningful). Simpler
  to start, clumsier long-term.

**Frontend framework:** your call. Svelte 5 matches the Command Center, but the head is its
own app behind a URL, so anything works. Keep it small.

## Constraints & gotchas (from the digest — these will bite you)

- **Config path resolution:** the CLI resolves config relative to `process.cwd()`; the
  **library loaders take absolute paths.** If you import the library, pass absolute paths to
  `loadOrgConfig`/`loadAppConfig`. Decide and document the Halyard config root your head uses.
- **State is git-backed JSON on the local filesystem** (a `stateDir`), not a DB/service. Your
  head and Halyard must agree on `stateDir`. Let the engine own writes; treat the head as the
  operator/reader plus the approve/flip actions. Concurrent writers rely on git rebase-retry.
- **No internal auth/permission boundary** — Halyard trusts its caller completely. Your head
  is trusted first-party, which is fine, but **don't expose Halyard's functions to untrusted
  callers.** Bind the head's server to localhost.
- **Deterministic gates are invariants:** no model decides ship/flip/post; third-party posts
  never auto-publish. Your UI **proposes/approves**, it does not bypass these gates.
- **`zod` is a Halyard dependency** — if you share schemas across a boundary, dedupe to one
  `zod` instance to avoid dual-instance type mismatches.
- **Safe offline defaults:** with no secrets set, Halyard runs offline with local defaults
  (git flags, file notifier, template drafter). Good for a credential-free dev head.
- **Open-core gating:** AI agents / auto-merge / multi-app are Pro (gated on
  `HALYARD_LICENSE_KEY`, fail-safe to free). A free-tier head should degrade gracefully.

## Definition of done

- Halyard exposes a web head: a URL serving an operator UI, with a documented **start
  command**, a **health endpoint**, and a clean **stop**.
- The head covers: release status board, approval queue + approve, flag flip, and read-only
  launches/releases browse.
- It runs **credential-free in dev** (offline defaults) and reads/writes the agreed
  `stateDir`.
- README/INTEGRATION updated with: how to start it, the port, the health endpoint, the config
  root, and the `stateDir` it expects — i.e. exactly what the Command Center manifest needs.
- Tests for the new server/API layer; existing Halyard tests stay green.
- Branch off Halyard's default; do not push to its main without asking the owner.

## Suggested skills

- **superpowers:brainstorming** — briefly, to settle the head's API shape + framework before building.
- **superpowers:writing-plans** then **superpowers:test-driven-development** — plan + build it.
- **frontend-design:frontend-design** — for the operator UI.
- **superpowers:verification-before-completion** — run it, show it healthy + serving, before claiming done.
