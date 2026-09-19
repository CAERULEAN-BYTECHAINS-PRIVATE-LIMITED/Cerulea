// The mock runtime for the Custody Engine tests.
// The runtime carries the system pallet and the custody pallet. The validator
// source and the Standing Engine are answered by mock implementations that keep
// real state inside the test externalities.
use crate as pallet_cerulea_custody;
use codec::{Decode, Encode};
use frame_support::{
    parameter_types,
    traits::{ConstU32, ConstU64},
};
use frame_system as system;
use sp_core::H256;
use sp_runtime::{
    traits::{BlakeTwo256, IdentityLookup},
    BuildStorage,
};
use sp_std::vec::Vec;

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        Custody: pallet_cerulea_custody,
    }
);

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const SS58Prefix: u8 = 42;
}

impl system::Config for Test {
    type BaseCallFilter = frame_support::traits::Everything;
    type BlockWeights = ();
    type BlockLength = ();
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    type Nonce = u64;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = u64;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Block = Block;
    type RuntimeEvent = RuntimeEvent;
    type BlockHashCount = BlockHashCount;
    type DbWeight = ();
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = ();
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type SS58Prefix = SS58Prefix;
    type OnSetCode = ();
    type MaxConsumers = ConstU32<16>;
    type RuntimeTask = ();
    type ExtensionsWeightInfo = ();
    type SingleBlockMigrations = ();
    type MultiBlockMigrator = ();
    type PreInherents = ();
    type PostInherents = ();
    type PostTransactions = ();
}

// Storage keys the mock interfaces use inside the test externalities.
const CANDIDATES: &[u8] = b"mock/custody/candidates";
const SHARE_PREFIX: &[u8] = b"mock/custody/share/";
const FAULT_PREFIX: &[u8] = b"mock/custody/fault/";

fn key_for(prefix: &[u8], validator: &u64) -> Vec<u8> {
    let mut key = prefix.to_vec();
    key.extend_from_slice(&validator.encode());
    key
}

fn read_value<T: Decode + Default>(key: &[u8]) -> T {
    match sp_io::storage::get(key) {
        Some(raw) => T::decode(&mut &raw[..]).unwrap_or_default(),
        None => T::default(),
    }
}

fn write_value<T: Encode>(key: &[u8], value: &T) {
    sp_io::storage::set(key, &value.encode());
}

/// The validator set the Custody Engine draws against in these tests.
pub struct MockValidators;

impl pallet_cerulea_custody::CustodyValidators<u64> for MockValidators {
    fn custody_candidates() -> Vec<u64> {
        read_value::<Vec<u64>>(CANDIDATES)
    }
}

/// The Standing Engine in these tests. Every report lands in test storage.
pub struct MockStanding;

impl pallet_cerulea_custody::CustodyStanding<u64> for MockStanding {
    fn report_custody_share(validator: &u64, share: u32) {
        write_value(&key_for(SHARE_PREFIX, validator), &share);
    }

    fn report_custody_fault(validator: &u64, faults: u32) {
        write_value(&key_for(FAULT_PREFIX, validator), &faults);
    }
}

/// Read the custody share the Standing Engine received.
pub fn reported_share(validator: u64) -> u32 {
    read_value(&key_for(SHARE_PREFIX, &validator))
}

/// Read the fault count the Standing Engine received.
pub fn reported_faults(validator: u64) -> u32 {
    read_value(&key_for(FAULT_PREFIX, &validator))
}

/// Replace the validator set the engine draws against.
pub fn set_candidates(candidates: Vec<u64>) {
    write_value(CANDIDATES, &candidates);
}

impl pallet_cerulea_custody::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Validators = MockValidators;
    type Standing = MockStanding;
    type ChallengeInterval = ConstU32<5>;
    type DeadlineBlocks = ConstU32<3>;
    type MaxRecordsPerBlock = ConstU32<64>;
    type MaxOpenChallenges = ConstU32<16>;
}

/// Build genesis storage for the mock runtime.
/// The returned externalities seed four candidate validators.
pub fn new_test_ext() -> sp_io::TestExternalities {
    let storage = system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    let mut ext: sp_io::TestExternalities = storage.into();
    ext.execute_with(|| {
        set_candidates(sp_std::vec![1u64, 2, 3, 4]);
        frame_system::Pallet::<Test>::set_block_number(1);
    });
    ext
}

// Compile-time contract check.
// The block below names every trait the mock runtime claims to satisfy.
// A trait that gains an item stops this crate from compiling here first, so a
// mock can never drift behind its trait again. The check runs on every build
// of the test target, which every commit gate runs.
const _: fn() = || {
    fn settings_hold<T: crate::Config>() {}
    settings_hold::<Test>();

    fn validators_hold<T: crate::CustodyValidators<u64>>() {}
    validators_hold::<<Test as crate::Config>::Validators>();

    fn standing_holds<T: crate::CustodyStanding<u64>>() {}
    standing_holds::<<Test as crate::Config>::Standing>();
};
