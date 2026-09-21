use crate::literal::Literal;
use crate::program::Identifier;
use crate::span::Span;

/// The `permissions { ... }` block — declares the program's capabilities.
///
/// Absence of this block means "do not enforce" (open behaviour, backward compatible).
/// Presence of this block means "closed by default": every undeclared key is denied.
#[derive(Debug, Clone)]
pub struct PermissionsBlock {
    pub entries: Vec<PermissionsEntry>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum PermissionsEntry {
    Network(NetworkRule),
    Filesystem(FilesystemRule),
    System(SystemRule),
}

/// `network: ["api.openai.com", "api.telegram.org"]`
#[derive(Debug, Clone)]
pub struct NetworkRule {
    pub hosts: Vec<String>,
    pub span: Span,
}

/// `filesystem { read: [...] write: [...] }`
#[derive(Debug, Clone)]
pub struct FilesystemRule {
    pub read: Vec<String>,
    pub write: Vec<String>,
    pub span: Span,
}

/// `system { metrics: true restart: false }`
#[derive(Debug, Clone)]
pub struct SystemRule {
    pub metrics: Option<bool>,
    pub restart: Option<bool>,
    pub span: Span,
}

/// The `limits { ... }` block — execution ceilings.
#[derive(Debug, Clone)]
pub struct LimitsBlock {
    pub entries: Vec<LimitsEntry>,
    pub span: Span,
}

/// `concurrency: 8` or `timeout: 10s`
#[derive(Debug, Clone)]
pub struct LimitsEntry {
    pub name: Identifier,
    pub value: Literal,
    pub span: Span,
}
