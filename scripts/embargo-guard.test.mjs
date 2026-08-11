// Tests for the embargo guard. Run: node --test scripts/embargo-guard.test.mjs
//
// Every case builds its own throwaway token at runtime and derives a denylist
// from it, so the fixtures never contain a real embargoed value — the same
// property the guard itself is built around. A test that needed the real token
// inline would reintroduce exactly the bug under test.
//
// Each case runs a COPY of the guard inside a scratch git repo, never the one in
// this checkout. That is what keeps it hermetic: the guard falls back to a local
// denylist beside the repo root and then beside its own script, so running the
// real script in place would silently pick up the developer's real denylist and
// mask every fail-closed assertion.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createHash, randomBytes } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { writeFileSync, mkdtempSync, rmSync, mkdirSync, copyFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const GUARD = join(dirname(fileURLToPath(import.meta.url)), 'embargo-guard.mjs');

/** A random lowercase token, so no real value is ever committed in a fixture. */
function throwawayToken(len = 14) {
  const alphabet = 'abcdefghijklmnopqrstuvwxyz';
  return Array.from(randomBytes(len), (b) => alphabet[b % 26]).join('');
}

function denylistFor(token, id = 'test-token') {
  const norm = token.toLowerCase().replace(/[^a-z0-9]/g, '');
  const salt = randomBytes(16);
  const digest = createHash('sha256').update(salt).update(Buffer.from(norm, 'latin1')).digest('hex');
  return JSON.stringify({ algorithm: 'sha256', entries: [{ id, length: norm.length, salt: salt.toString('hex'), digest }] });
}

/**
 * Run the guard against `content` inside a throwaway git repo.
 * mode 'staged' stages the file first (the pre-commit path); mode 'message'
 * points the guard straight at it (the commit-msg path).
 */
function runGuard(content, { denylist, mode = 'staged', filename = 'subject.md' } = {}) {
  const dir = mkdtempSync(join(tmpdir(), 'embargo-test-'));
  try {
    execFileSync('git', ['init', '-q'], { cwd: dir });
    // Run a copy, so both resolution roots (repo root and script dir) land inside
    // the scratch repo and no real denylist is reachable.
    mkdirSync(join(dir, 'scripts'));
    const guard = join(dir, 'scripts', 'embargo-guard.mjs');
    copyFileSync(GUARD, guard);
    const file = join(dir, filename);
    writeFileSync(file, content);

    const env = { ...process.env, EMBARGO_GUARD_CONFIG_FILE: '' };
    if (denylist === undefined) delete env.EMBARGO_GUARD_CONFIG;
    else env.EMBARGO_GUARD_CONFIG = denylist;

    const args = mode === 'staged' ? ['--staged'] : ['--message', file];
    if (mode === 'staged') execFileSync('git', ['add', filename], { cwd: dir });

    try {
      const stdout = execFileSync(process.execPath, [guard, ...args], {
        cwd: dir, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'], env,
      });
      return { code: 0, stdout, stderr: '' };
    } catch (err) {
      return { code: err.status, stdout: err.stdout ?? '', stderr: err.stderr ?? '' };
    }
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

test('blocks the token written contiguously', () => {
  const token = throwawayToken();
  const r = runGuard(`prose before ${token} prose after`, { denylist: denylistFor(token) });
  assert.equal(r.code, 1);
  assert.match(r.stderr, /EMBARGO GUARD: blocked/);
});

test('blocks the token split across a line break', () => {
  // The original leak wrapped mid-token in a markdown file; this is the case a
  // naive substring grep misses.
  const token = throwawayToken();
  const split = `${token.slice(0, 4)}\n${token.slice(4)}`;
  const r = runGuard(`a sentence that wraps at ${split} and continues`, { denylist: denylistFor(token) });
  assert.equal(r.code, 1);
});

test('blocks the token mangled with case and punctuation', () => {
  const token = throwawayToken();
  const mangled = token.split('').map((c, i) => (i % 2 ? c.toUpperCase() : c)).join('-');
  const r = runGuard(`sneaky: ${mangled}`, { denylist: denylistFor(token) });
  assert.equal(r.code, 1);
});

test('blocks the token broken up by markdown emphasis', () => {
  const token = throwawayToken();
  const r = runGuard(`**${token.slice(0, 3)}**${token.slice(3)}`, { denylist: denylistFor(token) });
  assert.equal(r.code, 1);
});

test('blocks the token in a commit message', () => {
  const token = throwawayToken();
  const r = runGuard(`chore: mentions ${token}`, { denylist: denylistFor(token), mode: 'message' });
  assert.equal(r.code, 1);
});

test('passes clean content, and does not flag a near-miss', () => {
  const token = throwawayToken();
  const nearMiss = `${token.slice(0, -1)}${token.at(-1) === 'a' ? 'b' : 'a'}`;
  const r = runGuard(`entirely unrelated prose, plus ${nearMiss}`, { denylist: denylistFor(token) });
  assert.equal(r.code, 0);
  assert.match(r.stdout, /clean/);
});

test('reports the location but never echoes the matched text', () => {
  // Load-bearing: this output reaches public CI logs. Printing the hit would
  // re-leak the exact thing the guard exists to keep out.
  const token = throwawayToken();
  const r = runGuard(`line one\nline two has ${token}\nline three`, { denylist: denylistFor(token) });
  assert.equal(r.code, 1);
  assert.match(r.stderr, /subject\.md:2/, 'should report file and line');
  assert.ok(!r.stderr.includes(token), 'must not print the matched token');
});

test('names which entry matched, so a multi-entry denylist is actionable', () => {
  const token = throwawayToken();
  const r = runGuard(`x ${token} y`, { denylist: denylistFor(token, 'some-id') });
  assert.match(r.stderr, /matches entry "some-id"/);
});

test('fails closed when no denylist is available', () => {
  const r = runGuard('harmless content', { denylist: undefined });
  assert.equal(r.code, 1, 'must not pass when it cannot check');
  assert.match(r.stderr, /No denylist found/);
});

test('finds a denylist beside the script when the repo root has none', () => {
  // Regression: in a git worktree whose branch predates the guard, the toplevel
  // has no denylist and no scripts/ — the hooks resolve the guard by their own
  // path, so it must fall back to the denylist beside itself. Before this, the
  // guard fell through to "no denylist" and (with a relative core.hooksPath, no
  // hook at all) a leaking commit sailed through in every worktree.
  const token = throwawayToken();
  const home = mkdtempSync(join(tmpdir(), 'embargo-guardhome-'));
  const repo = mkdtempSync(join(tmpdir(), 'embargo-worktree-'));
  try {
    mkdirSync(join(home, 'scripts'));
    const guard = join(home, 'scripts', 'embargo-guard.mjs');
    copyFileSync(GUARD, guard);
    // Denylist lives beside the guard, NOT in the repo being scanned.
    writeFileSync(join(home, '.embargo-guard.local.json'), denylistFor(token));

    execFileSync('git', ['init', '-q'], { cwd: repo });
    writeFileSync(join(repo, 'subject.md'), `leaking ${token} here`);
    execFileSync('git', ['add', 'subject.md'], { cwd: repo });

    const env = { ...process.env, EMBARGO_GUARD_CONFIG_FILE: '' };
    delete env.EMBARGO_GUARD_CONFIG;

    let code = 0;
    let stderr = '';
    try {
      execFileSync(process.execPath, [guard, '--staged'], { cwd: repo, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'], env });
    } catch (err) {
      code = err.status;
      stderr = err.stderr ?? '';
    }
    assert.equal(code, 1, 'must block');
    // Exit 1 alone proves nothing here: failing closed for want of a denylist
    // also exits 1. It has to block because it MATCHED.
    assert.match(stderr, /EMBARGO GUARD: blocked/, 'must block on a match, not by failing closed');
    assert.doesNotMatch(stderr, /No denylist found/, 'must have located the denylist beside the script');
  } finally {
    rmSync(home, { recursive: true, force: true });
    rmSync(repo, { recursive: true, force: true });
  }
});

test('fails closed on a malformed denylist', () => {
  const r = runGuard('harmless content', { denylist: 'not json at all' });
  assert.equal(r.code, 1);
  assert.match(r.stderr, /not valid JSON/);
});

test('fails closed on a denylist with no entries', () => {
  const r = runGuard('harmless content', { denylist: JSON.stringify({ entries: [] }) });
  assert.equal(r.code, 1);
  assert.match(r.stderr, /no entries/);
});

test('fails closed on a structurally invalid entry', () => {
  const bad = JSON.stringify({ entries: [{ id: 'x', length: 5, salt: 'zz', digest: 'short' }] });
  const r = runGuard('harmless content', { denylist: bad });
  assert.equal(r.code, 1);
  assert.match(r.stderr, /malformed/);
});
