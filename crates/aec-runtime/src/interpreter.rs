//! Interpreter — executing the AST

use crate::errors::RuntimeError;
use crate::llm;
use crate::memory::Memory;
use crate::permissions::{Limits, Permissions};
use crate::value::{Closure, Env, Environment, Function, Value};
use aec_ast::{
    AssignOp, BinaryOp, Block, ElseBranch, Expr, ForStmt, FunctionDecl, ModelDecl,
    IfStmt, LValue, LValueStep, MatchBody, Pattern, Program, Statement, UnaryOp,
    WhileStmt,
};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

pub struct Interpreter {
    pub global: Env,
    pub memory: Memory,
    /// Permission policy. `None` = the program has no `permissions` block → no gate is applied.
    pub permissions: Option<Permissions>,
    pub limits: Limits,
    module_envs: HashMap<String, Env>,
    module_env_ids: HashMap<usize, String>,
    models: HashMap<String, ModelDecl>,
    module_models: HashMap<String, ModelDecl>,
    module_secrets: HashMap<String, HashMap<String, String>>,
    secrets: HashMap<String, String>,
}

impl Interpreter {
    pub fn new() -> Self {
        let global = Environment::new();
        Self {
            global,
            memory: Memory::new(),
            permissions: None,
            limits: Limits::default(),
            module_envs: HashMap::new(),
            module_env_ids: HashMap::new(),
            models: HashMap::new(),
            module_models: HashMap::new(),
            module_secrets: HashMap::new(),
            secrets: HashMap::new(),
        }
    }

    pub fn run(&mut self, program: &Program) -> Result<(), RuntimeError> {
        self.global = Environment::new();
        self.memory = Memory::new();
        self.permissions = None;
        self.limits = Limits::default();
        self.module_envs.clear();
        self.module_env_ids.clear();
        self.models.clear();
        self.module_models.clear();
        self.module_secrets.clear();
        self.secrets.clear();

        let mut scopes: Vec<String> = program
            .item_modules
            .iter()
            .filter_map(|module| module.clone())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        scopes.sort_by_key(|scope| scope.matches('.').count());
        for scope in scopes {
            let parent = scope
                .rsplit_once('.')
                .and_then(|(parent, _)| self.module_envs.get(parent).cloned())
                .unwrap_or_else(|| self.global.clone());
            let env = Environment::with_parent(parent);
            self.module_env_ids
                .insert(Rc::as_ptr(&env) as usize, scope.clone());
            self.module_envs.insert(scope, env);
        }

        for (index, item) in program.items.iter().enumerate() {
            let module = program.item_modules.get(index).cloned().flatten();
            let env = module
                .as_deref()
                .and_then(|scope| self.module_envs.get(scope).cloned())
                .unwrap_or_else(|| self.global.clone());
            match item {
                aec_ast::TopLevelItem::Function(f) => {
                    self.register_function_in_env(f, module.as_deref(), env);
                }
                aec_ast::TopLevelItem::Model(model) => {
                    if let Some(scope) = module.as_deref() {
                        self.module_models
                            .insert(format!("{}::{}", scope, model.name.name), model.clone());
                        if model.is_public {
                            self.models
                                .insert(format!("{}.{}", scope, model.name.name), model.clone());
                        }
                    } else {
                        self.models.insert(model.name.name.clone(), model.clone());
                    }
                }
                aec_ast::TopLevelItem::Secrets(secrets) => {
                    let values = if let Some(scope) = module.as_deref() {
                        self.module_secrets
                            .entry(scope.to_string())
                            .or_default()
                    } else {
                        &mut self.secrets
                    };
                    for entry in &secrets.entries {
                        let value = match &entry.value {
                            aec_ast::SecretValue::String(value) => value.clone(),
                            aec_ast::SecretValue::Env(name) => {
                                std::env::var(name).unwrap_or_default()
                            }
                        };
                        values.insert(entry.name.name.clone(), value);
                    }
                }
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

    fn register_function_in_env(
        &mut self,
        f: &FunctionDecl,
        module: Option<&str>,
        env: Env,
    ) {
        let params = f.params.iter().map(|p| p.name.name.clone()).collect();
        let param_defaults = f.params.iter().map(|p| p.default.clone()).collect();
        let function = Rc::new(Function {
            name: f.name.name.clone(),
            params,
            param_defaults,
            body: f.body.clone(),
            env: env.clone(),
        });
        env.borrow_mut()
            .set(f.name.name.clone(), Value::Function(function.clone()));
        if let Some(scope) = module {
            if f.is_public {
                self.global.borrow_mut().set(
                    format!("{}.{}", scope, f.name.name),
                    Value::Function(function),
                );
            }
        } else {
            self.global
                .borrow_mut()
                .set(f.name.name.clone(), Value::Function(function));
        }
    }

    fn module_for_env(&self, env: &Env) -> Option<&str> {
        let mut current = Some(env.clone());
        while let Some(candidate) = current {
            let key = Rc::as_ptr(&candidate) as usize;
            if let Some(module) = self.module_env_ids.get(&key) {
                return Some(module.as_str());
            }
            current = candidate.borrow().parent.clone();
        }
        None
    }

    fn resolve_model(&self, name: &str, env: Option<&Env>) -> Option<(String, ModelDecl)> {
        if let Some(env) = env {
            if let Some(module) = self.module_for_env(env) {
                let public_key = format!("{}.{}", module, name);
                if let Some(model) = self.models.get(&public_key) {
                    return Some((public_key, model.clone()));
                }
                let key = format!("{}::{}", module, name);
                if let Some(model) = self.module_models.get(&key) {
                    return Some((key, model.clone()));
                }
            }
        }
        self.models
            .get(name)
            .cloned()
            .map(|model| (name.to_string(), model))
    }

    fn resolve_secret(&self, name: &str, env: Option<&Env>) -> Option<String> {
        env.and_then(|env| self.module_for_env(env))
            .and_then(|module| self.module_secrets.get(module))
            .and_then(|secrets| secrets.get(name).cloned())
            .or_else(|| self.secrets.get(name).cloned())
    }

    fn call_secrets_get(
        &mut self,
        args: &[(Option<String>, Value)],
        span: aec_ast::Span,
        env: Option<&Env>,
    ) -> Result<Value, RuntimeError> {
        if args.len() != 1 {
            return Err(RuntimeError::WrongArgCount {
                expected: 1,
                got: args.len(),
                span,
            });
        }
        if self.permissions.is_some() {
            return Err(crate::permissions::denied(
                "secret access is not available when permissions are declared".to_string(),
                span,
            ));
        }
        let name = match args.first().map(|(_, value)| value) {
            Some(Value::String(value)) => value.clone(),
            _ => {
                return Err(RuntimeError::TypeError {
                    message: "secrets.get needs a string".to_string(),
                    span,
                })
            }
        };
        Ok(self
            .resolve_secret(&name, env)
            .map(Value::String)
            .unwrap_or(Value::None))
    }

    fn expression_path(expr: &Expr) -> Option<String> {
        match expr {
            Expr::Identifier(identifier) => Some(identifier.name.clone()),
            Expr::Member(member) => {
                let mut path = Self::expression_path(&member.object)?;
                path.push('.');
                path.push_str(&member.property.name);
                Some(path)
            }
            _ => None,
        }
    }

    pub fn call_function(
        &mut self,
        name: &str,
        args: Vec<Value>,
        span: aec_ast::Span,
    ) -> Result<Value, RuntimeError> {
        let args = args.into_iter().map(|value| (None, value)).collect::<Vec<_>>();
        self.call_function_values(name, &args, span)
    }

    pub fn call_function_with_arguments(
        &mut self,
        name: &str,
        args: &[aec_ast::Argument],
        span: aec_ast::Span,
    ) -> Result<Value, RuntimeError> {
        let mut values = Vec::with_capacity(args.len());
        for argument in args {
            values.push((
                argument.name.as_ref().map(|name| name.name.clone()),
                self.eval_expr(&argument.value, self.global.clone())?,
            ));
        }
        self.call_function_values(name, &values, span)
    }

    fn call_function_values(
        &mut self,
        name: &str,
        args: &[(Option<String>, Value)],
        span: aec_ast::Span,
    ) -> Result<Value, RuntimeError> {
        let effective_name = name.to_string();
        if let Some((namespace, method)) = effective_name.rsplit_once('.') {
            if method == "think" || method == "complete" {
                if let Some((model_name, model)) = self.resolve_model(namespace, None) {
                    return self.call_model(&model_name, model, args, span);
                }
            }
            if namespace == "secrets" && method == "get" {
                if args.len() != 1 {
                    return Err(RuntimeError::WrongArgCount {
                        expected: 1,
                        got: args.len(),
                        span,
                    });
                }
                if self.permissions.is_some() {
                    return Err(crate::permissions::denied(
                        "secret access is not available when permissions are declared".to_string(),
                        span,
                    ));
                }
                let name = match args.first().map(|(_, value)| value) {
                    Some(Value::String(value)) => value.clone(),
                    _ => {
                        return Err(RuntimeError::TypeError {
                            message: "secrets.get needs a string".to_string(),
                            span,
                        })
                    }
                };
                return Ok(self
                    .secrets
                    .get(&name)
                    .cloned()
                    .map(Value::String)
                    .unwrap_or(Value::None));
            }
        }
        let bound = {
            let environment = self.global.borrow();
            environment.get(&effective_name)
        };
        if let Some(value) = bound {
            match value {
                Value::Function(function) => return self.invoke_function(function, args, span),
                Value::Closure(closure) => return self.call_closure_values(closure, args, span),
                other => {
                    return Err(RuntimeError::TypeError {
                        message: format!("'{}' is not a function (got {})", effective_name, other.type_name()),
                        span,
                    });
                }
            }
        }

        if args.iter().any(|(argument_name, _)| argument_name.is_some()) {
            return Err(RuntimeError::Generic {
                message: format!("named arguments are not supported by builtin '{}'", effective_name),
                span,
            });
        }
        let positional = args.iter().map(|(_, value)| value.clone()).collect::<Vec<_>>();
        if let Some(result) = self.try_builtin(&effective_name, &positional, span)? {
            return Ok(result);
        }

        Err(RuntimeError::UndefinedFunction {
            name: name.to_string(),
            span,
        })
    }

    fn invoke_function(
        &mut self,
        function: Rc<Function>,
        args: &[(Option<String>, Value)],
        span: aec_ast::Span,
    ) -> Result<Value, RuntimeError> {
        let local_env = Environment::with_parent(function.env.clone());
        let values = self.bind_function_args(&function, args, local_env.clone(), span)?;
        for (param, value) in function.params.iter().zip(values) {
            local_env.borrow_mut().set(param.clone(), value);
        }

        match self.exec_block(&function.body, local_env) {
            Ok(Flow::Normal(v)) | Ok(Flow::Return(v)) => Ok(v),
            Err(RuntimeError::EarlyReturn { value }) => Ok(Value::Result(Err(value))),
            Err(e) => Err(e),
        }
    }

    fn bind_function_args(
        &mut self,
        function: &Function,
        args: &[(Option<String>, Value)],
        local_env: Env,
        span: aec_ast::Span,
    ) -> Result<Vec<Value>, RuntimeError> {
        let mut slots = vec![None; function.params.len()];
        let mut next_positional = 0;
        let mut named_seen = false;
        for (name, value) in args {
            if let Some(name) = name {
                named_seen = true;
                let Some(index) = function.params.iter().position(|param| param == name) else {
                    return Err(RuntimeError::Generic {
                        message: format!("unknown argument '{}' for function '{}',", name, function.name),
                        span,
                    });
                };
                if slots[index].is_some() {
                    return Err(RuntimeError::Generic {
                        message: format!("argument '{}' was provided more than once", name),
                        span,
                    });
                }
                slots[index] = Some(value.clone());
            } else {
                if named_seen {
                    return Err(RuntimeError::Generic {
                        message: "positional arguments cannot follow named arguments".to_string(),
                        span,
                    });
                }
                if next_positional >= slots.len() {
                    return Err(RuntimeError::WrongArgCount {
                        expected: slots.len(),
                        got: args.len(),
                        span,
                    });
                }
                slots[next_positional] = Some(value.clone());
                next_positional += 1;
            }
        }

        let mut values = Vec::with_capacity(slots.len());
        for (index, slot) in slots.into_iter().enumerate() {
            if let Some(value) = slot {
                values.push(value);
                continue;
            }
            let Some(default) = function.param_defaults.get(index).and_then(|value| value.clone())
            else {
                let expected = function
                    .params
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| {
                        function
                            .param_defaults
                            .get(*index)
                            .and_then(|default| default.as_ref())
                            .is_none()
                    })
                    .count();
                return Err(RuntimeError::WrongArgCount {
                    expected,
                    got: args.len(),
                    span,
                });
            };
            values.push(self.eval_expr(&default, local_env.clone())?);
        }
        Ok(values)
    }

    fn call_closure_values(
        &mut self,
        closure: Rc<Closure>,
        args: &[(Option<String>, Value)],
        span: aec_ast::Span,
    ) -> Result<Value, RuntimeError> {
        let mut slots = vec![None; closure.params.len()];
        let mut next_positional = 0;
        for (name, value) in args {
            if let Some(name) = name {
                let Some(index) = closure.params.iter().position(|param| param == name) else {
                    return Err(RuntimeError::Generic {
                        message: format!("unknown argument '{}' for lambda", name),
                        span,
                    });
                };
                if slots[index].is_some() {
                    return Err(RuntimeError::Generic {
                        message: format!("argument '{}' was provided more than once", name),
                        span,
                    });
                }
                slots[index] = Some(value.clone());
            } else {
                if next_positional >= slots.len() {
                    return Err(RuntimeError::WrongArgCount {
                        expected: slots.len(),
                        got: args.len(),
                        span,
                    });
                }
                slots[next_positional] = Some(value.clone());
                next_positional += 1;
            }
        }
        let values = slots
            .into_iter()
            .map(|value| {
                value.ok_or_else(|| RuntimeError::WrongArgCount {
                    expected: closure.params.len(),
                    got: args.len(),
                    span,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.call_closure(closure, values, span)
    }

    fn try_builtin(
        &mut self,
        name: &str,
        args: &[Value],
        span: aec_ast::Span,
    ) -> Result<Option<Value>, RuntimeError> {
        crate::stdlib::validate_builtin_args(name, args, span)?;
        // Permission gate: if the program declared a `permissions` block, every builtin
        // is checked against the allowlist before it runs. Unknown names (user
        // functions) pass through untouched.
        if let Some(perms) = &self.permissions {
            perms
                .check_builtin(name, args)
                .map_err(|message| crate::permissions::denied(message, span))?;
        }

        // check stdlib first
        let restricted = self.permissions.is_some();
        if let Some(v) = crate::stdlib::call_builtin(name, args, span, &self.limits, restricted)? {
            return Ok(Some(v));
        }
        if let Some(v) = crate::stdlib::call_builtin2(name, args, span)? {
            return Ok(Some(v));
        }
        if let Some(v) = crate::stdlib_extended::call_extended(name, args, span, &self.limits, restricted)? {
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
            "bool" => {
                require_args(args, 1, span)?;
                Value::Bool(match &args[0] {
                    Value::String(value) => value.eq_ignore_ascii_case("true"),
                    other => other.is_truthy(),
                })
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
                    message: format!("pop() doesn't work on {}", v.type_name()),
                    span,
                }),
            },
            "map" => {
                let (Value::Array(values), Value::Closure(function)) = (&args[0], &args[1]) else {
                    return Err(RuntimeError::TypeError {
                        message: "map() needs an array and a function".to_string(),
                        span,
                    });
                };
                let mut result = Vec::with_capacity(values.len());
                for value in values {
                    result.push(self.call_closure(function.clone(), vec![value.clone()], span)?);
                }
                Value::Array(result)
            }
            "filter" => {
                let (Value::Array(values), Value::Closure(function)) = (&args[0], &args[1]) else {
                    return Err(RuntimeError::TypeError {
                        message: "filter() needs an array and a function".to_string(),
                        span,
                    });
                };
                let mut result = Vec::new();
                for value in values {
                    let keep = self.call_closure(function.clone(), vec![value.clone()], span)?;
                    if keep.is_truthy() {
                        result.push(value.clone());
                    }
                }
                Value::Array(result)
            }
            "reduce" => {
                let (Value::Array(values), Value::Closure(function)) = (&args[0], &args[1]) else {
                    return Err(RuntimeError::TypeError {
                        message: "reduce() needs an array and a function".to_string(),
                        span,
                    });
                };
                let mut accumulator = args[2].clone();
                for value in values {
                    accumulator = self.call_closure(
                        function.clone(),
                        vec![accumulator, value.clone()],
                        span,
                    )?;
                }
                accumulator
            }
            "range" => match args.len() {

                1 => match &args[0] {
                    Value::Int(n) => {
                        if *n < 0 {
                            return Err(RuntimeError::Generic {
                                message: "range() end must be non-negative".to_string(),
                                span,
                            });
                        }
                        if *n > 10_000_000 {
                            return Err(RuntimeError::Generic {
                                message: "range() result is too large".to_string(),
                                span,
                            });
                        }
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
                        if *start < 0 || *end < *start {
                            return Err(RuntimeError::Generic {
                                message: "range() requires 0 <= start <= end".to_string(),
                                span,
                            });
                        }
                        if (*end - *start) > 10_000_000 {
                            return Err(RuntimeError::Generic {
                                message: "range() result is too large".to_string(),
                                span,
                            });
                        }
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
                Value::Int(n) => Value::Int(n.checked_abs().ok_or_else(|| RuntimeError::Generic {
                    message: "integer overflow in abs()".to_string(),
                    span,
                })?),
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
                (Value::Int(a), Value::Int(b)) => {
                    if *b < 0 {
                        return Err(RuntimeError::Generic {
                            message: "pow() does not accept a negative integer exponent".to_string(),
                            span,
                        });
                    }
                    let exponent = u32::try_from(*b).map_err(|_| RuntimeError::Generic {
                        message: "pow() exponent is too large".to_string(),
                        span,
                    })?;
                    Value::Int(a.checked_pow(exponent).ok_or_else(|| RuntimeError::Generic {
                        message: "integer overflow in pow()".to_string(),
                        span,
                    })?)
                }
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
                return llm::llm_complete(args, span, restricted, &self.limits).map(Some);
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

    fn call_model(
        &mut self,
        name: &str,
        model: ModelDecl,
        args: &[(Option<String>, Value)],
        span: aec_ast::Span,
    ) -> Result<Value, RuntimeError> {
        if args.len() != 1 {
            return Err(RuntimeError::WrongArgCount {
                expected: 1,
                got: args.len(),
                span,
            });
        }
        let mut options = HashMap::new();
        for field in &model.fields {
            if let aec_ast::ModelField::Property(property) = field {
                let key = match property.name.name.as_str() {
                    "name" => "model",
                    "endpoint" => "base_url",
                    other => other,
                };
                options.insert(key.to_string(), literal_value(&property.value));
            }
        }
        for (argument_name, value) in args {
            match argument_name.as_deref() {
                Some("prompt") | Some("message") => {
                    options.insert("prompt".to_string(), value.clone());
                }
                None => {
                    if let Value::Object(values) = value {
                        options.extend(values.clone());
                    } else {
                        options.insert("prompt".to_string(), value.clone());
                    }
                }
                Some(key) => {
                    options.insert(key.to_string(), value.clone());
                }
            }
        }
        if !options.contains_key("prompt") {
            return Err(RuntimeError::TypeError {
                message: format!("model '{}' needs a prompt", name),
                span,
            });
        }
        if let Some(permissions) = &self.permissions {
            permissions
                .check_builtin("llm.complete", &[Value::Object(options.clone())])
                .map_err(|message| crate::permissions::denied(message, span))?;
        }
        let restricted = self.permissions.is_some();
        llm::llm_complete(
            &[Value::Object(options)],
            span,
            restricted,
            &self.limits,
        )
    }

    fn call_value(
        &mut self,
        value: &Value,
        args: &[(Option<String>, Value)],
        span: aec_ast::Span,
    ) -> Result<Value, RuntimeError> {
        match value {
            Value::Function(function) => self.invoke_function(function.clone(), args, span),
            Value::Closure(closure) => self.call_closure_values(closure.clone(), args, span),
            other => Err(RuntimeError::TypeError {
                message: format!("value of type {} is not callable", other.type_name()),
                span,
            }),
        }
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

        let mut root = env
            .borrow()
            .get(&lv.base.name)
            .ok_or_else(|| RuntimeError::UndefinedVariable {
                name: lv.base.name.clone(),
                span: lv.span,
            })?;
        self.assign_path(&mut root, &lv.path, env.clone(), lv.span, value)?;
        if !env.borrow_mut().assign(&lv.base.name, root) {
            return Err(RuntimeError::UndefinedVariable {
                name: lv.base.name.clone(),
                span: lv.span,
            });
        }
        Ok(())
    }

    fn assign_path(
        &mut self,
        current: &mut Value,
        path: &[LValueStep],
        env: Env,
        span: aec_ast::Span,
        value: Value,
    ) -> Result<(), RuntimeError> {
        let Some((step, rest)) = path.split_first() else {
            *current = value;
            return Ok(());
        };

        match step {
            LValueStep::Member(id) => {
                let Value::Object(object) = current else {
                    return Err(RuntimeError::TypeError {
                        message: format!("can't assign .{} on {}", id.name, current.type_name()),
                        span,
                    });
                };
                if rest.is_empty() {
                    object.insert(id.name.clone(), value);
                    return Ok(());
                }
                let child = object.get_mut(&id.name).ok_or_else(|| RuntimeError::Generic {
                    message: format!("no field '{}'", id.name),
                    span,
                })?;
                self.assign_path(child, rest, env, span, value)
            }
            LValueStep::Index(index_expr) => {
                let index = self.eval_expr(index_expr, env.clone())?;
                match (current, index) {
                    (Value::Array(array), Value::Int(index)) => {
                        let index = if index < 0 {
                            array.len() as i64 + index
                        } else {
                            index
                        };
                        if index < 0 || index as usize >= array.len() {
                            return Err(RuntimeError::Generic {
                                message: format!("index out of bounds: {}", index),
                                span,
                            });
                        }
                        if rest.is_empty() {
                            array[index as usize] = value;
                            return Ok(());
                        }
                        let child = &mut array[index as usize];
                        self.assign_path(child, rest, env, span, value)
                    }
                    (Value::Object(object), Value::String(key)) => {
                        if rest.is_empty() {
                            object.insert(key, value);
                            return Ok(());
                        }
                        let child = object.get_mut(&key).ok_or_else(|| RuntimeError::Generic {
                            message: format!("no key '{}'", key),
                            span,
                        })?;
                        self.assign_path(child, rest, env, span, value)
                    }
                    (value, _) => Err(RuntimeError::TypeError {
                        message: format!("can't index {}", value.type_name()),
                        span,
                    }),
                }
            }
        }
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
                        Value::Int(n) => n.checked_neg().map(Value::Int).ok_or_else(|| RuntimeError::Generic {
                            message: "integer overflow in unary -".to_string(),
                            span: u.span,
                        }),
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
                let mut args = Vec::with_capacity(call.args.len());
                for argument in &call.args {
                    args.push((
                        argument.name.as_ref().map(|name| name.name.clone()),
                        self.eval_expr(&argument.value, env.clone())?,
                    ));
                }

                if let Expr::Identifier(id) = &call.callee {
                    let value = {
                        let environment = env.borrow();
                        environment.get(&id.name)
                    };
                    if let Some(value) = value {
                        return self.call_value(&value, &args, call.span);
                    }
                    return self.call_function_values(&id.name, &args, call.span);
                }

                if let Some(path) = Self::expression_path(&call.callee) {
                    if path == "secrets.get" {
                        return self.call_secrets_get(&args, call.span, Some(&env));
                    }
                    if let Some((model_name, method)) = path.rsplit_once('.') {
                        if method == "think" || method == "complete" {
                            if let Some((model_key, model)) =
                                self.resolve_model(model_name, Some(&env))
                            {
                                return self.call_model(&model_key, model, &args, call.span);
                            }
                        }
                    }
                }

                if let Expr::Member(member) = &call.callee {
                    if let Expr::Identifier(namespace) = &member.object {
                        let name = format!("{}.{}", namespace.name, member.property.name);
                        let effective_name = self
                            .module_for_env(&env)
                            .map(|module| format!("{}.{}", module, name))
                            .filter(|candidate| self.global.borrow().get(candidate).is_some())
                            .unwrap_or(name);
                        return self.call_function_values(&effective_name, &args, call.span);
                    }
                    let object = self.eval_expr(&member.object, env.clone())?;
                    if let Value::Object(fields) = object {
                        if let Some(value) = fields.get(&member.property.name) {
                            return self.call_value(value, &args, call.span);
                        }
                    }
                    return Err(RuntimeError::Generic {
                        message: format!("no callable field '{}'", member.property.name),
                        span: call.span,
                    });
                }

                let callee = self.eval_expr(&call.callee, env)?;
                self.call_value(&callee, &args, call.span)
            }
            Expr::Member(m) => {
                if let Some(path) = Self::expression_path(&Expr::Member(m.clone())) {
                    if let Some((model_name, field_name)) = path.rsplit_once('.') {
                        if let Some((_, model)) = self.resolve_model(model_name, Some(&env)) {
                            let mut fields = HashMap::new();
                            for field in &model.fields {
                                if let aec_ast::ModelField::Property(property) = field {
                                    fields.insert(
                                        property.name.name.clone(),
                                        literal_value(&property.value),
                                    );
                                }
                            }
                            return fields.get(field_name).cloned().ok_or_else(|| {
                                RuntimeError::Generic {
                                    message: format!("no model field '{}'", field_name),
                                    span: m.span,
                                }
                            });
                        }
                    }
                }
                if let Expr::Identifier(identifier) = &m.object {
                    if identifier.name == "secrets" {
                        if self.permissions.is_some() {
                            return Err(crate::permissions::denied(
                                "secret access is not available when permissions are declared".to_string(),
                                m.span,
                            ));
                        }
                        return self
                            .resolve_secret(&m.property.name, Some(&env))
                            .map(Value::String)
                            .ok_or_else(|| RuntimeError::Generic {
                                message: format!("unknown secret '{}'", m.property.name),
                                span: m.span,
                            });
                    }
                }
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
            Expr::Await(a) => Err(RuntimeError::Generic {
                message: "await is not supported by the synchronous interpreter".to_string(),
                span: a.span,
            }),
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
            if let Some(bindings) = self.pattern_bindings(&arm.pattern, &scrutinee)? {
                let arm_env = Environment::with_parent(env.clone());
                for (name, value) in bindings {
                    arm_env.borrow_mut().set(name, value);
                }
                return match &arm.body {
                    MatchBody::Expr(expression) => self.eval_expr(expression, arm_env),
                    MatchBody::Block(block) => match self.exec_block(block, arm_env)? {
                        Flow::Normal(value) | Flow::Return(value) => Ok(value),
                    },
                };
            }
        }
        Ok(Value::None)
    }

    fn pattern_bindings(
        &mut self,
        pattern: &Pattern,
        value: &Value,
    ) -> Result<Option<Vec<(String, Value)>>, RuntimeError> {
        match pattern {
            Pattern::Wildcard(_) => Ok(Some(Vec::new())),
            Pattern::Literal(literal) => {
                let expected = match literal {
                    aec_ast::Literal::Int(number) => Value::Int(*number),
                    aec_ast::Literal::Float(number) => Value::Float(*number),
                    aec_ast::Literal::String(text) | aec_ast::Literal::RawString(text) => {
                        Value::String(text.clone())
                    }
                    aec_ast::Literal::Bool(value) => Value::Bool(*value),
                    aec_ast::Literal::None => Value::None,
                    aec_ast::Literal::Uuid(value) => Value::String(value.to_string()),
                    aec_ast::Literal::ByteSize(value) => Value::Int(*value as i64),
                    aec_ast::Literal::Duration(value) => Value::Int(
                        value
                            .value
                            .checked_mul(value.unit.to_ms())
                            .and_then(|milliseconds| i64::try_from(milliseconds).ok())
                            .ok_or_else(|| RuntimeError::Generic {
                                message: "duration pattern is too large".to_string(),
                                span: aec_ast::Span::dummy(),
                            })?,
                    ),
                    aec_ast::Literal::Interpolated(_) => return Ok(None),
                };
                Ok(values_equal(&expected, value).then_some(Vec::new()))
            }
            Pattern::None(_) => Ok(matches!(value, Value::None).then_some(Vec::new())),
            Pattern::Some(identifier) => match value {
                Value::None => Ok(None),
                Value::Result(Ok(inner)) => Ok(Some(vec![(
                    identifier.name.clone(),
                    (**inner).clone(),
                )])),
                other => Ok(Some(vec![(identifier.name.clone(), other.clone())])),
            },
            Pattern::Identifier(identifier) => {
                Ok(Some(vec![(identifier.name.clone(), value.clone())]))
            }
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
            aec_ast::Literal::Duration(d) => Value::Int(
                d.value
                    .checked_mul(d.unit.to_ms())
                    .and_then(|value| i64::try_from(value).ok())
                    .ok_or_else(|| RuntimeError::Generic {
                        message: "duration literal is too large".to_string(),
                        span: _span,
                    })?,
            ),
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
                (Value::Int(a), Value::Int(b)) => a
                    .checked_add(*b)
                    .map(Value::Int)
                    .ok_or_else(|| RuntimeError::Generic {
                        message: "integer overflow in +".to_string(),
                        span,
                    }),
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
            BinaryOp::Sub => num_op(BinaryOp::Sub, left, right, span),
            BinaryOp::Mul => num_op(BinaryOp::Mul, left, right, span),
            BinaryOp::Div => {
                if matches!(right, Value::Int(0) | Value::Float(0.0)) {
                    return Err(RuntimeError::DivisionByZero { span });
                }
                num_op(BinaryOp::Div, left, right, span)
            }
            BinaryOp::Mod => {
                if matches!(right, Value::Int(0) | Value::Float(0.0)) {
                    return Err(RuntimeError::DivisionByZero { span });
                }
                num_op(BinaryOp::Mod, left, right, span)
            }
            BinaryOp::Eq => Ok(Value::Bool(values_equal(&left, &right))),
            BinaryOp::Neq => Ok(Value::Bool(!values_equal(&left, &right))),
            BinaryOp::Lt => cmp_op(left, right, span, BinaryOp::Lt),
            BinaryOp::Gt => cmp_op(left, right, span, BinaryOp::Gt),
            BinaryOp::Lte => cmp_op(left, right, span, BinaryOp::Lte),
            BinaryOp::Gte => cmp_op(left, right, span, BinaryOp::Gte),
            BinaryOp::And => Ok(Value::Bool(left.is_truthy() && right.is_truthy())),
            BinaryOp::Or => Ok(Value::Bool(left.is_truthy() || right.is_truthy())),
        }
    }
}

impl Default for Interpreter {
    fn default() -> Self {
        Self::new()
    }
}

enum Flow {
    Normal(Value),
    Return(Value),
}

fn num_op(
    op: BinaryOp,
    left: Value,
    right: Value,
    span: aec_ast::Span,
) -> Result<Value, RuntimeError> {
    match (&left, &right) {
        (Value::Int(a), Value::Int(b)) => {
            let result = match op {
                BinaryOp::Sub => a.checked_sub(*b),
                BinaryOp::Mul => a.checked_mul(*b),
                BinaryOp::Div => a.checked_div(*b),
                BinaryOp::Mod => a.checked_rem(*b),
                _ => None,
            };
            result.map(Value::Int).ok_or_else(|| RuntimeError::Generic {
                message: format!("integer operation failed for {} and {}", a, b),
                span,
            })
        }
        (Value::Float(a), Value::Float(b)) => Ok(Value::Float(match op {
            BinaryOp::Sub => a - b,
            BinaryOp::Mul => a * b,
            BinaryOp::Div => a / b,
            BinaryOp::Mod => a % b,
            _ => return Err(RuntimeError::Generic { message: "invalid numeric operation".to_string(), span }),
        })),
        (Value::Int(a), Value::Float(b)) => Ok(Value::Float(match op {
            BinaryOp::Sub => *a as f64 - b,
            BinaryOp::Mul => *a as f64 * b,
            BinaryOp::Div => *a as f64 / b,
            BinaryOp::Mod => *a as f64 % b,
            _ => return Err(RuntimeError::Generic { message: "invalid numeric operation".to_string(), span }),
        })),
        (Value::Float(a), Value::Int(b)) => Ok(Value::Float(match op {
            BinaryOp::Sub => a - *b as f64,
            BinaryOp::Mul => a * *b as f64,
            BinaryOp::Div => a / *b as f64,
            BinaryOp::Mod => a % *b as f64,
            _ => return Err(RuntimeError::Generic { message: "invalid numeric operation".to_string(), span }),
        })),
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
    op: BinaryOp,
) -> Result<Value, RuntimeError> {
    let result = match (&left, &right) {
        (Value::Int(a), Value::Int(b)) => match op {
            BinaryOp::Lt => a < b,
            BinaryOp::Gt => a > b,
            BinaryOp::Lte => a <= b,
            BinaryOp::Gte => a >= b,
            _ => return Err(RuntimeError::Generic { message: "invalid comparison".to_string(), span }),
        },
        (Value::Float(a), Value::Float(b)) => match op {
            BinaryOp::Lt => a < b,
            BinaryOp::Gt => a > b,
            BinaryOp::Lte => a <= b,
            BinaryOp::Gte => a >= b,
            _ => return Err(RuntimeError::Generic { message: "invalid comparison".to_string(), span }),
        },
        (Value::Int(a), Value::Float(b)) => match op {
            BinaryOp::Lt => (*a as f64) < *b,
            BinaryOp::Gt => (*a as f64) > *b,
            BinaryOp::Lte => (*a as f64) <= *b,
            BinaryOp::Gte => (*a as f64) >= *b,
            _ => return Err(RuntimeError::Generic { message: "invalid comparison".to_string(), span }),
        },
        (Value::Float(a), Value::Int(b)) => match op {
            BinaryOp::Lt => *a < (*b as f64),
            BinaryOp::Gt => *a > (*b as f64),
            BinaryOp::Lte => *a <= (*b as f64),
            BinaryOp::Gte => *a >= (*b as f64),
            _ => return Err(RuntimeError::Generic { message: "invalid comparison".to_string(), span }),
        },
        _ => {
            return Err(RuntimeError::TypeError {
                message: format!("can't compare {} and {}", left.type_name(), right.type_name()),
                span,
            })
        }
    };
    Ok(Value::Bool(result))
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
fn literal_value(literal: &aec_ast::Literal) -> Value {
    match literal {
        aec_ast::Literal::None => Value::None,
        aec_ast::Literal::String(value) | aec_ast::Literal::RawString(value) => {
            Value::String(value.clone())
        }
        aec_ast::Literal::Int(value) => Value::Int(*value),
        aec_ast::Literal::Float(value) => Value::Float(*value),
        aec_ast::Literal::Bool(value) => Value::Bool(*value),
        aec_ast::Literal::Uuid(value) => Value::String(value.to_string()),
        aec_ast::Literal::ByteSize(value) => Value::Int(*value as i64),
        aec_ast::Literal::Duration(value) => Value::Int(
            value
                .value
                .checked_mul(value.unit.to_ms())
                .and_then(|value| i64::try_from(value).ok())
                .unwrap_or(i64::MAX),
        ),
        aec_ast::Literal::Interpolated(parts) => Value::String(
            parts
                .iter()
                .map(|part| match part {
                    aec_ast::InterpPart::Text(text) => text.clone(),
                    aec_ast::InterpPart::Expr(_) => String::new(),
                })
                .collect(),
        ),
    }
}

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
