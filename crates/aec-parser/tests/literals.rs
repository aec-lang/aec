//! Tests for `literal` — a regression guard for the alternative ordering.
//!
//! Fixed bug: `literal` tried `int_literal` first, so `duration_literal`,
//! `byte_size_literal`, and `uuid_literal` were **never** reached. That produced
//! two wrong behaviours:
//!   - in `model m { timeout: 10s }` -> a parse error
//!   - in `let x = 10s` -> it **silently** became two statements: `let x = 10` and `s`
//!
//! If the alternative ordering is changed, these tests must fail.

use aec_ast::{Expr, Literal, Statement, TopLevelItem, UiStatement};
use aec_parser::parse;

fn let_literal(src: &str) -> Literal {
    let program = parse(src).expect("parse should succeed");
    let body = program
        .items
        .iter()
        .find_map(|i| match i {
            TopLevelItem::Function(f) => Some(&f.body),
            _ => None,
        })
        .expect("function body");

    let let_stmt = body
        .statements
        .iter()
        .find_map(|s| match s {
            Statement::Let(l) => Some(l),
            _ => None,
        })
        .expect("a Let statement");

    match &let_stmt.value {
        Expr::Literal(lit) => lit.value.clone(),
        other => panic!("expected Literal, got {:?}", other),
    }
}

fn fn_src(expr: &str) -> String {
    format!(
        "agent T\n\nfn f() -> int {{\n    let x = {}\n    return 0\n}}\n",
        expr
    )
}

#[test]
fn duration_literal_is_not_shadowed_by_int_literal() {
    match let_literal(&fn_src("10s")) {
        Literal::Duration(d) => {
            assert_eq!(d.value, 10);
            assert_eq!(d.unit.to_ms(), 1_000);
        }
        // If int_literal were first again, this would be Int(10)
        other => panic!("expected Duration, got {:?}", other),
    }
}

#[test]
fn duration_expr_does_not_split_into_two_statements() {
    // The buggy version silently turned this into `let x = 10` + statement `s`
    let program = parse(&fn_src("10s")).expect("parse should succeed");
    let body = program
        .items
        .iter()
        .find_map(|i| match i {
            TopLevelItem::Function(f) => Some(&f.body),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        body.statements.len(),
        2,
        "should be only let and return (not let + s + return): {:?}",
        body.statements
    );
}

#[test]
fn byte_size_literal_is_not_shadowed_by_int_literal() {
    assert_eq!(let_literal(&fn_src("512KB")), Literal::ByteSize(512 * 1024));
    assert_eq!(
        let_literal(&fn_src("2MB")),
        Literal::ByteSize(2 * 1024 * 1024)
    );
}

#[test]
fn uuid_literal_is_not_shadowed_by_int_literal() {
    let expected = "550e8400-e29b-41d4-a716-446655440000";
    match let_literal(&fn_src(expected)) {
        Literal::Uuid(u) => assert_eq!(u.to_string(), expected),
        other => panic!("expected Uuid, got {:?}", other),
    }
}

#[test]
fn plain_ints_floats_and_hex_still_parse() {
    assert_eq!(let_literal(&fn_src("10")), Literal::Int(10));
    assert_eq!(let_literal(&fn_src("1.5")), Literal::Float(1.5));
    assert_eq!(let_literal(&fn_src("0x1F")), Literal::Int(31));
    assert_eq!(let_literal(&fn_src("true")), Literal::Bool(true));
}

#[test]
fn duration_rejects_space_before_unit() {
    // `${ }` is deliberate: `10 s` must not become a duration, because then
    // `size: 20 spacing: 4` would swallow the number together with the next property.
    let program = parse(&fn_src("10 s")).expect("this still parses (as an int and an identifier)");
    let body = program
        .items
        .iter()
        .find_map(|i| match i {
            TopLevelItem::Function(f) => Some(&f.body),
            _ => None,
        })
        .unwrap();
    let first = body.statements.first().expect("statement");
    match first {
        Statement::Let(l) => assert!(
            matches!(
                l.value,
                Expr::Literal(ref lit) if matches!(lit.value, Literal::Int(10))
            ),
            "should be Int(10), not Duration: {:?}",
            l.value
        ),
        other => panic!("expected Let, got {:?}", other),
    }
}

#[test]
fn adjacent_inline_ui_properties_are_not_swallowed_as_duration() {
    // Direct regression for the `${ }` hazard: `20` and `4` must be two separate properties.
    let src = "agent T\n\nui Main = Screen \"x\" {\n    Column {\n        Text \"hi\" size: 20 spacing: 4\n        Text \"y\" margin: 8 min_width: 3\n    }\n}\n";
    let program = parse(src).expect("parse should succeed");
    let ui = program
        .items
        .iter()
        .find_map(|i| match i {
            TopLevelItem::Ui(u) => Some(u),
            _ => None,
        })
        .unwrap();

    let UiStatement::Element(column) = &ui.screen.body[0] else {
        panic!("expected Column");
    };
    let children = column.children.as_ref().expect("children");

    let props: Vec<Vec<(&str, i64)>> = children
        .iter()
        .map(|child| {
            let UiStatement::Element(el) = child else {
                panic!("expected Element");
            };
            el.modifiers
                .iter()
                .map(|m| match m {
                    aec_ast::ElementModifier::Property(p) => {
                        let Expr::Literal(lit) = &p.value else {
                            panic!("expected Literal");
                        };
                        let Literal::Int(n) = lit.value else {
                            panic!("expected Int, got {:?}", lit.value);
                        };
                        (p.name.name.as_str(), n)
                    }
                    other => panic!("expected Property, got {:?}", other),
                })
                .collect()
        })
        .collect();

    assert_eq!(
        props,
        vec![
            vec![("size", 20), ("spacing", 4)],
            vec![("margin", 8), ("min_width", 3)],
        ]
    );
}

#[test]
fn durations_work_in_model_properties_and_array_elements() {
    let src = "agent T\n\nmodel m {\n    timeout: 10s\n    window: 512KB\n    retries: 3\n}\n";
    let program = parse(src).expect("parse should succeed");
    let model = program
        .items
        .iter()
        .find_map(|i| match i {
            TopLevelItem::Model(m) => Some(m),
            _ => None,
        })
        .unwrap();

    let values: Vec<String> = model
        .fields
        .iter()
        .map(|f| match f {
            aec_ast::ModelField::Property(p) => format!("{:?}", p.value),
            other => format!("{:?}", other),
        })
        .collect();

    assert!(values[0].contains("Duration"), "timeout: {:?}", values[0]);
    assert!(values[1].contains("ByteSize"), "window: {:?}", values[1]);
    assert_eq!(values[2], "Int(3)");

    // An array of durations
    let arr_src = fn_src("[1s, 2s]");
    let program = parse(&arr_src).expect("an array with durations should parse");
    assert!(program.items.iter().any(|i| matches!(i, TopLevelItem::Function(_))));
}
