# Changelog

## 0.2.0 — 2026-10-02

- Add signed HTTP(S) APM registry installation and verification.
- Stream and verify remote payloads before committing them to the package cache.
- Support multiple trust keys for signer rotation.
- Record verified registry tree digests in `apm.lock`.
- Re-verify imported cached packages before `aec run`.
- Document the remote registry protocol and rotation procedure.
