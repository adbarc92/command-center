import fs from "node:fs";
import path from "node:path";
import { execFileSync } from "node:child_process";

export function parsePorcelainV2(text) {
  let branch = null, detached = false; const dirty = [];
  for (const line of text.split(/\r?\n/)) {
    if (line.startsWith("# branch.head ")) {
      const head = line.slice("# branch.head ".length).trim();
      if (head === "(detached)") { detached = true; branch = null; } else branch = head;
    } else if (line.startsWith("1 ") || line.startsWith("2 ")) {
      const left = line.split("\t")[0];
      dirty.push(left.split(" ").pop());
    } else if (line.startsWith("u ")) {
      dirty.push(line.split("\t")[0].split(" ").pop());
    } else if (line.startsWith("? ")) {
      dirty.push(line.slice(2).trim());
    }
  }
  return { branch, detached, dirty };
}

export function inProgress(gitDir) {
  const has = (n) => fs.existsSync(path.join(gitDir, n));
  if (has("rebase-merge") || has("rebase-apply")) return "rebase";
  if (has("MERGE_HEAD")) return "merge";
  if (has("BISECT_LOG")) return "bisect";
  return null;
}

function git(cwd, args) {
  return execFileSync("git", ["-C", cwd, ...args], { encoding: "utf8", timeout: 5000 });
}

export function collectGitFacts(cwd, cap = 50) {
  try { execFileSync("git", ["--version"], { timeout: 5000 }); }
  catch { return { branch: null, detached: false, in_progress: null, head: null, dirty: [], worktree: null, git_unavailable: true }; }
  let top;
  try { top = git(cwd, ["rev-parse", "--show-toplevel"]).trim(); }
  catch { return null; } // not a git repo
  if (!top) return null;
  const status = (() => { try { return git(cwd, ["--no-optional-locks", "status", "--porcelain=v2", "--branch"]); } catch { return ""; } })();
  const parsed = parsePorcelainV2(status);
  let head = null;
  try { head = git(cwd, ["log", "-1", "--format=%h %s"]).trim(); } catch {}
  let gitDir = path.join(cwd, ".git");
  try { gitDir = path.resolve(cwd, git(cwd, ["rev-parse", "--git-dir"]).trim()); } catch {}
  let worktree = null;
  try {
    const common = git(cwd, ["rev-parse", "--path-format=absolute", "--git-common-dir"]).trim();
    if (common && path.dirname(common) !== top) worktree = path.basename(top);
  } catch {}
  return {
    branch: parsed.branch, detached: parsed.detached, in_progress: inProgress(gitDir),
    head, dirty: parsed.dirty.slice(0, cap), worktree, git_unavailable: false,
  };
}
