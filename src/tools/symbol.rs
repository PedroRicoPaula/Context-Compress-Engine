//! `get_symbol`: read one definition back out of a file, whole.
//!
//! The counterpart to outline mode. Without it, eliding a body is loss; with
//! it, eliding is deferral (ADR-009).

use std::path::Path;

use serde::Deserialize;
use serde_json::{json, Value};

use crate::compress;
use crate::mcp::dispatch;
use crate::mcp::protocol::{codes, ToolSpec};

pub const NAME: &str = "get_symbol";

/// Arguments for `get_symbol`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetSymbolArgs {
    file_path: String,
    symbol: String,
}

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: NAME,
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
    }
}

/// Read one symbol back out of a file, whole.
pub fn run(arguments: &Value, root: &Path) -> Result<Value, (i32, String)> {
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
    use crate::tools::call_tool;

    #[test]
    fn get_symbol_returns_the_whole_definition() {
        let root = std::env::current_dir().expect("cwd");
        let result = call_tool(
            NAME,
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
            NAME,
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
            NAME,
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
        let error =
            call_tool(NAME, &json!({ "filePath": "a.rs" }), &root).expect_err("symbol is required");
        assert_eq!(error.0, codes::INVALID_PARAMS);
    }
}
