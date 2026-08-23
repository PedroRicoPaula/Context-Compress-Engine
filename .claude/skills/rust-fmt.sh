#!/usr/bin/env bash
# Format in place. Silent on success -> zero tokens in the common case.
# Pass --check to only report which files are unformatted (no diff dump).
set -uo pipefail
cd "$(dirname "$0")/../.." || exit 2

# The toolchain lives in ~/.cargo/bin, which non-interactive shells do not pick
# up from ~/.zshenv. Put it on PATH ourselves rather than depending on the caller.
[ -d "$HOME/.cargo/bin" ] && PATH="$HOME/.cargo/bin:$PATH"

if ! command -v cargo >/dev/null 2>&1; then
  echo "FAIL: cargo not found. Install it: see docs/ENVIRONMENT.md"
  exit 127
fi

if [ "${1:-}" = "--check" ]; then
  cargo fmt -- --check --files-with-diff >/dev/null 2>&1 && { echo "OK fmt"; exit 0; }
  cargo fmt -- --check -l 2>/dev/null
  echo "FAIL fmt (run ./.claude/skills/rust-fmt.sh)"
  exit 1
fi

cargo fmt "$@" 2>&1 | head -20
echo "OK fmt"
