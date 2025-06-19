use crate::{config, db, models};
use anyhow::{anyhow, Result};
use ratatui::widgets::ListState;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use reqwest::Client;
use rusqlite::Connection;

#[derive(PartialEq, Eq)]
pub enum AppMode {
    Normal,
    ModelSelection,
    SessionSelection,
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
    pub http_client: Client,
    pub db_conn: Connection,
    pub ollama_base_url: String,
    pub config: models::Config,
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
            http_client: client,
            db_conn: conn,
            ollama_base_url,
            config,
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
        self.scroll_offset = 0;
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
            self.scroll_offset = 0;
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
}

