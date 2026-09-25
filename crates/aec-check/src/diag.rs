//! Diagnostics of the type checker

use aec_ast::Span;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Blocks execution
    Error,
    /// Suspicious, but does not stop execution
    Warning,
}

/// Error kind — for testability and more precise messages
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagKind {
    TypeMismatch,
    InvalidOperand,
    ArityMismatch,
    NotCallable,
    NotIndexable,
    UndeclaredVariable,
    AssignToImmutable,
    UnknownFunction,
    InvalidUi,
    DuplicateDeclaration,
    UnsupportedFeature,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub kind: DiagKind,
    pub message: String,
    pub span: Span,
}

impl Diagnostic {
    pub fn error(kind: DiagKind, span: Span, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            kind,
            message: message.into(),
            span,
        }
    }

    pub fn warning(kind: DiagKind, span: Span, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            kind,
            message: message.into(),
            span,
        }
    }

    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let tag = match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        let pos = self.span.start;
        write!(
            f,
            "{}:{}:{}: {}: {}",
            pos.line, pos.column, tag, tag, self.message
        )
    }
}

/// Number of errors (severity = Error) among the diagnostics.
pub fn error_count(diags: &[Diagnostic]) -> usize {
    diags.iter().filter(|d| d.is_error()).count()
}
