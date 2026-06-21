import fs from "node:fs";
import path from "node:path";
import * as keying from "./keying.mjs";
import * as store from "./store.mjs";
import { renderLatestMd } from "./merge.mjs";

function resolveDir(selector) {
  if (!selector) return keying.stateDir(process.cwd(), { create: false });
  if (fs.existsSync(selector)) return keying.stateDir(selector, { create: false });
  return path.join(keying.claudeHome(), "state", "sessions", selector);
}

const [cmd, selector] = process.argv.slice(2);
if (cmd === "list") {
  const root = path.join(keying.claudeHome(), "state", "sessions");
  if (fs.existsSync(root)) for (const d of fs.readdirSync(root).sort()) {
    const flag = fs.existsSync(path.join(root, d, "COLLISION")) ? " [COLLISION]" : "";
    console.log(d + flag);
  }
} else if (cmd === "show") {
  const dir = resolveDir(selector);
  console.log(renderLatestMd(store.readTimeline(dir, { tail: 50 }), store.readScratches(dir)));
} else if (cmd === "prune") {
  store.prune(resolveDir(selector));
  console.log("pruned.");
} else {
  console.error("usage: cli.mjs list|show [selector]|prune [selector]");
  process.exit(2);
}
