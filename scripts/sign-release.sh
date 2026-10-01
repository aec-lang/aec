#!/usr/bin/env bash
set -euo pipefail

if [[ "$#" -lt 1 ]]; then
    printf '%s\n' 'usage: scripts/sign-release.sh <binary-or-archive> [key-id]' >&2
    exit 2
fi

TARGET="$1"
KEY_ID="${2:-}"

if ! command -v gpg >/dev/null 2>&1; then
    printf '%s\n' 'gpg is required for release signing' >&2
    exit 127
fi

if [[ ! -f "$TARGET" ]]; then
    printf '%s\n' "file not found: $TARGET" >&2
    exit 1
fi

SIG_ARGS=(--detach-sign --armor)
if [[ -n "$KEY_ID" ]]; then
    SIG_ARGS+=(--local-user "$KEY_ID")
fi

gpg "${SIG_ARGS[@]}" --output "${TARGET}.asc" "$TARGET"
sha256sum "$TARGET" > "${TARGET}.sha256"

printf '%s\n' "Signed: ${TARGET}.asc"
printf '%s\n' "Checksum: ${TARGET}.sha256"
