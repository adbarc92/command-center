# Factory M0: workspace scaffold — Implementation Plan

> **For agentic workers:** steps use checkbox (`- [ ]`) syntax for tracking. This plan has four
> lanes. Each lane is executed by one agent, in its own git worktree, and ends with a pull request
> that the agent does not merge. Lane CC-COORD runs first and alone; the other three start only
> after its pull request has merged into `factory/m0`. Lane CC-FORMS has a second part that
> waits for four lanes of the sibling plans.

**Goal:** Give `command-center` the workspace that the factory build needs before any behaviour
moves: an empty skeleton of every new crate, one command per test tier, a dependency-direction
check, a gate registry with a parity check, the CI jobs that run them, tests that pin how
today's `fleetd` behaves, the fix for the flaky required test (#72), and the Form documents for
the crates milestone 1 builds.

**Architecture:** A coordinating lane creates every shared file once (the workspace manifest,
the lockfile, CI, the `xtask` task runner, the registry), so that every later lane owns a
disjoint set of files and never edits a manifest. `cargo xtask test <tier>` is the single
definition of each tier; CI jobs and developers both call it. `fleetd` itself is not
restructured: its behaviour is pinned from outside, through its HTTP routes and its store,
before milestone 1 moves any of it.

**Tech Stack:** Rust 2021 on the stable toolchain (1.93 on the build host); cargo workspaces;
`serde_json` (the task runner's only dependency); Node 20 or later for the parity script and
its `node:test` suite (Node is already installed by CI for the cockpit); GitHub Actions; the
`gh` CLI.

**Spec:** The ReqDrive factory design v0.4, in the private nexus repository: https://github.com/adbarc92/nexus/blob/main/docs/specs/2026-10-04-reqdrive-factory-design.md

**Words used here.** A *lane* is one agent's exclusive set of files. A *skeleton crate* is a
crate that compiles and has one passing test but no behaviour. A *tier* is a named group of
tests with one command to run it. A *Form* is the written contract for one crate (format in
[Lane CC-FORMS](#lane-cc-forms)). A *gate* is a check that fails, and blocks, when a rule is
broken. A *characterisation test* asserts what existing code does today, right or wrong, so
that a later move can be shown to change nothing.

## Global Constraints

These are decisions already made. A lane follows them and does not reopen them.

**Repository and conduct**

- `command-center` is public. Never paste text from the private design, doctrine or templates
  into it. Cite doctrine principles by id only (for example K4, C1).
- Work only in the worktree your lane names. Never edit, commit in, stash in or switch branches
  in `D:\MajorProjects\INFRASTRUCTURE\command-center`: it holds the owner's uncommitted work.
  `git -C <that path> fetch`, `push` of a new branch and `worktree add` are allowed, because
  they do not touch its working tree.
- Write only the files under your lane's **Owns**. A change you need elsewhere goes in your
  final report as a request to the coordinator.
- Commit messages and pull-request bodies carry no `Co-Authored-By` line and no "Generated
  with" footer. Never force-push. Never push to `main`. Never merge your own pull request.
- A hook checks `gh pr create`. If it denies your pull request, report the denial word for
  word and stop. Do not work around it.
- Spend nothing and reach nothing live: no deploys, no real API keys, no posts to an external
  service. The only outside contact is cargo downloading crates, pushing your branch and
  opening your pull request.
- Before claiming a lane is done, run every command under its **Verify** and report the real
  output, including failures. Before parsing a tool's output, check that it is not empty.
- The build host is Windows 11 with PowerShell 7 and Git-Bash. A command block marked
  *Git-Bash* uses POSIX syntax and must be run there. Unmarked `git`, `cargo`, `gh` and `node`
  commands work in either shell. Do not mix the two syntaxes in one command.

**What milestone 0 builds in this repository**

- Ten new library crates, created empty: `factory-spec`, `factory-presets`, `fleet-store`,
  `fleet-runner`, `fleet-forge`, `fleet-verify`, `fleet-supervisor`, `fleet-admission`,
  `fleet-schedule`, `fleet-api`. Each declares a `testkit` cargo feature and has one trivial
  passing test, so that its CI job exists from the first day. One binary crate, `xtask`.
- `fleet-core`, `harness-protocol`, `harness-conformance` and `fleetd` already exist.
  **`fleetd` is not restructured in milestone 0.** No file under `crates/fleetd/src/` changes,
  except the test module that lane CC-72 touches.

**Dependency direction** (enforced by `cargo xtask test static`)

- `fleet-core`, `factory-spec` and `factory-presets` do no I/O: they depend on no crate that
  does (no `tokio`, `rusqlite`, `axum`, `reqwest`, and no process or filesystem crate).
- Only `fleet-runner` may depend on a crate that talks to Docker.
- Only `fleetd` may depend on every other crate. Nothing depends on `fleetd` or on `xtask`.
- Every workspace crate has one row in `xtask/src/deps.rs`. A crate with no row fails the
  check.

**Shared files belong to the coordinator**

- Lane CC-COORD owns the root `Cargo.toml`, `Cargo.lock`, every crate's `Cargo.toml`,
  `.cargo/config.toml`, `.github/workflows/ci.yml`, `xtask/**`, `forms/registry.md`,
  `forms/README.md` and `scripts/parity.*`.
- CC-COORD declares, in its own pull request, every dependency the milestone-0 and
  milestone-1 lanes will need. No later lane edits a manifest or the lockfile. A lane that
  needs a dependency asks the coordinator.
- `cargo xtask` passes `--locked` to cargo, so a manifest change that was not accompanied by
  a lockfile change fails instead of being resolved silently.

**Test tiers** — one command each: `cargo xtask test <tier>`

| Tier | Runs in milestone 0 | Needs |
|---|---|---|
| `static` | `cargo fmt --check` and `cargo clippy -D warnings` for both cargo workspaces; the dependency-direction check; the CI job-name pin; the parity script's own tests; registry parity | Node; the cockpit sidecar unless `--root-only` |
| `unit` | `cargo test -p <crate> --lib`, one crate per command (`--bins` for a crate with no library), and a second run with `--features testkit` for a crate that declares that feature | Nothing: no Docker, no network, no token |
| `contract` | The `tests/` targets the naming rule below places here, then the conformance kit against `harness-fake` | Nothing |
| `integration` | The `tests/` targets the naming rule places here | Docker for some, which stay `#[ignore]`d |
| `e2e` | The `tests/` targets the naming rule places here | Empty in milestone 0 |
| `live` | The `tests/` targets the naming rule places here | Empty in milestone 0 |

- A tier with nothing to run prints `xtask: no tests in this tier yet (<tier>)` and succeeds.

**The tier naming rule.** This rule is adopted for the whole program, in every repository
that has these tiers. It is stated here once; other plans refer to it.

> A test inside `src/` is a `unit` test. A test target under `tests/` is placed by its file
> name, and by nothing else:
>
> | File name under `tests/` | Tier |
> |---|---|
> | `e2e_*.rs` | `e2e` |
> | `live_*.rs` | `live` |
> | `*_it.rs` or `characterisation_*.rs` | `integration` |
> | any other name | `contract` |
>
> The first row that matches wins. A test that needs Docker, a network or a token must not
> be a `unit` or a `contract` test, so it lives under `tests/` in a file named for one of
> the first three rows.

The rule is one function, `tier_of_test_target` in `xtask/src/tiers.rs` (CC-COORD Task 5).

**CI**

- Every existing job is kept. `cargo test (workspace)` and `harness conformance` keep their
  names exactly, because branch protection refers to them.
- Added: `static`; a per-crate `unit` matrix on `ubuntu-latest` and `windows-latest`, built
  from the workspace so a new crate gets a job automatically; `unit (all crates)`, the one
  name that stands for the whole matrix; `contract` on both operating systems;
  `integration` on Linux. With these the list of job names is final for milestone 0
  (CC-COORD Task 8 has the full table).
- **This plan does not change branch protection.** It lists the names; the owner applies them.

**Registry parity ("gate zero")**

- `forms/registry.md` has one row per gate: `id`, `form`, `mechanism`, `location`, `command`.
- `scripts/parity.mjs` fails when: the registry has no rows; a row's command is empty or is
  not a command CI runs; a row's location does not exist; a Form lists a gate that is not in
  the registry; or a Form invariant has neither a gate nor a line under that Form's
  `Unenforced` heading.

**Characterisation tests**

- They go through `fleetd`'s public API (`fleetd::server::router`, `reconcile_on_startup`,
  `fleetd::driver::run`) or its store, on the scripted runner and the fake forge only. No
  Docker, no network, no token.
- They assert what the code does today. Where that is a known defect, the test says so and
  names the issue, so the fix has to change the test on purpose.
- Where a test already pins a behaviour, it is cited by name and not repeated.
- No assertion may depend on how far a background driver has run. Issue #72 was a test that
  did.

**Baseline.** On `origin/main` at `a3fd1c8`, `cargo test --workspace` gives 166 passed,
0 failed, 3 ignored. Each lane's **Verify** states the count it expects after its own work.

## Owner actions this plan depends on

The plan creates none of these. Each is something only the owner does.

**Open questions for the owner.** Three things this plan does not decide. Each has a default
that the lanes follow until the owner says otherwise.

| # | Question | What the plan does meanwhile |
|---|---|---|
| Q1 | **Where does lane CC-72's pull request go?** Into `factory/m0`, like every other lane, or straight into `main`? The fix is test code only, the flaky test fails `main`'s required check today, and a pull request into `factory/m0` does not close #72 by itself. | Opens it against `factory/m0`. |
| Q2 | **Is `factory/m0` protected?** Without required checks on it, a lane pull request can be merged while a check is red. | Assumes it is not, and tells each lane to report its checks and not to merge. |
| Q3 | **Is `fmt + clippy` retired once `static` is required?** `static` runs the same two commands for both cargo workspaces, and more, so the older job then duplicates it. | Keeps `fmt + clippy`, unchanged and still pinned by name. Retiring it means removing its name from `xtask/src/ci.rs` and from branch protection in the same change. |

| # | Action | Needed by |
|---|---|---|
| 1 | Give the go-ahead for lane CC-COORD, then for the three lanes after it. | Every lane |
| 2 | Merge the pull request that adds this plan and its sibling plans to `main` before CC-COORD starts, so that `factory/m0` is cut from a `main` that contains them. | CC-COORD Task 11; CC-FORMS |
| 3 | Pull the fixed pull-request hook into the live hooks checkout. Until then `gh pr create` may be denied for lane CC-72. | CC-72 Task 3 |
| 4 | Answer the three open questions above. | Q1: CC-72 Task 3. Q2: every lane's merge. Q3: action 7 |
| 5 | Approve the Form documents from lane CC-FORMS: seven for the crates milestone 1 builds, and four for the contract crates. Approval is changing `status: draft` to `status: frozen` in each file, by the owner or in a commit the owner asks for. | Milestone 1; the contract crates' release tag |
| 6 | Merge `factory/m0` into `main` once the milestone is reviewed. | Closing #72; action 7 |
| 7 | After every job below has been green on `main`, change branch protection on `main` to require: `cargo test (workspace)`, `harness conformance`, `fmt + clippy`, `svelte-check + tsc`, `vitest (cockpit/ui)`, `cargo test (cockpit)`, `static`, `unit (all crates)`, `contract (ubuntu-latest)`, `contract (windows-latest)`, `integration`. The three `tauri build (...)` jobs and the per-crate `unit (<crate>, <os>)` jobs are not on this list: `unit (all crates)` stands for the matrix. | Issue #95 |
| 8 | Do not make `unit (all crates)` required before lane CC-72's fix is on `main`: the flaky test of #72 is one of `fleetd`'s unit tests, so `unit (fleetd, ubuntu-latest)` and `unit (fleetd, windows-latest)` inherit the flake until then. | Action 7 |
| 9 | Close #72 when its fix reaches `main` (a pull request into `factory/m0` does not close an issue by itself). Leave #74 open: this plan records the defect; milestone 1 fixes it. Decide what remains of #95: this plan adds Windows jobs for the `unit` and `contract` tiers, and does not run `fleetd`'s `tests/` targets on Windows. | Issue hygiene |
| 10 | Optionally delete the remote branch `fix/flaky-cap-race-test` after CC-72's pull request merges; its two commits are carried by `fix/m0-flaky-cap-race-test`. | Tidiness |

## Review Focus

Five ways this work is most likely to go wrong without any task noticing. Each has a test or
a check in the task that owns it.

| # | Failure mode | Where it is caught |
|---|---|---|
| 1 | **The task runner or the parity script works on Linux and breaks on Windows**: a path joined with `/`, a binary run without `.exe`, a file read with CRLF line endings, or a script that does not recognise it is the main module when its path has a drive letter. | CC-COORD Task 5: `paths_are_built_from_components_not_from_separators`, `contract_ends_by_running_the_kit_against_the_fake`. Task 6: `crlf_line_endings_and_quoted_names_still_match`. Task 9: `a CRLF checkout gives the same answer as an LF one` and the two command-line tests, which start the script as a child process. Task 8: the `unit` matrix and `contract` run on `windows-latest`. Every lane's **Verify** runs on the Windows build host. |
| 2 | **A crate added later is silently left out**: of the per-crate matrix, or of the dependency rules. | Task 8: the matrix is built from `cargo xtask crates`, not typed into the workflow, and `unit (all crates)` fails if the matrix was skipped. Task 4: `a_crate_with_no_rule_fails`. Task 7: `the_real_workspace_satisfies_every_static_check` asserts one unit command and one rule per workspace crate. |
| 3 | **A CI job is renamed, so branch protection waits for a name that no longer reports**, and the requirement stops binding without anyone seeing a failure. | Task 6: `xtask/src/ci.rs` pins every job name; `a_renamed_job_fails`; `an_empty_workflow_fails_rather_than_passing_vacuously`. It runs in `static` on every pull request. |
| 4 | **The parity check passes on an empty registry**, or on one whose table header is misspelled so that zero rows are read. | Task 9: `a registry with a header and no rows fails`, `a registry whose header is misspelled fails instead of reading zero rows`, and `this repository's own registry passes and is not empty`. |
| 5 | **A characterisation test races the demo driver it started**, passes locally and fails one run in fifty on CI, on a check that is about to be required. This is exactly what #72 was. | CC-CHAR: every seeded amount is chosen so the answer is the same whether an earlier unit is still running or has finished; every wait has a deadline; each task runs its file twenty times before committing. |

---

## Lane CC-COORD

**Owns:** `Cargo.toml`, `Cargo.lock`, `.cargo/config.toml`, `.github/workflows/ci.yml`,
`xtask/**`, `forms/registry.md`, `forms/README.md`, `scripts/parity.mjs`,
`scripts/parity.test.mjs`; the first two files (`Cargo.toml`, `src/lib.rs`) of each new crate:
`factory-spec`, `factory-presets`, `fleet-store`, `fleet-runner`, `fleet-forge`,
`fleet-verify`, `fleet-supervisor`, `fleet-admission`, `fleet-schedule`, `fleet-api`; and every
crate manifest `crates/*/Cargo.toml`. It edits `fleet-core`'s, `harness-protocol`'s and
`harness-conformance`'s manifests in Task 2, and any other manifest only to satisfy a
dependency request found in Task 11.
Once this lane has merged, each crate's `src/` and `tests/` belong to the lane that builds
that crate; the manifests stay with the coordinator.

**Reads:** this plan; `.github/workflows/ci.yml`; the "Requests to the coordinator" sections
of `docs/superpowers/plans/2026-10-04-factory-m0-contracts.md` and
`2026-10-04-factory-m0-spec-and-presets.md`, whose requests are already written into Task 2;
and of `2026-10-04-factory-m1-control-plane.md` and `2026-10-04-factory-m1-cockpit.md`, if
they exist by then.

**Worktree and branch:** creates the integration branch `factory/m0` from `origin/main` and
pushes it. Works in `D:\MajorProjects\.swarm-wt\m0-cc-coord` on `feat/m0-scaffold`. Pull
request: `feat/m0-scaffold` into `factory/m0`.

**Needs:** owner actions 1 and 2.

**Blocks:** every other milestone-0 lane in this repository: CC-CHAR, CC-72 and CC-FORMS
below, and the lanes of the two sibling plans.

**Verify** (run in the worktree, after Task 11; expected results beside each):

| Command | Expected |
|---|---|
| *Git-Bash:* `cargo test --workspace --locked 2>&1 \| grep -E "^test result" \| awk '{p+=$4; f+=$6; i+=$8} END {print "passed="p" failed="f" ignored="i}'` | `passed=209 failed=0 ignored=3` (166 baseline, 10 skeleton tests, 33 `xtask` tests) |
| `cargo xtask test static --root-only` | last line `xtask: static: 6 step(s) passed`; the output contains `parity: ok (4 gates, 0 forms)` |
| `cargo xtask test unit` | last line `xtask: unit: 27 step(s) passed` (15 crates, and a second run with the `testkit` feature for the 12 that declare it) |
| `cargo xtask test contract` | six `PASS` lines from the conformance kit, then `xtask: contract: 4 step(s) passed`. Six is the count in this lane's state, before lane CC-PROTO has merged. Once CC-PROTO is in `factory/m0` the kit has a seventh case and prints seven |
| `cargo xtask test integration` | `xtask: integration: 1 step(s) passed` |
| `cargo xtask test e2e` | `xtask: no tests in this tier yet (e2e)` |
| `cargo xtask test live` | `xtask: no tests in this tier yet (live)` |
| `cargo xtask crates` | one line: a JSON array of the 15 crate names, starting `["factory-presets","factory-spec",` |
| `cargo xtask test smoke`; then print the exit code | the usage text; exit code 2 |
| `git diff --name-only origin/factory/m0...HEAD` | only paths under **Owns** |

The cockpit half of `static` (`cargo xtask test static`, without `--root-only`) needs
`npm ci` and `npm run sidecar` in `cockpit/ui` and compiles the Tauri crate. CI runs it. Run
it locally only if you changed something under `cockpit/`; this lane does not.

### Task 1: Create the integration branch and the worktree

**Files:** none in the repository.

**Interfaces:**
- Consumes: `origin/main`.
- Produces: the remote branch `factory/m0`; the worktree
  `D:\MajorProjects\.swarm-wt\m0-cc-coord` on `feat/m0-scaffold`.

- [ ] **Step 1: Check that the integration branch does not exist yet**

```
git -C D:/MajorProjects/INFRASTRUCTURE/command-center fetch origin
git -C D:/MajorProjects/INFRASTRUCTURE/command-center ls-remote --heads origin factory/m0
```

Expected: the second command prints nothing. If it prints a line, the branch already exists:
skip Step 2 and continue from Step 3.

- [ ] **Step 2: Create `factory/m0` from `origin/main` and push it**

```
git -C D:/MajorProjects/INFRASTRUCTURE/command-center push origin origin/main:refs/heads/factory/m0
git -C D:/MajorProjects/INFRASTRUCTURE/command-center fetch origin
```

Expected: `* [new branch]      origin/main -> factory/m0`. This creates a branch; it rewrites
nothing.

- [ ] **Step 3: Create the worktree on a new branch that does not track `factory/m0`**

```
git -C D:/MajorProjects/INFRASTRUCTURE/command-center worktree add --no-track -b feat/m0-scaffold D:/MajorProjects/.swarm-wt/m0-cc-coord origin/factory/m0
```

`--no-track` matters: without it the new branch's upstream would be `factory/m0`, and a plain
`git push` would be refused or, worse, aimed at the integration branch.

Every later command in this lane runs in `D:\MajorProjects\.swarm-wt\m0-cc-coord`.

- [ ] **Step 4: Record the baseline**

*Git-Bash:*

```bash
cargo test --workspace 2>&1 | grep -E "^test result" | awk '{p+=$4; f+=$6; i+=$8} END {print "passed="p" failed="f" ignored="i}'
```

Expected: `passed=166 failed=0 ignored=3`.

If exactly one test fails and it is
`server::tests::concurrent_missions_cannot_both_breach_the_cap`, that is issue #72: run the
command once more and record both results. Any other failure: stop and report.

### Task 2: The workspace manifest, the skeleton crates and the shared dependencies

**Files:**
- Modify: `Cargo.toml`, `crates/fleet-core/Cargo.toml`, `crates/harness-protocol/Cargo.toml`,
  `crates/harness-conformance/Cargo.toml`
- Create: `.cargo/config.toml`, `xtask/Cargo.toml`, `xtask/src/main.rs`
- Create: `Cargo.toml` and `src/lib.rs` in each of the ten new crate directories
- Regenerate: `Cargo.lock`

**Interfaces:**
- Consumes: nothing.
- Produces: fifteen workspace members; `[workspace.dependencies]` entries for `serde`,
  `serde_json`, `thiserror`, `tokio`, `async-trait`, `axum`, `dotenvy`, `rusqlite`, `schemars`
  and `sha2`; a `testkit` feature on every library crate except `harness-conformance`; the
  `cargo xtask` alias; and every dependency the two sibling plans ask for.

**Requests from the sibling plans, declared in this task.** Each line below is taken from
the "Requests to the coordinator" section of the plan named. Task 11 checks them again.

| Request | Crate | Dependency, exactly as asked | Kind | Declared in |
|---|---|---|---|---|
| contracts R1 | `harness-protocol` | `sha2 = { workspace = true }` | normal | Step 3 |
| contracts R2 | `harness-conformance` | `fleet-core = { path = "../fleet-core" }` | dev | Step 5 |
| contracts R3 | `harness-conformance` | `serde_json = { workspace = true }` | normal | Step 5 |
| spec-and-presets 2 | `factory-spec` | `serde-saphyr = { version = "1.3", default-features = false, features = ["deserialize"] }` | normal | Step 7 |
| spec-and-presets 2 | `factory-spec` | `sha2 = { workspace = true }` | normal | Step 7 |
| spec-and-presets 2 | `factory-spec` | `serde_json = { workspace = true }` | dev | Step 7 |
| spec-and-presets 3 | `factory-presets` | `serde = { workspace = true }`, `serde_json = { workspace = true }` | normal | Step 7 |
| spec-and-presets 3 | `factory-presets` | `quick-xml = "0.42"` | normal | Step 7 |
| spec-and-presets 3 | `factory-presets` | `toml = { version = "1", default-features = false, features = ["std", "parse", "serde"] }` | normal | Step 7 |
| spec-and-presets 3 | `factory-presets` | `regex = "1"` | dev | Step 7 |
| spec-and-presets 5 | both | run each crate's tests with and without `--features testkit`, on Linux and on Windows | CI | Task 5 (the `unit` tier) and Task 8 (the `unit` matrix) |

The spec-and-presets plan gives the two manifests in full and says that everything but the
`description` line must match. Step 7 uses its text, `description` included. The four
parser and test crates are therefore declared with their versions in those two manifests,
as that plan writes them, and not in `[workspace.dependencies]`.

**How the dependency-direction check treats the new crates.** `factory-spec` and
`factory-presets` are pure, so each of their direct, normal dependencies has to be on that
crate's allowlist in `xtask/src/deps.rs`. Task 4 puts `serde-saphyr` on `factory-spec`'s
list and `quick-xml` and `toml` on `factory-presets`'s. They are parsers: they turn text
they are handed into data, and open no file, socket or process. Being on the list is not a
blanket pass: the check still walks everything each of them pulls in and fails if any of it
is in `IO_CRATES`. `regex` and `factory-spec`'s `serde_json` are dev-dependencies, which
the check leaves free. `harness-conformance`'s dev-dependency on `fleet-core` needs no rule
change for the same reason.

What each new crate declares now, and why:

| Crate | Workspace crates it may use | External dependencies declared now | Why |
|---|---|---|---|
| `factory-spec` | none | `serde`, `serde-saphyr`, `sha2`; dev: `serde_json` | No I/O. Reads a YAML header and hashes the exact bytes of a spec. As the spec-and-presets plan asks. |
| `factory-presets` | none | `serde`, `serde_json`, `quick-xml`, `toml`; dev: `regex` | No I/O. Reads reports and manifests given to it as text. As the spec-and-presets plan asks. |
| `fleet-store` | `fleet-core` | `serde`, `serde_json`, `thiserror`, `rusqlite` | Takes over `fleetd/src/store.rs`, which uses these today. |
| `fleet-runner` | `fleet-core` | `serde`, `serde_json`, `thiserror`, `tokio`, `async-trait` | Takes over `runner.rs` and `local_docker.rs`. |
| `fleet-forge` | none | `serde`, `serde_json`, `thiserror`, `tokio`, `async-trait` | Takes over `forge.rs` and `gh_forge.rs`. |
| `fleet-verify` | `fleet-core`, `harness-protocol`, `factory-spec`, `factory-presets` | `serde`, `serde_json`, `thiserror`, `tokio`, `async-trait` | New. Reads harness evidence and applies the spec and preset rules. |
| `fleet-supervisor` | `fleet-core`, `harness-protocol` | `serde`, `serde_json`, `thiserror`, `tokio`, `async-trait` | New. Speaks the protocol to one harness process. |
| `fleet-admission` | `fleet-core`, `factory-spec` | `serde`, `serde_json`, `thiserror` | Takes over the admission part of `server.rs`. |
| `fleet-schedule` | `fleet-core` | `serde`, `serde_json`, `thiserror`, `tokio`, `async-trait` | New. Built in a later milestone; created now so no later lane touches the manifest. |
| `fleet-api` | `fleet-core`, `harness-protocol` | `serde`, `serde_json`, `thiserror`, `tokio`, `async-trait`, `axum`, `schemars` | Takes over the routes of `server.rs`. |

Task 11 checks that nothing a plan asks for is missing, and is where a request that appears
later (from a milestone-1 plan) is added.

- [ ] **Step 1: Replace the root `Cargo.toml`**

```toml
[workspace]
resolver = "2"
members = [
    "crates/factory-presets",
    "crates/factory-spec",
    "crates/fleet-admission",
    "crates/fleet-api",
    "crates/fleet-core",
    "crates/fleet-forge",
    "crates/fleet-runner",
    "crates/fleet-schedule",
    "crates/fleet-store",
    "crates/fleet-supervisor",
    "crates/fleet-verify",
    "crates/fleetd",
    "crates/harness-conformance",
    "crates/harness-protocol",
    "xtask",
]

[workspace.package]
edition = "2021"
version = "0.1.0"
license = "MIT"

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
tokio = { version = "1", features = ["full"] }
async-trait = "0.1"
axum = { version = "0.7", features = ["ws"] }
dotenvy = "0.15"
rusqlite = { version = "0.32", features = ["bundled"] }
schemars = "1.2"
sha2 = "0.10"
```

- [ ] **Step 2: Create `.cargo/config.toml`**

```toml
[alias]
xtask = "run --package xtask --"
```

- [ ] **Step 3: Replace `crates/harness-protocol/Cargo.toml`**

`schemars` and `sha2` move to the workspace table. `sha2` becomes a normal dependency (it was
a dev-dependency), because the protocol crate will compute hashes itself. A `testkit` feature
is added.

```toml
[package]
name = "harness-protocol"
edition.workspace = true
version.workspace = true
license.workspace = true
description = "Wire types for the command-center harness protocol: JSON-RPC 2.0 over stdio."

[features]
# Builders for this crate's own types, for other crates' tests.
testkit = []

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
schemars = { workspace = true }
sha2 = { workspace = true }
```

- [ ] **Step 4: Replace `crates/fleet-core/Cargo.toml`**

Only the `testkit` feature is new.

```toml
[package]
name = "fleet-core"
edition.workspace = true
version.workspace = true
license.workspace = true

[features]
# Builders for this crate's own types, for other crates' tests.
testkit = []

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
```

- [ ] **Step 5: Replace `crates/harness-conformance/Cargo.toml`**

Two lines are new: `serde_json` under `[dependencies]` (contracts R3), and a
`[dev-dependencies]` table holding `fleet-core` (contracts R2).

```toml
[package]
name = "harness-conformance"
edition.workspace = true
version.workspace = true
license.workspace = true
description = "Conformance kit for harness-protocol, plus harness-fake, a scripted harness."

[dependencies]
harness-protocol = { path = "../harness-protocol" }
serde_json = { workspace = true }

[dev-dependencies]
fleet-core = { path = "../fleet-core" }

[[bin]]
name = "harness-fake"
path = "src/bin/harness-fake.rs"

[[bin]]
name = "harness-conformance"
path = "src/bin/harness-conformance.rs"
```

- [ ] **Step 6: Create the `xtask` crate as a stub**

`xtask/Cargo.toml`:

```toml
[package]
name = "xtask"
edition.workspace = true
version.workspace = true
license.workspace = true
description = "Repository tasks: the test tiers, the dependency-direction check, the CI job-name pin."
publish = false

[dependencies]
serde_json = { workspace = true }
```

`xtask/src/main.rs` (Tasks 3 to 6 add modules to it; Task 7 replaces it):

```rust
//! `cargo xtask` — the repository's task runner. The next tasks build it up.
#![allow(dead_code)] // removed in Task 7, when `main` uses every module

fn main() {}
```

- [ ] **Step 7: Create the ten skeleton crates**

Each is two files. Create all twenty exactly as shown. The manifests of `factory-spec` and
`factory-presets` are the ones the spec-and-presets plan gives, which is why they have no
comment line under `[features]`.

`crates/factory-spec/Cargo.toml`:

```toml
[package]
name = "factory-spec"
edition.workspace = true
version.workspace = true
license.workspace = true
description = "The factory spec format: parsing, validation, the split, and the scope and overlap rules. No I/O."

[features]
testkit = []

[dependencies]
serde = { workspace = true }
serde-saphyr = { version = "1.3", default-features = false, features = ["deserialize"] }
sha2 = { workspace = true }

[dev-dependencies]
serde_json = { workspace = true }
```

`crates/factory-spec/src/lib.rs`:

```rust
//! `factory-spec` — The work-spec format: parsing, validation, the split into child specs, and the scope and overlap rules. No I/O.
//!
//! Skeleton only: this crate holds no behaviour yet. Its interface is fixed by
//! `forms/factory-spec.md` once that Form is approved.

/// Builders for this crate's own types, for other crates' tests.
#[cfg(feature = "testkit")]
pub mod testkit {}

#[cfg(test)]
mod tests {
    #[test]
    fn skeleton_builds() {}
}
```

`crates/factory-presets/Cargo.toml`:

```toml
[package]
name = "factory-presets"
edition.workspace = true
version.workspace = true
license.workspace = true
description = "Per-stack rules for the cargo and node presets: reports, test ids, protected paths, manifests, secret rules. No I/O."

[features]
testkit = []

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
quick-xml = "0.42"
toml = { version = "1", default-features = false, features = ["std", "parse", "serde"] }

[dev-dependencies]
regex = "1"
```

`crates/factory-presets/src/lib.rs`:

```rust
//! `factory-presets` — Per-stack rules: reading test reports and manifests, enumerating test ids, and protected-path patterns. No I/O.
//!
//! Skeleton only: this crate holds no behaviour yet. Its interface is fixed by
//! `forms/factory-presets.md` once that Form is approved.

/// Builders for this crate's own types, for other crates' tests.
#[cfg(feature = "testkit")]
pub mod testkit {}

#[cfg(test)]
mod tests {
    #[test]
    fn skeleton_builds() {}
}
```

`crates/fleet-store/Cargo.toml`:

```toml
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
```

`crates/fleet-store/src/lib.rs`:

```rust
//! `fleet-store` — Control-plane persistence: the event log and its projections.
//!
//! Skeleton only: this crate holds no behaviour yet. Its interface is fixed by
//! `forms/fleet-store.md` once that Form is approved.

/// Builders for this crate's own types, for other crates' tests.
#[cfg(feature = "testkit")]
pub mod testkit {}

#[cfg(test)]
mod tests {
    #[test]
    fn skeleton_builds() {}
}
```

`crates/fleet-runner/Cargo.toml`:

```toml
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
```

`crates/fleet-runner/src/lib.rs`:

```rust
//! `fleet-runner` — Check containers for the verifier, and reaping containers by label. The only crate that may reach Docker.
//!
//! Skeleton only: this crate holds no behaviour yet. Its interface is fixed by
//! `forms/fleet-runner.md` once that Form is approved.

/// Builders for this crate's own types, for other crates' tests.
#[cfg(feature = "testkit")]
pub mod testkit {}

#[cfg(test)]
mod tests {
    #[test]
    fn skeleton_builds() {}
}
```

`crates/fleet-forge/Cargo.toml`:

```toml
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
```

`crates/fleet-forge/src/lib.rs`:

```rust
//! `fleet-forge` — Git and GitHub work done on the host: bundles, push, pull requests and mergeability.
//!
//! Skeleton only: this crate holds no behaviour yet. Its interface is fixed by
//! `forms/fleet-forge.md` once that Form is approved.

/// Builders for this crate's own types, for other crates' tests.
#[cfg(feature = "testkit")]
pub mod testkit {}

#[cfg(test)]
mod tests {
    #[test]
    fn skeleton_builds() {}
}
```

`crates/fleet-verify/Cargo.toml`:

```toml
[package]
name = "fleet-verify"
edition.workspace = true
version.workspace = true
license.workspace = true
description = "The control plane's own verification of what a harness reports."

[features]
# Builders for this crate's own types, for other crates' tests.
testkit = []

[dependencies]
fleet-core = { path = "../fleet-core" }
harness-protocol = { path = "../harness-protocol" }
factory-spec = { path = "../factory-spec" }
factory-presets = { path = "../factory-presets" }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tokio = { workspace = true }
async-trait = { workspace = true }
```

`crates/fleet-verify/src/lib.rs`:

```rust
//! `fleet-verify` — The control plane's own verification of what a harness reports.
//!
//! Skeleton only: this crate holds no behaviour yet. Its interface is fixed by
//! `forms/fleet-verify.md` once that Form is approved.

/// Builders for this crate's own types, for other crates' tests.
#[cfg(feature = "testkit")]
pub mod testkit {}

#[cfg(test)]
mod tests {
    #[test]
    fn skeleton_builds() {}
}
```

`crates/fleet-supervisor/Cargo.toml`:

```toml
[package]
name = "fleet-supervisor"
edition.workspace = true
version.workspace = true
license.workspace = true
description = "Supervises one harness process over a byte stream."

[features]
# Builders for this crate's own types, for other crates' tests.
testkit = []

[dependencies]
fleet-core = { path = "../fleet-core" }
harness-protocol = { path = "../harness-protocol" }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tokio = { workspace = true }
async-trait = { workspace = true }
```

`crates/fleet-supervisor/src/lib.rs`:

```rust
//! `fleet-supervisor` — Supervises one harness process over a byte stream.
//!
//! Skeleton only: this crate holds no behaviour yet. Its interface is fixed by
//! `forms/fleet-supervisor.md` once that Form is approved.

/// Builders for this crate's own types, for other crates' tests.
#[cfg(feature = "testkit")]
pub mod testkit {}

#[cfg(test)]
mod tests {
    #[test]
    fn skeleton_builds() {}
}
```

`crates/fleet-admission/Cargo.toml`:

```toml
[package]
name = "fleet-admission"
edition.workspace = true
version.workspace = true
license.workspace = true
description = "Decides whether a unit may start: spend, concurrency, signature, eligibility and scope."

[features]
# Builders for this crate's own types, for other crates' tests.
testkit = []

[dependencies]
fleet-core = { path = "../fleet-core" }
factory-spec = { path = "../factory-spec" }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
```

`crates/fleet-admission/src/lib.rs`:

```rust
//! `fleet-admission` — Decides whether a unit may start: spend, concurrency, signature, eligibility and scope.
//!
//! Skeleton only: this crate holds no behaviour yet. Its interface is fixed by
//! `forms/fleet-admission.md` once that Form is approved.

/// Builders for this crate's own types, for other crates' tests.
#[cfg(feature = "testkit")]
pub mod testkit {}

#[cfg(test)]
mod tests {
    #[test]
    fn skeleton_builds() {}
}
```

`crates/fleet-schedule/Cargo.toml`:

```toml
[package]
name = "fleet-schedule"
edition.workspace = true
version.workspace = true
license.workspace = true
description = "Scheduling of auxiliary runs."

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
```

`crates/fleet-schedule/src/lib.rs`:

```rust
//! `fleet-schedule` — Scheduling of auxiliary runs.
//!
//! Skeleton only: this crate holds no behaviour yet. Its interface is fixed by
//! `forms/fleet-schedule.md` once that Form is approved.

/// Builders for this crate's own types, for other crates' tests.
#[cfg(feature = "testkit")]
pub mod testkit {}

#[cfg(test)]
mod tests {
    #[test]
    fn skeleton_builds() {}
}
```

`crates/fleet-api/Cargo.toml`:

```toml
[package]
name = "fleet-api"
edition.workspace = true
version.workspace = true
license.workspace = true
description = "HTTP and WebSocket routes, authentication and schema export."

[features]
# Builders for this crate's own types, for other crates' tests.
testkit = []

[dependencies]
fleet-core = { path = "../fleet-core" }
harness-protocol = { path = "../harness-protocol" }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tokio = { workspace = true }
async-trait = { workspace = true }
axum = { workspace = true }
schemars = { workspace = true }
```

`crates/fleet-api/src/lib.rs`:

```rust
//! `fleet-api` — HTTP and WebSocket routes, authentication and schema export.
//!
//! Skeleton only: this crate holds no behaviour yet. Its interface is fixed by
//! `forms/fleet-api.md` once that Form is approved.

/// Builders for this crate's own types, for other crates' tests.
#[cfg(feature = "testkit")]
pub mod testkit {}

#[cfg(test)]
mod tests {
    #[test]
    fn skeleton_builds() {}
}
```

- [ ] **Step 8: Regenerate the lockfile and check that it only grew**

```
cargo update --workspace
git diff --stat Cargo.lock
```

`cargo update --workspace` adds lock entries for the new workspace members and for the
crates they newly need, and leaves every package that is already locked at its version. It
downloads the registry index, which is the one network access this task makes.

Expected: `Adding` lines for the eleven new workspace members (the ten skeletons and
`xtask`) and for the new registry crates: `serde-saphyr`, `quick-xml`, `toml`, `regex` and
what they depend on. When this plan was written that was 38 `Adding` lines and
`1 file changed, 363 insertions(+), 1 deletion(-)`. The exact numbers can differ if one of
the new crates has published a patch release since; what must hold is the next check.

*Git-Bash:*

```bash
git diff Cargo.lock | grep -E '^-(version|checksum)'
```

Expected: no output. No package that was already locked changed version. (The one removed
line is a reference: an existing entry's `"hashbrown",` becomes `"hashbrown 0.x.y",` because
a second version of that crate enters the graph. That is a rename of a reference, not a
change of what is built.)

- [ ] **Step 9: Run the whole suite against the locked graph**

*Git-Bash:*

```bash
cargo test --workspace --locked 2>&1 | grep -E "^test result" | awk '{p+=$4; f+=$6; i+=$8} END {print "passed="p" failed="f" ignored="i}'
```

Expected: `passed=176 failed=0 ignored=3` (166 plus one test in each of the ten skeletons).

```
cargo xtask
```

Expected: it builds and exits 0 with no output. The alias works.

- [ ] **Step 10: Commit**

```
git add Cargo.toml Cargo.lock .cargo/config.toml xtask crates/fleet-core/Cargo.toml crates/harness-protocol/Cargo.toml crates/harness-conformance/Cargo.toml crates/factory-spec crates/factory-presets crates/fleet-store crates/fleet-runner crates/fleet-forge crates/fleet-verify crates/fleet-supervisor crates/fleet-admission crates/fleet-schedule crates/fleet-api
git commit -m "chore(workspace): add ten skeleton crates, the xtask crate and every requested dependency"
```

### Task 3: `xtask` — read `cargo metadata`

**Files:**
- Create: `xtask/src/meta.rs`
- Modify: `xtask/src/main.rs`

**Interfaces:**
- Consumes: the JSON printed by `cargo metadata --format-version 1 --locked`.
- Produces: `meta::Meta { members: Vec<Package>, target_dir: PathBuf }` with
  `Meta::load(root: &Path) -> Result<Meta, String>`, `Meta::parse(json: &str) -> Result<Meta, String>`,
  `Meta::member(&self, name: &str) -> Option<&Package>`, `Meta::is_member(&self, name: &str) -> bool`,
  `Meta::external_closure(&self, member: &Package) -> BTreeSet<String>`;
  `meta::Package { id, name, features: Vec<String>, targets: Vec<Target>, deps: Vec<Dep> }` with
  `has_lib()` and `test_targets() -> Vec<&str>`; `meta::Dep { name, kind: DepKind }`;
  `meta::DepKind { Normal, Dev, Build }`; and, for tests only, `meta::fixture::metadata(members, external) -> String`.

- [ ] **Step 1: Write the failing tests**

Create `xtask/src/meta.rs` containing only the test fixture and the tests:

```rust
#[cfg(test)]
pub mod fixture {
    //! A hand-built `cargo metadata` document for tests.

    /// One workspace member: `(name, features, test targets, [(dependency, kind)])`.
    pub type Member<'a> = (
        &'a str,
        &'a [&'a str],
        &'a [&'a str],
        &'a [(&'a str, &'a str)],
    );

    /// Build metadata JSON. `external` lists `(crate, [its dependencies])` for crates
    /// outside the workspace. A dependency kind is `"normal"`, `"dev"` or `"build"`.
    pub fn metadata(members: &[Member], external: &[(&str, &[&str])]) -> String {
        let id = |name: &str| format!("path+file:///ws/{name}#0.1.0");
        let mut packages = Vec::new();
        let mut nodes = Vec::new();
        for (name, features, tests, deps) in members {
            let mut targets = vec![serde_json::json!({"name": name, "kind": ["lib"]})];
            for t in *tests {
                targets.push(serde_json::json!({"name": t, "kind": ["test"]}));
            }
            let declared: Vec<_> = deps
                .iter()
                .map(|(d, kind)| {
                    let kind = if *kind == "normal" {
                        serde_json::Value::Null
                    } else {
                        serde_json::json!(kind)
                    };
                    serde_json::json!({"name": d, "kind": kind})
                })
                .collect();
            let features: serde_json::Map<String, serde_json::Value> = features
                .iter()
                .map(|f| (f.to_string(), serde_json::json!([])))
                .collect();
            packages.push(serde_json::json!({
                "id": id(name), "name": name, "features": features,
                "targets": targets, "dependencies": declared,
            }));
            let resolved: Vec<_> = deps
                .iter()
                .map(|(d, kind)| {
                    let kind = if *kind == "normal" {
                        serde_json::Value::Null
                    } else {
                        serde_json::json!(kind)
                    };
                    serde_json::json!({"pkg": id(d), "dep_kinds": [{"kind": kind}]})
                })
                .collect();
            nodes.push(serde_json::json!({"id": id(name), "deps": resolved}));
        }
        for (name, deps) in external {
            packages.push(serde_json::json!({
                "id": id(name), "name": name, "features": {}, "targets": [], "dependencies": [],
            }));
            let resolved: Vec<_> = deps
                .iter()
                .map(|d| serde_json::json!({"pkg": id(d), "dep_kinds": [{"kind": null}]}))
                .collect();
            nodes.push(serde_json::json!({"id": id(name), "deps": resolved}));
        }
        let member_ids: Vec<_> = members.iter().map(|m| id(m.0)).collect();
        serde_json::json!({
            "packages": packages,
            "workspace_members": member_ids,
            "resolve": {"nodes": nodes},
            "target_directory": "/ws/target",
        })
        .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_members_sorted_with_features_and_test_targets() {
        let json = fixture::metadata(
            &[
                ("zeta", &[], &["contract"], &[]),
                (
                    "alpha",
                    &["testkit"],
                    &["b_it", "a_it"],
                    &[("serde", "normal")],
                ),
            ],
            &[("serde", &[])],
        );
        let meta = Meta::parse(&json).unwrap();
        let names: Vec<&str> = meta.members.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, ["alpha", "zeta"]);
        let alpha = meta.member("alpha").unwrap();
        assert_eq!(alpha.features, ["testkit"]);
        assert_eq!(alpha.test_targets(), ["a_it", "b_it"]);
        assert!(alpha.has_lib());
        assert_eq!(alpha.deps[0].kind, DepKind::Normal);
    }

    #[test]
    fn external_closure_follows_transitive_edges_but_not_dev_or_members() {
        let json = fixture::metadata(
            &[
                (
                    "alpha",
                    &[],
                    &[],
                    &[("serde", "normal"), ("tempfile", "dev"), ("beta", "normal")],
                ),
                ("beta", &[], &[], &[("tokio", "normal")]),
            ],
            &[
                ("serde", &["serde_derive"]),
                ("serde_derive", &[]),
                ("tempfile", &[]),
                ("tokio", &["mio"]),
                ("mio", &[]),
            ],
        );
        let meta = Meta::parse(&json).unwrap();
        let closure = meta.external_closure(meta.member("alpha").unwrap());
        let got: Vec<&str> = closure.iter().map(String::as_str).collect();
        assert_eq!(got, ["serde", "serde_derive"]);
    }

    #[test]
    fn refuses_metadata_with_no_members_or_no_graph() {
        assert!(Meta::parse("{}").is_err());
        assert!(Meta::parse("not json").is_err());
        let no_graph = r#"{"packages":[{"id":"a","name":"a"}],"workspace_members":["a"]}"#;
        assert!(Meta::parse(no_graph).is_err());
    }
}
```

Replace `xtask/src/main.rs` with:

```rust
//! `cargo xtask` — the repository's task runner. The next tasks build it up.
#![allow(dead_code)] // removed in Task 7, when `main` uses every module

mod meta;

fn main() {}
```

- [ ] **Step 2: Run the tests and watch them fail**

```
cargo test -p xtask
```

Expected: the crate does not compile. The errors are
``error[E0433]: failed to resolve: use of undeclared type `Meta` `` (five times) and the same
for `DepKind`.

- [ ] **Step 3: Write the implementation**

Put this above the fixture module in `xtask/src/meta.rs`, so the file starts with it:

```rust
//! The parts of `cargo metadata` the tasks read.

use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepKind {
    Normal,
    Dev,
    Build,
}

/// A dependency as a manifest declares it.
#[derive(Debug, Clone)]
pub struct Dep {
    pub name: String,
    pub kind: DepKind,
}

#[derive(Debug, Clone)]
pub struct Target {
    pub name: String,
    pub kinds: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Package {
    pub id: String,
    pub name: String,
    pub features: Vec<String>,
    pub targets: Vec<Target>,
    pub deps: Vec<Dep>,
}

impl Package {
    pub fn has_lib(&self) -> bool {
        self.targets
            .iter()
            .any(|t| t.kinds.iter().any(|k| k == "lib" || k == "rlib"))
    }

    /// Names of this package's integration-test targets (`tests/*.rs`), sorted.
    pub fn test_targets(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self
            .targets
            .iter()
            .filter(|t| t.kinds.iter().any(|k| k == "test"))
            .map(|t| t.name.as_str())
            .collect();
        names.sort_unstable();
        names
    }
}

#[derive(Debug)]
pub struct Meta {
    /// Workspace members, sorted by name.
    pub members: Vec<Package>,
    pub target_dir: PathBuf,
    /// Package id -> name, for every package in the graph.
    names: BTreeMap<String, String>,
    /// Package id -> ids it reaches by an edge that is not dev-only.
    edges: BTreeMap<String, Vec<String>>,
}

impl Meta {
    /// Run `cargo metadata` in `root` and parse it.
    pub fn load(root: &Path) -> Result<Meta, String> {
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
        let out = Command::new(cargo)
            .args(["metadata", "--format-version", "1", "--locked"])
            .current_dir(root)
            .output()
            .map_err(|e| format!("could not run cargo metadata: {e}"))?;
        if !out.status.success() {
            return Err(format!(
                "cargo metadata failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        let json = String::from_utf8_lossy(&out.stdout);
        if json.trim().is_empty() {
            return Err("cargo metadata printed nothing".into());
        }
        Meta::parse(&json)
    }

    pub fn parse(json: &str) -> Result<Meta, String> {
        let v: Value =
            serde_json::from_str(json).map_err(|e| format!("cargo metadata is not JSON: {e}"))?;
        let member_ids: BTreeSet<String> = strings(&v["workspace_members"]).into_iter().collect();
        if member_ids.is_empty() {
            return Err("cargo metadata lists no workspace member".into());
        }

        let mut names = BTreeMap::new();
        let mut members = Vec::new();
        for p in v["packages"].as_array().into_iter().flatten() {
            let id = text(&p["id"]);
            let name = text(&p["name"]);
            names.insert(id.clone(), name.clone());
            if !member_ids.contains(&id) {
                continue;
            }
            let targets = p["targets"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|t| Target {
                    name: text(&t["name"]),
                    kinds: strings(&t["kind"]),
                })
                .collect();
            let deps = p["dependencies"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|d| Dep {
                    name: text(&d["name"]),
                    kind: match d["kind"].as_str() {
                        Some("dev") => DepKind::Dev,
                        Some("build") => DepKind::Build,
                        _ => DepKind::Normal,
                    },
                })
                .collect();
            let features = p["features"]
                .as_object()
                .map(|f| f.keys().cloned().collect())
                .unwrap_or_default();
            members.push(Package {
                id,
                name,
                features,
                targets,
                deps,
            });
        }
        members.sort_by(|a, b| a.name.cmp(&b.name));
        if members.len() != member_ids.len() {
            return Err("cargo metadata names a workspace member it does not describe".into());
        }

        let mut edges = BTreeMap::new();
        for node in v["resolve"]["nodes"].as_array().into_iter().flatten() {
            let reached = node["deps"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|d| {
                    d["dep_kinds"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .any(|k| k["kind"].as_str() != Some("dev"))
                })
                .map(|d| text(&d["pkg"]))
                .collect();
            edges.insert(text(&node["id"]), reached);
        }
        if edges.is_empty() {
            return Err("cargo metadata has no resolve graph".into());
        }

        Ok(Meta {
            members,
            target_dir: PathBuf::from(text(&v["target_directory"])),
            names,
            edges,
        })
    }

    pub fn member(&self, name: &str) -> Option<&Package> {
        self.members.iter().find(|m| m.name == name)
    }

    pub fn is_member(&self, name: &str) -> bool {
        self.member(name).is_some()
    }

    /// Names of every crate outside the workspace that `member` reaches through
    /// normal and build edges, without passing through another workspace member.
    pub fn external_closure(&self, member: &Package) -> BTreeSet<String> {
        let member_ids: BTreeSet<&str> = self.members.iter().map(|m| m.id.as_str()).collect();
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        let mut stack: Vec<&str> = vec![member.id.as_str()];
        while let Some(id) = stack.pop() {
            for next in self.edges.get(id).into_iter().flatten() {
                if member_ids.contains(next.as_str()) || !seen.insert(next.as_str()) {
                    continue;
                }
                stack.push(next.as_str());
            }
        }
        seen.into_iter()
            .filter_map(|id| self.names.get(id).cloned())
            .collect()
    }
}

fn text(v: &Value) -> String {
    v.as_str().unwrap_or_default().to_string()
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array().into_iter().flatten().map(text).collect()
}
```

- [ ] **Step 4: Run the tests and watch them pass**

```
cargo test -p xtask
```

Expected: `test result: ok. 3 passed; 0 failed`.

- [ ] **Step 5: Commit**

```
git add xtask/src/meta.rs xtask/src/main.rs
git commit -m "feat(xtask): read workspace members and the dependency graph from cargo metadata"
```

### Task 4: `xtask` — the dependency-direction check

**Files:**
- Create: `xtask/src/deps.rs`
- Modify: `xtask/src/main.rs`

**Interfaces:**
- Consumes: `meta::Meta`, `meta::DepKind`, `meta::fixture::metadata`.
- Produces: `deps::RULES: &[Rule]` (one row per workspace crate), `deps::Rule { name, internal: Internal, pure: Option<&[&str]>, testkit: bool }`,
  `deps::Internal { Only(&[&str]), Any }`, `deps::IO_CRATES`, `deps::DOCKER_CRATES`, and
  `deps::check(meta: &Meta) -> Vec<String>` (one line per violation; empty means pass).

What the check asserts, in the order the code checks it:

1. Every row of `RULES` names a workspace crate, and every workspace crate has a row.
2. Nothing depends on `fleetd` or `xtask`, in any dependency kind.
3. A crate's normal and build dependencies on other workspace crates are all in its row.
   `fleetd` alone has `Internal::Any`.
4. A crate marked pure takes direct external dependencies only from its allowlist, and
   reaches no crate in `IO_CRATES`, however indirectly.
5. No crate but `fleet-runner` reaches a crate in `DOCKER_CRATES`, including as a
   dev-dependency.
6. Every library crate except `harness-conformance` declares a `testkit` feature.

Dev-dependencies are otherwise free: a pure crate may use `tempfile` in its tests.

Rule 4 is how the three parser crates the sibling plans ask for are handled. `serde-saphyr`
is on `factory-spec`'s allowlist, and `quick-xml` and `toml` are on `factory-presets`'s,
by name, with a comment saying why: a parser is given text and returns data. The second half
of rule 4 still applies to them, so if a later release of one of them began to depend on,
say, `tokio`, the check would fail and say that `factory-presets` does no I/O but reaches
`tokio`.

- [ ] **Step 1: Write the failing tests**

Create `xtask/src/deps.rs` containing only the tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::meta::fixture::{metadata, Member};

    /// A workspace that satisfies every rule: each crate in `RULES`, with no
    /// dependencies, and a `testkit` feature where one is required.
    fn clean() -> Vec<Member<'static>> {
        RULES
            .iter()
            .map(|r| {
                let features: &[&str] = if r.testkit { &["testkit"] } else { &[] };
                (r.name, features, &[] as &[&str], &[] as &[(&str, &str)])
            })
            .collect()
    }

    fn with(
        name: &'static str,
        deps: &'static [(&'static str, &'static str)],
    ) -> Vec<Member<'static>> {
        let mut members = clean();
        let slot = members
            .iter_mut()
            .find(|m| m.0 == name)
            .expect("crate has a rule");
        slot.3 = deps;
        members
    }

    fn errors(members: &[Member], external: &[(&str, &[&str])]) -> Vec<String> {
        check(&Meta::parse(&metadata(members, external)).unwrap())
    }

    #[test]
    fn a_workspace_that_follows_the_rules_passes() {
        assert_eq!(errors(&clean(), &[]), Vec::<String>::new());
    }

    #[test]
    fn a_crate_with_no_rule_fails() {
        let mut members = clean();
        members.push(("fleet-newcomer", &["testkit"], &[], &[]));
        let got = errors(&members, &[]);
        assert_eq!(got.len(), 1, "{got:?}");
        assert!(got[0].contains("`fleet-newcomer` has no rule"), "{got:?}");
    }

    #[test]
    fn a_rule_for_a_crate_that_left_the_workspace_fails() {
        let members: Vec<Member> = clean()
            .into_iter()
            .filter(|m| m.0 != "fleet-forge")
            .collect();
        let got = errors(&members, &[]);
        assert_eq!(got.len(), 1, "{got:?}");
        assert!(got[0].contains("rule for `fleet-forge`"), "{got:?}");
    }

    #[test]
    fn a_pure_crate_may_not_take_an_unlisted_direct_dependency() {
        let got = errors(
            &with("fleet-core", &[("regex", "normal")]),
            &[("regex", &[])],
        );
        assert_eq!(got.len(), 1, "{got:?}");
        assert!(got[0].contains("`fleet-core` does no I/O and may not depend on `regex`"));
    }

    #[test]
    fn a_pure_crate_may_not_reach_an_io_crate_through_an_allowed_one() {
        let got = errors(
            &with("factory-spec", &[("serde", "normal")]),
            &[("serde", &["tokio"]), ("tokio", &[])],
        );
        assert_eq!(got, ["`factory-spec` does no I/O but reaches `tokio`"]);
    }

    #[test]
    fn a_pure_crate_may_dev_depend_on_anything_but_docker() {
        let ok = errors(
            &with("fleet-core", &[("tempfile", "dev")]),
            &[("tempfile", &[])],
        );
        assert_eq!(ok, Vec::<String>::new());
        let bad = errors(
            &with("fleet-core", &[("testcontainers", "dev")]),
            &[("testcontainers", &[])],
        );
        assert_eq!(bad.len(), 1, "{bad:?}");
        assert!(bad[0].contains("only `fleet-runner` may reach Docker"));
    }

    #[test]
    fn only_the_runner_may_reach_a_docker_crate() {
        let runner = errors(
            &with("fleet-runner", &[("bollard", "normal")]),
            &[("bollard", &[])],
        );
        assert_eq!(runner, Vec::<String>::new());
        let store = errors(
            &with("fleet-store", &[("wrapper", "normal")]),
            &[("wrapper", &["bollard"]), ("bollard", &[])],
        );
        assert_eq!(
            store,
            ["`fleet-store` reaches `bollard`: only `fleet-runner` may reach Docker"]
        );
    }

    #[test]
    fn an_internal_edge_outside_the_rule_fails_and_fleetd_may_use_all() {
        let got = errors(&with("fleet-store", &[("fleet-api", "normal")]), &[]);
        assert_eq!(
            got,
            ["`fleet-store` depends on `fleet-api`, which its rule does not allow"]
        );
        let wired = errors(
            &with(
                "fleetd",
                &[("fleet-store", "normal"), ("fleet-api", "normal")],
            ),
            &[],
        );
        assert_eq!(wired, Vec::<String>::new());
    }

    #[test]
    fn nothing_may_depend_on_fleetd_or_xtask_even_for_tests() {
        let got = errors(&with("fleet-api", &[("fleetd", "dev")]), &[]);
        assert_eq!(
            got,
            ["`fleet-api` depends on `fleetd`, which nothing may depend on"]
        );
    }

    #[test]
    fn a_library_crate_without_a_testkit_feature_fails() {
        let mut members = clean();
        members.iter_mut().find(|m| m.0 == "fleet-store").unwrap().1 = &[];
        assert_eq!(
            errors(&members, &[]),
            ["`fleet-store` must declare a `testkit` feature"]
        );
    }

    #[test]
    fn exactly_one_crate_may_depend_on_every_other() {
        let any: Vec<&str> = RULES
            .iter()
            .filter(|r| matches!(r.internal, Internal::Any))
            .map(|r| r.name)
            .collect();
        assert_eq!(any, ["fleetd"]);
    }
}
```

In `xtask/src/main.rs`, the module lines become (alphabetical, which `cargo fmt` requires):

```rust
mod deps;
mod meta;
```

- [ ] **Step 2: Run the tests and watch them fail**

```
cargo test -p xtask
```

Expected: the crate does not compile. The errors include
``cannot find value `RULES` in this scope`` and ``cannot find function `check` in this scope``.

- [ ] **Step 3: Write the implementation**

Put this above the test module in `xtask/src/deps.rs`:

```rust
//! The dependency-direction check: an allowlist asserted over `cargo metadata`.
//!
//! Every workspace crate has one row in [`RULES`]. A crate with no row fails the
//! check, so a crate cannot join the workspace without its direction being decided.

use crate::meta::{DepKind, Meta};

/// Which workspace crates a crate may depend on (normal and build dependencies).
pub enum Internal {
    /// Only these.
    Only(&'static [&'static str]),
    /// Any of them. For the binary that wires the others together, and no one else.
    Any,
}

pub struct Rule {
    pub name: &'static str,
    pub internal: Internal,
    /// `Some(list)` marks a crate that does no I/O: its direct dependencies outside
    /// the workspace must all be on the list, and nothing it reaches may be in
    /// [`IO_CRATES`].
    pub pure: Option<&'static [&'static str]>,
    /// The crate must declare a `testkit` feature.
    pub testkit: bool,
}

const fn rule(name: &'static str, internal: &'static [&'static str]) -> Rule {
    Rule {
        name,
        internal: Internal::Only(internal),
        pure: None,
        testkit: true,
    }
}

const fn pure(name: &'static str, external: &'static [&'static str]) -> Rule {
    Rule {
        name,
        internal: Internal::Only(&[]),
        pure: Some(external),
        testkit: true,
    }
}

/// One row per workspace crate. Adding a dependency edge between workspace crates,
/// or a direct dependency to a pure crate, is a change to this table.
pub const RULES: &[Rule] = &[
    // `quick-xml`, `toml` and `serde-saphyr` are parsers: they turn text they are handed
    // into data and open nothing themselves. `IO_CRATES` still applies to all they pull in.
    pure(
        "factory-presets",
        &["quick-xml", "serde", "serde_json", "thiserror", "toml"],
    ),
    pure(
        "factory-spec",
        &["serde", "serde-saphyr", "serde_json", "sha2", "thiserror"],
    ),
    pure("fleet-core", &["serde", "serde_json", "thiserror"]),
    rule("fleet-admission", &["factory-spec", "fleet-core"]),
    rule("fleet-api", &["fleet-core", "harness-protocol"]),
    rule("fleet-forge", &[]),
    rule("fleet-runner", &["fleet-core"]),
    rule("fleet-schedule", &["fleet-core"]),
    rule("fleet-store", &["fleet-core"]),
    rule("fleet-supervisor", &["fleet-core", "harness-protocol"]),
    rule(
        "fleet-verify",
        &[
            "factory-presets",
            "factory-spec",
            "fleet-core",
            "harness-protocol",
        ],
    ),
    rule("harness-protocol", &[]),
    Rule {
        name: "fleetd",
        internal: Internal::Any,
        pure: None,
        testkit: false,
    },
    Rule {
        name: "harness-conformance",
        internal: Internal::Only(&["harness-protocol"]),
        pure: None,
        testkit: false,
    },
    Rule {
        name: "xtask",
        internal: Internal::Only(&[]),
        pure: None,
        testkit: false,
    },
];

/// Workspace crates nothing else may depend on, in any dependency kind.
const LEAVES: &[&str] = &["fleetd", "xtask"];

/// The one crate allowed to reach a Docker client crate.
const DOCKER_OWNER: &str = "fleet-runner";

/// Crates that do I/O. A pure crate may not reach any of them, however indirectly.
pub const IO_CRATES: &[&str] = &[
    "async-std",
    "axum",
    "bollard",
    "dotenvy",
    "duct",
    "fs_extra",
    "git2",
    "hyper",
    "libsqlite3-sys",
    "mio",
    "notify",
    "reqwest",
    "rusqlite",
    "smol",
    "socket2",
    "sqlx",
    "tempfile",
    "tokio",
    "tokio-tungstenite",
    "tower-http",
    "tungstenite",
    "ureq",
    "walkdir",
    "which",
];

/// Crates that talk to Docker.
pub const DOCKER_CRATES: &[&str] = &[
    "bollard",
    "bollard-stubs",
    "docker-api",
    "dockworker",
    "shiplift",
    "testcontainers",
    "testcontainers-modules",
];

/// Every violation of the rules, one line each. Empty means the check passes.
pub fn check(meta: &Meta) -> Vec<String> {
    let mut errors = Vec::new();

    for r in RULES {
        if !meta.is_member(r.name) {
            errors.push(format!(
                "xtask/src/deps.rs has a rule for `{}`, which is not a workspace crate",
                r.name
            ));
        }
    }

    for m in &meta.members {
        let Some(r) = RULES.iter().find(|r| r.name == m.name) else {
            errors.push(format!(
                "`{}` has no rule in xtask/src/deps.rs: decide what it may depend on and add one",
                m.name
            ));
            continue;
        };

        for d in &m.deps {
            let internal = meta.is_member(&d.name);
            if internal && LEAVES.contains(&d.name.as_str()) {
                errors.push(format!(
                    "`{}` depends on `{}`, which nothing may depend on",
                    m.name, d.name
                ));
                continue;
            }
            if d.kind == DepKind::Dev {
                if DOCKER_CRATES.contains(&d.name.as_str()) && m.name != DOCKER_OWNER {
                    errors.push(format!(
                        "`{}` dev-depends on `{}`: only `{DOCKER_OWNER}` may reach Docker",
                        m.name, d.name
                    ));
                }
                continue;
            }
            if internal {
                let allowed = match r.internal {
                    Internal::Any => true,
                    Internal::Only(list) => list.contains(&d.name.as_str()),
                };
                if !allowed {
                    errors.push(format!(
                        "`{}` depends on `{}`, which its rule does not allow",
                        m.name, d.name
                    ));
                }
            } else if let Some(allowed) = r.pure {
                if !allowed.contains(&d.name.as_str()) {
                    errors.push(format!(
                        "`{}` does no I/O and may not depend on `{}` (not on its allowlist)",
                        m.name, d.name
                    ));
                }
            }
        }

        let reached = meta.external_closure(m);
        if r.pure.is_some() {
            for io in reached.iter().filter(|c| IO_CRATES.contains(&c.as_str())) {
                errors.push(format!("`{}` does no I/O but reaches `{io}`", m.name));
            }
        }
        if m.name != DOCKER_OWNER {
            for d in reached
                .iter()
                .filter(|c| DOCKER_CRATES.contains(&c.as_str()))
            {
                errors.push(format!(
                    "`{}` reaches `{d}`: only `{DOCKER_OWNER}` may reach Docker",
                    m.name
                ));
            }
        }

        if r.testkit && !m.features.iter().any(|f| f == "testkit") {
            errors.push(format!("`{}` must declare a `testkit` feature", m.name));
        }
    }

    errors
}
```

- [ ] **Step 4: Run the tests and watch them pass**

```
cargo test -p xtask
```

Expected: `test result: ok. 14 passed; 0 failed`.

- [ ] **Step 5: Commit**

```
git add xtask/src/deps.rs xtask/src/main.rs
git commit -m "feat(xtask): assert the dependency-direction allowlist over cargo metadata"
```

### Task 5: `xtask` — what each tier runs

**Files:**
- Create: `xtask/src/tiers.rs`
- Modify: `xtask/src/main.rs`

**Interfaces:**
- Consumes: `meta::Meta`, `meta::fixture::metadata`.
- Produces: `tiers::Tier { Static, Unit, Contract, Integration, E2e, Live }` with `Tier::ALL`,
  `name()` and `parse(&str) -> Option<Tier>`; `tiers::tier_of_test_target(name: &str) -> Tier`;
  `tiers::Cmd { label: String, program: PathBuf, args: Vec<String> }`;
  `tiers::unit(meta, only: Option<&str>) -> Result<Vec<Cmd>, String>` (one command per crate,
  and a second with `--features testkit` for a crate that declares that feature);
  `tiers::test_targets(meta, tier) -> Vec<Cmd>`; `tiers::contract(meta) -> Vec<Cmd>`;
  `tiers::static_cmds(root: &Path, root_only: bool) -> Vec<Cmd>`;
  `tiers::debug_bin(target_dir: &Path, name: &str) -> PathBuf`;
  `tiers::cockpit_manifest(root: &Path) -> PathBuf`; `tiers::sidecar_built(root: &Path) -> bool`.

`tier_of_test_target` is the program's tier naming rule (Global Constraints) as code. It is
total: every name gets a tier, so no test target can be left out of every tier.

The second `unit` run exists because the spec-and-presets plan asks for each of its crates
to be tested with and without the `testkit` feature on both operating systems. It is done
for every crate that has the feature, so the rule has no list of names to keep up to date.
That plan writes the request as `cargo test -p <crate>`; the tier runs `--lib`, which is the
same tests for those two crates, because they keep every test inside `src/`.

- [ ] **Step 1: Write the failing tests**

Create `xtask/src/tiers.rs` containing only the tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::meta::fixture::metadata;

    fn meta() -> Meta {
        Meta::parse(&metadata(
            &[
                (
                    "fleetd",
                    &[],
                    &["demo_mode_it", "characterisation_store", "e2e_one_unit"],
                    &[],
                ),
                ("harness-protocol", &["testkit"], &["contract"], &[]),
                (
                    "factory-spec",
                    &["testkit"],
                    &["vectors", "live_models"],
                    &[],
                ),
                ("fleet-store", &["testkit"], &[], &[]),
            ],
            &[],
        ))
        .unwrap()
    }

    fn line(c: &Cmd) -> String {
        c.args.join(" ")
    }

    #[test]
    fn every_tier_name_round_trips() {
        for t in Tier::ALL {
            assert_eq!(Tier::parse(t.name()), Some(t));
        }
        assert_eq!(Tier::parse("smoke"), None);
    }

    #[test]
    fn a_test_target_is_placed_by_its_name() {
        assert_eq!(tier_of_test_target("e2e_one_unit"), Tier::E2e);
        assert_eq!(tier_of_test_target("live_models"), Tier::Live);
        assert_eq!(tier_of_test_target("local_docker_it"), Tier::Integration);
        assert_eq!(
            tier_of_test_target("characterisation_store"),
            Tier::Integration
        );
        assert_eq!(tier_of_test_target("contract"), Tier::Contract);
        assert_eq!(tier_of_test_target("vectors"), Tier::Contract);
        assert_eq!(tier_of_test_target("fake_conforms"), Tier::Contract);
    }

    #[test]
    fn unit_runs_one_command_per_crate_and_never_a_tests_target() {
        let cmds = unit(&meta(), None).unwrap();
        let lines: Vec<String> = cmds.iter().map(line).collect();
        assert_eq!(
            lines,
            [
                "test --locked -p factory-spec --lib",
                "test --locked -p factory-spec --lib --features testkit",
                "test --locked -p fleet-store --lib",
                "test --locked -p fleet-store --lib --features testkit",
                "test --locked -p fleetd --lib",
                "test --locked -p harness-protocol --lib",
                "test --locked -p harness-protocol --lib --features testkit",
            ]
        );
        assert!(lines.iter().all(|l| !l.contains("--test ")));
    }

    #[test]
    fn unit_can_be_narrowed_to_one_crate_and_refuses_an_unknown_one() {
        let one = unit(&meta(), Some("fleet-store")).unwrap();
        assert_eq!(one.len(), 2, "once plain, once with its test kit");
        assert_eq!(line(&one[0]), "test --locked -p fleet-store --lib");
        assert_eq!(unit(&meta(), Some("fleetd")).unwrap().len(), 1);
        assert!(unit(&meta(), Some("fleet-stroe")).is_err());
    }

    #[test]
    fn each_tier_selects_only_its_own_test_targets() {
        let m = meta();
        let lines = |tier| test_targets(&m, tier).iter().map(line).collect::<Vec<_>>();
        assert_eq!(
            lines(Tier::Integration),
            ["test --locked -p fleetd --test characterisation_store --test demo_mode_it"]
        );
        assert_eq!(
            lines(Tier::Contract),
            [
                "test --locked -p factory-spec --test vectors",
                "test --locked -p harness-protocol --test contract",
            ]
        );
        assert_eq!(
            lines(Tier::E2e),
            ["test --locked -p fleetd --test e2e_one_unit"]
        );
        assert_eq!(
            lines(Tier::Live),
            ["test --locked -p factory-spec --test live_models"]
        );
    }

    #[test]
    fn a_tier_with_no_target_selects_nothing() {
        let m = Meta::parse(&metadata(&[("fleet-store", &["testkit"], &[], &[])], &[])).unwrap();
        assert!(test_targets(&m, Tier::E2e).is_empty());
        assert!(test_targets(&m, Tier::Live).is_empty());
    }

    #[test]
    fn contract_ends_by_running_the_kit_against_the_fake() {
        let cmds = contract(&meta());
        let kit = cmds.last().unwrap();
        let suffix = std::env::consts::EXE_SUFFIX;
        assert_eq!(
            kit.program,
            Path::new("/ws/target")
                .join("debug")
                .join(format!("harness-conformance{suffix}"))
        );
        assert_eq!(
            kit.args[..5],
            ["--wall-clock-secs", "30", "--grace-secs", "5", "--"]
        );
        assert!(kit.args[5].ends_with(&format!("harness-fake{suffix}")));
    }

    #[test]
    fn paths_are_built_from_components_not_from_separators() {
        let root = Path::new("repo");
        let manifest = cockpit_manifest(root);
        let parts: Vec<_> = manifest
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        assert_eq!(parts, ["repo", "cockpit", "ui", "src-tauri", "Cargo.toml"]);
        let bin = debug_bin(Path::new("t"), "harness-fake");
        assert_eq!(bin.parent().unwrap(), Path::new("t").join("debug"));
    }

    #[test]
    fn static_root_only_leaves_the_cockpit_workspace_out() {
        let full: Vec<String> = static_cmds(Path::new("repo"), false)
            .iter()
            .map(|c| c.label.clone())
            .collect();
        let root: Vec<String> = static_cmds(Path::new("repo"), true)
            .iter()
            .map(|c| c.label.clone())
            .collect();
        assert_eq!(
            full,
            [
                "static fmt (root)",
                "static clippy (root)",
                "static fmt (cockpit)",
                "static clippy (cockpit)",
                "static parity self-test",
                "static parity",
            ]
        );
        assert_eq!(
            root,
            [
                "static fmt (root)",
                "static clippy (root)",
                "static parity self-test",
                "static parity",
            ]
        );
    }
}
```

In `xtask/src/main.rs`, the module lines become:

```rust
mod deps;
mod meta;
mod tiers;
```

- [ ] **Step 2: Run the tests and watch them fail**

```
cargo test -p xtask
```

Expected: the crate does not compile. The errors include
``use of undeclared type `Tier` `` and ``cannot find function `tier_of_test_target` in this scope``.

- [ ] **Step 3: Write the implementation**

Put this above the test module in `xtask/src/tiers.rs`:

```rust
//! The six test tiers: which commands each one runs.
//!
//! A tier is chosen for an integration-test target (`tests/<name>.rs`) by its name:
//!
//! | Name                                  | Tier          |
//! |---------------------------------------|---------------|
//! | `e2e_*`                               | `e2e`         |
//! | `live_*`                              | `live`        |
//! | `*_it`, `characterisation_*`          | `integration` |
//! | anything else                         | `contract`    |
//!
//! Tests inside `src/` are the `unit` tier. A test that needs Docker, a network or a
//! token therefore has to live in a `tests/` target, which `unit` never builds.

use crate::meta::Meta;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    Static,
    Unit,
    Contract,
    Integration,
    E2e,
    Live,
}

impl Tier {
    pub const ALL: [Tier; 6] = [
        Tier::Static,
        Tier::Unit,
        Tier::Contract,
        Tier::Integration,
        Tier::E2e,
        Tier::Live,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Tier::Static => "static",
            Tier::Unit => "unit",
            Tier::Contract => "contract",
            Tier::Integration => "integration",
            Tier::E2e => "e2e",
            Tier::Live => "live",
        }
    }

    pub fn parse(s: &str) -> Option<Tier> {
        Tier::ALL.into_iter().find(|t| t.name() == s)
    }
}

/// The tier an integration-test target belongs to. Total: every name has one.
pub fn tier_of_test_target(name: &str) -> Tier {
    if name.starts_with("e2e_") {
        Tier::E2e
    } else if name.starts_with("live_") {
        Tier::Live
    } else if name.ends_with("_it") || name.starts_with("characterisation_") {
        Tier::Integration
    } else {
        Tier::Contract
    }
}

/// One command to run from the repository root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cmd {
    pub label: String,
    pub program: PathBuf,
    pub args: Vec<String>,
}

fn cargo(label: impl Into<String>, args: &[&str]) -> Cmd {
    Cmd {
        label: label.into(),
        program: PathBuf::from(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into())),
        args: args.iter().map(|a| a.to_string()).collect(),
    }
}

/// `unit`: each crate's in-source tests, one crate per command, and a second command
/// with `--features testkit` for a crate that declares that feature.
pub fn unit(meta: &Meta, only: Option<&str>) -> Result<Vec<Cmd>, String> {
    if let Some(name) = only {
        if !meta.is_member(name) {
            return Err(format!("`{name}` is not a workspace crate"));
        }
    }
    let mut cmds = Vec::new();
    for m in meta
        .members
        .iter()
        .filter(|m| only.is_none_or(|name| name == m.name))
    {
        let targets = if m.has_lib() { "--lib" } else { "--bins" };
        let plain = ["test", "--locked", "-p", m.name.as_str(), targets];
        cmds.push(cargo(format!("unit {}", m.name), &plain));
        // A crate with a test kit is run again with it on, so the kit is compiled and
        // tested wherever this tier runs.
        if m.features.iter().any(|f| f == "testkit") {
            let mut with_kit = plain.to_vec();
            with_kit.extend(["--features", "testkit"]);
            cmds.push(cargo(format!("unit {} (testkit)", m.name), &with_kit));
        }
    }
    if cmds.is_empty() {
        return Err("the workspace has no crate to test".into());
    }
    Ok(cmds)
}

/// One `cargo test` per crate that has integration-test targets in `tier`.
pub fn test_targets(meta: &Meta, tier: Tier) -> Vec<Cmd> {
    meta.members
        .iter()
        .filter_map(|m| {
            let names: Vec<&str> = m
                .test_targets()
                .into_iter()
                .filter(|t| tier_of_test_target(t) == tier)
                .collect();
            if names.is_empty() {
                return None;
            }
            let mut args = vec!["test", "--locked", "-p", m.name.as_str()];
            for n in &names {
                args.extend(["--test", n]);
            }
            Some(cargo(format!("{} {}", tier.name(), m.name), &args))
        })
        .collect()
}

/// A binary built into the debug profile, with the platform's executable suffix.
pub fn debug_bin(target_dir: &Path, name: &str) -> PathBuf {
    target_dir
        .join("debug")
        .join(format!("{name}{}", std::env::consts::EXE_SUFFIX))
}

/// `contract`: the contract-tier test targets, then the conformance kit run against
/// `harness-fake` exactly as a third-party harness would be checked.
pub fn contract(meta: &Meta) -> Vec<Cmd> {
    let mut cmds = test_targets(meta, Tier::Contract);
    cmds.push(cargo(
        "contract build the conformance kit",
        &["build", "--locked", "-p", "harness-conformance", "--bins"],
    ));
    cmds.push(Cmd {
        label: "contract conformance kit against harness-fake".into(),
        program: debug_bin(&meta.target_dir, "harness-conformance"),
        args: vec![
            "--wall-clock-secs".into(),
            "30".into(),
            "--grace-secs".into(),
            "5".into(),
            "--".into(),
            debug_bin(&meta.target_dir, "harness-fake")
                .to_string_lossy()
                .into_owned(),
        ],
    });
    cmds
}

/// The cockpit's own cargo workspace, which nothing run from the root reaches.
pub fn cockpit_manifest(root: &Path) -> PathBuf {
    root.join("cockpit")
        .join("ui")
        .join("src-tauri")
        .join("Cargo.toml")
}

/// True once `npm run sidecar` has put the `fleetd-serve-*` binary where the cockpit
/// crate's build script looks for it. Without it that crate does not compile.
pub fn sidecar_built(root: &Path) -> bool {
    let dir = root
        .join("cockpit")
        .join("ui")
        .join("src-tauri")
        .join("binaries");
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .any(|e| e.file_name().to_string_lossy().starts_with("fleetd-serve-"))
        })
        .unwrap_or(false)
}

/// `static`, the commands half: formatting, lints, and the registry parity check.
/// The in-process checks (dependency direction, CI job names) are run by `main`.
pub fn static_cmds(root: &Path, root_only: bool) -> Vec<Cmd> {
    let mut cmds = vec![
        cargo("static fmt (root)", &["fmt", "--all", "--", "--check"]),
        cargo(
            "static clippy (root)",
            &[
                "clippy",
                "--locked",
                "--workspace",
                "--all-targets",
                "--all-features",
                "--",
                "-D",
                "warnings",
            ],
        ),
    ];
    if !root_only {
        let manifest = cockpit_manifest(root).to_string_lossy().into_owned();
        cmds.push(cargo(
            "static fmt (cockpit)",
            &[
                "fmt",
                "--all",
                "--manifest-path",
                &manifest,
                "--",
                "--check",
            ],
        ));
        cmds.push(cargo(
            "static clippy (cockpit)",
            &[
                "clippy",
                "--manifest-path",
                &manifest,
                "--all-targets",
                "--",
                "-D",
                "warnings",
            ],
        ));
    }
    let script = |name: &str| {
        root.join("scripts")
            .join(name)
            .to_string_lossy()
            .into_owned()
    };
    cmds.push(Cmd {
        label: "static parity self-test".into(),
        program: PathBuf::from("node"),
        args: vec!["--test".into(), script("parity.test.mjs")],
    });
    cmds.push(Cmd {
        label: "static parity".into(),
        program: PathBuf::from("node"),
        args: vec![script("parity.mjs")],
    });
    cmds
}
```

- [ ] **Step 4: Run the tests and watch them pass**

```
cargo test -p xtask
```

Expected: `test result: ok. 23 passed; 0 failed`.

- [ ] **Step 5: Commit**

```
git add xtask/src/tiers.rs xtask/src/main.rs
git commit -m "feat(xtask): choose each tier's commands from test-target names"
```

### Task 6: `xtask` — pin the CI job names

**Files:**
- Create: `xtask/src/ci.rs`
- Modify: `xtask/src/main.rs`

**Interfaces:**
- Consumes: the text of `.github/workflows/ci.yml`.
- Produces: `ci::WORKFLOW: [&str; 3]` (the workflow's path components), `ci::PINNED_JOBS: &[&str]`,
  `ci::job_names(workflow: &str) -> Vec<String>`, `ci::check(workflow: &str) -> Vec<String>`.

`PINNED_JOBS` is the final set of job names for this milestone: the seven that exist today,
exactly as written, and the six that Task 8 adds. A matrix job is pinned by its template; the
check contexts it produces are in the comment beside it.

- [ ] **Step 1: Write the failing tests**

Create `xtask/src/ci.rs` containing only the tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn workflow() -> String {
        let mut text = String::from("name: CI\njobs:\n");
        for (i, name) in PINNED_JOBS.iter().enumerate() {
            text.push_str(&format!(
                "  job{i}:\n    name: {name}\n    steps:\n      - name: a step\n        run: cargo xtask test static\n"
            ));
        }
        text
    }

    #[test]
    fn a_workflow_with_every_pinned_name_passes() {
        assert_eq!(check(&workflow()), Vec::<String>::new());
    }

    #[test]
    fn step_names_and_the_workflow_name_are_not_job_names() {
        let names = job_names(&workflow());
        assert_eq!(names.len(), PINNED_JOBS.len());
        assert!(!names.iter().any(|n| n == "a step" || n == "CI"));
    }

    #[test]
    fn a_renamed_job_fails() {
        let renamed = workflow().replace("name: cargo test (workspace)", "name: cargo test");
        let got = check(&renamed);
        assert_eq!(got.len(), 1, "{got:?}");
        assert!(got[0].contains("no job named `cargo test (workspace)`"));
    }

    #[test]
    fn crlf_line_endings_and_quoted_names_still_match() {
        let crlf = workflow()
            .replace("name: static\n", "name: \"static\"\n")
            .replace('\n', "\r\n");
        assert_eq!(check(&crlf), Vec::<String>::new());
    }

    #[test]
    fn an_empty_workflow_fails_rather_than_passing_vacuously() {
        let got = check("");
        assert!(got[0].contains("declares no named job"), "{got:?}");
        assert_eq!(got.len(), 1 + PINNED_JOBS.len());
    }

    #[test]
    fn ci_may_not_skip_the_cockpit_half_of_static() {
        let skipping = workflow().replace(
            "cargo xtask test static",
            "cargo xtask test static --root-only",
        );
        assert!(check(&skipping).iter().any(|e| e.contains("--root-only")));
    }

    #[test]
    fn the_two_names_protection_requires_today_are_pinned() {
        assert!(PINNED_JOBS.contains(&"cargo test (workspace)"));
        assert!(PINNED_JOBS.contains(&"harness conformance"));
    }
}
```

In `xtask/src/main.rs`, the module lines become:

```rust
mod ci;
mod deps;
mod meta;
mod tiers;
```

- [ ] **Step 2: Run the tests and watch them fail**

```
cargo test -p xtask
```

Expected: the crate does not compile. The errors include
``cannot find value `PINNED_JOBS` in this scope`` and ``cannot find function `job_names` in this scope``.

- [ ] **Step 3: Write the implementation**

Put this above the test module in `xtask/src/ci.rs`:

```rust
//! Pins the CI job names that branch protection refers to.
//!
//! Branch protection matches a required check by its job name. A job renamed in the
//! workflow keeps running, but under a name protection no longer waits for, so the
//! requirement silently stops binding. This check fails when a pinned name is gone.

/// The workflow file, as path components from the repository root.
pub const WORKFLOW: [&str; 3] = [".github", "workflows", "ci.yml"];

/// Job-level `name:` values, exactly as the workflow writes them. A matrix job is
/// pinned by its template; the check contexts it produces are listed beside it.
pub const PINNED_JOBS: &[&str] = &[
    "fmt + clippy",
    "svelte-check + tsc",
    "vitest (cockpit/ui)",
    "cargo test (workspace)",
    "harness conformance",
    "cargo test (cockpit)",
    "tauri build (${{ matrix.os }})", // tauri build (windows-latest | macos-latest | ubuntu-latest)
    "static",
    "unit (list crates)",
    "unit (${{ matrix.crate }}, ${{ matrix.os }})", // one per crate per OS; not for protection
    "unit (all crates)",
    "contract (${{ matrix.os }})", // contract (ubuntu-latest | windows-latest)
    "integration",
];

/// Job names in a workflow: a job's `name:` key sits at exactly four spaces of indent.
pub fn job_names(workflow: &str) -> Vec<String> {
    workflow
        .lines()
        .filter_map(|l| l.strip_prefix("    name:"))
        .map(|rest| rest.trim().trim_matches(['"', '\'']).to_string())
        .collect()
}

/// Every violation, one line each. Empty means the check passes.
pub fn check(workflow: &str) -> Vec<String> {
    let names = job_names(workflow);
    let mut errors = Vec::new();
    if names.is_empty() {
        errors.push("the workflow declares no named job".to_string());
    }
    for pinned in PINNED_JOBS {
        if !names.iter().any(|n| n == pinned) {
            errors.push(format!(
                "the workflow has no job named `{pinned}`: branch protection refers to job names, \
                 so renaming one needs the owner to change protection first"
            ));
        }
    }
    if workflow.contains("--root-only") {
        errors.push("the workflow passes --root-only: CI must run the whole static tier".into());
    }
    errors
}
```

- [ ] **Step 4: Run the tests and watch them pass**

```
cargo test -p xtask
```

Expected: `test result: ok. 30 passed; 0 failed`.

- [ ] **Step 5: Commit**

```
git add xtask/src/ci.rs xtask/src/main.rs
git commit -m "feat(xtask): pin the CI job names that branch protection refers to"
```

### Task 7: `xtask` — the command line

**Files:**
- Modify: `xtask/src/main.rs` (replaced)

**Interfaces:**
- Consumes: `meta`, `deps`, `tiers`, `ci`.
- Produces: the commands `cargo xtask test <static|unit|contract|integration|e2e|live> [-p <crate>] [--root-only]`,
  `cargo xtask deps` and `cargo xtask crates`. Exit code 0 when every step passed, 1 when one
  failed, 2 when the command line was wrong. `-p` narrows `unit` to one crate and is refused
  for other tiers. `--root-only` leaves the cockpit workspace out of `static` and is refused
  for other tiers; CI never passes it (Task 6 checks that).

- [ ] **Step 1: Write the failing tests**

Append this test module to the stub `xtask/src/main.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_parse_the_two_flags_and_refuse_anything_else() {
        assert_eq!(Options::parse(&[]), Some(Options::default()));
        assert_eq!(
            Options::parse(&["-p", "fleet-store", "--root-only"]),
            Some(Options {
                only: Some("fleet-store".into()),
                root_only: true
            })
        );
        assert_eq!(Options::parse(&["-p"]), None, "-p needs a crate name");
        assert_eq!(Options::parse(&["--fast"]), None);
    }

    #[test]
    fn the_root_holds_the_workspace_manifest_and_the_workflow() {
        let root = root();
        assert!(root.join("Cargo.toml").is_file());
        let workflow = workflow_path(&root);
        assert!(workflow.is_file(), "{} is missing", workflow.display());
    }

    /// The real workspace, not a fixture: catches a rule table, a tier selector or a
    /// job-name pin that has drifted from the repository it is meant to describe.
    #[test]
    fn the_real_workspace_satisfies_every_static_check() {
        let root = root();
        let meta = Meta::load(&root).expect("cargo metadata runs");
        assert_eq!(deps::check(&meta), Vec::<String>::new());

        let text = std::fs::read_to_string(workflow_path(&root)).unwrap();
        assert_eq!(ci::check(&text), Vec::<String>::new());

        let unit = tiers::unit(&meta, None).unwrap();
        for m in &meta.members {
            let label = format!("unit {}", m.name);
            assert!(
                unit.iter().any(|c| c.label == label),
                "no unit command for `{}`",
                m.name
            );
        }
        assert_eq!(
            meta.members.len(),
            deps::RULES.len(),
            "one dependency rule per workspace crate"
        );

        let lines = |tier| -> Vec<String> {
            tiers::test_targets(&meta, tier)
                .into_iter()
                .map(|c| c.args.join(" "))
                .collect()
        };
        assert!(
            lines(Tier::Contract)
                .iter()
                .any(|l| l.contains("-p harness-protocol --test contract")),
            "the contract tier must run the protocol contract test"
        );
        assert!(
            lines(Tier::Integration)
                .iter()
                .any(|l| l.contains("--test demo_mode_it")),
            "the integration tier must run the demo-mode test"
        );
    }
}
```

- [ ] **Step 2: Run the tests and watch them fail**

```
cargo test -p xtask
```

Expected: the crate does not compile. The errors name `Options`, `root` and `workflow_path`.

- [ ] **Step 3: Write the implementation**

Replace `xtask/src/main.rs` with the whole file below. It ends with the same test module, and
the `#![allow(dead_code)]` line is gone.

````rust
//! `cargo xtask` — the repository's task runner.
//!
//! ```text
//! cargo xtask test <static|unit|contract|integration|e2e|live> [-p <crate>] [--root-only]
//! cargo xtask deps      the dependency-direction check on its own
//! cargo xtask crates    workspace crate names as a JSON array (CI builds its matrix from it)
//! ```
//!
//! Exit codes: 0 every step passed, 1 a step failed, 2 the command line was wrong.

mod ci;
mod deps;
mod meta;
mod tiers;

use meta::Meta;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use tiers::{Cmd, Tier};

const USAGE: &str = "usage:
  cargo xtask test <static|unit|contract|integration|e2e|live> [-p <crate>] [--root-only]
  cargo xtask deps
  cargo xtask crates";

/// The repository root: this crate's manifest directory is `<root>/xtask`.
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask sits one level below the repository root")
        .to_path_buf()
}

fn workflow_path(root: &Path) -> PathBuf {
    ci::WORKFLOW
        .iter()
        .fold(root.to_path_buf(), |path, part| path.join(part))
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let root = root();
    let result = match args.as_slice() {
        ["deps"] => {
            Meta::load(&root).map(|meta| report("dependency direction", deps::check(&meta)))
        }
        ["crates"] => Meta::load(&root).map(|meta| {
            let names: Vec<&str> = meta.members.iter().map(|m| m.name.as_str()).collect();
            println!(
                "{}",
                serde_json::to_string(&names).expect("names serialise")
            );
            true
        }),
        ["test", tier, rest @ ..] => match (Tier::parse(tier), Options::parse(rest)) {
            (Some(tier), Some(options)) => run_tier(&root, tier, &options),
            _ => return usage(),
        },
        _ => return usage(),
    };
    match result {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(message) => {
            eprintln!("xtask: {message}");
            ExitCode::from(1)
        }
    }
}

fn usage() -> ExitCode {
    eprintln!("{USAGE}");
    ExitCode::from(2)
}

#[derive(Debug, Default, PartialEq, Eq)]
struct Options {
    /// `-p <crate>`: narrow `unit` to one crate.
    only: Option<String>,
    /// `--root-only`: leave the cockpit workspace out of `static`.
    root_only: bool,
}

impl Options {
    fn parse(args: &[&str]) -> Option<Options> {
        let mut options = Options::default();
        let mut it = args.iter();
        while let Some(arg) = it.next() {
            match *arg {
                "-p" => options.only = Some(it.next()?.to_string()),
                "--root-only" => options.root_only = true,
                _ => return None,
            }
        }
        Some(options)
    }
}

/// Print a check's violations. Returns whether it passed.
fn report(label: &str, errors: Vec<String>) -> bool {
    for e in &errors {
        eprintln!("xtask: {label}: {e}");
    }
    if errors.is_empty() {
        println!("xtask: {label}: ok");
    }
    errors.is_empty()
}

/// Run one command from the repository root. Returns whether it succeeded.
fn run(root: &Path, cmd: &Cmd) -> bool {
    println!(
        "xtask: [{}] {} {}",
        cmd.label,
        cmd.program.display(),
        cmd.args.join(" ")
    );
    match Command::new(&cmd.program)
        .args(&cmd.args)
        .current_dir(root)
        .status()
    {
        Ok(status) => status.success(),
        Err(e) => {
            eprintln!(
                "xtask: [{}] could not start {}: {e}",
                cmd.label,
                cmd.program.display()
            );
            false
        }
    }
}

fn run_tier(root: &Path, tier: Tier, options: &Options) -> Result<bool, String> {
    if options.only.is_some() && tier != Tier::Unit {
        return Err("-p narrows the unit tier only".into());
    }
    if options.root_only && tier != Tier::Static {
        return Err("--root-only applies to the static tier only".into());
    }
    let meta = Meta::load(root)?;
    let mut failed: Vec<String> = Vec::new();
    let mut ran = 0usize;
    let mut step = |label: &str, ok: bool| {
        ran += 1;
        if !ok {
            failed.push(label.to_string());
        }
    };

    let cmds = match tier {
        Tier::Static => {
            if !options.root_only && !tiers::sidecar_built(root) {
                return Err(
                    "the cockpit workspace cannot be linted before its sidecar is built: run \
                     `npm ci` and `npm run sidecar` in cockpit/ui, or pass --root-only if you \
                     changed nothing under cockpit/"
                        .into(),
                );
            }
            step(
                "static dependency direction",
                report("dependency direction", deps::check(&meta)),
            );
            let workflow = workflow_path(root);
            let text = std::fs::read_to_string(&workflow)
                .map_err(|e| format!("could not read {}: {e}", workflow.display()))?;
            step(
                "static ci job names",
                report("ci job names", ci::check(&text)),
            );
            tiers::static_cmds(root, options.root_only)
        }
        Tier::Unit => tiers::unit(&meta, options.only.as_deref())?,
        Tier::Contract => tiers::contract(&meta),
        Tier::Integration | Tier::E2e | Tier::Live => tiers::test_targets(&meta, tier),
    };
    for cmd in &cmds {
        step(&cmd.label, run(root, cmd));
    }

    if ran == 0 {
        println!("xtask: no tests in this tier yet ({})", tier.name());
        return Ok(true);
    }
    if failed.is_empty() {
        println!("xtask: {}: {ran} step(s) passed", tier.name());
        Ok(true)
    } else {
        eprintln!("xtask: {}: FAILED: {}", tier.name(), failed.join("; "));
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_parse_the_two_flags_and_refuse_anything_else() {
        assert_eq!(Options::parse(&[]), Some(Options::default()));
        assert_eq!(
            Options::parse(&["-p", "fleet-store", "--root-only"]),
            Some(Options {
                only: Some("fleet-store".into()),
                root_only: true
            })
        );
        assert_eq!(Options::parse(&["-p"]), None, "-p needs a crate name");
        assert_eq!(Options::parse(&["--fast"]), None);
    }

    #[test]
    fn the_root_holds_the_workspace_manifest_and_the_workflow() {
        let root = root();
        assert!(root.join("Cargo.toml").is_file());
        let workflow = workflow_path(&root);
        assert!(workflow.is_file(), "{} is missing", workflow.display());
    }

    /// The real workspace, not a fixture: catches a rule table, a tier selector or a
    /// job-name pin that has drifted from the repository it is meant to describe.
    #[test]
    fn the_real_workspace_satisfies_every_static_check() {
        let root = root();
        let meta = Meta::load(&root).expect("cargo metadata runs");
        assert_eq!(deps::check(&meta), Vec::<String>::new());

        let text = std::fs::read_to_string(workflow_path(&root)).unwrap();
        assert_eq!(ci::check(&text), Vec::<String>::new());

        let unit = tiers::unit(&meta, None).unwrap();
        for m in &meta.members {
            let label = format!("unit {}", m.name);
            assert!(
                unit.iter().any(|c| c.label == label),
                "no unit command for `{}`",
                m.name
            );
        }
        assert_eq!(
            meta.members.len(),
            deps::RULES.len(),
            "one dependency rule per workspace crate"
        );

        let lines = |tier| -> Vec<String> {
            tiers::test_targets(&meta, tier)
                .into_iter()
                .map(|c| c.args.join(" "))
                .collect()
        };
        assert!(
            lines(Tier::Contract)
                .iter()
                .any(|l| l.contains("-p harness-protocol --test contract")),
            "the contract tier must run the protocol contract test"
        );
        assert!(
            lines(Tier::Integration)
                .iter()
                .any(|l| l.contains("--test demo_mode_it")),
            "the integration tier must run the demo-mode test"
        );
    }
}
````

- [ ] **Step 4: Run the tests; exactly one fails, for the reason Task 8 fixes**

```
cargo test -p xtask
```

Expected: `test result: FAILED. 32 passed; 1 failed`. The failure is
`tests::the_real_workspace_satisfies_every_static_check`, and its message lists six missing
jobs: `static`, `unit (list crates)`, `unit (${{ matrix.crate }}, ${{ matrix.os }})`,
`unit (all crates)`, `contract (${{ matrix.os }})` and `integration`. No
dependency-direction error appears in it. If one does, a manifest from Task 2 or an
allowlist from Task 4 is wrong: fix that before going on.

- [ ] **Step 5: Try the commands that do not depend on CI**

```
cargo xtask deps
cargo xtask crates
cargo xtask test unit -p fleet-core
cargo xtask test e2e
```

Expected, in order: `xtask: dependency direction: ok`; a JSON array of fifteen names; two
runs of `fleet-core`'s tests, `cargo test --locked -p fleet-core --lib` and the same with
`--features testkit`, each with `30 passed`, then `xtask: unit: 2 step(s) passed`;
`xtask: no tests in this tier yet (e2e)`.

- [ ] **Step 6: Commit**

The commit is made with one test red on purpose; Task 8 turns it green. Say so in the message.

```
git add xtask/src/main.rs
git commit -m "feat(xtask): add the test, deps and crates commands (job-name pin red until the CI jobs land)"
```

### Task 8: The CI jobs

**Files:**
- Modify: `.github/workflows/ci.yml`

**Interfaces:**
- Consumes: `cargo xtask test static`, `cargo xtask test unit -p <crate>`,
  `cargo xtask test contract`, `cargo xtask test integration`, `cargo xtask crates`.
- Produces: the jobs `static`, `crates`, `unit`, `unit-all`, `contract` and `integration`,
  whose names are `static`, `unit (list crates)`, `unit (<crate>, <os>)`,
  `unit (all crates)`, `contract (<os>)` and `integration`.

**This is the final list of job names for milestone 0.** No later lane of the milestone adds,
renames or removes a job. Thirteen jobs; the check each one reports:

| Job id | Name in the workflow | Check contexts | New |
|---|---|---|---|
| `lint` | `fmt + clippy` | `fmt + clippy` | no |
| `check` | `svelte-check + tsc` | `svelte-check + tsc` | no |
| `test-ui` | `vitest (cockpit/ui)` | `vitest (cockpit/ui)` | no |
| `test` | `cargo test (workspace)` | `cargo test (workspace)` (required today) | no |
| `conformance` | `harness conformance` | `harness conformance` (required today) | no |
| `test-cockpit` | `cargo test (cockpit)` | `cargo test (cockpit)` | no |
| `build` | `tauri build (${{ matrix.os }})` | `tauri build (windows-latest)`, `tauri build (macos-latest)`, `tauri build (ubuntu-latest)` | no |
| `static` | `static` | `static` | yes |
| `crates` | `unit (list crates)` | `unit (list crates)` | yes |
| `unit` | `unit (${{ matrix.crate }}, ${{ matrix.os }})` | one per crate per OS, thirty today, for example `unit (fleetd, windows-latest)` | yes |
| `unit-all` | `unit (all crates)` | `unit (all crates)` | yes |
| `contract` | `contract (${{ matrix.os }})` | `contract (ubuntu-latest)`, `contract (windows-latest)` | yes |
| `integration` | `integration` | `integration` | yes |

What three of the new jobs run, so the names can be read against the requests they answer:

- `unit (<crate>, <os>)` runs `cargo xtask test unit -p <crate>`: the crate's in-source
  tests alone, then again with `--features testkit` if the crate has that feature, on
  `ubuntu-latest` and `windows-latest`. This is the spec-and-presets plan's CI request, met
  for every crate.
- `contract (<os>)` runs every `contract`-tier target and then the conformance kit against
  `harness-fake`. The kit prints six `PASS` lines until lane CC-PROTO merges, and seven after.
- `integration` runs the `integration` tier on Linux. Four `*_it` targets exist today:
  `demo_mode_it` needs no Docker and runs (3 tests); `local_docker_it`, `preflight_it` and
  `swarm_smoke_it` need Docker, are `#[ignore]`d, and report as ignored. Lane CC-CHAR's
  five `characterisation_*` targets join it when that lane merges. If the tier ever has no
  target at all, the job prints `xtask: no tests in this tier yet (integration)` and passes.

- [ ] **Step 1: Bring the header comment up to date**

In `.github/workflows/ci.yml`, replace these four lines:

```yaml
# TWO CARGO WORKSPACES: the root one (crates/fleet-core, crates/fleetd,
# crates/harness-protocol, crates/harness-conformance) and the standalone
# cockpit/ui/src-tauri crate. Nothing run from the repo root reaches the latter,
# so the fmt, clippy and test gates each run once per manifest.
```

with these three:

```yaml
# TWO CARGO WORKSPACES: the root one (every crate under crates/, plus xtask) and
# the standalone cockpit/ui/src-tauri crate. Nothing run from the repo root
# reaches the latter, so the fmt, clippy and test gates each run once per manifest.
```

- [ ] **Step 2: Append the new jobs**

Append the block below to the end of the file, after the last line of the `build` job. It
starts with a blank line. Change nothing above it.

```yaml

  # ---------------------------------------------------------------------------
  # Factory test tiers. Each job below runs one `cargo xtask test <tier>`, so CI
  # and a developer's machine run the same commands (xtask/src/tiers.rs says what
  # each tier runs). The jobs above are kept as they were.
  #
  # JOB NAMES ARE PINNED. Branch protection refers to a check by its job name, so
  # xtask/src/ci.rs lists every name in this file and the `static` job fails if
  # one is renamed or removed. Changing a name is an owner action first.
  # ---------------------------------------------------------------------------
  static:
    name: static
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy, rustfmt

      - name: Cache cargo registry + target
        uses: Swatinem/rust-cache@v2
        with:
          workspaces: |
            .
            cockpit/ui/src-tauri

      # The static tier lints the cockpit crate too, so that crate has to compile:
      # WebKitGTK system deps, plus the fleetd sidecar tauri-build resolves at
      # compile time (same prerequisites as the `lint` job).
      - name: Install Tauri Linux system deps
        run: |
          sudo apt-get update
          sudo apt-get install -y \
            libwebkit2gtk-4.1-dev \
            libgtk-3-dev \
            libayatana-appindicator3-dev \
            librsvg2-dev \
            patchelf \
            build-essential \
            curl \
            wget \
            file \
            libssl-dev

      - name: Install Node.js
        uses: actions/setup-node@v4
        with:
          node-version: 20
          cache: npm
          cache-dependency-path: cockpit/ui/package-lock.json

      - name: Install frontend dependencies
        run: npm ci
        working-directory: cockpit/ui

      - name: Build fleetd sidecar
        run: npm run sidecar
        working-directory: cockpit/ui

      # fmt, clippy (both cargo workspaces), dependency direction, the job-name
      # pin, the parity script's own tests, then registry parity.
      - name: Run the static tier
        run: cargo xtask test static

  # The unit matrix is built from the workspace itself, so a crate added later
  # gets a job without anyone editing this file.
  crates:
    name: unit (list crates)
    runs-on: ubuntu-latest
    outputs:
      crates: ${{ steps.list.outputs.crates }}
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable

      - name: Cache cargo registry + target
        uses: Swatinem/rust-cache@v2

      - name: List the workspace crates
        id: list
        run: |
          crates=$(cargo xtask crates)
          echo "$crates"
          test -n "$crates"
          test "$crates" != "[]"
          echo "crates=$crates" >> "$GITHUB_OUTPUT"

  # One crate per job: `cargo test -p <crate> --lib` builds that crate alone, so a
  # crate that only compiles because a sibling enabled a feature fails here. These
  # tests need no Docker, no network and no token.
  unit:
    name: unit (${{ matrix.crate }}, ${{ matrix.os }})
    needs: crates
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, windows-latest]
        crate: ${{ fromJSON(needs.crates.outputs.crates) }}
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable

      - name: Cache cargo registry + target
        uses: Swatinem/rust-cache@v2
        with:
          key: ${{ matrix.crate }}

      - name: Run this crate's unit tests in isolation
        shell: bash
        env:
          CRATE: ${{ matrix.crate }}
        run: cargo xtask test unit -p "$CRATE"

  # The one name branch protection needs for the whole matrix. It runs even when
  # a matrix job failed or was skipped, and passes only if every one succeeded.
  unit-all:
    name: unit (all crates)
    if: always()
    needs: [crates, unit]
    runs-on: ubuntu-latest
    steps:
      - name: Every per-crate unit job succeeded
        env:
          LIST: ${{ needs.crates.result }}
          UNIT: ${{ needs.unit.result }}
        run: |
          echo "list crates: $LIST; unit matrix: $UNIT"
          test "$LIST" = "success"
          test "$UNIT" = "success"

  # Contract tier: every crate's interface tests (tests/*.rs not named for another
  # tier), then the conformance kit against harness-fake. Run on Windows as well
  # because the kit spawns and kills child processes over pipes.
  contract:
    name: contract (${{ matrix.os }})
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, windows-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable

      - name: Cache cargo registry + target
        uses: Swatinem/rust-cache@v2

      - name: Run the contract tier
        run: cargo xtask test contract

  # Integration tier: tests/*.rs targets named `*_it` or `characterisation_*`, on
  # Linux. The targets that need Docker are `#[ignore]`d and stay ignored here: a
  # hosted runner has no cc-agent:dev image (see the coverage note at the top of
  # this file). What runs is everything in the tier that needs no Docker.
  integration:
    name: integration
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable

      - name: Cache cargo registry + target
        uses: Swatinem/rust-cache@v2

      - name: Run the integration tier
        run: cargo xtask test integration
```

- [ ] **Step 3: Check the size of the change and that no existing job changed**

```
git diff --stat .github/workflows/ci.yml
```

Expected: `1 file changed, 180 insertions(+), 4 deletions(-)`. The four deletions are the
header lines of Step 1.

- [ ] **Step 4: Run the test that was red**

```
cargo test -p xtask
```

Expected: `test result: ok. 33 passed; 0 failed`.

- [ ] **Step 5: Commit**

```
git add .github/workflows/ci.yml
git commit -m "ci: add the static, per-crate unit, contract and integration jobs that call cargo xtask"
```

### Task 9: The parity script

**Files:**
- Create: `scripts/parity.test.mjs`
- Create: `scripts/parity.mjs`

**Interfaces:**
- Consumes: `forms/registry.md`, `forms/<crate>.md`, `.github/workflows/ci.yml`, and the files
  that registry rows and Forms point at.
- Produces: `node scripts/parity.mjs [--root <dir>]`, which prints one `parity: <file>: <violation>`
  line per violation on stderr and exits 1, or prints `parity: ok (<n> gates, <m> forms)` and
  exits 0; and the module export `check(root) -> { errors: string[], gates: number, forms: number }`.

The script has no dependencies and reads the formats that Task 10 documents in
`forms/README.md`. Its tests build a small fixture repository in a temporary directory, so the
fixture Forms live inside the test file and need no directory of their own.

- [ ] **Step 1: Write the failing tests**

Create `scripts/parity.test.mjs`:

```js
// Tests for scripts/parity.mjs, run by `node --test scripts/parity.test.mjs`.
//
// Each test writes a small fixture repository into a temporary directory, changes one
// thing about it, and asserts the exact violation. The fixture is valid as written, so
// a test that expects a failure fails for one reason only.
import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';

import { check } from './parity.mjs';

const SCRIPT = fileURLToPath(new URL('./parity.mjs', import.meta.url));
const REPO = resolve(dirname(SCRIPT), '..');

const REGISTRY = `# Gate registry

| id | form | mechanism | location | command |
|----|------|-----------|----------|---------|
| workspace.G1 | (workspace) | Build graph: dependency allowlist | \`xtask/src/deps.rs#pub const RULES\` | \`cargo xtask test static\` |
| fleet-store.G1 | forms/fleet-store.md | Locked test: the event log contract | \`crates/fleet-store/tests/contract_event_log.rs\` | \`cargo xtask test contract\` |
`;

const FORM = `# Form: fleet-store

- status: draft
- owner: adbarc92
- level: E0
- last-drill: never
- interface-files:
  - crates/fleet-store/src/lib.rs

## Purpose

Durable state for the control plane: one append-only event log per unit.

## Interface

\`\`\`rust
pub fn append_event(&self, unit_id: &str, seq: u64, ts: i64, json: &str) -> Result<(), StoreError>;
\`\`\`

## Invariants

I1. Appending an event whose unit and sequence number already exist changes nothing.
I2. The crate depends on no workspace crate other than \`fleet-core\`.
I3. Everything written before the process exits is readable after the file is reopened.

## Hidden decisions

- The storage engine and its journal mode.

## Gates

| id | guards | mechanism | location | blocks |
|----|--------|-----------|----------|--------|
| fleet-store.G1 | I1 | Locked test: the event log contract | \`crates/fleet-store/tests/contract_event_log.rs\` | merge |
| workspace.G1 | I2 | Build graph: dependency allowlist | \`xtask/src/deps.rs#pub const RULES\` | merge |

## Unenforced

- I3 (2026-10-04): no gate yet. Planned: a reopen test in the contract tier.

## Regeneration

- Tests: \`cargo test -p fleet-store\`.
`;

const WORKFLOW = `jobs:
  static:
    name: static
    steps:
      - run: cargo xtask test static
  contract:
    name: contract
    steps:
      - run: cargo xtask test contract
`;

/** The valid fixture: path -> content. */
function files() {
  return {
    'forms/registry.md': REGISTRY,
    'forms/fleet-store.md': FORM,
    '.github/workflows/ci.yml': WORKFLOW,
    'xtask/src/deps.rs': 'pub const RULES: &[Rule] = &[];\n',
    'crates/fleet-store/src/lib.rs': '//! skeleton\n',
    'crates/fleet-store/tests/contract_event_log.rs': '#[test]\nfn appends() {}\n',
  };
}

/** Write a fixture repository, run `body(root)`, and remove it. */
function withRepo(fixture, body) {
  const root = mkdtempSync(join(tmpdir(), 'parity-'));
  try {
    for (const [rel, content] of Object.entries(fixture)) {
      if (content === null) continue;
      const path = join(root, ...rel.split('/'));
      mkdirSync(dirname(path), { recursive: true });
      writeFileSync(path, content);
    }
    return body(root);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

/** The violations for the valid fixture after `edit` has changed it. */
function errorsAfter(edit) {
  const fixture = files();
  edit(fixture);
  return withRepo(fixture, (root) => check(root).errors);
}

/** Assert exactly one violation, containing `text`. */
function assertOnly(errors, text) {
  assert.equal(errors.length, 1, `expected one violation, got: ${JSON.stringify(errors)}`);
  assert.ok(errors[0].includes(text), `expected "${text}" in "${errors[0]}"`);
}

const replaceIn = (key, from, to) => (f) => {
  assert.ok(f[key].includes(from), `fixture ${key} does not contain "${from}"`);
  f[key] = f[key].replace(from, to);
};

test('the valid fixture passes, and reports what it counted', () => {
  withRepo(files(), (root) => {
    assert.deepEqual(check(root), { errors: [], gates: 2, forms: 1 });
  });
});

test('a registry with a header and no rows fails', () => {
  const errors = errorsAfter((f) => {
    f['forms/registry.md'] = REGISTRY.split('\n').slice(0, 4).join('\n') + '\n';
    f['forms/fleet-store.md'] = null;
  });
  assertOnly(errors, 'the registry has no gate rows');
});

test('a registry whose header is misspelled fails instead of reading zero rows', () => {
  const errors = errorsAfter((f) => {
    replaceIn('forms/registry.md', '| command |', '| cmd |')(f);
    f['forms/fleet-store.md'] = null;
  });
  assertOnly(errors, 'no table with the header | id | form | mechanism | location | command |');
});

test('a missing registry fails', () => {
  const errors = errorsAfter((f) => {
    f['forms/registry.md'] = null;
    f['forms/fleet-store.md'] = null;
  });
  assertOnly(errors, 'forms/registry.md: missing');
});

test('a row with an empty command fails', () => {
  const errors = errorsAfter(
    replaceIn('forms/registry.md', '| `cargo xtask test contract` |', '| |'),
  );
  assertOnly(errors, 'fleet-store.G1: command is empty');
});

test('a row whose command CI does not run fails', () => {
  const errors = errorsAfter(
    replaceIn('forms/registry.md', '`cargo xtask test contract`', '`cargo test --all`'),
  );
  assertOnly(errors, 'command `cargo test --all` is not run by .github/workflows/ci.yml');
});

test('a row whose location does not exist fails', () => {
  const errors = errorsAfter((f) => {
    f['xtask/src/deps.rs'] = null;
  });
  assertOnly(errors, 'workspace.G1: location `xtask/src/deps.rs` does not exist');
});

test('a row whose location no longer contains its anchor fails', () => {
  const errors = errorsAfter((f) => {
    f['xtask/src/deps.rs'] = 'pub const TABLE: &[Rule] = &[];\n';
  });
  assertOnly(errors, 'does not contain `pub const RULES`');
});

test('a duplicated id fails', () => {
  const errors = errorsAfter((f) => {
    f['forms/registry.md'] += REGISTRY.trimEnd().split('\n').at(-1) + '\n';
  });
  assertOnly(errors, 'fleet-store.G1: duplicate id');
});

test('a row naming a Form that does not exist fails', () => {
  const errors = errorsAfter((f) => {
    f['forms/fleet-store.md'] = null;
  });
  assertOnly(errors, 'fleet-store.G1: form `forms/fleet-store.md` does not exist');
});

test('a Form that declares a gate absent from the registry fails', () => {
  const errors = errorsAfter(
    replaceIn('forms/fleet-store.md', '| workspace.G1 | I2 |', '| fleet-store.G9 | I2 |'),
  );
  assertOnly(errors, 'forms/fleet-store.md: gate fleet-store.G9 is not in forms/registry.md');
});

test('a Form gate whose location differs from the registry fails', () => {
  const errors = errorsAfter(
    replaceIn(
      'forms/fleet-store.md',
      '`crates/fleet-store/tests/contract_event_log.rs` | merge',
      '`crates/fleet-store/tests/other.rs` | merge',
    ),
  );
  assertOnly(errors, 'gate fleet-store.G1: location differs from forms/registry.md');
});

test('an invariant with neither a gate nor an Unenforced entry fails', () => {
  const errors = errorsAfter(
    replaceIn(
      'forms/fleet-store.md',
      '- I3 (2026-10-04): no gate yet. Planned: a reopen test in the contract tier.\n',
      '',
    ),
  );
  assertOnly(errors, 'invariant I3 has neither a gate nor an entry under Unenforced');
});

test('an Unenforced entry without a date or a reason fails', () => {
  const errors = errorsAfter(
    replaceIn('forms/fleet-store.md', '- I3 (2026-10-04): no gate yet.', '- I3: later.'),
  );
  assert.equal(errors.length, 2, JSON.stringify(errors));
  assert.ok(errors[0].includes('unenforced entry must read'));
  assert.ok(errors[1].includes('invariant I3 has neither'));
});

test('a gate that guards an invariant the Form does not declare fails', () => {
  const errors = errorsAfter(
    replaceIn('forms/fleet-store.md', '| workspace.G1 | I2 |', '| workspace.G1 | I2, I7 |'),
  );
  assertOnly(errors, 'gate workspace.G1 guards I7, which is not an invariant of this Form');
});

test('a gate that blocks nothing fails', () => {
  const errors = errorsAfter(
    replaceIn('forms/fleet-store.md', '`xtask/src/deps.rs#pub const RULES` | merge |', '`xtask/src/deps.rs#pub const RULES` | |'),
  );
  assertOnly(errors, 'gate workspace.G1: blocks must be one of build, commit, merge');
});

test('invariants must be numbered in order', () => {
  const errors = errorsAfter(replaceIn('forms/fleet-store.md', 'I2. The crate', 'I4. The crate'));
  assert.ok(errors.some((e) => e.includes('numbered I1, I2, ... in order; found I4')), JSON.stringify(errors));
});

test('a Form with no interface file, or one that does not exist, fails', () => {
  const none = errorsAfter(
    replaceIn('forms/fleet-store.md', '  - crates/fleet-store/src/lib.rs\n', ''),
  );
  assertOnly(none, 'interface-files lists no file');
  const missing = errorsAfter((f) => {
    f['crates/fleet-store/src/lib.rs'] = null;
  });
  assertOnly(missing, 'interface file `crates/fleet-store/src/lib.rs` does not exist');
});

test('a Form with an unknown status or a missing section fails', () => {
  const status = errorsAfter(replaceIn('forms/fleet-store.md', '- status: draft', '- status: done'));
  assertOnly(status, 'status must be one of draft, frozen, deprecated');
  const section = errorsAfter(replaceIn('forms/fleet-store.md', '## Hidden decisions', '## Notes'));
  assertOnly(section, 'missing section `## Hidden decisions`');
});

test('a Form with an unknown level or a malformed last-drill fails', () => {
  const level = errorsAfter(replaceIn('forms/fleet-store.md', '- level: E0', '- level: high'));
  assertOnly(level, 'level must be one of E0, E1, E2, E3');
  const drill = errorsAfter(replaceIn('forms/fleet-store.md', '- last-drill: never', '- last-drill: soon'));
  assertOnly(drill, 'last-drill must be `never` or a date written YYYY-MM-DD');
});

test('a Form whose heading does not match its file name fails', () => {
  const errors = errorsAfter(replaceIn('forms/fleet-store.md', '# Form: fleet-store', '# Form: store'));
  assertOnly(errors, 'first heading must be `# Form: fleet-store`');
});

test('README.md in forms/ is documentation, not a Form', () => {
  const errors = errorsAfter((f) => {
    f['forms/README.md'] = '# How Forms are written\n';
  });
  assert.deepEqual(errors, []);
});

test('a CRLF checkout gives the same answer as an LF one', () => {
  const fixture = files();
  for (const key of Object.keys(fixture)) fixture[key] = fixture[key].replace(/\n/g, '\r\n');
  withRepo(fixture, (root) => {
    assert.deepEqual(check(root), { errors: [], gates: 2, forms: 1 });
  });
});

test('the command line exits 0 on a valid tree and prints the counts', () => {
  withRepo(files(), (root) => {
    const out = execFileSync(process.execPath, [SCRIPT, '--root', root], { encoding: 'utf8' });
    assert.equal(out.trim(), 'parity: ok (2 gates, 1 forms)');
  });
});

test('the command line exits 1 and names each violation', () => {
  const fixture = files();
  fixture['xtask/src/deps.rs'] = null;
  fixture['crates/fleet-store/tests/contract_event_log.rs'] = null;
  withRepo(fixture, (root) => {
    const run = spawnSync(process.execPath, [SCRIPT, '--root', root], { encoding: 'utf8' });
    assert.equal(run.status, 1);
    assert.match(run.stderr, /parity: forms\/registry\.md: workspace\.G1: location `xtask\/src\/deps\.rs` does not exist/);
    assert.match(run.stderr, /parity: forms\/registry\.md: fleet-store\.G1: location `crates\/fleet-store\/tests\/contract_event_log\.rs` does not exist/);
    assert.match(run.stderr, /parity: FAILED \(2 violations\)/);
    assert.equal(run.stdout, '');
  });
});

test("this repository's own registry passes and is not empty", () => {
  const result = check(REPO);
  assert.deepEqual(result.errors, []);
  assert.ok(result.gates >= 4, `expected at least 4 gates, found ${result.gates}`);
});
```

- [ ] **Step 2: Run the tests and watch them fail**

```
node --test scripts/parity.test.mjs
```

Expected: the file fails to load with `ERR_MODULE_NOT_FOUND` for `parity.mjs`; the summary
shows `# pass 0`.

- [ ] **Step 3: Write the script**

Create `scripts/parity.mjs`:

```js
// Gate zero: the gate registry and what actually runs must agree.
//
//   node scripts/parity.mjs [--root <dir>]
//
// Reads forms/registry.md, every other forms/*.md, and .github/workflows/ci.yml under
// the root (default: the repository this script lives in). Prints one line per
// violation and exits 1, or prints `parity: ok (...)` and exits 0. The formats are
// documented in forms/README.md.
import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const REGISTRY = 'forms/registry.md';
const WORKFLOW = '.github/workflows/ci.yml';
const REGISTRY_HEADER = ['id', 'form', 'mechanism', 'location', 'command'];
const GATES_HEADER = ['id', 'guards', 'mechanism', 'location', 'blocks'];
const STATUSES = ['draft', 'frozen', 'deprecated'];
const LEVELS = ['E0', 'E1', 'E2', 'E3'];
const BLOCKS = ['build', 'commit', 'merge'];
const SECTIONS = [
  'Purpose',
  'Interface',
  'Invariants',
  'Hidden decisions',
  'Gates',
  'Unenforced',
  'Regeneration',
];
const WORKSPACE_FORM = '(workspace)';

/** Repository-relative, forward-slash path -> absolute path on this OS. */
function abs(root, rel) {
  return join(root, ...rel.split('/'));
}

function read(root, rel) {
  return readFileSync(abs(root, rel), 'utf8');
}

/** Lines of a file, tolerant of CRLF checkouts. */
function linesOf(text) {
  return text.split(/\r?\n/);
}

function cells(line) {
  return line
    .trim()
    .replace(/^\|/, '')
    .replace(/\|$/, '')
    .split('|')
    .map((c) => c.trim());
}

const isRow = (line) => line.trim().startsWith('|');
const isRule = (line) => /^\|[\s:|-]+\|?$/.test(line.trim());
const unquote = (s) => s.replace(/^`(.*)`$/, '$1');

/**
 * The first table in `lines` whose header row equals `header` (case-insensitive).
 * Returns its data rows as arrays of cells, or null when no such table exists.
 */
function table(lines, header) {
  for (let i = 0; i < lines.length; i++) {
    if (!isRow(lines[i])) continue;
    const got = cells(lines[i]).map((c) => c.toLowerCase());
    if (got.length !== header.length || got.some((c, j) => c !== header[j])) continue;
    const rows = [];
    for (let j = i + 1; j < lines.length && isRow(lines[j]); j++) {
      if (!isRule(lines[j])) rows.push(cells(lines[j]));
    }
    return rows;
  }
  return null;
}

/** `path` or `path#text`: the file must exist and, with `#text`, contain that text. */
function checkLocation(root, location) {
  const [rel, anchor] = unquote(location).split('#');
  if (!rel) return 'location is empty';
  if (!existsSync(abs(root, rel))) return `location \`${rel}\` does not exist`;
  if (anchor !== undefined && !read(root, rel).includes(anchor)) {
    return `location \`${rel}\` does not contain \`${anchor}\``;
  }
  return null;
}

function readRegistry(root, errors) {
  const gates = new Map();
  if (!existsSync(abs(root, REGISTRY))) {
    errors.push(`${REGISTRY}: missing`);
    return gates;
  }
  const rows = table(linesOf(read(root, REGISTRY)), REGISTRY_HEADER);
  if (rows === null) {
    errors.push(`${REGISTRY}: no table with the header | ${REGISTRY_HEADER.join(' | ')} |`);
    return gates;
  }
  if (rows.length === 0) {
    errors.push(`${REGISTRY}: the registry has no gate rows`);
    return gates;
  }
  const workflow = existsSync(abs(root, WORKFLOW)) ? read(root, WORKFLOW) : null;
  if (workflow === null) errors.push(`${WORKFLOW}: missing`);

  for (const row of rows) {
    if (row.length !== REGISTRY_HEADER.length) {
      errors.push(`${REGISTRY}: row \`${row.join(' | ')}\` has ${row.length} cells, expected 5`);
      continue;
    }
    const [id, form, mechanism, location, rawCommand] = row;
    const command = unquote(rawCommand);
    const where = `${REGISTRY}: ${id || '(no id)'}`;
    if (!/^[a-z0-9-]+\.G[0-9]+$/.test(id)) {
      errors.push(`${where}: id must look like <name>.G<number>`);
      continue;
    }
    if (gates.has(id)) {
      errors.push(`${where}: duplicate id`);
      continue;
    }
    gates.set(id, { form, location: unquote(location) });
    if (form !== WORKSPACE_FORM && !existsSync(abs(root, form))) {
      errors.push(`${where}: form \`${form}\` does not exist`);
    }
    if (!mechanism) errors.push(`${where}: mechanism is empty`);
    const bad = checkLocation(root, location);
    if (bad) errors.push(`${where}: ${bad}`);
    if (!command) {
      errors.push(`${where}: command is empty`);
    } else if (workflow !== null && !workflow.includes(command)) {
      errors.push(`${where}: command \`${command}\` is not run by ${WORKFLOW}`);
    }
  }
  return gates;
}

/** Split a Form into its header lines and its `## ` sections. */
function sectionsOf(lines) {
  const sections = new Map();
  const header = [];
  let current = null;
  for (const line of lines) {
    const m = /^## (.+?)\s*$/.exec(line);
    if (m) {
      current = m[1];
      sections.set(current, []);
    } else if (current === null) {
      header.push(line);
    } else {
      sections.get(current).push(line);
    }
  }
  return { header, sections };
}

function checkForm(root, name, gates, errors) {
  const rel = `forms/${name}.md`;
  const err = (message) => errors.push(`${rel}: ${message}`);
  const { header, sections } = sectionsOf(linesOf(read(root, rel)));

  if (!header.some((l) => l.trim() === `# Form: ${name}`)) {
    err(`first heading must be \`# Form: ${name}\``);
  }
  const field = (key) => {
    const line = header.find((l) => l.startsWith(`- ${key}:`));
    return line === undefined ? undefined : line.slice(key.length + 3).trim();
  };
  const status = field('status');
  if (!STATUSES.includes(status)) err(`status must be one of ${STATUSES.join(', ')}`);
  if (!field('owner')) err('owner is empty');
  if (!LEVELS.includes(field('level'))) err(`level must be one of ${LEVELS.join(', ')}`);
  if (!/^(never|[0-9]{4}-[0-9]{2}-[0-9]{2})$/.test(field('last-drill') ?? '')) {
    err('last-drill must be `never` or a date written YYYY-MM-DD');
  }

  const start = header.findIndex((l) => l.startsWith('- interface-files:'));
  const files = [];
  for (let i = start + 1; start >= 0 && i < header.length; i++) {
    const m = /^\s+- (.+?)\s*$/.exec(header[i]);
    if (!m) break;
    files.push(unquote(m[1]));
  }
  if (files.length === 0) err('interface-files lists no file');
  for (const file of files) {
    if (!existsSync(abs(root, file))) err(`interface file \`${file}\` does not exist`);
  }

  for (const section of SECTIONS) {
    if (!sections.has(section)) err(`missing section \`## ${section}\``);
  }

  const invariants = [];
  for (const line of sections.get('Invariants') ?? []) {
    const m = /^(I[0-9]+)\.\s+\S/.exec(line);
    if (m) invariants.push(m[1]);
  }
  if (invariants.length === 0) err('no invariant: write them as `I1. <sentence>`');
  invariants.forEach((id, i) => {
    if (id !== `I${i + 1}`) err(`invariants must be numbered I1, I2, ... in order; found ${id}`);
  });

  const guarded = new Set();
  const rows = table(sections.get('Gates') ?? [], GATES_HEADER);
  if (rows === null) {
    err(`no gates table with the header | ${GATES_HEADER.join(' | ')} |`);
  }
  for (const row of rows ?? []) {
    if (row.length !== GATES_HEADER.length) {
      err(`gate row \`${row.join(' | ')}\` has ${row.length} cells, expected 5`);
      continue;
    }
    const [id, guards, mechanism, location, blocks] = row;
    const registered = gates.get(id);
    if (!registered) {
      err(`gate ${id} is not in ${REGISTRY}`);
    } else if (registered.location !== unquote(location)) {
      err(`gate ${id}: location differs from ${REGISTRY}`);
    }
    if (!mechanism) err(`gate ${id}: mechanism is empty`);
    if (!BLOCKS.includes(blocks)) err(`gate ${id}: blocks must be one of ${BLOCKS.join(', ')}`);
    for (const inv of guards.split(',').map((g) => g.trim())) {
      if (!invariants.includes(inv)) err(`gate ${id} guards ${inv || '(nothing)'}, which is not an invariant of this Form`);
      guarded.add(inv);
    }
  }

  const unenforced = new Set();
  for (const line of sections.get('Unenforced') ?? []) {
    if (!line.startsWith('- ')) continue;
    const m = /^- (I[0-9]+) \(([0-9]{4}-[0-9]{2}-[0-9]{2})\): \S/.exec(line);
    if (!m) {
      err(`unenforced entry must read \`- I<n> (YYYY-MM-DD): <reason>\`; found \`${line}\``);
      continue;
    }
    if (!invariants.includes(m[1])) err(`unenforced entry names ${m[1]}, which is not an invariant of this Form`);
    unenforced.add(m[1]);
  }

  for (const id of invariants) {
    if (!guarded.has(id) && !unenforced.has(id)) {
      err(`invariant ${id} has neither a gate nor an entry under Unenforced`);
    }
  }
}

/** Run every check under `root`. Returns the violations and what was counted. */
export function check(root) {
  const errors = [];
  const gates = readRegistry(root, errors);
  const formsDir = abs(root, 'forms');
  const forms = existsSync(formsDir)
    ? readdirSync(formsDir)
        .filter((f) => f.endsWith('.md') && f !== 'registry.md' && f !== 'README.md')
        .map((f) => f.slice(0, -3))
        .sort()
    : [];
  for (const name of forms) checkForm(root, name, gates, errors);
  return { errors, gates: gates.size, forms: forms.length };
}

function main(argv) {
  const i = argv.indexOf('--root');
  const root =
    i >= 0 && argv[i + 1]
      ? resolve(argv[i + 1])
      : resolve(dirname(fileURLToPath(import.meta.url)), '..');
  const { errors, gates, forms } = check(root);
  for (const e of errors) console.error(`parity: ${e}`);
  if (errors.length > 0) {
    console.error(`parity: FAILED (${errors.length} violation${errors.length === 1 ? '' : 's'})`);
    return 1;
  }
  console.log(`parity: ok (${gates} gates, ${forms} forms)`);
  return 0;
}

// Compare as file URLs: on Windows `process.argv[1]` uses backslashes and a drive letter.
if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  process.exit(main(process.argv.slice(2)));
}
```

- [ ] **Step 4: Run the tests; exactly one fails, for the reason Task 10 fixes**

```
node --test scripts/parity.test.mjs
```

Expected: `# tests 26`, `# pass 25`, `# fail 1`. The failure is
`this repository's own registry passes and is not empty`, with `forms/registry.md: missing`.

- [ ] **Step 5: Commit**

```
git add scripts/parity.mjs scripts/parity.test.mjs
git commit -m "feat(parity): add the registry parity check and its tests (own-registry test red until the registry lands)"
```

### Task 10: The gate registry and the format document

**Files:**
- Create: `forms/registry.md`
- Create: `forms/README.md`

**Interfaces:**
- Consumes: `scripts/parity.mjs`; the gates that exist after Tasks 4, 6 and 8.
- Produces: the registry with four rows: `workspace.G1` (dependency direction and `testkit`),
  `workspace.G2` (the protocol contract's hash pin), `workspace.G3` (the conformance kit),
  `workspace.G4` (the CI job-name pin). And the public description of the Form and registry
  formats, which lane CC-FORMS and every later Form author follow. That document also states
  the rule for a gate that is planned today and built later: the lane that builds it says so
  in its pull request, and the coordinating lane edits the Form and the registry together.

The parity check is not a row: it is the thing that reads the rows, and its own tests (Task 9)
are what show it can fail.

- [ ] **Step 1: Create `forms/registry.md`**

```markdown
# Gate registry

One row per gate that runs in this repository. A gate is a check that fails, and blocks,
when a rule is broken. `scripts/parity.mjs` reads this table on every pull request and
fails when it disagrees with the Forms or with what CI runs. The columns are described in
[`README.md`](README.md).

| id | form | mechanism | location | command |
|----|------|-----------|----------|---------|
| workspace.G1 | (workspace) | Build graph: an allowlist of dependency edges asserted over `cargo metadata`, which also requires each library crate's `testkit` feature | `xtask/src/deps.rs#pub const RULES` | `cargo xtask test static` |
| workspace.G2 | (workspace) | Locked test: the committed protocol schema equals the generated one, and its hash equals the pinned constant | `crates/harness-protocol/tests/contract.rs#committed_contract_hash_is_pinned` | `cargo xtask test contract` |
| workspace.G3 | (workspace) | Conformance suite: the kit run against `harness-fake` | `crates/harness-conformance/src/cases.rs` | `cargo xtask test contract` |
| workspace.G4 | (workspace) | CI script: the job names branch protection refers to are pinned | `xtask/src/ci.rs#pub const PINNED_JOBS` | `cargo xtask test static` |
```

- [ ] **Step 2: Create `forms/README.md`**

`````markdown
# Forms and the gate registry

A **Form** is the written contract for one crate: what the crate is for, the interface other
code may rely on, the rules every implementation of it must keep, and the checks that
enforce those rules. The code behind a Form can be rewritten freely as long as its checks
pass. The Form changes only when its owner decides it should.

A **gate** is a check that fails, and blocks, when a rule is broken. `registry.md` lists every
gate that runs in this repository. `scripts/parity.mjs` compares the Forms, the registry and
the CI workflow on every pull request, and fails when they disagree.

## A Form: `forms/<crate>.md`

One file per crate, named for the crate. The layout is fixed, because a script reads it.

````markdown
# Form: <crate>

- status: draft
- owner: <a person's handle>
- level: E0
- last-drill: never
- interface-files:
  - crates/<crate>/src/lib.rs

## Purpose

## Interface

## Invariants

I1. <a sentence a test can prove false>
I2. <another>

## Hidden decisions

- <a choice callers must not depend on>

## Gates

| id | guards | mechanism | location | blocks |
|----|--------|-----------|----------|--------|
| <crate>.G1 | I1 | <what kind of check> | `<path>#<text in that file>` | merge |

## Unenforced

- I2 (2026-10-04): <why nothing enforces it yet, and what is planned>

## Regeneration
````

| Part | What goes in it |
|---|---|
| `status` | `draft` until the owner approves the Form; `frozen` once approved; `deprecated` when the crate is being retired. |
| `owner` | The named person who decides changes to this Form. |
| `level` | How far enforcement has got. `E0`: no gate protects this Form yet. `E1`: at least one gate blocks. `E2`: every invariant has a gate, so `Unenforced` is empty. `E3`: someone has thrown the implementation away and written it again using only this document and the crate's tests, and every gate passed. |
| `last-drill` | The date that rewrite was last done, or `never`. |
| `interface-files` | The source files that hold the interface. Each must exist. A change to one of them is a change to the Form. |
| Purpose | Two or three sentences on why the crate exists, including the failure a user would see if it were missing. |
| Interface | Everything other code may call or name: function signatures, types, events and error cases. What is absent from this section is free to change. |
| Invariants | Numbered `I1.`, `I2.`, … in order, one per line. Each is true of every implementation, and specific enough that a test could show it to be false. |
| Hidden decisions | Things the implementation is free to choose and callers must not rely on, such as a storage engine. Listing them stops a later change from leaking one into the interface. |
| Gates | One row per gate that guards this Form. `id` is the gate's id in the registry. `guards` lists the invariants it protects, separated by commas. `location` must equal the registry's. `blocks` is `build`, `commit` or `merge`. |
| Unenforced | One line per invariant no gate protects yet, written `- I<n> (YYYY-MM-DD): <reason>`. Each line is work still owed. |
| Regeneration | Practical notes for rewriting the crate behind its interface: the commands that run its tests, where its fixtures and test kit live, what the tests assume about the machine, and which crates it may not depend on. |

Every invariant must appear in the `guards` cell of a gate or on a line under `Unenforced`. A
gate that is planned but not built goes under `Unenforced`, not in the table: the table and
the registry describe only what runs today.

## The registry: `forms/registry.md`

One table, one row per gate.

| Column | Meaning |
|---|---|
| `id` | `<name>.G<number>`, unique. `<name>` is the crate, or `workspace` for a rule that spans crates. |
| `form` | The Form that defines the gate, as `forms/<crate>.md`, or `(workspace)`. |
| `mechanism` | The kind of check: type system, build graph, locked test, conformance suite, CI script. |
| `location` | A file, optionally followed by `#` and a piece of text that file must contain. |
| `command` | The command that runs the gate. It must be a command the CI workflow runs. |

A Form may cite a `(workspace)` gate in its own table when that gate protects one of its
invariants.

## What the parity check fails on

- The registry is missing, has no table with exactly these five columns, or has no rows.
- A row has an empty mechanism or command, a duplicated or malformed id, a `form` that does
  not exist, a `location` whose file does not exist or no longer contains its text, or a
  `command` that `.github/workflows/ci.yml` does not run.
- A Form's heading, header fields, sections, invariant numbering or gates table do not follow
  the layout above, or one of its `interface-files` does not exist.
- A Form lists a gate that is not in the registry, or gives it a different location.
- A gate guards an invariant its Form does not declare.
- An invariant has neither a gate nor a line under `Unenforced`.

Run it with `cargo xtask test static`, or on its own with `node scripts/parity.mjs`.

## When a planned gate is built

A Form lists a gate that does not exist yet under `Unenforced`, with what is planned. The lane
that later builds the gate does not edit the Form or the registry. It says so in its pull
request, naming the invariant, the file that holds the check and the command that runs it.
The coordinating lane then makes both edits together: it adds the row to `registry.md`, adds
the matching row to the Form's Gates table, and removes the invariant's line from
`Unenforced`. Until that change merges the invariant stays listed as unenforced, which is
accurate: no registered gate protects it yet.
`````

- [ ] **Step 3: Run the parity tests and the check itself**

```
node --test scripts/parity.test.mjs
node scripts/parity.mjs
```

Expected: `# pass 26`, `# fail 0`; then `parity: ok (4 gates, 0 forms)`.

- [ ] **Step 4: Watch gate zero fail on the real tree**

In `forms/registry.md`, change `crates/harness-conformance/src/cases.rs` to
`crates/harness-conformance/src/gone.rs`, then:

```
node scripts/parity.mjs
```

Expected, on stderr, and exit code 1:

```
parity: forms/registry.md: workspace.G3: location `crates/harness-conformance/src/gone.rs` does not exist
parity: FAILED (1 violation)
```

Change the path back by hand (the file is not committed yet, so git cannot restore it) and
confirm:

```
node scripts/parity.mjs
```

Expected: `parity: ok (4 gates, 0 forms)`.

- [ ] **Step 5: Run the whole static tier**

```
cargo xtask test static --root-only
```

Expected: `xtask: dependency direction: ok`, `xtask: ci job names: ok`, a clean `fmt` and
`clippy`, `# pass 26`, `parity: ok (4 gates, 0 forms)`, and the last line
`xtask: static: 6 step(s) passed`.

- [ ] **Step 6: Commit**

```
git add forms/registry.md forms/README.md
git commit -m "docs(forms): add the gate registry and the Form and registry formats"
```

### Task 11: Check that every dependency request is declared

**Files:**
- Modify (only if this check finds a request that Task 2 lacks): `Cargo.toml`, a crate's
  `Cargo.toml`, `xtask/src/deps.rs`, `Cargo.lock`

**Interfaces:**
- Consumes: the "Requests to the coordinator" section of each plan listed below; the
  manifests written in Task 2.
- Produces: the evidence, for the pull request, that every request is declared and locked;
  and the declaration of any request that has appeared since this plan was written.

The requests of the two milestone-0 sibling plans are already declared, in Task 2. This task
does not add them. It proves they are there, against those plans as they stand on the day
the lane runs, and it picks up the milestone-1 plans if they exist by then.

- [ ] **Step 1: Find the request sections**

*Git-Bash:*

```bash
for f in m0-contracts m0-spec-and-presets m1-control-plane m1-cockpit; do
  p="docs/superpowers/plans/2026-10-04-factory-$f.md"
  if [ -f "$p" ]; then grep -n "^## Requests to the coordinator" "$p" || echo "$p: no requests section"; else echo "$p: ABSENT"; fi
done
```

Expected when this plan was written:

```
42:## Requests to the coordinator
75:## Requests to the coordinator
docs/superpowers/plans/2026-10-04-factory-m1-control-plane.md: ABSENT
docs/superpowers/plans/2026-10-04-factory-m1-cockpit.md: ABSENT
```

The line numbers may have moved. If one of the first two plans is absent or has no such
section, stop and report: Task 2 was written from them. For a milestone-1 plan, `ABSENT` is
acceptable: write "not collected: the plan did not exist at `<git rev-parse --short HEAD>`"
in the pull-request body. Do not guess what it would ask for; milestone 1's coordinator
collects it.

- [ ] **Step 2: Check the two manifests the spec-and-presets plan gives in full**

That plan prints the exact `Cargo.toml` it needs for each of its crates. Compare each with
the file Task 2 created. *Git-Bash:*

```bash
p=docs/superpowers/plans/2026-10-04-factory-m0-spec-and-presets.md
for c in factory-spec factory-presets; do
  awk -v want="crates/$c/Cargo.toml\` reads:" 'index($0, want) {armed=1; next} armed && /^```toml/ {on=1; next} on && /^```/ {exit} on {print}' "$p" | diff - "crates/$c/Cargo.toml" && echo "$c: the manifest matches the request"
done
```

Expected:

```
factory-spec: the manifest matches the request
factory-presets: the manifest matches the request
```

A `diff` here means the sibling plan changed after this one was written. A different
`description` line is allowed by that plan and can be ignored. Any other difference is a new
or changed request: apply it with Step 6.

- [ ] **Step 3: Check the three requests of the contracts plan**

*Git-Bash:*

```bash
grep -n '^sha2 = { workspace = true }' crates/harness-protocol/Cargo.toml
grep -n '^serde_json = { workspace = true }\|^fleet-core = { path = "../fleet-core" }\|^\[dev-dependencies\]' crates/harness-conformance/Cargo.toml
```

Expected:

```
16:sha2 = { workspace = true }
10:serde_json = { workspace = true }
12:[dev-dependencies]
13:fleet-core = { path = "../fleet-core" }
```

`serde_json` is above the `[dev-dependencies]` line, so it is a normal dependency (R3);
`fleet-core` is below it, so it is a dev-dependency (R2); `sha2` is a normal dependency of
`harness-protocol` (R1).

- [ ] **Step 4: Check the CI request of the spec-and-presets plan**

```
cargo xtask test unit -p factory-spec
cargo xtask test unit -p factory-presets
```

Expected, for each: two `cargo test` runs, the second with `--features testkit`, then
`xtask: unit: 2 step(s) passed`. The `unit` matrix of Task 8 runs exactly this command for
every crate on `ubuntu-latest` and `windows-latest`.

- [ ] **Step 5: Read each request section in full, against the table in Task 2**

Read the whole "Requests to the coordinator" section of every plan Step 1 found. Each request
in it must be a row of the table "Requests from the sibling plans" in Task 2. Two outcomes:

- Every request is in the table: write "every request of `<plan>` is declared" in the
  pull-request body, with the output of Steps 2 to 4, and go to Step 8.
- A request is not in the table (a sibling plan changed, or a milestone-1 plan now exists):
  add it with Steps 6 and 7.

Refuse a request, and report it to the owner instead of adding it, when it would break a rule
in Global Constraints: an I/O crate for `fleet-core`, `factory-spec` or `factory-presets`; a
Docker client crate for any crate but `fleet-runner`; a dependency on `fleetd` or `xtask`; or
an edge that would let a crate other than `fleetd` depend on every other.

- [ ] **Step 6: Add a missing request (only if Step 5 found one)**

If the request gives the manifest line in full, add that line exactly as given, in the table
it names. Otherwise, for an **external crate** `<dep>` wanted by `<crate>`:

1. Root `Cargo.toml`, under `[workspace.dependencies]`: add `<dep> = "<version>"`, or
   `<dep> = { version = "<version>", features = [...] }`. If it is already there, leave it,
   unless the request needs a feature it lacks: then add the feature. If the request states
   no version, run `cargo search <dep> --limit 1` and use the major and minor it prints (for
   `1.4.2`, write `"1.4"`).
2. `crates/<crate>/Cargo.toml`: add `<dep> = { workspace = true }` under `[dependencies]`, or
   under `[dev-dependencies]` for a test-only request (create that table if it is missing).

Then, in either case:

3. If `<crate>` is `fleet-core`, `factory-spec` or `factory-presets` and the dependency is a
   normal one: add `"<dep>"` to that crate's list in `RULES` in `xtask/src/deps.rs`, keeping
   the list in alphabetical order, and say in a comment beside it why the crate does no I/O.
   The I/O list in the same file still applies to everything the new crate reaches; Step 7
   checks it.
4. Lock it without touching anything else: `cargo update -p <crate>`.

For a **dependency on another workspace crate** `<other>`:

1. `crates/<crate>/Cargo.toml`: add `<other> = { path = "../<other>" }` under
   `[dependencies]`. For a test-only use, add it under `[dev-dependencies]`, with
   `features = ["testkit"]` if it is that crate's test kit that is wanted.
2. For a normal dependency only: add `"<other>"` to `<crate>`'s `rule(...)` list in `RULES`,
   in alphabetical order. A dev-dependency needs no rule change.
3. `cargo update -p <crate>`.

- [ ] **Step 7: Check each addition before the next (only if Step 6 ran)**

```
cargo xtask deps
cargo build --locked -p <crate>
```

Expected: `xtask: dependency direction: ok`, then a successful build. (`cargo xtask deps`
reads the graph with `--locked`, so it also fails if the lockfile is behind a manifest.)
Then, *Git-Bash:*

```bash
git diff Cargo.lock | grep -E '^-(version|checksum)'
```

Expected: no output. A line here means an already-locked package changed version, which this
task must not do: run `git checkout -- Cargo.lock` and repeat Step 6's `cargo update -p <crate>`.

If `cargo xtask deps` reports that a pure crate "reaches" an I/O crate through the new
dependency, the request is refused: undo it and record why.

- [ ] **Step 8: Run the suite, and commit if anything changed**

*Git-Bash:*

```bash
cargo test --workspace --locked 2>&1 | grep -E "^test result" | awk '{p+=$4; f+=$6; i+=$8} END {print "passed="p" failed="f" ignored="i}'
```

Expected: `passed=209 failed=0 ignored=3` (a dependency adds no test).

If Steps 6 and 7 did not run, there is nothing to commit: `git status --short` is empty. Say
so in the pull-request body and go on. Otherwise:

```
git add Cargo.toml Cargo.lock crates/*/Cargo.toml xtask/src/deps.rs
git commit -m "chore(workspace): declare the dependency requests that arrived after the scaffold plan"
```

### Task 12: Verify, push and open the pull request

**Files:** none.

**Interfaces:**
- Consumes: everything above.
- Produces: the pull request `feat/m0-scaffold` into `factory/m0`.

- [ ] **Step 1: Run every command in this lane's Verify table**

Record the real output of each. If one differs from its expected result, stop and fix it, or
report it; do not open the pull request on a guess.

- [ ] **Step 2: Push**

```
git push -u origin feat/m0-scaffold
```

- [ ] **Step 3: Open the pull request**

*Git-Bash:*

```bash
gh pr create -R adbarc92/command-center --base factory/m0 --head feat/m0-scaffold \
  --title "Factory M0: workspace scaffold (skeleton crates, xtask tiers, parity check, CI jobs)" \
  --body "$(cat <<'EOF'
## What changed

- Ten skeleton crates and the `xtask` task runner join the workspace. `fleetd` is unchanged.
- `cargo xtask test <static|unit|contract|integration|e2e|live>` is the one command per tier.
- `cargo xtask test static` now also runs a dependency-direction check, a pin on the CI job
  names, and registry parity (`scripts/parity.mjs` over `forms/registry.md`).
- CI gains `static`, a per-crate `unit` matrix on Linux and Windows with `unit (all crates)`
  standing for it, `contract` on both, and `integration` on Linux. No existing job was
  renamed or removed. The list of job names is final for this milestone.
- Every dependency the contracts plan and the spec-and-presets plan ask for is declared and
  locked, so their lanes edit no manifest.
- Plan: `docs/superpowers/plans/2026-10-04-factory-m0-scaffold.md`, lane CC-COORD.

## How it was verified

<paste the Verify table with the real output of each command>

## Dependency requests

<the output of Task 11 Steps 2 to 4, and for each of the four plans: "every request is declared", or "not collected: absent at <sha>", or the requests added in Task 11 Step 6>

## For the owner

- Branch protection is unchanged. The names to require later are in the plan under
  "Owner actions this plan depends on".
- Two files outside the lane's original list were needed: `.cargo/config.toml` (the
  `cargo xtask` alias) and `forms/README.md` (the Form and registry formats).
- Three questions are open for you, at the top of the plan's owner-actions section.
EOF
)"
```

Replace the two `<...>` lines with the real content before running it.

- [ ] **Step 4: Watch the checks and report them**

```
gh pr checks --watch
```

Expected check names: the nine that exist today (`fmt + clippy`, `svelte-check + tsc`,
`vitest (cockpit/ui)`, `cargo test (workspace)`, `harness conformance`, `cargo test (cockpit)`
and three `tauri build (...)`), plus `static`, `unit (list crates)`, thirty
`unit (<crate>, <os>)`, `unit (all crates)`, `contract (ubuntu-latest)`,
`contract (windows-latest)` and `integration`: forty-five in all.

This is the first time the new jobs run on a hosted runner, so report each new job's result
by name. If a `windows-latest` job is red:

- in `xtask`, `scripts/` or a skeleton crate: it is this lane's to fix, here;
- in `fleetd`, `fleet-core`, `harness-protocol` or `harness-conformance`: do not edit that
  crate. Report the job, the failing test and the log excerpt. If the only failure is
  `concurrent_missions_cannot_both_breach_the_cap`, it is issue #72, which lane CC-72 fixes.

Do not merge. A reviewing session merges the pull request into `factory/m0`.

---

## Lane CC-CHAR

**Owns:** new files `crates/fleetd/tests/characterisation_*.rs`, and nothing else:
`characterisation_store.rs`, `characterisation_missions.rs`, `characterisation_admission.rs`,
`characterisation_reconcile.rs`, `characterisation_forge.rs`.

**Reads:** `crates/fleetd/src/server.rs`, `store.rs`, `reconcile.rs`, `driver.rs`, `forge.rs`,
`fake.rs`, `runner.rs`; the existing tests in `crates/fleetd/tests/` and in the `mod tests` of
each of those files, to avoid repeating what they already pin.

**Worktree and branch:** `D:\MajorProjects\.swarm-wt\m0-cc-char`, on `test/m0-characterisation`,
cut from `origin/factory/m0`. Pull request: `test/m0-characterisation` into `factory/m0`.

**Needs:** lane CC-COORD merged into `factory/m0` (for `cargo xtask` and the `integration`
tier, which is where these targets run, locally and in the CI job `integration`).

**Blocks:** every milestone-1 lane that moves code out of `fleetd`. These tests are the proof
that such a move changed nothing.

**Verify** (run in the worktree, after Task 6):

| Command | Expected |
|---|---|
| `cargo test -p fleetd --test characterisation_store --test characterisation_missions --test characterisation_admission --test characterisation_reconcile --test characterisation_forge` | five `test result: ok.` lines: 7, 8, 9, 5 and 11 passed (cargo runs the targets in alphabetical order: admission, forge, missions, reconcile, store) |
| `cargo xtask test integration` | the same five targets, then `demo_mode_it` (3 passed) and three ignored Docker targets; last line `xtask: integration: 1 step(s) passed` |
| *Git-Bash:* `cargo test --workspace --locked 2>&1 \| grep -E "^test result" \| awk '{p+=$4; f+=$6; i+=$8} END {print "passed="p" failed="f" ignored="i}'` | `passed=<N+40> failed=0 ignored=3`, where `N` is the count recorded in Task 1 (209 when lane CC-COORD is the only lane merged; every other merged lane raises it) |
| `cargo xtask test static --root-only` | last line `xtask: static: 6 step(s) passed` (the new files are formatted and clippy-clean) |
| `git diff --name-only origin/factory/m0...HEAD` | exactly the five files under **Owns** |
| `git status --short` | empty: no mutation from a "watch it fail" step was left behind |
| *Git-Bash:* `grep -n "CHARACTERISED-DEFECT" crates/fleetd/tests/*.rs` | two lines, both in `characterisation_admission.rs`, both naming `#74` |

### How these tests are written

A characterisation test pins behaviour that already exists, so there is no implementation to
write and the test passes the first time it runs. A test nobody has seen fail proves nothing.
So each task below has a **watch it fail** step: make one small change to the production code
in your worktree, run the file, see the named tests fail, and undo the change with
`git checkout -- crates/fleetd/src`. That change is never committed: this lane does not own
`crates/fleetd/src/`. The lane's final `git status --short` must be empty.

Three rules keep these tests from becoming the next flaky check (Review Focus 5):

1. Never assert something that depends on whether a background driver has finished. A demo
   unit finishes in milliseconds, and a finished unit counts what it spent (9 cents for one
   review round) where a running one counts what it reserved ($5).
2. Wait for something to happen by polling the store with a ten-second deadline, never by
   sleeping for a fixed time. (Two tests sleep briefly to check that nothing further
   happened. A check of that kind cannot fail because a machine was slow.)
3. Each test builds its own store. Nothing is shared between tests.

The tests read two environment variables indirectly (`CC_GLOBAL_USD_CAP`,
`CC_MAX_CONCURRENT`), because `AppState::new` does. They pin the defaults, $20 a day and three
units at once, and fail with a clear message if either variable is set.

What is already pinned, and what this lane adds:

| Behaviour | Already pinned by | Added by this lane |
|---|---|---|
| Admission refuses at the 24-hour spend ceiling | `server::tests::create_mission_refused_over_global_cap`, `create_mission_refused_when_committed_reservations_breach_cap`, `concurrent_missions_cannot_both_breach_the_cap`; `store::tests::committed_spend_counts_reservations_and_overcap_and_planner`, `committed_spend_window_excludes_old` | Through the route: the status and the exact body; that the comparison is "at or above"; that nothing is stored; the 24-hour boundary; that a refusal uses up a unit id |
| The concurrency limit | `driver::tests::driver_waits_for_a_concurrency_slot` (one driver, one slot) | Through the route: the default of three; that a unit waiting at its oracle gate keeps its slot; that a fourth unit waits and says so; that it runs when a slot frees |
| What `POST /missions` stores | nothing | Every column of the stored row; which request fields are honoured and which are ignored; refusals before and inside the handler; id allocation |
| The event forwarder's projection | `server::tests::fold_persists_oracle_hash_and_freezes_on_oracle_proposed` (one event kind, through a private function) | That a finished unit's row equals the fold of its log, for every projected column; the log is contiguous; the exact first event; the exact phase sequence |
| Startup reconcile | `reconcile::tests::*` (the decision table); `server::tests::reconcile_halts_stranded_unit_with_coherent_event` | The exact synthetic event; that cost and oracle hash survive; finished units untouched; which containers are reaped; that units waiting on a person are rewritten to `halted`; resume through the command route |
| The store's event log and rollups | `store::tests::*` (listed at the top of the new file) | Ordering and byte-exactness; that a repeated sequence number keeps the first payload; set-once columns beyond the two already pinned; the spend window's boundary; rollup phases; durability across a reopen |
| The forge seam | `demo_mode_it` (a clean run ends in a PR artifact); `gh_forge::tests::*` (parsing) | Which forge methods the driver calls, in what order, with what arguments; what a conflict, a non-mergeable PR, a PR that stays pending and each kind of forge error does to the unit |
| Issue #74 | nothing | Two tests that record the defect as today's behaviour |

### Task 1: Create the worktree and record the lane's baseline

**Files:** none.

**Interfaces:**
- Consumes: `origin/factory/m0` with lane CC-COORD merged.
- Produces: the worktree, and the number `N` used by this lane's Verify.

- [ ] **Step 1: Create the worktree**

```
git -C D:/MajorProjects/INFRASTRUCTURE/command-center fetch origin
git -C D:/MajorProjects/INFRASTRUCTURE/command-center worktree add --no-track -b test/m0-characterisation D:/MajorProjects/.swarm-wt/m0-cc-char origin/factory/m0
```

Every later command in this lane runs in `D:\MajorProjects\.swarm-wt\m0-cc-char`.

- [ ] **Step 2: Confirm the scaffold is there, and record `N`**

```
cargo xtask crates
```

Expected: a JSON array of crate names. If cargo says there is no such command `xtask`, lane
CC-COORD has not merged: stop and report.

*Git-Bash:*

```bash
cargo test --workspace --locked 2>&1 | grep -E "^test result" | awk '{p+=$4; f+=$6; i+=$8} END {print "passed="p" failed="f" ignored="i}'
```

Expected: `passed=209 failed=0 ignored=3` when lane CC-COORD is the only lane merged. Every
other lane that has merged raises the count (lane CC-72 by 1, lane CC-PROTO by 100, and so
on), so a larger number is not a problem. Write the number down as `N`. If the only failure
is
`server::tests::concurrent_missions_cannot_both_breach_the_cap`, that is issue #72: run once
more and record both results.

### Task 2: The store

**Files:**
- Create: `crates/fleetd/tests/characterisation_store.rs`

**Interfaces:**
- Consumes: `fleetd::store::{Store, UnitRow}`: `Store::open`, `open_memory`, `upsert_unit`,
  `update_unit`, `append_event`, `events_since`, `get_unit`, `list_units`, `committed_spend`,
  `swarm_rollup`, `max_unit_seq`.
- Produces: eleven tests.

- [ ] **Step 1: Write the tests**

```rust
//! Characterisation: `fleetd::store` as it behaves today.
//!
//! These tests pin behaviour that already exists so that moving the store into its
//! own crate can be shown to change nothing. They assert what the code does, which
//! is not always what it should do. Changing an assertion here is a behaviour change
//! and needs a reviewer's decision; changing a `use` line is not.
//!
//! Already pinned inside `src/store.rs` and not repeated here:
//! `append_is_idempotent_on_seq` (row count only), `persists_mode_and_review_floor`,
//! `oracle_hash_round_trips_and_freezes_on_update`,
//! `committed_spend_counts_reservations_and_overcap_and_planner`,
//! `committed_spend_window_excludes_old`, `max_unit_seq_reads_highest_numeric_suffix`,
//! `max_swarm_seq_reads_highest_numeric_suffix`, `lane_crud_and_commit_is_atomic`,
//! `swarm_rollup_counts_terminal_and_awaiting_human`.

use fleetd::store::{Store, UnitRow};

fn unit(id: &str) -> UnitRow {
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

#[test]
fn events_come_back_in_seq_order_byte_for_byte() {
    let s = Store::open_memory().unwrap();
    // Appended out of order, with payloads that are not valid JSON and not trimmed:
    // the log stores text and never interprets it.
    s.append_event("u1", 3, 30, " third ").unwrap();
    s.append_event("u1", 1, 10, r#"{"seq":1}"#).unwrap();
    s.append_event("u1", 2, 20, "not json").unwrap();
    assert_eq!(
        s.events_since("u1", 0).unwrap(),
        [r#"{"seq":1}"#, "not json", " third "]
    );
}

#[test]
fn since_is_exclusive_and_each_unit_has_its_own_log() {
    let s = Store::open_memory().unwrap();
    for seq in 1..=3 {
        s.append_event("u1", seq, 0, &format!("a{seq}")).unwrap();
    }
    s.append_event("u2", 1, 0, "b1").unwrap();
    assert_eq!(s.events_since("u1", 1).unwrap(), ["a2", "a3"]);
    assert_eq!(s.events_since("u1", 3).unwrap(), Vec::<String>::new());
    assert_eq!(s.events_since("u2", 0).unwrap(), ["b1"]);
    assert_eq!(s.events_since("nobody", 0).unwrap(), Vec::<String>::new());
}

#[test]
fn a_repeated_seq_keeps_the_first_payload_and_reports_success() {
    let s = Store::open_memory().unwrap();
    s.append_event("u1", 1, 10, "first").unwrap();
    // The second write is dropped, and the caller is not told.
    s.append_event("u1", 1, 99, "second").unwrap();
    assert_eq!(s.events_since("u1", 0).unwrap(), ["first"]);
}

#[test]
fn an_event_can_be_logged_for_a_unit_that_has_no_row() {
    let s = Store::open_memory().unwrap();
    s.append_event("ghost", 1, 0, "e").unwrap();
    assert_eq!(s.events_since("ghost", 0).unwrap(), ["e"]);
    assert!(s.get_unit("ghost").unwrap().is_none());
    assert!(s.list_units().unwrap().is_empty());
}

#[test]
fn a_second_upsert_moves_the_projection_and_leaves_the_rest_as_first_written() {
    let s = Store::open_memory().unwrap();
    s.upsert_unit(&unit("u1"), 1000).unwrap();

    let mut later = unit("u1");
    later.tier = "t3".into();
    later.task = "a different task".into();
    later.repo_url = "https://example.invalid/other".into();
    later.repo_slug = "other/repo".into();
    later.base_branch = "develop".into();
    later.branch = "agent/other".into();
    later.test_cmd = "cargo test".into();
    later.usd_cap = 99.0;
    later.wall_clock_secs = 1;
    later.swarm_id = Some("sw9".into());
    later.oracle_hash = Some("h-later".into());
    later.phase = "building".into();
    later.cost = 0.25;
    later.last_seq = 7;
    later.oracle_frozen = true;
    later.terminal_reason = Some("why".into());
    s.upsert_unit(&later, 2000).unwrap();

    let got = s.get_unit("u1").unwrap().unwrap();
    // Written once, by the first upsert.
    assert_eq!(got.tier, "t1");
    assert_eq!(got.task, "first task");
    assert_eq!(got.repo_url, "https://example.invalid/repo");
    assert_eq!(got.repo_slug, "owner/repo");
    assert_eq!(got.base_branch, "main");
    assert_eq!(got.branch, "agent/u1");
    assert_eq!(got.test_cmd, "node --test");
    assert_eq!(got.usd_cap, 5.0);
    assert_eq!(got.wall_clock_secs, 1800);
    assert_eq!(got.swarm_id, None);
    assert_eq!(
        got.oracle_hash, None,
        "an upsert never changes a stored hash"
    );
    // Moved by every upsert.
    assert_eq!(got.phase, "building");
    assert_eq!(got.cost, 0.25);
    assert_eq!(got.last_seq, 7);
    assert!(got.oracle_frozen);
    assert_eq!(got.terminal_reason.as_deref(), Some("why"));
}

#[test]
fn update_unit_overwrites_the_terminal_reason_even_with_none() {
    // Unlike the oracle hash, which a `None` leaves alone, the reason is replaced.
    let s = Store::open_memory().unwrap();
    s.upsert_unit(&unit("u1"), 1).unwrap();
    s.update_unit(
        "u1",
        "halted",
        0.5,
        4,
        Some("daemon restarted"),
        Some("h1"),
        2,
    )
    .unwrap();
    s.update_unit("u1", "provisioning", 0.5, 5, None, None, 3)
        .unwrap();
    let got = s.get_unit("u1").unwrap().unwrap();
    assert_eq!(got.terminal_reason, None);
    assert_eq!(got.oracle_hash.as_deref(), Some("h1"));
    assert_eq!(got.phase, "provisioning");
    assert_eq!(got.last_seq, 5);
}

#[test]
fn updating_a_unit_that_does_not_exist_succeeds_and_creates_nothing() {
    let s = Store::open_memory().unwrap();
    s.update_unit("ghost", "done", 1.0, 9, Some("done"), None, 1)
        .unwrap();
    assert!(s.get_unit("ghost").unwrap().is_none());
    assert_eq!(s.committed_spend(0).unwrap(), 0.0);
}

#[test]
fn the_spend_window_is_measured_from_first_write_and_includes_its_boundary() {
    let s = Store::open_memory().unwrap();
    s.upsert_unit(&unit("u1"), 1000).unwrap();
    // A later upsert does not move the unit into a later window.
    let mut later = unit("u1");
    later.phase = "building".into();
    s.upsert_unit(&later, 5000).unwrap();

    assert_eq!(
        s.committed_spend(1000).unwrap(),
        5.0,
        "created exactly at `since` counts"
    );
    assert_eq!(
        s.committed_spend(1001).unwrap(),
        0.0,
        "created before `since` does not"
    );
}

#[test]
fn an_empty_store_has_spent_nothing_and_an_unknown_swarm_rolls_up_to_zero() {
    let s = Store::open_memory().unwrap();
    assert_eq!(s.committed_spend(0).unwrap(), 0.0);
    assert_eq!(s.swarm_rollup("nobody").unwrap(), (0, 0, 0));
    assert_eq!(s.max_unit_seq().unwrap(), 0);
}

#[test]
fn a_rollup_counts_halted_and_needs_human_as_awaiting_and_only_three_phases_as_terminal() {
    let s = Store::open_memory().unwrap();
    for (id, phase) in [
        ("u1", "done"),
        ("u2", "no_change"),
        ("u3", "failed"),
        ("u4", "halted"),
        ("u5", "needs_human"),
        ("u6", "pr_open"),
        ("u7", "awaiting_oracle_approval"),
    ] {
        let mut r = unit(id);
        r.phase = phase.into();
        r.swarm_id = Some("sw1".into());
        s.upsert_unit(&r, 1).unwrap();
    }
    // A unit of another swarm, and one of none, are not counted.
    let mut other = unit("u8");
    other.swarm_id = Some("sw2".into());
    s.upsert_unit(&other, 1).unwrap();
    s.upsert_unit(&unit("u9"), 1).unwrap();

    assert_eq!(s.swarm_rollup("sw1").unwrap(), (7, 3, 2));
}

#[test]
fn a_file_backed_store_keeps_units_and_events_across_a_reopen() {
    let path = std::env::temp_dir().join(format!(
        "fleetd-characterisation-store-{}-{:?}.db",
        std::process::id(),
        std::thread::current().id()
    ));
    let cleanup = || {
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
        }
    };
    cleanup();
    {
        let s = Store::open(&path).unwrap();
        let mut r = unit("u7");
        r.phase = "building".into();
        r.cost = 0.4;
        s.upsert_unit(&r, 1000).unwrap();
        s.append_event("u7", 1, 1000, "kept").unwrap();
    }
    {
        // Opening again runs the schema setup a second time; it must not lose data.
        let s = Store::open(&path).unwrap();
        let got = s.get_unit("u7").unwrap().unwrap();
        assert_eq!(got.phase, "building");
        assert_eq!(got.cost, 0.4);
        assert_eq!(s.events_since("u7", 0).unwrap(), ["kept"]);
        assert_eq!(s.max_unit_seq().unwrap(), 7);
        assert_eq!(s.committed_spend(0).unwrap(), 5.0);
    }
    cleanup();
}
```

- [ ] **Step 2: Watch it fail**

In `crates/fleetd/src/store.rs`, in `events_since`, change

```rust
            .prepare("SELECT json FROM events WHERE unit_id=?1 AND seq>?2 ORDER BY seq")?;
```

to

```rust
            .prepare("SELECT json FROM events WHERE unit_id=?1 AND seq>?2 ORDER BY seq DESC")?;
```

```
cargo test -p fleetd --test characterisation_store
```

Expected: `test result: FAILED. 9 passed; 2 failed`. The two are
`events_come_back_in_seq_order_byte_for_byte` and
`since_is_exclusive_and_each_unit_has_its_own_log`.

- [ ] **Step 3: Undo the change and watch it pass**

```
git checkout -- crates/fleetd/src
cargo test -p fleetd --test characterisation_store
```

Expected: `test result: ok. 11 passed; 0 failed`.

- [ ] **Step 4: Commit**

```
git status --short
```

Expected: one line, `?? crates/fleetd/tests/characterisation_store.rs`.

```
git add crates/fleetd/tests/characterisation_store.rs
git commit -m "test(fleetd): characterise the store's event log, set-once columns and rollups"
```

### Task 3: `POST /missions`, the forwarder's projection and the read routes

**Files:**
- Create: `crates/fleetd/tests/characterisation_missions.rs`

**Interfaces:**
- Consumes: `fleetd::server::{router, AppState}` and the routes `POST /missions`, `GET /units`,
  `GET /units/:id`, `GET /units/:id/events?since=`, `POST /units/:id/commands`, `GET /health`;
  `fleetd::store::{Store, UnitRow}`; `tower::ServiceExt::oneshot` and `axum::body::to_bytes`
  (both already dependencies of `fleetd`).
- Produces: nine tests. (Each file that drives the router carries its own small `Daemon`
  helper, because this lane owns no shared test module.)

- [ ] **Step 1: Write the tests**

```rust
//! Characterisation: what `POST /missions` stores, what the event forwarder writes
//! as a unit runs, and the shape of the read routes, as they behave today.
//!
//! Everything here goes through `fleetd::server::router` in demo mode: the scripted
//! runner and the fake forge, with no Docker, no network and no token. These tests
//! assert what the code does, which is not always what it should do. Changing an
//! assertion is a behaviour change and needs a reviewer's decision.
//!
//! Already pinned elsewhere and not repeated here:
//! `server::tests::ws_stream_delivers_the_unit_event_log_to_a_client` (the WebSocket
//! stream), `server::tests::fold_persists_oracle_hash_and_freezes_on_oracle_proposed`,
//! `server::tests::next_id_seeds_above_persisted_units`, the three CORS tests in
//! `server::tests`, and `demo_mode_it` (the demo driver's phases, cost and fake PR).

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use fleetd::server::{router, AppState};
use fleetd::store::{Store, UnitRow};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tower::ServiceExt;

const TERMINAL: [&str; 3] = ["done", "no_change", "failed"];

/// A daemon with an in-memory store and the default limits.
struct Daemon {
    store: Arc<Mutex<Store>>,
    state: AppState,
}

impl Daemon {
    fn new() -> Self {
        for key in ["CC_GLOBAL_USD_CAP", "CC_MAX_CONCURRENT"] {
            assert!(
                std::env::var_os(key).is_none(),
                "unset {key}: these tests pin the default limits"
            );
        }
        let store = Arc::new(Mutex::new(Store::open_memory().unwrap()));
        let state = AppState::new(store.clone());
        Daemon { store, state }
    }

    /// Send one request through the real router. Returns the status and the body.
    async fn send(&self, method: &str, uri: &str, body: Option<Value>) -> (StatusCode, String) {
        let request = Request::builder().method(method).uri(uri);
        let request = match body {
            Some(json) => request
                .header("content-type", "application/json")
                .body(Body::from(json.to_string())),
            None => request.body(Body::empty()),
        }
        .unwrap();
        let response = router(self.state.clone()).oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        (status, String::from_utf8(bytes.to_vec()).unwrap())
    }

    async fn get_json(&self, uri: &str) -> Value {
        let (status, body) = self.send("GET", uri, None).await;
        assert_eq!(status, StatusCode::OK, "GET {uri}: {body}");
        serde_json::from_str(&body).unwrap()
    }

    /// Create a mission and return its unit id.
    async fn mission(&self, body: Value) -> String {
        let (status, text) = self.send("POST", "/missions", Some(body)).await;
        assert_eq!(status, StatusCode::OK, "POST /missions: {text}");
        let reply: Value = serde_json::from_str(&text).unwrap();
        reply["unit_id"].as_str().unwrap().to_string()
    }

    fn row(&self, unit: &str) -> Option<UnitRow> {
        self.store.lock().unwrap().get_unit(unit).unwrap()
    }

    fn events(&self, unit: &str) -> Vec<Value> {
        let raw = self.store.lock().unwrap().events_since(unit, 0).unwrap();
        raw.iter()
            .map(|j| serde_json::from_str(j).unwrap())
            .collect()
    }

    /// Wait until the unit's stored phase is one of `phases`, and its row has caught
    /// up with its log. Fails after ten seconds instead of hanging.
    async fn wait_for(&self, unit: &str, phases: &[&str]) -> UnitRow {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(row) = self.row(unit) {
                if phases.contains(&row.phase.as_str())
                    && (!TERMINAL.contains(&row.phase.as_str()) || row.terminal_reason.is_some())
                {
                    return row;
                }
            }
            assert!(
                Instant::now() < deadline,
                "{unit} never reached {phases:?}; row: {:?}",
                self.row(unit)
            );
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }
}

fn keys(v: &Value) -> Vec<&str> {
    let mut keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
    keys.sort_unstable();
    keys
}

#[tokio::test]
async fn a_mission_with_only_a_task_is_stored_with_these_defaults() {
    let d = Daemon::new();
    let (status, body) = d
        .send(
            "POST",
            "/missions",
            Some(json!({"task": "add a sum() helper"})),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, r#"{"unit_id":"u1"}"#);

    // The row exists by the time the response is sent. These columns never change.
    let row = d.row("u1").expect("the row is written before the reply");
    assert_eq!(row.unit_id, "u1");
    assert_eq!(row.tier, "t1");
    assert_eq!(row.task, "add a sum() helper");
    assert_eq!(
        row.repo_url,
        "https://github.com/adbarc92/command-center-agent-sandbox"
    );
    assert_eq!(row.repo_slug, "adbarc92/command-center-agent-sandbox");
    assert_eq!(row.base_branch, "main");
    assert_eq!(row.branch, "agent/u1");
    assert_eq!(row.test_cmd, "node --test");
    assert_eq!(row.usd_cap, 5.0);
    assert_eq!(row.wall_clock_secs, 1800);
    assert_eq!(row.mode, "demo");
    assert_eq!(row.min_review_rounds, 2);
    assert_eq!(row.swarm_id, None);
}

#[tokio::test]
async fn the_request_sets_tier_and_review_floor_and_a_floor_of_zero_becomes_one() {
    let d = Daemon::new();
    let id = d
        .mission(json!({"task": "x", "tier": "t3", "mode": "demo", "min_review_rounds": 0}))
        .await;
    let row = d.row(&id).unwrap();
    assert_eq!(row.tier, "t3");
    assert_eq!(row.min_review_rounds, 1);
    // The caller cannot choose the repository, the branch, the cap or the test command.
    let id = d
        .mission(json!({"task": "y", "usd_cap": 99.0, "branch": "main", "repo_slug": "evil/repo"}))
        .await;
    let row = d.row(&id).unwrap();
    assert_eq!(row.usd_cap, 5.0);
    assert_eq!(row.branch, "agent/u2");
    assert_eq!(row.repo_slug, "adbarc92/command-center-agent-sandbox");
}

#[tokio::test]
async fn an_unknown_mode_is_refused_stores_nothing_and_still_uses_up_an_id() {
    let d = Daemon::new();
    let (status, body) = d
        .send(
            "POST",
            "/missions",
            Some(json!({"task": "x", "mode": "weird"})),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body, "unknown mode: weird");
    assert!(d.store.lock().unwrap().list_units().unwrap().is_empty());
    // The id counter moved before the mode was checked, so `u1` is never issued.
    assert_eq!(d.mission(json!({"task": "x"})).await, "u2");
}

#[tokio::test]
async fn a_malformed_body_is_refused_before_the_handler_and_uses_up_no_id() {
    let d = Daemon::new();
    for bad in [
        json!({}),
        json!({"task": "x", "tier": "t9"}),
        json!({"task": "x", "tier": "T1"}),
        json!({"task": "x", "min_review_rounds": -1}),
    ] {
        let (status, _) = d.send("POST", "/missions", Some(bad.clone())).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{bad}");
    }
    assert!(d.store.lock().unwrap().list_units().unwrap().is_empty());
    assert_eq!(d.mission(json!({"task": "x"})).await, "u1");
}

#[tokio::test]
async fn a_finished_unit_s_row_is_the_fold_of_its_event_log() {
    let d = Daemon::new();
    let id = d.mission(json!({"task": "add a sum() helper"})).await;
    let row = d.wait_for(&id, &TERMINAL).await;
    let events = d.events(&id);

    // The log is contiguous from 1, and every entry is an envelope for this unit.
    let seqs: Vec<u64> = events.iter().map(|e| e["seq"].as_u64().unwrap()).collect();
    assert_eq!(seqs, (1..=events.len() as u64).collect::<Vec<_>>());
    for e in &events {
        assert_eq!(keys(e), ["event", "seq", "unit_id"]);
        assert_eq!(e["unit_id"], "u1");
        assert!(e["event"]["type"].is_string());
    }

    // The row holds the last value the log gave for each projected column.
    let last = |kind: &str| {
        events
            .iter()
            .rev()
            .find(|e| e["event"]["type"] == kind)
            .map(|e| e["event"].clone())
            .unwrap_or_else(|| panic!("the log has no {kind} event"))
    };
    assert_eq!(row.last_seq, events.len() as u64);
    assert_eq!(row.phase, last("phase_changed")["to"]);
    assert_eq!(row.cost, last("metric")["cost_usd"].as_f64().unwrap());
    assert_eq!(
        row.terminal_reason.as_deref(),
        last("done")["result"].as_str()
    );
    assert_eq!(
        row.oracle_hash.as_deref(),
        last("oracle_proposed")["hash"].as_str()
    );
    assert!(row.oracle_frozen);

    // And the values themselves, for a default (two review rounds) demo mission.
    assert_eq!(row.phase, "done");
    assert_eq!(row.terminal_reason.as_deref(), Some("done"));
    assert!((row.cost - 0.16).abs() < 1e-9, "cost was {}", row.cost);
    let hash = row.oracle_hash.unwrap();
    assert!(hash.starts_with('h') && hash.len() == 17, "hash was {hash}");
}

#[tokio::test]
async fn a_t1_demo_mission_walks_exactly_these_phases() {
    let d = Daemon::new();
    let id = d
        .mission(json!({"task": "x", "min_review_rounds": 1}))
        .await;
    d.wait_for(&id, &TERMINAL).await;
    let events = d.events(&id);

    assert_eq!(
        events[0],
        json!({
            "unit_id": "u1",
            "seq": 1,
            "event": {"type": "phase_changed", "from": "queued", "to": "provisioning"}
        })
    );
    let phases: Vec<&str> = events
        .iter()
        .filter(|e| e["event"]["type"] == "phase_changed")
        .map(|e| e["event"]["to"].as_str().unwrap())
        .collect();
    assert_eq!(
        phases,
        [
            "provisioning",
            "spec",
            "building",
            "checking",
            "reviewing",
            "merge_check",
            "pr_open",
            "done"
        ]
    );
    assert_eq!(
        events.last().unwrap()["event"],
        json!({"type": "done", "result": "done"})
    );
}

#[tokio::test]
async fn the_read_routes_return_these_shapes() {
    let d = Daemon::new();
    assert_eq!(d.get_json("/units").await, json!([]));

    let id = d
        .mission(json!({"task": "x", "min_review_rounds": 1}))
        .await;
    let row = d.wait_for(&id, &TERMINAL).await;

    let list = d.get_json("/units").await;
    assert_eq!(list.as_array().unwrap().len(), 1);
    assert_eq!(
        list[0],
        json!({
            "unit_id": "u1", "phase": "done", "cost": row.cost, "usd_cap": 5.0,
            "tier": "t1", "task": "x", "last_seq": row.last_seq
        })
    );

    let snapshot = d.get_json("/units/u1").await;
    assert_eq!(
        keys(&snapshot),
        ["cost", "events", "phase", "unit_id", "usd_cap"]
    );
    assert_eq!(snapshot["phase"], "done");
    assert_eq!(
        snapshot["events"].as_array().unwrap().len() as u64,
        row.last_seq
    );

    let tail = d
        .get_json(&format!("/units/u1/events?since={}", row.last_seq - 1))
        .await;
    assert_eq!(tail.as_array().unwrap().len(), 1);
    assert_eq!(tail[0]["event"]["type"], "done");
    assert_eq!(d.get_json("/units/u1/events").await, snapshot["events"]);

    let health = d.get_json("/health").await;
    assert_eq!(keys(&health), ["anthropic_key", "docker", "version"]);
    assert!(health["docker"].is_boolean() && health["anthropic_key"].is_boolean());
    assert_eq!(health["version"], env!("CARGO_PKG_VERSION"));
}

#[tokio::test]
async fn an_unknown_unit_is_404_for_a_snapshot_or_a_command_but_an_empty_list_of_events() {
    let d = Daemon::new();
    let (status, body) = d.send("GET", "/units/u404", None).await;
    assert_eq!((status, body.as_str()), (StatusCode::NOT_FOUND, ""));
    assert_eq!(d.get_json("/units/u404/events").await, json!([]));
    let halt = json!({"command": "halt", "cmd_id": "c1"});
    let (status, _) = d.send("POST", "/units/u404/commands", Some(halt)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_command_to_a_finished_unit_is_410_and_a_malformed_command_is_422() {
    let d = Daemon::new();
    let id = d
        .mission(json!({"task": "x", "min_review_rounds": 1}))
        .await;
    d.wait_for(&id, &TERMINAL).await;

    let uri = format!("/units/{id}/commands");
    let (status, _) = d
        .send(
            "POST",
            &uri,
            Some(json!({"command": "dance", "cmd_id": "c1"})),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    // The driver has returned, so nothing is listening for commands any more. The
    // send can only fail once the finished task has been dropped, so allow for that.
    let halt = json!({"command": "halt", "cmd_id": "c2"});
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let (status, _) = d.send("POST", &uri, Some(halt.clone())).await;
        if status == StatusCode::GONE {
            break;
        }
        assert_eq!(status, StatusCode::ACCEPTED);
        assert!(
            Instant::now() < deadline,
            "a finished unit kept accepting commands"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    let before = d.row(&id).unwrap().last_seq;
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(
        d.row(&id).unwrap().last_seq,
        before,
        "a refused command writes no event"
    );
}
```

- [ ] **Step 2: Watch it fail**

In `crates/fleetd/src/server.rs`, in `create_mission`, change

```rust
        branch: format!("agent/{unit_id}"),
```

to

```rust
        branch: format!("work/{unit_id}"),
```

```
cargo test -p fleetd --test characterisation_missions
```

Expected: `test result: FAILED. 7 passed; 2 failed`. The two are
`a_mission_with_only_a_task_is_stored_with_these_defaults` and
`the_request_sets_tier_and_review_floor_and_a_floor_of_zero_becomes_one`.

- [ ] **Step 3: Undo the change and watch it pass**

```
git checkout -- crates/fleetd/src
cargo test -p fleetd --test characterisation_missions
```

Expected: `test result: ok. 9 passed; 0 failed`.

- [ ] **Step 4: Run it twenty times**

*Git-Bash:*

```bash
pass=0; for i in $(seq 1 20); do cargo test -q -p fleetd --test characterisation_missions >/dev/null 2>&1 && pass=$((pass+1)); done; echo "pass=$pass/20"
```

Expected: `pass=20/20`. Anything less is a race in a test: find it and remove it before
committing. Do not raise a timeout to hide it.

- [ ] **Step 5: Commit**

```
git add crates/fleetd/tests/characterisation_missions.rs
git commit -m "test(fleetd): characterise what POST /missions stores, the projection and the read routes"
```

### Task 4: Admission, the concurrency limit, and the defect of issue #74

**Files:**
- Create: `crates/fleetd/tests/characterisation_admission.rs`

**Interfaces:**
- Consumes: the same as Task 3, and `rusqlite::Connection` (already a dependency of `fleetd`)
  for a second connection to a shared in-memory database.
- Produces: seven tests. Two carry the marker `CHARACTERISED-DEFECT: #74` in their doc
  comment.

About the two defect tests. `create_mission` discards the result of the insert that records a
mission's reservation, and treats a failed read of committed spend as zero. Neither failure
can be provoked through the store's own API, so each test opens the store on a named, shared
in-memory database (`file:...?mode=memory&cache=shared`) and uses a second connection to it:
one test installs a trigger that refuses inserts into `units`; the other drops the `swarms`
table that the spend query reads. No file is created. Each test asserts what happens today,
which is that the mission is admitted. When milestone 1 fixes #74, both tests fail, and the
fix replaces their assertions in the same commit.

- [ ] **Step 1: Write the tests**

```rust
//! Characterisation: how `POST /missions` admits or refuses a unit, as it behaves
//! today: the rolling 24-hour spend ceiling, the concurrency limit, and two defects
//! that are recorded here on purpose.
//!
//! Everything goes through `fleetd::server::router` in demo mode: no Docker, no
//! network, no token. These tests assert what the code does, which is not always
//! what it should do. Changing an assertion is a behaviour change and needs a
//! reviewer's decision.
//!
//! None of these tests depends on how far a demo driver has run when it asserts:
//! every seeded amount is chosen so the answer is the same whether a unit admitted
//! earlier in the test is still running or has finished. (Issue #72 was a test that
//! got this wrong.)
//!
//! Already pinned inside `src/` and not repeated here:
//! `server::tests::create_mission_refused_over_global_cap`,
//! `server::tests::create_mission_refused_when_committed_reservations_breach_cap`,
//! `server::tests::concurrent_missions_cannot_both_breach_the_cap`,
//! `store::tests::committed_spend_counts_reservations_and_overcap_and_planner`,
//! `store::tests::committed_spend_window_excludes_old`, and
//! `driver::tests::driver_waits_for_a_concurrency_slot`.

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use fleetd::server::{router, AppState};
use fleetd::store::{Store, UnitRow};
use serde_json::{json, Value};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tower::ServiceExt;

const HOUR_MS: i64 = 3_600_000;

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

/// A running unit that reserves `usd_cap` against the ceiling.
fn reservation(id: &str, usd_cap: f64) -> UnitRow {
    UnitRow {
        unit_id: id.into(),
        tier: "t1".into(),
        task: "seeded".into(),
        repo_url: "https://example.invalid/repo".into(),
        repo_slug: "owner/repo".into(),
        base_branch: "main".into(),
        branch: format!("agent/{id}"),
        test_cmd: "node --test".into(),
        usd_cap,
        wall_clock_secs: 1800,
        phase: "building".into(),
        cost: 0.0,
        last_seq: 0,
        oracle_frozen: true,
        oracle_hash: None,
        terminal_reason: None,
        mode: "demo".into(),
        min_review_rounds: 1,
        swarm_id: None,
    }
}

struct Daemon {
    store: Arc<Mutex<Store>>,
    state: AppState,
}

impl Daemon {
    fn over(store: Store) -> Self {
        for key in ["CC_GLOBAL_USD_CAP", "CC_MAX_CONCURRENT"] {
            assert!(
                std::env::var_os(key).is_none(),
                "unset {key}: these tests pin the default limits ($20 a day, 3 at once)"
            );
        }
        let store = Arc::new(Mutex::new(store));
        let state = AppState::new(store.clone());
        Daemon { store, state }
    }

    fn new() -> Self {
        Daemon::over(Store::open_memory().unwrap())
    }

    /// Seed a unit row whose first write happened at `created_ms`.
    fn seed(&self, row: &UnitRow, created_ms: i64) {
        self.store
            .lock()
            .unwrap()
            .upsert_unit(row, created_ms)
            .unwrap();
    }

    async fn send(&self, method: &str, uri: &str, body: Option<Value>) -> (StatusCode, String) {
        let request = Request::builder().method(method).uri(uri);
        let request = match body {
            Some(json) => request
                .header("content-type", "application/json")
                .body(Body::from(json.to_string())),
            None => request.body(Body::empty()),
        }
        .unwrap();
        let response = router(self.state.clone()).oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        (status, String::from_utf8(bytes.to_vec()).unwrap())
    }

    /// `POST /missions` for a one-round demo unit of the given tier.
    async fn post_mission(&self, tier: &str) -> (StatusCode, String) {
        let body = json!({"task": "x", "tier": tier, "min_review_rounds": 1});
        self.send("POST", "/missions", Some(body)).await
    }

    fn unit_ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = self
            .store
            .lock()
            .unwrap()
            .list_units()
            .unwrap()
            .into_iter()
            .map(|u| u.unit_id)
            .collect();
        ids.sort();
        ids
    }

    fn phase(&self, unit: &str) -> Option<String> {
        let row = self.store.lock().unwrap().get_unit(unit).unwrap();
        row.map(|r| r.phase)
    }

    fn events(&self, unit: &str) -> Vec<Value> {
        let raw = self.store.lock().unwrap().events_since(unit, 0).unwrap();
        raw.iter()
            .map(|j| serde_json::from_str(j).unwrap())
            .collect()
    }

    /// Wait until `done()` holds. Fails after ten seconds instead of hanging.
    async fn until(&self, what: &str, done: impl Fn(&Daemon) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while !done(self) {
            assert!(Instant::now() < deadline, "timed out waiting until {what}");
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }
}

const REFUSAL: &str = "global daily cost cap reached";

#[tokio::test]
async fn at_the_ceiling_a_mission_is_refused_with_429_and_nothing_is_stored() {
    let d = Daemon::new();
    // Exactly the default ceiling: the comparison is "at or above", not "above".
    d.seed(&reservation("seed", 20.0), now_ms());

    let (status, body) = d.post_mission("t1").await;

    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(body, REFUSAL);
    assert_eq!(d.unit_ids(), ["seed"], "a refused mission leaves no row");
    assert!(d.events("u1").is_empty(), "and starts no driver");
}

#[tokio::test]
async fn one_cent_under_the_ceiling_a_mission_is_admitted_and_the_next_is_refused() {
    let d = Daemon::new();
    d.seed(&reservation("seed", 19.99), now_ms());

    let (status, body) = d.post_mission("t1").await;
    assert_eq!(
        (status, body.as_str()),
        (StatusCode::OK, r#"{"unit_id":"u1"}"#)
    );

    // 19.99 + u1 is at least 20 whether u1 still reserves $5.00 or has finished and
    // counts the $0.09 it spent, so this does not depend on how far u1 has run.
    let (status, body) = d.post_mission("t1").await;
    assert_eq!(
        (status, body.as_str()),
        (StatusCode::TOO_MANY_REQUESTS, REFUSAL)
    );
    assert_eq!(d.unit_ids(), ["seed", "u1"]);
}

#[tokio::test]
async fn only_units_first_written_in_the_last_24_hours_count() {
    let d = Daemon::new();
    d.seed(&reservation("old", 999.0), now_ms() - 25 * HOUR_MS);
    let (status, _) = d.post_mission("t1").await;
    assert_eq!(
        status,
        StatusCode::OK,
        "a reservation 25 hours old is outside the window"
    );

    let d = Daemon::new();
    d.seed(&reservation("recent", 999.0), now_ms() - 23 * HOUR_MS);
    let (status, _) = d.post_mission("t1").await;
    assert_eq!(
        status,
        StatusCode::TOO_MANY_REQUESTS,
        "23 hours old is inside it"
    );
}

#[tokio::test]
async fn a_refused_mission_still_uses_up_a_unit_id() {
    let d = Daemon::new();
    d.seed(&reservation("seed", 20.0), now_ms());
    assert_eq!(d.post_mission("t1").await.0, StatusCode::TOO_MANY_REQUESTS);

    // Free the ceiling: the seeded unit finishes having spent nothing.
    d.store
        .lock()
        .unwrap()
        .update_unit("seed", "done", 0.0, 1, Some("done"), None, now_ms())
        .unwrap();

    let (status, body) = d.post_mission("t1").await;
    assert_eq!(
        (status, body.as_str()),
        (StatusCode::OK, r#"{"unit_id":"u2"}"#)
    );
}

/// TODAY a unit keeps its concurrency slot while it waits for a person to approve
/// its oracle. The factory design releases the slot while a gate is pending, so
/// milestone 1 changes this test deliberately: three parked units must then leave
/// room for a fourth.
#[tokio::test]
async fn a_fourth_unit_waits_for_a_slot_while_three_wait_at_the_oracle_gate() {
    let d = Daemon::new();
    // Three T2 units: each stops at the oracle gate and holds one of the 3 slots.
    for n in 1..=3 {
        let (status, body) = d.post_mission("t2").await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let unit = format!("u{n}");
        d.until("the T2 unit is at its oracle gate", |d| {
            d.phase(&unit).as_deref() == Some("awaiting_oracle_approval")
        })
        .await;
    }

    // The fourth is admitted (3 x $5 reserved is under $20) but cannot start.
    let (status, body) = d.post_mission("t1").await;
    assert_eq!(
        (status, body.as_str()),
        (StatusCode::OK, r#"{"unit_id":"u4"}"#)
    );
    d.until("u4 reports that it is waiting for a slot", |d| {
        d.events("u4").iter().any(|e| {
            e["event"]["type"] == "blocked" && e["event"]["reason"] == "awaiting concurrency slot"
        })
    })
    .await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(d.phase("u4").as_deref(), Some("provisioning"));
    let events = d.events("u4");
    assert_eq!(
        events.last().unwrap()["event"],
        json!({"type": "blocked", "reason": "awaiting concurrency slot", "detail": ""}),
        "u4 has done nothing since it began to wait"
    );
    assert_eq!(
        events.len(),
        2,
        "queued -> provisioning, then blocked: {events:?}"
    );

    // Approving one oracle lets that unit finish, which frees its slot for u4.
    let approve = json!({"command": "approve_oracle", "cmd_id": "a1"});
    let (status, _) = d.send("POST", "/units/u1/commands", Some(approve)).await;
    assert_eq!(status, StatusCode::ACCEPTED);
    d.until("u1 and u4 are done", |d| {
        d.phase("u1").as_deref() == Some("done") && d.phase("u4").as_deref() == Some("done")
    })
    .await;
    assert_eq!(d.phase("u2").as_deref(), Some("awaiting_oracle_approval"));
    assert_eq!(d.phase("u3").as_deref(), Some("awaiting_oracle_approval"));
}

/// An in-memory store, and a second connection to the same database. A test uses the
/// second connection to make one kind of statement fail, which the store's own API
/// gives no way to do.
fn store_with_a_side_door(name: &str) -> (Store, rusqlite::Connection) {
    let uri = format!("file:fleetd-characterisation-{name}?mode=memory&cache=shared");
    let store = Store::open(Path::new(&uri)).unwrap();
    let side = rusqlite::Connection::open(&uri).unwrap();
    (store, side)
}

/// CHARACTERISED-DEFECT: #74. `create_mission` discards the result of the insert
/// that records a mission's reservation. TODAY, when that insert fails, the mission
/// is still admitted and its driver runs with no row behind it: the reply is 200,
/// `GET /units/<id>` is 404, the unit's events are logged all the same, and because
/// no reservation was recorded the ceiling never fills.
///
/// When #74 is fixed this test must fail. Replace its assertions with the corrected
/// behaviour (a refusal, no driver, no events) in the commit that fixes it. Do not
/// delete it, and do not mark it ignored, to get a green run.
#[tokio::test]
async fn defect_74_a_mission_is_admitted_when_its_reservation_cannot_be_recorded() {
    let (store, side) = store_with_a_side_door("defect-74-insert");
    let d = Daemon::over(store);
    side.execute_batch(
        "CREATE TRIGGER refuse_unit_rows BEFORE INSERT ON units
             BEGIN SELECT RAISE(ABORT, 'characterisation: unit insert refused'); END;",
    )
    .unwrap();

    // Six missions reserve $30 between them, half as much again as the $20 ceiling.
    for n in 1..=6 {
        let (status, body) = d.post_mission("t1").await;
        assert_eq!(status, StatusCode::OK, "mission {n} was refused: {body}");
        assert_eq!(body, format!(r#"{{"unit_id":"u{n}"}}"#));
    }

    assert!(
        d.unit_ids().is_empty(),
        "no reservation row was ever written"
    );
    assert_eq!(d.store.lock().unwrap().committed_spend(0).unwrap(), 0.0);
    let (status, _) = d.send("GET", "/units/u1", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // The drivers ran regardless, and logged events for units that have no row.
    d.until("the unrecorded unit's driver has finished", |d| {
        d.events("u1").last().map(|e| e["event"]["type"] == "done") == Some(true)
    })
    .await;
    assert_eq!(d.events("u1").last().unwrap()["event"]["result"], "done");
}

/// CHARACTERISED-DEFECT: #74. The same handler treats a failed read of committed
/// spend as zero spend. TODAY, when that read fails, a mission is admitted however
/// much is already committed.
///
/// When #74 is fixed this test must fail. Replace its assertions with the corrected
/// behaviour in the commit that fixes it. Do not delete it or mark it ignored.
#[tokio::test]
async fn defect_74_a_failed_spend_read_is_treated_as_nothing_spent() {
    let (store, side) = store_with_a_side_door("defect-74-read");
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
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, r#"{"unit_id":"u2"}"#);
    assert_eq!(d.unit_ids(), ["seed", "u2"]);
}
```

- [ ] **Step 2: Watch it fail**

In `crates/fleetd/src/server.rs`, in `create_mission` (the first of two places this line
appears), change

```rust
        if s.committed_spend(since).unwrap_or(0.0) >= st.global_cap {
```

to

```rust
        if s.committed_spend(since).unwrap_or(0.0) > st.global_cap {
```

```
cargo test -p fleetd --test characterisation_admission
```

Expected: `test result: FAILED. 5 passed; 2 failed`. The two are
`at_the_ceiling_a_mission_is_refused_with_429_and_nothing_is_stored` and
`a_refused_mission_still_uses_up_a_unit_id`.

- [ ] **Step 3: Undo the change and watch it pass**

```
git checkout -- crates/fleetd/src
cargo test -p fleetd --test characterisation_admission
```

Expected: `test result: ok. 7 passed; 0 failed`.

- [ ] **Step 4: Watch a defect test fail when the defect is fixed**

This shows that the #74 tests bind. In `create_mission`, change

```rust
        s.upsert_unit(&row, now_ms()).ok();
```

to

```rust
        if s.upsert_unit(&row, now_ms()).is_err() {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "could not record the reservation".into(),
            ));
        }
```

```
cargo test -p fleetd --test characterisation_admission
```

Expected: `test result: FAILED. 6 passed; 1 failed`. The one is
`defect_74_a_mission_is_admitted_when_its_reservation_cannot_be_recorded`, with
`mission 1 was refused: could not record the reservation`.

Undo it. This lane does not fix #74.

```
git checkout -- crates/fleetd/src
git status --short
```

Expected: one line, `?? crates/fleetd/tests/characterisation_admission.rs`.

- [ ] **Step 5: Run it twenty times**

*Git-Bash:*

```bash
pass=0; for i in $(seq 1 20); do cargo test -q -p fleetd --test characterisation_admission >/dev/null 2>&1 && pass=$((pass+1)); done; echo "pass=$pass/20"
```

Expected: `pass=20/20`.

- [ ] **Step 6: Commit**

```
git add crates/fleetd/tests/characterisation_admission.rs
git commit -m "test(fleetd): characterise admission and the concurrency limit; record the #74 defect"
```

### Task 5: Startup reconcile

**Files:**
- Create: `crates/fleetd/tests/characterisation_reconcile.rs`

**Interfaces:**
- Consumes: `fleetd::server::{reconcile_on_startup, router, AppState}`;
  `fleetd::fake::FakeRunner` with `with_unit_containers` and its `teardowns` and `discards`
  counters; `fleetd::store::{Store, UnitRow}`.
- Produces: five tests.

- [ ] **Step 1: Write the tests**

```rust
//! Characterisation: what startup reconciliation does to stored units, as it
//! behaves today.
//!
//! `reconcile_on_startup` runs before the daemon serves, against a scripted runner
//! that reports which units still have a container: no Docker, no network, no token.
//! These tests assert what the code does, which is not always what it should do.
//! Changing an assertion is a behaviour change and needs a reviewer's decision.
//!
//! Already pinned inside `src/` and not repeated here: the pure decision table
//! (`reconcile::tests::*`), `server::tests::reconcile_halts_stranded_unit_with_coherent_event`
//! (phase, `last_seq` and reason of one stranded unit),
//! `server::tests::reconcile_tick_reaps_strays_but_spares_live_units`,
//! `server::tests::reconcile_tick_halts_a_stranded_unit_with_no_driver`,
//! `server::tests::reconcile_marks_planning_swarm_failed_and_resumes_fanning_out`, and
//! `server::tests::rehydrate_reloads_oracle_hash_so_tamper_gate_rearms`.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use fleetd::fake::FakeRunner;
use fleetd::server::{reconcile_on_startup, router, AppState};
use fleetd::store::{Store, UnitRow};
use serde_json::{json, Value};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tower::ServiceExt;

/// A demo unit as a daemon that died mid-run would have left it.
fn stored(id: &str, phase: &str) -> UnitRow {
    UnitRow {
        unit_id: id.into(),
        tier: "t1".into(),
        task: "left behind".into(),
        repo_url: "https://example.invalid/repo".into(),
        repo_slug: "owner/repo".into(),
        base_branch: "main".into(),
        branch: format!("agent/{id}"),
        test_cmd: "node --test".into(),
        usd_cap: 5.0,
        wall_clock_secs: 1800,
        phase: phase.into(),
        cost: 0.25,
        last_seq: 7,
        oracle_frozen: true,
        oracle_hash: Some("h-before-restart".into()),
        terminal_reason: None,
        mode: "demo".into(),
        min_review_rounds: 1,
        swarm_id: None,
    }
}

fn store_with(rows: &[UnitRow]) -> Arc<Mutex<Store>> {
    let store = Store::open_memory().unwrap();
    for row in rows {
        store.upsert_unit(row, 1).unwrap();
    }
    Arc::new(Mutex::new(store))
}

fn row(store: &Arc<Mutex<Store>>, id: &str) -> UnitRow {
    store.lock().unwrap().get_unit(id).unwrap().unwrap()
}

fn events(store: &Arc<Mutex<Store>>, id: &str) -> Vec<Value> {
    let raw = store.lock().unwrap().events_since(id, 0).unwrap();
    raw.iter()
        .map(|j| serde_json::from_str(j).unwrap())
        .collect()
}

#[tokio::test]
async fn a_stranded_unit_gets_one_synthetic_event_and_keeps_its_cost_and_oracle_hash() {
    let store = store_with(&[stored("u1", "building")]);
    let state = AppState::new(store.clone());

    reconcile_on_startup(&state, &FakeRunner::new(vec![])).await;

    assert_eq!(
        events(&store, "u1"),
        [json!({
            "unit_id": "u1",
            "seq": 8,
            "event": {
                "type": "phase_changed",
                "from": "building",
                "to": "halted",
                "reason": "daemon restarted"
            }
        })]
    );
    let got = row(&store, "u1");
    assert_eq!(got.phase, "halted");
    assert_eq!(got.last_seq, 8);
    assert_eq!(got.terminal_reason.as_deref(), Some("daemon restarted"));
    assert_eq!(got.cost, 0.25, "what the unit had spent is kept");
    assert_eq!(got.oracle_hash.as_deref(), Some("h-before-restart"));
    assert!(got.oracle_frozen);
}

#[tokio::test]
async fn finished_units_are_left_exactly_as_they_were() {
    let mut rows = Vec::new();
    for (id, phase) in [("u1", "done"), ("u2", "no_change"), ("u3", "failed")] {
        let mut r = stored(id, phase);
        r.terminal_reason = Some(phase.into());
        rows.push(r);
    }
    let store = store_with(&rows);
    let state = AppState::new(store.clone());
    let runner = FakeRunner::new(vec![]);

    reconcile_on_startup(&state, &runner).await;

    for (id, phase) in [("u1", "done"), ("u2", "no_change"), ("u3", "failed")] {
        let got = row(&store, id);
        assert_eq!(got.phase, phase);
        assert_eq!(got.last_seq, 7);
        assert_eq!(got.terminal_reason.as_deref(), Some(phase));
        assert!(events(&store, id).is_empty());
    }
    assert_eq!(runner.teardowns.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn containers_are_reaped_for_stranded_units_finished_units_and_unknown_ids() {
    let mut finished = stored("u2", "done");
    finished.terminal_reason = Some("done".into());
    let store = store_with(&[stored("u1", "checking"), finished, stored("u3", "queued")]);
    let state = AppState::new(store.clone());
    // u1: stranded, container still up. u2: finished, container left behind.
    // u9: a container with no unit row at all. u3: stranded with no container.
    let runner =
        FakeRunner::new(vec![]).with_unit_containers(vec!["u1".into(), "u2".into(), "u9".into()]);

    reconcile_on_startup(&state, &runner).await;

    assert_eq!(
        runner.teardowns.load(Ordering::Relaxed),
        3,
        "u1, u2 and u9 are reaped"
    );
    assert_eq!(
        runner.discards.load(Ordering::Relaxed),
        0,
        "reaping keeps the volume"
    );
    assert_eq!(row(&store, "u1").phase, "halted");
    assert_eq!(row(&store, "u3").phase, "halted");
    assert_eq!(row(&store, "u2").phase, "done");
    assert!(store.lock().unwrap().get_unit("u9").unwrap().is_none());
    assert_eq!(events(&store, "u3")[0]["event"]["from"], "queued");
}

/// TODAY a unit that was waiting for a person is not left waiting across a restart:
/// `needs_human` and `awaiting_oracle_approval` are both rewritten to `halted`, with
/// the restart as the reason, and a unit that is already `halted` gains one more
/// event at every start. The original reason for the stop survives only in the log.
#[tokio::test]
async fn units_waiting_on_a_person_are_rewritten_to_halted_at_every_start() {
    let mut parked = stored("u1", "needs_human");
    parked.terminal_reason = Some("cap breach".into());
    let store = store_with(&[parked, stored("u2", "awaiting_oracle_approval")]);

    reconcile_on_startup(&AppState::new(store.clone()), &FakeRunner::new(vec![])).await;

    let u1 = row(&store, "u1");
    assert_eq!(u1.phase, "halted");
    assert_eq!(u1.terminal_reason.as_deref(), Some("daemon restarted"));
    assert_eq!(events(&store, "u1")[0]["event"]["from"], "needs_human");
    assert_eq!(row(&store, "u2").phase, "halted");
    assert_eq!(
        events(&store, "u2")[0]["event"]["from"],
        "awaiting_oracle_approval"
    );

    // A second start: the unit is already halted, and is halted again.
    reconcile_on_startup(&AppState::new(store.clone()), &FakeRunner::new(vec![])).await;

    let again = events(&store, "u1");
    assert_eq!(again.len(), 2);
    assert_eq!(
        again[1]["event"],
        json!({
            "type": "phase_changed", "from": "halted", "to": "halted",
            "reason": "daemon restarted"
        })
    );
    assert_eq!(row(&store, "u1").last_seq, 9);
}

#[tokio::test]
async fn a_halted_demo_unit_is_resumed_by_a_command_and_runs_to_done() {
    // No stored hash: with one, the resumed run compares it to the oracle it reads
    // back, which `rehydrate_reloads_oracle_hash_so_tamper_gate_rearms` pins.
    let mut left = stored("u1", "building");
    left.oracle_hash = None;
    let store = store_with(&[left]);
    let state = AppState::new(store.clone());
    reconcile_on_startup(&state, &FakeRunner::new(vec![])).await;
    assert_eq!(row(&store, "u1").phase, "halted");

    // No driver exists for u1 after a restart; the command route brings one back.
    let request = Request::builder()
        .method("POST")
        .uri("/units/u1/commands")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({"command": "resume", "cmd_id": "r1"}).to_string(),
        ))
        .unwrap();
    let response = router(state.clone()).oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);

    let deadline = Instant::now() + Duration::from_secs(10);
    while row(&store, "u1").terminal_reason.as_deref() != Some("done") {
        assert!(
            Instant::now() < deadline,
            "u1 never finished: {:?}",
            row(&store, "u1")
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }

    let got = row(&store, "u1");
    assert_eq!(got.phase, "done");
    let log = events(&store, "u1");
    // The log continues from the synthetic halt (seq 8); nothing is renumbered.
    assert_eq!(log[0]["seq"], 8);
    assert_eq!(
        log[1],
        json!({
            "unit_id": "u1",
            "seq": 9,
            "event": {
                "type": "phase_changed", "from": "halted", "to": "provisioning",
                "cmd_id": "r1"
            }
        })
    );
    assert_eq!(got.last_seq, 7 + log.len() as u64);
    // The oracle was frozen before the restart, so the resumed run does not redo it,
    // and the cost carries on from what was already spent: 0.25 + 0.03 + 0.04.
    assert!(!log.iter().any(|e| e["event"]["type"] == "oracle_proposed"));
    assert!((got.cost - 0.32).abs() < 1e-9, "cost was {}", got.cost);
    assert_eq!(got.oracle_hash, None);
}
```

- [ ] **Step 2: Watch it fail**

In `crates/fleetd/src/server.rs`, in `halt_in_store`, change

```rust
        let seq = row.last_seq + 1;
```

to

```rust
        let seq = row.last_seq + 2;
```

```
cargo test -p fleetd --test characterisation_reconcile
```

Expected: `test result: FAILED. 2 passed; 3 failed`. The three are
`a_stranded_unit_gets_one_synthetic_event_and_keeps_its_cost_and_oracle_hash`,
`units_waiting_on_a_person_are_rewritten_to_halted_at_every_start` and
`a_halted_demo_unit_is_resumed_by_a_command_and_runs_to_done`.

- [ ] **Step 3: Undo the change and watch it pass**

```
git checkout -- crates/fleetd/src
cargo test -p fleetd --test characterisation_reconcile
```

Expected: `test result: ok. 5 passed; 0 failed`.

- [ ] **Step 4: Run it twenty times**

*Git-Bash:*

```bash
pass=0; for i in $(seq 1 20); do cargo test -q -p fleetd --test characterisation_reconcile >/dev/null 2>&1 && pass=$((pass+1)); done; echo "pass=$pass/20"
```

Expected: `pass=20/20`.

- [ ] **Step 5: Commit**

```
git add crates/fleetd/tests/characterisation_reconcile.rs
git commit -m "test(fleetd): characterise startup reconcile and resume after a restart"
```

### Task 6: The forge seam

**Files:**
- Create: `crates/fleetd/tests/characterisation_forge.rs`

**Interfaces:**
- Consumes: `fleetd::forge::{Forge, ForgeError, MergeResult, Mergeability}`;
  `fleetd::fake::{FakeForge, FakeRunner}`; `fleetd::driver::{run, EventEnvelope, RunCtx}`;
  `fleetd::runner::{ExecOutput, UnitSpec}`; `async_trait` (already a dependency of `fleetd`).
- Produces: eight tests, and a `RecordingForge` that implements the public `Forge` trait
  inside the test file.

- [ ] **Step 1: Write the tests**

```rust
//! Characterisation: the `Forge` seam as the lifecycle driver uses it today: which
//! methods it calls, in what order, with what arguments, and what each answer does
//! to the unit.
//!
//! The driver runs against the scripted runner and either `FakeForge` or a recording
//! forge defined here: no git, no GitHub, no Docker, no network. These tests assert
//! what the code does, which is not always what it should do. Changing an assertion
//! is a behaviour change and needs a reviewer's decision.
//!
//! Already pinned elsewhere and not repeated here: `demo_mode_it` (a clean merge
//! ends in a PR artifact and `done`), `gh_forge::tests::mergeable_mapping` and
//! `gh_forge::tests::branch_guard_blocks_injection` (the real forge's parsing), and
//! the transition table in `fleet-core`.

use async_trait::async_trait;
use fleet_core::{Command, Event, GateConfig, Phase, Tier};
use fleetd::driver::{run, EventEnvelope, RunCtx};
use fleetd::fake::{FakeForge, FakeRunner};
use fleetd::forge::{Forge, ForgeError, MergeResult, Mergeability};
use fleetd::runner::{ExecOutput, UnitSpec};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;

fn spec() -> UnitSpec {
    UnitSpec {
        unit_id: "u1".into(),
        tier: Tier::T1,
        task: "add a sum() helper".into(),
        usd_cap: 5.0,
        wall_clock_secs: 1800,
        gate: GateConfig {
            min_review_rounds: 1,
        },
        repo_url: "https://example.invalid/repo".into(),
        repo_slug: "owner/repo".into(),
        base_branch: "main".into(),
        branch: "agent/u1".into(),
        test_cmd: "node --test".into(),
        oracle_frozen: false,
    }
}

/// Oracle, build, check, then a review that reports no blockers: the unit reaches
/// the merge check on its first round.
fn script() -> Vec<ExecOutput> {
    vec![
        FakeRunner::ok(0.02, &["sum.test.js"]),
        FakeRunner::ok(0.03, &["implementing the change"]),
        FakeRunner::ok(0.0, &["tests: 1 passing"]),
        FakeRunner::ok(0.04, &["review done\nBLOCKERS=0"]),
    ]
}

/// What the driver asked the forge, in order.
type Calls = Arc<Mutex<Vec<String>>>;

/// A forge that records every call and answers from fixed values.
struct RecordingForge {
    calls: Calls,
    merge: Result<MergeResult, String>,
    pr: Result<String, String>,
    mergeable: Result<Mergeability, String>,
}

impl RecordingForge {
    fn new(calls: &Calls) -> Self {
        RecordingForge {
            calls: calls.clone(),
            merge: Ok(MergeResult::Clean),
            pr: Ok("https://example.invalid/pr/7".into()),
            mergeable: Ok(Mergeability::Mergeable),
        }
    }
}

#[async_trait]
impl Forge for RecordingForge {
    async fn trial_merge(&self, bundle: &Path, branch: &str) -> Result<MergeResult, ForgeError> {
        let bundle = bundle.to_string_lossy().replace('\\', "/");
        self.calls
            .lock()
            .unwrap()
            .push(format!("trial_merge({bundle}, {branch})"));
        self.merge.clone().map_err(ForgeError::Failed)
    }

    async fn open_pr(&self, branch: &str) -> Result<String, ForgeError> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("open_pr({branch})"));
        self.pr.clone().map_err(ForgeError::Failed)
    }

    async fn poll_mergeable(&self, pr_url: &str) -> Result<Mergeability, ForgeError> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("poll_mergeable({pr_url})"));
        self.mergeable.clone().map_err(ForgeError::Failed)
    }
}

/// Run one unit against `forge`. If it stops for a person, abandon it so the driver
/// returns. Gives back the final phase and every event.
async fn drive<F: Forge + 'static>(forge: F) -> (Phase, Vec<Event>) {
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel::<Command>();
    let (evt_tx, mut evt_rx) = mpsc::unbounded_channel::<EventEnvelope>();
    let driver = tokio::spawn(run(
        FakeRunner::new(script()),
        forge,
        spec(),
        RunCtx::standalone(),
        cmd_rx,
        evt_tx,
    ));

    let mut events = Vec::new();
    loop {
        let next = tokio::time::timeout(Duration::from_secs(10), evt_rx.recv())
            .await
            .expect("the driver stopped emitting events without finishing");
        let Some(envelope) = next else { break };
        if matches!(
            envelope.event,
            Event::PhaseChanged {
                to: Phase::NeedsHuman,
                ..
            }
        ) {
            cmd_tx
                .send(Command::Abandon {
                    cmd_id: "end-of-test".into(),
                })
                .expect("the driver is waiting for a command");
        }
        events.push(envelope.event);
    }
    (driver.await.unwrap(), events)
}

/// The `to` phase and reason of every phase change, as `"phase"` or `"phase: reason"`.
fn phases(events: &[Event]) -> Vec<String> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::PhaseChanged { to, reason, .. } => {
                let to = serde_json::to_value(to).unwrap();
                let to = to.as_str().unwrap().to_string();
                Some(match reason {
                    Some(r) => format!("{to}: {r}"),
                    None => to,
                })
            }
            _ => None,
        })
        .collect()
}

/// The tail of `phases` from the merge check onward.
fn from_merge_check(events: &[Event]) -> Vec<String> {
    let all = phases(events);
    let start = all
        .iter()
        .position(|p| p == "merge_check")
        .expect("the unit reached the merge check");
    all[start..].to_vec()
}

fn artifacts(events: &[Event]) -> Vec<String> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::Artifact { kind, reference } => {
                let kind = serde_json::to_value(kind).unwrap();
                Some(format!("{}={reference}", kind.as_str().unwrap()))
            }
            _ => None,
        })
        .collect()
}

fn errors(events: &[Event]) -> Vec<String> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::Error {
                scope,
                retryable,
                detail,
            } => {
                let scope = serde_json::to_value(scope).unwrap();
                Some(format!(
                    "{} retryable={retryable}: {detail}",
                    scope.as_str().unwrap()
                ))
            }
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn a_clean_run_calls_the_forge_three_times_in_this_order_with_these_arguments() {
    let calls = Calls::default();
    let (phase, events) = drive(RecordingForge::new(&calls)).await;

    assert_eq!(phase, Phase::Done);
    assert_eq!(
        *calls.lock().unwrap(),
        [
            // The bundle path is whatever the runner's `export_bundle` returned.
            "trial_merge(/fake/out.bundle, agent/u1)",
            "open_pr(agent/u1)",
            // The poll is given back the URL that `open_pr` returned.
            "poll_mergeable(https://example.invalid/pr/7)",
        ]
    );
    assert_eq!(
        from_merge_check(&events),
        ["merge_check", "pr_open", "done"]
    );
    assert_eq!(
        artifacts(&events),
        ["branch=agent/u1", "pr=https://example.invalid/pr/7"]
    );
    assert!(errors(&events).is_empty());
}

#[tokio::test]
async fn the_fake_forge_answers_clean_mergeable_and_one_fixed_pr_url() {
    let fake = FakeForge::default();
    assert_eq!(fake.merge, MergeResult::Clean);
    assert_eq!(fake.mergeable, Mergeability::Mergeable);
    assert_eq!(
        fake.trial_merge(Path::new("anything"), "any-branch")
            .await
            .unwrap(),
        MergeResult::Clean
    );
    assert_eq!(
        fake.open_pr("any-branch").await.unwrap(),
        "https://fake/pr/1"
    );
    assert_eq!(
        fake.poll_mergeable("any-url").await.unwrap(),
        Mergeability::Mergeable
    );
}

#[tokio::test]
async fn a_merge_conflict_stops_for_a_person_and_no_pr_is_opened() {
    let calls = Calls::default();
    let forge = RecordingForge {
        merge: Ok(MergeResult::Conflict),
        ..RecordingForge::new(&calls)
    };
    let (phase, events) = drive(forge).await;

    assert_eq!(
        calls.lock().unwrap().len(),
        1,
        "only the trial merge was called"
    );
    assert_eq!(
        from_merge_check(&events),
        ["merge_check", "needs_human: base conflict", "failed"]
    );
    assert_eq!(artifacts(&events), ["branch=agent/u1"]);
    assert_eq!(phase, Phase::Failed, "abandoning a stopped unit fails it");
}

#[tokio::test]
async fn a_pr_that_is_not_mergeable_stops_for_a_person_after_the_pr_is_open() {
    let calls = Calls::default();
    let forge = RecordingForge {
        mergeable: Ok(Mergeability::Dirty),
        ..RecordingForge::new(&calls)
    };
    let (_, events) = drive(forge).await;

    assert_eq!(calls.lock().unwrap().len(), 3);
    assert_eq!(
        from_merge_check(&events),
        [
            "merge_check",
            "pr_open",
            "needs_human: base moved",
            "failed"
        ]
    );
    assert_eq!(
        artifacts(&events),
        ["branch=agent/u1", "pr=https://example.invalid/pr/7"]
    );
}

#[tokio::test]
async fn a_pr_that_stays_pending_is_polled_ten_times_then_stops_for_a_person() {
    let calls = Calls::default();
    let forge = RecordingForge {
        mergeable: Ok(Mergeability::Pending),
        ..RecordingForge::new(&calls)
    };
    let (_, events) = drive(forge).await;

    let polls = calls
        .lock()
        .unwrap()
        .iter()
        .filter(|c| c.starts_with("poll_mergeable"))
        .count();
    assert_eq!(polls, 10);
    assert_eq!(
        from_merge_check(&events),
        [
            "merge_check",
            "pr_open",
            "needs_human: mergeable poll timeout",
            "failed"
        ]
    );
    assert!(events.iter().any(|e| matches!(
        e,
        Event::Blocked { reason, cap: None, detail }
            if reason == "mergeability poll timed out" && detail == "after 10 polls"
    )));
}

#[tokio::test]
async fn a_failing_trial_merge_fails_the_unit_outright() {
    let calls = Calls::default();
    let forge = RecordingForge {
        merge: Err("remote hung up".into()),
        ..RecordingForge::new(&calls)
    };
    let (phase, events) = drive(forge).await;

    assert_eq!(phase, Phase::Failed);
    assert_eq!(
        from_merge_check(&events),
        ["merge_check", "failed: merge failed"]
    );
    assert_eq!(
        errors(&events),
        ["github retryable=true: forge failure: remote hung up"]
    );
    assert_eq!(calls.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn a_failing_open_pr_fails_the_unit_outright() {
    let calls = Calls::default();
    let forge = RecordingForge {
        pr: Err("403".into()),
        ..RecordingForge::new(&calls)
    };
    let (phase, events) = drive(forge).await;

    assert_eq!(phase, Phase::Failed);
    assert_eq!(
        from_merge_check(&events),
        ["merge_check", "pr_open", "failed: open pr failed"]
    );
    assert_eq!(
        errors(&events),
        ["github retryable=true: forge failure: 403"]
    );
    assert_eq!(artifacts(&events), ["branch=agent/u1"]);
}

#[tokio::test]
async fn a_failing_poll_is_retried_ten_times_then_stops_for_a_person() {
    let calls = Calls::default();
    let forge = RecordingForge {
        mergeable: Err("502".into()),
        ..RecordingForge::new(&calls)
    };
    let (_, events) = drive(forge).await;

    assert_eq!(errors(&events).len(), 10, "one error event per failed poll");
    assert_eq!(
        from_merge_check(&events),
        [
            "merge_check",
            "pr_open",
            "needs_human: mergeable poll timeout",
            "failed"
        ]
    );
}
```

- [ ] **Step 2: Watch it fail**

In `crates/fleetd/src/driver.rs`, change

```rust
const MAX_MERGEABLE_POLLS: u32 = 10;
```

to

```rust
const MAX_MERGEABLE_POLLS: u32 = 3;
```

```
cargo test -p fleetd --test characterisation_forge
```

Expected: `test result: FAILED. 6 passed; 2 failed`. The two are
`a_pr_that_stays_pending_is_polled_ten_times_then_stops_for_a_person` and
`a_failing_poll_is_retried_ten_times_then_stops_for_a_person`.

- [ ] **Step 3: Undo the change and watch it pass**

```
git checkout -- crates/fleetd/src
cargo test -p fleetd --test characterisation_forge
```

Expected: `test result: ok. 8 passed; 0 failed`.

- [ ] **Step 4: Commit**

```
git add crates/fleetd/tests/characterisation_forge.rs
git commit -m "test(fleetd): characterise how the driver uses the forge seam"
```

### Task 7: Verify, push and open the pull request

**Files:** none.

**Interfaces:**
- Consumes: Tasks 2 to 6.
- Produces: the pull request `test/m0-characterisation` into `factory/m0`.

- [ ] **Step 1: Run every command in this lane's Verify table**

Then run all five files together twenty times. *Git-Bash:*

```bash
pass=0; for i in $(seq 1 20); do cargo test -q -p fleetd --test characterisation_store --test characterisation_missions --test characterisation_admission --test characterisation_reconcile --test characterisation_forge >/dev/null 2>&1 && pass=$((pass+1)); done; echo "pass=$pass/20"
```

Expected: `pass=20/20`.

- [ ] **Step 2: Push**

```
git push -u origin test/m0-characterisation
```

- [ ] **Step 3: Open the pull request**

*Git-Bash:*

```bash
gh pr create -R adbarc92/command-center --base factory/m0 --head test/m0-characterisation \
  --title "Characterisation tests for today's fleetd, before any code moves" \
  --body "$(cat <<'EOF'
## What changed

Five new test files under `crates/fleetd/tests/`, forty tests. Nothing under
`crates/fleetd/src/` changed. They pin how `fleetd` behaves today, through its routes and its
store, on the scripted runner and the fake forge: no Docker, no network, no token.
Plan: `docs/superpowers/plans/2026-10-04-factory-m0-scaffold.md`, lane CC-CHAR.

## How it was verified

<paste the Verify table with the real output of each command, and the result of each 20-run loop>

Each file was also seen failing against a one-line change to the production code, which was
then reverted (Tasks 2 to 6).

## Behaviour these tests record that milestone 1 has to decide about

- A unit keeps its concurrency slot while it waits at the oracle gate. The factory design
  releases it; `a_fourth_unit_waits_for_a_slot_while_three_wait_at_the_oracle_gate` must
  change when that lands.
- Issue #74, twice: a mission is admitted when its reservation cannot be recorded, and a
  failed spend read counts as zero. Both tests carry `CHARACTERISED-DEFECT: #74`.
- At every start, units in `needs_human` and `awaiting_oracle_approval` are rewritten to
  `halted`, and an already-halted unit gains another event.
- A refused mission (400 for an unknown mode, 429 at the ceiling) still uses up a unit id.
- Appending an event with a sequence number already in the log is dropped without an error.
- `update_unit` replaces the stored reason even with `None`, though it keeps the oracle hash.
- A command to a finished unit answers 410; `GET /units/<id>/events` for an unknown unit
  answers 200 with an empty list, where the snapshot route answers 404.

This pull request records them. It fixes none.
EOF
)"
```

Replace the `<...>` line with the real content before running it.

- [ ] **Step 4: Watch the checks**

```
gh pr checks --watch
```

Expected: every check green. `cargo test (workspace)` and `integration` now run the forty
new tests on Linux, and they have only been run on Windows until this moment, so a failure
there is information: report the test name and the log excerpt.

Do not merge.

---

## Lane CC-72

**Owns:** the two commits already on `origin/fix/flaky-cap-race-test`
(`a3cfabb` and `686fb70`), which change only the `mod tests` block of
`crates/fleetd/src/server.rs`; and the new branch that carries them.

**Reads:** issue #72 (`gh issue view 72 -R adbarc92/command-center`); the two commits
(`git show a3cfabb 686fb70`).

**Worktree and branch:** `D:\MajorProjects\.swarm-wt\m0-cc-72`, on a new branch
`fix/m0-flaky-cap-race-test`, cut from `origin/factory/m0`. The two commits are cherry-picked
onto it. Rebasing the old branch in place would need a force-push, which is not allowed, so
the old branch is left as it is. Pull request: `fix/m0-flaky-cap-race-test` into
`factory/m0`, unless the owner has answered open question Q1 with `main`. In that case cut
the branch from `origin/main` instead, use `--base main` in Task 3, and expect `N` to be 166.

**Needs:** lane CC-COORD merged into `factory/m0`; owner action 3 (the fixed pull-request
hook is live).

**Blocks:** owner actions 7 and 8. Until this fix is on `main`, `cargo test (workspace)`,
`unit (fleetd, ubuntu-latest)` and `unit (fleetd, windows-latest)` can fail at random, and
issue #95 depends on #72.

**What the fix is.** `concurrent_missions_cannot_both_breach_the_cap` seeded $16 of committed
spend and relied on the first admitted unit still reserving $5, so that the second was
refused at $21. A demo unit can finish first, and a finished unit counts the 9 cents it
spent, not the $5 it reserved: $16.09 is under the $20 ceiling, and the second mission is
then correctly admitted. The test was wrong, not the ceiling. The fix seeds $19.95, which
refuses the second mission either way ($24.95 while the first runs, $20.04 once it has
finished), and adds `cap_still_binds_after_the_first_unit_goes_terminal`, which forces the
schedule that used to fail. No production code changes.

**Verify** (run in the worktree, after Task 2):

| Command | Expected |
|---|---|
| `git log --oneline origin/factory/m0..HEAD` | two commits, subjects ending `(#72)` |
| `git diff --stat origin/factory/m0...HEAD` | `crates/fleetd/src/server.rs \| 67 +++...`; `1 file changed, 62 insertions(+), 5 deletions(-)` |
| `cargo test -p fleetd --lib cap_still_binds_after_the_first_unit_goes_terminal` | `1 passed` |
| *Git-Bash:* the 50-run loop of Task 2 Step 3 | `pass=50/50` |
| *Git-Bash:* `cargo test --workspace --locked 2>&1 \| grep -E "^test result" \| awk '{p+=$4; f+=$6; i+=$8} END {print "passed="p" failed="f" ignored="i}'` | `passed=<N+1> failed=0 ignored=3`, where `N` is the count recorded in Task 1 (209 when lane CC-COORD is the only lane merged; every other merged lane raises it) |
| `cargo xtask test static --root-only` | last line `xtask: static: 6 step(s) passed` |
| `git status --short` | empty |

### Task 1: Create the worktree and carry the fix onto `factory/m0`

**Files:**
- Modify (by cherry-pick only): `crates/fleetd/src/server.rs`, inside `mod tests`

**Interfaces:**
- Consumes: commits `a3cfabb` and `686fb70`.
- Produces: the branch `fix/m0-flaky-cap-race-test` with those two commits on top of
  `origin/factory/m0`.

- [ ] **Step 1: Create the worktree and record the lane's baseline**

```
git -C D:/MajorProjects/INFRASTRUCTURE/command-center fetch origin
git -C D:/MajorProjects/INFRASTRUCTURE/command-center worktree add --no-track -b fix/m0-flaky-cap-race-test D:/MajorProjects/.swarm-wt/m0-cc-72 origin/factory/m0
```

Every later command in this lane runs in `D:\MajorProjects\.swarm-wt\m0-cc-72`.

*Git-Bash:*

```bash
cargo test --workspace --locked 2>&1 | grep -E "^test result" | awk '{p+=$4; f+=$6; i+=$8} END {print "passed="p" failed="f" ignored="i}'
```

Expected: `passed=209 failed=0 ignored=3` when lane CC-COORD is the only lane merged. Every
other lane that has merged raises the count (lane CC-CHAR by 40, lane CC-PROTO by 100, and so
on). Write the number down as `N`. This run can itself hit the flake: if the only failure is
`server::tests::concurrent_missions_cannot_both_breach_the_cap`, run once more and record
both results. That failure is the thing this lane removes.

- [ ] **Step 2: Confirm the two commits are what the issue describes**

```
git log --oneline origin/main..origin/fix/flaky-cap-race-test
git diff --stat origin/main...origin/fix/flaky-cap-race-test
```

Expected:

```
686fb70 test(fleetd): pin the terminal-first schedule that made the cap race test flaky (#72)
a3cfabb test(fleetd): stop the cap race test racing the driver it spawns (#72)
 crates/fleetd/src/server.rs | 67 +++++++++++++++++++++++++++++++++++++++++----
 1 file changed, 62 insertions(+), 5 deletions(-)
```

If the branch holds anything else, stop and report: someone has added to it since this plan
was written.

- [ ] **Step 3: Cherry-pick them, oldest first**

```
git cherry-pick a3cfabb 686fb70
```

Expected: two commits applied with no conflict. Lane CC-COORD does not touch `server.rs`, so
a conflict means something unexpected is on `factory/m0`: run `git cherry-pick --abort` and
report.

### Task 2: Show that the fix holds and that its new test binds

**Files:** none (one temporary edit, reverted).

**Interfaces:**
- Consumes: Task 1.
- Produces: the evidence for the pull request.

- [ ] **Step 1: Watch the new test fail when the old seed is put back**

In `crates/fleetd/src/server.rs` there are now two lines reading

```rust
            r.usd_cap = 19.95;
```

one in `concurrent_missions_cannot_both_breach_the_cap` and one in
`cap_still_binds_after_the_first_unit_goes_terminal`. Change both to

```rust
            r.usd_cap = 16.0;
```

```
cargo test -p fleetd --lib cap_still_binds_after_the_first_unit_goes_terminal
```

Expected: `test result: FAILED. 0 passed; 1 failed`, with
`expected 429 — cap must bind once the first unit is terminal`. This is issue #72's failure,
made to happen every time instead of occasionally.

- [ ] **Step 2: Undo the change and watch both tests pass**

```
git checkout -- crates/fleetd/src
cargo test -p fleetd --lib cap
```

Expected: `test result: ok. 11 passed; 0 failed`, including
`concurrent_missions_cannot_both_breach_the_cap` and
`cap_still_binds_after_the_first_unit_goes_terminal`.

- [ ] **Step 3: Run the two tests fifty times**

*Git-Bash:*

```bash
pass=0; for i in $(seq 1 50); do cargo test -q -p fleetd --lib -- concurrent_missions_cannot_both_breach_the_cap cap_still_binds_after_the_first_unit_goes_terminal >/dev/null 2>&1 && pass=$((pass+1)); done; echo "pass=$pass/50"
```

Expected: `pass=50/50`.

- [ ] **Step 4: Run the lane's Verify table**

Record the real output of each command.

### Task 3: Push and open the pull request

**Files:** none.

**Interfaces:**
- Consumes: Tasks 1 and 2.
- Produces: the pull request `fix/m0-flaky-cap-race-test` into `factory/m0`.

- [ ] **Step 1: Push**

```
git push -u origin fix/m0-flaky-cap-race-test
```

- [ ] **Step 2: Open the pull request**

*Git-Bash:*

```bash
gh pr create -R adbarc92/command-center --base factory/m0 --head fix/m0-flaky-cap-race-test \
  --title "test(fleetd): stop the cap race test racing the driver it spawns (#72)" \
  --body "$(cat <<'EOF'
## What changed

Test code only, inside `mod tests` of `crates/fleetd/src/server.rs`. No production change.

- `concurrent_missions_cannot_both_breach_the_cap` seeded $16 and relied on the first admitted
  unit still reserving $5. A demo unit can finish first, and a finished unit counts the 9
  cents it spent, so the second mission was then correctly admitted and the test failed. It
  now seeds $19.95, which refuses the second mission whichever way the race goes.
- New: `cap_still_binds_after_the_first_unit_goes_terminal` forces the schedule that used to
  fail.

These are the two commits of `fix/flaky-cap-race-test`, cherry-picked onto `factory/m0`.
Plan: `docs/superpowers/plans/2026-10-04-factory-m0-scaffold.md`, lane CC-72.

## How it was verified

<paste the Verify table with the real output, the forced failure of Task 2 Step 1, and the 50-run loop>

## Issue

Fixes #72. A pull request into `factory/m0` does not close the issue by itself: it closes when
`factory/m0` reaches `main`, or by hand.
EOF
)"
```

Replace the `<...>` line with the real content before running it.

If the hook denies the command, copy its message into your report word for word and stop.
Owner action 3 has not happened yet.

- [ ] **Step 3: Watch the checks**

```
gh pr checks --watch
```

Expected: every check green, including `unit (fleetd, ubuntu-latest)` and
`unit (fleetd, windows-latest)`. Do not merge.

---

## Lane CC-FORMS

**Owns:** eleven Form documents. Seven for the crates milestone 1 builds:
`forms/fleet-store.md`, `forms/fleet-forge.md`, `forms/fleet-runner.md`,
`forms/fleet-supervisor.md`, `forms/fleet-verify.md`, `forms/fleet-admission.md`,
`forms/fleet-api.md`. Four for the contract crates: `forms/fleet-core.md`,
`forms/harness-protocol.md`, `forms/factory-spec.md`, `forms/factory-presets.md`. Not
`forms/registry.md` and not `forms/README.md`: those are the coordinator's.

**Reads:** `forms/README.md` (the format); `forms/registry.md` (the gates that exist);
`xtask/src/deps.rs` (each crate's dependency rule); the milestone-1 control-plane plan,
`docs/superpowers/plans/2026-10-04-factory-m1-control-plane.md`, and in it the interface
block of each of the seven crates. For Task 5: the source of the four contract crates as
merged into `factory/m0`, and the two sibling plans that built them.

**Worktree and branch:** two parts, two pull requests, both into `factory/m0`.
Tasks 1 to 4: `D:\MajorProjects\.swarm-wt\m0-cc-forms`, on `docs/m0-forms`.
Task 5: `D:\MajorProjects\.swarm-wt\m0-cc-forms-contracts`, on `docs/m0-contract-forms`.
Each is cut from `origin/factory/m0` when its own inputs are ready.

**Needs:** lane CC-COORD merged into `factory/m0`, for both parts. Then:

- Tasks 1 to 4 need the milestone-1 control-plane plan present in the worktree. That plan is
  written by someone else. If it is absent, these tasks are blocked: stop and report. Do not
  invent an interface.
- Task 5 needs four lanes of the sibling plans merged into `factory/m0`: **CC-PROTO** and
  **CC-CORE** (the contracts plan), **CC-SPEC** and **CC-PRESETS** (the spec-and-presets
  plan). It writes each Form from the code those lanes merged.

The two parts do not depend on each other. Do whichever has its inputs first.

**Blocks:** owner action 5 (approval of the Forms). Through the seven, every milestone-1
control-plane lane, each of which builds against one of them. Through the four, the owner's
approval of the contract crates' public types.

**Verify**, first half (Tasks 1 to 4; run in `m0-cc-forms`, after Task 3):

| Command | Expected |
|---|---|
| `node scripts/parity.mjs` | `parity: ok (4 gates, 7 forms)` |
| `cargo xtask test static --root-only` | last line `xtask: static: 6 step(s) passed` |
| `git diff --name-only origin/factory/m0...HEAD` | exactly the seven files Tasks 2 and 3 create |
| *Git-Bash:* `grep -c "^- status: draft" forms/fleet-*.md` | seven lines, each ending `:1` |
| *Git-Bash:* `grep -L "workspace.G1" forms/fleet-*.md` | no output: every Form cites the dependency gate |

These numbers are for the case where this part comes first. If Task 5's pull request is
already in `factory/m0`, the parity line says `11 forms`, and the two `grep` commands also
see `forms/fleet-core.md`, so they print eight lines. In either half, the gate count is 4
unless the coordinating lane has added registry rows since; a larger number is not an error.

**Verify**, second half (Task 5; run in `m0-cc-forms-contracts`, after its Step 8):

| Command | Expected |
|---|---|
| `node scripts/parity.mjs` | `parity: ok (4 gates, 4 forms)`, or `11 forms` if the first part has merged |
| `cargo xtask test static --root-only` | last line `xtask: static: 6 step(s) passed` |
| `cargo xtask test contract` | seven `PASS` lines from the conformance kit (lane CC-PROTO is merged), then `xtask: contract: 4 step(s) passed` |
| `git diff --name-only origin/factory/m0...HEAD` | exactly `forms/factory-presets.md`, `forms/factory-spec.md`, `forms/fleet-core.md`, `forms/harness-protocol.md` |
| *Git-Bash:* `grep -c "^- status: draft" forms/fleet-core.md forms/harness-protocol.md forms/factory-spec.md forms/factory-presets.md` | four lines, each ending `:1` |
| *Git-Bash:* the loop of Task 5 Step 6, once per crate | no output |

### What this lane can and cannot decide

A Form is the owner's document. This lane drafts eleven of them for the owner's approval,
and is careful about which parts are transcription and which are judgement. For the seven
whose crates are not built yet:

| Section | Source | Latitude |
|---|---|---|
| Interface | The crate's interface block in the milestone-1 plan | None. Copy it exactly. Do not add, rename, reorder or drop anything. |
| Invariants | Statements in the milestone-1 plan about what must always or never hold, and the behaviours its tests name | Rewording into one falsifiable sentence each. Nothing new. |
| Hidden decisions | What the milestone-1 plan's tasks choose that the interface does not show | Judgement. Flag each one you inferred in the pull-request body. |
| Gates | `forms/registry.md` as it is today | None. Only gates that already run. |
| Unenforced | Every invariant no gate protects yet | None. One dated line each. |
| Purpose, Regeneration | The milestone-1 plan's goal for the crate; this repository's commands | Your own plain words. |

For the four contract crates the table is the same, with one difference: the Interface comes
from the merged code, and rustdoc's list of public items decides what it must contain
(Task 5).

Every Form is written with `status: draft`. Only the owner makes one `frozen`.

A Form lists a gate only if the registry already has a row for it. When this lane finds an
invariant that a test already protects but no row names, or when a later lane builds a gate
that a Form lists as planned, the rule in `forms/README.md` applies: the lane says so in its
pull request, and the coordinating lane edits the Form and the registry together.

### The Form format

The format is defined in `forms/README.md`, which lane CC-COORD created (its full text is in
that lane's Task 10). In short: a heading `# Form: <crate>`; five header fields (`status`,
`owner`, `level`, `last-drill`, `interface-files`); and seven sections in this order:
Purpose, Interface, Invariants, Hidden decisions, Gates, Unenforced, Regeneration. The parity
check reads the heading, the header, the invariant numbers, the gates table and the
`Unenforced` lines, and fails when one of them is missing or malformed. It does not check
the order of the sections; keep the order anyway, so every Form reads the same way.

### One worked example

This is a complete, valid Form. It is **an illustration of the format, not the Form to
commit**: its Interface section was written from today's `fleetd::store`, before the
milestone-1 plan existed. When you write the real `forms/fleet-store.md`, the milestone-1
interface block replaces that section and the invariants are re-derived from it.

````markdown
# Form: fleet-store

- status: draft
- owner: adbarc92
- level: E1
- last-drill: never
- interface-files:
  - crates/fleet-store/src/lib.rs

## Purpose

Durable state for the control plane: one append-only event log per unit, and one row per
unit that summarises its log. Without it a restart loses every running unit, and admission
has no record of what has been spent.

## Interface

```rust
pub struct Store { /* private */ }

pub struct UnitRow { /* the fields of today's `fleetd::store::UnitRow` */ }

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("store failure: {0}")]
    Failed(String),
}

impl Store {
    pub fn open(path: &std::path::Path) -> Result<Store, StoreError>;
    pub fn open_memory() -> Result<Store, StoreError>;

    pub fn upsert_unit(&self, row: &UnitRow, now_ms: i64) -> Result<(), StoreError>;
    pub fn get_unit(&self, unit_id: &str) -> Result<Option<UnitRow>, StoreError>;
    pub fn list_units(&self) -> Result<Vec<UnitRow>, StoreError>;

    pub fn append_event(&self, unit_id: &str, seq: u64, ts_ms: i64, json: &str) -> Result<(), StoreError>;
    pub fn events_since(&self, unit_id: &str, seq: u64) -> Result<Vec<String>, StoreError>;

    pub fn committed_spend(&self, since_ms: i64) -> Result<f64, StoreError>;
}
```

## Invariants

I1. `events_since(unit, n)` returns exactly that unit's events with a sequence number above `n`, in ascending order, each byte for byte as it was appended.
I2. Appending an event whose unit and sequence number are already in the log changes nothing.
I3. The first write of a unit fixes its tier, task, repository, branch, test command, cap, mode and review floor; a later write changes only its phase, cost, last sequence number, oracle state and reason.
I4. `committed_spend(since)` counts each unit first written at or after `since`: its cost if it has finished, otherwise the larger of its cap and its cost.
I5. Everything written before the process exits is readable after the same file is opened again.
I6. The crate depends on no workspace crate other than `fleet-core`, and declares a `testkit` feature.

## Hidden decisions

- The storage engine, its journal mode and its file layout. No type from the engine appears in the interface.
- Table and column names, and how an older file is migrated.
- Whether one connection or several are used, and how writes are serialised.

## Gates

| id | guards | mechanism | location | blocks |
|----|--------|-----------|----------|--------|
| workspace.G1 | I6 | Build graph: an allowlist of dependency edges, which also requires the `testkit` feature | `xtask/src/deps.rs#pub const RULES` | merge |

## Unenforced

- I1 (2026-10-04): no gate in this crate yet. `crates/fleetd/tests/characterisation_store.rs` pins it for today's code; the store lane adds a contract test here when the code moves.
- I2 (2026-10-04): as I1.
- I3 (2026-10-04): as I1.
- I4 (2026-10-04): as I1.
- I5 (2026-10-04): as I1.

## Regeneration

- Run `cargo test -p fleet-store` for the crate alone; `cargo xtask test contract` runs its interface tests with every other crate's.
- Builders for `UnitRow` live in the `testkit` module, behind the `testkit` feature.
- The tests need no Docker, no network and no token. A test that needs a file uses the system temporary directory and removes what it creates.
- Allowed workspace dependency: `fleet-core` only. The list is the `fleet-store` row of `xtask/src/deps.rs`.
````

What to notice in it:

- `interface-files` lists only `crates/fleet-store/src/lib.rs`, because that is the only
  source file the skeleton has. The parity check refuses a path that does not exist. If the
  milestone-1 plan puts the interface in further files, name them in the pull-request body
  under "interface files to add when they exist".
- `level` is `E1`: one gate blocks, and five invariants are still unenforced.
- The Gates table has one row, and its `location` is copied character for character from
  the registry row `workspace.G1`. The contract tests that milestone 1 will write are **not**
  in the table, because they do not exist yet. They are named under `Unenforced`.
- Every invariant is a sentence a test could prove false. I6 is the dependency rule, taken
  from the crate's row in `xtask/src/deps.rs`.
- Nothing in Hidden decisions appears in the Interface. (Today's `store.rs` returns
  `rusqlite::Result`, which would expose the storage engine. A Form review is where that kind
  of leak is caught.)

### Task 1: Create the worktree and check the inputs

**Files:** none.

**Interfaces:**
- Consumes: `origin/factory/m0` with lane CC-COORD merged; the milestone-1 control-plane plan.
- Produces: the worktree, and a list of where each crate's interface block is.

- [ ] **Step 1: Create the worktree**

```
git -C D:/MajorProjects/INFRASTRUCTURE/command-center fetch origin
git -C D:/MajorProjects/INFRASTRUCTURE/command-center worktree add --no-track -b docs/m0-forms D:/MajorProjects/.swarm-wt/m0-cc-forms origin/factory/m0
```

Every later command in this lane runs in `D:\MajorProjects\.swarm-wt\m0-cc-forms`.

- [ ] **Step 2: Confirm the scaffold and the parity check**

```
node scripts/parity.mjs
```

Expected: `parity: ok (4 gates, 0 forms)`. If the script is missing, lane CC-COORD has not
merged: stop and report.

- [ ] **Step 3: Confirm the milestone-1 plan is present, and find the seven blocks**

*Git-Bash:*

```bash
p=docs/superpowers/plans/2026-10-04-factory-m1-control-plane.md
test -f "$p" && for c in fleet-store fleet-forge fleet-runner fleet-supervisor fleet-verify fleet-admission fleet-api; do echo "== $c"; grep -n "crates/$c\b\|\`$c\`" "$p" | head -5; done
```

Expected: the file exists, and each of the seven names is found. If the file does not exist,
this lane is blocked: stop and report. If a crate has no interface block (a fenced `rust`
block of public signatures under an **Interfaces** or **Produces** heading in its lane's
section), do not write that crate's Form: list it in your report as missing.

Write down, for each crate, the line range of its interface block. That list goes in the
pull-request body.

### Task 2: Write the first Form, `forms/fleet-store.md`

**Files:**
- Create: `forms/fleet-store.md`

**Interfaces:**
- Consumes: the `fleet-store` interface block of the milestone-1 plan; the worked example;
  the `fleet-store` row of `RULES` in `xtask/src/deps.rs`.
- Produces: `forms/fleet-store.md`, `status: draft`, passing the parity check.

- [ ] **Step 1: Start from the worked example**

Create `forms/fleet-store.md` with the worked example's text. Keep the heading and the five
header fields as they are.

- [ ] **Step 2: Replace the Interface section**

Delete everything between `## Interface` and `## Invariants`, and paste the milestone-1
plan's `fleet-store` interface block there, inside one fenced `rust` block. Copy it exactly.
If the plan spreads the interface over several blocks, paste them in the plan's order,
separated by a blank line.

- [ ] **Step 3: Re-derive the invariants**

For each of these three sources, in this order, write one numbered line:

1. Every sentence in the `fleet-store` section of the milestone-1 plan that says what must
   always, or must never, be true of the crate.
2. Every test the plan has that lane write first: its name states a behaviour. Where that
   behaviour must hold for any implementation of the interface, it is an invariant. Where it
   is a detail of one implementation, it is not.
3. Last, always, the dependency rule: "The crate depends on no workspace crate other than
   `<the names in its row of RULES>`, and declares a `testkit` feature."

For each line, ask which test would fail if the sentence were false. If you cannot name one,
it is not an invariant: leave it out, and list it in the pull-request body under "statements
I could not turn into an invariant". Number the lines `I1.`, `I2.`, … with no gaps. Keep the
worked example's invariants only where the new interface still supports them.

- [ ] **Step 4: Rewrite Hidden decisions, Gates and Unenforced to match**

- Hidden decisions: at least one. Each is something the Interface does not show and a caller
  might be tempted to rely on. If you cannot find one, say so in the pull-request body: a
  crate with nothing to hide may not need a Form.
- Gates: exactly one row, for the dependency invariant, citing `workspace.G1`. Copy the
  `location` cell from `forms/registry.md`. Put the number of the dependency invariant in
  `guards`.
- Unenforced: one line for every other invariant, written
  `- I<n> (<today, as YYYY-MM-DD>): no gate yet. Planned: <the test file the milestone-1 plan names for it>, built by the store lane in milestone 1.`
- Purpose and Regeneration: rewrite in your own words for the new interface.

- [ ] **Step 5: Run the parity check**

```
node scripts/parity.mjs
```

Expected: `parity: ok (4 gates, 1 forms)`. If it prints violations, each names the line to
fix. The two most likely are an invariant with neither a gate nor an `Unenforced` line, and
a `location` that differs from the registry's by a character.

- [ ] **Step 6: Commit**

```
git add forms/fleet-store.md
git commit -m "docs(forms): draft the fleet-store Form from the milestone-1 interface"
```

### Task 3: Write the other six Forms

**Files:**
- Create: `forms/fleet-forge.md`, `forms/fleet-runner.md`, `forms/fleet-supervisor.md`,
  `forms/fleet-verify.md`, `forms/fleet-admission.md`, `forms/fleet-api.md`

**Interfaces:**
- Consumes: each crate's interface block in the milestone-1 plan; each crate's row of `RULES`.
- Produces: six more Forms, each `status: draft`, each passing the parity check.

This plan cannot give their content: it is the milestone-1 plan's interface blocks, which did
not exist when this plan was written. The procedure is the same for each.

- [ ] **Step 1: For each crate, repeat Task 2 Steps 1 to 5**

Use the crate's own name in the heading, in `interface-files` (`crates/<crate>/src/lib.rs`)
and in the dependency invariant. Take the allowed workspace crates from the crate's row in
`xtask/src/deps.rs`:

| Crate | Row in `RULES` today | The dependency invariant says |
|---|---|---|
| `fleet-forge` | `rule("fleet-forge", &[])` | depends on no other workspace crate |
| `fleet-runner` | `rule("fleet-runner", &["fleet-core"])` | depends on no workspace crate other than `fleet-core`; and it is the only crate that may depend on a Docker client crate |
| `fleet-supervisor` | `rule("fleet-supervisor", &["fleet-core", "harness-protocol"])` | depends on no workspace crate other than those two |
| `fleet-verify` | `rule("fleet-verify", &["factory-presets", "factory-spec", "fleet-core", "harness-protocol"])` | depends on no workspace crate other than those four |
| `fleet-admission` | `rule("fleet-admission", &["factory-spec", "fleet-core"])` | depends on no workspace crate other than those two |
| `fleet-api` | `rule("fleet-api", &["fleet-core", "harness-protocol"])` | depends on no workspace crate other than those two |

Read the row from the file, not from this table: lane CC-COORD's Task 11 may have widened a
row to satisfy a dependency request, and the Form must say what the file says.

If the milestone-1 plan's interface for a crate uses a type from a workspace crate that the
crate's row does not allow, do not edit the row. Write the Form as the plan has it, and put
the mismatch at the top of the pull-request body as a request to the coordinator.

- [ ] **Step 2: After each Form, run the parity check and commit it alone**

```
node scripts/parity.mjs
git add forms/<crate>.md
git commit -m "docs(forms): draft the <crate> Form from the milestone-1 interface"
```

Expected after the sixth: `parity: ok (4 gates, 7 forms)`.

- [ ] **Step 3: Compare each Interface section with its source, character for character**

For each crate, save the Form's Interface block and the plan's block to two scratch files
outside the repository and run `git diff --no-index <a> <b>`. Expected: no output for each of
the seven. Any difference is a transcription error: fix the Form, not the plan.

### Task 4: Verify, push and open the pull request

**Files:** none.

**Interfaces:**
- Consumes: Tasks 2 and 3.
- Produces: the pull request `docs/m0-forms` into `factory/m0`, and the list the owner
  approves from.

- [ ] **Step 1: Run every command in this lane's Verify table**

Record the real output of each.

- [ ] **Step 2: Push**

```
git push -u origin docs/m0-forms
```

- [ ] **Step 3: Open the pull request**

*Git-Bash:*

```bash
gh pr create -R adbarc92/command-center --base factory/m0 --head docs/m0-forms \
  --title "Draft Forms for the seven control-plane crates milestone 1 builds" \
  --body "$(cat <<'EOF'
## What changed

Seven new documents under `forms/`, one per crate milestone 1 builds: `fleet-store`,
`fleet-forge`, `fleet-runner`, `fleet-supervisor`, `fleet-verify`, `fleet-admission`,
`fleet-api`. Each is `status: draft`. No code, no registry row and no manifest changed.
Plan: `docs/superpowers/plans/2026-10-04-factory-m0-scaffold.md`, lane CC-FORMS.

## For the owner: what approval means

Each Interface section is copied from the milestone-1 control-plane plan without change
(checked by diff, Task 3 Step 3). Approving a Form is changing its `status: draft` to
`status: frozen`. After that, an additive change needs a reviewer's approval and a breaking
change needs yours.

| Form | Interface copied from (plan lines) | Invariants | Unenforced | Hidden decisions I inferred |
|---|---|---|---|---|
<one row per Form>

## Statements I could not turn into an invariant

<list, or "none">

## Interface files to add when they exist

<list, or "none">

## Requests to the coordinator

<dependency-rule mismatches and registry rows wanted, or "none">

## How it was verified

<paste the Verify table with the real output of each command>
EOF
)"
```

Replace every `<...>` line with the real content before running it.

- [ ] **Step 4: Watch the checks**

```
gh pr checks --watch
```

Expected: every check green; `static` is the one that reads these files. Do not merge.

### Task 5: Forms for the four contract crates, from their merged code

**Files:**
- Create: `forms/fleet-core.md`, `forms/harness-protocol.md`, `forms/factory-spec.md`,
  `forms/factory-presets.md`

**Interfaces:**
- Consumes: the code of the four crates as merged into `origin/factory/m0`; rustdoc's list
  of each crate's public items; the names of each crate's tests; the Global Constraints of
  the contracts plan and the "Decisions this plan builds on" of the spec-and-presets plan;
  the registry rows `workspace.G1`, `workspace.G2` and `workspace.G3`; each crate's row of
  `RULES`.
- Produces: four more draft Forms that pass the parity check, and, in the pull request, the
  registry rows this lane asks the coordinating lane to add.

**This task waits on four lanes of the sibling plans:** CC-PROTO (`harness-protocol` and
`harness-conformance`) and CC-CORE (`fleet-core`), from the contracts plan; CC-SPEC
(`factory-spec`) and CC-PRESETS (`factory-presets`), from the spec-and-presets plan. All
four must be merged into `factory/m0`. It does not wait on the milestone-1 plan, so it can
run before or after Tasks 1 to 4.

Tasks 2 and 3 copy an interface from a plan, because the code does not exist yet. Here the
code exists, so the interface is read from the code, and a tool, not a reader, says what the
public items are.

- [ ] **Step 1: Create a worktree for this part**

```
git -C D:/MajorProjects/INFRASTRUCTURE/command-center fetch origin
git -C D:/MajorProjects/INFRASTRUCTURE/command-center worktree add --no-track -b docs/m0-contract-forms D:/MajorProjects/.swarm-wt/m0-cc-forms-contracts origin/factory/m0
```

Every later command in this task runs in `D:\MajorProjects\.swarm-wt\m0-cc-forms-contracts`.

- [ ] **Step 2: Confirm that all four lanes have merged**

*Git-Bash:*

```bash
grep -n 'pub const PROTOCOL_VERSION: &str = "0.2";' crates/harness-protocol/src/lib.rs || echo "CC-PROTO has not merged"
git ls-files --error-unmatch crates/fleet-core/src/auxiliary.rs || echo "CC-CORE has not merged"
git ls-files --error-unmatch crates/factory-spec/src/split.rs || echo "CC-SPEC has not merged"
grep -n 'pub const PRESETS_VERSION' crates/factory-presets/src/lib.rs || echo "CC-PRESETS has not merged"
```

Expected: four lines, each a match or a path, and no "has not merged" line. Then:

```
cargo xtask test contract
```

Expected: **seven** `PASS` lines from the conformance kit, then
`xtask: contract: 4 step(s) passed`. Seven is the count once lane CC-PROTO is in; six would
mean it is not.

If any of the five checks fails, this task is blocked: stop and report which lane is missing.

- [ ] **Step 3: For each crate, list its public items with rustdoc**

Do Steps 3 to 8 for one crate, then the next, in this order: `fleet-core`,
`harness-protocol`, `factory-spec`, `factory-presets`. In the commands, `<crate>` is the
crate's name and `<crate_>` is the same name with each `-` written as `_`. *Git-Bash:*

```bash
cargo doc --locked --no-deps -p <crate>
grep -o '<a href="[^"]*\.html">[^<]*</a>' target/doc/<crate_>/all.html | sed 's/<[^>]*>//g' | sort -u > ../<crate>-items.txt
wc -l ../<crate>-items.txt
```

`all.html` is rustdoc's index of every public item in the crate. The file written beside
the worktree, outside the repository, is the list the Form's Interface must cover. Expected:
a count above zero. (On `main` at `a3fd1c8` the command gives 15 names for `fleet-core` and
52 for `harness-protocol`; the merged lanes add to both.) If the count is zero, the `grep`
did not match: stop and report rather than write a Form from an empty list.

- [ ] **Step 4: Work out the crate's interface files**

The interface files are `src/lib.rs` and every module that `lib.rs` makes public or
re-exports from. *Git-Bash:*

```bash
grep -oE "^pub (mod|use) [a-z_]+" crates/<crate>/src/lib.rs | awk '{print $3}' | sort -u
```

For each name printed, the interface file is `crates/<crate>/src/<name>.rs`; if that file
does not exist, it is every `.rs` file under `crates/<crate>/src/<name>/`. If a name printed
is `crate` or `self`, open `lib.rs`, read that line, and take the module named after it. A
module that `lib.rs` declares with a plain `mod` and never re-exports from is implementation,
and is left out.

For `harness-protocol`, add one more interface file:
`crates/harness-protocol/contract/harness-protocol.contract.json`. It is the wire contract in
schema form.

- [ ] **Step 5: Write the Form**

Create `forms/<crate>.md` in the layout of `forms/README.md`, with `status: draft`,
`owner: adbarc92`, `level: E1`, `last-drill: never`, and the interface files of Step 4.

*Interface.* One fenced `rust` block. For every name in `../<crate>-items.txt`, copy the
item's declaration from the source file that defines it:

- a function: its signature, ending in `;` where the body was;
- a constant: its whole line, value included;
- a struct or an enum: its definition with every public field or variant, and any
  `#[serde(...)]` attribute, because that changes the wire form. Leave out doc comments and
  `#[derive(...)]` lines;
- a trait: its definition with each method's signature.

Group the items under a `// src/<file>.rs` comment per source file. Add nothing that is not
in the list, and leave nothing in the list out.

For `harness-protocol` only, the wire types may be given by kind and name, one per line (for
example `pub struct WorkOrder;`), under this sentence: "The fields of these types are fixed
by `contract/harness-protocol.contract.json`, an interface file of this Form, which gate
`workspace.G2` pins." Functions, constants, traits and the types that never cross the wire
are still copied in full.

*Invariants.* One numbered, falsifiable sentence each, from these sources in this order:

1. The settled decisions that concern the crate. For `harness-protocol` and `fleet-core`:
   the Global Constraints of `2026-10-04-factory-m0-contracts.md`. For `factory-spec` and
   `factory-presets`: "Decisions this plan builds on" and "Two contracts that cross the lanes"
   in `2026-10-04-factory-m0-spec-and-presets.md`. Restate each in your own words.
2. The names of the crate's tests, which those lanes write as sentences:
   `cargo test --locked -p <crate> -- --list`. Where several tests state one behaviour that
   every implementation must have, that behaviour is one invariant.
3. Last, always, the dependency rule, from the crate's row of `RULES` in
   `xtask/src/deps.rs`. For the three pure crates it reads: "The crate does no I/O: its
   direct dependencies are only `<the names in its row>`, and it declares a `testkit`
   feature."

Beside each invariant, in your notes, write the test that would fail if it were false, as
`<file>#<test name>`. An invariant with no such test is left out of the Form and listed in
the pull-request body. Aim for between five and twelve invariants per Form.

*Hidden decisions.* At least one: for example, which parser library a crate uses, how its
modules are laid out, or the wording of an error's message as opposed to its stable code.

*Gates.* Only rows that are in `forms/registry.md` today, each with its `location` copied
from the registry:

- `workspace.G1`, guarding the dependency invariant: every one of the four Forms.
- `workspace.G2`, in `forms/harness-protocol.md` only, guarding the invariant that the
  committed schema equals the generated one and that its hash is the pinned one.
- `workspace.G3`, in `forms/harness-protocol.md` only, and only if one of its invariants is
  something the conformance kit's run against `harness-fake` would show to be false.

*Unenforced.* Every other invariant gets a line, even though a test for it exists and runs.
A test is not a registered gate until the registry has a row for it:

```
- I<n> (<today, as YYYY-MM-DD>): tested by `<test name>` in `<file>`, in the `<unit or contract>` tier; not yet a registry row. Requested from the coordinating lane as `<crate>.G<k>`.
```

*Purpose* and *Regeneration*: in your own plain words, as in Task 2.

- [ ] **Step 6: Check the Interface against the list**

*Git-Bash:*

```bash
while IFS= read -r item; do name="${item##*::}"; grep -qw -- "$name" forms/<crate>.md || echo "MISSING: $item"; done < ../<crate>-items.txt
```

Expected: no output. Every public item rustdoc lists is named in the Form.

- [ ] **Step 7: Run the parity check**

```
node scripts/parity.mjs
```

Expected: `parity: ok (4 gates, <m> forms)`, where `<m>` grows by one with each Form.

- [ ] **Step 8: Commit the Form alone**

```
git add forms/<crate>.md
git commit -m "docs(forms): draft the <crate> Form from its merged code"
```

- [ ] **Step 9: Verify, push and open the pull request**

Run the second half of this lane's Verify table. Then:

```
git push -u origin docs/m0-contract-forms
```

*Git-Bash:*

```bash
gh pr create -R adbarc92/command-center --base factory/m0 --head docs/m0-contract-forms \
  --title "Draft Forms for the four contract crates, from their merged code" \
  --body "$(cat <<'EOF'
## What changed

Four new documents under `forms/`: `fleet-core`, `harness-protocol`, `factory-spec`,
`factory-presets`. Each is `status: draft`. No code, no registry row and no manifest changed.
Plan: `docs/superpowers/plans/2026-10-04-factory-m0-scaffold.md`, lane CC-FORMS, Task 5.

## For the owner: what approval means

Each Interface section was written from the code merged by lanes CC-PROTO, CC-CORE, CC-SPEC
and CC-PRESETS, and covers every public item rustdoc lists for the crate (Task 5 Step 6).
Approving a Form is changing its `status: draft` to `status: frozen`.

| Form | Public items | Interface files | Invariants | Unenforced |
|---|---|---|---|---|
<one row per Form>

## Requests to the coordinating lane: registry rows

Each invariant below has a test that runs today and no registry row. Per `forms/README.md`
("When a planned gate is built"), the coordinating lane adds the row, adds it to the Form's
Gates table, and removes the line from `Unenforced`.

| id | form | mechanism | location | command | guards |
|---|---|---|---|---|---|
<one row per requested gate; `command` is `cargo xtask test unit` or `cargo xtask test contract`>

## Statements I could not turn into an invariant

<list, or "none">

## How it was verified

<paste the second half of the Verify table with the real output of each command>
EOF
)"
```

Replace every `<...>` line with the real content before running it. Then:

```
gh pr checks --watch
```

Expected: every check green; `static` is the one that reads these files. Do not merge.

---

## Self-review

Everything this plan shows as code was written and run before the plan was written, in a
scratch clone of this repository at `a3fd1c8`, on the Windows 11 build host (cargo 1.93.1,
Node 22.17.1). Every whole-file code block in this plan was copied from that clone by a
script, not retyped; the short fragments (the stub `main.rs`, the one-line changes of the
"watch it fail" steps) were typed and then run. Nothing was committed or pushed to this
repository.

The plan was revised once after the two sibling plans appeared: their dependency requests
are now declared in Task 2, the `unit` tier runs each crate a second time with its `testkit`
feature, the `integration` CI job was added, lane CC-FORMS gained Task 5, and the rule for a
planned gate went into `forms/README.md`. Every changed piece was run again in the same
clone; the table below gives the results after the revision.

**What was run, and what it showed**

| Check | Result |
|---|---|
| Baseline, `cargo test --workspace` at `a3fd1c8` | 166 passed, 0 failed, 3 ignored |
| Lane CC-COORD's files applied, with every sibling request; `cargo test --workspace --locked` | 209 passed, 0 failed, 3 ignored |
| `cargo update --workspace` from the original lockfile, with the requested crates | 38 packages added (11 workspace members, 27 from the registry); 363 insertions and 1 deletion; no locked package changed version or checksum |
| `cargo xtask deps` on the real graph, with `serde-saphyr`, `quick-xml`, `toml` and `regex` in it | `dependency direction: ok`. With `quick-xml` taken off the allowlist it failed and named the crate, then was restored |
| The two manifests compared with the spec-and-presets plan's text (Task 11 Step 2) | both match; the contracts plan's three lines found by the `grep` of Step 3 |
| The `xtask` build-up replayed task by task (Tasks 2 to 8), again after the revision | 0, 3, 14, 23, 30 tests passing after each, and `cargo fmt --check` clean at each; compile errors at each red step; 32 passed and 1 failed before the CI jobs, 33 passed after |
| `cargo xtask test static --root-only` | 6 steps passed, including `cargo clippy --all-targets --all-features -D warnings` |
| `cargo xtask test unit` | 27 steps passed: 15 crates, and 12 second runs with `--features testkit` |
| `cargo xtask test contract` | 4 steps passed, with six `PASS` lines. This is the state before lane CC-PROTO; its plan says the kit prints seven after it |
| `cargo xtask test integration` / `e2e` / `live` | 1 step passed (`demo_mode_it` runs, three Docker targets ignored); and the two "no tests in this tier yet" lines |
| `node --test scripts/parity.test.mjs` | 26 passed; the two red states of Task 9 reproduced; the Task 10 failure message reproduced |
| The workflow file parsed as YAML | 13 jobs, with exactly the names `xtask/src/ci.rs` pins |
| The five characterisation files | 40 passed; all five together 30 times, 30 of 30 |
| One production-code mutation per characterisation file | each failed exactly the tests the task names, then reverted |
| A sketch of the #74 fix | failed `defect_74_a_mission_is_admitted_when_its_reservation_cannot_be_recorded`, then reverted |
| Lane CC-72's two commits cherry-picked onto the scaffold | no conflict; the forced failure of its Task 2 Step 1 reproduced; 50 of 50; all three code lanes together 250 passed, 0 failed, 3 ignored |
| The worked example Form against the real registry | `parity: ok (4 gates, 1 forms)` |
| The commands of CC-FORMS Task 5 (rustdoc item list, interface-file list, the completeness loop), on today's `fleet-core` and `harness-protocol` | 15 and 52 public items listed; the loop reported exactly the names left out of a deliberately incomplete file |

**Checklist**

- [x] Every decision in the brief is restated under Global Constraints and none is reopened.
- [x] Each lane opens with Owns, Reads, Worktree and branch, Needs, Blocks and Verify.
- [x] No two lanes own the same file. CC-72 is the only lane that touches `crates/fleetd/src/`, and only a test module.
- [x] Every code step shows the code; every configuration step shows the content and the command that checks it.
- [x] Each of the five Review Focus failure modes has a named test or check in a named task.
- [x] The job names in the workflow, in `xtask/src/ci.rs`, in the Task 8 table and in owner action 7 are the same set.
- [x] Every request in the two sibling plans' "Requests to the coordinator" sections is a row of the table in Task 2, and Task 11 checks each one mechanically.
- [x] The tier naming rule is stated once, in Global Constraints, and is one function in the code.
- [x] The total of 209 and the 33 `xtask` tests, which the sibling plans quote, did not change in the revision.
- [x] No sentence from the private design, doctrine or templates appears. The Form and registry formats are described in this repository's own words.
- [x] No commit message or pull-request body carries a co-author line or a generator footer.

**Gaps this plan could not close**

1. **The new CI jobs have never run on a hosted runner.** The workflow was parsed, not
   executed. The matrix built from `fromJSON`, the cache keys and every `windows-latest`
   job are first exercised by CC-COORD's pull request (its Task 12 Step 4 says what to do
   with a red one). The contract tier does pass on the Windows build host.
2. **The cockpit half of `static` was not run locally.** It needs `npm ci`, the sidecar and
   a Tauri compile. It runs the same two commands the existing `fmt + clippy` job runs.
3. **Node 22 locally, Node 20 in CI.** The script uses nothing newer than Node 20, but it was
   not run there.
4. **The milestone-1 plans did not exist when this was written.** So Task 11 can only give
   the procedure for their dependency requests, and Tasks 1 to 4 of lane CC-FORMS can only
   give the procedure and one illustrative Form.
5. **Task 5 of lane CC-FORMS could not be rehearsed end to end**, because the code it reads
   is not written yet. Its commands were run on today's code; its judgement steps (which
   behaviours are invariants) were not.
6. **The dependency check sees crates, not calls.** A pure crate can still call `std::fs` or
   `std::process` directly. And Docker is reached today by spawning the `docker` program,
   not through a crate, so "only `fleet-runner` may talk to Docker" is enforced only against
   Docker client crates. Closing either needs a source-level rule, which this milestone does
   not add.
7. **The second `unit` run is wider than what was asked.** The spec-and-presets plan asks for
   its two crates to be tested with `--features testkit`; the tier does it for every crate
   that has the feature. That doubles the compile work of each `unit (<crate>, <os>)` job for
   those crates. For a skeleton crate the cost is seconds.
8. **The lock numbers in Task 2 are a snapshot.** `quick-xml = "0.42"`, `toml = "1"`,
   `serde-saphyr = "1.3"` and `regex = "1"` resolve to whatever is newest on the day the lane
   runs, so the count of added packages can differ from 38. The check that matters, that no
   already-locked package changed, does not depend on the count.
9. **Thirty clean runs on one machine do not prove a test cannot flake** on a slower runner.
   The tests avoid the known cause (rule 1 under "How these tests are written"); the first CI
   runs are the real evidence.
10. **The pull-request hook was not exercised.** Whether it denies any of these pull
    requests is unknown until they are opened.
