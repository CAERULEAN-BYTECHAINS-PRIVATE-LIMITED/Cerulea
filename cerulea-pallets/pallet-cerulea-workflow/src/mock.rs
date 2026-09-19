//! The test runtime of the workflow module.
//!
//! The runtime carries the system module, the schema module, the state
//! module, the identity module, the audit module and the workflow module.
//! Every test therefore drives a real record store, a real registry and a
//! real audit log.
//!
//! Two small components stand in for runtime services the test runtime
//! lacks. The test gate resolves permission from a list of refused
//! collections the tests set. The test clock reads a millisecond count
//! from test storage, so a test moves the wall clock past a timer and
//! watches the schedule fire.

use crate as pallet_cerulea_workflow;
use alloc::string::{String, ToString};
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
        Identity: pallet_cerulea_identity,
        Audit: pallet_cerulea_audit,
        Workflow: pallet_cerulea_workflow,
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
    type MaxDelegationDepth = ConstU32<4>;
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

impl pallet_cerulea_workflow::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type Schema = Schema;
    type Records = State;
    type Identities = Identity;
    type Permissions = TestGate;
    type Guards = pallet_cerulea_workflow::MeteredGuards;
    type Actions = pallet_cerulea_workflow::QueuedActions<Test>;
    type Audit = Audit;
    type Time = TestClock;
    type MaxMachineBytes = ConstU32<65_536>;
    type MaxMachinesPerDeployment = ConstU32<32>;
    type MaxStepsPerInstance = ConstU32<64>;
    type MaxTimersPerState = ConstU32<8>;
    type MaxScheduledPerBlock = ConstU32<4>;
    type MaxCompensationStepsPerBlock = ConstU32<2>;
    type MaxCompensatingInstances = ConstU32<16>;
    type GuardStepLimit = ConstU64<10_000>;
}

const REFUSED: &[u8] = b"cerulea:test:workflow:refused";
const CLOCK: &[u8] = b"cerulea:test:workflow:clock";

/// The moment the test clock starts at, in milliseconds.
pub const START: u64 = 1_000_000;

fn stored<V: Decode>(key: &[u8]) -> Option<V> {
    sp_io::storage::get(key).and_then(|held| V::decode(&mut &held.to_vec()[..]).ok())
}

/// The collections the test gate refuses.
pub fn refused_collections() -> alloc::vec::Vec<String> {
    stored::<alloc::vec::Vec<String>>(REFUSED).unwrap_or_default()
}

/// Refuse every request naming one collection.
pub fn refuse(collection: &str) {
    let mut held = refused_collections();
    held.push(collection.to_string());
    sp_io::storage::set(REFUSED, &held.encode());
}

/// Admit every request again.
pub fn admit_every_collection() {
    sp_io::storage::clear(REFUSED);
}

/// Set the clock the timers and the deadlines read, in milliseconds.
pub fn set_clock(millis: u64) {
    sp_io::storage::set(CLOCK, &millis.encode());
}

/// The moment the test clock reads now, in milliseconds.
pub fn clock() -> u64 {
    stored::<u64>(CLOCK).unwrap_or(START)
}

/// Move the clock forward by a span of milliseconds.
pub fn advance_clock(span: u64) {
    set_clock(clock().saturating_add(span));
}

/// Run the block hook of the workflow module once and move to the next
/// block.
pub fn roll_one_block() {
    let next = System::block_number() + 1;
    System::set_block_number(next);
    <Workflow as frame_support::traits::Hooks<u64>>::on_initialize(next);
}

/// Run the block hook a number of times.
pub fn roll_blocks(count: u32) {
    for _ in 0..count {
        roll_one_block();
    }
}

/// The test gate. A collection on the refused list is refused.
pub struct TestGate;

impl PermissionGate for TestGate {
    type AccountId = u64;

    fn resolve(request: &AccessRequest<'_, u64>) -> Result<AccessGrant, AccessDenial> {
        if refused_collections()
            .iter()
            .any(|held| held == request.collection.as_str())
        {
            return Err(AccessDenial::owned(
                alloc::format!(
                    "The role reaches no record of `{}`, so the {} is refused.",
                    request.collection,
                    request.action.label()
                ),
                "Grant the role the capability over that collection.".to_string(),
            ));
        }
        Ok(AccessGrant::every_field(Some(RoleId::new("registrar"))))
    }
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

    fn schema_holds<T: pallet_cerulea_schema::registry::SchemaRegistry>() {}
    schema_holds::<<Test as crate::Config>::Schema>();

    fn records_hold<T: pallet_cerulea_state::RecordStore<AccountId = u64>>() {}
    records_hold::<<Test as crate::Config>::Records>();

    fn identities_hold<T: pallet_cerulea_identity::IdentityRegistry<AccountId = u64>>() {}
    identities_hold::<<Test as crate::Config>::Identities>();

    fn gate_holds<T: PermissionGate<AccountId = u64>>() {}
    gate_holds::<<Test as crate::Config>::Permissions>();

    fn guards_hold<T: crate::GuardEngine>() {}
    guards_hold::<<Test as crate::Config>::Guards>();

    fn actions_hold<T: crate::ActionRunner<AccountId = u64>>() {}
    actions_hold::<<Test as crate::Config>::Actions>();

    fn audit_holds<T: pallet_cerulea_audit::AuditSink<AccountId = u64>>() {}
    audit_holds::<<Test as crate::Config>::Audit>();

    fn view_holds<T: crate::ProcessView>() {}
    view_holds::<crate::Pallet<Test>>();

    fn clock_holds<T: UnixTime>() {}
    clock_holds::<<Test as crate::Config>::Time>();
};
