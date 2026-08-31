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
