import { test } from "node:test";
import assert from "node:assert";
import { mkdtempSync, mkdirSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import * as g from "../src/gitfacts.mjs";

const CLEAN = "# branch.oid abc\n# branch.head main\n# branch.upstream origin/main\n# branch.ab +0 -0\n";
const DIRTY = "# branch.oid abc\n# branch.head main\n1 .M N... 100644 100644 100644 a b src/foo.rs\n2 R. N... 100644 100644 100644 c d R100 new.rs\told.rs\n? untracked.txt\n";
const DETACHED = "# branch.oid abc\n# branch.head (detached)\n1 .M N... 100644 100644 100644 a b x.rs\n";
const CONFLICT = "# branch.oid abc\n# branch.head (detached)\nu UU N... 100644 100644 100644 100644 a b c conflicted.rs\n";

test("parse clean", () => assert.deepEqual(g.parsePorcelainV2(CLEAN), { branch: "main", detached: false, dirty: [] }));
test("parse dirty incl rename(new) + untracked", () => {
  const r = g.parsePorcelainV2(DIRTY);
  assert.equal(r.branch, "main");
  assert.ok(r.dirty.includes("src/foo.rs") && r.dirty.includes("new.rs") && r.dirty.includes("untracked.txt"));
});
test("parse detached", () => { const r = g.parsePorcelainV2(DETACHED); assert.equal(r.branch, null); assert.equal(r.detached, true); assert.deepEqual(r.dirty, ["x.rs"]); });
test("parse conflict unmerged", () => assert.ok(g.parsePorcelainV2(CONFLICT).dirty.includes("conflicted.rs")));
test("inProgress detects rebase", () => { const d = mkdtempSync(path.join(tmpdir(), "gd-")); mkdirSync(path.join(d, "rebase-merge")); assert.equal(g.inProgress(d), "rebase"); });
test("inProgress none", () => assert.equal(g.inProgress(mkdtempSync(path.join(tmpdir(), "gd-"))), null));
test("collectGitFacts non-git is null", () => assert.equal(g.collectGitFacts(mkdtempSync(path.join(tmpdir(), "ng-"))), null));
