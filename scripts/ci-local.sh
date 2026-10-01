#!/usr/bin/env bash
set -euo pipefail

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT"
export CARGO_TERM_COLOR=always

if [[ "${OFFLINE:-0}" == "1" ]]; then
  cargo fetch --locked --offline
else
  cargo fetch --locked
fi
cargo check --workspace --locked
cargo test --workspace --locked --no-fail-fast
cargo clippy --workspace --all-targets --locked -- -D warnings
package_args=(--workspace --allow-dirty --no-verify --locked)
if [[ "${OFFLINE:-0}" == "1" ]]; then
  package_args+=(--offline)
fi
cargo package "${package_args[@]}"
cargo build --release -p aec-cli --locked
cargo run --release -p aec-cli --bin aec -- check examples/theme.aec
cargo run --release -p aec-cli --bin aec -- run examples/modules/main.aec --cli
cargo run --release -p aec-cli --bin aec -- check examples/nominal.aec
cargo run --release -p aec-cli --bin aec -- run examples/nominal.aec --cli
cargo run --release -p aec-cli --bin apm -- --help
if command -v bwrap >/dev/null 2>&1; then
    scripts/sandbox-linux.sh /workspace/target/release/aec check examples/nominal.aec
else
    printf '%s\n' 'bwrap unavailable; Linux OS sandbox smoke skipped'
fi
git diff --check

if [[ "${RUN_FMT:-0}" == "1" ]]; then
  cargo fmt --all -- --check
fi
