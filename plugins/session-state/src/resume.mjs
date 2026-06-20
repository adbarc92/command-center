// Spike 0a sentinel: prove the SessionStart hook fires, forwards stdin, and can emit the envelope.
import fs from "node:fs";

let stdin = "";
try { stdin = fs.readFileSync(0, "utf8"); } catch {}

const sentinel = process.env.SPIKE_SENTINEL;
if (sentinel) {
  try {
    fs.appendFileSync(sentinel,
      `resume fired | shape=${process.env.SPIKE_SHAPE || "?"} | root=${process.env.CLAUDE_PLUGIN_ROOT || "?"} | stdin=${stdin.replace(/\s+/g, " ").trim()}\n`);
  } catch {}
}

// Emit the SessionStart envelope only for startup/resume (source-gate test).
try {
  const data = stdin.trim() ? JSON.parse(stdin) : {};
  if (["startup", "resume"].includes(data.source)) {
    console.log(JSON.stringify({ hookSpecificOutput: {
      hookEventName: "SessionStart",
      additionalContext: "<session-state>spike: resume hook reached</session-state>" } }));
  }
} catch {}
process.exit(0);
