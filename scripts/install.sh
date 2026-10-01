#!/usr/bin/env bash
set -euo pipefail

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
CARGO_ROOT=${CARGO_HOME:-"$HOME/.cargo"}
PREFIX=${PREFIX:-"$CARGO_ROOT"}

if ! command -v cargo >/dev/null 2>&1; then
    printf '%s\n' 'cargo is required to install AEC' >&2
    exit 1
fi

mkdir -p "$PREFIX"
cargo install \
    --path "$ROOT/crates/aec-cli" \
    --locked \
    --bin aec \
    --bin apm \
    --root "$PREFIX" \
    --force \
    "$@"

printf 'installed aec and apm under %s/bin\n' "$PREFIX"
