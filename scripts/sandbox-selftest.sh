#!/usr/bin/env bash
# Runs the AEC OS sandbox for this host and reports what was actually verified.
#
# The sandbox is a real boundary, but only the host's own platform can prove it.
# This script exercises the matching script and reports the others as untested
# rather than claiming coverage it does not have.
set -euo pipefail

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
AEC="$ROOT/target/release/aec"
SAMPLE="$ROOT/examples/nominal.aec"

if [[ ! -x "$AEC" ]]; then
    printf '%s\n' "build the release binary first: cargo build --release -p aec-cli" >&2
    exit 1
fi

status=0

report() {
    local platform=$1 state=$2 detail=$3
    printf '%-10s %-9s %s\n' "$platform" "$state" "$detail"
    [[ "$state" == verified ]] || status=1
}

case "$(uname -s)" in
    Linux)
        if command -v bwrap >/dev/null 2>&1; then
            if "$ROOT/scripts/sandbox-linux.sh" "$AEC" check "$SAMPLE" >/dev/null 2>&1; then
                report linux verified 'bubblewrap boundary ran aec check successfully'
            else
                report linux failed 'bubblewrap boundary rejected the program'
            fi
        else
            report linux skipped 'bwrap is not installed on this host'
        fi
        report macos untested 'requires a macOS host to verify'
        report windows untested 'requires a Windows host to verify'
        ;;
    Darwin)
        if command -v sandbox-exec >/dev/null 2>&1; then
            if "$ROOT/scripts/sandbox-macos.sh" "$AEC" check "$SAMPLE" >/dev/null 2>&1; then
                report macos verified 'sandbox-exec profile ran aec check successfully'
            else
                report macos failed 'sandbox-exec profile rejected the program'
            fi
        else
            report macos skipped 'sandbox-exec is unavailable on this host'
        fi
        report linux untested 'requires a Linux host to verify'
        report windows untested 'requires a Windows host to verify'
        ;;
    MINGW* | MSYS* | CYGWIN*)
        if command -f pwsh >/dev/null 2>&1; then
            if pwsh -NoProfile -File "$ROOT/scripts/sandbox-windows.ps1" -EmitConfig -ConfigFile "$ROOT/aec-sandbox.wsb" >/dev/null 2>&1; then
                report windows verified 'generated a .wsb Windows Sandbox configuration'
            else
                report windows failed 'could not generate the .wsb configuration'
            fi
            rm -f "$ROOT/aec-sandbox.wsb"
        else
            report windows skipped 'pwsh is not installed on this host'
        fi
        report linux untested 'requires a Linux host to verify'
        report macos untested 'requires a macOS host to verify'
        ;;
    *)
        printf 'unsupported host: %s\n' "$(uname -s)" >&2
        exit 2
        ;;
esac

exit "$status"
