import { describe, it, expect } from 'vitest'
import { FakeKV } from './fakes'
import { handleIssues, toDto } from '../src/read'
import type { Env } from '../src/types'
import type { RawIssue } from '../src/github'

const env = () => ({
  TELLTALE_KV: new FakeKV() as unknown as KVNamespace,
  OPERATOR_READ_TOKEN: 'op-token',
  GITHUB_TOKEN_PRIMARY: 'a', GITHUB_TOKEN_SECONDARY: 'b',
  TELLTALE_SENDER_SECRETS: '{}', IP_HASH_SALT: 's',
}) as Env

const raw = (o: Partial<RawIssue> = {}): RawIssue => ({
  number: 1, title: 't', body: 'b', state: 'open',
  labels: [{ name: 'telltale' }, { name: 'telltale:bug' }],
  assignee: null, created_at: '2026-08-01T00:00:00Z', updated_at: '2026-08-02T00:00:00Z',
  html_url: 'https://example.test/1', ...o,
})

const req = (token?: string) =>
  new Request('https://t.test/v1/issues', token ? { headers: { Authorization: `Bearer ${token}` } } : undefined)

describe('GET /v1/issues auth', () => {
  it('401s without the operator read token', async () => {
    // The Worker can read PRIVATE registry repos. An open read endpoint would
    // serve every private bug-report body to anyone who guesses the hostname.
    expect((await handleIssues(req(), env(), { gh: () => ({} as never) })).status).toBe(401)
    expect((await handleIssues(req('wrong'), env(), { gh: () => ({} as never) })).status).toBe(401)
  })
})

describe('toDto', () => {
  it('derives kind from an explicit whitelist, not a telltale:* prefix parse', () => {
    expect(toDto('x/y', 'tenzy', raw()).kind).toBe('bug')
    expect(toDto('x/y', 'tenzy', raw({ labels: [{ name: 'telltale:crash' }] })).kind).toBe('crash')
    // telltale:muted also matches the prefix; it is not a kind.
    expect(toDto('x/y', 'tenzy', raw({ labels: [{ name: 'telltale:muted' }] })).kind).toBe('unknown')
    expect(toDto('x/y', 'tenzy', raw({ labels: [{ name: 'telltale' }] })).kind).toBe('unknown')
  })

  it('exposes assignee presence, the triage signal the board gates Blocked on', () => {
    expect(toDto('x/y', 'tenzy', raw()).hasAssignee).toBe(false)
    expect(toDto('x/y', 'tenzy', raw({ assignee: { login: 'a' } })).hasAssignee).toBe(true)
  })

  it('carries createdIso separately from updatedIso', () => {
    const d = toDto('x/y', 'tenzy', raw())
    expect(d.createdIso).toBe('2026-08-01T00:00:00Z')
    expect(d.updatedIso).toBe('2026-08-02T00:00:00Z')
  })
})

describe('per-repo error isolation', () => {
  it('returns the repos that answered plus a per-repo error list', async () => {
    // Some registry repos are archived or private with broken billing. One 403
    // must not blank the whole feedback lane.
    const gh = (entry: { repo: string }) => ({
      listTelltaleIssues: async () => {
        if (entry.repo.includes('lineage')) throw new Error('403')
        return [raw()]
      },
    }) as never
    const res = await handleIssues(req('op-token'), env(), { gh })
    expect(res.status).toBe(200)
    const out = await res.json() as { issues: unknown[]; errors: Array<{ project: string }> }
    expect(out.issues.length).toBeGreaterThan(0)
    expect(out.errors.some((e) => e.project === 'lineage')).toBe(true)
  })
})
