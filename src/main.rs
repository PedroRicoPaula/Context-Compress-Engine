//! `context-compressor-mcp` — MCP server over stdio.
//!
//! This file is the wiring layer, and the only place that names both
//! `mcp::` and `compress::`. Keeping it that way is what lets either side be
//! replaced without touching the other (`docs/ARCHITECTURE.md`).

#![forbid(unsafe_code)]

mod compress;
mod mcp;
mod tool;

use std::path::Path;
use std::process::ExitCode;

use serde_json::{json, Value};
use tokio::io::{stdin, stdout, BufReader};

use mcp::dispatch::{self, Method};
use mcp::protocol::{codes, Request, Response};
use mcp::transport::{self, Incoming};
use tool::{call_tool, tool_specs};

/// Build the reply for one request, or `None` if it was a notification.
fn handle(request: &Request, root: &Path) -> Option<Response> {
    // JSON-RPC 2.0: a notification carries no id and MUST NOT be answered --
    // whatever its method. Replying would leave the client matching a response
    // to a request it never made.
    if request.is_notification() {
        return None;
    }
    let id = request.id.clone().unwrap_or(Value::Null);

    match dispatch::classify(request) {
        // `Initialized` is reachable here only if a client sent it *with* an id,
        // against the spec. Answer anyway rather than leave it waiting.
        Method::Initialized | Method::Ping => Some(Response::success(id, json!({}))),
        Method::Initialize => Some(Response::success(id, dispatch::initialize_result())),
        Method::ToolsList => Some(Response::success(
            id,
            dispatch::tools_list_result(&tool_specs()),
        )),
        Method::ToolsCall { name, arguments } => Some(match call_tool(&name, &arguments, root) {
            Ok(result) => Response::success(id, result),
            Err((code, message)) => Response::failure(id, code, message),
        }),
        Method::Unknown(method) => Some(dispatch::unknown_method_response(id, &method)),
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    let Ok(root) = std::env::current_dir() else {
        eprintln!("fatal: cannot determine working directory");
        return ExitCode::FAILURE;
    };
    eprintln!(
        "{} {} ready on stdio (root: {})",
        dispatch::SERVER_NAME,
        dispatch::SERVER_VERSION,
        root.display()
    );

    let mut reader = BufReader::new(stdin());
    let mut writer = stdout();

    loop {
        let incoming = match transport::read_message(&mut reader).await {
            Ok(value) => value,
            Err(error) => {
                eprintln!("fatal: stdin read failed: {error}");
                return ExitCode::FAILURE;
            }
        };

        let line = match incoming {
            Incoming::Eof => return ExitCode::SUCCESS,
            Incoming::TooLong => {
                let response =
                    Response::failure(Value::Null, codes::INVALID_REQUEST, "message too large");
                if !write_or_bail(&mut writer, &response).await {
                    return ExitCode::FAILURE;
                }
                continue;
            }
            Incoming::Line(line) => line,
        };

        // A malformed line still gets an answer: silence would hang the client.
        let response = match serde_json::from_str::<Request>(&line) {
            Ok(request) => handle(&request, &root),
            Err(_) => Some(Response::failure(
                Value::Null,
                codes::PARSE_ERROR,
                "invalid JSON-RPC request",
            )),
        };

        if let Some(response) = response {
            if !write_or_bail(&mut writer, &response).await {
                return ExitCode::FAILURE;
            }
        }
    }
}

/// Write one response. Returns false when the wire is gone and we should exit.
async fn write_or_bail<W>(writer: &mut W, response: &Response) -> bool
where
    W: tokio::io::AsyncWrite + Unpin,
{
    match transport::write_message(writer, response).await {
        Ok(()) => true,
        Err(error) => {
            // Client closed the pipe, or the disk is gone. Either way we are done.
            eprintln!("fatal: stdout write failed: {error}");
            false
        }
    }
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
    use tool::TOOL_COMPRESS_FILE;

    fn request(line: &str) -> Request {
        serde_json::from_str(line).expect("valid request json")
    }

    fn reply(line: &str) -> Value {
        let root = std::env::current_dir().expect("cwd");
        let response = handle(&request(line), &root).expect("expected a reply");
        serde_json::to_value(response).expect("serializable")
    }

    #[test]
    fn initialize_returns_server_info() {
        let value = reply(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#);
        assert_eq!(value["id"], 1);
        assert_eq!(value["result"]["serverInfo"]["name"], dispatch::SERVER_NAME);
    }

    #[test]
    fn the_initialized_notification_gets_no_reply() {
        let root = std::env::current_dir().expect("cwd");
        let request = request(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#);
        assert!(handle(&request, &root).is_none());
    }

    #[test]
    fn no_notification_is_ever_answered_whatever_its_method() {
        let root = std::env::current_dir().expect("cwd");
        for method in ["tools/list", "initialize", "ping", "resources/list"] {
            let line = format!(r#"{{"jsonrpc":"2.0","method":"{method}"}}"#);
            assert!(
                handle(&request(&line), &root).is_none(),
                "answered {method}"
            );
        }
    }

    #[test]
    fn tools_list_advertises_compress_file_with_a_schema() {
        let value = reply(r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#);
        let tool = &value["result"]["tools"][0];
        assert_eq!(tool["name"], TOOL_COMPRESS_FILE);
        assert_eq!(tool["inputSchema"]["required"][0], "filePath");
    }

    #[test]
    fn unknown_methods_get_method_not_found() {
        let value = reply(r#"{"jsonrpc":"2.0","id":3,"method":"resources/list"}"#);
        assert_eq!(value["error"]["code"], codes::METHOD_NOT_FOUND);
    }

    #[test]
    fn a_tool_call_with_bad_arguments_is_an_invalid_params_error() {
        let value = reply(
            r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"compress_file","arguments":{}}}"#,
        );
        assert_eq!(value["error"]["code"], codes::INVALID_PARAMS);
    }

    #[test]
    fn an_unknown_tool_name_is_an_invalid_params_error() {
        let value = reply(
            r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"nope","arguments":{}}}"#,
        );
        assert_eq!(value["error"]["code"], codes::INVALID_PARAMS);
    }

    #[test]
    fn a_rejected_path_is_a_tool_error_not_a_transport_error() {
        // The model must see why, and be able to retry with a better path.
        let value = reply(
            r#"{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"compress_file","arguments":{"filePath":"/etc/hosts"}}}"#,
        );
        assert!(value["error"].is_null(), "{value}");
        assert_eq!(value["result"]["isError"], true);
        let text = value["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_default();
        assert!(text.contains("outside the allowed root"), "{text}");
    }
}
