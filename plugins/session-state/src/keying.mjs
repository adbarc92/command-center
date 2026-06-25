import { homedir } from "node:os";
import path from "node:path";
import fs from "node:fs";
import { execFileSync } from "node:child_process";

export function claudeHome() {
  return process.env.CLAUDE_CONFIG_DIR || path.join(homedir(), ".claude");
}

// Collapse "<root>/.claude/worktrees/<name>" back to "<root>" (string-based; sep-agnostic).
export function canonicalProjectRoot(cwd) {
  const m = cwd.match(/^(.*)[\\/]\.claude[\\/]worktrees[\\/][^\\/]+(?:[\\/].*)?$/);
  return m ? m[1] : cwd;
}

export function pathToSlug(p) {
  return p.replace(/[\\/:]/g, "-");
}

// Canonicalize separators for repo-path comparison/keying only (display is unaffected):
// "\" and "/" denote the same repo, so collapse to a single canonical separator before
// any compare. Applied to BOTH sides so a sep difference can never produce a mismatch.
function normSep(p) {
  return p == null ? p : p.replace(/\\/g, "/");
}

export function repoRoot(cwd) {
  try {
    const out = execFileSync("git", ["-C", cwd, "rev-parse", "--show-toplevel"],
      { encoding: "utf8", timeout: 5000 }).trim();
    return out ? canonicalProjectRoot(out) : null;
  } catch { return null; }
}

export function repoKey(cwd) {
  return pathToSlug(repoRoot(cwd) || cwd);
}

export function stateDir(cwd, { create = true } = {}) {
  const d = path.join(claudeHome(), "state", "sessions", repoKey(cwd));
  if (create) fs.mkdirSync(path.join(d, "scratch"), { recursive: true });
  return d;
}

export function checkMeta(dir, canonicalRepo) {
  const meta = path.join(dir, "meta.json");
  if (!fs.existsSync(meta)) {
    fs.mkdirSync(dir, { recursive: true });
    fs.writeFileSync(meta, JSON.stringify({ repo: canonicalRepo }));
    return true;
  }
  let existing = null;
  let parsed = false;
  try {
    existing = JSON.parse(fs.readFileSync(meta, "utf8")).repo;
    parsed = true;
  } catch {}
  // Corrupt/unparseable meta, or a parsed meta with no repo recorded, is recoverable:
  // heal it by rewriting with the current repo. Only a parsed meta that records a
  // genuinely different repo is a real COLLISION.
  if (!parsed || existing == null) {
    fs.writeFileSync(meta, JSON.stringify({ repo: canonicalRepo }));
    return true;
  }
  // Compare separator-normalized: a "\" vs "/" difference is the same repo, not a collision.
  if (normSep(existing) === normSep(canonicalRepo)) return true;
  fs.writeFileSync(path.join(dir, "COLLISION"), `expected ${existing} got ${canonicalRepo}`);
  return false;
}
