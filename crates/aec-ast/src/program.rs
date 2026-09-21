use crate::declarative::SecretsBlock;
use crate::function::FunctionDecl;
use crate::model::ModelDecl;
use crate::permissions::{LimitsBlock, PermissionsBlock};
use crate::span::Span;
use crate::theme::ThemeDecl;
use crate::ui::{ComponentDecl, UiDecl};

#[derive(Debug, Clone)]
pub struct Program {
    pub header: AgentHeader,
    pub items: Vec<TopLevelItem>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct AgentHeader {
    pub name: Identifier,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Identifier {
    pub name: String,
    pub span: Span,
}

impl Identifier {
    pub fn new(name: impl Into<String>, span: Span) -> Self {
        Self {
            name: name.into(),
            span,
        }
    }
}

#[derive(Debug, Clone)]
pub enum TopLevelItem {
    Secrets(SecretsBlock),
    Permissions(PermissionsBlock),
    Limits(LimitsBlock),
    Model(ModelDecl),
    Import(ImportStmt),
    Function(FunctionDecl),
    Component(ComponentDecl),
    Theme(ThemeDecl),
    Ui(UiDecl),
}

#[derive(Debug, Clone)]
pub struct ImportStmt {
    pub path: String,
    pub alias: Option<Identifier>,
    pub span: Span,
}
