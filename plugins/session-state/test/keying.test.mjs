import { test } from "node:test";
import assert from "node:assert";
import { mkdtempSync, existsSync, writeFileSync, readFileSync } from "node:fs";
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

test("checkMeta heals a malformed meta.json without flagging COLLISION", () => {
  const d = mkdtempSync(path.join(tmpdir(), "ss-"));
  const meta = path.join(d, "meta.json");
  writeFileSync(meta, "{ this is not valid json ");
  assert.equal(k.checkMeta(d, "D:/repo-a"), true);
  // No collision marker for corrupt meta — it's recoverable.
  assert.ok(!existsSync(path.join(d, "COLLISION")));
  // Meta was rewritten with the current repo.
  assert.equal(JSON.parse(readFileSync(meta, "utf8")).repo, "D:/repo-a");
});

test("checkMeta heals a meta.json missing the repo field without flagging COLLISION", () => {
  const d = mkdtempSync(path.join(tmpdir(), "ss-"));
  const meta = path.join(d, "meta.json");
  writeFileSync(meta, JSON.stringify({ somethingElse: 1 }));
  assert.equal(k.checkMeta(d, "D:/repo-a"), true);
  assert.ok(!existsSync(path.join(d, "COLLISION")));
  assert.equal(JSON.parse(readFileSync(meta, "utf8")).repo, "D:/repo-a");
});

test("checkMeta still flags COLLISION for a genuinely different repo", () => {
  const d = mkdtempSync(path.join(tmpdir(), "ss-"));
  const meta = path.join(d, "meta.json");
  writeFileSync(meta, JSON.stringify({ repo: "D:/repo-a" }));
  assert.equal(k.checkMeta(d, "D:/repo-b"), false);
  assert.ok(existsSync(path.join(d, "COLLISION")));
});
