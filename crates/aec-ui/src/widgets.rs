//! Widget tree — تبدیل AST به widgets داخلی

use aec_ast::{
    UiStatement, ElementExpr, ElementModifier, StateDeclUi, Expr,
};
use std::collections::HashMap;

/// Widget tree
#[derive(Debug, Clone)]
pub enum Widget {
    /// Column { ... }
    Column(Vec<Widget>),

    /// Row { ... }
    Row(Vec<Widget>),

    /// Text "Hello"
    Text(String),

    /// Input bind value to message
    Input {
        bind_target: Option<String>,
        placeholder: Option<String>,
    },

    /// Button "Send" on click -> send()
    Button {
        label: String,
        on_click: Option<String>,  // نام تابع
    },

    /// Card { ... }
    Card(Vec<Widget>),

    /// Container خالی
    Container(Vec<Widget>),
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
}

impl UiValue {
    pub fn as_string(&self) -> String {
        match self {
            UiValue::String(s) => s.clone(),
            UiValue::Int(n) => n.to_string(),
            UiValue::Float(f) => f.to_string(),
            UiValue::Bool(b) => b.to_string(),
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
            // مقدار اولیه
            let val = expr_to_value(&s.initial);
            state.values.insert(s.name.name.clone(), val);
            None
        }
        UiStatement::Element(el) => Some(build_element(el, state)),
        UiStatement::If(_) => {
            // TODO: پیاده‌سازی if
            None
        }
        UiStatement::For(_) => {
            // TODO: پیاده‌سازی for
            None
        }
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
            let text = el.primary_arg.as_ref()
                .and_then(|e| expr_to_string(e))
                .unwrap_or_default();
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

            Widget::Input { bind_target, placeholder }
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

        _ => {
            // ناشناخته → Container
            let children = el.children.as_ref()
                .map(|c| build_widgets(c, state))
                .unwrap_or_default();
            Widget::Container(children)
        }
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
