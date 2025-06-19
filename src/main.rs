
mod app;
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
use ratatui::prelude::*;
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
            if crossterm::event::poll(Duration::from_millis(50)).unwrap_or(false) {
                if let crossterm::event::Event::Key(key) = crossterm::event::read().unwrap() {
                    if event_tx.send(events::AppEvent::Terminal(key)).await.is_err() {
                        break;
                    }
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
        terminal.draw(|f| ui::ui(f, &mut app_state))?;

        match rx.recv().await {
            Some(events::AppEvent::Terminal(key)) => {
                if key.kind == crossterm::event::KeyEventKind::Press {
                    if events::handle_key_event(key, &mut app_state, tx.clone()).await {
                        break;
                    }
                }
            }
            Some(events::AppEvent::OllamaChunk(Ok(chunk))) => {
                if let Some(last_message) = app_state.current_messages_mut().last_mut() {
                    if last_message.role == models::Role::Assistant {
                        last_message.content.push_str(&chunk);
                        app_state.scroll_offset = 0;
                    }
                }
            }
            Some(events::AppEvent::OllamaChunk(Err(e))) => {
                if let Some(last_message) = app_state.current_messages_mut().last_mut() {
                    if last_message.role == models::Role::Assistant {
                        let err_msg = format!("\n[STREAM ERROR: {}]", e);
                        last_message.content.push_str(&err_msg);
                    }
                }
                app_state.is_loading = false;
            }
            Some(events::AppEvent::OllamaDone) => {
                app_state.is_loading = false;
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
                app_state.available_models = models;
                if !app_state.available_models.is_empty()
                    && app_state.current_model == "No model selected"
                {
                    app_state.current_model = app_state.available_models[0].clone();
                    app_state.model_list_state.select(Some(0));
                }
            }
            Some(events::AppEvent::Models(Err(e))) => {
                app_state.current_messages_mut().push(models::Message {
                    role: models::Role::Assistant,
                    content: format!("Error fetching models: {}. Is Ollama running?", e),
                });
            }
            None => break,
        }
    }

    restore_terminal(&mut terminal)?;
    Ok(())
}

