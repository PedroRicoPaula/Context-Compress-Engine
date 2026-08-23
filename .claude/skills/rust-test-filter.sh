#!/usr/bin/env bash
# Test run, failures only. Strips "running 47 tests", dots, and passing names.
# Keeps: failed test names, the panic/assert line, and the final tally.
# Usage: ./.claude/skills/rust-test-filter.sh [test name filter]
set -uo pipefail
cd "$(dirname "$0")/../.." || exit 2

out=$(cargo test "$@" 2>&1)
status=$?

if [ $status -eq 0 ]; then
  echo "$out" | grep -E '^test result:' | sort -u
  echo "OK tests"
  exit 0
fi

# ---- failures ---- block holds the panic message per failing test.
echo "$out" | sed -n '/^failures:$/,/^test result:/p' \
  | grep -Ev '^\s*$|^failures:$|^---- .* stdout ----$' \
  | head -60
echo "$out" | grep -E "^(error|error\[E)" | head -20
echo "$out" | grep -E '^test result:' | sort -u
echo "FAIL tests (exit $status)"
exit $status
