//! `context-compressor-mcp` — MCP server over stdio.
//!
//! This file is the wiring layer, and the only place that names both
//! `mcp::` and `compress::`. Keeping it that way is what lets either side be
//! replaced without touching the other (`docs/ARCHITECTURE.md`).

#![forbid(unsafe_code)]

mod compress;
mod mcp;

use std::path::Path;
use std::process::ExitCode;

use serde::Deserialize;
use serde_json::{json, Value};
use tokio::io::{stdin, stdout, BufReader};

use compress::OUTLINE_THRESHOLD_BYTES;
use mcp::dispatch::{self, Method};
use mcp::protocol::{codes, Request, Response, ToolSpec};
use mcp::transport::{self, Incoming};

const TOOL_COMPRESS_FILE: &str = "compress_file";

/// Arguments for `compress_file`, exactly as the JSON schema below declares them.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompressFileArgs {
    file_path: String,
    /// Accepted and echoed, not yet used for ranking. It is the hook for V2
    /// relevance scoring — see `docs/BACKLOG.md`.
    #[serde(default)]
    task_description: String,
}

fn tool_specs() -> Vec<ToolSpec> {
    vec![ToolSpec {
        name: TOOL_COMPRESS_FILE,
        description: "Compress a source file into a high-signal context pack. Strips \
                      non-doc comments, hoists and dedupes imports, collapses whitespace, \
                      and falls back to a signature-only outline for large files.",
        input_schema: json!({
            "type": "object",
            "properties": {
                "filePath": {
                    "type": "string",
                    "description": "Path to the file, relative to the server's working directory."
                },
                "taskDescription": {
                    "type": "string",
                    "description": "What the context is for. Reserved for relevance ranking; ignored in V1."
                }
            },
            "required": ["filePath"],
            "additionalProperties": false
        }),
    }]
}

/// Render a compression report as the text the model will read.
fn render_report(report: &compress::Report, task: &str) -> String {
    let mode = if report.outlined { "outline" } else { "full" };
    let mut header = format!(
        "// context pack | {lang:?} | {mode} | {orig} -> {new} bytes ({saved}% saved)\n",
        lang = report.language,
        orig = report.original_bytes,
        new = report.compressed_bytes,
        saved = report.saved_percent(),
    );
    if !task.trim().is_empty() {
        header.push_str(&format!("// task: {}\n", task.trim()));
    }
    header.push('\n');
    header.push_str(&report.text);
    header
}

/// Execute a tool call. Tool-level failures come back as `Ok(error text)` so
/// the model sees the reason instead of the client seeing a transport fault.
fn call_tool(name: &str, arguments: &Value, root: &Path) -> Result<Value, (i32, String)> {
    if name != TOOL_COMPRESS_FILE {
        return Err((codes::INVALID_PARAMS, format!("unknown tool: {name}")));
    }

    let args: CompressFileArgs = serde_json::from_value(arguments.clone())
        .map_err(|_| (codes::INVALID_PARAMS, "expected { filePath: string }".to_owned()))?;

    match compress::compress_file(&args.file_path, root, OUTLINE_THRESHOLD_BYTES) {
        Ok(report) => Ok(dispatch::tool_result(render_report(&report, &args.task_description), false)),
        // Guard errors are categories, never OS messages (docs/SECURITY.md).
        Err(error) => Ok(dispatch::tool_result(format!("compress_file failed: {error}"), true)),
    }
}

/// Build the reply for one request, or `None` if it was a notification.
fn handle(request: &Request, root: &Path) -> Option<Response> {
    let id = request.id.clone().unwrap_or(Value::Null);

    match dispatch::classify(request) {
        Method::Initialized => None,
        Method::Initialize => Some(Response::success(id, dispatch::initialize_result())),
        Method::Ping => Some(Response::success(id, json!({}))),
        Method::ToolsList => {
            Some(Response::success(id, dispatch::tools_list_result(&tool_specs())))
        }
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
    #![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]
    use super::*;
    use compress::Language;

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
        let text = value["result"]["content"][0]["text"].as_str().unwrap_or_default();
        assert!(text.contains("outside the allowed root"), "{text}");
    }

    #[test]
    fn compressing_a_real_file_returns_a_stats_header() {
        // src/main.rs is guaranteed to exist relative to the crate root.
        let root = std::env::current_dir().expect("cwd");
        let result = call_tool(
            TOOL_COMPRESS_FILE,
            &json!({ "filePath": "src/main.rs", "taskDescription": "review the wiring" }),
            &root,
        )
        .expect("tool call succeeds");
        let text = result["content"][0]["text"].as_str().unwrap_or_default();
        assert!(text.starts_with("// context pack | Rust |"), "{text}");
        assert!(text.contains("// task: review the wiring"), "{text}");
        assert_eq!(result["isError"], false);
    }

    #[test]
    fn the_task_line_is_omitted_when_no_task_is_given() {
        let report = compress::compress_str("fn f() {}\n", Language::Rust, OUTLINE_THRESHOLD_BYTES);
        assert!(!render_report(&report, "   ").contains("// task:"));
    }
}
