import type { Env, RegistryEntry } from './types'
import { REGISTRY } from './registry'
import { readStats } from './kv'
import { equals } from './auth'
import type { GitHubClient, RawIssue } from './github'

export interface TelltaleIssueDTO {
  repo: string; number: number; title: string; body: string
  kind: 'bug' | 'crash' | 'unknown'
  project: string; isOpen: boolean; hasAssignee: boolean
  createdIso: string; updatedIso: string; labels: string[]; url: string
}

export interface IssuesResponse {
  issues: TelltaleIssueDTO[]
  errors: Array<{ project: string; message: string }>
}

/** Explicit whitelist. A `telltale:*` prefix parse would yield 'muted'. */
function kindOf(labels: string[]): 'bug' | 'crash' | 'unknown' {
  if (labels.includes('telltale:crash')) return 'crash'
  if (labels.includes('telltale:bug')) return 'bug'
  return 'unknown'
}

export function toDto(repo: string, project: string, i: RawIssue): TelltaleIssueDTO {
  const labels = i.labels.map((l) => l.name)
  return {
    repo, number: i.number, title: i.title, body: i.body ?? '',
    kind: kindOf(labels), project,
    isOpen: i.state === 'open',
    hasAssignee: i.assignee !== null && i.assignee !== undefined,
    createdIso: i.created_at, updatedIso: i.updated_at,
    labels, url: i.html_url,
  }
}

/**
 * A missing secret denies everything. Interpolating an unset OPERATOR_READ_TOKEN
 * makes the expected header the literal string "Bearer undefined" — a guessable
 * constant that would serve every bug-report body in every private registry repo
 * to anyone who sends it. Never "accept a guessable constant"; always "deny".
 */
function unauthorized(req: Request, env: Env): boolean {
  if (!env.OPERATOR_READ_TOKEN) return true
  return !equals(req.headers.get('Authorization') ?? '', `Bearer ${env.OPERATOR_READ_TOKEN}`)
}

export async function handleIssues(
  req: Request, env: Env, deps: { gh: (entry: RegistryEntry) => GitHubClient },
): Promise<Response> {
  if (unauthorized(req, env)) {
    return new Response(JSON.stringify({ error: 'unauthorized' }), {
      status: 401, headers: { 'content-type': 'application/json' },
    })
  }

  const issues: TelltaleIssueDTO[] = []
  const errors: IssuesResponse['errors'] = []

  for (const [project, entry] of Object.entries(REGISTRY)) {
    if (project === '__probe__') continue
    try {
      const raw = await deps.gh(entry).listTelltaleIssues(entry.repo)
      // GET /issues returns pull requests as issues.
      for (const i of raw) {
        if (i.pull_request === undefined) issues.push(toDto(entry.repo, project, i))
      }
    } catch (e) {
      // Degrade only the affected project, never the whole lane.
      errors.push({ project, message: e instanceof Error ? e.message : 'unknown' })
    }
  }

  return new Response(JSON.stringify({ issues, errors } satisfies IssuesResponse), {
    status: 200,
    headers: { 'content-type': 'application/json', 'cache-control': 'max-age=60' },
  })
}

export async function handleStats(req: Request, env: Env): Promise<Response> {
  if (unauthorized(req, env)) {
    return new Response(JSON.stringify({ error: 'unauthorized' }), {
      status: 401, headers: { 'content-type': 'application/json' },
    })
  }
  return new Response(JSON.stringify(await readStats(env.TELLTALE_KV)), {
    status: 200, headers: { 'content-type': 'application/json' },
  })
}
