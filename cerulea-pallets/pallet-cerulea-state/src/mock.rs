//! The test runtime of the state module.
//!
//! The runtime carries the system module, the schema module and the state
//! module. Two small components stand in for runtime services the test
//! runtime lacks.
//!
//! 1. The test gate resolves permission from a list of refused collections
//!    the tests set. The list lives in test storage, so a refusal behaves
//!    like a refusal from the permission module.
//! 2. The test clock reads a millisecond count from test storage.

use crate as pallet_cerulea_state;
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
}

const REFUSED: &[u8] = b"cerulea:test:refused";
const CLOCK: &[u8] = b"cerulea:test:clock";

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

/// Set the clock the row predicates read, in milliseconds.
pub fn set_clock(millis: u64) {
    sp_io::storage::set(CLOCK, &millis.encode());
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
        Duration::from_millis(stored::<u64>(CLOCK).unwrap_or(1_000_000))
    }
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
