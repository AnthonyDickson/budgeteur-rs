//! Configuration loading for the TUI client. Reads `config.toml` from the
//! XDG config directory. All values can be overridden via CLI flags or env vars.

use serde::Deserialize;
use std::path::{Path, PathBuf};

const PRIVATE_KEY_FILENAME: &str = "tui_private_key";

/// Resolved configuration (defaults merged with file and CLI overrides).
#[derive(Debug)]
pub struct Config {
    /// The base URL of the Budgeteur server (e.g. `http://192.168.1.100:3000`).
    pub server_url: String,

    /// The path of the private key.
    pub private_key_path: String,
}

/// Explicit overrides (e.g. from CLI flags). Only `Some` fields overwrite.
#[derive(Debug, Default)]
pub struct ConfigOverrides {
    pub server_url: Option<String>,
    pub private_key_path: Option<String>,
}

/// Raw config file fields — all optional so we know what the user set.
#[derive(Debug, Deserialize)]
struct ConfigFile {
    server_url: Option<String>,
    private_key_path: Option<String>,
}

fn default_server_url() -> String {
    "http://localhost:3000".into()
}

pub fn default_private_key_path() -> String {
    private_key_path()
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_else(|| PRIVATE_KEY_FILENAME.to_string())
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server_url: default_server_url(),
            private_key_path: default_private_key_path(),
        }
    }
}

impl Config {
    /// Resolve configuration: defaults → config file → explicit overrides.
    /// Only `Some` fields from each layer overwrite previous values.
    pub fn resolve(overrides: ConfigOverrides) -> Self {
        let mut config = Self::default();
        Self::apply_file(&mut config);
        Self::apply_overrides(&mut config, overrides);
        config
    }

    /// Overrides the config fields with those defined in the config file `~/.config/budgeteur/config.toml`.
    fn apply_file(config: &mut Self) {
        let path = match config_path() {
            Some(p) => p,
            None => return,
        };

        let contents = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return,
            Err(e) => {
                eprintln!(
                    "Warning: could not read {}: {e}. Using defaults.",
                    path.display()
                );
                return;
            }
        };

        let file: ConfigFile = match toml::from_str(&contents) {
            Ok(f) => f,
            Err(e) => {
                eprintln!(
                    "Warning: could not parse {}: {e}. Using defaults.",
                    path.display()
                );
                return;
            }
        };

        if let Some(url) = file.server_url {
            config.server_url = url;
        }
        if let Some(path) = file.private_key_path {
            config.private_key_path = path;
        }
    }

    /// Overrides the config fields with those defined in `overrides`.
    fn apply_overrides(config: &mut Self, overrides: ConfigOverrides) {
        if let Some(url) = overrides.server_url {
            config.server_url = url;
        }
        if let Some(path) = overrides.private_key_path {
            config.private_key_path = path;
        }
    }
}

/// Path to the config file: `~/.config/budgeteur/config.toml`.
pub fn config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("budgeteur").join("config.toml"))
}

/// Path to the TUI private key: `~/.local/share/budgeteur/tui_private_key`.
pub fn private_key_path() -> Option<PathBuf> {
    dirs::data_local_dir().map(|d| d.join("budgeteur").join(PRIVATE_KEY_FILENAME))
}

/// Ensure the directory for `path` exists, creating it if needed.
pub fn ensure_parent_dir(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}
