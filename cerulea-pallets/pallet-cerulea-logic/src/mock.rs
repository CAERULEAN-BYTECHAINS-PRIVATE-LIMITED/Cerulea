//! The test runtime of the logic module.
//!
//! The runtime carries the system module, the schema module, the identity
//! module, the permission module, the state module and the logic module.
//! Every resolution therefore runs against real role assignments, real
//! capability entries and real records.

use crate as pallet_cerulea_logic;
use codec::{Decode, Encode};
use core::time::Duration;
use frame_support::{derive_impl, traits::ConstU32, traits::ConstU64, traits::UnixTime};
use sp_runtime::{traits::IdentityLookup, BuildStorage};

extern crate alloc;

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        Schema: pallet_cerulea_schema,
        Identity: pallet_cerulea_identity,
        Permission: pallet_cerulea_permission,
        State: pallet_cerulea_state,
        Logic: pallet_cerulea_logic,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
    type AccountId = u64;
    type Lookup = IdentityLookup<Self::AccountId>;
    type AccountData = ();
}

impl pallet_cerulea_schema::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type Records = State;
    type MaxTypesPerDeployment = ConstU32<64>;
    type MaxCollectionsPerDeployment = ConstU32<64>;
    type MaxDefinitionBytes = ConstU32<65_536>;
    type MaxMigrationBytes = ConstU32<65_536>;
    type MaxVersionsPerCollection = ConstU32<32>;
    type MigrationStepLimit = ConstU64<100_000>;
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

impl pallet_cerulea_permission::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type Schema = Schema;
    type Identity = Identity;
    type MaxEntryBytes = ConstU32<16_384>;
    type MaxEntriesPerPair = ConstU32<16>;
    type RowStepLimit = ConstU64<10_000>;
}

impl pallet_cerulea_state::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type Schema = Schema;
    type Permissions = Permission;
    type Time = TestClock;
    type MaxRecordSize = ConstU32<8_192>;
    type MaxRangeRead = ConstU32<256>;
    type MaxRecordsPerCollection = ConstU32<1_024>;
    type MaxIndexEntries = ConstU32<128>;
    type MaxIndexesPerCollection = ConstU32<8>;
    type MaxBucketEntries = ConstU32<64>;
}

impl pallet_cerulea_logic::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type Schema = Schema;
    type Records = State;
    type Permissions = Permission;
    type Identity = Identity;
    type MaxBlueprintBytes = ConstU32<262_144>;
    type MaxPayloadBytes = ConstU32<16_384>;
    type StepLimit = ConstU64<200_000>;
    type MaxRowsPerRead = ConstU32<256>;
    type MaxEffectsPerAction = ConstU32<64>;
    type Workflows = pallet_cerulea_logic::NoWorkflows<u64>;
}

const CLOCK: &[u8] = b"cerulea:test:logic:clock";

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
