//! Theme System — named, reusable design tokens for UI
//!
//! A theme is a set of groups (color, text, spacing, ...), each holding several
//! tokens. Tokens are referenced by the path `theme.<group>.<token>`.

use crate::program::Identifier;
use crate::span::Span;
use crate::style::StyleValue;

/// theme Dark { color { ... } text { ... } }
#[derive(Debug, Clone)]
pub struct ThemeDecl {
    pub name: Identifier,
    /// Should this theme be applied to the UI by default?
    pub is_default: bool,
    /// Inherit from another theme
    pub extends: Option<Identifier>,
    pub groups: Vec<ThemeGroup>,
    pub span: Span,
}

/// color { background: "#1e1e2e" primary: "#89b4fa" }
#[derive(Debug, Clone)]
pub struct ThemeGroup {
    pub name: Identifier,
    pub entries: Vec<ThemeEntry>,
    pub span: Span,
}

/// background: "#1e1e2e"
#[derive(Debug, Clone)]
pub struct ThemeEntry {
    pub name: Identifier,
    pub value: StyleValue,
    pub span: Span,
}
