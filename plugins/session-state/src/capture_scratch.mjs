import fs from "node:fs";
import * as keying from "./keying.mjs";
import * as gitfacts from "./gitfacts.mjs";
import * as store from "./store.mjs";

const THROTTLE_MS = 30000;
function readStdin() { try { return fs.readFileSync(0, "utf8"); } catch { return ""; } }

try {
  if (!process.env.CC_SESSION_STATE_DISABLE) {
    const raw = readStdin();
    const data = raw.trim() ? JSON.parse(raw) : {};
    const sessionId = data.session_id || "unknown";
    const cwd = process.cwd();
    const root = keying.repoRoot(cwd);
    const repo = root || cwd;
    const dir = keying.stateDir(cwd);
    if (keying.checkMeta(dir, repo)) {
      const git = gitfacts.collectGitFacts(cwd);
      const prev = store.scratchPath(dir, sessionId);
      let skip = false;
      if (fs.existsSync(prev)) {
        try {
          const old = JSON.parse(fs.readFileSync(prev, "utf8"));
          const age = Date.now() - new Date(old.ts).getTime();
          if (age < THROTTLE_MS && JSON.stringify(old.git) === JSON.stringify(git)) skip = true;
        } catch {}
      }
      if (!skip) store.writeScratch(dir, store.makeRecord("auto", "Stop", sessionId, repo, git));
    }
  }
} catch {}
process.exit(0);
