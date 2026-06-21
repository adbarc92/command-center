import { test } from "node:test";
import assert from "node:assert";
import { mkdtempSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import * as k from "../src/keying.mjs";

test("pathToSlug replaces separators and drive colon", () => {
  assert.equal(k.pathToSlug("D:\\MajorProjects\\CURRENT\\command-center"),
    "D--MajorProjects-CURRENT-command-center");
});

test("canonicalProjectRoot collapses .claude/worktrees/<name>", () => {
  assert.equal(k.canonicalProjectRoot("/repo/.claude/worktrees/agent-x"), "/repo");
  assert.equal(k.canonicalProjectRoot("/repo/src"), "/repo/src");
});

test("repoKey for non-git dir uses slug of cwd", () => {
  const d = mkdtempSync(path.join(tmpdir(), "ss-"));
  assert.equal(k.repoKey(d), k.pathToSlug(d));
});

test("stateDir created under claude home, checkMeta detects collision", () => {
  const home = mkdtempSync(path.join(tmpdir(), "cc-"));
  process.env.CLAUDE_CONFIG_DIR = home;
  const d = k.stateDir(mkdtempSync(path.join(tmpdir(), "ss-")));
  assert.ok(existsSync(d));
  assert.equal(path.basename(path.dirname(d)), "sessions");
  assert.equal(k.checkMeta(d, "D:/repo-a"), true);
  assert.equal(k.checkMeta(d, "D:/repo-a"), true);
  assert.equal(k.checkMeta(d, "D:/other"), false);
  assert.ok(existsSync(path.join(d, "COLLISION")));
  delete process.env.CLAUDE_CONFIG_DIR;
});
