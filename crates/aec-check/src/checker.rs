//! Type checker — a single pass over the AST before execution

use crate::diag::{DiagKind, Diagnostic};
use crate::ty::{compatible, ty_from_expr, Ty};
use aec_ast::{
    AssignOp, AssignStmt, BinaryOp, Block, ElseBranch, Expr, ForStmt, FunctionDecl, IfStmt, LValue,
    LValueStep, LetStmt, MatchBody, Program, ReturnStmt, Span, Statement, TopLevelItem, UnaryOp,
    WhileStmt,
};
use std::collections::{HashMap, HashSet};

/// Namespaces that exist as globals at runtime (`file.read`, `http.get`, ...).
///
/// Keep in sync with the match arms in `aec-runtime`:
/// `interpreter.rs` (`try_builtin`), `stdlib.rs`, `stdlib_extended.rs`.
const BUILTIN_NAMESPACES: &[&str] = &[
    "secrets", "memory", "ui", "system", "env", "file", "http", "json", "time", "math", "sys",
    "crypto", "regex", "shell", "uuid", "log", "llm",
];

/// Built-in functions without a namespace.
///
/// The runtime accepts both the dotted and the underscored spelling of a
/// builtin, so bare `file_read(...)` is as valid as `file.read(...)`. Both
/// forms have to be listed here or the checker emits a false
/// "undeclared variable" warning.
///
/// Keep in sync with the match arms in `aec-runtime`:
/// `interpreter.rs` (`try_builtin`), `stdlib.rs`, `stdlib_extended.rs`.
const BUILTIN_FUNCTIONS: &[&str] = &[
    // core / strings
    "print",
    "read_line",
    "len",
    "str",
    "int",
    "float",
    "bool",
    "upper",
    "lower",
    "trim",
    "split",
    "join",
    "contains",
    "replace",
    "starts_with",
    "ends_with",
    "repeat",
    "char_at",
    // arrays / objects
    "push",
    "push_to",
    "pop",
    "sort",
    "reverse",
    "first",
    "last",
    "slice",
    "range",
    "keys",
    "values",
    "has",
    // math
    "abs",
    "min",
    "max",
    "sqrt",
    "pow",
    "pi",
    "e",
    "sin",
    "cos",
    "tan",
    "log",
    "log10",
    "exp",
    "floor",
    "ceil",
    "round",
    "random",
    "random_int",
    // time
    "now",
    "now_ms",
    "sleep",
    // process
    "exit",
    "args",
    // llm / memory
    "llm_complete",
    "memory_open",
    "memory_add",
    "memory_get",
    "memory_clear",
    "memory_count",
    // env / file
    "env_get",
    "env_set",
    "file_read",
    "file_write",
    "file_append",
    "file_exists",
    "file_delete",
    "file_copy",
    "file_mkdir",
    "file_size",
    "file_list_dir",
    "ls",
    // http / json
    "http_get",
    "http_post",
    "http_put",
    "http_delete",
    "json_parse",
    "json_stringify",
    // sys
    "sys_exit",
    "sys_args",
    "sys_info",
    "sys_env_all",
    // time (underscored)
    "time_now_ms",
    "time_now_sec",
    "time_sleep",
    // math (underscored)
    "math_pi",
    "math_e",
    "math_sin",
    "math_cos",
    "math_tan",
    "math_log",
    "math_floor",
    "math_ceil",
    "math_round",
    "math_exp",
    "math_random",
    "math_random_int",
    // shell / regex
    "shell_run",
    "regex_match",
    "regex_find",
    "regex_find_all",
    "regex_replace",
    // crypto / uuid
    "md5",
    "sha256",
    "sha512",
    "base64_encode",
    "base64_decode",
    "b64_encode",
    "b64_decode",
    "crypto_md5",
    "crypto_sha256",
    "crypto_sha512",
    "uuid",
    "uuid_v4",
];

struct ParamSig {
    ty: Ty,
    has_default: bool,
}

/// A name bound in a local scope.
struct Binding {
    ty: Ty,
    /// `false` for `let` — assigning to it is an error.
    mutable: bool,
}

struct FuncSig {
    params: Vec<ParamSig>,
    ret: Ty,
}

/// Type checker for a program. Use it via `check_program`.
pub struct Checker {
    functions: HashMap<String, FuncSig>,
    globals: HashSet<String>,
    diagnostics: Vec<Diagnostic>,
    scopes: Vec<HashMap<String, Binding>>,
    /// Return type of the function currently being checked
    expected_return: Option<Ty>,
}

impl Default for Checker {
    fn default() -> Self {
        Self::new()
    }
}

impl Checker {
    pub fn new() -> Self {
        Self {
            functions: HashMap::new(),
            globals: HashSet::new(),
            diagnostics: Vec::new(),
            scopes: Vec::new(),
            expected_return: None,
        }
    }

    /// Checks the program and returns the diagnostics (errors + warnings).
    pub fn check_program(mut self, program: &Program) -> Vec<Diagnostic> {
        self.collect_globals(program);
        self.collect_functions(program);

        for item in &program.items {
            if let TopLevelItem::Function(f) = item {
                self.check_function(f);
            }
        }

        self.diagnostics
    }

    // ---------- collection ----------

    fn collect_globals(&mut self, program: &Program) {
        for name in BUILTIN_NAMESPACES {
            self.globals.insert((*name).to_string());
        }
        for name in BUILTIN_FUNCTIONS {
            self.globals.insert((*name).to_string());
        }
        for item in &program.items {
            if let TopLevelItem::Model(m) = item {
                self.globals.insert(m.name.name.clone());
            }
        }
    }

    fn collect_functions(&mut self, program: &Program) {
        for item in &program.items {
            if let TopLevelItem::Function(f) = item {
                let params = f
                    .params
                    .iter()
                    .map(|p| ParamSig {
                        ty: ty_from_expr(&p.ty),
                        has_default: p.default.is_some(),
                    })
                    .collect();
                let ret = f.return_type.as_ref().map(ty_from_expr).unwrap_or(Ty::Unit);
                self.functions
                    .insert(f.name.name.clone(), FuncSig { params, ret });
            }
        }
    }

    fn check_function(&mut self, f: &FunctionDecl) {
        let expected = self.return_type_of(&f.name.name).unwrap_or(Ty::Unit);

        self.expected_return = Some(expected.clone());
        self.scopes.push(HashMap::new());
        for p in &f.params {
            self.declare(&p.name.name, ty_from_expr(&p.ty), true);
        }
        self.check_block(&f.body, Some(&expected));
        self.scopes.pop();
    }

    fn return_type_of(&self, name: &str) -> Option<Ty> {
        self.functions.get(name).map(|s| s.ret.clone())
    }

    // ---------- scope ----------

    fn declare(&mut self, name: &str, ty: Ty, mutable: bool) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_string(), Binding { ty, mutable });
        }
    }

    fn lookup_binding(&self, name: &str) -> Option<&Binding> {
        for scope in self.scopes.iter().rev() {
            if let Some(binding) = scope.get(name) {
                return Some(binding);
            }
        }
        None
    }

    fn lookup(&self, name: &str) -> Option<&Ty> {
        self.lookup_binding(name).map(|b| &b.ty)
    }

    fn is_declared(&self, name: &str) -> bool {
        self.lookup(name).is_some()
            || self.globals.contains(name)
            || self.functions.contains_key(name)
    }

    // ---------- statements ----------

    fn check_block(&mut self, block: &Block, expected: Option<&Ty>) {
        self.scopes.push(HashMap::new());
        for stmt in &block.statements {
            self.check_statement(stmt, expected);
        }
        self.scopes.pop();
    }

    fn check_statement(&mut self, stmt: &Statement, expected: Option<&Ty>) {
        match stmt {
            Statement::Let(l) => self.check_let(l),
            Statement::Assign(a) => self.check_assign(a),
            Statement::Return(r) => self.check_return(r, expected),
            Statement::Expr(e) => {
                self.expr_ty(e);
            }
            Statement::If(i) => self.check_if(i, expected),
            Statement::While(w) => self.check_while(w, expected),
            Statement::For(f) => self.check_for(f, expected),
        }
    }

    fn check_let(&mut self, l: &LetStmt) {
        let value_ty = self.expr_ty(&l.value);
        match &l.ty {
            Some(declared_expr) => {
                let declared = ty_from_expr(declared_expr);
                if !compatible(&declared, &value_ty) {
                    self.error(
                        DiagKind::TypeMismatch,
                        l.span,
                        format!(
                            "invalid type in let \"{}\": expected {}, got {}",
                            l.name.name, declared, value_ty
                        ),
                    );
                }
                self.declare(&l.name.name, declared, l.mutable);
            }
            None => self.declare(&l.name.name, value_ty, l.mutable),
        }
    }

    fn check_assign(&mut self, a: &AssignStmt) {
        let target_ty = self.lvalue_ty(&a.target);
        let value_ty = self.expr_ty(&a.value);

        if !self.is_declared(&a.target.base.name) {
            self.warning(
                DiagKind::UndeclaredVariable,
                a.span,
                format!("undeclared variable \"{}\"", a.target.base.name),
            );
            return;
        }

        // `let` bindings are immutable; mutating them is an error. The value is
        // still type-checked below so the user sees both problems at once.
        if let Some(binding) = self.lookup_binding(&a.target.base.name) {
            if !binding.mutable {
                self.error(
                    DiagKind::AssignToImmutable,
                    a.span,
                    format!(
                        "cannot assign to \"{}\": it was declared with \"let\" (use \"var\" to make it mutable)",
                        a.target.base.name
                    ),
                );
            }
        }

        match a.op {
            AssignOp::Assign => {
                if !compatible(&target_ty, &value_ty) {
                    self.error(
                        DiagKind::TypeMismatch,
                        a.span,
                        format!(
                            "invalid type in assignment to \"{}\": expected {}, got {}",
                            a.target.base.name, target_ty, value_ty
                        ),
                    );
                }
            }
            AssignOp::AddAssign => {
                let ok = target_ty.is_any()
                    || value_ty.is_any()
                    || (target_ty.is_numeric() && value_ty.is_numeric())
                    || (target_ty == Ty::String && value_ty == Ty::String);
                if !ok {
                    self.error(
                        DiagKind::InvalidOperand,
                        a.span,
                        format!("operator += is not valid for {} and {}", target_ty, value_ty),
                    );
                }
            }
            _ => {
                if !(target_ty.is_any()
                    || value_ty.is_any()
                    || (target_ty.is_numeric() && value_ty.is_numeric()))
                {
                    self.error(
                        DiagKind::InvalidOperand,
                        a.span,
                        format!(
                            "arithmetic assignment is not valid for {} and {}",
                            target_ty, value_ty
                        ),
                    );
                }
            }
        }
    }

    fn check_return(&mut self, r: &ReturnStmt, expected: Option<&Ty>) {
        let value_ty = match &r.value {
            Some(e) => self.expr_ty(e),
            None => Ty::Unit,
        };
        // the current function's return type takes priority, so `return` inside match/block is checked too.
        let expected = self.expected_return.as_ref().or(expected);
        if let Some(exp) = expected {
            if !compatible(exp, &value_ty) {
                self.error(
                    DiagKind::TypeMismatch,
                    r.span,
                    format!(
                        "invalid return type: expected {}, got {}",
                        exp, value_ty
                    ),
                );
            }
        }
    }

    fn check_if(&mut self, i: &IfStmt, expected: Option<&Ty>) {
        self.expr_ty(&i.condition);
        self.check_block(&i.then_block, expected);
        match &i.else_branch {
            Some(ElseBranch::ElseIf(nested)) => self.check_if(nested, expected),
            Some(ElseBranch::Else(block)) => self.check_block(block, expected),
            None => {}
        }
    }

    fn check_while(&mut self, w: &WhileStmt, expected: Option<&Ty>) {
        self.expr_ty(&w.condition);
        self.check_block(&w.body, expected);
    }

    fn check_for(&mut self, f: &ForStmt, expected: Option<&Ty>) {
        let iterable_ty = self.expr_ty(&f.iterable);
        let element_ty = match iterable_ty {
            Ty::Array(inner) => *inner,
            _ => Ty::Any,
        };
        self.scopes.push(HashMap::new());
        // The loop variable is a fresh immutable binding, like Rust's `for`.
        self.declare(&f.variable.name, element_ty, false);
        self.check_block(&f.body, expected);
        self.scopes.pop();
    }

    // ---------- lvalue ----------

    fn lvalue_ty(&mut self, lv: &LValue) -> Ty {
        let mut ty = self.lookup(&lv.base.name).cloned().unwrap_or(Ty::Any);
        for step in &lv.path {
            ty = match step {
                LValueStep::Member(id) => match &ty {
                    Ty::Object(fields) => fields
                        .iter()
                        .find(|(k, _)| *k == id.name)
                        .map(|(_, t)| t.clone())
                        .unwrap_or(Ty::Any),
                    _ => Ty::Any,
                },
                LValueStep::Index(e) => {
                    self.expr_ty(e);
                    match &ty {
                        Ty::Array(inner) => (**inner).clone(),
                        _ => Ty::Any,
                    }
                }
            };
        }
        ty
    }

    // ---------- expressions ----------

    fn expr_ty(&mut self, expr: &Expr) -> Ty {
        match expr {
            Expr::Literal(lit) => literal_ty(&lit.value),
            Expr::Identifier(id) => self.identifier_ty(id.name.clone(), id.span),
            Expr::Paren(p) => self.expr_ty(&p.inner),
            Expr::Array(arr) => {
                let mut element: Option<Ty> = None;
                for e in &arr.elements {
                    let t = self.expr_ty(e);
                    element = Some(match element {
                        None => t,
                        Some(prev) => join(prev, t),
                    });
                }
                Ty::Array(Box::new(element.unwrap_or(Ty::Any)))
            }
            Expr::Object(obj) => {
                let fields = obj
                    .fields
                    .iter()
                    .map(|f| (f.key.name.clone(), self.expr_ty(&f.value)))
                    .collect();
                Ty::Object(fields)
            }
            Expr::Binary(b) => {
                let left = self.expr_ty(&b.left);
                let right = self.expr_ty(&b.right);
                self.binary_ty(b.op, left, right, b.span)
            }
            Expr::Unary(u) => {
                let operand = self.expr_ty(&u.operand);
                self.unary_ty(u.op, operand, u.span)
            }
            Expr::Call(call) => self.call_ty(&call.callee, &call.args, call.span),
            Expr::Member(m) => {
                let object = self.expr_ty(&m.object);
                match object {
                    Ty::Object(fields) => fields
                        .into_iter()
                        .find(|(k, _)| *k == m.property.name)
                        .map(|(_, t)| t)
                        .unwrap_or(Ty::Any),
                    _ => Ty::Any,
                }
            }
            Expr::Index(idx) => {
                let object = self.expr_ty(&idx.object);
                self.expr_ty(&idx.index);
                match object {
                    Ty::Array(inner) => *inner,
                    _ => Ty::Any,
                }
            }
            Expr::Await(a) => self.expr_ty(&a.inner),
            Expr::Try(t) => match self.expr_ty(&t.inner) {
                // `ok(v)?` yields `v`
                Ty::Result(ok, _) => *ok,
                // Anything else (including `Any`) stays Any: the checker is lenient.
                _ => Ty::Any,
            },
            Expr::Lambda(l) => {
                // The body is checked with the lambda's own parameters in scope.
                self.scopes.push(HashMap::new());
                for p in &l.params {
                    self.declare(&p.name, Ty::Any, true);
                }
                self.expr_ty(&l.body);
                self.scopes.pop();
                Ty::Function
            }
            Expr::Match(m) => {
                self.expr_ty(&m.scrutinee);
                let mut result: Option<Ty> = None;
                for arm in &m.arms {
                    let t = match &arm.body {
                        MatchBody::Expr(e) => self.expr_ty(e),
                        MatchBody::Block(b) => {
                            self.check_block(b, None);
                            Ty::Any
                        }
                    };
                    result = Some(match result {
                        None => t,
                        Some(prev) => join(prev, t),
                    });
                }
                result.unwrap_or(Ty::Any)
            }
        }
    }

    fn identifier_ty(&mut self, name: String, span: Span) -> Ty {
        if let Some(ty) = self.lookup(&name).cloned() {
            return ty;
        }
        if self.globals.contains(&name) || self.functions.contains_key(&name) {
            return Ty::Any;
        }
        self.warning(
            DiagKind::UndeclaredVariable,
            span,
            format!("undeclared variable \"{}\"", name),
        );
        Ty::Any
    }

    fn binary_ty(&mut self, op: BinaryOp, left: Ty, right: Ty, span: Span) -> Ty {
        use BinaryOp::*;
        if left.is_any() || right.is_any() {
            return match op {
                Eq | Neq | Lt | Gt | Lte | Gte | And | Or => Ty::Bool,
                Add if left == Ty::String || right == Ty::String => Ty::String,
                _ => Ty::Any,
            };
        }
        match op {
            Add => {
                if left == Ty::String && right == Ty::String {
                    Ty::String
                } else if left.is_numeric() && right.is_numeric() {
                    Ty::numeric_result(&left, &right)
                } else {
                    self.error(
                        DiagKind::InvalidOperand,
                        span,
                        format!("operator + cannot be applied to {} and {}", left, right),
                    );
                    Ty::Any
                }
            }
            Sub | Mul | Div | Mod => {
                if left.is_numeric() && right.is_numeric() {
                    Ty::numeric_result(&left, &right)
                } else {
                    self.error(
                        DiagKind::InvalidOperand,
                        span,
                        format!(
                            "operator {} cannot be applied to {} and {}",
                            op.as_str(),
                            left,
                            right
                        ),
                    );
                    Ty::Any
                }
            }
            Lt | Gt | Lte | Gte => {
                if left.is_numeric() && right.is_numeric() {
                    Ty::Bool
                } else {
                    self.error(
                        DiagKind::InvalidOperand,
                        span,
                        format!(
                            "comparison operator {} cannot be applied to {} and {}",
                            op.as_str(),
                            left,
                            right
                        ),
                    );
                    Ty::Bool
                }
            }
            Eq | Neq => Ty::Bool,
            And | Or => {
                if left.is_bool() && right.is_bool() {
                    Ty::Bool
                } else {
                    self.error(
                        DiagKind::InvalidOperand,
                        span,
                        format!(
                            "logical operator {} requires bool, got {} and {}",
                            op.as_str(),
                            left,
                            right
                        ),
                    );
                    Ty::Bool
                }
            }
        }
    }

    fn unary_ty(&mut self, op: UnaryOp, operand: Ty, span: Span) -> Ty {
        match op {
            UnaryOp::Neg => {
                if operand.is_any() {
                    Ty::Any
                } else if operand.is_numeric() {
                    operand
                } else {
                    self.error(
                        DiagKind::InvalidOperand,
                        span,
                        format!("operator - cannot be applied to {}", operand),
                    );
                    Ty::Any
                }
            }
            UnaryOp::Not => {
                if operand.is_any() || operand.is_bool() {
                    Ty::Bool
                } else {
                    self.error(
                        DiagKind::InvalidOperand,
                        span,
                        format!("operator not requires bool, got {}", operand),
                    );
                    Ty::Bool
                }
            }
        }
    }

    fn call_ty(&mut self, callee: &Expr, args: &[aec_ast::Argument], span: Span) -> Ty {
        // check the arguments first so errors inside them are seen.
        let arg_types: Vec<Ty> = args.iter().map(|a| self.expr_ty(&a.value)).collect();

        let name = match callee {
            Expr::Identifier(id) => id.name.clone(),
            _ => {
                self.expr_ty(callee);
                return Ty::Any;
            }
        };

        // user-defined function?
        if let Some(sig) = self.functions.get(&name).map(|s| FuncSigView {
            params: s
                .params
                .iter()
                .map(|p| (p.ty.clone(), p.has_default))
                .collect(),
            ret: s.ret.clone(),
        }) {
            let required = sig.params.iter().filter(|(_, has)| !*has).count();
            if args.len() < required || args.len() > sig.params.len() {
                self.error(
                    DiagKind::ArityMismatch,
                    span,
                    format!(
                        "function \"{}\" expects {} argument(s), got {}",
                        name,
                        sig.params.len(),
                        args.len()
                    ),
                );
                return sig.ret;
            }
            for (i, arg_ty) in arg_types.iter().enumerate() {
                let expected = &sig.params[i].0;
                if !compatible(expected, arg_ty) {
                    self.error(
                        DiagKind::TypeMismatch,
                        args[i].span,
                        format!(
                            "argument {} of function \"{}\" expects {}, got {}",
                            i + 1,
                            name,
                            expected,
                            arg_ty
                        ),
                    );
                }
            }
            return sig.ret;
        }

        // `ok` / `err` are the Result constructors, and `is_ok` / `is_err` the
        // predicates. They are typed here so `?` can be checked.
        match name.as_str() {
            "ok" | "err" | "is_ok" | "is_err" => {
                if args.len() != 1 {
                    self.error(
                        DiagKind::ArityMismatch,
                        span,
                        format!("function \"{}\" expects 1 argument, got {}", name, args.len()),
                    );
                }
                let payload = arg_types.first().cloned().unwrap_or(Ty::Any);
                return match name.as_str() {
                    "ok" => Ty::Result(Box::new(payload), Box::new(Ty::Any)),
                    "err" => Ty::Result(Box::new(Ty::Any), Box::new(payload)),
                    _ => Ty::Bool,
                };
            }
            _ => {}
        }

        // a variable that is clearly not a function?
        if let Some(ty) = self.lookup(&name).cloned() {
            if !ty.is_any() && ty != Ty::Function {
                self.error(
                    DiagKind::NotCallable,
                    span,
                    format!("\"{}\" is of type {} and is not callable", name, ty),
                );
            }
            return Ty::Any;
        }

        // built-in or unknown → dynamic
        Ty::Any
    }

    // ---------- emit ----------

    fn error(&mut self, kind: DiagKind, span: Span, message: impl Into<String>) {
        self.diagnostics
            .push(Diagnostic::error(kind, span, message));
    }

    fn warning(&mut self, kind: DiagKind, span: Span, message: impl Into<String>) {
        self.diagnostics
            .push(Diagnostic::warning(kind, span, message));
    }
}

/// Read-only view of a function signature (to avoid borrow conflicts).
struct FuncSigView {
    params: Vec<(Ty, bool)>,
    ret: Ty,
}

fn literal_ty(lit: &aec_ast::Literal) -> Ty {
    use aec_ast::Literal;
    match lit {
        Literal::String(_) | Literal::RawString(_) | Literal::Interpolated(_) => Ty::String,
        Literal::Int(_) => Ty::Int,
        Literal::Float(_) => Ty::Float,
        Literal::Bool(_) => Ty::Bool,
        Literal::Uuid(_) => Ty::Uuid,
        Literal::None => Ty::None,
        Literal::Duration(_) => Ty::Any,
        Literal::ByteSize(_) => Ty::Bytes,
    }
}

/// Common type of two branches (unknown if either is unknown).
fn join(a: Ty, b: Ty) -> Ty {
    if a == b {
        return a;
    }
    if a.is_any() || b.is_any() {
        return Ty::Any;
    }
    if a.is_numeric() && b.is_numeric() {
        return Ty::numeric_result(&a, &b);
    }
    Ty::Any
}

/// Entry point: checks a program.
pub fn check_program(program: &Program) -> Vec<Diagnostic> {
    Checker::new().check_program(program)
}
