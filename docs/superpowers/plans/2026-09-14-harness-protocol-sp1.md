# Harness Protocol — SP-1 (protocol crate, conformance kit, harness-fake) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the harness protocol v0 as a Rust crate with a hash-pinned JSON Schema, a conformance kit that checks any harness binary over stdio, and `harness-fake`, a scripted harness that passes the kit and can be made to misbehave on purpose.

**Architecture:** `crates/harness-protocol` holds only wire types (serde + `schemars`), JSON-RPC message helpers, a newline-delimited codec, and schema export pinned by a contract test. `crates/harness-conformance` holds a `Session` that spawns a harness as a child process, the conformance cases, and two binaries: `harness-conformance` (the CLI kit) and `harness-fake`. Nothing in `fleetd` or `fleet-core` changes in SP-1.

**Tech Stack:** Rust 2021 (toolchain 1.93), `serde` 1, `serde_json` 1, `schemars` 1.2, `sha2` 0.10 (dev-only), std threads + `mpsc` (no async in SP-1).

**Spec:** NEXUS [`docs/specs/2026-09-14-swappable-harness-design.md`](https://github.com/adbarc92/nexus/blob/main/docs/specs/2026-09-14-swappable-harness-design.md) — §3 (protocol v0) and §6 items 1, 2 and 7. ADR: NEXUS `docs/adr/0003-swappable-harness.md` (proposed).

## Global Constraints

- **Protocol version:** `PROTOCOL_VERSION = "0.1"`. A mismatched **major** is refused with JSON-RPC error code `-32001`.
- **Transport:** JSON-RPC 2.0, one JSON object per line (`\n`), over the harness's stdin/stdout. Stderr is free-form log.
- **Wire names are exactly the spec's:** `initialize`, `unit/start`, `unit/event`, `gate/request`, `unit/halt`, `unit/resume`, `unit/abandon`, `unit/result`.
- **Enum wire strings are snake_case**; tiers are `t1`/`t2`/`t3` (matching `fleet-core`'s `Tier` serde, `crates/fleet-core/src/tier.rs:7`).
- **`fleet-core` and `fleetd` are not modified in SP-1.** The protocol crate declares its own mirror enums; mapping to `fleet-core` is SP-2.
- **Canonical contract hash** = SHA-256 of `serde_json::to_string(&serde_json::from_str::<Value>(file))`, which equals NEXUS `docs/contracts/README.md`'s `JSON.stringify(JSON.parse(raw))` because the committed file is written from a `serde_json::Value` (sorted keys). Verified by spike, 2026-09-14.
- **New dependencies allowed:** `schemars = "1.2"` (runtime, protocol crate only) and `sha2 = "0.10"` (dev-dependency only). No others.
- **CI must stay green under** `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace`.
- **Windows:** child-process kill is `Child::kill()` in SP-1; Job Objects are SP-2 (spec §9).

**Deviation from spec, recorded:** spec §2 names `fake.rs`'s `FakeRunner` as the basis of `harness-fake`. `FakeRunner` is a `Runner` inside `fleetd`'s driver and cannot speak the protocol without SP-2's driver extraction, so SP-1's `harness-fake` is a standalone scripted protocol speaker. `FakeRunner` moves in SP-2.

## File Structure

| Path | Responsibility |
|---|---|
| `Cargo.toml` (modify) | add the two crates to `members` |
| `crates/harness-protocol/Cargo.toml` | crate manifest |
| `crates/harness-protocol/src/lib.rs` | module wiring, `PROTOCOL_VERSION`, re-exports |
| `crates/harness-protocol/src/types.rs` | every params/result payload type |
| `crates/harness-protocol/src/rpc.rs` | `RpcMessage`, `RpcError`, method + error-code constants, constructors, `MessageKind` |
| `crates/harness-protocol/src/codec.rs` | `write_message`, `read_message`, `ReadError` |
| `crates/harness-protocol/src/schema.rs` | `ProtocolSchema` root + `schema_json()` |
| `crates/harness-protocol/contract/harness-protocol.contract.json` | committed schema (the contract) |
| `crates/harness-protocol/tests/contract.rs` | drift gate + canonical-hash pin |
| `crates/harness-conformance/Cargo.toml` | crate manifest with two `[[bin]]`s |
| `crates/harness-conformance/src/lib.rs` | `KitConfig`, `CaseReport`, `CaseOutcome`, `Violation`, `run_all` |
| `crates/harness-conformance/src/session.rs` | spawn child, send, receive-with-timeout, exit/kill |
| `crates/harness-conformance/src/fixtures.rs` | the work orders the kit sends |
| `crates/harness-conformance/src/cases.rs` | the conformance cases |
| `crates/harness-conformance/src/bin/harness-fake.rs` | scripted harness, `HARNESS_FAKE_MODE` selects behaviour |
| `crates/harness-conformance/src/bin/harness-conformance.rs` | CLI |
| `crates/harness-conformance/tests/fake_conforms.rs` | kit passes the conformant fake |
| `crates/harness-conformance/tests/detects_violations.rs` | kit fails each misbehaving fake mode |
| `crates/harness-conformance/tests/cli.rs` | CLI exit codes |
| `crates/harness-protocol/README.md` | how a third party implements and self-checks a harness |
| `.github/workflows/ci.yml` (modify) | `harness conformance` job |

---

### Task 1: `harness-protocol` crate and payload types

**Files:**
- Modify: `Cargo.toml` (workspace `members`)
- Create: `crates/harness-protocol/Cargo.toml`
- Create: `crates/harness-protocol/src/lib.rs`
- Create: `crates/harness-protocol/src/types.rs` (tests inline)

**Interfaces:**
- Produces (all `pub`, all derive `Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema`):
  `PROTOCOL_VERSION: &str`; `Tier{T1,T2,T3}`; `Isolation{Container,Host,None}`; `Metering{Usd,None}`; `GateKind{Oracle}`; `Delivery{Bundle,Push}`; `Capabilities{isolation, metering, gates: Vec<GateKind>, delivery, resume: bool, halt: bool}`; `HarnessInfo{name, version}`; `InitializeParams{protocol_version}`; `InitializeResult{protocol_version, harness, capabilities}`; `WorkItemKind{RoadmapItem,Issue}`; `WorkItem{kind, reference /* wire "ref" */, fingerprint: Option<String>}`; `Repo{url, slug, base_branch}`; `Caps{usd: f64, wall_clock_secs: u64, min_review_rounds: u32}`; `Resume{oracle_frozen: bool}`; `WorkOrder{unit_id, work_item, tier, task, repo, branch, test_cmd, caps, resume: Option<Resume>}`; `Observation{Provisioned, OracleFrozen, BuildFinished, ChecksPassed, ChecksFailed, EmptyDiff, ReviewFinished{round: u32, unresolved_blockers: u32, checks_green: bool}}`; `LogStream{Agent,Check,System}`; `Severity{Info,Minor,Blocker}`; `ArtifactKind{Branch,Pr,Diff}`; `ErrorScope{Docker,Github,Agent,System,Harness}`; `UnitEvent{Observed{observation}, Metric{tokens_in: u64, tokens_out: u64, cost_usd: f64, elapsed_ms: u64}, Log{stream, line}, Finding{round, severity, title, file: Option<String>, resolved: bool}, Artifact{kind, reference}, Error{scope, retryable: bool, detail}}`; `GateRequest{Oracle{test_files: Vec<String>, hash, summary}}`; `GateReply{approved: bool, edited_test_files: Option<Vec<String>>}`; `Outcome{PrOpen,NoChange,Failed}`; `DeliveryEvidence{Bundle{bundle_path}, Push}`; `PrRef{url, number: u64}`; `TestRun{command, exit_code: i32}`; `Evidence{branch, head_sha, delivery: DeliveryEvidence, pr: Option<PrRef>, test: TestRun, oracle_hash: Option<String>}`; `Failure{scope, detail}`; `UnitResult{outcome, evidence: Option<Evidence>, failure: Option<Failure>}`; `Empty{}`.
  Plus `impl Tier { pub fn requires_oracle(self) -> bool }` (true for `T2`/`T3`).

- [ ] **Step 1: Add the crate to the workspace and create its manifest**

`Cargo.toml` — change the `members` line to:

```toml
members = ["crates/fleet-core", "crates/fleetd", "crates/harness-protocol"]
```

`crates/harness-protocol/Cargo.toml`:

```toml
[package]
name = "harness-protocol"
edition.workspace = true
version.workspace = true
license.workspace = true
description = "Wire types for the command-center harness protocol: JSON-RPC 2.0 over stdio."

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
schemars = "1.2"

[dev-dependencies]
sha2 = "0.10"
```

`crates/harness-protocol/src/lib.rs`:

```rust
//! `harness-protocol` — the wire contract between the `fleetd` control plane and a harness.
//!
//! A harness is a child process that runs one unit of work. It speaks JSON-RPC 2.0, one message
//! per line, over stdin/stdout. This crate holds only the shapes on that wire; it has no IO beyond
//! the line codec and no knowledge of `fleet-core`. See NEXUS
//! `docs/specs/2026-09-14-swappable-harness-design.md` §3.

mod types;

pub use types::*;

/// The protocol version this crate speaks. A peer with a different **major** is refused.
pub const PROTOCOL_VERSION: &str = "0.1";
```

- [ ] **Step 2: Write the failing wire-shape tests**

Create `crates/harness-protocol/src/types.rs` containing only this test module for now:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, to_value};

    #[test]
    fn tier_and_enums_are_snake_case_on_the_wire() {
        assert_eq!(to_value(Tier::T2).unwrap(), json!("t2"));
        assert_eq!(to_value(Isolation::Container).unwrap(), json!("container"));
        assert_eq!(to_value(WorkItemKind::RoadmapItem).unwrap(), json!("roadmap_item"));
        assert_eq!(to_value(Outcome::PrOpen).unwrap(), json!("pr_open"));
        assert!(Tier::T2.requires_oracle() && Tier::T3.requires_oracle() && !Tier::T1.requires_oracle());
    }

    #[test]
    fn work_item_reference_is_ref_and_optional_fields_are_omitted() {
        let item = WorkItem {
            kind: WorkItemKind::Issue,
            reference: "adbarc92/audience#82".into(),
            fingerprint: None,
        };
        assert_eq!(
            to_value(&item).unwrap(),
            json!({"kind": "issue", "ref": "adbarc92/audience#82"})
        );
    }

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
        let metric = UnitEvent::Metric { tokens_in: 10, tokens_out: 2, cost_usd: 0.5, elapsed_ms: 7 };
        assert_eq!(to_value(&metric).unwrap()["type"], json!("metric"));
    }

    #[test]
    fn evidence_delivery_is_kind_tagged() {
        let ev = Evidence {
            branch: "agent/u1".into(),
            head_sha: "a".repeat(40),
            delivery: DeliveryEvidence::Bundle { bundle_path: "u1.bundle".into() },
            pr: None,
            test: TestRun { command: "cargo test".into(), exit_code: 0 },
            oracle_hash: None,
        };
        let v = to_value(&ev).unwrap();
        assert_eq!(v["delivery"], json!({"kind": "bundle", "bundle_path": "u1.bundle"}));
        assert!(v.get("pr").is_none() && v.get("oracle_hash").is_none());
        assert_eq!(to_value(DeliveryEvidence::Push).unwrap(), json!({"kind": "push"}));
    }

    #[test]
    fn gate_request_is_gate_tagged_and_round_trips() {
        let req = GateRequest::Oracle {
            test_files: vec!["tests/a.test.js".into()],
            hash: "h".into(),
            summary: "s".into(),
        };
        let v = to_value(&req).unwrap();
        assert_eq!(v["gate"], json!("oracle"));
        let back: GateRequest = serde_json::from_value(v).unwrap();
        assert_eq!(back, req);
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p harness-protocol`
Expected: FAIL to compile — `cannot find type Tier in this scope` (and the other types).

- [ ] **Step 4: Implement the types above the test module**

Insert at the top of `crates/harness-protocol/src/types.rs`:

```rust
//! Payload types for every params/result on the harness wire (spec §3).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    T1,
    T2,
    T3,
}

impl Tier {
    /// T2/T3 need a human-approved oracle before building (mirrors `fleet-core` `Tier`).
    pub fn requires_oracle(self) -> bool {
        matches!(self, Tier::T2 | Tier::T3)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Isolation {
    Container,
    Host,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Metering {
    Usd,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GateKind {
    Oracle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Delivery {
    Bundle,
    Push,
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
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct HarnessInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct InitializeParams {
    pub protocol_version: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct InitializeResult {
    pub protocol_version: String,
    pub harness: HarnessInfo,
    pub capabilities: Capabilities,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkItemKind {
    RoadmapItem,
    Issue,
}

/// The id that must travel end to end (SMOKE-LAP-01 item 14: nothing carried it).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct WorkItem {
    pub kind: WorkItemKind,
    #[serde(rename = "ref")]
    pub reference: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Repo {
    pub url: String,
    pub slug: String,
    pub base_branch: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Caps {
    pub usd: f64,
    pub wall_clock_secs: u64,
    pub min_review_rounds: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Resume {
    pub oracle_frozen: bool,
}

/// `unit/start` params.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct WorkOrder {
    pub unit_id: String,
    pub work_item: WorkItem,
    pub tier: Tier,
    pub task: String,
    pub repo: Repo,
    pub branch: String,
    pub test_cmd: String,
    pub caps: Caps,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resume: Option<Resume>,
}

/// What the harness saw. `fleetd` maps each to a `fleet-core` trigger (SP-2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Observation {
    Provisioned,
    OracleFrozen,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LogStream {
    Agent,
    Check,
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Minor,
    Blocker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    Branch,
    Pr,
    Diff,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ErrorScope {
    Docker,
    Github,
    Agent,
    System,
    Harness,
}

/// `unit/event` params (a notification).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UnitEvent {
    Observed {
        observation: Observation,
    },
    Metric {
        tokens_in: u64,
        tokens_out: u64,
        cost_usd: f64,
        elapsed_ms: u64,
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

/// `gate/request` params (a request the harness blocks on).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "gate", rename_all = "snake_case")]
pub enum GateRequest {
    Oracle {
        test_files: Vec<String>,
        hash: String,
        summary: String,
    },
}

/// The control plane's answer to `gate/request`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct GateReply {
    pub approved: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edited_test_files: Option<Vec<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    PrOpen,
    NoChange,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DeliveryEvidence {
    Bundle { bundle_path: String },
    Push,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PrRef {
    pub url: String,
    pub number: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TestRun {
    pub command: String,
    pub exit_code: i32,
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
}

/// Params/result for messages that carry nothing (`unit/halt`, `unit/resume`, `unit/abandon`).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub struct Empty {}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p harness-protocol`
Expected: PASS — 5 tests.

- [ ] **Step 6: Lint**

Run: `cargo fmt --all -- --check && cargo clippy -p harness-protocol --all-targets -- -D warnings`
Expected: no output, exit 0.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml Cargo.lock crates/harness-protocol
git commit -m "feat(harness-protocol): wire payload types for protocol v0"
```

---

### Task 2: JSON-RPC envelope, method names, and the line codec

**Files:**
- Create: `crates/harness-protocol/src/rpc.rs` (tests inline)
- Create: `crates/harness-protocol/src/codec.rs` (tests inline)
- Modify: `crates/harness-protocol/src/lib.rs`

**Interfaces:**
- Consumes: `PROTOCOL_VERSION` (Task 1).
- Produces:
  - `pub mod method { INITIALIZE, UNIT_START, UNIT_EVENT, GATE_REQUEST, UNIT_HALT, UNIT_RESUME, UNIT_ABANDON, UNIT_RESULT: &str }`
  - `pub mod error_code { METHOD_NOT_FOUND = -32601, INVALID_PARAMS = -32602, PROTOCOL_VERSION_UNSUPPORTED = -32001 }` (all `i64`)
  - `pub struct RpcError { code: i64, message: String }`
  - `pub struct RpcMessage { jsonrpc: String, id: Option<u64>, method: Option<String>, params: Option<Value>, result: Option<Value>, error: Option<RpcError> }` with
    `fn request<P: Serialize>(id: u64, method: &str, params: &P) -> Self`,
    `fn notification<P: Serialize>(method: &str, params: &P) -> Self`,
    `fn response<R: Serialize>(id: u64, result: &R) -> Self`,
    `fn error(id: u64, code: i64, message: impl Into<String>) -> Self`,
    `fn kind(&self) -> MessageKind<'_>`,
    `fn params_as<T: DeserializeOwned>(&self) -> Result<T, serde_json::Error>`,
    `fn result_as<T: DeserializeOwned>(&self) -> Result<T, serde_json::Error>`
  - `pub enum MessageKind<'a> { Request { id: u64, method: &'a str }, Notification { method: &'a str }, Response { id: u64 }, ErrorResponse { id: u64, error: &'a RpcError }, Invalid }`
  - `pub fn version_compatible(peer: &str) -> bool` — true iff `peer`'s major equals `PROTOCOL_VERSION`'s major
  - `pub enum ReadError { Eof, Malformed { line: String, error: String }, Io(std::io::Error) }`
  - `pub fn write_message<W: Write>(w: &mut W, msg: &RpcMessage) -> std::io::Result<()>`
  - `pub fn read_message<R: BufRead>(r: &mut R) -> Result<RpcMessage, ReadError>`

- [ ] **Step 1: Write the failing tests**

`crates/harness-protocol/src/rpc.rs` — test module only for now:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Empty, InitializeParams};
    use serde_json::json;

    #[test]
    fn request_omits_absent_fields_on_the_wire() {
        let msg = RpcMessage::request(
            1,
            method::INITIALIZE,
            &InitializeParams { protocol_version: "0.1".into() },
        );
        assert_eq!(
            serde_json::to_value(&msg).unwrap(),
            json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocol_version": "0.1"}})
        );
    }

    #[test]
    fn kind_classifies_all_four_shapes_and_rejects_the_rest() {
        let req = RpcMessage::request(7, method::UNIT_HALT, &Empty {});
        assert_eq!(req.kind(), MessageKind::Request { id: 7, method: "unit/halt" });

        let note = RpcMessage::notification(method::UNIT_EVENT, &Empty {});
        assert_eq!(note.kind(), MessageKind::Notification { method: "unit/event" });

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

        let both: RpcMessage =
            serde_json::from_value(json!({"jsonrpc": "2.0", "id": 1, "result": {}, "error": {"code": 1, "message": "x"}}))
                .unwrap();
        assert_eq!(both.kind(), MessageKind::Invalid);
    }

    #[test]
    fn params_round_trip_through_params_as() {
        let msg = RpcMessage::request(2, method::INITIALIZE, &InitializeParams { protocol_version: "0.1".into() });
        let back: InitializeParams = msg.params_as().unwrap();
        assert_eq!(back.protocol_version, "0.1");
    }

    #[test]
    fn version_compatibility_is_by_major() {
        assert!(version_compatible("0.1"));
        assert!(version_compatible("0.9"));
        assert!(!version_compatible("1.0"));
        assert!(!version_compatible("99.0"));
        assert!(!version_compatible("not-a-version"));
    }
}
```

`crates/harness-protocol/src/codec.rs` — test module only for now:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{method, Empty, RpcMessage};
    use std::io::Cursor;

    #[test]
    fn write_then_read_round_trips_and_skips_blank_lines() {
        let mut buf = Vec::new();
        write_message(&mut buf, &RpcMessage::notification(method::UNIT_EVENT, &Empty {})).unwrap();
        buf.extend_from_slice(b"\n\n");
        write_message(&mut buf, &RpcMessage::response(3, &Empty {})).unwrap();
        assert_eq!(buf.iter().filter(|b| **b == b'\n').count(), 4);

        let mut r = Cursor::new(buf);
        assert_eq!(read_message(&mut r).unwrap().method.as_deref(), Some("unit/event"));
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
}
```

Modify `crates/harness-protocol/src/lib.rs` — replace `mod types;` / `pub use types::*;` with:

```rust
mod codec;
mod rpc;
mod types;

pub use codec::{read_message, write_message, ReadError};
pub use rpc::{error_code, method, version_compatible, MessageKind, RpcError, RpcMessage};
pub use types::*;
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p harness-protocol`
Expected: FAIL to compile — `cannot find type RpcMessage` / `cannot find function write_message`.

- [ ] **Step 3: Implement `rpc.rs` above its test module**

```rust
//! JSON-RPC 2.0 envelopes, the protocol's method names, and its error codes.

use schemars::JsonSchema;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;

/// Method names, exactly as on the wire (spec §3).
pub mod method {
    pub const INITIALIZE: &str = "initialize";
    pub const UNIT_START: &str = "unit/start";
    pub const UNIT_EVENT: &str = "unit/event";
    pub const GATE_REQUEST: &str = "gate/request";
    pub const UNIT_HALT: &str = "unit/halt";
    pub const UNIT_RESUME: &str = "unit/resume";
    pub const UNIT_ABANDON: &str = "unit/abandon";
    pub const UNIT_RESULT: &str = "unit/result";
}

/// JSON-RPC error codes this protocol uses.
pub mod error_code {
    pub const METHOD_NOT_FOUND: i64 = -32601;
    pub const INVALID_PARAMS: i64 = -32602;
    /// The peer's protocol major differs from ours.
    pub const PROTOCOL_VERSION_UNSUPPORTED: i64 = -32001;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
}

/// One line on the wire. Which fields are present decides its kind (`RpcMessage::kind`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RpcMessage {
    pub jsonrpc: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MessageKind<'a> {
    Request { id: u64, method: &'a str },
    Notification { method: &'a str },
    Response { id: u64 },
    ErrorResponse { id: u64, error: &'a RpcError },
    Invalid,
}

fn to_json<T: Serialize>(value: &T) -> Value {
    serde_json::to_value(value).expect("protocol payload types always serialize")
}

impl RpcMessage {
    fn empty() -> Self {
        Self { jsonrpc: "2.0".into(), id: None, method: None, params: None, result: None, error: None }
    }

    pub fn request<P: Serialize>(id: u64, method: &str, params: &P) -> Self {
        Self { id: Some(id), method: Some(method.into()), params: Some(to_json(params)), ..Self::empty() }
    }

    pub fn notification<P: Serialize>(method: &str, params: &P) -> Self {
        Self { method: Some(method.into()), params: Some(to_json(params)), ..Self::empty() }
    }

    pub fn response<R: Serialize>(id: u64, result: &R) -> Self {
        Self { id: Some(id), result: Some(to_json(result)), ..Self::empty() }
    }

    pub fn error(id: u64, code: i64, message: impl Into<String>) -> Self {
        Self { id: Some(id), error: Some(RpcError { code, message: message.into() }), ..Self::empty() }
    }

    pub fn kind(&self) -> MessageKind<'_> {
        if self.jsonrpc != "2.0" {
            return MessageKind::Invalid;
        }
        match (self.id, self.method.as_deref(), &self.result, &self.error) {
            (Some(id), Some(method), None, None) => MessageKind::Request { id, method },
            (None, Some(method), None, None) => MessageKind::Notification { method },
            (Some(id), None, Some(_), None) => MessageKind::Response { id },
            (Some(id), None, None, Some(error)) => MessageKind::ErrorResponse { id, error },
            _ => MessageKind::Invalid,
        }
    }

    pub fn params_as<T: DeserializeOwned>(&self) -> Result<T, serde_json::Error> {
        serde_json::from_value(self.params.clone().unwrap_or(Value::Null))
    }

    pub fn result_as<T: DeserializeOwned>(&self) -> Result<T, serde_json::Error> {
        serde_json::from_value(self.result.clone().unwrap_or(Value::Null))
    }
}

fn major(version: &str) -> Option<u64> {
    version.split('.').next()?.parse().ok()
}

/// True iff `peer` has the same major version as this crate's `PROTOCOL_VERSION`.
pub fn version_compatible(peer: &str) -> bool {
    match (major(peer), major(crate::PROTOCOL_VERSION)) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}
```

- [ ] **Step 4: Implement `codec.rs` above its test module**

```rust
//! Newline-delimited framing: exactly one JSON-RPC message per line.

use crate::rpc::RpcMessage;
use std::io::{self, BufRead, Write};

#[derive(Debug)]
pub enum ReadError {
    /// The peer closed its end.
    Eof,
    /// A non-empty line that is not a JSON-RPC message. `line` is kept verbatim for the report.
    Malformed { line: String, error: String },
    Io(io::Error),
}

/// Write `msg` as one line and flush, so the peer sees it immediately.
pub fn write_message<W: Write>(w: &mut W, msg: &RpcMessage) -> io::Result<()> {
    let line = serde_json::to_string(msg).map_err(io::Error::other)?;
    w.write_all(line.as_bytes())?;
    w.write_all(b"\n")?;
    w.flush()
}

/// Read the next message, skipping blank lines.
pub fn read_message<R: BufRead>(r: &mut R) -> Result<RpcMessage, ReadError> {
    loop {
        let mut line = String::new();
        let n = r.read_line(&mut line).map_err(ReadError::Io)?;
        if n == 0 {
            return Err(ReadError::Eof);
        }
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

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p harness-protocol`
Expected: PASS — 11 tests (5 from Task 1, 4 rpc, 2 codec).

- [ ] **Step 6: Lint**

Run: `cargo fmt --all && cargo clippy -p harness-protocol --all-targets -- -D warnings`
Expected: exit 0, no warnings.

- [ ] **Step 7: Commit**

```bash
git add crates/harness-protocol
git commit -m "feat(harness-protocol): JSON-RPC envelope, method names, line codec"
```

---

### Task 3: JSON Schema export and the hash-pinned contract

**Files:**
- Create: `crates/harness-protocol/src/schema.rs`
- Modify: `crates/harness-protocol/src/lib.rs`
- Create: `crates/harness-protocol/tests/contract.rs`
- Create (generated, committed): `crates/harness-protocol/contract/harness-protocol.contract.json`

**Interfaces:**
- Consumes: every type from Tasks 1–2.
- Produces: `pub struct ProtocolSchema` (JsonSchema-only root), `pub fn schema_json() -> String` (pretty JSON written from a `serde_json::Value`, trailing newline), and the committed contract file plus its canonical SHA-256 — the value Task 10 registers in NEXUS.

**Why the hash check agrees with NEXUS's rule:** the committed file is written from a `serde_json::Value`, so reparsing it and re-serialising compactly (Rust) and `JSON.stringify(JSON.parse(raw))` (Node) produce the same bytes in the same key order. The 2026-09-14 spike measured identical digests with `schemars` 1.2.2. CRLF checkouts don't matter because both sides hash parsed JSON.

- [ ] **Step 1: Write the failing contract tests**

`crates/harness-protocol/tests/contract.rs`:

```rust
//! The harness protocol is a cross-repository wire contract (NEXUS `docs/contracts/README.md`).
//! Two guarantees: the committed schema equals what the types generate (drift gate), and the
//! committed file's canonical SHA-256 equals the pinned constant (tamper-evidence). To change the
//! protocol on purpose: re-bless, update `CONTRACT_SHA256`, and update the NEXUS registry row.

use harness_protocol::schema_json;
use sha2::{Digest, Sha256};
use std::path::PathBuf;

/// Canonical SHA-256 of `contract/harness-protocol.contract.json`. Set in Task 3 Step 5.
const CONTRACT_SHA256: &str = "0000000000000000000000000000000000000000000000000000000000000000";

fn contract_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("contract")
        .join("harness-protocol.contract.json")
}

fn canonical_sha256(raw: &str) -> String {
    let value: serde_json::Value = serde_json::from_str(raw).expect("contract is valid JSON");
    let canonical = serde_json::to_string(&value).expect("contract reserializes");
    format!("{:x}", Sha256::digest(canonical.as_bytes()))
}

#[test]
fn generated_schema_matches_the_committed_contract() {
    let generated = schema_json();
    if std::env::var_os("HARNESS_PROTOCOL_BLESS").is_some() {
        std::fs::create_dir_all(contract_path().parent().unwrap()).unwrap();
        std::fs::write(contract_path(), &generated).unwrap();
    }
    let committed = std::fs::read_to_string(contract_path())
        .expect("contract file missing: run once with HARNESS_PROTOCOL_BLESS=1");
    let committed: serde_json::Value = serde_json::from_str(&committed).unwrap();
    let generated: serde_json::Value = serde_json::from_str(&generated).unwrap();
    assert_eq!(
        committed, generated,
        "wire types changed: re-bless the contract, update CONTRACT_SHA256 and the NEXUS registry"
    );
}

#[test]
fn committed_contract_hash_is_pinned() {
    let committed = std::fs::read_to_string(contract_path()).expect("contract file exists");
    assert_eq!(
        canonical_sha256(&committed),
        CONTRACT_SHA256,
        "contract moved: this constant and NEXUS docs/contracts/README.md must change together"
    );
}

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
        "WorkOrder",
        "UnitEvent",
        "Observation",
        "GateRequest",
        "GateReply",
        "UnitResult",
        "Evidence",
        "Empty",
    ] {
        assert!(defs.contains_key(name), "schema is missing $defs.{name}");
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p harness-protocol --test contract`
Expected: FAIL to compile — `unresolved import harness_protocol::schema_json`.

- [ ] **Step 3: Implement `schema.rs` and export it**

`crates/harness-protocol/src/schema.rs`:

```rust
//! The protocol's JSON Schema: one document whose `$defs` hold every wire type.

use crate::rpc::{RpcError, RpcMessage};
use crate::types::{
    Empty, GateReply, GateRequest, InitializeParams, InitializeResult, UnitEvent, UnitResult, WorkOrder,
};
use schemars::{schema_for, JsonSchema};

/// Exists only to pull every wire type into a single schema document.
/// Field names say which message each type belongs to.
#[derive(JsonSchema)]
pub struct ProtocolSchema {
    pub message: RpcMessage,
    pub rpc_error: RpcError,
    pub initialize_params: InitializeParams,
    pub initialize_result: InitializeResult,
    pub unit_start_params: WorkOrder,
    pub unit_event_params: UnitEvent,
    pub gate_request_params: GateRequest,
    pub gate_reply: GateReply,
    pub unit_result_params: UnitResult,
    pub empty_params: Empty,
}

/// The schema as pretty JSON, written from a `serde_json::Value`, with a trailing newline.
/// This is the committed contract's exact form.
pub fn schema_json() -> String {
    let value = serde_json::to_value(schema_for!(ProtocolSchema)).expect("schema serializes");
    let mut out = serde_json::to_string_pretty(&value).expect("schema serializes");
    out.push('\n');
    out
}
```

In `crates/harness-protocol/src/lib.rs`, add `mod schema;` beside the other modules and this re-export:

```rust
pub use schema::{schema_json, ProtocolSchema};
```

- [ ] **Step 4: Generate the contract file**

bash:
```bash
HARNESS_PROTOCOL_BLESS=1 cargo test -p harness-protocol --test contract
```
PowerShell:
```powershell
$env:HARNESS_PROTOCOL_BLESS = '1'; cargo test -p harness-protocol --test contract; Remove-Item Env:HARNESS_PROTOCOL_BLESS
```
Expected: `generated_schema_matches_the_committed_contract` and `schema_defines_every_wire_type` PASS; `committed_contract_hash_is_pinned` FAILS, and its `left:` value is the real canonical hash. `crates/harness-protocol/contract/harness-protocol.contract.json` now exists.

- [ ] **Step 5: Pin the hash, cross-checked with Node**

Run from the repo root:
```bash
node -e "const c=require('crypto');const f=require('fs').readFileSync('crates/harness-protocol/contract/harness-protocol.contract.json','utf8');console.log(c.createHash('sha256').update(JSON.stringify(JSON.parse(f))).digest('hex'))"
```
Expected: a 64-hex-character digest **equal to** the `left:` value from Step 4. If they differ, stop: the canonicalisation assumption is broken, and this plan's Global Constraints need revisiting before continuing.

Replace the 64 zeros in `CONTRACT_SHA256` (`tests/contract.rs`) with that digest.

- [ ] **Step 6: Run the tests to verify they pass without blessing**

Run: `cargo test -p harness-protocol`
Expected: PASS — 14 tests (11 unit + 3 contract).

- [ ] **Step 7: Lint**

Run: `cargo fmt --all && cargo clippy -p harness-protocol --all-targets -- -D warnings`
Expected: exit 0.

- [ ] **Step 8: Commit**

```bash
git add crates/harness-protocol
git commit -m "feat(harness-protocol): JSON Schema export and hash-pinned contract"
```

---

### Task 4: `harness-fake`, a scripted harness

**Files:**
- Modify: `Cargo.toml` (workspace `members`)
- Create: `crates/harness-conformance/Cargo.toml`
- Create: `crates/harness-conformance/src/bin/harness-fake.rs`
- Test: `crates/harness-conformance/tests/fake_smoke.rs`

**Interfaces:**
- Consumes: everything exported by `harness-protocol` (Tasks 1–3).
- Produces: binary `harness-fake` (tests reach it as `env!("CARGO_BIN_EXE_harness-fake")`).
  - **Environment:**
    - `HARNESS_FAKE_MODE` ∈ `conformant` (default), `crash`, `malformed`, `event_after_result`, `silent_metering`, `hang`, `unknown_method`, `accept_any_version`.
    - `HARNESS_FAKE_STEP_MS` is the pause between events (default `50`).
  - **Declared capabilities:** `isolation: none, metering: usd, gates: [oracle], delivery: bundle, resume: false, halt: true`.
  - **Conformant behaviour, in order:**
    1. Answer `initialize`, or reply `-32001` and exit if the major version differs.
    2. Answer `unit/start` with `{}`.
    3. Send `observed: provisioned`.
    4. At T2/T3, send `observed: oracle_frozen`, then `gate/request{gate: oracle}` (request id `1000`). If the reply rejects, send `unit/result{outcome: failed, failure: {scope: agent, detail: "oracle rejected"}}` and exit.
    5. Send `metric`, `observed: build_finished`, `observed: checks_passed`, `observed: review_finished{round: max(min_review_rounds, 1), unresolved_blockers: 0, checks_green: true}`, and `artifact{kind: branch, ref: <branch>}`.
    6. Send `unit/result{outcome: pr_open, evidence{branch, head_sha: "0123456789abcdef0123456789abcdef01234567", delivery: bundle{bundle_path: "<unit_id>.bundle"}, test{command: <test_cmd>, exit_code: 0}, oracle_hash: "fake-oracle-hash" at T2/T3}}`.
    7. Exit 0.
  - **Interrupts:** while pausing or waiting on a gate, `unit/halt` or `unit/abandon` is answered with `{}` and the harness exits 0 **without** sending `unit/result`. `unit/resume` is answered with `{}`. Any other request gets `-32601`.

- [ ] **Step 1: Add the crate and write the failing smoke test**

`Cargo.toml` — `members` becomes:

```toml
members = ["crates/fleet-core", "crates/fleetd", "crates/harness-protocol", "crates/harness-conformance"]
```

`crates/harness-conformance/Cargo.toml`:

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

[[bin]]
name = "harness-fake"
path = "src/bin/harness-fake.rs"
```

`crates/harness-conformance/tests/fake_smoke.rs`:

```rust
//! Drives `harness-fake` by hand over the codec, before the kit exists.

use harness_protocol::{
    error_code, method, read_message, write_message, Caps, InitializeParams, InitializeResult,
    MessageKind, Outcome, ReadError, Repo, RpcMessage, Tier, UnitEvent, UnitResult, WorkItem,
    WorkItemKind, WorkOrder, PROTOCOL_VERSION,
};
use std::io::BufReader;
use std::process::{Command, Stdio};

fn order(tier: Tier) -> WorkOrder {
    WorkOrder {
        unit_id: "u1".into(),
        work_item: WorkItem { kind: WorkItemKind::RoadmapItem, reference: "sandbox#smoke".into(), fingerprint: None },
        tier,
        task: "smoke".into(),
        repo: Repo {
            url: "https://example.invalid/sandbox.git".into(),
            slug: "example/sandbox".into(),
            base_branch: "main".into(),
        },
        branch: "agent/u1".into(),
        test_cmd: "node --test".into(),
        caps: Caps { usd: 1.0, wall_clock_secs: 60, min_review_rounds: 1 },
        resume: None,
    }
}

fn spawn_fake() -> std::process::Child {
    Command::new(env!("CARGO_BIN_EXE_harness-fake"))
        .env("HARNESS_FAKE_STEP_MS", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("harness-fake builds and starts")
}

#[test]
fn conformant_fake_runs_a_t1_unit_to_pr_open() {
    let mut child = spawn_fake();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    let init = RpcMessage::request(1, method::INITIALIZE, &InitializeParams { protocol_version: PROTOCOL_VERSION.into() });
    write_message(&mut stdin, &init).unwrap();
    let reply: InitializeResult = read_message(&mut stdout).unwrap().result_as().unwrap();
    assert_eq!(reply.harness.name, "harness-fake");

    write_message(&mut stdin, &RpcMessage::request(2, method::UNIT_START, &order(Tier::T1))).unwrap();
    assert_eq!(read_message(&mut stdout).unwrap().kind(), MessageKind::Response { id: 2 });

    let mut saw_metric = false;
    let result = loop {
        let msg = read_message(&mut stdout).expect("harness keeps talking until its result");
        match msg.method.as_deref() {
            Some(method::UNIT_EVENT) => {
                if matches!(msg.params_as::<UnitEvent>().unwrap(), UnitEvent::Metric { .. }) {
                    saw_metric = true;
                }
            }
            Some(method::UNIT_RESULT) => break msg.params_as::<UnitResult>().unwrap(),
            other => panic!("unexpected message: {other:?}"),
        }
    };
    assert!(saw_metric, "metering: usd was declared, so a metric must arrive");
    assert_eq!(result.outcome, Outcome::PrOpen);
    assert_eq!(result.evidence.expect("pr_open carries evidence").branch, "agent/u1");

    drop(stdin);
    assert!(matches!(read_message(&mut stdout), Err(ReadError::Eof)));
    assert!(child.wait().unwrap().success());
}

#[test]
fn fake_refuses_a_foreign_major_version() {
    let mut child = spawn_fake();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    let init = RpcMessage::request(1, method::INITIALIZE, &InitializeParams { protocol_version: "99.0".into() });
    write_message(&mut stdin, &init).unwrap();
    let reply = read_message(&mut stdout).unwrap();
    match reply.kind() {
        MessageKind::ErrorResponse { id, error } => {
            assert_eq!(id, 1);
            assert_eq!(error.code, error_code::PROTOCOL_VERSION_UNSUPPORTED);
        }
        other => panic!("expected an error response, got {other:?}"),
    }
    assert!(child.wait().unwrap().success());
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p harness-conformance --test fake_smoke`
Expected: FAIL — `couldn't read src/bin/harness-fake.rs` (the binary does not exist yet).

- [ ] **Step 3: Implement `harness-fake`**

`crates/harness-conformance/src/bin/harness-fake.rs`:

```rust
//! `harness-fake` — a scripted harness. It passes the conformance kit in `conformant` mode, and
//! `HARNESS_FAKE_MODE` makes it break the protocol in one specific way so the kit's detection can
//! be proven. `HARNESS_FAKE_STEP_MS` paces events so a controller can interrupt mid-run.

use harness_protocol::{
    error_code, method, read_message, version_compatible, write_message, ArtifactKind, Capabilities,
    Delivery, DeliveryEvidence, Empty, ErrorScope, Evidence, Failure, GateKind, GateReply,
    GateRequest, HarnessInfo, InitializeParams, InitializeResult, Isolation, MessageKind, Metering,
    Observation, Outcome, RpcMessage, TestRun, UnitEvent, UnitResult, WorkOrder, PROTOCOL_VERSION,
};
use std::io::{self, BufReader, Write};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

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
}

fn mode() -> Mode {
    match std::env::var("HARNESS_FAKE_MODE").as_deref() {
        Ok("crash") => Mode::Crash,
        Ok("malformed") => Mode::Malformed,
        Ok("event_after_result") => Mode::EventAfterResult,
        Ok("silent_metering") => Mode::SilentMetering,
        Ok("hang") => Mode::Hang,
        Ok("unknown_method") => Mode::UnknownMethod,
        Ok("accept_any_version") => Mode::AcceptAnyVersion,
        _ => Mode::Conformant,
    }
}

fn pace() -> Duration {
    let ms = std::env::var("HARNESS_FAKE_STEP_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(50);
    Duration::from_millis(ms)
}

const GATE_REQUEST_ID: u64 = 1_000;
const FAKE_HEAD_SHA: &str = "0123456789abcdef0123456789abcdef01234567";

fn capabilities() -> Capabilities {
    Capabilities {
        isolation: Isolation::None,
        metering: Metering::Usd,
        gates: vec![GateKind::Oracle],
        delivery: Delivery::Bundle,
        resume: false,
        halt: true,
    }
}

fn send(msg: &RpcMessage) {
    let mut out = io::stdout().lock();
    if write_message(&mut out, msg).is_err() {
        // The controller went away; nothing left to do.
        std::process::exit(0);
    }
}

fn event(ev: UnitEvent) {
    send(&RpcMessage::notification(method::UNIT_EVENT, &ev));
}

fn observe(observation: Observation) {
    event(UnitEvent::Observed { observation });
}

fn spawn_reader() -> Receiver<RpcMessage> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut input = BufReader::new(io::stdin());
        while let Ok(msg) = read_message(&mut input) {
            if tx.send(msg).is_err() {
                break;
            }
        }
    });
    rx
}

enum Flow {
    Continue,
    Stop,
}

/// Answer a control request. `Some(Flow::Stop)` means the unit must end now, without a result.
fn handle_control(msg: &RpcMessage) -> Option<Flow> {
    match msg.kind() {
        MessageKind::Request { id, method: m } if m == method::UNIT_HALT || m == method::UNIT_ABANDON => {
            send(&RpcMessage::response(id, &Empty {}));
            Some(Flow::Stop)
        }
        MessageKind::Request { id, method: m } if m == method::UNIT_RESUME => {
            send(&RpcMessage::response(id, &Empty {}));
            None
        }
        MessageKind::Request { id, .. } => {
            send(&RpcMessage::error(id, error_code::METHOD_NOT_FOUND, "unsupported in harness-fake"));
            None
        }
        _ => None,
    }
}

/// Wait `pace`, answering control requests as they arrive.
fn pause(rx: &Receiver<RpcMessage>, pace: Duration) -> Flow {
    let deadline = Instant::now() + pace;
    loop {
        let now = Instant::now();
        if now >= deadline {
            return Flow::Continue;
        }
        match rx.recv_timeout(deadline - now) {
            Ok(msg) => {
                if let Some(Flow::Stop) = handle_control(&msg) {
                    return Flow::Stop;
                }
            }
            Err(RecvTimeoutError::Timeout) => return Flow::Continue,
            Err(RecvTimeoutError::Disconnected) => return Flow::Stop,
        }
    }
}

/// Block on the gate reply. `None` means the unit must stop (interrupted or controller gone).
fn await_gate(rx: &Receiver<RpcMessage>) -> Option<bool> {
    loop {
        let msg = rx.recv().ok()?;
        match msg.kind() {
            MessageKind::Response { id } if id == GATE_REQUEST_ID => {
                return Some(msg.result_as::<GateReply>().map(|r| r.approved).unwrap_or(false));
            }
            MessageKind::ErrorResponse { id, .. } if id == GATE_REQUEST_ID => return Some(false),
            _ => {
                if let Some(Flow::Stop) = handle_control(&msg) {
                    return None;
                }
            }
        }
    }
}

macro_rules! step {
    ($rx:expr, $pace:expr) => {
        if let Flow::Stop = pause($rx, $pace) {
            return;
        }
    };
}

fn run_unit(mode: Mode, order: &WorkOrder, rx: &Receiver<RpcMessage>) {
    let pace = pace();

    observe(Observation::Provisioned);
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
        _ => {}
    }

    if order.tier.requires_oracle() {
        observe(Observation::OracleFrozen);
        let gate = GateRequest::Oracle {
            test_files: vec!["tests/fake.test.js".into()],
            hash: "fake-oracle-hash".into(),
            summary: "one scripted test".into(),
        };
        send(&RpcMessage::request(GATE_REQUEST_ID, method::GATE_REQUEST, &gate));
        match await_gate(rx) {
            None => return,
            Some(false) => {
                let result = UnitResult {
                    outcome: Outcome::Failed,
                    evidence: None,
                    failure: Some(Failure { scope: ErrorScope::Agent, detail: "oracle rejected".into() }),
                };
                send(&RpcMessage::notification(method::UNIT_RESULT, &result));
                return;
            }
            Some(true) => {}
        }
    }

    if mode != Mode::SilentMetering {
        event(UnitEvent::Metric {
            tokens_in: 1_200,
            tokens_out: 300,
            cost_usd: 0.02,
            elapsed_ms: pace.as_millis() as u64,
        });
    }
    observe(Observation::BuildFinished);
    step!(rx, pace);
    observe(Observation::ChecksPassed);
    step!(rx, pace);
    observe(Observation::ReviewFinished {
        round: order.caps.min_review_rounds.max(1),
        unresolved_blockers: 0,
        checks_green: true,
    });
    step!(rx, pace);
    event(UnitEvent::Artifact { kind: ArtifactKind::Branch, reference: order.branch.clone() });

    let result = UnitResult {
        outcome: Outcome::PrOpen,
        evidence: Some(Evidence {
            branch: order.branch.clone(),
            head_sha: FAKE_HEAD_SHA.into(),
            delivery: DeliveryEvidence::Bundle { bundle_path: format!("{}.bundle", order.unit_id) },
            pr: None,
            test: TestRun { command: order.test_cmd.clone(), exit_code: 0 },
            oracle_hash: order.tier.requires_oracle().then(|| "fake-oracle-hash".to_string()),
        }),
        failure: None,
    };
    send(&RpcMessage::notification(method::UNIT_RESULT, &result));

    if mode == Mode::EventAfterResult {
        event(UnitEvent::Log { stream: harness_protocol::LogStream::System, line: "after result".into() });
    }
}

fn main() {
    let mode = mode();
    let rx = spawn_reader();

    let Ok(init) = rx.recv() else { return };
    let MessageKind::Request { id, method: m } = init.kind() else { return };
    if m != method::INITIALIZE {
        send(&RpcMessage::error(id, error_code::METHOD_NOT_FOUND, "expected initialize"));
        return;
    }
    let params: InitializeParams = match init.params_as() {
        Ok(p) => p,
        Err(e) => {
            send(&RpcMessage::error(id, error_code::INVALID_PARAMS, e.to_string()));
            return;
        }
    };
    if mode != Mode::AcceptAnyVersion && !version_compatible(&params.protocol_version) {
        let message = format!("harness-fake speaks protocol {PROTOCOL_VERSION}");
        send(&RpcMessage::error(id, error_code::PROTOCOL_VERSION_UNSUPPORTED, message));
        return;
    }
    send(&RpcMessage::response(
        id,
        &InitializeResult {
            protocol_version: PROTOCOL_VERSION.into(),
            harness: HarnessInfo { name: "harness-fake".into(), version: env!("CARGO_PKG_VERSION").into() },
            capabilities: capabilities(),
        },
    ));

    let Ok(start) = rx.recv() else { return };
    let MessageKind::Request { id, method: m } = start.kind() else { return };
    if m != method::UNIT_START {
        send(&RpcMessage::error(id, error_code::METHOD_NOT_FOUND, "expected unit/start"));
        return;
    }
    let order: WorkOrder = match start.params_as() {
        Ok(o) => o,
        Err(e) => {
            send(&RpcMessage::error(id, error_code::INVALID_PARAMS, e.to_string()));
            return;
        }
    };
    send(&RpcMessage::response(id, &Empty {}));
    run_unit(mode, &order, &rx);
}
```

- [ ] **Step 4: Run the smoke tests to verify they pass**

Run: `cargo test -p harness-conformance --test fake_smoke`
Expected: PASS — 2 tests.

- [ ] **Step 5: Lint**

Run: `cargo fmt --all && cargo clippy -p harness-conformance --all-targets -- -D warnings`
Expected: exit 0. (`Mode::Hang`'s `loop` is intentional. If clippy flags `empty_loop`, that's a false read: the loop body sleeps.)

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock crates/harness-conformance
git commit -m "feat(harness-conformance): harness-fake, a scripted harness with misbehaviour modes"
```

---

### Task 5: The kit — `Session`, the unit driver, and the first two cases

**Files:**
- Create: `crates/harness-conformance/src/lib.rs`
- Create: `crates/harness-conformance/src/session.rs`
- Create: `crates/harness-conformance/src/fixtures.rs`
- Create: `crates/harness-conformance/src/cases.rs`
- Test: `crates/harness-conformance/tests/fake_conforms.rs`

**Interfaces:**
- Consumes: `harness-fake` (Task 4) and the `harness-protocol` API.
- Produces:
  - `pub struct KitConfig { command: Vec<String>, env: Vec<(String, String)>, wall_clock: Duration, grace: Duration }` and `KitConfig::new(command) -> Self` (defaults: wall_clock 30 s, grace 5 s)
  - `pub enum Violation { SpawnFailed(String), NoInitializeResponse, InitializeRejected(String), VersionMismatchAccepted, ExitedWithoutResult, Malformed { line: String }, InvalidMessage, UnknownMethod { method: String }, UnexpectedGateRequest, MessageAfterResult { method: String }, DidNotExitAfterResult, MeteringDeclaredButSilent, WallClockExceeded, InvalidResult(String), GateRejectionIgnored, InterruptNotHonored { method: String } }`
  - `pub enum CaseOutcome { Pass, Skipped(String), Fail(Violation) }`
  - `pub struct CaseReport { name: &'static str, outcome: CaseOutcome }` with `fn passed(&self) -> bool` (true unless `Fail`)
  - `pub fn run_all(cfg: &KitConfig) -> Vec<CaseReport>`, running `version_mismatch_refused` and `happy_path_t1` in this task
  - `pub fn work_order(tier: Tier) -> WorkOrder`
  - `pub struct Session` with `spawn(&KitConfig) -> Result<Self, String>`, `send(&mut self, &RpcMessage) -> bool`, `close_stdin(&mut self)`, `recv(&self, Duration) -> Recv`, `wait_exit(&mut self, Duration) -> bool`, `kill(&mut self)`; it kills the child on `Drop`
  - `pub enum Recv { Message(RpcMessage), Malformed(String), Eof, Timeout }`
- **The kit checks protocol behaviour, not whether the work succeeded.** Its synthetic work order points at `example.invalid`, so a real harness may legitimately end `failed`. Any well-formed result passes: `pr_open` must carry `evidence`, and `failed` must carry `failure`.

- [ ] **Step 1: Write the failing test**

`crates/harness-conformance/tests/fake_conforms.rs`:

```rust
use harness_conformance::{run_all, CaseOutcome, KitConfig};
use std::time::Duration;

fn fake_config(mode: &str) -> KitConfig {
    let mut cfg = KitConfig::new(vec![env!("CARGO_BIN_EXE_harness-fake").to_string()]);
    cfg.env = vec![
        ("HARNESS_FAKE_MODE".into(), mode.into()),
        ("HARNESS_FAKE_STEP_MS".into(), "20".into()),
    ];
    cfg.wall_clock = Duration::from_secs(10);
    cfg.grace = Duration::from_secs(2);
    cfg
}

#[test]
fn conformant_fake_passes_every_case() {
    let reports = run_all(&fake_config("conformant"));
    assert!(!reports.is_empty());
    for report in &reports {
        assert_eq!(report.outcome, CaseOutcome::Pass, "case `{}` did not pass", report.name);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p harness-conformance --test fake_conforms`
Expected: FAIL to compile — `unresolved import harness_conformance::run_all` (no library target yet).

- [ ] **Step 3: Implement `lib.rs`**

```rust
//! Conformance kit for `harness-protocol`. It spawns a harness as a child process and checks, from
//! outside, that it speaks the protocol correctly. It never inspects a harness's internals.

mod cases;
mod fixtures;
mod session;

pub use cases::run_all;
pub use fixtures::work_order;
pub use session::{Recv, Session};

use std::time::Duration;

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
}

impl KitConfig {
    pub fn new(command: Vec<String>) -> Self {
        Self { command, env: Vec::new(), wall_clock: Duration::from_secs(30), grace: Duration::from_secs(5) }
    }
}

/// One way a harness broke the protocol. Every variant is observable from outside the process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Violation {
    SpawnFailed(String),
    NoInitializeResponse,
    InitializeRejected(String),
    VersionMismatchAccepted,
    ExitedWithoutResult,
    Malformed { line: String },
    InvalidMessage,
    UnknownMethod { method: String },
    UnexpectedGateRequest,
    MessageAfterResult { method: String },
    DidNotExitAfterResult,
    MeteringDeclaredButSilent,
    WallClockExceeded,
    InvalidResult(String),
    GateRejectionIgnored,
    InterruptNotHonored { method: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaseOutcome {
    Pass,
    /// The harness declared it lacks the capability this case needs.
    Skipped(String),
    Fail(Violation),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseReport {
    pub name: &'static str,
    pub outcome: CaseOutcome,
}

impl CaseReport {
    pub fn passed(&self) -> bool {
        !matches!(self.outcome, CaseOutcome::Fail(_))
    }
}
```

- [ ] **Step 4: Implement `session.rs`**

```rust
//! A harness child process: write messages to its stdin, read its stdout on a background thread.

use crate::KitConfig;
use harness_protocol::{read_message, write_message, ReadError, RpcMessage};
use std::io::BufReader;
use std::process::{Child, ChildStdin, Command, Stdio};
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
}

impl Session {
    pub fn spawn(cfg: &KitConfig) -> Result<Self, String> {
        let (program, args) = cfg.command.split_first().ok_or("empty harness command")?;
        let mut child = Command::new(program)
            .args(args)
            .envs(cfg.env.clone())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
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
                    Err(ReadError::Eof) | Err(ReadError::Io(_)) => break,
                };
                if tx.send(item).is_err() {
                    break;
                }
            }
        });
        Ok(Self { child, stdin, rx })
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
            if let Ok(Some(_)) = self.child.try_wait() {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
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

- [ ] **Step 5: Implement `fixtures.rs`**

```rust
//! The synthetic work order the kit sends. It names no real repository.

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
        caps: Caps { usd: 0.10, wall_clock_secs: 30, min_review_rounds: 1 },
        resume: None,
    }
}
```

- [ ] **Step 6: Implement `cases.rs`**

```rust
//! The conformance cases. Each spawns a fresh harness.

use crate::fixtures::work_order;
use crate::session::{Recv, Session};
use crate::{CaseOutcome, CaseReport, KitConfig, Violation};
use harness_protocol::{
    error_code, method, Capabilities, Empty, GateReply, InitializeParams, InitializeResult, MessageKind,
    Metering, Outcome, RpcMessage, Tier, UnitEvent, UnitResult, WorkOrder, PROTOCOL_VERSION,
};
use std::time::Instant;

const INIT_ID: u64 = 1;
const START_ID: u64 = 2;
const INTERRUPT_ID: u64 = 3;

/// What the kit does when a `gate/request` arrives.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum GatePolicy {
    Unexpected,
    Approve,
    Reject,
}

/// What one unit run produced.
#[derive(Default)]
pub(crate) struct Transcript {
    pub metrics: usize,
    pub gate_requests: usize,
    /// `None` when the run ended by an honoured interrupt.
    pub result: Option<UnitResult>,
}

pub(crate) fn pass(name: &'static str) -> CaseReport {
    CaseReport { name, outcome: CaseOutcome::Pass }
}

pub(crate) fn fail(name: &'static str, violation: Violation) -> CaseReport {
    CaseReport { name, outcome: CaseOutcome::Fail(violation) }
}

/// Spawn, initialize at our version, and return the declared capabilities.
pub(crate) fn start_session(cfg: &KitConfig) -> Result<(Session, Capabilities), Violation> {
    let mut session = Session::spawn(cfg).map_err(Violation::SpawnFailed)?;
    session.send(&RpcMessage::request(
        INIT_ID,
        method::INITIALIZE,
        &InitializeParams { protocol_version: PROTOCOL_VERSION.into() },
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
            Ok((session, result.capabilities))
        }
        MessageKind::ErrorResponse { id: INIT_ID, error } => Err(Violation::InitializeRejected(error.message.clone())),
        _ => Err(Violation::NoInitializeResponse),
    }
}

fn validate_result(result: &UnitResult) -> Result<(), Violation> {
    match result.outcome {
        Outcome::PrOpen if result.evidence.is_none() => Err(Violation::InvalidResult("pr_open without evidence".into())),
        Outcome::Failed if result.failure.is_none() => Err(Violation::InvalidResult("failed without failure".into())),
        _ => Ok(()),
    }
}

/// After `unit/result` nothing more may arrive, and the process must exit within grace.
fn finish(session: &mut Session, cfg: &KitConfig, transcript: Transcript) -> Result<Transcript, Violation> {
    session.close_stdin();
    match session.recv(cfg.grace) {
        Recv::Message(msg) => {
            let name = msg.method.clone().unwrap_or_else(|| "<response>".into());
            return Err(Violation::MessageAfterResult { method: name });
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

/// Start `order` and read until the unit ends. `gate` answers any `gate/request`. If `interrupt`
/// is a method name, it is sent as a request after the first `unit/event`, and the harness must
/// then acknowledge it and exit within grace without sending `unit/result`.
pub(crate) fn drive(
    session: &mut Session,
    cfg: &KitConfig,
    caps: &Capabilities,
    order: &WorkOrder,
    gate: GatePolicy,
    interrupt: Option<&'static str>,
) -> Result<Transcript, Violation> {
    let deadline = Instant::now() + cfg.wall_clock;
    session.send(&RpcMessage::request(START_ID, method::UNIT_START, order));
    let mut transcript = Transcript::default();
    let mut interrupt_sent: Option<&'static str> = None;

    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            session.kill();
            return Err(Violation::WallClockExceeded);
        }
        let msg = match session.recv(remaining) {
            Recv::Message(msg) => msg,
            Recv::Malformed(line) => return Err(Violation::Malformed { line }),
            Recv::Timeout => {
                session.kill();
                return Err(Violation::WallClockExceeded);
            }
            Recv::Eof => {
                return match interrupt_sent {
                    Some(ctl) if session.wait_exit(cfg.grace) => {
                        let _ = ctl;
                        Ok(transcript)
                    }
                    Some(ctl) => {
                        session.kill();
                        Err(Violation::InterruptNotHonored { method: ctl.to_string() })
                    }
                    None => Err(Violation::ExitedWithoutResult),
                };
            }
        };

        match msg.kind() {
            MessageKind::Response { id: START_ID } => {}
            MessageKind::Response { id: INTERRUPT_ID } if interrupt_sent.is_some() => {}
            MessageKind::ErrorResponse { id: INTERRUPT_ID, .. } => {
                let ctl = interrupt_sent.unwrap_or("interrupt");
                return Err(Violation::InterruptNotHonored { method: ctl.to_string() });
            }
            MessageKind::Notification { method: m } if m == method::UNIT_EVENT => {
                let ev: UnitEvent = msg
                    .params_as()
                    .map_err(|e| Violation::InvalidResult(format!("unit/event params: {e}")))?;
                if matches!(ev, UnitEvent::Metric { .. }) {
                    transcript.metrics += 1;
                }
                if let (Some(ctl), None) = (interrupt, interrupt_sent) {
                    session.send(&RpcMessage::request(INTERRUPT_ID, ctl, &Empty {}));
                    interrupt_sent = Some(ctl);
                }
            }
            MessageKind::Notification { method: m } if m == method::UNIT_RESULT => {
                if let Some(ctl) = interrupt_sent {
                    return Err(Violation::InterruptNotHonored { method: ctl.to_string() });
                }
                let result: UnitResult = msg
                    .params_as()
                    .map_err(|e| Violation::InvalidResult(format!("unit/result params: {e}")))?;
                validate_result(&result)?;
                if caps.metering == Metering::Usd && transcript.metrics == 0 && result.outcome != Outcome::Failed {
                    return Err(Violation::MeteringDeclaredButSilent);
                }
                transcript.result = Some(result);
                return finish(session, cfg, transcript);
            }
            MessageKind::Request { id, method: m } if m == method::GATE_REQUEST => {
                transcript.gate_requests += 1;
                let approved = match gate {
                    GatePolicy::Unexpected => return Err(Violation::UnexpectedGateRequest),
                    GatePolicy::Approve => true,
                    GatePolicy::Reject => false,
                };
                session.send(&RpcMessage::response(id, &GateReply { approved, edited_test_files: None }));
            }
            MessageKind::Notification { method: m } | MessageKind::Request { method: m, .. } => {
                return Err(Violation::UnknownMethod { method: m.to_string() });
            }
            _ => return Err(Violation::InvalidMessage),
        }
    }
}

/// A harness must refuse `initialize` from a different protocol major with `-32001`.
pub(crate) fn version_mismatch_refused(cfg: &KitConfig) -> CaseReport {
    const NAME: &str = "version_mismatch_refused";
    let mut session = match Session::spawn(cfg) {
        Ok(s) => s,
        Err(e) => return fail(NAME, Violation::SpawnFailed(e)),
    };
    session.send(&RpcMessage::request(INIT_ID, method::INITIALIZE, &InitializeParams { protocol_version: "99.0".into() }));
    match session.recv(cfg.grace) {
        Recv::Message(msg) => match msg.kind() {
            MessageKind::ErrorResponse { id: INIT_ID, error } if error.code == error_code::PROTOCOL_VERSION_UNSUPPORTED => pass(NAME),
            MessageKind::Response { id: INIT_ID } => fail(NAME, Violation::VersionMismatchAccepted),
            _ => fail(NAME, Violation::InvalidMessage),
        },
        Recv::Malformed(line) => fail(NAME, Violation::Malformed { line }),
        Recv::Eof | Recv::Timeout => fail(NAME, Violation::NoInitializeResponse),
    }
}

/// A T1 unit runs to a well-formed result with no gate request, then the harness exits.
pub(crate) fn happy_path_t1(cfg: &KitConfig) -> CaseReport {
    const NAME: &str = "happy_path_t1";
    let (mut session, caps) = match start_session(cfg) {
        Ok(started) => started,
        Err(v) => return fail(NAME, v),
    };
    match drive(&mut session, cfg, &caps, &work_order(Tier::T1), GatePolicy::Unexpected, None) {
        Ok(t) if t.result.is_some() => pass(NAME),
        Ok(_) => fail(NAME, Violation::ExitedWithoutResult),
        Err(v) => fail(NAME, v),
    }
}

pub fn run_all(cfg: &KitConfig) -> Vec<CaseReport> {
    vec![version_mismatch_refused(cfg), happy_path_t1(cfg)]
}
```

- [ ] **Step 7: Run the tests to verify they pass**

Run: `cargo test -p harness-conformance`
Expected: PASS — `fake_smoke` (2) and `fake_conforms` (1). If clippy later flags `Transcript::gate_requests` as unread, leave it: Task 6 reads it.

- [ ] **Step 8: Lint**

Run: `cargo fmt --all && cargo clippy -p harness-conformance --all-targets -- -D warnings`
Expected: exit 0. If `dead_code` fires on `GatePolicy::Approve`/`Reject` or `Transcript::gate_requests` (used from Task 6), add `#[allow(dead_code)] // used by the gate cases, Task 6` on those items for this commit only. Task 6 removes the allows.

- [ ] **Step 9: Commit**

```bash
git add crates/harness-conformance
git commit -m "feat(harness-conformance): session, unit driver, version and happy-path cases"
```

---

### Task 6: Gate and interrupt cases

**Files:**
- Modify: `crates/harness-conformance/src/lib.rs` (add `Violation::GateNotRequested`)
- Modify: `crates/harness-conformance/src/cases.rs`
- Test: `crates/harness-conformance/tests/fake_conforms.rs`

**Interfaces:**
- Consumes: `start_session`, `drive`, `GatePolicy`, `Transcript`, `pass`, `fail` (Task 5).
- Produces: cases `gate_approved_t2`, `gate_rejected_t2`, `halt`, `abandon`. `run_all` returns exactly these six, in this order: `version_mismatch_refused, happy_path_t1, gate_approved_t2, gate_rejected_t2, halt, abandon`. Adds `Violation::GateNotRequested`.
- **The rules, from spec §3 and `Tier::requires_oracle`:**
  - At T2, a harness that declares `gates: [oracle]` must send `gate/request` before any `pr_open`.
  - Ending `failed` or `no_change` before reaching the gate is allowed. The approved case still passes, and the rejected case is `Skipped`, because the rejection path went unexercised.
  - After a rejection, `pr_open` is `GateRejectionIgnored`.
  - `halt` applies only when `capabilities.halt` is true, and is `Skipped` otherwise. `abandon` applies to every harness.
  - An interrupted harness answers the request and exits within grace **without** `unit/result`.

- [ ] **Step 1: Write the failing test**

Append to `crates/harness-conformance/tests/fake_conforms.rs`:

```rust
#[test]
fn run_all_covers_every_case_in_order() {
    let names: Vec<&str> = run_all(&fake_config("conformant")).iter().map(|r| r.name).collect();
    assert_eq!(
        names,
        ["version_mismatch_refused", "happy_path_t1", "gate_approved_t2", "gate_rejected_t2", "halt", "abandon"]
    );
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p harness-conformance --test fake_conforms run_all_covers_every_case_in_order`
Expected: FAIL — `left: ["version_mismatch_refused", "happy_path_t1"]` does not equal the six names.

- [ ] **Step 3: Add the violation**

In `crates/harness-conformance/src/lib.rs`, add this variant to `Violation`, directly after `UnexpectedGateRequest,`:

```rust
    /// A tier that requires an oracle ended `pr_open` without ever sending `gate/request`.
    GateNotRequested,
```

- [ ] **Step 4: Implement the cases**

In `crates/harness-conformance/src/cases.rs`:

1. Remove any `#[allow(dead_code)]` added in Task 5 Step 8.
2. Add `GateKind` to the `harness_protocol` import list.
3. Replace the existing `run_all` function with the code below:

```rust
fn skipped(name: &'static str, why: &str) -> CaseReport {
    CaseReport { name, outcome: CaseOutcome::Skipped(why.into()) }
}

fn has_oracle_gate(caps: &Capabilities) -> bool {
    caps.gates.contains(&GateKind::Oracle)
}

/// T2: the harness asks for oracle approval before building; approved, it ends well-formed.
pub(crate) fn gate_approved_t2(cfg: &KitConfig) -> CaseReport {
    const NAME: &str = "gate_approved_t2";
    let (mut session, caps) = match start_session(cfg) {
        Ok(started) => started,
        Err(v) => return fail(NAME, v),
    };
    if !has_oracle_gate(&caps) {
        return skipped(NAME, "harness declares no oracle gate, so it may only take T1 units");
    }
    match drive(&mut session, cfg, &caps, &work_order(Tier::T2), GatePolicy::Approve, None) {
        Err(v) => fail(NAME, v),
        Ok(t) => match t.result {
            None => fail(NAME, Violation::ExitedWithoutResult),
            Some(ref r) if t.gate_requests == 0 && r.outcome == Outcome::PrOpen => {
                fail(NAME, Violation::GateNotRequested)
            }
            Some(_) => pass(NAME),
        },
    }
}

/// T2: a rejected oracle must never lead to `pr_open`.
pub(crate) fn gate_rejected_t2(cfg: &KitConfig) -> CaseReport {
    const NAME: &str = "gate_rejected_t2";
    let (mut session, caps) = match start_session(cfg) {
        Ok(started) => started,
        Err(v) => return fail(NAME, v),
    };
    if !has_oracle_gate(&caps) {
        return skipped(NAME, "harness declares no oracle gate, so it may only take T1 units");
    }
    match drive(&mut session, cfg, &caps, &work_order(Tier::T2), GatePolicy::Reject, None) {
        Err(v) => fail(NAME, v),
        Ok(t) => match t.result {
            None => fail(NAME, Violation::ExitedWithoutResult),
            Some(ref r) if t.gate_requests == 0 && r.outcome == Outcome::PrOpen => {
                fail(NAME, Violation::GateNotRequested)
            }
            Some(_) if t.gate_requests == 0 => {
                skipped(NAME, "harness ended before reaching the gate; the rejection path was not exercised")
            }
            Some(ref r) if r.outcome == Outcome::PrOpen => fail(NAME, Violation::GateRejectionIgnored),
            Some(_) => pass(NAME),
        },
    }
}

/// After the first `unit/event`, send `ctl`; the harness must answer it and exit without a result.
fn interrupt_case(cfg: &KitConfig, name: &'static str, ctl: &'static str, needs_halt: bool) -> CaseReport {
    let (mut session, caps) = match start_session(cfg) {
        Ok(started) => started,
        Err(v) => return fail(name, v),
    };
    if needs_halt && !caps.halt {
        return skipped(name, "harness declares halt: false");
    }
    match drive(&mut session, cfg, &caps, &work_order(Tier::T1), GatePolicy::Unexpected, Some(ctl)) {
        Ok(t) if t.result.is_none() => pass(name),
        Ok(_) => fail(name, Violation::InterruptNotHonored { method: ctl.to_string() }),
        Err(v) => fail(name, v),
    }
}

pub(crate) fn halt(cfg: &KitConfig) -> CaseReport {
    interrupt_case(cfg, "halt", method::UNIT_HALT, true)
}

pub(crate) fn abandon(cfg: &KitConfig) -> CaseReport {
    interrupt_case(cfg, "abandon", method::UNIT_ABANDON, false)
}

pub fn run_all(cfg: &KitConfig) -> Vec<CaseReport> {
    vec![
        version_mismatch_refused(cfg),
        happy_path_t1(cfg),
        gate_approved_t2(cfg),
        gate_rejected_t2(cfg),
        halt(cfg),
        abandon(cfg),
    ]
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p harness-conformance`
Expected: PASS — `fake_smoke` (2), `fake_conforms` (2). `conformant_fake_passes_every_case` now exercises all six cases, and every one is `Pass`. (`harness-fake` declares `gates: [oracle]` and `halt: true`, so nothing is skipped.)

- [ ] **Step 6: Lint**

Run: `cargo fmt --all && cargo clippy -p harness-conformance --all-targets -- -D warnings`
Expected: exit 0, with no `allow(dead_code)` left in `cases.rs`.

- [ ] **Step 7: Commit**

```bash
git add crates/harness-conformance
git commit -m "feat(harness-conformance): oracle gate approve/reject, halt and abandon cases"
```

---

### Task 7: Prove the kit catches every violation

**Files:**
- Modify: `crates/harness-conformance/src/bin/harness-fake.rs` (three more modes)
- Test: `crates/harness-conformance/tests/detects_violations.rs`

**Interfaces:**
- Consumes: `run_all`, `KitConfig`, `CaseOutcome`, `CaseReport`, `Violation` (Tasks 5–6); `harness-fake` modes (Task 4).
- Produces: three new `HARNESS_FAKE_MODE` values:
  - `skip_gate` never sends `gate/request`, even at T2.
  - `ignore_gate_rejection` continues to `pr_open` after a rejected gate.
  - `ignore_halt` answers `unit/halt` with error `-32601` and keeps running.
- **Expected detection, per mode:**

| `HARNESS_FAKE_MODE` | Case | Expected outcome |
|---|---|---|
| `crash` | `happy_path_t1` | `Fail(ExitedWithoutResult)` |
| `malformed` | `happy_path_t1` | `Fail(Malformed { line: "this is not json" })` |
| `event_after_result` | `happy_path_t1` | `Fail(MessageAfterResult { method: "unit/event" })` |
| `silent_metering` | `happy_path_t1` | `Fail(MeteringDeclaredButSilent)` |
| `hang` | `happy_path_t1` | `Fail(WallClockExceeded)` |
| `unknown_method` | `happy_path_t1` | `Fail(UnknownMethod { method: "unit/whatever" })` |
| `accept_any_version` | `version_mismatch_refused` | `Fail(VersionMismatchAccepted)` |
| `skip_gate` | `gate_approved_t2` | `Fail(GateNotRequested)` |
| `ignore_gate_rejection` | `gate_rejected_t2` | `Fail(GateRejectionIgnored)` |
| `ignore_halt` | `halt` | `Fail(InterruptNotHonored { method: "unit/halt" })` |

- [ ] **Step 1: Write the tests**

`crates/harness-conformance/tests/detects_violations.rs`:

```rust
//! Each misbehaving `harness-fake` mode must be caught by exactly the case that owns that rule.

use harness_conformance::{run_all, CaseOutcome, CaseReport, KitConfig, Violation};
use std::time::Duration;

fn fake_config(mode: &str) -> KitConfig {
    let mut cfg = KitConfig::new(vec![env!("CARGO_BIN_EXE_harness-fake").to_string()]);
    cfg.env = vec![
        ("HARNESS_FAKE_MODE".into(), mode.into()),
        ("HARNESS_FAKE_STEP_MS".into(), "20".into()),
    ];
    // Short, because `hang` makes every driving case wait out the whole wall clock.
    cfg.wall_clock = Duration::from_secs(2);
    cfg.grace = Duration::from_secs(1);
    cfg
}

fn outcome_of<'a>(reports: &'a [CaseReport], case: &str) -> &'a CaseOutcome {
    &reports.iter().find(|r| r.name == case).expect("case exists in run_all").outcome
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
fn crash_is_exited_without_result() {
    assert_detects("crash", "happy_path_t1", Violation::ExitedWithoutResult);
}

#[test]
fn malformed_line_is_reported_verbatim() {
    assert_detects("malformed", "happy_path_t1", Violation::Malformed { line: "this is not json".into() });
}

#[test]
fn a_message_after_the_result_is_caught() {
    assert_detects("event_after_result", "happy_path_t1", Violation::MessageAfterResult { method: "unit/event".into() });
}

#[test]
fn declared_metering_that_never_reports_is_caught() {
    assert_detects("silent_metering", "happy_path_t1", Violation::MeteringDeclaredButSilent);
}

#[test]
fn a_hung_harness_is_killed_at_the_wall_clock() {
    assert_detects("hang", "happy_path_t1", Violation::WallClockExceeded);
}

#[test]
fn an_unknown_method_is_caught() {
    assert_detects("unknown_method", "happy_path_t1", Violation::UnknownMethod { method: "unit/whatever".into() });
}

#[test]
fn accepting_a_foreign_major_is_caught() {
    assert_detects("accept_any_version", "version_mismatch_refused", Violation::VersionMismatchAccepted);
}

#[test]
fn skipping_the_oracle_gate_is_caught() {
    assert_detects("skip_gate", "gate_approved_t2", Violation::GateNotRequested);
}

#[test]
fn ignoring_a_rejected_gate_is_caught() {
    assert_detects("ignore_gate_rejection", "gate_rejected_t2", Violation::GateRejectionIgnored);
}

#[test]
fn ignoring_halt_is_caught() {
    assert_detects("ignore_halt", "halt", Violation::InterruptNotHonored { method: "unit/halt".into() });
}
```

- [ ] **Step 2: Run the tests to see the three new modes fail**

Run: `cargo test -p harness-conformance --test detects_violations`
Expected: 7 PASS (the modes from Task 4, caught by the Task 5–6 logic) and 3 FAIL: `skipping_the_oracle_gate_is_caught`, `ignoring_a_rejected_gate_is_caught`, `ignoring_halt_is_caught`. The fake treats those unknown modes as `conformant`, so each case reads `Pass`.

- [ ] **Step 3: Add the three modes to `harness-fake`**

In `crates/harness-conformance/src/bin/harness-fake.rs`:

(a) In `enum Mode`, after `AcceptAnyVersion,` add:

```rust
    SkipGate,
    IgnoreGateRejection,
    IgnoreHalt,
```

(b) In `fn mode()`, after the `Ok("accept_any_version") => Mode::AcceptAnyVersion,` arm add:

```rust
        Ok("skip_gate") => Mode::SkipGate,
        Ok("ignore_gate_rejection") => Mode::IgnoreGateRejection,
        Ok("ignore_halt") => Mode::IgnoreHalt,
```

(c) In `fn handle_control`, insert this as the **first** arm of the `match`:

```rust
        MessageKind::Request { id, method: m } if m == method::UNIT_HALT && mode() == Mode::IgnoreHalt => {
            send(&RpcMessage::error(id, error_code::METHOD_NOT_FOUND, "halt ignored (ignore_halt mode)"));
            None
        }
```

(d) In `fn run_unit`, replace `if order.tier.requires_oracle() {` with:

```rust
    if order.tier.requires_oracle() && mode != Mode::SkipGate {
```

(e) In the same block, replace `Some(false) => {` with `Some(false) if mode != Mode::IgnoreGateRejection => {`, and replace `Some(true) => {}` with `Some(_) => {}`.

- [ ] **Step 4: Run the whole package**

Run: `cargo test -p harness-conformance`
Expected: PASS — `fake_smoke` (2), `fake_conforms` (2), `detects_violations` (10).

- [ ] **Step 5: Lint**

Run: `cargo fmt --all && cargo clippy -p harness-conformance --all-targets -- -D warnings`
Expected: exit 0.

- [ ] **Step 6: Commit**

```bash
git add crates/harness-conformance
git commit -m "test(harness-conformance): prove the kit detects each protocol violation"
```

---

### Task 8: The `harness-conformance` CLI

**Files:**
- Modify: `crates/harness-conformance/Cargo.toml` (second `[[bin]]`)
- Create: `crates/harness-conformance/src/bin/harness-conformance.rs`
- Test: `crates/harness-conformance/tests/cli.rs`

**Interfaces:**
- Consumes: `run_all`, `KitConfig`, `CaseOutcome` (Tasks 5–6).
- Produces: binary `harness-conformance`.
  - **Usage:** `harness-conformance [--wall-clock-secs N] [--grace-secs N] -- <harness command> [args...]`
  - **Output:** one line per case, `PASS  <case>`, `SKIP  <case>  (<why>)` or `FAIL  <case>  <Violation:?>`, followed by `<p> passed, <s> skipped, <f> failed`.
  - **Exit codes:** `0` when nothing failed, `1` when any case failed, `2` on a usage error. The harness inherits the CLI's environment, which is how third parties pass their own settings.

- [ ] **Step 1: Write the failing tests**

`crates/harness-conformance/tests/cli.rs`:

```rust
use std::process::Command;

fn kit() -> Command {
    Command::new(env!("CARGO_BIN_EXE_harness-conformance"))
}

fn run_against_fake(mode: &str) -> (Option<i32>, String) {
    let out = kit()
        .env("HARNESS_FAKE_MODE", mode)
        .env("HARNESS_FAKE_STEP_MS", "20")
        .args(["--wall-clock-secs", "10", "--grace-secs", "2", "--", env!("CARGO_BIN_EXE_harness-fake")])
        .output()
        .expect("harness-conformance runs");
    (out.status.code(), String::from_utf8_lossy(&out.stdout).into_owned())
}

#[test]
fn exits_zero_against_the_conformant_fake() {
    let (code, stdout) = run_against_fake("conformant");
    assert_eq!(code, Some(0), "{stdout}");
    assert!(stdout.contains("6 passed, 0 skipped, 0 failed"), "{stdout}");
}

#[test]
fn exits_one_and_names_the_case_when_the_harness_breaks_the_protocol() {
    let (code, stdout) = run_against_fake("silent_metering");
    assert_eq!(code, Some(1), "{stdout}");
    assert!(stdout.contains("FAIL  happy_path_t1  MeteringDeclaredButSilent"), "{stdout}");
}

#[test]
fn exits_two_on_a_usage_error() {
    let out = kit().output().expect("harness-conformance runs");
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("usage: harness-conformance"));
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p harness-conformance --test cli`
Expected: FAIL to compile — `environment variable CARGO_BIN_EXE_harness-conformance not defined`.

- [ ] **Step 3: Declare the binary and implement it**

Append to `crates/harness-conformance/Cargo.toml`:

```toml
[[bin]]
name = "harness-conformance"
path = "src/bin/harness-conformance.rs"
```

`crates/harness-conformance/src/bin/harness-conformance.rs`:

```rust
//! `harness-conformance [--wall-clock-secs N] [--grace-secs N] -- <harness command> [args...]`
//!
//! Runs every conformance case against a harness and exits 0 only if none failed.

use harness_conformance::{run_all, CaseOutcome, KitConfig};
use std::process::ExitCode;
use std::time::Duration;

const USAGE: &str =
    "usage: harness-conformance [--wall-clock-secs N] [--grace-secs N] -- <harness command> [args...]";

fn parse(args: Vec<String>) -> Result<KitConfig, String> {
    let split = args.iter().position(|a| a == "--").ok_or_else(|| USAGE.to_string())?;
    let (flags, rest) = args.split_at(split);
    let command = rest[1..].to_vec();
    if command.is_empty() {
        return Err(USAGE.to_string());
    }
    let mut cfg = KitConfig::new(command);
    let mut it = flags.iter();
    while let Some(flag) = it.next() {
        let value = it.next().ok_or_else(|| format!("{flag} needs a value\n{USAGE}"))?;
        let secs: u64 = value
            .parse()
            .map_err(|_| format!("{flag}: `{value}` is not a whole number of seconds"))?;
        match flag.as_str() {
            "--wall-clock-secs" => cfg.wall_clock = Duration::from_secs(secs),
            "--grace-secs" => cfg.grace = Duration::from_secs(secs),
            other => return Err(format!("unknown flag `{other}`\n{USAGE}")),
        }
    }
    Ok(cfg)
}

fn main() -> ExitCode {
    let cfg = match parse(std::env::args().skip(1).collect()) {
        Ok(cfg) => cfg,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::from(2);
        }
    };

    let reports = run_all(&cfg);
    let (mut passed, mut skipped, mut failed) = (0, 0, 0);
    for report in &reports {
        match &report.outcome {
            CaseOutcome::Pass => {
                passed += 1;
                println!("PASS  {}", report.name);
            }
            CaseOutcome::Skipped(why) => {
                skipped += 1;
                println!("SKIP  {}  ({why})", report.name);
            }
            CaseOutcome::Fail(violation) => {
                failed += 1;
                println!("FAIL  {}  {violation:?}", report.name);
            }
        }
    }
    println!("{passed} passed, {skipped} skipped, {failed} failed");

    if failed == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p harness-conformance --test cli`
Expected: PASS — 3 tests.

- [ ] **Step 5: Run the kit by hand once**

bash:
```bash
cargo build -p harness-conformance --bins && target/debug/harness-conformance -- target/debug/harness-fake
```
PowerShell:
```powershell
cargo build -p harness-conformance --bins; .\target\debug\harness-conformance.exe -- .\target\debug\harness-fake.exe; "EXIT=$LASTEXITCODE"
```
Expected: six `PASS` lines, then `6 passed, 0 skipped, 0 failed`, and exit 0.

- [ ] **Step 6: Lint**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings`
Expected: exit 0 across the whole workspace.

- [ ] **Step 7: Commit**

```bash
git add crates/harness-conformance
git commit -m "feat(harness-conformance): CLI with PASS/SKIP/FAIL report and exit codes"
```

---

### Task 9: A required CI check, and the README for harness authors

**Files:**
- Modify: `.github/workflows/ci.yml` (new `conformance` job)
- Create: `crates/harness-protocol/README.md`

**Interfaces:**
- Consumes: binaries `harness-conformance` and `harness-fake` (Tasks 4 and 8).
- Produces: a CI status check named exactly **`harness conformance`**, required on `main` (spec §6 item 7, the GAP-133 lesson). The new crates are already covered by the existing required `cargo test (workspace)` job, because they are workspace members.

- [ ] **Step 1: Add the CI job**

In `.github/workflows/ci.yml`, insert this job immediately after the `test:` job, whose last line is `        run: cargo test --workspace` (currently `ci.yml:180`), and before the comment block that introduces `test-cockpit`:

```yaml

  # ---------------------------------------------------------------------------
  # Harness conformance (NEXUS spec 2026-09-14-swappable-harness-design §6.2, §6.7).
  # Runs the conformance kit's CLI against harness-fake. This check must be
  # REQUIRED on main: an advisory check here would repeat GAP-133.
  # ---------------------------------------------------------------------------
  conformance:
    name: harness conformance
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable

      - name: Cache cargo registry + target
        uses: Swatinem/rust-cache@v2

      - name: Build the conformance kit and harness-fake
        run: cargo build -p harness-conformance --bins

      - name: Run the conformance kit against harness-fake
        run: target/debug/harness-conformance --wall-clock-secs 30 --grace-secs 5 -- target/debug/harness-fake
```

- [ ] **Step 2: Write the README**

`crates/harness-protocol/README.md`:

````markdown
# harness-protocol

The wire contract between `fleetd` (the control plane) and a **harness**, the process that runs
one unit of work. Design: NEXUS `docs/specs/2026-09-14-swappable-harness-design.md` §3.

## Transport

JSON-RPC 2.0, **one JSON object per line**, over the harness's stdin and stdout. Stderr is free-form
log. One harness process per unit. Closing stdin means "shut down".

## Lifecycle

| Step | Message | Direction |
|---|---|---|
| 1 | `initialize` `{protocol_version}` → `{protocol_version, harness, capabilities}` | control plane → harness |
| 2 | `unit/start` work order → `{}` | → |
| 3 | `unit/event` observations, metrics, logs, findings, artifacts, errors | ← notification |
| 4 | `gate/request` `{gate: oracle, …}` → `{approved}` (T2/T3) | ← request |
| — | `unit/halt`, `unit/abandon`, `unit/resume` → `{}` | → at any time |
| 5 | `unit/result` `{outcome, evidence?, failure?}`, then exit | ← notification |

Refuse an `initialize` from a different protocol **major** with error code `-32001`.

## Rules the control plane enforces

- A harness **never merges** and **never declares done**. `evidence` is re-read from git, CI and
  the test run before any unit reaches `PrOpen`.
- `capabilities` decide eligibility. T2/T3 units need `gates: [oracle]`. Isolation weaker than
  `container`, or `metering: none`, needs per-unit operator opt-in.
- If you declare `metering: usd`, send at least one `metric` before a non-failed result.
- After `unit/result`, send nothing more and exit.
- After answering `unit/halt` or `unit/abandon`, exit **without** `unit/result`.

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
violation looks like.
````

- [ ] **Step 3: Verify the whole workspace locally**

Run: `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: exit 0. The existing `fleet-core`/`fleetd` counts are unchanged, and the new crates add 14 (`harness-protocol`) + 17 (`harness-conformance`) tests. Docker-gated tests stay ignored, as before.

- [ ] **Step 4: Commit, push, and open the PR**

```bash
git add .github/workflows/ci.yml crates/harness-protocol/README.md
git commit -m "ci: required harness conformance check; README for harness authors"
git push -u origin feat/harness-protocol-sp1
gh pr create --base main --title "SP-1: harness protocol v0, conformance kit, harness-fake" \
  --body "Implements SP-1 of NEXUS docs/specs/2026-09-14-swappable-harness-design.md (plan: docs/superpowers/plans/2026-09-14-harness-protocol-sp1.md)."
```
Expected: CI runs both `cargo test (workspace)` and the new `harness conformance`, and both are green.

- [ ] **Step 5 (operator): Make the check required**

**This changes repository settings, so it needs Alex's approval.** Run it only after `harness conformance` has completed once on the PR, because GitHub only offers check names it has seen:

```bash
echo '["harness conformance"]' | gh api -X POST \
  repos/adbarc92/command-center/branches/main/protection/required_status_checks/contexts --input -
gh api repos/adbarc92/command-center/branches/main/protection/required_status_checks --jq .contexts
```
Expected: `["cargo test (workspace)","harness conformance"]`.

---

### Task 10: Register the contract in NEXUS

**Files (NEXUS repository, `D:\MajorProjects\NEXUS`):**
- Create: `docs/contracts/harness-protocol.contract.json`, vendored **byte-for-byte** from `command-center/crates/harness-protocol/contract/harness-protocol.contract.json`
- Modify: `docs/contracts/README.md` (registry table)

**Interfaces:**
- Consumes: the committed contract file and the `CONTRACT_SHA256` constant (Task 3).
- Produces: a NEXUS registry row. Each **consumer** vendors the same file and pins the same hash in its own test when it is built: `fleetd` in SP-2, `harness-reqdrive` in SP-4. SP-1 has no consumer test to write.

**Prerequisite:** the NEXUS design PR (branch `docs/swappable-harness-design`) is merged, so the registry can cite the spec on `main`.

- [ ] **Step 1: Branch and vendor the file**

```bash
cd D:/MajorProjects/NEXUS
git fetch origin && git checkout -b docs/register-harness-protocol-contract origin/main
cp ../INFRASTRUCTURE/command-center/crates/harness-protocol/contract/harness-protocol.contract.json docs/contracts/
```

- [ ] **Step 2: Verify the vendored hash equals the pinned constant**

```bash
node -e "const c=require('crypto');const f=require('fs').readFileSync('docs/contracts/harness-protocol.contract.json','utf8');console.log(c.createHash('sha256').update(JSON.stringify(JSON.parse(f))).digest('hex'))"
```
Expected: exactly the `CONTRACT_SHA256` value in `command-center/crates/harness-protocol/tests/contract.rs`. If not, stop: the file was not copied byte-for-byte.

- [ ] **Step 3: Add the registry row**

In `docs/contracts/README.md`, append a row to the **Registry** table, putting the verified digest in the last column:

```markdown
| [`harness-protocol.contract.json`](./harness-protocol.contract.json) — harness protocol v0, JSON-RPC over stdio ([spec](../specs/2026-09-14-swappable-harness-design.md) §3) | `command-center` · `crates/harness-protocol` | every harness: `harness-fake` (SP-1), `fleetd` supervisor + `harness-claude-docker` (SP-2), `harness-reqdrive` (SP-4) | `<the CONTRACT_SHA256 value from Step 2>` |
```

- [ ] **Step 4: Commit and open the PR**

```bash
git add docs/contracts/harness-protocol.contract.json docs/contracts/README.md
git commit -m "docs(contracts): register the harness protocol v0 contract"
git push -u origin docs/register-harness-protocol-contract
gh pr create --base main --title "Register the harness protocol v0 contract" \
  --body "Vendors command-center crates/harness-protocol/contract/harness-protocol.contract.json and records its canonical SHA-256 (SP-1 of docs/specs/2026-09-14-swappable-harness-design.md)."
```

---

## Spec coverage

| Spec requirement | Task |
|---|---|
| §3 transport: JSON-RPC 2.0, newline-delimited, stdio | 2 |
| §3 versioning: crate, JSON Schema, hash-pinned, registered, major-version refusal | 1, 2, 3, 10 |
| §3 message table: every method and payload | 1, 2 |
| §3 capabilities and eligibility inputs (`isolation`, `metering`, `gates`, `delivery`, `resume`, `halt`) | 1 (types); enforcement is SP-2 |
| §6.1 serde round-trips; schema snapshot pinned by hash | 1, 2, 3 |
| §6.2 conformance kit: version mismatch, happy path, halt, abandon, gate approve/reject, crash, malformed, event after result, silent metering, wall clock | 5, 6, 7 |
| §6.2 every shipped harness runs the kit in CI | 9 (`harness-fake`; later harnesses join in SP-2/SP-4) |
| §6.7 new crates in the root workspace; the kit as a **required** check | 1, 4, 9 |
| §7 SP-1 row: `harness-protocol` + JSON Schema, conformance kit, `harness-fake` | 1–9 |

**Not in SP-1, by design:** the evidence verifier, supervisor, caps ledger and `GET /harnesses` (§4, §5, §6.3–6.4 → SP-2); Dispatch, auth and write-back (§5, §6.5 → SP-3); generated cockpit types (§6.6 → SP-2); `SMOKE-LAP-02` (§6.8 → finish line).
