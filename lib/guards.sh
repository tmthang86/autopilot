#!/bin/sh
# Guards. Each answers one question: should this run proceed?
# Returning 1 is a normal, quiet outcome — not an error.

guard_stop_file() {
    if [ -e "$1/STOP" ]; then
        log_info "STOP file present, standing down"
        return 1
    fi
    return 0
}

# mkdir is atomic on every filesystem we care about, so it is the lock.
guard_lock() {
    _lock="$1/lock"
    if mkdir "$_lock" 2>/dev/null; then
        printf '%s' "$$" > "$_lock/pid"
        return 0
    fi
    _pid=$(cat "$_lock/pid" 2>/dev/null || printf '0')
    if [ "$_pid" != "0" ] && kill -0 "$_pid" 2>/dev/null; then
        log_info "another run is in progress (pid $_pid)"
        return 1
    fi
    log_warn "breaking stale lock from pid $_pid"
    rm -rf "$_lock"
    if mkdir "$_lock" 2>/dev/null; then
        printf '%s' "$$" > "$_lock/pid"
        return 0
    fi
    log_info "could not acquire lock"
    return 1
}

guard_unlock() { rm -rf "$1/lock"; }

guard_resume_after() {
    _until=$(state_get resume_after 0)
    _now=$(date +%s)
    if [ "$_until" -gt "$_now" ]; then
        log_info "usage window closed for another $(( _until - _now ))s"
        return 1
    fi
    return 0
}

guard_daily_cap() {
    _today=$(date -u +%Y-%m-%d)
    if [ "$(state_get tasks_today_date '')" != "$_today" ]; then
        state_set tasks_today_date "$_today"
        state_set_num tasks_today 0
    fi
    _cap=$(cfg_get pacing.daily_task_cap 12)
    _done=$(state_get tasks_today 0)
    if [ "$_done" -ge "$_cap" ]; then
        log_info "daily cap reached ($_done/$_cap)"
        return 1
    fi
    return 0
}

# quiet_hours entries are "HH:MM-HH:MM" in local time. Inside one means stand
# down. Leading zeros are stripped before comparison because 08 and 09 are
# invalid octal and would abort the shell's arithmetic.
#
# AUTOPILOT_NOW_HM overrides the clock. Without a seam here the tests would
# have to pick windows that happen not to contain the moment the suite runs,
# which is a test that passes 1,438 minutes a day and fails the other two.
guard_quiet_hours() {
    _now=$(printf '%s' "${AUTOPILOT_NOW_HM:-$(date +%H%M)}" | sed 's/^0*//')
    [ -z "$_now" ] && _now=0
    for _range in $(cfg_list pacing.quiet_hours); do
        _from=$(printf '%s' "$_range" | cut -d- -f1 | tr -d ':' | sed 's/^0*//')
        _to=$(printf '%s' "$_range" | cut -d- -f2 | tr -d ':' | sed 's/^0*//')
        [ -z "$_from" ] && _from=0
        [ -z "$_to" ] && _to=0
        if [ "$_from" -le "$_to" ]; then
            if [ "$_now" -ge "$_from" ] && [ "$_now" -le "$_to" ]; then
                log_info "inside quiet hours $_range"
                return 1
            fi
        else
            # The range wraps midnight, which is the common case for nights.
            if [ "$_now" -ge "$_from" ] || [ "$_now" -le "$_to" ]; then
                log_info "inside quiet hours $_range"
                return 1
            fi
        fi
    done
    return 0
}

# The project may live on a volume that is not mounted.
guard_project_present() {
    if [ ! -d "$1/.git" ]; then
        log_info "project root unavailable: $1"
        return 1
    fi
    return 0
}

guard_all() {
    _root=$1
    _dir="$_root/.autopilot"
    guard_project_present "$_root" || return 1
    guard_stop_file "$_dir"        || return 1
    guard_resume_after             || return 1
    guard_quiet_hours              || return 1
    guard_daily_cap                || return 1
    guard_lock "$_dir"             || return 1
    return 0
}
