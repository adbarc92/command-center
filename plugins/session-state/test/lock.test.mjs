import { test } from "node:test";
import assert from "node:assert";
import { mkdtempSync, writeFileSync, existsSync, readFileSync, utimesSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { withLock, LockTimeout } from "../src/lock.mjs";

test("withLock runs fn and releases (lockfile gone after)", () => {
  const lf = path.join(mkdtempSync(path.join(tmpdir(), "lk-")), "t.lock");
  let ran = false;
  withLock(lf, () => { ran = true; assert.ok(existsSync(lf)); });
  assert.ok(ran);
  assert.ok(!existsSync(lf));
});

test("live holder is not stolen → LockTimeout", () => {
  const lf = path.join(mkdtempSync(path.join(tmpdir(), "lk-")), "t.lock");
  // a live holder = THIS process's pid, fresh mtime
  writeFileSync(lf, JSON.stringify({ pid: process.pid, start: Date.now(), rand: 1 }));
  assert.throws(() => withLock(lf, () => {}, { tries: 2, backoffMs: 1 }), LockTimeout);
});

test("dead-PID lock is stolen", () => {
  const lf = path.join(mkdtempSync(path.join(tmpdir(), "lk-")), "t.lock");
  writeFileSync(lf, JSON.stringify({ pid: 999999999, start: Date.now(), rand: 1 })); // unused pid
  let ran = false;
  withLock(lf, () => { ran = true; }, { tries: 3, backoffMs: 1 });
  assert.ok(ran);
});

test("age-backstop steals a stale live-looking lock", () => {
  const lf = path.join(mkdtempSync(path.join(tmpdir(), "lk-")), "t.lock");
  writeFileSync(lf, JSON.stringify({ pid: process.pid, start: Date.now(), rand: 1 }));
  const old = (Date.now() - 120000) / 1000;
  utimesSync(lf, old, old);
  let ran = false;
  withLock(lf, () => { ran = true; }, { tries: 3, backoffMs: 1, maxAgeMs: 60000 });
  assert.ok(ran);
});
