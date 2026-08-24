//! The two tools: `compress_file` and `get_symbol`.
//!
//! Part of the wiring layer with `main.rs` -- this is where the MCP side and
//! the compression side are allowed to meet (`docs/ARCHITECTURE.md`). Split out
//! of `main.rs` to keep both under the 300-line cap.

use std::path::Path;

use serde::Deserialize;
use serde_json::{json, Value};

use crate::compress::{self, OUTLINE_THRESHOLD_BYTES};
use crate::mcp::dispatch;
use crate::mcp::protocol::{codes, ToolSpec};

pub const TOOL_COMPRESS_FILE: &str = "compress_file";
pub const TOOL_GET_SYMBOL: &str = "get_symbol";

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

/// Arguments for `get_symbol`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetSymbolArgs {
    file_path: String,
    symbol: String,
}

pub fn tool_specs() -> Vec<ToolSpec> {
    vec![
        ToolSpec {
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
        },
        ToolSpec {
            name: TOOL_GET_SYMBOL,
            description: "Read one named function, class, struct, or constant from a file, \
                          whole and unmodified. The counterpart to compress_file's outline \
                          mode: when an outline elides a body, call this to get that body \
                          back instead of reading the entire file.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "filePath": {
                        "type": "string",
                        "description": "Path to the file, relative to the server's working directory."
                    },
                    "symbol": {
                        "type": "string",
                        "description": "Exact name of the symbol, as it appears in the outline."
                    }
                },
                "required": ["filePath", "symbol"],
                "additionalProperties": false
            }),
        },
    ]
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
    let task = task.trim();
    if !task.is_empty() {
        header.push_str("// task: ");
        header.push_str(task);
        header.push('\n');
    }
    // Without this line the elisions look like loss. They are deferral, but
    // only if the reader knows the way back.
    if report.outlined {
        header.push_str("// bodies elided. To read one whole: get_symbol(filePath, symbol)\n");
    }
    header.push('\n');
    header.push_str(&report.text);
    header
}

/// Execute a tool call. Tool-level failures come back as `Ok(error text)` so
/// the model sees the reason instead of the client seeing a transport fault.
pub fn call_tool(name: &str, arguments: &Value, root: &Path) -> Result<Value, (i32, String)> {
    match name {
        TOOL_COMPRESS_FILE => compress_file(arguments, root),
        TOOL_GET_SYMBOL => get_symbol(arguments, root),
        other => Err((codes::INVALID_PARAMS, format!("unknown tool: {other}"))),
    }
}

/// Compress one file into a context pack.
fn compress_file(arguments: &Value, root: &Path) -> Result<Value, (i32, String)> {
    let args: CompressFileArgs = serde_json::from_value(arguments.clone()).map_err(|_| {
        (
            codes::INVALID_PARAMS,
            "expected { filePath: string }".to_owned(),
        )
    })?;

    match compress::compress_file(&args.file_path, root, OUTLINE_THRESHOLD_BYTES) {
        Ok(report) => Ok(dispatch::tool_result(
            &render_report(&report, &args.task_description),
            false,
        )),
        // Guard errors are categories, never OS messages (docs/SECURITY.md).
        Err(error) => Ok(dispatch::tool_result(
            &format!("compress_file failed: {error}"),
            true,
        )),
    }
}

/// Read one symbol back out of a file, whole.
fn get_symbol(arguments: &Value, root: &Path) -> Result<Value, (i32, String)> {
    let args: GetSymbolArgs = serde_json::from_value(arguments.clone()).map_err(|_| {
        (
            codes::INVALID_PARAMS,
            "expected { filePath: string, symbol: string }".to_owned(),
        )
    })?;

    match compress::extract_symbol(&args.file_path, root, &args.symbol) {
        Ok(Some(snippet)) => {
            let mut text = format!(
                "// {} lines {}-{}{}\n\n",
                args.file_path,
                snippet.start_line,
                snippet.end_line,
                if snippet.truncated {
                    " (truncated: block never closed)"
                } else {
                    ""
                },
            );
            text.push_str(&snippet.text);
            Ok(dispatch::tool_result(&text, false))
        }
        // Not found is an answer, not a fault: the model can try another name.
        Ok(None) => Ok(dispatch::tool_result(
            &format!("no symbol named `{}` in {}", args.symbol, args.file_path),
            true,
        )),
        Err(error) => Ok(dispatch::tool_result(
            &format!("get_symbol failed: {error}"),
            true,
        )),
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
    use crate::compress::Language;

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
    fn both_tools_are_advertised() {
        let names: Vec<&str> = tool_specs().iter().map(|t| t.name).collect();
        assert_eq!(names, vec![TOOL_COMPRESS_FILE, TOOL_GET_SYMBOL]);
    }

    #[test]
    fn get_symbol_returns_the_whole_definition() {
        let root = std::env::current_dir().expect("cwd");
        let result = call_tool(
            TOOL_GET_SYMBOL,
            &json!({ "filePath": "src/compress/whitespace.rs", "symbol": "collapse" }),
            &root,
        )
        .expect("tool call succeeds");
        let text = result["content"][0]["text"].as_str().unwrap_or_default();
        assert_eq!(result["isError"], false, "{text}");
        assert!(
            text.contains("pub fn collapse(source: &str) -> String {"),
            "{text}"
        );
        assert!(
            text.contains("lines"),
            "header names the line range: {text}"
        );
    }

    #[test]
    fn an_unknown_symbol_is_an_answer_not_a_transport_error() {
        let root = std::env::current_dir().expect("cwd");
        let result = call_tool(
            TOOL_GET_SYMBOL,
            &json!({ "filePath": "src/compress/whitespace.rs", "symbol": "no_such_thing" }),
            &root,
        )
        .expect("tool call succeeds");
        assert_eq!(result["isError"], true);
        let text = result["content"][0]["text"].as_str().unwrap_or_default();
        assert!(text.contains("no symbol named"), "{text}");
    }

    #[test]
    fn get_symbol_enforces_the_same_path_guard() {
        let root = std::env::current_dir().expect("cwd");
        let result = call_tool(
            TOOL_GET_SYMBOL,
            &json!({ "filePath": "/etc/hosts", "symbol": "anything" }),
            &root,
        )
        .expect("tool call succeeds");
        assert_eq!(result["isError"], true);
        let text = result["content"][0]["text"].as_str().unwrap_or_default();
        assert!(text.contains("outside the allowed root"), "{text}");
    }

    #[test]
    fn get_symbol_rejects_arguments_missing_the_symbol() {
        let root = std::env::current_dir().expect("cwd");
        let error = call_tool(TOOL_GET_SYMBOL, &json!({ "filePath": "a.rs" }), &root)
            .expect_err("symbol is required");
        assert_eq!(error.0, codes::INVALID_PARAMS);
    }

    #[test]
    fn an_outlined_pack_tells_the_reader_how_to_expand_it() {
        // An elision the reader cannot undo is loss. This line is the way back.
        let report = compress::compress_str("fn f() {\n    body();\n}\n", Language::Rust, 1);
        assert!(report.outlined);
        assert!(
            render_report(&report, "").contains("get_symbol"),
            "{}",
            render_report(&report, "")
        );
    }

    #[test]
    fn the_task_line_is_omitted_when_no_task_is_given() {
        let report = compress::compress_str("fn f() {}\n", Language::Rust, OUTLINE_THRESHOLD_BYTES);
        assert!(!render_report(&report, "   ").contains("// task:"));
    }
}
