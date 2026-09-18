// The mock runtime for the inference pallet tests.
// The runtime carries the system pallet and the inference pallet.
// The stake interface and the framework interface are answered by mock
// implementations that keep real state inside the test externalities.
use crate as pallet_cerulea_poi;
use codec::{Decode, Encode};
use frame_support::{
    parameter_types,
    traits::{ConstU128, ConstU32, ConstU64},
};
use frame_system as system;
use sp_core::H256;
use sp_runtime::{
    traits::{BlakeTwo256, IdentityLookup},
    BuildStorage,
};
use sp_std::vec::Vec;

// The mock block type for the test runtime.
type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        PalletCeruleaPoi: pallet_cerulea_poi,
    }
);

// Parameters the system pallet reads.
parameter_types! {
    pub const BlockHashCount: u64 = 250; // Recent block hashes kept.
    pub const SS58Prefix: u8 = 42;       // Default address prefix.
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
    type AccountId = u64; // Tests use u64 account identifiers.
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
    type MaxConsumers = frame_support::traits::ConstU32<16>;
    type RuntimeTask = ();
    type ExtensionsWeightInfo = ();
    type SingleBlockMigrations = ();
    type MultiBlockMigrator = ();
    type PreInherents = ();
    type PostInherents = ();
    type PostTransactions = ();
}

// Storage keys the mock interfaces use inside the test externalities.
const ACTIVE_KEY: &[u8] = b"mock/active_validators";
const SCORE_PREFIX: &[u8] = b"mock/score/";
const ACTIVITY_PREFIX: &[u8] = b"mock/activity/";
const FINAL_PREFIX: &[u8] = b"mock/final/";
const EJECTED_KEY: &[u8] = b"mock/ejected";

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

/// The mock stake interface. Score changes land in test storage.
pub struct MockStakeInterface;

impl pallet_cerulea_poi::PosInterface<u64> for MockStakeInterface {
    fn boost_score(validator: &u64, weight: u32) -> frame_support::dispatch::DispatchResult {
        let key = key_for(SCORE_PREFIX, validator);
        let current: u64 = read_value(&key);
        write_value(&key, &current.saturating_add(u64::from(weight)));
        Ok(())
    }

    fn slash_score(validator: &u64, weight: u32) -> frame_support::dispatch::DispatchResult {
        let key = key_for(SCORE_PREFIX, validator);
        let current: u64 = read_value(&key);
        write_value(&key, &current.saturating_sub(u64::from(weight)));
        Ok(())
    }

    fn get_active_validators() -> Vec<u64> {
        read_value::<Vec<u64>>(ACTIVE_KEY)
    }
}

/// The mock framework interface. Activity, ejection and final score land in
/// test storage so tests read the real effect of every call.
pub struct MockFrameworkInterface;

impl pallet_cerulea_poi::DcfInterface<u64> for MockFrameworkInterface {
    fn record_inference_activity(validator: &u64) -> frame_support::dispatch::DispatchResult {
        let key = key_for(ACTIVITY_PREFIX, validator);
        let current: u32 = read_value(&key);
        write_value(&key, &current.saturating_add(1));
        Ok(())
    }

    fn eject_validator(
        validator: &u64,
        _reason: &'static str,
    ) -> frame_support::dispatch::DispatchResult {
        let mut active: Vec<u64> = read_value(ACTIVE_KEY);
        active.retain(|held| held != validator);
        write_value(ACTIVE_KEY, &active);

        let mut ejected: Vec<u64> = read_value(EJECTED_KEY);
        if ejected.iter().all(|held| held != validator) {
            ejected.push(*validator);
        }
        write_value(EJECTED_KEY, &ejected);
        Ok(())
    }

    fn update_final_score(validator: &u64) -> frame_support::dispatch::DispatchResult {
        let score: u64 = read_value(&key_for(SCORE_PREFIX, validator));
        let activity: u32 = read_value(&key_for(ACTIVITY_PREFIX, validator));
        let settled = score.saturating_add(u64::from(activity));
        write_value(&key_for(FINAL_PREFIX, validator), &settled);
        Ok(())
    }
}

/// Read the mock stake score held for a validator.
pub fn mock_score(validator: u64) -> u64 {
    read_value(&key_for(SCORE_PREFIX, &validator))
}

/// Read the count of inference activity records held for a validator.
pub fn mock_activity(validator: u64) -> u32 {
    read_value(&key_for(ACTIVITY_PREFIX, &validator))
}

/// Read the settled final score held for a validator.
pub fn mock_final_score(validator: u64) -> u64 {
    read_value(&key_for(FINAL_PREFIX, &validator))
}

/// Read the validators the mock framework interface removed.
pub fn mock_ejected() -> Vec<u64> {
    read_value(EJECTED_KEY)
}

/// Read the active validator set the mock stake interface serves.
pub fn mock_active_validators() -> Vec<u64> {
    read_value(ACTIVE_KEY)
}

impl pallet_cerulea_poi::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type MinInferenceConfidence = ConstU32<50>; // Lowest confidence accepted.
    type MaxInferenceAge = ConstU32<10>; // Epochs an inference stays valid.
    type ChallengeWindow = ConstU32<5>; // Epochs open for a challenge.
    type InferenceReward = ConstU128<1000>; // Reward for a correct inference.
    type ChallengeReward = ConstU128<500>; // Reward for a winning challenge.
    type DcfInterface = MockFrameworkInterface;
    type PosInterface = MockStakeInterface;

    // Scoring parameters. Values match the runtime defaults.
    type InferenceBoostLow = ConstU64<2>;
    type InferenceBoostMedium = ConstU64<5>;
    type InferenceBoostHigh = ConstU64<10>;
    type InferencePenaltyLow = ConstU64<1>;
    type InferencePenaltyMedium = ConstU64<3>;
    type InferencePenaltyHigh = ConstU64<7>;
    type InferenceConfidenceThresholdLow = ConstU32<70>;
    type InferenceConfidenceThresholdHigh = ConstU32<90>;

    type MaxValidatorScore = ConstU64<100>;
    type PercentagePrecision = ConstU32<10000>;
    type OffchainWorkerInterval = ConstU32<5>;

    // Bounds on the off-chain score application pass.
    type MaxValidatorIterationWeight = ConstU64<1_000_000_000>;
    type MaxLoopIterations = ConstU32<1000>;
}

/// Build genesis storage for the mock runtime.
/// The returned externalities seed three active validators.
pub fn new_test_ext() -> sp_io::TestExternalities {
    let storage = system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    let mut ext: sp_io::TestExternalities = storage.into();
    ext.execute_with(|| {
        write_value(ACTIVE_KEY, &sp_std::vec![1u64, 2u64, 3u64]);
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

    fn stake_interface_holds<T: crate::PosInterface<u64>>() {}
    stake_interface_holds::<<Test as crate::Config>::PosInterface>();

    fn framework_interface_holds<T: crate::DcfInterface<u64>>() {}
    framework_interface_holds::<<Test as crate::Config>::DcfInterface>();
};
