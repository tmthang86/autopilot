#!/bin/sh
# The queue. This is the only file that knows GitHub exists.

QUEUE_CACHE=""

queue_candidates() {
    _label=$(cfg_get queue.ready_label autopilot)
    gh issue list --state open --limit 50 \
        --json number,title,body,labels,milestone \
        --label "$_label" 2>/dev/null || printf '[]'
}

# Only the configured phrase counts. A bare "#7" is a cross-reference, not a
# dependency, and treating it as one would stall work behind unrelated issues.
queue_deps() {
    printf '%s\n' "$1" | sed -n 's/.*[Dd]epends [Oo]n #\([0-9][0-9]*\).*/\1/p'
}

# Only an explicit CLOSED releases a dependent issue. Anything else — an open
# issue, a gh failure, an unrecognised payload — leaves the dependent blocked.
# The other direction fails open: a transient network error would let work run
# before the thing it depends on exists, and the damage lands in a commit.
queue_is_closed() {
    _state=$(gh issue view "$1" --json state -q .state 2>/dev/null || printf 'UNKNOWN')
    case "$_state" in
        CLOSED) return 0 ;;
        OPEN)   return 1 ;;
        *)
            log_warn "unrecognised state for #$1 ([$_state]); treating as still open"
            return 1
            ;;
    esac
}

queue_pick() {
    QUEUE_CACHE=$(queue_candidates)
    printf '%s' "$QUEUE_CACHE" | jq -e 'type == "array"' >/dev/null 2>&1 || QUEUE_CACHE='[]'

    _excluded=$(cfg_list queue.exclude_labels | tr '\n' ' ')
    _count=$(printf '%s' "$QUEUE_CACHE" | jq 'length')
    _i=0
    while [ "$_i" -lt "$_count" ]; do
        _num=$(printf '%s' "$QUEUE_CACHE"  | jq -r ".[$_i].number")
        _body=$(printf '%s' "$QUEUE_CACHE" | jq -r ".[$_i].body // \"\"")
        _labels=$(printf '%s' "$QUEUE_CACHE" | jq -r ".[$_i].labels[]?.name" 2>/dev/null)
        _i=$((_i + 1))

        _skip=0
        for _ex in $_excluded; do
            for _lb in $_labels; do
                [ "$_lb" = "$_ex" ] && _skip=1
            done
        done
        [ "$_skip" -eq 1 ] && continue

        _blocked=0
        for _dep in $(queue_deps "$_body"); do
            queue_is_closed "$_dep" || _blocked=1
        done
        [ "$_blocked" -eq 1 ] && continue

        printf '%s' "$_num"
        return 0
    done
    return 0
}

queue_field() {
    printf '%s' "$QUEUE_CACHE" | jq -r ".[] | select(.number == $1) | .$2 // \"\""
}

# Reads the issue's actual labels. Never search the title or body for a label
# name — an issue whose body merely mentions "needs-human" is not labelled.
queue_has_label() {
    printf '%s' "$QUEUE_CACHE" \
        | jq -e --arg l "$2" ".[] | select(.number == $1) | .labels[]? | select(.name == \$l)" \
          >/dev/null 2>&1
}

queue_claim()   { gh issue edit "$1" --add-label "status:in-progress" >/dev/null 2>&1; }
queue_release() { gh issue edit "$1" --remove-label "status:in-progress" >/dev/null 2>&1; }
