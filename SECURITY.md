# Security policy

## Supported release

Security fixes are provided for the current `0.1.x` release line. The project is a pre-1.0 interpreted language and should be treated as an early release.

## Reporting a vulnerability

Report suspected vulnerabilities through the repository's private security-advisory or vulnerability-reporting channel. Do not put API keys, signing keys, personal data, or exploit payloads in a public issue. Include the affected version, platform, minimal reproduction, impact, and whether the issue crosses the interpreter's permission boundary.

## Security boundaries

- A `permissions` block uses default-deny checks inside the interpreter for network, filesystem, system, environment, memory, and shell capabilities. It is cooperative and is not a complete process or operating-system isolation boundary.
- Without a `permissions` block, legacy built-ins retain their unrestricted behavior. Programs handling untrusted input should declare a policy and should be run inside an appropriate external boundary.
- Platform-specific OS sandbox scripts wrap a program in a real host boundary. Each one is verified only on its own platform; `scripts/sandbox-selftest.sh` reports `verified`, `untested`, or `failed` per platform instead of assuming coverage.
  - **Linux — verified:** `scripts/sandbox-linux.sh` runs the program under rootless `bubblewrap` with a read-only workspace bind, private tmpfs directories, an unshared network namespace, dropped capabilities, and a cleared environment. Confirmed behavior: reads outside the workspace fail, writes into the workspace fail with `EROFS`, and only the system paths plus the repository are reachable.
  - **macOS — not verified here:** `scripts/sandbox-macos.sh` builds a `sandbox-exec` profile that denies by default and allows system reads, workspace reads, the command's own directory, and temporary writes. It needs a macOS host to confirm.
  - **Windows — not verified here:** `scripts/sandbox-windows.ps1` emits a `.wsb` configuration for Windows Sandbox, a Hyper-V VM with a clean image that receives the repository as a read-only mapped folder. Running the program in a PowerShell job was deliberately rejected: a job is not a security boundary. On editions without Windows Sandbox, use AppContainer or a WDAC policy.
  These wrappers complement the interpreter's internal permission checks; they do not replace them.
- Redirects are not followed for restricted HTTP and model requests. The live model adapter is blocking and requires an explicitly configured endpoint and key.
- APM local registry publication binds payload files with SHA-256 metadata and detached Ed25519 signatures. Verification requires an explicit trust key; a public key embedded in registry metadata is not sufficient by itself.
- Release archives include `SHA256SUMS`. The GitHub Actions release workflow automatically signs all artifacts (`.tar.gz`, `.zip`) with a detached GPG signature (`.asc`) when the `GPG_RELEASE_KEY` secret is configured. Local signing is also available via `scripts/sign-release.sh <binary> [key-id]`. Required secrets: `GPG_RELEASE_KEY` (base64-encoded private key), `GPG_PASSPHRASE`, and optionally `GPG_KEY_ID`.

## Secrets

Never commit `.env` files, `*.pkcs8` signing keys, SQLite databases, or model credentials. The repository ignores the common secret and key extensions, and APM rejects private keys that are group- or world-readable on Unix.
