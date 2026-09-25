use anyhow::{bail, Context, Result};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

pub const MANIFEST_FILE: &str = "apm.toml";
pub const LOCK_FILE: &str = "apm.lock";
pub const LEGACY_MANIFEST_FILE: &str = "aecpm.toml";
pub const LEGACY_LOCK_FILE: &str = "aecpm.lock";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub name: String,
    pub version: String,
    pub dependencies: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockEntry {
    pub name: String,
    pub version: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lockfile {
    pub entries: Vec<LockEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyInfo {
    pub name: String,
    pub version: String,
    pub path: String,
}

pub fn load_manifest(root: &Path) -> Result<Manifest> {
    let path = manifest_path(root);
    let source = fs::read_to_string(&path)
        .with_context(|| format!("cannot read {}", path.display()))?;
    parse_manifest(&source).with_context(|| format!("invalid {}", path.display()))
}

pub fn save_manifest(root: &Path, manifest: &Manifest) -> Result<()> {
    validate_manifest(manifest)?;
    let mut source = String::new();
    source.push_str("[package]\n");
    source.push_str(&format!("name = \"{}\"\n", escape(&manifest.name)));
    source.push_str(&format!("version = \"{}\"\n", escape(&manifest.version)));
    source.push_str("\n[dependencies]\n");
    for (name, path) in &manifest.dependencies {
        source.push_str(&format!("{} = \"{}\"\n", name, escape(path)));
    }
    write_atomic(&root.join(MANIFEST_FILE), &source)
}

pub fn init(root: &Path, name: Option<&str>) -> Result<Manifest> {
    if manifest_path(root).exists() {
        bail!("an APM manifest already exists");
    }
    let manifest = Manifest {
        name: name
            .map(str::to_string)
            .unwrap_or_else(|| default_package_name(root)),
        version: "0.1.0".to_string(),
        dependencies: BTreeMap::new(),
    };
    validate_manifest(&manifest)?;
    save_manifest(root, &manifest)?;
    Ok(manifest)
}

pub fn add_dependency(root: &Path, name: &str, path: &Path) -> Result<Manifest> {
    validate_package_name(name)?;
    let manifest = if manifest_path(root).exists() {
        load_manifest(root)?
    } else {
        Manifest {
            name: default_package_name(root),
            version: "0.1.0".to_string(),
            dependencies: BTreeMap::new(),
        }
    };
    let resolved = resolve_dependency_path(root, path)?;
    if !Path::new(&resolved).is_dir() {
        bail!("dependency '{}' does not point to a directory: {}", name, resolved);
    }
    let stored = relative_to_root(root, Path::new(&resolved)).unwrap_or(resolved);
    let mut updated = manifest;
    updated.dependencies.insert(name.to_string(), stored);
    save_manifest(root, &updated)?;
    Ok(updated)
}

pub fn remove_dependency(root: &Path, name: &str) -> Result<Manifest> {
    let mut manifest = load_manifest(root)?;
    if manifest.dependencies.remove(name).is_none() {
        bail!("dependency '{}' is not present", name);
    }
    save_manifest(root, &manifest)?;
    Ok(manifest)
}

pub fn install(root: &Path) -> Result<Lockfile> {
    let manifest = load_manifest(root)?;
    validate_manifest(&manifest)?;
    validate_dependency_graph(root, &manifest)?;
    let mut entries = Vec::new();
    for (name, raw_path) in &manifest.dependencies {
        let path = PathBuf::from(resolve_dependency_path(root, Path::new(raw_path))?);
        if !path.is_dir() {
            bail!("dependency '{}' does not point to a directory: {}", name, path.display());
        }
        let dependency_manifest = load_optional_manifest(&path)?;
        if let Some(dependency_manifest) = &dependency_manifest {
            if dependency_manifest.name != *name {
                bail!(
                    "dependency '{}' contains package '{}' instead",
                    name,
                    dependency_manifest.name
                );
            }
        }
        entries.push(LockEntry {
            name: name.clone(),
            version: dependency_manifest
                .as_ref()
                .map(|manifest| manifest.version.clone())
                .unwrap_or_else(|| "*".to_string()),
            path: path.to_string_lossy().to_string(),
        });
    }
    entries.sort_by(|left, right| left.name.cmp(&right.name));
    let lockfile = Lockfile { entries };
    save_lockfile(root, &lockfile)?;
    Ok(lockfile)
}

fn validate_dependency_graph(root: &Path, manifest: &Manifest) -> Result<()> {
    let canonical_root = fs::canonicalize(root)
        .with_context(|| format!("cannot resolve {}", root.display()))?;
    let mut stack = BTreeSet::new();
    stack.insert(canonical_root);
    let mut visited = BTreeSet::new();
    validate_graph(manifest, root, &mut stack, &mut visited)
}

fn validate_graph(
    manifest: &Manifest,
    directory: &Path,
    stack: &mut BTreeSet<PathBuf>,
    visited: &mut BTreeSet<PathBuf>,
) -> Result<()> {
    for (name, raw_path) in &manifest.dependencies {
        let path = PathBuf::from(resolve_dependency_path(directory, Path::new(raw_path))?);
        if !path.is_dir() {
            bail!("dependency '{}' does not point to a directory: {}", name, path.display());
        }
        let child = load_optional_manifest(&path)?;
        if let Some(child) = &child {
            if child.name != *name {
                bail!(
                    "dependency '{}' contains package '{}' instead",
                    name,
                    child.name
                );
            }
        }
        if stack.contains(&path) {
            bail!("dependency cycle detected at '{}'", path.display());
        }
        if visited.insert(path.clone()) {
            if let Some(child) = child {
                stack.insert(path.clone());
                validate_graph(&child, &path, stack, visited)?;
                stack.remove(&path);
            }
        }
    }
    Ok(())
}

pub fn list_dependencies(root: &Path) -> Result<Vec<DependencyInfo>> {
    let manifest = load_manifest(root)?;
    let mut dependencies = Vec::new();
    for (name, raw_path) in manifest.dependencies {
        let path = resolve_dependency_path(root, Path::new(&raw_path))?;
        if !Path::new(&path).is_dir() {
            bail!("dependency '{}' does not point to a directory: {}", name, path);
        }
        let dependency_manifest = load_optional_manifest(Path::new(&path))?;
        if let Some(dependency_manifest) = &dependency_manifest {
            if dependency_manifest.name != name {
                bail!(
                    "dependency '{}' contains package '{}' instead",
                    name,
                    dependency_manifest.name
                );
            }
        }
        dependencies.push(DependencyInfo {
            name,
            version: dependency_manifest
                .map(|manifest| manifest.version)
                .unwrap_or_else(|| "*".to_string()),
            path,
        });
    }
    Ok(dependencies)
}

pub fn dependency_tree(root: &Path) -> Result<String> {
    let manifest = load_manifest(root)?;
    let canonical_root = fs::canonicalize(root).with_context(|| format!("cannot resolve {}", root.display()))?;
    let mut lines = vec![format!("{}@{}", manifest.name, manifest.version)];
    let mut stack = BTreeSet::new();
    stack.insert(canonical_root);
    collect_tree(&manifest, root, "", &mut stack, &mut lines)?;
    Ok(lines.join("\n"))
}

pub fn save_lockfile(root: &Path, lockfile: &Lockfile) -> Result<()> {
    let mut source = String::from("# generated by apm install\n");
    for entry in &lockfile.entries {
        source.push_str(&format!(
            "{} = {{ version = \"{}\", path = \"{}\" }}\n",
            entry.name,
            escape(&entry.version),
            escape(&entry.path)
        ));
    }
    write_atomic(&root.join(LOCK_FILE), &source)
}

fn collect_tree(
    manifest: &Manifest,
    directory: &Path,
    prefix: &str,
    stack: &mut BTreeSet<PathBuf>,
    lines: &mut Vec<String>,
) -> Result<()> {
    let count = manifest.dependencies.len();
    for (index, (name, raw_path)) in manifest.dependencies.iter().enumerate() {
        let path = PathBuf::from(resolve_dependency_path(directory, Path::new(raw_path))?);
        if !path.is_dir() {
            bail!("dependency '{}' does not point to a directory: {}", name, path.display());
        }
        let child = load_optional_manifest(&path)?;
        let version = child
            .as_ref()
            .map(|manifest| manifest.version.clone())
            .unwrap_or_else(|| "*".to_string());
        let last = index + 1 == count;
        let branch = if last { "└── " } else { "├── " };
        lines.push(format!("{}{}{}@{}", prefix, branch, name, version));
        if stack.contains(&path) {
            lines.push(format!("{}└── (cycle)", prefix));
            continue;
        }
        if let Some(child) = child {
            stack.insert(path.clone());
            let child_prefix = format!("{}{}", prefix, if last { "    " } else { "│   " });
            collect_tree(&child, &path, &child_prefix, stack, lines)?;
            stack.remove(&path);
        }
    }
    Ok(())
}

fn load_optional_manifest(root: &Path) -> Result<Option<Manifest>> {
    let path = manifest_path(root);
    if !path.exists() {
        return Ok(None);
    }
    let source = fs::read_to_string(&path)
        .with_context(|| format!("cannot read {}", path.display()))?;
    parse_manifest(&source)
        .with_context(|| format!("invalid {}", path.display()))
        .map(Some)
}

fn manifest_path(root: &Path) -> PathBuf {
    let current = root.join(MANIFEST_FILE);
    if current.exists() {
        current
    } else {
        root.join(LEGACY_MANIFEST_FILE)
    }
}

fn parse_manifest(source: &str) -> Result<Manifest> {
    let mut section = String::new();
    let mut name = None;
    let mut version = None;
    let mut dependencies = BTreeMap::new();
    for (line_number, raw_line) in source.lines().enumerate() {
        let line = raw_line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = line[1..line.len() - 1].to_string();
            if section != "package" && section != "dependencies" {
                bail!("unknown manifest section '{}'", section);
            }
            continue;
        }
        let Some((key, raw_value)) = line.split_once('=') else {
            bail!("line {} is not a key/value pair", line_number + 1);
        };
        let key = key.trim();
        let value = unescape(raw_value.trim());
        if value.is_empty() {
            bail!("line {} has an empty value", line_number + 1);
        }
        match section.as_str() {
            "package" if key == "name" => {
                if name.replace(value).is_some() {
                    bail!("duplicate package.name on line {}", line_number + 1);
                }
            }
            "package" if key == "version" => {
                if version.replace(value).is_some() {
                    bail!("duplicate package.version on line {}", line_number + 1);
                }
            }
            "dependencies" => {
                validate_package_name(key).with_context(|| format!("line {}", line_number + 1))?;
                if dependencies.insert(key.to_string(), value).is_some() {
                    bail!("duplicate dependency '{}' on line {}", key, line_number + 1);
                }
            }
            _ => bail!("unknown manifest entry on line {}", line_number + 1),
        }
    }
    let name = name.context("manifest is missing package.name")?;
    validate_package_name(&name)?;
    let version = version.context("manifest is missing package.version")?;
    validate_version(&version)?;
    Ok(Manifest {
        name,
        version,
        dependencies,
    })
}

fn default_package_name(root: &Path) -> String {
    let candidate = root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("aec-package");
    if validate_package_name(candidate).is_ok() {
        candidate.to_string()
    } else {
        "aec-package".to_string()
    }
}

fn validate_manifest(manifest: &Manifest) -> Result<()> {
    validate_package_name(&manifest.name)?;
    validate_version(&manifest.version)?;
    for (name, path) in &manifest.dependencies {
        validate_package_name(name)?;
        if path.trim().is_empty() {
            bail!("dependency '{}' has an empty path", name);
        }
    }
    Ok(())
}

fn resolve_dependency_path(root: &Path, path: &Path) -> Result<String> {
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    let canonical = fs::canonicalize(&joined)
        .with_context(|| format!("cannot resolve dependency path {}", joined.display()))?;
    Ok(canonical.to_string_lossy().to_string())
}

fn relative_to_root(root: &Path, path: &Path) -> Option<String> {
    let root = fs::canonicalize(root).ok()?;
    path.strip_prefix(root)
        .ok()
        .map(|path| path.to_string_lossy().to_string())
}

fn validate_package_name(name: &str) -> Result<()> {
    if name.is_empty()
        || !name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_' || character == '-')
    {
        bail!("invalid package name '{}'; use letters, numbers, '_' or '-'", name);
    }
    Ok(())
}

fn validate_version(version: &str) -> Result<()> {
    if version.is_empty()
        || !version
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '+'))
        || !version.chars().any(|character| character.is_ascii_digit())
    {
        bail!("invalid version '{}'; use a version such as 0.1.0", version);
    }
    Ok(())
}

fn write_atomic(path: &Path, contents: &str) -> Result<()> {
    let temporary = path.with_file_name(format!(
        ".{}.tmp",
        path.file_name().and_then(|name| name.to_str()).unwrap_or("apm")
    ));
    fs::write(&temporary, contents)
        .with_context(|| format!("cannot write {}", temporary.display()))?;
    fs::rename(&temporary, path)
        .with_context(|| format!("cannot replace {}", path.display()))?;
    Ok(())
}

fn escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn unescape(value: &str) -> String {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(value)
        .replace("\\\"", "\"")
        .replace("\\\\", "\\")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT: AtomicUsize = AtomicUsize::new(0);

    fn fixture() -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "apm-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("dependency")).unwrap();
        root
    }

    #[test]
    fn add_and_install_write_a_deterministic_local_lockfile() {
        let root = fixture();
        let manifest = init(&root, Some("demo")).unwrap();
        assert_eq!(manifest.name, "demo");
        let manifest = add_dependency(&root, "helper", Path::new("dependency")).unwrap();
        assert_eq!(manifest.dependencies.len(), 1);
        let lockfile = install(&root).unwrap();
        assert_eq!(lockfile.entries.len(), 1);
        assert_eq!(lockfile.entries[0].version, "*");
        assert!(root.join(MANIFEST_FILE).exists());
        assert!(root.join(LOCK_FILE).exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn remove_list_and_tree_manage_local_dependencies() {
        let root = fixture();
        add_dependency(&root, "helper", Path::new("dependency")).unwrap();
        let listed = list_dependencies(&root).unwrap();
        assert_eq!(listed[0].name, "helper");
        assert!(dependency_tree(&root).unwrap().contains("helper@*"));
        remove_dependency(&root, "helper").unwrap();
        assert!(load_manifest(&root).unwrap().dependencies.is_empty());
        assert!(remove_dependency(&root, "helper").is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn install_validates_package_identity() {
        let root = fixture();
        save_manifest(
            &root.join("dependency"),
            &Manifest {
                name: "other".to_string(),
                version: "1.0.0".to_string(),
                dependencies: BTreeMap::new(),
            },
        )
        .unwrap();
        add_dependency(&root, "helper", Path::new("dependency")).unwrap();
        let error = install(&root).unwrap_err();
        assert!(format!("{error:#}").contains("contains package 'other'"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn dependency_tree_reports_local_cycles() {
        let root = fixture();
        let dependency = root.join("dependency");
        save_manifest(
            &root,
            &Manifest {
                name: "root".to_string(),
                version: "1.0.0".to_string(),
                dependencies: BTreeMap::new(),
            },
        )
        .unwrap();
        save_manifest(
            &dependency,
            &Manifest {
                name: "helper".to_string(),
                version: "1.0.0".to_string(),
                dependencies: BTreeMap::from([("root".to_string(), "..".to_string())]),
            },
        )
        .unwrap();
        add_dependency(&root, "helper", Path::new("dependency")).unwrap();
        let error = install(&root).unwrap_err();
        assert!(format!("{error:#}").contains("dependency cycle"));
        let tree = dependency_tree(&root).unwrap();
        assert!(tree.contains("(cycle)"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn invalid_manifest_is_rejected() {
        let error = parse_manifest("[package]\nname = \"bad name\"\n").unwrap_err();
        assert!(format!("{error:#}").contains("invalid package name"));
    }
}
