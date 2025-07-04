use crate::{config, db, models};
use anyhow::{anyhow, Result};
use ratatui::widgets::ListState;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use reqwest::Client;
use rusqlite::Connection;
use textwrap::wrap;

#[derive(PartialEq, Eq)]
pub enum AppMode {
    Normal,
    ModelSelection,
    SessionSelection,
    Agent,  // New agent mode
}

pub struct AppState {
    pub mode: AppMode,
    pub sessions: Vec<models::ChatSession>,
    pub current_session_index: usize,
    pub session_list_state: ListState,
    pub input: String,
    pub current_model: String,
    pub available_models: Vec<String>,
    pub model_list_state: ListState,
    pub is_loading: bool,
    pub is_fetching_models: bool,
    pub scroll_offset: u16,
    pub target_scroll_offset: u16, // Target scroll position for smooth scrolling
    pub auto_scroll: bool,
    pub terminal_width: u16,
    pub terminal_height: u16,
    pub chat_list_state: ListState, // For chat message list view
    pub http_client: Client,
    pub db_conn: Connection,
    pub ollama_base_url: String,
    pub config: models::Config,
    // Agent mode fields
    pub agent_mode: bool,
    pub pending_commands: Vec<models::AgentCommand>,
    pub command_approval_index: Option<usize>,
    pub agent_context: String,
}

impl AppState {
    pub fn load(config: models::Config) -> Result<Self> {
        let db_path = config::get_config_path()?
            .parent()
            .ok_or_else(|| anyhow!("Config path has no parent directory"))?
            .join(&config.db_filename);
        let conn = db::get_connection(&db_path)?;
        let mut sessions = db::load_sessions(&conn)?;

        let last_model =
            db::load_config(&conn, "current_model")?.unwrap_or_else(|| "No model selected".to_string());
        let last_session_id: i64 = db::load_config(&conn, "current_session_id")?
            .and_then(|id_str| id_str.parse().ok())
            .unwrap_or(0);

        if sessions.is_empty() {
            let mut new_session = models::ChatSession::new(&conn)?;
            db::save_session(&conn, &mut new_session)?;
            sessions.push(new_session);
        }

        let current_session_index = sessions
            .iter()
            .position(|s| s.id == last_session_id)
            .unwrap_or(0);

        let mut session_list_state = ListState::default();
        session_list_state.select(Some(current_session_index));

        let ollama_base_url = format!("{}:{}", config.ollama_host, config.ollama_port);

        let mut headers = HeaderMap::new();
        if config.auth_enabled {
            if let Some(auth_method) = &config.auth_method {
                match auth_method {
                    models::AuthMethod::Bearer { token } => {
                        headers.insert(
                            AUTHORIZATION,
                            HeaderValue::from_str(&format!("Bearer {}", token))?,
                        );
                    }
                    _ => {}
                }
            }
        }

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()?;

        Ok(Self {
            mode: AppMode::Normal,
            sessions,
            current_session_index,
            session_list_state,
            input: String::new(),
            current_model: last_model,
            available_models: Vec::new(),
            model_list_state: ListState::default(),
            is_loading: false,
            is_fetching_models: false,
            scroll_offset: 0,
            target_scroll_offset: 0,
            auto_scroll: true, // Initialize auto-scroll to true
            terminal_width: 80, // Default values
            terminal_height: 24,
            chat_list_state: ListState::default(),
            http_client: client,
            db_conn: conn,
            ollama_base_url,
            config,
            // Initialize agent fields
            agent_mode: false,
            pending_commands: Vec::new(),
            command_approval_index: None,
            agent_context: String::new(),
        })
    }

    pub fn new_session(&mut self) -> Result<()> {
        let mut new_session = models::ChatSession::new(&self.db_conn)?;
        db::save_session(&self.db_conn, &mut new_session)?;
        self.sessions.push(new_session);
        self.current_session_index = self.sessions.len() - 1;
        self.session_list_state
            .select(Some(self.current_session_index));
        self.scroll_offset = 0;
        self.mode = AppMode::Normal;
        db::save_config(
            &self.db_conn,
            "current_session_id",
            &self.sessions[self.current_session_index].id.to_string(),
        )?;
        Ok(())
    }

    pub fn clear_current_session(&mut self) -> Result<()> {
        let session_id = self.current_session_id();
        db::clear_messages_for_session(&self.db_conn, session_id)?;
        let messages = self.current_messages_mut();
        messages.clear();
        messages.push(models::Message {
            role: models::Role::Assistant,
            content: "History Cleared.".to_string(),
        });
        self.chat_list_state = ListState::default(); // Reset chat list state
        Ok(())
    }

    pub fn delete_current_session(&mut self) -> Result<()> {
        if self.sessions.len() <= 1 {
            // Don't delete the last session, just clear it instead
            return self.clear_current_session();
        }

        let session_id = self.current_session_id();
        
        // Delete from database
        db::delete_session(&self.db_conn, session_id)?;
        
        // Remove from sessions list
        self.sessions.remove(self.current_session_index);
        
        // Adjust current session index
        if self.current_session_index >= self.sessions.len() {
            self.current_session_index = self.sessions.len() - 1;
        }
        
        // Update UI states
        self.session_list_state.select(Some(self.current_session_index));
        self.chat_list_state = ListState::default();
        
        // Save new current session
        db::save_config(
            &self.db_conn,
            "current_session_id",
            &self.sessions[self.current_session_index].id.to_string(),
        )?;
        
        Ok(())
    }

    pub fn next_session(&mut self) {
        let i = match self.session_list_state.selected() {
            Some(i) => if i >= self.sessions.len() - 1 { 0 } else { i + 1 },
            None => 0,
        };
        self.session_list_state.select(Some(i));
    }

    pub fn previous_session(&mut self) {
        let i = match self.session_list_state.selected() {
            Some(i) => if i == 0 { self.sessions.len() - 1 } else { i - 1 },
            None => 0,
        };
        self.session_list_state.select(Some(i));
    }

    pub fn switch_to_selected_session(&mut self) -> Result<()> {
        if let Some(selected) = self.session_list_state.selected() {
            self.current_session_index = selected;
            self.chat_list_state = ListState::default(); // Reset chat list state
            self.mode = AppMode::Normal;
            db::save_config(
                &self.db_conn,
                "current_session_id",
                &self.sessions[self.current_session_index].id.to_string(),
            )?;
        }
        Ok(())
    }

    pub fn current_messages_mut(&mut self) -> &mut Vec<models::Message> {
        &mut self.sessions[self.current_session_index].messages
    }

    pub fn current_messages(&self) -> &Vec<models::Message> {
        &self.sessions[self.current_session_index].messages
    }

    pub fn current_session_id(&self) -> i64 {
        self.sessions[self.current_session_index].id
    }

    pub fn next_model(&mut self) {
        let i = match self.model_list_state.selected() {
            Some(i) => if i >= self.available_models.len() - 1 { 0 } else { i + 1 },
            None => 0,
        };
        self.model_list_state.select(Some(i));
    }

    pub fn previous_model(&mut self) {
        let i = match self.model_list_state.selected() {
            Some(i) => if i == 0 { self.available_models.len() - 1 } else { i - 1 },
            None => 0,
        };
        self.model_list_state.select(Some(i));
    }

    pub fn confirm_model_selection(&mut self) -> Result<()> {
        if let Some(selected) = self.model_list_state.selected() {
            if let Some(model_name) = self.available_models.get(selected) {
                self.current_model = model_name.clone();
                db::save_config(&self.db_conn, "current_model", &self.current_model)?;
            }
        }
        self.mode = AppMode::Normal;
        Ok(())
    }

    pub fn update_terminal_dimensions(&mut self, width: u16, height: u16) {
        self.terminal_width = width;
        self.terminal_height = height;
    }

    pub fn trigger_auto_scroll(&mut self) {
        if self.auto_scroll {
            let chat_width = (self.terminal_width * 3) / 4;
            let chat_height = self.terminal_height.saturating_sub(6);
            self.auto_scroll_to_bottom(chat_height, chat_width);
        }
    }

    pub fn trigger_auto_scroll_aggressive(&mut self) {
        if self.auto_scroll {
            let chat_width = (self.terminal_width * 3) / 4;
            let total_lines = self.calculate_total_message_lines(chat_width);
            
            // During streaming, always scroll to the last item
            if total_lines > 0 {
                let last_index = total_lines.saturating_sub(1);
                self.chat_list_state.select(Some(last_index));
            }
        }
    }

    pub fn calculate_total_message_lines(&self, chat_width: u16) -> usize {
        let mut total_lines = 0;
        for message in self.current_messages() {
            // Use the same wrap width calculation as in render_messages
            let wrap_width = (chat_width as usize).saturating_sub(6);
            let wrapped_content = wrap(&message.content, wrap_width);
            
            // Each message gets at least 1 line (for the first line with prefix)
            total_lines += std::cmp::max(1, wrapped_content.len());
            
            // Add empty line after each message (if content is not empty)
            if !message.content.is_empty() {
                total_lines += 1;
            }
        }
        total_lines
    }

    pub fn auto_scroll_to_bottom(&mut self, _chat_height: u16, chat_width: u16) {
        if !self.auto_scroll {
            return;
        }
        let total_lines = self.calculate_total_message_lines(chat_width);
        
        // Auto-scroll to show the last lines
        if total_lines > 0 {
            let last_index = total_lines.saturating_sub(1);
            self.chat_list_state.select(Some(last_index));
        }
    }

}

