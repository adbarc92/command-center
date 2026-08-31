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
  | 'duplicate_fingerprint' | 'github_error' | 'ignored' | 'config_error'

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

/** At most one comment per project per fingerprint per hour: a crash hitting a
 *  thousand users must produce one issue and a handful of comments, not a
 *  thousand notifications.
 *
 *  The key is PROJECT-SCOPED, like `rl:proj:`. The fingerprint is a hash of the
 *  scrubbed title alone, and KV is a single namespace across all eleven
 *  projects: two apps whose users type the same generic title ("Save button
 *  does nothing") share a fingerprint, and an unscoped key would silently
 *  suppress the second project's genuine recurrence comment for an hour. */
export async function shouldComment(kv: KVNamespace, project: string, fp: string): Promise<boolean> {
  const k = `ct:${project}:${fp}:${bucket()}`
  if (await kv.get(k)) return false
  await kv.put(k, '1', { expirationTtl: HOUR * 2 })
  return true
}

/** Never throws. Every counter here is a hot key by construction
 *  (`st:bad_signature:{hour}`) and KV documents roughly one write per second
 *  per key, so a rejection is expected under real traffic. Swallowing it here
 *  rather than at each call site means a KV hiccup degrades observability
 *  instead of taking down the pipeline /v1/stats exists to describe. */
export async function recordStat(kv: KVNamespace, reason: StatReason): Promise<void> {
  try {
    const k = `st:${reason}:${bucket()}`
    const n = Number((await kv.get(k)) ?? '0')
    await kv.put(k, String(n + 1), { expirationTtl: HOUR * 26 })
  } catch {
    // Best-effort by design; see above.
  }
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
