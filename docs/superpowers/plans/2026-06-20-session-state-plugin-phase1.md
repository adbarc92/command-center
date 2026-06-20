# Session-State Plugin — Phase 1 + Migration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Re-implement the session-state runtime in dependency-free Node ESM, packaged as a Claude Code plugin with a self-marketplace in this repo, and migrate off the merged Python tool — so users get the hooks at the harness layer.

**Architecture:** Plain `.mjs` (`node:` builtins only, no build) under `plugins/session-state/src/`, invoked by three plugin hooks via the **bare `node`+`args` exec form proven by Spike 0a**. Storage format/paths are parse-compatible with the Python version (same `~/.claude/state/sessions/<repo-key>/` layout). `save-state` ships as a plugin skill that resolves its script via Claude Code's `installed_plugins.json` registry. Migration removes the merged Python `tools/session-state` + `install.ps1` + `.claude/skills/save-state` in an atomic swap.

**Tech Stack:** Node ≥18 (ESM, `node:test`), Git, Claude Code plugin system, PowerShell (migration/uninstall of the old tool only).

**Spec:** `docs/superpowers/specs/2026-06-19-session-state-plugin-distribution-design.md` (read it). **Spikes:** `spikes/SPIKE-RESULTS-session-state-plugin.md` (0a/0b PASSED). **Behavior reference:** the merged Python tool at `tools/session-state/src/session_state/*.py` on `main` — port behavior faithfully.

## Global Constraints

- **Runtime: Node `node:` builtins ONLY** in every shipped `plugins/session-state/src/*.mjs` — no npm deps, no build step, runnable by bare `node`. (Spec §3)
- **Hooks never block/crash a session:** every hook entry wraps logic in try/catch and **always `process.exit(0)`**; emits nothing on the empty/error path. (Spec §3)
- **Kill-switch:** every entry checks `process.env.CC_SESSION_STATE_DISABLE` first and no-ops (exit 0). (Spec §3)
- **State root:** `~/.claude/state/sessions/<repo-key>/`, honoring `CLAUDE_CONFIG_DIR`. Format parse-compatible with the Python version: same record keys, same `ts` format (`YYYY-MM-DDTHH:MM:SSZ`), same paths/slug. (Spec §3)
- **Hook command form (Spike 0a):** `{"type":"command","command":"node","args":["${CLAUDE_PLUGIN_ROOT}/src/<entry>.mjs"]}`; SessionStart carries `"matcher":"startup|resume"`. NO wrapper. (Spec §2)
- **`save-state` is a plugin SKILL** invoked via its `description`; it resolves `capture_rich.mjs` by reading `~/.claude/plugins/installed_plugins.json` key `session-state@command-center` → `installPath`. (Spec §3) `end-session` does NOT drive it — do not assume that contract.
- **Tooling lives at** `plugins/session-state/` with `.claude-plugin/marketplace.json` at repo root (self-marketplace `command-center`). The spike skeleton already created these; replace the sentinel scripts.
- **Tests:** `node --test plugins/session-state/test/`. No third-party test runner.

---

## File Structure

```
command-center/
  .claude-plugin/marketplace.json          # EXISTS (spike) — finalize version
  plugins/session-state/
    .claude-plugin/plugin.json             # EXISTS (spike) — finalize version
    hooks/hooks.json                        # EXISTS (spike, shape i) — add SessionEnd entry
    skills/save-state/SKILL.md              # EXISTS (spike stub) — replace with real skill (Task 10)
    src/
      keying.mjs gitfacts.mjs lock.mjs store.mjs merge.mjs resolve.mjs   # Tasks 2-7
      capture_scratch.mjs capture_end.mjs capture_rich.mjs resume.mjs cli.mjs  # Tasks 8-9
    test/
      import_guard.test.mjs keying.test.mjs gitfacts.test.mjs lock.test.mjs
      store.test.mjs merge.test.mjs resolve.test.mjs entries.test.mjs cli.test.mjs
      manifest.test.mjs
    README.md
  tools/session-state/  install.ps1  .claude/skills/save-state/   # REMOVED in Task 11 (migration)
```

Tasks build the testable core first (keying → gitfacts → lock → store → merge → resolve), then entries, cli, the skill + manifests, then migration.

---

### Task 1: Finalize plugin manifests + import-guard test

**Files:**
- Modify: `plugins/session-state/.claude-plugin/plugin.json` (version → `0.1.0`, drop "spike")
- Modify: `plugins/session-state/hooks/hooks.json` (add the SessionEnd entry)
- Modify: `.claude-plugin/marketplace.json` (version → `0.1.0`, drop "spike")
- Create: `plugins/session-state/test/import_guard.test.mjs`
- Create: `plugins/session-state/README.md`

**Interfaces:**
- Produces: the import-guard test other tasks rely on staying green; final `hooks.json` with 3 entries.

- [ ] **Step 1: Finalize `plugin.json`**

```json
{
  "name": "session-state",
  "version": "0.1.0",
  "description": "Per-repo dev-session capture + zero-friction resume for Claude Code.",
  "author": { "name": "Alex Barclay", "email": "adbarclay92@gmail.com" }
}
```

- [ ] **Step 2: Finalize `hooks/hooks.json` (3 entries, shape i)**

```json
{
  "hooks": {
    "SessionStart": [
      { "matcher": "startup|resume", "hooks": [
        { "type": "command", "command": "node", "args": ["${CLAUDE_PLUGIN_ROOT}/src/resume.mjs"] }
      ] }
    ],
    "Stop": [
      { "hooks": [
        { "type": "command", "command": "node", "args": ["${CLAUDE_PLUGIN_ROOT}/src/capture_scratch.mjs"], "timeout": 5 }
      ] }
    ],
    "SessionEnd": [
      { "hooks": [
        { "type": "command", "command": "node", "args": ["${CLAUDE_PLUGIN_ROOT}/src/capture_end.mjs"] }
      ] }
    ]
  }
}
```

- [ ] **Step 3: Finalize `.claude-plugin/marketplace.json`**

```json
{
  "$schema": "https://json.schemastore.org/claude-code-marketplace.json",
  "name": "command-center",
  "owner": { "name": "Alex Barclay", "email": "adbarclay92@gmail.com" },
  "plugins": [
    { "name": "session-state", "source": "./plugins/session-state",
      "description": "Per-repo dev-session capture + zero-friction resume.", "version": "0.1.0" }
  ]
}
```

- [ ] **Step 4: Write the import-guard test** `plugins/session-state/test/import_guard.test.mjs`

```javascript
import { test } from "node:test";
import assert from "node:assert";
import { readdirSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

const SRC = path.join(path.dirname(fileURLToPath(import.meta.url)), "..", "src");

test("runtime modules import node: builtins or relative only", () => {
  const offenders = {};
  for (const f of readdirSync(SRC).filter((f) => f.endsWith(".mjs"))) {
    const text = readFileSync(path.join(SRC, f), "utf8");
    const specs = [...text.matchAll(/^\s*import\s+[^;]*?from\s+["']([^"']+)["']/gm)].map((m) => m[1]);
    const bad = specs.filter((s) => !s.startsWith("node:") && !s.startsWith("."));
    if (bad.length) offenders[f] = bad;
  }
  assert.deepEqual(offenders, {}, `non-stdlib imports: ${JSON.stringify(offenders)}`);
});
```

- [ ] **Step 5: Write `README.md`** (install/uninstall/disable/inspect)

```markdown
# session-state (plugin)

Per-repo dev-session capture + zero-friction resume, shipped as a Claude Code plugin.
Spec: `docs/superpowers/specs/2026-06-19-session-state-plugin-distribution-design.md`.

## Install (standalone host)
    claude plugin marketplace add <command-center repo>
    claude plugin install session-state@command-center

## Disable temporarily
Set `CC_SESSION_STATE_DISABLE=1` in a shell to make all hooks no-op there.

## Inspect
    node plugins/session-state/src/cli.mjs list
    node plugins/session-state/src/cli.mjs show [<path-or-repo-key>]

## Tests
    node --test plugins/session-state/test/
```

- [ ] **Step 6: Run the import guard (no src yet → trivially passes) and commit**

Run: `node --test plugins/session-state/test/import_guard.test.mjs`
Expected: PASS (no `.mjs` in src yet besides the spike sentinels — if the spike `resume.mjs`/`capture_scratch.mjs` are still present, they import only `node:fs`, so still green).

```bash
git add plugins/session-state/.claude-plugin plugins/session-state/hooks plugins/session-state/test/import_guard.test.mjs plugins/session-state/README.md .claude-plugin/marketplace.json
git commit -m "feat(plugin): finalize session-state manifests + import guard"
```

---

### Task 2: keying.mjs (repo→state-dir)

**Files:**
- Create: `plugins/session-state/src/keying.mjs`
- Test: `plugins/session-state/test/keying.test.mjs`

**Interfaces:**
- Produces: `claudeHome()`, `canonicalProjectRoot(cwd)`, `pathToSlug(p)`, `repoRoot(cwd)`, `repoKey(cwd)`, `stateDir(cwd,{create})`, `checkMeta(dir, canonicalRepo)`.

- [ ] **Step 1: Write failing tests** `plugins/session-state/test/keying.test.mjs`

```javascript
import { test } from "node:test";
import assert from "node:assert";
import { mkdtempSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import * as k from "../src/keying.mjs";

test("pathToSlug replaces separators and drive colon", () => {
  assert.equal(k.pathToSlug("D:\\MajorProjects\\CURRENT\\command-center"),
    "D--MajorProjects-CURRENT-command-center");
});

test("canonicalProjectRoot collapses .claude/worktrees/<name>", () => {
  assert.equal(k.canonicalProjectRoot("/repo/.claude/worktrees/agent-x"), "/repo");
  assert.equal(k.canonicalProjectRoot("/repo/src"), "/repo/src");
});

test("repoKey for non-git dir uses slug of cwd", () => {
  const d = mkdtempSync(path.join(tmpdir(), "ss-"));
  assert.equal(k.repoKey(d), k.pathToSlug(d));
});

test("stateDir created under claude home, checkMeta detects collision", () => {
  const home = mkdtempSync(path.join(tmpdir(), "cc-"));
  process.env.CLAUDE_CONFIG_DIR = home;
  const d = k.stateDir(mkdtempSync(path.join(tmpdir(), "ss-")));
  assert.ok(existsSync(d));
  assert.equal(path.basename(path.dirname(d)), "sessions");
  assert.equal(k.checkMeta(d, "D:/repo-a"), true);
  assert.equal(k.checkMeta(d, "D:/repo-a"), true);
  assert.equal(k.checkMeta(d, "D:/other"), false);
  assert.ok(existsSync(path.join(d, "COLLISION")));
  delete process.env.CLAUDE_CONFIG_DIR;
});
```

- [ ] **Step 2: Run to verify failure**

Run: `node --test plugins/session-state/test/keying.test.mjs`
Expected: FAIL — cannot find `../src/keying.mjs`.

- [ ] **Step 3: Implement `keying.mjs`**

```javascript
import { homedir } from "node:os";
import path from "node:path";
import fs from "node:fs";
import { execFileSync } from "node:child_process";

export function claudeHome() {
  return process.env.CLAUDE_CONFIG_DIR || path.join(homedir(), ".claude");
}

// Collapse "<root>/.claude/worktrees/<name>" back to "<root>" (string-based; sep-agnostic).
export function canonicalProjectRoot(cwd) {
  const m = cwd.match(/^(.*)[\\/]\.claude[\\/]worktrees[\\/][^\\/]+(?:[\\/].*)?$/);
  return m ? m[1] : cwd;
}

export function pathToSlug(p) {
  return p.replace(/[\\/:]/g, "-");
}

export function repoRoot(cwd) {
  try {
    const out = execFileSync("git", ["-C", cwd, "rev-parse", "--show-toplevel"],
      { encoding: "utf8", timeout: 5000 }).trim();
    return out ? canonicalProjectRoot(out) : null;
  } catch { return null; }
}

export function repoKey(cwd) {
  return pathToSlug(repoRoot(cwd) || cwd);
}

export function stateDir(cwd, { create = true } = {}) {
  const d = path.join(claudeHome(), "state", "sessions", repoKey(cwd));
  if (create) fs.mkdirSync(path.join(d, "scratch"), { recursive: true });
  return d;
}

export function checkMeta(dir, canonicalRepo) {
  const meta = path.join(dir, "meta.json");
  if (!fs.existsSync(meta)) {
    fs.mkdirSync(dir, { recursive: true });
    fs.writeFileSync(meta, JSON.stringify({ repo: canonicalRepo }));
    return true;
  }
  let existing = null;
  try { existing = JSON.parse(fs.readFileSync(meta, "utf8")).repo; } catch {}
  if (existing === canonicalRepo) return true;
  fs.writeFileSync(path.join(dir, "COLLISION"), `expected ${existing} got ${canonicalRepo}`);
  return false;
}
```

- [ ] **Step 4: Run tests to verify pass**

Run: `node --test plugins/session-state/test/keying.test.mjs plugins/session-state/test/import_guard.test.mjs`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add plugins/session-state/src/keying.mjs plugins/session-state/test/keying.test.mjs
git commit -m "feat(plugin): keying.mjs (repo->state-dir, slug, collision guard)"
```

---

### Task 3: gitfacts.mjs

**Files:**
- Create: `plugins/session-state/src/gitfacts.mjs`
- Test: `plugins/session-state/test/gitfacts.test.mjs`

**Interfaces:**
- Produces: `parsePorcelainV2(text)` → `{branch, detached, dirty}`; `inProgress(gitDir)` → `"rebase"|"merge"|"bisect"|null`; `collectGitFacts(cwd, cap=50)` → git object (keys: `branch, detached, in_progress, head, dirty, worktree, git_unavailable`) | `null` (non-git).

- [ ] **Step 1: Write failing tests** `plugins/session-state/test/gitfacts.test.mjs`

```javascript
import { test } from "node:test";
import assert from "node:assert";
import { mkdtempSync, mkdirSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import * as g from "../src/gitfacts.mjs";

const CLEAN = "# branch.oid abc\n# branch.head main\n# branch.upstream origin/main\n# branch.ab +0 -0\n";
const DIRTY = "# branch.oid abc\n# branch.head main\n1 .M N... 100644 100644 100644 a b src/foo.rs\n2 R. N... 100644 100644 100644 c d R100 new.rs\told.rs\n? untracked.txt\n";
const DETACHED = "# branch.oid abc\n# branch.head (detached)\n1 .M N... 100644 100644 100644 a b x.rs\n";
const CONFLICT = "# branch.oid abc\n# branch.head (detached)\nu UU N... 100644 100644 100644 100644 a b c conflicted.rs\n";

test("parse clean", () => assert.deepEqual(g.parsePorcelainV2(CLEAN), { branch: "main", detached: false, dirty: [] }));
test("parse dirty incl rename(new) + untracked", () => {
  const r = g.parsePorcelainV2(DIRTY);
  assert.equal(r.branch, "main");
  assert.ok(r.dirty.includes("src/foo.rs") && r.dirty.includes("new.rs") && r.dirty.includes("untracked.txt"));
});
test("parse detached", () => { const r = g.parsePorcelainV2(DETACHED); assert.equal(r.branch, null); assert.equal(r.detached, true); assert.deepEqual(r.dirty, ["x.rs"]); });
test("parse conflict unmerged", () => assert.ok(g.parsePorcelainV2(CONFLICT).dirty.includes("conflicted.rs")));
test("inProgress detects rebase", () => { const d = mkdtempSync(path.join(tmpdir(), "gd-")); mkdirSync(path.join(d, "rebase-merge")); assert.equal(g.inProgress(d), "rebase"); });
test("inProgress none", () => assert.equal(g.inProgress(mkdtempSync(path.join(tmpdir(), "gd-"))), null));
test("collectGitFacts non-git is null", () => assert.equal(g.collectGitFacts(mkdtempSync(path.join(tmpdir(), "ng-"))), null));
```

- [ ] **Step 2: Run to verify failure**

Run: `node --test plugins/session-state/test/gitfacts.test.mjs`
Expected: FAIL — cannot find module.

- [ ] **Step 3: Implement `gitfacts.mjs`**

```javascript
import fs from "node:fs";
import path from "node:path";
import { execFileSync } from "node:child_process";

export function parsePorcelainV2(text) {
  let branch = null, detached = false; const dirty = [];
  for (const line of text.split(/\r?\n/)) {
    if (line.startsWith("# branch.head ")) {
      const head = line.slice("# branch.head ".length).trim();
      if (head === "(detached)") { detached = true; branch = null; } else branch = head;
    } else if (line.startsWith("1 ") || line.startsWith("2 ")) {
      const left = line.split("\t")[0];
      dirty.push(left.split(" ").pop());
    } else if (line.startsWith("u ")) {
      dirty.push(line.split("\t")[0].split(" ").pop());
    } else if (line.startsWith("? ")) {
      dirty.push(line.slice(2).trim());
    }
  }
  return { branch, detached, dirty };
}

export function inProgress(gitDir) {
  const has = (n) => fs.existsSync(path.join(gitDir, n));
  if (has("rebase-merge") || has("rebase-apply")) return "rebase";
  if (has("MERGE_HEAD")) return "merge";
  if (has("BISECT_LOG")) return "bisect";
  return null;
}

function git(cwd, args) {
  return execFileSync("git", ["-C", cwd, ...args], { encoding: "utf8", timeout: 5000 });
}

export function collectGitFacts(cwd, cap = 50) {
  try { execFileSync("git", ["--version"], { timeout: 5000 }); }
  catch { return { branch: null, detached: false, in_progress: null, head: null, dirty: [], worktree: null, git_unavailable: true }; }
  let top;
  try { top = git(cwd, ["rev-parse", "--show-toplevel"]).trim(); }
  catch { return null; } // not a git repo
  if (!top) return null;
  const status = (() => { try { return git(cwd, ["--no-optional-locks", "status", "--porcelain=v2", "--branch"]); } catch { return ""; } })();
  const parsed = parsePorcelainV2(status);
  let head = null;
  try { head = git(cwd, ["log", "-1", "--format=%h %s"]).trim(); } catch {}
  let gitDir = path.join(cwd, ".git");
  try { gitDir = path.resolve(cwd, git(cwd, ["rev-parse", "--git-dir"]).trim()); } catch {}
  let worktree = null;
  try {
    const common = git(cwd, ["rev-parse", "--path-format=absolute", "--git-common-dir"]).trim();
    if (common && path.dirname(common) !== top) worktree = path.basename(top);
  } catch {}
  return {
    branch: parsed.branch, detached: parsed.detached, in_progress: inProgress(gitDir),
    head, dirty: parsed.dirty.slice(0, cap), worktree, git_unavailable: false,
  };
}
```

- [ ] **Step 4: Run tests to verify pass**

Run: `node --test plugins/session-state/test/gitfacts.test.mjs plugins/session-state/test/import_guard.test.mjs`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add plugins/session-state/src/gitfacts.mjs plugins/session-state/test/gitfacts.test.mjs
git commit -m "feat(plugin): gitfacts.mjs (porcelain v2 + in-progress probe)"
```

---

### Task 4: lock.mjs (O_EXCL + liveness + age-backstop + ownership release)

**Files:**
- Create: `plugins/session-state/src/lock.mjs`
- Test: `plugins/session-state/test/lock.test.mjs`

**Interfaces:**
- Produces: `class LockTimeout extends Error`; `withLock(lockfile, fn, {tries=20, backoffMs=50, maxAgeMs=60000})` — acquires an exclusive lock, runs `fn()` (sync), releases (ownership-checked); throws `LockTimeout` if it can't acquire. (Sync API keeps callers simple; appends are short.)

- [ ] **Step 1: Write failing tests** `plugins/session-state/test/lock.test.mjs`

```javascript
import { test } from "node:test";
import assert from "node:assert";
import { mkdtempSync, writeFileSync, existsSync, readFileSync, utimesSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { withLock, LockTimeout } from "../src/lock.mjs";

test("withLock runs fn and releases (lockfile gone after)", () => {
  const lf = path.join(mkdtempSync(path.join(tmpdir(), "lk-")), "t.lock");
  let ran = false;
  withLock(lf, () => { ran = true; assert.ok(existsSync(lf)); });
  assert.ok(ran);
  assert.ok(!existsSync(lf));
});

test("live holder is not stolen → LockTimeout", () => {
  const lf = path.join(mkdtempSync(path.join(tmpdir(), "lk-")), "t.lock");
  // a live holder = THIS process's pid, fresh mtime
  writeFileSync(lf, JSON.stringify({ pid: process.pid, start: Date.now(), rand: 1 }));
  assert.throws(() => withLock(lf, () => {}, { tries: 2, backoffMs: 1 }), LockTimeout);
});

test("dead-PID lock is stolen", () => {
  const lf = path.join(mkdtempSync(path.join(tmpdir(), "lk-")), "t.lock");
  writeFileSync(lf, JSON.stringify({ pid: 999999999, start: Date.now(), rand: 1 })); // unused pid
  let ran = false;
  withLock(lf, () => { ran = true; }, { tries: 3, backoffMs: 1 });
  assert.ok(ran);
});

test("age-backstop steals a stale live-looking lock", () => {
  const lf = path.join(mkdtempSync(path.join(tmpdir(), "lk-")), "t.lock");
  writeFileSync(lf, JSON.stringify({ pid: process.pid, start: Date.now(), rand: 1 }));
  const old = (Date.now() - 120000) / 1000;
  utimesSync(lf, old, old);
  let ran = false;
  withLock(lf, () => { ran = true; }, { tries: 3, backoffMs: 1, maxAgeMs: 60000 });
  assert.ok(ran);
});
```

- [ ] **Step 2: Run to verify failure**

Run: `node --test plugins/session-state/test/lock.test.mjs`
Expected: FAIL — cannot find module.

- [ ] **Step 3: Implement `lock.mjs`**

```javascript
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
      if ((tok && !alive(tok.pid)) || stale) {
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
```

- [ ] **Step 4: Run tests to verify pass**

Run: `node --test plugins/session-state/test/lock.test.mjs plugins/session-state/test/import_guard.test.mjs`
Expected: PASS (4 lock tests).

- [ ] **Step 5: Commit**

```bash
git add plugins/session-state/src/lock.mjs plugins/session-state/test/lock.test.mjs
git commit -m "feat(plugin): lock.mjs (O_EXCL + liveness + age-backstop + ownership release)"
```

---

### Task 5: store.mjs

**Files:**
- Create: `plugins/session-state/src/store.mjs`
- Test: `plugins/session-state/test/store.test.mjs`

**Interfaces:**
- Consumes: `withLock` (Task 4), `renderLatestMd` (Task 6 — lazy import inside `appendRecord`).
- Produces: `nowIso()`, `makeRecord(type, source, sessionId, repo, git, narrative={})`, `scratchPath(dir, sessionId)`, `writeScratch(dir, rec)`, `readScratches(dir)`, `readTimeline(dir, {tail})`, `appendRecord(dir, rec)` → bool, `prune(dir, {maxRecords=1000, orphanDays=7})`.

- [ ] **Step 1: Write failing tests** `plugins/session-state/test/store.test.mjs`

```javascript
import { test } from "node:test";
import assert from "node:assert";
import { mkdtempSync, writeFileSync, readFileSync, existsSync, mkdirSync, utimesSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import * as s from "../src/store.mjs";

const mk = () => mkdtempSync(path.join(tmpdir(), "st-"));

test("nowIso ends with Z", () => assert.ok(s.nowIso().endsWith("Z")));
test("makeRecord auto omits narrative", () => { const r = s.makeRecord("auto", "x", "sid", "D:/r", { branch: "main" }); assert.equal(r.type, "auto"); assert.ok(!("did" in r)); });
test("scratch roundtrip", () => { const d = mk(); s.writeScratch(d, s.makeRecord("auto", "Stop", "sid1", "D:/r", { branch: "main" })); assert.equal(s.readScratches(d).length, 1); });
test("readTimeline skips corrupt", () => { const d = mk(); writeFileSync(path.join(d, "timeline.jsonl"), '{"ts":"1"}\nNOPE\n{"ts":"2"}\n'); assert.deepEqual(s.readTimeline(d).map(r => r.ts), ["1", "2"]); });
test("appendRecord writes line + latest.md", () => {
  const d = mk();
  assert.equal(s.appendRecord(d, s.makeRecord("rich", "save-state", "sid", "D:/r", { branch: "main" }, { did: "x", next: ["a"], open_threads: [] })), true);
  assert.equal(s.readTimeline(d).length, 1);
  assert.ok(existsSync(path.join(d, "latest.md")));
});
test("prune truncates oldest", () => {
  const d = mk();
  writeFileSync(path.join(d, "timeline.jsonl"), Array.from({ length: 10 }, (_, i) => `{"ts":"${i}"}`).join("\n") + "\n");
  s.prune(d, { maxRecords: 4 });
  assert.deepEqual(s.readTimeline(d).map(r => r.ts), ["6", "7", "8", "9"]);
});
test("prune removes orphan scratch", () => {
  const d = mk(); mkdirSync(path.join(d, "scratch"), { recursive: true });
  const f = path.join(d, "scratch", "old.json"); writeFileSync(f, "{}");
  const eight = (Date.now() - 8 * 86400000) / 1000; utimesSync(f, eight, eight);
  s.prune(d, { orphanDays: 7 });
  assert.ok(!existsSync(f));
});
```

- [ ] **Step 2: Run to verify failure**

Run: `node --test plugins/session-state/test/store.test.mjs`
Expected: FAIL — cannot find module. (Note: `appendRecord` lazily imports `merge.mjs`, added in Task 6; that one test fails until Task 6. Mark it `{ skip: true }` with a `// TODO Task 6` note, OR do Task 6 first. Prefer: keep it skipped, unskip in Task 6 Step 4.)

- [ ] **Step 3: Implement `store.mjs`**

```javascript
import fs from "node:fs";
import path from "node:path";
import { withLock, LockTimeout } from "./lock.mjs";

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
      // render latest.md inside the same lock hold (lazy import avoids cycle)
      const { renderLatestMd } = require_merge();
      const md = renderLatestMd(readTimeline(dir, { tail: 50 }), readScratches(dir));
      fs.writeFileSync(path.join(dir, "latest.md"), md);
    });
    return true;
  } catch (e) {
    if (e instanceof LockTimeout) return false;
    throw e;
  }
}

// lazy ESM import shim (dynamic import is async; use a cached synchronous require via createRequire)
import { createRequire } from "node:module";
let _merge = null;
function require_merge() {
  if (!_merge) {
    // merge.mjs is ESM; load synchronously via a tiny re-export bridge is not possible — so
    // import it eagerly at module top instead (see note). Replaced in Task 6 wiring.
  }
  return _merge;
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
```

> **Implementer note (resolve the lazy-merge correctly):** ESM has no synchronous `require` for `.mjs`. Replace the `require_merge()` placeholder with a **static top import** `import { renderLatestMd } from "./merge.mjs";` and call it directly inside `appendRecord`. `merge.mjs` (Task 6) imports nothing from `store.mjs`, so there is **no cycle** — the lazy shim is unnecessary. Use the static import; delete `require_merge`/`createRequire`. (This note exists because Task 5 lands before Task 6; if you implement Task 6 first, just write the static import now.)

- [ ] **Step 4: Run tests (the latest.md test stays skipped until Task 6)**

Run: `node --test plugins/session-state/test/store.test.mjs plugins/session-state/test/import_guard.test.mjs`
Expected: PASS for all except the skipped `appendRecord writes line + latest.md` (pending Task 6's `renderLatestMd`).

- [ ] **Step 5: Commit**

```bash
git add plugins/session-state/src/store.mjs plugins/session-state/test/store.test.mjs
git commit -m "feat(plugin): store.mjs (records, locked append, scratch, prune)"
```

---

### Task 6: merge.mjs

**Files:**
- Create: `plugins/session-state/src/merge.mjs`
- Modify: `plugins/session-state/src/store.mjs` (replace lazy-merge shim with `import { renderLatestMd } from "./merge.mjs";`)
- Test: `plugins/session-state/test/merge.test.mjs`

**Interfaces:**
- Produces: `resolveState(timeline, scratches)` → `{git, gitSource, narrative, branchBanner}`; `renderResumeBlock(timeline, scratches)` → string | null; `renderLatestMd(timeline, scratches)` → string.

- [ ] **Step 1: Write failing tests** `plugins/session-state/test/merge.test.mjs`

```javascript
import { test } from "node:test";
import assert from "node:assert";
import * as m from "../src/merge.mjs";

const rec = (ts, type, branch, { did, detached = false, in_progress = null } = {}) => {
  const git = { branch, detached, in_progress, head: "abc x", dirty: [], worktree: null, git_unavailable: false };
  const r = { ts, type, source: "t", session_id: "s", repo: "D:/r", git };
  if (type === "rich") { r.did = did || "did"; r.next = ["n1"]; r.open_threads = ["t1"]; }
  return r;
};

test("freshest git from scratch over older timeline", () => {
  const st = m.resolveState([rec("2026-01-01T00:00:00Z", "auto", "main")], [rec("2026-02-01T00:00:00Z", "auto", "feat/x")]);
  assert.equal(st.git.branch, "feat/x");
});
test("narrative is newest rich", () => {
  const st = m.resolveState([rec("2026-01-01T00:00:00Z", "rich", "main", { did: "old" }), rec("2026-02-01T00:00:00Z", "rich", "main", { did: "new" })], []);
  assert.equal(st.narrative.did, "new");
});
test("branch banner when branches differ", () => {
  const st = m.resolveState([rec("2026-01-01T00:00:00Z", "rich", "feat/x")], [rec("2026-02-01T00:00:00Z", "auto", "main")]);
  assert.ok(st.branchBanner && st.branchBanner.includes("feat/x"));
});
test("no banner when detached", () => {
  const st = m.resolveState([rec("2026-01-01T00:00:00Z", "rich", "feat/x")], [rec("2026-02-01T00:00:00Z", "auto", null, { detached: true })]);
  assert.equal(st.branchBanner, null);
});
test("resume block null when empty", () => assert.equal(m.renderResumeBlock([], []), null));
test("resume block has next + threads", () => {
  const b = m.renderResumeBlock([rec("2026-02-01T00:00:00Z", "rich", "main", { did: "shipped X" })], []);
  assert.ok(b.includes("shipped X") && b.includes("n1") && b.includes("t1"));
});
```

- [ ] **Step 2: Run to verify failure**

Run: `node --test plugins/session-state/test/merge.test.mjs`
Expected: FAIL — cannot find module.

- [ ] **Step 3: Implement `merge.mjs`**

```javascript
function newest(records) {
  let best = null;
  for (const r of records) if (!best || (r.ts || "") > (best.ts || "")) best = r;
  return best;
}

export function resolveState(timeline, scratches) {
  const gitBearing = [...timeline, ...scratches].filter((r) => r.git);
  const freshest = newest(gitBearing);
  const rich = newest(timeline.filter((r) => r.type === "rich"));
  let branchBanner = null;
  if (freshest && rich && freshest.git && rich.git) {
    const fg = freshest.git, rg = rich.git;
    const suppress = fg.detached || fg.in_progress;
    if (!suppress && fg.branch && rg.branch && fg.branch !== rg.branch) {
      const wt = rg.worktree ? ` (worktree ${rg.worktree})` : "";
      branchBanner = `narrative captured on \`${rg.branch}\`${wt} — newest activity is on \`${fg.branch}\``;
    }
  }
  return { git: freshest ? freshest.git : null, gitSource: freshest, narrative: rich, branchBanner };
}

function fmtGit(g) {
  if (!g) return "_no git facts_";
  if (g.git_unavailable) return "_git binary unavailable_";
  if (g.detached || g.in_progress) return `HEAD ${g.head || "?"} (${g.in_progress || "detached"})`;
  let line = `branch \`${g.branch}\` @ ${g.head || "?"}`;
  if (g.dirty && g.dirty.length) line += ` — ${g.dirty.length} changed`;
  return line;
}

export function renderResumeBlock(timeline, scratches) {
  if (!timeline.length && !scratches.length) return null;
  const st = resolveState(timeline, scratches);
  const lines = ["<session-state>", "Resuming this repo — last known state:", fmtGit(st.git)];
  if (st.branchBanner) lines.push(`⚠ ${st.branchBanner}`);
  const n = st.narrative;
  if (n) {
    if (n.did) lines.push(`Last: ${n.did}`);
    for (const i of (n.next || []).slice(0, 5)) lines.push(`Next: ${i}`);
    for (const i of (n.open_threads || []).slice(0, 5)) lines.push(`Open: ${i}`);
  } else {
    lines.push("(no narrative captured yet — run /save-state to record one)");
  }
  lines.push("</session-state>");
  return lines.join("\n");
}

export function renderLatestMd(timeline, scratches) {
  const st = resolveState(timeline, scratches);
  const out = ["# Latest session state", "", `**Git:** ${fmtGit(st.git)}`, ""];
  if (st.branchBanner) out.push(`> ⚠ ${st.branchBanner}`, "");
  const n = st.narrative;
  if (n) {
    out.push(`**Last:** ${n.did || ""}`, "", "**Next:**", ...(n.next || []).map((i) => `- ${i}`),
      "", "**Open threads:**", ...(n.open_threads || []).map((i) => `- ${i}`));
  } else out.push("_No narrative captured yet._");
  out.push("", "## Recent history", "");
  for (const r of timeline.slice(-5).reverse()) out.push(`- \`${r.ts}\` ${r.type} (${r.source})`);
  return out.join("\n") + "\n";
}
```

- [ ] **Step 4: Wire store→merge + unskip the Task-5 test, run full suite**

Edit `store.mjs`: delete the `require_merge`/`createRequire` shim and add at the top `import { renderLatestMd } from "./merge.mjs";`, calling it directly in `appendRecord`. Unskip `appendRecord writes line + latest.md` in `store.test.mjs`.

Run: `node --test plugins/session-state/test/`
Expected: PASS — all tests incl. the unskipped store test.

- [ ] **Step 5: Commit**

```bash
git add plugins/session-state/src/merge.mjs plugins/session-state/src/store.mjs plugins/session-state/test/merge.test.mjs plugins/session-state/test/store.test.mjs
git commit -m "feat(plugin): merge.mjs (branch-scoped merge + render) + store wiring"
```

---

### Task 7: resolve.mjs (registry resolution)

**Files:**
- Create: `plugins/session-state/src/resolve.mjs`
- Test: `plugins/session-state/test/resolve.test.mjs`

**Interfaces:**
- Consumes: `claudeHome` (Task 2).
- Produces: `pluginInstallPath(marketplaceName="command-center", pluginName="session-state")` → absolute path string | null. Reads `<claudeHome>/plugins/installed_plugins.json`, key `"<plugin>@<marketplace>"`, returns the first entry's `installPath` **if it exists on disk**; else scans `<claudeHome>/plugins/cache/<marketplace>/<plugin>/*` for the newest version dir containing `src/capture_rich.mjs`; else null.

- [ ] **Step 1: Write failing tests** `plugins/session-state/test/resolve.test.mjs`

```javascript
import { test } from "node:test";
import assert from "node:assert";
import { mkdtempSync, mkdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { pluginInstallPath } from "../src/resolve.mjs";

function seed(home, installPath, { create = true } = {}) {
  mkdirSync(path.join(home, "plugins"), { recursive: true });
  writeFileSync(path.join(home, "plugins", "installed_plugins.json"),
    JSON.stringify({ version: 2, plugins: { "session-state@command-center": [{ scope: "user", installPath, version: "0.1.0" }] } }));
  if (create) { mkdirSync(path.join(installPath, "src"), { recursive: true }); writeFileSync(path.join(installPath, "src", "capture_rich.mjs"), "//"); }
}

test("returns installPath from registry when it exists", () => {
  const home = mkdtempSync(path.join(tmpdir(), "cc-"));
  process.env.CLAUDE_CONFIG_DIR = home;
  const ip = path.join(home, "plugins", "cache", "command-center", "session-state", "0.1.0");
  seed(home, ip);
  assert.equal(pluginInstallPath(), ip);
  delete process.env.CLAUDE_CONFIG_DIR;
});

test("falls back to cache scan when registry path missing", () => {
  const home = mkdtempSync(path.join(tmpdir(), "cc-"));
  process.env.CLAUDE_CONFIG_DIR = home;
  seed(home, path.join(home, "GONE"), { create: false });
  const real = path.join(home, "plugins", "cache", "command-center", "session-state", "0.2.0");
  mkdirSync(path.join(real, "src"), { recursive: true }); writeFileSync(path.join(real, "src", "capture_rich.mjs"), "//");
  assert.equal(pluginInstallPath(), real);
  delete process.env.CLAUDE_CONFIG_DIR;
});

test("null when nothing found", () => {
  const home = mkdtempSync(path.join(tmpdir(), "cc-"));
  process.env.CLAUDE_CONFIG_DIR = home;
  assert.equal(pluginInstallPath(), null);
  delete process.env.CLAUDE_CONFIG_DIR;
});
```

- [ ] **Step 2: Run to verify failure**

Run: `node --test plugins/session-state/test/resolve.test.mjs`
Expected: FAIL — cannot find module.

- [ ] **Step 3: Implement `resolve.mjs`**

```javascript
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
```

- [ ] **Step 4: Run tests to verify pass**

Run: `node --test plugins/session-state/test/resolve.test.mjs plugins/session-state/test/import_guard.test.mjs`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add plugins/session-state/src/resolve.mjs plugins/session-state/test/resolve.test.mjs
git commit -m "feat(plugin): resolve.mjs (registry-based plugin path resolution)"
```

---

### Task 8: Entry scripts (resume, capture_scratch, capture_end, capture_rich)

**Files:**
- Create/replace: `plugins/session-state/src/resume.mjs`, `capture_scratch.mjs`, `capture_end.mjs` (replace the spike sentinels), `capture_rich.mjs`
- Test: `plugins/session-state/test/entries.test.mjs`

**Interfaces:**
- Consumes: `keying`, `gitfacts`, `store`, `merge`.
- Produces (each runnable as `node src/<entry>.mjs`): behaviors below. All: read stdin to end; check `CC_SESSION_STATE_DISABLE` first; hooks `process.exit(0)`.

- [ ] **Step 1: Write failing tests** `plugins/session-state/test/entries.test.mjs`

```javascript
import { test } from "node:test";
import assert from "node:assert";
import { execFileSync } from "node:child_process";
import { mkdtempSync, writeFileSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import * as store from "../src/store.mjs";
import * as keying from "../src/keying.mjs";

const SRC = path.join(path.dirname(fileURLToPath(import.meta.url)), "..", "src");
const node = process.execPath;

function gitRepo() {
  const d = mkdtempSync(path.join(tmpdir(), "repo-"));
  execFileSync("git", ["init", "-q"], { cwd: d });
  execFileSync("git", ["config", "user.email", "t@t"], { cwd: d });
  execFileSync("git", ["config", "user.name", "t"], { cwd: d });
  writeFileSync(path.join(d, "f.txt"), "x");
  execFileSync("git", ["add", "."], { cwd: d });
  execFileSync("git", ["commit", "-qm", "init"], { cwd: d });
  return d;
}

function run(entry, { cwd, stdin = "", env = {} }) {
  return execFileSync(node, [path.join(SRC, entry)], { cwd, input: stdin, encoding: "utf8", env: { ...process.env, ...env } });
}

test("capture_scratch writes scratch", () => {
  const home = mkdtempSync(path.join(tmpdir(), "cc-")), repo = gitRepo();
  run("capture_scratch.mjs", { cwd: repo, stdin: JSON.stringify({ session_id: "s1" }), env: { CLAUDE_CONFIG_DIR: home } });
  process.env.CLAUDE_CONFIG_DIR = home;
  assert.equal(store.readScratches(keying.stateDir(repo)).length, 1);
  delete process.env.CLAUDE_CONFIG_DIR;
});

test("kill-switch no-ops capture_scratch", () => {
  const home = mkdtempSync(path.join(tmpdir(), "cc-")), repo = gitRepo();
  run("capture_scratch.mjs", { cwd: repo, stdin: JSON.stringify({ session_id: "s1" }), env: { CLAUDE_CONFIG_DIR: home, CC_SESSION_STATE_DISABLE: "1" } });
  process.env.CLAUDE_CONFIG_DIR = home;
  assert.equal(store.readScratches(keying.stateDir(repo)).length, 0);
  delete process.env.CLAUDE_CONFIG_DIR;
});

test("capture_end skips clear reason", () => {
  const home = mkdtempSync(path.join(tmpdir(), "cc-")), repo = gitRepo();
  run("capture_end.mjs", { cwd: repo, stdin: JSON.stringify({ session_id: "s", reason: "clear" }), env: { CLAUDE_CONFIG_DIR: home } });
  process.env.CLAUDE_CONFIG_DIR = home;
  assert.equal(store.readTimeline(keying.stateDir(repo)).length, 0);
  delete process.env.CLAUDE_CONFIG_DIR;
});

test("resume silent on compact, envelope on startup", () => {
  const home = mkdtempSync(path.join(tmpdir(), "cc-")), repo = gitRepo();
  assert.equal(run("resume.mjs", { cwd: repo, stdin: JSON.stringify({ source: "compact" }), env: { CLAUDE_CONFIG_DIR: home } }).trim(), "");
  // seed a record then resume on startup
  process.env.CLAUDE_CONFIG_DIR = home;
  const dir = keying.stateDir(repo);
  store.appendRecord(dir, store.makeRecord("rich", "save-state", "s", repo, { branch: "main", head: "abc x", dirty: [] }, { did: "hi", next: [], open_threads: [] }));
  delete process.env.CLAUDE_CONFIG_DIR;
  const out = run("resume.mjs", { cwd: repo, stdin: JSON.stringify({ source: "startup" }), env: { CLAUDE_CONFIG_DIR: home } });
  assert.ok(out.includes("hookSpecificOutput") && out.includes("hi"));
});

test("capture_rich appends + deletes temp on success", () => {
  const home = mkdtempSync(path.join(tmpdir(), "cc-")), repo = gitRepo();
  const payload = path.join(mkdtempSync(path.join(tmpdir(), "p-")), "r.json");
  writeFileSync(payload, JSON.stringify({ did: "shipped", next: ["a"], open_threads: [] }));
  run("capture_rich.mjs", { cwd: repo, stdin: "", env: { CLAUDE_CONFIG_DIR: home, SS_INPUT: payload } });
  assert.ok(!existsSync(payload));
  process.env.CLAUDE_CONFIG_DIR = home;
  const recs = store.readTimeline(keying.stateDir(repo));
  assert.ok(recs.length && recs[recs.length - 1].did === "shipped");
  delete process.env.CLAUDE_CONFIG_DIR;
});
```

> Note: the test invokes `capture_rich.mjs` with the input path via env `SS_INPUT` for simplicity; the script accepts **both** `--input <file>` (the skill's contract) and `SS_INPUT` (test/convenience). Implement both.

- [ ] **Step 2: Run to verify failure**

Run: `node --test plugins/session-state/test/entries.test.mjs`
Expected: FAIL — entry modules don't exist yet (or are the spike sentinels).

- [ ] **Step 3a: Implement `resume.mjs`**

```javascript
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
```

- [ ] **Step 3b: Implement `capture_scratch.mjs`**

```javascript
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
```

- [ ] **Step 3c: Implement `capture_end.mjs`**

```javascript
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
```

- [ ] **Step 3d: Implement `capture_rich.mjs`**

```javascript
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

let deleteTemp = true;
try {
  const data = JSON.parse(fs.readFileSync(input, "utf8"));
  const cwd = process.cwd();
  const root = keying.repoRoot(cwd);
  const repo = root || cwd;
  const dir = keying.stateDir(cwd);
  if (!keying.checkMeta(dir, repo)) { deleteTemp = false; console.error("session-state: repo-key collision; narrative NOT saved."); process.exit(1); }
  const git = gitfacts.collectGitFacts(cwd);
  const rec = store.makeRecord("rich", "save-state", data.session_id || null, repo, git,
    { did: data.did || "", next: data.next || [], open_threads: data.open_threads || [] });
  if (store.appendRecord(dir, rec)) { console.log("session-state: narrative saved."); process.exit(0); }
  deleteTemp = false;
  console.error(`session-state: could not acquire lock; narrative NOT saved. Temp preserved at ${input}. Retry: node capture_rich.mjs --input ${input}`);
  process.exit(1);
} catch (e) {
  deleteTemp = false;
  console.error(`session-state: error saving narrative: ${e.message}. Temp preserved at ${input}.`);
  process.exit(1);
} finally {
  if (deleteTemp) { try { fs.unlinkSync(input); } catch {} }
}
```

- [ ] **Step 4: Run tests to verify pass**

Run: `node --test plugins/session-state/test/entries.test.mjs plugins/session-state/test/import_guard.test.mjs`
Expected: PASS (requires `git` on PATH).

- [ ] **Step 5: Commit**

```bash
git add plugins/session-state/src/resume.mjs plugins/session-state/src/capture_scratch.mjs plugins/session-state/src/capture_end.mjs plugins/session-state/src/capture_rich.mjs plugins/session-state/test/entries.test.mjs
git commit -m "feat(plugin): entry scripts (resume/scratch/end/rich)"
```

---

### Task 9: cli.mjs + manifest validation

**Files:**
- Create: `plugins/session-state/src/cli.mjs`
- Test: `plugins/session-state/test/cli.test.mjs`, `plugins/session-state/test/manifest.test.mjs`

**Interfaces:**
- Consumes: `keying`, `store`, `merge`.
- Produces: `cli.mjs` with `list` / `show [SELECTOR]` / `prune [SELECTOR]` (SELECTOR = canonical path or repo-key; default = cwd's canonical repo).

- [ ] **Step 1: Write failing tests** `plugins/session-state/test/cli.test.mjs`

```javascript
import { test } from "node:test";
import assert from "node:assert";
import { execFileSync } from "node:child_process";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import * as store from "../src/store.mjs";
import * as keying from "../src/keying.mjs";

const CLI = path.join(path.dirname(fileURLToPath(import.meta.url)), "..", "src", "cli.mjs");
const node = process.execPath;

function gitRepo() {
  const d = mkdtempSync(path.join(tmpdir(), "repo-"));
  for (const a of [["init", "-q"], ["config", "user.email", "t@t"], ["config", "user.name", "t"]]) execFileSync("git", a, { cwd: d });
  writeFileSync(path.join(d, "f.txt"), "x"); execFileSync("git", ["add", "."], { cwd: d }); execFileSync("git", ["commit", "-qm", "i"], { cwd: d });
  return d;
}

test("cli show renders state", () => {
  const home = mkdtempSync(path.join(tmpdir(), "cc-")), repo = gitRepo();
  process.env.CLAUDE_CONFIG_DIR = home;
  store.appendRecord(keying.stateDir(repo), store.makeRecord("rich", "save-state", "s", repo, { branch: "main", head: "abc x", dirty: [] }, { did: "hello", next: [], open_threads: [] }));
  delete process.env.CLAUDE_CONFIG_DIR;
  const out = execFileSync(node, [CLI, "show"], { cwd: repo, encoding: "utf8", env: { ...process.env, CLAUDE_CONFIG_DIR: home } });
  assert.ok(out.includes("hello"));
});

test("cli list runs", () => {
  const home = mkdtempSync(path.join(tmpdir(), "cc-")), repo = gitRepo();
  execFileSync(node, [CLI, "list"], { cwd: repo, encoding: "utf8", env: { ...process.env, CLAUDE_CONFIG_DIR: home } });
});
```

- [ ] **Step 2: Write manifest test** `plugins/session-state/test/manifest.test.mjs`

```javascript
import { test } from "node:test";
import assert from "node:assert";
import { readFileSync, existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

const ROOT = path.join(path.dirname(fileURLToPath(import.meta.url)), "..");
const REPO = path.join(ROOT, "..", "..");

test("manifests are valid JSON and reference existing scripts", () => {
  const plugin = JSON.parse(readFileSync(path.join(ROOT, ".claude-plugin", "plugin.json"), "utf8"));
  const market = JSON.parse(readFileSync(path.join(REPO, ".claude-plugin", "marketplace.json"), "utf8"));
  const hooks = JSON.parse(readFileSync(path.join(ROOT, "hooks", "hooks.json"), "utf8"));
  assert.equal(plugin.name, "session-state");
  assert.equal(market.name, "command-center");
  assert.equal(plugin.version, market.plugins.find((p) => p.name === "session-state").version, "version drift: plugin.json vs marketplace.json");
  for (const evt of Object.keys(hooks.hooks)) {
    for (const entry of hooks.hooks[evt]) for (const h of entry.hooks) {
      const rel = h.args[0].replace("${CLAUDE_PLUGIN_ROOT}/", "");
      assert.ok(existsSync(path.join(ROOT, rel)), `missing hook script ${rel}`);
    }
  }
});
```

- [ ] **Step 3: Run to verify failure**

Run: `node --test plugins/session-state/test/cli.test.mjs plugins/session-state/test/manifest.test.mjs`
Expected: FAIL — `cli.mjs` missing (cli tests); manifest test may pass already.

- [ ] **Step 4: Implement `cli.mjs`**

```javascript
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
```

- [ ] **Step 5: Run all tests + commit**

Run: `node --test plugins/session-state/test/`
Expected: PASS (full suite).

```bash
git add plugins/session-state/src/cli.mjs plugins/session-state/test/cli.test.mjs plugins/session-state/test/manifest.test.mjs
git commit -m "feat(plugin): cli.mjs (list/show/prune) + manifest validation"
```

---

### Task 10: save-state skill (registry-resolved)

**Files:**
- Replace: `plugins/session-state/skills/save-state/SKILL.md` (remove the spike stub)

**Interfaces:**
- Consumes: `resolve.mjs` (the agent reads the registry per the skill body), `capture_rich.mjs`.

- [ ] **Step 1: Write the real `SKILL.md`**

```markdown
---
name: save-state
description: Save the current dev-session's resumable state (what we did, next steps, open threads) to the per-repo session-state timeline so the next session resumes instantly. Use at the end of a work session, at a phase/spike boundary, or when the user says "save state", "checkpoint", or before ending a session.
---

# Save Session State

Append an agent-authored **rich** record to this repo's session-state timeline (auto git facts are
captured by hooks; this records the *meaning*).

## Steps

1. Compose the narrative from this session:
   - `did`: 1-3 sentences — what was accomplished and where work paused.
   - `next`: list of concrete next actions.
   - `open_threads`: active bugs, blockers, pending decisions, things to watch.
2. Write it to a uniquely-named temp JSON file in the OS temp dir:
   ```json
   { "did": "...", "next": ["..."], "open_threads": ["..."] }
   ```
3. Resolve the plugin's script path from the registry (the plugin's install dir is version-stamped, so
   read it rather than hardcode). Read `~/.claude/plugins/installed_plugins.json` (honor
   `CLAUDE_CONFIG_DIR`), key `"session-state@command-center"`, take the first entry's `installPath`.
   If that path doesn't exist, scan `~/.claude/plugins/cache/command-center/session-state/` for the
   newest version dir containing `src/capture_rich.mjs`.
4. Run: `node "<installPath>/src/capture_rich.mjs" --input "<tempfile>"`
5. Read the output:
   - "session-state: narrative saved." → done (the temp file was deleted for you).
   - "narrative NOT saved … Temp preserved at <path>" → tell the user; do NOT blind-retry; surface the
     printed retry command.

## Notes
- This is invoked by you (the model) via this skill's description — it is not auto-called by
  `end-session`. Run it at the end of a session or a phase boundary.
- The next session's SessionStart hook surfaces this automatically.
```

- [ ] **Step 2: Manual verification (no unit test — skill is docs)**

In a temp git repo, simulate the skill's hop manually:
```bash
echo '{"did":"manual test","next":["x"],"open_threads":[]}' > "$TMP/r.json"
node plugins/session-state/src/capture_rich.mjs --input "$TMP/r.json"
node plugins/session-state/src/cli.mjs show
```
Expected: "narrative saved." then `show` renders "manual test"; temp file gone.

- [ ] **Step 3: Commit**

```bash
git add plugins/session-state/skills/save-state/SKILL.md
git commit -m "feat(plugin): real save-state skill (registry-resolved capture_rich)"
```

---

### Task 11: Migration — remove merged Python tool (atomic swap) + acceptance

**Files:**
- Remove: `tools/session-state/` (entire dir), `install.ps1` (if present at repo root or under tools), `.claude/skills/save-state/SKILL.md`
- Verify: live `~/.claude/settings.json`

**Interfaces:** none new (migration + acceptance).

- [ ] **Step 1: De-conflict the live machine FIRST (abort gate)**

The live `~/.claude/settings.json` references the **installed** Python hooks under
`~/.claude/tools/session-state/…`. Run the merged installer's uninstall:
```bash
pwsh -NoProfile -File tools/session-state/install.ps1 -Uninstall
```
Then verify settings.json no longer references the Python scripts:
```bash
node -e "const s=require(require('os').homedir()+'/.claude/settings.json');const j=JSON.stringify(s.hooks||{});console.log(/session_state|session-state[\\\\/].*\.py/.test(j)?'STILL-REFERENCED':'CLEAN')"
```
Expected: `CLEAN`. **If `STILL-REFERENCED`, STOP** — do not proceed (double-fire risk); investigate the marker mismatch first.

- [ ] **Step 2: Atomic swap — remove the merged repo Python skill + tool in one commit**

```bash
git rm -r tools/session-state .claude/skills/save-state
# install.ps1 lived under tools/session-state, removed above; if a root copy exists: git rm install.ps1
git commit -m "chore(session-state): remove merged Python tool + manual skill (superseded by plugin)"
```

- [ ] **Step 3: Install the Node plugin for this machine + acceptance**

```bash
pwsh -NoProfile -Command "claude plugin marketplace add 'D:\MajorProjects\CURRENT\command-center'; claude plugin install session-state@command-center"
```
Then verify exactly one `save-state` resolves and it's the plugin's (not Python):
```bash
pwsh -NoProfile -Command "claude plugin list | Select-String session-state"
```
Expected: `session-state@command-center … enabled`. The merged `.claude/skills/save-state` is gone, so no collision.

- [ ] **Step 4: Full suite + end-to-end acceptance**

Run: `node --test plugins/session-state/test/`
Expected: ALL PASS.

Manual end-to-end (real Claude Code session in this repo): start a session → confirm the resume block appears (or "no narrative yet"); end the session → confirm an `auto` record via `node plugins/session-state/src/cli.mjs show`; invoke `/save-state` → confirm a `rich` record is written.

- [ ] **Step 5: Final commit + open PR**

```bash
git add -A && git commit -m "test(session-state): plugin acceptance notes" || echo "nothing to commit"
```
Open a PR from `spike/session-state-plugin` (rename to `feat/session-state-plugin` if preferred) into `main`. The PR removes the merged Python tool and adds the Node plugin.

---

## Self-Review

**1. Spec coverage:**
- §2 plugin layout / hooks (shape i) / marketplace / version contract → Tasks 1, 9 (manifest test asserts version parity). ✓
- §3 runtime port (keying/gitfacts/lock/store/merge/resolve/entries), parse-compatible format, save-state-as-skill via registry → Tasks 2–8, 10. ✓
- §3 lock (O_EXCL + liveness + age-backstop + ownership release) → Task 4. ✓
- §6 migration (de-conflict first w/ abort gate, atomic skill swap, install) → Task 11. ✓
- §7 tests (import guard, slug parity, porcelain, lock incl. dead-PID/age, store/merge, entries real-invocation, resolve, manifest+version) → every task + Task 9. ✓
- §4/§5 (Phase 2/3) → out of scope (spike-gated), correctly omitted.

**2. Placeholder scan:** No TBD/TODO. The one cross-task nuance (store→merge lazy import) is resolved explicitly in Task 6 Step 4 with the implementer note in Task 5 Step 3.

**3. Type consistency:** record shape from `store.makeRecord` is consumed unchanged by `merge.resolveState`/renders, the entries, and cli. `withLock(lockfile, fn, opts)` signature matches store's two call sites. `pluginInstallPath()` return (path|null) matches the skill's usage. `stateDir(cwd,{create})` create-flag is used `false` on read paths (resume, cli) and default-true on write paths.

**Note:** Spikes 0a/0b are already PASSED (`spikes/SPIKE-RESULTS-session-state-plugin.md`); Phase 1 needs no further spike. The spike skeleton's sentinel `resume.mjs`/`capture_scratch.mjs` are replaced in Task 8; `hooks.json`/`plugin.json`/`marketplace.json`/`skills/save-state/SKILL.md` are finalized in Tasks 1/10.
