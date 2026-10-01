#[cfg(unix)]
use std::os::unix::fs::{symlink, PermissionsExt};
#[cfg(unix)]
use std::os::unix::net::UnixListener;
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
    assert!(
        init.status.success(),
        "{}",
        String::from_utf8_lossy(&init.stderr)
    );
    let add = run(&root, &["add", "helper", "helper"]);
    assert!(
        add.status.success(),
        "{}",
        String::from_utf8_lossy(&add.stderr)
    );
    let install = run(&root, &["install"]);
    assert!(
        install.status.success(),
        "{}",
        String::from_utf8_lossy(&install.stderr)
    );
    assert!(root.join("apm.toml").exists());
    assert!(root.join("apm.lock").exists());

    let list = run(&root, &["list"]);
    assert!(
        list.status.success(),
        "{}",
        String::from_utf8_lossy(&list.stderr)
    );
    assert!(String::from_utf8_lossy(&list.stdout).contains("helper@1.2.3"));

    let tree = run(&root, &["tree"]);
    assert!(
        tree.status.success(),
        "{}",
        String::from_utf8_lossy(&tree.stderr)
    );
    assert!(String::from_utf8_lossy(&tree.stdout).contains("helper@1.2.3"));

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn apm_remove_updates_manifest_and_lockfile() {
    let root = fixture();
    assert!(run(&root, &["add", "helper", "helper"]).status.success());
    assert!(run(&root, &["install"]).status.success());
    let remove = run(&root, &["remove", "helper"]);
    assert!(
        remove.status.success(),
        "{}",
        String::from_utf8_lossy(&remove.stderr)
    );
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
    assert!(
        add.status.success(),
        "{}",
        String::from_utf8_lossy(&add.stderr)
    );
    let install = Command::new(env!("CARGO_BIN_EXE_aec"))
        .current_dir(&root)
        .arg("install")
        .output()
        .unwrap();
    assert!(
        install.status.success(),
        "{}",
        String::from_utf8_lossy(&install.stderr)
    );
    assert!(root.join("apm.toml").exists());
    assert!(root.join("apm.lock").exists());
    std::fs::remove_dir_all(root).unwrap();
}

struct RegistryFixture {
    root: PathBuf,
    package: PathBuf,
    registry: PathBuf,
    private_key: PathBuf,
    trust_key: PathBuf,
}

fn registry_fixture() -> RegistryFixture {
    let root = fixture();
    let package = root.join("signed-package");
    std::fs::create_dir_all(package.join("src")).unwrap();
    std::fs::write(
        package.join("apm.toml"),
        "[package]\nname = \"signed-demo\"\nversion = \"1.2.3\"\n\n[dependencies]\n",
    )
    .unwrap();
    std::fs::write(package.join("src/main.aec"), "fn main() {}\n").unwrap();
    let private_key = root.join("signing.pkcs8");
    let trust_key = root.join("trust.pub");
    let keygen = run(
        &root,
        &[
            "keygen",
            "--private-key",
            private_key.to_str().unwrap(),
            "--public-key",
            trust_key.to_str().unwrap(),
        ],
    );
    assert!(
        keygen.status.success(),
        "{}",
        String::from_utf8_lossy(&keygen.stderr)
    );
    let registry = root.join("registry");
    let init = run(&root, &["registry-init", registry.to_str().unwrap()]);
    assert!(
        init.status.success(),
        "{}",
        String::from_utf8_lossy(&init.stderr)
    );
    RegistryFixture {
        root,
        package,
        registry,
        private_key,
        trust_key,
    }
}

fn publish(fixture: &RegistryFixture) -> Output {
    let output = run(
        &fixture.root,
        &[
            "publish",
            "--registry",
            fixture.registry.to_str().unwrap(),
            "--private-key",
            fixture.private_key.to_str().unwrap(),
            "--package",
            fixture.package.to_str().unwrap(),
        ],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn verify(fixture: &RegistryFixture) -> Output {
    run(
        &fixture.root,
        &[
            "verify",
            "--registry",
            fixture.registry.to_str().unwrap(),
            "--name",
            "signed-demo",
            "--version",
            "1.2.3",
            "--trust-key",
            fixture.trust_key.to_str().unwrap(),
        ],
    )
}

fn package_version_directory(fixture: &RegistryFixture) -> PathBuf {
    fixture
        .registry
        .join("packages")
        .join("signed-demo")
        .join("1.2.3")
}

fn assert_staging_is_empty(fixture: &RegistryFixture) {
    let staging = fixture.registry.join(".staging");
    assert_eq!(std::fs::read_dir(staging).unwrap().count(), 0);
}

#[test]
fn keygen_publish_and_verify_use_a_local_signed_registry() {
    let fixture = registry_fixture();
    let public_key = std::fs::read_to_string(&fixture.trust_key).unwrap();
    assert_eq!(public_key.len(), 65);
    assert!(public_key.ends_with('\n'));
    assert!(public_key[..64]
        .bytes()
        .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()));
    let private_key = std::fs::read(&fixture.private_key).unwrap();
    assert!(!private_key.is_empty());
    #[cfg(unix)]
    {
        let mode = std::fs::metadata(&fixture.private_key)
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o077, 0);
        std::fs::set_permissions(&fixture.private_key, std::fs::Permissions::from_mode(0o644))
            .unwrap();
        let loose_key = run(
            &fixture.root,
            &[
                "publish",
                "--registry",
                fixture.registry.to_str().unwrap(),
                "--private-key",
                fixture.private_key.to_str().unwrap(),
                "--package",
                fixture.package.to_str().unwrap(),
            ],
        );
        assert!(!loose_key.status.success());
        assert!(String::from_utf8_lossy(&loose_key.stderr).contains("permissions"));
        std::fs::set_permissions(&fixture.private_key, std::fs::Permissions::from_mode(0o600))
            .unwrap();
        assert!(!package_version_directory(&fixture).exists());
        assert_staging_is_empty(&fixture);
    }

    publish(&fixture);
    let version_directory = package_version_directory(&fixture);
    assert!(version_directory.join("payload/apm.toml").is_file());
    assert!(version_directory.join("metadata").is_file());
    assert_eq!(
        std::fs::metadata(version_directory.join("signature"))
            .unwrap()
            .len(),
        64
    );
    let metadata = std::fs::read_to_string(version_directory.join("metadata")).unwrap();
    assert!(metadata.starts_with("apm-registry-metadata-v1\n"));
    assert!(metadata.contains("name=signed-demo\n"));
    assert!(metadata.contains("version=1.2.3\n"));
    assert!(!metadata.contains(fixture.private_key.to_str().unwrap()));
    let second_registry = fixture.root.join("registry-two");
    let second_init = run(
        &fixture.root,
        &["registry-init", second_registry.to_str().unwrap()],
    );
    assert!(second_init.status.success());
    let second_publish = run(
        &fixture.root,
        &[
            "publish",
            "--registry",
            second_registry.to_str().unwrap(),
            "--private-key",
            fixture.private_key.to_str().unwrap(),
            "--package",
            fixture.package.to_str().unwrap(),
        ],
    );
    assert!(second_publish.status.success());
    let second_version = second_registry.join("packages/signed-demo/1.2.3");
    assert_eq!(
        std::fs::read(second_version.join("metadata")).unwrap(),
        metadata.as_bytes()
    );
    assert_eq!(
        std::fs::read(second_version.join("signature")).unwrap(),
        std::fs::read(version_directory.join("signature")).unwrap()
    );

    let registry_trust = fixture.registry.join("registry-key.pub");
    std::fs::copy(&fixture.trust_key, &registry_trust).unwrap();
    let registry_trust_output = run(
        &fixture.root,
        &[
            "verify",
            "--registry",
            fixture.registry.to_str().unwrap(),
            "--name",
            "signed-demo",
            "--version",
            "1.2.3",
            "--trust-key",
            registry_trust.to_str().unwrap(),
        ],
    );
    assert!(!registry_trust_output.status.success());
    assert!(String::from_utf8_lossy(&registry_trust_output.stderr)
        .contains("trust key must be stored outside the registry"));
    let wrong_private = fixture.root.join("wrong.pkcs8");
    let wrong_trust = fixture.root.join("wrong.pub");
    let wrong_keygen = run(
        &fixture.root,
        &[
            "keygen",
            "--private-key",
            wrong_private.to_str().unwrap(),
            "--public-key",
            wrong_trust.to_str().unwrap(),
        ],
    );
    assert!(wrong_keygen.status.success());
    let wrong_trust_output = run(
        &fixture.root,
        &[
            "verify",
            "--registry",
            fixture.registry.to_str().unwrap(),
            "--name",
            "signed-demo",
            "--version",
            "1.2.3",
            "--trust-key",
            wrong_trust.to_str().unwrap(),
        ],
    );
    assert!(!wrong_trust_output.status.success());
    assert!(String::from_utf8_lossy(&wrong_trust_output.stderr)
        .contains("does not match the explicit trust key"));

    let verified = verify(&fixture);
    assert!(
        verified.status.success(),
        "{}",
        String::from_utf8_lossy(&verified.stderr)
    );
    assert!(String::from_utf8_lossy(&verified.stdout).contains("verified signed-demo@1.2.3"));
    assert_staging_is_empty(&fixture);
    std::fs::remove_dir_all(fixture.root).unwrap();
}

#[test]
fn verify_rejects_tamper_and_publish_keeps_versions_immutable() {
    let fixture = registry_fixture();
    publish(&fixture);
    let version_directory = package_version_directory(&fixture);
    let metadata_path = version_directory.join("metadata");
    let original_metadata = std::fs::read(&metadata_path).unwrap();
    let payload_file = version_directory.join("payload/src/main.aec");
    std::fs::write(&payload_file, "fn changed() {}\n").unwrap();
    let tampered_payload = verify(&fixture);
    assert!(!tampered_payload.status.success());
    assert!(String::from_utf8_lossy(&tampered_payload.stderr).contains("tree digest"));

    std::fs::write(&payload_file, "fn main() {}\n").unwrap();
    let verified = verify(&fixture);
    assert!(
        verified.status.success(),
        "{}",
        String::from_utf8_lossy(&verified.stderr)
    );

    let mut changed_metadata = original_metadata.clone();
    let marker = b"tree-sha256=";
    let marker_position = changed_metadata
        .windows(marker.len())
        .position(|window| window == marker)
        .unwrap()
        + marker.len();
    changed_metadata[marker_position] = if changed_metadata[marker_position] == b'0' {
        b'1'
    } else {
        b'0'
    };
    std::fs::write(&metadata_path, &changed_metadata).unwrap();
    let tampered_metadata = verify(&fixture);
    assert!(!tampered_metadata.status.success());
    assert!(String::from_utf8_lossy(&tampered_metadata.stderr).contains("signature verification"));

    std::fs::write(&metadata_path, &original_metadata).unwrap();
    let collision = run(
        &fixture.root,
        &[
            "publish",
            "--registry",
            fixture.registry.to_str().unwrap(),
            "--private-key",
            fixture.private_key.to_str().unwrap(),
            "--package",
            fixture.package.to_str().unwrap(),
        ],
    );
    assert!(!collision.status.success());
    assert!(String::from_utf8_lossy(&collision.stderr).contains("immutable"));
    assert_eq!(std::fs::read(&metadata_path).unwrap(), original_metadata);
    assert!(verify(&fixture).status.success());
    assert_staging_is_empty(&fixture);
    std::fs::remove_dir_all(fixture.root).unwrap();
}

#[test]
fn publish_rejects_missing_keys_and_concurrent_collisions_atomically() {
    let fixture = registry_fixture();
    let missing_key = run(
        &fixture.root,
        &[
            "publish",
            "--registry",
            fixture.registry.to_str().unwrap(),
            "--package",
            fixture.package.to_str().unwrap(),
        ],
    );
    assert!(!missing_key.status.success());
    assert!(String::from_utf8_lossy(&missing_key.stderr).contains("--private-key"));
    assert!(!package_version_directory(&fixture).exists());
    assert_staging_is_empty(&fixture);

    let root = fixture.root.clone();
    let registry = fixture.registry.clone();
    let private_key = fixture.private_key.clone();
    let package = fixture.package.clone();
    let (left, right) = std::thread::scope(|scope| {
        let left = scope.spawn(|| {
            run(
                &root,
                &[
                    "publish",
                    "--registry",
                    registry.to_str().unwrap(),
                    "--private-key",
                    private_key.to_str().unwrap(),
                    "--package",
                    package.to_str().unwrap(),
                ],
            )
        });
        let right = scope.spawn(|| {
            run(
                &root,
                &[
                    "publish",
                    "--registry",
                    registry.to_str().unwrap(),
                    "--private-key",
                    private_key.to_str().unwrap(),
                    "--package",
                    package.to_str().unwrap(),
                ],
            )
        });
        (left.join().unwrap(), right.join().unwrap())
    });
    assert_eq!(
        usize::from(left.status.success()) + usize::from(right.status.success()),
        1
    );
    let failure = if left.status.success() { &right } else { &left };
    assert!(String::from_utf8_lossy(&failure.stderr).contains("immutable"));
    assert!(verify(&fixture).status.success());
    assert_staging_is_empty(&fixture);
    std::fs::remove_dir_all(fixture.root).unwrap();
}

#[cfg(unix)]
#[test]
fn publish_rejects_symlink_and_special_payload_entries_without_partial_publish() {
    let fixture = registry_fixture();
    let target = fixture.root.join("outside.txt");
    std::fs::write(&target, "outside\n").unwrap();
    symlink(&target, fixture.package.join("payload-link")).unwrap();
    let symlink_output = run(
        &fixture.root,
        &[
            "publish",
            "--registry",
            fixture.registry.to_str().unwrap(),
            "--private-key",
            fixture.private_key.to_str().unwrap(),
            "--package",
            fixture.package.to_str().unwrap(),
        ],
    );
    assert!(!symlink_output.status.success());
    assert!(String::from_utf8_lossy(&symlink_output.stderr).contains("symbolic links"));
    assert!(!package_version_directory(&fixture).exists());
    assert_staging_is_empty(&fixture);
    std::fs::remove_file(fixture.package.join("payload-link")).unwrap();

    let socket_path = fixture.package.join("payload.sock");
    let listener = UnixListener::bind(&socket_path).unwrap();
    let special_output = run(
        &fixture.root,
        &[
            "publish",
            "--registry",
            fixture.registry.to_str().unwrap(),
            "--private-key",
            fixture.private_key.to_str().unwrap(),
            "--package",
            fixture.package.to_str().unwrap(),
        ],
    );
    drop(listener);
    assert!(!special_output.status.success());
    assert!(String::from_utf8_lossy(&special_output.stderr).contains("special files"));
    assert!(!package_version_directory(&fixture).exists());
    assert_staging_is_empty(&fixture);
    std::fs::remove_file(socket_path).unwrap();
    std::fs::remove_dir_all(fixture.root).unwrap();
}
