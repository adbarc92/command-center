import fs from "node:fs";
import * as keying from "./keying.mjs";
import * as store from "./store.mjs";
import { renderResumeBlock } from "./merge.mjs";

function readStdin() { try { return fs.readFileSync(0, "utf8"); } catch { return ""; } }

try {
  if (!process.env.CC_SESSION_STATE_DISABLE) {
    const raw = readStdin();
    const data = raw.trim() ? JSON.parse(raw) : {};
    if (["startup", "resume"].includes(data.source)) {
      const dir = keying.stateDir(process.cwd(), { create: false });
      const block = renderResumeBlock(store.readTimeline(dir, { tail: 50 }), store.readScratches(dir));
      if (block) console.log(JSON.stringify({ hookSpecificOutput: { hookEventName: "SessionStart", additionalContext: block } }));
    }
  }
} catch {}
process.exit(0);
