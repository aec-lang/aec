#!/usr/bin/env bash
set -euo pipefail

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
if [[ "$#" -eq 0 ]]; then
    printf '%s\n' 'usage: scripts/sandbox-macos.sh command [args...]' >&2
    exit 2
fi
if ! command -v sandbox-exec >/dev/null 2>&1; then
    printf '%s\n' 'sandbox-exec is required for the macOS OS sandbox' >&2
    exit 127
fi

# An installed binary lives outside the repository. The profile below only reads
# the workspace, so the directory containing the command must be allowed too.
command_path=$(command -v -- "$1" 2>/dev/null || true)
if [[ -z "$command_path" ]]; then
    command_path="$1"
fi
if [[ -x "$command_path" ]]; then
    command_path=$(CDPATH= cd -- "$(dirname -- "$command_path")" && pwd)/$(basename -- "$command_path")
    command_dir=$(dirname -- "$command_path")
    set -- "$command_path" "${@:2}"
else
    command_dir=""
fi

PROFILE=$(mktemp -t aec-sandbox.XXXXXX.sb)
trap 'rm -f "$PROFILE"' EXIT

{
    cat <<SB
(version 1)
(deny default)
(allow process-exec (subpath "/usr"))
(allow process-exec (subpath "/bin"))
(allow file-read* (subpath "/usr"))
(allow file-read* (subpath "/System"))
(allow file-read* (subpath "/Library/Preferences"))
(allow file-read* (subpath "/private/etc"))
(allow file-read* (subpath "$ROOT"))
(allow file-read-metadata)
(allow file-write* (subpath "/private/var/folders/"))
(allow file-write* (subpath "/private/tmp"))
(allow mach-lookup (global-name "com.apple.launchd.per-user.$(id -u)"))
(allow signal (target same-sandbox))
(allow sysctl-read)
(allow process-info*)
(allow iokit-open (iokit-user-client-class "IOHIDParamUserClient"))
SB
    if [[ -n "$command_dir" && "$command_dir" != "$ROOT" ]]; then
        printf '(allow file-read* (subpath "%s"))\n' "$command_dir"
    fi
} > "$PROFILE"

exec sandbox-exec -f "$PROFILE" "$@"