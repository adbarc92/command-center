import fs from "node:fs";
import path from "node:path";
import { claudeHome } from "./keying.mjs";

// Parse a version dir into comparable numeric parts. Returns null for non-semver
// garbage so callers can sort it last instead of crashing. A trailing pre-release
// tag (e.g. "1.0.0-beta") is tolerated by parsing only the leading numeric core.
function parseSemver(name) {
  const core = String(name).split("-")[0];
  const parts = core.split(".");
  if (parts.length === 0) return null;
  const nums = [];
  for (const p of parts) {
    if (!/^\d+$/.test(p)) return null;
    nums.push(Number(p));
  }
  return nums;
}

// Descending semver comparator. Valid semvers sort before garbage; among valid
// ones, higher version first (so "0.10.0" beats "0.9.0"). A pre-release of an
// otherwise-equal core sorts after the release (release > pre-release).
function compareVersionsDesc(a, b) {
  const pa = parseSemver(a);
  const pb = parseSemver(b);
  if (pa && !pb) return -1;
  if (!pa && pb) return 1;
  if (!pa && !pb) return a < b ? 1 : a > b ? -1 : 0;
  const len = Math.max(pa.length, pb.length);
  for (let i = 0; i < len; i++) {
    const x = pa[i] ?? 0;
    const y = pb[i] ?? 0;
    if (x !== y) return y - x;
  }
  // Equal numeric core: release (no pre-release tag) ranks above a pre-release.
  const preA = String(a).includes("-");
  const preB = String(b).includes("-");
  if (preA !== preB) return preA ? 1 : -1;
  return a < b ? 1 : a > b ? -1 : 0;
}

export function pluginInstallPath(marketplaceName = "command-center", pluginName = "session-state") {
  const home = claudeHome();
  const key = `${pluginName}@${marketplaceName}`;
  // 1) registry
  try {
    const reg = JSON.parse(fs.readFileSync(path.join(home, "plugins", "installed_plugins.json"), "utf8"));
    const entries = reg.plugins && reg.plugins[key];
    if (Array.isArray(entries)) {
      for (const e of entries) {
        if (e.installPath && fs.existsSync(path.join(e.installPath, "src", "capture_rich.mjs"))) return e.installPath;
      }
    }
  } catch {}
  // 2) cache scan: newest version dir that has the script
  try {
    const base = path.join(home, "plugins", "cache", marketplaceName, pluginName);
    const versions = fs.readdirSync(base).sort(compareVersionsDesc);
    for (const v of versions) {
      const p = path.join(base, v);
      if (fs.existsSync(path.join(p, "src", "capture_rich.mjs"))) return p;
    }
  } catch {}
  return null;
}
