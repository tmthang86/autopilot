#!/bin/sh
set -u
cd "$(dirname "$0")" || exit 1
overall=0
for t in test_*.sh; do
    [ -f "$t" ] || continue
    printf '\n%s\n' "$t"
    sh "$t" || overall=1
done
printf '\n'
if [ "$overall" -eq 0 ]; then printf 'ALL PASS\n'; else printf 'FAILURES\n'; fi
exit "$overall"
