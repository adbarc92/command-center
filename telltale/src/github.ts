import type { CandidateIssue } from './decide'
import type { Env, RegistryEntry } from './types'

export interface RawIssue {
  number: number
  title: string
  body: string | null
  state: 'open' | 'closed'
  state_reason?: 'completed' | 'not_planned' | null
  labels: Array<{ name: string }>
  assignee: unknown | null
  created_at: string
  updated_at: string
  html_url: string
  pull_request?: unknown
}

export interface GitHubClient {
  findByLabel(repo: string, label: string): Promise<CandidateIssue[]>
  createIssue(repo: string, i: { title: string; body: string; labels: string[] }):
    Promise<{ number: number; url: string; labelsDropped: boolean }>
  commentIssue(repo: string, number: number, body: string): Promise<void>
  listTelltaleIssues(repo: string): Promise<RawIssue[]>
}

/** A fine-grained PAT is per-account, so the registry names which one to use. */
export function tokenFor(env: Env, entry: RegistryEntry): string {
  return entry.account === 'primary' ? env.GITHUB_TOKEN_PRIMARY : env.GITHUB_TOKEN_SECONDARY
}

function toCandidate(i: RawIssue): CandidateIssue {
  return {
    number: i.number,
    state: i.state,
    // The REST list endpoint omits state_reason on older closures; decide()
    // treats null as `completed`.
    stateReason: i.state_reason ?? null,
    labels: i.labels.map((l) => l.name),
    isPullRequest: i.pull_request !== undefined,
  }
}

export function restClient(token: string, fetchImpl: typeof fetch = fetch): GitHubClient {
  const headers = {
    Authorization: `Bearer ${token}`,
    Accept: 'application/vnd.github+json',
    'X-GitHub-Api-Version': '2022-11-28',
    'User-Agent': 'telltale',
    // Without this, a plain string body defaults to text/plain per the Fetch
    // spec and GitHub reads the POST body as text, not JSON.
    'Content-Type': 'application/json',
  }

  async function api(path: string, init?: RequestInit): Promise<Response> {
    const res = await fetchImpl(`https://api.github.com${path}`, { ...init, headers })
    if (!res.ok) throw new Error(`github ${res.status} on ${path}`)
    return res
  }

  return {
    // The REST LIST endpoint with `labels=` — not the search API, which is
    // eventually consistent and capped at 30 req/min. `labels=` is exact and
    // AND-semantic, which is what dedup needs.
    async findByLabel(repo, label) {
      const res = await api(`/repos/${repo}/issues?labels=${encodeURIComponent(label)}&state=all&per_page=100`)
      return ((await res.json()) as RawIssue[]).map(toCandidate)
    },

    async createIssue(repo, i) {
      const res = await api(`/repos/${repo}/issues`, { method: 'POST', body: JSON.stringify(i) })
      const created = (await res.json()) as RawIssue
      const got = created.labels.map((l) => l.name)
      return {
        number: created.number,
        url: created.html_url,
        // Verified, never assumed — see the label-drop test.
        labelsDropped: i.labels.some((l) => !got.includes(l)),
      }
    },

    async commentIssue(repo, number, body) {
      await api(`/repos/${repo}/issues/${number}/comments`, { method: 'POST', body: JSON.stringify({ body }) })
    },

    async listTelltaleIssues(repo) {
      const res = await api(`/repos/${repo}/issues?labels=telltale&state=open&per_page=100`)
      return (await res.json()) as RawIssue[]
    },
  }
}
