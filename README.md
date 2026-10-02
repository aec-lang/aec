<div align="center">

# AEC — Agent Easy Creator

**A programming language for building AI agents, with a native UI in the same file.**

[![CI](https://github.com/aec-lang/aec/workflows/AEC%20CI/badge.svg)](https://github.com/aec-lang/aec/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/aec-lang/aec?label=release)](https://github.com/aec-lang/aec/releases)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange)](https://www.rust-lang.org)

</div>

---

AEC is an interpreted language for agents. An agent's logic, its permissions, and its
window are written in one `.aec` file, type-checked before it runs, and rendered with
a native egui window or a terminal UI.

```aec
agent Greeting

ui Main = Screen "Greeting" {
    @name: string = ""
    Column {
        Input placeholder: "Name" bind value to name
        Text "Hello {name}"
    }
}
```

```sh
aec run greeting.aec
```

---

## Contents

- [Install](#install) · [Try it in 30 seconds](#try-it-in-30-seconds)
- [The language](#the-language) · [Native UI](#native-ui) · [Permissions](#permissions)
- [Package manager](#package-manager) · [Sandbox](#os-sandbox) · [CLI reference](#cli-reference)
- [Architecture](#architecture) · [Development](#development) · [Status](#status)

---

## Install

### Prebuilt binaries (no Rust needed)

```sh
curl -fsSL https://raw.githubusercontent.com/aec-lang/aec/main/scripts/install-release.sh | bash
```

This downloads the release archive for your platform, verifies it against the
published `SHA256SUMS`, and installs `aec` and `apm` into `$HOME/.local/bin`.

<details>
<summary>Manual download, or Windows</summary>

Grab the archive for your platform from the
[releases page](https://github.com/aec-lang/aec/releases), then verify it:

```sh
sha256sum --check SHA256SUMS          # after extracting both files together
```

| Platform | Archive | Contains |
|---|---|---|
| Linux x86_64 | `aec-linux-x86_64.tar.gz` | `aec`, `apm` |
| macOS x86_64 | `aec-macos-x86_64.tar.gz` | `aec`, `apm` |
| Windows x86_64 | `aec-windows-x86_64.zip` | `aec.exe`, `apm.exe` |

Windows is not covered by the shell installer; download the `.zip` and extract it.
</details>

### From source

Requires Rust 1.75 or newer. Linux builds of the native UI also need the GTK, XCB,
and OpenSSL development packages.

```sh
git clone https://github.com/aec-lang/aec.git
cd aec
PREFIX="$HOME/.local" ./scripts/install.sh
export PATH="$HOME/.local/bin:$PATH"
```

Or with Cargo directly:

```sh
cargo install --path crates/aec-cli --locked --bin aec --bin apm
```

<details>
<summary>Debian/Ubuntu build dependencies</summary>

```sh
sudo apt-get install -y libclang-dev libgtk-3-dev libxcb-render0-dev \
    libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-dev libssl-dev bubblewrap
```
</details>

---

## Try it in 30 seconds

```sh
aec check examples/language.aec      # parse + type check
aec run   examples/language.aec --cli
aec run   examples/chatbot.aec       # opens a native window
aec run   examples/chatbot.aec --tui # same UI, drawn in the terminal
```

`examples/` holds runnable programs: `language.aec`, `nominal.aec`, `result.aec`,
`lambda.aec`, `chatbot.aec`, `theme.aec`, `memory.aec`, `sandbox.aec`, and a
two-file `modules/` example.

---

## The language

Every file starts with `agent Name`.

```aec
agent Basics

struct User {
    name: string
    age: int
}

enum Role {
    Admin
    Suspended(string)
}

fn describe(user: User) -> string {
    return match user {
        User { name: name, age: age } -> "{name} ({age})"
    }
}

fn main() -> int {
    var count = 0
    for item in [1, 2, 3] {
        count += item
    }
    print("sum is {count}")
    return 0
}
```

| Feature | Syntax |
|---|---|
| Bindings | `let x = 1` (immutable), `var y = 2` (assignable), `x += 1` |
| Types | `int` `float` `string` `bool` `bytes` `uuid` `timestamp` `unit` `[T]` `T?` `Result(T,E)` `function` |
| Nominal types | `struct` with named-argument constructors, `enum` with unit or one-payload variants |
| Error handling | `ok(v)` / `err(v)`, `expr?` to propagate |
| Closures | `x => x + 1` captures locals by snapshot |
| Aliases | `type UserId = string` |
| Interpolation | `"Hello {name}"` — a brace group that is not a valid expression stays literal |
| Control flow | `if`/`else`, `while`, `for ... in`, `match`, `return` |
| Logic | `not`, `and`, `or` |
| Modules | `import "./math.aec" as math`, then `math.double(21)`; `pub` controls export |

Struct and enum equality is **nominal**: two structurally identical values of
different named types are not equal. Pattern matching works on both.

Full reference: [language guide](docs/language.md) · [built-ins](docs/stdlib.md)

---

## Native UI

A `ui` declaration builds a real window. State is declared with `@name`, bound to
widgets, and the tree is rebuilt against current state after every event.

```aec
ui Main = Screen "Messages" {
    @draft: string = ""
    @messages: [object] = []

    Column {
        Messages list: messages
        Input placeholder: "Say something" bind value to draft
        Button "Send" on click -> send()
    }
}
```

- **Widgets:** `Text` `Input` `Button` `Column` `Row` `Card` `Messages`
- **Binding:** `Input placeholder: "…" bind value to name`
- **Events:** `Button "Send" on click -> send()` — event arguments are passed to the handler
- **Reactivity:** `if` and `for` work inside the tree, including `range(3)`; the
  widget tree is rebuilt after input and events, so conditions and loops update
- **State:** `push_to("messages", item)` updates the global array of the same name
- **Components:** reusable `component` blocks with `prop` values and a `render` body;
  each instance gets identity-scoped local state
- **Themes:** `theme Dark default { color { … } text { … } }`, accessed as
  `theme.color.primary`, with `extends` for inheritance

Themes and components from an aliased import use their qualified name
(`library.Panel`, `theme.library.Dark.color.primary`).

Right-to-left text is laid out correctly and the bundled Vazirmatn font covers Persian.

Reference: [components](docs/components.md) · [themes](docs/themes.md)

---

## Permissions

Capabilities are default-deny once a `permissions` block is present.

```aec
permissions {
    network: ["api.example.com"]
    filesystem {
        read: ["."]
        write: ["data"]
    }
    system { metrics: true }
}

limits {
    timeout: 5s
}
```

With a block, undeclared network, filesystem, model, and system capabilities are
denied; `shell.run`, environment access, and persistent memory are denied outright;
HTTP and model requests do not follow redirects. Without a block, built-ins keep
their unrestricted behavior.

This is an **in-process cooperative guard, not an OS sandbox.** For untrusted input,
combine it with a real host boundary — see below.

Reference: [language guide § Permissions](docs/language.md#permissions) · [SECURITY.md](SECURITY.md)

---

## Package manager

`apm` is AEC's package manager. It has the commands you expect, a signed immutable
registry, and an HTTP(S) registry client.

```sh
apm init my-agent                        # create apm.toml
apm add helper ./packages/helper         # local path dependency
apm install ui                           # install by name from the default registry
apm install                              # resolve apm.toml, write apm.lock
apm list && apm tree
```

### Signed registry

```sh
apm keygen --private-key keys/signing.pkcs8 --public-key keys/signing.pub
apm registry-init ./registry
apm publish --registry ./registry --private-key keys/signing.pkcs8 --package .
apm verify --registry ./registry --name my-agent --version 0.1.0 \
    --trust-key keys/signing.pub
```

Publication copies the payload into a staging directory, writes deterministic
metadata binding every file's SHA-256, size, and the aggregate tree digest, and signs
that metadata with a detached Ed25519 signature. Versions are immutable — an existing
version is never replaced. Verification always requires an explicitly selected
`--trust-key`; a public key embedded in registry metadata does not establish trust by
itself.

`apm install <name>` resolves the version from the default registry's
`packages/<name>/latest` pointer, verifies the signed payload, and records
`registry://<name>@<version>` in `apm.toml`. The default registry is
`https://aec-lang.github.io/apm-registry` and can be overridden with
`--registry` or the `AEC_APM_REGISTRY` environment variable.

Any HTTP(S) base URL can be passed to `apm install` or `apm verify`. The
server must expose the same `packages/<name>/<version>/` layout as a local
registry. Multiple public keys may be listed in one trust-key file
or passed with repeated `--trust-key` flags, which permits signer rotation without
trusting a key merely because it appears in metadata. Before execution, imported
cached registry payloads are re-hashed and compared with `apm.lock`.

Reference: [APM](docs/apm.md)

---

## OS sandbox

The interpreter guard runs inside the process. These scripts add a real host
boundary on top of it:

| Platform | Script | Mechanism | Verified |
|---|---|---|---|
| Linux | `scripts/sandbox-linux.sh` | rootless `bubblewrap`: read-only workspace, private tmpfs, unshared network namespace, dropped capabilities | yes |
| macOS | `scripts/sandbox-macos.sh` | `sandbox-exec` profile, deny-by-default | needs a macOS host |
| Windows | `scripts/sandbox-windows.ps1` | emits a `.wsb` config for Windows Sandbox (Hyper-V VM, read-only repo mount) | needs a Windows host |

```sh
./scripts/sandbox-linux.sh aec run untrusted.aec --cli
./scripts/sandbox-selftest.sh    # reports verified / untested / failed per platform
```

On Linux, a sandboxed run cannot read files outside the repository and cannot write
into it — both were confirmed to fail closed. `sandbox-selftest.sh` deliberately
reports `untested` for platforms it cannot run on rather than claiming coverage.

Reference: [SECURITY.md](SECURITY.md)

---

## CLI reference

### `aec`

| Command | Description |
|---|---|
| `aec check <file>` | Parse and type check. Exits non-zero on any error. |
| `aec ast <file>` | Print the parsed AST. |
| `aec run <file>` | Type check, then run. Opens a native window if a `ui` exists. |
| `aec run <file> --cli` | Force terminal mode, ignoring the UI. |
| `aec run <file> --tui` | Draw the UI in the terminal instead of a window. |
| `aec run <file> --entry <fn>` | Call a different entry function (default `main`). |
| `aec ui-snap <file>` | Render headlessly and write PNG snapshots (visual QA). |
| `aec ui-snap <file> --click Send --type draft=hello` | Drive the UI, then snapshot the result. |

`aec` also accepts `init`, `add`, `remove`, `install`, `list`, and `tree` as aliases of
the `apm` commands.

### `apm`

`init` · `add` · `remove` · `install` · `list` · `tree` · `keygen` · `registry-init` ·
`publish` · `verify`

### Headless visual QA

```sh
aec ui-snap examples/chatbot.aec --out snapshots/ --click Send --type draft=سلام
```

Writes PNG frames and reports the painted-pixel count and the interactable widgets, so
UI regressions are detectable in CI without a display server.

---

## Architecture

Six crates, no runtime dependencies beyond the standard library and a focused set of
crates:

| Crate | Responsibility |
|---|---|
| `aec-ast` | Tokens, spans, declarations, expressions, types, styles, themes, permissions |
| `aec-parser` | `pest` grammar → AST, with spanned diagnostics |
| `aec-check` | Static checker: types, arity, named arguments, nominal identity |
| `aec-runtime` | Tree-walking interpreter, built-ins, permission guard, SQLite memory |
| `aec-ui` | egui/eframe native window, terminal renderer, headless snapshot harness |
| `aec-cli` | The `aec` and `apm` binaries, module loader, diagnostic renderer |

`aec run` will not execute a program that fails the type check — the gate is
structural, not advisory.

Built-ins cover collections, text, files, HTTP/JSON, environment, time, math, regex,
crypto, UUID, OpenAI-compatible model calls, and SQLite-backed conversation memory.

---

## Development

```sh
cargo test --workspace --locked --no-fail-fast   # 378 tests
cargo clippy --workspace --all-targets --locked -- -D warnings
./scripts/ci-local.sh                            # the same checks CI runs
```

CI runs on Linux, macOS, and Windows on every push and pull request, and includes a
bubblewrap sandbox smoke test on Linux. Pushing a `v*` tag builds release binaries for
all three platforms, publishes them with `SHA256SUMS`, and signs every archive with a
detached GPG signature when the `GPG_RELEASE_KEY` secret is configured.

---

## Status

`0.1.0` — an early, pre-1.0 release. Working today:

- Full language: modules, nominal `struct`/`enum`, `Result`/`?`, closures, type aliases
- Reactive native UI with components, themes, RTL text, and a terminal renderer
- Static type checker that gates execution
- Cooperative permission guard plus real OS sandbox scripts for all three platforms
- `apm` with a signed, immutable registry, an HTTP(S) registry client, signer
  rotation via multiple trust keys, and re-verification of imported cached packages
  before every `aec run`
- 378 tests, green across Linux, macOS, and Windows

Known boundaries, stated plainly:

- `permissions` is an in-process guard, not OS isolation; use the sandbox scripts for
  untrusted input
- The macOS and Windows sandbox paths are written but not yet verified on their hosts
- No generic inference, async runtime, A2A protocol, or IDE
- The chatbot example is an offline echo, not a live model call
- Native UI support is x86_64 only; there is no aarch64 release

---

## Documentation

Read it online at **[aec-lang.github.io/aec](https://aec-lang.github.io/aec)** — the
documentation site is generated from these same files, so the two never disagree.

| Document | Contents |
|---|---|
| [Language guide](docs/language.md) | Syntax, modules, types, UI, permissions |
| [Built-ins](docs/stdlib.md) | Every built-in, with argument notes |
| [Components](docs/components.md) | Reusable components and scoped state |
| [Themes](docs/themes.md) | Theme tokens, inheritance, precedence |
| [APM](docs/apm.md) | Manifests, lockfile, keys, registry, verification |
| [Release guide](docs/release.md) | Artifacts, checksums, maintainer procedure |
| [Presentation (FA)](docs/AEC_Presentation_FA.md) | Project overview, in Persian |
| [Roadmap](docs/roadmap.html) | Feature status with per-item verification state |
| [SECURITY.md](SECURITY.md) | Trust boundaries, sandbox status, reporting |
| [CHANGELOG.md](CHANGELOG.md) | Release history |

To build and preview the site locally:

```sh
node scripts/build-site.cjs          # docs/ and README.md -> website/
node scripts/build-registry.cjs      # registry page from a real APM directory
python3 scripts/check-site.py        # link, nesting, and search-index checks
python3 -m http.server --directory website 8000
```

## Contributing

Issues and pull requests are welcome. Run `./scripts/ci-local.sh` before opening a pull
request, and add a test that fails without your change.

## License

Licensed under either of MIT or Apache-2.0, at your option.

The full texts are the standard upstream documents:

- MIT: <https://opensource.org/license/mit>
- Apache-2.0: <https://www.apache.org/licenses/license-2.0>
