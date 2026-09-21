//! Memory — conversation memory.
//!
//! It has two modes:
//! - **in-memory** (`Memory::new`) — the default; cleared when the process ends.
//! - **persistent** (`Memory::open`) — backed by a SQLite file, survives between runs.
//!
//! In persistent mode the database is the source of truth (not a cache), so that
//! several concurrent instances on one file do not diverge.

use crate::errors::RuntimeError;
use crate::value::Value;
use aec_ast::Span;
use rusqlite::{params, Connection};
use std::collections::HashMap;
use std::fmt;
use std::path::Path;

/// A single message in memory
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub role: String,
    pub content: String,
}

/// Conversation memory
pub struct Memory {
    /// Only populated in in-memory mode.
    pub conversations: HashMap<String, Vec<Message>>,
    /// `None` = in-memory. `Some` = SQLite file.
    db: Option<Connection>,
}

impl fmt::Debug for Memory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Memory")
            .field(
                "backend",
                &if self.db.is_some() {
                    "sqlite"
                } else {
                    "in-memory"
                },
            )
            .field(
                "conversations",
                &self.conversations.keys().collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl Default for Memory {
    fn default() -> Self {
        Self::new()
    }
}

const SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS messages (
        id      INTEGER PRIMARY KEY AUTOINCREMENT,
        conv_id TEXT NOT NULL,
        role    TEXT NOT NULL,
        content TEXT NOT NULL
    );
    CREATE INDEX IF NOT EXISTS idx_messages_conv ON messages (conv_id, id);
";

fn db_error(message: String) -> RuntimeError {
    RuntimeError::Generic {
        message,
        span: Span::dummy(),
    }
}

impl Memory {
    /// In-memory store (the previous behaviour).
    pub fn new() -> Self {
        Self {
            conversations: HashMap::new(),
            db: None,
        }
    }

    /// Explicit name for the in-memory mode.
    pub fn in_memory() -> Self {
        Self::new()
    }

    /// Open (or create) persistent memory backed by a SQLite file.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, RuntimeError> {
        let path = path.as_ref();
        let conn = Connection::open(path).map_err(|e| {
            db_error(format!(
                "failed to open memory '{}': {}",
                path.display(),
                e
            ))
        })?;
        conn.execute_batch(SCHEMA)
            .map_err(|e| db_error(format!("failed to create memory table: {}", e)))?;
        Ok(Self {
            conversations: HashMap::new(),
            db: Some(conn),
        })
    }

    /// Is this memory persistent (backed by a file)?
    pub fn is_persistent(&self) -> bool {
        self.db.is_some()
    }

    pub fn add(&mut self, conv_id: &str, role: &str, content: &str) -> Result<(), RuntimeError> {
        match &self.db {
            Some(conn) => {
                conn.execute(
                    "INSERT INTO messages (conv_id, role, content) VALUES (?1, ?2, ?3)",
                    params![conv_id, role, content],
                )
                .map_err(|e| db_error(format!("failed to append message: {}", e)))?;
            }
            None => {
                self.conversations
                    .entry(conv_id.to_string())
                    .or_default()
                    .push(Message {
                        role: role.to_string(),
                        content: content.to_string(),
                    });
            }
        }
        Ok(())
    }

    pub fn get(&self, conv_id: &str) -> Result<Vec<Message>, RuntimeError> {
        let Some(conn) = &self.db else {
            return Ok(self.conversations.get(conv_id).cloned().unwrap_or_default());
        };
        let mut stmt = conn
            .prepare("SELECT role, content FROM messages WHERE conv_id = ?1 ORDER BY id")
            .map_err(|e| db_error(format!("failed to read memory: {}", e)))?;
        let rows = stmt
            .query_map(params![conv_id], |row| {
                Ok(Message {
                    role: row.get(0)?,
                    content: row.get(1)?,
                })
            })
            .map_err(|e| db_error(format!("failed to read memory: {}", e)))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| db_error(format!("failed to read memory messages: {}", e)))
    }

    pub fn clear(&mut self, conv_id: &str) -> Result<(), RuntimeError> {
        match &self.db {
            Some(conn) => {
                conn.execute("DELETE FROM messages WHERE conv_id = ?1", params![conv_id])
                    .map_err(|e| db_error(format!("failed to clear memory: {}", e)))?;
            }
            None => {
                self.conversations.remove(conv_id);
            }
        }
        Ok(())
    }

    pub fn len(&self, conv_id: &str) -> Result<usize, RuntimeError> {
        match &self.db {
            Some(conn) => {
                let count: i64 = conn
                    .query_row(
                        "SELECT COUNT(*) FROM messages WHERE conv_id = ?1",
                        params![conv_id],
                        |row| row.get(0),
                    )
                    .map_err(|e| db_error(format!("failed to count memory rows: {}", e)))?;
                Ok(count as usize)
            }
            None => Ok(self.conversations.get(conv_id).map_or(0, Vec::len)),
        }
    }

    pub fn is_empty(&self, conv_id: &str) -> Result<bool, RuntimeError> {
        Ok(self.len(conv_id)? == 0)
    }

    /// Convert messages to Value::Array (to hand to the LLM)
    pub fn to_value(&self, conv_id: &str) -> Result<Value, RuntimeError> {
        let msgs = self.get(conv_id)?;
        Ok(Value::Array(
            msgs.iter()
                .map(|m| {
                    let mut obj = HashMap::new();
                    obj.insert("role".to_string(), Value::String(m.role.clone()));
                    obj.insert("content".to_string(), Value::String(m.content.clone()));
                    Value::Object(obj)
                })
                .collect(),
        ))
    }
}
