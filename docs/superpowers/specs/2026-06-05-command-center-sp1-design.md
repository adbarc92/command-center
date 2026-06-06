# SP1 — Fleet Engine + Cockpit (Walking Skeleton)

> Status: **passed 3 adversarial critique rounds + user grilling; awaiting final user
> review of this spec.** All design open questions resolved (see Open Questions). Remaining
> unknowns are two implementation-prerequisite validation spikes. See the Design Critique
> Log at the bottom.
> Parent: [../../command-center-vision.md](../../command-center-vision.md)
> Last updated: 2026-06-05

## Goal of the walking skeleton

Prove the riskiest, most novel part end-to-end with **one unit**: dispatch a task →
**oracle phase generates + freezes a test set** (tier-gated human approval) → one container
working in an **isolated per-unit clone (never the user's real repo)** → a
build/check/review loop with hard caps that must satisfy the frozen tests → **daemon exports
the branch and opens a verified-mergeable PR from the host** → see it in a bare cockpit.

### What SP1 actually proves (read before believing any claim below)

SP1 **generates its own objective oracle** (the user pulled this forward from SP2): a
separate oracle phase turns the task into a **frozen test set** the builder must satisfy
but cannot author or weaken — with a **tier-gated human approval** that makes the bar truly
objective for medium/high-stakes missions. SP1 therefore de-risks four things:
(1) a **host-damage boundary** (the container) + a **credential boundary** (push creds
host-only) so a skip-permissions agent can neither wreck the machine nor push/merge on its
own; (2) the **objective-oracle-via-separation-of-powers** loop (oracle agent ≠ builder,
frozen tests, tamper-guard, tier-gated human approval); (3) the **loop + cap mechanics**
terminating without runaway cost *even if the daemon dies*; (4) **cross-platform
mergeable-PR mechanics** — the branch escapes the container to a byte-valid host repo and
the PR is verified *actually mergeable*. (What stays in SP2: the *rich* onboarding UX —
interactive PRD/grill-me, test-quality scoring. SP1's oracle is the minimal
generate-then-freeze step.)

### Isolation & threat model (RESOLVED 2026-06-05)

The real purpose of the container is **to protect the user's computer from a
`--dangerously-skip-permissions` agent**, not to lock down its network. The threats split:

| Threat | Boundary | SP1 stance |
|--------|----------|------------|
| **Host damage** (rm -rf, writes outside project, OS tampering) | **container** filesystem/process isolation | primary goal |
| **Git/repo damage** (force-push, cross-repo, self-merge) | **push creds live only in `fleetd`** | enforced |
| **Data exfil via the model channel** | ~unfixable while the agent talks to a model | **accepted residual** (user's own repo) |

Because exfil is an accepted residual, **network egress is OPEN** — dependencies install
normally, the test oracle runs, and the earlier "egress allowlist / registry-mirror"
trilemma and its validation spike are **deleted.**

### Isolation backend: Docker, cross-platform, hardened on Linux via the Runner trait

The app must run on **Windows, macOS, and Linux**, which makes **Docker the only universal
isolation primitive** (Firecracker = Linux+KVM only; gVisor = Linux only). The payoff:

- **Windows / macOS:** Docker runs containers **inside a lightweight VM** (WSL2 / Docker's
  Linux VM), so the agent is **VM-isolated from the host for free** on exactly the two
  platforms where the host is a daily-driver.
- **Linux / remote `fleetd`:** a container shares the host kernel (weaker). The `Runner`
  trait makes **gVisor (`runsc`) or a microVM (Kata/Firecracker)** an **optional Linux-only
  hardening backend** behind the same interface — strong isolation where it's actually
  needed, with **zero portability cost and no rearchitecture.**

### Validation spikes — both **PASSED 2026-06-05** ✅

Full evidence: [/spikes/SPIKE-RESULTS.md](../../../spikes/SPIKE-RESULTS.md).

1. **Cross-platform git escape round-trip — PASS.** Commit in a **named Docker volume**
   clone → `git bundle create` (complete, `git bundle verify` = no prerequisites) →
   temp-container `docker cp` to host → host `git fetch`/clone → `git fsck` clean →
   **byte-identical SHA** (`81877030…`). No NTFS `.git` corruption. Becomes
   `Runner::export_bundle` in Phase 2 (set `core.autocrlf/fileMode/symlinks=false` at clone
   time; host clone is non-bare).
2. **Cost/token metering — PASS (best case).** The `result` record carries `total_cost_usd`
   + full `usage` tokens; daemon sums per run for the USD cap, and the built-in
   **`--max-budget-usd`** flag is the daemon-independent backstop. (Resolved the Section 4
   cost caveat; `--max-turns` doesn't exist in 2.1.x — wall-clock via `timeout`.)
3. *(Deferred, non-blocking)* **gVisor/microVM on Linux** — validate the optional hardening
   backend when `fleetd` first targets a remote Linux host (not needed for the local
   Win/Mac skeleton).

---

## Section 1 — Architecture & process topology

Three processes, one isolation seam, two contract directions (events out, commands in).

```
┌─────────────────────────────┐         ┌──────────────────────────────┐
│  COCKPIT (Tauri app)         │         │  fleetd (Rust daemon)        │
│  · launches + health-checks  │ HTTP +  │  · REST: create/inspect      │
│    the fleetd sidecar        │  WS     │  · REST control: halt/resume │
│  · Web UI (Vite + Svelte+TS) │◀───────▶│    /abandon (IN)             │
│    new-mission form;         │ (local- │  · WS /stream: events (OUT)  │
│    one unit; PR link         │  host)  │  · owns lifecycle + caps     │
└─────────────────────────────┘         │  · HOLDS git push creds      │
                                         │  · git ops in host clone only│
                                         │            │ Runner trait    │
                                         └────────────┼─────────────────┘
                                                      │  (the seam)
                                   ┌──────────────────┴──────────────────┐
                                   │ LocalDockerRunner (bollard → Docker) │
                                   │  · 1 labeled container per unit       │
                                   │  · clone in a NAMED VOLUME (POSIX)    │
                                   │  · NO push creds · network OPEN       │
                                   │  · self-kill watchdog (daemon-indep.) │
                                   │  · claude --dangerously-skip-perms    │
                                   └──────────────────────────────────────┘
```

**Decisions:**

1. **`fleetd` is a separate Rust binary**, talked to over localhost HTTP+WS; Tauri
   launches it as a sidecar. Remote later = repoint the cockpit.
2. **The `Runner` trait is the isolation seam**, kept in SP1 because the `FakeRunner` it
   enables is how we test `fleetd` without Docker (Section 6). Surface:
   - `provision(spec) -> Handle` — pulls pinned image **by digest**; creates a **labeled**
     container (`cc.unit_id=<id>`) with **CPU/mem limits**, **open network** (the container
     is a host-damage boundary, not a network jail; optional gVisor/microVM hardening on
     Linux via this same trait), a **per-unit named volume** holding the isolated clone, and
     a **self-kill watchdog** wrapping the agent (a `timeout`/in-container guard) so the
     container terminates at max-wall-clock **even if `fleetd` is dead**.
   - `exec(handle, cmd) -> Stream<Line>` — **streaming**, **cancellable**; cancellation is
     `SIGTERM` + grace period (let an in-flight `git commit` finish) then `SIGKILL`.
   - `health(handle) -> Liveness` — for stall detection.
   - `export_bundle(handle) -> HostPath` — runs `git bundle` in the volume and `docker cp`s
     it to the host (the cross-platform escape; **no NTFS bind-mount of `.git`**).
   - `teardown(handle)` — SIGTERM+grace+remove; the **named volume persists** (it is the
     durable unit state used by `resume`).
3. **Credential boundary is architectural:** push creds + GitHub API client live **only in
   `fleetd`**. The daemon never operates on the container's `.git`; it imports the exported
   bundle into its **own host clone** and pushes from there (no shared-`.git` race).
4. **Two contract directions:** events OUT on WS `/stream`; commands IN on REST control.

**Stack:** Cargo workspace — `fleetd`, `cockpit` (Tauri Rust), `cockpit/ui` (Vite +
Svelte + TS). Docker via `bollard`.

---

## Section 2 — Unit lifecycle + the two-way contract

### State machine (re-entry edges + timeouts)

```
QUEUED → PROVISIONING → SPEC ─(oracle gate)─→ BUILDING ⇄ CHECKING → REVIEWING ──┐
                         │                       ▲   ▲      │pass     │ no new  │
              (oracle agent generates the        │   └──────┘         │ blockers│
               test set from task/PRD in a   (blockers→rebuild)       │ & green │
               SEPARATE context, then FREEZE)                         ┤ & floor ▼
                         │                                            │  met)  MERGE_CHECK
   oracle gate (tier):   ▼                                           │           │ mergeable?
     T1 → auto-freeze, proceed                                       │           ▼ (poll; null=retry)
     T2/T3 → AWAITING_ORACLE_APPROVAL ─(approve/edit)→ freeze+build  │        PR_OPEN → DONE
                                       └(reject)──────→ SPEC         │      [T3: → NEEDS_HUMAN
                                                                     │       before PR; human ships]
   from ANY active phase:                                           ├─ empty diff → NO_CHANGE
     · cap breach ($/wall)     → NEEDS_HUMAN                        └─ conflict / dirty → NEEDS_HUMAN
     · stall                   → NEEDS_HUMAN
     · oracle tampering        → NEEDS_HUMAN
     · fatal error             → FAILED
     · user halt               → HALTED
   re-entry: NEEDS_HUMAN | HALTED | AWAITING_ORACLE_APPROVAL
               → (resume/approve) continue | (abandon) FAILED
   resume re-provisions a fresh container from the per-unit volume's last commit;
   if no commit exists yet, resume == cold restart from base
   (phase_changed{reason:"resumed_from_base_no_commit"}). A WIP commit is taken at the
   end of each build round so "last commit" is always meaningful.
```

**The oracle phase (new — SP1 now generates its own tests).** A distinct **oracle agent**
turns the task (and optional PRD) into a **test set**, in a *separate context from the
builder*. The set is **frozen and content-hashed**, then the builder must satisfy it
without being able to author or weaken it. This separation-of-powers is what keeps
generated tests an *objective* bar instead of the agent grading its own homework. (Richer
PRD/grill-me onboarding is SP2; SP1's oracle is the minimal generate-then-freeze step.)

**The build loop:** build → check → (red → rebuild with the failure as feedback) → green →
review → (new blockers → rebuild) → when the **gate floor is met** → `MERGE_CHECK` → clean
merge into a fresh base → `PR_OPEN`; else `NEEDS_HUMAN`.

**Gate = green checks (the objective signal) + review adds confidence, not truth.**
Auto-advance requires: checks green, AND the latest review round has no unresolved
blockers, AND blocker count non-increasing across rounds, AND a minimum round floor
(default 3). The reviewer is the **repo's `code-review` skill** — an LLM heuristic, not an
oracle.

**Tier = the autonomy ladder (now first-class in SP1).** Tier gates the two human
checkpoints:

| tier | oracle/test-set gate | final PR gate | objectivity of the bar |
|------|----------------------|---------------|------------------------|
| **T1** (low) | auto-freeze, no human | auto-open PR | separation-of-powers only (oracle agent ≠ builder); documented softer guarantee |
| **T2** (med) | **human approves/edits** frozen tests | auto-open PR | strong — human-blessed, builder can't game |
| **T3** (high)| **human approves/edits** frozen tests | **never auto-PR** → `NEEDS_HUMAN`, human ships | strongest |

**Oracle-tampering guard.** The frozen, content-hashed test set is the baseline. The gate
fails (→ `NEEDS_HUMAN`) if the branch **modifies or deletes** any baseline file; **adding
new tests is allowed and they must also pass.** Deletion can't buy green (baseline still
runs); added tests can't be vacuously green (they execute).

**Container preservation across pause (resolved):** cap/stall/halt → NEEDS_HUMAN/HALTED
**tears down the container** so cost stops (the north star demands it). The **per-unit
named volume persists**; `resume` re-provisions a fresh container from the last commit.
Live agent conversation state is intentionally **not** preserved — resume continues from
committed work + the outstanding `finding`s, not in-memory context. This is the accepted
tradeoff and the reason caps can actually stop the burn.

### Events — WS `/stream` (OUT)

Newline-delimited JSON, each `{unit_id, seq, ts, type, …}`; `seq` monotonic per unit.

| `type`         | payload                                          | drives in UI            |
|----------------|--------------------------------------------------|-------------------------|
| `phase_changed`| `from, to, reason?, cmd_id?`                      | status badge            |
| `oracle_proposed`| `test_files[], hash, summary`                  | test-set approval panel (T2/T3) |
| `iteration`    | `kind: build\|check\|review, n`                  | iteration counter       |
| `log`          | `stream: agent\|check\|system, line`             | live activity feed      |
| `metric`       | `tokens_in/out, cost_usd, elapsed`               | $/token meters          |
| `finding`      | `round, severity, title, file?, resolved?`       | review findings list    |
| `artifact`     | `kind: branch\|pr\|diff, ref`                    | PR link / diff button   |
| `blocked`      | `reason, cap?, detail`                            | "needs you" alert       |
| `error`        | `scope: docker\|github\|agent\|system, retryable, detail` | error banner   |
| `done`         | `result`                                         | terminal state          |

**SP1 keeps this minimal (scope control):** events are held in an **in-memory ring buffer**
per unit; a (re)connecting client calls `GET /units/:id` for the snapshot then tails live.
**No durable event log, no `since`-replay, no `log_dropped` backpressure** in SP1 — one
local unit + one local UI doesn't need them; they move to SP3 (fleet/remote). `seq` is a
simple in-memory per-unit counter; if `fleetd` restarts, the client refetches the snapshot.

### Commands — REST control (IN)

`POST /units/:id/{halt|resume|abandon}` and, for the T2/T3 oracle gate,
`POST /units/:id/{approve_oracle|reject_oracle}` (approve carries the optionally-edited test
set; reject sends the unit back to `SPEC`). All carry a client `cmd_id`; the effect is acked
on the stream via a `phase_changed`/`error` echoing `cmd_id`. **`takeover` is dropped from
SP1** (it was never defined; halt/resume/abandon cover the skeleton demo). A command lost to
a mid-flight daemon crash is recovered by the client refetching the snapshot and re-issuing —
acceptable for one local unit; a durable command journal is SP3.

**`halt` interleaving (defined):** cancellation `SIGTERM`s the in-flight `exec`, waits the
grace period so any in-progress `git commit` completes in the disposable volume, then tears
down. Because git activity is confined to the per-unit volume (never the user's repo or a
shared `.git`), the worst case of a torn commit damages only a disposable volume.

**Mission vs. Unit** stay separate (1:1 in SP1) — the one forward-looking concession.

---

## Section 3 — Execution pipeline

- **Oracle phase (`SPEC`):** a separate **oracle agent** (own context) turns the task +
  optional PRD into a test set; `fleetd` writes it into the per-unit clone, **content-hashes
  and freezes** it. T1 auto-proceeds; T2/T3 emit `oracle_proposed` and wait in
  `AWAITING_ORACLE_APPROVAL` for `approve_oracle`/`reject_oracle`.
- **Loop driver:** `fleetd` drives from the host via discrete bounded `exec` calls; it
  never trusts the agent's self-report of "done."
- **`checks` = the objective signal** (the frozen test set's command; exit code is truth),
  run by the daemon after the oracle-tampering guard passes.
- **Reviewer = the repo's `code-review` skill** (a separate `exec` in its own context),
  emitting structured `finding`s. An LLM heuristic — improves quality, not an oracle.
- **Findings feed back** as explicit context into the next build round.
- **Separation of powers:** oracle agent, builder, and reviewer run in **distinct contexts**
  so no single agent both sets the bar and clears it.

---

## Section 4 — Safety, secrets & caps (the crux)

See the Isolation & threat model section near the top for the full posture. In brief:

- **The container is the host-damage boundary.** A skip-permissions agent's destructive
  commands hit the container's filesystem, not the host. VM-isolated from the host on
  Win/Mac (Docker's VM); on Linux/remote, optionally hardened with gVisor/microVM via the
  `Runner` trait.
- **No push credential in the container, ever.** Container can `git commit` locally; it
  cannot push. `fleetd` exports the bundle and pushes from its own host clone.
- **Network egress is OPEN.** Dependencies install normally so the test oracle can run on
  arbitrary projects. The accepted residual is exfil-via-model (user's own repo). No
  allowlist proxy, no registry mirror, no dep pre-baking required.
- **Host-side token is fine-grained, single-repo, short-TTL**, with branch protection
  forbidding force-push and self-merge. Worst case the daemon opens a PR on one repo; it
  cannot merge or rewrite history.
- **Pinned base image by digest** (Claude CLI + git + toolchain). No `:latest`.
- **No host secrets mounted** beyond what the daemon controls; the per-unit clone lives in
  a named volume, not a bind-mount of the user's real repo.

**Hard caps (breach → NEEDS_HUMAN):**

| cap            | enforced by                                   | why                         |
|----------------|-----------------------------------------------|-----------------------------|
| **USD cost**   | daemon sums `total_cost_usd` from each run's `result` record | a huge iteration blows budget |
| **USD (backstop)** | **`--max-budget-usd`** passed into the agent (daemon-independent) | a real dollar ceiling that holds even if `fleetd` dies |
| **wall-clock** | daemon **and** in-container `timeout` wrapper | bounds elapsed even if daemon dies |
| **stall**      | daemon via `health` liveness                  | hung/silent container        |

**Cost cap (resolved by Spike 2 — see [/spikes/SPIKE-RESULTS.md](../../../spikes/SPIKE-RESULTS.md)).**
Claude Code's `--print --output-format stream-json --verbose` emits a terminal `result`
record with `total_cost_usd` and full `usage` tokens. The daemon sums `total_cost_usd`
across the unit's `exec` runs to enforce the USD cap (no price table needed — cost is
reported). The **daemon-independent backstop is the built-in `--max-budget-usd <remaining>`**
flag passed into the agent, which self-terminates the run at the dollar ceiling even when
the daemon is dead — strictly better than the token/`--max-turns` proxy originally assumed.
(`--max-turns` does not exist in CLI 2.1.x; wall-clock uses a `timeout` wrapper. Iteration
count is subsumed by wall-clock.)

**Daemon-crash cost containment.** The in-container watchdog terminates the agent at
max-wall-clock independent of `fleetd`. On startup, `fleetd` **reconciles**: `docker ps`
by `cc.unit_id` label and adopts (resumes tracking) or reaps (tears down) orphaned
containers. So a dead daemon cannot leave cost burning unbounded, and a restarted daemon
cleans up.

**Docker preflight.** On launch, `fleetd` verifies Docker is installed/running/reachable
and the pinned image is present (pull if not); failure → clear `error{scope:docker}`, not a
crash. Docker is a hard external dependency for SP1.

---

## Section 5 — PR mechanics & mergeability (cross-platform escape)

- **Isolated clone in a named volume**, created by the daemon from a host-side fetch of the
  base — **never the user's working repo, never an NTFS bind-mount of `.git`.**
- **Branch escape:** `export_bundle` runs `git bundle create` for a **complete,
  self-contained bundle** (no prerequisites — verified by `git bundle verify`) and
  `docker cp`s it to the host (identical on Windows/macOS/Linux). The daemon `git fetch`es
  the bundle into its own host clone. (Incremental bundles are out of scope — see spike #1.)
- **Base-drift (`MERGE_CHECK`):** in the host clone, fetch a fresh base and attempt a trial
  merge (advisory). Clean → push `agent/<id>` with the scoped token → open PR via GitHub
  API. Conflict → `NEEDS_HUMAN` (never push an un-mergeable branch).
- **Mergeability is GitHub-authoritative and async (R3).** GitHub computes `mergeable`
  lazily; it is `null` until ready. After PR creation the daemon **polls**
  `mergeable`/`mergeable_state` with bounded retries (`null` = retry, timeout →
  `NEEDS_HUMAN`). `mergeable_state: dirty` (base advanced after open) transitions
  `PR_OPEN → NEEDS_HUMAN`. Only `mergeable: true` reaches `DONE`. The local trial-merge is
  advisory; GitHub's async result is authoritative.
- **Preconditions:** branch has ≥1 commit and is pushed before PR creation (avoids 422s).
  A green-checks run with an **empty diff** is the legitimate **`NO_CHANGE`** terminal
  (`done{result:"no_change"}`) — a correct no-op is not a `FAILED`.

---

## Section 6 — Testing strategy

- **`FakeRunner`** (scripted `exec`/`export_bundle`) tests the whole `fleetd` state
  machine, **oracle phase + tier approval gate**, gate logic, oracle-tampering guard, caps,
  events, and control commands **without Docker or Claude.**
- Edge-coverage tests for every transition incl. oracle approval/reject, re-entry,
  timeouts, tampering, conflict.
- One **smoke test** runs the real `LocalDockerRunner` against a trivial repo end-to-end.
- The two **validation spikes** (cross-platform git escape; cost/token metering) are
  prerequisites.

---

## Section 7 — Cockpit UI (SP1 minimal)

One view: new-mission form (incl. **tier** selector); one unit card with phase badge, live
log, $/token/elapsed meters, findings list, a NEEDS_HUMAN/error alert with
halt/resume/abandon buttons, and the PR link at `PR_OPEN`. For T2/T3, an
**oracle-approval panel** at `AWAITING_ORACLE_APPROVAL` showing the proposed test set with
approve/edit/reject. No grid, no game skin.

---

## Open Questions (for the user)

0. ~~Containers vs. host clone.~~ **RESOLVED 2026-06-05: containerized.**
1. ~~Isolation backend / VM.~~ **RESOLVED 2026-06-05: Docker as the universal
   cross-platform backend** (VM-isolated on Win/Mac); gVisor/microVM as an optional
   Linux-only hardening backend behind the `Runner` trait.
2. ~~Model-API egress / dependencies.~~ **RESOLVED 2026-06-05: network OPEN.** The
   container is the host-damage boundary, not a network jail; exfil-via-model accepted as
   residual. Dep/egress trilemma and its spike deleted.
3. ~~SP1's honest scope.~~ **RESOLVED 2026-06-05: SP1 generates its own oracle.** Tests are
   generated in a separate oracle phase, frozen, builder-immutable, with **tier-gated human
   approval** (T1 autonomous / T2 human-approves tests / T3 human-approves tests + ships
   PR). The *rich* PRD/grill-me onboarding stays in SP2.
4. ~~Reviewer identity.~~ **RESOLVED 2026-06-05: the repo's `code-review` skill.**

_All SP1 open questions resolved. Remaining unknowns are the two validation spikes
(cross-platform git escape; cost/token metering), which are implementation prerequisites,
not design decisions._

---

## Design Critique Log

### Critique Round 1
Meta-flaw: "LOCKED" sections committed to a happy-path contract/state machine while
deferring load-bearing mechanisms. Resolutions: reframed the "objective" gate to be
evidence-based with tests as the only objective signal and honest about SP1's scope;
replaced `fetch_artifact` with a host-pushed model; added `MERGE_CHECK` + mergeability
verification; added human re-entry edges + stall/wall-clock timeouts; promoted the
credential/network boundary to a full section (no creds/egress in container, host-only
scoped token, pinned image); added the inbound REST command channel with `cmd_id` acks;
added `since`-replay/durability/backpressure/`error` event; added Docker preflight + USD
cap; trimmed Tier 2/3 and view-plugin/RemoteRunner framing; made `exec` streaming +
cancellable with health/resource limits.

### Critique Round 2
Found the Round-1 fixes were partly assertions, and two were platform-wrong. Resolutions:
- **Windows bind-mount `.git` is unsafe (#1–#3):** abandoned the NTFS bind-mount entirely.
  The isolated clone now lives in a **named Docker volume** (POSIX, in-VM); the branch
  escapes via **`git bundle` + `docker cp`** into the daemon's own host clone. The user's
  real repo is never touched; teardown can only damage a disposable volume. Added a
  **validation spike** to prove the Windows round-trip before building.
- **Egress proxy = theater + breaks builds (#4):** added an explicit **honest threat model**
  (boundary stops cred-theft/payload-fetch, NOT exfil-via-model); base provided by daemon
  (no git egress needed); build deps via **pre-warmed cache in the pinned image**; added a
  validation spike for egress-vs-build.
- **Gameable oracle — agent edits its own tests (#5):** added the **oracle-tampering
  guard** — any branch diff touching test files / runner config fails the gate →
  NEEDS_HUMAN.
- **NEEDS_HUMAN container preservation (#6):** decided — pause **tears down** the container
  (cost stops); the **named volume** is the durable state; `resume` re-provisions from the
  last commit; live conversation state is intentionally not preserved.
- **cmd_id durability / halt interleaving (#7):** defined halt as SIGTERM+grace (commit
  finishes in the disposable volume) then teardown; for one local unit a lost command is
  recovered by snapshot-refetch + re-issue (durable command journal deferred to SP3).
- **`seq` durability (#8):** since SP1 drops the durable log, `seq` is an in-memory counter
  and restart = snapshot refetch (consistent with the trimmed scope below).
- **Over-correction into a non-skeleton (#9):** **cut** durable event log, `since`-replay,
  `log_dropped` backpressure, the `takeover` verb (undefined), and the separate iteration
  cap — all deferred to SP3. **Kept** `MERGE_CHECK`, the credential boundary, halt, and
  USD/wall-clock/stall caps as genuinely core to the novel risk.
- **Orphaned-container cost on daemon death (#10):** added an **in-container self-kill
  watchdog** (daemon-independent wall-clock) and **label-based startup reconciliation**
  (`docker ps` by `cc.unit_id` → adopt or reap).

### Critique Round 3
Tested the Round-2 resolutions and found new issues, two of which are user-facing forks.
Resolutions:
- **Bundle prerequisites (#1):** mandated **complete, self-contained bundles** (verified
  via `git bundle verify` = no prerequisites); incremental bundles declared out of scope.
- **Dep-baking silently narrows scope (#2):** surfaced honestly as Open Question 4 —
  arbitrary-project + no-registry-egress + image-baked-deps is impossible; pick two
  (vendored-only, or a registry mirror in the allowlist).
- **Tampering guard forbade "add feature + its tests" (#3):** redefined — oracle = the
  **baseline test set pinned by hash at dispatch**; gate fails only on modify/delete of a
  baseline file; **adding new (passing) tests is allowed.**
- **USD cap unenforced when daemon dies (#4):** added a daemon-independent **token /
  `--max-turns` budget** in the in-container watchdog as a countable cost proxy; stated the
  honest caveat that USD-proper is only daemon-enforced.
- **`metric` source unproven (#5):** added **validation spike #3** (parseable Claude Code
  usage → cost); token budget becomes the primary cap if metering is unreliable.
- **`mergeable` is async/`null` + TOCTOU (#6):** added **polling** with bounded retries
  (`null`=retry, timeout→NEEDS_HUMAN), a `PR_OPEN → NEEDS_HUMAN` edge for `dirty` base, and
  noted GitHub's async result is authoritative over the advisory local trial-merge.
- **Resume before first commit (#7):** defined resume-from-base as a cold restart with a
  distinct `resumed_from_base_no_commit` event; added an end-of-build-round WIP commit so
  "last commit" is always meaningful.
- **No legitimate no-op terminal (#8):** added the **`NO_CHANGE`** terminal
  (`done{result:"no_change"}`) for green-checks-with-empty-diff.
- **Containers may be overkill for the skeleton (#9):** elevated to **Open Question 0** (the
  fork to settle first) with a recommendation to prove the spine host-side in SP1 and add
  container isolation as a fast follow; container design retained as the target.

**Status after 3 rounds:** the design is sound enough to present, but it is explicitly a
*hypothesis with a test plan* — Open Question 0 (containers vs. host) and validation spikes
must be resolved with the user before any implementation.

### Post-critique user grilling (2026-06-05)
After the 3 rounds, the user resolved every open question and made two scope-shaping calls:
- **Isolation reframed:** the container's job is **host-damage protection, not a network
  jail**. Network is now **OPEN** (deps install, oracle runs on arbitrary projects); exfil
  accepted as residual. Deleted the egress allowlist/proxy, the registry-mirror trilemma,
  and the egress validation spike.
- **Cross-platform mandate** (Win/Mac/Linux) → **Docker** is the universal isolation
  backend (VM-isolated on Win/Mac for free); gVisor/microVM is an optional Linux-only
  hardening backend behind the `Runner` trait.
- **Oracle pulled into SP1:** SP1 now **generates its own tests** (separate oracle phase,
  frozen/builder-immutable set, **tier-gated human approval** T1/T2/T3), keeping the bar
  objective via separation-of-powers. This enlarges the skeleton but captures the core of
  "useful autonomy." The *rich* onboarding UX (PRD/grill-me) remains SP2.
- **Reviewer = the repo's `code-review` skill.**

Two validation spikes remain as build prerequisites: cross-platform git escape, cost/token
metering. (The historical round logs above describe earlier states — e.g. "egress denied",
"tests supplied at dispatch" — that these post-critique decisions supersede.)
