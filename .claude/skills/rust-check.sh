#!/usr/bin/env bash
# Fast type check. Emits only "file:line:col LEVEL CODE message".
# A raw `cargo check` failure is ~4k tokens of ASCII art; this is ~200.
# Usage: ./.claude/skills/rust-check.sh [extra cargo args]
set -uo pipefail
cd "$(dirname "$0")/../.." || exit 2

# The toolchain lives in ~/.cargo/bin, which non-interactive shells do not pick
# up from ~/.zshenv. Put it on PATH ourselves rather than depending on the caller.
[ -d "$HOME/.cargo/bin" ] && PATH="$HOME/.cargo/bin:$PATH"

if ! command -v cargo >/dev/null 2>&1; then
  echo "FAIL: cargo not found. Install it: see docs/ENVIRONMENT.md"
  exit 127
fi
here=".claude/skills"

out=$(cargo check --message-format=json --all-targets "$@" 2>/dev/null)
status=$?

echo "$out" | jq -r -f "$here/_cargo-json.jq" 2>/dev/null | sort -u

if [ $status -eq 0 ]; then
  echo "OK check"
else
  # Surface non-diagnostic failures (bad Cargo.toml, missing toolchain, resolver errors)
  # that never appear as compiler-message JSON.
  echo "$out" | jq -r 'select(.reason == null) | .' 2>/dev/null | head -20
  echo "FAIL check (exit $status)"
fi
exit $status
