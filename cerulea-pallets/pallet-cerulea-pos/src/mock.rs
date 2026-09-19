use crate as pallet_cerulea_pos;
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

type Block = frame_system::mocking::MockBlock<Test>;

// The mock runtime below carries the system pallet, the balances pallet
// and the stake pallet. Tests run against this runtime.
frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        Balances: pallet_balances,
        PalletCeruleaPos: pallet_cerulea_pos,
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
    type AccountData = pallet_balances::AccountData<u64>;
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

impl pallet_balances::Config for Test {
    type MaxLocks = ConstU32<50>;
    type MaxReserves = ();
    type ReserveIdentifier = [u8; 8];
    type Balance = u64;
    type RuntimeEvent = RuntimeEvent;
    type DustRemoval = ();
    type ExistentialDeposit = ConstU64<500>;
    type AccountStore = System;
    type WeightInfo = ();
    type FreezeIdentifier = ();
    type MaxFreezes = ();
    type RuntimeHoldReason = ();
    type RuntimeFreezeReason = ();
    type DoneSlashHandler = ();
}

impl pallet_cerulea_pos::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type MinValidatorScore = ConstU32<50>;
    type MinActiveValidators = ConstU32<3>;
    type MaxValidators = ConstU32<10>;
    type ValidatorScoreDecay = ConstU32<5>;
    type MaxSlashingCount = ConstU32<3>;
    type MinStake = ConstU64<1000>;
    type Balance = u64;
    type Currency = Balances;
    type LeaveCooldown = ConstU32<1000>;
    type ValidatorReward = ConstU64<10000>;
    type SlashPercent = ConstU32<10>;
    type MaxSlashPerEpoch = ConstU64<50000>;
    type MaxSlashPerValidator = ConstU64<20000>;
    type MaxRewardPerEpoch = ConstU64<30000>;
    type MaxRewardPerValidator = ConstU64<10000>;
    type SlashPenaltyDivisor = ConstU64<1000>;
    type MaxSlashPenalty = ConstU64<50>;
    type RewardBoostDivisor = ConstU64<1000>;
    type MaxRewardBoost = ConstU64<20>;
    type HighPerformanceScore = ConstU64<80>;
    type TopPerformerPercentage = ConstU32<20>;
    type ValidatorHandler = TestAdmission;
}

/// The key the mock Admission Engine writes its active set under.
const ADMITTED_KEY: &[u8] = b"cerulea/test/admission/active";

/// A stand-in for the Admission Engine inside this mock runtime.
///
/// The Admission Engine governs entry to and exit from the validator set. The
/// stake pallet holds the stake and the registration record. The stake pallet
/// reads the active set from the engine through this handler. The stand-in
/// keeps the same contract, so a test reads real delegation rather than an
/// empty answer.
pub struct TestAdmission;

impl TestAdmission {
    /// Read the admitted set.
    pub fn admitted() -> Vec<u64> {
        frame_support::storage::unhashed::get::<Vec<u64>>(ADMITTED_KEY).unwrap_or_default()
    }

    fn write(set: Vec<u64>) {
        frame_support::storage::unhashed::put(ADMITTED_KEY, &set);
    }
}

impl crate::ValidatorHandler<u64, u64> for TestAdmission {
    fn on_joined(validator: &u64, _stake: u64) -> sp_runtime::DispatchResult {
        let mut set = Self::admitted();
        if !set.contains(validator) {
            set.push(*validator);
            Self::write(set);
        }
        Ok(())
    }

    fn on_leave_requested(_validator: &u64) -> sp_runtime::DispatchResult {
        Ok(())
    }

    fn on_left(validator: &u64) -> sp_runtime::DispatchResult {
        let mut set = Self::admitted();
        set.retain(|held| held != validator);
        Self::write(set);
        Ok(())
    }

    fn on_stake_increased(_validator: &u64, _amount: u64) -> sp_runtime::DispatchResult {
        Ok(())
    }

    fn on_stake_decreased(_validator: &u64, _amount: u64) -> sp_runtime::DispatchResult {
        Ok(())
    }

    fn on_slashed(_validator: &u64, _amount: u64, _penalty: u64) -> sp_runtime::DispatchResult {
        Ok(())
    }

    fn on_rewarded(_validator: &u64, _amount: u64, _boost: u64) -> sp_runtime::DispatchResult {
        Ok(())
    }

    fn get_validator_score(validator: &u64) -> u64 {
        crate::ValidatorScores::<Test>::get(validator).unwrap_or(0) as u64
    }

    fn get_active_validators() -> Vec<u64> {
        Self::admitted()
    }
}

// Build genesis storage for the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut storage = system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    pallet_balances::GenesisConfig::<Test> {
        balances: vec![
            (1, 10000),
            (2, 10000),
            (3, 10000),
        ],
        dev_accounts: None,
    }
    .assimilate_storage(&mut storage)
    .unwrap();

    storage.into()
} 

// Compile-time contract check.
// The block below names every trait the mock runtime claims to satisfy.
// A trait that gains an item stops this crate from compiling here first, so a
// mock can never drift behind its trait again. The check runs on every build
// of the test target, which every commit gate runs.
const _: fn() = || {
    fn settings_hold<T: crate::Config>() {}
    settings_hold::<Test>();

    fn validator_handler_holds<T: crate::ValidatorHandler<u64, u64>>() {}
    validator_handler_holds::<<Test as crate::Config>::ValidatorHandler>();
};
