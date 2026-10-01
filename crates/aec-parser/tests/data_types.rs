use aec_ast::{Expr, Literal, MatchBody, Pattern, Statement, TopLevelItem, TypeExpr};
use aec_parser::parse;

#[test]
fn parses_struct_declaration() {
    let program = parse("agent Test\nstruct User { name: string age: int }\n")
        .expect("should parse struct");

    let TopLevelItem::Struct(declaration) = &program.items[0] else {
        panic!("expected struct");
    };
    assert!(!declaration.is_public);
    assert_eq!(declaration.name.name, "User");
    assert_eq!(declaration.span.start.line, 2);
    assert_eq!(declaration.fields.len(), 2);
    assert_eq!(declaration.fields[0].name.name, "name");
    assert_eq!(declaration.fields[0].ty, TypeExpr::String);
    assert_eq!(declaration.fields[1].name.name, "age");
    assert_eq!(declaration.fields[1].ty, TypeExpr::Int);
    assert!(!declaration.fields[0].span.is_empty());
    assert!(!declaration.fields[1].span.is_empty());
}

#[test]
fn parses_enum_declaration_with_tuple_payload() {
    let program = parse("agent Test\nenum Role { Admin Member Suspended(string) }\n")
        .expect("should parse enum");

    let TopLevelItem::Enum(declaration) = &program.items[0] else {
        panic!("expected enum");
    };
    assert!(!declaration.is_public);
    assert_eq!(declaration.name.name, "Role");
    assert_eq!(declaration.span.start.line, 2);
    assert_eq!(declaration.variants.len(), 3);
    assert_eq!(declaration.variants[0].name.name, "Admin");
    assert_eq!(declaration.variants[0].payload, None);
    assert_eq!(declaration.variants[1].name.name, "Member");
    assert_eq!(declaration.variants[1].payload, None);
    assert_eq!(declaration.variants[2].name.name, "Suspended");
    assert_eq!(declaration.variants[2].payload, Some(TypeExpr::String));
    assert!(!declaration.variants[2].span.is_empty());
}

#[test]
fn keeps_named_types_in_fields_and_enum_payloads() {
    let program = parse(
        "agent Test\nstruct Wrapper { role: Role }\nenum Access { Role(Role) }\n",
    )
    .expect("should parse named types");

    let TopLevelItem::Struct(struct_decl) = &program.items[0] else {
        panic!("expected struct");
    };
    let TopLevelItem::Enum(enum_decl) = &program.items[1] else {
        panic!("expected enum");
    };
    assert!(matches!(
        &struct_decl.fields[0].ty,
        TypeExpr::Named(identifier) if identifier.name == "Role"
    ));
    assert!(matches!(
        &enum_decl.variants[0].payload,
        Some(TypeExpr::Named(identifier)) if identifier.name == "Role"
    ));
}

#[test]
fn parses_public_struct_and_enum_declarations() {
    let program = parse(
        "agent Test\npub struct PublicUser { name: string }\npub enum PublicRole { User }\n",
    )
    .expect("should parse public declarations");

    let TopLevelItem::Struct(struct_decl) = &program.items[0] else {
        panic!("expected struct");
    };
    let TopLevelItem::Enum(enum_decl) = &program.items[1] else {
        panic!("expected enum");
    };
    assert!(struct_decl.is_public);
    assert!(enum_decl.is_public);
}

#[test]
fn keeps_struct_and_enum_constructors_as_expressions() {
    let program = parse(
        r#"agent Test
fn make() {
    let user = User(name: "Ada", age: 42)
    let object = { name: "Ada", age: 42 }
    let admin = Role.Admin
    let suspended = Role.Suspended("x")
}
"#,
    )
    .expect("should parse expressions");

    let TopLevelItem::Function(function) = &program.items[0] else {
        panic!("expected function");
    };
    let values = function
        .body
        .statements
        .iter()
        .filter_map(|statement| match statement {
            Statement::Let(binding) => Some(&binding.value),
            _ => None,
        })
        .collect::<Vec<_>>();

    let Expr::Call(user_call) = values[0] else {
        panic!("expected call expression");
    };
    assert!(matches!(user_call.callee, Expr::Identifier(ref id) if id.name == "User"));
    assert_eq!(user_call.args.len(), 2);
    assert_eq!(user_call.args[0].name.as_ref().unwrap().name, "name");
    assert_eq!(user_call.args[1].name.as_ref().unwrap().name, "age");
    assert!(matches!(values[1], Expr::Object(_)));
    let Expr::Member(admin) = values[2] else {
        panic!("expected member expression");
    };
    assert!(matches!(admin.object, Expr::Identifier(ref id) if id.name == "Role"));
    assert_eq!(admin.property.name, "Admin");
    let Expr::Call(suspended) = values[3] else {
        panic!("expected call expression");
    };
    let Expr::Member(callee) = &suspended.callee else {
        panic!("expected member callee");
    };
    assert_eq!(callee.property.name, "Suspended");
    assert_eq!(suspended.args.len(), 1);
}

#[test]
fn parses_enum_variant_patterns_and_nested_payload_patterns() {
    let program = parse(
        r#"agent Test
fn classify(role: Role) -> int {
    match role {
        Role.Admin -> 1
        Role.Suspended(reason) -> 2
        _ -> 0
    }
}
"#,
    )
    .expect("should parse enum patterns");

    let TopLevelItem::Function(function) = &program.items[0] else {
        panic!("expected function");
    };
    let Statement::Expr(Expr::Match(match_expr)) = &function.body.statements[0] else {
        panic!("expected match expression");
    };
    assert_eq!(match_expr.arms.len(), 3);
    let Pattern::EnumVariant(admin) = &match_expr.arms[0].pattern else {
        panic!("expected enum variant pattern");
    };
    assert_eq!(admin.path[0].name, "Role");
    assert_eq!(admin.path[1].name, "Admin");
    assert_eq!(admin.payload, None);
    let Pattern::EnumVariant(suspended) = &match_expr.arms[1].pattern else {
        panic!("expected enum variant pattern");
    };
    let Some(Pattern::Identifier(reason)) = suspended.payload.as_deref() else {
        panic!("expected nested identifier pattern");
    };
    assert_eq!(reason.name, "reason");
    assert!(matches!(match_expr.arms[2].pattern, Pattern::Wildcard(_)));
}

#[test]
fn parses_recursively_nested_enum_variant_patterns() {
    let program = parse(
        r#"agent Test
fn reason(value: Wrapper) -> string {
    match value {
        Wrapper.Wrapped(Role.Suspended(reason)) -> reason
        _ -> ""
    }
}
"#,
    )
    .expect("should parse nested enum patterns");

    let TopLevelItem::Function(function) = &program.items[0] else {
        panic!("expected function");
    };
    let Statement::Expr(Expr::Match(match_expr)) = &function.body.statements[0] else {
        panic!("expected match expression");
    };
    let Pattern::EnumVariant(outer) = &match_expr.arms[0].pattern else {
        panic!("expected outer enum pattern");
    };
    assert_eq!(outer.path[0].name, "Wrapper");
    assert_eq!(outer.path[1].name, "Wrapped");
    let Some(Pattern::EnumVariant(inner)) = outer.payload.as_deref() else {
        panic!("expected inner enum pattern");
    };
    assert_eq!(inner.path[0].name, "Role");
    assert_eq!(inner.path[1].name, "Suspended");
    let Some(Pattern::Identifier(reason)) = inner.payload.as_deref() else {
        panic!("expected identifier pattern");
    };
    assert_eq!(reason.name, "reason");
    assert!(matches!(match_expr.arms[0].body, MatchBody::Expr(_)));
}

#[test]
fn preserves_wildcard_literal_and_identifier_patterns() {
    let program = parse(
        r#"agent Test
fn classify(value: int) -> int {
    match value {
        42 -> 1
        other -> 2
        _ -> 0
    }
}
"#,
    )
    .expect("should parse legacy patterns");

    let TopLevelItem::Function(function) = &program.items[0] else {
        panic!("expected function");
    };
    let Statement::Expr(Expr::Match(match_expr)) = &function.body.statements[0] else {
        panic!("expected match expression");
    };
    assert!(matches!(
        &match_expr.arms[0].pattern,
        Pattern::Literal(Literal::Int(42))
    ));
    let Pattern::Identifier(identifier) = &match_expr.arms[1].pattern else {
        panic!("expected identifier pattern");
    };
    assert_eq!(identifier.name, "other");
    assert!(matches!(match_expr.arms[2].pattern, Pattern::Wildcard(_)));
}

#[test]
fn parses_struct_patterns_with_nested_patterns() {
    let program = parse(
        r#"agent Test
fn inspect(value: User) -> string {
    match value {
        User { name: name, age: age } -> name
        _ -> ""
    }
}
"#,
    )
    .expect("should parse struct patterns");

    let TopLevelItem::Function(function) = &program.items[0] else {
        panic!("expected function");
    };
    let Statement::Expr(Expr::Match(match_expr)) = &function.body.statements[0] else {
        panic!("expected match expression");
    };
    let Pattern::Struct(pattern) = &match_expr.arms[0].pattern else {
        panic!("expected struct pattern");
    };
    assert_eq!(pattern.path[0].name, "User");
    assert_eq!(pattern.fields.len(), 2);
    assert!(matches!(pattern.fields[0].pattern.as_ref(), Pattern::Identifier(_)));
    assert!(matches!(pattern.fields[1].pattern.as_ref(), Pattern::Identifier(_)));
}

#[test]
fn rejects_named_and_multi_type_enum_payloads() {
    assert!(parse("agent Test\nenum Role { Suspended(reason: string) }\n").is_err());
    assert!(parse("agent Test\nenum Role { Pair(int, string) }\n").is_err());
}
