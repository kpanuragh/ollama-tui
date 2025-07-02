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
    AgentCommands(Vec<models::AgentCommand>),
    CommandExecuted(usize, Result<String, String>),
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
            KeyCode::Char('a') => {
                app.mode = AppMode::Agent;
                app.agent_mode = true;
                return false;
            }
            KeyCode::Char('s') => {
                // Toggle auto-scroll
                app.auto_scroll = !app.auto_scroll;
                if app.auto_scroll {
                    app.scroll_offset = 0; // Jump to bottom when enabling auto-scroll
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
                    app.auto_scroll = true;
                    app.auto_scroll_to_bottom();

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
            KeyCode::Up => {
                app.scroll_offset = app.scroll_offset.saturating_add(1);
                app.auto_scroll = false; // Disable auto-scroll when user manually scrolls
            },
            KeyCode::Down => {
                app.scroll_offset = app.scroll_offset.saturating_sub(1);
                // If user scrolls to bottom (offset 0), re-enable auto-scroll
                if app.scroll_offset == 0 {
                    app.auto_scroll = true;
                }
            },
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
        AppMode::Agent => match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                app.mode = AppMode::Normal;
                app.agent_mode = false;
                app.pending_commands.clear();
                app.command_approval_index = None;
            }
            KeyCode::Enter => {
                if !app.input.trim().is_empty() && !app.is_loading {
                    let input_content = app.input.clone();
                    
                    // Add user message with agent prompt
                    let user_message = if app.agent_mode {
                        let context = std::env::current_dir()
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_else(|_| "Unknown directory".to_string());
                        
                        crate::agent::Agent::create_agent_prompt(&input_content, &context)
                    } else {
                        input_content.clone()
                    };

                    app.current_messages_mut().push(models::Message {
                        role: models::Role::User,
                        content: input_content,
                    });
                    app.input.clear();

                    app.current_messages_mut().push(models::Message {
                        role: models::Role::Assistant,
                        content: String::new(),
                    });

                    app.is_loading = true;
                    app.auto_scroll = true;
                    app.auto_scroll_to_bottom();

                    let client = app.http_client.clone();
                    let model = app.current_model.clone();
                    let base_url = app.ollama_base_url.clone();
                    let auth_config = app.config.auth_method.clone();
                    let auth_enabled = app.config.auth_enabled;
                    let tx_clone = tx.clone();

                    // Create messages with agent prompt
                    let mut messages = app.current_messages().clone();
                    if let Some(last_user_msg) = messages.iter_mut().rev().find(|m| m.role == models::Role::User) {
                        last_user_msg.content = user_message;
                    }

                    tokio::spawn(async move {
                        ollama::stream_chat_request(
                            &client,
                            &base_url,
                            &model,
                            &messages,
                            auth_enabled,
                            auth_config.as_ref(),
                            tx_clone,
                        )
                        .await;
                    });
                }
            }
            KeyCode::Char('y') => {
                // Approve current pending command
                if let Some(index) = app.command_approval_index {
                    if let Some(cmd) = app.pending_commands.get_mut(index) {
                        if !cmd.executed {
                            cmd.approved = true;
                            let command = cmd.command.clone();
                            let tx_clone = tx.clone();
                            
                            tokio::spawn(async move {
                                let result = crate::agent::Agent::execute_command(&command).await;
                                tx_clone.send(AppEvent::CommandExecuted(index, result.map_err(|e| e.to_string()))).await.ok();
                            });
                            
                            // Move to next command
                            if index + 1 < app.pending_commands.len() {
                                app.command_approval_index = Some(index + 1);
                            } else {
                                app.command_approval_index = None;
                            }
                        }
                    }
                }
            }
            KeyCode::Char('n') => {
                // Reject current pending command and move to next
                if let Some(index) = app.command_approval_index {
                    if index + 1 < app.pending_commands.len() {
                        app.command_approval_index = Some(index + 1);
                    } else {
                        app.command_approval_index = None;
                    }
                }
            }
            KeyCode::Up => {
                app.scroll_offset = app.scroll_offset.saturating_add(1);
                app.auto_scroll = false; // Disable auto-scroll when user manually scrolls
            },
            KeyCode::Down => {
                app.scroll_offset = app.scroll_offset.saturating_sub(1);
                // If user scrolls to bottom (offset 0), re-enable auto-scroll
                if app.scroll_offset == 0 {
                    app.auto_scroll = true;
                }
            },
            KeyCode::Char(c) => app.input.push(c),
            KeyCode::Backspace => {
                app.input.pop();
            }
            _ => {}
        },
    }
    false
}

