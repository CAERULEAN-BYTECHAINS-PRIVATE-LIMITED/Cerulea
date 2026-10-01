//! The test runtime of the collections module.
//!
//! The runtime carries the system module, the schema module, the state
//! module and the collections module. The test gate admits every request,
//! since the permission module joins the runtime at a later task and the
//! state module holds its own refusal tests.

use crate as pallet_cerulea_collections;
use alloc::string::ToString;
use cerulea_blueprint::types::RoleId;
use core::time::Duration;
use frame_support::{derive_impl, traits::ConstU32, traits::ConstU64, traits::UnixTime};
use pallet_cerulea_schema::access::{AccessDenial, AccessGrant, AccessRequest, PermissionGate};
use sp_runtime::{traits::IdentityLookup, BuildStorage};

extern crate alloc;

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        Schema: pallet_cerulea_schema,
        State: pallet_cerulea_state,
        Collections: pallet_cerulea_collections,
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

impl pallet_cerulea_state::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type Schema = Schema;
    type Permissions = OpenGate;
    type Time = TestClock;
    type MaxRecordSize = ConstU32<8_192>;
    type MaxRangeRead = ConstU32<256>;
    type MaxRecordsPerCollection = ConstU32<1_024>;
    type MaxIndexEntries = ConstU32<128>;
    type MaxIndexesPerCollection = ConstU32<8>;
    type MaxBucketEntries = ConstU32<64>;
    type MaxVersionHistory = ConstU32<32>;
    type MaxWatchedCollections = ConstU32<64>;
    type MaxRetentionScan = ConstU32<64>;
    type Custody = ();
}

impl pallet_cerulea_collections::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type Schema = Schema;
    type Records = State;
    type MaxRecordsPerPage = ConstU32<2>;
    type MaxKeyHolders = ConstU32<8>;
    type MaxWrappedKeyBytes = ConstU32<128>;
    type MaxPayloadBytes = ConstU32<8_192>;
    type KeyHistoryDepth = ConstU32<1>;
}

/// The test gate. Every request is admitted.
pub struct OpenGate;

impl PermissionGate for OpenGate {
    type AccountId = u64;

    fn resolve(_request: &AccessRequest<'_, u64>) -> Result<AccessGrant, AccessDenial> {
        Ok(AccessGrant::every_field(Some(RoleId::new("registrar"))))
    }
}

/// The test clock.
pub struct TestClock;

impl UnixTime for TestClock {
    fn now() -> Duration {
        Duration::from_millis(1_000_000)
    }
}

/// A denial the tests read when they need one.
pub fn sample_denial() -> AccessDenial {
    AccessDenial::owned(
        "The role reaches no record.".to_string(),
        "Grant the role the capability.".to_string(),
    )
}

/// Build the genesis storage of the test runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
    let storage = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .expect("the genesis storage builds");
    let mut ext = sp_io::TestExternalities::new(storage);
    ext.execute_with(|| System::set_block_number(1));
    ext
}
