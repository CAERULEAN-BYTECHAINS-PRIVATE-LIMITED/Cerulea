// Lifecycle Tracer Module
// Provides comprehensive tracing of blockchain node lifecycle from initialization to block production

use std::sync::{Arc, Mutex};
use std::sync::atomic::AtomicU32;
use std::time::{Duration, SystemTime};

// Submodules
mod config;
mod formatter;
mod output;
mod buffer;
mod entry;
mod error;

// Re-exports for public API
pub use config::{TracerConfig, TraceFormat, OutputDestination};
pub use formatter::{TraceFormatter, JsonFormatter, HumanReadableFormatter};
pub use output::{TraceOutput, StdoutOutput, FileOutput, MetricsOutput};
pub use buffer::TraceBuffer;
pub use entry::{TraceEntry, TraceEntryType, TraceMetadata};
pub use error::{TracerError, OutputError};

/// Global lifecycle tracer instance
static GLOBAL_TRACER: once_cell::sync::Lazy<LifecycleTracer> = once_cell::sync::Lazy::new(|| {
    // Default configuration - will be overridden by init()
    LifecycleTracer::new(TracerConfig::default())
});

/// Main lifecycle tracer structure
pub struct LifecycleTracer {
    config: Arc<Mutex<TracerConfig>>,
    formatter: Arc<Mutex<Box<dyn TraceFormatter>>>,
    outputs: Arc<Mutex<Vec<Box<dyn TraceOutput>>>>,
    buffer: Arc<Mutex<TraceBuffer>>,
    #[allow(dead_code)]
    step_counter: Arc<AtomicU32>,
}

impl LifecycleTracer {
    /// Create a new lifecycle tracer with the given configuration
    fn new(config: TracerConfig) -> Self {
        let formatter: Box<dyn TraceFormatter> = match config.format {
            TraceFormat::Json => Box::new(JsonFormatter),
            TraceFormat::HumanReadable => Box::new(HumanReadableFormatter),
        };

        let mut outputs: Vec<Box<dyn TraceOutput>> = Vec::new();
        for dest in &config.output_destinations {
            match dest {
                OutputDestination::Stdout => outputs.push(Box::new(StdoutOutput)),
                OutputDestination::File(path) => {
                    match FileOutput::new(path.clone()) {
                        Ok(output) => outputs.push(Box::new(output)),
                        Err(e) => eprintln!("Failed to create file output: {}", e),
                    }
                }
                OutputDestination::Metrics => outputs.push(Box::new(MetricsOutput)),
            }
        }

        let buffer = TraceBuffer::new(
            config.buffer_size,
            Duration::from_millis(config.flush_interval_ms),
        );

        Self {
            config: Arc::new(Mutex::new(config)),
            formatter: Arc::new(Mutex::new(formatter)),
            outputs: Arc::new(Mutex::new(outputs)),
            buffer: Arc::new(Mutex::new(buffer)),
            step_counter: Arc::new(AtomicU32::new(0)),
        }
    }

    /// Initialize the global tracer with custom configuration
    pub fn init(config: TracerConfig) -> Result<(), TracerError> {
        // Force initialization of the lazy static with new config
        // Note: This only works on first call, subsequent calls won't change config
        let tracer = Self::global();
        
        // Update the configuration
        *tracer.config.lock().unwrap() = config.clone();
        
        // Update formatter based on new config
        let new_formatter: Box<dyn TraceFormatter> = match config.format {
            TraceFormat::Json => Box::new(JsonFormatter),
            TraceFormat::HumanReadable => Box::new(HumanReadableFormatter),
        };
        *tracer.formatter.lock().unwrap() = new_formatter;
        
        // Update outputs based on new config
        let mut new_outputs: Vec<Box<dyn TraceOutput>> = Vec::new();
        for dest in &config.output_destinations {
            match dest {
                OutputDestination::Stdout => new_outputs.push(Box::new(StdoutOutput)),
                OutputDestination::File(path) => {
                    match FileOutput::new(path.clone()) {
                        Ok(output) => new_outputs.push(Box::new(output)),
                        Err(e) => return Err(TracerError::InitializationFailed(
                            format!("Failed to create file output: {}", e)
                        )),
                    }
                }
                OutputDestination::Metrics => new_outputs.push(Box::new(MetricsOutput)),
            }
        }
        
        *tracer.outputs.lock().unwrap() = new_outputs;
        
        Ok(())
    }

    /// Get the global tracer instance
    pub fn global() -> &'static LifecycleTracer {
        &GLOBAL_TRACER
    }

    /// Log a numbered trace step
    pub fn trace_step(&self, step: u32, location: &str, message: &str, metadata: Option<TraceMetadata>) {
        let config = self.config.lock().unwrap();
        
        if !config.enabled {
            return;
        }
        
        if config.milestone_only {
            return; // Only log milestones when milestone_only is enabled
        }
        
        drop(config); // Release lock early
        
        let entry = TraceEntry {
            step: Some(step),
            location: location.to_string(),
            message: message.to_string(),
            timestamp: SystemTime::now(),
            thread_id: if self.config.lock().unwrap().include_thread_id {
                Some(std::thread::current().id())
            } else {
                None
            },
            metadata,
            entry_type: TraceEntryType::Step,
        };
        
        self.log_entry(entry);
    }

    /// Log a milestone event
    pub fn trace_milestone(&self, message: &str, metadata: Option<TraceMetadata>) {
        let config = self.config.lock().unwrap();
        
        if !config.enabled {
            return;
        }
        
        drop(config); // Release lock early
        
        let entry = TraceEntry {
            step: None,
            location: "milestone".to_string(),
            message: message.to_string(),
            timestamp: SystemTime::now(),
            thread_id: if self.config.lock().unwrap().include_thread_id {
                Some(std::thread::current().id())
            } else {
                None
            },
            metadata,
            entry_type: TraceEntryType::Milestone,
        };
        
        self.log_entry(entry);
    }

    /// Log an error at a specific step
    pub fn trace_error(&self, step: u32, location: &str, error: &dyn std::error::Error, context: &str) {
        let config = self.config.lock().unwrap();
        
        if !config.enabled {
            return;
        }
        
        drop(config); // Release lock early
        
        let error_message = format!("{}: {} - {}", context, error, std::backtrace::Backtrace::capture());
        
        let entry = TraceEntry {
            step: Some(step),
            location: location.to_string(),
            message: error_message,
            timestamp: SystemTime::now(),
            thread_id: if self.config.lock().unwrap().include_thread_id {
                Some(std::thread::current().id())
            } else {
                None
            },
            metadata: None,
            entry_type: TraceEntryType::Error,
        };
        
        self.log_entry(entry);
    }

    /// Internal method to log an entry
    fn log_entry(&self, entry: TraceEntry) {
        let formatted = self.formatter.lock().unwrap().format(&entry);
        
        // Add to buffer
        let mut buffer = self.buffer.lock().unwrap();
        buffer.push(formatted.clone());
        
        // Check if we should flush
        if buffer.should_flush() {
            self.flush_buffer(&mut buffer);
        }
    }

    /// Flush all buffered traces
    pub fn flush(&self) {
        let mut buffer = self.buffer.lock().unwrap();
        self.flush_buffer(&mut buffer);
    }

    /// Internal flush implementation
    fn flush_buffer(&self, buffer: &mut TraceBuffer) {
        let outputs = self.outputs.lock().unwrap();
        
        for output in outputs.iter() {
            if let Err(e) = buffer.flush_to(output.as_ref()) {
                eprintln!("Failed to flush buffer to output: {}", e);
            }
        }
    }
}

// Implement Send and Sync for LifecycleTracer
unsafe impl Send for LifecycleTracer {}
unsafe impl Sync for LifecycleTracer {}
