import { test } from "node:test";
import assert from "node:assert";
import { mkdtempSync, mkdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { pluginInstallPath } from "../src/resolve.mjs";

function seed(home, installPath, { create = true } = {}) {
  mkdirSync(path.join(home, "plugins"), { recursive: true });
  writeFileSync(path.join(home, "plugins", "installed_plugins.json"),
    JSON.stringify({ version: 2, plugins: { "session-state@command-center": [{ scope: "user", installPath, version: "0.1.0" }] } }));
  if (create) { mkdirSync(path.join(installPath, "src"), { recursive: true }); writeFileSync(path.join(installPath, "src", "capture_rich.mjs"), "//"); }
}

test("returns installPath from registry when it exists", () => {
  const home = mkdtempSync(path.join(tmpdir(), "cc-"));
  process.env.CLAUDE_CONFIG_DIR = home;
  const ip = path.join(home, "plugins", "cache", "command-center", "session-state", "0.1.0");
  seed(home, ip);
  assert.equal(pluginInstallPath(), ip);
  delete process.env.CLAUDE_CONFIG_DIR;
});

test("falls back to cache scan when registry path missing", () => {
  const home = mkdtempSync(path.join(tmpdir(), "cc-"));
  process.env.CLAUDE_CONFIG_DIR = home;
  seed(home, path.join(home, "GONE"), { create: false });
  const real = path.join(home, "plugins", "cache", "command-center", "session-state", "0.2.0");
  mkdirSync(path.join(real, "src"), { recursive: true }); writeFileSync(path.join(real, "src", "capture_rich.mjs"), "//");
  assert.equal(pluginInstallPath(), real);
  delete process.env.CLAUDE_CONFIG_DIR;
});

test("cache scan picks highest semver, not lexically-last", () => {
  const home = mkdtempSync(path.join(tmpdir(), "cc-"));
  process.env.CLAUDE_CONFIG_DIR = home;
  seed(home, path.join(home, "GONE"), { create: false });
  const base = path.join(home, "plugins", "cache", "command-center", "session-state");
  // Created in an order where lexical sort would wrongly rank "0.9.0" above "0.10.0".
  for (const v of ["0.2.0", "0.9.0", "0.10.0"]) {
    mkdirSync(path.join(base, v, "src"), { recursive: true });
    writeFileSync(path.join(base, v, "src", "capture_rich.mjs"), "//");
  }
  assert.equal(pluginInstallPath(), path.join(base, "0.10.0"));
  delete process.env.CLAUDE_CONFIG_DIR;
});

test("cache scan tolerates non-semver dirs without crashing", () => {
  const home = mkdtempSync(path.join(tmpdir(), "cc-"));
  process.env.CLAUDE_CONFIG_DIR = home;
  seed(home, path.join(home, "GONE"), { create: false });
  const base = path.join(home, "plugins", "cache", "command-center", "session-state");
  for (const v of ["garbage", "0.1.0", "1.0.0-beta"]) {
    mkdirSync(path.join(base, v, "src"), { recursive: true });
    writeFileSync(path.join(base, v, "src", "capture_rich.mjs"), "//");
  }
  assert.equal(pluginInstallPath(), path.join(base, "1.0.0-beta"));
  delete process.env.CLAUDE_CONFIG_DIR;
});

test("null when nothing found", () => {
  const home = mkdtempSync(path.join(tmpdir(), "cc-"));
  process.env.CLAUDE_CONFIG_DIR = home;
  assert.equal(pluginInstallPath(), null);
  delete process.env.CLAUDE_CONFIG_DIR;
});
