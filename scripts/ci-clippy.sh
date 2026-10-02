#!/usr/bin/env bash
# Runs clippy and turns each diagnostic into a GitHub annotation, so a lint
# failure is readable without a log download (the log server can be blocked).
#
# Usage: scripts/ci-clippy.sh
set -uo pipefail

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT"

# Force plain output so diagnostics parse regardless of the runner's TTY.
export CARGO_TERM_COLOR=never

log=$(mktemp)
cargo clippy --workspace --all-targets --locked -- -D warnings >"$log" 2>&1
status=$?

emit() {
    # %25 escapes `%`, and %0A keeps a multi-line payload in one command.
    local payload=${1//'%'/'%25'}
    printf '::error title=clippy::%s\n' "$payload"
}

# Any line that carries a diagnostic is emitted; the leading whitespace clippy
# uses for context lines is trimmed so nothing is missed.
grep -nE '(^|[[:space:]])(warning|error)(\[[^]]*\])?: ' "$log" | head -40 | while IFS= read -r match; do
    emit "$match"
done

# Belt and braces: if nothing matched, dump the tail so there is still evidence.
if ! grep -qE '(^|[[:space:]])(warning|error)(\[[^]]*\])?: ' "$log"; then
    emit "no diagnostic lines matched; last output follows"
    tail -25 "$log" | while IFS= read -r line; do
        emit "$line"
    done
fi

echo "check-run annotations emitted; clippy exit status: $status"
rm -f "$log"
exit "$status"
