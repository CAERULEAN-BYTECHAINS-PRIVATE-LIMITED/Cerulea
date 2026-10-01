//! The test runtime of the relayer module.
//!
//! The runtime carries the system module, the identity module, the audit
//! module, the tier module and the relayer module. The tier module holds a
//! real licence, so a test checks the entitlement gate as well as the
//! quota.

use crate as pallet_cerulea_relayer;
use codec::{Decode, Encode};
use core::time::Duration;
use frame_support::{derive_impl, traits::ConstU32, traits::ConstU64, traits::UnixTime};
use sp_runtime::{traits::IdentityLookup, BuildStorage};

extern crate alloc;

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        Identity: pallet_cerulea_identity,
        Audit: pallet_cerulea_audit,
        Tier: pallet_cerulea_tier,
        Relayer: pallet_cerulea_relayer,
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
        *who == 1
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

impl pallet_cerulea_relayer::Config for Test {
    type Stewards = TestStewards;
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type Identities = Identity;
    type Entitlements = Tier;
    type Audit = Audit;
    type Time = TestClock;
    type UnitsPerSubmission = ConstU64<10>;
    type MaxAuthorisationSpan = ConstU64<3_600_000>;
}

const CLOCK: &[u8] = b"cerulea:test:relayer:clock";

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

    fn identities_hold<T: pallet_cerulea_identity::IdentityRegistry<AccountId = u64>>() {}
    identities_hold::<<Test as crate::Config>::Identities>();

    fn entitlements_hold<T: pallet_cerulea_tier::EntitlementGate<AccountId = u64>>() {}
    entitlements_hold::<<Test as crate::Config>::Entitlements>();

    fn audit_holds<T: pallet_cerulea_audit::AuditSink<AccountId = u64>>() {}
    audit_holds::<<Test as crate::Config>::Audit>();

    fn actors_hold<T: crate::RelayedActor<AccountId = u64>>() {}
    actors_hold::<crate::Pallet<Test>>();

    fn clock_holds<T: UnixTime>() {}
    clock_holds::<<Test as crate::Config>::Time>();
};
