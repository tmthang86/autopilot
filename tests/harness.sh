#!/bin/sh
# Test harness. Source this at the top of every tests/test_*.sh.
set -u

TESTS_RUN=0
TESTS_FAILED=0
REPO_ROOT=$(cd "$(dirname "$0")/.." && pwd)
RUNNER_ROOT="$REPO_ROOT/runner"
export REPO_ROOT RUNNER_ROOT

# Every test gets its own scratch dir, removed on exit.
TEST_TMP=$(mktemp -d "${TMPDIR:-/tmp}/autopilot-test.XXXXXX")
export TEST_TMP
trap 'rm -rf "$TEST_TMP"' EXIT

# Stubs live here and take precedence over real binaries.
STUB_BIN="$TEST_TMP/stub-bin"
mkdir -p "$STUB_BIN"
PATH="$STUB_BIN:$PATH"
export PATH

assert_eq() {
    TESTS_RUN=$((TESTS_RUN + 1))
    if [ "$1" = "$2" ]; then
        printf '  ok   %s\n' "$3"
    else
        TESTS_FAILED=$((TESTS_FAILED + 1))
        printf '  FAIL %s\n       expected: [%s]\n       actual:   [%s]\n' "$3" "$1" "$2"
    fi
}

assert_contains() {
    TESTS_RUN=$((TESTS_RUN + 1))
    case "$1" in
        *"$2"*) printf '  ok   %s\n' "$3" ;;
        *)
            TESTS_FAILED=$((TESTS_FAILED + 1))
            printf '  FAIL %s\n       missing:  [%s]\n       in:       [%s]\n' "$3" "$2" "$1"
            ;;
    esac
}

# Build a real git repo. Git behaviour is what we depend on, so we never mock it.
make_repo() {
    _dir=$(mktemp -d "$TEST_TMP/repo.XXXXXX")
    git -C "$_dir" init -q
    git -C "$_dir" config user.name "Test"
    git -C "$_dir" config user.email "test@example.com"
    git -C "$_dir" checkout -q -b main 2>/dev/null || true
    echo "seed" > "$_dir/seed.txt"
    git -C "$_dir" add -A
    git -C "$_dir" commit -q -m "seed"
    # Canonicalised: TMPDIR often ends in a slash, and anything that resolves
    # the path will normalise the double slash away, breaking string compares.
    (cd "$_dir" && pwd)
}

# stub_bin gh 'echo {}'  → a fake `gh` that prints {}
stub_bin() {
    printf '#!/bin/sh\n%s\n' "$2" > "$STUB_BIN/$1"
    chmod +x "$STUB_BIN/$1"
}

finish() {
    printf '\n  %d run, %d failed\n' "$TESTS_RUN" "$TESTS_FAILED"
    [ "$TESTS_FAILED" -eq 0 ] || exit 1
    exit 0
}
