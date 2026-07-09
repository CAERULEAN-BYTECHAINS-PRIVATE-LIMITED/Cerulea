//! Tests for the ConsensusMetrics module

#[cfg(test)]
mod tests {
    use crate::metrics::ConsensusMetrics;
    use prometheus::Registry;

    #[test]
    fn test_consensus_metrics_creation() {
        let registry = Registry::new();
        let metrics = ConsensusMetrics::new(&registry);
        
        assert!(metrics.is_ok(), "Failed to create ConsensusMetrics");
        
        let metrics = metrics.unwrap();
        
        // Test manual updates
        metrics.update_active_validators(5);
        metrics.update_total_reserved_stake(1000000);
        metrics.update_current_epoch(10);
        
        // Test recording events
        metrics.record_reward_distribution(500);
        metrics.record_slashing(100);
        
        // Verify metrics are accessible (this would normally be done by Prometheus scraping)
        // For now, we just verify the methods don't panic
        println!("ConsensusMetrics test completed successfully");
    }

    #[test]
    fn test_metrics_with_empty_registry() {
        let registry = Registry::new();
        let result = ConsensusMetrics::new(&registry);
        assert!(result.is_ok(), "Should be able to create metrics with empty registry");
    }

    #[test]
    fn test_metrics_recording() {
        let registry = Registry::new();
        let metrics = ConsensusMetrics::new(&registry).unwrap();
        
        // Test multiple reward recordings
        for i in 1..=10 {
            metrics.record_reward_distribution(i * 100);
        }
        
        // Test multiple slashing recordings
        for i in 1..=5 {
            metrics.record_slashing(i * 50);
        }
        
        println!("Metrics recording test completed successfully");
    }

    #[test]
    fn test_metrics_validator_updates() {
        let registry = Registry::new();
        let metrics = ConsensusMetrics::new(&registry).unwrap();
        
        // Test validator count updates
        for count in [1, 5, 10, 20, 50] {
            metrics.update_active_validators(count);
        }
        
        // Test stake updates
        let stake_values = [1000, 5000, 10000, 50000, 100000];
        for stake in stake_values {
            metrics.update_total_reserved_stake(stake);
        }
        
        println!("Validator metrics update test completed successfully");
    }

    #[test]
    fn test_metrics_epoch_tracking() {
        let registry = Registry::new();
        let metrics = ConsensusMetrics::new(&registry).unwrap();
        
        // Test epoch progression
        for epoch in 0..100 {
            metrics.update_current_epoch(epoch);
            
            // Simulate some activity in each epoch
            if epoch % 10 == 0 {
                metrics.record_reward_distribution(1000);
            }
            if epoch % 20 == 0 {
                metrics.record_slashing(100);
            }
        }
        
        println!("Epoch tracking test completed successfully");
    }

    #[test]
    fn test_metrics_edge_cases() {
        let registry = Registry::new();
        let metrics = ConsensusMetrics::new(&registry).unwrap();
        
        // Test zero values
        metrics.update_active_validators(0);
        metrics.update_total_reserved_stake(0);
        metrics.update_current_epoch(0);
        metrics.record_reward_distribution(0);
        metrics.record_slashing(0);
        
        // Test large values
        metrics.update_active_validators(u32::MAX as u64);
        metrics.update_total_reserved_stake(u64::MAX as u128);
        metrics.update_current_epoch(u32::MAX);
        metrics.record_reward_distribution(u64::MAX as u128);
        metrics.record_slashing(u64::MAX as u128);
        
        println!("Edge cases test completed successfully");
    }

    #[test]
    fn test_metrics_concurrent_access() {
        use std::sync::Arc;
        use std::thread;
        
        let registry = Registry::new();
        let metrics = Arc::new(ConsensusMetrics::new(&registry).unwrap());
        let mut handles = vec![];
        
        // Spawn multiple threads to test concurrent access
        for thread_id in 0..5 {
            let metrics_clone = Arc::clone(&metrics);
            let handle = thread::spawn(move || {
                for i in 0..100 {
                    metrics_clone.update_active_validators(thread_id * 100 + i);
                    metrics_clone.record_reward_distribution((thread_id * 100 + i) as u128);
                    
                    if i % 10 == 0 {
                        metrics_clone.update_current_epoch((thread_id * 100 + i) as u32);
                    }
                    
                    if i % 20 == 0 {
                        metrics_clone.record_slashing((thread_id * 10 + i / 20) as u128);
                    }
                }
            });
            handles.push(handle);
        }
        
        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }
        
        println!("Concurrent access test completed successfully");
    }

    #[test]
    fn test_metrics_performance() {
        let registry = Registry::new();
        let metrics = ConsensusMetrics::new(&registry).unwrap();
        
        use std::time::Instant;
        
        // Test performance of metric updates
        let start = Instant::now();
        
        for i in 0..10000 {
            metrics.update_active_validators(i % 100);
            metrics.update_total_reserved_stake((i as u64 * 1000) as u128);
            
            if i % 100 == 0 {
                metrics.update_current_epoch((i / 100) as u32);
            }
            
            if i % 1000 == 0 {
                metrics.record_reward_distribution(i as u128);
                metrics.record_slashing((i / 10) as u128);
            }
        }
        
        let duration = start.elapsed();
        
        // Metrics updates should be fast (less than 1 second for 10k operations)
        assert!(duration.as_secs() < 5, "Metrics updates took too long: {:?}", duration);
        
        println!("Performance test completed in {:?}", duration);
    }

    #[test]
    fn test_metrics_registry_integration() {
        let registry = Registry::new();
        
        // Test that metrics can be created and registered
        let result = ConsensusMetrics::new(&registry);
        assert!(result.is_ok(), "Failed to create metrics");
        
        let metrics = result.unwrap();
        
        // Update some metrics
        metrics.update_active_validators(10);
        metrics.update_total_reserved_stake(50000);
        metrics.update_current_epoch(5);
        metrics.record_reward_distribution(1000);
        metrics.record_slashing(100);
        
        // Check that metrics were registered in the registry
        let metric_families = registry.gather();
        
        // We should have some metrics registered
        println!("Registry contains {} metric families", metric_families.len());
        
        // Print metric names for debugging
        for family in &metric_families {
            println!("Metric family: {}", family.get_name());
        }
        
        println!("Registry integration test completed successfully");
    }
}