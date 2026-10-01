//! The test runtime of the tier module.
//!
//! The runtime carries the system module, the schema module, the state
//! module, the audit module and the tier module. The state module lets a
//! test store a real record, suspend the deployment and then read and
//! export that record again.

use crate as pallet_cerulea_tier;
use cerulea_blueprint::types::RoleId;
use codec::{Decode, Encode};
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
        Audit: pallet_cerulea_audit,
        Tier: pallet_cerulea_tier,
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
    type Permissions = TestGate;
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

impl pallet_cerulea_audit::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type Time = TestClock;
    type MaxPayloadBytes = ConstU32<4_096>;
    type MaxEventDefinitions = ConstU32<64>;
    type MaxSubscribedEvents = ConstU32<16>;
    type MaxReadBatch = ConstU32<256>;
}

/// The deployment stewards the tests read.
///
/// Account one installed every deployment. The accounts named here
/// administer it.
pub struct TestStewards;

impl pallet_cerulea_schema::stewards::DeploymentStewards<u64> for TestStewards {
    fn installer(_deployment: &pallet_cerulea_schema::primitives::DeploymentId) -> Option<u64> {
        Some(1)
    }

    fn is_steward(
        _deployment: &pallet_cerulea_schema::primitives::DeploymentId,
        who: &u64,
    ) -> bool {
        *who == 1 || *who == 2
    }
}

impl pallet_cerulea_tier::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type Audit = Audit;
    type Stewards = TestStewards;
    type Time = TestClock;
    type MaxFeatures = ConstU32<32>;
    type MaxAddons = ConstU32<16>;
    type MaxWatchedDeployments = ConstU32<8>;
}

const CLOCK: &[u8] = b"cerulea:test:tier:clock";

/// The moment the test clock starts at, in milliseconds.
pub const START: u64 = 1_000_000;

/// Set the clock every expiry reads, in milliseconds.
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

/// Run the block hook of the tier module once and move to the next block.
pub fn roll_one_block() {
    let next = System::block_number() + 1;
    System::set_block_number(next);
    <Tier as frame_support::traits::Hooks<u64>>::on_initialize(next);
}

/// The test clock.
pub struct TestClock;

impl UnixTime for TestClock {
    fn now() -> Duration {
        Duration::from_millis(clock())
    }
}

/// The test gate. Every request passes, since the tests drive the licence
/// rather than the capability matrix.
pub struct TestGate;

impl PermissionGate for TestGate {
    type AccountId = u64;

    fn resolve(_request: &AccessRequest<'_, u64>) -> Result<AccessGrant, AccessDenial> {
        Ok(AccessGrant::every_field(Some(RoleId::new("registrar"))))
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

    fn gate_holds<T: crate::EntitlementGate<AccountId = u64>>() {}
    gate_holds::<crate::Pallet<Test>>();

    fn production_holds<T: crate::ProductionGate>() {}
    production_holds::<crate::Pallet<Test>>();

    fn clock_holds<T: UnixTime>() {}
    clock_holds::<<Test as crate::Config>::Time>();
};
