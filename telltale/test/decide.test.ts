import { describe, it, expect } from 'vitest'
import { decide, type CandidateIssue } from '../src/decide'

const base: CandidateIssue = {
  number: 7, state: 'open', stateReason: null, labels: ['telltale'], isPullRequest: false,
}

describe('decide', () => {
  it('creates when there is no match', () => {
    expect(decide([])).toEqual({ action: 'create' })
  })

  it('comments on an existing open issue, never opening a second', () => {
    expect(decide([base])).toEqual({ action: 'comment', issue: 7 })
  })

  it('comments but does NOT reopen a completed-closed issue', () => {
    // Mobile users run old builds for months: a bug fixed in 1.4.3 keeps
    // arriving from 1.4.1 clients and must not perpetually reopen its issue.
    const closed = { ...base, state: 'closed' as const, stateReason: 'completed' as const }
    expect(decide([closed])).toEqual({ action: 'comment', issue: 7 })
  })

  it('treats a legacy closure with a null state_reason as completed', () => {
    const legacy = { ...base, state: 'closed' as const, stateReason: null }
    expect(decide([legacy])).toEqual({ action: 'comment', issue: 7 })
  })

  it('is silent for a not_planned closure', () => {
    const wontfix = { ...base, state: 'closed' as const, stateReason: 'not_planned' as const }
    expect(decide([wontfix])).toEqual({ action: 'ignore', reason: 'not_planned' })
  })

  it('is silent for a muted issue even when open', () => {
    const muted = { ...base, labels: ['telltale', 'telltale:muted'] }
    expect(decide([muted])).toEqual({ action: 'ignore', reason: 'muted' })
  })

  it('skips pull requests, which the issues endpoint also returns', () => {
    // A fix PR carrying the telltale label would otherwise read as an open bug.
    const pr = { ...base, number: 99, isPullRequest: true }
    expect(decide([pr])).toEqual({ action: 'create' })
    expect(decide([pr, base])).toEqual({ action: 'comment', issue: 7 })
  })

  it('prefers the lowest-numbered open issue when duplicates exist', () => {
    // Duplicates are the EXPECTED outcome of a concurrent create race, not an
    // anomaly — see the plan's note on retry-vs-concurrency idempotency.
    const later = { ...base, number: 12 }
    expect(decide([later, base])).toEqual({ action: 'comment', issue: 7 })
  })

  it('prefers an open issue over a closed one', () => {
    const closed = { ...base, number: 3, state: 'closed' as const, stateReason: 'completed' as const }
    expect(decide([closed, base])).toEqual({ action: 'comment', issue: 7 })
  })
})
