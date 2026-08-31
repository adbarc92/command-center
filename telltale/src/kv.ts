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
