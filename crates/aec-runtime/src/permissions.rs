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
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

/// Default HTTP cap when `limits { timeout }` is not declared.
pub const DEFAULT_HTTP_TIMEOUT_MS: u64 = 30_000;
pub const DEFAULT_MAX_RESPONSE_BYTES: u64 = 8 * 1024 * 1024;

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
        let target = parse_network_origin(url, false).map_err(|reason| {
            format!("network denied: invalid URL \"{}\": {}", url, reason)
        })?;

        let allowed = self
            .network
            .as_ref()
            .ok_or_else(|| "network denied: no `network` key declared in the permissions block".to_string())?;

        if allowed.is_empty() {
            return Err("network denied: the allowlist is empty (no host is permitted)".to_string());
        }

        let mut origins = Vec::with_capacity(allowed.len());
        for entry in allowed {
            let entry = entry.trim();
            let origin = parse_network_origin(entry, true).map_err(|reason| {
                format!(
                    "network denied: invalid allowlist entry \"{}\": {}",
                    entry, reason
                )
            })?;
            origins.push(origin);
        }

        if origins.contains(&target) {
            Ok(())
        } else {
            Err(format!(
                "network denied: {}://{}:{} is not in the allowlist (allowed: {})",
                target.scheme,
                target.host,
                target.port,
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
    pub max_response_bytes: Option<u64>,
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
                ("max_response_bytes" | "response_bytes", aec_ast::Literal::Int(bytes))
                    if *bytes >= 0 =>
                {
                    limits.max_response_bytes = Some(*bytes as u64);
                }
                (
                    "max_response_bytes" | "response_bytes",
                    aec_ast::Literal::ByteSize(bytes),
                ) => {
                    limits.max_response_bytes = Some(*bytes);
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

    pub fn max_response_bytes(&self) -> u64 {
        self.max_response_bytes
            .unwrap_or(DEFAULT_MAX_RESPONSE_BYTES)
    }
}

pub fn build_http_client(
    limits: &Limits,
    restricted: bool,
) -> Result<reqwest::blocking::Client, reqwest::Error> {
    reqwest::blocking::Client::builder()
        .timeout(limits.http_timeout())
        .redirect(if restricted {
            reqwest::redirect::Policy::none()
        } else {
            reqwest::redirect::Policy::default()
        })
        .build()
}

pub fn read_limited_response(
    response: &mut reqwest::blocking::Response,
    operation: &str,
    max_bytes: u64,
    span: Span,
) -> Result<String, RuntimeError> {
    if let Some(length) = response.content_length() {
        if length > max_bytes {
            return Err(RuntimeError::Generic {
                message: format!(
                    "{} response exceeds the {} byte limit",
                    operation, max_bytes
                ),
                span,
            });
        }
    }

    let mut body = Vec::new();
    response
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut body)
        .map_err(|error| RuntimeError::Generic {
            message: format!("{} failed to read response: {}", operation, error),
            span,
        })?;

    if u64::try_from(body.len()).unwrap_or(u64::MAX) > max_bytes {
        return Err(RuntimeError::Generic {
            message: format!(
                "{} response exceeds the {} byte limit",
                operation, max_bytes
            ),
            span,
        });
    }

    String::from_utf8(body).map_err(|error| RuntimeError::Generic {
        message: format!("{} response is not valid UTF-8: {}", operation, error),
        span,
    })
}

fn current_dir() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NetworkOrigin {
    scheme: String,
    host: String,
    port: u16,
}

fn parse_network_origin(raw: &str, allow_bare: bool) -> Result<NetworkOrigin, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("URL is empty".to_string());
    }
    if raw.chars().any(|character| character.is_control() || character.is_whitespace()) {
        return Err("URL contains whitespace or control characters".to_string());
    }
    if raw.contains('\\') {
        return Err("URL contains a backslash".to_string());
    }
    if allow_bare && !trimmed.contains("://") {
        let prefix = trimmed.split(':').next().unwrap_or_default();
        if matches!(prefix.to_ascii_lowercase().as_str(), "http" | "https") {
            return Err("URL scheme must include ://".to_string());
        }
    }

    let candidate = if allow_bare && !trimmed.contains("://") {
        format!("https://{trimmed}")
    } else {
        trimmed.to_string()
    };
    let parsed = reqwest::Url::parse(&candidate).map_err(|error| format!("invalid URL: {}", error))?;
    let scheme = parsed.scheme().to_ascii_lowercase();
    if scheme != "http" && scheme != "https" {
        return Err(format!("unsupported URL scheme \"{}\"", parsed.scheme()));
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err("URL userinfo is not allowed".to_string());
    }

    let raw_host = parsed
        .host_str()
        .ok_or_else(|| "URL does not contain a host".to_string())?;
    let host = raw_host
        .trim_start_matches('[')
        .trim_end_matches(']')
        .trim_end_matches('.')
        .to_ascii_lowercase();
    if host.is_empty() || host.contains('/') || host.contains('@') {
        return Err("URL contains an invalid host".to_string());
    }

    let port = parsed
        .port_or_known_default()
        .ok_or_else(|| "URL does not contain a valid port".to_string())?;
    if port == 0 {
        return Err("URL port must be greater than zero".to_string());
    }

    Ok(NetworkOrigin { scheme, host, port })
}

pub fn extract_host(url: &str) -> Option<String> {
    parse_network_origin(url, true).ok().map(|origin| origin.host)
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
