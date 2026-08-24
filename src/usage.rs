//! Opt-in usage log, for answering questions about real use with data.
//!
//! The open question after ADR-010 is how many `get_symbol` calls a real task
//! costs. One or two means the 8 KB outline threshold is right; five means we
//! outlined too eagerly. Nothing in the system could answer that, because
//! nothing recorded it.
//!
//! Off unless `CCE_USAGE_LOG` names a file. One JSON object per line, appended.
//!
//! **Metadata only.** Tool name, file path, byte counts, mode. Never file
//! contents — `docs/SECURITY.md` forbids it, and a log is exactly the place
//! that rule gets broken by accident.

use std::fmt::Write as _;
use std::fs::OpenOptions;
use std::io::Write as _;
use std::time::{SystemTime, UNIX_EPOCH};

/// Environment variable naming the log file. Unset means no logging.
pub const ENV_VAR: &str = "CCE_USAGE_LOG";

/// One recorded call.
pub struct Record<'a> {
    pub tool: &'a str,
    pub file: &'a str,
    /// Input size in bytes, when the call read a file.
    pub input_bytes: Option<usize>,
    /// Returned size in bytes.
    pub output_bytes: usize,
    /// `full`, `outline`, `symbol`, or an error category.
    pub outcome: &'a str,
}

fn escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Format one record as a single JSON line, newline included.
///
/// Pure: no environment, no I/O. This is the half worth testing hard.
#[must_use]
fn format_record(record: &Record<'_>) -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());

    let mut line = String::with_capacity(160);
    let _ = write!(
        line,
        r#"{{"ts":{seconds},"tool":"{}","file":"{}","outcome":"{}""#,
        escape(record.tool),
        escape(record.file),
        escape(record.outcome),
    );
    if let Some(input) = record.input_bytes {
        let _ = write!(line, r#","in":{input}"#);
    }
    let _ = writeln!(line, r#","out":{}}}"#, record.output_bytes);
    line
}

/// Append `record` to `path`.
///
/// Failures are silent by design: a telemetry file that cannot be written must
/// never turn a working tool call into a failed one.
fn append_to(path: &std::path::Path, record: &Record<'_>) {
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = file.write_all(format_record(record).as_bytes());
    }
}

/// Append `record` to the usage log, if `CCE_USAGE_LOG` names one.
pub fn record(record: &Record<'_>) {
    let Ok(path) = std::env::var(ENV_VAR) else {
        return;
    };
    if path.trim().is_empty() {
        return;
    }
    append_to(std::path::Path::new(&path), record);
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
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    fn scratch_path() -> std::path::PathBuf {
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("cce-usage-{}-{unique}.jsonl", std::process::id()))
    }

    fn sample<'a>(tool: &'a str, outcome: &'a str) -> Record<'a> {
        Record {
            tool,
            file: "src/main.rs",
            input_bytes: Some(100),
            output_bytes: 42,
            outcome,
        }
    }

    #[test]
    fn a_record_formats_as_one_json_line() {
        let line = format_record(&sample("compress_file", "outline"));
        assert!(line.ends_with('\n'), "{line}");
        assert_eq!(line.matches('\n').count(), 1, "{line}");

        let parsed: serde_json::Value = serde_json::from_str(line.trim()).expect("valid json");
        assert_eq!(parsed["tool"], "compress_file");
        assert_eq!(parsed["outcome"], "outline");
        assert_eq!(parsed["in"], 100);
        assert_eq!(parsed["out"], 42);
        assert!(parsed["ts"].as_u64().unwrap_or(0) > 0);
    }

    #[test]
    fn an_absent_input_size_is_omitted_rather_than_zero() {
        // get_symbol does not read a whole file, so reporting in:0 would lie.
        let line = format_record(&Record {
            tool: "get_symbol",
            file: "a.rs",
            input_bytes: None,
            output_bytes: 7,
            outcome: "symbol",
        });
        let parsed: serde_json::Value = serde_json::from_str(line.trim()).expect("valid json");
        assert!(parsed["in"].is_null(), "{line}");
    }

    #[test]
    fn quotes_and_backslashes_in_a_path_stay_valid_json() {
        let line = format_record(&Record {
            tool: "compress_file",
            file: r#"weird "name" \ here.rs"#,
            input_bytes: None,
            output_bytes: 7,
            outcome: "refused",
        });
        let parsed: serde_json::Value = serde_json::from_str(line.trim()).expect("escaping holds");
        assert_eq!(parsed["file"], r#"weird "name" \ here.rs"#);
    }

    #[test]
    fn never_records_file_contents() {
        // docs/SECURITY.md: metadata only. A log is where that rule breaks.
        let line = format_record(&sample("compress_file", "full"));
        for field in ["text", "content", "body", "source"] {
            assert!(!line.contains(field), "{field} leaked into {line}");
        }
    }

    #[test]
    fn appends_one_line_per_call_without_overwriting() {
        let path = scratch_path();
        append_to(&path, &sample("compress_file", "outline"));
        append_to(&path, &sample("get_symbol", "symbol"));

        let body = std::fs::read_to_string(&path).expect("log written");
        let _ = std::fs::remove_file(&path);

        let lines: Vec<&str> = body.lines().collect();
        assert_eq!(lines.len(), 2, "{body}");
        assert!(lines[0].contains("compress_file"), "{body}");
        assert!(lines[1].contains("get_symbol"), "{body}");
    }

    #[test]
    fn an_unwritable_path_does_not_fail_the_call() {
        append_to(
            std::path::Path::new("/nonexistent-dir-9f2a/usage.jsonl"),
            &sample("compress_file", "full"),
        ); // silent, not fatal
    }

    #[test]
    fn logging_is_off_unless_the_variable_names_a_file() {
        // The only test that touches the process-wide variable, and it only
        // ever unsets it -- so it cannot collide with tests running in
        // parallel that call record() incidentally.
        std::env::remove_var(ENV_VAR);
        record(&sample("compress_file", "full")); // must be a no-op
    }
}
