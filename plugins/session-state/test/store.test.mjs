import { test } from "node:test";
import assert from "node:assert";
import { mkdtempSync, writeFileSync, readFileSync, existsSync, mkdirSync, utimesSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import * as s from "../src/store.mjs";

const mk = () => mkdtempSync(path.join(tmpdir(), "st-"));

test("nowIso ends with Z", () => assert.ok(s.nowIso().endsWith("Z")));
test("makeRecord auto omits narrative", () => { const r = s.makeRecord("auto", "x", "sid", "D:/r", { branch: "main" }); assert.equal(r.type, "auto"); assert.ok(!("did" in r)); });
test("scratch roundtrip", () => { const d = mk(); s.writeScratch(d, s.makeRecord("auto", "Stop", "sid1", "D:/r", { branch: "main" })); assert.equal(s.readScratches(d).length, 1); });
test("readTimeline skips corrupt", () => { const d = mk(); writeFileSync(path.join(d, "timeline.jsonl"), '{"ts":"1"}\nNOPE\n{"ts":"2"}\n'); assert.deepEqual(s.readTimeline(d).map(r => r.ts), ["1", "2"]); });
test("appendRecord writes line + latest.md", () => {
  const d = mk();
  assert.equal(s.appendRecord(d, s.makeRecord("rich", "save-state", "sid", "D:/r", { branch: "main" }, { did: "x", next: ["a"], open_threads: [] })), true);
  assert.equal(s.readTimeline(d).length, 1);
  assert.ok(existsSync(path.join(d, "latest.md")));
});
test("prune truncates oldest", () => {
  const d = mk();
  writeFileSync(path.join(d, "timeline.jsonl"), Array.from({ length: 10 }, (_, i) => `{"ts":"${i}"}`).join("\n") + "\n");
  s.prune(d, { maxRecords: 4 });
  assert.deepEqual(s.readTimeline(d).map(r => r.ts), ["6", "7", "8", "9"]);
});
test("prune removes orphan scratch", () => {
  const d = mk(); mkdirSync(path.join(d, "scratch"), { recursive: true });
  const f = path.join(d, "scratch", "old.json"); writeFileSync(f, "{}");
  const eight = (Date.now() - 8 * 86400000) / 1000; utimesSync(f, eight, eight);
  s.prune(d, { orphanDays: 7 });
  assert.ok(!existsSync(f));
});
