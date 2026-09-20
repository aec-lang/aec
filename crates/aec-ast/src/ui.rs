//! UI DSL — AST nodes for UI definitions
//! این بخشی از **زبان** AEC ـه، نه یه کتابخونه

use crate::expr::Expr;
use crate::program::Identifier;
use crate::span::Span;

/// ui Main = Screen "Title" { ... }
#[derive(Debug, Clone)]
pub struct UiDecl {
    pub name: Identifier,
    pub screen: ScreenExpr,
    pub span: Span,
}

/// Screen "Title" { ... }
#[derive(Debug, Clone)]
pub struct ScreenExpr {
    pub title: String,
    pub body: Vec<UiStatement>,
    pub span: Span,
}

/// یه statement داخل UI
#[derive(Debug, Clone)]
pub enum UiStatement {
    /// @message: string = ""
    State(StateDeclUi),

    /// Text "Hello"
    /// Button "Send" on click -> do_this()
    /// Column { ... }
    Element(ElementExpr),

    /// if cond { ... }
    If(UiIf),

    /// for item in items { ... }
    For(UiFor),
}

/// @message: string = ""
#[derive(Debug, Clone)]
pub struct StateDeclUi {
    pub name: Identifier,
    pub ty: Option<TypeRef>,
    pub initial: Expr,
    pub span: Span,
}

/// Text "Hello" prop: value { ... }
#[derive(Debug, Clone)]
pub struct ElementExpr {
    pub name: Identifier,
    pub primary_arg: Option<Expr>,
    pub modifiers: Vec<ElementModifier>,
    pub children: Option<Vec<UiStatement>>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ElementModifier {
    /// size: large
    Property(ElementProperty),

    /// bind value to message
    Binding(Binding),

    /// on click -> do_this()
    Event(EventHandler),
}

#[derive(Debug, Clone)]
pub struct ElementProperty {
    pub name: Identifier,
    pub value: Expr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Binding {
    pub target: Identifier,      // value
    pub source: Expr,            // message
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct EventHandler {
    pub event: Identifier,       // click
    pub handler: Expr,           // do_this()
    pub span: Span,
}

/// if cond { ... } else { ... }
#[derive(Debug, Clone)]
pub struct UiIf {
    pub condition: Expr,
    pub then_body: Vec<UiStatement>,
    pub else_body: Option<Vec<UiStatement>>,
    pub span: Span,
}

/// for item in items { ... }
#[derive(Debug, Clone)]
pub struct UiFor {
    pub variable: Identifier,
    pub iterable: Expr,
    pub body: Vec<UiStatement>,
    pub span: Span,
}

/// نوع ساده برای UI state
#[derive(Debug, Clone)]
pub enum TypeRef {
    String,
    Int,
    Float,
    Bool,
    Named(String),
}
