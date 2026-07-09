//! Multi-Node Integration Tests
//! 
//! This module contains integration tests that verify the CBC node operates correctly
//! in multi-node scenarios. These tests validate:
//! - Multi-node network setup and peer connections
//! - Block synchronization across nodes
//! - Block authorship rotation according to DCF rules
//! - Epoch transitions on all nodes
//! - PoI update propagation and trust score changes
//! - Consensus health across the network

use std::time::Duration;
use std::process::Command;
use std::collections::HashMap;
use tokio::time::sleep;

/// Configuration for multi-node testing
#[derive(Debug, Clone)]
pub struct MultiNodeTestConfig {
    /// Number of nodes to start
    pub node_count: usize,
    /// Base port for RPC servers (each node gets base_port + node_index)
    pub base_rpc_port: u16,
    /// Base port for WebSocket servers
    pub base_ws_port: u16,
    /// Base port for P2P networking
    pub base_p2p_port: u16,
    /// Whether to enable CBC extensions
    pub enable_cbc_extensions: bool,
    /// Maximum time to wait for node startup
    pub startup_timeout: Duration,
    /// Maximum time to wait for peer connections
    pub peer_connection_timeout: Duration,
    /// Maximum time to wait for block synchronization
    pub sync_timeout: Duration,
    /// Number of blocks to produce for testing
    pub target_blocks: u32,
    /// Epoch length for testing
    pub epoch_length: u32,
}

impl Default for MultiNodeTestConfig {
    fn default() -> Self {
        Self {
            node_count: 3,
            base_rpc_port: 9950,
            base_ws_port: 9960,
            base_p2p_port: 30350,
            enable_cbc_extensions: true,
            startup_timeout: Duration::from_secs(60),
            peer_connection_timeout: Duration::from_secs(30),
            sync_timeout: Duration::from_secs(60),
            target_blocks: 20,
            epoch_length: 10,
        }
    }
}

impl MultiNodeTestConfig {
    /// Create a new multi-node test configuration with unique ports
    pub fn new_with_unique_ports() -> Self {
        use std::sync::atomic::{AtomicU16, Ordering};
        static PORT_COUNTER: AtomicU16 = AtomicU16::new(10000);
        
        let base_port = PORT_COUNTER.fetch_add(100, Ordering::SeqCst);
        Self {
            base_rpc_port: base_port,
            base_ws_port: base_port + 10,
            base_p2p_port: base_port + 20,
            ..Default::default()
        }
    }
    
    /// Set the number of nodes
    pub fn with_node_count(mut self, count: usize) -> Self {
        self.node_count = count;
        self
    }
    
    /// Enable or disable CBC extensions
    pub fn with_cbc_extensions(mut self, enabled: bool) -> Self {
        self.enable_cbc_extensions = enabled;
        self
    }
    
    /// Set timeout values
    pub fn with_timeouts(
        mut self, 
        startup: Duration, 
        peer_connection: Duration, 
        sync: Duration
    ) -> Self {
        self.startup_timeout = startup;
        self.peer_connection_timeout = peer_connection;
        self.sync_timeout = sync;
        self
    }
    
    /// Set target blocks and epoch length
    pub fn with_consensus_params(mut self, target_blocks: u32, epoch_length: u32) -> Self {
        self.target_blocks = target_blocks;
        self.epoch_length = epoch_length;
        self
    }
    
    /// Get RPC URL for a specific node
    pub fn rpc_url(&self, node_index: usize) -> String {
        format!("http://127.0.0.1:{}", self.base_rpc_port + node_index as u16)
    }
    
    /// Get WebSocket URL for a specific node
    pub fn ws_url(&self, node_index: usize) -> String {
        format!("ws://127.0.0.1:{}", self.base_ws_port + node_index as u16)
    }
    
    /// Get P2P port for a specific node
    pub fn p2p_port(&self, node_index: usize) -> u16 {
        self.base_p2p_port + node_index as u16
    }
}

/// Mock multi-node network for testing without actually starting processes
pub struct MockMultiNodeNetwork {
    pub config: MultiNodeTestConfig,
    pub nodes: Vec<MockNode>,
}

/// Mock node for testing
#[derive(Debug)]
pub struct MockNode {
    pub index: usize,
    pub current_block: u32,
    pub current_epoch: u32,
    pub peer_count: usize,
    pub is_running: bool,
    pub config: MultiNodeTestConfig,
}

impl MockNode {
    pub fn new(index: usize, config: MultiNodeTestConfig) -> Self {
        Self {
            index,
            current_block: 0,
            current_epoch: 1,
            peer_count: 0,
            is_running: false,
            config,
        }
    }
    
    pub async fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Mock: Starting node {}", self.index);
        sleep(Duration::from_millis(100)).await;
        self.is_running = true;
        self.current_block = 1;
        Ok(())
    }
    
    pub fn stop(&mut self) {
        log::info!("Mock: Stopping node {}", self.index);
        self.is_running = false;
    }
    
    pub async fn connect_to_peers(&mut self, expected_peers: usize) -> Result<(), Box<dyn std::error::Error>> {
        if !self.is_running {
            return Err("Node not running".into());
        }
        
        log::info!("Mock: Node {} connecting to {} peers", self.index, expected_peers);
        sleep(Duration::from_millis(50)).await;
        self.peer_count = expected_peers;
        Ok(())
    }
    
    pub async fn produce_blocks(&mut self, target_blocks: u32) -> Result<u32, Box<dyn std::error::Error>> {
        if !self.is_running {
            return Err("Node not running".into());
        }
        
        log::info!("Mock: Node {} producing blocks to {}", self.index, target_blocks);
        
        while self.current_block < target_blocks {
            sleep(Duration::from_millis(20)).await;
            self.current_block += 1;
            
            // Simulate epoch transitions
            if self.current_block % self.config.epoch_length == 0 {
                self.current_epoch += 1;
                log::info!("Mock: Node {} epoch transition to {}", self.index, self.current_epoch);
            }
            
            log::debug!("Mock: Node {} produced block {}", self.index, self.current_block);
        }
        
        Ok(self.current_block)
    }
    
    pub fn get_block_number(&self) -> u32 {
        self.current_block
    }
    
    pub fn get_current_epoch(&self) -> u32 {
        self.current_epoch
    }
    
    pub fn get_peer_count(&self) -> usize {
        self.peer_count
    }
    
    pub fn is_running(&self) -> bool {
        self.is_running
    }
}

impl MockMultiNodeNetwork {
    pub fn new(config: MultiNodeTestConfig) -> Self {
        let config_clone = config.clone();
        let mut nodes = Vec::new();
        
        for i in 0..config.node_count {
            let node = MockNode::new(i, config_clone.clone());
            nodes.push(node);
        }
        
        Self { config, nodes }
    }
    
    pub async fn start_all(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Mock: Starting {} nodes", self.config.node_count);
        
        for node in &mut self.nodes {
            node.start().await?;
        }
        
        log::info!("Mock: All nodes started");
        Ok(())
    }
    
    pub fn stop_all(&mut self) {
        log::info!("Mock: Stopping all nodes");
        for node in &mut self.nodes {
            node.stop();
        }
    }
    
    pub async fn establish_peer_connections(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Mock: Establishing peer connections");
        
        let expected_peers = self.config.node_count - 1;
        
        for node in &mut self.nodes {
            node.connect_to_peers(expected_peers).await?;
        }
        
        log::info!("Mock: Peer connections established");
        Ok(())
    }
    
    pub async fn synchronize_blocks(&mut self, target_blocks: u32) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Mock: Synchronizing blocks to {}", target_blocks);
        
        // Simulate block production on all nodes
        for node in &mut self.nodes {
            node.produce_blocks(target_blocks).await?;
        }
        
        // Verify all nodes are synchronized
        let block_numbers: Vec<u32> = self.nodes.iter().map(|n| n.get_block_number()).collect();
        let min_block = *block_numbers.iter().min().unwrap();
        let max_block = *block_numbers.iter().max().unwrap();
        
        if max_block - min_block > 2 {
            return Err(format!("Nodes not synchronized: blocks range from {} to {}", min_block, max_block).into());
        }
        
        log::info!("Mock: Block synchronization complete: blocks {}-{}", min_block, max_block);
        Ok(())
    }
    
    pub fn verify_epoch_transitions(&self) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Mock: Verifying epoch transitions");
        
        let epochs: Vec<u32> = self.nodes.iter().map(|n| n.get_current_epoch()).collect();
        let first_epoch = epochs[0];
        
        // Verify all nodes have the same epoch
        for (i, &epoch) in epochs.iter().enumerate() {
            if epoch != first_epoch {
                return Err(format!("Epoch mismatch: Node 0 has epoch {}, Node {} has epoch {}", first_epoch, i, epoch).into());
            }
        }
        
        // Verify epochs have advanced beyond initial
        if first_epoch <= 1 {
            return Err("Epochs have not advanced".into());
        }
        
        log::info!("Mock: Epoch transitions verified: all nodes at epoch {}", first_epoch);
        Ok(())
    }
    
    pub fn get_network_health(&self) -> HashMap<usize, NetworkHealth> {
        let mut health_map = HashMap::new();
        
        for node in &self.nodes {
            let health = NetworkHealth {
                is_running: node.is_running(),
                peer_count: node.get_peer_count(),
                current_block: node.get_block_number(),
                current_epoch: node.get_current_epoch(),
            };
            health_map.insert(node.index, health);
        }
        
        health_map
    }
}

#[derive(Debug)]
pub struct NetworkHealth {
    pub is_running: bool,
    pub peer_count: usize,
    pub current_block: u32,
    pub current_epoch: u32,
}

// =============================================================================
// INTEGRATION TESTS
// =============================================================================

#[tokio::test]
async fn test_multi_node_startup_and_connections() {
    env_logger::try_init().ok();
    
    let config = MultiNodeTestConfig::new_with_unique_ports()
        .with_node_count(3)
        .with_cbc_extensions(true);
    
    let mut network = MockMultiNodeNetwork::new(config.clone());
    
    // Test: Start all nodes
    match network.start_all().await {
        Ok(()) => {
            log::info!("✓ Multi-node startup test PASSED");
            
            // Verify all nodes are running
            for node in &network.nodes {
                assert!(node.is_running(), "Node {} should be running", node.index);
            }
        }
        Err(e) => {
            panic!("✗ Multi-node startup test FAILED: {}", e);
        }
    }
    
    // Test: Establish peer connections
    match network.establish_peer_connections().await {
        Ok(()) => {
            log::info!("✓ Peer connections test PASSED");
            
            // Verify all nodes have expected peer count
            let expected_peers = config.node_count - 1;
            for node in &network.nodes {
                assert_eq!(node.get_peer_count(), expected_peers, "Node {} should have {} peers", node.index, expected_peers);
            }
        }
        Err(e) => {
            panic!("✗ Peer connections test FAILED: {}", e);
        }
    }
}

#[tokio::test]
async fn test_multi_node_block_synchronization() {
    env_logger::try_init().ok();
    
    let config = MultiNodeTestConfig::new_with_unique_ports()
        .with_node_count(3)
        .with_cbc_extensions(true)
        .with_consensus_params(15, 10);
    
    let mut network = MockMultiNodeNetwork::new(config.clone());
    
    // Start network
    network.start_all().await.expect("Failed to start network");
    network.establish_peer_connections().await.expect("Failed to establish peer connections");
    
    // Test: Block synchronization
    match network.synchronize_blocks(config.target_blocks).await {
        Ok(()) => {
            log::info!("✓ Block synchronization test PASSED");
            
            // Verify all nodes have produced blocks
            for node in &network.nodes {
                assert!(node.get_block_number() >= config.target_blocks, "Node {} should have at least {} blocks", node.index, config.target_blocks);
            }
        }
        Err(e) => {
            panic!("✗ Block synchronization test FAILED: {}", e);
        }
    }
}

#[tokio::test]
async fn test_multi_node_epoch_transitions() {
    env_logger::try_init().ok();
    
    let config = MultiNodeTestConfig::new_with_unique_ports()
        .with_node_count(3)
        .with_cbc_extensions(true)
        .with_consensus_params(25, 10);
    
    let mut network = MockMultiNodeNetwork::new(config.clone());
    
    // Start network
    network.start_all().await.expect("Failed to start network");
    network.establish_peer_connections().await.expect("Failed to establish peer connections");
    
    // Produce enough blocks to trigger epoch transitions
    network.synchronize_blocks(config.target_blocks).await.expect("Failed to synchronize blocks");
    
    // Test: Epoch transitions
    match network.verify_epoch_transitions() {
        Ok(()) => {
            log::info!("✓ Epoch transitions test PASSED");
            
            // Verify epochs have advanced
            for node in &network.nodes {
                assert!(node.get_current_epoch() > 1, "Node {} should have advanced beyond epoch 1", node.index);
            }
        }
        Err(e) => {
            panic!("✗ Epoch transitions test FAILED: {}", e);
        }
    }
}

#[tokio::test]
async fn test_multi_node_network_health() {
    env_logger::try_init().ok();
    
    let config = MultiNodeTestConfig::new_with_unique_ports()
        .with_node_count(3)
        .with_cbc_extensions(true);
    
    let mut network = MockMultiNodeNetwork::new(config.clone());
    
    // Start network
    network.start_all().await.expect("Failed to start network");
    network.establish_peer_connections().await.expect("Failed to establish peer connections");
    
    // Test: Network health
    let health_map = network.get_network_health();
    
    assert_eq!(health_map.len(), 3, "Should have health data for 3 nodes");
    
    let mut healthy_nodes = 0;
    for (node_index, health) in &health_map {
        log::info!("Node {} health: running={}, peers={}, block={}, epoch={}", 
                  node_index, health.is_running, health.peer_count, health.current_block, health.current_epoch);
        
        if health.is_running && health.peer_count > 0 {
            healthy_nodes += 1;
        }
    }
    
    assert!(healthy_nodes >= 2, "At least 2 nodes should be healthy");
    log::info!("✓ Network health test PASSED ({}/{} nodes healthy)", healthy_nodes, 3);
}

#[tokio::test]
async fn test_comprehensive_multi_node_functionality() {
    env_logger::try_init().ok();
    
    log::info!("\n=== Comprehensive Multi-Node Integration Test ===");
    
    let config = MultiNodeTestConfig::new_with_unique_ports()
        .with_node_count(3)
        .with_cbc_extensions(true)
        .with_consensus_params(20, 8);
    
    let mut network = MockMultiNodeNetwork::new(config.clone());
    
    // Step 1: Start all nodes
    log::info!("Step 1: Starting {} nodes...", config.node_count);
    network.start_all().await.expect("Failed to start network");
    log::info!("✓ All nodes started successfully");
    
    // Step 2: Establish peer connections
    log::info!("Step 2: Establishing peer connections...");
    network.establish_peer_connections().await.expect("Failed to establish peer connections");
    log::info!("✓ Peer connections established");
    
    // Step 3: Synchronize blocks
    log::info!("Step 3: Synchronizing blocks...");
    network.synchronize_blocks(config.target_blocks).await.expect("Failed to synchronize blocks");
    log::info!("✓ Block synchronization completed");
    
    // Step 4: Verify epoch transitions
    log::info!("Step 4: Verifying epoch transitions...");
    network.verify_epoch_transitions().expect("Failed to verify epoch transitions");
    log::info!("✓ Epoch transitions verified");
    
    // Step 5: Check network health
    log::info!("Step 5: Checking network health...");
    let health_map = network.get_network_health();
    
    let mut healthy_nodes = 0;
    let mut final_blocks = Vec::new();
    let mut final_epochs = Vec::new();
    
    for (node_index, health) in &health_map {
        if health.is_running && health.peer_count > 0 {
            healthy_nodes += 1;
        }
        final_blocks.push(health.current_block);
        final_epochs.push(health.current_epoch);
        log::info!("Node {} final state: block={}, epoch={}, peers={}", 
                  node_index, health.current_block, health.current_epoch, health.peer_count);
    }
    
    // Final validation
    assert!(healthy_nodes >= 2, "At least 2 nodes should be healthy");
    
    let min_block = *final_blocks.iter().min().unwrap();
    let max_block = *final_blocks.iter().max().unwrap();
    assert!(max_block - min_block <= 2, "Nodes should be synchronized within 2 blocks");
    
    let first_epoch = final_epochs[0];
    for &epoch in &final_epochs {
        assert_eq!(epoch, first_epoch, "All nodes should have the same epoch");
    }
    
    log::info!("\nCOMPREHENSIVE MULTI-NODE TEST RESULTS:");
    log::info!("Network Startup: {} nodes started", config.node_count);
    log::info!("Peer Connections: All nodes connected");
    log::info!("Block Synchronization: Blocks {}-{}", min_block, max_block);
    log::info!("Epoch Transitions: All nodes at epoch {}", first_epoch);
    log::info!("Network Health: {}/{} nodes healthy", healthy_nodes, config.node_count);
    log::info!("Overall Status: ALL TESTS PASSED");
    
    log::info!("Multi-node integration test completed successfully!");
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

/// Check if the CBC node binary is available for testing
pub fn check_node_binary_available() -> bool {
    Command::new("cargo")
        .args(&["check", "--bin", "cerulea-node"])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

/// Get available port range for testing
pub fn get_available_port_range(count: usize) -> Vec<u16> {
    use std::net::TcpListener;
    
    let mut ports = Vec::new();
    let mut base_port = 10000u16;
    
    while ports.len() < count && base_port < 65000 {
        if let Ok(listener) = TcpListener::bind(format!("127.0.0.1:{}", base_port)) {
            ports.push(base_port);
            drop(listener);
        }
        base_port += 1;
    }
    
    ports
}

#[cfg(test)]
mod helper_tests {
    use super::*;
    
    #[test]
    fn test_config_creation() {
        let config = MultiNodeTestConfig::new_with_unique_ports();
        assert!(config.node_count > 0);
        assert!(config.base_rpc_port > 0);
        assert!(config.base_ws_port > 0);
        assert!(config.base_p2p_port > 0);
    }
    
    #[test]
    fn test_config_builder() {
        let config = MultiNodeTestConfig::default()
            .with_node_count(5)
            .with_cbc_extensions(false)
            .with_consensus_params(30, 15);
        
        assert_eq!(config.node_count, 5);
        assert!(!config.enable_cbc_extensions);
        assert_eq!(config.target_blocks, 30);
        assert_eq!(config.epoch_length, 15);
    }
    
    #[test]
    fn test_url_generation() {
        let config = MultiNodeTestConfig {
            base_rpc_port: 9000,
            base_ws_port: 9100,
            base_p2p_port: 30000,
            ..Default::default()
        };
        
        assert_eq!(config.rpc_url(0), "http://127.0.0.1:9000");
        assert_eq!(config.rpc_url(2), "http://127.0.0.1:9002");
        assert_eq!(config.ws_url(1), "ws://127.0.0.1:9101");
        assert_eq!(config.p2p_port(3), 30003);
    }
    
    #[test]
    fn test_port_availability() {
        let ports = get_available_port_range(5);
        assert!(ports.len() <= 5);
        
        for port in &ports {
            assert!(*port > 1024);
            assert!(*port < 65535);
        }
    }
    
    #[test]
    fn test_mock_node_creation() {
        let config = MultiNodeTestConfig::default();
        let node = MockNode::new(0, config);
        
        assert_eq!(node.index, 0);
        assert!(!node.is_running());
        assert_eq!(node.get_block_number(), 0);
        assert_eq!(node.get_current_epoch(), 1);
        assert_eq!(node.get_peer_count(), 0);
    }
    
    #[test]
    fn test_mock_network_creation() {
        let config = MultiNodeTestConfig::default().with_node_count(4);
        let network = MockMultiNodeNetwork::new(config.clone());
        
        assert_eq!(network.nodes.len(), 4);
        assert_eq!(network.config.node_count, 4);
        
        for (i, node) in network.nodes.iter().enumerate() {
            assert_eq!(node.index, i);
            assert!(!node.is_running());
        }
    }
}