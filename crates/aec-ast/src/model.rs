use crate::literal::Literal;
use crate::program::Identifier;
use crate::span::Span;

#[derive(Debug, Clone)]
pub struct ModelDecl {
    pub name: Identifier,
    pub fields: Vec<ModelField>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ModelField {
    Property(ModelProperty),
    Retry(RetryBlock),
    Capabilities(CapabilitiesBlock),
}

#[derive(Debug, Clone)]
pub struct ModelProperty {
    pub name: Identifier,
    pub value: Literal,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct RetryBlock {
    pub fields: Vec<RetryField>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum RetryField {
    Property(ModelProperty),
    On(Vec<Literal>),
}

#[derive(Debug, Clone)]
pub struct CapabilitiesBlock {
    pub entries: Vec<ModelProperty>,
    pub span: Span,
}
