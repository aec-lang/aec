use crate::program::Identifier;
use crate::span::Span;

#[derive(Debug, Clone)]
pub struct SecretsBlock {
    pub entries: Vec<SecretsEntry>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct SecretsEntry {
    pub name: Identifier,
    pub value: SecretValue,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum SecretValue {
    String(String),
    Env(String),
}
