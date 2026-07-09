use std::process::Command;

#[test]
fn test_fork_check_cli_help() {
    let output = Command::new("cargo")
        .args(&["run", "--bin", "cerulea-node", "--", "fork-check", "--help"])
        .output()
        .expect("Failed to execute command");
    
    assert!(output.status.success(), "Command should succeed");
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Fork detection tool"), "Should contain fork detection description");
    assert!(stdout.contains("--local-rpc"), "Should contain local-rpc option");
    assert!(stdout.contains("--peer-rpc"), "Should contain peer-rpc option");
    assert!(stdout.contains("--threshold"), "Should contain threshold option");
    assert!(stdout.contains("--format"), "Should contain format option");
    assert!(stdout.contains("--timeout"), "Should contain timeout option");
}

#[test]
fn test_fork_check_cli_validation() {
    // Test that fork-check command exists and can be invoked
    let output = Command::new("cargo")
        .args(&["run", "--bin", "cerulea-node", "--", "fork-check", "--help"])
        .output()
        .expect("Failed to execute command");
    
    // The help command should succeed
    assert!(output.status.success(), "Fork-check help command should succeed");
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Fork detection tool"), "Should contain fork detection description");
}

#[test]
fn test_standalone_fork_checker_help() {
    let output = Command::new("cargo")
        .args(&["run", "--bin", "fork-checker", "--", "--help"])
        .current_dir("../tools")
        .output()
        .expect("Failed to execute command");
    
    assert!(output.status.success(), "Command should succeed");
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Cerulea Fork Detection Tool"), "Should contain tool description");
    assert!(stdout.contains("check"), "Should contain check subcommand");
    assert!(stdout.contains("serve"), "Should contain serve subcommand");
}