use crate::{
    app::{AppMode, AppState},
    models, ollama,
};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tokio::sync::mpsc;

pub enum AppEvent {
    Terminal(KeyEvent),
    OllamaChunk(Result<String, String>),
    OllamaDone,
    Models(Result<Vec<String>, String>),
}

pub async fn handle_key_event(key: KeyEvent, app: &mut AppState, tx: mpsc::Sender<AppEvent>) -> bool {
    if key.modifiers == KeyModifiers::CONTROL {
        match key.code {
            KeyCode::Char('c') => return true,
            KeyCode::Char('d') => {
                app.clear_current_session().ok();
                return false;
            }
            KeyCode::Char('n') => {
                app.new_session().ok();
                return false;
            }
            KeyCode::Char('l') => {
                app.mode = AppMode::ModelSelection;
                if !app.is_fetching_models {
                    app.is_fetching_models = true;
                    let models_tx = tx.clone();
                    let http_client_clone = app.http_client.clone();
                    let base_url_clone = app.ollama_base_url.clone();
                    let auth_config_clone = app.config.auth_method.clone();
                    let auth_enabled_clone = app.config.auth_enabled;
                    tokio::spawn(async move {
                        let result = ollama::fetch_models(
                            &http_client_clone,
                            &base_url_clone,
                            auth_enabled_clone,
                            auth_config_clone.as_ref(),
                        )
                        .await;
                        models_tx.send(AppEvent::Models(result)).await.ok();
                    });
                }
                return false;
            }
            _ => {}
        }
    }

    match app.mode {
        AppMode::Normal => match key.code {
            KeyCode::Char(c) => app.input.push(c),
            KeyCode::Backspace => {
                app.input.pop();
            }
            KeyCode::Enter => {
                if !app.input.is_empty() && !app.is_loading {
                    let user_input: String = app.input.drain(..).collect();
                    app.current_messages_mut().push(models::Message {
                        role: models::Role::User,
                        content: user_input,
                    });
                    app.current_messages_mut().push(models::Message {
                        role: models::Role::Assistant,
                        content: String::new(),
                    });

                    app.is_loading = true;
                    app.scroll_offset = 0;

                    let client = app.http_client.clone();
                    let model = app.current_model.clone();
                    let messages = app.current_messages().clone();
                    let base_url = app.ollama_base_url.clone();
                    let auth_config = app.config.auth_method.clone();
                    let auth_enabled = app.config.auth_enabled;

                    tokio::spawn(async move {
                        ollama::stream_chat_request(
                            &client,
                            &base_url,
                            &model,
                            &messages,
                            auth_enabled,
                            auth_config.as_ref(),
                            tx,
                        )
                        .await;
                    });
                }
            }
            KeyCode::Tab => app.mode = AppMode::SessionSelection,
            KeyCode::Up => app.scroll_offset = app.scroll_offset.saturating_add(1),
            KeyCode::Down => app.scroll_offset = app.scroll_offset.saturating_sub(1),
            _ => {}
        },
        AppMode::ModelSelection => match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                app.mode = AppMode::Normal;
            }
            KeyCode::Up => app.previous_model(),
            KeyCode::Down => app.next_model(),
            KeyCode::Enter => {
                app.confirm_model_selection().ok();
            }
            _ => {}
        },
        AppMode::SessionSelection => match key.code {
            KeyCode::Char('q') | KeyCode::Tab | KeyCode::Esc => app.mode = AppMode::Normal,
            KeyCode::Up => app.previous_session(),
            KeyCode::Down => app.next_session(),
            KeyCode::Enter => {
                app.switch_to_selected_session().ok();
            }
            _ => {}
        },
    }
    false
}

