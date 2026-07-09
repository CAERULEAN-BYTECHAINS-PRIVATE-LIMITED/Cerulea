//! Tests for Task 8 Prometheus metrics implementation
//! 
//! This module tests the new metrics added for Task 8:
//! - author_mismatch_total counter
//! - epoch_transitions_total counter  
//! - validator_score_gauge gauge vector
//! - block_production_time histogram
//! - rpc_request_duration histogram vector

#[cfg(test)]
mod tests {
    use crate::metrics::ConsensusMetrics;
    use prometheus::Registry;

    #[test]
    fn test_task8_metrics_registration() {
        let registry = Registry::new();
        let metrics = ConsensusMetrics::new(&registry).expect("Failed to create metrics");
        
        // Add some data to the vector metrics so they appear in the registry
        metrics.update_validator_score("test_validator", 50.0);
        metrics.record_rpc_request_duration("test_method", 0.1);
        
        // Test that all Task 8 metrics are registered
        let metric_families = registry.gather();
        let metric_names: Vec<String> = metric_families.iter()
            .map(|mf| mf.get_name().to_string())
            .collect();
        
        // Debug: print all metric names
        println!("Registered metrics: {:?}", metric_names);
        
        // Check that all new Task 8 metrics are present
        assert!(metric_names.contains(&"cerulea_author_mismatch_total".to_string()));
        assert!(metric_names.contains(&"cerulea_epoch_transitions_total".to_string()));
        assert!(metric_names.contains(&"cerulea_validator_score".to_string()));
        assert!(metric_names.contains(&"cerulea_block_production_time_seconds".to_string()));
        assert!(metric_names.contains(&"cerulea_rpc_request_duration_seconds".to_string()));
        
        println!("Task 8 metrics registration test completed successfully");
    }

    #[test]
    fn test_author_mismatch_counter() {
        let registry = Registry::new();
        let metrics = ConsensusMetrics::new(&registry).expect("Failed to create metrics");
        
        // Record some author mismatches
        metrics.record_author_mismatch();
        metrics.record_author_mismatch();
        metrics.record_author_mismatch();
        
        // Verify the counter incremented
        let metric_families = registry.gather();
        let author_mismatch_metric = metric_families.iter()
            .find(|mf| mf.get_name() == "cerulea_author_mismatch_total")
            .expect("Author mismatch metric not found");
        
        let counter_value = author_mismatch_metric.get_metric()[0].get_counter().get_value();
        assert_eq!(counter_value, 3.0);
        
        println!("Author mismatch counter test completed successfully");
    }

    #[test]
    fn test_epoch_transitions_counter() {
        let registry = Registry::new();
        let metrics = ConsensusMetrics::new(&registry).expect("Failed to create metrics");
        
        // Record some epoch transitions
        metrics.record_epoch_transition();
        metrics.record_epoch_transition();
        
        // Verify the counter incremented
        let metric_families = registry.gather();
        let epoch_transitions_metric = metric_families.iter()
            .find(|mf| mf.get_name() == "cerulea_epoch_transitions_total")
            .expect("Epoch transitions metric not found");
        
        let counter_value = epoch_transitions_metric.get_metric()[0].get_counter().get_value();
        assert_eq!(counter_value, 2.0);
        
        println!("Epoch transitions counter test completed successfully");
    }

    #[test]
    fn test_validator_score_gauge() {
        let registry = Registry::new();
        let metrics = ConsensusMetrics::new(&registry).expect("Failed to create metrics");
        
        // Update validator scores
        metrics.update_validator_score("validator1", 85.0);
        metrics.update_validator_score("validator2", 92.0);
        metrics.update_validator_score("validator1", 88.0); // Update existing
        
        // Verify the gauge values
        let metric_families = registry.gather();
        let validator_score_metric = metric_families.iter()
            .find(|mf| mf.get_name() == "cerulea_validator_score")
            .expect("Validator score metric not found");
        
        // Should have 2 validators
        assert_eq!(validator_score_metric.get_metric().len(), 2);
        
        // Check specific values
        for metric in validator_score_metric.get_metric() {
            let validator_id = metric.get_label()[0].get_value();
            let score = metric.get_gauge().get_value();
            
            if validator_id == "validator1" {
                assert_eq!(score, 88.0);
            } else if validator_id == "validator2" {
                assert_eq!(score, 92.0);
            } else {
                panic!("Unexpected validator ID: {}", validator_id);
            }
        }
        
        println!("Validator score gauge test completed successfully");
    }

    #[test]
    fn test_block_production_time_histogram() {
        let registry = Registry::new();
        let metrics = ConsensusMetrics::new(&registry).expect("Failed to create metrics");
        
        // Record some block production times
        metrics.record_block_production_time(0.5);
        metrics.record_block_production_time(1.2);
        metrics.record_block_production_time(0.8);
        
        // Verify the histogram
        let metric_families = registry.gather();
        let block_production_metric = metric_families.iter()
            .find(|mf| mf.get_name() == "cerulea_block_production_time_seconds")
            .expect("Block production time metric not found");
        
        let histogram = block_production_metric.get_metric()[0].get_histogram();
        assert_eq!(histogram.get_sample_count(), 3);
        
        // Check that the sum is approximately correct (0.5 + 1.2 + 0.8 = 2.5)
        let sum = histogram.get_sample_sum();
        assert!((sum - 2.5).abs() < 0.001);
        
        println!("Block production time histogram test completed successfully");
    }

    #[test]
    fn test_rpc_request_duration_histogram() {
        let registry = Registry::new();
        let metrics = ConsensusMetrics::new(&registry).expect("Failed to create metrics");
        
        // Record some RPC request durations
        metrics.record_rpc_request_duration("pos_getValidatorScore", 0.01);
        metrics.record_rpc_request_duration("pos_getValidatorStake", 0.02);
        metrics.record_rpc_request_duration("pos_getValidatorScore", 0.015);
        
        // Verify the histogram
        let metric_families = registry.gather();
        let rpc_duration_metric = metric_families.iter()
            .find(|mf| mf.get_name() == "cerulea_rpc_request_duration_seconds")
            .expect("RPC request duration metric not found");
        
        // Should have 2 different methods
        assert_eq!(rpc_duration_metric.get_metric().len(), 2);
        
        // Check specific method metrics
        for metric in rpc_duration_metric.get_metric() {
            let method = metric.get_label()[0].get_value();
            let histogram = metric.get_histogram();
            
            if method == "pos_getValidatorScore" {
                assert_eq!(histogram.get_sample_count(), 2);
                // Sum should be 0.01 + 0.015 = 0.025
                assert!((histogram.get_sample_sum() - 0.025).abs() < 0.001);
            } else if method == "pos_getValidatorStake" {
                assert_eq!(histogram.get_sample_count(), 1);
                assert!((histogram.get_sample_sum() - 0.02).abs() < 0.001);
            } else {
                panic!("Unexpected method: {}", method);
            }
        }
        
        println!("RPC request duration histogram test completed successfully");
    }

    #[test]
    fn test_all_task8_metrics_together() {
        let registry = Registry::new();
        let metrics = ConsensusMetrics::new(&registry).expect("Failed to create metrics");
        
        // Use all Task 8 metrics
        metrics.record_author_mismatch();
        metrics.record_epoch_transition();
        metrics.update_validator_score("test_validator", 75.0);
        metrics.record_block_production_time(1.5);
        metrics.record_rpc_request_duration("test_method", 0.05);
        
        // Verify all metrics are present and have expected values
        let metric_families = registry.gather();
        assert_eq!(metric_families.len(), 10); // 5 existing + 5 new Task 8 metrics
        
        // Check that we can find all Task 8 metrics
        let task8_metrics = ["cerulea_author_mismatch_total", "cerulea_epoch_transitions_total", 
                            "cerulea_validator_score", "cerulea_block_production_time_seconds", 
                            "cerulea_rpc_request_duration_seconds"];
        
        for metric_name in &task8_metrics {
            assert!(metric_families.iter().any(|mf| mf.get_name() == *metric_name),
                   "Task 8 metric {} not found", metric_name);
        }
        
        println!("All Task 8 metrics integration test completed successfully");
    }
}