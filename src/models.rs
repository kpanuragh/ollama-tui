use crate::db;
use anyhow::Result;
use chrono::DateTime;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type")]
pub enum AuthMethod {
    #[serde(rename = "basic")]
    Basic {
        username: String,
        password: String,
    },
    #[serde(rename = "bearer")]
    Bearer { token: String },
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub ollama_host: String,
    pub ollama_port: u16,
    pub db_filename: String,
    #[serde(default)]
    pub auth_enabled: bool,
    #[serde(flatten)]
    pub auth_method: Option<AuthMethod>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

#[derive(Clone, Debug)]
pub struct ChatSession {
    pub id: i64,
    pub name: String,
    pub messages: Vec<Message>,
    pub created_at: DateTime<chrono::Utc>,
}

impl ChatSession {
    pub fn new(conn: &Connection) -> Result<Self> {
        let session = Self {
            id: 0, // temp id
            name: format!("Chat {}", db::get_next_session_id(conn)?),
            messages: vec![Message {
                role: Role::Assistant,
                content: "New chat started. Ask me anything!".to_string(),
            }],
            created_at: chrono::Utc::now(),
        };
        Ok(session)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}

#[derive(Deserialize, Debug)]
pub struct ModelsResponse {
    pub models: Vec<ModelDetails>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ModelDetails {
    pub name: String,
}

#[derive(Serialize, Debug)]
pub struct ChatRequest<'a> {
    pub model: &'a str,
    pub messages: &'a [Message],
    pub stream: bool,
}

#[derive(Deserialize, Debug)]
pub struct StreamChatResponse {
    pub message: Message,
    pub done: bool,
}

