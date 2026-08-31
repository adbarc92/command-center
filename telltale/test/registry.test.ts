import { describe, it, expect } from 'vitest'
import { REGISTRY, lookup } from '../src/registry'

describe('registry', () => {
  it('resolves a registered project', () => {
    const e = lookup('tenzy')
    expect(e).not.toBeNull()
    expect(e!.repo).toMatch(/^[\w.-]+\/[\w.-]+$/)
  })

  it('returns null for an unregistered project', () => {
    expect(lookup('not-a-real-project')).toBeNull()
  })

  it('never infers a repo from the slug', () => {
    // A slug that is not an explicit entry must not resolve, even though it
    // looks exactly like a plausible repo name.
    expect(lookup('command-center')).toBeNull()
  })

  it('includes a __probe__ entry that is not a product repo', () => {
    const probe = lookup('__probe__')
    expect(probe).not.toBeNull()
    const products = Object.entries(REGISTRY)
      .filter(([k]) => k !== '__probe__')
      .map(([, v]) => v.repo)
    expect(products).not.toContain(probe!.repo)
  })

  it('gives every entry at least the telltale label', () => {
    for (const entry of Object.values(REGISTRY)) {
      expect(entry.labels).toContain('telltale')
    }
  })
})
