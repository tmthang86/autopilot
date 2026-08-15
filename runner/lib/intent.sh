#!/bin/sh
# Intent binding. Every task names the documents that authorise it; the runner
# validates that pointer before spending a single token. The body of an issue
# is untrusted text from GitHub, and the runner executes with permission checks
# disabled, so containment here is a security boundary, not a style check.

# Resolves symlinks without readlink -f (not POSIX). Walks the link chain, then
# canonicalises the containing directory with pwd -P so a link pointing outside
# the project root is caught after resolution rather than by a naive prefix.
_resolve_intent() {
    _p=$1
    _n=0
    while [ -L "$_p" ]; do
        _n=$((_n + 1))
        if [ "$_n" -gt 32 ]; then
            log_error "too many symlink levels resolving: $1"
            return 1
        fi
        _t=$(readlink "$_p" 2>/dev/null) || { log_error "cannot read symlink: $_p"; return 1; }
        case "$_t" in
            /*) _p=$_t ;;
            *)  _p=$(dirname "$_p")/$_t ;;
        esac
    done
    _dir=$(dirname "$_p")
    _dir=$(cd "$_dir" 2>/dev/null && pwd -P) || { log_error "cannot resolve directory: $1"; return 1; }
    printf '%s/%s' "$_dir" "$(basename "$_p")"
}

# Reads the marker line from the issue body, validates every path it names, and
# prints the resolved absolute paths, one per line. Returns 0 only when at least
# one path was named and every path passed containment. Anything else — no
# marker line, an empty list, an absolute path, a `..` component, a path that
# resolves outside the project root, or a path that does not exist — is a
# refusal with the specific reason logged.
#
# IFS is reset immediately after the word split. This file is sourced into
# shell scripts whose later loops depend on the default IFS, and a function
# that returned while IFS was still redefined would corrupt the rest of the
# run.
queue_intent() {
    _body=$1
    _root=$2
    _marker=$(cfg_get queue.intent_marker "Intent:")
    # Distinguish "no marker line at all" from "a marker line with no paths".
    # Both truncate to an empty _line, but they are different refusals: the
    # first is a task that never named a source, the second one that named
    # nothing. wc -l counts the printed lines, including an empty replacement
    # for an "Intent:" that names nothing.
    if [ "$(printf '%s\n' "$_body" | sed -n "s/^[[:space:]]*${_marker}//p" | wc -l | tr -d ' ')" -eq 0 ]; then
        log_error "task names no intent — it must carry a '$_marker <path>' line naming the documents that authorise it"
        return 1
    fi
    _line=$(printf '%s\n' "$_body" | sed -n "s/^[[:space:]]*${_marker}[[:space:]]*//p" | head -1)
    if [ -z "$_line" ]; then
        log_error "task names no intent — the '$_marker' line lists no paths"
        return 1
    fi
    _root=$(cd "$_root" 2>/dev/null && pwd -P) || {
        log_error "cannot resolve project root: $2"
        return 1
    }
    _OIFS=$IFS
    IFS=' '
    # A `.` in the line (e.g. "Intent: .") would split into our parameter list
    # and then resolve to the project root itself, which is a directory, not a
    # file. The word split is the last point at which word separators exist;
    # everything below operates on whole paths.
    # shellcheck disable=SC2086
    set -- $_line
    IFS=$_OIFS
    _found=0
    for _p in "$@"; do
        case "$_p" in
            /*)
                log_error "intent path is absolute, refusing: $_p"
                return 1
                ;;
        esac
    # A `..` component means the named path may leave the managed tree
    # before it has even been resolved. Refuse the component itself rather
    # than resolve-then-check, so the refusal names the actual escape.
    # Padding both sides with slashes turns "a/../b", "../x", "a/..", and even
    # a bare ".." into one shape containing literal "/../", which a single
    # pattern matches. A ".." that merely appears inside a segment (e.g.
    # "foo..md") is not an escape and passes.
    case "/$_p/" in
        */../*)
            log_error "intent path traverses upward, refusing: $_p"
            return 1
            ;;
    esac
        _full="$_root/$_p"
        if [ ! -e "$_full" ]; then
            log_error "intent path does not exist: $_p"
            return 1
        fi
        if ! _resolved=$(_resolve_intent "$_full"); then
            return 1
        fi
        # The plan says a task names a document — a file. A directory is not a
        # boundary the agent can be told to read.
        if [ ! -f "$_resolved" ]; then
            log_error "intent path is not a file: $_p"
            return 1
        fi
        case "$_resolved" in
            "$_root"/*) ;;
            *)
                log_error "intent path resolves outside the project root: $_p"
                return 1
                ;;
        esac
        printf '%s\n' "$_resolved"
        _found=1
    done
    [ "$_found" -eq 1 ] || {
        log_error "task names no intent — the '$_marker' line lists no paths"
        return 1
    }
    return 0
}