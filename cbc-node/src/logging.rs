// CBC Logging Infrastructure
//
// This module provides structured logging with CBC-specific prefixes and formatting.
// It supports color-coded output, log filtering, log file redirection, startup information display,
// and log deduplication for repeated messages.

use std::env;
use std::fs::{File, OpenOptions};
use std::io::{self, Write, BufWriter};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::time::{SystemTime, Duration, UNIX_EPOCH};
use log::Level;

/// CBC-specific log formatter with color coding and prefixes
pub struct CbcLogFormatter {
    pub enable_colors: bool,
    #[allow(dead_code)]
    pub enable_timestamps: bool,
}

/// Log deduplication tracker to summarize repeated messages
#[derive(Clone)]
#[allow(dead_code)]
pub struct LogDeduplicator {
    message_counts: Arc<Mutex<HashMap<String, (u32, SystemTime)>>>,
    dedup_window: Duration,
}

#[allow(dead_code)]
impl LogDeduplicator {
    pub fn new(dedup_window_secs: u64) -> Self {
        Self {
            message_counts: Arc::new(Mutex::new(HashMap::new())),
            dedup_window: Duration::from_secs(dedup_window_secs),
        }
    }

    /// Check if a message should be logged or deduplicated
    /// Returns Some(count) if this is a duplicate that should be summarized
    pub fn should_deduplicate(&self, message: &str) -> Option<u32> {
        let mut counts = self.message_counts.lock().unwrap();
        let now = SystemTime::now();
        
        // Clean up old entries
        counts.retain(|_, (_, timestamp)| {
            now.duration_since(*timestamp).unwrap_or(Duration::MAX) < self.dedup_window
        });
        
        if let Some((count, _first_seen)) = counts.get_mut(message) {
            *count += 1;
            if *count > 1 {
                return Some(*count);
            }
        } else {
            counts.insert(message.to_string(), (1, now));
        }
        
        None
    }

    /// Get summary of deduplicated messages
    pub fn get_summary(&self) -> Vec<(String, u32)> {
        let counts = self.message_counts.lock().unwrap();
        counts.iter()
            .filter(|(_, (count, _))| *count > 1)
            .map(|(msg, (count, _))| (msg.clone(), *count))
            .collect()
    }
}

/// Log file writer that handles file redirection and rotation
#[allow(dead_code)]
pub struct LogFileWriter {
    writer: Option<BufWriter<File>>,
    #[allow(dead_code)]
    file_path: Option<String>,
}

#[allow(dead_code)]
impl LogFileWriter {
    pub fn new(file_path: Option<String>) -> io::Result<Self> {
        let writer = if let Some(ref path) = file_path {
            // Create parent directories if they don't exist
            if let Some(parent) = Path::new(path).parent() {
                std::fs::create_dir_all(parent)?;
            }
            
            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)?;
            Some(BufWriter::new(file))
        } else {
            None
        };

        Ok(Self {
            writer,
            file_path,
        })
    }

    pub fn write_log(&mut self, message: &str) -> io::Result<()> {
        if let Some(ref mut writer) = self.writer {
            writeln!(writer, "{}", message)?;
            writer.flush()?;
        }
        Ok(())
    }

    pub fn is_file_logging(&self) -> bool {
        self.writer.is_some()
    }
}

/// Startup information display
pub struct StartupInfo {
    pub runtime_version: String,
    pub node_version: String,
    pub peer_id: Option<String>,
    pub network_latency: Option<Duration>,
    pub chain_name: String,
    pub node_role: String,
}

impl StartupInfo {
    pub fn new(runtime_version: String, node_version: String, chain_name: String, node_role: String) -> Self {
        Self {
            runtime_version,
            node_version,
            peer_id: None,
            network_latency: None,
            chain_name,
            node_role,
        }
    }

    pub fn set_peer_id(&mut self, peer_id: String) {
        self.peer_id = Some(peer_id);
    }

    pub fn set_network_latency(&mut self, latency: Duration) {
        self.network_latency = Some(latency);
    }

    pub fn display(&self) {
        println!("CBC Node Startup Information");
        println!("================================");
        println!("Node Version:     {}", self.node_version);
        println!("Runtime Version:  {}", self.runtime_version);
        println!("Chain:            {}", self.chain_name);
        println!("Role:             {}", self.node_role);
        
        if let Some(ref peer_id) = self.peer_id {
            println!("Peer ID:          {}", peer_id);
        }
        
        if let Some(latency) = self.network_latency {
            println!("Network Latency:  {:?}", latency);
        }
        
        println!("Timestamp:        {}", 
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        );
        println!("================================");
    }
}

impl CbcLogFormatter {
    /// Create a new CBC log formatter
    pub fn new(enable_colors: bool, enable_timestamps: bool) -> Self {
        Self {
            enable_colors,
            enable_timestamps,
        }
    }

    /// Format a validator-related log message with [CBC-VAL] prefix
    pub fn format_validator_log(&self, message: &str) -> String {
        let prefix = if self.enable_colors {
            format!("{}[CBC-VAL]{}", self.color_for_level(&Level::Info), self.reset_color())
        } else {
            "[CBC-VAL]".to_string()
        };
        format!("{} {}", prefix, message)
    }

    /// Format an epoch-related log message with [CBC-EPOCH] prefix
    pub fn format_epoch_log(&self, message: &str) -> String {
        let prefix = if self.enable_colors {
            format!("{}[CBC-EPOCH]{}", self.color_for_level(&Level::Info), self.reset_color())
        } else {
            "[CBC-EPOCH]".to_string()
        };
        format!("{} {}", prefix, message)
    }

    /// Format a consensus-related log message with [CBC-CONSENSUS] prefix
    pub fn format_consensus_log(&self, message: &str) -> String {
        let prefix = if self.enable_colors {
            format!("{}[CBC-CONSENSUS]{}", self.color_for_level(&Level::Info), self.reset_color())
        } else {
            "[CBC-CONSENSUS]".to_string()
        };
        format!("{} {}", prefix, message)
    }

    /// Get ANSI color code for log level
    fn color_for_level(&self, level: &Level) -> &'static str {
        if !self.enable_colors {
            return "";
        }
        
        match level {
            Level::Error => "\x1b[31m",   // Red
            Level::Warn => "\x1b[33m",    // Yellow
            Level::Info => "\x1b[32m",    // Green
            Level::Debug => "\x1b[36m",   // Cyan
            Level::Trace => "\x1b[37m",   // White
        }
    }

    /// Get ANSI reset code
    fn reset_color(&self) -> &'static str {
        if self.enable_colors {
            "\x1b[0m"
        } else {
            ""
        }
    }

    /// Format a log message with appropriate color coding
    #[allow(dead_code)]
    pub fn format_log_with_level(&self, level: &Level, message: &str) -> String {
        if self.enable_colors {
            format!("{}{}{} {}", 
                self.color_for_level(level), 
                level, 
                self.reset_color(), 
                message
            )
        } else {
            format!("{} {}", level, message)
        }
    }
}

/// Configure RUST_LOG environment variable for CBC logging
pub fn configure_rust_log(cbc_log_only: bool, quiet: bool) -> String {
    let log_config = if quiet {
        // Minimal logging - only errors and warnings
        "warn,cbc_node=info,cbc_consensus=info,pallet_cbc_dcf=info"
    } else if cbc_log_only {
        // Only show CBC-related logs when --cbc-log-only is set
        "cbc_node=info,cbc_consensus=info,pallet_cbc_dcf=info,pallet_cbc_pos=info,pallet_cbc_poi=info"
    } else {
        // Show general info logs plus CBC logs (reduced from debug to info)
        "info,cbc_node=info,cbc_consensus=info,pallet_cbc_dcf=info,pallet_cbc_pos=info,pallet_cbc_poi=info"
    };
    
    // Set the environment variable
    env::set_var("RUST_LOG", log_config);
    log_config.to_string()
}

/// Initialize CBC logging configuration (RUST_LOG only, no custom logger)
pub fn init_cbc_logging_config(cbc_log_only: bool, quiet: bool) -> String {
    configure_rust_log(cbc_log_only, quiet)
}

/// Initialize CBC logging system with optional file output
/// 
/// **WARNING**: This function should only be used in tests!
/// In production, Substrate handles logger initialization automatically.
/// Use `init_cbc_logging_config()` instead to just configure RUST_LOG.
/// 
/// Returns Ok(()) if successful, Err if logger is already initialized
#[cfg(test)]
pub fn init_cbc_logging(_enable_colors: bool, cbc_log_only: bool, log_file_path: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
    // Configure RUST_LOG environment variable
    configure_rust_log(cbc_log_only, false);
    
    // Initialize env_logger (this might fail if already initialized, which is fine)
    let result = env_logger::try_init();
    
    // If we have a log file path, create the log file writer
    if let Some(path) = log_file_path {
        let _log_writer = LogFileWriter::new(Some(path))?;
        // Note: In a real implementation, you'd want to store this writer
        // and use it in a custom logger. For now, we just verify it can be created.
    }
    
    match result {
        Ok(()) => Ok(()),
        Err(_) => {
            // Logger already initialized, which is fine in tests
            Ok(())
        }
    }
}

/// Display startup information for the CBC node
pub fn display_startup_info(
    runtime_version: &str,
    node_version: &str,
    chain_name: &str,
    node_role: &str,
    peer_id: Option<&str>,
    network_latency: Option<Duration>,
) {
    let mut startup_info = StartupInfo::new(
        runtime_version.to_string(),
        node_version.to_string(),
        chain_name.to_string(),
        node_role.to_string(),
    );
    
    if let Some(peer_id) = peer_id {
        startup_info.set_peer_id(peer_id.to_string());
    }
    
    if let Some(latency) = network_latency {
        startup_info.set_network_latency(latency);
    }
    
    startup_info.display();
}

/// Log a validator-related event
#[allow(dead_code)]
pub fn log_validator_event(message: &str) {
    let formatter = CbcLogFormatter::new(true, true);
    log::info!("{}", formatter.format_validator_log(message));
}

/// Log an epoch-related event
#[allow(dead_code)]
pub fn log_epoch_event(message: &str) {
    let formatter = CbcLogFormatter::new(true, true);
    log::info!("{}", formatter.format_epoch_log(message));
}

/// Log a consensus-related event
pub fn log_consensus_event(message: &str) {
    let formatter = CbcLogFormatter::new(true, true);
    log::info!("{}", formatter.format_consensus_log(message));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cbc_log_formatter_creation() {
        let formatter = CbcLogFormatter::new(true, true);
        assert!(formatter.enable_colors);
        assert!(formatter.enable_timestamps);
        
        let formatter_no_color = CbcLogFormatter::new(false, false);
        assert!(!formatter_no_color.enable_colors);
        assert!(!formatter_no_color.enable_timestamps);
    }

    #[test]
    fn test_validator_log_formatting() {
        let formatter = CbcLogFormatter::new(false, true);
        let result = formatter.format_validator_log("Validator Alice is active");
        assert!(result.contains("[CBC-VAL]"));
        assert!(result.contains("Validator Alice is active"));
    }

    #[test]
    fn test_epoch_log_formatting() {
        let formatter = CbcLogFormatter::new(false, true);
        let result = formatter.format_epoch_log("Epoch 42 started");
        assert!(result.contains("[CBC-EPOCH]"));
        assert!(result.contains("Epoch 42 started"));
    }

    #[test]
    fn test_consensus_log_formatting() {
        let formatter = CbcLogFormatter::new(false, true);
        let result = formatter.format_consensus_log("Block author selected");
        assert!(result.contains("[CBC-CONSENSUS]"));
        assert!(result.contains("Block author selected"));
    }

    #[test]
    fn test_color_formatting() {
        let formatter = CbcLogFormatter::new(true, true);
        let result = formatter.format_log_with_level(&Level::Error, "Test error");
        assert!(result.contains("\x1b[31m")); // Red color for error
        assert!(result.contains("\x1b[0m"));  // Reset color
        
        let formatter_no_color = CbcLogFormatter::new(false, true);
        let result_no_color = formatter_no_color.format_log_with_level(&Level::Error, "Test error");
        assert!(!result_no_color.contains("\x1b[31m"));
    }

    #[test]
    fn test_configure_rust_log() {
        let config_quiet = configure_rust_log(false, true);
        assert!(config_quiet.contains("warn,"));
        assert!(config_quiet.contains("cbc_node=info"));
        
        let config_cbc_only = configure_rust_log(true, false);
        assert!(config_cbc_only.contains("cbc_node=info"));
        assert!(config_cbc_only.contains("pallet_cbc_dcf=info"));
        assert!(!config_cbc_only.contains("warn,"));
        
        let config_all = configure_rust_log(false, false);
        assert!(config_all.contains("info,"));
        assert!(config_all.contains("cbc_node=info"));
    }

    #[test]
    fn test_log_level_colors() {
        let formatter = CbcLogFormatter::new(true, true);
        
        assert_eq!(formatter.color_for_level(&Level::Error), "\x1b[31m");
        assert_eq!(formatter.color_for_level(&Level::Warn), "\x1b[33m");
        assert_eq!(formatter.color_for_level(&Level::Info), "\x1b[32m");
        assert_eq!(formatter.color_for_level(&Level::Debug), "\x1b[36m");
        assert_eq!(formatter.color_for_level(&Level::Trace), "\x1b[37m");
        
        let formatter_no_color = CbcLogFormatter::new(false, true);
        assert_eq!(formatter_no_color.color_for_level(&Level::Error), "");
    }

    #[test]
    fn test_log_deduplicator() {
        let deduplicator = LogDeduplicator::new(1); // 1 second window
        
        // First occurrence should not be deduplicated
        assert!(deduplicator.should_deduplicate("Test message").is_none());
        
        // Second occurrence should be deduplicated
        assert_eq!(deduplicator.should_deduplicate("Test message"), Some(2));
        
        // Third occurrence should be deduplicated
        assert_eq!(deduplicator.should_deduplicate("Test message"), Some(3));
        
        // Different message should not be deduplicated
        assert!(deduplicator.should_deduplicate("Different message").is_none());
    }

    #[test]
    fn test_log_file_writer_creation() {
        // Test without file path
        let writer = LogFileWriter::new(None).unwrap();
        assert!(!writer.is_file_logging());
        
        // Test with file path (using temp directory)
        let temp_dir = std::env::temp_dir();
        let log_path = temp_dir.join("test_cbc.log");
        let writer = LogFileWriter::new(Some(log_path.to_string_lossy().to_string())).unwrap();
        assert!(writer.is_file_logging());
        
        // Clean up
        let _ = std::fs::remove_file(log_path);
    }

    #[test]
    fn test_startup_info() {
        let mut startup_info = StartupInfo::new(
            "1.0.0".to_string(),
            "0.1.0".to_string(),
            "CBC Testnet".to_string(),
            "Full Node".to_string(),
        );
        
        assert_eq!(startup_info.runtime_version, "1.0.0");
        assert_eq!(startup_info.node_version, "0.1.0");
        assert_eq!(startup_info.chain_name, "CBC Testnet");
        assert_eq!(startup_info.node_role, "Full Node");
        assert!(startup_info.peer_id.is_none());
        assert!(startup_info.network_latency.is_none());
        
        startup_info.set_peer_id("12D3KooWTest".to_string());
        startup_info.set_network_latency(Duration::from_millis(50));
        
        assert_eq!(startup_info.peer_id, Some("12D3KooWTest".to_string()));
        assert_eq!(startup_info.network_latency, Some(Duration::from_millis(50)));
    }
}