# Factory M1: the control plane — Implementation Plan

> **For agentic workers:** steps use checkbox (`- [ ]`) syntax for tracking. This plan has three
> parts. **Part 1 ("wave 0")** is executed first, by one agent alone: it writes every crate's
> interface, an in-memory fake of every seam and a contract suite for each, as code that is
> printed here in full. **Part 2** has one section per lane; each lane is executed by one agent,
> in its own git worktree, after Part 1 has merged, and ends with a pull request that the agent
> does not merge. A lane is given its interface and its tests in full and writes the
> implementation itself. **Part 3** is the order the lanes merge in, the end-to-end scenarios and
> the checklist for the watched proof.

**Goal:** Make one real unit work end to end through `command-center`: a person signs a spec in a
sandbox repository, dispatches it through the API, the `reqdrive` harness runs it in a child
process, `fleetd` verifies the result with its own runs, and a pull request is opened.

**Architecture:** `fleetd` stops running the coding agent in-process. Seven library crates take
over its parts (`fleet-store`, `fleet-forge`, `fleet-runner`, `fleet-verify`, `fleet-supervisor`,
`fleet-admission`, `fleet-api`); each owns one seam, ships a fake of it and a contract suite for
it, and reaches the others only through traits, so every crate is built and tested alone. The
`fleetd` binary becomes wiring: it picks the real implementations, joins the crates' ports, and
starts a harness process per unit, which it speaks to over stdin and stdout in JSON-RPC (harness
protocol 0.2). The control plane keeps the state machine, admission, the caps, the human gates
and the decision that a unit is done, which it takes only from evidence it checked itself.

**Tech Stack:** Rust 2021 (toolchain 1.93); `tokio` 1; `axum` 0.7 with WebSockets; `rusqlite`
0.32 (bundled SQLite); `serde`, `serde_json`, `schemars` 1.2, `toml` 1, `sha2` 0.10, `thiserror`
2, `async-trait`; `tower-http` 0.6 (CORS); `windows-sys` 0.61 on Windows (Job Objects); host
`git`, `gh` and `docker` reached by running the programs; `tempfile`, `tower`,
`tokio-tungstenite` and `reqwest` in tests only.

**Spec:** The ReqDrive factory design v0.4, in the private nexus repository: https://github.com/adbarc92/nexus/blob/main/docs/specs/2026-10-04-reqdrive-factory-design.md

The supervisor's behaviour is specified in this repository, in
`docs/superpowers/specs/2026-09-16-sp2a-harness-supervisor-design.md` ("the supervisor spec"
below; it arrives with pull request #96). This plan cites its section numbers. Where milestone 1
departs from it, the departure is listed under [Lane CC-SUPERVISOR](#lane-cc-supervisor).

**Words used here.** A *unit* is one piece of work run for one signed spec. A *harness* is the
child process that runs a unit's agents. A *seam* is a trait one crate offers and another calls.
A *fake* is an in-memory implementation of a seam, scriptable from a test. A *contract suite*
is a public function that runs a fixed set of assertions against any implementation of a seam.
A *port* is a trait a crate defines for something it needs and `fleetd` implements. The
*driver path* is today's in-process agent loop (`crates/fleetd/src/driver.rs`); the *harness
path* is the new one. A *Form* is the written contract of one crate (`forms/README.md`).

## Global Constraints

These are decisions already made. A lane follows them and does not reopen them.

**Repository and conduct**

- This repository is public. The text of the private design and of the doctrine stays out of
  it.
  Cite doctrine principles by id only (for example C1, D15, K4).
- Work only in the worktree your part or lane names. The checkout at
  `D:\MajorProjects\INFRASTRUCTURE\command-center` holds the owner's uncommitted work: make
  no edit, commit, stash or branch switch there. `git -C <that path> fetch`, pushing a new
  branch and `worktree add` are allowed, because they do not touch its working tree.
- Write only the files under your lane's **Owns**. If you need a change anywhere else, ask the
  coordinator for it in your final report. No lane edits the root `Cargo.toml`, `Cargo.lock`,
  any crate's `Cargo.toml`, `.github/**`, `xtask/**`, `forms/registry.md`, `forms/README.md`,
  a roadmap file, `CLAUDE.md` or `docs/STATUS.md`.
- No commit message and no pull-request body has a `Co-Authored-By` line or a "Generated
  with" footer. Never force-push. Never push to `main`. Never merge your own pull request.
- Spend nothing and reach nothing live: no model call, no deploy, no real API key. The only
  network use is `cargo` fetching already-locked crates, pushing your branch, and opening your
  pull request.
- Test first (C1): each behaviour has a failing test, run and seen to fail for the reason this
  plan states, before the code that passes it.
- Before claiming a lane is done, run every command under its **Verify** and report the real
  output, failures included. Before parsing a tool's output, check that it is not empty.
- The build host is Windows 11 with PowerShell 7 and Git-Bash. A command block marked
  *Git-Bash* uses POSIX syntax. Unmarked `git`, `cargo`, `gh` commands work in either shell.
  Create and edit files with your file tool, never by typing a here-document into a command
  line: quoting differs between the shells and mangles backslashes.

**Names are fixed**

- Everything milestone 0 delivers keeps its name: the protocol types, `ProtocolMonitor`,
  `Inbound`, `MonitorViolation`, `file_sha256`, `bundle_hash`, `negotiate`, `MAX_LINE_BYTES`
  (the M0 contracts plan); `Trigger::{HarnessStopped, Integrated, VerifyStopped, Reverify}`,
  `Command::Reverify`, `ErrorScope::Harness`, `Command::to_trigger() -> Option<Trigger>` (the M0
  contracts plan, lane CC-CORE, Tasks 13 to 16); `factory_spec::{parse, sha256_hex, validate,
  validate_split, child_view, Spec, Scope, FormsIndex, Problem, ParseError}` and
  `factory_presets::{preset, Preset, TestCase, TestStatus, test_id, PRESETS_VERSION}` (the M0
  spec-and-presets plan); the tier naming rule and the Form format (the M0 scaffold plan).
- Every other name a lane uses is defined in Part 1 of this plan. A lane that cannot build its
  behaviour on Part 1's interface as written stops and reports; it does not change a signature.
  Adding to an interface is approved by the reviewing session first; breaking one is the
  owner's decision.

**How the crates depend on each other** (enforced by `cargo xtask test static`, after the
requests below are applied)

| Crate | May use these workspace crates |
|---|---|
| `fleet-store` | `fleet-core` |
| `fleet-forge` | none |
| `fleet-runner` | `fleet-core` |
| `fleet-verify` | `fleet-core`, `harness-protocol`, `factory-spec`, `factory-presets`, `fleet-forge`, `fleet-runner` |
| `fleet-supervisor` | `fleet-core`, `harness-protocol`, `fleet-forge`, `fleet-verify` |
| `fleet-admission` | `fleet-core`, `harness-protocol`, `factory-spec`, `fleet-store`, `fleet-forge` |
| `fleet-api` | `fleet-core`, `harness-protocol`, `factory-spec`, `fleet-store`, `fleet-forge`, `fleet-admission` |
| `fleetd` | all of them |
| `fleet-e2e` (`tests/e2e`) | `fleet-api`, `harness-protocol`; it starts `fleetd` as a program and links none of it |

- `fleet-api` does not link the supervisor, the verifier or the runner, and `fleet-admission`
  does not link the supervisor. What they need from those crates they ask for through a port
  they define themselves: `fleet_api::Launcher` and `fleet_api::HarnessDirectory`,
  `fleet_admission::HarnessCatalog`. The supervisor reports through its own `Sink` and starts
  harnesses through its own `Connector`; the reaper asks `fleet_runner::UnitCensus`. `fleetd`
  implements each port in a few lines (`crates/fleetd/src/wire.rs`). An adapter joins two
  crates' types and holds no rule.
- Dev-dependencies are free: a crate's tests may use any other crate's `testkit`.

**Tests**

- Tiers are placed by file name, by the M0 scaffold plan's rule: a test in `src/` is `unit`;
  `tests/e2e_*.rs` is `e2e`; `tests/*_it.rs` and `tests/characterisation_*.rs` are
  `integration`; any other `tests/*.rs` is `contract`.
- `unit` and `contract` tests need no Docker, no `git`, no network and no token. A test that
  needs `git`, Docker or a built harness binary is in a `*_it.rs` target and says what it needs
  in its first comment. A test that needs Docker is also `#[ignore]`d, as the existing ones are.
- Each crate's fake, builders and contract suite are in its `testkit` module, compiled under
  `cfg(any(test, feature = "testkit"))`. A crate's own `tests/` targets get the module through
  a dev-dependency on the crate itself with the feature on (request R8).
- Time is tokio's clock, paused in tests; unit ids come from one counter seeded from the
  store; the wall clock reaches a route only through `fleet_api::Clock`.
- A test listed in Part 2 is the contract for its behaviour. It may be added to; it is not
  weakened. If a lane finds a listed test that contradicts the document it cites, the document
  wins: stop and report, do not edit the test to pass.

**Two run paths until milestone 2**

- A unit's stored `harness` column decides its path, when it is started and when it is brought
  back after a restart. A value names a registry entry, and the unit runs on
  `fleet_supervisor::run`. No value means the driver path, `fleetd::driver::run`, exactly as
  today.
- `POST /missions` takes two request shapes. A body with `spec_sha256` is the new one: it goes
  through admission and always has a harness. A body with `task` is the old one, kept until
  milestone 2: `mode: "demo"` now gets `harness = "fake"` and runs the scripted `harness-fake`
  through the supervisor (supervisor spec section 3.5); `mode: "real"` gets no harness and runs
  the in-process driver, as do the swarm routes.
- No stored row is rewritten to move it between paths. A demo unit stored before milestone 1 has
  no harness and finishes on the driver path.
- The driver path's files (`driver.rs`, `steps.rs`, `claude_meter.rs`, `retry.rs`, `fake.rs`,
  `runner.rs`, `local_docker.rs`, `swarm.rs`, `planner.rs`, `docsource.rs`) are not changed in
  milestone 1, except that `forge.rs`, `gh_forge.rs`, `store.rs` and `reconcile.rs` become
  re-exports of the crates their content moved to.

**What a stored unit looks like**

- `fleet_store::UnitRow` keeps exactly its nineteen fields: the characterisation tests build it
  with a struct literal. Everything the harness path adds is in `UnitExt`, read and written
  through `UnitRecord`. No lane adds a field to `UnitRow`.
- The JSON logged for an event is `{"unit_id":…,"seq":…,"event":{…}}`, as today. A stage note
  is logged in the same envelope with `"type":"stage"`.

**Fixed values** (provisional where marked; the owner confirms or changes them)

| Value | Where |
|---|---|
| Grace 30 s, stall window 600 s, 10 mergeability polls | `fleet_supervisor::{DEFAULT_GRACE, DEFAULT_STALL, MAX_MERGEABLE_POLLS}` (supervisor spec 4.5, 4.8) |
| Container and volume label `cc.unit_id` | `fleet_runner::UNIT_LABEL` (provisional: must equal the harness's; assumption A4) |
| Cache mount `/cache`; 1 MiB kept of each output stream | `fleet_runner::{CACHE_MOUNT, MAX_CAPTURE_BYTES}` (provisional; assumption A2) |
| Token: at least 32 printable ASCII characters, in `FLEETD_TOKEN` | `fleet_api::{Token, TOKEN_ENV}` (provisional length) |
| WebSocket subprotocols `fleetd.v1` and `fleetd.token.<token>` | `fleet_api::{WS_PROTOCOL, WS_TOKEN_PREFIX}` |
| Unit branch `factory/<issue number>-<unit id>` | lane CC-ADMISSION |
| Spec file `.reqdrive/specs/<id>.md`; configuration `.reqdrive/config.toml` | `fleet_verify::SPEC_DIR`, `fleet_admission::REPO_CONFIG_PATH` |
| Pause reasons `oracle tampering`, `awaiting ship`, `verification stopped`, `daemon restarted` | `fleet_supervisor::REASON_*`, `fleet_store::REASON_*` |

**Not in milestone 1** (each is milestone 2 or 3; a lane that meets one reports it and does not
build it): child units and `Integrated`; auxiliary runs; holdouts and their custody; the
`secrets`, `dependencies` and `eidos_gates` controls in the verifier; the regression check over tests that were green at the base; in-file test regions; disjoint-scope admission; scope grants (the
verifier already honours `scope_grants`; nothing issues one); raising a cap; restart as a
command; the merge queue; deleting the driver path.

## Assumptions awaiting spikes

Each is something this plan builds on that a milestone-0 spike, or the other repository's plan,
has not confirmed. The lane that owns the task reconciles it before its pull request, and
stops for the owner if the finding contradicts the plan.

| # | Assumption | Confirmed by | Reconciled in |
|---|---|---|---|
| A1 | On Windows, ending a harness that was started inside a Job Object with kill-on-close ends its whole process tree, and the containers it labelled can then be reaped by label. | Spike S5 | Lane CC-SUPERVISOR, test `dropping_the_run_ends_the_harness_process_and_everything_it_started` and the implementation note on `ProcessConnector`. If S5 found the Job Object insufficient, the note's fallback applies and the finding goes in the pull request. |
| A2 | A repository's `setup` can fill a cache once, with a network and no credential, and `test` can then run offline on a discarded copy of the tree with that cache mounted read-only at one fixed path (`/cache`), for both presets. | Spike S7 | Lane CC-RUNNER, tests `the_docker_runner_passes_the_check_runner_contract` (the suite's cache rules) and `setup_has_a_network_and_a_check_on_the_same_cache_does_not`. If S7 found that a preset needs the cache at another path or writable, `CACHE_MOUNT` and the read-only rule are an interface change: stop and report. |
| A3 | The test ids a preset enumerates from a frozen file's source equal the ids its report names, so the verifier can recompute `frozen_ids` from the frozen files. | Spike S6; the M0 spec-and-presets plan, Task 25 | Lane CC-VERIFY, test `frozen_ids_that_are_not_the_ids_in_the_frozen_files_are_rejected`. If S6 found that enumeration needs more than a path and a source text (a package name, for `cargo`), the verifier passes what `factory-presets` then asks for; the check itself stays. Observed while writing this plan: Node 22.17.1's built-in JUnit reporter names no file, so for that toolchain the ids do not agree; the end-to-end fixture carries its own reporter (lane CC-E2E, Task 2). |
| A4 | The harness labels every container and volume it creates with `cc.unit_id=<unit id>`. | The `reqdrive` milestone-1 plan (no spike) | Lane CC-RUNNER, test `reaping_removes_a_units_containers_whoever_started_them_and_keeps_its_volume`; lane CC-E2E, scenario 7. `UNIT_LABEL` is one constant; if `reqdrive` uses another key, the coordinator decides which side changes. |
| A5 | The `reqdrive` release that milestone 1 pins is started as `reqdrive harness` with flags that select a real workspace and a scripted agent, and a scenario file can make that agent write the fixture's test and implementation. | The `reqdrive` milestone-1 plan (no spike) | Lane CC-E2E, Task 1. Only scenarios 1 and 7 use `reqdrive`; the other five use this repository's own scripted harness and do not depend on A5. |
| A6 | `harness-fake` declares no presets (M0 contracts plan, gap 4), so admission refuses it for a unit run from a signed spec. Demo mode does not go through admission, so this is intended; the end-to-end suite's scripted harness declares both presets. | Read from the M0 contracts plan | Lane CC-E2E, Task 2. |

## Requests to the coordinator

The coordinating lane owns the root manifest, the lock file, every crate's `Cargo.toml`,
`xtask/**` and CI. Part 1 assumes every request below is already on `factory/m1`; its Task 1
checks them and stops if one is missing. No lane edits a manifest.

| # | What | Why |
|---|---|---|
| R1 | Root `[workspace.dependencies]` gains `tempfile = "3"`, `toml = { version = "1", default-features = false, features = ["std", "parse", "serde"] }`, `tower = { version = "0.5", features = ["util"] }`, `tower-http = { version = "0.6", features = ["cors"] }`, `reqwest = { version = "0.12", default-features = false, features = ["json"] }`, and `windows-sys = { version = "0.61", features = ["Win32_Foundation", "Win32_Security", "Win32_System_JobObjects", "Win32_System_Threading"] }` | `toml`, `tower`, `tower-http` and `windows-sys` are already in `Cargo.lock` at these versions (`toml` 1.1 through `factory-presets`, `tower` and `tower-http` through `fleetd`, `windows-sys` 0.61 through `tokio`), so they add no package. `tempfile` 3 and `reqwest` 0.12 are new to the lock file; both are test-only. `reqwest` has no default features: no TLS stack is built, and the suite only talks to loopback |
| R2 | Root `members` gains `tests/e2e` (package `fleet-e2e`) | The hermetic suite is a workspace crate so the tier runner finds its `e2e_*.rs` target |
| R3 | `xtask/src/deps.rs` `RULES`: `fleet-admission` may also use `fleet-forge`, `fleet-store`, `harness-protocol`; `fleet-api` may also use `factory-spec`, `fleet-admission`, `fleet-forge`, `fleet-store`; `fleet-supervisor` may also use `fleet-forge`, `fleet-verify`; `fleet-verify` may also use `fleet-forge`, `fleet-runner`; a new row for `fleet-e2e`: `Internal::Only(&["fleet-api", "harness-protocol"])`, not pure, `testkit: true` | The table under Global Constraints. No crate but `fleetd` reaches every other: `fleet-api` reaches neither the supervisor, the verifier nor the runner |
| R4 | The seven manifests below, exactly | Each crate's dependencies, its `testkit` feature and what that feature turns on in other crates |
| R5 | `crates/fleetd/Cargo.toml` as below | `fleetd` links every crate. It turns on `testkit` in `fleet-forge` and `fleet-verify` as normal dependencies: demo mode runs on those crates' fakes (supervisor spec 3.5), in the shipped binary |
| R6 | `tests/e2e/Cargo.toml` as below | The suite's own crate |
| R7 | The `integration` tier builds `harness-fake` before it runs: `cargo build -p harness-conformance --bins` | `crates/fleet-supervisor/tests/process_it.rs` and `crates/fleetd/tests/wire_it.rs` start that binary from `target/<profile>/` |
| R8 | Every `fleet-*` crate dev-depends on itself with `features = ["testkit"]` (shown in the manifests) | A crate's `tests/` targets link the library without `cfg(test)`. The self dev-dependency turns the feature on for them without a `required-features` line, which would let `cargo test -p <crate>` skip the target silently. Checked: `cargo xtask deps` accepts it |
| R9 | When a lane's pull request turns a gate from "planned" into a test that exists, the coordinator adds its row to `forms/registry.md` and moves the invariant from **Unenforced** into the Form's gates table, in the same merge | A Form and the registry have different owners (M0 scaffold plan, gap 8). Each lane lists the rows it wants under **Form draft** |
| R10 | A CI job named `e2e`, added to the workflow and to `xtask`'s `PINNED_JOBS`, that runs `cargo xtask test e2e` on Linux and is not a required check while every scenario is unfrozen. The tier builds the daemon before it runs (`cargo build -p fleetd --bin serve`); the job has Docker and puts the pinned `reqdrive` binary on the path given by `REQDRIVE_BIN`; the tag is the content of `tests/e2e/reqdrive.pin` | Lane CC-E2E starts `target/<profile>/serve` as a program. Cargo builds `harness-scripted` itself, because it is a binary of the package under test |

The manifests of requests R4 to R6:

`crates/fleet-store/Cargo.toml`:

````toml
[package]
name = "fleet-store"
edition.workspace = true
version.workspace = true
license.workspace = true
description = "Control-plane persistence: the event log and its projections."

[features]
# Builders for this crate's own types, for other crates' tests.
testkit = []

[dependencies]
fleet-core = { path = "../fleet-core" }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
rusqlite = { workspace = true }
sha2 = { workspace = true }

[dev-dependencies]
fleet-store = { path = ".", features = ["testkit"] }
tempfile = { workspace = true }
````

`crates/fleet-forge/Cargo.toml`:

````toml
[package]
name = "fleet-forge"
edition.workspace = true
version.workspace = true
license.workspace = true
description = "Git and GitHub work done on the host: bundles, push, pull requests and mergeability."

[features]
# Builders for this crate's own types, for other crates' tests.
testkit = []

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tokio = { workspace = true }
async-trait = { workspace = true }
sha2 = { workspace = true }

[dev-dependencies]
fleet-forge = { path = ".", features = ["testkit"] }
tempfile = { workspace = true }
````

`crates/fleet-runner/Cargo.toml`:

````toml
[package]
name = "fleet-runner"
edition.workspace = true
version.workspace = true
license.workspace = true
description = "Check containers for the verifier, and reaping containers by label. The only crate that may reach Docker."

[features]
# Builders for this crate's own types, for other crates' tests.
testkit = []

[dependencies]
fleet-core = { path = "../fleet-core" }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tokio = { workspace = true }
async-trait = { workspace = true }
sha2 = { workspace = true }

[dev-dependencies]
fleet-runner = { path = ".", features = ["testkit"] }
tempfile = { workspace = true }
````

`crates/fleet-verify/Cargo.toml`:

````toml
[package]
name = "fleet-verify"
edition.workspace = true
version.workspace = true
license.workspace = true
description = "The control plane's own verification of what a harness reports."

[features]
# Builders for this crate's own types, for other crates' tests.
testkit = [
    "fleet-forge/testkit",
    "fleet-runner/testkit",
    "factory-spec/testkit",
    "factory-presets/testkit",
]

[dependencies]
fleet-core = { path = "../fleet-core" }
fleet-forge = { path = "../fleet-forge" }
fleet-runner = { path = "../fleet-runner" }
harness-protocol = { path = "../harness-protocol" }
factory-spec = { path = "../factory-spec" }
factory-presets = { path = "../factory-presets" }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tokio = { workspace = true }
async-trait = { workspace = true }

[dev-dependencies]
fleet-verify = { path = ".", features = ["testkit"] }
fleet-forge = { path = "../fleet-forge", features = ["testkit"] }
fleet-runner = { path = "../fleet-runner", features = ["testkit"] }
factory-spec = { path = "../factory-spec", features = ["testkit"] }
factory-presets = { path = "../factory-presets", features = ["testkit"] }
tempfile = { workspace = true }
tokio = { workspace = true, features = ["test-util"] }
````

`crates/fleet-supervisor/Cargo.toml`:

````toml
[package]
name = "fleet-supervisor"
edition.workspace = true
version.workspace = true
license.workspace = true
description = "Supervises one harness process over a byte stream."

[features]
# Builders for this crate's own types, for other crates' tests.
testkit = ["fleet-verify/testkit", "fleet-forge/testkit"]

[dependencies]
fleet-core = { path = "../fleet-core" }
fleet-forge = { path = "../fleet-forge" }
fleet-verify = { path = "../fleet-verify" }
harness-protocol = { path = "../harness-protocol" }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tokio = { workspace = true }
async-trait = { workspace = true }
toml = { workspace = true }

[target.'cfg(windows)'.dependencies]
windows-sys = { workspace = true }

[dev-dependencies]
fleet-supervisor = { path = ".", features = ["testkit"] }
fleet-forge = { path = "../fleet-forge", features = ["testkit"] }
fleet-verify = { path = "../fleet-verify", features = ["testkit"] }
tempfile = { workspace = true }
tokio = { workspace = true, features = ["test-util"] }
````

`crates/fleet-admission/Cargo.toml`:

````toml
[package]
name = "fleet-admission"
edition.workspace = true
version.workspace = true
license.workspace = true
description = "Decides whether a unit may start: spend, concurrency, signature, eligibility and scope."

[features]
# Builders for this crate's own types, for other crates' tests.
testkit = ["fleet-store/testkit", "fleet-forge/testkit", "factory-spec/testkit"]

[dependencies]
fleet-core = { path = "../fleet-core" }
fleet-forge = { path = "../fleet-forge" }
fleet-store = { path = "../fleet-store" }
harness-protocol = { path = "../harness-protocol" }
factory-spec = { path = "../factory-spec" }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tokio = { workspace = true }
async-trait = { workspace = true }
toml = { workspace = true }

[dev-dependencies]
fleet-admission = { path = ".", features = ["testkit"] }
fleet-store = { path = "../fleet-store", features = ["testkit"] }
fleet-forge = { path = "../fleet-forge", features = ["testkit"] }
factory-spec = { path = "../factory-spec", features = ["testkit"] }
````

`crates/fleet-api/Cargo.toml`:

````toml
[package]
name = "fleet-api"
edition.workspace = true
version.workspace = true
license.workspace = true
description = "HTTP and WebSocket routes, authentication and schema export."

[features]
# Builders for this crate's own types, for other crates' tests.
testkit = [
    "dep:tower",
    "fleet-store/testkit",
    "fleet-forge/testkit",
    "fleet-admission/testkit",
    "factory-spec/testkit",
]

[dependencies]
fleet-core = { path = "../fleet-core" }
fleet-admission = { path = "../fleet-admission" }
fleet-forge = { path = "../fleet-forge" }
fleet-store = { path = "../fleet-store" }
harness-protocol = { path = "../harness-protocol" }
factory-spec = { path = "../factory-spec" }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tokio = { workspace = true }
async-trait = { workspace = true }
axum = { workspace = true }
schemars = { workspace = true }
sha2 = { workspace = true }
tower-http = { workspace = true }
tower = { workspace = true, optional = true }

[dev-dependencies]
fleet-api = { path = ".", features = ["testkit"] }
fleet-store = { path = "../fleet-store", features = ["testkit"] }
fleet-forge = { path = "../fleet-forge", features = ["testkit"] }
fleet-admission = { path = "../fleet-admission", features = ["testkit"] }
factory-spec = { path = "../factory-spec", features = ["testkit"] }
tower = { workspace = true }
tokio = { workspace = true, features = ["test-util"] }
tokio-tungstenite = "0.24"
futures-util = "0.3"
````

`crates/fleetd/Cargo.toml`:

````toml
[package]
name = "fleetd"
edition.workspace = true
version.workspace = true
license.workspace = true

[dependencies]
fleet-core = { path = "../fleet-core" }
fleet-admission = { path = "../fleet-admission" }
fleet-api = { path = "../fleet-api" }
fleet-forge = { path = "../fleet-forge", features = ["testkit"] }
fleet-runner = { path = "../fleet-runner" }
fleet-store = { path = "../fleet-store" }
fleet-supervisor = { path = "../fleet-supervisor" }
fleet-verify = { path = "../fleet-verify", features = ["testkit"] }
factory-presets = { path = "../factory-presets" }
harness-protocol = { path = "../harness-protocol" }
toml = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tokio = { workspace = true }
async-trait = { workspace = true }
axum = { workspace = true }
dotenvy = { workspace = true }
rusqlite = { workspace = true }
# CORS. The one crate this adds to the graph (`tower` 0.5.3 is already resolved via
# axum). Hand-rolling the headers is where `Vary: Origin` cache-poisoning bugs live,
# and preflight is fiddly enough to be worth a maintained implementation.
tower-http = { version = "0.6", features = ["cors"] }

[dev-dependencies]
fleet-admission = { path = "../fleet-admission", features = ["testkit"] }
fleet-api = { path = "../fleet-api", features = ["testkit"] }
fleet-runner = { path = "../fleet-runner", features = ["testkit"] }
fleet-store = { path = "../fleet-store", features = ["testkit"] }
fleet-supervisor = { path = "../fleet-supervisor", features = ["testkit"] }
tempfile = { workspace = true }
tokio = { version = "1", features = ["test-util"] }
# Real WebSocket client for the /stream integration test (same versions axum
# already resolves, so no new crates enter the lock graph).
tokio-tungstenite = "0.24"
futures-util = "0.3"
# `ServiceExt::oneshot` for router-level request tests. Already in the lock graph
# via axum, so this adds no new crate.
tower = { version = "0.5", features = ["util"] }
````

`tests/e2e/Cargo.toml`:

````toml
[package]
name = "fleet-e2e"
edition.workspace = true
version.workspace = true
license.workspace = true
description = "The hermetic end-to-end suite: a real fleetd, a real harness, a local git remote."
publish = false

[features]
# Builders for this crate's own types, for other crates' tests.
testkit = []

[dependencies]
fleet-api = { path = "../../crates/fleet-api" }
harness-protocol = { path = "../../crates/harness-protocol" }
serde = { workspace = true }
serde_json = { workspace = true }
tokio = { workspace = true }
tempfile = { workspace = true }
reqwest = { workspace = true }
````

What each dependency is for, where it is not obvious:

| Crate | Dependency | Why this one |
|---|---|---|
| `fleet-store` | `sha2` | A signature is refused unless its hash is the SHA-256 of its bytes. The store checks that itself, so no caller can store a signature that binds other bytes |
| `fleet-forge`, `fleet-runner` | `sha2` | The in-memory forge names commits by a digest; the runner keys a dependency cache by the digest of the manifests and lockfiles |
| `fleet-admission` | `toml` | Reads `.reqdrive/config.toml` |
| `fleet-admission` | `tokio` | Only `tokio::sync::Semaphore`, the concurrency slots it looks at |
| `fleet-supervisor` | `toml` | Reads the harness registry file |
| `fleet-supervisor` | `windows-sys` (Windows only) | Job Objects, so that ending a harness ends its process tree (assumption A1) |
| `fleet-api` | `tower-http` | CORS, moved from `fleetd` with the routes |
| `fleet-api` | `tower` (optional, behind `testkit`) | `testkit::call` sends one request through a router with `ServiceExt::oneshot` |
| `fleet-api` | `tokio-tungstenite` 0.24, `futures-util` 0.3 (dev) | A WebSocket client for the stream test; the versions `fleetd` already locks |
| `fleet-e2e` | `reqwest` | Talks HTTP to the real daemon |

No constant-time comparison crate is requested: `fleet_api::Token::matches` compares SHA-256
digests of the two values with a loop that never exits early, using `sha2`.

## Review Focus

Five ways this work is most likely to go wrong where an ordinary test of the happy path would
not notice. Each has a named test in the lane that owns it.

| # | Failure mode | Why it bites here | Tests |
|---|---|---|---|
| 1 | **A scope check that starts one commit too late.** The harness commits the signed spec first, and the scope check covers "everything after the spec commit". A first commit that also carries another file would then be checked by nothing. | The verifier is the last defence against a change outside what the person signed. | Lane CC-VERIFY: `a_first_commit_that_carries_anything_but_the_spec_file_is_rejected`, `a_delivery_with_no_commit_at_all_is_never_approved`, `a_spec_file_changed_or_deleted_by_a_later_commit_is_rejected` |
| 2 | **"Every frozen test passed" over an empty or wrong list.** If the harness reports no frozen ids, or ids that are not the ones in the frozen files, the per-test check passes while the real tests never ran. | The list comes from the harness; the verifier must not trust it. | Lane CC-VERIFY: `a_unit_with_no_freeze_or_an_empty_one_is_never_verified`, `frozen_ids_that_are_not_the_ids_in_the_frozen_files_are_rejected`, `a_frozen_id_missing_from_the_report_is_a_failure_even_when_the_command_exits_zero` |
| 3 | **Windows processes and pipes.** A harness that outlives the daemon, a child whose output fills its pipe while nobody reads it, a line with no end, standard error read only after standard output closes. | CI ran on Linux only until milestone 0; the owner's host is Windows. | Lane CC-SUPERVISOR: `a_line_longer_than_the_protocol_allows_fails_the_unit_without_being_buffered_whole`, `a_harness_that_writes_without_pausing_does_not_deadlock_the_unit`, `dropping_the_run_ends_the_harness_process_and_everything_it_started`. Lane CC-RUNNER: `a_command_that_prints_more_than_the_cap_keeps_the_end_of_its_output` |
| 4 | **Two requests through one check.** Two missions racing for the last dollar under the ceiling, or carrying one idempotency key, each pass a check made before either wrote. | Issue #74 is this class; SQLite gives one writer only if the check and the write are one transaction. | Lane CC-STORE: `ten_threads_racing_for_the_last_slot_under_the_ceiling_admit_exactly_one`, `a_reservation_that_cannot_be_written_is_an_error_and_leaves_no_row`. Lane CC-ADMISSION: `two_requests_with_one_key_at_the_same_moment_start_one_unit`, `a_reservation_that_cannot_be_recorded_refuses_the_request` |
| 5 | **`git` on the host doing more than it was asked.** A hook in a clone running during verification, `core.autocrlf` rewriting a frozen file so its hash changes, a path with a space or a non-ASCII letter quoted in `git diff` output and then compared as text, a branch name read as an option. | The forge runs `git` beside the push credential, on Windows. | Lane CC-FORGE: `a_hook_planted_in_the_scratch_clone_does_not_run_when_the_tree_is_exported`, `paths_with_spaces_and_non_ascii_names_read_and_export_unchanged`, `a_path_that_climbs_out_of_the_repository_is_refused`, and the contract suite's CRLF file, run against `git` |

## How to read this plan

- A fenced block after a file path is the whole content of that file, exactly. Every listing in
  Part 1 and every test in Part 2 was copied by script from a workspace in which it compiles;
  the Self-review says what was run.
- In Part 1, a step that says "run … expected" names what you must see before going on.
- In Part 2, each lane's **Behaviours** list names tests. Their code is the listing that follows
  the list, in the file its heading names. Create that file first, run it, and see every test
  fail with the message the lane states (`not implemented: lane … builds this`). Then build the
  implementation until they pass.
- A line number cited for an existing file is the file's as milestone 0 leaves it; for the
  files of `crates/fleetd/src/`, outside their `mod tests`, that is also today's. Each number
  comes with the name of what it points at. If the file has moved, find the name.
- `unimplemented!("lane X builds this")` marks exactly the bodies lane X writes. When a lane is
  done, no such call is left in the files it owns: its **Verify** greps for it.

---

# Part 1 — Wave 0: interfaces, fakes and contract suites

One agent executes this part alone, before any lane starts.

**Owns:** `src/**` of `crates/fleet-store`, `crates/fleet-forge`, `crates/fleet-runner`,
`crates/fleet-verify`, `crates/fleet-admission`, `crates/fleet-supervisor`, `crates/fleet-api`
(replacing each skeleton's `src/lib.rs`); the new file `crates/fleetd/src/wire.rs` and one line
of `crates/fleetd/src/lib.rs`; `tests/e2e/src/lib.rs`. No manifest, no lock file.

**Reads:** this plan; `forms/README.md`; the seven Forms under `forms/` if they are already
drafted (their Interface sections are the signatures printed here).

**Worktree and branch:** creates the integration branch `factory/m1` from `origin/main` (which
by then holds milestone 0) and pushes it. Works in `D:\MajorProjects\.swarm-wt\m1-cc-interfaces`
on `feat/m1-interfaces`. Pull request: `feat/m1-interfaces` into `factory/m1`.

**Needs:** milestone 0 merged to `main`; the requests R1 to R8 applied on `factory/m1` by the
coordinating lane.

**Blocks:** every lane of Part 2.

**What wave 0 is.** For each crate it writes three things, all complete:

1. *The interface.* Every trait, type and function another crate or a lane's test names, with
   its documentation. A function a lane will write has the body
   `unimplemented!("lane … builds this")`.
2. *The fake.* An in-memory implementation of each seam, in the crate's `testkit` module, with
   builders for the crate's types.
3. *The contract suite.* A public function in the same module that takes a constructor for any
   implementation of the seam and asserts what every implementation must do; and a test that
   runs it against the fake. Each lane later runs the same function against its real
   implementation.

A few pure functions that two sides must agree on are also complete here, with their tests,
because a fake or another crate needs them from the first day: the fold from events to a row
(`fleet_store::Projection`), the pull-request body and its check (`fleet_forge::pr_body`,
`body_carries`), the cache key (`fleet_runner::cache_key`), and the schema mirrors of the core
event and command types (`fleet_api::UnitEventDto`, `CommandDto`).

**How each task goes.** The source files other than `testkit.rs` are created first. Then the
test at the end of `testkit.rs` is written alone and seen to fail to compile, because the fake
and the suite it names do not exist. Then `testkit.rs` is written in full and the test passes.
The red state named in each task is the compiler's: `E0433` (use of an undeclared type),
`E0425` (cannot find a function) and `E0432` (unresolved import) for the names the test uses.

**Verify** (run in the worktree after Task 9):

| Command | Expected |
|---|---|
| `cargo build --workspace --all-targets --locked` | exit 0 |
| `cargo fmt --all -- --check` | no output |
| `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | exit 0, no warning |
| `cargo test -p fleet-store -p fleet-forge -p fleet-runner -p fleet-verify -p fleet-admission -p fleet-supervisor -p fleet-api --lib` | seven `test result: ok.` lines: 4 passed (`fleet-admission`), 8 (`fleet-api`), 10 (`fleet-forge`), 10 (`fleet-runner`), 14 (`fleet-store`), 5 (`fleet-supervisor`), 4 (`fleet-verify`) |
| the same command with `--features testkit` appended for one crate at a time | the same count for that crate |
| *Git-Bash:* `cargo test --workspace --locked 2>&1 \| grep -E "^test result" \| awk '{p+=$4; f+=$6; i+=$8} END {print "passed="p" failed="f" ignored="i}'` | `failed=0`, `ignored` unchanged, and 48 more passed than at the branch point recorded in Task 1 (55 new tests; seven skeleton tests replaced) |
| `cargo xtask test static --root-only` | exit 0; `xtask: dependency direction: ok` |
| `cargo xtask test unit` and `cargo xtask test contract` | exit 0 |
| *Git-Bash:* `grep -rnE "unimplemented!\(\)\|todo!\(" crates/fleet-*/src crates/fleetd/src/wire.rs tests/e2e/src` | no output: every placeholder carries a message that names the lane that removes it, written out or through its file's `NEW` or `MOVE` constant |
| `git diff --name-only origin/factory/m1...HEAD` | only paths under **Owns** |

### Task 1: The integration branch, the worktree and the coordinator's requests

**Files:** none.

- [ ] **Step 1: Create `factory/m1` if it does not exist, and the worktree**

```
git -C D:/MajorProjects/INFRASTRUCTURE/command-center fetch origin
git -C D:/MajorProjects/INFRASTRUCTURE/command-center ls-remote --heads origin factory/m1
```

If the second command prints nothing:

```
git -C D:/MajorProjects/INFRASTRUCTURE/command-center push origin origin/main:refs/heads/factory/m1
git -C D:/MajorProjects/INFRASTRUCTURE/command-center fetch origin
```

Then, in either case:

```
git -C D:/MajorProjects/INFRASTRUCTURE/command-center worktree add --no-track -b feat/m1-interfaces D:/MajorProjects/.swarm-wt/m1-cc-interfaces origin/factory/m1
```

`--no-track` keeps a plain `git push` from being aimed at the integration branch. Every later
command of Part 1 runs in `D:\MajorProjects\.swarm-wt\m1-cc-interfaces`.

- [ ] **Step 2: Check that milestone 0 is there**

*Git-Bash:*

```bash
grep -n 'pub const PROTOCOL_VERSION: &str = "0.2"' crates/harness-protocol/src/lib.rs
grep -n 'HarnessStopped\|VerifyStopped\|Reverify' crates/fleet-core/src/transition.rs | head -3
grep -n 'Reverify' crates/fleet-core/src/event.rs | head -1
ls crates/factory-spec/src/validate.rs crates/factory-presets/src/presets.rs crates/fleetd/tests/characterisation_store.rs forms/README.md
```

Expected: each `grep` prints at least one line and `ls` lists four files. If one is missing,
milestone 0 is not on this branch: stop and report.

- [ ] **Step 3: Check the coordinator's requests**

*Git-Bash:*

```bash
grep -n '^tempfile\|^toml\|^tower\|^reqwest\|^windows-sys\|"tests/e2e"' Cargo.toml
for c in fleet-store fleet-forge fleet-runner fleet-verify fleet-supervisor fleet-admission fleet-api; do grep -c "^$c = { path = \".\", features = \[\"testkit\"\] }" crates/$c/Cargo.toml; done
grep -n 'fleet-supervisor\|fleet-verify' crates/fleetd/Cargo.toml
cargo xtask deps
```

Expected: seven lines from the first `grep` (`tempfile`, `toml`, `tower`, `tower-http`,
`reqwest`, `windows-sys` and the `tests/e2e` member); seven `1`s; three lines from the third; and
`xtask: dependency direction: ok`. Compare each of the nine manifests with the listings under
"Requests to the coordinator". Any difference: stop and report; do not edit a manifest.

- [ ] **Step 4: Record the branch point**

*Git-Bash:*

```bash
cargo test --workspace --locked 2>&1 | grep -E "^test result" | awk '{p+=$4; f+=$6; i+=$8} END {print "passed="p" failed="f" ignored="i}'
```

Expected: `failed=0`. Write the `passed` and `ignored` numbers down; the Verify table compares
against them. (The skeleton crates still have their one test each here; Tasks 2 to 8 replace
each skeleton, so the seven `skeleton_builds` tests go and 55 tests arrive: 48 more.)

### Task 2: `fleet-store` — the unit store and the signature store

**Files:**
- Replace: `crates/fleet-store/src/lib.rs`
- Create: `crates/fleet-store/src/error.rs`, `types.rs`, `traits.rs`, `projection.rs`,
  `sqlite.rs`, `testkit.rs`

**Produces:** `Store`, `SharedStore`, `UnitRow`, `UnitExt`, `UnitRecord`, `SwarmRow`, `LaneRow`,
`Signature`, `SignatureSummary`, `Reservation`, `Restarted`, `StoreError`, `sha256_hex`; the
seams `UnitStore` and `SignatureStore`; `Projection` with `REASON_ORACLE_TAMPERING` and
`REASON_DAEMON_RESTARTED`. In `testkit`: `MemStore`, `unit_row`, `record`, `harness_record`,
`signature`, `envelope_json`, `phase_changed`, `unit_store_contract`,
`signature_store_contract`.

What to know before reading the listings:

- `Store` keeps the method names and arguments of `crates/fleetd/src/store.rs`, so the
  characterisation tests compile against it with one `use` line changed. Its error type is
  `StoreError`, not the storage engine's.
- `UnitStore` and `SignatureStore` are implemented for `Mutex<Store>` (`SharedStore`), not for
  `Store`: an `Arc<SharedStore>` is then both `Arc<dyn UnitStore>` and the
  `Arc<Mutex<Store>>` the driver path already passes around, over one connection.
- `record_event` appends and folds in one step. The caller no longer keeps a projection of its
  own, so a row can never disagree with its log, and a resumed unit cannot lose its cost
  (supervisor spec section 5).
- `reserve_unit` is the ceiling check and the insert as one step (issue #74).

- [ ] **Step 1: Create the source files other than `testkit.rs`**

`crates/fleet-store/src/lib.rs`:

````rust
//! `fleet-store` — durable state for the control plane: one append-only event log per unit, one
//! row per unit that summarises its log, and the signatures of the specs people have signed.
//!
//! The interface has three parts. [`Store`] is the SQLite implementation, with the methods the
//! in-process driver path has always used. [`UnitStore`] and [`SignatureStore`] are the seams
//! other crates depend on; `Mutex<Store>` implements both, and so does the in-memory
//! `testkit::MemStore`. [`Projection`] is the pure fold from a unit's events to its row.

mod error;
mod projection;
mod sqlite;
mod traits;
mod types;

#[cfg(any(test, feature = "testkit"))]
pub mod testkit;

pub use error::StoreError;
pub use projection::{Projection, REASON_DAEMON_RESTARTED, REASON_ORACLE_TAMPERING};
pub use sqlite::{SharedStore, Store};
pub use traits::{SignatureStore, UnitStore};
pub use types::{
    sha256_hex, LaneRow, Reservation, Restarted, Signature, SignatureSummary, SwarmRow, UnitExt,
    UnitRecord, UnitRow,
};
````

`crates/fleet-store/src/error.rs`:

````rust
//! The one error type of this crate. No type of the storage engine appears in it.

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StoreError {
    /// The storage engine refused or failed the operation. The text is its message.
    #[error("store failure: {0}")]
    Failed(String),
    /// A signature whose `sha256` is not the SHA-256 of its `bytes`.
    #[error("signature hash {claimed} is not the hash of its bytes ({actual})")]
    HashMismatch { claimed: String, actual: String },
}
````

`crates/fleet-store/src/types.rs`:

````rust
//! The rows this crate stores.

use sha2::{Digest, Sha256};

/// SHA-256 of `bytes` as 64 lowercase hex characters: the hash a signature is keyed by.
pub fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// The persisted projection of a unit. These nineteen fields are the ones the in-process driver
/// path has always written; what the factory path adds is in [`UnitExt`].
#[derive(Debug, Clone, PartialEq)]
pub struct UnitRow {
    pub unit_id: String,
    pub tier: String,
    pub task: String,
    pub repo_url: String,
    pub repo_slug: String,
    pub base_branch: String,
    pub branch: String,
    pub test_cmd: String,
    pub usd_cap: f64,
    pub wall_clock_secs: u64,
    pub phase: String,
    pub cost: f64,
    pub last_seq: u64,
    pub oracle_frozen: bool,
    pub oracle_hash: Option<String>,
    pub terminal_reason: Option<String>,
    /// `demo` or `real`. Set once, when the unit is first written.
    pub mode: String,
    /// Set once, when the unit is first written.
    pub min_review_rounds: u32,
    /// The swarm a lane's unit belongs to. Set once.
    pub swarm_id: Option<String>,
}

/// What a harness-run unit stores beside its [`UnitRow`]. Every field is `None`, zero or `false`
/// for a unit the in-process driver runs.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct UnitExt {
    /// The registry name of the harness that runs the unit. `None` routes the unit to the
    /// in-process driver. Set once.
    pub harness: Option<String>,
    /// The oracle hash a person approved (T2, T3). Cleared when tampering is found.
    pub approved_oracle_hash: Option<String>,
    /// Agent time used so far, in milliseconds.
    pub elapsed_ms: u64,
    /// Why the unit is waiting for a person: the reason of the phase change that paused it.
    /// `None` when the unit is not in `needs_human` or `halted`.
    pub pause_reason: Option<String>,
    /// The phase the unit was in when it paused, as its wire string.
    pub paused_from: Option<String>,
    /// The work item, as `owner/repo#N`. Set once.
    pub work_item: Option<String>,
    /// Set once.
    pub spec_id: Option<String>,
    /// SHA-256 of the signed spec bytes the unit runs from. Set once.
    pub spec_sha256: Option<String>,
    /// The commit the unit's source bundle is cut at. Set once.
    pub base_sha: Option<String>,
    /// Set once.
    pub profile: Option<String>,
    /// The person opted in to a weaker guarantee for this unit. Set once.
    pub opt_in: bool,
    /// The caller's idempotency key. Unique across units. Set once.
    pub idempotency_key: Option<String>,
    /// The harness's oracle freeze, as the JSON of `harness_protocol::OracleFreeze`.
    pub freeze_json: Option<String>,
    /// The harness's last result, as the JSON of `harness_protocol::UnitResult`.
    pub result_json: Option<String>,
    /// The pull request opened for the unit.
    pub pr_url: Option<String>,
}

/// A unit as stored: the row and its factory extension.
#[derive(Debug, Clone, PartialEq)]
pub struct UnitRecord {
    pub row: UnitRow,
    pub ext: UnitExt,
}

/// What [`crate::UnitStore::reserve_unit`] decided. Checking the ceiling and writing the row are
/// one atomic step, so two requests cannot both pass the check.
#[derive(Debug, Clone, PartialEq)]
pub enum Reservation {
    /// The row was written and its cap now counts as committed spend.
    Reserved,
    /// Committed spend in the window is at or above the ceiling. Nothing was written.
    OverCap { committed: f64 },
    /// A unit already holds this idempotency key. Nothing was written.
    DuplicateKey { unit_id: String },
}

/// What [`crate::UnitStore::mark_restarted`] did to one unit found without a live supervisor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Restarted {
    /// The unit was running; it is now `halted`, with one synthetic event at `seq`.
    Halted { seq: u64 },
    /// The unit was already waiting for a person (`needs_human` or `halted`); nothing changed,
    /// so its reason survives.
    KeptForHuman,
    /// The unit is finished or unknown; nothing changed.
    Untouched,
}

/// A person's signature over the exact bytes of one spec file at one commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature {
    pub repo_slug: String,
    pub spec_id: String,
    /// The commit the bytes were read at.
    pub commit: String,
    /// SHA-256 of `bytes`, lowercase hex. The store refuses a signature where it is not.
    pub sha256: String,
    /// The signed bytes, exactly as read. Units run from these and from no branch.
    pub bytes: Vec<u8>,
    pub signed_by: String,
    pub signed_at_ms: i64,
}

/// A signature without its bytes, for listings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureSummary {
    pub repo_slug: String,
    pub spec_id: String,
    pub commit: String,
    pub sha256: String,
    pub signed_by: String,
    pub signed_at_ms: i64,
}

impl Signature {
    pub fn summary(&self) -> SignatureSummary {
        SignatureSummary {
            repo_slug: self.repo_slug.clone(),
            spec_id: self.spec_id.clone(),
            commit: self.commit.clone(),
            sha256: self.sha256.clone(),
            signed_by: self.signed_by.clone(),
            signed_at_ms: self.signed_at_ms,
        }
    }
}

/// Persisted swarm configuration and its projection. Used only by the in-process driver path.
#[derive(Debug, Clone, PartialEq)]
pub struct SwarmRow {
    pub swarm_id: String,
    pub repo_url: String,
    pub repo_slug: String,
    pub base_branch: String,
    pub doc_path: String,
    pub tier: String,
    pub mode: String,
    pub lane_cap: u32,
    pub usd_budget: f64,
    pub per_lane_cap: f64,
    pub status: String,
    pub planner_cost: f64,
    pub lanes_launched: u32,
    pub lanes_dropped: u32,
    pub min_review_rounds: u32,
    pub terminal_reason: Option<String>,
}

/// One lane of a swarm: the admission decision and the unit it became, if any.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaneRow {
    pub swarm_id: String,
    pub idx: u32,
    pub title: String,
    pub task: String,
    pub rationale: String,
    pub decision: String,
    pub unit_id: Option<String>,
}
````

`crates/fleet-store/src/traits.rs`:

````rust
//! The seams other crates depend on. Every method is synchronous and short: a caller must never
//! hold a store call across an `.await`.

use crate::error::StoreError;
use crate::types::{Reservation, Restarted, Signature, SignatureSummary, UnitRecord, UnitRow};
use fleet_core::Event;

/// Units: their rows, their event logs, and the spend they commit.
pub trait UnitStore: Send + Sync {
    /// Admit a unit against the spend ceiling and record it, as one atomic step. Answers
    /// [`Reservation::DuplicateKey`] when another unit holds `record.ext.idempotency_key`, and
    /// [`Reservation::OverCap`] when `committed_spend(window_start_ms)` is at or above
    /// `global_cap`. An `Err` means nothing was written, and the caller must refuse the request.
    fn reserve_unit(
        &self,
        record: &UnitRecord,
        window_start_ms: i64,
        global_cap: f64,
        now_ms: i64,
    ) -> Result<Reservation, StoreError>;

    fn get_unit(&self, unit_id: &str) -> Result<Option<UnitRow>, StoreError>;
    fn get_record(&self, unit_id: &str) -> Result<Option<UnitRecord>, StoreError>;
    /// Every unit, in the order they were first written.
    fn list_records(&self) -> Result<Vec<UnitRecord>, StoreError>;
    /// The highest `n` among unit ids of the form `u<n>`; 0 when there is none.
    fn max_unit_seq(&self) -> Result<u64, StoreError>;
    fn find_by_idempotency_key(&self, key: &str) -> Result<Option<String>, StoreError>;

    /// Worst-case spend of the units first written at or after `since_ms`: a finished unit
    /// counts its cost, any other the larger of its cap and its cost.
    fn committed_spend(&self, since_ms: i64) -> Result<f64, StoreError>;

    /// Append one record to the unit's log and fold it into the unit's row, as one atomic step.
    /// `json` is stored byte for byte. `event` is the typed form of the same record when it is a
    /// `fleet_core::Event`; `None` for a record that changes no column but `last_seq` (a stage
    /// note). A `seq` already in the log changes nothing.
    fn record_event(
        &self,
        unit_id: &str,
        seq: u64,
        ts_ms: i64,
        json: &str,
        event: Option<&Event>,
    ) -> Result<(), StoreError>;

    /// The unit's records with a sequence number above `since`, ascending, byte for byte.
    fn events_since(&self, unit_id: &str, since: u64) -> Result<Vec<String>, StoreError>;

    /// Keep the oracle freeze the harness reported, replacing any earlier one.
    fn put_freeze(&self, unit_id: &str, freeze_json: &str, now_ms: i64) -> Result<(), StoreError>;
    /// Keep the harness's last result, replacing any earlier one.
    fn put_result(&self, unit_id: &str, result_json: &str, now_ms: i64) -> Result<(), StoreError>;

    /// Startup reconciliation for one unit that has no live supervisor. A running unit becomes
    /// `halted` with one synthetic event. A unit already waiting for a person is left exactly as
    /// it is, so the reason it needs a human is still there after a restart.
    fn mark_restarted(&self, unit_id: &str, now_ms: i64) -> Result<Restarted, StoreError>;
}

/// Signatures, keyed by repository and by the SHA-256 of the signed bytes.
pub trait SignatureStore: Send + Sync {
    /// Store a signature with its bytes. Signing the same bytes again replaces who and when.
    /// Refused with [`StoreError::HashMismatch`] when `sha256` is not the hash of `bytes`.
    fn put_signature(&self, signature: &Signature) -> Result<(), StoreError>;
    fn get_signature(&self, repo_slug: &str, sha256: &str)
        -> Result<Option<Signature>, StoreError>;
    /// A repository's signatures, newest first.
    fn list_signatures(&self, repo_slug: &str) -> Result<Vec<SignatureSummary>, StoreError>;
}
````

`crates/fleet-store/src/projection.rs`:

````rust
//! The fold from a unit's events to its row. Pure: no storage, no clock.
//!
//! Every implementation of [`crate::UnitStore::record_event`] applies this fold, so the row of a
//! unit is always the fold of its log. The rules are the supervisor spec's persistence section
//! (section 5), plus the two columns that let a paused unit keep its reason across a restart.

use crate::types::UnitRecord;
use fleet_core::{ArtifactKind, Event, Phase};

/// The reason on the phase change that pauses a unit for oracle tampering. The supervisor emits
/// it and the fold reads it, so the two crates must use the same text; `fleetd` has a test that
/// compares this constant with the supervisor's.
pub const REASON_ORACLE_TAMPERING: &str = "oracle tampering";

/// The reason on the synthetic event written for a unit found running at startup.
pub const REASON_DAEMON_RESTARTED: &str = "daemon restarted";

/// The columns of a unit's row that its events decide.
#[derive(Debug, Clone, PartialEq)]
pub struct Projection {
    pub phase: String,
    pub cost: f64,
    pub last_seq: u64,
    pub terminal_reason: Option<String>,
    pub oracle_hash: Option<String>,
    pub oracle_frozen: bool,
    pub approved_oracle_hash: Option<String>,
    pub elapsed_ms: u64,
    pub pause_reason: Option<String>,
    pub paused_from: Option<String>,
    pub pr_url: Option<String>,
}

fn wire(phase: Phase) -> String {
    serde_json::to_value(phase)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_else(|| "unknown".to_string())
}

impl Projection {
    /// The projection a stored unit has now.
    pub fn of(record: &UnitRecord) -> Self {
        Self {
            phase: record.row.phase.clone(),
            cost: record.row.cost,
            last_seq: record.row.last_seq,
            terminal_reason: record.row.terminal_reason.clone(),
            oracle_hash: record.row.oracle_hash.clone(),
            oracle_frozen: record.row.oracle_frozen,
            approved_oracle_hash: record.ext.approved_oracle_hash.clone(),
            elapsed_ms: record.ext.elapsed_ms,
            pause_reason: record.ext.pause_reason.clone(),
            paused_from: record.ext.paused_from.clone(),
            pr_url: record.ext.pr_url.clone(),
        }
    }

    /// Write this projection's columns into a record, leaving its set-once columns alone.
    pub fn write_into(&self, record: &mut UnitRecord) {
        record.row.phase = self.phase.clone();
        record.row.cost = self.cost;
        record.row.last_seq = self.last_seq;
        record.row.terminal_reason = self.terminal_reason.clone();
        record.row.oracle_hash = self.oracle_hash.clone();
        record.row.oracle_frozen = self.oracle_frozen;
        record.ext.approved_oracle_hash = self.approved_oracle_hash.clone();
        record.ext.elapsed_ms = self.elapsed_ms;
        record.ext.pause_reason = self.pause_reason.clone();
        record.ext.paused_from = self.paused_from.clone();
        record.ext.pr_url = self.pr_url.clone();
    }

    /// Fold one record of the log. `event` is `None` for a record that is not a
    /// `fleet_core::Event`. `harness_path` is true when the unit's `harness` column is set: four
    /// rules apply only then, because the in-process driver emits the same phase pairs with a
    /// different meaning.
    pub fn fold(&mut self, seq: u64, event: Option<&Event>, harness_path: bool) {
        self.last_seq = seq;
        // As the forwarder has always done: the reason is whatever the latest record says, so a
        // "daemon restarted" left by reconciliation is cleared by the next event.
        self.terminal_reason = None;
        let Some(event) = event else { return };
        match event {
            Event::PhaseChanged {
                from, to, reason, ..
            } => {
                self.phase = wire(*to);
                if matches!(to, Phase::NeedsHuman | Phase::Halted) {
                    self.pause_reason = reason.clone();
                    self.paused_from = Some(wire(*from));
                } else {
                    self.pause_reason = None;
                    self.paused_from = None;
                }
                if harness_path {
                    match (from, to) {
                        (Phase::AwaitingOracleApproval, Phase::Building) => {
                            self.approved_oracle_hash = self.oracle_hash.clone();
                        }
                        (Phase::AwaitingOracleApproval, Phase::Spec) => {
                            self.oracle_hash = None;
                            self.oracle_frozen = false;
                        }
                        (Phase::Spec, Phase::Building) => self.oracle_frozen = true,
                        (_, Phase::NeedsHuman)
                            if reason.as_deref() == Some(REASON_ORACLE_TAMPERING) =>
                        {
                            self.approved_oracle_hash = None;
                        }
                        _ => {}
                    }
                }
            }
            Event::Metric {
                cost_usd,
                elapsed_ms,
                ..
            } => {
                self.cost = *cost_usd;
                self.elapsed_ms = *elapsed_ms;
            }
            Event::Done { result } => self.terminal_reason = Some(result.clone()),
            Event::OracleProposed { hash, .. } => {
                self.oracle_hash = Some(hash.clone());
                self.oracle_frozen = true;
            }
            Event::Artifact {
                kind: ArtifactKind::Pr,
                reference,
            } => self.pr_url = Some(reference.clone()),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::record;

    fn changed(from: Phase, to: Phase, reason: Option<&str>) -> Event {
        Event::PhaseChanged {
            from,
            to,
            reason: reason.map(str::to_string),
            cmd_id: None,
        }
    }

    fn proposed(hash: &str) -> Event {
        Event::OracleProposed {
            test_files: vec!["t.test.js".into()],
            hash: hash.into(),
            summary: String::new(),
        }
    }

    fn folded(events: &[Event], harness_path: bool) -> Projection {
        let mut p = Projection::of(&record("u1"));
        for (i, event) in events.iter().enumerate() {
            p.fold(i as u64 + 1, Some(event), harness_path);
        }
        p
    }

    #[test]
    fn a_phase_change_sets_the_phase_and_every_record_moves_last_seq() {
        let p = folded(&[changed(Phase::Queued, Phase::Provisioning, None)], false);
        assert_eq!(p.phase, "provisioning");
        assert_eq!(p.last_seq, 1);
        let mut p = p;
        p.fold(2, None, false);
        assert_eq!((p.last_seq, p.phase.as_str()), (2, "provisioning"));
    }

    #[test]
    fn a_metric_is_cumulative_so_the_latest_one_wins() {
        let metric = |cost_usd, elapsed_ms| Event::Metric {
            tokens_in: 1,
            tokens_out: 1,
            cost_usd,
            elapsed_ms,
        };
        let p = folded(&[metric(0.25, 1_000), metric(0.75, 4_000)], true);
        assert_eq!((p.cost, p.elapsed_ms), (0.75, 4_000));
    }

    #[test]
    fn done_sets_the_reason_and_any_other_record_clears_it() {
        let mut p = Projection::of(&record("u1"));
        p.terminal_reason = Some(REASON_DAEMON_RESTARTED.into());
        p.fold(
            1,
            Some(&changed(Phase::Halted, Phase::Provisioning, None)),
            true,
        );
        assert_eq!(p.terminal_reason, None);
        p.fold(
            2,
            Some(&Event::Done {
                result: "done".into(),
            }),
            true,
        );
        assert_eq!(p.terminal_reason.as_deref(), Some("done"));
    }

    #[test]
    fn an_approved_gate_copies_the_proposed_hash_on_the_harness_path_only() {
        let events = [
            proposed("h1"),
            changed(Phase::AwaitingOracleApproval, Phase::Building, None),
        ];
        assert_eq!(
            folded(&events, true).approved_oracle_hash.as_deref(),
            Some("h1")
        );
        assert_eq!(folded(&events, false).approved_oracle_hash, None);
    }

    #[test]
    fn a_rejected_gate_clears_the_freeze_on_the_harness_path_only() {
        let events = [
            proposed("h1"),
            changed(Phase::AwaitingOracleApproval, Phase::Spec, None),
        ];
        let harness = folded(&events, true);
        assert_eq!((harness.oracle_hash, harness.oracle_frozen), (None, false));
        let driver = folded(&events, false);
        assert_eq!(
            (driver.oracle_hash.as_deref(), driver.oracle_frozen),
            (Some("h1"), true)
        );
    }

    #[test]
    fn a_t1_freeze_is_recorded_on_the_harness_path_only() {
        let events = [changed(Phase::Spec, Phase::Building, None)];
        assert!(folded(&events, true).oracle_frozen);
        assert!(!folded(&events, false).oracle_frozen);
    }

    #[test]
    fn tampering_clears_the_approval_and_is_kept_as_the_pause_reason() {
        let events = [
            proposed("h1"),
            changed(Phase::AwaitingOracleApproval, Phase::Building, None),
            changed(
                Phase::MergeCheck,
                Phase::NeedsHuman,
                Some(REASON_ORACLE_TAMPERING),
            ),
        ];
        let p = folded(&events, true);
        assert_eq!(p.approved_oracle_hash, None);
        assert_eq!(p.pause_reason.as_deref(), Some(REASON_ORACLE_TAMPERING));
        assert_eq!(p.paused_from.as_deref(), Some("merge_check"));
    }

    #[test]
    fn leaving_a_pause_clears_its_reason_and_origin() {
        let events = [
            changed(Phase::Building, Phase::NeedsHuman, Some("usd cap")),
            changed(Phase::NeedsHuman, Phase::Provisioning, None),
        ];
        let p = folded(&events, false);
        assert_eq!((p.pause_reason, p.paused_from), (None, None));
    }

    #[test]
    fn a_pr_artifact_is_kept_and_a_branch_artifact_is_not() {
        let artifact = |kind, reference: &str| Event::Artifact {
            kind,
            reference: reference.into(),
        };
        let p = folded(
            &[
                artifact(ArtifactKind::Branch, "factory/u1"),
                artifact(ArtifactKind::Pr, "https://example.invalid/pr/7"),
            ],
            true,
        );
        assert_eq!(p.pr_url.as_deref(), Some("https://example.invalid/pr/7"));
    }

    #[test]
    fn write_into_leaves_set_once_columns_alone() {
        let mut stored = record("u1");
        stored.ext.work_item = Some("owner/repo#7".into());
        let mut p = Projection::of(&stored);
        p.phase = "building".into();
        p.cost = 1.5;
        p.write_into(&mut stored);
        assert_eq!(stored.row.phase, "building");
        assert_eq!(stored.row.cost, 1.5);
        assert_eq!(stored.row.task, record("u1").row.task);
        assert_eq!(stored.ext.work_item.as_deref(), Some("owner/repo#7"));
    }
}
````

`crates/fleet-store/src/sqlite.rs`:

````rust
//! The SQLite store. One connection; a caller shares it as [`SharedStore`], whose mutex is the
//! single writer SQLite wants.
//!
//! Wave 0 gives the signatures only. Lane CC-STORE moves the bodies here from
//! `crates/fleetd/src/store.rs` without changing what they do, and adds the new ones.

use crate::error::StoreError;
use crate::projection::Projection;
use crate::traits::{SignatureStore, UnitStore};
use crate::types::{
    LaneRow, Reservation, Restarted, Signature, SignatureSummary, SwarmRow, UnitRecord, UnitRow,
};
use fleet_core::Event;
use std::path::Path;
use std::sync::Mutex;

/// The store as the rest of the control plane holds it: `Arc<SharedStore>` coerces to
/// `Arc<dyn UnitStore>` and to `Arc<dyn SignatureStore>`.
pub type SharedStore = Mutex<Store>;

pub struct Store {
    #[allow(dead_code)] // read by the bodies lane CC-STORE moves in
    conn: rusqlite::Connection,
}

const MOVE: &str = "lane CC-STORE moves this from crates/fleetd/src/store.rs";
const NEW: &str = "lane CC-STORE builds this";

// --- the methods the in-process driver path uses, with today's names and arguments ---
impl Store {
    /// Open, or create, the database file at `path`. A `file:` URI is accepted, as today.
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        let _ = path;
        unimplemented!("{MOVE}")
    }

    pub fn open_memory() -> Result<Self, StoreError> {
        unimplemented!("{MOVE}")
    }

    /// Insert a unit, or update the projection columns of one that exists. The set-once columns
    /// keep what the first write gave them.
    pub fn upsert_unit(&self, row: &UnitRow, now: i64) -> Result<(), StoreError> {
        let _ = (row, now);
        unimplemented!("{MOVE}")
    }

    /// Update the projection columns of an existing unit. A `None` hash leaves a stored hash and
    /// freeze alone; a `None` reason replaces the stored reason.
    #[allow(clippy::too_many_arguments)]
    pub fn update_unit(
        &self,
        unit_id: &str,
        phase: &str,
        cost: f64,
        last_seq: u64,
        terminal_reason: Option<&str>,
        oracle_hash: Option<&str>,
        now: i64,
    ) -> Result<(), StoreError> {
        let _ = (
            unit_id,
            phase,
            cost,
            last_seq,
            terminal_reason,
            oracle_hash,
            now,
        );
        unimplemented!("{MOVE}")
    }

    pub fn append_event(
        &self,
        unit_id: &str,
        seq: u64,
        ts: i64,
        json: &str,
    ) -> Result<(), StoreError> {
        let _ = (unit_id, seq, ts, json);
        unimplemented!("{MOVE}")
    }

    pub fn get_unit(&self, id: &str) -> Result<Option<UnitRow>, StoreError> {
        let _ = id;
        unimplemented!("{MOVE}")
    }

    pub fn list_units(&self) -> Result<Vec<UnitRow>, StoreError> {
        unimplemented!("{MOVE}")
    }

    pub fn events_since(&self, id: &str, since: u64) -> Result<Vec<String>, StoreError> {
        let _ = (id, since);
        unimplemented!("{MOVE}")
    }

    pub fn max_swarm_seq(&self) -> Result<u64, StoreError> {
        unimplemented!("{MOVE}")
    }

    pub fn max_unit_seq(&self) -> Result<u64, StoreError> {
        unimplemented!("{MOVE}")
    }

    pub fn committed_spend(&self, since_ts: i64) -> Result<f64, StoreError> {
        let _ = since_ts;
        unimplemented!("{MOVE}")
    }

    pub fn upsert_swarm(&self, row: &SwarmRow, now: i64) -> Result<(), StoreError> {
        let _ = (row, now);
        unimplemented!("{MOVE}")
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_swarm(
        &self,
        id: &str,
        status: &str,
        planner_cost: f64,
        lanes_launched: u32,
        lanes_dropped: u32,
        terminal_reason: Option<&str>,
        now: i64,
    ) -> Result<(), StoreError> {
        let _ = (
            id,
            status,
            planner_cost,
            lanes_launched,
            lanes_dropped,
            terminal_reason,
            now,
        );
        unimplemented!("{MOVE}")
    }

    pub fn get_swarm(&self, id: &str) -> Result<Option<SwarmRow>, StoreError> {
        let _ = id;
        unimplemented!("{MOVE}")
    }

    pub fn list_swarms(&self) -> Result<Vec<SwarmRow>, StoreError> {
        unimplemented!("{MOVE}")
    }

    /// `(total, finished, waiting for a person)` over a swarm's units.
    pub fn swarm_rollup(&self, swarm_id: &str) -> Result<(u64, u64, u64), StoreError> {
        let _ = swarm_id;
        unimplemented!("{MOVE}")
    }

    #[allow(clippy::too_many_arguments)]
    pub fn upsert_lane(
        &self,
        swarm_id: &str,
        idx: u32,
        title: &str,
        task: &str,
        rationale: &str,
        decision: &str,
        unit_id: Option<&str>,
    ) -> Result<(), StoreError> {
        let _ = (swarm_id, idx, title, task, rationale, decision, unit_id);
        unimplemented!("{MOVE}")
    }

    pub fn lanes_for_swarm(&self, swarm_id: &str) -> Result<Vec<LaneRow>, StoreError> {
        let _ = swarm_id;
        unimplemented!("{MOVE}")
    }

    /// Insert a lane's unit and link the lane to it, in one transaction.
    pub fn commit_lane_unit(
        &self,
        swarm_id: &str,
        idx: u32,
        unit: &UnitRow,
        now: i64,
    ) -> Result<(), StoreError> {
        let _ = (swarm_id, idx, unit, now);
        unimplemented!("{MOVE}")
    }
}

// --- new in milestone 1 ---
impl Store {
    /// Write every projection column of a unit exactly as given, including a `None` that clears
    /// one. A unit that does not exist is left absent.
    pub fn write_projection(
        &self,
        unit_id: &str,
        projection: &Projection,
        now: i64,
    ) -> Result<(), StoreError> {
        let _ = (unit_id, projection, now);
        unimplemented!("{NEW}")
    }
}

impl UnitStore for SharedStore {
    fn reserve_unit(
        &self,
        record: &UnitRecord,
        window_start_ms: i64,
        global_cap: f64,
        now_ms: i64,
    ) -> Result<Reservation, StoreError> {
        let _ = (record, window_start_ms, global_cap, now_ms);
        unimplemented!("{NEW}")
    }

    fn get_unit(&self, unit_id: &str) -> Result<Option<UnitRow>, StoreError> {
        let _ = unit_id;
        unimplemented!("{NEW}")
    }

    fn get_record(&self, unit_id: &str) -> Result<Option<UnitRecord>, StoreError> {
        let _ = unit_id;
        unimplemented!("{NEW}")
    }

    fn list_records(&self) -> Result<Vec<UnitRecord>, StoreError> {
        unimplemented!("{NEW}")
    }

    fn max_unit_seq(&self) -> Result<u64, StoreError> {
        unimplemented!("{NEW}")
    }

    fn find_by_idempotency_key(&self, key: &str) -> Result<Option<String>, StoreError> {
        let _ = key;
        unimplemented!("{NEW}")
    }

    fn committed_spend(&self, since_ms: i64) -> Result<f64, StoreError> {
        let _ = since_ms;
        unimplemented!("{NEW}")
    }

    fn record_event(
        &self,
        unit_id: &str,
        seq: u64,
        ts_ms: i64,
        json: &str,
        event: Option<&Event>,
    ) -> Result<(), StoreError> {
        let _ = (unit_id, seq, ts_ms, json, event);
        unimplemented!("{NEW}")
    }

    fn events_since(&self, unit_id: &str, since: u64) -> Result<Vec<String>, StoreError> {
        let _ = (unit_id, since);
        unimplemented!("{NEW}")
    }

    fn put_freeze(&self, unit_id: &str, freeze_json: &str, now_ms: i64) -> Result<(), StoreError> {
        let _ = (unit_id, freeze_json, now_ms);
        unimplemented!("{NEW}")
    }

    fn put_result(&self, unit_id: &str, result_json: &str, now_ms: i64) -> Result<(), StoreError> {
        let _ = (unit_id, result_json, now_ms);
        unimplemented!("{NEW}")
    }

    fn mark_restarted(&self, unit_id: &str, now_ms: i64) -> Result<Restarted, StoreError> {
        let _ = (unit_id, now_ms);
        unimplemented!("{NEW}")
    }
}

impl SignatureStore for SharedStore {
    fn put_signature(&self, signature: &Signature) -> Result<(), StoreError> {
        let _ = signature;
        unimplemented!("{NEW}")
    }

    fn get_signature(
        &self,
        repo_slug: &str,
        sha256: &str,
    ) -> Result<Option<Signature>, StoreError> {
        let _ = (repo_slug, sha256);
        unimplemented!("{NEW}")
    }

    fn list_signatures(&self, repo_slug: &str) -> Result<Vec<SignatureSummary>, StoreError> {
        let _ = repo_slug;
        unimplemented!("{NEW}")
    }
}
````

- [ ] **Step 2: Write the failing test**

Create `crates/fleet-store/src/testkit.rs` with only this:

````rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_in_memory_store_passes_the_unit_store_contract() {
        unit_store_contract(MemStore::new);
    }

    #[test]
    fn the_in_memory_store_passes_the_signature_store_contract() {
        signature_store_contract(MemStore::new);
    }

    #[test]
    fn a_failing_store_refuses_writes_and_reads_and_recovers() {
        let s = MemStore::new();
        s.fail_writes(true);
        assert!(s.reserve_unit(&record("u1"), 0, CAP, 0).is_err());
        s.fail_writes(false);
        assert_eq!(
            s.get_record("u1").unwrap(),
            None,
            "a failed write wrote nothing"
        );
        s.fail_reads(true);
        assert!(s.committed_spend(0).is_err());
        s.fail_reads(false);
        assert_eq!(s.committed_spend(0).unwrap(), 0.0);
    }

    #[test]
    fn seed_bypasses_the_ceiling_and_dates_the_unit() {
        let s = MemStore::new();
        let mut big = record("seed");
        big.row.usd_cap = 999.0;
        s.seed(big, 500);
        assert_eq!(s.committed_spend(500).unwrap(), 999.0);
        assert_eq!(s.committed_spend(501).unwrap(), 0.0);
    }
}
````

- [ ] **Step 3: Run it and see it fail**

```
cargo test -p fleet-store --lib
```

Expected: the crate does not compile. The errors name `MemStore` (E0433), `record`,
`unit_store_contract` and `signature_store_contract` (E0425), `CAP` (E0425), and
`crate::testkit::record` in `projection.rs` (E0432).

- [ ] **Step 4: Write the fake, the builders and the suites**

Replace `crates/fleet-store/src/testkit.rs` with:

````rust
//! Builders for this crate's types, an in-memory store, and the contract suites every
//! implementation of [`UnitStore`] and [`SignatureStore`] must pass.
//!
//! Compiled for this crate's own tests, and for other crates behind the `testkit` feature.

use crate::error::StoreError;
use crate::projection::{Projection, REASON_DAEMON_RESTARTED};
use crate::traits::{SignatureStore, UnitStore};
use crate::types::{
    sha256_hex, Reservation, Restarted, Signature, SignatureSummary, UnitExt, UnitRecord, UnitRow,
};
use fleet_core::{Event, Phase, TERMINAL_PHASE_STRS};
use std::collections::BTreeMap;
use std::sync::Mutex;

// ---------------------------------------------------------------------------------------------
// Builders
// ---------------------------------------------------------------------------------------------

/// A queued T1 demo unit with a $5 cap, as the in-process driver path writes one.
pub fn unit_row(id: &str) -> UnitRow {
    UnitRow {
        unit_id: id.into(),
        tier: "t1".into(),
        task: "first task".into(),
        repo_url: "https://example.invalid/repo".into(),
        repo_slug: "owner/repo".into(),
        base_branch: "main".into(),
        branch: format!("agent/{id}"),
        test_cmd: "node --test".into(),
        usd_cap: 5.0,
        wall_clock_secs: 1800,
        phase: "queued".into(),
        cost: 0.0,
        last_seq: 0,
        oracle_frozen: false,
        oracle_hash: None,
        terminal_reason: None,
        mode: "demo".into(),
        min_review_rounds: 2,
        swarm_id: None,
    }
}

/// [`unit_row`] with an empty extension: a unit the in-process driver runs.
pub fn record(id: &str) -> UnitRecord {
    UnitRecord {
        row: unit_row(id),
        ext: UnitExt::default(),
    }
}

/// A unit a harness runs: `mode` is `real`, and `harness` names the registry entry.
pub fn harness_record(id: &str, harness: &str) -> UnitRecord {
    let mut r = record(id);
    r.row.mode = "real".into();
    r.ext.harness = Some(harness.into());
    r
}

/// A signature over `bytes`, with the right hash, signed by `owner` at time 1000.
pub fn signature(repo_slug: &str, spec_id: &str, bytes: &[u8]) -> Signature {
    Signature {
        repo_slug: repo_slug.into(),
        spec_id: spec_id.into(),
        commit: "c0ffee0000000000000000000000000000000000".into(),
        sha256: sha256_hex(bytes),
        bytes: bytes.to_vec(),
        signed_by: "owner".into(),
        signed_at_ms: 1000,
    }
}

#[derive(serde::Serialize)]
struct Envelope<'a> {
    unit_id: &'a str,
    seq: u64,
    event: &'a Event,
}

/// The JSON the control plane logs for one event: `{"unit_id":…,"seq":…,"event":{…}}`.
pub fn envelope_json(unit_id: &str, seq: u64, event: &Event) -> String {
    serde_json::to_string(&Envelope {
        unit_id,
        seq,
        event,
    })
    .expect("an event serialises")
}

pub fn phase_changed(from: Phase, to: Phase, reason: Option<&str>) -> Event {
    Event::PhaseChanged {
        from,
        to,
        reason: reason.map(str::to_string),
        cmd_id: None,
    }
}

// ---------------------------------------------------------------------------------------------
// The in-memory store
// ---------------------------------------------------------------------------------------------

#[derive(Default)]
struct Inner {
    units: Vec<(UnitRecord, i64)>,
    events: BTreeMap<(String, u64), String>,
    signatures: Vec<Signature>,
    fail_writes: bool,
    fail_reads: bool,
}

/// An in-memory [`UnitStore`] and [`SignatureStore`]. It keeps the same rules as the SQLite
/// store, and can be told to fail so a caller's refusal paths can be tested.
#[derive(Default)]
pub struct MemStore {
    inner: Mutex<Inner>,
}

fn parse_phase(s: &str) -> Phase {
    serde_json::from_value(serde_json::Value::String(s.to_string())).unwrap_or(Phase::Queued)
}

impl MemStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// While on, every method that writes returns `Err` and writes nothing.
    pub fn fail_writes(&self, on: bool) {
        self.inner.lock().unwrap().fail_writes = on;
    }

    /// While on, every method that only reads returns `Err`.
    pub fn fail_reads(&self, on: bool) {
        self.inner.lock().unwrap().fail_reads = on;
    }

    /// Put a unit in the store as if it had been first written at `created_ms`, whatever the
    /// ceiling. For arranging a test; production code calls `reserve_unit`.
    pub fn seed(&self, record: UnitRecord, created_ms: i64) {
        self.inner.lock().unwrap().units.push((record, created_ms));
    }

    fn read<T>(&self, f: impl FnOnce(&Inner) -> T) -> Result<T, StoreError> {
        let inner = self.inner.lock().unwrap();
        if inner.fail_reads {
            return Err(StoreError::Failed("scripted read failure".into()));
        }
        Ok(f(&inner))
    }

    fn write<T>(&self, f: impl FnOnce(&mut Inner) -> T) -> Result<T, StoreError> {
        let mut inner = self.inner.lock().unwrap();
        if inner.fail_writes {
            return Err(StoreError::Failed("scripted write failure".into()));
        }
        Ok(f(&mut inner))
    }
}

fn spend(inner: &Inner, since_ms: i64) -> f64 {
    inner
        .units
        .iter()
        .filter(|(_, created)| *created >= since_ms)
        .map(|(u, _)| {
            if TERMINAL_PHASE_STRS.contains(&u.row.phase.as_str()) {
                u.row.cost
            } else {
                u.row.usd_cap.max(u.row.cost)
            }
        })
        .sum()
}

fn append_and_fold(inner: &mut Inner, unit_id: &str, seq: u64, json: &str, event: Option<&Event>) {
    let key = (unit_id.to_string(), seq);
    if inner.events.contains_key(&key) {
        return;
    }
    inner.events.insert(key, json.to_string());
    if let Some((unit, _)) = inner
        .units
        .iter_mut()
        .find(|(u, _)| u.row.unit_id == unit_id)
    {
        let mut projection = Projection::of(unit);
        projection.fold(seq, event, unit.ext.harness.is_some());
        projection.write_into(unit);
    }
}

impl UnitStore for MemStore {
    fn reserve_unit(
        &self,
        record: &UnitRecord,
        window_start_ms: i64,
        global_cap: f64,
        now_ms: i64,
    ) -> Result<Reservation, StoreError> {
        self.write(|inner| {
            if let Some(key) = &record.ext.idempotency_key {
                let holder = inner
                    .units
                    .iter()
                    .find(|(u, _)| u.ext.idempotency_key.as_ref() == Some(key));
                if let Some((u, _)) = holder {
                    return Reservation::DuplicateKey {
                        unit_id: u.row.unit_id.clone(),
                    };
                }
            }
            let committed = spend(inner, window_start_ms);
            if committed >= global_cap {
                return Reservation::OverCap { committed };
            }
            inner.units.push((record.clone(), now_ms));
            Reservation::Reserved
        })
    }

    fn get_unit(&self, unit_id: &str) -> Result<Option<UnitRow>, StoreError> {
        Ok(self.get_record(unit_id)?.map(|r| r.row))
    }

    fn get_record(&self, unit_id: &str) -> Result<Option<UnitRecord>, StoreError> {
        self.read(|inner| {
            inner
                .units
                .iter()
                .find(|(u, _)| u.row.unit_id == unit_id)
                .map(|(u, _)| u.clone())
        })
    }

    fn list_records(&self) -> Result<Vec<UnitRecord>, StoreError> {
        self.read(|inner| inner.units.iter().map(|(u, _)| u.clone()).collect())
    }

    fn max_unit_seq(&self) -> Result<u64, StoreError> {
        self.read(|inner| {
            inner
                .units
                .iter()
                .filter_map(|(u, _)| u.row.unit_id.strip_prefix('u')?.parse::<u64>().ok())
                .max()
                .unwrap_or(0)
        })
    }

    fn find_by_idempotency_key(&self, key: &str) -> Result<Option<String>, StoreError> {
        self.read(|inner| {
            inner
                .units
                .iter()
                .find(|(u, _)| u.ext.idempotency_key.as_deref() == Some(key))
                .map(|(u, _)| u.row.unit_id.clone())
        })
    }

    fn committed_spend(&self, since_ms: i64) -> Result<f64, StoreError> {
        self.read(|inner| spend(inner, since_ms))
    }

    fn record_event(
        &self,
        unit_id: &str,
        seq: u64,
        _ts_ms: i64,
        json: &str,
        event: Option<&Event>,
    ) -> Result<(), StoreError> {
        self.write(|inner| append_and_fold(inner, unit_id, seq, json, event))
    }

    fn events_since(&self, unit_id: &str, since: u64) -> Result<Vec<String>, StoreError> {
        self.read(|inner| {
            inner
                .events
                .iter()
                .filter(|((unit, seq), _)| unit == unit_id && *seq > since)
                .map(|(_, json)| json.clone())
                .collect()
        })
    }

    fn put_freeze(&self, unit_id: &str, freeze_json: &str, _now_ms: i64) -> Result<(), StoreError> {
        self.write(|inner| {
            if let Some((u, _)) = inner
                .units
                .iter_mut()
                .find(|(u, _)| u.row.unit_id == unit_id)
            {
                u.ext.freeze_json = Some(freeze_json.to_string());
            }
        })
    }

    fn put_result(&self, unit_id: &str, result_json: &str, _now_ms: i64) -> Result<(), StoreError> {
        self.write(|inner| {
            if let Some((u, _)) = inner
                .units
                .iter_mut()
                .find(|(u, _)| u.row.unit_id == unit_id)
            {
                u.ext.result_json = Some(result_json.to_string());
            }
        })
    }

    fn mark_restarted(&self, unit_id: &str, _now_ms: i64) -> Result<Restarted, StoreError> {
        self.write(|inner| {
            let Some((unit, _)) = inner.units.iter().find(|(u, _)| u.row.unit_id == unit_id) else {
                return Restarted::Untouched;
            };
            let phase = unit.row.phase.clone();
            if TERMINAL_PHASE_STRS.contains(&phase.as_str()) {
                return Restarted::Untouched;
            }
            if phase == "needs_human" || phase == "halted" {
                return Restarted::KeptForHuman;
            }
            let seq = unit.row.last_seq + 1;
            let event = phase_changed(
                parse_phase(&phase),
                Phase::Halted,
                Some(REASON_DAEMON_RESTARTED),
            );
            let json = envelope_json(unit_id, seq, &event);
            append_and_fold(inner, unit_id, seq, &json, Some(&event));
            let (unit, _) = inner
                .units
                .iter_mut()
                .find(|(u, _)| u.row.unit_id == unit_id)
                .expect("found above");
            unit.row.terminal_reason = Some(REASON_DAEMON_RESTARTED.into());
            Restarted::Halted { seq }
        })
    }
}

impl SignatureStore for MemStore {
    fn put_signature(&self, signature: &Signature) -> Result<(), StoreError> {
        let actual = sha256_hex(&signature.bytes);
        if actual != signature.sha256 {
            return Err(StoreError::HashMismatch {
                claimed: signature.sha256.clone(),
                actual,
            });
        }
        self.write(|inner| {
            inner
                .signatures
                .retain(|s| !(s.repo_slug == signature.repo_slug && s.sha256 == signature.sha256));
            inner.signatures.push(signature.clone());
        })
    }

    fn get_signature(
        &self,
        repo_slug: &str,
        sha256: &str,
    ) -> Result<Option<Signature>, StoreError> {
        self.read(|inner| {
            inner
                .signatures
                .iter()
                .find(|s| s.repo_slug == repo_slug && s.sha256 == sha256)
                .cloned()
        })
    }

    fn list_signatures(&self, repo_slug: &str) -> Result<Vec<SignatureSummary>, StoreError> {
        self.read(|inner| {
            let mut found: Vec<SignatureSummary> = inner
                .signatures
                .iter()
                .filter(|s| s.repo_slug == repo_slug)
                .map(Signature::summary)
                .collect();
            found.sort_by(|a, b| b.signed_at_ms.cmp(&a.signed_at_ms));
            found
        })
    }
}

// ---------------------------------------------------------------------------------------------
// Contract suites
// ---------------------------------------------------------------------------------------------

const CAP: f64 = 20.0;

fn reserved<S: UnitStore>(store: &S, record: &UnitRecord, now_ms: i64) {
    assert_eq!(
        store.reserve_unit(record, 0, CAP, now_ms).unwrap(),
        Reservation::Reserved,
        "reserving {}",
        record.row.unit_id
    );
}

fn log<S: UnitStore>(store: &S, unit_id: &str, seq: u64, event: &Event) {
    store
        .record_event(
            unit_id,
            seq,
            0,
            &envelope_json(unit_id, seq, event),
            Some(event),
        )
        .unwrap();
}

/// What every [`UnitStore`] must do. `make` returns a new, empty store each time it is called.
/// Panics on the first rule an implementation breaks.
pub fn unit_store_contract<S: UnitStore>(make: impl Fn() -> S) {
    // A reserved unit reads back exactly, row and extension.
    let s = make();
    let mut first = harness_record("u1", "reqdrive");
    first.ext.work_item = Some("owner/repo#7".into());
    first.ext.idempotency_key = Some("key-1".into());
    first.ext.spec_sha256 = Some("ab".repeat(32));
    reserved(&s, &first, 100);
    assert_eq!(s.get_record("u1").unwrap(), Some(first.clone()));
    assert_eq!(s.get_unit("u1").unwrap(), Some(first.row.clone()));
    assert_eq!(s.get_record("nobody").unwrap(), None);
    assert_eq!(
        s.find_by_idempotency_key("key-1").unwrap().as_deref(),
        Some("u1")
    );
    assert_eq!(s.find_by_idempotency_key("key-2").unwrap(), None);

    // A second unit with the same key is refused and not written.
    let mut again = harness_record("u2", "reqdrive");
    again.ext.idempotency_key = Some("key-1".into());
    assert_eq!(
        s.reserve_unit(&again, 0, CAP, 101).unwrap(),
        Reservation::DuplicateKey {
            unit_id: "u1".into()
        }
    );
    assert_eq!(s.get_record("u2").unwrap(), None);

    // Units list in the order they were first written, and ids of the form u<n> are counted.
    reserved(&s, &record("u7"), 102);
    reserved(&s, &record("lane-x"), 103);
    let ids: Vec<String> = s
        .list_records()
        .unwrap()
        .into_iter()
        .map(|r| r.row.unit_id)
        .collect();
    assert_eq!(ids, ["u1", "u7", "lane-x"]);
    assert_eq!(s.max_unit_seq().unwrap(), 7);
    assert_eq!(make().max_unit_seq().unwrap(), 0);

    // The ceiling: a reservation counts its cap, "at or above" refuses, nothing is written.
    let s = make();
    for (n, id) in ["a", "b", "c", "d"].iter().enumerate() {
        reserved(&s, &record(id), n as i64);
    }
    assert_eq!(s.committed_spend(0).unwrap(), 20.0);
    assert_eq!(
        s.reserve_unit(&record("e"), 0, CAP, 9).unwrap(),
        Reservation::OverCap { committed: 20.0 }
    );
    assert_eq!(s.get_record("e").unwrap(), None);
    // The window starts at `since`, boundary included.
    assert_eq!(s.committed_spend(2).unwrap(), 10.0);
    assert_eq!(s.committed_spend(4).unwrap(), 0.0);
    // A finished unit counts what it cost, not what it reserved.
    log(
        &s,
        "a",
        1,
        &Event::Metric {
            tokens_in: 1,
            tokens_out: 1,
            cost_usd: 0.5,
            elapsed_ms: 10,
        },
    );
    log(&s, "a", 2, &phase_changed(Phase::PrOpen, Phase::Done, None));
    assert_eq!(s.committed_spend(0).unwrap(), 15.5);

    // The log: ascending, byte for byte, `since` exclusive, one log per unit.
    let s = make();
    reserved(&s, &record("u1"), 0);
    reserved(&s, &record("u2"), 0);
    s.record_event("u1", 2, 0, "{\"n\": 2}", None).unwrap();
    s.record_event("u1", 1, 0, "{ \"n\":1 }", None).unwrap();
    s.record_event("u2", 1, 0, "other", None).unwrap();
    assert_eq!(
        s.events_since("u1", 0).unwrap(),
        ["{ \"n\":1 }", "{\"n\": 2}"]
    );
    assert_eq!(s.events_since("u1", 1).unwrap(), ["{\"n\": 2}"]);
    assert_eq!(s.events_since("u1", 2).unwrap(), Vec::<String>::new());
    assert_eq!(s.events_since("u2", 0).unwrap(), ["other"]);
    // A repeated sequence number keeps the first payload and is not an error.
    s.record_event("u1", 1, 0, "replacement", None).unwrap();
    assert_eq!(s.events_since("u1", 0).unwrap()[0], "{ \"n\":1 }");
    // A record with no typed event moves `last_seq` and nothing else.
    s.record_event("u1", 3, 0, "{\"n\": 3}", None).unwrap();
    let row = s.get_unit("u1").unwrap().unwrap();
    assert_eq!((row.last_seq, row.phase.as_str()), (3, "queued"));

    // The row is the fold of the log. On the harness path an approved gate copies the hash.
    let s = make();
    reserved(&s, &harness_record("h", "reqdrive"), 0);
    reserved(&s, &record("d"), 0);
    for unit in ["h", "d"] {
        log(
            &s,
            unit,
            1,
            &Event::OracleProposed {
                test_files: vec!["a.test.js".into()],
                hash: "hash-1".into(),
                summary: String::new(),
            },
        );
        log(
            &s,
            unit,
            2,
            &phase_changed(Phase::AwaitingOracleApproval, Phase::Building, None),
        );
        log(
            &s,
            unit,
            3,
            &phase_changed(Phase::Building, Phase::NeedsHuman, Some("usd cap")),
        );
    }
    let h = s.get_record("h").unwrap().unwrap();
    assert_eq!(h.row.phase, "needs_human");
    assert_eq!(h.row.oracle_hash.as_deref(), Some("hash-1"));
    assert!(h.row.oracle_frozen);
    assert_eq!(h.ext.approved_oracle_hash.as_deref(), Some("hash-1"));
    assert_eq!(h.ext.pause_reason.as_deref(), Some("usd cap"));
    assert_eq!(h.ext.paused_from.as_deref(), Some("building"));
    assert_eq!(
        s.get_record("d").unwrap().unwrap().ext.approved_oracle_hash,
        None
    );

    // The freeze and the result are kept, and later events do not disturb them.
    s.put_freeze("h", "{\"frozen_files\":[]}", 5).unwrap();
    s.put_result("h", "{\"outcome\":\"pr_open\"}", 5).unwrap();
    log(
        &s,
        "h",
        4,
        &phase_changed(Phase::NeedsHuman, Phase::Provisioning, None),
    );
    let h = s.get_record("h").unwrap().unwrap();
    assert_eq!(h.ext.freeze_json.as_deref(), Some("{\"frozen_files\":[]}"));
    assert_eq!(
        h.ext.result_json.as_deref(),
        Some("{\"outcome\":\"pr_open\"}")
    );
    assert_eq!((h.ext.pause_reason, h.ext.paused_from), (None, None));

    // Restart: a running unit is halted with one synthetic event, and keeps its cost and hash.
    let s = make();
    let mut running = record("run");
    running.row.phase = "building".into();
    running.row.cost = 1.25;
    running.row.last_seq = 6;
    running.row.oracle_hash = Some("hash-9".into());
    running.row.oracle_frozen = true;
    reserved(&s, &running, 0);
    assert_eq!(
        s.mark_restarted("run", 50).unwrap(),
        Restarted::Halted { seq: 7 }
    );
    let after = s.get_record("run").unwrap().unwrap();
    assert_eq!(after.row.phase, "halted");
    assert_eq!(after.row.last_seq, 7);
    assert_eq!(after.row.cost, 1.25);
    assert_eq!(after.row.oracle_hash.as_deref(), Some("hash-9"));
    assert_eq!(
        after.row.terminal_reason.as_deref(),
        Some(REASON_DAEMON_RESTARTED)
    );
    assert_eq!(after.ext.paused_from.as_deref(), Some("building"));
    let logged: serde_json::Value =
        serde_json::from_str(&s.events_since("run", 6).unwrap()[0]).unwrap();
    assert_eq!(
        logged,
        serde_json::json!({"unit_id": "run", "seq": 7, "event": {
            "type": "phase_changed", "from": "building", "to": "halted",
            "reason": REASON_DAEMON_RESTARTED
        }})
    );
    // Restart: a unit waiting for a person is left exactly as it is, however often.
    let mut waiting = record("wait");
    waiting.row.phase = "needs_human".into();
    waiting.row.last_seq = 3;
    waiting.ext.pause_reason = Some("oracle tampering".into());
    waiting.ext.paused_from = Some("merge_check".into());
    reserved(&s, &waiting, 0);
    for _ in 0..2 {
        assert_eq!(
            s.mark_restarted("wait", 50).unwrap(),
            Restarted::KeptForHuman
        );
        assert_eq!(
            s.mark_restarted("run", 50).unwrap(),
            Restarted::KeptForHuman
        );
    }
    assert_eq!(s.get_record("wait").unwrap(), Some(waiting));
    assert_eq!(s.events_since("wait", 0).unwrap(), Vec::<String>::new());
    assert_eq!(s.events_since("run", 0).unwrap().len(), 1);
    // Restart: finished and unknown units are untouched.
    let mut done = record("done");
    done.row.phase = "done".into();
    reserved(&s, &done, 0);
    assert_eq!(s.mark_restarted("done", 50).unwrap(), Restarted::Untouched);
    assert_eq!(
        s.mark_restarted("nobody", 50).unwrap(),
        Restarted::Untouched
    );
    assert_eq!(s.get_record("done").unwrap(), Some(done));
}

/// What every [`SignatureStore`] must do. `make` returns a new, empty store each time.
pub fn signature_store_contract<S: SignatureStore>(make: impl Fn() -> S) {
    let s = make();
    // The bytes come back exactly, including a byte-order mark and CRLF line endings.
    let bytes = b"\xEF\xBB\xBF---\r\nid: SPEC-1\r\n---\r\n";
    let signed = signature("owner/repo", "SPEC-1", bytes);
    s.put_signature(&signed).unwrap();
    assert_eq!(
        s.get_signature("owner/repo", &signed.sha256).unwrap(),
        Some(signed.clone())
    );
    // A signature belongs to one repository and one exact hash.
    assert_eq!(s.get_signature("other/repo", &signed.sha256).unwrap(), None);
    assert_eq!(
        s.get_signature("owner/repo", &"0".repeat(64)).unwrap(),
        None
    );

    // A signature whose hash is not the hash of its bytes is refused and not stored.
    let mut forged = signature("owner/repo", "SPEC-2", b"what was read");
    forged.bytes = b"what is stored".to_vec();
    assert!(matches!(
        s.put_signature(&forged),
        Err(StoreError::HashMismatch { .. })
    ));
    assert_eq!(s.get_signature("owner/repo", &forged.sha256).unwrap(), None);

    // Signing the same bytes again replaces who and when; it does not add an entry.
    let mut resigned = signed.clone();
    resigned.signed_by = "second".into();
    resigned.signed_at_ms = 3000;
    s.put_signature(&resigned).unwrap();
    assert_eq!(
        s.get_signature("owner/repo", &signed.sha256).unwrap(),
        Some(resigned.clone())
    );

    // Listings are per repository, newest first, and carry no bytes.
    let mut older = signature("owner/repo", "SPEC-3", b"three");
    older.signed_at_ms = 2000;
    s.put_signature(&older).unwrap();
    s.put_signature(&signature("other/repo", "SPEC-9", b"nine"))
        .unwrap();
    let listed = s.list_signatures("owner/repo").unwrap();
    assert_eq!(listed, [resigned.summary(), older.summary()]);
    assert_eq!(s.list_signatures("nobody/none").unwrap(), []);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_in_memory_store_passes_the_unit_store_contract() {
        unit_store_contract(MemStore::new);
    }

    #[test]
    fn the_in_memory_store_passes_the_signature_store_contract() {
        signature_store_contract(MemStore::new);
    }

    #[test]
    fn a_failing_store_refuses_writes_and_reads_and_recovers() {
        let s = MemStore::new();
        s.fail_writes(true);
        assert!(s.reserve_unit(&record("u1"), 0, CAP, 0).is_err());
        s.fail_writes(false);
        assert_eq!(
            s.get_record("u1").unwrap(),
            None,
            "a failed write wrote nothing"
        );
        s.fail_reads(true);
        assert!(s.committed_spend(0).is_err());
        s.fail_reads(false);
        assert_eq!(s.committed_spend(0).unwrap(), 0.0);
    }

    #[test]
    fn seed_bypasses_the_ceiling_and_dates_the_unit() {
        let s = MemStore::new();
        let mut big = record("seed");
        big.row.usd_cap = 999.0;
        s.seed(big, 500);
        assert_eq!(s.committed_spend(500).unwrap(), 999.0);
        assert_eq!(s.committed_spend(501).unwrap(), 0.0);
    }
}
````

- [ ] **Step 5: Run the tests**

```
cargo test -p fleet-store --lib
cargo test -p fleet-store --lib --features testkit
```

Expected, both times: `test result: ok. 14 passed; 0 failed`.

- [ ] **Step 6: Commit**

```
git add crates/fleet-store/src
git commit -m "feat(fleet-store): the store seams, the event fold, an in-memory store and its contract suites"
```

### Task 3: `fleet-forge` — the forge seams

**Files:**
- Replace: `crates/fleet-forge/src/lib.rs`
- Create: `crates/fleet-forge/src/legacy.rs`, `factory.rs`, `git.rs`, `testkit.rs`

**Produces:** the moved `Forge`, `ForgeError`, `MergeResult`, `Mergeability`, `GhForge`; the new
`RepoForge` and `Scratch` seams with `RepoRef`, `WorkItemRef`, `PrRequest`, `PrRef`, `PrState`,
`PrLifecycle`, `Change`, `ChangeKind`; `pr_body`, `body_carries`, `safe_branch`; `GitForge<H>`,
the `PrHost` seam, `GhCli`, `LocalPrHost`. In `testkit`: `MemForge`, `ForgeFixture`,
`MemFixture`, `FileChange`, `repo`, `pr_request`, `repo_forge_contract`.

What to know:

- `Forge` is today's trait, unchanged; only the driver path uses it.
- `RepoForge` serves every repository from one value and hands back a `Scratch` for a delivered
  bundle. A `Scratch` can read, diff, export and trial-merge; it cannot push. Pushing happens in
  `open_pr`, from the forge's own clone, and only for the commit the request names.
- `GitForge<H>` separates `git` from the pull-request host. `GitForge<GhCli>` is production;
  `GitForge<LocalPrHost>` is the hermetic suite's forge: real `git` against a local bare
  repository, pull requests as files.
- `MemForge` models commits as snapshots, so `changes`, `read`, `export_tree` and `trial_merge`
  behave like the real thing for the cases the suite checks.

- [ ] **Step 1: Create the source files other than `testkit.rs`**

`crates/fleet-forge/src/lib.rs`:

````rust
//! `fleet-forge` — everything the control plane does with git and with the pull-request host.
//! Credentials for pushing live here, on the host, and never in a container.
//!
//! Two traits. [`Forge`] is the three-call seam the in-process driver has always used; it moves
//! here unchanged and goes when that driver does. [`RepoForge`] is the factory's forge: it reads
//! a repository at a commit, cuts the source bundle a harness starts from, imports the bundle a
//! harness delivers into a [`Scratch`] clone that holds no credential and runs no hook, and
//! opens the pull request from its own, separate clone.

mod factory;
mod git;
mod legacy;

#[cfg(any(test, feature = "testkit"))]
pub mod testkit;

pub use factory::{
    body_carries, pr_body, safe_branch, Change, ChangeKind, PrLifecycle, PrRef, PrRequest, PrState,
    RepoForge, RepoRef, Scratch, WorkItemRef,
};
pub use git::{GhCli, GitForge, LocalPrHost, PrHost};
pub use legacy::{Forge, ForgeError, GhForge, MergeResult, Mergeability};
````

`crates/fleet-forge/src/legacy.rs`:

````rust
//! The forge seam of the in-process driver, moved from `crates/fleetd/src/forge.rs` and
//! `gh_forge.rs` without change. It is deleted with that driver.

use async_trait::async_trait;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MergeResult {
    Clean,
    Conflict,
}

/// The pull-request host works out mergeability in the background; `Pending` means "ask again".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mergeability {
    Mergeable,
    Dirty,
    Pending,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ForgeError {
    #[error("forge failure: {0}")]
    Failed(String),
}

#[async_trait]
pub trait Forge: Send + Sync {
    /// Import the bundle into the host clone, fetch a fresh base, and trial-merge.
    async fn trial_merge(&self, bundle: &Path, branch: &str) -> Result<MergeResult, ForgeError>;
    /// Push the branch and open a pull request; returns its URL.
    async fn open_pr(&self, branch: &str) -> Result<String, ForgeError>;
    /// One poll of the open pull request's mergeability.
    async fn poll_mergeable(&self, pr_url: &str) -> Result<Mergeability, ForgeError>;
}

/// [`Forge`] over host `git` and the `gh` command.
pub struct GhForge {
    #[allow(dead_code)] // read by the bodies lane CC-FORGE moves in
    repo_url: String,
    #[allow(dead_code)]
    repo_slug: String,
    #[allow(dead_code)]
    base_branch: String,
    #[allow(dead_code)]
    host_clone: PathBuf,
    #[allow(dead_code)]
    title: String,
}

impl GhForge {
    pub fn new(
        repo_url: impl Into<String>,
        repo_slug: impl Into<String>,
        base_branch: impl Into<String>,
        host_clone: PathBuf,
        title: impl Into<String>,
    ) -> Self {
        Self {
            repo_url: repo_url.into(),
            repo_slug: repo_slug.into(),
            base_branch: base_branch.into(),
            host_clone,
            title: title.into(),
        }
    }
}

#[async_trait]
impl Forge for GhForge {
    async fn trial_merge(&self, bundle: &Path, branch: &str) -> Result<MergeResult, ForgeError> {
        let _ = (bundle, branch);
        unimplemented!("lane CC-FORGE moves this from crates/fleetd/src/gh_forge.rs")
    }

    async fn open_pr(&self, branch: &str) -> Result<String, ForgeError> {
        let _ = branch;
        unimplemented!("lane CC-FORGE moves this from crates/fleetd/src/gh_forge.rs")
    }

    async fn poll_mergeable(&self, pr_url: &str) -> Result<Mergeability, ForgeError> {
        let _ = pr_url;
        unimplemented!("lane CC-FORGE moves this from crates/fleetd/src/gh_forge.rs")
    }
}
````

`crates/fleet-forge/src/factory.rs`:

````rust
//! The factory's forge: the types, the two traits, and the pure rules for a pull request's body.

use crate::legacy::{ForgeError, MergeResult, Mergeability};
use async_trait::async_trait;
use std::path::Path;

/// One onboarded repository.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RepoRef {
    /// `owner/name`.
    pub slug: String,
    /// What `git` fetches from and pushes to.
    pub url: String,
    pub base_branch: String,
}

/// A work item: an issue, named as `owner/repo#N`, with an optional fingerprint alias.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkItemRef {
    pub repo_slug: String,
    pub number: u64,
    pub fingerprint: Option<String>,
}

impl WorkItemRef {
    /// Parse `owner/repo#N`. Anything else is refused: an empty owner or name, no `#`, a number
    /// that is not decimal digits or is zero.
    pub fn parse(reference: &str, fingerprint: Option<&str>) -> Result<Self, ForgeError> {
        let bad = || ForgeError::Failed(format!("not a work item reference: {reference:?}"));
        let (slug, number) = reference.split_once('#').ok_or_else(bad)?;
        let (owner, name) = slug.split_once('/').ok_or_else(bad)?;
        let name_ok = |s: &str| {
            !s.is_empty()
                && s.chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        };
        if !name_ok(owner) || !name_ok(name) {
            return Err(bad());
        }
        if number.is_empty() || !number.bytes().all(|b| b.is_ascii_digit()) {
            return Err(bad());
        }
        let number: u64 = number.parse().map_err(|_| bad())?;
        if number == 0 {
            return Err(bad());
        }
        Ok(Self {
            repo_slug: slug.to_string(),
            number,
            fingerprint: fingerprint.map(str::to_string),
        })
    }

    /// `owner/repo#N`.
    pub fn reference(&self) -> String {
        format!("{}#{}", self.repo_slug, self.number)
    }
}

fn fixes_line(work_item: &WorkItemRef, pr_repo_slug: &str) -> String {
    if work_item.repo_slug.eq_ignore_ascii_case(pr_repo_slug) {
        format!("Fixes #{}", work_item.number)
    } else {
        format!("Fixes {}", work_item.reference())
    }
}

fn trailer_line(work_item: &WorkItemRef) -> String {
    match &work_item.fingerprint {
        Some(fp) => format!("Work-Item: {} tt:{fp}", work_item.reference()),
        None => format!("Work-Item: {}", work_item.reference()),
    }
}

/// The body of a unit's pull request: the summary, a `Fixes` line that closes the issue when the
/// pull request merges, and a `Work-Item:` trailer as the last line. `pr_repo_slug` is the
/// repository the pull request is opened in; an issue in another repository is named in full.
pub fn pr_body(work_item: &WorkItemRef, pr_repo_slug: &str, summary: &str) -> String {
    format!(
        "{}\n\n{}\n\n{}\n",
        summary.trim_end(),
        fixes_line(work_item, pr_repo_slug),
        trailer_line(work_item)
    )
}

/// True when `body` links `work_item` the way [`pr_body`] writes it: one whole line is the
/// `Fixes` line, and one whole line is the `Work-Item:` trailer. A reference inside a sentence,
/// to another issue, or with another fingerprint does not count.
pub fn body_carries(body: &str, work_item: &WorkItemRef, pr_repo_slug: &str) -> bool {
    let fixes = fixes_line(work_item, pr_repo_slug);
    let trailer = trailer_line(work_item);
    let lines: Vec<&str> = body.lines().map(str::trim_end).collect();
    lines.iter().any(|l| *l == fixes) && lines.iter().any(|l| *l == trailer)
}

/// True for a branch name that is safe to pass to `git` and to the pull-request host as an
/// argument: not empty, no leading `-`, no `..`, and only letters, digits, `_`, `.`, `/`, `-`.
pub fn safe_branch(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('-')
        && !name.contains("..")
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '/' | '-'))
}

/// What to open a pull request for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrRequest {
    pub repo: RepoRef,
    /// The unit whose imported bundle holds the branch.
    pub unit_id: String,
    pub branch: String,
    /// The commit that was verified. The forge pushes exactly this commit and refuses when the
    /// imported branch is at any other.
    pub head_sha: String,
    pub title: String,
    /// See [`pr_body`].
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrRef {
    pub url: String,
    pub number: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrLifecycle {
    Open,
    Merged,
    Closed,
}

/// A pull request as the host reports it now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrState {
    pub lifecycle: PrLifecycle,
    pub mergeable: Mergeability,
    pub body: String,
    pub head_sha: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ChangeKind {
    Added,
    Modified,
    Deleted,
}

/// One changed path between two commits. A rename is reported as a `Deleted` and an `Added`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Change {
    /// From the repository root, with `/` separators.
    pub path: String,
    pub kind: ChangeKind,
}

/// The factory's forge. One value serves every onboarded repository.
#[async_trait]
pub trait RepoForge: Send + Sync {
    /// Fetch the repository and return the commit its base branch is at now.
    async fn resolve_base(&self, repo: &RepoRef) -> Result<String, ForgeError>;

    /// The bytes of one file at one commit, exactly as stored; `None` when the commit has no
    /// such file. An unknown commit is an error.
    async fn read_file(
        &self,
        repo: &RepoRef,
        commit: &str,
        path: &str,
    ) -> Result<Option<Vec<u8>>, ForgeError>;

    /// The files at `commit` whose path starts with `prefix`, sorted.
    async fn list_files(
        &self,
        repo: &RepoRef,
        commit: &str,
        prefix: &str,
    ) -> Result<Vec<String>, ForgeError>;

    /// Write a bundle of the repository at `base_sha` to `dest`: what a harness starts from.
    async fn source_bundle(
        &self,
        repo: &RepoRef,
        base_sha: &str,
        dest: &Path,
    ) -> Result<(), ForgeError>;

    /// Import the bundle a harness delivered into a scratch clone of its own: no remote, no
    /// credential, hooks disabled. `branch` must be in the bundle and must descend from
    /// `base_sha`. Importing again for the same unit replaces the earlier scratch clone.
    async fn import_bundle(
        &self,
        repo: &RepoRef,
        unit_id: &str,
        base_sha: &str,
        bundle: &Path,
        branch: &str,
    ) -> Result<Box<dyn Scratch>, ForgeError>;

    /// Push the unit's imported branch from the forge's own clone and open the pull request.
    async fn open_pr(&self, request: &PrRequest) -> Result<PrRef, ForgeError>;

    /// Read a pull request's state from the host.
    async fn pr_state(&self, repo: &RepoRef, pr_url: &str) -> Result<PrState, ForgeError>;
}

/// A unit's delivered branch, in a clone made for verification. Nothing here can push.
#[async_trait]
pub trait Scratch: Send + Sync {
    fn base_sha(&self) -> &str;
    /// The commit the delivered branch is at.
    fn head_sha(&self) -> &str;
    /// The commits after `base_sha` up to the head, oldest first.
    async fn commits(&self) -> Result<Vec<String>, ForgeError>;
    /// The paths that differ between two commits, sorted by path.
    async fn changes(&self, from: &str, to: &str) -> Result<Vec<Change>, ForgeError>;
    async fn read(&self, commit: &str, path: &str) -> Result<Option<Vec<u8>>, ForgeError>;
    /// Write the tree at `commit` into the empty directory `dest` as plain files, with no `.git`.
    /// This copy is what a check container is given.
    async fn export_tree(&self, commit: &str, dest: &Path) -> Result<(), ForgeError>;
    /// Merge the head onto the base branch's current tip, in memory or in a throwaway worktree,
    /// and say whether it merged.
    async fn trial_merge(&self) -> Result<MergeResult, ForgeError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(fp: Option<&str>) -> WorkItemRef {
        WorkItemRef::parse("owner/repo#12", fp).unwrap()
    }

    #[test]
    fn a_work_item_reference_parses_and_prints_back() {
        let w = item(None);
        assert_eq!((w.repo_slug.as_str(), w.number), ("owner/repo", 12));
        assert_eq!(w.reference(), "owner/repo#12");
    }

    #[test]
    fn malformed_work_item_references_are_refused() {
        for bad in [
            "",
            "owner/repo",
            "owner/repo#",
            "owner/repo#0",
            "owner/repo#-1",
            "owner/repo#1x",
            "owner#1",
            "/repo#1",
            "owner/#1",
            "owner/re po#1",
            "owner/repo#99999999999999999999999",
        ] {
            assert!(WorkItemRef::parse(bad, None).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn the_body_closes_the_issue_and_ends_with_the_trailer() {
        let body = pr_body(&item(None), "owner/repo", "Adds percent codes.\n\n");
        assert_eq!(
            body,
            "Adds percent codes.\n\nFixes #12\n\nWork-Item: owner/repo#12\n"
        );
        assert!(body_carries(&body, &item(None), "owner/repo"));
    }

    #[test]
    fn an_issue_in_another_repository_is_named_in_full() {
        let body = pr_body(&item(Some("abc123")), "owner/sandbox", "x");
        assert!(body.contains("\nFixes owner/repo#12\n"));
        assert!(body.ends_with("Work-Item: owner/repo#12 tt:abc123\n"));
        assert!(body_carries(&body, &item(Some("abc123")), "owner/sandbox"));
    }

    #[test]
    fn a_body_missing_either_line_or_naming_another_item_does_not_carry_it() {
        let w = item(None);
        assert!(!body_carries("Fixes #12\n", &w, "owner/repo"));
        assert!(!body_carries(
            "Work-Item: owner/repo#12\n",
            &w,
            "owner/repo"
        ));
        assert!(!body_carries(
            "Fixes #13\n\nWork-Item: owner/repo#12\n",
            &w,
            "owner/repo"
        ));
        assert!(!body_carries(
            "This fixes #12 maybe.\n\nWork-Item: owner/repo#12\n",
            &w,
            "owner/repo"
        ));
        assert!(!body_carries(
            "Fixes #12\n\nWork-Item: owner/repo#123\n",
            &w,
            "owner/repo"
        ));
        assert!(!body_carries(
            "Fixes #12\n\nWork-Item: owner/repo#12 tt:other\n",
            &item(Some("abc123")),
            "owner/repo"
        ));
    }

    #[test]
    fn a_body_with_crlf_line_endings_still_carries_the_item() {
        let body = "Summary\r\n\r\nFixes #12\r\n\r\nWork-Item: owner/repo#12\r\n";
        assert!(body_carries(body, &item(None), "owner/repo"));
    }

    #[test]
    fn branch_names_that_could_be_read_as_options_or_paths_are_unsafe() {
        for good in ["factory/u7", "agent/sw1/0-lane.one", "a_b-c"] {
            assert!(safe_branch(good), "{good}");
        }
        for bad in ["", "-x", "a..b", "a b", "a;b", "a\\b", "a:b", "a~1", "a^"] {
            assert!(!safe_branch(bad), "{bad:?}");
        }
    }
}
````

`crates/fleet-forge/src/git.rs`:

````rust
//! [`RepoForge`] over the host's `git`, with the pull-request host behind its own seam so that a
//! hermetic run can use a local bare repository and record pull requests in a directory.
//!
//! Wave 0 gives the signatures only; lane CC-FORGE builds the bodies.

use crate::factory::{PrRef, PrRequest, PrState, RepoForge, RepoRef, Scratch};
use crate::legacy::ForgeError;
use async_trait::async_trait;
use std::path::{Path, PathBuf};

const NEW: &str = "lane CC-FORGE builds this";

/// Where pull requests are opened and read. `git push` is not part of it: [`GitForge`] pushes.
#[async_trait]
pub trait PrHost: Send + Sync {
    /// Open a pull request for a branch that has already been pushed.
    async fn create(&self, request: &PrRequest) -> Result<PrRef, ForgeError>;
    async fn view(&self, repo: &RepoRef, pr_url: &str) -> Result<PrState, ForgeError>;
}

/// GitHub, through the `gh` command and whatever credential it holds.
#[derive(Debug, Default, Clone)]
pub struct GhCli;

#[async_trait]
impl PrHost for GhCli {
    async fn create(&self, request: &PrRequest) -> Result<PrRef, ForgeError> {
        let _ = request;
        unimplemented!("{NEW}")
    }

    async fn view(&self, repo: &RepoRef, pr_url: &str) -> Result<PrState, ForgeError> {
        let _ = (repo, pr_url);
        unimplemented!("{NEW}")
    }
}

/// A pull-request host that is a directory: each pull request is one JSON file, and its state is
/// whatever that file says. For hermetic runs against a local bare repository; it reaches no
/// network.
#[derive(Debug, Clone)]
pub struct LocalPrHost {
    #[allow(dead_code)] // read by the bodies lane CC-FORGE builds
    dir: PathBuf,
}

impl LocalPrHost {
    /// `dir` is created if it does not exist.
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    /// The file that records pull request `number`: `<dir>/pr-<number>.json`.
    pub fn record_path(&self, number: u64) -> PathBuf {
        let _ = number;
        unimplemented!("{NEW}")
    }
}

#[async_trait]
impl PrHost for LocalPrHost {
    async fn create(&self, request: &PrRequest) -> Result<PrRef, ForgeError> {
        let _ = request;
        unimplemented!("{NEW}")
    }

    async fn view(&self, repo: &RepoRef, pr_url: &str) -> Result<PrState, ForgeError> {
        let _ = (repo, pr_url);
        unimplemented!("{NEW}")
    }
}

/// [`RepoForge`] over host `git`. Under `root` it keeps, per repository, one clone it fetches
/// into and pushes from, and per unit one scratch clone. The two never share a directory.
pub struct GitForge<H: PrHost> {
    #[allow(dead_code)] // read by the bodies lane CC-FORGE builds
    root: PathBuf,
    #[allow(dead_code)]
    host: H,
}

impl<H: PrHost> GitForge<H> {
    pub fn new(root: PathBuf, host: H) -> Self {
        Self { root, host }
    }

    /// The directory of a unit's scratch clone. A check container may be given a copy exported
    /// from it; nothing may be given [`GitForge::push_clone_dir`].
    pub fn scratch_dir(&self, unit_id: &str) -> PathBuf {
        let _ = unit_id;
        unimplemented!("{NEW}")
    }

    /// The directory of the clone this forge pushes from for `repo`.
    pub fn push_clone_dir(&self, repo: &RepoRef) -> PathBuf {
        let _ = repo;
        unimplemented!("{NEW}")
    }
}

#[async_trait]
impl<H: PrHost> RepoForge for GitForge<H> {
    async fn resolve_base(&self, repo: &RepoRef) -> Result<String, ForgeError> {
        let _ = repo;
        unimplemented!("{NEW}")
    }

    async fn read_file(
        &self,
        repo: &RepoRef,
        commit: &str,
        path: &str,
    ) -> Result<Option<Vec<u8>>, ForgeError> {
        let _ = (repo, commit, path);
        unimplemented!("{NEW}")
    }

    async fn list_files(
        &self,
        repo: &RepoRef,
        commit: &str,
        prefix: &str,
    ) -> Result<Vec<String>, ForgeError> {
        let _ = (repo, commit, prefix);
        unimplemented!("{NEW}")
    }

    async fn source_bundle(
        &self,
        repo: &RepoRef,
        base_sha: &str,
        dest: &Path,
    ) -> Result<(), ForgeError> {
        let _ = (repo, base_sha, dest);
        unimplemented!("{NEW}")
    }

    async fn import_bundle(
        &self,
        repo: &RepoRef,
        unit_id: &str,
        base_sha: &str,
        bundle: &Path,
        branch: &str,
    ) -> Result<Box<dyn Scratch>, ForgeError> {
        let _ = (repo, unit_id, base_sha, bundle, branch);
        unimplemented!("{NEW}")
    }

    async fn open_pr(&self, request: &PrRequest) -> Result<PrRef, ForgeError> {
        let _ = request;
        unimplemented!("{NEW}")
    }

    async fn pr_state(&self, repo: &RepoRef, pr_url: &str) -> Result<PrState, ForgeError> {
        let _ = (repo, pr_url);
        unimplemented!("{NEW}")
    }
}
````

- [ ] **Step 2: Write the failing test**

Create `crates/fleet-forge/src/testkit.rs` with only this:

````rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn the_in_memory_forge_passes_the_repo_forge_contract() {
        let scratch = tempfile::tempdir().unwrap();
        let n = std::sync::atomic::AtomicU32::new(0);
        repo_forge_contract(|| {
            let i = n.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let dir = scratch.path().join(format!("world-{i}"));
            std::fs::create_dir_all(&dir).unwrap();
            MemFixture::new(dir)
        })
        .await;
    }

    #[tokio::test]
    async fn a_scripted_failure_fails_one_operation_until_healed() {
        let forge = MemForge::new();
        let base = forge.commit_base(&repo(), &[("a.txt", Some("a"))]);
        forge.fail("resolve_base");
        assert!(forge.resolve_base(&repo()).await.is_err());
        assert_eq!(
            forge.read_file(&repo(), &base, "a.txt").await.unwrap(),
            Some(b"a".to_vec())
        );
        forge.heal("resolve_base");
        assert_eq!(forge.resolve_base(&repo()).await.unwrap(), base);
        assert_eq!(forge.calls(), ["resolve_base", "read_file", "resolve_base"]);
    }

    #[tokio::test]
    async fn a_pull_request_reports_what_the_test_sets() {
        let dir = tempfile::tempdir().unwrap();
        let forge = MemForge {
            mergeable: Mergeability::Pending,
            ..MemForge::default()
        };
        let base = forge.commit_base(&repo(), &[("a.txt", Some("a"))]);
        let bundle = dir.path().join("u1.bundle");
        let head =
            forge.write_unit_bundle(&base, "factory/u1", &[&[("b.txt", Some("b"))]], &bundle);
        forge
            .import_bundle(&repo(), "u1", &base, &bundle, "factory/u1")
            .await
            .unwrap();
        let pr = forge
            .open_pr(&pr_request("u1", "factory/u1", &head))
            .await
            .unwrap();
        assert_eq!(pr.url, "https://fake/pr/1");
        assert_eq!(
            forge.pr_state(&repo(), &pr.url).await.unwrap().mergeable,
            Mergeability::Pending
        );
        forge.set_pr(&pr.url, PrLifecycle::Merged, Mergeability::Mergeable);
        let state = forge.pr_state(&repo(), &pr.url).await.unwrap();
        assert_eq!(
            (state.lifecycle, state.mergeable),
            (PrLifecycle::Merged, Mergeability::Mergeable)
        );
        assert_eq!(
            forge.pushed(),
            [("owner/sandbox".to_string(), "factory/u1".to_string(), head)]
        );
        assert_eq!(forge.opened().len(), 1);
    }
}
````

- [ ] **Step 3: Run it and see it fail**

```
cargo test -p fleet-forge --lib
```

Expected: the crate does not compile; the errors name `MemForge`, `MemFixture`,
`repo_forge_contract`, `repo` and `pr_request`.

- [ ] **Step 4: Write the fake, the fixture seam and the suite**

Replace `crates/fleet-forge/src/testkit.rs` with:

````rust
//! An in-memory forge, a fixture seam for arranging repositories and bundles, and the contract
//! suite every [`RepoForge`] must pass.
//!
//! Compiled for this crate's own tests, and for other crates behind the `testkit` feature.

use crate::factory::{
    safe_branch, Change, ChangeKind, PrLifecycle, PrRef, PrRequest, PrState, RepoForge, RepoRef,
    Scratch,
};
use crate::legacy::{ForgeError, MergeResult, Mergeability};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

// ---------------------------------------------------------------------------------------------
// Builders
// ---------------------------------------------------------------------------------------------

/// `owner/sandbox` on `main`, at a URL that resolves nowhere.
pub fn repo() -> RepoRef {
    RepoRef {
        slug: "owner/sandbox".into(),
        url: "https://example.invalid/owner/sandbox".into(),
        base_branch: "main".into(),
    }
}

/// One file in a commit: its path and its new content, or `None` to delete it.
pub type FileChange<'a> = (&'a str, Option<&'a str>);

/// A request to open a pull request for `unit_id` at `head_sha`, with a body that carries
/// `owner/sandbox#7`.
pub fn pr_request(unit_id: &str, branch: &str, head_sha: &str) -> PrRequest {
    PrRequest {
        repo: repo(),
        unit_id: unit_id.into(),
        branch: branch.into(),
        head_sha: head_sha.into(),
        title: format!("unit {unit_id}"),
        body: "Summary\n\nFixes #7\n\nWork-Item: owner/sandbox#7\n".into(),
    }
}

// ---------------------------------------------------------------------------------------------
// The in-memory forge
// ---------------------------------------------------------------------------------------------

type Files = BTreeMap<String, Vec<u8>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Commit {
    id: String,
    parent: Option<String>,
    files: Files,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Bundle {
    branch: String,
    commits: Vec<Commit>,
}

fn commit_id(parent: Option<&str>, files: &Files) -> String {
    let mut h = Sha256::new();
    h.update(parent.unwrap_or("root").as_bytes());
    for (path, bytes) in files {
        h.update([0u8]);
        h.update(path.as_bytes());
        h.update([0u8]);
        h.update(bytes);
    }
    format!("{:x}", h.finalize())[..40].to_string()
}

fn apply(parent: &Files, changes: &[FileChange<'_>]) -> Files {
    let mut files = parent.clone();
    for (path, content) in changes {
        match content {
            Some(text) => files.insert(path.to_string(), text.as_bytes().to_vec()),
            None => files.remove(*path),
        };
    }
    files
}

fn diff(from: &Files, to: &Files) -> Vec<Change> {
    let mut out = Vec::new();
    for (path, bytes) in to {
        match from.get(path) {
            None => out.push(Change {
                path: path.clone(),
                kind: ChangeKind::Added,
            }),
            Some(old) if old != bytes => out.push(Change {
                path: path.clone(),
                kind: ChangeKind::Modified,
            }),
            Some(_) => {}
        }
    }
    for path in from.keys() {
        if !to.contains_key(path) {
            out.push(Change {
                path: path.clone(),
                kind: ChangeKind::Deleted,
            });
        }
    }
    out.sort();
    out
}

#[derive(Debug, Clone)]
struct Pr {
    request: PrRequest,
    reference: PrRef,
    lifecycle: PrLifecycle,
    mergeable: Mergeability,
}

#[derive(Default)]
struct Model {
    commits: HashMap<String, Commit>,
    tips: HashMap<String, String>,
    imported: HashMap<String, String>,
    pushed: Vec<(String, String, String)>,
    prs: Vec<Pr>,
    failing: Vec<String>,
    slow: HashMap<String, std::time::Duration>,
    calls: Vec<String>,
}

impl Model {
    fn files(&self, commit: &str) -> Result<&Files, ForgeError> {
        self.commits
            .get(commit)
            .map(|c| &c.files)
            .ok_or_else(|| ForgeError::Failed(format!("unknown commit {commit}")))
    }

    fn enter(&mut self, op: &str) -> Result<(), ForgeError> {
        self.calls.push(op.to_string());
        if self.failing.iter().any(|f| f == op) {
            return Err(ForgeError::Failed(format!("scripted failure of {op}")));
        }
        Ok(())
    }
}

/// An in-memory [`RepoForge`]: commits are snapshots, a "bundle" is a JSON file of them, and a
/// pull request is a record. A new pull request is `Open` and has [`MemForge::mergeable`].
#[derive(Clone)]
pub struct MemForge {
    model: Arc<Mutex<Model>>,
    /// The mergeability a newly opened pull request reports.
    pub mergeable: Mergeability,
}

impl Default for MemForge {
    fn default() -> Self {
        Self {
            model: Arc::default(),
            mergeable: Mergeability::Mergeable,
        }
    }
}

impl MemForge {
    pub fn new() -> Self {
        Self::default()
    }

    /// A forge whose newly opened pull requests report `mergeable`.
    pub fn reporting(mergeable: Mergeability) -> Self {
        Self {
            mergeable,
            ..Self::default()
        }
    }

    /// Commit `changes` on the repository's base branch and return the new tip. The first call
    /// for a repository creates it.
    pub fn commit_base(&self, repo: &RepoRef, changes: &[FileChange<'_>]) -> String {
        let mut m = self.model.lock().unwrap();
        let parent = m.tips.get(&repo.slug).cloned();
        let parent_files = parent
            .as_ref()
            .map(|p| m.commits[p].files.clone())
            .unwrap_or_default();
        let files = apply(&parent_files, changes);
        let id = commit_id(parent.as_deref(), &files);
        m.commits.insert(
            id.clone(),
            Commit {
                id: id.clone(),
                parent,
                files,
            },
        );
        m.tips.insert(repo.slug.clone(), id.clone());
        id
    }

    /// Write what a harness would deliver: a bundle of `branch`, forked at `base_sha`, with one
    /// commit per entry of `commits`. Returns the head commit.
    pub fn write_unit_bundle(
        &self,
        base_sha: &str,
        branch: &str,
        commits: &[&[FileChange<'_>]],
        dest: &Path,
    ) -> String {
        let m = self.model.lock().unwrap();
        let mut parent = base_sha.to_string();
        let mut files = m.commits[base_sha].files.clone();
        let mut out = Vec::new();
        for changes in commits {
            files = apply(&files, changes);
            let id = commit_id(Some(&parent), &files);
            out.push(Commit {
                id: id.clone(),
                parent: Some(parent.clone()),
                files: files.clone(),
            });
            parent = id;
        }
        let bundle = Bundle {
            branch: branch.to_string(),
            commits: out,
        };
        std::fs::write(dest, serde_json::to_vec(&bundle).unwrap()).unwrap();
        parent
    }

    /// Pretend `unit_id`'s branch was imported at `head_sha`, so a pull request can be opened for
    /// it without a bundle. For tests of a caller that never imports.
    pub fn mark_imported(&self, unit_id: &str, head_sha: &str) {
        self.model
            .lock()
            .unwrap()
            .imported
            .insert(unit_id.to_string(), head_sha.to_string());
    }

    /// Make every later call of one operation fail. `op` is the method's name, for example
    /// `"open_pr"` or `"pr_state"`.
    pub fn fail(&self, op: &str) {
        self.model.lock().unwrap().failing.push(op.to_string());
    }

    /// Make every later call of one operation take `by` before it does anything, on tokio's
    /// clock.
    pub fn slow(&self, op: &str, by: std::time::Duration) {
        self.model.lock().unwrap().slow.insert(op.to_string(), by);
    }

    async fn pause(&self, op: &str) {
        let by = self.model.lock().unwrap().slow.get(op).copied();
        if let Some(by) = by {
            tokio::time::sleep(by).await;
        }
    }

    /// Stop failing `op`.
    pub fn heal(&self, op: &str) {
        self.model.lock().unwrap().failing.retain(|f| f != op);
    }

    /// The names of the methods called so far, in order.
    pub fn calls(&self) -> Vec<String> {
        self.model.lock().unwrap().calls.clone()
    }

    /// Every pull request opened so far, in order.
    pub fn opened(&self) -> Vec<(PrRequest, PrRef)> {
        let m = self.model.lock().unwrap();
        m.prs
            .iter()
            .map(|p| (p.request.clone(), p.reference.clone()))
            .collect()
    }

    /// Every push so far, as `(repository slug, branch, commit)`.
    pub fn pushed(&self) -> Vec<(String, String, String)> {
        self.model.lock().unwrap().pushed.clone()
    }

    /// Change what the host reports for an open pull request.
    pub fn set_pr(&self, pr_url: &str, lifecycle: PrLifecycle, mergeable: Mergeability) {
        let mut m = self.model.lock().unwrap();
        let pr = m
            .prs
            .iter_mut()
            .find(|p| p.reference.url == pr_url)
            .expect("set_pr: no such pull request");
        pr.lifecycle = lifecycle;
        pr.mergeable = mergeable;
    }
}

struct MemScratch {
    model: Arc<Mutex<Model>>,
    repo_slug: String,
    base: String,
    head: String,
    order: Vec<String>,
}

#[async_trait]
impl Scratch for MemScratch {
    fn base_sha(&self) -> &str {
        &self.base
    }

    fn head_sha(&self) -> &str {
        &self.head
    }

    async fn commits(&self) -> Result<Vec<String>, ForgeError> {
        Ok(self.order.clone())
    }

    async fn changes(&self, from: &str, to: &str) -> Result<Vec<Change>, ForgeError> {
        let m = self.model.lock().unwrap();
        Ok(diff(m.files(from)?, m.files(to)?))
    }

    async fn read(&self, commit: &str, path: &str) -> Result<Option<Vec<u8>>, ForgeError> {
        let m = self.model.lock().unwrap();
        Ok(m.files(commit)?.get(path).cloned())
    }

    async fn export_tree(&self, commit: &str, dest: &Path) -> Result<(), ForgeError> {
        let files = self.model.lock().unwrap().files(commit)?.clone();
        for (path, bytes) in files {
            let target = dest.join(&path);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).map_err(|e| ForgeError::Failed(e.to_string()))?;
            }
            std::fs::write(&target, bytes).map_err(|e| ForgeError::Failed(e.to_string()))?;
        }
        Ok(())
    }

    async fn trial_merge(&self) -> Result<MergeResult, ForgeError> {
        let mut m = self.model.lock().unwrap();
        m.enter("trial_merge")?;
        let tip = m.tips[&self.repo_slug].clone();
        let fork = m.files(&self.base)?;
        let ours = diff(fork, m.files(&self.head)?);
        let theirs = diff(fork, m.files(&tip)?);
        let head_files = m.files(&self.head)?;
        let tip_files = m.files(&tip)?;
        let conflict = ours.iter().any(|c| {
            theirs.iter().any(|t| t.path == c.path)
                && head_files.get(&c.path) != tip_files.get(&c.path)
        });
        Ok(if conflict {
            MergeResult::Conflict
        } else {
            MergeResult::Clean
        })
    }
}

#[async_trait]
impl RepoForge for MemForge {
    async fn resolve_base(&self, repo: &RepoRef) -> Result<String, ForgeError> {
        let mut m = self.model.lock().unwrap();
        m.enter("resolve_base")?;
        m.tips
            .get(&repo.slug)
            .cloned()
            .ok_or_else(|| ForgeError::Failed(format!("unknown repository {}", repo.slug)))
    }

    async fn read_file(
        &self,
        _repo: &RepoRef,
        commit: &str,
        path: &str,
    ) -> Result<Option<Vec<u8>>, ForgeError> {
        let mut m = self.model.lock().unwrap();
        m.enter("read_file")?;
        Ok(m.files(commit)?.get(path).cloned())
    }

    async fn list_files(
        &self,
        _repo: &RepoRef,
        commit: &str,
        prefix: &str,
    ) -> Result<Vec<String>, ForgeError> {
        let mut m = self.model.lock().unwrap();
        m.enter("list_files")?;
        Ok(m.files(commit)?
            .keys()
            .filter(|p| p.starts_with(prefix))
            .cloned()
            .collect())
    }

    async fn source_bundle(
        &self,
        _repo: &RepoRef,
        base_sha: &str,
        dest: &Path,
    ) -> Result<(), ForgeError> {
        let mut m = self.model.lock().unwrap();
        m.enter("source_bundle")?;
        let commit = m
            .commits
            .get(base_sha)
            .cloned()
            .ok_or_else(|| ForgeError::Failed(format!("unknown commit {base_sha}")))?;
        let bundle = Bundle {
            branch: "base".into(),
            commits: vec![commit],
        };
        std::fs::write(dest, serde_json::to_vec(&bundle).unwrap())
            .map_err(|e| ForgeError::Failed(e.to_string()))
    }

    async fn import_bundle(
        &self,
        repo: &RepoRef,
        unit_id: &str,
        base_sha: &str,
        bundle: &Path,
        branch: &str,
    ) -> Result<Box<dyn Scratch>, ForgeError> {
        let mut m = self.model.lock().unwrap();
        m.enter("import_bundle")?;
        if !safe_branch(branch) {
            return Err(ForgeError::Failed(format!(
                "unsafe branch name: {branch:?}"
            )));
        }
        let bytes = std::fs::read(bundle).map_err(|e| ForgeError::Failed(e.to_string()))?;
        let parsed: Bundle = serde_json::from_slice(&bytes)
            .map_err(|e| ForgeError::Failed(format!("not a bundle: {e}")))?;
        if parsed.branch != branch {
            return Err(ForgeError::Failed(format!(
                "the bundle holds {:?}, not {branch:?}",
                parsed.branch
            )));
        }
        m.files(base_sha)?;
        let Some(head) = parsed.commits.last().map(|c| c.id.clone()) else {
            return Err(ForgeError::Failed(format!(
                "the bundle holds no commit of {branch}"
            )));
        };
        for commit in &parsed.commits {
            m.commits.insert(commit.id.clone(), commit.clone());
        }
        // Walk back from the head: the base must be one of its ancestors.
        let mut order = Vec::new();
        let mut at = head.clone();
        while at != base_sha {
            order.push(at.clone());
            let parent = m.commits.get(&at).and_then(|c| c.parent.clone());
            match parent {
                Some(parent) => at = parent,
                None => {
                    return Err(ForgeError::Failed(format!(
                        "{branch} does not descend from {base_sha}"
                    )))
                }
            }
        }
        order.reverse();
        let parent = head;
        m.imported.insert(unit_id.to_string(), parent.clone());
        Ok(Box::new(MemScratch {
            model: self.model.clone(),
            repo_slug: repo.slug.clone(),
            base: base_sha.to_string(),
            head: parent,
            order,
        }))
    }

    async fn open_pr(&self, request: &PrRequest) -> Result<PrRef, ForgeError> {
        self.pause("open_pr").await;
        let mut m = self.model.lock().unwrap();
        m.enter("open_pr")?;
        if !safe_branch(&request.branch) {
            return Err(ForgeError::Failed(format!(
                "unsafe branch name: {:?}",
                request.branch
            )));
        }
        match m.imported.get(&request.unit_id) {
            Some(head) if *head == request.head_sha => {}
            Some(head) => {
                return Err(ForgeError::Failed(format!(
                    "unit {} is at {head}, not at the verified commit {}",
                    request.unit_id, request.head_sha
                )))
            }
            None => {
                return Err(ForgeError::Failed(format!(
                    "unit {} has no imported branch",
                    request.unit_id
                )))
            }
        }
        m.pushed.push((
            request.repo.slug.clone(),
            request.branch.clone(),
            request.head_sha.clone(),
        ));
        let number = m.prs.len() as u64 + 1;
        let reference = PrRef {
            url: format!("https://fake/pr/{number}"),
            number,
        };
        m.prs.push(Pr {
            request: request.clone(),
            reference: reference.clone(),
            lifecycle: PrLifecycle::Open,
            mergeable: self.mergeable,
        });
        Ok(reference)
    }

    async fn pr_state(&self, _repo: &RepoRef, pr_url: &str) -> Result<PrState, ForgeError> {
        self.pause("pr_state").await;
        let mut m = self.model.lock().unwrap();
        m.enter("pr_state")?;
        let pr = m
            .prs
            .iter()
            .find(|p| p.reference.url == pr_url)
            .ok_or_else(|| ForgeError::Failed(format!("no pull request at {pr_url}")))?;
        Ok(PrState {
            lifecycle: pr.lifecycle,
            mergeable: pr.mergeable,
            body: pr.request.body.clone(),
            head_sha: pr.request.head_sha.clone(),
        })
    }
}

// ---------------------------------------------------------------------------------------------
// The fixture seam and the contract suite
// ---------------------------------------------------------------------------------------------

/// How the contract suite arranges a repository and a harness's delivery for one forge. The
/// in-memory forge fills its model; the `git` forge runs `git` against a local bare repository.
#[async_trait]
pub trait ForgeFixture: Send + Sync {
    type Forge: RepoForge;
    fn forge(&self) -> &Self::Forge;
    fn repo(&self) -> RepoRef;
    /// Commit `changes` on the base branch (creating the repository on first use) and return the
    /// new tip.
    async fn commit_base(&self, changes: &[FileChange<'_>]) -> String;
    /// Build a bundle of `branch`, forked at `base_sha`, with one commit per entry of `commits`.
    /// Returns the bundle's path and the head commit.
    async fn unit_bundle(
        &self,
        base_sha: &str,
        branch: &str,
        commits: &[&[FileChange<'_>]],
    ) -> (PathBuf, String);
    /// A new, empty directory the suite may write into.
    fn empty_dir(&self, name: &str) -> PathBuf;
}

/// [`ForgeFixture`] for [`MemForge`]. `dir` is a directory the fixture may write into.
pub struct MemFixture {
    forge: MemForge,
    dir: PathBuf,
}

impl MemFixture {
    pub fn new(dir: PathBuf) -> Self {
        Self {
            forge: MemForge::new(),
            dir,
        }
    }
}

#[async_trait]
impl ForgeFixture for MemFixture {
    type Forge = MemForge;

    fn forge(&self) -> &MemForge {
        &self.forge
    }

    fn repo(&self) -> RepoRef {
        repo()
    }

    async fn commit_base(&self, changes: &[FileChange<'_>]) -> String {
        self.forge.commit_base(&repo(), changes)
    }

    async fn unit_bundle(
        &self,
        base_sha: &str,
        branch: &str,
        commits: &[&[FileChange<'_>]],
    ) -> (PathBuf, String) {
        let dest = self
            .dir
            .join(format!("{}.bundle", branch.replace('/', "_")));
        let head = self
            .forge
            .write_unit_bundle(base_sha, branch, commits, &dest);
        (dest, head)
    }

    fn empty_dir(&self, name: &str) -> PathBuf {
        let dir = self.dir.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
}

fn kinds(changes: &[Change]) -> Vec<(&str, ChangeKind)> {
    changes.iter().map(|c| (c.path.as_str(), c.kind)).collect()
}

/// What every [`RepoForge`] must do. `make` returns a fixture over a new, empty world each time.
/// Panics on the first rule an implementation breaks.
pub async fn repo_forge_contract<X: ForgeFixture>(make: impl Fn() -> X) {
    use ChangeKind::{Added, Deleted, Modified};

    // Reading a repository at a commit.
    let x = make();
    let (forge, repo) = (x.forge(), x.repo());
    let first = x
        .commit_base(&[
            ("src/cart.js", Some("export const total = () => 0;\n")),
            ("test/cart.test.js", Some("// one\r\n")),
            ("old.txt", Some("to be removed\n")),
            ("forms/cart.md", Some("# Form: cart\n")),
        ])
        .await;
    let base = x.commit_base(&[("README.md", Some("hello\n"))]).await;
    assert_ne!(first, base);
    assert_eq!(forge.resolve_base(&repo).await.unwrap(), base);
    // Bytes are exact, line endings included; an earlier commit still reads as it was.
    assert_eq!(
        forge
            .read_file(&repo, &base, "test/cart.test.js")
            .await
            .unwrap(),
        Some(b"// one\r\n".to_vec())
    );
    assert_eq!(
        forge.read_file(&repo, &first, "README.md").await.unwrap(),
        None
    );
    assert!(forge
        .read_file(&repo, &"0".repeat(40), "README.md")
        .await
        .is_err());
    assert_eq!(
        forge.list_files(&repo, &base, "forms/").await.unwrap(),
        ["forms/cart.md"]
    );
    assert_eq!(
        forge.list_files(&repo, &base, "nowhere/").await.unwrap(),
        Vec::<String>::new()
    );

    // A source bundle is a file on disk.
    let source = x.empty_dir("source").join("source.bundle");
    forge.source_bundle(&repo, &base, &source).await.unwrap();
    assert!(std::fs::metadata(&source).unwrap().len() > 0);
    assert!(forge
        .source_bundle(&repo, &"0".repeat(40), &x.empty_dir("bad").join("b.bundle"))
        .await
        .is_err());

    // Importing a delivered branch: its commits, its changes and its files.
    let spec: &[FileChange<'_>] = &[(".reqdrive/specs/SPEC-1.md", Some("---\nid: SPEC-1\n---\n"))];
    let work: &[FileChange<'_>] = &[
        ("src/cart.js", Some("export const total = () => 1;\n")),
        ("src/codes.js", Some("export const codes = [];\n")),
        ("old.txt", None),
    ];
    let (bundle, head) = x.unit_bundle(&base, "factory/u1", &[spec, work]).await;
    let scratch = forge
        .import_bundle(&repo, "u1", &base, &bundle, "factory/u1")
        .await
        .unwrap();
    assert_eq!(
        (scratch.base_sha(), scratch.head_sha()),
        (base.as_str(), head.as_str())
    );
    let commits = scratch.commits().await.unwrap();
    assert_eq!(commits.len(), 2);
    assert_eq!(commits[1], head);
    assert_eq!(
        kinds(&scratch.changes(&base, &commits[0]).await.unwrap()),
        [(".reqdrive/specs/SPEC-1.md", Added)]
    );
    assert_eq!(
        kinds(&scratch.changes(&commits[0], &head).await.unwrap()),
        [
            ("old.txt", Deleted),
            ("src/cart.js", Modified),
            ("src/codes.js", Added)
        ]
    );
    assert_eq!(
        scratch
            .read(&head, ".reqdrive/specs/SPEC-1.md")
            .await
            .unwrap(),
        Some(b"---\nid: SPEC-1\n---\n".to_vec())
    );
    assert_eq!(scratch.read(&head, "old.txt").await.unwrap(), None);
    assert_eq!(
        scratch.read(&base, "old.txt").await.unwrap(),
        Some(b"to be removed\n".to_vec())
    );

    // The exported tree is plain files at the commit asked for, and nothing else.
    let tree = x.empty_dir("tree");
    scratch.export_tree(&head, &tree).await.unwrap();
    assert_eq!(
        std::fs::read(tree.join("src/codes.js")).unwrap(),
        b"export const codes = [];\n"
    );
    assert_eq!(
        std::fs::read(tree.join("test/cart.test.js")).unwrap(),
        b"// one\r\n"
    );
    assert!(!tree.join("old.txt").exists());
    assert!(
        !tree.join(".git").exists(),
        "an exported tree carries no repository"
    );

    // The trial merge: clean while the base has not touched the same file, a conflict once it has.
    assert_eq!(scratch.trial_merge().await.unwrap(), MergeResult::Clean);
    x.commit_base(&[("docs/notes.md", Some("unrelated\n"))])
        .await;
    assert_eq!(scratch.trial_merge().await.unwrap(), MergeResult::Clean);
    x.commit_base(&[("src/cart.js", Some("export const total = () => 2;\n"))])
        .await;
    assert_eq!(scratch.trial_merge().await.unwrap(), MergeResult::Conflict);

    // A bundle that does not hold the branch, or a branch name that is not safe, is refused.
    assert!(forge
        .import_bundle(&repo, "u2", &base, &bundle, "factory/other")
        .await
        .is_err());
    assert!(forge
        .import_bundle(&repo, "u2", &base, &bundle, "--upload-pack=x")
        .await
        .is_err());
    // A branch that does not descend from the stated base is refused: this one was cut at
    // the first commit, and the base named is a later one it does not contain.
    let (stale, _) = x
        .unit_bundle(&first, "factory/u3", &[&[("src/late.js", Some("late\n"))]])
        .await;
    assert!(forge
        .import_bundle(&repo, "u3", &base, &stale, "factory/u3")
        .await
        .is_err());
    // Opening the pull request: only for the commit that was imported.
    let mut request = pr_request("u1", "factory/u1", &head);
    request.repo = repo.clone();
    let mut wrong = request.clone();
    wrong.head_sha = commits[0].clone();
    assert!(
        forge.open_pr(&wrong).await.is_err(),
        "a commit other than the imported head"
    );
    let mut stranger = request.clone();
    stranger.unit_id = "never-imported".into();
    assert!(
        forge.open_pr(&stranger).await.is_err(),
        "a unit with no imported branch"
    );
    let opened = forge.open_pr(&request).await.unwrap();
    assert!(opened.number > 0 && !opened.url.is_empty());
    let state = forge.pr_state(&repo, &opened.url).await.unwrap();
    assert_eq!(state.lifecycle, PrLifecycle::Open);
    assert_eq!(state.body, request.body);
    assert_eq!(state.head_sha, head);
    assert!(forge
        .pr_state(&repo, "https://example.invalid/pr/0")
        .await
        .is_err());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn the_in_memory_forge_passes_the_repo_forge_contract() {
        let scratch = tempfile::tempdir().unwrap();
        let n = std::sync::atomic::AtomicU32::new(0);
        repo_forge_contract(|| {
            let i = n.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let dir = scratch.path().join(format!("world-{i}"));
            std::fs::create_dir_all(&dir).unwrap();
            MemFixture::new(dir)
        })
        .await;
    }

    #[tokio::test]
    async fn a_scripted_failure_fails_one_operation_until_healed() {
        let forge = MemForge::new();
        let base = forge.commit_base(&repo(), &[("a.txt", Some("a"))]);
        forge.fail("resolve_base");
        assert!(forge.resolve_base(&repo()).await.is_err());
        assert_eq!(
            forge.read_file(&repo(), &base, "a.txt").await.unwrap(),
            Some(b"a".to_vec())
        );
        forge.heal("resolve_base");
        assert_eq!(forge.resolve_base(&repo()).await.unwrap(), base);
        assert_eq!(forge.calls(), ["resolve_base", "read_file", "resolve_base"]);
    }

    #[tokio::test]
    async fn a_pull_request_reports_what_the_test_sets() {
        let dir = tempfile::tempdir().unwrap();
        let forge = MemForge {
            mergeable: Mergeability::Pending,
            ..MemForge::default()
        };
        let base = forge.commit_base(&repo(), &[("a.txt", Some("a"))]);
        let bundle = dir.path().join("u1.bundle");
        let head =
            forge.write_unit_bundle(&base, "factory/u1", &[&[("b.txt", Some("b"))]], &bundle);
        forge
            .import_bundle(&repo(), "u1", &base, &bundle, "factory/u1")
            .await
            .unwrap();
        let pr = forge
            .open_pr(&pr_request("u1", "factory/u1", &head))
            .await
            .unwrap();
        assert_eq!(pr.url, "https://fake/pr/1");
        assert_eq!(
            forge.pr_state(&repo(), &pr.url).await.unwrap().mergeable,
            Mergeability::Pending
        );
        forge.set_pr(&pr.url, PrLifecycle::Merged, Mergeability::Mergeable);
        let state = forge.pr_state(&repo(), &pr.url).await.unwrap();
        assert_eq!(
            (state.lifecycle, state.mergeable),
            (PrLifecycle::Merged, Mergeability::Mergeable)
        );
        assert_eq!(
            forge.pushed(),
            [("owner/sandbox".to_string(), "factory/u1".to_string(), head)]
        );
        assert_eq!(forge.opened().len(), 1);
    }
}
````

- [ ] **Step 5: Run the tests**

```
cargo test -p fleet-forge --lib
cargo test -p fleet-forge --lib --features testkit
```

Expected, both times: `test result: ok. 10 passed; 0 failed`.

- [ ] **Step 6: Commit**

```
git add crates/fleet-forge/src
git commit -m "feat(fleet-forge): the forge seams, the pull-request body rules, an in-memory forge and its contract suite"
```

### Task 4: `fleet-runner` — check containers and reaping

**Files:**
- Replace: `crates/fleet-runner/src/lib.rs`
- Create: `crates/fleet-runner/src/check.rs`, `reap.rs`, `docker.rs`, `testkit.rs`

**Produces:** the `CheckRunner` seam with `CacheRequest`, `CacheRef`, `CheckRequest`,
`CheckOutput`, `ContainerInfo`, `RunnerError`, `cache_key`, `UNIT_LABEL`, `CACHE_MOUNT`,
`MAX_CAPTURE_BYTES`; `DockerRunner`; `Action`, `reconcile`, `reconcile_live`, the `UnitCensus`
port, `ReapReport`, `reap_at_startup`, `reap_tick`. In `testkit`: `FakeRunner`, `Scripted`,
`FakeCensus`, `check`, `cache`, `output`, `IMAGE`, `contract_commands`, `contract_fake`,
`check_runner_contract`.

What to know:

- The reaping decision functions move here from `crates/fleetd/src/reconcile.rs`, not to
  `fleet-store`: they relate containers to units, and this is the crate that reaps.
- The contract suite runs real shell commands. A real runner executes them; the scripted
  runner is loaded with what each does (`contract_commands`). The suite then checks what the
  seam reports: exit codes, captured files, the untouched host tree, the timeout flag, the cache
  rules, and that reaping keeps volumes.

- [ ] **Step 1: Create the source files other than `testkit.rs`**

`crates/fleet-runner/src/lib.rs`:

````rust
//! `fleet-runner` — where the control plane runs a target repository's commands for itself, and
//! how it removes containers that outlived the unit they belonged to.
//!
//! A check container gets a copy of a tree, no network and no credential, runs one command and
//! is discarded. Dependency setup is the one run that has a network; what it leaves in the
//! unit's cache is mounted read-only into every check. Nothing here starts an agent.

mod check;
mod docker;
mod reap;

#[cfg(any(test, feature = "testkit"))]
pub mod testkit;

pub use check::{
    cache_key, CacheRef, CacheRequest, CheckOutput, CheckRequest, CheckRunner, ContainerInfo,
    RunnerError, CACHE_MOUNT, MAX_CAPTURE_BYTES, UNIT_LABEL,
};
pub use docker::DockerRunner;
pub use reap::{
    reap_at_startup, reap_tick, reconcile, reconcile_live, Action, ReapReport, UnitCensus,
};
````

`crates/fleet-runner/src/check.rs`:

````rust
//! The check-container seam: its requests, its results and the trait.

use async_trait::async_trait;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

/// The label every container and volume of a unit carries; its value is the unit id. The
/// harness labels what it creates with the same key, so one reaper finds both.
pub const UNIT_LABEL: &str = "cc.unit_id";

/// Where a unit's dependency cache is mounted in a container: read-write during setup,
/// read-only during a check.
pub const CACHE_MOUNT: &str = "/cache";

/// The most a run keeps of each of stdout and stderr. A longer stream keeps its end, which is
/// where a test runner prints its summary.
pub const MAX_CAPTURE_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RunnerError {
    /// The container engine could not do what was asked: it is not running, the image cannot be
    /// had, the tree cannot be copied. A command that ran and failed is not an error; it is a
    /// [`CheckOutput`] with a non-zero exit code.
    #[error("runner failure: {0}")]
    Failed(String),
}

/// One digest for a set of files: SHA-256 over one record per file (`<path>\0<sha256>\n`), the
/// records in bytewise path order. The dependency cache of a unit is keyed by this digest of its
/// manifests and lockfiles, so a changed lockfile means a new cache.
pub fn cache_key(files: &[(String, Vec<u8>)]) -> String {
    let mut records: Vec<(String, String)> = files
        .iter()
        .map(|(path, bytes)| (path.clone(), format!("{:x}", Sha256::digest(bytes))))
        .collect();
    records.sort();
    let mut hasher = Sha256::new();
    for (path, digest) in records {
        hasher.update(path.as_bytes());
        hasher.update([0u8]);
        hasher.update(digest.as_bytes());
        hasher.update(b"\n");
    }
    format!("{:x}", hasher.finalize())
}

/// Build, or find, a unit's dependency cache.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheRequest {
    pub unit_id: String,
    /// The image the repository declares, by digest.
    pub image: String,
    pub env: BTreeMap<String, String>,
    /// A directory on the host holding the tree to set up from. It is copied, never mounted.
    pub tree: PathBuf,
    /// The repository's `setup` command, run by `sh -c` at the root of the copy.
    pub setup: String,
    /// The manifests and lockfiles, as paths inside `tree`. Their contents are the cache's key.
    pub key_files: Vec<String>,
    pub timeout: Duration,
}

/// A unit's dependency cache.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheRef {
    /// [`cache_key`] of the request's key files.
    pub key: String,
    /// The engine's name for the volume. Opaque to callers.
    pub volume: String,
}

/// Run one command in a check container.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckRequest {
    pub unit_id: String,
    pub image: String,
    pub env: BTreeMap<String, String>,
    /// A directory on the host holding the tree to check. The container works on a copy, which
    /// is discarded with it; the directory itself is never changed.
    pub tree: PathBuf,
    /// Mounted read-only at [`CACHE_MOUNT`] when present.
    pub cache: Option<CacheRef>,
    /// Run by `sh -c` at the root of the copy.
    pub command: String,
    pub timeout: Duration,
    /// Files to read back from the container's copy after the command ends, as paths from the
    /// tree's root: a test report, for example. A path that does not exist is left out.
    pub collect: Vec<String>,
}

/// What one command did.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CheckOutput {
    /// `None` when the command was killed for running past its timeout.
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub stdout: String,
    pub stderr: String,
    pub collected: BTreeMap<String, Vec<u8>>,
}

impl CheckOutput {
    /// True only for a command that ran to the end and exited 0.
    pub fn succeeded(&self) -> bool {
        self.exit_code == Some(0) && !self.timed_out
    }
}

/// A container that carries [`UNIT_LABEL`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerInfo {
    pub unit_id: String,
    pub name: String,
}

/// Check containers, dependency caches and reaping.
#[async_trait]
pub trait CheckRunner: Send + Sync {
    /// Run the repository's setup once for this unit and these key files, in a container with a
    /// network and no credential, and keep what it writes under [`CACHE_MOUNT`]. Asking again
    /// with the same unit and the same key files returns the same cache without running setup.
    async fn build_cache(&self, request: &CacheRequest) -> Result<CacheRef, RunnerError>;

    /// Run one command on a copy of a tree, with no network and no credential, and discard the
    /// container. A command that exits non-zero or times out is an `Ok` output.
    async fn run_check(&self, request: &CheckRequest) -> Result<CheckOutput, RunnerError>;

    /// Every container, running or stopped, that carries [`UNIT_LABEL`].
    async fn unit_containers(&self) -> Result<Vec<ContainerInfo>, RunnerError>;

    /// The ids of the units that own at least one volume.
    async fn unit_volumes(&self) -> Result<Vec<String>, RunnerError>;

    /// Remove every container of a unit and return how many there were. Volumes are kept: a
    /// unit that is not finished needs its volume to resume.
    async fn reap_unit(&self, unit_id: &str) -> Result<usize, RunnerError>;

    /// Remove every volume of a unit and return how many there were. The caller asks only for a
    /// unit that is finished.
    async fn drop_unit_volumes(&self, unit_id: &str) -> Result<usize, RunnerError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str, bytes: &str) -> (String, Vec<u8>) {
        (path.to_string(), bytes.as_bytes().to_vec())
    }

    #[test]
    fn the_cache_key_ignores_the_order_files_arrive_in() {
        let a = [file("Cargo.toml", "[package]"), file("Cargo.lock", "v3")];
        let b = [file("Cargo.lock", "v3"), file("Cargo.toml", "[package]")];
        assert_eq!(cache_key(&a), cache_key(&b));
        assert_eq!(cache_key(&a).len(), 64);
    }

    #[test]
    fn the_cache_key_changes_with_content_with_a_path_and_with_a_missing_file() {
        let base = cache_key(&[file("Cargo.toml", "[package]"), file("Cargo.lock", "v3")]);
        assert_ne!(
            base,
            cache_key(&[file("Cargo.toml", "[package]"), file("Cargo.lock", "v4")])
        );
        assert_ne!(
            base,
            cache_key(&[file("cargo.toml", "[package]"), file("Cargo.lock", "v3")])
        );
        assert_ne!(base, cache_key(&[file("Cargo.toml", "[package]")]));
        assert_ne!(cache_key(&[]), base);
    }

    #[test]
    fn content_cannot_be_moved_between_a_path_and_its_bytes() {
        assert_ne!(cache_key(&[file("ab", "c")]), cache_key(&[file("a", "bc")]));
    }

    #[test]
    fn only_a_zero_exit_that_was_not_a_timeout_succeeded() {
        let out = |exit_code, timed_out| CheckOutput {
            exit_code,
            timed_out,
            ..CheckOutput::default()
        };
        assert!(out(Some(0), false).succeeded());
        assert!(!out(Some(1), false).succeeded());
        assert!(!out(None, true).succeeded());
        assert!(!out(None, false).succeeded());
    }
}
````

`crates/fleet-runner/src/reap.rs`:

````rust
//! Reaping: which containers no longer belong to a live unit, and the two passes that remove
//! them. The decisions are pure; the passes apply them through a [`CheckRunner`] and a
//! [`UnitCensus`].
//!
//! Wave 0 gives the signatures only. Lane CC-RUNNER moves the two decision functions from
//! `crates/fleetd/src/reconcile.rs` without changing what they return, and builds the passes.

use crate::check::CheckRunner;

/// One thing to do for one unit id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// A unit that is not finished, has no live supervisor, and has a container: reap the
    /// container, then tell the census the unit is stranded.
    HaltWithContainer(String),
    /// The same, with no container: tell the census only.
    HaltNoContainer(String),
    /// A container whose unit is finished or unknown: reap it.
    ReapStray(String),
}

/// At startup, when no unit has a live supervisor. `persisted_nonterminal` are the ids of the
/// stored units that are not finished; `running` are the ids that have a container.
pub fn reconcile(persisted_nonterminal: &[String], running: &[String]) -> Vec<Action> {
    let _ = (persisted_nonterminal, running);
    unimplemented!("lane CC-RUNNER moves this from crates/fleetd/src/reconcile.rs")
}

/// On a timer, while supervisors are live. A unit in `live` is healthy and never appears in the
/// result, even when it owns a container.
pub fn reconcile_live(
    persisted_nonterminal: &[String],
    live: &[String],
    running: &[String],
) -> Vec<Action> {
    let _ = (persisted_nonterminal, live, running);
    unimplemented!("lane CC-RUNNER moves this from crates/fleetd/src/reconcile.rs")
}

/// What the reaper needs to know about units, and the one thing it tells back. `fleetd`
/// implements it over the store and the live-unit table.
pub trait UnitCensus: Send + Sync {
    /// Ids of the stored units that are not finished.
    fn nonterminal(&self) -> Vec<String>;
    /// Ids of the stored units that are finished.
    fn finished(&self) -> Vec<String>;
    /// Ids of the units a supervisor holds now.
    fn live(&self) -> Vec<String>;
    /// A unit that is not finished was found with no supervisor.
    fn stranded(&self, unit_id: &str);
}

/// What one pass did.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ReapReport {
    /// Unit ids whose containers were removed.
    pub reaped: Vec<String>,
    /// Unit ids reported to [`UnitCensus::stranded`].
    pub stranded: Vec<String>,
    /// Unit ids whose volumes were removed, each a finished unit.
    pub volumes_dropped: Vec<String>,
    /// One line per runner call that failed. A failure never stops the pass.
    pub errors: Vec<String>,
}

/// The pass run once, before the daemon accepts a request: every unit that is not finished is
/// stranded, every labelled container is reaped, and finished units lose their volumes.
pub async fn reap_at_startup(runner: &dyn CheckRunner, census: &dyn UnitCensus) -> ReapReport {
    let _ = (runner, census);
    unimplemented!("lane CC-RUNNER builds this")
}

/// The pass run on an interval: as [`reap_at_startup`], except that a unit with a live
/// supervisor, and everything it owns, is left alone.
pub async fn reap_tick(runner: &dyn CheckRunner, census: &dyn UnitCensus) -> ReapReport {
    let _ = (runner, census);
    unimplemented!("lane CC-RUNNER builds this")
}
````

`crates/fleet-runner/src/docker.rs`:

````rust
//! [`CheckRunner`] over the `docker` command. Wave 0 gives the signatures only; lane CC-RUNNER
//! builds the bodies.

use crate::check::{
    CacheRef, CacheRequest, CheckOutput, CheckRequest, CheckRunner, ContainerInfo, RunnerError,
};
use async_trait::async_trait;
use std::path::PathBuf;

const NEW: &str = "lane CC-RUNNER builds this";

/// Check containers on the local Docker engine, reached by running the `docker` program.
#[derive(Debug, Clone)]
pub struct DockerRunner {
    #[allow(dead_code)] // read by the bodies lane CC-RUNNER builds
    program: PathBuf,
}

impl Default for DockerRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl DockerRunner {
    /// Uses the `docker` found on `PATH`.
    pub fn new() -> Self {
        Self {
            program: PathBuf::from("docker"),
        }
    }

    /// Uses the program at `program` in place of `docker`.
    pub fn with_program(program: PathBuf) -> Self {
        Self { program }
    }

    /// True when the engine answers. For health reporting; never cached here.
    pub async fn available(&self) -> bool {
        unimplemented!("{NEW}")
    }
}

#[async_trait]
impl CheckRunner for DockerRunner {
    async fn build_cache(&self, request: &CacheRequest) -> Result<CacheRef, RunnerError> {
        let _ = request;
        unimplemented!("{NEW}")
    }

    async fn run_check(&self, request: &CheckRequest) -> Result<CheckOutput, RunnerError> {
        let _ = request;
        unimplemented!("{NEW}")
    }

    async fn unit_containers(&self) -> Result<Vec<ContainerInfo>, RunnerError> {
        unimplemented!("{NEW}")
    }

    async fn unit_volumes(&self) -> Result<Vec<String>, RunnerError> {
        unimplemented!("{NEW}")
    }

    async fn reap_unit(&self, unit_id: &str) -> Result<usize, RunnerError> {
        let _ = unit_id;
        unimplemented!("{NEW}")
    }

    async fn drop_unit_volumes(&self, unit_id: &str) -> Result<usize, RunnerError> {
        let _ = unit_id;
        unimplemented!("{NEW}")
    }
}
````

- [ ] **Step 2: Write the failing test**

Create `crates/fleet-runner/src/testkit.rs` with only this:

````rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn the_scripted_runner_passes_the_check_runner_contract() {
        let dir = tempfile::tempdir().unwrap();
        check_runner_contract(contract_fake, IMAGE, dir.path()).await;
    }

    #[tokio::test]
    async fn an_unscripted_command_exits_zero_and_every_request_is_recorded() {
        let dir = tempfile::tempdir().unwrap();
        let runner = FakeRunner::new();
        let out = runner
            .run_check(&check(dir.path(), "cargo test"))
            .await
            .unwrap();
        assert_eq!(out, output(0, ""));
        assert_eq!(runner.checks().len(), 1);
        assert_eq!(runner.checks()[0].command, "cargo test");
    }

    #[tokio::test]
    async fn a_command_scripted_twice_behaves_differently_on_each_run_then_repeats() {
        let dir = tempfile::tempdir().unwrap();
        let runner = FakeRunner::new();
        runner
            .on("npm test", Scripted::exit(1, "1 failing"))
            .on("npm test", Scripted::exit(0, "all passing"));
        let run = || async {
            runner
                .run_check(&check(dir.path(), "npm test"))
                .await
                .unwrap()
        };
        assert_eq!(run().await.exit_code, Some(1));
        assert_eq!(run().await.exit_code, Some(0));
        assert_eq!(run().await.exit_code, Some(0));
    }

    #[tokio::test]
    async fn setup_runs_once_per_key_and_a_failing_setup_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("package-lock.json"), "{}").unwrap();
        let runner = FakeRunner::new();
        let request = cache(dir.path(), "npm ci", &["package-lock.json"]);
        runner.build_cache(&request).await.unwrap();
        runner.build_cache(&request).await.unwrap();
        assert_eq!(runner.setups().len(), 1);

        let failing = FakeRunner::new();
        failing.on("npm ci", Scripted::exit(1, ""));
        assert!(failing.build_cache(&request).await.is_err());
    }

    #[tokio::test]
    async fn containers_are_reaped_by_unit_and_an_unavailable_engine_is_an_error() {
        let runner = FakeRunner::new();
        runner.add_container("u1", "cc_u1_check");
        runner.add_container("u1", "cc_u1_agent");
        runner.add_container("u2", "cc_u2_agent");
        runner.add_volume("u1");
        assert_eq!(runner.reap_unit("u1").await.unwrap(), 2);
        assert_eq!(runner.unit_containers().await.unwrap().len(), 1);
        assert_eq!(runner.unit_volumes().await.unwrap(), ["u1"]);
        runner.unavailable(true);
        assert!(runner.unit_containers().await.is_err());
        assert!(runner.reap_unit("u2").await.is_err());
    }

    #[test]
    fn the_scripted_census_records_what_it_is_told() {
        let census = FakeCensus::new(&["u1"], &["u0"], &[]);
        census.stranded("u1");
        assert_eq!(census.stranded_units(), ["u1"]);
        assert_eq!(census.nonterminal(), ["u1"]);
        assert_eq!(census.finished(), ["u0"]);
        assert!(census.live().is_empty());
    }
}
````

- [ ] **Step 3: Run it and see it fail**

```
cargo test -p fleet-runner --lib
```

Expected: the crate does not compile; the errors name `FakeRunner`, `FakeCensus`, `Scripted`,
`check_runner_contract`, `contract_fake`, `check`, `cache`, `output` and `IMAGE`.

- [ ] **Step 4: Write the fakes and the suite**

Replace `crates/fleet-runner/src/testkit.rs` with:

````rust
//! A scripted runner, a scripted census, builders, and the contract suite every [`CheckRunner`]
//! must pass.
//!
//! Compiled for this crate's own tests, and for other crates behind the `testkit` feature.

use crate::check::{
    cache_key, CacheRef, CacheRequest, CheckOutput, CheckRequest, CheckRunner, ContainerInfo,
    RunnerError, CACHE_MOUNT,
};
use crate::reap::UnitCensus;
use async_trait::async_trait;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

// ---------------------------------------------------------------------------------------------
// Builders
// ---------------------------------------------------------------------------------------------

/// The image name the builders use. The scripted runner never pulls it.
pub const IMAGE: &str = "example.invalid/check@sha256:0000";

/// A check of `command` on `tree` for unit `u1`: no cache, no environment, a 60 second timeout.
pub fn check(tree: &Path, command: &str) -> CheckRequest {
    CheckRequest {
        unit_id: "u1".into(),
        image: IMAGE.into(),
        env: BTreeMap::new(),
        tree: tree.to_path_buf(),
        cache: None,
        command: command.into(),
        timeout: Duration::from_secs(60),
        collect: Vec::new(),
    }
}

/// A cache request for unit `u1` that runs `setup` on `tree`, keyed by `key_files`.
pub fn cache(tree: &Path, setup: &str, key_files: &[&str]) -> CacheRequest {
    CacheRequest {
        unit_id: "u1".into(),
        image: IMAGE.into(),
        env: BTreeMap::new(),
        tree: tree.to_path_buf(),
        setup: setup.into(),
        key_files: key_files.iter().map(|s| s.to_string()).collect(),
        timeout: Duration::from_secs(60),
    }
}

/// An output that exited `code` with `stdout`.
pub fn output(code: i32, stdout: &str) -> CheckOutput {
    CheckOutput {
        exit_code: Some(code),
        stdout: stdout.into(),
        ..CheckOutput::default()
    }
}

// ---------------------------------------------------------------------------------------------
// The scripted runner
// ---------------------------------------------------------------------------------------------

/// What a scripted command does when the scripted runner "runs" it.
#[derive(Debug, Clone, Default)]
pub struct Scripted {
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub stdout: String,
    pub stderr: String,
    /// Files the command leaves in the container's copy of the tree, by path from its root. A
    /// path under [`CACHE_MOUNT`] is written to the cache when the command is a setup.
    pub writes: Vec<(String, Vec<u8>)>,
}

impl Scripted {
    /// Exits `code` with `stdout`.
    pub fn exit(code: i32, stdout: &str) -> Self {
        Self {
            exit_code: Some(code),
            stdout: stdout.into(),
            ..Self::default()
        }
    }

    /// Also leaves `bytes` at `path`.
    pub fn writing(mut self, path: &str, bytes: &[u8]) -> Self {
        self.writes.push((path.to_string(), bytes.to_vec()));
        self
    }
}

#[derive(Default)]
struct State {
    script: HashMap<String, Vec<Scripted>>,
    checks: Vec<CheckRequest>,
    setups: Vec<CacheRequest>,
    caches: HashMap<(String, String), BTreeMap<String, Vec<u8>>>,
    containers: Vec<ContainerInfo>,
    volumes: Vec<String>,
    unavailable: bool,
}

/// A [`CheckRunner`] that runs nothing. A command it has a script for does what the script says;
/// any other command exits 0 with no output. It records every request, and models containers
/// and volumes so reaping can be tested.
#[derive(Default)]
pub struct FakeRunner {
    state: Mutex<State>,
}

impl FakeRunner {
    pub fn new() -> Self {
        Self::default()
    }

    /// Script what `command` does. Scripting the same command again queues a second behaviour:
    /// each run takes the next one, and the last one repeats.
    pub fn on(&self, command: &str, scripted: Scripted) -> &Self {
        self.state
            .lock()
            .unwrap()
            .script
            .entry(command.to_string())
            .or_default()
            .push(scripted);
        self
    }

    /// While on, every call returns `Err`, as when the container engine is not running.
    pub fn unavailable(&self, on: bool) {
        self.state.lock().unwrap().unavailable = on;
    }

    /// Pretend a container of `unit_id` exists.
    pub fn add_container(&self, unit_id: &str, name: &str) {
        self.state.lock().unwrap().containers.push(ContainerInfo {
            unit_id: unit_id.into(),
            name: name.into(),
        });
    }

    /// Pretend a volume of `unit_id` exists.
    pub fn add_volume(&self, unit_id: &str) {
        self.state.lock().unwrap().volumes.push(unit_id.into());
    }

    /// Every check asked for, in order.
    pub fn checks(&self) -> Vec<CheckRequest> {
        self.state.lock().unwrap().checks.clone()
    }

    /// Every setup that actually ran, in order. A cache hit is not in the list.
    pub fn setups(&self) -> Vec<CacheRequest> {
        self.state.lock().unwrap().setups.clone()
    }

    fn take(state: &mut State, command: &str) -> Scripted {
        match state.script.get_mut(command) {
            Some(queue) if queue.len() > 1 => queue.remove(0),
            Some(queue) => queue[0].clone(),
            None => Scripted::exit(0, ""),
        }
    }

    fn enter(state: &State) -> Result<(), RunnerError> {
        if state.unavailable {
            return Err(RunnerError::Failed(
                "the container engine is not available".into(),
            ));
        }
        Ok(())
    }
}

#[async_trait]
impl CheckRunner for FakeRunner {
    async fn build_cache(&self, request: &CacheRequest) -> Result<CacheRef, RunnerError> {
        let mut files = Vec::new();
        for path in &request.key_files {
            let bytes = std::fs::read(request.tree.join(path))
                .map_err(|e| RunnerError::Failed(format!("key file {path}: {e}")))?;
            files.push((path.clone(), bytes));
        }
        let key = cache_key(&files);
        let mut state = self.state.lock().unwrap();
        Self::enter(&state)?;
        let slot = (request.unit_id.clone(), key.clone());
        if !state.caches.contains_key(&slot) {
            let scripted = Self::take(&mut state, &request.setup);
            state.setups.push(request.clone());
            if scripted.exit_code != Some(0) || scripted.timed_out {
                return Err(RunnerError::Failed(format!(
                    "setup did not succeed: exit {:?}: {}",
                    scripted.exit_code, scripted.stderr
                )));
            }
            let kept = scripted
                .writes
                .into_iter()
                .filter(|(path, _)| path.starts_with(CACHE_MOUNT))
                .collect();
            state.caches.insert(slot, kept);
            if !state.volumes.contains(&request.unit_id) {
                state.volumes.push(request.unit_id.clone());
            }
        }
        Ok(CacheRef {
            volume: format!("fake-cache-{}-{}", request.unit_id, &key[..12]),
            key,
        })
    }

    async fn run_check(&self, request: &CheckRequest) -> Result<CheckOutput, RunnerError> {
        let mut state = self.state.lock().unwrap();
        Self::enter(&state)?;
        state.checks.push(request.clone());
        let scripted = Self::take(&mut state, &request.command);
        let mut collected = BTreeMap::new();
        for path in &request.collect {
            let written = scripted.writes.iter().rev().find(|(p, _)| p == path);
            if let Some((_, bytes)) = written {
                collected.insert(path.clone(), bytes.clone());
            } else if let Ok(bytes) = std::fs::read(request.tree.join(path)) {
                collected.insert(path.clone(), bytes);
            }
        }
        Ok(CheckOutput {
            exit_code: if scripted.timed_out {
                None
            } else {
                scripted.exit_code
            },
            timed_out: scripted.timed_out,
            stdout: scripted.stdout,
            stderr: scripted.stderr,
            collected,
        })
    }

    async fn unit_containers(&self) -> Result<Vec<ContainerInfo>, RunnerError> {
        let state = self.state.lock().unwrap();
        Self::enter(&state)?;
        Ok(state.containers.clone())
    }

    async fn unit_volumes(&self) -> Result<Vec<String>, RunnerError> {
        let state = self.state.lock().unwrap();
        Self::enter(&state)?;
        Ok(state.volumes.clone())
    }

    async fn reap_unit(&self, unit_id: &str) -> Result<usize, RunnerError> {
        let mut state = self.state.lock().unwrap();
        Self::enter(&state)?;
        let before = state.containers.len();
        state.containers.retain(|c| c.unit_id != unit_id);
        Ok(before - state.containers.len())
    }

    async fn drop_unit_volumes(&self, unit_id: &str) -> Result<usize, RunnerError> {
        let mut state = self.state.lock().unwrap();
        Self::enter(&state)?;
        let before = state.volumes.len();
        state.volumes.retain(|v| v != unit_id);
        state.caches.retain(|(unit, _), _| unit != unit_id);
        Ok(before - state.volumes.len())
    }
}

// ---------------------------------------------------------------------------------------------
// The scripted census
// ---------------------------------------------------------------------------------------------

/// A [`UnitCensus`] with fixed answers that records which units it was told are stranded.
#[derive(Default)]
pub struct FakeCensus {
    pub nonterminal: Vec<String>,
    pub finished: Vec<String>,
    pub live: Vec<String>,
    stranded: Mutex<Vec<String>>,
}

impl FakeCensus {
    pub fn new(nonterminal: &[&str], finished: &[&str], live: &[&str]) -> Self {
        let owned = |ids: &[&str]| ids.iter().map(|s| s.to_string()).collect();
        Self {
            nonterminal: owned(nonterminal),
            finished: owned(finished),
            live: owned(live),
            stranded: Mutex::default(),
        }
    }

    /// The units reported stranded, in the order they were reported.
    pub fn stranded_units(&self) -> Vec<String> {
        self.stranded.lock().unwrap().clone()
    }
}

impl UnitCensus for FakeCensus {
    fn nonterminal(&self) -> Vec<String> {
        self.nonterminal.clone()
    }

    fn finished(&self) -> Vec<String> {
        self.finished.clone()
    }

    fn live(&self) -> Vec<String> {
        self.live.clone()
    }

    fn stranded(&self, unit_id: &str) {
        self.stranded.lock().unwrap().push(unit_id.to_string());
    }
}

// ---------------------------------------------------------------------------------------------
// The contract suite
// ---------------------------------------------------------------------------------------------

/// The commands the contract suite runs, with what each does in a POSIX shell. A real runner
/// ignores this table, because its shell does these things; the scripted runner is loaded from
/// it. Either way the suite then checks what the runner reports.
pub fn contract_commands() -> Vec<(&'static str, Scripted)> {
    vec![
        ("echo hello", Scripted::exit(0, "hello\n")),
        (
            "echo oops >&2; exit 3",
            Scripted {
                exit_code: Some(3),
                stderr: "oops\n".into(),
                ..Scripted::default()
            },
        ),
        ("cat input.txt", Scripted::exit(0, "from the tree\n")),
        ("echo \"$GREETING\"", Scripted::exit(0, "hi\n")),
        (
            "echo changed > input.txt; echo made > made.txt",
            Scripted::exit(0, "")
                .writing("input.txt", b"changed\n")
                .writing("made.txt", b"made\n"),
        ),
        (
            "sleep 30",
            Scripted {
                timed_out: true,
                ..Scripted::default()
            },
        ),
        (
            "echo dep > /cache/dep.txt",
            Scripted::exit(0, "").writing("/cache/dep.txt", b"dep\n"),
        ),
        ("cat /cache/dep.txt", Scripted::exit(0, "dep\n")),
        (
            "echo x > /cache/blocked.txt",
            Scripted {
                exit_code: Some(1),
                stderr: "read-only file system\n".into(),
                ..Scripted::default()
            },
        ),
    ]
}

/// A [`FakeRunner`] loaded with [`contract_commands`].
pub fn contract_fake() -> FakeRunner {
    let runner = FakeRunner::new();
    for (command, scripted) in contract_commands() {
        runner.on(command, scripted);
    }
    runner
}

/// What every [`CheckRunner`] must do. `make` returns a new runner each time; `image` is an
/// image with a POSIX `sh`; `dir` is an empty directory the suite may write into. Panics on the
/// first rule an implementation breaks.
pub async fn check_runner_contract<R: CheckRunner>(make: impl Fn() -> R, image: &str, dir: &Path) {
    let tree: PathBuf = dir.join("tree");
    std::fs::create_dir_all(&tree).unwrap();
    std::fs::write(tree.join("input.txt"), "from the tree\n").unwrap();
    std::fs::write(tree.join("deps.lock"), "v1\n").unwrap();
    let runner = make();
    let ask = |command: &str| {
        let mut request = check(&tree, command);
        request.image = image.to_string();
        request
    };

    // A command's exit code and output come back; a failing command is an output, not an error.
    let hello = runner.run_check(&ask("echo hello")).await.unwrap();
    assert_eq!(
        (hello.exit_code, hello.stdout.as_str()),
        (Some(0), "hello\n")
    );
    assert!(hello.succeeded());
    let failed = runner
        .run_check(&ask("echo oops >&2; exit 3"))
        .await
        .unwrap();
    assert_eq!(failed.exit_code, Some(3));
    assert!(failed.stderr.contains("oops"));
    assert!(!failed.succeeded());

    // The command sees the tree, and the environment it was given.
    assert_eq!(
        runner
            .run_check(&ask("cat input.txt"))
            .await
            .unwrap()
            .stdout,
        "from the tree\n"
    );
    let mut greeting = ask("echo \"$GREETING\"");
    greeting.env.insert("GREETING".into(), "hi".into());
    assert_eq!(runner.run_check(&greeting).await.unwrap().stdout, "hi\n");

    // The command works on a copy: files it writes can be collected, and the tree on the host
    // is exactly as it was. A path that does not exist is left out, not an error.
    let mut writes = ask("echo changed > input.txt; echo made > made.txt");
    writes.collect = vec!["input.txt".into(), "made.txt".into(), "absent.txt".into()];
    let out = runner.run_check(&writes).await.unwrap();
    assert_eq!(
        out.collected.get("input.txt").map(Vec::as_slice),
        Some(&b"changed\n"[..])
    );
    assert_eq!(
        out.collected.get("made.txt").map(Vec::as_slice),
        Some(&b"made\n"[..])
    );
    assert!(!out.collected.contains_key("absent.txt"));
    assert_eq!(
        std::fs::read(tree.join("input.txt")).unwrap(),
        b"from the tree\n"
    );
    assert!(!tree.join("made.txt").exists());

    // A command that runs past its timeout is stopped and reported as timed out.
    let mut slow = ask("sleep 30");
    slow.timeout = Duration::from_secs(1);
    let started = std::time::Instant::now();
    let out = runner.run_check(&slow).await.unwrap();
    assert!(out.timed_out);
    assert_eq!(out.exit_code, None);
    assert!(!out.succeeded());
    assert!(
        started.elapsed() < Duration::from_secs(20),
        "the timeout was not enforced"
    );

    // Nothing is left behind by a check. (Only this suite's unit is looked at: a real engine
    // may be running other units.)
    let ours = |ids: Vec<String>| ids.into_iter().filter(|id| id == "u1").collect::<Vec<_>>();
    let containers = runner.unit_containers().await.unwrap();
    assert_eq!(
        ours(containers.into_iter().map(|c| c.unit_id).collect()),
        Vec::<String>::new()
    );

    // The cache: setup writes it, a check reads it and cannot write it.
    let mut setup = cache(&tree, "echo dep > /cache/dep.txt", &["deps.lock"]);
    setup.image = image.to_string();
    let built = runner.build_cache(&setup).await.unwrap();
    assert_eq!(
        built.key,
        cache_key(&[("deps.lock".to_string(), b"v1\n".to_vec())])
    );
    assert_eq!(
        runner.build_cache(&setup).await.unwrap(),
        built,
        "the same key is the same cache"
    );
    let mut read = ask("cat /cache/dep.txt");
    read.cache = Some(built.clone());
    assert_eq!(runner.run_check(&read).await.unwrap().stdout, "dep\n");
    let mut write = ask("echo x > /cache/blocked.txt");
    write.cache = Some(built.clone());
    assert!(
        !runner.run_check(&write).await.unwrap().succeeded(),
        "the cache is read-only"
    );
    // A changed key file is a different cache.
    std::fs::write(tree.join("deps.lock"), "v2\n").unwrap();
    assert_ne!(runner.build_cache(&setup).await.unwrap().key, built.key);
    // A key file that is missing is an error, never an empty key.
    let mut missing = setup.clone();
    missing.key_files = vec!["no-such.lock".into()];
    assert!(runner.build_cache(&missing).await.is_err());

    // The cache is the unit's volume: it is listed, survives reaping, and goes when dropped.
    assert_eq!(ours(runner.unit_volumes().await.unwrap()), ["u1"]);
    assert_eq!(runner.reap_unit("u1").await.unwrap(), 0);
    assert_eq!(
        ours(runner.unit_volumes().await.unwrap()),
        ["u1"],
        "reaping keeps volumes"
    );
    assert!(runner.drop_unit_volumes("u1").await.unwrap() >= 1);
    assert_eq!(
        ours(runner.unit_volumes().await.unwrap()),
        Vec::<String>::new()
    );
    assert_eq!(runner.reap_unit("nobody").await.unwrap(), 0);
    assert_eq!(runner.drop_unit_volumes("nobody").await.unwrap(), 0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn the_scripted_runner_passes_the_check_runner_contract() {
        let dir = tempfile::tempdir().unwrap();
        check_runner_contract(contract_fake, IMAGE, dir.path()).await;
    }

    #[tokio::test]
    async fn an_unscripted_command_exits_zero_and_every_request_is_recorded() {
        let dir = tempfile::tempdir().unwrap();
        let runner = FakeRunner::new();
        let out = runner
            .run_check(&check(dir.path(), "cargo test"))
            .await
            .unwrap();
        assert_eq!(out, output(0, ""));
        assert_eq!(runner.checks().len(), 1);
        assert_eq!(runner.checks()[0].command, "cargo test");
    }

    #[tokio::test]
    async fn a_command_scripted_twice_behaves_differently_on_each_run_then_repeats() {
        let dir = tempfile::tempdir().unwrap();
        let runner = FakeRunner::new();
        runner
            .on("npm test", Scripted::exit(1, "1 failing"))
            .on("npm test", Scripted::exit(0, "all passing"));
        let run = || async {
            runner
                .run_check(&check(dir.path(), "npm test"))
                .await
                .unwrap()
        };
        assert_eq!(run().await.exit_code, Some(1));
        assert_eq!(run().await.exit_code, Some(0));
        assert_eq!(run().await.exit_code, Some(0));
    }

    #[tokio::test]
    async fn setup_runs_once_per_key_and_a_failing_setup_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("package-lock.json"), "{}").unwrap();
        let runner = FakeRunner::new();
        let request = cache(dir.path(), "npm ci", &["package-lock.json"]);
        runner.build_cache(&request).await.unwrap();
        runner.build_cache(&request).await.unwrap();
        assert_eq!(runner.setups().len(), 1);

        let failing = FakeRunner::new();
        failing.on("npm ci", Scripted::exit(1, ""));
        assert!(failing.build_cache(&request).await.is_err());
    }

    #[tokio::test]
    async fn containers_are_reaped_by_unit_and_an_unavailable_engine_is_an_error() {
        let runner = FakeRunner::new();
        runner.add_container("u1", "cc_u1_check");
        runner.add_container("u1", "cc_u1_agent");
        runner.add_container("u2", "cc_u2_agent");
        runner.add_volume("u1");
        assert_eq!(runner.reap_unit("u1").await.unwrap(), 2);
        assert_eq!(runner.unit_containers().await.unwrap().len(), 1);
        assert_eq!(runner.unit_volumes().await.unwrap(), ["u1"]);
        runner.unavailable(true);
        assert!(runner.unit_containers().await.is_err());
        assert!(runner.reap_unit("u2").await.is_err());
    }

    #[test]
    fn the_scripted_census_records_what_it_is_told() {
        let census = FakeCensus::new(&["u1"], &["u0"], &[]);
        census.stranded("u1");
        assert_eq!(census.stranded_units(), ["u1"]);
        assert_eq!(census.nonterminal(), ["u1"]);
        assert_eq!(census.finished(), ["u0"]);
        assert!(census.live().is_empty());
    }
}
````

- [ ] **Step 5: Run the tests**

```
cargo test -p fleet-runner --lib
cargo test -p fleet-runner --lib --features testkit
```

Expected, both times: `test result: ok. 10 passed; 0 failed`.

- [ ] **Step 6: Commit**

```
git add crates/fleet-runner/src
git commit -m "feat(fleet-runner): the check-runner seam, the reaping port, a scripted runner and its contract suite"
```

### Task 5: `fleet-verify` — the verifier seam

**Files:**
- Replace: `crates/fleet-verify/src/lib.rs`
- Create: `crates/fleet-verify/src/seam.rs`, `verifier.rs`, `testkit.rs`

**Produces:** the `EvidenceVerifier` seam with `VerifyInput`, `Verdict`, `Check`; `Verifier`,
`VerifyConfig`, `SPEC_DIR`. In `testkit`: `ScriptedVerifier`, `RecordedInput`, `World`,
`capabilities`, `node_config`, `bare_order`, `evidence`, `pr_open`, `no_change`, `freeze`,
`good_work`, `report`, the constants `FROZEN_TEST_PATH`, `FROZEN_TEST_SOURCE`, `FROZEN_ID`,
`BASE_ID`, and `evidence_verifier_contract`.

What to know:

- `VerifyInput` is the supervisor spec's (section 3.4) with three more fields: the oracle
  freeze, the signed spec bytes and the body the pull request will have. `Verdict` has two more
  cases, `Unrunnable` and `TestsFailed`; `Check` has three more, `Spec`, `Protected` and `Merge`.
- `ScriptedVerifier` answers from a queue. It still refuses the handful of inputs no verifier
  may approve, and that handful is the seam's contract suite: a scripted fake cannot be held to
  more. The real verifier's behaviour is specified by lane CC-VERIFY's tests, on `World`.
- `World` is a whole unit arranged on `MemForge` and `FakeRunner` that a correct verifier
  answers `Verified` for. Its own test checks that it is consistent with the presets and the
  forge, so a lane's failing test is never the fixture's fault.

- [ ] **Step 1: Create the source files other than `testkit.rs`**

`crates/fleet-verify/src/lib.rs`:

````rust
//! `fleet-verify` — the control plane's own check of what a harness delivered. A harness reports
//! evidence; nothing it reports is decisive. Every check here is a run of the control plane's
//! own: its own import of the delivered bundle, its own reading of the files, its own test run
//! in its own check container.
//!
//! [`EvidenceVerifier`] is the seam the supervisor calls. [`Verifier`] is the real one, over a
//! forge and a check runner. `testkit::ScriptedVerifier` answers from a queue.

mod seam;
mod verifier;

#[cfg(any(test, feature = "testkit"))]
pub mod testkit;

pub use seam::{Check, EvidenceVerifier, Verdict, VerifyInput};
pub use verifier::{Verifier, VerifyConfig, SPEC_DIR};
````

`crates/fleet-verify/src/seam.rs`:

````rust
//! The verifier seam: what the supervisor hands over, and the verdicts it can get back.
//!
//! This is the supervisor spec's `EvidenceVerifier` (section 3.4) at protocol 0.2: the input
//! also carries the oracle freeze and the signed spec bytes, and a verdict can say that a check
//! could not run or that the control plane's own test run failed.

use async_trait::async_trait;
use harness_protocol::{Capabilities, OracleFreeze, UnitResult, WorkOrder};

/// Everything a verification reads that does not come from the delivered bundle.
#[derive(Debug, Clone, Copy)]
pub struct VerifyInput<'a> {
    pub order: &'a WorkOrder,
    pub capabilities: &'a Capabilities,
    /// What the harness reported. Recorded, compared, and never trusted.
    pub result: &'a UnitResult,
    /// The oracle hash a person approved (T2, T3), from the store; `None` at T1.
    pub approved_oracle_hash: Option<&'a str>,
    /// The freeze the harness sent with `oracle_frozen`: the frozen files with their hashes and
    /// the test ids in them. `None` for a harness that sent none.
    pub freeze: Option<&'a OracleFreeze>,
    /// The spec bytes a person signed, from the control plane's store. `None` for a unit that
    /// runs from no spec.
    pub signed_spec: Option<&'a [u8]>,
    /// The body the pull request will be opened with. `None` when no pull request follows.
    pub pr_body: Option<&'a str>,
}

/// Which check a verdict is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Check {
    /// The spec file at the head is byte for byte what was signed.
    Spec,
    /// The delivered branch is at the commit the harness named, on the base it was given.
    Commit,
    /// Every change after the spec commit is inside the unit's scope.
    Diff,
    /// No protected path changed.
    Protected,
    /// Every frozen file still has its frozen hash, and the frozen ids are the files' ids.
    Oracle,
    /// The control plane's own test run: every frozen id present and passed.
    Tests,
    /// The delivered branch merges onto the base as it is now.
    Merge,
    /// The pull request's body carries the unit's work item.
    Link,
}

/// What verification concluded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Every check held, and the delivered branch is imported and ready to be pushed.
    Verified,
    /// A `no_change` result was confirmed: there is nothing to deliver.
    NoChange,
    /// The delivery is wrong in a way no re-run can change. The unit fails.
    Rejected { check: Check, detail: String },
    /// A frozen test or a protected path was changed. The unit stops for a person.
    Tampered { detail: String },
    /// The delivered branch does not merge onto the current base.
    MergeConflict,
    /// A check could not be run at all: the container engine is down, the bundle cannot be read,
    /// a report was not produced by a command that exited 0. Never a pass. The unit stops for a
    /// person, who may ask for verification again.
    Unrunnable { check: Check, detail: String },
    /// The control plane's own test run did not pass, whatever the harness reported: `failing`
    /// are the frozen ids that were missing or did not pass. The unit stops for a person, who
    /// may ask for verification again, so one flaky test does not cost the unit.
    TestsFailed {
        failing: Vec<String>,
        detail: String,
    },
}

impl Verdict {
    /// True for the two verdicts after which verification may simply be tried again.
    pub fn can_be_retried(&self) -> bool {
        matches!(
            self,
            Verdict::Unrunnable { .. } | Verdict::TestsFailed { .. }
        )
    }
}

#[async_trait]
pub trait EvidenceVerifier: Send + Sync {
    /// Verify one result. Never panics on what a harness sent, and never answers `Verified` or
    /// `NoChange` for a result it could not check.
    async fn verify(&self, input: VerifyInput<'_>) -> Verdict;
}
````

`crates/fleet-verify/src/verifier.rs`:

````rust
//! The real verifier. Wave 0 gives the shell; lane CC-VERIFY builds the checks.

use crate::seam::{EvidenceVerifier, Verdict, VerifyInput};
use async_trait::async_trait;
use fleet_forge::RepoForge;
use fleet_runner::CheckRunner;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

/// Where a unit's spec file is committed, relative to the repository root. The file's name is
/// the spec's id with `.md`.
pub const SPEC_DIR: &str = ".reqdrive/specs/";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyConfig {
    /// A directory the verifier owns. It exports each unit's tree under it and removes the
    /// export when the verification ends.
    pub work_dir: PathBuf,
    /// The longest one command of a check may run.
    pub check_timeout: Duration,
    /// The longest dependency setup may run.
    pub setup_timeout: Duration,
}

/// [`EvidenceVerifier`] over a forge and a check runner.
pub struct Verifier {
    #[allow(dead_code)] // read by the checks lane CC-VERIFY builds
    forge: Arc<dyn RepoForge>,
    #[allow(dead_code)]
    runner: Arc<dyn CheckRunner>,
    #[allow(dead_code)]
    config: VerifyConfig,
}

impl Verifier {
    pub fn new(
        forge: Arc<dyn RepoForge>,
        runner: Arc<dyn CheckRunner>,
        config: VerifyConfig,
    ) -> Self {
        Self {
            forge,
            runner,
            config,
        }
    }
}

#[async_trait]
impl EvidenceVerifier for Verifier {
    async fn verify(&self, input: VerifyInput<'_>) -> Verdict {
        let _ = input;
        unimplemented!("lane CC-VERIFY builds this")
    }
}
````

- [ ] **Step 2: Write the failing test**

Create `crates/fleet-verify/src/testkit.rs` with only this:

````rust
#[cfg(test)]
mod tests {
    use super::*;
    use factory_presets::preset;
    use fleet_forge::RepoForge;

    #[tokio::test]
    async fn the_scripted_verifier_passes_the_evidence_verifier_contract() {
        evidence_verifier_contract(ScriptedVerifier::new).await;
    }

    #[tokio::test]
    async fn the_scripted_verifier_answers_its_queue_then_verified_and_records_inputs() {
        let dir = tempfile::tempdir().unwrap();
        let world = World::good(dir.path());
        let verifier = ScriptedVerifier::with(vec![Verdict::MergeConflict]);
        verifier.push(Verdict::Tampered { detail: "x".into() });
        assert_eq!(verifier.verify(world.input()).await, Verdict::MergeConflict);
        assert!(matches!(
            verifier.verify(world.input()).await,
            Verdict::Tampered { .. }
        ));
        assert_eq!(verifier.verify(world.input()).await, Verdict::Verified);
        let inputs = verifier.inputs();
        assert_eq!(inputs.len(), 3);
        assert_eq!(inputs[0].order, world.order);
        assert_eq!(inputs[0].freeze.as_ref(), Some(&world.freeze));
        assert_eq!(
            inputs[0].signed_spec.as_deref(),
            Some(&world.spec_bytes[..])
        );
        assert_eq!(inputs[0].pr_body.as_deref(), Some(world.pr_body.as_str()));
    }

    #[tokio::test(start_paused = true)]
    async fn a_delayed_scripted_verifier_answers_after_its_delay() {
        let dir = tempfile::tempdir().unwrap();
        let world = World::good(dir.path());
        let verifier = ScriptedVerifier::new();
        verifier.delay(Duration::from_secs(900));
        let started = tokio::time::Instant::now();
        assert_eq!(verifier.verify(world.input()).await, Verdict::Verified);
        assert!(started.elapsed() >= Duration::from_secs(900));
    }

    #[tokio::test]
    async fn the_good_world_is_consistent_with_the_presets_and_the_forge() {
        let dir = tempfile::tempdir().unwrap();
        let world = World::good(dir.path());
        let node = preset("node").unwrap();
        // The frozen id is the id the preset enumerates from the frozen source and reads from
        // the scripted report.
        assert_eq!(
            node.enumerate_ids(FROZEN_TEST_PATH, FROZEN_TEST_SOURCE)
                .unwrap(),
            [FROZEN_ID]
        );
        let cases = node
            .read_report(&report(TestStatus::Passed, TestStatus::Passed))
            .unwrap();
        let ids: Vec<&str> = cases.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, [FROZEN_ID, BASE_ID]);
        // The delivered bundle imports, its first commit is the spec, and the body carries the item.
        let evidence = world.result.evidence.as_ref().unwrap();
        let DeliveryEvidence::Bundle { bundle_path } = &evidence.delivery else {
            panic!("a bundle delivery")
        };
        let scratch = world
            .forge
            .import_bundle(
                &world.repo,
                "u1",
                &world.base_sha,
                Path::new(bundle_path),
                "factory/u1",
            )
            .await
            .unwrap();
        assert_eq!(scratch.head_sha(), evidence.head_sha);
        let commits = scratch.commits().await.unwrap();
        assert_eq!(commits.len(), 2);
        assert_eq!(
            scratch.read(&commits[0], &world.spec_path()).await.unwrap(),
            Some(world.spec_bytes.clone())
        );
        let item = WorkItemRef::parse("example/sandbox#1", None).unwrap();
        assert!(fleet_forge::body_carries(
            &world.pr_body,
            &item,
            "owner/sandbox"
        ));
        // The scripted test command writes the report where the configuration says.
        assert_eq!(world.spec_path(), ".reqdrive/specs/SPEC-0001.md");
        assert_eq!(
            world.order.config.as_ref().unwrap().test_report,
            "reports/junit.xml"
        );
    }
}
````

- [ ] **Step 3: Run it and see it fail**

```
cargo test -p fleet-verify --lib
```

Expected: the crate does not compile; the errors name `ScriptedVerifier`, `World`,
`evidence_verifier_contract`, `report` and the `FROZEN_*` constants.

- [ ] **Step 4: Write the builders, the scripted verifier, the world and the suite**

Replace `crates/fleet-verify/src/testkit.rs` with:

````rust
//! Builders for the verifier's inputs, a scripted verifier, a whole arranged unit ([`World`]),
//! and the contract suite every [`EvidenceVerifier`] must pass.
//!
//! Compiled for this crate's own tests, and for other crates behind the `testkit` feature.

use crate::seam::{Check, EvidenceVerifier, Verdict, VerifyInput};
use crate::verifier::{Verifier, VerifyConfig, SPEC_DIR};
use async_trait::async_trait;
use factory_presets::testkit::ReportBuilder;
use factory_presets::TestStatus;
use factory_spec::testkit::SpecBuilder;
use fleet_forge::testkit::{FileChange, MemForge};
use fleet_forge::{pr_body, RepoRef, WorkItemRef};
use fleet_runner::testkit::{FakeRunner, Scripted, IMAGE};
use harness_protocol::{
    bundle_hash, file_sha256, Capabilities, Caps, ControlKind, Delivery, DeliveryEvidence,
    Evidence, FrozenFile, GateKind, Isolation, Metering, OracleFreeze, Outcome, PresetInfo, Repo,
    RepoCommands, RepoConfig, Scope, Source, SpecRef, TestRun, Tier, UnitKind, UnitResult,
    WorkItem, WorkItemKind, WorkOrder,
};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

// ---------------------------------------------------------------------------------------------
// Builders
// ---------------------------------------------------------------------------------------------

/// What a container harness that meters in USD, has the oracle gate and delivers bundles
/// declares, with the `node` and `cargo` presets at this build's version.
pub fn capabilities() -> Capabilities {
    let preset = |name: &str| PresetInfo {
        name: name.into(),
        version: factory_presets::PRESETS_VERSION.into(),
    };
    Capabilities {
        isolation: Isolation::Container,
        metering: Metering::Usd,
        gates: vec![GateKind::Oracle],
        delivery: Delivery::Bundle,
        resume: true,
        halt: true,
        holdouts: false,
        controls: vec![ControlKind::Scope, ControlKind::Protected],
        network: None,
        profiles: Vec::new(),
        kinds: vec![UnitKind::Build],
        presets: vec![preset("cargo"), preset("node")],
    }
}

/// The configuration of a repository on the `node` preset.
pub fn node_config() -> RepoConfig {
    RepoConfig {
        preset: "node".into(),
        image: IMAGE.into(),
        env: Default::default(),
        commands: RepoCommands {
            setup: "npm ci".into(),
            build: "npm run build".into(),
            test: "npm test".into(),
            format_check: "npm run format:check".into(),
            lint: "npm run lint".into(),
        },
        test_report: "reports/junit.xml".into(),
        test_dirs: vec!["test/".into()],
        manifests: vec!["package.json".into()],
        lockfiles: vec!["package-lock.json".into()],
    }
}

/// A T1 work order for `unit_id` in `owner/sandbox`, on branch `factory/<unit_id>`, with no
/// spec, source, scope or configuration: what a 0.1-shaped unit carries.
pub fn bare_order(unit_id: &str) -> WorkOrder {
    WorkOrder {
        unit_id: unit_id.into(),
        work_item: WorkItem {
            kind: WorkItemKind::Issue,
            reference: "example/sandbox#1".into(),
            fingerprint: None,
        },
        tier: Tier::T1,
        task: "A cart accepts one percent discount code.".into(),
        repo: Repo {
            url: "https://example.invalid/owner/sandbox".into(),
            slug: "owner/sandbox".into(),
            base_branch: "main".into(),
        },
        branch: format!("factory/{unit_id}"),
        test_cmd: "npm test".into(),
        caps: Caps {
            usd: 5.0,
            wall_clock_secs: 1800,
            min_review_rounds: 1,
        },
        resume: None,
        kind: Some(UnitKind::Build),
        spec: None,
        source: None,
        scope: None,
        scope_grants: Vec::new(),
        permitted_dependencies: Vec::new(),
        expected_red: Vec::new(),
        config: None,
        controls: None,
        profile: None,
        parent: None,
    }
}

/// Evidence for a bundle of `branch` at `head_sha`, with a test run that exited 0.
pub fn evidence(branch: &str, head_sha: &str, bundle_path: &str) -> Evidence {
    Evidence {
        branch: branch.into(),
        head_sha: head_sha.into(),
        delivery: DeliveryEvidence::Bundle {
            bundle_path: bundle_path.into(),
        },
        pr: None,
        test: TestRun {
            command: "npm test".into(),
            exit_code: 0,
        },
        oracle_hash: None,
        spec_hash: None,
        map: None,
        test_report: None,
        controls: Vec::new(),
        review: None,
    }
}

/// A `pr_open` result carrying `evidence`.
pub fn pr_open(evidence: Evidence) -> UnitResult {
    UnitResult {
        outcome: Outcome::PrOpen,
        evidence: Some(evidence),
        failure: None,
        stop: None,
    }
}

/// A `no_change` result carrying `evidence`.
pub fn no_change(evidence: Evidence) -> UnitResult {
    UnitResult {
        outcome: Outcome::NoChange,
        ..pr_open(evidence)
    }
}

/// A freeze of `files` (path and content), listing `ids`.
pub fn freeze(files: &[(&str, &str)], ids: &[&str]) -> OracleFreeze {
    OracleFreeze {
        frozen_files: files
            .iter()
            .map(|(path, content)| FrozenFile {
                path: path.to_string(),
                sha256: file_sha256(content.as_bytes()),
            })
            .collect(),
        frozen_ids: ids.iter().map(|s| s.to_string()).collect(),
        holdout_bundle_path: None,
        holdout_hash: None,
        holdout_ids: Vec::new(),
    }
}

// ---------------------------------------------------------------------------------------------
// The scripted verifier
// ---------------------------------------------------------------------------------------------

/// An owned copy of one [`VerifyInput`], kept so a test can assert what reached the verifier.
#[derive(Debug, Clone, PartialEq)]
pub struct RecordedInput {
    pub order: WorkOrder,
    pub capabilities: Capabilities,
    pub result: UnitResult,
    pub approved_oracle_hash: Option<String>,
    pub freeze: Option<OracleFreeze>,
    pub signed_spec: Option<Vec<u8>>,
    pub pr_body: Option<String>,
}

/// An [`EvidenceVerifier`] that answers from a queue, `Verified` once the queue is empty, and
/// records every input. It still refuses what no verifier may approve: a result with no
/// evidence, a push delivery, and a spec whose signed bytes are missing or have another hash.
#[derive(Default)]
pub struct ScriptedVerifier {
    queue: Mutex<VecDeque<Verdict>>,
    inputs: Mutex<Vec<RecordedInput>>,
    delay: Mutex<Duration>,
}

impl ScriptedVerifier {
    pub fn new() -> Self {
        Self::default()
    }

    /// Answers `verdicts` in order, then `Verified`.
    pub fn with(verdicts: Vec<Verdict>) -> Self {
        Self {
            queue: Mutex::new(verdicts.into()),
            ..Self::default()
        }
    }

    /// Add one verdict to the end of the queue.
    pub fn push(&self, verdict: Verdict) {
        self.queue.lock().unwrap().push_back(verdict);
    }

    /// Make every later verification take this long before it answers.
    pub fn delay(&self, by: Duration) {
        *self.delay.lock().unwrap() = by;
    }

    /// Every input received so far, in order.
    pub fn inputs(&self) -> Vec<RecordedInput> {
        self.inputs.lock().unwrap().clone()
    }
}

fn never_approvable(input: &VerifyInput<'_>) -> Option<Verdict> {
    let Some(evidence) = &input.result.evidence else {
        return Some(Verdict::Rejected {
            check: Check::Commit,
            detail: "the result carries no evidence".into(),
        });
    };
    if matches!(evidence.delivery, DeliveryEvidence::Push) {
        return Some(Verdict::Rejected {
            check: Check::Link,
            detail: "push delivery is not accepted".into(),
        });
    }
    if let Some(spec) = &input.order.spec {
        match input.signed_spec {
            None => {
                return Some(Verdict::Unrunnable {
                    check: Check::Spec,
                    detail: "the signed bytes were not supplied".into(),
                })
            }
            Some(bytes) if factory_spec::sha256_hex(bytes) != spec.signed_hash => {
                return Some(Verdict::Rejected {
                    check: Check::Spec,
                    detail: "the supplied bytes are not the bytes that were signed".into(),
                })
            }
            Some(_) => {}
        }
    }
    None
}

#[async_trait]
impl EvidenceVerifier for ScriptedVerifier {
    async fn verify(&self, input: VerifyInput<'_>) -> Verdict {
        self.inputs.lock().unwrap().push(RecordedInput {
            order: input.order.clone(),
            capabilities: input.capabilities.clone(),
            result: input.result.clone(),
            approved_oracle_hash: input.approved_oracle_hash.map(str::to_string),
            freeze: input.freeze.cloned(),
            signed_spec: input.signed_spec.map(<[u8]>::to_vec),
            pr_body: input.pr_body.map(str::to_string),
        });
        let delay = *self.delay.lock().unwrap();
        if !delay.is_zero() {
            tokio::time::sleep(delay).await;
        }
        if let Some(refusal) = never_approvable(&input) {
            return refusal;
        }
        self.queue
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(Verdict::Verified)
    }
}

// ---------------------------------------------------------------------------------------------
// A whole arranged unit
// ---------------------------------------------------------------------------------------------

/// The frozen test file every [`World`] starts with.
pub const FROZEN_TEST_PATH: &str = "test/codes.test.js";
/// Its content.
pub const FROZEN_TEST_SOURCE: &str = "import { describe, it } from 'node:test';\n\ndescribe('codes', () => {\n  it('ac1 applies a percent code', () => {});\n});\n";
/// The one test id in it.
pub const FROZEN_ID: &str = "test/codes.test.js > codes > ac1 applies a percent code";
/// The id of the test the base already had.
pub const BASE_ID: &str = "test/cart.test.js > cart > totals";

/// One unit, arranged end to end on the in-memory forge and the scripted runner, that a correct
/// verifier answers `Verified` for. A test changes one thing and asserts the verdict.
///
/// The repository is `owner/sandbox` on the `node` preset. Its base has `src/cart.js`, one test
/// file, a manifest, a lockfile and `.reqdrive/config.toml`. The signed spec is
/// `SpecBuilder::new()`: it may change `src/cart.js` and add files under `test/`. The delivered
/// branch `factory/u1` has two commits: the spec file, then the work (the frozen test file and a
/// change to `src/cart.js`). `npm test` is scripted to write a report in which both tests pass.
pub struct World {
    pub forge: Arc<MemForge>,
    pub runner: Arc<FakeRunner>,
    pub dir: PathBuf,
    pub repo: RepoRef,
    pub base_sha: String,
    pub spec_bytes: Vec<u8>,
    pub order: WorkOrder,
    pub capabilities: Capabilities,
    pub freeze: OracleFreeze,
    pub result: UnitResult,
    pub pr_body: String,
    pub approved_oracle_hash: Option<String>,
}

/// The work commit of a good unit.
pub fn good_work() -> Vec<(&'static str, Option<&'static str>)> {
    vec![
        (FROZEN_TEST_PATH, Some(FROZEN_TEST_SOURCE)),
        (
            "src/cart.js",
            Some("export const total = (code) => (code ? 90 : 100);\n"),
        ),
    ]
}

/// A report in which the frozen test and the base's test have the given outcomes.
pub fn report(frozen: TestStatus, base: TestStatus) -> String {
    ReportBuilder::vitest()
        .case(
            FROZEN_TEST_PATH,
            "codes > ac1 applies a percent code",
            frozen,
        )
        .case("test/cart.test.js", "cart > totals", base)
        .build()
}

impl World {
    /// `dir` is an empty directory the world may write into.
    pub fn good(dir: &Path) -> World {
        let forge = Arc::new(MemForge::new());
        let repo = fleet_forge::testkit::repo();
        let base_sha = forge.commit_base(
            &repo,
            &[
                ("src/cart.js", Some("export const total = () => 100;\n")),
                (
                    "test/cart.test.js",
                    Some("import { describe, it } from 'node:test';\n\ndescribe('cart', () => {\n  it('totals', () => {});\n});\n"),
                ),
                ("package.json", Some("{\"name\":\"sandbox\"}\n")),
                ("package-lock.json", Some("{\"lockfileVersion\":3}\n")),
                (".reqdrive/config.toml", Some("preset = \"node\"\n")),
                ("vitest.config.js", Some("export default {};\n")),
            ],
        );
        let spec_bytes = SpecBuilder::new().build();
        let spec = SpecBuilder::new().spec();
        let spec_file = dir.join("spec.md");
        std::fs::write(&spec_file, &spec_bytes).unwrap();
        let source_bundle = dir.join("source.bundle");

        let mut order = bare_order("u1");
        order.spec = Some(SpecRef {
            id: spec.id.clone(),
            bytes_path: spec_file.to_string_lossy().into_owned(),
            signed_hash: factory_spec::sha256_hex(&spec_bytes),
            child: None,
        });
        order.source = Some(Source {
            bundle_path: source_bundle.to_string_lossy().into_owned(),
            base_sha: base_sha.clone(),
        });
        order.scope = Some(Scope {
            touched_files: spec.scope.touched_files.clone(),
            create_paths: spec.scope.create_paths.clone(),
            test_paths: spec.scope.test_paths.clone(),
            touched_tests: spec.scope.touched_tests.clone(),
        });
        order.config = Some(node_config());

        let work_item = WorkItemRef::parse(&order.work_item.reference, None).unwrap();
        let mut world = World {
            forge,
            runner: Arc::new(FakeRunner::new()),
            dir: dir.to_path_buf(),
            repo,
            base_sha,
            spec_bytes,
            order,
            capabilities: capabilities(),
            freeze: freeze(&[(FROZEN_TEST_PATH, FROZEN_TEST_SOURCE)], &[FROZEN_ID]),
            result: pr_open(evidence("factory/u1", "", "")),
            pr_body: pr_body(
                &work_item,
                "owner/sandbox",
                "A cart accepts one percent discount code.",
            ),
            approved_oracle_hash: None,
        };
        world.deliver(&[&good_work()]);
        world.script_tests(Scripted::exit(0, "2 passed\n").writing(
            "reports/junit.xml",
            report(TestStatus::Passed, TestStatus::Passed).as_bytes(),
        ));
        world
    }

    /// The path of the spec file in the repository.
    pub fn spec_path(&self) -> String {
        format!("{SPEC_DIR}{}.md", self.order.spec.as_ref().unwrap().id)
    }

    /// Deliver again: the spec commit, exactly as signed, then one commit per entry of `work`.
    pub fn deliver(&mut self, work: &[&[FileChange<'_>]]) {
        let spec_path = self.spec_path();
        let spec_text = String::from_utf8(self.spec_bytes.clone()).unwrap();
        let spec_commit: Vec<FileChange<'_>> = vec![(spec_path.as_str(), Some(spec_text.as_str()))];
        let mut commits: Vec<&[FileChange<'_>]> = vec![&spec_commit];
        commits.extend_from_slice(work);
        self.deliver_exactly(&commits);
    }

    /// Deliver again with exactly these commits, and no spec commit added.
    pub fn deliver_exactly(&mut self, commits: &[&[FileChange<'_>]]) {
        let bundle = self.dir.join("delivered.bundle");
        let head = self
            .forge
            .write_unit_bundle(&self.base_sha, "factory/u1", commits, &bundle);
        let mut reported = evidence("factory/u1", &head, &bundle.to_string_lossy());
        reported.spec_hash = Some(factory_spec::sha256_hex(&self.spec_bytes));
        reported.oracle_hash = Some(bundle_hash(&self.freeze.frozen_files));
        self.result = pr_open(reported);
    }

    /// Replace what the repository's test command does in the verifier's check container.
    pub fn script_tests(&mut self, scripted: Scripted) {
        let runner = FakeRunner::new();
        runner.on("npm test", scripted);
        self.runner = Arc::new(runner);
    }

    pub fn input(&self) -> VerifyInput<'_> {
        VerifyInput {
            order: &self.order,
            capabilities: &self.capabilities,
            result: &self.result,
            approved_oracle_hash: self.approved_oracle_hash.as_deref(),
            freeze: Some(&self.freeze),
            signed_spec: Some(&self.spec_bytes),
            pr_body: Some(&self.pr_body),
        }
    }

    /// The real verifier over this world's forge and runner.
    pub fn verifier(&self) -> Verifier {
        let work_dir = self.dir.join("verify");
        std::fs::create_dir_all(&work_dir).unwrap();
        Verifier::new(
            self.forge.clone(),
            self.runner.clone(),
            VerifyConfig {
                work_dir,
                check_timeout: Duration::from_secs(60),
                setup_timeout: Duration::from_secs(60),
            },
        )
    }
}

// ---------------------------------------------------------------------------------------------
// The contract suite
// ---------------------------------------------------------------------------------------------

fn approves(verdict: &Verdict) -> bool {
    matches!(verdict, Verdict::Verified | Verdict::NoChange)
}

/// What every [`EvidenceVerifier`] must do, whatever it is built on: these are the results no
/// verifier may approve, and none of them needs a repository to refuse. `make` returns a new
/// verifier each time. Panics on the first rule an implementation breaks.
pub async fn evidence_verifier_contract<V: EvidenceVerifier>(make: impl Fn() -> V) {
    let capabilities = capabilities();
    let spec_bytes = SpecBuilder::new().build();
    let mut order = bare_order("u1");
    order.spec = Some(SpecRef {
        id: "SPEC-0001".into(),
        bytes_path: "unused".into(),
        signed_hash: factory_spec::sha256_hex(&spec_bytes),
        child: None,
    });
    let delivered = pr_open(evidence("factory/u1", &"a".repeat(40), "no-such.bundle"));
    fn input<'a>(
        order: &'a WorkOrder,
        capabilities: &'a Capabilities,
        result: &'a UnitResult,
        signed_spec: Option<&'a [u8]>,
    ) -> VerifyInput<'a> {
        VerifyInput {
            order,
            capabilities,
            result,
            approved_oracle_hash: None,
            freeze: None,
            signed_spec,
            pr_body: None,
        }
    }
    let ask = |result: UnitResult, signed: Option<Vec<u8>>| {
        let verifier = make();
        let (order, capabilities) = (order.clone(), capabilities.clone());
        async move {
            verifier
                .verify(input(&order, &capabilities, &result, signed.as_deref()))
                .await
        }
    };

    // A result with no evidence.
    let mut bare = delivered.clone();
    bare.evidence = None;
    let verdict = ask(bare, Some(spec_bytes.clone())).await;
    assert!(!approves(&verdict), "no evidence was approved: {verdict:?}");

    // A push delivery.
    let mut pushed = delivered.clone();
    pushed.evidence.as_mut().unwrap().delivery = DeliveryEvidence::Push;
    let verdict = ask(pushed, Some(spec_bytes.clone())).await;
    assert!(
        !approves(&verdict),
        "a push delivery was approved: {verdict:?}"
    );

    // A unit with a spec, verified without the signed bytes.
    let verdict = ask(delivered.clone(), None).await;
    assert!(
        !approves(&verdict),
        "missing signed bytes were approved: {verdict:?}"
    );

    // Signed bytes that are not the bytes the work order names.
    let mut other = spec_bytes.clone();
    other.push(b' ');
    let verdict = ask(delivered.clone(), Some(other)).await;
    assert!(
        !approves(&verdict),
        "other bytes were approved: {verdict:?}"
    );

    // The same for a `no_change` result.
    let unchanged = no_change(delivered.evidence.clone().unwrap());
    let verdict = ask(unchanged, None).await;
    assert!(
        !approves(&verdict),
        "no_change without signed bytes was approved: {verdict:?}"
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use factory_presets::preset;
    use fleet_forge::RepoForge;

    #[tokio::test]
    async fn the_scripted_verifier_passes_the_evidence_verifier_contract() {
        evidence_verifier_contract(ScriptedVerifier::new).await;
    }

    #[tokio::test]
    async fn the_scripted_verifier_answers_its_queue_then_verified_and_records_inputs() {
        let dir = tempfile::tempdir().unwrap();
        let world = World::good(dir.path());
        let verifier = ScriptedVerifier::with(vec![Verdict::MergeConflict]);
        verifier.push(Verdict::Tampered { detail: "x".into() });
        assert_eq!(verifier.verify(world.input()).await, Verdict::MergeConflict);
        assert!(matches!(
            verifier.verify(world.input()).await,
            Verdict::Tampered { .. }
        ));
        assert_eq!(verifier.verify(world.input()).await, Verdict::Verified);
        let inputs = verifier.inputs();
        assert_eq!(inputs.len(), 3);
        assert_eq!(inputs[0].order, world.order);
        assert_eq!(inputs[0].freeze.as_ref(), Some(&world.freeze));
        assert_eq!(
            inputs[0].signed_spec.as_deref(),
            Some(&world.spec_bytes[..])
        );
        assert_eq!(inputs[0].pr_body.as_deref(), Some(world.pr_body.as_str()));
    }

    #[tokio::test(start_paused = true)]
    async fn a_delayed_scripted_verifier_answers_after_its_delay() {
        let dir = tempfile::tempdir().unwrap();
        let world = World::good(dir.path());
        let verifier = ScriptedVerifier::new();
        verifier.delay(Duration::from_secs(900));
        let started = tokio::time::Instant::now();
        assert_eq!(verifier.verify(world.input()).await, Verdict::Verified);
        assert!(started.elapsed() >= Duration::from_secs(900));
    }

    #[tokio::test]
    async fn the_good_world_is_consistent_with_the_presets_and_the_forge() {
        let dir = tempfile::tempdir().unwrap();
        let world = World::good(dir.path());
        let node = preset("node").unwrap();
        // The frozen id is the id the preset enumerates from the frozen source and reads from
        // the scripted report.
        assert_eq!(
            node.enumerate_ids(FROZEN_TEST_PATH, FROZEN_TEST_SOURCE)
                .unwrap(),
            [FROZEN_ID]
        );
        let cases = node
            .read_report(&report(TestStatus::Passed, TestStatus::Passed))
            .unwrap();
        let ids: Vec<&str> = cases.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, [FROZEN_ID, BASE_ID]);
        // The delivered bundle imports, its first commit is the spec, and the body carries the item.
        let evidence = world.result.evidence.as_ref().unwrap();
        let DeliveryEvidence::Bundle { bundle_path } = &evidence.delivery else {
            panic!("a bundle delivery")
        };
        let scratch = world
            .forge
            .import_bundle(
                &world.repo,
                "u1",
                &world.base_sha,
                Path::new(bundle_path),
                "factory/u1",
            )
            .await
            .unwrap();
        assert_eq!(scratch.head_sha(), evidence.head_sha);
        let commits = scratch.commits().await.unwrap();
        assert_eq!(commits.len(), 2);
        assert_eq!(
            scratch.read(&commits[0], &world.spec_path()).await.unwrap(),
            Some(world.spec_bytes.clone())
        );
        let item = WorkItemRef::parse("example/sandbox#1", None).unwrap();
        assert!(fleet_forge::body_carries(
            &world.pr_body,
            &item,
            "owner/sandbox"
        ));
        // The scripted test command writes the report where the configuration says.
        assert_eq!(world.spec_path(), ".reqdrive/specs/SPEC-0001.md");
        assert_eq!(
            world.order.config.as_ref().unwrap().test_report,
            "reports/junit.xml"
        );
    }
}
````

- [ ] **Step 5: Run the tests**

```
cargo test -p fleet-verify --lib
cargo test -p fleet-verify --lib --features testkit
```

Expected, both times: `test result: ok. 4 passed; 0 failed`.

- [ ] **Step 6: Commit**

```
git add crates/fleet-verify/src
git commit -m "feat(fleet-verify): the verifier seam at protocol 0.2, a scripted verifier and an arranged unit"
```

### Task 6: `fleet-admission` — the admission seam

**Files:**
- Replace: `crates/fleet-admission/src/lib.rs`
- Create: `crates/fleet-admission/src/types.rs`, `admission.rs`, `testkit.rs`

**Produces:** the `Admit` seam with `DispatchRequest`, `Admitted`, `Refusal`,
`AdmissionConfig`; the `HarnessCatalog` port with `HarnessStatus`; `Admission`,
`parse_repo_config`, `ConfigError`, `REPO_CONFIG_PATH`. In `testkit`: `FakeAdmission`,
`FakeCatalog`, `Asked`, `Desk`, `config`, `request`, `capabilities`, `repo_config`,
`admitted_record`, `REPO_CONFIG_TOML`, `PRESETS_VERSION`, `admit_contract`.

What to know:

- `Refusal`'s variants are in the order the checks run. Each has a stable `code()`; the API
  maps a code to an HTTP status.
- Admission holds no eligibility rule. It asks the `HarnessCatalog`, which `fleetd` implements
  with `fleet_supervisor::eligibility` over the capabilities a probe returned, so the rule has
  one implementation.
- `Admission::mint_unit_id` hands out the next id of the counter `admit` draws on. The old
  request shape, which does not go through `admit`, takes its ids there (lane CC-WIRE), so
  the two paths cannot mint one id twice.
- `Desk` is everything the real `Admission` reads, arranged so that its request is admitted.

- [ ] **Step 1: Create the source files other than `testkit.rs`**

`crates/fleet-admission/src/lib.rs`:

````rust
//! `fleet-admission` — decides whether a requested unit may start, and records the ones that
//! may. Every refusal names its reason, and the checks run in a fixed order, so the same request
//! is always refused for the same thing.
//!
//! [`Admit`] is the seam the API calls. [`Admission`] is the real one, over the store, the forge
//! and a [`HarnessCatalog`]. `testkit::FakeAdmission` admits whatever it is not told to refuse.

mod admission;
mod types;

#[cfg(any(test, feature = "testkit"))]
pub mod testkit;

pub use admission::{parse_repo_config, Admission, REPO_CONFIG_PATH};
pub use types::{
    AdmissionConfig, Admit, Admitted, ConfigError, DispatchRequest, HarnessCatalog, HarnessStatus,
    Refusal,
};
````

`crates/fleet-admission/src/types.rs`:

````rust
//! The admission seam: its configuration, its request, its two answers and its ports.

use async_trait::async_trait;
use fleet_core::Tier;
use fleet_forge::RepoRef;
use fleet_store::UnitRecord;
use harness_protocol::{Capabilities, RepoConfig};

/// What the operator configures. Nothing here is hard-coded anywhere else.
#[derive(Debug, Clone, PartialEq)]
pub struct AdmissionConfig {
    /// The repositories a unit may be dispatched to. Anything else is refused.
    pub repos: Vec<RepoRef>,
    /// The most one unit may spend, in USD. A request may ask for less and never for more.
    pub unit_usd_ceiling: f64,
    /// The longest one unit's agents may run, in seconds. A request may ask for less.
    pub unit_wall_clock_ceiling_secs: u64,
    /// The most all units first written inside the window may commit, in USD.
    pub global_usd_cap: f64,
    /// The length of the spend window, in milliseconds.
    pub spend_window_ms: i64,
    /// The review floor a unit is started with.
    pub min_review_rounds: u32,
    /// The names of the presets this build links.
    pub presets: Vec<String>,
    /// The version of the preset rules this build links. A harness on another version is refused.
    pub presets_version: String,
}

/// One request to start a unit from a signed spec.
#[derive(Debug, Clone, PartialEq)]
pub struct DispatchRequest {
    /// The work item, as `owner/repo#N`.
    pub work_item: String,
    pub fingerprint: Option<String>,
    /// The repository to change, as `owner/name`.
    pub repo: String,
    /// SHA-256 of the signed spec bytes to run from.
    pub spec_sha256: String,
    pub profile: Option<String>,
    /// The registry name of the harness to run it.
    pub harness: String,
    /// The caller accepts a weaker guarantee than the default for this one unit.
    pub opt_in: bool,
    /// Chosen by the caller. The same key never starts two units.
    pub idempotency_key: String,
    /// A cap below the configured ceiling, in USD.
    pub usd_cap: Option<f64>,
    /// A wall-clock cap below the configured ceiling, in seconds.
    pub wall_clock_secs: Option<u64>,
}

/// Why a request was refused. The variants are in the order the checks run.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum Refusal {
    #[error("the request has no idempotency key")]
    MissingKey,
    #[error("{reference:?} is not a work item of the form owner/repo#N")]
    WorkItemMalformed { reference: String },
    #[error("idempotency key already used by unit {unit_id}")]
    DuplicateKey { unit_id: String },
    #[error("repository {repo} is not on the allowlist")]
    RepoNotAllowed { repo: String },
    #[error("the repository could not be read: {detail}")]
    Forge { detail: String },
    #[error("repository {repo} is not onboarded: {detail}")]
    NotOnboarded { repo: String, detail: String },
    #[error("this control plane has no preset named {preset:?}")]
    PresetUnsupported { preset: String },
    #[error("no harness is registered as {harness:?}")]
    HarnessUnknown { harness: String },
    #[error("harness {harness} is not healthy: {error}")]
    HarnessUnhealthy { harness: String, error: String },
    #[error("harness {harness} does not declare the {preset} preset")]
    PresetNotDeclared { preset: String, harness: String },
    #[error("preset rules differ: the control plane has {ours}, harness {harness} has {theirs}")]
    PresetsVersionMismatch {
        harness: String,
        ours: String,
        theirs: String,
    },
    #[error("no signature is stored for spec bytes {spec_sha256} in {repo}")]
    NotSigned { repo: String, spec_sha256: String },
    #[error("the stored spec bytes do not parse: {detail}")]
    SpecUnreadable { detail: String },
    #[error("the signed spec is for {spec}, not for {request}")]
    WorkItemMismatch { request: String, spec: String },
    #[error("the {cap} cap asked for ({requested}) is above the ceiling ({ceiling})")]
    CapAboveCeiling {
        cap: &'static str,
        requested: f64,
        ceiling: f64,
    },
    #[error("the spend ceiling is reached: ${committed:.2} committed of ${cap:.2}")]
    SpendCeiling { committed: f64, cap: f64 },
    #[error("committed spend could not be read, so nothing is admitted: {detail}")]
    SpendUnknown { detail: String },
    #[error("harness {harness} may not run this unit: {reason}")]
    Ineligible { harness: String, reason: String },
    #[error("the unit's reservation could not be recorded, so it was not started: {detail}")]
    ReservationFailed { detail: String },
}

impl Refusal {
    /// A stable short name, for callers that branch on the reason.
    pub fn code(&self) -> &'static str {
        match self {
            Refusal::MissingKey => "missing_key",
            Refusal::WorkItemMalformed { .. } => "work_item_malformed",
            Refusal::DuplicateKey { .. } => "duplicate_key",
            Refusal::RepoNotAllowed { .. } => "repo_not_allowed",
            Refusal::HarnessUnknown { .. } => "harness_unknown",
            Refusal::HarnessUnhealthy { .. } => "harness_unhealthy",
            Refusal::NotOnboarded { .. } => "not_onboarded",
            Refusal::PresetUnsupported { .. } => "preset_unsupported",
            Refusal::PresetNotDeclared { .. } => "preset_not_declared",
            Refusal::PresetsVersionMismatch { .. } => "presets_version_mismatch",
            Refusal::NotSigned { .. } => "not_signed",
            Refusal::SpecUnreadable { .. } => "spec_unreadable",
            Refusal::WorkItemMismatch { .. } => "work_item_mismatch",
            Refusal::CapAboveCeiling { .. } => "cap_above_ceiling",
            Refusal::SpendCeiling { .. } => "spend_ceiling",
            Refusal::SpendUnknown { .. } => "spend_unknown",
            Refusal::Ineligible { .. } => "ineligible",
            Refusal::ReservationFailed { .. } => "reservation_failed",
            Refusal::Forge { .. } => "forge",
        }
    }
}

/// A unit that may start. Its row is already in the store, in `queued`, and its cap already
/// counts as committed spend.
#[derive(Debug, Clone, PartialEq)]
pub struct Admitted {
    pub unit_id: String,
    /// The row and extension exactly as they were stored.
    pub record: UnitRecord,
    /// The signed bytes, from the store.
    pub signed_spec: Vec<u8>,
    /// The repository's configuration, read at `base_sha`.
    pub config: RepoConfig,
    /// The commit the unit's source bundle is cut at.
    pub base_sha: String,
    /// True when every concurrency slot is taken, so the unit will wait in `provisioning`.
    pub waits_for_slot: bool,
}

/// The admission seam.
#[async_trait]
pub trait Admit: Send + Sync {
    /// Admit or refuse one request. `now_ms` is the caller's clock.
    async fn admit(&self, request: &DispatchRequest, now_ms: i64) -> Result<Admitted, Refusal>;
}

/// What admission knows about one registered harness.
#[derive(Debug, Clone, PartialEq)]
pub enum HarnessStatus {
    /// No harness has this name.
    Unknown,
    /// The harness did not answer its probe, or has not been probed.
    Unhealthy {
        error: String,
    },
    Healthy {
        capabilities: Capabilities,
    },
}

/// The harness registry, as admission sees it. `fleetd` implements it over the supervisor's
/// registry, so the eligibility rule has one implementation, the supervisor's.
pub trait HarnessCatalog: Send + Sync {
    fn status(&self, name: &str) -> HarnessStatus;
    /// Whether the named harness may run a unit of this tier, given what it declares. `Err`
    /// carries the reason, naming the guarantee that is missing.
    fn eligibility(
        &self,
        name: &str,
        tier: Tier,
        opt_in: bool,
        profile: Option<&str>,
    ) -> Result<(), String>;
}

/// `.reqdrive/config.toml` could not be read as a repository configuration.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConfigError {
    /// Not TOML, a missing key, an unknown key, or a value of the wrong type.
    #[error("the file does not read: {0}")]
    Malformed(String),
    /// The image is named by tag, not by `@sha256:` and 64 hex digits.
    #[error("image {image:?} is not pinned to a digest")]
    ImageNotPinned { image: String },
    /// A path is absolute, has a `..` segment, a backslash or a wildcard, or a directory in
    /// `test_dirs` does not end in `/`.
    #[error("{key}: {path:?} is not a usable path")]
    BadPath { key: &'static str, path: String },
    /// One of the five commands, or the report path, is empty.
    #[error("{key} is empty")]
    Empty { key: &'static str },
}
````

`crates/fleet-admission/src/admission.rs`:

````rust
//! The real admission. Wave 0 gives the shell; lane CC-ADMISSION builds the checks.

use crate::types::{
    AdmissionConfig, Admit, Admitted, ConfigError, DispatchRequest, HarnessCatalog, Refusal,
};
use async_trait::async_trait;
use fleet_forge::RepoForge;
use fleet_store::{SignatureStore, StoreError, UnitStore};
use harness_protocol::RepoConfig;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use tokio::sync::Semaphore;

/// Where a repository declares its configuration, relative to its root.
pub const REPO_CONFIG_PATH: &str = ".reqdrive/config.toml";

/// Read `.reqdrive/config.toml`. The file maps field for field onto the protocol's `RepoConfig`.
pub fn parse_repo_config(toml_text: &str) -> Result<RepoConfig, ConfigError> {
    let _ = toml_text;
    unimplemented!("lane CC-ADMISSION builds this")
}

/// [`Admit`] over the store, the forge, the harness catalog and the concurrency slots.
pub struct Admission {
    #[allow(dead_code)] // read by the checks lane CC-ADMISSION builds
    config: AdmissionConfig,
    #[allow(dead_code)]
    units: Arc<dyn UnitStore>,
    #[allow(dead_code)]
    signatures: Arc<dyn SignatureStore>,
    #[allow(dead_code)]
    forge: Arc<dyn RepoForge>,
    #[allow(dead_code)]
    harnesses: Arc<dyn HarnessCatalog>,
    #[allow(dead_code)]
    slots: Arc<Semaphore>,
    #[allow(dead_code)]
    next_id: AtomicU64,
}

impl Admission {
    /// `slots` is the semaphore the supervisors take their concurrency slot from; admission only
    /// looks at it. Unit ids continue after the highest `u<n>` in the store, so a restart never
    /// mints an id twice; an `Err` here means the store could not be read.
    pub fn new(
        config: AdmissionConfig,
        units: Arc<dyn UnitStore>,
        signatures: Arc<dyn SignatureStore>,
        forge: Arc<dyn RepoForge>,
        harnesses: Arc<dyn HarnessCatalog>,
        slots: Arc<Semaphore>,
    ) -> Result<Self, StoreError> {
        let _ = (config, units, signatures, forge, harnesses, slots);
        unimplemented!("lane CC-ADMISSION builds this")
    }

    /// The next unit id, `u<n>`, from the counter [`Admit::admit`] draws on. The daemon's old
    /// request shape, which does not go through `admit`, takes its ids here, so that the two
    /// paths can never mint one id twice.
    pub fn mint_unit_id(&self) -> String {
        unimplemented!("lane CC-ADMISSION builds this")
    }
}

#[async_trait]
impl Admit for Admission {
    async fn admit(&self, request: &DispatchRequest, now_ms: i64) -> Result<Admitted, Refusal> {
        let _ = (request, now_ms);
        unimplemented!("lane CC-ADMISSION builds this")
    }
}
````

- [ ] **Step 2: Write the failing test**

Create `crates/fleet-admission/src/testkit.rs` with only this:

````rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn the_fake_admission_passes_the_admit_contract() {
        admit_contract(|| (FakeAdmission::new(), request(&"ab".repeat(32), "key-1"))).await;
    }

    #[tokio::test]
    async fn a_queued_refusal_is_given_once_and_requests_are_recorded() {
        let admission = FakeAdmission::new();
        admission.refuse_next(Refusal::NotSigned {
            repo: "owner/sandbox".into(),
            spec_sha256: "ab".repeat(32),
        });
        let asked = request(&"ab".repeat(32), "key-1");
        assert_eq!(
            admission.admit(&asked, 0).await.unwrap_err().code(),
            "not_signed"
        );
        assert!(admission.admit(&asked, 0).await.is_ok());
        assert_eq!(admission.requests().len(), 2);
    }

    #[test]
    fn the_scripted_catalog_answers_what_it_was_told() {
        let catalog = FakeCatalog::with_reqdrive();
        assert!(matches!(
            catalog.status("reqdrive"),
            HarnessStatus::Healthy { .. }
        ));
        assert_eq!(catalog.status("nobody"), HarnessStatus::Unknown);
        assert_eq!(
            catalog.eligibility("reqdrive", Tier::T2, false, None),
            Ok(())
        );
        catalog.refuse("reqdrive", "no oracle gate");
        assert_eq!(
            catalog.eligibility("reqdrive", Tier::T2, true, Some("claude")),
            Err("no oracle gate".to_string())
        );
        assert_eq!(catalog.asked().len(), 2);
        assert_eq!(
            catalog.asked()[1],
            ("reqdrive".into(), Tier::T2, true, Some("claude".into()))
        );
    }

    #[tokio::test]
    async fn the_desk_is_arranged_as_documented() {
        use fleet_forge::RepoForge;
        let desk = Desk::new();
        assert_eq!(
            desk.forge.resolve_base(&desk.repo).await.unwrap(),
            desk.base_sha
        );
        assert_eq!(
            desk.forge
                .read_file(&desk.repo, &desk.base_sha, REPO_CONFIG_PATH)
                .await
                .unwrap(),
            Some(REPO_CONFIG_TOML.as_bytes().to_vec())
        );
        let stored = desk
            .store
            .get_signature("owner/sandbox", &desk.spec_sha256)
            .unwrap();
        assert_eq!(stored.unwrap().bytes, desk.spec_bytes);
        assert_eq!(
            factory_spec::parse(&desk.spec_bytes).unwrap().work_item,
            desk.request("k").work_item
        );
        let parsed: RepoConfig = toml::from_str(REPO_CONFIG_TOML).unwrap();
        assert_eq!(parsed, repo_config());
    }
}
````

- [ ] **Step 3: Run it and see it fail**

```
cargo test -p fleet-admission --lib
```

Expected: the crate does not compile; the errors name `FakeAdmission`, `FakeCatalog`, `Desk`,
`admit_contract`, `request`, `repo_config` and `REPO_CONFIG_TOML`.

- [ ] **Step 4: Write the fakes, the desk and the suite**

Replace `crates/fleet-admission/src/testkit.rs` with:

````rust
//! Builders, a scripted harness catalog, a fake admission, an arranged world for the real one
//! ([`Desk`]), and the contract suite every [`Admit`] must pass.
//!
//! Compiled for this crate's own tests, and for other crates behind the `testkit` feature.

use crate::admission::{Admission, REPO_CONFIG_PATH};
use crate::types::{
    AdmissionConfig, Admit, Admitted, DispatchRequest, HarnessCatalog, HarnessStatus, Refusal,
};
use async_trait::async_trait;
use factory_spec::testkit::SpecBuilder;
use fleet_core::Tier;
use fleet_forge::testkit::MemForge;
use fleet_forge::{RepoRef, WorkItemRef};
use fleet_store::testkit::{harness_record, signature, MemStore};
use fleet_store::{SignatureStore, UnitRecord};
use harness_protocol::{
    Capabilities, Delivery, GateKind, Isolation, Metering, PresetInfo, RepoCommands, RepoConfig,
    UnitKind,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::Semaphore;

// ---------------------------------------------------------------------------------------------
// Builders
// ---------------------------------------------------------------------------------------------

/// The version of the preset rules the builders agree on.
pub const PRESETS_VERSION: &str = "0.1.0";

/// The `.reqdrive/config.toml` of the sandbox repository the builders use.
pub const REPO_CONFIG_TOML: &str = r#"preset = "node"
image = "docker.io/library/node@sha256:0000000000000000000000000000000000000000000000000000000000000000"
test_report = "reports/junit.xml"
test_dirs = ["test/"]
manifests = ["package.json"]
lockfiles = ["package-lock.json"]

[commands]
setup = "npm ci --ignore-scripts"
build = "npm run build"
test = "npm test"
format_check = "npm run format:check"
lint = "npm run lint"
"#;

/// [`REPO_CONFIG_TOML`] as the value it parses to.
pub fn repo_config() -> RepoConfig {
    RepoConfig {
        preset: "node".into(),
        image: format!("docker.io/library/node@sha256:{}", "0".repeat(64)),
        env: Default::default(),
        commands: RepoCommands {
            setup: "npm ci --ignore-scripts".into(),
            build: "npm run build".into(),
            test: "npm test".into(),
            format_check: "npm run format:check".into(),
            lint: "npm run lint".into(),
        },
        test_report: "reports/junit.xml".into(),
        test_dirs: vec!["test/".into()],
        manifests: vec!["package.json".into()],
        lockfiles: vec!["package-lock.json".into()],
    }
}

/// One allowed repository (`owner/sandbox`), a $5 unit ceiling, a 1800 second ceiling, a $20
/// ceiling over 24 hours, a review floor of 1, and both presets at [`PRESETS_VERSION`].
pub fn config() -> AdmissionConfig {
    AdmissionConfig {
        repos: vec![fleet_forge::testkit::repo()],
        unit_usd_ceiling: 5.0,
        unit_wall_clock_ceiling_secs: 1800,
        global_usd_cap: 20.0,
        spend_window_ms: 24 * 3600 * 1000,
        min_review_rounds: 1,
        presets: vec!["cargo".into(), "node".into()],
        presets_version: PRESETS_VERSION.into(),
    }
}

/// A request for `example/sandbox#1` in `owner/sandbox`, from the spec bytes with this hash, on
/// the `reqdrive` harness, with no opt-in and no lowered cap.
pub fn request(spec_sha256: &str, idempotency_key: &str) -> DispatchRequest {
    DispatchRequest {
        work_item: "example/sandbox#1".into(),
        fingerprint: None,
        repo: "owner/sandbox".into(),
        spec_sha256: spec_sha256.into(),
        profile: None,
        harness: "reqdrive".into(),
        opt_in: false,
        idempotency_key: idempotency_key.into(),
        usd_cap: None,
        wall_clock_secs: None,
    }
}

/// What a container harness that meters in USD and has the oracle gate declares, with both
/// presets at [`PRESETS_VERSION`].
pub fn capabilities() -> Capabilities {
    let preset = |name: &str| PresetInfo {
        name: name.into(),
        version: PRESETS_VERSION.into(),
    };
    Capabilities {
        isolation: Isolation::Container,
        metering: Metering::Usd,
        gates: vec![GateKind::Oracle],
        delivery: Delivery::Bundle,
        resume: true,
        halt: true,
        holdouts: false,
        controls: Vec::new(),
        network: None,
        profiles: Vec::new(),
        kinds: vec![UnitKind::Build],
        presets: vec![preset("cargo"), preset("node")],
    }
}

// ---------------------------------------------------------------------------------------------
// The scripted catalog
// ---------------------------------------------------------------------------------------------

/// One eligibility question: the harness, the tier, the opt-in and the profile.
pub type Asked = (String, Tier, bool, Option<String>);

/// A [`HarnessCatalog`] with fixed answers. A harness it has never heard of is `Unknown`; a
/// known one is eligible for everything until a test says otherwise.
#[derive(Default)]
pub struct FakeCatalog {
    statuses: Mutex<HashMap<String, HarnessStatus>>,
    ineligible: Mutex<HashMap<String, String>>,
    asked: Mutex<Vec<Asked>>,
}

impl FakeCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    /// A catalog with one healthy harness, `reqdrive`, declaring [`capabilities`].
    pub fn with_reqdrive() -> Self {
        let catalog = Self::new();
        catalog.set(
            "reqdrive",
            HarnessStatus::Healthy {
                capabilities: capabilities(),
            },
        );
        catalog
    }

    pub fn set(&self, name: &str, status: HarnessStatus) {
        self.statuses.lock().unwrap().insert(name.into(), status);
    }

    /// Make the named harness ineligible for every unit, for this reason.
    pub fn refuse(&self, name: &str, reason: &str) {
        self.ineligible
            .lock()
            .unwrap()
            .insert(name.into(), reason.into());
    }

    /// Every eligibility question asked so far: harness, tier, opt-in and profile.
    pub fn asked(&self) -> Vec<Asked> {
        self.asked.lock().unwrap().clone()
    }
}

impl HarnessCatalog for FakeCatalog {
    fn status(&self, name: &str) -> HarnessStatus {
        self.statuses
            .lock()
            .unwrap()
            .get(name)
            .cloned()
            .unwrap_or(HarnessStatus::Unknown)
    }

    fn eligibility(
        &self,
        name: &str,
        tier: Tier,
        opt_in: bool,
        profile: Option<&str>,
    ) -> Result<(), String> {
        self.asked.lock().unwrap().push((
            name.to_string(),
            tier,
            opt_in,
            profile.map(str::to_string),
        ));
        match self.ineligible.lock().unwrap().get(name) {
            Some(reason) => Err(reason.clone()),
            None => Ok(()),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The fake admission
// ---------------------------------------------------------------------------------------------

#[derive(Default)]
struct FakeState {
    keys: HashMap<String, String>,
    refusals: Vec<Refusal>,
    requests: Vec<DispatchRequest>,
    next: u64,
}

/// An [`Admit`] for testing a caller. It admits every well-formed request once per idempotency
/// key, as unit `u1`, `u2`, …, unless a refusal has been queued. It reads no store and no forge.
#[derive(Default)]
pub struct FakeAdmission {
    state: Mutex<FakeState>,
    /// The cap a request that asks for none is given.
    pub unit_usd_ceiling: f64,
}

impl FakeAdmission {
    pub fn new() -> Self {
        Self {
            state: Mutex::default(),
            unit_usd_ceiling: 5.0,
        }
    }

    /// Refuse the next request with `refusal`, whatever it is.
    pub fn refuse_next(&self, refusal: Refusal) {
        self.state.lock().unwrap().refusals.push(refusal);
    }

    /// Every request received so far, in order.
    pub fn requests(&self) -> Vec<DispatchRequest> {
        self.state.lock().unwrap().requests.clone()
    }
}

/// The record a request would be stored as, for unit `unit_id`.
pub fn admitted_record(unit_id: &str, request: &DispatchRequest, usd_cap: f64) -> UnitRecord {
    let number = WorkItemRef::parse(&request.work_item, None)
        .map(|w| w.number)
        .unwrap_or(0);
    let mut record = harness_record(unit_id, &request.harness);
    record.row.repo_slug = request.repo.clone();
    record.row.branch = format!("factory/{number}-{unit_id}");
    record.row.usd_cap = usd_cap;
    record.row.test_cmd = "npm test".into();
    record.row.min_review_rounds = 1;
    record.ext.work_item = Some(request.work_item.clone());
    record.ext.spec_id = Some("SPEC-0001".into());
    record.ext.spec_sha256 = Some(request.spec_sha256.clone());
    record.ext.base_sha = Some("b".repeat(40));
    record.ext.profile = request.profile.clone();
    record.ext.opt_in = request.opt_in;
    record.ext.idempotency_key = Some(request.idempotency_key.clone());
    record
}

#[async_trait]
impl Admit for FakeAdmission {
    async fn admit(&self, request: &DispatchRequest, _now_ms: i64) -> Result<Admitted, Refusal> {
        let mut state = self.state.lock().unwrap();
        state.requests.push(request.clone());
        if !state.refusals.is_empty() {
            return Err(state.refusals.remove(0));
        }
        if request.idempotency_key.is_empty() {
            return Err(Refusal::MissingKey);
        }
        if WorkItemRef::parse(&request.work_item, None).is_err() {
            return Err(Refusal::WorkItemMalformed {
                reference: request.work_item.clone(),
            });
        }
        if let Some(unit_id) = state.keys.get(&request.idempotency_key) {
            return Err(Refusal::DuplicateKey {
                unit_id: unit_id.clone(),
            });
        }
        if let Some(asked) = request.usd_cap {
            if asked > self.unit_usd_ceiling {
                return Err(Refusal::CapAboveCeiling {
                    cap: "usd",
                    requested: asked,
                    ceiling: self.unit_usd_ceiling,
                });
            }
        }
        state.next += 1;
        let unit_id = format!("u{}", state.next);
        state
            .keys
            .insert(request.idempotency_key.clone(), unit_id.clone());
        let usd_cap = request.usd_cap.unwrap_or(self.unit_usd_ceiling);
        Ok(Admitted {
            record: admitted_record(&unit_id, request, usd_cap),
            unit_id,
            signed_spec: SpecBuilder::new().build(),
            config: repo_config(),
            base_sha: "b".repeat(40),
            waits_for_slot: false,
        })
    }
}

// ---------------------------------------------------------------------------------------------
// An arranged world for the real admission
// ---------------------------------------------------------------------------------------------

/// Everything the real [`Admission`] reads, arranged so that [`Desk::request`] is admitted: an
/// onboarded `owner/sandbox` on the in-memory forge, a stored signature for `SpecBuilder::new()`,
/// a healthy `reqdrive` harness, three free slots and [`config`]. A test changes one thing and
/// asserts the refusal.
pub struct Desk {
    pub store: Arc<MemStore>,
    pub forge: Arc<MemForge>,
    pub catalog: Arc<FakeCatalog>,
    pub slots: Arc<Semaphore>,
    pub config: AdmissionConfig,
    pub repo: RepoRef,
    pub base_sha: String,
    pub spec_bytes: Vec<u8>,
    pub spec_sha256: String,
}

impl Default for Desk {
    fn default() -> Self {
        Self::new()
    }
}

impl Desk {
    pub fn new() -> Self {
        let repo = fleet_forge::testkit::repo();
        let forge = Arc::new(MemForge::new());
        let base_sha = forge.commit_base(
            &repo,
            &[
                (REPO_CONFIG_PATH, Some(REPO_CONFIG_TOML)),
                ("src/cart.js", Some("export const total = () => 100;\n")),
            ],
        );
        let spec_bytes = SpecBuilder::new().build();
        let signed = signature(&repo.slug, "SPEC-0001", &spec_bytes);
        let store = Arc::new(MemStore::new());
        store.put_signature(&signed).unwrap();
        Desk {
            store,
            forge,
            catalog: Arc::new(FakeCatalog::with_reqdrive()),
            slots: Arc::new(Semaphore::new(3)),
            config: config(),
            repo,
            base_sha,
            spec_sha256: signed.sha256,
            spec_bytes,
        }
    }

    /// A request this desk admits, with the given idempotency key.
    pub fn request(&self, idempotency_key: &str) -> DispatchRequest {
        request(&self.spec_sha256, idempotency_key)
    }

    /// The real admission over this desk. Call it after changing `config`.
    pub fn admission(&self) -> Admission {
        Admission::new(
            self.config.clone(),
            self.store.clone(),
            self.store.clone(),
            self.forge.clone(),
            self.catalog.clone(),
            self.slots.clone(),
        )
        .expect("the in-memory store reads")
    }
}

// ---------------------------------------------------------------------------------------------
// The contract suite
// ---------------------------------------------------------------------------------------------

/// What every [`Admit`] must do. `make` returns a new admitter, and a request that admitter
/// admits. Panics on the first rule an implementation breaks.
pub async fn admit_contract<A: Admit>(make: impl Fn() -> (A, DispatchRequest)) {
    // The request is admitted, and what comes back is what was asked for.
    let (admission, good) = make();
    let first = admission
        .admit(&good, 1_000)
        .await
        .expect("the good request is admitted");
    assert!(!first.unit_id.is_empty());
    assert_eq!(first.record.row.unit_id, first.unit_id);
    assert_eq!(first.record.row.phase, "queued");
    assert_eq!(first.record.row.repo_slug, good.repo);
    assert_eq!(
        first.record.ext.harness.as_deref(),
        Some(good.harness.as_str())
    );
    assert_eq!(
        first.record.ext.work_item.as_deref(),
        Some(good.work_item.as_str())
    );
    assert_eq!(
        first.record.ext.spec_sha256.as_deref(),
        Some(good.spec_sha256.as_str())
    );
    assert_eq!(
        first.record.ext.idempotency_key.as_deref(),
        Some(good.idempotency_key.as_str())
    );
    assert_eq!(
        first.record.ext.base_sha.as_deref(),
        Some(first.base_sha.as_str())
    );
    assert!(first.record.row.usd_cap > 0.0);
    // The branch carries the work item's number and the unit's id.
    let number = WorkItemRef::parse(&good.work_item, None).unwrap().number;
    assert!(
        first.record.row.branch.contains(&number.to_string())
            && first.record.row.branch.contains(&first.unit_id),
        "branch {:?}",
        first.record.row.branch
    );

    // The same key again starts nothing and names the unit it already started.
    assert_eq!(
        admission.admit(&good, 1_001).await,
        Err(Refusal::DuplicateKey {
            unit_id: first.unit_id.clone()
        })
    );

    // A second key is a second unit, and a lower cap is honoured exactly.
    let mut lower = good.clone();
    lower.idempotency_key = format!("{}-lower", good.idempotency_key);
    lower.usd_cap = Some(1.0);
    let second = admission
        .admit(&lower, 1_002)
        .await
        .expect("a lower cap is admitted");
    assert_ne!(second.unit_id, first.unit_id);
    assert_eq!(second.record.row.usd_cap, 1.0);

    // A cap above the ceiling is refused, and says which cap.
    let mut higher = good.clone();
    higher.idempotency_key = format!("{}-higher", good.idempotency_key);
    higher.usd_cap = Some(first.record.row.usd_cap * 1000.0);
    match admission.admit(&higher, 1_003).await {
        Err(Refusal::CapAboveCeiling { cap, .. }) => assert_eq!(cap, "usd"),
        other => panic!("a cap above the ceiling was not refused as such: {other:?}"),
    }

    // A request with no key, or with a work item that is not owner/repo#N, is refused.
    let mut keyless = good.clone();
    keyless.idempotency_key = String::new();
    assert_eq!(
        admission.admit(&keyless, 1_004).await,
        Err(Refusal::MissingKey)
    );
    let mut malformed = good.clone();
    malformed.idempotency_key = format!("{}-malformed", good.idempotency_key);
    malformed.work_item = "not a work item".into();
    let refusal = admission.admit(&malformed, 1_005).await.unwrap_err();
    assert_eq!(refusal.code(), "work_item_malformed");
    assert!(!refusal.to_string().is_empty());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn the_fake_admission_passes_the_admit_contract() {
        admit_contract(|| (FakeAdmission::new(), request(&"ab".repeat(32), "key-1"))).await;
    }

    #[tokio::test]
    async fn a_queued_refusal_is_given_once_and_requests_are_recorded() {
        let admission = FakeAdmission::new();
        admission.refuse_next(Refusal::NotSigned {
            repo: "owner/sandbox".into(),
            spec_sha256: "ab".repeat(32),
        });
        let asked = request(&"ab".repeat(32), "key-1");
        assert_eq!(
            admission.admit(&asked, 0).await.unwrap_err().code(),
            "not_signed"
        );
        assert!(admission.admit(&asked, 0).await.is_ok());
        assert_eq!(admission.requests().len(), 2);
    }

    #[test]
    fn the_scripted_catalog_answers_what_it_was_told() {
        let catalog = FakeCatalog::with_reqdrive();
        assert!(matches!(
            catalog.status("reqdrive"),
            HarnessStatus::Healthy { .. }
        ));
        assert_eq!(catalog.status("nobody"), HarnessStatus::Unknown);
        assert_eq!(
            catalog.eligibility("reqdrive", Tier::T2, false, None),
            Ok(())
        );
        catalog.refuse("reqdrive", "no oracle gate");
        assert_eq!(
            catalog.eligibility("reqdrive", Tier::T2, true, Some("claude")),
            Err("no oracle gate".to_string())
        );
        assert_eq!(catalog.asked().len(), 2);
        assert_eq!(
            catalog.asked()[1],
            ("reqdrive".into(), Tier::T2, true, Some("claude".into()))
        );
    }

    #[tokio::test]
    async fn the_desk_is_arranged_as_documented() {
        use fleet_forge::RepoForge;
        let desk = Desk::new();
        assert_eq!(
            desk.forge.resolve_base(&desk.repo).await.unwrap(),
            desk.base_sha
        );
        assert_eq!(
            desk.forge
                .read_file(&desk.repo, &desk.base_sha, REPO_CONFIG_PATH)
                .await
                .unwrap(),
            Some(REPO_CONFIG_TOML.as_bytes().to_vec())
        );
        let stored = desk
            .store
            .get_signature("owner/sandbox", &desk.spec_sha256)
            .unwrap();
        assert_eq!(stored.unwrap().bytes, desk.spec_bytes);
        assert_eq!(
            factory_spec::parse(&desk.spec_bytes).unwrap().work_item,
            desk.request("k").work_item
        );
        let parsed: RepoConfig = toml::from_str(REPO_CONFIG_TOML).unwrap();
        assert_eq!(parsed, repo_config());
    }
}
````

- [ ] **Step 5: Run the tests**

```
cargo test -p fleet-admission --lib
cargo test -p fleet-admission --lib --features testkit
```

Expected, both times: `test result: ok. 4 passed; 0 failed`.

- [ ] **Step 6: Commit**

```
git add crates/fleet-admission/src
git commit -m "feat(fleet-admission): the admission seam, its refusals, a fake admission and its contract suite"
```

### Task 7: `fleet-supervisor` — a unit's run, its connector and its test rig

**Files:**
- Replace: `crates/fleet-supervisor/src/lib.rs`
- Create: `crates/fleet-supervisor/src/types.rs`, `connect.rs`, `eligibility.rs`,
  `registry.rs`, `run.rs`, `testkit.rs`

**Produces:** `run`; `UnitSpec`, `FactoryInputs`, `RunCtx`, `Pause`, `Deps`; the `Sink` port;
the `Connector` seam with `Connection`, `ProcessControl`, `Exit`, `ConnectError`,
`ProcessConnector`; `eligibility`; `Registry`, `HarnessEntry`, `ProbeState`, `RegistryError`,
`fake_binary_beside`, `FAKE_HARNESS`; the constants `DEFAULT_GRACE`, `DEFAULT_STALL`,
`MAX_MERGEABLE_POLLS` and `REASON_*`. In `testkit`: `Rig`, `RigBuilder`, `Peer`,
`DuplexConnector`, `Recorder`, `Signal`, `unit_spec`, `run_ctx`, `default_freeze`,
`peer_capabilities`, the command builders `halt`, `resume`, `abandon`, `ship`, `approve`,
`reject`, `reverify`, and the constants `HEAD`, `GATE_HASH`, `WAIT_LIMIT`.

What to know:

- `run` takes a `Connector`, not a process: it calls `connect()` once per spawn and gets two
  byte streams and a handle. Tests hand it `DuplexConnector`, whose every connection is an
  in-memory pipe with a `Peer` on the other end that the test drives line by line.
- `run` reports through a `Sink` and writes to no store. `fleetd` implements `Sink` over the
  API's feed. Besides events, the sink is told two things worth keeping: the oracle freeze and
  the harness's result, so that a unit brought back after a restart can still be verified.
- There is no contract suite for `Connector`: its one real implementation starts a process,
  and its contract is the process tests of lane CC-SUPERVISOR. The fake has loopback tests here.
- The `Rig` races every wait against the run's own task, and re-raises the run's panic. So in
  wave 0 every test that starts a rig fails with `not implemented: lane CC-SUPERVISOR builds
  this`, which the last test of this task pins.

- [ ] **Step 1: Create the source files other than `testkit.rs`**

`crates/fleet-supervisor/src/lib.rs`:

````rust
//! `fleet-supervisor` — runs one unit by supervising one harness process at a time. The harness
//! runs the agents; this crate keeps everything that decides: the state machine, the caps, the
//! oracle gate, halt and resume, and what a result is allowed to mean.
//!
//! [`run`] is the whole of a unit's life, over any byte stream a [`Connector`] hands it. A
//! [`Registry`] names the harness programs, and [`ProcessConnector`] starts one as a child
//! process. The behaviour is the supervisor specification in
//! `docs/superpowers/specs/2026-09-16-sp2a-harness-supervisor-design.md`, sections 3.3 and 4,
//! at protocol 0.2.

mod connect;
mod eligibility;
mod registry;
mod run;
mod types;

#[cfg(any(test, feature = "testkit"))]
pub mod testkit;

pub use connect::{ConnectError, Connection, Connector, Exit, ProcessConnector, ProcessControl};
pub use eligibility::eligibility;
pub use registry::{
    fake_binary_beside, HarnessEntry, ProbeState, Registry, RegistryError, FAKE_HARNESS,
};
pub use run::run;
pub use types::{
    Deps, FactoryInputs, Pause, RunCtx, Sink, UnitSpec, DEFAULT_GRACE, DEFAULT_STALL,
    MAX_MERGEABLE_POLLS, REASON_AWAITING_SHIP, REASON_AWAITING_SLOT, REASON_ORACLE_TAMPERING,
    REASON_VERIFY_STOPPED,
};
````

`crates/fleet-supervisor/src/types.rs`:

````rust
//! What a caller gives [`crate::run`], and what it gets told.

use fleet_core::{Event, Phase, Tier};
use fleet_forge::RepoForge;
use fleet_verify::EvidenceVerifier;
use harness_protocol::{
    OracleFreeze, Repo, RepoConfig, Scope, Stage, StageStatus, UnitResult, WorkItem,
};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;

/// The reason on the phase change that pauses a unit for oracle tampering. The store's fold
/// reads the same text; `fleetd` has a test that compares the two constants.
pub const REASON_ORACLE_TAMPERING: &str = "oracle tampering";
/// The reason of the `Blocked` event, and of the phase change, for a verified T3 unit that
/// waits for a person to ship it.
pub const REASON_AWAITING_SHIP: &str = "awaiting ship";
/// The reason on the phase change that pauses a unit whose verification could not run, or
/// whose tests failed in the control plane's own run. `Reverify` is accepted only for it.
pub const REASON_VERIFY_STOPPED: &str = "verification stopped";
/// The reason of the `Blocked` event for a unit waiting for a concurrency slot.
pub const REASON_AWAITING_SLOT: &str = "awaiting concurrency slot";

/// How long a harness is given to answer `initialize` and `unit/start`, and to exit after it
/// was asked to stop or sent its result.
pub const DEFAULT_GRACE: Duration = Duration::from_secs(30);
/// How long a live harness may send no `unit/event` before the unit is stalled.
pub const DEFAULT_STALL: Duration = Duration::from_secs(600);
/// How many times an open pull request's mergeability is polled before a person is asked.
pub const MAX_MERGEABLE_POLLS: u32 = 10;

/// What is fixed about a unit for its whole life.
#[derive(Debug, Clone, PartialEq)]
pub struct UnitSpec {
    pub unit_id: String,
    pub tier: Tier,
    /// A one-line summary.
    pub task: String,
    pub repo: Repo,
    pub branch: String,
    pub test_cmd: String,
    /// The most the unit may spend over all its spawns, in USD.
    pub usd_cap: f64,
    /// The longest its agents may run over all its spawns, in seconds; 0 disables the limit.
    pub wall_clock_secs: u64,
    pub min_review_rounds: u32,
    pub work_item: WorkItem,
    /// What a unit run from a signed spec carries. `None` for a demo unit.
    pub factory: Option<FactoryInputs>,
}

/// The inputs of a unit that runs from a signed spec.
#[derive(Debug, Clone, PartialEq)]
pub struct FactoryInputs {
    pub spec_id: String,
    /// The signed bytes, from the control plane's store.
    pub signed_spec: Vec<u8>,
    /// The commit the source bundle is cut at.
    pub base_sha: String,
    pub scope: Scope,
    pub permitted_dependencies: Vec<String>,
    pub expected_red: Vec<String>,
    pub config: RepoConfig,
    pub profile: Option<String>,
}

/// Why a unit that starts in `NeedsHuman` or `Halted` is paused, from the store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pause {
    /// The reason of the phase change that paused it.
    pub reason: Option<String>,
    /// The phase it was in.
    pub from: Phase,
}

/// What a unit has already been through, and the limits of this run.
#[derive(Debug, Clone)]
pub struct RunCtx {
    /// `Queued` for a new unit; `Halted` or `NeedsHuman` for one brought back from the store,
    /// which then waits for a command.
    pub start_phase: Phase,
    /// Spent by earlier spawns, in USD.
    pub cost_usd: f64,
    /// Agent time used by earlier spawns, in milliseconds.
    pub elapsed_ms: u64,
    /// The stored `oracle_frozen` column.
    pub oracle_frozen: bool,
    /// The oracle hash a person approved, if any.
    pub approved_oracle_hash: Option<String>,
    /// The freeze an earlier spawn reported, if any.
    pub freeze: Option<OracleFreeze>,
    /// The result an earlier spawn sent, if any, so that verification can be run again.
    pub last_result: Option<UnitResult>,
    /// Set when `start_phase` is `NeedsHuman` or `Halted`.
    pub pause: Option<Pause>,
    /// The person opted in to a weaker guarantee for this unit.
    pub opt_in: bool,
    pub grace: Duration,
    pub stall: Duration,
    /// The concurrency slots every unit shares.
    pub slots: Arc<Semaphore>,
    /// A directory this unit owns: the signed spec bytes and the source bundle are written here.
    pub work_dir: PathBuf,
}

/// The seams a unit's run calls out through.
#[derive(Clone)]
pub struct Deps {
    pub verifier: Arc<dyn EvidenceVerifier>,
    pub forge: Arc<dyn RepoForge>,
}

/// Where a unit's run reports. The supervisor writes nothing to a store itself; whoever
/// implements this decides what is kept.
pub trait Sink: Send {
    /// One event of the view contract, in order.
    fn event(&mut self, event: Event);
    /// A stage note from the harness. It changes no phase.
    fn stage(&mut self, stage: Stage, status: StageStatus, detail: Option<String>);
    /// The oracle freeze the harness reported, to be kept for later verifications.
    fn frozen(&mut self, freeze: &OracleFreeze);
    /// The result the harness sent, to be kept so verification can be run again.
    fn result(&mut self, result: &UnitResult);
}
````

`crates/fleet-supervisor/src/connect.rs`:

````rust
//! How a unit's run reaches a harness: a pair of byte streams and a handle on whatever is behind
//! them. [`ProcessConnector`] puts a child process there; a test puts an in-memory pipe.

use crate::registry::HarnessEntry;
use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::mpsc::UnboundedReceiver;

/// A harness could not be started.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct ConnectError(pub String);

/// How a harness process ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Exit {
    /// The exit code, when the process exited by itself.
    pub code: Option<i32>,
    /// A line for a person: `exit code: 3`, `killed`.
    pub description: String,
}

/// The process behind a [`Connection`].
#[async_trait]
pub trait ProcessControl: Send {
    /// Resolves when the process has ended, however it ended. May be called again after it
    /// resolved; it then resolves at once with the same answer.
    async fn wait(&mut self) -> Exit;
    /// End the process and everything it started. Returns once it is gone.
    async fn kill(&mut self);
}

/// One live harness.
pub struct Connection {
    /// The harness's standard output: one protocol message per line.
    pub reader: Box<dyn AsyncRead + Send + Unpin>,
    /// The harness's standard input. Shutting it down, or dropping it, tells the harness to stop.
    pub writer: Box<dyn AsyncWrite + Send + Unpin>,
    /// The harness's standard error, line by line. Not protocol; forwarded as system log lines.
    pub diagnostics: UnboundedReceiver<String>,
    pub control: Box<dyn ProcessControl>,
}

/// Starts a harness. A unit's run calls it once per spawn: at the start, and again on resume.
#[async_trait]
pub trait Connector: Send + Sync {
    /// The registry name of the harness, for error messages.
    fn name(&self) -> &str;
    async fn connect(&self) -> Result<Connection, ConnectError>;
}

/// [`Connector`] that starts a registry entry as a child process: standard input and output
/// piped, standard error read line by line, and the whole process tree ended on `kill` or when
/// the connection is dropped.
#[derive(Debug, Clone)]
pub struct ProcessConnector {
    entry: HarnessEntry,
}

impl ProcessConnector {
    pub fn new(entry: HarnessEntry) -> Self {
        Self { entry }
    }
}

#[async_trait]
impl Connector for ProcessConnector {
    fn name(&self) -> &str {
        &self.entry.name
    }

    async fn connect(&self) -> Result<Connection, ConnectError> {
        unimplemented!("lane CC-SUPERVISOR builds this")
    }
}
````

`crates/fleet-supervisor/src/eligibility.rs`:

````rust
//! Whether a harness may run a unit, decided from what the harness declares.

use fleet_core::Tier;
use harness_protocol::Capabilities;

/// `Ok` when a harness with these capabilities may run a unit of this tier. `Err` is the reason,
/// naming the guarantee that is missing. The rules, first match wins:
///
/// 1. a harness whose `kinds` is not empty and lacks `build` is refused;
/// 2. T2 and T3 need the `oracle` gate, and no opt-in changes that;
/// 3. `delivery: push` is refused, and no opt-in changes that;
/// 4. `isolation` other than `container`, or `metering: none`, needs the opt-in;
/// 5. a named profile must be one the harness declares, and an unpriced one needs the opt-in.
///
/// Admission asks this of the capabilities a probe returned; a unit's run asks it again of the
/// capabilities its own harness process returned, which are the ones that count.
pub fn eligibility(
    tier: Tier,
    capabilities: &Capabilities,
    opt_in: bool,
    profile: Option<&str>,
) -> Result<(), String> {
    let _ = (tier, capabilities, opt_in, profile);
    unimplemented!("lane CC-SUPERVISOR builds this")
}
````

`crates/fleet-supervisor/src/registry.rs`:

````rust
//! The harness registry: which programs are harnesses, and what each said when it was probed.

use crate::connect::Connector;
use harness_protocol::{Capabilities, HarnessInfo};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

/// The registry name of the scripted harness that demo mode runs.
pub const FAKE_HARNESS: &str = "fake";

/// One registered harness: a name, the command line that starts it, and extra environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessEntry {
    pub name: String,
    /// The program and its arguments. Never passed through a shell.
    pub command: Vec<String>,
    pub env: Vec<(String, String)>,
}

/// What a probe found.
#[derive(Debug, Clone, PartialEq)]
pub enum ProbeState {
    Unprobed,
    Healthy {
        harness: HarnessInfo,
        capabilities: Capabilities,
    },
    Unhealthy {
        error: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RegistryError {
    #[error("the registry file could not be read: {0}")]
    Unreadable(String),
    #[error("the registry file does not read: {0}")]
    Malformed(String),
    #[error("harness {name:?} is registered twice")]
    Duplicate { name: String },
    #[error("harness {name:?} has an empty command")]
    EmptyCommand { name: String },
}

/// Where `harness-fake` is expected, given the path of the running executable: in the same
/// directory, or in its parent when that directory is named `deps`. On Windows the name ends in
/// `.exe`.
pub fn fake_binary_beside(exe: &Path) -> PathBuf {
    let _ = exe;
    unimplemented!("lane CC-SUPERVISOR builds this")
}

/// The registered harnesses and their probe states. Cloning shares the states.
#[derive(Clone)]
pub struct Registry {
    #[allow(dead_code)] // read by the bodies lane CC-SUPERVISOR builds
    inner: Arc<std::sync::Mutex<Vec<(HarnessEntry, ProbeState)>>>,
}

impl Registry {
    /// A registry of these entries, none probed. A later entry with an earlier one's name
    /// replaces it.
    pub fn new(entries: Vec<HarnessEntry>) -> Self {
        let _ = entries;
        unimplemented!("lane CC-SUPERVISOR builds this")
    }

    /// One entry, [`FAKE_HARNESS`], at [`fake_binary_beside`] the running executable.
    pub fn default_fake() -> Self {
        unimplemented!("lane CC-SUPERVISOR builds this")
    }

    /// Read a registry file:
    ///
    /// ```toml
    /// [[harness]]
    /// name = "reqdrive"
    /// command = ["C:/tools/reqdrive.exe", "harness"]
    ///
    /// [harness.env]
    /// REQDRIVE_HOME = "C:/Users/me/.reqdrive"
    /// ```
    ///
    /// `env` is optional. An unknown key, a repeated name or an empty command is an error.
    pub fn from_toml(text: &str) -> Result<Self, RegistryError> {
        let _ = text;
        unimplemented!("lane CC-SUPERVISOR builds this")
    }

    /// [`Registry::from_toml`] of the file at `path`.
    pub fn load(path: &Path) -> Result<Self, RegistryError> {
        let _ = path;
        unimplemented!("lane CC-SUPERVISOR builds this")
    }

    /// Add `other`'s entries to this registry; an entry of `other` wins a shared name.
    pub fn merged(self, other: Registry) -> Self {
        let _ = other;
        unimplemented!("lane CC-SUPERVISOR builds this")
    }

    pub fn entry(&self, name: &str) -> Option<HarnessEntry> {
        let _ = name;
        unimplemented!("lane CC-SUPERVISOR builds this")
    }

    /// Every entry's name and probe state, in registration order.
    pub fn states(&self) -> Vec<(String, ProbeState)> {
        unimplemented!("lane CC-SUPERVISOR builds this")
    }

    pub fn state(&self, name: &str) -> Option<ProbeState> {
        let _ = name;
        unimplemented!("lane CC-SUPERVISOR builds this")
    }

    /// Start each entry, send `initialize`, record what came back, and close its input. A
    /// program that cannot be started, an answer that does not come within `grace`, a refused
    /// version or a reply that is not an `initialize` result is `Unhealthy` with the reason.
    pub async fn probe(&self, grace: Duration) {
        let _ = grace;
        unimplemented!("lane CC-SUPERVISOR builds this")
    }

    /// A connector that starts the named entry as a child process.
    pub fn connector(&self, name: &str) -> Option<Arc<dyn Connector>> {
        let _ = name;
        unimplemented!("lane CC-SUPERVISOR builds this")
    }
}
````

`crates/fleet-supervisor/src/run.rs`:

````rust
//! A unit's run. Wave 0 gives the signature; lane CC-SUPERVISOR builds the loop.

use crate::connect::Connector;
use crate::types::{Deps, RunCtx, Sink, UnitSpec};
use fleet_core::{Command, Phase};
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedReceiver;

/// Drive one unit until it is finished, and return the phase it finished in.
///
/// Everything happens in one loop: a line from the harness, a command, a timer, or the one
/// call in flight after a result (the exit wait, verification, opening the pull request, the
/// mergeability poll). The phase changes only inside that loop, one event at a time.
///
/// A closed `commands` channel means no more commands will come: a unit with a live harness or
/// a call in flight keeps running; a paused unit with nothing in flight fails closed.
pub async fn run(
    connector: Arc<dyn Connector>,
    deps: Deps,
    spec: UnitSpec,
    ctx: RunCtx,
    commands: UnboundedReceiver<Command>,
    sink: Box<dyn Sink>,
) -> Phase {
    let _ = (connector, deps, spec, ctx, commands, sink);
    unimplemented!("lane CC-SUPERVISOR builds this")
}
````

- [ ] **Step 2: Write the failing test**

Create `crates/fleet-supervisor/src/testkit.rs` with only this:

````rust
#[cfg(test)]
mod tests {
    use super::*;
    use harness_protocol::Outcome;

    #[tokio::test(start_paused = true)]
    async fn a_peer_and_a_connection_speak_the_protocol_to_each_other() {
        let (connector, mut peers) = DuplexConnector::new();
        let mut connection = connector.connect().await.unwrap();
        let mut peer = peers.recv().await.unwrap();
        let mut lines = BufReader::new(&mut connection.reader).lines();

        // The control-plane side writes initialize; the peer answers it.
        let hello = RpcMessage::request(
            1,
            method::INITIALIZE,
            &InitializeParams {
                protocol_version: PROTOCOL_VERSION.into(),
                accepted_versions: vec![PROTOCOL_VERSION.into()],
            },
        );
        let line = serde_json::to_string(&hello).unwrap() + "\n";
        connection.writer.write_all(line.as_bytes()).await.unwrap();
        let params = peer.initialize(peer_capabilities()).await;
        assert_eq!(params.accepted(), [PROTOCOL_VERSION]);
        let answer: RpcMessage =
            serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
        let result: InitializeResult = answer.result_as().unwrap();
        assert_eq!(result.capabilities, peer_capabilities());

        // The peer's events and result arrive as lines, in order.
        peer.provisioned_and_frozen().await;
        peer.metric(0.25).await;
        peer.deliver(&fleet_verify::testkit::bare_order("u1")).await;
        let mut methods = Vec::new();
        for _ in 0..4 {
            let message: RpcMessage =
                serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
            methods.push(message.method.clone().unwrap());
            if message.method.as_deref() == Some(method::UNIT_RESULT) {
                let result: UnitResult = message.params_as().unwrap();
                assert_eq!(result.outcome, Outcome::PrOpen);
                assert_eq!(result.evidence.unwrap().head_sha, HEAD);
            }
        }
        assert_eq!(
            methods,
            ["unit/event", "unit/event", "unit/event", "unit/result"]
        );

        // A diagnostic line arrives on its own channel.
        peer.diagnostic("scripted harness starting");
        assert_eq!(
            connection.diagnostics.recv().await.unwrap(),
            "scripted harness starting"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn closing_the_input_is_seen_by_the_peer_and_exit_is_seen_by_the_control() {
        let (connector, mut peers) = DuplexConnector::new();
        let mut connection = connector.connect().await.unwrap();
        let mut peer = peers.recv().await.unwrap();
        connection.writer.shutdown().await.unwrap();
        assert!(peer.recv().await.is_none());

        peer.exit(3);
        let exit = connection.control.wait().await;
        assert_eq!(
            (exit.code, exit.description.as_str()),
            (Some(3), "exit code: 3")
        );
        assert_eq!(
            connection.control.wait().await,
            exit,
            "wait can be asked again"
        );
        assert!(!peer.was_killed());
        // The peer's output is closed too.
        let mut rest = String::new();
        let read = BufReader::new(&mut connection.reader)
            .read_line(&mut rest)
            .await
            .unwrap();
        assert_eq!(read, 0);
    }

    #[tokio::test(start_paused = true)]
    async fn a_killed_peer_reports_killed_and_a_scripted_connect_failure_is_an_error() {
        let (connector, mut peers) = DuplexConnector::new();
        connector.fail_next("program not found");
        assert_eq!(
            connector.connect().await.err(),
            Some(ConnectError("program not found".into()))
        );
        let mut connection = connector.connect().await.unwrap();
        let peer = peers.recv().await.unwrap();
        connection.control.kill().await;
        assert!(peer.was_killed());
        assert_eq!(connection.control.wait().await.description, "killed");
        assert_eq!(connector.connects(), 2);
    }

    #[test]
    fn the_recorder_sorts_what_it_was_told() {
        let recorder = Recorder::new();
        let mut sink = recorder.sink();
        sink.event(Event::PhaseChanged {
            from: Phase::Queued,
            to: Phase::Provisioning,
            reason: None,
            cmd_id: None,
        });
        sink.stage(Stage::Red, StageStatus::Started, None);
        sink.event(Event::Blocked {
            reason: "usd cap".into(),
            cap: Some("usd".into()),
            detail: "spent $6.00 of $5.00".into(),
        });
        sink.event(Event::PhaseChanged {
            from: Phase::Provisioning,
            to: Phase::NeedsHuman,
            reason: Some("usd cap".into()),
            cmd_id: None,
        });
        sink.frozen(&default_freeze());
        assert_eq!(recorder.phases(), [Phase::Provisioning, Phase::NeedsHuman]);
        assert_eq!(recorder.last_reason().as_deref(), Some("usd cap"));
        assert_eq!(recorder.stages(), [(Stage::Red, StageStatus::Started)]);
        assert_eq!(recorder.blocked()[0].1.as_deref(), Some("usd"));
        assert_eq!(recorder.signals().len(), 5);
        assert_eq!(recorder.done(), None);
    }

    #[tokio::test(start_paused = true)]
    #[should_panic(expected = "lane CC-SUPERVISOR builds this")]
    async fn the_rig_raises_the_panic_of_the_run_it_started() {
        let mut rig = Rig::builder().start();
        let _ = rig.peer().await;
    }
}
````

- [ ] **Step 3: Run it and see it fail**

```
cargo test -p fleet-supervisor --lib
```

Expected: the crate does not compile; the errors name `DuplexConnector`, `Recorder`, `Rig`,
`peer_capabilities`, `default_freeze` and `HEAD`.

- [ ] **Step 4: Write the peer, the connector, the recorder and the rig**

Replace `crates/fleet-supervisor/src/testkit.rs` with:

````rust
//! A scripted harness peer over in-memory pipes, a recording sink, and a rig that runs one unit
//! against them.
//!
//! Compiled for this crate's own tests, and for other crates behind the `testkit` feature. The
//! rig's waits are measured on tokio's clock, so a test that uses it starts with the clock
//! paused: `#[tokio::test(start_paused = true)]`. Time then moves only when every task is
//! waiting, which makes a stall or a cap breach happen at an exact, repeatable moment.

use crate::connect::{ConnectError, Connection, Connector, Exit, ProcessControl};
use crate::run::run;
use crate::types::{
    Deps, FactoryInputs, Pause, RunCtx, Sink, UnitSpec, DEFAULT_GRACE, DEFAULT_STALL,
};
use async_trait::async_trait;
use fleet_core::{Command, ErrorScope, Event, Phase, Tier};
use fleet_forge::testkit::MemForge;
use fleet_verify::testkit::{
    capabilities, evidence, freeze, pr_open, ScriptedVerifier, World, FROZEN_ID, FROZEN_TEST_PATH,
    FROZEN_TEST_SOURCE,
};
use fleet_verify::Verdict;
use harness_protocol::{
    method, Capabilities, Empty, GateReply, GateRequest, HarnessInfo, InitializeParams,
    InitializeResult, MessageKind, Observation, OracleFreeze, Repo, RpcMessage, Stage, StageStatus,
    UnitEvent, UnitResult, WorkItem, WorkItemKind, WorkOrder, PROTOCOL_VERSION,
};
use std::collections::VecDeque;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, DuplexStream};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::sync::{watch, Notify, Semaphore};
use tokio::task::JoinHandle;

/// The commit a scripted peer says it delivered.
pub const HEAD: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

/// The hash a scripted peer proposes at the oracle gate.
pub const GATE_HASH: &str = "gate-hash-1";

/// How long, on tokio's clock, a rig or a peer waits for something before it gives up and
/// fails the test. Longer than any cap a test sets.
pub const WAIT_LIMIT: Duration = Duration::from_secs(6 * 3600);

// ---------------------------------------------------------------------------------------------
// The recording sink
// ---------------------------------------------------------------------------------------------

/// One call a unit's run made on its [`Sink`].
#[allow(clippy::large_enum_variant)] // a test records a handful of these
#[derive(Debug, Clone, PartialEq)]
pub enum Signal {
    Event(Event),
    Stage {
        stage: Stage,
        status: StageStatus,
        detail: Option<String>,
    },
    Frozen(OracleFreeze),
    Result(UnitResult),
}

#[derive(Default)]
struct Recorded {
    signals: Mutex<Vec<Signal>>,
    changed: Notify,
}

/// Records every [`Signal`], in order. Cloning shares the record.
#[derive(Clone, Default)]
pub struct Recorder {
    inner: Arc<Recorded>,
}

struct RecordingSink(Recorder);

impl RecordingSink {
    fn push(&mut self, signal: Signal) {
        self.0.inner.signals.lock().unwrap().push(signal);
        self.0.inner.changed.notify_waiters();
    }
}

impl Sink for RecordingSink {
    fn event(&mut self, event: Event) {
        self.push(Signal::Event(event));
    }

    fn stage(&mut self, stage: Stage, status: StageStatus, detail: Option<String>) {
        self.push(Signal::Stage {
            stage,
            status,
            detail,
        });
    }

    fn frozen(&mut self, freeze: &OracleFreeze) {
        self.push(Signal::Frozen(freeze.clone()));
    }

    fn result(&mut self, result: &UnitResult) {
        self.push(Signal::Result(result.clone()));
    }
}

impl Recorder {
    pub fn new() -> Self {
        Self::default()
    }

    /// A sink that records into this recorder.
    pub fn sink(&self) -> Box<dyn Sink> {
        Box::new(RecordingSink(self.clone()))
    }

    pub fn signals(&self) -> Vec<Signal> {
        self.inner.signals.lock().unwrap().clone()
    }

    /// The view-contract events, in order.
    pub fn events(&self) -> Vec<Event> {
        self.signals()
            .into_iter()
            .filter_map(|s| match s {
                Signal::Event(e) => Some(e),
                _ => None,
            })
            .collect()
    }

    /// The phase after each phase change, in order.
    pub fn phases(&self) -> Vec<Phase> {
        self.events()
            .into_iter()
            .filter_map(|e| match e {
                Event::PhaseChanged { to, .. } => Some(to),
                _ => None,
            })
            .collect()
    }

    /// The reason of the latest phase change, if it had one.
    pub fn last_reason(&self) -> Option<String> {
        self.events().into_iter().rev().find_map(|e| match e {
            Event::PhaseChanged { reason, .. } => Some(reason),
            _ => None,
        })?
    }

    /// Every error event, as its scope and detail.
    pub fn errors(&self) -> Vec<(ErrorScope, String)> {
        self.events()
            .into_iter()
            .filter_map(|e| match e {
                Event::Error { scope, detail, .. } => Some((scope, detail)),
                _ => None,
            })
            .collect()
    }

    /// Every blocked event, as its reason, cap and detail.
    pub fn blocked(&self) -> Vec<(String, Option<String>, String)> {
        self.events()
            .into_iter()
            .filter_map(|e| match e {
                Event::Blocked {
                    reason,
                    cap,
                    detail,
                } => Some((reason, cap, detail)),
                _ => None,
            })
            .collect()
    }

    /// The latest metric event, as its cumulative cost and agent time.
    pub fn last_metric(&self) -> Option<(f64, u64)> {
        self.events().into_iter().rev().find_map(|e| match e {
            Event::Metric {
                cost_usd,
                elapsed_ms,
                ..
            } => Some((cost_usd, elapsed_ms)),
            _ => None,
        })
    }

    /// The `result` of the `Done` event, if the unit has finished.
    pub fn done(&self) -> Option<String> {
        self.events().into_iter().find_map(|e| match e {
            Event::Done { result } => Some(result),
            _ => None,
        })
    }

    /// Every stage note, in order.
    pub fn stages(&self) -> Vec<(Stage, StageStatus)> {
        self.signals()
            .into_iter()
            .filter_map(|s| match s {
                Signal::Stage { stage, status, .. } => Some((stage, status)),
                _ => None,
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------------------------
// The scripted peer and its connector
// ---------------------------------------------------------------------------------------------

struct PeerControl {
    exit: watch::Receiver<Option<Exit>>,
    set: Arc<watch::Sender<Option<Exit>>>,
    killed: Arc<AtomicBool>,
}

#[async_trait]
impl ProcessControl for PeerControl {
    async fn wait(&mut self) -> Exit {
        let seen = self
            .exit
            .wait_for(|e| e.is_some())
            .await
            .expect("the peer's exit channel is open");
        seen.clone().expect("checked above")
    }

    async fn kill(&mut self) {
        self.killed.store(true, Ordering::SeqCst);
        self.set.send_if_modified(|exit| {
            if exit.is_none() {
                *exit = Some(Exit {
                    code: None,
                    description: "killed".into(),
                });
                true
            } else {
                false
            }
        });
    }
}

/// The harness side of one connection, driven by the test: it reads what the supervisor writes
/// and writes what a harness would.
pub struct Peer {
    reader: BufReader<DuplexStream>,
    writer: Option<DuplexStream>,
    exit: Arc<watch::Sender<Option<Exit>>>,
    killed: Arc<AtomicBool>,
    diagnostics: UnboundedSender<String>,
    next_id: u64,
}

impl Peer {
    /// The next message the supervisor wrote, or `None` once it has closed the harness's input.
    pub async fn recv(&mut self) -> Option<RpcMessage> {
        let mut line = String::new();
        loop {
            line.clear();
            let read = tokio::time::timeout(WAIT_LIMIT, self.reader.read_line(&mut line))
                .await
                .expect("the supervisor wrote nothing and did not close the harness's input")
                .expect("reading the supervisor's output");
            if read == 0 {
                return None;
            }
            if line.trim().is_empty() {
                continue;
            }
            return Some(
                serde_json::from_str(line.trim())
                    .unwrap_or_else(|e| panic!("the supervisor wrote {line:?}: {e}")),
            );
        }
    }

    /// The next message, which must be a request for `method`. Returns its id and the message.
    pub async fn request(&mut self, method: &str) -> (u64, RpcMessage) {
        let message = self
            .recv()
            .await
            .unwrap_or_else(|| panic!("expected {method}; the harness's input was closed"));
        match message.kind() {
            MessageKind::Request { id, method: m } if m == method => (id, message),
            _ => panic!("expected a {method} request, got {message:?}"),
        }
    }

    /// Wait until the supervisor has closed the harness's input, discarding what it wrote first.
    pub async fn input_closed(&mut self) {
        while self.recv().await.is_some() {}
    }

    pub async fn send(&mut self, message: &RpcMessage) {
        let line = serde_json::to_string(message).expect("a protocol message serialises");
        self.send_raw(&line).await;
    }

    /// Write one line exactly as given, valid or not.
    pub async fn send_raw(&mut self, line: &str) {
        let writer = self.writer.as_mut().expect("this peer has already exited");
        // A supervisor that has stopped reading is not this peer's failure to report.
        let _ = writer.write_all(format!("{line}\n").as_bytes()).await;
        let _ = writer.flush().await;
    }

    /// Write text with no line ending, as the start or the middle of a line.
    pub async fn send_raw_partial(&mut self, text: &str) {
        let writer = self.writer.as_mut().expect("this peer has already exited");
        let _ = writer.write_all(text.as_bytes()).await;
        let _ = writer.flush().await;
    }

    /// Answer `initialize` with this crate's protocol version and `capabilities`. Returns what
    /// the supervisor sent.
    pub async fn initialize(&mut self, capabilities: Capabilities) -> InitializeParams {
        self.initialize_as(PROTOCOL_VERSION, capabilities).await
    }

    /// Answer `initialize` claiming to speak `version`.
    pub async fn initialize_as(
        &mut self,
        version: &str,
        capabilities: Capabilities,
    ) -> InitializeParams {
        let (id, message) = self.request(method::INITIALIZE).await;
        let params = message.params_as().expect("initialize params");
        self.send(&RpcMessage::response(
            id,
            &InitializeResult {
                protocol_version: version.into(),
                harness: HarnessInfo {
                    name: "scripted".into(),
                    version: "0.0.0".into(),
                },
                capabilities,
            },
        ))
        .await;
        params
    }

    /// Read `unit/start` without answering it. Returns the request's id and the work order.
    pub async fn start_unacknowledged(&mut self) -> (u64, WorkOrder) {
        let (id, message) = self.request(method::UNIT_START).await;
        (id, message.params_as().expect("a work order"))
    }

    /// Read `unit/start` and acknowledge it.
    pub async fn start(&mut self) -> WorkOrder {
        let (id, order) = self.start_unacknowledged().await;
        self.reply(id).await;
        order
    }

    /// [`Peer::initialize`] then [`Peer::start`].
    pub async fn handshake(&mut self, capabilities: Capabilities) -> WorkOrder {
        self.initialize(capabilities).await;
        self.start().await
    }

    /// Answer the request `id` with an empty result.
    pub async fn reply(&mut self, id: u64) {
        self.send(&RpcMessage::response(id, &Empty {})).await;
    }

    /// Answer the request `id` with an error.
    pub async fn refuse(&mut self, id: u64) {
        self.send(&RpcMessage::error(id, -32000, "refused")).await;
    }

    pub async fn event(&mut self, event: UnitEvent) {
        self.send(&RpcMessage::notification(method::UNIT_EVENT, &event))
            .await;
    }

    pub async fn observe(&mut self, observation: Observation) {
        self.event(UnitEvent::Observed { observation }).await;
    }

    /// A stage note.
    pub async fn stage(&mut self, stage: Stage, status: StageStatus) {
        self.event(UnitEvent::Stage {
            stage,
            status,
            detail: None,
        })
        .await;
    }

    /// A metric of `cost_usd` more spend and one more second of agent time.
    pub async fn metric(&mut self, cost_usd: f64) {
        self.event(UnitEvent::Metric {
            tokens_in: 100,
            tokens_out: 10,
            cost_usd,
            elapsed_ms: 1000,
            cost_basis: None,
            stage: None,
            role: None,
            adapter: None,
            model: None,
        })
        .await;
    }

    /// `provisioned`, then `oracle_frozen` carrying [`default_freeze`].
    pub async fn provisioned_and_frozen(&mut self) {
        self.observe(Observation::Provisioned).await;
        self.observe(Observation::OracleFrozen {
            freeze: Some(default_freeze()),
        })
        .await;
    }

    /// One build, check and review cycle: `build_finished`, `checks_passed`, a 10 cent metric,
    /// and `review_finished` for `round` with green checks.
    pub async fn cycle(&mut self, round: u32, unresolved_blockers: u32) {
        self.observe(Observation::BuildFinished).await;
        self.observe(Observation::ChecksPassed).await;
        self.metric(0.10).await;
        self.observe(Observation::ReviewFinished {
            round,
            unresolved_blockers,
            checks_green: true,
        })
        .await;
    }

    /// Send an oracle `gate/request` for these files. Returns the request's id.
    pub async fn gate(&mut self, test_files: &[&str], hash: &str) -> u64 {
        self.next_id += 1;
        let id = self.next_id;
        let request = GateRequest::Oracle {
            test_files: test_files.iter().map(|s| s.to_string()).collect(),
            hash: hash.into(),
            summary: "one frozen test".into(),
            holdout_files: Vec::new(),
            holdout_hash: None,
        };
        self.send(&RpcMessage::request(id, method::GATE_REQUEST, &request))
            .await;
        id
    }

    /// The next message, which must be the answer to gate request `id`.
    pub async fn gate_reply(&mut self, id: u64) -> GateReply {
        let message = self
            .recv()
            .await
            .expect("a gate reply; the input was closed");
        match message.kind() {
            MessageKind::Response { id: got } if got == id => {
                message.result_as().expect("a gate reply")
            }
            _ => panic!("expected the reply to gate {id}, got {message:?}"),
        }
    }

    pub async fn result(&mut self, result: UnitResult) {
        self.send(&RpcMessage::notification(method::UNIT_RESULT, &result))
            .await;
    }

    /// A `pr_open` result for the work order's branch at [`HEAD`], delivered as a bundle.
    pub async fn deliver(&mut self, order: &WorkOrder) {
        self.result(pr_open(evidence(&order.branch, HEAD, "delivered.bundle")))
            .await;
    }

    /// Write a line on the harness's standard error.
    pub fn diagnostic(&self, line: &str) {
        let _ = self.diagnostics.send(line.to_string());
    }

    /// The harness process exits with `code`: its output closes.
    pub fn exit(&mut self, code: i32) {
        self.writer = None;
        self.exit.send_if_modified(|exit| {
            if exit.is_none() {
                *exit = Some(Exit {
                    code: Some(code),
                    description: format!("exit code: {code}"),
                });
                true
            } else {
                false
            }
        });
    }

    /// True once the supervisor has killed this process.
    pub fn was_killed(&self) -> bool {
        self.killed.load(Ordering::SeqCst)
    }
}

/// The freeze a scripted peer reports: one frozen test file with one test id.
pub fn default_freeze() -> OracleFreeze {
    freeze(&[(FROZEN_TEST_PATH, FROZEN_TEST_SOURCE)], &[FROZEN_ID])
}

/// A [`Connector`] whose every connection is an in-memory pipe to a [`Peer`], handed to the
/// test through a channel.
pub struct DuplexConnector {
    peers: UnboundedSender<Peer>,
    failures: Mutex<VecDeque<String>>,
    connects: AtomicUsize,
}

impl DuplexConnector {
    /// The connector, and the channel each new [`Peer`] arrives on.
    pub fn new() -> (Arc<Self>, UnboundedReceiver<Peer>) {
        let (peers, rx) = mpsc::unbounded_channel();
        (
            Arc::new(Self {
                peers,
                failures: Mutex::default(),
                connects: AtomicUsize::new(0),
            }),
            rx,
        )
    }

    /// Make the next connection attempt fail with this message.
    pub fn fail_next(&self, error: &str) {
        self.failures.lock().unwrap().push_back(error.to_string());
    }

    /// How many times `connect` has been called.
    pub fn connects(&self) -> usize {
        self.connects.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl Connector for DuplexConnector {
    fn name(&self) -> &str {
        "scripted"
    }

    async fn connect(&self) -> Result<Connection, ConnectError> {
        self.connects.fetch_add(1, Ordering::SeqCst);
        if let Some(error) = self.failures.lock().unwrap().pop_front() {
            return Err(ConnectError(error));
        }
        let (to_harness, harness_input) = tokio::io::duplex(1 << 16);
        let (harness_output, from_harness) = tokio::io::duplex(1 << 16);
        let (exit_tx, exit_rx) = watch::channel(None);
        let exit_tx = Arc::new(exit_tx);
        let killed = Arc::new(AtomicBool::new(false));
        let (diagnostics_tx, diagnostics_rx) = mpsc::unbounded_channel();
        let peer = Peer {
            reader: BufReader::new(harness_input),
            writer: Some(harness_output),
            exit: exit_tx.clone(),
            killed: killed.clone(),
            diagnostics: diagnostics_tx,
            next_id: 100,
        };
        self.peers
            .send(peer)
            .map_err(|_| ConnectError("the test dropped its peer channel".into()))?;
        Ok(Connection {
            reader: Box::new(from_harness),
            writer: Box::new(to_harness),
            diagnostics: diagnostics_rx,
            control: Box::new(PeerControl {
                exit: exit_rx,
                set: exit_tx,
                killed,
            }),
        })
    }
}

// ---------------------------------------------------------------------------------------------
// The rig
// ---------------------------------------------------------------------------------------------

/// A demo-shaped T1 unit `u1`: a $5 cap, an 1800 second wall clock, a review floor of 1, and a
/// roadmap-item work item, with no spec.
pub fn unit_spec() -> UnitSpec {
    UnitSpec {
        unit_id: "u1".into(),
        tier: Tier::T1,
        task: "A cart accepts one percent discount code.".into(),
        repo: Repo {
            url: "https://example.invalid/owner/sandbox".into(),
            slug: "owner/sandbox".into(),
            base_branch: "main".into(),
        },
        branch: "factory/u1".into(),
        test_cmd: "npm test".into(),
        usd_cap: 5.0,
        wall_clock_secs: 1800,
        min_review_rounds: 1,
        work_item: WorkItem {
            kind: WorkItemKind::RoadmapItem,
            reference: "demo:u1".into(),
            fingerprint: None,
        },
        factory: None,
    }
}

/// A fresh unit's context: `Queued`, nothing spent, the default windows, `slots` shared slots.
pub fn run_ctx(slots: Arc<Semaphore>, work_dir: &Path) -> RunCtx {
    RunCtx {
        start_phase: Phase::Queued,
        cost_usd: 0.0,
        elapsed_ms: 0,
        oracle_frozen: false,
        approved_oracle_hash: None,
        freeze: None,
        last_result: None,
        pause: None,
        opt_in: false,
        grace: DEFAULT_GRACE,
        stall: DEFAULT_STALL,
        slots,
        work_dir: work_dir.to_path_buf(),
    }
}

/// Arranges one unit's run. Start from [`Rig::builder`].
pub struct RigBuilder {
    spec: UnitSpec,
    ctx: RunCtx,
    forge: Arc<MemForge>,
    verifier: Arc<ScriptedVerifier>,
    connect_failures: Vec<String>,
}

impl RigBuilder {
    pub fn tier(mut self, tier: Tier) -> Self {
        self.spec.tier = tier;
        self
    }

    /// The unit's USD cap and wall-clock cap (0 disables the wall clock).
    pub fn caps(mut self, usd: f64, wall_clock_secs: u64) -> Self {
        self.spec.usd_cap = usd;
        self.spec.wall_clock_secs = wall_clock_secs;
        self
    }

    pub fn review_floor(mut self, min_review_rounds: u32) -> Self {
        self.spec.min_review_rounds = min_review_rounds;
        self
    }

    pub fn opt_in(mut self) -> Self {
        self.ctx.opt_in = true;
        self
    }

    /// Start as a unit brought back from the store in `phase` (`Halted` or `NeedsHuman`).
    pub fn paused(mut self, phase: Phase, reason: Option<&str>, from: Phase) -> Self {
        self.ctx.start_phase = phase;
        self.ctx.pause = Some(Pause {
            reason: reason.map(str::to_string),
            from,
        });
        self
    }

    /// What earlier spawns used.
    pub fn spent(mut self, cost_usd: f64, elapsed_ms: u64) -> Self {
        self.ctx.cost_usd = cost_usd;
        self.ctx.elapsed_ms = elapsed_ms;
        self
    }

    /// The store says the oracle is frozen, with [`default_freeze`], and approved with this hash
    /// if one is given.
    pub fn frozen(mut self, approved_oracle_hash: Option<&str>) -> Self {
        self.ctx.oracle_frozen = true;
        self.ctx.freeze = Some(default_freeze());
        self.ctx.approved_oracle_hash = approved_oracle_hash.map(str::to_string);
        self
    }

    /// The result an earlier spawn sent.
    pub fn last_result(mut self, result: UnitResult) -> Self {
        self.ctx.last_result = Some(result);
        self
    }

    pub fn windows(mut self, grace: Duration, stall: Duration) -> Self {
        self.ctx.grace = grace;
        self.ctx.stall = stall;
        self
    }

    /// Share these concurrency slots, so a test can hold some.
    pub fn slots(mut self, slots: Arc<Semaphore>) -> Self {
        self.ctx.slots = slots;
        self
    }

    /// Run the unit from `world`'s signed spec, on `world`'s forge, writing under `work_dir`.
    pub fn world(mut self, world: &World, work_dir: &Path) -> Self {
        let order = &world.order;
        self.spec.work_item = order.work_item.clone();
        self.spec.repo = order.repo.clone();
        self.spec.branch = order.branch.clone();
        self.spec.test_cmd = order.test_cmd.clone();
        self.spec.factory = Some(FactoryInputs {
            spec_id: order
                .spec
                .as_ref()
                .expect("the world has a spec")
                .id
                .clone(),
            signed_spec: world.spec_bytes.clone(),
            base_sha: world.base_sha.clone(),
            scope: order.scope.clone().expect("the world has a scope"),
            permitted_dependencies: order.permitted_dependencies.clone(),
            expected_red: order.expected_red.clone(),
            config: order.config.clone().expect("the world has a configuration"),
            profile: order.profile.clone(),
        });
        self.forge = world.forge.clone();
        self.ctx.work_dir = work_dir.to_path_buf();
        self
    }

    /// What the pull-request host reports for the pull request this unit opens.
    pub fn mergeable(mut self, mergeable: fleet_forge::Mergeability) -> Self {
        self.forge = Arc::new(MemForge::reporting(mergeable));
        self
    }

    /// Make the first connection attempts fail, one per message.
    pub fn failing_connects(mut self, errors: &[&str]) -> Self {
        self.connect_failures = errors.iter().map(|s| s.to_string()).collect();
        self
    }

    /// Queue verdicts for the scripted verifier; it answers `Verified` after them.
    pub fn verdicts(self, verdicts: Vec<Verdict>) -> Self {
        for verdict in verdicts {
            self.verifier.push(verdict);
        }
        self
    }

    /// Spawn the unit's run.
    pub fn start(self) -> Rig {
        let (connector, peers) = DuplexConnector::new();
        for error in &self.connect_failures {
            connector.fail_next(error);
        }
        // The scripted verifier imports nothing, so tell the forge the branch is there.
        self.forge.mark_imported(&self.spec.unit_id, HEAD);
        let recorder = Recorder::new();
        let (commands, command_rx) = mpsc::unbounded_channel();
        let deps = Deps {
            verifier: self.verifier.clone(),
            forge: self.forge.clone(),
        };
        let slots = self.ctx.slots.clone();
        let task = tokio::spawn(run(
            connector.clone(),
            deps,
            self.spec.clone(),
            self.ctx,
            command_rx,
            recorder.sink(),
        ));
        Rig {
            commands: Some(commands),
            verifier: self.verifier,
            forge: self.forge,
            slots,
            recorder,
            connector,
            spec: self.spec,
            peers,
            task: Some(task),
            finished: None,
        }
    }
}

/// One unit's run against a scripted peer, a scripted verifier and the in-memory forge.
pub struct Rig {
    commands: Option<UnboundedSender<Command>>,
    pub verifier: Arc<ScriptedVerifier>,
    pub forge: Arc<MemForge>,
    pub slots: Arc<Semaphore>,
    pub recorder: Recorder,
    pub connector: Arc<DuplexConnector>,
    pub spec: UnitSpec,
    peers: UnboundedReceiver<Peer>,
    task: Option<JoinHandle<Phase>>,
    finished: Option<Phase>,
}

impl Rig {
    /// [`unit_spec`] with [`run_ctx`] over three slots, writing under the system's temporary
    /// directory (nothing is written unless [`RigBuilder::world`] is used).
    pub fn builder() -> RigBuilder {
        RigBuilder {
            spec: unit_spec(),
            ctx: run_ctx(Arc::new(Semaphore::new(3)), &std::env::temp_dir()),
            forge: Arc::new(MemForge::new()),
            verifier: Arc::new(ScriptedVerifier::new()),
            connect_failures: Vec::new(),
        }
    }

    fn settle(&mut self, joined: Result<Phase, tokio::task::JoinError>) {
        self.task = None;
        match joined {
            Ok(phase) => self.finished = Some(phase),
            Err(error) if error.is_panic() => std::panic::resume_unwind(error.into_panic()),
            Err(error) => panic!("the unit's run was cancelled: {error}"),
        }
    }

    /// The next harness the unit's run starts. If the run panicked, the panic is raised here.
    pub async fn peer(&mut self) -> Peer {
        loop {
            let Some(task) = self.task.as_mut() else {
                return self.peers.try_recv().unwrap_or_else(|_| {
                    panic!(
                        "the unit finished in {:?} without starting another harness",
                        self.finished
                    )
                });
            };
            tokio::select! {
                biased;
                peer = self.peers.recv() => return peer.expect("the connector is alive"),
                joined = task => self.settle(joined),
                _ = tokio::time::sleep(WAIT_LIMIT) => panic!("no harness was started"),
            }
        }
    }

    /// The next harness, after the handshake: `initialize` answered with
    /// [`peer_capabilities`] and `unit/start` acknowledged.
    pub async fn started(&mut self) -> (Peer, WorkOrder) {
        let mut peer = self.peer().await;
        let order = peer.handshake(peer_capabilities()).await;
        (peer, order)
    }

    /// [`Rig::started`], then `provisioned` and `oracle_frozen`: a T1 unit is in `Building`.
    pub async fn building(&mut self) -> (Peer, WorkOrder) {
        let (mut peer, order) = self.started().await;
        peer.provisioned_and_frozen().await;
        self.in_phase(Phase::Building).await;
        (peer, order)
    }

    /// [`Rig::started`], then `provisioned`, `oracle_frozen` and the oracle gate request: a
    /// T2 or T3 unit is in `AwaitingOracleApproval`. Also returns the gate request's id.
    pub async fn at_gate(&mut self) -> (Peer, WorkOrder, u64) {
        let (mut peer, order) = self.started().await;
        peer.provisioned_and_frozen().await;
        let gate = peer.gate(&[FROZEN_TEST_PATH], GATE_HASH).await;
        self.in_phase(Phase::AwaitingOracleApproval).await;
        (peer, order, gate)
    }

    /// [`Rig::building`], then one clean cycle: a T1 unit with a review floor of 1 is in
    /// `MergeCheck`, its harness still live.
    pub async fn merge_check(&mut self) -> (Peer, WorkOrder) {
        let (mut peer, order) = self.building().await;
        peer.cycle(1, 0).await;
        self.in_phase(Phase::MergeCheck).await;
        (peer, order)
    }

    /// True when the run has started no harness the test has not yet taken.
    pub fn no_peer_waiting(&mut self) -> bool {
        self.peers.try_recv().is_err()
    }

    /// Send a command to the unit.
    pub fn command(&self, command: Command) {
        self.commands
            .as_ref()
            .expect("the command channel was closed by the test")
            .send(command)
            .expect("the unit's run is listening");
    }

    /// Close the command channel, as a caller that will send nothing more.
    pub fn close_commands(&mut self) {
        self.commands = None;
    }

    /// Wait until `done` is true of the signals so far. Fails the test, naming `what`, if the
    /// unit finishes or [`WAIT_LIMIT`] passes first.
    pub async fn until(&mut self, what: &str, done: impl Fn(&Recorder) -> bool) {
        loop {
            let changed = self.recorder.inner.changed.notified();
            if done(&self.recorder) {
                return;
            }
            let Some(task) = self.task.as_mut() else {
                panic!(
                    "{what}: never happened, and the unit finished in {:?}. Signals: {:#?}",
                    self.finished,
                    self.recorder.signals()
                );
            };
            tokio::select! {
                biased;
                _ = changed => {}
                joined = task => self.settle(joined),
                _ = tokio::time::sleep(WAIT_LIMIT) => panic!(
                    "{what}: never happened. Signals: {:#?}",
                    self.recorder.signals()
                ),
            }
        }
    }

    /// Wait until the unit's latest phase is `phase`.
    pub async fn in_phase(&mut self, phase: Phase) {
        self.until(&format!("phase {phase:?}"), |r| {
            r.phases().last() == Some(&phase)
        })
        .await;
    }

    /// Wait until an error event whose detail contains `needle` has been reported.
    pub async fn error_containing(&mut self, needle: &str) {
        self.until(&format!("an error containing {needle:?}"), |r| {
            r.errors().iter().any(|(_, detail)| detail.contains(needle))
        })
        .await;
    }

    /// Let every task run until all of them are waiting, without moving the clock.
    pub async fn settle_tasks(&self) {
        for _ in 0..50 {
            tokio::task::yield_now().await;
        }
    }

    /// Wait for the unit's run to return, and give the phase it finished in.
    pub async fn finish(&mut self) -> Phase {
        if let Some(task) = self.task.as_mut() {
            let joined = tokio::time::timeout(WAIT_LIMIT, task)
                .await
                .unwrap_or_else(|_| {
                    panic!(
                        "the unit never finished. Signals: {:#?}",
                        self.recorder.signals()
                    )
                });
            self.settle(joined);
        }
        self.finished.expect("settled above")
    }

    /// The unit's latest phase, or its starting phase if it has not changed yet.
    pub fn phase(&self) -> Option<Phase> {
        self.recorder.phases().last().copied()
    }
}

fn cmd(id: &str) -> String {
    id.to_string()
}

/// `Command::Halt` with this id.
pub fn halt(id: &str) -> Command {
    Command::Halt { cmd_id: cmd(id) }
}

/// `Command::Resume` with this id.
pub fn resume(id: &str) -> Command {
    Command::Resume { cmd_id: cmd(id) }
}

/// `Command::Abandon` with this id.
pub fn abandon(id: &str) -> Command {
    Command::Abandon { cmd_id: cmd(id) }
}

/// `Command::Ship` with this id.
pub fn ship(id: &str) -> Command {
    Command::Ship { cmd_id: cmd(id) }
}

/// `Command::ApproveOracle` with this id and no edits.
pub fn approve(id: &str) -> Command {
    Command::ApproveOracle {
        cmd_id: cmd(id),
        edited_test_files: None,
    }
}

/// `Command::RejectOracle` with this id.
pub fn reject(id: &str) -> Command {
    Command::RejectOracle { cmd_id: cmd(id) }
}

/// `Command::Reverify` with this id.
pub fn reverify(id: &str) -> Command {
    Command::Reverify { cmd_id: cmd(id) }
}

/// The capabilities a scripted peer declares unless a test changes them.
pub fn peer_capabilities() -> Capabilities {
    capabilities()
}

#[cfg(test)]
mod tests {
    use super::*;
    use harness_protocol::Outcome;

    #[tokio::test(start_paused = true)]
    async fn a_peer_and_a_connection_speak_the_protocol_to_each_other() {
        let (connector, mut peers) = DuplexConnector::new();
        let mut connection = connector.connect().await.unwrap();
        let mut peer = peers.recv().await.unwrap();
        let mut lines = BufReader::new(&mut connection.reader).lines();

        // The control-plane side writes initialize; the peer answers it.
        let hello = RpcMessage::request(
            1,
            method::INITIALIZE,
            &InitializeParams {
                protocol_version: PROTOCOL_VERSION.into(),
                accepted_versions: vec![PROTOCOL_VERSION.into()],
            },
        );
        let line = serde_json::to_string(&hello).unwrap() + "\n";
        connection.writer.write_all(line.as_bytes()).await.unwrap();
        let params = peer.initialize(peer_capabilities()).await;
        assert_eq!(params.accepted(), [PROTOCOL_VERSION]);
        let answer: RpcMessage =
            serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
        let result: InitializeResult = answer.result_as().unwrap();
        assert_eq!(result.capabilities, peer_capabilities());

        // The peer's events and result arrive as lines, in order.
        peer.provisioned_and_frozen().await;
        peer.metric(0.25).await;
        peer.deliver(&fleet_verify::testkit::bare_order("u1")).await;
        let mut methods = Vec::new();
        for _ in 0..4 {
            let message: RpcMessage =
                serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
            methods.push(message.method.clone().unwrap());
            if message.method.as_deref() == Some(method::UNIT_RESULT) {
                let result: UnitResult = message.params_as().unwrap();
                assert_eq!(result.outcome, Outcome::PrOpen);
                assert_eq!(result.evidence.unwrap().head_sha, HEAD);
            }
        }
        assert_eq!(
            methods,
            ["unit/event", "unit/event", "unit/event", "unit/result"]
        );

        // A diagnostic line arrives on its own channel.
        peer.diagnostic("scripted harness starting");
        assert_eq!(
            connection.diagnostics.recv().await.unwrap(),
            "scripted harness starting"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn closing_the_input_is_seen_by_the_peer_and_exit_is_seen_by_the_control() {
        let (connector, mut peers) = DuplexConnector::new();
        let mut connection = connector.connect().await.unwrap();
        let mut peer = peers.recv().await.unwrap();
        connection.writer.shutdown().await.unwrap();
        assert!(peer.recv().await.is_none());

        peer.exit(3);
        let exit = connection.control.wait().await;
        assert_eq!(
            (exit.code, exit.description.as_str()),
            (Some(3), "exit code: 3")
        );
        assert_eq!(
            connection.control.wait().await,
            exit,
            "wait can be asked again"
        );
        assert!(!peer.was_killed());
        // The peer's output is closed too.
        let mut rest = String::new();
        let read = BufReader::new(&mut connection.reader)
            .read_line(&mut rest)
            .await
            .unwrap();
        assert_eq!(read, 0);
    }

    #[tokio::test(start_paused = true)]
    async fn a_killed_peer_reports_killed_and_a_scripted_connect_failure_is_an_error() {
        let (connector, mut peers) = DuplexConnector::new();
        connector.fail_next("program not found");
        assert_eq!(
            connector.connect().await.err(),
            Some(ConnectError("program not found".into()))
        );
        let mut connection = connector.connect().await.unwrap();
        let peer = peers.recv().await.unwrap();
        connection.control.kill().await;
        assert!(peer.was_killed());
        assert_eq!(connection.control.wait().await.description, "killed");
        assert_eq!(connector.connects(), 2);
    }

    #[test]
    fn the_recorder_sorts_what_it_was_told() {
        let recorder = Recorder::new();
        let mut sink = recorder.sink();
        sink.event(Event::PhaseChanged {
            from: Phase::Queued,
            to: Phase::Provisioning,
            reason: None,
            cmd_id: None,
        });
        sink.stage(Stage::Red, StageStatus::Started, None);
        sink.event(Event::Blocked {
            reason: "usd cap".into(),
            cap: Some("usd".into()),
            detail: "spent $6.00 of $5.00".into(),
        });
        sink.event(Event::PhaseChanged {
            from: Phase::Provisioning,
            to: Phase::NeedsHuman,
            reason: Some("usd cap".into()),
            cmd_id: None,
        });
        sink.frozen(&default_freeze());
        assert_eq!(recorder.phases(), [Phase::Provisioning, Phase::NeedsHuman]);
        assert_eq!(recorder.last_reason().as_deref(), Some("usd cap"));
        assert_eq!(recorder.stages(), [(Stage::Red, StageStatus::Started)]);
        assert_eq!(recorder.blocked()[0].1.as_deref(), Some("usd"));
        assert_eq!(recorder.signals().len(), 5);
        assert_eq!(recorder.done(), None);
    }

    #[tokio::test(start_paused = true)]
    #[should_panic(expected = "lane CC-SUPERVISOR builds this")]
    async fn the_rig_raises_the_panic_of_the_run_it_started() {
        let mut rig = Rig::builder().start();
        let _ = rig.peer().await;
    }
}
````

- [ ] **Step 5: Run the tests**

```
cargo test -p fleet-supervisor --lib
cargo test -p fleet-supervisor --lib --features testkit
```

Expected, both times: `test result: ok. 5 passed; 0 failed`. One of the five,
`the_rig_raises_the_panic_of_the_run_it_started`, passes because `run` is not written yet; lane
CC-SUPERVISOR deletes that one test when it writes `run`.

- [ ] **Step 6: Commit**

```
git add crates/fleet-supervisor/src
git commit -m "feat(fleet-supervisor): the run interface, the connector seam, the registry types and a scripted-peer rig"
```

### Task 8: `fleet-api` — the routes' types, ports and state

**Files:**
- Replace: `crates/fleet-api/src/lib.rs`
- Create: `crates/fleet-api/src/auth.rs`, `dto.rs`, `ports.rs`, `hub.rs`, `routes.rs`,
  `schema.rs`, `testkit.rs`

**Produces:** `router`, `secured`, `ApiState`, `ROUTES`, `refusal_status`, `forms_index`;
`Token`, `AuthError`, `loopback_address`, `TOKEN_ENV`, `WS_PROTOCOL`, `WS_TOKEN_PREFIX`; `Hub`,
`UnitChannels`, `UnitFeed`, `LoggedEvent`, `Delivery`; the ports `Launcher`,
`HarnessDirectory`, `Clock` with `LaunchError` and `SystemClock`; every request and response
type (`MissionRequest`, `LegacyMissionRequest`, `MissionBody`, `MissionResponse`, `ApiError`,
`UnitSummary`, `Snapshot`, `UnitDetail`, `EventEnvelopeDto`, `UnitEventDto`, `CommandDto`,
`PhaseDto`, `TierDto`, `HarnessView`, `SpecView`, `ProblemView`, `SignRequest`, `SignResponse`,
`SignatureView`); `ApiSchema` and `schema_json`. In `testkit`: `TestApi`, `FakeLauncher`,
`FakeDirectory`, `FixedClock`, `call`, `healthy`, `unhealthy`, `TOKEN`, `SPEC_PATH`,
`harness_directory_contract`, `launcher_contract`.

The routes:

| Method and path | Request | Answer |
|---|---|---|
| `POST /missions` | `MissionBody`: a `MissionRequest`, or the deprecated `LegacyMissionRequest` | `MissionResponse` |
| `GET /units` | | `[UnitSummary]` |
| `GET /details` | | `[UnitDetail]` |
| `GET /units/{id}` | | `Snapshot`; 404 with an empty body for an unknown unit |
| `GET /units/{id}/detail` | | `UnitDetail` |
| `GET /units/{id}/events?since=` | | `[EventEnvelopeDto]` |
| `POST /units/{id}/commands` | `CommandDto` | 202, 404, 410 or 422, with an empty body |
| `GET /units/{id}/stream?since=` | a WebSocket upgrade | one `EventEnvelopeDto` per text frame |
| `GET /harnesses` | | `[HarnessView]` |
| `GET /specs?repo=&spec_id=&commit=` | | `SpecView` |
| `POST /signatures` | `SignRequest` | `SignResponse` |
| `GET /signatures?repo=` | | `[SignatureView]` |
| `GET /schema` | | the JSON Schema document of `ApiSchema` |

`/health`, `/swarms` and `/swarms/{id}` stay in `fleetd` with the driver path and are served
behind the same token.

What to know:

- `router` has no authentication, so tests and the characterisation suite can go through it
  directly. `secured` is the only thing the binary serves.
- `fleet-core`'s event and command types cannot derive `JsonSchema`, so `dto.rs` mirrors them.
  Two tests in `schema.rs` compare every variant's wire shape with its mirror; they pass in
  wave 0 and are the gate that keeps the mirrors honest.
- The two original read routes keep their exact keys. Everything the harness path adds is on
  the two `detail` routes.

- [ ] **Step 1: Create the source files other than `testkit.rs`**

`crates/fleet-api/src/lib.rs`:

````rust
//! `fleet-api` — the daemon's public surface: commands in over HTTP, events out over a
//! WebSocket, and a JSON Schema of every type on that surface.
//!
//! [`router`] builds the routes with no authentication, for composition and for tests.
//! [`secured`] wraps any router so that every route needs the caller's token; the binary serves
//! nothing else. The routes reach the rest of the control plane only through the seams in
//! [`ApiState`]: the stores, the forge, admission, and two ports this crate defines,
//! [`Launcher`] and [`HarnessDirectory`].

mod auth;
mod dto;
mod hub;
mod ports;
mod routes;
mod schema;

#[cfg(any(test, feature = "testkit"))]
pub mod testkit;

pub use auth::{
    loopback_address, secured, AuthError, Token, TOKEN_ENV, WS_PROTOCOL, WS_TOKEN_PREFIX,
};
pub use dto::{
    ApiError, CommandDto, EventEnvelopeDto, HarnessView, LegacyMissionRequest, MissionBody,
    MissionRequest, MissionResponse, PhaseDto, ProblemView, SignRequest, SignResponse,
    SignatureView, Snapshot, SpecView, TierDto, UnitDetail, UnitEventDto, UnitSummary,
};
pub use hub::{Delivery, Hub, LoggedEvent, UnitChannels, UnitFeed};
pub use ports::{Clock, HarnessDirectory, LaunchError, Launcher, SystemClock};
pub use routes::{forms_index, refusal_status, router, ApiState, ROUTES};
pub use schema::{schema_json, ApiSchema};
````

`crates/fleet-api/src/auth.rs`:

````rust
//! The caller's token. The cockpit's sidecar mints it, starts the daemon with it in the
//! environment, and sends it with every request. Nothing that runs in a container ever sees it.

use axum::Router;
use std::net::SocketAddr;

/// The environment variable the daemon reads its token from. Never a command-line argument: a
/// process's arguments are visible to every other process of the same user.
pub const TOKEN_ENV: &str = "FLEETD_TOKEN";

/// The WebSocket subprotocol the daemon selects. A browser cannot set a header on a WebSocket,
/// so it offers two subprotocols: this one, and [`WS_TOKEN_PREFIX`] followed by the token.
pub const WS_PROTOCOL: &str = "fleetd.v1";

/// What the token-carrying subprotocol starts with.
pub const WS_TOKEN_PREFIX: &str = "fleetd.token.";

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AuthError {
    #[error("{TOKEN_ENV} is not set: the daemon serves nothing without a token")]
    Missing,
    /// Shorter than 32 characters, or not printable ASCII without spaces.
    #[error("the token is not usable: {0}")]
    Weak(&'static str),
    #[error("{address} is not a loopback address: the daemon binds to loopback only")]
    NotLoopback { address: String },
    #[error("{0:?} is not a socket address")]
    BadAddress(String),
}

/// The token every request must carry. Its value is never printed: `Debug` shows `Token(***)`.
#[derive(Clone)]
pub struct Token(#[allow(dead_code)] String);

impl std::fmt::Debug for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Token(***)")
    }
}

impl Token {
    /// Accepts a token of at least 32 printable ASCII characters with no space.
    pub fn new(value: &str) -> Result<Self, AuthError> {
        let _ = value;
        unimplemented!("lane CC-API builds this")
    }

    /// Reads [`TOKEN_ENV`].
    pub fn from_env() -> Result<Self, AuthError> {
        unimplemented!("lane CC-API builds this")
    }

    /// True when `presented` is this token. The comparison takes the same time whatever the
    /// two values are and wherever they first differ.
    pub fn matches(&self, presented: &str) -> bool {
        let _ = presented;
        unimplemented!("lane CC-API builds this")
    }
}

/// Wrap `inner` so that every route answers 401 unless the request carries the token: as
/// `Authorization: Bearer <token>`, or, for a WebSocket upgrade, as a `Sec-WebSocket-Protocol`
/// entry of [`WS_TOKEN_PREFIX`] followed by the token. Also applies the browser-origin
/// allowlist. A preflight `OPTIONS` request is answered without a token, as browsers require.
pub fn secured(inner: Router, token: Token, allowed_origins: &[String]) -> Router {
    let _ = (inner, token, allowed_origins);
    unimplemented!("lane CC-API builds this")
}

/// Parse `text` as a socket address and refuse one that is not loopback.
pub fn loopback_address(text: &str) -> Result<SocketAddr, AuthError> {
    let _ = text;
    unimplemented!("lane CC-API builds this")
}
````

`crates/fleet-api/src/dto.rs`:

````rust
//! Every type that crosses the HTTP surface. Each derives `JsonSchema`, and [`crate::ApiSchema`]
//! names them all, so the cockpit's client is generated and never written by hand.
//!
//! `fleet-core`'s `Phase`, `Tier`, `Command` and `Event` do not derive `JsonSchema` (that crate
//! may depend on nothing that would let it), so each has a mirror here with the same wire shape.
//! A test in this crate compares the two shapes variant by variant.

use harness_protocol::{Capabilities, HarnessInfo, Stage, StageStatus};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------------------------
// Mirrors of fleet-core
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum TierDto {
    #[default]
    T1,
    T2,
    T3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PhaseDto {
    Queued,
    Provisioning,
    Spec,
    AwaitingOracleApproval,
    Building,
    Checking,
    Reviewing,
    MergeCheck,
    PrOpen,
    Done,
    NoChange,
    Failed,
    NeedsHuman,
    Halted,
}

/// One inbound control command, as `fleet_core::Command` is on the wire.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum CommandDto {
    Halt {
        cmd_id: String,
    },
    Resume {
        cmd_id: String,
    },
    Abandon {
        cmd_id: String,
    },
    Ship {
        cmd_id: String,
    },
    ApproveOracle {
        cmd_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        edited_test_files: Option<Vec<String>>,
    },
    RejectOracle {
        cmd_id: String,
    },
    Reverify {
        cmd_id: String,
    },
}

/// One record of a unit's log: every `fleet_core::Event`, as it is on the wire, plus `stage`,
/// the harness's note of which of the seven stages it is in.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UnitEventDto {
    PhaseChanged {
        from: PhaseDto,
        to: PhaseDto,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cmd_id: Option<String>,
    },
    OracleProposed {
        test_files: Vec<String>,
        hash: String,
        summary: String,
    },
    Iteration {
        /// `build`, `check` or `review`.
        kind: String,
        n: u32,
    },
    Log {
        /// `agent`, `check` or `system`.
        stream: String,
        line: String,
    },
    /// Cumulative for the unit: the latest one is the total.
    Metric {
        tokens_in: u64,
        tokens_out: u64,
        cost_usd: f64,
        elapsed_ms: u64,
    },
    Finding {
        round: u32,
        /// `info`, `minor` or `blocker`.
        severity: String,
        title: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        file: Option<String>,
        resolved: bool,
    },
    Artifact {
        /// `branch`, `pr` or `diff`.
        kind: String,
        #[serde(rename = "ref")]
        reference: String,
    },
    Blocked {
        reason: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cap: Option<String>,
        detail: String,
    },
    Error {
        /// `docker`, `github`, `agent`, `system` or `harness`.
        scope: String,
        retryable: bool,
        detail: String,
    },
    Done {
        /// `done`, `no_change` or `failed`.
        result: String,
    },
    /// Informational: it feeds the stage rail and changes no phase.
    Stage {
        stage: Stage,
        status: StageStatus,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
    },
}

/// One logged record with its ordering: what `GET /units/{id}/events` lists and what the
/// stream sends, one per text frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EventEnvelopeDto {
    pub unit_id: String,
    pub seq: u64,
    pub event: UnitEventDto,
}

// ---------------------------------------------------------------------------------------------
// Missions
// ---------------------------------------------------------------------------------------------

/// `POST /missions`: start a unit from a signed spec. The tier is not here: it is read from the
/// signed bytes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MissionRequest {
    /// The work item, as `owner/repo#N`.
    pub work_item: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,
    /// The repository to change, as `owner/name`.
    pub repo: String,
    /// SHA-256 of the signed spec bytes to run from.
    pub spec_sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    /// The registry name of the harness.
    pub harness: String,
    /// Accept a weaker guarantee than the default for this one unit.
    #[serde(default)]
    pub opt_in: bool,
    /// The same key never starts two units.
    pub idempotency_key: String,
    /// A cap below the configured ceiling, in USD.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usd_cap: Option<f64>,
    /// A wall-clock cap below the configured ceiling, in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wall_clock_secs: Option<u64>,
}

fn default_mode() -> String {
    "demo".into()
}

fn default_floor() -> u32 {
    2
}

/// `POST /missions`, the shape used before signed specs. `demo` runs the scripted harness;
/// `real` runs the in-process driver. Deprecated: it goes when that driver does.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct LegacyMissionRequest {
    pub task: String,
    #[serde(default)]
    pub tier: TierDto,
    #[serde(default = "default_mode")]
    pub mode: String,
    #[serde(default = "default_floor")]
    pub min_review_rounds: u32,
}

/// The body of `POST /missions`: a body with `spec_sha256` is a [`MissionRequest`]; one with
/// `task` is a [`LegacyMissionRequest`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum MissionBody {
    Factory(MissionRequest),
    Legacy(LegacyMissionRequest),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MissionResponse {
    pub unit_id: String,
    /// Present for a unit started from a signed spec: true when it waits for a concurrency slot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waits_for_slot: Option<bool>,
}

/// The body of every refusal of a route added in milestone 1.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ApiError {
    /// A stable short name: an admission refusal's code, or one of `unauthorized`,
    /// `bad_spec_id`, `spec_not_found`, `spec_id_mismatch`, `hash_mismatch`, `spec_unreadable`,
    /// `spec_not_ready`, `repo_not_allowed`, `launch_failed`, `store`, `forge`.
    pub code: String,
    /// One sentence for a person.
    pub message: String,
    /// For `spec_not_ready`: what stops the spec being signed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub problems: Vec<ProblemView>,
}

// ---------------------------------------------------------------------------------------------
// Units
// ---------------------------------------------------------------------------------------------

/// One row of `GET /units`. These seven keys are the ones the route has always returned.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct UnitSummary {
    pub unit_id: String,
    pub phase: String,
    pub cost: f64,
    pub usd_cap: f64,
    pub tier: String,
    pub task: String,
    pub last_seq: u64,
}

/// `GET /units/{id}`. These five keys are the ones the route has always returned.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Snapshot {
    pub unit_id: String,
    pub phase: String,
    pub usd_cap: f64,
    pub cost: f64,
    pub events: Vec<EventEnvelopeDto>,
}

/// `GET /units/{id}/detail` and each row of `GET /details`: everything stored about a unit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct UnitDetail {
    pub unit_id: String,
    pub phase: PhaseDto,
    pub tier: TierDto,
    pub task: String,
    pub repo_slug: String,
    pub base_branch: String,
    pub branch: String,
    pub mode: String,
    pub usd_cap: f64,
    pub cost: f64,
    pub wall_clock_secs: u64,
    pub elapsed_ms: u64,
    pub min_review_rounds: u32,
    pub last_seq: u64,
    pub terminal_reason: Option<String>,
    /// The registry name of the harness; absent for a unit the in-process driver runs.
    pub harness: Option<String>,
    pub work_item: Option<String>,
    pub spec_id: Option<String>,
    pub spec_sha256: Option<String>,
    pub base_sha: Option<String>,
    pub profile: Option<String>,
    pub opt_in: bool,
    pub oracle_frozen: bool,
    pub oracle_hash: Option<String>,
    pub approved_oracle_hash: Option<String>,
    /// Why the unit is waiting for a person, while it is.
    pub pause_reason: Option<String>,
    pub paused_from: Option<PhaseDto>,
    pub pr_url: Option<String>,
    /// True while a run holds the unit in memory: a command will reach it without a rehydrate.
    pub live: bool,
}

// ---------------------------------------------------------------------------------------------
// Harnesses
// ---------------------------------------------------------------------------------------------

/// One row of `GET /harnesses`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct HarnessView {
    pub name: String,
    /// `unprobed`, `healthy` or `unhealthy`.
    pub state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub harness: Option<HarnessInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<Capabilities>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// ---------------------------------------------------------------------------------------------
// Specs and signatures
// ---------------------------------------------------------------------------------------------

/// One reason a spec is not ready to sign.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProblemView {
    /// `factory_spec::Problem::code()`.
    pub code: String,
    pub message: String,
}

/// `GET /specs?repo=&spec_id=&commit=`: the spec exactly as the daemon read it. `commit` may be
/// left out, and is then the base branch's tip; the answer always names the commit it read.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SpecView {
    pub repo: String,
    pub spec_id: String,
    pub commit: String,
    /// SHA-256 of the file's bytes: what a signature will bind.
    pub sha256: String,
    /// The file's bytes as text. Bytes that are not UTF-8 are shown as replacement characters,
    /// and such a spec cannot be signed.
    pub text: String,
    pub tier: Option<TierDto>,
    pub work_item: Option<String>,
    /// Empty when the spec is ready to sign.
    pub problems: Vec<ProblemView>,
    /// The stored signature for exactly these bytes, if there is one.
    pub signature: Option<SignatureView>,
}

/// `POST /signatures`: sign the bytes of one spec file at one commit. `sha256` is the hash the
/// person was shown; the daemon reads the file again and refuses if it does not match.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SignRequest {
    pub repo: String,
    pub spec_id: String,
    pub commit: String,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SignatureView {
    pub repo: String,
    pub spec_id: String,
    pub commit: String,
    pub sha256: String,
    pub signed_by: String,
    pub signed_at_ms: i64,
}

/// The answer to `POST /signatures`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SignResponse {
    pub signature: SignatureView,
    pub tier: TierDto,
    pub work_item: String,
}
````

`crates/fleet-api/src/ports.rs`:

````rust
//! The ports this crate defines and `fleetd` implements: starting a unit's run, and reading the
//! harness registry. The routes know nothing of the supervisor.

use crate::dto::{HarnessView, LegacyMissionRequest};
use fleet_admission::Admitted;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LaunchError {
    /// A run already holds this unit.
    #[error("unit {unit_id} is already running")]
    AlreadyRunning { unit_id: String },
    /// The unit's harness is not in the registry.
    #[error("no harness is registered as {harness:?}")]
    UnknownHarness { harness: String },
    #[error("the unit could not be started: {0}")]
    Failed(String),
}

/// Starts and revives units' runs.
pub trait Launcher: Send + Sync {
    /// Start the run of a unit admission has just recorded. Returns once the run is registered
    /// with the hub; it does not wait for the harness.
    fn launch(&self, admitted: &Admitted) -> Result<(), LaunchError>;

    /// Bring a stored unit that no run holds back into memory, parked where the store says it
    /// is, so that a command can reach it. False when the unit is unknown or already held.
    fn rehydrate(&self, unit_id: &str) -> bool;

    /// Handle the deprecated request shape. `Ok` is the new unit's id; `Err` is the HTTP status
    /// and the plain-text body to answer with, exactly as the route has always answered.
    fn legacy_mission(&self, request: LegacyMissionRequest) -> Result<String, (u16, String)>;
}

/// The harness registry, as `GET /harnesses` shows it.
pub trait HarnessDirectory: Send + Sync {
    /// One row per registered harness, in registration order. No harness is started to answer.
    fn list(&self) -> Vec<HarnessView>;
}

/// The wall clock, as milliseconds since the Unix epoch.
pub trait Clock: Send + Sync {
    fn now_ms(&self) -> i64;
}

/// The system's clock.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_ms(&self) -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0)
    }
}
````

`crates/fleet-api/src/hub.rs`:

````rust
//! The live units: for each, a command channel in, a broadcast of its log out, and the one task
//! that writes its records to the store.
//!
//! Wave 0 gives the signatures only; lane CC-API moves the bodies from
//! `crates/fleetd/src/server.rs` (`register_unit_if_absent`, `spawn_forwarder`).

use crate::ports::Clock;
use fleet_core::{Command, Event};
use fleet_store::UnitStore;
use harness_protocol::{OracleFreeze, Stage, StageStatus, UnitResult};
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::sync::mpsc::UnboundedReceiver;

const NEW: &str = "lane CC-API builds this";

/// One record of a unit's log as it was stored: `json` is `{"unit_id":…,"seq":…,"event":{…}}`,
/// byte for byte what the store holds and what a stream subscriber is sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoggedEvent {
    pub unit_id: String,
    pub seq: u64,
    pub json: Arc<str>,
}

/// What happened to a command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delivery {
    /// The unit's run has it.
    Accepted,
    /// The unit was live once, and its run has ended.
    Gone,
    /// No run holds this unit.
    Unknown,
}

/// What a unit's run is given when it registers.
pub struct UnitChannels {
    pub commands: UnboundedReceiver<Command>,
    pub feed: UnitFeed,
}

/// Where a unit's run reports. Each call becomes one record with the next sequence number, is
/// written to the store, and is then broadcast. Cloning shares the numbering.
#[derive(Clone)]
pub struct UnitFeed {
    #[allow(dead_code)] // read by the bodies lane CC-API builds
    tx: tokio::sync::mpsc::UnboundedSender<Record>,
}

/// One thing a run reported, before it is numbered.
#[allow(dead_code, clippy::large_enum_variant)] // matched by the forwarder lane CC-API builds
pub(crate) enum Record {
    Event(Event),
    Stage {
        stage: Stage,
        status: StageStatus,
        detail: Option<String>,
    },
    Frozen(OracleFreeze),
    Result(UnitResult),
}

impl UnitFeed {
    /// One event of the view contract.
    pub fn event(&self, event: Event) {
        let _ = event;
        unimplemented!("{NEW}")
    }

    /// A stage note: logged and broadcast as `{"type":"stage",…}`, folded into no column.
    pub fn stage(&self, stage: Stage, status: StageStatus, detail: Option<String>) {
        let _ = (stage, status, detail);
        unimplemented!("{NEW}")
    }

    /// The oracle freeze: kept in the unit's row, not logged.
    pub fn frozen(&self, freeze: &OracleFreeze) {
        let _ = freeze;
        unimplemented!("{NEW}")
    }

    /// The harness's result: kept in the unit's row, not logged.
    pub fn result(&self, result: &UnitResult) {
        let _ = result;
        unimplemented!("{NEW}")
    }
}

/// The table of live units. Cloning shares it.
#[derive(Clone)]
pub struct Hub {
    #[allow(dead_code)] // read by the bodies lane CC-API builds
    units: Arc<dyn UnitStore>,
    #[allow(dead_code)]
    clock: Arc<dyn Clock>,
}

impl Hub {
    pub fn new(units: Arc<dyn UnitStore>, clock: Arc<dyn Clock>) -> Self {
        Self { units, clock }
    }

    /// Register a unit and start the task that numbers, stores and broadcasts its records,
    /// continuing after the `last_seq` the store has for it. `None` when a run already holds
    /// the unit: the check and the insert are one step, so two callers cannot both get channels.
    pub fn register(&self, unit_id: &str) -> Option<UnitChannels> {
        let _ = unit_id;
        unimplemented!("{NEW}")
    }

    /// Hand a command to the unit's run.
    pub fn send(&self, unit_id: &str, command: Command) -> Delivery {
        let _ = (unit_id, command);
        unimplemented!("{NEW}")
    }

    /// Subscribe to the unit's records from now on. `None` when no run holds the unit.
    pub fn subscribe(&self, unit_id: &str) -> Option<broadcast::Receiver<LoggedEvent>> {
        let _ = unit_id;
        unimplemented!("{NEW}")
    }

    /// The ids of the units a run holds whose command channel is still open.
    pub fn live(&self) -> Vec<String> {
        unimplemented!("{NEW}")
    }

    /// True when a run has ever registered this unit in this process.
    pub fn knows(&self, unit_id: &str) -> bool {
        let _ = unit_id;
        unimplemented!("{NEW}")
    }
}
````

`crates/fleet-api/src/routes.rs`:

````rust
//! The routes. Wave 0 gives the state, the route table and the signatures; lane CC-API moves
//! the handlers from `crates/fleetd/src/server.rs` and builds the new ones.

use crate::hub::Hub;
use crate::ports::{Clock, HarnessDirectory, Launcher};
use axum::Router;
use factory_spec::FormsIndex;
use fleet_admission::{Admit, Refusal};
use fleet_forge::{RepoForge, RepoRef};
use fleet_store::{SignatureStore, UnitStore};
use std::sync::Arc;

/// Every route of this crate, as its method and its path. `fleetd` has a test that sends each
/// one without a token through the router it serves and expects 401.
pub const ROUTES: &[(&str, &str)] = &[
    ("POST", "/missions"),
    ("GET", "/units"),
    ("GET", "/details"),
    ("GET", "/units/:id"),
    ("GET", "/units/:id/detail"),
    ("GET", "/units/:id/events"),
    ("POST", "/units/:id/commands"),
    ("GET", "/units/:id/stream"),
    ("GET", "/harnesses"),
    ("GET", "/specs"),
    ("POST", "/signatures"),
    ("GET", "/signatures"),
    ("GET", "/schema"),
];

/// Everything the routes reach.
#[derive(Clone)]
pub struct ApiState {
    pub units: Arc<dyn UnitStore>,
    pub signatures: Arc<dyn SignatureStore>,
    pub forge: Arc<dyn RepoForge>,
    pub admission: Arc<dyn Admit>,
    pub launcher: Arc<dyn Launcher>,
    pub harnesses: Arc<dyn HarnessDirectory>,
    pub hub: Hub,
    /// The repositories a spec may be read from and signed in: the admission allowlist.
    pub repos: Vec<RepoRef>,
    /// Who signs: the name recorded with each signature.
    pub operator: String,
    pub clock: Arc<dyn Clock>,
}

/// The routes of [`ROUTES`], with no authentication and no origin check. The binary serves this
/// only inside [`crate::secured`].
pub fn router(state: ApiState) -> Router {
    let _ = state;
    unimplemented!("lane CC-API builds this")
}

/// The HTTP status a refusal is answered with: 400 for a request that is wrong in itself
/// (`missing_key`, `work_item_malformed`, `cap_above_ceiling`), 403 for `repo_not_allowed` and
/// `not_signed`, 409 for `duplicate_key`, `work_item_mismatch` and `presets_version_mismatch`,
/// 422 for a repository or harness that cannot take the unit (`not_onboarded`,
/// `preset_unsupported`, `harness_unknown`, `preset_not_declared`, `ineligible`), 429 for
/// `spend_ceiling`, 500 for `reservation_failed` and `spec_unreadable`, 502 for `forge`, and 503
/// for `harness_unhealthy` and `spend_unknown`.
pub fn refusal_status(refusal: &Refusal) -> u16 {
    let _ = refusal;
    unimplemented!("lane CC-API builds this")
}

/// Build the index of Form interface files from a repository's Form documents. `forms` are the
/// files under `forms/` at one commit, as path and text. A Form is a file whose first heading is
/// `# Form: <name>`; its interface files are the entries of the `- interface-files:` list in its
/// header. A file that is not a Form (the registry, a README) contributes nothing.
pub fn forms_index(forms: &[(String, String)]) -> FormsIndex {
    let _ = forms;
    unimplemented!("lane CC-API builds this")
}
````

`crates/fleet-api/src/schema.rs`:

````rust
//! The API's JSON Schema: one document whose `$defs` hold every type on the HTTP surface.

use crate::dto::{
    ApiError, CommandDto, EventEnvelopeDto, HarnessView, MissionBody, MissionResponse, SignRequest,
    SignResponse, SignatureView, Snapshot, SpecView, UnitDetail, UnitSummary,
};
use schemars::{schema_for, JsonSchema};

/// Exists only to pull every API type into a single schema document. Field names say which
/// route each type belongs to.
///
/// | Route | Request | Response |
/// |---|---|---|
/// | `POST /missions` | `MissionBody` | `MissionResponse` |
/// | `GET /units` | | `[UnitSummary]` |
/// | `GET /details` | | `[UnitDetail]` |
/// | `GET /units/{id}` | | `Snapshot` |
/// | `GET /units/{id}/detail` | | `UnitDetail` |
/// | `GET /units/{id}/events?since=` | | `[EventEnvelopeDto]` |
/// | `POST /units/{id}/commands` | `CommandDto` | empty, 202 |
/// | `GET /units/{id}/stream?since=` | | one `EventEnvelopeDto` per text frame |
/// | `GET /harnesses` | | `[HarnessView]` |
/// | `GET /specs?repo=&spec_id=&commit=` | | `SpecView` |
/// | `POST /signatures` | `SignRequest` | `SignResponse` |
/// | `GET /signatures?repo=` | | `[SignatureView]` |
/// | any refusal of a route added in milestone 1 | | `ApiError` |
#[derive(JsonSchema)]
pub struct ApiSchema {
    pub post_missions_request: MissionBody,
    pub post_missions_response: MissionResponse,
    pub get_units_response: Vec<UnitSummary>,
    pub get_details_response: Vec<UnitDetail>,
    pub get_unit_response: Snapshot,
    pub get_unit_detail_response: UnitDetail,
    pub get_unit_events_response: Vec<EventEnvelopeDto>,
    pub post_unit_commands_request: CommandDto,
    pub unit_stream_frame: EventEnvelopeDto,
    pub get_harnesses_response: Vec<HarnessView>,
    pub get_specs_response: SpecView,
    pub post_signatures_request: SignRequest,
    pub post_signatures_response: SignResponse,
    pub get_signatures_response: Vec<SignatureView>,
    pub error: ApiError,
}

/// The schema as pretty JSON with a trailing newline: the committed contract's exact form, and
/// the body of `GET /schema`.
pub fn schema_json() -> String {
    let value = serde_json::to_value(schema_for!(ApiSchema)).expect("the schema serialises");
    let mut out = serde_json::to_string_pretty(&value).expect("the schema serialises");
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::{CommandDto, UnitEventDto};
    use fleet_core::{
        ArtifactKind, Command, ErrorScope, Event, IterationKind, LogStream, Phase, Severity,
    };

    #[test]
    fn the_schema_defines_every_api_type() {
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let defs = schema["$defs"].as_object().expect("a $defs object");
        for name in [
            "MissionBody",
            "MissionRequest",
            "LegacyMissionRequest",
            "MissionResponse",
            "UnitSummary",
            "UnitDetail",
            "Snapshot",
            "EventEnvelopeDto",
            "UnitEventDto",
            "CommandDto",
            "PhaseDto",
            "TierDto",
            "HarnessView",
            "Capabilities",
            "SpecView",
            "ProblemView",
            "SignRequest",
            "SignResponse",
            "SignatureView",
            "ApiError",
            "Stage",
            "StageStatus",
        ] {
            assert!(
                defs.contains_key(name),
                "the schema has no definition of {name}"
            );
        }
        assert!(schema_json().ends_with("}\n"));
    }

    fn phases() -> [Phase; 14] {
        [
            Phase::Queued,
            Phase::Provisioning,
            Phase::Spec,
            Phase::AwaitingOracleApproval,
            Phase::Building,
            Phase::Checking,
            Phase::Reviewing,
            Phase::MergeCheck,
            Phase::PrOpen,
            Phase::Done,
            Phase::NoChange,
            Phase::Failed,
            Phase::NeedsHuman,
            Phase::Halted,
        ]
    }

    fn every_event() -> Vec<Event> {
        let mut events: Vec<Event> = phases()
            .into_iter()
            .map(|to| Event::PhaseChanged {
                from: Phase::Queued,
                to,
                reason: None,
                cmd_id: None,
            })
            .collect();
        events.push(Event::PhaseChanged {
            from: Phase::Building,
            to: Phase::NeedsHuman,
            reason: Some("usd cap".into()),
            cmd_id: Some("c1".into()),
        });
        events.push(Event::OracleProposed {
            test_files: vec!["a.test.js".into()],
            hash: "h".into(),
            summary: "s".into(),
        });
        for kind in [
            IterationKind::Build,
            IterationKind::Check,
            IterationKind::Review,
        ] {
            events.push(Event::Iteration { kind, n: 2 });
        }
        for stream in [LogStream::Agent, LogStream::Check, LogStream::System] {
            events.push(Event::Log {
                stream,
                line: "l".into(),
            });
        }
        events.push(Event::Metric {
            tokens_in: 1,
            tokens_out: 2,
            cost_usd: 0.5,
            elapsed_ms: 9,
        });
        for (severity, file) in [
            (Severity::Info, None),
            (Severity::Minor, Some("src/a.js".to_string())),
            (Severity::Blocker, None),
        ] {
            events.push(Event::Finding {
                round: 1,
                severity,
                title: "t".into(),
                file,
                resolved: false,
            });
        }
        for kind in [ArtifactKind::Branch, ArtifactKind::Pr, ArtifactKind::Diff] {
            events.push(Event::Artifact {
                kind,
                reference: "r".into(),
            });
        }
        events.push(Event::Blocked {
            reason: "stalled".into(),
            cap: None,
            detail: "d".into(),
        });
        events.push(Event::Blocked {
            reason: "usd cap".into(),
            cap: Some("usd".into()),
            detail: "d".into(),
        });
        for scope in [
            ErrorScope::Docker,
            ErrorScope::Github,
            ErrorScope::Agent,
            ErrorScope::System,
            ErrorScope::Harness,
        ] {
            events.push(Event::Error {
                scope,
                retryable: true,
                detail: "d".into(),
            });
        }
        events.push(Event::Done {
            result: "done".into(),
        });
        events
    }

    #[test]
    fn every_core_event_has_the_same_wire_shape_as_its_mirror() {
        for event in every_event() {
            let wire = serde_json::to_value(&event).unwrap();
            let mirror: UnitEventDto = serde_json::from_value(wire.clone())
                .unwrap_or_else(|e| panic!("{wire} does not read as a UnitEventDto: {e}"));
            assert_eq!(serde_json::to_value(&mirror).unwrap(), wire);
        }
    }

    #[test]
    fn every_core_command_has_the_same_wire_shape_as_its_mirror() {
        let id = || "c1".to_string();
        let commands = [
            Command::Halt { cmd_id: id() },
            Command::Resume { cmd_id: id() },
            Command::Abandon { cmd_id: id() },
            Command::Ship { cmd_id: id() },
            Command::ApproveOracle {
                cmd_id: id(),
                edited_test_files: None,
            },
            Command::ApproveOracle {
                cmd_id: id(),
                edited_test_files: Some(vec!["a.test.js".into()]),
            },
            Command::RejectOracle { cmd_id: id() },
            Command::Reverify { cmd_id: id() },
        ];
        for command in commands {
            let wire = serde_json::to_value(&command).unwrap();
            let mirror: CommandDto = serde_json::from_value(wire.clone())
                .unwrap_or_else(|e| panic!("{wire} does not read as a CommandDto: {e}"));
            assert_eq!(serde_json::to_value(&mirror).unwrap(), wire);
            let back: Command =
                serde_json::from_value(serde_json::to_value(&mirror).unwrap()).unwrap();
            assert_eq!(back, command);
        }
    }

    #[test]
    fn a_stage_note_is_a_record_the_core_event_type_does_not_have() {
        let note = UnitEventDto::Stage {
            stage: harness_protocol::Stage::Red,
            status: harness_protocol::StageStatus::Started,
            detail: None,
        };
        let wire = serde_json::to_value(&note).unwrap();
        assert_eq!(
            wire,
            serde_json::json!({"type": "stage", "stage": "red", "status": "started"})
        );
        assert!(serde_json::from_value::<Event>(wire).is_err());
    }
}
````

- [ ] **Step 2: Write the failing test**

Create `crates/fleet-api/src/testkit.rs` with only this:

````rust
#[cfg(test)]
mod tests {
    use super::*;
    use axum::routing::get;

    #[test]
    fn the_fakes_pass_the_port_contracts() {
        harness_directory_contract(&FakeDirectory(vec![
            healthy("reqdrive"),
            unhealthy("fake", "harness-fake not found"),
            HarnessView {
                name: "later".into(),
                state: "unprobed".into(),
                harness: None,
                capabilities: None,
                error: None,
            },
        ]));
        launcher_contract(&FakeLauncher::new());
    }

    #[test]
    fn the_fake_launcher_records_and_can_be_made_to_fail() {
        let launcher = FakeLauncher::new();
        assert_eq!(
            launcher.legacy_mission(LegacyMissionRequest {
                task: "x".into(),
                tier: crate::dto::TierDto::T2,
                mode: "demo".into(),
                min_review_rounds: 1,
            }),
            Ok("u1".to_string())
        );
        assert_eq!(launcher.legacy_requests().len(), 1);
        launcher.stored(&["u7"]);
        assert!(launcher.rehydrate("u7"));
        assert_eq!(launcher.rehydrated(), ["u7"]);
        assert!(launcher.take_channels("u7").is_none(), "no hub is attached");
    }

    #[tokio::test]
    async fn call_sends_one_request_and_returns_status_and_body() {
        let router = Router::new().route(
            "/echo",
            get(|headers: axum::http::HeaderMap| async move {
                headers
                    .get("x-test")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("none")
                    .to_string()
            }),
        );
        let (status, body) = call(router.clone(), "GET", "/echo", None, &[("x-test", "yes")]).await;
        assert_eq!((status, body.as_str()), (StatusCode::OK, "yes"));
        let (status, _) = call(router, "GET", "/missing", None, &[]).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn the_arranged_api_holds_the_spec_it_describes() {
        use fleet_forge::RepoForge;
        let api = TestApi::new();
        assert_eq!(
            api.forge
                .read_file(&api.repo, &api.commit, SPEC_PATH)
                .await
                .unwrap(),
            Some(api.spec_bytes.clone())
        );
        assert_eq!(api.sign_request()["sha256"], api.spec_sha256.as_str());
        assert_eq!(api.mission("k")["spec_sha256"], api.spec_sha256.as_str());
        assert_eq!(api.clock.now_ms(), 1_700_000_000_000);
        assert!(TOKEN.len() >= 32);
    }
}
````

- [ ] **Step 3: Run it and see it fail**

```
cargo test -p fleet-api --lib
```

Expected: the crate does not compile; the errors name `FakeDirectory`, `FakeLauncher`,
`TestApi`, `call`, `healthy`, `unhealthy`, `harness_directory_contract` and
`launcher_contract`.

- [ ] **Step 4: Write the fakes, the arranged API and the port suites**

Replace `crates/fleet-api/src/testkit.rs` with:

````rust
//! Fakes for this crate's ports, an arranged API ([`TestApi`]), a helper that sends one request
//! through a router, and the contract suites for the two ports.
//!
//! Compiled for this crate's own tests, and for other crates behind the `testkit` feature.

use crate::dto::{HarnessView, LegacyMissionRequest};
use crate::hub::{Hub, UnitChannels};
use crate::ports::{Clock, HarnessDirectory, LaunchError, Launcher};
use crate::routes::ApiState;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use factory_spec::testkit::SpecBuilder;
use fleet_admission::testkit::FakeAdmission;
use fleet_admission::Admitted;
use fleet_forge::testkit::MemForge;
use fleet_forge::RepoRef;
use fleet_store::testkit::MemStore;
use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use tower::ServiceExt;

/// A token long enough to be accepted.
pub const TOKEN: &str = "test-token-0123456789abcdef0123456789abcdef";

/// The path of the spec file [`TestApi`] commits.
pub const SPEC_PATH: &str = ".reqdrive/specs/SPEC-0001.md";

// ---------------------------------------------------------------------------------------------
// Fakes
// ---------------------------------------------------------------------------------------------

/// A [`Clock`] a test sets.
#[derive(Debug, Default)]
pub struct FixedClock(AtomicI64);

impl FixedClock {
    pub fn at(now_ms: i64) -> Self {
        Self(AtomicI64::new(now_ms))
    }

    pub fn set(&self, now_ms: i64) {
        self.0.store(now_ms, Ordering::SeqCst);
    }
}

impl Clock for FixedClock {
    fn now_ms(&self) -> i64 {
        self.0.load(Ordering::SeqCst)
    }
}

#[derive(Default)]
struct LauncherState {
    hub: Option<Hub>,
    launched: Vec<Admitted>,
    rehydrated: Vec<String>,
    legacy: Vec<LegacyMissionRequest>,
    stored: Vec<String>,
    held: HashMap<String, UnitChannels>,
    fail_launch: Option<LaunchError>,
    next_legacy: u64,
}

/// A [`Launcher`] that starts nothing. It records what it was asked. Given a hub, it registers
/// each unit it "starts" and keeps the unit's channels, so a test can play the run: read the
/// commands the routes deliver, and report events through the feed.
#[derive(Default)]
pub struct FakeLauncher {
    state: Mutex<LauncherState>,
}

impl FakeLauncher {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register started and rehydrated units with this hub.
    pub fn attach(&self, hub: Hub) {
        self.state.lock().unwrap().hub = Some(hub);
    }

    /// Make the next `launch` fail.
    pub fn fail_launch(&self, error: LaunchError) {
        self.state.lock().unwrap().fail_launch = Some(error);
    }

    /// The units `rehydrate` will find, as if they were in the store with no run.
    pub fn stored(&self, unit_ids: &[&str]) {
        self.state.lock().unwrap().stored = unit_ids.iter().map(|s| s.to_string()).collect();
    }

    pub fn launched(&self) -> Vec<Admitted> {
        self.state.lock().unwrap().launched.clone()
    }

    pub fn rehydrated(&self) -> Vec<String> {
        self.state.lock().unwrap().rehydrated.clone()
    }

    pub fn legacy_requests(&self) -> Vec<LegacyMissionRequest> {
        self.state.lock().unwrap().legacy.clone()
    }

    /// Take the channels of a unit this launcher registered, to play its run. `None` if the
    /// unit was never registered, the hub was not attached, or the channels were already taken.
    pub fn take_channels(&self, unit_id: &str) -> Option<UnitChannels> {
        self.state.lock().unwrap().held.remove(unit_id)
    }

    fn register(state: &mut LauncherState, unit_id: &str) -> bool {
        let Some(hub) = state.hub.clone() else {
            return true;
        };
        match hub.register(unit_id) {
            Some(channels) => {
                state.held.insert(unit_id.to_string(), channels);
                true
            }
            None => false,
        }
    }
}

impl Launcher for FakeLauncher {
    fn launch(&self, admitted: &Admitted) -> Result<(), LaunchError> {
        let mut state = self.state.lock().unwrap();
        if let Some(error) = state.fail_launch.take() {
            return Err(error);
        }
        if !Self::register(&mut state, &admitted.unit_id) {
            return Err(LaunchError::AlreadyRunning {
                unit_id: admitted.unit_id.clone(),
            });
        }
        state.launched.push(admitted.clone());
        Ok(())
    }

    fn rehydrate(&self, unit_id: &str) -> bool {
        let mut state = self.state.lock().unwrap();
        if !state.stored.iter().any(|u| u == unit_id) {
            return false;
        }
        if !Self::register(&mut state, unit_id) {
            return false;
        }
        state.rehydrated.push(unit_id.to_string());
        true
    }

    fn legacy_mission(&self, request: LegacyMissionRequest) -> Result<String, (u16, String)> {
        let mut state = self.state.lock().unwrap();
        state.legacy.push(request.clone());
        if request.mode != "demo" && request.mode != "real" {
            return Err((400, format!("unknown mode: {}", request.mode)));
        }
        state.next_legacy += 1;
        Ok(format!("u{}", state.next_legacy))
    }
}

/// A [`HarnessDirectory`] with a fixed list.
#[derive(Debug, Clone, Default)]
pub struct FakeDirectory(pub Vec<HarnessView>);

impl HarnessDirectory for FakeDirectory {
    fn list(&self) -> Vec<HarnessView> {
        self.0.clone()
    }
}

/// A healthy row for `name`, declaring the admission testkit's capabilities.
pub fn healthy(name: &str) -> HarnessView {
    HarnessView {
        name: name.into(),
        state: "healthy".into(),
        harness: Some(harness_protocol::HarnessInfo {
            name: name.into(),
            version: "0.0.0".into(),
        }),
        capabilities: Some(fleet_admission::testkit::capabilities()),
        error: None,
    }
}

/// An unhealthy row for `name`.
pub fn unhealthy(name: &str, error: &str) -> HarnessView {
    HarnessView {
        name: name.into(),
        state: "unhealthy".into(),
        harness: None,
        capabilities: None,
        error: Some(error.into()),
    }
}

// ---------------------------------------------------------------------------------------------
// One request through a router
// ---------------------------------------------------------------------------------------------

/// Send one request through `router` and return the status and the body as text. `headers` are
/// added as given; a JSON body sets `content-type`.
pub async fn call(
    router: Router,
    method: &str,
    uri: &str,
    body: Option<serde_json::Value>,
    headers: &[(&str, &str)],
) -> (StatusCode, String) {
    let mut request = Request::builder().method(method).uri(uri);
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let request = match body {
        Some(json) => request
            .header("content-type", "application/json")
            .body(Body::from(json.to_string())),
        None => request.body(Body::empty()),
    }
    .expect("a well-formed request");
    let response = router.oneshot(request).await.expect("the router answers");
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("a readable body");
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

// ---------------------------------------------------------------------------------------------
// An arranged API
// ---------------------------------------------------------------------------------------------

/// The routes over fakes: the in-memory store and forge, the fake admission, the fake launcher
/// (attached to the hub) and a directory with one healthy harness, `reqdrive`. The forge holds
/// `owner/sandbox` with the spec `SpecBuilder::new()` committed at [`SPEC_PATH`].
pub struct TestApi {
    pub store: Arc<MemStore>,
    pub forge: Arc<MemForge>,
    pub admission: Arc<FakeAdmission>,
    pub launcher: Arc<FakeLauncher>,
    pub clock: Arc<FixedClock>,
    pub state: ApiState,
    pub repo: RepoRef,
    /// The commit that holds the spec file.
    pub commit: String,
    pub spec_bytes: Vec<u8>,
    pub spec_sha256: String,
}

impl Default for TestApi {
    fn default() -> Self {
        Self::new()
    }
}

impl TestApi {
    pub fn new() -> Self {
        let repo = fleet_forge::testkit::repo();
        let forge = Arc::new(MemForge::new());
        let spec_bytes = SpecBuilder::new().build();
        let spec_text = String::from_utf8(spec_bytes.clone()).expect("the builder writes UTF-8");
        let commit = forge.commit_base(
            &repo,
            &[
                (SPEC_PATH, Some(spec_text.as_str())),
                ("src/cart.js", Some("export const total = () => 100;\n")),
            ],
        );
        let store = Arc::new(MemStore::new());
        let admission = Arc::new(FakeAdmission::new());
        let launcher = Arc::new(FakeLauncher::new());
        let clock = Arc::new(FixedClock::at(1_700_000_000_000));
        let hub = Hub::new(store.clone(), clock.clone());
        launcher.attach(hub.clone());
        let state = ApiState {
            units: store.clone(),
            signatures: store.clone(),
            forge: forge.clone(),
            admission: admission.clone(),
            launcher: launcher.clone(),
            harnesses: Arc::new(FakeDirectory(vec![healthy("reqdrive")])),
            hub,
            repos: vec![repo.clone()],
            operator: "owner".into(),
            clock: clock.clone(),
        };
        TestApi {
            store,
            forge,
            admission,
            launcher,
            clock,
            state,
            repo,
            commit,
            spec_sha256: factory_spec::sha256_hex(&spec_bytes),
            spec_bytes,
        }
    }

    /// The routes, with no authentication.
    pub fn router(&self) -> Router {
        crate::router(self.state.clone())
    }

    pub async fn get(&self, uri: &str) -> (StatusCode, String) {
        call(self.router(), "GET", uri, None, &[]).await
    }

    pub async fn post(&self, uri: &str, body: serde_json::Value) -> (StatusCode, String) {
        call(self.router(), "POST", uri, Some(body), &[]).await
    }

    /// A `POST /missions` body this API admits.
    pub fn mission(&self, idempotency_key: &str) -> serde_json::Value {
        serde_json::json!({
            "work_item": "example/sandbox#1",
            "repo": "owner/sandbox",
            "spec_sha256": self.spec_sha256,
            "harness": "reqdrive",
            "idempotency_key": idempotency_key,
        })
    }

    /// A `POST /signatures` body for the committed spec.
    pub fn sign_request(&self) -> serde_json::Value {
        serde_json::json!({
            "repo": "owner/sandbox",
            "spec_id": "SPEC-0001",
            "commit": self.commit,
            "sha256": self.spec_sha256,
        })
    }
}

// ---------------------------------------------------------------------------------------------
// Contract suites for the ports
// ---------------------------------------------------------------------------------------------

/// What every [`HarnessDirectory`] must do: names are unique, and each row's optional fields
/// agree with its state.
pub fn harness_directory_contract<D: HarnessDirectory>(directory: &D) {
    let rows = directory.list();
    let mut names: Vec<&str> = rows.iter().map(|r| r.name.as_str()).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), rows.len(), "a harness is listed twice");
    for row in &rows {
        assert!(!row.name.is_empty());
        match row.state.as_str() {
            "healthy" => {
                assert!(
                    row.harness.is_some() && row.capabilities.is_some(),
                    "{row:?}"
                );
                assert!(row.error.is_none(), "{row:?}");
            }
            "unhealthy" => {
                assert!(
                    row.error.as_deref().is_some_and(|e| !e.is_empty()),
                    "{row:?}"
                );
                assert!(
                    row.harness.is_none() && row.capabilities.is_none(),
                    "{row:?}"
                );
            }
            "unprobed" => {
                assert!(
                    row.harness.is_none() && row.capabilities.is_none() && row.error.is_none(),
                    "{row:?}"
                );
            }
            other => panic!("{other:?} is not a harness state"),
        }
    }
    assert_eq!(
        directory.list(),
        rows,
        "listing twice gives the same answer"
    );
}

/// What every [`Launcher`] must do without a unit to start: an unknown unit is not rehydrated,
/// and the deprecated request shape with an unknown mode is refused as it always was.
pub fn launcher_contract<L: Launcher>(launcher: &L) {
    assert!(!launcher.rehydrate("no-such-unit"));
    let refused = launcher.legacy_mission(LegacyMissionRequest {
        task: "x".into(),
        tier: crate::dto::TierDto::T1,
        mode: "sideways".into(),
        min_review_rounds: 2,
    });
    assert_eq!(refused, Err((400, "unknown mode: sideways".to_string())));
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::routing::get;

    #[test]
    fn the_fakes_pass_the_port_contracts() {
        harness_directory_contract(&FakeDirectory(vec![
            healthy("reqdrive"),
            unhealthy("fake", "harness-fake not found"),
            HarnessView {
                name: "later".into(),
                state: "unprobed".into(),
                harness: None,
                capabilities: None,
                error: None,
            },
        ]));
        launcher_contract(&FakeLauncher::new());
    }

    #[test]
    fn the_fake_launcher_records_and_can_be_made_to_fail() {
        let launcher = FakeLauncher::new();
        assert_eq!(
            launcher.legacy_mission(LegacyMissionRequest {
                task: "x".into(),
                tier: crate::dto::TierDto::T2,
                mode: "demo".into(),
                min_review_rounds: 1,
            }),
            Ok("u1".to_string())
        );
        assert_eq!(launcher.legacy_requests().len(), 1);
        launcher.stored(&["u7"]);
        assert!(launcher.rehydrate("u7"));
        assert_eq!(launcher.rehydrated(), ["u7"]);
        assert!(launcher.take_channels("u7").is_none(), "no hub is attached");
    }

    #[tokio::test]
    async fn call_sends_one_request_and_returns_status_and_body() {
        let router = Router::new().route(
            "/echo",
            get(|headers: axum::http::HeaderMap| async move {
                headers
                    .get("x-test")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("none")
                    .to_string()
            }),
        );
        let (status, body) = call(router.clone(), "GET", "/echo", None, &[("x-test", "yes")]).await;
        assert_eq!((status, body.as_str()), (StatusCode::OK, "yes"));
        let (status, _) = call(router, "GET", "/missing", None, &[]).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn the_arranged_api_holds_the_spec_it_describes() {
        use fleet_forge::RepoForge;
        let api = TestApi::new();
        assert_eq!(
            api.forge
                .read_file(&api.repo, &api.commit, SPEC_PATH)
                .await
                .unwrap(),
            Some(api.spec_bytes.clone())
        );
        assert_eq!(api.sign_request()["sha256"], api.spec_sha256.as_str());
        assert_eq!(api.mission("k")["spec_sha256"], api.spec_sha256.as_str());
        assert_eq!(api.clock.now_ms(), 1_700_000_000_000);
        assert!(TOKEN.len() >= 32);
    }
}
````

- [ ] **Step 5: Run the tests**

```
cargo test -p fleet-api --lib
cargo test -p fleet-api --lib --features testkit
```

Expected, both times: `test result: ok. 8 passed; 0 failed`: four in `schema` and four in
`testkit`.

- [ ] **Step 6: Commit**

```
git add crates/fleet-api/src
git commit -m "feat(fleet-api): the API's types, ports, state and schema, with fakes for its ports"
```

### Task 9: The wiring interface and the end-to-end harness's interface

Neither has a fake or a suite: `fleetd` is where the real implementations are chosen, and
`fleet-e2e` is test support. Their signatures are fixed here so that lanes CC-WIRE and CC-E2E
start from the same names as everyone else.

**Files:**
- Create: `crates/fleetd/src/wire.rs`
- Modify: `crates/fleetd/src/lib.rs` (one line)
- Create: `tests/e2e/src/lib.rs`

- [ ] **Step 1: Create `crates/fleetd/src/wire.rs`**

````rust
//! Wiring: the one place real implementations are chosen and joined. Nothing here decides
//! anything about a unit; every rule lives in the crate this module calls.
//!
//! Wave 0 gives the signatures only; lane CC-WIRE builds the bodies.

use axum::Router;
use fleet_admission::{AdmissionConfig, HarnessCatalog, HarnessStatus};
use fleet_api::{HarnessDirectory, HarnessView, Hub, Token, UnitFeed};
use fleet_core::{Event, Tier};
use fleet_forge::RepoForge;
use fleet_runner::{CheckRunner, UnitCensus};
use fleet_store::SharedStore;
use fleet_supervisor::{Registry, Sink};
use fleet_verify::EvidenceVerifier;
use harness_protocol::{OracleFreeze, Stage, StageStatus, UnitResult};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

const NEW: &str = "lane CC-WIRE builds this";

/// Where pull requests are opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrHostChoice {
    /// GitHub, through the `gh` command.
    GitHub,
    /// A directory of JSON records, for a hermetic run against a local bare repository.
    Local { dir: PathBuf },
}

/// Everything the daemon is configured with.
#[derive(Debug, Clone)]
pub struct Settings {
    /// A loopback address.
    pub addr: SocketAddr,
    pub db: PathBuf,
    /// The directory the daemon owns: forge clones, units' work directories, verifier exports.
    pub data_dir: PathBuf,
    /// The harness registry file, if there is one. The scripted harness is always registered.
    pub registry_file: Option<PathBuf>,
    pub token: Token,
    pub allowed_origins: Vec<String>,
    pub admission: AdmissionConfig,
    pub max_concurrent: usize,
    pub grace: Duration,
    pub stall: Duration,
    /// Seconds between reaping passes; 0 disables the timer.
    pub reconcile_secs: u64,
    /// The name recorded with each signature.
    pub operator: String,
    pub pr_host: PrHostChoice,
    /// The image of the in-process driver's agent container.
    pub legacy_image: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SettingsError {
    #[error("{0}")]
    Auth(String),
    #[error("the configuration file does not read: {0}")]
    Malformed(String),
    #[error("{key}: {problem}")]
    Invalid { key: &'static str, problem: String },
}

/// Read the settings. `file` is the text of the configuration file, if there is one; `env`
/// looks up an environment variable. With no file and no variable but the token, the result is
/// today's defaults: `127.0.0.1:8787`, `fleet.db`, one allowed repository (the sandbox), a $5
/// and 1800 second unit ceiling, $20 over 24 hours, three units at once.
///
/// The file:
///
/// ```toml
/// [caps]
/// unit_usd = 5.0
/// unit_wall_clock_secs = 1800
/// global_usd = 20.0
/// max_concurrent = 3
///
/// [[repo]]
/// slug = "adbarc92/command-center-agent-sandbox"
/// url = "https://github.com/adbarc92/command-center-agent-sandbox"
/// base_branch = "main"
/// ```
pub fn settings(
    file: Option<&str>,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<Settings, SettingsError> {
    let _ = (file, env);
    unimplemented!("{NEW}")
}

/// The implementations a daemon runs on. [`Parts::real`] chooses the real ones; a test passes
/// fakes.
pub struct Parts {
    pub store: Arc<SharedStore>,
    pub forge: Arc<dyn RepoForge>,
    pub runner: Arc<dyn CheckRunner>,
    /// The verifier for a unit run from a signed spec.
    pub verifier: Arc<dyn EvidenceVerifier>,
    pub registry: Registry,
    /// What a demo unit is verified and delivered with: a scripted verifier and the
    /// in-memory forge, because the scripted harness's evidence names nothing real.
    pub demo: fleet_supervisor::Deps,
}

impl Parts {
    /// SQLite at `settings.db`, `git` with the chosen pull-request host, Docker, the real
    /// verifier over those, the registry file merged over the scripted harness, and the
    /// demo doubles.
    pub fn real(settings: &Settings) -> Result<Parts, WireError> {
        let _ = settings;
        unimplemented!("{NEW}")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WireError {
    #[error("the store could not be opened: {0}")]
    Store(String),
    #[error("the harness registry could not be read: {0}")]
    Registry(String),
    #[error("the daemon could not bind {addr}: {error}")]
    Bind { addr: String, error: String },
}

/// A wired daemon, not yet listening.
pub struct Wired {
    /// Every route, behind the token. This is what [`serve`] serves.
    pub secured: Router,
    /// The same routes with no token check, for tests that go through the router directly.
    pub open: Router,
    pub hub: Hub,
    pub registry: Registry,
}

/// Join the parts: admission over the store, the forge and the registry; the API over all of
/// them; the launcher that starts a unit's run on the supervisor, or on the in-process driver
/// for a unit with no harness; and the routes the in-process path still owns (`/health`,
/// `/swarms`).
pub fn wire(settings: &Settings, parts: Parts) -> Result<Wired, WireError> {
    let _ = (settings, parts);
    unimplemented!("{NEW}")
}

/// Probe the registry, reap what a dead daemon left, start the reaping timer, bind and serve
/// until the process ends.
pub async fn serve(settings: Settings, parts: Parts) -> Result<(), WireError> {
    let _ = (settings, parts);
    unimplemented!("{NEW}")
}

// --- adapters: each joins a port of one crate to a type of another, and holds no rule ---

/// A supervisor [`Sink`] that reports into the API's [`UnitFeed`].
pub struct FeedSink(pub UnitFeed);

impl Sink for FeedSink {
    fn event(&mut self, event: Event) {
        let _ = event;
        unimplemented!("{NEW}")
    }

    fn stage(&mut self, stage: Stage, status: StageStatus, detail: Option<String>) {
        let _ = (stage, status, detail);
        unimplemented!("{NEW}")
    }

    fn frozen(&mut self, freeze: &OracleFreeze) {
        let _ = freeze;
        unimplemented!("{NEW}")
    }

    fn result(&mut self, result: &UnitResult) {
        let _ = result;
        unimplemented!("{NEW}")
    }
}

/// Admission's and the API's views of the supervisor's registry.
pub struct RegistryView(pub Registry);

impl HarnessCatalog for RegistryView {
    fn status(&self, name: &str) -> HarnessStatus {
        let _ = name;
        unimplemented!("{NEW}")
    }

    fn eligibility(
        &self,
        name: &str,
        tier: Tier,
        opt_in: bool,
        profile: Option<&str>,
    ) -> Result<(), String> {
        let _ = (name, tier, opt_in, profile);
        unimplemented!("{NEW}")
    }
}

impl HarnessDirectory for RegistryView {
    fn list(&self) -> Vec<HarnessView> {
        unimplemented!("{NEW}")
    }
}

/// The reaper's view of the store and the hub.
pub struct Census {
    pub store: Arc<SharedStore>,
    pub hub: Hub,
}

impl UnitCensus for Census {
    fn nonterminal(&self) -> Vec<String> {
        unimplemented!("{NEW}")
    }

    fn finished(&self) -> Vec<String> {
        unimplemented!("{NEW}")
    }

    fn live(&self) -> Vec<String> {
        unimplemented!("{NEW}")
    }

    fn stranded(&self, unit_id: &str) {
        let _ = unit_id;
        unimplemented!("{NEW}")
    }
}
````

- [ ] **Step 2: Declare the module**

In `crates/fleetd/src/lib.rs`, after the line `pub mod swarm;`, add:

```rust
pub mod wire;
```

- [ ] **Step 3: Create `tests/e2e/src/lib.rs`**

````rust
//! `fleet-e2e` — the hermetic end-to-end harness. It starts the real `fleetd` binary on a
//! temporary directory, points it at a local bare git repository that stands in for GitHub, and
//! drives it over HTTP exactly as the cockpit would.
//!
//! Two harnesses can run a unit here. `reqdrive` is the real one, at the tag `reqdrive.pin`
//! names, with its scripted agent. `harness-scripted` (a binary of this crate) is a harness
//! that does what a script tells it with real `git`, so a scenario can make a harness deliver
//! something a correct harness never would, and show that the control plane catches it.
//!
//! Wave 0 gives the signatures only; lane CC-E2E builds the bodies.

use serde_json::Value;
use std::path::PathBuf;
use std::time::Duration;

const NEW: &str = "lane CC-E2E builds this";

/// One end-to-end scenario and whether it gates. An unfrozen scenario runs and reports; a
/// frozen one fails the tier. A scenario is frozen only by the owner, after the functional
/// smoke has confirmed the behaviour it encodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Scenario {
    pub name: &'static str,
    pub frozen: bool,
}

/// Milestone 1's scenarios. Each is a test of the same name in `tests/e2e_m1.rs`.
pub const SCENARIOS: &[Scenario] = &[
    Scenario {
        name: "a_t1_unit_becomes_an_open_pull_request",
        frozen: false,
    },
    Scenario {
        name: "tests_that_fail_in_the_control_planes_run_stop_the_unit_and_can_be_reverified",
        frozen: false,
    },
    Scenario {
        name: "a_tampered_frozen_test_stops_the_unit_for_oracle_tampering",
        frozen: false,
    },
    Scenario {
        name: "a_spec_changed_after_signing_is_refused_at_admission",
        frozen: false,
    },
    Scenario {
        name: "an_unsigned_spec_is_refused",
        frozen: false,
    },
    Scenario {
        name: "a_change_outside_scope_fails_verification",
        frozen: false,
    },
    Scenario {
        name: "a_restart_mid_unit_reaps_the_container_and_the_unit_resumes",
        frozen: false,
    },
];

/// Run one scenario's body under its frozen flag: a failure of a frozen scenario panics; a
/// failure of an unfrozen one is printed as `UNFROZEN-FAIL <name>: <why>` and the test passes.
/// A name that is not in [`SCENARIOS`] panics.
pub async fn scenario<F>(name: &str, body: F)
where
    F: std::future::Future<Output = Result<(), String>>,
{
    let _ = (name, body);
    unimplemented!("{NEW}")
}

/// Which fixture repository a factory starts on. Each is a directory under `fixtures/`. A
/// fixture for the `cargo` preset arrives with the first scenario that needs one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fixture {
    /// `fixtures/node`: the `node` preset.
    Node,
}

/// What the scripted harness does with a unit. Every variant commits the signed spec first and
/// reports `pr_open` with honest hashes of what it froze; they differ in the work commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Script {
    /// A correct change: the frozen test and an implementation that passes it.
    Honest,
    /// The frozen test, and an implementation that fails it. The harness reports success.
    TestsFail,
    /// A correct change, then one more commit that edits the frozen test file.
    TampersWithFrozenTest,
    /// A correct change, plus a file the spec's scope does not name.
    LeavesScope,
}

/// Which harness runs the units of a factory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HarnessChoice {
    /// The `reqdrive` binary at the pinned tag, with its scripted agent on the honest path.
    Reqdrive,
    /// `harness-scripted`, following a script. It runs no container and meters nothing, so a
    /// unit dispatched to it needs the opt-in.
    Scripted(Script),
}

/// A running daemon and everything around it. Dropping it stops the daemon and removes its
/// directory.
pub struct Factory {
    #[allow(dead_code)] // read by the bodies lane CC-E2E builds
    dir: tempfile::TempDir,
}

impl Factory {
    /// Build the fixture into a bare repository, write the daemon's configuration and registry
    /// files, start `fleetd` and wait until it answers. `Err` says what is missing (Docker, the
    /// pinned `reqdrive` binary, the `fleetd` binary).
    pub async fn start(fixture: Fixture, harness: HarnessChoice) -> Result<Factory, String> {
        let _ = (fixture, harness);
        unimplemented!("{NEW}")
    }

    /// The registry name of the harness this factory was started with.
    pub fn harness_name(&self) -> &str {
        unimplemented!("{NEW}")
    }

    /// The repository's slug, as the daemon's allowlist has it.
    pub fn repo(&self) -> &str {
        unimplemented!("{NEW}")
    }

    /// The token the daemon was started with.
    pub fn token(&self) -> &str {
        unimplemented!("{NEW}")
    }

    /// `GET path` with the token. Returns the status and the body as JSON (`null` when empty).
    pub async fn get(&self, path: &str) -> (u16, Value) {
        let _ = path;
        unimplemented!("{NEW}")
    }

    /// `POST path` with the token and a JSON body.
    pub async fn post(&self, path: &str, body: Value) -> (u16, Value) {
        let _ = (path, body);
        unimplemented!("{NEW}")
    }

    /// `GET path` with no token.
    pub async fn get_without_token(&self, path: &str) -> u16 {
        let _ = path;
        unimplemented!("{NEW}")
    }

    /// Read the fixture's spec through `GET /specs` and sign it. Returns the signed hash.
    pub async fn sign(&self, spec_id: &str) -> Result<String, String> {
        let _ = spec_id;
        unimplemented!("{NEW}")
    }

    /// `POST /missions` for the fixture's work item and these spec bytes, on this factory's
    /// harness, with the opt-in when the harness needs it.
    pub async fn dispatch(&self, spec_sha256: &str, idempotency_key: &str) -> (u16, Value) {
        let _ = (spec_sha256, idempotency_key);
        unimplemented!("{NEW}")
    }

    /// Send a command to a unit. Returns the status.
    pub async fn command(&self, unit_id: &str, command: Value) -> u16 {
        let _ = (unit_id, command);
        unimplemented!("{NEW}")
    }

    /// Poll `GET /units/{id}/detail` until the unit's phase is one of `phases`, and return the
    /// detail. `Err` after `within`, with the unit's last detail and its events.
    pub async fn until_phase(
        &self,
        unit_id: &str,
        phases: &[&str],
        within: Duration,
    ) -> Result<Value, String> {
        let _ = (unit_id, phases, within);
        unimplemented!("{NEW}")
    }

    /// The unit's logged events.
    pub async fn events(&self, unit_id: &str) -> Vec<Value> {
        let _ = unit_id;
        unimplemented!("{NEW}")
    }

    /// The pull requests the daemon has opened, as the local host recorded them.
    pub fn pull_requests(&self) -> Vec<Value> {
        unimplemented!("{NEW}")
    }

    /// The commit a branch of the bare repository is at, if the branch exists.
    pub fn remote_branch(&self, branch: &str) -> Option<String> {
        let _ = branch;
        unimplemented!("{NEW}")
    }

    /// Commit a change to a file on the bare repository's base branch, as a person pushing.
    pub fn push_to_base(&self, path: &str, content: &str) -> String {
        let _ = (path, content);
        unimplemented!("{NEW}")
    }

    /// The names of the containers that carry this unit's label.
    pub fn containers(&self, unit_id: &str) -> Vec<String> {
        let _ = unit_id;
        unimplemented!("{NEW}")
    }

    /// Kill the daemon without warning and start it again on the same database and directory.
    pub async fn restart(&mut self) -> Result<(), String> {
        unimplemented!("{NEW}")
    }

    /// The directory the daemon runs in.
    pub fn dir(&self) -> PathBuf {
        unimplemented!("{NEW}")
    }
}
````

- [ ] **Step 4: Build, format, lint and run everything**

```
cargo fmt --all
cargo build --workspace --all-targets --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
```

Expected: `cargo fmt` changes nothing (the listings are already formatted); the build and
clippy exit 0. Then run every command in Part 1's Verify table and record the output.

- [ ] **Step 5: Commit**

```
git add crates/fleetd/src/wire.rs crates/fleetd/src/lib.rs tests/e2e/src/lib.rs
git commit -m "feat(fleetd): the wiring interface, and the end-to-end harness's interface"
```

### Task 10: Push and open the pull request

- [ ] **Step 1: Push**

```
git push -u origin feat/m1-interfaces
```

- [ ] **Step 2: Open the pull request**

Write the body to a file outside the worktree's tracked files (for example
`D:\MajorProjects\.swarm-wt\m1-interfaces-pr.md`) and do not commit it. It states: that this
is wave 0 of milestone 1 and adds interfaces, fakes and contract suites and no behaviour; the
seven crates and the two interface files; the output of each Verify command; that every
`unimplemented!` names the lane that removes it; and the plan's path. It carries no
"Generated with" footer.

```
gh pr create -R adbarc92/command-center --base factory/m1 --head feat/m1-interfaces --title "M1 wave 0: interfaces, fakes and contract suites for the control-plane crates" --body-file D:/MajorProjects/.swarm-wt/m1-interfaces-pr.md
```

- [ ] **Step 3: Watch the checks**

```
gh pr checks --watch
```

Expected: `static`, the `unit` matrix and `contract` green on both operating systems. Do not
merge.

---

# Part 2 — The lanes

Every lane's brief starts with the rules of the program plan, section 2, and this plan's Global
Constraints. Each lane cuts its branch from `origin/factory/m1` after Part 1 has merged:

```
git -C D:/MajorProjects/INFRASTRUCTURE/command-center fetch origin
git -C D:/MajorProjects/INFRASTRUCTURE/command-center worktree add --no-track -b <branch> <worktree> origin/factory/m1
```

and ends with `git push -u origin <branch>` and
`gh pr create -R adbarc92/command-center --base factory/m1 --head <branch> --title "<title>" --body-file <file outside the worktree>`.
The body says what changed, how it was verified (the real output of every **Verify** command),
which issue it closes, and what it asks of the coordinator. No lane merges.

Lanes CC-STORE, CC-FORGE, CC-RUNNER, CC-SUPERVISOR, CC-VERIFY, CC-ADMISSION and CC-API can all
run at the same time: each needs only Part 1. CC-WIRE needs all seven merged. CC-E2E's harness
can be built beside the seven; its scenarios go green after CC-WIRE.

In every lane, the first step is the same and is not repeated below:

- [ ] Create the test files this lane's **Behaviours** list, exactly as printed.
- [ ] Run the lane's test command and see every listed test fail with
      `not implemented: lane <LANE> …`. A test that fails for another reason, or passes, is a
      finding: stop and report.
- [ ] Build the implementation, one behaviour at a time, until the tests pass. Commit after
      each behaviour with a conventional subject (`feat(<crate>): …`).
- [ ] Run **Verify**, push, open the pull request.

---

## Lane CC-STORE

**Owns:** `crates/fleet-store/src/sqlite.rs`, `crates/fleet-store/tests/**`. It may add private
modules under `crates/fleet-store/src/`. It does not change `types.rs`, `traits.rs`,
`projection.rs`, `error.rs`, `lib.rs` or `testkit.rs` (Part 1's interface).

**Reads:** `crates/fleetd/src/store.rs` (the code to move); `crates/fleetd/tests/characterisation_store.rs`
and `characterisation_admission.rs` (what must not change); the supervisor spec, section 5;
`forms/fleet-store.md`; issue #74.

**Worktree and branch:** `D:\MajorProjects\.swarm-wt\m1-cc-store`, branch `feat/m1-fleet-store`.

**Needs:** Part 1 merged into `factory/m1`.

**Blocks:** CC-WIRE.

### Form draft: `fleet-store`

**Purpose.** Durable state for the control plane: an append-only log per unit, one row per
unit that is always the fold of its log, and the signatures people gave, each with the exact
bytes it binds. Without it a restart loses every unit and every reason one was waiting for a
person, the spend ceiling has no record to check, and a unit could run from bytes nobody signed.

**interface-files:** `crates/fleet-store/src/lib.rs`, `error.rs`, `types.rs`, `traits.rs`,
`projection.rs`, `sqlite.rs` (the public items of each).

**Interfaces** (the signatures of Part 1, Task 2; documentation and bodies are there):

````rust
// crates/fleet-store/src/error.rs
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StoreError {
    #[error("store failure: {0}")]
    Failed(String),
    #[error("signature hash {claimed} is not the hash of its bytes ({actual})")]
    HashMismatch { claimed: String, actual: String },
}

// crates/fleet-store/src/types.rs
pub fn sha256_hex(bytes: &[u8]) -> String;

#[derive(Debug, Clone, PartialEq)]
pub struct UnitRow {
    pub unit_id: String,
    pub tier: String,
    pub task: String,
    pub repo_url: String,
    pub repo_slug: String,
    pub base_branch: String,
    pub branch: String,
    pub test_cmd: String,
    pub usd_cap: f64,
    pub wall_clock_secs: u64,
    pub phase: String,
    pub cost: f64,
    pub last_seq: u64,
    pub oracle_frozen: bool,
    pub oracle_hash: Option<String>,
    pub terminal_reason: Option<String>,
    pub mode: String,
    pub min_review_rounds: u32,
    pub swarm_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct UnitExt {
    pub harness: Option<String>,
    pub approved_oracle_hash: Option<String>,
    pub elapsed_ms: u64,
    pub pause_reason: Option<String>,
    pub paused_from: Option<String>,
    pub work_item: Option<String>,
    pub spec_id: Option<String>,
    pub spec_sha256: Option<String>,
    pub base_sha: Option<String>,
    pub profile: Option<String>,
    pub opt_in: bool,
    pub idempotency_key: Option<String>,
    pub freeze_json: Option<String>,
    pub result_json: Option<String>,
    pub pr_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UnitRecord {
    pub row: UnitRow,
    pub ext: UnitExt,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Reservation {
    Reserved,
    OverCap { committed: f64 },
    DuplicateKey { unit_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Restarted {
    Halted { seq: u64 },
    KeptForHuman,
    Untouched,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature {
    pub repo_slug: String,
    pub spec_id: String,
    pub commit: String,
    pub sha256: String,
    pub bytes: Vec<u8>,
    pub signed_by: String,
    pub signed_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureSummary {
    pub repo_slug: String,
    pub spec_id: String,
    pub commit: String,
    pub sha256: String,
    pub signed_by: String,
    pub signed_at_ms: i64,
}

impl Signature {
    pub fn summary(&self) -> SignatureSummary;
}

#[derive(Debug, Clone, PartialEq)]
pub struct SwarmRow {
    pub swarm_id: String,
    pub repo_url: String,
    pub repo_slug: String,
    pub base_branch: String,
    pub doc_path: String,
    pub tier: String,
    pub mode: String,
    pub lane_cap: u32,
    pub usd_budget: f64,
    pub per_lane_cap: f64,
    pub status: String,
    pub planner_cost: f64,
    pub lanes_launched: u32,
    pub lanes_dropped: u32,
    pub min_review_rounds: u32,
    pub terminal_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaneRow {
    pub swarm_id: String,
    pub idx: u32,
    pub title: String,
    pub task: String,
    pub rationale: String,
    pub decision: String,
    pub unit_id: Option<String>,
}

// crates/fleet-store/src/traits.rs
pub trait UnitStore: Send + Sync {
    fn reserve_unit(
        &self,
        record: &UnitRecord,
        window_start_ms: i64,
        global_cap: f64,
        now_ms: i64,
    ) -> Result<Reservation, StoreError>;

    fn get_unit(&self, unit_id: &str) -> Result<Option<UnitRow>, StoreError>;
    fn get_record(&self, unit_id: &str) -> Result<Option<UnitRecord>, StoreError>;
    fn list_records(&self) -> Result<Vec<UnitRecord>, StoreError>;
    fn max_unit_seq(&self) -> Result<u64, StoreError>;
    fn find_by_idempotency_key(&self, key: &str) -> Result<Option<String>, StoreError>;

    fn committed_spend(&self, since_ms: i64) -> Result<f64, StoreError>;

    fn record_event(
        &self,
        unit_id: &str,
        seq: u64,
        ts_ms: i64,
        json: &str,
        event: Option<&Event>,
    ) -> Result<(), StoreError>;

    fn events_since(&self, unit_id: &str, since: u64) -> Result<Vec<String>, StoreError>;

    fn put_freeze(&self, unit_id: &str, freeze_json: &str, now_ms: i64) -> Result<(), StoreError>;
    fn put_result(&self, unit_id: &str, result_json: &str, now_ms: i64) -> Result<(), StoreError>;

    fn mark_restarted(&self, unit_id: &str, now_ms: i64) -> Result<Restarted, StoreError>;
}

pub trait SignatureStore: Send + Sync {
    fn put_signature(&self, signature: &Signature) -> Result<(), StoreError>;
    fn get_signature(&self, repo_slug: &str, sha256: &str)
        -> Result<Option<Signature>, StoreError>;
    fn list_signatures(&self, repo_slug: &str) -> Result<Vec<SignatureSummary>, StoreError>;
}

// crates/fleet-store/src/projection.rs
pub const REASON_ORACLE_TAMPERING: &str = "oracle tampering";

pub const REASON_DAEMON_RESTARTED: &str = "daemon restarted";

#[derive(Debug, Clone, PartialEq)]
pub struct Projection {
    pub phase: String,
    pub cost: f64,
    pub last_seq: u64,
    pub terminal_reason: Option<String>,
    pub oracle_hash: Option<String>,
    pub oracle_frozen: bool,
    pub approved_oracle_hash: Option<String>,
    pub elapsed_ms: u64,
    pub pause_reason: Option<String>,
    pub paused_from: Option<String>,
    pub pr_url: Option<String>,
}

impl Projection {
    pub fn of(record: &UnitRecord) -> Self;

    pub fn write_into(&self, record: &mut UnitRecord);

    pub fn fold(&mut self, seq: u64, event: Option<&Event>, harness_path: bool);
}

// crates/fleet-store/src/sqlite.rs
pub type SharedStore = Mutex<Store>;

pub struct Store {
    /* private */
}

impl Store {
    pub fn open(path: &Path) -> Result<Self, StoreError>;

    pub fn open_memory() -> Result<Self, StoreError>;

    pub fn upsert_unit(&self, row: &UnitRow, now: i64) -> Result<(), StoreError>;

    pub fn update_unit(
        &self,
        unit_id: &str,
        phase: &str,
        cost: f64,
        last_seq: u64,
        terminal_reason: Option<&str>,
        oracle_hash: Option<&str>,
        now: i64,
    ) -> Result<(), StoreError>;

    pub fn append_event(
        &self,
        unit_id: &str,
        seq: u64,
        ts: i64,
        json: &str,
    ) -> Result<(), StoreError>;

    pub fn get_unit(&self, id: &str) -> Result<Option<UnitRow>, StoreError>;

    pub fn list_units(&self) -> Result<Vec<UnitRow>, StoreError>;

    pub fn events_since(&self, id: &str, since: u64) -> Result<Vec<String>, StoreError>;

    pub fn max_swarm_seq(&self) -> Result<u64, StoreError>;

    pub fn max_unit_seq(&self) -> Result<u64, StoreError>;

    pub fn committed_spend(&self, since_ts: i64) -> Result<f64, StoreError>;

    pub fn upsert_swarm(&self, row: &SwarmRow, now: i64) -> Result<(), StoreError>;

    pub fn update_swarm(
        &self,
        id: &str,
        status: &str,
        planner_cost: f64,
        lanes_launched: u32,
        lanes_dropped: u32,
        terminal_reason: Option<&str>,
        now: i64,
    ) -> Result<(), StoreError>;

    pub fn get_swarm(&self, id: &str) -> Result<Option<SwarmRow>, StoreError>;

    pub fn list_swarms(&self) -> Result<Vec<SwarmRow>, StoreError>;

    pub fn swarm_rollup(&self, swarm_id: &str) -> Result<(u64, u64, u64), StoreError>;

    pub fn upsert_lane(
        &self,
        swarm_id: &str,
        idx: u32,
        title: &str,
        task: &str,
        rationale: &str,
        decision: &str,
        unit_id: Option<&str>,
    ) -> Result<(), StoreError>;

    pub fn lanes_for_swarm(&self, swarm_id: &str) -> Result<Vec<LaneRow>, StoreError>;

    pub fn commit_lane_unit(
        &self,
        swarm_id: &str,
        idx: u32,
        unit: &UnitRow,
        now: i64,
    ) -> Result<(), StoreError>;
}

impl Store {
    pub fn write_projection(
        &self,
        unit_id: &str,
        projection: &Projection,
        now: i64,
    ) -> Result<(), StoreError>;
}
````

**Invariants**

- I1. `events_since(unit, n)` returns exactly that unit's records with a sequence number above
  `n`, ascending, each byte for byte as it was recorded.
- I2. Recording a sequence number the unit's log already has changes nothing and is not an error.
- I3. After `record_event`, the unit's row is `Projection::fold` of its row before and that
  record; the append and the row write happen together or not at all.
- I4. A unit's set-once columns (`UnitRow`'s first ten fields, `mode`, `min_review_rounds`,
  `swarm_id`, and `UnitExt`'s `harness`, `work_item`, `spec_id`, `spec_sha256`, `base_sha`,
  `profile`, `opt_in`, `idempotency_key`) never change after the first write.
- I5. `committed_spend(since)` counts each unit first written at or after `since`: its cost if
  it is finished, otherwise the larger of its cap and its cost.
- I6. Of any number of concurrent `reserve_unit` calls, no more are `Reserved` than fit under
  the ceiling; a call that returns `Err`, `OverCap` or `DuplicateKey` leaves no row.
- I7. At most one unit holds a given idempotency key.
- I8. A stored signature's `sha256` is the SHA-256 of its `bytes`, and `get_signature` returns
  those bytes exactly.
- I9. `mark_restarted` leaves a unit in `needs_human` or `halted` exactly as it was; a running
  unit becomes `halted` with exactly one more record.
- I10. Everything written before the process exits is readable after the same file is opened
  again, including a file written before milestone 1.
- I11. `UnitRow` has exactly the nineteen fields `crates/fleetd/tests/characterisation_store.rs`
  builds it from.
- I12. No type of the storage engine appears in a public signature.
- I13. The crate depends on no workspace crate other than `fleet-core`, and declares a
  `testkit` feature.

**Hidden decisions.** The storage engine, its journal mode and file layout; table, column and
index names; how an older file is migrated; that one connection is used and how writes are
serialised; how a failed write is rolled back.

**Gates**

| id | guards | mechanism | location | command | blocks |
|---|---|---|---|---|---|
| workspace.G1 | I13 | Build graph: the dependency allowlist, which also requires the `testkit` feature | `xtask/src/deps.rs#pub const RULES` | `cargo xtask test static` | merge |

**Unenforced (planned).** Each becomes a registry row when this lane's pull request merges
(request R9).

| planned id | guards | mechanism | location | command | built by |
|---|---|---|---|---|---|
| fleet-store.G1 | I1, I2, I3, I5, I6, I7, I9 | Contract suite: `unit_store_contract` against SQLite | `crates/fleet-store/tests/contract.rs#the_sqlite_store_passes_the_unit_store_contract` | `cargo xtask test contract` | behaviour 1 |
| fleet-store.G2 | I8 | Contract suite: `signature_store_contract` against SQLite | `crates/fleet-store/tests/contract.rs#the_sqlite_store_passes_the_signature_store_contract` | `cargo xtask test contract` | behaviour 2 |
| fleet-store.G3 | I6 | Locked test: ten threads, one slot | `crates/fleet-store/tests/contract.rs#ten_threads_racing_for_the_last_slot_under_the_ceiling_admit_exactly_one` | `cargo xtask test contract` | behaviour 8 |
| fleet-store.G4 | I10 | Locked test: migrate and reopen | `crates/fleet-store/tests/contract.rs#a_database_from_before_milestone_one_is_migrated_in_place_and_keeps_its_rows` | `cargo xtask test contract` | behaviour 11 |
| fleet-store.G5 | I11 | Type system: the characterisation tests build `UnitRow` with a struct literal | `crates/fleet-store/tests/characterisation_store.rs` | `cargo xtask test integration` | behaviour 16 |

I4 and I12 have no gate planned: I4 is covered by the moved test `persists_mode_and_review_floor`
and by behaviour 6 for the extension, I12 by review of the interface files.

### Behaviours

All in `crates/fleet-store/tests/contract.rs`, printed below, except 16 and 17.

1. `the_sqlite_store_passes_the_unit_store_contract`: the SQLite store does everything the
   in-memory one does.
2. `the_sqlite_store_passes_the_signature_store_contract`.
3. `one_shared_store_coerces_to_both_seams_and_they_see_the_same_database`.
4. `a_unit_written_by_the_driver_path_reads_through_the_seam_with_an_empty_extension`: no row
   is moved to the harness path by being read.
5. `a_unit_reserved_through_the_seam_reads_through_the_driver_path_methods`.
6. `update_unit_leaves_the_extension_columns_alone`.
7. `write_projection_can_clear_a_column_that_update_unit_cannot`, and
   `write_projection_for_a_unit_that_does_not_exist_creates_nothing` (supervisor spec 5).
8. `ten_threads_racing_for_the_last_slot_under_the_ceiling_admit_exactly_one` (Review Focus 4).
9. `a_reservation_that_cannot_be_written_is_an_error_and_leaves_no_row` (issue #74).
10. `a_spend_read_that_fails_is_an_error_not_zero` (issue #74).
11. `a_database_from_before_milestone_one_is_migrated_in_place_and_keeps_its_rows`.
12. `a_file_backed_store_keeps_records_extensions_events_and_signatures_across_a_reopen`: this is
    also where the reason a unit needs a human is shown to survive a restart.
13. `an_event_whose_fold_cannot_be_written_is_not_logged_either`.
14. `an_event_for_a_unit_with_no_row_is_logged_and_folds_nothing` (today's behaviour, kept).
15. The moved unit tests: everything in `crates/fleetd/src/store.rs`'s `mod tests` (lines 483
    to 804), moved into `sqlite.rs` with its `use` lines changed and nothing else.
16. `crates/fleet-store/tests/characterisation_store.rs`: a copy of
    `crates/fleetd/tests/characterisation_store.rs` whose only difference is its `use` line,
    which becomes `use fleet_store::{Store, UnitRow};`. All eleven tests pass unchanged.
17. A copy of the two `defect_74_*` tests is **not** made here: they exercise `create_mission`,
    which lane CC-WIRE changes.

Run: `cargo test -p fleet-store --test contract`. Expected before the implementation: 15
failed, each with `not implemented: lane CC-STORE moves this from crates/fleetd/src/store.rs`.

`crates/fleet-store/tests/contract.rs`:

````rust
//! Lane CC-STORE: the SQLite store behind the interface of `forms/fleet-store.md`.
//!
//! No Docker, no network, no token. A test that needs a file uses a temporary directory.

use fleet_core::{Event, Phase};
use fleet_store::testkit::{
    envelope_json, harness_record, phase_changed, record, signature, signature_store_contract,
    unit_row, unit_store_contract,
};
use fleet_store::{
    Projection, Reservation, Restarted, SharedStore, SignatureStore, Store, UnitStore,
    REASON_DAEMON_RESTARTED,
};
use std::sync::{Arc, Mutex};

fn shared() -> SharedStore {
    Mutex::new(Store::open_memory().expect("an in-memory database opens"))
}

// --- 1. the contract suites, against SQLite ---

#[test]
fn the_sqlite_store_passes_the_unit_store_contract() {
    unit_store_contract(shared);
}

#[test]
fn the_sqlite_store_passes_the_signature_store_contract() {
    signature_store_contract(shared);
}

// --- 2. one object serves both seams ---

#[test]
fn one_shared_store_coerces_to_both_seams_and_they_see_the_same_database() {
    let store = Arc::new(shared());
    let units: Arc<dyn UnitStore> = store.clone();
    let signatures: Arc<dyn SignatureStore> = store.clone();
    units.reserve_unit(&record("u1"), 0, 20.0, 10).unwrap();
    signatures
        .put_signature(&signature("owner/repo", "SPEC-1", b"bytes"))
        .unwrap();
    assert!(store.lock().unwrap().get_unit("u1").unwrap().is_some());
    assert_eq!(units.list_records().unwrap().len(), 1);
}

// --- 3. the two paths share one table ---

#[test]
fn a_unit_written_by_the_driver_path_reads_through_the_seam_with_an_empty_extension() {
    let store = shared();
    store
        .lock()
        .unwrap()
        .upsert_unit(&unit_row("u1"), 10)
        .unwrap();
    let read = store.get_record("u1").unwrap().unwrap();
    assert_eq!(read, record("u1"));
    assert_eq!(
        read.ext.harness, None,
        "no backfill: a driver-path unit stays on the driver"
    );
}

#[test]
fn a_unit_reserved_through_the_seam_reads_through_the_driver_path_methods() {
    let store = shared();
    let mut unit = harness_record("u1", "reqdrive");
    unit.ext.work_item = Some("owner/repo#7".into());
    store.reserve_unit(&unit, 0, 20.0, 10).unwrap();
    let inner = store.lock().unwrap();
    assert_eq!(inner.get_unit("u1").unwrap(), Some(unit.row.clone()));
    assert_eq!(inner.list_units().unwrap(), [unit.row]);
    assert_eq!(inner.committed_spend(0).unwrap(), 5.0);
}

#[test]
fn update_unit_leaves_the_extension_columns_alone() {
    let store = shared();
    let mut unit = harness_record("u1", "reqdrive");
    unit.ext.pause_reason = Some("usd cap".into());
    unit.ext.freeze_json = Some("{}".into());
    store.reserve_unit(&unit, 0, 20.0, 10).unwrap();
    store
        .lock()
        .unwrap()
        .update_unit(
            "u1",
            "halted",
            1.0,
            4,
            Some(REASON_DAEMON_RESTARTED),
            None,
            20,
        )
        .unwrap();
    let after = store.get_record("u1").unwrap().unwrap();
    assert_eq!(after.row.phase, "halted");
    assert_eq!(after.ext, unit.ext);
}

// --- 4. write_projection writes every column as given ---

#[test]
fn write_projection_can_clear_a_column_that_update_unit_cannot() {
    let store = shared();
    let mut unit = harness_record("u1", "reqdrive");
    unit.row.oracle_hash = Some("h1".into());
    unit.row.oracle_frozen = true;
    unit.ext.approved_oracle_hash = Some("h1".into());
    store.reserve_unit(&unit, 0, 20.0, 10).unwrap();

    let mut projection = Projection::of(&unit);
    projection.oracle_hash = None;
    projection.oracle_frozen = false;
    projection.approved_oracle_hash = None;
    projection.phase = "spec".into();
    projection.last_seq = 9;
    store
        .lock()
        .unwrap()
        .write_projection("u1", &projection, 20)
        .unwrap();

    let after = store.get_record("u1").unwrap().unwrap();
    assert_eq!(Projection::of(&after), projection);
    assert_eq!(
        after.ext.harness.as_deref(),
        Some("reqdrive"),
        "set-once columns are untouched"
    );
}

#[test]
fn write_projection_for_a_unit_that_does_not_exist_creates_nothing() {
    let store = shared();
    let projection = Projection::of(&record("ghost"));
    store
        .lock()
        .unwrap()
        .write_projection("ghost", &projection, 20)
        .unwrap();
    assert_eq!(store.get_record("ghost").unwrap(), None);
}

// --- 5. the reservation is one atomic step (issue 74) ---

#[test]
fn ten_threads_racing_for_the_last_slot_under_the_ceiling_admit_exactly_one() {
    let store = Arc::new(shared());
    for id in ["a", "b", "c"] {
        store.reserve_unit(&record(id), 0, 20.0, 1).unwrap();
    }
    let handles: Vec<_> = (0..10)
        .map(|n| {
            let store = store.clone();
            std::thread::spawn(move || {
                store
                    .reserve_unit(&record(&format!("racer-{n}")), 0, 20.0, 2)
                    .unwrap()
            })
        })
        .collect();
    let outcomes: Vec<Reservation> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    let reserved = outcomes
        .iter()
        .filter(|o| **o == Reservation::Reserved)
        .count();
    assert_eq!(reserved, 1, "{outcomes:?}");
    assert_eq!(store.committed_spend(0).unwrap(), 20.0);
    assert_eq!(store.list_records().unwrap().len(), 4);
}

#[test]
fn a_reservation_that_cannot_be_written_is_an_error_and_leaves_no_row() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("fleet.db");
    let store = Mutex::new(Store::open(&path).unwrap());
    // A second connection makes the insert fail, as the characterisation test for issue 74 does.
    let side = rusqlite::Connection::open(&path).unwrap();
    side.execute_batch(
        "CREATE TRIGGER refuse_unit_rows BEFORE INSERT ON units
             BEGIN SELECT RAISE(ABORT, 'insert refused'); END;",
    )
    .unwrap();
    assert!(store.reserve_unit(&record("u1"), 0, 20.0, 1).is_err());
    assert_eq!(store.get_record("u1").unwrap(), None);
    assert_eq!(store.committed_spend(0).unwrap(), 0.0);
}

#[test]
fn a_spend_read_that_fails_is_an_error_not_zero() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("fleet.db");
    let store = Mutex::new(Store::open(&path).unwrap());
    let side = rusqlite::Connection::open(&path).unwrap();
    side.execute_batch("DROP TABLE swarms;").unwrap();
    assert!(store.committed_spend(0).is_err());
    assert!(
        store.reserve_unit(&record("u1"), 0, 20.0, 1).is_err(),
        "a reservation that cannot read the spend refuses"
    );
    assert_eq!(store.get_record("u1").unwrap(), None);
}

// --- 6. an older database gains the new columns and tables ---

#[test]
fn a_database_from_before_milestone_one_is_migrated_in_place_and_keeps_its_rows() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("old.db");
    {
        let old = rusqlite::Connection::open(&path).unwrap();
        old.execute_batch(
            "CREATE TABLE units(
               unit_id TEXT PRIMARY KEY, tier TEXT, task TEXT, repo_url TEXT, repo_slug TEXT,
               base_branch TEXT, branch TEXT, test_cmd TEXT, usd_cap REAL, wall_clock_secs INTEGER,
               phase TEXT, cost REAL, last_seq INTEGER, oracle_frozen INTEGER, oracle_hash TEXT,
               created_ts INTEGER, updated_ts INTEGER, terminal_reason TEXT,
               mode TEXT NOT NULL DEFAULT 'demo', min_review_rounds INTEGER NOT NULL DEFAULT 2,
               swarm_id TEXT);
             CREATE TABLE events(unit_id TEXT, seq INTEGER, ts INTEGER, json TEXT,
               PRIMARY KEY(unit_id, seq));
             INSERT INTO units(unit_id,tier,task,repo_url,repo_slug,base_branch,branch,test_cmd,
               usd_cap,wall_clock_secs,phase,cost,last_seq,oracle_frozen,created_ts,updated_ts,mode,
               min_review_rounds)
             VALUES('u1','t2','old task','https://example.invalid/r','o/r','main','agent/u1',
               'node --test',5.0,1800,'needs_human',1.5,7,1,100,100,'real',3);
             INSERT INTO events VALUES('u1', 7, 100, '{\"old\":true}');",
        )
        .unwrap();
    }
    for _ in 0..2 {
        // Opening twice: the migration is idempotent.
        let store = Mutex::new(Store::open(&path).unwrap());
        let unit = store.get_record("u1").unwrap().unwrap();
        assert_eq!(unit.row.task, "old task");
        assert_eq!(unit.row.phase, "needs_human");
        assert_eq!(unit.row.mode, "real");
        assert_eq!(unit.ext, Default::default());
        assert_eq!(store.events_since("u1", 0).unwrap(), ["{\"old\":true}"]);
        store
            .put_signature(&signature("o/r", "SPEC-1", b"bytes"))
            .unwrap();
    }
}

// --- 7. durability ---

#[test]
fn a_file_backed_store_keeps_records_extensions_events_and_signatures_across_a_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("fleet.db");
    let mut unit = harness_record("u1", "reqdrive");
    unit.ext.idempotency_key = Some("key-1".into());
    unit.ext.spec_sha256 = Some("ab".repeat(32));
    let signed = signature("owner/repo", "SPEC-1", b"\xEF\xBB\xBFsigned\r\n");
    let event = phase_changed(Phase::Building, Phase::NeedsHuman, Some("oracle tampering"));
    {
        let store = Mutex::new(Store::open(&path).unwrap());
        store.reserve_unit(&unit, 0, 20.0, 10).unwrap();
        store
            .record_event("u1", 1, 11, &envelope_json("u1", 1, &event), Some(&event))
            .unwrap();
        store.put_freeze("u1", "{\"frozen_files\":[]}", 12).unwrap();
        store.put_signature(&signed).unwrap();
    }
    let store = Mutex::new(Store::open(&path).unwrap());
    let read = store.get_record("u1").unwrap().unwrap();
    assert_eq!(read.row.phase, "needs_human");
    assert_eq!(read.ext.pause_reason.as_deref(), Some("oracle tampering"));
    assert_eq!(read.ext.paused_from.as_deref(), Some("building"));
    assert_eq!(
        read.ext.freeze_json.as_deref(),
        Some("{\"frozen_files\":[]}")
    );
    assert_eq!(
        store.find_by_idempotency_key("key-1").unwrap().as_deref(),
        Some("u1")
    );
    assert_eq!(store.events_since("u1", 0).unwrap().len(), 1);
    assert_eq!(
        store.get_signature("owner/repo", &signed.sha256).unwrap(),
        Some(signed)
    );
    // The reason survives a restart's reconciliation.
    assert_eq!(
        store.mark_restarted("u1", 20).unwrap(),
        Restarted::KeptForHuman
    );
    assert_eq!(
        store
            .get_record("u1")
            .unwrap()
            .unwrap()
            .ext
            .pause_reason
            .as_deref(),
        Some("oracle tampering")
    );
}

// --- 8. a record and its fold are one transaction ---

#[test]
fn an_event_whose_fold_cannot_be_written_is_not_logged_either() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("fleet.db");
    let store = Mutex::new(Store::open(&path).unwrap());
    store
        .reserve_unit(&harness_record("u1", "reqdrive"), 0, 20.0, 1)
        .unwrap();
    let side = rusqlite::Connection::open(&path).unwrap();
    side.execute_batch(
        "CREATE TRIGGER refuse_unit_updates BEFORE UPDATE ON units
             BEGIN SELECT RAISE(ABORT, 'update refused'); END;",
    )
    .unwrap();
    let event = Event::Metric {
        tokens_in: 1,
        tokens_out: 1,
        cost_usd: 0.5,
        elapsed_ms: 10,
    };
    assert!(store
        .record_event("u1", 1, 5, &envelope_json("u1", 1, &event), Some(&event))
        .is_err());
    assert_eq!(store.events_since("u1", 0).unwrap(), Vec::<String>::new());
    assert_eq!(store.get_unit("u1").unwrap().unwrap().cost, 0.0);
}

#[test]
fn an_event_for_a_unit_with_no_row_is_logged_and_folds_nothing() {
    let store = shared();
    let event = phase_changed(Phase::Queued, Phase::Provisioning, None);
    store
        .record_event(
            "ghost",
            1,
            5,
            &envelope_json("ghost", 1, &event),
            Some(&event),
        )
        .unwrap();
    assert_eq!(store.events_since("ghost", 0).unwrap().len(), 1);
    assert_eq!(store.get_record("ghost").unwrap(), None);
}
````

### Implementation notes

**What moves, and what must not change while it moves.** From `crates/fleetd/src/store.rs`:

| Lines | What | Note |
|---|---|---|
| 42–85 | `open`, `open_memory`, `init`: the pragmas, the four `CREATE TABLE`s, the idempotent `ALTER` list | Keep `Connection::open(path)` with its default flags: the characterisation tests open `file:…?mode=memory&cache=shared`, which only works while URI file names are on |
| 87–176 | `upsert_unit`, `update_unit`, `append_event`, `get_unit`, `list_units`, `events_since` | The SQL text does not change. `update_unit` keeps writing `terminal_reason` as given, `None` included |
| 178–225 | `max_swarm_seq`, `max_unit_seq`, `committed_spend` | The terminal list still comes from `fleet_core::TERMINAL_PHASE_STRS` |
| 227–255 | `map_row` and the four `SELECT` constants | |
| 257–481 | The swarm and lane methods, `commit_lane_unit` | |
| 483–804 | `mod tests` | Moves with the code |

`crates/fleetd/src/store.rs` itself is not edited by this lane (lane CC-WIRE turns it into a
re-export), so for a while the code exists twice.

The one deliberate change to moved code is the error type: every `rusqlite::Result<T>` becomes
`Result<T, StoreError>`, by `map_err(|e| StoreError::Failed(e.to_string()))`. Callers only ever
`unwrap`, `ok` or default the result.

**New columns** go into the existing ignore-if-exists `ALTER TABLE` list, so an old file
gains them when it is opened. None is in the `SELECT` constants the driver-path methods use:

```sql
ALTER TABLE units ADD COLUMN harness TEXT;
ALTER TABLE units ADD COLUMN approved_oracle_hash TEXT;
ALTER TABLE units ADD COLUMN elapsed_ms INTEGER NOT NULL DEFAULT 0;
ALTER TABLE units ADD COLUMN pause_reason TEXT;
ALTER TABLE units ADD COLUMN paused_from TEXT;
ALTER TABLE units ADD COLUMN work_item TEXT;
ALTER TABLE units ADD COLUMN spec_id TEXT;
ALTER TABLE units ADD COLUMN spec_sha256 TEXT;
ALTER TABLE units ADD COLUMN base_sha TEXT;
ALTER TABLE units ADD COLUMN profile TEXT;
ALTER TABLE units ADD COLUMN opt_in INTEGER NOT NULL DEFAULT 0;
ALTER TABLE units ADD COLUMN idempotency_key TEXT;
ALTER TABLE units ADD COLUMN freeze_json TEXT;
ALTER TABLE units ADD COLUMN result_json TEXT;
ALTER TABLE units ADD COLUMN pr_url TEXT;
CREATE UNIQUE INDEX IF NOT EXISTS idx_units_idempotency
  ON units(idempotency_key) WHERE idempotency_key IS NOT NULL;
CREATE TABLE IF NOT EXISTS signatures(
  repo_slug TEXT NOT NULL, sha256 TEXT NOT NULL, spec_id TEXT NOT NULL,
  commit_sha TEXT NOT NULL, bytes BLOB NOT NULL,
  signed_by TEXT NOT NULL, signed_at_ms INTEGER NOT NULL,
  PRIMARY KEY(repo_slug, sha256));
```

The supervisor spec's backfill (`harness = 'fake'` for stored demo rows) is **not** run: a
stored demo row keeps no harness and finishes on the driver path (Global Constraints, "Two run
paths"). The old test fixture `fleetd` uses to seed demo rows therefore still means what it did.

**The single writer.** One connection, behind the `Mutex` of `SharedStore`. Every method of
the two seams locks, does its work and unlocks; none holds the lock across anything that can
wait. The three multi-statement operations are transactions, in the style
`commit_lane_unit` already uses (`BEGIN IMMEDIATE` … `COMMIT`, `ROLLBACK` on any error):

- `reserve_unit`: look up the idempotency key; read `committed_spend`; insert the row with its
  extension columns. An error from the spend read is returned as `Err`: it is never treated as
  zero.
- `record_event`: `INSERT OR IGNORE` the record; if a row was inserted and the unit exists,
  read the unit's `UnitRecord`, `Projection::of` it, `fold`, and `write_projection`. If the
  insert was ignored, do nothing more.
- `mark_restarted`: read the phase; for a running unit append the synthetic record
  (`testkit::envelope_json` shows its exact shape), fold it, and then set `terminal_reason` to
  `REASON_DAEMON_RESTARTED`, which is what the reconcile step has always written.

`BEGIN IMMEDIATE` matters: a deferred transaction that reads and then writes can fail with
`SQLITE_BUSY` when a second connection wrote in between, and the retry would re-read. Tests 9,
10 and 13 use a second connection to the same file for exactly this reason.

**Pitfalls.** A `Mutex` poisoned by a panicking test thread must not poison the store for
good: recover the guard with `unwrap_or_else(|e| e.into_inner())`. `list_records` orders by
`rowid`, which is insertion order. SQLite has no boolean: `opt_in` and `oracle_frozen` are
`INTEGER`. `commit` is a keyword, hence `commit_sha`. A `u64` sequence number is stored as
`i64`; today's code casts, keep the casts.

### Verify

| Command | Expected |
|---|---|
| `cargo test -p fleet-store` | every target passes: the lib tests (14 from Part 1 plus the moved ones), `contract` (15), `characterisation_store` (11) |
| `cargo test -p fleet-store --features testkit` | the same |
| `cargo test -p fleetd --test characterisation_store --test characterisation_admission` | 11 and 7 passed: `fleetd` is untouched by this lane |
| `cargo xtask test static --root-only` | exit 0 |
| `cargo xtask test unit` and `cargo xtask test contract` and `cargo xtask test integration` | exit 0 |
| *Git-Bash:* `grep -rn "unimplemented!" crates/fleet-store/src` | no output |
| *Git-Bash:* `diff <(sed 1,16d crates/fleetd/tests/characterisation_store.rs) <(sed 1,16d crates/fleet-store/tests/characterisation_store.rs)` | no output: the two files differ only in their `use` line (line 16) |
| `git diff --name-only origin/factory/m1...HEAD` | only paths under **Owns** |

### Done when

- [ ] Behaviours 1 to 16 pass; no `unimplemented!` is left in `crates/fleet-store/src`.
- [ ] The characterisation copy differs from its original by one line.
- [ ] No public signature names a `rusqlite` type.
- [ ] The pull request names the registry rows it wants (the planned gates above) and says
      `Closes` nothing: issue #74 closes with lane CC-WIRE, which removes the last caller of
      the unchecked insert.

---

## Lane CC-FORGE

**Owns:** `crates/fleet-forge/src/git.rs`, the `impl Forge for GhForge` block in
`crates/fleet-forge/src/legacy.rs`, `crates/fleet-forge/tests/**`. It may add private modules
under `crates/fleet-forge/src/`. It does not change `factory.rs`, `lib.rs`, `testkit.rs`, or
the types and the trait in `legacy.rs`.

**Reads:** `crates/fleetd/src/forge.rs` and `gh_forge.rs` (the code to move);
`crates/fleetd/tests/characterisation_forge.rs`; `forms/fleet-forge.md`; issues #86 and #90 (the
link check and the pull-request body).

**Worktree and branch:** `D:\MajorProjects\.swarm-wt\m1-cc-forge`, branch `feat/m1-fleet-forge`.

**Needs:** Part 1 merged. `git` 2.38 or later on the build host (`git merge-tree --write-tree`).

**Blocks:** CC-WIRE; CC-E2E's harness (which uses `GitForge<LocalPrHost>`).

### Form draft: `fleet-forge`

**Purpose.** Everything the control plane does with `git` and with the pull-request host. It
reads a repository at a commit, cuts the bundle a harness starts from, imports the bundle a
harness delivers into a clone that can reach nothing, and pushes and opens the pull request
from a separate clone that holds the credential. Without it a harness would need a repository
credential, and the commit that reaches a pull request could differ from the one verified.

**interface-files:** `crates/fleet-forge/src/lib.rs`, `legacy.rs`, `factory.rs`, `git.rs`.

**Interfaces** (the signatures of Part 1, Task 3):

````rust
// crates/fleet-forge/src/legacy.rs
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MergeResult {
    Clean,
    Conflict,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mergeability {
    Mergeable,
    Dirty,
    Pending,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ForgeError {
    #[error("forge failure: {0}")]
    Failed(String),
}

#[async_trait]
pub trait Forge: Send + Sync {
    async fn trial_merge(&self, bundle: &Path, branch: &str) -> Result<MergeResult, ForgeError>;
    async fn open_pr(&self, branch: &str) -> Result<String, ForgeError>;
    async fn poll_mergeable(&self, pr_url: &str) -> Result<Mergeability, ForgeError>;
}

pub struct GhForge {
    /* private */
}

impl GhForge {
    pub fn new(
        repo_url: impl Into<String>,
        repo_slug: impl Into<String>,
        base_branch: impl Into<String>,
        host_clone: PathBuf,
        title: impl Into<String>,
    ) -> Self;
}

// crates/fleet-forge/src/factory.rs
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RepoRef {
    pub slug: String,
    pub url: String,
    pub base_branch: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkItemRef {
    pub repo_slug: String,
    pub number: u64,
    pub fingerprint: Option<String>,
}

impl WorkItemRef {
    pub fn parse(reference: &str, fingerprint: Option<&str>) -> Result<Self, ForgeError>;

    pub fn reference(&self) -> String;
}

pub fn pr_body(work_item: &WorkItemRef, pr_repo_slug: &str, summary: &str) -> String;

pub fn body_carries(body: &str, work_item: &WorkItemRef, pr_repo_slug: &str) -> bool;

pub fn safe_branch(name: &str) -> bool;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrRequest {
    pub repo: RepoRef,
    pub unit_id: String,
    pub branch: String,
    pub head_sha: String,
    pub title: String,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrRef {
    pub url: String,
    pub number: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrLifecycle {
    Open,
    Merged,
    Closed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrState {
    pub lifecycle: PrLifecycle,
    pub mergeable: Mergeability,
    pub body: String,
    pub head_sha: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ChangeKind {
    Added,
    Modified,
    Deleted,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Change {
    pub path: String,
    pub kind: ChangeKind,
}

#[async_trait]
pub trait RepoForge: Send + Sync {
    async fn resolve_base(&self, repo: &RepoRef) -> Result<String, ForgeError>;

    async fn read_file(
        &self,
        repo: &RepoRef,
        commit: &str,
        path: &str,
    ) -> Result<Option<Vec<u8>>, ForgeError>;

    async fn list_files(
        &self,
        repo: &RepoRef,
        commit: &str,
        prefix: &str,
    ) -> Result<Vec<String>, ForgeError>;

    async fn source_bundle(
        &self,
        repo: &RepoRef,
        base_sha: &str,
        dest: &Path,
    ) -> Result<(), ForgeError>;

    async fn import_bundle(
        &self,
        repo: &RepoRef,
        unit_id: &str,
        base_sha: &str,
        bundle: &Path,
        branch: &str,
    ) -> Result<Box<dyn Scratch>, ForgeError>;

    async fn open_pr(&self, request: &PrRequest) -> Result<PrRef, ForgeError>;

    async fn pr_state(&self, repo: &RepoRef, pr_url: &str) -> Result<PrState, ForgeError>;
}

#[async_trait]
pub trait Scratch: Send + Sync {
    fn base_sha(&self) -> &str;
    fn head_sha(&self) -> &str;
    async fn commits(&self) -> Result<Vec<String>, ForgeError>;
    async fn changes(&self, from: &str, to: &str) -> Result<Vec<Change>, ForgeError>;
    async fn read(&self, commit: &str, path: &str) -> Result<Option<Vec<u8>>, ForgeError>;
    async fn export_tree(&self, commit: &str, dest: &Path) -> Result<(), ForgeError>;
    async fn trial_merge(&self) -> Result<MergeResult, ForgeError>;
}

// crates/fleet-forge/src/git.rs
#[async_trait]
pub trait PrHost: Send + Sync {
    async fn create(&self, request: &PrRequest) -> Result<PrRef, ForgeError>;
    async fn view(&self, repo: &RepoRef, pr_url: &str) -> Result<PrState, ForgeError>;
}

#[derive(Debug, Default, Clone)]
pub struct GhCli;

#[derive(Debug, Clone)]
pub struct LocalPrHost {
    /* private */
}

impl LocalPrHost {
    pub fn new(dir: PathBuf) -> Self;

    pub fn record_path(&self, number: u64) -> PathBuf;
}

pub struct GitForge<H: PrHost> {
    /* private */
}

impl<H: PrHost> GitForge<H> {
    pub fn new(root: PathBuf, host: H) -> Self;

    pub fn scratch_dir(&self, unit_id: &str) -> PathBuf;

    pub fn push_clone_dir(&self, repo: &RepoRef) -> PathBuf;
}
````

**Invariants**

- I1. `read_file` and `Scratch::read` return a file's bytes exactly as committed: no
  line-ending conversion, no re-encoding.
- I2. A scratch clone has no remote and no credential helper, and no hook runs during any
  operation on it.
- I3. A unit's scratch directory and a repository's push clone are different directories,
  neither inside the other, both under the forge's root; no caller-supplied name becomes a
  path segment unsanitised.
- I4. `open_pr` pushes exactly `request.head_sha` to `refs/heads/<request.branch>` and nothing
  else, and refuses when the unit's imported branch is at any other commit.
- I5. `changes` reports a rename as a `Deleted` and an `Added`, and every path unquoted, from
  the repository root, with `/`.
- I6. `import_bundle` refuses a bundle that does not hold the branch, a branch that does not
  descend from the stated base, and a branch name `safe_branch` refuses.
- I7. `trial_merge` changes no branch and no working tree, in either clone.
- I8. For every work item, repository and summary, `body_carries(&pr_body(w, r, s), w, r)` is
  true; a body without both whole lines is not carried.
- I9. `Forge` and `GhForge` behave as they did in `fleetd` (the characterisation tests).
- I10. Nothing from a request reaches a `git` or `gh` command line without being validated:
  branch names by `safe_branch`, commits as 40 or 64 hex digits, paths as repository-relative
  with no `..` segment.
- I11. The crate depends on no other workspace crate, and declares a `testkit` feature.

**Hidden decisions.** The layout under the forge's root; which `git` plumbing each operation
uses; how a bundle's ref is named inside this crate; how the pull-request host is spoken to
(`gh` today); the record format of `LocalPrHost` beyond the four fields a test edits.

**Gates**

| id | guards | mechanism | location | command | blocks |
|---|---|---|---|---|---|
| workspace.G1 | I11 | Build graph: the dependency allowlist and the `testkit` feature | `xtask/src/deps.rs#pub const RULES` | `cargo xtask test static` | merge |

**Unenforced (planned)**

| planned id | guards | mechanism | location | command | built by |
|---|---|---|---|---|---|
| fleet-forge.G1 | I1, I4, I5, I6 | Contract suite: `repo_forge_contract` against `git` | `crates/fleet-forge/tests/git_it.rs#the_git_forge_passes_the_repo_forge_contract` | `cargo xtask test integration` | behaviour 6 |
| fleet-forge.G2 | I2, I7 | Locked test: a planted hook does not run | `crates/fleet-forge/tests/git_it.rs#a_hook_planted_in_the_scratch_clone_does_not_run_when_the_tree_is_exported` | `cargo xtask test integration` | behaviour 8 |
| fleet-forge.G3 | I3 | Locked test: the two directories | `crates/fleet-forge/tests/contract.rs#a_units_scratch_clone_and_the_push_clone_are_separate_directories_under_the_root` | `cargo xtask test contract` | behaviour 5 |
| fleet-forge.G4 | I8 | Unit tests of `pr_body` and `body_carries` (exist since Part 1) | `crates/fleet-forge/src/factory.rs#the_body_closes_the_issue_and_ends_with_the_trailer` | `cargo xtask test unit` | Part 1, Task 3 |

I9 is guarded by `crates/fleetd/tests/characterisation_forge.rs`, which is `fleetd`'s. I10 has
no gate beyond behaviours 1, 6 and 12.

### Behaviours

In `crates/fleet-forge/tests/contract.rs` (contract tier: no `git`):

1. `the_moved_forge_refuses_an_unsafe_branch_name_without_starting_a_process`.
2. `the_local_host_records_a_pull_request_as_a_file_and_reads_it_back`.
3. `editing_a_recorded_pull_request_changes_what_the_local_host_reports`: this is how the
   end-to-end suite stands in for a person merging on GitHub.
4. `the_local_host_numbers_on_from_the_records_already_in_its_directory`.
5. `a_units_scratch_clone_and_the_push_clone_are_separate_directories_under_the_root`.

In `crates/fleet-forge/tests/git_it.rs` (integration tier: needs `git`; no network, no
credential):

6. `the_git_forge_passes_the_repo_forge_contract`.
7. `a_scratch_clone_has_no_remote_no_credential_helper_and_hooks_disabled`.
8. `a_hook_planted_in_the_scratch_clone_does_not_run_when_the_tree_is_exported` (Review Focus 5).
9. `opening_a_pull_request_pushes_exactly_the_verified_commit_to_the_remote`.
10. `a_second_import_replaces_the_first_and_only_its_head_can_be_opened`.
11. `a_source_bundle_clones_to_the_tree_at_the_base_commit`.
12. `paths_with_spaces_and_non_ascii_names_read_and_export_unchanged`, and
    `a_path_that_climbs_out_of_the_repository_is_refused` (Review Focus 5).
13. The moved unit tests of `crates/fleetd/src/gh_forge.rs` (lines 194 to 212), in a private
    module beside the moved code.

Run: `cargo test -p fleet-forge --test contract --test git_it`. Expected before the
implementation: 5 and 8 failed, with `not implemented: lane CC-FORGE builds this` (and, for
behaviour 1, `… moves this from crates/fleetd/src/gh_forge.rs`). The fixture in `git_it.rs`
was run against real `git` on the build host and makes its bare repository, its commits and its
bundles correctly; a failure before the forge is called is the host's `git`, not the test.

`crates/fleet-forge/tests/contract.rs`:

````rust
//! Lane CC-FORGE: what the forge does without `git` and without a network.

use fleet_forge::testkit::{pr_request, repo};
use fleet_forge::{
    Forge, ForgeError, GhForge, GitForge, LocalPrHost, Mergeability, PrHost, PrLifecycle,
};
use std::path::Path;

fn message(error: ForgeError) -> String {
    error.to_string()
}

// --- 1. the moved three-call forge still refuses an unsafe branch before it runs anything ---

#[tokio::test]
async fn the_moved_forge_refuses_an_unsafe_branch_name_without_starting_a_process() {
    let dir = tempfile::tempdir().unwrap();
    let forge = GhForge::new(
        "https://example.invalid/owner/repo",
        "owner/repo",
        "main",
        dir.path().join("never-cloned"),
        "title",
    );
    for branch in ["--upload-pack=evil", "", "a..b", "a b"] {
        let refused = forge.open_pr(branch).await.unwrap_err();
        assert!(
            message(refused).contains("unsafe branch name"),
            "{branch:?}"
        );
        let refused = forge
            .trial_merge(Path::new("x.bundle"), branch)
            .await
            .unwrap_err();
        assert!(
            message(refused).contains("unsafe branch name"),
            "{branch:?}"
        );
    }
    assert!(
        !dir.path().join("never-cloned").exists(),
        "nothing was cloned"
    );
}

// --- 2. the local pull-request host ---

#[tokio::test]
async fn the_local_host_records_a_pull_request_as_a_file_and_reads_it_back() {
    let dir = tempfile::tempdir().unwrap();
    let host = LocalPrHost::new(dir.path().join("prs"));
    let request = pr_request("u1", "factory/7-u1", &"a".repeat(40));
    let first = host.create(&request).await.unwrap();
    assert_eq!(first.number, 1);
    assert!(host.record_path(1).is_file());
    let state = host.view(&repo(), &first.url).await.unwrap();
    assert_eq!(state.lifecycle, PrLifecycle::Open);
    assert_eq!(state.mergeable, Mergeability::Mergeable);
    assert_eq!(state.body, request.body);
    assert_eq!(state.head_sha, request.head_sha);

    let second = host
        .create(&pr_request("u2", "factory/7-u2", &"b".repeat(40)))
        .await
        .unwrap();
    assert_eq!(second.number, 2);
    assert_ne!(second.url, first.url);
}

#[tokio::test]
async fn editing_a_recorded_pull_request_changes_what_the_local_host_reports() {
    let dir = tempfile::tempdir().unwrap();
    let host = LocalPrHost::new(dir.path().join("prs"));
    let opened = host
        .create(&pr_request("u1", "factory/7-u1", &"a".repeat(40)))
        .await
        .unwrap();
    let path = host.record_path(opened.number);
    let mut record: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    record["lifecycle"] = "merged".into();
    record["mergeable"] = "dirty".into();
    std::fs::write(&path, serde_json::to_vec_pretty(&record).unwrap()).unwrap();
    let state = host.view(&repo(), &opened.url).await.unwrap();
    assert_eq!(state.lifecycle, PrLifecycle::Merged);
    assert_eq!(state.mergeable, Mergeability::Dirty);
}

#[tokio::test]
async fn the_local_host_numbers_on_from_the_records_already_in_its_directory() {
    let dir = tempfile::tempdir().unwrap();
    let prs = dir.path().join("prs");
    let opened = LocalPrHost::new(prs.clone())
        .create(&pr_request("u1", "factory/7-u1", &"a".repeat(40)))
        .await
        .unwrap();
    // A new host over the same directory: a restarted daemon.
    let host = LocalPrHost::new(prs);
    let next = host
        .create(&pr_request("u2", "factory/7-u2", &"b".repeat(40)))
        .await
        .unwrap();
    assert_eq!(next.number, opened.number + 1);
    assert!(host
        .view(&repo(), "https://example.invalid/pr/999")
        .await
        .is_err());
}

// --- 3. the two clones never share a directory ---

#[test]
fn a_units_scratch_clone_and_the_push_clone_are_separate_directories_under_the_root() {
    let dir = tempfile::tempdir().unwrap();
    let forge = GitForge::new(
        dir.path().to_path_buf(),
        LocalPrHost::new(dir.path().join("prs")),
    );
    let scratch = forge.scratch_dir("u1");
    let push = forge.push_clone_dir(&repo());
    assert!(scratch.starts_with(dir.path()) && push.starts_with(dir.path()));
    assert!(!scratch.starts_with(&push) && !push.starts_with(&scratch));
    assert_ne!(forge.scratch_dir("u2"), scratch);
    // A unit id is never used as a path without being made safe.
    let hostile = forge.scratch_dir("../../etc");
    assert!(hostile.starts_with(dir.path()));
    assert!(!hostile.components().any(|c| c.as_os_str() == ".."));
}
````

`crates/fleet-forge/tests/git_it.rs`:

````rust
//! Lane CC-FORGE, integration tier: the `git` forge against a local bare repository.
//!
//! Needs `git` on `PATH`. Needs no network, no GitHub and no credential: the "remote" is a bare
//! repository in a temporary directory, and pull requests are recorded by `LocalPrHost`.

use async_trait::async_trait;
use fleet_forge::testkit::{pr_request, repo_forge_contract, FileChange, ForgeFixture};
use fleet_forge::{body_carries, GitForge, LocalPrHost, RepoForge, RepoRef, WorkItemRef};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

/// Run `git` with a clean configuration, so the developer's own settings (a credential helper,
/// `autocrlf`, a signing key, hooks) cannot change what a test sees.
fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .current_dir(dir)
        .args(["-c", "core.autocrlf=false", "-c", "commit.gpgsign=false"])
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env(
            "GIT_CONFIG_GLOBAL",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
        )
        .env("GIT_AUTHOR_NAME", "fixture")
        .env("GIT_AUTHOR_EMAIL", "fixture@example.invalid")
        .env("GIT_COMMITTER_NAME", "fixture")
        .env("GIT_COMMITTER_EMAIL", "fixture@example.invalid")
        .output()
        .expect("git is on PATH");
    assert!(
        out.status.success(),
        "git {args:?} failed in {}: {}",
        dir.display(),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn apply(dir: &Path, changes: &[FileChange<'_>]) {
    for (path, content) in changes {
        let target = dir.join(path);
        match content {
            Some(text) => {
                std::fs::create_dir_all(target.parent().unwrap()).unwrap();
                std::fs::write(&target, text).unwrap();
            }
            None => std::fs::remove_file(&target).unwrap(),
        }
    }
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-m", "fixture"]);
}

/// A bare "origin", a working clone the fixture commits through, and the forge under test.
struct GitFixture {
    root: PathBuf,
    origin: PathBuf,
    work: PathBuf,
    forge: GitForge<LocalPrHost>,
    dirs: AtomicU32,
}

impl GitFixture {
    fn new(root: &Path) -> Self {
        let origin = root.join("origin.git");
        let work = root.join("work");
        std::fs::create_dir_all(&origin).unwrap();
        git(&origin, &["init", "--bare", "--initial-branch=main"]);
        git(root, &["clone", &origin.to_string_lossy(), "work"]);
        GitFixture {
            origin,
            work,
            forge: GitForge::new(root.join("forge"), LocalPrHost::new(root.join("prs"))),
            root: root.to_path_buf(),
            dirs: AtomicU32::new(0),
        }
    }
}

#[async_trait]
impl ForgeFixture for GitFixture {
    type Forge = GitForge<LocalPrHost>;

    fn forge(&self) -> &Self::Forge {
        &self.forge
    }

    fn repo(&self) -> RepoRef {
        RepoRef {
            slug: "owner/sandbox".into(),
            url: self.origin.to_string_lossy().into_owned(),
            base_branch: "main".into(),
        }
    }

    async fn commit_base(&self, changes: &[FileChange<'_>]) -> String {
        git(&self.work, &["checkout", "-B", "main"]);
        apply(&self.work, changes);
        git(&self.work, &["push", "origin", "main"]);
        git(&self.work, &["rev-parse", "HEAD"])
    }

    async fn unit_bundle(
        &self,
        base_sha: &str,
        branch: &str,
        commits: &[&[FileChange<'_>]],
    ) -> (PathBuf, String) {
        git(&self.work, &["checkout", "-B", branch, base_sha]);
        for changes in commits {
            apply(&self.work, changes);
        }
        let head = git(&self.work, &["rev-parse", "HEAD"]);
        let dest = self
            .root
            .join(format!("{}.bundle", branch.replace('/', "_")));
        git(
            &self.work,
            &[
                "bundle",
                "create",
                &dest.to_string_lossy(),
                &format!("{base_sha}..{branch}"),
            ],
        );
        git(&self.work, &["checkout", "main"]);
        (dest, head)
    }

    fn empty_dir(&self, name: &str) -> PathBuf {
        let n = self.dirs.fetch_add(1, Ordering::Relaxed);
        let dir = self.root.join(format!("{name}-{n}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
}

// --- 1. the contract suite, against git ---

#[tokio::test]
async fn the_git_forge_passes_the_repo_forge_contract() {
    let root = tempfile::tempdir().unwrap();
    let worlds = AtomicU32::new(0);
    repo_forge_contract(|| {
        let n = worlds.fetch_add(1, Ordering::Relaxed);
        let dir = root.path().join(format!("world-{n}"));
        std::fs::create_dir_all(&dir).unwrap();
        GitFixture::new(&dir)
    })
    .await;
}

/// A fixture with one base commit and one delivered unit, imported.
async fn imported(root: &Path) -> (GitFixture, String, String, Box<dyn fleet_forge::Scratch>) {
    let x = GitFixture::new(root);
    let base = x.commit_base(&[("src/a.txt", Some("a\n"))]).await;
    let (bundle, head) = x
        .unit_bundle(&base, "factory/7-u1", &[&[("src/b.txt", Some("b\n"))]])
        .await;
    let scratch = x
        .forge()
        .import_bundle(&x.repo(), "u1", &base, &bundle, "factory/7-u1")
        .await
        .unwrap();
    (x, base, head, scratch)
}

// --- 2. the scratch clone can reach nothing and runs no hook ---

#[tokio::test]
async fn a_scratch_clone_has_no_remote_no_credential_helper_and_hooks_disabled() {
    let root = tempfile::tempdir().unwrap();
    let (x, _, _, _scratch) = imported(root.path()).await;
    let dir = x.forge().scratch_dir("u1");
    assert_eq!(git(&dir, &["remote"]), "", "a scratch clone has no remote");
    let hooks = git(&dir, &["config", "--get", "core.hooksPath"]);
    assert!(
        !hooks.is_empty(),
        "hooks are pointed away from the repository"
    );
    assert!(
        !Path::new(&hooks).join("post-checkout").exists(),
        "the hooks directory is empty"
    );
    let helper = Command::new("git")
        .current_dir(&dir)
        .args(["config", "--get", "credential.helper"])
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&helper.stdout).trim(), "");
    assert_ne!(dir, x.forge().push_clone_dir(&x.repo()));
}

#[tokio::test]
async fn a_hook_planted_in_the_scratch_clone_does_not_run_when_the_tree_is_exported() {
    let root = tempfile::tempdir().unwrap();
    let (x, _, head, scratch) = imported(root.path()).await;
    let marker = root.path().join("hook-ran");
    let hooks = x.forge().scratch_dir("u1").join(".git").join("hooks");
    std::fs::create_dir_all(&hooks).unwrap();
    for name in [
        "post-checkout",
        "post-merge",
        "pre-commit",
        "reference-transaction",
    ] {
        let hook = hooks.join(name);
        std::fs::write(
            &hook,
            format!(
                "#!/bin/sh\necho ran > '{}'\n",
                marker.to_string_lossy().replace('\\', "/")
            ),
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
    }
    let tree = x.empty_dir("tree");
    scratch.export_tree(&head, &tree).await.unwrap();
    scratch.trial_merge().await.unwrap();
    assert!(tree.join("src/b.txt").is_file());
    assert!(!marker.exists(), "a hook in the scratch clone ran");
}

// --- 3. what is pushed is the verified commit, and nothing else ---

#[tokio::test]
async fn opening_a_pull_request_pushes_exactly_the_verified_commit_to_the_remote() {
    let root = tempfile::tempdir().unwrap();
    let (x, _, head, _scratch) = imported(root.path()).await;
    let mut request = pr_request("u1", "factory/7-u1", &head);
    request.repo = x.repo();
    let opened = x.forge().open_pr(&request).await.unwrap();
    assert_eq!(
        git(&x.origin, &["rev-parse", "refs/heads/factory/7-u1"]),
        head
    );
    // Nothing else was pushed: the remote has the base branch and this one.
    let mut branches: Vec<String> = git(
        &x.origin,
        &["for-each-ref", "--format=%(refname)", "refs/heads"],
    )
    .lines()
    .map(str::to_string)
    .collect();
    branches.sort();
    assert_eq!(branches, ["refs/heads/factory/7-u1", "refs/heads/main"]);
    let state = x.forge().pr_state(&x.repo(), &opened.url).await.unwrap();
    assert_eq!(state.head_sha, head);
    let item = WorkItemRef::parse("owner/sandbox#7", None).unwrap();
    assert!(body_carries(&state.body, &item, "owner/sandbox"));
}

#[tokio::test]
async fn a_second_import_replaces_the_first_and_only_its_head_can_be_opened() {
    let root = tempfile::tempdir().unwrap();
    let (x, base, first_head, _first) = imported(root.path()).await;
    let (bundle, second_head) = x
        .unit_bundle(&base, "factory/7-u1", &[&[("src/c.txt", Some("c\n"))]])
        .await;
    x.forge()
        .import_bundle(&x.repo(), "u1", &base, &bundle, "factory/7-u1")
        .await
        .unwrap();
    let mut stale = pr_request("u1", "factory/7-u1", &first_head);
    stale.repo = x.repo();
    assert!(x.forge().open_pr(&stale).await.is_err());
    let mut fresh = pr_request("u1", "factory/7-u1", &second_head);
    fresh.repo = x.repo();
    x.forge().open_pr(&fresh).await.unwrap();
    assert_eq!(
        git(&x.origin, &["rev-parse", "refs/heads/factory/7-u1"]),
        second_head
    );
}

// --- 4. the source bundle is a real bundle of the base ---

#[tokio::test]
async fn a_source_bundle_clones_to_the_tree_at_the_base_commit() {
    let root = tempfile::tempdir().unwrap();
    let x = GitFixture::new(root.path());
    let first = x.commit_base(&[("a.txt", Some("one\n"))]).await;
    let second = x
        .commit_base(&[("a.txt", Some("two\r\n")), ("dir/b.txt", Some("b\n"))])
        .await;
    for (commit, expected) in [(&first, "one\n"), (&second, "two\r\n")] {
        let bundle = x.empty_dir("bundle").join("source.bundle");
        x.forge()
            .source_bundle(&x.repo(), commit, &bundle)
            .await
            .unwrap();
        git(
            root.path(),
            &["bundle", "verify", &bundle.to_string_lossy()],
        );
        let clone = x.empty_dir("clone");
        git(
            root.path(),
            &["clone", &bundle.to_string_lossy(), &clone.to_string_lossy()],
        );
        git(&clone, &["checkout", "--detach", commit]);
        assert_eq!(
            std::fs::read(clone.join("a.txt")).unwrap(),
            expected.as_bytes()
        );
    }
}

// --- 5. paths ---

#[tokio::test]
async fn paths_with_spaces_and_non_ascii_names_read_and_export_unchanged() {
    let root = tempfile::tempdir().unwrap();
    let x = GitFixture::new(&root.path().join("a dir with spaces"));
    let odd = "docs/caf\u{e9} notes.md";
    let base = x
        .commit_base(&[(odd, Some("caf\u{e9}\n")), ("a.txt", Some("a\n"))])
        .await;
    assert_eq!(
        x.forge().read_file(&x.repo(), &base, odd).await.unwrap(),
        Some("caf\u{e9}\n".as_bytes().to_vec())
    );
    assert_eq!(
        x.forge()
            .list_files(&x.repo(), &base, "docs/")
            .await
            .unwrap(),
        [odd]
    );
    let (bundle, head) = x
        .unit_bundle(&base, "factory/7-u1", &[&[(odd, Some("changed\n"))]])
        .await;
    let scratch = x
        .forge()
        .import_bundle(&x.repo(), "u1", &base, &bundle, "factory/7-u1")
        .await
        .unwrap();
    let changes = scratch.changes(&base, &head).await.unwrap();
    assert_eq!(changes.len(), 1);
    assert_eq!(
        changes[0].path, odd,
        "a path is reported unquoted and with forward slashes"
    );
    let tree = x.empty_dir("tree");
    scratch.export_tree(&head, &tree).await.unwrap();
    assert_eq!(std::fs::read(tree.join(odd)).unwrap(), b"changed\n");
}

#[tokio::test]
async fn a_path_that_climbs_out_of_the_repository_is_refused() {
    let root = tempfile::tempdir().unwrap();
    let x = GitFixture::new(root.path());
    let base = x.commit_base(&[("a.txt", Some("a\n"))]).await;
    for path in [
        "../outside.txt",
        "/etc/passwd",
        "a/../../b",
        "C:\\Windows\\win.ini",
    ] {
        assert!(
            x.forge().read_file(&x.repo(), &base, path).await.is_err(),
            "{path:?} was read"
        );
    }
}
````

### Implementation notes

**What moves.** `crates/fleetd/src/gh_forge.rs` lines 36 to 191 become the bodies of
`impl Forge for GhForge` and its private helpers (`ensure_clone`, `map_mergeable`,
`guard_branch`, `run`, `run_status`); lines 194 to 212 are its tests. Nothing in it changes:
the argument lists, the order of the `git` calls and the fixed pull-request body are what
`characterisation_forge.rs` and the driver's tests pin. `guard_branch` and
`factory::safe_branch` are the same rule; `guard_branch` may call `safe_branch`.

**The layout under the root** (a hidden decision; this is the one the tests were written
against):

```
<root>/repos/<slug with '/' replaced by '__'>/push.git    a bare clone of repo.url; the only clone that pushes
<root>/scratch/<sanitised unit id>/                         one clone per unit; no remote
<root>/no-hooks/                                            an empty directory
```

Sanitise a unit id and a slug by keeping ASCII letters, digits, `_`, `-` and `.` and replacing
everything else with `_`; then refuse `.` and `..`. Behaviour 5 hands `scratch_dir` the id
`../../etc`.

**Every `git` call** is made with a fixed prefix and environment, never through a shell:

```
git -c core.hooksPath=<root>/no-hooks -c core.autocrlf=false -c core.symlinks=false
    -c core.fsmonitor=false -c core.quotepath=false -c protocol.file.allow=always <args…>
```

with `GIT_TERMINAL_PROMPT=0` always, and for every call on a scratch clone also
`GIT_CONFIG_NOSYSTEM=1`, `GIT_CONFIG_GLOBAL` set to the null device (`NUL` on Windows,
`/dev/null` elsewhere) and `-c credential.helper=`. Calls on the push clone keep the user's
global configuration, because that is where the `gh` credential helper lives. Write
`core.hooksPath` into the scratch clone's own configuration as well: behaviour 7 reads it back.

**Which plumbing.** These choices are what make invariants I1, I2, I5 and I7 hold on Windows:

| Operation | Command | Why |
|---|---|---|
| `resolve_base` | `git -C push.git fetch --prune origin +refs/heads/*:refs/heads/*`, then `rev-parse refs/heads/<base>` | The push clone is a bare mirror of branches only |
| `read_file`, `Scratch::read` | `git cat-file -e <commit>^{commit}` (unknown commit is an error), then `git ls-tree -z <commit> -- <path>` to tell "no such file" from failure, then `git cat-file blob <commit>:<path>`, reading stdout as bytes | No checkout, so no line-ending conversion |
| `list_files` | `git ls-tree -r -z --name-only <commit> -- <prefix>`, split on NUL | `-z` output is never quoted |
| `changes` | `git diff --name-status -z --no-renames <from> <to>`, split on NUL; a type change `T` is `Modified` | `--no-renames` gives the delete and the add |
| `source_bundle` | `git update-ref refs/heads/fleet-base <base_sha>` in the push clone, `git bundle create <dest> refs/heads/fleet-base`, then delete the ref | A bundle needs a ref. The bundle carries one ref, `refs/heads/fleet-base`, at the base commit; the harness is given `base_sha` and needs no other name. The ref is created and deleted under a per-repository lock, so two units cannot race on it |
| `import_bundle` | remove the unit's scratch directory; `git init`; `git fetch <push.git path> <base_sha>` is not possible for an arbitrary commit, so fetch `refs/heads/<base branch>` from the push clone and check `git merge-base --is-ancestor <base_sha> FETCH_HEAD`; then `git fetch <bundle> refs/heads/<branch>:refs/heads/<branch>`; then `git merge-base --is-ancestor <base_sha> <branch>` | The scratch clone gets its objects from two local paths and has no configured remote |
| `export_tree` | with `GIT_INDEX_FILE` set to a temporary file: `git read-tree <commit>`, then `git checkout-index -a -f --prefix=<dest>/` | `checkout-index` runs no hook and touches no branch |
| `trial_merge` | fetch the base branch from the push clone into the scratch clone (after `resolve_base`), then `git merge-tree --write-tree <base tip> <head>`: exit 0 is clean, exit 1 is a conflict, anything else is an error | Touches no working tree and no ref |
| `open_pr` | check the scratch clone's branch is at `head_sha`; `git -C push.git fetch <scratch path> +<head_sha>:refs/fleet/out/<unit>`; `git -C push.git push origin <head_sha>:refs/heads/<branch>`; then `host.create(request)` | The refspec names a commit, so nothing but that commit can be pushed |

A commit is validated as 40 or 64 lowercase hex digits before it is put on a command line; a
path is refused if it is absolute, has a drive letter, a backslash or a `..` segment.

**`LocalPrHost`.** One file per pull request, `<dir>/pr-<number>.json`, written with
`serde_json::to_vec_pretty` and holding at least `number`, `url`, `repo`, `branch`, `head_sha`,
`title`, `body`, `lifecycle` (`open`, `merged`, `closed`) and `mergeable` (`mergeable`, `dirty`,
`pending`). `url` is `local://<repo slug>/pull/<number>`. The next number is one more than the
highest in the directory, found under a file lock or by creating the file with
`create_new(true)` and retrying, so two daemons on one directory cannot both write `pr-3.json`.
A new record is `open` and `mergeable`.

**`GhCli`.** `create` runs `gh pr create --repo <slug> --base <base> --head <branch> --title
<title> --body-file <temp file>` (a file, so a body is never an argument) and parses the URL it
prints; the number is the URL's last segment. `view` runs `gh pr view <url> --json
state,mergeable,body,headRefOid` and maps `state` `OPEN`/`MERGED`/`CLOSED` and `mergeable`
`MERGEABLE`/`CONFLICTING`/anything else to the three cases, as `map_mergeable` does today.
`GhCli` has no test in this lane beyond its argument building, which is a pure function to
unit-test; it is first exercised for real in the watched proof (Part 3).

**Pitfalls.** On Windows a bundle path or a clone path with a space must be passed as one
argument, never interpolated (`Command::arg`, not a formatted string). `git` prints paths with
`/` even on Windows; compare with `/`. A long-running `git fetch` writes progress to stderr:
read stdout and stderr together (`Command::output`) so neither pipe fills. Do not `canonicalize`
paths on Windows before handing them to `git`: the `\\?\` prefix it adds is not understood.

### Verify

| Command | Expected |
|---|---|
| `cargo test -p fleet-forge` | every target passes: lib (10 from Part 1 plus the moved tests), `contract` (5), `git_it` (8) |
| `cargo test -p fleetd --test characterisation_forge` | 8 passed: `fleetd` is untouched by this lane |
| `cargo xtask test static --root-only`, `unit`, `contract`, `integration` | exit 0 |
| *Git-Bash:* `grep -rn "unimplemented!" crates/fleet-forge/src` | no output |
| *Git-Bash:* `cargo test -p fleet-forge --test git_it 2>&1 \| grep -c "test result: ok. 8 passed"`, run three times | `1` each time: the suite leaves nothing behind that a second run trips on |
| `git diff --name-only origin/factory/m1...HEAD` | only paths under **Owns** |

### Done when

- [ ] Behaviours 1 to 13 pass on the Windows build host; the pull request's checks pass them
      on Linux.
- [ ] `git_it.rs` passes with the developer's own global `git` configuration in place
      (`core.autocrlf=true` is the Windows default).
- [ ] No `unimplemented!` is left in `crates/fleet-forge/src`.
- [ ] The pull request names the registry rows it wants and reports the `git` version it ran on.

---

## Lane CC-RUNNER

**Owns:** `crates/fleet-runner/src/docker.rs`, `crates/fleet-runner/src/reap.rs` (the bodies;
not the signatures, the `Action` enum, the `UnitCensus` trait or `ReapReport`),
`crates/fleet-runner/tests/**`. It may add private modules. It does not change `check.rs`,
`lib.rs` or `testkit.rs`.

**Reads:** `crates/fleetd/src/reconcile.rs` (the decisions to move), `crates/fleetd/src/server.rs`
lines 753 to 836 (the two passes as they are today), `crates/fleetd/src/local_docker.rs` (how
the `docker` program is driven today; none of it moves, it stays with the driver path);
`crates/fleetd/tests/characterisation_reconcile.rs`; `forms/fleet-runner.md`; the findings of
spikes S5 and S7 when they exist.

**Worktree and branch:** `D:\MajorProjects\.swarm-wt\m1-cc-runner`, branch `feat/m1-fleet-runner`.

**Needs:** Part 1 merged. Docker running, for the integration tests only.

**Blocks:** CC-WIRE; CC-E2E.

### Form draft: `fleet-runner`

**Purpose.** Where the control plane runs a target repository's commands for itself: one
command, on a discarded copy of a tree, with no network and no credential, in the image the
repository declares. And how containers that outlived their unit are removed without taking a
resumable unit's volume with them. Without it the verifier could only believe the harness, and
a dead daemon would leave containers running.

**interface-files:** `crates/fleet-runner/src/lib.rs`, `check.rs`, `reap.rs`, `docker.rs`.

**Interfaces** (the signatures of Part 1, Task 4):

````rust
// crates/fleet-runner/src/check.rs
pub const UNIT_LABEL: &str = "cc.unit_id";

pub const CACHE_MOUNT: &str = "/cache";

pub const MAX_CAPTURE_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RunnerError {
    #[error("runner failure: {0}")]
    Failed(String),
}

pub fn cache_key(files: &[(String, Vec<u8>)]) -> String;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheRequest {
    pub unit_id: String,
    pub image: String,
    pub env: BTreeMap<String, String>,
    pub tree: PathBuf,
    pub setup: String,
    pub key_files: Vec<String>,
    pub timeout: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheRef {
    pub key: String,
    pub volume: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckRequest {
    pub unit_id: String,
    pub image: String,
    pub env: BTreeMap<String, String>,
    pub tree: PathBuf,
    pub cache: Option<CacheRef>,
    pub command: String,
    pub timeout: Duration,
    pub collect: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CheckOutput {
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub stdout: String,
    pub stderr: String,
    pub collected: BTreeMap<String, Vec<u8>>,
}

impl CheckOutput {
    pub fn succeeded(&self) -> bool;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerInfo {
    pub unit_id: String,
    pub name: String,
}

#[async_trait]
pub trait CheckRunner: Send + Sync {
    async fn build_cache(&self, request: &CacheRequest) -> Result<CacheRef, RunnerError>;

    async fn run_check(&self, request: &CheckRequest) -> Result<CheckOutput, RunnerError>;

    async fn unit_containers(&self) -> Result<Vec<ContainerInfo>, RunnerError>;

    async fn unit_volumes(&self) -> Result<Vec<String>, RunnerError>;

    async fn reap_unit(&self, unit_id: &str) -> Result<usize, RunnerError>;

    async fn drop_unit_volumes(&self, unit_id: &str) -> Result<usize, RunnerError>;
}

// crates/fleet-runner/src/reap.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    HaltWithContainer(String),
    HaltNoContainer(String),
    ReapStray(String),
}

pub fn reconcile(persisted_nonterminal: &[String], running: &[String]) -> Vec<Action>;

pub fn reconcile_live(
    persisted_nonterminal: &[String],
    live: &[String],
    running: &[String],
) -> Vec<Action>;

pub trait UnitCensus: Send + Sync {
    fn nonterminal(&self) -> Vec<String>;
    fn finished(&self) -> Vec<String>;
    fn live(&self) -> Vec<String>;
    fn stranded(&self, unit_id: &str);
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ReapReport {
    pub reaped: Vec<String>,
    pub stranded: Vec<String>,
    pub volumes_dropped: Vec<String>,
    pub errors: Vec<String>,
}

pub async fn reap_at_startup(runner: &dyn CheckRunner, census: &dyn UnitCensus) -> ReapReport;

pub async fn reap_tick(runner: &dyn CheckRunner, census: &dyn UnitCensus) -> ReapReport;

// crates/fleet-runner/src/docker.rs
#[derive(Debug, Clone)]
pub struct DockerRunner {
    /* private */
}

impl DockerRunner {
    pub fn new() -> Self;

    pub fn with_program(program: PathBuf) -> Self;

    pub async fn available(&self) -> bool;
}
````

**Invariants**

- I1. A check's command runs on a copy of the tree; the directory on the host is byte for byte
  what it was, whatever the command does.
- I2. A check container has no network interface but loopback, none of the host's environment,
  and exactly the environment its request gives.
- I3. A check container carries `UNIT_LABEL` with the request's unit id while it runs, and is
  gone when `run_check` returns, however the command ended.
- I4. A command that runs past its timeout is stopped and reported `timed_out` with no exit
  code; a command that exits non-zero is an `Ok` output.
- I5. No more than `MAX_CAPTURE_BYTES` of each output stream is kept, and what is kept is the
  end of the stream.
- I6. For one unit and one set of key-file contents there is one cache; asking again does not
  run setup again; a check can read the cache and cannot write it; a key file that is missing
  is an error.
- I7. `reap_unit` removes a unit's containers and none of its volumes.
- I8. A pass never touches the containers or volumes of a unit in `UnitCensus::live`, and a
  runner call that fails never stops a pass.
- I9. A pass drops volumes only for units in `UnitCensus::finished`.
- I10. On the timer, a unit is reported stranded only when the containers could be listed.
- I11. With the container engine unavailable, every `CheckRunner` call is an `Err`.
- I12. The crate depends on no workspace crate other than `fleet-core`, is the only crate that
  may depend on a Docker client crate, and declares a `testkit` feature.

**Hidden decisions.** That the engine is reached by running the `docker` program; how the tree
gets into the container; container and volume names; how a cache is marked complete; how output
is truncated.

**Gates**

| id | guards | mechanism | location | command | blocks |
|---|---|---|---|---|---|
| workspace.G1 | I12 | Build graph: the dependency allowlist and the `testkit` feature | `xtask/src/deps.rs#pub const RULES` | `cargo xtask test static` | merge |

**Unenforced (planned)**

| planned id | guards | mechanism | location | command | built by |
|---|---|---|---|---|---|
| fleet-runner.G1 | I7, I8, I9, I10 | Locked tests of the two passes on the scripted runner | `crates/fleet-runner/tests/contract.rs#the_timer_pass_leaves_a_live_unit_alone_and_cleans_up_around_it` | `cargo xtask test contract` | behaviours 3 to 7 |
| fleet-runner.G2 | I11 | Locked test: a missing engine | `crates/fleet-runner/tests/contract.rs#a_docker_program_that_does_not_exist_is_an_error_from_every_call` | `cargo xtask test contract` | behaviour 9 |

I1 to I6 are checked by `crates/fleet-runner/tests/docker_it.rs`, which is `#[ignore]`d
because it needs Docker; a test CI does not run is not a gate, so they stay unenforced until
the `e2e` tier (which has Docker) runs them. The pull request says so.

### Behaviours

In `crates/fleet-runner/tests/contract.rs` (contract tier: no Docker):

1. `at_startup_every_unfinished_unit_is_halted_and_every_other_container_is_a_stray`.
2. `on_the_timer_a_live_unit_and_its_container_are_never_touched`.
3. `the_startup_pass_reaps_every_labelled_container_and_reports_each_unfinished_unit`.
4. `a_unit_that_is_not_finished_keeps_its_volume_and_a_finished_one_loses_it`.
5. `the_timer_pass_leaves_a_live_unit_alone_and_cleans_up_around_it`.
6. `a_pass_with_nothing_to_do_does_nothing_however_often_it_runs`.
7. `an_unavailable_engine_is_reported_and_no_unit_is_stranded_on_the_timer`, and
   `at_startup_an_unavailable_engine_still_strands_every_unfinished_unit`.
8. The moved unit tests of `crates/fleetd/src/reconcile.rs` (lines 81 to 163), beside the moved
   functions.
9. `a_docker_program_that_does_not_exist_is_an_error_from_every_call`.

In `crates/fleet-runner/tests/docker_it.rs` (integration tier; needs Docker; every test is
`#[ignore]`d; run with `-- --ignored --test-threads=1`):

10. `the_docker_runner_passes_the_check_runner_contract`.
11. `a_check_container_has_no_network_and_none_of_the_hosts_environment`, and
    `setup_has_a_network_and_a_check_on_the_same_cache_does_not` (assumption A2).
12. `a_running_check_carries_the_units_label_and_a_timeout_leaves_nothing_behind`, and
    `reaping_removes_a_units_containers_whoever_started_them_and_keeps_its_volume` (assumption A4).
13. `a_command_that_prints_more_than_the_cap_keeps_the_end_of_its_output` (Review Focus 3).
14. `a_command_that_deletes_everything_it_can_reach_leaves_the_hosts_tree_intact`.

Run: `cargo test -p fleet-runner --test contract`. Expected before the implementation: 9
failed, with `not implemented: lane CC-RUNNER builds this` (behaviours 1 and 2:
`… moves this from crates/fleetd/src/reconcile.rs`). `docker_it` shows `7 ignored` until it is
run with `--ignored`, and then fails the same way.

`crates/fleet-runner/tests/contract.rs`:

````rust
//! Lane CC-RUNNER: reaping decisions and passes, on the scripted runner. No Docker.

use fleet_runner::testkit::{check, FakeCensus, FakeRunner};
use fleet_runner::{
    reap_at_startup, reap_tick, reconcile, reconcile_live, Action, CheckRunner, DockerRunner,
};
use std::path::PathBuf;

fn ids(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

fn sorted(mut list: Vec<String>) -> Vec<String> {
    list.sort();
    list
}

// --- 1. the decision tables, moved ---

#[test]
fn at_startup_every_unfinished_unit_is_halted_and_every_other_container_is_a_stray() {
    let actions = reconcile(&ids(&["a", "b"]), &ids(&["a", "c"]));
    assert_eq!(
        actions,
        [
            Action::HaltWithContainer("a".into()),
            Action::HaltNoContainer("b".into()),
            Action::ReapStray("c".into()),
        ]
    );
    assert!(reconcile(&[], &[]).is_empty());
}

#[test]
fn on_the_timer_a_live_unit_and_its_container_are_never_touched() {
    let actions = reconcile_live(
        &ids(&["u1", "u2", "u9"]),
        &ids(&["u1"]),
        &ids(&["u1", "u3", "u9"]),
    );
    assert_eq!(
        actions,
        [
            Action::HaltNoContainer("u2".into()),
            Action::HaltWithContainer("u9".into()),
            Action::ReapStray("u3".into()),
        ]
    );
    assert!(reconcile_live(&ids(&["u1"]), &ids(&["u1"]), &ids(&["u1"])).is_empty());
}

// --- 2. the startup pass ---

#[tokio::test]
async fn the_startup_pass_reaps_every_labelled_container_and_reports_each_unfinished_unit() {
    let runner = FakeRunner::new();
    runner.add_container("running", "cc_running_agent");
    runner.add_container("running", "cc_running_check");
    runner.add_container("finished", "cc_finished_agent");
    runner.add_container("unknown", "cc_unknown_agent");
    let census = FakeCensus::new(&["running", "waiting"], &["finished"], &[]);

    let report = reap_at_startup(&runner, &census).await;

    assert_eq!(runner.unit_containers().await.unwrap(), Vec::new());
    assert_eq!(sorted(report.reaped), ["finished", "running", "unknown"]);
    assert_eq!(sorted(census.stranded_units()), ["running", "waiting"]);
    assert_eq!(sorted(report.stranded), ["running", "waiting"]);
    assert!(report.errors.is_empty());
}

#[tokio::test]
async fn a_unit_that_is_not_finished_keeps_its_volume_and_a_finished_one_loses_it() {
    let runner = FakeRunner::new();
    for unit in ["running", "paused", "finished", "unknown"] {
        runner.add_volume(unit);
    }
    runner.add_container("running", "cc_running_agent");
    let census = FakeCensus::new(&["running", "paused"], &["finished"], &[]);

    let report = reap_at_startup(&runner, &census).await;

    assert_eq!(report.volumes_dropped, ["finished"]);
    assert_eq!(
        sorted(runner.unit_volumes().await.unwrap()),
        ["paused", "running", "unknown"],
        "an unfinished unit resumes from its volume; an unknown unit's volume is not this daemon's to drop"
    );
}

// --- 3. the timer pass ---

#[tokio::test]
async fn the_timer_pass_leaves_a_live_unit_alone_and_cleans_up_around_it() {
    let runner = FakeRunner::new();
    runner.add_container("live", "cc_live_agent");
    runner.add_volume("live");
    runner.add_container("stranded", "cc_stranded_agent");
    runner.add_container("stray", "cc_stray_agent");
    runner.add_volume("finished");
    let census = FakeCensus::new(&["live", "stranded"], &["finished"], &["live"]);

    let report = reap_tick(&runner, &census).await;

    let left: Vec<String> = runner
        .unit_containers()
        .await
        .unwrap()
        .into_iter()
        .map(|c| c.unit_id)
        .collect();
    assert_eq!(left, ["live"]);
    assert_eq!(census.stranded_units(), ["stranded"]);
    assert_eq!(sorted(report.reaped), ["stranded", "stray"]);
    assert_eq!(report.volumes_dropped, ["finished"]);
    assert_eq!(runner.unit_volumes().await.unwrap(), ["live"]);
}

#[tokio::test]
async fn a_pass_with_nothing_to_do_does_nothing_however_often_it_runs() {
    let runner = FakeRunner::new();
    runner.add_container("live", "cc_live_agent");
    let census = FakeCensus::new(&["live"], &[], &["live"]);
    for _ in 0..3 {
        let report = reap_tick(&runner, &census).await;
        assert_eq!(report, Default::default());
    }
    assert!(census.stranded_units().is_empty());
}

// --- 4. a failing engine never stops a pass or halts a unit it cannot see ---

#[tokio::test]
async fn an_unavailable_engine_is_reported_and_no_unit_is_stranded_on_the_timer() {
    let runner = FakeRunner::new();
    runner.unavailable(true);
    let census = FakeCensus::new(&["u1"], &["u0"], &["u1"]);
    let report = reap_tick(&runner, &census).await;
    assert!(!report.errors.is_empty());
    assert!(report.reaped.is_empty() && report.volumes_dropped.is_empty());
    assert!(census.stranded_units().is_empty());
}

#[tokio::test]
async fn at_startup_an_unavailable_engine_still_strands_every_unfinished_unit() {
    // The units are stranded whether or not their containers can be listed: no supervisor
    // holds them, and resume will reap again.
    let runner = FakeRunner::new();
    runner.unavailable(true);
    let census = FakeCensus::new(&["u1", "u2"], &[], &[]);
    let report = reap_at_startup(&runner, &census).await;
    assert_eq!(sorted(census.stranded_units()), ["u1", "u2"]);
    assert!(!report.errors.is_empty());
}

// --- 5. a missing engine is an error, never a panic and never a pass ---

#[tokio::test]
async fn a_docker_program_that_does_not_exist_is_an_error_from_every_call() {
    let dir = tempfile::tempdir().unwrap();
    let runner = DockerRunner::with_program(PathBuf::from("no-such-docker-program"));
    assert!(!runner.available().await);
    assert!(runner
        .run_check(&check(dir.path(), "echo hello"))
        .await
        .is_err());
    assert!(runner.unit_containers().await.is_err());
    assert!(runner.reap_unit("u1").await.is_err());
}
````

`crates/fleet-runner/tests/docker_it.rs`:

````rust
//! Lane CC-RUNNER, integration tier: check containers on a real Docker engine.
//!
//! Needs Docker running and the image named by `FLEET_TEST_IMAGE` (default
//! `docker.io/library/alpine:3.20`) present or pullable. Every test is `#[ignore]`d, as the
//! other Docker tests of this repository are: run them with
//! `cargo test -p fleet-runner --test docker_it -- --ignored --test-threads=1`.

use fleet_runner::testkit::{cache, check, check_runner_contract};
use fleet_runner::{CheckRunner, DockerRunner, MAX_CAPTURE_BYTES, UNIT_LABEL};
use std::process::Command;
use std::time::Duration;

fn image() -> String {
    std::env::var("FLEET_TEST_IMAGE").unwrap_or_else(|_| "docker.io/library/alpine:3.20".into())
}

fn docker(args: &[&str]) -> String {
    let out = Command::new("docker")
        .args(args)
        .output()
        .expect("docker is on PATH");
    assert!(
        out.status.success(),
        "docker {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// A unit id no other test and no other run uses.
fn unit(name: &str) -> String {
    format!("it-{name}-{}", std::process::id())
}

fn labelled(unit_id: &str) -> Vec<String> {
    docker(&[
        "ps",
        "-a",
        "--filter",
        &format!("label={UNIT_LABEL}={unit_id}"),
        "--format",
        "{{.Names}}",
    ])
    .lines()
    .map(str::to_string)
    .collect()
}

// --- 1. the contract suite, against Docker ---

#[tokio::test]
#[ignore = "needs Docker"]
async fn the_docker_runner_passes_the_check_runner_contract() {
    let dir = tempfile::tempdir().unwrap();
    check_runner_contract(DockerRunner::new, &image(), dir.path()).await;
}

// --- 2. a check container can reach nothing ---

#[tokio::test]
#[ignore = "needs Docker"]
async fn a_check_container_has_no_network_and_none_of_the_hosts_environment() {
    let dir = tempfile::tempdir().unwrap();
    std::env::set_var("FLEET_RUNNER_IT_SECRET", "must-not-leak");
    let runner = DockerRunner::new();
    let mut request = check(dir.path(), "ip -o addr | grep -v ' lo ' | wc -l; env");
    request.image = image();
    request.unit_id = unit("net");
    let out = runner.run_check(&request).await.unwrap();
    assert!(out.succeeded(), "{out:?}");
    assert!(
        out.stdout.starts_with("0\n"),
        "an interface other than loopback: {}",
        out.stdout
    );
    assert!(!out.stdout.contains("must-not-leak"));
    assert!(!out.stdout.contains("ANTHROPIC_API_KEY") && !out.stdout.contains("GH_TOKEN"));
}

#[tokio::test]
#[ignore = "needs Docker"]
async fn setup_has_a_network_and_a_check_on_the_same_cache_does_not() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("deps.lock"), "v1\n").unwrap();
    let runner = DockerRunner::new();
    let mut setup = cache(
        dir.path(),
        "ip -o addr | grep -v ' lo ' | wc -l > /cache/interfaces.txt",
        &["deps.lock"],
    );
    setup.image = image();
    setup.unit_id = unit("setup");
    let built = runner.build_cache(&setup).await.unwrap();
    let mut read = check(dir.path(), "cat /cache/interfaces.txt");
    read.image = image();
    read.unit_id = setup.unit_id.clone();
    read.cache = Some(built);
    let seen = runner.run_check(&read).await.unwrap();
    assert_ne!(seen.stdout.trim(), "0", "setup had no network interface");
    runner.drop_unit_volumes(&setup.unit_id).await.unwrap();
}

// --- 3. a check container is labelled while it runs and gone when it ends ---

#[tokio::test]
#[ignore = "needs Docker"]
async fn a_running_check_carries_the_units_label_and_a_timeout_leaves_nothing_behind() {
    let dir = tempfile::tempdir().unwrap();
    let runner = DockerRunner::new();
    let unit_id = unit("label");
    let mut request = check(dir.path(), "sleep 20");
    request.image = image();
    request.unit_id = unit_id.clone();
    request.timeout = Duration::from_secs(5);
    let running = tokio::spawn({
        let runner = runner.clone();
        async move { runner.run_check(&request).await }
    });
    tokio::time::sleep(Duration::from_secs(2)).await;
    assert_eq!(
        labelled(&unit_id).len(),
        1,
        "the check container is labelled while it runs"
    );
    let out = running.await.unwrap().unwrap();
    assert!(out.timed_out);
    assert_eq!(
        labelled(&unit_id),
        Vec::<String>::new(),
        "a timed-out container was left behind"
    );
}

// --- 4. output is bounded ---

#[tokio::test]
#[ignore = "needs Docker"]
async fn a_command_that_prints_more_than_the_cap_keeps_the_end_of_its_output() {
    let dir = tempfile::tempdir().unwrap();
    let runner = DockerRunner::new();
    // About 4 MiB of output, then a marker. A child that fills its pipe must not block forever.
    let mut request = check(
        dir.path(),
        "i=0; while [ $i -lt 70000 ]; do echo 0123456789012345678901234567890123456789012345678901234567; i=$((i+1)); done; echo END-MARKER",
    );
    request.image = image();
    request.unit_id = unit("flood");
    let out = runner.run_check(&request).await.unwrap();
    assert!(out.succeeded());
    assert!(out.stdout.len() <= MAX_CAPTURE_BYTES);
    assert!(out.stdout.trim_end().ends_with("END-MARKER"));
}

// --- 5. reaping by label ---

#[tokio::test]
#[ignore = "needs Docker"]
async fn reaping_removes_a_units_containers_whoever_started_them_and_keeps_its_volume() {
    let unit_id = unit("reap");
    let label = format!("{UNIT_LABEL}={unit_id}");
    let volume = format!("it-vol-{unit_id}");
    docker(&["volume", "create", "--label", &label, &volume]);
    // As a harness would have started it: not by this runner, and still running.
    docker(&[
        "run",
        "-d",
        "--label",
        &label,
        "-v",
        &format!("{volume}:/work"),
        &image(),
        "sleep",
        "600",
    ]);
    docker(&["run", "--label", &label, &image(), "true"]);
    let runner = DockerRunner::new();
    let listed: Vec<String> = runner
        .unit_containers()
        .await
        .unwrap()
        .into_iter()
        .filter(|c| c.unit_id == unit_id)
        .map(|c| c.name)
        .collect();
    assert_eq!(
        listed.len(),
        2,
        "running and exited containers are both listed"
    );
    assert_eq!(runner.reap_unit(&unit_id).await.unwrap(), 2);
    assert_eq!(labelled(&unit_id), Vec::<String>::new());
    assert!(runner.unit_volumes().await.unwrap().contains(&unit_id));
    assert_eq!(runner.drop_unit_volumes(&unit_id).await.unwrap(), 1);
    assert!(!runner.unit_volumes().await.unwrap().contains(&unit_id));
}

// --- 6. the tree on the host is copied, never mounted ---

#[tokio::test]
#[ignore = "needs Docker"]
async fn a_command_that_deletes_everything_it_can_reach_leaves_the_hosts_tree_intact() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::write(dir.path().join("src/keep.txt"), "keep\n").unwrap();
    let runner = DockerRunner::new();
    let mut request = check(dir.path(), "rm -rf ./* ./.[!.]* 2>/dev/null; ls -A | wc -l");
    request.image = image();
    request.unit_id = unit("copy");
    let out = runner.run_check(&request).await.unwrap();
    assert_eq!(out.stdout.trim(), "0");
    assert_eq!(
        std::fs::read(dir.path().join("src/keep.txt")).unwrap(),
        b"keep\n"
    );
}
````

### Implementation notes

**What moves.** `reconcile` and `reconcile_live` are `crates/fleetd/src/reconcile.rs` lines 18
to 79, verbatim, with their tests (lines 81 to 163). The order of the actions they return is
pinned by behaviours 1 and 2.

**The two passes** are `crates/fleetd/src/server.rs` lines 756 to 775 and 812 to 836 with the
store and the unit table replaced by `UnitCensus`, the swarm part left behind (it stays with
the driver path), and volumes added:

```
reap_at_startup:  running = unit_containers()            (on error: record it, treat as empty)
                  for each action of reconcile(nonterminal, running):
                      HaltWithContainer(u): reap_unit(u); stranded(u)
                      HaltNoContainer(u):   stranded(u)
                      ReapStray(u):         reap_unit(u)
                  for each u in unit_volumes() that is in finished: drop_unit_volumes(u)

reap_tick:        the same with reconcile_live(nonterminal, live, running), except that when
                  unit_containers() fails the pass records the error and returns: with no
                  view of the containers it strands nothing.
```

A unit is added to `ReapReport::reaped` only when `reap_unit` returned a count above zero.
Every `Err` from the runner becomes one line of `ReapReport::errors` and the pass goes on.
`live` is read once, before the containers are listed: a unit that becomes live in between is
at worst skipped until the next tick, never reaped.

**Driving `docker`.** One private helper runs the program with an argument vector (never a
shell), a deadline, and both output streams read concurrently into bounded buffers. A spawn
error (`NotFound` when the program is missing) is `RunnerError::Failed` naming the program.
`run_check` in order:

1. `docker create --label cc.unit_id=<unit> --network none --workdir /work [--env K=V …]
   [--mount type=volume,source=<cache volume>,target=/cache,readonly] <image> sh -c <command>`,
   with a generated name `cc_check_<sanitised unit>_<8 random hex digits>`.
2. `docker cp <tree>/. <name>:/work` (the trailing `/.` copies the directory's content).
3. `docker start --attach <name>` under the request's timeout. On timeout: `docker kill <name>`.
4. For each path in `collect`: `docker cp <name>:/work/<path> <temp dir>`, read the file, ignore
   "no such file".
5. `docker rm --force --volumes <name>`, always, including after an error in steps 2 to 4: do
   it from a guard that runs on every exit path. `--volumes` removes only anonymous volumes; a
   named cache volume is kept.

`build_cache`: compute `cache_key` from the key files read from `tree`; the volume is
`cc_cache_<sanitised unit>_<first 16 hex digits of the key>`, created with
`--label cc.unit_id=<unit>`. A cache is complete when the file `/cache/.fleet-ready` exists in
it (checked with a one-line container: `test -f /cache/.fleet-ready`). If the volume exists
without the marker, an earlier setup died: remove it and build again. Setup is `run_check`'s
sequence with the default bridge network in place of `--network none`, the volume mounted
read-write, and `touch /cache/.fleet-ready` appended only when the command exited 0. A setup
that exits non-zero is `RunnerError::Failed` carrying the end of its stderr.

`unit_containers`: `docker ps --all --filter label=cc.unit_id --format
'{{.Label "cc.unit_id"}}\t{{.Names}}'`. `unit_volumes`: `docker volume ls --filter
label=cc.unit_id --format '{{.Label "cc.unit_id"}}'`, de-duplicated. `reap_unit`: `docker ps
--all --quiet --filter label=cc.unit_id=<unit>`, then `docker rm --force --volumes` for each id.
`drop_unit_volumes`: `docker volume ls --quiet --filter label=cc.unit_id=<unit>`, then `docker
volume rm` for each.

**Bounded output.** Do not use `Command::output()` for a check: it keeps everything. Read each
stream in its own task into a ring that keeps the last `MAX_CAPTURE_BYTES` bytes, cut at a
character boundary; both tasks must be running before the process is waited on, or a child
that fills one pipe while the other is being read blocks for ever (behaviour 13).

**Pitfalls.** Sanitise a unit id before it goes into a name (keep ASCII letters, digits, `_`,
`.`, `-`; `local_docker.rs` lines 66 to 78 is the rule in use today); a label value is passed
as one argument and needs no quoting. `docker cp` from a Windows host loses the executable
bit: a repository whose commands run a script by path must invoke it through its interpreter,
which is the repository's concern and goes in the pull request as a note, not in the runner.
On Windows, killing the `docker start --attach` client does not stop the container: always
`docker kill`. `docker` prints CRLF on Windows for some formats: trim each line. Timeouts are
wall-clock; the tests leave a wide margin.

**Reconciling the spikes.** If S7 reports that a preset's offline run needs the cache
somewhere other than `/cache`, or writable, that is a change to `CACHE_MOUNT` or to invariant
I6: stop and report (assumption A2). If S5 reports that containers started by a killed harness
keep running on Windows, nothing changes here: that is exactly what the two passes reap.

### Verify

| Command | Expected |
|---|---|
| `cargo test -p fleet-runner` | lib (10 from Part 1 plus the moved tests) and `contract` (9) pass; `docker_it` shows 7 ignored |
| `cargo test -p fleet-runner --test docker_it -- --ignored --test-threads=1` | 7 passed, on a host with Docker; report the Docker version and the image digest used |
| *Git-Bash:* `docker ps -a --filter label=cc.unit_id --format '{{.Names}}' \| grep -c '^cc_check_'` after the run above | `0`: no test left a container |
| `cargo test -p fleetd --test characterisation_reconcile` | 5 passed: `fleetd` is untouched by this lane |
| `cargo xtask test static --root-only`, `unit`, `contract`, `integration` | exit 0 |
| *Git-Bash:* `grep -rn "unimplemented!" crates/fleet-runner/src` | no output |
| `git diff --name-only origin/factory/m1...HEAD` | only paths under **Owns** |

### Done when

- [ ] Behaviours 1 to 9 pass everywhere; 10 to 14 pass on the Windows build host with Docker
      Desktop and are reported with their real output.
- [ ] No `unimplemented!` is left in `crates/fleet-runner/src`.
- [ ] The pull request says how assumptions A2 and A4 were reconciled, or that the spike
      findings were not available.

---

## Lane CC-VERIFY

**Owns:** `crates/fleet-verify/src/verifier.rs` (the body of `verify`; not `VerifyConfig`,
`Verifier::new` or `SPEC_DIR`), `crates/fleet-verify/tests/**`. It may add private modules. It
does not change `seam.rs`, `lib.rs` or `testkit.rs`.

**Reads:** `crates/fleetd/src/driver.rs` lines 646 to 686 (today's `MergeCheck` step: export,
trial merge) and lines 61 to 67 (today's oracle fingerprint, which this replaces);
`crates/harness-protocol/README.md` (what `no_change` evidence describes; how frozen files are
hashed); `crates/factory-spec/src/scope.rs`; `crates/factory-presets/src/types.rs`;
`forms/fleet-verify.md`; issue #86.

**Worktree and branch:** `D:\MajorProjects\.swarm-wt\m1-cc-verify`, branch `feat/m1-fleet-verify`.

**Needs:** Part 1 merged. Nothing from lanes CC-FORGE or CC-RUNNER: every test here runs on
their fakes.

**Blocks:** CC-WIRE.

### Form draft: `fleet-verify`

**Purpose.** The control plane's own check of what a harness delivered: nothing a harness
reports is decisive. It imports the delivered bundle itself, reads the files itself, and runs
the repository's tests itself, in its own container. Without it a harness bug, or a builder
that edited a test, would end in a pull request.

**interface-files:** `crates/fleet-verify/src/lib.rs`, `seam.rs`, `verifier.rs`.

**Interfaces** (the signatures of Part 1, Task 5):

````rust
// crates/fleet-verify/src/seam.rs
#[derive(Debug, Clone, Copy)]
pub struct VerifyInput<'a> {
    pub order: &'a WorkOrder,
    pub capabilities: &'a Capabilities,
    pub result: &'a UnitResult,
    pub approved_oracle_hash: Option<&'a str>,
    pub freeze: Option<&'a OracleFreeze>,
    pub signed_spec: Option<&'a [u8]>,
    pub pr_body: Option<&'a str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Check {
    Spec,
    Commit,
    Diff,
    Protected,
    Oracle,
    Tests,
    Merge,
    Link,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Verified,
    NoChange,
    Rejected { check: Check, detail: String },
    Tampered { detail: String },
    MergeConflict,
    Unrunnable { check: Check, detail: String },
    TestsFailed {
        failing: Vec<String>,
        detail: String,
    },
}

impl Verdict {
    pub fn can_be_retried(&self) -> bool;
}

#[async_trait]
pub trait EvidenceVerifier: Send + Sync {
    async fn verify(&self, input: VerifyInput<'_>) -> Verdict;
}

// crates/fleet-verify/src/verifier.rs
pub const SPEC_DIR: &str = ".reqdrive/specs/";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyConfig {
    pub work_dir: PathBuf,
    pub check_timeout: Duration,
    pub setup_timeout: Duration,
}

pub struct Verifier {
    /* private */
}

impl Verifier {
    pub fn new(
        forge: Arc<dyn RepoForge>,
        runner: Arc<dyn CheckRunner>,
        config: VerifyConfig,
    ) -> Self;
}
````

**Invariants**

- I1. `verify` answers `Verified` only when every check below ran and held, and `NoChange`
  only for a `no_change` result whose checks ran and held, or a `pr_open` whose delivery
  changes nothing.
- I2. The spec file at the delivered head is byte for byte the signed bytes, and the first
  commit after the base adds that file and nothing else.
- I3. Every path changed after that first commit is inside the unit's scope and grants, by
  `factory_spec::Scope::contains`.
- I4. A change to a protected path, or a frozen file that is missing or whose bytes do not
  have the frozen hash, is `Tampered`.
- I5. No field of `result.evidence` other than `branch`, `head_sha` and `delivery` changes a
  verdict: the harness's own test results, control results and review are never read.
- I6. A check that could not be run is `Unrunnable`, never a pass and never a rejection: the
  engine is down, setup failed, the bundle cannot be imported, the trial merge errored, a test
  command exited 0 without a readable report.
- I7. The frozen id list is not empty and is exactly the ids the preset enumerates from the
  frozen files at the head.
- I8. A verdict about tests comes from one run of the repository's `test` command in a check
  container on an exported tree; every frozen id and every `expected_red` id is present in the
  report and passed, no reported test failed, and the command exited 0.
- I9. The exported tree has no `.git`, lies under `VerifyConfig::work_dir`, and is removed
  when `verify` returns.
- I10. For a work item that is an issue, the pull-request body carries it
  (`fleet_forge::body_carries`).
- I11. The crate depends on no workspace crate other than `fleet-core`, `harness-protocol`,
  `factory-spec`, `factory-presets`, `fleet-forge` and `fleet-runner`, and declares a `testkit`
  feature.

**Hidden decisions.** The order of the checks beyond what the invariants need; how the export
directory is named; how failing ids are ordered; the wording of every `detail`.

**Gates**

| id | guards | mechanism | location | command | blocks |
|---|---|---|---|---|---|
| workspace.G1 | I11 | Build graph: the dependency allowlist and the `testkit` feature | `xtask/src/deps.rs#pub const RULES` | `cargo xtask test static` | merge |

**Unenforced (planned)**

| planned id | guards | mechanism | location | command | built by |
|---|---|---|---|---|---|
| fleet-verify.G1 | I1 | Contract suite: `evidence_verifier_contract` against the real verifier | `crates/fleet-verify/tests/contract.rs#the_real_verifier_passes_the_evidence_verifier_contract` | `cargo xtask test contract` | behaviour 1 |
| fleet-verify.G2 | I2 | Locked tests of the spec check | `crates/fleet-verify/tests/contract.rs#a_first_commit_that_carries_anything_but_the_spec_file_is_rejected` | `cargo xtask test contract` | behaviours 5 to 8 |
| fleet-verify.G3 | I3, I4 | Locked tests of scope and protection | `crates/fleet-verify/tests/contract.rs#changing_a_protected_path_is_tampering_even_inside_scope` | `cargo xtask test contract` | behaviours 11 to 18 |
| fleet-verify.G4 | I5, I6, I7, I8 | Locked tests of per-test evidence | `crates/fleet-verify/tests/contract.rs#a_frozen_id_missing_from_the_report_is_a_failure_even_when_the_command_exits_zero` | `cargo xtask test contract` | behaviours 4 and 19 to 32 |
| fleet-verify.G5 | I9 | Locked test: the export is gone | `crates/fleet-verify/tests/contract.rs#the_exported_tree_is_removed_when_verification_ends_whatever_the_verdict` | `cargo xtask test contract` | behaviour 3 |
| fleet-verify.G6 | I10 | Locked test of the link check | `crates/fleet-verify/tests/contract.rs#a_pull_request_body_that_does_not_carry_the_work_item_is_rejected` | `cargo xtask test contract` | behaviour 35 |

The coverage floor and the mutation run the design asks of this crate get their numbers
once milestone 1 is over; they are not gates of this lane.

### Behaviours

All in `crates/fleet-verify/tests/contract.rs`. Every test starts from
`fleet_verify::testkit::World::good`, changes one thing, and asserts the verdict.

The seam and the happy path:

1. `the_real_verifier_passes_the_evidence_verifier_contract`
2. `a_good_unit_is_verified_from_the_control_planes_own_run`
3. `the_exported_tree_is_removed_when_verification_ends_whatever_the_verdict`
4. `what_the_harness_reports_about_its_own_tests_changes_nothing`

The spec at the head is the signed bytes:

5. `a_spec_file_that_differs_by_one_byte_is_rejected`
6. `a_spec_file_changed_or_deleted_by_a_later_commit_is_rejected`
7. `a_first_commit_that_carries_anything_but_the_spec_file_is_rejected` (Review Focus 1)
8. `a_delivery_with_no_commit_at_all_is_never_approved`

The commit:

9. `a_head_other_than_the_one_the_harness_named_is_rejected`
10. `a_bundle_that_cannot_be_imported_is_unrunnable_not_a_failure`

The diff is inside scope:

11. `a_file_outside_scope_is_rejected_and_named`
12. `deleting_a_file_outside_scope_is_a_change_to_it`
13. `a_scope_grant_admits_the_granted_file_and_nothing_else`
14. `scope_is_compared_without_regard_to_letter_case_or_separator_style`

Protected paths and the frozen files:

15. `changing_a_protected_path_is_tampering_even_inside_scope`
16. `an_existing_test_file_may_change_only_when_the_spec_lists_it`
17. `a_frozen_file_whose_bytes_changed_after_the_freeze_is_tampering`
18. `a_frozen_file_that_is_missing_from_the_head_is_tampering`
19. `at_t2_the_frozen_files_must_be_the_ones_a_person_approved`
20. `a_unit_with_no_freeze_or_an_empty_one_is_never_verified` (Review Focus 2)
21. `frozen_ids_that_are_not_the_ids_in_the_frozen_files_are_rejected` (Review Focus 2;
    assumption A3)

Per-test evidence, from the control plane's own run:

22. `a_frozen_test_that_failed_in_the_control_planes_run_stops_the_unit`
23. `a_frozen_id_missing_from_the_report_is_a_failure_even_when_the_command_exits_zero`
24. `a_passing_report_from_a_command_that_exited_non_zero_is_not_a_pass`
25. `a_test_other_than_a_frozen_one_that_fails_is_reported_too`
26. `no_report_is_a_failure_when_the_command_failed_and_unrunnable_when_it_succeeded`
27. `a_report_that_does_not_read_is_unrunnable`
28. `a_test_run_that_timed_out_stops_the_unit_and_can_be_tried_again`
29. `an_engine_that_is_down_is_unrunnable_never_a_pass_and_never_a_failure`
30. `a_setup_that_fails_is_unrunnable`
31. `an_expected_red_id_must_have_passed`

The trial merge and the link:

32. `a_base_that_moved_under_the_same_file_is_a_merge_conflict`
33. `a_trial_merge_that_cannot_be_run_is_unrunnable`
34. `a_pull_request_body_that_does_not_carry_the_work_item_is_rejected`
35. `a_unit_whose_work_item_is_not_an_issue_needs_no_link`

`no_change`, and a `pr_open` that changes nothing:

36. `no_change_is_confirmed_when_the_frozen_tests_pass_with_nothing_but_themselves_added`
37. `no_change_is_rejected_when_the_delivery_changes_code_or_the_frozen_tests_fail`
38. `a_pr_open_whose_only_commit_is_the_spec_is_reported_as_empty`

Run: `cargo test -p fleet-verify --test contract`. Expected before the implementation: 38
failed, each with `not implemented: lane CC-VERIFY builds this`.

`crates/fleet-verify/tests/contract.rs`:

````rust
//! Lane CC-VERIFY: the real verifier, on the in-memory forge and the scripted runner.
//!
//! No Docker, no git, no network. Each test starts from `World::good`, a unit a correct
//! verifier answers `Verified` for, changes one thing, and asserts the verdict.

use factory_presets::testkit::ReportBuilder;
use factory_presets::TestStatus;
use fleet_forge::testkit::FileChange;
use fleet_runner::testkit::Scripted;
use fleet_verify::testkit::{
    evidence_verifier_contract, good_work, report, World, BASE_ID, FROZEN_ID, FROZEN_TEST_PATH,
    FROZEN_TEST_SOURCE,
};
use fleet_verify::{Check, EvidenceVerifier, Verdict};
use harness_protocol::{bundle_hash, Outcome, Tier, WorkItemKind};
use std::path::Path;

async fn verdict(world: &World) -> Verdict {
    world.verifier().verify(world.input()).await
}

fn world(dir: &Path) -> World {
    World::good(dir)
}

/// The good work commit with more changes added.
fn work_plus<'a>(extra: &[FileChange<'a>]) -> Vec<FileChange<'a>> {
    let mut work: Vec<FileChange<'a>> = good_work();
    work.extend_from_slice(extra);
    work
}

fn rejected(verdict: &Verdict, check: Check) -> &str {
    match verdict {
        Verdict::Rejected { check: got, detail } if *got == check => detail,
        other => panic!("expected Rejected for {check:?}, got {other:?}"),
    }
}

fn unrunnable(verdict: &Verdict, check: Check) -> &str {
    match verdict {
        Verdict::Unrunnable { check: got, detail } if *got == check => detail,
        other => panic!("expected Unrunnable for {check:?}, got {other:?}"),
    }
}

fn tampered(verdict: &Verdict) -> &str {
    match verdict {
        Verdict::Tampered { detail } => detail,
        other => panic!("expected Tampered, got {other:?}"),
    }
}

// --- 1. the seam's contract, and the happy path ---

#[tokio::test]
async fn the_real_verifier_passes_the_evidence_verifier_contract() {
    let dir = tempfile::tempdir().unwrap();
    let world = world(dir.path());
    evidence_verifier_contract(|| world.verifier()).await;
}

#[tokio::test]
async fn a_good_unit_is_verified_from_the_control_planes_own_run() {
    let dir = tempfile::tempdir().unwrap();
    let world = world(dir.path());
    assert_eq!(verdict(&world).await, Verdict::Verified);

    // It imported the bundle itself and ran the repository's test command itself.
    assert!(world.forge.calls().contains(&"import_bundle".to_string()));
    let checks = world.runner.checks();
    let test_run = checks
        .iter()
        .find(|c| c.command == "npm test")
        .expect("the tests were run");
    assert_eq!(test_run.unit_id, "u1");
    assert_eq!(test_run.image, world.order.config.as_ref().unwrap().image);
    assert_eq!(test_run.collect, ["reports/junit.xml"]);
    assert!(
        test_run.cache.is_some(),
        "the check ran on the unit's own dependency cache"
    );
    let setups = world.runner.setups();
    assert_eq!(setups.len(), 1);
    assert_eq!(setups[0].setup, "npm ci");
    assert_eq!(setups[0].key_files, ["package.json", "package-lock.json"]);
    // The tree it checked was an export: the frozen test is there, and no repository.
    assert!(!test_run.tree.join(".git").exists());
}

#[tokio::test]
async fn the_exported_tree_is_removed_when_verification_ends_whatever_the_verdict() {
    let dir = tempfile::tempdir().unwrap();
    let mut world = world(dir.path());
    assert_eq!(verdict(&world).await, Verdict::Verified);
    let tree = world.runner.checks()[0].tree.clone();
    assert!(!tree.exists(), "the export outlived a verified unit");

    world.script_tests(Scripted::exit(1, "boom"));
    let _ = verdict(&world).await;
    let tree = world.runner.checks()[0].tree.clone();
    assert!(!tree.exists(), "the export outlived a failed check");
}

#[tokio::test]
async fn what_the_harness_reports_about_its_own_tests_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let mut world = world(dir.path());
    let evidence = world.result.evidence.as_mut().unwrap();
    evidence.test.exit_code = 1;
    evidence.test_report = Some(harness_protocol::TestReport {
        ids_passed: Vec::new(),
    });
    assert_eq!(verdict(&world).await, Verdict::Verified);
}

// --- 2. the spec at the head is the signed bytes ---

#[tokio::test]
async fn a_spec_file_that_differs_by_one_byte_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let mut world = world(dir.path());
    let altered = String::from_utf8(world.spec_bytes.clone()).unwrap() + "\n";
    let path = world.spec_path();
    world.deliver_exactly(&[&[(path.as_str(), Some(altered.as_str()))], &good_work()]);
    let got = verdict(&world).await;
    rejected(&got, Check::Spec);
    assert!(
        world.runner.checks().is_empty(),
        "no test was run for a unit with the wrong spec"
    );
}

#[tokio::test]
async fn a_spec_file_changed_or_deleted_by_a_later_commit_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let mut world = world(dir.path());
    let path = world.spec_path();
    world.deliver(&[
        &good_work(),
        &[(path.as_str(), Some("---\nid: SPEC-0001\n---\n"))],
    ]);
    rejected(&verdict(&world).await, Check::Spec);
    world.deliver(&[&good_work(), &[(path.as_str(), None)]]);
    rejected(&verdict(&world).await, Check::Spec);
}

#[tokio::test]
async fn a_first_commit_that_carries_anything_but_the_spec_file_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let mut world = world(dir.path());
    let path = world.spec_path();
    let spec = String::from_utf8(world.spec_bytes.clone()).unwrap();
    // The spec is byte-identical, but the same commit also changes a file outside scope. If
    // only the commits after the first were checked against scope, this would pass.
    world.deliver_exactly(&[
        &[
            (path.as_str(), Some(spec.as_str())),
            ("src/unrelated.js", Some("export const smuggled = true;\n")),
        ],
        &good_work(),
    ]);
    let got = verdict(&world).await;
    assert!(
        matches!(
            got,
            Verdict::Rejected {
                check: Check::Spec | Check::Diff,
                ..
            }
        ),
        "{got:?}"
    );
}

#[tokio::test]
async fn a_delivery_with_no_commit_at_all_is_never_approved() {
    let dir = tempfile::tempdir().unwrap();
    let mut world = world(dir.path());
    world.deliver_exactly(&[]);
    let got = verdict(&world).await;
    assert!(
        !matches!(got, Verdict::Verified | Verdict::NoChange),
        "a delivery with no commit was approved: {got:?}"
    );
}

// --- 3. the commit is the one the harness named ---

#[tokio::test]
async fn a_head_other_than_the_one_the_harness_named_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let mut world = world(dir.path());
    world.result.evidence.as_mut().unwrap().head_sha = "f".repeat(40);
    rejected(&verdict(&world).await, Check::Commit);
}

#[tokio::test]
async fn a_bundle_that_cannot_be_imported_is_unrunnable_not_a_failure() {
    let dir = tempfile::tempdir().unwrap();
    let mut world = world(dir.path());
    if let harness_protocol::DeliveryEvidence::Bundle { bundle_path } =
        &mut world.result.evidence.as_mut().unwrap().delivery
    {
        *bundle_path = dir
            .path()
            .join("no-such.bundle")
            .to_string_lossy()
            .into_owned();
    }
    unrunnable(&verdict(&world).await, Check::Commit);
}

// --- 4. the diff after the spec commit is inside scope ---

#[tokio::test]
async fn a_file_outside_scope_is_rejected_and_named() {
    let dir = tempfile::tempdir().unwrap();
    let mut world = world(dir.path());
    world.deliver(&[&work_plus(&[(
        "src/other.js",
        Some("export const x = 1;\n"),
    )])]);
    assert!(rejected(&verdict(&world).await, Check::Diff).contains("src/other.js"));
}

#[tokio::test]
async fn deleting_a_file_outside_scope_is_a_change_to_it() {
    let dir = tempfile::tempdir().unwrap();
    let mut world = world(dir.path());
    world.deliver(&[&work_plus(&[("package.json", None)])]);
    let got = verdict(&world).await;
    assert!(
        matches!(
            &got,
            Verdict::Rejected {
                check: Check::Diff,
                ..
            } | Verdict::Tampered { .. }
        ),
        "{got:?}"
    );
}

#[tokio::test]
async fn a_scope_grant_admits_the_granted_file_and_nothing_else() {
    let dir = tempfile::tempdir().unwrap();
    let mut world = world(dir.path());
    world.order.scope_grants = vec!["src/other.js".into()];
    world.deliver(&[&work_plus(&[(
        "src/other.js",
        Some("export const x = 1;\n"),
    )])]);
    assert_eq!(verdict(&world).await, Verdict::Verified);
    world.deliver(&[&work_plus(&[(
        "src/third.js",
        Some("export const y = 1;\n"),
    )])]);
    assert!(rejected(&verdict(&world).await, Check::Diff).contains("src/third.js"));
}

#[tokio::test]
async fn scope_is_compared_without_regard_to_letter_case_or_separator_style() {
    let dir = tempfile::tempdir().unwrap();
    let mut world = world(dir.path());
    // The spec names `src/cart.js`; a repository checked out on Windows may report `SRC/Cart.js`.
    let scope = world.order.scope.as_mut().unwrap();
    scope.touched_files = vec!["SRC\\Cart.js".into()];
    assert_eq!(verdict(&world).await, Verdict::Verified);
}

// --- 5. protected paths ---

#[tokio::test]
async fn changing_a_protected_path_is_tampering_even_inside_scope() {
    let dir = tempfile::tempdir().unwrap();
    for path in [
        "vitest.config.js",
        ".reqdrive/config.toml",
        "package-lock.json",
        "forms/cart.md",
        ".github/workflows/ci.yml",
        "CLAUDE.md",
    ] {
        let mut world = world(&tempfile::tempdir_in(dir.path()).unwrap().keep());
        // Put the path in scope, so only the protection can stop it.
        world
            .order
            .scope
            .as_mut()
            .unwrap()
            .touched_files
            .push(path.to_string());
        world.deliver(&[&work_plus(&[(path, Some("changed\n"))])]);
        assert!(tampered(&verdict(&world).await).contains(path), "{path}");
    }
}

#[tokio::test]
async fn an_existing_test_file_may_change_only_when_the_spec_lists_it() {
    let dir = tempfile::tempdir().unwrap();
    let mut world = world(dir.path());
    let edit: &[FileChange<'_>] = &[("test/cart.test.js", Some("// weakened\n"))];
    world.deliver(&[&work_plus(edit)]);
    assert!(tampered(&verdict(&world).await).contains("test/cart.test.js"));

    world.order.scope.as_mut().unwrap().touched_tests = vec!["test/cart.test.js".into()];
    world.deliver(&[&work_plus(edit)]);
    // Allowed to change, so the verdict now rests on the tests.
    assert_eq!(verdict(&world).await, Verdict::Verified);
}

// --- 6. the frozen files and their ids ---

#[tokio::test]
async fn a_frozen_file_whose_bytes_changed_after_the_freeze_is_tampering() {
    let dir = tempfile::tempdir().unwrap();
    let mut world = world(dir.path());
    let weakened = FROZEN_TEST_SOURCE.replace("() => {}", "() => { /* always passes */ }");
    world.deliver(&[&good_work(), &[(FROZEN_TEST_PATH, Some(weakened.as_str()))]]);
    assert!(tampered(&verdict(&world).await).contains(FROZEN_TEST_PATH));
    assert!(
        world.runner.checks().is_empty(),
        "no test was run on a tampered oracle"
    );
}

#[tokio::test]
async fn a_frozen_file_that_is_missing_from_the_head_is_tampering() {
    let dir = tempfile::tempdir().unwrap();
    let mut world = world(dir.path());
    world.deliver(&[&good_work(), &[(FROZEN_TEST_PATH, None)]]);
    assert!(tampered(&verdict(&world).await).contains(FROZEN_TEST_PATH));
}

#[tokio::test]
async fn at_t2_the_frozen_files_must_be_the_ones_a_person_approved() {
    let dir = tempfile::tempdir().unwrap();
    let mut world = world(dir.path());
    world.order.tier = Tier::T2;
    world.approved_oracle_hash = Some(bundle_hash(&world.freeze.frozen_files));
    assert_eq!(verdict(&world).await, Verdict::Verified);
    world.approved_oracle_hash = Some("0".repeat(64));
    tampered(&verdict(&world).await);
    world.approved_oracle_hash = None;
    let got = verdict(&world).await;
    assert!(
        !matches!(got, Verdict::Verified),
        "a T2 unit with no approval was verified"
    );
}

#[tokio::test]
async fn a_unit_with_no_freeze_or_an_empty_one_is_never_verified() {
    let dir = tempfile::tempdir().unwrap();
    let mut world = world(dir.path());
    let mut input = world.input();
    input.freeze = None;
    let got = world.verifier().verify(input).await;
    rejected(&got, Check::Oracle);

    // An empty id list would make "every frozen id passed" true of nothing.
    world.freeze.frozen_ids.clear();
    rejected(&verdict(&world).await, Check::Oracle);
    world.freeze.frozen_files.clear();
    rejected(&verdict(&world).await, Check::Oracle);
}

#[tokio::test]
async fn frozen_ids_that_are_not_the_ids_in_the_frozen_files_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let mut world = world(dir.path());
    // The harness lists an id that the frozen file does not declare, and leaves out the one it
    // does: the report could then "pass" without the real test having run.
    world.freeze.frozen_ids = vec![BASE_ID.to_string()];
    rejected(&verdict(&world).await, Check::Oracle);
}

// --- 7. per-test evidence from the control plane's own run ---

#[tokio::test]
async fn a_frozen_test_that_failed_in_the_control_planes_run_stops_the_unit() {
    let dir = tempfile::tempdir().unwrap();
    for status in [TestStatus::Failed, TestStatus::Errored, TestStatus::Skipped] {
        let mut world = world(&tempfile::tempdir_in(dir.path()).unwrap().keep());
        world.script_tests(Scripted::exit(1, "1 failed").writing(
            "reports/junit.xml",
            report(status, TestStatus::Passed).as_bytes(),
        ));
        match verdict(&world).await {
            Verdict::TestsFailed { failing, .. } => assert_eq!(failing, [FROZEN_ID], "{status:?}"),
            other => panic!("{status:?}: expected TestsFailed, got {other:?}"),
        }
    }
}

#[tokio::test]
async fn a_frozen_id_missing_from_the_report_is_a_failure_even_when_the_command_exits_zero() {
    let dir = tempfile::tempdir().unwrap();
    let mut world = world(dir.path());
    let without_frozen = ReportBuilder::vitest()
        .case("test/cart.test.js", "cart > totals", TestStatus::Passed)
        .build();
    world.script_tests(
        Scripted::exit(0, "1 passed").writing("reports/junit.xml", without_frozen.as_bytes()),
    );
    match verdict(&world).await {
        Verdict::TestsFailed { failing, .. } => assert_eq!(failing, [FROZEN_ID]),
        other => panic!("expected TestsFailed, got {other:?}"),
    }
}

#[tokio::test]
async fn a_passing_report_from_a_command_that_exited_non_zero_is_not_a_pass() {
    let dir = tempfile::tempdir().unwrap();
    let mut world = world(dir.path());
    world.script_tests(Scripted::exit(1, "runner crashed after reporting").writing(
        "reports/junit.xml",
        report(TestStatus::Passed, TestStatus::Passed).as_bytes(),
    ));
    assert!(matches!(verdict(&world).await, Verdict::TestsFailed { .. }));
}

#[tokio::test]
async fn a_test_other_than_a_frozen_one_that_fails_is_reported_too() {
    let dir = tempfile::tempdir().unwrap();
    let mut world = world(dir.path());
    world.script_tests(Scripted::exit(1, "1 failed").writing(
        "reports/junit.xml",
        report(TestStatus::Passed, TestStatus::Failed).as_bytes(),
    ));
    match verdict(&world).await {
        Verdict::TestsFailed { failing, .. } => assert_eq!(failing, [BASE_ID]),
        other => panic!("expected TestsFailed, got {other:?}"),
    }
}

#[tokio::test]
async fn no_report_is_a_failure_when_the_command_failed_and_unrunnable_when_it_succeeded() {
    let dir = tempfile::tempdir().unwrap();
    let mut world = world(dir.path());
    // A build failure writes no report and exits non-zero: the check ran, and failed.
    world.script_tests(Scripted::exit(101, "error: could not compile"));
    assert!(matches!(verdict(&world).await, Verdict::TestsFailed { .. }));
    // Exit 0 and no report: the command did not do what the configuration says it does.
    world.script_tests(Scripted::exit(0, "ok"));
    unrunnable(&verdict(&world).await, Check::Tests);
}

#[tokio::test]
async fn a_report_that_does_not_read_is_unrunnable() {
    let dir = tempfile::tempdir().unwrap();
    let mut world = world(dir.path());
    world.script_tests(Scripted::exit(0, "ok").writing("reports/junit.xml", b"<testsuites><oops"));
    unrunnable(&verdict(&world).await, Check::Tests);
}

#[tokio::test]
async fn a_test_run_that_timed_out_stops_the_unit_and_can_be_tried_again() {
    let dir = tempfile::tempdir().unwrap();
    let mut world = world(dir.path());
    world.script_tests(Scripted {
        timed_out: true,
        ..Scripted::default()
    });
    let got = verdict(&world).await;
    assert!(got.can_be_retried(), "{got:?}");
}

#[tokio::test]
async fn an_engine_that_is_down_is_unrunnable_never_a_pass_and_never_a_failure() {
    let dir = tempfile::tempdir().unwrap();
    let world = world(dir.path());
    world.runner.unavailable(true);
    unrunnable(&verdict(&world).await, Check::Tests);
    world.runner.unavailable(false);
    assert_eq!(
        verdict(&world).await,
        Verdict::Verified,
        "the same unit verifies once it can run"
    );
}

#[tokio::test]
async fn a_setup_that_fails_is_unrunnable() {
    let dir = tempfile::tempdir().unwrap();
    let world = world(dir.path());
    world
        .runner
        .on("npm ci", Scripted::exit(1, "registry unreachable"));
    unrunnable(&verdict(&world).await, Check::Tests);
}

#[tokio::test]
async fn an_expected_red_id_must_have_passed() {
    let dir = tempfile::tempdir().unwrap();
    let mut world = world(dir.path());
    world.order.expected_red = vec![BASE_ID.to_string()];
    assert_eq!(verdict(&world).await, Verdict::Verified);
    world.order.expected_red = vec!["test/gate.test.js > gate > migrated".to_string()];
    match verdict(&world).await {
        Verdict::TestsFailed { failing, .. } => {
            assert_eq!(failing, ["test/gate.test.js > gate > migrated"])
        }
        other => panic!("expected TestsFailed, got {other:?}"),
    }
}

// --- 8. the trial merge ---

#[tokio::test]
async fn a_base_that_moved_under_the_same_file_is_a_merge_conflict() {
    let dir = tempfile::tempdir().unwrap();
    let world = world(dir.path());
    world
        .forge
        .commit_base(&world.repo, &[("docs/notes.md", Some("unrelated\n"))]);
    assert_eq!(verdict(&world).await, Verdict::Verified);
    world.forge.commit_base(
        &world.repo,
        &[("src/cart.js", Some("export const total = () => 0;\n"))],
    );
    assert_eq!(verdict(&world).await, Verdict::MergeConflict);
}

#[tokio::test]
async fn a_trial_merge_that_cannot_be_run_is_unrunnable() {
    let dir = tempfile::tempdir().unwrap();
    let world = world(dir.path());
    world.forge.fail("trial_merge");
    unrunnable(&verdict(&world).await, Check::Merge);
}

// --- 9. the work item in the pull request's body ---

#[tokio::test]
async fn a_pull_request_body_that_does_not_carry_the_work_item_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let mut world = world(dir.path());
    world.pr_body = "A summary with no link.\n".into();
    rejected(&verdict(&world).await, Check::Link);
    world.pr_body = "Fixes example/sandbox#2\n\nWork-Item: example/sandbox#2\n".into();
    rejected(&verdict(&world).await, Check::Link);

    let mut input = world.input();
    input.pr_body = None;
    rejected(&world.verifier().verify(input).await, Check::Link);
}

#[tokio::test]
async fn a_unit_whose_work_item_is_not_an_issue_needs_no_link() {
    let dir = tempfile::tempdir().unwrap();
    let mut world = world(dir.path());
    world.order.work_item.kind = WorkItemKind::RoadmapItem;
    world.order.work_item.reference = "demo:u1".into();
    world.pr_body = "A summary with no link.\n".into();
    assert_eq!(verdict(&world).await, Verdict::Verified);
}

// --- 10. no_change ---

#[tokio::test]
async fn no_change_is_confirmed_when_the_frozen_tests_pass_with_nothing_but_themselves_added() {
    let dir = tempfile::tempdir().unwrap();
    let mut world = world(dir.path());
    world.deliver(&[&[(FROZEN_TEST_PATH, Some(FROZEN_TEST_SOURCE))]]);
    world.result.outcome = Outcome::NoChange;
    assert_eq!(verdict(&world).await, Verdict::NoChange);
}

#[tokio::test]
async fn no_change_is_rejected_when_the_delivery_changes_code_or_the_frozen_tests_fail() {
    let dir = tempfile::tempdir().unwrap();
    let mut world = world(dir.path());
    world.result.outcome = Outcome::NoChange;
    // The good delivery changes src/cart.js: that is a change.
    let got = verdict(&world).await;
    assert!(matches!(got, Verdict::Rejected { .. }), "{got:?}");

    world.deliver(&[&[(FROZEN_TEST_PATH, Some(FROZEN_TEST_SOURCE))]]);
    world.result.outcome = Outcome::NoChange;
    world.script_tests(Scripted::exit(1, "1 failed").writing(
        "reports/junit.xml",
        report(TestStatus::Failed, TestStatus::Passed).as_bytes(),
    ));
    let got = verdict(&world).await;
    assert!(
        !matches!(got, Verdict::NoChange | Verdict::Verified),
        "{got:?}"
    );
}

// --- 11. a pr_open result with nothing in it ---

#[tokio::test]
async fn a_pr_open_whose_only_commit_is_the_spec_is_reported_as_empty() {
    let dir = tempfile::tempdir().unwrap();
    let mut world = world(dir.path());
    world.deliver(&[]);
    let got = verdict(&world).await;
    assert!(
        !matches!(got, Verdict::Verified),
        "a unit that changed nothing was verified: {got:?}"
    );
}
````

### Implementation notes

**The checks, in the order to run them.** Everything before step 4 touches no forge and no
runner, so a malformed input is refused without I/O (this is what the contract suite asks).

| # | Check | When it does not hold |
|---|---|---|
| 1 | The result has evidence. Its delivery is a bundle | `Rejected{Commit}`; `Rejected{Link}` |
| 2 | The work order has a spec, a source, a scope and a configuration; `signed_spec` is given; its SHA-256 is `order.spec.signed_hash`; the configuration's preset is one `factory_presets::preset` knows | `Unrunnable{Spec}` for anything missing; `Rejected{Spec}` for a hash that differs |
| 3 | A freeze is given, with at least one frozen file and one frozen id. At T2 and T3, `approved_oracle_hash` is given and equals `harness_protocol::bundle_hash(freeze.frozen_files)`. For an `issue` work item, `pr_body` is given and `fleet_forge::body_carries` it (`WorkItemRef::parse(reference, fingerprint)`, repository `order.repo.slug`) | `Rejected{Oracle}` for a missing or empty freeze or a missing approval; `Tampered` for an approval of other files; `Rejected{Link}` |
| 4 | `forge.import_bundle(repo, unit_id, base_sha, bundle, order.branch)` succeeds | `Unrunnable{Commit}` |
| 5 | `evidence.branch` is `order.branch`; the scratch head is `evidence.head_sha` | `Rejected{Link}`; `Rejected{Commit}` |
| 6 | There is at least one commit after the base. `changes(base, first)` is exactly one `Added`, the spec path (`SPEC_DIR` + id + `.md`). `read(head, spec path)` is the signed bytes | `Rejected{Commit}` for no commit; `Rejected{Spec}` otherwise |
| 7 | `delta = changes(first, head)`. For `pr_open`: an empty `delta` is `NoChange`. For `no_change`: every entry of `delta` is an `Added` frozen file | `NoChange` (the supervisor fails a `pr_open` that verifies as empty); `Rejected{Diff}` |
| 8 | No path of `delta` is protected: the preset's `is_protected`; anything under `.reqdrive/`, `forms/` or `.github/`; a path in `config.lockfiles`; a `Modified` or `Deleted` path under one of `config.test_dirs` that is not in `scope.touched_tests` | `Tampered`, naming the paths |
| 9 | Every path of `delta` is inside `Scope{…order.scope}.with_grants(&order.scope_grants)` | `Rejected{Diff}`, naming the paths |
| 10 | Each frozen file read at the head exists and has its frozen SHA-256 (`file_sha256`). The ids `preset.enumerate_ids(path, source)` gives for the frozen files, taken together, are exactly `freeze.frozen_ids` as a set | `Tampered` for a file; `Rejected{Oracle}` for the ids, or for a file the preset cannot enumerate |
| 11 | Export the head (`scratch.export_tree`) into a new directory under `work_dir`; `runner.build_cache` with `config.commands.setup`, key files `config.manifests` then `config.lockfiles`; `runner.run_check` with `config.commands.test`, `config.image`, `config.env`, the cache, and `collect = [config.test_report]`. Then evaluate (below) | `Unrunnable{Tests}` when the export, the cache or the run is an `Err`; otherwise the evaluation's verdict |
| 12 | For `pr_open`: `scratch.trial_merge()` is clean | `Unrunnable{Merge}` for an `Err`; `MergeConflict` |
| 13 | | `Verified` for `pr_open`; `NoChange` for `no_change` |

Step 8 comes before step 9 on purpose: a path that is both protected and out of scope is
tampering, which stops for a person, not a rejection, which fails the unit.

**Evaluating the test run.**

```
timed out                                   -> TestsFailed{failing: [], detail: "the test command timed out after <n>s"}
no report collected, exit code 0            -> Unrunnable{Tests}   (the command did not do what the configuration says)
no report collected, exit code not 0        -> TestsFailed{failing: [], detail: the end of stderr}
preset.read_report(report) is an Err        -> Unrunnable{Tests}
failing = reported ids whose status is not Passed, in report order
        + frozen ids not in the report
        + expected_red ids not in the report      (each id once)
failing is not empty                        -> TestsFailed{failing, detail}
exit code not 0                             -> TestsFailed{failing: [], detail: "exit <n> with every reported test passing"}
otherwise                                   -> the run passed
```

For a `no_change` result a failed run is `Rejected{Tests}`: the harness claimed the frozen
tests already pass, and they do not. The report is the report's bytes as UTF-8; bytes that are
not UTF-8 are `Unrunnable{Tests}`.

**The export.** Create it with a name no other verification uses (`work_dir/<unit id>-<a
counter or random suffix>`), and remove it from a guard that runs on every return path,
including the early ones after step 11 has begun. `World::verifier` gives a `work_dir` inside
the test's temporary directory; behaviour 3 checks the tree is gone.

**What milestone 2 adds, and where.** None of it is built here.

| Milestone 2 check | Where it goes |
|---|---|
| `secrets`: no new match against the fingerprint baseline | After step 9, over `delta`; a violation is `Rejected` with a new `Check::Secrets` |
| `dependencies`: a manifest changes only by permitted additions; the lockfile is regenerated and compared | Step 8 stops treating a lockfile as protected when the manifest check passes; the regeneration is a setup run in `fleet-runner`; new `Check::Dependencies` |
| Existing in-file test code is unchanged (`Preset::test_regions`) | Step 8, for `Modified` source files |
| No test that was green at the base goes red | Step 11 gains a second run on the exported base, and the evaluation a `base_passed` list |
| Holdouts: the bundle in the store matches its hash; the holdout ids pass in a second container | A step 11b on a second export with the holdout files added; the bundle comes through `VerifyInput` |
| `eidos_gates`: every registered gate command passes | After step 11, one `run_check` per gate; a gate with no command is `Unrunnable` |

**Pitfalls.** `harness_protocol::Scope` and `factory_spec::Scope` are two types with the same
four lists: convert field by field, and take `contains` from `factory-spec` only, so that case
and separator rules are the shared ones. The frozen ids are compared as sets, but a duplicate
in `freeze.frozen_ids` is itself a rejection. Do not log the signed bytes or a report: a report
can be 64 MiB (`factory_presets::MAX_REPORT_BYTES`). `Verdict`'s `detail` strings reach the
cockpit: keep them to one line.

### Verify

| Command | Expected |
|---|---|
| `cargo test -p fleet-verify` | lib (4) and `contract` (38) pass |
| `cargo test -p fleet-verify --features testkit` | the same |
| `cargo xtask test static --root-only`, `unit`, `contract` | exit 0 |
| *Git-Bash:* `grep -rn "unimplemented!" crates/fleet-verify/src` | no output |
| *Git-Bash:* `grep -rn "test_report\b\|ids_passed\|\.controls\|\.review" crates/fleet-verify/src --include=*.rs \| grep -v testkit.rs \| grep -v "config.test_report"` | no output: the harness's reported results are not read (invariant I5) |
| `git diff --name-only origin/factory/m1...HEAD` | only paths under **Owns** |

### Done when

- [ ] Behaviours 1 to 38 pass; no `unimplemented!` is left in `crates/fleet-verify/src`.
- [ ] Mutating any one of steps 1 to 12 into "always holds" fails at least one listed test
      (try each by hand once and report which test caught it).
- [ ] The pull request says how assumption A3 was reconciled, or that S6's findings were not
      available, and names the registry rows it wants.

---

## Lane CC-ADMISSION

**Owns:** `crates/fleet-admission/src/admission.rs` (the bodies of `parse_repo_config`,
`Admission::new` and `admit`), `crates/fleet-admission/tests/**`. It may add private modules.
It does not change `types.rs`, `lib.rs` or `testkit.rs`.

**Reads:** `crates/fleetd/src/server.rs` lines 348 to 403 (`create_mission`: today's admission,
its hard-coded repository, command and caps) and lines 384 to 397 (the unchecked insert of
issue #74); `crates/fleetd/tests/characterisation_admission.rs`; the `reqdrive` repository's
`docs/repo-config.md` (the configuration file's format); `forms/fleet-admission.md`; issues #74
and #90.

**Worktree and branch:** `D:\MajorProjects\.swarm-wt\m1-cc-admission`, branch
`feat/m1-fleet-admission`.

**Needs:** Part 1 merged. Nothing from lanes CC-STORE or CC-FORGE: every test runs on their
fakes.

**Blocks:** CC-WIRE.

### Form draft: `fleet-admission`

**Purpose.** Decides whether a requested unit may start, in a fixed order, and records the ones
that may. Every refusal names its reason. Without it a unit could run from bytes nobody signed,
in a repository nobody allowed, past the spend ceiling, or twice for one click.

**interface-files:** `crates/fleet-admission/src/lib.rs`, `types.rs`, `admission.rs`.

**Interfaces** (the signatures of Part 1, Task 6):

````rust
// crates/fleet-admission/src/types.rs
#[derive(Debug, Clone, PartialEq)]
pub struct AdmissionConfig {
    pub repos: Vec<RepoRef>,
    pub unit_usd_ceiling: f64,
    pub unit_wall_clock_ceiling_secs: u64,
    pub global_usd_cap: f64,
    pub spend_window_ms: i64,
    pub min_review_rounds: u32,
    pub presets: Vec<String>,
    pub presets_version: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DispatchRequest {
    pub work_item: String,
    pub fingerprint: Option<String>,
    pub repo: String,
    pub spec_sha256: String,
    pub profile: Option<String>,
    pub harness: String,
    pub opt_in: bool,
    pub idempotency_key: String,
    pub usd_cap: Option<f64>,
    pub wall_clock_secs: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum Refusal {
    #[error("the request has no idempotency key")]
    MissingKey,
    #[error("{reference:?} is not a work item of the form owner/repo#N")]
    WorkItemMalformed { reference: String },
    #[error("idempotency key already used by unit {unit_id}")]
    DuplicateKey { unit_id: String },
    #[error("repository {repo} is not on the allowlist")]
    RepoNotAllowed { repo: String },
    #[error("the repository could not be read: {detail}")]
    Forge { detail: String },
    #[error("repository {repo} is not onboarded: {detail}")]
    NotOnboarded { repo: String, detail: String },
    #[error("this control plane has no preset named {preset:?}")]
    PresetUnsupported { preset: String },
    #[error("no harness is registered as {harness:?}")]
    HarnessUnknown { harness: String },
    #[error("harness {harness} is not healthy: {error}")]
    HarnessUnhealthy { harness: String, error: String },
    #[error("harness {harness} does not declare the {preset} preset")]
    PresetNotDeclared { preset: String, harness: String },
    #[error("preset rules differ: the control plane has {ours}, harness {harness} has {theirs}")]
    PresetsVersionMismatch {
        harness: String,
        ours: String,
        theirs: String,
    },
    #[error("no signature is stored for spec bytes {spec_sha256} in {repo}")]
    NotSigned { repo: String, spec_sha256: String },
    #[error("the stored spec bytes do not parse: {detail}")]
    SpecUnreadable { detail: String },
    #[error("the signed spec is for {spec}, not for {request}")]
    WorkItemMismatch { request: String, spec: String },
    #[error("the {cap} cap asked for ({requested}) is above the ceiling ({ceiling})")]
    CapAboveCeiling {
        cap: &'static str,
        requested: f64,
        ceiling: f64,
    },
    #[error("the spend ceiling is reached: ${committed:.2} committed of ${cap:.2}")]
    SpendCeiling { committed: f64, cap: f64 },
    #[error("committed spend could not be read, so nothing is admitted: {detail}")]
    SpendUnknown { detail: String },
    #[error("harness {harness} may not run this unit: {reason}")]
    Ineligible { harness: String, reason: String },
    #[error("the unit's reservation could not be recorded, so it was not started: {detail}")]
    ReservationFailed { detail: String },
}

impl Refusal {
    pub fn code(&self) -> &'static str;
}

#[derive(Debug, Clone, PartialEq)]
pub struct Admitted {
    pub unit_id: String,
    pub record: UnitRecord,
    pub signed_spec: Vec<u8>,
    pub config: RepoConfig,
    pub base_sha: String,
    pub waits_for_slot: bool,
}

#[async_trait]
pub trait Admit: Send + Sync {
    async fn admit(&self, request: &DispatchRequest, now_ms: i64) -> Result<Admitted, Refusal>;
}

#[derive(Debug, Clone, PartialEq)]
pub enum HarnessStatus {
    Unknown,
    Unhealthy {
        error: String,
    },
    Healthy {
        capabilities: Capabilities,
    },
}

pub trait HarnessCatalog: Send + Sync {
    fn status(&self, name: &str) -> HarnessStatus;
    fn eligibility(
        &self,
        name: &str,
        tier: Tier,
        opt_in: bool,
        profile: Option<&str>,
    ) -> Result<(), String>;
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConfigError {
    #[error("the file does not read: {0}")]
    Malformed(String),
    #[error("image {image:?} is not pinned to a digest")]
    ImageNotPinned { image: String },
    #[error("{key}: {path:?} is not a usable path")]
    BadPath { key: &'static str, path: String },
    #[error("{key} is empty")]
    Empty { key: &'static str },
}

// crates/fleet-admission/src/admission.rs
pub const REPO_CONFIG_PATH: &str = ".reqdrive/config.toml";

pub fn parse_repo_config(toml_text: &str) -> Result<RepoConfig, ConfigError>;

pub struct Admission {
    /* private */
}

impl Admission {
    pub fn new(
        config: AdmissionConfig,
        units: Arc<dyn UnitStore>,
        signatures: Arc<dyn SignatureStore>,
        forge: Arc<dyn RepoForge>,
        harnesses: Arc<dyn HarnessCatalog>,
        slots: Arc<Semaphore>,
    ) -> Result<Self, StoreError>;

    pub fn mint_unit_id(&self) -> String;
}
````

**Invariants**

- I1. The checks run in the order of `Refusal`'s variants, and a request is refused for the
  first check it fails.
- I2. A refused request stores nothing and reserves nothing.
- I3. The forge is never asked about a repository that is not on the allowlist, and the
  allowlist compares slugs exactly.
- I4. A unit is admitted only when a signature is stored for exactly the request's bytes in
  exactly the request's repository, and the signed spec names the request's work item.
- I5. A unit's tier, scope and spec id come from the signed bytes, and its test command and
  configuration from the repository at the base commit; no request field can set them.
- I6. A unit's caps are at most the configured ceilings; a request can lower one and can never
  raise one, zero it, or set one that is not a number.
- I7. An error from the store refuses the request: a spend that cannot be read is not zero,
  and a reservation that cannot be written is not an admission.
- I8. One idempotency key starts at most one unit, however the requests are timed.
- I9. Admission takes no concurrency slot; it only reports whether one is free.
- I10. The crate depends on no workspace crate other than `fleet-core`, `harness-protocol`,
  `factory-spec`, `fleet-store` and `fleet-forge`, and declares a `testkit` feature.

**Hidden decisions.** How a unit id is minted; what a unit's one-line task is taken from; the
wording of refusals; whether the spend is read before the reservation re-checks it.

**Gates**

| id | guards | mechanism | location | command | blocks |
|---|---|---|---|---|---|
| workspace.G1 | I10 | Build graph: the dependency allowlist and the `testkit` feature | `xtask/src/deps.rs#pub const RULES` | `cargo xtask test static` | merge |

**Unenforced (planned)**

| planned id | guards | mechanism | location | command | built by |
|---|---|---|---|---|---|
| fleet-admission.G1 | I6, I8 | Contract suite: `admit_contract` against the real admission | `crates/fleet-admission/tests/contract.rs#the_real_admission_passes_the_admit_contract` | `cargo xtask test contract` | behaviour 1 |
| fleet-admission.G2 | I1, I2, I3 | Locked tests of the order | `crates/fleet-admission/tests/contract.rs#a_request_wrong_in_several_ways_is_refused_for_the_first_of_them` | `cargo xtask test contract` | behaviours 5, 27, 28 |
| fleet-admission.G3 | I4, I5 | Locked tests of the signature check | `crates/fleet-admission/tests/contract.rs#bytes_nobody_signed_are_refused` | `cargo xtask test contract` | behaviours 2, 3, 14 to 16 |
| fleet-admission.G4 | I7 | Locked tests for issue #74 | `crates/fleet-admission/tests/contract.rs#a_reservation_that_cannot_be_recorded_refuses_the_request` | `cargo xtask test contract` | behaviours 22, 23 |

I9 is checked by behaviour 26 and has no gate of its own.

### Behaviours

All in `crates/fleet-admission/tests/contract.rs`. Each starts from
`fleet_admission::testkit::Desk::new`, whose request is admitted.

What an admitted unit is:

1. `the_real_admission_passes_the_admit_contract`
2. `an_admitted_unit_is_stored_queued_with_everything_read_from_the_signed_bytes`
3. `the_tier_of_a_t2_spec_is_what_eligibility_is_asked_about`
4. `unit_ids_continue_after_the_highest_one_in_the_store`
35. `ids_minted_for_the_old_request_shape_come_from_the_same_counter`

The checks, in order:

5. `a_repository_that_is_not_on_the_allowlist_is_refused_before_it_is_read`
6. `the_allowlist_compares_slugs_exactly`
7. `a_repository_with_no_configuration_file_is_not_onboarded`
8. `a_configuration_file_that_does_not_read_is_not_onboarded`
9. `the_configuration_is_read_at_the_base_commit_the_unit_is_given`
10. `a_preset_this_build_does_not_have_is_refused`
11. `a_harness_that_is_unknown_or_unhealthy_is_refused_by_name`
12. `a_harness_that_does_not_declare_the_repositorys_preset_is_refused`
13. `a_harness_on_another_version_of_the_preset_rules_is_refused`
14. `bytes_nobody_signed_are_refused`
15. `a_signature_in_another_repository_does_not_count`
16. `a_signed_spec_for_another_work_item_is_refused`

Caps:

17. `the_ceilings_come_from_configuration`
18. `a_wall_clock_cap_may_be_lowered_and_not_raised`
19. `a_cap_that_is_zero_negative_or_not_a_number_is_refused`

The spend ceiling and issue #74:

20. `at_the_spend_ceiling_a_request_is_refused_and_nothing_is_stored`
21. `only_units_first_written_inside_the_window_count`
22. `a_reservation_that_cannot_be_recorded_refuses_the_request` (Review Focus 4)
23. `committed_spend_that_cannot_be_read_refuses_the_request`

Eligibility and the slot:

24. `an_ineligible_harness_is_refused_with_its_reason_and_nothing_is_reserved`
25. `the_opt_in_and_the_profile_are_passed_to_eligibility_and_stored`
26. `with_every_slot_taken_a_unit_is_admitted_and_told_it_will_wait`

The order of refusals:

27. `a_request_wrong_in_several_ways_is_refused_for_the_first_of_them`
28. `a_used_key_is_refused_before_anything_else_is_looked_at`
29. `two_requests_with_one_key_at_the_same_moment_start_one_unit` (Review Focus 4)

The repository configuration file:

30. `the_documented_configuration_file_parses_to_the_protocols_type`
31. `a_configuration_file_read_with_crlf_line_endings_is_the_same_configuration`
32. `an_image_named_by_tag_is_refused`
33. `paths_that_leave_the_repository_or_use_a_wildcard_are_refused`
34. `a_missing_command_an_empty_one_or_an_unknown_key_is_refused`

Run: `cargo test -p fleet-admission --test contract`. Expected before the implementation: 35
failed, each with `not implemented: lane CC-ADMISSION builds this`.

`crates/fleet-admission/tests/contract.rs`:

````rust
//! Lane CC-ADMISSION: the real admission, on the in-memory store and forge and a scripted
//! harness catalog. No Docker, no git, no network.
//!
//! Each test starts from `Desk::new`, whose request is admitted, changes one thing, and asserts
//! the refusal.

use factory_spec::testkit::SpecBuilder;
use fleet_admission::testkit::{
    admit_contract, capabilities, repo_config, Desk, PRESETS_VERSION, REPO_CONFIG_TOML,
};
use fleet_admission::{
    parse_repo_config, Admit, ConfigError, HarnessStatus, Refusal, REPO_CONFIG_PATH,
};
use fleet_core::Tier;
use fleet_store::testkit::{record, signature};
use fleet_store::{SignatureStore, UnitStore};

const NOW: i64 = 1_700_000_000_000;

async fn refusal(desk: &Desk, key: &str) -> Refusal {
    desk.admission()
        .admit(&desk.request(key), NOW)
        .await
        .expect_err("the request was admitted")
}

// --- 1. the seam's contract, and what an admitted unit is ---

#[tokio::test]
async fn the_real_admission_passes_the_admit_contract() {
    admit_contract(|| {
        let desk = Desk::new();
        (desk.admission(), desk.request("key-1"))
    })
    .await;
}

#[tokio::test]
async fn an_admitted_unit_is_stored_queued_with_everything_read_from_the_signed_bytes() {
    let desk = Desk::new();
    let admitted = desk
        .admission()
        .admit(&desk.request("key-1"), NOW)
        .await
        .unwrap();
    let stored = desk.store.get_record(&admitted.unit_id).unwrap().unwrap();
    assert_eq!(stored, admitted.record);
    assert_eq!(stored.row.unit_id, "u1");
    assert_eq!(stored.row.phase, "queued");
    assert_eq!(
        stored.row.tier, "t1",
        "the tier comes from the spec, not from the request"
    );
    assert_eq!(stored.row.mode, "real");
    assert_eq!(stored.row.repo_url, desk.repo.url);
    assert_eq!(stored.row.base_branch, "main");
    assert_eq!(stored.row.branch, "factory/1-u1");
    assert_eq!(stored.row.test_cmd, "npm test");
    assert_eq!(stored.row.usd_cap, 5.0);
    assert_eq!(stored.row.wall_clock_secs, 1800);
    assert_eq!(stored.row.min_review_rounds, 1);
    assert_eq!(stored.ext.spec_id.as_deref(), Some("SPEC-0001"));
    assert_eq!(stored.ext.base_sha.as_deref(), Some(desk.base_sha.as_str()));
    assert_eq!(admitted.signed_spec, desk.spec_bytes);
    assert_eq!(admitted.config, repo_config());
    assert_eq!(admitted.base_sha, desk.base_sha);
    assert!(!admitted.waits_for_slot);
    assert_eq!(
        desk.store.committed_spend(0).unwrap(),
        5.0,
        "the cap is reserved"
    );
}

#[tokio::test]
async fn the_tier_of_a_t2_spec_is_what_eligibility_is_asked_about() {
    let mut desk = Desk::new();
    desk.spec_bytes = SpecBuilder::new().tier("t2").build();
    let signed = signature("owner/sandbox", "SPEC-0001", &desk.spec_bytes);
    desk.spec_sha256 = signed.sha256.clone();
    desk.store.put_signature(&signed).unwrap();
    let admitted = desk
        .admission()
        .admit(&desk.request("key-1"), NOW)
        .await
        .unwrap();
    assert_eq!(admitted.record.row.tier, "t2");
    assert_eq!(
        desk.catalog.asked(),
        [("reqdrive".to_string(), Tier::T2, false, None)]
    );
}

#[tokio::test]
async fn unit_ids_continue_after_the_highest_one_in_the_store() {
    let desk = Desk::new();
    desk.store.seed(record("u41"), 0);
    let admitted = desk
        .admission()
        .admit(&desk.request("key-1"), NOW)
        .await
        .unwrap();
    assert_eq!(admitted.unit_id, "u42");
}

#[tokio::test]
async fn ids_minted_for_the_old_request_shape_come_from_the_same_counter() {
    let desk = Desk::new();
    desk.store.seed(record("u41"), 0);
    let admission = desk.admission();
    assert_eq!(admission.mint_unit_id(), "u42");
    let admitted = admission.admit(&desk.request("key-1"), NOW).await.unwrap();
    assert_eq!(admitted.unit_id, "u43");
    assert_eq!(admission.mint_unit_id(), "u44");
}

// --- 2. the checks, in order ---

#[tokio::test]
async fn a_repository_that_is_not_on_the_allowlist_is_refused_before_it_is_read() {
    let mut desk = Desk::new();
    desk.config.repos.clear();
    assert_eq!(
        refusal(&desk, "key-1").await,
        Refusal::RepoNotAllowed {
            repo: "owner/sandbox".into()
        }
    );
    assert!(
        desk.forge.calls().is_empty(),
        "the forge was asked about a repository not allowed"
    );
}

#[tokio::test]
async fn the_allowlist_compares_slugs_exactly() {
    let desk = Desk::new();
    let mut request = desk.request("key-1");
    for slug in [
        "owner/sandbox-evil",
        "owner/sandbo",
        "other/sandbox",
        "OWNER/SANDBOX/",
    ] {
        request.repo = slug.into();
        let refused = desk.admission().admit(&request, NOW).await.unwrap_err();
        assert_eq!(refused.code(), "repo_not_allowed", "{slug}");
    }
}

#[tokio::test]
async fn a_repository_with_no_configuration_file_is_not_onboarded() {
    let desk = Desk::new();
    desk.forge
        .commit_base(&desk.repo, &[(REPO_CONFIG_PATH, None)]);
    match refusal(&desk, "key-1").await {
        Refusal::NotOnboarded { repo, detail } => {
            assert_eq!(repo, "owner/sandbox");
            assert!(detail.contains(REPO_CONFIG_PATH), "{detail}");
        }
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn a_configuration_file_that_does_not_read_is_not_onboarded() {
    let desk = Desk::new();
    desk.forge
        .commit_base(&desk.repo, &[(REPO_CONFIG_PATH, Some("preset = "))]);
    assert_eq!(refusal(&desk, "key-1").await.code(), "not_onboarded");
}

#[tokio::test]
async fn the_configuration_is_read_at_the_base_commit_the_unit_is_given() {
    let desk = Desk::new();
    let newer = REPO_CONFIG_TOML.replace("npm test", "npm run test:ci");
    let tip = desk
        .forge
        .commit_base(&desk.repo, &[(REPO_CONFIG_PATH, Some(newer.as_str()))]);
    let admitted = desk
        .admission()
        .admit(&desk.request("key-1"), NOW)
        .await
        .unwrap();
    assert_eq!(admitted.base_sha, tip);
    assert_eq!(admitted.config.commands.test, "npm run test:ci");
    assert_eq!(admitted.record.row.test_cmd, "npm run test:ci");
}

#[tokio::test]
async fn a_preset_this_build_does_not_have_is_refused() {
    let desk = Desk::new();
    let python = REPO_CONFIG_TOML.replace("preset = \"node\"", "preset = \"python\"");
    desk.forge
        .commit_base(&desk.repo, &[(REPO_CONFIG_PATH, Some(python.as_str()))]);
    assert_eq!(
        refusal(&desk, "key-1").await,
        Refusal::PresetUnsupported {
            preset: "python".into()
        }
    );
}

#[tokio::test]
async fn a_harness_that_is_unknown_or_unhealthy_is_refused_by_name() {
    let desk = Desk::new();
    let mut request = desk.request("key-1");
    request.harness = "nobody".into();
    assert_eq!(
        desk.admission().admit(&request, NOW).await,
        Err(Refusal::HarnessUnknown {
            harness: "nobody".into()
        })
    );
    desk.catalog.set(
        "reqdrive",
        HarnessStatus::Unhealthy {
            error: "reqdrive not found at C:/tools/reqdrive.exe".into(),
        },
    );
    match refusal(&desk, "key-2").await {
        Refusal::HarnessUnhealthy { harness, error } => {
            assert_eq!(harness, "reqdrive");
            assert!(error.contains("not found"));
        }
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn a_harness_that_does_not_declare_the_repositorys_preset_is_refused() {
    let desk = Desk::new();
    let mut declared = capabilities();
    declared.presets.retain(|p| p.name != "node");
    desk.catalog.set(
        "reqdrive",
        HarnessStatus::Healthy {
            capabilities: declared,
        },
    );
    assert_eq!(
        refusal(&desk, "key-1").await,
        Refusal::PresetNotDeclared {
            preset: "node".into(),
            harness: "reqdrive".into()
        }
    );
}

#[tokio::test]
async fn a_harness_on_another_version_of_the_preset_rules_is_refused() {
    let desk = Desk::new();
    let mut declared = capabilities();
    for preset in &mut declared.presets {
        preset.version = "0.2.0".into();
    }
    desk.catalog.set(
        "reqdrive",
        HarnessStatus::Healthy {
            capabilities: declared,
        },
    );
    assert_eq!(
        refusal(&desk, "key-1").await,
        Refusal::PresetsVersionMismatch {
            harness: "reqdrive".into(),
            ours: PRESETS_VERSION.into(),
            theirs: "0.2.0".into()
        }
    );
}

#[tokio::test]
async fn bytes_nobody_signed_are_refused() {
    let desk = Desk::new();
    let mut request = desk.request("key-1");
    request.spec_sha256 = "0".repeat(64);
    assert_eq!(
        desk.admission().admit(&request, NOW).await,
        Err(Refusal::NotSigned {
            repo: "owner/sandbox".into(),
            spec_sha256: "0".repeat(64)
        })
    );
    assert_eq!(desk.store.list_records().unwrap().len(), 0);
}

#[tokio::test]
async fn a_signature_in_another_repository_does_not_count() {
    let mut desk = Desk::new();
    desk.spec_bytes = SpecBuilder::new().id("SPEC-0002").build();
    let elsewhere = signature("other/repo", "SPEC-0002", &desk.spec_bytes);
    desk.spec_sha256 = elsewhere.sha256.clone();
    desk.store.put_signature(&elsewhere).unwrap();
    assert_eq!(refusal(&desk, "key-1").await.code(), "not_signed");
}

#[tokio::test]
async fn a_signed_spec_for_another_work_item_is_refused() {
    let desk = Desk::new();
    let mut request = desk.request("key-1");
    request.work_item = "example/sandbox#2".into();
    assert_eq!(
        desk.admission().admit(&request, NOW).await,
        Err(Refusal::WorkItemMismatch {
            request: "example/sandbox#2".into(),
            spec: "example/sandbox#1".into()
        })
    );
}

// --- 3. caps: configuration sets the ceilings, and a request may only lower them ---

#[tokio::test]
async fn the_ceilings_come_from_configuration() {
    let mut desk = Desk::new();
    desk.config.unit_usd_ceiling = 2.5;
    desk.config.unit_wall_clock_ceiling_secs = 600;
    let admitted = desk
        .admission()
        .admit(&desk.request("key-1"), NOW)
        .await
        .unwrap();
    assert_eq!(admitted.record.row.usd_cap, 2.5);
    assert_eq!(admitted.record.row.wall_clock_secs, 600);
}

#[tokio::test]
async fn a_wall_clock_cap_may_be_lowered_and_not_raised() {
    let desk = Desk::new();
    let mut lower = desk.request("key-1");
    lower.wall_clock_secs = Some(300);
    let admitted = desk.admission().admit(&lower, NOW).await.unwrap();
    assert_eq!(admitted.record.row.wall_clock_secs, 300);

    let mut higher = desk.request("key-2");
    higher.wall_clock_secs = Some(1801);
    assert_eq!(
        desk.admission().admit(&higher, NOW).await,
        Err(Refusal::CapAboveCeiling {
            cap: "wall_clock",
            requested: 1801.0,
            ceiling: 1800.0
        })
    );
}

#[tokio::test]
async fn a_cap_that_is_zero_negative_or_not_a_number_is_refused() {
    let desk = Desk::new();
    for (n, bad) in [0.0, -1.0, f64::NAN, f64::INFINITY].into_iter().enumerate() {
        let mut request = desk.request(&format!("key-{n}"));
        request.usd_cap = Some(bad);
        let refused = desk.admission().admit(&request, NOW).await.unwrap_err();
        assert_eq!(refused.code(), "cap_above_ceiling", "{bad}");
    }
    let mut request = desk.request("key-zero-clock");
    request.wall_clock_secs = Some(0);
    assert_eq!(
        desk.admission()
            .admit(&request, NOW)
            .await
            .unwrap_err()
            .code(),
        "cap_above_ceiling",
        "0 would disable the wall clock, which is above every ceiling"
    );
}

// --- 4. the global spend ceiling, and issue 74 ---

#[tokio::test]
async fn at_the_spend_ceiling_a_request_is_refused_and_nothing_is_stored() {
    let desk = Desk::new();
    for n in 0..4 {
        desk.store.seed(record(&format!("seed-{n}")), NOW - 1_000);
    }
    assert_eq!(
        refusal(&desk, "key-1").await,
        Refusal::SpendCeiling {
            committed: 20.0,
            cap: 20.0
        }
    );
    assert_eq!(desk.store.list_records().unwrap().len(), 4);
}

#[tokio::test]
async fn only_units_first_written_inside_the_window_count() {
    let desk = Desk::new();
    let day = 24 * 3600 * 1000;
    for n in 0..4 {
        desk.store.seed(record(&format!("old-{n}")), NOW - day - 1);
    }
    assert!(desk
        .admission()
        .admit(&desk.request("key-1"), NOW)
        .await
        .is_ok());
}

#[tokio::test]
async fn a_reservation_that_cannot_be_recorded_refuses_the_request() {
    // Issue 74: admitting a unit whose reservation was not written lets the ceiling admit
    // more than it should, silently.
    let desk = Desk::new();
    let admission = desk.admission();
    desk.store.fail_writes(true);
    let refused = admission
        .admit(&desk.request("key-1"), NOW)
        .await
        .unwrap_err();
    assert_eq!(refused.code(), "reservation_failed");
    desk.store.fail_writes(false);
    assert_eq!(desk.store.list_records().unwrap().len(), 0);
    // The same key is not burnt by the failure.
    assert!(admission.admit(&desk.request("key-1"), NOW).await.is_ok());
}

#[tokio::test]
async fn committed_spend_that_cannot_be_read_refuses_the_request() {
    // Issue 74, the other half: a failed read is not "nothing spent".
    let desk = Desk::new();
    let admission = desk.admission();
    desk.store.fail_reads(true);
    let refused = admission
        .admit(&desk.request("key-1"), NOW)
        .await
        .unwrap_err();
    assert!(
        matches!(refused.code(), "spend_unknown" | "reservation_failed"),
        "{refused:?}"
    );
    desk.store.fail_reads(false);
    assert_eq!(desk.store.list_records().unwrap().len(), 0);
}

// --- 5. eligibility and the slot ---

#[tokio::test]
async fn an_ineligible_harness_is_refused_with_its_reason_and_nothing_is_reserved() {
    let desk = Desk::new();
    desk.catalog
        .refuse("reqdrive", "isolation is host: the unit needs the opt-in");
    assert_eq!(
        refusal(&desk, "key-1").await,
        Refusal::Ineligible {
            harness: "reqdrive".into(),
            reason: "isolation is host: the unit needs the opt-in".into()
        }
    );
    assert_eq!(desk.store.committed_spend(0).unwrap(), 0.0);
}

#[tokio::test]
async fn the_opt_in_and_the_profile_are_passed_to_eligibility_and_stored() {
    let desk = Desk::new();
    let mut request = desk.request("key-1");
    request.opt_in = true;
    request.profile = Some("claude".into());
    let admitted = desk.admission().admit(&request, NOW).await.unwrap();
    assert_eq!(
        desk.catalog.asked(),
        [(
            "reqdrive".to_string(),
            Tier::T1,
            true,
            Some("claude".to_string())
        )]
    );
    assert!(admitted.record.ext.opt_in);
    assert_eq!(admitted.record.ext.profile.as_deref(), Some("claude"));
}

#[tokio::test]
async fn with_every_slot_taken_a_unit_is_admitted_and_told_it_will_wait() {
    let desk = Desk::new();
    let _held = desk.slots.clone().acquire_many_owned(3).await.unwrap();
    let admitted = desk
        .admission()
        .admit(&desk.request("key-1"), NOW)
        .await
        .unwrap();
    assert!(admitted.waits_for_slot);
    assert_eq!(
        desk.slots.available_permits(),
        0,
        "admission takes no slot itself"
    );
}

// --- 6. the order of refusals ---

#[tokio::test]
async fn a_request_wrong_in_several_ways_is_refused_for_the_first_of_them() {
    let desk = Desk::new();
    desk.catalog.refuse("reqdrive", "ineligible");
    for n in 0..4 {
        desk.store.seed(record(&format!("seed-{n}")), NOW - 1_000);
    }
    // Over the ceiling, ineligible, unsigned, and not allowed: the allowlist speaks first.
    let mut request = desk.request("key-1");
    request.spec_sha256 = "0".repeat(64);
    request.repo = "other/repo".into();
    let refused = desk.admission().admit(&request, NOW).await.unwrap_err();
    assert_eq!(refused.code(), "repo_not_allowed");
    // Allowed again: the signature speaks before the ceiling and before eligibility.
    request.repo = "owner/sandbox".into();
    let refused = desk.admission().admit(&request, NOW).await.unwrap_err();
    assert_eq!(refused.code(), "not_signed");
    // Signed: the ceiling speaks before eligibility.
    let refused = desk
        .admission()
        .admit(&desk.request("key-1"), NOW)
        .await
        .unwrap_err();
    assert_eq!(refused.code(), "spend_ceiling");
    assert!(
        desk.catalog.asked().is_empty(),
        "eligibility was asked of a request already refused"
    );
}

#[tokio::test]
async fn a_used_key_is_refused_before_anything_else_is_looked_at() {
    let mut desk = Desk::new();
    let first = desk
        .admission()
        .admit(&desk.request("key-1"), NOW)
        .await
        .unwrap();
    let calls = desk.forge.calls().len();
    desk.config.repos.clear();
    assert_eq!(
        refusal(&desk, "key-1").await,
        Refusal::DuplicateKey {
            unit_id: first.unit_id
        }
    );
    assert_eq!(desk.forge.calls().len(), calls);
}

#[tokio::test]
async fn two_requests_with_one_key_at_the_same_moment_start_one_unit() {
    let desk = Desk::new();
    let admission = std::sync::Arc::new(desk.admission());
    let request = desk.request("key-1");
    let (a, b) = tokio::join!(
        admission.admit(&request, NOW),
        admission.admit(&request, NOW)
    );
    assert_eq!([a.is_ok(), b.is_ok()].iter().filter(|ok| **ok).count(), 1);
    assert_eq!(desk.store.list_records().unwrap().len(), 1);
}

// --- 7. the repository configuration file ---

#[test]
fn the_documented_configuration_file_parses_to_the_protocols_type() {
    assert_eq!(parse_repo_config(REPO_CONFIG_TOML), Ok(repo_config()));
    let with_env = format!("{REPO_CONFIG_TOML}\n[env]\nCI = \"true\"\n");
    let parsed = parse_repo_config(&with_env).unwrap();
    assert_eq!(parsed.env.get("CI").map(String::as_str), Some("true"));
}

#[test]
fn a_configuration_file_read_with_crlf_line_endings_is_the_same_configuration() {
    let crlf = REPO_CONFIG_TOML.replace('\n', "\r\n");
    assert_eq!(parse_repo_config(&crlf), Ok(repo_config()));
}

#[test]
fn an_image_named_by_tag_is_refused() {
    for image in [
        "node:22",
        "docker.io/library/node:22-slim",
        "node@sha256:abc",
        "node@sha256:",
    ] {
        let text = REPO_CONFIG_TOML.replace(&repo_config().image, image);
        assert_eq!(
            parse_repo_config(&text),
            Err(ConfigError::ImageNotPinned {
                image: image.into()
            }),
            "{image}"
        );
    }
}

#[test]
fn paths_that_leave_the_repository_or_use_a_wildcard_are_refused() {
    for (key, good, bad) in [
        ("test_report", "\"reports/junit.xml\"", "\"../junit.xml\""),
        ("test_report", "\"reports/junit.xml\"", "\"/tmp/junit.xml\""),
        ("test_dirs", "[\"test/\"]", "[\"test\"]"),
        ("test_dirs", "[\"test/\"]", "[\"test/**/\"]"),
        (
            "manifests",
            "[\"package.json\"]",
            "[\"C:\\\\package.json\"]",
        ),
        (
            "lockfiles",
            "[\"package-lock.json\"]",
            "[\"pkg\\\\package-lock.json\"]",
        ),
    ] {
        let text = REPO_CONFIG_TOML.replace(&format!("{key} = {good}"), &format!("{key} = {bad}"));
        assert_ne!(text, REPO_CONFIG_TOML, "the fixture has {key} = {good}");
        match parse_repo_config(&text) {
            Err(ConfigError::BadPath { key: got, .. }) => assert_eq!(got, key, "{bad}"),
            other => panic!("{key} = {bad}: {other:?}"),
        }
    }
}

#[test]
fn a_missing_command_an_empty_one_or_an_unknown_key_is_refused() {
    let missing = REPO_CONFIG_TOML.replace("lint = \"npm run lint\"\n", "");
    assert!(matches!(
        parse_repo_config(&missing),
        Err(ConfigError::Malformed(_))
    ));
    let empty = REPO_CONFIG_TOML.replace("test = \"npm test\"", "test = \"  \"");
    assert_eq!(
        parse_repo_config(&empty),
        Err(ConfigError::Empty {
            key: "commands.test"
        })
    );
    let unknown = format!("allow_network = true\n{REPO_CONFIG_TOML}");
    assert!(matches!(
        parse_repo_config(&unknown),
        Err(ConfigError::Malformed(_))
    ));
    assert!(matches!(
        parse_repo_config(""),
        Err(ConfigError::Malformed(_))
    ));
}
````

### Implementation notes

**The checks.** `admit` is this sequence; each row returns its refusal and does nothing more.

| # | Check | Refusal |
|---|---|---|
| 1 | `idempotency_key`, trimmed, is not empty | `MissingKey` |
| 2 | `fleet_forge::WorkItemRef::parse(work_item, fingerprint)` | `WorkItemMalformed` |
| 3 | `units.find_by_idempotency_key(key)` is `None` | `DuplicateKey`; a store error is `SpendUnknown` |
| 4 | `request.repo` equals the slug of an entry of `config.repos`, exactly | `RepoNotAllowed` |
| 5 | `forge.resolve_base(repo)`; `forge.read_file(repo, base, REPO_CONFIG_PATH)` is `Some`; the bytes are UTF-8 and `parse_repo_config` accepts them; `config.presets` contains the preset | `Forge` for a forge error; `NotOnboarded` (its detail names the path, or carries the `ConfigError`); `PresetUnsupported` |
| 6 | `harnesses.status(harness)` is `Healthy`; its capabilities list the preset; that entry's version is `config.presets_version` | `HarnessUnknown`; `HarnessUnhealthy`; `PresetNotDeclared`; `PresetsVersionMismatch` |
| 7 | `signatures.get_signature(repo.slug, spec_sha256)` is `Some`; `factory_spec::parse(bytes)` succeeds; the spec's `work_item` equals the request's | `NotSigned`; `SpecUnreadable`; `WorkItemMismatch` |
| 8 | A requested USD cap is a finite number above 0 and at most `unit_usd_ceiling`; a requested wall-clock cap is above 0 and at most `unit_wall_clock_ceiling_secs`. A cap not asked for is the ceiling | `CapAboveCeiling { cap: "usd" }` or `{ cap: "wall_clock" }`, with the request and the ceiling as numbers |
| 9 | `units.committed_spend(now_ms - spend_window_ms)` is below `global_usd_cap` | `SpendCeiling`; a store error is `SpendUnknown` |
| 10 | `harnesses.eligibility(harness, tier, opt_in, profile)`, with the tier of the signed spec | `Ineligible` |
| 11 | Mint the unit id, build the record, `units.reserve_unit(record, now_ms - spend_window_ms, global_usd_cap, now_ms)` | `Err` is `ReservationFailed`; `OverCap` is `SpendCeiling`; `DuplicateKey` is `DuplicateKey` |

Step 9 reads the spend so that an over-ceiling request is refused before eligibility is asked
(behaviour 27); step 11's reservation checks it again, atomically, which is the check that
counts when two requests race.

**The record** of an admitted unit: `unit_id` is `u<n>`, `n` from a counter seeded in
`Admission::new` with `units.max_unit_seq() + 1`; `tier` is the spec's tier in lower case;
`task` is the first non-empty line of the spec's intent, cut to 200 characters; `repo_url`,
`repo_slug` and `base_branch` from the allowlist entry; `branch` is
`factory/<work item number>-<unit id>`; `test_cmd` is the configuration's `commands.test`;
`usd_cap` and `wall_clock_secs` from step 8; `phase` is `queued`; `mode` is `real`;
`min_review_rounds` from the configuration of admission; and in the extension the harness, the
work item, the spec id, the spec hash, the base commit, the profile, the opt-in and the key.
`Admitted::waits_for_slot` is `slots.available_permits() == 0`, read after the reservation.

A counter value is used once even when the reservation then fails: an id is never reused.
`mint_unit_id` takes the next value of the same counter and stores nothing: lane CC-WIRE calls
it for a mission in the old request shape, which is how the two paths share one sequence.

**`parse_repo_config`.** Deserialise into a private struct that mirrors
`harness_protocol::RepoConfig` field for field with `#[serde(deny_unknown_fields)]` on it and
on its `commands` table (the protocol's type accepts unknown fields, which is right on a wire
and wrong for a file a person writes), then check, in this order: `image` contains `@sha256:`
followed by exactly 64 hex digits (`ImageNotPinned`); `test_report` and every entry of
`test_dirs`, `manifests` and `lockfiles` is a relative path with `/` separators, no `..`
segment, no drive letter, no backslash and none of `*`, `?`, `[`, and every entry of `test_dirs`
ends in `/` (`BadPath`, with the key's name); each of the five commands and `test_report` is
not blank (`Empty`, with `commands.<name>` or `test_report`). Anything `toml` rejects is
`Malformed` with its message. `[env]` may be absent.

**Pitfalls.** `f64` comparisons: `NaN > ceiling` is false, so test `is_finite()` first. The
spend window's start is `now_ms - spend_window_ms`, inclusive, as today. Do not hold any lock
across the two forge calls: `admit` is `async` and several run at once; the only thing that
makes two of them safe is step 11. The forge calls of step 5 are the slow part; steps 1 to 4
are deliberately before them.

### Verify

| Command | Expected |
|---|---|
| `cargo test -p fleet-admission` | lib (4) and `contract` (35) pass |
| `cargo test -p fleet-admission --features testkit` | the same |
| `cargo xtask test static --root-only`, `unit`, `contract` | exit 0 |
| *Git-Bash:* `grep -rn "unimplemented!" crates/fleet-admission/src` | no output |
| *Git-Bash:* `grep -n "5\.0\|1800\|node --test\|command-center-agent-sandbox" crates/fleet-admission/src/admission.rs` | no output: no cap, command or repository is hard-coded (issue #90) |
| `git diff --name-only origin/factory/m1...HEAD` | only paths under **Owns** |

### Done when

- [ ] Behaviours 1 to 35 pass; no `unimplemented!` is left in `crates/fleet-admission/src`.
- [ ] The pull request names the registry rows it wants and says it does not close #74 or #90:
      both close with lane CC-WIRE, which puts this crate behind the route.

---

## Lane CC-SUPERVISOR

**Owns:** `crates/fleet-supervisor/src/run.rs`, `eligibility.rs`, `registry.rs` (the bodies),
the body of `ProcessConnector::connect` in `connect.rs`, `crates/fleet-supervisor/tests/**`. It
may add private modules. In `testkit.rs` it deletes one test,
`the_rig_raises_the_panic_of_the_run_it_started`, which only holds while `run` is unwritten; it
changes nothing else there, nor `types.rs`, `lib.rs`, or the traits and types of `connect.rs`.

**Reads:** the supervisor spec, all of it; `crates/harness-protocol/README.md` (what a harness
must do; the nine obligations protocol 0.2 adds); `crates/harness-protocol/src/monitor.rs`;
`crates/fleet-core/src/transition.rs` and `gate.rs`; `crates/fleetd/src/driver.rs` (the loop
this replaces: its `MergeCheck` and `PrOpen` steps at lines 649 to 733, its terminal events
at 743 to 761, its mergeability poll at 766 to 800, and `fail_closed` from line 802);
`crates/harness-conformance/src/bin/harness-fake.rs` (the peer the process tests start);
`forms/fleet-supervisor.md`; issues #84 and #85; the finding of spike S5 when it exists.

**Worktree and branch:** `D:\MajorProjects\.swarm-wt\m1-cc-supervisor`, branch
`feat/m1-fleet-supervisor`.

**Needs:** Part 1 merged. Nothing from lanes CC-VERIFY or CC-FORGE: the tests run on
`ScriptedVerifier` and `MemForge`.

**Blocks:** CC-WIRE.

### Form draft: `fleet-supervisor`

**Purpose.** Runs one unit by supervising one harness process at a time, over any byte stream.
The harness runs the agents; this crate keeps what decides: the state machine, the caps, the
oracle gate, halt and resume, and what a result may mean. Without it the agent loop and the
decision that a unit is done would be the same code.

**interface-files:** `crates/fleet-supervisor/src/lib.rs`, `types.rs`, `connect.rs`,
`eligibility.rs`, `registry.rs`, `run.rs`.

**Interfaces** (the signatures of Part 1, Task 7):

````rust
// crates/fleet-supervisor/src/types.rs
pub const REASON_ORACLE_TAMPERING: &str = "oracle tampering";
pub const REASON_AWAITING_SHIP: &str = "awaiting ship";
pub const REASON_VERIFY_STOPPED: &str = "verification stopped";
pub const REASON_AWAITING_SLOT: &str = "awaiting concurrency slot";

pub const DEFAULT_GRACE: Duration = Duration::from_secs(30);
pub const DEFAULT_STALL: Duration = Duration::from_secs(600);
pub const MAX_MERGEABLE_POLLS: u32 = 10;

#[derive(Debug, Clone, PartialEq)]
pub struct UnitSpec {
    pub unit_id: String,
    pub tier: Tier,
    pub task: String,
    pub repo: Repo,
    pub branch: String,
    pub test_cmd: String,
    pub usd_cap: f64,
    pub wall_clock_secs: u64,
    pub min_review_rounds: u32,
    pub work_item: WorkItem,
    pub factory: Option<FactoryInputs>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FactoryInputs {
    pub spec_id: String,
    pub signed_spec: Vec<u8>,
    pub base_sha: String,
    pub scope: Scope,
    pub permitted_dependencies: Vec<String>,
    pub expected_red: Vec<String>,
    pub config: RepoConfig,
    pub profile: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pause {
    pub reason: Option<String>,
    pub from: Phase,
}

#[derive(Debug, Clone)]
pub struct RunCtx {
    pub start_phase: Phase,
    pub cost_usd: f64,
    pub elapsed_ms: u64,
    pub oracle_frozen: bool,
    pub approved_oracle_hash: Option<String>,
    pub freeze: Option<OracleFreeze>,
    pub last_result: Option<UnitResult>,
    pub pause: Option<Pause>,
    pub opt_in: bool,
    pub grace: Duration,
    pub stall: Duration,
    pub slots: Arc<Semaphore>,
    pub work_dir: PathBuf,
}

#[derive(Clone)]
pub struct Deps {
    pub verifier: Arc<dyn EvidenceVerifier>,
    pub forge: Arc<dyn RepoForge>,
}

pub trait Sink: Send {
    fn event(&mut self, event: Event);
    fn stage(&mut self, stage: Stage, status: StageStatus, detail: Option<String>);
    fn frozen(&mut self, freeze: &OracleFreeze);
    fn result(&mut self, result: &UnitResult);
}

// crates/fleet-supervisor/src/connect.rs
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct ConnectError(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Exit {
    pub code: Option<i32>,
    pub description: String,
}

#[async_trait]
pub trait ProcessControl: Send {
    async fn wait(&mut self) -> Exit;
    async fn kill(&mut self);
}

pub struct Connection {
    pub reader: Box<dyn AsyncRead + Send + Unpin>,
    pub writer: Box<dyn AsyncWrite + Send + Unpin>,
    pub diagnostics: UnboundedReceiver<String>,
    pub control: Box<dyn ProcessControl>,
}

#[async_trait]
pub trait Connector: Send + Sync {
    fn name(&self) -> &str;
    async fn connect(&self) -> Result<Connection, ConnectError>;
}

#[derive(Debug, Clone)]
pub struct ProcessConnector {
    /* private */
}

impl ProcessConnector {
    pub fn new(entry: HarnessEntry) -> Self;
}

// crates/fleet-supervisor/src/eligibility.rs
pub fn eligibility(
    tier: Tier,
    capabilities: &Capabilities,
    opt_in: bool,
    profile: Option<&str>,
) -> Result<(), String>;

// crates/fleet-supervisor/src/registry.rs
pub const FAKE_HARNESS: &str = "fake";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessEntry {
    pub name: String,
    pub command: Vec<String>,
    pub env: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ProbeState {
    Unprobed,
    Healthy {
        harness: HarnessInfo,
        capabilities: Capabilities,
    },
    Unhealthy {
        error: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RegistryError {
    #[error("the registry file could not be read: {0}")]
    Unreadable(String),
    #[error("the registry file does not read: {0}")]
    Malformed(String),
    #[error("harness {name:?} is registered twice")]
    Duplicate { name: String },
    #[error("harness {name:?} has an empty command")]
    EmptyCommand { name: String },
}

pub fn fake_binary_beside(exe: &Path) -> PathBuf;

#[derive(Clone)]
pub struct Registry {
    /* private */
}

impl Registry {
    pub fn new(entries: Vec<HarnessEntry>) -> Self;

    pub fn default_fake() -> Self;

    pub fn from_toml(text: &str) -> Result<Self, RegistryError>;

    pub fn load(path: &Path) -> Result<Self, RegistryError>;

    pub fn merged(self, other: Registry) -> Self;

    pub fn entry(&self, name: &str) -> Option<HarnessEntry>;

    pub fn states(&self) -> Vec<(String, ProbeState)>;

    pub fn state(&self, name: &str) -> Option<ProbeState>;

    pub async fn probe(&self, grace: Duration);

    pub fn connector(&self, name: &str) -> Option<Arc<dyn Connector>>;
}

// crates/fleet-supervisor/src/run.rs
pub async fn run(
    connector: Arc<dyn Connector>,
    deps: Deps,
    spec: UnitSpec,
    ctx: RunCtx,
    commands: UnboundedReceiver<Command>,
    sink: Box<dyn Sink>,
) -> Phase;
````

**Invariants**

- I1. A unit's phase changes only by `fleet_core::transition`; a message the state machine or
  the supervisor spec's rules forbid fails the unit as a protocol violation, with the line.
- I2. The verifier is called only for a `pr_open` result received in `MergeCheck` or a
  `no_change` result received in `Checking` after `empty_diff`, after the harness process has
  ended; never for `failed` or `needs_human`.
- I3. A pull request is opened only after a `Verified` verdict (and, at T3, a `Ship` command),
  for the commit the result named, with the body the verifier was shown, at most once.
- I4. A verdict of `Unrunnable` or `TestsFailed` neither finishes nor fails a unit: it pauses
  it, and `Reverify` runs verification again on the same result without starting a harness.
- I5. Spend is the stored cost plus every metric of this spawn, counted even while stopping;
  a breach is raised when a cap is exceeded, once per stop.
- I6. No clock runs before `unit/start` is acknowledged, while an oracle gate is pending, or
  after the result.
- I7. A unit that waits for a person or for a slot holds no slot; an approved gate is answered
  only once the unit holds a slot again.
- I8. From the moment a stop begins, nothing the harness sends is acted on except its spend,
  and a pending gate is never answered.
- I9. A resume that fails before its transition leaves the unit paused; a unit paused for
  oracle tampering is never resumed.
- I10. A harness process, and every process it started, does not outlive the run that started it.
- I11. Eligibility is decided from the capabilities the unit's own harness process returned.
- I12. A line longer than `harness_protocol::MAX_LINE_BYTES` is a violation and is never held
  in memory whole.
- I13. The run writes to no store: everything it reports goes through its `Sink`.
- I14. The crate depends on no workspace crate other than `fleet-core`, `harness-protocol`,
  `fleet-forge` and `fleet-verify`, and declares a `testkit` feature.

**Hidden decisions.** The loop's internal states; how lines are read and buffered; how a
process tree is ended on each operating system; the registry file's location (the caller gives
a path); the wording of error details beyond the fragments the tests pin.

**Gates**

| id | guards | mechanism | location | command | blocks |
|---|---|---|---|---|---|
| workspace.G1 | I14 | Build graph: the dependency allowlist and the `testkit` feature | `xtask/src/deps.rs#pub const RULES` | `cargo xtask test static` | merge |
| workspace.G3 | I1 (the harness side) | Conformance suite: the kit against `harness-fake` | `crates/harness-conformance/src/cases.rs` | `cargo xtask test contract` | merge |

**Unenforced (planned)**

| planned id | guards | mechanism | location | command | built by |
|---|---|---|---|---|---|
| fleet-supervisor.G1 | I1, I2, I3, I11, I12, I13 | Locked tests of a unit's life | `crates/fleet-supervisor/tests/life.rs#a_t1_unit_runs_to_done_and_one_pull_request_is_opened` | `cargo xtask test contract` | `life.rs` |
| fleet-supervisor.G2 | I5, I6, I7 | Locked tests of the gate, the caps and the clocks | `crates/fleet-supervisor/tests/gate_and_caps.rs#an_approved_gate_is_not_answered_until_the_unit_has_its_slot_again` | `cargo xtask test contract` | `gate_and_caps.rs` |
| fleet-supervisor.G3 | I8, I9 | Locked tests of the command matrix | `crates/fleet-supervisor/tests/commands.rs#what_a_harness_sends_after_it_was_told_to_halt_is_discarded_except_its_spend` | `cargo xtask test contract` | `commands.rs` |
| fleet-supervisor.G4 | I2, I3, I4 | Locked tests of verdicts and re-verification | `crates/fleet-supervisor/tests/verdicts.rs#reverify_runs_verification_again_on_the_same_result_without_starting_a_harness` | `cargo xtask test contract` | `verdicts.rs` |
| fleet-supervisor.G5 | I10 | Locked test with a real process | `crates/fleet-supervisor/tests/process_it.rs#dropping_the_run_ends_the_harness_process_and_everything_it_started` | `cargo xtask test integration` | `process_it.rs` |

### Where milestone 1 departs from the supervisor spec

The spec stands as written. These are the differences, and each has a test.

| # | Spec | Milestone 1 | Tests |
|---|---|---|---|
| D1 | `harness_supervisor::run(entry, verifier, forge, spec, ctx, commands, events)` in `fleetd`, numbering its own events (4) | `fleet_supervisor::run(connector, deps, spec, ctx, commands, sink)`: a `Connector` gives the byte streams, a `Sink` takes the reports, and nothing here numbers or stores an event (5) | every test, through the rig |
| D2 | Protocol 0.1 | Protocol 0.2: `initialize` offers `accepted_versions`, and a reply on another minor is refused; the work order carries `kind`, `spec`, `source`, `scope`, `config` and the rest; `oracle_frozen` carries a freeze, which is kept and handed to the verifier at every tier | `life.rs`: `initialize_offers_…`, `a_factory_units_work_order_…`, `the_freeze_and_the_result_are_handed_to_the_sink_to_be_kept` |
| D3 | No `stage` event | A `stage` event is forwarded through `Sink::stage`, changes no phase, resets the stall window, and is legal before `provisioned` | `life.rs`: `a_stage_note_is_forwarded_…`; `gate_and_caps.rs`: `a_stage_note_counts_as_activity_…` |
| D4 | Three outcomes (4.8) | A fourth, `needs_human`: legal in an agent-active phase; applies `HarnessStopped`; the verifier is not called; announced with `Blocked{reason: <the stop reason's wire name>, detail: <its detail>}` | `life.rs`: `a_needs_human_result_…` (three tests) |
| D5 | The slot is held from `Provisioning` until the stop sequence or the result (4.1, 4.6) | The slot is also given back when the oracle gate becomes pending. On approval it is taken again first, with `Blocked{"awaiting concurrency slot"}` if it must wait, and only then is the gate answered and `OracleApproved` applied | `gate_and_caps.rs`: `the_slot_is_given_back_…`, `an_approved_gate_is_not_answered_until_…`, `a_unit_halted_while_it_waits_for_its_slot_back_…` |
| D6 | Five verdicts (3.4, 4.8) | Seven. `Unrunnable` and `TestsFailed` apply `VerifyStopped` (reason `verification stopped`); `Command::Reverify` applies `Reverify{from}` and runs verification again on the kept result; with no kept result it is refused | `verdicts.rs`, from `a_verification_that_could_not_run_…` to the end |
| D7 | A unit paused for tampering is resumed with a fresh gate (4.6, 6 item 4) | `Resume` is refused for it: the oracle of a unit is frozen only once, so that unit is abandoned and a new one started | `commands.rs`: `a_unit_stopped_for_oracle_tampering_cannot_be_resumed_only_abandoned` |
| D8 | After a rejected gate the harness may freeze again (4.4) | The reference harness always ends the unit with a `failed` result; the supervisor needs no change and a test pins the sequence | `gate_and_caps.rs`: `a_rejected_oracle_is_answered_false_…` |
| D9 | `Forge::open_pr(branch)` and `poll_mergeable` (4.8) | `RepoForge::open_pr(&PrRequest)` with `fleet_forge::pr_body` for an issue work item, and `pr_state` for mergeability; the body is given to the verifier first | `verdicts.rs`: `a_factory_unit_is_verified_against_the_signed_bytes_…` |
| D10 | The work order is built from `UnitSpec` (4) | For a unit with `factory` inputs the run first writes the signed bytes under `work_dir` and asks the forge for the source bundle; if either fails the unit fails before any harness is started | `life.rs`: `a_factory_units_work_order_…`, `a_source_bundle_that_cannot_be_made_…` |
| D11 | Awaiting Ship is in memory and lost on restart (4.6) | `RunCtx::pause` and `last_result` carry it: a unit brought back awaiting ship can be shipped, and one stopped by verification can be verified again | `commands.rs`: `ship_is_accepted_only_from_awaiting_ship`; `verdicts.rs`: `a_unit_brought_back_from_the_store_…` |
| D12 | A registry built in code (3.3) | Also `Registry::from_toml` and `merged`; the probe and the default entry are as specified | `registry.rs` |
| D13 | "raising caps arrives in SP-3" in the exhausted-cap error (4.6) | The error names the cap and says it is exhausted; raising a cap is milestone 2 | `commands.rs`: `a_unit_at_an_exhausted_cap_is_not_resumed_…` |

Not built here: child units and `Integrated`, auxiliary runs (`draft_ready` is a protocol
violation in milestone 1), holdouts, `unit/resume` on the wire, push delivery.

### Behaviours

Five contract-tier files and one integration-tier file. Every test in the first four runs on a
paused clock against a scripted peer.

**The supervisor spec's test list (section 6 item 4), row by row:**

| Spec row | Tests |
|---|---|
| Happy path T1, T2, T3 | `life.rs`: `a_t1_unit_runs_to_done_and_one_pull_request_is_opened`, `a_t2_unit_waits_at_the_oracle_gate_and_runs_on_when_it_is_approved`, `a_t3_unit_is_verified_then_waits_for_ship_and_opens_no_pull_request_until_then` |
| Eligibility | `life.rs`: `a_t2_unit_is_refused_on_a_harness_without_the_oracle_gate_even_with_the_opt_in`, `a_harness_with_host_isolation_or_no_metering_needs_the_opt_in`, `push_delivery_is_refused_whatever_the_opt_in`; `registry.rs`: the five `eligibility` tests |
| The T2 gate | `gate_and_caps.rs`: `at_t2_oracle_frozen_alone_leaves_the_unit_in_spec`, `the_gate_request_announces_the_proposal_before_the_phase_changes`, `a_breach_pauses_the_unit_in_every_phase_a_harness_can_be_alive_in` (the metric between the freeze and the gate), `approving_before_the_gate_exists_is_an_error_and_changes_nothing`, `approving_with_edited_files_is_refused_and_the_gate_stays_pending`, `a_gate_request_with_no_oracle_frozen_before_it_is_a_violation` |
| Startup failures | `life.rs`: `a_harness_that_never_answers_initialize_is_killed_after_the_grace_period`, `initialize_offers_this_protocol_version_and_a_harness_on_another_minor_fails_the_unit`, `a_harness_that_never_acknowledges_unit_start_is_killed_after_the_grace_period`, `a_harness_that_cannot_be_started_fails_the_unit` |
| Caps | `gate_and_caps.rs`: `metrics_are_summed_on_what_earlier_spawns_spent_and_a_breach_pauses_the_unit`, `a_breach_pauses_the_unit_in_every_phase_a_harness_can_be_alive_in`, `agent_time_past_the_wall_clock_cap_pauses_the_unit`, `metrics_that_arrive_while_a_breach_is_being_handled_are_counted_and_raise_no_second_breach`, `an_unanswered_gate_waits_without_limit_and_no_clock_runs_while_it_does`, `no_clock_runs_while_a_unit_waits_for_a_concurrency_slot`, `a_harness_that_sends_no_event_for_the_stall_window_is_stopped_and_the_unit_pauses`, `the_clocks_stop_at_the_result_so_a_slow_verification_cannot_stall_the_unit`; `life.rs`: `no_provisioned_within_the_stall_window_fails_the_unit`; `commands.rs`: `a_resumed_harness_waits_for_a_slot_without_any_clock_running` |
| An unmetered, opted-in harness | `gate_and_caps.rs`: `a_harness_that_never_meters_still_has_its_agent_time_recorded_when_the_spawn_ends` |
| Violations | `gate_and_caps.rs`: `a_gate_request_at_t1_is_a_violation`; `life.rs`: `a_review_round_out_of_sequence_is_a_protocol_violation`, `an_observation_the_state_machine_rejects_is_a_protocol_violation`, `a_pr_open_result_before_merge_check_is_a_protocol_violation_and_is_not_verified`; `verdicts.rs`: `after_empty_diff_only_closing_messages_are_legal` |
| Exit without result; `failed`; no exit after the result | `life.rs`: `a_harness_that_exits_without_a_result_fails_the_unit_with_its_exit_status`, `a_failed_result_fails_the_unit_with_the_failures_own_scope`, `a_harness_that_lingers_after_its_result_is_killed_and_the_result_still_stands` |
| Draining | `commands.rs`: `what_a_harness_sends_after_it_was_told_to_halt_is_discarded_except_its_spend` (keeps emitting; a result after the halt; a gate request while draining), `a_harness_that_refuses_to_halt_is_killed_after_the_grace_period_and_the_unit_is_halted`, `a_harness_that_did_not_declare_halt_is_not_asked_and_just_has_its_input_closed`, `abandon_always_sends_unit_abandon_and_the_unit_fails` |
| Resume | `commands.rs`: `a_t2_unit_halted_while_its_gate_was_pending_is_given_a_fresh_gate_on_resume`, `a_t2_unit_halted_after_a_rejection_is_given_a_fresh_gate_on_resume`, `a_t2_unit_halted_after_approval_resumes_with_no_new_gate`, `a_t1_unit_halted_while_building_resumes_without_freezing_again_and_rounds_restart_at_one`, `a_unit_stopped_for_oracle_tampering_cannot_be_resumed_only_abandoned` (departure D7), `the_harness_is_started_before_a_resume_changes_the_phase_and_a_failure_leaves_it_paused`, `a_unit_at_an_exhausted_cap_is_not_resumed_and_no_harness_is_started` |
| The command matrix | `commands.rs`: `a_unit_waiting_for_a_slot_can_be_halted_and_then_resumed`, `a_unit_waiting_for_a_slot_can_be_abandoned`, `abandon_during_a_halt_in_progress_ends_the_unit_failed_not_halted`, `abandon_while_a_resumed_harness_is_starting_kills_it_and_fails_the_unit`, `while_ships_pull_request_is_being_opened_halt_resume_and_ship_are_refused`, `abandon_while_a_result_is_being_verified_cancels_the_verification`, `ship_is_accepted_only_from_awaiting_ship`, `a_closed_command_channel_keeps_a_live_unit_running_and_fails_a_paused_one_closed`, `a_second_halt_while_one_is_in_progress_is_refused`, `a_paused_unit_can_be_halted_and_either_kind_of_pause_can_be_abandoned`, `a_command_that_does_not_apply_is_refused_by_id_and_phase_and_changes_nothing` |
| Verdicts | `life.rs`: the T1 happy path (`Verified`); `verdicts.rs`: `a_pr_open_that_verifies_as_empty_fails_the_unit`, `a_rejected_delivery_fails_the_unit_naming_the_check_and_is_not_retried`, `tampering_pauses_the_unit_with_the_reason_the_store_folds_on`, `a_merge_conflict_pauses_the_unit_and_opens_no_pull_request`, `evidence_for_another_branch_fails_the_unit_without_calling_the_verifier`, `a_push_delivery_fails_the_unit_without_calling_the_verifier`, `at_t2_the_approved_hash_reaches_the_verifier`, `a_pull_request_that_cannot_be_opened_fails_the_unit`, `a_dirty_pull_request_pauses_the_unit_with_base_moved`, `a_pull_request_that_stays_pending_is_polled_a_fixed_number_of_times_then_a_person_is_asked`, `a_poll_that_fails_is_reported_and_counted_as_one_of_the_polls` |
| `no_change` | `verdicts.rs`: `empty_diff_keeps_the_unit_in_checking_until_the_no_change_result_is_verified`, `a_no_change_result_that_does_not_verify_as_no_change_fails_the_unit`, `a_no_change_result_with_no_empty_diff_before_it_is_a_violation` |
| The `blockers` script | `life.rs`: `a_review_with_blockers_or_below_the_floor_returns_the_unit_to_building`, `each_build_check_and_review_is_announced_with_its_number` |

**Section 6 items 5 and 6** (the process shell; the registry): `process_it.rs` and
`registry.rs`, all of both files.

**The departures** each have the tests named in the table above. Three more tests cover what
neither list does: `life.rs`: `logs_findings_and_errors_are_forwarded_and_change_no_phase`,
`a_line_that_is_not_a_message_fails_the_unit_and_is_quoted_up_to_two_kilobytes`,
`a_line_longer_than_the_protocol_allows_fails_the_unit_without_being_buffered_whole` (Review
Focus 3).

Run: `cargo test -p fleet-supervisor --test life --test gate_and_caps --test commands --test verdicts --test registry`.
Expected before the implementation: 31, 22, 24, 24 and 14 failed, each with
`not implemented: lane CC-SUPERVISOR builds this`. `cargo test -p fleet-supervisor --test process_it`
needs `cargo build -p harness-conformance --bins` first and then fails 7 the same way.

`crates/fleet-supervisor/tests/life.rs`:

````rust
//! Lane CC-SUPERVISOR: a unit's life from `Queued` to its end, over an in-memory pipe.
//! Supervisor spec sections 4.1, 4.2, 4.7 and 4.8, and section 6 item 4: the happy paths,
//! eligibility, startup failures, violations, exits, and the `needs_human` result.
//!
//! No process, no Docker, no network. Every test runs on a paused clock.

use fleet_core::{ArtifactKind, ErrorScope, Event, Phase, Tier};
use fleet_supervisor::testkit::{
    approve, default_freeze, peer_capabilities, ship, Rig, Signal, GATE_HASH, HEAD,
};
use fleet_supervisor::REASON_AWAITING_SHIP;
use fleet_verify::testkit::World;
use harness_protocol::{
    Delivery, ErrorScope as WireScope, Failure, Isolation, LogStream, Metering, Observation,
    Outcome, Stage, StageStatus, Stop, StopReason, UnitEvent, UnitKind, UnitResult, WorkItemKind,
    PROTOCOL_VERSION,
};
use std::time::Duration;

use Phase::*;

fn failed_with(rig: &Rig, scope: ErrorScope, needle: &str) {
    let errors = rig.recorder.errors();
    assert!(
        errors
            .iter()
            .any(|(s, detail)| *s == scope && detail.contains(needle)),
        "no {scope:?} error containing {needle:?} in {errors:?}"
    );
}

fn stopped(reason: StopReason, detail: &str) -> UnitResult {
    UnitResult {
        outcome: Outcome::NeedsHuman,
        evidence: None,
        failure: None,
        stop: Some(Stop {
            reason,
            detail: detail.into(),
            request: Vec::new(),
        }),
    }
}

// --- the happy paths ---

#[tokio::test(start_paused = true)]
async fn a_t1_unit_runs_to_done_and_one_pull_request_is_opened() {
    let mut rig = Rig::builder().start();
    let (mut peer, order) = rig.building().await;
    peer.cycle(1, 0).await;
    peer.deliver(&order).await;
    peer.exit(0);

    assert_eq!(rig.finish().await, Done);
    assert_eq!(
        rig.recorder.phases(),
        [
            Provisioning,
            Spec,
            Building,
            Checking,
            Reviewing,
            MergeCheck,
            PrOpen,
            Done
        ]
    );
    assert_eq!(rig.recorder.done().as_deref(), Some("done"));
    let opened = rig.forge.opened();
    assert_eq!(opened.len(), 1);
    assert_eq!(opened[0].0.branch, "factory/u1");
    assert_eq!(opened[0].0.head_sha, HEAD);
    assert_eq!(opened[0].0.unit_id, "u1");
    let pr_artifacts: Vec<String> = rig
        .recorder
        .events()
        .into_iter()
        .filter_map(|e| match e {
            Event::Artifact {
                kind: ArtifactKind::Pr,
                reference,
            } => Some(reference),
            _ => None,
        })
        .collect();
    assert_eq!(pr_artifacts, [opened[0].1.url.clone()]);
    assert_eq!(
        rig.verifier.inputs().len(),
        1,
        "the result was verified exactly once"
    );
    assert_eq!(rig.slots.available_permits(), 3, "the slot was given back");
}

#[tokio::test(start_paused = true)]
async fn a_t2_unit_waits_at_the_oracle_gate_and_runs_on_when_it_is_approved() {
    let mut rig = Rig::builder().tier(Tier::T2).start();
    let (mut peer, order, gate) = rig.at_gate().await;
    rig.command(approve("c1"));
    let reply = peer.gate_reply(gate).await;
    assert!(reply.approved && reply.edited_test_files.is_none());
    rig.in_phase(Building).await;
    peer.cycle(1, 0).await;
    peer.deliver(&order).await;
    peer.exit(0);
    assert_eq!(rig.finish().await, Done);
    assert_eq!(
        rig.recorder.phases(),
        [
            Provisioning,
            Spec,
            AwaitingOracleApproval,
            Building,
            Checking,
            Reviewing,
            MergeCheck,
            PrOpen,
            Done
        ]
    );
    // The command's id is echoed on the transition it caused.
    assert!(rig.recorder.events().iter().any(|e| matches!(
        e,
        Event::PhaseChanged { to: Building, cmd_id: Some(id), .. } if id == "c1"
    )));
}

#[tokio::test(start_paused = true)]
async fn a_t3_unit_is_verified_then_waits_for_ship_and_opens_no_pull_request_until_then() {
    let mut rig = Rig::builder().tier(Tier::T3).start();
    let (mut peer, order, gate) = rig.at_gate().await;
    rig.command(approve("c1"));
    peer.gate_reply(gate).await;
    rig.in_phase(Building).await;
    peer.cycle(1, 0).await;
    peer.deliver(&order).await;
    peer.exit(0);

    rig.in_phase(NeedsHuman).await;
    assert_eq!(
        rig.recorder.last_reason().as_deref(),
        Some(REASON_AWAITING_SHIP)
    );
    assert_eq!(
        rig.recorder.blocked().last().cloned(),
        Some((
            REASON_AWAITING_SHIP.to_string(),
            None,
            "T3: a human ships the PR".to_string()
        ))
    );
    assert!(
        rig.forge.opened().is_empty(),
        "a T3 pull request was opened before Ship"
    );

    rig.command(ship("c2"));
    assert_eq!(rig.finish().await, Done);
    assert_eq!(rig.forge.opened().len(), 1);
    assert_eq!(
        rig.recorder.phases()[rig.recorder.phases().len() - 3..],
        [NeedsHuman, PrOpen, Done]
    );
}

// --- the handshake and the work order ---

#[tokio::test(start_paused = true)]
async fn initialize_offers_this_protocol_version_and_a_harness_on_another_minor_fails_the_unit() {
    let mut rig = Rig::builder().start();
    let mut peer = rig.peer().await;
    let offered = peer.initialize_as("0.1", peer_capabilities()).await;
    assert_eq!(offered.protocol_version, PROTOCOL_VERSION);
    assert_eq!(offered.accepted(), [PROTOCOL_VERSION]);
    assert_eq!(rig.finish().await, Failed);
    failed_with(&rig, ErrorScope::Harness, "0.1");
    assert!(peer.was_killed());
    assert_eq!(rig.recorder.done().as_deref(), Some("failed"));
}

#[tokio::test(start_paused = true)]
async fn a_demo_units_work_order_has_no_spec_and_carries_its_caps_and_a_roadmap_item() {
    let mut rig = Rig::builder().caps(5.0, 1800).review_floor(2).start();
    let (_peer, order) = rig.started().await;
    assert_eq!(order.unit_id, "u1");
    assert_eq!(order.tier, harness_protocol::Tier::T1);
    assert_eq!(order.branch, "factory/u1");
    assert_eq!(order.work_item.kind, WorkItemKind::RoadmapItem);
    assert_eq!(order.work_item.reference, "demo:u1");
    assert_eq!(order.caps.usd, 5.0);
    assert_eq!(order.caps.wall_clock_secs, 1800);
    assert_eq!(order.caps.min_review_rounds, 2);
    assert_eq!(order.resume, None);
    assert_eq!(order.kind, Some(UnitKind::Build));
    assert!(order.spec.is_none() && order.source.is_none() && order.config.is_none());
}

#[tokio::test(start_paused = true)]
async fn a_factory_units_work_order_carries_the_signed_bytes_and_a_source_bundle_it_made() {
    let dir = tempfile::tempdir().unwrap();
    let world = World::good(dir.path());
    let work_dir = dir.path().join("work");
    let mut rig = Rig::builder().world(&world, &work_dir).start();
    let (_peer, order) = rig.started().await;

    let spec = order.spec.expect("a spec reference");
    assert_eq!(spec.id, "SPEC-0001");
    assert_eq!(spec.signed_hash, factory_hash(&world.spec_bytes));
    assert_eq!(std::fs::read(&spec.bytes_path).unwrap(), world.spec_bytes);
    assert!(std::path::Path::new(&spec.bytes_path).starts_with(&work_dir));
    let source = order.source.expect("a source");
    assert_eq!(source.base_sha, world.base_sha);
    assert!(std::fs::metadata(&source.bundle_path).unwrap().len() > 0);
    assert!(rig.forge.calls().contains(&"source_bundle".to_string()));
    assert_eq!(order.scope, world.order.scope);
    assert_eq!(order.config, world.order.config);
    assert_eq!(
        order.test_cmd, "npm test",
        "test_cmd repeats the configuration's test command"
    );
    assert_eq!(order.work_item, world.order.work_item);
}

fn factory_hash(bytes: &[u8]) -> String {
    harness_protocol::file_sha256(bytes)
}

#[tokio::test(start_paused = true)]
async fn a_source_bundle_that_cannot_be_made_fails_the_unit_before_any_harness_is_started() {
    let dir = tempfile::tempdir().unwrap();
    let world = World::good(dir.path());
    world.forge.fail("source_bundle");
    let mut rig = Rig::builder()
        .world(&world, &dir.path().join("work"))
        .start();
    assert_eq!(rig.finish().await, Failed);
    failed_with(&rig, ErrorScope::System, "source_bundle");
    assert_eq!(rig.connector.connects(), 0);
}

// --- eligibility, from the capabilities this unit's own harness returned ---

#[tokio::test(start_paused = true)]
async fn a_t2_unit_is_refused_on_a_harness_without_the_oracle_gate_even_with_the_opt_in() {
    let mut rig = Rig::builder().tier(Tier::T2).opt_in().start();
    let mut peer = rig.peer().await;
    let mut gateless = peer_capabilities();
    gateless.gates.clear();
    peer.initialize(gateless).await;
    assert_eq!(rig.finish().await, Failed);
    failed_with(&rig, ErrorScope::Harness, "harness scripted refused:");
    assert!(
        peer.recv().await.is_none(),
        "unit/start was sent to an ineligible harness"
    );
}

#[tokio::test(start_paused = true)]
async fn a_harness_with_host_isolation_or_no_metering_needs_the_opt_in() {
    for weaken in [
        (|c: &mut harness_protocol::Capabilities| c.isolation = Isolation::Host) as fn(&mut _),
        |c| c.metering = Metering::None,
    ] {
        let mut weak = peer_capabilities();
        weaken(&mut weak);

        let mut rig = Rig::builder().start();
        let mut peer = rig.peer().await;
        peer.initialize(weak.clone()).await;
        assert_eq!(rig.finish().await, Failed, "{weak:?}");
        failed_with(&rig, ErrorScope::Harness, "refused:");

        let mut rig = Rig::builder().opt_in().start();
        let mut peer = rig.peer().await;
        let order = peer.handshake(weak).await;
        assert_eq!(order.unit_id, "u1", "with the opt-in the unit is started");
    }
}

#[tokio::test(start_paused = true)]
async fn push_delivery_is_refused_whatever_the_opt_in() {
    let mut rig = Rig::builder().opt_in().start();
    let mut peer = rig.peer().await;
    let mut pushing = peer_capabilities();
    pushing.delivery = Delivery::Push;
    peer.initialize(pushing).await;
    assert_eq!(rig.finish().await, Failed);
    failed_with(&rig, ErrorScope::Harness, "push");
}

// --- startup failures ---

#[tokio::test(start_paused = true)]
async fn a_harness_that_cannot_be_started_fails_the_unit() {
    let mut rig = Rig::builder()
        .failing_connects(&["harness-fake not found"])
        .start();
    assert_eq!(rig.finish().await, Failed);
    failed_with(&rig, ErrorScope::Harness, "harness-fake not found");
    assert_eq!(rig.slots.available_permits(), 3);
}

#[tokio::test(start_paused = true)]
async fn a_harness_that_never_answers_initialize_is_killed_after_the_grace_period() {
    let mut rig = Rig::builder().start();
    let peer = rig.peer().await;
    let started = tokio::time::Instant::now();
    assert_eq!(rig.finish().await, Failed);
    assert_eq!(started.elapsed(), Duration::from_secs(30));
    assert!(peer.was_killed());
    failed_with(&rig, ErrorScope::Harness, "initialize");
}

#[tokio::test(start_paused = true)]
async fn a_harness_that_never_acknowledges_unit_start_is_killed_after_the_grace_period() {
    let mut rig = Rig::builder().start();
    let mut peer = rig.peer().await;
    peer.initialize(peer_capabilities()).await;
    let (_id, _order) = peer.start_unacknowledged().await;
    assert_eq!(rig.finish().await, Failed);
    assert!(peer.was_killed());
    failed_with(&rig, ErrorScope::Harness, "unit/start");
}

#[tokio::test(start_paused = true)]
async fn no_provisioned_within_the_stall_window_fails_the_unit() {
    let mut rig = Rig::builder().start();
    let (mut peer, _order) = rig.started().await;
    // Logs are not `provisioned`, however many arrive.
    peer.event(UnitEvent::Log {
        stream: LogStream::System,
        line: "still cloning".into(),
    })
    .await;
    assert_eq!(rig.finish().await, Failed);
    failed_with(&rig, ErrorScope::Harness, "no provisioned within 600s");
    assert!(peer.was_killed());
}

// --- observations become triggers, and nothing else does ---

#[tokio::test(start_paused = true)]
async fn logs_findings_and_errors_are_forwarded_and_change_no_phase() {
    let mut rig = Rig::builder().start();
    let (mut peer, _order) = rig.building().await;
    peer.event(UnitEvent::Log {
        stream: LogStream::Agent,
        line: "thinking".into(),
    })
    .await;
    peer.event(UnitEvent::Finding {
        round: 1,
        severity: harness_protocol::Severity::Minor,
        title: "naming".into(),
        file: Some("src/cart.js".into()),
        resolved: false,
    })
    .await;
    peer.event(UnitEvent::Error {
        scope: WireScope::Harness,
        retryable: true,
        detail: "model endpoint slow".into(),
    })
    .await;
    rig.error_containing("model endpoint slow").await;
    assert_eq!(rig.phase(), Some(Building));
    let events = rig.recorder.events();
    assert!(events
        .iter()
        .any(|e| matches!(e, Event::Log { line, .. } if line == "thinking")));
    assert!(events
        .iter()
        .any(|e| matches!(e, Event::Finding { title, .. } if title == "naming")));
    failed_with(&rig, ErrorScope::Harness, "model endpoint slow");
}

#[tokio::test(start_paused = true)]
async fn each_build_check_and_review_is_announced_with_its_number() {
    let mut rig = Rig::builder().review_floor(2).start();
    let (mut peer, _order) = rig.building().await;
    peer.cycle(1, 0).await;
    rig.in_phase(Building).await;
    peer.cycle(2, 0).await;
    rig.in_phase(MergeCheck).await;
    let iterations: Vec<(String, u32)> = rig
        .recorder
        .events()
        .into_iter()
        .filter_map(|e| match e {
            Event::Iteration { kind, n } => Some((
                serde_json::to_value(kind)
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_string(),
                n,
            )),
            _ => None,
        })
        .collect();
    let named = |kind: &str, n: u32| (kind.to_string(), n);
    assert_eq!(
        iterations,
        [
            named("build", 1),
            named("check", 1),
            named("review", 1),
            named("build", 2),
            named("check", 2),
            named("review", 2)
        ]
    );
}

#[tokio::test(start_paused = true)]
async fn a_review_with_blockers_or_below_the_floor_returns_the_unit_to_building() {
    // Blockers: round 1 has one, round 2 has none.
    let mut rig = Rig::builder().start();
    let (mut peer, _order) = rig.building().await;
    peer.cycle(1, 1).await;
    rig.until("back in Building after a blocker", |r| {
        r.phases().ends_with(&[Reviewing, Building])
    })
    .await;
    peer.cycle(2, 0).await;
    rig.in_phase(MergeCheck).await;

    // The floor: no blockers in round 1, and the floor is 2.
    let mut rig = Rig::builder().review_floor(2).start();
    let (mut peer, _order) = rig.building().await;
    peer.cycle(1, 0).await;
    rig.until("back in Building below the floor", |r| {
        r.phases().ends_with(&[Reviewing, Building])
    })
    .await;
    assert!(!rig.recorder.phases().contains(&MergeCheck));
}

#[tokio::test(start_paused = true)]
async fn a_stage_note_is_forwarded_changes_no_phase_and_is_accepted_before_provisioned() {
    let mut rig = Rig::builder().start();
    let (mut peer, _order) = rig.started().await;
    peer.stage(Stage::Provision, StageStatus::Started).await;
    peer.provisioned_and_frozen().await;
    rig.in_phase(Building).await;
    peer.stage(Stage::Green, StageStatus::Started).await;
    rig.until("the second stage note", |r| r.stages().len() == 2)
        .await;
    assert_eq!(
        rig.recorder.stages(),
        [
            (Stage::Provision, StageStatus::Started),
            (Stage::Green, StageStatus::Started)
        ]
    );
    assert_eq!(rig.phase(), Some(Building));
}

#[tokio::test(start_paused = true)]
async fn the_freeze_and_the_result_are_handed_to_the_sink_to_be_kept() {
    let mut rig = Rig::builder().start();
    let (mut peer, order) = rig.building().await;
    peer.cycle(1, 0).await;
    peer.deliver(&order).await;
    peer.exit(0);
    assert_eq!(rig.finish().await, Done);
    let signals = rig.recorder.signals();
    let frozen: Vec<&Signal> = signals
        .iter()
        .filter(|s| matches!(s, Signal::Frozen(_)))
        .collect();
    assert_eq!(frozen, [&Signal::Frozen(default_freeze())]);
    let results = signals
        .iter()
        .filter(|s| matches!(s, Signal::Result(_)))
        .count();
    assert_eq!(results, 1);
    // The freeze reaches the verifier too, at T1 as at any tier.
    assert_eq!(rig.verifier.inputs()[0].freeze, Some(default_freeze()));
}

// --- violations fail the unit, with the offending line ---

#[tokio::test(start_paused = true)]
async fn an_observation_the_state_machine_rejects_is_a_protocol_violation() {
    // `checks_passed` while still building.
    let mut rig = Rig::builder().start();
    let (mut peer, _order) = rig.building().await;
    peer.observe(Observation::ChecksPassed).await;
    assert_eq!(rig.finish().await, Failed);
    failed_with(&rig, ErrorScope::Harness, "protocol violation");
    failed_with(&rig, ErrorScope::Harness, "checks_passed");
    assert!(peer.was_killed());
}

#[tokio::test(start_paused = true)]
async fn a_review_round_out_of_sequence_is_a_protocol_violation() {
    let mut rig = Rig::builder().start();
    let (mut peer, _order) = rig.building().await;
    peer.cycle(2, 0).await;
    assert_eq!(rig.finish().await, Failed);
    failed_with(&rig, ErrorScope::Harness, "protocol violation");
}

#[tokio::test(start_paused = true)]
async fn a_pr_open_result_before_merge_check_is_a_protocol_violation_and_is_not_verified() {
    let mut rig = Rig::builder().start();
    let (mut peer, order) = rig.building().await;
    peer.deliver(&order).await;
    assert_eq!(rig.finish().await, Failed);
    failed_with(&rig, ErrorScope::Harness, "protocol violation");
    assert!(rig.verifier.inputs().is_empty());
    assert!(rig.forge.opened().is_empty());
}

#[tokio::test(start_paused = true)]
async fn a_line_that_is_not_a_message_fails_the_unit_and_is_quoted_up_to_two_kilobytes() {
    let mut rig = Rig::builder().start();
    let (mut peer, _order) = rig.building().await;
    let garbage = format!("not json {}", "x".repeat(10_000));
    peer.send_raw(&garbage).await;
    assert_eq!(rig.finish().await, Failed);
    let (_, detail) = rig
        .recorder
        .errors()
        .into_iter()
        .find(|(_, d)| d.contains("protocol violation"))
        .expect("a protocol violation error");
    assert!(detail.contains("not json xxxx"));
    assert!(
        detail.len() < 2_300,
        "the quoted line was not truncated: {} bytes",
        detail.len()
    );
}

#[tokio::test(start_paused = true)]
async fn a_line_longer_than_the_protocol_allows_fails_the_unit_without_being_buffered_whole() {
    let mut rig = Rig::builder().start();
    let (mut peer, _order) = rig.building().await;
    // One line of 5 MiB, sent in pieces with no newline until the end.
    let chunk = "y".repeat(64 * 1024);
    let sending = tokio::spawn(async move {
        for _ in 0..80 {
            peer.send_raw_partial(&chunk).await;
        }
        peer
    });
    assert_eq!(rig.finish().await, Failed);
    failed_with(&rig, ErrorScope::Harness, "protocol violation");
    sending.abort();
}

// --- exits and failures ---

#[tokio::test(start_paused = true)]
async fn a_harness_that_exits_without_a_result_fails_the_unit_with_its_exit_status() {
    let mut rig = Rig::builder().start();
    let (mut peer, _order) = rig.building().await;
    peer.exit(3);
    assert_eq!(rig.finish().await, Failed);
    failed_with(
        &rig,
        ErrorScope::Harness,
        "exited without result (exit code: 3)",
    );
}

#[tokio::test(start_paused = true)]
async fn a_failed_result_fails_the_unit_with_the_failures_own_scope() {
    let mut rig = Rig::builder().start();
    let (mut peer, _order) = rig.building().await;
    peer.result(UnitResult {
        outcome: Outcome::Failed,
        evidence: None,
        failure: Some(Failure {
            scope: WireScope::Docker,
            detail: "image pull failed".into(),
        }),
        stop: None,
    })
    .await;
    peer.exit(1);
    assert_eq!(rig.finish().await, Failed);
    failed_with(&rig, ErrorScope::Docker, "image pull failed");
    assert!(rig.verifier.inputs().is_empty());
}

#[tokio::test(start_paused = true)]
async fn a_harness_that_lingers_after_its_result_is_killed_and_the_result_still_stands() {
    let mut rig = Rig::builder().start();
    let (mut peer, order) = rig.merge_check().await;
    peer.deliver(&order).await;
    // No exit. The supervisor closes the input, waits out the grace period, and kills.
    peer.input_closed().await;
    assert_eq!(rig.finish().await, Done);
    assert!(peer.was_killed());
    failed_with(&rig, ErrorScope::Harness, "did not exit");
    assert_eq!(rig.verifier.inputs().len(), 1);
}

// --- the needs_human result ---

#[tokio::test(start_paused = true)]
async fn a_needs_human_result_pauses_the_unit_with_its_reason_and_is_never_verified() {
    let mut rig = Rig::builder().start();
    let (mut peer, _order) = rig.building().await;
    peer.result(stopped(StopReason::ScopeRequest, "needs src/tax.js"))
        .await;
    peer.exit(0);
    rig.in_phase(NeedsHuman).await;
    assert_eq!(rig.recorder.last_reason().as_deref(), Some("scope_request"));
    let blocked = rig.recorder.blocked();
    assert_eq!(blocked.last().unwrap().0, "scope_request");
    assert!(blocked.last().unwrap().2.contains("needs src/tax.js"));
    assert!(
        rig.verifier.inputs().is_empty(),
        "a needs_human result was verified"
    );
    assert_eq!(
        rig.slots.available_permits(),
        3,
        "a stopped unit holds no slot"
    );
    assert!(rig
        .recorder
        .signals()
        .iter()
        .any(|s| matches!(s, Signal::Result(_))));
}

#[tokio::test(start_paused = true)]
async fn a_needs_human_result_is_legal_in_every_agent_active_phase_and_in_no_other() {
    // Spec (before the freeze), Checking and Reviewing: each pauses.
    for stop_in in [Spec, Checking, Reviewing] {
        let mut rig = Rig::builder().start();
        let (mut peer, _order) = rig.started().await;
        peer.observe(Observation::Provisioned).await;
        if stop_in != Spec {
            peer.observe(Observation::OracleFrozen {
                freeze: Some(default_freeze()),
            })
            .await;
            peer.observe(Observation::BuildFinished).await;
        }
        if stop_in == Reviewing {
            peer.observe(Observation::ChecksPassed).await;
        }
        rig.in_phase(stop_in).await;
        peer.result(stopped(StopReason::BaselineRed, "the base is red"))
            .await;
        peer.exit(0);
        rig.in_phase(NeedsHuman).await;
        assert_eq!(
            rig.recorder.last_reason().as_deref(),
            Some("baseline_red"),
            "{stop_in:?}"
        );
    }
    // MergeCheck is not agent-active: there the same result is a violation.
    let mut rig = Rig::builder().start();
    let (mut peer, _order) = rig.merge_check().await;
    peer.result(stopped(StopReason::BudgetExhausted, "late"))
        .await;
    assert_eq!(rig.finish().await, Failed);
    failed_with(&rig, ErrorScope::Harness, "protocol violation");
}

#[tokio::test(start_paused = true)]
async fn a_metering_harness_may_stop_for_a_human_before_it_has_metered_anything() {
    let mut rig = Rig::builder().start();
    let (mut peer, _order) = rig.started().await;
    peer.observe(Observation::Provisioned).await;
    rig.in_phase(Spec).await;
    peer.result(stopped(StopReason::MapUntrusted, "drift 0.4"))
        .await;
    peer.exit(0);
    rig.in_phase(NeedsHuman).await;
    assert!(
        rig.recorder.errors().is_empty(),
        "{:?}",
        rig.recorder.errors()
    );
}

#[tokio::test(start_paused = true)]
async fn the_gate_hash_constant_is_what_a_scripted_gate_proposes() {
    let mut rig = Rig::builder().tier(Tier::T2).start();
    let (_peer, _order, _gate) = rig.at_gate().await;
    assert!(rig.recorder.events().iter().any(|e| matches!(
        e,
        Event::OracleProposed { hash, .. } if hash == GATE_HASH
    )));
}
````

`crates/fleet-supervisor/tests/gate_and_caps.rs`:

````rust
//! Lane CC-SUPERVISOR: the oracle gate, the caps, the stall window and the clocks.
//! Supervisor spec sections 4.4 and 4.5, and section 6 item 4.

use fleet_core::{Command, ErrorScope, Event, Phase, Tier};
use fleet_supervisor::testkit::{approve, peer_capabilities, reject, Rig, GATE_HASH};
use fleet_supervisor::REASON_AWAITING_SLOT;
use fleet_verify::testkit::FROZEN_TEST_PATH;
use harness_protocol::{
    method, ErrorScope as WireScope, Failure, LogStream, Metering, Observation, Outcome, Stage,
    StageStatus, UnitEvent, UnitResult,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;

use Phase::*;

fn secs(n: u64) -> Duration {
    Duration::from_secs(n)
}

fn position(rig: &Rig, found: impl Fn(&Event) -> bool) -> usize {
    rig.recorder
        .events()
        .iter()
        .position(found)
        .expect("the event was emitted")
}

fn has_error(rig: &Rig, scope: ErrorScope, needle: &str) -> bool {
    rig.recorder
        .errors()
        .iter()
        .any(|(s, detail)| *s == scope && detail.contains(needle))
}

// --- the oracle gate ---

#[tokio::test(start_paused = true)]
async fn at_t2_oracle_frozen_alone_leaves_the_unit_in_spec() {
    let mut rig = Rig::builder().tier(Tier::T2).start();
    let (mut peer, _order) = rig.started().await;
    peer.provisioned_and_frozen().await;
    peer.event(UnitEvent::Log {
        stream: LogStream::System,
        line: "freezing done".into(),
    })
    .await;
    rig.until("the log line", |r| {
        r.events()
            .iter()
            .any(|e| matches!(e, Event::Log { line, .. } if line == "freezing done"))
    })
    .await;
    assert_eq!(
        rig.phase(),
        Some(Spec),
        "the phase moved before the gate existed"
    );
}

#[tokio::test(start_paused = true)]
async fn the_gate_request_announces_the_proposal_before_the_phase_changes() {
    let mut rig = Rig::builder().tier(Tier::T2).start();
    let (_peer, _order, _gate) = rig.at_gate().await;
    let proposed = position(&rig, |e| {
        matches!(e, Event::OracleProposed { test_files, hash, .. }
            if test_files == &[FROZEN_TEST_PATH.to_string()] && hash == GATE_HASH)
    });
    let changed = position(&rig, |e| {
        matches!(
            e,
            Event::PhaseChanged {
                to: AwaitingOracleApproval,
                ..
            }
        )
    });
    assert!(proposed < changed);
}

#[tokio::test(start_paused = true)]
async fn approving_before_the_gate_exists_is_an_error_and_changes_nothing() {
    let mut rig = Rig::builder().tier(Tier::T2).start();
    let (mut peer, _order) = rig.started().await;
    peer.provisioned_and_frozen().await;
    rig.in_phase(Spec).await;
    rig.command(approve("early"));
    rig.error_containing("early").await;
    assert_eq!(rig.phase(), Some(Spec));
    assert!(has_error(&rig, ErrorScope::System, "early"));
}

#[tokio::test(start_paused = true)]
async fn approving_with_edited_files_is_refused_and_the_gate_stays_pending() {
    let mut rig = Rig::builder().tier(Tier::T2).start();
    let (mut peer, _order, gate) = rig.at_gate().await;
    rig.command(Command::ApproveOracle {
        cmd_id: "edited".into(),
        edited_test_files: Some(vec!["test/other.test.js".into()]),
    });
    rig.error_containing("edited oracle").await;
    assert_eq!(rig.phase(), Some(AwaitingOracleApproval));
    // The gate can still be approved as proposed.
    rig.command(approve("plain"));
    assert!(peer.gate_reply(gate).await.approved);
    rig.in_phase(Building).await;
}

#[tokio::test(start_paused = true)]
async fn a_gate_request_with_no_oracle_frozen_before_it_is_a_violation() {
    let mut rig = Rig::builder().tier(Tier::T2).start();
    let (mut peer, _order) = rig.started().await;
    peer.observe(Observation::Provisioned).await;
    rig.in_phase(Spec).await;
    peer.gate(&[FROZEN_TEST_PATH], GATE_HASH).await;
    assert_eq!(rig.finish().await, Failed);
    assert!(has_error(&rig, ErrorScope::Harness, "protocol violation"));
}

#[tokio::test(start_paused = true)]
async fn a_gate_request_at_t1_is_a_violation() {
    let mut rig = Rig::builder().start();
    let (mut peer, _order) = rig.building().await;
    peer.gate(&[FROZEN_TEST_PATH], GATE_HASH).await;
    assert_eq!(rig.finish().await, Failed);
    assert!(has_error(&rig, ErrorScope::Harness, "protocol violation"));
}

#[tokio::test(start_paused = true)]
async fn a_rejected_oracle_is_answered_false_and_the_harness_then_ends_the_unit() {
    let mut rig = Rig::builder().tier(Tier::T2).start();
    let (mut peer, _order, gate) = rig.at_gate().await;
    rig.command(reject("no"));
    assert!(!peer.gate_reply(gate).await.approved);
    rig.until("back in Spec", |r| {
        r.phases().ends_with(&[AwaitingOracleApproval, Spec])
    })
    .await;
    peer.result(UnitResult {
        outcome: Outcome::Failed,
        evidence: None,
        failure: Some(Failure {
            scope: WireScope::System,
            detail: "oracle rejected".into(),
        }),
        stop: None,
    })
    .await;
    peer.exit(1);
    assert_eq!(rig.finish().await, Failed);
    assert!(has_error(&rig, ErrorScope::System, "oracle rejected"));
}

#[tokio::test(start_paused = true)]
async fn an_unanswered_gate_waits_without_limit_and_no_clock_runs_while_it_does() {
    let mut rig = Rig::builder().tier(Tier::T2).caps(5.0, 100).start();
    let (mut peer, _order, gate) = rig.at_gate().await;
    tokio::time::sleep(secs(48 * 3600)).await;
    assert_eq!(rig.phase(), Some(AwaitingOracleApproval));
    assert!(
        rig.recorder.blocked().is_empty(),
        "{:?}",
        rig.recorder.blocked()
    );
    assert!(!peer.was_killed());
    // And the clocks start again from where they were, not from two days later.
    rig.command(approve("late"));
    peer.gate_reply(gate).await;
    rig.in_phase(Building).await;
    tokio::time::sleep(secs(50)).await;
    assert_eq!(
        rig.phase(),
        Some(Building),
        "the wall clock counted the wait at the gate"
    );
}

// --- the concurrency slot around the gate (released while pending, taken back before the reply) ---

#[tokio::test(start_paused = true)]
async fn the_slot_is_given_back_while_the_gate_is_pending() {
    let slots = Arc::new(Semaphore::new(1));
    let mut rig = Rig::builder().tier(Tier::T2).slots(slots.clone()).start();
    let (mut peer, _order) = rig.started().await;
    assert_eq!(
        slots.available_permits(),
        0,
        "a running unit holds its slot"
    );
    peer.provisioned_and_frozen().await;
    peer.gate(&[FROZEN_TEST_PATH], GATE_HASH).await;
    rig.in_phase(AwaitingOracleApproval).await;
    rig.settle_tasks().await;
    assert_eq!(
        slots.available_permits(),
        1,
        "a unit waiting for a person holds no slot"
    );
}

#[tokio::test(start_paused = true)]
async fn an_approved_gate_is_not_answered_until_the_unit_has_its_slot_again() {
    let slots = Arc::new(Semaphore::new(1));
    let mut rig = Rig::builder().tier(Tier::T2).slots(slots.clone()).start();
    let (mut peer, _order, gate) = rig.at_gate().await;
    // Another unit takes the only slot while this one waits for its approval.
    let other = slots.clone().acquire_owned().await.unwrap();

    rig.command(approve("c1"));
    rig.until("the wait for a slot is announced", |r| {
        r.blocked()
            .iter()
            .any(|(reason, _, _)| reason == REASON_AWAITING_SLOT)
    })
    .await;
    assert_eq!(
        rig.phase(),
        Some(AwaitingOracleApproval),
        "the phase holds until the reply"
    );
    let early = tokio::time::timeout(secs(3600), peer.gate_reply(gate)).await;
    assert!(
        early.is_err(),
        "the harness was told to go on without a slot"
    );

    drop(other);
    assert!(peer.gate_reply(gate).await.approved);
    rig.in_phase(Building).await;
    assert_eq!(slots.available_permits(), 0);
}

#[tokio::test(start_paused = true)]
async fn a_unit_halted_while_it_waits_for_its_slot_back_never_answers_the_gate() {
    let slots = Arc::new(Semaphore::new(1));
    let mut rig = Rig::builder().tier(Tier::T2).slots(slots.clone()).start();
    let (mut peer, _order, _gate) = rig.at_gate().await;
    let _other = slots.clone().acquire_owned().await.unwrap();
    rig.command(approve("c1"));
    rig.until("waiting for a slot", |r| {
        r.blocked()
            .iter()
            .any(|(reason, _, _)| reason == REASON_AWAITING_SLOT)
    })
    .await;
    rig.command(Command::Halt {
        cmd_id: "h1".into(),
    });
    rig.in_phase(Halted).await;
    // The harness saw a halt and its input closing, and no gate reply.
    while let Some(message) = peer.recv().await {
        let approved = message.result.as_ref().and_then(|r| r.get("approved"));
        assert!(approved.is_none(), "the gate was answered: {message:?}");
    }
}

// --- the USD cap ---

#[tokio::test(start_paused = true)]
async fn metrics_are_summed_on_what_earlier_spawns_spent_and_a_breach_pauses_the_unit() {
    let mut rig = Rig::builder().caps(5.0, 1800).spent(4.0, 0).start();
    let (mut peer, order) = rig.building().await;
    assert_eq!(order.caps.usd, 1.0, "the harness is told what is left");
    peer.metric(0.5).await;
    peer.metric(0.5).await;
    rig.until("the second metric", |r| {
        r.last_metric().map(|m| m.0) == Some(5.0)
    })
    .await;
    assert_eq!(
        rig.phase(),
        Some(Building),
        "spending exactly the cap is not a breach"
    );

    peer.metric(0.25).await;
    rig.in_phase(NeedsHuman).await;
    assert_eq!(
        rig.recorder.blocked(),
        [(
            "usd cap".to_string(),
            Some("usd".to_string()),
            "spent $5.25 of $5.00".to_string()
        )]
    );
    assert_eq!(rig.recorder.last_reason().as_deref(), Some("usd cap"));
    // The stop sequence: the harness declared `halt`, so it is asked to, then its input closes.
    let (id, _) = peer.request(method::UNIT_HALT).await;
    peer.reply(id).await;
    peer.exit(0);
    assert_eq!(rig.slots.available_permits(), 3);
}

#[tokio::test(start_paused = true)]
async fn each_metric_is_re_emitted_as_the_units_running_total() {
    let mut rig = Rig::builder().spent(1.0, 30_000).start();
    let (mut peer, _order) = rig.building().await;
    peer.metric(0.25).await;
    peer.metric(0.25).await;
    rig.until("two metrics", |r| {
        r.events()
            .iter()
            .filter(|e| matches!(e, Event::Metric { .. }))
            .count()
            >= 2
    })
    .await;
    let totals: Vec<f64> = rig
        .recorder
        .events()
        .into_iter()
        .filter_map(|e| match e {
            Event::Metric { cost_usd, .. } => Some(cost_usd),
            _ => None,
        })
        .collect();
    assert_eq!(totals, [1.25, 1.5]);
}

#[tokio::test(start_paused = true)]
async fn a_breach_pauses_the_unit_in_every_phase_a_harness_can_be_alive_in() {
    // Provisioning: a metric before `provisioned`.
    let mut rig = Rig::builder().caps(1.0, 1800).start();
    let (mut peer, _order) = rig.started().await;
    peer.metric(1.5).await;
    rig.in_phase(NeedsHuman).await;
    assert!(rig.recorder.phases().ends_with(&[Provisioning, NeedsHuman]));

    // Spec at T2: a metric between `oracle_frozen` and the gate request.
    let mut rig = Rig::builder().tier(Tier::T2).caps(1.0, 1800).start();
    let (mut peer, _order) = rig.started().await;
    peer.provisioned_and_frozen().await;
    peer.metric(1.5).await;
    rig.in_phase(NeedsHuman).await;
    assert!(rig.recorder.phases().ends_with(&[Spec, NeedsHuman]));

    // AwaitingOracleApproval: a metric while the gate is pending.
    let mut rig = Rig::builder().tier(Tier::T2).caps(1.0, 1800).start();
    let (mut peer, _order, _gate) = rig.at_gate().await;
    peer.metric(1.5).await;
    rig.in_phase(NeedsHuman).await;
    assert!(rig
        .recorder
        .phases()
        .ends_with(&[AwaitingOracleApproval, NeedsHuman]));

    // MergeCheck: the review gate is met and the harness is still bundling.
    let mut rig = Rig::builder().caps(1.0, 1800).start();
    let (mut peer, _order) = rig.merge_check().await;
    peer.metric(1.5).await;
    rig.in_phase(NeedsHuman).await;
    assert!(rig.recorder.phases().ends_with(&[MergeCheck, NeedsHuman]));
}

#[tokio::test(start_paused = true)]
async fn metrics_that_arrive_while_a_breach_is_being_handled_are_counted_and_raise_no_second_breach(
) {
    let mut rig = Rig::builder().caps(1.0, 1800).start();
    let (mut peer, _order) = rig.building().await;
    peer.metric(1.5).await;
    peer.metric(2.0).await;
    peer.metric(3.0).await;
    rig.in_phase(NeedsHuman).await;
    peer.exit(0);
    rig.until("the last metric", |r| {
        r.last_metric().map(|m| m.0) == Some(6.5)
    })
    .await;
    assert_eq!(rig.recorder.blocked().len(), 1);
    assert_eq!(
        rig.recorder
            .phases()
            .iter()
            .filter(|p| **p == NeedsHuman)
            .count(),
        1
    );
}

// --- the wall clock and the stall window ---

#[tokio::test(start_paused = true)]
async fn agent_time_past_the_wall_clock_cap_pauses_the_unit() {
    let mut rig = Rig::builder().caps(5.0, 100).spent(0.0, 40_000).start();
    let (mut peer, order) = rig.building().await;
    assert_eq!(
        order.caps.wall_clock_secs, 60,
        "the harness is told what is left"
    );
    // Keep the unit from stalling while the clock runs out.
    for _ in 0..7 {
        tokio::time::sleep(secs(10)).await;
        peer.stage(Stage::Green, StageStatus::Started).await;
    }
    rig.in_phase(NeedsHuman).await;
    let blocked = rig.recorder.blocked();
    assert_eq!(blocked.len(), 1);
    assert_eq!(blocked[0].0, "wall-clock cap");
    assert_eq!(blocked[0].1.as_deref(), Some("wall_clock"));
    assert!(
        blocked[0].2.starts_with("ran 10") && blocked[0].2.ends_with("s of 100s"),
        "{}",
        blocked[0].2
    );
}

#[tokio::test(start_paused = true)]
async fn a_wall_clock_cap_of_zero_disables_the_wall_clock() {
    let mut rig = Rig::builder()
        .caps(5.0, 0)
        .windows(secs(30), secs(1_000_000))
        .start();
    let (_peer, order) = rig.building().await;
    assert_eq!(order.caps.wall_clock_secs, 0);
    tokio::time::sleep(secs(500_000)).await;
    assert_eq!(rig.phase(), Some(Building));
}

#[tokio::test(start_paused = true)]
async fn a_harness_that_sends_no_event_for_the_stall_window_is_stopped_and_the_unit_pauses() {
    let mut rig = Rig::builder().caps(5.0, 0).start();
    let (peer, _order) = rig.building().await;
    let started = tokio::time::Instant::now();
    rig.in_phase(NeedsHuman).await;
    assert!(started.elapsed() >= secs(600) && started.elapsed() < secs(700));
    assert_eq!(
        rig.recorder.blocked(),
        [("stalled".to_string(), None, "no event for 600s".to_string())]
    );
    assert!(
        peer.was_killed(),
        "a harness that ignores the stop is killed after the grace period"
    );
}

#[tokio::test(start_paused = true)]
async fn a_stage_note_counts_as_activity_and_a_line_on_standard_error_does_not() {
    let mut rig = Rig::builder().caps(5.0, 0).start();
    let (mut peer, _order) = rig.building().await;
    // Stage notes every 400 seconds: never 600 seconds of silence.
    for _ in 0..4 {
        tokio::time::sleep(secs(400)).await;
        peer.stage(Stage::Green, StageStatus::Started).await;
    }
    assert_eq!(rig.phase(), Some(Building));
    // Standard error every 400 seconds: forwarded as system log lines, and not activity.
    for _ in 0..2 {
        tokio::time::sleep(secs(400)).await;
        peer.diagnostic("still here");
    }
    rig.in_phase(NeedsHuman).await;
    assert_eq!(rig.recorder.blocked()[0].0, "stalled");
    assert!(rig.recorder.events().iter().any(|e| matches!(
        e,
        Event::Log { stream: fleet_core::LogStream::System, line } if line == "still here"
    )));
}

#[tokio::test(start_paused = true)]
async fn no_clock_runs_while_a_unit_waits_for_a_concurrency_slot() {
    let slots = Arc::new(Semaphore::new(1));
    let held = slots.clone().acquire_owned().await.unwrap();
    let mut rig = Rig::builder().caps(5.0, 100).slots(slots.clone()).start();
    rig.until("the wait for a slot is announced", |r| {
        r.blocked() == [(REASON_AWAITING_SLOT.to_string(), None, String::new())]
    })
    .await;
    tokio::time::sleep(secs(10_000)).await;
    assert_eq!(rig.phase(), Some(Provisioning));
    assert_eq!(
        rig.recorder.blocked().len(),
        1,
        "a waiting unit stalled or breached"
    );
    drop(held);
    let (_peer, order) = rig.building().await;
    assert_eq!(
        order.caps.wall_clock_secs, 100,
        "the wait cost no agent time"
    );
}

#[tokio::test(start_paused = true)]
async fn the_clocks_stop_at_the_result_so_a_slow_verification_cannot_stall_the_unit() {
    let mut rig = Rig::builder().caps(5.0, 100).start();
    rig.verifier.delay(secs(7200));
    let (mut peer, order) = rig.merge_check().await;
    peer.deliver(&order).await;
    peer.exit(0);
    assert_eq!(rig.finish().await, Done);
    assert!(
        rig.recorder.blocked().is_empty(),
        "{:?}",
        rig.recorder.blocked()
    );
}

#[tokio::test(start_paused = true)]
async fn a_harness_that_never_meters_still_has_its_agent_time_recorded_when_the_spawn_ends() {
    let mut rig = Rig::builder().opt_in().start();
    let mut peer = rig.peer().await;
    let mut unmetered = peer_capabilities();
    unmetered.metering = Metering::None;
    let order = peer.handshake(unmetered).await;
    peer.provisioned_and_frozen().await;
    tokio::time::sleep(secs(5)).await;
    peer.observe(Observation::BuildFinished).await;
    peer.observe(Observation::ChecksPassed).await;
    peer.observe(Observation::ReviewFinished {
        round: 1,
        unresolved_blockers: 0,
        checks_green: true,
    })
    .await;
    peer.deliver(&order).await;
    peer.exit(0);
    assert_eq!(rig.finish().await, Done);
    let (cost, elapsed_ms) = rig.recorder.last_metric().expect("a spawn-end metric");
    assert_eq!(cost, 0.0);
    assert!(elapsed_ms >= 5_000, "{elapsed_ms}");
}
````

`crates/fleet-supervisor/tests/commands.rs`:

````rust
//! Lane CC-SUPERVISOR: stopping, draining, resuming, and every row of the command matrix.
//! Supervisor spec sections 4.3 and 4.6, and section 6 item 4.

use fleet_core::{ErrorScope, Phase, Tier};
use fleet_supervisor::testkit::{
    abandon, approve, halt, peer_capabilities, reject, resume, ship, Rig, GATE_HASH,
};
use fleet_supervisor::{REASON_AWAITING_SHIP, REASON_AWAITING_SLOT, REASON_ORACLE_TAMPERING};
use fleet_verify::testkit::{evidence, pr_open};
use fleet_verify::Verdict;
use harness_protocol::{method, Observation, Resume};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;

use Phase::*;

fn secs(n: u64) -> Duration {
    Duration::from_secs(n)
}

fn system_error(rig: &Rig, needle: &str) -> bool {
    rig.recorder
        .errors()
        .iter()
        .any(|(scope, detail)| *scope == ErrorScope::System && detail.contains(needle))
}

// --- halt: the stop sequence and draining ---

#[tokio::test(start_paused = true)]
async fn halt_asks_the_harness_to_halt_closes_its_input_and_the_unit_is_halted_when_it_exits() {
    let mut rig = Rig::builder().start();
    let (mut peer, _order) = rig.building().await;
    rig.command(halt("h1"));
    let (id, _) = peer.request(method::UNIT_HALT).await;
    peer.reply(id).await;
    assert!(
        peer.recv().await.is_none(),
        "the harness's input was not closed"
    );
    peer.exit(0);
    rig.in_phase(Halted).await;
    assert!(!peer.was_killed());
    assert_eq!(rig.slots.available_permits(), 3);
    assert!(rig.recorder.events().iter().any(|e| matches!(
        e,
        fleet_core::Event::PhaseChanged { to: Halted, cmd_id: Some(id), .. } if id == "h1"
    )));
}

#[tokio::test(start_paused = true)]
async fn what_a_harness_sends_after_it_was_told_to_halt_is_discarded_except_its_spend() {
    let mut rig = Rig::builder().start();
    let (mut peer, order) = rig.building().await;
    peer.metric(0.5).await;
    rig.command(halt("h1"));
    peer.request(method::UNIT_HALT).await;
    // In-flight work keeps arriving: observations, a gate request, more spend, even a result.
    peer.observe(Observation::BuildFinished).await;
    peer.observe(Observation::ChecksPassed).await;
    let gate = peer.gate(&["test/x.test.js"], GATE_HASH).await;
    peer.metric(0.25).await;
    peer.deliver(&order).await;
    peer.exit(0);
    rig.in_phase(Halted).await;

    assert_eq!(
        rig.recorder.phases(),
        [Provisioning, Spec, Building, Halted]
    );
    assert_eq!(
        rig.recorder.last_metric().map(|m| m.0),
        Some(0.75),
        "the spend is real"
    );
    assert!(
        rig.verifier.inputs().is_empty(),
        "a result sent after the halt was verified"
    );
    assert!(
        rig.recorder.errors().is_empty(),
        "{:?}",
        rig.recorder.errors()
    );
    // The gate request was never answered.
    while let Some(message) = peer.recv().await {
        assert_ne!(
            message.id,
            Some(gate),
            "a gate request was answered while draining"
        );
    }
}

#[tokio::test(start_paused = true)]
async fn a_harness_that_refuses_to_halt_is_killed_after_the_grace_period_and_the_unit_is_halted() {
    let mut rig = Rig::builder().start();
    let (mut peer, _order) = rig.building().await;
    rig.command(halt("h1"));
    let (id, _) = peer.request(method::UNIT_HALT).await;
    peer.refuse(id).await;
    let asked = tokio::time::Instant::now();
    rig.in_phase(Halted).await;
    assert!(peer.was_killed());
    assert!(asked.elapsed() >= secs(30));
}

#[tokio::test(start_paused = true)]
async fn a_harness_that_did_not_declare_halt_is_not_asked_and_just_has_its_input_closed() {
    let mut rig = Rig::builder().start();
    let mut peer = rig.peer().await;
    let mut no_halt = peer_capabilities();
    no_halt.halt = false;
    peer.handshake(no_halt).await;
    peer.provisioned_and_frozen().await;
    rig.in_phase(Building).await;
    rig.command(halt("h1"));
    assert!(
        peer.recv().await.is_none(),
        "unit/halt was sent to a harness that cannot halt"
    );
    peer.exit(0);
    rig.in_phase(Halted).await;
}

#[tokio::test(start_paused = true)]
async fn abandon_always_sends_unit_abandon_and_the_unit_fails() {
    let mut rig = Rig::builder().start();
    let mut peer = rig.peer().await;
    let mut no_halt = peer_capabilities();
    no_halt.halt = false;
    peer.handshake(no_halt).await;
    peer.provisioned_and_frozen().await;
    rig.in_phase(Building).await;
    rig.command(abandon("a1"));
    let (id, _) = peer.request(method::UNIT_ABANDON).await;
    peer.reply(id).await;
    peer.exit(0);
    assert_eq!(rig.finish().await, Failed);
    assert_eq!(rig.recorder.last_reason().as_deref(), Some("abandoned"));
}

#[tokio::test(start_paused = true)]
async fn abandon_during_a_halt_in_progress_ends_the_unit_failed_not_halted() {
    let mut rig = Rig::builder().start();
    let (mut peer, _order) = rig.building().await;
    rig.command(halt("h1"));
    peer.request(method::UNIT_HALT).await;
    rig.command(abandon("a1"));
    peer.exit(0);
    assert_eq!(rig.finish().await, Failed);
    assert!(!rig.recorder.phases().contains(&Halted));
    assert_eq!(rig.recorder.last_reason().as_deref(), Some("abandoned"));
}

#[tokio::test(start_paused = true)]
async fn a_second_halt_while_one_is_in_progress_is_refused() {
    let mut rig = Rig::builder().start();
    let (mut peer, _order) = rig.building().await;
    rig.command(halt("h1"));
    peer.request(method::UNIT_HALT).await;
    rig.command(halt("h2"));
    rig.error_containing("halt not available now; abandon instead")
        .await;
    peer.exit(0);
    rig.in_phase(Halted).await;
}

// --- a unit that has no harness yet ---

#[tokio::test(start_paused = true)]
async fn a_unit_waiting_for_a_slot_can_be_halted_and_then_resumed() {
    let slots = Arc::new(Semaphore::new(1));
    let held = slots.clone().acquire_owned().await.unwrap();
    let mut rig = Rig::builder().slots(slots.clone()).start();
    rig.until("waiting for a slot", |r| {
        r.blocked()
            .iter()
            .any(|(reason, _, _)| reason == REASON_AWAITING_SLOT)
    })
    .await;
    rig.command(halt("h1"));
    rig.in_phase(Halted).await;
    assert_eq!(
        rig.connector.connects(),
        0,
        "no harness was started for a unit with no slot"
    );
    drop(held);
    assert_eq!(
        slots.available_permits(),
        1,
        "a halted unit is not in the queue for a slot"
    );

    rig.command(resume("r1"));
    let (_peer, order) = rig.started().await;
    assert_eq!(
        order.resume,
        Some(Resume {
            oracle_frozen: false
        })
    );
}

#[tokio::test(start_paused = true)]
async fn a_unit_waiting_for_a_slot_can_be_abandoned() {
    let slots = Arc::new(Semaphore::new(1));
    let _held = slots.clone().acquire_owned().await.unwrap();
    let mut rig = Rig::builder().slots(slots).start();
    rig.until("waiting for a slot", |r| !r.blocked().is_empty())
        .await;
    rig.command(abandon("a1"));
    assert_eq!(rig.finish().await, Failed);
    assert_eq!(rig.connector.connects(), 0);
}

// --- resume ---

#[tokio::test(start_paused = true)]
async fn a_t1_unit_halted_while_building_resumes_without_freezing_again_and_rounds_restart_at_one()
{
    let mut rig = Rig::builder().start();
    let (mut first, _order) = rig.building().await;
    first.cycle(1, 1).await;
    rig.until("back in Building", |r| {
        r.phases().ends_with(&[Reviewing, Building])
    })
    .await;
    rig.command(halt("h1"));
    first.request(method::UNIT_HALT).await;
    first.exit(0);
    rig.in_phase(Halted).await;

    rig.command(resume("r1"));
    let (mut second, order) = rig.started().await;
    assert_eq!(
        order.resume,
        Some(Resume {
            oracle_frozen: true
        })
    );
    // The resumed harness sends `provisioned` and goes straight on; the freeze is synthesised.
    second.observe(Observation::Provisioned).await;
    rig.until("Building again", |r| {
        r.phases()
            .ends_with(&[Halted, Provisioning, Spec, Building])
    })
    .await;
    // Rounds count from 1 in each process.
    second.cycle(1, 0).await;
    rig.in_phase(MergeCheck).await;
    assert_eq!(rig.connector.connects(), 2);
}

#[tokio::test(start_paused = true)]
async fn a_t2_unit_halted_after_approval_resumes_with_no_new_gate() {
    let mut rig = Rig::builder().tier(Tier::T2).start();
    let (mut first, _order, gate) = rig.at_gate().await;
    rig.command(approve("c1"));
    first.gate_reply(gate).await;
    rig.in_phase(Building).await;
    rig.command(halt("h1"));
    first.request(method::UNIT_HALT).await;
    first.exit(0);
    rig.in_phase(Halted).await;

    rig.command(resume("r1"));
    let (mut second, order) = rig.started().await;
    assert_eq!(
        order.resume,
        Some(Resume {
            oracle_frozen: true
        })
    );
    second.observe(Observation::Provisioned).await;
    rig.until("both gate transitions synthesised", |r| {
        r.phases()
            .ends_with(&[Provisioning, Spec, AwaitingOracleApproval, Building])
    })
    .await;
    let reasons: Vec<Option<String>> = rig
        .recorder
        .events()
        .into_iter()
        .rev()
        .filter_map(|e| match e {
            fleet_core::Event::PhaseChanged { reason, .. } => Some(reason),
            _ => None,
        })
        .take(2)
        .collect();
    assert_eq!(
        reasons,
        [
            Some("approved in a prior run".to_string()),
            Some("oracle already frozen".to_string())
        ]
    );
}

#[tokio::test(start_paused = true)]
async fn a_t2_unit_halted_while_its_gate_was_pending_is_given_a_fresh_gate_on_resume() {
    let mut rig = Rig::builder().tier(Tier::T2).start();
    let (mut first, _order, gate) = rig.at_gate().await;
    rig.command(halt("h1"));
    first.request(method::UNIT_HALT).await;
    first.exit(0);
    rig.in_phase(Halted).await;
    while let Some(message) = first.recv().await {
        assert_ne!(message.id, Some(gate), "a gate was answered by a halt");
    }

    rig.command(resume("r1"));
    let (mut second, order) = rig.started().await;
    assert_eq!(
        order.resume,
        Some(Resume {
            oracle_frozen: false
        })
    );
    // The harness sends the same freeze and the same gate again; this time it is approved.
    second.provisioned_and_frozen().await;
    let again = second.gate(&["test/codes.test.js"], GATE_HASH).await;
    rig.in_phase(AwaitingOracleApproval).await;
    rig.command(approve("c2"));
    assert!(second.gate_reply(again).await.approved);
    rig.in_phase(Building).await;
}

#[tokio::test(start_paused = true)]
async fn a_t2_unit_halted_after_a_rejection_is_given_a_fresh_gate_on_resume() {
    let mut rig = Rig::builder().tier(Tier::T2).start();
    let (mut first, _order, gate) = rig.at_gate().await;
    rig.command(reject("no"));
    first.gate_reply(gate).await;
    rig.until("back in Spec", |r| {
        r.phases().ends_with(&[AwaitingOracleApproval, Spec])
    })
    .await;
    rig.command(halt("h1"));
    first.request(method::UNIT_HALT).await;
    first.exit(0);
    rig.in_phase(Halted).await;
    rig.command(resume("r1"));
    let (_second, order) = rig.started().await;
    assert_eq!(
        order.resume,
        Some(Resume {
            oracle_frozen: false
        })
    );
}

#[tokio::test(start_paused = true)]
async fn the_harness_is_started_before_a_resume_changes_the_phase_and_a_failure_leaves_it_paused() {
    // A harness that cannot be started.
    let mut rig = Rig::builder()
        .paused(Halted, Some("daemon restarted"), Building)
        .frozen(None)
        .start();
    rig.connector.fail_next("harness binary missing");
    rig.command(resume("r1"));
    rig.error_containing("harness binary missing").await;
    assert_eq!(rig.phase(), None, "a failed resume moved the unit");

    // A harness that does not declare resume.
    rig.command(resume("r2"));
    let mut peer = rig.peer().await;
    let mut no_resume = peer_capabilities();
    no_resume.resume = false;
    peer.initialize(no_resume).await;
    rig.until("the second refusal", |r| r.errors().len() == 2)
        .await;
    assert!(
        rig.recorder.errors()[1].1.contains("resume"),
        "{:?}",
        rig.recorder.errors()
    );
    assert_eq!(rig.phase(), None);
    rig.settle_tasks().await;
    assert!(peer.was_killed());

    // Third time: it starts, and only then does the phase change.
    rig.command(resume("r3"));
    let (_peer, order) = rig.started().await;
    assert_eq!(
        order.resume,
        Some(Resume {
            oracle_frozen: true
        })
    );
    assert_eq!(rig.recorder.phases(), [Provisioning]);
}

#[tokio::test(start_paused = true)]
async fn a_unit_at_an_exhausted_cap_is_not_resumed_and_no_harness_is_started() {
    for (spent, elapsed_ms, cap) in [(5.0, 0, "usd"), (0.0, 1_800_000, "wall")] {
        let mut rig = Rig::builder()
            .caps(5.0, 1800)
            .spent(spent, elapsed_ms)
            .paused(NeedsHuman, Some("usd cap"), Building)
            .frozen(None)
            .start();
        rig.command(resume("r1"));
        rig.error_containing("exhausted").await;
        assert!(system_error(&rig, cap), "{:?}", rig.recorder.errors());
        assert_eq!(rig.phase(), None);
        assert_eq!(rig.connector.connects(), 0);
    }
}

#[tokio::test(start_paused = true)]
async fn a_resumed_harness_waits_for_a_slot_without_any_clock_running() {
    let slots = Arc::new(Semaphore::new(1));
    let held = slots.clone().acquire_owned().await.unwrap();
    let mut rig = Rig::builder()
        .slots(slots)
        .caps(5.0, 100)
        .paused(Halted, Some("daemon restarted"), Building)
        .frozen(None)
        .start();
    rig.command(resume("r1"));
    let mut peer = rig.peer().await;
    peer.initialize(peer_capabilities()).await;
    rig.in_phase(Provisioning).await;
    tokio::time::sleep(secs(10_000)).await;
    assert_eq!(
        rig.phase(),
        Some(Provisioning),
        "an idle harness behind a held slot stalled"
    );
    assert!(!peer.was_killed());
    drop(held);
    let order = peer.start().await;
    assert_eq!(order.caps.wall_clock_secs, 100);
}

#[tokio::test(start_paused = true)]
async fn abandon_while_a_resumed_harness_is_starting_kills_it_and_fails_the_unit() {
    let mut rig = Rig::builder()
        .paused(Halted, None, Building)
        .frozen(None)
        .start();
    rig.command(resume("r1"));
    let peer = rig.peer().await;
    // The harness has not answered `initialize`; the unit is still Halted.
    rig.command(abandon("a1"));
    assert_eq!(rig.finish().await, Failed);
    assert!(peer.was_killed());
    assert_eq!(rig.recorder.phases(), [Failed]);
}

#[tokio::test(start_paused = true)]
async fn a_unit_stopped_for_oracle_tampering_cannot_be_resumed_only_abandoned() {
    let mut rig = Rig::builder()
        .tier(Tier::T2)
        .paused(NeedsHuman, Some(REASON_ORACLE_TAMPERING), MergeCheck)
        .start();
    rig.command(resume("r1"));
    rig.error_containing("oracle tampering").await;
    assert_eq!(rig.phase(), None);
    assert_eq!(
        rig.connector.connects(),
        0,
        "a harness was started for a tampered unit"
    );
    rig.command(abandon("a1"));
    assert_eq!(rig.finish().await, Failed);
}

// --- the rest of the matrix ---

#[tokio::test(start_paused = true)]
async fn a_paused_unit_can_be_halted_and_either_kind_of_pause_can_be_abandoned() {
    let mut rig = Rig::builder()
        .paused(NeedsHuman, Some("stalled"), Building)
        .start();
    rig.command(halt("h1"));
    rig.in_phase(Halted).await;
    rig.command(abandon("a1"));
    assert_eq!(rig.finish().await, Failed);
    assert_eq!(rig.recorder.phases(), [Halted, Failed]);
    assert_eq!(rig.recorder.done().as_deref(), Some("failed"));

    let mut rig = Rig::builder()
        .paused(NeedsHuman, Some("stalled"), Building)
        .start();
    rig.command(abandon("a1"));
    assert_eq!(rig.finish().await, Failed);
}

#[tokio::test(start_paused = true)]
async fn a_command_that_does_not_apply_is_refused_by_id_and_phase_and_changes_nothing() {
    let mut rig = Rig::builder().start();
    let (_peer, _order) = rig.building().await;
    for (command, id) in [
        (resume("r1"), "r1"),
        (ship("s1"), "s1"),
        (approve("o1"), "o1"),
    ] {
        rig.command(command);
        rig.error_containing(id).await;
    }
    assert!(system_error(&rig, "command r1 not valid in building"));
    assert_eq!(rig.phase(), Some(Building));
}

#[tokio::test(start_paused = true)]
async fn ship_is_accepted_only_from_awaiting_ship() {
    // Paused for a cap: not awaiting ship.
    let mut rig = Rig::builder()
        .tier(Tier::T3)
        .paused(NeedsHuman, Some("usd cap"), Building)
        .start();
    rig.command(ship("s1"));
    rig.error_containing("s1").await;
    assert_eq!(rig.phase(), None);
    assert!(rig.forge.opened().is_empty());

    // Brought back from the store awaiting ship, with the result an earlier process verified.
    let delivered = pr_open(evidence(
        "factory/u1",
        fleet_supervisor::testkit::HEAD,
        "delivered.bundle",
    ));
    let mut rig = Rig::builder()
        .tier(Tier::T3)
        .paused(NeedsHuman, Some(REASON_AWAITING_SHIP), MergeCheck)
        .last_result(delivered)
        .start();
    rig.command(ship("s2"));
    assert_eq!(rig.finish().await, Done);
    assert_eq!(rig.forge.opened().len(), 1);
    assert_eq!(rig.connector.connects(), 0, "shipping started a harness");
}

#[tokio::test(start_paused = true)]
async fn while_ships_pull_request_is_being_opened_halt_resume_and_ship_are_refused() {
    let delivered = pr_open(evidence(
        "factory/u1",
        fleet_supervisor::testkit::HEAD,
        "delivered.bundle",
    ));
    let mut rig = Rig::builder()
        .tier(Tier::T3)
        .paused(NeedsHuman, Some(REASON_AWAITING_SHIP), MergeCheck)
        .last_result(delivered)
        .start();
    rig.forge.slow("open_pr", secs(60));
    rig.command(ship("s1"));
    rig.in_phase(PrOpen).await;
    rig.command(halt("h1"));
    rig.command(resume("r1"));
    rig.command(ship("s2"));
    rig.error_containing("s2").await;
    assert!(system_error(
        &rig,
        "halt not available now; abandon instead"
    ));
    assert!(system_error(&rig, "r1"));
    assert_eq!(rig.finish().await, Done);
    assert_eq!(
        rig.forge.opened().len(),
        1,
        "a second pull request was opened"
    );
}

#[tokio::test(start_paused = true)]
async fn abandon_while_a_result_is_being_verified_cancels_the_verification() {
    let mut rig = Rig::builder().start();
    rig.verifier.delay(secs(3600));
    let (mut peer, order) = rig.merge_check().await;
    peer.deliver(&order).await;
    peer.exit(0);
    rig.settle_tasks().await;
    assert_eq!(rig.verifier.inputs().len(), 1, "verification has begun");
    rig.command(halt("h1"));
    rig.error_containing("halt not available now; abandon instead")
        .await;
    rig.command(abandon("a1"));
    let asked = tokio::time::Instant::now();
    assert_eq!(rig.finish().await, Failed);
    assert!(
        asked.elapsed() < secs(60),
        "abandon waited for the verification to finish"
    );
    assert!(rig.forge.opened().is_empty());
}

#[tokio::test(start_paused = true)]
async fn a_closed_command_channel_keeps_a_live_unit_running_and_fails_a_paused_one_closed() {
    // Live: the caller sends nothing more, and the unit still runs to the end.
    let mut rig = Rig::builder().start();
    let (mut peer, order) = rig.building().await;
    rig.close_commands();
    peer.cycle(1, 0).await;
    peer.deliver(&order).await;
    peer.exit(0);
    assert_eq!(rig.finish().await, Done);

    // Paused with nothing in flight: nobody can ever resume it, so it fails closed.
    let mut rig = Rig::builder()
        .verdicts(vec![Verdict::MergeConflict])
        .start();
    let (mut peer, order) = rig.merge_check().await;
    peer.deliver(&order).await;
    peer.exit(0);
    rig.in_phase(NeedsHuman).await;
    rig.close_commands();
    assert_eq!(rig.finish().await, Failed);
    assert!(system_error(&rig, "command channel closed"));
}
````

`crates/fleet-supervisor/tests/verdicts.rs`:

````rust
//! Lane CC-SUPERVISOR: what each result and each verdict does, the pull request, `no_change`,
//! and verifying again without a harness. Supervisor spec section 4.8 and section 6 item 4,
//! with the two verdicts and the `Reverify` command that protocol 0.2 adds.

use fleet_core::{ErrorScope, Event, Phase, Tier};
use fleet_forge::{body_carries, Mergeability, WorkItemRef};
use fleet_supervisor::testkit::{abandon, approve, resume, reverify, ship, Rig, GATE_HASH, HEAD};
use fleet_supervisor::{MAX_MERGEABLE_POLLS, REASON_ORACLE_TAMPERING, REASON_VERIFY_STOPPED};
use fleet_verify::testkit::{evidence, no_change, pr_open, World};
use fleet_verify::{Check, Verdict};
use harness_protocol::{DeliveryEvidence, Observation};

use Phase::*;

fn has_error(rig: &Rig, scope: ErrorScope, needle: &str) -> bool {
    rig.recorder
        .errors()
        .iter()
        .any(|(s, detail)| *s == scope && detail.contains(needle))
}

/// A unit whose harness delivered and exited, verified with `verdict`.
async fn verified_as(verdict: Verdict) -> Rig {
    let mut rig = Rig::builder().verdicts(vec![verdict]).start();
    let (mut peer, order) = rig.merge_check().await;
    peer.deliver(&order).await;
    peer.exit(0);
    rig
}

fn unrunnable() -> Verdict {
    Verdict::Unrunnable {
        check: Check::Tests,
        detail: "the container engine is not available".into(),
    }
}

fn tests_failed() -> Verdict {
    Verdict::TestsFailed {
        failing: vec!["test/codes.test.js > codes > ac1 applies a percent code".into()],
        detail: "1 of 2 frozen ids did not pass".into(),
    }
}

// --- pr_open, by verdict ---

#[tokio::test(start_paused = true)]
async fn a_rejected_delivery_fails_the_unit_naming_the_check_and_is_not_retried() {
    let mut rig = verified_as(Verdict::Rejected {
        check: Check::Diff,
        detail: "src/other.js is outside scope".into(),
    })
    .await;
    assert_eq!(rig.finish().await, Failed);
    assert!(has_error(
        &rig,
        ErrorScope::System,
        "evidence rejected: Diff: src/other.js is outside scope"
    ));
    assert_eq!(rig.verifier.inputs().len(), 1);
    assert!(rig.forge.opened().is_empty());
}

#[tokio::test(start_paused = true)]
async fn a_pr_open_that_verifies_as_empty_fails_the_unit() {
    let mut rig = verified_as(Verdict::NoChange).await;
    assert_eq!(rig.finish().await, Failed);
    assert!(has_error(
        &rig,
        ErrorScope::System,
        "pr_open verified as empty"
    ));
}

#[tokio::test(start_paused = true)]
async fn tampering_pauses_the_unit_with_the_reason_the_store_folds_on() {
    let mut rig = verified_as(Verdict::Tampered {
        detail: "test/codes.test.js changed after the freeze".into(),
    })
    .await;
    rig.in_phase(NeedsHuman).await;
    assert_eq!(
        rig.recorder.last_reason().as_deref(),
        Some(REASON_ORACLE_TAMPERING)
    );
    assert!(rig.recorder.phases().ends_with(&[MergeCheck, NeedsHuman]));
    assert!(rig.forge.opened().is_empty());
    assert!(rig
        .recorder
        .blocked()
        .iter()
        .any(|(_, _, detail)| detail.contains("changed after the freeze")));
}

#[tokio::test(start_paused = true)]
async fn a_merge_conflict_pauses_the_unit_and_opens_no_pull_request() {
    let mut rig = verified_as(Verdict::MergeConflict).await;
    rig.in_phase(NeedsHuman).await;
    assert!(rig.forge.opened().is_empty());
    // Not awaiting ship: Ship is refused.
    rig.command(ship("s1"));
    rig.error_containing("s1").await;
    assert_eq!(rig.phase(), Some(NeedsHuman));
}

// --- the supervisor's own checks before the verifier is called ---

#[tokio::test(start_paused = true)]
async fn evidence_for_another_branch_fails_the_unit_without_calling_the_verifier() {
    let mut rig = Rig::builder().start();
    let (mut peer, _order) = rig.merge_check().await;
    peer.result(pr_open(evidence(
        "factory/someone-else",
        HEAD,
        "delivered.bundle",
    )))
    .await;
    peer.exit(0);
    assert_eq!(rig.finish().await, Failed);
    assert!(has_error(
        &rig,
        ErrorScope::System,
        "evidence rejected: Link"
    ));
    assert!(rig.verifier.inputs().is_empty());
}

#[tokio::test(start_paused = true)]
async fn a_push_delivery_fails_the_unit_without_calling_the_verifier() {
    let mut rig = Rig::builder().start();
    let (mut peer, order) = rig.merge_check().await;
    let mut pushed = evidence(&order.branch, HEAD, "unused");
    pushed.delivery = DeliveryEvidence::Push;
    peer.result(pr_open(pushed)).await;
    peer.exit(0);
    assert_eq!(rig.finish().await, Failed);
    assert!(has_error(
        &rig,
        ErrorScope::System,
        "evidence rejected: Link"
    ));
    assert!(rig.verifier.inputs().is_empty());
}

// --- what the verifier is handed ---

#[tokio::test(start_paused = true)]
async fn at_t2_the_approved_hash_reaches_the_verifier() {
    let mut rig = Rig::builder().tier(Tier::T2).start();
    let (mut peer, order, gate) = rig.at_gate().await;
    rig.command(approve("c1"));
    peer.gate_reply(gate).await;
    rig.in_phase(Building).await;
    peer.cycle(1, 0).await;
    peer.deliver(&order).await;
    peer.exit(0);
    assert_eq!(rig.finish().await, Done);
    let input = &rig.verifier.inputs()[0];
    assert_eq!(input.approved_oracle_hash.as_deref(), Some(GATE_HASH));
    assert_eq!(input.order.unit_id, "u1");
    assert_eq!(input.result.evidence.as_ref().unwrap().head_sha, HEAD);
}

#[tokio::test(start_paused = true)]
async fn a_factory_unit_is_verified_against_the_signed_bytes_and_its_pull_request_carries_the_work_item(
) {
    let dir = tempfile::tempdir().unwrap();
    let world = World::good(dir.path());
    let mut rig = Rig::builder()
        .world(&world, &dir.path().join("work"))
        .start();
    let (mut peer, order) = rig.merge_check().await;
    peer.deliver(&order).await;
    peer.exit(0);
    assert_eq!(rig.finish().await, Done);

    let input = &rig.verifier.inputs()[0];
    assert_eq!(input.signed_spec.as_deref(), Some(&world.spec_bytes[..]));
    let item = WorkItemRef::parse("example/sandbox#1", None).unwrap();
    let body = input
        .pr_body
        .as_deref()
        .expect("the body the pull request will have");
    assert!(body_carries(body, &item, "owner/sandbox"));
    let (request, _) = &rig.forge.opened()[0];
    assert_eq!(
        request.body, body,
        "the body that was verified is the body that was sent"
    );
    assert_eq!(request.repo.slug, "owner/sandbox");
    assert!(request.title.contains("example/sandbox#1"));
}

// --- the pull request ---

#[tokio::test(start_paused = true)]
async fn a_pull_request_that_cannot_be_opened_fails_the_unit() {
    let mut rig = Rig::builder().start();
    rig.forge.fail("open_pr");
    let (mut peer, order) = rig.merge_check().await;
    peer.deliver(&order).await;
    peer.exit(0);
    assert_eq!(rig.finish().await, Failed);
    assert!(has_error(&rig, ErrorScope::Github, "open_pr"));
}

#[tokio::test(start_paused = true)]
async fn a_pull_request_that_stays_pending_is_polled_a_fixed_number_of_times_then_a_person_is_asked(
) {
    let mut rig = Rig::builder().mergeable(Mergeability::Pending).start();
    let (mut peer, order) = rig.merge_check().await;
    peer.deliver(&order).await;
    peer.exit(0);
    rig.in_phase(NeedsHuman).await;
    let polls = rig
        .forge
        .calls()
        .iter()
        .filter(|c| *c == "pr_state")
        .count();
    assert_eq!(polls as u32, MAX_MERGEABLE_POLLS);
    assert_eq!(
        rig.recorder
            .blocked()
            .last()
            .map(|b| b.0.clone())
            .as_deref(),
        Some("mergeability poll timed out")
    );
    assert!(rig.recorder.phases().ends_with(&[PrOpen, NeedsHuman]));
    assert_eq!(rig.forge.opened().len(), 1, "the pull request stays open");
    rig.command(ship("s1"));
    rig.error_containing("s1").await;
    assert_eq!(
        rig.phase(),
        Some(NeedsHuman),
        "Ship re-polled a unit that was not awaiting ship"
    );
}

#[tokio::test(start_paused = true)]
async fn a_dirty_pull_request_pauses_the_unit_with_base_moved() {
    let mut rig = Rig::builder().mergeable(Mergeability::Dirty).start();
    let (mut peer, order) = rig.merge_check().await;
    peer.deliver(&order).await;
    peer.exit(0);
    rig.in_phase(NeedsHuman).await;
    assert_eq!(rig.recorder.last_reason().as_deref(), Some("base moved"));
    assert_eq!(
        rig.forge
            .calls()
            .iter()
            .filter(|c| *c == "pr_state")
            .count(),
        1
    );
}

#[tokio::test(start_paused = true)]
async fn a_poll_that_fails_is_reported_and_counted_as_one_of_the_polls() {
    let mut rig = Rig::builder().start();
    rig.forge.fail("pr_state");
    let (mut peer, order) = rig.merge_check().await;
    peer.deliver(&order).await;
    peer.exit(0);
    rig.in_phase(NeedsHuman).await;
    let github_errors = rig
        .recorder
        .errors()
        .iter()
        .filter(|(scope, _)| *scope == ErrorScope::Github)
        .count();
    assert_eq!(github_errors as u32, MAX_MERGEABLE_POLLS);
}

// --- no_change ---

#[tokio::test(start_paused = true)]
async fn empty_diff_keeps_the_unit_in_checking_until_the_no_change_result_is_verified() {
    let mut rig = Rig::builder().verdicts(vec![Verdict::NoChange]).start();
    let (mut peer, order) = rig.building().await;
    peer.observe(Observation::BuildFinished).await;
    peer.observe(Observation::EmptyDiff).await;
    peer.metric(0.1).await;
    rig.until("the metric", |r| r.last_metric().is_some()).await;
    assert_eq!(
        rig.phase(),
        Some(Checking),
        "empty_diff ended the unit before it was verified"
    );

    peer.result(no_change(evidence(
        &order.branch,
        HEAD,
        "frozen-tests.bundle",
    )))
    .await;
    peer.exit(0);
    assert_eq!(rig.finish().await, NoChange);
    assert_eq!(rig.recorder.done().as_deref(), Some("no_change"));
    assert!(rig.forge.opened().is_empty());
}

#[tokio::test(start_paused = true)]
async fn a_no_change_result_that_does_not_verify_as_no_change_fails_the_unit() {
    for verdict in [Verdict::Verified, Verdict::MergeConflict] {
        let mut rig = Rig::builder().verdicts(vec![verdict.clone()]).start();
        let (mut peer, order) = rig.building().await;
        peer.observe(Observation::BuildFinished).await;
        peer.observe(Observation::EmptyDiff).await;
        peer.metric(0.1).await;
        peer.result(no_change(evidence(
            &order.branch,
            HEAD,
            "frozen-tests.bundle",
        )))
        .await;
        peer.exit(0);
        assert_eq!(rig.finish().await, Failed, "{verdict:?}");
    }
}

#[tokio::test(start_paused = true)]
async fn after_empty_diff_only_closing_messages_are_legal() {
    let mut rig = Rig::builder().start();
    let (mut peer, _order) = rig.building().await;
    peer.observe(Observation::BuildFinished).await;
    peer.observe(Observation::EmptyDiff).await;
    peer.observe(Observation::ChecksPassed).await;
    assert_eq!(rig.finish().await, Failed);
    assert!(has_error(&rig, ErrorScope::Harness, "protocol violation"));
}

#[tokio::test(start_paused = true)]
async fn a_no_change_result_with_no_empty_diff_before_it_is_a_violation() {
    let mut rig = Rig::builder().start();
    let (mut peer, order) = rig.building().await;
    peer.observe(Observation::BuildFinished).await;
    peer.metric(0.1).await;
    peer.result(no_change(evidence(
        &order.branch,
        HEAD,
        "frozen-tests.bundle",
    )))
    .await;
    assert_eq!(rig.finish().await, Failed);
    assert!(has_error(&rig, ErrorScope::Harness, "protocol violation"));
    assert!(rig.verifier.inputs().is_empty());
}

// --- a verification that could not run, or whose tests failed: stop, and verify again ---

#[tokio::test(start_paused = true)]
async fn a_verification_that_could_not_run_pauses_the_unit_and_never_fails_or_passes_it() {
    let mut rig = verified_as(unrunnable()).await;
    rig.in_phase(NeedsHuman).await;
    assert_eq!(
        rig.recorder.last_reason().as_deref(),
        Some(REASON_VERIFY_STOPPED)
    );
    assert!(rig.recorder.phases().ends_with(&[MergeCheck, NeedsHuman]));
    let blocked = rig.recorder.blocked();
    assert_eq!(blocked.last().unwrap().0, REASON_VERIFY_STOPPED);
    assert!(blocked
        .last()
        .unwrap()
        .2
        .contains("the container engine is not available"));
    assert!(rig.forge.opened().is_empty());
    assert_eq!(rig.slots.available_permits(), 3);
}

#[tokio::test(start_paused = true)]
async fn reverify_runs_verification_again_on_the_same_result_without_starting_a_harness() {
    let mut rig = verified_as(unrunnable()).await;
    rig.in_phase(NeedsHuman).await;
    rig.command(reverify("v1"));
    assert_eq!(rig.finish().await, Done);

    assert_eq!(
        rig.connector.connects(),
        1,
        "re-verifying started a harness"
    );
    let inputs = rig.verifier.inputs();
    assert_eq!(inputs.len(), 2);
    assert_eq!(
        inputs[0], inputs[1],
        "the second verification saw a different input"
    );
    assert!(rig
        .recorder
        .phases()
        .ends_with(&[NeedsHuman, MergeCheck, PrOpen, Done]));
    assert!(rig.recorder.events().iter().any(|e| matches!(
        e,
        Event::PhaseChanged { from: NeedsHuman, to: MergeCheck, cmd_id: Some(id), .. } if id == "v1"
    )));
    assert_eq!(rig.forge.opened().len(), 1);
}

#[tokio::test(start_paused = true)]
async fn tests_that_fail_in_the_control_planes_run_stop_the_unit_as_often_as_they_fail() {
    let mut rig = Rig::builder()
        .verdicts(vec![tests_failed(), tests_failed()])
        .start();
    let (mut peer, order) = rig.merge_check().await;
    peer.deliver(&order).await;
    peer.exit(0);
    rig.in_phase(NeedsHuman).await;
    assert!(rig
        .recorder
        .blocked()
        .last()
        .unwrap()
        .2
        .contains("ac1 applies a percent code"));

    rig.command(reverify("v1"));
    rig.until("stopped a second time", |r| {
        r.phases().ends_with(&[NeedsHuman, MergeCheck, NeedsHuman])
    })
    .await;
    assert_eq!(
        rig.recorder.last_reason().as_deref(),
        Some(REASON_VERIFY_STOPPED)
    );

    // The third verification passes: one flaky test did not cost the unit.
    rig.command(reverify("v2"));
    assert_eq!(rig.finish().await, Done);
    assert_eq!(rig.verifier.inputs().len(), 3);
}

#[tokio::test(start_paused = true)]
async fn reverify_is_refused_for_a_unit_that_was_not_stopped_by_verification() {
    let mut rig = verified_as(Verdict::MergeConflict).await;
    rig.in_phase(NeedsHuman).await;
    rig.command(reverify("v1"));
    rig.error_containing("v1").await;
    assert_eq!(rig.phase(), Some(NeedsHuman));
    assert_eq!(rig.verifier.inputs().len(), 1);

    let mut rig = Rig::builder().start();
    let (_peer, _order) = rig.building().await;
    rig.command(reverify("v2"));
    rig.error_containing("command v2 not valid in building")
        .await;
}

#[tokio::test(start_paused = true)]
async fn a_no_change_result_whose_verification_could_not_run_is_verified_again_from_checking() {
    let mut rig = Rig::builder()
        .verdicts(vec![unrunnable(), Verdict::NoChange])
        .start();
    let (mut peer, order) = rig.building().await;
    peer.observe(Observation::BuildFinished).await;
    peer.observe(Observation::EmptyDiff).await;
    peer.metric(0.1).await;
    peer.result(no_change(evidence(
        &order.branch,
        HEAD,
        "frozen-tests.bundle",
    )))
    .await;
    peer.exit(0);
    rig.in_phase(NeedsHuman).await;
    assert!(rig.recorder.phases().ends_with(&[Checking, NeedsHuman]));
    rig.command(reverify("v1"));
    assert_eq!(rig.finish().await, NoChange);
    assert!(rig
        .recorder
        .phases()
        .ends_with(&[NeedsHuman, Checking, NoChange]));
}

#[tokio::test(start_paused = true)]
async fn a_unit_brought_back_from_the_store_can_be_verified_again_from_its_kept_result() {
    let kept = pr_open(evidence("factory/u1", HEAD, "delivered.bundle"));
    let mut rig = Rig::builder()
        .paused(NeedsHuman, Some(REASON_VERIFY_STOPPED), MergeCheck)
        .frozen(None)
        .last_result(kept.clone())
        .start();
    rig.command(reverify("v1"));
    assert_eq!(rig.finish().await, Done);
    assert_eq!(rig.connector.connects(), 0);
    let input = &rig.verifier.inputs()[0];
    assert_eq!(input.result, kept);
    assert!(
        input.freeze.is_some(),
        "the kept freeze reaches the verifier after a restart"
    );
    assert_eq!(rig.recorder.phases(), [MergeCheck, PrOpen, Done]);
}

#[tokio::test(start_paused = true)]
async fn without_a_kept_result_reverify_is_refused_and_the_unit_can_still_be_resumed() {
    let mut rig = Rig::builder()
        .paused(NeedsHuman, Some(REASON_VERIFY_STOPPED), MergeCheck)
        .frozen(None)
        .start();
    rig.command(reverify("v1"));
    rig.error_containing("no result is held").await;
    assert_eq!(rig.phase(), None);
    assert!(rig.verifier.inputs().is_empty());
    rig.command(resume("r1"));
    let (_peer, order) = rig.started().await;
    assert!(order.resume.is_some());
}

#[tokio::test(start_paused = true)]
async fn a_unit_stopped_by_verification_can_be_abandoned() {
    let mut rig = verified_as(tests_failed()).await;
    rig.in_phase(NeedsHuman).await;
    rig.command(abandon("a1"));
    assert_eq!(rig.finish().await, Failed);
}
````

`crates/fleet-supervisor/tests/registry.rs`:

````rust
//! Lane CC-SUPERVISOR: the eligibility rule and the harness registry.
//! Supervisor spec sections 3.3 and 4.1, and section 6 item 6. No process is started here
//! except one that does not exist.

use fleet_core::Tier;
use fleet_supervisor::testkit::peer_capabilities;
use fleet_supervisor::{
    eligibility, fake_binary_beside, HarnessEntry, ProbeState, Registry, RegistryError,
    FAKE_HARNESS,
};
use harness_protocol::{Capabilities, Delivery, Isolation, Metering, ProfileInfo, UnitKind};
use std::path::{Path, PathBuf};
use std::time::Duration;

fn with(change: impl Fn(&mut Capabilities)) -> Capabilities {
    let mut capabilities = peer_capabilities();
    change(&mut capabilities);
    capabilities
}

// --- eligibility ---

#[test]
fn a_container_harness_that_meters_and_has_the_gate_may_run_every_tier() {
    for tier in [Tier::T1, Tier::T2, Tier::T3] {
        assert_eq!(eligibility(tier, &peer_capabilities(), false, None), Ok(()));
    }
}

#[test]
fn t2_and_t3_need_the_oracle_gate_and_no_opt_in_changes_that() {
    let gateless = with(|c| c.gates.clear());
    assert_eq!(eligibility(Tier::T1, &gateless, false, None), Ok(()));
    for tier in [Tier::T2, Tier::T3] {
        for opt_in in [false, true] {
            let reason = eligibility(tier, &gateless, opt_in, None).unwrap_err();
            assert!(reason.contains("oracle"), "{reason}");
        }
    }
}

#[test]
fn weaker_isolation_or_no_metering_needs_the_opt_in_and_the_reason_names_the_guarantee() {
    for (weak, word) in [
        (with(|c| c.isolation = Isolation::Host), "isolation"),
        (with(|c| c.isolation = Isolation::None), "isolation"),
        (with(|c| c.metering = Metering::None), "metering"),
    ] {
        let reason = eligibility(Tier::T1, &weak, false, None).unwrap_err();
        assert!(reason.contains(word), "{reason}");
        assert_eq!(eligibility(Tier::T1, &weak, true, None), Ok(()));
    }
}

#[test]
fn push_delivery_and_a_harness_that_cannot_build_are_refused_whatever_the_opt_in() {
    let pushing = with(|c| c.delivery = Delivery::Push);
    let drafting = with(|c| c.kinds = vec![UnitKind::Draft]);
    for opt_in in [false, true] {
        assert!(eligibility(Tier::T1, &pushing, opt_in, None)
            .unwrap_err()
            .contains("push"));
        assert!(eligibility(Tier::T1, &drafting, opt_in, None)
            .unwrap_err()
            .contains("build"));
    }
    // A 0.1 harness declares no kinds at all: that means build only.
    assert_eq!(
        eligibility(Tier::T1, &with(|c| c.kinds.clear()), false, None),
        Ok(())
    );
}

#[test]
fn a_profile_must_be_declared_and_an_unpriced_one_needs_the_opt_in() {
    let profiles = with(|c| {
        c.profiles = vec![
            ProfileInfo {
                name: "claude".into(),
                priced: true,
            },
            ProfileInfo {
                name: "hybrid-local".into(),
                priced: false,
            },
        ]
    });
    assert_eq!(
        eligibility(Tier::T1, &profiles, false, Some("claude")),
        Ok(())
    );
    assert!(eligibility(Tier::T1, &profiles, false, Some("nobody"))
        .unwrap_err()
        .contains("nobody"));
    assert!(
        eligibility(Tier::T1, &profiles, false, Some("hybrid-local"))
            .unwrap_err()
            .contains("unpriced")
    );
    assert_eq!(
        eligibility(Tier::T1, &profiles, true, Some("hybrid-local")),
        Ok(())
    );
    // No profile asked for: the harness's own default, which needs nothing declared.
    assert_eq!(
        eligibility(Tier::T1, &peer_capabilities(), false, None),
        Ok(())
    );
}

// --- where the scripted harness is looked for ---

#[test]
fn the_scripted_harness_is_expected_beside_the_executable_or_above_a_deps_directory() {
    let name = if cfg!(windows) {
        "harness-fake.exe"
    } else {
        "harness-fake"
    };
    let beside = Path::new("target").join("debug").join("fleetd-serve");
    assert_eq!(
        fake_binary_beside(&beside),
        Path::new("target").join("debug").join(name)
    );
    let test_binary = Path::new("target")
        .join("debug")
        .join("deps")
        .join("life-0123abcd");
    assert_eq!(
        fake_binary_beside(&test_binary),
        Path::new("target").join("debug").join(name)
    );
    // A directory merely containing "deps" in its name is not the cargo deps directory.
    let lookalike = Path::new("opt").join("my-deps").join("serve");
    assert_eq!(
        fake_binary_beside(&lookalike),
        Path::new("opt").join("my-deps").join(name)
    );
}

#[test]
fn the_default_registry_has_one_unprobed_entry_for_the_scripted_harness() {
    let registry = Registry::default_fake();
    assert_eq!(
        registry.states(),
        [(FAKE_HARNESS.to_string(), ProbeState::Unprobed)]
    );
    let entry = registry.entry(FAKE_HARNESS).unwrap();
    assert_eq!(entry.command.len(), 1);
    assert!(entry.command[0].contains("harness-fake"));
    assert!(registry.entry("reqdrive").is_none());
    assert!(registry.connector("reqdrive").is_none());
}

// --- the registry file ---

const FILE: &str = r#"
[[harness]]
name = "reqdrive"
command = ["C:/tools/reqdrive.exe", "harness"]

[harness.env]
REQDRIVE_HOME = "C:/Users/me/.reqdrive"

[[harness]]
name = "second"
command = ["second-harness"]
"#;

#[test]
fn a_registry_file_names_each_harness_its_command_and_its_environment() {
    let registry = Registry::from_toml(FILE).unwrap();
    let names: Vec<String> = registry
        .states()
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    assert_eq!(names, ["reqdrive", "second"]);
    assert_eq!(
        registry.entry("reqdrive"),
        Some(HarnessEntry {
            name: "reqdrive".into(),
            command: vec!["C:/tools/reqdrive.exe".into(), "harness".into()],
            env: vec![("REQDRIVE_HOME".into(), "C:/Users/me/.reqdrive".into())],
        })
    );
    assert_eq!(registry.entry("second").unwrap().env, Vec::new());
}

#[test]
fn a_registry_file_with_crlf_line_endings_and_a_byte_order_mark_reads_the_same() {
    let windows = format!("\u{feff}{}", FILE.replace('\n', "\r\n"));
    let registry = Registry::from_toml(&windows).unwrap();
    assert_eq!(
        registry.entry("reqdrive"),
        Registry::from_toml(FILE).unwrap().entry("reqdrive")
    );
}

#[test]
fn a_registry_file_that_is_wrong_is_refused_with_what_is_wrong() {
    let twice = format!("{FILE}\n[[harness]]\nname = \"second\"\ncommand = [\"x\"]\n");
    assert_eq!(
        Registry::from_toml(&twice).err(),
        Some(RegistryError::Duplicate {
            name: "second".into()
        })
    );
    assert_eq!(
        Registry::from_toml("[[harness]]\nname = \"empty\"\ncommand = []\n").err(),
        Some(RegistryError::EmptyCommand {
            name: "empty".into()
        })
    );
    for malformed in [
        "[[harness]]\nname = \"x\"\n",
        "[[harness]]\nname = \"x\"\ncommand = \"one string\"\n",
        "[[harness]]\nname = \"x\"\ncommand = [\"x\"]\nshell = true\n",
        "not toml at all [",
    ] {
        assert!(
            matches!(
                Registry::from_toml(malformed),
                Err(RegistryError::Malformed(_))
            ),
            "{malformed:?}"
        );
    }
    assert_eq!(Registry::from_toml("").unwrap().states(), Vec::new());
}

#[test]
fn a_registry_file_that_does_not_exist_is_unreadable_not_empty() {
    let missing = PathBuf::from("no-such-directory").join("harnesses.toml");
    assert!(matches!(
        Registry::load(&missing),
        Err(RegistryError::Unreadable(_))
    ));
}

#[test]
fn a_file_entry_replaces_a_built_in_entry_of_the_same_name_and_adds_the_rest() {
    let file = Registry::from_toml(
        "[[harness]]\nname = \"fake\"\ncommand = [\"my-fake\"]\n\n[[harness]]\nname = \"reqdrive\"\ncommand = [\"reqdrive\", \"harness\"]\n",
    )
    .unwrap();
    let merged = Registry::default_fake().merged(file);
    let names: Vec<String> = merged.states().into_iter().map(|(name, _)| name).collect();
    assert_eq!(names, ["fake", "reqdrive"]);
    assert_eq!(merged.entry("fake").unwrap().command, ["my-fake"]);
}

// --- the probe ---

#[tokio::test]
async fn a_program_that_does_not_exist_probes_unhealthy_with_its_path_and_the_others_are_still_probed(
) {
    let registry = Registry::new(vec![
        HarnessEntry {
            name: "missing".into(),
            command: vec!["no-such-harness-program".into()],
            env: Vec::new(),
        },
        HarnessEntry {
            name: "also-missing".into(),
            command: vec!["no-such-harness-program-2".into(), "harness".into()],
            env: Vec::new(),
        },
    ]);
    registry.probe(Duration::from_secs(5)).await;
    for name in ["missing", "also-missing"] {
        match registry.state(name) {
            Some(ProbeState::Unhealthy { error }) => {
                assert!(error.contains("no-such-harness-program"), "{error}")
            }
            other => panic!("{name}: {other:?}"),
        }
    }
    assert_eq!(registry.state("nobody"), None);
}

#[tokio::test]
async fn a_missing_scripted_harness_says_how_to_build_it() {
    let registry = Registry::new(vec![HarnessEntry {
        name: FAKE_HARNESS.into(),
        command: vec![PathBuf::from("nowhere")
            .join("harness-fake")
            .to_string_lossy()
            .into_owned()],
        env: Vec::new(),
    }]);
    registry.probe(Duration::from_secs(5)).await;
    match registry.state(FAKE_HARNESS) {
        Some(ProbeState::Unhealthy { error }) => {
            assert!(
                error.contains("cargo build -p harness-conformance --bins"),
                "{error}"
            )
        }
        other => panic!("{other:?}"),
    }
}
````

`crates/fleet-supervisor/tests/process_it.rs`:

````rust
//! Lane CC-SUPERVISOR, integration tier: the process shell, against the real `harness-fake`.
//! Supervisor spec section 6 items 5 and 6.
//!
//! Needs the `harness-fake` binary: `cargo build -p harness-conformance --bins` (a workspace
//! `cargo test` builds it). Needs no Docker and no network. Real time, real pipes.

use fleet_core::{Event, LogStream, Phase, Tier};
use fleet_forge::testkit::MemForge;
use fleet_supervisor::testkit::{approve, run_ctx, unit_spec, Recorder};
use fleet_supervisor::{
    fake_binary_beside, run, Connector, Deps, HarnessEntry, ProbeState, ProcessConnector, Registry,
    FAKE_HARNESS,
};
use fleet_verify::testkit::ScriptedVerifier;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Semaphore};

/// The commit `harness-fake` always says it delivered.
const FAKE_HEAD: &str = "0123456789abcdef0123456789abcdef01234567";

/// The registry entry for the built `harness-fake`, with extra environment.
fn fake(env: &[(&str, &str)]) -> HarnessEntry {
    let exe = std::env::current_exe().expect("the test binary's path");
    let binary = fake_binary_beside(&exe);
    assert!(
        binary.is_file(),
        "{} not found; run: cargo build -p harness-conformance --bins",
        binary.display()
    );
    HarnessEntry {
        name: FAKE_HARNESS.into(),
        command: vec![binary.to_string_lossy().into_owned()],
        env: env
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
    }
}

struct Running {
    commands: mpsc::UnboundedSender<fleet_core::Command>,
    recorder: Recorder,
    task: tokio::task::JoinHandle<Phase>,
}

fn start(entry: HarnessEntry, tier: Tier, grace: Duration, stall: Duration) -> Running {
    let forge = Arc::new(MemForge::new());
    // harness-fake reports a fixed head; the scripted verifier imports nothing.
    forge.mark_imported("u1", FAKE_HEAD);
    let mut spec = unit_spec();
    spec.tier = tier;
    spec.branch = "agent/u1".into();
    let mut ctx = run_ctx(Arc::new(Semaphore::new(1)), &std::env::temp_dir());
    ctx.opt_in = true; // harness-fake isolates nothing, and says so
    ctx.grace = grace;
    ctx.stall = stall;
    let recorder = Recorder::new();
    let (commands, command_rx) = mpsc::unbounded_channel();
    let connector: Arc<dyn Connector> = Arc::new(ProcessConnector::new(entry));
    let deps = Deps {
        verifier: Arc::new(ScriptedVerifier::new()),
        forge,
    };
    let task = tokio::spawn(run(connector, deps, spec, ctx, command_rx, recorder.sink()));
    Running {
        commands,
        recorder,
        task,
    }
}

async fn until(recorder: &Recorder, what: &str, done: impl Fn(&Recorder) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while !done(recorder) {
        assert!(
            Instant::now() < deadline,
            "{what}: never happened. {:#?}",
            recorder.signals()
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn the_real_scripted_harness_runs_a_t1_unit_to_done() {
    let running = start(
        fake(&[]),
        Tier::T1,
        Duration::from_secs(10),
        Duration::from_secs(30),
    );
    let phase = tokio::time::timeout(Duration::from_secs(60), running.task)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(phase, Phase::Done, "{:#?}", running.recorder.signals());
    // Its one line on standard error arrives as a system log line.
    assert!(running.recorder.events().iter().any(|e| matches!(
        e,
        Event::Log { stream: LogStream::System, line } if line.contains("harness-fake") && line.contains("starting")
    )));
    // All seven stages were announced.
    assert!(
        running.recorder.stages().len() >= 14,
        "{:?}",
        running.recorder.stages()
    );
}

#[tokio::test]
async fn the_real_scripted_harness_runs_a_t2_unit_through_its_gate_to_done() {
    let running = start(
        fake(&[]),
        Tier::T2,
        Duration::from_secs(10),
        Duration::from_secs(30),
    );
    until(&running.recorder, "the gate", |r| {
        r.phases().last() == Some(&Phase::AwaitingOracleApproval)
    })
    .await;
    running.commands.send(approve("c1")).unwrap();
    let phase = tokio::time::timeout(Duration::from_secs(60), running.task)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(phase, Phase::Done, "{:#?}", running.recorder.signals());
}

#[tokio::test]
async fn a_harness_that_hangs_is_stopped_by_the_stall_window_and_killed_after_the_grace_period() {
    let running = start(
        fake(&[("HARNESS_FAKE_MODE", "hang")]),
        Tier::T1,
        Duration::from_secs(1),
        Duration::from_secs(2),
    );
    let started = Instant::now();
    until(&running.recorder, "the stall", |r| {
        r.phases().last() == Some(&Phase::NeedsHuman)
    })
    .await;
    assert!(started.elapsed() < Duration::from_secs(20));
    assert_eq!(running.recorder.blocked().last().unwrap().0, "stalled");
    running.task.abort();
}

#[tokio::test]
async fn the_scripted_harness_that_stops_for_a_human_pauses_the_unit_through_the_real_pipe() {
    let running = start(
        fake(&[("HARNESS_FAKE_MODE", "needs_human")]),
        Tier::T1,
        Duration::from_secs(10),
        Duration::from_secs(30),
    );
    until(&running.recorder, "the stop", |r| {
        r.phases().last() == Some(&Phase::NeedsHuman)
    })
    .await;
    assert!(
        running.recorder.errors().is_empty(),
        "{:?}",
        running.recorder.errors()
    );
    running.task.abort();
}

#[tokio::test]
async fn a_harness_that_writes_without_pausing_does_not_deadlock_the_unit() {
    // No pause between the harness's messages: it writes as fast as the pipe takes them.
    // Standard error and standard output are drained independently, so neither can fill
    // while the other is being waited on.
    let running = start(
        fake(&[("HARNESS_FAKE_STEP_MS", "0")]),
        Tier::T1,
        Duration::from_secs(10),
        Duration::from_secs(30),
    );
    let phase = tokio::time::timeout(Duration::from_secs(60), running.task)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(phase, Phase::Done);
}

#[tokio::test]
async fn dropping_the_run_ends_the_harness_process_and_everything_it_started() {
    let running = start(
        fake(&[("HARNESS_FAKE_MODE", "hang")]),
        Tier::T1,
        Duration::from_secs(30),
        Duration::from_secs(600),
    );
    until(&running.recorder, "the harness has started", |r| {
        r.events()
            .iter()
            .any(|e| matches!(e, Event::Log { line, .. } if line.contains("starting")))
    })
    .await;
    let before = harness_fake_processes();
    assert!(before >= 1, "no harness-fake process is running");
    running.task.abort();
    let _ = running.task.await;
    let deadline = Instant::now() + Duration::from_secs(10);
    while harness_fake_processes() >= before {
        assert!(
            Instant::now() < deadline,
            "the harness process outlived the run that started it"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// How many `harness-fake` processes this machine is running now.
fn harness_fake_processes() -> usize {
    let (program, args): (&str, &[&str]) = if cfg!(windows) {
        (
            "tasklist",
            &["/FI", "IMAGENAME eq harness-fake.exe", "/NH", "/FO", "CSV"],
        )
    } else {
        ("pgrep", &["-c", "-x", "harness-fake"])
    };
    let out = std::process::Command::new(program)
        .args(args)
        .output()
        .expect("the process lister runs");
    let text = String::from_utf8_lossy(&out.stdout);
    if cfg!(windows) {
        text.lines()
            .filter(|l| l.contains("harness-fake.exe"))
            .count()
    } else {
        text.trim().parse().unwrap_or(0)
    }
}

#[tokio::test]
async fn probing_the_default_registry_finds_the_built_scripted_harness_healthy() {
    let registry = Registry::new(vec![fake(&[])]);
    registry.probe(Duration::from_secs(10)).await;
    match registry.state(FAKE_HARNESS) {
        Some(ProbeState::Healthy {
            harness,
            capabilities,
        }) => {
            assert_eq!(harness.name, "harness-fake");
            assert!(capabilities.resume);
        }
        other => panic!("{other:?}"),
    }
    // Probing started a process per entry and left none behind.
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(registry.connector(FAKE_HARNESS).is_some());
}
````

### Implementation notes

**The shape of `run`.** One `tokio::select!` loop, as the spec's section 4 requires, whose arms
are: the next line from the harness; the next line of its standard error; a command; the stall
timer; the wall-clock timer; the wait for a slot; and at most one call in flight (the exit
wait, `verify`, `open_pr`, a `pr_state` poll) held as a pinned, boxed future that `Abandon`
can drop. Keep three pieces of state beside the phase: the harness state (`None`, `Live`,
`Finished`), whether a stop is in progress and which transition it ends with, and whether a
gate is pending. Every phase change goes through one function that calls
`fleet_core::transition`, emits `PhaseChanged` through the sink, and treats `None` as a
violation.

**Reading lines without losing or hoarding bytes.** Do not put `read_line` in a `select!` arm:
a read dropped half way loses what it had read. Give each connection a reader task that owns
the stream and sends parsed lines down a bounded channel (capacity 64); the loop's arm is
`recv()`, which is safe to drop. The task reads with the same bound the protocol crate's
blocking codec uses, so a line with no end costs at most one line of memory (compiled and
run in the scratch workspace against a line one byte past the limit):

````rust
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt};

/// One line from the harness, or why there is none.
enum Line {
    Message(harness_protocol::RpcMessage),
    /// Not a JSON-RPC message; the text is what was read.
    Garbage(String),
    TooLong,
    Eof,
}

async fn next_line<R>(reader: &mut R) -> std::io::Result<Line>
where
    R: AsyncBufRead + Unpin,
{
    let limit = harness_protocol::MAX_LINE_BYTES;
    loop {
        let mut buf = Vec::new();
        // One byte past the limit is enough to tell "at the limit" from "too long".
        let read = (&mut *reader)
            .take(limit as u64 + 1)
            .read_until(b'\n', &mut buf)
            .await?;
        if read == 0 {
            return Ok(Line::Eof);
        }
        if buf.last() != Some(&b'\n') && buf.len() > limit {
            return Ok(Line::TooLong);
        }
        let text = String::from_utf8_lossy(&buf).trim().to_string();
        if text.is_empty() {
            continue;
        }
        return Ok(match serde_json::from_str(&text) {
            Ok(message) => Line::Message(message),
            Err(_) => Line::Garbage(text),
        });
    }
}
````

A bounded channel also means a harness that floods is slowed by its own pipe, not buffered
without limit. Hand `Line::Message` and `Line::Garbage` to `ProtocolMonitor::on_line` as
`Ok(message)` and `Err(text)`; `TooLong` is a violation of its own. Truncate the quoted line to
2048 bytes at a character boundary before it goes into an error.

**Writing.** One message per line, flushed. A write error means the harness is gone: treat it
as end of output and let the exit path report it. Closing the input is `shutdown()` on the
writer followed by dropping it.

**The clocks.** Use `tokio::time::Instant` and `tokio::time::sleep_until`, never
`std::time`, or the paused-clock tests cannot move time. Agent time is a sum of intervals:
start one at the start acknowledgement and when a gate is answered; end one when a gate
becomes pending, at the result, and when a stop begins. The wall-clock timer fires when the
stored milliseconds plus the sum exceed the cap. The stall timer is re-armed by every
`unit/event` and runs under the same conditions. A cap of 0 seconds arms no wall-clock timer.

**The slot.** `Semaphore::acquire_owned` on `ctx.slots`, as a `select!` arm so that `Halt` and
`Abandon` can drop the wait. Emit `Blocked{REASON_AWAITING_SLOT, None, ""}` only when
`try_acquire_owned` fails first. Drop the permit: when a stop sequence ends, after the exit
wait that follows a result, and when a gate becomes pending.

**The wording.** The tests assert the reasons below literally and each error by a fragment
of the text below (the command id, the cap's name, the check's name, the quoted line). Use
the full text given here: the store folds on two of the reasons and the dashboard shows the
rest.

| Where | Text |
|---|---|
| `Blocked` reasons, with cap and detail | `awaiting concurrency slot` (no cap, empty detail); `usd cap` / `usd` / `spent $5.25 of $5.00` (two decimals); `wall-clock cap` / `wall_clock` / `ran <n>s of <cap>s`; `stalled` / none / `no event for 600s`; `awaiting ship` / none / `T3: a human ships the PR`; `verification stopped` / none / the verdict's detail, with the failing ids for `TestsFailed`; the stop reason's wire name / none / the stop's detail; `mergeability poll timed out` / none / `after 10 polls`. For tampering, the `Blocked` detail is the verdict's detail |
| `PhaseChanged` reasons | The breach's `Blocked` reason for a breach or a stall; `REASON_ORACLE_TAMPERING`; `REASON_AWAITING_SHIP`; `REASON_VERIFY_STOPPED`; the stop reason's wire name; `abandoned`; `base moved`; `oracle already frozen` and `approved in a prior run` for the two transitions a resume synthesises |
| Errors, scope `harness` | `harness <connector name> refused: <eligibility reason>`; one containing `initialize` when it is not answered in time, and the refused version when it is; one containing `unit/start`; `no provisioned within 600s`; `protocol violation: <kind>: <the line, cut to 2 KB>`; `exited without result (<the exit's description>)`; one containing `did not exit` when a harness lingers after its result; the connector's own message when a harness cannot be started |
| Errors, scope `system` | `command <id> not valid in <phase's wire name>`; `halt not available now; abandon instead`; one containing `edited oracle`; one containing the cap's name (`usd` or `wall`) and `exhausted`; one containing `resume` when a harness does not declare it; one containing `oracle tampering` when such a unit is resumed; `no result is held`; `evidence rejected: <check as Debug>: <detail>`; `pr_open verified as empty`; `no_change verified as <the verdict as Debug>`; `command channel closed`; the forge's message when the source bundle cannot be made |
| Errors, scope `github` | the forge's message when a pull request cannot be opened or a poll fails |

**Two checks before the verifier is called.** Evidence whose `branch` is not the work
order's branch, and evidence whose delivery is `push`, fail the unit with
`evidence rejected: Link: <which of the two>` and the verifier is not called: the first is a
result for another unit, the second a delivery no milestone-1 harness may declare.

**The pull request.** Title: `<work item reference>: <task>`. Body: for a work item of kind
`issue`, `fleet_forge::pr_body(&WorkItemRef::parse(reference, fingerprint)?, &repo.slug, &task)`;
for any other kind the task alone, and `pr_body: None` to the verifier. Build it once, give it
to the verifier, and send exactly that string in `PrRequest::body`. `RepoRef` is built from
`UnitSpec::repo`. Mergeability: up to `MAX_MERGEABLE_POLLS` calls of `pr_state` with no delay
between them; `Mergeable` applies `PrMergeable`, `Dirty` applies `PrDirty` with the reason
`base moved`; an error counts as a poll; when they run out, `Blocked` then `PrDirty`.

**`ProcessConnector::connect`.** Start `entry.command[0]` with the rest as arguments and
`entry.env` added to the daemon's environment; no shell. Pipe all three streams. Set
`kill_on_drop(true)`. Start one task that reads standard error line by line into the
`diagnostics` channel (lossy UTF-8, each line cut to 4 KB), independently of standard output,
so that neither pipe can fill while the other is being waited on. Put the child in a process
tree that can be ended as a whole; this module compiles and was run on the Windows build host,
where it ended a grandchild process. The Unix half was not compiled there: if the locked
`tokio` has no `Command::process_group`, set the group through `as_std_mut()` and
`std::os::unix::process::CommandExt::process_group`, and say so in the pull request:

````rust
//! Ending a child's whole process tree. On Windows a child is put in a Job Object that kills
//! every process in it when its last handle closes; elsewhere the child leads its own process
//! group and the group is signalled.

#[cfg(windows)]
mod imp {
    use std::io;
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, TerminateJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };

    /// Owns one Job Object. Dropping it closes the handle, which ends every process in the job.
    pub struct ProcessTree(HANDLE);

    // The handle is only ever used through the three calls below, each of which is thread-safe.
    unsafe impl Send for ProcessTree {}
    unsafe impl Sync for ProcessTree {}

    impl ProcessTree {
        /// Put `child` in a new job. Call it straight after `spawn`, before the child has had
        /// time to start children of its own.
        pub fn adopt(child: &tokio::process::Child) -> io::Result<Self> {
            let process = child
                .raw_handle()
                .ok_or_else(|| io::Error::other("the child has already exited"))?;
            // SAFETY: plain FFI calls; every handle is checked before it is used, and the job
            // handle is closed exactly once, in Drop.
            unsafe {
                let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
                if job.is_null() {
                    return Err(io::Error::last_os_error());
                }
                let tree = ProcessTree(job);
                let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
                limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                let set = SetInformationJobObject(
                    job,
                    JobObjectExtendedLimitInformation,
                    (&raw const limits).cast(),
                    std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                );
                if set == 0 {
                    return Err(io::Error::last_os_error());
                }
                if AssignProcessToJobObject(job, process as HANDLE) == 0 {
                    return Err(io::Error::last_os_error());
                }
                Ok(tree)
            }
        }

        /// End every process in the job now.
        pub fn kill(&self) {
            // SAFETY: the handle is valid until Drop.
            unsafe {
                TerminateJobObject(self.0, 1);
            }
        }
    }

    impl Drop for ProcessTree {
        fn drop(&mut self) {
            // SAFETY: closes the handle this value owns, once.
            unsafe {
                CloseHandle(self.0);
            }
        }
    }

    /// Nothing to set before the spawn on Windows.
    pub fn prepare(_command: &mut tokio::process::Command) {}
}

#[cfg(unix)]
mod imp {
    use std::io;

    /// The child's process group id.
    pub struct ProcessTree(i32);

    impl ProcessTree {
        pub fn adopt(child: &tokio::process::Child) -> io::Result<Self> {
            let pid = child
                .id()
                .ok_or_else(|| io::Error::other("the child has already exited"))?;
            Ok(ProcessTree(pid as i32))
        }

        /// Signal the whole group. `kill` is run as a program so that this crate needs no
        /// binding to the C library.
        pub fn kill(&self) {
            let _ = std::process::Command::new("kill")
                .args(["-KILL", "--", &format!("-{}", self.0)])
                .status();
        }
    }

    impl Drop for ProcessTree {
        fn drop(&mut self) {
            self.kill();
        }
    }

    /// Make the child the leader of a new process group, so the group can be signalled.
    pub fn prepare(command: &mut tokio::process::Command) {
        command.process_group(0);
    }
}

pub use imp::{prepare, ProcessTree};
````

`ProcessControl::kill` calls `ProcessTree::kill` and then `Child::kill`; `wait` is
`Child::wait` mapped to `Exit` (`exit code: <n>`, or `killed` or the signal when there is no
code). The `ProcessTree` lives in the `ProcessControl`, so dropping the connection ends the
tree. On Windows a process already inside a job that forbids nesting cannot be assigned to
another: if `adopt` fails, log it as a diagnostic line and continue with `kill_on_drop` alone.

**Reconciling spike S5** (assumption A1). If S5 found that a Job Object with kill-on-close
ends the whole tree, nothing changes. If it found a case where it does not (a process that
left the job, or a harness started with breakaway rights), report it in the pull request with
the case; the fallback is what the code above already does when `adopt` fails, and the
containers such a harness leaves are reaped by label by `fleet-runner`.

**The registry.** `fake_binary_beside`: the executable's directory, or that directory's parent
when its file name is exactly `deps`; append `harness-fake` plus
`std::env::consts::EXE_SUFFIX`. `from_toml`: strip a leading byte-order mark; deserialise with
`deny_unknown_fields` into `harness: Vec<{name, command, env: BTreeMap}>` (so `env` comes out
sorted by key); then check duplicates and empty commands. An empty file is an empty registry.
`probe`: for each entry, `ProcessConnector::connect`, send `initialize`, wait up to `grace` for
the reply, check the version with `harness_protocol::negotiate`, close the input, wait briefly
and kill. A spawn error whose kind is `NotFound` for the entry named `fake` gets the hint
`run: cargo build -p harness-conformance --bins` appended. Probe the entries one after another;
a failure of one never stops the next.

**Pitfalls.** `Command::to_trigger()` returns `None` for `Reverify`: build
`Trigger::Reverify { from }` from the phase the unit was paused in (kept in memory, or
`ctx.pause` after a restart). The per-spawn counters (round count, previous blockers, the
recorded `empty_diff`, the build, check and review numbers) reset at every spawn. A `metric`'s
figures are increments; the event the sink gets is the running total. Emit the spawn-end
metric on every path a spawn ends by. `needs_human` and `failed` results are exempt from the
monitor's metering rule; do not add a second check.

### Verify

| Command | Expected |
|---|---|
| `cargo test -p fleet-supervisor --lib` | 4 passed (Part 1's five, less the one this lane deletes), plus any unit tests this lane adds |
| `cargo test -p fleet-supervisor --test life --test gate_and_caps --test commands --test verdicts --test registry` | 31, 22, 24, 24 and 14 passed |
| `cargo build -p harness-conformance --bins`, then `cargo test -p fleet-supervisor --test process_it` | 7 passed |
| the two test commands above, run 20 times in a loop | no failure: nothing here may depend on real time except `process_it`, whose deadlines are generous |
| `cargo xtask test static --root-only`, `unit`, `contract`, `integration` | exit 0 |
| *Git-Bash:* `grep -rn "unimplemented!" crates/fleet-supervisor/src` | no output |
| *Git-Bash:* `grep -rn "std::time::Instant\|std::thread::sleep" crates/fleet-supervisor/src --include=*.rs \| grep -v testkit.rs` | no output |
| `git diff --name-only origin/factory/m1...HEAD` | only paths under **Owns** |

### Done when

- [ ] Every test of the six files passes on the Windows build host, and in the pull request's
      checks on Linux.
- [ ] No `unimplemented!` is left in `crates/fleet-supervisor/src`.
- [ ] The pull request says how assumption A1 was reconciled, lists the thirteen departures as
      the amendment the supervisor spec needs, and names the registry rows it wants.
- [ ] Issue #84 is named as closed by this pull request.

---

## Lane CC-API

**Owns:** `crates/fleet-api/src/auth.rs`, `hub.rs`, `routes.rs` (the bodies),
`crates/fleet-api/tests/**`, and the committed schema
`crates/fleet-api/contract/fleet-api.schema.json`, which this lane creates. It may add private
modules. It does not change `dto.rs`, `ports.rs`, `schema.rs`, `testkit.rs` or `lib.rs`: a type
on the wire is an interface change, and the cockpit's plan is written from Part 1.

**Reads:** `crates/fleetd/src/server.rs` lines 153 to 201 (the origin allowlist, CORS and the
route table), 348 to 403 (`create_mission`), 509 to 546 (`spawn_forwarder`), 549 to 715 (the
read routes, `post_command`, the stream), and the three CORS tests at the end of its `mod tests`
(`an_unlisted_origin_gets_no_allow_origin_header` is one); `crates/fleetd/tests/characterisation_missions.rs`
(the status codes and bodies that must not change); `forms/README.md` (what a Form's header
looks like); `forms/fleet-api.md`; issue #91.

**Worktree and branch:** `D:\MajorProjects\.swarm-wt\m1-cc-api`, branch `feat/m1-fleet-api`.

**Needs:** Part 1 merged. Nothing from the other lanes: every test runs on `MemStore`,
`MemForge`, `FakeAdmission`, `FakeLauncher` and `FakeDirectory`.

**Blocks:** CC-WIRE; and the cockpit's plan, which generates its client from the schema this
lane commits.

### Form draft: `fleet-api`

**Purpose.** The daemon's public surface: commands in over HTTP, events out over a WebSocket,
and a schema of every type on that surface. It turns requests into calls on the seams of the
other crates and decides nothing itself; without it every client would be written by hand
against routes nobody had written down.

**interface-files:** `crates/fleet-api/src/lib.rs`, `auth.rs`, `dto.rs`, `ports.rs`, `hub.rs`,
`routes.rs`, `schema.rs`, `crates/fleet-api/contract/fleet-api.schema.json`.

**Interfaces** (the signatures of Part 1, Task 8; `dto.rs` is listed there in full):

````rust
// crates/fleet-api/src/auth.rs
pub const TOKEN_ENV: &str = "FLEETD_TOKEN";

pub const WS_PROTOCOL: &str = "fleetd.v1";

pub const WS_TOKEN_PREFIX: &str = "fleetd.token.";

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AuthError {
    #[error("{TOKEN_ENV} is not set: the daemon serves nothing without a token")]
    Missing,
    #[error("the token is not usable: {0}")]
    Weak(&'static str),
    #[error("{address} is not a loopback address: the daemon binds to loopback only")]
    NotLoopback { address: String },
    #[error("{0:?} is not a socket address")]
    BadAddress(String),
}

#[derive(Clone)]
pub struct Token(#[allow(dead_code)] String);

impl Token {
    pub fn new(value: &str) -> Result<Self, AuthError>;

    pub fn from_env() -> Result<Self, AuthError>;

    pub fn matches(&self, presented: &str) -> bool;
}

pub fn secured(inner: Router, token: Token, allowed_origins: &[String]) -> Router;

pub fn loopback_address(text: &str) -> Result<SocketAddr, AuthError>;

// crates/fleet-api/src/ports.rs
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LaunchError {
    #[error("unit {unit_id} is already running")]
    AlreadyRunning { unit_id: String },
    #[error("no harness is registered as {harness:?}")]
    UnknownHarness { harness: String },
    #[error("the unit could not be started: {0}")]
    Failed(String),
}

pub trait Launcher: Send + Sync {
    fn launch(&self, admitted: &Admitted) -> Result<(), LaunchError>;

    fn rehydrate(&self, unit_id: &str) -> bool;

    fn legacy_mission(&self, request: LegacyMissionRequest) -> Result<String, (u16, String)>;
}

pub trait HarnessDirectory: Send + Sync {
    fn list(&self) -> Vec<HarnessView>;
}

pub trait Clock: Send + Sync {
    fn now_ms(&self) -> i64;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

// crates/fleet-api/src/hub.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoggedEvent {
    pub unit_id: String,
    pub seq: u64,
    pub json: Arc<str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delivery {
    Accepted,
    Gone,
    Unknown,
}

pub struct UnitChannels {
    pub commands: UnboundedReceiver<Command>,
    pub feed: UnitFeed,
}

#[derive(Clone)]
pub struct UnitFeed {
    /* private */
}

pub(crate) enum Record {
    Event(Event),
    Stage {
        stage: Stage,
        status: StageStatus,
        detail: Option<String>,
    },
    Frozen(OracleFreeze),
    Result(UnitResult),
}

impl UnitFeed {
    pub fn event(&self, event: Event);

    pub fn stage(&self, stage: Stage, status: StageStatus, detail: Option<String>);

    pub fn frozen(&self, freeze: &OracleFreeze);

    pub fn result(&self, result: &UnitResult);
}

#[derive(Clone)]
pub struct Hub {
    /* private */
}

impl Hub {
    pub fn new(units: Arc<dyn UnitStore>, clock: Arc<dyn Clock>) -> Self;

    pub fn register(&self, unit_id: &str) -> Option<UnitChannels>;

    pub fn send(&self, unit_id: &str, command: Command) -> Delivery;

    pub fn subscribe(&self, unit_id: &str) -> Option<broadcast::Receiver<LoggedEvent>>;

    pub fn live(&self) -> Vec<String>;

    pub fn knows(&self, unit_id: &str) -> bool;
}

// crates/fleet-api/src/routes.rs
pub const ROUTES: &[(&str, &str)] = &[
    ("POST", "/missions"),
    ("GET", "/units"),
    ("GET", "/details"),
    ("GET", "/units/:id"),
    ("GET", "/units/:id/detail"),
    ("GET", "/units/:id/events"),
    ("POST", "/units/:id/commands"),
    ("GET", "/units/:id/stream"),
    ("GET", "/harnesses"),
    ("GET", "/specs"),
    ("POST", "/signatures"),
    ("GET", "/signatures"),
    ("GET", "/schema"),
];

#[derive(Clone)]
pub struct ApiState {
    pub units: Arc<dyn UnitStore>,
    pub signatures: Arc<dyn SignatureStore>,
    pub forge: Arc<dyn RepoForge>,
    pub admission: Arc<dyn Admit>,
    pub launcher: Arc<dyn Launcher>,
    pub harnesses: Arc<dyn HarnessDirectory>,
    pub hub: Hub,
    pub repos: Vec<RepoRef>,
    pub operator: String,
    pub clock: Arc<dyn Clock>,
}

pub fn router(state: ApiState) -> Router;

pub fn refusal_status(refusal: &Refusal) -> u16;

pub fn forms_index(forms: &[(String, String)]) -> FormsIndex;

// crates/fleet-api/src/schema.rs
#[derive(JsonSchema)]
pub struct ApiSchema {
    pub post_missions_request: MissionBody,
    pub post_missions_response: MissionResponse,
    pub get_units_response: Vec<UnitSummary>,
    pub get_details_response: Vec<UnitDetail>,
    pub get_unit_response: Snapshot,
    pub get_unit_detail_response: UnitDetail,
    pub get_unit_events_response: Vec<EventEnvelopeDto>,
    pub post_unit_commands_request: CommandDto,
    pub unit_stream_frame: EventEnvelopeDto,
    pub get_harnesses_response: Vec<HarnessView>,
    pub get_specs_response: SpecView,
    pub post_signatures_request: SignRequest,
    pub post_signatures_response: SignResponse,
    pub get_signatures_response: Vec<SignatureView>,
    pub error: ApiError,
}

pub fn schema_json() -> String;
````

**The routes**

| Route | Request | Answer |
|---|---|---|
| `POST /missions` | `MissionBody`: a `MissionRequest`, or the deprecated `LegacyMissionRequest` | 200 `MissionResponse`; a refusal as `ApiError` with the status of `refusal_status`; 500 `launch_failed`. The deprecated shape answers exactly as before: 200 `{"unit_id":…}`, or the status and plain text the launcher returns |
| `GET /units` | none | 200 `Vec<UnitSummary>` |
| `GET /details` | none | 200 `Vec<UnitDetail>` |
| `GET /units/:id` | none | 200 `Snapshot`; 404 with an empty body |
| `GET /units/:id/detail` | none | 200 `UnitDetail`; 404 with an empty body |
| `GET /units/:id/events` | `?since=<seq>`, default 0 | 200 `Vec<EventEnvelopeDto>`, empty for a unit nobody knows |
| `POST /units/:id/commands` | `CommandDto` | 202; 404 for a unit that is neither running nor stored; 410 for a unit whose run has ended; 422 for a body that is not a command |
| `GET /units/:id/stream` | `?since=<seq>`; a WebSocket upgrade offering `fleetd.v1` and `fleetd.token.<token>` | one `EventEnvelopeDto` per text frame: the log after `since`, then what follows |
| `GET /harnesses` | none | 200 `Vec<HarnessView>` |
| `GET /specs` | `?repo=<owner/name>&spec_id=<id>&commit=<sha>`, `commit` optional | 200 `SpecView`; 400 `bad_spec_id`; 403 `repo_not_allowed`; 404 `spec_not_found`; 502 `forge` |
| `POST /signatures` | `SignRequest` | 200 `SignResponse`; 400 `bad_spec_id`; 403 `repo_not_allowed`; 404 `spec_not_found`; 409 `hash_mismatch`; 422 `spec_unreadable`, `spec_id_mismatch`, `spec_not_ready` (with `problems`); 502 `forge`; 503 `store` |
| `GET /signatures` | `?repo=<owner/name>` | 200 `Vec<SignatureView>`; 403 `repo_not_allowed`; 503 `store` |
| `GET /schema` | none | 200, the text of `schema_json()` |

Every route answers 401 `unauthorized` without the token, and every read route answers 503
`store` when the store cannot be read. `/health` and `/swarms` are not this crate's: they stay
in `fleetd`, behind the same `secured` wrapper.

**Invariants**

- I1. Every route of `ROUTES` answers 401 unless the request carries the token, and a 401
  never contains the token.
- I2. The token is never printed, never compared with a comparison that stops at the first
  difference, and read from the environment only.
- I3. The daemon's address is a loopback address or the daemon does not start.
- I4. A unit is started only by `Admit::admit` followed by `Launcher::launch`; a request
  cannot name a tier.
- I5. A signature is stored only for bytes the daemon itself read at the named commit, whose
  hash is the hash the request carried, which parse as a spec with the requested id and have no
  readiness problem.
- I6. A spec id reaches the forge only as a file name of letters, digits, `-`, `_` and `.`
  that does not start with a dot.
- I7. Each record a run reports gets the next sequence number, is written to the store and is
  then broadcast, in that order; a write that fails loses that record from the log and nothing
  else.
- I8. A stream sends each record once: the stored log after `since`, then the live records
  with a higher number.
- I9. A read route never answers with an empty list because the store could not be read.
- I10. `GET /units` and `GET /units/:id` keep exactly the keys they had before milestone 1.
- I11. The schema the crate serves is the schema committed beside it.
- I12. The crate depends on no workspace crate other than `fleet-core`, `harness-protocol`,
  `factory-spec`, `fleet-store`, `fleet-forge` and `fleet-admission`, and declares a `testkit`
  feature.

**Hidden decisions.** How the table of live units is locked; the size of the broadcast
buffer; the wording of every `message`; how a Form's header is scanned.

**Gates**

| id | guards | mechanism | location | command | blocks |
|---|---|---|---|---|---|
| workspace.G1 | I12 | Build graph: the dependency allowlist and the `testkit` feature | `xtask/src/deps.rs#pub const RULES` | `cargo xtask test static` | merge |

**Unenforced (planned)**

| planned id | guards | mechanism | location | command | built by |
|---|---|---|---|---|---|
| fleet-api.G1 | I1, I2, I3 | Locked tests of the token and the address | `crates/fleet-api/tests/contract.rs#every_route_answers_401_without_the_token_and_with_a_wrong_one` | `cargo xtask test contract` | behaviours 1 to 6 |
| fleet-api.G2 | I4 | Locked tests of `POST /missions` | `crates/fleet-api/tests/contract.rs#a_request_may_not_name_its_own_tier` | `cargo xtask test contract` | behaviours 7 to 12 |
| fleet-api.G3 | I5, I6 | Locked tests of signing | `crates/fleet-api/tests/contract.rs#the_file_is_read_again_at_signing_and_a_hash_that_no_longer_matches_is_refused` | `cargo xtask test contract` | behaviours 23 to 30 |
| fleet-api.G4 | I7, I8 | Locked tests of the hub and the stream | `crates/fleet-api/tests/contract.rs#the_hub_numbers_stores_and_broadcasts_each_record_in_order` | `cargo xtask test contract` | behaviours 18 to 21 |
| fleet-api.G5 | I11 | Schema snapshot | `crates/fleet-api/tests/contract.rs#the_schema_route_serves_the_committed_contract` | `cargo xtask test contract` | behaviour 32 |

I9 and I10 are checked by behaviours 13 to 15 and have no gate of their own. The parity of the
mirror types with `fleet-core` is checked by the four unit tests of `schema.rs` that Part 1
delivers.

### Behaviours

All in `crates/fleet-api/tests/contract.rs`, each starting from
`fleet_api::testkit::TestApi::new`.

The token, the address and the origin:

1. `every_route_answers_401_without_the_token_and_with_a_wrong_one`
2. `every_route_is_reached_with_the_token_as_a_bearer_header`
3. `a_websocket_upgrade_carries_the_token_as_a_subprotocol`
4. `a_token_must_be_long_and_plain_and_never_prints_itself`
5. `the_daemon_binds_to_loopback_and_to_nothing_else`
6. `a_preflight_request_needs_no_token_and_only_an_allowed_origin_is_admitted`

Starting a unit:

7. `a_mission_from_a_signed_spec_is_admitted_then_started`
8. `a_repeated_key_is_409_and_starts_nothing`
9. `a_request_may_not_name_its_own_tier`
10. `each_refusal_is_answered_with_its_status_and_its_code`
11. `a_unit_that_was_admitted_and_could_not_be_started_is_a_500`
12. `the_deprecated_request_shape_goes_to_the_launcher_and_answers_as_it_always_did`

Reading units:

13. `the_two_original_read_routes_keep_exactly_their_keys`
14. `the_detail_routes_show_what_the_factory_path_stores`
15. `a_store_that_cannot_be_read_is_a_503_not_an_empty_list`

Commands, the hub and the stream:

16. `a_command_reaches_the_units_run_and_reverify_is_one_of_them`
17. `a_command_for_a_stored_unit_with_no_run_brings_it_back_first`
18. `the_hub_numbers_stores_and_broadcasts_each_record_in_order`
19. `the_freeze_and_the_result_are_kept_in_the_row_and_not_logged`
20. `a_record_the_store_refuses_is_still_broadcast_and_the_next_one_is_stored`
21. `the_stream_replays_the_log_then_follows_it_and_sends_no_record_twice`

Harnesses:

22. `the_harness_list_is_the_registrys_stored_state`

Reading and signing a spec:

23. `a_spec_is_shown_exactly_as_it_was_read_at_a_named_commit`
24. `signing_stores_who_when_the_hash_and_the_bytes_that_were_read`
25. `the_file_is_read_again_at_signing_and_a_hash_that_no_longer_matches_is_refused`
26. `a_spec_that_is_not_ready_is_not_signed_and_the_answer_says_why`
27. `a_spec_that_touches_a_forms_interface_without_naming_the_form_is_not_signed`
28. `bytes_that_are_not_a_spec_or_a_file_that_is_not_there_cannot_be_signed`
29. `a_spec_id_is_never_used_as_a_path`
30. `a_repository_that_is_not_allowed_or_cannot_be_read_is_refused`

Forms and the schema:

31. `the_forms_index_reads_interface_files_from_form_headers_and_ignores_other_files`
32. `the_schema_route_serves_the_committed_contract`

Run: `cargo test -p fleet-api --test contract`. Expected before the implementation: 32 failed,
each with `not implemented: lane CC-API builds this`.

`crates/fleet-api/tests/contract.rs`:

````rust
//! Lane CC-API: the routes, the token, signing, the hub and the schema, over fakes.
//!
//! No Docker, no git, no network beyond a loopback socket for the WebSocket test.

use axum::http::StatusCode;
use factory_spec::testkit::SpecBuilder;
use fleet_admission::Refusal;
use fleet_api::testkit::{call, FakeDirectory, TestApi, SPEC_PATH, TOKEN};
use fleet_api::{
    forms_index, loopback_address, refusal_status, schema_json, secured, ApiError, AuthError,
    HarnessView, LaunchError, Token, ROUTES, WS_PROTOCOL, WS_TOKEN_PREFIX,
};
use fleet_core::{Command, Event, Phase};
use fleet_store::testkit::{harness_record, phase_changed, record};
use fleet_store::{SignatureStore, UnitStore};
use harness_protocol::{Stage, StageStatus};
use serde_json::{json, Value};
use std::sync::Arc;

fn parsed(body: &str) -> Value {
    serde_json::from_str(body).unwrap_or_else(|e| panic!("{body:?} is not JSON: {e}"))
}

fn error(body: &str) -> ApiError {
    serde_json::from_str(body).unwrap_or_else(|e| panic!("{body:?} is not an ApiError: {e}"))
}

fn keys(v: &Value) -> Vec<&str> {
    let mut keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
    keys.sort_unstable();
    keys
}

fn concrete(path: &str) -> String {
    path.replace(":id", "u1")
}

// --- the token ---

#[tokio::test]
async fn every_route_answers_401_without_the_token_and_with_a_wrong_one() {
    let api = TestApi::new();
    let secured = secured(api.router(), Token::new(TOKEN).unwrap(), &[]);
    let wrong = format!("Bearer {}", TOKEN.replace("test", "tset"));
    for (method, path) in ROUTES {
        let uri = concrete(path);
        for headers in [
            vec![],
            vec![("authorization", wrong.as_str())],
            vec![("authorization", TOKEN)],
        ] {
            let (status, body) = call(secured.clone(), method, &uri, None, &headers).await;
            assert_eq!(
                status,
                StatusCode::UNAUTHORIZED,
                "{method} {path} with {headers:?}"
            );
            assert_eq!(error(&body).code, "unauthorized");
            assert!(!body.contains(TOKEN), "a 401 echoed the token");
        }
    }
    assert!(api.admission.requests().is_empty() && api.launcher.launched().is_empty());
}

#[tokio::test]
async fn every_route_is_reached_with_the_token_as_a_bearer_header() {
    let api = TestApi::new();
    let secured = secured(api.router(), Token::new(TOKEN).unwrap(), &[]);
    let bearer = format!("Bearer {TOKEN}");
    for (method, path) in ROUTES {
        let (status, _) = call(
            secured.clone(),
            method,
            &concrete(path),
            None,
            &[("authorization", bearer.as_str())],
        )
        .await;
        assert_ne!(status, StatusCode::UNAUTHORIZED, "{method} {path}");
        assert_ne!(
            status,
            StatusCode::METHOD_NOT_ALLOWED,
            "{method} {path} is not routed"
        );
    }
}

#[tokio::test]
async fn a_websocket_upgrade_carries_the_token_as_a_subprotocol() {
    let api = TestApi::new();
    let secured = secured(api.router(), Token::new(TOKEN).unwrap(), &[]);
    let upgrade = |protocols: String| {
        let secured = secured.clone();
        async move {
            let headers = [
                ("connection", "upgrade"),
                ("upgrade", "websocket"),
                ("sec-websocket-version", "13"),
                ("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ=="),
                ("sec-websocket-protocol", protocols.as_str()),
            ];
            call(secured, "GET", "/units/u1/stream", None, &headers)
                .await
                .0
        }
    };
    assert_eq!(
        upgrade(WS_PROTOCOL.to_string()).await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        upgrade(format!(
            "{WS_PROTOCOL}, {WS_TOKEN_PREFIX}not-the-token-0123456789abcdef0123"
        ))
        .await,
        StatusCode::UNAUTHORIZED
    );
    // With the token the request passes authentication. (It cannot complete an upgrade through
    // a one-shot service, so any status but 401 shows it got past the check.)
    assert_ne!(
        upgrade(format!("{WS_PROTOCOL}, {WS_TOKEN_PREFIX}{TOKEN}")).await,
        StatusCode::UNAUTHORIZED
    );
}

#[test]
fn a_token_must_be_long_and_plain_and_never_prints_itself() {
    assert_eq!(
        Token::new("short").err(),
        Some(AuthError::Weak("shorter than 32 characters"))
    );
    assert!(matches!(
        Token::new(&"a b".repeat(20)),
        Err(AuthError::Weak(_))
    ));
    assert!(matches!(
        Token::new(&"\u{e9}".repeat(40)),
        Err(AuthError::Weak(_))
    ));
    let token = Token::new(TOKEN).unwrap();
    assert_eq!(format!("{token:?}"), "Token(***)");
    assert!(token.matches(TOKEN));
    assert!(!token.matches(&TOKEN[..TOKEN.len() - 1]));
    assert!(!token.matches(&format!("{TOKEN}x")));
    assert!(!token.matches(""));
}

#[test]
fn the_daemon_binds_to_loopback_and_to_nothing_else() {
    assert!(loopback_address("127.0.0.1:8787").is_ok());
    assert!(loopback_address("[::1]:8787").is_ok());
    for outside in ["0.0.0.0:8787", "192.168.1.5:8787", "[::]:8787"] {
        assert_eq!(
            loopback_address(outside),
            Err(AuthError::NotLoopback {
                address: outside.into()
            })
        );
    }
    assert!(matches!(
        loopback_address("localhost:8787"),
        Err(AuthError::BadAddress(_))
    ));
    assert!(matches!(
        loopback_address(""),
        Err(AuthError::BadAddress(_))
    ));
}

#[tokio::test]
async fn a_preflight_request_needs_no_token_and_only_an_allowed_origin_is_admitted() {
    let api = TestApi::new();
    let origins = ["http://tauri.localhost".to_string()];
    let secured = secured(api.router(), Token::new(TOKEN).unwrap(), &origins);
    let preflight = |origin: &'static str| {
        let secured = secured.clone();
        async move {
            let request = axum::http::Request::builder()
                .method("OPTIONS")
                .uri("/missions")
                .header("origin", origin)
                .header("access-control-request-method", "POST")
                .header(
                    "access-control-request-headers",
                    "authorization,content-type",
                )
                .body(axum::body::Body::empty())
                .unwrap();
            tower::ServiceExt::oneshot(secured, request).await.unwrap()
        }
    };
    let allowed = preflight("http://tauri.localhost").await;
    assert_ne!(allowed.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        allowed
            .headers()
            .get("access-control-allow-origin")
            .and_then(|v| v.to_str().ok()),
        Some("http://tauri.localhost")
    );
    let allow_headers = allowed
        .headers()
        .get("access-control-allow-headers")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();
    assert!(allow_headers.contains("authorization"), "{allow_headers}");
    let refused = preflight("https://evil.example").await;
    assert!(refused
        .headers()
        .get("access-control-allow-origin")
        .is_none());
}

// --- POST /missions ---

#[tokio::test]
async fn a_mission_from_a_signed_spec_is_admitted_then_started() {
    let api = TestApi::new();
    let mut body = api.mission("key-1");
    body["profile"] = json!("claude");
    body["opt_in"] = json!(true);
    body["usd_cap"] = json!(2.5);
    let (status, text) = api.post("/missions", body).await;
    assert_eq!(status, StatusCode::OK, "{text}");
    assert_eq!(
        parsed(&text),
        json!({"unit_id": "u1", "waits_for_slot": false})
    );

    let asked = &api.admission.requests()[0];
    assert_eq!(asked.work_item, "example/sandbox#1");
    assert_eq!(asked.repo, "owner/sandbox");
    assert_eq!(asked.spec_sha256, api.spec_sha256);
    assert_eq!(asked.harness, "reqdrive");
    assert_eq!(asked.profile.as_deref(), Some("claude"));
    assert!(asked.opt_in);
    assert_eq!(asked.usd_cap, Some(2.5));
    assert_eq!(asked.idempotency_key, "key-1");
    let launched = api.launcher.launched();
    assert_eq!(launched.len(), 1);
    assert_eq!(launched[0].unit_id, "u1");
}

#[tokio::test]
async fn a_repeated_key_is_409_and_starts_nothing() {
    let api = TestApi::new();
    api.post("/missions", api.mission("key-1")).await;
    let (status, text) = api.post("/missions", api.mission("key-1")).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(error(&text).code, "duplicate_key");
    assert!(error(&text).message.contains("u1"));
    assert_eq!(api.launcher.launched().len(), 1);
}

#[tokio::test]
async fn a_request_may_not_name_its_own_tier() {
    let api = TestApi::new();
    let mut body = api.mission("key-1");
    body["tier"] = json!("t1");
    let (status, _) = api.post("/missions", body).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(
        api.admission.requests().is_empty(),
        "the tier is read from the signed bytes only"
    );
}

#[tokio::test]
async fn each_refusal_is_answered_with_its_status_and_its_code() {
    let cases: Vec<(Refusal, u16)> = vec![
        (Refusal::MissingKey, 400),
        (
            Refusal::WorkItemMalformed {
                reference: "x".into(),
            },
            400,
        ),
        (
            Refusal::DuplicateKey {
                unit_id: "u1".into(),
            },
            409,
        ),
        (Refusal::RepoNotAllowed { repo: "o/r".into() }, 403),
        (
            Refusal::Forge {
                detail: "fetch failed".into(),
            },
            502,
        ),
        (
            Refusal::NotOnboarded {
                repo: "o/r".into(),
                detail: "no file".into(),
            },
            422,
        ),
        (
            Refusal::PresetUnsupported {
                preset: "python".into(),
            },
            422,
        ),
        (
            Refusal::HarnessUnknown {
                harness: "x".into(),
            },
            422,
        ),
        (
            Refusal::HarnessUnhealthy {
                harness: "x".into(),
                error: "gone".into(),
            },
            503,
        ),
        (
            Refusal::PresetNotDeclared {
                preset: "node".into(),
                harness: "x".into(),
            },
            422,
        ),
        (
            Refusal::PresetsVersionMismatch {
                harness: "x".into(),
                ours: "0.1.0".into(),
                theirs: "0.2.0".into(),
            },
            409,
        ),
        (
            Refusal::NotSigned {
                repo: "o/r".into(),
                spec_sha256: "0".repeat(64),
            },
            403,
        ),
        (
            Refusal::SpecUnreadable {
                detail: "bad".into(),
            },
            500,
        ),
        (
            Refusal::WorkItemMismatch {
                request: "o/r#2".into(),
                spec: "o/r#1".into(),
            },
            409,
        ),
        (
            Refusal::CapAboveCeiling {
                cap: "usd",
                requested: 9.0,
                ceiling: 5.0,
            },
            400,
        ),
        (
            Refusal::SpendCeiling {
                committed: 20.0,
                cap: 20.0,
            },
            429,
        ),
        (
            Refusal::SpendUnknown {
                detail: "read failed".into(),
            },
            503,
        ),
        (
            Refusal::Ineligible {
                harness: "x".into(),
                reason: "no gate".into(),
            },
            422,
        ),
        (
            Refusal::ReservationFailed {
                detail: "insert refused".into(),
            },
            500,
        ),
    ];
    for (refusal, status) in cases {
        assert_eq!(refusal_status(&refusal), status, "{refusal:?}");
        let api = TestApi::new();
        api.admission.refuse_next(refusal.clone());
        let (got, text) = api.post("/missions", api.mission("key-1")).await;
        assert_eq!(got.as_u16(), status, "{refusal:?}");
        let body = error(&text);
        assert_eq!(body.code, refusal.code());
        assert_eq!(body.message, refusal.to_string());
        assert!(
            api.launcher.launched().is_empty(),
            "{refusal:?} started a unit"
        );
    }
}

#[tokio::test]
async fn a_unit_that_was_admitted_and_could_not_be_started_is_a_500() {
    let api = TestApi::new();
    api.launcher.fail_launch(LaunchError::UnknownHarness {
        harness: "reqdrive".into(),
    });
    let (status, text) = api.post("/missions", api.mission("key-1")).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(error(&text).code, "launch_failed");
}

#[tokio::test]
async fn the_deprecated_request_shape_goes_to_the_launcher_and_answers_as_it_always_did() {
    let api = TestApi::new();
    let (status, text) = api
        .post("/missions", json!({"task": "add a sum() helper"}))
        .await;
    assert_eq!(
        (status, text.as_str()),
        (StatusCode::OK, r#"{"unit_id":"u1"}"#)
    );
    let legacy = &api.launcher.legacy_requests()[0];
    assert_eq!(
        (
            legacy.task.as_str(),
            legacy.mode.as_str(),
            legacy.min_review_rounds
        ),
        ("add a sum() helper", "demo", 2)
    );
    assert!(api.admission.requests().is_empty());

    let (status, text) = api
        .post("/missions", json!({"task": "x", "mode": "sideways"}))
        .await;
    assert_eq!(
        (status, text.as_str()),
        (StatusCode::BAD_REQUEST, "unknown mode: sideways")
    );

    for bad in [
        json!({}),
        json!({"task": "x", "tier": "T1"}),
        json!({"task": "x", "min_review_rounds": -1}),
    ] {
        let (status, _) = api.post("/missions", bad.clone()).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{bad}");
    }
}

// --- reading units ---

#[tokio::test]
async fn the_two_original_read_routes_keep_exactly_their_keys() {
    let api = TestApi::new();
    assert_eq!(parsed(&api.get("/units").await.1), json!([]));
    let mut unit = harness_record("u1", "reqdrive");
    unit.row.cost = 0.25;
    api.store.seed(unit, 0);
    let event = phase_changed(Phase::Queued, Phase::Provisioning, None);
    api.store
        .record_event(
            "u1",
            1,
            0,
            &fleet_store::testkit::envelope_json("u1", 1, &event),
            Some(&event),
        )
        .unwrap();

    let list = parsed(&api.get("/units").await.1);
    assert_eq!(
        list,
        json!([{"unit_id": "u1", "phase": "provisioning", "cost": 0.25, "usd_cap": 5.0,
                "tier": "t1", "task": "first task", "last_seq": 1}])
    );
    let snapshot = parsed(&api.get("/units/u1").await.1);
    assert_eq!(
        keys(&snapshot),
        ["cost", "events", "phase", "unit_id", "usd_cap"]
    );
    assert_eq!(snapshot["events"][0]["event"]["type"], "phase_changed");
    assert_eq!(
        parsed(&api.get("/units/u1/events?since=1").await.1),
        json!([])
    );
    assert_eq!(
        parsed(&api.get("/units/u1/events").await.1),
        snapshot["events"]
    );

    let (status, body) = api.get("/units/u404").await;
    assert_eq!((status, body.as_str()), (StatusCode::NOT_FOUND, ""));
    assert_eq!(parsed(&api.get("/units/u404/events").await.1), json!([]));
}

#[tokio::test]
async fn the_detail_routes_show_what_the_factory_path_stores() {
    let api = TestApi::new();
    let mut unit = harness_record("u1", "reqdrive");
    unit.row.phase = "needs_human".into();
    unit.ext.work_item = Some("example/sandbox#1".into());
    unit.ext.spec_id = Some("SPEC-0001".into());
    unit.ext.pause_reason = Some("verification stopped".into());
    unit.ext.paused_from = Some("merge_check".into());
    unit.ext.pr_url = Some("https://fake/pr/1".into());
    unit.ext.elapsed_ms = 9_000;
    api.store.seed(unit, 0);
    api.store.seed(record("u2"), 0);

    let detail = parsed(&api.get("/units/u1/detail").await.1);
    assert_eq!(detail["phase"], "needs_human");
    assert_eq!(detail["harness"], "reqdrive");
    assert_eq!(detail["work_item"], "example/sandbox#1");
    assert_eq!(detail["pause_reason"], "verification stopped");
    assert_eq!(detail["paused_from"], "merge_check");
    assert_eq!(detail["pr_url"], "https://fake/pr/1");
    assert_eq!(detail["elapsed_ms"], 9_000);
    assert_eq!(detail["live"], false);
    let all = parsed(&api.get("/details").await.1);
    assert_eq!(all.as_array().unwrap().len(), 2);
    assert_eq!(
        all[1]["harness"],
        Value::Null,
        "a driver-path unit has no harness"
    );
    assert_eq!(api.get("/units/u404/detail").await.0, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_store_that_cannot_be_read_is_a_503_not_an_empty_list() {
    let api = TestApi::new();
    api.store.fail_reads(true);
    for uri in [
        "/units",
        "/details",
        "/units/u1",
        "/units/u1/detail",
        "/units/u1/events",
    ] {
        let (status, body) = api.get(uri).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{uri}");
        assert_eq!(error(&body).code, "store", "{uri}");
    }
}

// --- commands and the hub ---

#[tokio::test]
async fn a_command_reaches_the_units_run_and_reverify_is_one_of_them() {
    let api = TestApi::new();
    api.post("/missions", api.mission("key-1")).await;
    let mut channels = api
        .launcher
        .take_channels("u1")
        .expect("the launcher registered u1");
    for (body, expected) in [
        (
            json!({"command": "halt", "cmd_id": "c1"}),
            Command::Halt {
                cmd_id: "c1".into(),
            },
        ),
        (
            json!({"command": "reverify", "cmd_id": "c2"}),
            Command::Reverify {
                cmd_id: "c2".into(),
            },
        ),
    ] {
        let (status, _) = api.post("/units/u1/commands", body).await;
        assert_eq!(status, StatusCode::ACCEPTED);
        assert_eq!(channels.commands.recv().await, Some(expected));
    }
    let (status, _) = api
        .post(
            "/units/u1/commands",
            json!({"command": "dance", "cmd_id": "c3"}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    // The run ends: nothing is listening any more.
    drop(channels);
    let (status, _) = api
        .post(
            "/units/u1/commands",
            json!({"command": "halt", "cmd_id": "c4"}),
        )
        .await;
    assert_eq!(status, StatusCode::GONE);
}

#[tokio::test]
async fn a_command_for_a_stored_unit_with_no_run_brings_it_back_first() {
    let api = TestApi::new();
    api.launcher.stored(&["u7"]);
    let (status, _) = api
        .post(
            "/units/u7/commands",
            json!({"command": "resume", "cmd_id": "c1"}),
        )
        .await;
    assert_eq!(status, StatusCode::ACCEPTED);
    assert_eq!(api.launcher.rehydrated(), ["u7"]);
    let mut channels = api.launcher.take_channels("u7").unwrap();
    assert_eq!(
        channels.commands.recv().await,
        Some(Command::Resume {
            cmd_id: "c1".into()
        })
    );

    let (status, _) = api
        .post(
            "/units/u404/commands",
            json!({"command": "halt", "cmd_id": "c1"}),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn the_hub_numbers_stores_and_broadcasts_each_record_in_order() {
    let api = TestApi::new();
    let mut stored = harness_record("u1", "reqdrive");
    stored.row.last_seq = 4;
    stored.row.cost = 1.5;
    api.store.seed(stored, 0);
    let hub = &api.state.hub;
    let channels = hub.register("u1").expect("first registration");
    assert!(hub.register("u1").is_none(), "a unit was registered twice");
    assert_eq!(hub.live(), ["u1"]);
    let mut stream = hub.subscribe("u1").unwrap();

    channels
        .feed
        .event(phase_changed(Phase::Halted, Phase::Provisioning, None));
    channels
        .feed
        .stage(Stage::Red, StageStatus::Started, Some("baseline".into()));
    channels.feed.event(Event::Error {
        scope: fleet_core::ErrorScope::Harness,
        retryable: false,
        detail: "exited without result".into(),
    });
    let mut seen = Vec::new();
    for _ in 0..3 {
        seen.push(stream.recv().await.unwrap());
    }
    assert_eq!(
        seen.iter().map(|e| e.seq).collect::<Vec<_>>(),
        [5, 6, 7],
        "numbering continues the log"
    );
    let logged = api.store.events_since("u1", 4).unwrap();
    assert_eq!(
        logged,
        seen.iter().map(|e| e.json.to_string()).collect::<Vec<_>>()
    );
    assert_eq!(
        parsed(&logged[1]),
        json!({"unit_id": "u1", "seq": 6,
               "event": {"type": "stage", "stage": "red", "status": "started", "detail": "baseline"}})
    );
    // The row is the fold: the phase moved, and an event with no metric did not zero the cost.
    let row = api.store.get_unit("u1").unwrap().unwrap();
    assert_eq!(
        (row.phase.as_str(), row.last_seq, row.cost),
        ("provisioning", 7, 1.5)
    );
}

#[tokio::test]
async fn the_freeze_and_the_result_are_kept_in_the_row_and_not_logged() {
    let api = TestApi::new();
    api.store.seed(harness_record("u1", "reqdrive"), 0);
    let channels = api.state.hub.register("u1").unwrap();
    let freeze = harness_protocol::OracleFreeze {
        frozen_files: Vec::new(),
        frozen_ids: vec!["a > b".into()],
        holdout_bundle_path: None,
        holdout_hash: None,
        holdout_ids: Vec::new(),
    };
    channels.feed.frozen(&freeze);
    channels.feed.event(Event::Done {
        result: "failed".into(),
    });
    let mut stream = api.state.hub.subscribe("u1").unwrap();
    channels.feed.event(Event::Log {
        stream: fleet_core::LogStream::System,
        line: "after".into(),
    });
    stream.recv().await.unwrap();
    let stored = api.store.get_record("u1").unwrap().unwrap();
    let kept: harness_protocol::OracleFreeze =
        serde_json::from_str(stored.ext.freeze_json.as_deref().unwrap()).unwrap();
    assert_eq!(kept, freeze);
    assert_eq!(
        api.store.events_since("u1", 0).unwrap().len(),
        2,
        "the freeze was logged as an event"
    );
}

#[tokio::test]
async fn a_record_the_store_refuses_is_still_broadcast_and_the_next_one_is_stored() {
    let api = TestApi::new();
    api.store.seed(harness_record("u1", "reqdrive"), 0);
    let channels = api.state.hub.register("u1").unwrap();
    let mut stream = api.state.hub.subscribe("u1").unwrap();
    api.store.fail_writes(true);
    channels.feed.event(Event::Log {
        stream: fleet_core::LogStream::System,
        line: "lost".into(),
    });
    assert_eq!(stream.recv().await.unwrap().seq, 1);
    api.store.fail_writes(false);
    channels.feed.event(Event::Log {
        stream: fleet_core::LogStream::System,
        line: "kept".into(),
    });
    assert_eq!(stream.recv().await.unwrap().seq, 2);
    let logged = api.store.events_since("u1", 0).unwrap();
    assert_eq!(logged.len(), 1);
    assert!(logged[0].contains("kept"));
}

#[tokio::test]
async fn the_stream_replays_the_log_then_follows_it_and_sends_no_record_twice() {
    use futures_util::StreamExt;
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;

    let api = TestApi::new();
    api.store.seed(harness_record("u1", "reqdrive"), 0);
    let channels = api.state.hub.register("u1").unwrap();
    channels
        .feed
        .event(phase_changed(Phase::Queued, Phase::Provisioning, None));
    channels
        .feed
        .event(phase_changed(Phase::Provisioning, Phase::Spec, None));
    let mut warm = api.state.hub.subscribe("u1").unwrap();
    channels.feed.stage(Stage::Red, StageStatus::Started, None);
    warm.recv().await.unwrap();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = secured(api.router(), Token::new(TOKEN).unwrap(), &[]);
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

    let mut request = format!("ws://{address}/units/u1/stream?since=1")
        .into_client_request()
        .unwrap();
    request.headers_mut().insert(
        "sec-websocket-protocol",
        format!("{WS_PROTOCOL}, {WS_TOKEN_PREFIX}{TOKEN}")
            .parse()
            .unwrap(),
    );
    let (mut socket, response) = tokio_tungstenite::connect_async(request).await.unwrap();
    assert_eq!(
        response
            .headers()
            .get("sec-websocket-protocol")
            .and_then(|v| v.to_str().ok()),
        Some(WS_PROTOCOL),
        "the daemon selects its own subprotocol, never the one that carries the token"
    );
    channels
        .feed
        .event(phase_changed(Phase::Spec, Phase::Building, None));
    let mut seqs = Vec::new();
    while seqs.len() < 3 {
        let frame = socket.next().await.unwrap().unwrap();
        seqs.push(parsed(frame.to_text().unwrap())["seq"].as_u64().unwrap());
    }
    assert_eq!(seqs, [2, 3, 4]);

    // Without the token the upgrade is refused.
    let bare = format!("ws://{address}/units/u1/stream")
        .into_client_request()
        .unwrap();
    assert!(tokio_tungstenite::connect_async(bare).await.is_err());
}

// --- harnesses ---

#[tokio::test]
async fn the_harness_list_is_the_registrys_stored_state() {
    let mut api = TestApi::new();
    api.state.harnesses = Arc::new(FakeDirectory(vec![
        fleet_api::testkit::healthy("reqdrive"),
        fleet_api::testkit::unhealthy("fake", "harness-fake not found"),
        HarnessView {
            name: "later".into(),
            state: "unprobed".into(),
            harness: None,
            capabilities: None,
            error: None,
        },
    ]));
    let list = parsed(&api.get("/harnesses").await.1);
    assert_eq!(list[0]["state"], "healthy");
    assert_eq!(list[0]["capabilities"]["delivery"], "bundle");
    assert_eq!(
        list[1],
        json!({"name": "fake", "state": "unhealthy", "error": "harness-fake not found"})
    );
    assert_eq!(list[2], json!({"name": "later", "state": "unprobed"}));
}

// --- reading and signing a spec ---

#[tokio::test]
async fn a_spec_is_shown_exactly_as_it_was_read_at_a_named_commit() {
    let api = TestApi::new();
    let view = parsed(
        &api.get("/specs?repo=owner/sandbox&spec_id=SPEC-0001")
            .await
            .1,
    );
    assert_eq!(
        view["commit"],
        api.commit.as_str(),
        "with no commit asked for, the base tip is named"
    );
    assert_eq!(view["sha256"], api.spec_sha256.as_str());
    assert_eq!(
        view["text"].as_str().unwrap().as_bytes(),
        &api.spec_bytes[..]
    );
    assert_eq!(view["tier"], "t1");
    assert_eq!(view["work_item"], "example/sandbox#1");
    assert_eq!(view["problems"], json!([]));
    assert_eq!(view["signature"], Value::Null);

    let uri = format!(
        "/specs?repo=owner/sandbox&spec_id=SPEC-0001&commit={}",
        api.commit
    );
    assert_eq!(
        parsed(&api.get(&uri).await.1)["sha256"],
        api.spec_sha256.as_str()
    );
    let (status, body) = api.get("/specs?repo=owner/sandbox&spec_id=SPEC-9999").await;
    assert_eq!(
        (status, error(&body).code.as_str()),
        (StatusCode::NOT_FOUND, "spec_not_found")
    );
    let (status, body) = api.get("/specs?repo=other/repo&spec_id=SPEC-0001").await;
    assert_eq!(
        (status, error(&body).code.as_str()),
        (StatusCode::FORBIDDEN, "repo_not_allowed")
    );
}

#[tokio::test]
async fn signing_stores_who_when_the_hash_and_the_bytes_that_were_read() {
    let api = TestApi::new();
    let (status, text) = api.post("/signatures", api.sign_request()).await;
    assert_eq!(status, StatusCode::OK, "{text}");
    let answer = parsed(&text);
    assert_eq!(answer["signature"]["signed_by"], "owner");
    assert_eq!(answer["signature"]["signed_at_ms"], 1_700_000_000_000_i64);
    assert_eq!(answer["tier"], "t1");
    assert_eq!(answer["work_item"], "example/sandbox#1");

    let stored = api
        .store
        .get_signature("owner/sandbox", &api.spec_sha256)
        .unwrap()
        .unwrap();
    assert_eq!(stored.bytes, api.spec_bytes);
    assert_eq!(stored.commit, api.commit);
    assert_eq!(stored.spec_id, "SPEC-0001");
    let listed = parsed(&api.get("/signatures?repo=owner/sandbox").await.1);
    assert_eq!(listed.as_array().unwrap().len(), 1);
    assert!(
        listed[0].get("bytes").is_none(),
        "a listing carries no bytes"
    );
    let view = parsed(
        &api.get("/specs?repo=owner/sandbox&spec_id=SPEC-0001")
            .await
            .1,
    );
    assert_eq!(view["signature"]["sha256"], api.spec_sha256.as_str());
}

#[tokio::test]
async fn the_file_is_read_again_at_signing_and_a_hash_that_no_longer_matches_is_refused() {
    let api = TestApi::new();
    // The person was shown these bytes; the commit they name holds others.
    let mut request = api.sign_request();
    request["sha256"] = json!("0".repeat(64));
    let (status, text) = api.post("/signatures", request).await;
    assert_eq!(
        (status, error(&text).code.as_str()),
        (StatusCode::CONFLICT, "hash_mismatch")
    );
    assert!(api
        .store
        .list_signatures("owner/sandbox")
        .unwrap()
        .is_empty());

    // The spec changed on the branch after it was shown: the old commit still signs, and the
    // new tip, with the old hash, does not.
    let changed =
        String::from_utf8(SpecBuilder::new().criteria(&["AC-1", "AC-2"]).build()).unwrap();
    let tip = api
        .forge
        .commit_base(&api.repo, &[(SPEC_PATH, Some(changed.as_str()))]);
    let mut at_tip = api.sign_request();
    at_tip["commit"] = json!(tip);
    let (status, text) = api.post("/signatures", at_tip).await;
    assert_eq!(
        (status, error(&text).code.as_str()),
        (StatusCode::CONFLICT, "hash_mismatch")
    );
    assert_eq!(
        api.post("/signatures", api.sign_request()).await.0,
        StatusCode::OK
    );
}

#[tokio::test]
async fn a_spec_that_is_not_ready_is_not_signed_and_the_answer_says_why() {
    let api = TestApi::new();
    let draft = SpecBuilder::new()
        .open_question("Which currency?")
        .unconfirmed_invariant("Totals round down.")
        .build();
    let text = String::from_utf8(draft.clone()).unwrap();
    let commit = api
        .forge
        .commit_base(&api.repo, &[(SPEC_PATH, Some(text.as_str()))]);
    let request = json!({"repo": "owner/sandbox", "spec_id": "SPEC-0001", "commit": commit,
                         "sha256": factory_spec::sha256_hex(&draft)});
    let (status, body) = api.post("/signatures", request).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let refused = error(&body);
    assert_eq!(refused.code, "spec_not_ready");
    let codes: Vec<&str> = refused.problems.iter().map(|p| p.code.as_str()).collect();
    assert_eq!(codes, ["open_questions", "unconfirmed_invariants"]);
    assert!(api
        .store
        .list_signatures("owner/sandbox")
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn a_spec_that_touches_a_forms_interface_without_naming_the_form_is_not_signed() {
    let api = TestApi::new();
    let form = "# Form: cart\n\n- status: frozen\n- owner: someone\n- level: E1\n- last-drill: never\n- interface-files:\n  - src/cart.js\n\n## Purpose\n";
    let commit = api
        .forge
        .commit_base(&api.repo, &[("forms/cart.md", Some(form))]);
    let mut request = api.sign_request();
    request["commit"] = json!(commit);
    let (status, body) = api.post("/signatures", request).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(error(&body).problems[0].code, "form_not_declared");
}

#[tokio::test]
async fn bytes_that_are_not_a_spec_or_a_file_that_is_not_there_cannot_be_signed() {
    let api = TestApi::new();
    let commit = api
        .forge
        .commit_base(&api.repo, &[(SPEC_PATH, Some("just some text\n"))]);
    let request = json!({"repo": "owner/sandbox", "spec_id": "SPEC-0001", "commit": commit,
                         "sha256": factory_spec::sha256_hex(b"just some text\n")});
    let (status, body) = api.post("/signatures", request).await;
    assert_eq!(
        (status, error(&body).code.as_str()),
        (StatusCode::UNPROCESSABLE_ENTITY, "spec_unreadable")
    );

    let mut missing = api.sign_request();
    missing["spec_id"] = json!("SPEC-0404");
    let (status, body) = api.post("/signatures", missing).await;
    assert_eq!(
        (status, error(&body).code.as_str()),
        (StatusCode::NOT_FOUND, "spec_not_found")
    );

    // The file's own id must be the id that was asked for.
    let other = String::from_utf8(SpecBuilder::new().id("SPEC-0002").build()).unwrap();
    let commit = api
        .forge
        .commit_base(&api.repo, &[(SPEC_PATH, Some(other.as_str()))]);
    let request = json!({"repo": "owner/sandbox", "spec_id": "SPEC-0001", "commit": commit,
                         "sha256": factory_spec::sha256_hex(other.as_bytes())});
    let (status, body) = api.post("/signatures", request).await;
    assert_eq!(
        (status, error(&body).code.as_str()),
        (StatusCode::UNPROCESSABLE_ENTITY, "spec_id_mismatch")
    );
}

#[tokio::test]
async fn a_spec_id_is_never_used_as_a_path() {
    let api = TestApi::new();
    for hostile in [
        "../../README",
        "a/b",
        "SPEC 1",
        "",
        "..",
        "SPEC-1\\x",
        "%2e%2e",
    ] {
        let mut request = api.sign_request();
        request["spec_id"] = json!(hostile);
        let (status, body) = api.post("/signatures", request).await;
        assert_eq!(
            (status, error(&body).code.as_str()),
            (StatusCode::BAD_REQUEST, "bad_spec_id"),
            "{hostile:?}"
        );
    }
    assert!(
        !api.forge.calls().contains(&"read_file".to_string()),
        "a hostile id reached the forge"
    );
}

#[tokio::test]
async fn a_repository_that_is_not_allowed_or_cannot_be_read_is_refused() {
    let api = TestApi::new();
    let mut elsewhere = api.sign_request();
    elsewhere["repo"] = json!("other/repo");
    let (status, body) = api.post("/signatures", elsewhere).await;
    assert_eq!(
        (status, error(&body).code.as_str()),
        (StatusCode::FORBIDDEN, "repo_not_allowed")
    );
    api.forge.fail("read_file");
    let (status, body) = api.post("/signatures", api.sign_request()).await;
    assert_eq!(
        (status, error(&body).code.as_str()),
        (StatusCode::BAD_GATEWAY, "forge")
    );
}

// --- Forms ---

#[test]
fn the_forms_index_reads_interface_files_from_form_headers_and_ignores_other_files() {
    let form = |name: &str, files: &[&str]| {
        let list: String = files.iter().map(|f| format!("  - {f}\n")).collect();
        format!("# Form: {name}\n\n- status: frozen\n- owner: x\n- level: E1\n- last-drill: never\n- interface-files:\n{list}\n## Purpose\n\n- interface-files:\n  - not/in/the/header.rs\n")
    };
    let index = forms_index(&[
        (
            "forms/cart.md".into(),
            form("cart", &["src/cart.js", "src/cart/"]),
        ),
        (
            "forms/pay.md".into(),
            form("pay", &["src/pay.js"]).replace('\n', "\r\n"),
        ),
        (
            "forms/registry.md".into(),
            "# Gate registry\n\n| id | form |\n".into(),
        ),
        (
            "forms/README.md".into(),
            "# Forms\n\n- interface-files:\n  - README-example.rs\n".into(),
        ),
    ]);
    let entries: Vec<(&str, &str)> = index.entries().collect();
    assert_eq!(
        entries,
        [
            ("cart", "src/cart.js"),
            ("cart", "src/cart/"),
            ("pay", "src/pay.js")
        ]
    );
    assert_eq!(forms_index(&[]).entries().count(), 0);
}

// --- the schema ---

#[tokio::test]
async fn the_schema_route_serves_the_committed_contract() {
    let api = TestApi::new();
    let (status, body) = api.get("/schema").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, schema_json());
    let committed = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/contract/fleet-api.schema.json"
    );
    if std::env::var_os("FLEET_API_BLESS").is_some() {
        std::fs::create_dir_all(concat!(env!("CARGO_MANIFEST_DIR"), "/contract")).unwrap();
        std::fs::write(committed, schema_json()).unwrap();
    }
    let on_disk = std::fs::read_to_string(committed).unwrap_or_else(|_| {
        panic!(
            "{committed} is missing: run this test once with FLEET_API_BLESS=1 and commit the file"
        )
    });
    assert_eq!(
        on_disk.replace("\r\n", "\n"),
        schema_json(),
        "the API changed: re-bless with FLEET_API_BLESS=1 and regenerate the cockpit's types"
    );
}
````

### Implementation notes

**The token and the wrapper.** These bodies compile and were run against a small router
(bearer header, wrong header, the preflight, a real WebSocket upgrade); paste them under the
declarations Part 1 gives in `auth.rs`, with `use crate::dto::ApiError;` added:

````rust
use axum::extract::{Request, State};
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::{Json, Router};
use sha2::{Digest, Sha256};
use std::net::SocketAddr;
use tower_http::cors::{AllowOrigin, CorsLayer};

impl Token {
    pub fn new(value: &str) -> Result<Self, AuthError> {
        if value.len() < 32 {
            return Err(AuthError::Weak("shorter than 32 characters"));
        }
        if !value.chars().all(|c| c.is_ascii_graphic()) {
            return Err(AuthError::Weak("not printable ASCII without spaces"));
        }
        Ok(Token(value.to_string()))
    }

    pub fn from_env() -> Result<Self, AuthError> {
        match std::env::var(TOKEN_ENV) {
            Ok(value) => Token::new(&value),
            Err(_) => Err(AuthError::Missing),
        }
    }

    pub fn matches(&self, presented: &str) -> bool {
        // Digests have one length whatever the inputs are, and the loop reads all of both.
        let ours = Sha256::digest(self.0.as_bytes());
        let theirs = Sha256::digest(presented.as_bytes());
        let mut difference = 0u8;
        for (a, b) in ours.iter().zip(theirs.iter()) {
            difference |= a ^ b;
        }
        difference == 0
    }
}

pub fn secured(inner: Router, token: Token, allowed_origins: &[String]) -> Router {
    let origins: Vec<HeaderValue> = allowed_origins
        .iter()
        .filter_map(|origin| origin.parse().ok())
        .collect();
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list(origins))
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION]);
    // The layer added last runs first: CORS answers a preflight before the token is asked for,
    // and puts its headers on a 401 too, so a browser can read the refusal.
    inner
        .layer(middleware::from_fn_with_state(token, require_token))
        .layer(cors)
}

async fn require_token(State(token): State<Token>, request: Request, next: Next) -> Response {
    let admitted = presented(request.headers()).is_some_and(|value| token.matches(value));
    if admitted {
        return next.run(request).await;
    }
    let body = ApiError {
        code: "unauthorized".into(),
        message: "this request does not carry the daemon's token".into(),
        problems: Vec::new(),
    };
    (StatusCode::UNAUTHORIZED, Json(body)).into_response()
}

/// The token a request presents: a bearer header, or, on a WebSocket upgrade only, the
/// subprotocol entry that carries it.
fn presented(headers: &HeaderMap) -> Option<&str> {
    if let Some(value) = headers.get(header::AUTHORIZATION) {
        return value.to_str().ok()?.strip_prefix("Bearer ");
    }
    let upgrading = headers
        .get(header::UPGRADE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.eq_ignore_ascii_case("websocket"));
    if !upgrading {
        return None;
    }
    headers
        .get_all(header::SEC_WEBSOCKET_PROTOCOL)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .find_map(|entry| entry.trim().strip_prefix(WS_TOKEN_PREFIX))
}

pub fn loopback_address(text: &str) -> Result<SocketAddr, AuthError> {
    let address: SocketAddr = text
        .parse()
        .map_err(|_| AuthError::BadAddress(text.to_string()))?;
    if !address.ip().is_loopback() {
        return Err(AuthError::NotLoopback {
            address: text.to_string(),
        });
    }
    Ok(address)
}
````

Three things in it are easy to get wrong. The layer added last runs first, so CORS must be
added after the token check or a browser's preflight is answered 401. `Authorization` must be
in the allowed headers or a browser refuses to send the request at all (today's list has only
`Content-Type`). And the subprotocol form is accepted on an upgrade only, so a token never
arrives in a header that a proxy log treats as harmless on an ordinary request.

**The stream.** The upgrade names the daemon's own subprotocol:

````rust
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::Path;

/// The upgrade names the one subprotocol the daemon speaks. axum answers with the first entry
/// the client offered that is in this list, so the entry that carries the token is never echoed.
async fn stream(Path(id): Path<String>, upgrade: WebSocketUpgrade) -> Response {
    upgrade
        .protocols([WS_PROTOCOL])
        .on_upgrade(move |socket| follow(socket, id))
}

async fn follow(mut socket: WebSocket, id: String) {
    let _ = socket.send(Message::Text(format!("hello {id}"))).await;
}
````

In the real handler `follow` is today's `stream_to_socket` (server.rs lines 662 to 707) with
the hub in place of the unit table: call `Hub::subscribe` first, then read
`UnitStore::events_since(id, since)`, send those, then send each broadcast record whose `seq`
is higher than the last one sent. Subscribing first is what makes the hand-over lose nothing;
the comparison is what makes it send nothing twice. A unit with no live run gets the replay
and then the socket is closed. A lagged receiver continues; a closed one ends the stream. The
`since` query is read with the same `SinceQuery` as the events route.

**The hub.** One `Mutex<HashMap<String, Entry>>`, where an entry holds the command sender and
the broadcast sender (capacity 1024, as today). An entry is never removed while the process
lives: `knows` is "has an entry", `live` is "has an entry whose command sender is not closed",
and `register` returns `None` whenever an entry exists, so the check and the insert are under
one lock (server.rs lines 410 to 427 do this today). `send` maps a missing entry to `Unknown`,
a failed send to `Gone`, and success to `Accepted`.

`register` starts one task per unit, the successor of `spawn_forwarder`. It reads the unit's
`last_seq` from the store once (0 when the unit is not stored or the store cannot be read) and
then, for each record from the feed:

| Record | What the task does |
|---|---|
| `Event(e)` | `seq += 1`; serialise `fleet_core::EventEnvelope { unit_id, seq, event: e }`; `record_event(unit_id, seq, clock.now_ms(), &json, Some(&e))`; broadcast `LoggedEvent` |
| `Stage{…}` | `seq += 1`; serialise `EventEnvelopeDto` with `UnitEventDto::Stage`; `record_event(…, None)`; broadcast |
| `Frozen(f)` | `put_freeze(unit_id, &json, now)`; no number, no broadcast |
| `Result(r)` | `put_result(unit_id, &json, now)`; no number, no broadcast |

The fold that `spawn_forwarder` did by hand (phase, cost, terminal reason, oracle hash) is now
the store's, inside `record_event`; do not repeat it here. A store error is written to standard
error with the unit id and the sequence number, and the record is still broadcast: the number
is not reused. The task ends when every `UnitFeed` clone is dropped; it must not hold the
hub's lock across an `await`.

**`POST /missions`.** Take the body as `Json<MissionBody>`, so that axum answers 422 for a
body that fits neither shape; `MissionRequest` denies unknown fields, which is what refuses
`tier`. For a `MissionRequest`: build `DispatchRequest` field for field, `admit`, and on
`Ok(admitted)` call `launch(&admitted)`. A refusal is `ApiError { code: refusal.code(),
message: refusal.to_string(), problems: [] }` with `refusal_status`; a `LaunchError` is 500
`launch_failed`. The unit is already stored when a launch fails; it stays `queued` and the
startup pass of lane CC-WIRE halts it, so do not delete it here. For a `LegacyMissionRequest`:
`launcher.legacy_mission(request)`, answering 200 with `MissionResponse { unit_id,
waits_for_slot: None }` or the status and text it returns, as plain text.

**`refusal_status`** is one `match` on the variant with no wildcard arm, so that a new refusal
cannot be added without choosing its status; the statuses are in the function's own
documentation in Part 1.

**The read routes.** `GET /units` and `GET /units/:id` build `UnitSummary` and `Snapshot` from
`UnitRecord::row` exactly as server.rs lines 558 to 608 do, but a store error is 503 `store`
where today it is an empty answer. `UnitDetail` is the row, the `UnitExt`, and
`live: hub.live().contains(id)`; `oracle_frozen` is `ext.freeze_json.is_some()`. A stored phase
or tier that does not parse is a 503 `store`, never a default. Stored event text is parsed into
`EventEnvelopeDto`; a line that does not parse is skipped, as today.

**`POST /units/:id/commands`.** Take `Json<CommandDto>`, convert to `fleet_core::Command`
variant by variant, then: `hub.send`; on `Unknown`, `launcher.rehydrate(id)` and, when that
returned true, `hub.send` once more. `Accepted` is 202, `Gone` is 410, a second `Unknown` or a
false `rehydrate` is 404.

**Reading and signing a spec.** One private function does the reading for both routes, in
this order, and returns at the first failure:

| Step | Check | Failure |
|---|---|---|
| 1 | `repo` is one of `ApiState::repos`, compared as a whole slug | 403 `repo_not_allowed` |
| 2 | `spec_id` is 1 to 64 characters, each an ASCII letter, a digit, `-`, `_` or `.`, and the first is not `.` | 400 `bad_spec_id` |
| 3 | `commit` is the one given, or `forge.resolve_base(repo)` | 502 `forge` |
| 4 | `forge.read_file(repo, commit, ".reqdrive/specs/<spec_id>.md")` | `Ok(None)` is 404 `spec_not_found`; an error is 502 `forge` |
| 5 | `sha256` is `factory_spec::sha256_hex(&bytes)` | (cannot fail) |

`GET /specs` then parses and validates and reports what it finds without refusing: a parse
error is one `ProblemView` with the parse error's code, and `tier` and `work_item` are then
absent. `signature` is `get_signature(repo, sha256)`, shown without its bytes.

`POST /signatures` continues:

| Step | Check | Failure |
|---|---|---|
| 6 | the request's `sha256` equals the hash of step 5 | 409 `hash_mismatch` |
| 7 | `factory_spec::parse(&bytes)` | 422 `spec_unreadable`, the parse error as the message |
| 8 | the parsed `id` equals `spec_id` | 422 `spec_id_mismatch` |
| 9 | `factory_spec::validate(&spec, &forms)` is empty, where `forms` is `forms_index` over each path `forge.list_files(repo, commit, "forms/")` names that ends in `.md`, read with `read_file` at the same commit | 422 `spec_not_ready` with every problem's `code()` and text; a forge error is 502 `forge` |
| 10 | `put_signature(Signature { repo_slug, spec_id, sha256, commit, bytes, signed_by: operator, signed_at_ms: clock.now_ms() })` | 503 `store` |

The bytes stored are the bytes of step 4: the request never carries any. Signing the same bytes
twice is not an error; the store keeps one row for a repository and a hash.

**`forms_index`.** For each file: strip a leading byte-order mark, read lines with `\r`
trimmed; it is a Form only if its first non-blank line starts `# Form: `, and the name is the
rest of that line. Its header is the lines before the first line that starts `## `. Inside the
header, find the line `- interface-files:` and take the following lines that start with two or
more spaces and `- ` as paths, stopping at the first line that does not. Paths are inserted in
file order with `FormsIndex::insert`.

**The schema.** `schema_json()` is delivered by Part 1 and already deterministic. Run
`FLEET_API_BLESS=1 cargo test -p fleet-api --test contract the_schema_route` once (PowerShell:
`$env:FLEET_API_BLESS = '1'` first, and remove it after), commit the file it writes, and check
that it ends with one newline and has no carriage return. The route serves the same text with
`content-type: application/json`.

**Pitfalls.** Handlers are `async` and the seams of `UnitStore` are not: a store call is short
and may be made directly, but never while holding the hub's lock. `ApiState` is cloned per
request, so everything in it is behind an `Arc` already. axum 0.7 writes a path parameter as
`:id`, and `ROUTES` uses the same spelling. Behaviours 1 and 2 walk `ROUTES`, so a route that
is in the router and not in the table is tested by nothing: the two are changed together. Take
`repo` and `spec_id` from `Query<…>` and validate the decoded value; never validate the raw
query string.

### Verify

| Command | Expected |
|---|---|
| `cargo test -p fleet-api` | lib (8) and `contract` (32) pass |
| `cargo test -p fleet-api --features testkit` | the same |
| `cargo xtask test static --root-only`, `unit`, `contract` | exit 0 |
| *Git-Bash:* `grep -rn "unimplemented!" crates/fleet-api/src` | no output |
| *Git-Bash:* `grep -rn "println!\|dbg!" crates/fleet-api/src --include=*.rs \| grep -v testkit.rs` | no output: nothing here prints a request |
| *Git-Bash:* `git diff --stat origin/factory/m1...HEAD -- crates/fleet-api/src/dto.rs crates/fleet-api/src/schema.rs crates/fleet-api/src/ports.rs` | no output |
| `git diff --name-only origin/factory/m1...HEAD` | only paths under **Owns** |

The characterisation tests of `fleetd` are untouched by this lane: nothing in `fleetd` calls
this crate until lane CC-WIRE.

### Done when

- [ ] Behaviours 1 to 32 pass; no `unimplemented!` is left in `crates/fleet-api/src`.
- [ ] `crates/fleet-api/contract/fleet-api.schema.json` is committed and behaviour 32 passes
      without `FLEET_API_BLESS`.
- [ ] The pull request names the registry rows it wants, and says that it does not close
      issue #91: the daemon starts refusing a request without the token in lane CC-WIRE, and
      the issue closes when the cockpit mints and sends one.

---

## Lane CC-WIRE

The last lane of the control plane. It joins the seven crates into the daemon, and it is where
the two run paths meet. It decides nothing about a unit: every rule it touches lives in the
crate it calls, and a rule found here that belongs elsewhere is a finding.

**Owns:** `crates/fleetd/src/wire.rs` (the bodies), `crates/fleetd/src/server.rs`,
`store.rs`, `forge.rs`, `gh_forge.rs`, `reconcile.rs`, `crates/fleetd/src/bin/serve.rs`,
`crates/fleetd/tests/wire.rs`, `crates/fleetd/tests/wire_it.rs`, and the three characterisation
files it changes on purpose (listed below). It may add private modules under
`crates/fleetd/src/`. It does not change `driver.rs`, `steps.rs`, `claude_meter.rs`, `retry.rs`,
`fake.rs`, `runner.rs`, `local_docker.rs`, `swarm.rs`, `planner.rs`, `docsource.rs`,
`bin/run_once.rs`, or anything in another crate.

**Reads:** every Form draft of Part 2; the supervisor spec, sections 3.3, 3.5, 5 and 6 items 7
and 8; `crates/fleetd/src/server.rs`, all of it; the five characterisation files of
`crates/fleetd/tests/`; issues #74, #84, #85, #90 and #91.

**Worktree and branch:** `D:\MajorProjects\.swarm-wt\m1-cc-wire`, branch `feat/m1-fleetd`.

**Needs:** lanes CC-STORE, CC-FORGE, CC-RUNNER, CC-VERIFY, CC-SUPERVISOR, CC-ADMISSION and
CC-API merged into `factory/m1`. Do not start on fakes of them: this lane's tests run the real
store, the real supervisor and the real `harness-fake`.

**Blocks:** CC-E2E's scenarios; the cockpit, which cannot reach the daemon without the token
once this lane has merged (Part 3 says how the two are ordered).

### Form draft: `fleetd`

**Purpose.** The daemon's wiring: it chooses the real implementation of every seam, joins them,
and serves the result on a loopback address behind a token. Until milestone 2 it also carries
the in-process driver, the path a unit takes when it has no harness. Without the wiring module
the choice of implementations would be spread through the server, and a test could not swap
one.

**interface-files:** `crates/fleetd/src/wire.rs`.

**Interfaces** (the signatures of Part 1, Task 9):

````rust
// crates/fleetd/src/wire.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrHostChoice {
    GitHub,
    Local { dir: PathBuf },
}

#[derive(Debug, Clone)]
pub struct Settings {
    pub addr: SocketAddr,
    pub db: PathBuf,
    pub data_dir: PathBuf,
    pub registry_file: Option<PathBuf>,
    pub token: Token,
    pub allowed_origins: Vec<String>,
    pub admission: AdmissionConfig,
    pub max_concurrent: usize,
    pub grace: Duration,
    pub stall: Duration,
    pub reconcile_secs: u64,
    pub operator: String,
    pub pr_host: PrHostChoice,
    pub legacy_image: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SettingsError {
    #[error("{0}")]
    Auth(String),
    #[error("the configuration file does not read: {0}")]
    Malformed(String),
    #[error("{key}: {problem}")]
    Invalid { key: &'static str, problem: String },
}

pub fn settings(
    file: Option<&str>,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<Settings, SettingsError>;

pub struct Parts {
    pub store: Arc<SharedStore>,
    pub forge: Arc<dyn RepoForge>,
    pub runner: Arc<dyn CheckRunner>,
    pub verifier: Arc<dyn EvidenceVerifier>,
    pub registry: Registry,
    pub demo: fleet_supervisor::Deps,
}

impl Parts {
    pub fn real(settings: &Settings) -> Result<Parts, WireError>;
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WireError {
    #[error("the store could not be opened: {0}")]
    Store(String),
    #[error("the harness registry could not be read: {0}")]
    Registry(String),
    #[error("the daemon could not bind {addr}: {error}")]
    Bind { addr: String, error: String },
}

pub struct Wired {
    pub secured: Router,
    pub open: Router,
    pub hub: Hub,
    pub registry: Registry,
}

pub fn wire(settings: &Settings, parts: Parts) -> Result<Wired, WireError>;

pub async fn serve(settings: Settings, parts: Parts) -> Result<(), WireError>;

pub struct FeedSink(pub UnitFeed);

pub struct RegistryView(pub Registry);

pub struct Census {
    pub store: Arc<SharedStore>,
    pub hub: Hub,
}
````

**Invariants**

- I1. The program serves no route without the token: what it binds is `Wired::secured` and
  nothing else, on an address `fleet_api::loopback_address` accepted.
- I2. A unit's stored `harness` decides its path, when it is started and when it is brought
  back: a registered name runs on `fleet_supervisor::run`; no value runs on
  `fleetd::driver::run`; a name that is not registered is not run at all.
- I3. No unit starts without a stored reservation, on either path: a mission is refused when
  `UnitStore::reserve_unit` does not answer `Reserved`.
- I4. Unit ids on both paths come from `Admission::mint_unit_id` and from `Admit::admit`, which
  share one counter.
- I5. A demo unit is verified and delivered with `Parts::demo` and never with the real
  verifier or the real forge; a unit run from a signed spec is never given `Parts::demo`.
- I6. A restart leaves a unit that waits for a person exactly as it was, and halts a unit
  that was running exactly once.
- I7. The registry is probed before anything is reaped and before the address is bound.
- I8. No cap, repository, test command or image is a literal in the code that starts a unit
  from a signed spec: each comes from `Settings` or from what admission read.
- I9. `wire.rs` holds no rule about a unit: its adapters convert types and call one seam each.

**Hidden decisions.** How the launcher's state is held; how the driver's own event envelopes
are fed to the hub; where under `data_dir` each thing lives; the configuration file's name
(`serve` reads the path in `FLEETD_CONFIG`, else `fleetd.toml` in the working directory if it
exists).

**Gates**

| id | guards | mechanism | location | command | blocks |
|---|---|---|---|---|---|
| workspace.G1 | (the crate graph) | Build graph: the dependency allowlist | `xtask/src/deps.rs#pub const RULES` | `cargo xtask test static` | merge |
| the characterisation suite | the driver path's behaviour | Locked tests | `crates/fleetd/tests/characterisation_*.rs` | `cargo xtask test integration` | merge |

**Unenforced (planned)**

| planned id | guards | mechanism | location | command | built by |
|---|---|---|---|---|---|
| fleetd.G1 | I1 | Locked test over every route the daemon serves | `crates/fleetd/tests/wire.rs#every_route_the_daemon_serves_needs_the_token_including_the_ones_the_old_path_owns` | `cargo xtask test contract` | behaviour 9 |
| fleetd.G2 | I2, I5 | Locked tests of the two paths | `crates/fleetd/tests/wire.rs#a_stored_unit_is_brought_back_on_the_path_its_harness_column_names` | `cargo xtask test contract` | behaviours 10 to 12 |
| fleetd.G3 | I3 | Locked tests for issue #74 on the old request shape | `crates/fleetd/tests/wire_it.rs#a_mission_whose_reservation_cannot_be_recorded_is_refused_and_starts_nothing` | `cargo xtask test integration` | behaviour 16 |
| fleetd.G4 | I6 | Locked test across a restart | `crates/fleetd/tests/wire_it.rs#a_unit_that_needs_a_human_still_needs_one_for_the_same_reason_after_a_restart` | `cargo xtask test integration` | behaviour 17 |

I4 is guarded by `fleet-admission`'s behaviour 35 and by the characterisation tests that pin
`u1`, `u2`, …; I7 and I8 by review and by the **Verify** greps; I9 by review.

### The two paths, exactly

| What arrives | What is stored | What runs it |
|---|---|---|
| `POST /missions` with `spec_sha256` | what admission built: `harness` is the request's, `mode` is `real` | `fleet_supervisor::run` with the registry's connector for that harness, `Parts::verifier` and `Parts::forge` |
| `POST /missions` with `task`, `mode: "demo"` (the default) | today's row, with `harness = "fake"` and `opt_in = true` in the extension | `fleet_supervisor::run` with the connector for `fake` and `Parts::demo` |
| `POST /missions` with `task`, `mode: "real"` | today's row; no harness | `fleetd::driver::run` with `LocalDockerRunner` and `GhForge`, as today |
| `POST /swarms`, either mode | today's rows; no harness | `fleetd::driver::run`, as today |
| a command for a stored unit with no run, `harness` set and registered | nothing new | `fleet_supervisor::run`, from the stored record; `Parts::demo` when the row's `mode` is `demo`, else the real parts |
| the same, `harness` set and not registered | nothing | nothing: the command is answered 404 |
| the same, no `harness` | nothing new | `fleetd::driver::run`, by the row's `mode`, as today |

No stored row is rewritten to move it between paths; the backfill of the supervisor spec's
section 5 is not run. A demo unit stored before this lane merged therefore finishes on the
driver path, and `fake.rs` and `demo_script` stay for it and for swarms.

**A mission in the old request shape** keeps today's order, because the characterisation tests
pin it step by step:

| Step | What | Refusal |
|---|---|---|
| 1 | Mint the id with `Admission::mint_unit_id`, before anything is checked | (none: a refused mission still uses up an id) |
| 2 | `mode` is `demo` or `real`; for `real`, `ANTHROPIC_API_KEY` is set | 400 `unknown mode: <mode>`; 400 `ANTHROPIC_API_KEY not set` |
| 3 | For `demo`: the registry's state for `fake` is not `Unhealthy` (an unprobed entry may be tried: the unit's own `initialize` decides) | 503 with the stored error |
| 4 | `reserve_unit(record, now - 24 h, global cap, now)` | `OverCap` is 429 `global daily cost cap reached`; an `Err` is 500 `the reservation could not be recorded` |
| 5 | `Hub::register(unit id)` | `None` is 409 `unit already exists` |
| 6 | Start the run on the path the table above names | |

Steps 4 and 5 are where issue #74 closes on this path: the old code discarded the insert's
result and read a failed spend query as zero.

**A unit started from a signed spec** (`Launcher::launch`): the connector for
`record.ext.harness`, or `LaunchError::UnknownHarness`; `Hub::register`, or `AlreadyRunning`;
then the run. Its `UnitSpec` is the record's row field for field (`repo` from `repo_url`,
`repo_slug`, `base_branch`), `work_item` of kind `issue` with the record's reference, and
`factory` built from the parsed signed bytes (`spec_id`, `scope`, `permitted_dependencies`,
`expected_red`), the bytes themselves, `Admitted::base_sha`, `Admitted::config` and the
record's profile. Its `RunCtx` is a fresh one (`Queued`, nothing spent) with the record's
`opt_in`, the settings' grace and stall, the shared slots, and
`work_dir = <data_dir>/units/<unit id>`.

**A demo unit's `UnitSpec`** is today's `create_mission` values (lines 354 to 369: the sandbox
repository, `agent/<id>`, `node --test`, $5, 1800 s, the request's floor raised to 1), with
`work_item` of kind `roadmap_item` and reference `demo:<unit id>`, and no `factory`. Its
`RunCtx::opt_in` is true.

**Bringing a unit back** (`Launcher::rehydrate`), for a unit with a harness. The route is not
`async`, so the function registers the unit with the hub first and returns true, and one task
then prepares the run and starts it:

| `RunCtx` field | From the stored record |
|---|---|
| `start_phase` | `row.phase` |
| `cost_usd`, `elapsed_ms` | `row.cost`, `ext.elapsed_ms` |
| `oracle_frozen`, `approved_oracle_hash` | `row.oracle_frozen`, `ext.approved_oracle_hash` |
| `freeze`, `last_result` | `ext.freeze_json`, `ext.result_json`, parsed; a value that does not parse is treated as absent and logged |
| `pause` | for `needs_human` and `halted`: `Pause { reason: ext.pause_reason, from: ext.paused_from }`; otherwise none |
| `opt_in` | `ext.opt_in` |

For a unit run from a signed spec the task also needs the signed bytes
(`SignatureStore::get_signature(repo_slug, spec_sha256)`) and the repository's configuration
(`RepoForge::read_file(repo, base_sha, REPO_CONFIG_PATH)` parsed with
`fleet_admission::parse_repo_config`: the same commit, so the same bytes admission read). If
either is missing the task reports one `Error` of scope `system` through the feed and ends;
the unit then answers 410 until the daemon restarts. `rehydrate` returns false, and registers
nothing, for a unit that is not stored, a unit in a terminal phase, and a unit whose harness
is not registered.

### Behaviours

Thirteen tests in `crates/fleetd/tests/wire.rs` (contract tier: no process, no Docker, no
network) and five in `crates/fleetd/tests/wire_it.rs` (integration tier: the real
`harness-fake`).

Settings:

1. `with_only_a_token_the_settings_are_todays_defaults`
2. `the_presets_admission_is_told_about_are_the_ones_this_build_links`
3. `without_a_token_or_on_a_public_address_there_are_no_settings`
4. `the_file_sets_caps_and_the_allowlist_and_the_environment_overrides_single_values`
5. `settings_that_would_spend_without_limit_or_name_a_repository_twice_are_refused`

The adapters:

6. `the_feed_sink_passes_each_of_the_four_reports_through`
7. `the_registry_view_answers_admission_and_the_api_from_the_stored_probe_state`
8. `the_census_reads_the_store_and_the_hub_and_a_stranded_unit_keeps_its_reason`

The wired router:

9. `every_route_the_daemon_serves_needs_the_token_including_the_ones_the_old_path_owns`
10. `a_demo_mission_is_refused_with_503_while_the_scripted_harness_is_unhealthy`
11. `a_real_mission_in_the_old_shape_still_goes_to_the_in_process_driver`
12. `a_stored_unit_is_brought_back_on_the_path_its_harness_column_names`
13. `the_supervisor_and_the_store_use_the_same_tampering_reason` (passes from Part 1 on)

Through the real scripted harness (supervisor spec section 6, items 7 and 8):

14. `a_demo_mission_runs_to_done_through_the_real_scripted_harness`
15. `three_units_waiting_at_their_gates_leave_room_for_a_fourth`
16. `a_mission_whose_reservation_cannot_be_recorded_is_refused_and_starts_nothing`
17. `a_unit_that_needs_a_human_still_needs_one_for_the_same_reason_after_a_restart`
18. `tampering_found_at_verification_pauses_a_demo_unit_and_clears_its_approval`

Run: `cargo test -p fleetd --test wire`, then `cargo build -p harness-conformance --bins` and
`cargo test -p fleetd --test wire_it`. Expected before the implementation, with the seven
lanes merged: 12 of the 13 fail and all 5 fail, each with
`not implemented: lane CC-WIRE builds this`. Behaviour 11 needs `ANTHROPIC_API_KEY` unset in
the shell that runs it, and says so if it is not.

`crates/fleetd/tests/wire.rs`:

````rust
//! Lane CC-WIRE: the settings, the adapters and the wired router, with no process started.
//!
//! No Docker, no git, no network, no harness binary. The store is SQLite in memory; the forge,
//! the runner and the verifiers are the other crates' fakes.

use axum::http::StatusCode;
use fleet_admission::{HarnessCatalog, HarnessStatus};
use fleet_api::testkit::{call, harness_directory_contract, TOKEN};
use fleet_api::{HarnessDirectory, Hub, SystemClock, ROUTES};
use fleet_core::{Event, Phase, Tier};
use fleet_forge::testkit::MemForge;
use fleet_runner::testkit::FakeRunner;
use fleet_runner::UnitCensus;
use fleet_store::testkit::{harness_record, phase_changed, record};
use fleet_store::{SharedStore, Store, UnitStore};
use fleet_supervisor::{HarnessEntry, ProbeState, Registry, Sink, FAKE_HARNESS};
use fleet_verify::testkit::ScriptedVerifier;
use fleetd::wire::{
    settings, wire, Census, FeedSink, Parts, PrHostChoice, RegistryView, Settings, SettingsError,
};
use harness_protocol::{OracleFreeze, Stage, StageStatus};
use serde_json::json;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
    let map: HashMap<String, String> = pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    move |key| map.get(key).cloned()
}

fn defaults() -> Settings {
    settings(None, &env(&[("FLEETD_TOKEN", TOKEN)])).unwrap()
}

fn store() -> Arc<SharedStore> {
    Arc::new(Mutex::new(Store::open_memory().unwrap()))
}

/// A registry whose scripted harness is a program that does not exist.
fn registry_without_a_fake() -> Registry {
    Registry::new(vec![HarnessEntry {
        name: FAKE_HARNESS.into(),
        command: vec!["no-such-harness-fake".into()],
        env: Vec::new(),
    }])
}

fn parts(store: Arc<SharedStore>, registry: Registry) -> Parts {
    let forge = Arc::new(MemForge::new());
    Parts {
        store,
        forge: forge.clone(),
        runner: Arc::new(FakeRunner::new()),
        verifier: Arc::new(ScriptedVerifier::new()),
        registry,
        demo: fleet_supervisor::Deps {
            verifier: Arc::new(ScriptedVerifier::new()),
            forge,
        },
    }
}

// --- settings ---

#[test]
fn with_only_a_token_the_settings_are_todays_defaults() {
    let s = defaults();
    assert_eq!(s.addr.to_string(), "127.0.0.1:8787");
    assert_eq!(s.db.to_string_lossy(), "fleet.db");
    assert_eq!(s.max_concurrent, 3);
    assert_eq!(s.admission.unit_usd_ceiling, 5.0);
    assert_eq!(s.admission.unit_wall_clock_ceiling_secs, 1800);
    assert_eq!(s.admission.global_usd_cap, 20.0);
    assert_eq!(s.admission.spend_window_ms, 24 * 3600 * 1000);
    assert_eq!(s.admission.repos.len(), 1);
    assert_eq!(
        s.admission.repos[0].slug,
        "adbarc92/command-center-agent-sandbox"
    );
    assert_eq!(s.admission.repos[0].base_branch, "main");
    assert_eq!(
        (s.grace, s.stall),
        (Duration::from_secs(30), Duration::from_secs(600))
    );
    assert_eq!(s.reconcile_secs, 30);
    assert_eq!(s.pr_host, PrHostChoice::GitHub);
    assert_eq!(s.registry_file, None);
    assert_eq!(s.legacy_image, "cc-agent:dev");
}

#[test]
fn the_presets_admission_is_told_about_are_the_ones_this_build_links() {
    let s = defaults();
    assert_eq!(
        s.admission.presets_version,
        factory_presets::PRESETS_VERSION
    );
    for name in &s.admission.presets {
        assert!(
            factory_presets::preset(name).is_some(),
            "{name} is not a linked preset"
        );
    }
    assert_eq!(s.admission.presets, ["cargo", "node"]);
}

#[test]
fn without_a_token_or_on_a_public_address_there_are_no_settings() {
    assert!(matches!(
        settings(None, &env(&[])),
        Err(SettingsError::Auth(_))
    ));
    assert!(matches!(
        settings(None, &env(&[("FLEETD_TOKEN", "short")])),
        Err(SettingsError::Auth(_))
    ));
    assert!(matches!(
        settings(
            None,
            &env(&[("FLEETD_TOKEN", TOKEN), ("CC_ADDR", "0.0.0.0:8787")])
        ),
        Err(SettingsError::Auth(_))
    ));
}

const FILE: &str = r#"
[caps]
unit_usd = 2.5
unit_wall_clock_secs = 900
global_usd = 10.0
max_concurrent = 2

[[repo]]
slug = "owner/one"
url = "https://example.invalid/owner/one"
base_branch = "main"

[[repo]]
slug = "owner/two"
url = "https://example.invalid/owner/two"
base_branch = "trunk"
"#;

#[test]
fn the_file_sets_caps_and_the_allowlist_and_the_environment_overrides_single_values() {
    let s = settings(
        Some(FILE),
        &env(&[
            ("FLEETD_TOKEN", TOKEN),
            ("CC_ADDR", "127.0.0.1:9999"),
            ("CC_DB", "elsewhere.db"),
            ("CC_GLOBAL_USD_CAP", "7.5"),
            ("FLEETD_HARNESS_GRACE_SECS", "5"),
            ("FLEETD_HARNESS_STALL_SECS", "60"),
            ("CC_RECONCILE_SECS", "0"),
            ("FLEETD_REGISTRY", "harnesses.toml"),
            ("FLEETD_PR_HOST", "local:prs"),
            (
                "FLEETD_ALLOWED_ORIGINS",
                "http://localhost:5173, http://tauri.localhost",
            ),
            ("FLEETD_OPERATOR", "alex"),
        ]),
    )
    .unwrap();
    assert_eq!(s.addr.port(), 9999);
    assert_eq!(s.db.to_string_lossy(), "elsewhere.db");
    assert_eq!(s.admission.unit_usd_ceiling, 2.5);
    assert_eq!(s.admission.unit_wall_clock_ceiling_secs, 900);
    assert_eq!(
        s.admission.global_usd_cap, 7.5,
        "the environment wins over the file"
    );
    assert_eq!(s.max_concurrent, 2);
    let slugs: Vec<&str> = s.admission.repos.iter().map(|r| r.slug.as_str()).collect();
    assert_eq!(
        slugs,
        ["owner/one", "owner/two"],
        "the file replaces the default allowlist"
    );
    assert_eq!(s.admission.repos[1].base_branch, "trunk");
    assert_eq!(
        (s.grace, s.stall),
        (Duration::from_secs(5), Duration::from_secs(60))
    );
    assert_eq!(s.reconcile_secs, 0);
    assert_eq!(
        s.registry_file
            .as_deref()
            .map(|p| p.to_string_lossy().into_owned())
            .as_deref(),
        Some("harnesses.toml")
    );
    assert_eq!(s.pr_host, PrHostChoice::Local { dir: "prs".into() });
    assert_eq!(
        s.allowed_origins,
        ["http://localhost:5173", "http://tauri.localhost"]
    );
    assert_eq!(s.operator, "alex");
}

#[test]
fn settings_that_would_spend_without_limit_or_name_a_repository_twice_are_refused() {
    let token = [("FLEETD_TOKEN", TOKEN)];
    for (bad, key) in [
        (
            FILE.replace("unit_usd = 2.5", "unit_usd = 0"),
            "caps.unit_usd",
        ),
        (
            FILE.replace("unit_usd = 2.5", "unit_usd = -1.0"),
            "caps.unit_usd",
        ),
        (
            FILE.replace("global_usd = 10.0", "global_usd = 1.0"),
            "caps.global_usd",
        ),
        (
            FILE.replace("max_concurrent = 2", "max_concurrent = 0"),
            "caps.max_concurrent",
        ),
        (
            FILE.replace("unit_wall_clock_secs = 900", "unit_wall_clock_secs = 0"),
            "caps.unit_wall_clock_secs",
        ),
        (FILE.replace("owner/two", "owner/one"), "repo"),
        (
            FILE.replace(
                "base_branch = \"trunk\"",
                "base_branch = \"--upload-pack=x\"",
            ),
            "repo",
        ),
    ] {
        match settings(Some(&bad), &env(&token)) {
            Err(SettingsError::Invalid { key: got, .. }) => assert_eq!(got, key),
            other => panic!("{key}: {other:?}"),
        }
    }
    assert!(matches!(
        settings(Some("[caps]\nunit_usd = "), &env(&token)),
        Err(SettingsError::Malformed(_))
    ));
    assert!(matches!(
        settings(Some("[caps]\nunit_dollars = 5\n"), &env(&token)),
        Err(SettingsError::Malformed(_))
    ));
    assert!(matches!(
        settings(
            None,
            &env(&[("FLEETD_TOKEN", TOKEN), ("CC_GLOBAL_USD_CAP", "lots")])
        ),
        Err(SettingsError::Invalid {
            key: "CC_GLOBAL_USD_CAP",
            ..
        })
    ));
}

// --- the constants two crates must agree on ---

#[test]
fn the_supervisor_and_the_store_use_the_same_tampering_reason() {
    assert_eq!(
        fleet_supervisor::REASON_ORACLE_TAMPERING,
        fleet_store::REASON_ORACLE_TAMPERING
    );
}

// --- adapters ---

#[tokio::test]
async fn the_feed_sink_passes_each_of_the_four_reports_through() {
    let store = store();
    store
        .reserve_unit(&harness_record("u1", "fake"), 0, 20.0, 0)
        .unwrap();
    let hub = Hub::new(store.clone(), Arc::new(SystemClock));
    let channels = hub.register("u1").unwrap();
    let mut stream = hub.subscribe("u1").unwrap();
    let mut sink = FeedSink(channels.feed);
    let freeze = OracleFreeze {
        frozen_files: Vec::new(),
        frozen_ids: vec!["a > b".into()],
        holdout_bundle_path: None,
        holdout_hash: None,
        holdout_ids: Vec::new(),
    };
    sink.event(phase_changed(Phase::Queued, Phase::Provisioning, None));
    sink.stage(Stage::Red, StageStatus::Started, None);
    sink.frozen(&freeze);
    sink.result(&fleet_verify::testkit::pr_open(
        fleet_verify::testkit::evidence("factory/u1", "abc", "x.bundle"),
    ));
    sink.event(Event::Done {
        result: "failed".into(),
    });
    for _ in 0..3 {
        stream.recv().await.unwrap();
    }
    let stored = store.get_record("u1").unwrap().unwrap();
    assert_eq!(stored.row.last_seq, 3);
    assert_eq!(stored.row.terminal_reason.as_deref(), Some("failed"));
    assert!(stored.ext.freeze_json.as_deref().unwrap().contains("a > b"));
    assert!(stored
        .ext
        .result_json
        .as_deref()
        .unwrap()
        .contains("pr_open"));
    assert!(store.events_since("u1", 0).unwrap()[1].contains("\"stage\""));
}

#[tokio::test]
async fn the_registry_view_answers_admission_and_the_api_from_the_stored_probe_state() {
    let registry = registry_without_a_fake();
    let view = RegistryView(registry.clone());
    assert_eq!(view.status("nobody"), HarnessStatus::Unknown);
    assert!(
        matches!(view.status(FAKE_HARNESS), HarnessStatus::Unhealthy { .. }),
        "unprobed is not healthy"
    );
    assert_eq!(view.list()[0].state, "unprobed");
    harness_directory_contract(&view);

    registry.probe(Duration::from_secs(2)).await;
    assert!(matches!(
        registry.state(FAKE_HARNESS),
        Some(ProbeState::Unhealthy { .. })
    ));
    match view.status(FAKE_HARNESS) {
        HarnessStatus::Unhealthy { error } => assert!(error.contains("no-such-harness-fake")),
        other => panic!("{other:?}"),
    }
    assert_eq!(view.list()[0].state, "unhealthy");
    harness_directory_contract(&view);
    // Eligibility of a harness with no known capabilities is a refusal, never a pass.
    assert!(view
        .eligibility(FAKE_HARNESS, Tier::T1, true, None)
        .is_err());
    assert!(view.eligibility("nobody", Tier::T1, true, None).is_err());
}

#[test]
fn the_census_reads_the_store_and_the_hub_and_a_stranded_unit_keeps_its_reason() {
    let store = store();
    let mut running = record("running");
    running.row.phase = "building".into();
    let mut waiting = harness_record("waiting", "reqdrive");
    waiting.row.phase = "needs_human".into();
    waiting.ext.pause_reason = Some("oracle tampering".into());
    let mut done = record("done");
    done.row.phase = "done".into();
    for unit in [&running, &waiting, &done] {
        store.reserve_unit(unit, 0, 1000.0, 0).unwrap();
    }
    let census = Census {
        store: store.clone(),
        hub: Hub::new(store.clone(), Arc::new(SystemClock)),
    };
    assert_eq!(census.nonterminal(), ["running", "waiting"]);
    assert_eq!(census.finished(), ["done"]);
    assert!(census.live().is_empty());

    census.stranded("running");
    census.stranded("waiting");
    assert_eq!(store.get_unit("running").unwrap().unwrap().phase, "halted");
    let kept = store.get_record("waiting").unwrap().unwrap();
    assert_eq!(kept.row.phase, "needs_human");
    assert_eq!(kept.ext.pause_reason.as_deref(), Some("oracle tampering"));
}

// --- the wired router ---

#[tokio::test]
async fn every_route_the_daemon_serves_needs_the_token_including_the_ones_the_old_path_owns() {
    let store = store();
    store.reserve_unit(&record("u1"), 0, 1000.0, 0).unwrap();
    let wired = wire(&defaults(), parts(store, registry_without_a_fake())).unwrap();
    let old_path = [
        ("GET", "/health"),
        ("POST", "/swarms"),
        ("GET", "/swarms"),
        ("GET", "/swarms/sw1"),
    ];
    for (method, path) in ROUTES.iter().chain(old_path.iter()) {
        let uri = path.replace(":id", "u1");
        let (status, _) = call(wired.secured.clone(), method, &uri, None, &[]).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{method} {path}");
        let bearer = format!("Bearer {TOKEN}");
        let (status, _) = call(
            wired.secured.clone(),
            method,
            &uri,
            None,
            &[("authorization", bearer.as_str())],
        )
        .await;
        assert_ne!(status, StatusCode::UNAUTHORIZED, "{method} {path}");
        // A swarm nobody stored is a 404 from its own handler. Every other route here has
        // something to answer with, so a 404 would mean the path is not routed at all.
        if *path != "/swarms/sw1" {
            assert_ne!(
                status,
                StatusCode::NOT_FOUND,
                "{method} {path} is not served"
            );
        }
    }
}

#[tokio::test]
async fn a_demo_mission_is_refused_with_503_while_the_scripted_harness_is_unhealthy() {
    let store = store();
    let registry = registry_without_a_fake();
    registry.probe(Duration::from_secs(2)).await;
    let wired = wire(&defaults(), parts(store.clone(), registry)).unwrap();
    let (status, body) = call(
        wired.open.clone(),
        "POST",
        "/missions",
        Some(json!({"task": "x"})),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(body.contains("no-such-harness-fake"), "{body}");
    assert!(
        store.list_records().unwrap().is_empty(),
        "a refused mission left a row"
    );
    assert!(wired.hub.live().is_empty());
}

#[tokio::test]
async fn a_real_mission_in_the_old_shape_still_goes_to_the_in_process_driver() {
    // Without a model key the old path refuses, exactly as it always has, before it stores
    // anything; that it answered at all shows the old shape is still routed to it.
    assert!(
        std::env::var_os("ANTHROPIC_API_KEY").is_none(),
        "unset ANTHROPIC_API_KEY for this test"
    );
    let store = store();
    let wired = wire(&defaults(), parts(store.clone(), registry_without_a_fake())).unwrap();
    let body = json!({"task": "x", "mode": "real"});
    let (status, text) = call(wired.open.clone(), "POST", "/missions", Some(body), &[]).await;
    assert_eq!(
        (status, text.as_str()),
        (StatusCode::BAD_REQUEST, "ANTHROPIC_API_KEY not set")
    );
    assert!(store.list_records().unwrap().is_empty());
}

#[tokio::test]
async fn a_stored_unit_is_brought_back_on_the_path_its_harness_column_names() {
    let store = store();
    // A demo unit from before milestone 1: no harness, so the in-process driver resumes it.
    let mut old_demo = record("u1");
    old_demo.row.phase = "halted".into();
    old_demo.row.oracle_frozen = true;
    // A unit whose harness is not registered here.
    let mut orphan = harness_record("u2", "not-registered");
    orphan.row.phase = "halted".into();
    for unit in [&old_demo, &orphan] {
        store.reserve_unit(unit, 0, 1000.0, 0).unwrap();
    }
    let wired = wire(&defaults(), parts(store.clone(), registry_without_a_fake())).unwrap();
    let resume = json!({"command": "resume", "cmd_id": "c1"});

    let (status, _) = call(
        wired.open.clone(),
        "POST",
        "/units/u1/commands",
        Some(resume.clone()),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED);
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while store.get_unit("u1").unwrap().unwrap().phase != "done" {
        assert!(
            std::time::Instant::now() < deadline,
            "the old demo unit never finished"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    let (status, _) = call(
        wired.open.clone(),
        "POST",
        "/units/u2/commands",
        Some(resume),
        &[],
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "a unit whose harness is gone cannot be brought back"
    );
    assert_eq!(store.get_unit("u2").unwrap().unwrap().phase, "halted");
}
````

`crates/fleetd/tests/wire_it.rs`:

````rust
//! Lane CC-WIRE, integration tier: the wired daemon driving the real `harness-fake`.
//! Supervisor spec section 6 items 7 and 8.
//!
//! Needs the `harness-fake` binary: `cargo build -p harness-conformance --bins` (a workspace
//! `cargo test` builds it). Needs no Docker and no network.

use axum::http::StatusCode;
use fleet_api::testkit::{call, TOKEN};
use fleet_forge::testkit::MemForge;
use fleet_runner::testkit::FakeRunner;
use fleet_runner::{reap_at_startup, CheckRunner};
use fleet_store::{SharedStore, Store, UnitStore};
use fleet_supervisor::{fake_binary_beside, Deps, HarnessEntry, Registry, FAKE_HARNESS};
use fleet_verify::testkit::ScriptedVerifier;
use fleet_verify::Verdict;
use fleetd::wire::{settings, wire, Census, Parts, Wired};
use serde_json::{json, Value};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

async fn probed(env: &[(&str, &str)]) -> Registry {
    let binary = fake_binary_beside(&std::env::current_exe().unwrap());
    assert!(
        binary.is_file(),
        "{} not found; run: cargo build -p harness-conformance --bins",
        binary.display()
    );
    let registry = Registry::new(vec![HarnessEntry {
        name: FAKE_HARNESS.into(),
        command: vec![binary.to_string_lossy().into_owned()],
        env: env
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
    }]);
    registry.probe(Duration::from_secs(10)).await;
    registry
}

struct Daemon {
    store: Arc<SharedStore>,
    wired: Wired,
    demo_verifier: Arc<ScriptedVerifier>,
    runner: Arc<FakeRunner>,
}

impl Daemon {
    async fn over(store: Arc<SharedStore>, env: &[(&str, &str)]) -> Daemon {
        let token = |key: &str| (key == "FLEETD_TOKEN").then(|| TOKEN.to_string());
        let settings = settings(None, &token).unwrap();
        let forge = Arc::new(MemForge::new());
        // harness-fake reports a fixed head for every unit; the scripted verifier imports nothing.
        for n in 1..=9 {
            forge.mark_imported(&format!("u{n}"), "0123456789abcdef0123456789abcdef01234567");
        }
        let demo_verifier = Arc::new(ScriptedVerifier::new());
        let runner = Arc::new(FakeRunner::new());
        let parts = Parts {
            store: store.clone(),
            forge: forge.clone(),
            runner: runner.clone(),
            verifier: Arc::new(ScriptedVerifier::new()),
            registry: probed(env).await,
            demo: Deps {
                verifier: demo_verifier.clone(),
                forge,
            },
        };
        Daemon {
            store,
            wired: wire(&settings, parts).unwrap(),
            demo_verifier,
            runner,
        }
    }

    async fn new(env: &[(&str, &str)]) -> Daemon {
        Daemon::over(Arc::new(Mutex::new(Store::open_memory().unwrap())), env).await
    }

    async fn post(&self, uri: &str, body: Value) -> (StatusCode, String) {
        call(self.wired.open.clone(), "POST", uri, Some(body), &[]).await
    }

    async fn mission(&self, body: Value) -> String {
        let (status, text) = self.post("/missions", body).await;
        assert_eq!(status, StatusCode::OK, "{text}");
        serde_json::from_str::<Value>(&text).unwrap()["unit_id"]
            .as_str()
            .unwrap()
            .to_string()
    }

    fn phase(&self, unit: &str) -> String {
        self.store
            .get_unit(unit)
            .unwrap()
            .map(|r| r.phase)
            .unwrap_or_default()
    }

    fn events(&self, unit: &str) -> Vec<Value> {
        self.store
            .events_since(unit, 0)
            .unwrap()
            .iter()
            .map(|j| serde_json::from_str(j).unwrap())
            .collect()
    }

    async fn until(&self, what: &str, done: impl Fn(&Daemon) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(30);
        while !done(self) {
            assert!(Instant::now() < deadline, "timed out waiting until {what}");
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }
}

#[tokio::test]
async fn a_demo_mission_runs_to_done_through_the_real_scripted_harness() {
    let d = Daemon::new(&[]).await;
    let id = d
        .mission(json!({"task": "x", "min_review_rounds": 1}))
        .await;
    d.until("the unit is done", |d| d.phase(&id) == "done")
        .await;

    let stored = d.store.get_record(&id).unwrap().unwrap();
    assert_eq!(stored.ext.harness.as_deref(), Some("fake"));
    assert_eq!(stored.row.mode, "demo");
    assert_eq!(stored.row.terminal_reason.as_deref(), Some("done"));
    assert!(stored.ext.freeze_json.is_some(), "the freeze was kept");
    let events = d.events(&id);
    let seqs: Vec<u64> = events.iter().map(|e| e["seq"].as_u64().unwrap()).collect();
    assert_eq!(
        seqs,
        (1..=events.len() as u64).collect::<Vec<_>>(),
        "the log is contiguous"
    );
    assert!(
        events.iter().any(|e| e["event"]["type"] == "stage"),
        "stage notes are logged"
    );
    assert!(events
        .iter()
        .any(|e| e["event"]["type"] == "artifact" && e["event"]["ref"] == "https://fake/pr/1"));
    assert_eq!(d.demo_verifier.inputs().len(), 1);
}

#[tokio::test]
async fn three_units_waiting_at_their_gates_leave_room_for_a_fourth() {
    let d = Daemon::new(&[]).await;
    for n in 1..=3 {
        let id = d
            .mission(json!({"task": "x", "tier": "t2", "min_review_rounds": 1}))
            .await;
        assert_eq!(id, format!("u{n}"));
        d.until("the T2 unit is at its gate", |d| {
            d.phase(&id) == "awaiting_oracle_approval"
        })
        .await;
    }
    // Three units at once is the default limit, and all three are parked: none holds a slot.
    let fourth = d
        .mission(json!({"task": "x", "min_review_rounds": 1}))
        .await;
    d.until("the fourth unit is done", |d| d.phase(&fourth) == "done")
        .await;
    assert!(!d
        .events(&fourth)
        .iter()
        .any(|e| e["event"]["type"] == "blocked"));

    let approve = json!({"command": "approve_oracle", "cmd_id": "a1"});
    assert_eq!(
        d.post("/units/u1/commands", approve).await.0,
        StatusCode::ACCEPTED
    );
    d.until("u1 is done", |d| d.phase("u1") == "done").await;
    assert_eq!(d.phase("u2"), "awaiting_oracle_approval");
    let approved = d
        .store
        .get_record("u1")
        .unwrap()
        .unwrap()
        .ext
        .approved_oracle_hash;
    assert!(approved.is_some(), "the approval was folded into the row");
}

#[tokio::test]
async fn a_mission_whose_reservation_cannot_be_recorded_is_refused_and_starts_nothing() {
    // Issue 74, on the path the old request shape takes.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("fleet.db");
    let store: Arc<SharedStore> = Arc::new(Mutex::new(Store::open(&path).unwrap()));
    let d = Daemon::over(store, &[]).await;
    let side = rusqlite::Connection::open(&path).unwrap();
    side.execute_batch(
        "CREATE TRIGGER refuse_unit_rows BEFORE INSERT ON units
             BEGIN SELECT RAISE(ABORT, 'insert refused'); END;",
    )
    .unwrap();
    let (status, _) = d.post("/missions", json!({"task": "x"})).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(d.events("u1").is_empty(), "a unit with no reservation ran");
    assert!(d.wired.hub.live().is_empty());
}

#[tokio::test]
async fn a_unit_that_needs_a_human_still_needs_one_for_the_same_reason_after_a_restart() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("fleet.db");
    let open =
        |path: &Path| -> Arc<SharedStore> { Arc::new(Mutex::new(Store::open(path).unwrap())) };
    let first = Daemon::over(open(&path), &[("HARNESS_FAKE_MODE", "needs_human")]).await;
    let id = first.mission(json!({"task": "x"})).await;
    first
        .until("the harness stopped for a human", |d| {
            d.phase(&id) == "needs_human"
        })
        .await;
    let before = first.store.get_record(&id).unwrap().unwrap();
    let reason = before.ext.pause_reason.clone().expect("a stop reason");
    drop(first);

    // A new daemon on the same database, as after a crash: the startup pass runs first.
    let second = Daemon::over(open(&path), &[]).await;
    second.runner.add_container(&id, "cc_leftover");
    let census = Census {
        store: second.store.clone(),
        hub: second.wired.hub.clone(),
    };
    let report = reap_at_startup(second.runner.as_ref(), &census).await;
    assert_eq!(report.reaped, std::slice::from_ref(&id));
    assert!(second.runner.unit_containers().await.unwrap().is_empty());

    let after = second.store.get_record(&id).unwrap().unwrap();
    assert_eq!(
        after.row.phase, "needs_human",
        "a restart rewrote needs_human"
    );
    assert_eq!(after.ext.pause_reason.as_deref(), Some(reason.as_str()));
    assert_eq!(
        after.row.last_seq, before.row.last_seq,
        "a restart added an event to a paused unit"
    );

    // It can be resumed: this harness finishes the unit.
    let resume = json!({"command": "resume", "cmd_id": "r1"});
    assert_eq!(
        second
            .post(&format!("/units/{id}/commands"), resume)
            .await
            .0,
        StatusCode::ACCEPTED
    );
    second
        .until("the resumed unit is done", |d| d.phase(&id) == "done")
        .await;
}

#[tokio::test]
async fn tampering_found_at_verification_pauses_a_demo_unit_and_clears_its_approval() {
    let d = Daemon::new(&[]).await;
    d.demo_verifier.push(Verdict::Tampered {
        detail: "a frozen test changed".into(),
    });
    let id = d
        .mission(json!({"task": "x", "tier": "t2", "min_review_rounds": 1}))
        .await;
    d.until("the unit is at its gate", |d| {
        d.phase(&id) == "awaiting_oracle_approval"
    })
    .await;
    let approve = json!({"command": "approve_oracle", "cmd_id": "a1"});
    d.post(&format!("/units/{id}/commands"), approve).await;
    d.until("the unit is paused", |d| d.phase(&id) == "needs_human")
        .await;

    let stored = d.store.get_record(&id).unwrap().unwrap();
    assert_eq!(stored.ext.pause_reason.as_deref(), Some("oracle tampering"));
    assert_eq!(stored.ext.approved_oracle_hash, None);
    assert_eq!(
        d.demo_verifier.inputs()[0].approved_oracle_hash.as_deref(),
        stored.row.oracle_hash.as_deref(),
        "the stored approval reached the verifier"
    );
    // It cannot be resumed; it can be abandoned.
    let resume = json!({"command": "resume", "cmd_id": "r1"});
    d.post(&format!("/units/{id}/commands"), resume).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(d.phase(&id), "needs_human");
    let abandon = json!({"command": "abandon", "cmd_id": "x1"});
    d.post(&format!("/units/{id}/commands"), abandon).await;
    d.until("the unit is failed", |d| d.phase(&id) == "failed")
        .await;
}
````

### Characterisation tests this lane changes on purpose

Three files, five tests. Each change is a decision of the design, made here and nowhere else;
every other characterisation test must pass unchanged, and one that does not is a finding to
report, not a test to edit.

**`crates/fleetd/tests/characterisation_admission.rs`.** A unit now gives its slot back while
its oracle gate is pending. Replace
`a_fourth_unit_waits_for_a_slot_while_three_wait_at_the_oracle_gate`, with its comment, by:

````rust
/// A unit gives its concurrency slot back while it waits for a person to approve its
/// oracle, so three parked units leave room for a fourth. Until milestone 1 a parked
/// unit kept its slot and this test pinned the fourth unit waiting; the factory design
/// changed that on purpose.
#[tokio::test]
async fn three_units_waiting_at_the_oracle_gate_leave_room_for_a_fourth() {
    let d = Daemon::new();
    for n in 1..=3 {
        let (status, body) = d.post_mission("t2").await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let unit = format!("u{n}");
        d.until("the T2 unit is at its oracle gate", |d| {
            d.phase(&unit).as_deref() == Some("awaiting_oracle_approval")
        })
        .await;
    }

    // The fourth is admitted (3 x $5 reserved is under $20) and runs straight through.
    let (status, body) = d.post_mission("t1").await;
    assert_eq!(
        (status, body.as_str()),
        (StatusCode::OK, r#"{"unit_id":"u4"}"#)
    );
    d.until("u4 is done", |d| d.phase("u4").as_deref() == Some("done"))
        .await;
    assert!(
        !d.events("u4")
            .iter()
            .any(|e| e["event"]["type"] == "blocked"),
        "u4 waited for a slot that a parked unit should not hold"
    );

    // Approving one oracle lets that unit finish; the other two keep waiting.
    let approve = json!({"command": "approve_oracle", "cmd_id": "a1"});
    let (status, _) = d.send("POST", "/units/u1/commands", Some(approve)).await;
    assert_eq!(status, StatusCode::ACCEPTED);
    d.until("u1 is done", |d| d.phase("u1").as_deref() == Some("done"))
        .await;
    assert_eq!(d.phase("u2").as_deref(), Some("awaiting_oracle_approval"));
    assert_eq!(d.phase("u3").as_deref(), Some("awaiting_oracle_approval"));
}
````

Issue #74 is fixed. Replace `defect_74_a_mission_is_admitted_when_its_reservation_cannot_be_recorded`
and `defect_74_a_failed_spend_read_is_treated_as_nothing_spent`, with their comments, by:

````rust
/// Issue #74, fixed in milestone 1. A mission whose reservation cannot be recorded is
/// refused, and nothing runs for it. (Until the fix this test pinned the defect: the
/// mission was admitted and its driver ran with no row behind it.)
#[tokio::test]
async fn a_mission_is_refused_when_its_reservation_cannot_be_recorded() {
    let (store, side) = store_with_a_side_door("issue-74-insert");
    let d = Daemon::over(store);
    side.execute_batch(
        "CREATE TRIGGER refuse_unit_rows BEFORE INSERT ON units
             BEGIN SELECT RAISE(ABORT, 'characterisation: unit insert refused'); END;",
    )
    .unwrap();

    for n in 1..=6 {
        let (status, body) = d.post_mission("t1").await;
        assert_eq!(
            (status, body.as_str()),
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "the reservation could not be recorded"
            ),
            "mission {n}"
        );
    }

    assert!(d.unit_ids().is_empty(), "a refused mission left a row");
    let (status, _) = d.send("GET", "/units/u1", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // Nothing ran: give a run that should not exist time to log something.
    tokio::time::sleep(Duration::from_millis(300)).await;
    for n in 1..=6 {
        assert!(
            d.events(&format!("u{n}")).is_empty(),
            "u{n} ran with no reservation behind it"
        );
    }
}

/// Issue #74, fixed in milestone 1. A failed read of committed spend refuses the
/// mission: it is never taken to mean that nothing was spent.
#[tokio::test]
async fn a_mission_is_refused_when_committed_spend_cannot_be_read() {
    let (store, side) = store_with_a_side_door("issue-74-read");
    let d = Daemon::over(store);
    d.seed(&reservation("seed", 999.0), now_ms());
    assert_eq!(
        d.post_mission("t1").await.0,
        StatusCode::TOO_MANY_REQUESTS,
        "with a working read, $999 committed refuses the mission"
    );

    // The spend query reads three tables; without this one it returns an error.
    side.execute_batch("DROP TABLE swarms;").unwrap();
    assert!(d.store.lock().unwrap().committed_spend(0).is_err());

    let (status, body) = d.post_mission("t1").await;
    assert_eq!(
        (status, body.as_str()),
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "the reservation could not be recorded"
        )
    );
    assert_eq!(d.unit_ids(), ["seed"]);
}
````

In the same file the comment of
`one_cent_under_the_ceiling_a_mission_is_admitted_and_the_next_is_refused` says a finished
unit counts `$0.09`; a one-round demo unit now costs `$0.02`. Change the figure in the comment.
The assertion holds either way (19.99 + 0.02 is above 20) and is not changed.

**`crates/fleetd/tests/characterisation_reconcile.rs`.** A restart no longer rewrites a unit
that waits for a person. Replace
`units_waiting_on_a_person_are_rewritten_to_halted_at_every_start`, with its comment, by:

````rust
/// A unit that waits for a person is left exactly as it was, at every start: its phase,
/// its reason and its log. A unit at its oracle gate has lost the process that was
/// waiting with it, so it is halted, once. Until milestone 1 both were rewritten to
/// `halted` at every start and the reason survived only in the log; the factory design
/// changed that on purpose.
#[tokio::test]
async fn a_unit_waiting_on_a_person_is_left_as_it_was_at_every_start() {
    let mut parked = stored("u1", "needs_human");
    parked.terminal_reason = Some("cap breach".into());
    let store = store_with(&[parked, stored("u2", "awaiting_oracle_approval")]);

    reconcile_on_startup(&AppState::new(store.clone()), &FakeRunner::new(vec![])).await;

    let u1 = row(&store, "u1");
    assert_eq!(u1.phase, "needs_human");
    assert_eq!(u1.terminal_reason.as_deref(), Some("cap breach"));
    assert_eq!(u1.last_seq, 7);
    assert!(
        events(&store, "u1").is_empty(),
        "a restart logged an event for a unit that waits for a person"
    );
    assert_eq!(row(&store, "u2").phase, "halted");
    assert_eq!(
        events(&store, "u2")[0]["event"],
        json!({
            "type": "phase_changed", "from": "awaiting_oracle_approval", "to": "halted",
            "reason": "daemon restarted"
        })
    );

    // A second start changes nothing more: a halted unit is not halted again.
    reconcile_on_startup(&AppState::new(store.clone()), &FakeRunner::new(vec![])).await;

    assert!(events(&store, "u1").is_empty());
    assert_eq!(row(&store, "u1").phase, "needs_human");
    assert_eq!(events(&store, "u2").len(), 1);
    assert_eq!(row(&store, "u2").last_seq, 8);
}
````

**`crates/fleetd/tests/characterisation_missions.rs`.** In
`a_finished_unit_s_row_is_the_fold_of_its_event_log`, a demo unit now runs on the scripted
harness: at T1 nothing proposes an oracle to a person, and the cost is the harness's. Replace
the end of the function, from `assert_eq!(row.last_seq, events.len() as u64);` to its closing
brace, by:

````rust
    assert_eq!(row.last_seq, events.len() as u64);
    assert_eq!(row.phase, last("phase_changed")["to"]);
    assert_eq!(row.cost, last("metric")["cost_usd"].as_f64().unwrap());
    assert_eq!(
        row.terminal_reason.as_deref(),
        last("done")["result"].as_str()
    );
    // A T1 unit has no oracle gate, so no proposal is logged and no hash is stored. The
    // oracle is frozen all the same.
    assert!(!events
        .iter()
        .any(|e| e["event"]["type"] == "oracle_proposed"));
    assert_eq!(row.oracle_hash, None);
    assert!(row.oracle_frozen);

    // And the values themselves, for a default (two review rounds) demo mission: the
    // scripted harness meters $0.02 for each build.
    assert_eq!(row.phase, "done");
    assert_eq!(row.terminal_reason.as_deref(), Some("done"));
    assert!((row.cost - 0.04).abs() < 1e-9, "cost was {}", row.cost);
}
````

The first half of that function (the log is contiguous; every entry is an envelope) is not
changed, and holds for stage notes too. Each replacement was compiled in the scratch workspace
against the file it goes into.

### Implementation notes

**What each file becomes**

| File | After this lane |
|---|---|
| `store.rs` | `pub use fleet_store::{LaneRow, SharedStore, Store, StoreError, SwarmRow, UnitRow};` and nothing else. Its code and its tests moved in lane CC-STORE |
| `forge.rs` | `pub use fleet_forge::{Forge, ForgeError, MergeResult, Mergeability};` |
| `gh_forge.rs` | `pub use fleet_forge::GhForge;` |
| `reconcile.rs` | `pub use fleet_runner::{reconcile, reconcile_live, Action};` |
| `wire.rs` | the settings, `Parts::real`, `wire`, `serve`, the three adapters, and the launcher |
| `server.rs` | what the driver path still owns: the swarm routes and `run_swarm`, `/health`, the driver half of `spawn_driver_for` and `rehydrate`, `demo_script`, `spec_from_row`; and the four names the tests and the binaries use (`AppState`, `router`, `reconcile_on_startup`, `reconcile_tick`), kept with their signatures |
| `bin/serve.rs` | read the configuration file and the environment with `settings`, then `Parts::real`, then `wire::serve`. A settings or wiring error is printed and the process exits non-zero |

Deleted from `server.rs` because they moved: `allowed_origins` and `cors_layer` (lines 153 to
188, now `fleet_api::secured`), the route table's six unit routes, `create_mission`'s handler
shell, `UnitHandle` and the `units` table (now `fleet_api::Hub`), `register_unit_if_absent`,
`spawn_forwarder`, the read handlers, `post_command`, `ws_stream` and `stream_to_socket`,
`seq_of`, and `halt_in_store` (now `UnitStore::mark_restarted`).

**`AppState` and `router` stay because five test files and two binaries name them.** They
become a view of the wiring. `wire` is written as two steps: a private `build(limits, parts)`
that makes the hub, the admission, the launcher and the open router, and
`fleet_api::secured(open, token, origins)` on top. `AppState::new(store)` calls `build` with
the same store, today's limits read from the environment, `Registry::default_fake()` (not
probed), an empty repository allowlist, and the demo parts in every position;
`server::router(state)` returns that open router. It has no token because a test router is
not a listener: the only thing the program binds is `Wired::secured`. The empty allowlist is
what keeps a test-built `AppState` from ever admitting a unit from a signed spec onto the demo
doubles.

`settings` splits the same way: one private function reads everything but the token and the
address (and is what `AppState::new` uses); `settings` adds those two and their checks.

**The demo parts.** The scripted harness reports a fixed commit that exists nowhere, and
`ScriptedVerifier` imports nothing, so the in-memory forge would refuse to open the pull
request. `Parts::real` therefore builds `demo` as `ScriptedVerifier::new()` and this adapter
over one `MemForge` (compiled and run in the scratch workspace):

````rust
use async_trait::async_trait;
use fleet_forge::testkit::MemForge;
use fleet_forge::{ForgeError, PrRef, PrRequest, PrState, RepoForge, RepoRef, Scratch};
use std::path::Path;
use std::sync::Arc;

/// The demo path's forge. The scripted harness's evidence names a commit that exists nowhere,
/// and the scripted verifier imports nothing; the in-memory forge, rightly, opens a pull
/// request only for a commit it imported. This tells it the demo unit's delivery was imported
/// at the commit its result names, and passes everything else through.
pub struct DemoForge(pub Arc<MemForge>);

#[async_trait]
impl RepoForge for DemoForge {
    async fn resolve_base(&self, repo: &RepoRef) -> Result<String, ForgeError> {
        self.0.resolve_base(repo).await
    }

    async fn read_file(
        &self,
        repo: &RepoRef,
        commit: &str,
        path: &str,
    ) -> Result<Option<Vec<u8>>, ForgeError> {
        self.0.read_file(repo, commit, path).await
    }

    async fn list_files(
        &self,
        repo: &RepoRef,
        commit: &str,
        prefix: &str,
    ) -> Result<Vec<String>, ForgeError> {
        self.0.list_files(repo, commit, prefix).await
    }

    async fn source_bundle(
        &self,
        repo: &RepoRef,
        base_sha: &str,
        dest: &Path,
    ) -> Result<(), ForgeError> {
        self.0.source_bundle(repo, base_sha, dest).await
    }

    async fn import_bundle(
        &self,
        repo: &RepoRef,
        unit_id: &str,
        base_sha: &str,
        bundle: &Path,
        branch: &str,
    ) -> Result<Box<dyn Scratch>, ForgeError> {
        self.0
            .import_bundle(repo, unit_id, base_sha, bundle, branch)
            .await
    }

    async fn open_pr(&self, request: &PrRequest) -> Result<PrRef, ForgeError> {
        self.0.mark_imported(&request.unit_id, &request.head_sha);
        self.0.open_pr(request).await
    }

    async fn pr_state(&self, repo: &RepoRef, pr_url: &str) -> Result<PrState, ForgeError> {
        self.0.pr_state(repo, pr_url).await
    }
}
````

The tests build `Parts::demo` themselves, with a plain `MemForge`, and mark the imports.

**`Parts::real`.** The store is `Store::open(&settings.db)` in a `Mutex` in an `Arc`. The
forge is `GitForge::new(data_dir/forge, GhCli)` or, for `PrHostChoice::Local { dir }`,
`GitForge::new(data_dir/forge, LocalPrHost::new(dir))`. The runner is `DockerRunner::new()`.
The verifier is `Verifier::new(forge, runner, VerifyConfig { work_dir: data_dir/verify,
check_timeout: 600 s, setup_timeout: 900 s })`. The registry is `Registry::default_fake()`,
merged with `Registry::load(file)` when `registry_file` is set: a file entry of the same name
wins. A store that does not open is `WireError::Store`; a registry file that does not read is
`WireError::Registry`. Nothing here creates a directory that a later step does not need:
`data_dir` and its children are created on first use.

**`serve`**, in this order, each step finishing before the next starts:

1. `registry.probe(settings.grace)`; print one line per harness with its state.
2. `fleet_runner::reap_at_startup(runner, &Census { … })`; print the report's counts and each
   error. This one pass covers both paths' containers: the driver path's carry the same label.
3. The swarm half of today's `reconcile_on_startup` (lines 777 to 806), kept in `server.rs` as
   its own function.
4. When `reconcile_secs` is above 0, a task that calls `fleet_runner::reap_tick` on that
   interval, skipping the first tick as today.
5. Bind `settings.addr`; a failure is `WireError::Bind`.
6. `axum::serve(listener, wired.secured)`.

Nothing is printed that contains the token. `reconcile_on_startup` and `reconcile_tick` stay
in `server.rs` for the old `Runner` trait, because `characterisation_reconcile.rs` calls them
with `fleetd::fake::FakeRunner`; their bodies are today's (lines 756 to 836) with the unit
table replaced by `Hub::live` and `halt_in_store` by `mark_restarted`. `serve` does not call
them.

**The settings.**

| Key in the file | Environment override | Default | Refused when |
|---|---|---|---|
| | `FLEETD_TOKEN` | none | missing or weak: `Auth` |
| | `CC_ADDR` | `127.0.0.1:8787` | not loopback: `Auth` |
| | `CC_DB` | `fleet.db` | |
| | `FLEETD_DATA_DIR` | `fleetd-data` | |
| `caps.unit_usd` | | 5.0 | not above 0: `Invalid` |
| `caps.unit_wall_clock_secs` | | 1800 | 0: `Invalid` |
| `caps.global_usd` | `CC_GLOBAL_USD_CAP` | 20.0 | below `unit_usd`: `Invalid` |
| `caps.max_concurrent` | `CC_MAX_CONCURRENT` | 3 | 0: `Invalid` |
| `[[repo]]` `slug`, `url`, `base_branch` | | the sandbox repository, `main` | a slug named twice, or a base branch `fleet_forge::safe_branch` refuses: `Invalid { key: "repo" }` |
| | `FLEETD_HARNESS_GRACE_SECS`, `FLEETD_HARNESS_STALL_SECS` | 30, 600 | |
| | `CC_RECONCILE_SECS` | 30 | |
| | `FLEETD_REGISTRY` | none | |
| | `FLEETD_PR_HOST` | `github` | anything but `github` or `local:<dir>`: `Invalid` |
| | `FLEETD_ALLOWED_ORIGINS` (comma-separated) | today's list (server.rs lines 153 to 180) | |
| | `FLEETD_OPERATOR` | `USERNAME`, else `USER`, else `operator` | |
| | `CC_IMAGE` | `cc-agent:dev` | |

An `Invalid` names the file key (`caps.unit_usd`) or the environment variable whose value did
not parse. The file is read with `deny_unknown_fields` at both levels; anything `toml` refuses
is `Malformed`. A `[[repo]]` table in the file replaces the default allowlist; it does not add
to it. `admission.presets` is the sorted names of the presets `factory-presets` links,
`presets_version` is `factory_presets::PRESETS_VERSION`, `spend_window_ms` is 24 hours, and
`min_review_rounds` is 2. Environment values are read through the `env` argument only, never
through `std::env` directly, which is what lets the tests run in parallel.

**The adapters.** `FeedSink` calls the `UnitFeed` method of the same name. `RegistryView`:
`status` is `Unknown` for a name with no entry, `Healthy` with the probed capabilities, and
`Unhealthy` otherwise (an unprobed entry reports `not probed yet`: admission needs
capabilities and has none); `eligibility` is `fleet_supervisor::eligibility` on the probed
capabilities, and an `Err` when there are none; `list` is one `HarnessView` per entry in
registry order with the state as `unprobed`, `healthy` or `unhealthy`. `Census`: `nonterminal`
and `finished` split `list_records` by `fleet_core::TERMINAL_PHASE_STRS`, in stored order;
`live` is `Hub::live`; `stranded` is `mark_restarted` with the present time, its result
ignored but an error printed.

**The driver path's events.** `driver::run` numbers its own envelopes; the hub numbers every
record. Give the driver a channel as today and forward each envelope's `event` to
`UnitFeed::event`, dropping the driver's number. The two agree anyway (both continue from the
stored `last_seq`), and the hub's is the one that is stored.

**Pitfalls.** `Launcher` methods are not `async` and are called on the runtime: they may
`tokio::spawn`, and must not block on the forge. Take the store's lock for one call at a time;
never hold it across `Hub::register` or a spawn. A demo unit's `work_dir` exists only if the
harness asks for one; create nothing for it up front. On Windows a test binary lives in
`target\<profile>\deps`, which is why `Registry::default_fake` looks one directory up; do not
"fix" the path in `AppState::new`. `characterisation_admission.rs` asserts that
`CC_GLOBAL_USD_CAP` and `CC_MAX_CONCURRENT` are unset because `AppState::new` reads them:
keep reading them there.

**The unit tests inside `server.rs`** (its `mod tests`, the last thousand lines) are this lane's to
carry. What happens to each:

| Test | After |
|---|---|
| `fold_persists_oracle_hash_and_freezes_on_oracle_proposed`, `ws_stream_delivers_the_unit_event_log_to_a_client` | Deleted: the fold is `fleet-store`'s and the stream is `fleet-api`'s, each tested there (lane CC-STORE's projection tests; lane CC-API's behaviour 21) |
| `register_unit_if_absent_is_atomic_under_races` | Kept, calling `Hub::register` from the racing tasks: exactly one gets channels |
| the three CORS tests (`preflight_on_a_command_route_is_answered_not_405`, `an_allowed_origin_gets_the_header_on_a_real_response`, `an_unlisted_origin_gets_no_allow_origin_header`) | Kept, sent through `Wired::secured` with a bearer header: CORS is no longer on the open router |
| `next_id_seeds_above_persisted_units` | Kept: a mission after a stored `u41` is `u42` |
| `rehydrate_reloads_oracle_hash_so_tamper_gate_rearms`, `rehydrate_demo_unit_resumes_as_demo_to_done`, `rehydrate_skips_units_absent_from_the_store` | Kept unchanged in what they assert: a stored demo row has no harness and is the driver path's. (The supervisor spec rewrites the first; that followed from its backfill, which is not run. Behaviour 18 is its counterpart on the new path) |
| every other test (the reconcile, cap, swarm and `demo_script` tests) | Kept; where one reached into the `units` table it asks the hub |

### Verify

| Command | Expected |
|---|---|
| `cargo build -p harness-conformance --bins` | exit 0 |
| `cargo test -p fleetd --test wire --test wire_it` | 13 and 5 passed |
| `cargo test -p fleetd --test characterisation_store --test characterisation_forge` | 11 and 8 passed, both files unchanged |
| `cargo test -p fleetd --test characterisation_admission --test characterisation_reconcile --test characterisation_missions` | 7, 5 and 9 passed, with exactly the five replacements above |
| `git diff origin/factory/m1...HEAD --stat -- crates/fleetd/tests/characterisation_store.rs crates/fleetd/tests/characterisation_forge.rs crates/fleetd/tests/demo_mode_it.rs` | no output |
| `cargo test -p fleetd --lib` | every remaining unit test passes; the two deleted ones are named in the pull request |
| `cargo xtask test static --root-only`, `unit`, `contract`, `integration` | exit 0 |
| *Git-Bash:* `grep -rn "unimplemented!" crates/fleetd/src/wire.rs` | no output |
| *Git-Bash:* `grep -n "command-center-agent-sandbox\|node --test\|cc-agent:dev" crates/fleetd/src/wire.rs` | only the lines that build the default `Settings` and the demo unit's `UnitSpec` |
| *PowerShell:* `$env:FLEETD_TOKEN = $null; cargo run -p fleetd --bin serve; "EXIT=$LASTEXITCODE"` | the message of `AuthError::Missing` and `EXIT=1`: no address is bound |
| *PowerShell:* `$env:FLEETD_TOKEN = 'verify-token-0123456789abcdef0123456789'; $env:CC_ADDR = '0.0.0.0:8787'; cargo run -p fleetd --bin serve; "EXIT=$LASTEXITCODE"` | the message of `AuthError::NotLoopback` and `EXIT=1` |
| `git diff --name-only origin/factory/m1...HEAD` | only paths under **Owns** |

### Done when

- [ ] Behaviours 1 to 18 pass, and every characterisation test passes with only the five
      replacements listed.
- [ ] No `unimplemented!` is left in `crates/fleetd/src/wire.rs`.
- [ ] The daemon refuses to start without a token and on a public address (the two commands
      above, with their real output in the pull request).
- [ ] The pull request closes #74, #85 and #90, names the registry rows it wants, and lists
      the two deleted unit tests and where each one's replacement lives.
- [ ] The pull request says that the daemon's half of #91 is done and that the issue closes
      with the cockpit's half, and tells the coordinator that the cockpit on `factory/m1`
      cannot reach the daemon until it sends the token.

---

## Lane CC-E2E

**Owns:** `tests/e2e/src/lib.rs` (the bodies), `tests/e2e/src/bin/harness-scripted.rs` (new),
`tests/e2e/tests/**`, `tests/e2e/fixtures/**` (new), `tests/e2e/reqdrive.pin` (new),
`tests/e2e/README.md` (new: how to run the suite on a workstation). It does not change
`SCENARIOS`, the `Scenario`, `Fixture`, `Script` and `HarnessChoice` types, or any signature of
`Factory`. It sets no `frozen` flag to true: that is the owner's, after the watched run of
Part 3.

**Reads:** the `reqdrive` repository's milestone-1 plan (how `reqdrive harness` is started,
what its scripted agent reads, which label it puts on containers);
`crates/harness-protocol/README.md`; `crates/harness-conformance/src/bin/harness-fake.rs` (the
conversation the scripted harness copies); `docs/repo-config.md`; lane CC-WIRE's settings
table; lane CC-FORGE's note on `LocalPrHost`.

**Worktree and branch:** `D:\MajorProjects\.swarm-wt\m1-cc-e2e`, branch `feat/m1-fleet-e2e`.

**Needs:** Part 1 merged, to start Tasks 1 to 3. Lane CC-WIRE merged, for any scenario to get
past `Factory::start`. Docker running. The pinned `reqdrive` binary, for scenarios 1 and 7
only.

**Blocks:** Part 3.

### What this suite is

`fleet-e2e` is test support, not a seam, so it has no Form. It has one rule that the rest of
the program relies on: a scenario asserts only what a person at the cockpit could observe
(HTTP answers, the event log, the remote repository, the recorded pull requests, the
containers on the host). It links `fleet-api` for its types and nothing else of the daemon
(Global Constraints), and starts the daemon as a program.

Every scenario is listed in `SCENARIOS` with `frozen: false`. Unfrozen, a scenario that fails
prints `UNFROZEN-FAIL <name>: <why>` and its test passes, so the `e2e` tier reports and does
not gate. The owner sets a flag to true after watching that behaviour in the functional smoke;
from then on the scenario gates.

### Tasks before the scenarios

**Task 1: pin `reqdrive` and reconcile assumption A5.**

- [ ] Read the `reqdrive` milestone-1 plan for: the subcommand and flags that start the
      harness on stdio; how its agent is made scripted and what file the script is; the
      container label; whether it declares `resume`.
- [ ] Write the release tag into `tests/e2e/reqdrive.pin`, one line, no trailing space.
- [ ] In `Factory::start`, build the registry entry for `HarnessChoice::Reqdrive` from
      `REQDRIVE_BIN` and those flags. With `REQDRIVE_BIN` unset, `start` returns
      `Err("REQDRIVE_BIN is not set: scenarios on the real harness need the reqdrive binary at the tag in tests/e2e/reqdrive.pin")`.
- [ ] If the label is not `cc.unit_id`, or `reqdrive` cannot run a scripted agent that writes
      the two files below, stop and report: A4 or A5 does not hold and the coordinator decides
      which side changes.

**Task 2: the fixture repository.** Create these files under `tests/e2e/fixtures/node/`,
exactly. `Factory::start` copies the directory, commits it as one commit on `main`, and clones
that into the bare repository the daemon is pointed at.

`package.json`:

```json
{
  "name": "factory-e2e-node",
  "version": "0.0.0",
  "private": true,
  "type": "module",
  "scripts": {
    "build": "node --check src/cart.js",
    "test": "node --test --test-reporter=./tools/junit-with-files.mjs --test-reporter-destination=reports/junit.xml",
    "format:check": "node --check src/cart.js",
    "lint": "node --check src/cart.js"
  }
}
```

`package-lock.json`:

```json
{
  "name": "factory-e2e-node",
  "version": "0.0.0",
  "lockfileVersion": 3,
  "requires": true,
  "packages": {
    "": {
      "name": "factory-e2e-node",
      "version": "0.0.0"
    }
  }
}
```

`src/cart.js`:

```js
export function total(prices) {
  return prices.reduce((sum, price) => sum + price, 0);
}
```

`test/cart.test.js`:

```js
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { total } from '../src/cart.js';

describe('cart', () => {
  it('totals', () => {
    assert.equal(total([40, 60]), 100);
  });
});
```

`.reqdrive/config.toml`, where `<digest>` is the 64 hex digits printed by
`docker buildx imagetools inspect docker.io/library/node:22-bookworm-slim --format "{{.Manifest.Digest}}"`
on the day the lane runs (record the command's output in the pull request):

```toml
preset = "node"
image = "docker.io/library/node@sha256:<digest>"
test_report = "reports/junit.xml"
test_dirs = ["test/"]
manifests = ["package.json"]
lockfiles = ["package-lock.json"]

[commands]
setup = "npm ci --ignore-scripts"
build = "npm run build"
test = "npm test"
format_check = "npm run format:check"
lint = "npm run lint"
```

`.reqdrive/specs/SPEC-0001.md`:

```markdown
---
id: "SPEC-0001"
work_item: "e2e/node#1"
kind: feature
tier: t1
touched_files: ["src/cart.js"]
create_paths: []
test_paths: ["test/"]
touched_tests: []
permitted_dependencies: []
expected_red: []
forms_affected: []
source_hash: "0000000000000000000000000000000000000000000000000000000000000000"
template_version: "1"
---

## Intent

A cart accepts one percent discount code.

## Invariants

- A total is never negative.

## Acceptance criteria

### AC-1

A cart of 100 with the code TEN totals 90.

Oracle: a unit test named for AC-1 checks it.

## Open questions
```

`.gitattributes`, so that no checkout rewrites a line ending and changes a hash:

```
* -text
```

`.gitignore`:

```
node_modules/
reports/*
!reports/.gitkeep
```

`reports/.gitkeep`, an empty file: `node` does not create the directory its reporter writes
into, and a check runs on an export of the committed tree.

`tools/junit-with-files.mjs`:

````js
// A node:test reporter that writes JUnit XML in which every test case names its file.
// Node's built-in junit reporter does not do that on every release, and a report that
// cannot tell two files apart cannot be matched to the tests a file declares.
import path from 'node:path';

const escape = (text) =>
  String(text)
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;');

export default async function* junitWithFiles(source) {
  // The names of the suites that are open in each file, by nesting level.
  const open = new Map();
  yield '<?xml version="1.0" encoding="utf-8"?>\n<testsuites>\n';
  for await (const event of source) {
    const data = event.data ?? {};
    if (event.type === 'test:start') {
      const names = open.get(data.file) ?? [];
      names.length = data.nesting;
      names[data.nesting] = data.name;
      open.set(data.file, names);
      continue;
    }
    if (event.type !== 'test:pass' && event.type !== 'test:fail') continue;
    // A suite reports too; so does the file itself when it is run in its own process.
    if (data.details?.type === 'suite' || data.file === undefined) continue;
    const file = path.relative(process.cwd(), data.file).split(path.sep).join('/');
    if (data.nesting === 0 && data.name === data.file) continue;
    const suites = (open.get(data.file) ?? []).slice(0, data.nesting);
    for (const suite of suites) yield `<testsuite name="${escape(suite)}">\n`;
    const head = `<testcase name="${escape(data.name)}" classname="test" file="${escape(file)}"`;
    if (data.skip !== undefined || data.todo !== undefined) {
      yield `${head}><skipped/></testcase>\n`;
    } else if (event.type === 'test:fail') {
      const message = escape(data.details?.error?.message ?? 'failed');
      yield `${head}><failure message="${message}"/></testcase>\n`;
    } else {
      yield `${head}/>\n`;
    }
    for (let i = 0; i < suites.length; i += 1) yield '</testsuite>\n';
  }
  yield '</testsuites>\n';
}
````

The reporter is there because of something observed while this plan was written, on Node
22.17.1: the built-in `junit` reporter writes `classname="test"` on every case and no `file`
attribute. The `node` preset then reads the id `test > codes > ac1 applies a percent code`
from the report, while the id it enumerates from the frozen file is
`test/codes.test.js > codes > ac1 applies a percent code`, and the verifier would rightly find
the frozen test missing from the report. That is assumption A3 failing for this toolchain.
With this reporter the two agree: the fixture was run in the scratch directory on both
implementations below, and `factory_presets::preset("node")` read `Passed` and `Failed` for
exactly the enumerated id. If spike S6 settled A3 another way (a Node release whose reporter
names the file, or a change to the preset), use that and delete the reporter; say which in the
pull request.

The repository's slug in the daemon's allowlist is `e2e/node`, which is why the work item is
`e2e/node#1`. The spec is the output of `SpecBuilder::new()` with the work item and the
first criterion's text changed; in the scratch workspace `factory_spec::parse` read it,
`validate` found no problem, and its scope contains `src/cart.js` and `test/codes.test.js`
and not `README.md`.

What a harness writes for this spec, on the honest path. The frozen test,
`test/codes.test.js`:

```js
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { total } from '../src/cart.js';

describe('codes', () => {
  it('ac1 applies a percent code', () => {
    assert.equal(total([100], 'TEN'), 90);
  });
});
```

and the implementation, `src/cart.js`:

```js
const CODES = { TEN: 10 };

export function total(prices, code) {
  const sum = prices.reduce((acc, price) => acc + price, 0);
  const percent = CODES[code] ?? 0;
  return Math.max(0, sum - (sum * percent) / 100);
}
```

Its one test id is `test/codes.test.js > codes > ac1 applies a percent code`, which is what
scenario 2 looks for (`ac1`). For `reqdrive`, the same two files are what its scripted agent
is told to write (Task 1).

**Task 3: `harness-scripted`.** A harness, in `tests/e2e/src/bin/harness-scripted.rs`, that
speaks protocol 0.2 and does real `git` in the work directory it is given. It runs no
container and no agent. Start from `harness-fake.rs`: its `send`, `event`, `observe`, `stage`,
`spawn_reader` and `handle_control` functions are the conversation, and are copied.
What differs:

| Step | `harness-scripted` |
|---|---|
| `initialize` | Capabilities: isolation `none`, metering `none`, no gates, delivery `bundle`, `halt` true, `resume` false, no holdouts, no controls, kinds `build`, presets `node` and `cargo` at version `0.1.0`. A unit dispatched to it therefore needs the opt-in and can only be T1 |
| after `unit/start` | Refuse a work order with no `spec`, `source` or `config` with a `failed` result of scope `system`. Announce `provision`, clone `source.bundle_path` into a directory under the work order's work directory, check out `order.branch` at `source.base_sha`, set `core.autocrlf=false`, `core.hooksPath` to an empty directory, and a fixed author; observe `provisioned` |
| the spec commit | Copy the bytes at `spec.bytes_path` to `.reqdrive/specs/<spec.id>.md`; commit that file and nothing else. This is always the first commit |
| `red` | Write `test/codes.test.js` (the frozen test above). Build the freeze: one frozen file with `harness_protocol::file_sha256` of its bytes, and the one frozen id above; observe `oracle_frozen` with it |
| `plan`, `green` | Announce both; write the implementation the script names; commit the test and the implementation as the work commit; observe `build_finished` |
| `check`, `review` | Announce both; observe `checks_passed` and a `review_finished` for round 1 with no blockers; repeat `green`, `check` and `review` without a new commit until the round reaches `caps.min_review_rounds` |
| `deliver` | `git bundle create <work dir>/delivered.bundle <base_sha>..<branch>`; send an `artifact` of kind `branch`; send a `pr_open` result whose evidence names the branch, the head commit and the bundle, with the freeze's hashes reported honestly; exit 0 |

The script is the environment variable `HARNESS_SCRIPTED`, set in the registry entry:

| `Script` | `HARNESS_SCRIPTED` | The work commit |
|---|---|---|
| `Honest` | `honest` | the frozen test and the implementation above |
| `TestsFail` | `tests_fail` | the frozen test, and `src/cart.js` unchanged but for a comment line, so the test fails when it is really run |
| `TampersWithFrozenTest` | `tampers` | as `honest`, then a third commit that changes `90` to `100` in `test/codes.test.js`. The freeze still reports the hash of the first version |
| `LeavesScope` | `leaves_scope` | as `honest`, plus a new file `README.md` in the same commit |

It handles `unit/halt` and `unit/abandon` as `harness-fake` does. It sends no `metric`. It
never reads a test result: every script reports `pr_open`, and what the control plane makes
of it is the scenario.

### Behaviours

The seven scenarios, and the test that keeps the table and the file in step. Each scenario's
harness, and what it proves:

| # | Test | Harness | Proves |
|---|---|---|---|
| 1 | `a_t1_unit_becomes_an_open_pull_request` | `reqdrive` | The whole path: token, sign, dispatch, seven stages, verification in the control plane's own container, one pull request for the verified commit with the work item in its body, nothing left running |
| 2 | `tests_that_fail_in_the_control_planes_run_stop_the_unit_and_can_be_reverified` | scripted, `TestsFail` | A harness's own "passed" is not believed; the unit stops for a person with the failing test named; `reverify` runs again with no harness and no pull request |
| 3 | `a_tampered_frozen_test_stops_the_unit_for_oracle_tampering` | scripted, `TampersWithFrozenTest` | A frozen test changed after the freeze is tampering; the unit cannot be resumed |
| 4 | `a_spec_changed_after_signing_is_refused_at_admission` | scripted, `Honest` | A signature binds bytes: the changed file is unsigned; the signed bytes still run, from the stored bytes |
| 5 | `an_unsigned_spec_is_refused` | scripted, `Honest` | No unit without a signature; nothing is stored |
| 6 | `a_change_outside_scope_fails_verification` | scripted, `LeavesScope` | A file the spec does not name fails the unit on the scope check; nothing is pushed |
| 7 | `a_restart_mid_unit_reaps_the_container_and_the_unit_resumes` | `reqdrive` | A killed daemon leaves no container; the unit is halted with a reason and resumes to a pull request on the same spec, base and branch |
| 8 | `every_scenario_in_the_table_has_a_test_that_runs_under_its_flag` | none | `SCENARIOS` and the test file agree (passes from Part 1 on) |

Run: `cargo build -p fleetd --bin serve`, then `cargo test -p fleet-e2e --test e2e_m1 -- --nocapture`.
Expected before the implementation: 7 failed with `not implemented: lane CC-E2E builds this`
and 1 passed. Expected when this lane is done: 8 passed and no line starting `UNFROZEN-FAIL`.

`tests/e2e/tests/e2e_m1.rs`:

````rust
//! Lane CC-E2E: milestone 1's end-to-end scenarios.
//!
//! Each starts the real `fleetd` binary on a temporary directory with a local bare repository
//! as its remote, and drives it over HTTP with the token. Needs Docker, `git`, the built
//! `fleetd` and `harness-scripted` binaries, and (for the two scenarios that use it) the
//! `reqdrive` binary at the pinned tag. Nothing here spends: the agent is scripted.
//!
//! A scenario body returns `Err` with what went wrong; `scenario` turns that into a failure
//! only for a scenario whose `frozen` flag is true.

use fleet_e2e::{scenario, Factory, Fixture, HarnessChoice, Script};
use serde_json::{json, Value};
use std::time::Duration;

const SPEC: &str = "SPEC-0001";
const A_MINUTE: Duration = Duration::from_secs(60);
const TEN_MINUTES: Duration = Duration::from_secs(600);

fn check(condition: bool, what: impl Into<String>) -> Result<(), String> {
    if condition {
        Ok(())
    } else {
        Err(what.into())
    }
}

fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().unwrap_or("")
}

/// Sign the fixture's spec and dispatch it. Returns the unit's id.
async fn dispatched(factory: &Factory, key: &str) -> Result<String, String> {
    let sha = factory.sign(SPEC).await?;
    let (status, body) = factory.dispatch(&sha, key).await;
    check(status == 200, format!("dispatch answered {status}: {body}"))?;
    Ok(text(&body, "unit_id").to_string())
}

fn event_types(events: &[Value]) -> Vec<&str> {
    events
        .iter()
        .filter_map(|e| e["event"]["type"].as_str())
        .collect()
}

#[tokio::test]
async fn a_t1_unit_becomes_an_open_pull_request() {
    scenario("a_t1_unit_becomes_an_open_pull_request", async {
        let factory = Factory::start(Fixture::Node, HarnessChoice::Reqdrive).await?;
        check(
            factory.get_without_token("/units").await == 401,
            "a request with no token was served",
        )?;
        let unit = dispatched(&factory, "t1-pr").await?;
        let detail = factory
            .until_phase(&unit, &["done", "failed", "needs_human"], TEN_MINUTES)
            .await?;
        check(
            text(&detail, "phase") == "done",
            format!("the unit ended {detail}"),
        )?;

        // One pull request, on the remote, for the verified commit, carrying the work item.
        let prs = factory.pull_requests();
        check(
            prs.len() == 1,
            format!("{} pull requests were opened", prs.len()),
        )?;
        let branch = text(&detail, "branch");
        let head = factory
            .remote_branch(branch)
            .ok_or("the branch was not pushed")?;
        check(
            text(&prs[0], "head_sha") == head,
            "the pull request is not for the pushed commit",
        )?;
        let body = text(&prs[0], "body");
        let work_item = text(&detail, "work_item");
        check(
            body.contains(&format!("Work-Item: {work_item}")),
            format!("no trailer in {body:?}"),
        )?;
        check(
            body.lines().any(|l| l.starts_with("Fixes ")),
            format!("no Fixes line in {body:?}"),
        )?;
        check(
            branch.contains(&unit),
            "the branch does not carry the unit id",
        )?;
        check(
            text(&detail, "pr_url") == text(&prs[0], "url"),
            "the stored pull request is another one",
        )?;

        // The log shows all seven stages and the phases in order, and nothing was left running.
        let events = factory.events(&unit).await;
        let stages: Vec<&str> = events
            .iter()
            .filter(|e| e["event"]["type"] == "stage" && e["event"]["status"] == "started")
            .filter_map(|e| e["event"]["stage"].as_str())
            .collect();
        for stage in [
            "provision",
            "red",
            "plan",
            "green",
            "check",
            "review",
            "deliver",
        ] {
            check(
                stages.contains(&stage),
                format!("stage {stage} was never announced: {stages:?}"),
            )?;
        }
        check(
            event_types(&events).last() == Some(&"done"),
            "the log does not end with done",
        )?;
        check(
            factory.containers(&unit).is_empty(),
            "a container outlived the unit",
        )
    })
    .await;
}

#[tokio::test]
async fn tests_that_fail_in_the_control_planes_run_stop_the_unit_and_can_be_reverified() {
    scenario(
        "tests_that_fail_in_the_control_planes_run_stop_the_unit_and_can_be_reverified",
        async {
            let harness = HarnessChoice::Scripted(Script::TestsFail);
            let factory = Factory::start(Fixture::Node, harness).await?;
            let unit = dispatched(&factory, "tests-fail").await?;
            let detail = factory
                .until_phase(&unit, &["needs_human", "done", "failed"], TEN_MINUTES)
                .await?;
            check(
                text(&detail, "phase") == "needs_human",
                format!("the unit ended {detail}"),
            )?;
            check(
                text(&detail, "pause_reason") == "verification stopped",
                format!("paused for {:?}", detail["pause_reason"]),
            )?;
            check(
                text(&detail, "paused_from") == "merge_check",
                "it was not paused at verification",
            )?;
            check(
                factory.pull_requests().is_empty(),
                "a pull request was opened for failing tests",
            )?;
            let events = factory.events(&unit).await;
            let named = events.iter().any(|e| {
                e["event"]["type"] == "blocked" && text(&e["event"], "detail").contains("ac1")
            });
            check(named, "the failing test was not named")?;

            // Re-verify is available, runs without a harness, and stops the unit again.
            let before = events.len();
            let status = factory
                .command(&unit, json!({"command": "reverify", "cmd_id": "v1"}))
                .await;
            check(status == 202, format!("reverify answered {status}"))?;
            tokio::time::sleep(Duration::from_secs(2)).await;
            let detail = factory
                .until_phase(&unit, &["needs_human", "done", "failed"], TEN_MINUTES)
                .await?;
            check(
                text(&detail, "phase") == "needs_human",
                "re-verifying passed a failing unit",
            )?;
            let events = factory.events(&unit).await;
            let phases: Vec<&str> = events[before..]
                .iter()
                .filter(|e| e["event"]["type"] == "phase_changed")
                .filter_map(|e| e["event"]["to"].as_str())
                .collect();
            check(
                phases == ["merge_check", "needs_human"],
                format!("after reverify: {phases:?}"),
            )
        },
    )
    .await;
}

#[tokio::test]
async fn a_tampered_frozen_test_stops_the_unit_for_oracle_tampering() {
    scenario(
        "a_tampered_frozen_test_stops_the_unit_for_oracle_tampering",
        async {
            let harness = HarnessChoice::Scripted(Script::TampersWithFrozenTest);
            let factory = Factory::start(Fixture::Node, harness).await?;
            let unit = dispatched(&factory, "tamper").await?;
            let detail = factory
                .until_phase(&unit, &["needs_human", "done", "failed"], TEN_MINUTES)
                .await?;
            check(
                text(&detail, "phase") == "needs_human",
                format!("the unit ended {detail}"),
            )?;
            check(
                text(&detail, "pause_reason") == "oracle tampering",
                "not paused for tampering",
            )?;
            check(
                factory.pull_requests().is_empty(),
                "a pull request was opened",
            )?;
            // It can only be abandoned: resume is refused.
            let status = factory
                .command(&unit, json!({"command": "resume", "cmd_id": "r1"}))
                .await;
            check(
                status == 202,
                format!("the command was not delivered: {status}"),
            )?;
            tokio::time::sleep(Duration::from_secs(2)).await;
            let (_, detail) = factory.get(&format!("/units/{unit}/detail")).await;
            check(
                text(&detail, "phase") == "needs_human",
                "a tampered unit was resumed",
            )
        },
    )
    .await;
}

#[tokio::test]
async fn a_spec_changed_after_signing_is_refused_at_admission() {
    scenario(
        "a_spec_changed_after_signing_is_refused_at_admission",
        async {
            let factory =
                Factory::start(Fixture::Node, HarnessChoice::Scripted(Script::Honest)).await?;
            let signed = factory.sign(SPEC).await?;
            // The file changes on the base branch after it was signed.
            let (_, view) = factory
                .get(&format!("/specs?repo={}&spec_id={SPEC}", factory.repo()))
                .await;
            let changed = format!("{}\nAn extra line nobody signed.\n", text(&view, "text"));
            factory.push_to_base(&format!(".reqdrive/specs/{SPEC}.md"), &changed);
            let (_, now) = factory
                .get(&format!("/specs?repo={}&spec_id={SPEC}", factory.repo()))
                .await;
            let unsigned = text(&now, "sha256").to_string();
            check(unsigned != signed, "the change did not change the hash")?;
            check(
                now["signature"].is_null(),
                "the changed bytes show a signature",
            )?;

            let (status, body) = factory.dispatch(&unsigned, "changed-spec").await;
            check(
                status == 403 && text(&body, "code") == "not_signed",
                format!("{status}: {body}"),
            )?;
            let (_, units) = factory.get("/units").await;
            check(
                units.as_array().map(Vec::len) == Some(0),
                "a unit was stored for unsigned bytes",
            )?;

            // The bytes that were signed still dispatch, and the unit runs from those bytes.
            let (status, body) = factory.dispatch(&signed, "signed-spec").await;
            check(
                status == 200,
                format!("the signed bytes were refused: {status}: {body}"),
            )?;
            let unit = text(&body, "unit_id").to_string();
            let detail = factory
                .until_phase(&unit, &["done", "failed", "needs_human"], TEN_MINUTES)
                .await?;
            check(
                text(&detail, "spec_sha256") == signed,
                "the unit ran from other bytes",
            )?;
            check(
                text(&detail, "phase") == "done",
                format!("the unit ended {detail}"),
            )
        },
    )
    .await;
}

#[tokio::test]
async fn an_unsigned_spec_is_refused() {
    scenario("an_unsigned_spec_is_refused", async {
        let factory =
            Factory::start(Fixture::Node, HarnessChoice::Scripted(Script::Honest)).await?;
        let (_, view) = factory
            .get(&format!("/specs?repo={}&spec_id={SPEC}", factory.repo()))
            .await;
        let sha = text(&view, "sha256").to_string();
        check(sha.len() == 64, format!("no hash in {view}"))?;
        let (status, body) = factory.dispatch(&sha, "unsigned").await;
        check(
            status == 403,
            format!("an unsigned spec was answered {status}: {body}"),
        )?;
        check(
            text(&body, "code") == "not_signed",
            format!("refused as {body}"),
        )?;
        tokio::time::sleep(Duration::from_secs(1)).await;
        let (_, units) = factory.get("/units").await;
        check(
            units.as_array().map(Vec::len) == Some(0),
            "a unit was stored",
        )?;
        check(
            factory.pull_requests().is_empty(),
            "a pull request was opened",
        )
    })
    .await;
}

#[tokio::test]
async fn a_change_outside_scope_fails_verification() {
    scenario("a_change_outside_scope_fails_verification", async {
        let factory =
            Factory::start(Fixture::Node, HarnessChoice::Scripted(Script::LeavesScope)).await?;
        let unit = dispatched(&factory, "scope").await?;
        let detail = factory
            .until_phase(&unit, &["failed", "done", "needs_human"], TEN_MINUTES)
            .await?;
        check(
            text(&detail, "phase") == "failed",
            format!("the unit ended {detail}"),
        )?;
        let events = factory.events(&unit).await;
        let rejected = events.iter().any(|e| {
            e["event"]["type"] == "error"
                && text(&e["event"], "detail").contains("evidence rejected: Diff")
        });
        check(rejected, "the unit did not fail on the scope check")?;
        check(
            factory.pull_requests().is_empty(),
            "a pull request was opened",
        )?;
        check(
            factory.remote_branch(text(&detail, "branch")).is_none(),
            "an unverified branch was pushed",
        )
    })
    .await;
}

#[tokio::test]
async fn a_restart_mid_unit_reaps_the_container_and_the_unit_resumes() {
    scenario(
        "a_restart_mid_unit_reaps_the_container_and_the_unit_resumes",
        async {
            let mut factory = Factory::start(Fixture::Node, HarnessChoice::Reqdrive).await?;
            let unit = dispatched(&factory, "restart").await?;
            factory
                .until_phase(&unit, &["building", "checking", "reviewing"], TEN_MINUTES)
                .await?;
            check(
                !factory.containers(&unit).is_empty(),
                "the unit has no container to reap",
            )?;

            factory.restart().await?;

            // Startup reconciliation: the container is gone, and the unit is halted with a reason.
            let detail = factory.until_phase(&unit, &["halted"], A_MINUTE).await?;
            check(
                text(&detail, "pause_reason") == "daemon restarted",
                format!("{detail}"),
            )?;
            check(
                factory.containers(&unit).is_empty(),
                "the container survived the restart",
            )?;
            check(
                !detail["live"].as_bool().unwrap_or(true),
                "a halted unit is shown live",
            )?;

            // Resume: the same unit, the same spec and base, to a pull request.
            let before = detail.clone();
            let status = factory
                .command(&unit, json!({"command": "resume", "cmd_id": "r1"}))
                .await;
            check(status == 202, format!("resume answered {status}"))?;
            let detail = factory
                .until_phase(&unit, &["done", "failed", "needs_human"], TEN_MINUTES)
                .await?;
            check(
                text(&detail, "phase") == "done",
                format!("the resumed unit ended {detail}"),
            )?;
            for key in ["spec_sha256", "base_sha", "branch"] {
                check(detail[key] == before[key], format!("resume changed {key}"))?;
            }
            check(
                factory.pull_requests().len() == 1,
                "the resumed unit opened no pull request",
            )
        },
    )
    .await;
}

#[test]
fn every_scenario_in_the_table_has_a_test_that_runs_under_its_flag() {
    let source = include_str!("e2e_m1.rs");
    for entry in fleet_e2e::SCENARIOS {
        assert!(
            source.contains(&format!("async fn {}()", entry.name)),
            "scenario {} has no test",
            entry.name
        );
        assert!(
            source.contains(&format!("scenario(\"{}\"", entry.name))
                || source.contains(&format!("\"{}\",", entry.name)),
            "the test {} does not run under its own flag",
            entry.name
        );
    }
    assert_eq!(fleet_e2e::SCENARIOS.len(), 7);
}
````

### Implementation notes

**`scenario`.** Look the name up in `SCENARIOS` (panic if it is not there), await the body,
and on `Err(why)`: panic with the name and `why` when the entry is frozen; otherwise print
`UNFROZEN-FAIL <name>: <why>` to standard error and return.

**`Factory::start`**, in order; each failure is an `Err` that says what is missing and how to
get it:

1. `docker version` exits 0, `git --version` is 2.38 or later.
2. The daemon's binary: `serve` plus `std::env::consts::EXE_SUFFIX` in the directory above
   the running test's (`target/<profile>/`), as `fleet_supervisor::fake_binary_beside` finds
   `harness-fake`. `harness-scripted` is in the same directory: cargo builds a package's own
   binaries before its integration tests.
3. A temporary directory. In it: `seed/` (the fixture, committed once on `main` with a fixed
   author and date and `core.autocrlf=false`), `remote.git` (a bare clone of `seed`), `prs/`,
   `data/`.
4. `fleetd.toml`: the `[caps]` defaults and one `[[repo]]` with `slug = "e2e/node"`,
   `url` the absolute path of `remote.git` written with forward slashes, `base_branch = "main"`.
5. `harnesses.toml`: one `[[harness]]`, named `reqdrive` or `scripted`, with the command and,
   for the scripted harness, `env = { HARNESS_SCRIPTED = "<script>" }`.
6. A free port: bind `127.0.0.1:0`, read the port, drop the listener.
7. Start the daemon with the working directory set to the temporary directory, standard
   output and error appended to `fleetd.log` there, and exactly this environment added:
   `FLEETD_TOKEN` (a fixed 40-character test value), `CC_ADDR`, `CC_DB=fleet.db`,
   `FLEETD_DATA_DIR=data`, `FLEETD_CONFIG=fleetd.toml`, `FLEETD_REGISTRY=harnesses.toml`,
   `FLEETD_PR_HOST=local:prs`, `FLEETD_OPERATOR=e2e`, `CC_RECONCILE_SECS=5`. Remove
   `ANTHROPIC_API_KEY` and every `CLAUDE_*` and `OPENAI_*` variable from what it inherits: a
   hermetic run must not be able to spend.
8. Poll `GET /harnesses` with the token until the factory's harness is `healthy`, for up to
   60 seconds. On timeout the `Err` carries the last 40 lines of `fleetd.log`.

`restart` kills the daemon's process without asking it to stop (`Child::kill`), waits for it,
and repeats steps 7 and 8 on the same directory and port. `Drop` kills the daemon and removes
every container that carries a `cc.unit_id` label of a unit this factory dispatched; the
temporary directory removes itself.

**The observers.** `pull_requests` reads every `prs/pr-*.json` in number order.
`remote_branch` is `git -C remote.git rev-parse --verify --quiet refs/heads/<branch>`.
`push_to_base` commits in `seed/` and pushes to `remote.git`, as a person would. `containers`
is `docker ps -a --filter label=cc.unit_id=<unit> --format {{.Names}}`. `dispatch` sends the
fixture's work item, the repository slug, the harness name, the hash, the key, and
`opt_in: true` exactly when the harness is the scripted one. `until_phase` polls every 250 ms
and, on timeout, returns the unit's last detail and its event types in the `Err`, so an
unfrozen failure is readable in the tier's output.

**Pitfalls.** The preset version the scripted harness declares is a literal (`0.1.0`): this
crate may not link `factory-presets`. When that crate's version changes, admission refuses
with `presets_version_mismatch` and every scripted scenario says so; change the literal then.
On Windows a path given to `git` as a remote must not be a `\\?\` path: build it from the
temporary directory with `/`. Docker Desktop on Windows mounts only directories it is allowed
to share; the temporary directory is under the user's profile, which is shared by default,
and step 1's error says so when a mount is refused. Scenario 7 depends on timing: the unit
must be seen in `building`, `checking` or `reviewing` before the kill, so the scripted agent
of `reqdrive` must take at least a few seconds in `green` (a setting of Task 1). Scenarios do
not share a factory: each starts its own daemon, repository and port, so the test binary may
run them in parallel.

### Verify

| Command | Expected |
|---|---|
| `cargo build -p fleetd --bin serve` | exit 0 |
| `cargo test -p fleet-e2e --test e2e_m1 -- --nocapture` with Docker running and `REQDRIVE_BIN` set | `8 passed`; no `UNFROZEN-FAIL` line |
| the same with `REQDRIVE_BIN` unset | `8 passed`; exactly two `UNFROZEN-FAIL` lines, for scenarios 1 and 7, each naming `REQDRIVE_BIN` |
| the same with Docker stopped | `8 passed`; seven `UNFROZEN-FAIL` lines, each saying Docker is not running |
| `cargo xtask test static --root-only`, `e2e` | exit 0 |
| *Git-Bash:* `grep -c "frozen: true" tests/e2e/src/lib.rs` | `0` |
| *Git-Bash:* `docker ps -a --filter label=cc.unit_id --format "{{.Names}}"` after a full run | no output: nothing was left behind |
| `git diff --name-only origin/factory/m1...HEAD` | only paths under **Owns** |

### Done when

- [ ] The suite passes three times in a row on the Windows build host with no `UNFROZEN-FAIL`
      line, and once in the `e2e` job on Linux.
- [ ] No `unimplemented!` is left in `tests/e2e/src`.
- [ ] The pull request says how assumptions A4, A5 and A6 were reconciled, gives the pinned
      tag and the image digest with the commands that produced them, and leaves every
      `frozen` flag false.

---

# Part 3 — Integration and proof

## Merge order

Everything merges into `factory/m1`, by the coordinator, never by the lane that wrote it.

| Step | What merges | Before it merges | After it merges |
|---|---|---|---|
| 1 | Part 1, `feat/m1-interfaces` | Requests R1 to R8 are on `factory/m1`; Part 1's Verify table | Every lane cuts its branch |
| 2 | Lanes CC-STORE, CC-FORGE, CC-RUNNER, CC-VERIFY, CC-SUPERVISOR, CC-ADMISSION, CC-API, in any order | The lane's Verify table, with real output in the pull request; the reviewing session's pass over its Review Focus tests | The coordinator moves the lane's planned gates into its Form and `forms/registry.md` (request R9). `cargo xtask test static`, `unit` and `contract` are green on `factory/m1` |
| 2a | Lane CC-E2E's Tasks 1 to 3 (the pin, the fixture, `harness-scripted`) may merge any time after step 1 | `cargo test -p fleet-e2e` passes: every scenario prints `UNFROZEN-FAIL`, because no daemon can start yet | Nothing gates on it |
| 3 | Lane CC-WIRE | All seven of step 2 are merged and the lane was rebased on them; its Verify table, including the five replaced characterisation tests and the two refused starts | `cargo xtask test integration` is green. The daemon now refuses a request without the token |
| 4 | The cockpit's change that mints the token and sends it (another plan) | The cockpit's own checks | The cockpit can reach the daemon again. Nothing in this plan needs the cockpit: the end-to-end suite and the watched proof talk to the daemon directly |
| 5 | Lane CC-E2E's scenarios | Step 3 is merged; the lane's Verify table: `8 passed` and no `UNFROZEN-FAIL` line, three times running, on the Windows build host | The `e2e` tier reports seven scenarios, all unfrozen |
| 6 | Nothing | The watched proof below | The owner freezes scenarios (below) |

Two lanes never edit one file: each lane's **Owns** is disjoint from every other's, and the
manifests, the lock file, `xtask` and the Forms are the coordinator's. A merge conflict
between two lanes is therefore a sign that one of them left its files; stop and find out
which.

After step 3 and again after step 5, on a clean checkout of `factory/m1`:

| Command | Expected |
|---|---|
| `cargo build --workspace --all-targets --locked` | exit 0 |
| `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | exit 0 |
| `cargo xtask test static` | exit 0 |
| `cargo xtask test unit` | exit 0 |
| `cargo xtask test contract` | exit 0 |
| `cargo xtask test integration` | exit 0; the tests that need Docker are reported as ignored unless Docker is running, and are then run by hand with `-- --ignored` and reported |
| `cargo xtask test e2e` | exit 0; after step 5, no `UNFROZEN-FAIL` line |
| *Git-Bash:* `grep -rn "unimplemented!(" crates tests --include=*.rs` | no output |

## The end-to-end scenarios

Seven, in `tests/e2e/tests/e2e_m1.rs`, each listed in `fleet_e2e::SCENARIOS` with
`frozen: false`. Each starts its own daemon on its own temporary directory, with a local bare
repository as the remote and pull requests recorded in files, and spends nothing.

| # | Scenario | Given | When | Then |
|---|---|---|---|---|
| 1 | `a_t1_unit_becomes_an_open_pull_request` | The fixture repository, onboarded, with one T1 spec; `reqdrive` at the pinned tag with a scripted agent | The spec is read, signed and dispatched with the token | A request without the token is 401. The unit reaches `done`. Exactly one pull request is recorded, for the commit that is on the remote branch, its body carrying `Fixes` and the `Work-Item:` trailer; the branch carries the unit id; the stored pull request is that one. The log announces all seven stages and ends with `done`. No container carries the unit's label |
| 2 | `tests_that_fail_in_the_control_planes_run_stop_the_unit_and_can_be_reverified` | A harness that delivers an implementation failing the frozen test and reports success | Dispatched; then `reverify` | The unit stops in `needs_human`, reason `verification stopped`, paused from `merge_check`, the failing test named; no pull request. `reverify` is accepted, runs `merge_check` again with no harness, and stops the unit again |
| 3 | `a_tampered_frozen_test_stops_the_unit_for_oracle_tampering` | A harness that edits the frozen test after freezing it | Dispatched; then `resume` | `needs_human`, reason `oracle tampering`; no pull request; the `resume` is delivered and changes nothing |
| 4 | `a_spec_changed_after_signing_is_refused_at_admission` | A signed spec whose file then changes on the base branch | The changed bytes are dispatched; then the signed bytes | The changed bytes show no signature and are refused 403 `not_signed`, with no unit stored. The signed bytes are admitted and the unit runs to `done` from them |
| 5 | `an_unsigned_spec_is_refused` | A spec nobody signed | Dispatched | 403 `not_signed`; no unit; no pull request |
| 6 | `a_change_outside_scope_fails_verification` | A harness that also adds a file the spec does not name | Dispatched | `failed`, with `evidence rejected: Diff` in the log; no pull request; the branch is not on the remote |
| 7 | `a_restart_mid_unit_reaps_the_container_and_the_unit_resumes` | `reqdrive` running a unit, seen in `building`, `checking` or `reviewing` with a container | The daemon is killed and started again; then `resume` | After the start: the unit is `halted` with reason `daemon restarted`, is not live, and has no container. After `resume`: `done`, one pull request, on the same spec hash, base commit and branch |

Scenarios 2 to 6 need Docker and this repository's own scripted harness. Scenarios 1 and 7
also need the `reqdrive` binary. The Review Focus failure modes are covered below the
end-to-end level, by the lane tests named in the header; these seven show the paths joined.

## The watched proof

One real unit, with a real model, on the real sandbox repository, watched by the operator.
This is the only step of milestone 1 that spends money and the only one that touches GitHub
beyond this repository. An agent may prepare it; the operator runs it. Every box is ticked by
the operator.

**Before the session**

- [ ] `factory/m1` is at the commit of merge-order step 5, and the table above is green there.
- [ ] A checkout of that commit for the proof, which touches no other worktree:
      `git -C D:/MajorProjects/INFRASTRUCTURE/command-center worktree add --detach D:/MajorProjects/.swarm-wt/m1-proof origin/factory/m1`.
- [ ] The sandbox repository `adbarc92/command-center-agent-sandbox` has, on `main`:
      `.reqdrive/config.toml` (the format of `docs/repo-config.md`, its image pinned to a
      digest) and one T1 spec at `.reqdrive/specs/<spec id>.md` whose `work_item` is
      `adbarc92/command-center-agent-sandbox#<N>` for an open issue `<N>`, whose scope names
      files that exist there, and which has no open question and no unconfirmed invariant.
- [ ] The sandbox's test command writes a JUnit report in which every test case names its
      file. Node 22.17.1's built-in reporter does not (lane CC-E2E, Task 2); if the sandbox's
      image has that reporter, the sandbox uses the reporter of `tests/e2e/fixtures/node/tools/`
      or whatever spike S6 settled. Without this the unit stops at verification with its
      frozen test reported missing, which is the control plane working and the proof failing.
- [ ] `gh auth status` shows an account that can push to the sandbox repository.
- [ ] Docker is running. `git --version` is 2.38 or later.
- [ ] The `reqdrive` binary is the tag in `tests/e2e/reqdrive.pin`, and a registry file names
      it:

      ```toml
      [[harness]]
      name = "reqdrive"
      command = ["C:/path/to/reqdrive.exe", "harness"]
      ```

      with the arguments the `reqdrive` plan gives for a real run.
- [ ] **The operator chooses the spend cap for this unit, in dollars, and writes it here:
      `$______`.** It goes into the daemon's configuration as the unit ceiling and into the
      dispatch request. Nothing in this plan chooses it.
- [ ] `fleetd.toml`, with the operator's number in both caps:

      ```toml
      [caps]
      unit_usd = <the cap>
      unit_wall_clock_secs = 1800
      global_usd = <the cap>
      max_concurrent = 1

      [[repo]]
      slug = "adbarc92/command-center-agent-sandbox"
      url = "https://github.com/adbarc92/command-center-agent-sandbox"
      base_branch = "main"
      ```

      With `global_usd` equal to `unit_usd`, a second unit cannot be admitted while the first
      holds its reservation.

**Start the daemon** (PowerShell, in an empty directory that holds the two files above)

- [ ] Mint a token and start:

      ```powershell
      $env:FLEETD_TOKEN = [Convert]::ToHexString([Security.Cryptography.RandomNumberGenerator]::GetBytes(24))
      $env:FLEETD_CONFIG = 'fleetd.toml'
      $env:FLEETD_REGISTRY = 'harnesses.toml'
      $env:FLEETD_OPERATOR = '<your name>'
      cargo run --manifest-path D:\MajorProjects\.swarm-wt\m1-proof\Cargo.toml -p fleetd --bin serve
      ```

      The model credential `reqdrive` needs is set in this same shell, by the operator, as the
      `reqdrive` plan's smoke section names it. Expected: one line per harness, `reqdrive`
      healthy; the reaping report; a line saying it listens on `http://127.0.0.1:8787`. The
      token is not printed.
- [ ] In a second PowerShell, with the same token copied by hand:

      ```powershell
      $h = @{ Authorization = "Bearer $env:FLEETD_TOKEN" }
      $base = 'http://127.0.0.1:8787'
      $repo = 'adbarc92/command-center-agent-sandbox'
      ```

**The run**

- [ ] No token, no service: `Invoke-WebRequest "$base/units" -SkipHttpErrorCheck | Select-Object StatusCode`
      shows `401`.
- [ ] The harness is the one intended:
      `Invoke-RestMethod "$base/harnesses" -Headers $h | ConvertTo-Json -Depth 6`. Expected:
      `reqdrive`, `healthy`, isolation `container`, metering `usd`, delivery `bundle`, the
      repository's preset among its presets.
- [ ] Read the spec as the daemon reads it:
      `$spec = Invoke-RestMethod "$base/specs?repo=$repo&spec_id=<spec id>" -Headers $h; $spec.text; $spec.problems; $spec.commit; $spec.sha256`.
      Expected: the text you expect to sign, no problems. **Read the text.** This is the only
      human judgment in the run.
- [ ] Sign exactly those bytes:

      ```powershell
      $sign = @{ repo = $repo; spec_id = '<spec id>'; commit = $spec.commit; sha256 = $spec.sha256 } | ConvertTo-Json
      Invoke-RestMethod "$base/signatures" -Method Post -Headers $h -ContentType 'application/json' -Body $sign
      ```

      Expected: your name, the time, tier `t1`, the work item.
- [ ] Dispatch, with the cap:

      ```powershell
      $mission = @{ work_item = "$repo#<N>"; repo = $repo; spec_sha256 = $spec.sha256; harness = 'reqdrive'; idempotency_key = 'm1-proof-1'; usd_cap = <the cap> } | ConvertTo-Json
      $unit = (Invoke-RestMethod "$base/missions" -Method Post -Headers $h -ContentType 'application/json' -Body $mission).unit_id
      ```

      Expected: a unit id.
- [ ] The same request again is refused:
      `Invoke-WebRequest "$base/missions" -Method Post -Headers $h -ContentType 'application/json' -Body $mission -SkipHttpErrorCheck | Select-Object StatusCode, Content`
      shows `409` and `duplicate_key`.
- [ ] Watch it, every half minute:
      `Invoke-RestMethod "$base/units/$unit/detail" -Headers $h | Select-Object phase, cost, elapsed_ms, pause_reason, pr_url`.
      Expected: the phases in order; `cost` rising and staying under the cap.
- [ ] While the harness runs, its container carries the unit's label:
      `docker ps --filter "label=cc.unit_id=$unit" --format "{{.Names}}"`. When the phase is
      `merge_check`, run it again: a container whose name starts `cc_check_` is the control
      plane's own run of the repository's tests. Seeing it is the point of this proof; if the
      phase passes too quickly to catch it, say so in the record instead of ticking the box.

**Stop conditions.** If `cost` passes the cap and the unit does not pause itself with reason
`usd cap`, or anything happens that this list does not describe, send
`@{ command = 'abandon'; cmd_id = 'stop-1' } | ConvertTo-Json` to `"$base/units/$unit/commands"`,
then stop the daemon with Ctrl+C, then `docker ps -a --filter label=cc.unit_id` and remove what
is left. Record what happened before trying again. A unit that pauses for a person is not a
failure of the proof to hide: read `pause_reason`, and decide.

**What proves it**

- [ ] The unit's phase is `done`, and `pr_url` is a pull request on the sandbox repository.
- [ ] `gh pr view <pr_url> --json state,headRefName,headRefOid,body`: state `OPEN`; the branch
      is `factory/<N>-<unit id>`; the body has a `Fixes` line for issue `<N>` and the trailer
      `Work-Item: adbarc92/command-center-agent-sandbox#<N>`.
- [ ] The pull request's first commit adds `.reqdrive/specs/<spec id>.md` and nothing else, and
      `git show <first commit>:.reqdrive/specs/<spec id>.md | sha256sum` (Git-Bash, in a clone
      of the sandbox) prints the hash you signed.
- [ ] The event log shows the control plane's own verification, not only the harness's word:
      `(Invoke-RestMethod "$base/units/$unit/events" -Headers $h).event | Where-Object type -eq 'phase_changed' | Select-Object from, to`
      includes `merge_check` to `pr_open`, which the supervisor applies only on a `Verified`
      verdict.
- [ ] `docker ps -a --filter "label=cc.unit_id=$unit"` shows nothing.
- [ ] The final `cost` is at or under the cap. Write it down: `$______`.
- [ ] The pull request is left open. Whether to merge it is a separate decision and not part
      of this proof.

**After the proof**

- [ ] The owner sets `frozen: true` in `tests/e2e/src/lib.rs` for each scenario whose behaviour
      was watched here or in the hermetic suite and judged right, in a pull request of the
      owner's own. From that commit the scenario gates the `e2e` tier.
- [ ] The run's date, the unit id, the pull request, the cap and the cost go into
      `docs/STATUS.md` in the session's wrap-up.
- [ ] Merging `factory/m1` into `main` is the program plan's step, not this plan's.

---

## Self-review

What was actually done to check this plan, and what could not be closed. Nothing below was
run in any repository: every check ran in a scratch workspace outside them.

### What was run

The scratch workspace is this repository's crates as milestone 0 plans to leave them (the
skeleton crates, `fleet-core`, `harness-protocol`, `harness-conformance`, `factory-spec`,
`factory-presets`, `xtask` and the characterisation tests, taken from the scratch builds the
three milestone-0 plans were written against), plus requests R1 to R8, plus every file this
plan prints.

- [x] Every Rust listing in Parts 1 and 2 was copied into this file by script from that
      workspace: no listing was typed here. The script fails if a file has a carriage return
      or if a listing marker is left unexpanded.
- [x] `cargo fmt --all -- --check`: no output.
- [x] `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`: exit 0.
      That compiles every lane's test file against Part 1's interface.
- [x] `cargo test --workspace --lib`: 494 passed, 0 failed. Part 1's seven crates account for
      55 of them (4, 8, 10, 10, 14, 5, 4), run with and without `--features testkit`.
- [x] `cargo xtask deps`: `dependency direction: ok`, with request R3's rows.
      `cargo xtask test static --root-only`: 6 steps passed. `cargo xtask test unit`: 29 steps
      passed.
- [x] The whole workspace's tests, with every lane's test file present: 288 tests failed and
      every one of the 288 panics is `not implemented: lane <LANE> builds this` (or `moves
      this`). The counts are the ones each lane states: store 15; forge 5 and 8; runner 9,
      with 7 Docker tests ignored; verify 38; admission 35; supervisor 31, 22, 24, 24, 14 and
      7; API 32; wiring 12 of 13 and 5; end to end 7 of 8. Every other target passed,
      including the five characterisation files as milestone 0 delivers them (11, 8, 7, 5, 9).
- [x] The red state of Part 1's test-first steps was observed for `fleet-store` (the compiler
      errors Task 2 names). For the other six crates it is stated, not observed.
- [x] The fixture of `git_it.rs`, which builds repositories and bundles with real `git`, was
      run on the build host. The contract suite of `fleet-forge` itself has been run only
      against the in-memory forge: nothing implements it over `git` yet.
- [x] Five pieces of implementation the plan prints were compiled and run on the Windows
      build host: the Job Object module (it ended a grandchild process: one `ping` before the
      kill, none after); the token check, the CORS order and the WebSocket subprotocol (4
      tests against a small router, one over a real socket); the bounded line reader (a line
      one byte past the limit is `TooLong`; a line of exactly the limit is read); the demo
      forge adapter (1 test); and the five replacement characterisation tests, which compile
      in the files they go into.
- [x] The end-to-end fixture was run with Node 22.17.1: the base passes; the frozen test fails
      on the unchanged implementation and passes on the honest one; `factory_spec::parse` and
      `validate` accept the spec with no problem; and `factory_presets::preset("node")` reads
      the fixture's reports to exactly the id it enumerates from the frozen file.
- [x] Every test name in prose (274 of them) was checked by script to exist as a function in
      a listing.
- [x] Searched for "TBD", "TODO", "similar to the above", "handle errors appropriately",
      "as appropriate", "and so on", "etc.": none.
- [x] The prose was compared by script, in runs of seven words, with the private design, the
      private program plan and the supervisor spec. The coincidences found were reworded; what
      is left shared is a function signature and a URL.
- [x] Line numbers cited for `crates/fleetd/src/server.rs`, `driver.rs`, `store.rs`,
      `reconcile.rs` and `gh_forge.rs` were checked against this repository's files today.
- [x] The milestone-0 plans were read again at the end for the names this plan uses: the
      tier names and `cargo xtask test <tier>`, the pinned CI job names, the Form format, and
      lane CC-CORE's Task 16 (`Command::Reverify`, `ErrorScope::Harness`,
      `Command::to_trigger() -> Option<Trigger>`).

**Found and fixed during this review:** a Verify row of Part 1 whose `grep` would have printed
99 where it promised 0; a wiring test that asserted "not 404" for routes whose handlers answer
404 for an unknown unit; two unit-id counters that would both have minted `u1`
(`Admission::mint_unit_id` and its test were added); an unused fixture variant in the
end-to-end interface; an issue this plan said it closed that the cockpit must close; `driver.rs`
line numbers written from memory and wrong; and a fixture test command that could not have
passed verification (below, gap 6).

### Gaps that could not be closed

1. **No lane's tests were seen to pass.** There is no reference implementation. Each test was
   compiled and seen to fail for the stated reason; a test that is wrong in a way only an
   implementation reveals (an assertion that contradicts another, a rig timing that a correct
   supervisor does not meet) is still possible. The rule for that case is in Global
   Constraints: the cited document wins, and the lane stops and reports.
2. **Lane CC-WIRE is specified, not built.** Its eighteen tests compile, but the design of the
   launcher, of `AppState` as a view of the wiring, and of the adapted unit tests inside
   `server.rs` is prose and tables. The claim that every characterisation test but the five
   named passes unchanged is a prediction. The ones most at risk:
   `a_t1_demo_mission_walks_exactly_these_phases` and `the_read_routes_return_these_shapes`
   (they need the supervisor's first logged record to be the phase change and its last to be
   `done`), and every test with a ten-second deadline, because the scripted harness paces
   itself at 50 ms a step.
3. **The demo cost `$0.04`** in the replaced characterisation test was worked out by reading
   `harness-fake.rs` (one metric of `$0.02` per build, two rounds), not observed.
4. **A unit that cannot be prepared when it is brought back** (its signed bytes or its
   repository configuration cannot be read) stays unreachable until the daemon restarts,
   because the hub never forgets a unit. Fixing it needs one more method on `Hub`; it was
   left out to keep Part 1's interface as small as its tests.
5. **`reqdrive` is not known to this plan.** How the harness is started, how its agent is
   scripted, which label it puts on containers and whether it resumes are read from its own
   plan by lane CC-E2E, Task 1 (assumptions A4 and A5). Scenarios 1 and 7 cannot be finished
   until that is known, and scenario 7 also depends on the scripted agent being slow enough
   to be caught mid-run.
6. **Assumption A3 does not hold for Node 22.17.1's own reporter.** Observed, not inferred:
   its JUnit report names no file, so the ids the `node` preset reads differ from the ids it
   enumerates. The end-to-end fixture works around it with a reporter of its own. The sandbox
   repository, and any real Node repository, needs the same or whatever spike S6 settles;
   Part 3's checklist says so. This plan does not change the preset.
7. **The Unix half of the process-tree module was not compiled** (the build host is Windows),
   and only the simple case was run on Windows: a daemon that is itself inside a job that
   forbids nesting was not tried (assumption A1).
8. **`fleet-runner`'s Docker tests were never run**, here or anywhere: they are listed,
   compiled and ignored. Assumption A2 (an offline check on a read-only cache) rests on spike
   S7 alone until lane CC-RUNNER runs them.
9. **`GhCli` is exercised by nothing hermetic.** Its first real use is the watched proof.
10. **The base commit is fixed at admission**, when the repository's configuration is read,
    not when a queued unit starts. A unit that waits long for a slot runs from an older base
    than it might; the trial merge catches a conflict, and nothing catches mere staleness.
11. **Thirteen departures from the supervisor spec** are listed in lane CC-SUPERVISOR and are
    not yet in the spec. The spec is this repository's and public; amending it is a change
    for whoever merges pull request #96, not for a lane.
12. **Swarms and `mode: "real"` are untouched.** The supervisor spec's 503 for a demo swarm
    when the scripted harness is unhealthy is not built, because swarms stay on the driver
    path. `server::router` and `AppState::new` stay public and take no token; the program
    does not serve them, and they go when the driver path goes.
13. **The shipped binary links two test doubles** (`MemForge`, `ScriptedVerifier`) for demo
    mode, through request R5. That is the supervisor spec's decision; it is repeated here
    because a reader of the manifest will ask.
14. **Milestone 0 has not run.** If its lanes land names, line numbers or counts that differ
    from what its plans print, Part 1's Task 1 is where that shows: it checks the branch
    point before anything is written.
15. **The wording table in lane CC-SUPERVISOR's notes goes further than the tests.** The tests
    pin the reasons and a fragment of each error; the rest of each text is this plan's choice
    and can change without an interface change.
