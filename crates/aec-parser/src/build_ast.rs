use crate::errors::build_error;
use crate::Rule;
use aec_ast::{
    AgentHeader, Argument, ArrayExpr, AssignOp, AssignStmt, AwaitExpr,
    BinaryExpr, BinaryOp, Block, CallExpr, ElseBranch, Expr, ForStmt,
    FunctionDecl, Identifier, IfStmt, ImportStmt, IndexExpr, LValue,
    LValueStep, LetStmt, Literal, LiteralExpr, MatchArm, MatchBody,
    MatchExpr, MemberExpr, ModelDecl, ModelField, ModelProperty,
    ObjectExpr, ObjectField, Parameter, ParenExpr, ParseError, Pattern,
    Position, Program, RetryBlock, RetryField, ReturnStmt, SecretValue,
    SecretsBlock, SecretsEntry, Span, Statement as AstStatement,
    TopLevelItem, TypeExpr, UnaryExpr, UnaryOp, WhileStmt,
};
use pest::iterators::Pair;

fn pair_span(pair: &Pair<Rule>) -> Span {
    let start_pos = pair.as_span().start_pos();
    let end_pos = pair.as_span().end_pos();
    let (start_line, start_col) = start_pos.line_col();
    let (end_line, end_col) = end_pos.line_col();
    Span::new(
        Position::new(start_line as u32, start_col as u32, start_pos.pos() as u32),
        Position::new(end_line as u32, end_col as u32, end_pos.pos() as u32),
    )
}

pub fn build_program(pair: Pair<Rule>) -> Result<Program, ParseError> {
    let span = pair_span(&pair);
    let mut inner = pair.into_inner();

    let header_pair = inner.next().ok_or_else(|| {
        build_error(span, "missing agent header")
    })?;
    let header = build_agent_header(header_pair)?;

    let mut items = Vec::new();
    for item_pair in inner {
        match item_pair.as_rule() {
            Rule::secrets_block => {
                items.push(TopLevelItem::Secrets(build_secrets_block(item_pair)?));
            }
            Rule::model_decl => {
                items.push(TopLevelItem::Model(build_model_decl(item_pair)?));
            }
            Rule::import_stmt => {
                items.push(TopLevelItem::Import(build_import_stmt(item_pair)?));
            }
            Rule::fn_decl => {
                items.push(TopLevelItem::Function(build_function_decl(item_pair)?));
            }
            Rule::EOI => break,
            _ => {}
        }
    }

    Ok(Program { header, items, span })
}

fn build_agent_header(pair: Pair<Rule>) -> Result<AgentHeader, ParseError> {
    let span = pair_span(&pair);
    let mut inner = pair.into_inner();
    let name_pair = inner.next().ok_or_else(|| {
        build_error(span, "agent header must have a name")
    })?;
    let name = build_identifier(name_pair)?;
    Ok(AgentHeader { name, span })
}

fn build_identifier(pair: Pair<Rule>) -> Result<Identifier, ParseError> {
    let span = pair_span(&pair);
    Ok(Identifier::new(pair.as_str(), span))
}

fn build_import_stmt(pair: Pair<Rule>) -> Result<ImportStmt, ParseError> {
    let span = pair_span(&pair);
    let mut inner = pair.into_inner();
    let path_pair = inner.next().ok_or_else(|| {
        build_error(span, "import needs a path")
    })?;
    let path = extract_string_literal(path_pair)?;

    let alias = inner.next().map(|alias_pair| {
        let id_pair = alias_pair.into_inner().next().unwrap();
        build_identifier(id_pair)
    }).transpose()?;

    Ok(ImportStmt { path, alias, span })
}

fn build_secrets_block(pair: Pair<Rule>) -> Result<SecretsBlock, ParseError> {
    let span = pair_span(&pair);
    let mut entries = Vec::new();
    for entry_pair in pair.into_inner() {
        if entry_pair.as_rule() == Rule::secrets_entry {
            entries.push(build_secrets_entry(entry_pair)?);
        }
    }
    Ok(SecretsBlock { entries, span })
}

fn build_secrets_entry(pair: Pair<Rule>) -> Result<SecretsEntry, ParseError> {
    let span = pair_span(&pair);
    let mut inner = pair.into_inner();
    let name_pair = inner.next().ok_or_else(|| {
        build_error(span, "secrets entry needs a name")
    })?;
    let name = build_identifier(name_pair)?;
    let value_pair = inner.next().ok_or_else(|| {
        build_error(span, "secrets entry needs a value")
    })?;
    let value = build_secret_value(value_pair)?;
    Ok(SecretsEntry { name, value, span })
}

fn build_secret_value(pair: Pair<Rule>) -> Result<SecretValue, ParseError> {
    let span = pair_span(&pair);
    match pair.as_rule() {
        Rule::env_call => {
            let mut inner = pair.into_inner();
            let arg_pair = inner.next().ok_or_else(|| {
                build_error(span, "env() needs an argument")
            })?;
            Ok(SecretValue::Env(extract_string_literal(arg_pair)?))
        }
        Rule::string_literal => {
            Ok(SecretValue::String(extract_string_literal(pair)?))
        }
        rule => Err(build_error(span, format!("invalid secret: {:?}", rule))),
    }
}

fn build_model_decl(pair: Pair<Rule>) -> Result<ModelDecl, ParseError> {
    let span = pair_span(&pair);
    let mut inner = pair.into_inner();
    let name_pair = inner.next().ok_or_else(|| {
        build_error(span, "model needs a name")
    })?;
    let name = build_identifier(name_pair)?;

    let mut fields = Vec::new();
    for field_pair in inner {
        match field_pair.as_rule() {
            Rule::model_property => {
                fields.push(ModelField::Property(build_model_property(field_pair)?));
            }
            Rule::retry_block => {
                fields.push(ModelField::Retry(build_retry_block(field_pair)?));
            }
            _ => {}
        }
    }
    Ok(ModelDecl { name, fields, span })
}

fn build_model_property(pair: Pair<Rule>) -> Result<ModelProperty, ParseError> {
    let span = pair_span(&pair);
    let mut inner = pair.into_inner();
    let name_pair = inner.next().ok_or_else(|| {
        build_error(span, "property needs a name")
    })?;
    let name = build_identifier(name_pair)?;
    let value_pair = inner.next().ok_or_else(|| {
        build_error(span, "property needs a value")
    })?;
    let value = build_literal(value_pair)?;
    Ok(ModelProperty { name, value, span })
}

fn build_retry_block(pair: Pair<Rule>) -> Result<RetryBlock, ParseError> {
    let span = pair_span(&pair);
    let mut fields = Vec::new();
    for field_pair in pair.into_inner() {
        match field_pair.as_rule() {
            Rule::retry_property => {
                fields.push(RetryField::Property(build_model_property(field_pair)?));
            }
            _ => {}
        }
    }
    Ok(RetryBlock { fields, span })
}

fn build_literal(pair: Pair<Rule>) -> Result<Literal, ParseError> {
    let span = pair_span(&pair);
    match pair.as_rule() {
        Rule::string_literal => Ok(Literal::String(extract_string_literal(pair)?)),
        Rule::raw_string => {
            let s = pair.as_str();
            Ok(Literal::RawString(s[2..s.len()-1].to_string()))
        }
        Rule::int_literal => {
            let s = pair.as_str();
            let n = if let Some(hex) = s.strip_prefix("0x") {
                i64::from_str_radix(hex, 16)
            } else {
                s.parse::<i64>()
            }.map_err(|e| build_error(span, format!("invalid int: {}", e)))?;
            Ok(Literal::Int(n))
        }
        Rule::float_literal => {
            let n = pair.as_str().parse::<f64>()
                .map_err(|e| build_error(span, format!("invalid float: {}", e)))?;
            Ok(Literal::Float(n))
        }
        Rule::bool_literal => Ok(Literal::Bool(pair.as_str() == "true")),
        Rule::duration_literal => {
            let mut inner = pair.into_inner();
            let value_pair = inner.next().unwrap();
            let unit_pair = inner.next().unwrap();
            let value = value_pair.as_str().parse::<u64>()
                .map_err(|e| build_error(span, format!("invalid duration: {}", e)))?;
            let unit = match unit_pair.as_str() {
                "ms" => aec_ast::DurationUnit::Milliseconds,
                "s" => aec_ast::DurationUnit::Seconds,
                "m" => aec_ast::DurationUnit::Minutes,
                "h" => aec_ast::DurationUnit::Hours,
                "d" => aec_ast::DurationUnit::Days,
                other => return Err(build_error(span, format!("unknown unit: {}", other))),
            };
            Ok(Literal::Duration(aec_ast::Duration { value, unit }))
        }
        Rule::byte_size_literal => {
            let mut inner = pair.into_inner();
            let value_pair = inner.next().unwrap();
            let unit_pair = inner.next().unwrap();
            let value = value_pair.as_str().parse::<u64>()
                .map_err(|e| build_error(span, format!("invalid size: {}", e)))?;
            let multiplier = match unit_pair.as_str() {
                "KB" => 1024u64,
                "MB" => 1024*1024,
                "GB" => 1024*1024*1024,
                other => return Err(build_error(span, format!("unknown unit: {}", other))),
            };
            Ok(Literal::ByteSize(value * multiplier))
        }
        Rule::uuid_literal => {
            let uuid = uuid::Uuid::parse_str(pair.as_str())
                .map_err(|e| build_error(span, format!("invalid uuid: {}", e)))?;
            Ok(Literal::Uuid(uuid))
        }
        rule => Err(build_error(span, format!("unsupported literal: {:?}", rule))),
    }
}

fn extract_string_literal(pair: Pair<Rule>) -> Result<String, ParseError> {
    let span = pair_span(&pair);
    let s = pair.as_str();
    if !s.starts_with('"') || !s.ends_with('"') || s.len() < 2 {
        return Err(build_error(span, "invalid string literal"));
    }
    let inner = &s[1..s.len()-1];
    let unescaped = inner
        .replace("\\n", "\n")
        .replace("\\t", "\t")
        .replace("\\r", "\r")
        .replace("\\\"", "\"")
        .replace("\\\\", "\\");
    Ok(unescaped)
}

// ============================================================
// Expressions
// ============================================================

pub fn build_expr(pair: Pair<Rule>) -> Result<Expr, ParseError> {
    let span = pair_span(&pair);
    match pair.as_rule() {
        Rule::expr => {
            let inner = pair.into_inner().next().unwrap();
            build_expr(inner)
        }
        Rule::comparison_expr => build_binary_chain(pair, &[
            ("==", BinaryOp::Eq),
            ("!=", BinaryOp::Neq),
            ("<=", BinaryOp::Lte),
            (">=", BinaryOp::Gte),
            ("<", BinaryOp::Lt),
            (">", BinaryOp::Gt),
        ]),
        Rule::additive_expr => build_binary_chain(pair, &[
            ("+", BinaryOp::Add),
            ("-", BinaryOp::Sub),
        ]),
        Rule::multiplicative_expr => build_binary_chain(pair, &[
            ("*", BinaryOp::Mul),
            ("/", BinaryOp::Div),
            ("%", BinaryOp::Mod),
        ]),
        Rule::unary_expr => {
            let mut inner = pair.into_inner();
            let first = inner.next().ok_or_else(|| {
                build_error(span, "expected expression")
            })?;

            if first.as_rule() == Rule::unary_op {
                let op = match first.as_str() {
                    "-" => UnaryOp::Neg,
                    "not" | "!" => UnaryOp::Not,
                    other => return Err(build_error(span, format!("unknown: {}", other))),
                };
                let operand_pair = inner.next().ok_or_else(|| {
                    build_error(span, "expected operand")
                })?;
                let operand = build_postfix(operand_pair)?;
                Ok(Expr::Unary(Box::new(UnaryExpr { op, operand, span })))
            } else {
                build_postfix(first)
            }
        }
        Rule::match_expr => build_match_expr(pair),
        rule => Err(build_error(span, format!("unsupported expr: {:?}", rule))),
    }
}

fn build_postfix(pair: Pair<Rule>) -> Result<Expr, ParseError> {
    match pair.as_rule() {
        Rule::postfix_expr => {
            let span = pair_span(&pair);
            let mut inner = pair.into_inner();
            let base = inner.next().ok_or_else(|| {
                build_error(span, "expected base expression")
            })?;
            let mut current = build_primary(base)?;

            for op in inner {
                match op.as_rule() {
                    Rule::call_op => {
                        let mut args = Vec::new();
                        for arg_pair in op.into_inner() {
                            if arg_pair.as_rule() == Rule::arg_list {
                                for a in arg_pair.into_inner() {
                                    if a.as_rule() == Rule::argument {
                                        args.push(build_argument(a)?);
                                    }
                                }
                            }
                        }
                        current = Expr::Call(Box::new(CallExpr {
                            callee: current,
                            args,
                            span,
                        }));
                    }
                    Rule::member_op => {
                        let id_pair = op.into_inner().next().unwrap();
                        let prop = build_identifier(id_pair)?;
                        current = Expr::Member(Box::new(MemberExpr {
                            object: current,
                            property: prop,
                            span,
                        }));
                    }
                    Rule::index_op => {
                        let idx_pair = op.into_inner().next().unwrap();
                        let idx = build_expr(idx_pair)?;
                        current = Expr::Index(Box::new(IndexExpr {
                            object: current,
                            index: idx,
                            span,
                        }));
                    }
                    _ => {}
                }
            }
            Ok(current)
        }
        Rule::match_expr => build_match_expr(pair),
        _ => build_primary(pair),
    }
}

fn build_argument(pair: Pair<Rule>) -> Result<Argument, ParseError> {
    let span = pair_span(&pair);
    let mut inner = pair.into_inner();
    let first = inner.next().ok_or_else(|| {
        build_error(span, "expected argument")
    })?;

    if first.as_rule() == Rule::identifier {
        let name = build_identifier(first)?;
        let value_pair = inner.next().ok_or_else(|| {
            build_error(span, "named argument needs a value")
        })?;
        let value = build_expr(value_pair)?;
        Ok(Argument { name: Some(name), value, span })
    } else {
        let value = build_expr(first)?;
        Ok(Argument { name: None, value, span })
    }
}

fn build_primary(pair: Pair<Rule>) -> Result<Expr, ParseError> {
    let span = pair_span(&pair);
    match pair.as_rule() {
        Rule::paren_expr => {
            let inner = pair.into_inner().next().unwrap();
            let inner_expr = build_expr(inner)?;
            Ok(Expr::Paren(Box::new(ParenExpr { inner: inner_expr, span })))
        }
        Rule::array_expr => {
            let mut elements = Vec::new();
            for e in pair.into_inner() {
                elements.push(build_expr(e)?);
            }
            Ok(Expr::Array(Box::new(ArrayExpr { elements, span })))
        }
        Rule::object_expr => {
            let mut fields = Vec::new();
            for f in pair.into_inner() {
                if f.as_rule() == Rule::object_field {
                    let mut fi = f.into_inner();
                    let key = build_identifier(fi.next().unwrap())?;
                    let value = build_expr(fi.next().unwrap())?;
                    fields.push(ObjectField { key, value, span });
                }
            }
            Ok(Expr::Object(Box::new(ObjectExpr { fields, span })))
        }
        Rule::await_expr => {
            let inner = pair.into_inner().next().unwrap();
            let inner_expr = build_expr(inner)?;
            Ok(Expr::Await(Box::new(AwaitExpr { inner: inner_expr, span })))
        }
        Rule::match_expr => build_match_expr(pair),
        Rule::identifier => {
            let id = build_identifier(pair)?;
            Ok(Expr::Identifier(id))
        }
        Rule::string_literal | Rule::raw_string | Rule::int_literal
        | Rule::float_literal | Rule::bool_literal | Rule::duration_literal
        | Rule::byte_size_literal | Rule::uuid_literal => {
            let lit = build_literal(pair)?;
            Ok(Expr::Literal(Box::new(LiteralExpr { value: lit, span })))
        }
        rule => Err(build_error(span, format!("unsupported primary: {:?}", rule))),
    }
}

fn build_binary_chain(
    pair: Pair<Rule>,
    ops: &[(&str, BinaryOp)],
) -> Result<Expr, ParseError> {
    let span = pair_span(&pair);
    let mut inner = pair.into_inner();

    let first = inner.next().ok_or_else(|| {
        build_error(span, "expected expression")
    })?;
    let mut left = build_expr(first)?;

    let mut op_str: Option<String> = None;
    for p in inner {
        match p.as_rule() {
            Rule::comparison_op | Rule::additive_op | Rule::multiplicative_op => {
                op_str = Some(p.as_str().to_string());
            }
            _ => {
                let right = build_expr(p)?;
                let op_name = op_str.take().ok_or_else(|| {
                    build_error(span, "expected operator")
                })?;
                let op = ops.iter()
                    .find(|(s, _)| *s == op_name)
                    .map(|(_, o)| *o)
                    .ok_or_else(|| build_error(span, format!("unknown: {}", op_name)))?;
                left = Expr::Binary(Box::new(BinaryExpr {
                    left,
                    op,
                    right,
                    span,
                }));
            }
        }
    }

    Ok(left)
}

// ============================================================
// Match
// ============================================================

fn build_match_expr(pair: Pair<Rule>) -> Result<Expr, ParseError> {
    let span = pair_span(&pair);
    let mut inner = pair.into_inner();

    let scrut_pair = inner.next().ok_or_else(|| {
        build_error(span, "match needs an expression")
    })?;
    let scrutinee = build_expr(scrut_pair)?;

    let mut arms = Vec::new();
    for arm_pair in inner {
        if arm_pair.as_rule() == Rule::match_arm {
            arms.push(build_match_arm(arm_pair)?);
        }
    }

    Ok(Expr::Match(Box::new(MatchExpr {
        scrutinee,
        arms,
        span,
    })))
}

fn build_match_arm(pair: Pair<Rule>) -> Result<MatchArm, ParseError> {
    let span = pair_span(&pair);
    let mut inner = pair.into_inner();

    let pat_pair = inner.next().ok_or_else(|| {
        build_error(span, "match arm needs a pattern")
    })?;
    let pattern = build_pattern(pat_pair)?;

    let body_pair = inner.next().ok_or_else(|| {
        build_error(span, "match arm needs a body")
    })?;
    let body = match body_pair.as_rule() {
        Rule::block => MatchBody::Block(build_block(body_pair)?),
        _ => MatchBody::Expr(build_expr(body_pair)?),
    };

    Ok(MatchArm { pattern, body, span })
}

fn build_pattern(pair: Pair<Rule>) -> Result<Pattern, ParseError> {
    let span = pair_span(&pair);
    match pair.as_rule() {
        Rule::wildcard_pattern => Ok(Pattern::Wildcard(span)),
        Rule::none_pattern => Ok(Pattern::None(span)),
        Rule::some_pattern => {
            let id = build_identifier(pair.into_inner().next().unwrap())?;
            Ok(Pattern::Some(id))
        }
        Rule::literal_pattern => {
            let lit_pair = pair.into_inner().next().unwrap();
            let lit = build_literal(lit_pair)?;
            Ok(Pattern::Literal(lit))
        }
        Rule::identifier => {
            let id = build_identifier(pair)?;
            Ok(Pattern::Identifier(id))
        }
        Rule::string_literal | Rule::int_literal | Rule::bool_literal
        | Rule::float_literal | Rule::raw_string => {
            let lit = build_literal(pair)?;
            Ok(Pattern::Literal(lit))
        }
        rule => Err(build_error(span, format!("unsupported pattern: {:?}", rule))),
    }
}

// ============================================================
// Functions
// ============================================================

fn build_function_decl(pair: Pair<Rule>) -> Result<FunctionDecl, ParseError> {
    let span = pair_span(&pair);
    let mut inner = pair.into_inner();

    let name_pair = inner.next().ok_or_else(|| {
        build_error(span, "fn needs a name")
    })?;
    let name = build_identifier(name_pair)?;

    let mut params = Vec::new();
    let mut return_type = None;
    let mut body = None;

    for p in inner {
        match p.as_rule() {
            Rule::param_list => {
                for param_pair in p.into_inner() {
                    params.push(build_parameter(param_pair)?);
                }
            }
            Rule::return_type => {
                let full = p.into_inner().next().unwrap();
                return_type = Some(build_full_type(full)?);
            }
            Rule::block => {
                body = Some(build_block(p)?);
            }
            _ => {}
        }
    }

    let body = body.ok_or_else(|| build_error(span, "fn needs a body"))?;

    Ok(FunctionDecl {
        name,
        params,
        return_type,
        body,
        span,
    })
}

fn build_parameter(pair: Pair<Rule>) -> Result<Parameter, ParseError> {
    let span = pair_span(&pair);
    let mut inner = pair.into_inner();
    let name = build_identifier(inner.next().unwrap())?;
    let ty_pair = inner.next().unwrap();
    let ty = build_base_type(ty_pair)?;
    let default = inner.next().map(build_expr).transpose()?;
    Ok(Parameter { name, ty, default, span })
}

fn build_full_type(pair: Pair<Rule>) -> Result<TypeExpr, ParseError> {
    let mut inner = pair.into_inner();
    let base = inner.next().unwrap();
    build_base_type(base)
}

fn build_base_type(pair: Pair<Rule>) -> Result<TypeExpr, ParseError> {
    let span = pair_span(&pair);
    match pair.as_rule() {
        Rule::primitive_type => {
            let ty = match pair.as_str() {
                "string" => TypeExpr::String,
                "int" => TypeExpr::Int,
                "float" => TypeExpr::Float,
                "bool" => TypeExpr::Bool,
                "bytes" => TypeExpr::Bytes,
                "unit" => TypeExpr::Unit,
                "uuid" => TypeExpr::Uuid,
                "timestamp" => TypeExpr::Timestamp,
                other => return Err(build_error(span, format!("unknown type: {}", other))),
            };
            Ok(ty)
        }
        Rule::named_type => {
            let id = build_identifier(pair.into_inner().next().unwrap())?;
            Ok(TypeExpr::Named(id))
        }
        Rule::array_type => {
            let inner = pair.into_inner().next().unwrap();
            let inner_ty = build_base_type(inner)?;
            Ok(TypeExpr::Array(Box::new(inner_ty)))
        }
        Rule::base_type => {
            let inner = pair.into_inner().next().unwrap();
            build_base_type(inner)
        }
        rule => Err(build_error(span, format!("unsupported type: {:?}", rule))),
    }
}

fn build_block(pair: Pair<Rule>) -> Result<Block, ParseError> {
    let span = pair_span(&pair);
    let mut statements = Vec::new();
    for stmt_pair in pair.into_inner() {
        statements.push(build_statement(stmt_pair)?);
    }
    Ok(Block { statements, span })
}

fn build_statement(pair: Pair<Rule>) -> Result<AstStatement, ParseError> {
    let span = pair_span(&pair);
    match pair.as_rule() {
        Rule::if_stmt => Ok(AstStatement::If(build_if_stmt(pair)?)),
        Rule::while_stmt => Ok(AstStatement::While(build_while_stmt(pair)?)),
        Rule::for_stmt => Ok(AstStatement::For(build_for_stmt(pair)?)),
        Rule::let_stmt => {
            let mut inner = pair.into_inner();
            let name = build_identifier(inner.next().unwrap())?;

            let mut ty = None;
            let mut next = inner.next().ok_or_else(|| {
                build_error(span, "let needs a value")
            })?;

            if matches!(next.as_rule(),
                Rule::base_type | Rule::primitive_type
                | Rule::named_type | Rule::array_type
            ) {
                ty = Some(build_base_type(next)?);
                next = inner.next().ok_or_else(|| {
                    build_error(span, "let needs a value")
                })?;
            }

            let value = build_expr(next)?;
            Ok(AstStatement::Let(LetStmt { name, ty, value, span }))
        }
        Rule::assign_stmt => Ok(AstStatement::Assign(build_assign_stmt(pair)?)),
        Rule::return_stmt => {
            let mut inner = pair.into_inner();
            let value = inner.next().map(build_expr).transpose()?;
            Ok(AstStatement::Return(ReturnStmt { value, span }))
        }
        Rule::expr_stmt => {
            let inner = pair.into_inner().next().unwrap();
            let expr = build_expr(inner)?;
            Ok(AstStatement::Expr(expr))
        }
        rule => Err(build_error(span, format!("unsupported statement: {:?}", rule))),
    }
}

fn build_assign_stmt(pair: Pair<Rule>) -> Result<AssignStmt, ParseError> {
    let span = pair_span(&pair);
    let mut inner = pair.into_inner();

    let lv_pair = inner.next().ok_or_else(|| {
        build_error(span, "assign needs a target")
    })?;
    let target = build_lvalue(lv_pair)?;

    let op_pair = inner.next().ok_or_else(|| {
        build_error(span, "assign needs an operator")
    })?;
    let op = match op_pair.as_str() {
        "=" => AssignOp::Assign,
        "+=" => AssignOp::AddAssign,
        "-=" => AssignOp::SubAssign,
        "*=" => AssignOp::MulAssign,
        "/=" => AssignOp::DivAssign,
        other => return Err(build_error(span, format!("unknown op: {}", other))),
    };

    let value_pair = inner.next().ok_or_else(|| {
        build_error(span, "assign needs a value")
    })?;
    let value = build_expr(value_pair)?;

    Ok(AssignStmt { target, op, value, span })
}

fn build_lvalue(pair: Pair<Rule>) -> Result<LValue, ParseError> {
    let span = pair_span(&pair);
    let mut inner = pair.into_inner();

    let base = build_identifier(inner.next().unwrap())?;

    let mut path = Vec::new();
    for step in inner {
        match step.as_rule() {
            Rule::member_op => {
                let id = build_identifier(step.into_inner().next().unwrap())?;
                path.push(LValueStep::Member(id));
            }
            Rule::index_op => {
                let idx = build_expr(step.into_inner().next().unwrap())?;
                path.push(LValueStep::Index(idx));
            }
            _ => {}
        }
    }

    Ok(LValue { base, path, span })
}

fn build_if_stmt(pair: Pair<Rule>) -> Result<IfStmt, ParseError> {
    let span = pair_span(&pair);
    let mut inner = pair.into_inner();

    let cond_pair = inner.next().ok_or_else(|| {
        build_error(span, "if needs a condition")
    })?;
    let condition = build_expr(cond_pair)?;

    let then_pair = inner.next().ok_or_else(|| {
        build_error(span, "if needs a then block")
    })?;
    let then_block = build_block(then_pair)?;

    let else_branch = inner
        .find(|p| p.as_rule() == Rule::else_clause)
        .map(build_else_branch)
        .transpose()?;

    Ok(IfStmt {
        condition,
        then_block,
        else_branch,
        span,
    })
}

fn build_else_branch(pair: Pair<Rule>) -> Result<ElseBranch, ParseError> {
    let inner = pair.into_inner().next().ok_or_else(|| {
        build_error(Span::dummy(), "empty else clause")
    })?;
    match inner.as_rule() {
        Rule::if_stmt => {
            let stmt = build_if_stmt(inner)?;
            Ok(ElseBranch::ElseIf(Box::new(stmt)))
        }
        Rule::block => {
            let block = build_block(inner)?;
            Ok(ElseBranch::Else(block))
        }
        rule => Err(build_error(Span::dummy(), format!("bad else: {:?}", rule))),
    }
}

fn build_while_stmt(pair: Pair<Rule>) -> Result<WhileStmt, ParseError> {
    let span = pair_span(&pair);
    let mut inner = pair.into_inner();

    let cond_pair = inner.next().ok_or_else(|| {
        build_error(span, "while needs a condition")
    })?;
    let condition = build_expr(cond_pair)?;

    let body_pair = inner.next().ok_or_else(|| {
        build_error(span, "while needs a body")
    })?;
    let body = build_block(body_pair)?;

    Ok(WhileStmt { condition, body, span })
}

fn build_for_stmt(pair: Pair<Rule>) -> Result<ForStmt, ParseError> {
    let span = pair_span(&pair);
    let mut inner = pair.into_inner();

    let var_pair = inner.next().ok_or_else(|| {
        build_error(span, "for needs a variable")
    })?;
    let variable = build_identifier(var_pair)?;

    let iter_pair = inner.next().ok_or_else(|| {
        build_error(span, "for needs an iterable")
    })?;
    let iterable = build_expr(iter_pair)?;

    let body_pair = inner.next().ok_or_else(|| {
        build_error(span, "for needs a body")
    })?;
    let body = build_block(body_pair)?;

    Ok(ForStmt { variable, iterable, body, span })
}
