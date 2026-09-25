use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_DIR: AtomicUsize = AtomicUsize::new(0);

fn fixture() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "aec-imports-{}-{}",
        std::process::id(),
        NEXT_DIR.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn write(dir: &Path, name: &str, source: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, source).unwrap();
    path
}

fn run(dir: &Path, args: &[&str]) -> (bool, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_aec"))
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap();
    (
        output.status.success(),
        format!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr)),
    )
}

#[test]
fn imports_relative_to_each_source_and_checks_cross_file_calls() {
    let dir = fixture();
    std::fs::create_dir(dir.join("lib")).unwrap();
    write(&dir, "main.aec", "agent Main\nimport \"./lib/first.aec\"\nfn main() -> int {\n    return helper_one()\n}\n");
    write(&dir.join("lib"), "first.aec", "agent First\nimport \"../second.aec\"\nfn helper_one() -> int {\n    return second()\n}\n");
    write(&dir, "second.aec", "agent Second\nfn second() -> int {\n    return 42\n}\n");

    let (ok, output) = run(&dir, &["run", "main.aec", "--cli"]);
    assert!(ok, "{output}");
    assert!(output.contains("Result: 42"), "{output}");
    let (ok, output) = run(&dir, &["check", "main.aec"]);
    assert!(ok, "{output}");
}

#[test]
fn repeated_imports_are_loaded_once() {
    let dir = fixture();
    write(&dir, "main.aec", "agent Main\nimport \"one.aec\"\nimport \"two.aec\"\nfn main() -> int {\n    return shared()\n}\n");
    write(&dir, "one.aec", "agent One\nimport \"shared.aec\"\n");
    write(&dir, "two.aec", "agent Two\nimport \"shared.aec\"\n");
    write(&dir, "shared.aec", "agent Shared\nfn shared() -> int {\n    return 7\n}\n");
    let (ok, output) = run(&dir, &["check", "main.aec"]);
    assert!(ok, "{output}");
    assert!(output.contains("Items: 2"), "{output}");
}

#[test]
fn circular_and_missing_imports_fail() {
    let dir = fixture();
    write(&dir, "main.aec", "agent Main\nimport \"other.aec\"\n");
    write(&dir, "other.aec", "agent Other\nimport \"main.aec\"\n");
    let (ok, output) = run(&dir, &["check", "main.aec"]);
    assert!(!ok, "{output}");
    assert!(output.contains("circular import"), "{output}");

    write(&dir, "other.aec", "agent Other\nimport \"missing.aec\"\n");
    let (ok, output) = run(&dir, &["run", "main.aec", "--cli"]);
    assert!(!ok, "{output}");
    assert!(output.contains("cannot import missing.aec"), "{output}");
}

#[test]
fn imported_parse_and_type_errors_report_the_right_file() {
    let dir = fixture();
    write(&dir, "main.aec", "agent Main\nimport \"other.aec\"\nfn main() -> int {\n    return bad()\n}\n");
    write(&dir, "other.aec", "agent Other\nfn bad() -> int {\n    let value: int = \"wrong\"\n    return value\n}\n");
    let (ok, output) = run(&dir, &["run", "main.aec", "--cli"]);
    assert!(!ok, "{output}");
    assert!(output.contains("other.aec"), "{output}");
    assert!(output.contains("let value: int"), "{output}");
    assert!(output.contains("Type check failed"), "{output}");

    write(&dir, "other.aec", "agent Other\nfn broken( {\n");
    let (ok, output) = run(&dir, &["check", "main.aec"]);
    assert!(!ok, "{output}");
    assert!(output.contains("other.aec"), "{output}");
    assert!(output.contains("fn broken"), "{output}");
}

#[test]
fn aliases_resolve_public_exports() {
    let dir = fixture();
    write(
        &dir,
        "main.aec",
        "agent Main\nimport \"other.aec\" as other\nfn main() -> int {\n    return other.answer()\n}\n",
    );
    write(
        &dir,
        "other.aec",
        "agent Other\nfn helper() -> int { return 40 }\nfn hidden() -> int { return 1 }\npub fn answer() -> int { return helper() + 2 }\n",
    );
    let (ok, output) = run(&dir, &["run", "main.aec", "--cli"]);
    assert!(ok, "{output}");
    assert!(output.contains("Result: 42"), "{output}");
}

#[test]
fn aliased_private_functions_are_not_in_the_root_namespace() {
    let dir = fixture();
    write(&dir, "main.aec", "agent Main\nimport \"other.aec\" as other\nfn main() -> int { return hidden() }\n");
    write(&dir, "other.aec", "agent Other\nfn hidden() -> int { return 1 }\n");
    let (ok, output) = run(&dir, &["check", "main.aec"]);
    assert!(!ok, "{output}");
    assert!(output.contains("unknown function \"hidden\""), "{output}");
}

#[test]
fn separate_aliases_can_reuse_declaration_names() {
    let dir = fixture();
    write(&dir, "one.aec", "agent One\npub fn shared() -> int { return 1 }\n");
    write(&dir, "two.aec", "agent Two\npub fn shared() -> int { return 2 }\n");
    write(
        &dir,
        "main.aec",
        "agent Main\nimport \"one.aec\" as first\nimport \"two.aec\" as second\nfn main() -> int { return first.shared() + second.shared() }\n",
    );
    let (ok, output) = run(&dir, &["run", "main.aec", "--cli"]);
    assert!(ok, "{output}");
    assert!(output.contains("Result: 3"), "{output}");
}

#[test]
fn aliased_public_components_are_visible_by_qualified_name() {
    let dir = fixture();
    write(
        &dir,
        "main.aec",
        "agent Main\nimport \"ui.aec\" as lib\nui Main = Screen \"Main\" {\n    lib.Panel\n}\n",
    );
    write(
        &dir,
        "ui.aec",
        "agent Ui\npub component Panel { render { Text \"ok\" } }\n",
    );
    let (ok, output) = run(&dir, &["check", "main.aec"]);
    assert!(ok, "{output}");
}

#[test]
fn aliased_private_components_are_not_visible() {
    let dir = fixture();
    write(
        &dir,
        "main.aec",
        "agent Main\nimport \"ui.aec\" as lib\nui Main = Screen \"Main\" {\n    lib.Panel\n}\n",
    );
    write(
        &dir,
        "ui.aec",
        "agent Ui\ncomponent Panel { render { Text \"no\" } }\n",
    );
    let (ok, output) = run(&dir, &["check", "main.aec"]);
    assert!(!ok, "{output}");
    assert!(output.contains("unknown component \"lib.Panel\""), "{output}");
}

#[test]
fn aliased_public_models_keep_their_namespace() {
    let dir = fixture();
    write(
        &dir,
        "main.aec",
        "agent Main\nimport \"models.aec\" as lib\nfn main() -> string { return lib.public_model.name }\n",
    );
    write(
        &dir,
        "models.aec",
        "agent Models\npub model public_model {\n    name: \"gpt-test\"\n}\nmodel private_model {\n    name: \"hidden\"\n}\n",
    );
    let (ok, output) = run(&dir, &["run", "main.aec", "--cli"]);
    assert!(ok, "{output}");
    assert!(output.contains("Result: gpt-test"), "{output}");
}

#[test]
fn one_file_can_be_imported_under_multiple_aliases() {
    let dir = fixture();
    write(
        &dir,
        "library.aec",
        "agent Library\npub fn answer() -> int { return 42 }\n",
    );
    write(
        &dir,
        "main.aec",
        "agent Main\nimport \"library.aec\" as first\nimport \"library.aec\" as second\nfn main() -> int { return first.answer() + second.answer() }\n",
    );
    let (ok, output) = run(&dir, &["run", "main.aec", "--cli"]);
    assert!(ok, "{output}");
    assert!(output.contains("Result: 84"), "{output}");
}

#[test]
fn a_file_can_be_available_flat_and_under_an_alias() {
    let dir = fixture();
    write(
        &dir,
        "library.aec",
        "agent Library\npub fn answer() -> int { return 42 }\n",
    );
    write(
        &dir,
        "main.aec",
        "agent Main\nimport \"library.aec\" as lib\nimport \"library.aec\"\nfn main() -> int { return lib.answer() + answer() }\n",
    );
    let (ok, output) = run(&dir, &["run", "main.aec", "--cli"]);
    assert!(ok, "{output}");
    assert!(output.contains("Result: 84"), "{output}");
}

#[test]
fn aliased_private_models_are_not_in_the_root_namespace() {
    let dir = fixture();
    write(
        &dir,
        "main.aec",
        "agent Main\nimport \"models.aec\" as lib\nfn main() -> string { return private_model.name }\n",
    );
    write(
        &dir,
        "models.aec",
        "agent Models\nmodel private_model {\n    name: \"hidden\"\n}\n",
    );
    let (ok, output) = run(&dir, &["check", "main.aec"]);
    assert!(!ok, "{output}");
    assert!(output.contains("private to its module"), "{output}");
}

#[test]
fn nested_aliases_resolve_relative_to_their_module() {
    let dir = fixture();
    std::fs::create_dir(dir.join("lib")).unwrap();
    write(
        &dir.join("lib"),
        "inner.aec",
        "agent Inner\npub fn value() -> int { return 40 }\n",
    );
    write(
        &dir.join("lib"),
        "outer.aec",
        "agent Outer\nimport \"./inner.aec\" as inner\npub fn answer() -> int { return inner.value() + 2 }\n",
    );
    write(
        &dir,
        "main.aec",
        "agent Main\nimport \"./lib/outer.aec\" as outer\nfn main() -> int { return outer.answer() }\n",
    );
    let (ok, output) = run(&dir, &["run", "main.aec", "--cli"]);
    assert!(ok, "{output}");
    assert!(output.contains("Result: 42"), "{output}");
}

#[test]
fn aliased_public_themes_are_visible_by_qualified_name() {
    let dir = fixture();
    write(
        &dir,
        "main.aec",
        "agent Main\nimport \"theme.aec\" as lib\nui Main = Screen \"Main\" {\n    Text \"ok\" { color: theme.lib.Dark.color.primary }\n}\n",
    );
    write(
        &dir,
        "theme.aec",
        "agent Theme\npub theme Dark {\n    color {\n        primary: \"#89b4fa\"\n    }\n}\n",
    );
    let (ok, output) = run(&dir, &["check", "main.aec"]);
    assert!(ok, "{output}");
}

#[test]
fn aliased_public_type_aliases_can_be_used_in_annotations() {
    let dir = fixture();
    write(&dir, "types.aec", "agent Types\npub type UserId = string\n");
    write(
        &dir,
        "main.aec",
        "agent Main\nimport \"types.aec\" as types\nfn main(value: types.UserId) -> types.UserId { return value }\n",
    );
    let (ok, output) = run(&dir, &["check", "main.aec"]);
    assert!(ok, "{output}");
}

#[test]
fn duplicate_imported_declarations_are_rejected() {
    let dir = fixture();
    write(&dir, "one.aec", "agent One\npub fn shared() -> int { return 1 }\n");
    write(&dir, "two.aec", "agent Two\npub fn shared() -> int { return 2 }\n");
    write(&dir, "main.aec", "agent Main\nimport \"one.aec\"\nimport \"two.aec\"\nfn main() -> int { return shared() }\n");
    let (ok, output) = run(&dir, &["check", "main.aec"]);
    assert!(!ok, "{output}");
    assert!(output.contains("duplicate function 'shared'"), "{output}");
}

#[test]
fn private_declarations_are_not_available_through_an_alias() {
    let dir = fixture();
    write(&dir, "main.aec", "agent Main\nimport \"other.aec\" as other\nfn main() -> int { return other.hidden() }\n");
    write(&dir, "other.aec", "agent Other\nfn hidden() -> int { return 1 }\n");
    let (ok, output) = run(&dir, &["check", "main.aec"]);
    assert!(!ok, "{output}");
    assert!(output.contains("has no export"), "{output}");
}

#[test]
fn imported_file_cannot_override_entry_permissions() {
    let dir = fixture();
    write(&dir, "main.aec", "agent Main\npermissions {\n    network: []\n}\nimport \"other.aec\"\n");
    write(&dir, "other.aec", "agent Other\npermissions {\n    network: [\"other.example\"]\n}\n");
    let (ok, output) = run(&dir, &["check", "main.aec"]);
    assert!(!ok, "{output}");
    assert!(output.contains("permissions and limits must be declared in the entry file"), "{output}");
}
