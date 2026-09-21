use aec_ast::{Expr, StyleValue, TopLevelItem, UiStatement};
use aec_parser::parse;

const SOURCE: &str = r##"agent Test

theme Dark default {
    color {
        background: "#1e1e2e"
        primary: "#89b4fa"
    }
    text {
        size: 16
        weight: "bold"
    }
    shape {
        radius: 8
    }
}

theme Ocean extends Dark {
    color {
        primary: "#00b4d8"
    }
}

ui Main = Screen "Chat" {
    Column background: theme.color.background {
        Text "hi" color: theme.color.primary
    }
}
"##;

fn themes(program: &aec_ast::Program) -> Vec<&aec_ast::ThemeDecl> {
    program
        .items
        .iter()
        .filter_map(|item| match item {
            TopLevelItem::Theme(t) => Some(t),
            _ => None,
        })
        .collect()
}

/// The path of a chained expr such as theme.color.primary
fn expr_path(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Identifier(id) => Some(id.name.clone()),
        Expr::Member(m) => Some(format!("{}.{}", expr_path(&m.object)?, m.property.name)),
        _ => None,
    }
}

#[test]
fn parses_theme_decls() {
    let program = parse(SOURCE).expect("should parse theme program");
    let themes = themes(&program);
    assert_eq!(themes.len(), 2);

    let dark = themes[0];
    assert_eq!(dark.name.name, "Dark");
    assert!(dark.is_default);
    assert!(dark.extends.is_none());
    assert_eq!(dark.groups.len(), 3);

    let color = &dark.groups[0];
    assert_eq!(color.name.name, "color");
    assert_eq!(color.entries.len(), 2);
    assert_eq!(color.entries[0].name.name, "background");
    assert!(matches!(&color.entries[0].value, StyleValue::String(s) if s == "#1e1e2e"));
    assert_eq!(color.entries[1].name.name, "primary");

    let text = &dark.groups[1];
    assert_eq!(text.name.name, "text");
    assert!(matches!(text.entries[0].value, StyleValue::Int(16)));
    assert!(matches!(&text.entries[1].value, StyleValue::String(s) if s == "bold"));

    let shape = &dark.groups[2];
    assert!(matches!(shape.entries[0].value, StyleValue::Int(8)));

    let ocean = themes[1];
    assert_eq!(ocean.name.name, "Ocean");
    assert!(!ocean.is_default);
    assert_eq!(ocean.extends.as_ref().unwrap().name, "Dark");
    assert_eq!(ocean.groups.len(), 1);
}

#[test]
fn parses_theme_reference_in_element_properties() {
    let program = parse(SOURCE).expect("should parse theme program");

    let ui = program
        .items
        .iter()
        .find_map(|item| match item {
            TopLevelItem::Ui(u) => Some(u),
            _ => None,
        })
        .expect("should have ui decl");

    let column = match &ui.screen.body[0] {
        UiStatement::Element(el) => el,
        other => panic!("expected element, got {:?}", other),
    };
    assert_eq!(column.name.name, "Column");

    let background = column
        .modifiers
        .iter()
        .find_map(|m| match m {
            aec_ast::ElementModifier::Property(p) if p.name.name == "background" => Some(p),
            _ => None,
        })
        .expect("Column should have background property");
    assert_eq!(
        expr_path(&background.value).as_deref(),
        Some("theme.color.background")
    );

    let text = match &column.children.as_ref().unwrap()[0] {
        UiStatement::Element(el) => el,
        other => panic!("expected element, got {:?}", other),
    };
    let color = text
        .modifiers
        .iter()
        .find_map(|m| match m {
            aec_ast::ElementModifier::Property(p) if p.name.name == "color" => Some(p),
            _ => None,
        })
        .expect("Text should have color property");
    assert_eq!(
        expr_path(&color.value).as_deref(),
        Some("theme.color.primary")
    );
}

#[test]
fn parses_theme_reference_in_style_block() {
    let source = r##"agent Test

theme Dark {
    color { primary: "#89b4fa" }
}

ui Main = Screen "Chat" {
    Text "hi" { color: theme.color.primary size: 20 }
}
"##;
    let program = parse(source).expect("should parse");
    let ui = program
        .items
        .iter()
        .find_map(|item| match item {
            TopLevelItem::Ui(u) => Some(u),
            _ => None,
        })
        .expect("ui decl");
    let text = match &ui.screen.body[0] {
        UiStatement::Element(el) => el,
        other => panic!("expected element, got {:?}", other),
    };
    let color = text.style.properties.get("color").expect("color in style");
    assert!(matches!(color, StyleValue::Ident(s) if s == "theme.color.primary"));
    assert!(matches!(
        text.style.properties.get("size"),
        Some(StyleValue::Int(20))
    ));
}
