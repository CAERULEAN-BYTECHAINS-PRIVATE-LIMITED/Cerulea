// This file is part of the Cerulea Runtime.
// It defines settings settings for the runtime, including consensus parameters,
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
    ChallengeReward,
    ChallengeWindow,
    DetailedLoggingInterval,
    // Percentage constants
    FullPercentage,
    HealthCheckSampleSize,
    HealthyMissedBlocksMax,
    HealthyParticipationRate,
    HealthyValidatorScore,
    HighParticipationRate,
    HighPerformancePercentage,
    HighPerformanceScore,
    ImbalanceCheckInterval,
    ImbalanceWarningThreshold,
    InferenceReward,
    // Block processing intervals
    LeaveRequestCheckInterval,
    // Misbehavior reporting
    MaxEvidenceLength,
    MaxInferenceAge,
    MaxMissedBlocks,
    MaxMissedBlocksHigh,
    MaxPoiContribution,
    // Contribution balance thresholds
    MaxPosContribution,
    MaxSlashingCount,
    MaxValidatorScore,
    MetricsUpdateInterval,
    MinActiveValidators,
    MinInferenceConfidence,
    MinParticipationRate,
    // Performance thresholds
    MinPerformanceScore,
    MinValidatorScore,
    MisbehaviorSlashThreshold,
    ScoreChangePercentage,
    // Score calculation thresholds
    ScoreChangeThreshold,
    ScoreImprovementPercentage,
    ScoreImprovementThreshold,
    ScoreRefreshInterval,
    TopPerformerPercentage,
    // Validator set limits
    TopValidatorsDisplayCount,
};

// === Constants ===
const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);

/// The bytes one block carries.
const BLOCK_LENGTH_BYTES: u32 = 5 * 1024 * 1024;

// The extrinsic body path of the shared bound table equals the normal share of
// the block length. The transaction pool check reads the block length, so one
// extrinsic body longer than the path bound fails validation before dispatch.
const _: () = assert!(
    (BLOCK_LENGTH_BYTES as usize) * 75 / 100
        == cerulea_primitives::bounds::MessagePath::ExtrinsicBody.max_bytes()
);

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
    pub RuntimeBlockLength: BlockLength = BlockLength::max_with_normal_ratio(BLOCK_LENGTH_BYTES, NORMAL_DISPATCH_RATIO);

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
    // The legacy stake and inference split the framework pallet still reads on
    // its retired scoring path. The Trust Score composition in the Standing
    // Engine carries the live weighting. These two values reach no live path.
    pub const RetiredStakeSplit: u64 = 6000;
    pub const RetiredInferenceSplit: u64 = 4000;
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

// === FRAME System Settings ===
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

// === Timestamping Settings ===
impl pallet_timestamp::Config for Runtime {
    type Moment = u64;
    type OnTimestampSet = ();
    type MinimumPeriod = ConstU64<{ SLOT_DURATION / 2 }>;
    type WeightInfo = ();
}

// === Balances Settings ===
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

// === Transaction Payment Settings ===
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

// === Sudo (Superuser) Settings ===
impl pallet_sudo::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type WeightInfo = pallet_sudo::weights::SubstrateWeight<Runtime>;
}

// === Cerulea POI Pallet Settings ===
pub struct PosInterfaceImpl;
impl pallet_cerulea_poi::PosInterface<AccountId> for PosInterfaceImpl {
    fn boost_score(validator: &AccountId, weight: u32) -> frame_support::dispatch::DispatchResult {
        pallet_cerulea_pos::Pallet::<Runtime>::boost_score(
            frame_system::RawOrigin::Root.into(),
            validator.clone(),
            weight,
        )
    }
    fn slash_score(validator: &AccountId, weight: u32) -> frame_support::dispatch::DispatchResult {
        pallet_cerulea_pos::Pallet::<Runtime>::slash_score(
            frame_system::RawOrigin::Root.into(),
            validator.clone(),
            weight,
        )
    }
    fn get_active_validators() -> sp_runtime::Vec<AccountId> {
        pallet_cerulea_pos::Pallet::<Runtime>::get_active_validators()
    }
}

impl pallet_cerulea_poi::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = pallet_cerulea_poi::weights::SubstrateWeight<Runtime>;
    type MinInferenceConfidence = MinInferenceConfidence;
    type MaxInferenceAge = MaxInferenceAge;
    type ChallengeWindow = ChallengeWindow;
    type InferenceReward = InferenceReward;
    type ChallengeReward = ChallengeReward;
    type PosInterface = PosInterfaceImpl;
    type DcfInterface = pallet_cerulea_dcf::Pallet<Runtime>;

    // PoI scoring parameters
    type InferenceBoostLow = ConstU64<2>;
    type InferenceBoostMedium = ConstU64<5>;
    type InferenceBoostHigh = ConstU64<10>;
    type InferencePenaltyLow = ConstU64<1>;
    type InferencePenaltyMedium = ConstU64<3>;
    type InferencePenaltyHigh = ConstU64<7>;
    type InferenceConfidenceThresholdLow = ConstU32<70>;
    type InferenceConfidenceThresholdHigh = ConstU32<90>;

    type MaxValidatorScore = MaxValidatorScore;
    type PercentagePrecision = ConstU32<10000>;
    type OffchainWorkerInterval = ConstU32<5>;

    // Rate limiting settingss for apply_offchain_poi_scores
    type MaxValidatorIterationWeight = ConstU64<1_000_000_000>;
    type MaxLoopIterations = ConstU32<1000>;
}

impl pallet_cerulea_pos::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = pallet_cerulea_pos::weights::SubstrateWeight<Runtime>;
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
    type ValidatorHandler = pallet_cerulea_dvf::Pallet<Runtime>;
}

// === Cerulea the framework pallet Pallet Settings ===
impl pallet_cerulea_dcf::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Balance = Balance;

    // Epoch history and scoring weights
    type MaxEpochHistory = ConstU32<24>;
    type DefaultPosWeight = RetiredStakeSplit;
    type DefaultPoiWeight = RetiredInferenceSplit;

    // Score decay and activity parameters
    type MaxInactiveEpochs = ConstU32<5>;
    type ScoreDecayInterval = ConstU32<10>; // every 10 blocks
    type ParticipationUpdateInterval = ConstU32<100>; // every 100 blocks
    type UnderperformanceCheckInterval = ConstU32<50>; // every 50 blocks
    type ValidatorProposalInterval = ConstU32<200>; // every 200 blocks
    type HealthMetricsInterval = ConstU32<1000>; // every 1000 blocks
    type EpochLength = ConstU32<100>; // 100 blocks per epoch
    // The governance range of the epoch length: 50 blocks to one day at six
    // second blocks.
    type MinEpochLength = ConstU32<50>;
    type MaxEpochLength = ConstU32<14_400>;

    // Block authorship rewards and penalties
    type BlockAuthorshipBoost = ConstU64<10>;
    type MissedBlockPenalty = ConstU64<5>;

    // Validator metadata limits
    type MaxValidatorNameLength = ConstU32<32>;
    type MaxValidatorWebsiteLength = ConstU32<64>;
    type MaxValidatorContactLength = ConstU32<64>;
    type MaxValidatorDescriptionLength = ConstU32<128>;
    type MaxValidatorLocationLength = ConstU32<32>;
    type MaxPerformanceHistoryLength = ConstU32<100>;
    type MaxValidatorHistoryLength = ConstU32<10>;
    type MaxCommissionRate = ConstU32<10000>; // 100.00%

    // Off-chain worker settings
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

    // Misbehavior reporting settings
    type MaxEvidenceLength = MaxEvidenceLength;
    type MisbehaviorSlashThreshold = MisbehaviorSlashThreshold;

    type WeightInfo = pallet_cerulea_dcf::weights::SubstrateWeight<Runtime>;

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

    // the validator registry WeightFreezer Integration
    type WeightFreezer = pallet_cerulea_dvf::Pallet<Runtime>;

    // Validator Registry
    type ValidatorRegistry = pallet_cerulea_dvf::Pallet<Runtime>;
}

// === The Cerulea schema module ===
// The schema module holds the registered types and collections of every
// deployment. The record store hook stays empty until the state module
// joins the runtime.
impl pallet_cerulea_schema::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = pallet_cerulea_schema::weights::CeruleaWeight<Runtime>;
    type Records = crate::State;
    type MaxTypesPerDeployment = ConstU32<512>;
    type MaxCollectionsPerDeployment = ConstU32<512>;
    type MaxDefinitionBytes = ConstU32<262144>;
    type MaxMigrationBytes = ConstU32<262144>;
    type MaxVersionsPerCollection = ConstU32<256>;
    type MigrationStepLimit = ConstU64<1000000>;
}

// === Cerulea Custody Engine ===
// The Custody Engine verifies continuously that every validator holds the
// complete history and serves that history on demand.

/// The validator set the Custody Engine draws its challenges against.
///
/// Every node reads the same active set from the stake pallet, so every node
/// derives the same challenge.
pub struct CustodyCandidates;

impl pallet_cerulea_custody::CustodyValidators<AccountId> for CustodyCandidates {
    fn custody_candidates() -> sp_runtime::Vec<AccountId> {
        pallet_cerulea_pos::Pallet::<Runtime>::get_active_validators()
    }
}

impl pallet_cerulea_custody::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Validators = CustodyCandidates;
    // The framework pallet holds the Trust Score, so the custody share lands
    // there.
    type Standing = pallet_cerulea_dcf::Pallet<Runtime>;
    // A challenge every ten blocks keeps the sampling steady while the message
    // volume stays low.
    type ChallengeInterval = ConstU32<10>;
    // Three blocks close faster than a network fetch and a proof rebuild
    // complete.
    type DeadlineBlocks = ConstU32<3>;
    type MaxRecordsPerBlock = ConstU32<1024>;
    type MaxOpenChallenges = ConstU32<64>;
}

// === The Cerulea state module ===
// The state module is the universal typed store. Every write resolves
// permission, validates against the registered schema, writes the record
// and maintains every declared index. The permission module joins the
// runtime later, so the resolver refuses every request until it does.
impl pallet_cerulea_state::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = pallet_cerulea_state::weights::CeruleaWeight<Runtime>;
    type Schema = crate::Schema;
    type Permissions = crate::Permission;
    type Time = crate::Timestamp;
    type MaxRecordSize = ConstU32<65536>;
    type MaxRangeRead = ConstU32<1024>;
    type MaxRecordsPerCollection = ConstU32<1048576>;
    type MaxIndexEntries = ConstU32<4096>;
    type MaxIndexesPerCollection = ConstU32<16>;
    type MaxBucketEntries = ConstU32<256>;
    type MaxVersionHistory = ConstU32<32>;
    type MaxWatchedCollections = ConstU32<512>;
    type MaxRetentionScan = ConstU32<64>;
    type Custody = crate::Custody;
}

// === The Cerulea collections module ===
// The collections module holds the privacy class of every collection. A
// class change rewrites every record into the new form in pages and
// rotates the content key.
impl pallet_cerulea_collections::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = pallet_cerulea_collections::weights::CeruleaWeight<Runtime>;
    type Schema = crate::Schema;
    type Records = crate::State;
    type MaxRecordsPerPage = ConstU32<64>;
    type MaxKeyHolders = ConstU32<64>;
    type MaxWrappedKeyBytes = ConstU32<128>;
    type MaxPayloadBytes = ConstU32<65536>;
    type KeyHistoryDepth = ConstU32<2>;
}

// === The Cerulea audit module ===
// The audit module holds the typed event log of every deployment. Cerulea
// Explorer, Cerulea Dashboard and every external subscriber read the log.
// Every other platform module writes into the log through the audit trait.
// The log is append only. A correction appends a further entry naming the
// entry it corrects.
impl pallet_cerulea_audit::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = pallet_cerulea_audit::weights::CeruleaWeight<Runtime>;
    type Time = crate::Timestamp;
    type MaxPayloadBytes = ConstU32<16384>;
    type MaxEventDefinitions = ConstU32<1024>;
    type MaxSubscribedEvents = ConstU32<64>;
    type MaxReadBatch = ConstU32<512>;
}

// === The Cerulea identity module ===
// The identity module holds the accounts, the organisation tree, the role
// definitions, the credential attestations and the delegation chains of
// every deployment.
impl pallet_cerulea_identity::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = pallet_cerulea_identity::weights::CeruleaWeight<Runtime>;
    type Time = crate::Timestamp;
    type MaxUnitsPerDeployment = ConstU32<4096>;
    type MaxRolesPerDeployment = ConstU32<1024>;
    type MaxRolesPerAccount = ConstU32<64>;
    type MaxCredentialsPerRole = ConstU32<16>;
    type MaxUnitDepth = ConstU32<16>;
    type MaxTextBytes = ConstU32<256>;
    type MaxCredentialsPerAccount = ConstU32<64>;
    type MaxScopePerDelegation = ConstU32<64>;
    type MaxDelegationsPerAccount = ConstU32<128>;
    type MaxDelegationDepth = ConstU32<8>;
}

// === The Cerulea workflow module ===
// The workflow module drives the long-running processes of a deployment.
// A machine definition comes from the Workflows section of the Blueprint.
// Guards run through the metered expression evaluator of the Blueprint.
// Timers and deadlines enter one schedule held in due order, and the block
// hook runs a bounded count of steps. A failed process runs its
// compensating steps in reverse order, a bounded count per block.
// The permission module resolves every change the module makes to a
// record.
impl pallet_cerulea_workflow::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = pallet_cerulea_workflow::weights::CeruleaWeight<Runtime>;
    type Schema = crate::Schema;
    type Records = crate::State;
    type Identities = crate::Identity;
    type Permissions = crate::Permission;
    type Guards = pallet_cerulea_workflow::MeteredGuards;
    type Actions = pallet_cerulea_workflow::QueuedActions<Runtime>;
    type Audit = crate::Audit;
    type Time = crate::Timestamp;
    type MaxMachineBytes = ConstU32<262144>;
    type MaxMachinesPerDeployment = ConstU32<256>;
    type MaxStepsPerInstance = ConstU32<256>;
    type MaxTimersPerState = ConstU32<16>;
    type MaxScheduledPerBlock = ConstU32<64>;
    type MaxCompensationStepsPerBlock = ConstU32<16>;
    type MaxCompensatingInstances = ConstU32<256>;
    type GuardStepLimit = ConstU64<1000000>;
}

// === The Cerulea rules module ===
// The rules module evaluates the two kinds of condition a Blueprint
// declares. A precondition gates one action and evaluates before the
// action runs. An invariant holds over one collection and evaluates after
// every state change inside the transaction. A failed rule answers with
// the two plain sentences the person wrote beside the rule in the Studio.
impl pallet_cerulea_rules::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = pallet_cerulea_rules::weights::CeruleaWeight<Runtime>;
    type Records = crate::State;
    type Audit = crate::Audit;
    type Time = crate::Timestamp;
    type MaxRuleBytes = ConstU32<65536>;
    type MaxRulesPerDeployment = ConstU32<1024>;
    type MaxRulesPerCollection = ConstU32<64>;
    type RuleStepLimit = ConstU64<1000000>;
}

// === The Cerulea permission module ===
// The permission module holds the capability matrix. Every read and every
// write of the state module passes through resolution here.
impl pallet_cerulea_permission::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = pallet_cerulea_permission::weights::CeruleaWeight<Runtime>;
    type Schema = crate::Schema;
    type Identity = crate::Identity;
    type MaxEntryBytes = ConstU32<65536>;
    type MaxEntriesPerPair = ConstU32<64>;
    type RowStepLimit = ConstU64<100000>;
}

// === The Cerulea governance module ===
// The governance module holds the change process of a deployed system.
// The scope covers a schema change, a permission change, a policy
// amendment, a validator set change and an upgrade approval. Each kind
// carries a class of work, and each class carries a threshold. The
// Governance section of the Blueprint raises a threshold above the floor
// of its class. The electorate is the weighted register the module keeps.
impl pallet_cerulea_governance::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = pallet_cerulea_governance::weights::CeruleaWeight<Runtime>;
    type Roll = pallet_cerulea_governance::RegisteredElectorate<Runtime>;
    type Audit = crate::Audit;
    type Time = crate::Timestamp;
    type MaxPayloadBytes = ConstU32<262144>;
    type MaxAuthorityRoles = ConstU32<64>;
    type MaxVoteSpan = ConstU64<2592000000>;
}

// === The Cerulea logic module ===
// The logic module is the Blueprint interpreter. Every operation is
// metered, deterministic and bounded.
impl pallet_cerulea_logic::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = pallet_cerulea_logic::weights::CeruleaWeight<Runtime>;
    type Schema = crate::Schema;
    type Records = crate::State;
    type Permissions = crate::Permission;
    type Identity = crate::Identity;
    type MaxBlueprintBytes = ConstU32<4194304>;
    type MaxPayloadBytes = ConstU32<262144>;
    type StepLimit = ConstU64<2000000>;
    type MaxRowsPerRead = ConstU32<4096>;
    type MaxEffectsPerAction = ConstU32<256>;
    type Workflows = WorkflowTransitions;
}

/// The workflow sink the logic module drives.
///
/// An action effect naming a transition moves one record to another state.
/// The adapter finds the running process of that record, names the
/// transition leading to the target state and asks the workflow module to
/// take it. The workflow module checks the role, the permission and the
/// guard before the record moves.
pub struct WorkflowTransitions;

impl pallet_cerulea_logic::TransitionSink for WorkflowTransitions {
    type AccountId = AccountId;

    fn transition(
        deployment: &pallet_cerulea_schema::primitives::DeploymentId,
        workflow: &cerulea_blueprint::types::WorkflowId,
        collection: &cerulea_blueprint::types::CollectionId,
        key: &pallet_cerulea_schema::primitives::RecordKey,
        to: &cerulea_blueprint::types::StateId,
        actor: &Self::AccountId,
    ) -> frame_support::pallet_prelude::DispatchResult {
        use pallet_cerulea_schema::primitives::name_of;
        let name = name_of(workflow.as_str()).map_err(|_| {
            sp_runtime::DispatchError::Other(
                "The workflow name runs past the limit. Shorten the workflow name in the Blueprint.",
            )
        })?;
        let machine = pallet_cerulea_workflow::Pallet::<Runtime>::machine(deployment, &name)?;
        if machine.collection != *collection {
            return Err(sp_runtime::DispatchError::Other(
                "The workflow drives another collection. Name the workflow that drives this collection.",
            ));
        }

        let mut found: Option<(u64, alloc::string::String)> = None;
        for (id, instance) in
            pallet_cerulea_workflow::Instances::<Runtime>::iter_prefix((*deployment, name.clone()))
        {
            if &instance.record != key {
                continue;
            }
            if instance.status != pallet_cerulea_workflow::InstanceStatus::Running {
                continue;
            }
            let from =
                alloc::string::String::from_utf8_lossy(instance.state.as_slice()).into_owned();
            found = Some((id, from));
            break;
        }
        let (id, from) = found.ok_or(sp_runtime::DispatchError::Other(
            "The record runs no process of that workflow. Start the process before moving the record.",
        ))?;

        let step = machine
            .transitions
            .iter()
            .find(|held| held.from.as_str() == from && held.to == *to)
            .ok_or(sp_runtime::DispatchError::Other(
                "The workflow declares no transition from the present state to that state. Name a state the present state reaches.",
            ))?;
        let transition = name_of(step.id.as_str()).map_err(|_| {
            sp_runtime::DispatchError::Other(
                "The transition name runs past the limit. Shorten the transition name in the Blueprint.",
            )
        })?;
        pallet_cerulea_workflow::Pallet::<Runtime>::take_transition(
            deployment,
            &name,
            id,
            &transition,
            actor,
        )
    }
}

// === The Cerulea evolution module ===
// The evolution module orders the version transition of a deployment. The
// module holds the version registry, the migration definitions, the
// rollback windows and the applied migration log. A plan that removes a
// field, renames a field or retypes a field breaks an existing
// integration, so such a plan applies only after the electorate approved
// it at the structural class threshold.
impl pallet_cerulea_evolution::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = pallet_cerulea_evolution::weights::CeruleaWeight<Runtime>;
    type Schema = pallet_cerulea_evolution::SchemaBridge<Runtime>;
    type Approvals = crate::Governance;
    type Audit = crate::Audit;
    type Time = crate::Timestamp;
    type MaxPlanBytes = ConstU32<262144>;
    type MaxVersionsPerDeployment = ConstU32<1024>;
    type MaxRollbackWindow = ConstU64<2592000000>;
}

// === The Cerulea oracle module ===
// The oracle module carries external data into a deployment with an
// attestation behind every value. A feed names the accounts that attest
// and the count of attestations one value collects. A value reaching that
// count opens a dispute window. A value passing the window without an
// upheld dispute becomes the current value of the feed.
impl pallet_cerulea_oracle::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = pallet_cerulea_oracle::weights::CeruleaWeight<Runtime>;
    type Audit = crate::Audit;
    type Time = crate::Timestamp;
    type MaxAttestorsPerFeed = ConstU32<64>;
    type MaxValueBytes = ConstU32<65536>;
    type MaxFeedsPerDeployment = ConstU32<256>;
    type MaxDisputeWindow = ConstU64<604800000>;
}

// === The Cerulea bridge module ===
// The bridge module carries messages between this deployment and a named
// network outside it. Seven target networks ship together. Networks
// running Ethereum Virtual Machine bytecode, Cosmos, Solana, Polkadot,
// Avalanche, Bitcoin and Layer 2 rollups. Every arriving message carries
// a proof, and the module holds one verifier per network. A message lands
// once. The replay registry declines a second attempt. An arriving
// message runs its Blueprint action through the logic module, under the
// higher of the class the message carries and the class the endpoint
// carries.
impl pallet_cerulea_bridge::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = pallet_cerulea_bridge::weights::CeruleaWeight<Runtime>;
    type Records = crate::State;
    type Audit = crate::Audit;
    type Approvals = crate::Governance;
    type Actions = pallet_cerulea_bridge::LogicActions<Runtime>;
    type Time = crate::Timestamp;
    type MaxEndpointsPerDeployment = ConstU32<64>;
    type MaxHeadersPerEndpoint = ConstU32<4096>;
    type MaxPendingPerBucket = ConstU32<512>;
    type MaxSweepPerBlock = ConstU32<64>;
    type MaxBucketsPerBlock = ConstU32<16>;
    type MaxLifetimeMillis = ConstU64<604800000>;
}

// === The Cerulea tier module ===
// The tier module gates entitlement on the Blockchain as well as in
// Cerulea APIs. The module holds the deployment tier, the enabled feature
// set, the seat count, the add-on registry and the licence attestation
// with its expiry. The control plane signs every licence with an ed25519
// key and the deployment checks the signature against the stored key.
// A lapsed licence stops block production through the production gate the
// authoring engine consults. Every existing record stays readable and
// exportable throughout.
/// The accounts that administer each deployment.
///
/// The account that installed the Blueprint administers the deployment. So
/// does an active Cerulea operator of the deployment, and so does the account
/// whose public key is the control plane key of the deployment.
pub struct RuntimeStewards;

impl pallet_cerulea_schema::stewards::DeploymentStewards<AccountId> for RuntimeStewards {
    fn installer(
        deployment: &pallet_cerulea_schema::primitives::DeploymentId,
    ) -> Option<AccountId> {
        pallet_cerulea_logic::InstalledBy::<Runtime>::get(deployment)
            .or_else(|| pallet_cerulea_schema::Stewards::<Runtime>::get(deployment))
    }

    fn is_steward(
        deployment: &pallet_cerulea_schema::primitives::DeploymentId,
        who: &AccountId,
    ) -> bool {
        if Self::installer(deployment).as_ref() == Some(who) {
            return true;
        }
        if <crate::Operator as pallet_cerulea_operator::OperatorRole>::is_operator(deployment, who)
        {
            return true;
        }
        pallet_cerulea_tier::ControlPlaneKey::<Runtime>::get(deployment)
            .map(|key| <AccountId as AsRef<[u8; 32]>>::as_ref(who) == &key)
            .unwrap_or(false)
    }
}

impl pallet_cerulea_tier::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = pallet_cerulea_tier::weights::CeruleaWeight<Runtime>;
    type Audit = crate::Audit;
    type Stewards = RuntimeStewards;
    type Time = crate::Timestamp;
    type MaxFeatures = ConstU32<256>;
    type MaxAddons = ConstU32<64>;
    type MaxWatchedDeployments = ConstU32<64>;
}

// === The Cerulea relayer module ===
// The relayer module keeps the end-user experience free of gas, wallets
// and fees. The user authorises an action by signing a short statement.
// The relayer submits and carries the cost. The Blockchain records the
// original authoriser as the actor. Metering continues internally for
// quota enforcement while the user encounters no fee. Quota is per tenant
// and rate limits apply per account and per action.
impl pallet_cerulea_relayer::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = pallet_cerulea_relayer::weights::CeruleaWeight<Runtime>;
    type Identities = crate::Identity;
    type Entitlements = crate::Tier;
    type Stewards = RuntimeStewards;
    type Audit = crate::Audit;
    type Time = crate::Timestamp;
    type UnitsPerSubmission = ConstU64<1>;
    type MaxAuthorisationSpan = ConstU64<3600000>;
}

// === The Cerulea operator module ===
// Every customer deployment carries a Cerulea operator role. The role is
// mandatory and strictly limited. The role upgrades and downgrades the
// platform version, verifies the licence, stops block production on a
// lapse and pushes platform updates. The role reads no customer record,
// writes no customer data, holds no permission over customer governance
// and alters no Blueprint.
// The limits are enforced by the permission module rather than by policy.
// Every request names the collection it reaches, and the resolver refuses
// the operator role over any customer collection. An action outside the
// scope aborts and appears in the customer audit log with the actor, the
// timestamp and the reason.
impl pallet_cerulea_operator::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = pallet_cerulea_operator::weights::CeruleaWeight<Runtime>;
    type Permissions = crate::Permission;
    type Versions = crate::Evolution;
    type Licences = crate::Tier;
    type Audit = crate::Audit;
    type Time = crate::Timestamp;
}
