use crate::app::{App, AppResult, Mode};
use crossterm::event::{KeyCode, KeyEvent};
use reqwest::Client;
use serde_json::Value;
use std::sync::{Arc, Mutex};

/// Handles keyboard input and updates the app state.
pub fn handle_key_events(key_event: KeyEvent, app: Arc<Mutex<App>>) -> AppResult<()> {
    let mut app_lock = app.lock().unwrap(); // Lock app to prevent concurrent access

    match key_event.code {
        KeyCode::Esc => app_lock.mode = Mode::Escape,
        KeyCode::Char('q') => {
            if app_lock.mode == Mode::Escape {
                app_lock.quit();
            } else {
                app_lock.input.push('q');
            }
        }
        KeyCode::Char('m') => {
            if app_lock.mode == Mode::Escape {
                app_lock.mode = Mode::Model;
            } else {
                app_lock.input.push('m');
            }
        }
        KeyCode::Char('c') => {
            if app_lock.mode == Mode::Escape {
                app_lock.mode = Mode::Chat;
            } else {
                app_lock.input.push('c');
            }
        }
        KeyCode::Enter => {
            if !app_lock.input.is_empty() {
                let input = app_lock.input.clone();
                app_lock.add_message(format!("You: {}", input));
                app_lock.input.clear();

                let app_clone = Arc::clone(&app); // ✅ Correctly cloning `Arc<Mutex<App>>`
                tokio::spawn(async move {
                    send_message_to_ollama(input, app_clone).await;
                });
            }
        }
        KeyCode::Char(c) => {
            if app_lock.mode != Mode::Escape {
                app_lock.input.push(c);
            }
        }
        KeyCode::Backspace => {
            app_lock.input.pop();
        }

        // Navigate model list with Up/Down arrows (Fixing Borrow Issue)
        KeyCode::Up => {
            if app_lock.mode == Mode::Chat {
                let message_scroll = app_lock.message_scroll;
                app_lock.set_message_scroll(message_scroll.saturating_sub(1));
            } else if app_lock.mode == Mode::Model {
                if let Some(index) = app_lock.models.iter().position(|m| m == &app_lock.model) {
                    if index > 0 {
                        let new_model = app_lock.models[index - 1].clone(); // ✅ Fix: Store in a variable
                        app_lock.set_model(new_model);
                    }
                }
                let model_index = app_lock.model_scroll;
                app_lock.set_model_scroll(model_index.saturating_sub(1));
            }
        }
        KeyCode::Down => {
            if app_lock.mode == Mode::Chat {
                let message_scroll = app_lock.message_scroll;
                app_lock.set_message_scroll(message_scroll.saturating_add(1));
            } else if app_lock.mode == Mode::Model {
                if let Some(index) = app_lock.models.iter().position(|m| m == &app_lock.model) {
                    if index < app_lock.models.len() - 1 {
                        let new_model = app_lock.models[index + 1].clone(); // ✅ Fix: Store in a variable
                        app_lock.set_model(new_model);
                    }
                }
                let model_index = app_lock.model_scroll;
                app_lock.set_model_scroll(model_index.saturating_add(1));
            }
        }
        KeyCode::Tab => {
            if let Some(first_model) = app_lock.models.first() {
                let first_model_name = first_model.clone();
                app_lock.set_model(first_model_name);
            }
        }
        _ => {}
    }
    Ok(())
}

/// Sends a message to the Ollama API and maintains context for future requests.
pub async fn send_message_to_ollama(message: String, app: Arc<Mutex<App>>) {
    let client = Client::new();
    // Retrieve model and context from the app state
    let (model, context,domain, username, password) = {
        let app_lock = app.lock().unwrap();
        (
            app_lock.model.clone(),
            app_lock.context.clone(),
            app_lock.config.domain.clone(),
            app_lock.config.username.clone(),
            app_lock.config.password.clone(),
        ) // Clone existing context

    };
    let url = format!("{}/api/generate", domain);
    let body = serde_json::json!({
        "model": model,
        "prompt": message,
        "stream": false,
        "context": context, // Send previous context
    });
    // Send the request and handle errors
    let response = client
        .post(url)
        .basic_auth(username, Some(password))
        .json(&body)
        .send()
        .await;

    match response {
        Ok(resp) => {
            if resp.status().is_success() {
                match resp.json::<Value>().await {
                    Ok(json) => {
                        let mut app_lock = app.lock().unwrap();

                        // Extract AI response
                        if let Some(reply) = json["response"].as_str() {
                            app_lock.add_message(format!("Ollama: {}", reply));
                        } else {
                            app_lock.add_message(
                                "⚠️ Error: Invalid response format from Ollama API.".to_string(),
                            );
                        }

                        // Extract and update context if available
                        if let Some(new_context) = json["context"].as_array() {
                            app_lock.context = new_context.iter().map(|c| c.clone()).collect();

                        }
                    }
                    Err(err) => {
                        let mut app_lock = app.lock().unwrap();
                        app_lock.add_message(format!("⚠️ Error parsing JSON response: {}", err));
                    }
                }
            } else {
                let mut app_lock = app.lock().unwrap();
                app_lock.add_message(format!(
                    "⚠️ API Error: Received status code {}",
                    resp.status()
                ));
            }
        }
        Err(err) => {
            let mut app_lock = app.lock().unwrap();
            app_lock.add_message(format!("⚠️ Network Error: {}", err));
        }
    }
}

/// Fetches available models from the Ollama API and updates the app state asynchronously.
pub async fn fetch_models(app: Arc<Mutex<App>>) -> AppResult<()> {
    let client = Client::new();

    // Retrieve domain, username, and password from the app's config
    let (domain, username, password) = {
        let app_lock = app.lock().unwrap();
        (
            app_lock.config.domain.clone(),
            app_lock.config.username.clone(),
            app_lock.config.password.clone(),
        )
    };

    // Construct the URL using the domain from config
    let url = format!("{}/api/tags", domain);

    let response = client
        .get(&url)
        .basic_auth(username, Some(password))
        .send()
        .await?
        .json::<Value>()
        .await?;

    if let Some(models) = response["models"].as_array() {
        let model_names: Vec<String> = models
            .iter()
            .filter_map(|m| m["name"].as_str().map(|s| s.to_string()))
            .collect();

        let mut app_lock = app.lock().unwrap();
        if !model_names.is_empty() {
            app_lock.set_models(model_names);
            let first_model_name = app_lock.models[0].clone();
            app_lock.set_model(first_model_name);
        }
    }
    Ok(())
}

