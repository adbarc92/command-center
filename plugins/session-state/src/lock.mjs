import fs from "node:fs";

export class LockTimeout extends Error {}

function alive(pid) {
  if (!pid || pid <= 0) return false;
  try { process.kill(pid, 0); return true; }       // exists & signalable
  catch (e) { if (e.code === "ESRCH") return false; return true; } // EPERM/other → treat as alive
}

function sleep(ms) {
  // synchronous sleep (appends are short; tries*backoff is bounded)
  const end = Date.now() + ms;
  while (Date.now() < end) { /* spin */ }
}

function readToken(lf) {
  try { return JSON.parse(fs.readFileSync(lf, "utf8")); } catch { return null; }
}

export function withLock(lockfile, fn, { tries = 20, backoffMs = 50, maxAgeMs = 60000 } = {}) {
  const me = { pid: process.pid, start: Date.now(), rand: Math.floor(Math.random() * 1e9) };
  let held = false;
  for (let i = 0; i < tries && !held; i++) {
    try {
      const fd = fs.openSync(lockfile, "wx");           // atomic create-exclusive
      fs.writeSync(fd, JSON.stringify(me));
      fs.closeSync(fd);
      held = true;
    } catch (e) {
      if (e.code !== "EEXIST") throw e;
      // decide whether to steal: dead holder OR stale beyond maxAge
      const tok = readToken(lockfile);
      let stale = false;
      try { stale = (Date.now() - fs.statSync(lockfile).mtimeMs) > maxAgeMs; } catch { stale = true; }
      // steal if: holder pid is dead, the token is torn/unparseable (readToken → null),
      // or the lock is stale beyond maxAge. A valid token with a live pid is NOT stolen.
      const torn = tok === null;
      if (torn || (tok && !alive(tok.pid)) || stale) {
        try { fs.unlinkSync(lockfile); } catch {}       // steal; loser of the race just retries
        continue;                                       // retry immediately
      }
      sleep(backoffMs);
    }
  }
  if (!held) throw new LockTimeout(`could not lock ${lockfile} after ${tries} tries`);
  try {
    return fn();
  } finally {
    // ownership-checked release: only unlink if the token is still ours
    const tok = readToken(lockfile);
    if (tok && tok.pid === me.pid && tok.rand === me.rand) {
      try { fs.unlinkSync(lockfile); } catch {}
    }
  }
}
