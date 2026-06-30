// This file is part of the CBC Runtime.
// It defines configuration settings for the runtime, including consensus parameters,
// block weights, and other system-wide constants.

// === Imports ===
// Core Substrate and FRAME support crates
use frame_support::{
    derive_impl, parameter_types,
    traits::{ConstU128, ConstU32, ConstU64, ConstU8, VariantCountOf},
    weights::{
        constants::{RocksDbWeight, WEIGHT_REF_TIME_PER_SECOND},
        IdentityFee, Weight,
    },
};
use frame_system::limits::{BlockLength, BlockWeights};
use pallet_transaction_payment::{ConstFeeMultiplier, FungibleAdapter, Multiplier};
use sp_runtime::{traits::One, Perbill};
use sp_version::RuntimeVersion;

// Local runtime modules and type aliases
use super::{
    AccountId, Balance, Balances, Block, BlockNumber, Hash, Nonce, PalletInfo, Runtime,
    RuntimeCall, RuntimeEvent, RuntimeFreezeReason, RuntimeHoldReason, RuntimeOrigin, RuntimeTask,
    System, EXISTENTIAL_DEPOSIT, SLOT_DURATION, VERSION,
};

use crate::{
    MinValidatorScore, MaxValidatorScore, MinActiveValidators, MaxSlashingCount,
    MinInferenceConfidence, MaxInferenceAge, ChallengeWindow, InferenceReward, ChallengeReward,
    // Performance thresholds
    MinPerformanceScore, HighPerformanceScore, MinParticipationRate, HighParticipationRate,
    MaxMissedBlocks, MaxMissedBlocksHigh, HealthyValidatorScore, HealthyParticipationRate, HealthyMissedBlocksMax,
    // Score calculation thresholds
    ScoreChangeThreshold, ScoreChangePercentage, ScoreImprovementThreshold, ScoreImprovementPercentage,
    // Contribution balance thresholds
    MaxPosContribution, MaxPoiContribution, ImbalanceWarningThreshold,
    // Block processing intervals
    LeaveRequestCheckInterval, MetricsUpdateInterval, ScoreRefreshInterval, DetailedLoggingInterval, ImbalanceCheckInterval,
    // Validator set limits
    TopValidatorsDisplayCount, HealthCheckSampleSize,
    // Percentage constants
    FullPercentage, HighPerformancePercentage, TopPerformerPercentage,
    // Misbehavior reporting
    MaxEvidenceLength, MisbehaviorSlashThreshold,
};

// === Constants ===
const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);

// === Global Runtime Parameters ===
parameter_types! {
    pub const BlockHashCount: BlockNumber = 2400;
    pub const Version: RuntimeVersion = VERSION;

    // Max compute time per block: 2s of compute for a 6s block.
    pub RuntimeBlockWeights: BlockWeights = BlockWeights::with_sensible_defaults(
        Weight::from_parts(2u64 * WEIGHT_REF_TIME_PER_SECOND, u64::MAX),
        NORMAL_DISPATCH_RATIO,
    );

    // Max block size: 5MB with 75% for normal transactions.
    pub RuntimeBlockLength: BlockLength = BlockLength::max_with_normal_ratio(5 * 1024 * 1024, NORMAL_DISPATCH_RATIO);

    // Substrate address prefix.
    pub const SS58Prefix: u8 = 42;

    pub const MaxValidators: u32 = 100;
    pub const MinStake: u128 = 1000;
    pub const MaxStake: u128 = 1000000;
    pub const EpochDuration: u32 = 100;
    pub const SlashingPenalty: u32 = 10;
    pub const RewardRate: u32 = 5;
    pub const MaxInferenceResults: u32 = 1000;
    pub const ChallengePeriod: u32 = 10;
    pub const MaxChallenges: u32 = 100;
    pub const PosWeight: u64 = 6000;  // 60% weight for PoS (6000/10000)
    pub const PoiWeight: u64 = 4000;  // 40% weight for PoI (4000/10000)
    pub const ValidatorScoreDecay: u32 = 5;  // 5% score decay per inactive epoch

    // New parameters for governance
    pub const GovernanceQuorum: u32 = 50;  // 50% of validators needed for quorum
    pub const ProposalLifetime: u32 = 1000; // Blocks until proposal expires
    pub const MinProposalDeposit: Balance = 100_000;
    pub const MaxProposalsPerValidator: u32 = 5;
    
    // Parameters for validator management
    pub const ValidatorUptimeRequirement: u32 = 90; // 90% uptime requirement
    pub const MaxMissedBlocksPerEpoch: u32 = 50;
    pub const InactivityEjectionBlocks: u32 = 1000;
    

}

// === FRAME System Configuration ===
#[derive_impl(frame_system::config_preludes::SolochainDefaultConfig)]
impl frame_system::Config for Runtime {
    type Block = Block;
    type BlockWeights = RuntimeBlockWeights;
    type BlockLength = RuntimeBlockLength;
    type AccountId = AccountId;
    type Nonce = Nonce;
    type Hash = Hash;
    type BlockHashCount = BlockHashCount;
    type DbWeight = RocksDbWeight;
    type Version = Version;
    type AccountData = pallet_balances::AccountData<Balance>;
    type SS58Prefix = SS58Prefix;
    type MaxConsumers = ConstU32<16>;
}

// === Timestamping Configuration ===
impl pallet_timestamp::Config for Runtime {
    type Moment = u64;
    type OnTimestampSet = ();
    type MinimumPeriod = ConstU64<{ SLOT_DURATION / 2 }>;
    type WeightInfo = ();
}

// === Balances Configuration ===
impl pallet_balances::Config for Runtime {
    type MaxLocks = ConstU32<50>;
    type MaxReserves = ();
    type ReserveIdentifier = [u8; 8];
    type Balance = Balance;
    type RuntimeEvent = RuntimeEvent;
    type DustRemoval = ();
    type ExistentialDeposit = ConstU128<EXISTENTIAL_DEPOSIT>;
    type AccountStore = System;
    type WeightInfo = pallet_balances::weights::SubstrateWeight<Runtime>;
    type FreezeIdentifier = RuntimeFreezeReason;
    type MaxFreezes = VariantCountOf<RuntimeFreezeReason>;
    type RuntimeHoldReason = RuntimeHoldReason;
    type RuntimeFreezeReason = RuntimeFreezeReason;
    type DoneSlashHandler = ();
}

// === Transaction Payment Configuration ===
parameter_types! {
    pub FeeMultiplier: Multiplier = Multiplier::one();
}

impl pallet_transaction_payment::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type OnChargeTransaction = FungibleAdapter<Balances, ()>;
    type OperationalFeeMultiplier = ConstU8<5>;
    type WeightToFee = IdentityFee<Balance>;
    type LengthToFee = IdentityFee<Balance>;
    type FeeMultiplierUpdate = ConstFeeMultiplier<FeeMultiplier>;
    type WeightInfo = pallet_transaction_payment::weights::SubstrateWeight<Runtime>;
}

// === Sudo (Superuser) Configuration ===
impl pallet_sudo::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type WeightInfo = pallet_sudo::weights::SubstrateWeight<Runtime>;
}


// === CBC POI Pallet Configuration ===
pub struct PosInterfaceImpl;
impl pallet_cbc_poi::PosInterface<AccountId> for PosInterfaceImpl {
    fn boost_score(validator: &AccountId, weight: u32) -> frame_support::dispatch::DispatchResult {
        pallet_cbc_pos::Pallet::<Runtime>::boost_score(
            frame_system::RawOrigin::Root.into(),
            validator.clone(),
            weight,
        )
    }
    fn slash_score(validator: &AccountId, weight: u32) -> frame_support::dispatch::DispatchResult {
        pallet_cbc_pos::Pallet::<Runtime>::slash_score(
            frame_system::RawOrigin::Root.into(),
            validator.clone(),
            weight,
        )
    }
}

impl pallet_cbc_poi::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = pallet_cbc_poi::weights::SubstrateWeight<Runtime>;
    type MinInferenceConfidence = MinInferenceConfidence;
    type MaxInferenceAge = MaxInferenceAge;
    type ChallengeWindow = ChallengeWindow;
    type InferenceReward = InferenceReward;
    type ChallengeReward = ChallengeReward;
    type PosInterface = PosInterfaceImpl;
    type DcfInterface = pallet_cbc_dcf::Pallet<Runtime>;
}

impl pallet_cbc_pos::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = pallet_cbc_pos::weights::SubstrateWeight<Runtime>;
    type MinValidatorScore = MinValidatorScore;
    type MinActiveValidators = MinActiveValidators;
    type MaxValidators = MaxValidators;
    type ValidatorScoreDecay = ValidatorScoreDecay;
    type MaxSlashingCount = MaxSlashingCount;
    type MinStake = ConstU128<1000>; // Minimum stake of 1000 units
    type Balance = Balance;
    type Currency = Balances;
    type LeaveCooldown = ConstU32<1000>;
    type ValidatorReward = ConstU128<10000>;
    type SlashPercent = ConstU32<10>;
    type MaxSlashPerEpoch = ConstU128<50000>;
    type MaxSlashPerValidator = ConstU128<20000>;
    type MaxRewardPerEpoch = ConstU128<30000>;
    type MaxRewardPerValidator = ConstU128<10000>;
    type SlashPenaltyDivisor = ConstU64<1000>;
    type MaxSlashPenalty = ConstU64<50>;
    type RewardBoostDivisor = ConstU64<1000>;
    type MaxRewardBoost = ConstU64<20>;
    type HighPerformanceScore = HighPerformanceScore;
    type TopPerformerPercentage = TopPerformerPercentage;
    type ValidatorHandler = pallet_cbc_dcf::Pallet<Runtime>;
}

// === CBC DCF Pallet Configuration ===
impl pallet_cbc_dcf::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Balance = Balance;

    // Epoch history and scoring weights
    type MaxEpochHistory = ConstU32<24>;
    type DefaultPosWeight = PosWeight;
    type DefaultPoiWeight = PoiWeight;
    type MaxValidatorScore = MaxValidatorScore;

    // Score decay and activity parameters
    type MaxInactiveEpochs = ConstU32<5>;
    type ScoreDecayInterval = ConstU32<10>; // every 10 blocks
    type ParticipationUpdateInterval = ConstU32<100>; // every 100 blocks
    type UnderperformanceCheckInterval = ConstU32<50>; // every 50 blocks
    type ValidatorProposalInterval = ConstU32<200>; // every 200 blocks
    type HealthMetricsInterval = ConstU32<1000>; // every 1000 blocks
    type OffchainWorkerInterval = ConstU32<5>; // every 5 blocks
    type EpochLength = ConstU32<100>; // 100 blocks per epoch

    // Block authorship rewards and penalties
    type BlockAuthorshipBoost = ConstU64<10>;
    type MissedBlockPenalty = ConstU64<5>;

    // Inference scoring parameters
    type InferenceBoostLow = ConstU64<2>;
    type InferenceBoostMedium = ConstU64<5>;
    type InferenceBoostHigh = ConstU64<10>;
    type InferencePenaltyLow = ConstU64<1>;
    type InferencePenaltyMedium = ConstU64<3>;
    type InferencePenaltyHigh = ConstU64<7>;
    type InferenceConfidenceThresholdLow = ConstU32<70>;
    type InferenceConfidenceThresholdHigh = ConstU32<90>;

    // Validator metadata limits
    type MaxValidatorNameLength = ConstU32<32>;
    type MaxValidatorWebsiteLength = ConstU32<64>;
    type MaxValidatorContactLength = ConstU32<64>;
    type MaxValidatorDescriptionLength = ConstU32<128>;
    type MaxValidatorLocationLength = ConstU32<32>;
    type MaxPerformanceHistoryLength = ConstU32<100>;
    type MaxValidatorHistoryLength = ConstU32<10>;
    type MaxCommissionRate = ConstU32<10000>; // 100.00%

    // Percentage calculation precision
    type PercentagePrecision = ConstU32<10000>; // 0.01% precision

    // Off-chain worker configuration
    type OffchainWorkerTimeout = ConstU64<30000>; // 30 seconds
    type EstimatedBlockTime = ConstU64<6000>; // 6 seconds

    // Performance thresholds (DCF-specific ones; HighPerformanceScore and TopPerformerPercentage inherited from pos)
    type MinPerformanceScore = MinPerformanceScore;
    type MinParticipationRate = MinParticipationRate;
    type HighParticipationRate = HighParticipationRate;
    type MaxMissedBlocks = MaxMissedBlocks;
    type MaxMissedBlocksHigh = MaxMissedBlocksHigh;
    type HealthyValidatorScore = HealthyValidatorScore;
    type HealthyParticipationRate = HealthyParticipationRate;
    type HealthyMissedBlocksMax = HealthyMissedBlocksMax;

    // Score calculation thresholds
    type ScoreChangeThreshold = ScoreChangeThreshold;
    type ScoreChangePercentage = ScoreChangePercentage;
    type ScoreImprovementThreshold = ScoreImprovementThreshold;
    type ScoreImprovementPercentage = ScoreImprovementPercentage;

    // Contribution balance thresholds
    type MaxPosContribution = MaxPosContribution;
    type MaxPoiContribution = MaxPoiContribution;
    type ImbalanceWarningThreshold = ImbalanceWarningThreshold;

    // Block processing intervals
    type LeaveRequestCheckInterval = LeaveRequestCheckInterval;
    type MetricsUpdateInterval = MetricsUpdateInterval;
    type ScoreRefreshInterval = ScoreRefreshInterval;
    type DetailedLoggingInterval = DetailedLoggingInterval;
    type ImbalanceCheckInterval = ImbalanceCheckInterval;
    
    // Validator set limits
    type TopValidatorsDisplayCount = TopValidatorsDisplayCount;
    type HealthCheckSampleSize = HealthCheckSampleSize;
    
    // Percentage constants (TopPerformerPercentage is inherited from pos::Config)
    type FullPercentage = FullPercentage;
    type HighPerformancePercentage = HighPerformancePercentage;
    
    // Reward distribution percentages
    type BaseRewardPercentage = ConstU32<60>; // 60% of rewards go to base pool
    type PerformanceRewardPercentage = ConstU32<25>; // 25% for high performers
    type TopPerformerRewardPercentage = ConstU32<15>; // 15% for top performers
    
    // Misbehavior reporting configuration
    type MaxEvidenceLength = MaxEvidenceLength;
    type MisbehaviorSlashThreshold = MisbehaviorSlashThreshold;

    type WeightInfo = pallet_cbc_dcf::weights::SubstrateWeight<Runtime>;

    // Constants for hardcoded values
    type MaxValidatorHistorySize = ConstU32<10>;
    type MaxValidatorNameSize = ConstU32<32>;
    type MaxRuntimeApiBoundedVecSize = ConstU32<100>;
    type MaxProposalActionBoundedVecSize = ConstU32<100>;
    type AuthorNotActiveErrorCode = ConstU8<1>;
    type AuthorMismatchErrorCode = ConstU8<2>;

    // Trust score calculation weights
    type TrustScoreUptimeWeight = ConstU64<4000>; // 40% weight for uptime
    type TrustScoreInferenceWeight = ConstU64<4000>; // 40% weight for inference success
    type TrustScoreSlashingWeight = ConstU64<2000>; // 20% weight for slashing penalty
    type MaxTrustScore = ConstU64<10000>; // Maximum trust score (100.00%)
    
    // Trust score bounds and stability parameters
    type MinTrustScore = ConstU64<1000>; // Minimum trust score (10.00%)
    type MaxTrustScoreGrowthRate = ConstU32<500>; // Maximum 5% growth per epoch
    type MaxTrustScoreDecayRate = ConstU32<200>; // Maximum 2% decay per epoch
    type TrustScoreStabilityFactor = ConstU32<8000>; // 80% stability factor

    // DVF WeightFreezer Integration
    type WeightFreezer = pallet_cbc_dvf::Pallet<Runtime>;
    // DVF is the primary finality authority; DCF progressive finality is capped at the DVF-finalized block
    type DvfFinalizedBlockProvider = pallet_cbc_dvf::Pallet<Runtime>;
}


// ── Todo pallet runtime configuration ────────────────────────
use frame_support::traits::ConstU32 as TodoConstU32;

impl pallet_todo::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    /// Maximum 256 bytes for a todo title on mainnet.
    type MaxTitleLength = TodoConstU32<256>;
    /// Maximum 1024 bytes for a todo description.
    type MaxDescriptionLength = TodoConstU32<1024>;
    /// Allow up to 500 todos per account.
    type MaxTodosPerAccount = TodoConstU32<500>;
}
