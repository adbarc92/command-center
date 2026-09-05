// Wire contract — telltale `GET /v1/issues`, consumer side.
//
// telltale declares this payload in `src/read.ts` (TelltaleIssueDTO / IssuesResponse);
// this repo declares it again in `feedback.ts` (TelltaleIssue / FeedbackIssuesResponse).
// The two agree today. Nothing made them keep agreeing, so either side could add,
// rename or drop a field with both suites green and the wire broken.
//
// The contract JSON is vendored byte-for-byte from NEXUS `docs/contracts/`, and
// telltale vendors the same file. Each side pins the contract's canonical hash. Moving
// the contract for one side therefore leaves the OTHER side's constant stale and red —
// which is the whole mechanism. See NEXUS `docs/contracts/README.md`.

import { describe, it, expect } from 'vitest';
import { createHash } from 'node:crypto';
// `?raw` keeps the file's own text, so the hash is over what is actually committed
// rather than over a re-serialization. `vite/client` types are already in
// tsconfig.app.json, so this needs no new declaration file.
import raw from './contracts/telltale-issues.contract.json?raw';
import {
  feedbackCards,
  type FeedbackReader,
  type FeedbackIssuesResponse,
  type TelltaleIssue,
} from './feedback';

// Bump ONLY together with telltale's copy of the same constant, and only after
// reading what actually changed on the wire.
const CONTRACT_SHA256 = '2f0e8cc1b55deac36014d3774db01e41194ecb2cdf32367c40a5a7f2c7b127ed';

const contract = JSON.parse(raw) as {
  issueFields: Record<string, string>;
  errorFields: Record<string, string>;
  sample: FeedbackIssuesResponse;
};

// Canonical, not raw bytes: a byte hash breaks the first time a repo checks this
// file out with CRLF, which on Windows clones is always.
const canonicalSha = (text: string) =>
  createHash('sha256').update(JSON.stringify(JSON.parse(text))).digest('hex');

// Compile-time exhaustiveness. tsc fails here if TelltaleIssue gains a field (this
// object is then missing a key) or loses one (this object then has an excess key).
// `npm run check` is where that fires; the runtime assertion below ties this literal
// to the contract file so the two cannot drift apart either.
const DECLARED_ISSUE_KEYS: Record<keyof TelltaleIssue, true> = {
  repo: true,
  number: true,
  title: true,
  body: true,
  kind: true,
  project: true,
  isOpen: true,
  hasAssignee: true,
  createdIso: true,
  updatedIso: true,
  labels: true,
  url: true,
};

type ErrorEntry = FeedbackIssuesResponse['errors'][number];
const DECLARED_ERROR_KEYS: Record<keyof ErrorEntry, true> = {
  project: true,
  message: true,
};

describe('telltale GET /v1/issues — wire contract', () => {
  it('the contract has not moved under this repo', () => {
    expect(canonicalSha(raw)).toBe(CONTRACT_SHA256);
  });

  it('TelltaleIssue declares exactly the contract issue fields — no more, no fewer', () => {
    expect(Object.keys(DECLARED_ISSUE_KEYS).sort()).toEqual(
      Object.keys(contract.issueFields).sort(),
    );
  });

  it('the per-project error entry declares exactly the contract error fields', () => {
    expect(Object.keys(DECLARED_ERROR_KEYS).sort()).toEqual(
      Object.keys(contract.errorFields).sort(),
    );
  });

  it('the contract sample carries exactly those fields on the wire, not merely a superset', () => {
    for (const issue of contract.sample.issues) {
      expect(Object.keys(issue).sort()).toEqual(Object.keys(contract.issueFields).sort());
    }
    for (const err of contract.sample.errors) {
      expect(Object.keys(err).sort()).toEqual(Object.keys(contract.errorFields).sort());
    }
  });

  it('the real adapter consumes a contract-shaped payload', async () => {
    // Not a shape assertion — this drives the actual production code path, so a
    // contract the adapter cannot read fails here rather than at runtime on the board.
    const reader: FeedbackReader = { issues: async () => contract.sample };
    const cards = await feedbackCards(reader, { now: () => new Date('2026-06-09T12:00:00Z') });

    expect(cards.length).toBeGreaterThan(0);
    // §6.3 — a project that failed to answer must not blank the lane; it is surfaced.
    expect(JSON.stringify(cards)).toContain('lineage');
  });
});
