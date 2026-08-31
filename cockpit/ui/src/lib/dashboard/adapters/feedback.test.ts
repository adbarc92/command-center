import { describe, it, expect } from 'vitest';
import {
  feedbackCards,
  FEEDBACK_SOURCE,
  type FeedbackReader,
  type FeedbackIssuesResponse,
  type TelltaleIssue,
} from './feedback';

const NOW = () => new Date('2026-06-09T12:00:00Z');

function issue(over: Partial<TelltaleIssue> = {}): TelltaleIssue {
  return {
    repo: 'adbarc92/hexy',
    number: 1,
    title: 'crash on launch',
    body: '',
    kind: 'bug',
    project: 'hexy',
    isOpen: true,
    hasAssignee: false,
    createdIso: '2026-06-08T12:00:00Z', // 1 day old ⇒ "new this week"
    updatedIso: '2026-06-08T12:00:00Z',
    labels: ['telltale'],
    url: 'https://github.com/adbarc92/hexy/issues/1',
    ...over,
  };
}

function reader(res: Partial<FeedbackIssuesResponse>): FeedbackReader {
  return { issues: async () => ({ issues: [], errors: [], ...res }) };
}

// ── §6.4 cards ──────────────────────────────────────────────────────────────
describe('feedback adapter', () => {
  it('emits one card per project with at least one open issue', async () => {
    const cards = await feedbackCards(
      reader({
        issues: [
          issue({ project: 'hexy', number: 1 }),
          issue({ project: 'hexy', number: 2 }),
          issue({ project: 'tenzy', number: 3, repo: 'OpenBarclay/tenzy' }),
        ],
      }),
      { now: NOW },
    );

    expect(cards).toHaveLength(2);
    expect(cards.map((c) => c.projectId).sort()).toEqual(['feedback:hexy', 'feedback:tenzy']);
    expect(cards.every((c) => c.source === FEEDBACK_SOURCE)).toBe(true);
  });

  it('emits NO card for a project whose issues are all closed', async () => {
    // ~10 permanently-grey Idle cards would pile at the bottom of the grid and
    // inflate the header total — §6.4 says omit, not render empty.
    const cards = await feedbackCards(
      reader({ issues: [issue({ isOpen: false })] }),
      { now: NOW },
    );
    expect(cards).toEqual([]);
  });

  it('is Idle, never Build — one old bug must not outrank a live project', async () => {
    const [card] = await feedbackCards(reader({ issues: [issue()] }), { now: NOW });
    expect(card.stage).toBe('Idle');
    expect(card.blocked).toBeNull();
  });

  it('counts open issues and how many are new this week', async () => {
    const [card] = await feedbackCards(
      reader({
        issues: [
          issue({ number: 1, createdIso: '2026-06-08T12:00:00Z' }), // 1d  → new
          issue({ number: 2, createdIso: '2026-06-05T12:00:00Z' }), // 4d  → new
          issue({ number: 3, createdIso: '2026-04-01T12:00:00Z' }), // old → not
        ],
      }),
      { now: NOW },
    );
    expect(card.detail).toBe('3 open · 2 new this week');
  });

  it('Blocked ONLY for an open crash with no assignee', async () => {
    const [card] = await feedbackCards(
      reader({ issues: [issue({ kind: 'crash', hasAssignee: false })] }),
      { now: NOW },
    );
    expect(card.stage).toBe('Blocked');
    expect(card.detail).toBe('1 open · untriaged crash');
    expect(card.blocked).toEqual({
      gate: 'manual',
      action: 'triage crash report',
      deepLink: 'https://github.com/adbarc92/hexy/issues/1',
    });
  });

  it('an ASSIGNED crash is not Blocked — self-assigning clears the headline', async () => {
    // blockedCount is the board's "NEEDS YOU" number; a condition that never
    // clears is a permanent false positive on its most valuable signal.
    const [card] = await feedbackCards(
      reader({ issues: [issue({ kind: 'crash', hasAssignee: true })] }),
      { now: NOW },
    );
    expect(card.stage).toBe('Idle');
    expect(card.blocked).toBeNull();
  });

  it('an unassigned BUG is not Blocked — crash is the whitelist, not telltale:*', async () => {
    const [card] = await feedbackCards(
      reader({ issues: [issue({ kind: 'bug', hasAssignee: false })] }),
      { now: NOW },
    );
    expect(card.stage).toBe('Idle');
  });

  it('crash precedence wins over other open issues on the same project', async () => {
    const [card] = await feedbackCards(
      reader({
        issues: [
          issue({ number: 1, kind: 'bug' }),
          issue({ number: 2, kind: 'crash', hasAssignee: false, url: 'https://github.com/adbarc92/hexy/issues/2' }),
        ],
      }),
      { now: NOW },
    );
    expect(card.stage).toBe('Blocked');
    expect(card.blocked?.deepLink).toBe('https://github.com/adbarc92/hexy/issues/2');
  });

  // ── §6.3 partial failure ──────────────────────────────────────────────────
  it('one failing repo degrades only that project, never the lane', async () => {
    const cards = await feedbackCards(
      reader({
        issues: [issue({ project: 'hexy' })],
        errors: [{ project: 'pawsport', message: '403 archived' }],
      }),
      { now: NOW },
    );

    expect(cards).toHaveLength(2);
    const hexy = cards.find((c) => c.projectId === 'feedback:hexy')!;
    const paws = cards.find((c) => c.projectId === 'feedback:pawsport')!;

    expect(hexy.health).toBe('ok'); // the good repo is untouched
    expect(paws.health).toBe('unknown');
    expect(paws.detail).toBe('unreachable: 403 archived');
  });

  it('a reader that throws yields the single synthetic __source__ card', async () => {
    const cards = await feedbackCards(
      { issues: async () => { throw new Error('worker down'); } },
      { now: NOW },
    );
    expect(cards).toHaveLength(1);
    expect(cards[0].projectId).toBe('feedback:__source__');
    expect(cards[0].health).toBe('unknown');
    expect(cards[0].detail).toContain('worker down');
  });

  it('uses the §6.3 600s freshness budget', async () => {
    const [card] = await feedbackCards(reader({ issues: [issue()] }), { now: NOW });
    expect(card.staleAfterSec).toBe(600);
  });

  // ── §4 overrides, same contract as every other adapter ────────────────────
  it('an unexpired override wins and surfaces the conflict', async () => {
    const [card] = await feedbackCards(reader({ issues: [issue()] }), {
      now: NOW,
      overrides: {
        'feedback:hexy': {
          stage: 'Live',
          reason: 'shipped',
          setBy: 'alex',
          setAtIso: '2026-06-09T11:00:00Z',
        },
      },
    });
    expect(card.stage).toBe('Live');
    expect(card.stageSource).toBe('declared');
    expect(card.conflict).toEqual({ declared: 'Live', inferred: 'Idle' });
  });
});
