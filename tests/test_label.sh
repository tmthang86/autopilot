#!/bin/sh
. "$(dirname "$0")/harness.sh"
. "$RUNNER_ROOT/lib/label.sh"

mk() {
    mkdir -p "$1"
    git -C "$1" init -q
    [ -n "${2:-}" ] && git -C "$1" remote add origin "$2"
    return 0
}

# The defect this file exists to fix: same directory name, different projects.
mk "$TEST_TMP/work/api"  "https://github.com/acme/api.git"
mk "$TEST_TMP/side/api"  "git@github.com:other/api.git"

assert_eq "com.autopilot.acme-api" "$(label_for_project "$TEST_TMP/work/api")" \
    "an https remote yields owner-repo"
assert_eq "com.autopilot.other-api" "$(label_for_project "$TEST_TMP/side/api")" \
    "an ssh remote yields owner-repo"

# repo_slug_for_project is the shared parse label_for_project builds on top of.
assert_eq "acme/api" "$(repo_slug_for_project "$TEST_TMP/work/api")" \
    "the slug uses / as its separator, unlike the label"
if repo_slug_for_project "$TEST_TMP/work/api" >/dev/null 2>&1; then rc=0; else rc=1; fi
assert_eq "0" "$rc" "the slug reports success when origin parses"

# A project with no remote still has to be distinguishable from another one
# with the same directory name, or the collision simply moves.
mk "$TEST_TMP/one/local"
mk "$TEST_TMP/two/local"
a=$(label_for_project "$TEST_TMP/one/local")
b=$(label_for_project "$TEST_TMP/two/local")

assert_contains "$a" "com.autopilot.local-" "no remote falls back to the directory name"
assert_eq "0" "$([ "$a" = "$b" ] && echo 1 || echo 0)" \
    "two remoteless projects with one name get different labels"

# The no-remote case must fail rather than print something misleading.
out=$(repo_slug_for_project "$TEST_TMP/one/local" 2>/dev/null)
if repo_slug_for_project "$TEST_TMP/one/local" >/dev/null 2>&1; then rc=0; else rc=1; fi
assert_eq "1" "$rc" "the slug fails when there is no origin remote"
assert_eq "" "$out" "the slug prints nothing when there is no origin remote"

finish
