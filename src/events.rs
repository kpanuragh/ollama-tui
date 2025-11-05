use crate::{
    app::{AppMode, AppState},
    models, ollama,
};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use tokio::sync::mpsc;

pub enum AppEvent {
    Terminal(KeyEvent),
    OllamaChunk(Result<String, String>),
    OllamaDone,
    Models(Result<Vec<String>, String>),
    #[allow(dead_code)]
    AgentCommands(Vec<models::AgentCommand>),
    #[allow(dead_code)]
    CommandExecuted(usize, Result<String, String>),
    // Autonomous agent events
    AutonomousReasoningComplete(Result<String, String>),  // JSON response from reasoning
    AutonomousCommandExecuted(Result<String, String>),     // Command execution result
    AutonomousAnalysisComplete(Result<String, String>),    // Analysis of command output
    Tick,
}

pub async fn handle_key_event(key: KeyEvent, app: &mut AppState, tx: mpsc::Sender<AppEvent>) -> bool {
    match app.mode {
        AppMode::Normal => handle_normal_mode(key, app, tx).await,
        AppMode::Insert => handle_insert_mode(key, app, tx).await,
        AppMode::Command => handle_command_mode(key, app).await,
        AppMode::Visual => handle_visual_mode(key, app).await,
        AppMode::ModelSelection => handle_model_selection_mode(key, app).await,
        AppMode::SessionSelection => handle_session_selection_mode(key, app).await,
        AppMode::Agent => handle_agent_mode(key, app, tx).await,
        AppMode::AgentApproval => handle_agent_approval_mode(key, app, tx).await,
        AppMode::Autonomous => handle_autonomous_mode(key, app, tx).await,
        AppMode::Help => handle_help_mode(key, app).await,
    }
}

async fn handle_normal_mode(key: KeyEvent, app: &mut AppState, _tx: mpsc::Sender<AppEvent>) -> bool {
    // Clear any status message on any key press
    app.clear_status_message();
    
    match key.code {
        KeyCode::Char('q') => return true, // Quick quit
        KeyCode::Char('i') => {
            app.mode = AppMode::Insert;
        }
        KeyCode::Char('o') => {
            app.mode = AppMode::Insert;
            app.input.clear();
        }
        KeyCode::Char('O') => {
            app.mode = AppMode::Insert;
            app.input.clear();
        }
        KeyCode::Char(':') => {
            app.mode = AppMode::Command;
            app.vim_command.clear();
        }
        KeyCode::Char('?') => {
            app.mode = AppMode::Help;
        }
        KeyCode::Char('v') => {
            app.start_visual_selection();
        }
        // Navigation in normal mode
        KeyCode::Char('j') | KeyCode::Down => {
            app.auto_scroll = false;
            let selected = app.chat_list_state.selected();
            let chat_width = (app.terminal_width * 3) / 4;
            let total_lines = app.calculate_total_message_lines(chat_width);
            
            if let Some(i) = selected {
                if i < total_lines.saturating_sub(1) {
                    app.chat_list_state.select(Some(i + 1));
                }
            } else if total_lines > 0 {
                app.chat_list_state.select(Some(0));
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.auto_scroll = false;
            let selected = app.chat_list_state.selected();
            if let Some(i) = selected {
                if i > 0 {
                    app.chat_list_state.select(Some(i - 1));
                }
            } else {
                let chat_width = (app.terminal_width * 3) / 4;
                let total_lines = app.calculate_total_message_lines(chat_width);
                if total_lines > 0 {
                    app.chat_list_state.select(Some(total_lines - 1));
                }
            }
        }
        KeyCode::Char('g') => {
            // Go to top
            app.chat_list_state.select(Some(0));
        }
        KeyCode::Char('G') => {
            // Go to bottom
            let chat_width = (app.terminal_width * 3) / 4;
            let total_lines = app.calculate_total_message_lines(chat_width);
            if total_lines > 0 {
                app.chat_list_state.select(Some(total_lines - 1));
            }
        }
        KeyCode::PageUp => {
            app.auto_scroll = false;
            let selected = app.chat_list_state.selected();
            let chat_height = app.terminal_height.saturating_sub(6);
            let page_size = chat_height as usize;
            
            if let Some(i) = selected {
                let new_index = i.saturating_sub(page_size);
                app.chat_list_state.select(Some(new_index));
            }
        }
        KeyCode::PageDown => {
            app.auto_scroll = false;
            let selected = app.chat_list_state.selected();
            let chat_height = app.terminal_height.saturating_sub(6);
            let page_size = chat_height as usize;
            let chat_width = (app.terminal_width * 3) / 4;
            let total_lines = app.calculate_total_message_lines(chat_width);
            
            if let Some(i) = selected {
                let new_index = std::cmp::min(i + page_size, total_lines.saturating_sub(1));
                app.chat_list_state.select(Some(new_index));
            }
        }
        _ => {}
    }
    false
}

async fn handle_insert_mode(key: KeyEvent, app: &mut AppState, tx: mpsc::Sender<AppEvent>) -> bool {
    match key.code {
        KeyCode::Esc => {
            app.mode = AppMode::Normal;
        }
        KeyCode::Char(c) => {
            app.input.push(c);
        }
        KeyCode::Backspace => {
            app.input.pop();
        }
        KeyCode::Enter => {
            if !app.input.is_empty() && !app.is_loading {
                let user_input: String = app.input.drain(..).collect();
                app.current_messages_mut().push(models::Message {
                    role: models::Role::User,
                    content: user_input,
                    timestamp: chrono::Utc::now(),
                });
                app.current_messages_mut().push(models::Message {
                    role: models::Role::Assistant,
                    content: String::new(),
                    timestamp: chrono::Utc::now(),
                });

                app.is_loading = true;
                app.auto_scroll = true;
                app.trigger_auto_scroll();

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
                        None, // No system prompt for normal chat
                        tx,
                    )
                    .await;
                });
            }
        }
        _ => {}
    }
    false
}

async fn handle_command_mode(key: KeyEvent, app: &mut AppState) -> bool {
    match key.code {
        KeyCode::Esc => {
            app.mode = AppMode::Normal;
            app.vim_command.clear();
        }
        KeyCode::Char(c) => {
            app.vim_command.push(c);
        }
        KeyCode::Backspace => {
            app.vim_command.pop();
        }
        KeyCode::Enter => {
            let command = app.vim_command.clone();
            app.vim_command.clear();
            
            if command == "q" || command == "wq" {
                return true; // Signal to quit
            }
            
            app.execute_vim_command(&command).ok();
            
            // Don't automatically return to Normal mode if we're entering a special mode
            if app.mode == AppMode::SessionSelection || app.mode == AppMode::ModelSelection || app.mode == AppMode::Help || app.mode == AppMode::Agent {
                // Stay in the current mode
            } else {
                app.mode = AppMode::Normal;
            }
        }
        _ => {}
    }
    false
}

async fn handle_model_selection_mode(key: KeyEvent, app: &mut AppState) -> bool {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => {
            app.mode = AppMode::Normal;
        }
        KeyCode::Up | KeyCode::Char('k') => app.previous_model(),
        KeyCode::Down | KeyCode::Char('j') => app.next_model(),
        KeyCode::Enter => {
            app.confirm_model_selection().ok();
            app.mode = AppMode::Normal;
        }
        _ => {}
    }
    false
}

async fn handle_session_selection_mode(key: KeyEvent, app: &mut AppState) -> bool {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => {
            app.mode = AppMode::Normal;
        }
        KeyCode::Up | KeyCode::Char('k') => app.previous_session(),
        KeyCode::Down | KeyCode::Char('j') => app.next_session(),
        KeyCode::Enter => {
            app.switch_to_selected_session().ok();
            app.mode = AppMode::Normal;
        }
        KeyCode::Delete | KeyCode::Char('d') => {
            app.delete_current_session().ok();
        }
        _ => {}
    }
    false
}

async fn handle_agent_mode(key: KeyEvent, app: &mut AppState, tx: mpsc::Sender<AppEvent>) -> bool {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => {
            app.mode = AppMode::Normal;
            app.agent_mode = false;
            app.pending_commands.clear();
            app.command_approval_index = None;
        }
        KeyCode::Enter => {
            if !app.input.trim().is_empty() && !app.is_loading {
                let input_content = app.input.clone();
                
                let _user_message = if app.agent_mode {
                    let _context = std::env::current_dir()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_else(|_| "Unknown directory".to_string());
                    
                    format!("Agent mode: {}", input_content)
                } else {
                    input_content.clone()
                };

                app.current_messages_mut().push(models::Message {
                    role: models::Role::User,
                    content: input_content,
                    timestamp: chrono::Utc::now(),
                });
                app.input.clear();

                app.current_messages_mut().push(models::Message {
                    role: models::Role::Assistant,
                    content: String::new(),
                    timestamp: chrono::Utc::now(),
                });

                app.is_loading = true;
                app.auto_scroll = true;
                app.trigger_auto_scroll();

                let client = app.http_client.clone();
                let model = app.current_model.clone();
                let messages = app.current_messages().clone();
                let base_url = app.ollama_base_url.clone();
                let auth_config = app.config.auth_method.clone();
                let auth_enabled = app.config.auth_enabled;
                let system_prompt = app.agent_system_prompt.clone();

                tokio::spawn(async move {
                    ollama::stream_chat_request(
                        &client,
                        &base_url,
                        &model,
                        &messages,
                        auth_enabled,
                        auth_config.as_ref(),
                        Some(&system_prompt), // Use agent system prompt
                        tx,
                    )
                    .await;
                });
            }
        }
        KeyCode::Char(c) => app.input.push(c),
        KeyCode::Backspace => {
            app.input.pop();
        }
        _ => {}
    }
    false
}

async fn handle_help_mode(key: KeyEvent, app: &mut AppState) -> bool {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc | KeyCode::Char('?') => {
            app.mode = AppMode::Normal;
        }
        _ => {}
    }
    false
}

async fn handle_visual_mode(key: KeyEvent, app: &mut AppState) -> bool {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            app.clear_visual_selection();
        }
        KeyCode::Char('y') => {
            // Copy selection to clipboard
            match app.copy_selection_to_clipboard() {
                Ok(_) => {
                    app.set_status_message("Copied to clipboard".to_string());
                }
                Err(e) => {
                    app.set_status_message(format!("Copy failed: {}", e));
                }
            }
            app.clear_visual_selection();
        }
        KeyCode::Char('j') | KeyCode::Down => {
            let selected = app.chat_list_state.selected();
            let chat_width = (app.terminal_width * 3) / 4;
            let total_lines = app.calculate_total_message_lines(chat_width);
            
            if let Some(i) = selected {
                if i < total_lines.saturating_sub(1) {
                    let new_pos = i + 1;
                    app.chat_list_state.select(Some(new_pos));
                    app.update_visual_selection(new_pos);
                }
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            let selected = app.chat_list_state.selected();
            if let Some(i) = selected {
                if i > 0 {
                    let new_pos = i - 1;
                    app.chat_list_state.select(Some(new_pos));
                    app.update_visual_selection(new_pos);
                }
            }
        }
        KeyCode::Char('g') => {
            // Go to top
            app.chat_list_state.select(Some(0));
            app.update_visual_selection(0);
        }
        KeyCode::Char('G') => {
            // Go to bottom
            let chat_width = (app.terminal_width * 3) / 4;
            let total_lines = app.calculate_total_message_lines(chat_width);
            if total_lines > 0 {
                let bottom = total_lines - 1;
                app.chat_list_state.select(Some(bottom));
                app.update_visual_selection(bottom);
            }
        }
        KeyCode::PageUp => {
            let selected = app.chat_list_state.selected();
            let chat_height = app.terminal_height.saturating_sub(6);
            let page_size = chat_height as usize;
            
            if let Some(i) = selected {
                let new_index = i.saturating_sub(page_size);
                app.chat_list_state.select(Some(new_index));
                app.update_visual_selection(new_index);
            }
        }
        KeyCode::PageDown => {
            let selected = app.chat_list_state.selected();
            let chat_height = app.terminal_height.saturating_sub(6);
            let page_size = chat_height as usize;
            let chat_width = (app.terminal_width * 3) / 4;
            let total_lines = app.calculate_total_message_lines(chat_width);
            
            if let Some(i) = selected {
                let new_index = std::cmp::min(i + page_size, total_lines.saturating_sub(1));
                app.chat_list_state.select(Some(new_index));
                app.update_visual_selection(new_index);
            }
        }
        _ => {}
    }
    false
}


async fn handle_agent_approval_mode(key: KeyEvent, app: &mut AppState, tx: mpsc::Sender<AppEvent>) -> bool {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => {
            // Cancel and return to agent mode
            app.mode = AppMode::Agent;
            app.pending_commands.clear();
            app.command_approval_index = None;
        }
        KeyCode::Char('j') | KeyCode::Down => {
            // Move to next command
            if let Some(current) = app.command_approval_index {
                if current < app.pending_commands.len().saturating_sub(1) {
                    app.command_approval_index = Some(current + 1);
                }
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            // Move to previous command
            if let Some(current) = app.command_approval_index {
                if current > 0 {
                    app.command_approval_index = Some(current - 1);
                }
            }
        }
        KeyCode::Char('y') => {
            // Approve current command
            if let Some(index) = app.command_approval_index {
                if let Some(cmd) = app.pending_commands.get_mut(index) {
                    cmd.approved = true;
                }
            }
        }
        KeyCode::Char('n') => {
            // Reject current command
            if let Some(index) = app.command_approval_index {
                if let Some(cmd) = app.pending_commands.get_mut(index) {
                    cmd.approved = false;
                }
            }
        }
        KeyCode::Char('a') => {
            // Approve all commands
            for cmd in &mut app.pending_commands {
                cmd.approved = true;
            }
        }
        KeyCode::Char('r') => {
            // Reject all commands
            for cmd in &mut app.pending_commands {
                cmd.approved = false;
            }
        }
        KeyCode::Enter | KeyCode::Char('x') => {
            // Execute approved commands
            let approved_commands: Vec<_> = app.pending_commands
                .iter()
                .enumerate()
                .filter(|(_, cmd)| cmd.approved && !cmd.executed)
                .map(|(i, _)| i)
                .collect();

            if approved_commands.is_empty() {
                app.set_status_message("No commands approved for execution".to_string());
                app.mode = AppMode::Agent;
                app.pending_commands.clear();
                app.command_approval_index = None;
            } else {
                // Execute each approved command
                for index in approved_commands {
                    if let Some(cmd) = app.pending_commands.get(index) {
                        let command = cmd.command.clone();
                        let tx_clone = tx.clone();

                        tokio::spawn(async move {
                            use crate::agent::Agent;
                            let result = Agent::execute_command(&command).await;
                            tx_clone.send(AppEvent::CommandExecuted(index, result)).await.ok();
                        });
                    }
                }

                app.mode = AppMode::Agent;
                app.pending_commands.clear();
                app.command_approval_index = None;
            }
        }
        _ => {}
    }
    false
}
async fn handle_autonomous_mode(key: KeyEvent, app: &mut AppState, tx: mpsc::Sender<AppEvent>) -> bool {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => {
            // Exit autonomous mode
            app.mode = AppMode::Normal;
            app.autonomous_agent = None;
            app.current_messages_mut().push(models::Message {
                role: models::Role::Assistant,
                content: "🛑 Autonomous mode stopped.".to_string(),
                timestamp: chrono::Utc::now(),
            });
        }
        KeyCode::Enter => {
            if !app.input.trim().is_empty() && !app.is_loading {
                let user_goal = app.input.clone();
                app.input.clear();

                // Set the goal in the autonomous agent
                if let Some(ref mut agent) = app.autonomous_agent {
                    agent.set_goal(user_goal.clone());

                    // Add user message
                    app.current_messages_mut().push(models::Message {
                        role: models::Role::User,
                        content: user_goal,
                        timestamp: chrono::Utc::now(),
                    });

                    // Add status message
                    app.current_messages_mut().push(models::Message {
                        role: models::Role::Assistant,
                        content: "🔍 Analyzing goal and planning first step...".to_string(),
                        timestamp: chrono::Utc::now(),
                    });

                    app.is_loading = true;
                    app.auto_scroll = true;

                    // Start the reasoning loop - ask AI what to do
                    let reasoning_prompt = agent.create_reasoning_prompt();
                    let client = app.http_client.clone();
                    let model = app.current_model.clone();
                    let base_url = app.ollama_base_url.clone();
                    let auth_config = app.config.auth_method.clone();
                    let auth_enabled = app.config.auth_enabled;

                    tokio::spawn(async move {
                        // Simple request without streaming for JSON responses
                        let messages = vec![models::Message {
                            role: models::Role::User,
                            content: reasoning_prompt,
                            timestamp: chrono::Utc::now(),
                        }];

                        // Collect the full response
                        let (temp_tx, mut temp_rx) = mpsc::channel::<AppEvent>(32);

                        let client_clone = client.clone();
                        tokio::spawn(async move {
                            ollama::stream_chat_request(
                                &client_clone,
                                &base_url,
                                &model,
                                &messages,
                                auth_enabled,
                                auth_config.as_ref(),
                                None,  // No system prompt, it's in the message
                                temp_tx,
                            )
                            .await;
                        });

                        // Collect full response
                        let mut full_response = String::new();
                        while let Some(event) = temp_rx.recv().await {
                            match event {
                                AppEvent::OllamaChunk(Ok(chunk)) => {
                                    full_response.push_str(&chunk);
                                }
                                AppEvent::OllamaDone => {
                                    break;
                                }
                                _ => {}
                            }
                        }

                        // Send the reasoning result
                        tx.send(AppEvent::AutonomousReasoningComplete(Ok(full_response))).await.ok();
                    });
                }
            }
        }
        KeyCode::Backspace => {
            app.input.pop();
        }
        KeyCode::Char(c) => {
            app.input.push(c);
        }
        _ => {}
    }
    false
}
