use crate::span::Span;
use thiserror::Error;

#[derive(Debug, Clone, Error)]
#[error("{kind} at {span:?}")]
pub struct ParseError {
    pub span: Span,
    pub kind: ParseErrorKind,
}

impl ParseError {
    pub fn new(span: Span, kind: ParseErrorKind) -> Self {
        Self { span, kind }
    }
}

#[derive(Debug, Clone, Error)]
pub enum ParseErrorKind {
    #[error("unexpected token: expected {expected:?}, found '{found}'")]
    UnexpectedToken { expected: Vec<String>, found: String },
    #[error("unexpected end of file: expected {expected:?}")]
    UnexpectedEof { expected: Vec<String> },
    #[error("invalid literal: {literal_type}")]
    InvalidLiteral { literal_type: String },
    #[error("unterminated string literal")]
    UnterminatedString,
    #[error("invalid escape sequence: '{sequence}'")]
    InvalidEscape { sequence: String },
    #[error("AST build error: {message}")]
    BuildError { message: String },
}
