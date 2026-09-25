# Themes and styles

```aec
agent ThemeExample

theme Dark default {
    color {
        background: "#1e1e2e"
        surface: "#313244"
        text: "#cdd6f4"
        primary: "#89b4fa"
    }
    text {
        size: 16
        weight: "normal"
    }
}

ui Main = Screen "Theme" {
    Column background: theme.color.background {
        Text "Hello" color: theme.color.primary
    }
}
```

Use `theme Name default` to select the active theme, or let the only declared theme become active. `theme Ocean extends Dark` inherits tokens and overrides entries it defines. Access tokens as `theme.group.token`. The renderer uses `color.background` for the panel, `color.surface` for cards, and `color.text`, `text.size`, `text.weight` as text defaults when available. Inline element properties override a style block. Missing tokens are ignored during rendering. See `examples/theme.aec` for a larger example.
