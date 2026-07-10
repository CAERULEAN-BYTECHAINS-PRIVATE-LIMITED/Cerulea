use crate as pallet_cbc_dvf;
use frame_support::{
    parameter_types,
    traits::ConstU32,
};
use sp_core::{sr25519, Pair, H256};
use sp_runtime::{
    traits::{BlakeTwo256, IdentifyAccount, IdentityLookup, Verify},
    BuildStorage, MultiSignature, Perbill,
};

type Block = frame_system::mocking::MockBlock<Test>;
type Signature = MultiSignature;
type AccountPublic = <Signature as Verify>::Signer;
pub type AccountId = <AccountPublic as IdentifyAccount>::AccountId;

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
    pub enum Test
    {
        System: frame_system,
        Balances: pallet_balances,
        PalletCbcPos: pallet_cbc_pos,
        Dvf: pallet_cbc_dvf,
    }
);

parameter_types! {
    pub const BlockHashCount: u64 = 250;
}

impl frame_system::Config for Test {
    type BaseCallFilter = frame_support::traits::Everything;
    type BlockWeights = ();
    type BlockLength = ();
    type DbWeight = ();
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    type Nonce = u64;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = AccountId;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Block = Block;
    type RuntimeEvent = RuntimeEvent;
    type BlockHashCount = BlockHashCount;
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = pallet_balances::AccountData<u128>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type SS58Prefix = ();
    type OnSetCode = ();
    type MaxConsumers = ConstU32<16>;
    type SingleBlockMigrations = ();
    type MultiBlockMigrator = ();
    type PreInherents = ();
    type PostInherents = ();
    type PostTransactions = ();
    type RuntimeTask = ();
    type ExtensionsWeightInfo = ();
}

impl pallet_balances::Config for Test {
    type MaxLocks = ConstU32<50>;
    type MaxReserves = ();
    type ReserveIdentifier = [u8; 8];
    type Balance = u128;
    type RuntimeEvent = RuntimeEvent;
    type DustRemoval = ();
    type ExistentialDeposit = frame_support::traits::ConstU128<500>;
    type AccountStore = System;
    type WeightInfo = ();
    type FreezeIdentifier = ();
    type MaxFreezes = ();
    type RuntimeHoldReason = ();
    type RuntimeFreezeReason = ();
    type DoneSlashHandler = ();
}

parameter_types! {
    pub const MinValidatorScore: u32 = 50;
    pub const MinActiveValidators: u32 = 3;
    pub const MaxValidatorsPos: u32 = 10;
    pub const ValidatorScoreDecay: u32 = 5;
    pub const MaxSlashingCount: u32 = 3;
    pub const MinStake: u128 = 1000;
    pub const LeaveCooldown: u32 = 1000;
    pub const ValidatorReward: u128 = 10000;
    pub const SlashPercent: u32 = 10;
    pub const MaxSlashPerEpoch: u128 = 50000;
    pub const MaxSlashPerValidator: u128 = 20000;
    pub const MaxRewardPerEpoch: u128 = 30000;
    pub const MaxRewardPerValidator: u128 = 10000;
    pub const SlashPenaltyDivisor: u64 = 1000;
    pub const MaxSlashPenalty: u64 = 50;
    pub const RewardBoostDivisor: u64 = 1000;
    pub const MaxRewardBoost: u64 = 20;
    pub const HighPerformanceScore: u64 = 80;
    pub const TopPerformerPercentage: u32 = 20;
}

impl pallet_cbc_pos::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type MinValidatorScore = MinValidatorScore;
    type MinActiveValidators = MinActiveValidators;
    type MaxValidators = MaxValidatorsPos;
    type ValidatorScoreDecay = ValidatorScoreDecay;
    type MaxSlashingCount = MaxSlashingCount;
    type MinStake = MinStake;
    type Balance = u128;
    type Currency = Balances;
    type LeaveCooldown = LeaveCooldown;
    type ValidatorReward = ValidatorReward;
    type SlashPercent = SlashPercent;
    type MaxSlashPerEpoch = MaxSlashPerEpoch;
    type MaxSlashPerValidator = MaxSlashPerValidator;
    type MaxRewardPerEpoch = MaxRewardPerEpoch;
    type MaxRewardPerValidator = MaxRewardPerValidator;
    type SlashPenaltyDivisor = SlashPenaltyDivisor;
    type MaxSlashPenalty = MaxSlashPenalty;
    type RewardBoostDivisor = RewardBoostDivisor;
    type MaxRewardBoost = MaxRewardBoost;
    type HighPerformanceScore = HighPerformanceScore;
    type TopPerformerPercentage = TopPerformerPercentage;
    type ValidatorHandler = ();
}

parameter_types! {
    pub const StakeWeightFactor: u128 = 1;
    pub const ScoreWeightFactor: u128 = 100;
    pub const ScoreBoostCap: u128 = 100_000;
    pub const FinalityThreshold: Perbill = Perbill::from_percent(67);
    pub const FinalityCheckpointInterval: u32 = 10;
    pub const VoteRetentionRounds: u32 = 20;
}

impl pallet_cbc_dvf::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Signer = AccountPublic;
    type Signature = Signature;
    type StakeWeightFactor = StakeWeightFactor;
    type ScoreWeightFactor = ScoreWeightFactor;
    type ScoreBoostCap = ScoreBoostCap;
    type FinalityThreshold = FinalityThreshold;
    type FinalityCheckpointInterval = FinalityCheckpointInterval;
    type VoteRetentionRounds = VoteRetentionRounds;
    type MaxValidators = ConstU32<100>;
    type MaxInactiveEpochs = ConstU32<5>;
    type UnderperformanceCheckInterval = ConstU32<50>;
    type MaxValidatorHistorySize = ConstU32<100>;
    type MaxValidatorNameSize = ConstU32<32>;
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
    // Create test validator weights synchronized with production pattern
    // Using realistic stakes matching the bug condition exploration test
    let test_validator_weights = vec![
        (account_key("Alice"), 10_000_000, 80),   // Alice: 10M stake, 80 score
        (account_key("Bob"), 8_000_000, 80),      // Bob: 8M stake, 80 score
        (account_key("Charlie"), 6_000_000, 80),  // Charlie: 6M stake, 80 score
    ];

    let mut t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    // Initialize DVF genesis configuration
    crate::GenesisConfig::<Test> {
        initial_validator_weights: test_validator_weights,
    }
    .assimilate_storage(&mut t)
    .unwrap();

    t.into()
}

pub fn account_key(s: &str) -> AccountId {
    let pair = sr25519::Pair::from_string(&format!("//{}", s), None).unwrap();
    AccountPublic::from(pair.public()).into_account()
}
