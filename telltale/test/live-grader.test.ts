import { describe, it, expect } from 'vitest'
import { sign } from '../src/auth'

/**
 * The independent grader (spec §9.1), in Halyard's verify-launch spirit: it
 * asserts by READING THE SINK BACK, never by trusting the Worker's own success
 * report — the only way a dedup regression is caught.
 *
 * Gated like this repo's real-Docker integration tests: it needs live
 * credentials, so it is skipped unless they are present. It targets the
 * __probe__ registry entry and NEVER a product repo — an earlier design would
 * have published synthetic issues into a shipped product's public tracker.
 */

// No @types/node in this project (no runtime deps, and the rest of the
// Worker only ever sees `env` bindings, never `process`) — this is the one
// file that reads real process env vars, so it declares just enough of the
// shape to satisfy `tsc --noEmit` without pulling in a new dependency.
declare const process: { env: Record<string, string | undefined> }

const BASE = process.env.TELLTALE_BASE_URL
const SECRET = process.env.TELLTALE_PROBE_SECRET
const GH = process.env.TELLTALE_PROBE_GH_TOKEN
const REPO = process.env.TELLTALE_PROBE_REPO

const live = BASE && SECRET && GH && REPO ? describe : describe.skip

live('live grader', () => {
  it('collapses N identical reports into exactly one issue', async () => {
    const title = `grader ${crypto.randomUUID()}`
    const raw = JSON.stringify({ schema_version: 1, title, body: 'synthetic', reporter: { anon_id: 'grader' } })
    const ts = String(Math.floor(Date.now() / 1000))
    const sig = await sign(SECRET!, ts, raw)

    for (let i = 0; i < 3; i++) {
      const res = await fetch(`${BASE}/v1/events`, {
        method: 'POST',
        headers: {
          'X-Telltale-Project': '__probe__',
          'X-Telltale-Timestamp': ts,
          'X-Telltale-Signature': sig,
        },
        body: raw,
      })
      expect(res.status).toBe(202)
    }

    // Read the sink back, not the Worker's own report.
    const listed = await fetch(
      `https://api.github.com/repos/${REPO}/issues?state=all&per_page=100&labels=telltale`,
      { headers: { Authorization: `Bearer ${GH}`, Accept: 'application/vnd.github+json' } },
    )
    const issues = (await listed.json()) as Array<{ number: number; title: string }>
    const mine = issues.filter((i) => i.title.includes(title))
    expect(mine).toHaveLength(1)

    // Clean up so a re-run tests the create path again, not the dedup path.
    await fetch(`https://api.github.com/repos/${REPO}/issues/${mine[0]!.number}`, {
      method: 'PATCH',
      headers: { Authorization: `Bearer ${GH}`, Accept: 'application/vnd.github+json' },
      body: JSON.stringify({ state: 'closed', state_reason: 'not_planned' }),
    })
  }, 30_000)
})
