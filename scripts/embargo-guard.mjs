#!/usr/bin/env node
// Embargo guard — blocks forbidden tokens from entering the repo.
//
// WHY THIS EXISTS: docs/STATUS.md once carried an attestation that an embargoed
// name was absent, and named that name inline to say so. The attestation was
// itself the leak, and it sat on the public default branch (and in code search)
// until someone happened to read it. A guard is the only thing that makes that
// class of mistake non-repeatable.
//
// THE AWKWARD PART: a guard that greps for a forbidden string has to contain the
// forbidden string, which recreates the exact problem it is solving.
//
// WHY THE DIGESTS ARE NOT COMMITTED: the first cut of this guard shipped salted
// SHA-256 digests in a tracked file, reasoning that a digest is not plaintext. It
// is not, but that is the wrong bar. The inputs here are LOW ENTROPY — a name, an
// email, a phone number — and the salt has to ship next to the digest for the
// guard to work, so it stops rainbow tables and nothing else. Measured: the
// 10-digit phone digest fell to a targeted search in 22.6 SECONDS on one CPU core
// (~9.1M candidates, single-threaded). A committed digest of a low-entropy secret
// is a slow-release copy of that secret.
//
// So the denylist lives OUTSIDE the repo, and the guard resolves it from, in order:
//   1. $EMBARGO_GUARD_CONFIG       — inline JSON (CI injects a repo secret)
//   2. $EMBARGO_GUARD_CONFIG_FILE  — path to a JSON file
//   3. .embargo-guard.local.json   — untracked, gitignored (local default)
// Nothing about the forbidden tokens is committed: not the plaintext, not a
// regex, not a digest, not a length.
//
// Matching is digest-based against a window of normalized text. Normalization
// (lowercase, then delete everything outside [a-z0-9]) is what makes it robust:
// case, spacing, punctuation, markdown emphasis and a hard line wrap all collapse
// to the same form, so "Foo Bar", "foo-bar", "**FooBar**" and a "Foo\nBar" line
// break are all caught by one digest.
//
// Usage:
//   node scripts/embargo-guard.mjs --staged            # pre-commit: staged blobs
//   node scripts/embargo-guard.mjs --message <file>    # commit-msg: the message
//   node scripts/embargo-guard.mjs --all               # CI: every tracked file
//   EG_TOKEN='...' node scripts/embargo-guard.mjs --add-entry <id>
//
// Exit 0 = clean, exit 1 = violation or the guard could not run. It fails CLOSED:
// a missing or malformed denylist blocks the commit rather than waving it through,
// because a guard that silently disables itself is worse than no guard at all.

import { createHash, randomBytes } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { readFileSync, writeFileSync, existsSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const LOCAL_CONFIG_NAME = '.embargo-guard.local.json';

// Files above this size are almost certainly build output or vendored blobs. We
// report every skip rather than passing silently — a guard that quietly declines
// to look at something reads as "clean" when it never checked.
const MAX_BYTES = 8 * 1024 * 1024;

function fail(msg) {
  console.error(`embargo-guard: ${msg}`);
  process.exit(1);
}

function git(args, opts = {}) {
  return execFileSync('git', args, { maxBuffer: 512 * 1024 * 1024, ...opts });
}

const repoRoot = git(['rev-parse', '--show-toplevel'], { encoding: 'utf8' }).trim();

// Where this script lives, which is NOT always the repo root: in a git worktree,
// --show-toplevel is that worktree, and if its branch predates the guard it has
// no scripts/ or denylist of its own. The hooks resolve this script by their own
// path, so fall back to a denylist sitting beside it — otherwise every commit in
// an older worktree fails closed for want of a config that is one directory away.
const scriptRoot = join(dirname(fileURLToPath(import.meta.url)), '..');
const localConfigPath = join(repoRoot, LOCAL_CONFIG_NAME);
const scriptConfigPath = join(scriptRoot, LOCAL_CONFIG_NAME);

const SETUP_HINT = [
  'No denylist found. It is deliberately not committed — see the header of this',
  'script. Provide one of:',
  '  - $EMBARGO_GUARD_CONFIG      (inline JSON; how CI injects its secret)',
  '  - $EMBARGO_GUARD_CONFIG_FILE (path to a JSON file)',
  `  - ${LOCAL_CONFIG_NAME}       (untracked, gitignored)`,
  'Seed the local one with:  EG_TOKEN=\'...\' node scripts/embargo-guard.mjs --add-entry <id>',
].join('\n  ');

/** Resolve the denylist from env or the untracked local file. Never from the tree. */
function readRawConfig() {
  if (process.env.EMBARGO_GUARD_CONFIG) {
    return { text: process.env.EMBARGO_GUARD_CONFIG, source: '$EMBARGO_GUARD_CONFIG' };
  }
  const fromFile = process.env.EMBARGO_GUARD_CONFIG_FILE;
  if (fromFile) {
    if (!existsSync(fromFile)) fail(`$EMBARGO_GUARD_CONFIG_FILE points at ${fromFile}, which does not exist. Fail closed.`);
    return { text: readFileSync(fromFile, 'utf8'), source: fromFile };
  }
  for (const candidate of [localConfigPath, scriptConfigPath]) {
    if (existsSync(candidate)) {
      return { text: readFileSync(candidate, 'utf8'), source: candidate };
    }
  }
  fail(SETUP_HINT);
}

function loadConfig() {
  const { text, source } = readRawConfig();
  let cfg;
  try {
    cfg = JSON.parse(text);
  } catch (err) {
    fail(`denylist from ${source} is not valid JSON (${err.message}). Fail closed.`);
  }
  const entries = cfg.entries;
  if (!Array.isArray(entries) || entries.length === 0) {
    fail(`denylist from ${source} declares no entries. Fail closed.`);
  }
  for (const e of entries) {
    if (!e.id || !Number.isInteger(e.length) || e.length < 1 || !/^[0-9a-f]+$/i.test(e.salt || '') || !/^[0-9a-f]{64}$/i.test(e.digest || '')) {
      fail(`denylist entry ${JSON.stringify(e.id ?? '?')} is malformed. Fail closed.`);
    }
  }
  return cfg;
}

/** Lowercase, then strip every character outside [a-z0-9]. */
function normalize(text) {
  return text.toLowerCase().replace(/[^a-z0-9]/g, '');
}

/**
 * Slide a window of entry.length over the normalized text, comparing the salted
 * digest of each window. Returns { entry, normIndex } on the first hit, else null.
 */
function scan(text, entries) {
  const norm = normalize(text);
  if (!norm) return null;
  // latin1 because `norm` is pure ASCII by construction; subarray then gives a
  // zero-copy view per window instead of allocating a string 2M times.
  const buf = Buffer.from(norm, 'latin1');

  for (const entry of entries) {
    if (buf.length < entry.length) continue;
    const salt = Buffer.from(entry.salt, 'hex');
    const last = buf.length - entry.length;
    for (let i = 0; i <= last; i++) {
      const h = createHash('sha256');
      h.update(salt);
      h.update(buf.subarray(i, i + entry.length));
      if (h.digest('hex') === entry.digest) return { entry, normIndex: i };
    }
  }
  return null;
}

/**
 * Map a normalized-string index back to a 1-based line number in the original
 * text. Only called on a hit, so the slow character walk costs nothing normally.
 */
function locateLine(text, normIndex) {
  let seen = 0;
  let line = 1;
  for (let i = 0; i < text.length; i++) {
    const ch = text[i];
    if (ch === '\n') line++;
    const c = ch.toLowerCase();
    if ((c >= 'a' && c <= 'z') || (c >= '0' && c <= '9')) {
      if (seen === normIndex) return line;
      seen++;
    }
  }
  return line;
}

function isBinary(buf) {
  return buf.subarray(0, 8000).includes(0);
}

/** Report a violation. Deliberately prints location only, never the matched text
 *  — CI logs on a public repo are public, so echoing the hit would re-leak it. */
function report(hits, skipped) {
  console.error('');
  console.error('  EMBARGO GUARD: blocked — forbidden token found.');
  console.error('');
  for (const h of hits) {
    console.error(`    ${h.where}:${h.line}  (matches entry "${h.id}")`);
  }
  console.error('');
  console.error('  The matched text is not printed here on purpose: this output');
  console.error('  reaches public CI logs, and echoing it would re-leak it.');
  console.error('');
  console.error('  Remove the token. Do not describe it by name in an attestation,');
  console.error('  a comment, or a commit message — use a placeholder. There is no');
  console.error('  allowlist: an exemption would just be another committed copy.');
  console.error('');
  if (skipped.length) console.error(`  (skipped ${skipped.length} file(s): ${skipped.join(', ')})`);
  process.exit(1);
}

function finish(hits, skipped, scannedCount, label) {
  if (hits.length) report(hits, skipped);
  for (const s of skipped) console.warn(`embargo-guard: skipped ${s}`);
  console.log(`embargo-guard: clean (${scannedCount} ${label} scanned).`);
  process.exit(0);
}

// --- modes -----------------------------------------------------------------

function runOverFiles(files, readContent, label) {
  const { entries } = loadConfig();
  const hits = [];
  const skipped = [];
  let scanned = 0;

  for (const file of files) {
    let buf;
    try {
      buf = readContent(file);
    } catch (err) {
      skipped.push(`${file} (unreadable: ${err.message})`);
      continue;
    }
    if (buf.length > MAX_BYTES) {
      skipped.push(`${file} (over ${MAX_BYTES} bytes)`);
      continue;
    }
    if (isBinary(buf)) continue; // binary: not a plausible carrier, not counted
    scanned++;
    const text = buf.toString('utf8');
    const hit = scan(text, entries);
    if (hit) hits.push({ where: file, line: locateLine(text, hit.normIndex), id: hit.entry.id });
  }
  finish(hits, skipped, scanned, label);
}

function modeStaged() {
  const out = git(['diff', '--cached', '--name-only', '--diff-filter=ACMR', '-z'], { encoding: 'utf8' });
  const files = out.split('\0').filter(Boolean);
  if (files.length === 0) {
    console.log('embargo-guard: no staged files.');
    process.exit(0);
  }
  // Read the STAGED blob, not the working tree — otherwise a bad version could be
  // staged and then cleaned up on disk before committing, and the guard would miss it.
  runOverFiles(files, (f) => git(['show', `:${f}`]), 'staged file(s)');
}

function modeAll() {
  const out = git(['ls-files', '-z'], { encoding: 'utf8' });
  const files = out.split('\0').filter(Boolean);
  runOverFiles(files, (f) => readFileSync(join(repoRoot, f)), 'tracked file(s)');
}

function modeMessage(path) {
  if (!path) fail('--message needs a path to the commit message file.');
  runOverFiles([path], (f) => readFileSync(f), 'commit message');
}

// Writes to the untracked local denylist, creating it if absent. The plaintext is
// read from the environment and never written anywhere.
function modeAddEntry(id) {
  if (!id) fail('--add-entry needs an id.');
  const token = process.env.EG_TOKEN;
  if (!token) fail('set EG_TOKEN to the token to forbid (it is never written to disk).');
  const norm = normalize(token);
  if (!norm) fail('EG_TOKEN normalizes to nothing.');

  const cfg = existsSync(localConfigPath)
    ? JSON.parse(readFileSync(localConfigPath, 'utf8'))
    : { _comment: `Untracked denylist for scripts/embargo-guard.mjs. Digests only, and NOT committed: these tokens are low-entropy, so a committed digest is a crackable copy of the token. Keep ${LOCAL_CONFIG_NAME} gitignored.`, algorithm: 'sha256', entries: [] };

  const salt = randomBytes(16);
  const digest = createHash('sha256').update(salt).update(Buffer.from(norm, 'latin1')).digest('hex');
  if (cfg.entries.some((e) => e.length === norm.length && e.digest === digest)) {
    fail('that token is already covered.');
  }
  cfg.entries.push({ id, length: norm.length, salt: salt.toString('hex'), digest });
  writeFileSync(localConfigPath, `${JSON.stringify(cfg, null, 2)}\n`);
  console.log(`embargo-guard: added entry "${id}" to ${LOCAL_CONFIG_NAME} (untracked).`);
}

const [mode, arg] = process.argv.slice(2);
switch (mode) {
  case '--staged': modeStaged(); break;
  case '--all': modeAll(); break;
  case '--message': modeMessage(arg); break;
  case '--add-entry': modeAddEntry(arg); break;
  default:
    fail('usage: --staged | --all | --message <file> | --add-entry <id>');
}
