//! The test runtime of the bridge module.
//!
//! The runtime carries the system module, the schema module, the identity
//! module, the permission module, the state module, the logic module, the
//! audit module, the governance module and the bridge module. An arriving
//! message therefore runs a real Blueprint action against real records,
//! and every event lands in the real audit log.
//!
//! The test clock reads a millisecond count from test storage, so a test
//! moves the clock past an expiry and watches the sweep cancel the
//! message.

use crate as pallet_cerulea_bridge;
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
        Audit: pallet_cerulea_audit,
        Governance: pallet_cerulea_governance,
        Bridge: pallet_cerulea_bridge,
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
    type MaxVersionHistory = ConstU32<16>;
    type MaxWatchedCollections = ConstU32<16>;
    type MaxRetentionScan = ConstU32<64>;
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

impl pallet_cerulea_audit::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type Time = TestClock;
    type MaxPayloadBytes = ConstU32<4_096>;
    type MaxEventDefinitions = ConstU32<64>;
    type MaxSubscribedEvents = ConstU32<16>;
    type MaxReadBatch = ConstU32<256>;
}

impl pallet_cerulea_governance::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type Roll = pallet_cerulea_governance::RegisteredElectorate<Test>;
    type Audit = Audit;
    type Time = TestClock;
    type MaxPayloadBytes = ConstU32<4_096>;
    type MaxAuthorityRoles = ConstU32<16>;
    type MaxVoteSpan = ConstU64<604_800_000>;
}

impl pallet_cerulea_bridge::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type Records = State;
    type Audit = Audit;
    type Approvals = Governance;
    type Actions = pallet_cerulea_bridge::LogicActions<Test>;
    type Time = TestClock;
    type MaxEndpointsPerDeployment = ConstU32<32>;
    type MaxHeadersPerEndpoint = ConstU32<64>;
    type MaxPendingPerBucket = ConstU32<64>;
    type MaxSweepPerBlock = ConstU32<16>;
    type MaxBucketsPerBlock = ConstU32<8>;
    type MaxLifetimeMillis = ConstU64<604_800_000>;
}

const CLOCK: &[u8] = b"cerulea:test:bridge:clock";

/// The moment the test clock starts at, in milliseconds.
pub const START: u64 = 1_700_000_000_000;

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

/// The test clock.
pub struct TestClock;

impl UnixTime for TestClock {
    fn now() -> Duration {
        Duration::from_millis(clock())
    }
}

/// Move the chain forward by one block and run the sweep.
pub fn roll_one_block() {
    let next = System::block_number() + 1;
    System::set_block_number(next);
    <Bridge as frame_support::traits::Hooks<u64>>::on_initialize(next);
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

    fn store_holds<T: pallet_cerulea_state::RecordStore<AccountId = u64>>() {}
    store_holds::<<Test as crate::Config>::Records>();

    fn audit_holds<T: pallet_cerulea_audit::AuditSink<AccountId = u64>>() {}
    audit_holds::<<Test as crate::Config>::Audit>();

    fn gate_holds<T: pallet_cerulea_governance::ApprovalGate>() {}
    gate_holds::<<Test as crate::Config>::Approvals>();

    fn actions_hold<T: crate::ActionRunner<AccountId = u64>>() {}
    actions_hold::<<Test as crate::Config>::Actions>();

    fn headers_hold<T: crate::endpoint::HeaderSource>() {}
    headers_hold::<crate::StoredHeaders<Test>>();

    fn clock_holds<T: UnixTime>() {}
    clock_holds::<<Test as crate::Config>::Time>();

    fn verifier_holds<V: crate::verify::Verifier>() {}
    verifier_holds::<crate::networks::evm::EvmVerifier>();
    verifier_holds::<crate::networks::cosmos::CosmosVerifier>();
    verifier_holds::<crate::networks::solana::SolanaVerifier>();
    verifier_holds::<crate::networks::polkadot::PolkadotVerifier>();
    verifier_holds::<crate::networks::avalanche::AvalancheVerifier>();
    verifier_holds::<crate::networks::bitcoin::BitcoinVerifier>();
    verifier_holds::<crate::networks::rollup::RollupVerifier>();
};
