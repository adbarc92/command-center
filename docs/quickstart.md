# Command Center — Day-1 Quickstart

> From a fresh clone to a **dispatched demo unit** in ~10 minutes — then a real T1
> mission, resuming a halted unit, and troubleshooting. Every command here is
> cross-checked against the real code: the npm scripts in
> [`cockpit/ui/package.json`](../cockpit/ui/package.json), the HTTP endpoints in
> [`crates/fleetd/src/server.rs`](../crates/fleetd/src/server.rs), and the env
> template in [`.env.example`](../.env.example).
>
> New here? Read [`docs/command-center-vision.md`](./command-center-vision.md)
> first for the *why*. This doc is the *how*.

**The fastest path to "it works":** the **demo mission** (Step 4) needs **no API
key and no Docker** — the daemon plays scripted agent output through a fake runner,
meters `$0`, and emits the same event stream the cockpit renders for a real run.
Start there to confirm your build before spending tokens.

---

## 1. Prerequisites

| Need | Why | Required for |
|------|-----|--------------|
| **Rust** (stable) + Cargo | builds the `fleetd` daemon + sidecar | always |
| **Node.js 20+** & npm | builds + runs the Tauri cockpit | always |
| **Docker** (Desktop or Engine) | isolates the real agent container | **real** missions only |
| **`gh` CLI**, authenticated | the daemon opens PRs via GitHub | **real** missions only |
| **`ANTHROPIC_API_KEY`** | pays for the agent model calls | **real** missions only |

**Demo mode needs none of Docker / `gh` / the key** — only Rust + Node.

Check Docker and `gh` before a real run:

```bash
docker version          # daemon up?  the cockpit also probes this via /health
gh auth status          # logged in?  the daemon shells out to gh to open the PR
```

A spend-capped key matters. From [`.env.example`](../.env.example): create the key
inside an Anthropic **Workspace** with a low monthly spend limit (Console → Settings
→ Workspaces) — that workspace cap is your hard external backstop, on top of the
daemon's per-unit `CC_USD_CAP` and `CC_WALL_SECS`.

---

## 2. Build

```bash
git clone https://github.com/adbarc92/command-center
cd command-center

# 2a. Build the workspace (the fleetd daemon + the `serve` binary the cockpit talks to)
cargo build --release

# 2b. Install the cockpit's frontend deps
cd cockpit/ui
npm install
```

### The agent image (real missions only)

The real runner launches a container from **`cc-agent:dev`**
(see [`crates/fleetd/src/bin/serve.rs`](../crates/fleetd/src/bin/serve.rs), which
defaults `CC_IMAGE` to `cc-agent:dev`). On the maintainer's machine this image is
already built. On a fresh machine, build it from
[`deploy/agent-image/Dockerfile`](../deploy/agent-image/Dockerfile):

```bash
# from the repo root
docker build -t cc-agent:dev deploy/agent-image
docker images cc-agent          # confirm cc-agent:dev exists
```

That image is just `node:22-slim` + `git` + the Claude Code CLI, running as the
non-root `node` user. **Demo missions never touch Docker**, so you can skip this
entirely until you want a real run.

---

## 3. Launch the app

The cockpit is a Tauri desktop app that talks to the `fleetd` daemon over
`http://127.0.0.1:8787`. There are two ways to run it.

### Option A — one command (recommended)

From `cockpit/ui`:

```bash
npm run desktop
```

Per [`cockpit/ui/package.json`](../cockpit/ui/package.json), `desktop` is
`npm run sidecar && tauri dev`:

- **`npm run sidecar`** runs [`scripts/build-sidecar.mjs`](../cockpit/ui/scripts/build-sidecar.mjs),
  which `cargo build`s the `serve` binary and copies it to
  `src-tauri/binaries/fleetd-serve-<target-triple>` so Tauri can bundle it as a
  sidecar.
- **`tauri dev`** then launches the desktop window.

> **Note on sidecar supervision:** automatic spawn/health-gate/restart of the
> bundled daemon is **Lane L1** (roadmap A2). Until that lands, if the window opens
> but shows no daemon, use Option B to run `serve` yourself in a second terminal.

### Option B — daemon + dev UI separately (works today, no Tauri host needed)

Terminal 1 — start the daemon:

```bash
# from the repo root
./target/release/serve            # Windows: .\target\release\serve.exe
# → fleetd listening on http://127.0.0.1:8787 (db: fleet.db)
```

Terminal 2 — start the cockpit dev server:

```bash
cd cockpit/ui
npm run dev                       # vite dev server; UI talks to 127.0.0.1:8787
```

The daemon binds `127.0.0.1:8787` (override with `CC_ADDR`) and persists to
`./fleet.db` (override with `CC_DB`). It auto-loads a `.env` from the repo root if
present — see Step 5.

Confirm the daemon is healthy:

```bash
curl http://127.0.0.1:8787/health
# → {"docker":false,"anthropic_key":false,"version":"..."}  (booleans reflect your setup)
```

---

## 4. Dispatch a demo mission (no key, no Docker, $0)

`mode` defaults to `"demo"`, so a bare mission is already a safe, free dry run.
Demo mode plays a scripted oracle → build → check → review cycle through the fake
runner and meters `$0`.

From the cockpit: fill in a task, leave the mode on **demo**, dispatch — then watch
the unit walk its phases in the fleet view.

Or with `curl` against the real `POST /missions` endpoint:

```bash
curl -s -X POST http://127.0.0.1:8787/missions \
  -H 'content-type: application/json' \
  -d '{"task":"add a sum() helper with tests","tier":"t1","mode":"demo","min_review_rounds":1}'
# → {"unit_id":"u1"}
```

Field reference (from `CreateReq` in `server.rs`):

| Field | Default | Notes |
|-------|---------|-------|
| `task` | — (required) | what the agent should do |
| `tier` | `t1` | `t1` \| `t2` \| `t3` |
| `mode` | `demo` | `demo` \| `real` |
| `min_review_rounds` | `2` | floored to ≥ 1 |

Watch it progress (poll the snapshot, or open the WebSocket stream):

```bash
curl -s http://127.0.0.1:8787/units                 # all units (summaries)
curl -s http://127.0.0.1:8787/units/u1              # one unit: phase + full event log
curl -s "http://127.0.0.1:8787/units/u1/events?since=0"   # raw events after a seq
```

A demo unit drives to a terminal phase with `$0` cost and **no Docker call**. If you
reached `{"unit_id":"u1"}` and see it advancing in `/units/u1`, the build is good.

---

## 5. Dispatch a real T1 mission (needs `ANTHROPIC_API_KEY` + Docker)

Real missions launch a `cc-agent:dev` container, run the model, and open a real PR.

**5a. Set the key.** Copy the template and fill it in (`serve` auto-loads `.env`
from the repo root):

```bash
# from the repo root
cp .env.example .env
# edit .env → set ANTHROPIC_API_KEY=sk-ant-...
```

Optional knobs in `.env` (defaults shown in the template): `CC_REPO_URL`,
`CC_REPO_SLUG`, `CC_BASE_BRANCH`, `CC_TEST_CMD`, `CC_IMAGE`, `CC_USD_CAP`,
`CC_WALL_SECS`, `CC_MIN_ROUNDS`, `CC_ADDR`.

**5b. Confirm prerequisites are live:**

```bash
docker version          # must succeed — the real runner needs it
gh auth status          # must be logged in — used to open the PR
curl http://127.0.0.1:8787/health
# → {"docker":true,"anthropic_key":true,"version":"..."}
```

> The daemon reads `ANTHROPIC_API_KEY` from its own environment at request time. If
> you put it in `.env`, **restart `serve`** so it loads. `POST /missions` with
> `mode:"real"` returns **400 `ANTHROPIC_API_KEY not set`** if the key is missing.

**5c. Dispatch:**

```bash
curl -s -X POST http://127.0.0.1:8787/missions \
  -H 'content-type: application/json' \
  -d '{"task":"add a sum() helper with tests","tier":"t1","mode":"real","min_review_rounds":1}'
# → {"unit_id":"u2"}
```

The unit provisions a container, generates + freezes a test oracle, runs the
build/check/review loop under hard caps, and opens a verified-mergeable PR against
the configured sandbox repo. Cost accrues against the per-unit cap; the daemon also
enforces a rolling-24h global spend ceiling (`CC_GLOBAL_USD_CAP`, default `$20`) and
returns **429 `global daily cost cap reached`** if you exceed it.

> First real run? Watch it. This is the one path that spends real tokens.

---

## 6. Resume a halted unit

A unit can `Halt` (you halted it, or the daemon halted it — e.g. after a restart,
startup reconciliation marks stranded units `halted`). Bring it back with a
**`resume`** command to `POST /units/:id/commands`. The daemon rehydrates a
store-only unit into memory first, so this works even after a daemon restart.

From the cockpit: the unit's detail rail exposes the resume action. Or by hand:

```bash
curl -s -X POST http://127.0.0.1:8787/units/u2/commands \
  -H 'content-type: application/json' \
  -d '{"command":"resume","cmd_id":"r1"}'
# HTTP 202 Accepted  (command queued to the unit's driver)
```

The command body is a `fleet_core::Command` — serde-tagged on the `command` field
(`event.rs`: `#[serde(tag = "command", rename_all = "snake_case")]`). Other valid
commands: `halt`, `abandon`, `ship` (the T3 final gate), `approve_oracle`,
`reject_oracle` — each with a client-generated `cmd_id`.

Status codes from `POST /units/:id/commands`:

- **202** — accepted, queued to the driver.
- **404** — no such unit (never existed / not in the store).
- **410** — the unit's driver is gone (already terminal); the command can't run.

A resumed **real** unit reuses its kept volume and skips the already-frozen oracle;
a resumed **demo** unit replays its fake script to completion (demo units survive a
restart only in-memory — they aren't meant to outlive the daemon).

---

## Endpoint reference

Every endpoint the cockpit and this guide use, straight from
[`crates/fleetd/src/server.rs`](../crates/fleetd/src/server.rs):

| Method & path | Purpose |
|---|---|
| `POST /missions` | dispatch a unit → `{ unit_id }` |
| `GET /units` | all units (summaries) |
| `GET /units/:id` | one unit: phase + full event log |
| `GET /units/:id/events?since=<seq>` | events after a seq |
| `POST /units/:id/commands` | send a control command (resume/halt/…) → 202 |
| `GET /units/:id/stream?since=<seq>` | WebSocket: replay since, then live |
| `GET /health` | `{ docker, anthropic_key, version }` |
| `POST /swarms` · `GET /swarms` · `GET /swarms/:id` | multi-lane swarm orchestration |

---

## 7. Troubleshooting

**`/health` shows `"docker":false` / real mission fails to provision.**
Docker isn't running or isn't reachable. Start Docker Desktop / the engine, confirm
`docker version` succeeds, then retry. The daemon caches the Docker probe for 5s, so
give it a moment after starting Docker. Demo missions don't need Docker at all.

**`POST /missions` returns 400 `ANTHROPIC_API_KEY not set`.**
You dispatched `mode:"real"` without a key. Set `ANTHROPIC_API_KEY` in `.env` (or the
daemon's environment) and **restart `serve`** — it reads `.env` only at startup. Or
dispatch with `mode:"demo"` to try the pipeline for free.

**`POST /missions` returns 400 `unknown mode: …`.**
`mode` must be exactly `demo` or `real`.

**`POST /missions` returns 429 `global daily cost cap reached`.**
Committed + in-flight spend over the rolling 24h hit `CC_GLOBAL_USD_CAP` (default
`$20`). Wait for the window to roll, raise the cap via env, or let in-flight units
finish.

**`bind 127.0.0.1:8787: …` — daemon won't start (port in use).**
Another `serve` (or process) holds 8787. Find and stop it, or bind elsewhere:

```bash
# Windows
netstat -ano | findstr :8787          # note the PID, then: taskkill /PID <pid> /F
# macOS / Linux
lsof -i :8787                         # then: kill <pid>

# or run on a different port (tell the cockpit too, via VITE_FLEET_URL)
CC_ADDR=127.0.0.1:8799 ./target/release/serve
```

The cockpit's daemon URL is `VITE_FLEET_URL` (defaults to `http://127.0.0.1:8787` —
see [`cockpit/ui/src/lib/api.ts`](../cockpit/ui/src/lib/api.ts)); set it to match a
non-default `CC_ADDR`.

**`npm run desktop` opens a window but nothing loads / no daemon.**
Automatic sidecar supervision is Lane L1 (not yet merged). Run the daemon yourself
(Step 3, Option B) in a separate terminal: `./target/release/serve`.

**`npm run sidecar` fails copying the binary.**
The script runs `cargo build -p fleetd --bin serve` and copies from
`target/debug/serve`. Make sure `cargo` and `rustc` are on `PATH`; re-run from inside
`cockpit/ui`.

---

## Useful npm scripts (`cockpit/ui`)

From [`cockpit/ui/package.json`](../cockpit/ui/package.json):

| Script | Does |
|---|---|
| `npm run dev` | Vite dev server for the UI alone |
| `npm run build` | production frontend build |
| `npm run sidecar` | build `serve` + stage it as the Tauri sidecar |
| `npm run desktop` | `sidecar` then `tauri dev` (the full app) |
| `npm run tauri` | the raw Tauri CLI passthrough |
| `npm run check` | `svelte-check` + `tsc` typecheck |
| `npm run test` | `vitest run` |
