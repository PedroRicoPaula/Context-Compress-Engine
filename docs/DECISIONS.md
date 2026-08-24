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

---

## ADR-009: Retrieval before relevance

**Context.** An external review proposed evolving this project into a "Context
Intelligence" engine, with a six-phase roadmap: V2 task-aware relevance, V3
AST, V4 Ollama reranking, V5 project-level context packs, V6 retrieval.

The review's central hypothesis is right: an agent does not need a compressed
copy of everything, it needs the smallest task-relevant representation. But the
proposed ordering puts the cheapest, most useful piece last.

**Decision.** Implement retrieval (`get_symbol`) as the next step, and leave
relevance, AST, Ollama, and project packs where they are until there is
evidence for them.

**Reasoning, in the order it mattered:**

1. **Retrieval removes the problem the other phases try to solve.** V2-V5 are
   increasingly elaborate ways to guess correctly what the model will need.
   Retrieval makes guessing cheap: guess wrong, the model asks. That is worth
   more than a better guess and costs a fraction as much.
2. **It fixes a defect we measured.** The scipy run showed outline mode is
   reliable for navigation and useless for use. Retrieval closes exactly that
   gap, and it closes it without making the outline bigger.
3. **It is ~250 lines.** No AST, no index, no cache, no model, no embeddings,
   no invalidation problem.
4. **The cost baseline forbids the alternatives, for now.** Compression is
   4.4 MB RSS and 30 ms for a 441 KB file. An Ollama call on this machine costs
   seconds and gigabytes, competing with the model the tool exists to serve, and
   trades determinism for a judgment a small model is weak at. Embeddings lose
   to lexical and structural matching on code, where a symbol called
   `processPayment` is found by searching for "payment".

**Consequence.** The roadmap is now: retrieval → real use → evidence →
whatever the evidence justifies. The rejected phases are not wrong, they are
unevidenced; each returns the moment a concrete failure demands it.

**What would reverse this.** Real use showing the model repeatedly calls
`get_symbol` three or four times per task, hunting. That is relevance
selection earning its place with evidence rather than argument.

**Not adopted:** the review also asked for a 25-section design document. A
project whose thesis is "too much context is the problem" should not answer it
with 6,000 speculative words about a system that has never been used in anger.
The decisions live here, where decisions are already looked for.

---

## ADR-010: Outline threshold is 8 KB, measured

**Context.** `OUTLINE_THRESHOLD_BYTES` was 24 KB, set by guess before outline
mode kept whole signatures (ADR-008), docstring summaries (ADR-007), or had a
way back via `get_symbol` (ADR-009). It was calibrated against a version of
the feature that no longer exists.

**Method.** `bench/threshold.py` runs the release binary over 274 real files
(6.6 MB) from three projects — this repo, a TypeScript/Python app, and scipy —
sweeping the threshold and comparing each file against itself compressed whole.

| threshold | files outlined | corpus saved | body loss/file |
|---|---|---|---|
| 2048 | 212 | 84.5% | 22.5 KB |
| 4096 | 148 | 82.7% | 31.5 KB |
| **8192** | **88** | **79.0%** | 50.1 KB |
| 16384 | 55 | 74.1% | 74.3 KB |
| 24576 (was) | 37 | 69.4% | 102.0 KB |
| 49152 | 25 | 64.2% | 137.4 KB |

**Decision.** 8192.

**Reasoning.** The curve has a knee there. 24 KB → 8 KB buys 9.6 points;
8 KB → 4 KB buys 3.7 more while nearly doubling the files that lose bodies,
and every outlined file is a potential extra `get_symbol` round trip. Above
8 KB we leave most of the value unclaimed; below it we pay in tool calls for
diminishing returns.

**Consequence.** Files between 8 and 24 KB now outline — 51 more in this
corpus. Their bodies are recoverable one call at a time, which is what makes
the more aggressive setting defensible at all: without ADR-009 this would be
straightforward loss.

**Also measured, worth recording:** structural compression alone (no
outlining) saves ~12% and is *flat across every file size* — 5.4% at 1–2 KB,
12.3% at 64 KB+. The cheap passes are not where the value is. Nearly all of it
is outline mode, which is why the threshold matters more than any heuristic
in `comments.rs` or `whitespace.rs`.

**What would change it.** Real use showing several `get_symbol` calls per
task: that means we outlined too eagerly, and the threshold goes back up.
`outlineThreshold` is now a per-call argument, so this is tunable without a
rebuild — and re-measurable with `bench/threshold.py` against any corpus.
