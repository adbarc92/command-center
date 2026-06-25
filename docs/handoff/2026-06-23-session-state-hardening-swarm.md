# Swarm Handoff — session-state plugin hardening (H1–H3)

> ⚠️ **SUPERSEDED (2026-06-25)** — H1–H3 were shipped via PR #31. The remaining hardening item is **H4**
> (keying path-separator spurious collision); see [`docs/ROADMAP.md`](../ROADMAP.md) and Lane H in
> [`2026-06-25-ship-readiness-swarm-handoff.md`](2026-06-25-ship-readiness-swarm-handoff.md). Kept for history.

**Source:** [`docs/ROADMAP.md` → Hardening backlog](../ROADMAP.md) (carried from PR #30).
**Date:** 2026-06-23
**Base branch:** `main` @ `f09f2ad` (PR #30 merged).
**Shape:** 3 independent single-file bugfixes → **3 parallel lanes, zero write-overlap, no shared-contract lane needed.**

> Each lane fixes one bug in one source file plus its own test file. The lanes do **not** share any
> writable file, so they integrate in any order. Reconciliation = run the whole plugin test suite.

## Dependency analysis

| Lane | Owns (exclusive write) | Reads (no write) | Collides with |
|---|---|---|---|
| A (H1) | `plugins/session-state/src/resolve.mjs`, `plugins/session-state/test/resolve.test.mjs` | `src/keying.mjs` (imports `claudeHome` only — does not modify it) | none |
| B (H2) | `plugins/session-state/src/lock.mjs`, `plugins/session-state/test/lock.test.mjs` | — | none |
| C (H3) | `plugins/session-state/src/keying.mjs`, `plugins/session-state/test/keying.test.mjs` | — | none |

**Note on A↔C:** Lane A *reads* `keying.mjs` (for `claudeHome`); Lane C *writes* `keying.mjs` (the
`checkMeta` function). Different functions, and A never modifies keying — so no write conflict. If
both land, the merged `keying.mjs` is exactly C's version and A still imports `claudeHome` unchanged.

**No version bump in any lane.** H1 is a *gate before* a `0.10.x` release but does not itself
release; leave `plugin.json` untouched.

---

## Lane A — H1: semver-aware version resolution   ·   ready
- **Scope:** Fix the cache-scan in `pluginInstallPath` so it picks the **highest semver** version dir,
  not the lexically-last one. Today `readdirSync(base).sort().reverse()` ranks `0.9.0` above `0.10.0`.
- **Owns (exclusive write):** `plugins/session-state/src/resolve.mjs`, `plugins/session-state/test/resolve.test.mjs`
- **Reads (no write):** `plugins/session-state/src/keying.mjs`
- **Shared contract:** none.
- **Depends on / blocks:** independent. (This is the 🔴 release gate before any `0.10.x` plugin release.)
- **Done when:** given cache dirs `["0.2.0","0.9.0","0.10.0"]` (in any readdir order), resolution
  selects `0.10.0`; existing resolve tests still pass.
- **Verify:** `node --test "plugins/session-state/test/resolve.test.mjs"`
- **Notes / open questions:** Write the failing test first (TDD red→green). Implement a small numeric
  semver comparator (split on `.`, compare numerically; tolerate pre-release/garbage dirs by sorting
  them last, not crashing). Keep the existing `installed_plugins.json` registry path (branch 1)
  untouched — only the cache-scan fallback (branch 2) is buggy. No new dependencies.

## Lane B — H2: steal torn/corrupt lock immediately   ·   ready
- **Scope:** In `withLock`, a corrupt/unparseable lock token (`readToken` → `null`) with a fresh
  mtime is currently not stolen until `maxAgeMs` (60s). A torn token should be stealable right away.
- **Owns (exclusive write):** `plugins/session-state/src/lock.mjs`, `plugins/session-state/test/lock.test.mjs`
- **Reads (no write):** —
- **Shared contract:** none.
- **Depends on / blocks:** independent.
- **Done when:** a lockfile containing non-JSON / truncated content is stolen on the next `withLock`
  attempt (no 60s wait); a *valid* token whose pid is still alive is still NOT stolen; ownership-checked
  release semantics unchanged.
- **Verify:** `node --test "plugins/session-state/test/lock.test.mjs"`
- **Notes / open questions:** Write the failing test first. The fix is at the steal decision
  (`lock.mjs:33-39`): treat `tok === null` (failed parse) as a stealable holder, the same as a dead
  pid. Be careful to distinguish "torn token" (steal) from "valid token, live pid" (wait) — don't
  regress the live-holder path. Keep the synchronous-sleep design; don't introduce async.

## Lane C — H3: don't flag COLLISION on malformed meta.json   ·   ready
- **Scope:** In `checkMeta`, a malformed `meta.json` leaves `existing = null`, so `null === repo`
  is false and a spurious `COLLISION` marker is written for what is actually the same repo.
- **Owns (exclusive write):** `plugins/session-state/src/keying.mjs`, `plugins/session-state/test/keying.test.mjs`
- **Reads (no write):** —
- **Shared contract:** none.
- **Depends on / blocks:** independent.
- **Done when:** a malformed/unparseable `meta.json` does NOT produce a `COLLISION` file — it is
  treated as recoverable (rewrite the meta with the current repo and return `true`); a meta with a
  *genuinely different* repo string still writes `COLLISION` and returns `false`.
- **Verify:** `node --test "plugins/session-state/test/keying.test.mjs"`
- **Notes / open questions:** Write the failing test first. The fix is at `keying.mjs:46-48`:
  separate "parse failed" (corrupt → heal by rewriting meta, no collision) from "parsed, repo
  differs" (real collision). Don't change `claudeHome`/`repoKey`/`stateDir` — Lane A imports
  `claudeHome` and must keep working. Preserve the COLLISION behavior for the real-mismatch case.

---

## Rules of the road (give to every dispatched agent)
1. **Stay in your lane.** Write only the files your lane owns. Need a change elsewhere? Report it; don't make it.
2. **Worktree/branch per lane.** Never commit to `main`. Branch from `main` @ `f09f2ad`.
3. **TDD:** failing test first (red), then the fix (green). Report the real verify output, not an assertion.
4. **Don't widen scope.** Only your H-item. Anything else you spot → report, don't fix.
5. **Report for integration:** files changed, any contract requests (expected: none), verify output.

## Integration order
- Lanes A, B, C are non-overlapping → merge in **any order**.
- **Reconcile:** run the full suite over the merged whole —
  `node --test "plugins/session-state/test/*.test.mjs"` (expect 40 + new cases, all green).
- No shared-contract files to assemble; no `plugin.json` change. If A lands, **H1 release-gate is cleared**.
