//! Function declarations

use crate::expr::Expr;
use crate::program::Identifier;
use crate::span::Span;

#[derive(Debug, Clone)]
pub struct FunctionDecl {
    pub name: Identifier,
    pub params: Vec<Parameter>,
    pub return_type: Option<TypeExpr>,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Parameter {
    pub name: Identifier,
    pub ty: TypeExpr,
    pub default: Option<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Block {
    pub statements: Vec<Statement>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum Statement {
    Let(LetStmt),
    Assign(AssignStmt),
    Return(ReturnStmt),
    Expr(Expr),
    If(IfStmt),
    While(WhileStmt),
    For(ForStmt),
}

#[derive(Debug, Clone)]
pub struct LetStmt {
    pub name: Identifier,
    pub ty: Option<TypeExpr>,
    pub value: Expr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct AssignStmt {
    pub target: LValue,
    pub op: AssignOp,
    pub value: Expr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct LValue {
    pub base: Identifier,
    pub path: Vec<LValueStep>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum LValueStep {
    Member(Identifier),
    Index(Expr),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignOp {
    Assign,
    AddAssign,
    SubAssign,
    MulAssign,
    DivAssign,
}

#[derive(Debug, Clone)]
pub struct ReturnStmt {
    pub value: Option<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct IfStmt {
    pub condition: Expr,
    pub then_block: Block,
    pub else_branch: Option<ElseBranch>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ElseBranch {
    ElseIf(Box<IfStmt>),
    Else(Block),
}

#[derive(Debug, Clone)]
pub struct WhileStmt {
    pub condition: Expr,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ForStmt {
    pub variable: Identifier,
    pub iterable: Expr,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum TypeExpr {
    String,
    Int,
    Float,
    Bool,
    Bytes,
    Unit,
    Uuid,
    Timestamp,
    Named(Identifier),
    Optional(Box<TypeExpr>),
    Array(Box<TypeExpr>),
}
