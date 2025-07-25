use crate::{agent, config, db, models};
use anyhow::{anyhow, Result};
use ratatui::widgets::ListState;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use reqwest::Client;
use rusqlite::Connection;
use textwrap::wrap;

#[derive(Debug, PartialEq, Eq)]
pub enum AppMode {
    Normal,         // Vim normal mode
    Insert,         // Vim insert mode for typing messages
    Command,        // Vim command mode
    Visual,         // Vim visual mode for text selection
    ModelSelection,
    SessionSelection,
    Agent,          // New agent mode
    Help,           // Help popup mode
}

pub struct AppState {
    pub mode: AppMode,
    pub vim_command: String,        // Command being typed in command mode
    pub visual_start: Option<usize>, // Start line of visual selection
    pub visual_end: Option<usize>,   // End line of visual selection
    pub status_message: Option<String>, // Temporary status message
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
    #[allow(dead_code)]
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
    pub agent: agent::Agent,
    pub pending_commands: Vec<models::AgentCommand>,
    pub command_approval_index: Option<usize>,
    pub pending_tool_calls: Vec<(String, std::collections::HashMap<String, serde_json::Value>)>,
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
            vim_command: String::new(),
            visual_start: None,
            visual_end: None,
            status_message: None, // Initialize status message
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
            agent: agent::Agent::new()?,
            pending_commands: Vec::new(),
            command_approval_index: None,
            pending_tool_calls: Vec::new(),
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

    #[allow(dead_code)]
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

    pub fn execute_vim_command(&mut self, command: &str) -> Result<()> {
        match command {
            "q" => {
                // This will be handled in the main loop to exit
                return Ok(());
            }
            "w" => {
                // Save current session
                if let Some(session) = self.sessions.get_mut(self.current_session_index) {
                    db::save_session(&self.db_conn, session)?;
                }
            }
            "wq" => {
                // Save and quit
                if let Some(session) = self.sessions.get_mut(self.current_session_index) {
                    db::save_session(&self.db_conn, session)?;
                }
                // Quit signal will be handled in main loop
            }
            "n" => {
                self.new_session()?;
            }
            "c" => {
                self.clear_current_session()?;
            }
            "m" => {
                self.mode = AppMode::ModelSelection;
                self.is_fetching_models = true;
                // The models will be fetched in the main loop
            }
            "s" => {
                self.mode = AppMode::SessionSelection;
                // Ensure the session list state is properly selected
                self.session_list_state.select(Some(self.current_session_index));
            }
            "a" => {
                self.enter_agent_mode();
            }
            "h" | "?" => {
                self.mode = AppMode::Help;
            }
            cmd if cmd.starts_with("d") => {
                // Delete session command
                if cmd == "d" {
                    self.delete_current_session()?;
                } else if let Some(session_num) = cmd.strip_prefix("d") {
                    if let Ok(index) = session_num.parse::<usize>() {
                        if index > 0 && index <= self.sessions.len() {
                            self.current_session_index = index - 1;
                            self.delete_current_session()?;
                        }
                    }
                }
            }
            cmd if cmd.starts_with("b") => {
                // Switch to buffer/session command
                if let Some(session_num) = cmd.strip_prefix("b") {
                    if let Ok(index) = session_num.parse::<usize>() {
                        if index > 0 && index <= self.sessions.len() {
                            self.current_session_index = index - 1;
                            self.session_list_state.select(Some(self.current_session_index));
                            self.chat_list_state = ListState::default();
                            db::save_config(
                                &self.db_conn,
                                "current_session_id",
                                &self.sessions[self.current_session_index].id.to_string(),
                            )?;
                        }
                    }
                }
            }
            _ => {
                // Unknown command, ignore
            }
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub fn should_fetch_models(&self) -> bool {
        self.mode == AppMode::ModelSelection && self.is_fetching_models && !self.available_models.is_empty() == false
    }

    pub fn start_visual_selection(&mut self) {
        if let Some(selected) = self.chat_list_state.selected() {
            self.visual_start = Some(selected);
            self.visual_end = Some(selected);
            self.mode = AppMode::Visual;
        }
    }

    pub fn update_visual_selection(&mut self, line: usize) {
        if self.visual_start.is_some() {
            self.visual_end = Some(line);
        }
    }

    pub fn get_selected_text(&self) -> String {
        if let (Some(start), Some(end)) = (self.visual_start, self.visual_end) {
            let start_line = std::cmp::min(start, end);
            let end_line = std::cmp::max(start, end);
            
            let chat_width = (self.terminal_width * 3) / 4;
            let mut selected_text = String::new();
            let mut line_index = 0;
            
            for message in self.current_messages() {
                let prefix = match message.role {
                    models::Role::User => "You: ",
                    models::Role::Assistant => "AI: ",
                };
                
                let wrapped_content = wrap(&message.content, (chat_width as usize).saturating_sub(6));
                
                for (i, line_content) in wrapped_content.iter().enumerate() {
                    if line_index >= start_line && line_index <= end_line {
                        if i == 0 {
                            selected_text.push_str(&format!("{}{}", prefix, line_content));
                        } else {
                            selected_text.push_str(&format!("     {}", line_content));
                        }
                        selected_text.push('\n');
                    }
                    line_index += 1;
                }
                
                // Add empty line after each message if content is not empty
                if !message.content.is_empty() {
                    if line_index >= start_line && line_index <= end_line {
                        selected_text.push('\n');
                    }
                    line_index += 1;
                }
            }
            
            selected_text.trim().to_string()
        } else {
            String::new()
        }
    }

    pub fn copy_selection_to_clipboard(&self) -> Result<()> {
        let selected_text = self.get_selected_text();
        if selected_text.is_empty() {
            return Ok(());
        }

        // Try to copy to clipboard using external commands
        #[cfg(target_os = "linux")]
        {
            use std::process::{Command, Stdio};
            use std::io::Write;
            
            // Try xclip first
            let xclip_result = Command::new("xclip")
                .arg("-selection")
                .arg("clipboard")
                .stdin(Stdio::piped())
                .spawn();
            
            if let Ok(mut child) = xclip_result {
                if let Some(stdin) = child.stdin.as_mut() {
                    let _ = stdin.write_all(selected_text.as_bytes());
                    let _ = stdin.flush();
                }
                drop(child.stdin.take()); // Close stdin
                let _ = child.wait();
                return Ok(());
            }
            
            // Fallback to xsel
            let xsel_result = Command::new("xsel")
                .arg("--clipboard")
                .arg("--input")
                .stdin(Stdio::piped())
                .spawn();
            
            if let Ok(mut child) = xsel_result {
                if let Some(stdin) = child.stdin.as_mut() {
                    let _ = stdin.write_all(selected_text.as_bytes());
                    let _ = stdin.flush();
                }
                drop(child.stdin.take()); // Close stdin
                let _ = child.wait();
                return Ok(());
            }
            
            // Last resort: try wl-copy for Wayland
            let wl_copy_result = Command::new("wl-copy")
                .stdin(Stdio::piped())
                .spawn();
            
            if let Ok(mut child) = wl_copy_result {
                if let Some(stdin) = child.stdin.as_mut() {
                    let _ = stdin.write_all(selected_text.as_bytes());
                    let _ = stdin.flush();
                }
                drop(child.stdin.take()); // Close stdin
                let _ = child.wait();
                return Ok(());
            }
            
            return Err(anyhow!("No clipboard utility found (tried xclip, xsel, wl-copy)"));
        }
        
        #[cfg(target_os = "macos")]
        {
            use std::process::{Command, Stdio};
            use std::io::Write;
            
            let mut child = Command::new("pbcopy")
                .stdin(Stdio::piped())
                .spawn()?;
            
            if let Some(stdin) = child.stdin.as_mut() {
                stdin.write_all(selected_text.as_bytes())?;
                stdin.flush()?;
            }
            drop(child.stdin.take()); // Close stdin
            child.wait()?;
            return Ok(());
        }
        
        #[cfg(target_os = "windows")]
        {
            use std::process::{Command, Stdio};
            use std::io::Write;
            
            let mut child = Command::new("clip")
                .stdin(Stdio::piped())
                .spawn()?;
            
            if let Some(stdin) = child.stdin.as_mut() {
                stdin.write_all(selected_text.as_bytes())?;
                stdin.flush()?;
            }
            drop(child.stdin.take()); // Close stdin
            child.wait()?;
            return Ok(());
        }
        
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            return Err(anyhow!("Clipboard not supported on this platform"));
        }
    }

    pub fn clear_visual_selection(&mut self) {
        self.visual_start = None;
        self.visual_end = None;
        self.mode = AppMode::Normal;
    }

    pub fn set_status_message(&mut self, message: String) {
        self.status_message = Some(message);
    }

    pub fn clear_status_message(&mut self) {
        self.status_message = None;
    }

    // Agent mode methods
    pub fn enter_agent_mode(&mut self) {
        self.mode = AppMode::Agent;
        self.agent_mode = true;
        self.set_status_message("Agent mode activated - AI has access to system tools".to_string());
    }

    pub fn exit_agent_mode(&mut self) {
        self.mode = AppMode::Normal;
        self.agent_mode = false;
        self.pending_tool_calls.clear();
        self.command_approval_index = None;
        self.set_status_message("Agent mode deactivated".to_string());
    }

    pub fn has_pending_tool_calls(&self) -> bool {
        !self.pending_tool_calls.is_empty()
    }

    pub fn get_current_tool_call(&self) -> Option<&(String, std::collections::HashMap<String, serde_json::Value>)> {
        self.command_approval_index
            .and_then(|index| self.pending_tool_calls.get(index))
    }

    pub fn approve_current_tool_call(&mut self) -> Option<(String, std::collections::HashMap<String, serde_json::Value>)> {
        if let Some(index) = self.command_approval_index {
            if index < self.pending_tool_calls.len() {
                let tool_call = self.pending_tool_calls.remove(index);
                self.command_approval_index = if self.pending_tool_calls.is_empty() {
                    None
                } else if index >= self.pending_tool_calls.len() {
                    Some(self.pending_tool_calls.len() - 1)
                } else {
                    Some(index)
                };
                return Some(tool_call);
            }
        }
        None
    }

    pub fn reject_current_tool_call(&mut self) {
        if let Some(index) = self.command_approval_index {
            if index < self.pending_tool_calls.len() {
                self.pending_tool_calls.remove(index);
                self.command_approval_index = if self.pending_tool_calls.is_empty() {
                    None
                } else if index >= self.pending_tool_calls.len() {
                    Some(self.pending_tool_calls.len() - 1)
                } else {
                    Some(index)
                };
            }
        }
    }

    pub fn add_tool_calls(&mut self, tool_calls: Vec<(String, std::collections::HashMap<String, serde_json::Value>)>) {
        for tool_call in tool_calls {
            self.pending_tool_calls.push(tool_call);
        }
        
        // Set approval index to first pending call if none set
        if self.command_approval_index.is_none() && !self.pending_tool_calls.is_empty() {
            self.command_approval_index = Some(0);
        }
    }

    pub async fn execute_tool_call(&self, tool_name: &str, args: &std::collections::HashMap<String, serde_json::Value>) -> Result<agent::ToolResult> {
        self.agent.execute_tool(tool_name, args).await
    }

    pub fn tool_requires_approval(&self, tool_name: &str) -> bool {
        self.agent.tool_requires_approval(tool_name)
    }

    pub fn create_agent_prompt(&self, user_input: &str) -> String {
        self.agent.create_agent_prompt(user_input)
    }

    pub fn parse_agent_response(&self, response: &str) -> Vec<(String, std::collections::HashMap<String, serde_json::Value>)> {
        agent::Agent::parse_tool_calls(response)
    }
}