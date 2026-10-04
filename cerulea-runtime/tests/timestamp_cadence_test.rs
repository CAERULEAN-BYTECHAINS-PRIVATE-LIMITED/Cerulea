//! Chain time keeps to the wall clock under the real block cadence.
//!
//! The author of each block sets the timestamp from its own clock, and the
//! timestamp module lifts it to the parent timestamp plus the minimum period
//! where the clock reads less. The nine validator network produces about one
//! block every 1,500 milliseconds, one rank window of the Authoring Engine.
//! A minimum period above that cadence pushed every block ahead, and chain
//! time ran 663 seconds ahead of the wall clock eleven minutes after genesis.
//!
//! The test drives the timestamp module of the runtime through 600 blocks at
//! the cadence the network shows, with the inherent each author creates, and
//! holds chain time within one slot of the wall clock throughout.

use cerulea_runtime::{Runtime, RuntimeOrigin, MINIMUM_TIMESTAMP_PERIOD, SLOT_DURATION};
use frame_support::inherent::ProvideInherent;
use frame_support::traits::{Get, Hooks, UnfilteredDispatchable};
use sp_inherents::InherentData;
use sp_runtime::BuildStorage;

/// The identifier the timestamp inherent carries.
const TIMESTAMP_INHERENT: [u8; 8] = *b"timstap0";

/// Run blocks at a fixed gap of wall time and report the largest lead of
/// chain time over the wall clock.
fn largest_lead(blocks: u32, gap_ms: u64, start_ms: u64) -> u64 {
    let storage = frame_system::GenesisConfig::<Runtime>::default()
        .build_storage()
        .expect("the system genesis builds");
    let mut ext: sp_io::TestExternalities = storage.into();
    ext.execute_with(|| {
        let mut wall = start_ms;
        let mut lead = 0u64;
        for number in 1..=blocks {
            frame_system::Pallet::<Runtime>::set_block_number(number);
            let mut data = InherentData::new();
            data.put_data(TIMESTAMP_INHERENT, &wall)
                .expect("the timestamp encodes");
            let call =
                <pallet_timestamp::Pallet<Runtime> as ProvideInherent>::create_inherent(&data)
                    .expect("the author creates the timestamp inherent");
            call.dispatch_bypass_filter(RuntimeOrigin::none())
                .expect("the timestamp applies");
            let chain = pallet_timestamp::Pallet::<Runtime>::get();
            lead = lead.max(chain.saturating_sub(wall));
            <pallet_timestamp::Pallet<Runtime> as Hooks<u32>>::on_finalize(number);
            wall += gap_ms;
        }
        lead
    })
}

#[test]
fn chain_time_stays_within_one_slot_of_the_wall_clock_at_the_real_cadence() {
    // The cadence the nine validator network shows: one block a rank window.
    let lead = largest_lead(600, 1_500, 1_790_000_000_000);
    assert!(
        lead <= SLOT_DURATION,
        "chain time ran {lead} milliseconds ahead of the wall clock"
    );
    // Blocks as fast as the minimum period still keep pace.
    let lead = largest_lead(600, MINIMUM_TIMESTAMP_PERIOD, 1_790_000_000_000);
    assert!(lead <= SLOT_DURATION, "lead {lead} at the minimum period");
}

#[test]
fn the_minimum_period_sits_under_the_real_cadence() {
    // The period the timestamp module runs with is the runtime constant.
    let period = <<Runtime as pallet_timestamp::Config>::MinimumPeriod as Get<u64>>::get();
    assert_eq!(period, MINIMUM_TIMESTAMP_PERIOD);
    // One rank window of the Authoring Engine is 1,500 milliseconds.
    let rank_window_ms = 1_500u64;
    assert!(
        period < rank_window_ms,
        "period {period} at or above the rank window"
    );
    assert!(period > 0, "a zero period lets two blocks share one moment");
}

#[test]
fn a_period_above_the_cadence_runs_chain_time_away() {
    // The earlier rule, half a six second slot, against 1,500 millisecond
    // blocks gains 1,500 milliseconds a block. The arithmetic the defect
    // showed: 440 blocks in eleven minutes put chain time 660 seconds ahead.
    let old_period = SLOT_DURATION / 2;
    let gained_per_block = old_period - 1_500;
    assert_eq!(gained_per_block * 440, 660_000);
}
