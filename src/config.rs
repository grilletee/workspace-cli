use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::Result;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct WorkspaceConfig {
    pub profiles: HashMap<String, Profile>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Profile {
    pub apps: Vec<AppSpec>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppSpec {
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub args: Vec<String>,
    pub position: Option<PositionSpec>,
    #[serde(default)]
    pub launch_delay_ms: u64,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PositionSpec {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

pub fn default_timeout_ms() -> u64 {
    5000
}

pub fn load_config(path: &Path) -> Result<WorkspaceConfig> {
    let contents = fs::read_to_string(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            anyhow::anyhow!("configuration file not found: {}", path.display())
        } else {
            anyhow::anyhow!("failed to read configuration file '{}': {error}", path.display())
        }
    })?;

    serde_json::from_str(&contents).map_err(|error| {
        let message = error.to_string();
        if message.contains("unknown field") {
            anyhow::anyhow!("unknown field in configuration JSON: {message}")
        } else {
            anyhow::anyhow!("invalid configuration JSON: {message}")
        }
    })
}
