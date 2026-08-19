#!/bin/sh
. "$(dirname "$0")/harness.sh"

pj="$REPO_ROOT/.claude-plugin/plugin.json"
mj="$REPO_ROOT/.claude-plugin/marketplace.json"

assert_eq "1" "$([ -f "$pj" ] && echo 1 || echo 0)" "the plugin manifest exists"
assert_eq "1" "$([ -f "$mj" ] && echo 1 || echo 0)" "the marketplace manifest exists"

assert_eq "0" "$(jq -e . "$pj" >/dev/null 2>&1; echo $?)" "the plugin manifest is valid JSON"
assert_eq "0" "$(jq -e . "$mj" >/dev/null 2>&1; echo $?)" "the marketplace manifest is valid JSON"

assert_eq "autopilot" "$(jq -r .name "$pj")" "the plugin is named autopilot"

# One version, or the deployed copy and the plugin disagree about what it is.
assert_eq "$(cat "$RUNNER_ROOT/VERSION")" "$(jq -r .version "$pj")" \
    "plugin.json agrees with runner/VERSION"
assert_eq "$(cat "$RUNNER_ROOT/VERSION")" \
    "$(jq -r '.plugins[0].version' "$mj")" \
    "marketplace.json agrees with runner/VERSION"

for s in autopilot-planning autopilot-deliver autopilot-review; do
    assert_eq "1" "$([ -f "$REPO_ROOT/skills/$s/SKILL.md" ] && echo 1 || echo 0)" \
        "the $s skill exists"
    assert_contains "$(head -3 "$REPO_ROOT/skills/$s/SKILL.md")" "name: $s" \
        "$s declares its name in frontmatter"
done

finish
