//! Type checker — a single pass over the AST before execution

use crate::diag::{DiagKind, Diagnostic};
use crate::ty::{compatible, ty_from_expr, Ty};
use aec_ast::{
    AssignOp, AssignStmt, BinaryOp, Block, ElseBranch, Expr, ForStmt, FunctionDecl, IfStmt, LValue,
    LValueStep, LetStmt, MatchBody, Program, ReturnStmt, Span, Statement, TopLevelItem, UnaryOp,
    ComponentDecl, ElementExpr, ElementModifier, TypeRef, UiDecl,
    TypeExpr, UiStatement, WhileStmt,
};
use std::collections::{HashMap, HashSet};

/// Namespaces that exist as globals at runtime (`file.read`, `http.get`, ...).
///
/// Keep in sync with the match arms in `aec-runtime`:
/// `interpreter.rs` (`try_builtin`), `stdlib.rs`, `stdlib_extended.rs`.
const BUILTIN_NAMESPACES: &[&str] = &[
    "memory", "env", "file", "http", "json", "time", "math", "sys", "crypto", "regex", "shell",
    "uuid", "llm", "theme", "secrets",
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
    "map",
    "filter",
    "reduce",
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
    "math_tau",
    "math_log10",
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

fn scoped_name(scope: Option<&str>, name: &str) -> String {
    scope
        .map(|scope| format!("{}::{}", scope, name))
        .unwrap_or_else(|| name.to_string())
}

fn builtin_arity(name: &str) -> Option<(usize, Option<usize>)> {
    let arity = match name {
        "print" | "read_line" => (0, None),
        "len" | "str" | "int" | "float" | "bool" | "upper" | "lower" | "trim" | "abs" | "sqrt"
        | "sort" | "reverse" | "first" | "last" | "pop" | "keys" | "values" | "md5"
        | "sha256" | "sha512" | "base64_encode" | "base64_decode" | "b64_encode"
        | "b64_decode" | "file.read" | "file_read" | "file.exists" | "file_exists"
        | "file.delete" | "file_delete" | "file.list_dir" | "file_list_dir" | "ls"
        | "file.mkdir" | "file_mkdir" | "file.size" | "file_size" | "env.get" | "env_get"
        | "http.get" | "http_get" | "http.delete" | "http_delete" | "json.parse"
        | "json_parse" | "json.stringify" | "json_stringify" | "time.sleep" | "time_sleep"
        | "sleep" | "crypto.md5" | "crypto_md5" | "crypto.sha256" | "crypto_sha256"
        | "crypto.sha512" | "crypto_sha512" | "crypto.base64_encode"
        | "crypto.base64_decode" => (1, Some(1)),
        "split" | "join" | "contains" | "starts_with" | "ends_with" | "push" | "min"
        | "max" | "pow" | "has" | "repeat" | "char_at" | "math.random_int" | "random_int"
        | "file.write" | "file_write" | "file.append" | "file_append" | "env.set" | "env_set"
        | "http.post" | "http_post" | "http.put" | "http_put" | "file.copy" | "file_copy"
        | "regex.match" | "regex_match" | "regex.find" | "regex_find"
        | "regex.find_all" | "regex_find_all" => (2, Some(2)),
        "ok" | "err" | "is_ok" | "is_err" | "llm.complete" | "llm_complete"
        | "memory.open" | "memory_open" | "memory.get" | "memory_get" | "memory.clear"
        | "memory_clear" | "memory.count" | "memory_count" => (1, Some(1)),
        "memory.add" | "memory_add" | "replace" | "regex.replace" | "regex_replace" => (3, Some(3)),
        "range" => (1, Some(2)),
        "map" | "filter" => (2, Some(2)),
        "reduce" => (3, Some(3)),
        "slice" => (2, Some(3)),
        "sys.exit" | "sys_exit" | "exit" => (0, Some(1)),
        "time.now_ms" | "time_now_ms" | "now_ms" | "time.now_sec" | "time_now_sec" | "now"
        | "sys.args" | "sys_args" | "args" | "sys.info" | "sys_info" | "sys.env_all"
        | "sys_env_all" | "uuid.v4" | "uuid_v4" | "uuid" | "math.pi" | "math_pi" | "pi"
        | "math.e" | "math_e" | "e" | "math.tau" | "math_tau" | "math.random"
        | "math_random" | "random" => (0, Some(0)),
        "math.sin" | "math_sin" | "sin" | "math.cos" | "math_cos" | "cos" | "math.tan"
        | "math_tan" | "tan" | "math.log" | "math_log" | "log" | "math.log10" | "log10"
        | "math.exp" | "math_exp" | "exp" | "math.floor" | "math_floor" | "floor"
        | "math.ceil" | "math_ceil" | "ceil" | "math.round" | "math_round" | "round" => {
            (1, Some(1))
        }
        _ => return None,
    };
    Some(arity)
}

#[derive(Clone)]
struct ParamSig {
    name: String,
    ty: Ty,
    has_default: bool,
}

/// A name bound in a local scope.
struct Binding {
    ty: Ty,
    /// `false` for `let` — assigning to it is an error.
    mutable: bool,
}

#[derive(Clone)]
struct FuncSig {
    params: Vec<ParamSig>,
    ret: Ty,
}

#[derive(Clone)]
struct ComponentSig {
    props: Vec<(String, Ty)>,
}

/// Type checker for a program. Use it via `check_program`.
pub struct Checker {
    functions: HashMap<String, FuncSig>,
    globals: HashSet<String>,
    local_globals: HashSet<String>,
    private_globals: HashSet<String>,
    type_aliases: HashMap<String, TypeExpr>,
    type_alias_scopes: HashMap<String, Option<String>>,
    resolved_type_aliases: HashMap<String, Ty>,
    module_aliases: HashSet<String>,
    components: HashMap<String, ComponentSig>,
    diagnostics: Vec<Diagnostic>,
    scopes: Vec<HashMap<String, Binding>>,
    current_module: Option<String>,
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
            local_globals: HashSet::new(),
            private_globals: HashSet::new(),
            type_aliases: HashMap::new(),
            type_alias_scopes: HashMap::new(),
            resolved_type_aliases: HashMap::new(),
            module_aliases: HashSet::new(),
            components: HashMap::new(),
            diagnostics: Vec::new(),
            scopes: Vec::new(),
            current_module: None,
            expected_return: None,
        }
    }

    /// Checks the program and returns the diagnostics (errors + warnings).
    pub fn check_program(self, program: &Program) -> Vec<Diagnostic> {
        self.check_program_with_origins(program)
            .into_iter()
            .map(|(_, diag)| diag)
            .collect()
    }

    /// Returns diagnostics with the index of the top-level item that produced each one.
    pub fn check_program_with_origins(mut self, program: &Program) -> Vec<(usize, Diagnostic)> {
        self.collect_type_aliases(program);
        self.collect_globals(program);
        self.collect_functions(program);

        let mut indexed = Vec::new();
        for (index, item) in program.items.iter().enumerate() {
            self.current_module = program.item_modules.get(index).cloned().flatten();
            match item {
                TopLevelItem::Function(function) => self.check_function(function),
                TopLevelItem::Component(component) => self.check_component(component),
                TopLevelItem::Ui(ui) => self.check_ui(ui),
                _ => {}
            }
            indexed.extend(self.diagnostics.drain(..).map(|diag| (index, diag)));
        }
        self.current_module = None;

        indexed
    }

    // ---------- collection ----------

    fn collect_type_aliases(&mut self, program: &Program) {
        for (index, item) in program.items.iter().enumerate() {
            let TopLevelItem::TypeAlias(alias) = item else {
                continue;
            };
            let module = program.item_modules.get(index).cloned().flatten();
            let local_key = scoped_name(module.as_deref(), &alias.name.name);
            self.type_aliases
                .insert(local_key.clone(), alias.target.clone());
            self.type_alias_scopes
                .insert(local_key, module.clone());
            if let Some(scope) = module.as_deref() {
                if alias.is_public {
                    let public_key = format!("{}.{}", scope, alias.name.name);
                    self.type_aliases
                        .insert(public_key.clone(), alias.target.clone());
                    self.type_alias_scopes.insert(public_key, module.clone());
                }
            }
        }
    }

    fn resolve_type_expr(&mut self, expr: &TypeExpr) -> Ty {
        let scope = self.current_module.clone();
        self.resolve_type_expr_in_scope(expr, scope.as_deref())
    }

    fn resolve_type_expr_in_scope(&mut self, expr: &TypeExpr, scope: Option<&str>) -> Ty {
        match expr {
            TypeExpr::Named(identifier) => {
                let local_key = scoped_name(scope, &identifier.name);
                let mut visiting = HashSet::new();
                if self.type_aliases.contains_key(&local_key) {
                    return self.resolve_type_alias(&local_key, &mut visiting);
                }
                if scope.is_some() && self.type_aliases.contains_key(&identifier.name) {
                    return self.resolve_type_alias(&identifier.name, &mut visiting);
                }
                Ty::Any
            }
            TypeExpr::Optional(inner) => Ty::Optional(Box::new(
                self.resolve_type_expr_in_scope(inner, scope),
            )),
            TypeExpr::Array(inner) => Ty::Array(Box::new(
                self.resolve_type_expr_in_scope(inner, scope),
            )),
            TypeExpr::Result(ok, err) => Ty::Result(
                Box::new(self.resolve_type_expr_in_scope(ok, scope)),
                Box::new(self.resolve_type_expr_in_scope(err, scope)),
            ),
            other => ty_from_expr(other),
        }
    }

    fn resolve_type_alias(&mut self, key: &str, visiting: &mut HashSet<String>) -> Ty {
        if let Some(ty) = self.resolved_type_aliases.get(key) {
            return ty.clone();
        }
        if !visiting.insert(key.to_string()) {
            return Ty::Any;
        }
        let Some(target) = self.type_aliases.get(key).cloned() else {
            return Ty::Any;
        };
        let scope = self.type_alias_scopes.get(key).cloned().flatten();
        let ty = self.resolve_type_expr_in_scope(&target, scope.as_deref());
        self.resolved_type_aliases.insert(key.to_string(), ty.clone());
        visiting.remove(key);
        ty
    }

    fn collect_globals(&mut self, program: &Program) {
        for import in &program.imports {
            if let Some(alias) = &import.alias {
                self.module_aliases.insert(alias.clone());
                self.globals.insert(alias.clone());
            }
        }
        for name in BUILTIN_NAMESPACES {
            self.globals.insert((*name).to_string());
        }
        for name in BUILTIN_FUNCTIONS {
            self.globals.insert((*name).to_string());
        }
        for (index, item) in program.items.iter().enumerate() {
            let module = program.item_modules.get(index).cloned().flatten();
            match item {
                TopLevelItem::Model(model) => {
                    if let Some(scope) = &module {
                        self.local_globals.insert(scoped_name(Some(scope), &model.name.name));
                        if model.is_public {
                            self.globals.insert(format!("{}.{}", scope, model.name.name));
                        } else {
                            self.private_globals.insert(model.name.name.clone());
                        }
                    } else {
                        self.globals.insert(model.name.name.clone());
                    }
                }
                TopLevelItem::Component(component) => {
                    let key = if let Some(scope) = &module {
                        if component.is_public {
                            format!("{}.{}", scope, component.name.name)
                        } else {
                            scoped_name(Some(scope), &component.name.name)
                        }
                    } else {
                        component.name.name.clone()
                    };
                    self.components.insert(
                        key,
                        ComponentSig {
                            props: component
                                .props
                                .iter()
                                .map(|prop| {
                                    (
                                        prop.name.name.clone(),
                                        prop.ty.as_ref().map(type_ref_ty).unwrap_or(Ty::Any),
                                    )
                                })
                                .collect(),
                        },
                    );
                }
                TopLevelItem::Ui(ui) if module.is_none() => self.collect_ui_states(&ui.screen.body),
                _ => {}
            }
        }
    }

    fn collect_ui_states(&mut self, statements: &[UiStatement]) {
        for statement in statements {
            match statement {
                UiStatement::State(state) => {
                    self.globals.insert(state.name.name.clone());
                }
                UiStatement::Element(element) => {
                    if let Some(children) = &element.children {
                        self.collect_ui_states(children);
                    }
                }
                UiStatement::If(branch) => {
                    self.collect_ui_states(&branch.then_body);
                    if let Some(otherwise) = &branch.else_body {
                        self.collect_ui_states(otherwise);
                    }
                }
                UiStatement::For(_) | UiStatement::Component(_) => {}
            }
        }
    }

    fn collect_functions(&mut self, program: &Program) {
        for (index, item) in program.items.iter().enumerate() {
            if let TopLevelItem::Function(f) = item {
                let module = program.item_modules.get(index).cloned().flatten();
                let params = f
                    .params
                    .iter()
                    .map(|p| ParamSig {
                        name: p.name.name.clone(),
                        ty: self.resolve_type_expr_in_scope(&p.ty, module.as_deref()),
                        has_default: p.default.is_some(),
                    })
                    .collect();
                let ret = f.return_type.as_ref().map(ty_from_expr).unwrap_or(Ty::Unit);
                let signature = FuncSig { params, ret };
                self.functions
                    .insert(scoped_name(module.as_deref(), &f.name.name), signature.clone());
                if let Some(scope) = module {
                    if f.is_public {
                        self.functions
                            .insert(format!("{}.{}", scope, f.name.name), signature);
                    }
                }
            }
        }
    }

    fn check_function(&mut self, f: &FunctionDecl) {
        let function_name = scoped_name(self.current_module.as_deref(), &f.name.name);
        let expected = self.return_type_of(&function_name).unwrap_or(Ty::Unit);

        self.expected_return = Some(expected.clone());
        self.scopes.push(HashMap::new());
        for p in &f.params {
            let parameter_type = self.resolve_type_expr(&p.ty);
            self.declare(&p.name.name, parameter_type.clone(), true);
            if let Some(default) = &p.default {
                let default_type = self.expr_ty(default);
                if !compatible(&parameter_type, &default_type) {
                    self.error(
                        DiagKind::TypeMismatch,
                        default.span(),
                        format!(
                            "default value for \"{}\" expects {}, got {}",
                            p.name.name, parameter_type, default_type
                        ),
                    );
                }
            }
        }
        self.check_block(&f.body, Some(&expected));
        self.scopes.pop();
    }

    fn check_component(&mut self, component: &ComponentDecl) {
        let mut names = HashSet::new();
        for prop in &component.props {
            if !names.insert(prop.name.name.clone()) {
                self.error(
                    DiagKind::DuplicateDeclaration,
                    prop.span,
                    format!("duplicate component prop \"{}\"", prop.name.name),
                );
            }
        }
        if let Some(body) = &component.render {
            self.scopes.push(HashMap::new());
            for prop in &component.props {
                self.declare(
                    &prop.name.name,
                    prop.ty.as_ref().map(type_ref_ty).unwrap_or(Ty::Any),
                    false,
                );
            }
            self.check_ui_statements(body);
            self.scopes.pop();
        } else {
            self.error(
                DiagKind::InvalidUi,
                component.span,
                format!("component \"{}\" has no render block", component.name.name),
            );
        }
    }

    fn check_ui(&mut self, ui: &UiDecl) {
        self.scopes.push(HashMap::new());
        self.check_ui_statements(&ui.screen.body);
        self.scopes.pop();
    }

    fn check_ui_statements(&mut self, statements: &[UiStatement]) {
        for statement in statements {
            if let UiStatement::State(state) = statement {
                let actual = self.expr_ty(&state.initial);
                let declared = state.ty.as_ref().map(type_ref_ty);
                if let Some(expected) = &declared {
                    if !compatible(expected, &actual) {
                        self.error(
                            DiagKind::TypeMismatch,
                            state.span,
                            format!(
                                "invalid type for UI state \"{}\": expected {}, got {}",
                                state.name.name, expected, actual
                            ),
                        );
                    }
                }
                if self.lookup_binding(&state.name.name).is_some() {
                    self.error(
                        DiagKind::DuplicateDeclaration,
                        state.span,
                        format!("duplicate UI state \"{}\"", state.name.name),
                    );
                } else {
                    self.declare(
                        &state.name.name,
                        declared.unwrap_or(actual),
                        true,
                    );
                }
            }
        }

        for statement in statements {
            match statement {
                UiStatement::State(_) => {}
                UiStatement::Element(element) => self.check_ui_element(element),
                UiStatement::If(branch) => {
                    let condition = self.expr_ty(&branch.condition);
                    if !condition.is_any() && condition != Ty::Bool {
                        self.error(
                            DiagKind::InvalidOperand,
                            branch.condition.span(),
                            format!("UI condition must be bool, got {}", condition),
                        );
                    }
                    self.scopes.push(HashMap::new());
                    self.check_ui_statements(&branch.then_body);
                    self.scopes.pop();
                    if let Some(otherwise) = &branch.else_body {
                        self.scopes.push(HashMap::new());
                        self.check_ui_statements(otherwise);
                        self.scopes.pop();
                    }
                }
                UiStatement::For(loop_ui) => {
                    let iterable = self.expr_ty(&loop_ui.iterable);
                    let element = match iterable {
                        Ty::Array(inner) => *inner,
                        Ty::Any => Ty::Any,
                        other => {
                            self.error(
                                DiagKind::InvalidOperand,
                                loop_ui.iterable.span(),
                                format!("UI for loop needs an array, got {}", other),
                            );
                            Ty::Any
                        }
                    };
                    self.scopes.push(HashMap::new());
                    self.declare(&loop_ui.variable.name, element, false);
                    self.check_ui_statements(&loop_ui.body);
                    self.scopes.pop();
                }
                UiStatement::Component(component_use) => {
                    let signature = if component_use.name.name.contains('.') {
                        self.current_module
                            .as_deref()
                            .and_then(|scope| {
                                self.components
                                    .get(&format!("{}.{}", scope, component_use.name.name))
                            })
                            .or_else(|| self.components.get(&component_use.name.name))
                            .cloned()
                    } else {
                        self.current_module
                            .as_deref()
                            .and_then(|scope| {
                                self.components
                                    .get(&scoped_name(Some(scope), &component_use.name.name))
                            })
                            .or_else(|| self.components.get(&component_use.name.name))
                            .cloned()
                    };
                    let Some(signature) = signature else {
                        self.error(
                            DiagKind::InvalidUi,
                            component_use.span,
                            format!("unknown component \"{}\"", component_use.name.name),
                        );
                        continue;
                    };
                    let mut provided = HashMap::new();
                    for property in &component_use.props {
                        if provided.insert(property.name.name.clone(), property.value.clone()).is_some() {
                            self.error(
                                DiagKind::InvalidUi,
                                property.span,
                                format!("duplicate prop \"{}\"", property.name.name),
                            );
                            continue;
                        }
                        let Some((_, expected)) = signature
                            .props
                            .iter()
                            .find(|(name, _)| *name == property.name.name)
                        else {
                            self.error(
                                DiagKind::InvalidUi,
                                property.span,
                                format!("unknown prop \"{}\" on component \"{}\"", property.name.name, component_use.name.name),
                            );
                            continue;
                        };
                        let actual = self.expr_ty(&property.value);
                        if !compatible(expected, &actual) {
                            self.error(
                                DiagKind::TypeMismatch,
                                property.span,
                                format!(
                                    "prop \"{}\" expects {}, got {}",
                                    property.name.name, expected, actual
                                ),
                            );
                        }
                    }
                    for (name, _) in &signature.props {
                        if !provided.contains_key(name) {
                            self.error(
                                DiagKind::InvalidUi,
                                component_use.span,
                                format!("missing prop \"{}\" on component \"{}\"", name, component_use.name.name),
                            );
                        }
                    }
                }
            }
        }
    }

    fn check_ui_element(&mut self, element: &ElementExpr) {
        const ELEMENTS: &[&str] = &[
            "Column", "Row", "Text", "Display", "Input", "Button", "Card", "Divider",
            "Heading", "Spacer", "Messages",
        ];
        if !ELEMENTS.contains(&element.name.name.as_str()) {
            self.error(
                DiagKind::InvalidUi,
                element.name.span,
                format!("unknown UI element \"{}\"", element.name.name),
            );
        }
        if let Some(argument) = &element.primary_arg {
            let _ = self.expr_ty(argument);
        }
        for modifier in &element.modifiers {
            match modifier {
                ElementModifier::Binding(binding) => {
                    if !self.is_declared(&binding.target.name) {
                        self.error(
                            DiagKind::InvalidUi,
                            binding.span,
                            format!("UI binding target \"{}\" is not declared", binding.target.name),
                        );
                    } else if let Some(ty) = self.lookup(&binding.target.name) {
                        if ty != &Ty::String && !ty.is_any() {
                            self.error(
                                DiagKind::TypeMismatch,
                                binding.span,
                                format!("Input binding \"{}\" requires string, got {}", binding.target.name, ty),
                            );
                        }
                    }
                }
                ElementModifier::Event(event) => {
                    if event.event.name != "click" {
                        self.error(
                            DiagKind::InvalidUi,
                            event.span,
                            format!("unsupported UI event \"{}\"", event.event.name),
                        );
                    }
                    if !matches!(event.handler, Expr::Call(_)) {
                        self.error(
                            DiagKind::InvalidUi,
                            event.handler.span(),
                            "UI event handler must be a function call".to_string(),
                        );
                    } else {
                        let _ = self.expr_ty(&event.handler);
                    }
                }
                ElementModifier::Property(property) => {
                    let _ = self.expr_ty(&property.value);
                }
            }
        }
        if let Some(children) = &element.children {
            self.scopes.push(HashMap::new());
            self.check_ui_statements(children);
            self.scopes.pop();
        }
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
                let declared = self.resolve_type_expr(declared_expr);
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
            Ty::Any => Ty::Any,
            other => {
                self.error(
                    DiagKind::InvalidOperand,
                    f.iterable.span(),
                    format!("for loop needs an array, got {}", other),
                );
                Ty::Any
            }
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
            Expr::Literal(lit) => {
                if let aec_ast::Literal::Interpolated(parts) = &lit.value {
                    for part in parts {
                        if let aec_ast::InterpPart::Expr(expression) = part {
                            self.expr_ty(expression);
                        }
                    }
                }
                literal_ty(&lit.value)
            }
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
                    Ty::Object(fields) => {
                        match fields.into_iter().find(|(key, _)| *key == m.property.name) {
                            Some((_, ty)) => ty,
                            None => {
                                self.error(
                                    DiagKind::InvalidOperand,
                                    m.property.span,
                                    format!("object has no field \"{}\"", m.property.name),
                                );
                                Ty::Any
                            }
                        }
                    }
                    Ty::Any => Ty::Any,
                    other => {
                        self.error(
                            DiagKind::InvalidOperand,
                            m.span,
                            format!("cannot access .{} on {}", m.property.name, other),
                        );
                        Ty::Any
                    }
                }
            }
            Expr::Index(idx) => {
                let object = self.expr_ty(&idx.object);
                self.expr_ty(&idx.index);
                match object {
                    Ty::Array(inner) => *inner,
                    Ty::String => Ty::String,
                    Ty::Object(_) | Ty::Any => Ty::Any,
                    other => {
                        self.error(
                            DiagKind::NotIndexable,
                            idx.span,
                            format!("cannot index {}", other),
                        );
                        Ty::Any
                    }
                }
            }
            Expr::Await(a) => {
                self.error(
                    DiagKind::UnsupportedFeature,
                    a.span,
                    "await is not supported by the synchronous runtime".to_string(),
                );
                self.expr_ty(&a.inner)
            }
            Expr::Try(t) => match self.expr_ty(&t.inner) {
                Ty::Result(ok, _) => *ok,
                other if other.is_any() => Ty::Any,
                other => {
                    self.error(
                        DiagKind::TypeMismatch,
                        t.span,
                        format!("operator '?' requires a Result, got {}", other),
                    );
                    Ty::Any
                }
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
        if let Some(scope) = self.current_module.as_deref() {
            let local = scoped_name(Some(scope), &name);
            if self.local_globals.contains(&local) || self.functions.contains_key(&local) {
                return Ty::Any;
            }
        }
        if self.private_globals.contains(&name) {
            self.error(
                DiagKind::UnknownFunction,
                span,
                format!("\"{}\" is private to its module", name),
            );
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

    fn resolve_call_name(&self, name: &str) -> String {
        if let Some(scope) = self.current_module.as_deref() {
            if !name.contains('.') {
                let local = scoped_name(Some(scope), name);
                if self.functions.contains_key(&local) {
                    return local;
                }
            } else {
                let relative = format!("{}.{}", scope, name);
                if self.functions.contains_key(&relative) {
                    return relative;
                }
            }
        }
        if self.functions.contains_key(name) {
            return name.to_string();
        }
        name.to_string()
    }

    fn call_ty(&mut self, callee: &Expr, args: &[aec_ast::Argument], span: Span) -> Ty {
        // check the arguments first so errors inside them are seen.
        let arg_types: Vec<Ty> = args.iter().map(|a| self.expr_ty(&a.value)).collect();

        let name = match callee {
            Expr::Identifier(id) => id.name.clone(),
            Expr::Member(member) => match &member.object {
                Expr::Identifier(namespace) => {
                    format!("{}.{}", namespace.name, member.property.name)
                }
                _ => {
                    self.expr_ty(callee);
                    return Ty::Any;
                }
            },
            _ => {
                self.expr_ty(callee);
                return Ty::Any;
            }
        };
        let lookup_name = self.resolve_call_name(&name);

        // user-defined function?
        if let Some(sig) = self.functions.get(&lookup_name).map(|s| FuncSigView {
            params: s
                .params
                .iter()
                .map(|p| (p.name.clone(), p.ty.clone(), p.has_default))
                .collect(),
            ret: s.ret.clone(),
        }) {
            let required = sig.params.iter().filter(|(_, _, has)| !*has).count();
            if args.len() < required || args.len() > sig.params.len() {
                self.error(
                    DiagKind::ArityMismatch,
                    span,
                    format!(
                        "function \"{}\" expects at least {} and at most {} arguments, got {}",
                        name,
                        required,
                        sig.params.len(),
                        args.len()
                    ),
                );
                return sig.ret;
            }
            let mut used = vec![false; sig.params.len()];
            let mut positional = 0;
            let mut named_seen = false;
            for (argument, argument_ty) in args.iter().zip(arg_types.iter()) {
                let index = if let Some(argument_name) = &argument.name {
                    named_seen = true;
                    let Some(index) = sig
                        .params
                        .iter()
                        .position(|(parameter_name, _, _)| parameter_name == &argument_name.name)
                    else {
                        self.error(
                            DiagKind::ArityMismatch,
                            argument.span,
                            format!("unknown argument \"{}\" for function \"{}\"", argument_name.name, name),
                        );
                        continue;
                    };
                    index
                } else {
                    if named_seen {
                        self.error(
                            DiagKind::ArityMismatch,
                            argument.span,
                            "positional arguments cannot follow named arguments".to_string(),
                        );
                        continue;
                    }
                    let index = positional;
                    positional += 1;
                    index
                };
                if index >= sig.params.len() {
                    self.error(
                        DiagKind::ArityMismatch,
                        argument.span,
                        format!("too many arguments for function \"{}\"", name),
                    );
                    continue;
                }
                if used[index] {
                    self.error(
                        DiagKind::ArityMismatch,
                        argument.span,
                        format!("argument \"{}\" was provided more than once", sig.params[index].0),
                    );
                    continue;
                }
                used[index] = true;
                let expected = &sig.params[index].1;
                if !compatible(expected, argument_ty) {
                    self.error(
                        DiagKind::TypeMismatch,
                        argument.span,
                        format!(
                            "argument \"{}\" of function \"{}\" expects {}, got {}",
                            sig.params[index].0, name, expected, argument_ty
                        ),
                    );
                }
            }
            for (index, provided) in used.iter().enumerate() {
                if !provided && !sig.params[index].2 {
                    self.error(
                        DiagKind::ArityMismatch,
                        span,
                        format!("missing argument \"{}\" for function \"{}\"", sig.params[index].0, name),
                    );
                }
            }
            return sig.ret;
        }

        // `ok` / `err` are the Result constructors, and `is_ok` / `is_err` the
        // predicates. They are typed here so `?` can be checked.
        match lookup_name.as_str() {
            "ok" | "err" | "is_ok" | "is_err" => {
                if args.len() != 1 {
                    self.error(
                        DiagKind::ArityMismatch,
                        span,
                        format!("function \"{}\" expects 1 argument, got {}", name, args.len()),
                    );
                }
                let payload = arg_types.first().cloned().unwrap_or(Ty::Any);
                return match lookup_name.as_str() {
                    "ok" => Ty::Result(Box::new(payload), Box::new(Ty::Any)),
                    "err" => Ty::Result(Box::new(Ty::Any), Box::new(payload)),
                    _ => Ty::Bool,
                };
            }
            _ => {}
        }

        if let Some((minimum, maximum)) = builtin_arity(&lookup_name) {
            if args.len() < minimum || maximum.is_some_and(|maximum| args.len() > maximum) {
                self.error(
                    DiagKind::ArityMismatch,
                    span,
                    format!(
                        "builtin \"{}\" expects {} argument(s), got {}",
                        name,
                        minimum,
                        args.len()
                    ),
                );
            }
            return builtin_return_ty(&lookup_name);
        }

        if let Some(ty) = self.lookup(&lookup_name).cloned() {
            if !ty.is_any() && ty != Ty::Function {
                self.error(
                    DiagKind::NotCallable,
                    span,
                    format!("\"{}\" is of type {} and is not callable", name, ty),
                );
            }
            return Ty::Any;
        }

        if self.globals.contains(&lookup_name) {
            return Ty::Any;
        }
        if let Some((module, _)) = lookup_name.split_once('.') {
            if self.module_aliases.contains(module) {
                self.error(
                    DiagKind::UnknownFunction,
                    span,
                    format!("module \"{}\" has no export \"{}\"", module, lookup_name),
                );
            }
            return Ty::Any;
        }
        self.error(
            DiagKind::UnknownFunction,
            span,
            format!("unknown function \"{}\"", name),
        );
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
    params: Vec<(String, Ty, bool)>,
    ret: Ty,
}

fn builtin_return_ty(name: &str) -> Ty {
    match name {
        "len" | "count" | "int" | "random_int" | "math_random_int" | "now" | "now_ms"
        | "time_now_ms" | "time_now_sec" | "math.floor" | "math_floor" | "floor"
        | "math.ceil" | "math_ceil" | "ceil" | "math.round" | "math_round" | "round" => Ty::Int,
        "float" | "math.sin" | "math_sin" | "sin" | "math.cos" | "math_cos" | "cos"
        | "math.tan" | "math_tan" | "tan" | "math.log" | "math_log" | "log"
        | "math.log10" | "log10" | "math.exp" | "math_exp" | "exp" | "math.pi" | "math_pi"
        | "pi" | "math.e" | "math_e" | "e" | "math.tau" | "math_tau" | "sqrt"
        | "math.random" | "math_random" | "random" => Ty::Float,
        "contains" | "starts_with" | "ends_with" | "has" | "is_ok" | "is_err" | "bool" => Ty::Bool,
        "str" | "upper" | "lower" | "trim" | "split" | "join" | "replace" | "repeat"
        | "char_at" | "json.stringify" | "json_stringify" | "md5" | "sha256" | "sha512"
        | "base64_encode" | "base64_decode" | "b64_encode" | "b64_decode" | "crypto.md5"
        | "crypto.sha256" | "crypto.sha512" | "crypto.base64_encode" | "crypto.base64_decode"
        | "regex.find" | "regex_find" | "regex.replace" | "regex_replace" => Ty::String,
        "range" | "regex.find_all" | "regex_find_all" | "keys" | "values" => {
            Ty::Array(Box::new(if name == "range" { Ty::Int } else { Ty::Any }))
        }
        "ok" => Ty::Result(Box::new(Ty::Any), Box::new(Ty::Any)),
        "err" => Ty::Result(Box::new(Ty::Any), Box::new(Ty::Any)),
        "map" | "filter" => Ty::Array(Box::new(Ty::Any)),
        "reduce" => Ty::Any,
        "sort" | "reverse" | "slice" | "push" | "file.list_dir" | "file_list_dir" | "ls" => {
            Ty::Any
        }
        "llm.complete" | "llm_complete" => Ty::Object(Vec::new()),
        _ => Ty::Any,
    }
}

fn type_ref_ty(type_ref: &TypeRef) -> Ty {
    match type_ref {
        TypeRef::String => Ty::String,
        TypeRef::Int => Ty::Int,
        TypeRef::Float => Ty::Float,
        TypeRef::Bool => Ty::Bool,
        TypeRef::Array(inner) => Ty::Array(Box::new(type_ref_ty(inner))),
        TypeRef::Named(_) => Ty::Any,
    }
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
