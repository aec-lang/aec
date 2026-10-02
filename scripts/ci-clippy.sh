#!/usr/bin/env bash
# Runs clippy on the whole workspace and turns every diagnostic into a GitHub
# annotation, so a lint failure is readable without a log download.
#
# Usage: scripts/ci-clippy.sh
set -uo pipefail

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT"

log=$(mktemp)
cargo clippy --workspace --all-targets --locked -- -D warnings >"$log" 2>&1
status=$?

# clippy prints `warning: <message>` followed by `  --> file:line:col`.
awk '
  /^(warning|error)(\[[^]]*\])?: / {
    msg = $0
    if (getline loc > 0 && loc ~ /^[[:space:]]*-->/) {
      gsub(/^[[:space:]]*-->[[:space:]]*/, "", loc)
      printf "::error title=clippy::%s (%s)\n", msg, loc
    } else {
      printf "::error title=clippy::%s\n", msg
    }
  }
' "$log" | head -40

echo "--- clippy exit status: $status ---"
grep -E "^(warning|error)" "$log" | sort | uniq -c | sort -rn | head -15

rm -f "$log"
exit "$status"
