#!/bin/sh
# The launchd job label. install-project.sh writes a plist under this name and
# ctl.sh manages the job under it; when the two disagree, ctl.sh quietly
# operates on a job the installer never wrote.

# Derived from the origin remote, because a basename alone collides:
# ~/work/api and ~/side/api produce one label, and the second install
# overwrites the first's plist with no warning.

# Prints <owner>/<repo> from the project's origin remote and returns 0, or
# prints nothing and returns non-zero when there is no origin, or it does not
# parse into owner/repo. lib/queue.sh:12-24 parses the same remote for the same
# reason (gh needs an explicit --repo); this is the one other caller of that
# parse, kept here so a third copy does not appear.
repo_slug_for_project() {
    _dir=$1
    _url=$(git -C "$_dir" remote get-url origin 2>/dev/null || printf '')
    [ -n "$_url" ] || return 1
    _slug=$(printf '%s' "$_url" \
        | sed -e 's|^git@[^:]*:||' -e 's|^https\{0,1\}://[^/]*/||' -e 's|\.git$||')
    case "$_slug" in
        */*) printf '%s' "$_slug"; return 0 ;;
        *) return 1 ;;
    esac
}

# Prints the launchd label on stdout and returns 0. Format is
# com.autopilot.<owner>-<repo> when an origin remote parses, and
# com.autopilot.<dirname>-<digest> otherwise.
label_for_project() {
    _dir=$1
    if _slug=$(repo_slug_for_project "$_dir"); then
        printf 'com.autopilot.%s' "$(printf '%s' "$_slug" | tr '/' '-')"
        return 0
    fi
    # No remote, or an origin this cannot parse. The directory name alone would
    # reintroduce the collision, so a digest of the absolute path joins it.
    # cksum is POSIX; shasum and md5 are not portable enough to rely on.
    _abs=$(cd "$_dir" && pwd)
    _digest=$(printf '%s' "$_abs" | cksum | cut -d' ' -f1)
    printf 'com.autopilot.%s-%s' "$(basename "$_abs")" "$_digest"
}
