//! Configuration loading for the TUI client. Reads `config.toml` from the
//! XDG config directory. All values can be overridden via CLI flags or env vars.

use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Configuration loaded from `~/.config/budgeteur/config.toml`.
#[derive(Debug, Deserialize)]
pub struct Config {
    /// The base URL of the Budgeteur server (e.g. `http://192.168.1.100:3000`).
    #[serde(default = "default_server_url")]
    pub server_url: String,
}

fn default_server_url() -> String {
    "http://localhost:3000".into()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server_url: default_server_url(),
        }
    }
}

impl Config {
    /// Load config from `~/.config/budgeteur/config.toml`. Returns defaults
    /// if the file doesn't exist or can't be parsed.
    pub fn load() -> Self {
        let path = match config_path() {
            Some(p) => p,
            None => return Self::default(),
        };

        match std::fs::read_to_string(&path) {
            Ok(contents) => toml::from_str(&contents).unwrap_or_else(|e| {
                eprintln!(
                    "Warning: could not parse {}: {e}. Using defaults.",
                    path.display()
                );
                Self::default()
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // No config file is fine — use defaults.
                Self::default()
            }
            Err(e) => {
                eprintln!(
                    "Warning: could not read {}: {e}. Using defaults.",
                    path.display()
                );
                Self::default()
            }
        }
    }
}

/// Path to the config file: `~/.config/budgeteur/config.toml`.
pub fn config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("budgeteur").join("config.toml"))
}

/// Path to the TUI private key: `~/.local/share/budgeteur/tui_private_key`.
pub fn private_key_path() -> Option<PathBuf> {
    dirs::data_local_dir().map(|d| d.join("budgeteur").join("tui_private_key"))
}

/// Ensure the directory for `path` exists, creating it if needed.
pub fn ensure_parent_dir(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}
