//! Lambda parsing: `x => body`, `x, y => body`, `=> body`.
//!
//! `lambda_expr` is tried first inside `primary_expr`, so these tests also guard
//! that an ordinary identifier / call is not mistaken for a lambda.

use aec_ast::{Expr, Statement, TopLevelItem};
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
        "agent Test\n\nfn f() -> int {{\n    let v = {}\n    return 1\n}}\n",
        expr
    )
}

fn params_of(expr: &Expr) -> Vec<String> {
    match expr {
        Expr::Lambda(l) => l.params.iter().map(|p| p.name.clone()).collect(),
        other => panic!("expected a lambda, got {:?}", other),
    }
}

#[test]
fn single_param_lambda_parses() {
    let expr = first_let_expr(&fn_src("x => x + 1"));
    assert_eq!(params_of(&expr), vec!["x"]);
    assert!(matches!(&expr, Expr::Lambda(l) if matches!(l.body, Expr::Binary(_))));
}

#[test]
fn zero_param_lambda_parses() {
    let expr = first_let_expr(&fn_src("=> 42"));
    assert!(params_of(&expr).is_empty());
}

#[test]
fn multi_param_lambda_parses() {
    let expr = first_let_expr(&fn_src("x, y => x + y"));
    assert_eq!(params_of(&expr), vec!["x", "y"]);
}

#[test]
fn lambda_body_can_be_nested() {
    let expr = first_let_expr(&fn_src("x => y => x"));
    match expr {
        Expr::Lambda(outer) => {
            assert!(matches!(outer.body, Expr::Lambda(_)));
        }
        other => panic!("expected a lambda, got {:?}", other),
    }
}

#[test]
fn lambda_works_as_a_call_argument() {
    // Without parentheses it is a direct lambda argument.
    match first_let_expr(&fn_src("g(1, x => x)")) {
        Expr::Call(call) => {
            assert_eq!(call.args.len(), 2);
            assert!(
                matches!(call.args[1].value, Expr::Lambda(_)),
                "got {:?}",
                call.args[1].value
            );
        }
        other => panic!("expected a call, got {:?}", other),
    }

    // Parentheses simply add a `Paren` wrapper around it.
    match first_let_expr(&fn_src("g(1, (x => x))")) {
        Expr::Call(call) => match &call.args[1].value {
            Expr::Paren(p) => assert!(matches!(p.inner, Expr::Lambda(_))),
            other => panic!("expected a parenthesized lambda, got {:?}", other),
        },
        other => panic!("expected a call, got {:?}", other),
    }
}

#[test]
fn multi_param_lambda_works_as_a_call_argument() {
    match first_let_expr(&fn_src("g(1, x, y => x)")) {
        Expr::Call(call) => {
            assert_eq!(call.args.len(), 2);
            assert!(
                matches!(call.args[1].value, Expr::Lambda(_)),
                "got {:?}",
                call.args[1].value
            );
        }
        other => panic!("expected a call, got {:?}", other),
    }
}

#[test]
fn a_plain_identifier_is_not_a_lambda() {
    let expr = first_let_expr(&fn_src("f"));
    assert!(matches!(expr, Expr::Identifier(_)), "got {:?}", expr);
}

#[test]
fn a_call_is_not_a_lambda() {
    let expr = first_let_expr(&fn_src("g(1, 2)"));
    assert!(matches!(expr, Expr::Call(_)), "got {:?}", expr);
}

#[test]
fn a_member_access_is_not_a_lambda() {
    let expr = first_let_expr(&fn_src("config.value"));
    assert!(matches!(expr, Expr::Member(_)), "got {:?}", expr);
}

#[test]
fn return_can_carry_a_lambda() {
    let program = parse("agent Test\n\nfn f() -> int {\n    return x => x + 1\n}\n")
        .expect("parse should succeed");
    let f = match &program.items[0] {
        TopLevelItem::Function(f) => f,
        _ => panic!("expected function"),
    };
    match &f.body.statements[0] {
        Statement::Return(r) => match r.value.as_ref() {
            Some(Expr::Lambda(_)) => {}
            other => panic!("expected a lambda, got {:?}", other),
        },
        other => panic!("expected return, got {:?}", other),
    }
}

#[test]
fn lambda_can_close_over_an_outer_variable() {
    let program = parse(
        "agent Test\n\nfn f(n: int) -> int {\n    let add = x => x + n\n    return add(1)\n}\n",
    )
    .expect("parse should succeed");
    assert!(matches!(&program.items[0], TopLevelItem::Function(_)));
}

#[test]
fn function_is_a_type_keyword() {
    let program = parse(
        "agent Test\n\nfn apply(f: function, v: int) -> function {\n    return f\n}\n",
    )
    .expect("parse should succeed");
    let f = match &program.items[0] {
        TopLevelItem::Function(f) => f,
        _ => panic!("expected function"),
    };
    assert!(matches!(f.params[0].ty, aec_ast::TypeExpr::Function));
    assert!(matches!(
        f.return_type,
        Some(aec_ast::TypeExpr::Function)
    ));
}
