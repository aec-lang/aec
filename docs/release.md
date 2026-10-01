# AEC release guide

AEC `0.1.0` is the first release candidate for the interpreted language, native UI runtime, and local package tooling. The release unit contains the `aec` interpreter CLI, the `apm` package manager, source crates, examples, and documentation.

## Install from a checkout

Rust and Cargo are required. From the repository root:

```sh
PREFIX="$HOME/.local" ./scripts/install.sh --offline
export PATH="$HOME/.local/bin:$PATH"
aec --version
apm --version
```

Use `--offline` only when the locked dependencies are already cached. Omit it for a normal first installation. The script installs both binaries into `$PREFIX/bin`; the default is Cargo's home directory.

The equivalent manual command is:

```sh
cargo install --path crates/aec-cli --locked --bin aec --bin apm
```

Verify a source build before installing:

```sh
cargo test --workspace --locked --no-fail-fast
cargo check --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo package --workspace --allow-dirty --no-verify --locked --offline
```

## Release artifacts

A tag matching `v*` runs `.github/workflows/release.yml`. The workflow runs the locked workspace checks on Linux, macOS, and Windows, builds release binaries, and attaches these archives:

| Platform | Archive | Contents |
|---|---|---|
| Linux x86_64 | `aec-linux-x86_64.tar.gz` | `aec`, `apm` |
| macOS x86_64 | `aec-macos-x86_64.tar.gz` | `aec`, `apm` |
| Windows x86_64 | `aec-windows-x86_64.zip` | `aec.exe`, `apm.exe` |

Every release also includes `SHA256SUMS`. Verify the downloaded archive before extracting it:

```sh
sha256sum --check SHA256SUMS
```

The release workflow provides integrity checksums and reproducible build steps. It does not claim a detached signature for the platform archives. Package payloads published through APM are a separate boundary: APM uses SHA-256 tree metadata and detached Ed25519 signatures verified with an explicitly selected trust key.

## Maintainer release procedure

1. Run the local verification commands above and inspect `git diff --check`.
2. Confirm the version in `Cargo.toml` matches the release tag.
3. Create the signed tag locally with the repository's normal signing policy.
4. Push the commit and the `vX.Y.Z` tag to `origin`.
5. Wait for the release workflow on Linux, macOS, and Windows.
6. Download `SHA256SUMS` and each archive, verify the checksums, and smoke-test `aec check`, `aec run --cli`, and `apm --help` on each target platform.
7. Publish release notes from the generated GitHub release notes and record manual visual QA separately.

## Current release boundaries

- The local APM registry is offline and directory-based. `apm install` still resolves local path dependencies; HTTP registry resolution and key rotation are post-release work.
- The language `permissions` block is an in-process cooperative guard. It is not an operating-system sandbox.
- `scripts/sandbox-linux.sh` adds a rootless bubblewrap boundary on Linux when bubblewrap is available. Native macOS and Windows isolation policies are not included.
- The checked-in chatbot is an offline echo demo. A live model call requires an explicitly configured endpoint and API key.
- Deep generic inference, asynchronous execution, A2A, and an IDE remain roadmap items outside this release's acceptance scope.
