//! Runtime values

use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;
use std::cell::RefCell;

#[derive(Debug, Clone)]
pub enum Value {
    Int(i64),
    Float(f64),
    String(String),
    Bool(bool),
    None,
    Array(Vec<Value>),
    Object(HashMap<String, Value>),
    Function(Rc<Function>),
    /// A lambda value (`x => expr`).
    Closure(Rc<Closure>),
    /// `ok(value)` / `err(value)` — see the `?` operator.
    Result(std::result::Result<Box<Value>, Box<Value>>),
}

impl Value {
    /// Builds `ok(value)`.
    pub fn ok(value: Value) -> Self {
        Value::Result(Ok(Box::new(value)))
    }

    /// Builds `err(value)`.
    pub fn err(value: Value) -> Self {
        Value::Result(Err(Box::new(value)))
    }
}

#[derive(Debug)]
pub struct Function {
    pub name: String,
    pub params: Vec<String>,
    pub param_defaults: Vec<Option<aec_ast::Expr>>,
    pub body: aec_ast::Block,
    pub env: Env,
}

/// An anonymous function from `x => expr`, with its captured environment.
#[derive(Debug)]
pub struct Closure {
    pub params: Vec<String>,
    pub body: aec_ast::Expr,
    pub env: Env,
}

pub type Env = Rc<RefCell<Environment>>;

#[derive(Debug)]
pub struct Environment {
    pub vars: HashMap<String, Value>,
    pub parent: Option<Env>,
}

impl Environment {
    pub fn new() -> Env {
        Rc::new(RefCell::new(Environment {
            vars: HashMap::new(),
            parent: None,
        }))
    }

    pub fn with_parent(parent: Env) -> Env {
        Rc::new(RefCell::new(Environment {
            vars: HashMap::new(),
            parent: Some(parent),
        }))
    }

    pub fn get(&self, name: &str) -> Option<Value> {
        if let Some(v) = self.vars.get(name) {
            return Some(v.clone());
        }
        if let Some(p) = &self.parent {
            return p.borrow().get(name);
        }
        None
    }

    pub fn set(&mut self, name: String, value: Value) {
        self.vars.insert(name, value);
    }

    pub fn assign(&mut self, name: &str, value: Value) -> bool {
        if self.vars.contains_key(name) {
            self.vars.insert(name.to_string(), value);
            return true;
        }
        if let Some(p) = &self.parent {
            return p.borrow_mut().assign(name, value);
        }
        false
    }

    pub fn has_local(&self, name: &str) -> bool {
        self.vars.contains_key(name)
    }
}

impl Value {
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Int(_) => "int",
            Value::Float(_) => "float",
            Value::String(_) => "string",
            Value::Bool(_) => "bool",
            Value::None => "none",
            Value::Array(_) => "array",
            Value::Object(_) => "object",
            Value::Function(_) => "function",
            Value::Closure(_) => "function",
            Value::Result(_) => "result",
        }
    }

    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            Value::None => false,
            Value::Int(n) => *n != 0,
            Value::Float(f) => *f != 0.0,
            Value::String(s) => !s.is_empty(),
            Value::Array(a) => !a.is_empty(),
            _ => true,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Value::Int(n) => write!(f, "{}", n),
            Value::Float(n) => write!(f, "{}", n),
            Value::String(s) => write!(f, "{}", s),
            Value::Bool(b) => write!(f, "{}", b),
            Value::None => write!(f, "none"),
            Value::Array(arr) => {
                write!(f, "[")?;
                for (i, v) in arr.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", v)?;
                }
                write!(f, "]")
            }
            Value::Object(obj) => {
                write!(f, "{{")?;
                for (i, (k, v)) in obj.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}: {}", k, v)?;
                }
                write!(f, "}}")
            }
            Value::Function(func) => write!(f, "<fn {}>", func.name),
            Value::Closure(_) => write!(f, "<lambda>"),
            Value::Result(Ok(v)) => write!(f, "ok({})", v),
            Value::Result(Err(e)) => write!(f, "err({})", e),
        }
    }
}
