use crate::models::Config;
use anyhow::{anyhow, Result};
use directories::ProjectDirs;
use std::fs;
use std::path::PathBuf;

impl Default for Config {
    fn default() -> Self {
        Self {
            ollama_host: "http://127.0.0.1".to_string(),
            ollama_port: 11434,
            db_filename: "ollama-tui.sqlite".to_string(),
            auth_enabled: false,
            auth_method: None,
        }
    }
}

pub fn get_config_path() -> Result<PathBuf> {
    let proj_dirs = ProjectDirs::from("com", "rust-tui", "ollama-tui")
        .ok_or_else(|| anyhow!("Could not find a valid config directory."))?;
    let config_dir = proj_dirs.config_dir();
    fs::create_dir_all(config_dir)?;
    Ok(config_dir.join("config.json"))
}

pub fn load_or_create() -> Result<Config> {
    let config_path = get_config_path()?;
    if !config_path.exists() {
        let config = Config::default();
        fs::write(&config_path, serde_json::to_string_pretty(&config)?)?;
        Ok(config)
    } else {
        let config_str = fs::read_to_string(config_path)?;
        let config: Config = serde_json::from_str(&config_str)?;
        Ok(config)
    }
}

