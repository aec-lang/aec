//! Runtime errors

use crate::value::Value;
use aec_ast::Span;
use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum RuntimeError {
    #[error("undefined variable: {name}")]
    UndefinedVariable { name: String, span: Span },

    #[error("type error: {message}")]
    TypeError { message: String, span: Span },

    #[error("division by zero")]
    DivisionByZero { span: Span },

    #[error("undefined function: {name}")]
    UndefinedFunction { name: String, span: Span },

    #[error("wrong number of arguments: expected {expected}, got {got}")]
    WrongArgCount {
        expected: usize,
        got: usize,
        span: Span,
    },

    #[error("return outside function")]
    ReturnOutsideFunction { span: Span },

    #[error("break outside loop")]
    BreakOutsideLoop { span: Span },

    #[error("permission denied: {message}")]
    PermissionDenied { message: String, span: Span },

    #[error("runtime error: {message}")]
    Generic { message: String, span: Span },

    /// Internal control-flow signal for the `?` operator: the enclosing function
    /// must return this value right away. It is produced by `Expr::Try`, caught
    /// in `Interpreter::call_function`, and never escapes a program.
    #[doc(hidden)]
    #[error("unwind: early return")]
    EarlyReturn { value: Box<Value> },
}

impl RuntimeError {
    pub fn span(&self) -> Span {
        match self {
            RuntimeError::UndefinedVariable { span, .. } => *span,
            RuntimeError::TypeError { span, .. } => *span,
            RuntimeError::DivisionByZero { span } => *span,
            RuntimeError::UndefinedFunction { span, .. } => *span,
            RuntimeError::WrongArgCount { span, .. } => *span,
            RuntimeError::ReturnOutsideFunction { span } => *span,
            RuntimeError::BreakOutsideLoop { span } => *span,
            RuntimeError::PermissionDenied { span, .. } => *span,
            RuntimeError::Generic { span, .. } => *span,
            // Not a real error: there is no location to report.
            RuntimeError::EarlyReturn { .. } => Span::dummy(),
        }
    }

    /// Replace the span with the real call site.
    /// Modules that have no access to a span (such as `memory`) build errors with
    /// `Span::dummy()`, and the interpreter layer pins them down with this method.
    pub fn with_span(self, span: Span) -> Self {
        match self {
            RuntimeError::UndefinedVariable { name, .. } => {
                RuntimeError::UndefinedVariable { name, span }
            }
            RuntimeError::TypeError { message, .. } => RuntimeError::TypeError { message, span },
            RuntimeError::DivisionByZero { .. } => RuntimeError::DivisionByZero { span },
            RuntimeError::UndefinedFunction { name, .. } => {
                RuntimeError::UndefinedFunction { name, span }
            }
            RuntimeError::WrongArgCount { expected, got, .. } => RuntimeError::WrongArgCount {
                expected,
                got,
                span,
            },
            RuntimeError::ReturnOutsideFunction { .. } => {
                RuntimeError::ReturnOutsideFunction { span }
            }
            RuntimeError::BreakOutsideLoop { .. } => RuntimeError::BreakOutsideLoop { span },
            RuntimeError::PermissionDenied { message, .. } => {
                RuntimeError::PermissionDenied { message, span }
            }
            RuntimeError::Generic { message, .. } => RuntimeError::Generic { message, span },
            // Control flow, not an error: leave it untouched.
            early @ RuntimeError::EarlyReturn { .. } => early,
        }
    }
}
