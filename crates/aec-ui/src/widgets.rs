//! Widget tree — تبدیل AST به widgets داخلی

use aec_ast::{
    UiStatement, ElementExpr, ElementModifier, Expr, UiIf, UiFor,
};
use std::collections::HashMap;

/// Widget tree
#[derive(Debug, Clone)]
pub enum Widget {
    Column(Vec<Widget>),
    Row(Vec<Widget>),
    Text(String),
    Input {
        bind_target: Option<String>,
        placeholder: Option<String>,
        value: String,
    },
    Button {
        label: String,
        on_click: Option<String>,
    },
    Card(Vec<Widget>),
    Container(Vec<Widget>),

    /// Messages list = history
    MessagesList {
        source: String,
        items: Vec<MessageItem>,
    },

    /// if cond { ... } else { ... }
    If {
        condition: bool,
        then_branch: Vec<Widget>,
        else_branch: Option<Vec<Widget>>,
    },

    /// for item in items { ... }
    For {
        variable: String,
        items: Vec<Widget>, // فعلاً ساده
    },
}

#[derive(Debug, Clone)]
pub struct MessageItem {
    pub role: String,
    pub content: String,
}

/// State runtime برای UI
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
        Self { values: HashMap::new() }
    }

    pub fn get_string(&self, name: &str) -> String {
        self.values.get(name)
            .map(|v| v.as_string())
            .unwrap_or_default()
    }

    pub fn set_string(&mut self, name: &str, value: String) {
        self.values.insert(name.to_string(), UiValue::String(value));
    }

    pub fn get_bool(&self, name: &str) -> bool {
        self.values.get(name)
            .map(|v| v.as_bool())
            .unwrap_or(false)
    }

    pub fn get_value(&self, name: &str) -> Option<&UiValue> {
        self.values.get(name)
    }
}

/// ساخت widget tree از UI statement ها
pub fn build_widgets(
    statements: &[UiStatement],
    state: &mut UiState,
) -> Vec<Widget> {
    let mut widgets = Vec::new();
    for stmt in statements {
        if let Some(w) = build_widget(stmt, state) {
            widgets.push(w);
        }
    }
    widgets
}

fn build_widget(stmt: &UiStatement, state: &mut UiState) -> Option<Widget> {
    match stmt {
        UiStatement::State(s) => {
            let val = expr_to_value(&s.initial);
            state.values.insert(s.name.name.clone(), val);
            None
        }
        UiStatement::Element(el) => Some(build_element(el, state)),
        UiStatement::If(ui_if) => Some(build_if(ui_if, state)),
        UiStatement::For(ui_for) => Some(build_for(ui_for, state)),
    }
}

fn build_if(ui_if: &UiIf, state: &mut UiState) -> Widget {
    // ارزیابی ساده شرط
    let condition = eval_condition(&ui_if.condition, state);

    if condition {
        Widget::If {
            condition: true,
            then_branch: build_widgets(&ui_if.then_body, state),
            else_branch: None,
        }
    } else if let Some(else_body) = &ui_if.else_body {
        Widget::If {
            condition: false,
            then_branch: vec![],
            else_branch: Some(build_widgets(else_body, state)),
        }
    } else {
        Widget::If {
            condition: false,
            then_branch: vec![],
            else_branch: None,
        }
    }
}

fn build_for(ui_for: &UiFor, state: &mut UiState) -> Widget {
    // ساده: فقط iterable رو ارزیابی می‌کنیم
    let items = if let Expr::Identifier(id) = &ui_for.iterable {
        state.get_value(&id.name)
            .map(|v| v.as_array())
            .unwrap_or_default()
    } else {
        vec![]
    };

    // ساخت widgetها برای هر آیتم
    let mut all_widgets = Vec::new();
    for item in items {
        let mut temp_state = state.clone();
        temp_state.values.insert(
            ui_for.variable.name.clone(),
            item.clone(),
        );
        let widgets = build_widgets(&ui_for.body, &mut temp_state);
        all_widgets.extend(widgets);
    }

    Widget::For {
        variable: ui_for.variable.name.clone(),
        items: all_widgets,
    }
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
                BinaryOp::Eq => {
                    let l = eval_value(&b.left, state);
                    let r = eval_value(&b.right, state);
                    l == r
                }
                BinaryOp::Neq => {
                    let l = eval_value(&b.left, state);
                    let r = eval_value(&b.right, state);
                    l != r
                }
                BinaryOp::And => {
                    eval_condition(&b.left, state) && eval_condition(&b.right, state)
                }
                BinaryOp::Or => {
                    eval_condition(&b.left, state) || eval_condition(&b.right, state)
                }
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
            _ => String::new(),
        },
        Expr::Identifier(id) => state.get_string(&id.name),
        _ => String::new(),
    }
}

fn build_element(el: &ElementExpr, state: &mut UiState) -> Widget {
    match el.name.name.as_str() {
        "Column" => {
            let children = el.children.as_ref()
                .map(|c| build_widgets(c, state))
                .unwrap_or_default();
            Widget::Column(children)
        }

        "Row" => {
            let children = el.children.as_ref()
                .map(|c| build_widgets(c, state))
                .unwrap_or_default();
            Widget::Row(children)
        }

        "Card" => {
            let children = el.children.as_ref()
                .map(|c| build_widgets(c, state))
                .unwrap_or_default();
            Widget::Card(children)
        }

        "Text" => {
            // چک کن آیا state هست
            let text = if let Some(arg) = &el.primary_arg {
                eval_text_expr(arg, state)
            } else {
                String::new()
            };
            Widget::Text(text)
        }

        "Input" => {
            let mut bind_target = None;
            let mut placeholder = None;

            for m in &el.modifiers {
                match m {
                    ElementModifier::Binding(b) => {
                        bind_target = Some(b.target.name.clone());
                    }
                    ElementModifier::Property(p) => {
                        if p.name.name == "placeholder" {
                            placeholder = expr_to_string(&p.value);
                        }
                    }
                    _ => {}
                }
            }

            let value = bind_target.as_ref()
                .map(|t| state.get_string(t))
                .unwrap_or_default();

            Widget::Input { bind_target, placeholder, value }
        }

        "Button" => {
            let label = el.primary_arg.as_ref()
                .and_then(|e| expr_to_string(e))
                .unwrap_or_else(|| "Button".to_string());

            let mut on_click = None;
            for m in &el.modifiers {
                if let ElementModifier::Event(e) = m {
                    if e.event.name == "click" {
                        on_click = extract_fn_name(&e.handler);
                    }
                }
            }

            Widget::Button { label, on_click }
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

            // items رو از state بگیر
            let items = state.get_value(&source)
                .map(|v| v.as_array())
                .unwrap_or_default()
                .into_iter()
                .filter_map(|item| {
                    if let UiValue::Object(o) = item {
                        let role = o.get("role")
                            .map(|v| v.as_string())
                            .unwrap_or_else(|| "user".to_string());
                        let content = o.get("content")
                            .map(|v| v.as_string())
                            .unwrap_or_default();
                        Some(MessageItem { role, content })
                    } else {
                        None
                    }
                })
                .collect();

            Widget::MessagesList { source, items }
        }

        _ => {
            let children = el.children.as_ref()
                .map(|c| build_widgets(c, state))
                .unwrap_or_default();
            Widget::Container(children)
        }
    }
}

fn eval_text_expr(expr: &Expr, state: &UiState) -> String {
    match expr {
        Expr::Literal(lit) => match &lit.value {
            aec_ast::Literal::String(s) => s.clone(),
            aec_ast::Literal::Int(n) => n.to_string(),
            _ => String::new(),
        },
        Expr::Identifier(id) => state.get_string(&id.name),
        Expr::Binary(b) => {
            use aec_ast::BinaryOp;
            if matches!(b.op, BinaryOp::Add) {
                format!("{}{}", eval_text_expr(&b.left, state), eval_text_expr(&b.right, state))
            } else {
                String::new()
            }
        }
        _ => String::new(),
    }
}

fn expr_to_string(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Literal(lit) => match &lit.value {
            aec_ast::Literal::String(s) => Some(s.clone()),
            aec_ast::Literal::Int(n) => Some(n.to_string()),
            aec_ast::Literal::Float(f) => Some(f.to_string()),
            aec_ast::Literal::Bool(b) => Some(b.to_string()),
            _ => None,
        },
        _ => None,
    }
}

fn expr_to_value(expr: &Expr) -> UiValue {
    match expr {
        Expr::Literal(lit) => match &lit.value {
            aec_ast::Literal::String(s) => UiValue::String(s.clone()),
            aec_ast::Literal::Int(n) => UiValue::Int(*n),
            aec_ast::Literal::Float(f) => UiValue::Float(*f),
            aec_ast::Literal::Bool(b) => UiValue::Bool(*b),
            _ => UiValue::String(String::new()),
        },
        Expr::Array(arr) => {
            let items = arr.elements.iter()
                .map(expr_to_value)
                .collect();
            UiValue::Array(items)
        }
        Expr::Object(obj) => {
            let mut map = HashMap::new();
            for field in &obj.fields {
                map.insert(field.key.name.clone(), expr_to_value(&field.value));
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
