//! Extended Standard Library — Sprint 9
//! shell, regex, crypto, uuid, sys, more files, more http

use crate::errors::RuntimeError;
use crate::value::Value;
use aec_ast::Span;
use std::collections::HashMap;
use std::fs;
use std::process::Command;

pub fn call_extended(
    name: &str,
    args: &[Value],
    span: Span,
    limits: &crate::permissions::Limits,
) -> Result<Option<Value>, RuntimeError> {
    let result = match name {
        // ============================================================
        // SHELL
        // ============================================================
        "shell.run" | "shell_run" => {
            if args.len() != 1 {
                return Err(RuntimeError::WrongArgCount { expected: 1, got: args.len(), span });
            }
            let cmd = match &args[0] {
                Value::String(s) => s.clone(),
                v => return Err(RuntimeError::TypeError {
                    message: format!("shell.run needs string, got {}", v.type_name()),
                    span,
                }),
            };
            let output = Command::new("sh")
                .arg("-c")
                .arg(&cmd)
                .output()
                .map_err(|e| RuntimeError::Generic {
                    message: format!("shell command failed: {}", e),
                    span,
                })?;
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let code = output.status.code().unwrap_or(-1) as i64;
            let mut obj = HashMap::new();
            obj.insert("stdout".to_string(), Value::String(stdout));
            obj.insert("stderr".to_string(), Value::String(stderr));
            obj.insert("exit_code".to_string(), Value::Int(code));
            obj.insert("success".to_string(), Value::Bool(output.status.success()));
            Value::Object(obj)
        }

        // ============================================================
        // REGEX
        // ============================================================
        "regex.match" | "regex_match" => {
            if args.len() != 2 {
                return Err(RuntimeError::WrongArgCount { expected: 2, got: args.len(), span });
            }
            let (pattern, text) = match (&args[0], &args[1]) {
                (Value::String(p), Value::String(t)) => (p.clone(), t.clone()),
                _ => return Err(RuntimeError::TypeError {
                    message: "regex.match needs two strings".to_string(),
                    span,
                }),
            };
            let re = regex::Regex::new(&pattern).map_err(|e| RuntimeError::Generic {
                message: format!("invalid regex: {}", e),
                span,
            })?;
            Value::Bool(re.is_match(&text))
        }

        "regex.find" | "regex_find" => {
            if args.len() != 2 {
                return Err(RuntimeError::WrongArgCount { expected: 2, got: args.len(), span });
            }
            let (pattern, text) = match (&args[0], &args[1]) {
                (Value::String(p), Value::String(t)) => (p.clone(), t.clone()),
                _ => return Err(RuntimeError::TypeError {
                    message: "regex.find needs two strings".to_string(),
                    span,
                }),
            };
            let re = regex::Regex::new(&pattern).map_err(|e| RuntimeError::Generic {
                message: format!("invalid regex: {}", e),
                span,
            })?;
            match re.find(&text) {
                Some(m) => Value::String(m.as_str().to_string()),
                None => Value::None,
            }
        }

        "regex.find_all" | "regex_find_all" => {
            if args.len() != 2 {
                return Err(RuntimeError::WrongArgCount { expected: 2, got: args.len(), span });
            }
            let (pattern, text) = match (&args[0], &args[1]) {
                (Value::String(p), Value::String(t)) => (p.clone(), t.clone()),
                _ => return Err(RuntimeError::TypeError {
                    message: "regex.find_all needs two strings".to_string(),
                    span,
                }),
            };
            let re = regex::Regex::new(&pattern).map_err(|e| RuntimeError::Generic {
                message: format!("invalid regex: {}", e),
                span,
            })?;
            let matches: Vec<Value> = re.find_iter(&text)
                .map(|m| Value::String(m.as_str().to_string()))
                .collect();
            Value::Array(matches)
        }

        "regex.replace" | "regex_replace" => {
            if args.len() != 3 {
                return Err(RuntimeError::WrongArgCount { expected: 3, got: args.len(), span });
            }
            let (pattern, text, replacement) = match (&args[0], &args[1], &args[2]) {
                (Value::String(p), Value::String(t), Value::String(r)) => (p.clone(), t.clone(), r.clone()),
                _ => return Err(RuntimeError::TypeError {
                    message: "regex.replace needs three strings".to_string(),
                    span,
                }),
            };
            let re = regex::Regex::new(&pattern).map_err(|e| RuntimeError::Generic {
                message: format!("invalid regex: {}", e),
                span,
            })?;
            Value::String(re.replace_all(&text, replacement.as_str()).to_string())
        }

        // ============================================================
        // CRYPTO
        // ============================================================
        "crypto.md5" | "crypto_md5" | "md5" => {
            if args.len() != 1 {
                return Err(RuntimeError::WrongArgCount { expected: 1, got: args.len(), span });
            }
            let text = match &args[0] {
                Value::String(s) => s.clone(),
                v => return Err(RuntimeError::TypeError {
                    message: format!("md5 needs string, got {}", v.type_name()),
                    span,
                }),
            };
            Value::String(format!("{:x}", md5::compute(text.as_bytes())))
        }

        "crypto.sha256" | "crypto_sha256" | "sha256" => {
            if args.len() != 1 {
                return Err(RuntimeError::WrongArgCount { expected: 1, got: args.len(), span });
            }
            let text = match &args[0] {
                Value::String(s) => s.clone(),
                v => return Err(RuntimeError::TypeError {
                    message: format!("sha256 needs string, got {}", v.type_name()),
                    span,
                }),
            };
            use sha2::{Sha256, Digest};
            let mut hasher = Sha256::new();
            hasher.update(text.as_bytes());
            Value::String(format!("{:x}", hasher.finalize()))
        }

        "crypto.sha512" | "crypto_sha512" | "sha512" => {
            if args.len() != 1 {
                return Err(RuntimeError::WrongArgCount { expected: 1, got: args.len(), span });
            }
            let text = match &args[0] {
                Value::String(s) => s.clone(),
                v => return Err(RuntimeError::TypeError {
                    message: format!("sha512 needs string, got {}", v.type_name()),
                    span,
                }),
            };
            use sha2::{Sha512, Digest};
            let mut hasher = Sha512::new();
            hasher.update(text.as_bytes());
            Value::String(format!("{:x}", hasher.finalize()))
        }

        "crypto.base64_encode" | "base64_encode" | "b64_encode" => {
            if args.len() != 1 {
                return Err(RuntimeError::WrongArgCount { expected: 1, got: args.len(), span });
            }
            let text = match &args[0] {
                Value::String(s) => s.clone(),
                v => return Err(RuntimeError::TypeError {
                    message: format!("base64_encode needs string, got {}", v.type_name()),
                    span,
                }),
            };
            use base64::Engine;
            Value::String(base64::engine::general_purpose::STANDARD.encode(text.as_bytes()))
        }

        "crypto.base64_decode" | "base64_decode" | "b64_decode" => {
            if args.len() != 1 {
                return Err(RuntimeError::WrongArgCount { expected: 1, got: args.len(), span });
            }
            let text = match &args[0] {
                Value::String(s) => s.clone(),
                v => return Err(RuntimeError::TypeError {
                    message: format!("base64_decode needs string, got {}", v.type_name()),
                    span,
                }),
            };
            use base64::Engine;
            let decoded = base64::engine::general_purpose::STANDARD
                .decode(text.as_bytes())
                .map_err(|e| RuntimeError::Generic {
                    message: format!("invalid base64: {}", e),
                    span,
                })?;
            Value::String(String::from_utf8_lossy(&decoded).to_string())
        }

        "uuid.v4" | "uuid_v4" | "uuid" => {
            Value::String(uuid::Uuid::new_v4().to_string())
        }

        // ============================================================
        // SYS
        // ============================================================
        "sys.info" | "sys_info" => {
            let mut obj = HashMap::new();
            obj.insert("os".to_string(), Value::String(std::env::consts::OS.to_string()));
            obj.insert("arch".to_string(), Value::String(std::env::consts::ARCH.to_string()));
            obj.insert("family".to_string(), Value::String(std::env::consts::FAMILY.to_string()));

            let hostname = Command::new("hostname")
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_else(|_| "unknown".to_string());
            obj.insert("hostname".to_string(), Value::String(hostname));

            let user = std::env::var("USER").unwrap_or_else(|_| "unknown".to_string());
            obj.insert("user".to_string(), Value::String(user));

            let pwd = std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| "unknown".to_string());
            obj.insert("cwd".to_string(), Value::String(pwd));

            Value::Object(obj)
        }

        "sys.env_all" | "sys_env_all" => {
            let mut obj = HashMap::new();
            for (k, v) in std::env::vars() {
                obj.insert(k, Value::String(v));
            }
            Value::Object(obj)
        }

        // ============================================================
        // FILE (more)
        // ============================================================
        "file.list_dir" | "file_list_dir" | "ls" => {
            if args.len() != 1 {
                return Err(RuntimeError::WrongArgCount { expected: 1, got: args.len(), span });
            }
            let path = match &args[0] {
                Value::String(s) => s.clone(),
                v => return Err(RuntimeError::TypeError {
                    message: format!("file.list_dir needs string, got {}", v.type_name()),
                    span,
                }),
            };
            let entries = fs::read_dir(&path).map_err(|e| RuntimeError::Generic {
                message: format!("can't read dir '{}': {}", path, e),
                span,
            })?;
            let mut files = Vec::new();
            for entry in entries.flatten() {
                files.push(Value::String(entry.file_name().to_string_lossy().to_string()));
            }
            Value::Array(files)
        }

        "file.mkdir" | "file_mkdir" => {
            if args.len() != 1 {
                return Err(RuntimeError::WrongArgCount { expected: 1, got: args.len(), span });
            }
            let path = match &args[0] {
                Value::String(s) => s.clone(),
                v => return Err(RuntimeError::TypeError {
                    message: format!("file.mkdir needs string, got {}", v.type_name()),
                    span,
                }),
            };
            fs::create_dir_all(&path).map_err(|e| RuntimeError::Generic {
                message: format!("can't create dir '{}': {}", path, e),
                span,
            })?;
            Value::None
        }

        "file.copy" | "file_copy" => {
            if args.len() != 2 {
                return Err(RuntimeError::WrongArgCount { expected: 2, got: args.len(), span });
            }
            let (from, to) = match (&args[0], &args[1]) {
                (Value::String(f), Value::String(t)) => (f.clone(), t.clone()),
                _ => return Err(RuntimeError::TypeError {
                    message: "file.copy needs two strings".to_string(),
                    span,
                }),
            };
            fs::copy(&from, &to).map_err(|e| RuntimeError::Generic {
                message: format!("can't copy: {}", e),
                span,
            })?;
            Value::None
        }

        "file.size" | "file_size" => {
            if args.len() != 1 {
                return Err(RuntimeError::WrongArgCount { expected: 1, got: args.len(), span });
            }
            let path = match &args[0] {
                Value::String(s) => s.clone(),
                v => return Err(RuntimeError::TypeError {
                    message: format!("file.size needs string, got {}", v.type_name()),
                    span,
                }),
            };
            let meta = fs::metadata(&path).map_err(|e| RuntimeError::Generic {
                message: format!("can't stat: {}", e),
                span,
            })?;
            Value::Int(meta.len() as i64)
        }

        // ============================================================
        // HTTP (more)
        // ============================================================
        "http.put" | "http_put" => {
            if args.len() < 2 {
                return Err(RuntimeError::WrongArgCount { expected: 2, got: args.len(), span });
            }
            let url = match &args[0] {
                Value::String(s) => s.clone(),
                _ => return Err(RuntimeError::TypeError {
                    message: "http.put url needs string".to_string(),
                    span,
                }),
            };
            let body = match &args[1] {
                Value::String(s) => s.clone(),
                _ => return Err(RuntimeError::TypeError {
                    message: "http.put body needs string".to_string(),
                    span,
                }),
            };
            let client = reqwest::blocking::Client::builder()
                .timeout(limits.http_timeout())
                .build()
                .map_err(|e| RuntimeError::Generic {
                    message: format!("client error: {}", e),
                    span,
                })?;
            let response = client
                .put(&url)
                .header("User-Agent", "AEC/0.1")
                .header("Content-Type", "application/json")
                .body(body)
                .send()
                .map_err(|e| RuntimeError::Generic {
                    message: format!("HTTP PUT failed: {}", e),
                    span,
                })?;
            let status = response.status().as_u16() as i64;
            let text = response.text().unwrap_or_default();
            let mut obj = HashMap::new();
            obj.insert("status".to_string(), Value::Int(status));
            obj.insert("text".to_string(), Value::String(text));
            Value::Object(obj)
        }

        "http.delete" | "http_delete" => {
            if args.len() != 1 {
                return Err(RuntimeError::WrongArgCount { expected: 1, got: args.len(), span });
            }
            let url = match &args[0] {
                Value::String(s) => s.clone(),
                _ => return Err(RuntimeError::TypeError {
                    message: "http.delete needs string".to_string(),
                    span,
                }),
            };
            let client = reqwest::blocking::Client::builder()
                .timeout(limits.http_timeout())
                .build()
                .map_err(|e| RuntimeError::Generic {
                    message: format!("client error: {}", e),
                    span,
                })?;
            let response = client
                .delete(&url)
                .header("User-Agent", "AEC/0.1")
                .send()
                .map_err(|e| RuntimeError::Generic {
                    message: format!("HTTP DELETE failed: {}", e),
                    span,
                })?;
            let status = response.status().as_u16() as i64;
            let text = response.text().unwrap_or_default();
            let mut obj = HashMap::new();
            obj.insert("status".to_string(), Value::Int(status));
            obj.insert("text".to_string(), Value::String(text));
            Value::Object(obj)
        }

        _ => return Ok(None),
    };
    Ok(Some(result))
}
