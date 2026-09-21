//! Memory tests — in-memory and persistent (SQLite)

use aec_ast::Span;
use aec_parser::parse;
use aec_runtime::memory::Memory;
use aec_runtime::{Interpreter, RuntimeError, Value};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// A unique file path in temp so parallel tests do not collide.
fn temp_db(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("aec-memory-tests");
    std::fs::create_dir_all(&dir).expect("failed to create the temp dir");
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    dir.join(format!("{}-{}-{}.sqlite", tag, std::process::id(), n))
}

// ---------------------------------------------------------------------------
// in-memory mode — regression: the previous behaviour must not change
// ---------------------------------------------------------------------------

#[test]
fn in_memory_add_get_len_clear() {
    let mut mem = Memory::new();
    assert!(!mem.is_persistent());
    assert_eq!(mem.len("c1").unwrap(), 0);

    mem.add("c1", "user", "سلام").unwrap();
    mem.add("c1", "ai", "سلام!").unwrap();
    mem.add("c2", "user", "یه گفتگوی دیگه").unwrap();

    assert_eq!(mem.len("c1").unwrap(), 2);
    assert_eq!(mem.len("c2").unwrap(), 1);

    let msgs = mem.get("c1").unwrap();
    assert_eq!(msgs.len(), 2);
    assert_eq!(msgs[0].role, "user");
    assert_eq!(msgs[0].content, "سلام");
    assert_eq!(msgs[1].role, "ai");

    mem.clear("c1").unwrap();
    assert_eq!(mem.len("c1").unwrap(), 0);
    assert!(mem.get("c1").unwrap().is_empty());
    // clearing c1 must not touch c2
    assert_eq!(mem.len("c2").unwrap(), 1);
}

#[test]
fn in_memory_to_value_shape() {
    let mut mem = Memory::new();
    mem.add("c1", "user", "hi").unwrap();

    let v = mem.to_value("c1").unwrap();
    let Value::Array(items) = v else {
        panic!("to_value must be an Array, got {:?}", v);
    };
    assert_eq!(items.len(), 1);
    let Value::Object(obj) = &items[0] else {
        panic!("every item must be an Object");
    };
    assert!(matches!(obj.get("role"), Some(Value::String(s)) if s == "user"));
    assert!(matches!(obj.get("content"), Some(Value::String(s)) if s == "hi"));
}

// ---------------------------------------------------------------------------
// persistent mode (SQLite)
// ---------------------------------------------------------------------------

#[test]
fn sqlite_survives_reopen() {
    let path = temp_db("reopen");

    {
        let mut mem = Memory::open(&path).expect("open should succeed");
        assert!(mem.is_persistent());
        mem.add("c1", "user", "سلام").unwrap();
        mem.add("c1", "ai", "سلام! چطور می‌تونم کمک کنم؟").unwrap();
        assert_eq!(mem.len("c1").unwrap(), 2);
    } // drop — the process/handle is closed

    let reopened = Memory::open(&path).expect("reopening should succeed");
    let msgs = reopened.get("c1").unwrap();
    assert_eq!(msgs.len(), 2);
    assert_eq!(msgs[0].role, "user");
    assert_eq!(msgs[0].content, "سلام");
    assert_eq!(msgs[1].role, "ai");
    assert_eq!(msgs[1].content, "سلام! چطور می‌تونم کمک کنم؟");
    assert_eq!(reopened.len("c1").unwrap(), 2);

    let _ = std::fs::remove_file(&path);
}

#[test]
fn sqlite_preserves_insertion_order() {
    let path = temp_db("order");
    let mut mem = Memory::open(&path).unwrap();
    for i in 0..25 {
        mem.add("c1", "user", &format!("m{i}")).unwrap();
    }
    let contents: Vec<String> = mem
        .get("c1")
        .unwrap()
        .into_iter()
        .map(|m| m.content)
        .collect();
    let expected: Vec<String> = (0..25).map(|i| format!("m{i}")).collect();
    assert_eq!(contents, expected);

    let _ = std::fs::remove_file(&path);
}

#[test]
fn sqlite_conversations_are_isolated_and_clearable() {
    let path = temp_db("isolate");
    {
        let mut mem = Memory::open(&path).unwrap();
        mem.add("c1", "user", "a").unwrap();
        mem.add("c2", "user", "b").unwrap();
        mem.add("c2", "ai", "c").unwrap();
        assert_eq!(mem.len("c1").unwrap(), 1);
        assert_eq!(mem.len("c2").unwrap(), 2);

        mem.clear("c1").unwrap();
        assert_eq!(mem.len("c1").unwrap(), 0);
    }

    let reopened = Memory::open(&path).unwrap();
    assert_eq!(
        reopened.len("c1").unwrap(),
        0,
        "clear must have persisted to disk too"
    );
    assert_eq!(reopened.len("c2").unwrap(), 2);
    // unknown conversation → empty, without error
    assert_eq!(reopened.len("nope").unwrap(), 0);
    assert!(reopened.is_empty("nope").unwrap());

    let _ = std::fs::remove_file(&path);
}

#[test]
fn sqlite_open_is_idempotent_and_reuses_existing_file() {
    let path = temp_db("idempotent");
    {
        let mut a = Memory::open(&path).unwrap();
        a.add("c1", "user", "first").unwrap();
    }
    {
        let mut b = Memory::open(&path).unwrap();
        b.add("c1", "user", "second").unwrap();
    }
    let c = Memory::open(&path).unwrap();
    assert_eq!(c.len("c1").unwrap(), 2);

    let _ = std::fs::remove_file(&path);
}

#[test]
fn sqlite_reads_see_writes_from_another_instance() {
    // Because the database is the source of truth (not a cache), two concurrent
    // instances do not diverge.
    let path = temp_db("shared");
    let mut writer = Memory::open(&path).unwrap();
    writer.add("c1", "user", "از نویسنده").unwrap();

    let reader = Memory::open(&path).unwrap();
    assert_eq!(reader.len("c1").unwrap(), 1);
    assert_eq!(reader.get("c1").unwrap()[0].content, "از نویسنده");

    let _ = std::fs::remove_file(&path);
}

#[test]
fn sqlite_open_fails_on_unwritable_path() {
    let dir = std::env::temp_dir()
        .join("aec-memory-tests")
        .join("definitely-not-a-dir");
    let bogus = dir.join("nested").join("db.sqlite");
    let err = Memory::open(&bogus).expect_err("should fail");
    assert_eq!(err.span(), Span::dummy());
    assert!(!err.to_string().is_empty());
}

// ---------------------------------------------------------------------------
// End-to-end: real parser → interpreter → memory.* builtins
// ---------------------------------------------------------------------------

#[test]
fn builtins_round_trip_through_sqlite() {
    let path = temp_db("e2e");
    let source = format!(
        r#"
agent Test

fn seed(path: string) -> int {{
    memory.open(path)
    memory.add("c1", "user", "سلام")
    memory.add("c1", "assistant", "سلام! چطور می‌تونم کمک کنم؟")
    return memory.count("c1")
}}

fn history_size() -> int {{
    return memory.count("c1")
}}
"#
    );

    let program = parse(&source).expect("parse should succeed");

    let mut interp = Interpreter::new();
    interp.run(&program).unwrap();
    let count = interp
        .call_function(
            "seed",
            vec![Value::String(path.display().to_string())],
            Span::dummy(),
        )
        .expect("running seed");
    assert!(matches!(count, Value::Int(2)));
    assert!(interp.memory.is_persistent());

    // the same instance must be able to read it
    let got = interp
        .call_function("history_size", vec![], Span::dummy())
        .unwrap();
    assert!(matches!(got, Value::Int(2)));

    // and a completely fresh run on the same file sees the same memory
    let reopened = Memory::open(&path).unwrap();
    let msgs = reopened.get("c1").unwrap();
    assert_eq!(msgs.len(), 2);
    assert_eq!(msgs[1].role, "assistant");
    assert_eq!(msgs[1].content, "سلام! چطور می‌تونم کمک کنم؟");

    let _ = std::fs::remove_file(&path);
}

#[test]
fn builtin_memory_open_with_wrong_type_is_a_type_error() {
    let source = r#"
agent Test

fn bad() -> int {
    memory.open(42)
    return 0
}
"#;
    let program = parse(source).expect("parse should succeed");
    let mut interp = Interpreter::new();
    interp.run(&program).unwrap();

    let err = interp
        .call_function("bad", vec![], Span::dummy())
        .expect_err("should raise a type error");
    assert!(
        matches!(err, RuntimeError::TypeError { .. }),
        "expected TypeError, got {:?}",
        err
    );
}

#[test]
fn interpreter_starts_in_memory_backend() {
    let interp = Interpreter::new();
    assert!(!interp.memory.is_persistent());
    assert_eq!(interp.memory.len("c1").unwrap(), 0);
}
