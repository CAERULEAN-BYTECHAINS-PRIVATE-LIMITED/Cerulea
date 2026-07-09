//! Mock runtime for CBC consensus tests

use super::*;
use crate::{ConsensusResult, ConsensusError, RealBlockImport, ConsensusParams};
use crate::types::AuthorSelectionMode;
use frame_support::{
    parameter_types,
    traits::{ConstU32, ConstU64, ConstU128, ConstU8},
    weights::Weight,
};
use sp_runtime::{
    traits::{BlakeTwo256, IdentityLookup},
    BuildStorage, Perbill,
};
use sp_core::H256;
use sp_keystore::{testing::MemoryKeystore, KeystoreExt};
use std::sync::Arc;
use std::pin::Pin;
use std::future::Future;
use futures::StreamExt;


// CBC consensus authority types
pub type AuthorityId = u64;

type Block = frame_system::mocking::MockBlock<Test>;

// Configure a mock runtime to test the consensus module
frame_support::construct_runtime!(
    pub enum Test
    {
        System: frame_system,
        Balances: pallet_balances,
        Timestamp: pallet_timestamp,
        DcfPallet: pallet_cbc_dcf,
        PosPallet: pallet_cbc_pos,
        PoiPallet: pallet_cbc_poi,
    }
);

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
    type AccountId = u64;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Block = Block;
    type RuntimeEvent = RuntimeEvent;
    type BlockHashCount = ConstU64<250>;
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = pallet_balances::AccountData<u128>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type SS58Prefix = ();
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

impl pallet_balances::Config for Test {
    type MaxLocks = ConstU32<50>;
    type MaxReserves = ();
    type ReserveIdentifier = [u8; 8];
    type Balance = u128;
    type RuntimeEvent = RuntimeEvent;
    type DustRemoval = ();
    type ExistentialDeposit = ConstU128<500>;
    type AccountStore = System;
    type WeightInfo = ();
    type FreezeIdentifier = ();
    type MaxFreezes = ();
    type RuntimeHoldReason = ();
    type RuntimeFreezeReason = ();
    type DoneSlashHandler = ();
}

impl pallet_timestamp::Config for Test {
    type Moment = u64;
    type OnTimestampSet = ();
    type MinimumPeriod = ConstU64<5>;
    type WeightInfo = ();
}

// CBC PoS pallet configuration
parameter_types! {
    pub const PosMaxValidators: u32 = 100;
    pub const PosMinStake: u128 = 1000;
}

impl pallet_cbc_pos::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type MaxValidators = PosMaxValidators;
    type MinStake = PosMinStake;
    type Balance = u128;
    type WeightInfo = ();
    type MinValidatorScore = ConstU32<50>;
    type MinActiveValidators = ConstU32<3>;
    type ValidatorScoreDecay = ConstU32<10>;
    type MaxSlashingCount = ConstU32<10>;
}

// CBC PoI pallet configuration
impl pallet_cbc_poi::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type MinInferenceConfidence = ConstU32<70>;
    type MaxInferenceAge = ConstU32<100>;
    type ChallengeWindow = ConstU32<50>;
    type InferenceReward = ConstU128<100>;
    type ChallengeReward = ConstU128<50>;
    type PosInterface = MockPosInterface;
    type DcfInterface = MockDcfInterface;
}

// Mock DCF pallet configuration
parameter_types! {
    pub const DcfMaxValidators: u32 = 100;
    pub const MaxEpochHistory: u32 = 24;
    pub const DefaultPosWeight: u64 = 6000;
    pub const DefaultPoiWeight: u64 = 4000;
    pub const MinActiveValidators: u32 = 3;
    pub const DcfMinValidatorScore: u32 = 50;
    pub const DcfValidatorScoreDecay: u32 = 10;
    pub const MaxValidatorScore: u64 = 100;
    pub const BlockAuthorshipBoost: u64 = 10;
    pub const MissedBlockPenalty: u64 = 5;
    pub const InferenceBoostLow: u64 = 2;
    pub const InferenceBoostMedium: u64 = 5;
    pub const InferenceBoostHigh: u64 = 10;
    pub const InferencePenaltyLow: u64 = 1;
    pub const InferencePenaltyMedium: u64 = 3;
    pub const InferencePenaltyHigh: u64 = 7;
    pub const DcfMinStake: u128 = 1000;
}

// Mock weight info for DCF pallet
pub struct MockWeightInfo;
impl pallet_cbc_dcf::WeightInfo for MockWeightInfo {
    fn on_initialize() -> Weight { Weight::from_parts(10_000, 0) }
    fn offchain_worker() -> Weight { Weight::from_parts(10_000, 0) }
    fn update_validator_stake_score() -> Weight { Weight::from_parts(10_000, 0) }
    fn update_validator_inference_score() -> Weight { Weight::from_parts(10_000, 0) }
    fn update_consensus_weights() -> Weight { Weight::from_parts(10_000, 0) }
    fn set_governance_mode() -> Weight { Weight::from_parts(10_000, 0) }
    fn sudo_advance_epoch() -> Weight { Weight::from_parts(50_000, 0) }
    fn submit_proposal() -> Weight { Weight::from_parts(20_000, 0) }
    fn vote_proposal() -> Weight { Weight::from_parts(15_000, 0) }
    fn execute_proposal() -> Weight { Weight::from_parts(30_000, 0) }
    fn join_validator_set() -> Weight { Weight::from_parts(25_000, 0) }
    fn leave_validator_set() -> Weight { Weight::from_parts(25_000, 0) }
    fn set_validator_name() -> Weight { Weight::from_parts(15_000, 0) }
    fn apply_offchain_poi_scores() -> Weight { Weight::from_parts(40_000, 0) }
    fn on_initialize_with_validators(_v: u32) -> Weight { Weight::from_parts(10_000, 0) }
    fn apply_score_decay_multiple(_v: u32) -> Weight { Weight::from_parts(5_000, 0) }
    fn validator_set_operations(_v: u32) -> Weight { Weight::from_parts(3_000, 0) }
    fn runtime_api_calls(_v: u32) -> Weight { Weight::from_parts(2_000, 0) }
    fn epoch_transition_multiple(_v: u32) -> Weight { Weight::from_parts(15_000, 0) }
    fn governance_with_multiple_voters(_v: u32) -> Weight { Weight::from_parts(8_000, 0) }
    fn join_validators() -> Weight { Weight::from_parts(10_000, 0) }
    fn leave_validators() -> Weight { Weight::from_parts(10_000, 0) }
    fn slash_validator() -> Weight { Weight::from_parts(10_000, 0) }
    fn cancel_leave_request() -> Weight { Weight::from_parts(10_000, 0) }
    fn report_validator_misbehavior() -> Weight { Weight::from_parts(10_000, 0) }
    fn simulate_inference() -> Weight { Weight::from_parts(10_000, 0) }
    fn propose_slash_validator() -> Weight { Weight::from_parts(10_000, 0) }
    fn propose_reward_validator() -> Weight { Weight::from_parts(10_000, 0) }
    fn propose_eject_validator() -> Weight { Weight::from_parts(10_000, 0) }
    fn increase_validator_stake() -> Weight { Weight::from_parts(10_000, 0) }
    fn decrease_validator_stake() -> Weight { Weight::from_parts(10_000, 0) }
    fn slash_validator_percentage() -> Weight { Weight::from_parts(10_000, 0) }
    fn slash_multiple_validators() -> Weight { Weight::from_parts(10_000, 0) }
    fn execute_proposals() -> Weight { Weight::from_parts(10_000, 0) }
    fn epoch_transition() -> Weight { Weight::from_parts(10_000, 0) }
    fn set_validator_metadata() -> Weight { Weight::from_parts(10_000, 0) }
    fn update_validator_activity() -> Weight { Weight::from_parts(10_000, 0) }
    fn distribute_epoch_rewards() -> Weight { Weight::from_parts(10_000, 0) }
    fn propose_default_reward_validator() -> Weight { Weight::from_parts(10_000, 0) }
    fn propose_default_reward_multiple_validators() -> Weight { Weight::from_parts(10_000, 0) }
    fn propose_reward_all_active_validators() -> Weight { Weight::from_parts(10_000, 0) }
    fn propose_reward_multiple_validators(_v: u32) -> Weight { Weight::from_parts(10_000, 0) }
}

impl pallet_cbc_dcf::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type MaxValidators = DcfMaxValidators;
    type MaxEpochHistory = MaxEpochHistory;
    type DefaultPosWeight = DefaultPosWeight;
    type DefaultPoiWeight = DefaultPoiWeight;
    type MinActiveValidators = MinActiveValidators;
    type MinValidatorScore = DcfMinValidatorScore;
    type ValidatorScoreDecay = DcfValidatorScoreDecay;
    type MaxValidatorScore = MaxValidatorScore;
    type BlockAuthorshipBoost = BlockAuthorshipBoost;
    type MissedBlockPenalty = MissedBlockPenalty;
    type InferenceBoostLow = InferenceBoostLow;
    type InferenceBoostMedium = InferenceBoostMedium;
    type InferenceBoostHigh = InferenceBoostHigh;
    type InferencePenaltyLow = InferencePenaltyLow;
    type InferencePenaltyMedium = InferencePenaltyMedium;
    type InferencePenaltyHigh = InferencePenaltyHigh;
    type MinStake = DcfMinStake;
    type Balance = u128;
    type Currency = Balances;
    type WeightInfo = MockWeightInfo;

    // Additional required parameters
    type MaxValidatorHistorySize = ConstU32<10>;
    type MaxValidatorNameSize = ConstU32<32>;
    type MaxRuntimeApiBoundedVecSize = ConstU32<100>;
    type MaxProposalActionBoundedVecSize = ConstU32<100>;
    type AuthorNotActiveErrorCode = ConstU8<1>;
    type AuthorMismatchErrorCode = ConstU8<2>;
    type MaxInactiveEpochs = ConstU32<5>;
    type ScoreDecayInterval = ConstU32<10>;
    type ParticipationUpdateInterval = ConstU32<100>;
    type UnderperformanceCheckInterval = ConstU32<50>;
    type ValidatorProposalInterval = ConstU32<200>;
    type HealthMetricsInterval = ConstU32<1000>;
    type OffchainWorkerInterval = ConstU32<5>;
    type LeaveCooldown = ConstU32<1000>;
    type EpochLength = ConstU32<2400>;
    type PercentagePrecision = ConstU32<10000>;
    type TrustScoreUptimeWeight = ConstU64<4000>;
    type TrustScoreInferenceWeight = ConstU64<4000>;
    type TrustScoreSlashingWeight = ConstU64<2000>;
    type MaxTrustScore = ConstU64<10000>;
    type MaxEvidenceLength = ConstU32<1000>;
    type MisbehaviorSlashThreshold = ConstU32<3>;
    type OffchainWorkerTimeout = ConstU64<5000>;
    type EstimatedBlockTime = ConstU64<6000>;
    type MinPerformanceScore = ConstU64<30>;
    type HighPerformanceScore = ConstU64<80>;
    type MinParticipationRate = ConstU32<50>;
    type HighParticipationRate = ConstU32<90>;
    type MaxMissedBlocks = ConstU32<10>;
    type MaxMissedBlocksHigh = ConstU32<2>;
    type HealthyValidatorScore = ConstU64<50>;
    type HealthyParticipationRate = ConstU32<80>;
    type HealthyMissedBlocksMax = ConstU32<5>;
    type ScoreChangeThreshold = ConstU64<1000>;
    type ScoreChangePercentage = ConstU32<10>;
    type ScoreImprovementThreshold = ConstU64<1000>;
    type ScoreImprovementPercentage = ConstU32<10>;
    type MaxPosContribution = ConstU32<90>;
    type MaxPoiContribution = ConstU32<90>;
    type ImbalanceWarningThreshold = ConstU32<85>;
    type LeaveRequestCheckInterval = ConstU32<10>;
    type MetricsUpdateInterval = ConstU32<10>;
    type ScoreRefreshInterval = ConstU32<50>;
    type DetailedLoggingInterval = ConstU32<100>;
    type ImbalanceCheckInterval = ConstU32<500>;
    type TopValidatorsDisplayCount = ConstU32<5>;
    type HealthCheckSampleSize = ConstU32<5>;
    type InferenceConfidenceThresholdLow = ConstU32<70>;
    type InferenceConfidenceThresholdHigh = ConstU32<90>;
    type MaxSlashPenalty = ConstU64<50>;
    type MaxRewardBoost = ConstU64<20>;
    type SlashPenaltyDivisor = ConstU64<1000>;
    type RewardBoostDivisor = ConstU64<1000>;
    type SlashPercent = ConstU32<10>;
    type ValidatorReward = ConstU128<10000>;
    type MaxValidatorNameLength = ConstU32<32>;
    type MaxValidatorWebsiteLength = ConstU32<64>;
    type MaxValidatorContactLength = ConstU32<64>;
    type MaxValidatorDescriptionLength = ConstU32<128>;
    type MaxValidatorLocationLength = ConstU32<32>;
    type MaxPerformanceHistoryLength = ConstU32<100>;
    type MaxValidatorHistoryLength = ConstU32<10>;
    type MaxCommissionRate = ConstU32<10000>;
    type FullPercentage = ConstU32<100>;
    type HighPerformancePercentage = ConstU32<80>;
    type TopPerformerPercentage = ConstU32<20>;
    type BaseRewardPercentage = ConstU32<60>;
    type PerformanceRewardPercentage = ConstU32<25>;
    type TopPerformerRewardPercentage = ConstU32<15>;
    type MaxSlashPerEpoch = ConstU128<1000>;
    type MaxSlashPerValidator = ConstU128<500>;
    type MaxRewardPerEpoch = ConstU128<2000>;
    type MaxRewardPerValidator = ConstU128<1000>;
    type MinTrustScore = ConstU64<0>;
    type MaxTrustScoreGrowthRate = ConstU32<20>;
    type MaxTrustScoreDecayRate = ConstU32<10>;
    type TrustScoreStabilityFactor = ConstU32<5>;
}

use frame_support::traits::ConstBool;

// Type aliases for test infrastructure - simplified for compilation
pub type TestBlock = frame_system::mocking::MockBlock<Test>;
pub type TestClient = ();
pub type DcfBlockImport = ();
pub type MockTransactionPool = ();

// Helper functions for test infrastructure - simplified for compilation
pub fn create_test_client() -> TestClient {
    ()
}

pub fn create_test_consensus_params() -> ConsensusParams {
    ConsensusParams {
        author_selection_mode: AuthorSelectionMode::RoundRobin,
        finality_threshold: 10,
        block_time: 6,
        max_block_size: 1024 * 1024,
        max_transactions_per_block: 1000,
        slot_duration: std::time::Duration::from_secs(6),
        min_block_time: 6000,
        metrics_update_interval: 10,
        score_refresh_interval: 50,
        consensus_loop_interval: 1000,
        detailed_logging_interval: 100,
        health_check_interval: 1000,
        min_performance_score: 30,
        high_performance_score: 80,
        min_participation_rate: 50,
        high_participation_rate: 90,
        max_missed_blocks: 10,
        max_missed_blocks_high: 2,
        healthy_validator_score: 50,
        healthy_participation_rate: 80,
        healthy_missed_blocks_max: 5,
        top_validators_display_count: 5,
        health_check_sample_size: 5,
    }
}

// Mock interfaces for CBC pallets
pub struct MockPosInterface;
impl pallet_cbc_poi::PosInterface<u64> for MockPosInterface {
    fn boost_score(_validator: &u64, _weight: u32) -> frame_support::dispatch::DispatchResult {
        Ok(())
    }
    
    fn slash_score(_validator: &u64, _weight: u32) -> frame_support::dispatch::DispatchResult {
        Ok(())
    }
}

pub struct MockDcfInterface;
impl pallet_cbc_poi::DcfInterface<u64> for MockDcfInterface {
    fn record_inference_activity(_validator: &u64) -> frame_support::dispatch::DispatchResult {
        Ok(())
    }
}

// Helper functions for testing
pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut storage = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    pallet_balances::GenesisConfig::<Test> {
        balances: vec![
            (1, 10000),
            (2, 10000),
            (3, 10000),
            (4, 10000),
        ],
        dev_accounts: None,
    }
    .assimilate_storage(&mut storage)
    .unwrap();

    let mut ext = sp_io::TestExternalities::from(storage);
    
    // Setup keystore for consensus testing
    let keystore = MemoryKeystore::new();
    ext.register_extension(KeystoreExt(Arc::new(keystore)));
    
    ext
}

pub fn new_test_ext_with_validators(validators: Vec<u64>) -> sp_io::TestExternalities {
    let mut ext = new_test_ext();
    
    ext.execute_with(|| {
        // Initialize CBC consensus with validators
        let bounded_validators = frame_support::BoundedVec::try_from(validators.clone())
            .expect("Too many validators for test");
        pallet_cbc_dcf::ValidatorSet::<Test>::put(bounded_validators.clone());
        pallet_cbc_dcf::ActiveValidators::<Test>::put(bounded_validators);
    });
    
    ext
}

// Helper to create test authorities (CBC consensus uses validator IDs)
pub fn create_test_authorities(count: u32) -> Vec<u64> {
    (1..=count as u64).collect()
}

// Helper to create test validator set
pub fn create_test_validator_set(count: u32) -> Vec<u64> {
    (1..=count as u64).collect()
}

// Mock consensus configuration
pub struct MockConsensusConfig {
    pub slot_duration: u64,
    pub epoch_length: u32,
    pub validators: Vec<u64>,
}

impl Default for MockConsensusConfig {
    fn default() -> Self {
        Self {
            slot_duration: 6000, // 6 seconds
            epoch_length: 2400,  // 4 hours at 6s per block
            validators: create_test_validator_set(4),
        }
    }
}

// Helper to setup consensus test environment
pub fn setup_consensus_test(config: MockConsensusConfig) -> sp_io::TestExternalities {
    let mut ext = new_test_ext_with_validators(config.validators.clone());
    
    ext.execute_with(|| {
        // Setup DCF pallet with test validators
        let genesis_config = pallet_cbc_dcf::GenesisConfig::<Test> {
            validators: config.validators.clone(),
            validator_scores: vec![6000; config.validators.len()],
            validator_stakes: vec![1000; config.validators.len()],
            validator_names: vec![],
            current_epoch: 0,
            epoch_config: pallet_cbc_dcf::EpochConfig {
                blocks_per_epoch: config.epoch_length,
                min_stake: 1000,
                max_validators: 100,
            },
            strict_validation: false,
        };
        
        // Manually initialize DCF storage
        for (i, validator) in config.validators.iter().enumerate() {
            let validator_state = pallet_cbc_dcf::ValidatorState {
                last_active_epoch: 0,
                current: pallet_cbc_dcf::EpochStats {
                    epoch: 0,
                    stake_score: 80,
                    inference_score: 70,
                    final_score: 75,
                    authored_blocks: 0,
                    missed_blocks: 0,
                },
                history: frame_support::BoundedVec::default(),
                uptime: 0,
                inference_success_count: 0,
                participation_rate: 100,
                inference_count: 0,
                last_active_block: 0,
                name: None,
                trust_score: 0,
            };
            
            pallet_cbc_dcf::ValidatorStates::<Test>::insert(validator, validator_state);
            pallet_cbc_dcf::ValidatorStake::<Test>::insert(validator, 1000u128);
        }
        
        let bounded_validators = frame_support::BoundedVec::try_from(config.validators.clone())
            .expect("Too many validators for test");
        pallet_cbc_dcf::ValidatorSet::<Test>::put(bounded_validators.clone());
        pallet_cbc_dcf::ActiveValidators::<Test>::put(bounded_validators);
        
        pallet_cbc_dcf::CurrentEpoch::<Test>::put(0);
        pallet_cbc_dcf::EpochConfigStorage::<Test>::put(pallet_cbc_dcf::EpochConfig {
            blocks_per_epoch: config.epoch_length,
            min_stake: 1000,
            max_validators: 100,
        });
        
        // Initialize consensus weights
        pallet_cbc_dcf::PosWeight::<Test>::put(60u64);
        pallet_cbc_dcf::PoiWeight::<Test>::put(40u64);
    });
    
    ext
}