# Changelog

## 0.2.1 — 2026-10-02

- Add `apm install <name>` for installing a package by name from a registry.
- Add a default registry (`https://aec-lang.github.io/apm-registry`) and a
  build-time trust key set, both overridable with `--registry`,
  `--trust-key`, and `AEC_APM_REGISTRY`.
- Resolve the installed version from the registry's `packages/<name>/latest`
  pointer when `--version` is omitted.

## 0.2.0 — 2026-10-02

- Add signed HTTP(S) APM registry installation and verification.
- Stream and verify remote payloads before committing them to the package cache.
- Support multiple trust keys for signer rotation.
- Record verified registry tree digests in `apm.lock`.
- Re-verify imported cached packages before `aec run`.
- Document the remote registry protocol and rotation procedure.
