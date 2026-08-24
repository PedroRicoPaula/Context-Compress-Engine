# Architecture Decision Records

Newest last. Format: context → decision → consequence.

---

## ADR-001: Custom JSON-RPC over an MCP crate

**Context.** README asks for "a lightweight MCP crate if a stable one exists".
The official `rmcp` Rust SDK exists but pulls a broad async surface, and the
protocol subset we need is tiny: `initialize`, `tools/list`, `tools/call`.

**Decision.** Hand-roll the protocol in `src/mcp/`. ~150 lines of serde structs
and a match on `method`.

**Consequence.** No dependency churn, no version pinning against a moving spec,
binary stays small. We own spec compliance: if we add resources, prompts,
sampling, or notifications, revisit — at that point the SDK earns its weight.

---

## ADR-002: Single crate, not a workspace

**Context.** Strict decoupling of MCP and parsing is required. A workspace with
`cce-mcp` + `cce-compress` crates would enforce it at compile time.

**Decision.** One crate, two module trees, boundary enforced by review.

**Consequence.** Faster builds, one `Cargo.toml`, less ceremony for an MVP. We
lose compiler-enforced separation. Escape hatch documented in ARCHITECTURE.md —
if the boundary is violated twice, split it.

---

## ADR-003: Tokio, but only the pieces used

**Context.** Async runtime needed for future concurrent Ollama calls; stdio
reading itself is sequential and would be fine blocking.

**Decision.** `tokio` with `features = ["rt-multi-thread", "macros", "io-std", "io-util"]`,
`default-features = false`. Not `features = ["full"]`.

**Consequence.** Smaller compile, no unused `net`/`process`/`signal`/`fs`
machinery. Adding a feature is one word in `Cargo.toml` when actually needed.

---

## ADR-004: Line heuristics before tree-sitter

**Context.** README specifies tree-sitter. Each grammar is a C library compiled
into the binary (~2-5MB per language), needs a C toolchain, and costs build time.

**Decision.** V1 ships regex/line heuristics. `tree-sitter` stays in
`BACKLOG.md`, not `Cargo.toml`.

**Consequence.** V1 is byte-cheap and instantly buildable, but will mis-handle
comment markers inside string literals and multi-line generic signatures. Those
are the failing cases that justify tree-sitter — collect them as tests first,
then swap the `signatures.rs` implementation behind its existing function
signature. The module boundary is already shaped to allow that swap.

---

## ADR-005: `reqwest` declared but unused in V1

**Context.** Ollama integration is V3. Declaring the dep now compiles TLS stacks
we do not call.

**Decision.** `reqwest` is listed as an **optional** dependency behind an
`ollama` feature flag, off by default.

**Consequence.** Default `cargo build` stays lean; the intended dependency is
still recorded in `Cargo.toml` rather than lost. `--features ollama` when V3 starts.

---

## ADR-006: Doc comments survive compression

**Context.** "Lossless semantic" compression. Comments are the biggest easy win
in byte terms.

**Decision.** Strip `//` and `#` inline/trailing comments. **Keep** `///`, `//!`,
`/** */`, and Python/JS docstrings.

**Consequence.** Docstrings carry intent an agent cannot re-derive from
signatures; inline comments mostly restate the line below them. This is the
"lossless" line we drew — recorded here because it is a judgment call, not a fact.

---

## ADR-007: Outline keeps the docstring summary, not the docstring

**Context.** ADR-006 says documentation survives compression. Outline mode
violated it for Python: a docstring is a string literal, not a comment, so the
declaration rules never saw one. On scipy's `_stats_py.py` all 88 docstrings
were dropped while 164 of their bullet lines survived as orphans.

Keeping them whole was the obvious fix and the wrong one — scipy's docstrings
carry full parameter tables and examples, and preserving them takes the output
from 28 KB to an estimated 150 KB, undoing most of the compression.

**Decision.** Keep the **summary line only**, re-emitted as a closed one-line
docstring. PEP 257 defines that line as a complete sentence describing the
function, which is exactly the granularity an outline wants.

**Consequence.** 88 of 88 docstrings now survive in summary form for about
1 KB — 93.7% compression becomes 93.5%. Parameter descriptions are gone, so
outline mode still cannot answer "what does this argument do"; it answers
"what is this function for", which is what an outline is asked. Emitting the
summary *closed* is not cosmetic: an unterminated `"""` would make the whole
output unparseable.

---

## ADR-008: Declarations are followed to balanced parentheses

**Context.** Outline mode decided line by line. A signature spanning two lines
kept only the first, so `def ttest_ind(a, b, *, axis=0, equal_var=True,` was
emitted with 6 of its 9 parameters and no closing paren.

**Decision.** On a declaration whose parentheses do not balance, keep
consuming lines until they do — capped at `MAX_CONTINUATION_LINES` (24).

**Consequence.** 15 truncated signatures and 53 truncated decorators went to
zero on the scipy file. The cap exists because the counter is naive about
parentheses inside string literals; when it misfires it absorbs at most 24
lines instead of the rest of the file. A real lexer removes the cap — that is
ADR-004's tree-sitter path, and this is another case building toward it.

**The general rule, worth stating once:** partial output that *looks* complete
is worse than no output. A truncated signature is trusted and acted on. Every
pass must either emit something whole or emit a marker saying it did not.
