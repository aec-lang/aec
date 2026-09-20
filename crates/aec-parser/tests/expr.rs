use aec_ast::{TopLevelItem, TypeExpr};
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
