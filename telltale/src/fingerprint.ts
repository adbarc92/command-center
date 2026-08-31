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
