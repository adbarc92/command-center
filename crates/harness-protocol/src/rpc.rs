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
        Self {
            jsonrpc: "2.0".into(),
            id: None,
            method: None,
            params: None,
            result: None,
            error: None,
        }
    }

    pub fn request<P: Serialize>(id: u64, method: &str, params: &P) -> Self {
        Self {
            id: Some(id),
            method: Some(method.into()),
            params: Some(to_json(params)),
            ..Self::empty()
        }
    }

    pub fn notification<P: Serialize>(method: &str, params: &P) -> Self {
        Self {
            method: Some(method.into()),
            params: Some(to_json(params)),
            ..Self::empty()
        }
    }

    pub fn response<R: Serialize>(id: u64, result: &R) -> Self {
        Self {
            id: Some(id),
            result: Some(to_json(result)),
            ..Self::empty()
        }
    }

    pub fn error(id: u64, code: i64, message: impl Into<String>) -> Self {
        Self {
            id: Some(id),
            error: Some(RpcError {
                code,
                message: message.into(),
            }),
            ..Self::empty()
        }
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
            &InitializeParams {
                protocol_version: "0.1".into(),
            },
        );
        assert_eq!(
            serde_json::to_value(&msg).unwrap(),
            json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocol_version": "0.1"}})
        );
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
                protocol_version: "0.1".into(),
            },
        );
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
