#!/bin/sh
. "$(dirname "$0")/harness.sh"

assert_eq "1" "$([ -f "$REPO_ROOT/AGENTS.md" ] && echo 1 || echo 0)" \
    "AGENTS.md exists"

assert_contains "$(cat "$REPO_ROOT/AGENTS.md")" "CLAUDE.md" \
    "AGENTS.md names CLAUDE.md"

# A pointer that grows rules is a second source of truth.
assert_eq "1" \
    "$([ "$(wc -l < "$REPO_ROOT/AGENTS.md")" -le 12 ] && echo 1 || echo 0)" \
    "AGENTS.md is a pointer, not a copy"

# The sync table is what keeps documents true. It must live in the canonical file.
assert_contains "$(cat "$REPO_ROOT/CLAUDE.md")" "you must update" \
    "CLAUDE.md carries the binding sync table"

# Rule Zero survives the language change; losing it would be silent.
assert_contains "$(cat "$REPO_ROOT/CLAUDE.md")" "runs unsupervised" \
    "CLAUDE.md still carries Rule Zero"

finish
