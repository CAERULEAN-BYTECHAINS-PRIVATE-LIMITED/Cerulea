use std::process::Command;

// The node binary the test drives.
//
// Cargo builds the binary for this test target and hands the path over, so
// the test runs the artefact the build just produced. Starting a second cargo
// here would build a second profile from cold instead.
const NODE_BINARY: &str = env!("CARGO_BIN_EXE_cerulea-node");

#[test]
fn test_fork_check_cli_help() {
    let output = Command::new(NODE_BINARY)
        .args(["fork-check", "--help"])
        .output()
        .expect("the node binary answers the help request");

    assert!(output.status.success(), "Command should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Fork detection tool"),
        "Should contain fork detection description"
    );
    assert!(
        stdout.contains("--local-rpc"),
        "Should contain local-rpc option"
    );
    assert!(
        stdout.contains("--peer-rpc"),
        "Should contain peer-rpc option"
    );
    assert!(
        stdout.contains("--threshold"),
        "Should contain threshold option"
    );
    assert!(stdout.contains("--format"), "Should contain format option");
    assert!(
        stdout.contains("--timeout"),
        "Should contain timeout option"
    );
}

#[test]
fn test_fork_check_cli_validation() {
    // Test that fork-check command exists and can be invoked
    let output = Command::new(NODE_BINARY)
        .args(["fork-check", "--help"])
        .output()
        .expect("the node binary answers the help request");

    // The help command should succeed
    assert!(
        output.status.success(),
        "Fork-check help command should succeed"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Fork detection tool"),
        "Should contain fork detection description"
    );
}

#[test]
fn test_standalone_fork_checker_help() {
    // The fork checker is a binary of the tools package, so cargo builds it
    // on demand here. The build is small and the artefacts are already warm.
    let output = Command::new("cargo")
        .args(["run", "--quiet", "--bin", "fork-checker", "--", "--help"])
        .current_dir("../tools")
        .output()
        .expect("the fork checker binary answers the help request");

    assert!(output.status.success(), "Command should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Cerulea Fork Detection Tool"),
        "Should contain tool description"
    );
    assert!(stdout.contains("check"), "Should contain check subcommand");
    assert!(stdout.contains("serve"), "Should contain serve subcommand");
}
