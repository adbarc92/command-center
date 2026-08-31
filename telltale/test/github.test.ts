import { describe, it, expect } from 'vitest'
import { FakeGitHub } from './fakes'
import { tokenFor } from '../src/github'
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
})
