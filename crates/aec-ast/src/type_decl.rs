use crate::function::TypeExpr;
use crate::program::Identifier;
use crate::span::Span;

#[derive(Debug, Clone, PartialEq)]
pub struct StructDecl {
    pub is_public: bool,
    pub name: Identifier,
    pub fields: Vec<StructField>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructField {
    pub name: Identifier,
    pub ty: TypeExpr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumDecl {
    pub is_public: bool,
    pub name: Identifier,
    pub variants: Vec<EnumVariantDecl>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumVariantDecl {
    pub name: Identifier,
    pub payload: Option<TypeExpr>,
    pub span: Span,
}
