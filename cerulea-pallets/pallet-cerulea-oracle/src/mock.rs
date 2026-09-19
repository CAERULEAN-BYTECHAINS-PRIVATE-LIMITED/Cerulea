//! The test runtime of the oracle module.
//!
//! The runtime carries the system module, the audit module and the oracle
//! module. The test clock reads a millisecond count from test storage, so
//! a test moves the clock past a dispute window and reads the value that
//! follows.

use crate as pallet_cerulea_oracle;
use codec::{Decode, Encode};
use core::time::Duration;
use frame_support::{derive_impl, traits::ConstU32, traits::ConstU64, traits::UnixTime};
use sp_runtime::{traits::IdentityLookup, BuildStorage};

extern crate alloc;

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        Audit: pallet_cerulea_audit,
        Oracle: pallet_cerulea_oracle,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
    type AccountId = u64;
    type Lookup = IdentityLookup<Self::AccountId>;
    type AccountData = ();
}

impl pallet_cerulea_audit::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type Time = TestClock;
    type MaxPayloadBytes = ConstU32<4_096>;
    type MaxEventDefinitions = ConstU32<64>;
    type MaxSubscribedEvents = ConstU32<16>;
    type MaxReadBatch = ConstU32<256>;
}

impl pallet_cerulea_oracle::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type Audit = Audit;
    type Time = TestClock;
    type MaxAttestorsPerFeed = ConstU32<8>;
    type MaxValueBytes = ConstU32<1_024>;
    type MaxFeedsPerDeployment = ConstU32<32>;
    type MaxDisputeWindow = ConstU64<604_800_000>;
}

const CLOCK: &[u8] = b"cerulea:test:oracle:clock";

/// The moment the test clock starts at, in milliseconds.
pub const START: u64 = 1_000_000;

/// Set the clock every window reads, in milliseconds.
pub fn set_clock(millis: u64) {
    sp_io::storage::set(CLOCK, &millis.encode());
}

/// The moment the test clock reads now, in milliseconds.
pub fn clock() -> u64 {
    sp_io::storage::get(CLOCK)
        .and_then(|found| u64::decode(&mut &found.to_vec()[..]).ok())
        .unwrap_or(START)
}

/// Move the clock forward by a span of milliseconds.
pub fn advance_clock(span: u64) {
    set_clock(clock().saturating_add(span));
}

/// The test clock.
pub struct TestClock;

impl UnixTime for TestClock {
    fn now() -> Duration {
        Duration::from_millis(clock())
    }
}

/// Build the genesis storage of the test runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
    let storage = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .expect("the genesis storage builds");
    let mut ext = sp_io::TestExternalities::new(storage);
    ext.execute_with(|| {
        System::set_block_number(1);
        set_clock(START);
    });
    ext
}

// Compile-time contract check.
// The block below names every trait the mock runtime claims to satisfy. A
// trait that gains an item stops this crate from compiling here first, so a
// mock can never drift behind its trait again. The check runs on every
// build of the test target, which every commit gate runs.
const _: fn() = || {
    fn settings_hold<T: crate::Config>() {}
    settings_hold::<Test>();

    fn audit_holds<T: pallet_cerulea_audit::AuditSink<AccountId = u64>>() {}
    audit_holds::<<Test as crate::Config>::Audit>();

    fn reader_holds<T: crate::FeedReader>() {}
    reader_holds::<crate::Pallet<Test>>();

    fn clock_holds<T: UnixTime>() {}
    clock_holds::<<Test as crate::Config>::Time>();
};
