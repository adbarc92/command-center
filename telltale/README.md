# Telltale ingest Worker

Accepts authenticated bug reports from the portfolio's apps and games, scrubs
them, deduplicates by fingerprint label, and opens or comments on a GitHub issue
in that project's own repo.

Spec: [`../docs/superpowers/specs/2026-08-30-telltale-feedback-pipeline-design.md`](../docs/superpowers/specs/2026-08-30-telltale-feedback-pipeline-design.md)

Crashes do NOT go through this Worker. They go Sentry → Sentry's native GitHub
integration, with no Telltale code — see spec §3.

## Routes

| Route | Auth |
|---|---|
| `POST /v1/events` | Per-project HMAC over raw bytes |
| `GET /v1/issues` | `Authorization: Bearer $OPERATOR_READ_TOKEN` |
| `GET /v1/stats` | `Authorization: Bearer $OPERATOR_READ_TOKEN` |

## Develop

```bash
npm ci
npm test          # unit suite; the live grader self-skips
npm run check     # tsc --noEmit
npm run dev       # wrangler dev
```

## Deploy

Two registry targets do not exist yet and must be created first, or
`GET /v1/issues` carries a permanent `errors` entry from its very first request
and the live grader can never pass:

```bash
gh repo create adbarc92/telltale-intake --private   # pawsport's target
gh repo create adbarc92/telltale-probe  --private   # the live grader's target
```

```bash
npx wrangler kv namespace create TELLTALE_KV   # paste the id into wrangler.toml
npx wrangler secret put TELLTALE_SENDER_SECRETS  # {"tenzy":"...","hexy":"..."}
npx wrangler secret put GITHUB_TOKEN_PRIMARY     # fine-grained PAT, Issues: read+write
npx wrangler secret put GITHUB_TOKEN_SECONDARY
npx wrangler secret put OPERATOR_READ_TOKEN
npx wrangler secret put IP_HASH_SALT
npm run deploy
```

## Adding a project

Add an explicit entry to `src/registry.ts` — there is no slug-to-repo inference
anywhere, because a wrong guess writes a user's bug report into a stranger's
repository. Then generate an HMAC secret and add it to
`TELLTALE_SENDER_SECRETS`.

`account` must name the PAT whose **resource owner actually owns the repo**: a
fine-grained PAT owned by the `adbarc92` user cannot write to an `OpenBarclay`
org repo at all, so `OpenBarclay/*` is always `secondary`. Verify the owner
against the repo's real location, not against `gh api repos/<owner>/<name>` —
that call silently follows a transfer redirect and reports the repo's NEW owner
under the OLD path, which is how a wrong entry got in once already.

## Running the live grader

Needs a throwaway repo, never a product repo:

```bash
TELLTALE_BASE_URL=https://telltale.<subdomain>.workers.dev \
TELLTALE_PROBE_SECRET=... TELLTALE_PROBE_GH_TOKEN=... \
TELLTALE_PROBE_REPO=<owner>/telltale-probe \
npx vitest run test/live-grader.test.ts
```
