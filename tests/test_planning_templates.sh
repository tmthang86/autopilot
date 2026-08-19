#!/bin/sh
. "$(dirname "$0")/harness.sh"

TEMPLATES="$REPO_ROOT/skills/autopilot-planning/templates"

# --- every template must actually be a template, not a real document copied
# in wholesale. A .tmpl with zero placeholders and byte-identical to one of
# this repo's own real docs is the file that installed itself into a target
# project, not a shape for that project to fill in. ---
for f in "$TEMPLATES"/*.tmpl; do
    name=$(basename "$f")
    # plan.md.tmpl predates the {{LIKE_THIS}} convention and uses
    # <angle-bracket> placeholders instead; both count as "has a placeholder".
    # open-items.md.tmpl is legitimately placeholder-free: its correct start
    # state is empty tables under generic prose, with nothing yet to fill in.
    case "$name" in
        open-items.md.tmpl) : ;;
        *)
            placeholders=$(grep -Ec '\{\{[A-Z_]+\}\}|<[A-Za-z][A-Za-z0-9_ ]*>' "$f")
            assert_eq "1" "$([ "$placeholders" -gt 0 ] && echo 1 || echo 0)" \
                "$name carries at least one placeholder, not fully concrete content"
            ;;
    esac

    identical=none
    for real in "$REPO_ROOT"/docs/decisions/*.md "$REPO_ROOT"/docs/design/*.md; do
        [ -f "$real" ] || continue
        if cmp -s "$f" "$real"; then
            identical=$(basename "$real")
        fi
    done
    assert_eq "none" "$identical" \
        "$name must not be byte-identical to one of this repo's own real documents"
done

finish
