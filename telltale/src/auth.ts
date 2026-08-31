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

/** Constant-time compare, so a secret cannot be recovered byte by byte.
 *  Shared with the operator read-token check in src/read.ts. */
export function equals(a: string, b: string): boolean {
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
