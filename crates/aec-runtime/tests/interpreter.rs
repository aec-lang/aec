//! Interpreter tests: evaluation of statements and expressions end to end
//! (source -> parser -> interpreter), independent of the UI.

use aec_ast::Span;
use aec_parser::parse;
use aec_runtime::{Interpreter, RuntimeError, Value};

/// Runs `fn_name` with the given arguments.
fn try_call(source: &str, fn_name: &str, args: Vec<Value>) -> Result<Value, RuntimeError> {
    let program = parse(source).expect("source should parse");
    let mut interp = Interpreter::new();
    interp.run(&program).expect("program should load");
    interp.call_function(fn_name, args, Span::dummy())
}

/// Runs `fn_name` with the given arguments and returns the value.
fn call(source: &str, fn_name: &str, args: Vec<Value>) -> Value {
    try_call(source, fn_name, args).expect("call should succeed")
}

/// `Value` has no `PartialEq` (it holds functions), so unwrap the simple cases.
fn as_int(v: Value) -> i64 {
    match v {
        Value::Int(n) => n,
        other => panic!("expected int, got {:?}", other),
    }
}

fn as_string(v: Value) -> String {
    match v {
        Value::String(s) => s,
        other => panic!("expected string, got {:?}", other),
    }
}

fn as_ok(v: Value) -> Value {
    match v {
        Value::Result(Ok(inner)) => *inner,
        other => panic!("expected ok(...), got {:?}", other),
    }
}

fn as_err(v: Value) -> Value {
    match v {
        Value::Result(Err(inner)) => *inner,
        other => panic!("expected err(...), got {:?}", other),
    }
}

#[test]
fn var_accumulates_across_loop_iterations() {
    let src = r#"agent Test

fn sum_to(n: int) -> int {
    var total = 0
    var i = 1
    while i <= n {
        total += i
        i += 1
    }
    return total
}
"#;
    assert_eq!(as_int(call(src, "sum_to", vec![Value::Int(5)])), 15);
}

#[test]
fn var_reassignment_holds_the_last_value() {
    let src = r#"agent Test

fn label(flag: bool) -> string {
    var out = "off"
    if flag {
        out = "on"
    }
    return out
}
"#;
    assert_eq!(as_string(call(src, "label", vec![Value::Bool(true)])), "on");
    assert_eq!(as_string(call(src, "label", vec![Value::Bool(false)])), "off");
}

#[test]
fn var_string_append_builds_a_message() {
    let src = r#"agent Test

fn greet(name: string) -> string {
    var msg = "hello "
    msg += name
    return msg
}
"#;
    assert_eq!(
        as_string(call(src, "greet", vec![Value::String("AEC".to_string())])),
        "hello AEC"
    );
}

#[test]
fn parameters_are_reassignable_locals() {
    let src = r#"agent Test

fn bump(x: int) -> int {
    x += 1
    return x
}
"#;
    assert_eq!(as_int(call(src, "bump", vec![Value::Int(41)])), 42);
}

// ---------- string interpolation ----------

#[test]
fn interpolation_inserts_variable_values() {
    let src = r#"agent Test

fn label(name: string, n: int) -> string {
    return "hi {name}, {n} left"
}
"#;
    let got = call(
        src,
        "label",
        vec![Value::String("Ada".to_string()), Value::Int(3)],
    );
    assert_eq!(as_string(got), "hi Ada, 3 left");
}

#[test]
fn interpolation_evaluates_an_expression() {
    let src = r#"agent Test

fn total(a: int) -> string {
    return "total: {a + 1}"
}
"#;
    assert_eq!(as_string(call(src, "total", vec![Value::Int(41)])), "total: 42");
}

#[test]
fn interpolation_unpacks_an_object_text_field() {
    let src = r#"agent Test

fn reply() -> string {
    let r = { text: "pong", model: "x" }
    return "answer: {r}"
}
"#;
    assert_eq!(as_string(call(src, "reply", vec![])), "answer: pong");
}

#[test]
fn plain_braces_are_not_interpolated() {
    let src = r#"agent Test

fn payload() -> string {
    return "{ \"a\": 1 }"
}
"#;
    assert_eq!(as_string(call(src, "payload", vec![])), "{ \"a\": 1 }");
}

// ---------- literal evaluation ----------

#[test]
fn raw_string_evaluates_to_a_string() {
    let src = "agent Test\n\nfn f() -> string {\n    return r\"raw\\n\"\n}\n";
    assert_eq!(as_string(call(src, "f", vec![])), "raw\\n");
}

#[test]
fn duration_literal_evaluates_to_milliseconds() {
    let src = "agent Test\n\nfn f() -> int {\n    return 2s\n}\n";
    assert_eq!(as_int(call(src, "f", vec![])), 2000);
}

#[test]
fn byte_size_literal_evaluates_to_bytes() {
    let src = "agent Test\n\nfn f() -> int {\n    return 2KB\n}\n";
    assert_eq!(as_int(call(src, "f", vec![])), 2048);
}

// ---------- Result and the `?` operator ----------

#[test]
fn ok_unwraps_with_the_try_operator() {
    let src = r#"agent Test

fn f() -> Result(int, string) {
    let v = ok(41)?
    return ok(v + 1)
}
"#;
    assert_eq!(as_int(as_ok(call(src, "f", vec![]))), 42);
}

#[test]
fn try_unwraps_nested_results() {
    let src = r#"agent Test

fn f() -> Result(int, string) {
    let v = ok(ok(7)?)?
    return ok(v)
}
"#;
    assert_eq!(as_int(as_ok(call(src, "f", vec![]))), 7);
}

#[test]
fn err_propagates_to_the_caller() {
    let src = r#"agent Test

fn inner() -> Result(int, string) {
    return err("boom")
}

fn outer() -> Result(int, string) {
    let v = inner()?
    return ok(v)
}
"#;
    assert_eq!(as_string(as_err(call(src, "outer", vec![]))), "boom");
}

#[test]
fn propagation_stops_the_rest_of_the_function() {
    let src = r#"agent Test

fn inner() -> Result(int, string) {
    return err("boom")
}

fn outer() -> Result(int, string) {
    let a = inner()?
    let b = ok(1)?
    return ok(a + b)
}
"#;
    // If `?` did not unwind, the function would return ok(1).
    assert_eq!(as_string(as_err(call(src, "outer", vec![]))), "boom");
}

#[test]
fn try_on_a_non_result_is_a_type_error() {
    let src = r#"agent Test

fn f() -> Result(int, string) {
    let v = 42?
    return ok(v)
}
"#;
    let err = try_call(src, "f", vec![]).expect_err("42? must fail");
    assert!(
        format!("{}", err).contains("expected a result"),
        "unexpected message: {}",
        err
    );
}

#[test]
fn is_ok_and_is_err_predicates() {
    let src = r#"agent Test

fn on_err(n: int) -> bool {
    return is_err(err(n))
}

fn on_ok(n: int) -> bool {
    return is_ok(ok(n))
}

fn wrong(n: int) -> bool {
    return is_ok(err(n))
}
"#;
    assert!(matches!(call(src, "on_err", vec![Value::Int(1)]), Value::Bool(true)));
    assert!(matches!(call(src, "on_ok", vec![Value::Int(1)]), Value::Bool(true)));
    assert!(matches!(
        call(src, "wrong", vec![Value::Int(1)]),
        Value::Bool(false)
    ));
}

#[test]
fn results_render_in_interpolation() {
    let src = r#"agent Test

fn f() -> string {
    return "r={ok(1)} e={err(\"x\")}"
}
"#;
    assert_eq!(as_string(call(src, "f", vec![])), "r=ok(1) e=err(x)");
}

// ---------- lambdas / closures ----------

#[test]
fn lambda_called_through_a_variable() {
    let src = r#"agent Test

fn f(n: int) -> int {
    let double = x => x * 2
    return double(n)
}
"#;
    assert_eq!(as_int(call(src, "f", vec![Value::Int(21)])), 42);
}

#[test]
fn lambda_with_two_params() {
    let src = r#"agent Test

fn f(a: int, b: int) -> int {
    let mul = x, y => x * y
    return mul(a, b)
}
"#;
    assert_eq!(
        as_int(call(src, "f", vec![Value::Int(6), Value::Int(7)])),
        42
    );
}

#[test]
fn lambda_with_no_params() {
    let src = r#"agent Test

fn f() -> int {
    let answer = => 42
    return answer()
}
"#;
    assert_eq!(as_int(call(src, "f", vec![])), 42);
}

/// `var base = 10`, then the lambda is created, then `base` changes to 100.
/// A by-move capture keeps the value seen at creation time.
#[test]
fn lambda_captures_by_move() {
    let src = r#"agent Test

fn f(start: int) -> int {
    var base = start
    let add = x => x + base
    base = 100
    return add(1)
}
"#;
    assert_eq!(as_int(call(src, "f", vec![Value::Int(10)])), 11);
}

#[test]
fn lambda_passed_to_a_user_function() {
    let src = r#"agent Test

fn apply(f: int, v: int) -> int {
    return f(v)
}

fn go() -> int {
    return apply(x => x + 5, 10)
}
"#;
    assert_eq!(as_int(call(src, "go", vec![])), 15);
}

#[test]
fn lambda_returned_from_a_function() {
    let src = r#"agent Test

fn make_adder(n: int) -> unit {
    return x => x + n
}

fn go() -> int {
    let add5 = make_adder(5)
    return add5(10)
}
"#;
    assert_eq!(as_int(call(src, "go", vec![])), 15);
}

/// The captured environment still reaches globals, so a lambda can call a
/// top-level function.
#[test]
fn lambda_can_call_a_top_level_function() {
    let src = r#"agent Test

fn twice(n: int) -> int {
    return n * 2
}

fn f(v: int) -> int {
    let g = x => twice(x + 1)
    return g(v)
}
"#;
    assert_eq!(as_int(call(src, "f", vec![Value::Int(4)])), 10);
}

#[test]
fn lambda_with_the_wrong_arg_count_errors() {
    let src = r#"agent Test

fn f() -> int {
    let g = => 1
    return g(1)
}
"#;
    let err = try_call(src, "f", vec![]).expect_err("g(1) must fail");
    assert!(
        format!("{}", err).contains("expected 0, got 1"),
        "unexpected message: {}",
        err
    );
}

#[test]
fn try_inside_a_lambda_returns_the_error_from_the_lambda() {
    let src = r#"agent Test

fn f() -> Result(int, string) {
    let unwrap_or_err = r => r?
    let v = unwrap_or_err(ok(7))
    return ok(v)
}
"#;
    // `err(...)` inside the lambda becomes the lambda's result, not an unwind
    // of `f` itself.
    let src2 = r#"agent Test

fn f() -> Result(int, string) {
    let unwrap_or_err = r => r?
    return unwrap_or_err(err("inner"))
}
"#;
    assert_eq!(as_int(as_ok(call(src, "f", vec![]))), 7);
    assert_eq!(as_string(as_err(call(src2, "f", vec![]))), "inner");
}

#[test]
fn try_inside_a_match_arm_propagates() {
    let src = r#"agent Test

fn pick(n: int) -> Result(int, string) {
    match n {
        0 -> { return err("zero") }
        _ -> { return ok(n) }
    }
}

fn outer(n: int) -> Result(int, string) {
    let v = pick(n)?
    return ok(v * 2)
}
"#;
    assert_eq!(as_int(as_ok(call(src, "outer", vec![Value::Int(3)]))), 6);
    assert_eq!(as_string(as_err(call(src, "outer", vec![Value::Int(0)]))), "zero");
}

#[test]
fn let_binding_is_readable() {
    let src = r#"agent Test

fn double(n: int) -> int {
    let twice = n * 2
    return twice
}
"#;
    assert_eq!(as_int(call(src, "double", vec![Value::Int(21)])), 42);
}

#[test]
fn missing_builtin_arguments_return_errors_without_panicking() {
    let src = "agent Test\nfn f() -> string { return upper() }\n";
    let error = try_call(src, "f", vec![]).expect_err("upper() must fail");
    assert!(matches!(error, RuntimeError::WrongArgCount { .. }));
}

#[test]
fn default_and_named_arguments_are_bound() {
    let src = r#"agent Test

fn greet(name: string = "world", punctuation: string = "!") -> string {
    return "hello " + name + punctuation
}

fn f() -> string {
    return greet(punctuation: "?", name: "AEC")
}
"#;
    assert_eq!(as_string(call(src, "f", vec![])), "hello AEC?");
    assert_eq!(as_string(call(src, "greet", vec![])), "hello world!");
}

#[test]
fn nested_assignment_updates_objects_and_arrays() {
    let src = r#"agent Test

fn f() -> int {
    var item = { value: 1 }
    var values = [0, 1]
    item.value = 7
    values[1] = 9
    return item.value + values[1]
}
"#;
    assert_eq!(as_int(call(src, "f", vec![])), 16);
}

#[test]
fn match_patterns_bind_values() {
    let src = r#"agent Test

fn f(value: int) -> int {
    return match value {
        other -> other + 1
    }
}
"#;
    assert_eq!(as_int(call(src, "f", vec![Value::Int(4)])), 5);
}

#[test]
fn user_functions_can_shadow_builtin_names() {
    let src = r#"agent Test

fn first(values: [int]) -> int {
    return 99
}

fn f() -> int {
    return first([1, 2])
}
"#;
    assert_eq!(as_int(call(src, "f", vec![])), 99);
}

#[test]
fn integer_overflow_is_a_runtime_error() {
    let src = "agent Test\nfn f() -> int { return 9223372036854775807 + 1 }\n";
    let error = try_call(src, "f", vec![]).expect_err("overflow must fail");
    assert!(format!("{}", error).contains("overflow"));
}

#[test]
fn negative_repeat_is_rejected() {
    let src = "agent Test\nfn f() -> string { return repeat(\"x\", -1) }\n";
    let error = try_call(src, "f", vec![]).expect_err("negative repeat must fail");
    assert!(format!("{}", error).contains("non-negative"));
}

#[test]
fn top_level_functions_are_first_class_values() {
    let src = r#"agent Test
fn double(value: int) -> int { return value * 2 }
fn f() -> int {
    let callable = double
    return callable(21)
}
"#;
    assert_eq!(as_int(call(src, "f", vec![])), 42);
}

#[test]
fn higher_order_collection_helpers_accept_closures() {
    let src = r#"agent Test
fn f() -> int {
    let mapped = map([1, 2, 3], x => x * 2)
    let filtered = filter(mapped, (x => x > 2))
    let add = a, b => a + b
    return reduce(filtered, add, 0)
}
"#;
    assert_eq!(as_int(call(src, "f", vec![])), 10);
}
