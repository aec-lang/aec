# APM

APM is the AEC package manager. It supports local path dependencies and signed
registries served either from a directory or over HTTP(S).

## Local path dependencies

Create or update `apm.toml`:

```sh
apm init
apm add helper ./path/to/helper
apm install
apm list
apm tree
apm remove helper
```

`apm init` creates a manifest with a validated package name and version. `apm add` records a canonical local path in the manifest. `apm install` structurally validates every local dependency, reads its optional `apm.toml`, and writes the deterministic `apm.lock` file. `apm list` prints resolved names, versions, and paths; `apm tree` detects local cycles. `apm remove` updates the manifest and refreshes an existing lockfile.

A manifest uses this format:

```toml
[package]
name = "my-agent"
version = "0.1.0"

[dependencies]
helper = "/absolute/or/relative/path"
```

Package names may contain ASCII letters, numbers, `_`, and `-`. Versions accept simple semantic-version-like strings such as `0.1.0` or `1.0.0-beta`. The lockfile remains a local path lockfile and should be committed with the project. Registry publication and verification do not change this lockfile behavior or resolve registry dependencies yet.

## Signing keys

Generate an unencrypted Ed25519 PKCS#8 v2 private key and a lowercase hexadecimal public key:

```sh
apm keygen --private-key ./keys/signing.pkcs8 --public-key ./keys/signing.pub
```

The private key is written only to the requested file. On Unix it is created with mode `0600`, and publishing rejects private keys readable by group or other users. Symlinks, invalid keys, and private keys located inside the registry or package payload are rejected. The registry and source package must also use separate directory trees. Registry commands never add private key bytes or paths to command output, registry metadata, or `apm.lock`; generated `*.pkcs8` files are ignored by Git.

The public key is informational inside registry metadata. It does not establish trust by itself. Verification always requires one or more explicitly selected `--trust-key PATH` files, stored outside the registry. A trust file may contain multiple public-key lines; this is the key-rotation mechanism. During rotation, keep both the old and new keys trusted, publish with the new key, then remove the old key after all clients have migrated.

## Local registry

Initialize a registry and publish a package:

```sh
apm registry-init ./registry
apm publish --registry ./registry --private-key ./keys/signing.pkcs8 --package ./my-agent
apm verify --registry ./registry --name my-agent --version 0.1.0 --trust-key ./keys/signing.pub
```

If `--package` is omitted, publish uses the current directory. A published version has this layout:

```text
registry/
├── .staging/
└── packages/
    └── <name>/
        └── <version>/
            ├── payload/
            ├── metadata
            └── signature
```

Publishing copies the package into a same-registry staging directory, excludes common build/VCS/secret paths, writes deterministic metadata and a detached Ed25519 signature, and atomically renames the completed version into place. An existing version is never replaced. A failed or colliding publish leaves no version directory and removes its staging directory.

The canonical `metadata` file is UTF-8, line-based, LF-terminated, and sorted by canonical payload path. It binds the package name and version, exact `apm.toml` digest, SHA-256 and size for every payload file, aggregate file count and size, a SHA-256 tree digest, and the publisher's public key. The `signature` file is the raw 64-byte detached Ed25519 signature over the exact metadata bytes. Verify first validates that signature with the explicit trust key, then reloads the manifest and payload, rejects links and special files, recomputes every file/tree digest, rebuilds canonical metadata byte-for-byte, and checks the requested name and version. Payload or metadata tampering and version collisions fail.

Payload traversal rejects symbolic links, FIFOs, sockets, devices, and other special files. The MVP limits are 10,000 files, 10,000 total payload entries including directories, 16 MiB per file, 256 MiB total payload size, 1,024 UTF-8 bytes per relative path, 64 directory levels, and 32 MiB of metadata.

Registry verification is an integrity and materialization boundary. The resolved
package is recorded in `apm.lock` with its tree digest, and the runtime re-checks
that digest before executing imported cached code.

## Remote registry

The remote protocol is deliberately static: an HTTP(S) registry serves the same
layout shown above. Install and verify download signed metadata first, then stream
each signed payload file, enforce size limits, recompute SHA-256 and the aggregate
tree digest, and only then commit the package to `.apm/packages`.

```sh
apm install \
  --registry https://registry.example.invalid \
  --trust-key ./keys/old.pub \
  --trust-key ./keys/new.pub

apm verify --registry https://registry.example.invalid \
  --name helper --version 0.2.0 \
  --trust-key ./keys/new.pub
```

When an AEC program imports a cached package, `aec run` re-verifies its complete
payload against the tree digest recorded in `apm.lock` before executing code.

The older `aec add`/`aec install` commands and legacy `aecpm.toml`/`aecpm.lock` files remain readable for migration, but new projects should use the `apm` binary and filenames.
