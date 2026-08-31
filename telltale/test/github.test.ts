import { describe, it, expect } from 'vitest'
import { FakeGitHub } from './fakes'
import { tokenFor, restClient } from '../src/github'
import type { Env, RegistryEntry } from '../src/types'

const env = { GITHUB_TOKEN_PRIMARY: 'tok-a', GITHUB_TOKEN_SECONDARY: 'tok-b' } as Env

describe('tokenFor', () => {
  it('selects the token by account, since a PAT cannot span two accounts', () => {
    expect(tokenFor(env, { repo: 'x/y', account: 'primary', labels: [] } as RegistryEntry)).toBe('tok-a')
    expect(tokenFor(env, { repo: 'x/y', account: 'secondary', labels: [] } as RegistryEntry)).toBe('tok-b')
  })
})

describe('createIssue label-drop detection', () => {
  it('reports labelsDropped when GitHub silently discards them', async () => {
    // GitHub drops `labels` on POST /issues without push access, WITHOUT an
    // error. Since tt: is simultaneously the idempotency key, the dedup key and
    // the read key, an undetected drop means every later report opens a fresh
    // duplicate forever while the Worker reports success.
    const gh = new FakeGitHub()
    gh.dropLabels = true
    const r = await gh.createIssue('x/y', { title: 't', body: 'b', labels: ['tt:abc'] })
    expect(r.labelsDropped).toBe(true)
  })

  it('reports no drop on the happy path', async () => {
    const gh = new FakeGitHub()
    const r = await gh.createIssue('x/y', { title: 't', body: 'b', labels: ['tt:abc'] })
    expect(r.labelsDropped).toBe(false)
  })

  it('detects a partial drop where other labels survive but tt: specifically is missing', async () => {
    // The scenario the spec singles out as most dangerous: a total-count check
    // would miss this because the returned label count never changes.
    const gh = new FakeGitHub()
    gh.dropLabel = 'tt:abc'
    const r = await gh.createIssue('x/y', { title: 't', body: 'b', labels: ['bug', 'tt:abc'] })
    expect(r.labelsDropped).toBe(true)
  })
})

describe('restClient', () => {
  function jsonResponse(body: unknown, status = 201): Response {
    return new Response(JSON.stringify(body), { status, headers: { 'Content-Type': 'application/json' } })
  }

  function captureFetch(respond: (url: string, init?: RequestInit) => Response) {
    const calls: Array<{ url: string; init?: RequestInit }> = []
    const fn = (async (url: string | URL, init?: RequestInit) => {
      const u = String(url)
      calls.push({ url: u, init })
      return respond(u, init)
    }) as typeof fetch
    return { fn, calls }
  }

  it('createIssue reports labelsDropped true when the response omits a requested label, and POSTs JSON with a Content-Type header', async () => {
    const { fn, calls } = captureFetch(() =>
      jsonResponse({ number: 1, html_url: 'https://example.test/i/1', labels: [{ name: 'bug' }] })
    )
    const gh = restClient('tok', fn)
    const r = await gh.createIssue('x/y', { title: 't', body: 'b', labels: ['bug', 'tt:abc'] })

    expect(r).toEqual({ number: 1, url: 'https://example.test/i/1', labelsDropped: true })

    // A plain string body defaults to text/plain per the Fetch spec unless the
    // client sets Content-Type explicitly — GitHub would then read the POST
    // body as text, not JSON.
    const headers = calls[0]!.init!.headers as Record<string, string>
    expect(headers['Content-Type']).toBe('application/json')
  })

  it('createIssue reports labelsDropped false when the response includes every requested label', async () => {
    const { fn } = captureFetch(() =>
      jsonResponse({ number: 2, html_url: 'https://example.test/i/2', labels: [{ name: 'bug' }, { name: 'tt:abc' }] })
    )
    const gh = restClient('tok', fn)
    const r = await gh.createIssue('x/y', { title: 't', body: 'b', labels: ['bug', 'tt:abc'] })
    expect(r.labelsDropped).toBe(false)
  })

  it('findByLabel discriminates pull requests from issues and passes state_reason through untouched', async () => {
    const raw = [
      { number: 10, state: 'closed', state_reason: 'not_planned', labels: [{ name: 'telltale' }] },
      { number: 11, state: 'open', labels: [{ name: 'telltale' }], pull_request: {} },
    ]
    const { fn } = captureFetch(() => jsonResponse(raw))
    const gh = restClient('tok', fn)
    const candidates = await gh.findByLabel('x/y', 'telltale')

    expect(candidates).toEqual([
      { number: 10, state: 'closed', stateReason: 'not_planned', labels: ['telltale'], isPullRequest: false },
      { number: 11, state: 'open', stateReason: null, labels: ['telltale'], isPullRequest: true },
    ])
  })

  it('refuses to follow a repo-transfer redirect, naming the new location', async () => {
    // Workers' default redirect:'follow' rewrites a 301'd POST into a GET, so a
    // transferred repo would return an issue ARRAY where createIssue expects
    // the created issue — created.labels undefined, and a stale registry entry
    // surfacing as a generic github_error far from its cause.
    const { fn, calls } = captureFetch(() =>
      new Response(null, {
        status: 301,
        headers: { location: 'https://api.github.com/repositories/12345/issues' },
      })
    )
    const gh = restClient('tok', fn)
    await expect(gh.createIssue('old/name', { title: 't', body: 'b', labels: ['tt:abc'] }))
      .rejects.toThrow(/301.*repositories\/12345\/issues/)
    expect((calls[0]!.init as RequestInit).redirect).toBe('manual')
  })

  it('throws on a non-2xx response instead of returning an empty array', async () => {
    // An empty array from a failed lookup reads as "no match" and would open
    // a duplicate issue — this must fail loudly instead.
    const { fn } = captureFetch(() => new Response('server error', { status: 500 }))
    const gh = restClient('tok', fn)
    await expect(gh.findByLabel('x/y', 'telltale')).rejects.toThrow()
  })
})
