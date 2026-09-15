//! `harness-protocol` — the wire contract between the `fleetd` control plane and a harness.
//!
//! A harness is a child process that runs one unit of work. It speaks JSON-RPC 2.0, one message
//! per line, over stdin/stdout. This crate holds only the shapes on that wire; it has no IO beyond
//! the line codec and no knowledge of `fleet-core`. See NEXUS
//! `docs/specs/2026-09-14-swappable-harness-design.md` §3.

mod codec;
mod rpc;
mod types;

pub use codec::{read_message, write_message, ReadError};
pub use rpc::{error_code, method, version_compatible, MessageKind, RpcError, RpcMessage};
pub use types::*;

/// The protocol version this crate speaks. A peer with a different **major** is refused.
pub const PROTOCOL_VERSION: &str = "0.1";
