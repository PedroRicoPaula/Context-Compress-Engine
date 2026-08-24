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

- [x] `OUTLINE_THRESHOLD_BYTES` measured and set to 8 KB (ADR-010). Now also a
      per-call `outlineThreshold` argument.
- [x] Measured against a foreign codebase — 274 files, 6.6 MB, three projects.
      `bench/threshold.py` re-runs it against any corpus.
- [ ] Byte ratio flatters outline mode and punishes doc retention. Token
      counting would tell the truth.

## Done

- [x] `get_symbol` retrieval (ADR-009). Was V6 in the proposed roadmap;
      promoted because it removes the problem the intervening phases solve.

## Retrieval debt

- [ ] First match wins: overloads, re-definitions, and same-named methods on
      different classes return the first one found.
- [ ] `impl Trait for Type` is indexed under `Trait`, not `Type`.
- [ ] No `list_symbols` yet. The outline already is that list, so it only earns
      its place if real use shows the model asking for names it cannot see.

## Open questions

- [ ] The 300-line module cap counts test code, which is ~60% of most files
      here. Cap non-test lines instead? Decide with evidence, not preference —
      the cap has caught two real cases of drift so far.

## V2 — relevance

**Gated on evidence** (ADR-009): build this when real use shows the model
calling `get_symbol` repeatedly per task, hunting for the right piece.

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

## Measured, 2026-08-24 (`bench/threshold.py`, 274 files, 6.6 MB)

- Structural passes alone save ~12%, and that is **flat across every file
  size** (5.4% at 1–2 KB, 12.3% at 64 KB+). Tuning `comments.rs` or
  `whitespace.rs` further is not where the value is.
- Outline mode at 8 KB takes the corpus to 79.0%.
- By language: Python 12.5%, TypeScript 11.1%, Rust 5.3% structural. Rust is
  lowest because we keep doc comments (ADR-006) and our own code is unusually
  doc-heavy.

## Ops

- [ ] `--stdio` vs `--cli` mode so the compressor is usable without a client.
- [ ] Bench on a 50k-line file, record peak RSS. The 8GB constraint is untested.
- [ ] CI: `rust-clippy-filter.sh` + `rust-test-filter.sh` as a gate.

## Known tech debt

- Boundary between `mcp/` and `compress/` is review-enforced, not compiler-enforced (ADR-002).
- No structured logging. stderr only, and never file contents (SECURITY.md).
