//! Standard Library — AEC built-in functions

use crate::errors::RuntimeError;
use crate::value::Value;
use aec_ast::Span;
use std::collections::HashMap;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH, Duration};

/// Run a built-in
pub fn call_builtin(
    name: &str,
    args: &[Value],
    span: Span,
    limits: &crate::permissions::Limits,
) -> Result<Option<Value>, RuntimeError> {
    let result: Value = match name {
        // ---------- File I/O ----------
        "file.read" | "file_read" => {
            if args.len() != 1 {
                return Err(RuntimeError::WrongArgCount { expected: 1, got: args.len(), span });
            }
            let path = match &args[0] {
                Value::String(s) => s.clone(),
                v => return Err(RuntimeError::TypeError {
                    message: format!("file.read needs string, got {}", v.type_name()),
                    span,
                }),
            };
            let content = fs::read_to_string(&path).map_err(|e| RuntimeError::Generic {
                message: format!("failed to read '{}': {}", path, e),
                span,
            })?;
            Value::String(content)
        }

        "file.write" | "file_write" => {
            if args.len() != 2 {
                return Err(RuntimeError::WrongArgCount { expected: 2, got: args.len(), span });
            }
            let path = match &args[0] {
                Value::String(s) => s.clone(),
                v => return Err(RuntimeError::TypeError {
                    message: format!("file.write path needs string, got {}", v.type_name()),
                    span,
                }),
            };
            let content = match &args[1] {
                Value::String(s) => s.clone(),
                v => return Err(RuntimeError::TypeError {
                    message: format!("file.write content needs string, got {}", v.type_name()),
                    span,
                }),
            };
            fs::write(&path, content).map_err(|e| RuntimeError::Generic {
                message: format!("failed to write '{}': {}", path, e),
                span,
            })?;
            Value::None
        }

        "file.append" | "file_append" => {
            if args.len() != 2 {
                return Err(RuntimeError::WrongArgCount { expected: 2, got: args.len(), span });
            }
            let path = match &args[0] {
                Value::String(s) => s.clone(),
                v => return Err(RuntimeError::TypeError {
                    message: format!("file.append path needs string, got {}", v.type_name()),
                    span,
                }),
            };
            let content = match &args[1] {
                Value::String(s) => s.clone(),
                v => return Err(RuntimeError::TypeError {
                    message: format!("file.append content needs string, got {}", v.type_name()),
                    span,
                }),
            };
            use std::io::Write;
            let mut file = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .map_err(|e| RuntimeError::Generic {
                    message: format!("failed to open '{}': {}", path, e),
                    span,
                })?;
            file.write_all(content.as_bytes()).map_err(|e| RuntimeError::Generic {
                message: format!("failed to append: {}", e),
                span,
            })?;
            Value::None
        }

        "file.exists" | "file_exists" => {
            if args.len() != 1 {
                return Err(RuntimeError::WrongArgCount { expected: 1, got: args.len(), span });
            }
            let path = match &args[0] {
                Value::String(s) => s.clone(),
                v => return Err(RuntimeError::TypeError {
                    message: format!("file.exists needs string, got {}", v.type_name()),
                    span,
                }),
            };
            Value::Bool(std::path::Path::new(&path).exists())
        }

        "file.delete" | "file_delete" => {
            if args.len() != 1 {
                return Err(RuntimeError::WrongArgCount { expected: 1, got: args.len(), span });
            }
            let path = match &args[0] {
                Value::String(s) => s.clone(),
                v => return Err(RuntimeError::TypeError {
                    message: format!("file.delete needs string, got {}", v.type_name()),
                    span,
                }),
            };
            fs::remove_file(&path).map_err(|e| RuntimeError::Generic {
                message: format!("failed to delete '{}': {}", path, e),
                span,
            })?;
            Value::None
        }

        // ---------- Env ----------
        "env.get" | "env_get" => {
            if args.len() != 1 {
                return Err(RuntimeError::WrongArgCount { expected: 1, got: args.len(), span });
            }
            let name = match &args[0] {
                Value::String(s) => s.clone(),
                v => return Err(RuntimeError::TypeError {
                    message: format!("env.get needs string, got {}", v.type_name()),
                    span,
                }),
            };
            match std::env::var(&name) {
                Ok(val) => Value::String(val),
                Err(_) => Value::None,
            }
        }

        "env.set" | "env_set" => {
            if args.len() != 2 {
                return Err(RuntimeError::WrongArgCount { expected: 2, got: args.len(), span });
            }
            let name = match &args[0] {
                Value::String(s) => s.clone(),
                _ => return Err(RuntimeError::TypeError {
                    message: "env.set name needs string".to_string(),
                    span,
                }),
            };
            let val = match &args[1] {
                Value::String(s) => s.clone(),
                _ => return Err(RuntimeError::TypeError {
                    message: "env.set value needs string".to_string(),
                    span,
                }),
            };
            std::env::set_var(name, val);
            Value::None
        }

        // ---------- HTTP ----------
        "http.get" | "http_get" => {
            if args.is_empty() {
                return Err(RuntimeError::WrongArgCount { expected: 1, got: 0, span });
            }
            let url = match &args[0] {
                Value::String(s) => s.clone(),
                v => return Err(RuntimeError::TypeError {
                    message: format!("http.get needs string, got {}", v.type_name()),
                    span,
                }),
            };
            let client = reqwest::blocking::Client::builder()
                .timeout(limits.http_timeout())
                .build()
                .map_err(|e| RuntimeError::Generic {
                    message: format!("failed to build client: {}", e),
                    span,
                })?;
            let response = client.get(&url).header("User-Agent", "AEC/0.1").send().map_err(|e| RuntimeError::Generic {
                message: format!("HTTP GET failed: {}", e),
                span,
            })?;
            let status = response.status().as_u16() as i64;
            let text = response.text().map_err(|e| RuntimeError::Generic {
                message: format!("failed to read response: {}", e),
                span,
            })?;
            let mut obj = HashMap::new();
            obj.insert("status".to_string(), Value::Int(status));
            obj.insert("text".to_string(), Value::String(text));
            Value::Object(obj)
        }

        "http.post" | "http_post" => {
            if args.len() < 2 {
                return Err(RuntimeError::WrongArgCount { expected: 2, got: args.len(), span });
            }
            let url = match &args[0] {
                Value::String(s) => s.clone(),
                _ => return Err(RuntimeError::TypeError {
                    message: "http.post url needs string".to_string(),
                    span,
                }),
            };
            let body = match &args[1] {
                Value::String(s) => s.clone(),
                _ => return Err(RuntimeError::TypeError {
                    message: "http.post body needs string".to_string(),
                    span,
                }),
            };
            let client = reqwest::blocking::Client::builder()
                .timeout(limits.http_timeout())
                .build()
                .map_err(|e| RuntimeError::Generic {
                    message: format!("failed to build client: {}", e),
                    span,
                })?;
            let response = client
                .post(&url)
                .header("Content-Type", "application/json")
                .body(body)
                .send()
                .map_err(|e| RuntimeError::Generic {
                    message: format!("HTTP POST failed: {}", e),
                    span,
                })?;
            let status = response.status().as_u16() as i64;
            let text = response.text().map_err(|e| RuntimeError::Generic {
                message: format!("failed to read response: {}", e),
                span,
            })?;
            let mut obj = HashMap::new();
            obj.insert("status".to_string(), Value::Int(status));
            obj.insert("text".to_string(), Value::String(text));
            Value::Object(obj)
        }

        // ---------- JSON ----------
        "json.parse" | "json_parse" => {
            if args.len() != 1 {
                return Err(RuntimeError::WrongArgCount { expected: 1, got: args.len(), span });
            }
            let text = match &args[0] {
                Value::String(s) => s.clone(),
                _ => return Err(RuntimeError::TypeError {
                    message: "json.parse needs string".to_string(),
                    span,
                }),
            };
            let json: serde_json::Value = serde_json::from_str(&text).map_err(|e| {
                RuntimeError::Generic {
                    message: format!("invalid JSON: {}", e),
                    span,
                }
            })?;
            json_to_value(&json)?
        }

        "json.stringify" | "json_stringify" => {
            if args.len() != 1 {
                return Err(RuntimeError::WrongArgCount { expected: 1, got: args.len(), span });
            }
            let json = value_to_json(&args[0]);
            Value::String(json.to_string())
        }

        // ---------- Time ----------
        "time.now_ms" | "time_now_ms" | "now_ms" => {
            let ms = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0);
            Value::Int(ms)
        }

        "time.now_sec" | "time_now_sec" | "now" => {
            let sec = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            Value::Int(sec)
        }

        "time.sleep" | "time_sleep" | "sleep" => {
            if args.len() != 1 {
                return Err(RuntimeError::WrongArgCount { expected: 1, got: args.len(), span });
            }
            let ms = match &args[0] {
                Value::Int(n) => *n as u64,
                _ => return Err(RuntimeError::TypeError {
                    message: "time.sleep needs int (milliseconds)".to_string(),
                    span,
                }),
            };
            std::thread::sleep(Duration::from_millis(ms));
            Value::None
        }

        // ---------- System ----------
        "sys.exit" | "sys_exit" | "exit" => {
            let code = if args.is_empty() {
                0
            } else {
                match &args[0] {
                    Value::Int(n) => *n as i32,
                    _ => 0,
                }
            };
            std::process::exit(code);
        }

        "sys.args" | "sys_args" | "args" => {
            let args: Vec<Value> = std::env::args()
                .map(Value::String)
                .collect();
            Value::Array(args)
        }

        _ => return Ok(None),
    };
    Ok(Some(result))
}

/// Convert serde_json::Value to Value
fn json_to_value(json: &serde_json::Value) -> Result<Value, RuntimeError> {
    Ok(match json {
        serde_json::Value::Null => Value::None,
        serde_json::Value::Bool(b) => Value::Bool(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Int(i)
            } else if let Some(f) = n.as_f64() {
                Value::Float(f)
            } else {
                Value::None
            }
        }
        serde_json::Value::String(s) => Value::String(s.clone()),
        serde_json::Value::Array(arr) => {
            let mut values = Vec::new();
            for v in arr {
                values.push(json_to_value(v)?);
            }
            Value::Array(values)
        }
        serde_json::Value::Object(obj) => {
            let mut map = HashMap::new();
            for (k, v) in obj {
                map.insert(k.clone(), json_to_value(v)?);
            }
            Value::Object(map)
        }
    })
}

/// Convert Value to serde_json::Value
fn value_to_json(value: &Value) -> serde_json::Value {
    match value {
        Value::None => serde_json::Value::Null,
        Value::Bool(b) => serde_json::Value::Bool(*b),
        Value::Int(n) => serde_json::Value::Number((*n).into()),
        Value::Float(f) => serde_json::Number::from_f64(*f)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        Value::String(s) => serde_json::Value::String(s.clone()),
        Value::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(value_to_json).collect())
        }
        Value::Object(obj) => {
            let mut map = serde_json::Map::new();
            for (k, v) in obj {
                map.insert(k.clone(), value_to_json(v));
            }
            serde_json::Value::Object(map)
        }
        Value::Function(_) | Value::Closure(_) => serde_json::Value::Null,
        // A result is not JSON-serializable; render it as its display text.
        Value::Result(_) => serde_json::Value::String(value.to_string()),
    }
}

// ============================================================
// Math Library + Array/Collection Functions
// ============================================================

pub fn call_builtin2(name: &str, args: &[Value], span: Span) -> Result<Option<Value>, RuntimeError> {
    let result = match name {
        // ---------- Math ----------
        "math.pi" | "math_pi" | "pi" => Value::Float(std::f64::consts::PI),
        "math.e" | "math_e" | "e" => Value::Float(std::f64::consts::E),
        "math.tau" | "math_tau" => Value::Float(std::f64::consts::TAU),

        "math.sin" | "math_sin" | "sin" => {
            let x = as_float(&args[0], span, "sin")?;
            Value::Float(x.sin())
        }
        "math.cos" | "math_cos" | "cos" => {
            let x = as_float(&args[0], span, "cos")?;
            Value::Float(x.cos())
        }
        "math.tan" | "math_tan" | "tan" => {
            let x = as_float(&args[0], span, "tan")?;
            Value::Float(x.tan())
        }
        "math.log" | "math_log" | "log" => {
            let x = as_float(&args[0], span, "log")?;
            Value::Float(x.ln())
        }
        "math.log10" | "log10" => {
            let x = as_float(&args[0], span, "log10")?;
            Value::Float(x.log10())
        }
        "math.exp" | "exp" => {
            let x = as_float(&args[0], span, "exp")?;
            Value::Float(x.exp())
        }
        "math.floor" | "floor" => {
            let x = as_float(&args[0], span, "floor")?;
            Value::Int(x.floor() as i64)
        }
        "math.ceil" | "ceil" => {
            let x = as_float(&args[0], span, "ceil")?;
            Value::Int(x.ceil() as i64)
        }
        "math.round" | "round" => {
            let x = as_float(&args[0], span, "round")?;
            Value::Int(x.round() as i64)
        }
        "math.random" | "random" => {
            use std::time::{SystemTime, UNIX_EPOCH};
            let seed = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(42);
            // simple LCG
            let r = (seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407)) >> 33;
            Value::Float((r as f64) / (u32::MAX as f64))
        }
        "math.random_int" | "random_int" => {
            if args.len() != 2 {
                return Err(RuntimeError::WrongArgCount { expected: 2, got: args.len(), span });
            }
            let min = match &args[0] {
                Value::Int(n) => *n,
                _ => return Err(RuntimeError::TypeError {
                    message: "random_int needs ints".to_string(),
                    span,
                }),
            };
            let max = match &args[1] {
                Value::Int(n) => *n,
                _ => return Err(RuntimeError::TypeError {
                    message: "random_int needs ints".to_string(),
                    span,
                }),
            };
            use std::time::{SystemTime, UNIX_EPOCH};
            let seed = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(42);
            let r = (seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407)) >> 33;
            let range = (max - min).abs() as u64 + 1;
            let val = min + ((r % range) as i64);
            Value::Int(val)
        }

        // ---------- Collections ----------
        "sort" => {
            if args.len() != 1 {
                return Err(RuntimeError::WrongArgCount { expected: 1, got: args.len(), span });
            }
            match &args[0] {
                Value::Array(arr) => {
                    let mut sorted = arr.clone();
                    sorted.sort_by(|a, b| compare_values(a, b));
                    Value::Array(sorted)
                }
                v => return Err(RuntimeError::TypeError {
                    message: format!("sort() needs array, got {}", v.type_name()),
                    span,
                }),
            }
        }
        "reverse" => {
            if args.len() != 1 {
                return Err(RuntimeError::WrongArgCount { expected: 1, got: args.len(), span });
            }
            match &args[0] {
                Value::Array(arr) => {
                    let mut rev = arr.clone();
                    rev.reverse();
                    Value::Array(rev)
                }
                Value::String(s) => {
                    Value::String(s.chars().rev().collect())
                }
                v => return Err(RuntimeError::TypeError {
                    message: format!("reverse() needs array or string, got {}", v.type_name()),
                    span,
                }),
            }
        }
        "first" => match &args[0] {
            Value::Array(arr) => arr.first().cloned().unwrap_or(Value::None),
            Value::String(s) => s.chars().next()
                .map(|c| Value::String(c.to_string()))
                .unwrap_or(Value::None),
            v => return Err(RuntimeError::TypeError {
                message: format!("first() needs array or string, got {}", v.type_name()),
                span,
            }),
        },
        "last" => match &args[0] {
            Value::Array(arr) => arr.last().cloned().unwrap_or(Value::None),
            Value::String(s) => s.chars().last()
                .map(|c| Value::String(c.to_string()))
                .unwrap_or(Value::None),
            v => return Err(RuntimeError::TypeError {
                message: format!("last() needs array or string, got {}", v.type_name()),
                span,
            }),
        },
        "slice" => {
            if args.len() < 2 {
                return Err(RuntimeError::WrongArgCount { expected: 2, got: args.len(), span });
            }
            let start = match &args[1] {
                Value::Int(n) => *n as usize,
                _ => 0,
            };
            let end = if args.len() >= 3 {
                match &args[2] {
                    Value::Int(n) => *n as usize,
                    _ => usize::MAX,
                }
            } else {
                usize::MAX
            };
            match &args[0] {
                Value::Array(arr) => {
                    let end = end.min(arr.len());
                    let start = start.min(end);
                    Value::Array(arr[start..end].to_vec())
                }
                Value::String(s) => {
                    let chars: Vec<char> = s.chars().collect();
                    let end = end.min(chars.len());
                    let start = start.min(end);
                    Value::String(chars[start..end].iter().collect())
                }
                v => return Err(RuntimeError::TypeError {
                    message: format!("slice() needs array or string, got {}", v.type_name()),
                    span,
                }),
            }
        }

        // ---------- String extras ----------
        "starts_with" => match (&args[0], &args[1]) {
            (Value::String(s), Value::String(p)) => Value::Bool(s.starts_with(p.as_str())),
            _ => return Err(RuntimeError::TypeError {
                message: "starts_with needs two strings".to_string(),
                span,
            }),
        },
        "ends_with" => match (&args[0], &args[1]) {
            (Value::String(s), Value::String(p)) => Value::Bool(s.ends_with(p.as_str())),
            _ => return Err(RuntimeError::TypeError {
                message: "ends_with needs two strings".to_string(),
                span,
            }),
        },
        "repeat" => match (&args[0], &args[1]) {
            (Value::String(s), Value::Int(n)) => Value::String(s.repeat(*n as usize)),
            _ => return Err(RuntimeError::TypeError {
                message: "repeat needs string and int".to_string(),
                span,
            }),
        },
        "char_at" => match (&args[0], &args[1]) {
            (Value::String(s), Value::Int(i)) => {
                let chars: Vec<char> = s.chars().collect();
                let idx = if *i < 0 { chars.len() as i64 + i } else { *i };
                if idx < 0 || idx as usize >= chars.len() {
                    Value::None
                } else {
                    Value::String(chars[idx as usize].to_string())
                }
            }
            _ => return Err(RuntimeError::TypeError {
                message: "char_at needs string and int".to_string(),
                span,
            }),
        },

        _ => return Ok(None),
    };
    Ok(Some(result))
}

fn as_float(v: &Value, span: Span, func: &str) -> Result<f64, RuntimeError> {
    match v {
        Value::Int(n) => Ok(*n as f64),
        Value::Float(f) => Ok(*f),
        _ => Err(RuntimeError::TypeError {
            message: format!("{}() needs number", func),
            span,
        }),
    }
}

fn compare_values(a: &Value, b: &Value) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => x.cmp(y),
        (Value::Float(x), Value::Float(y)) => {
            x.partial_cmp(y).unwrap_or(Ordering::Equal)
        }
        (Value::Int(x), Value::Float(y)) => {
            (*x as f64).partial_cmp(y).unwrap_or(Ordering::Equal)
        }
        (Value::Float(x), Value::Int(y)) => {
            x.partial_cmp(&(*y as f64)).unwrap_or(Ordering::Equal)
        }
        (Value::String(x), Value::String(y)) => x.cmp(y),
        _ => Ordering::Equal,
    }
}
