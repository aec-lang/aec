//! Expressions — قلب زبان AEC

use crate::function::Block;
use crate::literal::Literal;
use crate::program::Identifier;
use crate::span::Span;

#[derive(Debug, Clone)]
pub enum Expr {
    Literal(Box<LiteralExpr>),
    Identifier(Identifier),
    Paren(Box<ParenExpr>),
    Array(Box<ArrayExpr>),
    Object(Box<ObjectExpr>),
    Binary(Box<BinaryExpr>),
    Unary(Box<UnaryExpr>),
    Call(Box<CallExpr>),
    Member(Box<MemberExpr>),
    Index(Box<IndexExpr>),
    Await(Box<AwaitExpr>),
    Match(Box<MatchExpr>),
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::Literal(e) => e.span,
            Expr::Identifier(id) => id.span,
            Expr::Paren(e) => e.span,
            Expr::Array(e) => e.span,
            Expr::Object(e) => e.span,
            Expr::Binary(e) => e.span,
            Expr::Unary(e) => e.span,
            Expr::Call(e) => e.span,
            Expr::Member(e) => e.span,
            Expr::Index(e) => e.span,
            Expr::Await(e) => e.span,
            Expr::Match(e) => e.span,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LiteralExpr {
    pub value: Literal,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ParenExpr {
    pub inner: Expr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ArrayExpr {
    pub elements: Vec<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ObjectExpr {
    pub fields: Vec<ObjectField>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ObjectField {
    pub key: Identifier,
    pub value: Expr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct BinaryExpr {
    pub left: Expr,
    pub op: BinaryOp,
    pub right: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Eq, Neq, Lt, Gt, Lte, Gte,
    Add, Sub, Mul, Div, Mod,
    And, Or,
}

impl BinaryOp {
    pub fn precedence(self) -> u8 {
        match self {
            BinaryOp::Or => 1,
            BinaryOp::And => 2,
            BinaryOp::Eq | BinaryOp::Neq => 3,
            BinaryOp::Lt | BinaryOp::Gt | BinaryOp::Lte | BinaryOp::Gte => 4,
            BinaryOp::Add | BinaryOp::Sub => 5,
            BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => 6,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            BinaryOp::Eq => "==",
            BinaryOp::Neq => "!=",
            BinaryOp::Lt => "<",
            BinaryOp::Gt => ">",
            BinaryOp::Lte => "<=",
            BinaryOp::Gte => ">=",
            BinaryOp::Add => "+",
            BinaryOp::Sub => "-",
            BinaryOp::Mul => "*",
            BinaryOp::Div => "/",
            BinaryOp::Mod => "%",
            BinaryOp::And => "and",
            BinaryOp::Or => "or",
        }
    }
}

#[derive(Debug, Clone)]
pub struct UnaryExpr {
    pub op: UnaryOp,
    pub operand: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
    Not,
}

#[derive(Debug, Clone)]
pub struct CallExpr {
    pub callee: Expr,
    pub args: Vec<Argument>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Argument {
    pub name: Option<Identifier>,
    pub value: Expr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct MemberExpr {
    pub object: Expr,
    pub property: Identifier,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct IndexExpr {
    pub object: Expr,
    pub index: Expr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct AwaitExpr {
    pub inner: Expr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct MatchExpr {
    pub scrutinee: Expr,
    pub arms: Vec<MatchArm>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub body: MatchBody,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum MatchBody {
    Block(Block),
    Expr(Expr),
}

#[derive(Debug, Clone)]
pub enum Pattern {
    Wildcard(Span),
    Some(Identifier),
    None(Span),
    Literal(Literal),
    Identifier(Identifier),
}
