//! AEC type system — phase 1
//!
//! The checker is deliberately lenient: whenever unsure we yield `Ty::Any` to avoid false errors.
//! The goal is catching obvious mistakes, not fully proving program correctness.

use aec_ast::TypeExpr;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Ty {
    Int,
    Float,
    String,
    Bool,
    Bytes,
    Uuid,
    Timestamp,
    None,
    /// No value returned (function without a return value)
    Unit,
    Array(Box<Ty>),
    /// Object with known fields (may be incomplete)
    Object(Vec<(String, Ty)>),
    Function,
    Optional(Box<Ty>),
    /// `Result(ok, err)` — produced by `ok(...)` / `err(...)`
    Result(Box<Ty>, Box<Ty>),
    /// Unknown / dynamic — compatible with everything
    Any,
}

impl Ty {
    pub fn is_any(&self) -> bool {
        matches!(self, Ty::Any)
    }

    pub fn is_numeric(&self) -> bool {
        matches!(self, Ty::Int | Ty::Float)
    }

    pub fn is_bool(&self) -> bool {
        matches!(self, Ty::Bool)
    }

    /// Result type of an arithmetic operator on two numeric types.
    pub fn numeric_result(a: &Ty, b: &Ty) -> Ty {
        if matches!(a, Ty::Float) || matches!(b, Ty::Float) {
            Ty::Float
        } else {
            Ty::Int
        }
    }
}

impl fmt::Display for Ty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Ty::Int => write!(f, "int"),
            Ty::Float => write!(f, "float"),
            Ty::String => write!(f, "string"),
            Ty::Bool => write!(f, "bool"),
            Ty::Bytes => write!(f, "bytes"),
            Ty::Uuid => write!(f, "uuid"),
            Ty::Timestamp => write!(f, "timestamp"),
            Ty::None => write!(f, "none"),
            Ty::Unit => write!(f, "unit"),
            Ty::Array(t) => write!(f, "[{}]", t),
            Ty::Object(fields) => {
                if fields.is_empty() {
                    write!(f, "object")
                } else {
                    let inner: Vec<String> = fields
                        .iter()
                        .map(|(k, t)| format!("{}: {}", k, t))
                        .collect();
                    write!(f, "{{{}}}", inner.join(", "))
                }
            }
            Ty::Function => write!(f, "function"),
            Ty::Optional(t) => write!(f, "{}?", t),
            Ty::Result(ok, err) => write!(f, "Result({}, {})", ok, err),
            Ty::Any => write!(f, "any"),
        }
    }
}

/// Converts a type declared in the code (TypeExpr) into a Ty.
pub fn ty_from_expr(te: &TypeExpr) -> Ty {
    match te {
        TypeExpr::String => Ty::String,
        TypeExpr::Int => Ty::Int,
        TypeExpr::Float => Ty::Float,
        TypeExpr::Bool => Ty::Bool,
        TypeExpr::Bytes => Ty::Bytes,
        TypeExpr::Unit => Ty::Unit,
        TypeExpr::Uuid => Ty::Uuid,
        TypeExpr::Timestamp => Ty::Timestamp,
        TypeExpr::Function => Ty::Function,
        // user types (struct) are not supported for now
        TypeExpr::Named(_) => Ty::Any,
        TypeExpr::Optional(inner) => Ty::Optional(Box::new(ty_from_expr(inner))),
        TypeExpr::Array(inner) => Ty::Array(Box::new(ty_from_expr(inner))),
        TypeExpr::Result(ok, err) => {
            Ty::Result(Box::new(ty_from_expr(ok)), Box::new(ty_from_expr(err)))
        }
    }
}

/// Can a value of type `actual` be used where `expected` is required?
pub fn compatible(expected: &Ty, actual: &Ty) -> bool {
    if expected.is_any() || actual.is_any() {
        return true;
    }
    match (expected, actual) {
        // int and float are convertible at runtime
        (Ty::Int, Ty::Float) | (Ty::Float, Ty::Int) => true,
        (Ty::Optional(_), Ty::None) => true,
        (Ty::Optional(e), Ty::Optional(a)) => compatible(e, a),
        (Ty::Optional(e), a) => compatible(e, a),
        (Ty::Array(e), Ty::Array(a)) => compatible(e, a),
        (Ty::Object(_), Ty::Object(actual_fields)) => {
            // if the actual value's fields are unknown, we do not enforce strictly
            actual_fields.is_empty()
        }
        (Ty::Function, Ty::Function) => true,
        (Ty::Result(e_ok, e_err), Ty::Result(a_ok, a_err)) => {
            compatible(e_ok, a_ok) && compatible(e_err, a_err)
        }
        (a, b) => a == b,
    }
}
