import fs from "node:fs";
import path from "node:path";
import { claudeHome } from "./keying.mjs";

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
    const versions = fs.readdirSync(base).sort().reverse();
    for (const v of versions) {
      const p = path.join(base, v);
      if (fs.existsSync(path.join(p, "src", "capture_rich.mjs"))) return p;
    }
  } catch {}
  return null;
}
