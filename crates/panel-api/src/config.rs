//! Configuration loader for `panel-api`.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Failed to read config file: {0}")]
    Io(#[from] std::io::Error),
    #[error("Failed to parse TOML config: {0}")]
    Toml(#[from] toml::de::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelConfig {
    pub server: ServerConfig,
    pub agent: AgentConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub listen_addr: String,
    pub environment: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub socket_path: PathBuf,
}

impl Default for PanelConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                listen_addr: "127.0.0.1:7700".to_string(),
                environment: "development".to_string(),
            },
            agent: AgentConfig {
                socket_path: PathBuf::from("/run/panel/agent.sock"),
            },
        }
    }
}

impl PanelConfig {
    pub fn load(path_override: Option<&Path>) -> Result<Self, ConfigError> {
        let candidates = if let Some(p) = path_override {
            vec![p.to_path_buf()]
        } else if let Ok(env_path) = std::env::var("PANEL_CONFIG") {
            vec![PathBuf::from(env_path)]
        } else {
            vec![
                PathBuf::from("/etc/panel/panel.toml"),
                PathBuf::from("panel.toml"),
                PathBuf::from("../../panel.toml"),
            ]
        };

        for candidate in candidates {
            if candidate.exists() {
                let content = std::fs::read_to_string(&candidate)?;
                let config: PanelConfig = toml::from_str(&content)?;
                return Ok(config);
            }
        }

        // Return default configuration if no file exists
        Ok(Self::default())
    }
}
