# AEC language guide

Each source file starts with `agent Name`. A CLI entry point is usually `fn main() -> int`. Run `cargo run -p aec-cli -- check path/to/file.aec` before executing a program.

## Files and imports

```aec
agent Main
import "./math.aec"

fn main() -> int {
    return double(21)
}
```

The imported file also starts with an `agent` header. Paths are resolved relative to the importing file. The CLI loads transitive imports once, rejects cycles, duplicate declarations, and checks the combined program. An unaliased import keeps the legacy flat namespace. Use `pub` on declarations that should be exposed through an alias; private functions, models, components, themes, and type aliases stay inside their module:

```aec
import "./math.aec" as math
fn main() -> int { return math.double(21) }
```

Alias exports are explicit and private declarations are not exposed under the alias. Permissions and limits must be declared in the entry file; an imported file cannot override them. See `examples/modules/`.

## Values, bindings, and control flow

```aec
agent Basics

fn sum_to(limit: int) -> int {
    var total = 0
    for item in [1, 2, 3] {
        total += item
    }
    if total > limit and limit >= 0 {
        return total
    }
    return limit
}
```

`let` is immutable and `var` can be assigned to. A declared type uses `name: type`; functions use `-> type`. Types include `int`, `float`, `string`, `bool`, `bytes`, `uuid`, `timestamp`, `unit`, `[type]`, `type?`, `Result(type, type)`, and `function`. Literals include `none`, arrays, objects, durations such as `10s`, and strings. Strings evaluate valid expressions in braces: `"Hello {name}"`. A brace sequence that is not a valid expression remains text.

`if`/`else`, `while`, `for`, `return`, and `match` are supported. Boolean operators are `not`, `and`, `or`. Functions and closures can be called; `x => x + 1` creates a closure with a snapshot of local bindings at creation time.

`ok(value)` and `err(value)` construct results. `expression?` unwraps an `ok` result or returns the `err` from the current function. Named type aliases are also supported:

```aec
type UserId = string

fn normalize(value: UserId) -> UserId {
    return value
}
```

See `examples/result.aec` and `examples/lambda.aec`.

## UI

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

UI declarations build a native egui window. `@name` declares typed UI state. `Input` binds a string state value; `Button "Send" on click -> send()` calls a language function and event arguments are supported. The widget tree is rebuilt against current state, so text, conditions, and loops update after input or events. `Messages list: messages` shows an array of objects with `role` and `content`. `if` and `for` work inside the UI tree, including call-based iterables such as `range(3)`. Component instances have scoped local state. Public components and themes from an aliased import use their qualified name (`library.Panel`, `theme.library.Dark.color.primary`). See `examples/chatbot.aec`.

## Permissions

```aec
permissions {
    network: ["api.example.com"]
    filesystem {
        read: ["."]
        write: ["."]
    }
    system {
        metrics: true
    }
}

limits {
    timeout: 5s
}
```

Without a `permissions` block, built-ins retain their unrestricted behavior. With a block, undeclared network, file, model-request, and system capabilities are denied by the interpreter. `shell.run`, environment access, and persistent memory are denied, and HTTP/model redirects are not followed in this mode. This is an in-process guard, not isolation for untrusted native code. See `examples/sandbox.aec`.
