use cerulea_node::fork_detection::ForkReport;

#[tokio::test]
async fn test_fork_report_creation() {
    let report = ForkReport {
        peer_id: "test-peer".to_string(),
        local_best: 100,
        peer_best: 95,
        divergence: 5,
    };
    
    assert_eq!(report.peer_id, "test-peer");
    assert_eq!(report.local_best, 100);
    assert_eq!(report.peer_best, 95);
    assert_eq!(report.divergence, 5);
}

#[test]
fn test_fork_report_serialization() {
    let report = ForkReport {
        peer_id: "test-peer".to_string(),
        local_best: 100,
        peer_best: 95,
        divergence: 5,
    };
    
    let json = serde_json::to_string(&report).unwrap();
    let deserialized: ForkReport = serde_json::from_str(&json).unwrap();
    
    assert_eq!(report.peer_id, deserialized.peer_id);
    assert_eq!(report.local_best, deserialized.local_best);
    assert_eq!(report.peer_best, deserialized.peer_best);
    assert_eq!(report.divergence, deserialized.divergence);
}

#[test]
fn test_fork_detection_threshold_logic() {
    // Test divergence calculation
    let local_best = 100u32;
    let peer_best = 95u32;
    let threshold = 10u32;
    
    let divergence = if local_best > peer_best {
        local_best - peer_best
    } else {
        peer_best - local_best
    };
    
    assert_eq!(divergence, 5);
    assert!(divergence <= threshold, "Divergence should be within threshold");
    
    // Test case where divergence exceeds threshold
    let peer_best_far = 80u32;
    let divergence_far = if local_best > peer_best_far {
        local_best - peer_best_far
    } else {
        peer_best_far - local_best
    };
    
    assert_eq!(divergence_far, 20);
    assert!(divergence_far > threshold, "Large divergence should exceed threshold");
}