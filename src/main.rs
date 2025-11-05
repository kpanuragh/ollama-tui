mod agent;
mod app;
mod autonomous_agent;
mod config;
mod db;
mod events;
mod models;
mod ollama;
mod ui;

use anyhow::Result;
use app::AppState;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{prelude::*};
use std::io::{self, Stdout};
use std::time::Duration;
use tokio::sync::mpsc;

fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    let mut stdout = io::stdout();
    enable_raw_mode()?;
    execute!(stdout, EnterAlternateScreen)?;
    Ok(Terminal::new(CrosstermBackend::new(stdout))?)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut terminal = setup_terminal()?;
    let config = config::load_or_create()?;
    let mut app_state = AppState::load(config)?;

    let (tx, mut rx) = mpsc::channel(32);

    // Terminal event handler task
    let event_tx = tx.clone();
    tokio::spawn(async move {
        loop {
            // Poll for events with a timeout, allowing for UI updates even without input
            if crossterm::event::poll(Duration::from_millis(50)).unwrap_or(false) {
                if let crossterm::event::Event::Key(key) = crossterm::event::read().unwrap() {
                    if event_tx.send(events::AppEvent::Terminal(key)).await.is_err() {
                        break;
                    }
                }
            } else {
                // Send a tick event to allow for UI updates (e.g., smooth scrolling)
                if event_tx.send(events::AppEvent::Tick).await.is_err() {
                    break;
                }
            }
        }
    });

    // Initial model fetch task
    let models_tx = tx.clone();
    let http_client_clone = app_state.http_client.clone();
    let base_url_clone = app_state.ollama_base_url.clone();
    let auth_config_clone = app_state.config.auth_method.clone();
    let auth_enabled_clone = app_state.config.auth_enabled;
    app_state.is_fetching_models = true; // Set state before spawning the task

    tokio::spawn(async move {
        let result = ollama::fetch_models(
            &http_client_clone,
            &base_url_clone,
            auth_enabled_clone,
            auth_config_clone.as_ref(),
        )
        .await;
        models_tx.send(events::AppEvent::Models(result)).await.ok();
    });

    // Main application loop
    loop {
        // Update terminal dimensions before drawing
        let terminal_area = terminal.size()?;
        app_state.update_terminal_dimensions(terminal_area.width, terminal_area.height);
        
        terminal.draw(|f| ui::ui(f, &mut app_state))?;

        match rx.recv().await {
            Some(events::AppEvent::Terminal(key)) => {
                if key.kind == crossterm::event::KeyEventKind::Press {
                    if events::handle_key_event(key, &mut app_state, tx.clone()).await {
                        break;
                    }
                    
                    // Check if we need to fetch models after handling the key event
                    if app_state.mode == app::AppMode::ModelSelection && app_state.is_fetching_models {
                        let models_tx = tx.clone();
                        let http_client_clone = app_state.http_client.clone();
                        let base_url_clone = app_state.ollama_base_url.clone();
                        let auth_config_clone = app_state.config.auth_method.clone();
                        let auth_enabled_clone = app_state.config.auth_enabled;
                        
                        tokio::spawn(async move {
                            let result = ollama::fetch_models(
                                &http_client_clone,
                                &base_url_clone,
                                auth_enabled_clone,
                                auth_config_clone.as_ref(),
                            )
                            .await;
                            models_tx.send(events::AppEvent::Models(result)).await.ok();
                        });
                    }
                }
            }
            Some(events::AppEvent::Tick) => {
                // Handle auto-scroll for list view
                let terminal_area = terminal.size()?;
                app_state.update_terminal_dimensions(terminal_area.width, terminal_area.height);
                
                // Calculate chat area dimensions to match UI layout exactly:
                // 75% width for left side (this matches the UI layout)
                let chat_width = (terminal_area.width * 3) / 4;
                // Total height minus input (3 lines) and status (1 line) minus borders (2 lines)
                let chat_height = terminal_area.height.saturating_sub(6);
                
                // Only do auto-scroll during tick if we're not actively loading
                if app_state.auto_scroll && !app_state.is_loading {
                    app_state.auto_scroll_to_bottom(chat_height, chat_width);
                }
            }
            Some(events::AppEvent::OllamaChunk(Ok(chunk))) => {
                if let Some(last_message) = app_state.current_messages_mut().last_mut() {
                    if last_message.role == models::Role::Assistant {
                        last_message.content.push_str(&chunk);
                        // Enable auto-scroll but don't trigger it on every chunk
                        app_state.auto_scroll = true;
                    }
                }
            }
            Some(events::AppEvent::OllamaChunk(Err(e))) => {
                if let Some(last_message) = app_state.current_messages_mut().last_mut() {
                    if last_message.role == models::Role::Assistant {
                        let err_msg = format!("\n[STREAM ERROR: {}]", e);
                        last_message.content.push_str(&err_msg);
                        // Enable auto-scroll when error content arrives and trigger immediately
                        app_state.auto_scroll = true;
                        app_state.trigger_auto_scroll();
                    }
                }
                app_state.is_loading = false;
            }
            Some(events::AppEvent::OllamaDone) => {
                app_state.is_loading = false;
                
                // Trigger auto-scroll when streaming is complete
                if app_state.auto_scroll {
                    app_state.trigger_auto_scroll();
                }

                // Parse commands if in agent mode
                if app_state.agent_mode {
                    if let Some(last_message) = app_state.current_messages().last() {
                        if last_message.role == models::Role::Assistant {
                            let commands = agent::Agent::parse_commands_from_response(&last_message.content);
                            if !commands.is_empty() {
                                tx.send(events::AppEvent::AgentCommands(commands)).await.ok();
                            }
                        }
                    }
                }

                let messages = app_state.current_messages();
                if messages.len() >= 2 {
                    let user_msg = &messages[messages.len() - 2];
                    let assistant_msg = &messages[messages.len() - 1];
                    db::save_message(&app_state.db_conn, app_state.current_session_id(), user_msg)
                        .ok();
                    db::save_message(
                        &app_state.db_conn,
                        app_state.current_session_id(),
                        assistant_msg,
                    )
                    .ok();
                }
            }
            Some(events::AppEvent::Models(Ok(models))) => {
                app_state.is_fetching_models = false;
                app_state.available_models = models;
                if !app_state.available_models.is_empty()
                    && app_state.current_model == "No model selected"
                {
                    app_state.current_model = app_state.available_models[0].clone();
                    app_state.model_list_state.select(Some(0));
                }
            }
            Some(events::AppEvent::Models(Err(e))) => {
                app_state.is_fetching_models = false;
                app_state.available_models.clear(); // Clear any stale models
                app_state.current_messages_mut().push(models::Message {
                    role: models::Role::Assistant,
                    content: format!("Error fetching models: {}. Is Ollama running?", e),
                    timestamp: chrono::Utc::now(),
                });
            }
            Some(events::AppEvent::AgentCommands(commands)) => {
                app_state.pending_commands = commands;
                if !app_state.pending_commands.is_empty() {
                    app_state.command_approval_index = Some(0);
                    app_state.mode = app::AppMode::AgentApproval;
                }
            }
            Some(events::AppEvent::CommandExecuted(index, result)) => {
                if let Some(cmd) = app_state.pending_commands.get_mut(index) {
                    cmd.executed = true;
                    let cmd_command = cmd.command.clone();
                    match result {
                        Ok(output) => {
                            cmd.output = Some(output.clone());
                            // Add command output to chat
                            app_state.current_messages_mut().push(models::Message {
                                role: models::Role::Assistant,
                                content: format!("Command executed successfully:\n```\n{}\n```\n\nOutput:\n```\n{}\n```", cmd_command, output),
                                timestamp: chrono::Utc::now(),
                            });
                        }
                        Err(error) => {
                            cmd.error = Some(error.clone());
                            app_state.current_messages_mut().push(models::Message {
                                role: models::Role::Assistant,
                                content: format!("Command failed:\n```\n{}\n```\n\nError:\n```\n{}\n```", cmd_command, error),
                                timestamp: chrono::Utc::now(),
                            });
                        }
                    }
                }
            }
            Some(events::AppEvent::AutonomousReasoningComplete(result)) => {
                app_state.is_loading = false;

                match result {
                    Ok(json_response) => {
                        // Parse the JSON response from the AI
                        use serde_json::Value;

                        // Try to extract JSON from response
                        let json_str = if let Some(start) = json_response.find('{') {
                            if let Some(end) = json_response.rfind('}') {
                                &json_response[start..=end]
                            } else {
                                &json_response
                            }
                        } else {
                            &json_response
                        };

                        match serde_json::from_str::<Value>(json_str) {
                            Ok(json) => {
                                let reasoning = json.get("reasoning")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("No reasoning provided");
                                let action = json.get("action")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("execute");
                                let command = json.get("command")
                                    .and_then(|v| v.as_str());

                                // Show reasoning to user
                                app_state.current_messages_mut().push(models::Message {
                                    role: models::Role::Assistant,
                                    content: format!("💭 **Reasoning**: {}", reasoning),
                                    timestamp: chrono::Utc::now(),
                                });

                                match action {
                                    "goal_achieved" => {
                                        app_state.current_messages_mut().push(models::Message {
                                            role: models::Role::Assistant,
                                            content: "🎉 **Goal Achieved!** The task has been completed successfully.".to_string(),
                                            timestamp: chrono::Utc::now(),
                                        });
                                        if let Some(ref mut agent) = app_state.autonomous_agent {
                                            agent.state = crate::autonomous_agent::AgentState::GoalAchieved;
                                        }
                                    }
                                    "need_info" => {
                                        app_state.current_messages_mut().push(models::Message {
                                            role: models::Role::Assistant,
                                            content: "ℹ️ **Need Information**: I need more information from you to proceed.".to_string(),
                                            timestamp: chrono::Utc::now(),
                                        });
                                        if let Some(ref mut agent) = app_state.autonomous_agent {
                                            agent.state = crate::autonomous_agent::AgentState::Idle;
                                        }
                                    }
                                    "execute" => {
                                        if let Some(cmd) = command {
                                            // Update agent state
                                            if let Some(ref mut agent) = app_state.autonomous_agent {
                                                agent.state = crate::autonomous_agent::AgentState::Executing;
                                                agent.current_step += 1;

                                                // Check safety limit
                                                if agent.should_stop() {
                                                    app_state.current_messages_mut().push(models::Message {
                                                        role: models::Role::Assistant,
                                                        content: format!("⚠️ **Safety Limit Reached**: Maximum steps ({}) exceeded. Stopping for safety.", agent.max_steps),
                                                        timestamp: chrono::Utc::now(),
                                                    });
                                                    agent.state = crate::autonomous_agent::AgentState::Failed;
                                                    return;
                                                }
                                            }

                                            // Show command to user
                                            app_state.current_messages_mut().push(models::Message {
                                                role: models::Role::Assistant,
                                                content: format!("⚡ **Executing**: `{}`", cmd),
                                                timestamp: chrono::Utc::now(),
                                            });

                                            // Execute the command
                                            let cmd_clone = cmd.to_string();
                                            let tx_clone = tx.clone();
                                            tokio::spawn(async move {
                                                use crate::agent::Agent;
                                                let result = Agent::execute_command(&cmd_clone).await;
                                                tx_clone.send(events::AppEvent::AutonomousCommandExecuted(result)).await.ok();
                                            });
                                        } else {
                                            app_state.current_messages_mut().push(models::Message {
                                                role: models::Role::Assistant,
                                                content: "❌ **Error**: Action is 'execute' but no command provided.".to_string(),
                                                timestamp: chrono::Utc::now(),
                                            });
                                        }
                                    }
                                    _ => {
                                        app_state.current_messages_mut().push(models::Message {
                                            role: models::Role::Assistant,
                                            content: format!("❌ **Unknown action**: {}", action),
                                            timestamp: chrono::Utc::now(),
                                        });
                                    }
                                }
                            }
                            Err(e) => {
                                app_state.current_messages_mut().push(models::Message {
                                    role: models::Role::Assistant,
                                    content: format!("❌ **JSON Parse Error**: {}\n\nResponse: {}", e, json_response),
                                    timestamp: chrono::Utc::now(),
                                });
                            }
                        }
                    }
                    Err(e) => {
                        app_state.current_messages_mut().push(models::Message {
                            role: models::Role::Assistant,
                            content: format!("❌ **Reasoning Error**: {}", e),
                            timestamp: chrono::Utc::now(),
                        });
                    }
                }
            }
            Some(events::AppEvent::AutonomousCommandExecuted(result)) => {
                // Command execution completed - now analyze the output
                if let Some(ref mut agent) = app_state.autonomous_agent {
                    agent.state = crate::autonomous_agent::AgentState::AnalyzingOutput;

                    let (output, exit_code) = match &result {
                        Ok(out) => (out.clone(), 0),
                        Err(err) => (err.clone(), 1),
                    };

                    // Show output to user
                    app_state.current_messages_mut().push(models::Message {
                        role: models::Role::Assistant,
                        content: format!("📤 **Output**:\n```\n{}\n```", output),
                        timestamp: chrono::Utc::now(),
                    });

                    // Now ask AI to analyze this output
                    app_state.current_messages_mut().push(models::Message {
                        role: models::Role::Assistant,
                        content: "🔍 Analyzing results and deciding next step...".to_string(),
                        timestamp: chrono::Utc::now(),
                    });

                    app_state.is_loading = true;

                    // Get the last command executed (from reasoning)
                    let last_command = "command".to_string(); // TODO: track this properly
                    let analysis_prompt = agent.create_analysis_prompt(&last_command, &output, exit_code);

                    let client = app_state.http_client.clone();
                    let model = app_state.current_model.clone();
                    let base_url = app_state.ollama_base_url.clone();
                    let auth_config = app_state.config.auth_method.clone();
                    let auth_enabled = app_state.config.auth_enabled;
                    let tx_clone = tx.clone();

                    tokio::spawn(async move {
                        let messages = vec![models::Message {
                            role: models::Role::User,
                            content: analysis_prompt,
                            timestamp: chrono::Utc::now(),
                        }];

                        let (temp_tx, mut temp_rx) = mpsc::channel::<events::AppEvent>(32);
                        let client_clone = client.clone();

                        tokio::spawn(async move {
                            ollama::stream_chat_request(
                                &client_clone,
                                &base_url,
                                &model,
                                &messages,
                                auth_enabled,
                                auth_config.as_ref(),
                                None,
                                temp_tx,
                            )
                            .await;
                        });

                        let mut full_response = String::new();
                        while let Some(event) = temp_rx.recv().await {
                            match event {
                                events::AppEvent::OllamaChunk(Ok(chunk)) => {
                                    full_response.push_str(&chunk);
                                }
                                events::AppEvent::OllamaDone => break,
                                _ => {}
                            }
                        }

                        tx_clone.send(events::AppEvent::AutonomousAnalysisComplete(Ok(full_response))).await.ok();
                    });
                }
            }
            Some(events::AppEvent::AutonomousAnalysisComplete(result)) => {
                app_state.is_loading = false;

                match result {
                    Ok(analysis_response) => {
                        // Parse analysis JSON
                        use serde_json::Value;

                        let json_str = if let Some(start) = analysis_response.find('{') {
                            if let Some(end) = analysis_response.rfind('}') {
                                &analysis_response[start..=end]
                            } else {
                                &analysis_response
                            }
                        } else {
                            &analysis_response
                        };

                        match serde_json::from_str::<Value>(json_str) {
                            Ok(json) => {
                                let analysis = json.get("analysis")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("No analysis provided");
                                let progress = json.get("progress")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("");

                                // Show analysis
                                app_state.current_messages_mut().push(models::Message {
                                    role: models::Role::Assistant,
                                    content: format!("📊 **Analysis**: {}\n\n✅ **Progress**: {}", analysis, progress),
                                    timestamp: chrono::Utc::now(),
                                });

                                // Continue the loop - ask AI for next step
                                if let Some(ref mut agent) = app_state.autonomous_agent {
                                    agent.state = crate::autonomous_agent::AgentState::Reasoning;

                                    app_state.current_messages_mut().push(models::Message {
                                        role: models::Role::Assistant,
                                        content: "🔄 Planning next step...".to_string(),
                                        timestamp: chrono::Utc::now(),
                                    });

                                    app_state.is_loading = true;

                                    let reasoning_prompt = agent.create_reasoning_prompt();
                                    let client = app_state.http_client.clone();
                                    let model = app_state.current_model.clone();
                                    let base_url = app_state.ollama_base_url.clone();
                                    let auth_config = app_state.config.auth_method.clone();
                                    let auth_enabled = app_state.config.auth_enabled;
                                    let tx_clone = tx.clone();

                                    tokio::spawn(async move {
                                        let messages = vec![models::Message {
                                            role: models::Role::User,
                                            content: reasoning_prompt,
                                            timestamp: chrono::Utc::now(),
                                        }];

                                        let (temp_tx, mut temp_rx) = mpsc::channel::<events::AppEvent>(32);
                                        let client_clone = client.clone();

                                        tokio::spawn(async move {
                                            ollama::stream_chat_request(
                                                &client_clone,
                                                &base_url,
                                                &model,
                                                &messages,
                                                auth_enabled,
                                                auth_config.as_ref(),
                                                None,
                                                temp_tx,
                                            )
                                            .await;
                                        });

                                        let mut full_response = String::new();
                                        while let Some(event) = temp_rx.recv().await {
                                            match event {
                                                events::AppEvent::OllamaChunk(Ok(chunk)) => {
                                                    full_response.push_str(&chunk);
                                                }
                                                events::AppEvent::OllamaDone => break,
                                                _ => {}
                                            }
                                        }

                                        tx_clone.send(events::AppEvent::AutonomousReasoningComplete(Ok(full_response))).await.ok();
                                    });
                                }
                            }
                            Err(e) => {
                                app_state.current_messages_mut().push(models::Message {
                                    role: models::Role::Assistant,
                                    content: format!("❌ **Analysis Parse Error**: {}", e),
                                    timestamp: chrono::Utc::now(),
                                });
                            }
                        }
                    }
                    Err(e) => {
                        app_state.current_messages_mut().push(models::Message {
                            role: models::Role::Assistant,
                            content: format!("❌ **Analysis Error**: {}", e),
                            timestamp: chrono::Utc::now(),
                        });
                    }
                }
            }
            None => break,
        }
    }

    restore_terminal(&mut terminal)?;
    Ok(())
}

