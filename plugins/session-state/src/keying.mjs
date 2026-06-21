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
  try { existing = JSON.parse(fs.readFileSync(meta, "utf8")).repo; } catch {}
  if (existing === canonicalRepo) return true;
  fs.writeFileSync(path.join(dir, "COLLISION"), `expected ${existing} got ${canonicalRepo}`);
  return false;
}
