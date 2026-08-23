//! JSON-RPC 2.0 wire types for the MCP transport.
//!
//! This module knows about envelopes and error codes. It knows nothing about
//! what any tool does — see `docs/ARCHITECTURE.md` for the decoupling rule.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// MCP revision this server implements. Advertised in `initialize`.
pub const PROTOCOL_VERSION: &str = "2024-11-05";

const JSONRPC: &str = "2.0";

/// Standard JSON-RPC 2.0 error codes.
pub mod codes {
    pub const PARSE_ERROR: i32 = -32700;
    pub const INVALID_REQUEST: i32 = -32600;
    pub const METHOD_NOT_FOUND: i32 = -32601;
    pub const INVALID_PARAMS: i32 = -32602;
}

/// An inbound message. `id` absent means it is a notification: per JSON-RPC 2.0
/// a notification MUST NOT be answered.
#[derive(Debug, Deserialize)]
pub struct Request {
    #[serde(default)]
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
}

impl Request {
    #[must_use]
    pub const fn is_notification(&self) -> bool {
        self.id.is_none()
    }
}

#[derive(Debug, Serialize)]
pub struct Response {
    jsonrpc: &'static str,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<ErrorObject>,
}

#[derive(Debug, Serialize)]
pub struct ErrorObject {
    pub code: i32,
    pub message: String,
}

impl Response {
    /// Successful reply. `id` is echoed verbatim, including a null id.
    #[must_use]
    pub fn success(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: JSONRPC,
            id,
            result: Some(result),
            error: None,
        }
    }

    /// Failure reply. `message` must be a category, never a raw OS error —
    /// see `docs/SECURITY.md` on leaking directory structure.
    #[must_use]
    pub fn failure(id: Value, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: JSONRPC,
            id,
            result: None,
            error: Some(ErrorObject {
                code,
                message: message.into(),
            }),
        }
    }
}

/// Declaration of one tool, as returned by `tools/list`.
///
/// Held here as plain data so that `mcp` never needs to know which tools exist;
/// concrete specs are built in `main.rs`.
#[derive(Debug, Serialize)]
pub struct ToolSpec {
    pub name: &'static str,
    pub description: &'static str,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
}

#[cfg(test)]
mod tests {
    #![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]
    use super::*;
    use serde_json::json;

    #[test]
    fn request_without_id_is_a_notification() {
        let req: Request =
            serde_json::from_str(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
                .expect("valid json");
        assert!(req.is_notification());
    }

    #[test]
    fn request_with_id_is_not_a_notification() {
        let req: Request = serde_json::from_str(r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#)
            .expect("valid json");
        assert!(!req.is_notification());
    }

    #[test]
    fn id_zero_is_still_a_real_id() {
        // Guards against a truthiness-style bug: id 0 must not read as absent.
        let req: Request = serde_json::from_str(r#"{"jsonrpc":"2.0","id":0,"method":"ping"}"#)
            .expect("valid json");
        assert!(!req.is_notification());
    }

    #[test]
    fn success_omits_the_error_field() {
        let wire = serde_json::to_string(&Response::success(json!(7), json!({"ok": true})))
            .expect("serializable");
        assert!(wire.contains(r#""id":7"#), "{wire}");
        assert!(wire.contains(r#""result""#), "{wire}");
        assert!(!wire.contains("error"), "{wire}");
    }

    #[test]
    fn failure_omits_the_result_field() {
        let wire = serde_json::to_string(&Response::failure(
            json!(null),
            codes::PARSE_ERROR,
            "parse error",
        ))
        .expect("serializable");
        assert!(wire.contains(r#""code":-32700"#), "{wire}");
        assert!(!wire.contains(r#""result""#), "{wire}");
    }

    #[test]
    fn missing_params_deserializes_to_none() {
        let req: Request =
            serde_json::from_str(r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#)
                .expect("valid json");
        assert!(req.params.is_none());
    }
}
