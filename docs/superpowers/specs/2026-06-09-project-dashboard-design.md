# Central Project Manager — Project-Dashboard Design Spec

> Status: design approved (brainstorming complete, 3 adversarial critique rounds passed),
> pre-implementation. **Design only — this spec builds no code.**
> Date: 2026-06-09. Branch context: `feat/dashboard-spec`.
> Roadmap item: **4 — Central Project Manager** (`docs/ROADMAP.md` §4). Lane: `product`.
> Parent vision: `docs/command-center-vision.md` (the autonomous fleet whose output this surfaces).
> Serves the North Star pillar **"ship autonomously & fast"**: visibility into the autonomous
> fleet's output.

## 1. Goal & scope

Tell, **at a glance, what stage every project is in** — one board that aggregates the state of
every project the operator runs through the Command Center, from a Halyard release to an Audience
post run to a fleet build unit, into a single **stage** per project. The operator opens the
Command Center and immediately sees: which projects are moving, which are blocked on a human gate,
which have failed, and which are idle — without opening three separate tools.

**In scope (this spec — design only):**
- A **canonical project-stage model**: a fixed, cross-tool pipeline (`§3`) every project maps onto,
  so heterogeneous tools render as one comparable lane.
- A **signal → stage derivation** contract: how raw signals from each source map to a canonical
  stage, and the **inferred-with-declared-override** rule that governs it (`§4`).
- A **data model** (`§5`): the `ProjectCard` the dashboard renders and the `StageSignal` events
  that feed it.
- **Four signal sources** wired to that model (`§6`): Halyard, Audience, the fleet mission engine,
  and the app-plugin lifecycle.
- The **Halyard-head decision** (`§7`): whether item 4 justifies graduating Halyard from headless,
  resolved with a recommendation and the seam that keeps the dashboard correct either way.
- The dashboard's **read posture** and refresh model (`§8`): how it reads each source without
  becoming a competing writer.

**Out of scope (named so we don't build them — see `§9` roadmap):**
- **Any code.** This lane writes a spec; the build is roadmap item **4-build**, gated on this spec.
- **Writing** to any source from the dashboard (no approve/flip/dispatch *from* the board this
  cycle — those stay in each tool's own surface; the board is read-only and **links out**).
- A **history/analytics/burndown** view (the board shows *current* stage, not trend lines).
- **Multi-machine / hosted aggregation** (single operator, single machine this cycle).
- **Per-project notifications / alerting** (the board is pull-on-open + poll, not push).

### Locked decisions (do not relitigate)
1. **A stage is a fixed, canonical pipeline** — not per-project free-text. (`§3`; open question #1
   resolved.)
2. **Stage is inferred by default, declarable as an override** — derived from each source's own
   state, with an optional explicit per-project override that wins. (`§4`; open question #2 resolved.)
3. **Four signal sources**, each with a defined adapter that maps its native vocabulary to the
   canonical stage. No source's internal enum leaks into the dashboard. (`§6`.)
4. **The dashboard is a read-only projector.** It owns no source-of-truth state; every source
   remains authoritative for its own projects. The board never mutates a source. (`§8`.)
5. **Halyard is the natural project/work-state backend** but is **headless today**; this spec
   recommends **graduating Halyard with a read-only head + a small read API**, and defines a
   **CLI-spawn fallback** so the dashboard ships even if the head slips. (`§7`; open question #3
   resolved.)
6. **`ProjectCard` is the single render contract.** Every source adapter produces `ProjectCard`s;
   the board renders nothing else. Adding a fifth source later means writing one adapter, no board
   change. (`§5`.)

## 2. The three open questions — resolved up front

The roadmap (`§4`) and lane brief pose three questions this spec must resolve with no remaining
placeholders. Each is answered here in one line and developed in the cited section.

| # | Question | Resolution | Section |
|---|----------|------------|---------|
| Q1 | What **is** a "stage" — fixed pipeline or per-project? | A **fixed canonical pipeline** (`Idea → Spec → Plan → Build → Review → Ship → Live → Archived`) plus three **off-pipeline** states (`Blocked`, `Failed`, `Idle`). Per-project free-text was **cut** — it would make projects incomparable at a glance, defeating the goal. | §3 |
| Q2 | Is stage **inferred** (git/commits/CI/state) or **declared**? | **Inferred by default, declared as an override.** Each adapter derives a stage from its source's own machine-readable state; an optional explicit `stageOverride` (set by the operator or by a tool that *knows* better) wins when present and unexpired. | §4 |
| Q3 | How do Halyard & Audience feed it — and does this justify a **Halyard head**? | Via **per-source adapters** producing `ProjectCard`s. Halyard feeds best through a **thin read-only head + read API** — and **yes, item 4 is the concrete reason to graduate Halyard from headless** (it was "deferred" only for lack of a consumer; the dashboard is that consumer). A **CLI-spawn adapter** is the fallback that de-risks the head's timeline. | §6, §7 |

## 3. What a "stage" is (Q1 resolved)

**A stage is a position on one fixed, canonical pipeline that every project maps onto** — *not*
per-project free-text. The pipeline is the lowest common denominator across the tools the Command
Center runs, chosen so a Halyard release, an Audience post, and a fleet build unit are all
legible side by side.

**Canonical pipeline (the happy path), in order:**

```
Idea → Spec → Plan → Build → Review → Ship → Live → Archived
```

**Off-pipeline states (a project is in exactly one of these *instead of* a pipeline stage):**

```
Blocked   — waiting on a human gate (approval/flip/oracle-approval). The most operationally
            important state: "a human must act."
Failed    — terminal failure (dead release, failed build unit, errored plugin).
Idle      — known to a source but not currently advancing (no active run; e.g. a registered
            project with no open work).
```

**Why fixed, not per-project (the rejected alternative).** Per-project pipelines were considered
and **cut**: if every project defines its own stages, the board can't answer "what stage is
everything in?" at a glance — the whole point. A fixed pipeline makes a column-or-color
comparison meaningful across tools. The cost — some source states don't map perfectly — is paid
once, in the adapter's mapping table (`§6`), not by the operator.

**Stage is a coarse rollup, by design.** The canonical stage is deliberately coarser than any
source's native enum (Halyard has 10 release states; the fleet has ~15 mission phases). The board
shows the **stage**; the `ProjectCard` also carries a **`detail` string** (the native sub-state,
e.g. `in_review` or `CHECKING`) for the operator who wants the next level down without opening the
tool. Coarse stage = glanceability; `detail` = one-click-less depth. (See `§5`.)

**Stage ordering & precedence.** Stages have a fixed ordinal (`Idea`=0 … `Archived`=7) used only
for sort/display. The **off-pipeline states are not on the ordinal axis**; when a source's state
maps to both a pipeline position *and* an off-pipeline condition (e.g. a release that is `in_review`
*and* awaiting human approval), **off-pipeline wins** for the headline stage — `Blocked` is more
operationally urgent than `Review`. The pipeline position is preserved in `detail` so nothing is
lost. This precedence rule (`Failed` > `Blocked` > pipeline stage > `Idle`) is defined once here
and applied uniformly by every adapter.

## 4. Inferred vs. declared (Q2 resolved)

**Stage is inferred by default and declarable as an override** — "inferred-with-declared-override."

- **Inferred (the default, and the common case).** Each source already maintains a machine-readable
  state that *is* the project's real position: Halyard's release-state enum, Audience's post status
  + backend health, the fleet's mission phase, the app-plugin lifecycle state. The adapter reads
  that state and maps it to a canonical stage (`§6`). **No human keeps the board up to date** — it
  reflects ground truth from each tool. This is non-negotiable: a dashboard the operator must hand-
  maintain would rot, and a rotting board is worse than no board (it lies).
- **Declared (the override, for the gaps inference can't see).** Inference is blind to two things:
  (a) **projects no tool tracks yet** — an idea in someone's head, a repo with commits but no
  Halyard launch / fleet unit; and (b) **operator knowledge that contradicts the signal** — "this
  release reads `live` but we're actually rolling it back manually." For these, a `ProjectCard` may
  carry an explicit **`stageOverride`** that **wins over the inferred stage when present and not
  expired**.

**Override mechanics (kept honest):**
- An override is `{ stage, reason, setBy, setAtIso, ttlHours? }`. It is **stamped, not silent** —
  the board badges an overridden card ("declared") so the operator knows it isn't live-inferred.
- Overrides **expire** (`ttlHours`, default 72h). An expired override is dropped and the card
  reverts to inferred — this is the anti-rot guard: a stale manual stage can't outlive its truth.
- **Inference still runs under an override.** If the inferred stage and a live override **diverge**
  (the tool moved on past what the human declared), the card surfaces a **"declared vs. inferred"
  conflict chip** rather than silently hiding the drift — so a forgotten override is visible, not
  invisible.
- Where overrides live: in a small **dashboard-owned store** (`§5`/`§8`), *not* written back into
  any source. The dashboard never mutates Halyard/Audience/fleet state; an override is the board's
  own annotation layer.

**The "inferred from what" table** — each source's inference input is concrete, never "git
commits" hand-waving:

| Source | Inferred from | Not inferred from |
|--------|---------------|-------------------|
| Halyard | the release-state enum + open proposal queue (machine state it already maintains) | raw git log |
| Audience | post status (`draft/generating/approval/published`) + backend health probe | scraping the web UI |
| Fleet | the mission-phase state machine (`phase_changed` events / unit status) | re-deriving from CI logs |
| App-plugins | the `plugin://state` lifecycle enum | process-list guessing |

Raw **git/CI** is explicitly **not** a primary signal source this cycle: every tool already
projects git/CI into a clean state enum, and re-deriving stage from commits would duplicate (and
fight) that logic. A future "bare git repo with no tool" adapter is roadmap (`§9`).

## 5. Data model

Two types. The board renders `ProjectCard`s; adapters emit them by consuming `StageSignal`s.

### 5.1 `ProjectCard` — the single render contract

```jsonc
{
  "projectId": "halyard:aurora:rel_2026-06-08-01", // globally unique: "<source>:<...native id>"
  "source":    "halyard",            // "halyard" | "audience" | "fleet" | "app-plugin" | "manual"
  "name":      "Aurora 4.2 release", // operator-facing label
  "stage":     "Blocked",            // canonical stage (§3) — the headline
  "detail":    "in_review · 2 proposals awaiting approval", // native sub-state, one level down
  "blocked":   {                     // present iff stage == "Blocked"
    "gate":   "approval",            // "approval" | "flip" | "oracle-approval" | "manual"
    "action": "Approve 2 social_post proposals",
    "deepLink": "halyard://queue"    // where the operator acts (opens the owning tool, §8)
  },
  "stageSource": "inferred",         // "inferred" | "declared"
  "override":  null,                 // the §4 override object when stageSource == "declared"
  "conflict":  null,                 // §4 "declared vs inferred" chip payload when they diverge
  "updatedIso":"2026-06-09T14:03:00Z",// when this card was last refreshed from its source
  "staleAfterSec": 60,               // adapter's freshness budget; board greys the card past it
  "health":    "ok",                 // "ok" | "degraded" | "unknown" — source/probe liveness
  "url":       "http://localhost:3000" // optional: launch/open target (e.g. an app-plugin head)
}
```

- `projectId` is **source-prefixed** so two tools can't collide and the board can route a deep-link
  / refresh back to the right adapter.
- `stage` + `detail` is the **glance/depth split** from `§3`: stage for the at-a-glance read,
  `detail` for one-level-down without opening the tool.
- `blocked` is structured (not a free string) so the board can render a consistent **"act here"**
  affordance and count "how many projects need me" — the dashboard's single most valuable number.
- `health` separates **"the project's stage"** from **"can I currently trust this card"**: a
  source that's down yields `health: "unknown"` and a greyed card, never a *wrong* stage.

### 5.2 `StageSignal` — what adapters consume

A normalized event/poll-result an adapter turns into (or updates) a `ProjectCard`. Sources emit
their own native shapes; the adapter is the only thing that understands them.

```jsonc
{
  "projectId": "fleet:unit_8471",
  "source":    "fleet",
  "nativeState": "CHECKING",         // the source's own enum value (opaque to the board)
  "nativeDetail": { /* source-specific blob: proposal count, probe status, iteration n, … */ },
  "observedIso": "2026-06-09T14:03:00Z",
  "isHumanGate": true,               // adapter's read of "this native state is a human gate"
  "isTerminalFailure": false
}
```

The adapter applies the `§3` precedence rule (`Failed` > `Blocked` > pipeline > `Idle`) and the
`§6` mapping table to turn a `StageSignal` into a `ProjectCard.stage` + `detail` + `blocked`.

### 5.3 Dashboard-owned store (small, the only state the board writes)

The board persists exactly two things, nowhere near any source's state:
1. **Override store** — the `§4` `stageOverride`s, keyed by `projectId`, with TTLs.
2. **Manual-project registry** — operator-declared projects no tool tracks yet (`source: "manual"`,
   stage is fully declared). This is how an "Idea"-stage project with no Halyard launch / fleet
   unit appears on the board at all.

Both live in a single small JSON file under `~/.command-center/dashboard/` (git-ignorable,
machine-local). It is the board's annotation layer — **never** a second source of truth for
project state.

## 6. Signal sources & adapters (Q3 part 1)

Four sources this cycle, each a self-contained adapter implementing one interface: *given my
source, produce the current set of `ProjectCard`s.* The board composes their outputs. No source's
enum appears anywhere but inside its own adapter's mapping table.

### 6.1 Halyard adapter — release/launch state

**Native vocabulary** (from the Halyard digest): release-state enum
`tagged → built → tested → uploaded → in_review → shipped_dark → live → rolled_back`, plus `dead`
and `rejected`; an **approval queue** of proposals; **flag flip** as the human launch gate.

**Mapping to canonical stage:**

| Halyard native | Canonical stage | Notes |
|----------------|-----------------|-------|
| `tagged`, `built` | Build | being assembled |
| `tested`, `uploaded` | Review | passed gates, in transit to store/review |
| `in_review` | Review (or **Blocked** if ASC/human action pending) | precedence: human gate wins |
| `shipped_dark` | Ship | shipped but flag-off; the **flip** is the human gate → **Blocked** |
| `live` | Live | flag flipped, publicity fired |
| `rolled_back` | Archived | (with `detail: "rolled_back"`) |
| `dead`, `rejected` | Failed | terminal |
| any state with **open proposals in the queue** | **Blocked** (`gate: "approval"` or `"flip"`) | the queue is the human-gate signal; `detail` keeps the pipeline position |

**How it reads** (see `§7` for the head decision): preferred path is a **read-only Halyard head's
read API** (`summarizeRelease`, `listProposals`); fallback is **spawning the `halyard` CLI** and
parsing its stdout JSON (`status`, `queue`) — both are first-class in the digest. The adapter is
written against a small interface so the two read paths are swappable without touching the mapping.

### 6.2 Audience adapter — post-run + backend status

**Native vocabulary** (from the Audience digest): post status
(`draft → generating → approval-pending → published`, plus `rejected`/`failed`) read via
`GET /posts` / `GET /posts/:id`; plus **backend liveness** via `GET /health` (`:8080`).

**Mapping to canonical stage:**

| Audience native | Canonical stage | Notes |
|-----------------|-----------------|-------|
| backend `/health` down | Idle + `health: "unknown"` | stack not running ⇒ no live project state |
| `draft` | Spec | composed, not yet generating |
| `generating` | Build | AI generation in flight |
| `approval-pending` | **Blocked** (`gate: "approval"`, deep-link `/queue`) | the approve-before-post human gate |
| `published` | Live | posted to platforms |
| `rejected` | Archived | (`detail: "rejected"`) |
| `failed` | Failed | terminal publish failure |

Audience contributes **its own status** (per `§4` of the roadmap) — one `ProjectCard` per active
post run, or a single rolled-up card "Audience: N awaiting approval" if per-post granularity is too
noisy (the adapter chooses; the board renders whatever cards it gets). Reads go through the same
**HTTP API the web UI uses**; the dashboard adds no new Audience surface. Audience's auth posture
(devAuth in the proving cycle, per the app-plugins spec) governs how the adapter authenticates —
the adapter inherits that, it does not solve auth itself.

### 6.3 Fleet adapter — mission phases

**Native vocabulary** (from the SP1 spec's mission state machine):
`QUEUED → PROVISIONING → SPEC →(oracle gate)→ BUILDING ⇄ CHECKING → REVIEWING → MERGE_CHECK →
PR_OPEN → DONE`, plus exception/terminal states `AWAITING_ORACLE_APPROVAL`, `NEEDS_HUMAN`,
`HALTED`, `NO_CHANGE`, `FAILED`, and the `phase_changed{from,to,reason}` event.

**Mapping to canonical stage:**

| Fleet native | Canonical stage | Notes |
|--------------|-----------------|-------|
| `QUEUED`, `PROVISIONING` | Plan | accepted, environment being prepared |
| `SPEC` | Spec | oracle generating the frozen test set |
| `AWAITING_ORACLE_APPROVAL` | **Blocked** (`gate: "oracle-approval"`) | T2/T3 human test-set approval |
| `BUILDING`, `CHECKING` | Build | the build/check loop |
| `REVIEWING`, `MERGE_CHECK` | Review | review rounds + clean-merge check |
| `PR_OPEN`, `DONE` | Ship | verified-mergeable PR opened / merged |
| `NEEDS_HUMAN`, `HALTED` | **Blocked** (`gate: "manual"`) | re-entry states needing a human |
| `NO_CHANGE` | Archived | empty diff, nothing to ship |
| `FAILED` | Failed | terminal |

The fleet adapter is the **cheapest to wire** because the cockpit already emits `phase_changed`
events the dashboard can subscribe to (no polling needed) — the fleet is the one source that can
feed the board **push-style** rather than pull (`§8`). A fleet "project" is a **mission unit**; the
board may group units under a parent repo/run if that grouping exists, else one card per unit.

### 6.4 App-plugin lifecycle adapter — hosted apps

**Native vocabulary** (from the app-plugins spec's canonical `plugin://state` enum):
`stopped → building → starting → health-probing → ready-probing → healthy → (error | stopped)`.

This source answers a **different axis** than the others: not "what stage is the *work* in" but
"is this hosted app *up*." It feeds the board as **operational health of the hosted-app projects**,
mapped onto the off-pipeline states plus `Live`:

| `plugin://state` | Canonical stage | Notes |
|------------------|-----------------|-------|
| `stopped` | Idle | registered, not running |
| `building` | Build | first-run image build (`detail: "building images"`) |
| `starting`, `health-probing`, `ready-probing` | Build | coming up (`detail` carries which probe) |
| `healthy` | Live | running and serving its head |
| `error` | Failed | crashed / failed to start (`detail` = last stderr line) |

**Why include it as a stage source at all:** a hosted app like Audience is *itself* a project the
operator cares about being up. But note the **double-count risk** with `§6.2`: Audience appears
both as an app-plugin (is-it-up) and via its post-run adapter (what-are-the-posts-doing). These are
**deliberately two cards on two axes** — "Audience (app): Live" and "Audience: 3 posts awaiting
approval" — *not* a bug. The board groups them under one **project family** by a shared
`family` tag (`§5` extension: an optional `family: "audience"` on the card) so they render together
without being merged, keeping the operational axis (up/down) distinct from the work axis (stage).

## 7. The Halyard-head decision (Q3 part 2)

**Recommendation: yes — item 4 is the concrete justification to graduate Halyard from "deferred"
by building a thin read-only head + read API, and this spec records that as the trigger.**

**The reasoning chain:**
- The app-plugins spec deferred Halyard **only because it's headless** and there was no consumer
  forcing a head (it's a CLI + library over git-backed JSON; no UI, no port — per its digest and
  the Halyard-head handoff brief).
- The dashboard is **exactly that consumer.** It needs Halyard's release/launch/proposal state
  (`§6.1`). The cleanest way to read it cross-process is a **small read API** in front of Halyard's
  already-DI'd, side-effect-free library (`summarizeRelease`, `listProposals`, `readRelease`) — the
  "library-backed" option the handoff brief calls preferred.
- That read API **is** the seed of the Halyard head the handoff brief specs. The dashboard's needs
  (status board + approval-queue *read*) are a **subset** of the head's scope (status board +
  approve + flip). So: **the dashboard's read API and the Halyard head are the same artifact,
  staged** — build the read half first (serves the board), add the write half (approve/flip) to
  complete the head. Item 4 doesn't just *justify* the head; it **pays for its first increment.**

**But the dashboard must not be blocked on the head shipping.** The Halyard head is its own
project on its own branch (`docs/superpowers/HANDOFF-2026-06-07-halyard-head.md`), owned by the
Halyard repo. So `§6.1` defines a **CLI-spawn fallback adapter** (parse `halyard status`/`queue`
stdout JSON) behind the same interface. Sequencing:
- **Dashboard 4-build can start against the CLI-spawn adapter immediately** (no Halyard change
  required — the CLI already prints JSON).
- **When the read API/head lands, the adapter swaps to it** for a cleaner, type-safe, no-stdout-
  parsing read — zero board change.

**Net:** the dashboard is the reason to give Halyard a head; the dashboard's read API is the head's
first increment; and the CLI fallback means **neither project blocks the other.** This is flagged
as a **downstream dependency / coordination point** in `§10`.

## 8. Read posture & refresh (locked decision #4)

**The dashboard is a read-only projector.** It never writes project state to any source. Three
consequences, each a deliberate guard:

1. **It reads, it doesn't own.** Halyard's `stateDir`, Audience's DB, the fleet's unit store, the
   plugin manager's process table — each remains the single source of truth for its projects. The
   board holds only its own annotation layer (`§5.3`). This sidesteps the Halyard-digest warning
   that "a desktop host writing the same `stateDir` as a CI workflow could conflict" — the board is
   a **reader**, the conflict class doesn't arise.
2. **Actions deep-link out, they don't act in-board.** A `Blocked` card's "act here" affordance
   **opens the owning tool** at the right place (`blocked.deepLink`: Halyard's queue, Audience's
   `/queue`, the cockpit's unit view) — it does **not** approve/flip/dispatch from the board this
   cycle. This keeps every tool's human-gate invariant intact (the Halyard digest is emphatic: no
   caller bypasses its deterministic gates) and keeps the board's blast radius at zero. In-board
   actions are roadmap (`§9`).
3. **Refresh is hybrid: push where free, poll otherwise.**
   - **Fleet:** subscribe to `phase_changed` events (push) — already emitted (`§6.3`), no polling.
   - **App-plugins:** subscribe to `plugin://state` events (push) — already emitted by the plugin
     manager (`§6.4`).
   - **Halyard & Audience:** **poll** on a coarse interval (default 15s, per-source configurable),
     because neither pushes today (Halyard writes JSON files; Audience is request/response). Each
     card's `staleAfterSec` (`§5`) greys a card whose poll is overdue, so a wedged poller shows as
     *stale*, never as *silently wrong*.
   - A manual **refresh** control forces an immediate re-poll of all pull sources.

**Degradation is first-class.** If a source is down (Halyard config root missing, Audience
`/health` failing, fleet daemon not running), its adapter yields cards with `health: "unknown"`
and the board greys that source's lane with a "source unreachable" note — the board stays useful
for the live sources and **never invents a stage for a source it can't reach.** This mirrors the
"safe-by-default degradation" posture both digests describe.

## 9. Trust, scope guards & roadmap

**In scope — trusted, single-operator, read-only:**
- Single machine, single operator; every source is first-party and local. No auth model beyond
  what each adapter inherits from its source (Audience devAuth, Halyard's no-auth-local-tool, etc.).
- The board reads and **links out**; it writes only its own annotation store (`§5.3`).

**Roadmap (named, not built):**
- **In-board actions** — approve/flip/dispatch *from* the dashboard once each tool's human-gate
  invariant can be honored through a typed action channel (Halyard's read API would gain its write
  half — the rest of the head; the fleet would expose a dispatch/resume command). The biggest item.
- **A bare-git/CI adapter** — for projects no tool tracks, inferring stage from branch/PR/CI
  status (the `§4` raw-git path deferred this cycle).
- **History / trend view** — stage-over-time, cycle-time-per-stage, "stuck for N days" — once the
  board persists a stage-transition log (it persists none today).
- **Push for Halyard/Audience** — replace `§8` polling with a Halyard file-watch / `reconcile`-hook
  and an Audience webhook, if poll latency proves too coarse.
- **Multi-machine / hosted aggregation** — one board over fleets on several machines.
- **Notifications** — push "a project just entered `Blocked`" to phone (Halyard already has a
  notifier port + approval webhook the board could subscribe to).

## 10. Downstream dependencies (flagged for integration)

This spec is **design-only**; it blocks and informs two downstream efforts:

1. **4-build (the dashboard build)** is **gated on this spec.** It implements `§3`–`§8`: the
   `ProjectCard`/`StageSignal` model, the four adapters, the read posture, and the Svelte board UI
   in the Command Center shell (a sibling to the Fleet tab / app-plugin switcher). It should start
   against the **CLI-spawn Halyard adapter** (`§7`) so it isn't blocked on the head.
2. **The Halyard head** (`docs/superpowers/HANDOFF-2026-06-07-halyard-head.md`) is **informed and
   partly justified** by this spec: item 4 is the consumer that graduates Halyard from headless
   (`§7`), and the dashboard's **read API is the head's first increment**. Coordination point: the
   head project and 4-build should agree on the read-API shape (`summarizeRelease`/`listProposals`
   surface) so the dashboard adapter swaps from CLI-spawn to API with no board change.

Both dependencies are also noted inline at `§6.1`/`§7`.

## 11. Artifacts & references

- Roadmap item: `docs/ROADMAP.md` §4 (Central Project Manager).
- Halyard digest: `docs/digests/halyard-digest.md` (release-state enum, queue, read paths, the
  headless constraint and "host as reader" guidance).
- Audience digest: `docs/digests/audience-digest.md` (post-status flow, `/posts` + `/health` reads,
  auth posture).
- App-plugins spec: `docs/superpowers/specs/2026-06-07-app-plugins-design.md` (`§3` canonical
  `plugin://state` lifecycle enum — the `§6.4` source; and the Halyard-deferred note this spec
  re-opens with a consumer).
- Halyard-head handoff: `docs/superpowers/HANDOFF-2026-06-07-halyard-head.md` (the head this spec's
  read API seeds).
- Fleet mission state machine: `docs/superpowers/specs/2026-06-05-command-center-sp1-design.md`
  (`§ State machine` — the `§6.3` source vocabulary + `phase_changed` event).
- Vision: `docs/command-center-vision.md` (the autonomous fleet whose output the board surfaces).

## Design Critique Log

### Critique Round 1
An independent reviewer found seven load-bearing flaws, several structural to the dashboard's
purpose:

1. **Stage ambiguity was existential (Q1 under-resolved).** The first draft said "fixed pipeline"
   but didn't say what happens when a source's state is *both* a pipeline position and a human gate
   (a release `in_review` *and* awaiting approval) — adapters would map it inconsistently, so the
   same situation could read `Review` on one card and `Blocked` on another. **Resolved:** `§3` now
   defines off-pipeline states as a separate axis and a single **precedence rule**
   (`Failed` > `Blocked` > pipeline > `Idle`) applied uniformly by every adapter, with the pipeline
   position preserved in `detail`.
2. **"Inferred or declared" was a false binary (Q2).** Pure-inferred misses untracked projects
   (an idea, a bare repo) and operator knowledge that contradicts the signal; pure-declared rots.
   **Resolved:** `§4` adopts **inferred-with-declared-override**, with the override **stamped,
   TTL-expiring, and conflict-surfaced** so a declared stage can neither rot silently nor hide
   drift from the live signal.
3. **The Halyard-head question was answered "maybe" (Q3).** The draft noted the head *could* help
   but didn't commit, leaving 4-build blocked on an undecided dependency. **Resolved:** `§7` commits
   — **yes**, item 4 is the justification; the dashboard's read API **is** the head's first
   increment; and a **CLI-spawn fallback** ensures neither project blocks the other. The
   relationship is now a sequencing plan, not a hope.
4. **No "what is inferred from" concreteness.** "Inferred from git/commits/CI" (the roadmap's own
   phrasing) would duplicate logic every tool already does. **Resolved:** `§4`'s table names the
   exact machine-state each adapter reads (release enum, post status, mission phase, plugin state)
   and **explicitly excludes raw git/CI** this cycle, deferring a bare-git adapter to `§9`.
5. **Audience would be double-counted.** It's both an app-plugin (is-it-up) and a post pipeline
   (what-are-posts-doing); naive aggregation either merges them (losing the up/down axis) or shows
   two confusing unrelated cards. **Resolved:** `§6.4` makes the two cards **deliberate, on two
   axes**, grouped by a `family` tag so they render together without merging.
6. **The board's write posture was unstated → conflict risk.** Without a stated read-only rule, a
   build could have the board write back to Halyard's `stateDir` — exactly the concurrent-writer
   conflict the Halyard digest warns about. **Resolved:** locked decision #4 + `§8` make the board a
   **read-only projector** that deep-links out for actions; the override store is its only writable
   state and lives nowhere near a source.
7. **No degradation story.** A source being down would either hang the board or show a stale/wrong
   stage. **Resolved:** `§8` makes degradation first-class — a down source yields
   `health: "unknown"` greyed cards and never an invented stage; `staleAfterSec` greys overdue
   polls so wedged = visibly stale, not silently wrong.

### Critique Round 2
A fresh reviewer confirmed R1's Q1–Q3 resolutions were sound, then found six remaining flaws,
mostly in the data model and refresh semantics:

1. **`ProjectCard.stage` alone was too coarse to be actionable.** "Blocked" doesn't tell the
   operator *what* to do; a board of bare stages forces opening each tool anyway, undercutting the
   glance goal. **Resolved:** `§5.1` adds a **structured `blocked` object** (`gate`/`action`/
   `deepLink`) so the board renders a consistent "act here" affordance and can count "how many need
   me" — the board's single most valuable number, named in `§5.1`.
2. **`projectId` collisions across sources.** Two tools could mint the same native id. **Resolved:**
   `§5.1` mandates **source-prefixed** ids (`<source>:<native>`), which also gives the board a clean
   route back to the right adapter for refresh/deep-link.
3. **Refresh model was hand-waved as "poll everything."** Polling the fleet when it already emits
   `phase_changed` is wasteful and laggy. **Resolved:** `§8` defines a **hybrid** model — push for
   fleet + app-plugins (events already emitted), poll for Halyard + Audience (no push today) — with
   per-source intervals and a manual refresh.
4. **"Health" and "stage" were conflated.** A source being unreachable was modeled as a stage,
   so "Halyard down" could read as a *project* stage. **Resolved:** `§5.1` splits **`health`**
   (`ok`/`degraded`/`unknown`, source/probe liveness) from **`stage`** (the project's position), so
   an unreachable source greys cards without ever asserting a false stage.
5. **The manual/untracked-project path from R1's Q2 had nowhere to live.** Declaring an "Idea"
   project with no tool behind it needs storage. **Resolved:** `§5.3` adds the **dashboard-owned
   store** (override store + manual-project registry) in a single machine-local JSON file, explicitly
   *not* a second source of truth.
6. **Override expiry could still strand a card mid-air.** If an override expires while the source is
   *also* unreachable, what stage shows? **Resolved:** precedence is explicit — an expired override
   is dropped and the card reverts to **inferred**; if inference is also unavailable (source down),
   `health: "unknown"` + greyed wins (`§4`/`§8`), so the card degrades visibly rather than showing a
   stale declared stage.

### Critique Round 3
A final reviewer confirmed the R1/R2 fixes are internally consistent and mutually compatible, that
scope is still single-spec-sized (one board, four adapters, read-only), and that the Halyard-head
sequencing holds. Four remaining flaws, all cheap, resolved:

1. **[HIGH] The fleet "project" unit was undefined.** A fleet mission is a *unit*, not obviously a
   "project," and one repo can have many units — the board could explode into hundreds of cards.
   **Resolved:** `§6.3` defines a fleet project as a **mission unit**, with optional grouping under
   a parent repo/run where that grouping exists, so card count stays legible.
2. **[MED] Stage-vocabulary drift risk between spec and adapters.** The canonical stage names
   appeared in prose in several places with slightly different casing/wording. **Resolved:** `§3`
   declares the canonical pipeline + off-pipeline names **verbatim once**, and every `§6` mapping
   table maps *into* those exact strings — the single source of truth for stage names, mirroring how
   the app-plugins spec pinned its `plugin://state` enum.
3. **[MED] Audience per-post granularity could swamp the board.** One card per post run could mean
   dozens of cards from one tool. **Resolved:** `§6.2` lets the Audience adapter **roll up** to a
   single "Audience: N awaiting approval" card when per-post is too noisy — the board renders
   whatever cards the adapter emits, so granularity is an adapter policy, not a board concern.
4. **[LOW] The CLI-spawn fallback's correctness rested on an assumption.** `§7` assumed `halyard
   status`/`queue` print enough to derive every mapped stage. **Resolved:** `§6.1`'s mapping is
   driven by the **release-state enum + queue contents**, both of which the digest confirms the CLI
   prints as JSON on stdout — so the fallback adapter is sufficient on its own, and the head is a
   *cleanliness* upgrade, not a correctness prerequisite.

**Outcome:** no remaining existential flaws, no scope creep, no unresolved contradictions. All
three roadmap open questions (Q1 fixed-pipeline, Q2 inferred-with-declared-override, Q3
adapters + a justified-and-sequenced Halyard head) are resolved with no remaining placeholders.
Design approved through three independent adversarial rounds.
