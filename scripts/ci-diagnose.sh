#!/usr/bin/env bash
# Runs the workspace test suite and turns every failure into a GitHub
# annotation, so the exact test name and panic message are readable from the
# run page instead of being buried in a log download.
#
# Usage: scripts/ci-diagnose.sh
set -uo pipefail

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT"

export RUST_BACKTRACE=1

log=$(mktemp)
cargo test --workspace --locked --no-fail-fast >"$log" 2>&1
status=$?

echo "cargo test exit status: $status"
echo "--- failing tests ---"

awk '
  /^---- .* stdout ----$/ {
    name = $2
    getline; getline
    msg = $0
    getline more
    if (more ~ /^[[:space:]]/) msg = msg " " more
    printf "%s\t%s\n", name, msg
  }
' "$log" | while IFS=$'\t' read -r name message; do
  printf '::error title=failing test::%s — %s\n' "$name" "$message"
done

echo "--- result lines ---"
grep -E "^test result:" "$log" | sort | uniq -c

echo "--- first panic block ---"
grep -n "panicked at" -A 6 "$log" | head -40

rm -f "$log"
exit "$status"
