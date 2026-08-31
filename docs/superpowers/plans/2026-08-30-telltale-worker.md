# Telltale Ingest Worker — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the Telltale ingest Worker — the standalone service that accepts authenticated bug reports from any app or game, scrubs them, deduplicates them by fingerprint label, and opens or comments on a GitHub issue in that project's own repo.

**Architecture:** One Cloudflare Worker with three routes. Every decision that can be a pure function is one (`schema`, `scrub`, `fingerprint`, `decide`, `registry`), so the logic is unit-tested without network or KV. Two I/O modules (`github`, `kv`) sit behind narrow interfaces that tests replace with fakes. `index.ts` is thin wiring and owns no logic.

**Tech Stack:** TypeScript, Cloudflare Workers (WebCrypto, KV), wrangler, vitest, Node 20. No runtime dependencies.

**Spec:** [`docs/superpowers/specs/2026-08-30-telltale-feedback-pipeline-design.md`](../specs/2026-08-30-telltale-feedback-pipeline-design.md)

---

## Scope

This plan covers **P1 only** — the Worker (spec T1, T3, T4). It produces working, testable software on its own, per the spec's locked decision #8.

The spec's other phases are separate subsystems and get their own plans:

| Phase | Why it is not in this plan |
|---|---|
| **P0** — Sentry setup | Operator configuration in the Sentry console (spec §3, four steps). No code. |
| **P2** — senders | Four reference files across **nine other repositories** in four languages. Nothing here to test until this Worker is deployed. |
| **P3** — dashboard adapter | Lives in `cockpit/ui`, and consumes `GET /v1/issues`, which this plan builds. |

## ⚠️ Two deliberate deviations from the spec — review these before starting

Both simplify without weakening a security property. **If either is rejected, stop and revise the plan rather than improvising.**

1. **The registry is `src/registry.ts` (a typed TS module), not `registry.yml`.** Spec §5.1 says YAML bundled at build time. A Worker cannot read files at runtime, so YAML needs a bundler plugin plus a parser dependency. A TS module preserves every property §5.1 actually asks for — checked in, explicit entries, no slug inference, bundled with the Worker — and adds one: **a typo becomes a compile error instead of a runtime 404.** Deletes a dependency and a build step.

2. **Two fine-grained PATs (one per GitHub account), not GitHub App installation tokens.** Spec §5.4 specifies App installation tokens. Minting those inside a Worker means RS256 JWT signing with the App private key on every request. A fine-grained PAT scoped to selected repos with `Issues: read and write` gives the same property the spec's §5.4 was protecting — *a scoped, write-limited credential held only by the Worker, never by a client* — at a fraction of the machinery. The spec's own objection (round 1) was to **classic** `public_repo`, which is a coarse write scope that also cannot read private repos; a fine-grained PAT is neither. Cost: manual rotation at expiry, acceptable at single-operator scale.

---

## Global Constraints

Copied verbatim from the spec. Every task's requirements implicitly include these.

- **Node 20.** Matches every existing CI job in this repo.
- **No runtime dependencies.** The Worker ships zero `dependencies`; `devDependencies` only.
- **WebCrypto, not `node:crypto`.** Use `crypto.subtle` / `crypto.getRandomValues` — available in both Workers and Node 20.
- `schema_version` **MUST be `1`**. Unknown → `400`, never best-effort parsed.
- `title`: **1–120 chars after trim.** `body`: **0–8000 chars**, truncated with a visible marker, **never rejected**.
- `context`: bounded key set — exactly `platform`, `os_version`, `locale`. **Unknown keys are dropped, not stored.**
- `release.surface` ∈ `ios | android | web | desktop`.
- `reporter.anon_id`: opaque, ≤64 chars. **Never a security boundary.**
- **The `X-Telltale-Project` header is the sole project authority.** The body carries no `project` field.
- **HMAC is over raw request bytes:** `HMAC-SHA256(secret, timestamp + "." + rawBody)`. Never over re-serialized JSON.
- **Replay window: ±10 minutes** against server time. No nonce.
- **The scrub applies to BOTH `title` and `body`.** The long-digit-run rule is **`body`-only**.
- **The fingerprint is computed over the SCRUBBED title.**
- **Never auto-reopen a closed issue.**
- Rate limits: IP+`anon_id` **10/hour**; IP **200/hour**; project **1000/hour**.
- **Client IP is stored only as a salted hash, TTL 1h, and never written to an issue.**

---

## File Structure

All new, under `telltale/` — a third npm project in this repo, matching how `cockpit/ui` and `cockpit/ui/src-tauri` are already independent.

| File | Responsibility |
|---|---|
| `telltale/package.json` | npm project: vitest, wrangler, `@cloudflare/workers-types` |
| `telltale/tsconfig.json` | TS config targeting the Workers runtime |
| `telltale/vitest.config.ts` | vitest, **node** environment (not jsdom) |
| `telltale/wrangler.toml` | Worker name, KV binding, compatibility date |
| `telltale/src/types.ts` | `Env`, `FeedbackEvent`, `RegistryEntry`, `TelltaleIssue` |
| `telltale/src/registry.ts` | The typed project registry + `lookup()` |
| `telltale/src/schema.ts` | `parseEvent()` — validation, pure |
| `telltale/src/scrub.ts` | `scrubTitle()` / `scrubBody()` — pure |
| `telltale/src/fingerprint.ts` | `normalize()` / `fingerprint()` |
| `telltale/src/auth.ts` | `verifySignature()` — HMAC + replay window |
| `telltale/src/decide.ts` | `decide()` — the §4.4 decision table, pure |
| `telltale/src/github.ts` | `GitHubClient` — issues list/create/comment |
| `telltale/src/kv.ts` | Rate limits, comment throttle, stats counters |
| `telltale/src/index.ts` | The router. Wiring only, no logic. |
| `telltale/test/*.test.ts` | One test file per source module |
| `telltale/test/fakes.ts` | `FakeKV`, `FakeGitHub` |
| `telltale/test/live-grader.test.ts` | Gated integration test against a real repo |

---

## Task 1: Project scaffold and the registry

**Files:**
- Create: `telltale/package.json`, `telltale/tsconfig.json`, `telltale/vitest.config.ts`, `telltale/wrangler.toml`, `telltale/src/types.ts`, `telltale/src/registry.ts`
- Test: `telltale/test/registry.test.ts`

**Interfaces:**
- Consumes: nothing.
- Produces: `Env`, `FeedbackEvent`, `RegistryEntry`, `Surface` (from `types.ts`); `REGISTRY: Record<string, RegistryEntry>` and `lookup(project: string): RegistryEntry | null` (from `registry.ts`).

- [ ] **Step 1: Create the npm project**

`telltale/package.json`:

```json
{
  "name": "telltale",
  "private": true,
  "type": "module",
  "scripts": {
    "test": "vitest run",
    "check": "tsc --noEmit",
    "dev": "wrangler dev",
    "deploy": "wrangler deploy"
  },
  "devDependencies": {
    "@cloudflare/workers-types": "^4.20240909.0",
    "typescript": "^5.6.0",
    "vitest": "^2.1.0",
    "wrangler": "^3.78.0"
  }
}
```

`telltale/tsconfig.json`:

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "lib": ["ES2022"],
    "module": "ES2022",
    "moduleResolution": "bundler",
    "types": ["@cloudflare/workers-types", "vitest/globals"],
    "strict": true,
    "noUncheckedIndexedAccess": true,
    "noEmit": true,
    "skipLibCheck": true
  },
  "include": ["src/**/*.ts", "test/**/*.ts"]
}
```

`telltale/vitest.config.ts`:

```ts
import { defineConfig } from 'vitest/config'

// Node environment, not jsdom: this is a Worker, there is no DOM. Node 20 provides
// the same WebCrypto globals (crypto.subtle) the Workers runtime does, so the pure
// modules and the crypto ones both run unmodified here.
export default defineConfig({
  test: {
    environment: 'node',
    globals: true,
    include: ['test/**/*.test.ts'],
  },
})
```

`telltale/wrangler.toml`:

```toml
name = "telltale"
main = "src/index.ts"
compatibility_date = "2026-08-30"

# Rate-limit counters, comment throttles, and stats. Approximate by design —
# see spec §4.2 for why KV is correct here and was not for the deleted dedup gate.
[[kv_namespaces]]
binding = "TELLTALE_KV"
id = "REPLACE_ME_AFTER_wrangler_kv_namespace_create"
```

- [ ] **Step 2: Define the shared types**

`telltale/src/types.ts`:

```ts
export type Surface = 'ios' | 'android' | 'web' | 'desktop'

export interface FeedbackEvent {
  schema_version: 1
  title: string
  body: string
  release?: { version: string; surface: Surface }
  context?: { platform?: string; os_version?: string; locale?: string }
  reporter?: { anon_id?: string }
  occurred_at?: string
}

export interface RegistryEntry {
  /** "owner/name" */
  repo: string
  /** Which PAT to use, keyed by account. */
  account: 'primary' | 'secondary'
  labels: string[]
}

export interface Env {
  TELLTALE_KV: KVNamespace
  /** JSON: { "<project>": "<hmac secret>" } */
  TELLTALE_SENDER_SECRETS: string
  /** Fine-grained PAT, Issues: read+write, primary account. */
  GITHUB_TOKEN_PRIMARY: string
  /** Fine-grained PAT, Issues: read+write, secondary account. */
  GITHUB_TOKEN_SECONDARY: string
  /** Bearer token the cockpit presents to GET /v1/issues and /v1/stats. */
  OPERATOR_READ_TOKEN: string
  /** Salt for hashing client IPs. */
  IP_HASH_SALT: string
}
```

- [ ] **Step 3: Write the failing registry test**

`telltale/test/registry.test.ts`:

```ts
import { describe, it, expect } from 'vitest'
import { REGISTRY, lookup } from '../src/registry'

describe('registry', () => {
  it('resolves a registered project', () => {
    const e = lookup('tenzy')
    expect(e).not.toBeNull()
    expect(e!.repo).toMatch(/^[\w.-]+\/[\w.-]+$/)
  })

  it('returns null for an unregistered project', () => {
    expect(lookup('not-a-real-project')).toBeNull()
  })

  it('never infers a repo from the slug', () => {
    // A slug that is not an explicit entry must not resolve, even though it
    // looks exactly like a plausible repo name.
    expect(lookup('command-center')).toBeNull()
  })

  it('includes a __probe__ entry that is not a product repo', () => {
    const probe = lookup('__probe__')
    expect(probe).not.toBeNull()
    const products = Object.entries(REGISTRY)
      .filter(([k]) => k !== '__probe__')
      .map(([, v]) => v.repo)
    expect(products).not.toContain(probe!.repo)
  })

  it('gives every entry at least the telltale label', () => {
    for (const entry of Object.values(REGISTRY)) {
      expect(entry.labels).toContain('telltale')
    }
  })
})
```

- [ ] **Step 4: Run the test and confirm it fails**

Run: `cd telltale && npm install && npx vitest run test/registry.test.ts`
Expected: FAIL — `Failed to resolve import "../src/registry"`.

- [ ] **Step 5: Implement the registry**

`telltale/src/registry.ts`. Replace `<primary-org>` / `<secondary-org>` with the real GitHub account names when the repo is not under an embargo constraint.

```ts
import type { RegistryEntry } from './types'

/**
 * The project registry — spec §5.1, with one deliberate deviation: a typed TS
 * module rather than registry.yml (see the plan's "deliberate deviations").
 *
 * Entries are EXPLICIT. There is no slug-to-repo inference anywhere in this
 * Worker: a wrong guess writes a user's bug report into a stranger's repository.
 *
 * An entry with no shipping sender (e.g. pawsport) is crash-only — it receives
 * Sentry-created issues and appears on the board, but no app POSTs to it.
 */
export const REGISTRY: Record<string, RegistryEntry> = {
  tenzy:           { repo: '<primary-org>/tenzy',        account: 'primary',   labels: ['telltale'] },
  giftkeeper:      { repo: '<secondary-org>/giftkeeper',  account: 'secondary', labels: ['telltale'] },
  purposefull:     { repo: '<secondary-org>/purposefull', account: 'secondary', labels: ['telltale'] },
  ironsoul:        { repo: '<secondary-org>/ironsoul',    account: 'secondary', labels: ['telltale'] },
  audience:        { repo: '<secondary-org>/audience',    account: 'secondary', labels: ['telltale'] },
  lineage:         { repo: '<secondary-org>/lineage',     account: 'secondary', labels: ['telltale'] },
  'robo.learn':    { repo: '<primary-org>/robo.learn',    account: 'primary',   labels: ['telltale'] },
  'prima-tactica': { repo: '<secondary-org>/prima-tactica', account: 'secondary', labels: ['telltale', 'game'] },
  hexy:            { repo: '<secondary-org>/hexy',        account: 'secondary', labels: ['telltale', 'game'] },

  // Crash-only: archived on GitHub, so it cannot receive issue writes (spec §5.3).
  pawsport:        { repo: '<secondary-org>/telltale-intake', account: 'secondary', labels: ['telltale'] },

  // The live grader's target (spec §9.1). NEVER a product repo: the grader
  // creates real issues, and pointing it at a shipped product would publish
  // synthetic reports into a public tracker.
  __probe__:       { repo: '<secondary-org>/telltale-probe', account: 'secondary', labels: ['telltale'] },
}

export function lookup(project: string): RegistryEntry | null {
  return Object.prototype.hasOwnProperty.call(REGISTRY, project)
    ? REGISTRY[project]!
    : null
}
```

- [ ] **Step 6: Run the test and confirm it passes**

Run: `cd telltale && npx vitest run test/registry.test.ts`
Expected: PASS, 5 tests.

- [ ] **Step 7: Commit**

```bash
git add telltale/
git commit -m "feat(telltale): scaffold the ingest Worker and its project registry"
```

---

## Task 2: Event schema validation

**Files:**
- Create: `telltale/src/schema.ts`
- Test: `telltale/test/schema.test.ts`

**Interfaces:**
- Consumes: `FeedbackEvent`, `Surface` from `types.ts`.
- Produces: `parseEvent(raw: unknown): ParseResult`, where
  `type ParseResult = { ok: true; event: FeedbackEvent } | { ok: false; reason: string }`.

- [ ] **Step 1: Write the failing test**

`telltale/test/schema.test.ts`:

```ts
import { describe, it, expect } from 'vitest'
import { parseEvent } from '../src/schema'

const valid = {
  schema_version: 1,
  title: 'Save button does nothing',
  body: 'Tapped save, nothing happened.',
  release: { version: '1.4.2', surface: 'android' },
  context: { platform: 'android', os_version: '14', locale: 'en-US' },
  reporter: { anon_id: 'abc123' },
  occurred_at: '2026-08-30T18:04:11Z',
}

describe('parseEvent', () => {
  it('accepts a well-formed event', () => {
    const r = parseEvent(valid)
    expect(r.ok).toBe(true)
  })

  it('rejects an unknown schema_version rather than best-effort parsing', () => {
    const r = parseEvent({ ...valid, schema_version: 2 })
    expect(r).toEqual({ ok: false, reason: 'schema_version' })
  })

  it('rejects a body-level project field, which is not part of the schema', () => {
    // The X-Telltale-Project header is the sole authority. A body copy would be
    // a second, unvalidated identity — the exact defect round 3 found.
    const r = parseEvent({ ...valid, project: 'tenzy' })
    expect(r).toEqual({ ok: false, reason: 'project_in_body' })
  })

  it('rejects an empty or overlong title', () => {
    expect(parseEvent({ ...valid, title: '   ' })).toEqual({ ok: false, reason: 'title' })
    expect(parseEvent({ ...valid, title: 'x'.repeat(121) })).toEqual({ ok: false, reason: 'title' })
  })

  it('trims the title', () => {
    const r = parseEvent({ ...valid, title: '  spaced  ' })
    expect(r.ok && r.event.title).toBe('spaced')
  })

  it('truncates an overlong body with a visible marker instead of rejecting it', () => {
    const r = parseEvent({ ...valid, body: 'x'.repeat(9000) })
    expect(r.ok).toBe(true)
    if (!r.ok) return
    expect(r.event.body.length).toBeLessThanOrEqual(8000 + 32)
    expect(r.event.body).toContain('[truncated]')
  })

  it('drops unknown context keys rather than storing them', () => {
    const r = parseEvent({ ...valid, context: { platform: 'ios', email: 'a@b.c' } })
    expect(r.ok).toBe(true)
    if (!r.ok) return
    expect(r.event.context).toEqual({ platform: 'ios' })
    expect(JSON.stringify(r.event)).not.toContain('a@b.c')
  })

  it('rejects an unknown release surface', () => {
    const r = parseEvent({ ...valid, release: { version: '1.0.0', surface: 'watch' } })
    expect(r).toEqual({ ok: false, reason: 'release.surface' })
  })

  it('rejects an overlong anon_id', () => {
    const r = parseEvent({ ...valid, reporter: { anon_id: 'x'.repeat(65) } })
    expect(r).toEqual({ ok: false, reason: 'reporter.anon_id' })
  })
})
```

- [ ] **Step 2: Run the test and confirm it fails**

Run: `cd telltale && npx vitest run test/schema.test.ts`
Expected: FAIL — cannot resolve `../src/schema`.

- [ ] **Step 3: Implement the validator**

`telltale/src/schema.ts`:

```ts
import type { FeedbackEvent, Surface } from './types'

export type ParseResult =
  | { ok: true; event: FeedbackEvent }
  | { ok: false; reason: string }

const SURFACES: readonly Surface[] = ['ios', 'android', 'web', 'desktop']
const CONTEXT_KEYS = ['platform', 'os_version', 'locale'] as const
const BODY_MAX = 8000

function isRecord(v: unknown): v is Record<string, unknown> {
  return typeof v === 'object' && v !== null && !Array.isArray(v)
}

export function parseEvent(raw: unknown): ParseResult {
  if (!isRecord(raw)) return { ok: false, reason: 'not_an_object' }

  if (raw.schema_version !== 1) return { ok: false, reason: 'schema_version' }

  // The header is the sole project authority (spec §4.1). A body copy is
  // rejected outright rather than ignored, so a stale sender fails loudly.
  if ('project' in raw) return { ok: false, reason: 'project_in_body' }

  if (typeof raw.title !== 'string') return { ok: false, reason: 'title' }
  const title = raw.title.trim()
  if (title.length < 1 || title.length > 120) return { ok: false, reason: 'title' }

  if (raw.body !== undefined && typeof raw.body !== 'string') {
    return { ok: false, reason: 'body' }
  }
  const rawBody = typeof raw.body === 'string' ? raw.body : ''
  // Truncate, never reject: a user's report is not worth losing to a length rule.
  const body = rawBody.length > BODY_MAX
    ? rawBody.slice(0, BODY_MAX) + '\n\n…[truncated]'
    : rawBody

  const event: FeedbackEvent = { schema_version: 1, title, body }

  if (raw.release !== undefined) {
    if (!isRecord(raw.release)) return { ok: false, reason: 'release' }
    const { version, surface } = raw.release
    if (typeof version !== 'string' || version.length > 64) {
      return { ok: false, reason: 'release.version' }
    }
    if (typeof surface !== 'string' || !SURFACES.includes(surface as Surface)) {
      return { ok: false, reason: 'release.surface' }
    }
    event.release = { version, surface: surface as Surface }
  }

  if (raw.context !== undefined) {
    if (!isRecord(raw.context)) return { ok: false, reason: 'context' }
    const ctx: Record<string, string> = {}
    // Bounded key set. Unknown keys are DROPPED, not stored — this is what stops
    // `context` becoming an unaudited PII channel.
    for (const k of CONTEXT_KEYS) {
      const v = raw.context[k]
      if (typeof v === 'string' && v.length <= 64) ctx[k] = v
    }
    event.context = ctx
  }

  if (raw.reporter !== undefined) {
    if (!isRecord(raw.reporter)) return { ok: false, reason: 'reporter' }
    const id = raw.reporter.anon_id
    if (id !== undefined) {
      if (typeof id !== 'string' || id.length > 64) {
        return { ok: false, reason: 'reporter.anon_id' }
      }
      event.reporter = { anon_id: id }
    }
  }

  if (typeof raw.occurred_at === 'string') event.occurred_at = raw.occurred_at

  return { ok: true, event }
}
```

- [ ] **Step 4: Run the test and confirm it passes**

Run: `cd telltale && npx vitest run test/schema.test.ts`
Expected: PASS, 9 tests.

- [ ] **Step 5: Commit**

```bash
git add telltale/src/schema.ts telltale/test/schema.test.ts
git commit -m "feat(telltale): validate FeedbackEvent, dropping unknown context keys"
```

---

## Task 3: PII scrub

**Files:**
- Create: `telltale/src/scrub.ts`
- Test: `telltale/test/scrub.test.ts`

**Interfaces:**
- Consumes: nothing.
- Produces: `scrubTitle(s: string): string`, `scrubBody(s: string): string`.

- [ ] **Step 1: Write the failing test**

`telltale/test/scrub.test.ts`:

```ts
import { describe, it, expect } from 'vitest'
import { scrubTitle, scrubBody } from '../src/scrub'

describe('scrub', () => {
  it('redacts an email from the body', () => {
    expect(scrubBody('reach me at alex@example.com ok'))
      .toBe('reach me at [redacted:email] ok')
  })

  it('redacts an email from the TITLE too', () => {
    // The title is the most visible, most indexed, most notification-carrying
    // field in the system. An earlier draft scrubbed only the body.
    expect(scrubTitle('crash when alex@example.com logs in'))
      .toBe('crash when [redacted:email] logs in')
  })

  it('redacts phone numbers in both fields', () => {
    expect(scrubBody('call +1 415 555 0132')).toContain('[redacted:phone]')
    expect(scrubTitle('call 415-555-0132 please')).toContain('[redacted:phone]')
  })

  it('redacts a long digit run in the body', () => {
    expect(scrubBody('card 4111111111111111 declined'))
      .toBe('card [redacted:number] declined')
  })

  it('does NOT apply the long-digit rule to titles', () => {
    // On a 120-char title the rule is all false positives.
    expect(scrubTitle('build 4111111111111111 fails')).toBe('build 4111111111111111 fails')
  })

  it('leaves a version string that looks phone-shaped alone', () => {
    expect(scrubBody('broke in 1.4.2.0 build 20260830')).toBe('broke in 1.4.2.0 build 20260830')
  })

  it('leaves a crash digest stack address alone', () => {
    // Digit-run redaction must not destroy the data triage needs. Hex addresses
    // and short frame offsets carry no digit run of 12+.
    const digest = 'at 0x00007ff8 in frame 42 (offset 1024)'
    expect(scrubBody(digest)).toBe(digest)
  })

  it('is idempotent, so a re-scrub does not mangle a marker', () => {
    const once = scrubBody('mail alex@example.com')
    expect(scrubBody(once)).toBe(once)
  })
})
```

- [ ] **Step 2: Run the test and confirm it fails**

Run: `cd telltale && npx vitest run test/scrub.test.ts`
Expected: FAIL — cannot resolve `../src/scrub`.

- [ ] **Step 3: Implement the scrub**

`telltale/src/scrub.ts`:

```ts
/**
 * Ingest-side PII redaction (spec §4.3).
 *
 * Recorded operator decision: issues land in each project's own repo, including
 * public ones. That exposure was raised and knowingly accepted; this module is
 * the mitigation, not a guarantee — it misses obfuscated forms like
 * "alex at example dot com".
 *
 * Redaction is LOSSY AND ONE-WAY BY DESIGN. The original is never stored
 * anywhere: a store of unredacted originals would recreate the hazard.
 */

const EMAIL = /[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}/g

// E.164 and NANP shapes. Requires a separator or a leading +, so a bare run of
// digits is left to the DIGIT_RUN rule (body only) and version strings survive.
const PHONE = /(?:\+\d{1,3}[\s.-]?)?(?:\(\d{3}\)|\d{3})[\s.-]\d{3}[\s.-]\d{4}\b/g

// 12+ consecutive digits: card and account numbers. BODY ONLY.
const DIGIT_RUN = /\b\d{12,}\b/g

export function scrubTitle(s: string): string {
  return s.replace(EMAIL, '[redacted:email]').replace(PHONE, '[redacted:phone]')
}

export function scrubBody(s: string): string {
  return scrubTitle(s).replace(DIGIT_RUN, '[redacted:number]')
}
```

- [ ] **Step 4: Run the test and confirm it passes**

Run: `cd telltale && npx vitest run test/scrub.test.ts`
Expected: PASS, 8 tests.

- [ ] **Step 5: Commit**

```bash
git add telltale/src/scrub.ts telltale/test/scrub.test.ts
git commit -m "feat(telltale): scrub PII from both title and body"
```

---

## Task 4: Fingerprint

**Files:**
- Create: `telltale/src/fingerprint.ts`
- Test: `telltale/test/fingerprint.test.ts`

**Interfaces:**
- Consumes: nothing.
- Produces: `normalize(title: string): string`, `fingerprint(scrubbedTitle: string): Promise<string>` (16 lowercase hex chars), `labelFor(fp: string): string` (returns `tt:<fp>`).

- [ ] **Step 1: Write the failing test**

`telltale/test/fingerprint.test.ts`:

```ts
import { describe, it, expect } from 'vitest'
import { normalize, fingerprint, labelFor } from '../src/fingerprint'

describe('fingerprint', () => {
  it('normalizes case, punctuation and whitespace', () => {
    expect(normalize('Crash on save!')).toBe(normalize('crash   on save'))
  })

  it('gives verbatim repeats the same fingerprint', async () => {
    expect(await fingerprint('Crash on save!')).toBe(await fingerprint('crash on save'))
  })

  it('gives different titles different fingerprints', async () => {
    expect(await fingerprint('crash on save')).not.toBe(await fingerprint('crash on load'))
  })

  it('produces exactly 16 lowercase hex chars', async () => {
    expect(await fingerprint('anything at all')).toMatch(/^[0-9a-f]{16}$/)
  })

  it('builds a label well inside GitHub is 50-char limit', () => {
    const label = labelFor('0123456789abcdef')
    expect(label).toBe('tt:0123456789abcdef')
    expect(label.length).toBeLessThanOrEqual(50)
  })
})
```

- [ ] **Step 2: Run the test and confirm it fails**

Run: `cd telltale && npx vitest run test/fingerprint.test.ts`
Expected: FAIL — cannot resolve `../src/fingerprint`.

- [ ] **Step 3: Implement**

`telltale/src/fingerprint.ts`:

```ts
/**
 * Dedup identity (spec §4.4).
 *
 * Deliberately WEAK grouping over the title alone. It catches verbatim repeats —
 * the common case when a visible bug is reported by many people — and misses
 * paraphrases. Semantic grouping is a model call, and a model call in the dedup
 * path makes issue identity non-deterministic and untestable. Not doing it is
 * the decision.
 *
 * ALWAYS called with the SCRUBBED title, so identity is stable regardless of
 * what redaction removed.
 */

export function normalize(title: string): string {
  return title
    .toLowerCase()
    .replace(/[^\p{L}\p{N}\s]/gu, '')
    .replace(/\s+/g, ' ')
    .trim()
}

export async function fingerprint(scrubbedTitle: string): Promise<string> {
  const data = new TextEncoder().encode(normalize(scrubbedTitle))
  const digest = await crypto.subtle.digest('SHA-256', data)
  return [...new Uint8Array(digest)]
    .map((b) => b.toString(16).padStart(2, '0'))
    .join('')
    .slice(0, 16)
}

/** The label IS the idempotency key, the dedup key, and the read key. */
export function labelFor(fp: string): string {
  return `tt:${fp}`
}
```

- [ ] **Step 4: Run the test and confirm it passes**

Run: `cd telltale && npx vitest run test/fingerprint.test.ts`
Expected: PASS, 5 tests.

- [ ] **Step 5: Commit**

```bash
git add telltale/src/fingerprint.ts telltale/test/fingerprint.test.ts
git commit -m "feat(telltale): fingerprint the scrubbed title for dedup"
```

---

## Task 5: HMAC authentication and the replay window

**Files:**
- Create: `telltale/src/auth.ts`
- Test: `telltale/test/auth.test.ts`

**Interfaces:**
- Consumes: nothing.
- Produces: `sign(secret: string, timestamp: string, rawBody: string): Promise<string>` and
  `verifySignature(args: { secret: string; timestamp: string | null; signature: string | null; rawBody: string; nowMs: number }): AuthResult`, where
  `type AuthResult = { ok: true } | { ok: false; reason: 'missing' | 'clock_skew' | 'bad_signature' }`.

- [ ] **Step 1: Write the failing test**

`telltale/test/auth.test.ts`:

```ts
import { describe, it, expect } from 'vitest'
import { sign, verifySignature } from '../src/auth'

const SECRET = 'test-secret'
const NOW = Date.UTC(2026, 7, 30, 18, 0, 0)
const ts = String(Math.floor(NOW / 1000))
const BODY = '{"schema_version":1,"title":"x","body":"y"}'

async function good() {
  return { secret: SECRET, timestamp: ts, signature: await sign(SECRET, ts, BODY), rawBody: BODY, nowMs: NOW }
}

describe('verifySignature', () => {
  it('accepts a correctly signed request', async () => {
    expect(verifySignature(await good())).resolves.toEqual({ ok: true })
  })

  it('rejects a missing signature or timestamp', async () => {
    const g = await good()
    await expect(verifySignature({ ...g, signature: null })).resolves.toEqual({ ok: false, reason: 'missing' })
    await expect(verifySignature({ ...g, timestamp: null })).resolves.toEqual({ ok: false, reason: 'missing' })
  })

  it('rejects a wrong secret', async () => {
    const g = await good()
    await expect(verifySignature({ ...g, secret: 'other-secret' }))
      .resolves.toEqual({ ok: false, reason: 'bad_signature' })
  })

  it('rejects a tampered body even with a valid-looking signature', async () => {
    const g = await good()
    await expect(verifySignature({ ...g, rawBody: BODY.replace('"x"', '"z"') }))
      .resolves.toEqual({ ok: false, reason: 'bad_signature' })
  })

  it('rejects a replay outside the +/-10 minute window', async () => {
    const g = await good()
    await expect(verifySignature({ ...g, nowMs: NOW + 11 * 60_000 }))
      .resolves.toEqual({ ok: false, reason: 'clock_skew' })
    await expect(verifySignature({ ...g, nowMs: NOW - 11 * 60_000 }))
      .resolves.toEqual({ ok: false, reason: 'clock_skew' })
  })

  it('accepts inside the window in both directions', async () => {
    const g = await good()
    await expect(verifySignature({ ...g, nowMs: NOW + 9 * 60_000 })).resolves.toEqual({ ok: true })
    await expect(verifySignature({ ...g, nowMs: NOW - 9 * 60_000 })).resolves.toEqual({ ok: true })
  })

  it('signs over raw bytes, so key order changes the signature', async () => {
    const a = await sign(SECRET, ts, '{"a":1,"b":2}')
    const b = await sign(SECRET, ts, '{"b":2,"a":1}')
    expect(a).not.toBe(b)
  })
})
```

- [ ] **Step 2: Run the test and confirm it fails**

Run: `cd telltale && npx vitest run test/auth.test.ts`
Expected: FAIL — cannot resolve `../src/auth`.

- [ ] **Step 3: Implement**

`telltale/src/auth.ts`:

```ts
/**
 * Request authentication (spec §4.1).
 *
 * The threat: this endpoint turns an HTTP request into a public GitHub issue in
 * the operator's repo, authored by the operator's token. An unauthenticated
 * endpoint is a remote "publish arbitrary text under Alex's identity" primitive.
 *
 * The signature is over RAW REQUEST BYTES, never a canonicalized JSON
 * re-serialization: five independent canonicalizers (GDScript, Dart, RN, browser
 * JS, Worker) agreeing byte-for-byte on key order and number formatting is a
 * silent-401 generator.
 *
 * There is NO NONCE. A captured request is replayable inside the +/-10 minute
 * window. Accepted: the payoff is a duplicate report, which dedup collapses.
 */

export type AuthResult = { ok: true } | { ok: false; reason: 'missing' | 'clock_skew' | 'bad_signature' }

const WINDOW_MS = 10 * 60_000

async function key(secret: string): Promise<CryptoKey> {
  return crypto.subtle.importKey(
    'raw',
    new TextEncoder().encode(secret),
    { name: 'HMAC', hash: 'SHA-256' },
    false,
    ['sign'],
  )
}

export async function sign(secret: string, timestamp: string, rawBody: string): Promise<string> {
  const mac = await crypto.subtle.sign(
    'HMAC',
    await key(secret),
    new TextEncoder().encode(`${timestamp}.${rawBody}`),
  )
  return [...new Uint8Array(mac)].map((b) => b.toString(16).padStart(2, '0')).join('')
}

/** Constant-time compare, so a signature cannot be recovered byte by byte. */
function equals(a: string, b: string): boolean {
  if (a.length !== b.length) return false
  let diff = 0
  for (let i = 0; i < a.length; i++) diff |= a.charCodeAt(i) ^ b.charCodeAt(i)
  return diff === 0
}

export async function verifySignature(args: {
  secret: string
  timestamp: string | null
  signature: string | null
  rawBody: string
  nowMs: number
}): Promise<AuthResult> {
  const { secret, timestamp, signature, rawBody, nowMs } = args
  if (!timestamp || !signature) return { ok: false, reason: 'missing' }

  const tsSec = Number(timestamp)
  if (!Number.isFinite(tsSec)) return { ok: false, reason: 'missing' }
  if (Math.abs(nowMs - tsSec * 1000) > WINDOW_MS) return { ok: false, reason: 'clock_skew' }

  const expected = await sign(secret, timestamp, rawBody)
  return equals(expected, signature) ? { ok: true } : { ok: false, reason: 'bad_signature' }
}
```

- [ ] **Step 4: Run the test and confirm it passes**

Run: `cd telltale && npx vitest run test/auth.test.ts`
Expected: PASS, 7 tests.

- [ ] **Step 5: Commit**

```bash
git add telltale/src/auth.ts telltale/test/auth.test.ts
git commit -m "feat(telltale): HMAC request auth over raw bytes with a replay window"
```

---

## Task 6: The dedup decision table

**Files:**
- Create: `telltale/src/decide.ts`
- Test: `telltale/test/decide.test.ts`

**Interfaces:**
- Consumes: nothing.
- Produces:

```ts
export interface CandidateIssue {
  number: number
  state: 'open' | 'closed'
  stateReason: 'completed' | 'not_planned' | null
  labels: string[]
  isPullRequest: boolean
}
export type Decision =
  | { action: 'create' }
  | { action: 'comment'; issue: number }
  | { action: 'ignore'; reason: 'muted' | 'not_planned' }
export function decide(candidates: CandidateIssue[]): Decision
```

- [ ] **Step 1: Write the failing test**

`telltale/test/decide.test.ts`:

```ts
import { describe, it, expect } from 'vitest'
import { decide, type CandidateIssue } from '../src/decide'

const base: CandidateIssue = {
  number: 7, state: 'open', stateReason: null, labels: ['telltale'], isPullRequest: false,
}

describe('decide', () => {
  it('creates when there is no match', () => {
    expect(decide([])).toEqual({ action: 'create' })
  })

  it('comments on an existing open issue, never opening a second', () => {
    expect(decide([base])).toEqual({ action: 'comment', issue: 7 })
  })

  it('comments but does NOT reopen a completed-closed issue', () => {
    // Mobile users run old builds for months: a bug fixed in 1.4.3 keeps
    // arriving from 1.4.1 clients and must not perpetually reopen its issue.
    const closed = { ...base, state: 'closed' as const, stateReason: 'completed' as const }
    expect(decide([closed])).toEqual({ action: 'comment', issue: 7 })
  })

  it('treats a legacy closure with a null state_reason as completed', () => {
    const legacy = { ...base, state: 'closed' as const, stateReason: null }
    expect(decide([legacy])).toEqual({ action: 'comment', issue: 7 })
  })

  it('is silent for a not_planned closure', () => {
    const wontfix = { ...base, state: 'closed' as const, stateReason: 'not_planned' as const }
    expect(decide([wontfix])).toEqual({ action: 'ignore', reason: 'not_planned' })
  })

  it('is silent for a muted issue even when open', () => {
    const muted = { ...base, labels: ['telltale', 'telltale:muted'] }
    expect(decide([muted])).toEqual({ action: 'ignore', reason: 'muted' })
  })

  it('skips pull requests, which the issues endpoint also returns', () => {
    // A fix PR carrying the telltale label would otherwise read as an open bug.
    const pr = { ...base, number: 99, isPullRequest: true }
    expect(decide([pr])).toEqual({ action: 'create' })
    expect(decide([pr, base])).toEqual({ action: 'comment', issue: 7 })
  })

  it('prefers the lowest-numbered open issue when duplicates exist', () => {
    // Duplicates are the EXPECTED outcome of a concurrent create race, not an
    // anomaly — see the plan's note on retry-vs-concurrency idempotency.
    const later = { ...base, number: 12 }
    expect(decide([later, base])).toEqual({ action: 'comment', issue: 7 })
  })

  it('prefers an open issue over a closed one', () => {
    const closed = { ...base, number: 3, state: 'closed' as const, stateReason: 'completed' as const }
    expect(decide([closed, base])).toEqual({ action: 'comment', issue: 7 })
  })
})
```

- [ ] **Step 2: Run the test and confirm it fails**

Run: `cd telltale && npx vitest run test/decide.test.ts`
Expected: FAIL — cannot resolve `../src/decide`.

- [ ] **Step 3: Implement**

`telltale/src/decide.ts`:

```ts
/**
 * The dedup decision table (spec §4.4, §5.3). Pure — no I/O, fully testable.
 *
 * Automated report-to-issue WITHOUT dedup is issue spam, and issue spam destroys
 * the tracker the pipeline exists to feed.
 */

export interface CandidateIssue {
  number: number
  state: 'open' | 'closed'
  stateReason: 'completed' | 'not_planned' | null
  labels: string[]
  isPullRequest: boolean
}

export type Decision =
  | { action: 'create' }
  | { action: 'comment'; issue: number }
  | { action: 'ignore'; reason: 'muted' | 'not_planned' }

export function decide(candidates: CandidateIssue[]): Decision {
  // GET /issues returns pull requests as issues. A labelled fix PR is not a report.
  const issues = candidates.filter((c) => !c.isPullRequest)
  if (issues.length === 0) return { action: 'create' }

  // Operator intent to stay silent wins over everything else.
  if (issues.some((i) => i.labels.includes('telltale:muted'))) {
    return { action: 'ignore', reason: 'muted' }
  }

  const open = issues.filter((i) => i.state === 'open').sort((a, b) => a.number - b.number)
  if (open.length > 0) return { action: 'comment', issue: open[0]!.number }

  const closed = [...issues].sort((a, b) => a.number - b.number)
  const first = closed[0]!
  if (first.stateReason === 'not_planned') return { action: 'ignore', reason: 'not_planned' }

  // Closed as completed, or a legacy closure with no state_reason: comment so the
  // recurrence is recorded, but NEVER auto-reopen.
  return { action: 'comment', issue: first.number }
}
```

- [ ] **Step 4: Run the test and confirm it passes**

Run: `cd telltale && npx vitest run test/decide.test.ts`
Expected: PASS, 9 tests.

- [ ] **Step 5: Commit**

```bash
git add telltale/src/decide.ts telltale/test/decide.test.ts
git commit -m "feat(telltale): dedup decision table, never auto-reopening"
```

---

## Task 7: KV counters — rate limits, comment throttle, stats

**Files:**
- Create: `telltale/src/kv.ts`, `telltale/test/fakes.ts`
- Test: `telltale/test/kv.test.ts`

**Interfaces:**
- Consumes: `Env` from `types.ts`.
- Produces: `hashIp(ip, salt): Promise<string>`; `checkRateLimits(kv, { ipHash, anonId, project }): Promise<RateResult>` where `type RateResult = { ok: true } | { ok: false; scope: 'pair' | 'ip' | 'project' }`; `shouldComment(kv, fp): Promise<boolean>`; `recordStat(kv, reason): Promise<void>`; `readStats(kv): Promise<Record<string, number>>`. `FakeKV` from `test/fakes.ts`.

- [ ] **Step 1: Write the fake KV**

`telltale/test/fakes.ts`:

```ts
/** Minimal in-memory KVNamespace stand-in. Enough for counters and TTL-less reads. */
export class FakeKV {
  store = new Map<string, string>()
  async get(k: string): Promise<string | null> { return this.store.get(k) ?? null }
  async put(k: string, v: string, _o?: { expirationTtl?: number }): Promise<void> { this.store.set(k, v) }
  async delete(k: string): Promise<void> { this.store.delete(k) }
}
```

- [ ] **Step 2: Write the failing test**

`telltale/test/kv.test.ts`:

```ts
import { describe, it, expect } from 'vitest'
import { FakeKV } from './fakes'
import { hashIp, checkRateLimits, shouldComment, recordStat, readStats } from '../src/kv'

const kv = () => new FakeKV() as unknown as KVNamespace

describe('hashIp', () => {
  it('is stable, salted, and never returns the raw IP', async () => {
    const a = await hashIp('203.0.113.7', 'salt')
    expect(a).toBe(await hashIp('203.0.113.7', 'salt'))
    expect(a).not.toContain('203.0.113.7')
    expect(a).not.toBe(await hashIp('203.0.113.7', 'other-salt'))
  })
})

describe('checkRateLimits', () => {
  it('allows traffic under every ceiling', async () => {
    const k = kv()
    const r = await checkRateLimits(k, { ipHash: 'h', anonId: 'a', project: 'tenzy' })
    expect(r).toEqual({ ok: true })
  })

  it('blocks the 11th event from one install on one IP', async () => {
    const k = kv()
    const args = { ipHash: 'h', anonId: 'a', project: 'tenzy' }
    for (let i = 0; i < 10; i++) expect(await checkRateLimits(k, args)).toEqual({ ok: true })
    expect(await checkRateLimits(k, args)).toEqual({ ok: false, scope: 'pair' })
  })

  it('allows many installs behind one CGNAT IP, up to the higher IP ceiling', async () => {
    // Carrier-grade NAT puts thousands of mobile users behind one address, and
    // four of five sender platforms are mobile. A tight per-IP cap would
    // silently destroy the 21st genuine reporter on a carrier.
    const k = kv()
    for (let i = 0; i < 20; i++) {
      expect(await checkRateLimits(k, { ipHash: 'h', anonId: `install-${i}`, project: 'tenzy' }))
        .toEqual({ ok: true })
    }
  })
})

describe('shouldComment', () => {
  it('allows the first comment then throttles within the hour', async () => {
    const k = kv()
    expect(await shouldComment(k, 'fp1')).toBe(true)
    expect(await shouldComment(k, 'fp1')).toBe(false)
    expect(await shouldComment(k, 'fp2')).toBe(true)
  })
})

describe('stats', () => {
  it('counts by reason so silence is diagnosable', async () => {
    const k = kv()
    await recordStat(k, 'accepted')
    await recordStat(k, 'accepted')
    await recordStat(k, 'bad_signature')
    expect(await readStats(k)).toMatchObject({ accepted: 2, bad_signature: 1 })
  })
})
```

- [ ] **Step 3: Run the test and confirm it fails**

Run: `cd telltale && npx vitest run test/kv.test.ts`
Expected: FAIL — cannot resolve `../src/kv`.

- [ ] **Step 4: Implement**

`telltale/src/kv.ts`:

```ts
/**
 * Approximate counters (spec §4.2, §4.6).
 *
 * KV is CORRECT HERE and was not correct for the dedup gate an earlier design
 * draft used it for: these are abuse counters and throttles where a lost
 * increment under concurrency is harmless. Correctness-critical dedup lives in
 * the GitHub label lookup (src/decide.ts), not here.
 */

const HOUR = 3600

export type RateResult = { ok: true } | { ok: false; scope: 'pair' | 'ip' | 'project' }

export type StatReason =
  | 'accepted' | 'bad_signature' | 'clock_skew' | 'rate_limited'
  | 'unregistered_project' | 'invalid_schema' | 'labels_dropped'
  | 'duplicate_fingerprint' | 'github_error' | 'ignored'

/** The client IP is stored ONLY as a salted hash, and never written to an issue. */
export async function hashIp(ip: string, salt: string): Promise<string> {
  const digest = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(`${salt}:${ip}`))
  return [...new Uint8Array(digest)].map((b) => b.toString(16).padStart(2, '0')).join('').slice(0, 32)
}

function bucket(): string {
  return String(Math.floor(Date.now() / 1000 / HOUR))
}

async function bump(kv: KVNamespace, key: string, limit: number): Promise<boolean> {
  const k = `rl:${key}:${bucket()}`
  const n = Number((await kv.get(k)) ?? '0')
  if (n >= limit) return false
  await kv.put(k, String(n + 1), { expirationTtl: HOUR * 2 })
  return true
}

export async function checkRateLimits(
  kv: KVNamespace,
  a: { ipHash: string; anonId: string; project: string },
): Promise<RateResult> {
  if (!(await bump(kv, `pair:${a.ipHash}:${a.anonId}`, 10))) return { ok: false, scope: 'pair' }
  if (!(await bump(kv, `ip:${a.ipHash}`, 200))) return { ok: false, scope: 'ip' }
  if (!(await bump(kv, `proj:${a.project}`, 1000))) return { ok: false, scope: 'project' }
  return { ok: true }
}

/** At most one comment per fingerprint per hour: a crash hitting a thousand users
 *  must produce one issue and a handful of comments, not a thousand notifications. */
export async function shouldComment(kv: KVNamespace, fp: string): Promise<boolean> {
  const k = `ct:${fp}:${bucket()}`
  if (await kv.get(k)) return false
  await kv.put(k, '1', { expirationTtl: HOUR * 2 })
  return true
}

export async function recordStat(kv: KVNamespace, reason: StatReason): Promise<void> {
  const k = `st:${reason}:${bucket()}`
  const n = Number((await kv.get(k)) ?? '0')
  await kv.put(k, String(n + 1), { expirationTtl: HOUR * 26 })
}

export async function readStats(kv: KVNamespace): Promise<Record<string, number>> {
  const out: Record<string, number> = {}
  const list = await kv.list({ prefix: 'st:' })
  for (const { name } of list.keys) {
    const reason = name.split(':')[1]!
    out[reason] = (out[reason] ?? 0) + Number((await kv.get(name)) ?? '0')
  }
  return out
}
```

Add `list` to the fake so `readStats` works:

```ts
// append to telltale/test/fakes.ts, inside class FakeKV
  async list({ prefix }: { prefix: string }) {
    return { keys: [...this.store.keys()].filter((k) => k.startsWith(prefix)).map((name) => ({ name })) }
  }
```

- [ ] **Step 5: Run the test and confirm it passes**

Run: `cd telltale && npx vitest run test/kv.test.ts`
Expected: PASS, 6 tests.

- [ ] **Step 6: Commit**

```bash
git add telltale/src/kv.ts telltale/test/kv.test.ts telltale/test/fakes.ts
git commit -m "feat(telltale): KV rate limits, comment throttle and stats counters"
```

---

## Task 8: GitHub client, with label-drop detection

**Files:**
- Create: `telltale/src/github.ts`
- Modify: `telltale/test/fakes.ts` (add `FakeGitHub`)
- Test: `telltale/test/github.test.ts`

**Interfaces:**
- Consumes: `CandidateIssue` from `decide.ts`; `RegistryEntry`, `Env` from `types.ts`.
- Produces:

```ts
export interface GitHubClient {
  findByLabel(repo: string, label: string): Promise<CandidateIssue[]>
  createIssue(repo: string, i: { title: string; body: string; labels: string[] }):
    Promise<{ number: number; url: string; labelsDropped: boolean }>
  commentIssue(repo: string, number: number, body: string): Promise<void>
  listTelltaleIssues(repo: string): Promise<RawIssue[]>
}
export function tokenFor(env: Env, entry: RegistryEntry): string
export function restClient(token: string, fetchImpl?: typeof fetch): GitHubClient
```

- [ ] **Step 1: Write the fake and the failing test**

Append to `telltale/test/fakes.ts`:

```ts
import type { GitHubClient } from '../src/github'
import type { CandidateIssue } from '../src/decide'

export class FakeGitHub implements GitHubClient {
  issues: Array<CandidateIssue & { title: string; body: string }> = []
  comments: Array<{ number: number; body: string }> = []
  /** Simulates GitHub silently dropping labels when the token lacks push access. */
  dropLabels = false
  private next = 1

  async findByLabel(_repo: string, label: string): Promise<CandidateIssue[]> {
    return this.issues.filter((i) => i.labels.includes(label))
  }

  async createIssue(_repo: string, i: { title: string; body: string; labels: string[] }) {
    const labels = this.dropLabels ? [] : i.labels
    const number = this.next++
    this.issues.push({
      number, state: 'open', stateReason: null, labels,
      isPullRequest: false, title: i.title, body: i.body,
    })
    return { number, url: `https://example.test/i/${number}`, labelsDropped: labels.length !== i.labels.length }
  }

  async commentIssue(_repo: string, number: number, body: string) {
    this.comments.push({ number, body })
  }

  async listTelltaleIssues() { return [] }
}
```

`telltale/test/github.test.ts`:

```ts
import { describe, it, expect } from 'vitest'
import { FakeGitHub } from './fakes'
import { tokenFor } from '../src/github'
import type { Env, RegistryEntry } from '../src/types'

const env = { GITHUB_TOKEN_PRIMARY: 'tok-a', GITHUB_TOKEN_SECONDARY: 'tok-b' } as Env

describe('tokenFor', () => {
  it('selects the token by account, since a PAT cannot span two accounts', () => {
    expect(tokenFor(env, { repo: 'x/y', account: 'primary', labels: [] } as RegistryEntry)).toBe('tok-a')
    expect(tokenFor(env, { repo: 'x/y', account: 'secondary', labels: [] } as RegistryEntry)).toBe('tok-b')
  })
})

describe('createIssue label-drop detection', () => {
  it('reports labelsDropped when GitHub silently discards them', async () => {
    // GitHub drops `labels` on POST /issues without push access, WITHOUT an
    // error. Since tt: is simultaneously the idempotency key, the dedup key and
    // the read key, an undetected drop means every later report opens a fresh
    // duplicate forever while the Worker reports success.
    const gh = new FakeGitHub()
    gh.dropLabels = true
    const r = await gh.createIssue('x/y', { title: 't', body: 'b', labels: ['tt:abc'] })
    expect(r.labelsDropped).toBe(true)
  })

  it('reports no drop on the happy path', async () => {
    const gh = new FakeGitHub()
    const r = await gh.createIssue('x/y', { title: 't', body: 'b', labels: ['tt:abc'] })
    expect(r.labelsDropped).toBe(false)
  })
})
```

- [ ] **Step 2: Run the test and confirm it fails**

Run: `cd telltale && npx vitest run test/github.test.ts`
Expected: FAIL — cannot resolve `../src/github`.

- [ ] **Step 3: Implement**

`telltale/src/github.ts`:

```ts
import type { CandidateIssue } from './decide'
import type { Env, RegistryEntry } from './types'

export interface RawIssue {
  number: number
  title: string
  body: string | null
  state: 'open' | 'closed'
  labels: Array<{ name: string }>
  assignee: unknown | null
  created_at: string
  updated_at: string
  html_url: string
  pull_request?: unknown
}

export interface GitHubClient {
  findByLabel(repo: string, label: string): Promise<CandidateIssue[]>
  createIssue(repo: string, i: { title: string; body: string; labels: string[] }):
    Promise<{ number: number; url: string; labelsDropped: boolean }>
  commentIssue(repo: string, number: number, body: string): Promise<void>
  listTelltaleIssues(repo: string): Promise<RawIssue[]>
}

/** A fine-grained PAT is per-account, so the registry names which one to use. */
export function tokenFor(env: Env, entry: RegistryEntry): string {
  return entry.account === 'primary' ? env.GITHUB_TOKEN_PRIMARY : env.GITHUB_TOKEN_SECONDARY
}

function toCandidate(i: RawIssue): CandidateIssue {
  return {
    number: i.number,
    state: i.state,
    // The REST list endpoint omits state_reason on older closures; decide()
    // treats null as `completed`.
    stateReason: ((i as { state_reason?: 'completed' | 'not_planned' | null }).state_reason) ?? null,
    labels: i.labels.map((l) => l.name),
    isPullRequest: i.pull_request !== undefined,
  }
}

export function restClient(token: string, fetchImpl: typeof fetch = fetch): GitHubClient {
  const headers = {
    Authorization: `Bearer ${token}`,
    Accept: 'application/vnd.github+json',
    'X-GitHub-Api-Version': '2022-11-28',
    'User-Agent': 'telltale',
  }

  async function api(path: string, init?: RequestInit): Promise<Response> {
    const res = await fetchImpl(`https://api.github.com${path}`, { ...init, headers })
    if (!res.ok) throw new Error(`github ${res.status} on ${path}`)
    return res
  }

  return {
    // The REST LIST endpoint with `labels=` — not the search API, which is
    // eventually consistent and capped at 30 req/min. `labels=` is exact and
    // AND-semantic, which is what dedup needs.
    async findByLabel(repo, label) {
      const res = await api(`/repos/${repo}/issues?labels=${encodeURIComponent(label)}&state=all&per_page=100`)
      return ((await res.json()) as RawIssue[]).map(toCandidate)
    },

    async createIssue(repo, i) {
      const res = await api(`/repos/${repo}/issues`, { method: 'POST', body: JSON.stringify(i) })
      const created = (await res.json()) as RawIssue
      const got = created.labels.map((l) => l.name)
      return {
        number: created.number,
        url: created.html_url,
        // Verified, never assumed — see the label-drop test.
        labelsDropped: i.labels.some((l) => !got.includes(l)),
      }
    },

    async commentIssue(repo, number, body) {
      await api(`/repos/${repo}/issues/${number}/comments`, { method: 'POST', body: JSON.stringify({ body }) })
    },

    async listTelltaleIssues(repo) {
      const res = await api(`/repos/${repo}/issues?labels=telltale&state=open&per_page=100`)
      return (await res.json()) as RawIssue[]
    },
  }
}
```

- [ ] **Step 4: Run the test and confirm it passes**

Run: `cd telltale && npx vitest run test/github.test.ts`
Expected: PASS, 3 tests.

- [ ] **Step 5: Commit**

```bash
git add telltale/src/github.ts telltale/test/github.test.ts telltale/test/fakes.ts
git commit -m "feat(telltale): GitHub client verifying labels on create"
```

---

## Task 9: The router — `POST /v1/events`

**Files:**
- Create: `telltale/src/index.ts`
- Test: `telltale/test/events.test.ts`

**Interfaces:**
- Consumes: everything from Tasks 1–8.
- Produces: the default Worker export `{ fetch(req: Request, env: Env): Promise<Response> }`, and `handleEvent(req, env, deps)` where `deps = { gh: (entry: RegistryEntry) => GitHubClient; nowMs: number }` so tests inject a fake client.

**Ordering (spec §4.5), which the implementation must follow exactly:** auth → registry → schema → rate limit → scrub → fingerprint → label lookup → decide → create-or-comment → verify labels → record stat.

- [ ] **Step 1: Write the failing test**

`telltale/test/events.test.ts`:

```ts
import { describe, it, expect } from 'vitest'
import { FakeKV, FakeGitHub } from './fakes'
import { handleEvent } from '../src/index'
import { sign } from '../src/auth'
import type { Env } from '../src/types'

const SECRET = 'sender-secret'
const NOW = Date.UTC(2026, 7, 30, 18, 0, 0)

function makeEnv(kv: FakeKV): Env {
  return {
    TELLTALE_KV: kv as unknown as KVNamespace,
    TELLTALE_SENDER_SECRETS: JSON.stringify({ tenzy: SECRET }),
    GITHUB_TOKEN_PRIMARY: 'a', GITHUB_TOKEN_SECONDARY: 'b',
    OPERATOR_READ_TOKEN: 'op', IP_HASH_SALT: 'salt',
  }
}

async function post(body: object, opts: { secret?: string; ts?: string } = {}) {
  const raw = JSON.stringify(body)
  const ts = opts.ts ?? String(Math.floor(NOW / 1000))
  return new Request('https://t.test/v1/events', {
    method: 'POST',
    headers: {
      'X-Telltale-Project': 'tenzy',
      'X-Telltale-Timestamp': ts,
      'X-Telltale-Signature': await sign(opts.secret ?? SECRET, ts, raw),
      'CF-Connecting-IP': '203.0.113.7',
    },
    body: raw,
  })
}

const EVENT = { schema_version: 1, title: 'Save fails', body: 'nothing happens', reporter: { anon_id: 'a1' } }

function deps(gh: FakeGitHub) {
  return { gh: () => gh, nowMs: NOW }
}

describe('POST /v1/events', () => {
  it('creates an issue carrying the tt: fingerprint label', async () => {
    const gh = new FakeGitHub()
    const res = await handleEvent(await post(EVENT), makeEnv(new FakeKV()), deps(gh))
    expect(res.status).toBe(202)
    expect(gh.issues).toHaveLength(1)
    expect(gh.issues[0]!.labels.some((l) => l.startsWith('tt:'))).toBe(true)
    expect(gh.issues[0]!.title).toBe('[bug] Save fails')
  })

  it('comments instead of opening a second issue for the same title', async () => {
    const gh = new FakeGitHub()
    const env = makeEnv(new FakeKV())
    await handleEvent(await post(EVENT), env, deps(gh))
    await handleEvent(await post({ ...EVENT, reporter: { anon_id: 'a2' } }), env, deps(gh))
    expect(gh.issues).toHaveLength(1)
    expect(gh.comments).toHaveLength(1)
  })

  it('is idempotent across a sender retry', async () => {
    // A create that succeeded with a lost response, retried by the sender,
    // must not double-open. The tt: label is what makes the retry safe.
    const gh = new FakeGitHub()
    const env = makeEnv(new FakeKV())
    const req = await post(EVENT)
    await handleEvent(req.clone(), env, deps(gh))
    await handleEvent(await post(EVENT), env, deps(gh))
    expect(gh.issues).toHaveLength(1)
  })

  it('scrubs an email out of the title before the issue is created', async () => {
    const gh = new FakeGitHub()
    await handleEvent(
      await post({ ...EVENT, title: 'crash for alex@example.com' }),
      makeEnv(new FakeKV()), deps(gh),
    )
    expect(gh.issues[0]!.title).toContain('[redacted:email]')
    expect(gh.issues[0]!.title).not.toContain('alex@example.com')
  })

  it('rejects a wrong signature with 401 and reaches no sink', async () => {
    const gh = new FakeGitHub()
    const res = await handleEvent(await post(EVENT, { secret: 'wrong' }), makeEnv(new FakeKV()), deps(gh))
    expect(res.status).toBe(401)
    expect(gh.issues).toHaveLength(0)
  })

  it('returns the server time on a clock-skew rejection so the retry can re-sign', async () => {
    const stale = String(Math.floor(NOW / 1000) - 20 * 60)
    const res = await handleEvent(await post(EVENT, { ts: stale }), makeEnv(new FakeKV()), deps(new FakeGitHub()))
    expect(res.status).toBe(401)
    expect(res.headers.get('X-Telltale-Server-Time')).toBeTruthy()
  })

  it('404s an unregistered project so a typo fails loudly', async () => {
    const raw = JSON.stringify(EVENT)
    const ts = String(Math.floor(NOW / 1000))
    const req = new Request('https://t.test/v1/events', {
      method: 'POST',
      headers: {
        'X-Telltale-Project': 'nope', 'X-Telltale-Timestamp': ts,
        'X-Telltale-Signature': await sign(SECRET, ts, raw), 'CF-Connecting-IP': '203.0.113.7',
      },
      body: raw,
    })
    expect((await handleEvent(req, makeEnv(new FakeKV()), deps(new FakeGitHub()))).status).toBe(404)
  })

  it('400s a body that carries a project field', async () => {
    const res = await handleEvent(
      await post({ ...EVENT, project: 'tenzy' }), makeEnv(new FakeKV()), deps(new FakeGitHub()),
    )
    expect(res.status).toBe(400)
  })

  it('does not report success when GitHub silently drops the labels', async () => {
    const gh = new FakeGitHub()
    gh.dropLabels = true
    const res = await handleEvent(await post(EVENT), makeEnv(new FakeKV()), deps(gh))
    expect(res.status).toBe(500)
  })

  it('429s past the per-install ceiling', async () => {
    const gh = new FakeGitHub()
    const env = makeEnv(new FakeKV())
    let last = 0
    for (let i = 0; i < 12; i++) last = (await handleEvent(await post(EVENT), env, deps(gh))).status
    expect(last).toBe(429)
  })
})
```

- [ ] **Step 2: Run the test and confirm it fails**

Run: `cd telltale && npx vitest run test/events.test.ts`
Expected: FAIL — cannot resolve `../src/index`.

- [ ] **Step 3: Implement the router**

`telltale/src/index.ts`:

```ts
import type { Env, RegistryEntry } from './types'
import { lookup } from './registry'
import { parseEvent } from './schema'
import { scrubTitle, scrubBody } from './scrub'
import { fingerprint, labelFor } from './fingerprint'
import { verifySignature } from './auth'
import { decide } from './decide'
import { restClient, tokenFor, type GitHubClient } from './github'
import { hashIp, checkRateLimits, shouldComment, recordStat, readStats, type StatReason } from './kv'

export interface Deps {
  gh: (entry: RegistryEntry) => GitHubClient
  nowMs: number
}

function json(status: number, body: unknown, headers: Record<string, string> = {}): Response {
  return new Response(JSON.stringify(body), {
    status, headers: { 'content-type': 'application/json', ...headers },
  })
}

export async function handleEvent(req: Request, env: Env, deps: Deps): Promise<Response> {
  const kv = env.TELLTALE_KV
  const fail = async (status: number, reason: StatReason, extra?: Record<string, string>) => {
    await recordStat(kv, reason)
    return json(status, { error: reason }, extra)
  }

  const project = req.headers.get('X-Telltale-Project')
  if (!project) return fail(401, 'bad_signature')

  const secrets = JSON.parse(env.TELLTALE_SENDER_SECRETS) as Record<string, string>
  const secret = secrets[project]
  const rawBody = await req.text()

  // 1. Auth. The header is the sole project authority.
  if (!secret) return fail(401, 'bad_signature')
  const auth = await verifySignature({
    secret,
    timestamp: req.headers.get('X-Telltale-Timestamp'),
    signature: req.headers.get('X-Telltale-Signature'),
    rawBody,
    nowMs: deps.nowMs,
  })
  if (!auth.ok) {
    // Hand back server time so the sender's single mandated retry can re-sign.
    // Device clock skew is common on Android and would otherwise fail silently.
    const extra = { 'X-Telltale-Server-Time': String(Math.floor(deps.nowMs / 1000)) }
    return fail(401, auth.reason === 'clock_skew' ? 'clock_skew' : 'bad_signature', extra)
  }

  // 2. Registry. An unregistered slug fails loudly rather than dropping silently.
  const entry = lookup(project)
  if (!entry) return fail(404, 'unregistered_project')

  // 3. Schema.
  let parsed
  try { parsed = parseEvent(JSON.parse(rawBody)) } catch { return fail(400, 'invalid_schema') }
  if (!parsed.ok) return fail(400, 'invalid_schema')
  const event = parsed.event

  // 4. Rate limits, on server-observed identity as well as the client's anon_id.
  const ipHash = await hashIp(req.headers.get('CF-Connecting-IP') ?? '0.0.0.0', env.IP_HASH_SALT)
  const rate = await checkRateLimits(kv, { ipHash, anonId: event.reporter?.anon_id ?? 'anon', project })
  if (!rate.ok) return fail(429, 'rate_limited', { 'Retry-After': '3600' })

  // 5. Scrub, THEN fingerprint — so identity is stable regardless of redaction.
  const title = scrubTitle(event.title)
  const body = scrubBody(event.body)
  const fp = await fingerprint(title)
  const label = labelFor(fp)

  const gh = deps.gh(entry)
  let candidates
  try { candidates = await gh.findByLabel(entry.repo, label) } catch { return fail(503, 'github_error') }

  const decision = decide(candidates)
  if (decision.action === 'ignore') return fail(200, 'ignored')

  const footer =
    `\n\n---\n` +
    (event.release ? `Release: ${project}-${event.release.surface}@${event.release.version}\n` : '') +
    (event.context ? `Context: ${JSON.stringify(event.context)}\n` : '') +
    `<!-- telltale fingerprint=${fp} project=${project} -->`

  try {
    if (decision.action === 'create') {
      const created = await gh.createIssue(entry.repo, {
        title: `[bug] ${title}`,
        body: body + footer,
        labels: [...entry.labels, 'telltale:bug', label],
      })
      if (created.labelsDropped) {
        // Silent label loss breaks the idempotency key, the dedup key and the
        // read key at once, while otherwise reporting success. Fail loudly.
        await recordStat(kv, 'labels_dropped')
        return json(500, { error: 'labels_dropped', issue: created.number })
      }
      await recordStat(kv, 'accepted')
      return json(202, { issue: created.number, url: created.url })
    }

    if (await shouldComment(kv, fp)) {
      await gh.commentIssue(entry.repo, decision.issue, `Reported again.${footer}`)
    }
    await recordStat(kv, 'accepted')
    return json(202, { issue: decision.issue })
  } catch {
    return fail(503, 'github_error')
  }
}

export default {
  async fetch(req: Request, env: Env): Promise<Response> {
    const url = new URL(req.url)
    const deps: Deps = {
      gh: (entry) => restClient(tokenFor(env, entry)),
      nowMs: Date.now(),
    }
    if (req.method === 'POST' && url.pathname === '/v1/events') {
      return handleEvent(req, env, deps)
    }
    return json(404, { error: 'not_found' })
  },
}
```

- [ ] **Step 4: Run the test and confirm it passes**

Run: `cd telltale && npx vitest run test/events.test.ts`
Expected: PASS, 10 tests.

- [ ] **Step 5: Run the whole suite and the type check**

Run: `cd telltale && npm test && npm run check`
Expected: all green, `tsc --noEmit` clean.

- [ ] **Step 6: Commit**

```bash
git add telltale/src/index.ts telltale/test/events.test.ts
git commit -m "feat(telltale): POST /v1/events — auth, scrub, dedup, create-or-comment"
```

---

## Task 10: The read endpoints — `GET /v1/issues` and `GET /v1/stats`

**Files:**
- Modify: `telltale/src/index.ts`
- Create: `telltale/src/read.ts`
- Test: `telltale/test/read.test.ts`

**Interfaces:**
- Consumes: `GitHubClient`, `RawIssue`, `readStats`.
- Produces: `handleIssues(req, env, deps)`, `handleStats(req, env)`, and the wire type the P3 adapter consumes:

```ts
export interface TelltaleIssueDTO {
  repo: string; number: number; title: string; body: string
  kind: 'bug' | 'crash' | 'unknown'
  project: string; isOpen: boolean; hasAssignee: boolean
  createdIso: string; updatedIso: string; labels: string[]; url: string
}
export interface IssuesResponse { issues: TelltaleIssueDTO[]; errors: Array<{ project: string; message: string }> }
```

- [ ] **Step 1: Write the failing test**

`telltale/test/read.test.ts`:

```ts
import { describe, it, expect } from 'vitest'
import { FakeKV } from './fakes'
import { handleIssues, toDto } from '../src/read'
import type { Env } from '../src/types'
import type { RawIssue } from '../src/github'

const env = () => ({
  TELLTALE_KV: new FakeKV() as unknown as KVNamespace,
  OPERATOR_READ_TOKEN: 'op-token',
  GITHUB_TOKEN_PRIMARY: 'a', GITHUB_TOKEN_SECONDARY: 'b',
  TELLTALE_SENDER_SECRETS: '{}', IP_HASH_SALT: 's',
}) as Env

const raw = (o: Partial<RawIssue> = {}): RawIssue => ({
  number: 1, title: 't', body: 'b', state: 'open',
  labels: [{ name: 'telltale' }, { name: 'telltale:bug' }],
  assignee: null, created_at: '2026-08-01T00:00:00Z', updated_at: '2026-08-02T00:00:00Z',
  html_url: 'https://example.test/1', ...o,
})

const req = (token?: string) =>
  new Request('https://t.test/v1/issues', token ? { headers: { Authorization: `Bearer ${token}` } } : undefined)

describe('GET /v1/issues auth', () => {
  it('401s without the operator read token', async () => {
    // The Worker can read PRIVATE registry repos. An open read endpoint would
    // serve every private bug-report body to anyone who guesses the hostname.
    expect((await handleIssues(req(), env(), { gh: () => ({} as never) })).status).toBe(401)
    expect((await handleIssues(req('wrong'), env(), { gh: () => ({} as never) })).status).toBe(401)
  })
})

describe('toDto', () => {
  it('derives kind from an explicit whitelist, not a telltale:* prefix parse', () => {
    expect(toDto('x/y', 'tenzy', raw()).kind).toBe('bug')
    expect(toDto('x/y', 'tenzy', raw({ labels: [{ name: 'telltale:crash' }] })).kind).toBe('crash')
    // telltale:muted also matches the prefix; it is not a kind.
    expect(toDto('x/y', 'tenzy', raw({ labels: [{ name: 'telltale:muted' }] })).kind).toBe('unknown')
    expect(toDto('x/y', 'tenzy', raw({ labels: [{ name: 'telltale' }] })).kind).toBe('unknown')
  })

  it('exposes assignee presence, the triage signal the board gates Blocked on', () => {
    expect(toDto('x/y', 'tenzy', raw()).hasAssignee).toBe(false)
    expect(toDto('x/y', 'tenzy', raw({ assignee: { login: 'a' } })).hasAssignee).toBe(true)
  })

  it('carries createdIso separately from updatedIso', () => {
    const d = toDto('x/y', 'tenzy', raw())
    expect(d.createdIso).toBe('2026-08-01T00:00:00Z')
    expect(d.updatedIso).toBe('2026-08-02T00:00:00Z')
  })
})

describe('per-repo error isolation', () => {
  it('returns the repos that answered plus a per-repo error list', async () => {
    // Some registry repos are archived or private with broken billing. One 403
    // must not blank the whole feedback lane.
    const gh = (entry: { repo: string }) => ({
      listTelltaleIssues: async () => {
        if (entry.repo.includes('lineage')) throw new Error('403')
        return [raw()]
      },
    }) as never
    const res = await handleIssues(req('op-token'), env(), { gh })
    expect(res.status).toBe(200)
    const out = await res.json() as { issues: unknown[]; errors: Array<{ project: string }> }
    expect(out.issues.length).toBeGreaterThan(0)
    expect(out.errors.some((e) => e.project === 'lineage')).toBe(true)
  })
})
```

- [ ] **Step 2: Run the test and confirm it fails**

Run: `cd telltale && npx vitest run test/read.test.ts`
Expected: FAIL — cannot resolve `../src/read`.

- [ ] **Step 3: Implement**

`telltale/src/read.ts`:

```ts
import type { Env, RegistryEntry } from './types'
import { REGISTRY } from './registry'
import { readStats } from './kv'
import type { GitHubClient, RawIssue } from './github'

export interface TelltaleIssueDTO {
  repo: string; number: number; title: string; body: string
  kind: 'bug' | 'crash' | 'unknown'
  project: string; isOpen: boolean; hasAssignee: boolean
  createdIso: string; updatedIso: string; labels: string[]; url: string
}

export interface IssuesResponse {
  issues: TelltaleIssueDTO[]
  errors: Array<{ project: string; message: string }>
}

/** Explicit whitelist. A `telltale:*` prefix parse would yield 'muted'. */
function kindOf(labels: string[]): 'bug' | 'crash' | 'unknown' {
  if (labels.includes('telltale:crash')) return 'crash'
  if (labels.includes('telltale:bug')) return 'bug'
  return 'unknown'
}

export function toDto(repo: string, project: string, i: RawIssue): TelltaleIssueDTO {
  const labels = i.labels.map((l) => l.name)
  return {
    repo, number: i.number, title: i.title, body: i.body ?? '',
    kind: kindOf(labels), project,
    isOpen: i.state === 'open',
    hasAssignee: i.assignee !== null && i.assignee !== undefined,
    createdIso: i.created_at, updatedIso: i.updated_at,
    labels, url: i.html_url,
  }
}

function unauthorized(req: Request, env: Env): boolean {
  return req.headers.get('Authorization') !== `Bearer ${env.OPERATOR_READ_TOKEN}`
}

export async function handleIssues(
  req: Request, env: Env, deps: { gh: (entry: RegistryEntry) => GitHubClient },
): Promise<Response> {
  if (unauthorized(req, env)) {
    return new Response(JSON.stringify({ error: 'unauthorized' }), {
      status: 401, headers: { 'content-type': 'application/json' },
    })
  }

  const issues: TelltaleIssueDTO[] = []
  const errors: IssuesResponse['errors'] = []

  for (const [project, entry] of Object.entries(REGISTRY)) {
    if (project === '__probe__') continue
    try {
      const raw = await deps.gh(entry).listTelltaleIssues(entry.repo)
      // GET /issues returns pull requests as issues.
      for (const i of raw) {
        if (i.pull_request === undefined) issues.push(toDto(entry.repo, project, i))
      }
    } catch (e) {
      // Degrade only the affected project, never the whole lane.
      errors.push({ project, message: e instanceof Error ? e.message : 'unknown' })
    }
  }

  return new Response(JSON.stringify({ issues, errors } satisfies IssuesResponse), {
    status: 200,
    headers: { 'content-type': 'application/json', 'cache-control': 'max-age=60' },
  })
}

export async function handleStats(req: Request, env: Env): Promise<Response> {
  if (unauthorized(req, env)) {
    return new Response(JSON.stringify({ error: 'unauthorized' }), {
      status: 401, headers: { 'content-type': 'application/json' },
    })
  }
  return new Response(JSON.stringify(await readStats(env.TELLTALE_KV)), {
    status: 200, headers: { 'content-type': 'application/json' },
  })
}
```

- [ ] **Step 4: Wire the routes**

In `telltale/src/index.ts`, add the import and two branches inside `fetch`, immediately before the final `return json(404, ...)`:

```ts
import { handleIssues, handleStats } from './read'
```

```ts
    if (req.method === 'GET' && url.pathname === '/v1/issues') {
      return handleIssues(req, env, deps)
    }
    if (req.method === 'GET' && url.pathname === '/v1/stats') {
      return handleStats(req, env)
    }
```

- [ ] **Step 5: Run the whole suite and the type check**

Run: `cd telltale && npm test && npm run check`
Expected: all green.

- [ ] **Step 6: Commit**

```bash
git add telltale/src/read.ts telltale/src/index.ts telltale/test/read.test.ts
git commit -m "feat(telltale): authenticated read endpoints with per-repo error isolation"
```

---

## Task 11: CI job, the gated live grader, and the README

**Files:**
- Create: `telltale/test/live-grader.test.ts`, `telltale/README.md`
- Modify: `.github/workflows/ci.yml`

**Interfaces:**
- Consumes: the deployed Worker (live grader only).
- Produces: a `telltale` CI job; a documented deploy runbook.

- [ ] **Step 1: Write the gated live grader**

`telltale/test/live-grader.test.ts`:

```ts
import { describe, it, expect } from 'vitest'
import { sign } from '../src/auth'

/**
 * The independent grader (spec §9.1), in Halyard's verify-launch spirit: it
 * asserts by READING THE SINK BACK, never by trusting the Worker's own success
 * report — the only way a dedup regression is caught.
 *
 * Gated like this repo's real-Docker integration tests: it needs live
 * credentials, so it is skipped unless they are present. It targets the
 * __probe__ registry entry and NEVER a product repo — an earlier design would
 * have published synthetic issues into a shipped product's public tracker.
 */
const BASE = process.env.TELLTALE_BASE_URL
const SECRET = process.env.TELLTALE_PROBE_SECRET
const GH = process.env.TELLTALE_PROBE_GH_TOKEN
const REPO = process.env.TELLTALE_PROBE_REPO

const live = BASE && SECRET && GH && REPO ? describe : describe.skip

live('live grader', () => {
  it('collapses N identical reports into exactly one issue', async () => {
    const title = `grader ${crypto.randomUUID()}`
    const raw = JSON.stringify({ schema_version: 1, title, body: 'synthetic', reporter: { anon_id: 'grader' } })
    const ts = String(Math.floor(Date.now() / 1000))
    const sig = await sign(SECRET!, ts, raw)

    for (let i = 0; i < 3; i++) {
      const res = await fetch(`${BASE}/v1/events`, {
        method: 'POST',
        headers: {
          'X-Telltale-Project': '__probe__',
          'X-Telltale-Timestamp': ts,
          'X-Telltale-Signature': sig,
        },
        body: raw,
      })
      expect(res.status).toBe(202)
    }

    // Read the sink back, not the Worker's own report.
    const listed = await fetch(
      `https://api.github.com/repos/${REPO}/issues?state=all&per_page=100&labels=telltale`,
      { headers: { Authorization: `Bearer ${GH}`, Accept: 'application/vnd.github+json' } },
    )
    const issues = (await listed.json()) as Array<{ number: number; title: string }>
    const mine = issues.filter((i) => i.title.includes(title))
    expect(mine).toHaveLength(1)

    // Clean up so a re-run tests the create path again, not the dedup path.
    await fetch(`https://api.github.com/repos/${REPO}/issues/${mine[0]!.number}`, {
      method: 'PATCH',
      headers: { Authorization: `Bearer ${GH}`, Accept: 'application/vnd.github+json' },
      body: JSON.stringify({ state: 'closed', state_reason: 'not_planned' }),
    })
  }, 30_000)
})
```

- [ ] **Step 2: Verify the grader skips cleanly with no credentials**

Run: `cd telltale && npx vitest run test/live-grader.test.ts`
Expected: PASS with the suite reported as skipped. **No network call is made.**

- [ ] **Step 3: Add the CI job**

In `.github/workflows/ci.yml`, add this job to `jobs:`, modelled on the existing `vitest (cockpit/ui)` job:

```yaml
  telltale:
    name: vitest (telltale)
    runs-on: ubuntu-latest
    defaults:
      run:
        working-directory: telltale
    steps:
      - uses: actions/checkout@v4

      - name: Install Node.js
        uses: actions/setup-node@v4
        with:
          node-version: 20
          cache: npm
          cache-dependency-path: telltale/package-lock.json

      - run: npm ci

      # The live grader (test/live-grader.test.ts) self-skips without
      # TELLTALE_* credentials, so it never runs here — same posture as the
      # real-Docker ITs in crates/fleetd/tests/.
      - run: npm test

      - run: npm run check
```

- [ ] **Step 4: Write the README**

`telltale/README.md`:

```markdown
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

## Running the live grader

Needs a throwaway repo, never a product repo:

```bash
TELLTALE_BASE_URL=https://telltale.<subdomain>.workers.dev \
TELLTALE_PROBE_SECRET=... TELLTALE_PROBE_GH_TOKEN=... \
TELLTALE_PROBE_REPO=<owner>/telltale-probe \
npx vitest run test/live-grader.test.ts
```
```

- [ ] **Step 5: Run everything**

Run: `cd telltale && npm ci && npm test && npm run check`
Expected: all suites green, `tsc` clean.

- [ ] **Step 6: Commit**

```bash
git add telltale/test/live-grader.test.ts telltale/README.md telltale/package-lock.json .github/workflows/ci.yml
git commit -m "ci(telltale): add the vitest job, the gated live grader and the runbook"
```

---

## Self-review

**Spec coverage.** T1 → Tasks 1–2. T3 (auth §4.1, identity §4.2, scrub §4.3, fingerprint §4.4, ordering §4.5, stats §4.6) → Tasks 3–7, 9, 10. T4 (registry §5.1, issue shape §5.2, credentials §5.4) → Tasks 1, 8, 9. §9.1's full named-test list → Tasks 2–10, with the pull-request skip in Task 6, label-drop detection in Tasks 8–9, and the grader in Task 11.

**Deliberately not covered here:** T5/P3 (the dashboard adapter, spec §6) and P2 (senders, §8) are separate subsystems in other directories and repositories; P0 is Sentry console configuration. Each needs its own plan.

**Known gap, carried from the spec rather than introduced here.** Spec §10.5 records that create-idempotency holds for **retries but not for true concurrency** — the pre-create label lookup is a check-then-act with no mutual exclusion, so two simultaneous reports of the same title can both create. Task 6's `decide()` contains that outcome by preferring the lowest-numbered open issue, and Task 9's test covers the retry case only. **This is asserted as a risk, not as a passing test** — do not add a test claiming the concurrent case passes.

**Before Task 1, resolve these three external facts** (spec §10.7): whether `pawsport` / `elevation-broker` are archived and reject writes; current Cloudflare free-tier limits for Workers + KV; and the real GitHub account names for `src/registry.ts`.

---

## Execution handoff

Plan complete and saved to `docs/superpowers/plans/2026-08-30-telltale-worker.md`. Two execution options:

1. **Subagent-Driven (recommended)** — a fresh subagent per task, review between tasks, fast iteration.
2. **Inline Execution** — execute tasks in this session with checkpoints for review.
