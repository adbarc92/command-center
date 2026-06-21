import { test } from "node:test";
import assert from "node:assert";
import * as m from "../src/merge.mjs";

const rec = (ts, type, branch, { did, detached = false, in_progress = null } = {}) => {
  const git = { branch, detached, in_progress, head: "abc x", dirty: [], worktree: null, git_unavailable: false };
  const r = { ts, type, source: "t", session_id: "s", repo: "D:/r", git };
  if (type === "rich") { r.did = did || "did"; r.next = ["n1"]; r.open_threads = ["t1"]; }
  return r;
};

test("freshest git from scratch over older timeline", () => {
  const st = m.resolveState([rec("2026-01-01T00:00:00Z", "auto", "main")], [rec("2026-02-01T00:00:00Z", "auto", "feat/x")]);
  assert.equal(st.git.branch, "feat/x");
});
test("narrative is newest rich", () => {
  const st = m.resolveState([rec("2026-01-01T00:00:00Z", "rich", "main", { did: "old" }), rec("2026-02-01T00:00:00Z", "rich", "main", { did: "new" })], []);
  assert.equal(st.narrative.did, "new");
});
test("branch banner when branches differ", () => {
  const st = m.resolveState([rec("2026-01-01T00:00:00Z", "rich", "feat/x")], [rec("2026-02-01T00:00:00Z", "auto", "main")]);
  assert.ok(st.branchBanner && st.branchBanner.includes("feat/x"));
});
test("no banner when detached", () => {
  const st = m.resolveState([rec("2026-01-01T00:00:00Z", "rich", "feat/x")], [rec("2026-02-01T00:00:00Z", "auto", null, { detached: true })]);
  assert.equal(st.branchBanner, null);
});
test("resume block null when empty", () => assert.equal(m.renderResumeBlock([], []), null));
test("resume block has next + threads", () => {
  const b = m.renderResumeBlock([rec("2026-02-01T00:00:00Z", "rich", "main", { did: "shipped X" })], []);
  assert.ok(b.includes("shipped X") && b.includes("n1") && b.includes("t1"));
});
