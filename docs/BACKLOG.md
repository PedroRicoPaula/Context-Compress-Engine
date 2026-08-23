# Backlog

## V2 — relevance

- [ ] Use `taskDescription` (currently accepted, ignored) to rank/drop symbols.
- [ ] `compress_directory` tool — glob + per-file budget, respecting `.gitignore`.
- [ ] Token counting instead of byte counting for `stats` (byte ratio is a proxy).
- [ ] Output budget parameter (`maxTokens`) with graceful degradation order.

## V3 — local AI

- [ ] `reqwest` behind `--features ollama`; `POST /api/generate` to localhost:11434.
- [ ] Model configurable, default `qwen2.5-coder`. Timeout + hard fallback to
      heuristic-only output. Ollama being down must never fail a tool call.
- [ ] Semantic dedup of near-identical blocks.

## Parsing debt

- [ ] tree-sitter swap for `signatures.rs` — see ADR-004. Blocked on collecting
      real failing cases.
- [ ] Rust raw strings (`r#"a // b"#`) still confuse the comment scanner.
      `#[ignore]`d test in `comments.rs` documents it.
- [ ] Rust `'` is not treated as a string delimiter (lifetimes would break the
      scan), so a char literal containing a quote — `'"'` — misleads it.
- [ ] Multi-line function signatures are captured by first line only.
      `#[ignore]`d test in `signatures.rs` documents it.
- [ ] Languages: only Rust/Python/JS/TS have real rules. Everything else is the
      generic whitespace path.

## Ops

- [ ] `--stdio` vs `--cli` mode so the compressor is usable without a client.
- [ ] Bench on a 50k-line file, record peak RSS. The 8GB constraint is untested.
- [ ] CI: `rust-clippy-filter.sh` + `rust-test-filter.sh` as a gate.

## Known tech debt

- Boundary between `mcp/` and `compress/` is review-enforced, not compiler-enforced (ADR-002).
- No structured logging. stderr only, and never file contents (SECURITY.md).
