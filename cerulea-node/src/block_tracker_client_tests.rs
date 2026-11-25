//! The block authoring tracker, run against the real client.
//!
//! The tracker reads the participation record the framework pallet keeps.
//! The fixture chain records two authored blocks and nine missed slots for
//! the development validator. A reader stands the tracker at chosen heights
//! so the tracker sees the record grow the way a running node sees it.

use std::time::Duration;

use cerulea_runtime::opaque::Block;
use sp_blockchain::HeaderBackend;

use crate::block_tracker::{start_block_tracker, BlockTracker, BlockTrackerConfig};
use crate::chain_fixture_tests::{alice, fixture, ViewAt, MISSED_BLOCKS};

/// Tracker settings that report and alert on every pass.
fn eager() -> BlockTrackerConfig {
    BlockTrackerConfig {
        monitoring_interval: Duration::from_millis(50),
        stats_reporting_interval: Duration::ZERO,
        underperformance_threshold: 80.0,
        max_consecutive_misses: 3,
        enable_alerts: true,
        enable_detailed_logging: true,
    }
}

/// Run the tracker for a short while and stop it.
async fn run_for(tracker: &mut BlockTracker<Block, ViewAt>, span: Duration) {
    let stopped = tokio::time::timeout(span, tracker.run()).await;
    assert!(
        stopped.is_err(),
        "the tracker runs until the caller stops it"
    );
}

#[tokio::test]
async fn the_tracker_follows_the_authored_blocks_and_then_the_missed_slots() {
    let tip = fixture().client.info().best_number;
    let view = ViewAt::block(0);
    let mut tracker = BlockTracker::<Block, ViewAt>::new(view.clone(), eager());
    assert!(tracker.get_all_stats().is_empty());
    assert_eq!(tracker.get_total_blocks_tracked(), 0);

    // At block two the validator authored both blocks and missed none.
    view.move_to(2);
    run_for(&mut tracker, Duration::from_millis(300)).await;
    let stats = tracker
        .get_validator_stats(&alice())
        .expect("the tracker records the development validator")
        .clone();
    assert_eq!(stats.authored_blocks, 2);
    assert_eq!(stats.missed_blocks, 0);
    assert_eq!(stats.expected_blocks, 2);
    assert_eq!(stats.participation_rate, 100.0);
    assert_eq!(stats.consecutive_misses, 0);
    assert_eq!(stats.last_authored_block, Some(0));
    assert_eq!(tracker.get_total_blocks_tracked(), 2);

    // At the head the validator missed every later slot.
    view.move_to(tip);
    run_for(&mut tracker, Duration::from_millis(300)).await;
    let stats = tracker
        .get_validator_stats(&alice())
        .expect("the tracker still records the validator")
        .clone();
    assert_eq!(stats.authored_blocks, 2);
    assert_eq!(stats.missed_blocks, MISSED_BLOCKS);
    assert_eq!(stats.expected_blocks, 2 + MISSED_BLOCKS);
    assert_eq!(stats.consecutive_misses, MISSED_BLOCKS);
    assert!(stats.is_underperforming(80.0));
    assert!(stats.has_concerning_misses(3));
    assert_eq!(tracker.get_total_blocks_tracked(), tip);
    assert_eq!(tracker.get_all_stats().len(), 1);
    assert!(tracker.get_uptime() >= Duration::from_millis(600));
}

#[tokio::test]
async fn the_tracker_carries_on_after_a_failed_runtime_read() {
    let view = ViewAt::block(0);
    let mut tracker = BlockTracker::<Block, ViewAt>::new(view.clone(), eager());
    view.move_to_unknown();
    run_for(&mut tracker, Duration::from_millis(200)).await;
    // The failed read leaves the record untouched and the service running.
    assert!(tracker.get_all_stats().is_empty());
    assert_eq!(tracker.get_total_blocks_tracked(), 0);
}

#[tokio::test]
async fn the_quiet_tracker_records_without_alerts_or_reports() {
    let view = ViewAt::block(0);
    let settings = BlockTrackerConfig {
        monitoring_interval: Duration::from_millis(50),
        enable_alerts: false,
        ..BlockTrackerConfig::default()
    };
    let mut tracker = BlockTracker::<Block, ViewAt>::new(view.clone(), settings);
    view.move_to(1);
    run_for(&mut tracker, Duration::from_millis(200)).await;
    let stats = tracker
        .get_validator_stats(&alice())
        .expect("the tracker records the validator");
    assert_eq!(stats.authored_blocks, 1);
    assert_eq!(tracker.get_total_blocks_tracked(), 1);
}

#[tokio::test]
async fn the_tracker_service_runs_until_the_caller_stops_it() {
    let stopped = tokio::time::timeout(
        Duration::from_millis(200),
        start_block_tracker::<Block, _>(fixture().client.clone(), None),
    )
    .await;
    assert!(stopped.is_err());
}
