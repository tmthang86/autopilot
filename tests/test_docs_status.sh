#!/bin/sh
. "$(dirname "$0")/harness.sh"

MAP="$REPO_ROOT/docs/README.md"

assert_eq "1" "$([ -f "$MAP" ] && echo 1 || echo 0)" \
    "the documentation map exists"

assert_eq "1" "$([ -f "$REPO_ROOT/docs/product/open-items.md" ] && echo 1 || echo 0)" \
    "open-items.md exists"

# Every document under docs/ must appear in the status table. A document nobody
# listed is a document nobody maintains. Redirected, never piped: a pipeline
# runs the loop in a subshell and the count is lost on exit.
missing=""
while IFS= read -r f; do
    rel=${f#"$REPO_ROOT"/docs/}
    case "$rel" in
        README.md|*/_template.md) continue ;;
    esac
    case "$(cat "$MAP")" in
        *"$rel"*) ;;
        *) missing="$missing $rel" ;;
    esac
done <<EOF
$(find "$REPO_ROOT/docs" -name '*.md' | sort)
EOF

assert_eq "" "$missing" "every document under docs/ appears in the status table"

finish
