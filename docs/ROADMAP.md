# Command Center — Roadmap

> Cross-cutting roadmap for the Command Center. App-plugins-specific roadmap items live in
> [`docs/superpowers/specs/2026-06-07-app-plugins-design.md` §5](superpowers/specs/2026-06-07-app-plugins-design.md);
> this file holds the broader product + workflow backlog.
> Last updated: **2026-07-16** (work-audit reconcile: #36 Local-Tracker Phase 1 shipped; hardening
> backlog H1–H4 all merged; vision sharpened to "one-stop shop for agentic engineering"; Remote
> Control added as a future pillar; auth-foundation-first build order locked).
> 2026-06-25: H1–H3 shipped (PR #31), H4 (PR #34). 2026-06-11: items 1, 4, 5 shipped; 2, 3, 6 partial.

## North Star

> **Leverage Claude Code as intelligently, seamlessly, and autonomously as possible** — keep costs
> low through intelligent resource use, maintain good context hygiene, and ship useful code
> autonomously. Build cool, useful things quickly.

**The synthesis (2026-07-16):** the Command Center is becoming the operator's **one-stop shop for
agentic engineering** — a single surface to *dispatch* autonomous work, *see* every project's stage
across sources, *act* on it without alt-tabbing, and (soon) *host the other tools inside it*. The
sooner it is that single surface, the better. **Feature-complete before launch.**

**Membership test:** a feature belongs in the Command Center **if and only if it serves that
goal.** Use it to accept or reject roadmap items. Every item below is an expression of one pillar:

| Pillar | Items |
|---|---|
| **Low cost** (intelligent resource use) | 1 Cache timer · 5 Rate-limit retry · 6 Budget discipline |
| **Context hygiene** | 3 Context offload · 6B ContextCurator |
| **Ship autonomously & fast** | 2 Swarm Handoff · 4 Project dashboard (+ Local Tracker) |
| **Remote reach** (future) | **R Remote Control** — drive the fleet from away-from-desk |

### Build order (locked 2026-07-16) — auth-foundation-first

1. **Local-Tracker Phase 2 dispatch** — the keystone that turns the board from a *viewer* into a
   *command surface*. Its daemon-wide loopback-auth migration (tracker spec §7.4) is **also the exact
   security foundation Remote Control needs** — build it once, here.
2. **Resolve P4 → dispatch the app-plugin + view-plugin embedding swarms** — hosts the other tools
   inside the shell (the "no alt-tabbing" promise).
3. **Design overhaul** — make the one-stop shop pleasant to live in (blocked on Claude Design output).
4. **Remote Control** — its own brainstorm→spec, *after* Phase-2 auth lands (rides on it).

Legend — **Status:** 💡 idea · 🛠️ in progress · 🔗 blocked on a dependency · ✅ shipped.
**Lane:** `workflow` (how the agent operates — hooks/skills/harness) · `product` (Command Center app code).

> **In flight (current build, not a roadmap item):** **app-plugins** — host whole web apps
> (proving app: Audience) in the cockpit. Phases 2–5.1 done (backend lifecycle complete: 17 Rust +
> 2 JS tests green, clippy clean); gated on a hands-on Phase-0 webview spike before the embedding
> phases. **P3 status (2026-07-16):** the `spike_show` hang is fixed (async commands, off-main-thread
> webview creation) and the hide/show finding is captured in the spec — **effectively done, pending
> only the go/no-go writeup in `SPIKE-RESULTS.md`.** See [the spec](superpowers/specs/2026-06-07-app-plugins-design.md) and
> [plan](superpowers/plans/2026-06-07-app-plugins.md). Its own roadmap items (third-party isolation,
> production auth, host↔app bridge, secrets, external-nav hardening) live in the spec's §5.

---

## ⚠️ Requires your attention — human-gated (not swarmable)

These are the only blockers on the road to a **shippable + feature-complete** Command Center that an
agent **cannot** do autonomously: each needs a watched/visual session, a real credential + spend, or
out-of-repo procurement. Everything downstream of them is already built or dispatch-ready. Source:
[`docs/handoff/2026-06-11-post-launch-swarm-handoff.md`](handoff/2026-06-11-post-launch-swarm-handoff.md).

| # | Item | Why only you | Unblocks |
|---|---|---|---|
| **P3** | **App-plugin webview spike — write up the verdict.** Hang fixed + hide/show finding captured (see In-flight note); the spike is *effectively done* — just needs the go/no-go recorded to `spikes/SPIKE-RESULTS-app-plugins.md` so the embedding swarm is formally unblocked. | Interactive/visual judgment already done; only the writeup remains. | **App-plugin embedding** feature swarm (`app-plugins-design.md` §6). |
| **P4** | **View-plugin handshake spike — one debugging session from a verdict.** First watched run: **all 100 rounds dropped** (systematic, not a race). Leading hypothesis (documented, unverified): module scripts fetched CORS-mode → `sdk.js` never runs → no `plugin-hello`. Cheap fix identified (`Access-Control-Allow-Origin: *` on the scheme handler). **Re-run watched `tauri dev`, confirm the fix, record 100/100.** | Needs a watched run across dev + packaged builds. | **View-plugin runtime** swarm (+ the `feat/view-plugins` de-stale pre-step). |
| **S3** | **One live paid T1 mission.** Set `ANTHROPIC_API_KEY`; dispatch a real T1 mission oracle→build→review→PR on a throwaway repo, human-watched. | Real credential + real token spend + live observation. The last unproven slice of the SP1 spine. | Confidence in the end-to-end spine on real tokens. |
| **Certs** | **Code-signing certs.** Apple Developer ID ($99/yr + notarization) + Windows Authenticode. Wiring + exact secret names already done — see [`docs/release/signing-and-updates.md`](release/signing-and-updates.md) §4; `release.yml` consumes them by name. | Procurement (CA / Apple Developer Program) — out of repo. | The **signed cross-platform release run** (CI is otherwise ready). |

**Status of the rest:** human-authority overlays (PR #22) + packaging/release hardening (PR #23)
shipped; **Project Dashboard + Local-Tracker Phase 1 shipped (PR #36, 2026-07-12).** Once P3/P4 each
record a "go", the two **blocked feature swarms** (app-plugin embedding, view-plugin runtime) are
dispatch-ready. Per the locked build order, **Local-Tracker Phase 2 dispatch is the next build** (it
also lays the Remote-Control auth foundation). Remaining to **shippable** = certs + one signed release
run + one live paid T1 mission (S3).

> **Note (verify):** CI runner allocation was blocked by a GitHub Actions billing failure. #36 merged
> 2026-07-12, which *suggests* billing may now be resolved — confirm with a fresh `gh run` before
> relying on green CI.

---

## 1. Cache-aware approval timer  ·  ✅ shipped (installed & live in `~/.claude/settings.json`)  ·  lane: workflow

A live countdown of the Anthropic prompt-cache TTL (~5 min / 300s) shown whenever a task is
**awaiting user approval**, so the user responds before the cache goes cold and a large session
has to be re-read at full cost.

- **Mechanism (reference impl: [`KatsuJinCode/claude-cache-countdown`](https://github.com/KatsuJinCode/claude-cache-countdown)):**
  a `Stop` hook + `UserPromptSubmit` hook write `~/.claude/state/cache-timer-{session}.json`; a
  ticker reads it and counts down `295 − elapsed`, showing `🔥 HOT → 🟢 → 🟡 → 🔴 → ❄️ COLD` plus
  **cost-at-stake** (e.g. `🔴 0:45 $5.75`), with escalating bell alerts at 60/30/10s.
- **Here:** PowerShell installer (Windows box); Python ticker via **UV** (per global rules).
  Installs two entries into `~/.claude/settings.json`.
- **Serves:** low cost. **Pairs with:** 5 (retry within the cache window), 6E (cache-aware pacing).
- **Why it's the trigger that matters:** "task awaiting user approval" == the `Stop` hook fires.

## 2. Swarm Handoff — master-agent capability  ·  🛠️ partial (fleetd engine shipped; skill wrapper deferred)  ·  lane: workflow

Turn "Swarm Handoff" from a hand-run process into a **first-class ability the orchestrating agent
invokes itself**: given a spec/plan with several *independent* features, it automatically
decomposes the work into parallel lanes and dispatches a swarm.

- **Steps:** (1) **detect parallelizability** — dependency-analyze the doc; separate truly
  independent features from serially-coupled ones (false independence → merge collisions, the
  core risk); (2) **carve lanes** — one feature ≈ one lane with explicit file ownership, shared
  contracts, and "don't touch another lane's files"; (3) **dispatch** — fan out parallel agents
  (built on `Workflow` / `dispatching-parallel-agents` / `subagent-driven-development`, worktree
  isolation where lanes would collide); (4) **integrate** — merge order, checkpoints, reconcile.
- **Likely form:** a `swarm-handoff` skill.
- **Reference template:** the Swarm Handoff companion doc produced this session
  ([`docs/SWARM-HANDOFF.md`](SWARM-HANDOFF.md)) — its **Part I (protocol)** is the skill's body and
  **Part II** is the first worked example this capability would later auto-generate.
- **Serves:** ship autonomously & fast.

## 3. Dual-tier context offload for budget  ·  🛠️ partial (Tier 1 shipped; Tier 2 🔗 blocked on claude.ai connector)  ·  lane: workflow

Cut token re-spend by moving durable context out of the active window and pulling it on demand,
managed automatically by the agent. **Two complementary tiers (no conflict — different content):**

- **Tier 1 — Claude Code project memory + ContextCurator (local, always available):**
  agent-*derived* durable facts (decisions, gotchas, "why"). Small `MEMORY.md` index loaded each
  session; full notes recalled on demand. Works headless.
- **Tier 2 — Claude.ai Project knowledge base (cloud, retrieval-backed):** bulk *stable documents*
  (specs, digests, vision) retrieved on demand instead of carried in-window — where the big token
  savings live. Bridged from Claude Code via the claude.ai MCP connector.
- **Two guardrails (where a naive "both" would bite):**
  1. **Single source of truth:** **repo git = source of truth** for documents; the Claude.ai
     Project is a one-way *projection* synced from the repo; the local memory store holds only
     agent-derived facts (no doc duplication) — eliminates drift.
  2. **Graceful headless degradation:** the claude.ai connector is interactively-authenticated and
     can be absent in cron/CI; "behind the scenes" must fall back to Tier 1 + repo, never block.
- **Serves:** context hygiene + low cost. **Overlaps:** the existing memory system + ContextCurator.

## 4. Central Project Manager — project-tracking dashboard  ·  ✅ board shipped · 🛠️ Local-Tracker Phase 2 = the keystone next build  ·  lane: product

**A core piece of this build — and the spine of the "one-stop shop."** Tell at a glance the **stage
every project is in**, and act on it.

> **Status (2026-07-16):** board built + mounted; **Local Project Tracker Phase 1 shipped (PR #36)** —
> the board now reads a project's own `docs/STATUS.md` front-matter + `ROADMAP.md` `cc-item`s as a
> fifth (`local`) source. **Phase 2 (one-click dispatch a roadmap item → real fleet mission →
> loops back onto the board) is fully specced through 3 critique rounds (Option A) but NOT built —
> and it is the locked next build.** It converts the board from a viewer into a command surface and
> lays the daemon-wide loopback-auth foundation Remote Control (pillar R) reuses. Spec:
> [`2026-07-06-local-project-tracker-design.md`](superpowers/specs/2026-07-06-local-project-tracker-design.md).

- **Data sources / integration:** **Halyard** is the natural backend — per its digest it's already
  a git-backed JSON store *over project/work state*, so it's the obvious source of "what stage is
  this project in." **Audience** contributes its own status. The Command Center already emits
  aggregatable signals: fleet mission phases and the app-plugin lifecycle states
  (`building→…→healthy`).
- **Cross-repo changes are in-scope** for Halyard and Audience to feed this.
- **Big enough to need its own brainstorm → spec cycle.** Open design questions for that spec:
  - What *is* a "stage" — a fixed pipeline (spec→plan→build→review→ship) or per-project?
  - Is stage **inferred** (git/commits/CI) or **declared**?
  - This is exactly what a Halyard *head* could surface — giving Halyard a reason to graduate from
    "deferred" (see the app-plugins spec's Halyard notes + the Halyard-head brief).
- **Serves:** ship autonomously & fast (visibility into the autonomous fleet's output).

## 5. API rate-limit auto-retry  ·  ✅ shipped (harness `CLAUDE_CODE_MAX_RETRIES` verified + live)  ·  lane: workflow

Automatic, periodic retries when the **Anthropic API** returns "Server is temporarily limiting
requests" (429s) — at the **harness/agent level**.

- **Distinct from existing work:** the repo already has *fleetd-level* rate-limit resilience
  (merged `feat/rate-limit-resilience`; the cockpit rate-limited chip; fleetd backoff tests). This
  item is a *different layer* — the agent's own API calls, not the fleet's.
- **Coordinate with the cache window (1):** retry/backoff should stay **within** the ~5-min cache
  TTL where possible so a 429 doesn't force a cold, full-cost re-read.
- **Serves:** low cost + seamless autonomy (don't stall on transient limits).

## 6. Proactive budget discipline  ·  🛠️ partial (rules A/C/D/F/G shipped; 6B 🔗 ContextCurator, 6D hook optional)  ·  lane: workflow

Umbrella item: make existing capabilities into **automatic** budget discipline rather than manual
practice. Concrete mechanisms:

- **A. Exploration-always-via-subagent.** Broad searches / multi-file reads never enter the
  master's context — fan to `Explore`/subagents, keep only conclusions. (Caps master-context
  growth, the main cost driver in long sessions.)
- **B. Context-window hygiene via ContextCurator** (`cc_evict`/`cc_pin`). Auto-evict large stale
  tool results once their conclusion is captured; pin durable facts. 🔗 **Blocked / in-flight:**
  ContextCurator is the **user's own product**, not ours to build — **integrate its API when it
  ships.** See [[contextcurator-is-users-own-product]] in project memory.
- **C. Cheapest-capable-model routing** as a standing rule (mechanical → cheap, judgment/review →
  capable), applied everywhere, not just inside `Workflow`.
- **D. Proactive checkpoint at boundaries.** Auto-trigger `handoff`/`end-session` at phase/spike
  boundaries so the next session starts compact (as done at this project's spike gate). Pairs with 3.
- **E. Cache-window-aware pacing.** Coordinate work cadence + `ScheduleWakeup` with the ~5-min TTL;
  keep cache warm mid-task, let it cool when idle; retry within-window. Pairs with 1 and 5.
- **F. Workflow token-budget directives** as standard practice — explicit per-phase budgets, fan-out
  scaled to budget, `log()` what's dropped.
- **G. Digest-first reading.** Maintain codebase digests (the `codebase-digest` skill, used for
  Halyard/Audience) and read those instead of re-reading source.
- **Serves:** low cost + context hygiene. **Cross-links:** B+G+D feed 3; E feeds 1 & 5.

---

## R. Remote Control  ·  💡 named pillar — spec after Phase-2 auth lands  ·  lane: product

Drive the fleet **from away-from-desk** — check what needs a human, approve a gate, dispatch a roadmap
item, watch a mission, without being at the machine running the cockpit. The natural extension of the
"one-stop shop": the shop should be reachable, not desk-bound.

- **Why it waits on Phase 2, not the reverse:** `fleetd` today is an **unauthenticated** daemon on
  `127.0.0.1:8787` wielding the operator's ambient `gh`/git credentials (full push). It **cannot** be
  exposed remotely as-is. Local-Tracker Phase 2 already requires building **router-level loopback auth
  over every mutating endpoint + a token handshake** (tracker spec §7.4). That is exactly R's security
  prerequisite — so R is *cheap* once Phase 2 lands and *reckless* before it.
- **Open design questions (for R's own brainstorm→spec):**
  - **Transport:** expose the authed daemon over a tunnel/relay, or a thin cloud broker? (Never a raw
    port-forward of an ambient-credential daemon.)
  - **UI shell:** the cockpit is a Svelte SPA today wrapped in Tauri — could it *also* be served as a
    PWA/web client (same UI, two shells) talking to the authed daemon? Reuses the design-system work.
  - **AuthN/Z for a remote human:** device pairing, short-lived tokens, and which actions a remote
    session may take (view-only vs approve-gate vs dispatch).
  - **Push:** "a mission needs you" as an actual push notification to the phone.
- **Serves:** ship autonomously & fast (the operator supervises from anywhere). **Membership:** passes
  the test — it makes the autonomous fleet *more* usable, not a new unrelated surface.
- **Depends on:** Local-Tracker Phase 2 auth (hard prerequisite); benefits from the design overhaul
  (a served web shell wants the new design system).

## Hardening backlog (session-state plugin — item 3 Tier 1)

Non-blocking follow-ups carried over from the session-state plugin port (PR #30, merged 2026-06-23;
see [`docs/handoff/2026-06-21-session-state-plugin-shipped.md`](handoff/2026-06-21-session-state-plugin-shipped.md)).
The plugin is shipped and green; these are known edge cases, not regressions.

**✅ CLEARED (2026-07-16) — all four merged.** H1 (`resolve.mjs` semver sort), H2 (`lock.mjs`
torn-token steal), and H3 (`keying.mjs` malformed-meta collision) landed via **PR #31**; **H4**
(`keying.mjs` path-separator spurious collision that blocked `save-state` capture) landed via **PR #34**
— its merged fix also *heals* the stored path to canonical form so the mismatch cannot recur. The hard
release gate (H1) and the whole hardening backlog are cleared.

- **Follow-up (not a code item):** the *installed* session-state plugin on this machine is still
  v0.1.0 (pre-H1–H4); the fixes are on `main` but not re-released/reinstalled. Cut a `0.10.x` plugin
  release when convenient (now safe — H1 gate cleared).
- **Serves:** context hygiene (item 3 Tier 1 reliability).

---

## Dependency notes (for sequencing / Swarm Handoff lane-carving)

- **Independent, parallelizable now:** 1 (cache timer), 5 (rate-limit retry), 6A/C/F/G (discipline
  rules), and the *design/spec* of 4 (dashboard).
- **Blocked / sequenced:**
  - **6B** 🔗 waits on the user's ContextCurator shipping.
  - **3 Tier 2** 🔗 needs the claude.ai connector reliably available; Tier 1 can proceed now.
  - **4 (build)** is gated on its own spec, and benefits from a **Halyard head** (today Halyard is
    headless; see the app-plugins spec + Halyard-head brief).
  - **2 (Swarm Handoff capability)** is best built *after* this session's hand-made Swarm Handoff
    doc proves the format.
- **Recurring theme:** 1, 5, and 6D/E all orbit the same 5-minute cache window — treat them as one
  coherent "cache-economics" cluster when scheduling.
- **Auth foundation:** **Local-Tracker Phase 2** and **Remote Control (R)** share the daemon-wide
  loopback-auth migration — build it once in Phase 2 and R inherits it. Do not spec R's transport
  until Phase 2's auth model is settled in code.
- **Release gate:** cleared — the entire hardening backlog (H1–H4) is merged (PR #31 + #34); a
  `0.10.x` session-state plugin release is safe whenever convenient.
