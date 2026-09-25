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

#[test]
fn shell_cannot_bypass_declared_permissions() {
    let mut restricted = interpreter_for("agent Restricted\npermissions {\n    network: []\n}\n");
    for name in ["shell.run", "shell_run"] {
        let result = call(&mut restricted, name, vec![Value::String("echo bypass".into())]);
        assert!(matches!(result, Err(RuntimeError::PermissionDenied { .. })), "{name}: {result:?}");
    }

    let mut unrestricted = interpreter_for("agent Unrestricted\n");
    let result = call(&mut unrestricted, "shell.run", vec![Value::String("echo allowed".into())]);
    assert!(matches!(result, Ok(Value::Object(_))), "{result:?}");
}

#[test]
fn model_requests_respect_the_network_allowlist() {
    let mut interpreter = interpreter_for("agent Restricted\npermissions {\n    network: []\n}\n");
    for name in ["llm.complete", "llm_complete"] {
        let result = call(&mut interpreter, name, vec![Value::String("Hello".into())]);
        assert!(matches!(result, Err(RuntimeError::PermissionDenied { .. })), "{name}: {result:?}");
    }

    let mut interpreter = interpreter_for("agent Restricted\npermissions {\n    network: [\"api.openai.com\"]\n}\n");
    let mut options = std::collections::HashMap::new();
    options.insert("prompt".into(), Value::String("Hello".into()));
    options.insert("base_url".into(), Value::String("https://other.example/v1".into()));
    let result = call(&mut interpreter, "llm.complete", vec![Value::Object(options)]);
    assert!(matches!(result, Err(RuntimeError::PermissionDenied { .. })), "{result:?}");
}

#[test]
fn restricted_http_does_not_follow_redirects() {
    use std::io::{Read, Write};
    let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = server.local_addr().unwrap();
    let worker = std::thread::spawn(move || {
        let (mut stream, _) = server.accept().unwrap();
        let mut request = [0; 2048];
        let _ = stream.read(&mut request).unwrap();
        stream.write_all(b"HTTP/1.1 302 Found\r\nLocation: http://blocked.example/secret\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
    });

    let mut interpreter = Interpreter::new();
    interpreter.permissions = Some(Permissions {
        network: Some(vec!["127.0.0.1".into()]),
        ..Default::default()
    });
    let result = call(&mut interpreter, "http.get", vec![Value::String(format!("http://{address}/start"))]);
    worker.join().unwrap();
    assert!(matches!(result, Ok(Value::Object(fields)) if matches!(fields.get("status"), Some(Value::Int(302)))));
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

#[test]
fn llm_uses_configured_timeout_and_requires_response_content() {
    use std::io::{Read, Write};
    let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = server.local_addr().unwrap();
    let worker = std::thread::spawn(move || {
        let (mut stream, _) = server.accept().unwrap();
        let mut request = [0; 4096];
        let _ = stream.read(&mut request).unwrap();
        stream
            .write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 82\r\nConnection: close\r\n\r\n{\"choices\":[{\"message\":{\"content\":\"pong\"},\"finish_reason\":\"stop\"}],\"model\":\"test\"}",
            )
            .unwrap();
    });
    let mut options = std::collections::HashMap::new();
    options.insert("prompt".to_string(), Value::String("ping".to_string()));
    options.insert("base_url".to_string(), Value::String(format!("http://{address}/v1")));
    options.insert("api_key".to_string(), Value::String("test-key".to_string()));
    let mut interpreter = Interpreter::new();
    let result = call(
        &mut interpreter,
        "llm.complete",
        vec![Value::Object(options)],
    );
    worker.join().unwrap();
    let value = result.expect("LLM request should succeed");
    assert!(matches!(value, Value::Object(fields) if matches!(fields.get("text"), Some(Value::String(text)) if text == "pong")));
}

#[test]
fn declared_models_expose_think_through_the_model_adapter() {
    use std::io::{Read, Write};
    let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = server.local_addr().unwrap();
    let worker = std::thread::spawn(move || {
        let (mut stream, _) = server.accept().unwrap();
        let mut request = [0; 4096];
        let _ = stream.read(&mut request).unwrap();
        let body = r#"{"choices":[{"message":{"content":"model-pong"},"finish_reason":"stop"}],"model":"test"}"#;
        stream
            .write_all(
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                )
                .as_bytes(),
            )
            .unwrap();
    });
    let source = format!(
        "agent T\nmodel primary {{\n    name: \"test\"\n    base_url: \"http://{address}/v1\"\n    api_key: \"test-key\"\n}}\nfn f() -> int {{\n    let response = primary.think(\"ping\")\n    return len(response.text)\n}}\n"
    );
    let mut interpreter = interpreter_for(&source);
    let result = call(&mut interpreter, "f", vec![]);
    worker.join().unwrap();
    assert!(matches!(result, Ok(Value::Int(10))));
}

#[test]
fn restricted_environment_and_memory_are_denied() {
    let src = r#"agent T
permissions {
    network: []
}
fn read_env() -> string { return env.get("SECRET") }
fn all_env() -> int { return len(sys.env_all()) }
fn open_memory() -> int { memory.open("restricted.sqlite") return 1 }
"#;
    let mut interp = interpreter_for(src);
    for (name, args) in [
        ("read_env", vec![]),
        ("all_env", vec![]),
        ("open_memory", vec![]),
    ] {
        assert!(
            matches!(call(&mut interp, name, args), Err(RuntimeError::PermissionDenied { .. })),
            "{name} should be denied"
        );
    }
}

#[test]
fn running_a_new_program_resets_policy_and_memory() {
    let restricted = "agent Restricted\npermissions { network: [] }\nfn probe() -> int { return 1 }\n";
    let unrestricted = "agent Open\nfn probe() -> int { return 2 }\n";
    let mut interp = interpreter_for(restricted);
    assert!(interp.permissions.is_some());
    let open = aec_parser::parse(unrestricted).unwrap();
    interp.run(&open).unwrap();
    assert!(interp.permissions.is_none());
    assert!(matches!(call(&mut interp, "probe", vec![]), Ok(Value::Int(2))));
}
