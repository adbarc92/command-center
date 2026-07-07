# `docs/STATUS.md` front-matter convention (local project tracker)

Part of the "local" dashboard source (spec: `docs/superpowers/specs/2026-07-06-local-project-tracker-design.md`, §3.1/§3.3 — see note below on branch availability).

This convention lets the Project Dashboard's `local` source read a project's canonical stage
straight out of its `docs/STATUS.md`, via a small byte-0 YAML front-matter block, without needing
any additional per-project config file.

## The front-matter block

```yaml
---
stage: "Build"
readiness: "~85%, human-gated"
updated: "2026-07-06"
name: "Command Center"
blocked: "waiting on infra ticket #123"
base_branch: "main"
test_cmd: "npm test"
---
# Command Center
...
```

### Fields

| Field         | Required | Meaning                                                                 |
|---------------|----------|--------------------------------------------------------------------------|
| `stage`       | yes      | One of: `Idea \| Spec \| Plan \| Build \| Review \| Ship \| Live \| Archived \| Blocked \| Failed \| Idle` |
| `readiness`   | no       | Free-text readiness string (e.g. `"~85%, human-gated"`)                 |
| `updated`     | no       | Quoted ISO date (`"YYYY-MM-DD"`) of the last stamp                       |
| `blocked`     | no       | Free-text reason; only meaningful when `stage: "Blocked"`                |
| `name`        | no       | Display-name override for the dashboard (defaults to repo/dir name)      |
| `base_branch` | no       | Phase-2: the branch this project's PRs target                            |
| `test_cmd`    | no       | Phase-2: the command the dashboard/agents run to verify the project      |

### Rules

- The block **MUST** start at byte 0 of the file (line 1 is exactly `---`) and the file **MUST**
  be BOM-less. A leading UTF-8 BOM defeats the byte-0 fence check on Windows-written files, so any
  writer must strip it first.
- A pre-existing leading `# H1` is **not** part of the block — it is preserved and simply ends up
  *below* the inserted block once stamped.
- A `---` that does **not** appear on line 1 is **not** a front-matter fence — it's an ordinary
  Markdown horizontal rule (e.g. separating entries in a Session-log section further down the
  file) and must be left alone. Only the first `---` (line 1) and the next `---` line after it
  delimit the block.
- Only `stage`, `readiness`, and `updated` are *managed* keys — anything else already present in
  the block (`name`, `blocked`, `base_branch`, `test_cmd`, or any future key) is preserved
  untouched when the block is refreshed.

This is the inverse of the reader, `parseStatusFrontmatter` (`cockpit/ui/src/lib/dashboard/frontmatter.ts`),
which parses this same block for the dashboard's `local` source.

## How it's maintained

The reusable stamper lives in this repo at:

- `plugins/session-state/src/status_frontmatter.mjs` — exports `stampStatusFrontmatter(text, { stage, readiness, updated })`.

It is a pure string-in/string-out helper (no file I/O, no YAML dependency — the format is small
and controlled, so it's handled line-based):

- Given the current text of `docs/STATUS.md` and a `{ stage, readiness, updated }` fields object,
  it returns the new text with the front-matter block inserted (if absent) or refreshed in place
  (if already present), preserving unmanaged keys.
- `stage` is required and throws if missing; `readiness`/`updated` are optional.
- It strips a leading BOM and normalizes the block to sit at byte 0.
- It is idempotent: calling it twice with the same fields does not duplicate the block.

Callers own reading the file before calling the stamper and writing the result back out
afterward (the helper never touches disk).

**Important:** the in-repo `plugins/session-state` plugin's `capture_*.mjs` hooks write the
per-repo session *timeline* to `~/.claude/state` — they do **not** write `docs/STATUS.md`, and
this task does not add any such wiring to them. `docs/STATUS.md` is owned by the session-wrap
*skills* described below.

## Manual wiring (out-of-repo)

`docs/STATUS.md` is rewritten by the global session-wrap skills — `end-session`, `save-state`,
and `handoff` — which live in `~/.claude/skills/` (outside this repo), plus the STATUS.md
convention note in the user's global `~/.claude/CLAUDE.md`. Those skills/instructions must be
updated by hand (not by this task) so that whenever they rewrite `docs/STATUS.md`'s State
summary, they either:

1. Import and call `stampStatusFrontmatter` from `plugins/session-state/src/status_frontmatter.mjs`
   (when running in a context where that module is reachable, e.g. via the plugin), or
2. Replicate its behavior inline (byte-0 block, BOM-less, managed keys `stage`/`readiness`/`updated`,
   unmanaged keys preserved) when it is not.

This wiring is intentionally left as a manual, out-of-repo step: the skills in question are not
part of this repository, and this task's scope is limited to the in-repo, testable stamper helper
and this convention document.

**Note on the spec reference above:** the design spec this convention derives from,
`docs/superpowers/specs/2026-07-06-local-project-tracker-design.md`, was authored on a different
branch and is not present in this worktree at time of writing; the field list and rules above are
the canonical form as verified for this task, matching the reader already implemented at
`cockpit/ui/src/lib/dashboard/frontmatter.ts`.
