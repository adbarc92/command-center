# SP-2a — Harness supervisor in `fleetd`, proved against `harness-fake`

> **Parent design:** NEXUS [`docs/specs/2026-09-14-swappable-harness-design.md`](https://github.com/adbarc92/nexus/blob/main/docs/specs/2026-09-14-swappable-harness-design.md)
> (v0.2), §2–§4, §6 and §9. ADR: NEXUS `docs/adr/0003-swappable-harness.md` (**accepted
> 2026-09-16**). Predecessor: SP-1 (#79, `848332e`) — `harness-protocol`, `harness-conformance`,
> `harness-fake`.
>
> Status: **v4 — 2026-09-16.** v1 approved in brainstorm; revised after three critique rounds (§10). Target
> rung: **L0**.

---

## 1. Why a slice, and what it proves

The parent design's SP-2 bundles four subsystems: the harness supervisor and caps ledger, the
evidence verifier, the extraction of `harness-claude-docker` (moving the 1,762-line `driver.rs`), and
generated cockpit types. That is too much for one plan, so SP-2 is split:

| Slice | Delivers | Depends on |
|---|---|---|
| **SP-2a (this spec)** | shared protocol monitor; `fleetd` harness registry, `GET /harnesses`, harness supervisor (caps, gates, halt/resume, failures) behind an `EvidenceVerifier` seam; **demo mode runs through `harness-fake`** | SP-1 |
| SP-2b | the real evidence verifier (parent §4 checks 1–5, including bundle import and the trial merge; the hash rule for an edited oracle) and the `fleet-runner` crate | SP-2a |
| SP-2c | `harness-claude-docker` extracted, real mode switched to it, in-process driver deleted; registry file format; kit `--work-order` + sandbox repo; generated cockpit types; `Health{docker, anthropic_key}` retired; Windows Job Object kill; post-result state surviving a restart | SP-2b |

**SP-2a proves:** a unit can run its whole life — admission, eligibility, gate, caps, halt, resume,
failure, result, verification, PR, `Done` — with the agent loop in a **separate process** speaking
the protocol, while `fleetd` alone owns the state machine. Demo mode is the first production caller,
so the cockpit exercises the new path end to end.

**Coexistence rule.** 2a adds the harness path **beside** today's in-process `driver::run`. Demo
units (`mode: "demo"`) move to the harness path; real units (`mode: "real"`) stay on `driver::run`
until SP-2c. `FakeRunner`, `driver.rs`'s unit tests and `tests/demo_mode_it.rs` (which calls
`driver::run` directly) stay as tests of the old path until 2c deletes it.

## 2. Scope

**In:**
1. `harness_protocol::monitor` — a pure, runtime-agnostic protocol monitor (§3.1), and the kit
   rebuilt on it.
2. `harness-fake` made a valid control-plane peer; harness obligations written down (§3.2).
3. `fleet-core`: `ErrorScope::Harness`; `(MergeCheck, OracleTampering) => NeedsHuman` (§4.8);
   `CapBreach` and `Stall` also valid (→ `NeedsHuman`) in `Provisioning`, `AwaitingOracleApproval`
   and `MergeCheck` (§4.5). The old driver never raises them there, so its behaviour is unchanged;
   `transition.rs:244`'s assertion is updated.
4. `fleetd`: harness registry and `GET /harnesses` (§3.3); `EvidenceVerifier` with
   `ScriptedVerifier` (§3.4); `harness_supervisor` (§4); persistence (§5); demo mode switched
   (§3.5).
5. Packaging: the cockpit sidecar script also builds and places `harness-fake` (§3.5).

**Out (and where it goes):** real verifier, bundle import, trial merge, edited-oracle hashing,
`fleet-runner` → 2b. Everything listed for 2c above → 2c. **Push delivery** → SP-4 (the first push
harness); 2a refuses it at eligibility. `POST /missions` `harness`/`opt_in`/`work_item` fields,
raising caps, repo allowlist, loopback auth → SP-3. Automatic retry after a verification failure →
not in v0. Cockpit changes → none required (§6 item 9 lists the visible differences).

## 3. Components

### 3.1 `harness_protocol::monitor`

A module in the existing `harness-protocol` crate — no new crate, no new dependency, no async, no
`fleet-core` import. The contract hash is over the schema only and does not change.

```rust
pub struct ProtocolMonitor { /* capabilities, start_id, pending interrupt, metric count, result_seen */ }

pub enum Inbound {
    StartAck,
    Event(UnitEvent),
    GateRequest { id: u64, request: GateRequest },
    InterruptAck { method: String },
    InterruptRefused { method: String },
    Result(UnitResult),
}

pub enum MonitorViolation {
    Malformed { line: String },
    InvalidMessage,
    UnknownMethod { method: String },
    InvalidParams { method: String, detail: String },
    InvalidResult(String),           // pr_open without evidence, failed without failure
    MessageAfterResult { method: String },
    MeteringDeclaredButSilent,
}

impl ProtocolMonitor {
    pub fn new(capabilities: Capabilities, start_id: u64) -> Self;
    /// Note an outbound interrupt (`unit/halt` | `unit/abandon`) so its reply classifies.
    pub fn interrupt_sent(&mut self, id: u64, method: &str);
    pub fn on_line(&mut self, line: Result<RpcMessage, String>) -> Result<Inbound, MonitorViolation>;
}
```

It owns **what** an inbound message means and whether it is legal on the wire. It does **not** own
time, and it does **not** own post-interrupt policy: after `interrupt_sent`, a `unit/result` is
still returned as `Inbound::Result`, and each caller decides — the kit reports
`InterruptNotHonored` (as `cases.rs:196-201` does today), the supervisor discards it (§4.3). It knows
nothing about phases.

`cases::drive` is rewritten on it; the kit keeps its own deadline and post-interrupt checks. The
kit's `Violation` gains this mapping, so **every existing kit test stays green unchanged**:

| `MonitorViolation` | kit `Violation` |
|---|---|
| `Malformed{line}` | `Malformed{line}` |
| `InvalidMessage` | `InvalidMessage` |
| `UnknownMethod{method}` | `UnknownMethod{method}` |
| `InvalidParams{method, detail}` | `InvalidResult("<method> params: <detail>")` (today's wording, `cases.rs:181-183`, `:202-204`) |
| `InvalidResult(s)` | `InvalidResult(s)` |
| `MessageAfterResult{method}` | `MessageAfterResult{method}` |
| `MeteringDeclaredButSilent` | `MeteringDeclaredButSilent` |
| `Inbound::InterruptRefused` (not a violation) | kit maps to `InterruptNotHonored{method}` (`cases.rs:172-178`) |

### 3.2 `harness-fake` and the harness obligations

A conformant harness must also be a valid peer for `fleetd`'s state machine. SP-1's kit does not
check observation order, and `harness-fake` today violates it (at T1 it never sends `oracle_frozen`,
the only exit from `Spec`, `transition.rs:73-79`). 2a writes these obligations into
`crates/harness-protocol/README.md` and makes `harness-fake` meet them:

1. **Order follows the unit state machine:** `provisioned` → `oracle_frozen` (**every tier**) →
   `build_finished` → `checks_passed` | `checks_failed` | `empty_diff` → … → `review_finished` →
   … → `unit/result`.
2. **At T2/T3, `gate/request` follows `oracle_frozen`** with only `log`, `metric`, `finding` or
   `error` in between. At T1 there is no gate.
3. **The review loop ends only when the gate is met:** `unit/result{pr_open}` only after a
   `review_finished` satisfying `fleet_core::gate_met` (`gate.rs:39-48`): `checks_green`, zero
   unresolved blockers, `round >= caps.min_review_rounds`, blockers not above the previous round's.
   Otherwise the harness builds again.
4. **Rounds count from 1 per process.** A resumed harness starts at round 1.
5. **After `empty_diff`** the harness sends only `log`, `metric`, `finding`, `artifact`, `error`, and
   then `unit/result{no_change | failed}`.
6. **On `resume{oracle_frozen: true}`** the harness sends `provisioned` and continues at building,
   with no `oracle_frozen` and no gate.
7. **`unit/abandon` must always be honoured**, as `harness-protocol/README.md` already requires;
   `unit/halt` only when the harness declares `halt: true`.
8. `unit/resume` exists on the wire but is **unused in v0**: resume is a respawn.

`harness-fake` changes: send `oracle_frozen` at every tier; loop build/check/review until rule 3
holds (one `metric` per cycle); declare `resume: true`; honour rule 6; write one startup line to
stderr (`harness-fake <version> starting`); new mode `HARNESS_FAKE_MODE=blockers` (round 1 reports
one blocker, later rounds zero). Kit still green in every existing mode and in `blockers`.

**Enforced by test, not by the kit:** a `fleetd` test replays the transcripts of the two
protocol-valid modes, `conformant` and `blockers` (T1–T3, fresh and resumed), through
`fleet_core::transition` and §4.2. The other modes break the protocol on purpose. A kit case for order would
need `fleet-core` in the kit; that waits until a third-party harness exists.

### 3.3 Harness registry and `GET /harnesses`

- **Types.** `Registry` holds `HarnessEntry{name, command: Vec<String>, env: Vec<(String,
  String)>}` plus each entry's probe state (`Unprobed | Healthy{harness, capabilities} |
  Unhealthy{error}`). Built in code; a file format arrives in 2c with the second harness.
- **Default.** `Registry::default_fake()` has one entry, `fake`, whose command is
  `harness-fake[.exe]` in the running executable's directory — or, when that directory is named
  `deps`, its parent. That covers the Tauri bundle (sidecars sit beside `fleetd-serve`), `cargo
  run`/`cargo build` (`target/<profile>/`), and test binaries (`target/<profile>/deps/`).
- **Construction.** `AppState::new(store, registry)` stays synchronous; `AppState::default()` uses
  `Registry::default_fake()`, unprobed. Tests that need a mode pass their own entry with `env`
  (`HARNESS_FAKE_MODE`, `HARNESS_FAKE_STEP_MS`) — no process-wide env mutation.
- **Probe.** `Registry::probe().await` spawns each entry, sends `initialize`, records the result,
  and closes stdin; a spawn error, a timeout (grace), a refused version or an invalid reply is
  `Unhealthy{error}`. `serve` calls it **before** `reconcile_on_startup`. `GET /harnesses` returns
  the stored state; no re-spawn per request.
- **Endpoint.** `GET /harnesses` → `[{name, state: "unprobed"|"healthy"|"unhealthy", harness?,
  capabilities?, error?}]`. `/health` is unchanged in 2a.
- **Use.** Admission refuses only on `Unhealthy`. Eligibility always uses the capabilities the
  **unit's own** `initialize` returned.
- **Dev loop.** `cargo run -p fleetd --bin serve` does not build `harness-fake`; the probe then
  reports `Unhealthy{"harness-fake not found at <path>; run cargo build -p harness-conformance
  --bins"}` and demo requests return 503 with that text.

### 3.4 `EvidenceVerifier`

```rust
pub struct VerifyInput<'a> {
    pub order: &'a WorkOrder,
    pub capabilities: &'a Capabilities,
    pub result: &'a UnitResult,
    /// The hash the human approved (T2/T3), from the store; `None` at T1.
    pub approved_oracle_hash: Option<&'a str>,
}

#[async_trait]
pub trait EvidenceVerifier: Send + Sync {
    async fn verify(&self, input: VerifyInput<'_>) -> Verdict;
}

pub enum Verdict {
    /// Evidence holds, the trial merge is clean, and the branch is ready in the host clone for
    /// `Forge::open_pr`.
    Verified,
    /// `no_change` confirmed empty.
    NoChange,
    Rejected { check: Check, detail: String },
    Tampered { detail: String },
    MergeConflict,
}
pub enum Check { Commit, Diff, Oracle, Tests, Link }
```

**The verifier owns all of parent §4's checks, including bundle import and the trial merge**, so a
merge conflict has exactly one source and the branch is ready before `open_pr`. The supervisor
calls only `Forge::open_pr` and `Forge::poll_mergeable`, never `Forge::trial_merge`.

2a ships only `ScriptedVerifier`: a queue of verdicts (default `Verified`) that also records every
`VerifyInput` it receives, so tests can assert what reached it. Demo mode's `harness-fake` evidence
(a fixed `head_sha`, a bundle file that does not exist) is therefore never read in 2a; the cockpit
already labels demo runs as simulated.

### 3.5 Demo mode and packaging

- `spawn_driver_for` and `rehydrate`: a unit whose `harness` column is set →
  `harness_supervisor::run` with that entry, a verifier from `AppState.verifier_factory` and
  `FakeForge::default()`; otherwise → `driver::run`, unchanged. `verifier_factory` is an
  `Arc<dyn Fn() -> Arc<dyn EvidenceVerifier> + Send + Sync>`, defaulting to
  `ScriptedVerifier::default`; tests replace it to queue verdicts and read recorded inputs. New demo
  units get `harness = "fake"`; existing demo rows are backfilled (§5).
- `opt_in` (§4.1) is derived, not stored: `mode == "demo"`. Nothing else is opted in in 2a.
- `POST /missions` and `POST /swarms` in demo mode return **503** with the stored error when `fake`
  is `Unhealthy`.
- `cockpit/ui/scripts/build-sidecar.mjs` also builds `-p harness-conformance --bin harness-fake`
  and places it as `src-tauri/binaries/harness-fake-<triple>[.exe]`; `tauri.conf.json` lists it in
  `externalBin`. The cockpit's sidecar supervisor is unchanged: `fleetd` spawns `harness-fake`, not
  Tauri.

## 4. The supervisor

`harness_supervisor::run(entry, verifier, forge, spec, ctx, commands, events) -> Phase` has the same
channels and return as `driver::run`, and a `RunCtx` extended per §5, so the server's broadcast and
WebSocket code are untouched. It is split in two:

- **`Supervisor<T: AsyncRead + AsyncWrite>`** — the logic, over any byte stream. Unit tests drive it
  with `tokio::io::duplex` and a scripted in-test peer.
- **`HarnessProcess`** — spawns an entry via `tokio::process` with `kill_on_drop(true)`
  (stdin/stdout piped; stderr forwarded line by line as `Log{stream: system}`), exposes the
  streams, captures the `ExitStatus`, and kills with `Child::kill`. If `fleetd` dies, the harness's
  stdin closes, which the protocol defines as "shut down"; a harness that ignores that waits for
  2c's Job Object.

**One loop.** Everything the supervisor does happens in one `tokio::select!` loop whose arms are:
an inbound harness line, a command, the stall and wall-clock timers, and at most one in-flight
**post-result call** (exit wait, `verify`, `open_pr`, the mergeability poll) held as a pinned,
cancellable future. Phase is mutated only inside the loop, one event at a time. A closed command
channel means "no more commands": while the harness is `Live` or a call is in flight the loop keeps
running (today's `demo_mode_it` drops its sender); once the unit is paused with nothing in flight,
it fails closed as today's driver does (`driver.rs:805-817`).

**Harness state.** Alongside the phase, the supervisor tracks `harness: None | Live | Finished`:
`Live` from spawn until `unit/result` or exit; `Finished` while **any post-result call is in
flight** — exit wait, `verify`, `open_pr` (including `Ship`'s), and the mergeability poll; `None`
otherwise. A resume's pre-transition spawn (§4.6) is `Live`.

**Work order.** `unit_id`, `tier`, `task`, `repo`, `branch`, `test_cmd` from the `UnitSpec`;
`caps{usd: usd_cap − cost, wall_clock_secs: wall_clock_secs − elapsed (or 0 when disabled),
min_review_rounds}` — both positive, by §4.6's resume check; `work_item` = `{kind: roadmap_item, ref:
"demo:<unit_id>"}` until SP-3; `resume` per §4.6.

**Per-spawn state.** The round count, the previous round's blockers, the recorded `empty_diff` and
the `Iteration` numbers reset on every spawn, as today's driver resets them per run
(`driver.rs:86-90`). The review floor therefore applies again after a resume.

### 4.1 Admission, spawn and eligibility

1. **Unchanged:** the global 24 h cap at `POST /missions`; `Queued` → `Start` → `Provisioning`; the
   concurrency permit acquired in `Provisioning`, shown as `Blocked{reason: "awaiting concurrency
   slot", cap: None, detail: ""}`.
2. **Spawn and `initialize`** in `Provisioning` (for a resume, earlier — §4.6). Failure, timeout
   (grace) or a refused version → `Error{scope: harness}` + `FatalError`.
3. **Eligibility** from the returned capabilities. Any refusal → `Error{scope: harness, detail:
   "harness <name> refused: <reason>"}`, kill, `FatalError`:
   - T2/T3 and `gates` lacks `oracle` → refused; **no opt-in overrides it**;
   - `isolation != container` or `metering == none`, without `opt_in` → refused, naming the
     guarantee;
   - `delivery == push` → refused (`"push delivery lands in SP-4"`).
4. **`unit/start`**; the `StartAck` must arrive within grace, else as step 2. **The clocks start at
   the `StartAck`.**
5. **`provisioned` must arrive within the stall window** after the `StartAck`, else kill +
   `FatalError` (`"no provisioned within <n>s"`).

### 4.2 Observations → triggers

| Harness sends | `fleetd` does |
|---|---|
| `observed{provisioned}` | `Trigger::Provisioned` (then §4.6's resume synthesis, if any) |
| `observed{oracle_frozen}` | T1: `Trigger::OracleFrozen` (→ `Building`). T2/T3: **recorded only**; the trigger is applied when the `gate/request` arrives (§4.4) |
| `observed{build_finished}` | `Iteration{build, n}`; `Trigger::BuildFinished` |
| `observed{checks_passed}` / `checks_failed` | `Iteration{check, n}`; `ChecksPassed` / `ChecksFailed` |
| `observed{empty_diff}` | legal only in `Checking`; **recorded, not applied** — the unit stays in `Checking` until §4.8 verifies the `no_change` result. From then on only `log`, `metric`, `finding`, `artifact`, `error` and `unit/result{no_change \| failed}` are legal |
| `observed{review_finished{round, unresolved_blockers, checks_green}}` | increment the per-spawn round count; `round` ≠ count → violation. `Iteration{review, count}`; `gate_met(GateConfig{min_review_rounds}, {count, unresolved_blockers, prev, checks_green})`; `ReviewFinished{gate_met}`; `prev = unresolved_blockers` |
| `metric{…}` | add to totals and check the USD cap; emit a cumulative `Event::Metric` (§4.5) |
| `log`, `finding`, `artifact` | forwarded as the matching `Event` |
| `error{scope, retryable, detail}` | forwarded as `Event::Error` (`harness` → `ErrorScope::Harness`); no transition by itself |

A trigger `transition()` rejects, or a message the rules above forbid → **protocol violation**
(§4.7). `fleetd` defers or synthesises a trigger in exactly three places: `OracleFrozen` at T2/T3
(§4.4), `EmptyDiff` (§4.8) and the resume synthesis (§4.6).

### 4.3 Draining

From the moment `fleetd` decides to stop a live harness (halt, abandon, a cap or stall breach) the
supervisor is **draining**: inbound observations, logs, findings, artifacts and gate requests are
logged at debug level and discarded; `metric` is still summed and emitted (the spend is real); a
`unit/result` is logged and discarded; `InterruptRefused` changes nothing (the sequence proceeds to
close stdin and kill after grace). Only the stop's own transition applies (§4.6).

### 4.4 The oracle gate

- At T2/T3, `gate/request{oracle{test_files, hash, summary}}` is legal only in `Spec` **with
  `oracle_frozen` recorded**. On arrival: emit `OracleProposed{test_files, hash, summary}`, then
  apply `OracleFrozen` (→ `AwaitingOracleApproval`), then mark the gate pending and **pause both
  clocks**. The cockpit's approval overlay (keyed on the phase, `store.svelte.ts:79-81`) therefore
  never opens before the gate exists and its files are known. At T1, or without `oracle_frozen`
  recorded, it is a violation.
- `ApproveOracle{cmd_id, edited_test_files: None}` → reply `{approved: true}`; apply
  `OracleApproved` with `cmd_id`; record the approved hash (§5); resume both clocks.
- `ApproveOracle` **with** `edited_test_files` → `Error{scope: system, detail: "edited oracle
  approval arrives in SP-2b"}`; the gate stays pending. (The cockpit never sends edits today.)
- `RejectOracle{cmd_id}` → reply `{approved: false}`; apply `OracleRejected` (→ `Spec`); clear the
  recorded `oracle_frozen`; resume both clocks. The harness then fails the unit or freezes and
  proposes again.
- `ApproveOracle`/`RejectOracle` with no gate pending → `Error{scope: system}`, no transition.
- `Halt`/`Abandon` while the gate is pending → §4.6; the gate is never answered.
- An unanswered gate waits without limit.

### 4.5 Caps, stall and metrics

- **USD.** Cumulative cost = the unit's stored cost + Σ `metric.cost_usd` this spawn; breach when
  it **exceeds** `usd_cap` (today's `>`, `driver.rs:221`).
- **Wall clock.** `fleetd`'s agent-time clock: runs from the `StartAck` while the harness is `Live`
  and no gate is pending; stops at `unit/result`; starts from the stored `elapsed_ms`.
  `wall_clock_secs == 0` disables it, as today. Breach when it exceeds `wall_clock_secs`.
- **Stall.** Runs under the same conditions as the wall clock; resets on every `unit/event`
  (stderr lines do not count). Fires after the stall window.
- **Defaults** (parent §9): grace 30 s, stall 600 s — `RunCtx` fields; `serve` reads
  `FLEETD_HARNESS_GRACE_SECS` and `FLEETD_HARNESS_STALL_SECS`.
- **Metrics out.** The supervisor emits a cumulative `Event::Metric{tokens_in, tokens_out,
  cost_usd, elapsed_ms}` for every inbound `metric` (draining included) **and** whenever a spawn
  ends (result, exit, kill, halt), so cost and agent time are persisted even for a harness that
  never meters.

**On a breach:** emit `Blocked{reason, cap, detail}` — `"usd cap"`/`usd`/`"spent $X of $Y"`,
`"wall-clock cap"`/`wall_clock`/`"ran Xs of Ys"`, `"stalled"`/none/`"no event for Xs"` — then run
the stop sequence (§4.6), then apply `CapBreach` or `Stall` (→ `NeedsHuman`). A breach can happen in
any phase where the harness is `Live`: the agent-active phases, and also `Provisioning`,
`AwaitingOracleApproval` (a `metric` while the gate is pending) and `MergeCheck` (the harness is
still bundling). `fleet-core` gains rows for the last three (§2), so every breach pauses the unit
rather than failing it. **One breach per stop:** while draining, metrics are summed and emitted but
never raise a second breach.

### 4.6 Commands, halt and resume

**Stop sequence** `stop(method)`: enter draining; send `unit/abandon` if `method` is abandon, or
`unit/halt` if the harness declared `halt: true`; close stdin; wait up to grace for exit; kill if
still running; emit the spawn-end metric; release the permit.

Commands are matched on **(phase, harness state)**:

| Command | When | Effect |
|---|---|---|
Rows are checked top to bottom; the first match wins.

| Command | When | Effect |
|---|---|---|
| `Abandon` | a stop is already in progress | the stop continues; when it completes, `FatalError` (`"abandoned"`) replaces the stop's own transition |
| `Abandon` | a resume's pre-transition spawn is `Live` (phase still `Halted`/`NeedsHuman`) | kill it; `Trigger::Abandon` (→ `Failed`) |
| `Abandon` | harness `Live` | `stop(abandon)`; `FatalError` (`"abandoned"`) |
| `Abandon` | harness `Finished` | drop the post-result future; `FatalError` (`"abandoned"`); a PR already opened is left as is |
| `Abandon` | phase `Queued`, or `Provisioning` waiting for a permit | drop the permit wait; `FatalError` (`"abandoned"`) |
| `Abandon` | phase `NeedsHuman` / `Halted` | `Trigger::Abandon` (→ `Failed`) |
| `Halt` | a stop is already in progress, or harness `Finished` | refused: `Error{scope: system, detail: "halt not available now; abandon instead"}` |
| `Halt` | harness `Live` (any non-terminal phase) | `stop(halt)`; `Trigger::Halt` (→ `Halted`) |
| `Halt` | phase `Queued`, `Provisioning` waiting for a permit, or `NeedsHuman` | drop any permit wait; `Trigger::Halt` (→ `Halted`) |
| `Resume` | phase `Halted` or `NeedsHuman`, harness `None` | see below |
| `Ship` | phase `NeedsHuman` **awaiting Ship**, harness `None` | the harness becomes `Finished`; `Trigger::Ship` (→ `PrOpen`), then `forge.open_pr` and the §4.8 poll, as today's driver orders them |
| `ApproveOracle` / `RejectOracle` | — | §4.4 |
| anything else | — | `Error{scope: system, detail: "command <id> not valid in <phase>"}`, no transition |

**Resume.** While the unit is still paused and holding no permit:
1. **Caps check.** If the stored cost has reached `usd_cap`, or `wall_clock_secs > 0` and the
   stored `elapsed_ms` has reached it → `Error{scope: system, detail: "<cap> exhausted; raising
   caps arrives in SP-3"}`, **no transition**.
2. **Spawn and `initialize`**, then check `resume: true` and eligibility. Any failure → error, kill,
   **no transition** — the unit stays paused and can be resumed again or abandoned.
3. `Trigger::Resume` (→ `Provisioning`); acquire the permit (the idle harness waits; no clock runs
   before `StartAck`); `unit/start` with `resume{oracle_frozen: R}`, where

   > `R` = an approved oracle hash is stored (T2/T3), or `oracle_frozen` is stored (T1).

   A T2/T3 unit halted before or during its gate, after a rejection, or after tampering (which
   clears the approval, §5) gets a fresh freeze and a fresh gate.
4. When `R` is true, on `observed{provisioned}` `fleetd` applies `OracleFrozen` (reason `"oracle
   already frozen"`) and, at T2/T3, `OracleApproved` (reason `"approved in a prior run"`). At T2/T3
   the unit passes through `AwaitingOracleApproval` for one event, which can flash the cockpit's
   approval overlay; accepted in 2a (§6 item 9).

Failures after step 3 follow §4.1 and §4.7 (the unit is then `Failed`).

**Awaiting Ship.** Entered at T3 after a `Verified` verdict; announced with `Blocked{reason:
"awaiting ship", cap: None, detail: "T3: a human ships the PR"}`. `Resume` from it is allowed (it
reruns the harness, as today's driver does); `Ship` is allowed **only** from it. The flag is in
memory: a daemon restart halts every non-terminal unit (`reconcile.rs:18-27`), so the unit comes
back `Halted` and must be resumed — today's behaviour; persisting it is 2c's.

**Known defect in the old path, not fixed there:** `driver.rs:441` re-applies `OracleFrozen` on
resume even when the gate was never approved, parking a T2/T3 unit in `AwaitingOracleApproval` with
nothing to approve. The new path avoids it; the old driver is deleted in 2c. Filed as an issue when
this spec merges.

### 4.7 Failures

| Condition | Result |
|---|---|
| Spawn, `initialize`, start ack, `provisioned` timeout or eligibility fails (outside a resume's pre-transition steps) | `Error{scope: harness}`; kill; `FatalError` (→ `Failed`) |
| Monitor violation; a §4.2 or §4.4 rule broken; round mismatch; a `unit/result` in the wrong state (§4.8) | `Error{scope: harness, detail: "protocol violation: <kind>: <raw line, truncated to 2 KB>"}`; kill; `FatalError` |
| stdout EOF without `unit/result`, not draining | `Error{scope: harness, detail: "exited without result (<ExitStatus>)"}`; `FatalError`. **Terminal in v0** — a deliberate deviation from parent §4, which lets a resumable harness be resumed (see §7) |
| `unit/result{failed, failure}` | `Error{scope: failure.scope, detail}`; `FatalError` |
| No exit within grace after `unit/result` | kill; `Error{scope: harness, retryable: false}`; the result still stands |

### 4.8 Result, verification and PR

A `unit/result` is legal only as follows; otherwise it is a violation:

| Outcome | Required state | Then |
|---|---|---|
| `pr_open` | phase `MergeCheck` | `evidence.branch` must equal the assigned branch and `evidence.delivery` must be `bundle`, else `Rejected{Link}` without calling the verifier; then verify |
| `no_change` | phase `Checking` with `empty_diff` recorded | verify |
| `failed` | any non-terminal | §4.7 |

On a legal result the harness becomes `Finished`, the clocks stop, and the post-result future runs:
close stdin → wait for exit (§4.7 last row) → release the permit → `verify` → act on the verdict.

| Verdict | On `pr_open` (in `MergeCheck`) | On `no_change` (in `Checking`) |
|---|---|---|
| `Verified` | `MergeClean` (T1/T2 → `PrOpen`; T3 → `NeedsHuman`, awaiting Ship). At T1/T2, then `forge.open_pr(branch)` and `Artifact{pr}` — transition first, as today's driver does (`driver.rs:686-705`) | `Error{scope: system, detail: "no_change verified as non-empty"}`; `FatalError` |
| `NoChange` | `Error{scope: system, detail: "pr_open verified as empty"}`; `FatalError` | `EmptyDiff` (→ `NoChange`); `Done{result: "no_change"}` |
| `Rejected{check, detail}` | `Error{scope: system, detail: "evidence rejected: <check>: <detail>"}`; `FatalError`; no retry | same |
| `Tampered{detail}` | `OracleTampering` (→ `NeedsHuman`, reason `"oracle tampering"`) — **new `fleet-core` row** `(MergeCheck, OracleTampering) => NeedsHuman`, with a test | same trigger (`Checking` is agent-active) |
| `MergeConflict` | `MergeConflict` (→ `NeedsHuman`) | `FatalError` (a conflict with no change is impossible) |

`open_pr` failing → `Error{scope: github}`; `FatalError`. In `PrOpen`, poll `forge.poll_mergeable`
exactly as `driver.rs:766-790` does today (`MAX_MERGEABLE_POLLS`, no delay between polls):
`Mergeable` → `PrMergeable` (→ `Done`), `Dirty` → `PrDirty`, polls exhausted → as today. A
`NeedsHuman` reached from `PrOpen` is not awaiting Ship, so `Ship` no longer re-polls it (today's
driver allows that); `Resume` reruns the harness. Terminal events keep today's strings:
`Done{result: "done" | "no_change" | "failed"}` (`driver.rs:743-761`).

## 5. Persistence

The forwarder (`server.rs:509-546`) remains the only store writer for a running unit; the supervisor
communicates only through events.

**Store API.** `update_unit`'s `COALESCE`/`OR` columns cannot express a clear, so the forwarder
moves to a new `Store::write_projection(unit_id, &Projection, now)` that writes every projection
column as given. `Projection{phase, cost, last_seq, terminal_reason, oracle_hash, oracle_frozen,
approved_oracle_hash, elapsed_ms}` is **seeded from the stored row** when the forwarder starts —
except `terminal_reason`, which starts `None` as today, so startup reconcile's `"daemon restarted"`
(`server.rs:915-923`) is cleared by the first event. Today the forwarder starts cost at `0.0`
(`server.rs:516-519`), so a resumed unit that failed before its first `metric` wrote `cost = 0` and
dropped its spend from the 24 h ledger; seeding fixes that for both paths. `update_unit` stays for
its other callers.

**Folds** (applied in the forwarder, in event order). Rows marked **H** apply only when the unit's
`harness` column is set: the old driver emits the same phase pairs (`driver.rs:443`, `:484`, `:492`,
`:495`, `:566`) with different meaning — it re-enters the gate without a new `OracleProposed`, and
hashes with `DefaultHasher` — so the old path keeps today's writes exactly.

| Event | Projection change |
|---|---|
| `PhaseChanged{to}` | `phase` |
| `Metric` | `cost = cost_usd`; `elapsed_ms = elapsed_ms` (both cumulative) |
| `Done{result}` | `terminal_reason` |
| `OracleProposed{hash}` | `oracle_hash = hash`; `oracle_frozen = true` (unchanged behaviour) |
| **H** `PhaseChanged{from: AwaitingOracleApproval, to: Building}` | `approved_oracle_hash = oracle_hash` |
| **H** `PhaseChanged{from: AwaitingOracleApproval, to: Spec}` (rejected) | `oracle_hash = None`, `oracle_frozen = false` |
| **H** `PhaseChanged{from: Spec, to: Building}` (T1 freeze) | `oracle_frozen = true` |
| **H** `PhaseChanged{to: NeedsHuman, reason: "oracle tampering"}` | `approved_oracle_hash = None` |

The reason string is a shared `fleetd` constant used by the supervisor and the fold. The forwarder
reads the `harness` column when it seeds.

**Schema.** New columns, added by `Store::init`'s existing ignore-if-exists `ALTER TABLE` list
(`store.rs:72-85`): `harness TEXT`, `approved_oracle_hash TEXT`, `elapsed_ms INTEGER NOT NULL
DEFAULT 0`. After the `ALTER`s, `init` runs the idempotent backfill `UPDATE units SET harness =
'fake' WHERE mode = 'demo' AND harness IS NULL` every time. `UnitRow`, `upsert_unit`, `map_row`,
the `SELECT` column lists and `row_from_spec` gain the three fields. Migrated demo T2/T3 rows have no
approved hash, so they get a fresh gate on their next resume.

Test fixtures that insert demo rows directly (`building_row`, `server.rs:1400`; the rehydrate-demo
test, `:1690`) set `harness: Some("fake")` explicitly, because the backfill has already run when
they insert.

`RunCtx` gains `harness`, `oracle_frozen`, `approved_oracle_hash`, `elapsed_ms`, `grace` and
`stall`; `rehydrate` fills them from the row. The approved hash reaches the verifier through
`VerifyInput` (§3.4).

## 6. Testing

Red first for every behaviour row above. All new code is in the root workspace, so `cargo test
(workspace)` covers it; `harness conformance` stays a separate required check. CI runs on Linux only
(`ci.yml`); Windows behaviour is covered by the developer's local `cargo test --workspace` and the
packaged smoke (§8).

**Binary location.** `Registry::default_fake()` resolves `harness-fake` from `target/<profile>/`
when run from `deps/`. `cargo test --workspace` builds it before any test runs (`harness-conformance`
has integration tests); a test that cannot find it fails with the build command in its message.

1. **Monitor:** one test per `Inbound` and per `MonitorViolation`; a `unit/result` after
   `interrupt_sent` is still `Inbound::Result`. The kit's existing tests pass unchanged on the
   rebuilt `drive` through the §3.1 mapping.
2. **`harness-fake`:** conforms in `conformant` and `blockers` modes; sends `oracle_frozen` at T1;
   loops until the gate predicate holds (`blockers`; `min_review_rounds` 1 and 3); skips the gate on
   `resume{oracle_frozen: true}`; writes its startup line to stderr. **Transcript replay:** the
   `conformant` and `blockers` transcripts, T1–T3, fresh and resumed, are valid under
   `fleet_core::transition` and §4.2.
3. **`fleet-core`:** `(MergeCheck, OracleTampering) => NeedsHuman`; `CapBreach` and `Stall` →
   `NeedsHuman` in `Provisioning`, `AwaitingOracleApproval` and `MergeCheck`
   (`transition.rs:244` updated); other table tests unchanged.
4. **Supervisor over `duplex`**, each with a scripted peer:
   - happy path T1, T2, T3 (T3: `Blocked{"awaiting ship"}` → `Ship` → `PrOpen` → `Done`);
   - eligibility: T2 on a gateless harness refused **with** `opt_in`; host-isolated refused without
     `opt_in`, admitted with it; push delivery refused;
   - the T2 gate: `oracle_frozen` alone leaves the unit in `Spec`; `gate/request` emits
     `OracleProposed` before `PhaseChanged`; a `metric` between them that breaches the USD cap →
     `NeedsHuman` (from `Spec`); `ApproveOracle` before the gate → error, no transition; approval
     with edits → error, gate still pending; `gate/request` without `oracle_frozen` → violation;
   - startup failures → `Failed`: `initialize` timeout, refused version, no `StartAck`;
   - caps: USD breach from incremental `metric`s summed on the stored cost; a breach in
     `Provisioning`, in `AwaitingOracleApproval` (metric while the gate is pending) and in
     `MergeCheck` (harness still live) → `NeedsHuman`; a wall-clock breach → `NeedsHuman` with its
     `Blocked` detail; metrics while draining never raise a second breach; wall clock paused while a
     gate is pending; no clock before
     `StartAck` (a harness idle behind a held permit does not stall); no `provisioned` within the
     window → `Failed`; stall in `Building` → `NeedsHuman`; clocks stop at `unit/result` (a slow
     scripted verifier does not stall);
   - an unmetered, opted-in harness: a spawn-end `Metric` carries `elapsed_ms`;
   - violations → `Failed`: gate at T1; skipped round; `checks_passed` in `Building`;
     `unit/result{pr_open}` in `Building`; `checks_passed` after `empty_diff`;
   - exit without result → `Failed` with the exit status; `unit/result{failed}` → `Failed` with the
     failure's scope; no exit within grace after a `pr_open` result → killed, the result still
     verified;
   - draining: a harness that keeps emitting after `unit/halt` → `Halted`, its metrics still summed;
     a result sent after `unit/halt` discarded; `InterruptRefused` → killed after grace → `Halted`;
     `halt: false` harness → no `unit/halt` sent, stdin closed, `Halted`; `Abandon` always sends
     `unit/abandon`; a `gate/request` while draining is not answered;
   - resume: halt during a pending gate → resume → fresh gate; reject → halt → resume → fresh gate;
     approve → halt in `Building` → resume → no gate, both triggers synthesised, round count
     restarts at 1; tampered → `NeedsHuman` → resume → fresh gate; resume on a `resume: false`
     harness, a spawn failure, or an exhausted cap → error, stays paused;
   - the command matrix: every row of §4.6, including `Halt`/`Abandon` for a unit waiting for a
     permit, `Abandon` during a `stop(halt)` (ends `Failed`), `Abandon` during a resume's
     pre-transition spawn, `Halt`, `Resume` and `Ship` refused while `Ship`'s `open_pr` is in flight,
     `Abandon` while `Finished` cancels the post-result call, `Ship` outside awaiting Ship refused;
     a closed command channel keeps a live unit running and fails a paused one closed;
   - verdicts: `pr_open` × each verdict; wrong `evidence.branch` or `delivery: push` → `Failed`
     without calling the verifier (asserted through `ScriptedVerifier`'s recorded inputs); the
     approved hash reaches `VerifyInput`; `open_pr` failure → `Failed`; `PrDirty` and exhausted
     polls → `NeedsHuman`, where `Ship` is refused;
   - `no_change`: `empty_diff` keeps the unit in `Checking`; `Verdict::NoChange` → `NoChange`; any
     other verdict → `Failed`;
   - `blockers` script: round 1 not met → `Building`; round 2 met.
5. **Process shell:** runs the real `harness-fake` T1 and T2 to `Done`; a `hang`-mode entry, with a
   short stall window, is stopped and killed after grace; the startup stderr line arrives as a log;
   dropping the supervisor task reaps the child.
6. **Registry:** `default_fake()` resolution from an exe dir and from `deps/`; probe →
   `Unhealthy` for a missing binary with the build hint; `GET /harnesses` shape for all three states.
7. **Persistence:** each fold row of §5; **H** rows do not fire for a unit with no `harness` (an
   old-path T2 reject → approve → restart keeps its `oracle_hash`, as today); the seeded projection
   keeps a resumed unit's cost when it fails before any metric and clears `"daemon restarted"`; the
   backfill runs idempotently on every `init`.
8. **Server:** `POST /missions` demo → `Done` through the real `harness-fake`; the swarm demo and
   rehydrate-demo tests pass on the new path; `rehydrate_reloads_oracle_hash_so_tamper_gate_rearms`
   (`server.rs:1736-1790`), which relies on the old driver's own oracle re-hash, is **rewritten**:
   through a test `verifier_factory`, a rehydrated demo unit hands the stored approved hash to the
   verifier, and a scripted `Tampered` verdict routes it to `NeedsHuman`; demo `POST /missions` and `POST /swarms` return 503 when `fake`
   is `Unhealthy`; real mode untouched. `tests/demo_mode_it.rs` is unchanged (it tests the old path).
9. **Cockpit:** no code change; vitest and `svelte-check` stay green (`types.ts` types `scope` as
   `string`). Visible differences, accepted for 2a: `SHIP` on a non-awaiting `needs_human` unit now
   returns an error event instead of opening a PR or re-polling; the new `Blocked` reasons stay on
   the tile until the next blocked event (existing behaviour, `fleet.ts:139-143`); a resume after an
   approved T2/T3 gate passes through `awaiting_oracle_approval` for one event, which can flash the
   approval overlay.

## 7. Decisions settled here

| Item | Decision |
|---|---|
| Registry location and format (parent §9) | 2a: a code-built `Registry`; default `fake` beside the executable (or `deps/`'s parent). File format: 2c |
| Grace and stall (parent §9) | 30 s and 600 s; `RunCtx` fields, env-overridable in `serve` |
| Process-tree kill on Windows (parent §9) | 2c; 2a relies on `kill_on_drop` and the protocol's stdin-close rule |
| Runner crate name (parent §9) | `fleet-runner`, created in 2b |
| Type generation (parent §9) | 2c |
| **Deviation:** exit without result | terminal (`Failed`) in v0; parent §4 allows a human resume for a resumable harness, but `fleet-core` has no transition out of `Failed`. Revisit if 2c's real harness needs it |
| **Deviation:** `Ship` | only from awaiting Ship; today's driver accepts it in any `NeedsHuman`, including as a re-poll after `PrDirty` |
| **Change:** `CapBreach`/`Stall` validity | extended to `Provisioning`, `AwaitingOracleApproval`, `MergeCheck`, so a breach always pauses rather than fails |
| `GET /harnesses` | kept: cheap, and the cockpit's harness picker (SP-3) needs it |
| `unit/resume` | unused in v0 |
| Edited oracle approval | refused in 2a; 2b defines the approved hash for edited files |

## 8. Risks

- **Two run paths until 2c.** Demo and real diverge in code. Mitigation: the new path has the
  stricter tests, and 2c deletes the old one.
- **Sidecar packaging.** A bundle without `harness-fake` loses demo mode. Mitigation: the startup
  probe and the 503; the next packaged smoke checks it.
- **Windows is untested in CI.** Pipe, `.exe` resolution and kill behaviour run only locally.
  Mitigation: the plan's verification step runs `cargo test --workspace` on Windows, and the
  packaged smoke exercises demo mode.
- **Harness obligations beyond the kit.** A third-party harness can pass the kit and still break
  §3.2's order rules. Mitigation: the rules are written down, and `fleetd` fails such a unit loudly
  with the raw message. A kit case follows when a third party exists.

## 9. Glossary for this spec

- **Live / Finished / None** — the supervisor's harness state (§4).
- **Draining** — the stop-in-progress mode (§4.3).
- **Awaiting Ship** — a T3 `NeedsHuman` after a clean verification (§4.6).
- **R** — the `resume.oracle_frozen` value sent on respawn (§4.6).

## 10. Design Critique Log

### Critique Round 1

An independent reviewer checked v1 against `fleet-core`, `fleetd` and SP-1's code. Findings and
resolutions:

- **Critical — T1 could never finish through `harness-fake`** (it never sends `oracle_frozen` at
  T1, the only exit from `Spec`). → obligations written down; `harness-fake` sends it at every tier;
  transcript-replay test (§3.2).
- **Critical — `Tampered` in `MergeCheck` was an invalid transition.** → new `fleet-core` row (§2,
  §4.8).
- **Critical — resume deadlock for a T2/T3 unit halted during or after a rejected gate.** →
  `resume.oracle_frozen` keyed on the approval (§4.6, §5).
- **Critical — `no_change` became terminal before verification.** → `empty_diff` recorded, not
  applied (§4.2, §4.8).
- **Major — in-flight messages during a halt turned a pause into `Failed`.** → draining (§4.3).
- **Major — cap and stall triggers invalid outside agent-active phases.** → `FatalError` there
  (§4.5).
- **Major — fixed N review cycles could not satisfy the gate in `blockers` mode.** → loop until the
  predicate (§3.2).
- **Major — delivery contract incoherent.** → verifier owns the trial merge; push refused until
  SP-4 (§3.4, §4.1).
- **Major — per-spawn counters undefined on resume.** → reset per spawn (§4).
- **Major — persistence did not fit the event-only channel; a false claim about `oracle_frozen`;
  the forwarder's zeroed cost.** → §5 rebuilt on folds; claim removed; projection seeded.
- **Major — T3 Ship and post-result commands undefined.** → awaiting-Ship state; restart behaviour
  stated as today's (§4.6).
- **Major — default binary path wrong under `cargo test`.** → `deps/` rule (§3.3).
- **Major — resume checked capabilities before spawning; eligibility placed in `Queued`.** →
  reordered (§4.1, §4.6).
- **Minor — contract drift** (`Done` strings, `>`, `wall_clock_secs = 0`, `types.ts`, stall
  definition, `halt: false`, swarm 503, `kill_on_drop`). → aligned with today's code.
- **Scope — cut:** registry file and TTL re-probe, kit `--work-order` (→ 2c), `Blocked
  "unmetered"`, duplicate-gate-reply tracking.

### Critique Round 2

A fresh reviewer checked v2 against the code, the cockpit and CI. Findings and resolutions:

- **Critical — resume after a wall-clock breach failed the unit, and a resume queued for a permit
  could stall or breach in `Provisioning`; "same as today" was false** (today's driver resets its
  clock per run). → clocks start at `StartAck`; resume refuses an exhausted cap with no transition;
  the misleading claim removed (§4.1, §4.5, §4.6).
- **Major — `AwaitingOracleApproval` was entered before the gate existed**, racing the cockpit's
  phase-keyed approval overlay and making a metric in that window fatal. → at T2/T3 `OracleFrozen`
  is applied together with `gate/request`, after `OracleProposed` (§3.2 rule 2, §4.4).
- **Major — command matrix keyed on phase alone; `MergeCheck` is entered while the harness is
  still live; commands during awaited calls undefined.** → matrix keyed on (phase, harness state);
  missing rows added; post-result work is a cancellable `select!` arm; clocks stop at the result
  (§4, §4.6).
- **Major — cost and agent time persisted only through inbound metrics.** → spawn-end and draining
  metrics (§4.5).
- **Major — rehydrate routing and existing tests contradicted each other; the verifier had no
  approved hash.** → route on the `harness` column with a demo backfill; the tamper-rearm test
  rewritten, `demo_mode_it` kept for the old path; `VerifyInput.approved_oracle_hash` (§3.4, §3.5,
  §6).
- **Major — the folds could not be built on `update_unit`, whose `COALESCE` cannot clear.** →
  `write_projection` with a seeded `Projection` (§5).
- **Major — resume after tampering looped on the same tests; a spawn failure on resume failed a
  paused unit.** → tampering clears the approval; every pre-transition resume failure leaves the
  unit paused (§4.6, §5).
- **Major — monitor versus kit on a result after an interrupt; hidden error remaps; `unit/abandon`
  gated on `halt`.** → post-interrupt policy belongs to callers; mapping table; abandon always sent
  (§3.1, §3.2, §4.6).
- **Major — the recorded `empty_diff` was never invalidated.** → only closing messages are legal
  after it; reset per spawn (§3.2 rule 5, §4.2).
- **Minor — `Verified{bundle}` unused; the poll "interval" does not exist.** → `Verified` carries no
  path, the verifier leaves the branch ready; "no delay, as today" (§3.4, §4.8).
- **Minor — registry lacked per-entry env, probe timing and `Default`; closed command channel;
  Linux-only CI; dev loop without `harness-fake`.** → all specified (§3.3, §4, §6, §8).
- **Minor — silent cockpit changes and missing `Blocked.detail`.** → `Blocked{"awaiting ship"}`;
  details given; visible differences listed (§4.5, §4.6, §6 item 9); deviations recorded (§7).

### Critique Round 3

A third reviewer walked the (phase × harness state × input) space of v3 against `transition()` and
checked implementability. **No critical findings.** Findings and resolutions:

- **Major — breaches were also invalid in `AwaitingOracleApproval` and `MergeCheck`** (a metric
  during a pending gate; a stall while the harness is still bundling), not only in `Provisioning`,
  so they failed the unit as a "protocol violation". → `fleet-core` accepts `CapBreach`/`Stall` in
  all three; one breach per stop (§2, §4.5, §7).
- **Major — the §5 folds would have changed stored columns for real units**; the old driver emits
  the same phase pairs (`driver.rs:443`, `:484`, `:492`, `:495`, `:566`), and a real T2
  reject → approve → restart would have lost its tamper check. → the four new folds apply only to
  harness-path units; a regression test pins the old path (§5, §6 item 7).
- **Major — `Ship`'s `open_pr` and the mergeability poll ran with harness state `None`**, so `Halt`
  or `Resume` could race them; the T1/T2 order around `MergeClean` was ambiguous. → `Finished`
  covers every post-result call; the order is transition, then `open_pr`, as today (§4, §4.6, §4.8).
- **Major — a unit waiting for a permit could be neither halted nor abandoned.** → rows for
  `Provisioning` waiting on a permit (§4.6).
- **Major — the rewritten tamper-rearm server test could not inject a verifier.** →
  `AppState.verifier_factory` (§3.5, §6 item 8).
- **Minor — routing and backfill not buildable as written** (`UnitRow` fields, `init`'s
  ignore-error migrations run before test inserts, no `opt_in` source). → fields listed; idempotent
  backfill on every `init`; fixtures set `harness`; `opt_in` derived from `mode` (§3.5, §5).
- **Minor — the seeded projection would keep `"daemon restarted"`.** → `terminal_reason` not seeded
  (§5).
- **Minor — closed command channel in a paused unit.** → fails closed, as today (§4).
- **Minor — unwritable tests** (replay of deliberately broken modes; no stderr output from
  `harness-fake`; `hang` never killed without a stop; `caps.usd` could be zero). → replay limited to
  valid modes; startup stderr line; short stall window; resume check uses "reached" (§3.2, §4.6,
  §6).
- **Minor — behaviour rows without tests** (startup timeouts, post-result exit grace, failure scope,
  push delivery, `open_pr` failure, `PrDirty`, wall-clock breach, gate while draining, `Abandon`
  ordering). → each added to §6; `Abandon` precedence made explicit (first match wins, §4.6).
- **Scope — nothing removable;** `GET /harnesses` kept for SP-3's picker. The one-event overlay
  flash on resume is accepted and listed (§4.6, §6 item 9, §7).
