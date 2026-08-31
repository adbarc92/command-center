import type { GitHubClient } from '../src/github'
import type { CandidateIssue } from '../src/decide'

/** Minimal in-memory KVNamespace stand-in. Enough for counters and TTL-less reads. */
export class FakeKV {
  store = new Map<string, string>()
  async get(k: string): Promise<string | null> { return this.store.get(k) ?? null }
  async put(k: string, v: string, _o?: { expirationTtl?: number }): Promise<void> { this.store.set(k, v) }
  async delete(k: string): Promise<void> { this.store.delete(k) }
  async list({ prefix }: { prefix: string }) {
    return { keys: [...this.store.keys()].filter((k) => k.startsWith(prefix)).map((name) => ({ name })) }
  }
}

export class FakeGitHub implements GitHubClient {
  issues: Array<CandidateIssue & { title: string; body: string }> = []
  comments: Array<{ number: number; body: string }> = []
  /** Simulates GitHub silently dropping ALL labels when the token lacks push access. */
  dropLabels = false
  /** Simulates a PARTIAL drop: this one label is omitted from the response, others survive. */
  dropLabel?: string
  private next = 1

  async findByLabel(_repo: string, label: string): Promise<CandidateIssue[]> {
    return this.issues.filter((i) => i.labels.includes(label))
  }

  async createIssue(_repo: string, i: { title: string; body: string; labels: string[] }) {
    const labels = this.dropLabels ? [] : i.labels.filter((l) => l !== this.dropLabel)
    const number = this.next++
    this.issues.push({
      number, state: 'open', stateReason: null, labels,
      isPullRequest: false, title: i.title, body: i.body,
    })
    // Content-based, matching the real client: a count comparison would miss
    // a partial drop where another label backfills the missing slot.
    return { number, url: `https://example.test/i/${number}`, labelsDropped: i.labels.some((l) => !labels.includes(l)) }
  }

  async commentIssue(_repo: string, number: number, body: string) {
    this.comments.push({ number, body })
  }

  async listTelltaleIssues() { return [] }
}
