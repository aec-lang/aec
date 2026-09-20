//! خطاهای Runtime

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

    #[error("runtime error: {message}")]
    Generic { message: String, span: Span },
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
            RuntimeError::Generic { span, .. } => *span,
        }
    }
}
