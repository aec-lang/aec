//! # AEC Runtime

pub mod errors;
pub mod value;
pub mod interpreter;
pub mod llm;
pub mod memory;
pub mod stdlib;

pub use errors::RuntimeError;
pub use value::{Value, Environment, Env, Function};
pub use interpreter::Interpreter;
pub use memory::Memory;

use aec_ast::Program;

pub fn run(program: &Program) -> Result<(), RuntimeError> {
    let mut interp = Interpreter::new();
    interp.run(program)
}
