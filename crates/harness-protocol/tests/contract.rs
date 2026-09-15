//! The harness protocol is a cross-repository wire contract (NEXUS `docs/contracts/README.md`).
//! Two guarantees: the committed schema equals what the types generate (drift gate), and the
//! committed file's canonical SHA-256 equals the pinned constant (tamper-evidence). To change the
//! protocol on purpose: re-bless, update `CONTRACT_SHA256`, and update the NEXUS registry row.

use harness_protocol::schema_json;
use sha2::{Digest, Sha256};
use std::path::PathBuf;

/// Canonical SHA-256 of `contract/harness-protocol.contract.json`. Set in Task 3 Step 5.
const CONTRACT_SHA256: &str = "51c64132b2d3d898791f064820ffb92fdbd2b0d36f71913bd8d85e48f1df9463";

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
    // A bless run rewrites the file concurrently, so the pin is checked on the next normal run.
    if std::env::var_os("HARNESS_PROTOCOL_BLESS").is_some() {
        return;
    }
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
