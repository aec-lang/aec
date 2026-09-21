//! Interpreter — executing the AST

use crate::errors::RuntimeError;
use crate::llm;
use crate::memory::Memory;
use crate::permissions::{Limits, Permissions};
use crate::value::{Closure, Env, Environment, Function, Value};
use aec_ast::{
    AssignOp, BinaryOp, Block, ElseBranch, Expr, ForStmt, FunctionDecl,
    IfStmt, LValue, LValueStep, MatchBody, Pattern, Program, Statement, UnaryOp,
    WhileStmt,
};
use std::collections::HashMap;
use std::rc::Rc;

pub struct Interpreter {
    pub global: Env,
    pub memory: Memory,
    /// Permission policy. `None` = the program has no `permissions` block → no gate is applied.
    pub permissions: Option<Permissions>,
    pub limits: Limits,
}

impl Interpreter {
    pub fn new() -> Self {
        let global = Environment::new();
        Self {
            global,
            memory: Memory::new(),
            permissions: None,
            limits: Limits::default(),
        }
    }

    pub fn run(&mut self, program: &Program) -> Result<(), RuntimeError> {
        for item in &program.items {
            match item {
                aec_ast::TopLevelItem::Function(f) => self.register_function(f),
                aec_ast::TopLevelItem::Permissions(block) => {
                    self.permissions = Some(Permissions::from_block(block));
                }
                aec_ast::TopLevelItem::Limits(block) => {
                    self.limits = Limits::from_block(block);
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn register_function(&mut self, f: &FunctionDecl) {
        let params = f.params.iter().map(|p| p.name.name.clone()).collect();
        let func = Function {
            name: f.name.name.clone(),
            params,
            body: f.body.clone(),
            env: self.global.clone(),
        };
        self.global
            .borrow_mut()
            .set(f.name.name.clone(), Value::Function(Rc::new(func)));
    }

    pub fn call_function(
        &mut self,
        name: &str,
        args: Vec<Value>,
        span: aec_ast::Span,
    ) -> Result<Value, RuntimeError> {
        if let Some(result) = self.try_builtin(name, &args, span)? {
            return Ok(result);
        }

        let func_val = self
            .global
            .borrow()
            .get(name)
            .ok_or_else(|| RuntimeError::UndefinedFunction {
                name: name.to_string(),
                span,
            })?;

        let func = match func_val {
            Value::Function(f) => f,
            _ => {
                return Err(RuntimeError::TypeError {
                    message: format!("'{}' is not a function", name),
                    span,
                })
            }
        };

        if args.len() != func.params.len() {
            return Err(RuntimeError::WrongArgCount {
                expected: func.params.len(),
                got: args.len(),
                span,
            });
        }

        let local_env = Environment::with_parent(func.env.clone());
        for (param, arg) in func.params.iter().zip(args) {
            local_env.borrow_mut().set(param.clone(), arg);
        }

        match self.exec_block(&func.body, local_env) {
            Ok(Flow::Normal(v)) | Ok(Flow::Return(v)) => Ok(v),
            // `?` met an `err(...)`: the function returns that error (wrapped back
            // into a result) to its caller.
            Err(RuntimeError::EarlyReturn { value }) => Ok(Value::Result(Err(value))),
            Err(e) => Err(e),
        }
    }

    fn try_builtin(
        &mut self,
        name: &str,
        args: &[Value],
        span: aec_ast::Span,
    ) -> Result<Option<Value>, RuntimeError> {
        // Permission gate: if the program declared a `permissions` block, every builtin
        // is checked against the allowlist before it runs. Unknown names (user
        // functions) pass through untouched.
        if let Some(perms) = &self.permissions {
            perms
                .check_builtin(name, args)
                .map_err(|message| crate::permissions::denied(message, span))?;
        }

        // check stdlib first
        if let Some(v) = crate::stdlib::call_builtin(name, args, span, &self.limits)? {
            return Ok(Some(v));
        }
        if let Some(v) = crate::stdlib::call_builtin2(name, args, span)? {
            return Ok(Some(v));
        }
        if let Some(v) = crate::stdlib_extended::call_extended(name, args, span, &self.limits)? {
            return Ok(Some(v));
        }
        // then the internal built-ins
        let result = match name {
            // Result constructors and predicates (see the `?` operator).
            "ok" => {
                require_args(args, 1, span)?;
                Value::ok(args[0].clone())
            }
            "err" => {
                require_args(args, 1, span)?;
                Value::err(args[0].clone())
            }
            "is_ok" => {
                require_args(args, 1, span)?;
                Value::Bool(matches!(args[0], Value::Result(Ok(_))))
            }
            "is_err" => {
                require_args(args, 1, span)?;
                Value::Bool(matches!(args[0], Value::Result(Err(_))))
            }
            "print" => {
                let s = args.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(" ");
                println!("{}", s);
                Value::None
            }
            "read_line" => {
                use std::io::Write;
                print!("> ");
                std::io::stdout().flush().ok();
                let mut input = String::new();
                std::io::stdin().read_line(&mut input).map_err(|e| RuntimeError::Generic {
                    message: format!("failed to read input: {}", e),
                    span,
                })?;
                Value::String(input.trim_end().to_string())
            }
            "len" => {
                if args.len() != 1 {
                    return Err(RuntimeError::WrongArgCount { expected: 1, got: args.len(), span });
                }
                match &args[0] {
                    Value::String(s) => Value::Int(s.len() as i64),
                    Value::Array(a) => Value::Int(a.len() as i64),
                    Value::Object(o) => Value::Int(o.len() as i64),
                    v => return Err(RuntimeError::TypeError {
                        message: format!("len() doesn't work on {}", v.type_name()),
                        span,
                    }),
                }
            }
            "str" => {
                if args.len() != 1 {
                    return Err(RuntimeError::WrongArgCount { expected: 1, got: args.len(), span });
                }
                Value::String(args[0].to_string())
            }
            "int" => {
                if args.len() != 1 {
                    return Err(RuntimeError::WrongArgCount { expected: 1, got: args.len(), span });
                }
                match &args[0] {
                    Value::Int(n) => Value::Int(*n),
                    Value::Float(f) => Value::Int(*f as i64),
                    Value::String(s) => {
                        let n = s.parse::<i64>().map_err(|_| RuntimeError::TypeError {
                            message: format!("can't convert '{}' to int", s),
                            span,
                        })?;
                        Value::Int(n)
                    }
                    v => return Err(RuntimeError::TypeError {
                        message: format!("can't convert {} to int", v.type_name()),
                        span,
                    }),
                }
            }
            "float" => {
                if args.len() != 1 {
                    return Err(RuntimeError::WrongArgCount { expected: 1, got: args.len(), span });
                }
                match &args[0] {
                    Value::Int(n) => Value::Float(*n as f64),
                    Value::Float(f) => Value::Float(*f),
                    Value::String(s) => {
                        let f = s.parse::<f64>().map_err(|_| RuntimeError::TypeError {
                            message: format!("can't convert '{}' to float", s),
                            span,
                        })?;
                        Value::Float(f)
                    }
                    v => return Err(RuntimeError::TypeError {
                        message: format!("can't convert {} to float", v.type_name()),
                        span,
                    }),
                }
            }
            "upper" => match &args[0] {
                Value::String(s) => Value::String(s.to_uppercase()),
                v => return Err(RuntimeError::TypeError {
                    message: format!("upper() needs string, got {}", v.type_name()),
                    span,
                }),
            },
            "lower" => match &args[0] {
                Value::String(s) => Value::String(s.to_lowercase()),
                v => return Err(RuntimeError::TypeError {
                    message: format!("lower() needs string, got {}", v.type_name()),
                    span,
                }),
            },
            "trim" => match &args[0] {
                Value::String(s) => Value::String(s.trim().to_string()),
                v => return Err(RuntimeError::TypeError {
                    message: format!("trim() needs string, got {}", v.type_name()),
                    span,
                }),
            },
            "split" => match (&args[0], &args[1]) {
                (Value::String(s), Value::String(sep)) => {
                    let parts: Vec<Value> = s.split(sep.as_str())
                        .map(|p| Value::String(p.to_string()))
                        .collect();
                    Value::Array(parts)
                }
                _ => return Err(RuntimeError::TypeError {
                    message: "split() needs two strings".to_string(),
                    span,
                }),
            },
            "join" => match (&args[0], &args[1]) {
                (Value::Array(arr), Value::String(sep)) => {
                    let parts: Vec<String> = arr.iter().map(|v| v.to_string()).collect();
                    Value::String(parts.join(sep))
                }
                _ => return Err(RuntimeError::TypeError {
                    message: "join() needs array and string".to_string(),
                    span,
                }),
            },
            "contains" => match (&args[0], &args[1]) {
                (Value::String(s), Value::String(sub)) => Value::Bool(s.contains(sub.as_str())),
                _ => return Err(RuntimeError::TypeError {
                    message: "contains() needs two strings".to_string(),
                    span,
                }),
            },
            "replace" => match (&args[0], &args[1], &args[2]) {
                (Value::String(s), Value::String(from), Value::String(to)) => {
                    Value::String(s.replace(from.as_str(), to.as_str()))
                }
                _ => return Err(RuntimeError::TypeError {
                    message: "replace() needs three strings".to_string(),
                    span,
                }),
            },
            "push" => match &args[0] {
                Value::Array(arr) => {
                    let mut new_arr = arr.clone();
                    new_arr.push(args[1].clone());
                    Value::Array(new_arr)
                }
                v => return Err(RuntimeError::TypeError {
                    message: format!("push() needs array, got {}", v.type_name()),
                    span,
                }),
            },
            "pop" => match &args[0] {
                Value::Array(arr) => {
                    let mut new_arr = arr.clone();
                    new_arr.pop().unwrap_or(Value::None)
                }
                v => return Err(RuntimeError::TypeError {
                    message: format!("pop() needs array, got {}", v.type_name()),
                    span,
                }),
            },
            "range" => match args.len() {
                1 => match &args[0] {
                    Value::Int(n) => {
                        let items: Vec<Value> = (0..*n).map(Value::Int).collect();
                        Value::Array(items)
                    }
                    v => return Err(RuntimeError::TypeError {
                        message: format!("range() needs int, got {}", v.type_name()),
                        span,
                    }),
                },
                2 => match (&args[0], &args[1]) {
                    (Value::Int(start), Value::Int(end)) => {
                        let items: Vec<Value> = (*start..*end).map(Value::Int).collect();
                        Value::Array(items)
                    }
                    _ => return Err(RuntimeError::TypeError {
                        message: "range() needs two ints".to_string(),
                        span,
                    }),
                },
                _ => return Err(RuntimeError::WrongArgCount {
                    expected: 1,
                    got: args.len(),
                    span,
                }),
            },
            "abs" => match &args[0] {
                Value::Int(n) => Value::Int(n.abs()),
                Value::Float(f) => Value::Float(f.abs()),
                v => return Err(RuntimeError::TypeError {
                    message: format!("abs() needs number, got {}", v.type_name()),
                    span,
                }),
            },
            "min" => match (&args[0], &args[1]) {
                (Value::Int(a), Value::Int(b)) => Value::Int(*a.min(b)),
                (Value::Float(a), Value::Float(b)) => Value::Float(a.min(*b)),
                _ => return Err(RuntimeError::TypeError {
                    message: "min() needs two numbers of same type".to_string(),
                    span,
                }),
            },
            "max" => match (&args[0], &args[1]) {
                (Value::Int(a), Value::Int(b)) => Value::Int(*a.max(b)),
                (Value::Float(a), Value::Float(b)) => Value::Float(a.max(*b)),
                _ => return Err(RuntimeError::TypeError {
                    message: "max() needs two numbers of same type".to_string(),
                    span,
                }),
            },
            "sqrt" => match &args[0] {
                Value::Int(n) => Value::Float((*n as f64).sqrt()),
                Value::Float(f) => Value::Float(f.sqrt()),
                v => return Err(RuntimeError::TypeError {
                    message: format!("sqrt() needs number, got {}", v.type_name()),
                    span,
                }),
            },
            "pow" => match (&args[0], &args[1]) {
                (Value::Int(a), Value::Int(b)) => Value::Int(a.pow(*b as u32)),
                (Value::Float(a), Value::Float(b)) => Value::Float(a.powf(*b)),
                _ => return Err(RuntimeError::TypeError {
                    message: "pow() needs two numbers".to_string(),
                    span,
                }),
            },
            "keys" => match &args[0] {
                Value::Object(o) => {
                    let keys: Vec<Value> = o.keys().map(|k| Value::String(k.clone())).collect();
                    Value::Array(keys)
                }
                v => return Err(RuntimeError::TypeError {
                    message: format!("keys() needs object, got {}", v.type_name()),
                    span,
                }),
            },
            "values" => match &args[0] {
                Value::Object(o) => {
                    let values: Vec<Value> = o.values().cloned().collect();
                    Value::Array(values)
                }
                v => return Err(RuntimeError::TypeError {
                    message: format!("values() needs object, got {}", v.type_name()),
                    span,
                }),
            },
            "has" => match (&args[0], &args[1]) {
                (Value::Object(o), Value::String(k)) => Value::Bool(o.contains_key(k)),
                _ => return Err(RuntimeError::TypeError {
                    message: "has() needs object and string".to_string(),
                    span,
                }),
            },
            "llm.complete" | "llm_complete" => {
                return llm::llm_complete(args, span).map(Some);
            }
            "push_to" => {
                if args.len() != 2 {
                    return Err(RuntimeError::WrongArgCount {
                        expected: 2, got: args.len(), span,
                    });
                }
                let name = match &args[0] {
                    Value::String(s) => s.clone(),
                    _ => return Err(RuntimeError::TypeError {
                        message: "push_to: first arg must be variable name".to_string(),
                        span,
                    }),
                };
                let item = args[1].clone();

                let mut env = self.global.borrow_mut();
                let existing = env.get(&name);
                match existing {
                    Some(Value::Array(arr)) => {
                        let mut new_arr = arr.clone();
                        new_arr.push(item);
                        env.set(name.clone(), Value::Array(new_arr));
                        Value::None
                    }
                    None => {
                        env.set(name.clone(), Value::Array(vec![item]));
                        Value::None
                    }
                    _ => {
                        return Err(RuntimeError::TypeError {
                            message: format!("push_to: '{}' is not an array", name),
                            span,
                        });
                    }
                }
            }
            "memory.open" | "memory_open" => {
                if args.len() != 1 {
                    return Err(RuntimeError::WrongArgCount { expected: 1, got: args.len(), span });
                }
                let path = match &args[0] {
                    Value::String(s) => s.clone(),
                    v => return Err(RuntimeError::TypeError {
                        message: format!("memory.open path needs string, got {}", v.type_name()),
                        span,
                    }),
                };
                self.memory = Memory::open(&path).map_err(|e| e.with_span(span))?;
                Value::None
            }
            "memory.add" | "memory_add" => {
                if args.len() < 3 {
                    return Err(RuntimeError::WrongArgCount {
                        expected: 3,
                        got: args.len(),
                        span,
                    });
                }
                let conv_id = match &args[0] {
                    Value::String(s) => s.clone(),
                    v => return Err(RuntimeError::TypeError {
                        message: format!("memory.add conv_id needs string, got {}", v.type_name()),
                        span,
                    }),
                };
                let role = match &args[1] {
                    Value::String(s) => s.clone(),
                    v => return Err(RuntimeError::TypeError {
                        message: format!("memory.add role needs string, got {}", v.type_name()),
                        span,
                    }),
                };
                let content = match &args[2] {
                    Value::String(s) => s.clone(),
                    v => return Err(RuntimeError::TypeError {
                        message: format!("memory.add content needs string, got {}", v.type_name()),
                        span,
                    }),
                };
                self.memory
                    .add(&conv_id, &role, &content)
                    .map_err(|e| e.with_span(span))?;
                Value::None
            }
            "memory.get" | "memory_get" => {
                if args.len() != 1 {
                    return Err(RuntimeError::WrongArgCount { expected: 1, got: args.len(), span });
                }
                let conv_id = match &args[0] {
                    Value::String(s) => s.clone(),
                    v => return Err(RuntimeError::TypeError {
                        message: format!("memory.get conv_id needs string, got {}", v.type_name()),
                        span,
                    }),
                };
                self.memory.to_value(&conv_id).map_err(|e| e.with_span(span))?
            }
            "memory.clear" | "memory_clear" => {
                if args.len() != 1 {
                    return Err(RuntimeError::WrongArgCount { expected: 1, got: args.len(), span });
                }
                let conv_id = match &args[0] {
                    Value::String(s) => s.clone(),
                    v => return Err(RuntimeError::TypeError {
                        message: format!("memory.clear conv_id needs string, got {}", v.type_name()),
                        span,
                    }),
                };
                self.memory
                    .clear(&conv_id)
                    .map_err(|e| e.with_span(span))?;
                Value::None
            }
            "memory.count" | "memory_count" => {
                if args.len() != 1 {
                    return Err(RuntimeError::WrongArgCount { expected: 1, got: args.len(), span });
                }
                let conv_id = match &args[0] {
                    Value::String(s) => s.clone(),
                    v => return Err(RuntimeError::TypeError {
                        message: format!("memory.count conv_id needs string, got {}", v.type_name()),
                        span,
                    }),
                };
                Value::Int(self.memory.len(&conv_id).map_err(|e| e.with_span(span))? as i64)
            }
            _ => return Ok(None),
        };
        Ok(Some(result))
    }

    /// Calls a lambda value with the given arguments.
    fn call_closure(
        &mut self,
        closure: Rc<Closure>,
        args: Vec<Value>,
        span: aec_ast::Span,
    ) -> Result<Value, RuntimeError> {
        if args.len() != closure.params.len() {
            return Err(RuntimeError::WrongArgCount {
                expected: closure.params.len(),
                got: args.len(),
                span,
            });
        }

        let local_env = Environment::with_parent(closure.env.clone());
        for (param, arg) in closure.params.iter().zip(args) {
            local_env.borrow_mut().set(param.clone(), arg);
        }

        // A lambda body is one expression, so there is no `Flow` to unwind.
        // A `?` inside it returns the error *from the lambda*.
        match self.eval_expr(&closure.body, local_env) {
            Ok(value) => Ok(value),
            Err(RuntimeError::EarlyReturn { value }) => Ok(Value::Result(Err(value))),
            Err(e) => Err(e),
        }
    }

    /// Snapshot of the visible local bindings — a lambda's by-move capture.
    ///
    /// Globals are deliberately left out and reached through the parent link, so
    /// a lambda can still call top-level functions and recursion keeps working.
    fn capture_env(&self, env: &Env) -> Env {
        let captured = Environment::new();
        let mut bindings: Vec<(String, Value)> = Vec::new();
        let mut current = Some(env.clone());

        while let Some(scope) = current {
            if Rc::ptr_eq(&scope, &self.global) {
                break;
            }
            let borrowed = scope.borrow();
            for (name, value) in &borrowed.vars {
                // The innermost binding of a name wins.
                if !bindings.iter().any(|(existing, _)| existing == name) {
                    bindings.push((name.clone(), value.clone()));
                }
            }
            current = borrowed.parent.clone();
        }

        {
            let mut scope = captured.borrow_mut();
            scope.vars.extend(bindings);
            scope.parent = Some(self.global.clone());
        }
        captured
    }

    fn exec_block(&mut self, block: &Block, env: Env) -> Result<Flow, RuntimeError> {
        let mut last = Value::None;
        for stmt in &block.statements {
            match self.exec_statement(stmt, env.clone())? {
                Flow::Normal(v) => last = v,
                Flow::Return(v) => return Ok(Flow::Return(v)),
            }
        }
        Ok(Flow::Normal(last))
    }

    fn exec_statement(&mut self, stmt: &Statement, env: Env) -> Result<Flow, RuntimeError> {
        match stmt {
            Statement::Let(let_stmt) => {
                let value = self.eval_expr(&let_stmt.value, env.clone())?;
                env.borrow_mut().set(let_stmt.name.name.clone(), value.clone());
                Ok(Flow::Normal(value))
            }
            Statement::Assign(assign) => {
                let value = self.eval_expr(&assign.value, env.clone())?;
                let new_value = match assign.op {
                    AssignOp::Assign => value,
                    AssignOp::AddAssign => {
                        let current = self.eval_lvalue(&assign.target, env.clone())?;
                        self.binary_op(BinaryOp::Add, current, value, assign.span)?
                    }
                    AssignOp::SubAssign => {
                        let current = self.eval_lvalue(&assign.target, env.clone())?;
                        self.binary_op(BinaryOp::Sub, current, value, assign.span)?
                    }
                    AssignOp::MulAssign => {
                        let current = self.eval_lvalue(&assign.target, env.clone())?;
                        self.binary_op(BinaryOp::Mul, current, value, assign.span)?
                    }
                    AssignOp::DivAssign => {
                        let current = self.eval_lvalue(&assign.target, env.clone())?;
                        self.binary_op(BinaryOp::Div, current, value, assign.span)?
                    }
                };
                self.assign_lvalue(&assign.target, new_value.clone(), env.clone())?;
                Ok(Flow::Normal(new_value))
            }
            Statement::Return(ret) => {
                let value = match &ret.value {
                    Some(e) => self.eval_expr(e, env)?,
                    None => Value::None,
                };
                Ok(Flow::Return(value))
            }
            Statement::Expr(e) => {
                let v = self.eval_expr(e, env)?;
                Ok(Flow::Normal(v))
            }
            Statement::If(if_stmt) => self.exec_if(if_stmt, env),
            Statement::While(w) => self.exec_while(w, env),
            Statement::For(f) => self.exec_for(f, env),
        }
    }

    fn exec_if(&mut self, if_stmt: &IfStmt, env: Env) -> Result<Flow, RuntimeError> {
        let cond = self.eval_expr(&if_stmt.condition, env.clone())?;
        if cond.is_truthy() {
            let block_env = Environment::with_parent(env);
            return self.exec_block(&if_stmt.then_block, block_env);
        }
        match &if_stmt.else_branch {
            Some(ElseBranch::ElseIf(next_if)) => self.exec_if(next_if, env),
            Some(ElseBranch::Else(block)) => {
                let block_env = Environment::with_parent(env);
                self.exec_block(block, block_env)
            }
            None => Ok(Flow::Normal(Value::None)),
        }
    }

    fn exec_while(&mut self, w: &WhileStmt, env: Env) -> Result<Flow, RuntimeError> {
        loop {
            let cond = self.eval_expr(&w.condition, env.clone())?;
            if !cond.is_truthy() {
                break;
            }
            let block_env = Environment::with_parent(env.clone());
            match self.exec_block(&w.body, block_env)? {
                Flow::Return(v) => return Ok(Flow::Return(v)),
                Flow::Normal(_) => {}
            }
        }
        Ok(Flow::Normal(Value::None))
    }

    fn exec_for(&mut self, f: &ForStmt, env: Env) -> Result<Flow, RuntimeError> {
        let iter = self.eval_expr(&f.iterable, env.clone())?;
        let items = match iter {
            Value::Array(a) => a,
            v => {
                return Err(RuntimeError::TypeError {
                    message: format!("for loop needs array, got {}", v.type_name()),
                    span: f.span,
                })
            }
        };

        for item in items {
            let loop_env = Environment::with_parent(env.clone());
            loop_env.borrow_mut().set(f.variable.name.clone(), item);
            match self.exec_block(&f.body, loop_env)? {
                Flow::Return(v) => return Ok(Flow::Return(v)),
                Flow::Normal(_) => {}
            }
        }
        Ok(Flow::Normal(Value::None))
    }

    fn eval_lvalue(&mut self, lv: &LValue, env: Env) -> Result<Value, RuntimeError> {
        let base = env
            .borrow()
            .get(&lv.base.name)
            .ok_or_else(|| RuntimeError::UndefinedVariable {
                name: lv.base.name.clone(),
                span: lv.span,
            })?;
        self.navigate_lvalue(base, &lv.path, env.clone(), lv.span)
    }

    fn assign_lvalue(
        &mut self,
        lv: &LValue,
        value: Value,
        env: Env,
    ) -> Result<(), RuntimeError> {
        if lv.path.is_empty() {
            let ok = env.borrow_mut().assign(&lv.base.name, value);
            if !ok {
                return Err(RuntimeError::UndefinedVariable {
                    name: lv.base.name.clone(),
                    span: lv.span,
                });
            }
            return Ok(());
        }
        Err(RuntimeError::Generic {
            message: "nested assignment not yet fully implemented".to_string(),
            span: lv.span,
        })
    }

    fn navigate_lvalue(
        &mut self,
        value: Value,
        path: &[LValueStep],
        env: Env,
        span: aec_ast::Span,
    ) -> Result<Value, RuntimeError> {
        let mut current = value;
        for step in path {
            current = match step {
                LValueStep::Member(id) => match current {
                    Value::Object(o) => o.get(&id.name).cloned().ok_or_else(|| {
                        RuntimeError::Generic {
                            message: format!("no field '{}'", id.name),
                            span,
                        }
                    })?,
                    v => {
                        return Err(RuntimeError::TypeError {
                            message: format!("can't access .{} on {}", id.name, v.type_name()),
                            span,
                        })
                    }
                },
                LValueStep::Index(expr) => {
                    let idx = self.eval_expr(expr, env.clone())?;
                    match (current, idx) {
                        (Value::Array(arr), Value::Int(i)) => {
                            let i = if i < 0 { arr.len() as i64 + i } else { i };
                            if i < 0 || i as usize >= arr.len() {
                                return Err(RuntimeError::Generic {
                                    message: format!("index out of bounds: {}", i),
                                    span,
                                });
                            }
                            arr[i as usize].clone()
                        }
                        (Value::Object(o), Value::String(k)) => o.get(&k).cloned().ok_or_else(
                            || RuntimeError::Generic {
                                message: format!("no key '{}'", k),
                                span,
                            },
                        )?,
                        (v, _) => {
                            return Err(RuntimeError::TypeError {
                                message: format!("can't index {}", v.type_name()),
                                span,
                            })
                        }
                    }
                }
            };
        }
        Ok(current)
    }

    pub fn eval_expr(&mut self, expr: &Expr, env: Env) -> Result<Value, RuntimeError> {
        match expr {
            Expr::Literal(lit) => self.eval_literal(&lit.value, lit.span, env.clone()),
            Expr::Identifier(id) => env
                .borrow()
                .get(&id.name)
                .ok_or_else(|| RuntimeError::UndefinedVariable {
                    name: id.name.clone(),
                    span: id.span,
                }),
            Expr::Paren(p) => self.eval_expr(&p.inner, env),
            Expr::Array(arr) => {
                let mut values = Vec::new();
                for e in &arr.elements {
                    values.push(self.eval_expr(e, env.clone())?);
                }
                Ok(Value::Array(values))
            }
            Expr::Object(obj) => {
                let mut map = HashMap::new();
                for field in &obj.fields {
                    let val = self.eval_expr(&field.value, env.clone())?;
                    map.insert(field.key.name.clone(), val);
                }
                Ok(Value::Object(map))
            }
            Expr::Binary(b) => {
                let left = self.eval_expr(&b.left, env.clone())?;
                let right = self.eval_expr(&b.right, env.clone())?;
                self.binary_op(b.op, left, right, b.span)
            }
            Expr::Unary(u) => {
                let operand = self.eval_expr(&u.operand, env)?;
                match u.op {
                    UnaryOp::Neg => match operand {
                        Value::Int(n) => Ok(Value::Int(-n)),
                        Value::Float(f) => Ok(Value::Float(-f)),
                        v => Err(RuntimeError::TypeError {
                            message: format!("can't negate {}", v.type_name()),
                            span: u.span,
                        }),
                    },
                    UnaryOp::Not => Ok(Value::Bool(!operand.is_truthy())),
                }
            }
            Expr::Call(call) => {
                let mut args = Vec::new();
                for arg in &call.args {
                    args.push(self.eval_expr(&arg.value, env.clone())?);
                }

                // A name bound to a lambda is called through its value. Everything
                // else (builtins, global `fn`s, namespaces) resolves by name.
                if let Expr::Identifier(id) = &call.callee {
                    let callee = env.borrow().get(&id.name);
                    if let Some(Value::Closure(closure)) = callee {
                        return self.call_closure(closure, args, call.span);
                    }
                }

                let callee_name = match &call.callee {
                    Expr::Identifier(id) => id.name.clone(),
                    Expr::Member(m) => match &m.object {
                        Expr::Identifier(id) => format!("{}.{}", id.name, m.property.name),
                        _ => {
                            return Err(RuntimeError::Generic {
                                message: "complex method calls not yet supported".to_string(),
                                span: call.span,
                            })
                        }
                    },
                    _ => {
                        return Err(RuntimeError::Generic {
                            message: "only simple function calls supported".to_string(),
                            span: call.span,
                        })
                    }
                };
                self.call_function(&callee_name, args, call.span)
            }
            Expr::Member(m) => {
                let obj = self.eval_expr(&m.object, env)?;
                match obj {
                    Value::Object(o) => o.get(&m.property.name).cloned().ok_or_else(|| {
                        RuntimeError::Generic {
                            message: format!("no field '{}'", m.property.name),
                            span: m.span,
                        }
                    }),
                    v => Err(RuntimeError::TypeError {
                        message: format!("can't access .{} on {}", m.property.name, v.type_name()),
                        span: m.span,
                    }),
                }
            }
            Expr::Index(idx) => {
                let obj = self.eval_expr(&idx.object, env.clone())?;
                let index = self.eval_expr(&idx.index, env.clone())?;
                match (obj, index) {
                    (Value::Array(arr), Value::Int(i)) => {
                        let i = if i < 0 { arr.len() as i64 + i } else { i };
                        if i < 0 || i as usize >= arr.len() {
                            Err(RuntimeError::Generic {
                                message: format!("index out of bounds: {}", i),
                                span: idx.span,
                            })
                        } else {
                            Ok(arr[i as usize].clone())
                        }
                    }
                    (Value::String(s), Value::Int(i)) => {
                        let chars: Vec<char> = s.chars().collect();
                        let i = if i < 0 { chars.len() as i64 + i } else { i };
                        if i < 0 || i as usize >= chars.len() {
                            Err(RuntimeError::Generic {
                                message: format!("index out of bounds: {}", i),
                                span: idx.span,
                            })
                        } else {
                            Ok(Value::String(chars[i as usize].to_string()))
                        }
                    }
                    (Value::Object(o), Value::String(k)) => o.get(&k).cloned().ok_or_else(|| {
                        RuntimeError::Generic {
                            message: format!("no key '{}'", k),
                            span: idx.span,
                        }
                    }),
                    (o, _) => Err(RuntimeError::TypeError {
                        message: format!("can't index {}", o.type_name()),
                        span: idx.span,
                    }),
                }
            }
            Expr::Await(a) => self.eval_expr(&a.inner, env),
            Expr::Lambda(l) => Ok(Value::Closure(Rc::new(Closure {
                params: l.params.iter().map(|p| p.name.clone()).collect(),
                body: l.body.clone(),
                env: self.capture_env(&env),
            }))),
            Expr::Try(t) => match self.eval_expr(&t.inner, env)? {
                // `ok(v)?` evaluates to `v`
                Value::Result(Ok(value)) => Ok(*value),
                // `err(e)?` unwinds the enclosing function with `err(e)`
                Value::Result(Err(error)) => Err(RuntimeError::EarlyReturn { value: error }),
                other => Err(RuntimeError::TypeError {
                    message: format!(
                        "can't use '?' on {} — expected a result from ok(...)/err(...)",
                        other.type_name()
                    ),
                    span: t.span,
                }),
            },
            Expr::Match(m) => self.eval_match(m, env),
        }
    }

    fn eval_match(
        &mut self,
        m: &aec_ast::MatchExpr,
        env: Env,
    ) -> Result<Value, RuntimeError> {
        let scrutinee = self.eval_expr(&m.scrutinee, env.clone())?;

        for arm in &m.arms {
            if self.pattern_matches(&arm.pattern, &scrutinee)? {
                match &arm.body {
                    MatchBody::Expr(e) => return self.eval_expr(e, env),
                    MatchBody::Block(b) => {
                        let block_env = Environment::with_parent(env);
                        return match self.exec_block(b, block_env)? {
                            Flow::Normal(v) => Ok(v),
                            Flow::Return(v) => Ok(v),
                        };
                    }
                }
            }
        }
        Ok(Value::None)
    }

    fn pattern_matches(
        &mut self,
        pattern: &Pattern,
        value: &Value,
    ) -> Result<bool, RuntimeError> {
        match pattern {
            Pattern::Wildcard(_) => Ok(true),
            Pattern::Literal(lit) => {
                let pat_val = match lit {
                    aec_ast::Literal::Int(n) => Value::Int(*n),
                    aec_ast::Literal::Float(f) => Value::Float(*f),
                    aec_ast::Literal::String(s) => Value::String(s.clone()),
                    aec_ast::Literal::Bool(b) => Value::Bool(*b),
                    aec_ast::Literal::None => Value::None,
                    _ => return Ok(false),
                };
                Ok(values_equal(&pat_val, value))
            }
            Pattern::None(_) => Ok(matches!(value, Value::None)),
            Pattern::Some(_) => Ok(!matches!(value, Value::None)),
            Pattern::Identifier(_) => Ok(true),
        }
    }

    fn eval_literal(
        &mut self,
        lit: &aec_ast::Literal,
        _span: aec_ast::Span,
        env: Env,
    ) -> Result<Value, RuntimeError> {
        Ok(match lit {
            aec_ast::Literal::Int(n) => Value::Int(*n),
            aec_ast::Literal::Float(f) => Value::Float(*f),
            aec_ast::Literal::String(s) => Value::String(s.clone()),
            aec_ast::Literal::RawString(s) => Value::String(s.clone()),
            aec_ast::Literal::Bool(b) => Value::Bool(*b),
            aec_ast::Literal::None => Value::None,
            aec_ast::Literal::Uuid(u) => Value::String(u.to_string()),
            aec_ast::Literal::ByteSize(bytes) => Value::Int(*bytes as i64),
            aec_ast::Literal::Duration(d) => Value::Int((d.value * d.unit.to_ms()) as i64),
            aec_ast::Literal::Interpolated(parts) => {
                let mut out = String::new();
                for part in parts {
                    match part {
                        aec_ast::InterpPart::Text(text) => out.push_str(text),
                        aec_ast::InterpPart::Expr(expr) => {
                            let value = self.eval_expr(expr, env.clone())?;
                            out.push_str(&interp_text(value));
                        }
                    }
                }
                Value::String(out)
            }
        })
    }

    fn binary_op(
        &mut self,
        op: BinaryOp,
        left: Value,
        right: Value,
        span: aec_ast::Span,
    ) -> Result<Value, RuntimeError> {
        match op {
            BinaryOp::Add => match (&left, &right) {
                (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a + b)),
                (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a + b)),
                (Value::Int(a), Value::Float(b)) => Ok(Value::Float(*a as f64 + b)),
                (Value::Float(a), Value::Int(b)) => Ok(Value::Float(a + *b as f64)),
                (Value::String(a), Value::String(b)) => Ok(Value::String(format!("{}{}", a, b))),
                (Value::Array(a), Value::Array(b)) => {
                    let mut new = a.clone();
                    new.extend(b.clone());
                    Ok(Value::Array(new))
                }
                _ => Err(RuntimeError::TypeError {
                    message: format!("can't add {} + {}", left.type_name(), right.type_name()),
                    span,
                }),
            },
            BinaryOp::Sub => num_op(left, right, span, |a, b| a - b, |a, b| a - b),
            BinaryOp::Mul => num_op(left, right, span, |a, b| a * b, |a, b| a * b),
            BinaryOp::Div => {
                if matches!(right, Value::Int(0)) {
                    return Err(RuntimeError::DivisionByZero { span });
                }
                num_op(left, right, span, |a, b| a / b, |a, b| a / b)
            }
            BinaryOp::Mod => {
                if matches!(right, Value::Int(0)) {
                    return Err(RuntimeError::DivisionByZero { span });
                }
                num_op(left, right, span, |a, b| a % b, |a, b| a % b)
            }
            BinaryOp::Eq => Ok(Value::Bool(values_equal(&left, &right))),
            BinaryOp::Neq => Ok(Value::Bool(!values_equal(&left, &right))),
            BinaryOp::Lt => cmp_op(left, right, span, |a, b| a < b),
            BinaryOp::Gt => cmp_op(left, right, span, |a, b| a > b),
            BinaryOp::Lte => cmp_op(left, right, span, |a, b| a <= b),
            BinaryOp::Gte => cmp_op(left, right, span, |a, b| a >= b),
            BinaryOp::And => Ok(Value::Bool(left.is_truthy() && right.is_truthy())),
            BinaryOp::Or => Ok(Value::Bool(left.is_truthy() || right.is_truthy())),
        }
    }
}

enum Flow {
    Normal(Value),
    Return(Value),
}

fn num_op(
    left: Value,
    right: Value,
    span: aec_ast::Span,
    int_op: fn(i64, i64) -> i64,
    float_op: fn(f64, f64) -> f64,
) -> Result<Value, RuntimeError> {
    match (&left, &right) {
        (Value::Int(a), Value::Int(b)) => Ok(Value::Int(int_op(*a, *b))),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Float(float_op(*a, *b))),
        (Value::Int(a), Value::Float(b)) => Ok(Value::Float(float_op(*a as f64, *b))),
        (Value::Float(a), Value::Int(b)) => Ok(Value::Float(float_op(*a, *b as f64))),
        _ => Err(RuntimeError::TypeError {
            message: format!("can't do math on {} and {}", left.type_name(), right.type_name()),
            span,
        }),
    }
}

fn cmp_op(
    left: Value,
    right: Value,
    span: aec_ast::Span,
    op: fn(f64, f64) -> bool,
) -> Result<Value, RuntimeError> {
    let (a, b) = match (&left, &right) {
        (Value::Int(a), Value::Int(b)) => (*a as f64, *b as f64),
        (Value::Float(a), Value::Float(b)) => (*a, *b),
        (Value::Int(a), Value::Float(b)) => (*a as f64, *b),
        (Value::Float(a), Value::Int(b)) => (*a, *b as f64),
        _ => {
            return Err(RuntimeError::TypeError {
                message: format!("can't compare {} and {}", left.type_name(), right.type_name()),
                span,
            })
        }
    };
    Ok(Value::Bool(op(a, b)))
}

/// Guards a builtin against the wrong number of arguments.
fn require_args(args: &[Value], expected: usize, span: aec_ast::Span) -> Result<(), RuntimeError> {
    if args.len() == expected {
        return Ok(());
    }
    Err(RuntimeError::WrongArgCount {
        expected,
        got: args.len(),
        span,
    })
}

/// Text of a value inside a string interpolation.
///
/// An object carrying a `text` field (a model response) is unpacked to that
/// field, so `"{reply}"` prints the answer instead of a debug dump.
fn interp_text(value: Value) -> String {
    match &value {
        Value::Object(fields) => match fields.get("text") {
            Some(text) => interp_text(text.clone()),
            None => value.to_string(),
        },
        _ => value.to_string(),
    }
}

fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => x == y,
        (Value::Float(x), Value::Float(y)) => x == y,
        (Value::Int(x), Value::Float(y)) => (*x as f64) == *y,
        (Value::Float(x), Value::Int(y)) => *x == (*y as f64),
        (Value::String(x), Value::String(y)) => x == y,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::None, Value::None) => true,
        _ => false,
    }
}
