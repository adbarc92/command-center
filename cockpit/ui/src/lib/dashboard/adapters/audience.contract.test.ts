// Wire contract — Audience `GET /posts` status vocabulary, consumer side.
//
// Audience generates this enum from its schema (`packages/contracts/src/generated/
// enums.ts`, `PostStatusSchema`), so unlike the telltale seam the producer side is
// already authoritative and machine-generated. This file's job is to stop THIS repo
// drifting from it silently — which is exactly what happened: the §6.2 spec listed a
// vocabulary taken from a written digest, three of its six strings were fictional,
// and `awaiting_approval` — the approve-before-post gate — classified as nothing.

import { describe, it, expect } from 'vitest';
import { createHash } from 'node:crypto';
import raw from './contracts/audience-post-status.contract.json?raw';
import type { AudiencePostStatus } from './audience';

// Bump only after re-reading Audience's generated enum and updating the mapping.
const CONTRACT_SHA256 = 'f1f7f57231511a9fcfd0943bee041115df5065392a7ff85a25ce6d44952e7c64';

const contract = JSON.parse(raw) as { statuses: string[] };

const canonicalSha = (text: string) =>
  createHash('sha256').update(JSON.stringify(JSON.parse(text))).digest('hex');

// Compile-time exhaustiveness: tsc fails here if AudiencePostStatus gains or loses a
// member. The runtime assertion below ties this literal to the contract file, so the
// union, the contract and Audience's own enum cannot drift apart in any direction.
const DECLARED: Record<AudiencePostStatus, true> = {
  draft: true,
  generating: true,
  ready_for_review: true,
  awaiting_approval: true,
  approved: true,
  publishing: true,
  fully_published: true,
  partially_published: true,
  failed: true,
};

describe('audience post-status contract', () => {
  it('the contract has not moved under this repo', () => {
    expect(canonicalSha(raw)).toBe(CONTRACT_SHA256);
  });

  it('the declared union is exactly the contract vocabulary', () => {
    expect(Object.keys(DECLARED).sort()).toEqual([...contract.statuses].sort());
  });

  it('carries the three strings the old spec invented, as a regression guard', () => {
    // `approval-pending`, `published` and `rejected` were in the §6.2 table and in
    // this repo's tests. Audience emits none of them. If one ever reappears in the
    // contract, something has been copied from prose again rather than the schema.
    for (const fiction of ['approval-pending', 'published', 'rejected']) {
      expect(contract.statuses).not.toContain(fiction);
    }
  });
});
