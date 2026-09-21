//! End-to-end tests for `aec check` (parse + type check) via the real binary.

use std::path::PathBuf;
use std::process::Command;

fn tmp_file(name: &str, content: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("aec-cli-check-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let path = dir.join(name);
    std::fs::write(&path, content).expect("write temp source");
    path
}

fn run_check(path: &PathBuf) -> (bool, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_aec"))
        .arg("check")
        .arg(path)
        .output()
        .expect("run aec binary");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).to_string(),
    )
}

const VALID: &str = r#"agent Test

fn add(a: int, b: int) -> int {
    return a + b
}

fn main() -> int {
    let total = add(1, 2)
    if total > 0 and total < 10 {
        return total
    }
    return 0
}
"#;

const TYPE_ERROR: &str = r#"agent Test

fn add(a: int, b: int) -> int {
    return a + b
}

fn bad() -> int {
    let x: int = "hello"
    let y = add(1)
    return x
}
"#;

#[test]
fn check_passes_valid_program() {
    let path = tmp_file("valid.aec", VALID);
    let (ok, stdout) = run_check(&path);
    assert!(ok, "expected success, stdout:\n{}", stdout);
    assert!(stdout.contains("Parse successful"));
    assert!(stdout.contains("Type check passed"));
}

#[test]
fn check_fails_on_type_errors() {
    let path = tmp_file("bad.aec", TYPE_ERROR);
    let (ok, stdout) = run_check(&path);
    assert!(!ok, "expected failure, stdout:\n{}", stdout);
    assert!(stdout.contains("Type check failed"), "stdout:\n{}", stdout);
    assert!(stdout.contains("error"), "stdout:\n{}", stdout);
}

#[test]
fn check_reports_arity_mismatch() {
    let path = tmp_file("arity.aec", TYPE_ERROR);
    let (_, stdout) = run_check(&path);
    assert!(
        stdout.contains("argument"),
        "expected arity error message, stdout:\n{}",
        stdout
    );
}
