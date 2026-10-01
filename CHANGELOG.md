# Changelog

All notable changes to AEC are recorded here. The project follows a pre-1.0 versioning policy until the language and security boundaries stabilize.

## Unreleased

### Added

- `scripts/install-release.sh` — installs `aec` and `apm` from a GitHub release and verifies the archive against `SHA256SUMS`, so nobody needs a Rust toolchain to try the language.
- `scripts/sandbox-selftest.sh` — runs the OS sandbox for the current host and reports `verified`, `untested`, or `failed` per platform instead of assuming coverage.
- `scripts/sandbox-macos.sh` — a `sandbox-exec` profile that denies by default and allows system reads, workspace reads, the command's own directory, and temporary writes.
- A documentation website generated from `docs/` and `README.md` (`scripts/build-site.cjs`), with a registry page built from a real APM directory (`scripts/build-registry.cjs`) and `scripts/check-site.py` for link, nesting, and search-index validation.
- GitHub Pages deployment for that site, and a release workflow that signs every archive when `GPG_RELEASE_KEY` is configured.
- `LICENSE-MIT` and `LICENSE-APACHE`.

### Fixed

- `scripts/sandbox-linux.sh` could not start an installed `aec`, because the binary lives outside the repository and was not part of the workspace bind. The command's directory is now bound read-only and resolved through `PATH`.
- `scripts/sandbox-linux.sh` rejected absolute paths inside the repository, since the workspace is mounted at `/workspace`. Arguments under the repository root are now rewritten to the path the sandboxed program sees.
- `scripts/sandbox-windows.ps1` ran the program in a PowerShell job, which is not a security boundary. It now emits a `.wsb` configuration for Windows Sandbox and refuses the in-process form.
- Fixed two `cargo clippy -D warnings` failures that CI would have rejected: a `print_literal` and a `collapsible_if`.

### Changed

- The roadmap now separates *verified* from *written but not executed*, and reports roughly 95% instead of claiming 100%. The earlier figure counted unexecuted macOS and Windows sandbox code, GPG signing that had no key, and native visual QA that was never performed.
- `SECURITY.md` records which sandbox is verified on which host, and the confirmed Linux behavior: reads outside the workspace fail, writes into it fail with `EROFS`.

## 0.1.0 — 2026-09-25

### Added

- Dedicated lexer/parser, AST, static checker, tree-walking runtime, native egui UI, and `aec` CLI.
- `let`/`var`, interpolation, `Result`/`?`, closures, function types, default/named arguments, and type aliases.
- Multi-file imports with relative resolution, cycles and duplicate-declaration checks, `pub`, private/public namespaces, and nested module aliases.
- Nominal `struct` and `enum` values with explicit constructors, field/variant access, equality, and pattern matching.
- Native UI state synchronization, reactive rebuilds, component and loop scopes, event arguments, themes, RTL text, and the offline chatbot example.
- Built-ins for collections, files, HTTP/JSON, time, math, regex, crypto, UUID, models, and SQLite-backed conversation memory.
- Cooperative `permissions` and `limits`, plus a Linux bubblewrap smoke boundary.
- `apm` local path package management and an offline, immutable, Ed25519-signed local registry.
- Locked cross-platform CI, release packaging workflow, checksums, and a source-checkout installer.

### Known boundaries

- No HTTP APM registry client, runtime registry resolution, or key rotation.
- The macOS and Windows sandbox scripts exist but are not yet verified on their own hosts.
- No generic type inference, asynchronous runtime, A2A protocol, or IDE.
- The chatbot example is offline echo behavior, not a live model demonstration.
