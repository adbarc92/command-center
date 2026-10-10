# Factory M0: contracts — Implementation Plan

> **For agentic workers:** steps use checkbox (`- [ ]`) syntax for tracking. Each lane below is
> executed by one agent in its own worktree; the two lanes share no file and can run at the same
> time. Work through a lane's tasks in order, one step at a time, and tick each box as you finish it.

**Goal:** Take the harness protocol from 0.1 to 0.2 with a shared protocol monitor, a conformance kit rebuilt on it and a `harness-fake` the control plane can drive, and give `fleet-core` the unit-state-machine delta and the auxiliary-run state machine, so that every later lane builds against frozen contracts.

**Architecture:** `harness-protocol` stays a pure wire crate: 0.2 adds optional fields and new types, minor-aware version negotiation, the functions that hash frozen test files, a bounded line codec and `monitor`, a clock-free classifier of inbound lines. `harness-conformance` drives a harness process through that monitor and keeps its own deadlines; `harness-fake` becomes a scripted harness that follows the order the unit state machine needs. `fleet-core` stays pure: four new triggers on `transition()`, wider validity for two existing ones, a second, five-state machine for auxiliary runs, and on its event types the `Harness` error scope and the `Reverify` command.

**Tech Stack:** Rust 2021 (toolchain 1.93), `serde` 1, `serde_json` 1, `schemars` 1.2, `sha2` 0.10, std threads and `mpsc`. No async, no new crates.

**Spec:** The ReqDrive factory design v0.4, in the private nexus repository: https://github.com/adbarc92/nexus/blob/main/docs/specs/2026-10-04-reqdrive-factory-design.md

The protocol monitor, the kit's violation mapping, the `harness-fake` changes and the widened
`CapBreach` / `Stall` / `OracleTampering` rows come from the SP-2a supervisor spec in this
repository, `docs/superpowers/specs/2026-09-16-sp2a-harness-supervisor-design.md` (§2, §3.1, §3.2,
§6 items 1 to 3). Issues: #82, #83, #87.

## Global Constraints

- **Protocol version:** `PROTOCOL_VERSION = "0.2"`. A refused `initialize` carries JSON-RPC error code `-32001`.
- **Negotiation:** while the major is 0, the harness's minor must equal the minor of a version the control plane accepts; from 1.0 on an equal major is enough. One function decides it: `negotiate(accepted: &[&str], harness_speaks: &str) -> bool`.
- **Wire compatibility:** every field added to an existing type carries `#[serde(default)]`; where it is an `Option` it also carries `skip_serializing_if = "Option::is_none"`. A 0.1-shaped message must still parse.
- **Wire names:** enum values are `snake_case` strings; tiers are `t1` / `t2` / `t3`; method names are unchanged.
- **Names are fixed.** The types, fields and functions are the ones in the program plan's shared-interface section (5.1 and 5.2). Do not rename one. Report any you cannot build as written.
- **Line limit:** `MAX_LINE_BYTES = 4 * 1024 * 1024` (4194304 bytes before the newline), enforced on read and on write.
- **File hashes:** SHA-256 as 64 lowercase hex characters. A set of files has one digest: SHA-256 over one record per file (`<path>\0<sha256>\n`), the records in bytewise path order.
- **Contract hash:** the canonical SHA-256 of `crates/harness-protocol/contract/harness-protocol.contract.json` is pinned in `crates/harness-protocol/tests/contract.rs`. Tasks 1 to 4 change the schema; each re-blesses with `HARNESS_PROTOCOL_BLESS=1 cargo test -p harness-protocol --test contract` and re-pins in the same commit. The final hash goes in the pull request body. The contract registry lives in another repository and is updated by the owner, not by a lane.
- **Files a lane may not edit:** the root `Cargo.toml`, `Cargo.lock`, every crate's `Cargo.toml`, `.cargo/config.toml`, anything under `.github/`, `xtask/`, `forms/` or `scripts/`, `docs/STATUS.md`, `CLAUDE.md`, and any roadmap file. Dependencies are asked of the coordinator (next section).
- **`crates/fleetd` is not modified by either lane.** Its 98 passing tests must stay at 98.
- **`crates/harness-conformance/tests/detects_violations.rs` is not edited.** Its 16 tests are the proof that the rebuilt kit judges 0.1 behaviour as before.
- **No source file is named after a DOS device** (`aux`, `con`, `nul`, `prn`, `com1` …). On Windows, git cannot add `aux.rs`. The auxiliary-run module is therefore `auxiliary.rs`.
- **Gates every commit must pass:** `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo test --workspace`.
- **Baseline:** on `origin/main` at `a3fd1c8`, `cargo test --workspace` gives 166 passed, 0 failed, 3 ignored (the three need Docker). Confirmed on 2026-10-04 by running the suite on a copy of that commit, on Windows 11 with cargo 1.93.1. The scaffold plan records 209 passed on `factory/m0` once CC-COORD has merged (166, 10 skeleton tests, 33 `xtask` tests); that is the lanes' branch point.
- **Test first (C1):** each behaviour gets a failing test, run and seen to fail, before the code that passes it.
- **Commits:** conventional subjects (`feat(harness-protocol): …`). No `Co-Authored-By` line and no "Generated with" footer, in commits or in pull request bodies.
- **Shell:** commands are written for Git-Bash and run from the lane's worktree root. `VAR=value command` is Git-Bash syntax; do not paste it into PowerShell.
- **This repository is public.** Nothing from the private design or doctrine is pasted into code, comments, commits or pull requests. Doctrine is cited by principle id only.
- **Spend nothing.** No network beyond `cargo` fetching already-locked crates, no keys, no deploys.

## Requests to the coordinator

Lane CC-COORD (the scaffold plan, `2026-10-04-factory-m0-scaffold.md`) owns the root manifest, the
lock file and every crate's `Cargo.toml`, and collects this section in its Task 11. The tasks below
assume each request is already in `factory/m0`. If one is missing, stop at the task that names it
and report; do not edit a manifest or the lock file yourself.

| # | Crate that needs it | Dependency | Version and features | Kind | Needed by | Note |
|---|---|---|---|---|---|---|
| R1 | `harness-protocol` | `sha2` | workspace (`0.10`), no features | normal | Task 5 | The scaffold plan's Task 2 already moves it out of `[dev-dependencies]`. Listed so it is not lost. `Cargo.lock` does not change: the lock does not record a dependency's kind |
| R2 | `harness-conformance` | `fleet-core` (workspace crate) | `{ path = "../fleet-core" }`, no features | dev | Task 9 | For the test that replays `harness-fake`'s transcripts through `fleet_core::transition`. A dev-dependency needs no rule change in the dependency-direction check. Adds one line to `Cargo.lock` |
| R3 | `harness-conformance` | `serde_json` | workspace (`1`), no features | normal | Task 11 | Parses `--work-order`. SP-1 dropped it from this crate as unused. Adds one line to `Cargo.lock` |

Nothing is asked about `testkit`: the scaffold adds that feature to `harness-protocol` and
`fleet-core`, and this plan puts nothing behind it.

For the coordinator's information, not a request: once lane CC-PROTO merges, the conformance kit
prints seven `PASS` lines, not six, so `cargo xtask test contract` and the CI job
`harness conformance` show `7 passed, 0 skipped, 0 failed`. Every new `tests/*.rs` target in this
plan is a `contract`-tier target by the scaffold's naming rule; none needs Docker, a network or a
token.

After the lanes merge, the owner registers the new contract hash; that is not a coordinator task.

## Review Focus

The five inputs most likely to bite that an ordinary happy-path test would not exercise. Each has a
named test in the task that owns it.

1. **A 0.1-shaped message arriving at 0.2 types.** A missing new field must parse to its default, and an absent `Option` must not be written back as `null`. Tests: `a_0_1_initialize_still_parses_and_accepts_only_its_own_version` (Task 1); `a_0_1_capabilities_object_still_parses_with_empty_0_2_fields`, `a_0_1_work_order_still_parses_with_empty_0_2_fields` (Task 2); `a_bare_oracle_frozen_still_parses_and_serialises_bare`, `a_0_1_metric_still_parses_and_a_0_2_metric_names_its_source`, `a_0_1_oracle_gate_request_still_parses` (Task 3); `a_0_1_result_still_parses_and_has_no_stop` (Task 4).
2. **A version string that is not exactly `major.minor`.** `0.2.7`, `v0.2`, `0.+2`, an empty string, a trailing space, a number too large for `u64`. Test: `version_strings_that_are_not_major_dot_minor_match_nothing` (Task 1).
3. **Paths given to `bundle_hash` on Windows.** A backslash separator or a different case is a different path, and the order files arrive in must not matter. Tests: `bundle_hash_does_not_normalise_path_separators_or_case`, `bundle_hash_ignores_the_order_files_arrive_in`, `bundle_hash_sorts_bytewise_so_uppercase_and_punctuation_have_one_order` (Task 5).
4. **A line at the edge of the codec's limits.** Exactly at the limit, one byte over, at the limit with no final newline, CRLF endings, and invalid UTF-8 followed by a valid line. Tests: `a_line_at_the_limit_is_read_and_one_byte_more_is_too_long`, `a_line_at_the_limit_with_no_final_newline_is_still_read`, `invalid_utf8_is_reported_as_such_and_the_next_line_is_still_readable`, `crlf_line_endings_are_accepted` (Task 6).
5. **A new trigger in a phase it must be rejected in.** `fleet-core` cannot know which phase a paused unit came from, so `Reverify { from }` takes the caller's word; every `from` other than `MergeCheck` and `Checking` must be refused, and each new trigger must be refused in every phase not listed for it, at every tier. Tests: the `assert_only` table tests in Tasks 13 and 14, which walk all 14 phases at all 3 tiers, and `every_pair_of_state_and_trigger_matches_the_table` in Task 15, which walks all 25 pairs.

## How to read a task

- A fenced `rust` block is code to put in the file named above it, exactly as shown. Every listing in this plan was compiled, formatted with `rustfmt` and run.
- A fenced `diff` block is an edit to an existing file: remove the `-` lines, add the `+` lines. The unmarked lines are context and do not change. Line numbers in `@@` headers are for orientation.
- "Inside `mod tests`" means inside the file's existing `#[cfg(test)] mod tests { … }` block, before its closing brace. Those listings are already indented four spaces.
- "Expected" after a command is what you must see before going on. If you see something else, stop and work out why; do not adjust the test to match.
- Re-blessing (Tasks 1 to 4) is always the same three commands, written out in each task. The hash each task gives is the one this plan's code produces. If yours differs, your code differs from the listing, usually in a doc comment, because doc comments become schema descriptions. Find the difference before you pin.

---

## Lane CC-PROTO

**Owns:** `crates/harness-protocol/**` and `crates/harness-conformance/**`, except the two `Cargo.toml` files (coordinator, requests R1 to R3).

**Reads:** `crates/fleet-core/src/**` (Task 9's replay test calls `transition` and `gate_met`); `docs/superpowers/specs/2026-09-16-sp2a-harness-supervisor-design.md`; issues #82, #83 and #87.

**Worktree and branch:** worktree `D:\MajorProjects\.swarm-wt\m0-cc-proto`, branch `feat/m0-protocol-0.2`, cut from `origin/factory/m0`. The pull request goes against `factory/m0`.

```bash
git -C /d/MajorProjects/INFRASTRUCTURE/command-center fetch origin
git -C /d/MajorProjects/INFRASTRUCTURE/command-center worktree add /d/MajorProjects/.swarm-wt/m0-cc-proto -b feat/m0-protocol-0.2 origin/factory/m0
cd /d/MajorProjects/.swarm-wt/m0-cc-proto
```

Expected: `Preparing worktree (new branch 'feat/m0-protocol-0.2')`. Never switch branches or commit in the main checkout.

**Needs:** CC-COORD merged into `factory/m0`, which it creates, with requests R1 to R3. Nothing from CC-CORE.

**Blocks:** the `contracts-v0.2.0` tag and, through it, the harness repository's skeleton lane; milestone 1's supervisor (the monitor), verifier (file hashing) and store lanes. Resolves #82, #83 and #87.

**Verify** (run all of these before opening the pull request; the numbers are for this lane alone on top of the baseline):

| Command | Expected |
|---|---|
| `cargo fmt --all -- --check` | No output, exit 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | Exit 0, no warning |
| `cargo test -p harness-protocol` | 69 passed, 0 failed: 62 unit, 3 in `contract`, 4 in `readme` |
| `cargo test -p harness-conformance` | 69 passed, 0 failed: 7 unit, 10 `cli`, 10 `detects_v02`, 16 `detects_violations`, 2 `fake_conforms`, 10 `fake_peer`, 1 `fake_replay`, 2 `fake_smoke`, 9 `fake_v02`, 2 `session_exit` |
| `cargo test --workspace` | 0 failed, 3 ignored, and 100 more passed than the branch point: 309 on the scaffold's 209 (266 on the bare 166 baseline) |
| `cargo xtask test static --root-only` | Exit 0. The scaffold plan gives the last line as `xtask: static: 6 step(s) passed` |
| `cargo xtask test unit` | Exit 0; the last line ends `step(s) passed` |
| `cargo xtask test contract` | Exit 0; seven `PASS` lines from the conformance kit |
| `cargo build -p harness-conformance --bins && target/debug/harness-conformance --wall-clock-secs 30 --grace-secs 5 -- target/debug/harness-fake` | Seven `PASS` lines, then `7 passed, 0 skipped, 0 failed`; exit 0. This is the CI job `harness conformance` |
| `git diff --stat origin/factory/m0 -- crates/harness-conformance/tests/detects_violations.rs` | No output |
| `git diff --name-only origin/factory/m0` | Only paths under `crates/harness-protocol/` and `crates/harness-conformance/`; no `Cargo.toml`, no `Cargo.lock` |

The three `cargo xtask` commands exist once CC-COORD has merged; this plan could not run them. If `cargo xtask` is not found, the scaffold is not in your branch: stop and report.

### Task 1: Protocol 0.2 and minor-aware negotiation

**Files:**
- Modify: `crates/harness-protocol/src/lib.rs`, `crates/harness-protocol/src/rpc.rs`, `crates/harness-protocol/src/types.rs`
- Modify: `crates/harness-protocol/contract/harness-protocol.contract.json` (re-blessed), `crates/harness-protocol/tests/contract.rs` (re-pinned)
- Modify: `crates/harness-conformance/src/cases.rs`, `crates/harness-conformance/src/bin/harness-fake.rs`, `crates/harness-conformance/tests/fake_smoke.rs` (the callers of the changed type)
- Test: `crates/harness-protocol/src/rpc.rs` (inline `mod tests`)

**Interfaces:**
- Consumes: nothing new.
- Produces:
  - `pub const PROTOCOL_VERSION: &str = "0.2";`
  - `pub fn negotiate(accepted: &[&str], harness_speaks: &str) -> bool`
  - `pub fn version_compatible(peer: &str) -> bool` — kept; now `negotiate(&[peer], PROTOCOL_VERSION)`, so it is minor-aware
  - `pub struct InitializeParams { pub protocol_version: String, pub accepted_versions: Vec<String> }`
  - `impl InitializeParams { pub fn accepted(&self) -> Vec<&str> }` — the one place that knows an empty `accepted_versions` means "`protocol_version` only". It is an addition to the shared-interface list; see the self-review.

- [ ] **Step 1: Write the failing tests**

In `crates/harness-protocol/src/rpc.rs`, replace the whole `#[cfg(test)] mod tests { … }` block with:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Empty, InitializeParams};
    use serde_json::json;

    #[test]
    fn request_omits_absent_fields_on_the_wire() {
        let msg = RpcMessage::request(7, method::UNIT_HALT, &Empty {});
        assert_eq!(
            serde_json::to_value(&msg).unwrap(),
            json!({"jsonrpc": "2.0", "id": 7, "method": "unit/halt", "params": {}})
        );
    }

    #[test]
    fn initialize_params_carry_the_accepted_versions() {
        let msg = RpcMessage::request(
            1,
            method::INITIALIZE,
            &InitializeParams {
                protocol_version: "0.2".into(),
                accepted_versions: vec!["0.2".into(), "0.1".into()],
            },
        );
        assert_eq!(
            serde_json::to_value(&msg).unwrap()["params"],
            json!({"protocol_version": "0.2", "accepted_versions": ["0.2", "0.1"]})
        );
    }

    #[test]
    fn a_0_1_initialize_still_parses_and_accepts_only_its_own_version() {
        let params: InitializeParams =
            serde_json::from_value(json!({"protocol_version": "0.1"})).unwrap();
        assert!(params.accepted_versions.is_empty());
        assert_eq!(params.accepted(), ["0.1"]);

        let both: InitializeParams = serde_json::from_value(
            json!({"protocol_version": "0.2", "accepted_versions": ["0.2", "0.1"]}),
        )
        .unwrap();
        assert_eq!(both.accepted(), ["0.2", "0.1"]);
    }

    #[test]
    fn kind_classifies_all_four_shapes_and_rejects_the_rest() {
        let req = RpcMessage::request(7, method::UNIT_HALT, &Empty {});
        assert_eq!(
            req.kind(),
            MessageKind::Request {
                id: 7,
                method: "unit/halt"
            }
        );

        let note = RpcMessage::notification(method::UNIT_EVENT, &Empty {});
        assert_eq!(
            note.kind(),
            MessageKind::Notification {
                method: "unit/event"
            }
        );

        let resp = RpcMessage::response(7, &Empty {});
        assert_eq!(resp.kind(), MessageKind::Response { id: 7 });

        let err = RpcMessage::error(7, error_code::PROTOCOL_VERSION_UNSUPPORTED, "no");
        match err.kind() {
            MessageKind::ErrorResponse { id, error } => {
                assert_eq!(id, 7);
                assert_eq!(error.code, -32001);
            }
            other => panic!("expected ErrorResponse, got {other:?}"),
        }

        let mut wrong_version = RpcMessage::notification(method::UNIT_EVENT, &Empty {});
        wrong_version.jsonrpc = "1.0".into();
        assert_eq!(wrong_version.kind(), MessageKind::Invalid);

        let both: RpcMessage = serde_json::from_value(
            json!({"jsonrpc": "2.0", "id": 1, "result": {}, "error": {"code": 1, "message": "x"}}),
        )
        .unwrap();
        assert_eq!(both.kind(), MessageKind::Invalid);
    }

    #[test]
    fn params_round_trip_through_params_as() {
        let msg = RpcMessage::request(
            2,
            method::INITIALIZE,
            &InitializeParams {
                protocol_version: "0.2".into(),
                accepted_versions: vec!["0.2".into()],
            },
        );
        let back: InitializeParams = msg.params_as().unwrap();
        assert_eq!(back.protocol_version, "0.2");
        assert_eq!(back.accepted_versions, ["0.2"]);
    }

    #[test]
    fn while_the_major_is_zero_the_minor_must_match_an_accepted_version() {
        assert!(negotiate(&["0.2"], "0.2"));
        assert!(negotiate(&["0.1", "0.2"], "0.2"));
        assert!(negotiate(&["0.1", "0.2"], "0.1"));
        assert!(!negotiate(&["0.2"], "0.1"));
        assert!(!negotiate(&["0.2"], "0.3"));
        assert!(!negotiate(&["0.2"], "1.2"));
        assert!(!negotiate(&[], "0.2"));
    }

    #[test]
    fn from_one_point_zero_an_equal_major_is_enough() {
        assert!(negotiate(&["1.0"], "1.4"));
        assert!(negotiate(&["1.4"], "1.0"));
        assert!(!negotiate(&["1.0"], "2.0"));
        assert!(!negotiate(&["1.0"], "0.1"));
    }

    #[test]
    fn version_strings_that_are_not_major_dot_minor_match_nothing() {
        // A patch component is ignored; everything else here is refused.
        assert!(negotiate(&["0.2"], "0.2.7"));
        assert!(negotiate(&["0.2.0"], "0.2"));
        for bad in [
            "", "0", "0.", ".2", "v0.2", "0.2 ", " 0.2", "0.+2", "0.-2", "0.x", "0,2",
        ] {
            assert!(!negotiate(&["0.2"], bad), "`{bad}` must not negotiate");
            assert!(!negotiate(&[bad], "0.2"), "`{bad}` must not be accepted");
        }
        // A number too large for u64 is not a version.
        assert!(!negotiate(&["0.2"], "0.99999999999999999999999"));
    }

    #[test]
    fn version_compatible_is_negotiate_against_this_crate() {
        assert!(version_compatible("0.2"));
        assert!(!version_compatible("0.1"));
        assert!(!version_compatible("0.9"));
        assert!(!version_compatible("1.0"));
        assert!(!version_compatible("99.0"));
        assert!(!version_compatible("not-a-version"));
    }
}
```

`version_compatibility_is_by_major` is gone on purpose: its assertion that `0.9` is compatible is the 0.1 rule this task removes.

- [ ] **Step 2: Run the tests and watch them fail**

Run: `cargo test -p harness-protocol --lib rpc::`
Expected: the build fails. The errors include E0425 (cannot find function `negotiate` in this scope), E0560 (`InitializeParams` has no field named `accepted_versions`) and E0599 (no method named `accepted`).

- [ ] **Step 3: Implement negotiation**

In `crates/harness-protocol/src/rpc.rs`, replace the function `major` and the function `version_compatible` (the two items just above `mod tests`) with:

```rust
/// One dot-separated component: ASCII digits only, so `+2`, ` 2` and `` are not numbers.
fn component(part: &str) -> Option<u64> {
    if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    part.parse().ok()
}

/// `(major, minor)` of a version string. Anything after the minor is ignored.
fn major_minor(version: &str) -> Option<(u64, u64)> {
    let mut parts = version.split('.');
    let major = component(parts.next()?)?;
    let minor = component(parts.next()?)?;
    Some((major, minor))
}

/// True iff a harness that speaks `harness_speaks` may talk to a control plane that accepts
/// `accepted`. While the major is 0 the minor must equal an accepted version's minor; from 1.0 on
/// an equal major is enough. A version that does not parse matches nothing.
pub fn negotiate(accepted: &[&str], harness_speaks: &str) -> bool {
    let Some((major, minor)) = major_minor(harness_speaks) else {
        return false;
    };
    accepted
        .iter()
        .filter_map(|version| major_minor(version))
        .any(|(a_major, a_minor)| a_major == major && (major != 0 || a_minor == minor))
}

/// True iff `peer` may talk to this crate's `PROTOCOL_VERSION` (see `negotiate`).
pub fn version_compatible(peer: &str) -> bool {
    negotiate(&[peer], crate::PROTOCOL_VERSION)
}
```

In `crates/harness-protocol/src/types.rs`, replace the definition of `InitializeParams` with:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct InitializeParams {
    /// The version the control plane would rather speak.
    pub protocol_version: String,
    /// All versions the control plane can speak. When the list is empty, only
    /// `protocol_version` is on offer.
    #[serde(default)]
    pub accepted_versions: Vec<String>,
}

impl InitializeParams {
    /// The versions the control plane accepts: `accepted_versions`, or `protocol_version` alone
    /// when that list is empty. Pass the result to `negotiate`.
    pub fn accepted(&self) -> Vec<&str> {
        if self.accepted_versions.is_empty() {
            vec![self.protocol_version.as_str()]
        } else {
            self.accepted_versions.iter().map(String::as_str).collect()
        }
    }
}
```

In `crates/harness-protocol/src/lib.rs`, replace everything from `mod codec;` to the end of the file with:

```rust
mod codec;
mod rpc;
mod schema;
mod types;

pub use codec::{read_message, write_message, ReadError};
pub use rpc::{
    error_code, method, negotiate, version_compatible, MessageKind, RpcError, RpcMessage,
};
pub use schema::{schema_json, ProtocolSchema};
pub use types::*;

/// The protocol version this crate speaks. While the major is 0, a peer on a different **minor**
/// is refused; see `negotiate`.
pub const PROTOCOL_VERSION: &str = "0.2";
```

- [ ] **Step 4: Run the protocol crate's unit tests**

Run: `cargo test -p harness-protocol --lib`
Expected: `test result: ok. 16 passed; 0 failed`.

- [ ] **Step 5: Update the three callers in the conformance crate**

`InitializeParams` has a new field, so every struct literal must name it, and `harness-fake` must decide with `negotiate`:

```diff
--- a/crates/harness-conformance/src/bin/harness-fake.rs
+++ b/crates/harness-conformance/src/bin/harness-fake.rs
@@ -3,11 +3,10 @@
 //! be proven. `HARNESS_FAKE_STEP_MS` paces events so a controller can interrupt mid-run.
 
 use harness_protocol::{
-    error_code, method, read_message, version_compatible, write_message, ArtifactKind,
-    Capabilities, Delivery, DeliveryEvidence, Empty, ErrorScope, Evidence, Failure, GateKind,
-    GateReply, GateRequest, HarnessInfo, InitializeParams, InitializeResult, Isolation,
-    MessageKind, Metering, Observation, Outcome, RpcMessage, TestRun, UnitEvent, UnitResult,
-    WorkOrder, PROTOCOL_VERSION,
+    error_code, method, negotiate, read_message, write_message, ArtifactKind, Capabilities,
+    Delivery, DeliveryEvidence, Empty, ErrorScope, Evidence, Failure, GateKind, GateReply,
+    GateRequest, HarnessInfo, InitializeParams, InitializeResult, Isolation, MessageKind, Metering,
+    Observation, Outcome, RpcMessage, TestRun, UnitEvent, UnitResult, WorkOrder, PROTOCOL_VERSION,
 };
 use std::io::{self, BufReader, Write};
 use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
@@ -363,7 +362,7 @@ fn main() {
             return;
         }
     };
-    if mode != Mode::AcceptAnyVersion && !version_compatible(&params.protocol_version) {
+    if mode != Mode::AcceptAnyVersion && !negotiate(&params.accepted(), PROTOCOL_VERSION) {
         let message = format!("harness-fake speaks protocol {PROTOCOL_VERSION}");
         send(&RpcMessage::error(
             id,
--- a/crates/harness-conformance/src/cases.rs
+++ b/crates/harness-conformance/src/cases.rs
@@ -57,6 +57,7 @@ pub(crate) fn start_session(cfg: &KitConfig) -> Result<(Session, Capabilities),
         method::INITIALIZE,
         &InitializeParams {
             protocol_version: PROTOCOL_VERSION.into(),
+            accepted_versions: vec![PROTOCOL_VERSION.into()],
         },
     ));
     let reply = match session.recv(cfg.grace) {
@@ -259,6 +260,7 @@ pub(crate) fn version_mismatch_refused(cfg: &KitConfig) -> CaseReport {
         method::INITIALIZE,
         &InitializeParams {
             protocol_version: "99.0".into(),
+            accepted_versions: Vec::new(),
         },
     ));
     match session.recv(cfg.grace) {
--- a/crates/harness-conformance/tests/fake_smoke.rs
+++ b/crates/harness-conformance/tests/fake_smoke.rs
@@ -54,6 +54,7 @@ fn conformant_fake_runs_a_t1_unit_to_pr_open() {
         method::INITIALIZE,
         &InitializeParams {
             protocol_version: PROTOCOL_VERSION.into(),
+            accepted_versions: vec![PROTOCOL_VERSION.into()],
         },
     );
     write_message(&mut stdin, &init).unwrap();
@@ -112,6 +113,7 @@ fn fake_refuses_a_foreign_major_version() {
         method::INITIALIZE,
         &InitializeParams {
             protocol_version: "99.0".into(),
+            accepted_versions: Vec::new(),
         },
     );
     write_message(&mut stdin, &init).unwrap();
```

Run: `cargo test -p harness-conformance`
Expected: 24 passed, 0 failed (4 `cli`, 16 `detects_violations`, 2 `fake_conforms`, 2 `fake_smoke`).

- [ ] **Step 6: Re-bless the contract and re-pin its hash**

```bash
HARNESS_PROTOCOL_BLESS=1 cargo test -p harness-protocol --test contract
cargo test -p harness-protocol --test contract
```

Expected from the first command: `3 passed` (it rewrites the contract file). Expected from the second: `committed_contract_hash_is_pinned ... FAILED` with

```text
  left: "693516f1497420372126a26d43943b9e5d65c360c57e88d6c194d904f1479500"
 right: "51c64132b2d3d898791f064820ffb92fdbd2b0d36f71913bd8d85e48f1df9463"
```

In `crates/harness-protocol/tests/contract.rs`, set `CONTRACT_SHA256` to the `left` value. Run `cargo test -p harness-protocol --test contract` again. Expected: `3 passed`.

- [ ] **Step 7: Commit**

```bash
cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings
git add crates/harness-protocol/src/lib.rs crates/harness-protocol/src/rpc.rs crates/harness-protocol/src/types.rs crates/harness-protocol/contract/harness-protocol.contract.json crates/harness-protocol/tests/contract.rs crates/harness-conformance/src/cases.rs crates/harness-conformance/src/bin/harness-fake.rs crates/harness-conformance/tests/fake_smoke.rs
git commit -m "feat(harness-protocol): protocol 0.2 with minor-aware version negotiation"
```

### Task 2: Capabilities and the work order

**Files:**
- Modify: `crates/harness-protocol/src/types.rs`, `crates/harness-protocol/tests/contract.rs`, `crates/harness-protocol/contract/harness-protocol.contract.json`
- Modify: `crates/harness-conformance/src/fixtures.rs`, `crates/harness-conformance/src/bin/harness-fake.rs`, `crates/harness-conformance/tests/fake_smoke.rs` (struct literals)
- Test: `crates/harness-protocol/src/types.rs` (inline), `crates/harness-protocol/tests/contract.rs`

**Interfaces:**
- Consumes: nothing from Task 1.
- Produces (all derive `Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema`; the fieldless enums also `Copy, Eq`):
  - `enum Network { Open, Filtered }`, `enum ControlKind { Scope, Protected, Secrets, Dependencies, EidosGates, Oracle }`, `enum UnitKind { Build, Draft, Adopt }`
  - `struct ProfileInfo { name: String, priced: bool }`, `struct PresetInfo { name: String, version: String }`
  - `Capabilities` gains `holdouts: bool`, `controls: Vec<ControlKind>`, `network: Option<Network>`, `profiles: Vec<ProfileInfo>`, `kinds: Vec<UnitKind>`, `presets: Vec<PresetInfo>`
  - `struct SpecRef { id, bytes_path, signed_hash: String, child: Option<String> }`, `struct Source { bundle_path, base_sha: String }`, `struct Scope { touched_files, create_paths, test_paths, touched_tests: Vec<String> }`
  - `struct RepoCommands { setup, build, test, format_check, lint: String }`, `struct RepoConfig { preset, image: String, env: BTreeMap<String, String>, commands: RepoCommands, test_report: String, test_dirs, manifests, lockfiles: Vec<String> }`
  - `struct Controls { map_max_drift: f64 }`, `struct Parent { spec_id: String }`
  - `WorkOrder` gains `kind: Option<UnitKind>`, `spec: Option<SpecRef>`, `source: Option<Source>`, `scope: Option<Scope>`, `scope_grants`, `permitted_dependencies`, `expected_red: Vec<String>`, `config: Option<RepoConfig>`, `controls: Option<Controls>`, `profile: Option<String>`, `parent: Option<Parent>`

- [ ] **Step 1: Write the failing tests**

In `crates/harness-protocol/src/types.rs`, inside `mod tests`, add:

```rust
    #[test]
    fn a_0_1_capabilities_object_still_parses_with_empty_0_2_fields() {
        let caps: Capabilities = serde_json::from_value(json!({
            "isolation": "container", "metering": "usd", "gates": ["oracle"],
            "delivery": "bundle", "resume": false, "halt": true
        }))
        .unwrap();
        assert!(!caps.holdouts);
        assert!(caps.controls.is_empty() && caps.profiles.is_empty());
        assert!(caps.kinds.is_empty() && caps.presets.is_empty());
        assert_eq!(caps.network, None);
        // An absent `network` stays absent; the list fields are always written.
        let v = to_value(&caps).unwrap();
        assert!(v.get("network").is_none());
        assert_eq!(v["kinds"], json!([]));
    }

    #[test]
    fn capabilities_0_2_fields_use_the_wire_names() {
        let caps = Capabilities {
            isolation: Isolation::Container,
            metering: Metering::Usd,
            gates: vec![GateKind::Oracle],
            delivery: Delivery::Bundle,
            resume: true,
            halt: true,
            holdouts: true,
            controls: vec![ControlKind::Scope, ControlKind::EidosGates],
            network: Some(Network::Open),
            profiles: vec![ProfileInfo {
                name: "claude".into(),
                priced: true,
            }],
            kinds: vec![UnitKind::Build, UnitKind::Draft, UnitKind::Adopt],
            presets: vec![PresetInfo {
                name: "cargo".into(),
                version: "0.1.0".into(),
            }],
        };
        let v = to_value(&caps).unwrap();
        assert_eq!(v["holdouts"], json!(true));
        assert_eq!(v["controls"], json!(["scope", "eidos_gates"]));
        assert_eq!(v["network"], json!("open"));
        assert_eq!(v["profiles"], json!([{"name": "claude", "priced": true}]));
        assert_eq!(v["kinds"], json!(["build", "draft", "adopt"]));
        assert_eq!(v["presets"], json!([{"name": "cargo", "version": "0.1.0"}]));
        let back: Capabilities = serde_json::from_value(v).unwrap();
        assert_eq!(back, caps);
    }

    fn order_0_1() -> serde_json::Value {
        json!({
            "unit_id": "u1",
            "work_item": {"kind": "issue", "ref": "adbarc92/audience#82"},
            "tier": "t1",
            "task": "one line",
            "repo": {"url": "https://example.invalid/r.git", "slug": "example/r", "base_branch": "main"},
            "branch": "agent/u1",
            "test_cmd": "cargo test",
            "caps": {"usd": 1.0, "wall_clock_secs": 60, "min_review_rounds": 1}
        })
    }

    #[test]
    fn a_0_1_work_order_still_parses_with_empty_0_2_fields() {
        let order: WorkOrder = serde_json::from_value(order_0_1()).unwrap();
        assert_eq!(order.kind, None);
        assert!(order.spec.is_none() && order.source.is_none() && order.scope.is_none());
        assert!(order.scope_grants.is_empty());
        assert!(order.permitted_dependencies.is_empty() && order.expected_red.is_empty());
        assert!(order.config.is_none() && order.controls.is_none());
        assert!(order.profile.is_none() && order.parent.is_none());
        // Optional objects are omitted when absent, so a 0.1 peer sees no nulls.
        let v = to_value(&order).unwrap();
        for absent in [
            "kind", "spec", "source", "scope", "config", "controls", "profile", "parent", "resume",
        ] {
            assert!(v.get(absent).is_none(), "`{absent}` must be omitted");
        }
        assert_eq!(v["scope_grants"], json!([]));
    }

    #[test]
    fn a_0_2_work_order_round_trips_with_every_new_field() {
        let mut v = order_0_1();
        let extra = json!({
            "kind": "build",
            "spec": {"id": "spec-7", "bytes_path": "/store/spec-7.md", "signed_hash": "ab", "child": "c1"},
            "source": {"bundle_path": "/store/u1.bundle", "base_sha": "0123"},
            "scope": {
                "touched_files": ["src/lib.rs"], "create_paths": ["src/new/"],
                "test_paths": ["tests/"], "touched_tests": []
            },
            "scope_grants": ["src/extra.rs"],
            "permitted_dependencies": ["sha2"],
            "expected_red": ["gates::no_cycles"],
            "config": {
                "preset": "cargo",
                "image": "rust@sha256:00",
                "env": {"CARGO_TERM_COLOR": "never"},
                "commands": {
                    "setup": "cargo fetch", "build": "cargo build", "test": "cargo nextest run",
                    "format_check": "cargo fmt --check", "lint": "cargo clippy"
                },
                "test_report": "target/nextest/junit.xml",
                "test_dirs": ["tests/"], "manifests": ["Cargo.toml"], "lockfiles": ["Cargo.lock"]
            },
            "controls": {"map_max_drift": 0.1},
            "profile": "claude",
            "parent": {"spec_id": "spec-1"}
        });
        v.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        let order: WorkOrder = serde_json::from_value(v.clone()).unwrap();
        assert_eq!(order.kind, Some(UnitKind::Build));
        assert_eq!(order.spec.as_ref().unwrap().child.as_deref(), Some("c1"));
        assert_eq!(order.scope.as_ref().unwrap().create_paths, ["src/new/"]);
        assert_eq!(
            order.config.as_ref().unwrap().env["CARGO_TERM_COLOR"],
            "never"
        );
        assert_eq!(order.controls.as_ref().unwrap().map_max_drift, 0.1);
        assert_eq!(order.parent.as_ref().unwrap().spec_id, "spec-1");
        assert_eq!(to_value(&order).unwrap(), v);
    }

    #[test]
    fn a_spec_ref_without_a_child_omits_it() {
        let spec = SpecRef {
            id: "spec-7".into(),
            bytes_path: "/store/spec-7.md".into(),
            signed_hash: "ab".into(),
            child: None,
        };
        assert_eq!(
            to_value(&spec).unwrap(),
            json!({"id": "spec-7", "bytes_path": "/store/spec-7.md", "signed_hash": "ab"})
        );
    }
```

In `crates/harness-protocol/tests/contract.rs`, extend the list in `schema_defines_every_wire_type`:

```diff
--- a/crates/harness-protocol/tests/contract.rs
+++ b/crates/harness-protocol/tests/contract.rs
@@ -63,7 +63,19 @@ fn schema_defines_every_wire_type() {
         "InitializeParams",
         "InitializeResult",
         "Capabilities",
+        "Network",
+        "ControlKind",
+        "UnitKind",
+        "ProfileInfo",
+        "PresetInfo",
         "WorkOrder",
+        "SpecRef",
+        "Source",
+        "Scope",
+        "RepoCommands",
+        "RepoConfig",
+        "Controls",
+        "Parent",
         "UnitEvent",
         "Observation",
         "GateRequest",
```

- [ ] **Step 2: Run the tests and watch them fail**

Run: `cargo test -p harness-protocol --lib types::`
Expected: the build fails. The errors include E0560 (`Capabilities` has no field named `holdouts`) and unresolved names such as `ProfileInfo`, `ControlKind` and `SpecRef` (E0422, E0433).

- [ ] **Step 3: Add the capability types**

In `crates/harness-protocol/src/types.rs`, replace the definition of `Capabilities` (its doc comment, derive and body) with:

```rust
/// What an agent container may reach on the network.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Network {
    Open,
    Filtered,
}

/// A control the harness runs. The same names appear in `Evidence.controls`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ControlKind {
    Scope,
    Protected,
    Secrets,
    Dependencies,
    EidosGates,
    Oracle,
}

/// What a unit is for. A work order with no `kind` is a `Build`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UnitKind {
    Build,
    Draft,
    Adopt,
}

/// A model-routing profile the harness offers. `priced: false` means its usage is not metered in USD.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ProfileInfo {
    pub name: String,
    pub priced: bool,
}

/// A stack preset the harness links, with the preset crate's version.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PresetInfo {
    pub name: String,
    pub version: String,
}

/// What a harness guarantees. `fleetd` decides eligibility from this (spec §3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Capabilities {
    pub isolation: Isolation,
    pub metering: Metering,
    pub gates: Vec<GateKind>,
    pub delivery: Delivery,
    pub resume: bool,
    pub halt: bool,
    /// The harness writes hidden holdout tests and reports them in `OracleFreeze`.
    #[serde(default)]
    pub holdouts: bool,
    #[serde(default)]
    pub controls: Vec<ControlKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network: Option<Network>,
    #[serde(default)]
    pub profiles: Vec<ProfileInfo>,
    /// Empty means `build` only, as a 0.1 harness.
    #[serde(default)]
    pub kinds: Vec<UnitKind>,
    #[serde(default)]
    pub presets: Vec<PresetInfo>,
}
```

- [ ] **Step 4: Add the work-order types**

In the same file, replace the definition of `WorkOrder` (its doc comment, derive and body) with:

```rust
/// The signed spec a unit builds. The harness takes the spec's bytes from `bytes_path` and from
/// no other place.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SpecRef {
    pub id: String,
    pub bytes_path: String,
    /// SHA-256 (lowercase hex) of the bytes at `bytes_path`.
    pub signed_hash: String,
    /// The child of a spec set this unit builds, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub child: Option<String>,
}

/// Where the unit's source comes from: a git bundle the control plane prepared, and the commit
/// that bundle was cut at.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Source {
    pub bundle_path: String,
    pub base_sha: String,
}

/// The files a unit may change. An entry names one file by its path from the repository root; an
/// entry that ends in `/` names everything under that directory. Wildcards are not supported.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Scope {
    pub touched_files: Vec<String>,
    pub create_paths: Vec<String>,
    pub test_paths: Vec<String>,
    pub touched_tests: Vec<String>,
}

/// The commands a target repository declares.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RepoCommands {
    pub setup: String,
    pub build: String,
    pub test: String,
    pub format_check: String,
    pub lint: String,
}

/// How to build and test the target repository. The control plane supplies it; the harness does
/// not read it from the working tree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RepoConfig {
    pub preset: String,
    /// Container image reference, pinned to a digest.
    pub image: String,
    #[serde(default)]
    pub env: std::collections::BTreeMap<String, String>,
    pub commands: RepoCommands,
    /// Path of the JUnit XML file that `commands.test` produces.
    pub test_report: String,
    pub test_dirs: Vec<String>,
    pub manifests: Vec<String>,
    pub lockfiles: Vec<String>,
}

/// Thresholds for the harness's own controls.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Controls {
    pub map_max_drift: f64,
}

/// The spec set a child unit belongs to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Parent {
    pub spec_id: String,
}

/// `unit/start` params.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct WorkOrder {
    pub unit_id: String,
    pub work_item: WorkItem,
    pub tier: Tier,
    /// A short human-readable label. What to build is defined by `spec`.
    pub task: String,
    pub repo: Repo,
    pub branch: String,
    /// Kept for 0.1 harnesses. Under 0.2 it repeats `config.commands.test`.
    pub test_cmd: String,
    pub caps: Caps,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resume: Option<Resume>,
    /// Defaults to `Build` when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<UnitKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spec: Option<SpecRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<Source>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<Scope>,
    /// Files a human granted after a `scope_request` stop. They widen `scope`.
    #[serde(default)]
    pub scope_grants: Vec<String>,
    #[serde(default)]
    pub permitted_dependencies: Vec<String>,
    /// Tests or gates already failing before the unit starts, which the unit is expected to fix.
    #[serde(default)]
    pub expected_red: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<RepoConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub controls: Option<Controls>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<Parent>,
}
```

- [ ] **Step 5: Run the protocol crate's unit tests**

Run: `cargo test -p harness-protocol --lib`
Expected: `test result: ok. 21 passed; 0 failed`.

- [ ] **Step 6: Update the struct literals in the conformance crate**

```diff
--- a/crates/harness-conformance/src/bin/harness-fake.rs
+++ b/crates/harness-conformance/src/bin/harness-fake.rs
@@ -70,6 +70,12 @@ fn capabilities() -> Capabilities {
         delivery: Delivery::Bundle,
         resume: false,
         halt: true,
+        holdouts: false,
+        controls: Vec::new(),
+        network: None,
+        profiles: Vec::new(),
+        kinds: Vec::new(),
+        presets: Vec::new(),
     }
 }
 
--- a/crates/harness-conformance/src/fixtures.rs
+++ b/crates/harness-conformance/src/fixtures.rs
@@ -25,5 +25,16 @@ pub fn work_order(tier: Tier) -> WorkOrder {
             min_review_rounds: 1,
         },
         resume: None,
+        kind: None,
+        spec: None,
+        source: None,
+        scope: None,
+        scope_grants: Vec::new(),
+        permitted_dependencies: Vec::new(),
+        expected_red: Vec::new(),
+        config: None,
+        controls: None,
+        profile: None,
+        parent: None,
     }
 }
--- a/crates/harness-conformance/tests/fake_smoke.rs
+++ b/crates/harness-conformance/tests/fake_smoke.rs
@@ -31,6 +31,17 @@ fn order(tier: Tier) -> WorkOrder {
             min_review_rounds: 1,
         },
         resume: None,
+        kind: None,
+        spec: None,
+        source: None,
+        scope: None,
+        scope_grants: Vec::new(),
+        permitted_dependencies: Vec::new(),
+        expected_red: Vec::new(),
+        config: None,
+        controls: None,
+        profile: None,
+        parent: None,
     }
 }
 
```

Run: `cargo test -p harness-conformance`
Expected: 24 passed, 0 failed.

- [ ] **Step 7: Re-bless the contract and re-pin its hash**

```bash
HARNESS_PROTOCOL_BLESS=1 cargo test -p harness-protocol --test contract
cargo test -p harness-protocol --test contract
```

Expected from the second command: `committed_contract_hash_is_pinned ... FAILED` with `left: "e7e5392d65671da30560cff81c8cfaf9c3109d0f316572c36223a92b42513d99"`. Set `CONTRACT_SHA256` to that value and run `cargo test -p harness-protocol --test contract` again. Expected: `3 passed`, which now includes the twelve new names in `schema_defines_every_wire_type`.

- [ ] **Step 8: Commit**

```bash
cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings
git add crates/harness-protocol/src/types.rs crates/harness-protocol/tests/contract.rs crates/harness-protocol/contract/harness-protocol.contract.json crates/harness-conformance/src/fixtures.rs crates/harness-conformance/src/bin/harness-fake.rs crates/harness-conformance/tests/fake_smoke.rs
git commit -m "feat(harness-protocol): 0.2 capabilities and work-order fields"
```

### Task 3: Events and the oracle gate

**Files:**
- Modify: `crates/harness-protocol/src/types.rs`, `crates/harness-protocol/tests/contract.rs`, `crates/harness-protocol/contract/harness-protocol.contract.json`
- Modify: `crates/harness-conformance/src/bin/harness-fake.rs` (three literals)
- Test: `crates/harness-protocol/src/types.rs` (inline)

**Interfaces:**
- Consumes: nothing from Tasks 1 and 2.
- Produces:
  - `struct FrozenFile { path: String, sha256: String }` (also derives `Eq`)
  - `struct OracleFreeze { frozen_files: Vec<FrozenFile>, frozen_ids: Vec<String>, holdout_bundle_path: Option<String>, holdout_hash: Option<String>, holdout_ids: Vec<String> }`
  - `Observation::OracleFrozen` becomes the struct variant `OracleFrozen { freeze: Option<OracleFreeze> }`; `{"kind":"oracle_frozen"}` still parses
  - `enum Stage { Provision, Red, Plan, Green, Check, Review, Deliver }`, `enum StageStatus { Started, Finished }`, `enum CostBasis { Priced, Unpriced }`
  - `UnitEvent::Stage { stage: Stage, status: StageStatus, detail: Option<String> }`
  - `UnitEvent::Metric` gains `cost_basis: Option<CostBasis>`, `stage: Option<Stage>`, `role`, `adapter`, `model: Option<String>`
  - `GateRequest::Oracle` gains `holdout_files: Vec<String>`, `holdout_hash: Option<String>`

- [ ] **Step 1: Write the failing tests**

In `crates/harness-protocol/src/types.rs`, inside `mod tests`, add:

```rust
    #[test]
    fn a_bare_oracle_frozen_still_parses_and_serialises_bare() {
        let obs: Observation = serde_json::from_value(json!({"kind": "oracle_frozen"})).unwrap();
        assert_eq!(obs, Observation::OracleFrozen { freeze: None });
        assert_eq!(to_value(&obs).unwrap(), json!({"kind": "oracle_frozen"}));
    }

    #[test]
    fn oracle_frozen_carries_the_freeze_payload() {
        let obs = Observation::OracleFrozen {
            freeze: Some(OracleFreeze {
                frozen_files: vec![FrozenFile {
                    path: "tests/a.rs".into(),
                    sha256: "aa".into(),
                }],
                frozen_ids: vec!["a::ac1_adds".into()],
                holdout_bundle_path: Some("/store/u1.holdouts.tar".into()),
                holdout_hash: Some("bb".into()),
                holdout_ids: vec!["h::ac1_adds".into()],
            }),
        };
        let v = to_value(&obs).unwrap();
        assert_eq!(
            v,
            json!({"kind": "oracle_frozen", "freeze": {
                "frozen_files": [{"path": "tests/a.rs", "sha256": "aa"}],
                "frozen_ids": ["a::ac1_adds"],
                "holdout_bundle_path": "/store/u1.holdouts.tar",
                "holdout_hash": "bb",
                "holdout_ids": ["h::ac1_adds"]
            }})
        );
        assert_eq!(serde_json::from_value::<Observation>(v).unwrap(), obs);

        // A harness with no holdouts sends only the visible layer.
        let visible_only: Observation = serde_json::from_value(json!({
            "kind": "oracle_frozen",
            "freeze": {"frozen_files": [], "frozen_ids": []}
        }))
        .unwrap();
        let Observation::OracleFrozen {
            freeze: Some(freeze),
        } = visible_only
        else {
            panic!("expected a freeze payload");
        };
        assert!(freeze.holdout_ids.is_empty() && freeze.holdout_hash.is_none());
    }

    #[test]
    fn a_stage_event_is_type_tagged_and_omits_an_absent_detail() {
        let ev = UnitEvent::Stage {
            stage: Stage::Red,
            status: StageStatus::Started,
            detail: None,
        };
        assert_eq!(
            to_value(&ev).unwrap(),
            json!({"type": "stage", "stage": "red", "status": "started"})
        );
        let with_detail: UnitEvent = serde_json::from_value(
            json!({"type": "stage", "stage": "deliver", "status": "finished", "detail": "bundled"}),
        )
        .unwrap();
        assert_eq!(
            with_detail,
            UnitEvent::Stage {
                stage: Stage::Deliver,
                status: StageStatus::Finished,
                detail: Some("bundled".into()),
            }
        );
        let names: Vec<_> = [
            Stage::Provision,
            Stage::Red,
            Stage::Plan,
            Stage::Green,
            Stage::Check,
            Stage::Review,
            Stage::Deliver,
        ]
        .iter()
        .map(|s| to_value(s).unwrap())
        .collect();
        assert_eq!(
            names,
            [
                "provision",
                "red",
                "plan",
                "green",
                "check",
                "review",
                "deliver"
            ]
        );
    }

    #[test]
    fn a_0_1_metric_still_parses_and_a_0_2_metric_names_its_source() {
        let old: UnitEvent = serde_json::from_value(json!({
            "type": "metric", "tokens_in": 1, "tokens_out": 2, "cost_usd": 0.5, "elapsed_ms": 7
        }))
        .unwrap();
        let v = to_value(&old).unwrap();
        for absent in ["cost_basis", "stage", "role", "adapter", "model"] {
            assert!(v.get(absent).is_none(), "`{absent}` must be omitted");
        }

        let new = UnitEvent::Metric {
            tokens_in: 1,
            tokens_out: 2,
            cost_usd: 0.0,
            elapsed_ms: 7,
            cost_basis: Some(CostBasis::Unpriced),
            stage: Some(Stage::Green),
            role: Some("builder".into()),
            adapter: Some("opencode".into()),
            model: Some("local-model".into()),
        };
        let v = to_value(&new).unwrap();
        assert_eq!(v["cost_basis"], json!("unpriced"));
        assert_eq!(v["stage"], json!("green"));
        assert_eq!(v["role"], json!("builder"));
        assert_eq!(serde_json::from_value::<UnitEvent>(v).unwrap(), new);
    }

    #[test]
    fn a_0_1_oracle_gate_request_still_parses() {
        let req: GateRequest = serde_json::from_value(json!({
            "gate": "oracle", "test_files": ["tests/a.test.js"], "hash": "h", "summary": "s"
        }))
        .unwrap();
        let GateRequest::Oracle {
            holdout_files,
            holdout_hash,
            ..
        } = req;
        assert!(holdout_files.is_empty());
        assert_eq!(holdout_hash, None);
    }
```

Two existing tests build a `Metric` and an oracle gate request by struct literal. Replace them with these versions, which name the new fields:

```rust
    #[test]
    fn unit_event_observed_nests_a_kind_tagged_observation() {
        let ev = UnitEvent::Observed {
            observation: Observation::ReviewFinished {
                round: 3,
                unresolved_blockers: 0,
                checks_green: true,
            },
        };
        assert_eq!(
            to_value(&ev).unwrap(),
            json!({"type": "observed", "observation": {
                "kind": "review_finished", "round": 3, "unresolved_blockers": 0, "checks_green": true
            }})
        );
        let metric = UnitEvent::Metric {
            tokens_in: 10,
            tokens_out: 2,
            cost_usd: 0.5,
            elapsed_ms: 7,
            cost_basis: None,
            stage: None,
            role: None,
            adapter: None,
            model: None,
        };
        assert_eq!(to_value(&metric).unwrap()["type"], json!("metric"));
    }

    #[test]
    fn gate_request_is_gate_tagged_and_round_trips() {
        let req = GateRequest::Oracle {
            test_files: vec!["tests/a.test.js".into()],
            hash: "h".into(),
            summary: "s".into(),
            holdout_files: vec!["tests/holdout/a.test.js".into()],
            holdout_hash: Some("hh".into()),
        };
        let v = to_value(&req).unwrap();
        assert_eq!(v["gate"], json!("oracle"));
        let back: GateRequest = serde_json::from_value(v).unwrap();
        assert_eq!(back, req);
    }
```

In `crates/harness-protocol/tests/contract.rs`:

```diff
--- a/crates/harness-protocol/tests/contract.rs
+++ b/crates/harness-protocol/tests/contract.rs
@@ -78,6 +78,11 @@ fn schema_defines_every_wire_type() {
         "Parent",
         "UnitEvent",
         "Observation",
+        "OracleFreeze",
+        "FrozenFile",
+        "Stage",
+        "StageStatus",
+        "CostBasis",
         "GateRequest",
         "GateReply",
         "UnitResult",
```

- [ ] **Step 2: Run the tests and watch them fail**

Run: `cargo test -p harness-protocol --lib types::`
Expected: the build fails. The errors include unresolved `OracleFreeze`, `Stage` and `CostBasis`, and E0559 (variant `UnitEvent::Metric` has no field named `cost_basis`).

- [ ] **Step 3: Add the freeze payload and the stage types**

In `crates/harness-protocol/src/types.rs`, replace the definition of `Observation` (doc comment, derive, serde attribute and body) with:

```rust
/// One frozen test file and the SHA-256 (lowercase hex) of its bytes; see `file_sha256`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FrozenFile {
    pub path: String,
    pub sha256: String,
}

/// What the control plane uses to verify the oracle on its own. Sent at T1 as well as T2 and T3.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct OracleFreeze {
    /// The visible frozen test files.
    pub frozen_files: Vec<FrozenFile>,
    /// Ids of the tests in those files, listed when the files were frozen.
    pub frozen_ids: Vec<String>,
    /// An archive of the holdout tests, each stored under the path it has in the repository.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub holdout_bundle_path: Option<String>,
    /// `bundle_hash` over the holdout files.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub holdout_hash: Option<String>,
    #[serde(default)]
    pub holdout_ids: Vec<String>,
}

/// What the harness saw. `fleetd` maps each to a `fleet-core` trigger (SP-2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Observation {
    Provisioned,
    /// `freeze` is absent from a 0.1 harness; a 0.2 harness always sends it.
    OracleFrozen {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        freeze: Option<OracleFreeze>,
    },
    BuildFinished,
    ChecksPassed,
    ChecksFailed,
    EmptyDiff,
    ReviewFinished {
        round: u32,
        unresolved_blockers: u32,
        checks_green: bool,
    },
}

/// The seven stages of a unit, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Provision,
    Red,
    Plan,
    Green,
    Check,
    Review,
    Deliver,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StageStatus {
    Started,
    Finished,
}

/// Whether a `metric`'s `cost_usd` is a real price. `Unpriced` usage does not count against the
/// USD cap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CostBasis {
    Priced,
    Unpriced,
}
```

- [ ] **Step 4: Add the stage event and the metric fields**

Replace the definition of `UnitEvent` with:

```rust
/// `unit/event` params (a notification).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UnitEvent {
    Observed {
        observation: Observation,
    },
    /// Progress reporting. No phase changes because of it; it does show the harness is alive.
    Stage {
        stage: Stage,
        status: StageStatus,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
    },
    /// Every figure is incremental since the previous `metric` for this unit, not a running total.
    /// The control plane sums them into the per-unit spend (spec §4).
    Metric {
        tokens_in: u64,
        tokens_out: u64,
        cost_usd: f64,
        elapsed_ms: u64,
        /// Absent means `priced`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cost_basis: Option<CostBasis>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stage: Option<Stage>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        role: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        adapter: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        model: Option<String>,
    },

    Log {
        stream: LogStream,
        line: String,
    },
    Finding {
        round: u32,
        severity: Severity,
        title: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        file: Option<String>,
        resolved: bool,
    },
    Artifact {
        kind: ArtifactKind,
        #[serde(rename = "ref")]
        reference: String,
    },
    Error {
        scope: ErrorScope,
        retryable: bool,
        detail: String,
    },
}
```

- [ ] **Step 5: Add the holdout fields to the oracle gate**

Replace the definition of `GateRequest` with:

```rust
/// `gate/request` params (a request the harness blocks on).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "gate", rename_all = "snake_case")]
pub enum GateRequest {
    Oracle {
        test_files: Vec<String>,
        hash: String,
        summary: String,
        /// Paths of the holdout tests, shown to the approver beside the visible ones.
        #[serde(default)]
        holdout_files: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        holdout_hash: Option<String>,
    },
}
```

- [ ] **Step 6: Run the protocol crate's unit tests**

Run: `cargo test -p harness-protocol --lib`
Expected: `test result: ok. 26 passed; 0 failed`.

- [ ] **Step 7: Update the three literals in `harness-fake`**

`harness-fake` sends the new fields empty for now; Task 10 fills them.

```diff
--- a/crates/harness-conformance/src/bin/harness-fake.rs
+++ b/crates/harness-conformance/src/bin/harness-fake.rs
@@ -259,11 +259,13 @@ fn run_unit(mode: Mode, order: &WorkOrder, rx: &Receiver<RpcMessage>) {
     }
 
     if order.tier.requires_oracle() && mode != Mode::SkipGate {
-        observe(Observation::OracleFrozen);
+        observe(Observation::OracleFrozen { freeze: None });
         let gate = GateRequest::Oracle {
             test_files: vec!["tests/fake.test.js".into()],
             hash: "fake-oracle-hash".into(),
             summary: "one scripted test".into(),
+            holdout_files: Vec::new(),
+            holdout_hash: None,
         };
         send(&RpcMessage::request(
             GATE_REQUEST_ID,
@@ -294,6 +296,11 @@ fn run_unit(mode: Mode, order: &WorkOrder, rx: &Receiver<RpcMessage>) {
             tokens_out: 300,
             cost_usd: 0.02,
             elapsed_ms: pace.as_millis() as u64,
+            cost_basis: None,
+            stage: None,
+            role: None,
+            adapter: None,
+            model: None,
         });
     }
     observe(Observation::BuildFinished);
```

Run: `cargo test -p harness-conformance`
Expected: 24 passed, 0 failed.

- [ ] **Step 8: Re-bless the contract and re-pin its hash**

```bash
HARNESS_PROTOCOL_BLESS=1 cargo test -p harness-protocol --test contract
cargo test -p harness-protocol --test contract
```

Expected from the second command: `committed_contract_hash_is_pinned ... FAILED` with `left: "91954a351468a80ae62f655603f4fb3e843034dcaa693605367894ac5c24f343"`. Set `CONTRACT_SHA256` to that value and run the second command again. Expected: `3 passed`.

- [ ] **Step 9: Commit**

```bash
cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings
git add crates/harness-protocol/src/types.rs crates/harness-protocol/tests/contract.rs crates/harness-protocol/contract/harness-protocol.contract.json crates/harness-conformance/src/bin/harness-fake.rs
git commit -m "feat(harness-protocol): oracle freeze payload, stage events, metric source, gate holdouts"
```

### Task 4: Results and evidence

**Files:**
- Modify: `crates/harness-protocol/src/types.rs`, `crates/harness-protocol/tests/contract.rs`, `crates/harness-protocol/contract/harness-protocol.contract.json`
- Modify: `crates/harness-conformance/src/bin/harness-fake.rs` (five literals)
- Test: `crates/harness-protocol/src/types.rs` (inline)

**Interfaces:**
- Consumes: `ControlKind` from Task 2.
- Produces:
  - `enum Outcome { PrOpen, NoChange, Failed, NeedsHuman, DraftReady }`
  - `enum StopReason { ScopeRequest, SpecConflict, CheckUnrunnable, EscalationExhausted, HoldoutRoundsExhausted, ReviewRoundsExhausted, HoldoutDefect, MapUntrusted, BaselineRed, RuntimeUnavailable, BudgetExhausted }`
  - `struct Stop { reason: StopReason, detail: String, request: Vec<String> }`
  - `struct MapEvidence { base_sha, hash: String, drift: f64, eidos_level: String }`, `struct TestReport { ids_passed: Vec<String> }`
  - `enum ControlStatus { Passed, Failed, Unrunnable }`, `struct ControlResult { name: ControlKind, status: ControlStatus, detail: String }`
  - `enum Verdict { Holds, Violated, CannotDetermine }`, `struct ReviewVerdict { subject: String, verdict: Verdict, file: Option<String> }`, `struct ReviewEvidence { rounds: u32, prior_rounds: u32, verdicts: Vec<ReviewVerdict> }`
  - `Evidence` gains `spec_hash: Option<String>`, `map: Option<MapEvidence>`, `test_report: Option<TestReport>`, `controls: Vec<ControlResult>`, `review: Option<ReviewEvidence>`
  - `UnitResult` gains `stop: Option<Stop>`

- [ ] **Step 1: Write the failing tests**

In `crates/harness-protocol/src/types.rs`, inside `mod tests`, add:

```rust
    #[test]
    fn outcome_gains_needs_human_and_draft_ready() {
        assert_eq!(to_value(Outcome::NeedsHuman).unwrap(), json!("needs_human"));
        assert_eq!(to_value(Outcome::DraftReady).unwrap(), json!("draft_ready"));
        assert!(serde_json::from_value::<Outcome>(json!("merged")).is_err());
    }

    #[test]
    fn every_stop_reason_has_its_wire_name() {
        let reasons = [
            (StopReason::ScopeRequest, "scope_request"),
            (StopReason::SpecConflict, "spec_conflict"),
            (StopReason::CheckUnrunnable, "check_unrunnable"),
            (StopReason::EscalationExhausted, "escalation_exhausted"),
            (
                StopReason::HoldoutRoundsExhausted,
                "holdout_rounds_exhausted",
            ),
            (StopReason::ReviewRoundsExhausted, "review_rounds_exhausted"),
            (StopReason::HoldoutDefect, "holdout_defect"),
            (StopReason::MapUntrusted, "map_untrusted"),
            (StopReason::BaselineRed, "baseline_red"),
            (StopReason::RuntimeUnavailable, "runtime_unavailable"),
            (StopReason::BudgetExhausted, "budget_exhausted"),
        ];
        for (reason, wire) in reasons {
            assert_eq!(to_value(reason).unwrap(), json!(wire));
        }
    }

    #[test]
    fn a_needs_human_result_carries_a_stop_beside_the_outcome() {
        let result = UnitResult {
            outcome: Outcome::NeedsHuman,
            evidence: None,
            failure: None,
            stop: Some(Stop {
                reason: StopReason::ScopeRequest,
                detail: "the fix needs one more file".into(),
                request: vec!["src/extra.rs".into()],
            }),
        };
        let v = to_value(&result).unwrap();
        assert_eq!(
            v,
            json!({"outcome": "needs_human", "stop": {
                "reason": "scope_request",
                "detail": "the fix needs one more file",
                "request": ["src/extra.rs"]
            }})
        );
        assert_eq!(serde_json::from_value::<UnitResult>(v).unwrap(), result);

        // `request` may be left out for every other reason.
        let bare: Stop =
            serde_json::from_value(json!({"reason": "baseline_red", "detail": "2 tests red"}))
                .unwrap();
        assert!(bare.request.is_empty());
    }

    #[test]
    fn a_0_1_result_still_parses_and_has_no_stop() {
        let result: UnitResult = serde_json::from_value(json!({
            "outcome": "pr_open",
            "evidence": {
                "branch": "agent/u1", "head_sha": "0123",
                "delivery": {"kind": "bundle", "bundle_path": "u1.bundle"},
                "test": {"command": "cargo test", "exit_code": 0}
            }
        }))
        .unwrap();
        assert_eq!(result.stop, None);
        let evidence = result.evidence.unwrap();
        assert!(evidence.spec_hash.is_none() && evidence.map.is_none());
        assert!(evidence.test_report.is_none() && evidence.review.is_none());
        assert!(evidence.controls.is_empty());
    }

    #[test]
    fn evidence_0_2_fields_use_the_wire_names() {
        let evidence = Evidence {
            branch: "agent/u1".into(),
            head_sha: "0123".into(),
            delivery: DeliveryEvidence::Bundle {
                bundle_path: "u1.bundle".into(),
            },
            pr: None,
            test: TestRun {
                command: "cargo test".into(),
                exit_code: 0,
            },
            oracle_hash: Some("oh".into()),
            spec_hash: Some("sh".into()),
            map: Some(MapEvidence {
                base_sha: "0123".into(),
                hash: "mh".into(),
                drift: 0.02,
                eidos_level: "E2".into(),
            }),
            test_report: Some(TestReport {
                ids_passed: vec!["a::ac1_adds".into()],
            }),
            controls: vec![ControlResult {
                name: ControlKind::Dependencies,
                status: ControlStatus::Unrunnable,
                detail: "no network".into(),
            }],
            review: Some(ReviewEvidence {
                rounds: 2,
                prior_rounds: 3,
                verdicts: vec![ReviewVerdict {
                    subject: "AC-1".into(),
                    verdict: Verdict::CannotDetermine,
                    file: None,
                }],
            }),
        };
        let v = to_value(&evidence).unwrap();
        assert_eq!(v["spec_hash"], json!("sh"));
        assert_eq!(v["map"]["eidos_level"], json!("E2"));
        assert_eq!(v["test_report"], json!({"ids_passed": ["a::ac1_adds"]}));
        assert_eq!(
            v["controls"],
            json!([{"name": "dependencies", "status": "unrunnable", "detail": "no network"}])
        );
        assert_eq!(
            v["review"],
            json!({"rounds": 2, "prior_rounds": 3, "verdicts": [
                {"subject": "AC-1", "verdict": "cannot_determine"}
            ]})
        );
        assert_eq!(serde_json::from_value::<Evidence>(v).unwrap(), evidence);
    }
```

Replace the existing test `evidence_delivery_is_kind_tagged`, which builds an `Evidence` by struct literal, with:

```rust
    #[test]
    fn evidence_delivery_is_kind_tagged() {
        let ev = Evidence {
            branch: "agent/u1".into(),
            head_sha: "a".repeat(40),
            delivery: DeliveryEvidence::Bundle {
                bundle_path: "u1.bundle".into(),
            },
            pr: None,
            test: TestRun {
                command: "cargo test".into(),
                exit_code: 0,
            },
            oracle_hash: None,
            spec_hash: None,
            map: None,
            test_report: None,
            controls: Vec::new(),
            review: None,
        };
        let v = to_value(&ev).unwrap();
        assert_eq!(
            v["delivery"],
            json!({"kind": "bundle", "bundle_path": "u1.bundle"})
        );
        assert!(v.get("pr").is_none() && v.get("oracle_hash").is_none());
        assert_eq!(
            to_value(DeliveryEvidence::Push).unwrap(),
            json!({"kind": "push"})
        );
    }
```

In `crates/harness-protocol/tests/contract.rs`:

```diff
--- a/crates/harness-protocol/tests/contract.rs
+++ b/crates/harness-protocol/tests/contract.rs
@@ -86,7 +86,17 @@ fn schema_defines_every_wire_type() {
         "GateRequest",
         "GateReply",
         "UnitResult",
+        "Outcome",
+        "Stop",
+        "StopReason",
         "Evidence",
+        "MapEvidence",
+        "TestReport",
+        "ControlResult",
+        "ControlStatus",
+        "ReviewEvidence",
+        "ReviewVerdict",
+        "Verdict",
         "Empty",
     ] {
         assert!(defs.contains_key(name), "schema is missing $defs.{name}");
```

- [ ] **Step 2: Run the tests and watch them fail**

Run: `cargo test -p harness-protocol --lib types::`
Expected: the build fails. The errors include E0599 (no variant or associated item named `NeedsHuman` for enum `Outcome`) and unresolved `StopReason`, `Stop` and `MapEvidence`.

- [ ] **Step 3: Add the outcomes and the stop**

In `crates/harness-protocol/src/types.rs`, replace the definition of `Outcome` (derive, serde attribute and body) with:

```rust
/// How a unit ended: one of five fixed values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    PrOpen,
    NoChange,
    Failed,
    /// The unit stopped for a human; `UnitResult.stop` says why.
    NeedsHuman,
    /// A `draft` or `adopt` unit finished; `UnitResult.evidence` names its bundle.
    DraftReady,
}

/// Why a unit stopped for a human.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    ScopeRequest,
    SpecConflict,
    CheckUnrunnable,
    EscalationExhausted,
    HoldoutRoundsExhausted,
    ReviewRoundsExhausted,
    HoldoutDefect,
    MapUntrusted,
    BaselineRed,
    RuntimeUnavailable,
    BudgetExhausted,
}

/// Why a `needs_human` unit stopped. For that outcome it plays the part `Failure` plays for
/// `failed`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Stop {
    pub reason: StopReason,
    pub detail: String,
    /// For `scope_request`: the files the builder asked for.
    #[serde(default)]
    pub request: Vec<String>,
}
```

- [ ] **Step 4: Add the evidence types**

Replace the three definitions `Evidence`, `Failure` and `UnitResult` (from the doc comment above `Evidence` to the closing brace of `UnitResult`) with:

```rust
/// The repository map the unit was built against.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MapEvidence {
    pub base_sha: String,
    pub hash: String,
    pub drift: f64,
    pub eidos_level: String,
}

/// The test ids the harness saw pass. The control plane runs the tests again and never trusts this.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TestReport {
    pub ids_passed: Vec<String>,
}

/// How a control came out. `Unrunnable` means it could not be evaluated, which is not success.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ControlStatus {
    Passed,
    Failed,
    Unrunnable,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ControlResult {
    pub name: ControlKind,
    pub status: ControlStatus,
    pub detail: String,
}

/// A reviewer's finding on one invariant or criterion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Holds,
    Violated,
    CannotDetermine,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ReviewVerdict {
    /// The invariant or criterion id the verdict is about.
    pub subject: String,
    pub verdict: Verdict,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
}

/// `rounds` counts from 1 in this process; `prior_rounds` is the total from earlier processes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ReviewEvidence {
    pub rounds: u32,
    pub prior_rounds: u32,
    pub verdicts: Vec<ReviewVerdict>,
}

/// Claims the control plane re-reads from the sinks; never trusted as reported (spec §4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Evidence {
    pub branch: String,
    pub head_sha: String,
    pub delivery: DeliveryEvidence,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pr: Option<PrRef>,
    pub test: TestRun,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oracle_hash: Option<String>,
    /// SHA-256 of the spec bytes committed at the head.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spec_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub map: Option<MapEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub test_report: Option<TestReport>,
    #[serde(default)]
    pub controls: Vec<ControlResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review: Option<ReviewEvidence>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Failure {
    pub scope: ErrorScope,
    pub detail: String,
}

/// `unit/result` params — the last message a harness sends.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct UnitResult {
    pub outcome: Outcome,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<Evidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure: Option<Failure>,
    /// Required with `needs_human`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop: Option<Stop>,
}
```

- [ ] **Step 5: Run the protocol crate's unit tests**

Run: `cargo test -p harness-protocol --lib`
Expected: `test result: ok. 31 passed; 0 failed`.

- [ ] **Step 6: Update the five literals in `harness-fake`**

```diff
--- a/crates/harness-conformance/src/bin/harness-fake.rs
+++ b/crates/harness-conformance/src/bin/harness-fake.rs
@@ -222,6 +222,7 @@ fn run_unit(mode: Mode, order: &WorkOrder, rx: &Receiver<RpcMessage>) {
                 scope: ErrorScope::Agent,
                 detail: "nothing to do".into(),
             }),
+            stop: None,
         };
         send(&RpcMessage::notification(method::UNIT_RESULT, &result));
         return;
@@ -237,6 +238,7 @@ fn run_unit(mode: Mode, order: &WorkOrder, rx: &Receiver<RpcMessage>) {
                 scope: ErrorScope::Agent,
                 detail: "failed before gate".into(),
             }),
+            stop: None,
         };
         send(&RpcMessage::notification(method::UNIT_RESULT, &result));
         return;
@@ -282,6 +284,7 @@ fn run_unit(mode: Mode, order: &WorkOrder, rx: &Receiver<RpcMessage>) {
                         scope: ErrorScope::Agent,
                         detail: "oracle rejected".into(),
                     }),
+                    stop: None,
                 };
                 send(&RpcMessage::notification(method::UNIT_RESULT, &result));
                 return;
@@ -335,8 +338,14 @@ fn run_unit(mode: Mode, order: &WorkOrder, rx: &Receiver<RpcMessage>) {
                 .tier
                 .requires_oracle()
                 .then(|| "fake-oracle-hash".to_string()),
+            spec_hash: None,
+            map: None,
+            test_report: None,
+            controls: Vec::new(),
+            review: None,
         }),
         failure: None,
+        stop: None,
     };
     send(&RpcMessage::notification(method::UNIT_RESULT, &result));
 
```

Run: `cargo test -p harness-conformance`
Expected: 24 passed, 0 failed.

- [ ] **Step 7: Re-bless the contract and re-pin its hash**

```bash
HARNESS_PROTOCOL_BLESS=1 cargo test -p harness-protocol --test contract
cargo test -p harness-protocol --test contract
```

Expected from the second command: `committed_contract_hash_is_pinned ... FAILED` with `left: "9645ff197dde95906431461bc4a2e6631d0923b0258ccb385583bd29997cabbc"`. Set `CONTRACT_SHA256` to that value and run the second command again. Expected: `3 passed`.

No later task changes a wire type, so this is the contract hash of protocol 0.2. It goes in the pull request body.

After this task the whole list in `schema_defines_every_wire_type` reads:

```rust
#[test]
fn schema_defines_every_wire_type() {
    let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
    let defs = schema["$defs"].as_object().expect("schema has $defs");
    for name in [
        "RpcMessage",
        "RpcError",
        "InitializeParams",
        "InitializeResult",
        "Capabilities",
        "Network",
        "ControlKind",
        "UnitKind",
        "ProfileInfo",
        "PresetInfo",
        "WorkOrder",
        "SpecRef",
        "Source",
        "Scope",
        "RepoCommands",
        "RepoConfig",
        "Controls",
        "Parent",
        "UnitEvent",
        "Observation",
        "OracleFreeze",
        "FrozenFile",
        "Stage",
        "StageStatus",
        "CostBasis",
        "GateRequest",
        "GateReply",
        "UnitResult",
        "Outcome",
        "Stop",
        "StopReason",
        "Evidence",
        "MapEvidence",
        "TestReport",
        "ControlResult",
        "ControlStatus",
        "ReviewEvidence",
        "ReviewVerdict",
        "Verdict",
        "Empty",
    ] {
        assert!(defs.contains_key(name), "schema is missing $defs.{name}");
    }
}
```

- [ ] **Step 8: Commit**

```bash
cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings
git add crates/harness-protocol/src/types.rs crates/harness-protocol/tests/contract.rs crates/harness-protocol/contract/harness-protocol.contract.json crates/harness-conformance/src/bin/harness-fake.rs
git commit -m "feat(harness-protocol): needs_human and draft_ready outcomes, stop reasons, 0.2 evidence"
```

### Task 5: Hashing frozen test files

**Files:**
- Create: `crates/harness-protocol/src/hash.rs`
- Modify: `crates/harness-protocol/src/lib.rs`
- Test: `crates/harness-protocol/src/hash.rs` (inline)

**Interfaces:**
- Consumes: `FrozenFile` from Task 3; `sha2` as a normal dependency (request R1).
- Produces:
  - `pub fn file_sha256(bytes: &[u8]) -> String`
  - `pub fn bundle_hash(files: &[FrozenFile]) -> String`

  Both are re-exported from the crate root. They live in `hash.rs`, not in `types.rs`, so the wire types stay free of logic.

- [ ] **Step 1: Check the dependency is present**

Run: `cargo tree -p harness-protocol -e normal --depth 1`
Expected: four dependency lines, one of them `sha2 v0.10.9`. If `sha2` is missing, request R1 has not landed: stop and report to the coordinator.

- [ ] **Step 2: Write the failing tests**

Create `crates/harness-protocol/src/hash.rs` with only the imports and the tests:

```rust
use crate::types::FrozenFile;

#[cfg(test)]
mod tests {
    use super::*;

    const SHA_ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    const SHA_EMPTY: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    fn file(path: &str, sha256: &str) -> FrozenFile {
        FrozenFile {
            path: path.into(),
            sha256: sha256.into(),
        }
    }

    #[test]
    fn file_sha256_matches_the_published_vectors() {
        assert_eq!(file_sha256(b"abc"), SHA_ABC);
        assert_eq!(file_sha256(b""), SHA_EMPTY);
    }

    #[test]
    fn bundle_hash_matches_a_vector_computed_with_sha256sum() {
        // printf 'src/a.rs\0<SHA_ABC>\ntests/b.rs\0<SHA_EMPTY>\n' | sha256sum
        let files = [file("src/a.rs", SHA_ABC), file("tests/b.rs", SHA_EMPTY)];
        assert_eq!(
            bundle_hash(&files),
            "25957e705d3624094a2881e8dcbde7385b6d1fc352ad9f3861cebb95783c2fb7"
        );
    }

    #[test]
    fn bundle_hash_of_no_files_is_the_hash_of_nothing() {
        assert_eq!(bundle_hash(&[]), SHA_EMPTY);
    }

    #[test]
    fn bundle_hash_ignores_the_order_files_arrive_in() {
        let forward = [file("src/a.rs", SHA_ABC), file("tests/b.rs", SHA_EMPTY)];
        let reversed = [file("tests/b.rs", SHA_EMPTY), file("src/a.rs", SHA_ABC)];
        assert_eq!(bundle_hash(&forward), bundle_hash(&reversed));
    }

    #[test]
    fn bundle_hash_sorts_bytewise_so_uppercase_and_punctuation_have_one_order() {
        // Bytewise: 'B' (0x42) < 'a' (0x61), and "a-b" < "a/b" because '-' (0x2d) < '/' (0x2f).
        let mixed = [
            file("a/b", SHA_ABC),
            file("a-b", SHA_ABC),
            file("B", SHA_ABC),
        ];
        let sorted = [
            file("B", SHA_ABC),
            file("a-b", SHA_ABC),
            file("a/b", SHA_ABC),
        ];
        assert_eq!(bundle_hash(&mixed), bundle_hash(&sorted));
    }

    #[test]
    fn bundle_hash_does_not_normalise_path_separators_or_case() {
        let forward = [file("tests/a.rs", SHA_ABC)];
        let backward = [file(r"tests\a.rs", SHA_ABC)];
        let upper = [file("Tests/a.rs", SHA_ABC)];
        assert_ne!(bundle_hash(&forward), bundle_hash(&backward));
        assert_ne!(bundle_hash(&forward), bundle_hash(&upper));
    }

    #[test]
    fn bundle_hash_changes_when_a_file_hash_or_a_path_changes() {
        let base = [file("src/a.rs", SHA_ABC)];
        assert_ne!(
            bundle_hash(&base),
            bundle_hash(&[file("src/a.rs", SHA_EMPTY)])
        );
        assert_ne!(
            bundle_hash(&base),
            bundle_hash(&[file("src/b.rs", SHA_ABC)])
        );
        assert_ne!(bundle_hash(&base), bundle_hash(&[]));
    }
}
```

The vector in `bundle_hash_matches_a_vector_computed_with_sha256sum` was computed with `sha256sum`, not with this code.

Register the module in `crates/harness-protocol/src/lib.rs`:

```diff
--- a/crates/harness-protocol/src/lib.rs
+++ b/crates/harness-protocol/src/lib.rs
@@ -6,11 +6,13 @@
 //! `docs/specs/2026-09-14-swappable-harness-design.md` §3.
 
 mod codec;
+mod hash;
 mod rpc;
 mod schema;
 mod types;
 
 pub use codec::{read_message, write_message, ReadError};
+pub use hash::{bundle_hash, file_sha256};
 pub use rpc::{
     error_code, method, negotiate, version_compatible, MessageKind, RpcError, RpcMessage,
 };
```

- [ ] **Step 3: Run the tests and watch them fail**

Run: `cargo test -p harness-protocol --lib hash::`
Expected: the build fails with E0432 (unresolved imports `hash::bundle_hash`, `hash::file_sha256`) and E0425 (cannot find function `file_sha256`).

- [ ] **Step 4: Implement the two functions**

Replace the first line of `crates/harness-protocol/src/hash.rs` (the `use crate::types::FrozenFile;` line) with:

```rust
//! How frozen test files are hashed. Both sides of the protocol must get the same digest from
//! the same files, so the rule lives in this crate.

use crate::types::FrozenFile;
use sha2::{Digest, Sha256};

/// SHA-256 of `bytes`, as 64 lowercase hex characters.
pub fn file_sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// One digest for a set of files. Each file contributes a record: its path, a NUL byte, its hex
/// digest and a newline (`<path>\0<sha256>\n`). Records are taken in bytewise path order, whatever
/// order `files` is in, and SHA-256 is run over all of them. Paths are used exactly as given, so
/// callers pass paths from the repository root with `/` separators.
pub fn bundle_hash(files: &[FrozenFile]) -> String {
    let mut sorted: Vec<&FrozenFile> = files.iter().collect();
    sorted.sort_by(|a, b| (&a.path, &a.sha256).cmp(&(&b.path, &b.sha256)));
    let mut hasher = Sha256::new();
    for file in sorted {
        hasher.update(file.path.as_bytes());
        hasher.update([0u8]);
        hasher.update(file.sha256.as_bytes());
        hasher.update(b"\n");
    }
    format!("{:x}", hasher.finalize())
}
```

- [ ] **Step 5: Run the tests**

Run: `cargo test -p harness-protocol --lib hash::`
Expected: `test result: ok. 7 passed; 0 failed`.

Run: `cargo test -p harness-protocol`
Expected: 38 unit tests and 3 contract tests pass. The schema did not change, so there is no re-bless.

- [ ] **Step 6: Commit**

```bash
cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings
git add crates/harness-protocol/src/hash.rs crates/harness-protocol/src/lib.rs
git commit -m "feat(harness-protocol): file_sha256 and bundle_hash for frozen test files"
```

### Task 6: A line limit and honest UTF-8 errors in the codec

Parked SP-1 defect 2 (#82). Today a line that is not UTF-8 makes `read_line` return an I/O error, the kit's reader thread stops, and the kit reports the harness as having exited.

**Files:**
- Modify: `crates/harness-protocol/src/codec.rs`, `crates/harness-protocol/src/lib.rs`
- Modify: `crates/harness-conformance/src/session.rs`, `crates/harness-conformance/src/bin/harness-fake.rs`
- Create: `crates/harness-conformance/tests/detects_v02.rs`
- Test: `crates/harness-protocol/src/codec.rs` (inline), `crates/harness-conformance/tests/detects_v02.rs`

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces:
  - `pub const MAX_LINE_BYTES: usize = 4 * 1024 * 1024;`
  - `pub enum ReadError { Eof, Malformed { line: String, error: String }, InvalidUtf8 { line: String }, LineTooLong { limit: usize }, Io(io::Error) }`
  - `read_message` and `write_message` keep their signatures. `write_message` returns `io::ErrorKind::InvalidInput` for a message over the limit.
  - In the kit, both new errors surface as `Violation::Malformed { line }`, with `line` set to `invalid UTF-8: <lossy text>` or `line longer than 4194304 bytes`.
  - `harness-fake` modes `invalid_utf8` and `long_line`.

- [ ] **Step 1: Write the failing codec tests**

In `crates/harness-protocol/src/codec.rs`, replace the whole `#[cfg(test)] mod tests { … }` block with:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{method, Empty, RpcMessage};
    use std::io::Cursor;

    #[test]
    fn write_then_read_round_trips_and_skips_blank_lines() {
        let mut buf = Vec::new();
        write_message(
            &mut buf,
            &RpcMessage::notification(method::UNIT_EVENT, &Empty {}),
        )
        .unwrap();
        buf.extend_from_slice(b"\n\n");
        write_message(&mut buf, &RpcMessage::response(3, &Empty {})).unwrap();
        assert_eq!(buf.iter().filter(|b| **b == b'\n').count(), 4);

        let mut r = Cursor::new(buf);
        assert_eq!(
            read_message(&mut r).unwrap().method.as_deref(),
            Some("unit/event")
        );
        assert_eq!(read_message(&mut r).unwrap().id, Some(3));
        assert!(matches!(read_message(&mut r), Err(ReadError::Eof)));
    }

    #[test]
    fn a_non_json_line_is_malformed_and_keeps_the_raw_line() {
        let mut r = Cursor::new(b"this is not json\n".to_vec());
        match read_message(&mut r) {
            Err(ReadError::Malformed { line, .. }) => assert_eq!(line, "this is not json"),
            other => panic!("expected Malformed, got {other:?}"),
        }
    }

    /// A valid one-line message of exactly `len` bytes.
    fn message_of(len: usize) -> Vec<u8> {
        let empty = serde_json::to_string(&RpcMessage::notification("m", &"")).unwrap();
        let pad = "x".repeat(len - empty.len());
        let line = serde_json::to_string(&RpcMessage::notification("m", &pad)).unwrap();
        assert_eq!(line.len(), len);
        line.into_bytes()
    }

    #[test]
    fn a_line_at_the_limit_is_read_and_one_byte_more_is_too_long() {
        let mut at_limit = message_of(64);
        at_limit.push(b'\n');
        let mut r = Cursor::new(at_limit);
        assert!(read_with_limit(&mut r, 64).is_ok());
        assert!(matches!(read_with_limit(&mut r, 64), Err(ReadError::Eof)));

        let mut over = message_of(65);
        over.push(b'\n');
        let mut r = Cursor::new(over);
        assert!(matches!(
            read_with_limit(&mut r, 64),
            Err(ReadError::LineTooLong { limit: 64 })
        ));
    }

    #[test]
    fn a_line_at_the_limit_with_no_final_newline_is_still_read() {
        let mut r = Cursor::new(message_of(64));
        assert!(read_with_limit(&mut r, 64).is_ok());
        let mut r = Cursor::new(message_of(65));
        assert!(matches!(
            read_with_limit(&mut r, 64),
            Err(ReadError::LineTooLong { limit: 64 })
        ));
    }

    #[test]
    fn the_public_reader_uses_the_four_mebibyte_limit() {
        assert_eq!(MAX_LINE_BYTES, 4 * 1024 * 1024);
        let mut r = Cursor::new(vec![b'x'; MAX_LINE_BYTES + 1]);
        assert!(matches!(
            read_message(&mut r),
            Err(ReadError::LineTooLong {
                limit: MAX_LINE_BYTES
            })
        ));
    }

    #[test]
    fn invalid_utf8_is_reported_as_such_and_the_next_line_is_still_readable() {
        let mut bytes = vec![b'c', b'a', b'f', 0xff, b'\n'];
        write_message(&mut bytes, &RpcMessage::response(9, &Empty {})).unwrap();
        let mut r = Cursor::new(bytes);
        match read_message(&mut r) {
            Err(ReadError::InvalidUtf8 { line }) => assert_eq!(line, "caf\u{fffd}"),
            other => panic!("expected InvalidUtf8, got {other:?}"),
        }
        assert_eq!(read_message(&mut r).unwrap().id, Some(9));
    }

    #[test]
    fn crlf_line_endings_are_accepted() {
        let mut bytes = serde_json::to_vec(&RpcMessage::response(4, &Empty {})).unwrap();
        bytes.extend_from_slice(b"\r\n");
        let mut r = Cursor::new(bytes);
        assert_eq!(read_message(&mut r).unwrap().id, Some(4));
    }

    #[test]
    fn a_message_over_the_limit_is_not_written() {
        let huge = RpcMessage::notification("m", &"x".repeat(MAX_LINE_BYTES));
        let mut out = Vec::new();
        let err = write_message(&mut out, &huge).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
        assert!(out.is_empty());
    }
}
```

- [ ] **Step 2: Run the tests and watch them fail**

Run: `cargo test -p harness-protocol --lib codec::`
Expected: the build fails. The errors include E0425 (cannot find function `read_with_limit`; cannot find value `MAX_LINE_BYTES`) and E0599 (no variant named `InvalidUtf8`).

- [ ] **Step 3: Implement the bounded reader**

In `crates/harness-protocol/src/codec.rs`, replace everything above `#[cfg(test)]` with:

```rust
//! Newline-delimited framing: exactly one JSON-RPC message per line.

use crate::rpc::RpcMessage;
use std::io::{self, BufRead, Read, Write};

/// The longest line either side may send: the bytes before the `\n`.
pub const MAX_LINE_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug)]
pub enum ReadError {
    /// The peer closed its end.
    Eof,
    /// A non-empty line that is not a JSON-RPC message. `line` is kept verbatim for the report.
    Malformed {
        line: String,
        error: String,
    },
    /// A line that is not UTF-8. `line` is its lossy rendering. The reader is at the next line.
    InvalidUtf8 {
        line: String,
    },
    /// A line longer than `limit` bytes. The reader is left inside that line, so stop reading.
    LineTooLong {
        limit: usize,
    },
    Io(io::Error),
}

/// Write `msg` as one line and flush, so the peer sees it immediately. A message longer than
/// `MAX_LINE_BYTES` is refused with `InvalidInput` and nothing is written.
pub fn write_message<W: Write>(w: &mut W, msg: &RpcMessage) -> io::Result<()> {
    let line = serde_json::to_string(msg).map_err(io::Error::other)?;
    if line.len() > MAX_LINE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "message is {} bytes; the line limit is {MAX_LINE_BYTES}",
                line.len()
            ),
        ));
    }
    w.write_all(line.as_bytes())?;
    w.write_all(b"\n")?;
    w.flush()
}

/// Read the next message, skipping blank lines.
pub fn read_message<R: BufRead>(r: &mut R) -> Result<RpcMessage, ReadError> {
    read_with_limit(r, MAX_LINE_BYTES)
}

fn read_with_limit<R: BufRead>(r: &mut R, limit: usize) -> Result<RpcMessage, ReadError> {
    loop {
        let mut buf = Vec::new();
        // One byte past the limit is enough to tell "at the limit" from "too long".
        let mut bounded = (&mut *r).take(limit as u64 + 1);
        let n = bounded.read_until(b'\n', &mut buf).map_err(ReadError::Io)?;
        if n == 0 {
            return Err(ReadError::Eof);
        }
        if buf.last() != Some(&b'\n') && buf.len() > limit {
            return Err(ReadError::LineTooLong { limit });
        }
        let line = match String::from_utf8(buf) {
            Ok(line) => line,
            Err(e) => {
                return Err(ReadError::InvalidUtf8 {
                    line: String::from_utf8_lossy(e.as_bytes()).trim().to_string(),
                });
            }
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        return serde_json::from_str(trimmed).map_err(|e| ReadError::Malformed {
            line: trimmed.to_string(),
            error: e.to_string(),
        });
    }
}
```

Export the constant from `crates/harness-protocol/src/lib.rs`:

```diff
--- a/crates/harness-protocol/src/lib.rs
+++ b/crates/harness-protocol/src/lib.rs
@@ -11,7 +11,7 @@ mod rpc;
 mod schema;
 mod types;
 
-pub use codec::{read_message, write_message, ReadError};
+pub use codec::{read_message, write_message, ReadError, MAX_LINE_BYTES};
 pub use hash::{bundle_hash, file_sha256};
 pub use rpc::{
     error_code, method, negotiate, version_compatible, MessageKind, RpcError, RpcMessage,
```

`ReadError` has two new variants, so the kit's reader no longer compiles. For this step only, keep its old behaviour: in `crates/harness-conformance/src/session.rs`, replace the arm `Err(ReadError::Eof) | Err(ReadError::Io(_)) => break,` with

```rust
                    Err(ReadError::Eof)
                    | Err(ReadError::Io(_))
                    | Err(ReadError::InvalidUtf8 { .. })
                    | Err(ReadError::LineTooLong { .. }) => break,
```

- [ ] **Step 4: Run the codec tests**

Run: `cargo test -p harness-protocol --lib codec::`
Expected: `test result: ok. 8 passed; 0 failed`.

- [ ] **Step 5: Write the failing kit tests**

Give `harness-fake` two modes that send the two kinds of bad line:

```diff
--- a/crates/harness-conformance/src/bin/harness-fake.rs
+++ b/crates/harness-conformance/src/bin/harness-fake.rs
@@ -6,7 +6,8 @@ use harness_protocol::{
     error_code, method, negotiate, read_message, write_message, ArtifactKind, Capabilities,
     Delivery, DeliveryEvidence, Empty, ErrorScope, Evidence, Failure, GateKind, GateReply,
     GateRequest, HarnessInfo, InitializeParams, InitializeResult, Isolation, MessageKind, Metering,
-    Observation, Outcome, RpcMessage, TestRun, UnitEvent, UnitResult, WorkOrder, PROTOCOL_VERSION,
+    Observation, Outcome, RpcMessage, TestRun, UnitEvent, UnitResult, WorkOrder, MAX_LINE_BYTES,
+    PROTOCOL_VERSION,
 };
 use std::io::{self, BufReader, Write};
 use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
@@ -29,6 +30,8 @@ enum Mode {
     AckHaltLinger,
     SilentHaltExit,
     ResultWithoutEvents,
+    InvalidUtf8,
+    LongLine,
 }
 
 fn mode() -> Mode {
@@ -47,6 +50,8 @@ fn mode() -> Mode {
         Ok("skip_gate") => Mode::SkipGate,
         Ok("ignore_gate_rejection") => Mode::IgnoreGateRejection,
         Ok("ignore_halt") => Mode::IgnoreHalt,
+        Ok("invalid_utf8") => Mode::InvalidUtf8,
+        Ok("long_line") => Mode::LongLine,
         _ => Mode::Conformant,
     }
 }
@@ -257,6 +262,17 @@ fn run_unit(mode: Mode, order: &WorkOrder, rx: &Receiver<RpcMessage>) {
             let _ = out.flush();
         }
         Mode::UnknownMethod => send(&RpcMessage::notification("unit/whatever", &Empty {})),
+        Mode::InvalidUtf8 => {
+            let mut out = io::stdout().lock();
+            let _ = out.write_all(&[0xff, 0xfe, b'\n']);
+            let _ = out.flush();
+        }
+        Mode::LongLine => {
+            let mut out = io::stdout().lock();
+            let _ = out.write_all(&vec![b'x'; MAX_LINE_BYTES + 1]);
+            let _ = out.write_all(b"\n");
+            let _ = out.flush();
+        }
         _ => {}
     }
 
```

Create `crates/harness-conformance/tests/detects_v02.rs`:

```rust
//! Protocol 0.2 and the three defects parked at the end of SP-1. Each misbehaving `harness-fake`
//! mode must be caught by the case that owns the rule. `detects_violations.rs` holds the 0.1
//! rules and is not edited by this work.

use harness_conformance::{run_all, CaseOutcome, CaseReport, KitConfig, Violation};
use std::time::Duration;

fn fake_config(mode: &str) -> KitConfig {
    let mut cfg = KitConfig::new(vec![env!("CARGO_BIN_EXE_harness-fake").to_string()]);
    cfg.env = vec![
        ("HARNESS_FAKE_MODE".into(), mode.into()),
        ("HARNESS_FAKE_STEP_MS".into(), "20".into()),
    ];
    cfg.wall_clock = Duration::from_secs(5);
    cfg.grace = Duration::from_secs(1);
    cfg
}

fn outcome_of<'a>(reports: &'a [CaseReport], case: &str) -> &'a CaseOutcome {
    &reports
        .iter()
        .find(|r| r.name == case)
        .expect("case exists in run_all")
        .outcome
}

fn assert_detects(mode: &str, case: &str, want: Violation) {
    let reports = run_all(&fake_config(mode));
    assert_eq!(
        *outcome_of(&reports, case),
        CaseOutcome::Fail(want),
        "mode `{mode}`, case `{case}`; all reports: {reports:#?}"
    );
}

#[test]
fn invalid_utf8_is_reported_as_invalid_utf8_not_as_an_exit() {
    assert_detects(
        "invalid_utf8",
        "happy_path_t1",
        Violation::Malformed {
            line: "invalid UTF-8: \u{fffd}\u{fffd}".into(),
        },
    );
}

#[test]
fn a_line_over_the_limit_is_reported_with_the_limit() {
    assert_detects(
        "long_line",
        "happy_path_t1",
        Violation::Malformed {
            line: "line longer than 4194304 bytes".into(),
        },
    );
}
```

- [ ] **Step 6: Run the kit tests and watch them fail**

Run: `cargo test -p harness-conformance --test detects_v02`
Expected: both tests fail, each with `left: Fail(ExitedWithoutResult)`. That is the defect: the kit says the harness exited.

- [ ] **Step 7: Report both errors from the kit's reader**

In `crates/harness-conformance/src/session.rs`, replace the `let item = match read_message(&mut reader) { … };` statement with:

```rust
                let item = match read_message(&mut reader) {
                    Ok(msg) => Incoming::Message(msg),
                    Err(ReadError::Malformed { line, .. }) => Incoming::Malformed(line),
                    Err(ReadError::InvalidUtf8 { line }) => {
                        Incoming::Malformed(format!("invalid UTF-8: {line}"))
                    }
                    Err(ReadError::LineTooLong { limit }) => {
                        // The reader is inside the over-long line, so nothing after it can be
                        // trusted: report it and stop.
                        let _ = tx.send(Incoming::Malformed(format!(
                            "line longer than {limit} bytes"
                        )));
                        break;
                    }
                    Err(ReadError::Eof) | Err(ReadError::Io(_)) => break,
                };
```

- [ ] **Step 8: Run the kit tests**

Run: `cargo test -p harness-conformance --test detects_v02`
Expected: `test result: ok. 2 passed; 0 failed`.

Run: `cargo test -p harness-conformance`
Expected: 26 passed, 0 failed.

- [ ] **Step 9: Commit**

```bash
cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings
git add crates/harness-protocol/src/codec.rs crates/harness-protocol/src/lib.rs crates/harness-conformance/src/session.rs crates/harness-conformance/src/bin/harness-fake.rs crates/harness-conformance/tests/detects_v02.rs
git commit -m "fix(harness-protocol): bound line length and report invalid UTF-8 as such, not as EOF"
```

### Task 7: The protocol monitor

**Files:**
- Create: `crates/harness-protocol/src/monitor.rs`
- Modify: `crates/harness-protocol/src/lib.rs`
- Test: `crates/harness-protocol/src/monitor.rs` (inline)

**Interfaces:**
- Consumes: `Capabilities`, `GateRequest`, `Metering`, `Outcome`, `UnitEvent`, `UnitResult` from Tasks 2 to 4; `RpcMessage`, `MessageKind`, `method`.
- Produces, in the public module `harness_protocol::monitor`:
  - `pub struct ProtocolMonitor` with `pub fn new(capabilities: Capabilities, start_id: u64) -> Self`, `pub fn interrupt_sent(&mut self, id: u64, method: &str)`, `pub fn on_line(&mut self, line: Result<RpcMessage, String>) -> Result<Inbound, MonitorViolation>`
  - `pub enum Inbound { StartAck, Event(UnitEvent), GateRequest { id: u64, request: GateRequest }, InterruptAck { method: String }, InterruptRefused { method: String }, Result(UnitResult) }`
  - `pub enum MonitorViolation { Malformed { line: String }, InvalidMessage, UnknownMethod { method: String }, InvalidParams { method: String, detail: String }, InvalidResult(String), MessageAfterResult { method: String }, MeteringDeclaredButSilent }`

What the monitor decides and what it leaves to its caller:

| The monitor decides | The caller decides |
|---|---|
| What a line is: the start reply, an event, a gate request, the reply to an interrupt, a result | Every deadline |
| Whether a result carries what its outcome needs: `evidence` for `pr_open`, `no_change` and `draft_ready`; `failure` for `failed`; `stop` for `needs_human` (parked defect 3, and the two 0.2 cases) | Whether `unit/start` was answered first and in time |
| Whether a harness that declared `metering: usd` reported spend before a `pr_open`, `no_change` or `draft_ready` result. `failed` and `needs_human` are exempt: such a unit can end before any agent ran | What a result that arrives after an interrupt means |
| That nothing follows a result | Phases, and the order of observations |

- [ ] **Step 1: Write the failing tests**

Create `crates/harness-protocol/src/monitor.rs` with the imports and the test module:

```rust
use crate::rpc::{method, MessageKind, RpcMessage};
use crate::types::{Capabilities, GateRequest, Metering, Outcome, UnitEvent, UnitResult};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        error_code, Delivery, DeliveryEvidence, Empty, ErrorScope, Evidence, Failure, GateKind,
        Isolation, Observation, Stage, StageStatus, Stop, StopReason, TestRun,
    };
    use serde_json::json;

    const START_ID: u64 = 2;
    const INTERRUPT_ID: u64 = 3;

    fn caps(metering: Metering) -> Capabilities {
        Capabilities {
            isolation: Isolation::Container,
            metering,
            gates: vec![GateKind::Oracle],
            delivery: Delivery::Bundle,
            resume: true,
            halt: true,
            holdouts: false,
            controls: Vec::new(),
            network: None,
            profiles: Vec::new(),
            kinds: Vec::new(),
            presets: Vec::new(),
        }
    }

    fn monitor() -> ProtocolMonitor {
        ProtocolMonitor::new(caps(Metering::None), START_ID)
    }

    fn event(ev: &UnitEvent) -> Result<RpcMessage, String> {
        Ok(RpcMessage::notification(method::UNIT_EVENT, ev))
    }

    fn metric() -> UnitEvent {
        UnitEvent::Metric {
            tokens_in: 1,
            tokens_out: 1,
            cost_usd: 0.01,
            elapsed_ms: 1,
            cost_basis: None,
            stage: None,
            role: None,
            adapter: None,
            model: None,
        }
    }

    fn evidence() -> Evidence {
        Evidence {
            branch: "agent/u1".into(),
            head_sha: "0123".into(),
            delivery: DeliveryEvidence::Bundle {
                bundle_path: "u1.bundle".into(),
            },
            pr: None,
            test: TestRun {
                command: "true".into(),
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

    fn result(outcome: Outcome) -> UnitResult {
        UnitResult {
            outcome,
            evidence: None,
            failure: None,
            stop: None,
        }
    }

    fn result_line(result: &UnitResult) -> Result<RpcMessage, String> {
        Ok(RpcMessage::notification(method::UNIT_RESULT, result))
    }

    // --- one test per `Inbound` ---

    #[test]
    fn the_reply_to_unit_start_is_start_ack() {
        let line = Ok(RpcMessage::response(START_ID, &Empty {}));
        assert_eq!(monitor().on_line(line), Ok(Inbound::StartAck));
    }

    #[test]
    fn a_unit_event_is_returned_as_the_event_it_carries() {
        let ev = UnitEvent::Observed {
            observation: Observation::Provisioned,
        };
        assert_eq!(monitor().on_line(event(&ev)), Ok(Inbound::Event(ev)));
    }

    #[test]
    fn a_stage_event_is_an_ordinary_event() {
        let ev = UnitEvent::Stage {
            stage: Stage::Check,
            status: StageStatus::Finished,
            detail: None,
        };
        assert_eq!(monitor().on_line(event(&ev)), Ok(Inbound::Event(ev)));
    }

    #[test]
    fn a_gate_request_is_returned_with_its_id() {
        let request = GateRequest::Oracle {
            test_files: vec!["tests/a.rs".into()],
            hash: "h".into(),
            summary: "s".into(),
            holdout_files: Vec::new(),
            holdout_hash: None,
        };
        let line = Ok(RpcMessage::request(1_000, method::GATE_REQUEST, &request));
        assert_eq!(
            monitor().on_line(line),
            Ok(Inbound::GateRequest { id: 1_000, request })
        );
    }

    #[test]
    fn a_reply_to_the_pending_interrupt_is_interrupt_ack() {
        let mut m = monitor();
        m.interrupt_sent(INTERRUPT_ID, method::UNIT_HALT);
        let line = Ok(RpcMessage::response(INTERRUPT_ID, &Empty {}));
        assert_eq!(
            m.on_line(line),
            Ok(Inbound::InterruptAck {
                method: "unit/halt".into()
            })
        );
    }

    #[test]
    fn an_error_reply_to_the_pending_interrupt_is_interrupt_refused() {
        let mut m = monitor();
        m.interrupt_sent(INTERRUPT_ID, method::UNIT_ABANDON);
        let line = Ok(RpcMessage::error(
            INTERRUPT_ID,
            error_code::METHOD_NOT_FOUND,
            "no",
        ));
        assert_eq!(
            m.on_line(line),
            Ok(Inbound::InterruptRefused {
                method: "unit/abandon".into()
            })
        );
    }

    #[test]
    fn a_well_formed_result_is_returned_for_every_outcome() {
        let mut pr_open = result(Outcome::PrOpen);
        pr_open.evidence = Some(evidence());
        let mut no_change = result(Outcome::NoChange);
        no_change.evidence = Some(evidence());
        let mut draft_ready = result(Outcome::DraftReady);
        draft_ready.evidence = Some(evidence());
        let mut failed = result(Outcome::Failed);
        failed.failure = Some(Failure {
            scope: ErrorScope::Agent,
            detail: "d".into(),
        });
        let mut needs_human = result(Outcome::NeedsHuman);
        needs_human.stop = Some(Stop {
            reason: StopReason::BaselineRed,
            detail: "d".into(),
            request: Vec::new(),
        });
        for r in [pr_open, no_change, draft_ready, failed, needs_human] {
            assert_eq!(monitor().on_line(result_line(&r)), Ok(Inbound::Result(r)));
        }
    }

    #[test]
    fn a_result_after_an_interrupt_is_still_a_result() {
        // What it means is the caller's decision: the kit fails the case, the supervisor drops it.
        let mut m = monitor();
        m.interrupt_sent(INTERRUPT_ID, method::UNIT_HALT);
        let mut failed = result(Outcome::Failed);
        failed.failure = Some(Failure {
            scope: ErrorScope::Agent,
            detail: "d".into(),
        });
        assert_eq!(m.on_line(result_line(&failed)), Ok(Inbound::Result(failed)));
    }

    // --- one test per `MonitorViolation` ---

    #[test]
    fn an_unparsed_line_is_malformed_and_keeps_the_line() {
        assert_eq!(
            monitor().on_line(Err("this is not json".into())),
            Err(MonitorViolation::Malformed {
                line: "this is not json".into()
            })
        );
    }

    #[test]
    fn a_message_that_is_neither_request_nor_reply_is_invalid() {
        let mut both = RpcMessage::response(START_ID, &Empty {});
        both.method = Some(method::UNIT_EVENT.into());
        assert_eq!(
            monitor().on_line(Ok(both)),
            Err(MonitorViolation::InvalidMessage)
        );
    }

    #[test]
    fn a_reply_nobody_is_waiting_for_is_invalid() {
        // No interrupt was sent, so neither a reply nor an error with that id is expected.
        let reply = Ok(RpcMessage::response(INTERRUPT_ID, &Empty {}));
        assert_eq!(
            monitor().on_line(reply),
            Err(MonitorViolation::InvalidMessage)
        );
        let error = Ok(RpcMessage::error(
            START_ID,
            error_code::INVALID_PARAMS,
            "no",
        ));
        assert_eq!(
            monitor().on_line(error),
            Err(MonitorViolation::InvalidMessage)
        );
    }

    #[test]
    fn a_method_outside_the_protocol_is_unknown() {
        let note = Ok(RpcMessage::notification("unit/whatever", &Empty {}));
        assert_eq!(
            monitor().on_line(note),
            Err(MonitorViolation::UnknownMethod {
                method: "unit/whatever".into()
            })
        );
        // `unit/result` is a notification. Sent as a request, it is not the protocol's message.
        let mut failed = result(Outcome::Failed);
        failed.failure = Some(Failure {
            scope: ErrorScope::Agent,
            detail: "d".into(),
        });
        let as_request = Ok(RpcMessage::request(9, method::UNIT_RESULT, &failed));
        assert_eq!(
            monitor().on_line(as_request),
            Err(MonitorViolation::UnknownMethod {
                method: "unit/result".into()
            })
        );
    }

    #[test]
    fn params_that_do_not_parse_name_the_method() {
        for m in [method::UNIT_EVENT, method::UNIT_RESULT] {
            let line = Ok(RpcMessage::notification(m, &json!({"type": "nope"})));
            match monitor().on_line(line) {
                Err(MonitorViolation::InvalidParams { method, detail }) => {
                    assert_eq!(method, m);
                    assert!(!detail.is_empty());
                }
                other => panic!("expected InvalidParams for {m}, got {other:?}"),
            }
        }
        let gate = Ok(RpcMessage::request(7, method::GATE_REQUEST, &json!({})));
        assert!(matches!(
            monitor().on_line(gate),
            Err(MonitorViolation::InvalidParams { method, .. }) if method == "gate/request"
        ));
    }

    #[test]
    fn each_outcome_is_refused_without_the_field_that_must_accompany_it() {
        for (outcome, want) in [
            (Outcome::PrOpen, "pr_open without evidence"),
            (Outcome::NoChange, "no_change without evidence"),
            (Outcome::DraftReady, "draft_ready without evidence"),
            (Outcome::Failed, "failed without failure"),
            (Outcome::NeedsHuman, "needs_human without stop"),
        ] {
            assert_eq!(
                monitor().on_line(result_line(&result(outcome))),
                Err(MonitorViolation::InvalidResult(want.into()))
            );
        }
    }

    #[test]
    fn anything_after_the_result_is_a_violation() {
        let mut m = monitor();
        let mut failed = result(Outcome::Failed);
        failed.failure = Some(Failure {
            scope: ErrorScope::Agent,
            detail: "d".into(),
        });
        m.on_line(result_line(&failed)).unwrap();
        assert_eq!(
            m.on_line(event(&metric())),
            Err(MonitorViolation::MessageAfterResult {
                method: "unit/event".into()
            })
        );
        assert_eq!(
            m.on_line(result_line(&failed)),
            Err(MonitorViolation::MessageAfterResult {
                method: "unit/result".into()
            })
        );
        assert_eq!(
            m.on_line(Ok(RpcMessage::response(START_ID, &Empty {}))),
            Err(MonitorViolation::MessageAfterResult {
                method: "<response>".into()
            })
        );
    }

    #[test]
    fn declared_metering_with_no_metric_is_refused_for_outcomes_that_claim_work() {
        for outcome in [Outcome::PrOpen, Outcome::NoChange, Outcome::DraftReady] {
            let mut r = result(outcome);
            r.evidence = Some(evidence());
            let mut silent = ProtocolMonitor::new(caps(Metering::Usd), START_ID);
            assert_eq!(
                silent.on_line(result_line(&r)),
                Err(MonitorViolation::MeteringDeclaredButSilent)
            );

            let mut metered = ProtocolMonitor::new(caps(Metering::Usd), START_ID);
            metered.on_line(event(&metric())).unwrap();
            assert_eq!(metered.on_line(result_line(&r)), Ok(Inbound::Result(r)));
        }
    }

    #[test]
    fn a_failed_or_stopped_unit_may_end_before_any_metric() {
        let mut failed = result(Outcome::Failed);
        failed.failure = Some(Failure {
            scope: ErrorScope::Agent,
            detail: "d".into(),
        });
        let mut stopped = result(Outcome::NeedsHuman);
        stopped.stop = Some(Stop {
            reason: StopReason::MapUntrusted,
            detail: "d".into(),
            request: Vec::new(),
        });
        for r in [failed, stopped] {
            let mut m = ProtocolMonitor::new(caps(Metering::Usd), START_ID);
            assert_eq!(m.on_line(result_line(&r)), Ok(Inbound::Result(r)));
        }
    }

    #[test]
    fn a_refused_result_does_not_close_the_stream() {
        // The violation is reported once; the monitor does not then claim a result was seen.
        let mut m = monitor();
        assert!(m.on_line(result_line(&result(Outcome::PrOpen))).is_err());
        assert_eq!(m.on_line(event(&metric())), Ok(Inbound::Event(metric())));
    }
}
```

Register the module in `crates/harness-protocol/src/lib.rs`:

```diff
--- a/crates/harness-protocol/src/lib.rs
+++ b/crates/harness-protocol/src/lib.rs
@@ -7,6 +7,7 @@
 
 mod codec;
 mod hash;
+pub mod monitor;
 mod rpc;
 mod schema;
 mod types;
```

- [ ] **Step 2: Run the tests and watch them fail**

Run: `cargo test -p harness-protocol --lib monitor::`
Expected: the build fails. The errors include E0412 (cannot find type `ProtocolMonitor`) and unresolved `Inbound` and `MonitorViolation`.

- [ ] **Step 3: Implement the monitor**

Replace the two `use` lines at the top of `crates/harness-protocol/src/monitor.rs` with:

```rust
//! A pure protocol monitor: it says what each line from a harness means and whether it is legal
//! on the wire. It owns no clock, no process and no phase, so a blocking caller (the conformance
//! kit) and an async one (the control plane's supervisor) share it.
//!
//! What it does not decide: deadlines; whether `unit/start` was answered in time; and what a
//! `unit/result` that arrives after an interrupt means. Those belong to the caller.

use crate::rpc::{method, MessageKind, RpcMessage};
use crate::types::{Capabilities, GateRequest, Metering, Outcome, UnitEvent, UnitResult};

/// One legal message from the harness.
// One `Inbound` exists at a time and is consumed at once, so the size of `Result` costs nothing
// worth an indirection in the API.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq)]
pub enum Inbound {
    /// The reply to `unit/start`.
    StartAck,
    Event(UnitEvent),
    GateRequest {
        id: u64,
        request: GateRequest,
    },
    /// The harness answered the pending interrupt.
    InterruptAck {
        method: String,
    },
    /// The harness answered the pending interrupt with an error. Not a violation by itself.
    InterruptRefused {
        method: String,
    },
    Result(UnitResult),
}

/// One way a line broke the protocol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MonitorViolation {
    /// Not a JSON-RPC message. `line` is what the caller's reader reported.
    Malformed {
        line: String,
    },
    /// A JSON-RPC message of no kind this protocol uses here: a reply nobody is waiting for.
    InvalidMessage,
    UnknownMethod {
        method: String,
    },
    InvalidParams {
        method: String,
        detail: String,
    },
    /// A `unit/result` whose outcome lacks the field that must accompany it.
    InvalidResult(String),
    MessageAfterResult {
        method: String,
    },
    MeteringDeclaredButSilent,
}

pub struct ProtocolMonitor {
    capabilities: Capabilities,
    start_id: u64,
    pending_interrupt: Option<(u64, String)>,
    metrics: usize,
    result_seen: bool,
}

fn validate_result(result: &UnitResult) -> Result<(), MonitorViolation> {
    let missing = match result.outcome {
        Outcome::PrOpen if result.evidence.is_none() => "pr_open without evidence",
        Outcome::NoChange if result.evidence.is_none() => "no_change without evidence",
        Outcome::DraftReady if result.evidence.is_none() => "draft_ready without evidence",
        Outcome::Failed if result.failure.is_none() => "failed without failure",
        Outcome::NeedsHuman if result.stop.is_none() => "needs_human without stop",
        _ => return Ok(()),
    };
    Err(MonitorViolation::InvalidResult(missing.into()))
}

/// A harness that meters in USD must have reported spend before it claims work. A `failed` or
/// `needs_human` unit may have ended before any agent ran.
fn must_have_metered(outcome: Outcome) -> bool {
    matches!(
        outcome,
        Outcome::PrOpen | Outcome::NoChange | Outcome::DraftReady
    )
}

impl ProtocolMonitor {
    /// `capabilities` are the ones this harness process returned from `initialize`; `start_id` is
    /// the id of the `unit/start` request.
    pub fn new(capabilities: Capabilities, start_id: u64) -> Self {
        Self {
            capabilities,
            start_id,
            pending_interrupt: None,
            metrics: 0,
            result_seen: false,
        }
    }

    /// Note an outbound interrupt (`unit/halt` | `unit/abandon`) so its reply classifies.
    pub fn interrupt_sent(&mut self, id: u64, method: &str) {
        self.pending_interrupt = Some((id, method.to_string()));
    }

    /// Classify the next line. `Err(line)` is a line the reader could not parse.
    pub fn on_line(
        &mut self,
        line: Result<RpcMessage, String>,
    ) -> Result<Inbound, MonitorViolation> {
        let msg = match line {
            Ok(msg) => msg,
            Err(line) => return Err(MonitorViolation::Malformed { line }),
        };
        if self.result_seen {
            let method = msg.method.clone().unwrap_or_else(|| "<response>".into());
            return Err(MonitorViolation::MessageAfterResult { method });
        }
        match msg.kind() {
            MessageKind::Response { id } if id == self.start_id => Ok(Inbound::StartAck),
            MessageKind::Response { id } => match &self.pending_interrupt {
                Some((pending, method)) if *pending == id => Ok(Inbound::InterruptAck {
                    method: method.clone(),
                }),
                _ => Err(MonitorViolation::InvalidMessage),
            },
            MessageKind::ErrorResponse { id, .. } => match &self.pending_interrupt {
                Some((pending, method)) if *pending == id => Ok(Inbound::InterruptRefused {
                    method: method.clone(),
                }),
                _ => Err(MonitorViolation::InvalidMessage),
            },
            MessageKind::Notification { method: m } if m == method::UNIT_EVENT => {
                let event: UnitEvent = msg.params_as().map_err(|e| invalid_params(m, e))?;
                if matches!(event, UnitEvent::Metric { .. }) {
                    self.metrics += 1;
                }
                Ok(Inbound::Event(event))
            }
            MessageKind::Notification { method: m } if m == method::UNIT_RESULT => {
                let result: UnitResult = msg.params_as().map_err(|e| invalid_params(m, e))?;
                validate_result(&result)?;
                if self.capabilities.metering == Metering::Usd
                    && self.metrics == 0
                    && must_have_metered(result.outcome)
                {
                    return Err(MonitorViolation::MeteringDeclaredButSilent);
                }
                self.result_seen = true;
                Ok(Inbound::Result(result))
            }
            MessageKind::Request { id, method: m } if m == method::GATE_REQUEST => {
                let request: GateRequest = msg.params_as().map_err(|e| invalid_params(m, e))?;
                Ok(Inbound::GateRequest { id, request })
            }
            MessageKind::Notification { method: m } | MessageKind::Request { method: m, .. } => {
                Err(MonitorViolation::UnknownMethod {
                    method: m.to_string(),
                })
            }
            MessageKind::Invalid => Err(MonitorViolation::InvalidMessage),
        }
    }
}

fn invalid_params(method: &str, error: serde_json::Error) -> MonitorViolation {
    MonitorViolation::InvalidParams {
        method: method.to_string(),
        detail: error.to_string(),
    }
}
```

The `#[allow(clippy::large_enum_variant)]` on `Inbound` is deliberate: the API is fixed by the SP-2a spec, and boxing `Result` would change it.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p harness-protocol --lib monitor::`
Expected: `test result: ok. 18 passed; 0 failed`.

Run: `cargo test -p harness-protocol`
Expected: 62 unit tests and 3 contract tests pass.

- [ ] **Step 5: Commit**

```bash
cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings
git add crates/harness-protocol/src/monitor.rs crates/harness-protocol/src/lib.rs
git commit -m "feat(harness-protocol): protocol monitor, a clock-free classifier of harness lines"
```

### Task 8: The kit on the monitor, and parked defects 1 and 3

Issue #82. The kit's `drive` loop decides everything itself today, in blocking code the control plane's supervisor cannot reuse. This task moves "what does this line mean" to the monitor and keeps deadlines and interrupt policy in the kit. It also fixes two parked defects at kit level: the reply to `unit/start` is required, and the `protocol_version` in the `initialize` reply is checked (defect 1); `no_change` without evidence is refused (defect 3, which the monitor already implements).

Issue #82 words defect 1 as "a `unit/start` reply is required, and its `protocol_version` is checked". The reply to `unit/start` is an empty object, so this plan reads "its" as the handshake's: the version in the `initialize` reply. See the self-review.

**Files:**
- Modify: `crates/harness-conformance/src/lib.rs`, `crates/harness-conformance/src/cases.rs`, `crates/harness-conformance/src/session.rs`, `crates/harness-conformance/src/bin/harness-fake.rs`
- Modify: `crates/harness-conformance/tests/detects_v02.rs`
- Create: `crates/harness-conformance/tests/session_exit.rs`
- Test: `crates/harness-conformance/src/lib.rs` (inline), `tests/detects_v02.rs`, `tests/session_exit.rs`; and, unchanged, `tests/detects_violations.rs`, `tests/fake_conforms.rs`, `tests/cli.rs`, `tests/fake_smoke.rs`

**Interfaces:**
- Consumes: `harness_protocol::monitor::{Inbound, MonitorViolation, ProtocolMonitor}` from Task 7; `negotiate` from Task 1.
- Produces:
  - `Violation::StartNotAcknowledged`
  - `impl From<MonitorViolation> for Violation`, the SP-2a §3.1 mapping:

    | `MonitorViolation` | kit `Violation` |
    |---|---|
    | `Malformed { line }` | `Malformed { line }` |
    | `InvalidMessage` | `InvalidMessage` |
    | `UnknownMethod { method }` | `UnknownMethod { method }` |
    | `InvalidParams { method, detail }` | `InvalidResult("<method> params: <detail>")` |
    | `InvalidResult(s)` | `InvalidResult(s)` |
    | `MessageAfterResult { method }` | `MessageAfterResult { method }` |
    | `MeteringDeclaredButSilent` | `MeteringDeclaredButSilent` |
    | `Inbound::InterruptRefused { method }` (not a violation) | `InterruptNotHonored { method }` |

  - `impl Session { pub fn exit_status(&self) -> Option<std::process::ExitStatus> }`
  - `harness-fake` modes `no_start_ack`, `silent_start`, `wrong_version_reply`, `no_change_without_evidence`

- [ ] **Step 1: Write the failing mapping test**

At the end of `crates/harness-conformance/src/lib.rs`, add:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_monitor_violation_maps_to_its_kit_violation() {
        let line = || "raw".to_string();
        let method = || "unit/event".to_string();
        let cases = [
            (
                MonitorViolation::Malformed { line: line() },
                Violation::Malformed { line: line() },
            ),
            (MonitorViolation::InvalidMessage, Violation::InvalidMessage),
            (
                MonitorViolation::UnknownMethod { method: method() },
                Violation::UnknownMethod { method: method() },
            ),
            (
                MonitorViolation::InvalidParams {
                    method: method(),
                    detail: "missing field `type`".into(),
                },
                Violation::InvalidResult("unit/event params: missing field `type`".into()),
            ),
            (
                MonitorViolation::InvalidResult("pr_open without evidence".into()),
                Violation::InvalidResult("pr_open without evidence".into()),
            ),
            (
                MonitorViolation::MessageAfterResult { method: method() },
                Violation::MessageAfterResult { method: method() },
            ),
            (
                MonitorViolation::MeteringDeclaredButSilent,
                Violation::MeteringDeclaredButSilent,
            ),
        ];
        for (from, want) in cases {
            assert_eq!(Violation::from(from), want);
        }
    }
}
```

- [ ] **Step 2: Run it and watch it fail**

Run: `cargo test -p harness-conformance --lib`
Expected: the build fails with E0433 (use of undeclared type `MonitorViolation`).

- [ ] **Step 3: Add the violation and the mapping**

In `crates/harness-conformance/src/lib.rs`, add the import above `use std::time::Duration;`:

```rust
use harness_protocol::monitor::MonitorViolation;
```

Replace the definition of `Violation` with (one new variant, `StartNotAcknowledged`):

```rust
/// One way a harness broke the protocol. Every variant is observable from outside the process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Violation {
    SpawnFailed(String),
    NoInitializeResponse,
    InitializeRejected(String),
    VersionMismatchAccepted,
    ExitedWithoutResult,
    /// The first message after `unit/start` was not its reply, or no reply came within grace.
    StartNotAcknowledged,
    Malformed {
        line: String,
    },
    InvalidMessage,
    UnknownMethod {
        method: String,
    },
    UnexpectedGateRequest,
    /// A tier that requires an oracle ended `pr_open` without ever sending `gate/request`.
    GateNotRequested,
    MessageAfterResult {
        method: String,
    },
    DidNotExitAfterResult,
    MeteringDeclaredButSilent,
    WallClockExceeded,
    InvalidResult(String),
    GateRejectionIgnored,
    InterruptNotHonored {
        method: String,
    },
}
```

Add, above the test module:

```rust
/// How the protocol monitor's findings read as kit violations. The wording for bad params is the
/// kit's own, kept from before the monitor existed.
impl From<MonitorViolation> for Violation {
    fn from(violation: MonitorViolation) -> Self {
        match violation {
            MonitorViolation::Malformed { line } => Violation::Malformed { line },
            MonitorViolation::InvalidMessage => Violation::InvalidMessage,
            MonitorViolation::UnknownMethod { method } => Violation::UnknownMethod { method },
            MonitorViolation::InvalidParams { method, detail } => {
                Violation::InvalidResult(format!("{method} params: {detail}"))
            }
            MonitorViolation::InvalidResult(detail) => Violation::InvalidResult(detail),
            MonitorViolation::MessageAfterResult { method } => {
                Violation::MessageAfterResult { method }
            }
            MonitorViolation::MeteringDeclaredButSilent => Violation::MeteringDeclaredButSilent,
        }
    }
}
```

Run: `cargo test -p harness-conformance --lib`
Expected: `test result: ok. 1 passed; 0 failed`.

- [ ] **Step 4: Write the failing defect tests**

Give `harness-fake` the four modes:

```diff
--- a/crates/harness-conformance/src/bin/harness-fake.rs
+++ b/crates/harness-conformance/src/bin/harness-fake.rs
@@ -32,6 +32,10 @@ enum Mode {
     ResultWithoutEvents,
     InvalidUtf8,
     LongLine,
+    NoStartAck,
+    SilentStart,
+    WrongVersionReply,
+    NoChangeWithoutEvidence,
 }
 
 fn mode() -> Mode {
@@ -52,6 +56,10 @@ fn mode() -> Mode {
         Ok("ignore_halt") => Mode::IgnoreHalt,
         Ok("invalid_utf8") => Mode::InvalidUtf8,
         Ok("long_line") => Mode::LongLine,
+        Ok("no_start_ack") => Mode::NoStartAck,
+        Ok("silent_start") => Mode::SilentStart,
+        Ok("wrong_version_reply") => Mode::WrongVersionReply,
+        Ok("no_change_without_evidence") => Mode::NoChangeWithoutEvidence,
         _ => Mode::Conformant,
     }
 }
@@ -249,6 +257,17 @@ fn run_unit(mode: Mode, order: &WorkOrder, rx: &Receiver<RpcMessage>) {
         return;
     }
 
+    if mode == Mode::NoChangeWithoutEvidence {
+        let result = UnitResult {
+            outcome: Outcome::NoChange,
+            evidence: None,
+            failure: None,
+            stop: None,
+        };
+        send(&RpcMessage::notification(method::UNIT_RESULT, &result));
+        return;
+    }
+
     step!(rx, pace);
 
     match mode {
@@ -412,7 +431,11 @@ fn main() {
     send(&RpcMessage::response(
         id,
         &InitializeResult {
-            protocol_version: PROTOCOL_VERSION.into(),
+            protocol_version: if mode == Mode::WrongVersionReply {
+                "0.1".into()
+            } else {
+                PROTOCOL_VERSION.into()
+            },
             harness: HarnessInfo {
                 name: "harness-fake".into(),
                 version: env!("CARGO_PKG_VERSION").into(),
@@ -444,6 +467,13 @@ fn main() {
             return;
         }
     };
-    send(&RpcMessage::response(id, &Empty {}));
+    if mode == Mode::SilentStart {
+        loop {
+            std::thread::sleep(Duration::from_secs(3_600));
+        }
+    }
+    if mode != Mode::NoStartAck {
+        send(&RpcMessage::response(id, &Empty {}));
+    }
     run_unit(mode, &order, &rx);
 }
```

Append to `crates/harness-conformance/tests/detects_v02.rs`:

```rust
#[test]
fn a_unit_that_starts_without_answering_unit_start_is_caught() {
    assert_detects(
        "no_start_ack",
        "happy_path_t1",
        Violation::StartNotAcknowledged,
    );
}

#[test]
fn a_harness_silent_after_unit_start_is_caught_at_grace_not_at_the_wall_clock() {
    let started = std::time::Instant::now();
    assert_detects(
        "silent_start",
        "happy_path_t1",
        Violation::StartNotAcknowledged,
    );
    // Five driving cases, one second of grace each, against a five-second wall clock each.
    assert!(
        started.elapsed() < Duration::from_secs(15),
        "took {:?}",
        started.elapsed()
    );
}

#[test]
fn an_initialize_reply_on_a_version_the_kit_does_not_accept_is_caught() {
    assert_detects(
        "wrong_version_reply",
        "happy_path_t1",
        Violation::InvalidResult("initialize result: protocol_version 0.1 is not accepted".into()),
    );
}

#[test]
fn no_change_without_evidence_is_caught() {
    assert_detects(
        "no_change_without_evidence",
        "happy_path_t1",
        Violation::InvalidResult("no_change without evidence".into()),
    );
}
```

- [ ] **Step 5: Run them and watch them fail**

Run: `cargo test -p harness-conformance --test detects_v02`
Expected: 2 passed, 4 failed. The four are the new tests, failing with these `left` values against the old driver:

| Test | `left` today |
|---|---|
| `a_unit_that_starts_without_answering_unit_start_is_caught` | `Pass` |
| `a_harness_silent_after_unit_start_is_caught_at_grace_not_at_the_wall_clock` | `Fail(WallClockExceeded)` |
| `an_initialize_reply_on_a_version_the_kit_does_not_accept_is_caught` | `Pass` |
| `no_change_without_evidence_is_caught` | `Fail(MeteringDeclaredButSilent)` |

This run takes about 25 seconds, because the silent harness is waited out at the wall clock.

- [ ] **Step 6: Check the version in the `initialize` reply**

In `crates/harness-conformance/src/cases.rs`, replace the imports at the top of the file (through `use std::time::Instant;`) with:

```rust
//! The conformance cases. Each spawns a fresh harness.

use crate::fixtures::work_order;
use crate::session::{Recv, Session};
use crate::{CaseOutcome, CaseReport, KitConfig, Violation};
use harness_protocol::monitor::{Inbound, ProtocolMonitor};
use harness_protocol::{
    error_code, method, negotiate, Capabilities, Empty, GateKind, GateReply, InitializeParams,
    InitializeResult, MessageKind, Outcome, RpcMessage, Tier, UnitResult, WorkOrder,
    PROTOCOL_VERSION,
};
use std::time::Instant;
```

Replace the definition of `Transcript` with the following. The `metrics` field goes: the monitor counts metrics now.

```rust
/// What one unit run produced.
#[derive(Default)]
pub(crate) struct Transcript {
    pub gate_requests: usize,
    /// Whether the interrupt request was actually sent.
    pub interrupt_sent: bool,
    /// `None` when the run ended by an honoured interrupt.
    pub result: Option<UnitResult>,
}
```

Replace the function `start_session` with:

```rust
/// Spawn, initialize at our version, and return the declared capabilities.
pub(crate) fn start_session(cfg: &KitConfig) -> Result<(Session, Capabilities), Violation> {
    let mut session = Session::spawn(cfg).map_err(Violation::SpawnFailed)?;
    session.send(&RpcMessage::request(
        INIT_ID,
        method::INITIALIZE,
        &InitializeParams {
            protocol_version: PROTOCOL_VERSION.into(),
            accepted_versions: vec![PROTOCOL_VERSION.into()],
        },
    ));
    let reply = match session.recv(cfg.grace) {
        Recv::Message(msg) => msg,
        Recv::Malformed(line) => return Err(Violation::Malformed { line }),
        Recv::Eof | Recv::Timeout => return Err(Violation::NoInitializeResponse),
    };
    match reply.kind() {
        MessageKind::Response { id: INIT_ID } => {
            let result: InitializeResult = reply
                .result_as()
                .map_err(|e| Violation::InvalidResult(format!("initialize result: {e}")))?;
            if !negotiate(&[PROTOCOL_VERSION], &result.protocol_version) {
                return Err(Violation::InvalidResult(format!(
                    "initialize result: protocol_version {} is not accepted",
                    result.protocol_version
                )));
            }
            Ok((session, result.capabilities))
        }
        MessageKind::ErrorResponse { id: INIT_ID, error } => {
            Err(Violation::InitializeRejected(error.message.clone()))
        }
        _ => Err(Violation::NoInitializeResponse),
    }
}
```

The crate does not compile again until Step 7 is done: the old `drive` still reads `transcript.metrics`.

- [ ] **Step 7: Rebuild `drive` on the monitor**

In the same file, delete the function `validate_result`, and replace the functions `finish` and `drive` with:

```rust
/// After `unit/result` nothing more may arrive, and the process must exit within grace.
fn finish(
    session: &mut Session,
    cfg: &KitConfig,
    monitor: &mut ProtocolMonitor,
    transcript: Transcript,
) -> Result<Transcript, Violation> {
    session.close_stdin();
    match session.recv(cfg.grace) {
        // The monitor has seen the result, so it refuses whatever this is.
        Recv::Message(msg) => {
            return Err(monitor
                .on_line(Ok(msg))
                .err()
                .map_or(Violation::InvalidMessage, Violation::from));
        }
        Recv::Malformed(line) => return Err(Violation::Malformed { line }),
        Recv::Eof | Recv::Timeout => {}
    }
    if session.wait_exit(cfg.grace) {
        Ok(transcript)
    } else {
        session.kill();
        Err(Violation::DidNotExitAfterResult)
    }
}

/// Start `order` and read until the unit ends. The reply to `unit/start` must be the first
/// message back, within `cfg.grace`. `gate` answers any `gate/request`. If `interrupt` is a method
/// name, it is sent as a request after the first `unit/event` or, under `GatePolicy::Hold`, in
/// place of answering the first `gate/request`. From then on the harness has `cfg.grace` to answer
/// it and exit, without sending `unit/result`.
///
/// The protocol monitor says what each line means. Deadlines, the start reply and what a result
/// after an interrupt means are the kit's own judgements.
pub(crate) fn drive(
    session: &mut Session,
    cfg: &KitConfig,
    caps: &Capabilities,
    order: &WorkOrder,
    gate: GatePolicy,
    interrupt: Option<&'static str>,
) -> Result<Transcript, Violation> {
    let mut deadline = Instant::now() + cfg.wall_clock;
    let start_deadline = Instant::now() + cfg.grace;
    session.send(&RpcMessage::request(START_ID, method::UNIT_START, order));
    let mut monitor = ProtocolMonitor::new(caps.clone(), START_ID);
    let mut transcript = Transcript::default();
    let mut start_acked = false;
    let mut interrupt_sent: Option<&'static str> = None;
    let mut interrupt_acked = false;

    loop {
        let limit = if start_acked {
            deadline
        } else {
            deadline.min(start_deadline)
        };
        let remaining = limit.saturating_duration_since(Instant::now());
        let received = if remaining.is_zero() {
            Recv::Timeout
        } else {
            session.recv(remaining)
        };
        let line = match received {
            Recv::Message(msg) => Ok(msg),
            Recv::Malformed(line) => Err(line),
            Recv::Timeout => {
                session.kill();
                return Err(match interrupt_sent {
                    Some(ctl) => Violation::InterruptNotHonored {
                        method: ctl.to_string(),
                    },
                    None if !start_acked => Violation::StartNotAcknowledged,
                    None => Violation::WallClockExceeded,
                });
            }
            Recv::Eof => {
                return match interrupt_sent {
                    Some(_) if interrupt_acked && session.wait_exit(cfg.grace) => Ok(transcript),
                    Some(ctl) => {
                        session.kill();
                        Err(Violation::InterruptNotHonored {
                            method: ctl.to_string(),
                        })
                    }
                    None => Err(Violation::ExitedWithoutResult),
                };
            }
        };

        let inbound = monitor.on_line(line)?;
        if !start_acked {
            if inbound != Inbound::StartAck {
                return Err(Violation::StartNotAcknowledged);
            }
            start_acked = true;
            continue;
        }

        match inbound {
            // A repeated reply to `unit/start` changes nothing.
            Inbound::StartAck => {}
            Inbound::InterruptAck { .. } => interrupt_acked = true,
            Inbound::InterruptRefused { method } => {
                return Err(Violation::InterruptNotHonored { method });
            }
            Inbound::Event(_) => {
                if let (Some(ctl), None, true) =
                    (interrupt, interrupt_sent, gate != GatePolicy::Hold)
                {
                    session.send(&RpcMessage::request(INTERRUPT_ID, ctl, &Empty {}));
                    monitor.interrupt_sent(INTERRUPT_ID, ctl);
                    interrupt_sent = Some(ctl);
                    transcript.interrupt_sent = true;
                    deadline = deadline.min(Instant::now() + cfg.grace);
                }
            }
            Inbound::Result(result) => {
                if let Some(ctl) = interrupt_sent {
                    return Err(Violation::InterruptNotHonored {
                        method: ctl.to_string(),
                    });
                }
                transcript.result = Some(result);
                return finish(session, cfg, &mut monitor, transcript);
            }
            Inbound::GateRequest { id, .. } => {
                transcript.gate_requests += 1;
                let approved = match gate {
                    GatePolicy::Unexpected => return Err(Violation::UnexpectedGateRequest),
                    GatePolicy::Hold => {
                        // The gate stays pending: a harness blocked on it must still handle this.
                        if let (Some(ctl), None) = (interrupt, interrupt_sent) {
                            session.send(&RpcMessage::request(INTERRUPT_ID, ctl, &Empty {}));
                            monitor.interrupt_sent(INTERRUPT_ID, ctl);
                            interrupt_sent = Some(ctl);
                            transcript.interrupt_sent = true;
                            deadline = deadline.min(Instant::now() + cfg.grace);
                        }
                        continue;
                    }
                    GatePolicy::Approve => true,
                    GatePolicy::Reject => false,
                };
                session.send(&RpcMessage::response(
                    id,
                    &GateReply {
                        approved,
                        edited_test_files: None,
                    },
                ));
            }
        }
    }
}
```

- [ ] **Step 8: Run every kit test**

Run: `cargo test -p harness-conformance`
Expected: 31 passed, 0 failed: 1 unit, 4 `cli`, 6 `detects_v02`, 16 `detects_violations`, 2 `fake_conforms`, 2 `fake_smoke`.

Run: `git diff --stat origin/factory/m0 -- crates/harness-conformance/tests/detects_violations.rs crates/harness-conformance/tests/fake_conforms.rs crates/harness-conformance/tests/cli.rs`
Expected: no output. Those three files are as they were at the branch point: the rebuilt kit passes them unchanged. (`fake_smoke.rs` differs only by the struct-literal fields of Tasks 1 and 2; none of its assertions changed.)

- [ ] **Step 9: Commit the rebuild**

```bash
cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings
git add crates/harness-conformance/src/lib.rs crates/harness-conformance/src/cases.rs crates/harness-conformance/src/bin/harness-fake.rs crates/harness-conformance/tests/detects_v02.rs
git commit -m "refactor(harness-conformance): drive units through the protocol monitor; require the start reply and an accepted version"
```

- [ ] **Step 10: Write the failing exit-status test**

Create `crates/harness-conformance/tests/session_exit.rs`:

```rust
//! The kit's `Session` records how the harness process ended.

use harness_conformance::{work_order, KitConfig, Recv, Session};
use harness_protocol::{method, InitializeParams, RpcMessage, Tier, PROTOCOL_VERSION};
use std::time::Duration;

fn session(mode: &str) -> Session {
    let mut cfg = KitConfig::new(vec![env!("CARGO_BIN_EXE_harness-fake").to_string()]);
    cfg.env = vec![
        ("HARNESS_FAKE_MODE".into(), mode.into()),
        ("HARNESS_FAKE_STEP_MS".into(), "1".into()),
    ];
    let mut session = Session::spawn(&cfg).expect("harness-fake starts");
    let init = InitializeParams {
        protocol_version: PROTOCOL_VERSION.into(),
        accepted_versions: vec![PROTOCOL_VERSION.into()],
    };
    assert!(session.send(&RpcMessage::request(1, method::INITIALIZE, &init)));
    assert!(session.send(&RpcMessage::request(
        2,
        method::UNIT_START,
        &work_order(Tier::T1)
    )));
    session
}

fn drain(session: &Session) {
    loop {
        match session.recv(Duration::from_secs(10)) {
            Recv::Eof => return,
            Recv::Timeout => panic!("harness-fake did not close its stdout"),
            Recv::Message(_) | Recv::Malformed(_) => {}
        }
    }
}

#[test]
fn the_exit_status_is_unknown_until_the_process_is_seen_to_exit() {
    let mut session = session("crash");
    assert!(session.exit_status().is_none());
    drain(&session);
    assert!(session.wait_exit(Duration::from_secs(5)));
    assert_eq!(session.exit_status().and_then(|s| s.code()), Some(3));
}

#[test]
fn a_clean_run_exits_zero() {
    let mut session = session("conformant");
    drain(&session);
    session.close_stdin();
    assert!(session.wait_exit(Duration::from_secs(5)));
    assert_eq!(session.exit_status().and_then(|s| s.code()), Some(0));
}
```

Run: `cargo test -p harness-conformance --test session_exit`
Expected: the build fails with E0599 (no method named `exit_status` found for struct `Session`).

- [ ] **Step 11: Record the exit status**

Replace `crates/harness-conformance/src/session.rs` with:

```rust
//! A harness child process: write messages to its stdin, read its stdout on a background thread.

use crate::KitConfig;
use harness_protocol::{read_message, write_message, ReadError, RpcMessage};
use std::io::BufReader;
use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

enum Incoming {
    Message(RpcMessage),
    Malformed(String),
}

/// The result of waiting for the harness's next line.
pub enum Recv {
    Message(RpcMessage),
    Malformed(String),
    /// The harness closed its stdout.
    Eof,
    Timeout,
}

pub struct Session {
    child: Child,
    stdin: Option<ChildStdin>,
    rx: Receiver<Incoming>,
    exit: Option<ExitStatus>,
}

impl Session {
    pub fn spawn(cfg: &KitConfig) -> Result<Self, String> {
        let (program, args) = cfg.command.split_first().ok_or("empty harness command")?;
        let mut child = Command::new(program)
            .args(args)
            .envs(cfg.env.clone())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| format!("{program}: {e}"))?;
        let stdin = child.stdin.take();
        let stdout = child.stdout.take().expect("stdout is piped");
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                let item = match read_message(&mut reader) {
                    Ok(msg) => Incoming::Message(msg),
                    Err(ReadError::Malformed { line, .. }) => Incoming::Malformed(line),
                    Err(ReadError::InvalidUtf8 { line }) => {
                        Incoming::Malformed(format!("invalid UTF-8: {line}"))
                    }
                    Err(ReadError::LineTooLong { limit }) => {
                        // The reader is inside the over-long line, so nothing after it can be
                        // trusted: report it and stop.
                        let _ = tx.send(Incoming::Malformed(format!(
                            "line longer than {limit} bytes"
                        )));
                        break;
                    }
                    Err(ReadError::Eof) | Err(ReadError::Io(_)) => break,
                };
                if tx.send(item).is_err() {
                    break;
                }
            }
        });
        Ok(Self {
            child,
            stdin,
            rx,
            exit: None,
        })
    }

    /// False if the harness's stdin is closed or the write failed.
    pub fn send(&mut self, msg: &RpcMessage) -> bool {
        match self.stdin.as_mut() {
            Some(stdin) => write_message(stdin, msg).is_ok(),
            None => false,
        }
    }

    /// Closing stdin is the protocol's "shut down" signal.
    pub fn close_stdin(&mut self) {
        self.stdin = None;
    }

    pub fn recv(&self, timeout: Duration) -> Recv {
        match self.rx.recv_timeout(timeout) {
            Ok(Incoming::Message(msg)) => Recv::Message(msg),
            Ok(Incoming::Malformed(line)) => Recv::Malformed(line),
            Err(RecvTimeoutError::Timeout) => Recv::Timeout,
            Err(RecvTimeoutError::Disconnected) => Recv::Eof,
        }
    }

    /// True if the process exited within `timeout`.
    pub fn wait_exit(&mut self, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        loop {
            if let Ok(Some(status)) = self.child.try_wait() {
                self.exit = Some(status);
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// How the process ended, once `wait_exit` has seen it end.
    pub fn exit_status(&self) -> Option<ExitStatus> {
        self.exit
    }

    pub fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.kill();
    }
}
```

The changes from Task 6's version are the `ExitStatus` import, the `exit` field, its initialiser in `spawn`, the assignment in `wait_exit`, and the new method `exit_status`.

Run: `cargo test -p harness-conformance --test session_exit`
Expected: `test result: ok. 2 passed; 0 failed`.

- [ ] **Step 12: Commit**

```bash
cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings
git add crates/harness-conformance/src/session.rs crates/harness-conformance/tests/session_exit.rs
git commit -m "feat(harness-conformance): record how the harness process exited"
```

### Task 9: `harness-fake` as a peer the control plane can drive

Issue #83. `harness-fake` passes the kit today and still cannot drive the unit state machine: at T1 it never sends `oracle_frozen`, which is the only way out of the `Spec` phase; it reviews once whatever the round floor is; and it declares `resume: false`.

**Files:**
- Modify: `crates/harness-conformance/src/bin/harness-fake.rs`
- Create: `crates/harness-conformance/tests/common/mod.rs`, `crates/harness-conformance/tests/fake_peer.rs`, `crates/harness-conformance/tests/fake_replay.rs`
- Test: the two new test files

**Interfaces:**
- Consumes: `ProtocolMonitor` (Task 7); `fleet_core::{transition, gate_met, GateConfig, Phase, ReviewSnapshot, Trigger, Tier}` as a dev-dependency (request R2). Only items that exist on `main` today are used, so this lane does not wait for CC-CORE.
- Produces:
  - `harness-fake` sends `oracle_frozen` at every tier; repeats build, check and review until the review gate holds, with one `metric` per cycle; declares `resume: true`; on `resume { oracle_frozen: true }` sends `provisioned` and goes straight to building; writes `harness-fake <version> starting` to stderr.
  - `HARNESS_FAKE_MODE=blockers`: round 1 reports one unresolved blocker, later rounds none.
  - Test helper `tests/common/mod.rs`: `pub fn run_fake(mode: &str, order: &WorkOrder) -> Transcript`, `pub fn order(tier: Tier, min_review_rounds: u32, resume: Option<bool>) -> WorkOrder`, `pub enum Wire { Event(UnitEvent), Gate(GateRequest) }`, `pub struct Transcript { capabilities, wire: Vec<Wire>, result: UnitResult, stderr: String, exited_cleanly: bool }` with `observations()`, `gates()`, `metrics()`.

- [ ] **Step 1: Check the dev-dependency is present**

Run: `cargo tree -p harness-conformance -e dev --depth 1`
Expected: a `[dev-dependencies]` section listing `fleet-core v0.1.0`. If it is missing, request R2 has not landed: stop and report.

- [ ] **Step 2: Write the transcript helper**

Create `crates/harness-conformance/tests/common/mod.rs`:

```rust
//! Drives `harness-fake` by hand and records everything it sent, so a test can assert on the
//! order of a whole unit. The kit's own cases report only a verdict.

#![allow(dead_code)]

use harness_conformance::work_order;
use harness_protocol::monitor::{Inbound, ProtocolMonitor};
use harness_protocol::{
    method, read_message, write_message, Capabilities, GateReply, GateRequest, InitializeParams,
    InitializeResult, Observation, ReadError, Resume, RpcMessage, Tier, UnitEvent, UnitResult,
    WorkOrder, PROTOCOL_VERSION,
};
use std::io::{BufReader, Read};
use std::process::{Command, Stdio};

const START_ID: u64 = 2;

/// One thing the harness sent between the start reply and its result.
#[derive(Debug, Clone, PartialEq)]
pub enum Wire {
    Event(UnitEvent),
    Gate(GateRequest),
}

pub struct Transcript {
    pub capabilities: Capabilities,
    pub wire: Vec<Wire>,
    pub result: UnitResult,
    pub stderr: String,
    pub exited_cleanly: bool,
}

impl Transcript {
    pub fn observations(&self) -> Vec<Observation> {
        self.wire
            .iter()
            .filter_map(|w| match w {
                Wire::Event(UnitEvent::Observed { observation }) => Some(observation.clone()),
                _ => None,
            })
            .collect()
    }

    pub fn gates(&self) -> usize {
        self.wire
            .iter()
            .filter(|w| matches!(w, Wire::Gate(_)))
            .count()
    }

    pub fn metrics(&self) -> usize {
        self.wire
            .iter()
            .filter(|w| matches!(w, Wire::Event(UnitEvent::Metric { .. })))
            .count()
    }
}

/// The kit's fixture at `tier`, with the review floor and resume state a test asks for.
pub fn order(tier: Tier, min_review_rounds: u32, resume: Option<bool>) -> WorkOrder {
    let mut order = work_order(tier);
    order.caps.min_review_rounds = min_review_rounds;
    order.resume = resume.map(|oracle_frozen| Resume { oracle_frozen });
    order
}

/// Run one unit to its result, approving any gate, and wait for the process to exit.
pub fn run_fake(mode: &str, order: &WorkOrder) -> Transcript {
    let mut child = Command::new(env!("CARGO_BIN_EXE_harness-fake"))
        .env("HARNESS_FAKE_MODE", mode)
        .env("HARNESS_FAKE_STEP_MS", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("harness-fake builds and starts");
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    let init = InitializeParams {
        protocol_version: PROTOCOL_VERSION.into(),
        accepted_versions: vec![PROTOCOL_VERSION.into()],
    };
    write_message(
        &mut stdin,
        &RpcMessage::request(1, method::INITIALIZE, &init),
    )
    .unwrap();
    let reply: InitializeResult = read_message(&mut stdout)
        .expect("an initialize reply")
        .result_as()
        .expect("a well-formed initialize result");

    write_message(
        &mut stdin,
        &RpcMessage::request(START_ID, method::UNIT_START, order),
    )
    .unwrap();
    let mut monitor = ProtocolMonitor::new(reply.capabilities.clone(), START_ID);
    let mut wire = Vec::new();
    let result = loop {
        let msg = read_message(&mut stdout).expect("the harness talks until its result");
        match monitor.on_line(Ok(msg)).expect("a legal message") {
            Inbound::StartAck => {}
            Inbound::Event(event) => wire.push(Wire::Event(event)),
            Inbound::GateRequest { id, request } => {
                wire.push(Wire::Gate(request));
                let reply = GateReply {
                    approved: true,
                    edited_test_files: None,
                };
                write_message(&mut stdin, &RpcMessage::response(id, &reply)).unwrap();
            }
            Inbound::Result(result) => break result,
            other => panic!("unexpected message: {other:?}"),
        }
    };

    drop(stdin);
    assert!(
        matches!(read_message(&mut stdout), Err(ReadError::Eof)),
        "nothing may follow the result"
    );
    let exited_cleanly = child.wait().unwrap().success();
    let mut stderr = String::new();
    child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut stderr)
        .unwrap();
    Transcript {
        capabilities: reply.capabilities,
        wire,
        result,
        stderr,
        exited_cleanly,
    }
}
```

- [ ] **Step 3: Write the failing peer tests**

Create `crates/harness-conformance/tests/fake_peer.rs`:

```rust
//! `harness-fake` as a peer for the control plane's state machine: the order it sends
//! observations in, the review loop, resume, and its startup line.

mod common;

use common::{order, run_fake, Wire};
use harness_conformance::{run_all, CaseOutcome, KitConfig};
use harness_protocol::{Observation, Outcome, Severity, Tier, UnitEvent};
use std::time::Duration;

fn review(round: u32, unresolved_blockers: u32) -> Observation {
    Observation::ReviewFinished {
        round,
        unresolved_blockers,
        checks_green: true,
    }
}

/// `build_finished`, `checks_passed`, then that round's `review_finished`.
fn cycle(round: u32, unresolved_blockers: u32) -> [Observation; 3] {
    [
        Observation::BuildFinished,
        Observation::ChecksPassed,
        review(round, unresolved_blockers),
    ]
}

/// Compare observations ignoring any freeze payload, which `fake_v02.rs` checks.
fn kinds(observations: Vec<Observation>) -> Vec<Observation> {
    observations
        .into_iter()
        .map(|o| match o {
            Observation::OracleFrozen { .. } => Observation::OracleFrozen { freeze: None },
            other => other,
        })
        .collect()
}

#[test]
fn the_kit_passes_in_blockers_mode() {
    let mut cfg = KitConfig::new(vec![env!("CARGO_BIN_EXE_harness-fake").to_string()]);
    cfg.env = vec![
        ("HARNESS_FAKE_MODE".into(), "blockers".into()),
        ("HARNESS_FAKE_STEP_MS".into(), "20".into()),
    ];
    cfg.wall_clock = Duration::from_secs(10);
    cfg.grace = Duration::from_secs(2);
    for report in run_all(&cfg) {
        assert_eq!(report.outcome, CaseOutcome::Pass, "case `{}`", report.name);
    }
}

#[test]
fn a_t1_unit_freezes_its_oracle_and_sends_no_gate() {
    let t = run_fake("conformant", &order(Tier::T1, 1, None));
    let mut want = vec![
        Observation::Provisioned,
        Observation::OracleFrozen { freeze: None },
    ];
    want.extend(cycle(1, 0));
    assert_eq!(kinds(t.observations()), want);
    assert_eq!(t.gates(), 0);
    assert_eq!(t.result.outcome, Outcome::PrOpen);
}

#[test]
fn at_t2_and_t3_the_gate_follows_oracle_frozen_with_no_observation_between() {
    for tier in [Tier::T2, Tier::T3] {
        let t = run_fake("conformant", &order(tier, 1, None));
        let frozen = t
            .wire
            .iter()
            .position(|w| {
                matches!(
                    w,
                    Wire::Event(UnitEvent::Observed {
                        observation: Observation::OracleFrozen { .. }
                    })
                )
            })
            .expect("oracle_frozen is sent");
        let gate = t
            .wire
            .iter()
            .position(|w| matches!(w, Wire::Gate(_)))
            .expect("a gate is requested");
        assert!(
            frozen < gate,
            "{tier:?}: the gate must follow oracle_frozen"
        );
        for between in &t.wire[frozen + 1..gate] {
            assert!(
                matches!(
                    between,
                    Wire::Event(
                        UnitEvent::Log { .. }
                            | UnitEvent::Metric { .. }
                            | UnitEvent::Finding { .. }
                            | UnitEvent::Error { .. }
                    )
                ),
                "{tier:?}: {between:?} may not sit between oracle_frozen and the gate"
            );
        }
        assert_eq!(t.gates(), 1);
    }
}

#[test]
fn the_review_loop_runs_until_the_round_floor_is_reached() {
    let t = run_fake("conformant", &order(Tier::T1, 3, None));
    let mut want = vec![
        Observation::Provisioned,
        Observation::OracleFrozen { freeze: None },
    ];
    want.extend(cycle(1, 0));
    want.extend(cycle(2, 0));
    want.extend(cycle(3, 0));
    assert_eq!(kinds(t.observations()), want);
    assert_eq!(t.metrics(), 3, "one metric per cycle");
}

#[test]
fn a_review_floor_of_zero_still_runs_one_round() {
    let t = run_fake("conformant", &order(Tier::T1, 0, None));
    assert_eq!(kinds(t.observations())[2..], cycle(1, 0));
}

#[test]
fn blockers_mode_reports_one_blocker_in_round_one_and_builds_again() {
    for (floor, rounds) in [(1, 2), (3, 3)] {
        let t = run_fake("blockers", &order(Tier::T1, floor, None));
        let mut want = vec![
            Observation::Provisioned,
            Observation::OracleFrozen { freeze: None },
        ];
        want.extend(cycle(1, 1));
        for round in 2..=rounds {
            want.extend(cycle(round, 0));
        }
        assert_eq!(kinds(t.observations()), want, "floor {floor}");
        assert_eq!(t.metrics(), rounds as usize, "floor {floor}");
        let blockers = t
            .wire
            .iter()
            .filter(|w| {
                matches!(
                    w,
                    Wire::Event(UnitEvent::Finding {
                        round: 1,
                        severity: Severity::Blocker,
                        resolved: false,
                        ..
                    })
                )
            })
            .count();
        assert_eq!(blockers, 1, "floor {floor}");
        assert_eq!(t.result.outcome, Outcome::PrOpen);
    }
}

#[test]
fn a_unit_resumed_after_its_freeze_sends_no_oracle_frozen_and_no_gate() {
    for tier in [Tier::T1, Tier::T2, Tier::T3] {
        let t = run_fake("blockers", &order(tier, 1, Some(true)));
        let mut want = vec![Observation::Provisioned];
        // Rounds count from 1 in every process, so the scripted blocker appears again.
        want.extend(cycle(1, 1));
        want.extend(cycle(2, 0));
        assert_eq!(kinds(t.observations()), want, "{tier:?}");
        assert_eq!(t.gates(), 0, "{tier:?}");
    }
}

#[test]
fn a_unit_resumed_before_its_freeze_freezes_and_asks_again() {
    let t = run_fake("conformant", &order(Tier::T2, 1, Some(false)));
    let mut want = vec![
        Observation::Provisioned,
        Observation::OracleFrozen { freeze: None },
    ];
    want.extend(cycle(1, 0));
    assert_eq!(kinds(t.observations()), want);
    assert_eq!(t.gates(), 1);
}

#[test]
fn the_fake_declares_that_it_can_resume() {
    let t = run_fake("conformant", &order(Tier::T1, 1, None));
    assert!(t.capabilities.resume);
    assert!(t.capabilities.halt);
}

#[test]
fn the_fake_writes_one_startup_line_to_stderr() {
    let t = run_fake("conformant", &order(Tier::T1, 1, None));
    let want = format!("harness-fake {} starting", env!("CARGO_PKG_VERSION"));
    assert_eq!(t.stderr.lines().next(), Some(want.as_str()));
    assert!(t.exited_cleanly);
}
```

- [ ] **Step 4: Write the failing replay test**

Create `crates/harness-conformance/tests/fake_replay.rs`. It maps observations to triggers the way SP-2a §4.2 has the supervisor do it, and fails on any trigger `transition()` rejects.

```rust
//! A harness can pass the kit and still be a peer the control plane cannot drive. This replays
//! what `harness-fake` sends, in its two protocol-valid modes, through `fleet_core::transition`
//! the way the control plane maps observations to triggers, and fails on any trigger the state
//! machine rejects.

mod common;

use common::{order, run_fake, Transcript, Wire};
use fleet_core::{gate_met, transition, GateConfig, Phase, ReviewSnapshot, Trigger};
use harness_protocol::{Observation, Outcome, Tier, UnitEvent};

fn core_tier(tier: Tier) -> fleet_core::Tier {
    match tier {
        Tier::T1 => fleet_core::Tier::T1,
        Tier::T2 => fleet_core::Tier::T2,
        Tier::T3 => fleet_core::Tier::T3,
    }
}

/// The phase the unit is in when its result arrives.
fn replay(t: &Transcript, tier: Tier, min_review_rounds: u32, resume: Option<bool>) -> Phase {
    let core = core_tier(tier);
    let apply = |phase: Phase, trigger: Trigger| -> Phase {
        transition(phase, core, trigger)
            .unwrap_or_else(|| panic!("{trigger:?} is not valid in {phase:?}"))
    };
    // A fresh unit and a resumed one both start a process in `Provisioning`.
    let mut phase = Phase::Provisioning;
    let mut freeze_recorded = false;
    let mut round = 0;
    let mut previous_blockers = None;

    for item in &t.wire {
        match item {
            Wire::Event(UnitEvent::Observed { observation }) => match observation {
                Observation::Provisioned => {
                    phase = apply(phase, Trigger::Provisioned);
                    // Resumed after the freeze: the control plane supplies the triggers the
                    // harness no longer sends.
                    if resume == Some(true) {
                        phase = apply(phase, Trigger::OracleFrozen);
                        if tier.requires_oracle() {
                            phase = apply(phase, Trigger::OracleApproved);
                        }
                    }
                }
                Observation::OracleFrozen { .. } => {
                    assert_eq!(phase, Phase::Spec, "oracle_frozen outside Spec");
                    if tier.requires_oracle() {
                        // Recorded only; applied when the gate request arrives.
                        freeze_recorded = true;
                    } else {
                        phase = apply(phase, Trigger::OracleFrozen);
                    }
                }
                Observation::BuildFinished => phase = apply(phase, Trigger::BuildFinished),
                Observation::ChecksPassed => phase = apply(phase, Trigger::ChecksPassed),
                Observation::ChecksFailed => phase = apply(phase, Trigger::ChecksFailed),
                Observation::EmptyDiff => panic!("the scripted modes never report an empty diff"),
                Observation::ReviewFinished {
                    round: reported,
                    unresolved_blockers,
                    checks_green,
                } => {
                    round += 1;
                    assert_eq!(*reported, round, "rounds count from 1 in every process");
                    let met = gate_met(
                        GateConfig { min_review_rounds },
                        ReviewSnapshot {
                            round,
                            unresolved_blockers: *unresolved_blockers,
                            prev_unresolved_blockers: previous_blockers,
                            checks_green: *checks_green,
                        },
                    );
                    phase = apply(phase, Trigger::ReviewFinished { gate_met: met });
                    previous_blockers = Some(*unresolved_blockers);
                }
            },
            // Stage events, metrics, logs, findings and artifacts drive no transition.
            Wire::Event(_) => {}
            Wire::Gate(_) => {
                assert!(tier.requires_oracle(), "a gate request at T1");
                assert!(freeze_recorded, "a gate request before oracle_frozen");
                phase = apply(phase, Trigger::OracleFrozen);
                // `run_fake` approves every gate.
                phase = apply(phase, Trigger::OracleApproved);
                freeze_recorded = false;
            }
        }
    }
    phase
}

#[test]
fn every_valid_transcript_reaches_merge_check_and_ends_pr_open() {
    for mode in ["conformant", "blockers"] {
        for tier in [Tier::T1, Tier::T2, Tier::T3] {
            for resume in [None, Some(false), Some(true)] {
                for floor in [1, 3] {
                    let what = format!("{mode}, {tier:?}, resume {resume:?}, floor {floor}");
                    let t = run_fake(mode, &order(tier, floor, resume));
                    assert_eq!(replay(&t, tier, floor, resume), Phase::MergeCheck, "{what}");
                    assert_eq!(t.result.outcome, Outcome::PrOpen, "{what}");
                }
            }
        }
    }
}
```

- [ ] **Step 5: Run both and watch them fail**

Run: `cargo test -p harness-conformance --test fake_peer --test fake_replay`
Expected: `fake_peer` fails in most tests, among them `a_t1_unit_freezes_its_oracle_and_sends_no_gate` (no `oracle_frozen` at T1), `the_review_loop_runs_until_the_round_floor_is_reached`, `the_fake_declares_that_it_can_resume` and `the_fake_writes_one_startup_line_to_stderr`. `fake_replay` panics with `BuildFinished is not valid in Spec`, which is the defect in one line.

- [ ] **Step 6: Make `harness-fake` follow the state machine's order**

Apply to `crates/harness-conformance/src/bin/harness-fake.rs`:

```diff
--- a/crates/harness-conformance/src/bin/harness-fake.rs
+++ b/crates/harness-conformance/src/bin/harness-fake.rs
@@ -6,8 +6,8 @@ use harness_protocol::{
     error_code, method, negotiate, read_message, write_message, ArtifactKind, Capabilities,
     Delivery, DeliveryEvidence, Empty, ErrorScope, Evidence, Failure, GateKind, GateReply,
     GateRequest, HarnessInfo, InitializeParams, InitializeResult, Isolation, MessageKind, Metering,
-    Observation, Outcome, RpcMessage, TestRun, UnitEvent, UnitResult, WorkOrder, MAX_LINE_BYTES,
-    PROTOCOL_VERSION,
+    Observation, Outcome, RpcMessage, Severity, TestRun, UnitEvent, UnitResult, WorkOrder,
+    MAX_LINE_BYTES, PROTOCOL_VERSION,
 };
 use std::io::{self, BufReader, Write};
 use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
@@ -36,6 +36,7 @@ enum Mode {
     SilentStart,
     WrongVersionReply,
     NoChangeWithoutEvidence,
+    Blockers,
 }
 
 fn mode() -> Mode {
@@ -60,6 +61,7 @@ fn mode() -> Mode {
         Ok("silent_start") => Mode::SilentStart,
         Ok("wrong_version_reply") => Mode::WrongVersionReply,
         Ok("no_change_without_evidence") => Mode::NoChangeWithoutEvidence,
+        Ok("blockers") => Mode::Blockers,
         _ => Mode::Conformant,
     }
 }
@@ -81,7 +83,7 @@ fn capabilities() -> Capabilities {
         metering: Metering::Usd,
         gates: vec![GateKind::Oracle],
         delivery: Delivery::Bundle,
-        resume: false,
+        resume: true,
         halt: true,
         holdouts: false,
         controls: Vec::new(),
@@ -295,8 +297,13 @@ fn run_unit(mode: Mode, order: &WorkOrder, rx: &Receiver<RpcMessage>) {
         _ => {}
     }
 
-    if order.tier.requires_oracle() && mode != Mode::SkipGate {
+    // Every tier freezes its oracle; T2/T3 then wait at the gate. A unit resumed after its oracle
+    // was frozen sends neither and goes straight to building.
+    let resumed_past_oracle = order.resume.as_ref().is_some_and(|r| r.oracle_frozen);
+    if !resumed_past_oracle {
         observe(Observation::OracleFrozen { freeze: None });
+    }
+    if !resumed_past_oracle && order.tier.requires_oracle() && mode != Mode::SkipGate {
         let gate = GateRequest::Oracle {
             test_files: vec!["tests/fake.test.js".into()],
             hash: "fake-oracle-hash".into(),
@@ -328,29 +335,49 @@ fn run_unit(mode: Mode, order: &WorkOrder, rx: &Receiver<RpcMessage>) {
         }
     }
 
-    if mode != Mode::SilentMetering {
-        event(UnitEvent::Metric {
-            tokens_in: 1_200,
-            tokens_out: 300,
-            cost_usd: 0.02,
-            elapsed_ms: pace.as_millis() as u64,
-            cost_basis: None,
-            stage: None,
-            role: None,
-            adapter: None,
-            model: None,
+    // Build, check and review until the review gate holds: green checks, no unresolved blocker,
+    // and the round floor reached. Rounds count from 1 in every process.
+    let floor = order.caps.min_review_rounds.max(1);
+    let mut round = 0;
+    loop {
+        round += 1;
+        if mode != Mode::SilentMetering {
+            event(UnitEvent::Metric {
+                tokens_in: 1_200,
+                tokens_out: 300,
+                cost_usd: 0.02,
+                elapsed_ms: pace.as_millis() as u64,
+                cost_basis: None,
+                stage: None,
+                role: None,
+                adapter: None,
+                model: None,
+            });
+        }
+        observe(Observation::BuildFinished);
+        step!(rx, pace);
+        observe(Observation::ChecksPassed);
+        step!(rx, pace);
+        let unresolved_blockers = u32::from(mode == Mode::Blockers && round == 1);
+        if unresolved_blockers > 0 {
+            event(UnitEvent::Finding {
+                round,
+                severity: Severity::Blocker,
+                title: "scripted blocker".into(),
+                file: None,
+                resolved: false,
+            });
+        }
+        observe(Observation::ReviewFinished {
+            round,
+            unresolved_blockers,
+            checks_green: true,
         });
+        step!(rx, pace);
+        if unresolved_blockers == 0 && round >= floor {
+            break;
+        }
     }
-    observe(Observation::BuildFinished);
-    step!(rx, pace);
-    observe(Observation::ChecksPassed);
-    step!(rx, pace);
-    observe(Observation::ReviewFinished {
-        round: order.caps.min_review_rounds.max(1),
-        unresolved_blockers: 0,
-        checks_green: true,
-    });
-    step!(rx, pace);
     event(UnitEvent::Artifact {
         kind: ArtifactKind::Branch,
         reference: order.branch.clone(),
@@ -393,6 +420,7 @@ fn run_unit(mode: Mode, order: &WorkOrder, rx: &Receiver<RpcMessage>) {
 }
 
 fn main() {
+    eprintln!("harness-fake {} starting", env!("CARGO_PKG_VERSION"));
     let mode = mode();
     let rx = spawn_reader();
 
```

Three things change in `run_unit`: `oracle_frozen` is sent at every tier unless the unit was resumed after its freeze; the gate is requested only when that same condition holds at T2 or T3; and the single build, check and review becomes a loop that ends when a round has no unresolved blocker and has reached the floor. `min_review_rounds: 0` is treated as 1.

- [ ] **Step 7: Run the tests**

Run: `cargo test -p harness-conformance --test fake_peer --test fake_replay`
Expected: `fake_peer`: 10 passed. `fake_replay`: 1 passed (it runs 36 units: two modes, three tiers, fresh and both kinds of resume, two round floors).

Run: `cargo test -p harness-conformance`
Expected: 44 passed, 0 failed. `detects_violations` still passes unchanged: `skip_gate` now freezes but still sends no gate, so it is still caught as `GateNotRequested`.

- [ ] **Step 8: Commit**

```bash
cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings
git add crates/harness-conformance/src/bin/harness-fake.rs crates/harness-conformance/tests/common/mod.rs crates/harness-conformance/tests/fake_peer.rs crates/harness-conformance/tests/fake_replay.rs
git commit -m "feat(harness-conformance): harness-fake follows the unit state machine; replay its transcripts through fleet-core"
```

### Task 10: `harness-fake` on 0.2, and the kit's 0.2 cases

**Files:**
- Modify: `crates/harness-conformance/src/bin/harness-fake.rs`, `crates/harness-conformance/src/cases.rs`
- Modify: `crates/harness-conformance/tests/detects_v02.rs`, `crates/harness-conformance/tests/fake_conforms.rs`, `crates/harness-conformance/tests/cli.rs`
- Create: `crates/harness-conformance/tests/fake_v02.rs`
- Test: `tests/fake_v02.rs`, `tests/detects_v02.rs`

**Interfaces:**
- Consumes: the 0.2 types (Tasks 2 to 4), `file_sha256` and `bundle_hash` (Task 5), the transcript helper (Task 9).
- Produces:
  - Kit case `minor_mismatch_refused`, second in `run_all`: the kit offers only version `0.0`, and the harness must refuse with `-32001`. `run_all` now returns seven reports.
  - `harness-fake` declares `holdouts: true`, all six `controls`, `network: open`, one priced profile named `fake`, `kinds: [build]` and no presets; emits a `started` and a `finished` stage event for each of the seven stages; sends a freeze payload with `oracle_frozen` whose hashes agree with the gate request and the evidence; re-enters at Green on `resume { oracle_frozen: true }`.
  - Modes `needs_human` (ends `needs_human` with a `scope_request` stop from Green, then exits), `needs_human_without_stop`, `linger_after_stop`, `major_only`.

The four 0.2 behaviours and where each is proved:

| Behaviour | Proved by |
|---|---|
| A `needs_human` result is accepted and the process exits | `the_kit_accepts_a_needs_human_result`, `needs_human_mode_stops_from_green_with_a_scope_request_and_the_process_exits`; the kit refuses the two ways of getting it wrong in `needs_human_without_a_stop_is_caught` and `a_process_that_lingers_after_a_stop_for_a_human_is_caught` |
| A `stage` event is accepted and is not a violation | `a_fresh_unit_reports_all_seven_stages_in_order` proves the events are sent; `conformant_fake_passes_every_case` (unchanged) proves the kit passes a harness that sends them |
| A minor-version mismatch is refused | The new case, with `a_harness_that_checks_only_the_major_is_caught_by_the_minor_case_alone` |
| `oracle_frozen` with a freeze payload is accepted | `oracle_frozen_carries_a_freeze_that_agrees_with_the_gate_and_the_evidence`, and every kit case |

Two existing assertions count the kit's cases, so they change here and only here: the list in `tests/fake_conforms.rs` and the summary line in `tests/cli.rs`.

- [ ] **Step 1: Write the failing tests**

Create `crates/harness-conformance/tests/fake_v02.rs`:

```rust
//! What `harness-fake` adds for protocol 0.2: capabilities, stage events, the freeze payload, a
//! stop for a human, and where a resumed unit re-enters.

mod common;

use common::{order, run_fake, Transcript, Wire};
use harness_conformance::{run_all, CaseOutcome, KitConfig};
use harness_protocol::{
    bundle_hash, ControlKind, GateRequest, Network, Observation, OracleFreeze, Outcome, Stage,
    StageStatus, StopReason, Tier, UnitEvent, UnitKind,
};
use std::time::Duration;

fn stages(t: &Transcript) -> Vec<(Stage, StageStatus)> {
    t.wire
        .iter()
        .filter_map(|w| match w {
            Wire::Event(UnitEvent::Stage { stage, status, .. }) => Some((*stage, *status)),
            _ => None,
        })
        .collect()
}

/// Each stage in `order`, started and then finished.
fn started_and_finished(order: &[Stage]) -> Vec<(Stage, StageStatus)> {
    order
        .iter()
        .flat_map(|s| [(*s, StageStatus::Started), (*s, StageStatus::Finished)])
        .collect()
}

fn freeze(t: &Transcript) -> OracleFreeze {
    t.observations()
        .into_iter()
        .find_map(|o| match o {
            Observation::OracleFrozen { freeze } => freeze,
            _ => None,
        })
        .expect("oracle_frozen carries a freeze payload")
}

fn is_sha256(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

#[test]
fn the_fake_declares_the_0_2_capabilities() {
    let caps = run_fake("conformant", &order(Tier::T1, 1, None)).capabilities;
    assert!(caps.holdouts);
    assert_eq!(
        caps.controls,
        [
            ControlKind::Scope,
            ControlKind::Protected,
            ControlKind::Secrets,
            ControlKind::Dependencies,
            ControlKind::EidosGates,
            ControlKind::Oracle,
        ]
    );
    assert_eq!(caps.network, Some(Network::Open));
    assert_eq!(caps.profiles.len(), 1);
    assert!(caps.profiles[0].priced);
    assert_eq!(caps.kinds, [UnitKind::Build]);
    assert!(caps.presets.is_empty());
}

#[test]
fn a_fresh_unit_reports_all_seven_stages_in_order() {
    use Stage::*;
    let t = run_fake("conformant", &order(Tier::T1, 1, None));
    assert_eq!(
        stages(&t),
        started_and_finished(&[Provision, Red, Plan, Green, Check, Review, Deliver])
    );
    assert_eq!(t.result.outcome, Outcome::PrOpen);
}

#[test]
fn each_extra_review_round_repeats_green_check_and_review() {
    use Stage::*;
    let t = run_fake("conformant", &order(Tier::T1, 2, None));
    assert_eq!(
        stages(&t),
        started_and_finished(&[
            Provision, Red, Plan, Green, Check, Review, Green, Check, Review, Deliver
        ])
    );
}

#[test]
fn a_unit_resumed_after_its_freeze_re_enters_at_green() {
    use Stage::*;
    for tier in [Tier::T1, Tier::T2, Tier::T3] {
        let t = run_fake("conformant", &order(tier, 1, Some(true)));
        assert_eq!(
            stages(&t),
            started_and_finished(&[Provision, Green, Check, Review, Deliver]),
            "{tier:?}"
        );
    }
}

#[test]
fn oracle_frozen_carries_a_freeze_that_agrees_with_the_gate_and_the_evidence() {
    for tier in [Tier::T1, Tier::T2, Tier::T3] {
        let t = run_fake("conformant", &order(tier, 1, None));
        let freeze = freeze(&t);
        assert!(!freeze.frozen_files.is_empty() && !freeze.frozen_ids.is_empty());
        assert!(freeze.frozen_files.iter().all(|f| is_sha256(&f.sha256)));
        assert!(!freeze.holdout_ids.is_empty());
        assert!(freeze.holdout_bundle_path.is_some());
        assert!(is_sha256(freeze.holdout_hash.as_deref().unwrap()));

        let visible = bundle_hash(&freeze.frozen_files);
        let evidence = t
            .result
            .evidence
            .as_ref()
            .expect("pr_open carries evidence");
        assert_eq!(evidence.oracle_hash.as_deref(), Some(visible.as_str()));
        let passed = &evidence.test_report.as_ref().unwrap().ids_passed;
        for id in freeze.frozen_ids.iter().chain(&freeze.holdout_ids) {
            assert!(
                passed.contains(id),
                "{tier:?}: `{id}` is not reported passed"
            );
        }

        let gate = t.wire.iter().find_map(|w| match w {
            Wire::Gate(GateRequest::Oracle {
                test_files,
                hash,
                holdout_files,
                holdout_hash,
                ..
            }) => Some((test_files, hash, holdout_files, holdout_hash)),
            _ => None,
        });
        match (tier, gate) {
            (Tier::T1, None) => {}
            (Tier::T1, Some(_)) => panic!("a gate at T1"),
            (_, None) => panic!("{tier:?}: no gate"),
            (_, Some((test_files, hash, holdout_files, holdout_hash))) => {
                let frozen: Vec<&String> = freeze.frozen_files.iter().map(|f| &f.path).collect();
                assert_eq!(test_files.iter().collect::<Vec<_>>(), frozen);
                assert_eq!(*hash, visible);
                assert!(!holdout_files.is_empty());
                assert_eq!(*holdout_hash, freeze.holdout_hash);
            }
        }
    }
}

#[test]
fn a_unit_resumed_before_its_freeze_reports_the_same_freeze_again() {
    let fresh = run_fake("conformant", &order(Tier::T2, 1, None));
    let resumed = run_fake("conformant", &order(Tier::T2, 1, Some(false)));
    assert_eq!(freeze(&resumed), freeze(&fresh));
    assert_eq!(resumed.gates(), 1);
}

#[test]
fn review_evidence_counts_rounds_from_one_in_this_process() {
    let t = run_fake("blockers", &order(Tier::T1, 1, Some(true)));
    let review = t.result.evidence.unwrap().review.unwrap();
    assert_eq!((review.rounds, review.prior_rounds), (2, 0));
}

#[test]
fn needs_human_mode_stops_from_green_with_a_scope_request_and_the_process_exits() {
    for tier in [Tier::T1, Tier::T2] {
        let t = run_fake("needs_human", &order(tier, 1, None));
        assert_eq!(t.result.outcome, Outcome::NeedsHuman);
        assert!(t.result.evidence.is_none() && t.result.failure.is_none());
        let stop = t.result.stop.as_ref().expect("needs_human carries a stop");
        assert_eq!(stop.reason, StopReason::ScopeRequest);
        assert_eq!(stop.request, ["src/extra.rs"]);
        assert!(
            t.exited_cleanly,
            "{tier:?}: a stop for a human ends the process"
        );
        // The unit is past its freeze and in Green: an agent-active phase.
        assert_eq!(
            stages(&t).last(),
            Some(&(Stage::Green, StageStatus::Started))
        );
        assert!(matches!(
            t.observations().last(),
            Some(Observation::OracleFrozen { .. })
        ));
    }
}

#[test]
fn the_kit_accepts_a_needs_human_result() {
    let mut cfg = KitConfig::new(vec![env!("CARGO_BIN_EXE_harness-fake").to_string()]);
    cfg.env = vec![
        ("HARNESS_FAKE_MODE".into(), "needs_human".into()),
        ("HARNESS_FAKE_STEP_MS".into(), "20".into()),
    ];
    cfg.wall_clock = Duration::from_secs(10);
    cfg.grace = Duration::from_secs(2);
    for report in run_all(&cfg) {
        assert_eq!(report.outcome, CaseOutcome::Pass, "case `{}`", report.name);
    }
}
```

Append to `crates/harness-conformance/tests/detects_v02.rs`:

```rust
#[test]
fn needs_human_without_a_stop_is_caught() {
    assert_detects(
        "needs_human_without_stop",
        "happy_path_t1",
        Violation::InvalidResult("needs_human without stop".into()),
    );
}

#[test]
fn a_process_that_lingers_after_a_stop_for_a_human_is_caught() {
    assert_detects(
        "linger_after_stop",
        "happy_path_t1",
        Violation::DidNotExitAfterResult,
    );
}

#[test]
fn a_harness_that_checks_only_the_major_is_caught_by_the_minor_case_alone() {
    let reports = run_all(&fake_config("major_only"));
    assert_eq!(
        *outcome_of(&reports, "version_mismatch_refused"),
        CaseOutcome::Pass,
        "all reports: {reports:#?}"
    );
    assert_eq!(
        *outcome_of(&reports, "minor_mismatch_refused"),
        CaseOutcome::Fail(Violation::VersionMismatchAccepted),
        "all reports: {reports:#?}"
    );
}

#[test]
fn accepting_any_version_fails_the_minor_case_too() {
    assert_detects(
        "accept_any_version",
        "minor_mismatch_refused",
        Violation::VersionMismatchAccepted,
    );
}
```

Update the two assertions that count cases:

```diff
--- a/crates/harness-conformance/tests/cli.rs
+++ b/crates/harness-conformance/tests/cli.rs
@@ -28,7 +28,7 @@ fn run_against_fake(mode: &str) -> (Option<i32>, String) {
 fn exits_zero_against_the_conformant_fake() {
     let (code, stdout) = run_against_fake("conformant");
     assert_eq!(code, Some(0), "{stdout}");
-    assert!(stdout.contains("6 passed, 0 skipped, 0 failed"), "{stdout}");
+    assert!(stdout.contains("7 passed, 0 skipped, 0 failed"), "{stdout}");
 }
 
 #[test]
--- a/crates/harness-conformance/tests/fake_conforms.rs
+++ b/crates/harness-conformance/tests/fake_conforms.rs
@@ -36,6 +36,7 @@ fn run_all_covers_every_case_in_order() {
         names,
         [
             "version_mismatch_refused",
+            "minor_mismatch_refused",
             "happy_path_t1",
             "gate_approved_t2",
             "gate_rejected_t2",
```

- [ ] **Step 2: Run them and watch them fail**

Run: `cargo test -p harness-conformance --test fake_v02 --test detects_v02 --test fake_conforms --test cli`
Expected: failures in all four. `fake_v02`: `the_fake_declares_the_0_2_capabilities` and the stage, freeze and `needs_human` tests fail. `detects_v02`: the four new tests fail; the two version tests panic with `case exists in run_all`, and the two `needs_human` tests see `left: Pass`, because the fake does not know those modes yet and runs its conformant script. `fake_conforms`: `run_all_covers_every_case_in_order` fails on the missing name. `cli`: `exits_zero_against_the_conformant_fake` fails because the summary still says `6 passed`.

- [ ] **Step 3: Add the kit case**

In `crates/harness-conformance/src/cases.rs`, replace the function `version_mismatch_refused` with:

```rust
/// Send an `initialize` that accepts only `version`. The harness must refuse it with `-32001`.
fn refuses_version(cfg: &KitConfig, name: &'static str, version: &str) -> CaseReport {
    let mut session = match Session::spawn(cfg) {
        Ok(s) => s,
        Err(e) => return fail(name, Violation::SpawnFailed(e)),
    };
    session.send(&RpcMessage::request(
        INIT_ID,
        method::INITIALIZE,
        &InitializeParams {
            protocol_version: version.into(),
            accepted_versions: vec![version.into()],
        },
    ));
    match session.recv(cfg.grace) {
        Recv::Message(msg) => match msg.kind() {
            MessageKind::ErrorResponse { id: INIT_ID, error }
                if error.code == error_code::PROTOCOL_VERSION_UNSUPPORTED =>
            {
                pass(name)
            }
            MessageKind::Response { id: INIT_ID } => fail(name, Violation::VersionMismatchAccepted),
            _ => fail(name, Violation::InvalidMessage),
        },
        Recv::Malformed(line) => fail(name, Violation::Malformed { line }),
        Recv::Eof | Recv::Timeout => fail(name, Violation::NoInitializeResponse),
    }
}

/// A harness must refuse `initialize` from a different protocol major with `-32001`.
pub(crate) fn version_mismatch_refused(cfg: &KitConfig) -> CaseReport {
    refuses_version(cfg, "version_mismatch_refused", "99.0")
}

/// While the major is 0, a control plane on another minor is refused the same way. No harness
/// speaks `0.0`, so every harness must refuse it.
pub(crate) fn minor_mismatch_refused(cfg: &KitConfig) -> CaseReport {
    refuses_version(cfg, "minor_mismatch_refused", "0.0")
}
```

and replace `run_all` with:

```rust
pub fn run_all(cfg: &KitConfig) -> Vec<CaseReport> {
    vec![
        version_mismatch_refused(cfg),
        minor_mismatch_refused(cfg),
        happy_path_t1(cfg),
        gate_approved_t2(cfg),
        gate_rejected_t2(cfg),
        halt(cfg),
        abandon(cfg),
    ]
}
```

- [ ] **Step 4: Give `harness-fake` its 0.2 imports, modes and capabilities**

In `crates/harness-conformance/src/bin/harness-fake.rs`, replace the `use harness_protocol::{ … };` statement with:

```rust
use harness_protocol::{
    bundle_hash, error_code, file_sha256, method, negotiate, read_message, write_message,
    ArtifactKind, Capabilities, ControlKind, ControlResult, ControlStatus, CostBasis, Delivery,
    DeliveryEvidence, Empty, ErrorScope, Evidence, Failure, FrozenFile, GateKind, GateReply,
    GateRequest, HarnessInfo, InitializeParams, InitializeResult, Isolation, MessageKind, Metering,
    Network, Observation, OracleFreeze, Outcome, ProfileInfo, ReviewEvidence, RpcMessage, Severity,
    Stage, StageStatus, Stop, StopReason, TestReport, TestRun, UnitEvent, UnitKind, UnitResult,
    WorkOrder, MAX_LINE_BYTES, PROTOCOL_VERSION,
};
```

Replace the `Mode` enum and the function `mode` with (four new modes at the end of each):

```rust
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Conformant,
    Crash,
    Malformed,
    EventAfterResult,
    SilentMetering,
    Hang,
    UnknownMethod,
    AcceptAnyVersion,
    SkipGate,
    IgnoreGateRejection,
    IgnoreHalt,
    FailBeforeGate,
    AckHaltLinger,
    SilentHaltExit,
    ResultWithoutEvents,
    InvalidUtf8,
    LongLine,
    NoStartAck,
    SilentStart,
    WrongVersionReply,
    NoChangeWithoutEvidence,
    Blockers,
    NeedsHuman,
    NeedsHumanWithoutStop,
    LingerAfterStop,
    MajorOnly,
}

fn mode() -> Mode {
    match std::env::var("HARNESS_FAKE_MODE").as_deref() {
        Ok("result_without_events") => Mode::ResultWithoutEvents,
        Ok("fail_before_gate") => Mode::FailBeforeGate,
        Ok("ack_halt_linger") => Mode::AckHaltLinger,
        Ok("silent_halt_exit") => Mode::SilentHaltExit,
        Ok("crash") => Mode::Crash,
        Ok("malformed") => Mode::Malformed,
        Ok("event_after_result") => Mode::EventAfterResult,
        Ok("silent_metering") => Mode::SilentMetering,
        Ok("hang") => Mode::Hang,
        Ok("unknown_method") => Mode::UnknownMethod,
        Ok("accept_any_version") => Mode::AcceptAnyVersion,
        Ok("skip_gate") => Mode::SkipGate,
        Ok("ignore_gate_rejection") => Mode::IgnoreGateRejection,
        Ok("ignore_halt") => Mode::IgnoreHalt,
        Ok("invalid_utf8") => Mode::InvalidUtf8,
        Ok("long_line") => Mode::LongLine,
        Ok("no_start_ack") => Mode::NoStartAck,
        Ok("silent_start") => Mode::SilentStart,
        Ok("wrong_version_reply") => Mode::WrongVersionReply,
        Ok("no_change_without_evidence") => Mode::NoChangeWithoutEvidence,
        Ok("blockers") => Mode::Blockers,
        Ok("needs_human") => Mode::NeedsHuman,
        Ok("needs_human_without_stop") => Mode::NeedsHumanWithoutStop,
        Ok("linger_after_stop") => Mode::LingerAfterStop,
        Ok("major_only") => Mode::MajorOnly,
        _ => Mode::Conformant,
    }
}
```

Replace the constants `GATE_REQUEST_ID` and `FAKE_HEAD_SHA` and the function `capabilities` with:

```rust
const GATE_REQUEST_ID: u64 = 1_000;
const FAKE_HEAD_SHA: &str = "0123456789abcdef0123456789abcdef01234567";
const CONTROLS: [ControlKind; 6] = [
    ControlKind::Scope,
    ControlKind::Protected,
    ControlKind::Secrets,
    ControlKind::Dependencies,
    ControlKind::EidosGates,
    ControlKind::Oracle,
];

fn capabilities() -> Capabilities {
    Capabilities {
        isolation: Isolation::None,
        metering: Metering::Usd,
        gates: vec![GateKind::Oracle],
        delivery: Delivery::Bundle,
        resume: true,
        halt: true,
        holdouts: true,
        controls: CONTROLS.to_vec(),
        network: Some(Network::Open),
        profiles: vec![ProfileInfo {
            name: "fake".into(),
            priced: true,
        }],
        kinds: vec![UnitKind::Build],
        // A scripted harness applies no stack preset.
        presets: Vec::new(),
    }
}
```

In `main`, replace the `if mode != Mode::AcceptAnyVersion && !negotiate(…) { … }` block with:

```rust
    let acceptable = match mode {
        Mode::AcceptAnyVersion => true,
        // What a 0.1 harness did: any 0.x control plane is accepted.
        Mode::MajorOnly => params.accepted().iter().any(|v| v.starts_with("0.")),
        _ => negotiate(&params.accepted(), PROTOCOL_VERSION),
    };
    if !acceptable {
        let message = format!("harness-fake speaks protocol {PROTOCOL_VERSION}");
        send(&RpcMessage::error(
            id,
            error_code::PROTOCOL_VERSION_UNSUPPORTED,
            message,
        ));
        return;
    }
```

- [ ] **Step 5: Rewrite the unit script**

Replace the function `run_unit` with these four functions:

```rust
fn stage(stage: Stage, status: StageStatus) {
    event(UnitEvent::Stage {
        stage,
        status,
        detail: None,
    });
}

/// The scripted oracle. It is the same on every run of a unit, so a resumed unit that freezes
/// again reports the same hashes.
fn fake_freeze(order: &WorkOrder) -> OracleFreeze {
    let holdout_files = [FrozenFile {
        path: "tests/holdout/fake.holdout.test.js".into(),
        sha256: file_sha256(b"scripted holdout test\n"),
    }];
    OracleFreeze {
        frozen_files: vec![FrozenFile {
            path: "tests/fake.test.js".into(),
            sha256: file_sha256(b"scripted visible test\n"),
        }],
        frozen_ids: vec!["fake.test.js::ac1_visible".into()],
        holdout_bundle_path: Some(format!("{}.holdouts.tar", order.unit_id)),
        holdout_hash: Some(bundle_hash(&holdout_files)),
        holdout_ids: vec!["fake.holdout.test.js::ac1_holdout".into()],
    }
}

fn metric(pace: Duration) -> UnitEvent {
    UnitEvent::Metric {
        tokens_in: 1_200,
        tokens_out: 300,
        cost_usd: 0.02,
        elapsed_ms: pace.as_millis() as u64,
        cost_basis: Some(CostBasis::Priced),
        stage: Some(Stage::Green),
        role: Some("builder".into()),
        adapter: Some("fake".into()),
        model: Some("scripted".into()),
    }
}

fn run_unit(mode: Mode, order: &WorkOrder, rx: &Receiver<RpcMessage>) {
    let pace = pace();

    if mode == Mode::ResultWithoutEvents {
        let result = UnitResult {
            outcome: Outcome::Failed,
            evidence: None,
            failure: Some(Failure {
                scope: ErrorScope::Agent,
                detail: "nothing to do".into(),
            }),
            stop: None,
        };
        send(&RpcMessage::notification(method::UNIT_RESULT, &result));
        return;
    }

    // Provision. `provisioned` is not held back for anything slower.
    stage(Stage::Provision, StageStatus::Started);
    observe(Observation::Provisioned);
    stage(Stage::Provision, StageStatus::Finished);

    if mode == Mode::FailBeforeGate && order.tier.requires_oracle() {
        let result = UnitResult {
            outcome: Outcome::Failed,
            evidence: None,
            failure: Some(Failure {
                scope: ErrorScope::Agent,
                detail: "failed before gate".into(),
            }),
            stop: None,
        };
        send(&RpcMessage::notification(method::UNIT_RESULT, &result));
        return;
    }

    if mode == Mode::NoChangeWithoutEvidence {
        let result = UnitResult {
            outcome: Outcome::NoChange,
            evidence: None,
            failure: None,
            stop: None,
        };
        send(&RpcMessage::notification(method::UNIT_RESULT, &result));
        return;
    }

    step!(rx, pace);

    match mode {
        Mode::Crash => std::process::exit(3),
        Mode::Hang => loop {
            std::thread::sleep(Duration::from_secs(3_600));
        },
        Mode::Malformed => {
            let mut out = io::stdout().lock();
            let _ = out.write_all(b"this is not json\n");
            let _ = out.flush();
        }
        Mode::UnknownMethod => send(&RpcMessage::notification("unit/whatever", &Empty {})),
        Mode::InvalidUtf8 => {
            let mut out = io::stdout().lock();
            let _ = out.write_all(&[0xff, 0xfe, b'\n']);
            let _ = out.flush();
        }
        Mode::LongLine => {
            let mut out = io::stdout().lock();
            let _ = out.write_all(&vec![b'x'; MAX_LINE_BYTES + 1]);
            let _ = out.write_all(b"\n");
            let _ = out.flush();
        }
        _ => {}
    }

    let freeze = fake_freeze(order);
    let oracle_hash = bundle_hash(&freeze.frozen_files);

    // Red, then Plan. Every tier freezes its oracle; T2/T3 then wait at the gate. A unit resumed
    // after its oracle was frozen skips both stages and re-enters at Green.
    let resumed_past_oracle = order.resume.as_ref().is_some_and(|r| r.oracle_frozen);
    if !resumed_past_oracle {
        stage(Stage::Red, StageStatus::Started);
        observe(Observation::OracleFrozen {
            freeze: Some(freeze.clone()),
        });
        if order.tier.requires_oracle() && mode != Mode::SkipGate {
            let gate = GateRequest::Oracle {
                test_files: freeze.frozen_files.iter().map(|f| f.path.clone()).collect(),
                hash: oracle_hash.clone(),
                summary: "one scripted test and one scripted holdout".into(),
                holdout_files: vec!["tests/holdout/fake.holdout.test.js".into()],
                holdout_hash: freeze.holdout_hash.clone(),
            };
            send(&RpcMessage::request(
                GATE_REQUEST_ID,
                method::GATE_REQUEST,
                &gate,
            ));
            match await_gate(rx) {
                None => return,
                Some(false) if mode != Mode::IgnoreGateRejection => {
                    let result = UnitResult {
                        outcome: Outcome::Failed,
                        evidence: None,
                        failure: Some(Failure {
                            scope: ErrorScope::Agent,
                            detail: "oracle rejected".into(),
                        }),
                        stop: None,
                    };
                    send(&RpcMessage::notification(method::UNIT_RESULT, &result));
                    return;
                }
                Some(_) => {}
            }
        }
        stage(Stage::Red, StageStatus::Finished);
        stage(Stage::Plan, StageStatus::Started);
        stage(Stage::Plan, StageStatus::Finished);
    }

    // A stop for a human: sent from Green, and then the process ends.
    if matches!(
        mode,
        Mode::NeedsHuman | Mode::NeedsHumanWithoutStop | Mode::LingerAfterStop
    ) {
        stage(Stage::Green, StageStatus::Started);
        event(metric(pace));
        let result = UnitResult {
            outcome: Outcome::NeedsHuman,
            evidence: None,
            failure: None,
            stop: (mode != Mode::NeedsHumanWithoutStop).then(|| Stop {
                reason: StopReason::ScopeRequest,
                detail: "the scripted builder asked for one more file".into(),
                request: vec!["src/extra.rs".into()],
            }),
        };
        send(&RpcMessage::notification(method::UNIT_RESULT, &result));
        if mode == Mode::LingerAfterStop {
            loop {
                std::thread::sleep(Duration::from_secs(3_600));
            }
        }
        return;
    }

    // Green, Check and Review, until the review gate holds: green checks, no unresolved blocker,
    // and the round floor reached. Rounds count from 1 in every process.
    let floor = order.caps.min_review_rounds.max(1);
    let mut round = 0;
    loop {
        round += 1;
        stage(Stage::Green, StageStatus::Started);
        if mode != Mode::SilentMetering {
            event(metric(pace));
        }
        observe(Observation::BuildFinished);
        stage(Stage::Green, StageStatus::Finished);
        step!(rx, pace);
        stage(Stage::Check, StageStatus::Started);
        observe(Observation::ChecksPassed);
        stage(Stage::Check, StageStatus::Finished);
        step!(rx, pace);
        stage(Stage::Review, StageStatus::Started);
        let unresolved_blockers = u32::from(mode == Mode::Blockers && round == 1);
        if unresolved_blockers > 0 {
            event(UnitEvent::Finding {
                round,
                severity: Severity::Blocker,
                title: "scripted blocker".into(),
                file: None,
                resolved: false,
            });
        }
        observe(Observation::ReviewFinished {
            round,
            unresolved_blockers,
            checks_green: true,
        });
        stage(Stage::Review, StageStatus::Finished);
        step!(rx, pace);
        if unresolved_blockers == 0 && round >= floor {
            break;
        }
    }

    // Deliver.
    stage(Stage::Deliver, StageStatus::Started);
    event(UnitEvent::Artifact {
        kind: ArtifactKind::Branch,
        reference: order.branch.clone(),
    });
    stage(Stage::Deliver, StageStatus::Finished);

    let mut ids_passed = freeze.frozen_ids.clone();
    ids_passed.extend(freeze.holdout_ids.iter().cloned());
    let result = UnitResult {
        outcome: Outcome::PrOpen,
        evidence: Some(Evidence {
            branch: order.branch.clone(),
            head_sha: FAKE_HEAD_SHA.into(),
            delivery: DeliveryEvidence::Bundle {
                bundle_path: format!("{}.bundle", order.unit_id),
            },
            pr: None,
            test: TestRun {
                command: order.test_cmd.clone(),
                exit_code: 0,
            },
            oracle_hash: Some(oracle_hash),
            spec_hash: order.spec.as_ref().map(|s| s.signed_hash.clone()),
            map: None,
            test_report: Some(TestReport { ids_passed }),
            controls: CONTROLS
                .iter()
                .map(|&name| ControlResult {
                    name,
                    status: ControlStatus::Passed,
                    detail: "scripted".into(),
                })
                .collect(),
            review: Some(ReviewEvidence {
                rounds: round,
                prior_rounds: 0,
                verdicts: Vec::new(),
            }),
        }),
        failure: None,
        stop: None,
    };
    send(&RpcMessage::notification(method::UNIT_RESULT, &result));

    if mode == Mode::EventAfterResult {
        event(UnitEvent::Log {
            stream: harness_protocol::LogStream::System,
            line: "after result".into(),
        });
    }
}
```

Stage events never sit between `oracle_frozen` and the gate request: `stage(Red, Finished)` comes after the gate is answered. Task 9's test `at_t2_and_t3_the_gate_follows_oracle_frozen_with_no_observation_between` guards that.

- [ ] **Step 6: Run the tests**

Run: `cargo test -p harness-conformance`
Expected: 57 passed, 0 failed: 1 unit, 4 `cli`, 10 `detects_v02`, 16 `detects_violations`, 2 `fake_conforms`, 10 `fake_peer`, 1 `fake_replay`, 2 `fake_smoke`, 9 `fake_v02`, 2 `session_exit`.

Run: `cargo build -p harness-conformance --bins && target/debug/harness-conformance --wall-clock-secs 30 --grace-secs 5 -- target/debug/harness-fake`
Expected: seven `PASS` lines and `7 passed, 0 skipped, 0 failed`.

- [ ] **Step 7: Commit**

```bash
cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings
git add crates/harness-conformance/src/bin/harness-fake.rs crates/harness-conformance/src/cases.rs crates/harness-conformance/tests/fake_v02.rs crates/harness-conformance/tests/detects_v02.rs crates/harness-conformance/tests/fake_conforms.rs crates/harness-conformance/tests/cli.rs
git commit -m "feat(harness-conformance): harness-fake speaks 0.2; the kit refuses a minor mismatch and accepts needs_human"
```

### Task 11: The kit's `--work-order` override

Issue #87. The kit's work order names `example.invalid`. A scripted harness does not care; a real one has to clone the repository it is given.

**Files:**
- Modify: `crates/harness-conformance/src/fixtures.rs`, `crates/harness-conformance/src/lib.rs`, `crates/harness-conformance/src/cases.rs`, `crates/harness-conformance/src/bin/harness-conformance.rs`, `crates/harness-conformance/src/bin/harness-fake.rs`
- Modify: `crates/harness-conformance/tests/cli.rs`
- Create: `crates/harness-conformance/README.md`
- Test: `crates/harness-conformance/src/fixtures.rs` (inline), `tests/cli.rs`

**Interfaces:**
- Consumes: `WorkOrder`; `serde_json` as a dependency of the kit (request R3).
- Produces:
  - `harness-conformance --work-order <JSON | PATH>`: a value that starts with `{` is the work order; anything else is a file that holds it.
  - `pub fn parse_work_order(json: &str) -> Result<WorkOrder, String>`
  - `KitConfig { …, pub work_order: Option<WorkOrder> }`
  - `pub(crate) fn order_for(cfg: &KitConfig, tier: Tier) -> WorkOrder`
  - `harness-fake` writes `harness-fake unit <unit_id> on <repo.slug> branch <branch> tier <tier>` to stderr when a unit starts. The CLI test reads it to prove the override arrived.

Decisions the issue leaves open, settled here:

- **A string or a file: both.** Quoting JSON on a Windows command line is unreliable, so a file must work; the issue says `<json>`, so a string must too.
- **The case still chooses the tier.** Each case tests one tier's behaviour, so `tier` in the override is replaced per case. Nothing else is.
- **Validation** is the wire type's own (a missing or mistyped field), plus what the kit needs: `unit_id`, `branch`, `repo.url`, `repo.slug` and `repo.base_branch` are not blank; `caps.usd` is above 0; `resume` is absent, because the kit starts every unit fresh.
- **A bad value exits 2**, the kit's usage-error code, with the reason on stderr, before any case runs.
- **No case compares against the fixture's values**, so a replaced order cannot make a case assert against the old one. `fixtures.rs` stays the only place the fixture is built.

- [ ] **Step 1: Check the dependency is present**

Run: `cargo tree -p harness-conformance -e normal --depth 1`
Expected: two dependency lines, `harness-protocol` and `serde_json`. If `serde_json` is missing, request R3 has not landed: stop and report.

- [ ] **Step 2: Write the failing unit tests**

At the end of `crates/harness-conformance/src/fixtures.rs`, add:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use harness_protocol::Resume;

    fn json_of(order: &WorkOrder) -> String {
        serde_json::to_string(order).unwrap()
    }

    #[test]
    fn the_fixture_is_itself_a_valid_override() {
        let fixture = work_order(Tier::T2);
        assert_eq!(parse_work_order(&json_of(&fixture)), Ok(fixture));
    }

    #[test]
    fn a_byte_order_mark_and_surrounding_whitespace_are_ignored() {
        let fixture = work_order(Tier::T1);
        let text = format!("\u{feff}\n  {}\n", json_of(&fixture));
        assert_eq!(parse_work_order(&text), Ok(fixture));
    }

    #[test]
    fn text_that_is_not_a_work_order_is_refused_with_the_parser_reason() {
        let err = parse_work_order("{not json").unwrap_err();
        assert!(err.starts_with("not a work order: "), "{err}");
        let err = parse_work_order(r#"{"unit_id": "u1"}"#).unwrap_err();
        assert!(err.contains("missing field"), "{err}");
        let err = parse_work_order("").unwrap_err();
        assert!(err.starts_with("not a work order: "), "{err}");
    }

    #[test]
    fn each_field_the_kit_needs_is_checked() {
        let mut blank_id = work_order(Tier::T1);
        blank_id.unit_id = "  ".into();
        let mut no_branch = work_order(Tier::T1);
        no_branch.branch = String::new();
        let mut no_url = work_order(Tier::T1);
        no_url.repo.url = String::new();
        let mut no_slug = work_order(Tier::T1);
        no_slug.repo.slug = String::new();
        let mut no_base = work_order(Tier::T1);
        no_base.repo.base_branch = String::new();
        let mut free = work_order(Tier::T1);
        free.caps.usd = 0.0;
        let mut resumed = work_order(Tier::T1);
        resumed.resume = Some(Resume {
            oracle_frozen: true,
        });
        for (order, want) in [
            (blank_id, "unit_id is empty"),
            (no_branch, "branch is empty"),
            (no_url, "repo.url is empty"),
            (no_slug, "repo.slug is empty"),
            (no_base, "repo.base_branch is empty"),
            (free, "caps.usd must be above 0, got 0"),
            (
                resumed,
                "resume must be absent: the kit starts every unit fresh",
            ),
        ] {
            assert_eq!(parse_work_order(&json_of(&order)), Err(want.to_string()));
        }
    }

    #[test]
    fn a_case_sets_the_tier_and_keeps_everything_else_from_the_override() {
        let mut over = work_order(Tier::T3);
        over.unit_id = "override-7".into();
        over.repo.slug = "adbarc92/command-center-agent-sandbox".into();
        let mut cfg = KitConfig::new(vec!["harness".into()]);
        assert_eq!(order_for(&cfg, Tier::T2), work_order(Tier::T2));

        cfg.work_order = Some(over.clone());
        let sent = order_for(&cfg, Tier::T2);
        assert_eq!(sent.tier, Tier::T2);
        assert_eq!(
            WorkOrder {
                tier: Tier::T3,
                ..sent
            },
            over
        );
    }
}
```

- [ ] **Step 3: Run them and watch them fail**

Run: `cargo test -p harness-conformance --lib fixtures::`
Expected: the build fails with E0425 (cannot find function `parse_work_order`; cannot find function `order_for`).

- [ ] **Step 4: Implement the override in the library**

Replace everything above the test module in `crates/harness-conformance/src/fixtures.rs` with:

```rust
//! The work order the kit sends: a synthetic one that names no real repository, unless the caller
//! supplies its own.

use crate::KitConfig;
use harness_protocol::{Caps, Repo, Tier, WorkItem, WorkItemKind, WorkOrder};

pub fn work_order(tier: Tier) -> WorkOrder {
    WorkOrder {
        unit_id: "conformance-1".into(),
        work_item: WorkItem {
            kind: WorkItemKind::RoadmapItem,
            reference: "conformance#kit".into(),
            fingerprint: None,
        },
        tier,
        task: "Conformance kit synthetic unit. Do not modify any repository.".into(),
        repo: Repo {
            url: "https://example.invalid/conformance.git".into(),
            slug: "example/conformance".into(),
            base_branch: "main".into(),
        },
        branch: "agent/conformance-1".into(),
        test_cmd: "true".into(),
        caps: Caps {
            usd: 0.10,
            wall_clock_secs: 30,
            min_review_rounds: 1,
        },
        resume: None,
        kind: None,
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

/// The work order a case sends: the caller's override if there is one, otherwise the fixture.
/// Either way the case chooses the tier, because the tier is what the case tests.
pub(crate) fn order_for(cfg: &KitConfig, tier: Tier) -> WorkOrder {
    match &cfg.work_order {
        Some(order) => WorkOrder {
            tier,
            ..order.clone()
        },
        None => work_order(tier),
    }
}

/// Parse a `--work-order` value and check that the kit can use it. `Err` is the reason.
pub fn parse_work_order(json: &str) -> Result<WorkOrder, String> {
    // Editors on Windows often write a byte-order mark, which is not JSON.
    let json = json.strip_prefix('\u{feff}').unwrap_or(json);
    let order: WorkOrder =
        serde_json::from_str(json).map_err(|e| format!("not a work order: {e}"))?;
    for (name, value) in [
        ("unit_id", &order.unit_id),
        ("branch", &order.branch),
        ("repo.url", &order.repo.url),
        ("repo.slug", &order.repo.slug),
        ("repo.base_branch", &order.repo.base_branch),
    ] {
        if value.trim().is_empty() {
            return Err(format!("{name} is empty"));
        }
    }
    if order.caps.usd <= 0.0 {
        return Err(format!("caps.usd must be above 0, got {}", order.caps.usd));
    }
    if order.resume.is_some() {
        return Err("resume must be absent: the kit starts every unit fresh".into());
    }
    Ok(order)
}
```

In `crates/harness-conformance/src/lib.rs`:

```diff
--- a/crates/harness-conformance/src/lib.rs
+++ b/crates/harness-conformance/src/lib.rs
@@ -6,10 +6,11 @@ mod fixtures;
 mod session;
 
 pub use cases::run_all;
-pub use fixtures::work_order;
+pub use fixtures::{parse_work_order, work_order};
 pub use session::{Recv, Session};
 
 use harness_protocol::monitor::MonitorViolation;
+use harness_protocol::WorkOrder;
 use std::time::Duration;
 
 #[derive(Debug, Clone)]
@@ -22,6 +23,8 @@ pub struct KitConfig {
     pub wall_clock: Duration,
     /// How long a harness has to answer a message or exit.
     pub grace: Duration,
+    /// Sent in place of the fixture when set. Each case still chooses the tier.
+    pub work_order: Option<WorkOrder>,
 }
 
 impl KitConfig {
@@ -31,6 +34,7 @@ impl KitConfig {
             env: Vec::new(),
             wall_clock: Duration::from_secs(30),
             grace: Duration::from_secs(5),
+            work_order: None,
         }
     }
 }
```

so that `KitConfig` reads:

```rust
#[derive(Debug, Clone)]
pub struct KitConfig {
    /// Program and arguments that start the harness.
    pub command: Vec<String>,
    /// Extra environment for the harness process.
    pub env: Vec<(String, String)>,
    /// Longest one case may run before the harness is killed.
    pub wall_clock: Duration,
    /// How long a harness has to answer a message or exit.
    pub grace: Duration,
    /// Sent in place of the fixture when set. Each case still chooses the tier.
    pub work_order: Option<WorkOrder>,
}

impl KitConfig {
    pub fn new(command: Vec<String>) -> Self {
        Self {
            command,
            env: Vec::new(),
            wall_clock: Duration::from_secs(30),
            grace: Duration::from_secs(5),
            work_order: None,
        }
    }
}
```

In `crates/harness-conformance/src/cases.rs`, send the override instead of the fixture:

```diff
--- a/crates/harness-conformance/src/cases.rs
+++ b/crates/harness-conformance/src/cases.rs
@@ -1,6 +1,6 @@
 //! The conformance cases. Each spawns a fresh harness.
 
-use crate::fixtures::work_order;
+use crate::fixtures::order_for;
 use crate::session::{Recv, Session};
 use crate::{CaseOutcome, CaseReport, KitConfig, Violation};
 use harness_protocol::monitor::{Inbound, ProtocolMonitor};
@@ -293,7 +293,7 @@ pub(crate) fn happy_path_t1(cfg: &KitConfig) -> CaseReport {
         &mut session,
         cfg,
         &caps,
-        &work_order(Tier::T1),
+        &order_for(cfg, Tier::T1),
         GatePolicy::Unexpected,
         None,
     ) {
@@ -331,7 +331,7 @@ pub(crate) fn gate_approved_t2(cfg: &KitConfig) -> CaseReport {
         &mut session,
         cfg,
         &caps,
-        &work_order(Tier::T2),
+        &order_for(cfg, Tier::T2),
         GatePolicy::Approve,
         None,
     ) {
@@ -367,7 +367,7 @@ pub(crate) fn gate_rejected_t2(cfg: &KitConfig) -> CaseReport {
         &mut session,
         cfg,
         &caps,
-        &work_order(Tier::T2),
+        &order_for(cfg, Tier::T2),
         GatePolicy::Reject,
         None,
     ) {
@@ -410,7 +410,14 @@ fn interrupt_case(
     } else {
         (Tier::T1, GatePolicy::Unexpected)
     };
-    match drive(&mut session, cfg, &caps, &work_order(tier), gate, Some(ctl)) {
+    match drive(
+        &mut session,
+        cfg,
+        &caps,
+        &order_for(cfg, tier),
+        gate,
+        Some(ctl),
+    ) {
         Ok(t) if t.result.is_none() => pass(name),
         Ok(t) if !t.interrupt_sent => {
             skipped(name, "unit ended before an interrupt could be delivered")
```

Run: `cargo test -p harness-conformance --lib`
Expected: `test result: ok. 6 passed; 0 failed`.

- [ ] **Step 5: Write the failing CLI tests**

Append to `crates/harness-conformance/tests/cli.rs`:

```rust
// --- `--work-order` ---

use harness_conformance::work_order;
use harness_protocol::{Tier, WorkOrder};

struct Run {
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

fn run_with(flags: &[&str]) -> Run {
    let out = kit()
        .env("HARNESS_FAKE_MODE", "conformant")
        .env("HARNESS_FAKE_STEP_MS", "20")
        .args(["--wall-clock-secs", "10", "--grace-secs", "2"])
        .args(flags)
        .args(["--", env!("CARGO_BIN_EXE_harness-fake")])
        .output()
        .expect("harness-conformance runs");
    Run {
        code: out.status.code(),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    }
}

/// A work order that names the sandbox repository instead of the fixture's `example.invalid`.
fn sandbox_order() -> WorkOrder {
    let mut order = work_order(Tier::T3);
    order.unit_id = "override-7".into();
    order.branch = "agent/override-7".into();
    order.repo.url = "https://github.com/adbarc92/command-center-agent-sandbox.git".into();
    order.repo.slug = "adbarc92/command-center-agent-sandbox".into();
    order
}

#[test]
fn without_the_flag_the_fixture_is_sent() {
    let run = run_with(&[]);
    assert_eq!(run.code, Some(0), "{}", run.stdout);
    assert!(
        run.stderr
            .contains("harness-fake unit conformance-1 on example/conformance"),
        "{}",
        run.stderr
    );
}

#[test]
fn an_inline_work_order_reaches_the_harness_in_every_case() {
    let json = serde_json::to_string(&sandbox_order()).unwrap();
    let run = run_with(&["--work-order", &json]);
    assert_eq!(run.code, Some(0), "{}{}", run.stdout, run.stderr);
    assert!(
        run.stdout.contains("7 passed, 0 skipped, 0 failed"),
        "{}",
        run.stdout
    );
    let units: Vec<&str> = run
        .stderr
        .lines()
        .filter(|l| l.starts_with("harness-fake unit "))
        .collect();
    // Five cases start a unit; the two version cases stop at `initialize`.
    assert_eq!(units.len(), 5, "{}", run.stderr);
    for line in units {
        assert!(
            line.starts_with(
                "harness-fake unit override-7 on adbarc92/command-center-agent-sandbox \
                 branch agent/override-7 tier "
            ),
            "{line}"
        );
        // The override says T3; each case sends the tier it tests.
        assert!(
            line.ends_with("tier T1") || line.ends_with("tier T2"),
            "{line}"
        );
    }
}

#[test]
fn a_work_order_file_is_read_and_a_byte_order_mark_is_ignored() {
    let path = std::env::temp_dir().join(format!("kit-work-order-{}.json", std::process::id()));
    let pretty = serde_json::to_string_pretty(&sandbox_order()).unwrap();
    std::fs::write(&path, format!("\u{feff}{pretty}\n")).unwrap();
    let run = run_with(&["--work-order", path.to_str().unwrap()]);
    let _ = std::fs::remove_file(&path);
    assert_eq!(run.code, Some(0), "{}{}", run.stdout, run.stderr);
    assert!(
        run.stderr.contains("harness-fake unit override-7 on "),
        "{}",
        run.stderr
    );
}

#[test]
fn invalid_json_exits_two_with_the_reason() {
    let run = run_with(&["--work-order", "{not json"]);
    assert_eq!(run.code, Some(2), "{}", run.stdout);
    assert!(
        run.stderr.contains("--work-order: not a work order: "),
        "{}",
        run.stderr
    );
    assert!(run.stdout.is_empty(), "no case may run: {}", run.stdout);
}

#[test]
fn a_work_order_that_fails_validation_exits_two_with_the_reason() {
    let mut order = sandbox_order();
    order.unit_id = String::new();
    let json = serde_json::to_string(&order).unwrap();
    let run = run_with(&["--work-order", &json]);
    assert_eq!(run.code, Some(2), "{}", run.stdout);
    assert!(
        run.stderr.contains("--work-order: unit_id is empty"),
        "{}",
        run.stderr
    );
}

#[test]
fn a_work_order_file_that_does_not_exist_exits_two_with_the_reason() {
    let run = run_with(&["--work-order", "no-such-work-order.json"]);
    assert_eq!(run.code, Some(2), "{}", run.stdout);
    assert!(
        run.stderr
            .contains("--work-order: cannot read `no-such-work-order.json`: "),
        "{}",
        run.stderr
    );
}
```

- [ ] **Step 6: Run them and watch them fail**

Run: `cargo test -p harness-conformance --test cli`
Expected: 4 passed, 6 failed. `without_the_flag_the_fixture_is_sent` fails because `harness-fake` does not yet name its unit on stderr. The other five fail because the CLI reads every flag's value as a number of seconds: it exits 2 and says the value "is not a whole number of seconds", so the tests that expect exit 0 fail on the code and the tests that expect a reason fail on the text.

- [ ] **Step 7: Add the flag**

In `crates/harness-conformance/src/bin/harness-conformance.rs`, replace everything above `fn main()` with:

```rust
//! `harness-conformance [--wall-clock-secs N] [--grace-secs N] [--work-order JSON|PATH] -- <harness command> [args...]`
//!
//! Runs every conformance case against a harness and exits 0 only if none failed.

use harness_conformance::{parse_work_order, run_all, CaseOutcome, KitConfig};
use harness_protocol::WorkOrder;
use std::process::ExitCode;
use std::time::Duration;

const USAGE: &str =
    "usage: harness-conformance [--wall-clock-secs N] [--grace-secs N] [--work-order JSON|PATH] -- <harness command> [args...]";

fn seconds(flag: &str, value: &str) -> Result<Duration, String> {
    value
        .parse()
        .map(Duration::from_secs)
        .map_err(|_| format!("{flag}: `{value}` is not a whole number of seconds"))
}

/// A value that starts with `{` is the work order itself; anything else is a file that holds it.
fn load_work_order(value: &str) -> Result<WorkOrder, String> {
    let text = if value.trim_start().starts_with('{') {
        value.to_string()
    } else {
        std::fs::read_to_string(value)
            .map_err(|e| format!("--work-order: cannot read `{value}`: {e}"))?
    };
    parse_work_order(&text).map_err(|reason| format!("--work-order: {reason}"))
}

fn parse(args: Vec<String>) -> Result<KitConfig, String> {
    let split = args
        .iter()
        .position(|a| a == "--")
        .ok_or_else(|| USAGE.to_string())?;
    let (flags, rest) = args.split_at(split);
    let command = rest[1..].to_vec();
    if command.is_empty() {
        return Err(USAGE.to_string());
    }
    let mut cfg = KitConfig::new(command);
    let mut it = flags.iter();
    while let Some(flag) = it.next() {
        let value = it
            .next()
            .ok_or_else(|| format!("{flag} needs a value\n{USAGE}"))?;
        match flag.as_str() {
            "--wall-clock-secs" => cfg.wall_clock = seconds(flag, value)?,
            "--grace-secs" => cfg.grace = seconds(flag, value)?,
            "--work-order" => cfg.work_order = Some(load_work_order(value)?),
            other => return Err(format!("unknown flag `{other}`\n{USAGE}")),
        }
    }
    if cfg.grace.is_zero() {
        return Err(format!(
            "--grace-secs must be at least 1: a zero grace fails every case\n{USAGE}"
        ));
    }
    Ok(cfg)
}
```

In `crates/harness-conformance/src/bin/harness-fake.rs`, name the unit on stderr:

```diff
--- a/crates/harness-conformance/src/bin/harness-fake.rs
+++ b/crates/harness-conformance/src/bin/harness-fake.rs
@@ -612,6 +612,10 @@ fn main() {
             return;
         }
     };
+    eprintln!(
+        "harness-fake unit {} on {} branch {} tier {:?}",
+        order.unit_id, order.repo.slug, order.branch, order.tier
+    );
     if mode == Mode::SilentStart {
         loop {
             std::thread::sleep(Duration::from_secs(3_600));
```

- [ ] **Step 8: Run the tests**

Run: `cargo test -p harness-conformance --test cli`
Expected: `test result: ok. 10 passed; 0 failed`.

- [ ] **Step 9: Write the failing README test**

The kit's README will carry a complete work order for the sandbox repository. A test keeps that example valid. In `crates/harness-conformance/src/fixtures.rs`, inside `mod tests`, add:

```rust
    #[test]
    fn the_readme_example_is_a_work_order_the_kit_accepts() {
        let readme = include_str!("../README.md");
        let json = readme
            .split("```json")
            .nth(1)
            .and_then(|rest| rest.split("```").next())
            .expect("the README has a json example");
        let order = parse_work_order(json).expect("the README's example is valid");
        assert_eq!(order.repo.slug, "adbarc92/command-center-agent-sandbox");
    }
```

Run: `cargo test -p harness-conformance --lib fixtures::`
Expected: the build fails with `couldn't read` … `README.md` (the file does not exist yet).

- [ ] **Step 10: Write the kit's README**

Create `crates/harness-conformance/README.md`:

````markdown
# harness-conformance

The conformance kit for [`harness-protocol`](../harness-protocol/README.md), and `harness-fake`, a
scripted harness that passes it.

## Running the kit

```bash
cargo build -p harness-conformance --bins
target/debug/harness-conformance [flags] -- path/to/your-harness --its --args
```

| Flag | Default | Meaning |
|---|---|---|
| `--wall-clock-secs N` | 30 | The longest one case may run before the harness is killed |
| `--grace-secs N` | 5 | How long a harness has to answer a message or exit. At least 1 |
| `--work-order JSON\|PATH` | the fixture | The work order to send in place of the kit's own |

Exit `0`: no case failed. Exit `1`: a case failed; its line starts with `FAIL` and names the
violation. Exit `2`: the command line was wrong, and stderr says why.

## The work order

By default the kit sends a synthetic work order that names `https://example.invalid/conformance.git`.
A scripted harness does not care. A harness that clones its repository does, so give it a real one:

```bash
target/debug/harness-conformance --work-order sandbox-order.json -- path/to/your-harness
```

`--work-order` takes a path to a JSON file or, when the value starts with `{`, the JSON itself. A
file may start with a byte-order mark. The JSON is a `unit/start` work order, as
`harness-protocol` defines it.

The kit checks the work order before it runs anything, and exits `2` with the reason if:

- it is not JSON, or a required field is missing or has the wrong type;
- `unit_id`, `branch`, `repo.url`, `repo.slug` or `repo.base_branch` is blank;
- `caps.usd` is not above 0;
- `resume` is present. The kit starts every unit fresh.

**The kit sets `tier` itself.** Each case tests one tier's behaviour: `happy_path_t1` sends your work
order at T1, and the gate and interrupt cases send it at T2. Every other field is sent as you wrote
it. The two version cases stop at `initialize` and send no work order.

## The sandbox repository

Point a real harness at `adbarc92/command-center-agent-sandbox`. It exists to be changed by agents.
Do not point the kit at a repository you care about: a conformant harness builds a branch in it.

This is a complete work order for it. Replace `work_item.ref` with a real issue in that repository.

```json
{
  "unit_id": "conformance-sandbox-1",
  "work_item": { "kind": "issue", "ref": "adbarc92/command-center-agent-sandbox#1" },
  "tier": "t1",
  "task": "Conformance run against the sandbox repository.",
  "repo": {
    "url": "https://github.com/adbarc92/command-center-agent-sandbox.git",
    "slug": "adbarc92/command-center-agent-sandbox",
    "base_branch": "main"
  },
  "branch": "agent/conformance-sandbox-1",
  "test_cmd": "node --test",
  "caps": { "usd": 0.5, "wall_clock_secs": 600, "min_review_rounds": 1 }
}
```

A run against a real harness spends real money: up to `caps.usd` for each of the five cases that
start a unit. Raise `--wall-clock-secs` to match `caps.wall_clock_secs`.

## `harness-fake`

`target/debug/harness-fake` is the reference harness. `HARNESS_FAKE_MODE` selects a script, and
`HARNESS_FAKE_STEP_MS` paces it.

| Mode | What it does |
|---|---|
| `conformant` (default) | Runs a unit to `pr_open` through all seven stages |
| `blockers` | Reports one unresolved blocker in review round 1, then builds again |
| `needs_human` | Stops for a person from the Green stage with a `scope_request` |
| any other mode in `src/bin/harness-fake.rs` | Breaks the protocol in one specific way, so the kit's detection can be tested |

On stderr it writes `harness-fake <version> starting`, and one line naming each unit it is given.
````

- [ ] **Step 11: Run the tests**

Run: `cargo test -p harness-conformance --lib`
Expected: `test result: ok. 7 passed; 0 failed`.

Run: `cargo test -p harness-conformance`
Expected: 69 passed, 0 failed.

- [ ] **Step 12: Commit**

```bash
cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings
git add crates/harness-conformance/src/fixtures.rs crates/harness-conformance/src/lib.rs crates/harness-conformance/src/cases.rs crates/harness-conformance/src/bin/harness-conformance.rs crates/harness-conformance/src/bin/harness-fake.rs crates/harness-conformance/tests/cli.rs crates/harness-conformance/README.md
git commit -m "feat(harness-conformance): --work-order replaces the kit's fixture; document the sandbox repository"
```

### Task 12: The protocol README: messages, outcomes and harness obligations

The README is what a harness author reads instead of the schema. This task rewrites it for 0.2 and adds a test that keeps it honest: it must name every method, outcome, stop reason and stage the types define, and it must list the seventeen harness obligations (the eight of SP-2a §3.2 and the nine protocol 0.2 adds).

Two rules are stated in the README in so many words, because a harness author cannot work them out from the schema:

- **Metering.** A harness that declares `metering: usd` owes a `metric` before a `pr_open`, `no_change` or `draft_ready` result. A `failed` or a `needs_human` result needs none: such a unit may end before any agent ran.
- **`no_change` still carries `evidence`.** Its `branch` and `head_sha` name the commit that holds the frozen tests the harness wrote, which the control plane then runs against the base.

**Files:**
- Modify: `crates/harness-protocol/README.md`
- Create: `crates/harness-protocol/tests/readme.rs`
- Test: `crates/harness-protocol/tests/readme.rs`

**Interfaces:**
- Consumes: `method`, `Outcome`, `Stage`, `StopReason`, `MAX_LINE_BYTES`, `PROTOCOL_VERSION`.
- Produces: no code. The schema document needs no edit: `ProtocolSchema` in `src/schema.rs` already reaches every new type through `WorkOrder`, `UnitEvent`, `GateRequest`, `UnitResult` and `InitializeResult`, and `schema_defines_every_wire_type` (Tasks 2 to 4) asserts that each one has its own definition.

- [ ] **Step 1: Write the failing test**

Create `crates/harness-protocol/tests/readme.rs`:

```rust
//! The README is the contract in prose: what a harness author reads instead of the schema. It must
//! name every method, outcome, stop reason and stage the types define, and the harness
//! obligations.

use harness_protocol::{method, Outcome, Stage, StopReason, MAX_LINE_BYTES, PROTOCOL_VERSION};
use serde::Serialize;

fn readme() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/README.md");
    std::fs::read_to_string(path).expect("the crate has a README")
}

/// The string a value has on the wire.
fn wire<T: Serialize>(value: T) -> String {
    serde_json::to_value(value)
        .unwrap()
        .as_str()
        .expect("serialises as a string")
        .to_string()
}

fn assert_names(readme: &str, what: &str, names: &[String]) {
    for name in names {
        assert!(
            readme.contains(&format!("`{name}`")),
            "the README does not name the {what} `{name}`"
        );
    }
}

#[test]
fn the_readme_states_the_version_and_the_line_limit() {
    let readme = readme();
    assert!(readme.contains(&format!("protocol **{PROTOCOL_VERSION}**")));
    assert!(readme.contains(&format!("{MAX_LINE_BYTES} bytes")));
}

#[test]
fn the_readme_names_every_method() {
    let methods = [
        method::INITIALIZE,
        method::UNIT_START,
        method::UNIT_EVENT,
        method::GATE_REQUEST,
        method::UNIT_HALT,
        method::UNIT_RESUME,
        method::UNIT_ABANDON,
        method::UNIT_RESULT,
    ]
    .map(String::from);
    assert_names(&readme(), "method", &methods);
}

#[test]
fn the_readme_names_every_outcome_stop_reason_and_stage() {
    let readme = readme();
    let outcomes = [
        Outcome::PrOpen,
        Outcome::NoChange,
        Outcome::Failed,
        Outcome::NeedsHuman,
        Outcome::DraftReady,
    ]
    .map(wire);
    assert_names(&readme, "outcome", &outcomes);

    let reasons = [
        StopReason::ScopeRequest,
        StopReason::SpecConflict,
        StopReason::CheckUnrunnable,
        StopReason::EscalationExhausted,
        StopReason::HoldoutRoundsExhausted,
        StopReason::ReviewRoundsExhausted,
        StopReason::HoldoutDefect,
        StopReason::MapUntrusted,
        StopReason::BaselineRed,
        StopReason::RuntimeUnavailable,
        StopReason::BudgetExhausted,
    ]
    .map(wire);
    assert_names(&readme, "stop reason", &reasons);

    let stages = [
        Stage::Provision,
        Stage::Red,
        Stage::Plan,
        Stage::Green,
        Stage::Check,
        Stage::Review,
        Stage::Deliver,
    ]
    .map(wire);
    assert_names(&readme, "stage", &stages);
}

#[test]
fn the_readme_lists_seventeen_harness_obligations() {
    let readme = readme();
    let section = readme
        .split("## What a harness must do")
        .nth(1)
        .expect("the obligations section exists")
        .split("\n## ")
        .next()
        .unwrap();
    let numbered: Vec<u32> = section
        .lines()
        .filter_map(|line| line.split_once(". ")?.0.parse().ok())
        .collect();
    assert_eq!(numbered, (1..=17).collect::<Vec<u32>>());
}
```

- [ ] **Step 2: Run it and watch it fail**

Run: `cargo test -p harness-protocol --test readme`
Expected: 1 passed, 3 failed. `the_readme_names_every_outcome_stop_reason_and_stage` fails with the message "the README does not name the outcome `no_change`"; `the_readme_states_the_version_and_the_line_limit` and `the_readme_lists_seventeen_harness_obligations` fail too.

- [ ] **Step 3: Rewrite the README**

Replace `crates/harness-protocol/README.md` with:

````markdown
# harness-protocol

The wire contract between `fleetd` (the control plane) and a **harness**, the process that runs
one unit of work. This crate speaks protocol **0.2**.

## Transport

JSON-RPC 2.0, **one JSON object per line**, over the harness's stdin and stdout. Stderr is free-form
log. One harness process per unit. Closing stdin means "shut down".

A line is UTF-8 and at most 4194304 bytes (`MAX_LINE_BYTES`) before its newline. `read_message`
reports a longer line as `LineTooLong` and a line that is not UTF-8 as `InvalidUtf8`;
`write_message` refuses to write a longer one.

## Lifecycle

| Step | Message | Direction |
|---|---|---|
| 1 | `initialize` `{protocol_version, accepted_versions}` → `{protocol_version, harness, capabilities}` | control plane → harness |
| 2 | `unit/start` work order → `{}` | → |
| 3 | `unit/event` observations, stage events, metrics, logs, findings, artifacts, errors | ← notification |
| 4 | `gate/request` `{gate: oracle, …}` → `{approved}` (T2/T3) | ← request |
| — | `unit/halt`, `unit/abandon`, `unit/resume` → `{}` | → at any time |
| 5 | `unit/result` `{outcome, evidence?, failure?, stop?}`, then exit | ← notification |

## Versions

`initialize` carries `accepted_versions`: every version the control plane can talk to. An empty
list offers `protocol_version` alone. The reply names the single version the harness speaks.

While the major is 0, the **minor must match**: a harness that speaks 0.2 refuses a control plane
that accepts only 0.1, and the control plane refuses a harness whose reply it does not accept. From
1.0 on, an equal major is enough. Both sides use `negotiate(accepted, harness_speaks)`. A harness
refuses with error code `-32001`.

A 0.1 message still parses under 0.2, because each added field has a default. The two versions do
not mean the same thing, though, so they are not interchangeable: hence the minor rule.

## What each message carries

| Message | Type | Added in 0.2 |
|---|---|---|
| `initialize` params | `InitializeParams` | `accepted_versions` |
| `initialize` result | `InitializeResult` | `capabilities`: `holdouts`, `controls`, `network`, `profiles`, `kinds`, `presets` |
| `unit/start` params | `WorkOrder` | `kind`, `spec`, `source`, `scope`, `scope_grants`, `permitted_dependencies`, `expected_red`, `config`, `controls`, `profile`, `parent`. `test_cmd` and `task` are still sent |
| `unit/event` `observed` | `Observation` | `oracle_frozen` carries `freeze`: `frozen_files`, `frozen_ids`, `holdout_bundle_path`, `holdout_hash`, `holdout_ids` |
| `unit/event` `stage` | `UnitEvent::Stage` | New. `stage` is `provision`, `red`, `plan`, `green`, `check`, `review` or `deliver`; `status` is `started` or `finished`. Progress reporting: no phase changes because of it, and it does show the harness is alive |
| `unit/event` `metric` | `UnitEvent::Metric` | `cost_basis` (`priced` or `unpriced`), `stage`, `role`, `adapter`, `model` |
| `gate/request` `oracle` | `GateRequest::Oracle` | `holdout_files`, `holdout_hash` |
| `unit/result` | `UnitResult` | `outcome` gains `needs_human` and `draft_ready`; `stop` goes with `needs_human`; `evidence` gains `spec_hash`, `map`, `test_report`, `controls`, `review` |

### Outcomes

| `outcome` | Must carry | Meaning |
|---|---|---|
| `pr_open` | `evidence` | The work is ready for the control plane to verify |
| `no_change` | `evidence` | The base already satisfies the spec, so there is no change to hand over |
| `draft_ready` | `evidence` | A `draft` or `adopt` unit produced its bundle |
| `failed` | `failure` | The unit cannot continue |
| `needs_human` | `stop` | The unit stopped for a person to decide |

`stop.reason` is one of `scope_request`, `spec_conflict`, `check_unrunnable`,
`escalation_exhausted`, `holdout_rounds_exhausted`, `review_rounds_exhausted`, `holdout_defect`,
`map_untrusted`, `baseline_red`, `runtime_unavailable` or `budget_exhausted`. For `scope_request`,
`stop.request` lists the files asked for.

For `no_change`, `evidence` is still required: its `branch` and `head_sha` name the commit that
holds the frozen tests the harness wrote, which the control plane then runs against the base.

### How frozen tests are hashed

Both sides must get the same digests from the same files, so both use this crate's functions:

- `file_sha256(bytes)`: SHA-256 of a file's bytes, as 64 lowercase hex characters.
- `bundle_hash(files)`: one digest for a set of files. Each file contributes a record,
  `<path>\0<sha256>\n`; the records are taken in bytewise path order and hashed with SHA-256. Paths
  start at the repository root, use `/`, and are hashed exactly as given.

The oracle gate's `hash` is `bundle_hash` over `frozen_files`; `holdout_hash` is `bundle_hash` over
the holdout files. The holdout bundle is an ordinary archive in which each holdout file sits at the
path it has in the repository.

## Rules the control plane enforces

- A harness **never merges** and **never declares done**. `evidence` is re-read from git, CI and
  the test run before any unit reaches `PrOpen`.
- `capabilities` decide eligibility. T2/T3 units need `gates: [oracle]`. Isolation weaker than
  `container`, or `metering: none`, needs per-unit operator opt-in.
- If you declare `metering: usd`, send at least one `metric` before a `pr_open`, `no_change` or
  `draft_ready` result.
- A `failed` or a `needs_human` result needs no `metric`: such a unit may end before any agent ran.
- Answer `unit/start` before you send anything else.
- After `unit/result`, send nothing more and exit.
- After answering `unit/halt` or `unit/abandon`, exit **without** `unit/result`.

## What a harness must do

Passing the conformance kit is not enough to be driven by the control plane: the kit does not check
the order of observations. These are the obligations. `harness-fake` meets them.

1. Send observations in the order of the unit's life: `provisioned`, then `oracle_frozen` (at every
   tier), then `build_finished`, then one of `checks_passed`, `checks_failed` or `empty_diff`, then
   `review_finished`, and last `unit/result`. A failed check or an unmet review goes back to
   `build_finished`.
2. At T2 and T3, send `gate/request` after `oracle_frozen`, with nothing between them except `log`,
   `metric`, `finding` or `error`. At T1, send no gate.
3. End `pr_open` only after a `review_finished` that meets the review gate: checks green, no
   unresolved blocker, `round` at least `caps.min_review_rounds`, and no more blockers than the
   round before. Until then, build again.
4. Count review rounds from 1 in every process. A resumed harness starts again at round 1.
5. After `empty_diff`, send only `log`, `metric`, `finding`, `artifact` or `error`, and then
   `unit/result` with `no_change` or `failed`.
6. Started with `resume{oracle_frozen: true}`, send `provisioned` and continue at building. Send no
   `oracle_frozen` and no gate.
7. Always honour `unit/abandon`. Honour `unit/halt` if you declare `halt: true`.
8. `unit/resume` exists on the wire and is not used: a unit is resumed by starting a new process
   whose work order carries `resume`.
9. Do not hold `provisioned` back for slow preparation. Send it once the workspace is there;
   installing dependencies, mapping the repository and running the baseline tests come afterwards,
   each announced with `stage` events. That way a unit never ends `needs_human` before
   `provisioned`.
10. A unit waiting at the oracle gate must cost nothing. Shut its containers down first, then send
    `gate/request`. Until the answer comes, keep only your own process and the unit's storage
    volume.
11. If the gate reply is `{approved: false}`, the unit is over: send `unit/result` with outcome
    `failed` and the failure detail `oracle rejected`. Writing a new oracle and asking again is not
    allowed.
12. `needs_human` is final for the process. Remove your containers and exit once the result is
    sent.
13. Tag each container and each volume with the id of the unit it belongs to, so that leftovers can
    be found and removed.
14. On resume, pick the unit up at the build step however far it had got. The sequence is
    `provisioned`, any unfinished build work, `build_finished`, then a fresh check and a fresh
    review, even if the earlier process had passed them.
15. A unit's oracle is frozen once. If you are started with `resume{oracle_frozen: false}` and your
    own records show the freeze already happened, repeat it as it was: the identical
    `oracle_frozen` payload, followed at T2 and T3 by the identical gate request.
16. Because of obligation 4, `review_finished.round` restarts at 1 after a resume. Keep the running
    total yourself and put the rounds of earlier processes in `evidence.review.prior_rounds`.
17. The round floor can ask for another review when the last one found nothing. Do not send two
    `review_finished` in a row: go through an empty build first, so that `build_finished` and a
    check result come before each review.

## The protocol monitor

`harness_protocol::monitor::ProtocolMonitor` says what each line from a harness means and whether
it is legal on the wire. It holds no clock and no process, so the conformance kit and the control
plane's supervisor share it. Deadlines, the reply to `unit/start`, and what a result after an
interrupt means are the caller's decisions.

## The contract

`contract/harness-protocol.contract.json` is the JSON Schema for every message, pinned by canonical
SHA-256 in `tests/contract.rs` and registered in NEXUS `docs/contracts/README.md`. To change the
protocol on purpose, re-bless with `HARNESS_PROTOCOL_BLESS=1 cargo test -p harness-protocol --test
contract`, update the pinned hash, and update the NEXUS registry row in the same change.

## Checking your harness

```bash
cargo build -p harness-conformance --bins
target/debug/harness-conformance -- path/to/your-harness --its --args
```

Exit `0` means no case failed. Cases your harness declares it cannot do are reported `SKIP`, not
`PASS`. `target/debug/harness-fake` is a reference harness; set `HARNESS_FAKE_MODE` to see what each
violation looks like. `crates/harness-conformance/README.md` covers the kit's flags, including
`--work-order` for a harness that must clone a real repository.

## What the conformance kit checks

| Rule | Violation |
|---|---|
| Refuse an `initialize` from a different protocol major with error code `-32001`. | `VersionMismatchAccepted` |
| While the major is 0, refuse an `initialize` that accepts only another minor, the same way. | `VersionMismatchAccepted` |
| Reply to `initialize` with a version the control plane accepts. | `InvalidResult` |
| Answer `unit/start` before any other message, within grace. | `StartNotAcknowledged` |
| Send no `gate/request` for a T1 unit. | `UnexpectedGateRequest` |
| At T2/T3 with `gates: [oracle]`, send `gate/request` before any `pr_open`. | `GateNotRequested` |
| Never end `pr_open` after a rejected gate. | `GateRejectionIgnored` |
| `pr_open`, `no_change` and `draft_ready` need `evidence`; `failed` needs `failure`; `needs_human` needs `stop`. | `InvalidResult` |
| With `metering: usd`, send at least one `metric` before a `pr_open`, `no_change` or `draft_ready` result. | `MeteringDeclaredButSilent` |
| After `unit/result`, send nothing more, and exit within grace. This includes `needs_human`. | `MessageAfterResult` / `DidNotExitAfterResult` |
| Answer `unit/halt` / `unit/abandon`, then exit within grace without `unit/result`. `halt` is tested only when you declare `halt: true`; `abandon` always. With `gates: [oracle]`, the kit sends the interrupt while your `gate/request` is still unanswered. | `InterruptNotHonored` |
| Request ids are unsigned integers; every line is UTF-8, at most 4194304 bytes, and a protocol message. | `Malformed` |
| Use only the protocol's own methods. | `UnknownMethod` |
| Finish within the wall clock (`--wall-clock-secs`). | `WallClockExceeded` |

The kit passes your harness's stderr through to its own, so your diagnostics appear next to a
failing case. Start your harness binary directly, or `exec` it from a wrapper script: a shell or
launcher that stays alive can leave child processes holding stdout open, so the kit never sees your
harness exit.
````

- [ ] **Step 4: Run the test**

Run: `cargo test -p harness-protocol --test readme`
Expected: `test result: ok. 4 passed; 0 failed`.

- [ ] **Step 5: Commit**

```bash
git add crates/harness-protocol/README.md crates/harness-protocol/tests/readme.rs
git commit -m "docs(harness-protocol): README for 0.2: messages, outcomes, file hashing and the harness obligations"
```

### Finishing lane CC-PROTO

- [ ] **Step 1: Run every command in the lane's Verify table** and keep the output. If one does not give the expected result, fix the cause and run the whole table again.

- [ ] **Step 2: Read the final contract hash**

Run: `grep -n "const CONTRACT_SHA256" crates/harness-protocol/tests/contract.rs`
Expected: the value pinned in Task 4, `9645ff197dde95906431461bc4a2e6631d0923b0258ccb385583bd29997cabbc`, unless your code differs from the listings.

- [ ] **Step 3: Push and open the pull request**

```bash
git push -u origin feat/m0-protocol-0.2
gh pr create --base factory/m0 --head feat/m0-protocol-0.2 --title "Harness protocol 0.2: monitor, kit rebuilt on it, harness-fake as a valid peer" --body-file pr-body.md
```

Write `pr-body.md` outside the worktree's tracked files (for example in your scratch directory) and do not commit it. It states: what changed, crate by crate; the output of each Verify command; **the new contract hash, on a line of its own, for the owner to register**; the two existing assertions that changed and why (Task 10); the coordinator requests this lane relied on (R1 to R3); and `Resolves #82, #83, #87`. It carries no "Generated with" footer. Do not merge.

---

## Lane CC-CORE

**Owns:** `crates/fleet-core/**`.

**Reads:** `crates/fleetd/src/driver.rs` (to confirm no existing caller depends on a row this lane changes); `docs/superpowers/specs/2026-09-16-sp2a-harness-supervisor-design.md` §2 item 3, §4.5 and §4.8.

**Worktree and branch:** worktree `D:\MajorProjects\.swarm-wt\m0-cc-core`, branch `feat/m0-fleet-core-delta`, cut from `origin/factory/m0`. The pull request goes against `factory/m0`.

```bash
git -C /d/MajorProjects/INFRASTRUCTURE/command-center fetch origin
git -C /d/MajorProjects/INFRASTRUCTURE/command-center worktree add /d/MajorProjects/.swarm-wt/m0-cc-core -b feat/m0-fleet-core-delta origin/factory/m0
cd /d/MajorProjects/.swarm-wt/m0-cc-core
```

Expected: `Preparing worktree (new branch 'feat/m0-fleet-core-delta')`.

**Needs:** CC-COORD merged into `factory/m0`, which it creates. No dependency request: the three crates `fleet-core` may use (`serde`, `serde_json`, `thiserror`) are enough. Nothing from CC-PROTO.

**Blocks:** milestone 1's supervisor and store lanes (the new triggers, `ErrorScope::Harness`, `Command::Reverify`); milestone 2's auxiliary runs in the scheduling crate.

**Verify:**

| Command | Expected |
|---|---|
| `cargo fmt --all -- --check` | No output, exit 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | Exit 0, no warning |
| `cargo test -p fleet-core` | `test result: ok. 52 passed; 0 failed` |
| `cargo test -p fleetd` | 98 passed, 0 failed, 3 ignored, as before this lane: 95 unit, 3 in `demo_mode_it` |
| `cargo test --workspace` | 0 failed, 3 ignored, and 22 more passed than the branch point: 231 on the scaffold's 209 (188 on the bare 166 baseline) |
| `cargo xtask test static --root-only` | Exit 0. This includes the check that `fleet-core` reaches no I/O crate |
| `cargo xtask test unit` | Exit 0; the last line ends `step(s) passed` |
| `git diff --name-only origin/factory/m0` | Only `crates/fleet-core/src/transition.rs`, `crates/fleet-core/src/auxiliary.rs`, `crates/fleet-core/src/event.rs`, `crates/fleet-core/src/lib.rs` |

### Task 13: A breach, a stall or tampering wherever a harness process can be alive

The SP-2a rows. Today `CapBreach` and `Stall` are valid only in the four agent-active phases, so a breach while a harness is starting, waiting at the oracle gate or bundling its result would be an invalid trigger, and the supervisor would have to fail the unit. They become valid, and lead to `NeedsHuman`, in `Provisioning`, `AwaitingOracleApproval` and `MergeCheck` too. `OracleTampering` becomes valid in `MergeCheck`, where the control plane verifies the result. `RetriesExhausted` does not change.

The in-process driver in `fleetd` never raises these triggers in those phases, so its behaviour is unchanged.

**Files:**
- Modify: `crates/fleet-core/src/transition.rs`
- Test: `crates/fleet-core/src/transition.rs` (inline)

**Interfaces:**
- Consumes: `Phase::is_agent_active`.
- Produces: no new name. `transition(phase, tier, trigger)` returns `Some(Phase::NeedsHuman)` for these rows, at every tier:

  | Trigger | Phases, after this task |
  |---|---|
  | `CapBreach`, `Stall` | `Provisioning`, `Spec`, `AwaitingOracleApproval`, `Building`, `Checking`, `Reviewing`, `MergeCheck` |
  | `OracleTampering` | `Spec`, `Building`, `Checking`, `Reviewing`, `MergeCheck` |
  | `RetriesExhausted` | `Spec`, `Building`, `Checking`, `Reviewing` (unchanged) |

  and `None` for each of those triggers in every other phase.

- [ ] **Step 1: Write the failing table tests**

In `crates/fleet-core/src/transition.rs`, inside `mod tests`, add this directly after the `run` helper:

```rust
    const ALL_PHASES: [Phase; 14] = [
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
    ];
    const ALL_TIERS: [Tier; 3] = [Tier::T1, Tier::T2, Tier::T3];

    /// `trigger` moves each phase in `accepted` to the phase beside it, at every tier, and is
    /// rejected in every other phase.
    fn assert_only(trigger: Trigger, accepted: &[(Phase, Phase)]) {
        for tier in ALL_TIERS {
            for phase in ALL_PHASES {
                let want = accepted
                    .iter()
                    .find(|(from, _)| *from == phase)
                    .map(|(_, to)| *to);
                assert_eq!(
                    transition(phase, tier, trigger),
                    want,
                    "{trigger:?} in {phase:?} at {tier:?}"
                );
            }
        }
    }

    // --- a breach or a stall wherever a harness process can be alive ---

    #[test]
    fn cap_breach_and_stall_pause_every_phase_a_harness_can_be_alive_in() {
        let pauses = [
            (Provisioning, NeedsHuman),
            (Spec, NeedsHuman),
            (AwaitingOracleApproval, NeedsHuman),
            (Building, NeedsHuman),
            (Checking, NeedsHuman),
            (Reviewing, NeedsHuman),
            (MergeCheck, NeedsHuman),
        ];
        assert_only(Trigger::CapBreach, &pauses);
        assert_only(Trigger::Stall, &pauses);
    }

    #[test]
    fn retries_exhausted_still_applies_to_agent_active_phases_only() {
        assert_only(
            Trigger::RetriesExhausted,
            &[
                (Spec, NeedsHuman),
                (Building, NeedsHuman),
                (Checking, NeedsHuman),
                (Reviewing, NeedsHuman),
            ],
        );
    }

    #[test]
    fn oracle_tampering_found_at_verification_needs_a_human() {
        assert_only(
            Trigger::OracleTampering,
            &[
                (Spec, NeedsHuman),
                (Building, NeedsHuman),
                (Checking, NeedsHuman),
                (Reviewing, NeedsHuman),
                (MergeCheck, NeedsHuman),
            ],
        );
    }
```

One existing test asserts the row that changes. Replace `cap_breach_only_interrupts_agent_active_phases` with:

```rust
    #[test]
    fn cap_breach_only_interrupts_phases_a_harness_can_be_alive_in() {
        // Active phase: interrupted.
        assert_eq!(
            transition(Building, Tier::T1, Trigger::CapBreach),
            Some(NeedsHuman)
        );
        // The harness is still bundling its result in `MergeCheck`: a breach pauses the unit.
        assert_eq!(
            transition(MergeCheck, Tier::T1, Trigger::CapBreach),
            Some(NeedsHuman)
        );
        // No harness process once the PR is open: cap breach is not meaningful → invalid.
        assert_eq!(transition(PrOpen, Tier::T1, Trigger::CapBreach), None);
    }
```

- [ ] **Step 2: Run them and watch them fail**

Run: `cargo test -p fleet-core transition::`
Expected: 3 failed. `cap_breach_and_stall_pause_every_phase_a_harness_can_be_alive_in` fails with `CapBreach in Provisioning at T1` (`left: None`, `right: Some(NeedsHuman)`); `oracle_tampering_found_at_verification_needs_a_human` fails with `OracleTampering in MergeCheck at T1`; `cap_breach_only_interrupts_phases_a_harness_can_be_alive_in` fails on its `MergeCheck` assertion. `retries_exhausted_still_applies_to_agent_active_phases_only` passes already: it pins a row that must not move.

- [ ] **Step 3: Widen the three rows**

In `crates/fleet-core/src/transition.rs`, replace the start of `transition` — from its doc comment through the closing brace of the first `match trigger { … }` — with:

```rust
/// Phases in which a harness process can be alive, so a cap breach or a stall pauses the unit:
/// the agent-active phases, and the three where the process runs while no agent is working (it is
/// starting, waiting at the oracle gate, or bundling its result).
fn breach_pauses(phase: Phase) -> bool {
    phase.is_agent_active()
        || matches!(
            phase,
            Phase::Provisioning | Phase::AwaitingOracleApproval | Phase::MergeCheck
        )
}

/// Compute the next phase, or `None` if the trigger is invalid here.
pub fn transition(phase: Phase, tier: Tier, trigger: Trigger) -> Option<Phase> {
    use Phase::*;
    use Trigger::*;

    // Universal interrupts first (they win from any non-terminal phase).
    match trigger {
        FatalError if !phase.is_terminal() => return Some(Failed),
        Halt if !phase.is_terminal() => return Some(Halted),
        CapBreach | Stall if breach_pauses(phase) => return Some(NeedsHuman),
        RetriesExhausted if phase.is_agent_active() => return Some(NeedsHuman),
        OracleTampering if phase.is_agent_active() || phase == MergeCheck => {
            return Some(NeedsHuman)
        }
        _ => {}
    }
```

The rest of the function, from `// Phase-specific forward / re-entry transitions.`, does not change.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p fleet-core`
Expected: `test result: ok. 33 passed; 0 failed`.

Run: `cargo test -p fleetd`
Expected: 98 passed, 0 failed, 3 ignored.

- [ ] **Step 5: Commit**

```bash
cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings
git add crates/fleet-core/src/transition.rs
git commit -m "feat(fleet-core): a cap breach, a stall or tampering pauses the unit wherever a harness can be alive"
```

### Task 14: `HarnessStopped`, `Integrated`, `VerifyStopped` and `Reverify`

**Files:**
- Modify: `crates/fleet-core/src/transition.rs`
- Test: `crates/fleet-core/src/transition.rs` (inline)

**Interfaces:**
- Consumes: the `assert_only` helper and the `ALL_PHASES` / `ALL_TIERS` constants from Task 13.
- Produces: four variants on `Trigger`, with these rows and no others, at every tier:

  | Trigger | From | To |
  |---|---|---|
  | `HarnessStopped` | `Spec`, `Building`, `Checking`, `Reviewing` | `NeedsHuman` |
  | `Integrated` | `MergeCheck` | `Done` |
  | `VerifyStopped` | `MergeCheck`, `Checking` | `NeedsHuman` |
  | `Reverify { from: Phase::MergeCheck }` | `NeedsHuman` | `MergeCheck` |
  | `Reverify { from: Phase::Checking }` | `NeedsHuman` | `Checking` |

  On the wire: `{"trigger":"harness_stopped"}`, `{"trigger":"integrated"}`, `{"trigger":"verify_stopped"}`, `{"trigger":"reverify","from":"merge_check"}`.

`fleet-core` is pure and does not remember which phase a paused unit came from. `Reverify` carries that phase, supplied by the caller, which recorded it when it applied `VerifyStopped`. The state machine refuses every `from` except the two phases verification runs in; it cannot refuse a caller that names the wrong one of those two. That check belongs to the caller and is called out in Review Focus item 5.

Inside a spec set only the parent opens a pull request. Its children end by being merged onto the parent's integration branch, which is what `Integrated` records, so the T3 ship step does not apply to them. When that merge fails, the caller applies `MergeConflict`, as it would for any unit.

- [ ] **Step 1: Write the failing tests**

In `crates/fleet-core/src/transition.rs`, inside `mod tests`, add this after the three table tests of Task 13:

```rust
    // --- the factory delta ---

    #[test]
    fn harness_stopped_pauses_agent_active_phases_and_is_rejected_everywhere_else() {
        assert_only(
            Trigger::HarnessStopped,
            &[
                (Spec, NeedsHuman),
                (Building, NeedsHuman),
                (Checking, NeedsHuman),
                (Reviewing, NeedsHuman),
            ],
        );
    }

    #[test]
    fn integrated_finishes_a_unit_from_merge_check_and_nowhere_else() {
        assert_only(Trigger::Integrated, &[(MergeCheck, Done)]);
    }

    #[test]
    fn verify_stopped_pauses_the_two_phases_verification_runs_in() {
        assert_only(
            Trigger::VerifyStopped,
            &[(MergeCheck, NeedsHuman), (Checking, NeedsHuman)],
        );
    }

    #[test]
    fn reverify_returns_needs_human_to_merge_check_or_checking_and_to_no_other_phase() {
        assert_only(
            Trigger::Reverify { from: MergeCheck },
            &[(NeedsHuman, MergeCheck)],
        );
        assert_only(
            Trigger::Reverify { from: Checking },
            &[(NeedsHuman, Checking)],
        );
        // A caller that names any other phase is refused in every phase, `NeedsHuman` included.
        for from in ALL_PHASES {
            if from != MergeCheck && from != Checking {
                assert_only(Trigger::Reverify { from }, &[]);
            }
        }
    }

    #[test]
    fn a_child_unit_finishes_by_integration_at_every_tier_without_a_ship() {
        for tier in ALL_TIERS {
            run(
                Reviewing,
                tier,
                &[
                    (Trigger::ReviewFinished { gate_met: true }, MergeCheck),
                    (Trigger::Integrated, Done),
                ],
            );
        }
    }

    #[test]
    fn a_failed_verification_is_re_run_and_the_unit_carries_on() {
        // A `pr_open` result whose verification could not run.
        run(
            MergeCheck,
            Tier::T1,
            &[
                (Trigger::VerifyStopped, NeedsHuman),
                (Trigger::Reverify { from: MergeCheck }, MergeCheck),
                (Trigger::MergeClean, PrOpen),
            ],
        );
        // A `no_change` result, which is verified in `Checking`.
        run(
            Checking,
            Tier::T2,
            &[
                (Trigger::VerifyStopped, NeedsHuman),
                (Trigger::Reverify { from: Checking }, Checking),
                (Trigger::EmptyDiff, NoChange),
            ],
        );
    }

    #[test]
    fn a_unit_paused_by_verification_can_still_be_resumed_abandoned_or_halted() {
        let paused = transition(MergeCheck, Tier::T1, Trigger::VerifyStopped).unwrap();
        assert_eq!(
            transition(paused, Tier::T1, Trigger::Resume),
            Some(Provisioning)
        );
        assert_eq!(transition(paused, Tier::T1, Trigger::Abandon), Some(Failed));
        assert_eq!(transition(paused, Tier::T1, Trigger::Halt), Some(Halted));
    }

    #[test]
    fn a_stopped_harness_resumes_through_provisioning() {
        run(
            Building,
            Tier::T2,
            &[
                (Trigger::HarnessStopped, NeedsHuman),
                (Trigger::Resume, Provisioning),
                (Trigger::Provisioned, Spec),
            ],
        );
    }

    #[test]
    fn the_new_triggers_have_stable_wire_names() {
        use serde_json::{from_value, json, to_value};
        for (trigger, wire) in [
            (
                Trigger::HarnessStopped,
                json!({"trigger": "harness_stopped"}),
            ),
            (Trigger::Integrated, json!({"trigger": "integrated"})),
            (Trigger::VerifyStopped, json!({"trigger": "verify_stopped"})),
            (
                Trigger::Reverify { from: MergeCheck },
                json!({"trigger": "reverify", "from": "merge_check"}),
            ),
            (
                Trigger::Reverify { from: Checking },
                json!({"trigger": "reverify", "from": "checking"}),
            ),
        ] {
            assert_eq!(to_value(trigger).unwrap(), wire);
            assert_eq!(from_value::<Trigger>(wire).unwrap(), trigger);
        }
    }
```

- [ ] **Step 2: Run them and watch them fail**

Run: `cargo test -p fleet-core transition::`
Expected: the build fails with E0599 (no variant or associated item named `HarnessStopped` found for enum `Trigger`), and the same for `Integrated`, `VerifyStopped` and `Reverify`.

- [ ] **Step 3: Add the four triggers**

Replace the definition of `Trigger` (doc comment, derive, serde attribute and body) with:

```rust
/// Everything that can drive a unit between phases. Forward-progress triggers
/// are daemon observations; interrupt/re-entry triggers come from caps, the
/// runner, or human commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "trigger", rename_all = "snake_case")]
pub enum Trigger {
    // --- forward progress ---
    Start,
    Provisioned,
    OracleFrozen,
    OracleApproved,
    OracleRejected,
    BuildFinished,
    ChecksPassed,
    ChecksFailed,
    /// Green checks but the branch diff is empty: a legitimate no-op.
    EmptyDiff,
    /// A review round finished; `gate_met` is the evidence-based gate verdict
    /// (green + no unresolved blockers + non-increasing + round floor).
    ReviewFinished {
        gate_met: bool,
    },
    /// Trial merge into a fresh base succeeded.
    MergeClean,
    /// Trial merge hit conflicts.
    MergeConflict,
    /// GitHub reported the open PR mergeable.
    PrMergeable,
    /// GitHub reported the open PR's base moved (`dirty`).
    PrDirty,
    // --- interrupts (guarded by phase) ---
    CapBreach,
    Stall,
    OracleTampering,
    /// The rate-limit backoff envelope was exhausted (Anthropic still unavailable).
    RetriesExhausted,
    FatalError,
    Halt,
    // --- human re-entry ---
    Resume,
    Abandon,
    /// Human ships the PR from `NeedsHuman` (the T3 final gate).
    Ship,
    // --- the factory delta ---
    /// The harness ended the unit `needs_human`: it stopped for a person to decide.
    HarnessStopped,
    /// The unit's verified work was merged onto its parent's integration branch. A unit inside a
    /// spec set ends this way, not through a pull request of its own.
    Integrated,
    /// Verification by the control plane gave no verdict, or its tests came out red.
    VerifyStopped,
    /// A person asks for verification to be tried again; the harness is not started again. `from`
    /// is the phase the unit was in when `VerifyStopped` paused it, which the caller recorded.
    Reverify {
        from: Phase,
    },
}
```

- [ ] **Step 4: Add their rows**

Replace the function `transition` with:

```rust
/// Compute the next phase, or `None` if the trigger is invalid here.
pub fn transition(phase: Phase, tier: Tier, trigger: Trigger) -> Option<Phase> {
    use Phase::*;
    use Trigger::*;

    // Universal interrupts first (they win from any non-terminal phase).
    match trigger {
        FatalError if !phase.is_terminal() => return Some(Failed),
        Halt if !phase.is_terminal() => return Some(Halted),
        CapBreach | Stall if breach_pauses(phase) => return Some(NeedsHuman),
        RetriesExhausted | HarnessStopped if phase.is_agent_active() => return Some(NeedsHuman),
        OracleTampering if phase.is_agent_active() || phase == MergeCheck => {
            return Some(NeedsHuman)
        }
        VerifyStopped if matches!(phase, MergeCheck | Checking) => return Some(NeedsHuman),
        _ => {}
    }

    // Phase-specific forward / re-entry transitions.
    Some(match (phase, trigger) {
        (Queued, Start) => Provisioning,
        (Provisioning, Provisioned) => Spec,

        (Spec, OracleFrozen) => {
            if tier.requires_oracle_approval() {
                AwaitingOracleApproval
            } else {
                Building
            }
        }
        (AwaitingOracleApproval, OracleApproved) => Building,
        (AwaitingOracleApproval, OracleRejected) => Spec,

        (Building, BuildFinished) => Checking,
        (Checking, ChecksPassed) => Reviewing,
        (Checking, ChecksFailed) => Building,
        (Checking, EmptyDiff) => NoChange,

        (Reviewing, ReviewFinished { gate_met: true }) => MergeCheck,
        (Reviewing, ReviewFinished { gate_met: false }) => Building,

        (MergeCheck, MergeClean) => {
            if tier.auto_opens_pr() {
                PrOpen
            } else {
                NeedsHuman // T3: human ships
            }
        }
        (MergeCheck, MergeConflict) => NeedsHuman,
        (MergeCheck, Integrated) => Done,

        (PrOpen, PrMergeable) => Done,
        (PrOpen, PrDirty) => NeedsHuman,

        // Human re-entry. Resume re-provisions (the paused container was torn
        // down; provision reuses the persisted volume) before continuing.
        (NeedsHuman, Resume) => Provisioning,
        (NeedsHuman, Abandon) => Failed,
        (NeedsHuman, Ship) => PrOpen,
        (Halted, Resume) => Provisioning,
        (Halted, Abandon) => Failed,

        // Re-verification goes back to the phase verification runs in, and to no other.
        (NeedsHuman, Reverify { from: MergeCheck }) => MergeCheck,
        (NeedsHuman, Reverify { from: Checking }) => Checking,

        // Anything else is invalid in this phase.
        _ => return None,
    })
}
```

The changes from Task 13's version: `HarnessStopped` joins `RetriesExhausted`; the `VerifyStopped` guard; `(MergeCheck, Integrated) => Done`; and the two `Reverify` rows.

- [ ] **Step 5: Run the tests**

Run: `cargo test -p fleet-core`
Expected: `test result: ok. 42 passed; 0 failed`.

Run: `cargo test -p fleetd`
Expected: 98 passed, 0 failed, 3 ignored. `fleetd` never matches on `Trigger` exhaustively, so new variants do not break it.

- [ ] **Step 6: Commit**

```bash
cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings
git add crates/fleet-core/src/transition.rs
git commit -m "feat(fleet-core): HarnessStopped, Integrated, VerifyStopped and Reverify triggers"
```

### Task 15: The auxiliary-run state machine

An auxiliary run drafts a spec or adopts a repository. It has five states and five accepted transitions, and it cannot be resumed.

**Files:**
- Create: `crates/fleet-core/src/auxiliary.rs`
- Modify: `crates/fleet-core/src/lib.rs`
- Test: `crates/fleet-core/src/auxiliary.rs` (inline)

The shared-interface list names this module `aux.rs`. On Windows that file cannot be added to git (`AUX` is a reserved device name; `git add aux.rs` fails with `unable to index file`), so the file is `auxiliary.rs`. The three public names are exactly the listed ones and are re-exported from the crate root, so no caller sees the difference.

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `pub enum AuxState { Queued, Running, Ready, Failed, Halted }` — `snake_case` strings on the wire
  - `pub enum AuxTrigger { Start, Ready, Fail, Halt, Abandon }` — `snake_case` strings on the wire
  - `pub fn aux_transition(state: AuxState, trigger: AuxTrigger) -> Option<AuxState>`

  | State | Trigger | Next |
  |---|---|---|
  | `Queued` | `Start` | `Running` |
  | `Running` | `Ready` | `Ready` |
  | `Running` | `Fail` | `Failed` |
  | `Running` | `Halt` | `Halted` |
  | `Halted` | `Abandon` | `Failed` |

  Each of the other twenty pairs returns `None`.

- [ ] **Step 1: Write the failing tests**

Create `crates/fleet-core/src/auxiliary.rs` with the import and the test module:

```rust
use serde::{Deserialize, Serialize};

#[cfg(test)]
mod tests {
    use super::*;

    const STATES: [AuxState; 5] = [
        AuxState::Queued,
        AuxState::Running,
        AuxState::Ready,
        AuxState::Failed,
        AuxState::Halted,
    ];
    const TRIGGERS: [AuxTrigger; 5] = [
        AuxTrigger::Start,
        AuxTrigger::Ready,
        AuxTrigger::Fail,
        AuxTrigger::Halt,
        AuxTrigger::Abandon,
    ];

    /// The five accepted rows. Every other pair of the twenty-five is rejected.
    const ACCEPTED: [(AuxState, AuxTrigger, AuxState); 5] = [
        (AuxState::Queued, AuxTrigger::Start, AuxState::Running),
        (AuxState::Running, AuxTrigger::Ready, AuxState::Ready),
        (AuxState::Running, AuxTrigger::Fail, AuxState::Failed),
        (AuxState::Running, AuxTrigger::Halt, AuxState::Halted),
        (AuxState::Halted, AuxTrigger::Abandon, AuxState::Failed),
    ];

    #[test]
    fn every_pair_of_state_and_trigger_matches_the_table() {
        for state in STATES {
            for trigger in TRIGGERS {
                let want = ACCEPTED
                    .iter()
                    .find(|(s, t, _)| *s == state && *t == trigger)
                    .map(|(_, _, next)| *next);
                assert_eq!(
                    aux_transition(state, trigger),
                    want,
                    "{trigger:?} in {state:?}"
                );
            }
        }
    }

    #[test]
    fn a_run_goes_from_queued_to_ready() {
        let running = aux_transition(AuxState::Queued, AuxTrigger::Start).unwrap();
        assert_eq!(
            aux_transition(running, AuxTrigger::Ready),
            Some(AuxState::Ready)
        );
    }

    #[test]
    fn ready_and_failed_accept_nothing() {
        for state in [AuxState::Ready, AuxState::Failed] {
            for trigger in TRIGGERS {
                assert_eq!(
                    aux_transition(state, trigger),
                    None,
                    "{trigger:?} in {state:?}"
                );
            }
        }
    }

    #[test]
    fn a_halted_run_can_only_be_abandoned() {
        for trigger in TRIGGERS {
            let want = (trigger == AuxTrigger::Abandon).then_some(AuxState::Failed);
            assert_eq!(
                aux_transition(AuxState::Halted, trigger),
                want,
                "{trigger:?}"
            );
        }
    }

    #[test]
    fn a_queued_run_can_only_be_started() {
        for trigger in TRIGGERS {
            let want = (trigger == AuxTrigger::Start).then_some(AuxState::Running);
            assert_eq!(
                aux_transition(AuxState::Queued, trigger),
                want,
                "{trigger:?}"
            );
        }
    }

    #[test]
    fn states_and_triggers_are_snake_case_strings_on_the_wire() {
        let states: Vec<_> = STATES
            .iter()
            .map(|s| serde_json::to_value(s).unwrap())
            .collect();
        assert_eq!(states, ["queued", "running", "ready", "failed", "halted"]);
        let triggers: Vec<_> = TRIGGERS
            .iter()
            .map(|t| serde_json::to_value(t).unwrap())
            .collect();
        assert_eq!(triggers, ["start", "ready", "fail", "halt", "abandon"]);
        let back: AuxState = serde_json::from_str("\"halted\"").unwrap();
        assert_eq!(back, AuxState::Halted);
    }
}
```

Register and export the module in `crates/fleet-core/src/lib.rs`:

```diff
--- a/crates/fleet-core/src/lib.rs
+++ b/crates/fleet-core/src/lib.rs
@@ -5,12 +5,14 @@
 //! whose decisions must be exhaustively unit-testable. See
 //! `docs/superpowers/specs/2026-06-05-command-center-sp1-design.md`.
 
+mod auxiliary;
 mod event;
 mod gate;
 mod phase;
 mod tier;
 mod transition;
 
+pub use auxiliary::{aux_transition, AuxState, AuxTrigger};
 pub use event::{ArtifactKind, Command, ErrorScope, Event, IterationKind, LogStream, Severity};
 pub use gate::{gate_met, GateConfig, ReviewSnapshot};
 pub use phase::{Phase, TERMINAL_PHASE_STRS};
```

- [ ] **Step 2: Run them and watch them fail**

Run: `cargo test -p fleet-core auxiliary::`
Expected: the build fails with E0432 (unresolved imports `auxiliary::aux_transition`, `auxiliary::AuxState`, `auxiliary::AuxTrigger`).

- [ ] **Step 3: Implement the state machine**

Replace the first line of `crates/fleet-core/src/auxiliary.rs` (the `use serde::…;` line) with:

```rust
//! The auxiliary-run state machine: `(state, trigger) -> Option<state>`.
//!
//! An auxiliary run drafts a spec or adopts a repository. It writes only spec and configuration
//! files and has no oracle, no review and no pull request, so its life is much shorter than a
//! unit's. It cannot be resumed: after a halt or a failure, a new run is started.
//!
//! This file is `auxiliary.rs` and not `aux.rs` because `AUX` is a reserved device name on
//! Windows: git cannot add a file called `aux.rs` there.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuxState {
    Queued,
    Running,
    /// Terminal: the run's result was verified and is ready for a person.
    Ready,
    /// Terminal.
    Failed,
    /// Paused by a person. The only way on is to abandon it.
    Halted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuxTrigger {
    /// The run's harness process was started.
    Start,
    /// The harness ended `draft_ready` and the control plane verified the result.
    Ready,
    /// The harness ended `failed`, broke the protocol, breached a cap or stalled.
    Fail,
    Halt,
    Abandon,
}

/// Compute the next state, or `None` if the trigger is invalid here.
pub fn aux_transition(state: AuxState, trigger: AuxTrigger) -> Option<AuxState> {
    use AuxState::*;

    Some(match (state, trigger) {
        (Queued, AuxTrigger::Start) => Running,
        (Running, AuxTrigger::Ready) => Ready,
        (Running, AuxTrigger::Fail) => Failed,
        (Running, AuxTrigger::Halt) => Halted,
        (Halted, AuxTrigger::Abandon) => Failed,

        // Anything else is invalid in this state. There is no resume.
        _ => return None,
    })
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p fleet-core auxiliary::`
Expected: `test result: ok. 6 passed; 0 failed`.

Run: `cargo test -p fleet-core`
Expected: `test result: ok. 48 passed; 0 failed`.

- [ ] **Step 5: Commit**

```bash
cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings
git add crates/fleet-core/src/auxiliary.rs crates/fleet-core/src/lib.rs
git commit -m "feat(fleet-core): the auxiliary-run state machine"
```

### Task 16: `ErrorScope::Harness` and the `Reverify` command

Two additions to the control plane's own event and command types, in `event.rs`.

The supervisor spec (§2 item 3) has the control plane report errors that come from the harness process itself: it would not start, it broke the protocol, it exited without a result. The protocol crate's `ErrorScope` already has `Harness`; `fleet-core`'s does not, so such an error has no scope to carry.

`Reverify` is the command a person sends to have verification tried again. It leads to `Trigger::Reverify { from }` (Task 14), but this task adds the command only. The trigger needs the phase the unit was paused in, and the crate that records that phase builds the trigger.

**Files:**
- Modify: `crates/fleet-core/src/event.rs`
- Test: `crates/fleet-core/src/event.rs` (inline)

**Interfaces:**
- Consumes: `Trigger`, as `event.rs` does today. Nothing from Tasks 13 to 15.
- Produces:
  - `pub enum ErrorScope { Docker, Github, Agent, System, Harness }` — on the wire, `"harness"`
  - `Command::Reverify { cmd_id: String }` — on the wire, `{"command":"reverify","cmd_id":"…"}`, the same shape as `Halt`, `Resume`, `Abandon`, `Ship` and `RejectOracle`
  - `impl Command { pub fn to_trigger(&self) -> Option<Trigger> }` — it returned `Trigger` before this task

`to_trigger` is the one exhaustive match the new command breaks in a way that cannot be patched with a new arm: every other command names its trigger by itself, and `Reverify` cannot, because `Trigger::Reverify` carries a phase the command does not have. So the function now returns `Option<Trigger>` and gives `None` for `Reverify`. Nothing outside `fleet-core` calls it. `Command::cmd_id` is the other exhaustive match and takes one more arm.

- [ ] **Step 1: Write the failing tests**

In `crates/fleet-core/src/event.rs`, inside `mod tests`, add:

```rust
    #[test]
    fn error_scope_gains_harness_and_keeps_its_wire_names() {
        let scopes = [
            (ErrorScope::Docker, "docker"),
            (ErrorScope::Github, "github"),
            (ErrorScope::Agent, "agent"),
            (ErrorScope::System, "system"),
            (ErrorScope::Harness, "harness"),
        ];
        for (scope, wire) in scopes {
            assert_eq!(serde_json::to_value(scope).unwrap(), wire);
            let back: ErrorScope = serde_json::from_value(serde_json::json!(wire)).unwrap();
            assert_eq!(back, scope);
        }
    }

    #[test]
    fn a_harness_error_event_round_trips() {
        let e = Event::Error {
            scope: ErrorScope::Harness,
            retryable: false,
            detail: "exited without result".into(),
        };
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["type"], "error");
        assert_eq!(v["scope"], "harness");
        let back: Event = serde_json::from_value(v).unwrap();
        assert_eq!(back, e);
    }

    #[test]
    fn reverify_is_a_command_like_its_siblings_and_round_trips() {
        let c = Command::Reverify {
            cmd_id: "v1".into(),
        };
        let v = serde_json::to_value(&c).unwrap();
        assert_eq!(
            v,
            serde_json::json!({"command": "reverify", "cmd_id": "v1"})
        );
        let back: Command = serde_json::from_value(v).unwrap();
        assert_eq!(back, c);
        assert_eq!(back.cmd_id(), "v1");

        let parsed: Command =
            serde_json::from_str(r#"{"command":"reverify","cmd_id":"v2"}"#).unwrap();
        assert_eq!(parsed.cmd_id(), "v2");
    }

    #[test]
    fn every_command_but_reverify_names_its_trigger() {
        use crate::Trigger;
        let id = || "c".to_string();
        let mapped = [
            (Command::Halt { cmd_id: id() }, Trigger::Halt),
            (Command::Resume { cmd_id: id() }, Trigger::Resume),
            (Command::Abandon { cmd_id: id() }, Trigger::Abandon),
            (Command::Ship { cmd_id: id() }, Trigger::Ship),
            (
                Command::ApproveOracle {
                    cmd_id: id(),
                    edited_test_files: None,
                },
                Trigger::OracleApproved,
            ),
            (
                Command::RejectOracle { cmd_id: id() },
                Trigger::OracleRejected,
            ),
        ];
        for (command, trigger) in mapped {
            assert_eq!(command.to_trigger(), Some(trigger), "{command:?}");
        }
        // `Trigger::Reverify` carries the phase the unit was paused in. Only the caller knows it.
        assert_eq!(Command::Reverify { cmd_id: id() }.to_trigger(), None);
    }
```

Two existing tests compare `to_trigger()` with a bare trigger. Replace `command_parses_and_maps_to_trigger` and `approve_oracle_optional_edits_omitted_when_none` with:

```rust
    #[test]
    fn command_parses_and_maps_to_trigger() {
        let c: Command = serde_json::from_str(r#"{"command":"halt","cmd_id":"abc"}"#).unwrap();
        assert_eq!(c.cmd_id(), "abc");
        assert_eq!(c.to_trigger(), Some(crate::Trigger::Halt));
    }

    #[test]
    fn approve_oracle_optional_edits_omitted_when_none() {
        let c = Command::ApproveOracle {
            cmd_id: "x".into(),
            edited_test_files: None,
        };
        let v = serde_json::to_value(&c).unwrap();
        assert_eq!(v["command"], "approve_oracle");
        assert!(v.get("edited_test_files").is_none());
        assert_eq!(c.to_trigger(), Some(crate::Trigger::OracleApproved));
    }
```

- [ ] **Step 2: Run them and watch them fail**

Run: `cargo test -p fleet-core event::`
Expected: the build fails with seven errors: E0599 twice for `Harness` (no variant or associated item of that name on `ErrorScope`), E0599 twice for `Reverify` (no variant of that name on `Command`), and E0308 (mismatched types) three times, where a test compares `to_trigger()` with `Some(…)` or `None`.

- [ ] **Step 3: Add the scope**

Replace the definition of `ErrorScope` (derive, serde attribute and body) with:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorScope {
    Docker,
    Github,
    Agent,
    System,
    /// The harness process itself: it would not start, broke the protocol, or exited without a
    /// result.
    Harness,
}
```

- [ ] **Step 4: Add the command**

Replace the definition of `Command` and its `impl` block (from the doc comment above the enum to the closing brace of the `impl`) with:

```rust
/// One inbound control command. Each carries a client-generated `cmd_id` that the
/// daemon echoes back on the resulting `PhaseChanged`/`Error` event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum Command {
    Halt {
        cmd_id: String,
    },
    Resume {
        cmd_id: String,
    },
    Abandon {
        cmd_id: String,
    },
    /// The T3 final gate: human ships the PR from `NeedsHuman`.
    Ship {
        cmd_id: String,
    },
    /// Approve the frozen test set; may carry an edited set (T2/T3).
    ApproveOracle {
        cmd_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        edited_test_files: Option<Vec<String>>,
    },
    RejectOracle {
        cmd_id: String,
    },
    /// Try verification again for a unit that `VerifyStopped` paused. The harness is not started
    /// again.
    Reverify {
        cmd_id: String,
    },
}

impl Command {
    pub fn cmd_id(&self) -> &str {
        match self {
            Command::Halt { cmd_id }
            | Command::Resume { cmd_id }
            | Command::Abandon { cmd_id }
            | Command::Ship { cmd_id }
            | Command::ApproveOracle { cmd_id, .. }
            | Command::RejectOracle { cmd_id }
            | Command::Reverify { cmd_id } => cmd_id,
        }
    }

    /// Map a command to the state-machine trigger it requests. `Reverify` maps to `None`:
    /// `Trigger::Reverify` carries the phase the unit was paused in, and only the caller, which
    /// recorded that phase, can supply it.
    pub fn to_trigger(&self) -> Option<crate::Trigger> {
        use crate::Trigger;
        Some(match self {
            Command::Halt { .. } => Trigger::Halt,
            Command::Resume { .. } => Trigger::Resume,
            Command::Abandon { .. } => Trigger::Abandon,
            Command::Ship { .. } => Trigger::Ship,
            Command::ApproveOracle { .. } => Trigger::OracleApproved,
            Command::RejectOracle { .. } => Trigger::OracleRejected,
            Command::Reverify { .. } => return None,
        })
    }
}
```

- [ ] **Step 5: Run the tests**

Run: `cargo test -p fleet-core event::`
Expected: `test result: ok. 9 passed; 0 failed`.

Run: `cargo test -p fleet-core`
Expected: `test result: ok. 52 passed; 0 failed`.

Run: `cargo test -p fleetd`
Expected: 98 passed, 0 failed, 3 ignored. Nothing outside `fleet-core` has to change: every `match` on a `Command` in `fleetd` has a catch-all arm, which already answers an unexpected command with "not valid" for the phase; `fleetd` never calls `to_trigger`; and the cockpit types an error's scope as a string.

- [ ] **Step 6: Commit**

```bash
cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings
git add crates/fleet-core/src/event.rs
git commit -m "feat(fleet-core): ErrorScope::Harness and the Reverify command"
```

### Finishing lane CC-CORE

- [ ] **Step 1: Run every command in the lane's Verify table** and keep the output.

- [ ] **Step 2: Push and open the pull request**

```bash
git push -u origin feat/m0-fleet-core-delta
gh pr create --base factory/m0 --head feat/m0-fleet-core-delta --title "fleet-core: the unit state-machine delta and the auxiliary-run state machine" --body-file pr-body.md
```

Write `pr-body.md` outside the worktree's tracked files and do not commit it. It states: the rows added and the one existing assertion that changed (Task 13); that `Command::to_trigger` now returns `Option<Trigger>`, and why (Task 16); that the module file is `auxiliary.rs` and why; the output of each Verify command. It carries no "Generated with" footer. Do not merge.

---

## Self-review

Run on 2026-10-04 against this file.

**Every item of the scope maps to a task.**

| Scope item | Task |
|---|---|
| `PROTOCOL_VERSION = "0.2"`; `negotiate`; `initialize` carries `accepted_versions` | 1 |
| `Network`, `ControlKind`, `UnitKind`, `ProfileInfo`, `PresetInfo`; the six `Capabilities` fields | 2 |
| `SpecRef`, `Source`, `Scope`, `RepoCommands`, `RepoConfig`, `Controls`, `Parent`; the eleven `WorkOrder` fields | 2 |
| `FrozenFile`, `OracleFreeze`, `OracleFrozen { freeze }`; `Stage`, `StageStatus`, `CostBasis`; `UnitEvent::Stage`; the five `Metric` fields; the two `GateRequest::Oracle` fields | 3 |
| `Outcome` with five values; `StopReason`, `Stop`; `MapEvidence`, `TestReport`, `ControlStatus`, `ControlResult`, `Verdict`, `ReviewVerdict`, `ReviewEvidence`; the five `Evidence` fields; `UnitResult.stop` | 4 |
| `file_sha256`, `bundle_hash` (how frozen test files are hashed) | 5 |
| The schema document includes every new type | 2, 3, 4 (the list in `schema_defines_every_wire_type`); 12 notes why `schema.rs` needs no edit |
| The contract re-blessed and `CONTRACT_SHA256` re-pinned; the hash in the pull request body | 1 to 4; the lane's finishing steps |
| README method table and obligations, including the nine that 0.2 adds | 12 |
| `harness_protocol::monitor` as SP-2a §3.1 gives it, plus `InvalidResult` for `needs_human` without `stop` and `draft_ready` without `evidence` | 7 |
| `cases::drive` on the monitor with the SP-2a mapping; every existing kit test unchanged; `ExitStatus` captured | 8 |
| Defect 1: the start reply required, the handshake version checked | 8 |
| Defect 2: a line-length limit; invalid UTF-8 reported as such | 6 |
| Defect 3: `no_change` refused without evidence | 7 (monitor), 8 (kit test) |
| Kit 0.2 cases: `needs_human` accepted and the process exits; a `stage` event accepted; a minor mismatch refused; `oracle_frozen` with a freeze payload accepted | 10 |
| `harness-fake`, SP-2a §3.2: `oracle_frozen` at every tier; the review loop; `resume: true`; `resume { oracle_frozen: true }`; the startup line; `blockers` mode | 9 |
| `harness-fake`, 0.2: capabilities; all seven stage events; a freeze payload; a mode that ends `needs_human`; re-entry at building on every resume | 10 |
| `--work-order`, issue #87's done-when | 11 |
| `HarnessStopped`, `Integrated`, `VerifyStopped`, `Reverify { from }`, each with a table test including its rejections | 14 |
| `CapBreach` and `Stall` in `Provisioning`, `AwaitingOracleApproval`, `MergeCheck`; `(MergeCheck, OracleTampering)` | 13 |
| `AuxState`, `AuxTrigger`, `aux_transition` | 15 |
| `ErrorScope::Harness` in `fleet-core`; `Command::Reverify { cmd_id }` (added after the first draft, at the coordinator's request) | 16 |
| The metering exemption for `failed` and `needs_human`, and what `no_change` evidence describes, stated as rules in the README | 12 |

**No placeholder patterns.** Searched this file for "TBD", "TODO", "similar to Task", "appropriate", "as needed" and "etc.": none outside this sentence. Every step that changes code shows the code.

**The code is real.** Every listing was built task by task on a copy of `origin/main` at `a3fd1c8`, formatted with `rustfmt`, and run. The tree at the end of each task was built and tested, with two exceptions: Tasks 7 and 8 share one checkpoint, and so do Tasks 14 and 15. In the final state: `cargo test --workspace` gives 288 passed, 0 failed, 3 ignored (166 + 100 for CC-PROTO + 22 for CC-CORE); `cargo clippy --workspace --all-targets -- -D warnings` is clean; `cargo fmt --all -- --check` is clean; the CI conformance command prints `7 passed, 0 skipped, 0 failed`. The listings in this file were copied from that tree by script, not retyped.

**Which "expected failure" outputs were observed, and which are stated from the compiler's rules.** Observed: the contract drift and each pin failure (Tasks 1 to 4); the kit reporting `ExitedWithoutResult` (Task 6); the four `left` values against the old driver (Task 8); the peer tests failing before the fake changes, partly (Task 9, where five of the ten failed once three one-line edits had been applied early); the missing-README build failure (Task 11, Step 9); the README test (Task 12); the seven compile errors of Task 16. Stated, not observed: every compile error given as an expected failure (Tasks 1 to 5, 7, 8, 11, 14 and 15), and the runtime failures in Task 10 Step 2, Task 11 Step 6 and Task 13 Step 2. The error codes there are the ones those mistakes produce; the exact wording may differ by a path prefix.

**Names are consistent** across tasks and with the shared-interface list, with these additions and one placement, each forced or justified by the existing code:

| What | Why |
|---|---|
| `InitializeParams::accepted(&self) -> Vec<&str>` (added) | "An empty `accepted_versions` means `protocol_version` only" has to be decided in exactly one place, and both a harness and the control plane need it |
| `MAX_LINE_BYTES`, `ReadError::InvalidUtf8`, `ReadError::LineTooLong` (added) | The parked codec defect needs a limit and two honest errors. The list does not name them |
| `Violation::StartNotAcknowledged`, `KitConfig.work_order`, `parse_work_order`, `Session::exit_status` (added, kit only) | The kit's own types; not part of the contract |
| `file_sha256` and `bundle_hash` live in `hash.rs`, `negotiate` in `rpc.rs` | The list groups them under `types.rs`. All three are exported from the crate root, which is the path callers use |
| The module file is `auxiliary.rs`, not `aux.rs` | `aux.rs` cannot be added to git on Windows. Verified on the owner's machine |
| `version_compatible` kept, with a changed meaning | It is public today. It is now `negotiate(&[peer], PROTOCOL_VERSION)` |
| `Vec` fields added to existing types are always serialised (as `[]`) | The stated rule skips only `Option` fields. A 0.1 peer ignores unknown fields |
| `Inbound` carries `#[allow(clippy::large_enum_variant)]` | Its shape is fixed by the SP-2a spec, and `Result(UnitResult)` is large enough to fail `clippy -D warnings` without it |
| `Command::to_trigger` returns `Option<Trigger>`; it returned `Trigger` | `Command::Reverify` cannot name its trigger: `Trigger::Reverify` carries a phase the command does not have. This changes the signature of an existing public function. Nothing outside `fleet-core` calls it |
| `AuxState` and `AuxTrigger` are plain `snake_case` strings on the wire | The list gives no wire form. `Trigger` is tagged only because it has a variant with a field |

**Settled since the first draft.**

- The metering rule keeps `needs_human` exempt alongside `failed`. The README says so as a rule (Task 12).
- `no_change` keeps requiring `evidence`. The README says what that evidence describes (Task 12). `Evidence.delivery` is still a mandatory field of the type, so a `no_change` result names a delivery like any other.
- `ErrorScope::Harness` and `Command::Reverify` are built (Task 16).

**Gaps that could not be closed here.**

1. **Issue #82's wording of defect 1** ("its `protocol_version`") cannot mean the reply to `unit/start`, which is empty. This plan checks the version in the `initialize` reply. The owner should confirm that reading.
2. **`MAX_LINE_BYTES` is 4 MiB by this plan's choice.** The issue gives no number. An evidence list of test ids for a very large suite is the message most likely to approach it.
3. **A `needs_human` transcript is not replayed through `fleet-core`** in Task 9, because that needs `Trigger::HarnessStopped`, which is CC-CORE's. The first lane that has both (milestone 1's supervisor) should add that replay.
4. **`harness-fake` declares no presets.** It links no preset crate, and a hard-coded version string would go stale. If milestone 1's admission requires a declared preset, demo mode needs a decision.
5. **`Command::to_trigger` changed its return type** (Task 16). It is the only way to add `Reverify { cmd_id }` in its siblings' shape without inventing a phase. If the owner would rather keep the old signature, the alternative is to give the command a `from` field.
6. **`AuxState::Queued` accepts only `Start`.** A queued auxiliary run cannot be halted, abandoned or failed. The table is built as given; the scheduling lane should confirm it never needs to cancel a queued run.
7. **The order of `stage { provision, started }` and `observed { provisioned }`.** `harness-fake` sends the stage event first. The supervisor must accept informational events before `provisioned`.
8. **The `cargo xtask` commands in the two Verify tables** come from the scaffold plan and do not exist on `main` yet, so they were not run. Their expected outputs are that plan's, apart from the seven `PASS` lines, which this plan's kit produces.
9. **The spec file `docs/superpowers/specs/2026-09-16-sp2a-harness-supervisor-design.md`** is not on `main` at `a3fd1c8`; it arrives with pull request #96. The lanes need it merged before they start.
10. **The sandbox repository's test command** in the kit README's example (`node --test`) is a guess from the kit's existing smoke test. Whoever first runs the kit against a real harness should correct it.
