//! Widget tree

use aec_ast::{
    ComponentDecl, ComponentUse, ElementExpr, ElementModifier, Expr, Style, StyleValue, ThemeDecl,
    TopLevelItem, UiFor, UiIf, UiStatement, Span,
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
        let modules = vec![None; items.len()];
        Self::from_items_with_modules(items, &modules)
    }

    pub fn from_items_with_modules(
        items: &[TopLevelItem],
        item_modules: &[Option<String>],
    ) -> Self {
        let decls: Vec<(String, &ThemeDecl, Option<String>)> = items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| match item {
                TopLevelItem::Theme(theme) => {
                    let module = item_modules.get(index).cloned().flatten();
                    let key = theme_key(module.as_deref(), &theme.name.name, theme.is_public);
                    Some((key, theme, module))
                }
                _ => None,
            })
            .collect();

        let mut named = HashMap::new();
        for (key, theme, module) in &decls {
            let resolved = resolve_theme(theme, &decls, key, module.as_deref(), &mut HashSet::new());
            named.insert(key.clone(), resolved);
        }

        let roots: Vec<_> = decls
            .iter()
            .filter(|(_, _, module)| module.is_none())
            .collect();
        let default = roots
            .iter()
            .find(|(_, theme, _)| theme.is_default)
            .and_then(|(key, _, _)| named.get(key).cloned())
            .or_else(|| {
                (roots.len() == 1)
                    .then(|| named.get(&roots[0].0).cloned())
                    .flatten()
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
fn theme_key(module: Option<&str>, name: &str, is_public: bool) -> String {
    match module {
        Some(module) if is_public => format!("{}.{}", module, name),
        Some(module) => format!("{}::{}", module, name),
        None => name.to_string(),
    }
}

fn scoped_component_name(module: Option<&str>, name: &str) -> String {
    module
        .map(|module| format!("{}::{}", module, name))
        .unwrap_or_else(|| name.to_string())
}

/// Flattens a theme by following its `extends` chain.
fn resolve_theme(
    decl: &ThemeDecl,
    all: &[(String, &ThemeDecl, Option<String>)],
    key: &str,
    module: Option<&str>,
    visited: &mut HashSet<String>,
) -> ResolvedTheme {
    if !visited.insert(key.to_string()) {
        return ResolvedTheme::default();
    }

    let mut resolved = decl
        .extends
        .as_ref()
        .and_then(|parent| {
            let private_key = scoped_component_name(module, &parent.name);
            let public_key = module
                .map(|module| format!("{}.{}", module, parent.name))
                .unwrap_or_else(|| parent.name.clone());
            all.iter()
                .find(|(candidate_key, _, _)| {
                    candidate_key == &private_key || candidate_key == &public_key
                })
                .or_else(|| {
                    all.iter()
                        .find(|(_, candidate, _)| candidate.name.name == parent.name)
                })
                .map(|(parent_key, parent, parent_module)| {
                    resolve_theme(
                        parent,
                        all,
                        parent_key,
                        parent_module.as_deref(),
                        visited,
                    )
                })
        })
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

impl Default for WidgetStyle {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct EventAction {
    pub handler: Expr,
    pub span: Span,
    pub scope: Option<String>,
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
        on_click: Option<EventAction>,
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

#[derive(Debug, Clone, PartialEq)]
pub enum UiValue {
    None,
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
            UiValue::None => "none".to_string(),
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
            UiValue::None => false,
            UiValue::Bool(b) => *b,
            UiValue::String(s) => !s.is_empty(),
            UiValue::Int(n) => *n != 0,
            UiValue::Float(n) => *n != 0.0,
            _ => false,
        }
    }

    pub fn as_array(&self) -> Vec<UiValue> {
        match self {
            UiValue::Array(a) => a.clone(),
            _ => vec![],
        }
    }

    pub fn is_truthy(&self) -> bool {
        match self {
            UiValue::None => false,
            UiValue::String(value) => !value.is_empty(),
            UiValue::Int(value) => *value != 0,
            UiValue::Float(value) => *value != 0.0,
            UiValue::Bool(value) => *value,
            UiValue::Array(value) => !value.is_empty(),
            UiValue::Object(value) => !value.is_empty(),
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

    fn scoped_key(scope: &str, name: &str) -> String {
        format!("{}::{}", scope, name)
    }

    fn get_scoped_value(&self, scope: Option<&str>, name: &str) -> Option<&UiValue> {
        let key = scope.map(|scope| Self::scoped_key(scope, name));
        key.and_then(|key| self.values.get(&key))
            .or_else(|| self.values.get(name))
    }

    fn set_scoped_value(&mut self, scope: Option<&str>, name: &str, value: UiValue) {
        let key = scope.map(|scope| Self::scoped_key(scope, name));
        self.values.insert(key.unwrap_or_else(|| name.to_string()), value);
    }

    fn retain_scopes(&mut self, active: &HashSet<String>) {
        self.values.retain(|key, _| {
            !key.contains("::")
                || active
                    .iter()
                    .any(|scope| key.starts_with(&format!("{}::", scope)))
        });
    }
}

impl Default for UiState {
    fn default() -> Self {
        Self::new()
    }
}

/// Build context: components, the active theme, and the recursion guard.
struct BuildCtx<'a> {
    components: &'a ComponentRegistry,
    theme: &'a ResolvedTheme,
    themes: &'a Themes,
    active: HashSet<String>,
    scope: Option<String>,
    module: Option<String>,
    next_scope: usize,
    used_scopes: HashSet<String>,
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
        themes,
        active: HashSet::new(),
        scope: None,
        module: None,
        next_scope: 0,
        used_scopes: HashSet::new(),
    };
    let widgets = build_widgets_in_context(statements, state, &mut ctx);
    state.retain_scopes(&ctx.used_scopes);
    widgets
}

fn build_widgets_in_context(
    statements: &[UiStatement],
    state: &mut UiState,
    ctx: &mut BuildCtx,
) -> Vec<Widget> {
    for statement in statements {
        if let UiStatement::State(declaration) = statement {
            let key = state_key(ctx.scope.as_deref(), &declaration.name.name);
            if !state.values.contains_key(&key) {
                let value = expr_to_value(&declaration.initial, state, ctx.scope.as_deref());
                state.values.insert(key, value);
            }
        }
    }

    let mut widgets = Vec::new();
    for statement in statements {
        if let Some(widget) = build_widget_in_context(statement, state, ctx) {
            widgets.push(widget);
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
        UiStatement::State(_) => None,
        UiStatement::Element(el) => Some(build_element_in_context(el, state, ctx)),
        UiStatement::If(ui_if) => Some(build_if_in_context(ui_if, state, ctx)),
        UiStatement::For(ui_for) => Some(build_for_in_context(ui_for, state, ctx)),
        UiStatement::Component(component) => build_component_in_context(component, state, ctx),
    }
}

fn build_component_in_context(
    component: &ComponentUse,
    state: &mut UiState,
    ctx: &mut BuildCtx,
) -> Option<Widget> {
    let requested_name = component.name.name.clone();
    let lookup_name = if requested_name.contains('.') {
        ctx.module
            .as_deref()
            .map(|module| format!("{}.{}", module, requested_name))
            .unwrap_or_else(|| requested_name.clone())
    } else {
        ctx.module
            .as_deref()
            .map(|module| format!("{}::{}", module, requested_name))
            .unwrap_or_else(|| requested_name.clone())
    };
    let declaration = ctx
        .components
        .get(&lookup_name)
        .or_else(|| ctx.components.get(&requested_name))
        .cloned()?;
    let module = if requested_name.contains('.') {
        let qualified_requested = ctx
            .module
            .as_deref()
            .map(|module| format!("{}.{}", module, requested_name))
            .unwrap_or_else(|| requested_name.clone());
        qualified_requested
            .rsplit_once('.')
            .map(|(module, _)| module.to_string())
    } else {
        ctx.module.clone()
    };
    let identity = format!(
        "{}::{}",
        module.as_deref().unwrap_or_default(),
        declaration.name.name
    );
    if !ctx.active.insert(identity.clone()) {
        return None;
    }

    let result = declaration.render.as_ref().map(|body| {
        let scope = format!("component:{}:{}", identity, ctx.next_scope);
        ctx.next_scope += 1;
        ctx.used_scopes.insert(scope.clone());
        let previous_scope = ctx.scope.replace(scope.clone());
        let previous_module = ctx.module.clone();
        ctx.module = module;
        for prop in &component.props {
            let value = expr_to_value(&prop.value, state, previous_scope.as_deref());
            state.set_scoped_value(Some(&scope), &prop.name.name, value);
        }
        let widgets = build_widgets_in_context(body, state, ctx);
        ctx.scope = previous_scope;
        ctx.module = previous_module;
        Widget::Container(widgets)
    });

    ctx.active.remove(&identity);
    result
}

fn build_if_in_context(ui_if: &UiIf, state: &mut UiState, ctx: &mut BuildCtx) -> Widget {
    let condition = eval_condition(&ui_if.condition, state, ctx.scope.as_deref());
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
    let items = expr_to_value(&ui_for.iterable, state, ctx.scope.as_deref()).as_array();
    let mut all_widgets = Vec::new();
    for (index, item) in items.into_iter().enumerate() {
        let scope = format!("loop:{}:{}:{}", ui_for.variable.name, index, ctx.next_scope);
        ctx.next_scope += 1;
        ctx.used_scopes.insert(scope.clone());
        let previous_scope = ctx.scope.replace(scope.clone());
        state.set_scoped_value(Some(&scope), &ui_for.variable.name, item);
        all_widgets.extend(build_widgets_in_context(&ui_for.body, state, ctx));
        ctx.scope = previous_scope;
    }

    Widget::For {
        variable: ui_for.variable.name.clone(),
        items: all_widgets,
    }
}

fn state_key(scope: Option<&str>, name: &str) -> String {
    scope.map(|scope| format!("{}::{}", scope, name)).unwrap_or_else(|| name.to_string())
}

fn eval_interpolated(
    parts: &[aec_ast::InterpPart],
    state: &UiState,
    scope: Option<&str>,
) -> String {
    parts
        .iter()
        .map(|part| match part {
            aec_ast::InterpPart::Text(text) => text.clone(),
            aec_ast::InterpPart::Expr(expr) => eval_text_expr(expr, state, scope),
        })
        .collect()
}

fn eval_condition(expr: &Expr, state: &UiState, scope: Option<&str>) -> bool {
    eval_ui_value(expr, state, scope).is_truthy()
}

fn eval_text_expr(expr: &Expr, state: &UiState, scope: Option<&str>) -> String {
    eval_ui_value(expr, state, scope).as_string()
}

fn expr_to_string(expr: &Expr, state: &UiState, scope: Option<&str>) -> Option<String> {
    Some(eval_text_expr(expr, state, scope))
}

fn expr_to_value(expr: &Expr, state: &UiState, scope: Option<&str>) -> UiValue {
    eval_ui_value(expr, state, scope)
}

fn eval_ui_value(expr: &Expr, state: &UiState, scope: Option<&str>) -> UiValue {
    match expr {
        Expr::Literal(literal) => match &literal.value {
            aec_ast::Literal::None => UiValue::None,
            aec_ast::Literal::String(value) | aec_ast::Literal::RawString(value) => {
                UiValue::String(value.clone())
            }
            aec_ast::Literal::Interpolated(parts) => {
                UiValue::String(eval_interpolated(parts, state, scope))
            }
            aec_ast::Literal::Int(value) => UiValue::Int(*value),
            aec_ast::Literal::Float(value) => UiValue::Float(*value),
            aec_ast::Literal::Bool(value) => UiValue::Bool(*value),
            aec_ast::Literal::Uuid(value) => UiValue::String(value.to_string()),
            aec_ast::Literal::ByteSize(value) => UiValue::Int(*value as i64),
            aec_ast::Literal::Duration(value) => {
                UiValue::Int(value.value.saturating_mul(value.unit.to_ms()) as i64)
            }
        },
        Expr::Identifier(identifier) => state
            .get_scoped_value(scope, &identifier.name)
            .cloned()
            .unwrap_or(UiValue::String(String::new())),
        Expr::Paren(parenthesized) => eval_ui_value(&parenthesized.inner, state, scope),
        Expr::Array(array) => UiValue::Array(
            array
                .elements
                .iter()
                .map(|element| eval_ui_value(element, state, scope))
                .collect(),
        ),
        Expr::Object(object) => {
            let mut values = HashMap::new();
            for field in &object.fields {
                values.insert(
                    field.key.name.clone(),
                    eval_ui_value(&field.value, state, scope),
                );
            }
            UiValue::Object(values)
        }
        Expr::Unary(unary) => {
            let value = eval_ui_value(&unary.operand, state, scope);
            match unary.op {
                aec_ast::UnaryOp::Not => UiValue::Bool(!value.is_truthy()),
                aec_ast::UnaryOp::Neg => match value {
                    UiValue::Int(value) => UiValue::Int(value.saturating_neg()),
                    UiValue::Float(value) => UiValue::Float(-value),
                    _ => UiValue::None,
                },
            }
        }
        Expr::Binary(binary) => {
            let left = eval_ui_value(&binary.left, state, scope);
            let right = eval_ui_value(&binary.right, state, scope);
            ui_binary(binary.op, left, right)
        }
        Expr::Member(member) => match eval_ui_value(&member.object, state, scope) {
            UiValue::Object(values) => values
                .get(&member.property.name)
                .cloned()
                .unwrap_or(UiValue::None),
            _ => UiValue::None,
        },
        Expr::Index(index) => {
            let object = eval_ui_value(&index.object, state, scope);
            let key = eval_ui_value(&index.index, state, scope);
            match (object, key) {
                (UiValue::Array(values), UiValue::Int(index)) => {
                    let index = if index < 0 { values.len() as i64 + index } else { index };
                    if index < 0 || index as usize >= values.len() {
                        UiValue::None
                    } else {
                        values[index as usize].clone()
                    }
                }
                (UiValue::String(value), UiValue::Int(index)) => {
                    let characters: Vec<char> = value.chars().collect();
                    let index = if index < 0 { characters.len() as i64 + index } else { index };
                    if index < 0 || index as usize >= characters.len() {
                        UiValue::None
                    } else {
                        UiValue::String(characters[index as usize].to_string())
                    }
                }
                (UiValue::Object(values), UiValue::String(key)) => {
                    values.get(&key).cloned().unwrap_or(UiValue::None)
                }
                _ => UiValue::None,
            }
        }
        Expr::Call(call) => eval_ui_call(call, state, scope),
        _ => UiValue::None,
    }
}

fn eval_ui_call(call: &aec_ast::CallExpr, state: &UiState, scope: Option<&str>) -> UiValue {
    let name = match &call.callee {
        Expr::Identifier(identifier) => identifier.name.clone(),
        Expr::Member(member) => match &member.object {
            Expr::Identifier(namespace) => format!("{}.{}", namespace.name, member.property.name),
            _ => return UiValue::None,
        },
        _ => return UiValue::None,
    };
    let args: Vec<UiValue> = call
        .args
        .iter()
        .map(|argument| eval_ui_value(&argument.value, state, scope))
        .collect();
    match name.as_str() {
        "range" => {
            let (start, end) = match args.as_slice() {
                [UiValue::Int(end)] => (0, *end),
                [UiValue::Int(start), UiValue::Int(end)] => (*start, *end),
                _ => return UiValue::None,
            };
            if start < 0 || end < start || end - start > 10_000_000 {
                return UiValue::None;
            }
            UiValue::Array((start..end).map(UiValue::Int).collect())
        }
        "len" => match args.first() {
            Some(UiValue::String(value)) => UiValue::Int(value.chars().count() as i64),
            Some(UiValue::Array(value)) => UiValue::Int(value.len() as i64),
            Some(UiValue::Object(value)) => UiValue::Int(value.len() as i64),
            _ => UiValue::None,
        },
        "str" => args.first().map(|value| UiValue::String(value.as_string())).unwrap_or(UiValue::None),
        "int" => match args.first() {
            Some(UiValue::Int(value)) => UiValue::Int(*value),
            Some(UiValue::Float(value)) => UiValue::Int(*value as i64),
            Some(UiValue::String(value)) => value.parse().map(UiValue::Int).unwrap_or(UiValue::None),
            _ => UiValue::None,
        },
        "float" => match args.first() {
            Some(UiValue::Int(value)) => UiValue::Float(*value as f64),
            Some(UiValue::Float(value)) => UiValue::Float(*value),
            Some(UiValue::String(value)) => value.parse().map(UiValue::Float).unwrap_or(UiValue::None),
            _ => UiValue::None,
        },
        "upper" | "lower" | "trim" => match args.first() {
            Some(UiValue::String(value)) => UiValue::String(match name.as_str() {
                "upper" => value.to_uppercase(),
                "lower" => value.to_lowercase(),
                _ => value.trim().to_string(),
            }),
            _ => UiValue::None,
        },
        "split" => match args.as_slice() {
            [UiValue::String(value), UiValue::String(separator)] => UiValue::Array(
                value.split(separator.as_str()).map(|part| UiValue::String(part.to_string())).collect(),
            ),
            _ => UiValue::None,
        },
        "first" | "last" => match args.first() {
            Some(UiValue::Array(values)) => {
                if name == "first" {
                    values.first().cloned().unwrap_or(UiValue::None)
                } else {
                    values.last().cloned().unwrap_or(UiValue::None)
                }
            }
            _ => UiValue::None,
        },
        "push" => match args.as_slice() {
            [UiValue::Array(values), value] => {
                let mut values = values.clone();
                values.push(value.clone());
                UiValue::Array(values)
            }
            _ => UiValue::None,
        },
        "pop" => match args.first() {
            Some(UiValue::Array(values)) => values.last().cloned().unwrap_or(UiValue::None),
            _ => UiValue::None,
        },
        "abs" => match args.first() {
            Some(UiValue::Int(value)) => UiValue::Int(value.saturating_abs()),
            Some(UiValue::Float(value)) => UiValue::Float(value.abs()),
            _ => UiValue::None,
        },
        "min" | "max" => match args.as_slice() {
            [UiValue::Int(left), UiValue::Int(right)] => UiValue::Int(if name == "min" { *left.min(right) } else { *left.max(right) }),
            [UiValue::Float(left), UiValue::Float(right)] => UiValue::Float(if name == "min" { left.min(*right) } else { left.max(*right) }),
            _ => UiValue::None,
        },
        _ => UiValue::None,
    }
}

fn ui_binary(op: aec_ast::BinaryOp, left: UiValue, right: UiValue) -> UiValue {
    use aec_ast::BinaryOp;
    match op {
        BinaryOp::Eq => UiValue::Bool(ui_values_equal(&left, &right)),
        BinaryOp::Neq => UiValue::Bool(!ui_values_equal(&left, &right)),
        BinaryOp::And => UiValue::Bool(left.is_truthy() && right.is_truthy()),
        BinaryOp::Or => UiValue::Bool(left.is_truthy() || right.is_truthy()),
        BinaryOp::Add => match (left, right) {
            (UiValue::String(left), UiValue::String(right)) => UiValue::String(left + &right),
            (UiValue::Int(left), UiValue::Int(right)) => UiValue::Int(left.saturating_add(right)),
            (UiValue::Float(left), UiValue::Float(right)) => UiValue::Float(left + right),
            (UiValue::Int(left), UiValue::Float(right)) => UiValue::Float(left as f64 + right),
            (UiValue::Float(left), UiValue::Int(right)) => UiValue::Float(left + right as f64),
            (UiValue::Array(mut left), UiValue::Array(right)) => {
                left.extend(right);
                UiValue::Array(left)
            }
            _ => UiValue::None,
        },
        BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => {
            match (left, right) {
                (UiValue::Int(left), UiValue::Int(right)) => {
                    let result = match op {
                        BinaryOp::Sub => left.checked_sub(right),
                        BinaryOp::Mul => left.checked_mul(right),
                        BinaryOp::Div if right != 0 => left.checked_div(right),
                        BinaryOp::Mod if right != 0 => left.checked_rem(right),
                        _ => None,
                    };
                    result.map(UiValue::Int).unwrap_or(UiValue::None)
                }
                (UiValue::Float(left), UiValue::Float(right)) => UiValue::Float(match op {
                    BinaryOp::Sub => left - right,
                    BinaryOp::Mul => left * right,
                    BinaryOp::Div if right != 0.0 => left / right,
                    BinaryOp::Mod if right != 0.0 => left % right,
                    _ => return UiValue::None,
                }),
                _ => UiValue::None,
            }
        }
        BinaryOp::Lt | BinaryOp::Gt | BinaryOp::Lte | BinaryOp::Gte => {
            let ordering = match (&left, &right) {
                (UiValue::Int(left), UiValue::Int(right)) => left.partial_cmp(right),
                (UiValue::Float(left), UiValue::Float(right)) => left.partial_cmp(right),
                (UiValue::String(left), UiValue::String(right)) => Some(left.cmp(right)),
                _ => None,
            };
            let Some(ordering) = ordering else { return UiValue::Bool(false) };
            UiValue::Bool(match op {
                BinaryOp::Lt => ordering.is_lt(),
                BinaryOp::Gt => ordering.is_gt(),
                BinaryOp::Lte => ordering.is_le(),
                BinaryOp::Gte => ordering.is_ge(),
                _ => false,
            })
        }
    }
}

fn ui_values_equal(left: &UiValue, right: &UiValue) -> bool {
    match (left, right) {
        (UiValue::None, UiValue::None) => true,
        (UiValue::String(left), UiValue::String(right)) => left == right,
        (UiValue::Int(left), UiValue::Int(right)) => left == right,
        (UiValue::Float(left), UiValue::Float(right)) => left == right,
        (UiValue::Int(left), UiValue::Float(right)) => *left as f64 == *right,
        (UiValue::Float(left), UiValue::Int(right)) => *left == *right as f64,
        (UiValue::Bool(left), UiValue::Bool(right)) => left == right,
        (UiValue::Array(left), UiValue::Array(right)) => left == right,
        (UiValue::Object(left), UiValue::Object(right)) => left == right,
        _ => false,
    }
}

fn ui_value_to_style(value: UiValue) -> Option<StyleValue> {
    match value {
        UiValue::None => None,
        UiValue::String(value) => Some(StyleValue::String(value)),
        UiValue::Int(value) => Some(StyleValue::Int(value)),
        UiValue::Float(value) => Some(StyleValue::Float(value)),
        UiValue::Bool(value) => Some(StyleValue::Bool(value)),
        UiValue::Array(_) | UiValue::Object(_) => None,
    }
}
fn build_element_in_context(el: &ElementExpr, state: &mut UiState, ctx: &mut BuildCtx) -> Widget {
    let scope = ctx.scope.clone();
    let ws = build_element_style(
        el,
        ctx.themes,
        ctx.theme,
        ctx.module.as_deref(),
        scope.as_deref(),
        state,
    );

    match el.name.name.as_str() {
        "Column" => Widget::Column(
            el.children
                .as_ref()
                .map(|children| build_widgets_in_context(children, state, ctx))
                .unwrap_or_default(),
            ws,
        ),
        "Row" => Widget::Row(
            el.children
                .as_ref()
                .map(|children| build_widgets_in_context(children, state, ctx))
                .unwrap_or_default(),
            ws,
        ),
        "Card" => Widget::Card(
            el.children
                .as_ref()
                .map(|children| build_widgets_in_context(children, state, ctx))
                .unwrap_or_default(),
            apply_surface_default(ws, ctx.theme),
        ),
        "Text" => Widget::Text(
            el.primary_arg
                .as_ref()
                .map(|argument| eval_text_expr(argument, state, scope.as_deref()))
                .unwrap_or_default(),
            apply_text_defaults(ws, ctx.theme),
        ),
        "Display" => {
            let style = apply_text_defaults(ws, ctx.theme);
            let var_name = el
                .primary_arg
                .as_ref()
                .and_then(|argument| match argument {
                    Expr::Identifier(identifier) => Some(state_key(scope.as_deref(), &identifier.name)),
                    _ => None,
                })
                .unwrap_or_default();
            Widget::Display { var_name, style }
        }
        "Divider" => Widget::Divider,
        "Heading" => Widget::Heading(
            el.primary_arg
                .as_ref()
                .and_then(|argument| expr_to_string(argument, state, scope.as_deref()))
                .unwrap_or_default(),
            apply_text_defaults(ws, ctx.theme),
        ),
        "Spacer" => Widget::Spacer,
        "Input" => {
            let mut bind_target = None;
            let mut placeholder = None;
            for modifier in &el.modifiers {
                match modifier {
                    ElementModifier::Binding(binding) => {
                        bind_target = Some(state_key(scope.as_deref(), &binding.target.name));
                    }
                    ElementModifier::Property(property) if property.name.name == "placeholder" => {
                        placeholder = expr_to_string(&property.value, state, scope.as_deref());
                    }
                    _ => {}
                }
            }
            let value = bind_target
                .as_ref()
                .and_then(|target| state.get_value(target))
                .map(|value| value.as_string())
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
                .and_then(|argument| expr_to_string(argument, state, scope.as_deref()))
                .unwrap_or_else(|| "Button".to_string());
            let on_click = el.modifiers.iter().find_map(|modifier| match modifier {
                ElementModifier::Event(event) if event.event.name == "click" => Some(EventAction {
                    handler: event.handler.clone(),
                    span: event.span,
                    scope: scope.clone(),
                }),
                _ => None,
            });
            Widget::Button {
                label,
                on_click,
                style: ws,
            }
        }
        "Messages" => {
            let source = el.modifiers.iter().find_map(|modifier| match modifier {
                ElementModifier::Property(property)
                    if (property.name.name == "list" || property.name.name == "data")
                        && matches!(&property.value, Expr::Identifier(_)) =>
                {
                    match &property.value {
                        Expr::Identifier(identifier) => Some(state_key(scope.as_deref(), &identifier.name)),
                        _ => None,
                    }
                }
                _ => None,
            }).unwrap_or_default();
            Widget::MessagesList { source, style: ws }
        }
        _ => Widget::Container(
            el.children
                .as_ref()
                .map(|children| build_widgets_in_context(children, state, ctx))
                .unwrap_or_default(),
        ),
    }
}

fn build_element_style(
    el: &ElementExpr,
    themes: &Themes,
    theme: &ResolvedTheme,
    module: Option<&str>,
    scope: Option<&str>,
    state: &UiState,
) -> WidgetStyle {
    let mut style = WidgetStyle::new();
    for (name, value) in &el.style.properties {
        if let Some(value) = resolve_style_value(value, themes, theme, module) {
            style.properties.insert(name.clone(), value);
        }
    }
    for modifier in &el.modifiers {
        if let ElementModifier::Property(property) = modifier {
            if let Some(value) = expr_to_style_value(
                &property.value,
                themes,
                theme,
                module,
                scope,
                state,
            ) {
                style.properties.insert(property.name.name.clone(), value);
            }
        }
    }
    style
}

fn resolve_style_value(
    value: &StyleValue,
    themes: &Themes,
    active_theme: &ResolvedTheme,
    module: Option<&str>,
) -> Option<StyleValue> {
    match value {
        StyleValue::Ident(path) if path.starts_with("theme.") => {
            let key = path.strip_prefix("theme.")?;
            if let Some(value) = active_theme.get(key) {
                return Some(value.clone());
            }
            resolve_named_theme_token(key, themes, module)
        }
        other => Some(other.clone()),
    }
}

fn resolve_named_theme_token(
    key: &str,
    themes: &Themes,
    module: Option<&str>,
) -> Option<StyleValue> {
    let mut candidates: Vec<(String, String)> = themes
        .named
        .keys()
        .filter_map(|theme_name| {
            let relative = module.and_then(|module| {
                let private_prefix = format!("{}::", module);
                let public_prefix = format!("{}.", module);
                if let Some(relative) = theme_name.strip_prefix(&private_prefix) {
                    Some(relative.to_string())
                } else {
                    theme_name.strip_prefix(&public_prefix).map(str::to_string)
                }
            });
            let relative = relative.unwrap_or_else(|| theme_name.clone());
            (key == relative || key.starts_with(&format!("{}.", relative)))
                .then(|| (relative, theme_name.clone()))
        })
        .collect();
    candidates.sort_by_key(|(relative, _)| std::cmp::Reverse(relative.len()));
    candidates.into_iter().find_map(|(relative, theme_name)| {
        let token_key = key.strip_prefix(&format!("{}.", relative))?;
        themes
            .get(&theme_name)
            .and_then(|theme| theme.get(token_key).cloned())
    })
}

fn expr_to_style_value(
    expr: &Expr,
    themes: &Themes,
    theme: &ResolvedTheme,
    module: Option<&str>,
    scope: Option<&str>,
    state: &UiState,
) -> Option<StyleValue> {
    match expr {
        Expr::Literal(literal) => match &literal.value {
            aec_ast::Literal::String(value) | aec_ast::Literal::RawString(value) => {
                Some(StyleValue::String(value.clone()))
            }
            aec_ast::Literal::Int(value) => Some(StyleValue::Int(*value)),
            aec_ast::Literal::Float(value) => Some(StyleValue::Float(*value)),
            aec_ast::Literal::Bool(value) => Some(StyleValue::Bool(*value)),
            _ => None,
        },
        Expr::Identifier(_) => ui_value_to_style(expr_to_value(expr, state, scope)),
        Expr::Member(_) => {
            let path = expr_token_path(expr)?;
            resolve_style_value(&StyleValue::Ident(path), themes, theme, module)
                .or_else(|| ui_value_to_style(expr_to_value(expr, state, scope)))
        }
        _ => None,
    }
}

fn expr_token_path(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Identifier(identifier) => Some(identifier.name.clone()),
        Expr::Member(member) => Some(format!(
            "{}.{}",
            expr_token_path(&member.object)?,
            member.property.name
        )),
        _ => None,
    }
}

fn apply_text_defaults(mut style: WidgetStyle, theme: &ResolvedTheme) -> WidgetStyle {
    for (property, token) in [("color", "color.text"), ("size", "text.size"), ("weight", "text.weight")] {
        if !style.properties.contains_key(property) {
            if let Some(value) = theme.get(token) {
                style.properties.insert(property.to_string(), value.clone());
            }
        }
    }
    style
}

fn apply_surface_default(mut style: WidgetStyle, theme: &ResolvedTheme) -> WidgetStyle {
    if !style.properties.contains_key("background") {
        if let Some(value) = theme.get("color.surface") {
            style.properties.insert("background".to_string(), value.clone());
        }
    }
    style
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
                is_public: true,
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
            is_public: true,
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
    fn resolves_qualified_module_theme_token() {
        let theme = theme_decl(
            "Dark",
            false,
            None,
            vec![("color", vec![("primary", string_token("#89b4fa"))])],
        );
        let items = vec![TopLevelItem::Theme(theme)];
        let modules = vec![Some("lib".to_string())];
        let themes = Themes::from_items_with_modules(&items, &modules);
        let components = ComponentRegistry::new();
        let stmts = vec![element_with_style(
            "Text",
            vec![(
                "color",
                StyleValue::Ident("theme.lib.Dark.color.primary".to_string()),
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

    #[test]
    fn rebuilding_updates_text_conditions_and_call_iterables() {
        let source = r#"agent Test
ui Main = Screen "Test" {
    @count: int = 1
    Text "count={count}"
    if count > 0 {
        Text "positive"
    }
    for item in range(3) {
        Text "item={item}"
    }
}
"#;
        let program = aec_parser::parse(source).unwrap();
        let ui = program
            .items
            .iter()
            .find_map(|item| match item {
                TopLevelItem::Ui(ui) => Some(ui.clone()),
                _ => None,
            })
            .unwrap();
        let mut state = UiState::new();
        let first = build_widgets(&ui.screen.body, &mut state);
        let mut first_texts = Vec::new();
        collect_texts(&first, &mut first_texts);
        assert_eq!(first_texts, vec!["count=1", "positive", "item=0", "item=1", "item=2"]);

        state.values.insert("count".to_string(), UiValue::Int(0));
        let second = build_widgets(&ui.screen.body, &mut state);
        let mut second_texts = Vec::new();
        collect_texts(&second, &mut second_texts);
        assert_eq!(second_texts, vec!["count=0", "item=0", "item=1", "item=2"]);
    }
}
