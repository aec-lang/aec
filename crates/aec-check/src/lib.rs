//! # AEC Check
//!
//! Static (type) checking over the AST, before execution.
//!
//! The checker is deliberately lenient: whenever unsure it yields `Ty::Any` to avoid false errors.
//! Only clear-cut cases (type mismatch, invalid operand, argument count) become errors.
//!
//! ```ignore
//! let program = aec_parser::parse(source)?;
//! let diags = aec_check::check_program(&program);
//! if aec_check::error_count(&diags) > 0 { /* ... */ }
//! ```

pub mod checker;
pub mod diag;
pub mod ty;

pub use checker::{check_program, Checker};
pub use diag::{error_count, DiagKind, Diagnostic, Severity};
pub use ty::{compatible, ty_from_expr, Ty};
