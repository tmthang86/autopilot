#!/bin/sh
# Agent invocation and, more importantly, classification of what came back.

agent_args() {
    _mode=$(cfg_get agent.permission_mode bypassPermissions)
    case "$_mode" in
        bypassPermissions) _perm="--dangerously-skip-permissions" ;;
        *)                 _perm="--permission-mode $_mode" ;;
    esac
    printf -- '-p --output-format stream-json --verbose %s --model %s --effort %s --max-budget-usd %s --add-dir .' \
        "$_perm" "$1" "$2" "$(cfg_get agent.max_budget_usd 5.0)"
}

# The prefix above the TASK line is byte-identical on every run so the
# provider's prompt cache absorbs it. Only the task block below varies.
agent_prompt() {
    _num=$1; _title=$2; _body=$3; _branch=$4; _project=$5
    if [ ! -f "${AUTOPILOT_HOME:-}/templates/prompt.tmpl" ]; then
        # Silence here would hand the agent an empty task and let it improvise,
        # which is the one thing this whole design exists to prevent.
        log_error "prompt template not found; AUTOPILOT_HOME=[${AUTOPILOT_HOME:-unset}]"
        return 1
    fi
    sed -e "s|{{WORK_BRANCH}}|$_branch|g" \
        -e "s|{{PROJECT_NAME}}|$_project|g" \
        -e "s|{{ISSUE_NUMBER}}|$_num|g" \
        -e "s|{{ISSUE_TITLE}}|$_title|g" \
        "$AUTOPILOT_HOME/templates/prompt.tmpl"
    printf '\n%s\n' "$_body"
}

agent_run() {
    _prompt=$1; _cwd=$2; _log=$3
    # The unquoted expansion is deliberate: agent_args returns a flag list that
    # must split into separate arguments.
    # shellcheck disable=SC2046,SC2086
    ( cd "$_cwd" && claude $(agent_args "${AGENT_MODEL:-sonnet}" "${AGENT_EFFORT:-low}") "$_prompt" ) \
        > "$_log" 2>&1
}

# One trivial call. If it fails too, the account is throttled and the task is
# innocent. Deliberately empirical: the throttled payload shape is undocumented,
# so we do not parse it and cannot be broken by it changing.
agent_probe() {
    claude -p "ok" --model sonnet --tools "" --no-session-persistence >/dev/null 2>&1
}

agent_classify() {
    _log=$1; _rc=$2

    if [ ! -f "$_log" ]; then
        if [ "$_rc" -eq 0 ]; then printf 'ok'; else printf 'task_failure'; fi
        return 0
    fi

    # A rate_limit_event whose status is anything but "allowed" is decisive on
    # its own — no probe needed, no payload parsing beyond the status.
    if grep -q '"rate_limit_info"' "$_log" 2>/dev/null &&
       ! grep -q '"status":"allowed"' "$_log" 2>/dev/null; then
        printf 'usage_limit'
        return 0
    fi

    if [ "$_rc" -eq 0 ] && ! grep -q '"is_error":true' "$_log" 2>/dev/null; then
        printf 'ok'
        return 0
    fi

    if agent_probe; then
        printf 'task_failure'
    else
        log_warn "probe failed — attributing to the usage window, not to the task"
        printf 'usage_limit'
    fi
}
