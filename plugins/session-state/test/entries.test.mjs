import { test } from "node:test";
import assert from "node:assert";
import { execFileSync } from "node:child_process";
import { mkdtempSync, writeFileSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import * as store from "../src/store.mjs";
import * as keying from "../src/keying.mjs";

const SRC = path.join(path.dirname(fileURLToPath(import.meta.url)), "..", "src");
const node = process.execPath;

function gitRepo() {
  const d = mkdtempSync(path.join(tmpdir(), "repo-"));
  execFileSync("git", ["init", "-q"], { cwd: d });
  execFileSync("git", ["config", "user.email", "t@t"], { cwd: d });
  execFileSync("git", ["config", "user.name", "t"], { cwd: d });
  writeFileSync(path.join(d, "f.txt"), "x");
  execFileSync("git", ["add", "."], { cwd: d });
  execFileSync("git", ["commit", "-qm", "init"], { cwd: d });
  return d;
}

function run(entry, { cwd, stdin = "", env = {} }) {
  return execFileSync(node, [path.join(SRC, entry)], { cwd, input: stdin, encoding: "utf8", env: { ...process.env, ...env } });
}

test("capture_scratch writes scratch", () => {
  const home = mkdtempSync(path.join(tmpdir(), "cc-")), repo = gitRepo();
  run("capture_scratch.mjs", { cwd: repo, stdin: JSON.stringify({ session_id: "s1" }), env: { CLAUDE_CONFIG_DIR: home } });
  process.env.CLAUDE_CONFIG_DIR = home;
  assert.equal(store.readScratches(keying.stateDir(repo)).length, 1);
  delete process.env.CLAUDE_CONFIG_DIR;
});

test("kill-switch no-ops capture_scratch", () => {
  const home = mkdtempSync(path.join(tmpdir(), "cc-")), repo = gitRepo();
  run("capture_scratch.mjs", { cwd: repo, stdin: JSON.stringify({ session_id: "s1" }), env: { CLAUDE_CONFIG_DIR: home, CC_SESSION_STATE_DISABLE: "1" } });
  process.env.CLAUDE_CONFIG_DIR = home;
  assert.equal(store.readScratches(keying.stateDir(repo)).length, 0);
  delete process.env.CLAUDE_CONFIG_DIR;
});

test("capture_end skips clear reason", () => {
  const home = mkdtempSync(path.join(tmpdir(), "cc-")), repo = gitRepo();
  run("capture_end.mjs", { cwd: repo, stdin: JSON.stringify({ session_id: "s", reason: "clear" }), env: { CLAUDE_CONFIG_DIR: home } });
  process.env.CLAUDE_CONFIG_DIR = home;
  assert.equal(store.readTimeline(keying.stateDir(repo)).length, 0);
  delete process.env.CLAUDE_CONFIG_DIR;
});

test("resume silent on compact, envelope on startup", () => {
  const home = mkdtempSync(path.join(tmpdir(), "cc-")), repo = gitRepo();
  assert.equal(run("resume.mjs", { cwd: repo, stdin: JSON.stringify({ source: "compact" }), env: { CLAUDE_CONFIG_DIR: home } }).trim(), "");
  // seed a record then resume on startup
  process.env.CLAUDE_CONFIG_DIR = home;
  const dir = keying.stateDir(repo);
  store.appendRecord(dir, store.makeRecord("rich", "save-state", "s", repo, { branch: "main", head: "abc x", dirty: [] }, { did: "hi", next: [], open_threads: [] }));
  delete process.env.CLAUDE_CONFIG_DIR;
  const out = run("resume.mjs", { cwd: repo, stdin: JSON.stringify({ source: "startup" }), env: { CLAUDE_CONFIG_DIR: home } });
  assert.ok(out.includes("hookSpecificOutput") && out.includes("hi"));
});

test("capture_rich appends + deletes temp on success", () => {
  const home = mkdtempSync(path.join(tmpdir(), "cc-")), repo = gitRepo();
  const payload = path.join(mkdtempSync(path.join(tmpdir(), "p-")), "r.json");
  writeFileSync(payload, JSON.stringify({ did: "shipped", next: ["a"], open_threads: [] }));
  run("capture_rich.mjs", { cwd: repo, stdin: "", env: { CLAUDE_CONFIG_DIR: home, SS_INPUT: payload } });
  assert.ok(!existsSync(payload));
  process.env.CLAUDE_CONFIG_DIR = home;
  const recs = store.readTimeline(keying.stateDir(repo));
  assert.ok(recs.length && recs[recs.length - 1].did === "shipped");
  delete process.env.CLAUDE_CONFIG_DIR;
});
