// Trace output destinations

use crate::lifecycle_tracer::error::OutputError;
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Trait for trace output destinations
pub trait TraceOutput: Send + Sync {
    fn write(&self, formatted_entry: &str) -> Result<(), OutputError>;
    fn flush(&self) -> Result<(), OutputError>;
}

/// Stdout output implementation
pub struct StdoutOutput;

impl TraceOutput for StdoutOutput {
    fn write(&self, formatted_entry: &str) -> Result<(), OutputError> {
        println!("{}", formatted_entry);
        Ok(())
    }

    fn flush(&self) -> Result<(), OutputError> {
        use std::io::Write;
        std::io::stdout()
            .flush()
            .map_err(OutputError::StdoutWriteError)
    }
}

/// File output implementation with buffered writing
pub struct FileOutput {
    #[allow(dead_code)]
    path: PathBuf,
    writer: Arc<Mutex<BufWriter<File>>>,
}

impl FileOutput {
    pub fn new(path: PathBuf) -> Result<Self, OutputError> {
        // Create parent directories if they don't exist
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| OutputError::FileWriteError(e))?;
        }

        // Open file in append mode with create flag
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| OutputError::FileWriteError(e))?;

        // Set file permissions to 0600 (owner read/write only)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = file.metadata()
                .map_err(|e| OutputError::FileWriteError(e))?
                .permissions();
            perms.set_mode(0o600);
            std::fs::set_permissions(&path, perms)
                .map_err(|e| OutputError::FileWriteError(e))?;
        }

        let writer = BufWriter::new(file);

        Ok(Self {
            path,
            writer: Arc::new(Mutex::new(writer)),
        })
    }
}

impl TraceOutput for FileOutput {
    fn write(&self, formatted_entry: &str) -> Result<(), OutputError> {
        let mut writer = self.writer.lock().unwrap();
        writeln!(writer, "{}", formatted_entry)
            .map_err(|e| OutputError::FileWriteError(e))?;
        Ok(())
    }

    fn flush(&self) -> Result<(), OutputError> {
        let mut writer = self.writer.lock().unwrap();
        writer.flush()
            .map_err(|e| OutputError::FileWriteError(e))
    }
}

/// Metrics output implementation (placeholder for Prometheus integration)
pub struct MetricsOutput;

impl TraceOutput for MetricsOutput {
    fn write(&self, _formatted_entry: &str) -> Result<(), OutputError> {
        // TODO: Implement Prometheus metrics integration
        // For now, this is a no-op placeholder
        // In a full implementation, this would:
        // 1. Parse the trace entry
        // 2. Extract relevant metrics (step number, timing, etc.)
        // 3. Update Prometheus counters/gauges/histograms
        Ok(())
    }

    fn flush(&self) -> Result<(), OutputError> {
        // No-op for metrics output
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use tempfile::NamedTempFile;

    #[test]
    fn test_stdout_output() {
        let output = StdoutOutput;
        assert!(output.write("Test message").is_ok());
        assert!(output.flush().is_ok());
    }

    #[test]
    fn test_file_output_creation() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_path_buf();
        
        let output = FileOutput::new(path.clone());
        assert!(output.is_ok());
    }

    #[test]
    fn test_file_output_write() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_path_buf();
        
        let output = FileOutput::new(path.clone()).unwrap();
        assert!(output.write("Test line 1").is_ok());
        assert!(output.write("Test line 2").is_ok());
        assert!(output.flush().is_ok());
        
        // Read the file and verify contents
        let mut contents = String::new();
        let mut file = File::open(&path).unwrap();
        file.read_to_string(&mut contents).unwrap();
        
        assert!(contents.contains("Test line 1"));
        assert!(contents.contains("Test line 2"));
    }

    #[test]
    fn test_file_output_creates_directories() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("subdir").join("trace.log");
        
        let output = FileOutput::new(path.clone());
        assert!(output.is_ok());
        assert!(path.exists());
    }

    #[test]
    fn test_metrics_output() {
        let output = MetricsOutput;
        assert!(output.write("Test metric").is_ok());
        assert!(output.flush().is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn test_file_permissions() {
        use std::os::unix::fs::PermissionsExt;
        
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_path_buf();
        
        let _output = FileOutput::new(path.clone()).unwrap();
        
        let metadata = std::fs::metadata(&path).unwrap();
        let permissions = metadata.permissions();
        let mode = permissions.mode();
        
        // Check that only owner has read/write permissions (0600)
        assert_eq!(mode & 0o777, 0o600);
    }
}
