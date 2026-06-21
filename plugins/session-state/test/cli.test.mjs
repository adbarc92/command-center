import { test } from "node:test";
import assert from "node:assert";
import { execFileSync } from "node:child_process";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import * as store from "../src/store.mjs";
import * as keying from "../src/keying.mjs";

const CLI = path.join(path.dirname(fileURLToPath(import.meta.url)), "..", "src", "cli.mjs");
const node = process.execPath;

function gitRepo() {
  const d = mkdtempSync(path.join(tmpdir(), "repo-"));
  for (const a of [["init", "-q"], ["config", "user.email", "t@t"], ["config", "user.name", "t"]]) execFileSync("git", a, { cwd: d });
  writeFileSync(path.join(d, "f.txt"), "x"); execFileSync("git", ["add", "."], { cwd: d }); execFileSync("git", ["commit", "-qm", "i"], { cwd: d });
  return d;
}

test("cli show renders state", () => {
  const home = mkdtempSync(path.join(tmpdir(), "cc-")), repo = gitRepo();
  process.env.CLAUDE_CONFIG_DIR = home;
  store.appendRecord(keying.stateDir(repo), store.makeRecord("rich", "save-state", "s", repo, { branch: "main", head: "abc x", dirty: [] }, { did: "hello", next: [], open_threads: [] }));
  delete process.env.CLAUDE_CONFIG_DIR;
  const out = execFileSync(node, [CLI, "show"], { cwd: repo, encoding: "utf8", env: { ...process.env, CLAUDE_CONFIG_DIR: home } });
  assert.ok(out.includes("hello"));
});

test("cli list runs", () => {
  const home = mkdtempSync(path.join(tmpdir(), "cc-")), repo = gitRepo();
  execFileSync(node, [CLI, "list"], { cwd: repo, encoding: "utf8", env: { ...process.env, CLAUDE_CONFIG_DIR: home } });
});
