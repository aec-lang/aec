//! Type checker — a single pass over the AST before execution

use crate::diag::{DiagKind, Diagnostic};
use crate::ty::{compatible, ty_from_expr, Ty};
use aec_ast::{
    AssignOp, AssignStmt, BinaryOp, Block, ComponentDecl, ElementExpr, ElementModifier, ElseBranch,
    Expr, ForStmt, FunctionDecl, IfStmt, LValue, LValueStep, LetStmt, MatchBody, Pattern, Program,
    ReturnStmt, Span, Statement, TopLevelItem, TypeExpr, TypeRef, UiDecl, UiStatement, UnaryOp,
    WhileStmt,
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
        "len"
        | "str"
        | "int"
        | "float"
        | "bool"
        | "upper"
        | "lower"
        | "trim"
        | "abs"
        | "sqrt"
        | "sort"
        | "reverse"
        | "first"
        | "last"
        | "pop"
        | "keys"
        | "values"
        | "md5"
        | "sha256"
        | "sha512"
        | "base64_encode"
        | "base64_decode"
        | "b64_encode"
        | "b64_decode"
        | "file.read"
        | "file_read"
        | "file.exists"
        | "file_exists"
        | "file.delete"
        | "file_delete"
        | "file.list_dir"
        | "file_list_dir"
        | "ls"
        | "file.mkdir"
        | "file_mkdir"
        | "file.size"
        | "file_size"
        | "env.get"
        | "env_get"
        | "http.get"
        | "http_get"
        | "http.delete"
        | "http_delete"
        | "json.parse"
        | "json_parse"
        | "json.stringify"
        | "json_stringify"
        | "time.sleep"
        | "time_sleep"
        | "sleep"
        | "crypto.md5"
        | "crypto_md5"
        | "crypto.sha256"
        | "crypto_sha256"
        | "crypto.sha512"
        | "crypto_sha512"
        | "crypto.base64_encode"
        | "crypto.base64_decode" => (1, Some(1)),
        "split" | "join" | "contains" | "starts_with" | "ends_with" | "push" | "min" | "max"
        | "pow" | "has" | "repeat" | "char_at" | "math.random_int" | "random_int"
        | "file.write" | "file_write" | "file.append" | "file_append" | "env.set" | "env_set"
        | "http.post" | "http_post" | "http.put" | "http_put" | "file.copy" | "file_copy"
        | "regex.match" | "regex_match" | "regex.find" | "regex_find" | "regex.find_all"
        | "regex_find_all" => (2, Some(2)),
        "ok" | "err" | "is_ok" | "is_err" | "llm.complete" | "llm_complete" | "memory.open"
        | "memory_open" | "memory.get" | "memory_get" | "memory.clear" | "memory_clear"
        | "memory.count" | "memory_count" => (1, Some(1)),
        "memory.add" | "memory_add" | "replace" | "regex.replace" | "regex_replace" => (3, Some(3)),
        "range" => (1, Some(2)),
        "map" | "filter" => (2, Some(2)),
        "reduce" => (3, Some(3)),
        "slice" => (2, Some(3)),
        "sys.exit" | "sys_exit" | "exit" => (0, Some(1)),
        "time.now_ms" | "time_now_ms" | "now_ms" | "time.now_sec" | "time_now_sec" | "now"
        | "sys.args" | "sys_args" | "args" | "sys.info" | "sys_info" | "sys.env_all"
        | "sys_env_all" | "uuid.v4" | "uuid_v4" | "uuid" | "math.pi" | "math_pi" | "pi"
        | "math.e" | "math_e" | "e" | "math.tau" | "math_tau" | "math.random" | "math_random"
        | "random" => (0, Some(0)),
        "math.sin" | "math_sin" | "sin" | "math.cos" | "math_cos" | "cos" | "math.tan"
        | "math_tan" | "tan" | "math.log" | "math_log" | "log" | "math.log10" | "log10"
        | "math.exp" | "math_exp" | "exp" | "math.floor" | "math_floor" | "floor" | "math.ceil"
        | "math_ceil" | "ceil" | "math.round" | "math_round" | "round" => (1, Some(1)),
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

#[derive(Clone)]
struct StructSig {
    fields: Vec<(String, Ty)>,
}

#[derive(Clone)]
struct EnumSig {
    variants: Vec<(String, Option<Ty>)>,
}

#[derive(Clone)]
enum TypeDeclaration {
    Alias {
        target: TypeExpr,
        scope: Option<String>,
    },
    Struct {
        fields: Vec<(String, TypeExpr)>,
        scope: Option<String>,
    },
    Enum {
        variants: Vec<(String, Option<TypeExpr>)>,
        scope: Option<String>,
    },
}

enum NamedTypeLookup {
    Declared(String),
    Private,
    Missing,
}

struct TypeRegistry {
    declarations: HashMap<String, TypeDeclaration>,
    public_names: HashMap<String, String>,
    private_names: HashMap<String, String>,
    resolved: HashMap<String, Ty>,
    structs: HashMap<String, StructSig>,
    enums: HashMap<String, EnumSig>,
}

impl TypeRegistry {
    fn new() -> Self {
        Self {
            declarations: HashMap::new(),
            public_names: HashMap::new(),
            private_names: HashMap::new(),
            resolved: HashMap::new(),
            structs: HashMap::new(),
            enums: HashMap::new(),
        }
    }

    fn insert(
        &mut self,
        key: String,
        name: &str,
        module: Option<&str>,
        is_public: bool,
        declaration: TypeDeclaration,
    ) -> bool {
        if self.declarations.contains_key(&key) {
            return false;
        }
        self.declarations.insert(key.clone(), declaration);
        if let Some(module) = module {
            let path = format!("{}.{}", module, name);
            if is_public {
                self.public_names.insert(path, key);
            } else {
                self.private_names.insert(path, key);
            }
        }
        true
    }

    fn lookup(&self, name: &str, scope: Option<&str>) -> NamedTypeLookup {
        if let Some(scope) = scope {
            let local = scoped_name(Some(scope), name);
            if self.declarations.contains_key(&local) {
                return NamedTypeLookup::Declared(local);
            }
        }
        if self.declarations.contains_key(name) {
            return NamedTypeLookup::Declared(name.to_string());
        }
        if let Some(key) = self.public_names.get(name) {
            return NamedTypeLookup::Declared(key.clone());
        }
        if self.private_names.contains_key(name) {
            return NamedTypeLookup::Private;
        }
        NamedTypeLookup::Missing
    }
}

/// Type checker for a program. Use it via `check_program`.
pub struct Checker {
    functions: HashMap<String, FuncSig>,
    globals: HashSet<String>,
    local_globals: HashSet<String>,
    private_globals: HashSet<String>,
    type_registry: TypeRegistry,
    deferred_diagnostics: Vec<(usize, Diagnostic)>,
    current_item_index: Option<usize>,
    defer_diagnostics: bool,
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
            type_registry: TypeRegistry::new(),
            deferred_diagnostics: Vec::new(),
            current_item_index: None,
            defer_diagnostics: false,
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
        self.defer_diagnostics = true;
        self.collect_types(program);
        self.collect_globals(program);
        self.collect_functions(program);
        self.defer_diagnostics = false;

        let mut indexed = self.deferred_diagnostics.clone();
        indexed.sort_by_key(|(index, _)| *index);
        for (index, item) in program.items.iter().enumerate() {
            self.current_module = program.item_modules.get(index).cloned().flatten();
            self.current_item_index = Some(index);
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

    fn collect_types(&mut self, program: &Program) {
        let mut function_names = HashMap::new();
        for (index, item) in program.items.iter().enumerate() {
            let TopLevelItem::Function(function) = item else {
                continue;
            };
            let module = program.item_modules.get(index).cloned().flatten();
            let key = scoped_name(module.as_deref(), &function.name.name);
            function_names.entry(key).or_insert(index);
            if function.is_public {
                if let Some(module) = module {
                    function_names
                        .entry(format!("{}.{}", module, function.name.name))
                        .or_insert(index);
                }
            }
        }

        for (index, item) in program.items.iter().enumerate() {
            self.current_item_index = Some(index);
            let module = program.item_modules.get(index).cloned().flatten();
            let (name, is_public, declaration) = match item {
                TopLevelItem::TypeAlias(alias) => (
                    alias.name.name.clone(),
                    alias.is_public,
                    TypeDeclaration::Alias {
                        target: alias.target.clone(),
                        scope: module.clone(),
                    },
                ),
                TopLevelItem::Struct(declaration) => (
                    declaration.name.name.clone(),
                    declaration.is_public,
                    TypeDeclaration::Struct {
                        fields: declaration
                            .fields
                            .iter()
                            .map(|field| (field.name.name.clone(), field.ty.clone()))
                            .collect(),
                        scope: module.clone(),
                    },
                ),
                TopLevelItem::Enum(declaration) => (
                    declaration.name.name.clone(),
                    declaration.is_public,
                    TypeDeclaration::Enum {
                        variants: declaration
                            .variants
                            .iter()
                            .map(|variant| (variant.name.name.clone(), variant.payload.clone()))
                            .collect(),
                        scope: module.clone(),
                    },
                ),
                _ => continue,
            };
            let key = scoped_name(module.as_deref(), &name);
            if !self.type_registry.insert(
                key.clone(),
                &name,
                module.as_deref(),
                is_public,
                declaration,
            ) {
                self.type_error(
                    DiagKind::DuplicateDeclaration,
                    item_name_span(item),
                    format!("duplicate type name \"{}\"", key),
                );
            }
            if function_names.contains_key(&key) {
                self.type_error(
                    DiagKind::DuplicateDeclaration,
                    item_name_span(item),
                    format!("type \"{}\" collides with function \"{}\"", name, name),
                );
            }
            let mut members = HashSet::new();
            match item {
                TopLevelItem::Struct(declaration) => {
                    for field in &declaration.fields {
                        if !members.insert(field.name.name.clone()) {
                            self.type_error(
                                DiagKind::DuplicateDeclaration,
                                field.span,
                                format!(
                                    "duplicate field \"{}\" in struct \"{}\"",
                                    field.name.name, name
                                ),
                            );
                        }
                    }
                }
                TopLevelItem::Enum(declaration) => {
                    for variant in &declaration.variants {
                        if !members.insert(variant.name.name.clone()) {
                            self.type_error(
                                DiagKind::DuplicateDeclaration,
                                variant.span,
                                format!(
                                    "duplicate variant \"{}\" in enum \"{}\"",
                                    variant.name.name, name
                                ),
                            );
                        }
                    }
                }
                _ => {}
            }
        }

        let mut declarations = Vec::new();
        for (owner, item) in program.items.iter().enumerate() {
            let name = match item {
                TopLevelItem::TypeAlias(declaration) => Some(&declaration.name.name),
                TopLevelItem::Struct(declaration) => Some(&declaration.name.name),
                TopLevelItem::Enum(declaration) => Some(&declaration.name.name),
                _ => None,
            };
            let Some(name) = name else {
                continue;
            };
            let module = program.item_modules.get(owner).cloned().flatten();
            let key = scoped_name(module.as_deref(), name);
            if let Some(declaration) = self.type_registry.declarations.get(&key).cloned() {
                declarations.push((key, declaration, owner));
            }
        }
        for (key, declaration, owner) in declarations {
            self.current_item_index = Some(owner);
            match declaration {
                TypeDeclaration::Struct { fields, scope } => {
                    let mut names = HashSet::new();
                    let mut signature = StructSig { fields: Vec::new() };
                    for (name, field_type) in fields {
                        if names.insert(name.clone()) {
                            signature.fields.push((
                                name,
                                self.resolve_type_expr_in_scope(&field_type, scope.as_deref()),
                            ));
                        }
                    }
                    self.type_registry.structs.insert(key, signature);
                }
                TypeDeclaration::Enum { variants, scope } => {
                    let mut names = HashSet::new();
                    let mut signature = EnumSig {
                        variants: Vec::new(),
                    };
                    for (name, payload) in variants {
                        if names.insert(name.clone()) {
                            let payload = payload.as_ref().map(|payload| {
                                self.resolve_type_expr_in_scope(payload, scope.as_deref())
                            });
                            signature.variants.push((name, payload));
                        }
                    }
                    self.type_registry.enums.insert(key, signature);
                }
                TypeDeclaration::Alias { target, scope } => {
                    self.resolve_type_expr_in_scope(&target, scope.as_deref());
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
                match self.type_registry.lookup(&identifier.name, scope) {
                    NamedTypeLookup::Declared(key) => {
                        self.resolve_type_declaration(&key, &mut HashSet::new())
                    }
                    NamedTypeLookup::Private => {
                        self.type_error(
                            DiagKind::InvalidOperand,
                            identifier.span,
                            format!("type \"{}\" is private to its module", identifier.name),
                        );
                        Ty::Any
                    }
                    NamedTypeLookup::Missing => {
                        self.type_error(
                            DiagKind::TypeMismatch,
                            identifier.span,
                            format!("unknown named type \"{}\"", identifier.name),
                        );
                        Ty::Any
                    }
                }
            }
            TypeExpr::Optional(inner) => {
                Ty::Optional(Box::new(self.resolve_type_expr_in_scope(inner, scope)))
            }
            TypeExpr::Array(inner) => {
                Ty::Array(Box::new(self.resolve_type_expr_in_scope(inner, scope)))
            }
            TypeExpr::Result(ok, err) => Ty::Result(
                Box::new(self.resolve_type_expr_in_scope(ok, scope)),
                Box::new(self.resolve_type_expr_in_scope(err, scope)),
            ),
            other => ty_from_expr(other),
        }
    }

    fn resolve_type_declaration(&mut self, key: &str, visiting: &mut HashSet<String>) -> Ty {
        if let Some(ty) = self.type_registry.resolved.get(key) {
            return ty.clone();
        }
        let Some(declaration) = self.type_registry.declarations.get(key).cloned() else {
            return Ty::Any;
        };
        if !visiting.insert(key.to_string()) {
            return Ty::Any;
        }
        let ty = match declaration {
            TypeDeclaration::Alias { target, scope } => {
                self.resolve_type_expr_in_scope(&target, scope.as_deref())
            }
            TypeDeclaration::Struct { .. } => Ty::Struct(key.to_string()),
            TypeDeclaration::Enum { .. } => Ty::Enum(key.to_string()),
        };
        visiting.remove(key);
        self.type_registry
            .resolved
            .insert(key.to_string(), ty.clone());
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
                        self.local_globals
                            .insert(scoped_name(Some(scope), &model.name.name));
                        if model.is_public {
                            self.globals
                                .insert(format!("{}.{}", scope, model.name.name));
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
                self.current_item_index = Some(index);
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
                let ret = f
                    .return_type
                    .as_ref()
                    .map(|return_type| {
                        self.resolve_type_expr_in_scope(return_type, module.as_deref())
                    })
                    .unwrap_or(Ty::Unit);
                let signature = FuncSig { params, ret };
                self.functions.insert(
                    scoped_name(module.as_deref(), &f.name.name),
                    signature.clone(),
                );
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
        let signature = self.functions.get(&function_name).cloned();
        let expected = signature
            .as_ref()
            .map(|signature| signature.ret.clone())
            .unwrap_or(Ty::Unit);

        self.expected_return = Some(expected.clone());
        self.scopes.push(HashMap::new());
        for (index, p) in f.params.iter().enumerate() {
            let parameter_type = signature
                .as_ref()
                .and_then(|signature| signature.params.get(index))
                .map(|parameter| parameter.ty.clone())
                .unwrap_or_else(|| self.resolve_type_expr(&p.ty));
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

    fn ui_type_ref_ty(&mut self, type_ref: &TypeRef) -> Ty {
        match type_ref {
            TypeRef::String => Ty::String,
            TypeRef::Int => Ty::Int,
            TypeRef::Float => Ty::Float,
            TypeRef::Bool => Ty::Bool,
            TypeRef::Array(inner) => Ty::Array(Box::new(self.ui_type_ref_ty(inner))),
            TypeRef::Named(name) => {
                match self
                    .type_registry
                    .lookup(name, self.current_module.as_deref())
                {
                    NamedTypeLookup::Declared(key) => {
                        let ty = self.resolve_type_declaration(&key, &mut HashSet::new());
                        if matches!(ty, Ty::Struct(_) | Ty::Enum(_)) {
                            self.error(
                                DiagKind::UnsupportedFeature,
                                Span::dummy(),
                                format!("nominal type '{}' is not supported in UI values", name),
                            );
                            Ty::Any
                        } else {
                            ty
                        }
                    }
                    NamedTypeLookup::Private => {
                        self.error(
                            DiagKind::InvalidOperand,
                            Span::dummy(),
                            format!("type '{}' is private to its module", name),
                        );
                        Ty::Any
                    }
                    NamedTypeLookup::Missing => {
                        self.error(
                            DiagKind::TypeMismatch,
                            Span::dummy(),
                            format!("unknown named type \"{}\"", name),
                        );
                        Ty::Any
                    }
                }
            }
        }
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
                let prop_type = prop
                    .ty
                    .as_ref()
                    .map(|ty| self.ui_type_ref_ty(ty))
                    .unwrap_or(Ty::Any);
                self.declare(&prop.name.name, prop_type, false);
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
                let declared = state.ty.as_ref().map(|ty| self.ui_type_ref_ty(ty));
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
                    self.declare(&state.name.name, declared.unwrap_or(actual), true);
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
                        if provided
                            .insert(property.name.name.clone(), property.value.clone())
                            .is_some()
                        {
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
                                format!(
                                    "unknown prop \"{}\" on component \"{}\"",
                                    property.name.name, component_use.name.name
                                ),
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
                                format!(
                                    "missing prop \"{}\" on component \"{}\"",
                                    name, component_use.name.name
                                ),
                            );
                        }
                    }
                }
            }
        }
    }

    fn check_ui_element(&mut self, element: &ElementExpr) {
        const ELEMENTS: &[&str] = &[
            "Column", "Row", "Text", "Display", "Input", "Button", "Card", "Divider", "Heading",
            "Spacer", "Messages",
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
                            format!(
                                "UI binding target \"{}\" is not declared",
                                binding.target.name
                            ),
                        );
                    } else if let Some(ty) = self.lookup(&binding.target.name) {
                        if ty != &Ty::String && !ty.is_any() {
                            self.error(
                                DiagKind::TypeMismatch,
                                binding.span,
                                format!(
                                    "Input binding \"{}\" requires string, got {}",
                                    binding.target.name, ty
                                ),
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
                        format!(
                            "operator += is not valid for {} and {}",
                            target_ty, value_ty
                        ),
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
                    format!("invalid return type: expected {}, got {}", exp, value_ty),
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
                    Ty::Struct(identity) => {
                        let signature = self.type_registry.structs.get(identity);
                        match signature.and_then(|signature| {
                            signature
                                .fields
                                .iter()
                                .find(|(name, _)| *name == id.name)
                                .map(|(_, ty)| ty.clone())
                        }) {
                            Some(field_type) => field_type,
                            None => {
                                self.error(
                                    DiagKind::InvalidOperand,
                                    id.span,
                                    format!("struct {} has no field \"{}\"", identity, id.name),
                                );
                                Ty::Any
                            }
                        }
                    }
                    Ty::Enum(identity) => {
                        let signature = self.type_registry.enums.get(identity);
                        match signature.and_then(|signature| {
                            signature.variants.iter().find(|(name, _)| *name == id.name)
                        }) {
                            Some((_, None)) => ty.clone(),
                            Some((_, Some(_))) => {
                                self.error(
                                    DiagKind::InvalidOperand,
                                    id.span,
                                    format!(
                                        "enum variant {}.{} requires a payload",
                                        identity, id.name
                                    ),
                                );
                                Ty::Any
                            }
                            None => {
                                self.error(
                                    DiagKind::InvalidOperand,
                                    id.span,
                                    format!("enum {} has no variant \"{}\"", identity, id.name),
                                );
                                Ty::Any
                            }
                        }
                    }
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
            Expr::Member(member) => {
                if let Some(ty) = self.declared_type_member_ty(member) {
                    ty
                } else {
                    let object = self.expr_ty(&member.object);
                    match object {
                        Ty::Object(fields) => {
                            match fields
                                .into_iter()
                                .find(|(key, _)| *key == member.property.name)
                            {
                                Some((_, ty)) => ty,
                                None => {
                                    self.error(
                                        DiagKind::InvalidOperand,
                                        member.property.span,
                                        format!("object has no field \"{}\"", member.property.name),
                                    );
                                    Ty::Any
                                }
                            }
                        }
                        Ty::Struct(identity) => {
                            self.struct_member_ty(&identity, &member.property, member.span)
                        }
                        Ty::Enum(identity) => {
                            self.enum_member_ty(&identity, &member.property, member.span)
                        }
                        Ty::Any => Ty::Any,
                        other => {
                            self.error(
                                DiagKind::InvalidOperand,
                                member.span,
                                format!("cannot access .{} on {}", member.property.name, other),
                            );
                            Ty::Any
                        }
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
            Expr::Match(match_expr) => {
                let scrutinee = self.expr_ty(&match_expr.scrutinee);
                let mut result: Option<Ty> = None;
                for arm in &match_expr.arms {
                    self.scopes.push(HashMap::new());
                    self.check_pattern(&arm.pattern, &scrutinee, arm.span);
                    let arm_ty = match &arm.body {
                        MatchBody::Expr(expr) => self.expr_ty(expr),
                        MatchBody::Block(block) => {
                            self.check_block(block, None);
                            Ty::Any
                        }
                    };
                    self.scopes.pop();
                    result = Some(match result {
                        None => arm_ty,
                        Some(previous) => join(previous, arm_ty),
                    });
                }
                self.check_match_exhaustiveness(&scrutinee, &match_expr.arms, match_expr.span);
                result.unwrap_or(Ty::Any)
            }
        }
    }

    fn check_match_exhaustiveness(
        &mut self,
        scrutinee: &Ty,
        arms: &[aec_ast::MatchArm],
        span: Span,
    ) {
        if arms.iter().any(|arm| {
            matches!(
                arm.pattern,
                Pattern::Wildcard(_) | Pattern::Identifier(_)
            )
        }) {
            return;
        }

        match scrutinee {
            Ty::Enum(identity) => {
                let Some(signature) = self.type_registry.enums.get(identity) else {
                    return;
                };
                let covered = arms
                    .iter()
                    .filter(|arm| self.pattern_is_total(&arm.pattern, scrutinee))
                    .filter_map(|arm| match &arm.pattern {
                        Pattern::EnumVariant(variant) if variant.path.len() >= 2 => {
                            Some(variant.path[variant.path.len() - 1].name.clone())
                        }
                        _ => None,
                    })
                    .collect::<HashSet<_>>();
                let missing = signature
                    .variants
                    .iter()
                    .filter(|(name, _)| !covered.contains(name))
                    .map(|(name, _)| name.clone())
                    .collect::<Vec<_>>();
                if !missing.is_empty() {
                    self.error(
                        DiagKind::TypeMismatch,
                        span,
                        format!(
                            "non-exhaustive match for enum {}; missing variants: {}",
                            identity,
                            missing.join(", ")
                        ),
                    );
                }
            }
            Ty::Optional(_) => {
                let has_none = arms.iter().any(|arm| {
                    matches!(
                        arm.pattern,
                        Pattern::None(_)
                            | Pattern::Literal(aec_ast::Literal::None)
                    )
                });
                let has_some = arms
                    .iter()
                    .any(|arm| matches!(arm.pattern, Pattern::Some(_)));
                if !has_none || !has_some {
                    self.error(
                        DiagKind::TypeMismatch,
                        span,
                        "non-exhaustive match for optional value".to_string(),
                    );
                }
            }
            Ty::Bool => {
                let has_true = arms.iter().any(|arm| {
                    matches!(arm.pattern, Pattern::Literal(aec_ast::Literal::Bool(true)))
                });
                let has_false = arms.iter().any(|arm| {
                    matches!(arm.pattern, Pattern::Literal(aec_ast::Literal::Bool(false)))
                });
                if !has_true || !has_false {
                    self.error(
                        DiagKind::TypeMismatch,
                        span,
                        "non-exhaustive match for bool value".to_string(),
                    );
                }
            }
            Ty::None => {
                let has_none = arms.iter().any(|arm| {
                    matches!(
                        arm.pattern,
                        Pattern::None(_)
                            | Pattern::Literal(aec_ast::Literal::None)
                    )
                });
                if !has_none {
                    self.error(
                        DiagKind::TypeMismatch,
                        span,
                        "non-exhaustive match for none value".to_string(),
                    );
                }
            }
            _ => {}
        }
    }

    fn pattern_is_total(&self, pattern: &Pattern, expected: &Ty) -> bool {
        match pattern {
            Pattern::Wildcard(_) | Pattern::Identifier(_) => true,
            Pattern::Some(_) | Pattern::None(_) => false,
            Pattern::Literal(_) => false,
            Pattern::EnumVariant(variant) => {
                if variant.path.len() < 2 {
                    return false;
                }
                let Ty::Enum(identity) = expected else {
                    return false;
                };
                let type_name = variant.path[..variant.path.len() - 1]
                    .iter()
                    .map(|identifier| identifier.name.clone())
                    .collect::<Vec<_>>()
                    .join(".")
                    .replace('.', "::");
                if type_name != *identity {
                    return false;
                }
                let Some(signature) = self.type_registry.enums.get(identity) else {
                    return false;
                };
                let Some((_, payload)) = signature
                    .variants
                    .iter()
                    .find(|(name, _)| *name == variant.path[variant.path.len() - 1].name)
                else {
                    return false;
                };
                match (payload, &variant.payload) {
                    (None, None) => true,
                    (Some(payload), Some(pattern)) => self.pattern_is_total(pattern, payload),
                    _ => false,
                }
            }
            Pattern::Struct(pattern) => {
                let Ty::Struct(identity) = expected else {
                    return false;
                };
                let type_name = pattern
                    .path
                    .iter()
                    .map(|identifier| identifier.name.clone())
                    .collect::<Vec<_>>()
                    .join(".")
                    .replace('.', "::");
                if type_name != *identity {
                    return false;
                }
                let Some(signature) = self.type_registry.structs.get(identity) else {
                    return false;
                };
                let mut names = HashSet::new();
                if pattern.fields.len() != signature.fields.len() {
                    return false;
                }
                pattern.fields.iter().all(|field| {
                    if !names.insert(field.name.name.clone()) {
                        return false;
                    }
                    signature
                        .fields
                        .iter()
                        .find(|(name, _)| *name == field.name.name)
                        .map(|(_, ty)| self.pattern_is_total(&field.pattern, ty))
                        .unwrap_or(false)
                })
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
        match self
            .type_registry
            .lookup(&name, self.current_module.as_deref())
        {
            NamedTypeLookup::Declared(_) => return Ty::Any,
            NamedTypeLookup::Private => {
                self.error(
                    DiagKind::InvalidOperand,
                    span,
                    format!("type \"{}\" is private to its module", name),
                );
                return Ty::Any;
            }
            NamedTypeLookup::Missing => {}
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

    fn declared_type_member_ty(&mut self, member: &aec_ast::MemberExpr) -> Option<Ty> {
        let mut segments = expression_path(&member.object)?;
        segments.push(member.property.name.clone());
        if segments.len() < 2 {
            return None;
        }
        let type_name = segments[..segments.len() - 1].join(".");
        let scope = self.current_module.clone();
        match self.type_registry.lookup(&type_name, scope.as_deref()) {
            NamedTypeLookup::Declared(identity) => {
                match self.resolve_type_declaration(&identity, &mut HashSet::new()) {
                    Ty::Struct(identity) => {
                        Some(self.struct_member_ty(&identity, &member.property, member.span))
                    }
                    Ty::Enum(identity) => {
                        Some(self.enum_member_ty(&identity, &member.property, member.span))
                    }
                    _ => None,
                }
            }
            NamedTypeLookup::Private => {
                self.type_error(
                    DiagKind::InvalidOperand,
                    member.span,
                    format!("type \"{}\" is private to its module", type_name),
                );
                Some(Ty::Any)
            }
            NamedTypeLookup::Missing => None,
        }
    }

    fn struct_member_ty(
        &mut self,
        identity: &str,
        property: &aec_ast::Identifier,
        _span: Span,
    ) -> Ty {
        let field = self
            .type_registry
            .structs
            .get(identity)
            .and_then(|signature| {
                signature
                    .fields
                    .iter()
                    .find(|(name, _)| name == &property.name)
                    .map(|(_, ty)| ty.clone())
            });
        match field {
            Some(field_type) => field_type,
            None => {
                self.error(
                    DiagKind::InvalidOperand,
                    property.span,
                    format!("struct {} has no field \"{}\"", identity, property.name),
                );
                Ty::Any
            }
        }
    }

    fn enum_member_ty(
        &mut self,
        identity: &str,
        property: &aec_ast::Identifier,
        _span: Span,
    ) -> Ty {
        let variant = self
            .type_registry
            .enums
            .get(identity)
            .and_then(|signature| {
                signature
                    .variants
                    .iter()
                    .find(|(name, _)| name == &property.name)
                    .map(|(_, payload)| payload.clone())
            });
        match variant {
            Some(None) => Ty::Enum(identity.to_string()),
            Some(Some(_)) => {
                self.error(
                    DiagKind::InvalidOperand,
                    property.span,
                    format!(
                        "enum variant {}.{} requires a payload",
                        identity, property.name
                    ),
                );
                Ty::Any
            }
            None => {
                self.error(
                    DiagKind::InvalidOperand,
                    property.span,
                    format!("enum {} has no variant \"{}\"", identity, property.name),
                );
                Ty::Any
            }
        }
    }

    fn declared_constructor_ty(
        &mut self,
        segments: &[String],
        name: &str,
        args: &[aec_ast::Argument],
        arg_types: &[Ty],
        span: Span,
    ) -> Option<Ty> {
        if segments.len() > 1 {
            let type_name = segments[..segments.len() - 1].join(".");
            let scope = self.current_module.clone();
            match self.type_registry.lookup(&type_name, scope.as_deref()) {
                NamedTypeLookup::Declared(identity) => {
                    if let Ty::Enum(identity) =
                        self.resolve_type_declaration(&identity, &mut HashSet::new())
                    {
                        return Some(self.enum_constructor_ty(
                            &identity,
                            &segments[segments.len() - 1],
                            name,
                            args,
                            arg_types,
                            span,
                        ));
                    }
                }
                NamedTypeLookup::Private => {
                    self.type_error(
                        DiagKind::InvalidOperand,
                        span,
                        format!("type \"{}\" is private to its module", type_name),
                    );
                    return Some(Ty::Any);
                }
                NamedTypeLookup::Missing => {}
            }
        }

        let scope = self.current_module.clone();
        match self.type_registry.lookup(name, scope.as_deref()) {
            NamedTypeLookup::Declared(identity) => {
                match self.resolve_type_declaration(&identity, &mut HashSet::new()) {
                    Ty::Struct(identity) => {
                        Some(self.struct_constructor_ty(&identity, name, args, arg_types, span))
                    }
                    Ty::Enum(_) => None,
                    _ => None,
                }
            }
            NamedTypeLookup::Private => {
                self.type_error(
                    DiagKind::InvalidOperand,
                    span,
                    format!("type \"{}\" is private to its module", name),
                );
                Some(Ty::Any)
            }
            NamedTypeLookup::Missing => None,
        }
    }

    fn struct_constructor_ty(
        &mut self,
        identity: &str,
        name: &str,
        args: &[aec_ast::Argument],
        arg_types: &[Ty],
        span: Span,
    ) -> Ty {
        let fields = self
            .type_registry
            .structs
            .get(identity)
            .map(|signature| signature.fields.clone())
            .unwrap_or_default();
        let mut used = vec![false; fields.len()];
        for (argument, argument_ty) in args.iter().zip(arg_types.iter()) {
            let Some(argument_name) = &argument.name else {
                self.error(
                    DiagKind::ArityMismatch,
                    argument.span,
                    format!("struct constructor \"{}\" requires named arguments", name),
                );
                continue;
            };
            let Some(index) = fields
                .iter()
                .position(|(field_name, _)| field_name == &argument_name.name)
            else {
                self.error(
                    DiagKind::ArityMismatch,
                    argument.span,
                    format!(
                        "unknown field \"{}\" for struct \"{}\"",
                        argument_name.name, name
                    ),
                );
                continue;
            };
            if used[index] {
                self.error(
                    DiagKind::ArityMismatch,
                    argument.span,
                    format!(
                        "field \"{}\" was provided more than once for struct \"{}\"",
                        argument_name.name, name
                    ),
                );
                continue;
            }
            used[index] = true;
            if !compatible(&fields[index].1, argument_ty) {
                self.error(
                    DiagKind::TypeMismatch,
                    argument.span,
                    format!(
                        "field \"{}\" of struct \"{}\" expects {}, got {}",
                        argument_name.name, name, fields[index].1, argument_ty
                    ),
                );
            }
        }
        for (index, provided) in used.iter().enumerate() {
            if !provided {
                self.error(
                    DiagKind::ArityMismatch,
                    span,
                    format!(
                        "missing field \"{}\" for struct \"{}\"",
                        fields[index].0, name
                    ),
                );
            }
        }
        Ty::Struct(identity.to_string())
    }

    fn enum_constructor_ty(
        &mut self,
        identity: &str,
        variant_name: &str,
        name: &str,
        args: &[aec_ast::Argument],
        arg_types: &[Ty],
        span: Span,
    ) -> Ty {
        let variant = self
            .type_registry
            .enums
            .get(identity)
            .and_then(|signature| {
                signature
                    .variants
                    .iter()
                    .find(|(candidate, _)| candidate == variant_name)
                    .map(|(_, payload)| payload.clone())
            });
        match variant {
            Some(None) => {
                if !args.is_empty() {
                    self.error(
                        DiagKind::ArityMismatch,
                        span,
                        format!(
                            "enum variant \"{}\" does not take a payload, got {} arguments",
                            name,
                            args.len()
                        ),
                    );
                }
            }
            Some(Some(expected)) => {
                if args.len() != 1 {
                    self.error(
                        DiagKind::ArityMismatch,
                        span,
                        format!(
                            "enum variant \"{}\" expects 1 payload argument, got {}",
                            name,
                            args.len()
                        ),
                    );
                } else if args[0].name.is_some() {
                    self.error(
                        DiagKind::ArityMismatch,
                        args[0].span,
                        format!("enum variant \"{}\" requires a positional payload", name),
                    );
                } else if !compatible(&expected, &arg_types[0]) {
                    self.error(
                        DiagKind::TypeMismatch,
                        args[0].span,
                        format!(
                            "enum variant \"{}\" expects payload {}, got {}",
                            name, expected, arg_types[0]
                        ),
                    );
                }
            }
            None => {
                self.error(
                    DiagKind::InvalidOperand,
                    span,
                    format!("enum {} has no variant \"{}\"", identity, variant_name),
                );
            }
        }
        Ty::Enum(identity.to_string())
    }

    fn check_pattern(&mut self, pattern: &Pattern, expected: &Ty, span: Span) {
        match pattern {
            Pattern::Wildcard(_) => {}
            Pattern::Identifier(identifier) => {
                self.declare(&identifier.name, expected.clone(), false);
            }
            Pattern::Some(identifier) => {
                let payload = match expected {
                    Ty::Optional(inner) => (**inner).clone(),
                    Ty::Any => Ty::Any,
                    other => {
                        self.error(
                            DiagKind::TypeMismatch,
                            identifier.span,
                            format!("some pattern requires an optional value, got {}", other),
                        );
                        Ty::Any
                    }
                };
                self.declare(&identifier.name, payload, false);
            }
            Pattern::None(pattern_span) => {
                if !matches!(expected, Ty::Optional(_) | Ty::None | Ty::Any) {
                    self.error(
                        DiagKind::TypeMismatch,
                        *pattern_span,
                        format!("none pattern requires an optional value, got {}", expected),
                    );
                }
            }
            Pattern::Literal(literal) => {
                let literal = literal_ty(literal);
                if !compatible(expected, &literal) {
                    self.error(
                        DiagKind::TypeMismatch,
                        span,
                        format!("pattern of type {} cannot match {}", literal, expected),
                    );
                }
            }
            Pattern::EnumVariant(variant) => {
                if variant.path.len() < 2 {
                    self.error(
                        DiagKind::InvalidOperand,
                        variant.span,
                        "enum pattern path must include a type and variant".to_string(),
                    );
                    return;
                }
                let type_name = variant.path[..variant.path.len() - 1]
                    .iter()
                    .map(|identifier| identifier.name.clone())
                    .collect::<Vec<_>>()
                    .join(".");
                let scope = self.current_module.clone();
                let identity = match self.type_registry.lookup(&type_name, scope.as_deref()) {
                    NamedTypeLookup::Declared(identity) => identity,
                    NamedTypeLookup::Private => {
                        self.type_error(
                            DiagKind::InvalidOperand,
                            variant.span,
                            format!("type \"{}\" is private to its module", type_name),
                        );
                        return;
                    }
                    NamedTypeLookup::Missing => {
                        self.error(
                            DiagKind::InvalidOperand,
                            variant.span,
                            format!("unknown enum type \"{}\"", type_name),
                        );
                        return;
                    }
                };
                let enum_ty = self.resolve_type_declaration(&identity, &mut HashSet::new());
                let enum_identity = match enum_ty {
                    Ty::Enum(identity) => identity,
                    _ => {
                        self.error(
                            DiagKind::TypeMismatch,
                            variant.span,
                            format!("\"{}\" is not an enum type", type_name),
                        );
                        return;
                    }
                };
                if !expected.is_any() && *expected != Ty::Enum(enum_identity.clone()) {
                    self.error(
                        DiagKind::TypeMismatch,
                        variant.span,
                        format!(
                            "enum pattern for {} cannot match {}",
                            enum_identity, expected
                        ),
                    );
                }
                let variant_name = &variant.path[variant.path.len() - 1].name;
                let payload = self
                    .type_registry
                    .enums
                    .get(&enum_identity)
                    .and_then(|signature| {
                        signature
                            .variants
                            .iter()
                            .find(|(name, _)| name == variant_name)
                            .map(|(_, payload)| payload.clone())
                    });
                let Some(payload) = payload else {
                    self.error(
                        DiagKind::InvalidOperand,
                        variant.span,
                        format!("enum {} has no variant \"{}\"", enum_identity, variant_name),
                    );
                    return;
                };
                match (payload, &variant.payload) {
                    (None, None) => {}
                    (None, Some(_)) => {
                        self.error(
                            DiagKind::TypeMismatch,
                            variant.span,
                            format!(
                                "enum variant {}.{} has no payload",
                                enum_identity, variant_name
                            ),
                        );
                    }
                    (Some(_), None) => {
                        self.error(
                            DiagKind::TypeMismatch,
                            variant.span,
                            format!(
                                "enum variant {}.{} requires a payload pattern",
                                enum_identity, variant_name
                            ),
                        );
                    }
                    (Some(expected), Some(pattern)) => {
                        self.check_pattern(pattern, &expected, variant.span);
                    }
                }
            }
            Pattern::Struct(pattern) => {
                if pattern.path.is_empty() {
                    self.error(
                        DiagKind::InvalidOperand,
                        pattern.span,
                        "struct pattern path must include a type".to_string(),
                    );
                    return;
                }
                let type_name = pattern
                    .path
                    .iter()
                    .map(|identifier| identifier.name.clone())
                    .collect::<Vec<_>>()
                    .join(".");
                let scope = self.current_module.clone();
                let identity = match self.type_registry.lookup(&type_name, scope.as_deref()) {
                    NamedTypeLookup::Declared(identity) => identity,
                    NamedTypeLookup::Private => {
                        self.type_error(
                            DiagKind::InvalidOperand,
                            pattern.span,
                            format!("type \"{}\" is private to its module", type_name),
                        );
                        return;
                    }
                    NamedTypeLookup::Missing => {
                        self.error(
                            DiagKind::InvalidOperand,
                            pattern.span,
                            format!("unknown struct type \"{}\"", type_name),
                        );
                        return;
                    }
                };
                let struct_identity = match self.resolve_type_declaration(&identity, &mut HashSet::new()) {
                    Ty::Struct(identity) => identity,
                    _ => {
                        self.error(
                            DiagKind::TypeMismatch,
                            pattern.span,
                            format!("\"{}\" is not a struct type", type_name),
                        );
                        return;
                    }
                };
                if !expected.is_any() && *expected != Ty::Struct(struct_identity.clone()) {
                    self.error(
                        DiagKind::TypeMismatch,
                        pattern.span,
                        format!(
                            "struct pattern for {} cannot match {}",
                            struct_identity, expected
                        ),
                    );
                }
                let fields = self
                    .type_registry
                    .structs
                    .get(&struct_identity)
                    .map(|signature| signature.fields.clone())
                    .unwrap_or_default();
                let mut seen = HashSet::new();
                for field in &pattern.fields {
                    if !seen.insert(field.name.name.clone()) {
                        self.error(
                            DiagKind::DuplicateDeclaration,
                            field.span,
                            format!(
                                "duplicate field \"{}\" in pattern for {}",
                                field.name.name, struct_identity
                            ),
                        );
                        continue;
                    }
                    let Some((_, field_type)) = fields
                        .iter()
                        .find(|(name, _)| *name == field.name.name)
                    else {
                        self.error(
                            DiagKind::InvalidOperand,
                            field.span,
                            format!(
                                "struct {} has no field \"{}\"",
                                struct_identity, field.name.name
                            ),
                        );
                        continue;
                    };
                    self.check_pattern(&field.pattern, field_type, field.span);
                }
            }
        }
    }

    fn call_ty(&mut self, callee: &Expr, args: &[aec_ast::Argument], span: Span) -> Ty {
        // check the arguments first so errors inside them are seen.
        let arg_types: Vec<Ty> = args.iter().map(|a| self.expr_ty(&a.value)).collect();

        let Some(segments) = expression_path(callee) else {
            self.expr_ty(callee);
            return Ty::Any;
        };
        let name = segments.join(".");
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
                let index =
                    if let Some(argument_name) = &argument.name {
                        named_seen = true;
                        let Some(index) = sig.params.iter().position(|(parameter_name, _, _)| {
                            parameter_name == &argument_name.name
                        }) else {
                            self.error(
                                DiagKind::ArityMismatch,
                                argument.span,
                                format!(
                                    "unknown argument \"{}\" for function \"{}\"",
                                    argument_name.name, name
                                ),
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
                        format!(
                            "argument \"{}\" was provided more than once",
                            sig.params[index].0
                        ),
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
                        format!(
                            "missing argument \"{}\" for function \"{}\"",
                            sig.params[index].0, name
                        ),
                    );
                }
            }
            return sig.ret;
        }

        if let Some(ty) = self.declared_constructor_ty(&segments, &name, args, &arg_types, span) {
            return ty;
        }

        // `ok` / `err` are the Result constructors, and `is_ok` / `is_err` the
        // predicates. They are typed here so `?` can be checked.
        match lookup_name.as_str() {
            "ok" | "err" | "is_ok" | "is_err" => {
                if args.len() != 1 {
                    self.error(
                        DiagKind::ArityMismatch,
                        span,
                        format!(
                            "function \"{}\" expects 1 argument, got {}",
                            name,
                            args.len()
                        ),
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

    fn type_error(&mut self, kind: DiagKind, span: Span, message: impl Into<String>) {
        let diagnostic = Diagnostic::error(kind, span, message);
        if self.defer_diagnostics {
            self.deferred_diagnostics
                .push((self.current_item_index.unwrap_or_default(), diagnostic));
        } else {
            self.diagnostics.push(diagnostic);
        }
    }

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

fn expression_path(expr: &Expr) -> Option<Vec<String>> {
    match expr {
        Expr::Identifier(identifier) => Some(vec![identifier.name.clone()]),
        Expr::Member(member) => {
            let mut path = expression_path(&member.object)?;
            path.push(member.property.name.clone());
            Some(path)
        }
        _ => None,
    }
}

fn item_name_span(item: &TopLevelItem) -> Span {
    match item {
        TopLevelItem::TypeAlias(declaration) => declaration.name.span,
        TopLevelItem::Struct(declaration) => declaration.name.span,
        TopLevelItem::Enum(declaration) => declaration.name.span,
        _ => Span::dummy(),
    }
}

fn builtin_return_ty(name: &str) -> Ty {
    match name {
        "len" | "count" | "int" | "random_int" | "math_random_int" | "now" | "now_ms"
        | "time_now_ms" | "time_now_sec" | "math.floor" | "math_floor" | "floor" | "math.ceil"
        | "math_ceil" | "ceil" | "math.round" | "math_round" | "round" => Ty::Int,
        "float" | "math.sin" | "math_sin" | "sin" | "math.cos" | "math_cos" | "cos"
        | "math.tan" | "math_tan" | "tan" | "math.log" | "math_log" | "log" | "math.log10"
        | "log10" | "math.exp" | "math_exp" | "exp" | "math.pi" | "math_pi" | "pi" | "math.e"
        | "math_e" | "e" | "math.tau" | "math_tau" | "sqrt" | "math.random" | "math_random"
        | "random" => Ty::Float,
        "contains" | "starts_with" | "ends_with" | "has" | "is_ok" | "is_err" | "bool" => Ty::Bool,
        "str"
        | "upper"
        | "lower"
        | "trim"
        | "split"
        | "join"
        | "replace"
        | "repeat"
        | "char_at"
        | "json.stringify"
        | "json_stringify"
        | "md5"
        | "sha256"
        | "sha512"
        | "base64_encode"
        | "base64_decode"
        | "b64_encode"
        | "b64_decode"
        | "crypto.md5"
        | "crypto.sha256"
        | "crypto.sha512"
        | "crypto.base64_encode"
        | "crypto.base64_decode"
        | "regex.find"
        | "regex_find"
        | "regex.replace"
        | "regex_replace" => Ty::String,
        "range" | "regex.find_all" | "regex_find_all" | "keys" | "values" => {
            Ty::Array(Box::new(if name == "range" { Ty::Int } else { Ty::Any }))
        }
        "ok" => Ty::Result(Box::new(Ty::Any), Box::new(Ty::Any)),
        "err" => Ty::Result(Box::new(Ty::Any), Box::new(Ty::Any)),
        "map" | "filter" => Ty::Array(Box::new(Ty::Any)),
        "reduce" => Ty::Any,
        "sort" | "reverse" | "slice" | "push" | "file.list_dir" | "file_list_dir" | "ls" => Ty::Any,
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
fn join_result_component(a: Ty, b: Ty) -> Ty {
    if a == b {
        return a;
    }
    if a.is_any() {
        return b;
    }
    if b.is_any() {
        return a;
    }
    join(a, b)
}

fn join(a: Ty, b: Ty) -> Ty {
    if a == b {
        return a;
    }
    if a.is_any() || b.is_any() {
        return Ty::Any;
    }
    match (&a, &b) {
        (Ty::Optional(a_inner), Ty::Optional(b_inner)) => Ty::Optional(Box::new(join(
            (**a_inner).clone(),
            (**b_inner).clone(),
        ))),
        (Ty::Optional(a_inner), Ty::None) => Ty::Optional(a_inner.clone()),
        (Ty::None, Ty::Optional(b_inner)) => Ty::Optional(b_inner.clone()),
        (Ty::None, other) | (other, Ty::None) => Ty::Optional(Box::new(other.clone())),
        (Ty::Result(a_ok, a_err), Ty::Result(b_ok, b_err)) => Ty::Result(
            Box::new(join_result_component((**a_ok).clone(), (**b_ok).clone())),
            Box::new(join_result_component((**a_err).clone(), (**b_err).clone())),
        ),
        _ if a.is_numeric() && b.is_numeric() => Ty::numeric_result(&a, &b),
        _ => Ty::Any,
    }
}

/// Entry point: checks a program.
pub fn check_program(program: &Program) -> Vec<Diagnostic> {
    Checker::new().check_program(program)
}
