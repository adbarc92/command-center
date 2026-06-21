import fs from "node:fs";
import path from "node:path";
import { withLock, LockTimeout } from "./lock.mjs";
import { renderLatestMd } from "./merge.mjs";

export function nowIso() {
  return new Date().toISOString().replace(/\.\d{3}Z$/, "Z");
}

export function makeRecord(type, source, sessionId, repo, git, narrative = {}) {
  const rec = { ts: nowIso(), type, source, session_id: sessionId, repo, git };
  if (type === "rich") {
    rec.did = narrative.did || "";
    rec.next = narrative.next || [];
    rec.open_threads = narrative.open_threads || [];
  }
  return rec;
}

export function scratchPath(dir, sessionId) {
  return path.join(dir, "scratch", `${sessionId || "unknown"}.json`);
}

export function writeScratch(dir, rec) {
  fs.mkdirSync(path.join(dir, "scratch"), { recursive: true });
  fs.writeFileSync(scratchPath(dir, rec.session_id), JSON.stringify(rec));
}

export function readScratches(dir) {
  const sc = path.join(dir, "scratch");
  if (!fs.existsSync(sc)) return [];
  const out = [];
  for (const f of fs.readdirSync(sc).filter((f) => f.endsWith(".json"))) {
    try { out.push(JSON.parse(fs.readFileSync(path.join(sc, f), "utf8"))); } catch {}
  }
  return out;
}

export function readTimeline(dir, { tail } = {}) {
  const tl = path.join(dir, "timeline.jsonl");
  if (!fs.existsSync(tl)) return [];
  let lines = fs.readFileSync(tl, "utf8").split(/\r?\n/).filter(Boolean);
  if (tail != null) lines = lines.slice(-tail);
  const out = [];
  for (const ln of lines) { try { out.push(JSON.parse(ln)); } catch {} }
  return out;
}

export function appendRecord(dir, rec) {
  fs.mkdirSync(dir, { recursive: true });
  try {
    withLock(path.join(dir, "timeline.lock"), () => {
      fs.appendFileSync(path.join(dir, "timeline.jsonl"), JSON.stringify(rec) + "\n");
      const md = renderLatestMd(readTimeline(dir, { tail: 50 }), readScratches(dir));
      fs.writeFileSync(path.join(dir, "latest.md"), md);
    });
    return true;
  } catch (e) {
    if (e instanceof LockTimeout) return false;
    throw e;
  }
}

export function prune(dir, { maxRecords = 1000, orphanDays = 7 } = {}) {
  const tl = path.join(dir, "timeline.jsonl");
  if (fs.existsSync(tl)) {
    try {
      withLock(path.join(dir, "timeline.lock"), () => {
        const lines = fs.readFileSync(tl, "utf8").split(/\r?\n/).filter(Boolean);
        if (lines.length > maxRecords) fs.writeFileSync(tl, lines.slice(-maxRecords).join("\n") + "\n");
      });
    } catch (e) { if (!(e instanceof LockTimeout)) throw e; }
  }
  const sc = path.join(dir, "scratch");
  if (fs.existsSync(sc)) {
    const cutoff = Date.now() - orphanDays * 86400000;
    for (const f of fs.readdirSync(sc).filter((f) => f.endsWith(".json"))) {
      const p = path.join(sc, f);
      try { if (fs.statSync(p).mtimeMs < cutoff) fs.unlinkSync(p); } catch {}
    }
  }
}
