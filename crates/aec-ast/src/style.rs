//! Style System — properties for UI elements

use std::collections::HashMap;

/// A style block: { color: "#fff", size: 24 }
#[derive(Debug, Clone, Default)]
pub struct Style {
    pub properties: HashMap<String, StyleValue>,
}

/// The value of one style property
#[derive(Debug, Clone)]
pub enum StyleValue {
    String(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    Ident(String),
}

impl Style {
    pub fn new() -> Self {
        Self { properties: HashMap::new() }
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
            Some(StyleValue::String(s)) => match s.as_str() {
                "true" => Some(true),
                "false" => Some(false),
                _ => None,
            },
            _ => None,
        }
    }
}
