# AEC — Agent Easy Creator

AEC is an interpreted language for building agents with a native UI. The workspace contains the parser, AST, type checker, runtime, UI renderer, and `aec` command-line tool. Public module declarations, type aliases, and qualified UI components/themes are supported.

## Try it

Requires Rust and Cargo. From the repository root:

```sh
cargo test --workspace --locked --no-fail-fast
cargo run -p aec-cli -- check examples/language.aec
cargo run -p aec-cli -- run examples/language.aec --cli
cargo run -p aec-cli -- run examples/chatbot.aec
cargo run -p aec-cli --bin apm -- init
cargo run -p aec-cli --bin apm -- add helper ./path/to/helper
cargo run -p aec-cli --bin apm -- install
cargo run -p aec-cli --bin apm -- list
cargo run -p aec-cli --bin apm -- tree
cargo package --workspace --allow-dirty --no-verify --locked
```

`check` parses and type-checks; `ast` prints the parsed program; `run` checks before execution. `run` opens a native window when a UI is present unless `--cli` is specified. `--entry name` chooses the function called in CLI mode (default: `main`). `apm` is the package-manager binary; the older `aec add`/`aec install` commands remain compatible.

## Local CI

Run the same core checks locally with:

```sh
./scripts/ci-local.sh
```

The first run downloads roughly 300–800 MB of Rust dependencies and needs several GB of free disk space; later runs use Cargo's cache. Set `RUN_FMT=1` only when you explicitly want the repository-wide formatting check.## Example

```aec
agent Hello

fn main() -> int {
    var count = 1
    count += 1
    print("Count: {count}")
    return 0
}
```

Start with the [language guide](docs/language.md), [built-in reference](docs/stdlib.md), [components](docs/components.md), [themes](docs/themes.md), and [APM](docs/apm.md). The [roadmap](docs/roadmap.html) records current progress and remaining work. Examples under `examples/` can be checked with `aec check`.

## Current boundaries

Imports load relative `.aec` files recursively through the CLI. Unaliased imports retain the flat namespace; `pub` declarations can be exposed with `import "./math.aec" as math` and called as `math.double(...)`. Duplicate declarations and cycles are rejected. A `permissions` block restricts built-ins cooperatively inside the interpreter; it is not an operating-system sandbox. `shell.run`, environment access, and persistent memory are denied when that block is present. HTTP/model redirects are disabled in restricted mode. The chat UI example uses an offline echo reply and does not call a model. APM currently supports local path dependencies only; it does not contact a registry.
