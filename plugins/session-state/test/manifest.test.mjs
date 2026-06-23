import { test } from "node:test";
import assert from "node:assert";
import { readFileSync, existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

const ROOT = path.join(path.dirname(fileURLToPath(import.meta.url)), "..");
const REPO = path.join(ROOT, "..", "..");

test("manifests are valid JSON and reference existing scripts", () => {
  const plugin = JSON.parse(readFileSync(path.join(ROOT, ".claude-plugin", "plugin.json"), "utf8"));
  const market = JSON.parse(readFileSync(path.join(REPO, ".claude-plugin", "marketplace.json"), "utf8"));
  const hooks = JSON.parse(readFileSync(path.join(ROOT, "hooks", "hooks.json"), "utf8"));
  assert.equal(plugin.name, "session-state");
  assert.equal(market.name, "command-center");
  assert.equal(plugin.version, market.plugins.find((p) => p.name === "session-state").version, "version drift: plugin.json vs marketplace.json");
  for (const evt of Object.keys(hooks.hooks)) {
    for (const entry of hooks.hooks[evt]) for (const h of entry.hooks) {
      const rel = h.args[0].replace("${CLAUDE_PLUGIN_ROOT}/", "");
      assert.ok(existsSync(path.join(ROOT, rel)), `missing hook script ${rel}`);
    }
  }
});
