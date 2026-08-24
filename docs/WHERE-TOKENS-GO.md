# Where a coding session's tokens actually go

Measured 2026-08-24 against this project's own build session, by parsing the
real `usage` block Claude Code writes into its session transcript. Billed
figures, not estimates.

This exists because the project had been optimising read-side compression
without ever checking whether reads are what a session spends.

## The session

| | |
|---|---|
| Billed tokens | **101,382,076** |
| of which cache reads | **97,947,497 (97.3%)** |
| Unique content | ~631k characters (~158k tokens) |
| Verified commits | 4 |

Every unique token was re-read roughly **620 times**. That is the single most
important number here, and it cuts both ways: it multiplies the value of
anything removed from context, and multiplies the cost of anything added.

## Where the content went

| | characters | ~tokens |
|---|---|---|
| Bash calls | 335,209 | 83,802 |
| — of which file writes (heredocs) | 204,662 | 51,165 |
| — of which reads (grep/sed/cat) | 69,150 | 17,287 |
| Tool results | 97,993 | 24,498 |
| `Write`/`Edit` calls | 79,543 | 19,885 |
| Assistant text | 75,029 | 18,757 |
| User text | 42,323 | 10,580 |

Counted by destination rather than by tool:

```
writing files   83 operations   340,808 ch   ~85,200 tokens
reading files                    30,197 ch    ~7,500 tokens
```

**Writing outweighed reading 11:1.** The same files were rewritten whole,
repeatedly: `signatures.rs` 7 times, `main.rs` 7 times, `DECISIONS.md` 4 times.

Tool results were small — 181 of them, median 274 characters, **none over 5k**.
Reading files was not this session's cost.

## What this means for this project

**Honest version: on this session, the compressor would have saved about 3.7%.**
Reads were ~7,500 tokens; compressing them 80% saves ~6,000, times the ~620x
cache multiplier, against 101M billed. Real, but modest.

**Rewriting files whole cost 4–7x more than reading them.** Roughly 85k tokens
of write traffic, much of it re-emitting unchanged lines around a small change.
That is a working-method problem, and no amount of read-side compression
touches it.

This does not invalidate the tool. It bounds it:

- This session **built** the compressor, so it wrote far more than a normal
  session would. Maintenance work on an existing large codebase inverts the
  ratio — that is the case `compress_file` is for, and the scipy measurement
  (93.5% on a 441 KB file) still stands.
- The 620x cache multiplier means read-side savings compound. A file read once
  at turn 5 is re-billed for every turn after it. Cutting 19 KB from that file
  is not a 19 KB saving.
- But it also means **write traffic compounds identically**, and this session
  had 11x more of it.

## Actions

- [x] Recorded, so the next roadmap decision is not made on the assumption that
      reads dominate. They do not, in every session.
- [ ] Prefer surgical `Edit` over whole-file rewrites. Seven full rewrites of a
      300-line module is roughly 30k characters to change perhaps 400.
- [ ] Re-measure on a **maintenance** session — one that reads an unfamiliar
      codebase rather than writing a new one. That is the session the tool is
      designed for and the one where its value should show. Until then the 3.7%
      figure is the only measured number we have, and it is from the least
      favourable case.
- [ ] Compare a session with the MCP server enabled against one without, on
      comparable work. `bench/usage.jsonl` plus `session_tokens.py` gives both
      halves of that.

## Method note

The billed-token totals come straight from the transcript's `usage` blocks and
are exact. The category breakdown is derived by classifying tool calls, which
is approximate — one early version of that classifier filed 94% of reads as
"other" because it keyed on a command prefix that most commands happened to
share. Treat the totals as fact and the split as indicative.

Transcripts live in `~/.claude/projects/<project>/<session>.jsonl`.
