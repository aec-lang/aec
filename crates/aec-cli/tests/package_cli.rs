use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);

fn fixture() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "apm-cli-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(path.join("helper")).unwrap();
    std::fs::write(
        path.join("helper").join("apm.toml"),
        "[package]\nname = \"helper\"\nversion = \"1.2.3\"\n\n[dependencies]\n",
    )
    .unwrap();
    path
}

fn run(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_apm"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn apm_add_install_list_and_tree_manage_local_dependencies() {
    let root = fixture();
    let init = run(&root, &["init", "demo"]);
    assert!(init.status.success(), "{}", String::from_utf8_lossy(&init.stderr));
    let add = run(&root, &["add", "helper", "helper"]);
    assert!(add.status.success(), "{}", String::from_utf8_lossy(&add.stderr));
    let install = run(&root, &["install"]);
    assert!(install.status.success(), "{}", String::from_utf8_lossy(&install.stderr));
    assert!(root.join("apm.toml").exists());
    assert!(root.join("apm.lock").exists());

    let list = run(&root, &["list"]);
    assert!(list.status.success(), "{}", String::from_utf8_lossy(&list.stderr));
    assert!(String::from_utf8_lossy(&list.stdout).contains("helper@1.2.3"));

    let tree = run(&root, &["tree"]);
    assert!(tree.status.success(), "{}", String::from_utf8_lossy(&tree.stderr));
    assert!(String::from_utf8_lossy(&tree.stdout).contains("helper@1.2.3"));

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn apm_remove_updates_manifest_and_lockfile() {
    let root = fixture();
    assert!(run(&root, &["add", "helper", "helper"]).status.success());
    assert!(run(&root, &["install"]).status.success());
    let remove = run(&root, &["remove", "helper"]);
    assert!(remove.status.success(), "{}", String::from_utf8_lossy(&remove.stderr));
    let lock = std::fs::read_to_string(root.join("apm.lock")).unwrap();
    assert!(!lock.contains("helper"));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn legacy_aec_add_and_install_remain_compatible() {
    let root = fixture();
    let add = Command::new(env!("CARGO_BIN_EXE_aec"))
        .current_dir(&root)
        .args(["add", "helper", "helper"])
        .output()
        .unwrap();
    assert!(add.status.success(), "{}", String::from_utf8_lossy(&add.stderr));
    let install = Command::new(env!("CARGO_BIN_EXE_aec"))
        .current_dir(&root)
        .arg("install")
        .output()
        .unwrap();
    assert!(install.status.success(), "{}", String::from_utf8_lossy(&install.stderr));
    assert!(root.join("apm.toml").exists());
    assert!(root.join("apm.lock").exists());
    std::fs::remove_dir_all(root).unwrap();
}
