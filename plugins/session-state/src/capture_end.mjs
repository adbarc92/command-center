import fs from "node:fs";
import * as keying from "./keying.mjs";
import * as gitfacts from "./gitfacts.mjs";
import * as store from "./store.mjs";

const SKIP = new Set(["clear", "resume"]);
function readStdin() { try { return fs.readFileSync(0, "utf8"); } catch { return ""; } }

try {
  if (!process.env.CC_SESSION_STATE_DISABLE) {
    const raw = readStdin();
    const data = raw.trim() ? JSON.parse(raw) : {};
    const reason = data.reason || "other";
    if (!SKIP.has(reason)) {
      const sessionId = data.session_id || "unknown";
      const cwd = process.cwd();
      const root = keying.repoRoot(cwd);
      const repo = root || cwd;
      const dir = keying.stateDir(cwd);
      if (keying.checkMeta(dir, repo)) {
        const git = gitfacts.collectGitFacts(cwd);
        store.appendRecord(dir, store.makeRecord("auto", `SessionEnd:${reason}`, sessionId, repo, git));
        const own = store.scratchPath(dir, sessionId);
        try { if (fs.existsSync(own)) fs.unlinkSync(own); } catch {}
        store.prune(dir);
      }
    }
  }
} catch {}
process.exit(0);
