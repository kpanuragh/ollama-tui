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
    #[serde(default)]
    pub theme: Theme,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Theme {
    #[serde(default = "default_chat_border_color")]
    pub chat_border_color: String,
    #[serde(default = "default_sessions_border_color")]
    pub sessions_border_color: String,
    #[serde(default = "default_user_message_color")]
    pub user_message_color: String,
    #[serde(default = "default_assistant_message_color")]
    pub assistant_message_color: String,
    #[serde(default = "default_highlight_color")]
    pub highlight_color: String,
    #[serde(default = "default_highlight_bg_color")]
    pub highlight_bg_color: String,
    #[serde(default = "default_status_bar_color")]
    pub status_bar_color: String,
    #[serde(default = "default_popup_border_color")]
    pub popup_border_color: String,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            chat_border_color: default_chat_border_color(),
            sessions_border_color: default_sessions_border_color(),
            user_message_color: default_user_message_color(),
            assistant_message_color: default_assistant_message_color(),
            highlight_color: default_highlight_color(),
            highlight_bg_color: default_highlight_bg_color(),
            status_bar_color: default_status_bar_color(),
            popup_border_color: default_popup_border_color(),
        }
    }
}

// Default color functions
fn default_chat_border_color() -> String { "yellow".to_string() }
fn default_sessions_border_color() -> String { "yellow".to_string() }
fn default_user_message_color() -> String { "cyan".to_string() }
fn default_assistant_message_color() -> String { "light_green".to_string() }
fn default_highlight_color() -> String { "black".to_string() }
fn default_highlight_bg_color() -> String { "light_green".to_string() }
fn default_status_bar_color() -> String { "dark_gray".to_string() }
fn default_popup_border_color() -> String { "yellow".to_string() }

// Helper function to parse color strings to ratatui Color
impl Theme {
    pub fn parse_color(&self, color_str: &str) -> ratatui::style::Color {
        use ratatui::style::Color;
        match color_str.to_lowercase().as_str() {
            "black" => Color::Black,
            "red" => Color::Red,
            "green" => Color::Green,
            "yellow" => Color::Yellow,
            "blue" => Color::Blue,
            "magenta" => Color::Magenta,
            "cyan" => Color::Cyan,
            "gray" | "grey" => Color::Gray,
            "dark_gray" | "dark_grey" => Color::DarkGray,
            "light_red" => Color::LightRed,
            "light_green" => Color::LightGreen,
            "light_yellow" => Color::LightYellow,
            "light_blue" => Color::LightBlue,
            "light_magenta" => Color::LightMagenta,
            "light_cyan" => Color::LightCyan,
            "white" => Color::White,
            // Parse hex colors if needed (e.g., "#FF0000")
            hex if hex.starts_with('#') && hex.len() == 7 => {
                if let Ok(r) = u8::from_str_radix(&hex[1..3], 16) {
                    if let Ok(g) = u8::from_str_radix(&hex[3..5], 16) {
                        if let Ok(b) = u8::from_str_radix(&hex[5..7], 16) {
                            return Color::Rgb(r, g, b);
                        }
                    }
                }
                Color::White // fallback
            }
            _ => Color::White, // fallback for unknown colors
        }
    }
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

#[derive(Debug, Clone)]
pub struct AgentCommand {
    pub command: String,
    pub description: String,
    pub risk_level: RiskLevel,
    pub approved: bool,
    pub executed: bool,
    pub output: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RiskLevel {
    Safe,      // Read-only operations, basic file listing
    Moderate,  // File modifications, directory operations
    High,      // System operations, network operations, deletions
    Critical,  // Dangerous operations (rm -rf, sudo, etc.)
}

