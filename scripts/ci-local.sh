#!/usr/bin/env bash
set -euo pipefail

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT"
export CARGO_TERM_COLOR=always

cargo fetch --locked
cargo check --workspace --locked
cargo test --workspace --locked --no-fail-fast
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo package --workspace --allow-dirty --no-verify --locked
cargo build --release -p aec-cli --locked
cargo run --release -p aec-cli --bin aec -- check examples/theme.aec
cargo run --release -p aec-cli --bin aec -- run examples/modules/main.aec --cli
cargo run --release -p aec-cli --bin apm -- --help
git diff --check

if [[ "${RUN_FMT:-0}" == "1" ]]; then
  cargo fmt --all -- --check
fi
