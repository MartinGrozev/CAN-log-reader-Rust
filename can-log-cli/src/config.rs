//! Configuration loading and parsing (Phase 7)

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Main application configuration (loaded from config.toml)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppConfig {
    pub input: InputConfig,
    pub signals: SignalsConfig,
    pub output: OutputConfig,
    #[serde(default)]
    pub cantp: CanTpConfig,
    #[serde(default)]
    pub filtering: FilteringConfig,
    #[serde(default)]
    pub callbacks: CallbacksConfig,
    #[serde(default)]
    pub events: Vec<EventConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InputConfig {
    pub files: Vec<PathBuf>,
    pub dbc_files: Vec<PathBuf>,
    #[serde(default)]
    pub arxml_files: Vec<PathBuf>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SignalsConfig {
    pub track: SignalTrackMode,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum SignalTrackMode {
    All(String), // "all"
    List(Vec<String>),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OutputConfig {
    pub format: OutputFormat,
    pub output_dir: Option<PathBuf>,
    #[serde(default)]
    pub include_basic_info: bool,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    Txt,
    Html,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct CanTpConfig {
    #[serde(default)]
    pub pairs: Vec<CanTpPairConfig>,
    #[serde(default)]
    pub auto_detect: bool,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}

fn default_timeout() -> u64 {
    1000
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CanTpPairConfig {
    pub source: u32,
    pub target: u32,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct FilteringConfig {
    pub channels: Option<Vec<u8>>,
    pub message_ids: Option<Vec<u32>>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct CallbacksConfig {
    pub library: Option<PathBuf>,
    #[serde(default)]
    pub simple: Vec<SimpleCallbackConfig>,
    #[serde(default)]
    pub c_function: Vec<CFunctionCallbackConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SimpleCallbackConfig {
    pub signal: String,
    pub condition: String,
    pub action: String,
    pub message: Option<String>,
    pub event: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CFunctionCallbackConfig {
    pub signal: String,
    pub function: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EventConfig {
    pub name: String,
    pub start_condition: String,
    pub end_condition: String,
    pub error_condition: Option<String>,
    pub parent_event: Option<String>,
    #[serde(default)]
    pub allow_multiple_instances: bool,
    #[serde(default)]
    pub capture_signals_on_end: Vec<String>,
}

/// Load configuration from a TOML file
pub fn load_config(path: &Path) -> Result<AppConfig> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read config file: {:?}", path))?;

    let config: AppConfig = toml::from_str(&content)
        .with_context(|| format!("Failed to parse config file: {:?}", path))?;

    // TODO: Phase 7 - Validate configuration
    // - Check that files exist
    // - Validate signal names
    // - Validate event dependencies

    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_deserialization() {
        let toml_content = r#"
            [input]
            files = ["trace.blf"]
            dbc_files = ["powertrain.dbc"]

            [signals]
            track = ["EngineSpeed", "VehicleSpeed"]

            [output]
            format = "txt"
        "#;

        let config: AppConfig = toml::from_str(toml_content).unwrap();
        assert_eq!(config.input.files.len(), 1);
        assert_eq!(config.input.dbc_files.len(), 1);
    }
}
