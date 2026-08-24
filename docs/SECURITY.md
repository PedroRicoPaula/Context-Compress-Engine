# Security

This binary reads arbitrary local files on request from an LLM agent. The agent
is not a trusted caller — treat every tool argument as hostile input.

## Trust boundary

`tools/call` arguments are the boundary. Everything crossing it is validated in
`src/compress/guard.rs` before any file is opened.

## Path handling — non-negotiable

1. **Canonicalize first.** `fs::canonicalize` resolves `..` and symlinks. Validate
   the *resolved* path, never the string as given.
2. **Reject traversal.** Refuse a resolved path outside the configured root.
   The root is `CCE_ROOT` when set, otherwise the process CWD. String-matching
   `".."` is not sufficient — symlinks defeat it, canonicalization does not.
   A set-but-invalid `CCE_ROOT` is **fatal**, never a fallback to CWD: whoever
   named a root meant to restrict access, and quietly widening it inverts the
   request. This matters most for a globally-installed server, which otherwise
   inherits whatever directory its client started in — potentially `$HOME`.
3. **Regular files only.** Refuse dirs, symlinks-to-elsewhere, FIFOs, devices.
   Reading `/dev/zero` or a FIFO hangs the server forever.
4. **Size cap before read.** `MAX_FILE_BYTES` (8MB). `metadata()` then compare.
   Never `read_to_string` an unbounded file — 8GB RAM, shared with a model.
5. **Deny-list sensitive names** even inside the root: `.env`, `.git/config`,
   `id_rsa`, `*.pem`, `*.key`, `.aws/credentials`, `.npmrc`, `.netrc`.

## Output handling

- Compressed output goes to the caller. It may contain secrets that were in the
  source file — the deny-list is mitigation, not a guarantee. Do not add a
  "compress anything" bypass flag.
- **Never write file contents to stderr.** stderr is the only log channel
  (stdout is the JSON-RPC wire) and is commonly captured into client logs.
  Log paths and byte counts, never bodies.
- Errors returned to the caller carry a category, not an OS message. A raw
  `io::Error` leaks directory structure.

## Memory safety

- `#![forbid(unsafe_code)]` at crate root. If a future dep forces unsafe, it gets
  its own ADR.
- No `.unwrap()` / `.expect()` / indexing slices with untrusted values outside
  tests. A panic mid-request kills the server and the client's session with it.
- Slice by char boundary, not byte offset — UTF-8 input will otherwise panic on
  multi-byte characters.

## Not in scope for V1

- No network listener. stdio only; nothing is exposed to other processes.
- No auth. The security model *is* process isolation — whoever runs the binary
  already has the caller's file permissions. It grants no new access, it only
  makes existing access convenient. That is the whole threat model.
- When Ollama lands (V3): localhost only, no proxy env inheritance, and file
  content leaving the process is a new boundary needing its own review.
