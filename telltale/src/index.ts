import type { Env, RegistryEntry } from './types'
import { lookup } from './registry'
import { parseEvent } from './schema'
import { scrubTitle, scrubBody } from './scrub'
import { fingerprint, labelFor } from './fingerprint'
import { verifySignature } from './auth'
import { decide } from './decide'
import { restClient, tokenFor, type GitHubClient } from './github'
import { hashIp, checkRateLimits, shouldComment, recordStat, type StatReason } from './kv'

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

  const secrets = JSON.parse(env.TELLTALE_SENDER_SECRETS) as Record<string, string>
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

    if (await shouldComment(kv, fp)) {
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
    const url = new URL(req.url)
    const deps: Deps = {
      gh: (entry) => restClient(tokenFor(env, entry)),
      nowMs: Date.now(),
    }
    if (req.method === 'POST' && url.pathname === '/v1/events') {
      return handleEvent(req, env, deps)
    }
    return json(404, { error: 'not_found' })
  },
}
