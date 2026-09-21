//! End-to-end: parse a real `.aec` source, build the component registry from the
//! program items, and expand component uses into widgets.

use aec_ast::TopLevelItem;
use aec_parser::parse;
use aec_ui::widgets::{build_widgets_with_components, ComponentRegistry, UiState, Widget};

const SOURCE: &str = r##"agent Test

component MessageBubble {
    prop role: string
    prop content: string

    render {
        Column {
            Text role { color: "#667eea" weight: "bold" }
            Text content
        }
    }
}

ui Main = Screen "Chat" {
    Column {
        MessageBubble role: "user" content: "سلام"
        MessageBubble role: "ai" content: "سلام!"
    }
}
"##;

fn registry_from(program: &aec_ast::Program) -> ComponentRegistry {
    program
        .items
        .iter()
        .filter_map(|item| match item {
            TopLevelItem::Component(component) => {
                Some((component.name.name.clone(), component.clone()))
            }
            _ => None,
        })
        .collect()
}

fn collect_texts(widgets: &[Widget], out: &mut Vec<String>) {
    for widget in widgets {
        match widget {
            Widget::Container(children) => collect_texts(children, out),
            Widget::Column(children, _) => collect_texts(children, out),
            Widget::Row(children, _) => collect_texts(children, out),
            Widget::Card(children, _) => collect_texts(children, out),
            Widget::For { items, .. } => collect_texts(items, out),
            Widget::If { then_branch, else_branch, .. } => {
                collect_texts(then_branch, out);
                if let Some(else_branch) = else_branch {
                    collect_texts(else_branch, out);
                }
            }
            Widget::Text(text, _) => out.push(text.clone()),
            _ => {}
        }
    }
}

#[test]
fn parses_and_expands_components_end_to_end() {
    let program = parse(SOURCE).expect("source should parse");

    let components = registry_from(&program);
    assert_eq!(components.len(), 1);
    assert!(components.contains_key("MessageBubble"));

    let ui = program
        .items
        .iter()
        .find_map(|item| match item {
            TopLevelItem::Ui(ui) => Some(ui),
            _ => None,
        })
        .expect("program should have a ui decl");

    let mut state = UiState::new();
    let widgets = build_widgets_with_components(&ui.screen.body, &mut state, &components);

    let mut texts = Vec::new();
    collect_texts(&widgets, &mut texts);

    // Both props must reach the render body and become Text widgets.
    assert_eq!(texts, vec!["user", "سلام", "ai", "سلام!"]);
}
