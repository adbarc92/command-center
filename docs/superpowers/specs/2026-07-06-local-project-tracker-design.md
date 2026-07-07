# Local Project Tracker + Roadmap Dispatch — Design Spec

> A **fifth source** for the existing Project Dashboard (roadmap item 4, spec
> [`2026-06-09-project-dashboard-design.md`](2026-06-09-project-dashboard-design.md)). It tracks
> local projects by reading each project's own `docs/STATUS.md` (→ its stage) and `ROADMAP.md`
> (→ a queue of dispatchable work items), and lets the operator fire a roadmap item at the fleet
> as a live mission — which loops back onto the board as a Fleet card.
>
> Status: **design**. Author: brainstorming session 2026-07-06. Single operator, single machine
> (parent's trust model); **one cockpit instance at a time** is assumed (§7.5).

## 1. Goal & scope

**Goal:** see the stage of every *local* project at a glance — without that project running any
tool — by reading the status/roadmap docs the operator already maintains; and turn its roadmap into
a **work queue** whose items can be dispatched to the autonomous fleet with one click.

The human-**declared**, doc-driven counterpart to the dashboard spec's deferred "bare-git/CI adapter."

**In scope:** **U1** `STATUS.md` front-matter (stage marker); **U2** `ROADMAP.md` `cc-item`
structured items; **U3** session-wrap skill changes that maintain the markers; **U4** a `local`
source adapter (Rust FS half + pure-TS parse/map half); **U5 (Phase 2)** dispatch a roadmap item as
a **real** fleet mission + reconcile item status. **U5 reopens a parent locked decision — see §0.**

**Out of scope:** roadmap **prose** editing from the board (only a `cc-item`'s `status=` field is
written, §7.3); **cross-source dedup/merge** (a project may show a `local:` *and* an `audience:`/
`fleet:`/`manual:` card — no merge this cycle; `family` clustering is §9); history/trend;
multi-machine; notifications; push/file-watch refresh (poll this cycle).

### Locked decisions (do not relitigate)
1. **Discovery is hybrid** — bounded-recursive scan of configured roots **plus** manual pin/exclude
   (§4). **Nested markers are allowed** — each dir with `docs/STATUS.md` is its own project (§4).
2. **Stage is explicit-marker-only** — from `STATUS.md` front-matter; no prose inference. A *pinned*
   unmarked dir shows a degraded card (§6.5); an *auto-discovered* unmarked dir is simply not found.
3. **Stage lives in `STATUS.md`; work items live in `ROADMAP.md`.**
4. **"Feed an agent" = dispatch a `real`-mode fleet mission** (never `demo`, §7.1) that loops back
   via the existing Fleet adapter (§7.2).
5. **One spec, phased build** — Phase 1 = tracking (U1–U4); Phase 2 = dispatch (U5).
6. **Roadmap items are structured *in* `ROADMAP.md`** — a per-item machine header, not a separate file.
7. **New `Source = 'local'`.** No cross-source dedup this cycle (a project may appear under multiple
   sources; §6.5).
8. **Operational authority = the dashboard store link-map; `ROADMAP.md` is the durable record**,
   reconciled to *live mission phase* via safe writes (§7.3).

## 0. Amendment to the parent's locked decision #4 (read first)

The parent locks the board as a **read-only projector** ("the board never mutates a source";
in-board dispatch is its largest *deferred* item). **Phase 2 (U5) reopens this**, because **any
dispatch mutates a source** (it starts a containerized agent that opens a real PR) — a doc write is
not what makes it a mutation. So:
- Phase 1 (U1–U4) **fully honors** locked #4 (pure read-only adapter).
- Phase 2 is an **explicit amendment**. **Operator decision (2026-07-06): Option A** — dispatch +
  write-back, accepting the daemon-wide loopback-auth migration it entails (§7.4). Two options:
  - **(A) Dispatch + write-back** (default) — start the mission *and* write the item's `status=`
    field back to the operator's own `ROADMAP.md` (§7.3).
  - **(B) Deep-link only (genuinely read-only)** — the board does **not** POST; the "Dispatch"
    control **deep-links into the Fleet tab with a pre-filled mission** the operator submits there.
    This preserves locked #4 verbatim (the board never calls a mutating endpoint). It loses the
    one-click loop and auto-reconcile.
  - A prior draft's "store-only, no doc-write" is **not** a read-only option — it still POSTs — so it
    is folded into (A) as a `writeBack:false` flag, not presented as preserving #4.
- **Gate:** whichever is chosen, when Phase 2 lands the parent's locked #4 and the `model.ts`
  "read-only / renders nothing else" comments are amended to cite this section. Phase 2 must not
  merge until that amendment is written.

## 2. How it slots into the existing dashboard

Parent locked #6 (*"a fifth source is one new adapter"*) holds for Phase 1, plus **two additive,
backward-compatible model fields** (§5) and **no change to `stage.ts` or any existing adapter**.
Phase 2 additionally, and **acknowledged** (correcting an earlier "zero board change" overclaim):
adds a card dispatch affordance to `views/Dashboard.svelte` (gated behind the new `dispatch` field),
and stamps `family` in the **store compose step** (not `fleet.ts`, §7.2).

## 3. Doc conventions (U1, U2) and skill support (U3)

### 3.1 U1 — `STATUS.md` front-matter

YAML beginning at **byte 0** (after any BOM, see below) of `docs/STATUS.md`:

```yaml
---
stage: Build              # REQUIRED · canonical value (below)
readiness: "85%"          # optional · quoted string → card detail
updated: "2026-07-06"     # optional · QUOTED ISO date (unquoted → YAML Date) → else file mtime
blocked: "billing gate"   # optional · only when stage: Blocked
name: "Command Center"    # optional · display override; else H1; else dir basename
base_branch: "main"       # optional · dispatch target branch (§7.4); else resolved at dispatch time
test_cmd: "cargo test"    # optional · dispatch verify command (§7.4); REQUIRED for real dispatch
---
```

`stage` ∈ the canonical `model.ts` values (`Idea|Spec|Plan|Build|Review|Ship|Live|Archived` or
`Blocked|Failed|Idle`). No new stage vocabulary.

**Parsing (hardened):**
- Parser: **`js-yaml`** (new stated cockpit dep). **Strip a leading UTF-8 BOM** (`EF BB BF`) before
  the byte-0 check — Windows/PowerShell writers emit BOMs; a BOM would silently unmark the project.
- Front-matter exists only if (post-BOM) **line 1 is exactly `---`** (`^---\r?$`, CRLF-tolerant).
  The **closing fence** is the next `^---\r?$`. Because `STATUS.md` Session logs use `---` horizontal
  rules, a `---` is a fence **only** when the opening one is line 1; otherwise there is no
  front-matter. Not line 1 → unmarked.
- `stage` case-insensitive→normalized; `updated`/`readiness` coerced to strings.
- No `stage` → **unmarked** (§6.5). Non-canonical `stage` → **degraded card**
  (`health:'unknown'`, `detail:"invalid stage: <x>"`).

### 3.2 U2 — `ROADMAP.md` `cc-item` structured items

A dispatchable item = a **markdown heading** immediately followed by a machine-header HTML comment
(invisible when rendered) + prose + optional `**Dispatch:**` brief:

```markdown
## Cache-aware approval timer
<!-- cc-item id=cache-timer status=done tier=t2 lane=workflow -->
A live countdown of the prompt-cache TTL…

**Dispatch:** Implement the Stop + UserPromptSubmit hooks… Acceptance: alerts at 60/30/10s.
```

**Grammar:** `<!-- cc-item KEY=VALUE … -->`, lowercase keys, **space-separated bare-token values**
(no spaces in values), unknown keys ignored-with-warning.

| Field | Values | Meaning |
|---|---|---|
| `id` | slug, **unique per roadmap (hard error if duplicated, §7.3)** | link key — required to dispatch |
| `status` | `open`·`active`·`blocked`·`done` | `open`=dispatchable; others set by U5 |
| `tier` | `t1`\|`t2`\|`t3` (default `t1`) | mission tier |
| `lane` | slug (optional) | swarm-carving hint (§9) |

**Parsing (MUST be structural, not regex-over-raw-text):** use a markdown tokenizer so **fenced and
indented code blocks are skipped** — a `cc-item` comment inside a code fence (including the examples
in *this spec* and in a ROADMAP's own docs) is **not** an item (a phantom `open` item would be
one-click dispatchable, §7.4). An item = an HTML-comment node whose immediately-preceding block is a
heading (blank lines allowed). Tagged item missing `id` → shown **non-dispatchable** + warning.
Dispatch-task resolution: `**Dispatch:**` block → item prose → heading title.

### 3.3 U3 — skill support
`end-session`/`save-state`/`handoff` (which already rewrite `STATUS.md`) insert/refresh the U1
front-matter at byte 0 (**BOM-less**), relocating a prior H1 below it. `ROADMAP` `cc-item`s are a
documented convention enforced by the validator; Phase 2 writes `status` via the safe writer (§7.3).
The `~/.claude/CLAUDE.md` `STATUS.md` convention note gains the front-matter line.

## 4. Discovery (part of U4)

```ts
interface LocalTrackerConfig {
  scanRoots: string[];   // parent dirs to search
  maxDepth: number;      // default 5 — the real tree reaches D:/MajorProjects/CURRENT/<repo>/services/<svc>
  pins: string[];        // explicit project dirs (tracked even if unmarked, §6.5)
  excludes: string[];    // path globs (normalized forward-slash, case-insensitive on Windows)
}
```

Rust half: **bounded-recursive** walk to `maxDepth`, **pruning** `.git`/`node_modules`/`target`/
`dist` and excluded globs, **not following symlinks/junctions** (avoids OneDrive/junction cycles).
Every directory containing `docs/STATUS.md` is a project — **including nested ones** (a monorepo root
*and* its `services/<svc>` may each be a project; the operator's real tree has depth-4 service
projects, so nesting is a primary case, not an edge one). Add `pins`; drop `excludes`.

## 5. Data model additions

Two **additive** changes to `model.ts`; **no `stage.ts` change, no `applyOverride` call for local
cards** (this is the key Round-2 correction):

1. `Source` gains `'local'` (+ `SOURCE_LABEL['local']='LOCAL'`).
2. `ProjectCard` gains optional `dispatch`:

```ts
export interface RoadmapItem {
  id: string; title: string;
  status: 'open'|'active'|'blocked'|'done';   // DISPLAY = live phase if linked (§7.3), else doc
  tier: 't1'|'t2'|'t3'; lane?: string; task: string;
  missionId?: string; dispatchable: boolean;   // Phase-2 fields, inert in Phase 1
}
// added: dispatch?: { items: RoadmapItem[] };
```

**Stage identity (corrects the broken "synthesized override"):** verified against `stage.ts` —
`applyOverride` treats `ttlHours: undefined` as the **default 72h expiry** (`stage.ts:34`), so a
synthesized override would make any marker older than 72h *expire and flip stage*. Therefore the
`local` adapter does **not** synthesize an override and does **not** call `applyOverride`. It emits a
**fully-resolved card** the board renders verbatim (exactly as `fleetCard` already returns a built
card): `stage = markerStage`, `stageSource = 'declared'`, `override = null`, `conflict = null`. The
`DECLARED` chip renders correctly on `stageSource === 'declared'` with a null-safe
`title={c.override?.reason}` (verified `Dashboard.svelte:183-184`). **Anti-rot** (replacing the
parent's TTL for this source): the card footer (the existing `.cfoot` row, beside the health flag)
surfaces the `updated` age as a subtle "declared Nd ago" hint, so a forgotten marker is *visible* as
stale without silently changing stage. (This + the `SOURCE_LABEL['local']` entry are the only
Phase-1 board touches — §2's "one adapter" is otherwise accurate.)

## 6. The `local` adapter (U4)

**Two halves:** **Rust** `#[tauri::command] scan_local_projects(config) -> Vec<LocalProjectDoc>`
where `LocalProjectDoc = { projectDir, name?, statusText, roadmapText?, roadmapHash?, statusMtime, roadmapMtime }`
(`roadmapHash` = SHA-256 over the raw `ROADMAP.md` bytes, feeding the write-back CAS, §7.3) —
discovery + raw reads only, **no markdown parsing and no `git` subprocess** (git remote/branch
resolution is deferred to dispatch time, §7.4, so Phase 1 never shells out per poll). **TS** half
parses U1+U2 (front-matter via `js-yaml`, `cc-item`s via the **`marked`** tokenizer so code fences
are skipped, §3.2) and emits fully-resolved `ProjectCard[]`; pure, unit-tested.

**Card mapping:** `projectId="local:"+slug(projectDir)`; `source:'local'`; stage identity per §5;
`name = frontmatter.name ?? H1 ?? basename`; `detail = readiness ?? "<N> tagged · <M> open"`;
`blocked` iff `Blocked`; `updatedIso = frontmatter.updated ?? statusMtime`; `staleAfterSec ≈ 2×poll`
(greys only if polling stops, not on doc age); `health` ok/unknown/degraded; `dispatch = { items }`.

**Refresh:** poll ~30s. The Rust half tracks last-seen mtimes **in process memory** and returns an
"unchanged" marker to skip re-reads; because the tracking is in-memory, a cockpit restart forces a
full first read (the TS parse cache is never cold-but-skipped).

### 6.5 Unmarked pins; no cross-source dedup
A **pinned** unmarked dir → degraded "unmarked" card (honors the explicit pin). An auto-discovered
unmarked dir just isn't found (locked #2). **No cross-source dedup this cycle** — a project may
render as both `local:` and `manual:`/`audience:`/`fleet:` cards. (A prior "same-path collapse" rule
was dropped: `ProjectCard` carries no canonical path and a `manual` card may have none, so the rule
keyed on data the model doesn't expose. Dedup via a shared canonical-path key is §9.)

## 7. Dispatch, loopback & reconciliation (U5 — Phase 2) · amends parent #4 (§0)

**Affordance (option A):** a `local:` card lists `dispatch.items`; each `open` **dispatchable** item
shows **"▸ Dispatch (<tier>)"** → resolve `task`+`tier` → `POST /missions` → `unit_id`.

### 7.1 Real mode only; correct preconditions
Dispatch **always sends `mode:'real'`** — `fleetd`'s default `demo` runs a `FakeRunner` that marches
phases without touching a repo, which would write a **false `done`** into the real `ROADMAP.md`.
**Verified precondition facts:** `create_mission`'s real branch checks **only `ANTHROPIC_API_KEY`
(`server.rs:272`)** — it does **not** check docker (only `create_swarm` does, `:751`). So the
dispatch button gates on the cockpit's **`GET /health`** (`docker` + `anthropic_key`, `server.rs:589`)
before enabling, with the reason shown when unmet. **Recommended fleetd fix:** add the `docker_ok`
check to `create_mission`'s real branch to match `create_swarm`, so a docker-down dispatch fails at
the gate rather than mid-driver (which would feed a spurious `failed → open`).

### 7.2 Loopback & family
The mission emits `phase_changed`/`/units` → the **existing Fleet adapter renders it** (unchanged).
The **store compose step** stamps `family = <local projectId>` on a Fleet card whose `unit_id` is in
the link-map — no existing adapter modified.

### 7.3 Authority, link-map lifecycle & safe write-back
**Authority:** the **store link-map** is operational truth; **`ROADMAP.md` is the durable record**;
on disagreement **live mission phase wins** for display, then the doc is reconciled.

**Link-map** — persisted in the dashboard JSON store (**versioned**, with a `schemaVersion` +
forward-compatible read; unknown → ignore):
`"<projectId>#<itemId>" → { missionId, dispatchedAtIso, lastWrittenStatus }`.

**Dispatch idempotency:** dispatch is refused (button disabled) for an item already `active`/linked;
`fleetd` additionally accepts an **idempotency key = `<projectId>#<itemId>`** and refuses a second
live mission for the same key — guarding the multi-instance/double-click race even though single
cockpit is assumed (§7.5).

**Boot reconcile sweep:** on start, for each live link, read the mission's phase from `/units`; if
terminal, perform the possibly-missed write-back. Orphan cases → revert item to `open` + warn:
(a) `missionId` absent from `/units` (pruned); (b) **`projectDir` no longer exists** (project moved/
renamed — `projectId` is path-derived, so a rename orphans the link).

**Transitions** (real phase → item status) — **complete mapping** (Round-3 fix: gate phases now
produce the `blocked` value, which was otherwise unreachable):
`dispatch → active`; `building`/`checking`/`reviewing`/`merge_check → active`;
`awaiting_oracle_approval`/`needs_human`/`halted → blocked` (also written to the doc, so a re-scan or
another session sees the item is gated); `pr_open`/`done → done`; `failed → open` (retryable).

**Safe write-back** — `#[tauri::command] set_roadmap_item_status(projectDir, itemId, expectHash, newStatus)`.
`expectHash` byte-domain (Round-3 fix): `scan_local_projects` returns a **`roadmapHash`** computed
**over the raw file bytes (pre-decode, including any BOM/CRLF)** with a pinned algorithm (SHA-256);
the TS half echoes that exact value as `expectHash`; the Rust writer re-reads **raw bytes** and
hashes with the same algorithm. Both sides hash identical byte representations — never a decoded JS
string — so the CAS neither spuriously matches nor perpetually aborts.
Windows-correct protocol (Round-2 fix — no incoherent lock-then-replace):
1. **Single-writer serialization inside fleetd** (an in-process async mutex) — fleetd is the only
   *automated* writer, so this fully orders its own writes without OS file locks (which are
   mandatory-not-advisory on Windows and which human editors/skills don't take anyway).
2. **Hash compare-and-swap:** re-read the file immediately; if its hash ≠ `expectHash` (the parse the
   write is based on), **abort + warn** — concurrent human/skill edits are **detected and never
   clobbered** (they cannot be *prevented*, and the spec says so plainly).
3. **Structural targeted rewrite** of only the matching `id`'s `status=` token. **Duplicate id →
   hard error** (refuse). **id not found** (renamed) → **no silent no-op**: orphan the link + surface
   "item <id> gone from ROADMAP" on the card.
4. **Atomic replace with no open handle on the target:** write a temp file in the same dir, then
   `ReplaceFileW`/rename (holding no handle on `ROADMAP.md`, avoiding `ERROR_SHARING_VIOLATION`).
5. Poll debounce: the scan skips a project with an in-flight write; mtime-gating + atomic replace
   prevent torn reads.

### 7.4 Repo-targeting & the daemon security model (Round-2 corrected)
**Corrected premise:** the blast radius this feature needs does **not** start closed. `POST /swarms`
**already** accepts a caller-supplied real `repo_url` with no allowlist (`server.rs:731,764`), on an
**unauthenticated `127.0.0.1:8787`** daemon whose `GhForge` uses the operator's **ambient `gh`/git
credentials = full push to every repo they can push to** (`server.rs:237`). So the exposure pre-exists;
this feature is the first to *drive it from the UI with caller-chosen repos*, so it introduces the
controls that should cover the **whole** daemon, not just `/missions`:
1. **Loopback auth as router middleware over every mutating route** (`/missions`, `/swarms`,
   `/units/:id/commands`). Token provisioning avoids the bootstrap-circularity Round 2 flagged:
   **`fleetd` is a sidecar the Tauri host spawns** (`app.shell().sidecar("fleetd-serve")`, `sidecar.rs`),
   so the host **generates the token and passes it to `fleetd` at spawn via env** and holds it for
   the webview — no registration over the very socket being protected.

   **Auth cutover (Round-3 blocker — this is a daemon-wide client migration, not just a new button):**
   - The supervisor generates the token **once per app launch and reuses it across sidecar
     crash-respawns** (`sidecar.rs` respawns in a loop) — a per-spawn token would strand the webview
     after any fleetd restart. Persist it in supervisor memory for the process lifetime.
   - The webview obtains it via a **new Tauri command `get_fleet_token`** (today `api.ts` fetches
     `127.0.0.1:8787` directly, not over IPC), read once at startup and sent as an `Authorization`
     header.
   - **All existing callers retrofit:** `createMission`, `sendCommand` (Fleet unit Resume/Abandon),
     `createSwarm`, and `openStream` must attach the token, or they 401 the moment the middleware is
     on. This is in Phase-2 scope and gated by a **regression test that the existing Fleet + swarm
     UI still work with auth enabled** (not just "missing token refused").
   - If the operator prefers to defer this whole migration, **option B (deep-link, §0)** ships
     tracking + one-click prefill with the middleware unbuilt.
2. **Server-side repo allowlist:** `fleetd` accepts caller repo fields only for repos on an allowlist
   it derives from the registered project set (discovered/pinned projects, sent over the now-authed
   channel). Non-allowlisted repo → refused. Omitted repo fields → sandbox default (unchanged).
3. **Per-project `test_cmd` + `base_branch` required for real dispatch** — sourced from U1
   front-matter (`test_cmd`/`base_branch`, §3.1) or resolved (`git` default branch) at dispatch time;
   **underivable `test_cmd` → dispatch refused** (a Rust repo verified with the current hardcoded
   `node --test`, `server.rs:262`, would make the oracle gate meaningless and feed a false result).
4. **Human gates preserved:** `awaiting_oracle_approval`/`needs_human` still surface as Blocked Fleet
   cards; dispatch bypasses no gate.

Scope note: the loopback-auth middleware is a **daemon-wide** improvement this feature necessitates;
if the operator prefers to keep it minimal, option **B (deep-link, §0)** ships tracking + one-click
prefill without `fleetd` ever accepting a board-supplied repo — deferring the middleware.

### 7.5 Single-instance assumption
One cockpit at a time (consistent with the parent's single-operator/single-machine model). The
idempotency key (§7.3) is the backstop if two instances ever run; a hardened multi-instance story is §9.

## 8. Build phasing & testing

**Phase 1 (U1–U4)** — no fleet dependency:
- Rust `scan_local_projects`: bounded-recursive/pruned/no-symlink discovery incl. **nested markers**;
  in-memory mtime-gate; **no git subprocess**. Tests: nested/monorepo tree, excluded globs, unreadable
  dir, restart forces full read.
- TS parse+adapter tests: valid/missing/invalid marker; **BOM-prefixed file still parses**; CRLF
  fences; `---` session-rule not a fence; unquoted-`updated` coercion; **cc-item in a code fence
  ignored**; missing/duplicate id; Dispatch/prose/title resolution.
- **Stage identity test:** local card renders `DECLARED` chip with `override:null` and does **not**
  expire after 72h (no `applyOverride`); "declared Nd ago" hint present.
- Board: `SOURCE_LABEL` + additive fields render.
- U3: skills stamp BOM-less front-matter (relocating a prior H1).

**Phase 2 (U5)** — requires the §0 amendment first:
- fleetd: `CreateReq` repo/branch/test fields; **loopback-auth middleware over `/missions`+`/swarms`+
  `/units/:id/commands`**; **server-side allowlist**; idempotency key; (recommended) docker check on
  `create_mission`. Tests: non-allowlisted repo refused; missing token refused; duplicate-key refused.
- `set_roadmap_item_status`: in-proc serialization + hash-CAS + atomic replace; tests for
  concurrent-edit abort, duplicate-id refusal, id-not-found orphan, formatting preserved, crash-safe.
- Store link-map (versioned) + boot reconcile sweep (completion-while-closed reconciles;
  missing-mission and **moved-projectDir** both orphan→open); compose-step `family`; real-mode-only;
  precondition-gated button; `failed→open`.

## 9. Roadmap (named, not built)
fs-watch push; `test_cmd`-less inference; **shared canonical-path key for cross-source dedup**;
real `family` clustering; **swarm dispatch** of several `open` items (uses `lane`); bare-git/CI
inference for unmarked projects; hardened multi-cockpit story; a first-class ROADMAP-authoring skill.

## 10. References
- Parent spec: [`2026-06-09-project-dashboard-design.md`](2026-06-09-project-dashboard-design.md).
- [`model.ts`](../../../cockpit/ui/src/lib/dashboard/model.ts),
  [`stage.ts`](../../../cockpit/ui/src/lib/dashboard/stage.ts) (`ttlHours` default 72h `:34`;
  `applyOverride` `:58`), [`Dashboard.svelte`](../../../cockpit/ui/src/views/Dashboard.svelte)
  (`:183` chip), [`adapters/fleet.ts`](../../../cockpit/ui/src/lib/dashboard/adapters/fleet.ts),
  [`api.ts`](../../../cockpit/ui/src/lib/api.ts),
  [`crates/fleetd/src/server.rs`](../../../crates/fleetd/src/server.rs) (`:128` CreateReq, `:258`
  hardcoded repo, `:262` `test_cmd`, `:272` mission real-gate = key-only, `:589` health, `:731/:764`
  swarm arbitrary repo, `:751` swarm docker gate).
- Conventions/north-star: `~/.claude/CLAUDE.md`, `docs/ROADMAP.md` item 4, `docs/command-center-vision.md`.

## Design Critique Log

Three independent adversarial critique rounds, each on the prior round's revision (per the repo's
design-critique gate). Findings verified against the real code where they asserted code behavior.

### Critique Round 1
Found three CRITICAL issues and a dozen robustness gaps. **(F1)** Dispatch defaulted to fleetd's
`demo` mode → a *fake* run would write `status=done` into the real `ROADMAP.md`; **resolved** by
pinning `real` mode and gating write-back on a real terminal phase (§7.1). **(F2)** Write-back
silently overrode the parent's locked "read-only projector" decision; **resolved** by making it an
explicit, scoped amendment (§0). **(F3)** Threading a caller-supplied repo into an unauthenticated
localhost daemon was a security regression; **resolved** with a server-side allowlist + loopback auth
(§7.4). Robustness fixes: structural markdown parsing so a `cc-item` in a code fence isn't dispatched
(F5); safe write-back with concurrency/atomicity (F4); a boot reconcile sweep for completion missed
while closed (F6); per-project `test_cmd` (F7); bounded-recursive discovery matching the real tree
(F8); YAML CRLF/`---`-collision/`updated`-as-Date hardening (F10); honest acknowledgement of the
board + fleet-adapter deltas (F11); duplicate/renamed-id handling (F12); monorepo/symlink/mtime (F14).

### Critique Round 2
Verified claims against the code and caught two **factual errors** the Round-1 revision introduced.
**(#1)** `stage.ts:34` treats `ttlHours: undefined` as the **default 72h expiry**, so the
"synthesized override" would make any marker >72h old expire and flip stage — and "fixing" it would
require a non-additive `stage.ts` change; **resolved** by dropping the synthesized override entirely:
the adapter emits a fully-resolved card (`stageSource:'declared'`, `override:null`), the DECLARED chip
renders null-safe on `stageSource` (verified `Dashboard.svelte:183`), and anti-rot moves to a
"declared Nd ago" hint (§5). **(#2)** The security premise was wrong — `POST /swarms` **already**
accepts an arbitrary real `repo_url` with no allowlist (`server.rs:731,764`); **resolved** by making
loopback-auth router-level over *all* mutating endpoints and correcting the premise (§7.4). Also:
`test_cmd` had no schema source (added to U1, §3.1); Windows lock-then-`ReplaceFileW` was incoherent
(**resolved** with in-process serialization + hash-CAS + handle-free atomic replace, §7.3); BOM
defeats the byte-0 front-matter check (BOM strip, §3.1); `maxDepth` missed depth-4 service projects
and "stop at first marker" shadowed monorepos (**resolved**: nested markers allowed, default depth 5,
§4); deferred git resolution out of the Phase-1 poll (§6); dropped the unworkable same-path collapse
(§6.5); corrected the false docker-gate claim on `create_mission` (§7.1); versioned the store link-map
and added moved-`projectDir` orphan handling (§7.3).

### Critique Round 3
Verified all Round-2 fixes hold — the stage-identity rewrite is internally coherent (no consumer
re-runs `applyOverride` or assumes `override!=null`), and **Phase 1 is implementable as written**.
Found **three true blockers, all Phase-2, all additive.** **(1)** The auth cutover would 401 every
existing caller (`createMission`/`sendCommand`/`createSwarm`/`openStream`); **resolved** by specifying
one-token-per-launch reuse across sidecar respawns, a `get_fleet_token` IPC command, the client
retrofit, and an auth-on regression test (§7.4). **(2)** The write-back CAS had an unspecified
byte-domain (TS decoded string vs Rust raw bytes) that would break it; **resolved** by returning a
raw-bytes `roadmapHash` from the scan that both sides compare (§6, §7.3). **(3)** The `blocked` item
status was defined but unreachable (gate phases mapped to nothing); **resolved** by completing the
phase→status table so `awaiting_oracle_approval`/`needs_human`/`halted → blocked` (§7.3). Minor polish
folded in: named the markdown tokenizer (`marked`, §3.2/§6), located the anti-rot hint on the card
footer (§5), and noted that an aborted CAS leaves `ROADMAP.md` lagging the link-map (authoritative for
display) until the next terminal transition or boot sweep. **Verdict: Phase 1 ready to plan; Phase 2
ready once these three additive fixes (now incorporated) are in.**
