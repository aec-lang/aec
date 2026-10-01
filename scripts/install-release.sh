#!/usr/bin/env bash
# Installs the prebuilt `aec` and `apm` binaries from a GitHub release.
#
# This is the path for people who want to try AEC without a Rust toolchain.
# Building from source is still supported through scripts/install.sh.
set -euo pipefail

REPO=${AEC_REPO:-aec-lang/aec}
VERSION=${AEC_VERSION:-v0.1.0}
PREFIX=${PREFIX:-"$HOME/.local"}
BIN_DIR="$PREFIX/bin"

usage() {
    cat <<'USAGE'
usage: curl -fsSL https://aec-lang.org/install.sh | bash

environment:
  AEC_VERSION   release tag to install (default: v0.1.0)
  AEC_REPO      GitHub repository (default: aec-lang/aec)
  PREFIX        install prefix (default: $HOME/.local)
USAGE
}

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
    usage
    exit 0
fi

case "$(uname -s)" in
    Linux) platform=linux ;;
    Darwin) platform=macos ;;
    MINGW* | MSYS* | CYGWIN*)
        printf '%s\n' 'Windows is not supported by this installer. Download aec-windows-x86_64.zip from the release page and extract it.' >&2
        exit 1
        ;;
    *)
        printf 'unsupported platform: %s\n' "$(uname -s)" >&2
        exit 1
        ;;
esac

case "$(uname -m)" in
    x86_64 | amd64) arch=x86_64 ;;
    aarch64 | arm64) arch=aarch64 ;;
    *)
        printf 'unsupported architecture: %s\n' "$(uname -m)" >&2
        exit 1
        ;;
esac

if [[ "$platform" == linux && "$arch" == aarch64 ]]; then
    printf '%s\n' 'No aarch64 Linux release is published. Build from source with scripts/install.sh.' >&2
    exit 1
fi

archive="aec-${platform}-${arch}.tar.gz"
url="https://github.com/${REPO}/releases/download/${VERSION}/${archive}"

if ! command -v curl >/dev/null 2>&1; then
    printf '%s\n' 'curl is required to install AEC' >&2
    exit 127
fi

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

printf 'downloading %s %s (%s)\n' "$REPO" "$VERSION" "$archive"
curl -fSL --retry 3 -o "$tmp/$archive" "$url"

if command -v sha256sum >/dev/null 2>&1; then
    printf '%s\n' 'verifying SHA256SUMS'
    curl -fSL --retry 3 -o "$tmp/SHA256SUMS" \
        "https://github.com/${REPO}/releases/download/${VERSION}/SHA256SUMS"
    (cd "$tmp" && grep " ${archive}\$" SHA256SUMS | sha256sum -c -)
elif command -v shasum >/dev/null 2>&1; then
    curl -fSL --retry 3 -o "$tmp/SHA256SUMS" \
        "https://github.com/${REPO}/releases/download/${VERSION}/SHA256SUMS"
    (cd "$tmp" && grep " ${archive}\$" SHA256SUMS | shasum -a 256 -c -)
else
    printf '%s\n' 'warning: no sha256sum or shasum available, skipping checksum verification' >&2
fi

tar -xzf "$tmp/$archive" -C "$tmp"

mkdir -p "$BIN_DIR"
installed=0
for binary in aec apm; do
    source="$tmp/${binary}-${platform}-${arch}"
    if [[ ! -f "$source" ]]; then
        printf 'missing binary in archive: %s\n' "$source" >&2
        exit 1
    fi
    install -m 0755 "$source" "$BIN_DIR/$binary"
    installed=1
done

if [[ "$installed" -eq 0 ]]; then
    printf '%s\n' 'nothing was installed' >&2
    exit 1
fi

printf 'installed aec and apm into %s\n' "$BIN_DIR"

case ":$PATH:" in
    *":$BIN_DIR:"*) ;;
    *) printf 'add this to your PATH:\n  export PATH="%s:$PATH"\n' "$BIN_DIR" ;;
esac

printf '\nnext: aec run examples/chatbot.aec\n'
