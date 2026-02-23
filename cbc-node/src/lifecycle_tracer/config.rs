// Configuration module for lifecycle tracer

use std::path::PathBuf;
use serde::{Deserialize, Serialize};

/// Tracer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TracerConfig {
    pub enabled: bool,
    pub format: TraceFormat,
    pub output_destinations: Vec<OutputDestination>,
    pub buffer_size: usize,
    pub flush_interval_ms: u64,
    pub include_thread_id: bool,
    pub include_timestamps: bool,
    pub milestone_only: bool,
}

impl Default for TracerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            format: TraceFormat::HumanReadable,
            output_destinations: vec![OutputDestination::Stdout],
            buffer_size: 1000,
            flush_interval_ms: 100,
            include_thread_id: true,
            include_timestamps: true,
            milestone_only: false,
        }
    }
}

impl TracerConfig {
    /// Parse configuration from TOML file
    pub fn from_toml_file(path: &PathBuf) -> Result<Self, String> {
        let contents = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read config file: {}", e))?;
        
        let config: Self = toml::from_str(&contents)
            .map_err(|e| format!("Failed to parse TOML config: {}", e))?;
        
        Ok(config)
    }

    /// Parse configuration from CLI flags
    pub fn from_cli_flags(
        enabled: bool,
        format: Option<String>,
        output_file: Option<PathBuf>,
        milestone_only: bool,
    ) -> Self {
        let trace_format = match format.as_deref() {
            Some("json") => TraceFormat::Json,
            Some("human-readable") | Some("human") => TraceFormat::HumanReadable,
            _ => TraceFormat::HumanReadable,
        };

        let mut destinations = vec![OutputDestination::Stdout];
        if let Some(path) = output_file {
            destinations.push(OutputDestination::File(path));
        }

        Self {
            enabled,
            format: trace_format,
            output_destinations: destinations,
            milestone_only,
            ..Default::default()
        }
    }
}

/// Trace output format
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TraceFormat {
    Json,
    HumanReadable,
}

/// Output destination for traces
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OutputDestination {
    Stdout,
    File(PathBuf),
    Metrics,
}
