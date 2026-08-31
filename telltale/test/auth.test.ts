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
    await expect(verifySignature(await good())).resolves.toEqual({ ok: true })
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
