use aec_ast::ResolvedImport;
use aec_check::{check_program, DiagKind, Diagnostic};
use pretty_assertions::assert_eq;

fn diags(source: &str) -> Vec<Diagnostic> {
    let program = aec_parser::parse(source).expect("source should parse");
    check_program(&program)
}

fn errors(source: &str) -> Vec<Diagnostic> {
    diags(source).into_iter().filter(|d| d.is_error()).collect()
}

fn has(diags: &[Diagnostic], kind: DiagKind) -> bool {
    diags.iter().any(|d| d.kind == kind)
}

// ---------- valid programs ----------

#[test]
fn valid_program_has_no_errors() {
    let src = r#"agent Test

fn add(a: int, b: int) -> int {
    return a + b
}

fn main() -> int {
    let total = add(1, 2)
    let msg: string = "total"
    return total
}
"#;
    assert_eq!(errors(src), Vec::new());
}

#[test]
fn ui_state_names_are_known_to_event_functions() {
    let source = r#"agent Chat

fn send() {
    if draft != "" {
        push_to("messages", { role: "user", content: draft })
    }
}

ui Main = Screen "Chat" {
    Column {
        @draft: string = ""
        if true {
            @messages = []
        }
    }
}
"#;
    assert_eq!(diags(source), Vec::new());
}

#[test]
fn indexing_known_non_collection_reports_error() {
    let source = "agent Test\nfn main() -> int {\n    let value = 42\n    let bad = value[0]\n    return 0\n}\n";
    assert!(has(&errors(source), DiagKind::NotIndexable));
}

#[test]
fn string_indexing_remains_valid() {
    let source =
        "agent Test\nfn main() -> int {\n    let letter: string = \"abc\"[0]\n    return 0\n}\n";
    assert_eq!(errors(source), Vec::new());
}

#[test]
fn valid_string_concat_and_float_mix() {
    let src = r#"agent Test

fn label() -> string {
    return "a" + "b"
}

fn ratio() -> float {
    let x: float = 1 + 2.5
    return x
}
"#;
    assert_eq!(errors(src), Vec::new());
}

#[test]
fn valid_control_flow_and_collections() {
    let src = r#"agent Test

fn classify(n: int) -> string {
    if n > 0 {
        let items = [1, 2, 3]
        for it in items {
            let doubled = it * 2
        }
        return "positive"
    } else {
        return "other"
    }
}

fn counts() -> int {
    let x = 0
    while x < 3 {
        let y = x + 1
    }
    return x
}
"#;
    assert_eq!(errors(src), Vec::new());
}

#[test]
fn model_names_are_known_globals() {
    let src = r#"agent Test

model primary {
    provider: "openai"
    name: "gpt-4o"
}

fn ask() -> string {
    let r = primary.think("hi")
    return r
}
"#;
    assert_eq!(errors(src), Vec::new());
}

#[test]
fn named_type_aliases_are_resolved() {
    let src = r#"agent Test

type UserId = string

fn echo(value: UserId) -> UserId {
    return value
}

fn main() -> UserId {
    return echo("ok")
}
"#;
    assert_eq!(errors(src), Vec::new());
}

#[test]
fn named_type_alias_mismatch_is_reported() {
    let src = "agent Test\ntype UserId = string\nfn main() -> int {\n    let value: UserId = 1\n    return 0\n}\n";
    assert!(has(&errors(src), DiagKind::TypeMismatch));
}

#[test]
fn builtin_namespaces_are_not_flagged() {
    let src = r#"agent Test

fn f() -> int {
    let t = time.now_ms()
    let p = math.floor(1.5)
    let s = "x"
    return 1
}
"#;
    assert_eq!(errors(src), Vec::new());
}

// ---------- errors ----------

#[test]
fn let_type_mismatch_is_error() {
    let src = "agent Test\n\nfn f() -> int {\n    let x: int = \"hi\"\n    return 1\n}\n";
    let diags = errors(src);
    assert_eq!(
        diags.len(),
        1,
        "expected exactly one error, got {:?}",
        diags
    );
    assert_eq!(diags[0].kind, DiagKind::TypeMismatch);
    assert_eq!(diags[0].span.start.line, 4);
}

#[test]
fn return_type_mismatch_is_error() {
    let src = "agent Test\n\nfn f() -> int {\n    return \"nope\"\n}\n";
    let diags = errors(src);
    assert!(has(&diags, DiagKind::TypeMismatch), "got {:?}", diags);
}

#[test]
fn arithmetic_on_string_and_int_is_error() {
    let src = "agent Test\n\nfn f() -> int {\n    let x = \"a\" - 1\n    return 1\n}\n";
    let diags = errors(src);
    assert!(has(&diags, DiagKind::InvalidOperand), "got {:?}", diags);
}

#[test]
fn adding_bool_to_int_is_error() {
    let src = "agent Test\n\nfn f() -> int {\n    let x = 1 + true\n    return 1\n}\n";
    let diags = errors(src);
    assert!(has(&diags, DiagKind::InvalidOperand), "got {:?}", diags);
}

#[test]
fn logical_and_requires_bool() {
    let src = "agent Test\n\nfn f() -> bool {\n    return 1 and true\n}\n";
    let diags = errors(src);
    assert!(has(&diags, DiagKind::InvalidOperand), "got {:?}", diags);
}

#[test]
fn not_requires_bool() {
    let src = "agent Test\n\nfn f() -> bool {\n    return not 5\n}\n";
    let diags = errors(src);
    assert!(has(&diags, DiagKind::InvalidOperand), "got {:?}", diags);
}

#[test]
fn call_arity_mismatch_is_error() {
    let src = r#"agent Test

fn g(a: int) -> int {
    return a
}

fn f() -> int {
    let y = g()
    return 1
}
"#;
    let diags = errors(src);
    assert!(has(&diags, DiagKind::ArityMismatch), "got {:?}", diags);
}

#[test]
fn call_argument_type_mismatch_is_error() {
    let src = r#"agent Test

fn g(a: int) -> int {
    return a
}

fn f() -> int {
    let y = g("hi")
    return 1
}
"#;
    let diags = errors(src);
    assert!(has(&diags, DiagKind::TypeMismatch), "got {:?}", diags);
}

#[test]
fn assign_type_mismatch_is_error() {
    let src = "agent Test\n\nfn f() -> int {\n    let x: int = 1\n    x = \"s\"\n    return x\n}\n";
    let diags = errors(src);
    assert!(has(&diags, DiagKind::TypeMismatch), "got {:?}", diags);
}

// ---------- Result and the `?` operator ----------

#[test]
fn ok_and_err_are_typed() {
    // `ok(1)?` is an int, so binding it to a string must fail with "got int".
    let src = "agent Test\n\nfn f() -> int {\n    let v: string = ok(1)?\n    return 1\n}\n";
    let diags = diags(src);
    assert!(has(&diags, DiagKind::TypeMismatch), "got {:?}", diags);
    assert!(
        diags.iter().any(|d| d.message.contains("got int")),
        "the Ok payload type should be inferred: {:?}",
        diags
    );
}

#[test]
fn result_return_type_accepts_ok_and_err() {
    let ok_src = "agent Test\n\nfn f() -> Result(int, string) {\n    return ok(1)\n}\n";
    assert_eq!(diags(ok_src), Vec::new());

    let err_src = "agent Test\n\nfn f() -> Result(int, string) {\n    return err(\"x\")\n}\n";
    assert_eq!(diags(err_src), Vec::new());
}

#[test]
fn result_return_type_rejects_a_plain_value() {
    let src = "agent Test\n\nfn f() -> Result(int, string) {\n    return 1\n}\n";
    let diags = errors(src);
    assert!(has(&diags, DiagKind::TypeMismatch), "got {:?}", diags);
}

#[test]
fn result_constructor_arity_is_checked() {
    let src = "agent Test\n\nfn f() -> Result(int, string) {\n    return ok()\n}\n";
    let diags = errors(src);
    assert!(has(&diags, DiagKind::ArityMismatch), "got {:?}", diags);
}

#[test]
fn is_ok_returns_a_bool() {
    let src = "agent Test\n\nfn f() -> bool {\n    return is_ok(ok(1))\n}\n";
    assert_eq!(diags(src), Vec::new());
}

#[test]
fn try_on_a_non_result_is_rejected() {
    let src = "agent Test\n\nfn f() -> int {\n    let v = 1?\n    return v\n}\n";
    assert!(has(&errors(src), DiagKind::TypeMismatch));
}

// ---------- optional types ----------

#[test]
fn none_satisfies_an_optional_type() {
    let src = "agent Test\n\nfn f() -> int? {\n    return none\n}\n";
    assert_eq!(diags(src), Vec::new());
}

#[test]
fn optional_type_accepts_the_inner_type() {
    let src = "agent Test\n\nfn f() -> int {\n    let x: int? = 5\n    return 1\n}\n";
    assert_eq!(diags(src), Vec::new());
}

#[test]
fn optional_type_rejects_an_unrelated_type() {
    let src = "agent Test\n\nfn f() -> int {\n    let x: int? = \"s\"\n    return 1\n}\n";
    let diags = errors(src);
    assert!(has(&diags, DiagKind::TypeMismatch), "got {:?}", diags);
}

// ---------- lambdas ----------

#[test]
fn lambda_params_are_in_scope_in_the_body() {
    let src = "agent Test\n\nfn f() -> int {\n    let g = x => x + 1\n    return g(1)\n}\n";
    assert_eq!(diags(src), Vec::new());
}

#[test]
fn lambda_body_is_type_checked() {
    let src = "agent Test\n\nfn f() -> int {\n    let g = x => zzz + x\n    return 1\n}\n";
    let diags = diags(src);
    assert!(has(&diags, DiagKind::UndeclaredVariable), "got {:?}", diags);
}

#[test]
fn calling_a_lambda_binding_is_not_an_error() {
    let src = "agent Test\n\nfn f() -> int {\n    let g = x => x\n    return g(1)\n}\n";
    let diags = errors(src);
    assert!(!has(&diags, DiagKind::NotCallable), "got {:?}", diags);
}

#[test]
fn a_lambda_has_function_type() {
    let src =
        "agent Test\n\nfn f() -> int {\n    let g = x => x\n    let h: int = g\n    return 1\n}\n";
    let diags = errors(src);
    assert!(has(&diags, DiagKind::TypeMismatch), "got {:?}", diags);
}

#[test]
fn a_lambda_satisfies_the_function_type() {
    let src = "agent Test\n\nfn apply(f: function, v: int) -> function {\n    return f\n}\n\nfn go() -> int {\n    let g = apply(x => x + 1, 1)\n    return g(1)\n}\n";
    assert_eq!(diags(src), Vec::new());
}

#[test]
fn a_function_type_rejects_a_non_function() {
    let src = "agent Test\n\nfn apply(f: function) -> int {\n    return 1\n}\n\nfn go() -> int {\n    return apply(42)\n}\n";
    let diags = errors(src);
    assert!(has(&diags, DiagKind::TypeMismatch), "got {:?}", diags);
}

// ---------- let / var mutability ----------
#[test]
fn assign_to_let_is_error() {
    let src = "agent Test\n\nfn f() -> int {\n    let x = 1\n    x = 2\n    return x\n}\n";
    let diags = errors(src);
    assert!(has(&diags, DiagKind::AssignToImmutable), "got {:?}", diags);
}

#[test]
fn assign_to_var_is_accepted() {
    let src = "agent Test\n\nfn f() -> int {\n    var x = 1\n    x = 2\n    return x\n}\n";
    assert_eq!(diags(src), Vec::new());
}

#[test]
fn assign_to_var_still_checks_types() {
    let src = "agent Test\n\nfn f() -> int {\n    var x: int = 1\n    x = \"s\"\n    return x\n}\n";
    let diags = errors(src);
    assert!(has(&diags, DiagKind::TypeMismatch), "got {:?}", diags);
    assert!(
        !has(&diags, DiagKind::AssignToImmutable),
        "`var` is mutable: {:?}",
        diags
    );
}

#[test]
fn compound_assign_to_let_is_error() {
    let src = "agent Test\n\nfn f() -> int {\n    let x = 1\n    x += 1\n    return x\n}\n";
    let diags = errors(src);
    assert!(has(&diags, DiagKind::AssignToImmutable), "got {:?}", diags);
}

#[test]
fn function_parameters_are_mutable() {
    let src = "agent Test\n\nfn f(x: int) -> int {\n    x = x + 1\n    return x\n}\n";
    assert_eq!(diags(src), Vec::new());
}

#[test]
fn loop_variable_is_immutable() {
    let src = "agent Test\n\nfn f() -> int {\n    for it in [1, 2] {\n        it = 0\n    }\n    return 0\n}\n";
    let diags = errors(src);
    assert!(has(&diags, DiagKind::AssignToImmutable), "got {:?}", diags);
}

#[test]
fn calling_a_non_function_is_error() {
    let src =
        "agent Test\n\nfn f() -> int {\n    let x: int = 1\n    let y = x()\n    return x\n}\n";
    let diags = errors(src);
    assert!(has(&diags, DiagKind::NotCallable), "got {:?}", diags);
}

#[test]
fn return_inside_match_block_is_checked() {
    let src = r#"agent Test

fn f(n: int) -> int {
    match n {
        0 -> { return "zero" }
        _ -> { return 1 }
    }
}
"#;
    let diags = errors(src);
    assert!(has(&diags, DiagKind::TypeMismatch), "got {:?}", diags);
}

// ---------- warnings ----------

#[test]
fn undeclared_variable_is_warning_not_error() {
    let src = "agent Test\n\nfn f() -> int {\n    return zzz\n}\n";
    let diags = diags(src);
    assert_eq!(errors(src), Vec::new(), "must not be a hard error");
    assert!(has(&diags, DiagKind::UndeclaredVariable), "got {:?}", diags);
    assert!(!diags[0].is_error());
}

#[test]
fn declared_variable_is_not_flagged() {
    let src = "agent Test\n\nfn f() -> int {\n    let zzz = 5\n    return zzz\n}\n";
    assert_eq!(diags(src), Vec::new());
}

/// Guard: every name the runtime actually accepts must be known to the checker,
/// otherwise real built-in calls emit a false "undeclared variable" warning.
#[test]
fn builtin_calls_are_not_flagged_as_undeclared() {
    let src = r#"agent Test

fn f(s: string) -> string {
    print(s)
    let bare = [upper(s), lower(s), trim(s), repeat(s, 2), char_at(s, 0)]
    let strs = [split(s, ","), join(bare, "-"), replace(s, "a", "b"), str(1)]
    let predicates = [contains(s, "x"), starts_with(s, "a"), ends_with(s, "z"), has(bare, s)]
    let nums = [int("1"), float("1.5"), len(bare), abs(-1), min(1, 2), max(1, 2)]
    let arrs = [push(bare, s), push_to(bare, s), pop(bare), sort(bare), reverse(bare)]
    let picks = [first(bare), last(bare), slice(bare, 0, 1), range(0, 3), keys(bare), values(bare)]
    let math = [sqrt(4), pow(2, 3), pi, e, sin(1), cos(1), tan(1)]
    let math2 = [log(1), log10(1), exp(1), floor(1), ceil(1), round(1), random(), random_int(1, 5)]
    let time = [now(), now_ms(), time_now_ms(), time_now_sec(), math_pi, math_sin(1), math_floor(1)]
    let proc = [read_line(), exit(0), args()]
    let files = [file_read(s), file_write(s, s), file_exists(s), file_delete(s), ls(s)]
    let files2 = [file_append(s, s), file_copy(s, s), file_mkdir(s), file_size(s), file_list_dir(s)]
    let net = [http_get(s), http_post(s, s), http_put(s, s), http_delete(s)]
    let data = [json_parse(s), json_stringify(bare), env_get(s), env_set(s, s)]
    let sys = [sys_exit(0), sys_args(), sys_info(), sys_env_all()]
    let ext = [shell_run(s), regex_match(s, s), regex_find(s, s), regex_find_all(s, s), regex_replace(s, s, s)]
    let crypto = [md5(s), sha256(s), sha512(s), crypto_md5(s), crypto_sha256(s)]
    let crypto2 = [base64_encode(s), base64_decode(s), b64_encode(s), b64_decode(s)]
    let ids = [uuid(), uuid_v4()]
    let mem = [memory_open(s), memory_add(s, s, s), memory_get(s), memory_clear(s), memory_count(s)]
    let llm = llm_complete(s)
    return s
}
"#;
    assert_eq!(diags(src), Vec::new());
}

/// Dotted and underscored spellings are both valid at runtime.
#[test]
fn namespaced_builtin_calls_are_not_flagged_as_undeclared() {
    let src = r#"agent Test

fn f(s: string) -> string {
    let a = file.read(s)
    let b = file.exists(s)
    let c = env.get(s)
    let d = env.set(s, s)
    let e = http.get(s)
    let g = json.parse(s)
    let h = json.stringify(s)
    let i = time.now_ms()
    let j = time.sleep(1)
    let k = sys.info()
    let l = sys.exit(0)
    let m = math.sin(1)
    let n = math.floor(1)
    let o = crypto.md5(s)
    let p = regex.find(s, s)
    let q = shell.run(s)
    let r = uuid.v4()
    let t = llm.complete(s)
    let u = memory.open(s)
    let v = memory.add(s, s, s)
    let w = memory.get(s)
    let x = memory.clear(s)
    let y = memory.count(s)
    return s
}
"#;
    assert_eq!(diags(src), Vec::new());
}

// ---------- positions ----------

#[test]
fn diagnostics_carry_line_and_column() {
    let src = "agent Test\n\nfn f() -> int {\n    let x: int = \"hi\"\n    return 1\n}\n";
    let diags = errors(src);
    assert_eq!(diags[0].span.start.line, 4);
    assert!(diags[0].span.start.column > 1);
    // the message should contain the variable name
    assert!(
        diags[0].message.contains('x'),
        "message: {}",
        diags[0].message
    );
}

#[test]
fn ui_state_type_mismatch_is_reported() {
    let src = r#"agent Test
ui Main = Screen "Test" {
    @count: int = "wrong"
    Text count
}
"#;
    assert!(has(&errors(src), DiagKind::TypeMismatch));
}

#[test]
fn nominal_ui_values_are_rejected_instead_of_silently_dropped() {
    let src = r#"agent Test
struct User { name: string }
ui Main = Screen "Test" {
    @user: User = User(name: "Ada")
    Text "hello"
}
"#;
    let diagnostics = errors(src);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("not supported in UI")),
        "got {:?}",
        diagnostics
    );
}

#[test]
fn ui_binding_and_component_props_are_validated() {
    let src = r#"agent Test
component Card {
    prop title: string
    render { Text title }
}
ui Main = Screen "Test" {
    Input placeholder: "type" bind value to missing
    Card
}
"#;
    let diagnostics = errors(src);
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.kind == DiagKind::InvalidUi));
}

#[test]
fn unknown_functions_are_errors() {
    let src = "agent Test\nfn f() -> int { return missing_function() }\n";
    assert!(has(&errors(src), DiagKind::UnknownFunction));
}

#[test]
fn named_arguments_are_checked_by_name() {
    let src = r#"agent Test
fn pair(a: int, b: int) -> int { return a + b }
fn f() -> int { return pair(b: 2, a: 1) }
"#;
    assert_eq!(errors(src), Vec::new());
}

#[test]
fn struct_and_enum_types_flow_through_declarations_aliases_and_members() {
    let src = r#"agent Test

struct User {
    name: string
    age: int
    role: Role
}

enum Role {
    Admin
    Suspended(string)
}

type MaybeUser = User?
type Users = [User]

fn make(role: Role) -> User {
    return User(name: "Ada", age: 42, role: role)
}

fn inspect(user: User, users: [User], maybe: User?) -> string {
    let created: User = User(name: "Ada", age: 42, role: Role.Admin)
    let optional: MaybeUser = created
    let all: Users = [created]
    let first: User = users[0]
    let role: Role = Role.Admin
    let suspended: Role = Role.Suspended("x")
    let name: string = created.name
    let current_role: Role = created.role
    return name
}
"#;
    assert_eq!(errors(src), Vec::new());
}

#[test]
fn unknown_named_types_are_errors() {
    let src = r#"agent Test
fn passthrough(value: Missing) -> Missing {
    return value
}
"#;
    let diagnostics = errors(src);
    assert!(has(&diagnostics, DiagKind::TypeMismatch), "got {:?}", diagnostics);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("unknown named type \"Missing\"")),
        "got {:?}",
        diagnostics
    );
    assert!(diagnostics.iter().all(|diagnostic| diagnostic.span.start.offset > 0));
}

#[test]
fn duplicate_type_names_are_errors() {
    let src = r#"agent Test
struct Status {
    code: int
}
enum Status {
    Ready
}
"#;
    let diagnostics = errors(src);
    assert!(
        has(&diagnostics, DiagKind::DuplicateDeclaration),
        "got {:?}",
        diagnostics
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("duplicate type name")),
        "got {:?}",
        diagnostics
    );
}

#[test]
fn duplicate_struct_fields_are_errors() {
    let src = r#"agent Test
struct User {
    name: string
    name: string
}
"#;
    let diagnostics = errors(src);
    assert!(
        has(&diagnostics, DiagKind::DuplicateDeclaration),
        "got {:?}",
        diagnostics
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("duplicate field")),
        "got {:?}",
        diagnostics
    );
}

#[test]
fn duplicate_enum_variants_are_errors() {
    let src = r#"agent Test
enum Role {
    Admin
    Admin
}
"#;
    let diagnostics = errors(src);
    assert!(
        has(&diagnostics, DiagKind::DuplicateDeclaration),
        "got {:?}",
        diagnostics
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("duplicate variant")),
        "got {:?}",
        diagnostics
    );
}

#[test]
fn function_and_type_name_collisions_are_errors() {
    let src = r#"agent Test
struct User {
    name: string
}
fn User() -> int {
    return 1
}
"#;
    let diagnostics = errors(src);
    assert!(
        has(&diagnostics, DiagKind::DuplicateDeclaration),
        "got {:?}",
        diagnostics
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("collides with function")),
        "got {:?}",
        diagnostics
    );
}

#[test]
fn nominal_struct_rejects_structurally_equal_objects() {
    let src = r#"agent Test
struct User {
    name: string
}
fn make() -> User {
    return { name: "Ada" }
}
"#;
    let diagnostics = errors(src);
    assert!(
        has(&diagnostics, DiagKind::TypeMismatch),
        "got {:?}",
        diagnostics
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("expected User")),
        "got {:?}",
        diagnostics
    );
}

#[test]
fn struct_constructor_validates_named_fields() {
    let src = r#"agent Test
struct User {
    name: string
    age: int
}
fn make() {
    let first = User(name: "Ada", name: "Lin", age: "old", extra: true)
    let second = User("Ada")
}
"#;
    let diagnostics = errors(src);
    assert!(
        has(&diagnostics, DiagKind::TypeMismatch),
        "got {:?}",
        diagnostics
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("provided more than once")),
        "got {:?}",
        diagnostics
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("unknown field")),
        "got {:?}",
        diagnostics
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("requires named arguments")),
        "got {:?}",
        diagnostics
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("missing field")),
        "got {:?}",
        diagnostics
    );
}

#[test]
fn enum_expression_forms_are_checked() {
    let src = r#"agent Test
enum Role {
    Admin
    Suspended(string)
}
fn make() {
    let missing = Role.Missing
    let extra = Role.Admin(1)
    let bare_payload = Role.Suspended
    let wrong_payload = Role.Suspended(1)
    let unit = Role.Admin
    let payload = Role.Suspended("x")
}
"#;
    let diagnostics = errors(src);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("has no variant")),
        "got {:?}",
        diagnostics
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("does not take a payload")),
        "got {:?}",
        diagnostics
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("requires a payload")),
        "got {:?}",
        diagnostics
    );
    assert!(
        has(&diagnostics, DiagKind::TypeMismatch),
        "got {:?}",
        diagnostics
    );
}

#[test]
fn struct_member_access_is_typed() {
    let src = r#"agent Test
struct User {
    name: string
    age: int
}
fn inspect(user: User) -> string {
    let missing = user.missing
    let wrong: string = user.age
    return user.name
}
"#;
    let diagnostics = errors(src);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("has no field")),
        "got {:?}",
        diagnostics
    );
    assert!(
        has(&diagnostics, DiagKind::TypeMismatch),
        "got {:?}",
        diagnostics
    );
}

#[test]
fn enum_match_patterns_bind_and_type_payloads() {
    let src = r#"agent Test
enum Role {
    Admin
    Suspended(string)
}
fn describe(role: Role) -> string {
    return match role {
        Role.Admin -> "admin"
        Role.Suspended(reason) -> reason
    }
}
"#;
    assert_eq!(errors(src), Vec::new());
}

#[test]
fn enum_payload_binding_uses_the_declared_type() {
    let src = r#"agent Test
enum Role {
    Suspended(string)
}
fn invalid(role: Role) -> int {
    return match role {
        Role.Suspended(reason) -> reason
    }
}
"#;
    let diagnostics = errors(src);
    assert!(
        has(&diagnostics, DiagKind::TypeMismatch),
        "got {:?}",
        diagnostics
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("expected int, got string")),
        "got {:?}",
        diagnostics
    );
}

#[test]
fn enum_match_pattern_shape_and_payload_types_are_checked() {
    let src = r#"agent Test
enum Role {
    Admin
    Suspended(string)
}
fn describe(role: Role) -> int {
    return match role {
        Role.Admin(reason) -> 1
        Role.Missing -> 2
        Role.Suspended -> 3
        Role.Suspended(reason) -> reason
    }
}
"#;
    let diagnostics = errors(src);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("has no payload")),
        "got {:?}",
        diagnostics
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("has no variant")),
        "got {:?}",
        diagnostics
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("requires a payload pattern")),
        "got {:?}",
        diagnostics
    );
    assert!(
        has(&diagnostics, DiagKind::TypeMismatch),
        "got {:?}",
        diagnostics
    );
}

#[test]
fn enum_match_pattern_type_must_match_scrutinee() {
    let src = r#"agent Test
struct User {
    name: string
}
enum Role {
    Admin
}
fn invalid(user: User) -> int {
    return match user {
        Role.Admin -> 1
    }
}
"#;
    let diagnostics = errors(src);
    assert!(
        has(&diagnostics, DiagKind::TypeMismatch),
        "got {:?}",
        diagnostics
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot match")),
        "got {:?}",
        diagnostics
    );
}

#[test]
fn struct_match_patterns_bind_declared_fields() {
    let src = r#"agent Test
struct User {
    name: string
    age: int
}
fn describe(user: User) -> string {
    return match user {
        User { name: name, age: age } -> name + str(age)
    }
}
"#;
    assert_eq!(errors(src), Vec::new());
}

#[test]
fn invalid_struct_match_patterns_are_reported() {
    let src = r#"agent Test
struct User {
    name: string
}
fn describe(user: User) -> int {
    return match user {
        User { missing: value } -> 1
    }
}
"#;
    let diagnostics = errors(src);
    assert!(has(&diagnostics, DiagKind::InvalidOperand), "got {:?}", diagnostics);
}

#[test]
fn legacy_match_bindings_remain_typed() {
    let src = r#"agent Test
fn literal(value: int) -> int {
    return match value {
        1 -> 1
        other -> other
        _ -> 0
    }
}
fn optional(value: int?) -> int {
    return match value {
        some(inner) -> inner
        none -> 0
    }
}
"#;
    assert_eq!(diags(src), Vec::new());
}

#[test]
fn public_type_paths_resolve_to_scoped_identity() {
    let src = r#"agent Test
import "types.aec" as types
pub struct User {
    name: string
}
fn make() -> types.User {
    return types.User(name: "Ada")
}
"#;
    let mut program = aec_parser::parse(src).expect("source should parse");
    program.item_modules[1] = Some("types".to_string());
    program.imports.push(ResolvedImport {
        alias: Some("types".to_string()),
        path: "types.aec".to_string(),
        exports: vec!["User".to_string()],
    });
    assert_eq!(check_program(&program), Vec::new());
}

#[test]
fn private_type_paths_are_rejected() {
    let src = r#"agent Test
import "types.aec" as types
struct User {
    name: string
}
fn make() -> types.User {
    return types.User(name: "Ada")
}
"#;
    let mut program = aec_parser::parse(src).expect("source should parse");
    program.item_modules[1] = Some("types".to_string());
    program.imports.push(ResolvedImport {
        alias: Some("types".to_string()),
        path: "types.aec".to_string(),
        exports: Vec::new(),
    });
    let diagnostics = check_program(&program)
        .into_iter()
        .filter(|diagnostic| diagnostic.is_error())
        .collect::<Vec<_>>();
    assert!(!diagnostics.is_empty(), "expected private type diagnostics");
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.message.contains("is private to its module")),
        "got {:?}",
        diagnostics
    );
}

#[test]
fn nested_optional_array_and_result_types_are_checked() {
    let src = r#"agent Test
fn make(value: [int?]) -> Result([int?], string?) {
    return ok(value)
}
fn wrap(value: Result(int?, string)) -> Result([Result(int?, string)], Result(string, [int?])) {
    return ok([value])
}
"#;
    assert_eq!(errors(src), Vec::new());
}

#[test]
fn nested_optional_result_mismatches_are_reported() {
    let src = r#"agent Test
fn make(value: [int?]) -> Result([string?], string) {
    return ok(value)
}
"#;
    assert!(has(&errors(src), DiagKind::TypeMismatch));
}

#[test]
fn result_branches_are_joined_without_losing_their_types() {
    let src = r#"agent Test
fn choose(flag: bool) -> Result(int, string) {
    let result = match flag {
        true -> ok(1)
        false -> err("x")
    }
    let invalid: Result(string, int) = result
    return result
}
"#;
    assert!(has(&errors(src), DiagKind::TypeMismatch));
}

#[test]
fn optional_branches_are_joined_without_losing_their_types() {
    let src = r#"agent Test
fn choose(flag: bool) -> int {
    let value = match flag {
        true -> 1
        false -> none
    }
    let invalid: int = value
    return value
}
"#;
    assert!(has(&errors(src), DiagKind::TypeMismatch));
}

#[test]
fn enum_match_must_cover_every_variant() {
    let src = r#"agent Test
enum Role {
    Admin
    Member
}
fn describe(role: Role) -> int {
    return match role {
        Role.Admin -> 1
    }
}
"#;
    let diagnostics = errors(src);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("non-exhaustive match")),
        "got {:?}",
        diagnostics
    );
}

#[test]
fn optional_match_must_cover_some_and_none() {
    let src = r#"agent Test
fn read(value: int?) -> int {
    return match value {
        some(inner) -> inner
    }
}
"#;
    let diagnostics = errors(src);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("non-exhaustive match")),
        "got {:?}",
        diagnostics
    );
}
