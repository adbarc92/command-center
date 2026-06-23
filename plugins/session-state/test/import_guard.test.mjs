import { test } from "node:test";
import assert from "node:assert";
import { readdirSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

const SRC = path.join(path.dirname(fileURLToPath(import.meta.url)), "..", "src");

test("runtime modules import node: builtins or relative only", () => {
  const offenders = {};
  for (const f of readdirSync(SRC).filter((f) => f.endsWith(".mjs"))) {
    const text = readFileSync(path.join(SRC, f), "utf8");
    const specs = [...text.matchAll(/^\s*import\s+[^;]*?from\s+["']([^"']+)["']/gm)].map((m) => m[1]);
    const bad = specs.filter((s) => !s.startsWith("node:") && !s.startsWith("."));
    if (bad.length) offenders[f] = bad;
  }
  assert.deepEqual(offenders, {}, `non-stdlib imports: ${JSON.stringify(offenders)}`);
});
