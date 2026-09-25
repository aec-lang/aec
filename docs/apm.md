# APM

APM is the AEC package manager. It manages local path dependencies and intentionally has no registry or network client.

Create or update `apm.toml`:

```sh
apm init
apm add helper ./path/to/helper
apm install
apm list
apm tree
apm remove helper
```

`apm init` creates a manifest with a validated package name and version. `apm add` records a canonical local path in the manifest. `apm install` verifies every dependency, reads its optional `apm.toml`, and writes the deterministic `apm.lock` file. `apm list` prints resolved names, versions, and paths; `apm tree` detects local cycles. `apm remove` updates the manifest and refreshes an existing lockfile.

A manifest uses this format:

```toml
[package]
name = "my-agent"
version = "0.1.0"

[dependencies]
helper = "/absolute/or/relative/path"
```

Package names may contain ASCII letters, numbers, `_`, and `-`. Versions accept simple semantic-version-like strings such as `0.1.0` or `1.0.0-beta`. The lockfile is generated locally and should be committed with the project.

The older `aec add`/`aec install` commands and legacy `aecpm.toml`/`aecpm.lock` files remain readable for migration, but new projects should use the `apm` binary and filenames.
