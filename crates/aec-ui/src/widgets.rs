//! Widget tree

use aec_ast::{
    ComponentDecl, ComponentUse, ElementExpr, ElementModifier, Expr, Style, StyleValue, ThemeDecl,
    TopLevelItem, UiFor, UiIf, UiStatement,
};
use std::collections::{HashMap, HashSet};

pub type ComponentRegistry = HashMap<String, ComponentDecl>;

/// A theme whose `extends` chain has been resolved. Keys look like "group.token".
#[derive(Debug, Clone, Default)]
pub struct ResolvedTheme {
    pub tokens: HashMap<String, StyleValue>,
}

impl ResolvedTheme {
    pub fn get(&self, path: &str) -> Option<&StyleValue> {
        self.tokens.get(path)
    }

    pub fn insert(&mut self, group: &str, token: &str, value: StyleValue) {
        self.tokens.insert(format!("{}.{}", group, token), value);
    }
}

/// The set of a program's themes plus the active (default) theme.
#[derive(Debug, Clone, Default)]
pub struct Themes {
    pub named: HashMap<String, ResolvedTheme>,
    pub default: Option<ResolvedTheme>,
}

impl Themes {
    /// Builds the themes from the program items and resolves the extends chain.
    pub fn from_items(items: &[TopLevelItem]) -> Self {
        let decls: Vec<&ThemeDecl> = items
            .iter()
            .filter_map(|item| match item {
                TopLevelItem::Theme(t) => Some(t),
                _ => None,
            })
            .collect();

        let mut named = HashMap::new();
        for decl in &decls {
            let resolved = resolve_theme(decl, &decls, &mut HashSet::new());
            named.insert(decl.name.name.clone(), resolved);
        }

        // Active theme: the one marked default; otherwise, if exactly one theme
        // exists, that one.
        let default = decls
            .iter()
            .find(|d| d.is_default)
            .map(|d| named.get(&d.name.name).cloned().unwrap_or_default())
            .or_else(|| {
                if decls.len() == 1 {
                    named.get(&decls[0].name.name).cloned()
                } else {
                    None
                }
            });

        Self { named, default }
    }

    pub fn get(&self, name: &str) -> Option<&ResolvedTheme> {
        self.named.get(name)
    }

    /// The active theme used to resolve `theme.x.y` references.
    pub fn active(&self) -> Option<&ResolvedTheme> {
        self.default.as_ref()
    }
}

/// Flattens a theme by following its `extends` chain.
/// `visited` guards against cycles.
fn resolve_theme(
    decl: &ThemeDecl,
    all: &[&ThemeDecl],
    visited: &mut HashSet<String>,
) -> ResolvedTheme {
    if !visited.insert(decl.name.name.clone()) {
        return ResolvedTheme::default();
    }

    let mut resolved = decl
        .extends
        .as_ref()
        .and_then(|parent| all.iter().find(|d| d.name.name == parent.name))
        .map(|parent| resolve_theme(parent, all, visited))
        .unwrap_or_default();

    for group in &decl.groups {
        for entry in &group.entries {
            resolved.insert(&group.name.name, &entry.name.name, entry.value.clone());
        }
    }

    resolved
}

#[derive(Debug, Clone)]
pub struct WidgetStyle {
    pub properties: HashMap<String, StyleValue>,
}

impl WidgetStyle {
    pub fn new() -> Self {
        Self {
            properties: HashMap::new(),
        }
    }

    pub fn from_style(s: &Style) -> Self {
        Self {
            properties: s.properties.clone(),
        }
    }

    pub fn get_string(&self, name: &str) -> Option<String> {
        self.properties.get(name).map(|v| match v {
            StyleValue::String(s) => s.clone(),
            StyleValue::Int(n) => n.to_string(),
            StyleValue::Float(f) => f.to_string(),
            StyleValue::Bool(b) => b.to_string(),
            StyleValue::Ident(s) => s.clone(),
        })
    }

    pub fn get_float(&self, name: &str) -> Option<f64> {
        match self.properties.get(name) {
            Some(StyleValue::Float(f)) => Some(*f),
            Some(StyleValue::Int(n)) => Some(*n as f64),
            Some(StyleValue::String(s)) => s.parse().ok(),
            _ => None,
        }
    }

    pub fn get_bool(&self, name: &str) -> Option<bool> {
        match self.properties.get(name) {
            Some(StyleValue::Bool(b)) => Some(*b),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Widget {
    Column(Vec<Widget>, WidgetStyle),
    Row(Vec<Widget>, WidgetStyle),
    Text(String, WidgetStyle),
    Display {
        var_name: String,
        style: WidgetStyle,
    },
    Divider,
    Heading(String, WidgetStyle),
    Spacer,
    Input {
        bind_target: Option<String>,
        placeholder: Option<String>,
        value: String,
        style: WidgetStyle,
    },
    Button {
        label: String,
        on_click: Option<String>,
        style: WidgetStyle,
    },
    Card(Vec<Widget>, WidgetStyle),
    Container(Vec<Widget>),
    MessagesList {
        source: String,
        style: WidgetStyle,
    },
    If {
        condition: bool,
        then_branch: Vec<Widget>,
        else_branch: Option<Vec<Widget>>,
    },
    For {
        variable: String,
        items: Vec<Widget>,
    },
}

#[derive(Debug, Clone)]
pub struct MessageItem {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct UiState {
    pub values: HashMap<String, UiValue>,
}

#[derive(Debug, Clone)]
pub enum UiValue {
    String(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    Array(Vec<UiValue>),
    Object(HashMap<String, UiValue>),
}

impl UiValue {
    pub fn as_string(&self) -> String {
        match self {
            UiValue::String(s) => s.clone(),
            UiValue::Int(n) => n.to_string(),
            UiValue::Float(f) => f.to_string(),
            UiValue::Bool(b) => b.to_string(),
            UiValue::Array(_) => "[array]".to_string(),
            UiValue::Object(_) => "[object]".to_string(),
        }
    }

    pub fn as_bool(&self) -> bool {
        match self {
            UiValue::Bool(b) => *b,
            UiValue::String(s) => !s.is_empty(),
            UiValue::Int(n) => *n != 0,
            _ => false,
        }
    }

    pub fn as_array(&self) -> Vec<UiValue> {
        match self {
            UiValue::Array(a) => a.clone(),
            _ => vec![],
        }
    }
}

impl UiState {
    pub fn new() -> Self {
        Self {
            values: HashMap::new(),
        }
    }

    pub fn get_string(&self, name: &str) -> String {
        self.values
            .get(name)
            .map(|v| v.as_string())
            .unwrap_or_default()
    }

    pub fn set_string(&mut self, name: &str, value: String) {
        self.values.insert(name.to_string(), UiValue::String(value));
    }

    pub fn get_bool(&self, name: &str) -> bool {
        self.values.get(name).map(|v| v.as_bool()).unwrap_or(false)
    }

    pub fn get_value(&self, name: &str) -> Option<&UiValue> {
        self.values.get(name)
    }
}

/// Build context: components, the active theme, and the recursion guard.
struct BuildCtx<'a> {
    components: &'a ComponentRegistry,
    theme: &'a ResolvedTheme,
    active: HashSet<String>,
}

pub fn build_widgets(statements: &[UiStatement], state: &mut UiState) -> Vec<Widget> {
    let components = ComponentRegistry::new();
    let themes = Themes::default();
    build_widgets_full(statements, state, &components, &themes)
}

pub fn build_widgets_with_components(
    statements: &[UiStatement],
    state: &mut UiState,
    components: &ComponentRegistry,
) -> Vec<Widget> {
    let themes = Themes::default();
    build_widgets_full(statements, state, components, &themes)
}

/// Builds widgets with components and themes (used by the renderer).
pub fn build_widgets_with_themes(
    statements: &[UiStatement],
    state: &mut UiState,
    components: &ComponentRegistry,
    themes: &Themes,
) -> Vec<Widget> {
    build_widgets_full(statements, state, components, themes)
}

fn build_widgets_full(
    statements: &[UiStatement],
    state: &mut UiState,
    components: &ComponentRegistry,
    themes: &Themes,
) -> Vec<Widget> {
    let active_theme = themes.active().cloned().unwrap_or_default();
    let mut ctx = BuildCtx {
        components,
        theme: &active_theme,
        active: HashSet::new(),
    };
    build_widgets_in_context(statements, state, &mut ctx)
}

fn build_widgets_in_context(
    statements: &[UiStatement],
    state: &mut UiState,
    ctx: &mut BuildCtx,
) -> Vec<Widget> {
    let mut widgets = Vec::new();
    for stmt in statements {
        if let Some(w) = build_widget_in_context(stmt, state, ctx) {
            widgets.push(w);
        }
    }
    widgets
}

fn build_widget_in_context(
    stmt: &UiStatement,
    state: &mut UiState,
    ctx: &mut BuildCtx,
) -> Option<Widget> {
    match stmt {
        UiStatement::State(s) => {
            let val = expr_to_value(&s.initial, state);
            state.values.insert(s.name.name.clone(), val);
            None
        }
        UiStatement::Element(el) => Some(build_element_in_context(el, state, ctx)),
        UiStatement::If(ui_if) => Some(build_if_in_context(ui_if, state, ctx)),
        UiStatement::For(ui_for) => Some(build_for_in_context(ui_for, state, ctx)),
        UiStatement::Component(component) => build_component_in_context(component, state, ctx),
    }
}

fn build_component_in_context(
    component: &ComponentUse,
    state: &UiState,
    ctx: &mut BuildCtx,
) -> Option<Widget> {
    let name = component.name.name.clone();
    if !ctx.active.insert(name.clone()) {
        return None;
    }

    // Clone the component reference so the borrow on ctx is released.
    let components = ctx.components;
    let result = components.get(&name).and_then(|decl| {
        decl.render.as_ref().map(|body| {
            let mut component_state = state.clone();
            for prop in &component.props {
                component_state
                    .values
                    .insert(prop.name.name.clone(), expr_to_value(&prop.value, state));
            }
            Widget::Container(build_widgets_in_context(body, &mut component_state, ctx))
        })
    });

    ctx.active.remove(&name);
    result
}

fn build_if_in_context(ui_if: &UiIf, state: &mut UiState, ctx: &mut BuildCtx) -> Widget {
    let condition = eval_condition(&ui_if.condition, state);
    if condition {
        Widget::If {
            condition: true,
            then_branch: build_widgets_in_context(&ui_if.then_body, state, ctx),
            else_branch: None,
        }
    } else if let Some(else_body) = &ui_if.else_body {
        Widget::If {
            condition: false,
            then_branch: vec![],
            else_branch: Some(build_widgets_in_context(else_body, state, ctx)),
        }
    } else {
        Widget::If {
            condition: false,
            then_branch: vec![],
            else_branch: None,
        }
    }
}

fn build_for_in_context(ui_for: &UiFor, state: &mut UiState, ctx: &mut BuildCtx) -> Widget {
    let items = if let Expr::Identifier(id) = &ui_for.iterable {
        state
            .get_value(&id.name)
            .map(|v| v.as_array())
            .unwrap_or_default()
    } else {
        vec![]
    };

    let mut all_widgets = Vec::new();
    for item in items {
        let mut temp_state = state.clone();
        temp_state.values.insert(ui_for.variable.name.clone(), item);
        let widgets = build_widgets_in_context(&ui_for.body, &mut temp_state, ctx);
        all_widgets.extend(widgets);
    }

    Widget::For {
        variable: ui_for.variable.name.clone(),
        items: all_widgets,
    }
}

/// Text of an interpolated string (`"hi {name}"`), evaluated against the UI state.
fn eval_interpolated(parts: &[aec_ast::InterpPart], state: &UiState) -> String {
    parts
        .iter()
        .map(|part| match part {
            aec_ast::InterpPart::Text(text) => text.clone(),
            aec_ast::InterpPart::Expr(expr) => eval_text_expr(expr, state),
        })
        .collect()
}

fn eval_condition(expr: &Expr, state: &UiState) -> bool {
    match expr {
        Expr::Literal(lit) => match &lit.value {
            aec_ast::Literal::Bool(b) => *b,
            _ => false,
        },
        Expr::Identifier(id) => state.get_bool(&id.name),
        Expr::Unary(u) => {
            if matches!(u.op, aec_ast::UnaryOp::Not) {
                !eval_condition(&u.operand, state)
            } else {
                false
            }
        }
        Expr::Binary(b) => {
            use aec_ast::BinaryOp;
            match b.op {
                BinaryOp::Eq => eval_value(&b.left, state) == eval_value(&b.right, state),
                BinaryOp::Neq => eval_value(&b.left, state) != eval_value(&b.right, state),
                BinaryOp::And => eval_condition(&b.left, state) && eval_condition(&b.right, state),
                BinaryOp::Or => eval_condition(&b.left, state) || eval_condition(&b.right, state),
                _ => false,
            }
        }
        _ => false,
    }
}

fn eval_value(expr: &Expr, state: &UiState) -> String {
    match expr {
        Expr::Literal(lit) => match &lit.value {
            aec_ast::Literal::String(s) => s.clone(),
            aec_ast::Literal::Int(n) => n.to_string(),
            aec_ast::Literal::Bool(b) => b.to_string(),
            aec_ast::Literal::Interpolated(parts) => eval_interpolated(parts, state),
            _ => String::new(),
        },
        Expr::Identifier(id) => state.get_string(&id.name),
        _ => String::new(),
    }
}

fn build_element_in_context(el: &ElementExpr, state: &mut UiState, ctx: &mut BuildCtx) -> Widget {
    let ws = build_element_style(el, ctx.theme);

    match el.name.name.as_str() {
        "Column" => {
            let children = el
                .children
                .as_ref()
                .map(|c| build_widgets_in_context(c, state, ctx))
                .unwrap_or_default();
            Widget::Column(children, ws)
        }
        "Row" => {
            let children = el
                .children
                .as_ref()
                .map(|c| build_widgets_in_context(c, state, ctx))
                .unwrap_or_default();
            Widget::Row(children, ws)
        }
        "Card" => {
            let children = el
                .children
                .as_ref()
                .map(|c| build_widgets_in_context(c, state, ctx))
                .unwrap_or_default();
            Widget::Card(children, apply_surface_default(ws, ctx.theme))
        }
        "Text" => {
            let text = if let Some(arg) = &el.primary_arg {
                eval_text_expr(arg, state)
            } else {
                String::new()
            };
            Widget::Text(text, apply_text_defaults(ws, ctx.theme))
        }
        "Display" => {
            let ws = apply_text_defaults(ws, ctx.theme);
            if let Some(Expr::Identifier(id)) = &el.primary_arg {
                Widget::Display {
                    var_name: id.name.clone(),
                    style: ws,
                }
            } else {
                Widget::Display {
                    var_name: "".to_string(),
                    style: ws,
                }
            }
        }
        "Divider" => Widget::Divider,
        "Heading" => {
            let text = el
                .primary_arg
                .as_ref()
                .and_then(|e| expr_to_string(e, state))
                .unwrap_or_default();
            Widget::Heading(text, apply_text_defaults(ws, ctx.theme))
        }
        "Spacer" => Widget::Spacer,
        "Input" => {
            let mut bind_target = None;
            let mut placeholder = None;
            for m in &el.modifiers {
                match m {
                    ElementModifier::Binding(b) => bind_target = Some(b.target.name.clone()),
                    ElementModifier::Property(p) => {
                        if p.name.name == "placeholder" {
                            placeholder = expr_to_string(&p.value, state);
                        }
                    }
                    _ => {}
                }
            }
            let value = bind_target
                .as_ref()
                .map(|t| state.get_string(t))
                .unwrap_or_default();
            Widget::Input {
                bind_target,
                placeholder,
                value,
                style: ws,
            }
        }
        "Button" => {
            let label = el
                .primary_arg
                .as_ref()
                .and_then(|e| expr_to_string(e, state))
                .unwrap_or_else(|| "Button".to_string());
            let mut on_click = None;
            for m in &el.modifiers {
                if let ElementModifier::Event(e) = m {
                    if e.event.name == "click" {
                        on_click = extract_fn_name(&e.handler);
                    }
                }
            }
            Widget::Button {
                label,
                on_click,
                style: ws,
            }
        }
        "Messages" => {
            let mut source = String::new();
            for m in &el.modifiers {
                if let ElementModifier::Property(p) = m {
                    if p.name.name == "list" || p.name.name == "data" {
                        if let Expr::Identifier(id) = &p.value {
                            source = id.name.clone();
                        }
                    }
                }
            }
            Widget::MessagesList { source, style: ws }
        }
        _ => {
            let children = el
                .children
                .as_ref()
                .map(|c| build_widgets_in_context(c, state, ctx))
                .unwrap_or_default();
            Widget::Container(children)
        }
    }
}

/// Final style of an element: merges the style block with inline properties, then resolves theme tokens.
fn build_element_style(el: &ElementExpr, theme: &ResolvedTheme) -> WidgetStyle {
    let mut ws = WidgetStyle::new();

    for (name, value) in &el.style.properties {
        if let Some(resolved) = resolve_style_value(value, theme) {
            ws.properties.insert(name.clone(), resolved);
        }
    }

    // Inline properties take priority over the style block.
    for m in &el.modifiers {
        if let ElementModifier::Property(p) = m {
            if let Some(value) = expr_to_style_value(&p.value, theme) {
                ws.properties.insert(p.name.name.clone(), value);
            }
        }
    }

    ws
}

/// Replaces a theme reference with its real value.
/// Non-theme values are returned unchanged.
/// An unresolved theme reference yields `None` (the property is dropped).
fn resolve_style_value(value: &StyleValue, theme: &ResolvedTheme) -> Option<StyleValue> {
    match value {
        StyleValue::Ident(path) => match path.strip_prefix("theme.") {
            Some(key) => theme.get(key).cloned(),
            None => Some(value.clone()),
        },
        other => Some(other.clone()),
    }
}

/// Converts an inline property into a style value, resolving theme references.
fn expr_to_style_value(expr: &Expr, theme: &ResolvedTheme) -> Option<StyleValue> {
    match expr {
        Expr::Literal(lit) => match &lit.value {
            aec_ast::Literal::String(s) => Some(StyleValue::String(s.clone())),
            aec_ast::Literal::Int(n) => Some(StyleValue::Int(*n)),
            aec_ast::Literal::Float(f) => Some(StyleValue::Float(*f)),
            aec_ast::Literal::Bool(b) => Some(StyleValue::Bool(*b)),
            _ => None,
        },
        Expr::Identifier(id) => Some(StyleValue::Ident(id.name.clone())),
        Expr::Member(_) => {
            let path = expr_token_path(expr)?;
            match path.strip_prefix("theme.") {
                Some(key) => theme.get(key).cloned(),
                None => Some(StyleValue::Ident(path)),
            }
        }
        _ => None,
    }
}

/// Builds the path of a chained expr such as `theme.color.primary`.
fn expr_token_path(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Identifier(id) => Some(id.name.clone()),
        Expr::Member(m) => {
            let base = expr_token_path(&m.object)?;
            Some(format!("{}.{}", base, m.property.name))
        }
        _ => None,
    }
}

/// Text defaults taken from the active theme (color.text / text.size / text.weight).
fn apply_text_defaults(mut ws: WidgetStyle, theme: &ResolvedTheme) -> WidgetStyle {
    let defaults = [
        ("color", "color.text"),
        ("size", "text.size"),
        ("weight", "text.weight"),
    ];
    for (prop, token) in defaults {
        if !ws.properties.contains_key(prop) {
            if let Some(value) = theme.get(token) {
                ws.properties.insert(prop.to_string(), value.clone());
            }
        }
    }
    ws
}

/// Card surface default taken from the active theme (color.surface).
fn apply_surface_default(mut ws: WidgetStyle, theme: &ResolvedTheme) -> WidgetStyle {
    if !ws.properties.contains_key("background") {
        if let Some(value) = theme.get("color.surface") {
            ws.properties
                .insert("background".to_string(), value.clone());
        }
    }
    ws
}

fn eval_text_expr(expr: &Expr, state: &UiState) -> String {
    match expr {
        Expr::Literal(lit) => match &lit.value {
            aec_ast::Literal::String(s) => s.clone(),
            aec_ast::Literal::Int(n) => n.to_string(),
            aec_ast::Literal::Interpolated(parts) => eval_interpolated(parts, state),
            _ => String::new(),
        },
        Expr::Identifier(id) => state.get_string(&id.name),
        Expr::Binary(b) => {
            use aec_ast::BinaryOp;
            if matches!(b.op, BinaryOp::Add) {
                format!(
                    "{}{}",
                    eval_text_expr(&b.left, state),
                    eval_text_expr(&b.right, state)
                )
            } else {
                String::new()
            }
        }
        _ => String::new(),
    }
}

fn expr_to_string(expr: &Expr, state: &UiState) -> Option<String> {
    match expr {
        Expr::Literal(lit) => match &lit.value {
            aec_ast::Literal::String(s) => Some(s.clone()),
            aec_ast::Literal::Int(n) => Some(n.to_string()),
            aec_ast::Literal::Float(f) => Some(f.to_string()),
            aec_ast::Literal::Bool(b) => Some(b.to_string()),
            aec_ast::Literal::Interpolated(parts) => Some(eval_interpolated(parts, state)),
            _ => None,
        },
        Expr::Identifier(id) => Some(id.name.clone()),
        _ => None,
    }
}

fn expr_to_value(expr: &Expr, state: &UiState) -> UiValue {
    match expr {
        Expr::Literal(lit) => match &lit.value {
            aec_ast::Literal::String(s) => UiValue::String(s.clone()),
            aec_ast::Literal::Int(n) => UiValue::Int(*n),
            aec_ast::Literal::Float(f) => UiValue::Float(*f),
            aec_ast::Literal::Bool(b) => UiValue::Bool(*b),
            aec_ast::Literal::Interpolated(parts) => {
                UiValue::String(eval_interpolated(parts, state))
            }
            _ => UiValue::String(String::new()),
        },
        Expr::Identifier(id) => state
            .get_value(&id.name)
            .cloned()
            .unwrap_or(UiValue::String(String::new())),
        Expr::Array(arr) => UiValue::Array(
            arr.elements
                .iter()
                .map(|element| expr_to_value(element, state))
                .collect(),
        ),
        Expr::Object(obj) => {
            let mut map = HashMap::new();
            for field in &obj.fields {
                map.insert(field.key.name.clone(), expr_to_value(&field.value, state));
            }
            UiValue::Object(map)
        }
        _ => UiValue::String(String::new()),
    }
}

fn extract_fn_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Identifier(id) => Some(id.name.clone()),
        Expr::Call(call) => {
            if let Expr::Identifier(id) = &call.callee {
                Some(id.name.clone())
            } else {
                None
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aec_ast::{
        ArrayExpr, ComponentProp, ElementProperty, Identifier, Literal, LiteralExpr, Span,
        ThemeDecl, ThemeEntry, ThemeGroup,
    };

    fn id(name: &str) -> Identifier {
        Identifier::new(name, Span::dummy())
    }

    fn str_expr(value: &str) -> Expr {
        Expr::Literal(Box::new(LiteralExpr {
            value: Literal::String(value.to_string()),
            span: Span::dummy(),
        }))
    }

    fn bool_expr(value: bool) -> Expr {
        Expr::Literal(Box::new(LiteralExpr {
            value: Literal::Bool(value),
            span: Span::dummy(),
        }))
    }

    fn arr_expr(items: &[&str]) -> Expr {
        Expr::Array(Box::new(ArrayExpr {
            elements: items.iter().map(|s| str_expr(s)).collect(),
            span: Span::dummy(),
        }))
    }

    fn ident_expr(name: &str) -> Expr {
        Expr::Identifier(id(name))
    }

    fn text(arg: Expr) -> UiStatement {
        UiStatement::Element(ElementExpr {
            name: id("Text"),
            primary_arg: Some(arg),
            modifiers: vec![],
            children: None,
            style: Style::new(),
            span: Span::dummy(),
        })
    }

    fn column(children: Vec<UiStatement>) -> UiStatement {
        UiStatement::Element(ElementExpr {
            name: id("Column"),
            primary_arg: None,
            modifiers: vec![],
            children: Some(children),
            style: Style::new(),
            span: Span::dummy(),
        })
    }

    fn ui_if(condition: Expr, then_body: Vec<UiStatement>) -> UiStatement {
        UiStatement::If(UiIf {
            condition,
            then_body,
            else_body: None,
            span: Span::dummy(),
        })
    }

    fn ui_for(variable: &str, iterable: Expr, body: Vec<UiStatement>) -> UiStatement {
        UiStatement::For(UiFor {
            variable: id(variable),
            iterable,
            body,
            span: Span::dummy(),
        })
    }

    fn use_component(name: &str, props: Vec<(&str, Expr)>) -> UiStatement {
        UiStatement::Component(ComponentUse {
            name: id(name),
            props: props
                .into_iter()
                .map(|(n, v)| ElementProperty {
                    name: id(n),
                    value: v,
                    span: Span::dummy(),
                })
                .collect(),
            span: Span::dummy(),
        })
    }

    fn decl(name: &str, props: &[&str], render: Vec<UiStatement>) -> (String, ComponentDecl) {
        (
            name.to_string(),
            ComponentDecl {
                name: id(name),
                props: props
                    .iter()
                    .map(|p| ComponentProp {
                        name: id(p),
                        ty: None,
                        span: Span::dummy(),
                    })
                    .collect(),
                render: Some(render),
                span: Span::dummy(),
            },
        )
    }

    fn register(registry: &mut ComponentRegistry, (name, decl): (String, ComponentDecl)) {
        registry.insert(name, decl);
    }

    fn first_text(widget: &Widget) -> Option<String> {
        match widget {
            Widget::Container(children) => children.iter().find_map(first_text),
            Widget::Column(children, _) => children.iter().find_map(first_text),
            Widget::Row(children, _) => children.iter().find_map(first_text),
            Widget::Card(children, _) => children.iter().find_map(first_text),
            Widget::For { items, .. } => items.iter().find_map(first_text),
            Widget::If {
                condition,
                then_branch,
                else_branch,
            } => {
                if *condition {
                    then_branch.iter().find_map(first_text)
                } else {
                    else_branch
                        .as_ref()
                        .and_then(|b| b.iter().find_map(first_text))
                }
            }
            Widget::Text(t, _) => Some(t.clone()),
            _ => None,
        }
    }

    #[test]
    fn expands_component_prop_into_render_body() {
        let mut registry = ComponentRegistry::new();
        register(
            &mut registry,
            decl("MessageBubble", &["role"], vec![text(ident_expr("role"))]),
        );

        let stmts = vec![use_component(
            "MessageBubble",
            vec![("role", str_expr("user"))],
        )];
        let mut state = UiState::new();
        let widgets = build_widgets_with_components(&stmts, &mut state, &registry);

        assert_eq!(widgets.len(), 1);
        assert_eq!(first_text(&widgets[0]).as_deref(), Some("user"));
    }

    #[test]
    fn passes_multiple_props_to_render_body() {
        let mut registry = ComponentRegistry::new();
        register(
            &mut registry,
            decl(
                "MessageBubble",
                &["role", "content"],
                vec![text(ident_expr("role")), text(ident_expr("content"))],
            ),
        );

        let stmts = vec![use_component(
            "MessageBubble",
            vec![("role", str_expr("ai")), ("content", str_expr("سلام"))],
        )];
        let mut state = UiState::new();
        let widgets = build_widgets_with_components(&stmts, &mut state, &registry);

        let container = match &widgets[0] {
            Widget::Container(children) => children,
            other => panic!("expected Container, got {:?}", other),
        };
        assert_eq!(container.len(), 2);
        assert!(matches!(&container[0], Widget::Text(t, _) if t == "ai"));
        assert!(matches!(&container[1], Widget::Text(t, _) if t == "سلام"));
    }

    #[test]
    fn unknown_component_produces_no_widget() {
        let registry = ComponentRegistry::new();
        let stmts = vec![use_component("Ghost", vec![])];
        let mut state = UiState::new();
        let widgets = build_widgets_with_components(&stmts, &mut state, &registry);
        assert!(widgets.is_empty());
    }

    #[test]
    fn direct_recursion_is_guarded() {
        let mut registry = ComponentRegistry::new();
        register(
            &mut registry,
            decl("Self", &[], vec![use_component("Self", vec![])]),
        );

        let stmts = vec![use_component("Self", vec![])];
        let mut state = UiState::new();
        let widgets = build_widgets_with_components(&stmts, &mut state, &registry);

        // Must not hang or stack-overflow; it expands only once.
        assert_eq!(widgets.len(), 1);
        match &widgets[0] {
            Widget::Container(children) => assert!(children.is_empty()),
            other => panic!("expected Container, got {:?}", other),
        }
    }

    #[test]
    fn indirect_recursion_is_guarded() {
        let mut registry = ComponentRegistry::new();
        register(
            &mut registry,
            decl("A", &[], vec![use_component("B", vec![])]),
        );
        register(
            &mut registry,
            decl("B", &[], vec![use_component("A", vec![])]),
        );

        let stmts = vec![use_component("A", vec![])];
        let mut state = UiState::new();
        let widgets = build_widgets_with_components(&stmts, &mut state, &registry);

        assert_eq!(widgets.len(), 1);
    }

    #[test]
    fn nested_component_inside_column_expands() {
        let mut registry = ComponentRegistry::new();
        register(
            &mut registry,
            decl("MessageBubble", &["role"], vec![text(ident_expr("role"))]),
        );

        let stmts = vec![column(vec![use_component(
            "MessageBubble",
            vec![("role", str_expr("user"))],
        )])];
        let mut state = UiState::new();
        let widgets = build_widgets_with_components(&stmts, &mut state, &registry);

        assert_eq!(widgets.len(), 1);
        assert_eq!(first_text(&widgets[0]).as_deref(), Some("user"));
    }

    #[test]
    fn component_can_use_prop_in_if_condition() {
        let mut registry = ComponentRegistry::new();
        register(
            &mut registry,
            decl(
                "Banner",
                &["show"],
                vec![ui_if(ident_expr("show"), vec![text(str_expr("shown"))])],
            ),
        );

        let stmts = vec![use_component("Banner", vec![("show", bool_expr(true))])];
        let mut state = UiState::new();
        let widgets = build_widgets_with_components(&stmts, &mut state, &registry);

        assert_eq!(first_text(&widgets[0]).as_deref(), Some("shown"));
    }

    #[test]
    fn component_can_use_prop_in_for_loop() {
        let mut registry = ComponentRegistry::new();
        register(
            &mut registry,
            decl(
                "List",
                &["items"],
                vec![ui_for(
                    "item",
                    ident_expr("items"),
                    vec![text(ident_expr("item"))],
                )],
            ),
        );

        let stmts = vec![use_component(
            "List",
            vec![("items", arr_expr(&["a", "b", "c"]))],
        )];
        let mut state = UiState::new();
        let widgets = build_widgets_with_components(&stmts, &mut state, &registry);

        let container = match &widgets[0] {
            Widget::Container(children) => children,
            other => panic!("expected Container, got {:?}", other),
        };
        let mut texts = Vec::new();
        collect_texts(container, &mut texts);
        assert_eq!(texts, vec!["a", "b", "c"]);
    }

    #[test]
    fn component_state_does_not_leak_to_parent() {
        let mut registry = ComponentRegistry::new();
        register(
            &mut registry,
            decl(
                "Counter",
                &[],
                vec![
                    UiStatement::State(aec_ast::StateDeclUi {
                        name: id("secret"),
                        ty: None,
                        initial: str_expr("hidden"),
                        span: Span::dummy(),
                    }),
                    text(ident_expr("secret")),
                ],
            ),
        );

        let stmts = vec![use_component("Counter", vec![])];
        let mut state = UiState::new();
        let _ = build_widgets_with_components(&stmts, &mut state, &registry);

        assert!(
            state.get_value("secret").is_none(),
            "component-local state must not leak into parent state"
        );
    }

    #[test]
    fn build_widgets_compat_still_builds_plain_elements() {
        let stmts = vec![text(str_expr("hi"))];
        let mut state = UiState::new();
        let widgets = build_widgets(&stmts, &mut state);
        assert_eq!(widgets.len(), 1);
        assert!(matches!(&widgets[0], Widget::Text(t, _) if t == "hi"));
    }

    fn collect_texts(widgets: &[Widget], out: &mut Vec<String>) {
        for widget in widgets {
            match widget {
                Widget::Container(children) => collect_texts(children, out),
                Widget::Column(children, _) => collect_texts(children, out),
                Widget::Row(children, _) => collect_texts(children, out),
                Widget::Card(children, _) => collect_texts(children, out),
                Widget::For { items, .. } => collect_texts(items, out),
                Widget::If {
                    then_branch,
                    else_branch,
                    ..
                } => {
                    collect_texts(then_branch, out);
                    if let Some(else_branch) = else_branch {
                        collect_texts(else_branch, out);
                    }
                }
                Widget::Text(t, _) => out.push(t.clone()),
                _ => {}
            }
        }
    }

    // ---------- Theme tests ----------

    fn member_expr(path: &[&str]) -> Expr {
        let mut iter = path.iter();
        let mut expr = ident_expr(iter.next().unwrap());
        for segment in iter {
            expr = Expr::Member(Box::new(aec_ast::MemberExpr {
                object: expr,
                property: id(segment),
                span: Span::dummy(),
            }));
        }
        expr
    }

    fn theme_decl(
        name: &str,
        is_default: bool,
        extends: Option<&str>,
        groups: Vec<(&str, Vec<(&str, StyleValue)>)>,
    ) -> ThemeDecl {
        ThemeDecl {
            name: id(name),
            is_default,
            extends: extends.map(id),
            groups: groups
                .into_iter()
                .map(|(group, entries)| ThemeGroup {
                    name: id(group),
                    entries: entries
                        .into_iter()
                        .map(|(key, value)| ThemeEntry {
                            name: id(key),
                            value,
                            span: Span::dummy(),
                        })
                        .collect(),
                    span: Span::dummy(),
                })
                .collect(),
            span: Span::dummy(),
        }
    }

    fn themes_from(decls: Vec<ThemeDecl>) -> Themes {
        let items: Vec<TopLevelItem> = decls.into_iter().map(TopLevelItem::Theme).collect();
        Themes::from_items(&items)
    }

    fn string_token(value: &str) -> StyleValue {
        StyleValue::String(value.to_string())
    }

    fn element_with_style(name: &str, props: Vec<(&str, StyleValue)>) -> UiStatement {
        let mut style = Style::new();
        for (key, value) in props {
            style.properties.insert(key.to_string(), value);
        }
        UiStatement::Element(ElementExpr {
            name: id(name),
            primary_arg: None,
            modifiers: vec![],
            children: None,
            style,
            span: Span::dummy(),
        })
    }

    fn element_with_property(name: &str, prop: &str, value: Expr) -> UiStatement {
        UiStatement::Element(ElementExpr {
            name: id(name),
            primary_arg: None,
            modifiers: vec![ElementModifier::Property(ElementProperty {
                name: id(prop),
                value,
                span: Span::dummy(),
            })],
            children: None,
            style: Style::new(),
            span: Span::dummy(),
        })
    }

    fn style_of(widget: &Widget) -> &WidgetStyle {
        match widget {
            Widget::Text(_, s) => s,
            Widget::Column(_, s) => s,
            Widget::Row(_, s) => s,
            Widget::Card(_, s) => s,
            Widget::Heading(_, s) => s,
            other => panic!("widget has no style: {:?}", other),
        }
    }

    #[test]
    fn resolves_theme_token_in_style_block() {
        let themes = themes_from(vec![theme_decl(
            "Dark",
            false,
            None,
            vec![("color", vec![("primary", string_token("#89b4fa"))])],
        )]);
        let components = ComponentRegistry::new();
        let stmts = vec![element_with_style(
            "Text",
            vec![(
                "color",
                StyleValue::Ident("theme.color.primary".to_string()),
            )],
        )];
        let mut state = UiState::new();
        let widgets = build_widgets_with_themes(&stmts, &mut state, &components, &themes);

        assert_eq!(
            style_of(&widgets[0]).get_string("color").as_deref(),
            Some("#89b4fa")
        );
    }

    #[test]
    fn resolves_theme_token_in_inline_property() {
        let themes = themes_from(vec![theme_decl(
            "Dark",
            false,
            None,
            vec![("color", vec![("primary", string_token("#89b4fa"))])],
        )]);
        let components = ComponentRegistry::new();
        let stmts = vec![element_with_property(
            "Text",
            "color",
            member_expr(&["theme", "color", "primary"]),
        )];
        let mut state = UiState::new();
        let widgets = build_widgets_with_themes(&stmts, &mut state, &components, &themes);

        assert_eq!(
            style_of(&widgets[0]).get_string("color").as_deref(),
            Some("#89b4fa")
        );
    }

    #[test]
    fn unresolved_theme_token_is_dropped() {
        let themes = themes_from(vec![theme_decl("Dark", false, None, vec![])]);
        let components = ComponentRegistry::new();
        let stmts = vec![element_with_style(
            "Text",
            vec![(
                "color",
                StyleValue::Ident("theme.color.missing".to_string()),
            )],
        )];
        let mut state = UiState::new();
        let widgets = build_widgets_with_themes(&stmts, &mut state, &components, &themes);

        assert!(style_of(&widgets[0]).get_string("color").is_none());
    }

    #[test]
    fn extends_inherits_and_overrides() {
        let themes = themes_from(vec![
            theme_decl(
                "Dark",
                false,
                None,
                vec![(
                    "color",
                    vec![
                        ("primary", string_token("#111111")),
                        ("text", string_token("#eeeeee")),
                    ],
                )],
            ),
            theme_decl(
                "Ocean",
                false,
                Some("Dark"),
                vec![("color", vec![("primary", string_token("#00b4d8"))])],
            ),
        ]);

        let ocean = themes.get("Ocean").expect("Ocean should resolve");
        assert!(
            matches!(ocean.get("color.primary"), Some(StyleValue::String(s)) if s == "#00b4d8")
        );
        assert!(matches!(ocean.get("color.text"), Some(StyleValue::String(s)) if s == "#eeeeee"));
    }

    #[test]
    fn default_theme_applies_text_defaults() {
        let themes = themes_from(vec![theme_decl(
            "Dark",
            true,
            None,
            vec![
                ("color", vec![("text", string_token("#cdd6f4"))]),
                ("text", vec![("size", StyleValue::Int(16))]),
            ],
        )]);
        assert!(themes.active().is_some());

        let components = ComponentRegistry::new();
        let stmts = vec![element_with_style("Text", vec![])];
        let mut state = UiState::new();
        let widgets = build_widgets_with_themes(&stmts, &mut state, &components, &themes);

        let ws = style_of(&widgets[0]);
        assert_eq!(ws.get_string("color").as_deref(), Some("#cdd6f4"));
        assert_eq!(ws.get_float("size"), Some(16.0));
    }

    #[test]
    fn explicit_style_overrides_theme_default() {
        let themes = themes_from(vec![theme_decl(
            "Dark",
            true,
            None,
            vec![("color", vec![("text", string_token("#cdd6f4"))])],
        )]);
        let components = ComponentRegistry::new();
        let stmts = vec![element_with_style(
            "Text",
            vec![("color", string_token("#ff0000"))],
        )];
        let mut state = UiState::new();
        let widgets = build_widgets_with_themes(&stmts, &mut state, &components, &themes);

        assert_eq!(
            style_of(&widgets[0]).get_string("color").as_deref(),
            Some("#ff0000")
        );
    }

    #[test]
    fn card_gets_surface_default() {
        let themes = themes_from(vec![theme_decl(
            "Dark",
            true,
            None,
            vec![("color", vec![("surface", string_token("#313244"))])],
        )]);
        let components = ComponentRegistry::new();
        let stmts = vec![UiStatement::Element(ElementExpr {
            name: id("Card"),
            primary_arg: None,
            modifiers: vec![],
            children: Some(vec![]),
            style: Style::new(),
            span: Span::dummy(),
        })];
        let mut state = UiState::new();
        let widgets = build_widgets_with_themes(&stmts, &mut state, &components, &themes);

        assert_eq!(
            style_of(&widgets[0]).get_string("background").as_deref(),
            Some("#313244")
        );
    }

    #[test]
    fn interpolated_text_uses_state_values() {
        let stmts = vec![text(Expr::Literal(Box::new(LiteralExpr {
            value: Literal::Interpolated(vec![
                aec_ast::InterpPart::Text("hi ".to_string()),
                aec_ast::InterpPart::Expr(ident_expr("name")),
                aec_ast::InterpPart::Text("!".to_string()),
            ]),
            span: Span::dummy(),
        })))];
        let mut state = UiState::new();
        state.set_string("name", "Ada".to_string());

        let widgets = build_widgets(&stmts, &mut state);
        assert!(
            matches!(&widgets[0], Widget::Text(t, _) if t == "hi Ada!"),
            "got {:?}",
            widgets
        );
    }

    #[test]
    fn interpolated_state_value_is_evaluated() {
        let stmts = vec![UiStatement::State(aec_ast::StateDeclUi {
            name: id("greeting"),
            ty: None,
            initial: Expr::Literal(Box::new(LiteralExpr {
                value: Literal::Interpolated(vec![
                    aec_ast::InterpPart::Text("hello ".to_string()),
                    aec_ast::InterpPart::Expr(ident_expr("who")),
                ]),
                span: Span::dummy(),
            })),
            span: Span::dummy(),
        })];
        let mut state = UiState::new();
        state.set_string("who", "world".to_string());

        let _ = build_widgets(&stmts, &mut state);
        assert_eq!(state.get_string("greeting"), "hello world");
    }

    #[test]
    fn extends_cycle_does_not_hang() {
        let themes = themes_from(vec![
            theme_decl(
                "A",
                false,
                Some("B"),
                vec![("g", vec![("x", StyleValue::Int(1))])],
            ),
            theme_decl(
                "B",
                false,
                Some("A"),
                vec![("g", vec![("y", StyleValue::Int(2))])],
            ),
        ]);
        // It must simply terminate, without a stack overflow.
        assert!(themes.get("A").is_some());
        assert!(themes.get("B").is_some());
    }
}
