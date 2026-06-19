# Session-State Resume — Design Spec

> Status: **design approved** (brainstorming complete, 3 adversarial critique rounds passed),
> pre-implementation. **Design only — this spec builds no code.**
> Date: 2026-06-19. Lane: `workflow`.
> Serves the North Star pillar **context hygiene** (durable resume state out of the active window,
> pulled on demand) and **ship autonomously & fast** (zero-friction resume across all repos).
> Forward-compatible with roadmap item **4 — Central Project Manager** (the JSONL timeline can feed
> the dashboard later) and a future SQLite/daemon upgrade.

## 1. Goal & scope

Persist the most recent state of my **dev work sessions** so resuming any repo is zero-friction, and
keep durable snapshots as an **append-only timeline** so the same store can later track work over
time. The operator opens Claude Code in a repo and is automatically reminded of what got done, where
work paused, what's next, and which threads/gotchas are open — without reconstructing it from
transcripts or hand-maintaining a `CLAUDE.md` pickup pointer.

**In scope (this spec — design only):**
- A **global, machine-wide** capability covering **all repos**, central storage under
  `~/.claude/state/sessions/`, keyed by **canonical** repo (worktrees collapse to their parent).
- A **three-trigger capture** model (`§4`): a cheap, throttled `Stop`-hook **per-session scratch**
  (freshest git facts, crash-safe); a best-effort `SessionEnd` **boundary** record; agent-authored
  **rich** narrative via a `save-state` skill that `end-session` drives (`§4`, `§6`).
- A **snapshot data model** (`§2`) and the **append-only JSONL timeline + per-session scratch +
  Markdown render** storage format (`§3`).
- The **branch-scoped merge rule** (`§5`) defining "latest state," correct under concurrent sessions.
- **Components** (`§6`): scratch-capture, boundary-capture, rich-capture skill, resume, cli, install.
- **Wiring/install + uninstall** into `~/.claude/settings.json` via a PowerShell installer (`§7`),
  invoking the Python interpreter **directly** (the deployed `recall.py` precedent), not `uv run`.
- **Retention/rotation** (`§8`), **edge cases & concurrency** (`§9`), **testing** (`§10`).

**Out of scope (named so we don't build them):**
- **The managed-projects status dashboard** (project lifecycle stage across tools) — roadmap item 4,
  a separate spec. This spec's subject is *my dev sessions*. The two meet only at the JSONL seam.
- **A daemon/SQLite store or any cockpit UI** — deferred; the JSONL is the forward-compatible seam.
- **Multi-machine / hosted sync** — single operator, single machine. State is local-only.
- **AI summarization inside any hook** — hooks are AI-free; rich narrative is always agent-authored.
- **Replacing `end-session`/`handoff`** — the `save-state` skill complements them (and is driven by
  `end-session`, `§4`/`§7`).
- **Slug-collision hash-fallback machinery** — v1 detects a collision and **refuses loudly** (`§9`);
  the dual-dir hash fallback is deferred until a real collision ever occurs (YAGNI).
- **Month-bucketed archive rollover & ahead/behind tracking** — v1 keeps one timeline with a
  truncating `prune`; upstream `branch.ab` is not parsed or rendered (`§8`).

## 2. Snapshot data model

Two durable record kinds live in the timeline (`auto` boundary, `rich` narrative); a transient
**scratch** form (`§3`) shares the `git` shape but is never appended.

```json
{
  "ts": "2026-06-19T14:05:00Z",          // UTC ISO-8601 (tz-aware); semantics: see §4 (write vs change)
  "type": "auto",                          // "auto" (boundary) | "rich" (narrative)
  "source": "SessionEnd:other",            // provenance: which trigger wrote it (§4)
  "session_id": "ad8502f9-…",             // always present from hook stdin; null only for CLI
  "repo": "D:/MajorProjects/CURRENT/command-center",  // CANONICAL repo root (§3); cwd if non-git
  "git": {                                  // the WHOLE key is null when cwd is not a git repo
    "branch": "main",                       // null when detached (don't trust "HEAD")
    "detached": false,                      // true when HEAD is detached
    "in_progress": null,                    // "rebase" | "merge" | "bisect" | null  (filesystem probe, §9)
    "head": "a59b363 docs(session): P3 spike pickup",  // short sha + subject (always meaningful)
    "dirty": ["crates/fleetd/src/foo.rs"],  // changed/untracked/unmerged paths, capped (§9)
    "worktree": ".claude/worktrees/agent-x", // linked worktree subpath if in one, else null
    "git_unavailable": false               // true if the git binary couldn't be resolved (§9 #10)
  },

  // rich-only narrative fields (absent on auto records):
  "did": "Short prose: what got done this session + where we paused.",
  "next": ["Concrete next action", "…"],
  "open_threads": ["Active bug X deadlocks because …", "Decision pending: …"]
}
```

- `git` is `null` (the whole key) when the cwd is not a git repo; merge/readers must null-check it.
  When the git *binary* is missing, `git` is present with `git_unavailable: true` so the failure is
  visible in `cli show` rather than silently producing empty resumes (`§9`).
- `branch`/`detached`/`in_progress`/`worktree` are recorded on **every** record so the merge can
  branch-scope and suppress noise from detached/mid-operation states (`§5`, `§9`).
- Narrative fields are present only on `rich` records.

## 3. Storage layout & format

```
~/.claude/state/sessions/
  <repo-key>/
    meta.json              # { "repo": "<canonical abs path>" } — collision guard (§9)
    scratch/
      <session_id>.json    # PER-SESSION scratch: freshest git facts, overwritten each Stop.
    timeline.jsonl         # append-only durable record (auto boundaries + rich narratives)
    timeline.lock          # advisory-lock target for appends + latest.md regen (§9)
    latest.md              # render: merged current state + history tail (written under the lock, §6/§9)
```

- **`scratch/<session_id>.json`** is **per session** (not shared). Each Stop overwrites only the
  current session's file — so concurrent sessions in different worktrees of the same repo (which
  share the repo-key) never stomp each other. Crash-safe; survives Ctrl-C / crash / `/clear` where
  `SessionEnd` may not fire (`§4`). Orphans (crashed sessions) pruned by age (`§8`).
- **`timeline.jsonl`** is the durable append-only source of truth: `auto` boundaries + `rich`
  narratives. What a future dashboard / SQLite importer reads. v1 keeps a **single** file; `cli prune`
  truncates the oldest records past the cap (no month buckets — YAGNI for one operator, `§8`).
- **`latest.md`** is derived and disposable, regenerated **only on a durable append, inside the same
  lock hold** (`§6`/`§9`) — never on the read/resume path.
- **`meta.json`** records the canonical repo path so a slug collision (`§9`) is detected and refused.

### Repo keying (vendor the proven `recall.py` logic — do not import across tools)

Reuse the **logic** of `~/.claude/tools/context-offload/recall.py`, but **vendor** (copy) the ~30
relevant lines into `tools/session-state/` with a comment citing the source and a **parity test**
(`§10`). Do **not** make either tool import a module from the other's directory: they live in
different, independently-installed locations with no shared import path, and cross-tool imports create
install-order coupling.

- `canonical_project_root(cwd)` — collapse `<root>/.claude/worktrees/<name>` back to `<root>` so a
  worktree **shares** its parent repo's timeline (continuity is the point; `git.worktree`
  disambiguates the checkout). Root via `git rev-parse --show-toplevel`, then canonicalized; cwd if
  non-git.
- `path_to_slug(path)` — Claude Code's exact slug scheme (`\`,`/`,`:` → `-`), matched
  **case-insensitively** on read.

## 4. Capture model (three triggers)

`SessionEnd` is **not** a reliable "every session end" — it does not fire on crash/interrupt, and it
*does* fire on `/clear` and `--resume`. So capture is split across three triggers, each doing only
what it reliably can:

| Trigger | Writes | Why |
|---|---|---|
| **`Stop` hook** (fires per turn) | overwrites this session's `scratch/<session_id>.json` | Freshest git facts on disk, crash-safe, for the cases `SessionEnd` misses. **Never appends.** **Throttled + cheap** (below). |
| **`SessionEnd` hook** (best-effort) | appends one `auto` boundary record; deletes **only its own** `scratch/<session_id>.json` | Marks a real boundary. **Filtered by `reason`:** skip `clear`/`resume`; record `logout`/`prompt_input_exit`/`other`. If it never fires, the scratch still carries freshest facts. |
| **`save-state` skill** (agent, **driven by `end-session`**) | appends one `rich` record | A hook cannot summarize; meaning is agent-authored (`did`/`next`/`open_threads`). |

**Who writes `rich` records (the headline value, so it must be guaranteed, not opt-in):** `save-state`
is **not** meant to be manually remembered. The existing `end-session` skill already produces a
session summary + next steps; it is wired to call `save-state` (which runs `capture_rich.py`) as its
final step, so every clean end-session writes a `rich` record. `save-state` may still be invoked
directly at a phase/spike boundary. Resume degrades gracefully to facts-only when no rich record
exists, but the normal path guarantees one. This is the design's answer to "will rich records ever
exist."

**`Stop` cost control (runs every turn, behind the existing cache-timer + budget hooks):**
- Resolve the git binary once via `shutil.which("git")` (or a configured absolute path); if absent,
  write scratch with `git_unavailable: true` (`§9` #10) rather than silently no-op'ing forever.
- **One** git call: `git --no-optional-locks status --porcelain=v2 --branch` yields branch, detached
  state, and dirty paths in one pass (parse contract in `§9`); fetch the `head` subject from a second
  cheap `git log -1 --format=…` only when the sha changed.
- **Throttle (write-rate, not fact-freshness):** rewrite is skipped if the scratch was written within
  **T seconds (default 30)** **unless the git facts changed** since the last write. The `ts` is
  always stamped to *now* when facts actually change, so the merge's "freshest by `ts`" never lags
  real activity by more than the unchanged-throttle window. (See `§9` for the documented resolution
  limit.)
- **Bounded:** explicit hook `timeout` (5s) in the installer; on any error/timeout it exits 0 and the
  turn is unaffected. Honest cost: a process spawn + one or two git calls on the throttled turns,
  nothing on the rest — bounded and off the critical path, not literally sub-100ms.
- **Kill-switch:** all three hooks check `CC_SESSION_STATE_DISABLE` first and no-op if set (`§7`).

## 5. The branch-scoped merge rule (what "latest state" means)

Computed at read time, correct under concurrency:

- **Git facts** = the **freshest by `ts`** across (a) all `scratch/*.json` for this repo-key and (b)
  the newest `git`-bearing timeline record. Scanning *all* per-session scratch files (not one shared
  file) is what makes concurrent worktree sessions safe — resume picks the genuinely newest and knows
  its branch/worktree. "Freshest" means freshest *write* (the throttle, §4, keeps `ts` honest by
  re-stamping whenever facts change, so write-time ≈ change-time within T seconds).
- **Narrative** = the most recent `rich` record.
- **Branch-scope guard (applies scratch-vs-rich AND scratch-vs-scratch):** if the chosen freshest git
  facts and the chosen narrative were captured on **different branches/worktrees**, render the
  narrative under an explicit banner: *"narrative captured on `feat/x` (worktree …) — newest activity
  is on `main`."* Never fuse them silently.
- **Detached / mid-operation suppression:** if the freshest facts are `detached` or `in_progress`
  (rebase/merge/bisect), `branch` is `null`/untrusted — show `head` sha + the operation state and
  **suppress** the branch-mismatch banner (a rebase is not a "different branch of work").

If no `rich` record exists, the narrative section reads "no narrative captured yet." `latest.md` and
the resume hook render exactly this merged, branch-scoped, detached-aware view.

## 6. Components

All scripts: Python **stdlib only**, UTF-8-reconfigured stdout/stderr (`recall.py` precedent),
wrap-all-in-try/except. **Hooks always exit 0** and emit nothing on the empty/error path. The
**skill** path surfaces errors (it is agent-invoked, not a hook) and must never silently lose data.

1. **Scratch-capture** — `capture_scratch.py`, invoked by the **`Stop`** hook. Reads stdin
   (`session_id`), applies the throttle (`§4`), runs the single combined git call, overwrites
   `scratch/<session_id>.json`. No append, no render.

2. **Boundary-capture** — `capture_end.py`, invoked by the **`SessionEnd`** hook. Reads stdin
   (`session_id`, `reason`); if `reason ∈ {clear, resume}` exits 0 doing nothing; else acquires the
   lock, appends one `auto` record, regenerates `latest.md`, releases (append+render are **one lock
   hold**, `§9`), then deletes **only** `scratch/<session_id>.json`.

3. **Rich-capture skill** — a `save-state` skill. **A skill cannot pipe stdin to a script**, so the
   contract is: the agent (a) writes the narrative to a **uniquely-named temp JSON file in the OS temp
   dir** (`{ "did", "next", "open_threads" }`) and (b) runs `python capture_rich.py --input
   <tempfile>`. File-path-on-argv + JSON-in-file sidesteps both argv quoting of multi-line prose and
   the nonexistent stdin mechanism. `capture_rich.py` reads cwd git facts, acquires the lock, appends
   one `rich` record, regenerates `latest.md` (same lock hold), releases, and **always deletes the
   temp file in a `finally`**. **Durable-narrative guarantee:** on lock-acquire failure after bounded
   retry it does **not** silently drop the record — it **preserves the temp file** and prints
   `narrative NOT saved; temp preserved at <path>; retry with: python capture_rich.py --input <path>`.
   The skill tells the agent to surface that to the user and not blind-retry. Complements
   `end-session`/`handoff` and is **driven by `end-session`** (`§4`); obsoletes the `CLAUDE.md`
   pickup pointer.

4. **Resume** — `resume.py`, invoked by the **`SessionStart`** hook. Reads stdin `source`; emit the
   block **only when `source ∈ {startup, resume}`** (silent on `compact`/`clear` — re-injecting after
   every compaction fights context hygiene). Computes the merge (`§5`) and prints it. **Output:** for
   SessionStart, plain stdout already reaches context (this is why `recall.py`'s plain-text injection
   works); v1 nonetheless emits the explicit, forward-compatible JSON envelope
   `{"hookSpecificOutput":{"hookEventName":"SessionStart","additionalContext":"<session-state>…"}}`
   — the envelope is **optional**, not required, and is chosen only for explicitness. **Read-only:**
   resume never writes any file. The block is **terse** (merged current state + ≤5 next/open-thread
   bullets) so it adds minimal context alongside the existing `recall.py` block. Prints nothing if no
   state exists.

5. **CLI** — `cli.py`: `list` (repos with state; flags collisions), `show [SELECTOR]`,
   `prune [SELECTOR]` (timeline truncation past the cap + orphan-scratch cleanup),
   `uninstall`/`install` delegating to the installer. **`SELECTOR` = canonical path or repo-key**;
   default = cwd's canonical repo.

## 7. Wiring, install & uninstall

**Installer** — `tools/session-state/install.ps1` (PowerShell), mirroring `cache-countdown` but
following the **deployed `recall.py` hook shape** (direct interpreter), not `uv run`:

- Invoke an absolute `python.exe` **directly** (as the live `recall.py` SessionStart hook does).
  `uv run` adds a lockfile/venv check (hundreds of ms; a cold sync can exceed the 10s SessionStart
  timeout and silently drop resume) — unacceptable on every boundary for a stdlib-only script.
  `uv`/`pytest` are **dev/test only**.
- Adds three hook entries; **mirror the live schema** — the existing SessionStart entry has **no
  `matcher` key**, so match that shape; set an explicit `timeout` on the `Stop` entry:

```jsonc
// SessionStart → resume     (coexists with the recall.py hook already present)
{ "hooks": [{ "type": "command", "command": "<py> <abs>/tools/session-state/resume.py" }] }
// Stop → scratch            (bounded so it can never stall a turn)
{ "hooks": [{ "type": "command", "command": "<py> <abs>/tools/session-state/capture_scratch.py", "timeout": 5 }] }
// SessionEnd → boundary
{ "hooks": [{ "type": "command", "command": "<py> <abs>/tools/session-state/capture_end.py" }] }
```

- **Idempotency:** detect existing entries by a **path-independent marker** (the script basename,
  e.g. `session-state/resume.py`), normalize, **replace** rather than duplicate (a moved checkout
  changes `<abs>` but not the basename). Tested against a fixture already containing `recall.py`.
- **Uninstall:** `install.ps1 -Uninstall` removes exactly the three marker-identified entries (and
  nothing else); state under `~/.claude/state/sessions/` is left intact unless `-Purge` is passed.
  A per-process **kill-switch** env var `CC_SESSION_STATE_DISABLE` makes all three hooks no-op
  without editing settings — for disabling in one shell/repo.
- **Coexistence:** the new SessionStart hook runs alongside the existing `recall.py` hook (event
  hooks run in parallel; both append `additionalContext`; order irrelevant). No existing hook
  modified. Each script is self-contained; none depends on another hook's side effects.

> Code lives in the command-center checkout (`tools/session-state/`), installs globally via absolute
> paths — as `tools/cache-countdown` does. The Command Center is the home of these workflow tools.

## 8. Retention & rotation (deliberately minimal for v1)

- On append, if `timeline.jsonl` exceeds **N records (default 1000)**, `capture_*`/`cli prune`
  truncate the **oldest** records to the cap. One operator's dev sessions will rarely hit this;
  month-bucketed archives and age-based rollover are **deferred** (YAGNI) — a single truncating file
  is enough.
- **Orphan scratch cleanup:** `scratch/<session_id>.json` files older than **D days (default 7)** —
  left by crashed sessions whose `SessionEnd` never deleted them — are removed by `capture_end.py` and
  `cli prune`. Bounds the scratch dir.
- `latest.md` never grows (regenerated under the lock).

## 9. Edge cases, concurrency & errors

- **Crash / Ctrl-C / `/clear`:** `SessionEnd` may not fire — the per-session `Stop` scratch already
  holds freshest facts, so resume stays accurate. (The reason the scratch exists.)
- **Concurrent sessions, same repo-key (you run ~10; worktrees collapse to one key):** per-session
  scratch files mean no scratch stomping; SessionEnd deletes **only its own** scratch; the merge scans
  all scratch files and branch-scopes the winner. Durable appends take an **advisory lock** on a
  dedicated `timeline.lock` (not the JSONL itself, to avoid append-pointer interaction) and
  **the append + `latest.md` regeneration happen inside the same lock hold**, so two concurrent
  SessionEnds can't race the render and lose a record. Windows: `msvcrt.locking` of a fixed byte
  region with **bounded retry/backoff (default 10 tries, ~2s total)**; POSIX: `fcntl.flock`. On
  lock-acquire failure: an **`auto` boundary is skipped** (scratch covers resume); a **`rich` record
  is never silently dropped** — the temp file is preserved and the failure printed for the agent
  (`§6.3`).
- **porcelain v2 parsing contract:** parse record prefixes `1` (changed), `2` (renamed/copied),
  `u` (unmerged — present mid-conflict), and `?` (untracked) for `dirty`; the `# branch.head` header
  gives the branch or the literal `(detached)`; `# branch.upstream`/`# branch.ab` headers are
  **absent when detached** and are **not** required (ahead/behind is not tracked). `(detached)` is
  treated as "branch not authoritative" → the filesystem probe
  (`.git/rebase-merge|rebase-apply|MERGE_HEAD|BISECT_LOG`) decides `in_progress`.
- **Detached HEAD / mid-rebase/merge/bisect:** `branch: null`, `detached: true`, `in_progress` set
  from the probe; the merge suppresses the branch banner and shows `head` + operation state (`§5`).
- **Not a git repo / git missing from PATH:** non-git cwd → `git: null`. Git binary unresolvable →
  `git_unavailable: true` (resolved once via `shutil.which`), so a broken PATH is visible in
  `cli show` instead of producing permanently empty resumes.
- **Repo-key collision (rare on one machine):** `meta.json` mismatch → v1 **refuses loudly** (writes
  a `COLLISION` marker, `cli list` flags it, the hook no-ops that write) rather than guessing. The
  dual-dir hash fallback is deferred until a real collision occurs.
- **Temp-file lifecycle:** `capture_rich.py` deletes the temp file in a `finally` on success; on the
  lock-failure path it **intentionally preserves** it for the printed retry command (`§6.3`).
- **Hooks never block/crash:** try/except, always exit 0, `dirty` capped (50), resume tails only the
  last N lines, explicit `Stop` timeout, kill-switch env var.
- **Encoding:** UTF-8-reconfigured stdout/stderr (`recall.py` precedent).
- **`session_id`:** always present in hook stdin; `null` only on the non-hook CLI path.

## 10. Testing

- **Python unit tests (pytest via UV — dev/test only)** in `tools/session-state/test/`:
  - **stdlib-only assertion:** import each shipped script and assert no third-party imports.
  - record/scratch schema round-trips; `git: null` vs `git_unavailable` handling;
    `detached`/`in_progress` shapes.
  - **porcelain-v2 parser:** fixtures for clean, dirty (`1`/`2`/`?`), detached, and conflicted-rebase
    (`u` records, absent `branch.ab`) output → correct `branch`/`detached`/`dirty`.
  - **branch-scoped merge** (`§5`): freshest-of(all scratch, timeline); same-branch fuse;
    different-branch/worktree banner; detached/mid-rebase suppression; "no rich yet"; "scratch newer
    than any timeline record"; throttle-restamp keeps `ts` ≈ change-time.
  - **concurrency:** two per-session scratch files don't stomp; SessionEnd deletes only its own;
    N parallel locked appends produce N valid lines; append+render atomic under one lock;
    lock-fail skips `auto` but **preserves `rich`** + prints retry.
  - **trigger gating:** SessionEnd skips `reason ∈ {clear, resume}`; resume emits only for
    `source ∈ {startup, resume}`, silent on `compact`/`clear`; resume output is the exact JSON
    envelope; kill-switch env var no-ops all three.
  - keying: `canonical_project_root` collapse, case-insensitive slug, **vendoring parity** vs
    `recall.py` on a path table; collision → refuse + marker.
  - retention truncation; orphan-scratch cleanup; `latest.md` render snapshot; resume writes no files;
    `capture_rich.py` temp file removed on success, preserved on lock-fail.
- **Hook integration smoke test:** feed sample stdin JSON to each script in a temp git repo; assert
  scratch write, appended line, gating, envelope output.
- **Installer:** dry-run idempotency against a **fixture** `settings.json` already containing the
  `recall.py` hook (never the real file); no duplicates on re-run; path-drifted entry replaced;
  `Stop` entry carries the timeout; `-Uninstall` removes exactly the three entries.
- **Manual acceptance:** install; run `end-session` (confirm a `rich` record is written); start a new
  session in this repo **and concurrently in one of its worktrees**; confirm each surfaces correct,
  branch-scoped state without cross-stomping; toggle the kill-switch.

## 11. Relationship to existing work

- **Replaces** the manual `CLAUDE.md` "active session pickup" pointer with an automatic, per-repo,
  timeline-backed equivalent.
- **Driven by / complements** `end-session`/`handoff` — `end-session` calls `save-state` so rich
  records are guaranteed on clean boundaries (`§4`).
- **Coexists** with the `recall.py` SessionStart hook and the `Stop`-hook cache-timer/budget tools
  (parallel, independent, bounded).
- **Vendors** (does not import) `recall.py`'s `canonical_project_root`/`path_to_slug`, with a parity
  test — no cross-tool import coupling.
- **Feeds, later:** roadmap item 4 (dashboard) and a possible SQLite/daemon upgrade can consume the
  JSONL timeline without rework.

## Design Critique Log

Three independent adversarial critique rounds were run (a fresh subagent each round, each seeing the
prior round's revised design), as required before finalizing.

### Critique Round 1

**Findings (verified against live `~/.claude/settings.json`, `recall.py`, `cache-countdown`, and the
Claude Code hooks docs):**
- `SessionEnd` does **not** fire on crash/interrupt and **does** fire on `/clear` and `--resume`, so
  it cannot be the "every session end" safety net the design relied on.
- A `matcher:"*"` `SessionStart` resume hook would re-inject after every compaction/`/clear` — fighting
  context hygiene.
- `uv run` on every session boundary adds hundreds of ms (and a cold sync can exceed the 10s timeout)
  for a stdlib-only script; the deployed `recall.py` invokes `python.exe` directly for this reason.
- "Single `write()` atomic append, no locking" is not safe for concurrent writers on Windows.
- The merge fused freshest git facts with the latest narrative regardless of branch (Frankenstein
  state); worktree path-keying fragmented a worktree's timeline from its parent; lossy `:/\`→`-` slug
  could collide; timeline growth was unbounded; installer idempotency/marker + UTF-8 + skill/CLI
  contracts were under-specified.

**Resolved:** introduced the `Stop` per-session **scratch** (crash-safe freshest facts) and split
capture into three triggers with `SessionEnd` reason-filtering; **source-gated** resume to
`{startup,resume}`; switched to **direct interpreter** invocation; added **locked appends**;
**branch-scoped** the merge; **canonicalized worktrees** to the parent (vendoring `recall.py`);
added a `meta.json` collision guard, **retention/rotation**, marker-based idempotent install, and
UTF-8 reconfigure.

### Critique Round 2

**Findings (verified against `budget-checkpoint/hook.py`, `recall.py`, the two live `Stop` hooks, and
the docs):**
- The SessionStart output channel was mis-specified and its justification false-cited.
- The "skill passes stdin JSON" mechanism **does not exist** — a skill can't pipe stdin to a script.
- `Stop` fires **every turn**, so shelling git three times per turn across ~10 sessions behind two
  existing Stop hooks is a real latency tax.
- A **single shared** `current.json` (worktrees collapse to one key) meant concurrent same-repo
  sessions **stomp** each other, and `SessionEnd` clearing it **destroyed a live sibling's state**.
- The shared-module refactor created hidden **cross-tool import coupling**; the branch banner ignored
  **detached HEAD / mid-rebase**; the collision "hash fallback" silently lost timelines; the lock
  helper's Windows/POSIX semantics differed; resume writing `latest.md` was a read-path side effect.

**Resolved:** specified the **JSON envelope** for resume; replaced the skill contract with a
**temp-file `--input`**; made `Stop` a **single throttled `git status --porcelain=v2 --branch`** call
with an explicit timeout; made the scratch **per-session** (`scratch/<session_id>.json`) with
SessionEnd deleting only its own; **vendored** the keying helpers (no cross-tool import); added
**detached/`in_progress` suppression**; made resume **read-only** (render only on append); specified
the **lock** as a dedicated `timeline.lock` with bounded retry.

### Critique Round 3

**Findings (verified against live `git status --porcelain=v2 --branch` in normal/detached/rebase
states, `recall.py`, `budget-checkpoint/hook.py`, and the docs). Verdict: implementable after the
correctness/data-loss fixes below.**
- The "plain stdout is **not** the SessionStart contract" claim is **false** — plain stdout does reach
  context for SessionStart (so `recall.py` is fine); the envelope is *optional*, and the cited
  `budget-checkpoint` authority is a **Stop** hook, not SessionStart.
- The porcelain-v2 parser was under-specified (must handle `u` unmerged records and absent
  `branch.ab` when detached; `(detached)` can't distinguish rebase → needs the FS probe).
- The throttle could let scratch `ts` lag real activity and mislead the "freshest by ts" merge.
- `capture_rich.py` temp-file deletion wasn't crash-safe; **lock-fail silently dropping a `rich`
  record** is unacceptable data loss for hand-authored narrative; `latest.md` regen vs the lock was
  ambiguous (render must be inside the lock).
- **`rich` records had no guaranteed producer** (purely opt-in → likely unused).
- Over-engineering for a single-user tool: collision hash-fallback, month-bucketed rollover, and
  ahead/behind parsing should be cut from v1. Missing: an **uninstall/kill-switch** and explicit
  **git-binary resolution**.

**Resolved:** corrected the envelope rationale (optional; plain stdout valid; re-cited the docs);
added the **porcelain-v2 parse contract**; made the throttle **re-stamp `ts` whenever facts change**;
made temp-file cleanup a **`finally`** and made `capture_rich.py` **preserve the file + print a retry
command on lock-fail (never silent loss)**; specified **append+render in one lock hold**; **wired
`end-session` to drive `save-state`** so rich records are guaranteed; **cut** the collision
hash-fallback (now refuse-loudly), month buckets, and ahead/behind (all moved to out-of-scope/§8);
added **`install.ps1 -Uninstall`**, the **`CC_SESSION_STATE_DISABLE` kill-switch**, and
**`shutil.which` git resolution** with a `git_unavailable` flag.
