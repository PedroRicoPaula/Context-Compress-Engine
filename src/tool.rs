//! The `compress_file` tool: argument schema, execution, and output rendering.
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

pub fn tool_specs() -> Vec<ToolSpec> {
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
    let task = task.trim();
    if !task.is_empty() {
        header.push_str("// task: ");
        header.push_str(task);
        header.push('\n');
    }
    header.push('\n');
    header.push_str(&report.text);
    header
}

/// Execute a tool call. Tool-level failures come back as `Ok(error text)` so
/// the model sees the reason instead of the client seeing a transport fault.
pub fn call_tool(name: &str, arguments: &Value, root: &Path) -> Result<Value, (i32, String)> {
    if name != TOOL_COMPRESS_FILE {
        return Err((codes::INVALID_PARAMS, format!("unknown tool: {name}")));
    }

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
    fn the_task_line_is_omitted_when_no_task_is_given() {
        let report = compress::compress_str("fn f() {}\n", Language::Rust, OUTLINE_THRESHOLD_BYTES);
        assert!(!render_report(&report, "   ").contains("// task:"));
    }
}
