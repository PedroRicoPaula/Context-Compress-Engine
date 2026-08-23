//! Line-delimited JSON over stdio.
//!
//! stdout is the wire and carries nothing but JSON-RPC. All logging goes to
//! stderr — and never carries file contents (`docs/SECURITY.md`).

use serde::Serialize;
use tokio::io::{AsyncBufReadExt, AsyncWrite, AsyncWriteExt};

/// Refuse absurd single lines rather than growing the buffer without bound.
/// 16MB is double the file-size cap, leaving room for JSON escaping overhead.
pub const MAX_LINE_BYTES: usize = 16 * 1024 * 1024;

/// Outcome of pulling one message off the wire.
#[derive(Debug)]
pub enum Incoming {
    Line(String),
    /// Line exceeded [`MAX_LINE_BYTES`]; caller should report and continue.
    TooLong,
    /// Clean EOF: the client closed stdin. Shut down.
    Eof,
}

/// Read one line, skipping blank ones. Returns [`Incoming::Eof`] at end of input.
///
/// # Errors
/// Propagates I/O failures from the underlying reader.
pub async fn read_message<R>(reader: &mut R) -> std::io::Result<Incoming>
where
    R: AsyncBufReadExt + Unpin,
{
    let mut line = String::new();
    loop {
        line.clear();
        let bytes = reader.read_line(&mut line).await?;
        if bytes == 0 {
            return Ok(Incoming::Eof);
        }
        if bytes > MAX_LINE_BYTES {
            return Ok(Incoming::TooLong);
        }
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            return Ok(Incoming::Line(trimmed.to_owned()));
        }
    }
}

/// Serialize `message` and write it as one line, then flush.
///
/// Flushing per message is required: a client blocks waiting for the reply, so
/// buffering it would deadlock.
///
/// # Errors
/// Returns an error if serialization or the write fails.
pub async fn write_message<W, T>(writer: &mut W, message: &T) -> std::io::Result<()>
where
    W: AsyncWrite + Unpin,
    T: Serialize + ?Sized,
{
    let mut bytes = serde_json::to_vec(message)?;
    bytes.push(b'\n');
    writer.write_all(&bytes).await?;
    writer.flush().await
}

#[cfg(test)]
mod tests {
    #![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]
    use super::*;
    use serde_json::json;
    use tokio::io::BufReader;

    async fn read_all(input: &str) -> Vec<String> {
        let mut reader = BufReader::new(input.as_bytes());
        let mut out = Vec::new();
        loop {
            match read_message(&mut reader).await.expect("no io error") {
                Incoming::Line(l) => out.push(l),
                Incoming::TooLong => out.push("<too-long>".to_owned()),
                Incoming::Eof => return out,
            }
        }
    }

    #[tokio::test]
    async fn reads_messages_one_per_line() {
        let got = read_all("{\"a\":1}\n{\"b\":2}\n").await;
        assert_eq!(got, vec![r#"{"a":1}"#, r#"{"b":2}"#]);
    }

    #[tokio::test]
    async fn skips_blank_and_whitespace_only_lines() {
        let got = read_all("\n   \n{\"a\":1}\n\n").await;
        assert_eq!(got, vec![r#"{"a":1}"#]);
    }

    #[tokio::test]
    async fn final_line_without_newline_is_still_read() {
        let got = read_all(r#"{"a":1}"#).await;
        assert_eq!(got, vec![r#"{"a":1}"#]);
    }

    #[tokio::test]
    async fn empty_input_is_immediate_eof() {
        assert!(read_all("").await.is_empty());
    }

    #[tokio::test]
    async fn writes_exactly_one_newline_terminated_line() {
        let mut sink: Vec<u8> = Vec::new();
        write_message(&mut sink, &json!({"ok": true})).await.expect("write");
        let text = String::from_utf8(sink).expect("utf8");
        assert_eq!(text, "{\"ok\":true}\n");
    }

    #[tokio::test]
    async fn written_payload_never_contains_a_raw_newline() {
        // A newline inside the payload would split one message into two on the wire.
        let mut sink: Vec<u8> = Vec::new();
        write_message(&mut sink, &json!({"text": "line1\nline2"})).await.expect("write");
        let text = String::from_utf8(sink).expect("utf8");
        assert_eq!(text.matches('\n').count(), 1, "{text}");
    }
}
