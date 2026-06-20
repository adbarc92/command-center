// Spike 0a sentinel: prove the Stop hook fires and forwards stdin.
import fs from "node:fs";
let stdin = "";
try { stdin = fs.readFileSync(0, "utf8"); } catch {}
const sentinel = process.env.SPIKE_SENTINEL;
if (sentinel) {
  try {
    fs.appendFileSync(sentinel,
      `stop fired | shape=${process.env.SPIKE_SHAPE || "?"} | root=${process.env.CLAUDE_PLUGIN_ROOT || "?"} | stdin=${stdin.replace(/\s+/g, " ").trim()}\n`);
  } catch {}
}
process.exit(0);
