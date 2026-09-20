//! # AEC UI
//!
//! Native UI renderer for AEC.
//! GPU-based, cross-platform (Linux, Windows, macOS, Web).

pub mod renderer;
pub mod widgets;

pub use renderer::{run_ui, AecApp};
