# Backlog

## Measured V1 reality (2026-08-23)

First real run against this repo's own files:

| Input | Result |
|---|---|
| Small, doc-heavy, rustfmt'd source (1-9 KB) | 2-8% saved |
| Non-code (`.md`, `.toml`) | 0% — whitespace pass only |
| 60 KB file, outline mode | **66% saved** |
| 441 KB scipy module, outline mode | **93.5% saved**, all 138 functions and 88 docstring summaries kept |

The cheap passes barely pay on code that is already clean, because they remove
inline comments and stray whitespace and such files have neither. Nearly all of
V1's value is in outline mode. Two consequences worth acting on:

- [ ] `OUTLINE_THRESHOLD_BYTES` (24 KB) is a guess and is probably too high.
      Make it a tool argument before tuning it blind.
- [ ] Measure against a foreign codebase, not our own. Our files are unusually
      doc-heavy and unusually well formatted — the worst possible benchmark.
- [ ] Byte ratio flatters outline mode and punishes doc retention. Token
      counting would tell the truth.

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

- [ ] `MAX_CONTINUATION_LINES` (24) exists because parenthesis counting is
      naive about string literals. A real lexer removes the cap. Another case
      for ADR-004.
- [ ] Only the docstring *summary* survives outline mode (ADR-007). If an agent
      ever needs parameter docs, that is a tool argument, not a default.

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
