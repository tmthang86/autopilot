#!/bin/sh
# Agent invocation and, more importantly, classification of what came back.

agent_args() {
    _mode=$(cfg_get agent.permission_mode bypassPermissions)
    case "$_mode" in
        bypassPermissions) _perm="--dangerously-skip-permissions" ;;
        *)                 _perm="--permission-mode $_mode" ;;
    esac
    # --add-dir takes a variadic list, so it must not be last: a positional
    # prompt placed after it is swallowed as another directory, and the CLI then
    # reports it received no input at all.
    printf -- '-p --add-dir . --output-format stream-json --verbose %s --model %s --effort %s --max-budget-usd %s' \
        "$_perm" "$1" "$2" "$(cfg_get agent.max_budget_usd 5.0)"
}

# The prefix above the TASK line is byte-identical on every run so the
# provider's prompt cache absorbs it. Only the task block below varies.
agent_prompt() {
    _num=$1; _title=$2; _body=$3; _branch=$4; _project=$5; _intent=${6:-}
    if [ ! -f "${AUTOPILOT_HOME:-}/templates/prompt.tmpl" ]; then
        # Silence here would hand the agent an empty task and let it improvise,
        # which is the one thing this whole design exists to prevent.
        log_error "prompt template not found; AUTOPILOT_HOME=[${AUTOPILOT_HOME:-unset}]"
        return 1
    fi
    # The intent list lives below the --- TASK --- divider, in the varying task
    # block. The prefix above it stays byte-identical so the provider's prompt
    # cache absorbs it; dropping a path into the prefix would break every cache
    # hit for every subsequent run. _intent is given one path per line (the
    # output of queue_intent); indent each line under the header.
    if [ -n "$_intent" ]; then
        _intent_block=$(printf '%s\n' "$_intent" | sed 's/^/  /')
    else
        _intent_block=""
    fi
    sed -e "s|{{WORK_BRANCH}}|$_branch|g" \
        -e "s|{{PROJECT_NAME}}|$_project|g" \
        -e "s|{{ISSUE_NUMBER}}|$_num|g" \
        -e "s|{{ISSUE_TITLE}}|$_title|g" \
        -e "s|{{INTENT_FILES}}|$_intent_block|g" \
        "$AUTOPILOT_HOME/templates/prompt.tmpl"
    printf '\n%s\n' "$_body"
}

# The prompt goes in on stdin rather than as an argument. A task description is
# arbitrary text — it can begin with a dash, contain anything, and be long — and
# stdin is immune to all of that as well as to flag ordering.
agent_run() {
    _prompt=$1; _cwd=$2; _log=$3
    # The unquoted expansion is deliberate: agent_args returns a flag list that
    # must split into separate arguments.
    # shellcheck disable=SC2046,SC2086
    ( cd "$_cwd" && printf '%s' "$_prompt" \
        | claude $(agent_args "${AGENT_MODEL:-sonnet}" "${AGENT_EFFORT:-low}") ) \
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

    # Throttling announces itself with its own event type. Checking the status
    # only on those lines keeps an unrelated "allowed" elsewhere in the stream
    # from masking a real rejection.
    if grep '"type":"rate_limit_event"' "$_log" 2>/dev/null \
        | grep -qv '"status":"allowed"'; then
        printf 'usage_limit'
        return 0
    fi

    # Only the final result event decides the outcome. Scanning the whole stream
    # for is_error:true also catches tool_result entries, where it means one
    # shell command exited non-zero — an ordinary event inside a completely
    # successful task. Treating that as failure discards finished work.
    _final=$(grep '"type":"result"' "$_log" 2>/dev/null | tail -1)
    if [ -n "$_final" ]; then
        # Not `.is_error // true`: jq's alternative operator treats false the
        # same as null, so that expression reports every successful run as an
        # error. The same trap bit state.sh reading a stored zero.
        _err=$(printf '%s' "$_final" | jq -r 'if .is_error == false then "false" else "true" end' 2>/dev/null)
        if [ "$_rc" -eq 0 ] && [ "$_err" = "false" ]; then
            printf 'ok'
            return 0
        fi
    elif [ "$_rc" -eq 0 ]; then
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
