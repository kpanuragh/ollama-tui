
use dirs::config_dir;
use ratatui::widgets::ScrollbarState;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::error;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::fs;

/// Application result type.
pub type AppResult<T> = std::result::Result<T, Box<dyn error::Error>>;

#[derive(Debug, PartialEq, Eq)]
pub enum Mode {
    Chat,
    Model,
    Escape,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub domain: String,
    pub username: String,
    pub password: String,
}

/// Application state
#[derive(Debug)]
pub struct App {
    /// Is the application running?
    pub running: bool,
    /// List of chat messages.
    pub messages: Vec<String>,
    /// User input field.
    pub input: String,
    /// Selected model for Ollama.
    pub model: String,
    /// List of available models.
    pub models: Vec<String>,
    pub context: Vec<Value>,
    //create enum UI mode Chat Mode , Model Mode , Escape Mode
    pub mode: Mode,
    pub message_scroll: usize,
    pub message_scoll_state: ScrollbarState,
    pub model_scroll: usize,
    pub config: Config,
}

impl Default for App {
    fn default() -> Self {
        Self {
            running: true,
            input: String::new(),
            messages: Vec::new(),
            model: "mistral:latest".to_string(),
            models: Vec::new(),
            context: Vec::new(),
            message_scroll: 0, // 🔥 Default scroll position
            model_scroll: 0,   // 🔥 Default model selection
            message_scoll_state: ScrollbarState::new(0),
            mode: Mode::Escape,
            config: Config {
                domain: "http://localhost:11434".to_string(),
                username: "default_user".to_string(),
                password: "default_pass".to_string(),
            },
        }
    }
}

impl App {
    pub fn new() -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self::default()))
    }

    pub fn get_config_path(&mut self) -> PathBuf {
        let mut path = config_dir().expect("Could not find config directory");
        path.push("ollama-chat"); // App-specific directory
        path.push("config.json"); // Config file name
        path
    }

    /// Asynchronously reads the configuration from the config file.
    pub async fn read_config(&mut self) -> Result<bool, Box<dyn std::error::Error>> {
        let path = self.get_config_path();

        // Check if the file exists asynchronously
        if fs::metadata(&path).await.is_err() {
            return Err("Config file does not exist".into());
        }

        let content = fs::read_to_string(&path).await?;
        self.config = serde_json::from_str(&content)?;
        Ok(true)
    }

    /// Asynchronously writes the config file to the default system path.
    pub async fn write_config(&mut self) -> std::io::Result<()> {
        let path = self.get_config_path();

        // Ensure the directory exists asynchronously
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let json_str = serde_json::to_string_pretty(&self.config)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        fs::write(&path, json_str).await?;
        Ok(())
    }

    pub fn tick(&self) {}

    pub fn quit(&mut self) {
        self.running = false;
    }

    pub fn set_model(&mut self, model: String) {
        self.model = model;
    }
    pub fn set_context(&mut self, context: Vec<Value>) {
        self.context = context;
    }

    pub fn add_message(&mut self, message: String) {
        self.messages.push(message);
    }

    pub fn set_models(&mut self, models: Vec<String>) {
        self.models = models;
    }
    pub fn set_message_scroll(&mut self, message_scroll: usize) {
        self.message_scroll = message_scroll;
        self.message_scoll_state = self.message_scoll_state.position(message_scroll);
    }
    pub fn set_model_scroll(&mut self, model_scroll: usize) {
        self.model_scroll = model_scroll;
    }
}

