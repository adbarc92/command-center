import fs from "node:fs";
import * as keying from "./keying.mjs";
import * as gitfacts from "./gitfacts.mjs";
import * as store from "./store.mjs";

function argInput() {
  const i = process.argv.indexOf("--input");
  if (i >= 0 && process.argv[i + 1]) return process.argv[i + 1];
  return process.env.SS_INPUT || null;
}

if (process.env.CC_SESSION_STATE_DISABLE) process.exit(0);
const input = argInput();
if (!input) { console.error("session-state: --input <file> required"); process.exit(2); }

let deleteTemp = false;
let exitCode = 1;
try {
  const data = JSON.parse(fs.readFileSync(input, "utf8"));
  const cwd = process.cwd();
  const root = keying.repoRoot(cwd);
  const repo = root || cwd;
  const dir = keying.stateDir(cwd);
  if (!keying.checkMeta(dir, repo)) { console.error("session-state: repo-key collision; narrative NOT saved."); }
  else {
    const git = gitfacts.collectGitFacts(cwd);
    const rec = store.makeRecord("rich", "save-state", data.session_id || null, repo, git,
      { did: data.did || "", next: data.next || [], open_threads: data.open_threads || [] });
    if (store.appendRecord(dir, rec)) { console.log("session-state: narrative saved."); deleteTemp = true; exitCode = 0; }
    else { console.error(`session-state: could not acquire lock; narrative NOT saved. Temp preserved at ${input}. Retry: node capture_rich.mjs --input ${input}`); }
  }
} catch (e) {
  console.error(`session-state: error saving narrative: ${e.message}. Temp preserved at ${input}.`);
} finally {
  if (deleteTemp) { try { fs.unlinkSync(input); } catch {} }
}
process.exit(exitCode);
