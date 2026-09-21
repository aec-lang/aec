//! String interpolation: `"hi {name}!"`.
//!
//! A `{...}` segment becomes `InterpPart::Expr` **only** when its text parses as
//! an expression. Otherwise the braces stay literal text — that keeps payload
//! strings such as `json.parse("{ \"a\": 1 }")` working.

use aec_ast::{BinaryOp, Expr, InterpPart, Literal, Statement, TopLevelItem};
use aec_parser::{parse, parse_expr};

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
        "agent Test\n\nfn f(name: string, a: int) -> string {{\n    let s = {}\n    return s\n}}\n",
        expr
    )
}

fn parts_of(lit: Literal) -> Vec<InterpPart> {
    match lit {
        Literal::Interpolated(parts) => parts,
        other => panic!("expected Interpolated, got {:?}", other),
    }
}

#[test]
fn plain_string_is_just_a_string() {
    let lit = let_literal(&fn_src("\"hello\""));
    assert_eq!(lit, Literal::String("hello".to_string()));
}

#[test]
fn single_expression_is_interpolated() {
    let parts = parts_of(let_literal(&fn_src("\"{name}\"")));
    assert_eq!(parts.len(), 1);
    match &parts[0] {
        InterpPart::Expr(Expr::Identifier(id)) => assert_eq!(id.name, "name"),
        other => panic!("expected an identifier, got {:?}", other),
    }
}

#[test]
fn text_around_expressions_is_kept() {
    let parts = parts_of(let_literal(&fn_src("\"hi {name}! total {a}\"")));
    assert_eq!(parts.len(), 4);
    assert_eq!(parts[0], InterpPart::Text("hi ".to_string()));
    assert!(matches!(parts[1], InterpPart::Expr(_)));
    assert_eq!(parts[2], InterpPart::Text("! total ".to_string()));
    assert!(matches!(parts[3], InterpPart::Expr(_)));
}

#[test]
fn arbitrary_expression_inside_braces_is_parsed() {
    let parts = parts_of(let_literal(&fn_src("\"total: {a + 1}\"")));
    match &parts[1] {
        InterpPart::Expr(Expr::Binary(b)) => assert_eq!(b.op, BinaryOp::Add),
        other => panic!("expected a binary expression, got {:?}", other),
    }
}

#[test]
fn braces_that_are_not_an_expression_stay_literal() {
    // The decoded text is { "a": 1 } — not an expression, so no interpolation.
    let lit = let_literal(&fn_src("\"{ \\\"a\\\": 1 }\""));
    assert_eq!(lit, Literal::String("{ \"a\": 1 }".to_string()));
}

#[test]
fn empty_braces_stay_literal() {
    let lit = let_literal(&fn_src("\"{}\""));
    assert_eq!(lit, Literal::String("{}".to_string()));
}

#[test]
fn unbalanced_brace_stays_literal() {
    let lit = let_literal(&fn_src("\"a {b\""));
    assert_eq!(lit, Literal::String("a {b".to_string()));
}

#[test]
fn interpolation_works_inside_a_ui_text_element() {
    let src = "agent Test\n\nui Main = Screen \"T\" {\n    Text \"hi {name}\"\n    @name: string = \"x\"\n}\n";
    let program = parse(src).expect("parse should succeed");
    let ui = program
        .items
        .iter()
        .find_map(|i| match i {
            TopLevelItem::Ui(u) => Some(u),
            _ => None,
        })
        .expect("ui decl");
    let has_interp = ui.screen.body.iter().any(|s| match s {
        aec_ast::UiStatement::Element(el) => matches!(
            &el.primary_arg,
            Some(Expr::Literal(l)) if matches!(l.value, Literal::Interpolated(_))
        ),
        _ => false,
    });
    assert!(has_interp, "expected an interpolated primary arg");
}

#[test]
fn parse_expr_parses_a_standalone_expression() {
    let expr = parse_expr("1 + 2").expect("should parse");
    assert!(matches!(expr, Expr::Binary(b) if b.op == BinaryOp::Add));
}

#[test]
fn parse_expr_rejects_a_statement() {
    assert!(parse_expr("let x = 1").is_err());
}
