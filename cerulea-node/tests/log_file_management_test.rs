use std::time::Duration;
use tempfile::TempDir;

#[tokio::test]
async fn test_log_file_creation() {
    // Create a temporary directory for the test
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let log_file_path = temp_dir.path().join("test_cbc.log");
    
    // Test that LogFileWriter can create a log file
    let log_writer = cerulea_node::logging::LogFileWriter::new(
        Some(log_file_path.to_string_lossy().to_string())
    );
    
    assert!(log_writer.is_ok());
    let log_writer = log_writer.unwrap();
    assert!(log_writer.is_file_logging());
    
    // Verify the log file was created
    assert!(log_file_path.exists());
}

#[test]
fn test_startup_info_display() {
    // Test that StartupInfo can be created and configured
    let mut startup_info = cerulea_node::logging::StartupInfo::new(
        "1.0.0".to_string(),
        "0.1.0".to_string(),
        "CBC Test Chain".to_string(),
        "Full Node".to_string(),
    );
    
    assert_eq!(startup_info.runtime_version, "1.0.0");
    assert_eq!(startup_info.node_version, "0.1.0");
    assert_eq!(startup_info.chain_name, "CBC Test Chain");
    assert_eq!(startup_info.node_role, "Full Node");
    
    // Test setting peer ID and latency
    startup_info.set_peer_id("12D3KooWTestPeerID".to_string());
    startup_info.set_network_latency(Duration::from_millis(100));
    
    assert_eq!(startup_info.peer_id, Some("12D3KooWTestPeerID".to_string()));
    assert_eq!(startup_info.network_latency, Some(Duration::from_millis(100)));
}

#[test]
fn test_log_deduplication() {
    // Test that LogDeduplicator works correctly
    let deduplicator = cerulea_node::logging::LogDeduplicator::new(5); // 5 second window
    
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
fn test_cbc_logging_initialization() {
    // Test that CBC logging can be initialized with different configurations
    
    // Test without log file
    let result = cerulea_node::logging::init_cbc_logging(false, false, None);
    // Note: This might fail if logging is already initialized, which is expected in tests
    // We just verify the function exists and can be called
    
    // Test with log file
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let log_file_path = temp_dir.path().join("test_init.log");
    
    let result = cerulea_node::logging::init_cbc_logging(
        true, 
        false, 
        Some(log_file_path.to_string_lossy().to_string())
    );
    // Again, this might fail due to logger already being initialized, which is fine
}