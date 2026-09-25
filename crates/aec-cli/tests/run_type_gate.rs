//! `aec run` must refuse to execute a program that has type errors.

use std::path::{Path, PathBuf};
use std::process::Command;

fn tmp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("aec-cli-rungate-{}-{}", std::process::id(), name));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn write_file(dir: &Path, name: &str, content: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, content).expect("write temp source");
    path
}

fn run_cli(args: &[&str], cwd: &Path) -> (bool, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_aec"))
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("run aec binary");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).to_string(),
    )
}

/// The program prints a marker before failing the type check; the gate must stop
/// it before the marker ever appears.
const TYPE_ERROR: &str = r#"agent Test

fn main() -> int {
    let x: int = "hello"
    print("RAN")
    return x
}
"#;

const HEALTHY: &str = r#"agent Test

fn main() -> int {
    print("RAN")
    return 0
}
"#;

#[test]
fn run_refuses_program_with_type_errors() {
    let dir = tmp_dir("bad");
    let path = write_file(&dir, "bad.aec", TYPE_ERROR);
    let (ok, stdout) = run_cli(&["run", path.to_str().unwrap(), "--cli", "--entry", "main"], &dir);

    assert!(!ok, "expected non-zero exit. stdout:\n{}", stdout);
    assert!(
        stdout.contains("Type check failed"),
        "the type gate did not run. stdout:\n{}",
        stdout
    );
    assert!(
        !stdout.contains("RAN"),
        "the program must not execute when it has type errors. stdout:\n{}",
        stdout
    );
}

#[test]
fn run_still_executes_healthy_program() {
    let dir = tmp_dir("ok");
    let path = write_file(&dir, "ok.aec", HEALTHY);
    let (ok, stdout) = run_cli(&["run", path.to_str().unwrap(), "--cli", "--entry", "main"], &dir);

    assert!(ok, "expected success. stdout:\n{}", stdout);
    assert!(stdout.contains("RAN"), "stdout:\n{}", stdout);
}
