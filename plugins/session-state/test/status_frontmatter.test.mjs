import { test } from "node:test";
import assert from "node:assert";
import { stampStatusFrontmatter } from "../src/status_frontmatter.mjs";

test("inserts a byte-0 block and keeps a leading H1 below it", () => {
  const out = stampStatusFrontmatter("# Command Center\n\nbody\n", { stage: "Build", updated: "2026-07-06" });
  assert.ok(out.startsWith("---\n"), "block at byte 0");
  assert.match(out, /stage: "Build"/);
  assert.match(out, /updated: "2026-07-06"/);
  // H1 preserved, now below the block:
  assert.match(out, /---\n[\s\S]*# Command Center/);
});

test("updates stage/updated in an existing block, preserving other keys", () => {
  const src = '---\nname: "CC"\nstage: "Spec"\nupdated: "2026-06-01"\n---\n# Title\n';
  const out = stampStatusFrontmatter(src, { stage: "Build", updated: "2026-07-06" });
  assert.match(out, /name: "CC"/);          // preserved
  assert.match(out, /stage: "Build"/);       // updated
  assert.match(out, /updated: "2026-07-06"/);// updated
  assert.doesNotMatch(out, /stage: "Spec"/); // old value gone
});

test("strips a leading UTF-8 BOM so the block lands at byte 0", () => {
  const out = stampStatusFrontmatter("﻿# T\n", { stage: "Ship", updated: "2026-07-06" });
  assert.ok(out.charCodeAt(0) !== 0xfeff, "no BOM");
  assert.ok(out.startsWith("---\n"));
});

test("is idempotent — stamping twice does not duplicate the block", () => {
  const once = stampStatusFrontmatter("# T\n", { stage: "Build", updated: "2026-07-06" });
  const twice = stampStatusFrontmatter(once, { stage: "Build", updated: "2026-07-06" });
  const blocks = (twice.match(/^---$/gm) || []).length;
  assert.equal(blocks, 2, "exactly one block = two fence lines");
});

test("throws when stage is missing", () => {
  assert.throws(() => stampStatusFrontmatter("# T\n", {}), /stage is required/);
});
