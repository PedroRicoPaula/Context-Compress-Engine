//! Method-name routing.
//!
//! Turns a raw [`Request`] into a [`Method`] the wiring layer can act on. This
//! module deliberately does not *execute* anything — executing `ToolsCall`
//! would mean knowing what a tool is, which belongs in `main.rs`.

use serde_json::{json, Value};

use super::protocol::{Request, Response, ToolSpec, PROTOCOL_VERSION};

/// Server identity reported during `initialize`.
pub const SERVER_NAME: &str = "context-compressor-mcp";
pub const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

/// A recognised (or explicitly unrecognised) inbound method.
#[derive(Debug, PartialEq, Eq)]
pub enum Method {
    Initialize,
    /// Post-handshake notification. Requires no reply.
    Initialized,
    ToolsList,
    ToolsCall {
        name: String,
        arguments: Value,
    },
    Ping,
    Unknown(String),
}

/// Classify a request by its `method` field.
///
/// `tools/call` arguments default to an empty object when omitted; validating
/// their *shape* is the caller's job, since only the caller knows the tool.
#[must_use]
pub fn classify(request: &Request) -> Method {
    match request.method.as_str() {
        "initialize" => Method::Initialize,
        "notifications/initialized" | "initialized" => Method::Initialized,
        "tools/list" => Method::ToolsList,
        "ping" => Method::Ping,
        "tools/call" => {
            let params = request.params.as_ref();
            let name = params
                .and_then(|p| p.get("name"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            let arguments = params
                .and_then(|p| p.get("arguments"))
                .cloned()
                .unwrap_or_else(|| json!({}));
            Method::ToolsCall { name, arguments }
        }
        other => Method::Unknown(other.to_owned()),
    }
}

/// Handshake payload: what this server is and what it can do.
#[must_use]
pub fn initialize_result() -> Value {
    json!({
        "protocolVersion": PROTOCOL_VERSION,
        "capabilities": { "tools": { "listChanged": false } },
        "serverInfo": { "name": SERVER_NAME, "version": SERVER_VERSION }
    })
}

#[must_use]
pub fn tools_list_result(tools: &[ToolSpec]) -> Value {
    json!({ "tools": tools })
}

/// Wrap tool output in MCP `content` form.
///
/// A failing *tool* is reported here with `isError`, not as a JSON-RPC error:
/// the protocol call itself succeeded, so the model gets to see the reason and
/// retry rather than the client treating it as a transport fault.
#[must_use]
pub fn tool_result(text: &str, is_error: bool) -> Value {
    json!({
        "content": [ { "type": "text", "text": text } ],
        "isError": is_error
    })
}

/// Reply to an unknown method.
#[must_use]
pub fn unknown_method_response(id: Value, method: &str) -> Response {
    Response::failure(
        id,
        super::protocol::codes::METHOD_NOT_FOUND,
        format!("unknown method: {method}"),
    )
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::panic,
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::indexing_slicing
    )]
    use super::*;

    fn request(method: &str, params: Option<Value>) -> Request {
        let mut body = json!({ "jsonrpc": "2.0", "id": 1, "method": method });
        if let Some(p) = params {
            body["params"] = p;
        }
        serde_json::from_value(body).expect("valid request")
    }

    #[test]
    fn classifies_the_handshake_methods() {
        assert_eq!(classify(&request("initialize", None)), Method::Initialize);
        assert_eq!(classify(&request("tools/list", None)), Method::ToolsList);
        assert_eq!(classify(&request("ping", None)), Method::Ping);
    }

    #[test]
    fn accepts_both_spellings_of_the_initialized_notification() {
        assert_eq!(
            classify(&request("notifications/initialized", None)),
            Method::Initialized
        );
        assert_eq!(classify(&request("initialized", None)), Method::Initialized);
    }

    #[test]
    fn extracts_tool_name_and_arguments() {
        let req = request(
            "tools/call",
            Some(json!({ "name": "compress_file", "arguments": { "filePath": "a.rs" } })),
        );
        match classify(&req) {
            Method::ToolsCall { name, arguments } => {
                assert_eq!(name, "compress_file");
                assert_eq!(arguments["filePath"], "a.rs");
            }
            other => panic!("expected ToolsCall, got {other:?}"),
        }
    }

    #[test]
    fn tool_call_without_arguments_yields_an_empty_object() {
        let req = request("tools/call", Some(json!({ "name": "compress_file" })));
        match classify(&req) {
            Method::ToolsCall { arguments, .. } => assert_eq!(arguments, json!({})),
            other => panic!("expected ToolsCall, got {other:?}"),
        }
    }

    #[test]
    fn tool_call_with_no_params_at_all_does_not_panic() {
        // Hostile input: params omitted entirely on a method that needs them.
        match classify(&request("tools/call", None)) {
            Method::ToolsCall { name, arguments } => {
                assert!(name.is_empty());
                assert_eq!(arguments, json!({}));
            }
            other => panic!("expected ToolsCall, got {other:?}"),
        }
    }

    #[test]
    fn unknown_methods_are_named_in_the_error() {
        assert_eq!(
            classify(&request("resources/list", None)),
            Method::Unknown("resources/list".to_owned())
        );
    }

    #[test]
    fn initialize_result_advertises_tools_capability() {
        let result = initialize_result();
        assert_eq!(result["protocolVersion"], PROTOCOL_VERSION);
        assert!(result["capabilities"]["tools"].is_object());
        assert_eq!(result["serverInfo"]["name"], SERVER_NAME);
    }

    #[test]
    fn tool_result_marks_errors_without_failing_the_rpc() {
        let result = tool_result("boom", true);
        assert_eq!(result["isError"], true);
        assert_eq!(result["content"][0]["text"], "boom");
        assert_eq!(result["content"][0]["type"], "text");
    }
}
