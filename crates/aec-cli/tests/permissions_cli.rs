//! End-to-end tests for the permission sandbox through the real `aec` binary.
//!
//! Goal: make sure a permission violation reaches the user as a **clear error**
//! (not a panic), and that a program without a `permissions` block runs untouched.

use std::path::PathBuf;
use std::process::Command;

fn tmp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("aec-cli-perms-{}-{}", std::process::id(), name));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn write_file(dir: &PathBuf, name: &str, content: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, content).expect("write temp source");
    path
}

fn run_cli(args: &[&str], cwd: &PathBuf) -> (bool, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_aec"))
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("run aec binary");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

/// Network: a host outside the allowlist must be rejected with a clear message.
#[test]
fn denied_network_host_produces_a_clear_error() {
    let dir = tmp_dir("network");
    let src = r#"agent Sandbox

permissions {
    network: ["api.example.com"]
}

fn try_fetch() -> int {
    let r = http.get("https://evil.example/steal")
    return 0
}
"#;
    let path = write_file(&dir, "deny_net.aec", src);
    let (ok, stdout, stderr) = run_cli(
        &["run", path.to_str().unwrap(), "--cli", "--entry", "try_fetch"],
        &dir,
    );

    assert!(!ok, "expected failure. stdout:\n{}\nstderr:\n{}", stdout, stderr);
    assert!(
        !stderr.contains("panicked"),
        "must not panic. stderr:\n{}",
        stderr
    );
    assert!(
        stdout.contains("permission denied"),
        "no permission-denied message was printed. stdout:\n{}",
        stdout
    );
    assert!(
        stdout.contains("evil.example"),
        "the message should name the rejected host. stdout:\n{}",
        stdout
    );
}

/// Files: a write outside the allowed root must be rejected and create **nothing**.
#[test]
fn denied_file_write_is_blocked_and_creates_nothing() {
    let dir = tmp_dir("fs");
    let allowed = dir.join("allowed");
    std::fs::create_dir_all(&allowed).unwrap();
    let outside = dir.join("outside");

    let src = format!(
        r#"agent Sandbox

permissions {{
    filesystem {{
        write: ["{}"]
    }}
}}

fn try_write() -> int {{
    file.write("{}", "leaked")
    return 0
}}
"#,
        allowed.display(),
        outside.display()
    );
    let path = write_file(&dir, "deny_fs.aec", &src);
    let (ok, stdout, _) = run_cli(
        &["run", path.to_str().unwrap(), "--cli", "--entry", "try_write"],
        &dir,
    );

    assert!(!ok, "expected failure. stdout:\n{}", stdout);
    assert!(
        !outside.exists(),
        "the disallowed file must not have been created: {}",
        outside.display()
    );
    assert!(
        stdout.contains("file access denied"),
        "the clear denial message was not printed. stdout:\n{}",
        stdout
    );
}

/// Files: a write **inside** the allowed root must work.
#[test]
fn allowed_file_write_succeeds() {
    let dir = tmp_dir("fs-ok");
    let allowed = dir.join("allowed");
    std::fs::create_dir_all(&allowed).unwrap();
    let target = allowed.join("ok.txt");

    let src = format!(
        r#"agent Sandbox

permissions {{
    filesystem {{
        write: ["{}"]
        read: ["{}"]
    }}
}}

fn go() -> int {{
    file.write("{}", "written")
    let back = file.read("{}")
    print("read back:", back)
    return 0
}}
"#,
        allowed.display(),
        allowed.display(),
        target.display(),
        target.display()
    );
    let path = write_file(&dir, "allow_fs.aec", &src);
    let (ok, stdout, stderr) = run_cli(&["run", path.to_str().unwrap(), "--cli", "--entry", "go"], &dir);

    assert!(ok, "expected success. stdout:\n{}\nstderr:\n{}", stdout, stderr);
    assert!(target.exists(), "the allowed file should have been created");
    assert!(stdout.contains("read back"), "stdout:\n{}", stdout);
}

/// regression: without a permissions block, the previous program works untouched.
#[test]
fn program_without_permissions_block_is_unaffected() {
    let dir = tmp_dir("no-block");
    let target = dir.join("free.txt");

    let src = format!(
        r#"agent Free

fn go() -> int {{
    file.write("{}", "anything goes")
    return 0
}}
"#,
        target.display()
    );
    let path = write_file(&dir, "free.aec", &src);
    let (ok, stdout, stderr) = run_cli(&["run", path.to_str().unwrap(), "--cli", "--entry", "go"], &dir);

    assert!(ok, "expected success. stdout:\n{}\nstderr:\n{}", stdout, stderr);
    assert!(target.exists());
}

/// `aec check` must count the new blocks and report no type error.
#[test]
fn check_accepts_permissions_and_limits_blocks() {
    let dir = tmp_dir("check");
    let src = r#"agent Sandbox

permissions {
    network: ["api.example.com"]
    filesystem {
        read: ["/tmp"]
    }
    system {
        metrics: true
    }
}

limits {
    concurrency: 4
    timeout: 5s
}

fn go() -> int {
    return 0
}
"#;
    let path = write_file(&dir, "ok.aec", src);
    let (ok, stdout, _) = run_cli(&["check", path.to_str().unwrap()], &dir);

    assert!(ok, "expected success. stdout:\n{}", stdout);
    assert!(stdout.contains("Type check passed"), "stdout:\n{}", stdout);
}

/// `aec ast` must not panic.
#[test]
fn ast_dumps_permissions_block_without_panicking() {
    let dir = tmp_dir("ast");
    let src = "agent Sandbox\n\npermissions {\n    network: [\"a.com\"]\n}\n\nlimits {\n    timeout: 1s\n}\n\nfn go() -> int { return 0 }\n";
    let path = write_file(&dir, "ast.aec", src);
    let (ok, stdout, stderr) = run_cli(&["ast", path.to_str().unwrap()], &dir);

    assert!(ok, "expected success. stderr:\n{}", stderr);
    assert!(stdout.contains("Permissions"), "stdout:\n{}", stdout);
    assert!(stdout.contains("Limits"), "stdout:\n{}", stdout);
}
