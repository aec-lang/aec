//! Permissions — enforcing the permissions declared in the `permissions` block.
//!
//! **Security model (important):** this is an in-process, cooperative guard —
//! not an OS sandbox. The following blocks are covered:
//!   - `network`    → `http.get/post/put/delete`
//!   - `filesystem` → `file.read/write/append/exists/delete/mkdir/copy/size/list_dir`
//!   - `system`     → `sys.info`
//!
//! `shell.run` is denied whenever a permissions block is present. Allowing
//! arbitrary commands would bypass the other guards. Without a permissions
//! block it remains available for backwards compatibility. `sys.exit` is
//! always allowed (per the design document).
//!
//! **Activation rule:** if the program has no `permissions` block, no gate is
//! applied (open behaviour, backwards compatible). With the block present, the
//! **default is closed**: any key not declared means "not allowed".

use crate::errors::RuntimeError;
use crate::value::Value;
use aec_ast::{LimitsBlock, PermissionsBlock, PermissionsEntry, Span};
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

/// Default HTTP cap when `limits { timeout }` is not declared.
pub const DEFAULT_HTTP_TIMEOUT_MS: u64 = 30_000;

/// File access direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsMode {
    Read,
    Write,
}

/// Currently active permission policy.
///
/// `None` in any field means "key not declared" → that category is not allowed.
#[derive(Debug, Clone, Default)]
pub struct Permissions {
    /// Allowed hosts (without port, case-insensitive).
    pub network: Option<Vec<String>>,
    pub fs_read: Option<Vec<String>>,
    pub fs_write: Option<Vec<String>>,
    pub system_metrics: bool,
    pub system_restart: bool,
}

impl Permissions {
    pub fn from_block(block: &PermissionsBlock) -> Self {
        let mut perms = Permissions::default();
        for entry in &block.entries {
            match entry {
                PermissionsEntry::Network(rule) => {
                    perms.network = Some(rule.hosts.clone());
                }
                PermissionsEntry::Filesystem(rule) => {
                    perms.fs_read = Some(rule.read.clone());
                    perms.fs_write = Some(rule.write.clone());
                }
                PermissionsEntry::System(rule) => {
                    perms.system_metrics = rule.metrics.unwrap_or(false);
                    perms.system_restart = rule.restart.unwrap_or(false);
                }
            }
        }
        perms
    }

    /// Central gate: decides from the builtin name and its arguments.
    ///
    /// `Ok(())` for any unknown name — so calling user functions stays untouched.
    /// If the arguments are not of the expected type, no permission error is
    /// raised here; the builtin itself reports the more precise type error.
    pub fn check_builtin(&self, name: &str, args: &[Value]) -> Result<(), String> {
        let arg_str = |i: usize| match args.get(i) {
            Some(Value::String(s)) => Some(s.as_str()),
            _ => None,
        };

        match name {
            "shell.run" | "shell_run" => {
                Err("shell execution denied: shell.run is unavailable when permissions are declared".into())
            }
            "llm.complete" | "llm_complete" => match args.first() {
                Some(Value::String(_)) => self.check_network(crate::llm::DEFAULT_BASE_URL),
                Some(Value::Object(options)) => match options.get("base_url") {
                    Some(Value::String(url)) => self.check_network(url),
                    _ => self.check_network(crate::llm::DEFAULT_BASE_URL),
                },
                _ => Ok(()),
            },
            // ---------- network ----------
            "http.get" | "http_get" | "http.post" | "http_post" | "http.put" | "http_put"
            | "http.delete" | "http_delete" => match arg_str(0) {
                Some(url) => self.check_network(url),
                None => Ok(()),
            },

            // ---------- file: read ----------
            "file.read" | "file_read" | "file.exists" | "file_exists" | "file.size"
            | "file_size" | "file.list_dir" | "file_list_dir" | "ls" => match arg_str(0) {
                Some(path) => self.check_fs(path, FsMode::Read),
                None => Ok(()),
            },

            // ---------- file: write ----------
            "file.write" | "file_write" | "file.append" | "file_append" | "file.delete"
            | "file_delete" | "file.mkdir" | "file_mkdir" => match arg_str(0) {
                Some(path) => self.check_fs(path, FsMode::Write),
                None => Ok(()),
            },

            // copy needs both the source (read) and the destination (write)
            "file.copy" | "file_copy" => {
                if let Some(src) = arg_str(0) {
                    self.check_fs(src, FsMode::Read)?;
                }
                if let Some(dst) = arg_str(1) {
                    self.check_fs(dst, FsMode::Write)?;
                }
                Ok(())
            }

            // ---------- system ----------
            "sys.info" | "sys_info" => self.check_system_metrics(),
            "sys.args" | "sys_args" | "args" => {
                Err("system access denied: process arguments are not available when permissions are declared".into())
            }
            "env.get" | "env_get" | "env.set" | "env_set" | "sys.env_all"
            | "sys_env_all" => {
                Err("system access denied: environment access requires an explicit capability".into())
            }
            "memory.open" | "memory_open" | "memory.add" | "memory_add"
            | "memory.get" | "memory_get" | "memory.clear" | "memory_clear"
            | "memory.count" | "memory_count" => {
                Err("memory access denied: persistent memory is not available when permissions are declared".into())
            }

            _ => Ok(()),
        }
    }

    /// Is sending a request to this URL allowed?
    pub fn check_network(&self, url: &str) -> Result<(), String> {
        let host = extract_host(url).ok_or_else(|| {
            format!("network denied: cannot extract a host from URL \"{}\"", url)
        })?;

        let allowed = self
            .network
            .as_ref()
            .ok_or_else(|| "network denied: no `network` key declared in the permissions block".to_string())?;

        let hit = allowed
            .iter()
            .any(|entry| extract_host_or_bare(entry) == Some(host.clone()));

        if hit {
            Ok(())
        } else if allowed.is_empty() {
            Err("network denied: the allowlist is empty (no host is permitted)".to_string())
        } else {
            Err(format!(
                "network denied: host \"{}\" is not in the allowlist (allowed: {})",
                host,
                allowed.join(", ")
            ))
        }
    }

    /// Is `mode` access to this path allowed?
    pub fn check_fs(&self, raw_path: &str, mode: FsMode) -> Result<(), String> {
        let (roots, label) = match mode {
            FsMode::Read => (&self.fs_read, "read"),
            FsMode::Write => (&self.fs_write, "write"),
        };

        let roots = roots.as_ref().ok_or_else(|| {
            format!(
                "file access denied: no `filesystem {{ {}: [...] }}` key declared",
                label
            )
        })?;

        let base = current_dir();
        let request = normalize_path(raw_path, &base);
        let normalized_roots: Vec<PathBuf> = roots
            .iter()
            .map(|r| normalize_path(r, &base))
            .collect();

        if normalized_roots.iter().any(|root| {
            // Comparison is by component, not by string; so root `/allowed`
            // does not accept the path `/allowed-evil`.
            request == *root || request.starts_with(root)
        }) {
            Ok(())
        } else {
            Err(format!(
                "file access denied: path \"{}\" is outside the declared {} roots (allowed: {})",
                request.display(),
                label,
                roots.join(", ")
            ))
        }
    }

    pub fn check_system_metrics(&self) -> Result<(), String> {
        if self.system_metrics {
            Ok(())
        } else {
            Err("system access denied: sys.info requires `system {{ metrics: true }}`".to_string())
        }
    }
}

/// Turn a policy error into a runtime error carrying the real call site.
pub fn denied(message: String, span: Span) -> RuntimeError {
    RuntimeError::PermissionDenied { message, span }
}

/// Execution caps (`limits { ... }`).
#[derive(Debug, Clone, Default)]
pub struct Limits {
    /// Concurrency cap. The current interpreter is single-threaded, so this cap
    /// is never violated; it becomes meaningful once async lands (clause 18.7).
    pub concurrency: Option<u64>,
    /// Deadline for I/O operations (currently applied to HTTP requests).
    pub timeout_ms: Option<u64>,
}

impl Limits {
    pub fn from_block(block: &LimitsBlock) -> Self {
        let mut limits = Limits::default();
        for entry in &block.entries {
            match (entry.name.name.as_str(), &entry.value) {
                ("concurrency", aec_ast::Literal::Int(n)) if *n >= 0 => {
                    limits.concurrency = Some(*n as u64);
                }
                ("timeout", aec_ast::Literal::Duration(d)) => {
                    limits.timeout_ms = d.value.checked_mul(d.unit.to_ms());
                }
                ("timeout", aec_ast::Literal::Int(ms)) if *ms >= 0 => {
                    // `timeout: 500` → milliseconds
                    limits.timeout_ms = Some(*ms as u64);
                }
                _ => {}
            }
        }
        limits
    }

    /// Effective HTTP deadline.
    pub fn http_timeout(&self) -> Duration {
        Duration::from_millis(self.timeout_ms.unwrap_or(DEFAULT_HTTP_TIMEOUT_MS))
    }
}

fn current_dir() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

/// Extract the host from a URL: port and userinfo are dropped, letters lowercased.
///
/// A `/path` without a scheme is supported too (`example.com/x`).
pub fn extract_host(url: &str) -> Option<String> {
    let after_scheme = match url.split_once("://") {
        Some((_, rest)) => rest,
        None => url,
    };
    let authority = after_scheme
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default();
    if authority.is_empty() {
        return None;
    }

    // drop userinfo: user:pass@host
    let authority = authority.rsplit('@').next().unwrap_or(authority);

    // IPv6: [::1]:8080
    let host = if let Some(rest) = authority.strip_prefix('[') {
        rest.split(']').next().unwrap_or_default()
    } else {
        authority.split(':').next().unwrap_or_default()
    };

    if host.is_empty() {
        None
    } else {
        Some(host.to_ascii_lowercase())
    }
}

/// For the allowlist: accepts both a full URL and a bare host.
fn extract_host_or_bare(entry: &str) -> Option<String> {
    extract_host(entry.trim())
}

/// Turn a path into absolute, normalized form.
///
/// If the path (or its nearest existing ancestor) is on disk, it is
/// `canonicalize`d so symlinks are resolved. Otherwise normalization is
/// **textual** (`.`, `..`, duplicated `/`) so we don't depend on the file
/// existing.
pub fn normalize_path(raw: &str, base: &Path) -> PathBuf {
    let path = Path::new(raw);
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    };

    if let Ok(canonical) = std::fs::canonicalize(&absolute) {
        return canonical;
    }

    // resolve the nearest existing ancestor, append the rest.
    let mut existing = absolute.clone();
    let mut tail: Vec<std::ffi::OsString> = Vec::new();
    while !existing.exists() {
        let Some(name) = existing.file_name() else {
            break;
        };
        tail.push(name.to_os_string());
        match existing.parent() {
            Some(parent) => existing = parent.to_path_buf(),
            None => break,
        }
    }

    let mut out = std::fs::canonicalize(&existing).unwrap_or_else(|_| lexical_normalize(&existing));
    for segment in tail.iter().rev() {
        out.push(segment);
    }
    out
}

/// Textual normalization: drop `.`, resolve `..` and duplicated slashes.
fn lexical_normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}
