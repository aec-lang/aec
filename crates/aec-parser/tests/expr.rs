use aec_ast::{BinaryOp, Expr, Statement, TopLevelItem, TypeExpr};
use aec_parser::parse;

#[test]
fn parses_simple_fn() {
    let source = "agent Test\nfn hello() -> string {\n    return \"hi\"\n}\n";
    let program = parse(source).expect("should parse");
    assert_eq!(program.items.len(), 1);
    match &program.items[0] {
        TopLevelItem::Function(f) => {
            assert_eq!(f.name.name, "hello");
            assert!(f.return_type.is_some());
            assert_eq!(f.body.statements.len(), 1);
        }
        _ => panic!("expected function"),
    }
}

#[test]
fn parses_fn_with_params() {
    let source = "agent Test\nfn add(a: int, b: int) -> int {\n    return a + b\n}\n";
    let program = parse(source).expect("should parse");
    match &program.items[0] {
        TopLevelItem::Function(f) => {
            assert_eq!(f.params.len(), 2);
            assert_eq!(f.params[0].name.name, "a");
            assert!(matches!(f.params[0].ty, TypeExpr::Int));
            assert_eq!(f.params[1].name.name, "b");
        }
        _ => panic!("expected function"),
    }
}

#[test]
fn parses_let_stmt() {
    let source = "agent Test\nfn f() -> int {\n    let x = 5\n    return x\n}\n";
    let program = parse(source).expect("should parse");
    match &program.items[0] {
        TopLevelItem::Function(f) => {
            assert_eq!(f.body.statements.len(), 2);
        }
        _ => panic!("expected function"),
    }
}

#[test]
fn parses_let_with_type() {
    let source = "agent Test\nfn f() -> int {\n    let x: int = 5\n    return x\n}\n";
    let program = parse(source).expect("should parse");
    match &program.items[0] {
        TopLevelItem::Function(f) => {
            assert_eq!(f.body.statements.len(), 2);
        }
        _ => panic!("expected function"),
    }
}

#[test]
fn parses_fn_without_return_type() {
    let source = "agent Test\nfn greet() {\n    let x = 1\n}\n";
    let program = parse(source).expect("should parse");
    match &program.items[0] {
        TopLevelItem::Function(f) => {
            assert!(f.return_type.is_none());
        }
        _ => panic!("expected function"),
    }
}

#[test]
fn parses_fn_with_array_type() {
    let source = "agent Test\nfn f() -> [int] {\n    return [1, 2, 3]\n}\n";
    let program = parse(source).expect("should parse");
    match &program.items[0] {
        TopLevelItem::Function(f) => {
            assert!(matches!(f.return_type, Some(TypeExpr::Array(_))));
        }
        _ => panic!("expected function"),
    }
}

fn return_value(source: &str) -> Expr {
    match &program_of(source).items[0] {
        TopLevelItem::Function(f) => match &f.body.statements[0] {
            Statement::Return(r) => r.value.clone().expect("return should have a value"),
            other => panic!("expected return, got {:?}", other),
        },
        _ => panic!("expected function"),
    }
}

fn program_of(source: &str) -> aec_ast::Program {
    parse(source).expect("should parse")
}

#[test]
fn parses_logical_and_or_with_precedence() {
    // a and b or a  ==>  (a and b) or a   (and binds tighter than or)
    let expr = return_value("agent Test\nfn f(a: bool, b: bool) -> bool {\n    return a and b or a\n}\n");
    match expr {
        Expr::Binary(b) => {
            assert_eq!(b.op, BinaryOp::Or);
            assert!(matches!(&b.left, Expr::Binary(inner) if inner.op == BinaryOp::And));
        }
        other => panic!("expected binary or, got {:?}", other),
    }
}

#[test]
fn parses_plain_and_expression() {
    let expr = return_value("agent Test\nfn f(a: bool, b: bool) -> bool {\n    return a and b\n}\n");
    assert!(matches!(expr, Expr::Binary(b) if b.op == BinaryOp::And));
}

#[test]
fn parses_comparison_mixed_with_or() {
    // cpu > 85 or ram > 90
    let expr = return_value(
        "agent Test\nfn f(cpu: int, ram: int) -> bool {\n    return cpu > 85 or ram > 90\n}\n",
    );
    match expr {
        Expr::Binary(b) => {
            assert_eq!(b.op, BinaryOp::Or);
            assert!(matches!(&b.left, Expr::Binary(i) if i.op == BinaryOp::Gt));
            assert!(matches!(&b.right, Expr::Binary(i) if i.op == BinaryOp::Gt));
        }
        other => panic!("expected binary or, got {:?}", other),
    }
}

#[test]
fn identifier_starting_with_or_still_works() {
    // The variable name "orange" must not be confused with the or operator
    let expr =
        return_value("agent Test\nfn f(orange: int) -> int {\n    return orange + 1\n}\n");
    assert!(matches!(expr, Expr::Binary(b) if b.op == BinaryOp::Add));
}

fn first_let(source: &str) -> aec_ast::LetStmt {
    match &program_of(source).items[0] {
        TopLevelItem::Function(f) => match &f.body.statements[0] {
            Statement::Let(l) => l.clone(),
            other => panic!("expected let, got {:?}", other),
        },
        _ => panic!("expected function"),
    }
}

#[test]
fn let_binds_an_immutable_binding() {
    let l = first_let("agent Test\nfn f() -> int {\n    let x = 5\n    return x\n}\n");
    assert_eq!(l.name.name, "x");
    assert!(!l.mutable, "`let` must be immutable");
}

#[test]
fn var_binds_a_mutable_binding() {
    let l = first_let("agent Test\nfn f() -> int {\n    var x = 5\n    return x\n}\n");
    assert_eq!(l.name.name, "x");
    assert!(l.mutable, "`var` must be mutable");
}

#[test]
fn var_accepts_a_type_annotation() {
    let l = first_let("agent Test\nfn f() -> int {\n    var x: int = 5\n    return x\n}\n");
    assert!(l.mutable);
    assert!(matches!(l.ty, Some(TypeExpr::Int)));
}

/// `var` is a contextual keyword, not a reserved word: a variable actually named
/// `var` still binds and assigns as usual.
#[test]
fn var_is_not_a_reserved_word() {
    let program = program_of(
        "agent Test\nfn f() -> int {\n    let var = 5\n    var = 6\n    return var\n}\n",
    );
    match &program.items[0] {
        TopLevelItem::Function(f) => {
            assert_eq!(f.body.statements.len(), 3);
            assert!(matches!(&f.body.statements[1], Statement::Assign(_)));
        }
        _ => panic!("expected function"),
    }
}

#[test]
fn assign_to_var_is_accepted() {
    let program =
        program_of("agent Test\nfn f() -> int {\n    var x = 5\n    x = 6\n    return x\n}\n");
    match &program.items[0] {
        TopLevelItem::Function(f) => {
            assert_eq!(f.body.statements.len(), 3);
            assert!(matches!(&f.body.statements[1], Statement::Assign(_)));
        }
        _ => panic!("expected function"),
    }
}
