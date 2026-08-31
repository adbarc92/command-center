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
