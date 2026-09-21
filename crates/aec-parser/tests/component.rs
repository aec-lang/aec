use aec_ast::{ComponentUse, Literal, TopLevelItem, TypeRef, UiStatement};
use aec_parser::parse;

fn collect_component_uses(statements: &[UiStatement], out: &mut Vec<ComponentUse>) {
    for stmt in statements {
        match stmt {
            UiStatement::Component(use_) => out.push(use_.clone()),
            UiStatement::Element(el) => {
                if let Some(children) = &el.children {
                    collect_component_uses(children, out);
                }
            }
            UiStatement::If(ui_if) => {
                collect_component_uses(&ui_if.then_body, out);
                if let Some(else_body) = &ui_if.else_body {
                    collect_component_uses(else_body, out);
                }
            }
            UiStatement::For(ui_for) => collect_component_uses(&ui_for.body, out),
            UiStatement::State(_) => {}
        }
    }
}

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

#[test]
fn parses_component_decl_with_props_and_render() {
    let program = parse(SOURCE).expect("should parse component program");

    let component = program
        .items
        .iter()
        .find_map(|item| match item {
            TopLevelItem::Component(c) => Some(c),
            _ => None,
        })
        .expect("program should contain a component decl");

    assert_eq!(component.name.name, "MessageBubble");
    assert_eq!(component.props.len(), 2);
    assert_eq!(component.props[0].name.name, "role");
    assert_eq!(component.props[1].name.name, "content");
    assert!(matches!(component.props[0].ty, Some(TypeRef::String)));
    assert!(matches!(component.props[1].ty, Some(TypeRef::String)));

    let render = component
        .render
        .as_ref()
        .expect("component should have a render body");
    assert_eq!(render.len(), 1);
    assert!(matches!(render[0], UiStatement::Element(_)));
}

#[test]
fn parses_component_use_inside_ui() {
    let program = parse(SOURCE).expect("should parse component program");

    let ui = program
        .items
        .iter()
        .find_map(|item| match item {
            TopLevelItem::Ui(u) => Some(u),
            _ => None,
        })
        .expect("program should contain a ui decl");

    let mut uses = Vec::new();
    collect_component_uses(&ui.screen.body, &mut uses);

    assert_eq!(uses.len(), 2);
    assert_eq!(uses[0].name.name, "MessageBubble");
    assert_eq!(uses[0].props.len(), 2);
    assert_eq!(uses[0].props[0].name.name, "role");
    assert_eq!(uses[0].props[1].name.name, "content");

    assert_eq!(
        prop_string(&uses[0], "role").as_deref(),
        Some("user"),
        "first use should carry role: \"user\""
    );
    assert_eq!(prop_string(&uses[0], "content").as_deref(), Some("سلام"));
    assert_eq!(prop_string(&uses[1], "role").as_deref(), Some("ai"));
    assert_eq!(prop_string(&uses[1], "content").as_deref(), Some("سلام!"));
}

fn prop_string(use_: &ComponentUse, name: &str) -> Option<String> {
    use_.props
        .iter()
        .find(|p| p.name.name == name)
        .and_then(|p| match &p.value {
            aec_ast::Expr::Literal(lit) => match &lit.value {
                Literal::String(s) => Some(s.clone()),
                _ => None,
            },
            _ => None,
        })
}
