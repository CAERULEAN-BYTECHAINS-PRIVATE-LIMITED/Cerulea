// Error types for lifecycle tracer

use std::fmt;

/// Main tracer error type
#[derive(Debug)]
pub enum TracerError {
    /// Trace initialization failed
    InitializationFailed(String),
    /// Trace output failed
    OutputError(OutputError),
    /// Trace formatting failed
    FormattingError(String),
    /// Trace buffer overflowed
    BufferOverflow,
    /// Configuration parameter invalid
    ConfigurationError(String),
}

impl fmt::Display for TracerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TracerError::InitializationFailed(msg) => write!(f, "Tracer initialization failed: {}", msg),
            TracerError::OutputError(e) => write!(f, "Output error: {}", e),
            TracerError::FormattingError(msg) => write!(f, "Formatting error: {}", msg),
            TracerError::BufferOverflow => write!(f, "Buffer overflow"),
            TracerError::ConfigurationError(msg) => write!(f, "Configuration error: {}", msg),
        }
    }
}

impl std::error::Error for TracerError {}

/// Output-specific error type
#[derive(Debug)]
pub enum OutputError {
    /// Error writing to file
    FileWriteError(std::io::Error),
    /// Error writing to stdout
    StdoutWriteError(std::io::Error),
    /// Error recording metrics
    MetricsError(String),
}

impl fmt::Display for OutputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OutputError::FileWriteError(e) => write!(f, "File write error: {}", e),
            OutputError::StdoutWriteError(e) => write!(f, "Stdout write error: {}", e),
            OutputError::MetricsError(msg) => write!(f, "Metrics error: {}", msg),
        }
    }
}

impl std::error::Error for OutputError {}

impl From<std::io::Error> for OutputError {
    fn from(error: std::io::Error) -> Self {
        OutputError::FileWriteError(error)
    }
}
