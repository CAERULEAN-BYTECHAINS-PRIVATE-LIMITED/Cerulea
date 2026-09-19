//! The test runtime of the identity module.
//!
//! The runtime carries the system module and the identity module. The test
//! clock reads a millisecond count from test storage, so a test moves the
//! clock past an expiry and reads the result.

use crate as pallet_cerulea_identity;
use codec::{Decode, Encode};
use core::time::Duration;
use frame_support::{derive_impl, traits::ConstU32, traits::UnixTime};
use sp_runtime::{traits::IdentityLookup, BuildStorage};

extern crate alloc;

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        Identity: pallet_cerulea_identity,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
    type AccountId = u64;
    type Lookup = IdentityLookup<Self::AccountId>;
    type AccountData = ();
}

impl pallet_cerulea_identity::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type Time = TestClock;
    type MaxUnitsPerDeployment = ConstU32<32>;
    type MaxRolesPerDeployment = ConstU32<32>;
    type MaxRolesPerAccount = ConstU32<8>;
    type MaxCredentialsPerRole = ConstU32<4>;
    type MaxUnitDepth = ConstU32<4>;
    type MaxTextBytes = ConstU32<128>;
    type MaxCredentialsPerAccount = ConstU32<8>;
    type MaxScopePerDelegation = ConstU32<8>;
    type MaxDelegationsPerAccount = ConstU32<8>;
    type MaxDelegationDepth = ConstU32<3>;
}

const CLOCK: &[u8] = b"cerulea:test:identity:clock";

/// The moment the test clock reads, in milliseconds.
pub const START: u64 = 1_000_000;

/// Set the clock, in milliseconds.
pub fn set_clock(millis: u64) {
    sp_io::storage::set(CLOCK, &millis.encode());
}

/// The test clock.
pub struct TestClock;

impl UnixTime for TestClock {
    fn now() -> Duration {
        let held = sp_io::storage::get(CLOCK)
            .and_then(|found| u64::decode(&mut &found.to_vec()[..]).ok())
            .unwrap_or(START);
        Duration::from_millis(held)
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
