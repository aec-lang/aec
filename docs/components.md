# UI components

Declare a reusable component with props and a render body:

```aec
agent Example

component MessageBubble {
    prop role: string
    prop content: string

    render {
        Card {
            Text role
            Text content
        }
    }
}

ui Main = Screen "Messages" {
    Column {
        MessageBubble role: "user" content: "Hello"
    }
}
```

Props are evaluated using the parent UI state. Each component instance receives an identity-scoped local state map; its local `@state` is not inserted into the parent screen state. Local state is retained while the instance remains mounted and is discarded when a rebuild removes that instance. Screen-level state is synchronized with the interpreter around events. Recursive component expansion is guarded. Public components imported with an alias are addressed as `library.Component`; private module components are available only inside their module. For a themed example, see `examples/theme.aec`.
