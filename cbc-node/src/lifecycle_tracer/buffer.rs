// Trace buffer for batching log entries

use crate::lifecycle_tracer::output::TraceOutput;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Buffer for trace entries
pub struct TraceBuffer {
    entries: VecDeque<String>,
    max_size: usize,
    last_flush: Instant,
    flush_interval: Duration,
}

impl TraceBuffer {
    /// Create a new trace buffer
    pub fn new(max_size: usize, flush_interval: Duration) -> Self {
        Self {
            entries: VecDeque::with_capacity(max_size),
            max_size,
            last_flush: Instant::now(),
            flush_interval,
        }
    }

    /// Add an entry to the buffer
    pub fn push(&mut self, entry: String) {
        // If buffer is at capacity, remove oldest entry
        if self.entries.len() >= self.max_size {
            self.entries.pop_front();
        }
        
        self.entries.push_back(entry);
    }

    /// Check if the buffer should be flushed
    pub fn should_flush(&self) -> bool {
        // Flush if buffer is full or flush interval has elapsed
        self.entries.len() >= self.max_size || 
        self.last_flush.elapsed() >= self.flush_interval
    }

    /// Flush all entries to the given output
    pub fn flush_to(&mut self, output: &dyn TraceOutput) -> Result<(), std::io::Error> {
        while let Some(entry) = self.entries.pop_front() {
            // Write entry, but continue on error to avoid losing other entries
            if let Err(e) = output.write(&entry) {
                eprintln!("Failed to write trace entry: {}", e);
            }
        }
        
        // Flush the output
        if let Err(e) = output.flush() {
            eprintln!("Failed to flush output: {}", e);
        }
        
        self.last_flush = Instant::now();
        Ok(())
    }

    /// Get the current number of buffered entries
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the buffer is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear all entries from the buffer
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lifecycle_tracer::output::TraceOutput;
    use crate::lifecycle_tracer::error::OutputError;
    use std::sync::{Arc, Mutex};

    // Mock output for testing
    struct MockOutput {
        written: Arc<Mutex<Vec<String>>>,
    }

    impl MockOutput {
        fn new() -> Self {
            Self {
                written: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn get_written(&self) -> Vec<String> {
            self.written.lock().unwrap().clone()
        }
    }

    impl TraceOutput for MockOutput {
        fn write(&self, formatted_entry: &str) -> Result<(), OutputError> {
            self.written.lock().unwrap().push(formatted_entry.to_string());
            Ok(())
        }

        fn flush(&self) -> Result<(), OutputError> {
            Ok(())
        }
    }

    #[test]
    fn test_buffer_push() {
        let mut buffer = TraceBuffer::new(10, Duration::from_millis(100));
        
        buffer.push("Entry 1".to_string());
        buffer.push("Entry 2".to_string());
        
        assert_eq!(buffer.len(), 2);
        assert!(!buffer.is_empty());
    }

    #[test]
    fn test_buffer_overflow() {
        let mut buffer = TraceBuffer::new(3, Duration::from_millis(100));
        
        buffer.push("Entry 1".to_string());
        buffer.push("Entry 2".to_string());
        buffer.push("Entry 3".to_string());
        buffer.push("Entry 4".to_string()); // Should drop Entry 1
        
        assert_eq!(buffer.len(), 3);
    }

    #[test]
    fn test_buffer_should_flush_on_full() {
        let mut buffer = TraceBuffer::new(3, Duration::from_secs(100));
        
        buffer.push("Entry 1".to_string());
        buffer.push("Entry 2".to_string());
        assert!(!buffer.should_flush());
        
        buffer.push("Entry 3".to_string());
        assert!(buffer.should_flush());
    }

    #[test]
    fn test_buffer_should_flush_on_interval() {
        let mut buffer = TraceBuffer::new(100, Duration::from_millis(10));
        
        buffer.push("Entry 1".to_string());
        assert!(!buffer.should_flush());
        
        std::thread::sleep(Duration::from_millis(15));
        assert!(buffer.should_flush());
    }

    #[test]
    fn test_buffer_flush_to() {
        let mut buffer = TraceBuffer::new(10, Duration::from_millis(100));
        let output = MockOutput::new();
        
        buffer.push("Entry 1".to_string());
        buffer.push("Entry 2".to_string());
        buffer.push("Entry 3".to_string());
        
        assert!(buffer.flush_to(&output).is_ok());
        assert_eq!(buffer.len(), 0);
        
        let written = output.get_written();
        assert_eq!(written.len(), 3);
        assert_eq!(written[0], "Entry 1");
        assert_eq!(written[1], "Entry 2");
        assert_eq!(written[2], "Entry 3");
    }

    #[test]
    fn test_buffer_clear() {
        let mut buffer = TraceBuffer::new(10, Duration::from_millis(100));
        
        buffer.push("Entry 1".to_string());
        buffer.push("Entry 2".to_string());
        
        assert_eq!(buffer.len(), 2);
        
        buffer.clear();
        assert_eq!(buffer.len(), 0);
        assert!(buffer.is_empty());
    }

    #[test]
    fn test_buffer_maintains_order() {
        let mut buffer = TraceBuffer::new(10, Duration::from_millis(100));
        let output = MockOutput::new();
        
        for i in 1..=5 {
            buffer.push(format!("Entry {}", i));
        }
        
        buffer.flush_to(&output).unwrap();
        
        let written = output.get_written();
        for i in 1..=5 {
            assert_eq!(written[i - 1], format!("Entry {}", i));
        }
    }
}
