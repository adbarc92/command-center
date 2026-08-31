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
