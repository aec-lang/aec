//! LLM Adapter — اتصال به OpenAI (و هر API سازگار)

use crate::errors::RuntimeError;
use crate::value::Value;
use aec_ast::Span;
use std::collections::HashMap;

/// فراخوانی OpenAI-compatible API
pub fn llm_complete(
    args: &[Value],
    span: Span,
) -> Result<Value, RuntimeError> {
    if args.is_empty() {
        return Err(RuntimeError::WrongArgCount {
            expected: 1,
            got: 0,
            span,
        });
    }

    // پارامترها
    let mut prompt = String::new();
    let mut system = String::from("You are a helpful assistant.");
    let mut model = String::from("gpt-4o-mini");
    let mut temperature = 0.7_f64;
    let mut max_tokens = 500_i64;
    let mut api_key: Option<String> = None;
    let mut base_url = String::from("https://api.openai.com/v1");

    // آرگومان اول: یا string (prompt) یا object
    match &args[0] {
        Value::String(s) => {
            prompt = s.clone();
        }
        Value::Object(o) => {
            if let Some(Value::String(s)) = o.get("prompt") {
                prompt = s.clone();
            } else if let Some(Value::String(s)) = o.get("message") {
                prompt = s.clone();
            } else if let Some(Value::String(s)) = o.get("user") {
                prompt = s.clone();
            }
            if let Some(Value::String(s)) = o.get("system") {
                system = s.clone();
            }
            if let Some(Value::String(s)) = o.get("model") {
                model = s.clone();
            }
            if let Some(Value::Float(f)) = o.get("temperature") {
                temperature = *f;
            }
            if let Some(Value::Int(i)) = o.get("temperature") {
                temperature = *i as f64;
            }
            if let Some(Value::Int(i)) = o.get("max_tokens") {
                max_tokens = *i;
            }
            if let Some(Value::String(s)) = o.get("api_key") {
                api_key = Some(s.clone());
            }
            if let Some(Value::String(s)) = o.get("base_url") {
                base_url = s.clone();
            }
        }
        v => {
            return Err(RuntimeError::TypeError {
                message: format!("llm.complete() needs string or object, got {}", v.type_name()),
                span,
            })
        }
    }

    // اگه api_key پاس نشده، از env بگیر
    let api_key = match api_key {
        Some(k) => k,
        None => std::env::var("OPENAI_API_KEY").map_err(|_| RuntimeError::Generic {
            message: "OPENAI_API_KEY not set. Either pass api_key in options or set env var."
                .to_string(),
            span,
        })?,
    };

    if prompt.is_empty() {
        return Err(RuntimeError::Generic {
            message: "prompt is empty".to_string(),
            span,
        });
    }

    // ساخت request body
    let body = serde_json::json!({
        "model": model,
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": prompt }
        ],
        "temperature": temperature,
        "max_tokens": max_tokens
    });

    // ارسال HTTP request
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| RuntimeError::Generic {
            message: format!("failed to build HTTP client: {}", e),
            span,
        })?;

    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));

    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .map_err(|e| RuntimeError::Generic {
            message: format!("HTTP request failed: {}", e),
            span,
        })?;

    let status = response.status();
    let response_text = response.text().map_err(|e| RuntimeError::Generic {
        message: format!("failed to read response: {}", e),
        span,
    })?;

    if !status.is_success() {
        return Err(RuntimeError::Generic {
            message: format!("API error ({}): {}", status, response_text),
            span,
        });
    }

    // پارس پاسخ
    let json: serde_json::Value = serde_json::from_str(&response_text).map_err(|e| {
        RuntimeError::Generic {
            message: format!("invalid JSON from API: {}", e),
            span,
        }
    })?;

    // استخراج متن
    let text = json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string();

    // استخراج token usage
    let prompt_tokens = json["usage"]["prompt_tokens"].as_i64().unwrap_or(0);
    let completion_tokens = json["usage"]["completion_tokens"].as_i64().unwrap_or(0);
    let total_tokens = json["usage"]["total_tokens"].as_i64().unwrap_or(0);
    let finish_reason = json["choices"][0]["finish_reason"]
        .as_str()
        .unwrap_or("unknown")
        .to_string();
    let model_used = json["model"].as_str().unwrap_or(&model).to_string();

    // ساخت object نتیجه
    let mut result = HashMap::new();
    result.insert("text".to_string(), Value::String(text));
    result.insert("model".to_string(), Value::String(model_used));
    result.insert("finish_reason".to_string(), Value::String(finish_reason));

    let mut tokens = HashMap::new();
    tokens.insert("prompt".to_string(), Value::Int(prompt_tokens));
    tokens.insert("completion".to_string(), Value::Int(completion_tokens));
    tokens.insert("total".to_string(), Value::Int(total_tokens));
    result.insert("tokens".to_string(), Value::Object(tokens));

    Ok(Value::Object(result))
}
