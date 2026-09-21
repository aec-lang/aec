//! # AEC Runtime

pub mod errors;
pub mod value;
pub mod interpreter;
pub mod llm;
pub mod memory;
pub mod permissions;
pub mod stdlib;
pub mod stdlib_extended;

pub use errors::RuntimeError;
pub use value::{Value, Environment, Env, Function};
pub use interpreter::Interpreter;
pub use memory::Memory;
pub use permissions::{FsMode, Limits, Permissions};

use aec_ast::Program;

pub fn run(program: &Program) -> Result<(), RuntimeError> {
    let mut interp = Interpreter::new();
    interp.run(program)
}
