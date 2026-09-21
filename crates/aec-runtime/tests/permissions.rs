//! Sandbox / Permissions tests.
//!
//! Three things are guaranteed:
//!  1. allow and deny for the network
//!  2. allow and deny for the filesystem
//!  3. **regression**: a program without a `permissions` block sees no restriction

use aec_ast::Span;
use aec_parser::parse;
use aec_runtime::permissions::{extract_host, Limits, Permissions};
use aec_runtime::{FsMode, Interpreter, RuntimeError, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join("aec-permissions-tests")
        .join(format!("{}-{}", tag, COUNTER.fetch_add(1, Ordering::SeqCst)));
    std::fs::create_dir_all(&dir).expect("failed to create the temp dir");
    dir
}

/// An `Interpreter` that has run (registered) the given program.
fn interpreter_for(source: &str) -> Interpreter {
    let program = parse(source).expect("parse should succeed");
    let mut interp = Interpreter::new();
    interp.run(&program).expect("run");
    interp
}

fn call(interp: &mut Interpreter, name: &str, args: Vec<Value>) -> Result<Value, RuntimeError> {
    interp.call_function(name, args, Span::dummy())
}

// ---------------------------------------------------------------------------
// host extraction
// ---------------------------------------------------------------------------

#[test]
fn extract_host_ignores_scheme_port_and_case() {
    assert_eq!(extract_host("https://Example.COM/x"), Some("example.com".into()));
    assert_eq!(extract_host("http://api.openai.com:8443/v1"), Some("api.openai.com".into()));
    assert_eq!(extract_host("https://user:pw@api.telegram.org/bot"), Some("api.telegram.org".into()));
    assert_eq!(extract_host("api.telegram.org"), Some("api.telegram.org".into()));
    assert_eq!(extract_host("http://[::1]:8080/x"), Some("::1".into()));
    assert_eq!(extract_host(""), None);
}

// ---------------------------------------------------------------------------
// network — allow / deny
// ---------------------------------------------------------------------------

#[test]
fn network_allows_listed_host_and_denies_others() {
    let perms = Permissions {
        network: Some(vec!["example.com".into()]),
        ..Default::default()
    };

    // allow
    assert!(perms.check_network("https://example.com/api").is_ok());
    assert!(perms.check_network("http://EXAMPLE.com:8080/api").is_ok());

    // deny
    let err = perms
        .check_network("https://evil.com/steal")
        .expect_err("should be denied");
    assert!(err.contains("evil.com"), "message should name the host: {}", err);
    assert!(err.contains("network denied"), "message: {}", err);
}

#[test]
fn network_absent_key_denies_everything() {
    let perms = Permissions::default();
    assert!(perms.check_network("https://example.com").is_err());
}

#[test]
fn network_empty_allowlist_denies_everything() {
    let perms = Permissions {
        network: Some(vec![]),
        ..Default::default()
    };
    let err = perms.check_network("https://example.com").unwrap_err();
    assert!(err.contains("allowlist is empty"), "message: {}", err);
}

#[test]
fn network_denial_fails_the_http_builtin_call() {
    // Deliberately no real HTTP request is made: the deny path short-circuits
    // before any I/O, and the allow path is checked purely in
    // `network_allows_listed_host_and_denies_others`.
    let interp_src = r#"agent T

permissions {
    network: ["example.com"]
}

fn fetch(url: string) -> int {
    let r = http.get(url)
    return 0
}
"#;
    let mut interp = interpreter_for(interp_src);

    // outside the allowlist: must raise a permission error and never reach the network
    let denied = call(
        &mut interp,
        "fetch",
        vec![Value::String("https://evil.com/steal".into())],
    );
    match denied {
        Err(RuntimeError::PermissionDenied { message, .. }) => {
            assert!(message.contains("evil.com"), "message: {}", message);
        }
        other => panic!("expected PermissionDenied, got {:?}", other),
    }

    // an invalid URL must also be rejected with a permission error (not an I/O error)
    assert!(matches!(
        call(&mut interp, "fetch", vec![Value::String("not a url".into())]),
        Err(RuntimeError::PermissionDenied { .. })
    ));
}

// ---------------------------------------------------------------------------
// filesystem — allow / deny
// ---------------------------------------------------------------------------

#[test]
fn filesystem_allows_paths_under_declared_root() {
    let dir = temp_dir("fs-allow");
    let perms = Permissions {
        fs_read: Some(vec![dir.display().to_string()]),
        fs_write: Some(vec![dir.display().to_string()]),
        ..Default::default()
    };

    let inside = dir.join("notes.txt");
    assert!(perms.check_fs(inside.to_str().unwrap(), FsMode::Read).is_ok());
    assert!(perms.check_fs(inside.to_str().unwrap(), FsMode::Write).is_ok());

    // a file that does not exist yet must be allowed too (textual normalization)
    let not_yet = dir.join("sub").join("deep.txt");
    assert!(perms.check_fs(not_yet.to_str().unwrap(), FsMode::Write).is_ok());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn filesystem_denies_paths_outside_root() {
    let dir = temp_dir("fs-deny");
    let perms = Permissions {
        fs_read: Some(vec![dir.display().to_string()]),
        ..Default::default()
    };

    let outside = dir.parent().unwrap().join("elsewhere.txt");
    let err = perms
        .check_fs(outside.to_str().unwrap(), FsMode::Read)
        .expect_err("should be denied");
    assert!(err.contains("file access denied"), "message: {}", err);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn filesystem_does_not_allow_sibling_with_shared_prefix() {
    // root=`/tmp/.../allowed` must not accept `/tmp/.../allowed-evil`
    let dir = temp_dir("fs-prefix");
    let allowed = dir.join("allowed");
    let evil = dir.join("allowed-evil");
    std::fs::create_dir_all(&allowed).unwrap();
    std::fs::create_dir_all(&evil).unwrap();

    let perms = Permissions {
        fs_read: Some(vec![allowed.display().to_string()]),
        ..Default::default()
    };

    assert!(perms
        .check_fs(allowed.join("ok.txt").to_str().unwrap(), FsMode::Read)
        .is_ok());
    assert!(
        perms
            .check_fs(evil.join("bad.txt").to_str().unwrap(), FsMode::Read)
            .is_err(),
        "a sibling path with a shared prefix must not be allowed"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn filesystem_denies_parent_escape_via_dotdot() {
    let dir = temp_dir("fs-dotdot");
    let root = dir.join("root");
    std::fs::create_dir_all(&root).unwrap();

    let perms = Permissions {
        fs_read: Some(vec![root.display().to_string()]),
        ..Default::default()
    };

    let escape = root.join("..").join("secret.txt");
    assert!(
        perms.check_fs(escape.to_str().unwrap(), FsMode::Read).is_err(),
        "`..` must not escape the root: {}",
        escape.display()
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn filesystem_read_permission_does_not_grant_write() {
    let dir = temp_dir("fs-modes");
    let perms = Permissions {
        fs_read: Some(vec![dir.display().to_string()]),
        fs_write: None,
        ..Default::default()
    };
    let path = dir.join("x.txt");
    let path = path.to_str().unwrap();

    assert!(perms.check_fs(path, FsMode::Read).is_ok());
    let err = perms.check_fs(path, FsMode::Write).expect_err("write is not permitted");
    assert!(err.contains("filesystem { write"), "message: {}", err);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn file_builtins_are_gated_end_to_end() {
    let dir = temp_dir("fs-e2e");
    let allowed = dir.join("allowed");
    std::fs::create_dir_all(&allowed).unwrap();
    let secret = dir.join("secret.txt");
    std::fs::write(&secret, "top secret").unwrap();

    let src = format!(
        r#"agent T

permissions {{
    filesystem {{
        read: ["{}"]
        write: ["{}"]
    }}
}}

fn read_it(p: string) -> int {{
    let content = file.read(p)
    return len(content)
}}

fn write_it(p: string) -> int {{
    file.write(p, "data")
    return 1
}}
"#,
        allowed.display(),
        allowed.display()
    );

    let mut interp = interpreter_for(&src);

    // allow: reading inside the permitted path
    let note = allowed.join("note.txt");
    std::fs::write(&note, "hello").unwrap();
    assert!(call(
        &mut interp,
        "read_it",
        vec![Value::String(note.display().to_string())]
    )
    .is_ok());

    // deny: reading a secret file outside the allowlist
    match call(
        &mut interp,
        "read_it",
        vec![Value::String(secret.display().to_string())],
    ) {
        Err(RuntimeError::PermissionDenied { message, .. }) => {
            assert!(message.contains("read"), "message: {}", message);
        }
        other => panic!("expected PermissionDenied, got {:?}", other),
    }

    // allow: writing inside the permitted path
    let out = allowed.join("out.txt");
    assert!(call(
        &mut interp,
        "write_it",
        vec![Value::String(out.display().to_string())]
    )
    .is_ok());
    assert!(out.exists());

    // deny: writing outside
    let bad = dir.join("bad.txt");
    assert!(matches!(
        call(
            &mut interp,
            "write_it",
            vec![Value::String(bad.display().to_string())]
        ),
        Err(RuntimeError::PermissionDenied { .. })
    ));
    assert!(!bad.exists(), "the disallowed file must not have been created");

    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// system
// ---------------------------------------------------------------------------

#[test]
fn sys_info_requires_metrics_permission() {
    let denied_src = r#"agent T

permissions {
    network: ["example.com"]
}

fn probe() -> int {
    let info = sys.info()
    return 0
}
"#;
    let mut interp = interpreter_for(denied_src);
    match call(&mut interp, "probe", vec![]) {
        Err(RuntimeError::PermissionDenied { message, .. }) => {
            assert!(message.contains("metrics"), "message: {}", message);
        }
        other => panic!("expected PermissionDenied, got {:?}", other),
    }

    let allowed_src = r#"agent T

permissions {
    system {
        metrics: true
    }
}

fn probe() -> int {
    let info = sys.info()
    return 0
}
"#;
    let mut interp = interpreter_for(allowed_src);
    assert!(!matches!(
        call(&mut interp, "probe", vec![]),
        Err(RuntimeError::PermissionDenied { .. })
    ));
}

// ---------------------------------------------------------------------------
// regression: without a permissions block everything stays open as before
// ---------------------------------------------------------------------------

#[test]
fn without_permissions_block_nothing_is_gated() {
    let dir = temp_dir("no-block");
    let file = dir.join("free.txt");
    std::fs::write(&file, "content").unwrap();

    let src = r#"agent T

fn read_it(p: string) -> int {
    let c = file.read(p)
    return len(c)
}

fn write_it(p: string) -> int {
    file.write(p, "ok")
    return 1
}
"#;
    let mut interp = interpreter_for(src);

    // there is no block at all → policy is None
    assert!(
        interp.permissions.is_none(),
        "no policy must be built without a permissions block"
    );

    // reading outside any allowlist (since no allowlist exists)
    assert!(call(
        &mut interp,
        "read_it",
        vec![Value::String(file.display().to_string())]
    )
    .is_ok());

    let out = dir.join("out.txt");
    assert!(call(
        &mut interp,
        "write_it",
        vec![Value::String(out.display().to_string())]
    )
    .is_ok());
    assert!(out.exists());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn user_functions_are_never_gated() {
    // the gate only acts on builtin names; a user function must stay free
    let src = r#"agent T

permissions {
    network: []
}

fn add(a: int, b: int) -> int {
    return a + b
}
"#;
    let mut interp = interpreter_for(src);
    assert!(call(&mut interp, "add", vec![Value::Int(2), Value::Int(3)]).is_ok());
    // print must not be gated either
    assert!(call(&mut interp, "print", vec![Value::Int(1)]).is_ok());
}

// ---------------------------------------------------------------------------
// limits
// ---------------------------------------------------------------------------

#[test]
fn limits_block_reaches_the_interpreter() {
    let src = r#"agent T

limits {
    concurrency: 8
    timeout: 2s
}

fn noop() -> int { return 0 }
"#;
    let interp = interpreter_for(src);
    assert_eq!(interp.limits.concurrency, Some(8));
    assert_eq!(interp.limits.timeout_ms, Some(2_000));
}

#[test]
fn http_timeout_uses_limit_and_falls_back_to_default() {
    let with_limit = Limits {
        timeout_ms: Some(1_500),
        ..Default::default()
    };
    assert_eq!(with_limit.http_timeout(), std::time::Duration::from_millis(1_500));

    let without = Limits::default();
    assert_eq!(
        without.http_timeout(),
        std::time::Duration::from_millis(aec_runtime::permissions::DEFAULT_HTTP_TIMEOUT_MS)
    );
}

#[test]
fn interpreter_defaults_have_no_sandbox_and_default_limits() {
    let interp = Interpreter::new();
    assert!(interp.permissions.is_none());
    assert_eq!(interp.limits.concurrency, None);
    assert_eq!(interp.limits.timeout_ms, None);
    assert_eq!(
        interp.limits.http_timeout(),
        std::time::Duration::from_millis(30_000)
    );
}

// ---------------------------------------------------------------------------
// building the policy from the AST
// ---------------------------------------------------------------------------

#[test]
fn policy_is_built_from_the_ast_block() {
    let src = r#"agent T

permissions {
    network: ["a.com", "b.com"]
    filesystem {
        read: ["/var/log"]
        write: ["cache"]
    }
    system {
        metrics: true
        restart: false
    }
}

fn noop() -> int { return 0 }
"#;
    let interp = interpreter_for(src);
    let perms = interp.permissions.as_ref().expect("policy should be built");

    assert_eq!(
        perms.network.as_deref(),
        Some(&["a.com".to_string(), "b.com".to_string()][..])
    );
    assert_eq!(perms.fs_read.as_deref(), Some(&["/var/log".to_string()][..]));
    assert_eq!(perms.fs_write.as_deref(), Some(&["cache".to_string()][..]));
    assert!(perms.system_metrics);
    assert!(!perms.system_restart);
}

#[test]
fn permissions_relative_root_is_resolved_against_cwd() {
    // a relative root and the equivalent absolute path must count as the same
    let cwd = std::env::current_dir().expect("cwd");
    let perms = Permissions {
        fs_read: Some(vec![".".to_string()]),
        ..Default::default()
    };

    let absolute_cwd = Path::new(&cwd);
    assert!(perms
        .check_fs(absolute_cwd.join("Cargo.toml").to_str().unwrap(), FsMode::Read)
        .is_ok());
    assert!(perms.check_fs("Cargo.toml", FsMode::Read).is_ok());
}
