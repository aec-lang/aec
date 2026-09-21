//! Parsing of the `?` operator and the `Result(ok, err)` type.

use aec_ast::{Expr, Statement, TopLevelItem, TypeExpr};
use aec_parser::parse;

fn first_let_expr(src: &str) -> Expr {
    let program = parse(src).expect("parse should succeed");
    let f = program
        .items
        .iter()
        .find_map(|i| match i {
            TopLevelItem::Function(f) => Some(f),
            _ => None,
        })
        .expect("function");
    f.body
        .statements
        .iter()
        .find_map(|s| match s {
            Statement::Let(l) => Some(l.value.clone()),
            _ => None,
        })
        .expect("a Let statement")
}

fn fn_src(expr: &str) -> String {
    format!(
        "agent Test\n\nfn f() -> int {{\n    let v = {}\n    return v\n}}\n",
        expr
    )
}

#[test]
fn try_is_a_postfix_operator() {
    let expr = first_let_expr(&fn_src("compute()?"));
    assert!(matches!(expr, Expr::Try(_)), "got {:?}", expr);
}

#[test]
fn try_applies_to_a_member_chain() {
    let expr = first_let_expr(&fn_src("resp.value?"));
    match expr {
        Expr::Try(t) => assert!(matches!(t.inner, Expr::Member(_))),
        other => panic!("expected try, got {:?}", other),
    }
}

#[test]
fn try_binds_tighter_than_an_operator() {
    // `ok(1)? + 2` must parse as `(ok(1)?) + 2`
    let expr = first_let_expr(&fn_src("ok(1)? + 2"));
    match expr {
        Expr::Binary(b) => assert!(matches!(b.left, Expr::Try(_))),
        other => panic!("expected binary add, got {:?}", other),
    }
}

#[test]
fn optional_type_annotation_still_parses() {
    // Regression guard: `?` in type position must not be eaten by the try operator.
    let program = parse("agent Test\n\nfn f() -> int {\n    let x: int? = none\n    return 1\n}\n")
        .expect("parse should succeed");
    let f = match &program.items[0] {
        TopLevelItem::Function(f) => f,
        _ => panic!("expected function"),
    };
    match &f.body.statements[0] {
        Statement::Let(l) => assert!(matches!(l.ty, Some(TypeExpr::Optional(_)))),
        other => panic!("expected let, got {:?}", other),
    }
}

/// Regression guard: the optional marker used to be an anonymous literal, so
/// `int?` was silently built as plain `int`.
#[test]
fn optional_return_type_is_preserved() {
    let program = parse("agent Test\n\nfn f() -> int? {\n    return none\n}\n")
        .expect("parse should succeed");
    let f = match &program.items[0] {
        TopLevelItem::Function(f) => f,
        _ => panic!("expected function"),
    };
    match &f.return_type {
        Some(TypeExpr::Optional(inner)) => assert!(matches!(**inner, TypeExpr::Int)),
        other => panic!("expected Optional(int), got {:?}", other),
    }
}

#[test]
fn optional_parameter_type_is_preserved() {
    let program =
        parse("agent Test\n\nfn f(x: string?) -> int {\n    return 1\n}\n").expect("parse");
    let f = match &program.items[0] {
        TopLevelItem::Function(f) => f,
        _ => panic!("expected function"),
    };
    assert!(matches!(f.params[0].ty, TypeExpr::Optional(_)));
}

/// Regression guard: `none` used to produce no parse pair at all, so
/// `let x = none` failed with "expected base expression".
#[test]
fn none_is_usable_as_a_value() {
    let program = parse("agent Test\n\nfn f() -> int {\n    let x = none\n    return 1\n}\n")
        .expect("parse should succeed");
    let f = match &program.items[0] {
        TopLevelItem::Function(f) => f,
        _ => panic!("expected function"),
    };
    match &f.body.statements[0] {
        Statement::Let(l) => match &l.value {
            Expr::Literal(lit) => assert_eq!(lit.value, aec_ast::Literal::None),
            other => panic!("expected none literal, got {:?}", other),
        },
        other => panic!("expected let, got {:?}", other),
    }
}

#[test]
fn result_type_annotation_parses() {
    let program = parse("agent Test\n\nfn f() -> Result(int, string) {\n    return ok(1)\n}\n")
        .expect("parse should succeed");
    let f = match &program.items[0] {
        TopLevelItem::Function(f) => f,
        _ => panic!("expected function"),
    };
    match &f.return_type {
        Some(TypeExpr::Result(ok, err)) => {
            assert!(matches!(**ok, TypeExpr::Int));
            assert!(matches!(**err, TypeExpr::String));
        }
        other => panic!("expected Result type, got {:?}", other),
    }
}

#[test]
fn result_type_works_in_a_parameter() {
    let program = parse(
        "agent Test\n\nfn f(r: Result(int, string)) -> int {\n    return 1\n}\n",
    )
    .expect("parse should succeed");
    let f = match &program.items[0] {
        TopLevelItem::Function(f) => f,
        _ => panic!("expected function"),
    };
    assert!(matches!(f.params[0].ty, TypeExpr::Result(_, _)));
}

#[test]
fn named_type_called_result_is_still_a_named_type() {
    // `Result` without parentheses is an ordinary named type, not a type error.
    let program = parse("agent Test\n\nfn f(x: Result) -> int {\n    return 1\n}\n")
        .expect("parse should succeed");
    let f = match &program.items[0] {
        TopLevelItem::Function(f) => f,
        _ => panic!("expected function"),
    };
    assert!(matches!(f.params[0].ty, TypeExpr::Named(_)));
}
