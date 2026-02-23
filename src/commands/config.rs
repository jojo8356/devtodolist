use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use colored::Colorize;
use directories::ProjectDirs;

use crate::cli::ConfigCommands;
use crate::error::{DevTodoError, Result};

fn config_path() -> Result<PathBuf> {
    let dirs = ProjectDirs::from("", "", "devtodo")
        .ok_or_else(|| DevTodoError::Config("Cannot determine config directory".into()))?;
    let config_dir = dirs.config_dir();
    fs::create_dir_all(config_dir)?;
    Ok(config_dir.join("config.toml"))
}

fn load_config() -> Result<BTreeMap<String, String>> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(BTreeMap::new());
    }
    let content = fs::read_to_string(&path)?;
    let map: BTreeMap<String, String> = toml::from_str(&content)?;
    Ok(map)
}

fn save_config(map: &BTreeMap<String, String>) -> Result<()> {
    let path = config_path()?;
    let content = toml::to_string_pretty(map)?;
    fs::write(&path, content)?;

    // Set restrictive permissions (tokens security)
    let perms = fs::Permissions::from_mode(0o600);
    fs::set_permissions(&path, perms)?;

    Ok(())
}

const TOKEN_KEYS: &[&str] = &["github.token", "gitlab.token"];

fn is_token_key(key: &str) -> bool {
    TOKEN_KEYS.contains(&key)
}

fn mask_value(key: &str, value: &str) -> String {
    if is_token_key(key) && value.len() > 8 {
        format!("{}...{}", &value[..4], &value[value.len() - 4..])
    } else if is_token_key(key) {
        "****".to_string()
    } else {
        value.to_string()
    }
}

pub fn run(command: &ConfigCommands) -> Result<()> {
    match command {
        ConfigCommands::Set { key, value } => {
            let mut config = load_config()?;
            config.insert(key.clone(), value.clone());
            save_config(&config)?;
            let display_val = mask_value(key, value);
            println!(
                "{} Set {} = {}",
                "✓".green().bold(),
                key.bold(),
                display_val
            );
        }
        ConfigCommands::Get { key } => {
            let config = load_config()?;
            match config.get(key) {
                Some(value) => {
                    let display_val = mask_value(key, value);
                    println!("{} = {}", key.bold(), display_val);
                }
                None => {
                    println!("{} Key '{}' not found", "!".yellow().bold(), key);
                }
            }
        }
        ConfigCommands::List => {
            let config = load_config()?;
            if config.is_empty() {
                println!("{}", "No configuration set.".dimmed());
                return Ok(());
            }
            for (key, value) in &config {
                let display_val = mask_value(key, value);
                println!("{} = {}", key.bold(), display_val);
            }
        }
    }

    Ok(())
}

/// Read a config value (used by other modules like providers)
pub fn get_value(key: &str) -> Result<Option<String>> {
    let config = load_config()?;
    Ok(config.get(key).cloned())
}
