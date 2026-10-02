#!/usr/bin/env bash
# Turns failing tests into GitHub annotations, including the full panic text.
#
# The job log is not always reachable (the log server can be blocked), but
# check-run annotations are. GitHub reads one line per `::error::` command, so
# the panic message is newline-escaped with `%0A`, which GitHub decodes back
# into a readable multi-line annotation.
#
# Usage: scripts/ci-diagnose.sh [path-to-captured-test-output]
# Without a path the suite is run again with --no-fail-fast.
set -uo pipefail

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT"

export RUST_BACKTRACE=1

if [[ $# -ge 1 && -f "$1" ]]; then
    log=$1
else
    log=$(mktemp)
    cargo test --workspace --locked --no-fail-fast >"$log" 2>&1 || true
fi

echo "--- result lines ---"
grep -E "^test result:" "$log" | sort | uniq -c || true

# Each failure block is:
#   ---- <test name> stdout ----
#   <thread ... panicked at ...>
#   <message lines>
#   ---- <test name> stdout ----
#   (or end of file)
awk '
  /^---- .* stdout ----$/ {
    if (name != "") { emit() }
    name = $2
    msg = ""
    next
  }
  name != "" { msg = msg (msg == "" ? "" : "\\n") $0 }
  END { if (name != "") emit() }
  function emit() {
    gsub(/\r/, "", msg)
    gsub(/%/, "%25", msg)
    gsub(/\n/, "%0A", msg)
    printf "::error title=failing test: %s::%s\n", name, msg
    name = ""
    msg = ""
  }
' "$log"
