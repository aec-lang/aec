#!/usr/bin/env bash
# Runs clippy and turns each diagnostic into a GitHub annotation, so a lint
# failure is readable without a log download.
#
# Usage: scripts/ci-clippy.sh
set -uo pipefail

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT"

log=$(mktemp)
cargo clippy --workspace --all-targets --locked -- -D warnings >"$log" 2>&1
status=$?

# GitHub reads one annotation per `::error::` line. Emitting every diagnostic
# line (bounded) is simpler and more robust than parsing multi-line blocks.
count=0
while IFS= read -r line; do
    case "$line" in
        warning*|error*)
            # Escape the workflow-command payload characters.
            safe=${line//'%'/'%25'}
            printf '::error title=clippy::%s\n' "$safe"
            count=$((count + 1))
            [ "$count" -ge 40 ] && break
            ;;
    esac
done < "$log"

echo "--- clippy exit status: $status (emitted $count diagnostics) ---"
rm -f "$log"
exit "$status"
