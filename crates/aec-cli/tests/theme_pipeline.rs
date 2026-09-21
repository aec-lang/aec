//! End-to-end: parse a real `.aec` source with themes, resolve them, and check
//! that element styles come out with concrete token values.

use aec_ast::TopLevelItem;
use aec_parser::parse;
use aec_ui::{build_widgets_with_themes, ComponentRegistry, Themes, UiState, Widget};

const SOURCE: &str = r##"agent ChatApp

theme Dark default {
    color {
        background: "#1e1e2e"
        text: "#cdd6f4"
        primary: "#89b4fa"
    }
    text {
        size: 16
    }
    spacing {
        lg: 16
    }
}

theme Ocean extends Dark {
    color {
        primary: "#00b4d8"
    }
}

ui Main = Screen "Chat" {
    Column background: theme.color.background {
        Text "سلام" color: theme.color.primary
        Text "چطور هستی؟"
    }
}
"##;

fn collect_text_styles(widgets: &[Widget], out: &mut Vec<(String, Option<String>)>) {
    for widget in widgets {
        match widget {
            Widget::Container(children) => collect_text_styles(children, out),
            Widget::Column(children, _) => collect_text_styles(children, out),
            Widget::Row(children, _) => collect_text_styles(children, out),
            Widget::Card(children, _) => collect_text_styles(children, out),
            Widget::Text(text, style) => out.push((text.clone(), style.get_string("color"))),
            _ => {}
        }
    }
}

fn column_style(widgets: &[Widget]) -> Option<String> {
    widgets.iter().find_map(|w| match w {
        Widget::Column(_, style) => style.get_string("background"),
        _ => None,
    })
}

#[test]
fn parses_and_applies_theme_end_to_end() {
    let program = parse(SOURCE).expect("source should parse");

    // Two themes: Dark (default) and Ocean (extends Dark)
    let theme_count = program
        .items
        .iter()
        .filter(|i| matches!(i, TopLevelItem::Theme(_)))
        .count();
    assert_eq!(theme_count, 2);

    let themes = Themes::from_items(&program.items);
    assert!(themes.active().is_some(), "Dark is marked default");
    assert_eq!(
        themes
            .get("Ocean")
            .and_then(|t| t.get("color.primary"))
            .map(|v| v.as_deref_string()),
        Some("#00b4d8".to_string())
    );

    let ui = program
        .items
        .iter()
        .find_map(|item| match item {
            TopLevelItem::Ui(ui) => Some(ui),
            _ => None,
        })
        .expect("ui decl");

    let components = ComponentRegistry::new();
    let mut state = UiState::new();
    let widgets = build_widgets_with_themes(&ui.screen.body, &mut state, &components, &themes);

    // The Column background was resolved from the theme.color.background token
    assert_eq!(column_style(&widgets).as_deref(), Some("#1e1e2e"));

    let mut texts = Vec::new();
    collect_text_styles(&widgets, &mut texts);
    assert_eq!(texts.len(), 2);

    // A Text with no explicit color falls back to theme.text.* / color.text
    assert_eq!(texts[0].0, "سلام");
    assert_eq!(texts[0].1.as_deref(), Some("#89b4fa"));
    assert_eq!(texts[1].1.as_deref(), Some("#cdd6f4"));
}

/// Converts a StyleValue to String for asserts (there is no get_string on &StyleValue).
trait AsDerefString {
    fn as_deref_string(&self) -> String;
}

impl AsDerefString for aec_ast::StyleValue {
    fn as_deref_string(&self) -> String {
        match self {
            aec_ast::StyleValue::String(s) => s.clone(),
            aec_ast::StyleValue::Ident(s) => s.clone(),
            aec_ast::StyleValue::Int(n) => n.to_string(),
            aec_ast::StyleValue::Float(f) => f.to_string(),
            aec_ast::StyleValue::Bool(b) => b.to_string(),
        }
    }
}
