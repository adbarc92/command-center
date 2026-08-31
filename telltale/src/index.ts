import type { Env, RegistryEntry } from './types'
import { lookup } from './registry'
import { parseEvent } from './schema'
import { scrubTitle, scrubBody } from './scrub'
import { fingerprint, labelFor } from './fingerprint'
import { verifySignature } from './auth'
import { decide } from './decide'
import { restClient, tokenFor, type GitHubClient } from './github'
import { hashIp, checkRateLimits, shouldComment, recordStat, type StatReason } from './kv'
import { handleIssues, handleStats } from './read'

export interface Deps {
  gh: (entry: RegistryEntry) => GitHubClient
  nowMs: number
}

function json(status: number, body: unknown, headers: Record<string, string> = {}): Response {
  return new Response(JSON.stringify(body), {
    status, headers: { 'content-type': 'application/json', ...headers },
  })
}

export async function handleEvent(req: Request, env: Env, deps: Deps): Promise<Response> {
  const kv = env.TELLTALE_KV
  const fail = async (status: number, reason: StatReason, extra?: Record<string, string>) => {
    await recordStat(kv, reason)
    return json(status, { error: reason }, extra)
  }

  const project = req.headers.get('X-Telltale-Project')
  if (!project) return fail(401, 'bad_signature')

  // A malformed operator-configured secrets blob is a deploy problem, not a
  // rejected request: it must still land in stats (the whole point of
  // /v1/stats is to make silent failure visible) and still return the same
  // { error: reason } envelope every other path returns, not a bare runtime
  // throw.
  let secrets: Record<string, string>
  try {
    secrets = JSON.parse(env.TELLTALE_SENDER_SECRETS) as Record<string, string>
  } catch {
    return fail(500, 'config_error')
  }
  const secret = secrets[project]
  const rawBody = await req.text()

  // 1. Auth. The header is the sole project authority.
  if (!secret) {
    // No sender secret provisioned for this slug — in practice that almost
    // always means the slug itself is unregistered (a typo), not a real
    // project missing its secret. Peek at the registry so a typo still gets
    // the loud 404 rather than being folded into every other auth failure.
    return lookup(project) ? fail(401, 'bad_signature') : fail(404, 'unregistered_project')
  }
  const auth = await verifySignature({
    secret,
    timestamp: req.headers.get('X-Telltale-Timestamp'),
    signature: req.headers.get('X-Telltale-Signature'),
    rawBody,
    nowMs: deps.nowMs,
  })
  if (!auth.ok) {
    // Hand back server time so the sender's single mandated retry can re-sign.
    // Device clock skew is common on Android and would otherwise fail silently.
    const extra = { 'X-Telltale-Server-Time': String(Math.floor(deps.nowMs / 1000)) }
    return fail(401, auth.reason === 'clock_skew' ? 'clock_skew' : 'bad_signature', extra)
  }

  // 2. Registry. An unregistered slug fails loudly rather than dropping silently.
  const entry = lookup(project)
  if (!entry) return fail(404, 'unregistered_project')

  // 3. Schema.
  let parsed
  try { parsed = parseEvent(JSON.parse(rawBody)) } catch { return fail(400, 'invalid_schema') }
  if (!parsed.ok) return fail(400, 'invalid_schema')
  const event = parsed.event

  // 4. Rate limits, on server-observed identity as well as the client's anon_id.
  const ipHash = await hashIp(req.headers.get('CF-Connecting-IP') ?? '0.0.0.0', env.IP_HASH_SALT)
  const rate = await checkRateLimits(kv, { ipHash, anonId: event.reporter?.anon_id ?? 'anon', project })
  if (!rate.ok) return fail(429, 'rate_limited', { 'Retry-After': '3600' })

  // 5. Scrub, THEN fingerprint — so identity is stable regardless of redaction.
  const title = scrubTitle(event.title)
  const body = scrubBody(event.body)
  const fp = await fingerprint(title)
  const label = labelFor(fp)

  const gh = deps.gh(entry)
  let candidates
  try { candidates = await gh.findByLabel(entry.repo, label) } catch { return fail(503, 'github_error') }

  const decision = decide(candidates)
  if (decision.action === 'ignore') return fail(200, 'ignored')

  // The spec knowingly accepts a concurrent-create race that can open two
  // issues for one fingerprint (§4.4). That acceptance is only defensible while
  // the operator can SEE it happening, so a multi-match lookup is counted
  // alongside the normal 'accepted'.
  if (candidates.filter((c) => !c.isPullRequest && c.state === 'open').length > 1) {
    await recordStat(kv, 'duplicate_fingerprint')
  }

  const footer =
    `\n\n---\n` +
    (event.release ? `Release: ${project}-${event.release.surface}@${event.release.version}\n` : '') +
    (event.context ? `Context: ${JSON.stringify(event.context)}\n` : '') +
    `<!-- telltale fingerprint=${fp} project=${project} -->`

  try {
    if (decision.action === 'create') {
      const created = await gh.createIssue(entry.repo, {
        title: `[bug] ${title}`,
        body: body + footer,
        labels: [...entry.labels, 'telltale:bug', label],
      })
      if (created.labelsDropped) {
        // Silent label loss breaks the idempotency key, the dedup key and the
        // read key at once, while otherwise reporting success. Fail loudly.
        await recordStat(kv, 'labels_dropped')
        return json(500, { error: 'labels_dropped', issue: created.number })
      }
      await recordStat(kv, 'accepted')
      return json(202, { issue: created.number, url: created.url })
    }

    if (await shouldComment(kv, project, fp)) {
      await gh.commentIssue(entry.repo, decision.issue, `Reported again.${footer}`)
    }
    await recordStat(kv, 'accepted')
    return json(202, { issue: decision.issue })
  } catch {
    return fail(503, 'github_error')
  }
}

export default {
  async fetch(req: Request, env: Env): Promise<Response> {
    // Top-level boundary. Anything that escapes here becomes Cloudflare's bare
    // 5xx HTML — no { error } envelope and no stat — which is exactly the
    // silent failure /v1/stats exists to eliminate. KV is the realistic source:
    // its counters are hot keys by construction and a blip must not take the
    // pipeline down with it.
    try {
      const url = new URL(req.url)
      const deps: Deps = {
        gh: (entry) => restClient(tokenFor(env, entry)),
        nowMs: Date.now(),
      }
      if (req.method === 'POST' && url.pathname === '/v1/events') {
        return await handleEvent(req, env, deps)
      }
      if (req.method === 'GET' && url.pathname === '/v1/issues') {
        return await handleIssues(req, env, deps)
      }
      if (req.method === 'GET' && url.pathname === '/v1/stats') {
        return await handleStats(req, env)
      }
      return json(404, { error: 'not_found' })
    } catch {
      return json(500, { error: 'internal' })
    }
  },
}
