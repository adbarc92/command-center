import { describe, it, expect } from 'vitest'
import { normalize, fingerprint, labelFor } from '../src/fingerprint'

describe('fingerprint', () => {
  it('normalizes case, punctuation and whitespace', () => {
    expect(normalize('Crash on save!')).toBe(normalize('crash   on save'))
  })

  it('gives verbatim repeats the same fingerprint', async () => {
    expect(await fingerprint('Crash on save!')).toBe(await fingerprint('crash on save'))
  })

  it('gives different titles different fingerprints', async () => {
    expect(await fingerprint('crash on save')).not.toBe(await fingerprint('crash on load'))
  })

  it('produces exactly 16 lowercase hex chars', async () => {
    expect(await fingerprint('anything at all')).toMatch(/^[0-9a-f]{16}$/)
  })

  it('builds a label well inside GitHub is 50-char limit', () => {
    const label = labelFor('0123456789abcdef')
    expect(label).toBe('tt:0123456789abcdef')
    expect(label.length).toBeLessThanOrEqual(50)
  })
})
