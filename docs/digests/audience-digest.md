# Audience — Codebase Digest (for agents)

> Audience: an agent integrating Audience as an **app plugin** inside a Tauri v2 + Svelte 5 "command center" dashboard.
> Source: `fbd18095c83452386d8a7d566f1c88bc1e99fa59` · 2026-06-02 · branch `feat/local-bringup` · digested by reading ~25 files in `D:\MajorProjects\CURRENT\audience` (manifests, compose files, web app, api service, contracts; service/worker internals inferred from entry-point greps + the authoritative `docs/Status-2026-06-02.md` and `CLAUDE.md`).
> Purpose of this digest: **integration** (embed-as-plugin). Emphasis on how it runs, its UI entry point, backend dependencies, public surface, and embedding friction.

## TL;DR
Audience is a **pnpm + Turborepo TypeScript monorepo + a `uv` Python workspace** implementing a social-media content tool: AI text/video generation, model selection, multi-platform scheduling, and an approve-before-post human gate. The user-facing UI is a **Next.js 14 App Router web app** (`apps/web`, dev on `:3000`) — a server-rendered React/Tailwind site, **not** a static SPA and **not** a Tauri/Svelte app. It is emphatically **not a pure frontend**: the web app is a thin shell over a REST API (`services/api`, Hono on `:8080`) backed by **Postgres + Redis (BullMQ queues) + S3/MinIO** and four more backend services/workers (orchestrator, publishing, notifications, ai_content, video). To embed it you must run that whole backend stack; the web UI is the only thing loadable in a webview/iframe, and it has several root-path / origin / cookie-auth assumptions that complicate sandboxed embedding (see Gotchas). The single most important fact: **the web app is just a presentation layer — it does nothing useful without ~6 backend processes + 3 infra services running.**

## Where to look (navigation index)
| I need to… | Go to |
|------------|-------|
| Find the user-facing UI / what to load in a webview | `apps/web/` (Next.js, `next dev -p 3000`) |
| See the web UI's routes/pages | `apps/web/app/*/page.tsx` (`/dashboard`, `/compose`, `/queue`, `/calendar`, `/onboarding`, `/posts/[id]`) |
| Understand how web talks to backend | `apps/web/lib/api-client.ts`, `apps/web/lib/api-types.ts` (generated OpenAPI types) |
| Change the backend API the web hits | `services/api/src/index.ts` (Hono app) + `services/api/src/routes/*` |
| Understand auth | `services/api/src/auth.ts` (Clerk JWT in prod, `devAuth` env-stub in dev) |
| See the OAuth popup flow (social connect) | `apps/web/lib/oauth.ts` |
| See billing / Stripe checkout | `services/api/src/billing/*` |
| Find all backend services/workers | `services/`, `workers/` |
| Understand the event contract (queues, schema) | `packages/contracts/src/index.ts` (`QUEUES`, `SCHEMA_VERSION`) |
| Run the whole stack | `docker-compose.prod.yml` (+ `docker-compose.yml` for infra-only) |
| Required env vars | `.env.example`, `.env.production.example` |
| Authoritative project state / known gaps | `docs/Status-2026-06-02.md`, `CLAUDE.md` |
| Deploy / secrets runbooks | `docs/DEPLOY.md`, `docs/SECRETS.md` (referenced; not read here) |

## Architecture
**Shape:** contract-first monorepo / service-mesh. One frontend (`apps/web`) + 4 TS backend services + 2 Python workers + 2 shared packages, all communicating **only** via `packages/contracts` over REST (OpenAPI) and **BullMQ events on Redis**. Postgres uses **schema-per-service**. The web app never talks to the queues — it only calls `services/api` over HTTP.

| Unit | Path | Kind / Port | Purpose |
|------|------|-------------|---------|
| **web** | `apps/web` | Next.js 14, HTTP `:3000` | The only user-facing UI. App Router, React 18, Tailwind, Radix UI. SSR + client. **This is what a webview would load.** |
| **api** | `services/api` | Hono (`@hono/node-server`) `:8080` | Public REST surface the web app consumes. Auth (Clerk), posts/accounts/credit/media/brand-voice/notifications routes, Stripe billing. Also runs billing queue workers + expiry sweep in-process. |
| **orchestrator** | `services/orchestrator` | **No HTTP** — BullMQ worker | Saga engine. Consumes ~13 queues (compose/approve/credit/generation/publish/reminder). Owns `orchestrator.posts` / `post_targets` (which `api` cross-schema-reads). |
| **publishing** | `services/publishing` | Webhook server `:8081` (`WEBHOOK_PORT`) | Publishes to Bluesky/Mastodon/Threads + Ayrshare; ingests provider webhooks; token storage. |
| **notifications** | `services/notifications` | Hono `:8083` | Confirm/remind/reconnect notifications. |
| **ai_content** | `workers/ai_content` | **No HTTP** — Python BullMQ worker | Text generation + model selection (`main.py`, `queue.py`). |
| **video** | `workers/video` | **No HTTP** — Python BullMQ worker | Runway video gen, ffmpeg, media registry, S3/MinIO (`consumer.py`). |
| **contracts** | `packages/contracts` | library | **The seam.** JSON Schema → generated Zod + OpenAPI/Prism mock; mirrored to `workers/audience_contracts` (Pydantic). Exports `QUEUES`, `SCHEMA_VERSION=1`. |
| **shared** | `packages/shared` | library | ids, logger, error taxonomy, BullMQ helpers (`makeQueue`/`makeWorker`, lazy Redis), AES-GCM token crypto, signed-URL issuer. |
| **e2e** | `e2e/` | test suite | 3 cross-stack E2E suites (need infra up). |

**7 Dockerized deployables:** web, api, orchestrator, publishing, notifications, ai_content, video (all build in `docker-compose.prod.yml`; all 7 images verified building per Status doc).

## Key flows
### Compose → generate → approve → publish (the core product loop)
1. Browser POSTs to web's UI; web calls **`POST /posts`** → `services/api/src/routes/posts.ts:139`. Body `{text, platforms[], brandVoiceId?, generate}`.
2. api enqueues `audience.post.compose.requested` (no DB write of its own) → returns `202 {id, status: "draft"|"generating"}`. `posts.ts:158`.
3. **orchestrator** consumes it (`services/orchestrator/src/index.ts:74`), creates the Post row, reserves credit, drives generation saga (creditReserved → `content.generate.requested` → ai_content worker → `generate.completed` → `approval.requested`).
4. Web's dashboard/queue pages SSR-read state via **`GET /posts`** / **`GET /posts/:id`** (`posts.ts:83`/`:109`) — these **cross-schema read `orchestrator.posts`** directly (documented v1 shortcut).
5. User approves: **`POST /posts/:id/approve`** (`posts.ts:173`) → X-cost-consent gate (402 if `platforms` includes `x` w/o `xCostConsent`) → enqueues `audience.post.approved`.
6. orchestrator consumes `post.approved` (`index.ts:170`) → dispatches publish → **publishing** service posts to platforms; results flow back via `published`/`failed` queues.

### Auth (every request except `/health` and Stripe webhook)
- Web reads the **Clerk `__session` cookie** from `document.cookie` and attaches it as `Authorization: Bearer <token>` (`apps/web/lib/api-client.ts:19-31`).
- api middleware: `NODE_ENV==="production"` → `clerkAuth` (verifies JWT, upserts workspace+user) ; otherwise → `devAuth` which fabricates an identity from `DEV_WORKSPACE_ID`/`DEV_USER_ID` env vars (`services/api/src/auth.ts:56,109`; switched in `index.ts:42`). **No real login UI exists in `apps/web`** — auth assumes Clerk is mounted at the host level and a `__session` cookie is present.

### Social account connect (OAuth popup)
- `apps/web/lib/oauth.ts`: calls `GET /accounts/{platform}/oauth/start`, then **`window.open(...)` a 600×700 popup**, then polls `GET /accounts` every 2s for 120s. Popup-based — a constrained webview may block/handle popups differently.

## Contracts (integration surface)
### HTTP API — `services/api` (base `:8080`, all JSON, all auth-gated except noted)
| Method | Path | Purpose | Defined in |
|--------|------|---------|------------|
| GET | `/health` | liveness, **no auth** | `services/api/src/index.ts:35` |
| POST | `/billing/webhook` | Stripe webhook, **no auth** (before middleware) | `billing/stripeWebhookRoute.ts:30` |
| GET | `/posts` `?status=&limit=` | list posts (reads orchestrator schema) | `routes/posts.ts:83` |
| GET | `/posts/:id` | post + targets detail | `routes/posts.ts:109` |
| POST | `/posts` | compose / generate (202) | `routes/posts.ts:139` |
| POST | `/posts/:id/approve` | approve (X-cost gate → 402) | `routes/posts.ts:173` |
| POST | `/posts/:id/reject` | reject | `routes/posts.ts:207` |
| PUT | `/posts/:id/schedule` | set scheduledAt + timezone | `routes/posts.ts:225` |
| POST | `/posts/:id/confirm` | human-confirm manual publish (IG) | `routes/posts.ts:243` |
| POST/GET | `/accounts` | connect / list social accounts | `routes/accounts.ts:41,62` |
| GET | `/accounts/{platform}/oauth/start` | OAuth start URL (popup) | (referenced by `lib/oauth.ts`) |
| POST/GET | `/brand-voice` | create / list brand voices | `routes/brandVoice.ts:15,33` |
| GET | `/credit/balance` | credit balance | `routes/credit.ts:30` |
| POST | `/credit/grant` | grant credits | `routes/credit.ts:53` |
| POST | `/media/generate` | enqueue image/video gen | `routes/media.ts:17` |
| GET | `/media/:id` | media asset status/URL | `routes/media.ts:38` |
| GET/PATCH | `/notifications`, `/notifications/:id/read` | list / mark read | `routes/notifications.ts:8,15` |
| POST | `/billing/checkout` | Stripe checkout session (returns URL to redirect to) | `billing/checkoutRoute.ts:79` |
| GET | `/billing/config` | Stripe publishable config | `billing/checkoutRoute.ts:131` |

Other services expose HTTP too but are **not** consumed by the web app: publishing webhook server `:8081`, notifications `:8083`. orchestrator + both Python workers expose **no HTTP** (pure queue consumers).

### Event/queue contract — `packages/contracts/src/index.ts`
`QUEUES` (BullMQ on Redis, names frozen, `index.ts:74`): `audience.post.approved`, `…compose.requested`, `…schedule.requested`, `…post.rejected`, `…content.generate.requested/completed`, `…media.generate.requested/completed/failed`, `…credit.reserve.requested/reserved/denied/commit/refund`, `…account.connected/token_expiring/reconnect_required`, `…posttarget.publish.requested/published/failed/reminder_due/confirmed`, `…brandvoice.create.requested`, `…channelprefs.updated`. `SCHEMA_VERSION = 1` (changes are additive-only; generated Zod/Pydantic artifacts are committed + drift-guarded). **The web app does not touch queues** — irrelevant to a UI-only embed, but defines how the backend services interlock.

### Config & environment (from `.env.example`)
| Var / port / service | Required? | Notes |
|---|---|---|
| `DATABASE_URL` | yes | Postgres. Dev maps **`:55432`** (host pg occupies 5432–5434); prod compose uses `:5432`. Schema-per-service. |
| `REDIS_URL` | yes | `:6379`. BullMQ queue seam — backend won't function without it. |
| `STORAGE_ENDPOINT_URL`/`STORAGE_BUCKET`/`AWS_*` | yes for media | MinIO in dev (`:9000`/console `:9001`), S3 in prod. |
| `CLERK_SECRET_KEY`, `CLERK_JWT_KEY` | prod yes | Auth provider. Dev bypass via `devAuth`. |
| `DEV_WORKSPACE_ID`, `DEV_USER_ID` | dev yes | Fabricated identity when `NODE_ENV!=="production"`. |
| `TOKEN_ENC_KEY` | yes | AES-GCM static key. **🔒 HARD GATE: must become KMS-envelope before real users** (per Status doc). |
| `RUNWAY_API_KEY` (+ model/credit vars) | yes (fails fast) | Video gen. Tests pass `rw-test`. |
| `STRIPE_SECRET_KEY`/`PUBLISHABLE`/`WEBHOOK_SECRET` + `STRIPE_PRICE_*` | for billing | Payments → credit ledger. |
| `FRONTEND_URL` | billing | Default `http://localhost:3000`; used for Stripe `success_url`/`cancel_url` (`checkoutRoute.ts:107`). |
| `AI_PROVIDER=fake` / `MEDIA_PROVIDER=fake` | optional | **Deterministic, no-network providers** (CI/E2E). Useful for a demo embed without real LLM/Runway keys. |
| `API_URL` (SSR) / `NEXT_PUBLIC_API_URL` (browser) | web build | **Inlined at BUILD time** by `next.config.mjs` (`env:`), default `http://localhost:8080`. Changing the API origin requires a rebuild, not a runtime env. |
| `PORT` | web/api | Web `:3000`, api `:8080`. Note compose quirk: shared `.env` sets `PORT=8080`; web overrides to `3000`. |

## Build · run · test
Package manager: **pnpm 9.12.0** (`pnpm-lock.yaml`, `packageManager` field) for TS; **`uv`** for Python (`workers/`). Turborepo orchestrates (`turbo.json`: only `build` + `test` tasks).
- Infra only: `docker compose up -d` (Postgres `:55432`, Redis `:6379`, MinIO `:9000/:9001`).
- Install: `pnpm install` ; `(cd workers && uv sync)`.
- Web dev: `pnpm --filter @audience/web dev` → Next.js on **`:3000`** (script `next dev -p 3000`).
- API dev: `pnpm --filter @audience/api dev` (`tsx src/index.ts`) → **`:8080`**. Each service has its own `dev`/start.
- Full stack (prod-style): `docker compose -f docker-compose.prod.yml up --build` (needs `.env`; builds all 7 images + infra).
- Build: `pnpm build` (`turbo run build`; web → Next.js `output: "standalone"`, served via `node apps/web/server.js`).
- Test: `pnpm test` (turbo) ; per-pkg `pnpm --filter @audience/<pkg> test` (vitest) ; Python `(cd workers && RUNWAY_API_KEY=rw-test uv run pytest -q)` ; E2E `pnpm --filter @audience/e2e test` (needs infra). ~500 unit tests green per Status doc.
- **Mock-only web dev (no backend):** web has an `openapi-fetch` client + MSW handlers (`apps/web/lib/mock/handlers.ts`) and a Prism mock path — per `api-client.ts` comment, pointing `NEXT_PUBLIC_API_URL` at a Prism mock on `:4010` lets the UI run without the real stack. This is the lightest-weight way to demo the UI in a webview. *(unverified end-to-end here.)*

## Gotchas & invariants (embedding-critical)
- **Not a pure frontend.** `apps/web` is useless alone — every page SSR/CSR-fetches `services/api`, which needs Postgres + Redis + (for media) S3/MinIO, plus orchestrator/publishing/workers to actually do anything. A plugin embed must either (a) run the full backend (compose), (b) point at a hosted Audience backend, or (c) run web against the **Prism mock / MSW** for a non-functional demo.
- **API origin is baked at build time.** `next.config.mjs` inlines `API_URL`/`NEXT_PUBLIC_API_URL` via `env:` — you cannot retarget the backend with a runtime env var; you must rebuild the web image with the right build args (`docker-compose.prod.yml` passes them as `args`). Hardcoded defaults are `localhost:8080`.
- **Auth assumes a Clerk `__session` cookie on the document.** `apps/web/lib/api-client.ts` reads `document.cookie` for `__session` and sends it as a Bearer token. There is **no login screen in the web app** — it expects Clerk to be mounted at the host. In a Tauri webview at a custom scheme (`tauri://` / `app://`) or a different origin, that cookie won't exist → all authed calls 401. Dev workaround: run api with `NODE_ENV=development` (`devAuth`) so no token is needed at all.
- **Assumes it owns the whole window / root path.** Root `/` redirects to `/dashboard` (`apps/web/app/page.tsx`); `Nav` is a fixed full-height left sidebar (`w-56 h-screen`) and `layout.tsx` sets `min-h-screen` flex on `<body>`. Routing is absolute (`/dashboard`, `/compose`, …) with no `basePath` — it expects to be served at the origin root, not under a sub-path. Embedding in an iframe/webview works best if Audience gets its own full viewport.
- **OAuth + Stripe use popups / full-page redirects.** Social connect does `window.open` (`lib/oauth.ts`); Stripe checkout returns a URL the client redirects the whole window to, with `success_url`/`cancel_url` built from `FRONTEND_URL`. A sandboxed webview that blocks popups or cross-origin navigation will break account-connect and payments.
- **CORS / origin:** the Hono api registers **no CORS middleware** (greps show none). Same-origin browser calls are fine; if the webview origin differs from the api origin, browser fetches will be blocked unless CORS is added. (The web app's SSR fetches go server→server and are unaffected.)
- **Dev DB port is `55432`, not 5432** (host machine runs pg on 5432–5434). Prod compose uses 5432. `ioredis` is pinned in `pnpm-workspace.yaml` overrides (BullMQ type skew).
- **Contract is the law.** Never redeclare event/entity shapes — import `z.infer` types from `@audience/contracts` (TS) / `audience_contracts` (Python). Generated artifacts are committed + drift-guarded; regenerate with `pnpm gen:contracts`.
- **Cross-schema shortcut:** `services/api` reads `orchestrator.posts`/`post_targets` directly (v1 shortcut, documented). Fine to know; don't "fix" casually.
- **🔒 `TOKEN_ENC_KEY` is a static key** — a documented hard gate (KMS envelope) before any real-user launch.

## Recommended embedding strategy (for the command-center plugin host)
1. **Embed the web UI in a webview pointed at a running Audience web server** (`:3000`), not by importing code — it's a Next.js SSR app, not a component library. Give it a full panel/viewport (it owns its own sidebar + root routing).
2. **Stand up the backend** the plugin depends on: simplest is `docker compose -f docker-compose.prod.yml up` with `AI_PROVIDER=fake`/`MEDIA_PROVIDER=fake` and `NODE_ENV=development` (devAuth) for a credential-free demo, or wire real Clerk/Stripe/Runway/LLM keys for production.
3. **Decide auth boundary:** either (a) dev-mode `devAuth` (no Clerk, fixed workspace) for a trusted single-user shell, or (b) provision a Clerk `__session` cookie on the webview origin so `api-client.ts` finds it. There is no built-in way for the host to inject a token other than that cookie.
4. **Match origins or add CORS** if the webview origin ≠ api origin, and ensure popups (OAuth) and full-page redirects (Stripe) are permitted, or stub those flows.

## Open questions / unverified
- I did not run any commands (read-only digest); all "green tests / images build" claims come from `docs/Status-2026-06-02.md`, not re-verified here.
- The Prism-mock / MSW "run web without backend" path is asserted by code comments and the presence of `lib/mock/handlers.ts` + `codegen.ts`, but not executed end-to-end in this digest.
- `docs/DEPLOY.md` and `docs/SECRETS.md` are referenced throughout but were not read; consult them for the real-deploy + KMS specifics.
- Exact request/response JSON schemas live in `packages/contracts` (JSON Schema → Zod/OpenAPI) and `apps/web/lib/api-types.ts` (generated) — pull from there if you need precise field-level shapes; this digest lists routes + key body fields only.
- The `feat/local-bringup` branch is ahead of the `main` state described in the Status doc; any local-bringup-specific changes (compose/env tweaks) were not diffed against `main`.
