//! # AEC UI
//!
//! Native UI renderer for AEC.
//! GPU-based, cross-platform (Linux, Windows, macOS, Web).

pub mod harness;
pub mod renderer;
pub mod tui;
pub mod tui_driver;
pub mod widgets;

pub use egui;
pub use harness::{render_program, HarnessAction, RenderOptions, RenderedFrame};
pub use renderer::{run_ui, AecApp};
pub use tui::{render_grid, Grid, Key};
pub use widgets::{
    build_widgets, build_widgets_with_components, build_widgets_with_themes, ComponentRegistry,
    InputEdit, ResolvedTheme, Themes, UiState, UiValue, Widget, WidgetStyle, EventAction,
};
