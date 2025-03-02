
use std::io::{stdin, stdout, Write};
use std::sync::Arc;
use std::io;
use ratatui::{backend::CrosstermBackend, Terminal};
use crate::{
    app::{App, AppResult},
    event::{Event, EventHandler},
    handler::{handle_key_events, fetch_models},
    tui::Tui,
};

pub mod app;
pub mod event;
pub mod handler;
pub mod tui;
pub mod ui;

fn prompt(message: &str) -> String {
    print!("{}", message);
    stdout().flush().unwrap();
    let mut input = String::new();
    stdin().read_line(&mut input).unwrap();
    input.trim().to_string()
}

#[tokio::main]
async fn main() -> AppResult<()> {
    let app = App::new(); // Arc<Mutex<App>>

    // Attempt to read configuration from the config file.
    {
        let mut app_lock = app.lock().unwrap();
        match app_lock.read_config().await {
            Ok(true) => {
                println!("Config file exists and has been loaded successfully.");
            }
            Err(e) => {
                let err_msg = e.to_string();
                if err_msg.contains("Config file does not exist") {
                    println!("Config file does not exist. Let's create one.");

                    // Ask for domain (with default if left empty)
                    let domain_input = prompt("Enter domain (default: http://localhost:11434): ");
                    let domain = if domain_input.is_empty() {
                        "http://localhost:11434".to_string()
                    } else {
                        domain_input
                    };

                    // Ask if authentication is needed
                    let auth_needed = prompt("Do you need authentication? (y/n): ");
                    let (username, password) = if auth_needed.to_lowercase().starts_with('y') {
                        let username = prompt("Enter username: ");
                        let password = prompt("Enter password: ");
                        (username, password)
                    } else {
                        ("default_user".to_string(), "default_pass".to_string())
                    };

                    // Update the config fields
                    app_lock.config.domain = domain;
                    app_lock.config.username = username;
                    app_lock.config.password = password;

                    // Write the new config file asynchronously
                    if let Err(e) = app_lock.write_config().await {
                        eprintln!("Failed to write config: {}", e);
                    } else {
                        println!("Config file created successfully.");
                    }
                } else {
                    println!("An error occurred while reading config: {}", e);
                }
            }
            _ => {
                println!("Unexpected result while reading the config.");
            }
        }
    }

    // Fetch models asynchronously before starting the UI.
    if let Err(e) = fetch_models(Arc::clone(&app)).await {
        eprintln!("Failed to fetch models: {}", e);
    }

    let backend = CrosstermBackend::new(io::stdout());
    let terminal = Terminal::new(backend)?;
    let events = EventHandler::new(250);
    let mut tui = Tui::new(terminal, events);
    tui.init()?;

    while app.lock().unwrap().running {
        tui.draw(&mut app.lock().unwrap())?;

        match tui.events.next().await? {
            Event::Tick => app.lock().unwrap().tick(),
            Event::Key(key_event) => handle_key_events(key_event, Arc::clone(&app))?,
            Event::Mouse(_) => {}
            Event::Resize(_, _) => {}
        }
    }

    tui.exit()?;
    Ok(())
}

