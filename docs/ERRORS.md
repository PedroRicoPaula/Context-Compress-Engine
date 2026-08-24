# Error Log

Mistakes made in this project, why they happened, and what fixed them.

## Who this is for

**Primarily a Rust learning log for the maintainer.** Compiler errors in Rust
are dense but they teach a real rule each time — `E0308`, `E0502`, `E0597` are
not noise, they are the language explaining itself. Writing the rule down once
beats re-deriving it every time.

**Secondarily a note to Claude.** Worth being precise about this: Claude does
not carry memory between sessions and does not learn from files. This log only
helps when it is *read* — which is why `CLAUDE.md` indexes it. It prevents a
repeat correction; it does not train anything.

## What earns an entry

Only two kinds. Everything else is churn that makes the file too long to read:

1. **It taught a language rule** — something that will recur in other Rust code.
2. **It happened twice** — a repeat is proof the lesson did not stick.

A typo fixed in ten seconds does not go here. Neither does a design change —
that is `DECISIONS.md`. Neither does deferred work — that is `BACKLOG.md`.

## Format

```
## YYYY-MM-DD — short title
**Symptom** what was observed
**Cause** the actual reason
**Fix** what changed
**Rule** the one line worth remembering
```

---

## 2026-08-23 — Written but never compiled

**Symptom** ~1900 lines of Rust and ~90 tests committed across five commits,
none of it ever run.

**Cause** No Rust toolchain on the machine (`docs/ENVIRONMENT.md`). The
execution plan was followed to the end without stopping at the blocker.

**Fix** Not fixed yet. Recorded in `PROGRESS.md` as the blocker, and stated
plainly rather than reported as working code.

**Rule** "Written" and "verified" are different claims. Say which one you mean.
Everything below was caught by re-reading the code, **not** by a compiler — so
the first real `cargo check` will almost certainly add entries here.

---

## 2026-08-23 — `&PathBuf` where `&Path` belongs

**Symptom** `fn call_tool(..., root: &PathBuf)` — would trigger clippy's
`ptr_arg` lint.

**Cause** Reached for the type that was already in hand (`PathBuf`) instead of
the type the function actually needs.

**Fix** Changed the parameters to `&Path`. Call sites did not change: `&root`
on a `PathBuf` coerces to `&Path` automatically.

**Rule** In Rust, owned types come in pairs with borrowed ones:

| Owned (you hold it) | Borrowed (you look at it) |
|---|---|
| `String` | `&str` |
| `PathBuf` | `&Path` |
| `Vec<T>` | `&[T]` |

A function that only *reads* takes the borrowed form. It accepts strictly more
callers and copies nothing. `&String` and `&PathBuf` are almost always a smell.

---

## 2026-08-23 — `Result<(), ()>` as a return type

**Symptom** `async fn write_or_bail(...) -> Result<(), ()>`.

**Cause** Wanted to signal "worked / did not work" and reached for `Result`
out of habit.

**Fix** Returns `bool`.

**Rule** `Result<T, E>` exists to carry *why* it failed. With `()` as the error
it carries nothing, so it is a `bool` wearing a costume — and clippy says so
(`result_unit_err`). Either give the error real content, or return `bool`.

---

## 2026-08-23 — `assert_eq!` on a type that cannot be compared

**Symptom** A test asserted `assert_eq!(compress_file(...), Err(GuardError::NotFound))`,
where the success type `Report` does not implement `PartialEq`.

**Cause** Comparing a whole `Result` silently requires *both* sides to be
comparable, including the `Ok` variant that the test never intended to touch.

**Fix** `.expect_err("missing file must fail")` to unwrap the error, then
compare only the error.

**Rule** In Rust, `==` is not built in — it comes from the `PartialEq` trait,
which a type has to opt into with `#[derive(PartialEq)]`. Comparing a
`Result<A, B>` needs it on **both** `A` and `B`. Assert on the part you care
about, not the whole envelope.

---

## 2026-08-23 — Trust boundary crossed twice

**Symptom** `compress_file` called `guard::resolve(path, root)` and then
`guard::read_text(path, root)`, which called `resolve` internally again. The
same path was canonicalized and validated twice per request.

**Cause** `read_text` took a raw user-supplied `&str`, so it had no way to know
whether validation had already happened.

**Fix** `read_text` now takes a `&Path` that has already passed `resolve`.

**Rule** Make the type prove the check happened. A function taking a raw
caller-supplied string can be called before validation, so eventually it will
be. One taking an already-validated value cannot. This matters most on a
security boundary — see `SECURITY.md`.

---

## 2026-08-23 — Documentation drifted from the code within the hour

**Symptom** `BACKLOG.md` listed "`//` inside a string literal is treated as a
comment" as a known gap. By then `comments.rs` had a quote-aware scanner and
a passing test proving the opposite.

**Cause** The doc was written from the plan; the code then turned out better
than the plan, and nothing went back to correct the doc.

**Fix** Replaced with the two limits that genuinely remain (Rust raw strings,
and `'` not being treated as a delimiter), each pinned by an `#[ignore]`d test.

**Rule** A known limitation is worth an `#[ignore]`d test, not just a prose
bullet. The test lives next to the code and starts passing when the gap closes;
a bullet in a file rots quietly.

---

## 2026-08-23 — A wrapper script hid the error it was written to report

**Symptom** `./.claude/skills/rust-check.sh` printed `FAIL check (exit 127)`
and nothing else. Exit 127 means "command not found", but the script said so
nowhere.

**Cause** Two mistakes stacked. `cargo` was not on `PATH` (non-interactive
shells do not read `~/.zshenv`), and the script's `2>/dev/null` — added to keep
compiler noise out of the context — swallowed the `command not found` message
that would have explained it.

**Fix** Every cargo wrapper now puts `~/.cargo/bin` on `PATH` itself, and
checks `command -v cargo` up front with an explicit message.

**Rule** A tool that filters output must never filter away the reason it
failed. Silence the noise, never the diagnosis.

---

## 2026-08-23 — E0618: a variable shadowed the function it was calling

**Symptom** `error[E0618]: expected function, found imports::Split`.

**Cause** In a test: `let split = split(...)` bound a *variable* named `split`,
so the next `split(...)` call resolved to the variable, not the function.

**Fix** Named the bindings `rust` and `python` after what they hold.

**Rule** Rust allows **shadowing** — reusing a name for a new binding — and it
is genuinely useful (`let x = x.trim();`). But a binding shadows *everything*
with that name in scope, functions included. Do not name a variable after the
function that produced it.

---

## 2026-08-23 — A dead-code warning was hiding a protocol bug

**Symptom** `warning: method is_notification is never used`.

**Cause** The method was written and tested, then never wired in. The reason it
was never missed: `handle()` special-cased the one notification we expected
(`initialized`) instead of handling notifications as a class. Any *other*
notification — `{"method":"ping"}` with no id — got an answer with a null id,
which JSON-RPC 2.0 forbids.

**Fix** `handle()` now returns early for any notification, and a test asserts
it across four different methods.

**Rule** Treat dead-code warnings as questions, not noise. "Why did I write
this and never need it?" is sometimes answered by "because the thing that
should have used it is wrong".

---

## 2026-08-23 — A security test that never tested the security

**Symptom** `rejects_traversal_out_of_the_root` expected `OutsideRoot`, got
`NotFound`.

**Cause** The test asked for `nested/../../../etc/hosts`. Canonicalization
resolved that to a path that does not exist, so it failed at the *existence*
check and never reached the root-containment check. The traversal was blocked
— by the wrong guard. The test proved nothing about the guard it named.

**Fix** Rewrote it around a file that genuinely exists outside the root, so
only the root check can stop it. Kept the old case as a second, separately
named test documenting the `NotFound` path.

**Rule** A passing security test can still be testing nothing. Make sure the
check under test is the *only* thing standing between the input and the
resource — otherwise an unrelated guard can pass the test for you.

---

## 2026-08-23 — The compressor emitted Rust that would not compile

**Symptom** Compressing `guard.rs` put the `use` lines above the `//!` module
doc comment. In Rust `//!` and `#![...]` are legal only at the top of a file,
so the output was invalid source.

**Cause** Import hoisting moved every `use` to position zero without asking
what was already there.

**Fix** `imports::split_preamble` separates the leading run of `//!` / `#![` /
blank lines and re-emits it above the hoisted imports. Regression test asserts
the doc comment's index precedes the import's.

**Rule** Found by piping a real file through the built binary — no unit test
caught it, because every unit test used a fixture without a preamble. Tests
prove the cases you thought of; running the real thing on real input finds the
ones you did not.

---

## 2026-08-23 — My own 300-line rule caught me

**Symptom** `loc-guard.sh` failed: `src/main.rs 319`.

**Cause** Growth by a hundred small additions, none of which felt like the one
that crossed the line.

**Fix** Split the `compress_file` tool — its schema, execution, and output
rendering — into `src/tool.rs`, leaving `main.rs` with the event loop.

**Rule** The limit only works if something mechanical enforces it. Left to
judgment, no single edit ever looks like the problem.

---

## 2026-08-24 — A metric that measured the wrong unit

**Symptom** After fixing multi-line signatures, the measurement script still
reported "15 truncated signatures", unchanged — while the output visibly
contained the full two-line signature.

**Cause** The script counted parentheses **per line**. The first line of a
two-line signature is legitimately unbalanced; the second closes it. The same
mistake had already appeared an hour earlier in a unit test
(`every_kept_signature_has_balanced_parentheses`), which failed on correct code
for the identical reason.

**Fix** Both now accumulate depth across the block, and fail only if a
declaration is still open when the next one arrives.

**Rule** Made the same error twice in one session, in a test and then in a
metric — which is exactly the "happened twice" bar this file exists for. When
a fix appears to change nothing, suspect the measurement before the fix. And
when the unit of the thing (a signature) is bigger than the unit being counted
(a line), the count is wrong before it is even run.

---

## 2026-08-24 — String-replacement edits silently did nothing

**Symptom** A batch of edits to `signatures.rs` reported success but the
compiler still failed on the old code; the "removed" helpers were still there.

**Cause** `rustfmt` had run between writing the file and editing it, so exact
match strings no longer matched the reformatted source. Python's `str.replace`
returns the original string when nothing matches — it does not report failure.

**Fix** Re-read the file, anchored the edits on stable markers instead of
formatted bodies, and printed whether each pattern actually matched.

**Rule** Any edit-by-string-match must assert that it matched. A silent no-op
is worse than an error: it leaves you debugging a change you believe you made.

---

## 2026-08-24 — The elision marker was a syntax error in Python

**Symptom** Python output contained `    // ...` — 163 times.

**Cause** `ELISION` was a hard-coded constant. `//` is a comment in Rust and
JS, and a syntax error in Python.

**Fix** The marker now uses `Language::line_comment()`.

**Rule** Third instance of one pattern this project keeps hitting: **output
must be valid in the language it claims to be**. First the module doc pushed
below imports, then truncated signatures, then an unterminated docstring, now
a foreign comment marker. Any constant holding syntax is a language-dependent
value wearing a constant's clothes.
