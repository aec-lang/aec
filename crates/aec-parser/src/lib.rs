use pest::Parser;
use pest_derive::Parser;

pub mod build_ast;
pub mod errors;

pub use errors::{build_error, convert_pest_error};

use aec_ast::{Expr, Program};

#[derive(Parser)]
#[grammar = "grammar.pest"]
pub struct AecParser;

pub fn parse(source: &str) -> Result<Program, aec_ast::ParseError> {
    let mut pairs = AecParser::parse(Rule::program, source)
        .map_err(convert_pest_error)?;

    let program_pair = pairs.next().unwrap();
    build_ast::build_program(program_pair)
}

/// Parses a standalone expression. Used by string interpolation, where the
/// text between `{` and `}` has to be turned into an [`Expr`].
pub fn parse_expr(source: &str) -> Result<Expr, aec_ast::ParseError> {
    let mut pairs = AecParser::parse(Rule::expr_root, source)
        .map_err(convert_pest_error)?;

    let root = pairs.next().ok_or_else(|| {
        build_error(aec_ast::Span::dummy(), "empty interpolated expression")
    })?;
    let expr_pair = root
        .into_inner()
        .next()
        .ok_or_else(|| build_error(aec_ast::Span::dummy(), "empty interpolated expression"))?;
    build_ast::build_expr(expr_pair)
}
