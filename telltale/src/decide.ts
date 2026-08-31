/**
 * The dedup decision table (spec §4.4, §5.3). Pure — no I/O, fully testable.
 *
 * Automated report-to-issue WITHOUT dedup is issue spam, and issue spam destroys
 * the tracker the pipeline exists to feed.
 */

export interface CandidateIssue {
  number: number
  state: 'open' | 'closed'
  stateReason: 'completed' | 'not_planned' | null
  labels: string[]
  isPullRequest: boolean
}

export type Decision =
  | { action: 'create' }
  | { action: 'comment'; issue: number }
  | { action: 'ignore'; reason: 'muted' | 'not_planned' }

export function decide(candidates: CandidateIssue[]): Decision {
  // GET /issues returns pull requests as issues. A labelled fix PR is not a report.
  const issues = candidates.filter((c) => !c.isPullRequest)
  if (issues.length === 0) return { action: 'create' }

  // Operator intent to stay silent wins over everything else.
  if (issues.some((i) => i.labels.includes('telltale:muted'))) {
    return { action: 'ignore', reason: 'muted' }
  }

  const open = issues.filter((i) => i.state === 'open').sort((a, b) => a.number - b.number)
  if (open.length > 0) return { action: 'comment', issue: open[0]!.number }

  const closed = [...issues].sort((a, b) => a.number - b.number)

  // Operator intent to stay silent wins over everything else — same rule as mute,
  // so it is checked across all closed candidates, not just the lowest-numbered one.
  if (closed.some((i) => i.stateReason === 'not_planned')) {
    return { action: 'ignore', reason: 'not_planned' }
  }

  // Closed as completed, or a legacy closure with no state_reason: comment so the
  // recurrence is recorded, but NEVER auto-reopen.
  return { action: 'comment', issue: closed[0]!.number }
}
