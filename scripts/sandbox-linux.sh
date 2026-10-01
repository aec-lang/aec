#!/usr/bin/env bash
set -euo pipefail

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
MOUNT=/workspace
if ! command -v bwrap >/dev/null 2>&1; then
    printf '%s\n' 'bwrap is required for the Linux OS sandbox' >&2
    exit 127
fi
if [[ "$#" -eq 0 ]]; then
    printf '%s\n' 'usage: scripts/sandbox-linux.sh command [args...]' >&2
    exit 2
fi

binds=(--ro-bind /usr /usr --ro-bind /bin /bin --ro-bind /lib /lib --ro-bind /lib64 /lib64 --ro-bind /etc /etc)
for path in /lib32 /usr/local; do
    if [[ -e "$path" ]]; then
        binds+=(--ro-bind "$path" "$path")
    fi
done

# An installed binary lives outside the repository, so its directory is not part
# of the workspace bind and would be unreachable. Resolve the command to an
# absolute path first and expose the directory that contains it read-only.
command_path=$(command -v -- "$1" 2>/dev/null || true)
if [[ -z "$command_path" ]]; then
    command_path="$1"
fi
if [[ -x "$command_path" ]]; then
    command_path=$(CDPATH= cd -- "$(dirname -- "$command_path")" && pwd)/$(basename -- "$command_path")
    command_dir=$(dirname -- "$command_path")
    if [[ "$command_dir" != "$ROOT" ]]; then
        binds+=(--ro-bind "$command_dir" "$command_dir")
    fi
    set -- "$command_path" "${@:2}"
fi

# The workspace is mounted at $MOUNT, so any argument that points inside the
# repository has to be rewritten to the path the sandboxed program will see.
# Without this, `sandbox-linux.sh "$PWD/target/release/aec" check "$PWD/x.aec"`
# fails with "No such file or directory" even though the file is readable.
translated=()
for arg in "$@"; do
    case "$arg" in
        "$ROOT")
            translated+=("$MOUNT")
            ;;
        "$ROOT"/*)
            translated+=("$MOUNT/${arg#"$ROOT"/}")
            ;;
        *)
            translated+=("$arg")
            ;;
    esac
done
set -- "${translated[@]}"

exec bwrap \
    --die-with-parent \
    --new-session \
    --unshare-user \
    --unshare-pid \
    --unshare-net \
    --unshare-ipc \
    --unshare-uts \
    --cap-drop ALL \
    --clearenv \
    --setenv PATH /usr/bin:/bin \
    --setenv HOME /tmp \
    --setenv TMPDIR /tmp \
    --tmpfs /tmp \
    --tmpfs /home \
    --tmpfs /root \
    --tmpfs /var/tmp \
    --tmpfs /run \
    --proc /proc \
    --dev /dev \
    --tmpfs /dev/shm \
    "${binds[@]}" \
    --ro-bind "$ROOT" "$MOUNT" \
    --chdir "$MOUNT" \
    -- "$@"
