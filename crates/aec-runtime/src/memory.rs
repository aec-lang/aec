//! Memory — حافظه‌ی مکالمه (in-memory فعلاً)

use crate::value::Value;
use std::collections::HashMap;

/// یه پیام توی حافظه
#[derive(Debug, Clone)]
pub struct Message {
    pub role: String,
    pub content: String,
}

/// حافظه‌ی سراسری
#[derive(Debug)]
pub struct Memory {
    pub conversations: HashMap<String, Vec<Message>>,
}

impl Memory {
    pub fn new() -> Self {
        Self {
            conversations: HashMap::new(),
        }
    }

    pub fn add(&mut self, conv_id: &str, role: &str, content: &str) {
        self.conversations
            .entry(conv_id.to_string())
            .or_insert_with(Vec::new)
            .push(Message {
                role: role.to_string(),
                content: content.to_string(),
            });
    }

    pub fn get(&self, conv_id: &str) -> Vec<Message> {
        self.conversations
            .get(conv_id)
            .cloned()
            .unwrap_or_default()
    }

    pub fn clear(&mut self, conv_id: &str) {
        self.conversations.remove(conv_id);
    }

    pub fn len(&self, conv_id: &str) -> usize {
        self.conversations
            .get(conv_id)
            .map(|v| v.len())
            .unwrap_or(0)
    }

    /// تبدیل پیام‌ها به Value::Array (برای پاس دادن به LLM)
    pub fn to_value(&self, conv_id: &str) -> Value {
        let msgs = self.get(conv_id);
        let arr: Vec<Value> = msgs
            .iter()
            .map(|m| {
                let mut obj = HashMap::new();
                obj.insert("role".to_string(), Value::String(m.role.clone()));
                obj.insert("content".to_string(), Value::String(m.content.clone()));
                Value::Object(obj)
            })
            .collect();
        Value::Array(arr)
    }
}
