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
