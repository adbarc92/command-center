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
