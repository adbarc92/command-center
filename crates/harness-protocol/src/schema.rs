//! The protocol's JSON Schema: one document whose `$defs` hold every wire type.

use crate::rpc::{RpcError, RpcMessage};
use crate::types::{
    Empty, GateReply, GateRequest, InitializeParams, InitializeResult, UnitEvent, UnitResult,
    WorkOrder,
};
use schemars::{schema_for, JsonSchema};

/// Exists only to pull every wire type into a single schema document.
/// Field names say which message each type belongs to.
///
/// | Method | Direction | Kind | Params → Result |
/// |---|---|---|---|
/// | `initialize` | control plane → harness | request | `InitializeParams` → `InitializeResult` |
/// | `unit/start` | control plane → harness | request | `WorkOrder` → `Empty` |
/// | `unit/event` | harness → control plane | notification | `UnitEvent` |
/// | `gate/request` | harness → control plane | request | `GateRequest` → `GateReply` |
/// | `unit/halt`, `unit/resume`, `unit/abandon` | control plane → harness | request | `Empty` → `Empty` |
/// | `unit/result` | harness → control plane | notification | `UnitResult` |
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
