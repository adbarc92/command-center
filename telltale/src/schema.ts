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
