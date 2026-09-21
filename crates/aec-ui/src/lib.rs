//! # AEC UI
//!
//! Native UI renderer for AEC.
//! GPU-based, cross-platform (Linux, Windows, macOS, Web).

pub mod renderer;
pub mod widgets;

pub use renderer::{run_ui, AecApp};
pub use widgets::{
    build_widgets, build_widgets_with_components, build_widgets_with_themes, ComponentRegistry,
    ResolvedTheme, Themes, UiState, UiValue, Widget, WidgetStyle,
};
