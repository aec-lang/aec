use pest::Parser;
use pest_derive::Parser;

pub mod build_ast;
pub mod errors;

pub use errors::{build_error, convert_pest_error};

use aec_ast::Program;

#[derive(Parser)]
#[grammar = "grammar.pest"]
pub struct AecParser;

pub fn parse(source: &str) -> Result<Program, aec_ast::ParseError> {
    let mut pairs = AecParser::parse(Rule::program, source)
        .map_err(convert_pest_error)?;

    let program_pair = pairs.next().unwrap();
    build_ast::build_program(program_pair)
}
