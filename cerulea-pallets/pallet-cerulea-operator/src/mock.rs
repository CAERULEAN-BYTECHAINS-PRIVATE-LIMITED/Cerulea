//! The test runtime of the operator module.
//!
//! The runtime carries the system module, the schema module, the audit
//! module, the governance module, the evolution module, the tier module
//! and the operator module.
//!
//! The test gate stands in for the permission module. The gate grants the
//! operator role the reserved platform scope alone, so every request
//! naming a customer collection resolves to a denial. The limits of the
//! role are therefore enforced by the resolver rather than by a check
//! inside the operator module.

use crate as pallet_cerulea_operator;
use cerulea_blueprint::types::RoleId;
use codec::{Decode, Encode};
use core::time::Duration;
use frame_support::{derive_impl, traits::ConstU32, traits::ConstU64, traits::UnixTime};
use pallet_cerulea_schema::access::{AccessDenial, AccessGrant, AccessRequest, PermissionGate};
use sp_runtime::{traits::IdentityLookup, BuildStorage};

extern crate alloc;
use alloc::string::ToString;

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        Schema: pallet_cerulea_schema,
        Audit: pallet_cerulea_audit,
        Governance: pallet_cerulea_governance,
        Evolution: pallet_cerulea_evolution,
        Tier: pallet_cerulea_tier,
        Operator: pallet_cerulea_operator,
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
    type Records = ();
    type MaxTypesPerDeployment = ConstU32<64>;
    type MaxCollectionsPerDeployment = ConstU32<64>;
    type MaxDefinitionBytes = ConstU32<65_536>;
    type MaxMigrationBytes = ConstU32<65_536>;
    type MaxVersionsPerCollection = ConstU32<32>;
    type MigrationStepLimit = ConstU64<100_000>;
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
    type MaxPayloadBytes = ConstU32<8_192>;
    type MaxAuthorityRoles = ConstU32<16>;
    type MaxVoteSpan = ConstU64<604_800_000>;
}

impl pallet_cerulea_evolution::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type Schema = pallet_cerulea_evolution::SchemaBridge<Test>;
    type Approvals = Governance;
    type Audit = Audit;
    type Time = TestClock;
    type MaxPlanBytes = ConstU32<8_192>;
    type MaxVersionsPerDeployment = ConstU32<64>;
    type MaxRollbackWindow = ConstU64<604_800_000>;
}

impl pallet_cerulea_tier::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type Audit = Audit;
    type Time = TestClock;
    type MaxFeatures = ConstU32<32>;
    type MaxAddons = ConstU32<16>;
    type MaxWatchedDeployments = ConstU32<8>;
}

impl pallet_cerulea_operator::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type Permissions = PlatformScopeGate;
    type Versions = Evolution;
    type Licences = Tier;
    type Audit = Audit;
    type Time = TestClock;
}

const CLOCK: &[u8] = b"cerulea:test:operator:clock";

/// The moment the test clock starts at, in milliseconds.
pub const START: u64 = 1_000_000;

/// Set the clock every record stamps itself with, in milliseconds.
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

/// The resolver that grants the operator role the platform scope alone.
///
/// The permission module of a live deployment carries the same rule in its
/// capability matrix. A request naming any other collection resolves to a
/// denial, and the denial names the fix.
pub struct PlatformScopeGate;

impl PermissionGate for PlatformScopeGate {
    type AccountId = u64;

    fn resolve(request: &AccessRequest<'_, u64>) -> Result<AccessGrant, AccessDenial> {
        if request.collection.as_str() == pallet_cerulea_operator::PLATFORM_SCOPE {
            return Ok(AccessGrant::every_field(Some(RoleId::new(
                "cerulea_operator",
            ))));
        }
        Err(AccessDenial::owned(
            alloc::format!(
                "The Cerulea operator role reaches no record of `{}`, so the {} is refused.",
                request.collection,
                request.action.label()
            ),
            "Ask the customer steward to run the work inside the deployment.".to_string(),
        ))
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

    fn gate_holds<T: PermissionGate<AccountId = u64>>() {}
    gate_holds::<<Test as crate::Config>::Permissions>();

    fn versions_hold<T: pallet_cerulea_evolution::PlatformVersions<AccountId = u64>>() {}
    versions_hold::<<Test as crate::Config>::Versions>();

    fn licences_hold<T: pallet_cerulea_tier::LicenceControl<AccountId = u64>>() {}
    licences_hold::<<Test as crate::Config>::Licences>();

    fn audit_holds<T: pallet_cerulea_audit::AuditSink<AccountId = u64>>() {}
    audit_holds::<<Test as crate::Config>::Audit>();

    fn role_holds<T: crate::OperatorRole<AccountId = u64>>() {}
    role_holds::<crate::Pallet<Test>>();

    fn clock_holds<T: UnixTime>() {}
    clock_holds::<<Test as crate::Config>::Time>();
};
