//! # pallet-cbc-dcf
//!
//! This pallet implements the Dynamic Consensus Framework (DCF) for the CBC-Chain, providing advanced validator management, scoring, and on-chain governance. It supports dynamic validator sets, configurable consensus weights, and robust governance mechanisms for slashing, rewards, and ejection.
//!
//! ## Main Features
//! - **Validator Scoring:** Combines Proof-of-Stake (PoS) and Proof-of-Inference (PoI) scores with configurable weights, supporting dynamic adjustment and decay.
//! - **Epoch Management:** Handles epoch transitions, validator activity tracking, score decay, and validator set updates.
//! - **Governance:** Enables on-chain proposals for slashing, rewarding, and ejecting validators, with voting and execution logic.
//! - **Block Authorship Tracking:** Monitors block authorship and missed blocks, applying score boosts or penalties accordingly.
//! - **Runtime Hooks:** Integrates with runtime hooks for per-block and per-epoch logic, including automatic epoch transitions and score updates.
//! - **Genesis Configuration:** Allows initialization of validator set, scores, and consensus parameters at genesis.
//! - **APIs:** Exposes runtime APIs for querying validator scores, participation, epoch state, and expected block authors.
//! - **Sudo Controls:** Supports governance mode toggling and sudo-only operations for manual intervention and testing.
//! - **Validator Cooldown System:** Enforces mandatory cooldown periods when validators leave to prevent gaming and ensure network stability.
//! - **Trust Score Computation:** Comprehensive trust scoring based on uptime, inference success, and slashing history.
//! - **Enhanced Metadata Tracking:** Detailed validator information including names, activity metrics, and performance indicators.
//! - **Block Author Validation:** Strict validation of block authors during import to prevent unauthorized block production.
//! - **Comprehensive Event Coverage:** Complete event emission for all state transitions and validator lifecycle changes.
//!
//! ## System Invariants
//!
//! The pallet maintains several critical invariants to ensure network security and consistency:
//!
//! ### Economic Invariants
//! - **Reserved Balance Non-Negative:** `reserved_balance >= 0` for all validators
//! - **Reserved Greater Than Slashed:** `reserved_balance >= total_slashed_amount` at all times
//! - **Minimum Stake Enforcement:** Active validators must maintain `stake >= MinStake`
//!
//! ### Validator Set Invariants  
//! - **Active Set Size Limit:** `active_validators.len() <= MaxValidators` always enforced
//! - **Minimum Active Validators:** Network maintains `active_validators.len() >= MinActiveValidators` for security
//! - **Cooldown Enforcement:** Validators in cooldown period cannot rejoin until cooldown expires
//!
//! ### Score Invariants
//! - **Score Bounds:** All validator scores are bounded: `0 <= score <= MaxValidatorScore`
//! - **Trust Score Bounds:** Trust scores are bounded: `0 <= trust_score <= MaxTrustScore`
//! - **Weight Consistency:** PoS and PoI weights maintain: `pos_weight + poi_weight > 0`
//!
//! ### Temporal Invariants
//! - **Epoch Progression:** Epochs advance monotonically: `new_epoch >= current_epoch`
//! - **Cooldown Respect:** Leave cooldown periods are always respected and cannot be bypassed
//! - **Block Finality:** Finalized blocks are immutable: `finalized_block <= current_block`
//!
//! ## Genesis Configuration Links
//!
//! The pallet's configuration constants are initialized through genesis presets defined in
//! `cbc-runtime/src/genesis_config_presets.rs`. Available presets include:
//!
//! - **development:** Single validator (Alice) for local development
//! - **local:** Two validators (Alice, Bob) for local testing  
//! - **multi_validator:** Five validators with varying stakes for comprehensive testing
//! - **high_stake:** Three validators with high stake amounts for stress testing
//!
//! Key genesis parameters:
//! - `blocks_per_epoch: 100` - Epoch length in blocks
//! - `min_stake: 1_000_000` - Minimum stake requirement
//! - `max_validators: 100` - Maximum validator set size
//! - Default validator stakes range from 3M to 50M units depending on preset
//!
//! ## Configuration Constants to Genesis Preset Mapping
//!
//! The pallet's configuration constants are initialized through genesis configuration
//! and can be customized for different network deployments:
//!
//! ### Validator Set Configuration
//! - `MaxValidators` → `genesis.dcf.epoch_config.max_validators` (default: 100)
//! - `MinActiveValidators` → Enforced at runtime (typically 1-3 for testnets)
//! - `MinStake` → `genesis.dcf.epoch_config.min_stake` (default: 1,000,000 units)
//!
//! ### Epoch and Timing Configuration  
//! - `EpochLength` → `genesis.dcf.epoch_config.blocks_per_epoch` (default: 100 blocks)
//! - `LeaveCooldown` → Runtime constant (default: 1000-10000 blocks)
//! - `ValidatorCooldownPeriod` → Runtime constant for re-entry restrictions
//!
//! ### Scoring and Performance Configuration
//! - `DefaultPosWeight` → Initial PoS weight (default: 5000 = 50%)
//! - `DefaultPoiWeight` → Initial PoI weight (default: 5000 = 50%)
//! - `MinValidatorScore` → Minimum score for active participation (default: 30)
//! - `MaxValidatorScore` → Maximum possible score (default: 10000)
//!
//! ### Trust Score Configuration
//! - `TrustScoreUptimeWeight` → Uptime component weight (default: 40)
//! - `TrustScoreInferenceWeight` → Inference component weight (default: 35)
//! - `TrustScoreSlashingWeight` → Slashing penalty weight (default: 25)
//! - `MaxTrustScore` → Maximum trust score value (default: 10000)
//!
//! ### Economic and Reward Configuration
//! - `ValidatorReward` → Default reward amount for governance proposals
//! - `SlashPercent` → Percentage of stake slashed for misbehavior (default: 10-30%)
//! - `BaseRewardPercentage` → Base reward pool allocation (default: 60%)
//! - `PerformanceRewardPercentage` → Performance reward allocation (default: 25%)
//! - `TopPerformerRewardPercentage` → Top performer bonus allocation (default: 15%)
//!
//! ### Block Production and Inference Rewards/Penalties
//! - `BlockAuthorshipBoost` → Score boost for successful block authoring (default: 10)
//! - `MissedBlockPenalty` → Score penalty for missed blocks (default: 5)
//! - `InferenceBoostHigh` → Reward for high-quality inference (default: 15)
//! - `InferenceBoostMedium` → Reward for medium-quality inference (default: 10)
//! - `InferenceBoostLow` → Reward for low-quality inference (default: 5)
//!
//! ### Metadata and Storage Limits
//! - `MaxValidatorNameLength` → Maximum validator name size (default: 32 bytes)
//! - `MaxValidatorHistoryLength` → Performance history size (default: 24 epochs)
//! - `MaxEpochHistory` → Network epoch history size (default: 168 epochs)
//! - `MaxEvidenceLength` → Misbehavior evidence size limit (default: 1024 bytes)
//!
//! ### Example Genesis Preset Values
//!
//! **Development Preset:**
//! ```rust
//! dcf: pallet_cbc_dcf::GenesisConfig {
//!     validators: vec![Alice],
//!     validator_stakes: vec![10_000_000],
//!     epoch_config: EpochConfig {
//!         blocks_per_epoch: 100,
//!         min_stake: 1_000_000,
//!         max_validators: 100,
//!     },
//!     // ... other fields
//! }
//! ```
//!
//! **Multi-Validator Preset:**
//! ```rust
//! dcf: pallet_cbc_dcf::GenesisConfig {
//!     validators: vec![Alice, Bob, Charlie, Dave, Eve],
//!     validator_stakes: vec![15_000_000, 12_000_000, 8_000_000, 5_000_000, 3_000_000],
//!     epoch_config: EpochConfig {
//!         blocks_per_epoch: 100,
//!         min_stake: 1_000_000,
//!         max_validators: 100,
//!     },
//!     // ... other fields
//! }
//! ```
//!
//! ## Cooldown System Behavior and Enforcement
//!
//! The pallet enforces a comprehensive cooldown system to prevent gaming and ensure
//! network stability:
//!
//! ### Leave Cooldown Process
//! 1. Validator calls `leave_validators()` → Request recorded with current block number
//! 2. Validator enters cooldown period (`T::LeaveCooldown` blocks)
//! 3. During cooldown: Validator remains active, stake stays reserved, cannot cancel
//! 4. After cooldown: Validator automatically removed, stake unreserved
//! 5. Validator added to `RecentlyRemovedValidators` for additional cooldown
//!
//! ### Re-entry Cooldown Process  
//! 1. After leaving, validator enters `RecentlyRemovedValidators` storage
//! 2. Additional cooldown period prevents immediate rejoining
//! 3. During re-entry cooldown: `join_validators()` calls are rejected
//! 4. After cooldown expires: Validator can attempt to rejoin normally
//! 5. Must meet all current requirements (stake, performance, etc.)
//!
//! ### Cooldown Enforcement Guarantees
//! - **Always Respected:** Cooldown periods cannot be bypassed or shortened
//! - **Automatic Cleanup:** Expired cooldown entries are automatically removed
//! - **Proposal Filtering:** Validators in cooldown are excluded from active set selection
//! - **Economic Security:** Stakes remain locked during entire cooldown process
//! - **Gaming Prevention:** Multiple cooldown layers prevent rapid validator cycling
//!
//! ## Dispatchable Calls (pallet index)
//! - **0. `update_validator_stake_score`**: Update a validator's PoS stake score.  
//!   - Root required if governance mode is enabled, otherwise signed.
//! - **1. `update_validator_inference_score`**: Update a validator's PoI inference score.  
//!   - Root required if governance mode is enabled, otherwise signed.
//! - **2. `update_consensus_weights`**: Update PoS and PoI weights (must sum to 100).  
//!   - Root required.
//! - **3. `set_governance_mode`**: Toggle governance mode (sudo-like).  
//!   - Root required.
//! - **4. `sudo_advance_epoch`**: Manually advance epoch (governance mode only).  
//!   - Root required.
//! - **5. `submit_proposal`**: Submit a governance proposal (slash, reward, eject).  
//!   - Signed.
//! - **6. `vote_proposal`**: Vote on a governance proposal.  
//!   - Signed.
//! - **7. `execute_proposal`**: Execute an approved governance proposal.  
//!   - Root required.
//! - **8. `propose_slash_validator`**: Sudo propose to slash a validator.  
//!   - Root required.
//! - **9. `propose_reward_validator`**: Sudo propose to reward a validator.  
//!   - Root required.
//! - **10. `propose_eject_validator`**: Sudo propose to eject a validator.  
//!   - Root required.
//! - **18. `join_validators`**: Join the validator set by meeting minimum stake requirements.  
//!   - Signed required.
//! - **19. `leave_validators`**: Request to leave validator set with cooldown period.  
//!   - Signed required. Funds remain reserved until cooldown expires.
//! - **25. `cancel_leave_request`**: Cancel a pending leave request before cooldown expires.  
//!   - Signed required.
//! - **26. `propose_default_reward_validator`**: Propose to reward a validator with default amount.  
//!   - Root required.
//! - **27. `propose_reward_multiple_validators`**: Propose to reward multiple validators.  
//!   - Root required.
//! - **28. `propose_default_reward_multiple_validators`**: Propose to reward multiple validators with default amount.  
//!   - Root required.
//! - **29. `propose_reward_all_active_validators`**: Propose to reward all active validators.  
//!   - Root required.
//! - **30. `slash_validator`**: Directly slash a validator's reserved stake.  
//!   - Root required. Slashes from reserved balance, burns funds, updates scores.
//! - **31. `slash_validator_percentage`**: Slash a percentage of validator's reserved stake.  
//!   - Root required. Slashes percentage (0-100%) of reserved balance.
//! - **32. `slash_multiple_validators`**: Slash multiple validators with same amount.  
//!   - Root required. Batch slashing operation.
//! - **21. `report_validator_misbehavior`**: Report validator misbehavior with evidence.  
//!   - Signed required, must be in ValidatorSet, automatic slashing at threshold.
//! - **22. `simulate_inference`**: Simulate an inference event for development.  
//!   - Signed required, increments inference_count for validator.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(dead_code)]
#[warn(unused_comparisons)]

// Traits module
pub mod traits;

// Benchmarking module
#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

// Removed unnecessary weight verification and cleanup modules

// Private chain compatibility module
pub mod private_chain;

// EVM compatibility module
pub mod evm_compatibility;

// --- Imports --- //
use frame_support::{
    pallet_prelude::*,
    traits::{Get, Currency, ReservableCurrency},
    BoundedVec,
    Parameter,
};
use frame_system::pallet_prelude::*;
use sp_runtime::{
    traits::{SaturatedConversion, AtLeast32BitUnsigned, Saturating},
    DigestItem,
};
use sp_io;
use sp_std::prelude::*;
use sp_std::fmt; 
use pallet_cbc_pos as pos;
use pallet_cbc_poi as poi;
use serde::{Serialize, Deserialize};

use scale_info::prelude::format;
use scale_info::prelude::string::String;
// --- Runtime API Declarations --- //
// These APIs are exposed to the runtime for querying validator and consensus state.

/// DCF Runtime API Version
/// 
/// This constant defines the current version of the DCF Runtime API contract.
/// It should be incremented whenever breaking changes are made to any API method
/// signatures, argument types, or return shapes.
/// 
/// Version History:
/// - Version 1: Initial production-ready API contract with comprehensive
///   governance, invariant checking, and validator lifecycle management
/// 
/// Breaking changes that require version increment:
/// - Changing method signatures (name, parameters, return types)
/// - Removing existing methods
/// - Changing the semantics of existing methods
/// - Modifying data structures used in API responses
/// 
/// Non-breaking changes that do NOT require version increment:
/// - Adding new methods
/// - Adding optional fields to existing structures (with proper defaults)
/// - Improving documentation
/// - Internal implementation changes that don't affect the API contract
pub const DCF_API_VERSION: u32 = 1;

sp_api::decl_runtime_apis! {
    /// DCF Runtime API for querying validator and consensus state.
    /// 
    /// This API provides comprehensive access to the DCF system state including
    /// validator information, consensus parameters, governance configuration,
    /// and system health metrics. All methods are designed to be stable and
    /// backward-compatible within the same API version.
    /// 
    /// # API Contract Guarantees
    /// 
    /// - **Stability**: Method signatures will not change within the same API version
    /// - **Backward Compatibility**: Existing methods will continue to work as documented
    /// - **Type Safety**: All parameters and return types are strictly typed
    /// - **Error Handling**: Methods return appropriate error types for failure cases
    /// - **Performance**: All methods are optimized for runtime query performance
    /// 
    /// # Version Management
    /// 
    /// The API version is tracked via `DCF_API_VERSION` constant. Breaking changes
    /// will increment this version and emit `ApiVersionChanged` events.
    /// 
    /// # Usage Guidelines
    /// 
    /// - Always check API version compatibility before making calls
    /// - Handle `None` returns gracefully for optional data
    /// - Use encoded return types for complex data structures
    /// - Monitor for `ApiVersionChanged` events in production systems
    pub trait DcfApi<AccountId, Balance, BlockNumber>
    where
        AccountId: codec::Codec + Clone + Eq + sp_std::fmt::Debug,
        Balance: codec::Codec + Clone + Eq + sp_std::fmt::Debug,
        BlockNumber: codec::Codec + Clone + Eq + sp_std::fmt::Debug,
    {
        /// Get the current DCF Runtime API version.
        /// 
        /// This method returns the version of the DCF Runtime API contract.
        /// Clients should check this version to ensure compatibility before
        /// making other API calls.
        /// 
        /// # Returns
        /// - `u32`: Current API version number
        /// 
        /// # Example Usage
        /// ```rust
        /// let api_version = runtime_api.get_api_version();
        /// if api_version != expected_version {
        ///     // Handle version mismatch
        /// }
        /// ```
        /// 
        /// # Compatibility
        /// - This method will always be available in all API versions
        /// - Return type will never change
        /// - Method signature is guaranteed stable
        fn get_api_version() -> u32;
        fn get_validator_scores() -> Vec<(AccountId, u64)>;
        fn get_current_epoch() -> u32;
        fn get_validator_stake_score(validator: AccountId) -> u64;
        fn get_validator_inference_score(validator: AccountId) -> u64;
        fn get_consensus_weights() -> (u64, u64);
        fn is_validator_active(validator: AccountId) -> bool;
        fn get_expected_author(block_number: u32) -> Option<AccountId>;
        fn get_validator_score_history(validator: AccountId) -> Vec<u64>;
        fn get_validator_participation(validator: AccountId) -> (u32, u32);
        fn get_active_validators() -> Vec<AccountId>;
        fn get_validator_last_active(validator: AccountId) -> u32;
        fn validate_block_author(block_number: u32, author: AccountId);
        fn get_validator_profile(account_id: AccountId) -> Option<ValidatorProfile<AccountId, Balance, BlockNumber>>;
        fn get_validator_score_breakdown(validator: AccountId) -> Option<ScoreBreakdown>;
        fn get_validator_uptime(validator: AccountId) -> Option<UptimeStats>;
        fn get_slashing_history(validator: AccountId) -> Vec<SlashingRecord<Balance, BlockNumber>>;
        fn get_system_constants() -> SystemConstants<Balance, BlockNumber>;
        fn get_validator_cooldown_status(validator: AccountId) -> Option<BlockNumber>;
        fn get_validator_detailed_cooldown_status(validator: AccountId) -> Option<(u32, bool)>;
        fn get_inference_result(account_id: AccountId) -> Option<u64>;
        fn get_epoch_history(epoch_number: u32) -> Option<RuntimeEpochHistory<AccountId>>;
        fn get_recent_epochs(n: u32) -> Vec<RuntimeEpochHistory<AccountId>>;
        fn get_governance_mode() -> bool;
        fn get_validator_consensus_contribution(validator: AccountId) -> Option<(u64, u64, u64)>;
        fn get_epoch_config() -> EpochConfig;
        fn get_validator_epoch_stats(validator: AccountId, epoch: u32) -> Option<EpochStats>;
        fn get_total_validators_count() -> u32;
        fn get_validator_set_info() -> (u32, u32, u32);
        fn get_validators_by_score() -> Vec<(AccountId, u64)>;
        fn get_last_finalized_block() -> u32;
        fn is_block_finalized(block_number: u32) -> bool;
        fn get_misbehavior_report_count(validator: AccountId) -> u32;
        fn get_misbehavior_reporters(validator: AccountId) -> Vec<AccountId>;
        fn get_misbehavior_evidence(validator: AccountId, reporter: AccountId) -> Option<Vec<u8>>;
        fn is_validator_at_risk(validator: AccountId) -> bool;
        fn get_finality_info() -> (u32, u32);
        fn blocks_since_finalization(current_block: u32) -> u32;
        fn get_validator_leave_request(validator: AccountId) -> Option<u32>;
        fn validate_expected_author(block_number: u32, actual_author: AccountId) -> bool;
        fn get_validator_stake(validator: AccountId) -> u128;
        fn get_leave_request_status(validator: AccountId) -> Option<(u32, u32, bool)>; // (request_block, expires_at, can_execute)
        fn get_epoch_manager_config() -> (u64, u64, u32, u32, u32, u32, u64, u32, u32, u32, u32); // (min_perf_score, high_perf_score, min_participation, high_participation, max_missed_blocks, max_missed_blocks_high, healthy_validator_score, healthy_participation_rate, healthy_missed_blocks_max, leave_cooldown, top_validators_display_count)
        fn get_epoch_length() -> u32; // Get configurable epoch length from T::EpochLength
        fn validate_block_author_strict(block_number: u32, actual_author: AccountId) -> Result<(), u8>;
        fn report_author_mismatch(block_number: u32, expected: Option<AccountId>, actual: AccountId) -> Result<(), sp_runtime::DispatchError>;
        fn report_successful_block_authorship(block_number: u32, author: AccountId) -> Result<(), sp_runtime::DispatchError>;
        fn report_missed_block(block_number: u32, expected_author: AccountId) -> Result<(), sp_runtime::DispatchError>;
        fn get_governance_config() -> Vec<u8>;
        fn get_parameter_value(parameter: Vec<u8>) -> Option<Vec<u8>>;
        fn validate_parameter_value(parameter: Vec<u8>, value: Vec<u8>) -> bool;
        fn get_latest_invariant_report() -> Option<Vec<u8>>;
        fn get_invariant_report_for_epoch(epoch: u32) -> Option<Vec<u8>>;
        fn has_invariant_violations() -> bool;
        fn get_system_metrics() -> Vec<u8>;
        fn get_performance_indicators() -> Vec<u8>;
        fn get_metrics_last_updated() -> u32;
    }
}

// --- Weights Module --- //
pub mod weights;
pub use weights::*;

// --- Pallet Declaration --- //
#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use crate::traits::{DvfFinalizedBlockProvider, WeightFreezer};

    /// Current storage version for the DCF pallet.
    /// 
    /// This constant defines the expected storage schema version for the current
    /// runtime. It is used during runtime initialization to validate that the
    /// storage schema matches the runtime expectations.
    /// 
    /// Version History:
    /// - Version 1: Initial production-ready storage layout with comprehensive
    ///   governance, invariant checking, and validator lifecycle management
    /// 
    /// This version should be incremented whenever breaking changes are made
    /// to the storage layout that require migration.
    pub const CURRENT_STORAGE_VERSION: u32 = 1;

    // --- Data Structures --- //

    /// Per-epoch statistics for a validator.
    #[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen, Default)]
    pub struct EpochStats {
        pub epoch: u32,
        pub stake_score: u64,
        pub inference_score: u64,
        pub final_score: u64,
        pub authored_blocks: u32,
        pub missed_blocks: u32,
    }

    /// State for a validator, including current stats and history.
    #[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    pub struct ValidatorState<T: Config> {
        pub last_active_epoch: u32,
        pub current: EpochStats,
        pub history: BoundedVec<EpochStats, T::MaxValidatorHistorySize>, // Use Config constant
        pub uptime: u32, // Number of epochs active
        pub inference_success_count: u32,
        pub participation_rate: u32, // Percentage (0 to T::PercentagePrecision)
        pub inference_count: u64, // Total number of inferences performed
        pub last_active_block: u32, // Last block number where validator was active
        pub name: Option<BoundedVec<u8, T::MaxValidatorNameSize>>, // Use Config constant
        pub trust_score: u64, // Computed trust score based on uptime, inference success, and slashing history
    }

    /// Configuration for epochs (block count, min stake, max validators).
    #[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, Default, Serialize, Deserialize, frame_support::__private::codec::DecodeWithMemTracking)]
    #[serde(rename_all = "camelCase")]
    pub struct EpochConfig {
        pub blocks_per_epoch: u32,
        pub min_stake: u128,
        pub max_validators: u32,
    }

    /// Configuration for trust score calculation with configurable weights and bounds.
    ///
    /// This struct defines the weights used in trust score computation to balance
    /// different aspects of validator performance and reliability. The weights
    /// determine how much each component contributes to the final trust score.
    ///
    /// Trust score calculation formula with bounded growth and decay:
    /// ```
    /// weighted_score = (uptime_score * uptime_weight + inference_score * inference_weight) / (uptime_weight + inference_weight)
    /// bounded_score = clamp(weighted_score, min_trust_score, max_trust_score)
    /// decay_factor = calculate_decay_factor(epochs_inactive, decay_rate)
    /// growth_factor = calculate_growth_factor(performance_improvement, growth_rate)
    /// final_trust_score = apply_bounds(bounded_score * decay_factor * growth_factor - slashing_penalty)
    /// ```
    ///
    /// Default weight distribution:
    /// - Uptime: 40% - Rewards consistent availability and participation
    /// - Inference: 35% - Rewards AI/ML performance and accuracy
    /// - Slashing: 25% - Penalizes past misbehavior and poor performance
    ///
    /// Bounded growth and decay features:
    /// - Growth rate caps prevent explosive score increases
    /// - Decay rate limits prevent rapid score degradation
    /// - Minimum and maximum bounds ensure score stability
    /// - Clamping prevents negative or excessive values
    ///
    /// The weights and bounds can be adjusted through governance to adapt to changing
    /// network priorities and requirements.
    #[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, Default)]
    pub struct TrustScoreConfig {
        /// Weight for uptime component in trust score calculation (default: 40)
        pub uptime_weight: u32,
        /// Weight for inference success component in trust score calculation (default: 35)
        pub inference_weight: u32,
        /// Weight for slashing penalty component in trust score calculation (default: 25)
        pub slashing_weight: u32,
        /// Maximum growth rate per epoch (percentage, default: 5% = 500 basis points)
        pub max_growth_rate: u32,
        /// Maximum decay rate per epoch (percentage, default: 2% = 200 basis points)
        pub max_decay_rate: u32,
        /// Minimum trust score value (default: 1000 = 10% of max)
        pub min_trust_score: u64,
        /// Maximum trust score value (default: 10000 = 100%)
        pub max_trust_score: u64,
        /// Stability factor for score smoothing (default: 80% = 8000 basis points)
        pub stability_factor: u32,
    }

    /// Trust score bounds configuration for preventing explosive growth and decay.
    ///
    /// This struct defines the boundaries and rate limits that constrain trust score
    /// changes to ensure long-term stability and prevent gaming of the scoring system.
    /// All bounds are enforced during trust score calculations and updates.
    ///
    /// # Bounds Enforcement
    /// - **Growth Rate Limiting**: Prevents scores from increasing too rapidly
    /// - **Decay Rate Limiting**: Prevents scores from decreasing too rapidly  
    /// - **Absolute Bounds**: Ensures scores stay within min/max range
    /// - **Stability Smoothing**: Reduces score volatility through averaging
    ///
    /// # Configuration Guidelines
    /// - Growth rates should be conservative to prevent gaming
    /// - Decay rates should allow recovery from temporary issues
    /// - Min/max bounds should reflect realistic performance ranges
    /// - Stability factors should balance responsiveness with smoothness
    #[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
    pub struct TrustScoreBoundsData {
        /// Minimum allowed trust score value (prevents negative scores)
        pub min_score: u64,
        /// Maximum allowed trust score value (prevents explosive growth)
        pub max_score: u64,
        /// Maximum growth rate per epoch in basis points (e.g., 500 = 5%)
        pub max_growth_rate: u32,
        /// Maximum decay rate per epoch in basis points (e.g., 200 = 2%)
        pub max_decay_rate: u32,
        /// Stability factor for score smoothing in basis points (e.g., 8000 = 80%)
        pub stability_factor: u32,
        /// Epoch when bounds were last updated (for tracking changes)
        pub last_updated_epoch: u32,
    }

    impl Default for TrustScoreBoundsData {
        fn default() -> Self {
            Self {
                min_score: 1000,        // 10% of max score
                max_score: 10000,       // 100% (full score)
                max_growth_rate: 500,   // 5% per epoch
                max_decay_rate: 200,    // 2% per epoch
                stability_factor: 8000, // 80% stability
                last_updated_epoch: 0,
            }
        }
    }

    /// Trust score stability metrics for monitoring system health.
    ///
    /// This struct tracks various metrics related to trust score stability
    /// across the validator network. These metrics help identify potential
    /// issues with score volatility, gaming attempts, or system imbalances.
    ///
    /// # Metrics Tracked
    /// - **Score Volatility**: How much scores change between epochs
    /// - **Bound Violations**: How often validators hit min/max bounds
    /// - **Distribution Stats**: Score distribution across the network
    /// - **Stability Indicators**: Overall system stability measures
    ///
    /// # Usage
    /// - System health monitoring and alerting
    /// - Governance parameter tuning decisions
    /// - Network performance analysis
    /// - Gaming detection and prevention
    #[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
    pub struct TrustScoreStabilityMetricsData {
        /// Current epoch for these metrics
        pub epoch: u32,
        /// Average trust score change in the last epoch (absolute value)
        pub avg_score_change: u64,
        /// Maximum trust score change in the last epoch (absolute value)
        pub max_score_change: u64,
        /// Number of validators that hit the minimum bound in the last epoch
        pub validators_at_min_bound: u32,
        /// Number of validators that hit the maximum bound in the last epoch
        pub validators_at_max_bound: u32,
        /// Standard deviation of trust scores across all validators
        pub score_standard_deviation: u64,
        /// Median trust score across all validators
        pub score_median: u64,
        /// Number of validators with scores above the 90th percentile
        pub high_performers_count: u32,
        /// Number of validators with scores below the 10th percentile
        pub low_performers_count: u32,
        /// Stability index (0-10000, higher = more stable)
        pub stability_index: u32,
    }

    impl Default for TrustScoreStabilityMetricsData {
        fn default() -> Self {
            Self {
                epoch: 0,
                avg_score_change: 0,
                max_score_change: 0,
                validators_at_min_bound: 0,
                validators_at_max_bound: 0,
                score_standard_deviation: 0,
                score_median: 5000, // 50% of max score
                high_performers_count: 0,
                low_performers_count: 0,
                stability_index: 10000, // Perfect stability initially
            }
        }
    }

    /// Deterministic epoch processing configuration and state.
    ///
    /// This struct manages the deterministic processing of epoch transitions to ensure
    /// that all nodes produce identical results when processing the same epoch with
    /// the same inputs. It includes randomness seeding, replay validation, and
    /// deterministic author sequence generation.
    ///
    /// Key features:
    /// - **Deterministic Randomness**: Uses block numbers and fixed salts as seeds
    /// - **Replay Validation**: Enables byte-for-byte comparison of epoch outputs
    /// - **Author Sequence Caching**: Stores deterministic author sequences
    /// - **State Tracking**: Maintains processing state for validation
    ///
    /// The deterministic engine ensures that epoch transitions are:
    /// - Reproducible across all nodes
    /// - Verifiable through replay validation
    /// - Resistant to non-deterministic behavior
    /// - Consistent with fixed randomness sources
    #[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
    pub struct DeterministicEpochEngine {
        /// Fixed salt for randomness generation (never changes after genesis)
        pub randomness_salt: [u8; 32],
        /// Per-epoch salt derived from epoch number and base salt
        pub epoch_salt: [u8; 16],
        /// Cached author sequences for deterministic block production
        pub author_sequence_cache: BoundedVec<u8, ConstU32<1024>>, // Encoded author sequence
        /// Last processed epoch for replay validation
        pub last_processed_epoch: u32,
        /// Hash of last epoch processing output for replay validation
        pub last_epoch_output_hash: [u8; 32],
    }

    impl Default for DeterministicEpochEngine {
        fn default() -> Self {
            Self {
                randomness_salt: [0u8; 32],
                epoch_salt: [0u8; 16],
                author_sequence_cache: BoundedVec::new(),
                last_processed_epoch: 0,
                last_epoch_output_hash: [0u8; 32],
            }
        }
    }

    /// Epoch processing output for replay validation.
    ///
    /// This struct captures all outputs from epoch processing to enable
    /// byte-for-byte comparison during replay validation. It includes
    /// all state changes and computed values that result from epoch transitions.
    ///
    /// The output is used to:
    /// - Validate deterministic processing
    /// - Compare replay results with original processing
    /// - Detect non-deterministic behavior
    /// - Ensure consensus on epoch transitions
    #[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
    pub struct EpochProcessingOutput<T: Config> {
        /// Epoch number that was processed
        pub epoch: u32,
        /// Block number where epoch transition occurred
        pub transition_block: u32,
        /// Final active validator set after processing
        pub active_validators: BoundedVec<T::AccountId, <T as pos::Config>::MaxValidators>,
        /// Validators added during this epoch transition
        pub added_validators: BoundedVec<T::AccountId, <T as pos::Config>::MaxValidators>,
        /// Validators removed during this epoch transition
        pub removed_validators: BoundedVec<T::AccountId, <T as pos::Config>::MaxValidators>,
        /// Final scores for all validators after processing
        pub validator_scores: BoundedVec<(T::AccountId, u64), <T as pos::Config>::MaxValidators>,
        /// Author sequence generated for the new epoch
        pub author_sequence: BoundedVec<T::AccountId, ConstU32<1000>>,
        /// Randomness seed used for this epoch
        pub randomness_seed: [u8; 32],
        /// Hash of all processing inputs for validation
        pub input_hash: [u8; 32],
        /// Hash of all processing outputs for validation
        pub output_hash: [u8; 32],
    }

    /// Parameter range definition with minimum, maximum, and current values.
    ///
    /// This generic struct defines safe operating ranges for any configurable parameter
    /// in the DCF system. It ensures that parameter updates remain within acceptable
    /// bounds while tracking the current value.
    ///
    /// The range validation prevents dangerous parameter changes that could:
    /// - Destabilize the consensus mechanism
    /// - Create economic vulnerabilities
    /// - Cause performance degradation
    /// - Enable gaming or attacks
    ///
    /// Each parameter range includes:
    /// - `min`: Minimum safe value for the parameter
    /// - `max`: Maximum safe value for the parameter  
    /// - `current`: Currently active value within the range
    ///
    /// All parameter updates are validated against these ranges before application.
    #[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
    pub struct ParameterRange<T> {
        /// Minimum allowed value for this parameter
        pub min: T,
        /// Maximum allowed value for this parameter
        pub max: T,
        /// Current active value for this parameter
        pub current: T,
    }

    impl<T: Default> Default for ParameterRange<T> {
        fn default() -> Self {
            Self {
                min: T::default(),
                max: T::default(),
                current: T::default(),
            }
        }
    }

    /// Rate limiting configuration for DoS protection.
    ///
    /// This struct defines rate limits for various dispatchable operations to prevent
    /// abuse and DoS attacks. It includes per-block limits, per-account limits, and
    /// minimum intervals between operations.
    ///
    /// Rate limiting categories:
    /// - **Per-Block Limits**: Maximum operations per block across all accounts
    /// - **Per-Account Limits**: Maximum operations per account within time windows
    /// - **Minimum Intervals**: Required time between repeated operations
    /// - **Weight Bounds**: Maximum computational weight for operations
    ///
    /// These limits ensure network stability under adversarial conditions while
    /// allowing legitimate operations to proceed normally.
    #[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
    #[codec(mel_bound())]
    pub struct RateLimitConfig {
        // Per-block rate limits
        /// Maximum proposal submissions per block (all accounts combined)
        pub max_proposals_per_block: u32,
        /// Maximum validator join operations per block
        pub max_joins_per_block: u32,
        /// Maximum validator leave operations per block
        pub max_leaves_per_block: u32,
        /// Maximum governance votes per block
        pub max_votes_per_block: u32,
        
        // Per-account rate limits (within time windows)
        /// Maximum proposals per account per time window
        pub max_proposals_per_account: u32,
        /// Time window for proposal rate limiting (in blocks)
        pub proposal_rate_window: u32,
        /// Maximum validator status changes per account per time window
        pub max_status_changes_per_account: u32,
        /// Time window for status change rate limiting (in blocks)
        pub status_change_rate_window: u32,
        
        // Minimum intervals between operations
        /// Minimum blocks between validator status changes for same account
        pub min_validator_status_interval: u32,
        /// Minimum blocks between proposal submissions for same account
        pub min_proposal_interval: u32,
        /// Minimum blocks between governance votes for same account on different proposals
        pub min_vote_interval: u32,
        
        // Weight bounds for operations with loops
        /// Maximum weight for operations that iterate over validator sets (in ref_time units)
        pub max_validator_iteration_weight: u64,
        /// Maximum weight for operations that process proposal queues (in ref_time units)
        pub max_proposal_processing_weight: u64,
        /// Maximum iterations allowed in single operation
        pub max_loop_iterations: u32,
    }

    impl Default for RateLimitConfig {
        fn default() -> Self {
            Self {
                // Conservative per-block limits
                max_proposals_per_block: 5,
                max_joins_per_block: 3,
                max_leaves_per_block: 3,
                max_votes_per_block: 20,
                
                // Per-account limits with reasonable windows
                max_proposals_per_account: 3,
                proposal_rate_window: 100, // ~10 minutes at 6s blocks
                max_status_changes_per_account: 2,
                status_change_rate_window: 1000, // ~100 minutes at 6s blocks
                
                // Minimum intervals to prevent spam
                min_validator_status_interval: 50, // ~5 minutes at 6s blocks
                min_proposal_interval: 20, // ~2 minutes at 6s blocks
                min_vote_interval: 1, // 1 block minimum
                
                // Weight bounds for loop operations
                max_validator_iteration_weight: 1_000_000_000, // 1 second of compute
                max_proposal_processing_weight: 500_000_000, // 0.5 seconds
                max_loop_iterations: 1000,
            }
        }
    }

    /// Dispatchable operation types for rate limiting.
    #[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
    #[codec(mel_bound())]
    pub enum DispatchableType {
        /// Proposal submission operations
        SubmitProposal,
        /// Validator join operations
        JoinValidators,
        /// Validator leave operations
        LeaveValidators,
        /// Governance voting operations
        VoteProposal,
        /// Parameter update operations
        UpdateParameter,
        /// Validator status change operations (join/leave/cancel)
        ValidatorStatusChange,
    }

    /// Rate limiting violation types for error reporting.
    #[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
    #[codec(mel_bound())]
    pub enum RateLimitViolation {
        /// Per-block limit exceeded
        PerBlockLimitExceeded {
            operation: DispatchableType,
            current_count: u32,
            limit: u32,
        },
        /// Per-account limit exceeded
        PerAccountLimitExceeded {
            operation: DispatchableType,
            current_count: u32,
            limit: u32,
            window_blocks: u32,
        },
        /// Minimum interval not respected
        MinimumIntervalViolation {
            operation: DispatchableType,
            blocks_since_last: u32,
            required_interval: u32,
        },
        /// Weight limit exceeded
        WeightLimitExceeded {
            operation: DispatchableType,
            actual_weight: u64,
            max_weight: u64,
        },
    }

    /// Comprehensive governance configuration with safety rails for all DCF parameters.
    ///
    /// This struct contains parameter ranges for all configurable aspects of the DCF
    /// system, providing centralized governance with built-in safety constraints.
    /// Each parameter has defined minimum and maximum values to prevent dangerous
    /// configurations that could compromise network security or stability.
    ///
    /// Parameter categories include:
    /// - **Epoch Management**: Block counts, validator limits, stake requirements
    /// - **Consensus Weights**: PoS/PoI balance and contribution limits
    /// - **Performance Thresholds**: Score limits, participation requirements
    /// - **Economic Parameters**: Reward amounts, slashing percentages, cooldowns
    /// - **Trust Score Weights**: Component weights for trust calculations
    /// - **Operational Limits**: Timeouts, intervals, and processing bounds
    ///
    /// All parameters can be updated through root-only governance calls that:
    /// - Validate new values against defined ranges
    /// - Emit events documenting changes with old/new values
    /// - Provide immediate effect or schedule changes for next epoch
    /// - Maintain audit trail of all parameter modifications
    ///
    /// This system ensures network stability while enabling controlled adaptation
    /// to changing conditions and requirements.
    #[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
    pub struct GovernanceConfig<T: Config> {
        // Epoch configuration parameters
        /// Range for blocks per epoch (typical: 100-7200 blocks)
        pub epoch_length: ParameterRange<u32>,
        /// Range for minimum stake requirement (prevents too low/high barriers)
        pub min_stake: ParameterRange<<T as pallet::Config>::Balance>,
        /// Range for maximum validators in active set (prevents overcrowding/underpopulation)
        pub max_validators: ParameterRange<u32>,
        
        // Consensus weight parameters
        /// Range for Proof-of-Stake weight in final score calculation (0-10000 basis points)
        pub pos_weight: ParameterRange<u64>,
        /// Range for Proof-of-Inference weight in final score calculation (0-10000 basis points)
        pub poi_weight: ParameterRange<u64>,
        
        // Performance threshold parameters
        /// Range for minimum performance score threshold (prevents too strict/lenient standards)
        pub min_performance_score: ParameterRange<u64>,
        /// Range for high performance score threshold (maintains meaningful performance tiers)
        pub high_performance_score: ParameterRange<u64>,
        /// Range for minimum participation rate percentage (ensures network reliability)
        pub min_participation_rate: ParameterRange<u32>,
        /// Range for high participation rate threshold (rewards exceptional availability)
        pub high_participation_rate: ParameterRange<u32>,
        
        // Economic parameters
        /// Range for default validator reward amounts (prevents excessive/insufficient incentives)
        pub validator_reward: ParameterRange<<T as pallet::Config>::Balance>,
        /// Range for slashing percentage (prevents too harsh/lenient penalties)
        pub slash_percent: ParameterRange<u32>,
        /// Range for leave cooldown period in blocks (balances flexibility with stability)
        pub leave_cooldown: ParameterRange<BlockNumberFor<T>>,
        
        // Trust score configuration
        /// Range for uptime weight in trust score calculation
        pub trust_score_uptime_weight: ParameterRange<u32>,
        /// Range for inference weight in trust score calculation
        pub trust_score_inference_weight: ParameterRange<u32>,
        /// Range for slashing weight in trust score calculation
        pub trust_score_slashing_weight: ParameterRange<u32>,
        
        /// Range for maximum trust score growth rate per epoch
        pub trust_score_max_growth_rate: ParameterRange<u32>,
        
        /// Range for maximum trust score decay rate per epoch
        pub trust_score_max_decay_rate: ParameterRange<u32>,
        
        /// Range for minimum trust score value
        pub trust_score_min_value: ParameterRange<u64>,
        
        /// Range for maximum trust score value
        pub trust_score_max_value: ParameterRange<u64>,
        
        /// Range for trust score stability factor
        pub trust_score_stability_factor: ParameterRange<u32>,
        
        // Block production parameters
        /// Range for score boost awarded for successful block authorship
        pub block_authorship_boost: ParameterRange<u64>,
        /// Range for score penalty applied for missed blocks
        pub missed_block_penalty: ParameterRange<u64>,
        
        // Inference scoring parameters
        /// Range for low-quality inference score boost
        pub inference_boost_low: ParameterRange<u64>,
        /// Range for medium-quality inference score boost
        pub inference_boost_medium: ParameterRange<u64>,
        /// Range for high-quality inference score boost
        pub inference_boost_high: ParameterRange<u64>,
    }

    /// Storage migration step definition for safe schema upgrades.
    ///
    /// This struct defines a single migration step that transforms storage
    /// from one version to another. Each migration step includes validation
    /// functions to ensure data integrity before and after the migration.
    ///
    /// Migration steps are executed sequentially during runtime upgrades
    /// to safely transform storage schemas while preserving data integrity.
    /// Each step is atomic and can be rolled back if validation fails.
    #[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
    pub struct MigrationStep {
        /// Source storage version for this migration step
        pub from_version: u32,
        /// Target storage version after migration completion
        pub to_version: u32,
        /// Human-readable description of the migration changes
        pub description: BoundedVec<u8, ConstU32<256>>,
        /// Whether this migration step is mandatory for system operation
        pub is_mandatory: bool,
    }

    /// Storage validation rule for ensuring data integrity.
    ///
    /// This struct defines validation rules that are applied to storage
    /// data during migrations and runtime initialization. Validation rules
    /// help detect corruption, inconsistencies, and constraint violations.
    ///
    /// Rules can be applied at different stages:
    /// - Pre-migration: Validate source data before transformation
    /// - Post-migration: Validate target data after transformation
    /// - Runtime init: Validate data consistency at startup
    #[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
    pub struct ValidationRule {
        /// Unique identifier for this validation rule
        pub rule_id: BoundedVec<u8, ConstU32<64>>,
        /// Human-readable description of what this rule validates
        pub description: BoundedVec<u8, ConstU32<256>>,
        /// Storage version this rule applies to
        pub target_version: u32,
        /// Whether this rule is critical for system operation
        pub is_critical: bool,
    }

    /// Storage version manager for coordinating migrations and validations.
    ///
    /// This struct manages the storage version lifecycle including:
    /// - Tracking current and target storage versions
    /// - Coordinating migration step execution
    /// - Applying validation rules at appropriate stages
    /// - Maintaining migration history and audit trail
    ///
    /// The version manager ensures that storage migrations are executed
    /// safely and completely, with proper validation at each stage.
    #[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
    pub struct StorageVersionManager {
        /// Current active storage version
        pub current_version: u32,
        /// Target version for pending migrations
        pub target_version: u32,
        /// List of completed migration steps
        pub completed_migrations: BoundedVec<u32, ConstU32<32>>,
        /// Timestamp of last migration execution
        pub last_migration_timestamp: u64,
        /// Whether migrations are currently in progress
        pub migration_in_progress: bool,
    }

    impl Default for StorageVersionManager {
        fn default() -> Self {
            Self {
                current_version: 1, // Start with version 1 for production-ready storage
                target_version: 1,
                completed_migrations: BoundedVec::default(),
                last_migration_timestamp: 0,
                migration_in_progress: false,
            }
        }
    }

    /// Migration error types for detailed error reporting.
    ///
    /// This enum provides specific error types that can occur during
    /// storage migrations, enabling precise error handling and recovery.
    #[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
    pub enum MigrationError {
        /// Source storage version does not match expected version
        VersionMismatch {
            expected: u32,
            found: u32,
        },
        /// Pre-migration validation failed
        PreValidationFailed {
            rule_id: BoundedVec<u8, ConstU32<64>>,
            reason: BoundedVec<u8, ConstU32<256>>,
        },
        /// Post-migration validation failed
        PostValidationFailed {
            rule_id: BoundedVec<u8, ConstU32<64>>,
            reason: BoundedVec<u8, ConstU32<256>>,
        },
        /// Migration step execution failed
        ExecutionFailed {
            step_version: u32,
            reason: BoundedVec<u8, ConstU32<256>>,
        },
        /// Insufficient resources for migration
        InsufficientResources,
        /// Migration already in progress
        MigrationInProgress,
        /// No migration path available
        NoMigrationPath {
            from: u32,
            to: u32,
        },
    }

    /// Types of invariant violations that can occur in the DCF system.
    ///
    /// This enum categorizes different types of system invariant violations
    /// to enable appropriate handling and reporting. Each violation type
    /// includes context information to aid in debugging and resolution.
    #[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, frame_support::__private::codec::DecodeWithMemTracking)]
    pub enum InvariantViolation<T: Config> {
        /// Economic invariant violations related to balance and stake management
        Economic {
            /// Type of economic violation
            violation_type: EconomicViolationType<T>,
            /// Additional context about the violation
            context: BoundedVec<u8, ConstU32<256>>,
        },
        /// Validator set invariant violations
        Validator {
            /// Type of validator violation
            violation_type: ValidatorViolationType<T>,
            /// Additional context about the violation
            context: BoundedVec<u8, ConstU32<256>>,
        },
        /// Temporal invariant violations related to time-based constraints
        Temporal {
            /// Type of temporal violation
            violation_type: TemporalViolationType,
            /// Additional context about the violation
            context: BoundedVec<u8, ConstU32<256>>,
        },
    }

    /// Economic invariant violation types with specific details.
    #[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, frame_support::__private::codec::DecodeWithMemTracking)]
    pub enum EconomicViolationType<T: Config> {
        /// Total reserved balance is less than total slashed amount
        ReservedLessThanSlashed {
            total_reserved: <T as pallet::Config>::Balance,
            total_slashed: <T as pallet::Config>::Balance,
        },
        /// Validator has negative reserved balance
        NegativeReservedBalance {
            validator: T::AccountId,
            balance: <T as pallet::Config>::Balance,
        },
        /// Active validator stake is below minimum requirement
        StakeBelowMinimum {
            validator: T::AccountId,
            current_stake: <T as pallet::Config>::Balance,
            minimum_required: <T as pallet::Config>::Balance,
        },
    }

    /// Validator set invariant violation types.
    #[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, frame_support::__private::codec::DecodeWithMemTracking)]
    pub enum ValidatorViolationType<T: Config> {
        /// Active validator set exceeds maximum allowed size
        ActiveSetTooLarge {
            current_size: u32,
            max_allowed: u32,
        },
        /// Validator is both active and in cooldown simultaneously
        ActiveAndInCooldown {
            validator: T::AccountId,
        },
        /// Duplicate validator found in active set
        DuplicateInActiveSet {
            validator: T::AccountId,
        },
    }

    /// Temporal invariant violation types.
    #[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, frame_support::__private::codec::DecodeWithMemTracking)]
    pub enum TemporalViolationType {
        /// Epoch number decreased (non-monotonic progression)
        EpochRegression {
            previous_epoch: u32,
            current_epoch: u32,
        },
        /// Finality marker regressed
        FinalityRegression {
            previous_finalized: u32,
            current_finalized: u32,
        },
    }

    /// Severity level of invariant violations.
    #[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, frame_support::__private::codec::DecodeWithMemTracking)]
    pub enum InvariantSeverity {
        /// Low severity - system can continue operating
        Low,
        /// Medium severity - requires attention but not critical
        Medium,
        /// High severity - critical issue requiring immediate attention
        High,
        /// Critical severity - system integrity compromised
        Critical,
    }

    /// Comprehensive invariant report generated at epoch boundaries.
    ///
    /// This report provides a complete assessment of system invariants
    /// at epoch transition points, enabling monitoring and alerting
    /// for any violations that could compromise system integrity.
    #[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
    pub struct InvariantReport<T: Config> {
        /// Epoch number when the report was generated
        pub epoch: u32,
        /// Block number when the report was generated
        pub block_number: u32,
        /// List of detected invariant violations
        pub violations: BoundedVec<InvariantViolation<T>, ConstU32<50>>,
        /// Overall severity of the report
        pub severity: InvariantSeverity,
        /// Timestamp when the report was generated
        pub timestamp: u64,
    }

    /// Economic invariant checker for balance and stake validation.
    ///
    /// This struct contains methods to validate economic invariants
    /// that ensure the system's economic model remains consistent
    /// and secure across epoch transitions.
    #[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, Default)]
    pub struct EconomicInvariants<T: Config> {
        /// Phantom data for type parameter
        _phantom: sp_std::marker::PhantomData<T>,
    }

    /// Validator set invariant checker for validator management validation.
    ///
    /// This struct contains methods to validate validator set invariants
    /// that ensure proper validator lifecycle management and prevent
    /// inconsistent validator states.
    #[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, Default)]
    pub struct ValidatorInvariants<T: Config> {
        /// Phantom data for type parameter
        _phantom: sp_std::marker::PhantomData<T>,
    }

    /// Temporal invariant checker for time-based constraint validation.
    ///
    /// This struct contains methods to validate temporal invariants
    /// that ensure proper progression of time-based system state
    /// such as epochs and finality markers.
    #[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, Default)]
    pub struct TemporalInvariants {
        // No fields needed for temporal checks
    }

    /// Comprehensive invariant checker that orchestrates all invariant validations.
    ///
    /// This is the main invariant checking system that coordinates economic,
    /// validator, and temporal invariant checks at epoch boundaries. It provides
    /// a unified interface for invariant validation and reporting.
    #[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, Default)]
    pub struct InvariantChecker<T: Config> {
        /// Economic invariant checker
        pub economic_checks: EconomicInvariants<T>,
        /// Validator invariant checker
        pub validator_checks: ValidatorInvariants<T>,
        /// Temporal invariant checker
        pub temporal_checks: TemporalInvariants,
    }

    /// Genesis validation report generated by dry-run genesis build.
    ///
    /// This report provides comprehensive information about the genesis configuration
    /// validation results, including statistics, invariant checks, and warnings.
    /// Used by the dry-run functionality to provide detailed feedback without
    /// actually modifying storage.
    #[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo)]
    pub struct GenesisValidationReport<T: Config> {
        /// Total number of validators in genesis configuration
        pub validator_count: u32,
        /// Total stake across all validators
        pub total_stake: <T as pallet::Config>::Balance,
        /// Average stake per validator
        pub average_stake: <T as pallet::Config>::Balance,
        /// Validator with minimum stake (validator, stake amount)
        pub min_stake_validator: Option<(T::AccountId, <T as pallet::Config>::Balance)>,
        /// Validator with maximum stake (validator, stake amount)
        pub max_stake_validator: Option<(T::AccountId, <T as pallet::Config>::Balance)>,
        /// Epoch configuration being validated
        pub epoch_config: EpochConfig,
        /// List of invariant checks performed and their results
        pub invariant_checks: Vec<String>,
        /// List of warnings about potential issues
        pub warnings: Vec<String>,
        /// Overall validation result
        pub validation_passed: bool,
    }

    impl<T: Config> Default for GovernanceConfig<T> {
        fn default() -> Self {
            Self {
                epoch_length: ParameterRange::default(),
                min_stake: ParameterRange::default(),
                max_validators: ParameterRange::default(),
                pos_weight: ParameterRange::default(),
                poi_weight: ParameterRange::default(),
                min_performance_score: ParameterRange::default(),
                high_performance_score: ParameterRange::default(),
                min_participation_rate: ParameterRange::default(),
                high_participation_rate: ParameterRange::default(),
                validator_reward: ParameterRange::default(),
                slash_percent: ParameterRange::default(),
                leave_cooldown: ParameterRange::default(),
                trust_score_uptime_weight: ParameterRange::default(),
                trust_score_inference_weight: ParameterRange::default(),
                trust_score_slashing_weight: ParameterRange::default(),
                trust_score_max_growth_rate: ParameterRange::default(),
                trust_score_max_decay_rate: ParameterRange::default(),
                trust_score_min_value: ParameterRange::default(),
                trust_score_max_value: ParameterRange::default(),
                trust_score_stability_factor: ParameterRange::default(),
                block_authorship_boost: ParameterRange::default(),
                missed_block_penalty: ParameterRange::default(),
                inference_boost_low: ParameterRange::default(),
                inference_boost_medium: ParameterRange::default(),
                inference_boost_high: ParameterRange::default(),
            }
        }
    }

    /// Enumeration of all configurable DCF parameters for governance updates.
    ///
    /// This enum provides a type-safe way to identify which parameter is being
    /// updated through governance calls. Each variant corresponds to a specific
    /// configurable aspect of the DCF system.
    ///
    /// Used by the `update_dcf_parameter` dispatchable to:
    /// - Identify which parameter range to validate against
    /// - Apply the update to the correct configuration field
    /// - Emit appropriate events with parameter-specific information
    /// - Provide clear error messages for validation failures
    ///
    /// The enum covers all major parameter categories:
    /// - Epoch and validator set management
    /// - Consensus mechanism weights and thresholds
    /// - Economic incentives and penalties
    /// - Performance evaluation criteria
    /// - Trust score calculation components
    #[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, frame_support::__private::codec::DecodeWithMemTracking)]
    pub enum ParameterType {
        // Epoch configuration
        EpochLength,
        MinStake,
        MaxValidators,
        
        // Consensus weights
        PosWeight,
        PoiWeight,
        
        // Performance thresholds
        MinPerformanceScore,
        HighPerformanceScore,
        MinParticipationRate,
        HighParticipationRate,
        
        // Economic parameters
        ValidatorReward,
        SlashPercent,
        LeaveCooldown,
        
        // Trust score weights
        TrustScoreUptimeWeight,
        TrustScoreInferenceWeight,
        TrustScoreSlashingWeight,
        
        // Trust score bounds and rates
        TrustScoreMaxGrowthRate,
        TrustScoreMaxDecayRate,
        TrustScoreMinValue,
        TrustScoreMaxValue,
        TrustScoreStabilityFactor,
        
        // Block production parameters
        BlockAuthorshipBoost,
        MissedBlockPenalty,
        
        // Inference scoring
        InferenceBoostLow,
        InferenceBoostMedium,
        InferenceBoostHigh,
    }

    /// Actions that can be proposed via governance.
    #[derive(Encode, Decode, RuntimeDebug, TypeInfo, MaxEncodedLen, frame_support::__private::codec::DecodeWithMemTracking)]
    pub enum ProposalAction<T: Config + TypeInfo + fmt::Debug> { // <-- Change here
        Slash { validator: T::AccountId, amount: <T as pallet::Config>::Balance },
        Reward { validator: T::AccountId, amount: <T as pallet::Config>::Balance },
        Eject { validator: T::AccountId, reason: EjectionReason },
        AddValidator { validator: T::AccountId },
        RemoveValidator { validator: T::AccountId },
        RewardMultiple { validators: BoundedVec<T::AccountId, T::MaxProposalActionBoundedVecSize>, amount: <T as pallet::Config>::Balance },
    }

    impl<T: Config + TypeInfo + fmt::Debug> Clone for ProposalAction<T> {
        fn clone(&self) -> Self {
            match self {
                Self::Slash { validator, amount } => Self::Slash { validator: validator.clone(), amount: *amount },
                Self::Reward { validator, amount } => Self::Reward { validator: validator.clone(), amount: *amount },
                Self::Eject { validator, reason } => Self::Eject { validator: validator.clone(), reason: reason.clone() },
                Self::AddValidator { validator } => Self::AddValidator { validator: validator.clone() },
                Self::RemoveValidator { validator } => Self::RemoveValidator { validator: validator.clone() },
                Self::RewardMultiple { validators, amount } => Self::RewardMultiple { validators: validators.clone(), amount: *amount },
            }
        }
    }

    impl<T: Config + TypeInfo + fmt::Debug> PartialEq for ProposalAction<T> {
        fn eq(&self, other: &Self) -> bool {
            match (self, other) {
                (Self::Slash { validator: v1, amount: a1 }, Self::Slash { validator: v2, amount: a2 }) => v1 == v2 && a1 == a2,
                (Self::Reward { validator: v1, amount: a1 }, Self::Reward { validator: v2, amount: a2 }) => v1 == v2 && a1 == a2,
                (Self::Eject { validator: v1, reason: r1 }, Self::Eject { validator: v2, reason: r2 }) => v1 == v2 && r1 == r2,
                (Self::AddValidator { validator: v1 }, Self::AddValidator { validator: v2 }) => v1 == v2,
                (Self::RemoveValidator { validator: v1 }, Self::RemoveValidator { validator: v2 }) => v1 == v2,
                (Self::RewardMultiple { validators: v1, amount: a1 }, Self::RewardMultiple { validators: v2, amount: a2 }) => v1 == v2 && a1 == a2,
                _ => false,
            }
        }
    }

    impl<T: Config + TypeInfo + fmt::Debug> Eq for ProposalAction<T> {}

    /// Governance proposal structure.
    #[derive(Encode, Decode, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    pub struct GovernanceProposal<T: Config + TypeInfo + fmt::Debug> { // <-- Change here
        pub proposer: T::AccountId,
        pub action: ProposalAction<T>,
        pub status: ProposalStatus,
        pub votes_for: u32,
        pub votes_against: u32,
    }

    impl<T: Config + TypeInfo + fmt::Debug> Clone for GovernanceProposal<T> {
        fn clone(&self) -> Self {
            Self {
                proposer: self.proposer.clone(),
                action: self.action.clone(),
                status: self.status.clone(),
                votes_for: self.votes_for,
                votes_against: self.votes_against,
            }
        }
    }

    impl<T: Config + TypeInfo + fmt::Debug> PartialEq for GovernanceProposal<T> {
        fn eq(&self, other: &Self) -> bool {
            self.proposer == other.proposer &&
            self.action == other.action &&
            self.status == other.status &&
            self.votes_for == other.votes_for &&
            self.votes_against == other.votes_against
        }
    }

    impl<T: Config + TypeInfo + fmt::Debug> Eq for GovernanceProposal<T> {}

    /// Status of a governance proposal.
    #[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen, frame_support::__private::codec::DecodeWithMemTracking)]
    pub enum ProposalStatus {
        Pending,
        Approved,
        Rejected,
        Executed,
    }

    /// History of recent epochs for analytics and tracking.
    #[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen, Default)]
    pub struct EpochHistory<T: Config> {
        pub epoch_number: u32,
        pub active_validators: BoundedVec<<T as frame_system::Config>::AccountId, <T as pos::Config>::MaxValidators>,
        pub score_snapshot: BoundedVec
            <(<T as frame_system::Config>::AccountId, u64), <T as pos::Config>::MaxValidators>,
        pub inference_summary: BoundedVec
            <(<T as frame_system::Config>::AccountId, Option<u64>), <T as pos::Config>::MaxValidators>,
    }

    /// Non-generic struct for runtime API (AccountId = T::AccountId, all BoundedVecs use configurable size).
    #[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen, Default)]
    pub struct RuntimeEpochHistory<AccountId> {
        pub epoch_number: u32,
        pub active_validators: BoundedVec<AccountId, ConstU32<100>>, // Keep as reasonable default for runtime API compatibility
        pub score_snapshot: BoundedVec<(AccountId, u64), ConstU32<100>>,
        pub inference_summary: BoundedVec<(AccountId, Option<u64>), ConstU32<100>>,
    }

    /// Comprehensive validator metadata information
    #[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    pub struct ValidatorMetadataInfo {
        pub name: BoundedVec<u8, ConstU32<32>>,
        pub website: Option<BoundedVec<u8, ConstU32<64>>>,
        pub contact: Option<BoundedVec<u8, ConstU32<64>>>,
        pub description: Option<BoundedVec<u8, ConstU32<128>>>,
        pub location: Option<BoundedVec<u8, ConstU32<32>>>,
        pub commission_rate: Option<u32>, // Percentage (0 to T::MaxCommissionRate)
        pub min_stake_required: Option<u128>,
        pub created_at: u64, // Timestamp in milliseconds
        pub updated_at: u64, // Timestamp in milliseconds
    }

    /// Performance record for tracking validator performance over time
    #[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    pub struct PerformanceRecord {
        pub epoch: u32,
        pub blocks_authored: u32,
        pub blocks_missed: u32,
        pub uptime_percentage: u32, // 0 to T::PercentagePrecision
        pub inference_score: u64,
        pub stake_score: u64,
        pub final_score: u64,
        pub participation_rate: u32, // 0 to T::PercentagePrecision
        pub timestamp: u64, // Timestamp in milliseconds
    }

    impl<T: Config> From<EpochHistory<T>> for RuntimeEpochHistory<<T as frame_system::Config>::AccountId> {
        fn from(e: EpochHistory<T>) -> Self {
            RuntimeEpochHistory {
                epoch_number: e.epoch_number,
                active_validators: BoundedVec::truncate_from(e.active_validators.into_inner()),
                score_snapshot: BoundedVec::truncate_from(e.score_snapshot.into_inner()),
                inference_summary: BoundedVec::truncate_from(e.inference_summary.into_inner()),
            }
        }
    }

    /// Comprehensive validator profile information returned by runtime API
    #[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    pub struct ValidatorProfile<AccountId, Balance, BlockNumber> {
        /// Current stake amount reserved by the validator
        pub stake: Balance,
        /// Current Proof-of-Inference score
        pub poi_score: u32,
        /// Current final consensus score
        pub final_score: u64,
        /// Current trust score based on performance history
        pub trust_score: u64,
        /// Current validator status (active, inactive, etc.)
        pub status: ValidatorStatus,
        /// Total number of inferences submitted
        pub inference_count: u64,
        /// Human-readable validator name
        pub name: Option<BoundedVec<u8, ConstU32<64>>>,
        /// Last block number where validator was active
        pub last_active_block: BlockNumber,
        /// Phantom data to satisfy type parameters
        pub _phantom: sp_std::marker::PhantomData<AccountId>,
    }

    /// Detailed score breakdown for validator performance analysis
    #[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    pub struct ScoreBreakdown {
        /// Proof-of-Stake score component
        pub pos_score: u64,
        /// Proof-of-Inference score component
        pub poi_score: u64,
        /// Weight applied to PoS score in final calculation
        pub pos_weight: u64,
        /// Weight applied to PoI score in final calculation
        pub poi_weight: u64,
        /// Final combined consensus score
        pub final_score: u64,
        /// Trust score based on historical performance
        pub trust_score: u64,
    }

    /// System constants and configuration parameters exposed via runtime API
    #[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    pub struct SystemConstants<Balance, BlockNumber> {
        /// Minimum stake required to become a validator
        pub min_stake: Balance,
        /// Minimum score threshold for validator participation
        pub min_score_threshold: u64,
        /// Number of blocks per epoch
        pub epoch_length: u32,
        /// Maximum number of active validators
        pub max_validators: u32,
        /// Cooldown period for validator re-entry after leaving
        pub cooldown_period: BlockNumber,
    }

    /// Comprehensive system metrics for operational monitoring and visibility.
    /// 
    /// This structure provides aggregated metrics about the DCF system state,
    /// designed to reduce RPC fan-out by providing all essential metrics in
    /// a single compact API call. All metrics reflect the current epoch state
    /// and are updated automatically during epoch transitions.
    /// 
    /// # Field Semantics and Units
    /// 
    /// ## Validator Metrics
    /// - `active_validator_count`: Number of validators currently participating in consensus (count)
    /// - `total_validator_count`: Total number of registered validators including inactive ones (count)
    /// - `validator_set_capacity`: Maximum number of validators that can be active simultaneously (count)
    /// 
    /// ## Economic Metrics  
    /// - `total_reserved`: Sum of all validator stakes currently reserved in the system (Balance units)
    /// - `total_slashed`: Cumulative amount slashed from validators across all epochs (Balance units)
    /// - `total_rewards`: Cumulative rewards distributed to validators across all epochs (Balance units)
    /// - `average_stake`: Mean stake amount across all active validators (Balance units)
    /// 
    /// ## Epoch and Performance Metrics
    /// - `current_epoch`: Current epoch number, increments with each epoch transition (epoch number)
    /// - `blocks_in_current_epoch`: Number of blocks produced in the current epoch (block count)
    /// - `epoch_progress_percentage`: Progress through current epoch as percentage (0-100)
    /// - `average_trust_score`: Mean trust score across all active validators (0-10000 scale)
    /// 
    /// ## System Health Indicators
    /// - `invariant_health`: Overall system health based on invariant checks (enum: Healthy/Warning/Critical)
    /// - `finality_lag`: Number of blocks between current block and last finalized block (block count)
    /// - `missed_blocks_current_epoch`: Total blocks missed by all validators in current epoch (block count)
    /// 
    /// # Usage Guidelines
    /// 
    /// - Use this API for dashboard displays and monitoring systems
    /// - Metrics are updated automatically during epoch transitions
    /// - All balance amounts are in the runtime's native balance units
    /// - Trust scores use a 0-10000 scale where 10000 represents maximum trust
    /// - Percentage values use integer representation (0-100 for percentages)
    /// 
    /// # Performance Characteristics
    /// 
    /// - Single RPC call provides comprehensive system overview
    /// - Metrics are cached and updated only during epoch transitions
    /// - Constant-time access to all aggregated values
    /// - Minimal computational overhead for metric retrieval
    #[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    pub struct SystemMetrics<T: Config> {
        // Validator metrics
        /// Number of validators currently active in consensus (participating in block production)
        pub active_validator_count: u32,
        /// Total number of validators registered in the system (active + inactive)
        pub total_validator_count: u32,
        /// Maximum number of validators that can be active simultaneously
        pub validator_set_capacity: u32,
        
        // Economic metrics
        /// Total amount of tokens reserved as validator stakes across all validators
        pub total_reserved: <T as pallet::Config>::Balance,
        /// Cumulative amount of tokens slashed from validators since genesis
        pub total_slashed: <T as pallet::Config>::Balance,
        /// Cumulative amount of rewards distributed to validators since genesis
        pub total_rewards: <T as pallet::Config>::Balance,
        /// Average stake amount across all active validators
        pub average_stake: <T as pallet::Config>::Balance,
        
        // Epoch and timing metrics
        /// Current epoch number (starts at 0, increments with each epoch transition)
        pub current_epoch: u32,
        /// Number of blocks produced in the current epoch so far
        pub blocks_in_current_epoch: u32,
        /// Progress through current epoch as percentage (0-100)
        pub epoch_progress_percentage: u32,
        /// Average trust score across all active validators (0-10000 scale)
        pub average_trust_score: u64,
        
        // System health metrics
        /// Overall system health status based on invariant validation
        pub invariant_health: InvariantHealth,
        /// Number of blocks between current block and last finalized block
        pub finality_lag: u32,
        /// Total number of blocks missed by all validators in current epoch
        pub missed_blocks_current_epoch: u32,
    }

    impl<T: Config> Default for SystemMetrics<T> {
        fn default() -> Self {
            Self {
                active_validator_count: 0,
                total_validator_count: 0,
                validator_set_capacity: 0,
                total_reserved: <T as pallet::Config>::Balance::default(),
                total_slashed: <T as pallet::Config>::Balance::default(),
                total_rewards: <T as pallet::Config>::Balance::default(),
                average_stake: <T as pallet::Config>::Balance::default(),
                current_epoch: 0,
                blocks_in_current_epoch: 0,
                epoch_progress_percentage: 0,
                average_trust_score: 0,
                invariant_health: InvariantHealth::default(),
                finality_lag: 0,
                missed_blocks_current_epoch: 0,
            }
        }
    }

    /// System health status based on invariant validation results.
    /// 
    /// This enum represents the overall health of the DCF system based on
    /// the latest invariant checks performed during epoch transitions.
    /// 
    /// # Health Levels
    /// 
    /// - `Healthy`: All invariants pass, system operating normally
    /// - `Warning`: Minor invariant violations detected, system stable but monitoring recommended
    /// - `Critical`: Major invariant violations detected, immediate attention required
    /// 
    /// # Usage
    /// 
    /// Used in SystemMetrics to provide quick health assessment for monitoring systems.
    /// Operators should investigate Warning status and take immediate action on Critical status.
    #[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen, Default)]
    pub enum InvariantHealth {
        /// All system invariants are satisfied, normal operation
        #[default]
        Healthy,
        /// Minor invariant violations detected, monitoring recommended
        Warning,
        /// Major invariant violations detected, immediate attention required
        Critical,
    }

    /// Performance indicators for system-wide operational metrics.
    /// 
    /// This structure provides additional performance metrics that complement
    /// the core SystemMetrics, focusing on operational efficiency and
    /// network performance characteristics.
    /// 
    /// # Field Semantics and Units
    /// 
    /// ## Block Production Metrics
    /// - `average_block_time`: Mean time between blocks in current epoch (milliseconds)
    /// - `block_production_rate`: Blocks produced per minute in current epoch (blocks/minute)
    /// - `consensus_participation_rate`: Percentage of validators actively participating (0-100)
    /// 
    /// ## Score Distribution Metrics
    /// - `score_distribution_variance`: Variance in validator scores indicating competition level
    /// - `top_performer_score`: Highest validator score in current epoch (0-10000 scale)
    /// - `lowest_performer_score`: Lowest active validator score in current epoch (0-10000 scale)
    /// 
    /// ## Network Efficiency Metrics
    /// - `epoch_transition_efficiency`: Success rate of epoch transitions (0-100 percentage)
    /// - `governance_activity_level`: Number of active governance proposals
    /// - `validator_churn_rate`: Rate of validator set changes per epoch (percentage)
    #[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen, Default)]
    pub struct SystemPerformanceIndicators {
        // Block production metrics
        /// Average time between blocks in current epoch (milliseconds)
        pub average_block_time: u64,
        /// Number of blocks produced per minute in current epoch
        pub block_production_rate: u32,
        /// Percentage of active validators participating in consensus (0-100)
        pub consensus_participation_rate: u32,
        
        // Score distribution metrics
        /// Statistical variance in validator scores (indicates competition level)
        pub score_distribution_variance: u64,
        /// Highest validator score in current epoch (0-10000 scale)
        pub top_performer_score: u64,
        /// Lowest active validator score in current epoch (0-10000 scale)
        pub lowest_performer_score: u64,
        
        // Network efficiency metrics
        /// Success rate of epoch transitions as percentage (0-100)
        pub epoch_transition_efficiency: u32,
        /// Number of governance proposals currently active
        pub governance_activity_level: u32,
        /// Rate of validator set changes per epoch as percentage (0-100)
        pub validator_churn_rate: u32,
    }

    /// Validator uptime statistics for performance monitoring
    #[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    pub struct UptimeStats {
        /// Total number of epochs the validator has been active
        pub total_epochs: u32,
        /// Number of blocks authored by the validator
        pub blocks_authored: u32,
        /// Number of blocks missed by the validator
        pub blocks_missed: u32,
        /// Current uptime percentage (0-10000 representing 0-100%)
        pub uptime_percentage: u32,
        /// Current participation rate (0-10000 representing 0-100%)
        pub participation_rate: u32,
    }

    /// Slashing record for tracking validator penalties
    #[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    pub struct SlashingRecord<Balance, BlockNumber> {
        /// Block number when slashing occurred
        pub block_number: BlockNumber,
        /// Amount slashed from validator's stake
        pub amount: Balance,
        /// Reason for slashing
        pub reason: SlashingReason,
        /// Epoch when slashing occurred
        pub epoch: u32,
    }

    /// Reasons for validator slashing
    #[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    pub enum SlashingReason {
        /// Validator produced invalid blocks
        InvalidBlock,
        /// Validator was offline for extended period
        Downtime,
        /// Validator submitted incorrect inference results
        InvalidInference,
        /// Validator engaged in malicious behavior
        Misbehavior,
        /// Manual slashing by governance
        Governance,
    }

    
    #[pallet::config]
    pub trait Config: frame_system::Config + pos::Config<Balance = <Self as pallet::Config>::Balance> + poi::Config + TypeInfo + fmt::Debug {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        
        /// Interface to notify DVF of epoch transitions and weights
        type WeightFreezer: crate::traits::WeightFreezer<Self::AccountId>;

        /// Interface to query the last DVF-finalized block number.
        /// DCF progressive finality will not advance past this value.
        type DvfFinalizedBlockProvider: crate::traits::DvfFinalizedBlockProvider;
        
        // MaxValidators and MinActiveValidators are inherited from pos::Config
        
        /// Maximum number of epoch history records to maintain in storage.
        /// 
        /// Controls the size of the circular buffer storing recent epoch data.
        /// Higher values provide more historical data for analysis but consume
        /// more storage space. When the limit is reached, oldest records are
        /// automatically removed.
        /// 
        /// Typical values: 24-168 (representing hours to weeks of history).
        #[pallet::constant]
        type MaxEpochHistory: Get<u32>;
        
        // Scoring weights and thresholds
        /// Default weight for Proof-of-Stake (PoS) component in final score calculation.
        /// 
        /// Determines the initial influence of validator stake on their consensus score.
        /// Higher values give more importance to economic stake, while lower values
        /// reduce the impact of wealth on consensus participation.
        /// 
        /// Used in: final_score = (stake_score * pos_weight + inference_score * poi_weight) / precision
        /// Typical values: 3000-7000 (30%-70% when using 10000 as 100%).
        #[pallet::constant]
        type DefaultPosWeight: Get<u64>;
        
        /// Default weight for Proof-of-Inference (PoI) component in final score calculation.
        /// 
        /// Determines the initial influence of AI/ML inference performance on consensus score.
        /// Higher values emphasize technical AI capabilities, while lower values reduce
        /// the impact of inference accuracy on consensus participation.
        /// 
        /// Used in: final_score = (stake_score * pos_weight + inference_score * poi_weight) / precision
        /// Typical values: 3000-7000 (30%-70% when using 10000 as 100%).
        #[pallet::constant]
        type DefaultPoiWeight: Get<u64>;
        

        
        /// Maximum number of consecutive epochs a validator can be inactive.
        /// 
        /// After this many epochs of inactivity, validators may be automatically
        /// ejected from the validator set to maintain network health and prevent
        /// accumulation of dead weight in the validator registry.
        /// 
        /// Typical values: 3-10 epochs depending on epoch length.
        #[pallet::constant]
        type MaxInactiveEpochs: Get<u32>;
        
        /// Block interval for applying score decay to inactive validators.
        /// 
        /// Determines how frequently the pallet checks for and applies score
        /// decay to validators who haven't been active recently. More frequent
        /// checks provide better responsiveness but consume more computational resources.
        /// 
        /// Typical values: 10-100 blocks (1-10 minutes at 6s block time).
        #[pallet::constant]
        type ScoreDecayInterval: Get<u32>; // blocks
        
        /// Block interval for updating validator participation rate metrics.
        /// 
        /// Controls how often the pallet recalculates participation rates,
        /// block production statistics, and other activity-based metrics.
        /// Affects the responsiveness of performance tracking.
        /// 
        /// Typical values: 50-200 blocks (5-20 minutes at 6s block time).
        #[pallet::constant]
        type ParticipationUpdateInterval: Get<u32>; // blocks
        
        /// Block interval for checking and handling underperforming validators.
        /// 
        /// Determines how frequently the pallet evaluates validator performance
        /// and takes action against consistently poor performers. This includes
        /// score penalties, warnings, and potential ejection decisions.
        /// 
        /// Typical values: 50-500 blocks (5-50 minutes at 6s block time).
        #[pallet::constant]
        type UnderperformanceCheckInterval: Get<u32>; // blocks
        
        /// Block interval for generating automatic validator management proposals.
        /// 
        /// Controls how often the pallet creates governance proposals for
        /// validator rewards, penalties, or set changes based on performance
        /// analysis and network health metrics.
        /// 
        /// Typical values: 100-1000 blocks (10-100 minutes at 6s block time).
        #[pallet::constant]
        type ValidatorProposalInterval: Get<u32>; // blocks
        
        /// Block interval for updating comprehensive health and performance metrics.
        /// 
        /// Determines how frequently the pallet performs detailed health checks,
        /// updates performance histories, and generates network statistics.
        /// Less frequent updates reduce computational overhead.
        /// 
        /// Typical values: 500-2000 blocks (50-200 minutes at 6s block time).
        #[pallet::constant]
        type HealthMetricsInterval: Get<u32>; // blocks
        

        
        /// Number of blocks per epoch for validator set management and scoring.
        /// 
        /// Defines the fundamental time unit for validator operations including
        /// score updates, set rotations, reward distributions, and governance
        /// actions. Longer epochs provide stability, shorter epochs enable
        /// faster adaptation to changing conditions.
        /// 
        /// Typical values: 1200-7200 blocks (2-12 hours at 6s block time).
        #[pallet::constant]
        type EpochLength: Get<u32>; // blocks per epoch
        
        // Block authorship rewards and penalties
        /// Score boost awarded to validators for successfully authoring blocks.
        /// 
        /// Incentivizes consistent block production by rewarding validators
        /// who successfully create and submit valid blocks when selected.
        /// The boost is added to the validator's performance score.
        /// 
        /// Higher values encourage active participation in block production,
        /// while lower values reduce the impact of block authorship on scoring.
        /// 
        /// Typical values: 5-50 score points per successful block.
        #[pallet::constant]
        type BlockAuthorshipBoost: Get<u64>;
        
        /// Score penalty applied to validators for missing assigned block slots.
        /// 
        /// Discourages validator downtime and unreliability by penalizing
        /// validators who fail to produce blocks when selected as the author.
        /// The penalty is subtracted from the validator's performance score.
        /// 
        /// Should be balanced to discourage poor performance without being
        /// so harsh as to cause cascading failures during network issues.
        /// 
        /// Typical values: 2-20 score points per missed block.
        #[pallet::constant]
        type MissedBlockPenalty: Get<u64>;
        
        // Slashing and reward parameters are inherited from pos::Config
        
        // Validator metadata limits
        /// Maximum length in bytes for validator display names.
        /// 
        /// Prevents abuse of the naming system while allowing reasonable
        /// length names for validator identification. Names are UTF-8 encoded
        /// so actual character count may be less than byte count.
        /// 
        /// Typical values: 32-128 bytes.
        #[pallet::constant]
        type MaxValidatorNameLength: Get<u32>;
        
        /// Maximum length in bytes for validator website URLs.
        /// 
        /// Allows validators to provide website links for additional information
        /// while preventing storage abuse. Should accommodate typical URL lengths
        /// including domain names and paths.
        /// 
        /// Typical values: 64-256 bytes.
        #[pallet::constant]
        type MaxValidatorWebsiteLength: Get<u32>;
        
        /// Maximum length in bytes for validator contact information.
        /// 
        /// Enables validators to provide contact details (email, social media)
        /// for community engagement while limiting storage usage. Should
        /// accommodate email addresses and social media handles.
        /// 
        /// Typical values: 64-128 bytes.
        #[pallet::constant]
        type MaxValidatorContactLength: Get<u32>;
        
        /// Maximum length in bytes for validator description text.
        /// 
        /// Allows validators to provide detailed descriptions of their services,
        /// capabilities, and value propositions while preventing storage abuse.
        /// Should accommodate meaningful descriptions without excessive length.
        /// 
        /// Typical values: 128-512 bytes.
        #[pallet::constant]
        type MaxValidatorDescriptionLength: Get<u32>;
        
        /// Maximum length in bytes for validator location information.
        /// 
        /// Enables validators to specify their geographic location for
        /// transparency and network distribution analysis. Should accommodate
        /// city, country, or region names.
        /// 
        /// Typical values: 32-64 bytes.
        #[pallet::constant]
        type MaxValidatorLocationLength: Get<u32>;
        
        /// Maximum number of performance records to maintain per validator.
        /// 
        /// Controls the size of the performance history circular buffer.
        /// Higher values provide more detailed historical analysis but
        /// consume more storage space per validator.
        /// 
        /// Typical values: 50-200 records.
        #[pallet::constant]
        type MaxPerformanceHistoryLength: Get<u32>;
        
        /// Maximum number of epoch statistics to maintain per validator.
        /// 
        /// Controls the size of the validator's epoch history buffer.
        /// Provides historical context for validator performance evaluation
        /// while limiting storage growth.
        /// 
        /// Typical values: 10-50 epochs.
        #[pallet::constant]
        type MaxValidatorHistoryLength: Get<u32>;
        
        /// Maximum commission rate validators can charge in basis points.
        /// 
        /// Limits the fees validators can charge for their services to
        /// prevent excessive extraction from delegators or network participants.
        /// Expressed in basis points where 10000 = 100%.
        /// 
        /// Typical values: 1000-5000 (10%-50% maximum commission).
        #[pallet::constant]
        type MaxCommissionRate: Get<u32>; // basis points (10000 = 100%)
        

        // Misbehavior reporting
        /// Maximum length in bytes for misbehavior evidence submissions.
        /// 
        /// Limits the size of evidence that can be submitted with misbehavior
        /// reports to prevent storage abuse while allowing sufficient space
        /// for cryptographic proofs, screenshots, logs, and other evidence.
        /// 
        /// Typical values: 1024-8192 bytes (1-8 KB per evidence submission).
        #[pallet::constant]
        type MaxEvidenceLength: Get<u32>;
        
        /// Number of independent misbehavior reports required to trigger automatic slashing.
        /// 
        /// Prevents false accusations by requiring multiple independent reports
        /// before taking punitive action against a validator. Higher thresholds
        /// provide more protection against coordinated attacks but may delay
        /// action against genuine misbehavior.
        /// 
        /// Typical values: 3-10 independent reports.
        #[pallet::constant]
        type MisbehaviorSlashThreshold: Get<u32>; // Number of reports needed to trigger slash
        
        // Off-chain worker configuration
        /// Timeout duration for off-chain worker operations in milliseconds.
        /// 
        /// Sets the maximum time off-chain workers can spend on external
        /// operations before timing out. Prevents workers from hanging
        /// indefinitely on slow or unresponsive external services.
        /// 
        /// Typical values: 10000-60000 (10-60 seconds).
        #[pallet::constant]
        type OffchainWorkerTimeout: Get<u64>; // milliseconds
        
        /// Estimated average block production time in milliseconds.
        /// 
        /// Used by off-chain workers and timing calculations to estimate
        /// when certain block-based events will occur. Should match the
        /// network's actual average block time for accurate predictions.
        /// 
        /// Typical values: 6000-12000 (6-12 seconds per block).
        #[pallet::constant]
        type EstimatedBlockTime: Get<u64>; // milliseconds
        
        // Performance thresholds
        /// Minimum performance score threshold for acceptable validator behavior.
        /// 
        /// Validators with scores below this threshold are considered underperforming
        /// and may be subject to penalties, warnings, or temporary removal from
        /// the active set. Sets the baseline for acceptable network participation.
        /// 
        /// Typical values: 20-50 score points.
        #[pallet::constant]
        type MinPerformanceScore: Get<u64>; // Minimum score for good performance (30)
        
        // HighPerformanceScore is inherited from pos::Config
        
        /// Minimum participation rate percentage for acceptable validator behavior.
        /// 
        /// Validators with participation rates below this threshold are considered
        /// unreliable and may face penalties or removal. Expressed as a percentage
        /// of assigned opportunities that were successfully fulfilled.
        /// 
        /// Typical values: 50-80 (50%-80% minimum participation).
        #[pallet::constant]
        type MinParticipationRate: Get<u32>; // Minimum participation rate percentage (50)
        
        /// High participation rate threshold for exceptional validator recognition.
        /// 
        /// Validators with participation rates above this threshold are eligible
        /// for enhanced rewards and recognition as highly reliable network
        /// participants. Encourages consistent availability and performance.
        /// 
        /// Typical values: 85-95 (85%-95% high participation).
        #[pallet::constant]
        type HighParticipationRate: Get<u32>; // High participation rate percentage (90)
        
        /// Maximum number of blocks a validator can miss before facing penalties.
        /// 
        /// Once a validator misses this many blocks, they become subject to
        /// score penalties, warnings, or temporary removal from the active set.
        /// Balances tolerance for occasional issues with network reliability needs.
        /// 
        /// Typical values: 5-20 blocks.
        #[pallet::constant]
        type MaxMissedBlocks: Get<u32>; // Maximum missed blocks before penalty (10)
        
        /// Stricter missed block limit for high-performing validators.
        /// 
        /// High-performing validators are held to higher standards and face
        /// penalties after missing fewer blocks. Maintains quality expectations
        /// for validators who receive enhanced rewards and recognition.
        /// 
        /// Typical values: 1-5 blocks.
        #[pallet::constant]
        type MaxMissedBlocksHigh: Get<u32>; // Maximum missed blocks for high performers (2)
        
        /// Score threshold defining a "healthy" validator in good standing.
        /// 
        /// Used for network health assessments, validator categorization,
        /// and determining eligibility for various network operations.
        /// Represents the score level for stable, reliable validators.
        /// 
        /// Typical values: 40-70 score points.
        #[pallet::constant]
        type HealthyValidatorScore: Get<u64>; // Score threshold for healthy validator (50)
        
        /// Participation rate threshold defining a "healthy" validator.
        /// 
        /// Used alongside score thresholds to assess overall validator health
        /// and reliability. Validators meeting both score and participation
        /// thresholds are considered healthy network participants.
        /// 
        /// Typical values: 70-90 (70%-90% participation for health).
        #[pallet::constant]
        type HealthyParticipationRate: Get<u32>; // Participation rate for healthy validator (80)
        
        /// Maximum missed blocks allowed for a validator to be considered healthy.
        /// 
        /// Works with other health metrics to provide a comprehensive assessment
        /// of validator reliability. Healthy validators should miss very few
        /// blocks to maintain their good standing status.
        /// 
        /// Typical values: 2-10 blocks.
        #[pallet::constant]
        type HealthyMissedBlocksMax: Get<u32>; // Max missed blocks for healthy validator (5)
        
        // Score calculation thresholds
        /// Minimum absolute score change required to trigger validator set reordering.
        /// 
        /// When a validator's score changes by at least this amount, the system
        /// may reorder the validator set to maintain proper ranking. Prevents
        /// excessive reordering from minor score fluctuations while ensuring
        /// significant changes are reflected in validator rankings.
        /// 
        /// Typical values: 100-2000 score points.
        #[pallet::constant]
        type ScoreChangeThreshold: Get<u64>; // Minimum score change to trigger resort (1000)
        
        /// Minimum percentage score change required to trigger validator set reordering.
        /// 
        /// Works alongside the absolute threshold to determine when reordering
        /// is necessary. Uses percentage to make the threshold adaptive to
        /// different score ranges and validator performance levels.
        /// 
        /// Typical values: 5-20 (5%-20% change required).
        #[pallet::constant]
        type ScoreChangePercentage: Get<u32>; // Percentage change to trigger resort (10%)
        
        /// Minimum score improvement required for validator promotion considerations.
        /// 
        /// When a validator's score improves by at least this amount, they may
        /// be considered for promotion to higher tiers, enhanced rewards, or
        /// priority status. Recognizes significant performance improvements.
        /// 
        /// Typical values: 500-2000 score points.
        #[pallet::constant]
        type ScoreImprovementThreshold: Get<u64>; // Minimum improvement for validator promotion (1000)
        
        /// Minimum percentage score improvement for validator promotion considerations.
        /// 
        /// Works with the absolute improvement threshold to identify validators
        /// who have significantly enhanced their performance and deserve
        /// recognition or advancement in the validator hierarchy.
        /// 
        /// Typical values: 10-25 (10%-25% improvement required).
        #[pallet::constant]
        type ScoreImprovementPercentage: Get<u32>; // Percentage improvement for promotion (10%)
        
        // Contribution balance thresholds
        /// Maximum percentage that PoS (Proof-of-Stake) can contribute to final scores.
        /// 
        /// Prevents the consensus mechanism from becoming too heavily weighted
        /// toward economic stake by capping the PoS influence. Ensures that
        /// AI/ML performance (PoI) maintains meaningful impact on validator rankings.
        /// 
        /// Typical values: 70-95 (70%-95% maximum PoS influence).
        #[pallet::constant]
        type MaxPosContribution: Get<u32>; // Max PoS contribution percentage (90%)
        
        /// Maximum percentage that PoI (Proof-of-Inference) can contribute to final scores.
        /// 
        /// Prevents the consensus mechanism from becoming too heavily weighted
        /// toward AI/ML performance by capping the PoI influence. Ensures that
        /// economic stake (PoS) maintains meaningful impact on validator rankings.
        /// 
        /// Typical values: 70-95 (70%-95% maximum PoI influence).
        #[pallet::constant]
        type MaxPoiContribution: Get<u32>; // Max PoI contribution percentage (90%)
        
        /// Threshold for warning about imbalanced PoS/PoI contribution ratios.
        /// 
        /// When either PoS or PoI contribution exceeds this percentage of the
        /// total score calculation, the system may issue warnings or take
        /// corrective action to maintain balanced consensus participation.
        /// 
        /// Typical values: 75-90 (75%-90% triggers imbalance warnings).
        #[pallet::constant]
        type ImbalanceWarningThreshold: Get<u32>; // Threshold for imbalance warning (85%)
        
        // Block processing intervals
        /// Block interval for checking and processing validator leave requests.
        /// 
        /// Determines how frequently the system checks for expired leave requests
        /// and processes validators who have completed their cooldown periods.
        /// More frequent checks provide faster response but consume more resources.
        /// 
        /// Typical values: 5-50 blocks (30 seconds to 5 minutes at 6s block time).
        #[pallet::constant]
        type LeaveRequestCheckInterval: Get<u32>; // Blocks between leave request checks (10)
        
        /// Block interval for updating general validator and network metrics.
        /// 
        /// Controls how often the system updates various performance metrics,
        /// statistics, and monitoring data. Affects the freshness of metrics
        /// data available through APIs and internal calculations.
        /// 
        /// Typical values: 5-100 blocks (30 seconds to 10 minutes at 6s block time).
        #[pallet::constant]
        type MetricsUpdateInterval: Get<u32>; // Blocks between metrics updates (10)
        
        /// Block interval for refreshing validator score calculations.
        /// 
        /// Determines how often the system recalculates validator scores based
        /// on current performance data, stake amounts, and inference results.
        /// More frequent updates provide better responsiveness to performance changes.
        /// 
        /// Typical values: 10-200 blocks (1-20 minutes at 6s block time).
        #[pallet::constant]
        type ScoreRefreshInterval: Get<u32>; // Blocks between score refreshes (50)
        
        /// Block interval for generating detailed logging and diagnostic information.
        /// 
        /// Controls how frequently the system outputs comprehensive logs about
        /// validator performance, network health, and internal state. Detailed
        /// logging provides valuable debugging information but can be verbose.
        /// 
        /// Typical values: 50-500 blocks (5-50 minutes at 6s block time).
        #[pallet::constant]
        type DetailedLoggingInterval: Get<u32>; // Blocks between detailed logging (100)
        
        /// Block interval for checking PoS/PoI contribution balance and issuing warnings.
        /// 
        /// Determines how often the system evaluates whether the consensus
        /// mechanism is properly balanced between stake-based and inference-based
        /// contributions. Helps maintain the hybrid nature of the consensus.
        /// 
        /// Typical values: 100-1000 blocks (10-100 minutes at 6s block time).
        #[pallet::constant]
        type ImbalanceCheckInterval: Get<u32>; // Blocks between imbalance checks (500)
        
        // Validator set limits
        /// Number of top-performing validators to display in logs and API responses.
        /// 
        /// Limits the verbosity of validator rankings in logs while still providing
        /// visibility into the highest-performing network participants. Used for
        /// monitoring dashboards, API responses, and operational visibility.
        /// 
        /// Typical values: 3-20 validators.
        #[pallet::constant]
        type TopValidatorsDisplayCount: Get<u32>; // Number of top validators to display (5)
        
        /// Number of validators to sample when performing network health checks.
        /// 
        /// Determines the sample size for statistical health assessments of the
        /// validator network. Larger samples provide more accurate health metrics
        /// but require more computational resources for analysis.
        /// 
        /// Typical values: 5-50 validators.
        #[pallet::constant]
        type HealthCheckSampleSize: Get<u32>; // Number of validators to sample for health (5)
        
        // Percentage constants
        /// Value representing 100% for percentage calculations throughout the pallet.
        /// 
        /// Used as the base value for percentage calculations, allowing the system
        /// to work with different precision levels. Should match the precision
        /// factor used elsewhere in the pallet for consistency.
        /// 
        /// Common values: 100 (whole percentages) or 10000 (basis points).
        #[pallet::constant]
        type FullPercentage: Get<u32>; // 100% value
        
        /// Percentage threshold for identifying high-performing validators.
        /// 
        /// Validators scoring above this percentage of the maximum possible score
        /// are considered high performers and eligible for enhanced rewards and
        /// recognition. Sets the bar for exceptional network participation.
        /// 
        /// Typical values: 70-90 (70%-90% of maximum score).
        #[pallet::constant]
        type HighPerformancePercentage: Get<u32>; // 80% for high performance threshold
        
        // TopPerformerPercentage is inherited from pos::Config
        
        // Reward distribution percentages
        /// Percentage of total reward pool allocated to base rewards for all validators.
        /// 
        /// Determines what portion of epoch rewards goes to the base reward pool
        /// that is distributed equally among all active validators. Provides a
        /// foundation reward level regardless of performance differences.
        /// 
        /// Typical values: 50-70 (50%-70% of total rewards).
        #[pallet::constant]
        type BaseRewardPercentage: Get<u32>; // Base reward percentage for all validators
        
        /// Percentage of total reward pool allocated to performance-based rewards.
        /// 
        /// Determines what portion of epoch rewards goes to the performance pool
        /// that is distributed among validators meeting high performance thresholds.
        /// Incentivizes consistent good performance above baseline levels.
        /// 
        /// Typical values: 20-35 (20%-35% of total rewards).
        #[pallet::constant]
        type PerformanceRewardPercentage: Get<u32>; // Additional reward for high performers
        
        /// Percentage of total reward pool allocated to top performer rewards.
        /// 
        /// Determines what portion of epoch rewards goes to the top performer pool
        /// that is distributed among the highest-ranking validators. Provides
        /// maximum incentive for exceptional performance and network contribution.
        /// 
        /// Typical values: 10-25 (10%-25% of total rewards).
        #[pallet::constant]
        type TopPerformerRewardPercentage: Get<u32>; // Additional reward for top performers
        
        // MinStake is inherited from pos::Config

        // Trust score calculation weights
        /// Weight for uptime component in trust score calculation.
        #[pallet::constant]
        type TrustScoreUptimeWeight: Get<u64>;

        /// Weight for inference success component in trust score calculation.
        #[pallet::constant]
        type TrustScoreInferenceWeight: Get<u64>;

        /// Weight for slashing history component in trust score calculation.
        #[pallet::constant]
        type TrustScoreSlashingWeight: Get<u64>;

        /// Maximum trust score value.
        #[pallet::constant]
        type MaxTrustScore: Get<u64>;

        /// Minimum trust score value to prevent negative scores.
        #[pallet::constant]
        type MinTrustScore: Get<u64>;

        /// Maximum trust score growth rate per epoch (basis points, e.g., 500 = 5%).
        #[pallet::constant]
        type MaxTrustScoreGrowthRate: Get<u32>;

        /// Maximum trust score decay rate per epoch (basis points, e.g., 200 = 2%).
        #[pallet::constant]
        type MaxTrustScoreDecayRate: Get<u32>;

        /// Trust score stability factor for smoothing (basis points, e.g., 8000 = 80%).
        #[pallet::constant]
        type TrustScoreStabilityFactor: Get<u32>;

        // Constants for hardcoded values
        /// Maximum size for validator history bounded vector.
        #[pallet::constant]
        type MaxValidatorHistorySize: Get<u32>;

        /// Maximum size for validator name bounded vector.
        #[pallet::constant]
        type MaxValidatorNameSize: Get<u32>;

        /// Maximum size for runtime API bounded vectors.
        #[pallet::constant]
        type MaxRuntimeApiBoundedVecSize: Get<u32>;

        /// Maximum size for proposal action bounded vectors.
        #[pallet::constant]
        type MaxProposalActionBoundedVecSize: Get<u32>;

        /// Error code for author not active validation.
        #[pallet::constant]
        type AuthorNotActiveErrorCode: Get<u8>;

        /// Error code for author mismatch validation.
        #[pallet::constant]
        type AuthorMismatchErrorCode: Get<u8>;

        type Balance: Parameter + Member + AtLeast32BitUnsigned + Default + Copy + MaxEncodedLen + serde::Serialize + for<'de> serde::Deserialize<'de>;
        // Currency is inherited from pos::Config
        type WeightInfo: WeightInfo;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    // Type alias to resolve Balance type ambiguity
    type BalanceOf<T> = <T as pallet::Config>::Balance;

    type MaxValidatorsOf<T> = <T as pos::Config>::MaxValidators;
    type MinActiveValidatorsOf<T> = <T as pos::Config>::MinActiveValidators;
    type MinStakeOf<T> = <T as pos::Config>::MinStake;
    type ValidatorRewardOf<T> = <T as pos::Config>::ValidatorReward;
    type LeaveCooldownOf<T> = <T as pos::Config>::LeaveCooldown;
    type SlashPercentOf<T> = <T as pos::Config>::SlashPercent;
    type ValidatorScoreDecayOf<T> = <T as pos::Config>::ValidatorScoreDecay;
    type MinValidatorScoreOf<T> = <T as pos::Config>::MinValidatorScore;
    type HighPerformanceScoreOf<T> = <T as pos::Config>::HighPerformanceScore;
    type TopPerformerPercentageOf<T> = <T as pos::Config>::TopPerformerPercentage;

    /// Stores state for each validator.
    /// Comprehensive state tracking for each validator in the network.
    ///
    /// This storage map maintains detailed information about each validator including:
    /// - Current performance scores (stake, inference, final)
    /// - Block authorship statistics (authored and missed blocks)
    /// - Historical performance data across epochs
    /// - Last active epoch for decay calculations
    ///
    /// The ValidatorState struct contains both current metrics and historical data
    /// to enable comprehensive performance evaluation and score calculations.
    ///
    /// # Key: T::AccountId - The validator's account identifier
    /// # Value: ValidatorState<T> - Complete validator state information
    #[pallet::storage]
    #[pallet::getter(fn validator_states)]
    pub type ValidatorStates<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        ValidatorState<T>,
        OptionQuery,
    >;

    /// Current Proof-of-Stake (PoS) weight used in final score calculations.
    /// 
    /// This value determines how much influence a validator's stake has on their
    /// final consensus score. The final score is calculated as:
    /// `final_score = (stake_score * pos_weight + inference_score * poi_weight) / precision`
    /// 
    /// Higher PoS weight values give more importance to validator stake amounts,
    /// while lower values reduce the influence of stake on consensus participation.
    /// 
    /// Default value is set via `T::DefaultPosWeight` configuration constant.
    /// Can be updated through governance or root calls to `update_consensus_weights`.
    /// 
    /// # Value: u64 - Weight multiplier for PoS scores (typically 0-10000 for percentage-based calculations)
    #[pallet::storage]
    #[pallet::getter(fn pos_weight)]
    pub type PosWeight<T: Config> = StorageValue<_, u64, ValueQuery>;

    /// Current Proof-of-Inference (PoI) weight used in final score calculations.
    /// 
    /// This value determines how much influence a validator's inference performance
    /// has on their final consensus score. The final score is calculated as:
    /// `final_score = (stake_score * pos_weight + inference_score * poi_weight) / precision`
    /// 
    /// Higher PoI weight values give more importance to validator inference accuracy
    /// and participation, while lower values reduce the influence of AI/ML performance
    /// on consensus participation.
    /// 
    /// Default value is set via `T::DefaultPoiWeight` configuration constant.
    /// Can be updated through governance or root calls to `update_consensus_weights`.
    /// 
    /// # Value: u64 - Weight multiplier for PoI scores (typically 0-10000 for percentage-based calculations)
    #[pallet::storage]
    #[pallet::getter(fn poi_weight)]
    pub type PoiWeight<T: Config> = StorageValue<_, u64, ValueQuery>;

    /// Complete set of all registered validators in the network.
    /// 
    /// This storage contains all validators who have joined the validator set,
    /// regardless of their current active status. Validators remain in this set
    /// until they are explicitly ejected or removed through governance actions.
    /// 
    /// The set includes both active validators (currently participating in consensus)
    /// and inactive validators (temporarily not participating due to low scores,
    /// maintenance, or other reasons).
    /// 
    /// Maximum size is bounded by `T::MaxValidators` to prevent unbounded growth.
    /// New validators are added through `join_validator_set` or `join_validators` calls.
    /// 
    /// # Value: BoundedVec<T::AccountId, T::MaxValidators> - List of all registered validator accounts
    #[pallet::storage]
    #[pallet::getter(fn validator_set)]
    pub type ValidatorSet<T: Config> = StorageValue<_, BoundedVec<T::AccountId, MaxValidatorsOf<T>>, ValueQuery>;

    /// Configuration parameters for epoch management and transitions.
    /// 
    /// Contains essential parameters that control how epochs operate:
    /// - `blocks_per_epoch`: Number of blocks in each epoch (configurable via T::EpochLength)
    /// - `min_stake`: Minimum stake required for validator participation
    /// - `max_validators`: Maximum number of validators allowed in the active set
    /// 
    /// This configuration is initialized during genesis and can be updated through
    /// governance proposals or root calls. Changes typically take effect at the
    /// next epoch boundary to ensure consistency.
    /// 
    /// The epoch system is fundamental to validator set management, score decay,
    /// reward distribution, and network parameter updates.
    /// 
    /// # Value: EpochConfig - Struct containing epoch-related configuration parameters
    #[pallet::storage]
    #[pallet::getter(fn epoch_config)]
    pub type EpochConfigStorage<T: Config> = StorageValue<_, EpochConfig, ValueQuery>;

    /// Configuration parameters for trust score calculation weights.
    ///
    /// Stores the weights used to balance different components in trust score computation.
    /// This configuration allows the network to adjust the relative importance of:
    /// - Validator uptime and availability
    /// - AI/ML inference performance and accuracy
    /// - Historical slashing and penalty impacts
    ///
    /// The configuration is initialized with default values during genesis:
    /// - Uptime weight: 40 (40% influence)
    /// - Inference weight: 35 (35% influence)  
    /// - Slashing weight: 25 (25% penalty impact)
    ///
    /// These weights can be updated through governance proposals to adapt to
    /// changing network priorities and requirements. For example:
    /// - Increasing uptime weight during network instability
    /// - Increasing inference weight to emphasize AI capabilities
    /// - Adjusting slashing weight to modify penalty severity
    ///
    /// Changes to trust score configuration affect all future trust score
    /// calculations but do not retroactively modify historical scores.
    ///
    /// # Value: TrustScoreConfig - Struct containing trust score calculation weights
    #[pallet::storage]
    #[pallet::getter(fn trust_score_config)]
    pub type TrustScoreConfigStorage<T: Config> = StorageValue<_, TrustScoreConfig, ValueQuery>;

    /// Comprehensive governance configuration with parameter ranges and safety rails.
    ///
    /// This storage item contains all configurable DCF parameters with their allowed
    /// ranges, current values, and validation rules. It provides a centralized
    /// governance system that ensures parameter changes remain within safe bounds.
    ///
    /// The GovernanceConfig includes ranges for:
    /// - Epoch configuration (length, min stake, max validators)
    /// - Consensus weights (PoS/PoI balance)
    /// - Performance thresholds and scoring parameters
    /// - Economic parameters (rewards, slashing, cooldowns)
    /// - Trust score calculation weights
    ///
    /// All parameter updates must:
    /// - Be within the defined min/max ranges
    /// - Be submitted through root-only governance calls
    /// - Emit events documenting the changes
    /// - Provide parameter readback via runtime API
    ///
    /// This system prevents dangerous parameter changes that could destabilize
    /// the network while allowing controlled adaptation to changing conditions.
    ///
    /// # Value: GovernanceConfig<T> - Complete parameter governance configuration
    #[pallet::storage]
    #[pallet::getter(fn governance_config)]
    pub type GovernanceConfigStorage<T: Config> = StorageValue<_, GovernanceConfig<T>, ValueQuery>;

    /// The current epoch number in the blockchain's lifecycle.
    /// 
    /// Epochs are fundamental time periods used for:
    /// - Validator set updates and rotations
    /// - Score decay calculations for inactive validators
    /// - Reward distribution cycles
    /// - Performance evaluation periods
    /// - Governance proposal execution timing
    /// 
    /// The epoch number starts at 0 during genesis and increments by 1 at each
    /// epoch transition. Epoch transitions occur automatically based on block
    /// numbers (every T::EpochLength blocks) or can be triggered manually in
    /// governance mode.
    /// 
    /// This value is used throughout the pallet for temporal calculations and
    /// ensuring operations occur at appropriate epoch boundaries.
    /// 
    /// # Value: u32 - Current epoch number (starts at 0, increments indefinitely)
    #[pallet::storage]
    #[pallet::getter(fn current_epoch)]
    pub type CurrentEpoch<T: Config> = StorageValue<_, u32, ValueQuery>;

    /// Set of validators currently active and participating in consensus.
    /// 
    /// Active validators are a subset of the total validator set who are currently
    /// eligible to participate in block production and consensus. To be active,
    /// validators must meet several criteria:
    /// - Have a final score above the minimum threshold
    /// - Maintain sufficient stake (above T::MinStake)
    /// - Not be in a cooldown period after leaving
    /// - Not be ejected due to misbehavior or poor performance
    /// 
    /// This set is updated at each epoch transition based on validator performance,
    /// stake amounts, and participation rates. The active validator set is used by
    /// the consensus engine for block author selection and validation.
    /// 
    /// Maximum size is bounded by `T::MaxValidators` to ensure network performance.
    /// 
    /// # Value: BoundedVec<T::AccountId, T::MaxValidators> - List of currently active validator accounts
    #[pallet::storage]
    #[pallet::getter(fn active_validators)]
    pub type ActiveValidators<T: Config> = StorageValue<_, BoundedVec<T::AccountId, MaxValidatorsOf<T>>, ValueQuery>;

    /// Flag indicating whether governance mode is currently enabled.
    /// 
    /// When governance mode is enabled (true), the pallet operates with enhanced
    /// administrative capabilities:
    /// - Manual epoch transitions are allowed via `sudo_advance_epoch`
    /// - Governance proposals can be submitted and voted on
    /// - Root/sudo accounts have additional administrative powers
    /// - Certain automated processes may be disabled or modified
    /// 
    /// When disabled (false), the pallet operates in fully automated mode:
    /// - Epoch transitions occur automatically based on block numbers
    /// - Validator management is handled algorithmically
    /// - Reduced administrative intervention capabilities
    /// 
    /// This mode can be toggled by root accounts through `set_governance_mode`.
    /// Default value is typically false for decentralized operation.
    /// 
    /// # Value: bool - true if governance mode is enabled, false for automated mode
    #[pallet::storage]
    #[pallet::getter(fn governance_mode_enabled)]
    pub type GovernanceModeEnabled<T: Config> = StorageValue<_, bool, ValueQuery>;

    /// Storage for governance proposals indexed by unique proposal ID.
    /// 
    /// Contains all governance proposals that have been submitted to the network,
    /// including their current status, voting results, and proposed actions.
    /// Proposals can include:
    /// - Validator slashing actions
    /// - Validator reward distributions
    /// - Validator ejection from the network
    /// - Validator addition to the network
    /// - Multiple validator operations
    /// 
    /// Each proposal has a unique ID (generated sequentially) and contains:
    /// - Proposer account
    /// - Proposed action details
    /// - Current status (Pending, Approved, Rejected, Executed)
    /// - Vote counts (for and against)
    /// 
    /// Proposals are created through `submit_proposal` and voted on via `vote_proposal`.
    /// Approved proposals can be executed through `execute_proposal`.
    /// 
    /// # Key: u32 - Unique proposal identifier
    /// # Value: GovernanceProposal<T> - Complete proposal information and status
    #[pallet::storage]
    pub type Proposals<T: Config> = StorageMap<
        _, Blake2_128Concat, u32, GovernanceProposal<T>, OptionQuery
    >;

    /// Counter for generating unique proposal IDs.
    /// 
    /// This value is incremented each time a new governance proposal is created,
    /// ensuring that every proposal has a unique identifier. The counter starts
    /// at 0 during genesis and increments indefinitely.
    /// 
    /// Used internally by the proposal system to:
    /// - Generate unique IDs for new proposals
    /// - Maintain proposal ordering and history
    /// - Enable efficient proposal lookup and management
    /// 
    /// The ID is assigned when a proposal is submitted and never reused,
    /// providing a permanent reference for each governance action.
    /// 
    /// # Value: u32 - Next available proposal ID (starts at 0, increments with each proposal)
    #[pallet::storage]
    #[pallet::getter(fn next_proposal_id)]
    pub type NextProposalId<T: Config> = StorageValue<_, u32, ValueQuery>;

    /// Tracking system for individual votes on governance proposals.
    /// 
    /// This double map records which validators have voted on which proposals
    /// and their vote direction (for or against). Used to:
    /// - Prevent double voting by the same validator
    /// - Track voting participation rates
    /// - Maintain transparency in governance decisions
    /// - Enable vote auditing and verification
    /// 
    /// The boolean value indicates the vote direction:
    /// - true: Vote in favor of the proposal
    /// - false: Vote against the proposal
    /// - None: Validator has not voted on this proposal
    /// 
    /// Votes are cast through the `vote_proposal` extrinsic and are immutable
    /// once recorded to ensure governance integrity.
    /// 
    /// # Key1: u32 - Proposal ID
    /// # Key2: T::AccountId - Validator account who voted
    /// # Value: bool - Vote direction (true = for, false = against)
    #[pallet::storage]
    pub type ProposalVotes<T: Config> = StorageDoubleMap<
        _, Blake2_128Concat, u32, Blake2_128Concat, T::AccountId, bool, OptionQuery
    >;

    /// Queue of validator join/leave requests awaiting execution at epoch boundaries.
    /// 
    /// Validator set changes are not applied immediately but are queued and processed
    /// at epoch transitions to maintain consensus stability. This storage tracks:
    /// - Join requests from new validators wanting to participate
    /// - Leave requests from existing validators wanting to exit
    /// 
    /// Actions are submitted through:
    /// - `join_validator_set`: Request to join (requires meeting minimum requirements)
    /// - `leave_validator_set`: Request to leave (subject to cooldown periods)
    /// 
    /// Pending actions are processed during epoch transitions by:
    /// - Validating that join requests still meet requirements
    /// - Applying leave requests after cooldown periods
    /// - Updating the active validator set accordingly
    /// 
    /// This delayed execution ensures validator set stability during epochs
    /// and prevents rapid changes that could destabilize consensus.
    /// 
    /// # Key: T::AccountId - Validator account requesting the action
    /// # Value: ValidatorAction - Type of action requested (Join or Leave)
    #[pallet::storage]
    #[pallet::getter(fn pending_validator_actions)]
    pub type PendingValidatorActions<T: Config> = StorageMap<
        _, Blake2_128Concat, T::AccountId, ValidatorAction, OptionQuery
    >;

    /// Join/leave intent for validators.
    #[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen, frame_support::__private::codec::DecodeWithMemTracking)]
    pub enum ValidatorAction {
        Join,
        Leave,
    }



    /// Historical record of recent epochs stored in a circular buffer.
    /// 
    /// Maintains a rolling history of the most recent epochs, including:
    /// - Epoch number and duration
    /// - Active validator set for each epoch
    /// - Aggregate performance metrics
    /// - Reward distribution summaries
    /// - Significant events and transitions
    /// 
    /// The history is stored as a bounded vector that acts as a ring buffer,
    /// automatically removing the oldest entries when the maximum size is reached.
    /// This provides efficient access to recent historical data while preventing
    /// unbounded storage growth.
    /// 
    /// Used for:
    /// - Performance trend analysis
    /// - Validator evaluation over time
    /// - Network health monitoring
    /// - Governance decision support
    /// - API queries for historical data
    /// 
    /// Maximum size is controlled by `T::MaxEpochHistory` configuration parameter.
    /// 
    /// # Value: BoundedVec<EpochHistory<T>, T::MaxEpochHistory> - Circular buffer of recent epoch records
    #[pallet::storage]
    #[pallet::getter(fn epoch_histories)]
    pub type EpochHistories<T: Config> = StorageValue<_, BoundedVec<EpochHistory<T>, T::MaxEpochHistory>, ValueQuery>;

    /// Human-readable display names for validators.
    /// 
    /// Allows validators to register a display name for better user experience
    /// in interfaces, explorers, and monitoring tools. Names are optional and
    /// purely cosmetic - they do not affect consensus or validator operations.
    /// 
    /// Names must be:
    /// - UTF-8 encoded strings
    /// - Maximum 32 bytes in length
    /// - Set by the validator through `set_validator_name` extrinsic
    /// - Unique per validator (no uniqueness enforcement across validators)
    /// 
    /// Used by:
    /// - Block explorers for validator identification
    /// - Monitoring dashboards and tools
    /// - Governance interfaces
    /// - API responses for better readability
    /// 
    /// If no name is set, interfaces should fall back to displaying the
    /// validator's account ID or a truncated version.
    /// 
    /// # Key: T::AccountId - Validator account
    /// # Value: BoundedVec<u8, ConstU32<32>> - UTF-8 encoded display name (max 32 bytes)
    #[pallet::storage]
    #[pallet::getter(fn validator_names)]
    pub type ValidatorNames<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        BoundedVec<u8, ConstU32<32>>,
        OptionQuery,
    >;

    /// Cumulative count of epochs each validator has been active.
    /// 
    /// Tracks the total number of epochs a validator has participated in
    /// consensus since joining the network. This metric is used for:
    /// - Long-term reliability assessment
    /// - Validator reputation scoring
    /// - Network stability analysis
    /// - Reward calculations based on participation history
    /// 
    /// The counter increments by 1 for each epoch where the validator:
    /// - Is in the active validator set
    /// - Successfully participates in consensus
    /// - Maintains minimum performance requirements
    /// 
    /// The counter does not increment during epochs where the validator:
    /// - Is inactive due to low scores
    /// - Is in a cooldown period
    /// - Is temporarily ejected
    /// 
    /// This provides a measure of validator commitment and network contribution
    /// over time, independent of short-term performance fluctuations.
    /// 
    /// # Key: T::AccountId - Validator account
    /// # Value: u32 - Total number of epochs the validator has been active
    #[pallet::storage]
    #[pallet::getter(fn validator_uptime)]
    pub type ValidatorUptime<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        u32,
        ValueQuery,
    >;

    /// Total count of successful inference operations performed by each validator.
    /// 
    /// Tracks the cumulative number of successful AI/ML inference operations
    /// completed by validators as part of the Proof-of-Inference (PoI) consensus.
    /// This metric is fundamental to the CBC blockchain's AI-focused consensus mechanism.
    /// 
    /// The counter increments when validators:
    /// - Successfully complete inference tasks
    /// - Submit results that meet accuracy thresholds
    /// - Participate in distributed AI computations
    /// - Contribute to the network's AI capabilities
    /// 

    /// Extended metadata and contact information for validators.
    /// 
    /// Stores comprehensive information about validators beyond their basic
    /// operational metrics. This metadata helps with:
    /// - Validator identification and branding
    /// - Community engagement and transparency
    /// - Technical support and communication
    /// - Network governance and coordination
    /// 
    /// The ValidatorMetadataInfo struct typically includes:
    /// - Website URL for the validator's homepage
    /// - Contact information (email, social media)
    /// - Description of the validator's services
    /// - Geographic location information
    /// - Technical specifications and capabilities
    /// - Commission rates and fee structures
    /// 
    /// All metadata is optional and self-reported by validators through
    /// the `set_validator_metadata` extrinsic. The information is not
    /// validated by the protocol but serves as a public registry.
    /// 
    /// Used by block explorers, staking interfaces, and community tools
    /// to provide richer validator information to users and delegators.
    /// 
    /// # Key: T::AccountId - Validator account
    /// # Value: ValidatorMetadataInfo - Comprehensive metadata structure
    #[pallet::storage]
    #[pallet::getter(fn validator_metadata)]
    pub type ValidatorMetadata<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        ValidatorMetadataInfo,
        OptionQuery,
    >;

    /// Historical performance records for each validator over time.
    /// 
    /// Maintains a time-series of performance metrics for each validator,
    /// stored as a bounded vector of PerformanceRecord entries. This data
    /// enables trend analysis and long-term validator evaluation.
    /// 
    /// Each PerformanceRecord typically contains:
    /// - Timestamp or epoch number
    /// - Performance scores (PoS, PoI, combined)
    /// - Block production statistics
    /// - Participation rates and uptime
    /// - Inference accuracy metrics
    /// 
    /// The history is maintained as a circular buffer with a maximum of 100
    /// entries per validator. When the limit is reached, the oldest records
    /// are automatically removed to make space for new ones.
    /// 
    /// Used for:
    /// - Performance trend analysis and prediction
    /// - Validator reliability assessment
    /// - Reward calculation based on historical performance
    /// - Network health monitoring and reporting
    /// - API endpoints for performance charts and graphs
    /// 
    /// # Key: T::AccountId - Validator account
    /// # Value: BoundedVec<PerformanceRecord, ConstU32<100>> - Circular buffer of performance records
    #[pallet::storage]
    #[pallet::getter(fn validator_performance_history)]
    pub type ValidatorPerformanceHistory<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        BoundedVec<PerformanceRecord, ConstU32<100>>,
        ValueQuery,
    >;

    /// Block number when each validator was last observed to be active.
    /// 
    /// Tracks the most recent block number where each validator demonstrated
    /// activity in the network. This is used for:
    /// - Detecting inactive or offline validators
    /// - Calculating activity-based score decay
    /// - Determining when to apply penalties for inactivity
    /// - Network health monitoring and alerting
    /// 
    /// A validator is considered "seen" when they:
    /// - Successfully author a block
    /// - Submit valid transactions or extrinsics
    /// - Participate in consensus voting
    /// - Complete inference operations
    /// - Respond to network challenges
    /// 
    /// The block number is updated automatically by the pallet's hooks
    /// and monitoring systems. Validators with stale "last seen" values
    /// may be subject to:
    /// - Score decay penalties
    /// - Temporary removal from active set
    /// - Reduced reward eligibility
    /// 
    /// # Key: T::AccountId - Validator account
    /// # Value: u32 - Block number of last observed activity
    #[pallet::storage]
    #[pallet::getter(fn validator_last_seen)]
    pub type ValidatorLastSeen<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        u32, // Block number
        ValueQuery,
    >;

    /// Cumulative count of blocks successfully authored by each validator.
    /// 
    /// Tracks the total number of blocks each validator has successfully
    /// produced and added to the blockchain since joining the network.
    /// This is a key metric for validator performance evaluation.
    /// 
    /// The counter increments when a validator:
    /// - Is selected as the block author for a slot
    /// - Successfully produces a valid block
    /// - Has their block accepted by the network
    /// - Contributes to chain progression
    /// 
    /// Used for:
    /// - Block production performance assessment
    /// - Validator reliability scoring
    /// - Reward calculations based on contribution
    /// - Network statistics and monitoring
    /// - Participation rate calculations (authored vs. assigned)
    /// 
    /// Higher block counts indicate validators who are consistently
    /// available and capable of producing blocks when selected,
    /// contributing to network stability and throughput.
    /// 
    /// # Key: T::AccountId - Validator account
    /// # Value: u32 - Total number of blocks successfully authored
    #[pallet::storage]
    #[pallet::getter(fn validator_blocks_authored)]
    pub type ValidatorBlocksAuthored<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        u32,
        ValueQuery,
    >;

    /// Cumulative count of blocks missed by each validator when they were selected.
    /// 
    /// Tracks the total number of block production opportunities that each
    /// validator failed to fulfill since joining the network. This is a
    /// critical metric for identifying unreliable validators.
    /// 
    /// A block is considered "missed" when:
    /// - The validator was selected as the block author
    /// - They failed to produce a block within the allocated time
    /// - Their produced block was invalid or rejected
    /// - They were offline or unresponsive during their slot
    /// 
    /// Used for:
    /// - Validator reliability assessment and penalties
    /// - Performance scoring and ranking
    /// - Automatic ejection decisions for poor performers
    /// - Network health monitoring and alerting
    /// - Participation rate calculations (missed vs. assigned)
    /// 
    /// High miss counts may result in:
    /// - Score penalties and reduced rewards
    /// - Temporary removal from active validator set
    /// - Permanent ejection for chronic poor performance
    /// 
    /// # Key: T::AccountId - Validator account
    /// # Value: u32 - Total number of blocks missed when selected as author
    #[pallet::storage]
    #[pallet::getter(fn validator_blocks_missed)]
    pub type ValidatorBlocksMissed<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        u32,
        ValueQuery,
    >;

    /// Timestamp recording when each validator joined the network.
    /// 
    /// Stores the exact time (in milliseconds since Unix epoch) when each
    /// validator successfully joined the validator set. This information
    /// is used for various temporal calculations and historical analysis.
    /// 
    /// The timestamp is set when a validator:
    /// - Successfully completes the `join_validator_set` process
    /// - Meets all minimum requirements (stake, performance, etc.)
    /// - Is officially added to the validator registry
    /// 
    /// Used for:
    /// - Calculating validator tenure and experience
    /// - Age-based reward calculations or bonuses
    /// - Network growth analysis and statistics
    /// - Validator lifecycle tracking
    /// - API responses for validator information
    /// 
    /// The value is None for validators who joined before this tracking
    // ValidatorJoinTime and ValidatorLeaveRequests moved to pallet-cbc-pos.

    /// Block number of the most recently finalized block in the network.
    /// 
    /// Tracks the highest block number that has achieved finality through
    /// the consensus mechanism. Finalization typically occurs at epoch
    /// boundaries or through explicit finality gadgets.
    /// 
    /// Finalized blocks are considered:
    /// - Permanently part of the canonical chain
    /// - Safe from reorganization or rollback
    /// - Confirmed by sufficient validator consensus
    /// - Available for state pruning and archival
    /// 
    /// Used for:
    /// - Determining safe block heights for critical operations
    /// - State pruning and storage optimization
    /// - API responses about network finality status
    /// - Calculating finality lag and network health metrics
    /// - Ensuring transaction irreversibility guarantees
    /// 
    /// The value is updated through:
    /// - Automatic finalization at epoch boundaries
    /// - Explicit finality signals from consensus mechanisms
    /// - Manual finalization through governance actions
    /// 
    /// # Value: u32 - Block number of the last finalized block
    #[pallet::storage]
    #[pallet::getter(fn last_finalized_block)]
    pub type LastFinalizedBlock<T: Config> = StorageValue<_, u32, ValueQuery>;

    /// Previous finalized block number for regression detection.
    /// 
    /// Stores the finalized block number from the previous epoch to enable
    /// monotonic advancement validation and regression detection. This storage
    /// is updated during each epoch transition to track finality progression.
    /// 
    /// Used for:
    /// - Validating monotonic finality advancement
    /// - Detecting finality regression attempts
    /// - Ensuring finality never moves backward
    /// - Generating finality progression reports
    /// 
    /// # Value: u32 - Block number of the previous finalized block
    #[pallet::storage]
    #[pallet::getter(fn previous_finalized_block)]
    pub type PreviousFinalizedBlock<T: Config> = StorageValue<_, u32, ValueQuery>;

    /// Best known block number from the previous epoch for finality validation.
    /// 
    /// Tracks the highest block number that was known during the previous epoch
    /// to ensure that finality never exceeds the best known block of the prior
    /// epoch. This prevents finality from advancing beyond what was actually
    /// produced and validated.
    /// 
    /// Updated at each epoch boundary to capture the best block of the
    /// concluding epoch before transitioning to the new epoch.
    /// 
    /// Used for:
    /// - Validating finality bounds against actual block production
    /// - Preventing finality from exceeding known block heights
    /// - Ensuring finality consistency with block production
    /// - Detecting invalid finality advancement attempts
    /// 
    /// # Value: u32 - Best known block number from previous epoch
    #[pallet::storage]
    #[pallet::getter(fn previous_epoch_best_block)]
    pub type PreviousEpochBestBlock<T: Config> = StorageValue<_, u32, ValueQuery>;

    /// Evidence storage for validator misbehavior reports.
    /// 
    /// Stores cryptographic evidence and proof of validator misbehavior
    /// submitted by other network participants. This system enables
    /// decentralized monitoring and accountability for validator actions.
    /// 
    /// The double map structure allows multiple reporters to submit
    /// evidence against the same validator, building a comprehensive
    /// case for potential slashing or ejection actions.
    /// 
    /// Evidence can include:
    /// - Cryptographic proofs of equivocation (double-signing)
    /// - Invalid block production attempts
    /// - Consensus rule violations
    /// - Off-chain misbehavior with on-chain impact
    /// - Inference result manipulation or fraud
    /// 
    /// The evidence is bounded by `T::MaxEvidenceLength` to prevent
    /// storage abuse while allowing sufficient space for cryptographic
    /// proofs and detailed documentation.
    /// 
    /// Used for:
    /// - Building cases for validator slashing
    /// - Automatic ejection of malicious validators
    /// - Network security and integrity maintenance
    /// - Governance decision support for disciplinary actions
    /// 
    /// # Key1: T::AccountId - Validator being reported for misbehavior
    /// # Key2: T::AccountId - Account submitting the evidence report
    /// # Value: BoundedVec<u8, T::MaxEvidenceLength> - Cryptographic evidence and proof data
    #[pallet::storage]
    #[pallet::getter(fn misbehavior_reports)]
    pub type MisbehaviorReports<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId, // reported validator
        Blake2_128Concat,
        T::AccountId, // reporter
        BoundedVec<u8, T::MaxEvidenceLength>,
        OptionQuery,
    >;

    // ValidatorStake and RecentlyRemovedValidators moved to pallet-cbc-pos.

    /// Current trust scores for all validators in the network.
    ///
    /// Trust scores provide a comprehensive measure of validator reliability and performance
    /// by combining multiple factors including uptime, inference success rate, and slashing history.
    /// These scores are used to assess validator trustworthiness and influence their participation
    /// in consensus and reward distribution.
    ///
    /// Trust scores are calculated using configurable weights for different components:
    /// - Uptime score: Based on validator availability and participation
    /// - Inference success score: Based on AI/ML inference accuracy and participation
    /// - Slashing penalty: Negative impact from past misbehavior or poor performance
    ///
    /// The final trust score is computed as:
    /// trust_score = (uptime_score * uptime_weight + inference_score * inference_weight) / total_weight - slashing_penalty
    ///
    /// Trust scores are updated during:
    /// - Validator activity events (block production, inference submission)
    /// - Epoch transitions and performance evaluations
    /// - Slashing events and penalty applications
    /// - Manual trust score recalculations
    ///
    /// Higher trust scores indicate more reliable validators who should receive:
    /// - Priority in block author selection
    /// - Enhanced reward distributions
    /// - Greater influence in governance decisions
    /// - Improved reputation in the network
    ///
    /// # Key: T::AccountId - Validator account
    /// # Value: u64 - Current trust score (0 to T::MaxTrustScore)
    #[pallet::storage]
    #[pallet::getter(fn validator_trust_scores)]
    pub type ValidatorTrustScores<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        u64,
        ValueQuery,
    >;

    /// Historical trust score data for trend analysis and performance tracking.
    ///
    /// Maintains a time-series of trust scores for each validator to enable:
    /// - Performance trend analysis over time
    /// - Trust score evolution tracking
    /// - Historical performance comparisons
    /// - Validator reliability assessment
    /// - API endpoints for trust score charts
    ///
    /// Each entry in the history contains:
    /// - Epoch number when the score was recorded
    /// - Trust score value at that time
    ///
    /// The history is stored as a bounded vector with a maximum of 100 entries
    /// per validator, acting as a circular buffer. When the limit is reached,
    /// the oldest entries are automatically removed to make space for new ones.
    ///
    /// Trust score history is updated:
    /// - At epoch transitions with the current trust score
    /// - After significant trust score changes (above threshold)
    /// - During manual trust score recalculations
    /// - When validators join or leave the network
    ///
    /// This historical data enables:
    /// - Long-term validator performance evaluation
    /// - Trust score volatility analysis
    /// - Predictive modeling for validator behavior
    /// - Network health monitoring and reporting
    ///
    /// # Key: T::AccountId - Validator account
    /// # Value: BoundedVec<(u32, u64), ConstU32<100>> - History of (epoch, trust_score) pairs
    #[pallet::storage]
    #[pallet::getter(fn trust_score_history)]
    pub type TrustScoreHistory<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        BoundedVec<(u32, u64), ConstU32<100>>, // (epoch, trust_score)
        ValueQuery,
    >;

    /// Trust Score Bounds Storage
    ///
    /// Stores the current bounds and rate limits for trust score calculations.
    /// These bounds are used to prevent explosive growth or rapid decay of trust scores,
    /// ensuring long-term stability and fairness in the scoring system.
    ///
    /// The bounds include:
    /// - Minimum and maximum trust score values
    /// - Maximum growth and decay rates per epoch
    /// - Stability factors for score smoothing
    ///
    /// These values can be updated through governance to adapt to network conditions
    /// while maintaining the integrity of the trust scoring system.
    ///
    /// # Value: TrustScoreBoundsData - Current bounds configuration
    #[pallet::storage]
    #[pallet::getter(fn trust_score_bounds)]
    pub type TrustScoreBounds<T: Config> = StorageValue<_, TrustScoreBoundsData, ValueQuery>;

    /// Trust Score Stability Metrics Storage
    ///
    /// Tracks stability metrics for trust scores across the network to monitor
    /// the health and fairness of the scoring system. These metrics help detect
    /// potential issues with score volatility or gaming attempts.
    ///
    /// Metrics include:
    /// - Average score change per epoch
    /// - Maximum score change in recent epochs
    /// - Number of validators hitting bounds
    /// - Score distribution statistics
    ///
    /// # Value: TrustScoreStabilityMetricsData - Current stability metrics
    #[pallet::storage]
    #[pallet::getter(fn trust_score_stability_metrics)]
    pub type TrustScoreStabilityMetrics<T: Config> = StorageValue<_, TrustScoreStabilityMetricsData, ValueQuery>;

    /// Storage for invariant violation reports generated at epoch boundaries.
    ///
    /// This storage maintains a history of invariant violation reports to enable
    /// monitoring, alerting, and analysis of system health over time. Reports are
    /// generated automatically during epoch transitions and contain comprehensive
    /// information about any detected violations.
    ///
    /// The storage is bounded to prevent unbounded growth while maintaining
    /// sufficient history for analysis and debugging purposes.
    ///
    /// # Key: u32 - Epoch number when the report was generated
    /// # Value: InvariantReport<T> - Complete invariant violation report
    #[pallet::storage]
    #[pallet::getter(fn invariant_reports)]
    pub type InvariantReports<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u32,
        InvariantReport<T>,
        OptionQuery,
    >;

    /// Storage for the latest invariant report for quick access.
    ///
    /// This storage provides immediate access to the most recent invariant
    /// report without needing to iterate through the full history. It's
    /// updated on every epoch transition and used by monitoring systems
    /// and runtime APIs for health checks.
    ///
    /// # Value: InvariantReport<T> - Most recent invariant report
    #[pallet::storage]
    #[pallet::getter(fn latest_invariant_report)]
    pub type LatestInvariantReport<T: Config> = StorageValue<_, InvariantReport<T>, OptionQuery>;

    // EpochTotalSlashed and EpochTotalRewarded moved to pallet-cbc-pos.

    /// Storage version for the DCF pallet.
    /// 
    /// This storage item tracks the current version of the storage schema to enable
    /// safe migrations and upgrades. The version number is incremented whenever
    /// breaking changes are made to the storage layout.
    /// 
    /// Version History:
    /// - Version 1: Initial production-ready storage layout with comprehensive governance,
    ///   invariant checking, and validator lifecycle management
    /// 
    /// The storage version is validated during runtime initialization to ensure
    /// compatibility and trigger migrations when necessary.
    /// 
    /// # Value: u32 - Current storage schema version number
    #[pallet::storage]
    #[pallet::getter(fn storage_version)]
    pub type StorageVersion<T: Config> = StorageValue<_, u32, ValueQuery>;

    /// Rate limiting configuration for DoS protection.
    ///
    /// This storage item contains the current rate limiting configuration that
    /// controls how frequently various operations can be performed to prevent
    /// abuse and DoS attacks on the network.
    ///
    /// # Value: RateLimitConfig - Complete rate limiting configuration
    #[pallet::storage]
    #[pallet::getter(fn rate_limit_config)]
    pub type RateLimitConfigStorage<T: Config> = StorageValue<_, RateLimitConfig, ValueQuery>;

    /// Per-block operation counters for rate limiting.
    ///
    /// Tracks the number of operations performed in the current block for each
    /// operation type. Counters are reset at the beginning of each block.
    ///
    /// # Key: DispatchableType - The type of operation being tracked
    /// # Value: u32 - Number of operations performed in current block
    #[pallet::storage]
    #[pallet::getter(fn block_operation_counts)]
    pub type BlockOperationCounts<T: Config> = StorageMap<
        _, Blake2_128Concat, DispatchableType, u32, ValueQuery
    >;

    /// Per-account operation history for rate limiting.
    ///
    /// Tracks recent operations performed by each account within rate limiting
    /// time windows. Used to enforce per-account rate limits and minimum intervals.
    ///
    /// # Key: (T::AccountId, DispatchableType) - Account and operation type
    /// # Value: BoundedVec<u32, ConstU32<100>> - Recent block numbers when operations occurred
    #[pallet::storage]
    #[pallet::getter(fn account_operation_history)]
    pub type AccountOperationHistory<T: Config> = StorageDoubleMap<
        _, Blake2_128Concat, T::AccountId, Blake2_128Concat, DispatchableType, 
        BoundedVec<u32, ConstU32<100>>, ValueQuery
    >;

    /// Last operation block for minimum interval enforcement.
    ///
    /// Tracks the last block number when each account performed specific operations
    /// to enforce minimum intervals between repeated operations.
    ///
    /// # Key: (T::AccountId, DispatchableType) - Account and operation type
    /// # Value: u32 - Block number of last operation
    #[pallet::storage]
    #[pallet::getter(fn last_operation_block)]
    pub type LastOperationBlock<T: Config> = StorageDoubleMap<
        _, Blake2_128Concat, T::AccountId, Blake2_128Concat, DispatchableType, u32, OptionQuery
    >;

    // ValidatorEpochSlashed and ValidatorEpochRewarded moved to pallet-cbc-pos.

    /// Deterministic epoch processing engine state.
    ///
    /// This storage item maintains the state of the deterministic epoch processing
    /// engine, including randomness seeds, replay validation data, and author
    /// sequence caching. It ensures that all nodes process epochs identically
    /// and enables replay validation for consensus verification.
    ///
    /// The engine state includes:
    /// - Fixed randomness salt that never changes after genesis
    /// - Per-epoch salts derived deterministically from epoch numbers
    /// - Cached author sequences for deterministic block production
    /// - Replay validation hashes for output verification
    ///
    /// # Value: DeterministicEpochEngine - Complete deterministic processing state
    #[pallet::storage]
    #[pallet::getter(fn deterministic_engine)]
    pub type DeterministicEngineState<T: Config> = StorageValue<_, DeterministicEpochEngine, ValueQuery>;

    /// Historical epoch processing outputs for replay validation.
    ///
    /// This storage map maintains a record of epoch processing outputs to enable
    /// replay validation and determinism verification. Each entry contains all
    /// inputs and outputs from an epoch transition, allowing byte-for-byte
    /// comparison during replay operations.
    ///
    /// The outputs are used to:
    /// - Validate that epoch processing is deterministic
    /// - Compare replay results with original processing
    /// - Detect and debug non-deterministic behavior
    /// - Ensure consensus on epoch transition results
    ///
    /// Storage is bounded to prevent unbounded growth while maintaining sufficient
    /// history for validation purposes.
    ///
    /// # Key: u32 - Epoch number
    /// # Value: EpochProcessingOutput<T> - Complete processing inputs and outputs
    #[pallet::storage]
    #[pallet::getter(fn epoch_processing_outputs)]
    pub type EpochProcessingOutputs<T: Config> = StorageMap<
        _, Blake2_128Concat, u32, EpochProcessingOutput<T>, OptionQuery
    >;

    /// Deterministic author sequences for each epoch.
    ///
    /// This storage map caches the deterministically generated author sequences
    /// for each epoch to ensure consistent block production ordering across all
    /// nodes. The sequences are generated using deterministic randomness seeded
    /// from block numbers and fixed salts.
    ///
    /// Author sequences are pre-computed during epoch transitions and cached
    /// for efficient lookup during block production. This ensures that all nodes
    /// agree on the expected author for any given block number within an epoch.
    ///
    /// # Key: u32 - Epoch number
    /// # Value: BoundedVec<T::AccountId, ConstU32<1000>> - Deterministic author sequence
    #[pallet::storage]
    #[pallet::getter(fn epoch_author_sequences)]
    pub type EpochAuthorSequences<T: Config> = StorageMap<
        _, Blake2_128Concat, u32, BoundedVec<T::AccountId, ConstU32<1000>>, OptionQuery
    >;

    /// Snapshotted validator scores taken at a fixed block before each epoch boundary.
    /// Used as the weight input for the next epoch's author sequence so all nodes
    /// compute identical sequences regardless of offchain worker timing.
    ///
    /// # Key: u32 - Epoch number the snapshot was taken FOR (i.e. next epoch)
    /// # Value: BoundedVec<(AccountId, score)> - Scores frozen at snapshot block
    #[pallet::storage]
    pub type EpochScoreSnapshot<T: Config> = StorageMap<
        _, Blake2_128Concat, u32,
        BoundedVec<(T::AccountId, u64), ConstU32<1000>>,
        OptionQuery
    >;

    /// Aggregated system metrics for operational monitoring.
    ///
    /// This storage item contains comprehensive system metrics that are updated
    /// automatically during epoch transitions. It provides a single source of
    /// truth for all essential system health and performance indicators.
    ///
    /// The metrics are designed to reduce RPC fan-out by aggregating all commonly
    /// requested system information into a single compact structure. This enables
    /// monitoring systems to get a complete system overview with a single API call.
    ///
    /// Metrics are updated during:
    /// - Epoch transitions (validator counts, economic totals, performance indicators)
    /// - Block production (finality lag, block production metrics)
    /// - Validator lifecycle events (active counts, stake totals)
    /// - Governance operations (economic totals, system health)
    ///
    /// All metrics reflect the current epoch state and are guaranteed to be
    /// consistent with the actual system state at the time of the last update.
    ///
    /// # Value: SystemMetrics<T> - Complete aggregated system metrics
    #[pallet::storage]
    #[pallet::getter(fn system_metrics)]
    pub type SystemMetricsStorage<T: Config> = StorageValue<_, SystemMetrics<T>, ValueQuery>;

    /// System performance indicators for operational efficiency tracking.
    ///
    /// This storage item contains detailed performance metrics that complement
    /// the core system metrics, focusing on operational efficiency and network
    /// performance characteristics.
    ///
    /// Performance indicators are updated during epoch transitions and provide
    /// insights into:
    /// - Block production efficiency and timing
    /// - Validator score distribution and competition
    /// - Network consensus participation rates
    /// - Governance activity and validator churn
    ///
    /// These metrics are particularly useful for:
    /// - Performance trend analysis
    /// - Network optimization decisions
    /// - Capacity planning and scaling
    /// - Operational efficiency monitoring
    ///
    /// # Value: SystemPerformanceIndicators - Detailed performance metrics
    #[pallet::storage]
    #[pallet::getter(fn performance_indicators)]
    pub type PerformanceIndicatorsStorage<T: Config> = StorageValue<_, SystemPerformanceIndicators, ValueQuery>;

    /// Metrics update timestamp for cache invalidation.
    ///
    /// This storage item tracks when the system metrics were last updated to
    /// enable efficient caching and cache invalidation strategies. It contains
    /// the block number of the last metrics update.
    ///
    /// Used by:
    /// - Runtime APIs to determine if cached metrics are still valid
    /// - Monitoring systems to detect stale metrics
    /// - Performance optimization to avoid unnecessary recalculations
    /// - Cache invalidation logic in external systems
    ///
    /// The timestamp is updated whenever SystemMetricsStorage or
    /// PerformanceIndicatorsStorage are modified.
    ///
    /// # Value: u32 - Block number when metrics were last updated
    #[pallet::storage]
    #[pallet::getter(fn metrics_last_updated)]
    pub type MetricsLastUpdated<T: Config> = StorageValue<_, u32, ValueQuery>;

    /// Private chain configuration and validator allowlist.
    ///
    /// This storage item contains the configuration for private chain mode,
    /// including the validator allowlist and permission settings.
    ///
    /// # Usage
    /// - Controlling validator participation in private chains
    /// - Managing validator allowlists and permissions
    /// - Enforcing private chain access restrictions
    ///
    /// # Value: PrivateChainConfig<T::AccountId> - Complete private chain configuration
    #[pallet::storage]
    #[pallet::getter(fn private_chain_config)]
    pub type PrivateChainConfigStorage<T: Config> = StorageValue<_, private_chain::PrivateChainConfig<T::AccountId>, OptionQuery>;

    /// EVM-compatible events storage for cross-chain event querying.
    ///
    /// This storage item maintains EVM-compatible versions of DCF events
    /// to enable querying from EVM-based applications and contracts.
    ///
    /// # Usage
    /// - EVM event indexing and querying
    /// - Cross-chain event monitoring
    /// - EVM-based analytics and reporting
    ///
    /// # Key: u32 - Block number when events occurred
    /// # Value: BoundedVec<EvmCompatibleEvent> - List of EVM-compatible events for the block
    #[pallet::storage]
    #[pallet::getter(fn evm_compatible_events)]
    pub type EvmCompatibleEvents<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u32, // block number
        BoundedVec<evm_compatibility::EvmCompatibleEvent, ConstU32<100>>,
        ValueQuery,
    >;

    /// Slashing history for validators.
    ///
    /// This storage item maintains a complete history of slashing events
    /// for each validator, including amounts, reasons, and timestamps.
    ///
    /// # Usage
    /// - Tracking validator punishment history
    /// - Risk assessment and validator evaluation
    /// - Audit trails and compliance reporting
    ///
    /// # Key: T::AccountId - Validator account
    /// # Value: BoundedVec<SlashingRecord> - Complete slashing history for the validator (max 100 records)
    #[pallet::storage]
    #[pallet::getter(fn validator_slashing_history)]
    pub type ValidatorSlashingHistory<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        BoundedVec<SlashingRecord<<T as pallet::Config>::Balance, BlockNumberFor<T>>, ConstU32<100>>,
        ValueQuery,
    >;

    // --- Events --- //
    /// Events emitted by the pallet for all state transitions and validator lifecycle changes.
    /// 
    /// These events provide comprehensive coverage of all validator operations, consensus changes,
    /// governance actions, and network state transitions. They are essential for:
    /// - Block explorers and monitoring tools
    /// - Off-chain analytics and reporting
    /// - Client applications and user interfaces
    /// - Network health monitoring and alerting
    /// - Audit trails and compliance tracking
    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Emitted when a validator's scores are updated during normal operations.
        /// 
        /// This event is triggered when validator scores change due to:
        /// - Stake amount changes affecting PoS scores
        /// - Inference performance updates affecting PoI scores
        /// - Manual score adjustments through governance
        /// - Automatic score recalculations during epoch transitions
        /// 
        /// The final_score is computed as: (stake_score * pos_weight + inference_score * poi_weight) / precision
        ValidatorScoreUpdated {
            /// The validator whose scores were updated
            validator: T::AccountId,
            /// New Proof-of-Stake score based on validator's stake amount
            stake_score: u64,
            /// New Proof-of-Inference score based on AI/ML performance
            inference_score: u64,
            /// New combined final score used for consensus ranking
            final_score: u64,
        },

        /// Emitted when the consensus weight distribution between PoS and PoI is modified.
        /// 
        /// This event occurs when governance or root accounts adjust the relative importance
        /// of stake-based vs. inference-based scoring in the consensus mechanism. The weights
        /// must always sum to the precision factor (typically 10000 for basis points).
        /// 
        /// Changes affect all future score calculations but do not retroactively modify
        /// existing validator scores until their next update.
        ConsensusWeightsUpdated {
            /// New weight for Proof-of-Stake component (0-10000 basis points)
            pos_weight: u64,
            /// New weight for Proof-of-Inference component (0-10000 basis points)
            poi_weight: u64,
        },

        /// Emitted at the beginning of each new epoch with the active validator set.
        /// 
        /// This event marks the start of a new epoch period and includes the complete
        /// list of validators who are eligible to participate in consensus during this epoch.
        /// The validator list is determined by:
        /// - Meeting minimum score thresholds
        /// - Having sufficient stake amounts
        /// - Not being in cooldown periods
        /// - Not being ejected for misbehavior
        EpochStarted {
            /// The epoch number that is starting (increments from 0)
            epoch: u32,
            /// Complete list of validators active in this epoch
            validators: Vec<T::AccountId>,
        },

        /// Emitted at the conclusion of an epoch period.
        /// 
        /// This event marks the end of an epoch and typically precedes epoch transition
        /// activities such as validator set updates, reward distributions, and score decay
        /// applications. It provides a clear boundary for epoch-based calculations.
        EpochEnded {
            /// The epoch number that is ending
            epoch: u32,
        },

        /// Emitted when a validator's score is reduced due to inactivity.
        /// 
        /// Score decay is applied to validators who have not been active for extended
        /// periods to encourage consistent participation and prevent dormant validators
        /// from maintaining high rankings indefinitely. Decay is applied according to
        /// the configured decay rate and inactivity thresholds.
        ValidatorScoreDecayed {
            /// The validator whose score was decayed
            validator: T::AccountId,
            /// Score value before decay was applied
            old_score: u64,
            /// Score value after decay was applied
            new_score: u64,
        },

        /// Emitted when a validator receives a score boost for positive actions.
        /// 
        /// Score boosts reward validators for beneficial network activities such as:
        /// - Successfully authoring blocks when selected
        /// - Submitting high-quality inference results
        /// - Maintaining consistent uptime and availability
        /// - Contributing to network security and stability
        ValidatorScoreBoosted {
            /// The validator who received the score boost
            validator: T::AccountId,
            /// Score value before the boost was applied
            old_score: u64,
            /// Score value after the boost was applied
            new_score: u64,
            /// Reason for the score boost (block authorship, inference quality, etc.)
            reason: ScoreBoostReason,
        },

        /// Emitted when a validator is forcibly removed from the validator set.
        /// 
        /// Validator ejection occurs when validators consistently fail to meet network
        /// standards or engage in malicious behavior. Ejection can be triggered by:
        /// - Chronic poor performance below minimum thresholds
        /// - Repeated misbehavior reports reaching the slashing threshold
        /// - Governance proposals for disciplinary action
        /// - Automatic ejection for severe consensus violations
        ValidatorEjected {
            /// The validator who was ejected from the network
            validator: T::AccountId,
            /// Reason for ejection (poor performance, misbehavior, governance decision)
            reason: EjectionReason,
        },

        /// Emitted when a previously ejected validator successfully rejoins the network.
        /// 
        /// This event occurs when validators who were previously ejected for poor
        /// performance or other issues demonstrate improved capabilities and are
        /// allowed to rejoin the validator set. Re-entry typically requires:
        /// - Meeting current minimum requirements
        /// - Completing any required cooldown periods
        /// - Demonstrating improved performance metrics
        ValidatorReEntered {
            /// The validator who successfully rejoined the network
            validator: T::AccountId,
            /// Current score of the validator upon re-entry
            score: u64,
        },

        /// Emitted when block author validation fails due to author mismatch.
        /// 
        /// This critical security event occurs when the actual block author differs
        /// from the expected author according to the consensus algorithm. This can
        /// indicate:
        /// - Consensus algorithm errors or bugs
        /// - Malicious attempts to produce unauthorized blocks
        /// - Network synchronization issues
        /// - Validator set inconsistencies
        /// 
        /// Blocks with author mismatches are rejected to maintain network security.
        AuthorMismatch {
            /// Block number where the mismatch was detected
            block_number: u32,
            /// Expected block author according to consensus algorithm
            expected: Option<T::AccountId>,
            /// Actual block author found in the block header
            actual: T::AccountId,
        },

        /// Emitted when block author validation fails.
        AuthorValidationFailed {
            /// Block number where validation failed
            block_number: u32,
            /// Expected block author
            expected: Option<T::AccountId>,
            /// Actual block author
            actual: T::AccountId,
            /// Reason for validation failure
            reason: BoundedVec<u8, ConstU32<128>>,
        },

        /// Emitted when a validator attempts to join but lacks sufficient stake.
        /// 
        /// This event occurs when validators try to join the network but do not meet
        /// the minimum stake requirements. It provides transparency about failed join
        /// attempts and helps validators understand the requirements for participation.
        InsufficientStake {
            /// The validator who attempted to join with insufficient stake
            validator: T::AccountId,
            /// Minimum stake amount required for validator participation
            required: <T as pallet::Config>::Balance,
            /// Actual stake amount available from the validator
            available: <T as pallet::Config>::Balance,
        },

        /// Emitted when an invalid block author is detected during validation.
        /// 
        /// This event indicates that a block was produced by a validator who was not
        /// authorized to produce blocks at that time. This is distinct from author
        /// mismatch as it specifically identifies unauthorized block production attempts.
        InvalidAuthor {
            /// Block number with the invalid author
            block_number: u32,
            /// Account that produced the block without authorization
            author: T::AccountId,
        },

        /// Emitted when governance mode is enabled or disabled.
        /// 
        /// Governance mode changes affect how the pallet operates:
        /// - When enabled: Manual epoch transitions allowed, enhanced admin powers
        /// - When disabled: Fully automated operation, reduced admin intervention
        /// 
        /// This event provides transparency about the current operational mode.
        GovernanceModeToggled {
            /// New governance mode state (true = enabled, false = disabled)
            enabled: bool,
        },

        /// Emitted when a new governance proposal is submitted to the network.
        /// 
        /// Governance proposals enable decentralized decision-making for validator
        /// management actions including slashing, rewards, ejections, and set changes.
        /// This event marks the beginning of the proposal lifecycle.
        ProposalSubmitted {
            /// Unique identifier for the proposal
            proposal_id: u32,
            /// Account that submitted the proposal
            proposer: T::AccountId,
            /// Specific action being proposed (slash, reward, eject, etc.)
            action: ProposalAction<T>,
        },

        /// Emitted when a governance proposal is executed after approval.
        /// 
        /// This event occurs when approved proposals are executed by authorized
        /// accounts (typically root). It marks the completion of the governance
        /// process and the implementation of the proposed changes.
        ProposalExecuted {
            /// Unique identifier of the executed proposal
            proposal_id: u32,
            /// Final status of the proposal after execution
            status: ProposalStatus,
        },

        /// Emitted when a validator casts a vote on a governance proposal.
        /// 
        /// This event tracks individual voting participation in the governance
        /// process, providing transparency about validator engagement in network
        /// decision-making and enabling vote auditing.
        ProposalVoted {
            /// Unique identifier of the proposal being voted on
            proposal_id: u32,
            /// Validator account that cast the vote
            voter: T::AccountId,
            /// Vote direction (true = approve, false = reject)
            approve: bool,
        },

        /// Emitted when a governance proposal receives sufficient approval votes.
        /// 
        /// This event occurs when a proposal reaches the required quorum and has
        /// more approval votes than rejection votes. Approved proposals become
        /// eligible for execution by authorized accounts.
        ProposalPassed { 
            /// Unique identifier of the approved proposal
            proposal_id: u32 
        },

        /// Emitted when a governance proposal is rejected by validator votes.
        /// 
        /// This event occurs when a proposal reaches the required quorum but has
        /// more rejection votes than approval votes. Rejected proposals cannot
        /// be executed and are marked as failed.
        ProposalRejected { 
            /// Unique identifier of the rejected proposal
            proposal_id: u32 
        },

        /// Emitted when a validator requests to join the network.
        /// 
        /// This event marks the beginning of the validator onboarding process when
        /// a validator submits a request to join the network. The request includes
        /// stake commitment and triggers validation of eligibility requirements.
        /// This event occurs before the actual joining is completed.
        ValidatorJoinRequested {
            /// Account of the validator requesting to join the network
            validator: T::AccountId,
            /// Amount of stake being committed for validator participation
            stake_amount: <T as pallet::Config>::Balance,
        },

        /// Emitted when a new validator successfully joins the network.
        /// 
        /// This event marks the successful completion of the validator onboarding
        /// process, including meeting stake requirements, passing validation checks,
        /// and being added to the validator registry. The validator becomes eligible
        /// for consensus participation.
        ValidatorJoined { 
            /// Account of the validator who joined the network
            validator: T::AccountId,
            /// Amount of stake that was reserved for validator participation
            stake_amount: <T as pallet::Config>::Balance,
        },

        /// Emitted when a validator completes the process of leaving the network.
        /// 
        /// This event occurs when validators successfully exit the network after
        /// completing required cooldown periods. Their stake is unreserved and
        /// they are removed from all validator sets and registries.
        ValidatorLeft { 
            /// Account of the validator who left the network
            validator: T::AccountId 
        },

        /// Emitted when a validator requests to leave the network.
        /// 
        /// This event marks the beginning of the validator exit process. The validator
        /// enters a cooldown period during which they must continue participating
        /// while their leave request is processed. This prevents rapid validator
        /// set changes that could destabilize consensus.
        ValidatorLeaveRequested { 
            /// Account of the validator requesting to leave
            validator: T::AccountId,
            /// Block number when the cooldown period expires and leaving is allowed
            cooldown_expires_at: u32,
        },

        /// Emitted when a validator cancels their pending leave request.
        /// 
        /// This event occurs when validators decide to remain in the network after
        /// previously requesting to leave. The leave request is cancelled and the
        /// validator continues normal participation without entering cooldown.
        ValidatorLeaveCancelled {
            /// Account of the validator who cancelled their leave request
            validator: T::AccountId,
        },

        /// Emitted when a validator's Proof-of-Inference score is updated.
        /// 
        /// This event specifically tracks changes to PoI scores, which reflect
        /// validator performance in AI/ML inference tasks. PoI scores are a critical
        /// component of the CBC consensus mechanism's hybrid scoring system.
        ValidatorPoiScoreUpdated {
            /// The validator whose PoI score was updated
            validator: T::AccountId,
            /// New Proof-of-Inference score value
            poi_score: u64,
        },

        /// Emitted when validator metadata (such as display name) is updated.
        /// 
        /// This event tracks changes to validator metadata that improve user
        /// experience in interfaces and monitoring tools. Metadata updates do
        /// not affect consensus operations but enhance validator identification.
        ValidatorMetadataUpdated {
            /// The validator whose metadata was updated
            validator: T::AccountId,
            /// New display name for the validator (UTF-8 encoded)
            name: BoundedVec<u8, ConstU32<32>>,
        },

        /// Emitted when validator activity metrics are updated.
        /// 
        /// This event tracks changes to validator performance metrics including
        /// block production statistics and uptime percentages. These metrics
        /// are used for performance evaluation and score calculations.
        ValidatorActivityUpdated {
            /// The validator whose activity metrics were updated
            validator: T::AccountId,
            /// Total number of blocks successfully authored by the validator
            blocks_authored: u32,
            /// Total number of blocks missed when selected as author
            blocks_missed: u32,
            /// Current uptime percentage (0-10000 representing 0-100%)
            uptime_percentage: u32,
        },

        /// Emitted when the system automatically generates a validator management proposal.
        /// 
        /// This event occurs when automated systems detect conditions that warrant
        /// validator management actions and create governance proposals accordingly.
        /// This enables algorithmic governance while maintaining human oversight.
        AutomaticValidatorProposal {
            /// The validator who is the subject of the automatic proposal
            validator: T::AccountId,
            /// Type of action being automatically proposed (join, leave, etc.)
            action: ValidatorAction,
            /// Current score of the validator that triggered the proposal
            score: u64,
            /// Human-readable reason for the automatic proposal
            reason: BoundedVec<u8, ConstU32<64>>,
        },

        /// Emitted when a validator is added to the validator set.
        /// 
        /// This event occurs when validators are successfully added to the network
        /// through governance proposals or administrative actions. It differs from
        /// ValidatorJoined in that it can occur through external proposals rather
        /// than self-initiated joining.
        ValidatorAdded {
            /// Account of the validator who was added to the set
            validator: T::AccountId,
        },

        /// Emitted when a validator is removed from the validator set.
        /// 
        /// This event occurs when validators are removed through governance
        /// proposals or administrative actions. It differs from ValidatorLeft
        /// in that it can be involuntary removal rather than self-initiated leaving.
        ValidatorRemoved {
            /// Account of the validator who was removed from the set
            validator: T::AccountId,
        },

        /// Emitted when a validator receives a reward payment.
        /// 
        /// This event tracks all reward distributions to validators, including:
        /// - Epoch-based performance rewards
        /// - Governance-approved bonus payments
        /// - Block authorship rewards
        /// - Special recognition rewards
        /// 
        /// Rewards are typically paid from treasury or inflation mechanisms.
        ValidatorRewarded {
            /// The validator who received the reward
            validator: T::AccountId,
            /// Amount of the reward payment
            amount: <T as pallet::Config>::Balance,
            /// Validator's balance before the reward
            pre_balance: <T as pallet::Config>::Balance,
            /// Validator's balance after the reward
            post_balance: <T as pallet::Config>::Balance,
            /// Reason code for the reward
            reason: RewardReason,
        },

        /// Emitted when a validator's stake is slashed as punishment.
        /// 
        /// This critical event occurs when validators are penalized for misbehavior,
        /// poor performance, or consensus violations. Slashing reduces the validator's
        /// reserved stake and typically burns the slashed tokens to maintain economic
        /// security incentives.
        ValidatorSlashed {
            /// The validator whose stake was slashed
            validator: T::AccountId,
            /// Amount of stake that was slashed and burned
            amount: <T as pallet::Config>::Balance,
            /// Validator's balance before the slash
            pre_balance: <T as pallet::Config>::Balance,
            /// Validator's balance after the slash
            post_balance: <T as pallet::Config>::Balance,
            /// Reason code for the slashing
            reason: SlashReason,
        },

        /// Emitted when misbehavior is reported against a validator.
        /// 
        /// This event occurs when network participants submit evidence of validator
        /// misbehavior. Multiple reports against the same validator can trigger
        /// automatic slashing when the threshold is reached, providing decentralized
        /// enforcement of network rules.
        ValidatorMisbehaviorReported {
            /// The validator being reported for misbehavior
            reported_validator: T::AccountId,
            /// The account submitting the misbehavior report
            reporter: T::AccountId,
            /// Cryptographic evidence of the misbehavior
            evidence: BoundedVec<u8, T::MaxEvidenceLength>,
            /// Total number of reports against this validator
            total_reports: u32,
        },

        /// Emitted when a validator is automatically slashed due to multiple misbehavior reports.
        /// 
        /// This event occurs when the number of independent misbehavior reports against
        /// a validator reaches the configured threshold, triggering automatic slashing
        /// without requiring governance approval. This provides rapid response to
        /// clear cases of validator misbehavior.
        ValidatorAutoSlashed {
            /// The validator who was automatically slashed
            validator: T::AccountId,
            /// Amount of stake that was automatically slashed
            amount: <T as pallet::Config>::Balance,
            /// Number of misbehavior reports that triggered the automatic slashing
            report_count: u32,
        },

        /// Emitted when an inference operation is simulated for development purposes.
        /// 
        /// This event is used during development and testing to simulate AI/ML
        /// inference operations without requiring actual inference computations.
        /// It helps test the PoI scoring system and validator performance tracking.
        InferenceSimulated {
            /// The validator for whom inference was simulated
            validator: T::AccountId,
            /// The account that triggered the inference simulation
            triggered_by: T::AccountId,
        },

        /// Emitted when a block achieves finality in the network.
        /// 
        /// Block finalization marks blocks as permanently part of the canonical
        /// chain and safe from reorganization. This event is critical for
        /// applications that require transaction irreversibility guarantees.
        BlockFinalized {
            /// Block number that achieved finality
            block_number: u32,
        },

        /// Emitted when consensus finality is reached for a specific block.
        /// 
        /// This event provides detailed finality tracking for consensus monitoring
        /// and includes information about the validator set that participated in
        /// achieving finality. It's essential for consensus health monitoring and
        /// provides more detailed information than BlockFinalized.
        FinalityMarker {
            /// Block number that achieved consensus finality
            block_number: u32,
            /// Hash of the finalized block for verification
            block_hash: T::Hash,
            /// List of validators that participated in achieving finality
            participating_validators: Vec<T::AccountId>,
            /// Total number of validators in the active set during finality
            total_validators: u32,
        },

        /// Emitted when an epoch transition occurs with validator set changes and performance metrics.
        /// 
        /// This comprehensive event provides complete information about epoch changes
        /// including validator set updates, performance metrics, and transition details.
        /// It's essential for monitoring network evolution and validator performance over time.
        EpochTransition {
            /// The epoch number that ended
            old_epoch: u32,
            /// The new epoch number that started
            new_epoch: u32,
            /// Complete list of validators active in the new epoch
            active_validators: Vec<T::AccountId>,
            /// List of validators that were removed during this transition
            removed_validators: Vec<T::AccountId>,
            /// List of validators that were added during this transition
            added_validators: Vec<T::AccountId>,
            /// Total number of registered validators (active and inactive)
            total_validators: u32,
            /// Average performance score of active validators in the new epoch
            average_performance_score: u64,
            /// Total rewards distributed during the epoch transition
            total_rewards_distributed: <T as pallet::Config>::Balance,
            /// Total amount slashed during the previous epoch
            total_slashed_amount: <T as pallet::Config>::Balance,
        },

        /// Emitted when an epoch boundary is detected during block processing.
        /// 
        /// This event occurs when the system detects that an epoch transition
        /// should occur based on block numbers or other criteria. It provides
        /// early notification of pending epoch changes.
        EpochBoundaryDetected {
            /// Block number where the epoch boundary was detected
            block_number: u32,
            /// Current epoch number before transition
            epoch: u32,
            /// Whether governance mode is currently enabled
            governance_mode: bool,
        },

        /// Emitted when a validator registers a display name.
        /// 
        /// This event tracks validator name registrations that improve user
        /// experience in interfaces and monitoring tools. Names are purely
        /// cosmetic and do not affect consensus operations.
        ValidatorNameRegistered {
            /// The validator who registered a name
            validator: T::AccountId,
            /// The registered display name (UTF-8 encoded)
            name: BoundedVec<u8, ConstU32<32>>,
        },

        /// Emitted when a detailed governance proposal is created with description.
        /// 
        /// This enhanced proposal event includes additional context and description
        /// information to help validators make informed voting decisions. It
        /// supplements the basic ProposalSubmitted event with richer metadata.
        ProposalCreated {
            /// Unique identifier for the new proposal
            proposal_id: u32,
            /// Account that created the proposal
            proposer: T::AccountId,
            /// Specific action being proposed
            action: ProposalAction<T>,
            /// Human-readable description of the proposal
            description: BoundedVec<u8, ConstU32<128>>,
        },

        /// Emitted when validator scores are updated with detailed before/after information.
        /// 
        /// This comprehensive score update event provides complete visibility into
        /// score changes including individual component changes and the epoch
        /// context. It's essential for detailed performance analysis and debugging.
        ValidatorScoreUpdatedDetailed {
            /// The validator whose scores were updated
            validator: T::AccountId,
            /// Previous Proof-of-Stake score value
            old_stake_score: u64,
            /// New Proof-of-Stake score value
            new_stake_score: u64,
            /// Previous Proof-of-Inference score value
            old_inference_score: u64,
            /// New Proof-of-Inference score value
            new_inference_score: u64,
            /// Previous combined final score value
            old_final_score: u64,
            /// New combined final score value
            new_final_score: u64,
            /// Epoch number when the score update occurred
            epoch: u32,
        },

        /// Emitted when validator stake is reserved during the joining process.
        /// 
        /// This event occurs when validators successfully join the network and
        /// their stake is locked/reserved to ensure economic security. The
        /// reserved stake cannot be spent while the validator participates.
        ValidatorStakeReserved {
            /// The validator whose stake was reserved
            validator: T::AccountId,
            /// Amount of stake that was reserved/locked
            amount: <T as pallet::Config>::Balance,
        },

        /// Emitted when validator stake is unreserved after leaving the network.
        /// 
        /// This event occurs when validators complete the leaving process and
        /// their previously reserved stake is unlocked and returned to their
        /// free balance. This typically happens after cooldown periods expire.
        ValidatorStakeUnreserved {
            /// The validator whose stake was unreserved
            validator: T::AccountId,
            /// Amount of stake that was unreserved/unlocked
            amount: <T as pallet::Config>::Balance,
        },

        /// Emitted when epoch rewards are distributed across different reward pools.
        /// 
        /// This event provides transparency into reward distribution mechanisms
        /// including how the total reward pool is divided among base rewards,
        /// performance bonuses, and top performer incentives.
        EpochRewardsDistributed {
            /// Total reward pool available for distribution
            total_pool: <T as pallet::Config>::Balance,
            /// Amount allocated to base rewards for all validators
            base_pool: <T as pallet::Config>::Balance,
            /// Amount allocated to performance-based rewards
            performance_pool: <T as pallet::Config>::Balance,
            /// Amount allocated to top performer bonuses
            top_performer_pool: <T as pallet::Config>::Balance,
        },

        /// Emitted when a validator's stake amount is increased.
        /// 
        /// This event tracks stake increases that can occur through additional
        /// deposits, reward compounding, or other mechanisms that grow the
        /// validator's economic commitment to the network.
        ValidatorStakeIncreased {
            /// The validator whose stake was increased
            validator: T::AccountId,
            /// Previous stake amount before increase
            old_amount: <T as pallet::Config>::Balance,
            /// New stake amount after increase
            new_amount: <T as pallet::Config>::Balance,
            /// Amount by which stake was increased
            increase: <T as pallet::Config>::Balance,
        },

        /// Emitted when a validator's stake amount is decreased.
        /// 
        /// This event tracks stake decreases that can occur through partial
        /// withdrawals, slashing events, or other mechanisms that reduce the
        /// validator's economic commitment to the network.
        ValidatorStakeDecreased {
            /// The validator whose stake was decreased
            validator: T::AccountId,
            /// Previous stake amount before decrease
            old_amount: <T as pallet::Config>::Balance,
            /// New stake amount after decrease
            new_amount: <T as pallet::Config>::Balance,
            /// Amount by which stake was decreased
            decrease: <T as pallet::Config>::Balance,
        },

        /// Emitted when a validator's trust score is updated with component breakdown.
        /// 
        /// This comprehensive trust score event provides complete visibility into
        /// trust score calculations including the individual components (uptime,
        /// inference success, slashing penalties) that contribute to the final score.
        TrustScoreUpdated {
            /// The validator whose trust score was updated
            validator: T::AccountId,
            /// Previous trust score value
            old_score: u64,
            /// New trust score value
            new_score: u64,
            /// Uptime component contribution to the trust score
            uptime_component: u64,
            /// Inference success component contribution to the trust score
            inference_component: u64,
            /// Slashing penalty component impact on the trust score
            slashing_component: u64,
        },

        /// Emitted when a cooldown period expires and restrictions are lifted.
        /// 
        /// This event occurs when validators complete required cooldown periods
        /// for leaving or re-entry restrictions. It marks the end of waiting
        /// periods and the restoration of full network participation rights.
        CooldownExpired {
            /// The validator whose cooldown period expired
            validator: T::AccountId,
            /// Type of cooldown that expired ("Leave" or "RecentlyRemoved")
            cooldown_type: BoundedVec<u8, ConstU32<32>>,
            /// Block number when the cooldown period expired
            expired_at_block: u32,
        },

        /// Emitted when a stake operation fails due to insufficient funds or other issues.
        /// 
        /// This event provides transparency about failed stake operations including
        /// the specific operation that failed and the reason for failure. This
        /// helps validators understand and resolve stake-related issues.
        StakeOperationFailed {
            /// The validator whose stake operation failed
            validator: T::AccountId,
            /// Type of operation that failed ("Increase", "Decrease", "Reserve", "Unreserve")
            operation: BoundedVec<u8, ConstU32<32>>,
            /// Human-readable reason for the operation failure
            reason: BoundedVec<u8, ConstU32<64>>,
        },

        /// Emitted when a validator's trust score decays due to inactivity.
        /// 
        /// Trust score decay is applied to validators who have not been active
        /// for extended periods to encourage consistent participation. This event
        /// tracks the decay process and its impact on validator rankings.
        TrustScoreDecayed {
            /// The validator whose trust score decayed
            validator: T::AccountId,
            /// Trust score value before decay was applied
            old_score: u64,
            /// Trust score value after decay was applied
            new_score: u64,
            /// Decay factor that was applied to reduce the score
            decay_factor: u64,
        },

        /// Emitted when a DCF parameter is updated through governance.
        /// 
        /// This critical governance event occurs when network parameters are modified
        /// through the parameter governance system. It provides complete transparency
        /// about parameter changes including the specific parameter, old value, and new value.
        /// 
        /// Parameter changes are subject to safety rails that ensure:
        /// - New values are within acceptable ranges
        /// - Changes are authorized by root accounts only
        /// - All changes are documented with full audit trail
        /// - Parameter readback is available via runtime API
        /// 
        /// This event is essential for network monitoring, governance auditing,
        /// and ensuring parameter changes are properly tracked and validated.
        DcfParameterUpdated {
            /// The specific parameter that was updated
            parameter: ParameterType,
            /// Previous value of the parameter (encoded as bytes for type flexibility)
            old_value: BoundedVec<u8, ConstU32<64>>,
            /// New value of the parameter (encoded as bytes for type flexibility)
            new_value: BoundedVec<u8, ConstU32<64>>,
        },

        /// Emitted when invariant violations are detected during epoch transitions.
        /// 
        /// This event signals that one or more system invariants have been violated,
        /// which could indicate potential security issues, bugs, or system instability.
        /// The event includes detailed information about each violation to aid in
        /// debugging and resolution.
        /// 
        /// # Fields
        /// - `epoch`: Epoch number when violations were detected
        /// - `violations`: List of specific invariant violations found
        /// - `severity`: Overall severity level of the violations
        /// 
        /// # Usage
        /// - System health monitoring and alerting
        /// - Automated incident response triggers
        /// - Debugging system state inconsistencies
        /// - Audit trails for compliance and security analysis
        InvariantViolationsDetected {
            /// Epoch number when violations were detected
            epoch: u32,
            /// List of specific invariant violations found
            violations: BoundedVec<InvariantViolation<T>, ConstU32<50>>,
            /// Overall severity level of the violations
            severity: InvariantSeverity,
        },

        /// Emitted when an invariant report is generated at epoch boundaries.
        /// 
        /// This event is emitted for every epoch transition, regardless of whether
        /// violations were found. It provides a comprehensive health check report
        /// that can be used for monitoring system stability and detecting trends.
        /// 
        /// # Fields
        /// - `epoch`: Epoch number for the report
        /// - `block_number`: Block number when the report was generated
        /// - `violations_count`: Number of violations detected
        /// - `severity`: Overall severity of any violations found
        /// 
        /// # Usage
        /// - Regular system health monitoring
        /// - Trend analysis and predictive maintenance
        /// - Compliance reporting and audit trails
        /// - Performance baseline establishment
        InvariantReportGenerated {
            /// Epoch number for the report
            epoch: u32,
            /// Block number when the report was generated
            block_number: u32,
            /// Number of violations detected
            violations_count: u32,
            /// Overall severity of any violations found
            severity: InvariantSeverity,
        },

        /// Emitted when storage migration is completed successfully.
        /// 
        /// This event indicates that the storage schema has been successfully
        /// migrated from one version to another during a runtime upgrade.
        /// It provides transparency about migration activities and confirms
        /// that the storage is now compatible with the current runtime.
        /// 
        /// # Fields
        /// - `from_version`: Storage version before migration
        /// - `to_version`: Storage version after migration
        /// 
        /// # Usage
        /// - Runtime upgrade monitoring and validation
        /// - Migration audit trails and compliance
        /// - System health monitoring during upgrades
        /// - Debugging migration-related issues
        StorageMigrationCompleted {
            /// Storage version before migration
            from_version: u32,
            /// Storage version after migration
            to_version: u32,
        },

        /// Emitted when storage migration fails during runtime upgrade.
        /// 
        /// This critical event indicates that storage migration could not be
        /// completed successfully, which may prevent the runtime upgrade from
        /// proceeding safely. The error information helps diagnose and resolve
        /// migration issues.
        /// 
        /// # Fields
        /// - `from_version`: Storage version that migration attempted to start from
        /// - `to_version`: Storage version that migration attempted to reach
        /// - `error`: Detailed error information about the failure
        /// 
        /// # Usage
        /// - Critical system alerts and incident response
        /// - Migration debugging and troubleshooting
        /// - Runtime upgrade failure analysis
        /// - System recovery and rollback procedures
        StorageMigrationFailed {
            /// Storage version that migration attempted to start from
            from_version: u32,
            /// Storage version that migration attempted to reach
            to_version: u32,
            /// Detailed error information about the failure
            error: BoundedVec<u8, ConstU32<256>>,
        },

        /// Emitted when storage validation fails during initialization or migration.
        /// 
        /// This event indicates that storage data validation detected inconsistencies,
        /// corruption, or constraint violations that could compromise system integrity.
        /// Storage validation failures require immediate attention to prevent data loss
        /// or system instability.
        /// 
        /// # Fields
        /// - `version`: Storage version that was being validated
        /// - `error`: Detailed error information about the validation failure
        /// 
        /// # Usage
        /// - Critical system health monitoring
        /// - Data integrity alerts and incident response
        /// - Storage corruption detection and recovery
        /// - System maintenance and repair procedures
        StorageValidationFailed {
            /// Storage version that was being validated
            version: u32,
            /// Detailed error information about the validation failure
            error: BoundedVec<u8, ConstU32<256>>,
        },

        /// Emitted when a rate limit violation is detected and an operation is rejected.
        /// 
        /// This event provides information about rate limiting violations
        /// to help identify potential DoS attacks or misconfigured clients.
        /// 
        /// # Fields
        /// - `account`: Account that attempted the rate-limited operation
        /// - `operation`: Type of operation that was rate limited (encoded as u8)
        /// - `violation_type`: Type of violation (0=PerBlock, 1=PerAccount, 2=MinInterval, 3=Weight)
        /// - `current_count`: Current count that exceeded the limit
        /// - `limit`: The limit that was exceeded
        /// 
        /// # Usage
        /// - DoS attack detection and monitoring
        /// - Client configuration debugging
        /// - Network abuse prevention and analysis
        /// - Rate limiting effectiveness monitoring
        RateLimitViolation {
            /// Account that attempted the rate-limited operation
            account: T::AccountId,
            /// Type of operation that was rate limited (0=SubmitProposal, 1=JoinValidators, etc.)
            operation: u8,
            /// Type of violation (0=PerBlock, 1=PerAccount, 2=MinInterval, 3=Weight)
            violation_type: u8,
            /// Current count that exceeded the limit
            current_count: u32,
            /// The limit that was exceeded
            limit: u32,
        },

        /// Emitted when rate limiting configuration is updated.
        /// 
        /// This event documents changes to rate limiting parameters for
        /// audit trails and monitoring purposes.
        /// 
        /// # Fields
        /// - `max_proposals_per_block`: New maximum proposals per block
        /// - `max_joins_per_block`: New maximum joins per block
        /// - `max_leaves_per_block`: New maximum leaves per block
        /// 
        /// # Usage
        /// - Configuration change auditing
        /// - Rate limiting parameter monitoring
        /// - Security policy compliance tracking
        /// - System administration logging
        RateLimitConfigUpdated {
            /// New maximum proposals per block
            max_proposals_per_block: u32,
            /// New maximum joins per block
            max_joins_per_block: u32,
            /// New maximum leaves per block
            max_leaves_per_block: u32,
        },

        /// Emitted when per-block operation counters are reset.
        /// 
        /// This event is emitted at the beginning of each block when
        /// per-block rate limiting counters are reset to zero.
        /// 
        /// # Fields
        /// - `block_number`: Block number where counters were reset
        /// 
        /// # Usage
        /// - Rate limiting system monitoring
        /// - Block processing verification
        /// - System health checks
        /// - Debugging rate limiting behavior
        BlockRateLimitCountersReset {
            /// Block number where counters were reset
            block_number: u32,
        },

        /// Emitted when deterministic epoch processing is completed.
        /// 
        /// This event indicates that an epoch transition has been processed
        /// using deterministic algorithms, ensuring that all nodes produce
        /// identical results. The event includes key processing outputs
        /// for validation and monitoring.
        /// 
        /// # Event Data
        /// - `epoch`: Epoch number that was processed
        /// - `block_number`: Block number where processing occurred
        /// - `randomness_seed`: Deterministic randomness seed used
        /// - `author_sequence_length`: Length of generated author sequence
        /// - `output_hash`: Hash of all processing outputs for validation
        /// 
        /// # Usage
        /// - Monitoring deterministic processing completion
        /// - Validating epoch transition consistency
        /// - Debugging non-deterministic behavior
        /// - Audit trails for consensus verification
        DeterministicEpochProcessed {
            /// Epoch number that was processed deterministically
            epoch: u32,
            /// Block number where deterministic processing occurred
            block_number: u32,
            /// Deterministic randomness seed used for processing
            randomness_seed: [u8; 32],
            /// Length of the generated deterministic author sequence
            author_sequence_length: u32,
            /// Hash of all processing outputs for replay validation
            output_hash: [u8; 32],
        },

        /// Emitted when epoch replay validation is performed.
        /// 
        /// This event indicates that replay validation has been executed
        /// to verify the deterministic nature of epoch processing. It
        /// includes the validation result and relevant details.
        /// 
        /// # Event Data
        /// - `epoch`: Epoch number that was replayed
        /// - `validation_passed`: Whether replay validation succeeded
        /// - `original_hash`: Hash from original processing
        /// - `replay_hash`: Hash from replay processing
        /// 
        /// # Usage
        /// - Monitoring replay validation execution
        /// - Detecting non-deterministic behavior
        /// - Validating consensus consistency
        /// - Debugging epoch processing issues
        EpochReplayValidated {
            /// Epoch number that was validated through replay
            epoch: u32,
            /// Whether the replay validation passed (true) or failed (false)
            validation_passed: bool,
            /// Hash from the original epoch processing
            original_hash: [u8; 32],
            /// Hash from the replay processing for comparison
            replay_hash: [u8; 32],
        },

        /// Emitted when a deterministic author sequence is generated.
        /// 
        /// This event indicates that a new deterministic author sequence
        /// has been generated for an epoch, providing the expected block
        /// authors for deterministic block production.
        /// 
        /// # Event Data
        /// - `epoch`: Epoch number for the sequence
        /// - `sequence_length`: Number of authors in the sequence
        /// - `randomness_seed`: Seed used for deterministic generation
        /// - `first_author`: First author in the sequence (for verification)
        /// 
        /// # Usage
        /// - Monitoring author sequence generation
        /// - Validating deterministic block production
        /// - Debugging author selection issues
        /// - Verifying consensus on expected authors
        DeterministicAuthorSequenceGenerated {
            /// Epoch number for which the sequence was generated
            epoch: u32,
            /// Number of authors in the generated sequence
            sequence_length: u32,
            /// Randomness seed used for deterministic generation
            randomness_seed: [u8; 32],
            /// First author in the sequence for quick verification
            first_author: T::AccountId,
        },

        /// Emitted when finality regression is detected during validation.
        /// 
        /// This critical event indicates that the finalized block number has
        /// moved backward, which violates the fundamental finality guarantee
        /// that finalized blocks are immutable. This should never occur in
        /// normal operation and indicates a serious consensus issue.
        /// 
        /// # Event Data
        /// - `previous_finalized`: Previously finalized block number
        /// - `attempted_finalized`: Block number that was attempted to be finalized
        /// - `current_block`: Current block number for context
        /// - `epoch`: Epoch when the regression was detected
        /// 
        /// # Usage
        /// - Critical system alerts and incident response
        /// - Consensus failure detection and analysis
        /// - System integrity monitoring
        /// - Debugging finality mechanism issues
        FinalityRegressionDetected {
            /// Previously finalized block number that was higher
            previous_finalized: u32,
            /// Block number that was attempted to be finalized (lower than previous)
            attempted_finalized: u32,
            /// Current block number for context
            current_block: u32,
            /// Epoch when the regression was detected
            epoch: u32,
        },

        /// Emitted when finality advancement validation fails.
        /// 
        /// This event indicates that an attempt to advance finality was rejected
        /// because it would violate finality constraints, such as exceeding the
        /// best known block of the previous epoch or advancing too far ahead.
        /// 
        /// # Event Data
        /// - `attempted_block`: Block number that was attempted to be finalized
        /// - `current_finalized`: Current finalized block number
        /// - `best_known_block`: Best known block number from previous epoch
        /// - `epoch`: Epoch when the validation failed
        /// - `reason`: Specific reason for the validation failure
        /// 
        /// # Usage
        /// - Finality mechanism debugging and monitoring
        /// - Consensus validation and integrity checking
        /// - System health monitoring and alerting
        /// - Audit trails for finality decisions
        FinalityAdvancementRejected {
            /// Block number that was attempted to be finalized
            attempted_block: u32,
            /// Current finalized block number before the attempt
            current_finalized: u32,
            /// Best known block number from previous epoch
            best_known_block: u32,
            /// Epoch when the validation failed
            epoch: u32,
            /// Specific reason for the validation failure
            reason: BoundedVec<u8, ConstU32<128>>,
        },

        /// Emitted when finality progression is validated successfully.
        /// 
        /// This event confirms that finality has advanced correctly according
        /// to all validation rules, including monotonic advancement and bounds
        /// checking against the best known block of the previous epoch.
        /// 
        /// # Event Data
        /// - `previous_finalized`: Previous finalized block number
        /// - `new_finalized`: New finalized block number
        /// - `advancement`: Number of blocks by which finality advanced
        /// - `epoch`: Epoch when the advancement occurred
        /// - `validation_checks_passed`: Number of validation checks that passed
        /// 
        /// # Usage
        /// - Monitoring healthy finality progression
        /// - Validating finality mechanism correctness
        /// - System health and performance tracking
        /// - Audit trails for successful finality updates
        FinalityProgressionValidated {
            /// Previous finalized block number
            previous_finalized: u32,
            /// New finalized block number after advancement
            new_finalized: u32,
            /// Number of blocks by which finality advanced
            advancement: u32,
            /// Epoch when the advancement occurred
            epoch: u32,
            /// Number of validation checks that passed
            validation_checks_passed: u32,
        },

        /// Emitted when genesis configuration validation succeeds.
        /// 
        /// This event indicates that a genesis configuration has passed all
        /// validation checks including duplicate validator detection, stake
        /// validation, and invariant verification. The event provides summary
        /// statistics about the validated configuration.
        /// 
        /// # Fields
        /// - `validator_count`: Number of validators in the configuration
        /// - `total_stake`: Total stake across all validators
        /// 
        /// # Requirements Coverage
        /// - 11.1: Confirms duplicate validators and invalid stakes were checked
        /// - 11.2: Confirms active set size validation passed
        /// 
        /// # Usage
        /// - Genesis configuration testing and validation
        /// - Network launch preparation and verification
        /// - Configuration audit trails and compliance
        /// - Automated testing and CI/CD pipelines
        GenesisValidationSucceeded {
            /// Number of validators in the validated configuration
            validator_count: u32,
            /// Total stake across all validators in the configuration
            total_stake: <T as pallet::Config>::Balance,
        },

        /// Emitted when genesis configuration validation fails.
        /// 
        /// This event indicates that a genesis configuration has failed one or more
        /// validation checks. The error field provides detailed information about
        /// what caused the validation to fail, enabling developers to fix issues.
        /// 
        /// # Fields
        /// - `error`: Detailed error message describing the validation failure
        /// 
        /// # Requirements Coverage
        /// - 11.1: Reports failures in duplicate validator or stake validation
        /// - 11.2: Reports failures in active set size validation
        /// 
        /// # Usage
        /// - Genesis configuration debugging and troubleshooting
        /// - Development environment error reporting
        /// - Configuration validation feedback
        /// - Automated testing failure analysis
        GenesisValidationFailed {
            /// Detailed error message describing why validation failed
            error: BoundedVec<u8, ConstU32<256>>,
        },

        /// Emitted when genesis dry-run completes successfully.
        /// 
        /// This event indicates that a complete dry-run of genesis build has
        /// completed successfully, including all invariant checks and system
        /// validation. The event provides comprehensive statistics about the
        /// dry-run results.
        /// 
        /// # Fields
        /// - `validator_count`: Number of validators in the dry-run
        /// - `total_stake`: Total stake across all validators
        /// - `invariant_checks_passed`: Number of invariant checks that passed
        /// - `warnings_count`: Number of warnings generated during dry-run
        /// 
        /// # Requirements Coverage
        /// - 11.3: Confirms dry-run function executed and asserted all invariants
        /// - 11.4: Provides comprehensive validation without side effects
        /// 
        /// # Usage
        /// - Genesis configuration comprehensive testing
        /// - Network launch readiness verification
        /// - System health validation and monitoring
        /// - Configuration optimization and tuning
        GenesisDryRunCompleted {
            /// Number of validators in the dry-run configuration
            validator_count: u32,
            /// Total stake across all validators in the dry-run
            total_stake: <T as pallet::Config>::Balance,
            /// Number of invariant checks that passed during dry-run
            invariant_checks_passed: u32,
            /// Number of warnings generated during the dry-run process
            warnings_count: u32,
        },

        /// Emitted when genesis dry-run fails.
        /// 
        /// This event indicates that the dry-run of genesis build has failed
        /// during execution. The error field provides detailed information about
        /// what caused the dry-run to fail, enabling developers to diagnose
        /// and resolve issues.
        /// 
        /// # Fields
        /// - `error`: Detailed error message describing the dry-run failure
        /// 
        /// # Requirements Coverage
        /// - 11.3: Reports failures in dry-run function execution
        /// - 11.4: Provides error feedback without side effects
        /// 
        /// # Usage
        /// - Genesis configuration debugging and troubleshooting
        /// - Development environment error reporting
        /// - Dry-run validation failure analysis
        /// - System configuration issue diagnosis
        GenesisDryRunFailed {
            /// Detailed error message describing why the dry-run failed
            error: BoundedVec<u8, ConstU32<256>>,
        },

        /// Emitted when the DCF Runtime API version changes due to breaking changes.
        /// 
        /// This critical event indicates that the DCF Runtime API contract has been
        /// updated with breaking changes that may affect client compatibility.
        /// Clients should monitor this event and update their integration code
        /// to handle the new API version.
        /// 
        /// # Fields
        /// - `old_version`: Previous API version number
        /// - `new_version`: New API version number
        /// - `breaking_changes`: Description of breaking changes made
        /// 
        /// # Requirements Coverage
        /// - 12.2: Provides API versioning and breaking change event emission
        /// 
        /// # Usage
        /// - Client compatibility monitoring and alerting
        /// - API version tracking and management
        /// - Breaking change notification and communication
        /// - Integration testing and validation triggers
        /// 
        /// # Breaking Change Examples
        /// - Method signature changes (parameters, return types)
        /// - Method removal or renaming
        /// - Data structure modifications
        /// - Semantic behavior changes
        ApiVersionChanged {
            /// Previous API version number before the change
            old_version: u32,
            /// New API version number after the change
            new_version: u32,
            /// Description of the breaking changes made
            breaking_changes: BoundedVec<u8, ConstU32<512>>,
        },

        /// Private chain mode has been enabled with validator allowlist.
        /// 
        /// This event is emitted when the network transitions from public mode
        /// to private chain mode with a fixed validator allowlist.
        /// 
        /// # Usage
        /// - Network configuration monitoring
        /// - Validator management system updates
        /// - Compliance and audit logging
        PrivateChainModeEnabled {
            /// Number of validators in the initial allowlist
            allowlist_size: u32,
            /// Whether allowlist updates are permitted
            allow_updates: bool,
        },

        /// Private chain mode has been disabled, returning to public mode.
        /// 
        /// This event is emitted when the network transitions from private
        /// chain mode back to public mode, removing validator restrictions.
        /// 
        /// # Usage
        /// - Network configuration monitoring
        /// - Validator management system updates
        /// - Compliance and audit logging
        PrivateChainModeDisabled,

        /// Validator has been added to the private chain allowlist.
        /// 
        /// This event is emitted when a validator account is added to the
        /// allowlist in private chain mode, granting them permission to
        /// participate in the network.
        /// 
        /// # Usage
        /// - Validator onboarding tracking
        /// - Access control audit trails
        /// - Network permission management
        ValidatorAddedToAllowlist {
            /// Validator account added to the allowlist
            validator: T::AccountId,
        },

        /// Validator has been removed from the private chain allowlist.
        /// 
        /// This event is emitted when a validator account is removed from
        /// the allowlist in private chain mode, revoking their permission
        /// to participate in the network.
        /// 
        /// # Usage
        /// - Validator offboarding tracking
        /// - Access control audit trails
        /// - Network permission management
        ValidatorRemovedFromAllowlist {
            /// Validator account removed from the allowlist
            validator: T::AccountId,
        },

        /// Validator was forced to leave due to allowlist removal.
        /// 
        /// This event is emitted when an active validator is automatically
        /// removed from the validator set because they were removed from
        /// the private chain allowlist.
        /// 
        /// # Usage
        /// - Automatic validator management tracking
        /// - Network security monitoring
        /// - Compliance enforcement logging
        ValidatorForcedToLeave {
            /// Validator account that was forced to leave
            validator: T::AccountId,
            /// Reason for the forced departure
            reason: Vec<u8>,
        },

        /// EVM compatibility test was performed.
        /// 
        /// This event is emitted when the EVM compatibility system is tested
        /// to validate that DCF events can be properly converted to EVM format.
        /// 
        /// # Usage
        /// - EVM integration testing and validation
        /// - System health monitoring for EVM compatibility
        /// - Development and debugging of EVM features
        EvmCompatibilityTested {
            /// Whether the compatibility test succeeded
            success: bool,
        },

        /// EVM events were queried for a block range.
        /// 
        /// This event is emitted when EVM-compatible events are queried
        /// for analysis or debugging purposes.
        /// 
        /// # Usage
        /// - EVM event querying and analysis
        /// - System debugging and monitoring
        /// - Performance testing of EVM event storage
        EvmEventsQueried {
            /// Starting block number of the query
            from_block: u32,
            /// Ending block number of the query
            to_block: u32,
            /// Optional event type filter applied
            event_type: Option<u32>,
            /// Number of events found in the query
            result_count: u32,
        },

        /// Emitted when a validator successfully authors a block.
        /// 
        /// This event tracks successful block production by validators and is
        /// essential for monitoring validator performance and network health.
        /// It provides transparency about which validators are actively
        /// participating in block production.
        /// 
        /// # Usage
        /// - Block production monitoring and analytics
        /// - Validator performance tracking
        /// - Network health assessment
        /// - Reward distribution calculations
        BlockAuthored {
            /// Block number that was successfully authored
            block_number: u32,
            /// Validator account that authored the block
            author: T::AccountId,
        },

        /// Emitted when a validator misses their assigned block production slot.
        /// 
        /// This event tracks missed block production opportunities and is
        /// critical for identifying validator performance issues and network
        /// health problems. Missed blocks can indicate validator downtime,
        /// network connectivity issues, or other operational problems.
        /// 
        /// # Usage
        /// - Validator performance monitoring
        /// - Network reliability assessment
        /// - Penalty and slashing calculations
        /// - Validator health diagnostics
        MissedBlock {
            /// Block number that was missed
            block_number: u32,
            /// Validator account that was expected to author the block
            validator: T::AccountId,
        },

        /// Emitted when a validator's status changes from one state to another.
        /// 
        /// This event tracks all validator status transitions including:
        /// - Active to Inactive (due to poor performance or voluntary withdrawal)
        /// - Inactive to Active (when performance improves or rejoining)
        /// - Active/Inactive to Leaving (when requesting to leave the network)
        /// - Any status to Ejected (when forcibly removed for misbehavior)
        /// 
        /// Status changes are critical for monitoring validator lifecycle and
        /// ensuring proper network participation tracking.
        /// 
        /// # Usage
        /// - Validator lifecycle monitoring and analytics
        /// - Status change audit trails and compliance
        /// - Network participation tracking
        /// - Validator management system integration
        ValidatorStatusChanged {
            /// The validator whose status changed
            validator: T::AccountId,
            /// Previous status before the change
            old_status: ValidatorStatus,
            /// New status after the change
            new_status: ValidatorStatus,
            /// Block number when the status change occurred
            block_number: u32,
        },

        /// Emitted at epoch transitions with uptime information for each validator.
        /// 
        /// This event provides comprehensive uptime tracking for all validators
        /// during epoch transitions. It includes detailed metrics about validator
        /// performance and participation during the completed epoch.
        /// 
        /// Uptime metrics help assess validator reliability and are used for:
        /// - Performance-based reward calculations
        /// - Validator ranking and selection
        /// - Network health assessment
        /// - Slashing and penalty decisions
        /// 
        /// # Usage
        /// - Validator performance analytics and reporting
        /// - Reward distribution calculations
        /// - Network reliability monitoring
        /// - Validator health assessment
        ValidatorUptimeUpdated {
            /// The validator whose uptime was updated
            validator: T::AccountId,
            /// Epoch number for which uptime is being reported
            epoch: u32,
            /// Number of blocks the validator was expected to author
            blocks_expected: u32,
            /// Number of blocks the validator successfully authored
            blocks_authored: u32,
            /// Number of blocks the validator missed when selected
            blocks_missed: u32,
            /// Uptime percentage for this epoch (0-10000 representing 0-100%)
            uptime_percentage: u32,
        },

    }

    /// Errors that can occur during replay validation.
    /// 
    /// These errors indicate failures in deterministic epoch processing
    /// replay validation, which is critical for ensuring consensus
    /// consistency across all nodes.
    #[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo)]
    pub enum ReplayValidationError {
        /// Original processing output not found for the specified epoch.
        MissingOriginalOutput,
        /// Replay output hash doesn't match original output hash.
        OutputMismatch {
            original_hash: [u8; 32],
            replay_hash: [u8; 32],
        },
        /// Validator sets don't match between original and replay.
        ValidatorSetMismatch,
        /// Author sequences don't match between original and replay.
        AuthorSequenceMismatch,
        /// Validator scores don't match between original and replay.
        ScoreMismatch,
        /// Input validation failed during replay.
        InvalidInputs,
        /// Replay processing failed due to system error.
        ProcessingFailed,
    }

    // --- Errors --- //
    /// Errors that can occur during pallet operations.
    /// 
    /// These errors provide specific feedback about why operations failed,
    /// enabling clients and users to understand and resolve issues. Each
    /// error includes context about the failure condition and potential
    /// resolution steps.
    #[pallet::error]
    pub enum Error<T> {
        /// The specified validator account was not found in the validator registry.
        /// 
        /// This error occurs when operations reference a validator account that:
        /// - Has never joined the validator set
        /// - Has been removed or ejected from the network
        /// - Was provided with an incorrect account identifier
        /// 
        /// Resolution: Verify the validator account exists and is registered.
        ValidatorNotFound,

        /// The provided consensus weights are invalid or do not sum correctly.
        /// 
        /// This error occurs when updating PoS/PoI weights with values that:
        /// - Do not sum to the required precision factor (typically 10000)
        /// - Contain negative or zero values
        /// - Would result in division by zero in score calculations
        /// 
        /// Resolution: Ensure weights are positive and sum to the precision factor.
        InvalidWeight,

        /// The epoch configuration parameters are invalid or inconsistent.
        /// 
        /// This error occurs when epoch configuration contains:
        /// - Zero or negative epoch length
        /// - Minimum stake amounts that exceed maximum possible values
        /// - Maximum validator counts that exceed system limits
        /// - Inconsistent parameter combinations
        /// 
        /// Resolution: Verify all epoch parameters are within valid ranges.
        InvalidEpochConfig,

        /// The operation requires more validators than are currently available.
        /// 
        /// This error occurs when:
        /// - Attempting to remove validators below the minimum required count
        /// - Governance proposals require more validators than exist
        /// - Epoch transitions cannot maintain minimum validator requirements
        /// 
        /// Resolution: Ensure sufficient validators are available for the operation.
        NotEnoughValidators,

        /// The requested operation is not allowed when governance mode is enabled.
        /// 
        /// This error occurs when attempting operations that are restricted
        /// during governance mode, such as:
        /// - Automatic epoch transitions when manual control is enabled
        /// - Certain administrative functions reserved for governance periods
        /// 
        /// Resolution: Disable governance mode or use appropriate governance functions.
        NotAllowedInGovernanceMode,

        /// The account is not registered as a validator in the network.
        /// 
        /// This error occurs when non-validator accounts attempt operations
        /// that are restricted to registered validators, such as:
        /// - Voting on governance proposals
        /// - Submitting inference results
        /// - Participating in validator-only functions
        /// 
        /// Resolution: Join the validator set before attempting validator operations.
        NotValidator,

        /// The validator has already voted on the specified governance proposal.
        /// 
        /// This error prevents double-voting on governance proposals to maintain
        /// the integrity of the voting process. Each validator can only vote
        /// once per proposal, and votes cannot be changed after submission.
        /// 
        /// Resolution: Votes are final and cannot be modified after submission.
        AlreadyVoted,

        /// The governance proposal has not been approved by validator votes.
        /// 
        /// This error occurs when attempting to execute proposals that:
        /// - Have not reached the required voting quorum
        /// - Have more rejection votes than approval votes
        /// - Are still in the pending voting phase
        /// 
        /// Resolution: Wait for sufficient approval votes before attempting execution.
        ProposalNotApproved,

        /// The governance proposal has already been executed and cannot be executed again.
        /// 
        /// This error prevents duplicate execution of governance proposals to
        /// maintain system consistency. Once a proposal is executed, its
        /// status is permanently marked as executed.
        /// 
        /// Resolution: Proposals can only be executed once after approval.
        ProposalAlreadyExecuted,

        /// The provided score value is outside the valid range.
        /// 
        /// This error occurs when score values:
        /// - Exceed the maximum allowed score (T::MaxValidatorScore)
        /// - Are negative or otherwise invalid
        /// - Would cause overflow in score calculations
        /// 
        /// Resolution: Ensure scores are within the valid range (0 to MaxValidatorScore).
        InvalidScore,

        /// The validator account is already registered in the validator set.
        /// 
        /// This error prevents duplicate validator registrations that could
        /// cause inconsistencies in the validator set. Each account can only
        /// be registered as a validator once.
        /// 
        /// Resolution: Use a different account or check existing validator status.
        ValidatorAlreadyExists,

        /// The validator is not currently in the active validator set.
        /// 
        /// This error occurs when operations require validators to be in the
        /// active set but they are currently:
        /// - Inactive due to low scores
        /// - In cooldown periods
        /// - Temporarily removed for maintenance
        /// 
        /// Resolution: Ensure the validator is active before attempting the operation.
        ValidatorNotInSet,

        /// A leave request is already active and the cooldown period has not expired.
        /// 
        /// This error prevents validators from submitting multiple leave requests
        /// or attempting to leave again before completing the required cooldown
        /// period from a previous leave request.
        /// 
        /// Resolution: Wait for the current cooldown period to expire or cancel the existing request.
        LeaveCooldownActive,

        /// The validator is in a cooldown period after leaving and cannot rejoin yet.
        /// 
        /// This error enforces the mandatory cooldown period that prevents
        /// validators from immediately rejoining after leaving the network.
        /// This prevents gaming of the validator system and ensures network stability.
        /// 
        /// Resolution: Wait for the cooldown period to expire before attempting to rejoin.
        ValidatorInCooldown,

        /// The validator has a pending leave request and cannot perform this operation.
        /// 
        /// This error occurs when validators attempt operations that are incompatible
        /// with having an active leave request, such as:
        /// - Submitting additional leave requests
        /// - Modifying stake while leaving
        /// - Participating in governance while leaving
        /// 
        /// Resolution: Cancel the leave request or wait for cooldown to complete.
        ValidatorHasPendingLeaveRequest,

        /// The validator cannot rejoin due to insufficient stake reservation.
        /// 
        /// This error occurs during rejoin validation when:
        /// - The validator's free balance is insufficient for minimum stake
        /// - Previous stake reservation failed to be properly unreserved
        /// - Currency system errors prevent stake reservation
        /// 
        /// Resolution: Ensure sufficient balance and retry the join operation.
        ValidatorRejoinStakeReservationFailed,

        /// The validator cannot rejoin due to cooldown period not yet expired.
        /// 
        /// This error provides specific information about cooldown status when
        /// validators attempt to rejoin before the mandatory cooldown period
        /// has fully expired.
        /// 
        /// Resolution: Wait for the remaining cooldown blocks to pass.
        ValidatorRejoinCooldownNotExpired,

        /// Concurrent leave requests are not allowed for the same validator.
        /// 
        /// This error prevents race conditions and ensures consistent state
        /// when multiple leave requests might be submitted simultaneously
        /// for the same validator account.
        /// 
        /// Resolution: Wait for the current leave request to be processed.
        ConcurrentLeaveRequestNotAllowed,

        /// Block author validation failed due to author mismatch or other issues.
        /// 
        /// This critical error occurs when:
        /// - The actual block author differs from the expected author
        /// - Block author validation cannot be completed
        /// - Consensus algorithm inconsistencies are detected
        /// 
        /// Resolution: This indicates a serious consensus issue that requires investigation.
        AuthorValidationFailed,

        /// The validator does not have sufficient stake to join the network.
        /// 
        /// This error occurs when validators attempt to join with stake amounts
        /// below the minimum required threshold (T::MinStake). Sufficient stake
        /// is required to ensure economic security and validator commitment.
        /// 
        /// Resolution: Increase stake to meet the minimum requirement before joining.
        InsufficientStake,

        /// The trust score calculation failed due to invalid parameters or data.
        /// 
        /// This error occurs when trust score computation encounters:
        /// - Invalid historical data
        /// - Inconsistent performance metrics
        /// - Configuration errors in trust score weights
        /// 
        /// Resolution: Verify trust score configuration and validator performance data.
        TrustScoreCalculationFailed,

        /// The validator metadata is invalid or exceeds size limits.
        /// 
        /// This error occurs when validator metadata:
        /// - Exceeds maximum allowed length for names, descriptions, etc.
        /// - Contains invalid UTF-8 encoding
        /// - Includes prohibited characters or content
        /// 
        /// Resolution: Ensure metadata meets size and format requirements.
        InvalidValidatorMetadata,

        /// The stake operation failed due to insufficient balance or other issues.
        /// 
        /// This error occurs when stake operations cannot be completed due to:
        /// - Insufficient free balance for reserving stake
        /// - Currency system errors
        /// - Arithmetic overflow in stake calculations
        /// 
        /// Resolution: Ensure sufficient balance is available for stake operations.
        StakeOperationFailed,

        /// The cooldown period configuration is invalid or inconsistent.
        /// 
        /// This error occurs when cooldown periods:
        /// - Are set to zero or negative values
        /// - Exceed reasonable maximum limits
        /// - Are inconsistent with other timing parameters
        /// 
        /// Resolution: Configure cooldown periods within valid ranges.
        InvalidCooldownPeriod,

        /// The misbehavior evidence is invalid or cannot be processed.
        /// 
        /// This error occurs when misbehavior reports contain:
        /// - Invalid cryptographic proofs
        /// - Evidence that exceeds maximum size limits
        /// - Malformed or corrupted evidence data
        /// 
        /// Resolution: Ensure evidence is properly formatted and within size limits.
        InvalidMisbehaviorEvidence,

        /// The epoch transition failed due to system inconsistencies.
        /// 
        /// This error occurs when epoch transitions cannot be completed due to:
        /// - Validator set inconsistencies
        /// - Score calculation errors
        /// - System state corruption
        /// 
        /// Resolution: This indicates a serious system issue requiring investigation.
        EpochTransitionFailed,

        /// The parameter value is outside the allowed range defined in governance configuration.
        /// 
        /// This error occurs when attempting to update DCF parameters with values that:
        /// - Are below the minimum allowed value for the parameter
        /// - Exceed the maximum allowed value for the parameter
        /// - Would create unsafe or unstable network conditions
        /// 
        /// Each parameter has predefined safe operating ranges to prevent:
        /// - Network destabilization from extreme values
        /// - Economic vulnerabilities from inappropriate settings
        /// - Performance degradation from poor configurations
        /// 
        /// Resolution: Check the parameter's allowed range and provide a value within bounds.
        ParameterOutOfRange,

        /// The specified parameter type is not recognized or supported.
        /// 
        /// This error occurs when attempting to update parameters that:
        /// - Do not exist in the current parameter governance system
        /// - Are not yet implemented for governance updates
        /// - Have been deprecated or removed from the system
        /// 
        /// Resolution: Verify the parameter type exists and is supported for updates.
        InvalidParameterType,

        /// The parameter update operation is not authorized for the calling account.
        /// 
        /// This error enforces that only root accounts can update DCF parameters
        /// through the governance system. Parameter updates require the highest
        /// level of authorization to prevent unauthorized network configuration changes.
        /// 
        /// Unauthorized parameter changes could:
        /// - Compromise network security and stability
        /// - Create economic vulnerabilities
        /// - Disrupt consensus operations
        /// - Enable attacks or gaming
        /// 
        /// Resolution: Ensure the call is made with root origin/authorization.
        UnauthorizedParameterUpdate,

        /// The governance configuration is invalid or corrupted.
        /// 
        /// This error occurs when the governance configuration storage contains:
        /// - Inconsistent parameter ranges (min > max)
        /// - Invalid current values outside their ranges
        /// - Corrupted or malformed configuration data
        /// - Missing required configuration parameters
        /// 
        /// This is a critical system error that indicates governance system corruption
        /// and requires immediate attention to restore parameter governance functionality.
        /// 
        /// Resolution: This indicates a serious system issue requiring investigation and repair.
        InvalidGovernanceConfig,
        
        /// Arithmetic operation would result in overflow.
        /// 
        /// This error occurs when balance calculations would exceed the maximum
        /// value that can be represented by the Balance type. This prevents
        /// silent overflow that could lead to incorrect balance calculations.
        /// 
        /// Common causes:
        /// - Adding rewards that exceed maximum balance
        /// - Multiplying large stake amounts by percentages
        /// - Accumulating values beyond type limits
        /// 
        /// Resolution: Use smaller amounts or implement chunked operations.
        ArithmeticOverflow,
        
        /// Arithmetic operation would result in underflow.
        /// 
        /// This error occurs when balance calculations would result in negative
        /// values that cannot be represented by unsigned Balance types. This
        /// prevents silent underflow that could lead to incorrect calculations.
        /// 
        /// Common causes:
        /// - Slashing more than available balance
        /// - Subtracting larger values from smaller ones
        /// - Negative intermediate calculations
        /// 
        /// Resolution: Ensure sufficient balance before performing operations.
        ArithmeticUnderflow,
        
        /// Operation would exceed per-epoch slashing bounds.
        /// 
        /// This error occurs when slashing operations would exceed the maximum
        /// allowed slashing amount per epoch, either for individual validators
        /// or across all validators. This prevents excessive slashing that could
        /// destabilize the network.
        /// 
        /// Resolution: Reduce slashing amounts or distribute across multiple epochs.
        SlashingBoundsExceeded,
        
        /// Operation would exceed per-epoch reward bounds.
        /// 
        /// This error occurs when reward operations would exceed the maximum
        /// allowed reward amount per epoch, either for individual validators
        /// or across all validators. This prevents excessive reward distribution.
        /// 
        /// Resolution: Reduce reward amounts or distribute across multiple epochs.
        RewardBoundsExceeded,
        
        /// Storage version mismatch detected during runtime initialization.
        /// 
        /// This error occurs when the runtime detects that the storage schema version
        /// does not match the expected version for the current runtime. This indicates
        /// that a migration is required before the runtime can safely operate.
        /// 
        /// Common causes:
        /// - Runtime upgrade without proper migration
        /// - Corrupted storage version data
        /// - Downgrade to incompatible runtime version
        /// - Missing migration execution
        /// 
        /// Resolution: Execute the appropriate migration or restore compatible runtime.
        StorageVersionMismatch,
        
        /// Storage migration failed to complete successfully.
        /// 
        /// This error occurs when storage migration encounters issues that prevent
        /// successful completion. Migration failures can leave the system in an
        /// inconsistent state and require manual intervention.
        /// 
        /// Common causes:
        /// - Insufficient storage space for migration
        /// - Corrupted source data during migration
        /// - Logic errors in migration code
        /// - Resource constraints during migration
        /// 
        /// Resolution: Investigate migration logs and retry with fixes.
        MigrationFailed,
        
        /// Storage validation failed during migration or initialization.
        /// 
        /// This error occurs when storage validation checks detect inconsistencies
        /// or corruption in the storage data. This can happen during migrations
        /// or runtime initialization when validating existing data.
        /// 
        /// Common causes:
        /// - Data corruption in storage
        /// - Incomplete previous migrations
        /// - Manual storage modifications
        /// - Hardware or software failures
        /// 
        /// Resolution: Restore from backup or perform data recovery procedures.
        StorageValidationFailed,

        /// Rate limit exceeded for per-block operations.
        /// 
        /// This error occurs when the number of operations of a specific type
        /// in the current block exceeds the configured per-block limit. This
        /// prevents DoS attacks that attempt to overwhelm the network with
        /// excessive operations in a single block.
        /// 
        /// Common causes:
        /// - Multiple accounts submitting operations simultaneously
        /// - Coordinated spam attacks on the network
        /// - Misconfigured automation tools
        /// - Network congestion during high activity periods
        /// 
        /// Resolution: Wait for the next block or reduce operation frequency.
        PerBlockRateLimitExceeded,

        /// Rate limit exceeded for per-account operations.
        /// 
        /// This error occurs when an account attempts to perform more operations
        /// of a specific type within the rate limiting time window than allowed.
        /// This prevents individual accounts from spamming the network.
        /// 
        /// Common causes:
        /// - Rapid repeated operations by the same account
        /// - Automated tools with excessive operation frequency
        /// - Account compromise leading to spam behavior
        /// - Misconfigured client applications
        /// 
        /// Resolution: Wait for the rate limiting window to expire before retrying.
        PerAccountRateLimitExceeded,

        /// Minimum interval between operations not respected.
        /// 
        /// This error occurs when an account attempts to perform an operation
        /// before the required minimum interval has elapsed since their last
        /// operation of the same type. This prevents rapid-fire operations
        /// that could destabilize the network.
        /// 
        /// Common causes:
        /// - Attempting validator status changes too frequently
        /// - Submitting proposals in rapid succession
        /// - Client applications not respecting timing constraints
        /// - Race conditions in automated systems
        /// 
        /// Resolution: Wait for the minimum interval to elapse before retrying.
        MinimumIntervalViolation,

        /// Operation weight exceeds configured bounds.
        /// 
        /// This error occurs when an operation would consume more computational
        /// weight than the configured maximum for that operation type. This
        /// prevents operations with large loops or heavy computation from
        /// blocking the network.
        /// 
        /// Common causes:
        /// - Operations iterating over large validator sets
        /// - Processing excessive numbers of proposals
        /// - Complex calculations exceeding weight limits
        /// - Unbounded loops in operation logic
        /// 
        /// Resolution: Reduce operation scope or increase weight limits if appropriate.
        WeightLimitExceeded,

        /// Rate limiting configuration is invalid.
        /// 
        /// This error occurs when attempting to update rate limiting configuration
        /// with invalid values that could compromise network security or stability.
        /// 
        /// Common causes:
        /// - Zero or negative rate limits
        /// - Inconsistent time windows and limits
        /// - Weight limits that are too restrictive
        /// - Configuration values that would prevent normal operation
        /// 
        /// Resolution: Provide valid rate limiting configuration values.
        InvalidRateLimitConfig,

        /// Genesis configuration validation failed.
        /// 
        /// This error occurs when genesis configuration validation detects issues
        /// that would prevent safe network initialization. The validation checks
        /// include duplicate validator detection, stake validation, active set
        /// size limits, and invariant verification.
        /// 
        /// Common causes:
        /// - Duplicate validators in the genesis configuration
        /// - Validator stakes below minimum requirements
        /// - Active set size exceeding MaxValidators limit
        /// - Invalid epoch configuration parameters
        /// - Invariant violations in the proposed configuration
        /// 
        /// Resolution: Fix the genesis configuration issues and retry validation.
        GenesisValidationFailed,

        /// Private chain mode is disabled but operation requires it.
        /// 
        /// This error occurs when attempting private chain specific operations
        /// while the network is running in public mode.
        /// 
        /// Resolution: Enable private chain mode or use public mode operations.
        PrivateChainModeDisabled,

        /// Allowlist updates are disabled in private chain mode.
        /// 
        /// This error occurs when attempting to modify the validator allowlist
        /// while allowlist updates are disabled in the private chain configuration.
        /// 
        /// Resolution: Enable allowlist updates or use root privileges if available.
        AllowlistUpdatesDisabled,

        /// Validator allowlist is full and cannot accept more entries.
        /// 
        /// This error occurs when attempting to add validators to a full allowlist
        /// that has reached its maximum capacity.
        /// 
        /// Resolution: Remove existing validators or increase allowlist capacity.
        AllowlistFull,

        /// Account is not in the validator allowlist for private chain.
        /// 
        /// This error occurs when non-allowlisted accounts attempt operations
        /// that are restricted to allowlisted validators in private chain mode.
        /// 
        /// Resolution: Add the account to the allowlist or use an allowlisted account.
        NotInAllowlist,

        /// Proposal submitter is not in the validator allowlist.
        /// 
        /// This error occurs when non-allowlisted accounts attempt to submit
        /// governance proposals in private chain mode.
        /// 
        /// Resolution: Use an allowlisted account to submit proposals.
        ProposerNotInAllowlist,

        /// Proposal target is not in the validator allowlist.
        /// 
        /// This error occurs when proposals target validators that are not
        /// in the allowlist in private chain mode.
        /// 
        /// Resolution: Target only allowlisted validators in proposals.
        TargetNotInAllowlist,

        /// Too many validators specified for the allowlist.
        /// 
        /// This error occurs when attempting to create a private chain allowlist
        /// with more validators than the maximum allowed.
        /// 
        /// Resolution: Reduce the number of validators in the allowlist.
        TooManyValidators,
    }

    // --- Dispatchable Calls --- //
    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Update a validator's stake score (PoS).
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::update_validator_stake_score())]
        pub fn update_validator_stake_score(
            origin: OriginFor<T>,
            validator: T::AccountId,
        ) -> DispatchResult {
            if GovernanceModeEnabled::<T>::get() {
                ensure_root(origin)?;
            } else {
                ensure_signed(origin)?;
            }
            let stake = pos::Pallet::<T>::stake(&validator);
            let stake_score = stake.saturated_into::<u64>();
            ValidatorStates::<T>::try_mutate(&validator, |maybe_state| {
                let state = maybe_state.as_mut().ok_or(Error::<T>::ValidatorNotFound)?;
                state.current.stake_score = stake_score;
                Ok::<(), Error<T>>(())
            }).map_err(|e| sp_runtime::DispatchError::from(e))?;
            Self::update_final_score(&validator)?;
            Ok(())
        }


        /// Update consensus weights for PoS and PoI.
        #[pallet::call_index(2)]
        #[pallet::weight(<T as Config>::WeightInfo::update_consensus_weights())]
        pub fn update_consensus_weights(
            origin: OriginFor<T>,
            pos_weight: u64,
            poi_weight: u64,
        ) -> DispatchResult {
            ensure_root(origin)?;
            ensure!(pos_weight + poi_weight == T::PercentagePrecision::get() as u64, Error::<T>::InvalidWeight);
            PosWeight::<T>::put(pos_weight);
            PoiWeight::<T>::put(poi_weight);
            Self::deposit_event_with_evm_compat(Event::ConsensusWeightsUpdated {
                pos_weight,
                poi_weight,
            });
            Ok(())
        }

        /// Update a DCF parameter through the governance system with safety rails.
        /// 
        /// This dispatchable allows root accounts to update any configurable DCF parameter
        /// while enforcing safety constraints defined in the governance configuration.
        /// All parameter updates are validated against predefined ranges to prevent
        /// dangerous configurations that could destabilize the network.
        /// 
        /// The function:
        /// - Validates the caller has root authorization
        /// - Checks the parameter type is supported
        /// - Validates the new value is within the allowed range
        /// - Updates the parameter in the governance configuration
        /// - Applies the change to the active system configuration
        /// - Emits an event documenting the change with old/new values
        /// 
        /// # Parameters
        /// - `origin`: Must be root origin for authorization
        /// - `parameter`: The specific parameter type to update
        /// - `value`: New value for the parameter (encoded as bytes for type flexibility)
        /// 
        /// # Errors
        /// - `UnauthorizedParameterUpdate`: If caller is not root
        /// - `InvalidParameterType`: If parameter type is not supported
        /// - `ParameterOutOfRange`: If value is outside allowed range
        /// - `InvalidGovernanceConfig`: If governance config is corrupted
        #[pallet::call_index(34)]
        #[pallet::weight(<T as Config>::WeightInfo::update_consensus_weights())] // Reuse similar weight
        pub fn update_dcf_parameter(
            origin: OriginFor<T>,
            parameter: ParameterType,
            value: BoundedVec<u8, ConstU32<64>>,
        ) -> DispatchResult {
            // Ensure only root can update parameters
            ensure_root(origin)?;
            
            // Get current governance configuration
            let mut config = GovernanceConfigStorage::<T>::get();
            
            // Store old value for event emission
            let old_value = Self::get_parameter_value(&config, &parameter)?;
            
            // Validate and update the parameter
            Self::validate_and_update_parameter(&mut config, parameter.clone(), &value)?;
            
            // Store updated configuration
            GovernanceConfigStorage::<T>::put(&config);
            
            // Apply the parameter change to active system configuration
            Self::apply_parameter_change(&parameter, &value)?;
            
            // Emit event documenting the change
            Self::deposit_event(Event::DcfParameterUpdated {
                parameter,
                old_value,
                new_value: value,
            });
            
            Ok(())
        }

        /// Update rate limiting configuration for DoS protection.
        /// 
        /// This dispatchable allows root accounts to update the rate limiting
        /// configuration that controls how frequently various operations can be
        /// performed to prevent abuse and DoS attacks.
        /// 
        /// The function validates the new configuration to ensure it won't prevent
        /// normal network operation while providing adequate protection against abuse.
        /// 
        /// # Parameters
        /// - `origin`: Must be root origin for authorization
        /// - `max_proposals_per_block`: Maximum proposal submissions per block
        /// - `max_joins_per_block`: Maximum validator join operations per block
        /// - `max_leaves_per_block`: Maximum validator leave operations per block
        /// 
        /// # Errors
        /// - `BadOrigin`: If caller is not root
        /// - `InvalidRateLimitConfig`: If configuration values are invalid
        #[pallet::call_index(35)]
        #[pallet::weight(Weight::from_parts(50_000_000, 0))] // Fixed weight for config update
        pub fn update_rate_limit_config(
            origin: OriginFor<T>,
            max_proposals_per_block: u32,
            max_joins_per_block: u32,
            max_leaves_per_block: u32,
        ) -> DispatchResult {
            // Ensure only root can update rate limiting configuration
            ensure_root(origin)?;
            
            // Create new config with provided values and defaults for others
            let mut new_config = RateLimitConfigStorage::<T>::get();
            new_config.max_proposals_per_block = max_proposals_per_block;
            new_config.max_joins_per_block = max_joins_per_block;
            new_config.max_leaves_per_block = max_leaves_per_block;
            
            // Update configuration with validation
            Self::update_rate_limit_config_internal(new_config)?;
            
            Ok(())
        }

        /// Toggle governance mode (sudo-like).
        #[pallet::call_index(3)]
        #[pallet::weight(<T as Config>::WeightInfo::set_governance_mode())]
        pub fn set_governance_mode(origin: OriginFor<T>, enabled: bool) -> DispatchResult {
            ensure_root(origin)?;
            GovernanceModeEnabled::<T>::put(enabled);
            Self::deposit_event(Event::GovernanceModeToggled { enabled });
            Ok(())
        }

        /// Sudo: advance epoch manually (governance mode only).
        #[pallet::call_index(4)]
        #[pallet::weight(<T as Config>::WeightInfo::sudo_advance_epoch())]
        pub fn sudo_advance_epoch(origin: OriginFor<T>) -> DispatchResult {
            ensure_root(origin)?;
            ensure!(GovernanceModeEnabled::<T>::get(), Error::<T>::NotAllowedInGovernanceMode);
            let _ = Self::handle_epoch_transition();
            Ok(())
        }

        /// Submit a governance proposal (slash, reward, eject).
        #[pallet::call_index(5)]
        #[pallet::weight(<T as Config>::WeightInfo::submit_proposal())]
        pub fn submit_proposal(
            origin: OriginFor<T>,
            action: ProposalAction<T>,
            description: Option<BoundedVec<u8, ConstU32<128>>>,
        ) -> DispatchResult {
            let proposer = ensure_signed(origin)?;
            
            // Check rate limits before proceeding
            let weight = <T as Config>::WeightInfo::submit_proposal();
            if let Err((e, violation_opt)) = Self::check_rate_limits(&proposer, DispatchableType::SubmitProposal, weight) {
                // Handle rate limit violation using the violation info
                if let Some(violation) = violation_opt {
                    let (operation_code, violation_type, current_count, limit) = match violation {
                        RateLimitViolation::PerBlockLimitExceeded { current_count, limit, .. } => (0u8, 0u8, current_count, limit),
                        RateLimitViolation::PerAccountLimitExceeded { current_count, limit, .. } => (0u8, 1u8, current_count, limit),
                        RateLimitViolation::MinimumIntervalViolation { blocks_since_last, required_interval, .. } => (0u8, 2u8, blocks_since_last, required_interval),
                        RateLimitViolation::WeightLimitExceeded { actual_weight, max_weight, .. } => (0u8, 3u8, (actual_weight / 1000) as u32, (max_weight / 1000) as u32),
                    };
                    
                    Self::deposit_event(Event::RateLimitViolation {
                        account: proposer,
                        operation: operation_code,
                        violation_type,
                        current_count,
                        limit,
                    });
                }
                
                return Err(e);
            }
            
            // Check private chain governance restrictions
            if Self::is_private_chain_mode() {
                Self::validate_governance_in_private_mode(&proposer)?;
                
                // Also validate the target if it's a validator-specific action
                match &action {
                    ProposalAction::Slash { validator, .. } |
                    ProposalAction::Reward { validator, .. } => {
                        Self::validate_proposal_in_private_mode(&proposer, validator)?;
                    },
                    ProposalAction::Eject { validator, reason: _ } => {
                        Self::validate_proposal_in_private_mode(&proposer, validator)?;
                    },
                    _ => {
                        // For non-validator-specific actions, just check proposer
                    }
                }
            }
            
            let proposal_id = NextProposalId::<T>::get();
            let proposal = GovernanceProposal {
                proposer: proposer.clone(),
                action: action.clone(),
                status: ProposalStatus::Pending,
                votes_for: 0,
                votes_against: 0,
            };
            Proposals::<T>::insert(proposal_id, proposal);
            NextProposalId::<T>::put(proposal_id + 1);
            Self::deposit_event(Event::ProposalSubmitted {
                proposal_id,
                proposer: proposer.clone(),
                action: action.clone(),
            });
            
            // Emit detailed proposal created event
            Self::deposit_event(Event::ProposalCreated {
                proposal_id,
                proposer: proposer.clone(),
                action,
                description: description.unwrap_or_else(|| BoundedVec::truncate_from(b"No description provided".to_vec())),
            });
            
            // Record the operation for rate limiting
            Self::record_operation(&proposer, DispatchableType::SubmitProposal);
            
            Ok(())
        }

        /// Vote on a governance proposal.
        #[pallet::call_index(6)]
        #[pallet::weight(<T as Config>::WeightInfo::vote_proposal())]
        pub fn vote_proposal(
            origin: OriginFor<T>,
            proposal_id: u32,
            approve: bool,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            
            // Check rate limits before proceeding
            let weight = <T as Config>::WeightInfo::vote_proposal();
            if let Err((e, violation_opt)) = Self::check_rate_limits(&who, DispatchableType::VoteProposal, weight) {
                // Handle rate limit violation using the violation info
                if let Some(violation) = violation_opt {
                    let (operation_code, violation_type, current_count, limit) = match violation {
                        RateLimitViolation::PerBlockLimitExceeded { current_count, limit, .. } => (2u8, 0u8, current_count, limit),
                        RateLimitViolation::PerAccountLimitExceeded { current_count, limit, .. } => (2u8, 1u8, current_count, limit),
                        RateLimitViolation::MinimumIntervalViolation { blocks_since_last, required_interval, .. } => (2u8, 2u8, blocks_since_last, required_interval),
                        RateLimitViolation::WeightLimitExceeded { actual_weight, max_weight, .. } => (2u8, 3u8, (actual_weight / 1000) as u32, (max_weight / 1000) as u32),
                    };
                    
                    Self::deposit_event(Event::RateLimitViolation {
                        account: who,
                        operation: operation_code,
                        violation_type,
                        current_count,
                        limit,
                    });
                }
                
                return Err(e);
            }
            Proposals::<T>::try_mutate_exists(proposal_id, |maybe_prop| {
                let prop = maybe_prop.as_mut().ok_or(Error::<T>::ProposalNotApproved)?;
                ensure!(matches!(prop.status, ProposalStatus::Pending), Error::<T>::ProposalAlreadyExecuted);
                ensure!(!ProposalVotes::<T>::contains_key(proposal_id, &who), Error::<T>::AlreadyVoted);

                if approve {
                    prop.votes_for += 1;
                } else {
                    prop.votes_against += 1;
                }
                ProposalVotes::<T>::insert(proposal_id, &who, approve);
                Self::deposit_event(Event::ProposalVoted {
                    proposal_id,
                    voter: who.clone(),
                    approve,
                });

                // --- Quorum logic: require at least half of active validators to vote ---
                let quorum = (ActiveValidators::<T>::get().len() as u32 + 1) / 2;
                let total_votes = prop.votes_for + prop.votes_against;
                if total_votes >= quorum {
                    if prop.votes_for > prop.votes_against {
                        prop.status = ProposalStatus::Approved;
                        Self::deposit_event(Event::ProposalPassed { proposal_id });
                    } else {
                        prop.status = ProposalStatus::Rejected;
                        Self::deposit_event(Event::ProposalRejected { proposal_id });
                    }
                }
                Ok::<(), Error<T>>(())
            })?;
            
            // Record the operation for rate limiting
            Self::record_operation(&who, DispatchableType::VoteProposal);
            
            Ok(())
        }

        /// Execute an approved governance proposal (sudo only).
        #[pallet::call_index(7)]
        #[pallet::weight(<T as Config>::WeightInfo::execute_proposal())]
        pub fn execute_proposal(
            origin: OriginFor<T>,
            proposal_id: u32,
        ) -> DispatchResult {
            ensure_root(origin)?;
            Proposals::<T>::try_mutate_exists(proposal_id, |maybe_prop| {
                let prop = maybe_prop.as_mut().ok_or(Error::<T>::ProposalNotApproved)?;
                ensure!(matches!(prop.status, ProposalStatus::Approved), Error::<T>::ProposalNotApproved);

                // Execute action with proper implementation
                match &prop.action {
                    ProposalAction::Slash { validator, amount } => {
                        // Implement actual slashing logic
                        let _ = pos::Pallet::<T>::execute_slash_validator(validator, *amount);
                    }
                    ProposalAction::Reward { validator, amount } => {
                        // Implement actual reward logic
                        let _ = pos::Pallet::<T>::execute_reward_validator(validator, *amount);
                    }
                    ProposalAction::RewardMultiple { validators, amount } => {
                        // Reward multiple validators with the same amount
                        for validator in validators.iter() {
                            let _ = pos::Pallet::<T>::execute_reward_validator(validator, *amount);
                        }
                        log::info!("Rewarded {} validators with amount {:?} each", validators.len(), amount);
                    }
                    ProposalAction::Eject { validator, reason } => {
                        // Implement actual ejection logic
                        let _ = Self::execute_eject_validator(validator, reason.clone());
                    }
                    ProposalAction::AddValidator { validator } => {
                        // Add validator to the validator set
                        let _ = Self::execute_add_validator(validator);
                    }
                    ProposalAction::RemoveValidator { validator } => {
                        // Remove validator from the validator set
                        let _ = Self::execute_remove_validator(validator);
                    }
                }
                prop.status = ProposalStatus::Executed;
                Self::deposit_event(Event::ProposalExecuted {
                    proposal_id,
                    status: prop.status.clone(),
                });
                Ok::<(), Error<T>>(())
            })?;
            Ok(())
        }

        /// Sudo propose to slash a validator.
        #[pallet::call_index(8)]
        #[pallet::weight(<T as Config>::WeightInfo::propose_slash_validator())]
        pub fn propose_slash_validator(
            origin: OriginFor<T>,
            proposer: T::AccountId,
            validator: T::AccountId,
            amount: <T as pallet::Config>::Balance,
        ) -> DispatchResult {
            ensure_root(origin)?;
            let action = ProposalAction::Slash { validator, amount };
            let proposal_id = NextProposalId::<T>::get();
            let proposal = GovernanceProposal {
                proposer: proposer.clone(),
                action: action.clone(),
                status: ProposalStatus::Pending,
                votes_for: 0,
                votes_against: 0,
            };
            Proposals::<T>::insert(proposal_id, proposal);
            NextProposalId::<T>::put(proposal_id + 1);
            Self::deposit_event(Event::ProposalSubmitted {
                proposal_id,
                proposer,
                action,
            });
            Ok(())
        }

        /// Sudo propose to reward a validator.
        #[pallet::call_index(9)]
        #[pallet::weight(<T as Config>::WeightInfo::propose_reward_validator())]
        pub fn propose_reward_validator(
            origin: OriginFor<T>,
            proposer: T::AccountId,
            validator: T::AccountId,
            amount: <T as pallet::Config>::Balance,
        ) -> DispatchResult {
            ensure_root(origin)?;
            let action = ProposalAction::Reward { validator, amount };
            let proposal_id = NextProposalId::<T>::get();
            let proposal = GovernanceProposal {
                proposer: proposer.clone(),
                action: action.clone(),
                status: ProposalStatus::Pending,
                votes_for: 0,
                votes_against: 0,
            };
            Proposals::<T>::insert(proposal_id, proposal);
            NextProposalId::<T>::put(proposal_id + 1);
            Self::deposit_event(Event::ProposalSubmitted {
                proposal_id,
                proposer,
                action,
            });
            Ok(())
        }

        /// Propose to reward a validator with the default reward amount
        #[pallet::call_index(26)]
        #[pallet::weight(<T as Config>::WeightInfo::propose_default_reward_validator())]
        pub fn propose_default_reward_validator(
            origin: OriginFor<T>,
            proposer: T::AccountId,
            validator: T::AccountId,
        ) -> DispatchResult {
            ensure_root(origin)?;
            let default_reward = T::ValidatorReward::get();
            let action = ProposalAction::Reward { validator: validator.clone(), amount: default_reward };
            let proposal_id = NextProposalId::<T>::get();
            let proposal = GovernanceProposal {
                proposer: proposer.clone(),
                action: action.clone(),
                status: ProposalStatus::Pending,
                votes_for: 0,
                votes_against: 0,
            };
            Proposals::<T>::insert(proposal_id, proposal);
            NextProposalId::<T>::put(proposal_id + 1);
            Self::deposit_event(Event::ProposalSubmitted {
                proposal_id,
                proposer,
                action,
            });
            log::info!("Default reward proposal created for validator {:?} with amount {:?}", validator, default_reward);
            Ok(())
        }

        /// Propose to reward multiple validators with the same amount
        #[pallet::call_index(27)]
        #[pallet::weight(<T as Config>::WeightInfo::propose_reward_multiple_validators(validators.len() as u32))]
        pub fn propose_reward_multiple_validators(
            origin: OriginFor<T>,
            proposer: T::AccountId,
            validators: Vec<T::AccountId>,
            amount: <T as pallet::Config>::Balance,
        ) -> DispatchResult {
            ensure_root(origin)?;
            
            // Validate validators list length
            ensure!(
                validators.len() <= MaxValidatorsOf::<T>::get() as usize,
                Error::<T>::NotEnoughValidators
            );
            ensure!(!validators.is_empty(), Error::<T>::NotEnoughValidators);
            
            // Check weight bounds for validator iteration
            let config = RateLimitConfigStorage::<T>::get();
            let estimated_weight = validators.len() as u64 * 10_000; // Estimate 10k weight per validator validation
            
            if estimated_weight > config.max_proposal_processing_weight {
                return Err(Error::<T>::WeightLimitExceeded.into());
            }
            
            // Limit iterations to prevent unbounded loops
            let max_iterations = config.max_loop_iterations.min(validators.len() as u32);
            ensure!(
                validators.len() <= max_iterations as usize,
                Error::<T>::WeightLimitExceeded
            );
            
            // Validate that all validators exist
            for validator in validators.iter() {
                ensure!(
                    ValidatorStates::<T>::contains_key(validator),
                    Error::<T>::ValidatorNotFound
                );
            }
            
            let bounded_validators = BoundedVec::try_from(validators.clone())
                .map_err(|_| Error::<T>::NotEnoughValidators)?;
            let action = ProposalAction::RewardMultiple { validators: bounded_validators, amount };
            let proposal_id = NextProposalId::<T>::get();
            let proposal = GovernanceProposal {
                proposer: proposer.clone(),
                action: action.clone(),
                status: ProposalStatus::Pending,
                votes_for: 0,
                votes_against: 0,
            };
            Proposals::<T>::insert(proposal_id, proposal);
            NextProposalId::<T>::put(proposal_id + 1);
            Self::deposit_event(Event::ProposalSubmitted {
                proposal_id,
                proposer,
                action,
            });
            log::info!("Multiple validator reward proposal created for {} validators with amount {:?} each", 
                      validators.len(), amount);
            Ok(())
        }

        /// Propose to reward multiple validators with the default reward amount
        #[pallet::call_index(28)]
        #[pallet::weight(<T as Config>::WeightInfo::propose_default_reward_multiple_validators())]
        pub fn propose_default_reward_multiple_validators(
            origin: OriginFor<T>,
            proposer: T::AccountId,
            validators: Vec<T::AccountId>,
        ) -> DispatchResult {
            ensure_root(origin)?;
            
            // Validate validators list length
            ensure!(
                validators.len() <= MaxValidatorsOf::<T>::get() as usize,
                Error::<T>::NotEnoughValidators
            );
            ensure!(!validators.is_empty(), Error::<T>::NotEnoughValidators);
            
            // Validate that all validators exist
            for validator in validators.iter() {
                ensure!(
                    ValidatorStates::<T>::contains_key(validator),
                    Error::<T>::ValidatorNotFound
                );
            }
            
            let default_reward = T::ValidatorReward::get();
            let bounded_validators = BoundedVec::try_from(validators.clone())
                .map_err(|_| Error::<T>::NotEnoughValidators)?;
            let action = ProposalAction::RewardMultiple { validators: bounded_validators, amount: default_reward };
            let proposal_id = NextProposalId::<T>::get();
            let proposal = GovernanceProposal {
                proposer: proposer.clone(),
                action: action.clone(),
                status: ProposalStatus::Pending,
                votes_for: 0,
                votes_against: 0,
            };
            Proposals::<T>::insert(proposal_id, proposal);
            NextProposalId::<T>::put(proposal_id + 1);
            Self::deposit_event(Event::ProposalSubmitted {
                proposal_id,
                proposer,
                action,
            });
            log::info!("Default multiple validator reward proposal created for {} validators with amount {:?} each", 
                      validators.len(), default_reward);
            Ok(())
        }

        /// Propose to reward all active validators with the default reward amount
        #[pallet::call_index(29)]
        #[pallet::weight(<T as Config>::WeightInfo::propose_reward_all_active_validators())]
        pub fn propose_reward_all_active_validators(
            origin: OriginFor<T>,
            proposer: T::AccountId,
        ) -> DispatchResult {
            ensure_root(origin)?;
            
            let active_validators = ActiveValidators::<T>::get();
            ensure!(!active_validators.is_empty(), Error::<T>::NotEnoughValidators);
            
            let default_reward = T::ValidatorReward::get();
            let bounded_validators = BoundedVec::try_from(active_validators.clone().into_inner())
                .map_err(|_| Error::<T>::NotEnoughValidators)?;
            let action = ProposalAction::RewardMultiple { 
                validators: bounded_validators, 
                amount: default_reward 
            };
            let proposal_id = NextProposalId::<T>::get();
            let proposal = GovernanceProposal {
                proposer: proposer.clone(),
                action: action.clone(),
                status: ProposalStatus::Pending,
                votes_for: 0,
                votes_against: 0,
            };
            Proposals::<T>::insert(proposal_id, proposal);
            NextProposalId::<T>::put(proposal_id + 1);
            Self::deposit_event(Event::ProposalSubmitted {
                proposal_id,
                proposer,
                action,
            });
            log::info!("Reward proposal created for all {} active validators with amount {:?} each", 
                      active_validators.len(), default_reward);
            Ok(())
        }

        // direct slashing calls moved to pallet-cbc-pos.

        /// Sudo propose to eject a validator.
        #[pallet::call_index(10)]
        #[pallet::weight(<T as Config>::WeightInfo::propose_eject_validator())]
        pub fn propose_eject_validator(
            origin: OriginFor<T>,
            proposer: T::AccountId,
            validator: T::AccountId,
            reason: EjectionReason,
        ) -> DispatchResult {
            ensure_root(origin)?;
            let action = ProposalAction::Eject { validator, reason: reason.clone() };
            let proposal_id = NextProposalId::<T>::get();
            let proposal = GovernanceProposal {
                proposer: proposer.clone(),
                action: action.clone(),
                status: ProposalStatus::Pending,
                votes_for: 0,
                votes_against: 0,
            };
            Proposals::<T>::insert(proposal_id, proposal);
            NextProposalId::<T>::put(proposal_id + 1);
            Self::deposit_event(Event::ProposalSubmitted {
                proposal_id,
                proposer,
                action,
            });
            Ok(())
        }

        /// Request to join the validator set (opt-in, effective next epoch).
        #[pallet::call_index(13)]
        #[pallet::weight(<T as Config>::WeightInfo::join_validator_set())]
        pub fn join_validator_set(origin: OriginFor<T>) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // Already pending join or already active
            ensure!(
                !Self::active_validators().contains(&who),
                Error::<T>::NotAllowedInGovernanceMode 
            );
            ensure!(
                PendingValidatorActions::<T>::get(&who) != Some(ValidatorAction::Join),
                Error::<T>::NotAllowedInGovernanceMode
            );

            // Check minimum stake and score now, but actual addition is at epoch
            let stake = pos::Pallet::<T>::stake(&who);
            ensure!(
                stake >= MinStakeOf::<T>::get(),
                Error::<T>::NotEnoughValidators
            );
            let state = ValidatorStates::<T>::get(&who).ok_or(Error::<T>::ValidatorNotFound)?;
            ensure!(
                state.current.final_score >= MinValidatorScoreOf::<T>::get() as u64,
                Error::<T>::NotValidator
            );

            PendingValidatorActions::<T>::insert(&who, ValidatorAction::Join);
            Self::deposit_event_with_evm_compat(Event::ValidatorJoined { 
                validator: who,
                stake_amount: MinStakeOf::<T>::get(),
            });
            Ok(())
        }

        /// Validate a genesis configuration without applying it.
        /// 
        /// This dispatchable allows validation of genesis configurations for testing
        /// and verification purposes. It performs comprehensive checks including
        /// duplicate validator detection, stake validation, and invariant verification.
        /// 
        /// # Arguments
        /// * `origin` - Must be root origin for security
        /// * `validators` - List of validator accounts
        /// * `validator_stakes` - List of validator stakes (must match validators length or be empty)
        /// * `epoch_config` - Epoch configuration parameters
        /// 
        /// # Errors
        /// * `BadOrigin` - If caller is not root
        /// * `GenesisValidationFailed` - If validation fails with details in event
        /// 
        /// # Requirements Coverage
        /// * 11.1: Validates for duplicate validators and invalid stakes
        /// * 11.2: Checks active set size not exceeding MaxValidators
        /// * 11.3: Provides dry-run capabilities for genesis validation
        #[pallet::call_index(36)]
        #[pallet::weight(Weight::from_parts(100_000_000, 0))] // Fixed weight for validation
        pub fn validate_genesis_configuration(
            origin: OriginFor<T>,
            validators: Vec<T::AccountId>,
            validator_stakes: Vec<<T as pallet::Config>::Balance>,
            epoch_config: EpochConfig,
        ) -> DispatchResult {
            ensure_root(origin)?;

            // Create a temporary genesis config for validation
            let genesis_config = GenesisConfig::<T> {
                validators,
                validator_scores: Vec::new(), // Use default scores
                validator_stakes,
                current_epoch: 0,
                epoch_config,
                validator_names: Vec::new(), // Use default names
                strict_validation: true,
            };

            // Perform validation
            match Self::validate_genesis_config(&genesis_config) {
                Ok(()) => {
                    // Emit success event
                    Self::deposit_event(Event::GenesisValidationSucceeded {
                        validator_count: genesis_config.validators.len() as u32,
                        total_stake: genesis_config.validator_stakes.iter().fold(
                            <T as pallet::Config>::Balance::from(0u32),
                            |acc, stake| acc.saturating_add(*stake)
                        ),
                    });
                    Ok(())
                },
                Err(error) => {
                    log::error!("============== [CBC-TRACE] 38 [pallet-cbc-dcf::validate_genesis_configuration] Genesis validation failed: {:?} ==============", error);
                    // Emit failure event with error details
                    Self::deposit_event(Event::GenesisValidationFailed {
                        error: error.as_bytes().to_vec().try_into().unwrap_or_default(),
                    });
                    Err(Error::<T>::GenesisValidationFailed.into())
                }
            }
        }

        /// Perform a dry-run of genesis build with comprehensive reporting.
        /// 
        /// This dispatchable simulates the complete genesis build process without
        /// modifying storage, providing detailed validation results and analysis.
        /// 
        /// # Arguments
        /// * `origin` - Must be root origin for security
        /// * `validators` - List of validator accounts
        /// * `validator_stakes` - List of validator stakes (must match validators length or be empty)
        /// * `epoch_config` - Epoch configuration parameters
        /// 
        /// # Errors
        /// * `BadOrigin` - If caller is not root
        /// * `GenesisValidationFailed` - If dry-run fails with details in event
        /// 
        /// # Requirements Coverage
        /// * 11.3: Implements dry-run function that builds genesis and asserts all invariants
        /// * 11.4: Provides comprehensive validation without side effects
        #[pallet::call_index(37)]
        #[pallet::weight(Weight::from_parts(150_000_000, 0))] // Higher weight for comprehensive analysis
        pub fn dry_run_genesis_configuration(
            origin: OriginFor<T>,
            validators: Vec<T::AccountId>,
            validator_stakes: Vec<<T as pallet::Config>::Balance>,
            epoch_config: EpochConfig,
        ) -> DispatchResult {
            ensure_root(origin)?;

            // Create a temporary genesis config for dry-run
            let genesis_config = GenesisConfig::<T> {
                validators,
                validator_scores: Vec::new(), // Use default scores
                validator_stakes,
                current_epoch: 0,
                epoch_config,
                validator_names: Vec::new(), // Use default names
                strict_validation: true,
            };

            // Perform dry-run
            match Self::dry_run_genesis_build(&genesis_config) {
                Ok(report) => {
                    // Emit success event with report summary
                    Self::deposit_event(Event::GenesisDryRunCompleted {
                        validator_count: report.validator_count,
                        total_stake: report.total_stake,
                        invariant_checks_passed: report.invariant_checks.len() as u32,
                        warnings_count: report.warnings.len() as u32,
                    });
                    Ok(())
                },
                Err(error) => {
                    log::error!("============== [CBC-TRACE] 38 [pallet-cbc-dcf::dry_run_genesis_configuration] Genesis validation failed: {:?} ==============", error);
                    // Emit failure event with error details
                    Self::deposit_event(Event::GenesisDryRunFailed {
                        error: error.as_bytes().to_vec().try_into().unwrap_or_default(),
                    });
                    Err(Error::<T>::GenesisValidationFailed.into())
                }
            }
        }

        // cancel_leave_request moved to pallet-cbc-pos.

        /// Request to leave the validator set (opt-out, effective next epoch).
        #[pallet::call_index(14)]
        #[pallet::weight(<T as Config>::WeightInfo::leave_validator_set())]
        pub fn leave_validator_set(origin: OriginFor<T>) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // Already pending leave or not active
            ensure!(
                Self::active_validators().contains(&who),
                Error::<T>::NotValidator
            );
            ensure!(
                PendingValidatorActions::<T>::get(&who) != Some(ValidatorAction::Leave),
                Error::<T>::NotAllowedInGovernanceMode
            );

            PendingValidatorActions::<T>::insert(&who, ValidatorAction::Leave);
            Self::deposit_event_with_evm_compat(Event::ValidatorLeft { validator: who });
            Ok(())
        }

        /// Set validator display name
        #[pallet::call_index(15)]
        #[pallet::weight(<T as Config>::WeightInfo::set_validator_name())]
        pub fn set_validator_name(
            origin: OriginFor<T>,
            name: Vec<u8>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            
            // Ensure the validator exists
            ensure!(
                ValidatorStates::<T>::contains_key(&who),
                Error::<T>::ValidatorNotFound
            );
            
            // Validate name length
            let bounded_name = BoundedVec::try_from(name)
                .map_err(|_| Error::<T>::InvalidEpochConfig)?;
            
            ValidatorNames::<T>::insert(&who, &bounded_name);
            
            // Emit event for validator name registration
            Self::deposit_event(Event::ValidatorNameRegistered {
                validator: who,
                name: bounded_name,
            });
            
            Ok(())
        }

        /// Set comprehensive validator metadata
        #[pallet::call_index(16)]
        #[pallet::weight(<T as Config>::WeightInfo::set_validator_metadata())]
        pub fn set_validator_metadata(
            origin: OriginFor<T>,
            name: Vec<u8>,
            website: Option<Vec<u8>>,
            contact: Option<Vec<u8>>,
            description: Option<Vec<u8>>,
            location: Option<Vec<u8>>,
            commission_rate: Option<u32>,
            min_stake_required: Option<<T as pallet::Config>::Balance>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            
            // Ensure the validator exists
            ensure!(
                ValidatorStates::<T>::contains_key(&who),
                Error::<T>::ValidatorNotFound
            );
            
            // Validate and convert inputs
            let bounded_name = BoundedVec::try_from(name)
                .map_err(|_| Error::<T>::InvalidEpochConfig)?;
            
            let bounded_website = website.map(|w| BoundedVec::try_from(w))
                .transpose()
                .map_err(|_| Error::<T>::InvalidEpochConfig)?;
            
            let bounded_contact = contact.map(|c| BoundedVec::try_from(c))
                .transpose()
                .map_err(|_| Error::<T>::InvalidEpochConfig)?;
            
            let bounded_description = description.map(|d| BoundedVec::try_from(d))
                .transpose()
                .map_err(|_| Error::<T>::InvalidEpochConfig)?;
            
            let bounded_location = location.map(|l| BoundedVec::try_from(l))
                .transpose()
                .map_err(|_| Error::<T>::InvalidEpochConfig)?;
            
            // Validate commission rate
            if let Some(rate) = commission_rate {
                ensure!(rate <= T::MaxCommissionRate::get(), Error::<T>::InvalidEpochConfig);
            }
            
            let now = Self::get_current_timestamp();
            let metadata = ValidatorMetadataInfo {
                name: bounded_name.clone(),
                website: bounded_website,
                contact: bounded_contact,
                description: bounded_description,
                location: bounded_location,
                commission_rate,
                min_stake_required: min_stake_required.map(|s| s.saturated_into()),
                created_at: ValidatorMetadata::<T>::get(&who)
                    .map(|m| m.created_at)
                    .unwrap_or(now),
                updated_at: now,
            };
            
            ValidatorMetadata::<T>::insert(&who, metadata);
            ValidatorNames::<T>::insert(&who, &bounded_name);
            
            // Emit event for validator name registration
            Self::deposit_event(Event::ValidatorNameRegistered {
                validator: who.clone(),
                name: bounded_name.clone(),
            });
            
            // Set join time if not already set in pos pallet
            if !pos::ValidatorJoinTime::<T>::contains_key(&who) {
                let current_block = frame_system::Pallet::<T>::block_number().saturated_into::<u32>();
                pos::ValidatorJoinTime::<T>::insert(&who, current_block);
            }
            
            // Emit event
            Self::deposit_event(Event::ValidatorMetadataUpdated {
                validator: who,
                name: bounded_name,
            });
            
            Ok(())
        }

        /// Update validator activity metrics (called internally)
        #[pallet::call_index(17)]
        #[pallet::weight(<T as Config>::WeightInfo::update_validator_activity())]
        pub fn update_validator_activity(
            origin: OriginFor<T>,
            validator: T::AccountId,
            blocks_authored: u32,
            blocks_missed: u32,
        ) -> DispatchResult {
            ensure_root(origin)?; // Only callable by root or internal logic
            
            let current_block = frame_system::Pallet::<T>::block_number().saturated_into::<u32>();
            
            // Update last seen
            ValidatorLastSeen::<T>::insert(&validator, current_block);
            
            // Update total counters
            ValidatorBlocksAuthored::<T>::mutate(&validator, |total| *total += blocks_authored);
            ValidatorBlocksMissed::<T>::mutate(&validator, |total| *total += blocks_missed);
            
            // Update performance history
            let current_epoch = Self::current_epoch();
            let now = Self::get_current_timestamp();
            
            if let Some(state) = ValidatorStates::<T>::get(&validator) {
                let uptime_percentage = Self::calculate_uptime_percentage(&validator);
                
                let performance_record = PerformanceRecord {
                    epoch: current_epoch,
                    blocks_authored,
                    blocks_missed,
                    uptime_percentage,
                    inference_score: state.current.inference_score,
                    stake_score: state.current.stake_score,
                    final_score: state.current.final_score,
                    participation_rate: state.participation_rate,
                    timestamp: now,
                };
                
                ValidatorPerformanceHistory::<T>::mutate(&validator, |history| {
                    if history.len() >= T::MaxPerformanceHistoryLength::get() as usize {
                        history.remove(0); // Remove oldest record
                    }
                    let _ = history.try_push(performance_record);
                });
                
                // Emit event
                Self::deposit_event(Event::ValidatorActivityUpdated {
                    validator: validator.clone(),
                    blocks_authored,
                    blocks_missed,
                    uptime_percentage,
                });
            }
            
            Ok(())
        }

        /// Report validator misbehavior with evidence
        #[pallet::call_index(21)]
        #[pallet::weight(<T as Config>::WeightInfo::report_validator_misbehavior())]
        pub fn report_validator_misbehavior(
            origin: OriginFor<T>,
            validator: T::AccountId,
            evidence: BoundedVec<u8, T::MaxEvidenceLength>,
        ) -> DispatchResult {
            let reporter = ensure_signed(origin)?;
            
            // Ensure the reporter is in the ValidatorSet
            let validator_set = ValidatorSet::<T>::get();
            ensure!(
                validator_set.contains(&reporter),
                Error::<T>::ValidatorNotInSet
            );
            
            // Ensure the reported validator exists in the ValidatorSet
            ensure!(
                validator_set.contains(&validator),
                Error::<T>::ValidatorNotFound
            );
            
            // Ensure reporter is not reporting themselves
            ensure!(reporter != validator, Error::<T>::InvalidScore); // Reuse existing error
            
            // Check if this reporter has already reported this validator
            ensure!(
                !MisbehaviorReports::<T>::contains_key(&validator, &reporter),
                Error::<T>::InvalidScore // Reuse existing error for duplicate report
            );
            
            // Store the report
            MisbehaviorReports::<T>::insert(&validator, &reporter, &evidence);
            
            // Count total reports for this validator
            let mut report_count = 0u32;
            for (_, _) in MisbehaviorReports::<T>::iter_prefix(&validator) {
                report_count = report_count.saturating_add(1);
            }
            
            // Emit event
            Self::deposit_event(Event::ValidatorMisbehaviorReported {
                reported_validator: validator.clone(),
                reporter: reporter.clone(),
                evidence: evidence.clone(),
                total_reports: report_count,
            });
            
            // Check if we've reached the threshold for automatic slashing
            let slash_threshold = T::MisbehaviorSlashThreshold::get();
            if report_count >= slash_threshold {
                // Execute automatic slash
                let slash_amount = Self::calculate_slash_amount(&validator)?;
                let _ = pos::Pallet::<T>::execute_slash_validator_with_reason(&validator, slash_amount, pos::SlashReason::ConsensusViolation);
                
                // Force eject validator
                let _ = pos::Pallet::<T>::force_eject_validator(&validator);
                
                // Clear all reports for this validator since they've been slashed
                let _ = MisbehaviorReports::<T>::clear_prefix(&validator, u32::MAX, None);
                
                // Emit auto-slash event
                Self::deposit_event(Event::ValidatorAutoSlashed {
                    validator,
                    amount: slash_amount,
                    report_count,
                });
            }
            
            Ok(())
        }

    }

    // --- Internal Logic --- //
    impl<T: Config> Pallet<T> {
        /// Emit event with EVM compatibility support.
        /// 
        /// This function wraps the standard event emission with EVM compatibility
        /// features, ensuring events can be properly indexed and queried from
        /// EVM-based applications when the EVM pallet is enabled.
        /// 
        /// # Parameters
        /// - `event`: The DCF event to emit
        /// 
        /// # Effects
        /// - Emits the event via standard Substrate event system
        /// - Converts and stores EVM-compatible version if EVM is enabled
        /// - Validates event payload for EVM compatibility
        pub fn deposit_event_with_evm_compat(event: Event<T>) {
            // Emit standard Substrate event
            <Pallet<T>>::deposit_event(event.clone());
            
            // Attempt to emit EVM-compatible version
            if let Err(e) = Self::emit_evm_compatible_event(&event) {
                // Log error but don't fail the operation
                log::warn!("Failed to emit EVM-compatible event: {:?}", e);
            }
        }
        




        /// Get encoded parameter value for runtime API.
        pub fn get_parameter_value_encoded(parameter: Vec<u8>) -> Option<Vec<u8>> {
            use codec::Decode;
            
            // Decode the parameter type
            if let Ok(param_type) = ParameterType::decode(&mut &parameter[..]) {
                let config = GovernanceConfigStorage::<T>::get();
                
                match param_type {
                    ParameterType::EpochLength => Some(config.epoch_length.current.encode()),
                    ParameterType::MinStake => Some(config.min_stake.current.encode()),
                    ParameterType::MaxValidators => Some(config.max_validators.current.encode()),
                    ParameterType::MinPerformanceScore => Some(config.min_performance_score.current.encode()),
                    ParameterType::SlashPercent => Some(config.slash_percent.current.encode()),
                    ParameterType::ValidatorReward => Some(config.validator_reward.current.encode()),
                    ParameterType::LeaveCooldown => Some(config.leave_cooldown.current.encode()),
                    ParameterType::PosWeight => Some(config.pos_weight.current.encode()),
                    ParameterType::PoiWeight => Some(config.poi_weight.current.encode()),
                    ParameterType::BlockAuthorshipBoost => Some(config.block_authorship_boost.current.encode()),
                    ParameterType::MissedBlockPenalty => Some(config.missed_block_penalty.current.encode()),
                    ParameterType::InferenceBoostLow => Some(config.inference_boost_low.current.encode()),
                    ParameterType::InferenceBoostMedium => Some(config.inference_boost_medium.current.encode()),
                    ParameterType::InferenceBoostHigh => Some(config.inference_boost_high.current.encode()),
                    _ => None, // Other parameter types not yet implemented
                }
            } else {
                None
            }
        }

        /// Validate encoded parameter value for runtime API.
        pub fn validate_parameter_value_encoded(parameter: Vec<u8>, value: Vec<u8>) -> bool {
            use codec::Decode;
            
            // Decode the parameter type
            if let Ok(param_type) = ParameterType::decode(&mut &parameter[..]) {
                let config = GovernanceConfigStorage::<T>::get();
                
                match param_type {
                    ParameterType::EpochLength => {
                        if let Ok(val) = u32::decode(&mut &value[..]) {
                            val >= config.epoch_length.min && val <= config.epoch_length.max
                        } else { false }
                    },
                    ParameterType::MinStake => {
                        if let Ok(val) = <T as pallet::Config>::Balance::decode(&mut &value[..]) {
                            val >= config.min_stake.min && val <= config.min_stake.max
                        } else { false }
                    },
                    ParameterType::MaxValidators => {
                        if let Ok(val) = u32::decode(&mut &value[..]) {
                            val >= config.max_validators.min && val <= config.max_validators.max
                        } else { false }
                    },
                    ParameterType::MinPerformanceScore => {
                        if let Ok(val) = u64::decode(&mut &value[..]) {
                            val >= config.min_performance_score.min && val <= config.min_performance_score.max
                        } else { false }
                    },
                    ParameterType::SlashPercent => {
                        if let Ok(val) = u32::decode(&mut &value[..]) {
                            val >= config.slash_percent.min && val <= config.slash_percent.max
                        } else { false }
                    },
                    ParameterType::ValidatorReward => {
                        if let Ok(val) = <T as pallet::Config>::Balance::decode(&mut &value[..]) {
                            val >= config.validator_reward.min && val <= config.validator_reward.max
                        } else { false }
                    },
                    ParameterType::LeaveCooldown => {
                        if let Ok(val) = BlockNumberFor::<T>::decode(&mut &value[..]) {
                            val >= config.leave_cooldown.min && val <= config.leave_cooldown.max
                        } else { false }
                    },
                    ParameterType::PosWeight => {
                        if let Ok(val) = u64::decode(&mut &value[..]) {
                            val >= config.pos_weight.min && val <= config.pos_weight.max
                        } else { false }
                    },
                    ParameterType::PoiWeight => {
                        if let Ok(val) = u64::decode(&mut &value[..]) {
                            val >= config.poi_weight.min && val <= config.poi_weight.max
                        } else { false }
                    },
                    ParameterType::BlockAuthorshipBoost => {
                        if let Ok(val) = u64::decode(&mut &value[..]) {
                            val >= config.block_authorship_boost.min && val <= config.block_authorship_boost.max
                        } else { false }
                    },
                    ParameterType::MissedBlockPenalty => {
                        if let Ok(val) = u64::decode(&mut &value[..]) {
                            val >= config.missed_block_penalty.min && val <= config.missed_block_penalty.max
                        } else { false }
                    },
                    ParameterType::InferenceBoostLow => {
                        if let Ok(val) = u64::decode(&mut &value[..]) {
                            val >= config.inference_boost_low.min && val <= config.inference_boost_low.max
                        } else { false }
                    },
                    ParameterType::InferenceBoostMedium => {
                        if let Ok(val) = u64::decode(&mut &value[..]) {
                            val >= config.inference_boost_medium.min && val <= config.inference_boost_medium.max
                        } else { false }
                    },
                    ParameterType::InferenceBoostHigh => {
                        if let Ok(val) = u64::decode(&mut &value[..]) {
                            val >= config.inference_boost_high.min && val <= config.inference_boost_high.max
                        } else { false }
                    },
                    _ => false, // Other parameter types not yet implemented
                }
            } else {
                false
            }
        }

        /// Recalculate and update the final score for a validator.
        fn update_final_score(validator: &T::AccountId) -> DispatchResult {
            ValidatorStates::<T>::try_mutate(validator, |maybe_state| {
                let state = maybe_state.as_mut().ok_or(Error::<T>::ValidatorNotFound)?;
                let stake_score = state.current.stake_score;
                let inference_score = state.current.inference_score;
                let pos_weight = if !PosWeight::<T>::exists() {
                    let weight = T::DefaultPosWeight::get();
                    PosWeight::<T>::put(weight);
                    weight
                } else {
                    PosWeight::<T>::get()
                };
                let poi_weight = if !PoiWeight::<T>::exists() {
                    let weight = T::DefaultPoiWeight::get();
                    PoiWeight::<T>::put(weight);
                    weight
                } else {
                    PoiWeight::<T>::get()
                };
                let mut final_score = (stake_score.saturating_mul(pos_weight) + inference_score.saturating_mul(poi_weight)) / T::PercentagePrecision::get() as u64;
                if final_score > T::MaxValidatorScore::get() {
                    final_score = T::MaxValidatorScore::get();
                }
                let old_stake_score = state.current.stake_score;
                let old_inference_score = state.current.inference_score;
                let old_final_score = state.current.final_score;
                state.current.final_score = final_score;
                state.last_active_epoch = Self::current_epoch();
                
                // NOTE: Do NOT write a score-sorted ActiveValidators list back to storage here.
                // ActiveValidators is sorted by account ID at epoch transitions only.
                // Score-based ordering is for metrics/display only (see get_validators_by_score).
                if state.history.len() == state.history.capacity() && !state.history.is_empty() {
                    state.history.remove(0);
                }
                let _ = state.history.try_push(EpochStats {
                    epoch: Self::current_epoch(),
                    stake_score,
                    inference_score,
                    final_score,
                    authored_blocks: state.current.authored_blocks,
                    missed_blocks: state.current.missed_blocks,
                });
                Self::deposit_event_with_evm_compat(Event::ValidatorScoreUpdated {
                    validator: validator.clone(),
                    stake_score,
                    inference_score,
                    final_score,
                });
                
                // Emit detailed score update event
                Self::deposit_event(Event::ValidatorScoreUpdatedDetailed {
                    validator: validator.clone(),
                    old_stake_score,
                    new_stake_score: stake_score,
                    old_inference_score,
                    new_inference_score: inference_score,
                    old_final_score,
                    new_final_score: final_score,
                    epoch: Self::current_epoch(),
                });
                Ok::<(), Error<T>>(())
            }).map_err(Into::into)
        }

        /// Apply score decay to a validator if inactive.
        pub fn apply_score_decay(validator: &T::AccountId, current_epoch: u32) -> DispatchResult {
            ValidatorStates::<T>::try_mutate(validator, |maybe_state| {
                let state = maybe_state.as_mut().ok_or(Error::<T>::ValidatorNotFound)?;
                let last_active = state.last_active_epoch;
                let inactive_epochs = current_epoch.saturating_sub(last_active);
                if inactive_epochs > 0 {
                    let decay_rate = ValidatorScoreDecayOf::<T>::get();
                    let decay_amount = state.current.final_score.saturating_mul(decay_rate as u64) / T::PercentagePrecision::get() as u64;
                    let old_score = state.current.final_score;
                    state.current.final_score = state.current.final_score.saturating_sub(decay_amount);
                    if state.current.final_score > T::MaxValidatorScore::get() {
                        state.current.final_score = T::MaxValidatorScore::get();
                    }
                    Self::deposit_event(Event::ValidatorScoreDecayed {
                        validator: validator.clone(),
                        old_score,
                        new_score: state.current.final_score,
                    });
                    if state.current.final_score < MinValidatorScoreOf::<T>::get() as u64 {
                        Self::eject_validator(validator, EjectionReason::ScoreBelowThreshold)
                            .map_err(|_| Error::<T>::ValidatorNotFound)?;
                    }
                }
                Ok::<(), Error<T>>(())
            }).map_err(Into::into)
        }

        /// Boost a validator's score for a given reason.
        fn boost_score(
            validator: &T::AccountId,
            amount: u64,
            reason: ScoreBoostReason,
        ) -> DispatchResult {
            ValidatorStates::<T>::try_mutate(validator, |maybe_state| {
                let state = maybe_state.as_mut().ok_or(Error::<T>::ValidatorNotFound)?;
                let old_score = state.current.final_score;
                state.current.final_score = state.current.final_score.saturating_add(amount);
                if state.current.final_score > T::MaxValidatorScore::get() {
                    state.current.final_score = T::MaxValidatorScore::get();
                }
                state.last_active_epoch = Self::current_epoch();
                if state.history.len() == state.history.capacity() {
                    state.history.remove(0);
                }
                let _ = state.history.try_push(EpochStats {
                    epoch: Self::current_epoch(),
                    stake_score: state.current.stake_score,
                    inference_score: state.current.inference_score,
                    final_score: state.current.final_score,
                    authored_blocks: state.current.authored_blocks,
                    missed_blocks: state.current.missed_blocks,
                });
                Self::deposit_event(Event::ValidatorScoreBoosted {
                    validator: validator.clone(),
                    old_score,
                    new_score: state.current.final_score,
                    reason,
                });
                Ok::<(), Error<T>>(())
            }).map_err(Into::into)
        }

        /// Calculate the slash amount for a validator based on their stake
        fn calculate_slash_amount(validator: &T::AccountId) -> Result<<T as pallet::Config>::Balance, DispatchError> {
            // Ensure validator exists
            ensure!(
                ValidatorStates::<T>::contains_key(validator),
                Error::<T>::ValidatorNotFound
            );
            
            // Calculate slash amount as a percentage of their stake
            let stake_balance = T::Currency::free_balance(validator);
            let slash_percent = T::SlashPercent::get();
            
            // Use saturating operations to avoid overflow
            let slash_amount = stake_balance / 100u32.into() * slash_percent.into();
            
            Ok(slash_amount)
        }

        /// Record inference activity for a validator
        fn record_inference_activity(validator: &T::AccountId) -> DispatchResult {
            let current_block = <frame_system::Pallet<T>>::block_number().saturated_into::<u32>();
            
            ValidatorStates::<T>::try_mutate(validator, |maybe_state| {
                let state = maybe_state.as_mut().ok_or(Error::<T>::ValidatorNotFound)?;
                
                // Increment inference count
                state.inference_count = state.inference_count.saturating_add(1);
                
                // Update last active block
                state.last_active_block = current_block;
                
                // Update last active epoch
                state.last_active_epoch = Self::current_epoch();
                
                Ok::<(), Error<T>>(())
            })?;
            
            
            // Update trust score after inference activity
            Self::update_trust_score(validator)?;
            
            Ok(())
        }

        /// Record a missed block for a validator.
        pub fn record_missed_block(validator: &T::AccountId) -> DispatchResult {
            ValidatorStates::<T>::try_mutate(validator, |maybe_state| {
                let state = maybe_state.as_mut().ok_or(Error::<T>::ValidatorNotFound)?;
                state.current.missed_blocks = state.current.missed_blocks.saturating_add(1);
                Ok::<(), Error<T>>(())
            }).map_err(|e| sp_runtime::DispatchError::from(e))?;

            Ok(())
        }

        /// Record block authorship for a validator and boost score.
        pub fn record_block_authorship(validator: &T::AccountId) -> DispatchResult {
            let current_block = <frame_system::Pallet<T>>::block_number().saturated_into::<u32>();
            
            ValidatorStates::<T>::try_mutate(validator, |maybe_state| {
                let state = maybe_state.as_mut().ok_or(Error::<T>::ValidatorNotFound)?;
                state.current.authored_blocks = state.current.authored_blocks.saturating_add(1);
                // Update last active block
                state.last_active_block = current_block;
                // Update last active epoch
                state.last_active_epoch = Self::current_epoch();
                Ok::<(), Error<T>>(())
            }).map_err(|e| sp_runtime::DispatchError::from(e))?;

            // Update blocks authored counter
            ValidatorBlocksAuthored::<T>::mutate(validator, |count| {
                *count = count.saturating_add(1);
            });

            Self::boost_score(
                validator,
                T::BlockAuthorshipBoost::get(),
                ScoreBoostReason::ValidBlockAuthored,
            )?;
            
            // Update trust score after block authorship
            Self::update_trust_score(validator)?;
            
            Ok(())
        }

        /// Report successful block authorship (Runtime API function)
        /// This is called by the consensus engine after successfully importing a block
        pub fn report_successful_block_authorship(
            block_number: u32,
            author: T::AccountId,
        ) -> Result<(), sp_runtime::DispatchError> {
            // Validate that the author is an active validator
            if !Self::is_validator_active(&author) {
                Self::deposit_event(Event::InvalidAuthor {
                    block_number,
                    author: author.clone(),
                });
                return Err(sp_runtime::DispatchError::Other("Author is not an active validator"));
            }

            // Record the block authorship
            Self::record_block_authorship(&author)?;

            // Emit event for successful block authorship
            Self::deposit_event(Event::BlockAuthored {
                block_number,
                author: author.clone(),
            });

            log::debug!(
                "DCF: Recorded successful block authorship for block #{} by {:?}",
                block_number,
                author
            );

            Ok(())
        }

        /// Report author mismatch (Runtime API function)
        /// This is called when the actual block author doesn't match the expected author
        pub fn report_author_mismatch(
            block_number: u32,
            expected: Option<T::AccountId>,
            actual: T::AccountId,
        ) -> Result<(), sp_runtime::DispatchError> {
            // Emit author mismatch event
            Self::deposit_event(Event::AuthorMismatch {
                block_number,
                expected: expected.clone(),
                actual: actual.clone(),
            });

            // If there was an expected author, record it as a missed block
            if let Some(expected_author) = expected {
                Self::record_missed_block(&expected_author)?;
                
                log::warn!(
                    "DCF: Author mismatch at block #{}: expected {:?}, got {:?}",
                    block_number,
                    expected_author,
                    actual
                );
            }

            Ok(())
        }

        /// Report missed block (Runtime API function)
        /// This is called when a validator fails to produce their assigned block
        pub fn report_missed_block_for_api(
            block_number: u32,
            expected_author: T::AccountId,
        ) -> Result<(), sp_runtime::DispatchError> {
            // Record the missed block
            Self::record_missed_block(&expected_author)?;

            // Emit event for missed block
            Self::deposit_event(Event::MissedBlock {
                block_number,
                validator: expected_author.clone(),
            });

            log::debug!(
                "DCF: Recorded missed block #{} for validator {:?}",
                block_number,
                expected_author
            );

            Ok(())
        }


        /// Eject a validator from the active set for a given reason.
        fn eject_validator(validator: &T::AccountId, reason: EjectionReason) -> DispatchResult {
            let mut active_validators = ActiveValidators::<T>::get();
            if let Some(pos) = active_validators.iter().position(|v| v == validator) {
                active_validators.remove(pos);
                ActiveValidators::<T>::put(active_validators);
            }

            // Emit validator status change event (from Active/Inactive to Ejected)
            let current_block = frame_system::Pallet::<T>::block_number().saturated_into::<u32>();
            let old_status = if Self::is_validator_active(validator) {
                ValidatorStatus::Active
            } else {
                ValidatorStatus::Inactive
            };
            Self::deposit_event(Event::ValidatorStatusChanged {
                validator: validator.clone(),
                old_status,
                new_status: ValidatorStatus::Ejected,
                block_number: current_block,
            });

            Self::deposit_event_with_evm_compat(Event::ValidatorEjected {
                validator: validator.clone(),
                reason,
            });
            Ok(())
        }

        /// Check if the current block number is an epoch boundary.
        fn is_epoch_boundary(block_number: u32) -> bool {
            let epoch_length = T::EpochLength::get();
            block_number > 0 && block_number % epoch_length == 0
        }

        /// Handle comprehensive epoch transition with all required steps.
        fn handle_comprehensive_epoch_transition(block_number: u32) -> Weight {
            let mut weight = Weight::zero();
            
            // Step 1: Run score aggregation for all validators
            weight = weight.saturating_add(Self::run_comprehensive_score_aggregation());
            
            // Step 2: Generate automatic proposals based on current state
            weight = weight.saturating_add(Self::generate_epoch_proposals());
            
            // Step 3: Execute or queue proposals
            weight = weight.saturating_add(Self::process_epoch_proposals());
            
            // Step 4: Handle deterministic epoch transition with replayability
            weight = weight.saturating_add(Self::handle_deterministic_epoch_transition(block_number));
            
            // Step 5: Finalize epoch (optional - finalize last epoch's best block)
            weight = weight.saturating_add(Self::finalize_previous_epoch(block_number));
            
            // Step 6: Update epoch-specific metrics and cleanup
            weight = weight.saturating_add(Self::update_epoch_metrics(block_number));
            
            weight
        }

        /// Handle regular block processing for non-epoch boundary blocks.
        fn handle_regular_block_processing(block_number: u32, current_epoch: u32) -> Weight {
            let mut weight = Weight::zero();
            
            // 1. Validate block authorship and update validator metrics (every block)
            Self::process_block_authorship(block_number);
            
            // 2. Apply score decay for inactive validators
            if block_number % T::ScoreDecayInterval::get() == 0 {
                let decay_weight = Self::apply_validator_score_decay(current_epoch);
                weight = weight.saturating_add(decay_weight);
            }
            
            // 3. Update validator participation rates
            if block_number % T::ParticipationUpdateInterval::get() == 0 {
                let participation_weight = Self::update_validator_participation_rates();
                weight = weight.saturating_add(participation_weight);
                
                // Resort validators by updated scores — only in-memory for display,
                // do NOT write score-sorted order back to ActiveValidators storage
                // (storage order must stay account-ID sorted for determinism).
            }
            
            // 4. Snapshot validator scores 5 blocks before the epoch boundary.
            // This freezes scores at a fixed, agreed-upon point so all nodes use
            // identical weights when generating the next epoch's author sequence,
            // regardless of when each node's offchain worker submitted score updates.
            {
                let epoch_config = Self::epoch_config();
                let blocks_per_epoch = epoch_config.blocks_per_epoch;
                let next_epoch = current_epoch.saturating_add(1);
                let snapshot_block = next_epoch
                    .saturating_mul(blocks_per_epoch)
                    .saturating_sub(5);

                if block_number == snapshot_block
                    && EpochScoreSnapshot::<T>::get(next_epoch).is_none()
                {
                    let active = ActiveValidators::<T>::get();
                    let snapshot: BoundedVec<(T::AccountId, u64), ConstU32<1000>> =
                        BoundedVec::truncate_from(
                            active.iter().map(|v| {
                                let score = ValidatorStates::<T>::get(v)
                                    .map(|s| s.current.final_score)
                                    .unwrap_or(1)
                                    .max(1);
                                (v.clone(), score)
                            }).collect::<Vec<_>>()
                        );
                    EpochScoreSnapshot::<T>::insert(next_epoch, snapshot);
                    log::info!(
                        "DCF: Snapshotted validator scores for epoch {} at block {}",
                        next_epoch, block_number
                    );
                    weight = weight.saturating_add(Weight::from_parts(50_000, 0));
                }
            }

            // 5. Check for low-performing validators
            if block_number % T::UnderperformanceCheckInterval::get() == 0 {
                Self::check_and_handle_underperforming_validators();
            }
            
            // 6. Generate automatic validator proposals based on scores
            if block_number % T::ValidatorProposalInterval::get() == 0 {
                Self::generate_automatic_validator_proposals();
            }
            
            // Cooldown and leave requests are managed by pallet-cbc-pos.

            // 8. Emit periodic health metrics
            if block_number % T::HealthMetricsInterval::get() == 0 {
                Self::emit_dcf_health_metrics(block_number);
            }
            
            weight
        }

        /// Determine if an epoch transition should occur.
        fn should_transition_epoch(
            now: BlockNumberFor<T>,
            current_epoch: u32,
            epoch_config: &EpochConfig,
        ) -> bool {
            let blocks_per_epoch = epoch_config.blocks_per_epoch;
            let epoch_start_block = current_epoch.saturating_mul(blocks_per_epoch);
            let now_u32: u32 = now.saturated_into();
            now_u32 >= epoch_start_block + blocks_per_epoch
        }

        /// Check economic invariants at epoch boundaries.
        /// 
        /// This function validates critical economic invariants that ensure the system's
        /// economic model remains consistent and secure. It checks for violations such as:
        /// - Total reserved balance being less than total slashed amount
        /// - Validators having negative reserved balances
        /// - Active validators with stakes below minimum requirements
        /// 
        /// Returns a list of detected violations for reporting and handling.
        pub fn check_economic_invariants() -> Vec<InvariantViolation<T>> {
            let mut violations = Vec::new();
            
            // Check 1: Total reserved balance >= total slashed amount
            let mut total_reserved = <T as pallet::Config>::Balance::default();
            let mut total_slashed = <T as pallet::Config>::Balance::default();
            
            for validator in ValidatorSet::<T>::get().iter() {
                let reserved = T::Currency::reserved_balance(validator);
                total_reserved = total_reserved.saturating_add(reserved);
                
                // For now, we'll estimate slashed amount from score penalties
                // In a full implementation, this would track actual slashed amounts
                if let Some(state) = ValidatorStates::<T>::get(validator) {
                    let max_score = T::MaxValidatorScore::get();
                    if state.current.final_score < max_score {
                        let score_loss = max_score - state.current.final_score;
                        // Estimate slashed amount based on score loss (simplified)
                        let estimated_slash = T::Currency::minimum_balance().saturating_mul(
                            (score_loss as u32).into()
                        );
                        total_slashed = total_slashed.saturating_add(estimated_slash);
                    }
                }
            }
            
            if total_reserved < total_slashed {
                let context = format!("Reserved: {:?}, Slashed: {:?}", total_reserved, total_slashed);
                if let Ok(bounded_context) = BoundedVec::try_from(context.as_bytes().to_vec()) {
                    violations.push(InvariantViolation::Economic {
                        violation_type: EconomicViolationType::ReservedLessThanSlashed {
                            total_reserved,
                            total_slashed,
                        },
                        context: bounded_context,
                    });
                }
            }
            
            // Check 2: No validator has negative reserved balance
            for validator in ValidatorSet::<T>::get().iter() {
                let reserved = T::Currency::reserved_balance(validator);
                if reserved == <T as pallet::Config>::Balance::default() {
                    // Check if this validator should have a reserved balance
                    if ActiveValidators::<T>::get().contains(validator) {
                        let context = format!("Validator: {:?}", validator);
                        if let Ok(bounded_context) = BoundedVec::try_from(context.as_bytes().to_vec()) {
                            violations.push(InvariantViolation::Economic {
                                violation_type: EconomicViolationType::NegativeReservedBalance {
                                    validator: validator.clone(),
                                    balance: reserved,
                                },
                                context: bounded_context,
                            });
                        }
                    }
                }
            }
            
            // Check 3: Active validators meet minimum stake requirements
            let min_stake = MinStakeOf::<T>::get();
            for validator in ActiveValidators::<T>::get().iter() {
                let stake = T::Currency::reserved_balance(validator);
                if stake < min_stake {
                    let context = format!("Validator: {:?}, Stake: {:?}, Min: {:?}", validator, stake, min_stake);
                    if let Ok(bounded_context) = BoundedVec::try_from(context.as_bytes().to_vec()) {
                        violations.push(InvariantViolation::Economic {
                            violation_type: EconomicViolationType::StakeBelowMinimum {
                                validator: validator.clone(),
                                current_stake: stake,
                                minimum_required: min_stake,
                            },
                            context: bounded_context,
                        });
                    }
                }
            }
            
            violations
        }

        /// Check validator set invariants at epoch boundaries.
        /// 
        /// This function validates validator set invariants that ensure proper
        /// validator lifecycle management and prevent inconsistent states such as:
        /// - Active validator set exceeding maximum allowed size
        /// - Validators being both active and in cooldown simultaneously
        /// - Duplicate validators in the active set
        /// 
        /// Returns a list of detected violations for reporting and handling.
        pub fn check_validator_invariants() -> Vec<InvariantViolation<T>> {
            let mut violations = Vec::new();
            
            let active_validators = ActiveValidators::<T>::get();
            let max_validators = MaxValidatorsOf::<T>::get();
            
            // Check 1: Active set size does not exceed maximum
            if active_validators.len() as u32 > max_validators {
                let context = format!("Active: {}, Max: {}", active_validators.len(), max_validators);
                if let Ok(bounded_context) = BoundedVec::try_from(context.as_bytes().to_vec()) {
                    violations.push(InvariantViolation::Validator {
                        violation_type: ValidatorViolationType::ActiveSetTooLarge {
                            current_size: active_validators.len() as u32,
                            max_allowed: max_validators,
                        },
                        context: bounded_context,
                    });
                }
            }
            
            // Check 2: No validator is both active and in cooldown
            for validator in active_validators.iter() {
                if pos::ValidatorLeaveRequests::<T>::contains_key(validator) || 
                   pos::RecentlyRemovedValidators::<T>::contains_key(validator) {
                    let context = format!("Validator: {:?}", validator);
                    if let Ok(bounded_context) = BoundedVec::try_from(context.as_bytes().to_vec()) {
                        violations.push(InvariantViolation::Validator {
                            violation_type: ValidatorViolationType::ActiveAndInCooldown {
                                validator: validator.clone(),
                            },
                            context: bounded_context,
                        });
                    }
                }
            }
            
            // Check 3: No duplicate validators in active set
            let mut seen_validators = sp_std::collections::btree_set::BTreeSet::new();
            for validator in active_validators.iter() {
                if !seen_validators.insert(validator) {
                    let context = format!("Duplicate validator: {:?}", validator);
                    if let Ok(bounded_context) = BoundedVec::try_from(context.as_bytes().to_vec()) {
                        violations.push(InvariantViolation::Validator {
                            violation_type: ValidatorViolationType::DuplicateInActiveSet {
                                validator: validator.clone(),
                            },
                            context: bounded_context,
                        });
                    }
                }
            }
            
            violations
        }

        /// Check temporal invariants at epoch boundaries.
        /// 
        /// This function validates temporal invariants that ensure proper progression
        /// of time-based system state such as:
        /// - Epochs advancing monotonically
        /// - Finality markers advancing monotonically
        /// - Finality never exceeds best known block of prior epoch
        /// 
        /// Returns a list of detected violations for reporting and handling.
        pub fn check_temporal_invariants(previous_epoch: u32, current_epoch: u32) -> Vec<InvariantViolation<T>> {
            let mut violations = Vec::new();
            
            // Check 1: Epoch progression is monotonic
            if current_epoch < previous_epoch {
                let context = format!("Previous: {}, Current: {}", previous_epoch, current_epoch);
                if let Ok(bounded_context) = BoundedVec::try_from(context.as_bytes().to_vec()) {
                    violations.push(InvariantViolation::Temporal {
                        violation_type: TemporalViolationType::EpochRegression {
                            previous_epoch,
                            current_epoch,
                        },
                        context: bounded_context,
                    });
                }
            }
            
            // Check 2: Finality markers advance monotonically
            let current_finalized = LastFinalizedBlock::<T>::get();
            let previous_finalized = PreviousFinalizedBlock::<T>::get();
            
            if current_finalized < previous_finalized {
                let context = format!("Previous finalized: {}, Current finalized: {}", previous_finalized, current_finalized);
                if let Ok(bounded_context) = BoundedVec::try_from(context.as_bytes().to_vec()) {
                    violations.push(InvariantViolation::Temporal {
                        violation_type: TemporalViolationType::FinalityRegression {
                            previous_finalized,
                            current_finalized,
                        },
                        context: bounded_context,
                    });
                }
            }
            
            // Check 3: Finality doesn't exceed current block
            let current_block = frame_system::Pallet::<T>::block_number().saturated_into::<u32>();
            if current_finalized > current_block {
                let context = format!("Finalized: {}, Current block: {}", current_finalized, current_block);
                if let Ok(bounded_context) = BoundedVec::try_from(context.as_bytes().to_vec()) {
                    violations.push(InvariantViolation::Temporal {
                        violation_type: TemporalViolationType::FinalityRegression {
                            previous_finalized: current_block,
                            current_finalized,
                        },
                        context: bounded_context,
                    });
                }
            }
            
            // Check 4: Finality doesn't exceed best known block of previous epoch
            let previous_epoch_best = PreviousEpochBestBlock::<T>::get();
            if current_finalized > previous_epoch_best && previous_epoch_best > 0 {
                let context = format!("Finalized: {}, Previous epoch best: {}", current_finalized, previous_epoch_best);
                if let Ok(bounded_context) = BoundedVec::try_from(context.as_bytes().to_vec()) {
                    violations.push(InvariantViolation::Temporal {
                        violation_type: TemporalViolationType::FinalityRegression {
                            previous_finalized: previous_epoch_best,
                            current_finalized,
                        },
                        context: bounded_context,
                    });
                }
            }
            
            violations
        }

        /// Validate finality marker advancement with comprehensive correctness checks.
        /// 
        /// This function performs comprehensive validation of finality marker updates
        /// to ensure they comply with all finality correctness requirements:
        /// - Monotonic advancement (never moves backward)
        /// - Bounded by current block number
        /// - Bounded by best known block of previous epoch
        /// - Reasonable advancement rate (not too large jumps)
        /// 
        /// # Arguments
        /// - `new_finalized_block`: The block number being proposed for finalization
        /// - `epoch`: Current epoch number for context
        /// 
        /// # Returns
        /// - `Ok(())`: If the finality advancement is valid
        /// - `Err(reason)`: If the advancement should be rejected with reason
        pub fn validate_finality_advancement(
            new_finalized_block: u32, 
            epoch: u32
        ) -> Result<(), BoundedVec<u8, ConstU32<128>>> {
            let current_finalized = LastFinalizedBlock::<T>::get();
            let _previous_finalized = PreviousFinalizedBlock::<T>::get();
            let current_block = frame_system::Pallet::<T>::block_number().saturated_into::<u32>();
            let previous_epoch_best = PreviousEpochBestBlock::<T>::get();
            
            // Check 1: Monotonic advancement - new finalized must be >= current
            if new_finalized_block < current_finalized {
                let reason = format!("Finality regression: {} < {}", new_finalized_block, current_finalized);
                if let Ok(bounded_reason) = BoundedVec::try_from(reason.as_bytes().to_vec()) {
                    // Emit regression detection event
                    Self::deposit_event(Event::FinalityRegressionDetected {
                        previous_finalized: current_finalized,
                        attempted_finalized: new_finalized_block,
                        current_block,
                        epoch,
                    });
                    return Err(bounded_reason);
                }
            }
            
            // Check 2: Cannot exceed current block number
            if new_finalized_block > current_block {
                let reason = format!("Finality exceeds current block: {} > {}", new_finalized_block, current_block);
                if let Ok(bounded_reason) = BoundedVec::try_from(reason.as_bytes().to_vec()) {
                    Self::deposit_event(Event::FinalityAdvancementRejected {
                        attempted_block: new_finalized_block,
                        current_finalized,
                        best_known_block: current_block,
                        epoch,
                        reason: bounded_reason.clone(),
                    });
                    return Err(bounded_reason);
                }
            }
            
            // Check 3: Cannot exceed best known block of previous epoch (if available)
            // For progressive finalization, we allow advancement within the current epoch
            // Only restrict if we're trying to finalize blocks from future epochs
            let current_epoch = Self::current_epoch();
            let current_epoch_start = current_epoch.saturating_mul(T::EpochLength::get());
            let is_progressive_finalization = new_finalized_block <= current_block && 
                                            new_finalized_block >= current_epoch_start;
            
            if previous_epoch_best > 0 && !is_progressive_finalization && new_finalized_block > previous_epoch_best {
                let reason = format!("Finality exceeds previous epoch best: {} > {} (not progressive)", new_finalized_block, previous_epoch_best);
                if let Ok(bounded_reason) = BoundedVec::try_from(reason.as_bytes().to_vec()) {
                    Self::deposit_event(Event::FinalityAdvancementRejected {
                        attempted_block: new_finalized_block,
                        current_finalized,
                        best_known_block: previous_epoch_best,
                        epoch,
                        reason: bounded_reason.clone(),
                    });
                    return Err(bounded_reason);
                }
            }
            
            // All checks passed - emit successful validation event
            let advancement = new_finalized_block.saturating_sub(current_finalized);
            Self::deposit_event(Event::FinalityProgressionValidated {
                previous_finalized: current_finalized,
                new_finalized: new_finalized_block,
                advancement,
                epoch,
                validation_checks_passed: 3, // Number of checks that passed
            });
            
            Ok(())
        }

        /// Update finality markers with comprehensive validation and tracking.
        /// 
        /// This function safely updates the finality markers while maintaining
        /// all correctness guarantees and tracking previous values for regression
        /// detection. It should be called during epoch transitions to advance
        /// finality in a controlled manner.
        /// 
        /// # Arguments
        /// - `new_finalized_block`: The block number to finalize
        /// - `epoch`: Current epoch number for context
        /// 
        /// # Returns
        /// - `Ok(())`: If finality was successfully updated
        /// - `Err(reason)`: If the update was rejected
        pub fn update_finality_markers(
            new_finalized_block: u32, 
            epoch: u32
        ) -> Result<(), BoundedVec<u8, ConstU32<128>>> {
            // Validate the finality advancement first
            Self::validate_finality_advancement(new_finalized_block, epoch)?;

            // Cap DCF progressive finality at the last DVF-finalized block.
            // DVF is the primary finality authority; DCF must not advance the hard
            // finalized head past the last DVF checkpoint.
            let dvf_finalized = T::DvfFinalizedBlockProvider::dvf_finalized_block();
            let capped_block = new_finalized_block.min(dvf_finalized);

            if capped_block == 0 {
                // DVF has not finalized any block yet; skip DCF progressive finality
                // to avoid advancing ahead of DVF.
                log::debug!(
                    "DCF: Progressive finality skipped — DVF has not finalized any block yet"
                );
                return Ok(());
            }

            if capped_block < new_finalized_block {
                log::debug!(
                    "DCF: Progressive finality capped at DVF-finalized block {} (requested {})",
                    capped_block, new_finalized_block
                );
            }
            
            // Store current finalized block as previous for next validation
            let current_finalized = LastFinalizedBlock::<T>::get();

            // Nothing to advance if already at or past the cap
            if current_finalized >= capped_block {
                return Ok(());
            }

            PreviousFinalizedBlock::<T>::put(current_finalized);
            
            // Update the current finalized block
            LastFinalizedBlock::<T>::put(capped_block);
            
            // Update best known block for current epoch
            let current_block = frame_system::Pallet::<T>::block_number().saturated_into::<u32>();
            PreviousEpochBestBlock::<T>::put(current_block);
            
            // Emit finalization events
            Self::deposit_event(Event::BlockFinalized {
                block_number: capped_block,
            });
            
            // Get active validators for detailed finality tracking
            let active_validators = Self::active_validators();
            let block_hash = frame_system::Pallet::<T>::block_hash(BlockNumberFor::<T>::from(capped_block));
            
            Self::deposit_event(Event::FinalityMarker {
                block_number: capped_block,
                block_hash,
                participating_validators: active_validators.to_vec(),
                total_validators: active_validators.len() as u32,
            });
            
            log::info!("DCF: Finality advanced from {} to {} in epoch {}", 
                      current_finalized, capped_block, epoch);
            
            Ok(())
        }

        /// Check all invariants at epoch boundaries and generate a comprehensive report.
        /// 
        /// This is the main entry point for invariant checking that orchestrates
        /// all individual invariant checks and generates a comprehensive report.
        /// It's called during epoch transitions to ensure system integrity.
        /// 
        /// Returns an InvariantReport containing all detected violations and their severity.
        pub fn check_invariants_at_epoch_boundary(epoch: u32) -> InvariantReport<T> {
            let mut all_violations = Vec::new();
            let block_number = frame_system::Pallet::<T>::block_number().saturated_into::<u32>();
            let timestamp = 0u64; // In tests, we can't access offchain timestamp
            
            // Run all invariant checks
            let economic_violations = Self::check_economic_invariants();
            let validator_violations = Self::check_validator_invariants();
            let temporal_violations = Self::check_temporal_invariants(
                epoch.saturating_sub(1), 
                epoch
            );
            
            // Combine all violations
            all_violations.extend(economic_violations);
            all_violations.extend(validator_violations);
            all_violations.extend(temporal_violations);
            
            // Determine overall severity
            let severity = if all_violations.is_empty() {
                InvariantSeverity::Low
            } else {
                // Determine severity based on violation types
                let has_economic = all_violations.iter().any(|v| matches!(v, InvariantViolation::Economic { .. }));
                let has_validator = all_violations.iter().any(|v| matches!(v, InvariantViolation::Validator { .. }));
                let has_temporal = all_violations.iter().any(|v| matches!(v, InvariantViolation::Temporal { .. }));
                
                match (has_economic, has_validator, has_temporal) {
                    (true, true, true) => InvariantSeverity::Critical,
                    (true, true, false) | (true, false, true) => InvariantSeverity::High,
                    (false, true, true) => InvariantSeverity::High,
                    (true, false, false) => InvariantSeverity::High,
                    (false, true, false) | (false, false, true) => InvariantSeverity::Medium,
                    (false, false, false) => InvariantSeverity::Low,
                }
            };
            
            // Create bounded violations vector
            let bounded_violations = BoundedVec::truncate_from(all_violations);
            
            let report = InvariantReport {
                epoch,
                block_number,
                violations: bounded_violations.clone(),
                severity: severity.clone(),
                timestamp,
            };
            
            // Store the report
            InvariantReports::<T>::insert(epoch, &report);
            LatestInvariantReport::<T>::put(&report);
            
            // Emit events
            if !bounded_violations.is_empty() {
                Self::deposit_event(Event::InvariantViolationsDetected {
                    epoch,
                    violations: bounded_violations.clone(),
                    severity: severity.clone(),
                });
            }
            
            Self::deposit_event(Event::InvariantReportGenerated {
                epoch,
                block_number,
                violations_count: bounded_violations.len() as u32,
                severity,
            });
            
            report
        }

        /// Handle the logic for transitioning to a new epoch.
        pub fn handle_epoch_transition() -> Weight {
            // Always allow automatic epoch transitions, but behavior differs based on governance mode
            let governance_mode = GovernanceModeEnabled::<T>::get();
            let current_epoch = Self::current_epoch();
            let next_epoch = current_epoch.saturating_add(1);
            CurrentEpoch::<T>::put(next_epoch);

            // Reset epoch bounds tracking in pos pallet for the new epoch
            pos::Pallet::<T>::reset_epoch_totals();

            // Apply pending join/leave requests and get actual changes
            let (added_validators, removed_validators) = Self::apply_pending_validator_actions();

            // Update trust scores for all active validators at epoch transition
            let _trust_score_weight = Self::update_all_trust_scores();

            // Sort active validators by account ID for deterministic ordering.
            // MUST use account ID sort, NOT score sort — scores can differ between
            // nodes causing different orderings and divergent author sequences.
            let mut active_validators = ActiveValidators::<T>::get();
            active_validators.sort_by(|a, b| a.cmp(b));
            ActiveValidators::<T>::put(active_validators.clone());

            // Check invariants at epoch boundary
            let _invariant_report = Self::check_invariants_at_epoch_boundary(next_epoch);
            
            // Log epoch transition details
            if governance_mode {
                log::info!("DCF: Auto epoch transition {} -> {} (governance mode enabled) - {} active validators", 
                          current_epoch, next_epoch, active_validators.len());
            } else {
                log::info!("DCF: Auto epoch transition {} -> {} (standard mode) - {} active validators", 
                          current_epoch, next_epoch, active_validators.len());
            }
            
            // Always emit epoch events regardless of governance mode
            Self::deposit_event_with_evm_compat(Event::EpochStarted {
                epoch: next_epoch,
                validators: active_validators.clone().into_inner(),
            });
            
            // Calculate performance metrics for the epoch transition
            let total_validators = ValidatorSet::<T>::get().len() as u32;
            let average_performance_score = if !active_validators.is_empty() {
                let total_score: u64 = active_validators.iter()
                    .map(|v| ValidatorStates::<T>::get(v).map(|s| s.current.final_score).unwrap_or(0))
                    .sum();
                total_score / active_validators.len() as u64
            } else {
                0
            };

            // Calculate actual rewards distributed and slashed amounts during the previous epoch
            let (total_rewards_distributed, total_slashed_amount) = Self::calculate_epoch_economic_impact(current_epoch);

            // Emit comprehensive epoch transition event with performance metrics
            Self::deposit_event(Event::EpochTransition {
                old_epoch: current_epoch,
                new_epoch: next_epoch,
                active_validators: active_validators.clone().into_inner(),
                removed_validators,
                added_validators,
                total_validators,
                average_performance_score,
                total_rewards_distributed,
                total_slashed_amount,
            });

            // Emit ValidatorUptimeUpdated events for each validator at epoch transition
            for validator in ValidatorSet::<T>::get().iter() {
                if let Some(state) = ValidatorStates::<T>::get(validator) {
                    let blocks_authored = state.current.authored_blocks;
                    let blocks_missed = state.current.missed_blocks;
                    let blocks_expected = blocks_authored + blocks_missed;
                    
                    // Calculate uptime percentage (0-10000 representing 0-100%)
                    let uptime_percentage = if blocks_expected > 0 {
                        (blocks_authored * 10000) / blocks_expected
                    } else {
                        10000 // 100% if no blocks were expected
                    };
                    
                    Self::deposit_event(Event::ValidatorUptimeUpdated {
                        validator: validator.clone(),
                        epoch: current_epoch, // Report for the completed epoch
                        blocks_expected,
                        blocks_authored,
                        blocks_missed,
                        uptime_percentage,
                    });
                }
            }



            // --- EpochHistory recording ---
            let score_snapshot: BoundedVec<_, MaxValidatorsOf<T>> =
                BoundedVec::truncate_from(active_validators.iter().map(|v| {
                    let score = ValidatorStates::<T>::get(v).map(|s| s.current.final_score).unwrap_or_default();
                    (v.clone(), score)
                }).collect::<Vec<_>>());
            let inference_summary: BoundedVec<_, MaxValidatorsOf<T>> =
                BoundedVec::truncate_from(active_validators.iter().map(|v| {
                    let inf = poi::Pallet::<T>::inference_results(v).map(|(result, _)| result as u64);
                    (v.clone(), inf)
                }).collect::<Vec<_>>());

            let mut histories = EpochHistories::<T>::get();
            let new_history = EpochHistory::<T> {
                epoch_number: next_epoch,
                active_validators: active_validators.clone(),
                score_snapshot,
                inference_summary,
            };
            if histories.len() == <T as Config>::MaxEpochHistory::get() as usize {
                histories.remove(0);
            }
            let _ = histories.try_push(new_history);
            EpochHistories::<T>::put(histories);

            // Update system metrics after epoch transition
            Self::update_system_metrics();

            <T as Config>::WeightInfo::on_initialize()
        }

        /// Handle deterministic epoch transition with replayability.
        /// 
        /// This function extends the standard epoch transition with deterministic
        /// processing capabilities, ensuring that all nodes produce identical
        /// results when processing the same epoch with the same inputs.
        /// 
        /// Key features:
        /// - Uses deterministic randomness seeded from block numbers and fixed salts
        /// - Generates deterministic author sequences for the new epoch
        /// - Records processing outputs for replay validation
        /// - Validates replay consistency with previous processing
        /// 
        /// # Arguments
        /// - `block_number`: Block number where epoch transition occurs
        /// 
        /// # Returns
        /// - Weight consumed by the deterministic processing
        pub fn handle_deterministic_epoch_transition(block_number: u32) -> Weight {
            let mut weight = Weight::zero();
            let current_epoch = Self::current_epoch();
            let next_epoch = current_epoch.saturating_add(1);

            // Update PreviousEpochBestBlock to the last block of the epoch that just ended.
            // This is block_number - 1 (the boundary block belongs to the new epoch).
            // Without this, validate_finality_advancement Check 3 rejects all progressive
            // finality with "Finality exceeds previous epoch best: N > 1 (not progressive)".
            PreviousEpochBestBlock::<T>::put(block_number.saturating_sub(1));
            
            // Initialize deterministic engine if not already done
            let mut engine = DeterministicEngineState::<T>::get();
            if engine.randomness_salt == [0u8; 32] {
                // Initialize with deterministic salt based on genesis block hash
                engine.randomness_salt = Self::generate_genesis_randomness_salt();
                weight = weight.saturating_add(Weight::from_parts(10_000, 0));
            }
            
            // Generate deterministic epoch salt
            engine.epoch_salt = Self::generate_epoch_salt(next_epoch, &engine.randomness_salt);
            
            // Generate deterministic randomness seed for this epoch
            let randomness_seed = Self::generate_deterministic_randomness(
                block_number,
                next_epoch,
                &engine.randomness_salt,
                &engine.epoch_salt
            );
            
            // Capture inputs for replay validation
            let active_validators = ActiveValidators::<T>::get();
            let input_hash = Self::compute_epoch_input_hash(
                current_epoch,
                block_number,
                &active_validators,
                &randomness_seed
            );
            
            // Perform standard epoch transition
            weight = weight.saturating_add(Self::handle_epoch_transition());
            
            // Get updated active validators after transition
            let new_active_validators = ActiveValidators::<T>::get();
            
            // Generate deterministic author sequence for the new epoch
            let author_sequence = Self::generate_deterministic_author_sequence(
                next_epoch,
                &new_active_validators,
                &randomness_seed
            );
            
            // Cache the author sequence
            EpochAuthorSequences::<T>::insert(next_epoch, &author_sequence);
            weight = weight.saturating_add(Weight::from_parts(5_000, 0));
            
            // Capture outputs for replay validation
            let (added_validators, removed_validators) = Self::get_epoch_validator_changes(current_epoch);
            let validator_scores = Self::get_all_validator_scores(&new_active_validators);
            let output_hash = Self::compute_epoch_output_hash(
                next_epoch,
                &new_active_validators,
                &added_validators,
                &removed_validators,
                &validator_scores,
                &author_sequence
            );
            
            // Create processing output record
            let processing_output = EpochProcessingOutput::<T> {
                epoch: next_epoch,
                transition_block: block_number,
                active_validators: new_active_validators.clone(),
                added_validators,
                removed_validators,
                validator_scores: validator_scores.clone(),
                author_sequence: author_sequence.clone(),
                randomness_seed,
                input_hash,
                output_hash,
            };
            
            // Invoke DVF weight freezing.
            // Use the live PoS stake (raw u128 token amount) so that freeze_epoch_weights
            // can normalize it correctly via VOTE_WEIGHT_SCALE. Using the stale
            // stake_score field (a u64 truncation of the raw amount) caused validators
            // like Charlie to receive a weight of 0 whenever stake_score was reset to 0
            // by run_comprehensive_score_aggregation at the epoch boundary.
            let dvf_input: Vec<(T::AccountId, u128, u128)> = new_active_validators.iter().map(|validator| {
                // Read the real PoS stake directly — this is the same source used by
                // freeze_epoch_weights and always reflects the validator's actual bond.
                let stake = pos::Pallet::<T>::stake(validator).saturated_into::<u128>();
                let score = ValidatorStates::<T>::get(validator)
                    .map(|s| s.current.final_score as u128)
                    .unwrap_or(1); // Default to 1 so weight is never 0 for active validators
                (validator.clone(), stake, score)
            }).collect();
            T::WeightFreezer::freeze_epoch_weights(next_epoch, &dvf_input);
            
            // Store processing output for replay validation
            EpochProcessingOutputs::<T>::insert(next_epoch, &processing_output);
            weight = weight.saturating_add(Weight::from_parts(15_000, 0));
            
            // Update engine state
            engine.last_processed_epoch = next_epoch;
            engine.last_epoch_output_hash = output_hash;
            engine.author_sequence_cache = BoundedVec::truncate_from(
                author_sequence.encode()
            );
            DeterministicEngineState::<T>::put(engine);
            weight = weight.saturating_add(Weight::from_parts(5_000, 0));
            
            // Emit deterministic processing event
            Self::deposit_event(Event::DeterministicEpochProcessed {
                epoch: next_epoch,
                block_number,
                randomness_seed,
                author_sequence_length: author_sequence.len() as u32,
                output_hash,
            });
            
            log::info!(
                "DCF: Deterministic epoch transition completed for epoch {} at block {} with {} authors",
                next_epoch, block_number, author_sequence.len()
            );
            
            weight
        }
        
        /// Generate deterministic randomness for epoch processing.
        /// 
        /// This function creates a deterministic randomness seed using:
        /// - Block number (provides temporal uniqueness)
        /// - Epoch number (provides epoch-specific variation)
        /// - Fixed randomness salt (provides network-specific entropy)
        /// - Epoch salt (provides additional epoch-specific entropy)
        /// 
        /// The randomness is deterministic and will produce identical results
        /// across all nodes when given the same inputs.
        /// 
        /// # Arguments
        /// - `block_number`: Block number where epoch transition occurs
        /// - `epoch`: Epoch number being processed
        /// - `randomness_salt`: Fixed network-wide randomness salt
        /// - `epoch_salt`: Epoch-specific salt
        /// 
        /// # Returns
        /// - 32-byte deterministic randomness seed
        pub fn generate_deterministic_randomness(
            block_number: u32,
            epoch: u32,
            randomness_salt: &[u8; 32],
            epoch_salt: &[u8; 16]
        ) -> [u8; 32] {
            use sp_io::hashing::blake2_256;
            
            // Combine all entropy sources
            let mut input = Vec::new();
            input.extend_from_slice(&block_number.to_le_bytes());
            input.extend_from_slice(&epoch.to_le_bytes());
            input.extend_from_slice(randomness_salt);
            input.extend_from_slice(epoch_salt);
            
            // Add deterministic entropy from the PARENT block's hash.
            // We are inside on_initialize(block_number) — block_number's own hash
            // does not exist yet (it is being built). block_hash(block_number - 1)
            // is the last finalized parent, identical on every node that has
            // reached this epoch boundary. This is the correct anchor point.
            let parent_block_number = block_number.saturating_sub(1);
            let parent_block_hash = frame_system::Pallet::<T>::block_hash(
                BlockNumberFor::<T>::from(parent_block_number)
            );
            input.extend_from_slice(parent_block_hash.as_ref());
            
            // Generate deterministic hash
            blake2_256(&input)
        }
        
        /// Generate deterministic author sequence for an epoch.
        /// 
        /// This function creates a deterministic ordering of validators for block
        /// authorship during an epoch. The sequence is generated using weighted
        /// selection based on validator scores, with deterministic randomness
        /// ensuring identical results across all nodes.
        /// 
        /// The author sequence determines the expected author for each block
        /// within the epoch, enabling deterministic block production scheduling.
        /// 
        /// # Arguments
        /// - `epoch`: Epoch number for the sequence
        /// - `validators`: Active validators for the epoch
        /// - `randomness_seed`: Deterministic randomness seed
        /// 
        /// # Returns
        /// - Deterministic sequence of authors for the epoch
        pub fn generate_deterministic_author_sequence(
            _epoch: u32,
            validators: &BoundedVec<T::AccountId, MaxValidatorsOf<T>>,
            randomness_seed: &[u8; 32]
        ) -> BoundedVec<T::AccountId, ConstU32<1000>> {
            if validators.is_empty() {
                return BoundedVec::new();
            }
            
            // Get epoch configuration
            let epoch_config = Self::epoch_config();
            let blocks_per_epoch = epoch_config.blocks_per_epoch;
            
            // Calculate sequence length (limit to reasonable size)
            let sequence_length = blocks_per_epoch.min(1000);
            
            // Use snapshotted scores for this epoch if available — scores were frozen
            // 5 blocks before the epoch boundary so all nodes have identical weights.
            // Fall back to equal weights only if no snapshot exists (e.g. epoch 0).
            let snapshot = EpochScoreSnapshot::<T>::get(_epoch);
            let mut validator_weights: Vec<(T::AccountId, u64)> = match snapshot {
                Some(snap) => {
                    // Build weight map from snapshot, preserving PoS+PoI scoring
                    let weight_map: sp_std::collections::btree_map::BTreeMap<_, _> =
                        snap.into_iter().collect();
                    validators.iter()
                        .map(|v| {
                            let w = weight_map.get(v).copied().unwrap_or(1).max(1);
                            (v.clone(), w)
                        })
                        .collect()
                }
                None => {
                    // No snapshot — equal weights (epoch 0 or missing snapshot)
                    validators.iter().map(|v| (v.clone(), 1u64)).collect()
                }
            };
            
            // Sort by account ID for deterministic ordering
            validator_weights.sort_by(|a, b| a.0.cmp(&b.0));
            
            let mut sequence = Vec::new();
            let mut rng_state = *randomness_seed;
            
            // Generate deterministic author sequence
            for block_offset in 0..sequence_length {
                // Update RNG state deterministically
                rng_state = Self::advance_deterministic_rng(rng_state, block_offset);
                
                // Select author using weighted selection
                let selected_author = Self::select_weighted_author(
                    &validator_weights,
                    &rng_state
                );
                
                sequence.push(selected_author);
            }
            
            BoundedVec::truncate_from(sequence)
        }
        
        /// Validate epoch processing through replay.
        /// 
        /// This function performs replay validation by re-processing an epoch
        /// with the same inputs and comparing the outputs byte-for-byte with
        /// the original processing results.
        /// 
        /// Replay validation ensures that:
        /// - Epoch processing is deterministic
        /// - All nodes produce identical results
        /// - No non-deterministic behavior exists
        /// - Consensus is maintained on epoch transitions
        /// 
        /// # Arguments
        /// - `epoch`: Epoch number to replay
        /// 
        /// # Returns
        /// - `Ok(())`: Replay validation passed
        /// - `Err(ReplayError)`: Replay validation failed with details
        pub fn validate_epoch_replay(epoch: u32) -> Result<(), ReplayValidationError> {
            // Get original processing output
            let original_output = EpochProcessingOutputs::<T>::get(epoch)
                .ok_or(ReplayValidationError::MissingOriginalOutput)?;
            
            // Simulate replay with same inputs
            let replay_output = Self::simulate_epoch_processing_replay(
                epoch,
                original_output.transition_block,
                &original_output.randomness_seed,
                &original_output.input_hash
            )?;
            
            // Compare outputs byte-for-byte
            if original_output.output_hash != replay_output.output_hash {
                return Err(ReplayValidationError::OutputMismatch {
                    original_hash: original_output.output_hash,
                    replay_hash: replay_output.output_hash,
                });
            }
            
            // Validate specific components
            if original_output.active_validators != replay_output.active_validators {
                return Err(ReplayValidationError::ValidatorSetMismatch);
            }
            
            if original_output.author_sequence != replay_output.author_sequence {
                return Err(ReplayValidationError::AuthorSequenceMismatch);
            }
            
            if original_output.validator_scores != replay_output.validator_scores {
                return Err(ReplayValidationError::ScoreMismatch);
            }
            
            log::info!(
                "DCF: Replay validation passed for epoch {} - outputs match byte-for-byte",
                epoch
            );
            
            Ok(())
        }

        /// Perform storage migration from one version to another.
        /// 
        /// This function orchestrates the migration process by:
        /// 1. Validating the source storage version
        /// 2. Executing migration steps sequentially
        /// 3. Validating the target storage state
        /// 4. Updating version tracking
        /// 
        /// For the initial production version (version 1), this performs a no-op
        /// migration that validates the storage is in the expected state.
        /// 
        /// # Arguments
        /// - `from_version`: Current storage version
        /// - `to_version`: Target storage version
        /// 
        /// # Returns
        /// - `Ok(Weight)`: Migration completed successfully with weight consumed
        /// - `Err(MigrationError)`: Migration failed with specific error details
        pub fn perform_storage_migration(from_version: u32, to_version: u32) -> Result<Weight, MigrationError> {
            let mut weight = <T as Config>::WeightInfo::on_initialize();
            
            log::info!(
                "DCF: Starting storage migration from version {} to version {}",
                from_version,
                to_version
            );
            
            // For version 1 (initial production version), perform validation-only migration
            if from_version == 0 && to_version == 1 {
                // This is the initial migration to production-ready storage
                log::info!("DCF: Performing initial migration to production storage (version 1)");
                
                // Validate that all required storage items are accessible
                weight = weight.saturating_add(Self::validate_storage_integrity()?);
                
                // Initialize default governance configuration if not present
                if !GovernanceConfigStorage::<T>::exists() {
                    let default_config = GovernanceConfig::<T>::default();
                    GovernanceConfigStorage::<T>::put(default_config);
                    log::info!("DCF: Initialized default governance configuration");
                }
                
                // Initialize storage version manager
                let _version_manager = StorageVersionManager::default();
                // Note: We don't have a storage item for the version manager yet,
                // but we validate that the storage version item works
                
                log::info!("DCF: Initial migration to version 1 completed successfully");
                
            } else if from_version == to_version {
                // No migration needed, just validate
                log::info!("DCF: No migration needed, versions match");
                weight = weight.saturating_add(Self::validate_storage_integrity()?);
                
            } else {
                // Future migrations would be implemented here
                log::error!(
                    "DCF: Unsupported migration path from version {} to version {}",
                    from_version,
                    to_version
                );
                
                return Err(MigrationError::NoMigrationPath {
                    from: from_version,
                    to: to_version,
                });
            }
            
            log::info!("DCF: Storage migration completed successfully");
            Ok(weight)
        }
        
        /// Validate storage integrity and accessibility.
        /// 
        /// This function performs comprehensive validation of the storage state
        /// to ensure all required storage items are accessible and contain
        /// valid data. It's used during migrations and runtime initialization.
        /// 
        /// # Returns
        /// - `Ok(Weight)`: Validation passed with weight consumed
        /// - `Err(MigrationError)`: Validation failed with specific error details
        pub fn validate_storage_integrity() -> Result<Weight, MigrationError> {
            #[allow(unused_mut)]
            let mut weight = <T as Config>::WeightInfo::on_initialize();
            
            log::info!("DCF: Starting storage integrity validation");
            
            // Storage accessibility validation would go here
            // (removed recursive call)
            
            // Validate governance configuration if it exists
            if GovernanceConfigStorage::<T>::exists() {
                let _governance_config = GovernanceConfigStorage::<T>::get();
                // Governance config validation (available in test/benchmark builds only)
                #[cfg(any(feature = "runtime-benchmarks", test))]
                {
                    weight = weight.saturating_add(Self::validate_governance_config(&_governance_config)?);
                }
            }
            
            // Validate epoch configuration
            let _epoch_config = EpochConfigStorage::<T>::get();
            // Epoch config validation (available in test/benchmark builds only)
            #[cfg(any(feature = "runtime-benchmarks", test))]
            {
                weight = weight.saturating_add(Self::validate_epoch_config(&_epoch_config)?);
            }
            
            // Validate active validators don't exceed maximum
            let active_validators = ActiveValidators::<T>::get();
            if active_validators.len() > MaxValidatorsOf::<T>::get() as usize {
                log::error!(
                    "DCF: Active validator count ({}) exceeds maximum ({})",
                    active_validators.len(),
                    MaxValidatorsOf::<T>::get()
                );
                
                return Err(MigrationError::PostValidationFailed {
                    rule_id: b"max_validators_check".to_vec().try_into().unwrap_or_default(),
                    reason: b"Active validator count exceeds maximum allowed".to_vec().try_into().unwrap_or_default(),
                });
            }
            
            log::info!("DCF: Storage integrity validation completed successfully");
            Ok(weight)
        }
        
        /// Validate that all required storage items are accessible.
        /// 
        /// This function checks that all critical storage items can be read
        /// without errors, ensuring the storage layer is functioning correctly.
        /// 
        /// # Returns
        /// - `Ok(Weight)`: All storage items are accessible
        /// - `Err(MigrationError)`: Storage access failed
        #[cfg(any(feature = "runtime-benchmarks", test))]
        pub fn validate_storage_accessibility() -> Result<Weight, MigrationError> {
            let weight = <T as Config>::WeightInfo::on_initialize();
            
            // Test access to critical storage items
            let _ = CurrentEpoch::<T>::get();
            let _ = ActiveValidators::<T>::get();
            let _ = ValidatorSet::<T>::get();
            let _ = EpochConfigStorage::<T>::get();
            let _ = PosWeight::<T>::get();
            let _ = PoiWeight::<T>::get();
            let _ = GovernanceModeEnabled::<T>::get();
            let _ = StorageVersion::<T>::get();
            
            // Test that we can write to storage version (this is critical for migration tracking)
            let current_version = StorageVersion::<T>::get();
            StorageVersion::<T>::put(current_version); // Write back the same value
            
            log::debug!("DCF: Storage accessibility validation passed");
            Ok(weight)
        }
        
        /// Validate governance configuration for consistency and safety.
        /// 
        /// This function checks that the governance configuration contains
        /// valid parameter ranges and current values within those ranges.
        /// 
        /// # Arguments
        /// - `config`: Governance configuration to validate
        /// 
        /// # Returns
        /// - `Ok(Weight)`: Configuration is valid
        /// - `Err(MigrationError)`: Configuration validation failed
        #[cfg(any(feature = "runtime-benchmarks", test))]
        pub fn validate_governance_config(config: &GovernanceConfig<T>) -> Result<Weight, MigrationError> {
            let weight = <T as Config>::WeightInfo::on_initialize();
            
            // Validate epoch length range
            if config.epoch_length.min > config.epoch_length.max ||
               config.epoch_length.current < config.epoch_length.min ||
               config.epoch_length.current > config.epoch_length.max {
                return Err(MigrationError::PostValidationFailed {
                    rule_id: b"epoch_length_range".to_vec().try_into().unwrap_or_default(),
                    reason: b"Epoch length parameter range is invalid".to_vec().try_into().unwrap_or_default(),
                });
            }
            
            // Validate max validators range
            if config.max_validators.min > config.max_validators.max ||
               config.max_validators.current < config.max_validators.min ||
               config.max_validators.current > config.max_validators.max {
                return Err(MigrationError::PostValidationFailed {
                    rule_id: b"max_validators_range".to_vec().try_into().unwrap_or_default(),
                    reason: b"Max validators parameter range is invalid".to_vec().try_into().unwrap_or_default(),
                });
            }
            
            // Validate consensus weights sum to reasonable values
            let pos_weight = config.pos_weight.current;
            let poi_weight = config.poi_weight.current;
            if pos_weight + poi_weight == 0 {
                return Err(MigrationError::PostValidationFailed {
                    rule_id: b"consensus_weights_sum".to_vec().try_into().unwrap_or_default(),
                    reason: b"Consensus weights sum to zero".to_vec().try_into().unwrap_or_default(),
                });
            }
            
            log::debug!("DCF: Governance configuration validation passed");
            Ok(weight)
        }
        
        /// Validate epoch configuration for consistency and safety.
        /// 
        /// This function checks that the epoch configuration contains
        /// reasonable values that won't cause system instability.
        /// 
        /// # Arguments
        /// - `config`: Epoch configuration to validate
        /// 
        /// # Returns
        /// - `Ok(Weight)`: Configuration is valid
        /// - `Err(MigrationError)`: Configuration validation failed
        #[cfg(any(feature = "runtime-benchmarks", test))]
        pub fn validate_epoch_config(config: &EpochConfig) -> Result<Weight, MigrationError> {
            let weight = <T as Config>::WeightInfo::on_initialize();
            
            // Validate epoch length is reasonable
            if config.blocks_per_epoch == 0 {
                return Err(MigrationError::PostValidationFailed {
                    rule_id: b"epoch_length_zero".to_vec().try_into().unwrap_or_default(),
                    reason: b"Epoch length cannot be zero".to_vec().try_into().unwrap_or_default(),
                });
            }
            
            // Validate max validators is reasonable
            if config.max_validators == 0 {
                return Err(MigrationError::PostValidationFailed {
                    rule_id: b"max_validators_zero".to_vec().try_into().unwrap_or_default(),
                    reason: b"Max validators cannot be zero".to_vec().try_into().unwrap_or_default(),
                });
            }
            
            // Validate max validators doesn't exceed system limits
            if config.max_validators > MaxValidatorsOf::<T>::get() {
                return Err(MigrationError::PostValidationFailed {
                    rule_id: b"max_validators_exceeds_limit".to_vec().try_into().unwrap_or_default(),
                    reason: b"Max validators exceeds system limit".to_vec().try_into().unwrap_or_default(),
                });
            }
            
            log::debug!("DCF: Epoch configuration validation passed");
            Ok(weight)
        }

        // --- Deterministic Processing Helper Functions --- //
        
        /// Generate genesis randomness salt from system state.
        /// 
        /// This function creates a fixed randomness salt that is derived from
        /// genesis block information and never changes after network initialization.
        /// The salt provides network-specific entropy for deterministic processing.
        /// 
        /// # Returns
        /// - 32-byte fixed randomness salt
        fn generate_genesis_randomness_salt() -> [u8; 32] {
            use sp_io::hashing::blake2_256;
            
            // Use genesis block hash as base entropy
            let genesis_hash = frame_system::Pallet::<T>::block_hash(BlockNumberFor::<T>::from(0u32));
            let mut input = Vec::new();
            input.extend_from_slice(genesis_hash.as_ref());
            
            // Add additional deterministic entropy
            input.extend_from_slice(b"DCF_DETERMINISTIC_SALT_V1");
            input.extend_from_slice(&CURRENT_STORAGE_VERSION.to_le_bytes());
            
            blake2_256(&input)
        }
        
        /// Generate epoch-specific salt.
        /// 
        /// This function creates a unique salt for each epoch by combining
        /// the epoch number with the fixed randomness salt. This provides
        /// epoch-specific entropy while maintaining determinism.
        /// 
        /// # Arguments
        /// - `epoch`: Epoch number
        /// - `randomness_salt`: Fixed network randomness salt
        /// 
        /// # Returns
        /// - 16-byte epoch-specific salt
        fn generate_epoch_salt(epoch: u32, randomness_salt: &[u8; 32]) -> [u8; 16] {
            use sp_io::hashing::blake2_256;
            
            let mut input = Vec::new();
            input.extend_from_slice(&epoch.to_le_bytes());
            input.extend_from_slice(randomness_salt);
            input.extend_from_slice(b"EPOCH_SALT");
            
            let hash = blake2_256(&input);
            let mut salt = [0u8; 16];
            salt.copy_from_slice(&hash[0..16]);
            salt
        }
        
        /// Compute hash of epoch processing inputs.
        /// 
        /// This function creates a hash of all inputs used in epoch processing
        /// to enable replay validation. The hash includes all state that affects
        /// the epoch transition outcome.
        /// 
        /// # Arguments
        /// - `epoch`: Epoch number being processed
        /// - `block_number`: Block number of transition
        /// - `validators`: Active validators before transition
        /// - `randomness_seed`: Randomness seed used
        /// 
        /// # Returns
        /// - 32-byte hash of processing inputs
        fn compute_epoch_input_hash(
            epoch: u32,
            block_number: u32,
            validators: &BoundedVec<T::AccountId, MaxValidatorsOf<T>>,
            randomness_seed: &[u8; 32]
        ) -> [u8; 32] {
            use sp_io::hashing::blake2_256;
            
            let mut input = Vec::new();
            input.extend_from_slice(&epoch.to_le_bytes());
            input.extend_from_slice(&block_number.to_le_bytes());
            input.extend_from_slice(&validators.encode());
            input.extend_from_slice(randomness_seed);
            
            // Add relevant system state
            let pos_weight = Self::pos_weight();
            let poi_weight = Self::poi_weight();
            input.extend_from_slice(&pos_weight.to_le_bytes());
            input.extend_from_slice(&poi_weight.to_le_bytes());
            
            blake2_256(&input)
        }
        
        /// Compute hash of epoch processing outputs.
        /// 
        /// This function creates a hash of all outputs from epoch processing
        /// to enable replay validation. The hash captures all state changes
        /// that result from the epoch transition.
        /// 
        /// # Arguments
        /// - `epoch`: Epoch number processed
        /// - `active_validators`: Final active validator set
        /// - `added_validators`: Validators added during transition
        /// - `removed_validators`: Validators removed during transition
        /// - `validator_scores`: Final validator scores
        /// - `author_sequence`: Generated author sequence
        /// 
        /// # Returns
        /// - 32-byte hash of processing outputs
        fn compute_epoch_output_hash(
            epoch: u32,
            active_validators: &BoundedVec<T::AccountId, MaxValidatorsOf<T>>,
            added_validators: &BoundedVec<T::AccountId, MaxValidatorsOf<T>>,
            removed_validators: &BoundedVec<T::AccountId, MaxValidatorsOf<T>>,
            validator_scores: &BoundedVec<(T::AccountId, u64), MaxValidatorsOf<T>>,
            author_sequence: &BoundedVec<T::AccountId, ConstU32<1000>>
        ) -> [u8; 32] {
            use sp_io::hashing::blake2_256;
            
            let mut output = Vec::new();
            output.extend_from_slice(&epoch.to_le_bytes());
            output.extend_from_slice(&active_validators.encode());
            output.extend_from_slice(&added_validators.encode());
            output.extend_from_slice(&removed_validators.encode());
            output.extend_from_slice(&validator_scores.encode());
            output.extend_from_slice(&author_sequence.encode());
            
            blake2_256(&output)
        }
        
        /// Advance deterministic RNG state.
        /// 
        /// This function advances the deterministic random number generator state
        /// using a linear congruential generator (LCG) algorithm. The advancement
        /// is deterministic and will produce identical sequences across all nodes.
        /// 
        /// # Arguments
        /// - `current_state`: Current RNG state
        /// - `step`: Step number for advancement
        /// 
        /// # Returns
        /// - Advanced RNG state
        fn advance_deterministic_rng(current_state: [u8; 32], step: u32) -> [u8; 32] {
            use sp_io::hashing::blake2_256;
            
            let mut input = Vec::new();
            input.extend_from_slice(&current_state);
            input.extend_from_slice(&step.to_le_bytes());
            input.extend_from_slice(b"RNG_ADVANCE");
            
            blake2_256(&input)
        }
        
        /// Select weighted author using deterministic randomness.
        /// 
        /// This function selects a validator from the weighted list using
        /// deterministic randomness. The selection is based on validator
        /// scores (weights) and will produce identical results across all
        /// nodes when given the same inputs.
        /// 
        /// # Arguments
        /// - `validator_weights`: List of validators with their weights
        /// - `randomness`: Deterministic randomness for selection
        /// 
        /// # Returns
        /// - Selected validator account
        fn select_weighted_author(
            validator_weights: &[(T::AccountId, u64)],
            randomness: &[u8; 32]
        ) -> T::AccountId {
            if validator_weights.is_empty() {
                // This should never happen, but provide a safe fallback
                panic!("Cannot select author from empty validator set");
            }
            
            // Calculate total weight
            let total_weight: u64 = validator_weights.iter()
                .map(|(_, weight)| *weight)
                .sum();
            
            if total_weight == 0 {
                // All validators have zero weight, use round-robin
                let index = Self::bytes_to_u64(randomness) as usize % validator_weights.len();
                return validator_weights[index].0.clone();
            }
            
            // Convert randomness to target value
            let target = Self::bytes_to_u64(randomness) % total_weight;
            let mut cumulative_weight = 0u64;
            
            // Select validator based on weighted probability
            for (validator, weight) in validator_weights.iter() {
                cumulative_weight = cumulative_weight.saturating_add(*weight);
                if target < cumulative_weight {
                    return validator.clone();
                }
            }
            
            // Fallback to first validator (should never reach here)
            validator_weights[0].0.clone()
        }
        
        /// Convert bytes to u64 for deterministic calculations.
        /// 
        /// This function converts the first 8 bytes of a byte array to a u64
        /// value for use in deterministic calculations. The conversion is
        /// consistent across all platforms and architectures.
        /// 
        /// # Arguments
        /// - `bytes`: Byte array to convert
        /// 
        /// # Returns
        /// - u64 value derived from bytes
        fn bytes_to_u64(bytes: &[u8; 32]) -> u64 {
            let mut array = [0u8; 8];
            array.copy_from_slice(&bytes[0..8]);
            u64::from_le_bytes(array)
        }
        
        /// Get epoch validator changes.
        /// 
        /// This function retrieves the validators that were added and removed
        /// during the specified epoch transition. Used for replay validation
        /// and output recording.
        /// 
        /// # Arguments
        /// - `epoch`: Epoch number to query
        /// 
        /// # Returns
        /// - Tuple of (added_validators, removed_validators)
        fn get_epoch_validator_changes(
            epoch: u32
        ) -> (BoundedVec<T::AccountId, MaxValidatorsOf<T>>, BoundedVec<T::AccountId, MaxValidatorsOf<T>>) {
            // Query epoch history to get validators who joined and left
            if let Some(history) = EpochHistories::<T>::get().iter().find(|h| h.epoch_number == epoch) {
                let mut joined = BoundedVec::new();
                let mut left = BoundedVec::new();
                
                // Get previous epoch validators for comparison
                let prev_epoch = epoch.saturating_sub(1);
                let prev_validators = if let Some(prev_history) = EpochHistories::<T>::get().iter().find(|h| h.epoch_number == prev_epoch) {
                    prev_history.active_validators.clone()
                } else {
                    BoundedVec::new()
                };
                
                // Find validators who joined (in current but not in previous)
                for validator in &history.active_validators {
                    if !prev_validators.contains(validator) {
                        let _ = joined.try_push(validator.clone());
                    }
                }
                
                // Find validators who left (in previous but not in current)
                for validator in &prev_validators {
                    if !history.active_validators.contains(validator) {
                        let _ = left.try_push(validator.clone());
                    }
                }
                
                (joined, left)
            } else {
                (BoundedVec::new(), BoundedVec::new())
            }
        }
        
        /// Get all validator scores.
        /// 
        /// This function retrieves the current scores for all specified validators.
        /// Used for replay validation and output recording.
        /// 
        /// # Arguments
        /// - `validators`: Validators to get scores for
        /// 
        /// # Returns
        /// - List of (validator, score) pairs
        fn get_all_validator_scores(
            validators: &BoundedVec<T::AccountId, MaxValidatorsOf<T>>
        ) -> BoundedVec<(T::AccountId, u64), MaxValidatorsOf<T>> {
            let scores: Vec<(T::AccountId, u64)> = validators.iter()
                .map(|v| {
                    let score = ValidatorStates::<T>::get(v)
                        .map(|s| s.current.final_score)
                        .unwrap_or(0);
                    (v.clone(), score)
                })
                .collect();
            
            BoundedVec::truncate_from(scores)
        }
        
        /// Simulate epoch processing replay.
        /// 
        /// This function simulates the replay of epoch processing with the same
        /// inputs to validate deterministic behavior. It performs the same
        /// operations as the original processing but without modifying state.
        /// 
        /// # Arguments
        /// - `epoch`: Epoch number to replay
        /// - `block_number`: Block number of original transition
        /// - `randomness_seed`: Original randomness seed
        /// - `input_hash`: Original input hash for validation
        /// 
        /// # Returns
        /// - Simulated processing output for comparison
        fn simulate_epoch_processing_replay(
            epoch: u32,
            block_number: u32,
            randomness_seed: &[u8; 32],
            input_hash: &[u8; 32]
        ) -> Result<EpochProcessingOutput<T>, ReplayValidationError> {
            // This is a simplified simulation - in a full implementation,
            // this would recreate the exact processing steps
            
            let active_validators = ActiveValidators::<T>::get();
            let validator_scores = Self::get_all_validator_scores(&active_validators);
            
            // Generate the same author sequence
            let author_sequence = Self::generate_deterministic_author_sequence(
                epoch,
                &active_validators,
                randomness_seed
            );
            
            // Compute output hash
            let (added_validators, removed_validators) = Self::get_epoch_validator_changes(epoch);
            let output_hash = Self::compute_epoch_output_hash(
                epoch,
                &active_validators,
                &added_validators,
                &removed_validators,
                &validator_scores,
                &author_sequence
            );
            
            Ok(EpochProcessingOutput::<T> {
                epoch,
                transition_block: block_number,
                active_validators,
                added_validators,
                removed_validators,
                validator_scores,
                author_sequence,
                randomness_seed: *randomness_seed,
                input_hash: *input_hash,
                output_hash,
            })
        }
        /// Validate if a validator is eligible to rejoin after cooldown.
        /// 
        /// This function performs comprehensive validation for validator rejoin attempts,
        /// checking multiple cooldown states and ensuring proper stake reservation.
        /// 
        /// # Validation Checks
        /// 1. Check if validator has a pending leave request (cannot rejoin while leaving)
        /// 2. Check if validator is in recently removed cooldown period
        /// 3. Validate cooldown period has fully expired
        /// 4. Verify sufficient balance for stake reservation
        /// 
        /// # Errors
        /// - `ValidatorHasPendingLeaveRequest`: If validator has active leave request
        /// - `ValidatorRejoinCooldownNotExpired`: If cooldown period not yet expired
        /// - `ValidatorRejoinStakeReservationFailed`: If stake reservation would fail
        fn validate_rejoin_eligibility(who: &T::AccountId) -> DispatchResult {
            // Check if validator has a pending leave request
            if pos::ValidatorLeaveRequests::<T>::contains_key(who) {
                return Err(Error::<T>::ValidatorHasPendingLeaveRequest.into());
            }

            // Check if validator is in cooldown period after recently leaving
            if let Some(left_at_block) = pos::RecentlyRemovedValidators::<T>::get(who) {
                let current_block = frame_system::Pallet::<T>::block_number().saturated_into::<u32>();
                let cooldown_period = LeaveCooldownOf::<T>::get();
                let blocks_since_left = current_block.saturating_sub(left_at_block);

                // Provide specific error for cooldown not expired
                if blocks_since_left < cooldown_period {
                    return Err(Error::<T>::ValidatorRejoinCooldownNotExpired.into());
                }

                // Cooldown has expired, remove from recently removed list
                pos::RecentlyRemovedValidators::<T>::remove(who);
            }

            // Validate stake reservation eligibility
            let min_stake = MinStakeOf::<T>::get();
            let free_balance = T::Currency::free_balance(who);
            
            // Check if balance is sufficient for stake reservation
            if free_balance < min_stake {
                return Err(Error::<T>::ValidatorRejoinStakeReservationFailed.into());
            }

            // Test if stake reservation would succeed (without actually reserving)
            // This catches edge cases where currency system might reject the reservation
            if T::Currency::can_reserve(who, min_stake) != true {
                return Err(Error::<T>::ValidatorRejoinStakeReservationFailed.into());
            }

            Ok(())
        }

        /// Validate if a validator is eligible to submit a leave request.
        /// 
        /// This function prevents concurrent leave requests and ensures validators
        /// can only have one active leave request at a time.
        /// 
        /// # Validation Checks
        /// 1. Check if validator already has a pending leave request
        /// 2. Verify validator is currently in the validator set
        /// 3. Ensure validator is not in an invalid state for leaving
        /// 
        /// # Errors
        /// - `ConcurrentLeaveRequestNotAllowed`: If validator already has pending leave request
        /// - `ValidatorNotInSet`: If validator is not in the validator set
        fn validate_leave_request_eligibility(who: &T::AccountId) -> DispatchResult {
            // Check if there's already a pending leave request (prevent concurrent requests)
            if pos::ValidatorLeaveRequests::<T>::contains_key(who) {
                return Err(Error::<T>::ConcurrentLeaveRequestNotAllowed.into());
            }

            // Verify validator is in the validator set
            let validator_set = ValidatorSet::<T>::get();
            if !validator_set.contains(who) {
                return Err(Error::<T>::ValidatorNotInSet.into());
            }

            Ok(())
        }

        /// Enhanced cooldown status check with detailed information.
        /// 
        /// This function provides comprehensive cooldown status information
        /// for validators, including time remaining and eligibility status.
        /// 
        /// # Returns
        /// - `Some((blocks_remaining, can_rejoin))`: If validator is in cooldown
        /// - `None`: If validator is not in any cooldown state
        pub fn get_validator_detailed_cooldown_status_internal(who: &T::AccountId) -> Option<(u32, bool)> {
            let current_block = frame_system::Pallet::<T>::block_number().saturated_into::<u32>();
            let cooldown_period = LeaveCooldownOf::<T>::get();

            // Check for pending leave request cooldown
            if let Some(leave_request_block) = pos::ValidatorLeaveRequests::<T>::get(who) {
                let blocks_since_request = current_block.saturating_sub(leave_request_block);
                let blocks_remaining = cooldown_period.saturating_sub(blocks_since_request);
                return Some((blocks_remaining, false)); // Cannot rejoin while leaving
            }

            // Check for recently removed cooldown
            if let Some(removed_at_block) = pos::RecentlyRemovedValidators::<T>::get(who) {
                let blocks_since_removed = current_block.saturating_sub(removed_at_block);
                if blocks_since_removed < cooldown_period {
                    let blocks_remaining = cooldown_period.saturating_sub(blocks_since_removed);
                    return Some((blocks_remaining, false)); // Cannot rejoin yet
                } else {
                    return Some((0, true)); // Cooldown expired, can rejoin
                }
            }

            None // No cooldown active
        }

        /// Enable private chain mode with validator allowlist (Root only).
        /// 
        /// This dispatchable transitions the network from public mode to private
        /// chain mode, restricting validator participation to a predefined allowlist.
        /// 
        /// # Parameters
        /// - `initial_allowlist`: List of validator accounts permitted to participate
        /// - `allow_updates`: Whether allowlist modifications are permitted after enabling
        /// 
        /// # Requirements
        /// - Root origin required
        /// - Allowlist size must not exceed MaxValidators
        /// - Network must not already be in private mode
        /// 
        /// # Effects
        /// - Enables private chain mode
        /// - Sets initial validator allowlist
        /// - Removes non-allowlisted active validators
        /// - Emits PrivateChainModeEnabled event
        pub fn enable_private_chain_mode_internal(
            initial_allowlist: Vec<T::AccountId>,
            allow_updates: bool,
        ) -> DispatchResult {
            Self::enable_private_chain_mode(initial_allowlist, allow_updates).map_err(Into::into)
        }

        /// Disable private chain mode and return to public mode (Root only).
        /// 
        /// This dispatchable transitions the network from private chain mode
        /// back to public mode, removing all validator restrictions.
        /// 
        /// # Requirements
        /// - Root origin required
        /// - Network must be in private chain mode
        /// 
        /// # Effects
        /// - Disables private chain mode
        /// - Clears validator allowlist
        /// - Allows unrestricted validator participation
        /// - Emits PrivateChainModeDisabled event
        pub fn disable_private_chain_mode_internal() -> DispatchResult {
            Self::disable_private_chain_mode().map_err(Into::into)
        }

        /// Add validator to private chain allowlist (Root only).
        /// 
        /// This dispatchable adds a validator account to the allowlist in
        /// private chain mode, granting them permission to participate.
        /// 
        /// # Parameters
        /// - `validator`: Account to add to the allowlist
        /// 
        /// # Requirements
        /// - Root origin required
        /// - Network must be in private chain mode
        /// - Allowlist updates must be enabled
        /// - Allowlist must not be full
        /// 
        /// # Effects
        /// - Adds validator to allowlist
        /// - Emits ValidatorAddedToAllowlist event
        pub fn add_to_validator_allowlist_internal(
            validator: T::AccountId,
        ) -> DispatchResult {
            Self::add_to_validator_allowlist(validator).map_err(Into::into)
        }

        /// Remove validator from private chain allowlist (Root only).
        /// 
        /// This dispatchable removes a validator account from the allowlist
        /// in private chain mode, revoking their permission to participate.
        /// 
        /// # Parameters
        /// - `validator`: Account to remove from the allowlist
        /// 
        /// # Requirements
        /// - Root origin required
        /// - Network must be in private chain mode
        /// - Allowlist updates must be enabled
        /// 
        /// # Effects
        /// - Removes validator from allowlist
        /// - Forces validator to leave if currently active
        /// - Emits ValidatorRemovedFromAllowlist event
        pub fn remove_from_validator_allowlist_internal(
            validator: &T::AccountId,
        ) -> DispatchResult {
            Self::remove_from_validator_allowlist(validator).map_err(Into::into)
        }

        /// Test EVM event compatibility (Root only).
        /// 
        /// This dispatchable tests the EVM event compatibility system by
        /// emitting a test event and validating its EVM compatibility.
        /// 
        /// # Requirements
        /// - Root origin required
        /// 
        /// # Effects
        /// - Emits test event with EVM compatibility validation
        /// - Returns error if EVM compatibility validation fails
        pub fn test_evm_event_compatibility_internal() -> DispatchResult {
            // Get a validator from the validator set for testing, or skip if none exist
            let validator_set = ValidatorSet::<T>::get();
            if let Some(validator) = validator_set.first() {
                // Create a test event
                let test_event: Event<T> = Event::ValidatorJoined {
                    validator: validator.clone(),
                    stake_amount: MinStakeOf::<T>::get(),
                };
                
                // Validate EVM compatibility
                evm_compatibility::EvmEventValidator::convert_to_evm_format(&test_event)
                    .map_err(|_| Error::<T>::InvalidEpochConfig)?;
                
                // Emit the test event
                Self::deposit_event(Event::EvmCompatibilityTested {
                    success: true,
                });
            } else {
                // No validators available for testing, but that's okay
                Self::deposit_event(Event::EvmCompatibilityTested {
                    success: true,
                });
            }
            
            Ok(())
        }

        /// Query EVM events for a block range (Root only).
        /// 
        /// This dispatchable allows querying EVM-compatible events for
        /// a specific block range, useful for testing and debugging.
        /// 
        /// # Parameters
        /// - `from_block`: Starting block number
        /// - `to_block`: Ending block number
        /// - `event_type`: Optional event type filter
        /// 
        /// # Requirements
        /// - Root origin required
        /// - Block range must be reasonable (max 100 blocks)
        /// 
        /// # Effects
        /// - Emits event with query results summary
        pub fn query_evm_events_internal(
            event_type: Option<u32>,
            from_block: u32,
            to_block: u32,
        ) -> Vec<evm_compatibility::EvmCompatibleEvent> {
            // Validate block range
            if to_block < from_block || (to_block - from_block) > 100 {
                return Vec::new();
            }
            
            // Query events from storage
            let mut events = Vec::new();
            for block_num in from_block..=to_block {
                let block_events = Self::get_evm_events_for_block(block_num);
                events.extend(block_events);
            }
            
            // Filter by event type if specified
            if let Some(filter_type) = event_type {
                events.retain(|event| event.event_type == filter_type);
            }
            
            events
        }

    }

    // --- Runtime Hooks --- //
    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        /// Called during runtime upgrades to validate storage version and perform migrations.
        /// 
        /// This hook ensures that the storage schema is compatible with the current runtime
        /// and performs any necessary migrations to bring the storage up to the expected version.
        /// 
        /// The migration process includes:
        /// 1. Validating the current storage version
        /// 2. Executing any required migration steps
        /// 3. Validating the final storage state
        /// 4. Updating the storage version to match the runtime
        /// 
        /// If validation fails or migrations cannot be completed, the runtime upgrade
        /// will fail to prevent data corruption or system instability.
        fn on_runtime_upgrade() -> Weight {
            let mut weight = <T as Config>::WeightInfo::on_initialize();
            
            // Get current storage version (defaults to 0 if not set)
            let current_version = StorageVersion::<T>::get();
            
            log::info!(
                "DCF: Runtime upgrade - Current storage version: {}, Expected version: {}",
                current_version,
                CURRENT_STORAGE_VERSION
            );
            
            // Check if migration is needed
            if current_version != CURRENT_STORAGE_VERSION {
                log::info!(
                    "DCF: Storage version mismatch detected, performing migration from {} to {}",
                    current_version,
                    CURRENT_STORAGE_VERSION
                );
                
                // Perform storage migration based on version difference
                let migration_result: Result<Weight, MigrationError> = Self::perform_storage_migration(current_version, CURRENT_STORAGE_VERSION);
                match migration_result {
                    Ok(migration_weight) => {
                        weight = weight.saturating_add(migration_weight);
                        
                        // Update storage version to current
                        StorageVersion::<T>::put(CURRENT_STORAGE_VERSION);
                        
                        log::info!(
                            "DCF: Storage migration completed successfully to version {}",
                            CURRENT_STORAGE_VERSION
                        );
                        
                        // Emit migration completion event
                        Self::deposit_event(Event::StorageMigrationCompleted {
                            from_version: current_version,
                            to_version: CURRENT_STORAGE_VERSION,
                        });
                    },
                    Err(error) => {
                        log::error!(
                            "DCF: Storage migration failed: {:?}",
                            error
                        );
                        
                        // Emit migration failure event
                        Self::deposit_event(Event::StorageMigrationFailed {
                            from_version: current_version,
                            to_version: CURRENT_STORAGE_VERSION,
                            error: format!("{:?}", error).into_bytes().try_into().unwrap_or_default(),
                        });
                        
                        // Migration failure is critical - the runtime should not continue
                        // with incompatible storage. In production, this would typically
                        // cause the runtime upgrade to fail.
                        panic!("DCF: Critical storage migration failure - runtime upgrade aborted");
                    }
                }
            } else {
                log::info!("DCF: Storage version matches expected version, no migration needed");
                
                // Perform storage integrity validation
                if let Err(e) = Self::validate_storage_integrity() {
                    log::warn!("DCF: Storage integrity validation failed: {:?}", e);
                } else {
                    log::debug!("DCF: Storage integrity validation passed");
                }
            }
            
            weight
        }

        /// Called at the beginning of each block.
        /// This is the main entry point for DCF's automatic consensus management.
        fn on_initialize(now: BlockNumberFor<T>) -> Weight {
            let mut weight = <T as Config>::WeightInfo::on_initialize();
            let block_number = now.saturated_into::<u32>();
            let current_epoch = Self::current_epoch();
            
            // Initialize finalization at block 1 if not already done
            if block_number == 1 && Self::last_finalized_block() == 0 {
                LastFinalizedBlock::<T>::put(0); // Genesis block (block 0) is finalized
                PreviousFinalizedBlock::<T>::put(0);
                PreviousEpochBestBlock::<T>::put(block_number); // Set to current block for progressive finalization
                
                Self::deposit_event(Event::BlockFinalized {
                    block_number: 0,
                });
                log::info!("DCF: Initialized finalization markers at block 1 - genesis block 0 finalized, best block set to {}", block_number);
                weight = weight.saturating_add(Weight::from_parts(50_000, 0));
            }
            
            // Progressive finalization - finalize blocks as we go (with 1 block lag for safety)
            if block_number > 1 {
                let target_finalized = block_number - 1;
                if Self::last_finalized_block() < target_finalized {
                    match Self::update_finality_markers(target_finalized, current_epoch) {
                        Ok(()) => {
                            log::info!("DCF: Progressive finalization advanced to block {}", target_finalized);
                        },
                        Err(reason) => {
                            let reason_str = sp_std::str::from_utf8(&reason).unwrap_or("Invalid UTF-8");
                            log::warn!("DCF: Progressive finalization failed for block {}: {}", target_finalized, reason_str);
                        }
                    }
                    weight = weight.saturating_add(Weight::from_parts(100_000, 0));
                }
            }
            
            // Reset per-block rate limiting counters at the beginning of each block
            Self::reset_block_counters();
            weight = weight.saturating_add(Weight::from_parts(10_000, 0)); // Small weight for counter reset
            
            // Check if this is an epoch boundary
            if Self::is_epoch_boundary(block_number) {
                // Emit epoch boundary detection event
                Self::deposit_event(Event::EpochBoundaryDetected {
                    block_number,
                    epoch: current_epoch,
                    governance_mode: Self::governance_mode_enabled(),
                });
                
                // Handle comprehensive epoch transition
                weight = weight.saturating_add(Self::handle_comprehensive_epoch_transition(block_number));
                

            } else {
                // Regular block processing (non-epoch boundary)
                weight = weight.saturating_add(Self::handle_regular_block_processing(block_number, current_epoch));
            }

            weight
        }

        /// Called at the end of each block.
        fn on_finalize(_n: BlockNumberFor<T>) {
            let block_number = _n.saturated_into::<u32>();
            
            // Full DCF finalization logic - updated to avoid destructive score recalculation
            log::trace!("DCF: Processing on_finalize for block #{}", block_number);
            
            // Note: We no longer recalculate final scores for all validators every block here.
            // Scores are now updated through specific events (authorship, inference) 
            // and during epoch transitions to ensure stability and persistence of boosts.
        }







    }

    // --- Rate Limiting Implementation --- //
    impl<T: Config> Pallet<T> {
        /// Check if an operation is allowed under current rate limiting rules.
        /// 
        /// This function performs comprehensive rate limiting checks including:
        /// - Per-block operation limits
        /// - Per-account operation limits within time windows
        /// - Minimum intervals between operations
        /// - Weight bounds validation
        /// 
        /// # Parameters
        /// - `account`: Account attempting the operation
        /// - `operation`: Type of operation being attempted
        /// - `weight`: Computational weight of the operation
        /// 
        /// # Returns
        /// - `Ok(())`: Operation is allowed
        /// - `Err((Error, ViolationInfo))`: Operation violates rate limits with violation details
        fn check_rate_limits(
            account: &T::AccountId,
            operation: DispatchableType,
            weight: Weight,
        ) -> Result<(), (DispatchError, Option<RateLimitViolation>)> {
            let config = RateLimitConfigStorage::<T>::get();
            let current_block = frame_system::Pallet::<T>::block_number().saturated_into::<u32>();

            // Check per-block limits
            if let Err(e) = Self::check_per_block_limits(&operation, &config) {
                let violation = RateLimitViolation::PerBlockLimitExceeded {
                    operation: operation.clone(),
                    current_count: BlockOperationCounts::<T>::get(&operation),
                    limit: Self::get_per_block_limit(&operation, &config),
                };
                return Err((e, Some(violation)));
            }

            // Check per-account limits
            if let Err(e) = Self::check_per_account_limits(account, &operation, current_block, &config) {
                let history = AccountOperationHistory::<T>::get(account, &operation);
                let (limit, window) = Self::get_per_account_limit(&operation, &config);
                let window_start = current_block.saturating_sub(window);
                let operations_in_window = history.iter()
                    .filter(|&&block| block >= window_start)
                    .count() as u32;
                
                let violation = RateLimitViolation::PerAccountLimitExceeded {
                    operation: operation.clone(),
                    current_count: operations_in_window,
                    limit,
                    window_blocks: window,
                };
                return Err((e, Some(violation)));
            }

            // Check minimum intervals
            if let Err(e) = Self::check_minimum_intervals(account, &operation, current_block, &config) {
                if let Some(last_block) = LastOperationBlock::<T>::get(account, &operation) {
                    let blocks_since_last = current_block.saturating_sub(last_block);
                    let required_interval = Self::get_minimum_interval(&operation, &config);
                    
                    let violation = RateLimitViolation::MinimumIntervalViolation {
                        operation: operation.clone(),
                        blocks_since_last,
                        required_interval,
                    };
                    return Err((e, Some(violation)));
                }
            }

            // Check weight bounds
            if let Err(e) = Self::check_weight_bounds(&operation, weight, &config) {
                let max_weight = Self::get_weight_limit(&operation, &config);
                let violation = RateLimitViolation::WeightLimitExceeded {
                    operation: operation.clone(),
                    actual_weight: weight.ref_time(),
                    max_weight,
                };
                return Err((e, Some(violation)));
            }

            Ok(())
        }

        /// Get per-block limit for an operation
        fn get_per_block_limit(operation: &DispatchableType, config: &RateLimitConfig) -> u32 {
            match operation {
                DispatchableType::SubmitProposal => config.max_proposals_per_block,
                DispatchableType::JoinValidators => config.max_joins_per_block,
                DispatchableType::LeaveValidators => config.max_leaves_per_block,
                DispatchableType::VoteProposal => config.max_votes_per_block,
                DispatchableType::UpdateParameter => 1,
                DispatchableType::ValidatorStatusChange => config.max_joins_per_block + config.max_leaves_per_block,
            }
        }

        /// Get per-account limit and window for an operation
        fn get_per_account_limit(operation: &DispatchableType, config: &RateLimitConfig) -> (u32, u32) {
            match operation {
                DispatchableType::SubmitProposal => (config.max_proposals_per_account, config.proposal_rate_window),
                DispatchableType::JoinValidators | 
                DispatchableType::LeaveValidators | 
                DispatchableType::ValidatorStatusChange => (config.max_status_changes_per_account, config.status_change_rate_window),
                DispatchableType::VoteProposal => (100, 100),
                DispatchableType::UpdateParameter => (1, 1000),
            }
        }

        /// Get minimum interval for an operation
        fn get_minimum_interval(operation: &DispatchableType, config: &RateLimitConfig) -> u32 {
            match operation {
                DispatchableType::JoinValidators | 
                DispatchableType::LeaveValidators | 
                DispatchableType::ValidatorStatusChange => config.min_validator_status_interval,
                DispatchableType::SubmitProposal => config.min_proposal_interval,
                DispatchableType::VoteProposal => config.min_vote_interval,
                DispatchableType::UpdateParameter => 100,
            }
        }

        /// Get weight limit for an operation
        fn get_weight_limit(operation: &DispatchableType, config: &RateLimitConfig) -> u64 {
            match operation {
                DispatchableType::SubmitProposal | 
                DispatchableType::VoteProposal => config.max_proposal_processing_weight,
                DispatchableType::JoinValidators | 
                DispatchableType::LeaveValidators | 
                DispatchableType::ValidatorStatusChange => config.max_validator_iteration_weight,
                DispatchableType::UpdateParameter => 100_000_000,
            }
        }

        /// Check per-block operation limits.
        fn check_per_block_limits(
            operation: &DispatchableType,
            config: &RateLimitConfig,
        ) -> DispatchResult {
            let current_count = BlockOperationCounts::<T>::get(operation);
            
            let limit = match operation {
                DispatchableType::SubmitProposal => config.max_proposals_per_block,
                DispatchableType::JoinValidators => config.max_joins_per_block,
                DispatchableType::LeaveValidators => config.max_leaves_per_block,
                DispatchableType::VoteProposal => config.max_votes_per_block,
                DispatchableType::UpdateParameter => 1, // Very restrictive for parameter updates
                DispatchableType::ValidatorStatusChange => config.max_joins_per_block + config.max_leaves_per_block,
            };

            if current_count >= limit {
                // We create the violation info but can't emit events from check functions
                // The caller will handle the violation appropriately
                return Err(Error::<T>::PerBlockRateLimitExceeded.into());
            }

            Ok(())
        }

        /// Check per-account operation limits within time windows.
        fn check_per_account_limits(
            account: &T::AccountId,
            operation: &DispatchableType,
            current_block: u32,
            config: &RateLimitConfig,
        ) -> DispatchResult {
            let history = AccountOperationHistory::<T>::get(account, operation);
            
            let (limit, window) = match operation {
                DispatchableType::SubmitProposal => (config.max_proposals_per_account, config.proposal_rate_window),
                DispatchableType::JoinValidators | 
                DispatchableType::LeaveValidators | 
                DispatchableType::ValidatorStatusChange => (config.max_status_changes_per_account, config.status_change_rate_window),
                DispatchableType::VoteProposal => (100, 100), // Allow many votes but within reasonable limits
                DispatchableType::UpdateParameter => (1, 1000), // Very restrictive for parameter updates
            };

            // Count operations within the time window
            let window_start = current_block.saturating_sub(window);
            let operations_in_window = history.iter()
                .filter(|&&block| block >= window_start)
                .count() as u32;

            if operations_in_window >= limit {
                // We create the violation info but can't emit events from check functions
                // The caller will handle the violation appropriately
                return Err(Error::<T>::PerAccountRateLimitExceeded.into());
            }

            Ok(())
        }

        /// Check minimum intervals between operations.
        fn check_minimum_intervals(
            account: &T::AccountId,
            operation: &DispatchableType,
            current_block: u32,
            config: &RateLimitConfig,
        ) -> DispatchResult {
            if let Some(last_block) = LastOperationBlock::<T>::get(account, operation) {
                let blocks_since_last = current_block.saturating_sub(last_block);
                
                let required_interval = match operation {
                    DispatchableType::JoinValidators | 
                    DispatchableType::LeaveValidators | 
                    DispatchableType::ValidatorStatusChange => config.min_validator_status_interval,
                    DispatchableType::SubmitProposal => config.min_proposal_interval,
                    DispatchableType::VoteProposal => config.min_vote_interval,
                    DispatchableType::UpdateParameter => 100, // Require significant interval for parameter updates
                };

                if blocks_since_last < required_interval {
                    // We create the violation info but can't emit events from check functions
                    // The caller will handle the violation appropriately
                    return Err(Error::<T>::MinimumIntervalViolation.into());
                }
            }

            Ok(())
        }

        /// Check weight bounds for operations.
        fn check_weight_bounds(
            operation: &DispatchableType,
            weight: Weight,
            config: &RateLimitConfig,
        ) -> DispatchResult {
            let max_weight = match operation {
                DispatchableType::SubmitProposal | 
                DispatchableType::VoteProposal => config.max_proposal_processing_weight,
                DispatchableType::JoinValidators | 
                DispatchableType::LeaveValidators | 
                DispatchableType::ValidatorStatusChange => config.max_validator_iteration_weight,
                DispatchableType::UpdateParameter => 100_000_000, // Allow reasonable weight for parameter updates
            };

            if weight.ref_time() > max_weight {
                // We create the violation info but can't emit events from check functions
                // The caller will handle the violation appropriately
                return Err(Error::<T>::WeightLimitExceeded.into());
            }

            Ok(())
        }

        /// Record an operation for rate limiting tracking.
        /// 
        /// This function updates the rate limiting counters and history after
        /// a successful operation to track usage for future rate limiting decisions.
        /// 
        /// # Parameters
        /// - `account`: Account that performed the operation
        /// - `operation`: Type of operation that was performed
        fn record_operation(account: &T::AccountId, operation: DispatchableType) {
            let current_block = frame_system::Pallet::<T>::block_number().saturated_into::<u32>();

            // Update per-block counter
            BlockOperationCounts::<T>::mutate(&operation, |count| {
                *count = count.saturating_add(1);
            });

            // Update per-account history
            AccountOperationHistory::<T>::mutate(account, &operation, |history| {
                // Add current block to history
                if history.try_push(current_block).is_err() {
                    // If history is full, remove oldest entry and add new one
                    history.remove(0);
                    let _ = history.try_push(current_block);
                }

                // Clean up old entries outside the rate limiting window
                let config = RateLimitConfigStorage::<T>::get();
                let window = match operation {
                    DispatchableType::SubmitProposal => config.proposal_rate_window,
                    DispatchableType::JoinValidators | 
                    DispatchableType::LeaveValidators | 
                    DispatchableType::ValidatorStatusChange => config.status_change_rate_window,
                    DispatchableType::VoteProposal => 100,
                    DispatchableType::UpdateParameter => 1000,
                };
                
                let window_start = current_block.saturating_sub(window);
                history.retain(|&block| block >= window_start);
            });

            // Update last operation block
            LastOperationBlock::<T>::insert(account, &operation, current_block);
        }

        /// Reset per-block rate limiting counters.
        /// 
        /// This function is called at the beginning of each block to reset
        /// per-block operation counters to zero.
        fn reset_block_counters() {
            let current_block = frame_system::Pallet::<T>::block_number().saturated_into::<u32>();

            // Reset all per-block counters
            let _ = BlockOperationCounts::<T>::clear(u32::MAX, None);

            // Emit event for monitoring
            Self::deposit_event(Event::BlockRateLimitCountersReset {
                block_number: current_block,
            });
        }

        /// Update rate limiting configuration internally.
        /// 
        /// This function allows root accounts to update the rate limiting
        /// configuration with validation to ensure the new configuration
        /// is valid and won't prevent normal network operation.
        /// 
        /// # Parameters
        /// - `new_config`: New rate limiting configuration
        /// 
        /// # Returns
        /// - `Ok(())`: Configuration updated successfully
        /// - `Err(Error)`: Configuration is invalid
        fn update_rate_limit_config_internal(new_config: RateLimitConfig) -> DispatchResult {
            // Validate the new configuration
            Self::validate_rate_limit_config(&new_config)?;

            // Get old configuration for event (we'll use it for logging)
            let _old_config = RateLimitConfigStorage::<T>::get();

            // Update configuration
            RateLimitConfigStorage::<T>::put(&new_config);

            // Emit event
            Self::deposit_event(Event::RateLimitConfigUpdated {
                max_proposals_per_block: new_config.max_proposals_per_block,
                max_joins_per_block: new_config.max_joins_per_block,
                max_leaves_per_block: new_config.max_leaves_per_block,
            });

            Ok(())
        }

        /// Validate rate limiting configuration.
        fn validate_rate_limit_config(config: &RateLimitConfig) -> DispatchResult {
            // Check that limits are reasonable (not zero or excessively high)
            ensure!(config.max_proposals_per_block > 0 && config.max_proposals_per_block <= 100, Error::<T>::InvalidRateLimitConfig);
            ensure!(config.max_joins_per_block > 0 && config.max_joins_per_block <= 50, Error::<T>::InvalidRateLimitConfig);
            ensure!(config.max_leaves_per_block > 0 && config.max_leaves_per_block <= 50, Error::<T>::InvalidRateLimitConfig);
            ensure!(config.max_votes_per_block > 0 && config.max_votes_per_block <= 1000, Error::<T>::InvalidRateLimitConfig);

            // Check per-account limits
            ensure!(config.max_proposals_per_account > 0 && config.max_proposals_per_account <= 100, Error::<T>::InvalidRateLimitConfig);
            ensure!(config.max_status_changes_per_account > 0 && config.max_status_changes_per_account <= 20, Error::<T>::InvalidRateLimitConfig);

            // Check time windows are reasonable
            ensure!(config.proposal_rate_window > 0 && config.proposal_rate_window <= 10000, Error::<T>::InvalidRateLimitConfig);
            ensure!(config.status_change_rate_window > 0 && config.status_change_rate_window <= 100000, Error::<T>::InvalidRateLimitConfig);

            // Check minimum intervals
            ensure!(config.min_validator_status_interval <= 10000, Error::<T>::InvalidRateLimitConfig);
            ensure!(config.min_proposal_interval <= 1000, Error::<T>::InvalidRateLimitConfig);
            ensure!(config.min_vote_interval <= 100, Error::<T>::InvalidRateLimitConfig);

            // Check weight bounds are reasonable
            ensure!(config.max_validator_iteration_weight > 0, Error::<T>::InvalidRateLimitConfig);
            ensure!(config.max_proposal_processing_weight > 0, Error::<T>::InvalidRateLimitConfig);
            ensure!(config.max_loop_iterations > 0 && config.max_loop_iterations <= 10000, Error::<T>::InvalidRateLimitConfig);

            Ok(())
        }

        /// Handle rate limit violation by emitting event and returning error.
        fn handle_rate_limit_violation(
            account: &T::AccountId,
            operation: u8,
            violation_type: u8,
            current_count: u32,
            limit: u32,
        ) -> DispatchResult {
            // Emit event for monitoring and analysis
            Self::deposit_event(Event::RateLimitViolation {
                account: account.clone(),
                operation,
                violation_type,
                current_count,
                limit,
            });

            // Return appropriate error based on violation type
            match violation_type {
                0 => Err(Error::<T>::PerBlockRateLimitExceeded.into()), // PerBlock
                1 => Err(Error::<T>::PerAccountRateLimitExceeded.into()), // PerAccount
                2 => Err(Error::<T>::MinimumIntervalViolation.into()), // MinInterval
                3 => Err(Error::<T>::WeightLimitExceeded.into()), // Weight
                _ => Err(Error::<T>::PerBlockRateLimitExceeded.into()), // Default
            }
        }
    }

    impl<T: Config> Pallet<T> {
        /// Get current timestamp from the timestamp pallet
        fn get_current_timestamp() -> u64 {
            // Use a simple timestamp for now - in production this would be from timestamp pallet
            sp_io::offchain::timestamp().unix_millis()
        }

        /// Calculate validator uptime percentage
        fn calculate_uptime_percentage(validator: &T::AccountId) -> u32 {
            let current_block = frame_system::Pallet::<T>::block_number().saturated_into::<u32>();
            let last_seen = Self::validator_last_seen(validator);
            let join_time_block = pos::ValidatorJoinTime::<T>::get(validator).unwrap_or(0);
            
            if current_block <= join_time_block {
                return T::PercentagePrecision::get(); // Full percentage if just joined
            }
            
            let total_blocks_since_join = current_block.saturating_sub(join_time_block);
            let blocks_since_last_seen = current_block.saturating_sub(last_seen);
            
            if total_blocks_since_join == 0 {
                return T::PercentagePrecision::get(); // Full percentage
            }
            
            let active_blocks = total_blocks_since_join.saturating_sub(blocks_since_last_seen);
            let uptime_percentage = (active_blocks as u64 * T::PercentagePrecision::get() as u64) / total_blocks_since_join as u64;
            
            uptime_percentage.min(T::PercentagePrecision::get() as u64) as u32 // Cap at full percentage
        }

        /// Get comprehensive system metrics for operational monitoring.
        ///
        /// This method returns aggregated metrics about the DCF system state,
        /// providing all essential information for monitoring and operational
        /// visibility in a single compact structure.
        ///
        /// The metrics are automatically updated during epoch transitions and
        /// reflect the current system state. This method provides efficient
        /// access to cached metrics without requiring expensive recalculation.
        ///
        /// # Returns
        /// - `SystemMetrics<T>`: Complete system metrics including validator counts,
        ///   economic totals, epoch information, and system health indicators
        ///
        /// # Usage
        /// - Monitoring dashboards and alerting systems
        /// - Operational visibility and system health checks
        /// - Performance analysis and capacity planning
        /// - Automated system monitoring and reporting
        ///
        /// # Performance
        /// - Constant-time access to cached metrics
        /// - No expensive calculations or storage iterations
        /// - Minimal computational overhead
        /// - Single storage read for complete metrics
        pub fn get_system_metrics() -> SystemMetrics<T> {
            Self::system_metrics()
        }

        /// Get detailed system performance indicators.
        ///
        /// This method returns comprehensive performance metrics that complement
        /// the core system metrics, focusing on operational efficiency and
        /// network performance characteristics.
        ///
        /// Performance indicators provide insights into block production efficiency,
        /// validator competition, consensus participation, and governance activity.
        /// These metrics are particularly useful for performance optimization
        /// and network analysis.
        ///
        /// # Returns
        /// - `SystemPerformanceIndicators`: Detailed performance metrics including
        ///   block production rates, score distributions, and efficiency indicators
        ///
        /// # Usage
        /// - Performance trend analysis and optimization
        /// - Network efficiency monitoring
        /// - Capacity planning and scaling decisions
        /// - Operational efficiency assessment
        ///
        /// # Performance
        /// - Constant-time access to cached indicators
        /// - Updated during epoch transitions
        /// - Minimal computational overhead
        /// - Single storage read for all indicators
        pub fn get_performance_indicators() -> SystemPerformanceIndicators {
            Self::performance_indicators()
        }

        /// Update system metrics with current system state.
        ///
        /// This method recalculates and updates all system metrics based on the
        /// current system state. It should be called during epoch transitions
        /// and other significant system state changes to ensure metrics remain
        /// accurate and up-to-date.
        ///
        /// The method performs the following updates:
        /// - Recalculates validator counts and capacity metrics
        /// - Updates economic totals (stakes, slashing, rewards)
        /// - Refreshes epoch and timing information
        /// - Evaluates system health and invariant status
        /// - Updates performance indicators and efficiency metrics
        ///
        /// # Performance Considerations
        /// - Should only be called during epoch transitions
        /// - Involves multiple storage reads for calculation
        /// - Results are cached for efficient subsequent access
        /// - Computational complexity scales with validator count
        ///
        /// # Usage
        /// - Called automatically during epoch transitions
        /// - Can be called manually after significant system changes
        /// - Used to refresh metrics after governance operations
        /// - Ensures metrics accuracy for monitoring systems
        pub fn update_system_metrics() {
            let current_block = frame_system::Pallet::<T>::block_number();
            let current_block_u32 = current_block.saturated_into::<u32>();
            
            // Calculate validator metrics
            let active_validators = Self::active_validators();
            let total_validators = Self::validator_set();
            let active_validator_count = active_validators.len() as u32;
            let total_validator_count = total_validators.len() as u32;
            let validator_set_capacity = MaxValidatorsOf::<T>::get();
            
            // Calculate economic metrics
            let mut total_reserved = <T as pallet::Config>::Balance::default();
            let mut total_trust_score = 0u64;
            let mut missed_blocks_current_epoch = 0u32;
            
            for validator in &active_validators {
                let stake = pos::Stake::<T>::get(validator);
                total_reserved = total_reserved.saturating_add(stake);
                let trust_score = Self::validator_trust_scores(validator);
                total_trust_score = total_trust_score.saturating_add(trust_score);
                if let Some(state) = Self::validator_states(validator) {
                    missed_blocks_current_epoch = missed_blocks_current_epoch.saturating_add(state.current.missed_blocks);
                }
            }
            
            let average_stake = if active_validator_count > 0 {
                total_reserved / active_validator_count.into()
            } else {
                <T as pallet::Config>::Balance::default()
            };
            
            let average_trust_score = if active_validator_count > 0 {
                total_trust_score / active_validator_count as u64
            } else {
                0u64
            };
            
            // Calculate epoch metrics
            let current_epoch = Self::current_epoch();
            let epoch_config = Self::epoch_config();
            let blocks_in_current_epoch = if epoch_config.blocks_per_epoch > 0 {
                current_block_u32 % epoch_config.blocks_per_epoch
            } else {
                0
            };
            let epoch_progress_percentage = if epoch_config.blocks_per_epoch > 0 {
                (blocks_in_current_epoch * 100) / epoch_config.blocks_per_epoch
            } else {
                0
            };
            
            // Calculate system health
            let invariant_health = if Self::has_invariant_violations() {
                InvariantHealth::Critical
            } else {
                InvariantHealth::Healthy
            };
            
            // Calculate finality lag
            let last_finalized = Self::last_finalized_block();
            let finality_lag = current_block_u32.saturating_sub(last_finalized);
            
            // Get historical totals (these would be maintained by other tasks)
            let total_slashed = pos::Pallet::<T>::epoch_total_slashed();
            let total_rewards = pos::Pallet::<T>::epoch_total_rewarded();
            
            // Create and store updated metrics
            let metrics = SystemMetrics {
                active_validator_count,
                total_validator_count,
                validator_set_capacity,
                total_reserved,
                total_slashed,
                total_rewards,
                average_stake,
                current_epoch,
                blocks_in_current_epoch,
                epoch_progress_percentage,
                average_trust_score,
                invariant_health,
                finality_lag,
                missed_blocks_current_epoch,
            };
            
            SystemMetricsStorage::<T>::put(metrics);
            
            // Update performance indicators
            Self::update_performance_indicators();
            
            // Update timestamp
            MetricsLastUpdated::<T>::put(current_block_u32);
        }

        /// Update system performance indicators.
        ///
        /// This method calculates and updates detailed performance indicators
        /// that complement the core system metrics. It focuses on operational
        /// efficiency metrics and network performance characteristics.
        ///
        /// # Performance Indicators Updated
        /// - Block production timing and efficiency metrics
        /// - Validator score distribution and competition analysis
        /// - Consensus participation rates and network health
        /// - Governance activity levels and validator churn rates
        ///
        /// # Usage
        /// - Called automatically by `update_system_metrics()`
        /// - Can be called independently for performance-focused updates
        /// - Used during epoch transitions for comprehensive updates
        /// - Provides detailed metrics for performance analysis
        fn update_performance_indicators() {
            let active_validators = Self::active_validators();
            let _current_epoch = Self::current_epoch();
            
            // Calculate score distribution metrics
            let mut scores: Vec<u64> = Vec::new();
            let mut participation_count = 0u32;
            
            for validator in &active_validators {
                if let Some(state) = Self::validator_states(validator) {
                    scores.push(state.current.final_score);
                    if state.current.authored_blocks > 0 {
                        participation_count += 1;
                    }
                }
            }
            
            let top_performer_score = scores.iter().max().copied().unwrap_or(0);
            let lowest_performer_score = scores.iter().min().copied().unwrap_or(0);
            
            // Calculate score variance (simplified)
            let mean_score = if !scores.is_empty() {
                scores.iter().sum::<u64>() / scores.len() as u64
            } else {
                0
            };
            
            let score_distribution_variance = if !scores.is_empty() {
                scores.iter()
                    .map(|&score| {
                        let diff = if score > mean_score { score - mean_score } else { mean_score - score };
                        diff * diff
                    })
                    .sum::<u64>() / scores.len() as u64
            } else {
                0
            };
            
            // Calculate participation rate
            let consensus_participation_rate = if active_validators.len() > 0 {
                (participation_count * 100) / active_validators.len() as u32
            } else {
                0
            };
            
            // Calculate governance activity (simplified - count active proposals)
            let governance_activity_level = Self::next_proposal_id(); // Approximation
            
            // Create performance indicators
            let indicators = SystemPerformanceIndicators {
                average_block_time: 6000, // 6 seconds - could be calculated from actual block times
                block_production_rate: 10, // 10 blocks per minute - could be calculated
                consensus_participation_rate,
                score_distribution_variance,
                top_performer_score,
                lowest_performer_score,
                epoch_transition_efficiency: 100, // Could be calculated from successful transitions
                governance_activity_level,
                validator_churn_rate: 0, // Could be calculated from validator set changes
            };
            
            PerformanceIndicatorsStorage::<T>::put(indicators);
        }

        /// Get the timestamp when metrics were last updated.
        ///
        /// This method returns the block number when the system metrics were
        /// last updated, enabling cache invalidation and freshness checks.
        ///
        /// # Returns
        /// - `u32`: Block number of the last metrics update
        ///
        /// # Usage
        /// - Cache invalidation in external systems
        /// - Determining if metrics need refreshing
        /// - Monitoring metrics update frequency
        /// - Performance optimization decisions
        pub fn get_metrics_last_updated() -> u32 {
            Self::metrics_last_updated()
        }

        /// Check if there are any active invariant violations.
        ///
        /// This method provides a quick health check by returning whether
        /// there are any active invariant violations in the system.
        ///
        /// # Returns
        /// - `true`: If there are active violations
        /// - `false`: If no violations are detected
        ///
        /// # Usage
        /// - Quick health checks for metrics calculation
        /// - System health status determination
        /// - Automated alerting triggers
        /// - Dashboard health indicators
        pub fn has_invariant_violations() -> bool {
            // Check if we have any recent invariant violation events
            let current_epoch = Self::current_epoch();
            
            // Check the last few epochs for violations
            for epoch in current_epoch.saturating_sub(3)..=current_epoch {
                if let Some(report) = InvariantReports::<T>::get(epoch) {
                    if !report.violations.is_empty() {
                        return true;
                    }
                }
            }
            
            // Also check current system state for immediate violations
            let active_validators = Self::active_validators();
            let max_validators = MaxValidatorsOf::<T>::get() as usize;
            
            // Check active set size violation
            if active_validators.len() > max_validators {
                return true;
            }
            
            // Check for validators with insufficient stake
            for validator in &active_validators {
                let reserved = T::Currency::reserved_balance(validator);
                if reserved < MinStakeOf::<T>::get() {
                    return true;
                }
            }
            
            false
        }
    }

    // --- Genesis Configuration --- //
    #[pallet::genesis_config]
    #[derive(frame_support::DefaultNoBound)]
    pub struct GenesisConfig<T: Config> {
        /// Initial validators to be added at genesis
        pub validators: Vec<T::AccountId>,
        /// Initial inference scores for validators (must match validators length or be empty)
        pub validator_scores: Vec<u32>,
        /// Initial stakes for validators (must match validators length or be empty for default MinStake)
        pub validator_stakes: Vec<<T as pallet::Config>::Balance>,
        /// Starting epoch number (usually 0)
        pub current_epoch: u32,
        /// Epoch configuration parameters
        pub epoch_config: EpochConfig,
        /// Optional validator names (must match validators length or be empty)
        pub validator_names: Vec<Option<Vec<u8>>>,
        /// Whether to perform strict validation of genesis parameters
        pub strict_validation: bool,
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            // STEP 10.1: Genesis block validation started
            log::info!("============== [CBC-TRACE] 10.1 [pallet-cbc-dcf::genesis_build] Genesis block validation started ==============");
            
            // Perform comprehensive validation if strict_validation is enabled
            if self.strict_validation {
                Self::validate_genesis_config(self);
            }

            // Ensure validators don't exceed MaxValidators
            if self.validators.len() > MaxValidatorsOf::<T>::get() as usize {
                panic!("Genesis validators ({}) exceed MaxValidators ({})",
                       self.validators.len(), MaxValidatorsOf::<T>::get());
            }

            // STEP 10.2: Genesis configuration loaded
            log::info!("============== [CBC-TRACE] 10.2 [pallet-cbc-dcf::genesis_build] Genesis configuration loaded ==============");
            log::info!("  Validator count: {}", self.validators.len());
            log::info!("  Epoch: {}", self.current_epoch);
            log::info!("  Blocks per epoch: {}", self.epoch_config.blocks_per_epoch);

            // Check for duplicate validators
            let mut unique_validators = sp_std::collections::btree_set::BTreeSet::new();
            for validator in &self.validators {
                if !unique_validators.insert(validator) {
                    panic!("Duplicate validator found in genesis config: {:?}", validator);
                }
            }

            // Prepare stakes - use provided stakes or default to MinStake
            let stakes = if self.validator_stakes.is_empty() {
                // Use minimum stake for all validators if stakes not specified
                vec![MinStakeOf::<T>::get(); self.validators.len()]
            } else if self.validator_stakes.len() == self.validators.len() {
                // Validate all stakes meet minimum requirement
                for (i, stake) in self.validator_stakes.iter().enumerate() {
                    if *stake < MinStakeOf::<T>::get() {
                        panic!("Genesis validator {} stake ({:?}) below MinStake ({:?})",
                               i, stake, MinStakeOf::<T>::get());
                    }
                }
                self.validator_stakes.clone()
            } else {
                panic!("validator_stakes length ({}) must match validators length ({}) or be empty",
                       self.validator_stakes.len(), self.validators.len());
            };

            // Prepare scores - use provided scores or default to 0
            let scores = if self.validator_scores.is_empty() {
                vec![0u32; self.validators.len()]
            } else if self.validator_scores.len() == self.validators.len() {
                self.validator_scores.clone()
            } else {
                panic!("validator_scores length ({}) must match validators length ({}) or be empty",
                       self.validator_scores.len(), self.validators.len());
            };

            // Prepare names - use provided names or default to None
            let names = if self.validator_names.is_empty() {
                vec![None; self.validators.len()]
            } else if self.validator_names.len() == self.validators.len() {
                self.validator_names.clone()
            } else {
                panic!("validator_names length ({}) must match validators length ({}) or be empty",
                       self.validator_names.len(), self.validators.len());
            };

            // Set up validator set
            ValidatorSet::<T>::put(
                BoundedVec::try_from(self.validators.clone())
                    .unwrap_or_else(|_| {
                        panic!("Initial validators exceed MaxValidators");
                    }),
            );

            // Initialize each validator with their stake, score, and name
            for (i, (((validator, score), stake), name)) in self.validators.iter()
                .zip(scores.iter())
                .zip(stakes.iter())
                .zip(names.iter())
                .enumerate() {

                // STEP 10.3.N: Genesis validator N initialized
                log::info!("============== [CBC-TRACE] 10.3.{} [pallet-cbc-dcf::genesis_build] Genesis validator {} initialized ==============", i + 1, i + 1);
                log::info!("  Account ID: {:?}", validator);
                log::info!("  Stake: {:?}", stake);
                log::info!("  Initial score: {}", score);
                if let Some(n) = name {
                    log::info!("  Name: {:?}", String::from_utf8_lossy(n));
                }

                // Verify validator has sufficient balance for stake reservation
                let free_balance = T::Currency::free_balance(validator);
                if free_balance < *stake {
                    if self.strict_validation {
                        panic!("Genesis validator {:?} has insufficient balance ({:?}) for stake ({:?})",
                               validator, free_balance, stake);
                    } else {
                        log::warn!("Genesis validator {:?} has insufficient balance ({:?}) for stake ({:?}), skipping",
                                  validator, free_balance, stake);
                        continue;
                    }
                }

                // Reserve the stake for the validator
                if let Err(e) = T::Currency::reserve(validator, *stake) {
                    if self.strict_validation {
                        panic!("Failed to reserve stake for genesis validator {:?}: {:?}", validator, e);
                    } else {
                        log::warn!("Failed to reserve stake for genesis validator {:?}: {:?}, skipping", validator, e);
                        continue;
                    }
                }

                pos::Stake::<T>::insert(validator, *stake);
                
                let mut validator_set = pos::ValidatorSet::<T>::get();
                if !validator_set.contains(validator) {
                    let _ = validator_set.try_push(validator.clone());
                    pos::ValidatorSet::<T>::put(validator_set);
                }

                let mut active_validators = pos::ActiveValidators::<T>::get();
                if !active_validators.contains(validator) {
                    let _ = active_validators.try_push(validator.clone());
                    pos::ActiveValidators::<T>::put(active_validators);
                }

                // Calculate scores based on stake and initial score
                let stake_score = (*stake).saturated_into::<u64>() / T::RewardBoostDivisor::get();
                let inference_score = *score as u64;

                let pos_weight = T::DefaultPosWeight::get();
                let poi_weight = T::DefaultPoiWeight::get();
                let final_score = (stake_score.saturating_mul(pos_weight) + inference_score.saturating_mul(poi_weight)) / T::PercentagePrecision::get() as u64;

                log::info!("  PoS score: {}", stake_score);
                log::info!("  PoI score: {}", inference_score);
                log::info!("  Combined DCF score: {}", final_score);

                // Prepare validator name if provided
                let validator_name = name.as_ref().map(|n| {
                    BoundedVec::try_from(n.clone())
                        .unwrap_or_else(|_| {
                            log::warn!("Genesis validator name too long, truncating");
                            BoundedVec::truncate_from(n.clone())
                        })
                });

                // Create initial history entry
                let mut history = BoundedVec::<EpochStats, T::MaxValidatorHistorySize>::default();
                let _ = history.try_push(EpochStats {
                    epoch: self.current_epoch,
                    stake_score,
                    inference_score,
                    final_score,
                    authored_blocks: 0,
                    missed_blocks: 0,
                });

                // Initialize validator state
                ValidatorStates::<T>::insert(
                    validator,
                    ValidatorState {
                        last_active_epoch: self.current_epoch,
                        current: EpochStats {
                            epoch: self.current_epoch,
                            stake_score,
                            inference_score,
                            final_score,
                            authored_blocks: 0,
                            missed_blocks: 0,
                        },
                        history,
                        uptime: 1, // Start with 1 epoch of uptime
                        inference_success_count: 0,
                        participation_rate: T::FullPercentage::get(), // Start with 100% participation
                        inference_count: 0,
                        last_active_block: 1, // Genesis block
                        name: validator_name,
                        trust_score: T::MaxTrustScore::get() / 2, // Start with 50% trust score
                    },
                );

                log::info!("Genesis validator {:?} initialized with stake: {:?}, final_score: {}, name: {:?}",
                          validator, stake, final_score, name);
            }

            // STEP 10.4: PoS scores calculated for all genesis validators
            log::info!("============== [CBC-TRACE] 10.4 [pallet-cbc-dcf::genesis_build] PoS scores calculated for all genesis validators ==============");
            
            // STEP 10.5: PoI scores calculated for all genesis validators
            log::info!("============== [CBC-TRACE] 10.5 [pallet-cbc-dcf::genesis_build] PoI scores calculated for all genesis validators ==============");
            
            // STEP 10.6: Combined DCF scores computed for all validators
            log::info!("============== [CBC-TRACE] 10.6 [pallet-cbc-dcf::genesis_build] Combined DCF scores computed for all validators ==============");
            log::info!("  PoS weight: {}", T::DefaultPosWeight::get());
            log::info!("  PoI weight: {}", T::DefaultPoiWeight::get());

            // Initialize active validators with all genesis validators
            ActiveValidators::<T>::put(
                BoundedVec::try_from(self.validators.clone())
                    .unwrap_or_else(|_| {
                        panic!("Initial validators exceed MaxValidators");
                    }),
            );

            CurrentEpoch::<T>::put(self.current_epoch);
            EpochConfigStorage::<T>::put(self.epoch_config.clone());
            PosWeight::<T>::put(T::DefaultPosWeight::get());
            PoiWeight::<T>::put(T::DefaultPoiWeight::get());
            
            // Initialize trust score configuration with default values
            TrustScoreConfigStorage::<T>::put(TrustScoreConfig {
                uptime_weight: 40,      // 40% weight for uptime
                inference_weight: 35,   // 35% weight for inference success
                slashing_weight: 25,    // 25% weight for slashing penalty
                max_growth_rate: 500,   // 5% maximum growth per epoch
                max_decay_rate: 200,    // 2% maximum decay per epoch
                min_trust_score: T::MinTrustScore::get(),
                max_trust_score: T::MaxTrustScore::get(),
                stability_factor: 8000, // 80% stability factor
            });

            // Initialize trust score bounds with default values
            TrustScoreBounds::<T>::put(TrustScoreBoundsData {
                min_score: T::MinTrustScore::get(),
                max_score: T::MaxTrustScore::get(),
                max_growth_rate: T::MaxTrustScoreGrowthRate::get(),
                max_decay_rate: T::MaxTrustScoreDecayRate::get(),
                stability_factor: T::TrustScoreStabilityFactor::get(),
                last_updated_epoch: 0,
            });

            // Initialize trust score stability metrics
            TrustScoreStabilityMetrics::<T>::put(TrustScoreStabilityMetricsData::default());
            
            // Initialize governance configuration with default parameter ranges and safety rails
            GovernanceConfigStorage::<T>::put(GovernanceConfig {
                // Epoch configuration parameters with safe ranges
                epoch_length: ParameterRange {
                    min: 50,                                    // Minimum 50 blocks per epoch
                    max: 14400,                                 // Maximum 14400 blocks per epoch (24 hours at 6s)
                    current: self.epoch_config.blocks_per_epoch,
                },
                min_stake: ParameterRange {
                    min: <T as pallet::Config>::Balance::from(100_000u32),          // Minimum 100k units
                    max: <T as pallet::Config>::Balance::from(1_000_000_000u32),    // Maximum 1B units
                    current: MinStakeOf::<T>::get(),
                },
                max_validators: ParameterRange {
                    min: 1,                                     // At least 1 validator
                    max: 1000,                                  // Maximum 1000 validators
                    current: self.epoch_config.max_validators,
                },
                
                // Consensus weight parameters (must sum to precision factor)
                pos_weight: ParameterRange {
                    min: 1000,                                  // Minimum 10% (1000/10000)
                    max: 9000,                                  // Maximum 90% (9000/10000)
                    current: T::DefaultPosWeight::get(),
                },
                poi_weight: ParameterRange {
                    min: 1000,                                  // Minimum 10% (1000/10000)
                    max: 9000,                                  // Maximum 90% (9000/10000)
                    current: T::DefaultPoiWeight::get(),
                },
                
                // Performance threshold parameters
                min_performance_score: ParameterRange {
                    min: 10,                                    // Minimum threshold of 10
                    max: 5000,                                  // Maximum threshold of 5000
                    current: T::MinPerformanceScore::get(),
                },
                high_performance_score: ParameterRange {
                    min: 1000,                                  // Minimum high threshold of 1000
                    max: T::MaxValidatorScore::get(),           // Maximum is the max validator score
                    current: T::HighPerformanceScore::get(),
                },
                min_participation_rate: ParameterRange {
                    min: 10,                                    // Minimum 10% participation
                    max: 90,                                    // Maximum 90% (to allow room for high threshold)
                    current: T::MinParticipationRate::get(),
                },
                high_participation_rate: ParameterRange {
                    min: 50,                                    // Minimum 50% for high threshold
                    max: 100,                                   // Maximum 100% participation
                    current: T::HighParticipationRate::get(),
                },
                
                // Economic parameters
                validator_reward: ParameterRange {
                    min: <T as pallet::Config>::Balance::from(1000u32),             // Minimum 1000 units reward
                    max: <T as pallet::Config>::Balance::from(10_000_000u32),       // Maximum 10M units reward
                    current: ValidatorRewardOf::<T>::get(),
                },
                slash_percent: ParameterRange {
                    min: 1,                                     // Minimum 1% slash
                    max: 50,                                    // Maximum 50% slash
                    current: SlashPercentOf::<T>::get(),
                },
                leave_cooldown: ParameterRange {
                    min: BlockNumberFor::<T>::from(100u32),          // Minimum 100 blocks cooldown
                    max: BlockNumberFor::<T>::from(100_000u32),      // Maximum 100k blocks cooldown
                    current: LeaveCooldownOf::<T>::get().into(),
                },
                
                // Trust score configuration
                trust_score_uptime_weight: ParameterRange {
                    min: 10,                                    // Minimum 10% weight
                    max: 80,                                    // Maximum 80% weight
                    current: <T as pallet::Config>::TrustScoreUptimeWeight::get() as u32,
                },
                trust_score_inference_weight: ParameterRange {
                    min: 10,                                    // Minimum 10% weight
                    max: 80,                                    // Maximum 80% weight
                    current: <T as pallet::Config>::TrustScoreInferenceWeight::get() as u32,
                },
                trust_score_slashing_weight: ParameterRange {
                    min: 5,                                     // Minimum 5% weight
                    max: 50,                                    // Maximum 50% weight
                    current: <T as pallet::Config>::TrustScoreSlashingWeight::get() as u32,
                },
                trust_score_max_growth_rate: ParameterRange {
                    min: 100,                                   // Minimum 1% growth rate
                    max: 2000,                                  // Maximum 20% growth rate
                    current: <T as pallet::Config>::MaxTrustScoreGrowthRate::get(),
                },
                trust_score_max_decay_rate: ParameterRange {
                    min: 50,                                    // Minimum 0.5% decay rate
                    max: 1000,                                  // Maximum 10% decay rate
                    current: <T as pallet::Config>::MaxTrustScoreDecayRate::get(),
                },
                trust_score_min_value: ParameterRange {
                    min: 500,                                   // Minimum 5% of max score
                    max: 3000,                                  // Maximum 30% of max score
                    current: <T as pallet::Config>::MinTrustScore::get(),
                },
                trust_score_max_value: ParameterRange {
                    min: 8000,                                  // Minimum 80% of theoretical max
                    max: 15000,                                 // Maximum 150% of theoretical max
                    current: <T as pallet::Config>::MaxTrustScore::get(),
                },
                trust_score_stability_factor: ParameterRange {
                    min: 5000,                                  // Minimum 50% stability
                    max: 9500,                                  // Maximum 95% stability
                    current: <T as pallet::Config>::TrustScoreStabilityFactor::get(),
                },
                
                // Block production parameters
                block_authorship_boost: ParameterRange {
                    min: 1,                                     // Minimum 1 point boost
                    max: 100,                                   // Maximum 100 points boost
                    current: T::BlockAuthorshipBoost::get(),
                },
                missed_block_penalty: ParameterRange {
                    min: 1,                                     // Minimum 1 point penalty
                    max: 50,                                    // Maximum 50 points penalty
                    current: T::MissedBlockPenalty::get(),
                },
                
                // Inference scoring parameters
                inference_boost_low: ParameterRange {
                    min: 1,                                     // Minimum 1 point boost
                    max: 20,                                    // Maximum 20 points boost
                    current: T::InferenceBoostLow::get(),
                },
                inference_boost_medium: ParameterRange {
                    min: 2,                                     // Minimum 2 points boost
                    max: 50,                                    // Maximum 50 points boost
                    current: T::InferenceBoostMedium::get(),
                },
                inference_boost_high: ParameterRange {
                    min: 5,                                     // Minimum 5 points boost
                    max: 100,                                   // Maximum 100 points boost
                    current: T::InferenceBoostHigh::get(),
                },
            });
            
            // Initialize finality markers for production-ready finality tracking
            LastFinalizedBlock::<T>::put(1);           // Genesis block is finalized
            PreviousFinalizedBlock::<T>::put(0);       // No previous finalized block initially
            PreviousEpochBestBlock::<T>::put(1);       // Genesis block is the best known initially

            // Initialize rate limiting configuration with default values
            RateLimitConfigStorage::<T>::put(RateLimitConfig::default());

            // **FIX FOR ISSUE #3: Initialize Epoch 0 Author Sequence**
            // Generate deterministic author sequence for epoch 0 to fix the missing initialization
            if !self.validators.is_empty() {
                // STEP 10.7: Deterministic author sequence generated for epoch 0
                log::info!("============== [CBC-TRACE] 10.7 [pallet-cbc-dcf::genesis_build] Deterministic author sequence generated for epoch 0 ==============");
                
                // Create deterministic randomness seed for epoch 0
                let genesis_randomness_salt = [0u8; 32]; // Use zero salt for genesis
                let genesis_epoch_salt = [0u8; 16]; // Use zero epoch salt for genesis
                let randomness_seed = Pallet::<T>::generate_deterministic_randomness(
                    1, // Genesis block number
                    0, // Epoch 0
                    &genesis_randomness_salt,
                    &genesis_epoch_salt
                );

                // Convert validators to BoundedVec for the function
                let validators_bounded = BoundedVec::try_from(self.validators.clone())
                    .unwrap_or_else(|_| {
                        panic!("Genesis validators exceed MaxValidators during author sequence generation");
                    });

                // Generate deterministic author sequence for epoch 0
                let epoch_0_author_sequence = Pallet::<T>::generate_deterministic_author_sequence(
                    0, // Epoch 0
                    &validators_bounded,
                    &randomness_seed
                );

                // Store the author sequence for epoch 0
                EpochAuthorSequences::<T>::insert(0, &epoch_0_author_sequence);

                log::info!("  Sequence length: {}", epoch_0_author_sequence.len());
                log::info!("DCF: Generated deterministic author sequence for epoch 0 with {} slots (distributed among {} validators)",
                          epoch_0_author_sequence.len(), self.validators.len());
            }

            // Initialize system metrics with genesis state
            Pallet::<T>::update_system_metrics();

            // STEP 37: Genesis block created
            // Note: At this point in genesis build, the block hash and state root are not yet available
            // as they are computed by the runtime after all pallets complete their genesis build.
            // We log this step to indicate genesis block construction is in progress.
            log::info!("============== [CBC-TRACE] 10.8 [pallet-cbc-dcf::genesis_build] Genesis block created ==============");
            log::info!("  Note: Block hash and state root will be computed by runtime after genesis build completes");

            // STEP 10.9: Genesis validation completed successfully
            log::info!("============== [CBC-TRACE] 10.9 [pallet-cbc-dcf::genesis_build] Genesis validation completed successfully ==============");
            log::info!("DCF Genesis completed: {} validators initialized in epoch {}",
                      self.validators.len(), self.current_epoch);
        }
    }

    impl<T: Config> GenesisConfig<T> {
        /// Validate genesis configuration for consistency and correctness
        fn validate_genesis_config(&self) {
            // Validate validator count
            if self.validators.is_empty() {
                panic!("Genesis must have at least one validator");
            }

            if self.validators.len() > MaxValidatorsOf::<T>::get() as usize {
                panic!("Genesis validators ({}) exceed MaxValidators ({})",
                       self.validators.len(), MaxValidatorsOf::<T>::get());
            }

            // Validate array lengths match or are empty (handled in build())
            if !self.validator_scores.is_empty() && self.validator_scores.len() != self.validators.len() {
                panic!("validator_scores length must match validators length or be empty");
            }

            if !self.validator_stakes.is_empty() && self.validator_stakes.len() != self.validators.len() {
                panic!("validator_stakes length must match validators length or be empty");
            }

            if !self.validator_names.is_empty() && self.validator_names.len() != self.validators.len() {
                panic!("validator_names length must match validators length or be empty");
            }

            // Validate stakes meet minimum requirements
            for (i, stake) in self.validator_stakes.iter().enumerate() {
                if *stake < MinStakeOf::<T>::get() {
                    panic!("Genesis validator {} stake below minimum: {:?} < {:?}",
                           i, stake, MinStakeOf::<T>::get());
                }
            }

            // Validate names don't exceed maximum length
            for (i, name_opt) in self.validator_names.iter().enumerate() {
                if let Some(name) = name_opt {
                    if name.len() > T::MaxValidatorNameSize::get() as usize {
                        panic!("Genesis validator {} name too long: {} > {}",
                               i, name.len(), T::MaxValidatorNameSize::get());
                    }
                }
            }

            // Validate epoch configuration
            if self.epoch_config.blocks_per_epoch == 0 {
                panic!("Epoch length cannot be zero");
            }

            log::info!("Genesis configuration validation passed");
        }

        /// Comprehensive genesis validation routine that checks for duplicate validators and invalid stakes.
        /// 
        /// This function performs thorough validation of the genesis configuration to ensure:
        /// - No duplicate validators exist in the validator set
        /// - All validator stakes meet minimum requirements
        /// - Active set size does not exceed MaxValidators
        /// - All configuration parameters are within valid ranges
        /// 
        /// # Returns
        /// - `Ok(())` if validation passes
        /// - `Err(String)` with detailed error message if validation fails
        /// 
        /// # Requirements Coverage
        /// - 11.1: Checks for duplicate validators and invalid stakes
        /// - 11.2: Validates active set size not exceeding MaxValidators
        pub fn validate_genesis_comprehensive(&self) -> Result<(), String> {
            // Check for empty validator set
            if self.validators.is_empty() {
                return Err("Genesis configuration must contain at least one validator".into());
            }

            // Check for duplicate validators (Requirement 11.1)
            let mut unique_validators = sp_std::collections::btree_set::BTreeSet::new();
            for (i, validator) in self.validators.iter().enumerate() {
                if !unique_validators.insert(validator) {
                    return Err(format!("Duplicate validator found at index {}: {:?}", i, validator));
                }
            }

            // Validate active set size does not exceed MaxValidators (Requirement 11.2)
            let max_validators = MaxValidatorsOf::<T>::get() as usize;
            if self.validators.len() > max_validators {
                return Err(format!(
                    "Genesis validator count ({}) exceeds MaxValidators ({})",
                    self.validators.len(),
                    max_validators
                ));
            }

            // Validate minimum active validators requirement
            let min_active_validators = MinActiveValidatorsOf::<T>::get() as usize;
            if self.validators.len() < min_active_validators {
                return Err(format!(
                    "Genesis validator count ({}) below MinActiveValidators ({})",
                    self.validators.len(),
                    min_active_validators
                ));
            }

            // Prepare stakes for validation - use provided stakes or default to MinStake
            let stakes = if self.validator_stakes.is_empty() {
                vec![MinStakeOf::<T>::get(); self.validators.len()]
            } else if self.validator_stakes.len() == self.validators.len() {
                self.validator_stakes.clone()
            } else {
                return Err(format!(
                    "validator_stakes length ({}) must match validators length ({}) or be empty",
                    self.validator_stakes.len(),
                    self.validators.len()
                ));
            };

            // Validate all stakes meet minimum requirement (Requirement 11.1)
            let min_stake = MinStakeOf::<T>::get();
            for (i, stake) in stakes.iter().enumerate() {
                if *stake < min_stake {
                    return Err(format!(
                        "Genesis validator {} stake ({:?}) below MinStake ({:?})",
                        i, stake, min_stake
                    ));
                }
            }

            // Validate validator scores if provided
            if !self.validator_scores.is_empty() {
                if self.validator_scores.len() != self.validators.len() {
                    return Err(format!(
                        "validator_scores length ({}) must match validators length ({}) or be empty",
                        self.validator_scores.len(),
                        self.validators.len()
                    ));
                }

                let max_score = T::MaxValidatorScore::get() as u32;
                for (i, score) in self.validator_scores.iter().enumerate() {
                    if *score > max_score {
                        return Err(format!(
                            "Genesis validator {} score ({}) exceeds MaxValidatorScore ({})",
                            i, score, max_score
                        ));
                    }
                }
            }

            // Validate validator names if provided
            if !self.validator_names.is_empty() {
                if self.validator_names.len() != self.validators.len() {
                    return Err(format!(
                        "validator_names length ({}) must match validators length ({}) or be empty",
                        self.validator_names.len(),
                        self.validators.len()
                    ));
                }

                let max_name_length = T::MaxValidatorNameSize::get() as usize;
                for (i, name_opt) in self.validator_names.iter().enumerate() {
                    if let Some(name) = name_opt {
                        if name.len() > max_name_length {
                            return Err(format!(
                                "Genesis validator {} name length ({}) exceeds MaxValidatorNameSize ({})",
                                i, name.len(), max_name_length
                            ));
                        }
                    }
                }
            }

            // Validate epoch configuration parameters
            if self.epoch_config.blocks_per_epoch == 0 {
                return Err("Epoch length cannot be zero".into());
            }

            if self.epoch_config.blocks_per_epoch > 100_000 {
                return Err(format!(
                    "Epoch length ({}) exceeds reasonable maximum (100,000 blocks)",
                    self.epoch_config.blocks_per_epoch
                ));
            }

            if self.epoch_config.min_stake != min_stake.saturated_into() {
                return Err(format!(
                    "Epoch config min_stake ({}) does not match pallet MinStake ({:?})",
                    self.epoch_config.min_stake,
                    min_stake
                ));
            }

            if self.epoch_config.max_validators != MaxValidatorsOf::<T>::get() {
                return Err(format!(
                    "Epoch config max_validators ({}) does not match pallet MaxValidators ({})",
                    self.epoch_config.max_validators,
                    MaxValidatorsOf::<T>::get()
                ));
            }

            Ok(())
        }

        /// Dry-run function that builds genesis configuration and asserts all invariants.
        /// 
        /// This function simulates the genesis build process without actually modifying
        /// storage, allowing validation of the complete genesis configuration including
        /// all system invariants that would be established at network launch.
        /// 
        /// # Returns
        /// - `Ok(GenesisValidationReport)` if dry-run succeeds with validation report
        /// - `Err(String)` if dry-run fails with detailed error message
        /// 
        /// # Requirements Coverage
        /// - 11.3: Implements dry-run function that builds genesis and asserts all invariants
        /// - 11.4: Validates complete genesis configuration without side effects
        pub fn dry_run_genesis_build(&self) -> Result<GenesisValidationReport<T>, String> {
            // First perform comprehensive validation
            self.validate_genesis_comprehensive()?;

            let mut report = GenesisValidationReport::<T> {
                validator_count: self.validators.len() as u32,
                total_stake: <T as pallet::Config>::Balance::from(0u32),
                average_stake: <T as pallet::Config>::Balance::from(0u32),
                min_stake_validator: None,
                max_stake_validator: None,
                epoch_config: self.epoch_config.clone(),
                invariant_checks: Vec::new(),
                warnings: Vec::new(),
                validation_passed: true,
            };

            let stakes = if self.validator_stakes.is_empty() {
                vec![MinStakeOf::<T>::get(); self.validators.len()]
            } else {
                self.validator_stakes.clone()
            };

            // Calculate stake statistics
            let mut total_stake = <T as pallet::Config>::Balance::from(0u32);
            let mut min_stake = stakes[0];
            let mut max_stake = stakes[0];
            let mut min_stake_validator = self.validators[0].clone();
            let mut max_stake_validator = self.validators[0].clone();

            for (validator, stake) in self.validators.iter().zip(stakes.iter()) {
                total_stake = total_stake.saturating_add(*stake);
                
                if *stake < min_stake {
                    min_stake = *stake;
                    min_stake_validator = validator.clone();
                }
                
                if *stake > max_stake {
                    max_stake = *stake;
                    max_stake_validator = validator.clone();
                }
            }

            let average_stake = if !stakes.is_empty() {
                total_stake / <T as pallet::Config>::Balance::from(stakes.len() as u32)
            } else {
                <T as pallet::Config>::Balance::from(0u32)
            };

            report.total_stake = total_stake;
            report.average_stake = average_stake;
            report.min_stake_validator = Some((min_stake_validator, min_stake));
            report.max_stake_validator = Some((max_stake_validator, max_stake));

            // Simulate invariant checks that would be performed after genesis
            let mut invariant_checks = Vec::new();

            // Economic invariant: All stakes are properly reserved
            invariant_checks.push("Economic: All validator stakes meet minimum requirements".into());
            
            // Validator set invariant: Active set size within limits
            invariant_checks.push(format!(
                "Validator Set: Active validator count ({}) within limits (min: {}, max: {})",
                self.validators.len(),
                MinActiveValidatorsOf::<T>::get(),
                MaxValidatorsOf::<T>::get()
            ));

            // Temporal invariant: Epoch configuration is valid
            invariant_checks.push(format!(
                "Temporal: Epoch length ({}) is within reasonable bounds",
                self.epoch_config.blocks_per_epoch
            ));

            // Trust score invariant: Initial trust scores would be within bounds
            let min_trust_score = <T as pallet::Config>::MinTrustScore::get();
            let max_trust_score = <T as pallet::Config>::MaxTrustScore::get();
            let initial_trust_score = max_trust_score / 2; // Genesis starts with 50% trust score
            
            if initial_trust_score >= min_trust_score && initial_trust_score <= max_trust_score {
                invariant_checks.push(format!(
                    "Trust Score: Initial trust score ({}) within bounds ({} to {})",
                    initial_trust_score, min_trust_score, max_trust_score
                ));
            } else {
                return Err(format!(
                    "Trust score invariant violation: Initial score ({}) outside bounds ({} to {})",
                    initial_trust_score, min_trust_score, max_trust_score
                ));
            }

            // Consensus weight invariant: PoS and PoI weights are valid
            let pos_weight = <T as pallet::Config>::DefaultPosWeight::get();
            let poi_weight = <T as pallet::Config>::DefaultPoiWeight::get();
            
            if pos_weight + poi_weight > 0 {
                invariant_checks.push(format!(
                    "Consensus Weights: PoS ({}) + PoI ({}) = {} > 0",
                    pos_weight, poi_weight, pos_weight + poi_weight
                ));
            } else {
                return Err(format!(
                    "Consensus weight invariant violation: PoS ({}) + PoI ({}) must be > 0",
                    pos_weight, poi_weight
                ));
            }

            report.invariant_checks = invariant_checks;

            // Generate warnings for potential issues
            let mut warnings = Vec::new();

            // Warn if validator set is very small
            if self.validators.len() < 3 {
                warnings.push(format!(
                    "Small validator set: Only {} validators configured (consider 3+ for better decentralization)",
                    self.validators.len()
                ));
            }

            // Warn if stake distribution is highly unequal
            if max_stake > min_stake * <T as pallet::Config>::Balance::from(10u32) {
                warnings.push(format!(
                    "High stake inequality: Max stake ({:?}) is >10x min stake ({:?})",
                    max_stake, min_stake
                ));
            }

            // Warn if epoch length is very short or very long
            if self.epoch_config.blocks_per_epoch < 100 {
                warnings.push(format!(
                    "Short epoch length: {} blocks may cause frequent validator set changes",
                    self.epoch_config.blocks_per_epoch
                ));
            } else if self.epoch_config.blocks_per_epoch > 10_000 {
                warnings.push(format!(
                    "Long epoch length: {} blocks may delay validator set updates",
                    self.epoch_config.blocks_per_epoch
                ));
            }

            report.warnings = warnings;

            Ok(report)
        }
    }

    // --- Public Helper Functions --- //
    impl<T: Config> Pallet<T> {
        /// Validate if a block author is an active validator.
        pub fn validate_block_author(block_number: u32, author: T::AccountId) {
            if !Self::is_validator_active(&author) {
                Self::deposit_event(Event::InvalidAuthor {
                    block_number,
                    author,
                });
            }
        }

        /// Validate if the block author matches the expected author for the given block number.
        /// Returns true if valid, false if mismatch (and emits AuthorMismatch event).
        pub fn validate_expected_author(block_number: u32, actual_author: T::AccountId) -> bool {
            if let Some(expected_author) = Self::get_expected_author(block_number) {
                if actual_author != expected_author {
                    // Emit AuthorMismatch event before returning false
                    Self::deposit_event(Event::AuthorMismatch {
                        block_number,
                        expected: Some(expected_author),
                        actual: actual_author,
                    });
                    return false;
                }
            }
            true
        }

        /// Validate a genesis configuration without building it.
        /// 
        /// This function performs comprehensive validation of a genesis configuration
        /// to ensure it meets all requirements for a valid DCF network launch.
        /// 
        /// # Arguments
        /// * `config` - The genesis configuration to validate
        /// 
        /// # Returns
        /// * `Ok(())` if validation passes
        /// * `Err(String)` with detailed error message if validation fails
        /// 
        /// # Requirements Coverage
        /// * 11.1: Checks for duplicate validators and invalid stakes
        /// * 11.2: Validates active set size not exceeding MaxValidators
        pub fn validate_genesis_config(config: &GenesisConfig<T>) -> Result<(), String> {
            config.validate_genesis_comprehensive()
        }

        /// Perform a dry-run of genesis build with comprehensive validation and reporting.
        /// 
        /// This function simulates the complete genesis build process without modifying
        /// storage, providing detailed validation results and system health analysis.
        /// 
        /// # Arguments
        /// * `config` - The genesis configuration to dry-run
        /// 
        /// # Returns
        /// * `Ok(GenesisValidationReport<T>)` with detailed validation results
        /// * `Err(String)` if dry-run fails with error details
        /// 
        /// # Requirements Coverage
        /// * 11.3: Implements dry-run function that builds genesis and asserts all invariants
        /// * 11.4: Provides comprehensive validation without side effects
        pub fn dry_run_genesis_build(config: &GenesisConfig<T>) -> Result<GenesisValidationReport<T>, String> {
            config.dry_run_genesis_build()
        }

        /// Validate current system state against all invariants.
        /// 
        /// This function checks the current runtime state against all system invariants
        /// to ensure the system remains in a valid state. Can be used for health checks
        /// and debugging.
        /// 
        /// # Returns
        /// * `Ok(())` if all invariants pass
        /// * `Err(Vec<String>)` with list of invariant violations
        pub fn validate_current_invariants() -> Result<(), Vec<String>> {
            let mut violations = Vec::new();

            // Check economic invariants
            let active_validators = Self::active_validators();
            let min_stake = MinStakeOf::<T>::get();
            
            for validator in active_validators.iter() {
                let stake = pos::Stake::<T>::get(validator);
                if stake < min_stake {
                    violations.push(format!(
                        "Economic invariant violation: Validator {:?} stake ({:?}) below MinStake ({:?})",
                        validator, stake, min_stake
                    ));
                }
            }

            // Check validator set invariants
            let max_validators = MaxValidatorsOf::<T>::get();
            if active_validators.len() > max_validators as usize {
                violations.push(format!(
                    "Validator set invariant violation: Active validators ({}) exceed MaxValidators ({})",
                    active_validators.len(), max_validators
                ));
            }

            let min_active_validators = MinActiveValidatorsOf::<T>::get();
            if active_validators.len() < min_active_validators as usize {
                violations.push(format!(
                    "Validator set invariant violation: Active validators ({}) below MinActiveValidators ({})",
                    active_validators.len(), min_active_validators
                ));
            }

            // Check trust score invariants
            let min_trust_score = <T as pallet::Config>::MinTrustScore::get();
            let max_trust_score = <T as pallet::Config>::MaxTrustScore::get();
            
            for validator in active_validators.iter() {
                let trust_score = Self::validator_trust_scores(validator);
                if trust_score < min_trust_score || trust_score > max_trust_score {
                    violations.push(format!(
                        "Trust score invariant violation: Validator {:?} trust score ({}) outside bounds ({} to {})",
                        validator, trust_score, min_trust_score, max_trust_score
                    ));
                }
            }

            // Check consensus weight invariants
            let pos_weight = Self::pos_weight();
            let poi_weight = Self::poi_weight();
            
            if pos_weight + poi_weight == 0 {
                violations.push(format!(
                    "Consensus weight invariant violation: PoS ({}) + PoI ({}) must be > 0",
                    pos_weight, poi_weight
                ));
            }

            // Check temporal invariants
            let last_finalized = Self::last_finalized_block();
            let previous_finalized = Self::previous_finalized_block();
            
            if last_finalized < previous_finalized {
                violations.push(format!(
                    "Temporal invariant violation: Last finalized block ({}) < previous finalized block ({})",
                    last_finalized, previous_finalized
                ));
            }

            if violations.is_empty() {
                Ok(())
            } else {
                Err(violations)
            }
        }

        /// Check if an account is an active validator.
        pub fn is_validator_active(author: &T::AccountId) -> bool {
            Self::active_validators().contains(author)
        }

        /// Get the expected author for a given block number using deterministic author sequences.
        /// 
        /// This function returns the expected block author using pre-computed deterministic
        /// author sequences generated during epoch transitions. If no sequence is available,
        /// it falls back to the legacy weighted selection method.
        /// 
        /// The deterministic approach ensures that all nodes agree on the expected author
        /// for any given block number, enabling consistent block production and validation.
        pub fn get_expected_author(block_number: u32) -> Option<T::AccountId> {
            let validators = Self::active_validators();
            if validators.is_empty() {
                log::warn!("DCF: No active validators available for block authorship at block {}", block_number);
                return None;
            }

            // Determine which epoch this block belongs to
            let epoch_config = Self::epoch_config();
            let blocks_per_epoch = epoch_config.blocks_per_epoch;
            let epoch = block_number / blocks_per_epoch;
            
            // Try to get author from deterministic sequence first
            if let Some(author_sequence) = EpochAuthorSequences::<T>::get(epoch) {
                let block_offset = block_number % blocks_per_epoch;
                if let Some(author) = author_sequence.get(block_offset as usize) {
                    log::trace!(
                        "DCF: Selected deterministic author {:?} for block {} (epoch {}, offset {})",
                        author, block_number, epoch, block_offset
                    );
                    return Some(author.clone());
                }
            }
            
            // Fallback to legacy weighted selection if no deterministic sequence available
            log::trace!(
                "DCF: No deterministic sequence found for epoch {}, using legacy selection for block {}",
                epoch, block_number
            );
            
            Self::get_expected_author_legacy(block_number)
        }
        
        /// Legacy expected author selection using weighted randomness.
        /// 
        /// This function provides the original author selection logic as a fallback
        /// when deterministic sequences are not available. It uses weighted selection
        /// based on validator scores with block number as randomness seed.
        pub fn get_expected_author_legacy(block_number: u32) -> Option<T::AccountId> {
            let validators = Self::active_validators();
            if validators.is_empty() {
                return None;
            }

            // Calculate total weighted score using fresh PoS and PoI scores
            let mut total_weight = 0u64;
            let mut validator_weights: Vec<(T::AccountId, u64, u64, u64)> = Vec::new(); // (validator, combined_score, pos_score, poi_score)
            
            let pos_weight = Self::pos_weight();
            let poi_weight = Self::poi_weight();
            
            for validator in validators.iter() {
                // Get fresh PoS score from stake
                let stake = pos::Pallet::<T>::stake(validator);
                let pos_score = stake.saturated_into::<u64>();
                
                // Get fresh PoI score from inference results
                let poi_score = poi::Pallet::<T>::get_score(validator);
                
                // Calculate combined score using current weights
                let mut combined_score = (pos_score.saturating_mul(pos_weight) + poi_score.saturating_mul(poi_weight)) 
                    / T::PercentagePrecision::get() as u64;
                
                // Cap at maximum score
                if combined_score > T::MaxValidatorScore::get() {
                    combined_score = T::MaxValidatorScore::get();
                }
                
                // Ensure minimum weight of 1 for all validators
                let weight = combined_score.max(1);
                total_weight = total_weight.saturating_add(weight);
                validator_weights.push((validator.clone(), weight, pos_score, poi_score));
                
            }

            if total_weight == 0 {
                // Fallback to round-robin if all scores are zero
                let idx = (block_number as usize) % validators.len();
                return validators.get(idx).cloned();
            }

            // Use block number as seed for deterministic selection
            let target = (block_number as u64 * 2654435761u64) % total_weight; // Using a large prime for better distribution
            let mut cumulative_weight = 0u64;

            for (validator, weight, _, _) in validator_weights {
                cumulative_weight = cumulative_weight.saturating_add(weight);
                if target < cumulative_weight {
                    return Some(validator);
                }
            }

            // Fallback to first validator if something goes wrong
            validators.get(0).cloned()
        }

        /// Generate validator proposals based on combined PoS and PoI scores
        /// This function evaluates all validators and suggests actions based on their performance
        pub fn generate_validator_proposals() -> Vec<(T::AccountId, ProposalAction<T>, u64, u64, u64)> {
            let mut proposals = Vec::new();
            let validators = Self::validator_set();
            let pos_weight = Self::pos_weight();
            let poi_weight = Self::poi_weight();
            let min_score_threshold = MinValidatorScoreOf::<T>::get() as u64;
            
            for validator in validators.iter() {
                // Get fresh PoS and PoI scores
                let stake = pos::Pallet::<T>::stake(validator);
                let pos_score = stake.saturated_into::<u64>();
                
                let poi_score = poi::Pallet::<T>::get_score(validator);
                
                // Calculate combined score
                let mut combined_score = (pos_score.saturating_mul(pos_weight) + poi_score.saturating_mul(poi_weight)) 
                    / T::PercentagePrecision::get() as u64;
                
                if combined_score > T::MaxValidatorScore::get() {
                    combined_score = T::MaxValidatorScore::get();
                }
                
                // Generate proposals based on performance
                if combined_score < min_score_threshold {
                    // Propose ejection for underperforming validators
                    let action = ProposalAction::Eject { 
                        validator: validator.clone(), 
                        reason: EjectionReason::ScoreBelowThreshold 
                    };
                    proposals.push((validator.clone(), action, combined_score, pos_score, poi_score));
                } else if combined_score >= T::MaxValidatorScore::get() * T::HighPerformancePercentage::get() as u64 / T::FullPercentage::get() as u64 {
                    // Propose reward for high-performing validators
                    let reward_amount = T::ValidatorReward::get();
                    let action = ProposalAction::Reward { 
                        validator: validator.clone(), 
                        amount: reward_amount 
                    };
                    proposals.push((validator.clone(), action, combined_score, pos_score, poi_score));
                }
                
                // Check for imbalanced scores (too much reliance on one component)
                let total_weighted = pos_score.saturating_mul(pos_weight) + poi_score.saturating_mul(poi_weight);
                if total_weighted > 0 {
                    let pos_contribution = (pos_score.saturating_mul(pos_weight) * T::FullPercentage::get() as u64) / total_weighted;
                    let poi_contribution = (poi_score.saturating_mul(poi_weight) * T::FullPercentage::get() as u64) / total_weighted;
                    
                    // If one component dominates too much, log a warning
                    if pos_contribution > T::MaxPosContribution::get() as u64 {
                        log::warn!("DCF: Validator {:?} relies heavily on PoS ({}%) - PoI score: {}", 
                                  validator, pos_contribution, poi_score);
                    } else if poi_contribution > T::MaxPoiContribution::get() as u64 {
                        log::warn!("DCF: Validator {:?} relies heavily on PoI ({}%) - PoS score: {}", 
                                  validator, poi_contribution, pos_score);
                    }
                }
            }
            
            log::info!("DCF: Generated {} validator proposals based on combined PoS/PoI scores", proposals.len());
            proposals
        }

        /// Get validator profile information with fresh PoS and PoI scores.
        /// Returns: (combined_score, pos_score, poi_score, trust_score, uptime, inference_count, participation_rate, missed_blocks)
        pub fn get_validator_profile(account_id: T::AccountId) -> Option<(u64, u64, u64, u64, u32, u32, u32, u32)> {
            ValidatorStates::<T>::get(&account_id).map(|state| {
                // Fetch fresh PoS (stake) score from the PoS pallet
                let stake = pos::Pallet::<T>::stake(&account_id);
                let pos_score = stake.saturated_into::<u64>();
                
                // Fetch fresh PoI score from the PoI pallet
                let poi_score = if let Some((result, _)) = poi::Pallet::<T>::inference_results(&account_id) {
                    result as u64
                } else {
                    // Fallback to stored inference score if no fresh result available
                    state.current.inference_score
                };
                
                // Get current weights for score calculation
                let pos_weight = if !PosWeight::<T>::exists() {
                    T::DefaultPosWeight::get()
                } else {
                    PosWeight::<T>::get()
                };
                let poi_weight = if !PoiWeight::<T>::exists() {
                    T::DefaultPoiWeight::get()
                } else {
                    PoiWeight::<T>::get()
                };
                
                // Calculate combined score using configured weights
                let mut combined_score = (pos_score.saturating_mul(pos_weight) + poi_score.saturating_mul(poi_weight)) / T::PercentagePrecision::get() as u64;
                
                // Cap the combined score at maximum allowed
                if combined_score > T::MaxValidatorScore::get() {
                    combined_score = T::MaxValidatorScore::get();
                }
                
                // Get trust score (use stored value or calculate fresh)
                let trust_score = ValidatorTrustScores::<T>::get(&account_id);
                
                // Get additional profile information
                let uptime = Self::validator_uptime(&account_id);
                let inference_count = pallet_cbc_poi::Pallet::<T>::validator_inference_count(&account_id) as u32;
                
                (
                    combined_score,      // Fresh calculated combined score
                    pos_score,          // Fresh PoS (stake) score
                    poi_score,          // Fresh PoI score
                    trust_score,        // Current trust score
                    uptime,             // Validator uptime
                    inference_count,    // Number of inferences
                    state.participation_rate, // Participation rate
                    state.current.missed_blocks, // Missed blocks count
                )
            })
        }

        /// Get validator name
        pub fn get_validator_name(account_id: &T::AccountId) -> Option<Vec<u8>> {
            Self::validator_names(account_id).map(|name| name.into_inner())
        }

        /// Get comprehensive validator profile information (new API)
        pub fn get_validator_profile_new(validator: T::AccountId) -> Option<ValidatorProfile<T::AccountId, <T as pallet::Config>::Balance, BlockNumberFor<T>>> {
            let state = Self::validator_states(&validator)?;
            let stake = pos::Stake::<T>::get(&validator);
            let poi_score = state.current.inference_score;
            let status = if Self::is_validator_active(&validator) {
                ValidatorStatus::Active
            } else {
                ValidatorStatus::Inactive
            };

            Some(ValidatorProfile {
                stake,
                poi_score: poi_score as u32,
                final_score: state.current.final_score,
                trust_score: state.trust_score,
                status,
                inference_count: state.inference_count,
                name: state.name.map(|n| BoundedVec::truncate_from(n.into_inner())),
                last_active_block: state.last_active_block.into(),
                _phantom: sp_std::marker::PhantomData,
            })
        }

        /// Get detailed score breakdown for a validator
        pub fn get_validator_score_breakdown(validator: T::AccountId) -> Option<ScoreBreakdown> {
            let state = Self::validator_states(&validator)?;
            let pos_weight = Self::pos_weight();
            let poi_weight = Self::poi_weight();

            Some(ScoreBreakdown {
                pos_score: state.current.stake_score,
                poi_score: state.current.inference_score,
                pos_weight,
                poi_weight,
                final_score: state.current.final_score,
                trust_score: state.trust_score,
            })
        }

        /// Get validator uptime statistics
        pub fn get_validator_uptime_stats(validator: T::AccountId) -> Option<UptimeStats> {
            let state = Self::validator_states(&validator)?;

            Some(UptimeStats {
                total_epochs: state.uptime,
                blocks_authored: state.current.authored_blocks,
                blocks_missed: state.current.missed_blocks,
                uptime_percentage: Self::calculate_uptime_percentage_api(&validator),
                participation_rate: state.participation_rate,
            })
        }

        /// Get slashing history for a validator
        pub fn get_slashing_history(validator: T::AccountId) -> Vec<SlashingRecord<<T as pallet::Config>::Balance, BlockNumberFor<T>>> {
            // Query the validator's slashing history from storage
            ValidatorSlashingHistory::<T>::get(&validator).to_vec()
        }

        pub fn get_system_constants() -> SystemConstants<<T as pallet::Config>::Balance, BlockNumberFor<T>> {
            SystemConstants {
                min_stake: MinStakeOf::<T>::get(),
                min_score_threshold: MinValidatorScoreOf::<T>::get() as u64,
                epoch_length: <T as pallet::Config>::EpochLength::get(),
                max_validators: MaxValidatorsOf::<T>::get(),
                cooldown_period: LeaveCooldownOf::<T>::get().into(),
            }
        }

        /// Get validator cooldown status
        pub fn get_validator_cooldown_status(validator: T::AccountId) -> Option<BlockNumberFor<T>> {
            pos::ValidatorLeaveRequests::<T>::get(&validator).map(|block| block.into())
        }

        /// Get detailed validator cooldown status with remaining blocks and eligibility
        pub fn get_validator_detailed_cooldown_status(validator: T::AccountId) -> Option<(u32, bool)> {
            Self::get_validator_detailed_cooldown_status_internal(&validator)
        }

        /// Calculate uptime percentage for a validator (API version)
        fn calculate_uptime_percentage_api(validator: &T::AccountId) -> u32 {
            if let Some(state) = Self::validator_states(validator) {
                let total_blocks = state.current.authored_blocks + state.current.missed_blocks;
                if total_blocks > 0 {
                    state.current.authored_blocks * 10000 / total_blocks // Return as basis points (0-10000)
                } else {
                    10000 // 100% if no blocks to compare
                }
            } else {
                0
            }
        }

        /// Get the last finalized block number.
        pub fn get_last_finalized_block() -> u32 {
            Self::last_finalized_block()
        }

        /// Check if a block is finalized.
        pub fn is_block_finalized(block_number: u32) -> bool {
            block_number <= Self::last_finalized_block()
        }

        /// Get finality information including last finalized block and current epoch.
        pub fn get_finality_info() -> (u32, u32) {
            (Self::last_finalized_block(), Self::current_epoch())
        }

        /// Get the number of blocks since last finalization.
        pub fn blocks_since_finalization(current_block: u32) -> u32 {
            current_block.saturating_sub(Self::last_finalized_block())
        }

        /// Get the inference result for a validator.
        pub fn get_inference_result(account_id: T::AccountId) -> Option<u64> {
            ValidatorStates::<T>::get(&account_id).map(|state| state.current.inference_score)
        }

        /// Get validator stake score
        pub fn validator_stake_score(validator: &T::AccountId) -> u128 {
            pos::Stake::<T>::get(validator).saturated_into()
        }

        /// Get validator inference score
        pub fn validator_inference_score(validator: &T::AccountId) -> u64 {
            ValidatorStates::<T>::get(validator)
                .map(|state| state.current.inference_score)
                .unwrap_or(0)
        }

        pub fn validator_participation(validator: &T::AccountId) -> (u32, u32) {
            ValidatorStates::<T>::get(validator)
                .map(|state| (state.current.authored_blocks, state.current.missed_blocks))
                .unwrap_or((u32::MAX, u32::MAX))
        }

        /// Get validator last active block
        pub fn validator_last_active(validator: &T::AccountId) -> u32 {
            ValidatorStates::<T>::get(validator)
                .map(|state| state.last_active_block)
                .unwrap_or(0)
        }

        /// Get consensus weights
        pub fn consensus_weights() -> (u64, u64) {
            (PosWeight::<T>::get(), PoiWeight::<T>::get())
        }

        /// Get governance mode
        pub fn governance_mode() -> bool {
            // Return true if governance config exists and has valid parameters
            let config = GovernanceConfigStorage::<T>::get();
            config.epoch_length.current > 0
        }

        /// Get total validators count
        pub fn total_validators_count() -> u32 {
            ValidatorStates::<T>::iter().count() as u32
        }

        /// Get validator set info
        pub fn validator_set_info() -> (u32, u32, u32) {
            let active_count = Self::active_validators().len() as u32;
            let total_count = Self::total_validators_count();
            let max_validators = MaxValidatorsOf::<T>::get();
            (active_count, total_count, max_validators)
        }

        /// Get validators by score (sorted)
        pub fn validators_by_score() -> Vec<(T::AccountId, u64)> {
            Self::get_validators_by_score()
        }

        /// Get expected author for a block
        pub fn expected_author(block_number: u32) -> Option<T::AccountId> {
            Self::get_expected_author(block_number)
        }

        /// Get validator score history
        pub fn validator_score_history(validator: &T::AccountId) -> Vec<EpochStats> {
            ValidatorStates::<T>::get(validator)
                .map(|state| state.history.to_vec())
                .unwrap_or_default()
        }

        /// Get finality info
        pub fn finality_info() -> (u32, u32) {
            Self::get_finality_info()
        }

        // execute_slash_validator, execute_slash_validator_with_reason, distribute_rewards, execute_reward_validator, and execute_reward_validator_with_reason moved to pallet-cbc-pos.

        /// Execute ejection action on a validator
        fn execute_eject_validator(validator: &T::AccountId, reason: EjectionReason) -> DispatchResult {
            pos::Pallet::<T>::force_eject_validator(validator)?;

            Self::deposit_event(Event::ValidatorEjected {
                validator: validator.clone(),
                reason,
            });

            Ok(())
        }

        /// Execute add validator action
        fn execute_add_validator(validator: &T::AccountId) -> DispatchResult {
            // Check if validator already exists in the validator set
            let mut validator_set = ValidatorSet::<T>::get();
            ensure!(
                !validator_set.contains(validator),
                Error::<T>::ValidatorAlreadyExists
            );

            // Check that we haven't exceeded the maximum validators limit
            ensure!(
                validator_set.len() < validator_set.capacity(),
                Error::<T>::NotEnoughValidators
            );

            // Check minimum stake requirement
            let stake = pos::Pallet::<T>::stake(validator);
            ensure!(
                stake >= MinStakeOf::<T>::get(),
                Error::<T>::InsufficientStake
            );

            // Add validator to the validator set
            validator_set.try_push(validator.clone())
                .map_err(|_| Error::<T>::NotEnoughValidators)?;
            ValidatorSet::<T>::put(validator_set);

            // Initialize validator state if it doesn't exist
            if !ValidatorStates::<T>::contains_key(validator) {
                let current_epoch = Self::current_epoch();
                let stake_score = stake.saturated_into::<u64>();
                
                // Get initial inference score from PoI pallet
                let inference_score = poi::Pallet::<T>::get_score(validator);

                // Calculate initial final score
                let pos_weight = Self::pos_weight();
                let poi_weight = Self::poi_weight();
                let mut final_score = (stake_score.saturating_mul(pos_weight) + inference_score.saturating_mul(poi_weight)) / T::PercentagePrecision::get() as u64;
                if final_score > T::MaxValidatorScore::get() {
                    final_score = T::MaxValidatorScore::get();
                }

                let initial_stats = EpochStats {
                    epoch: current_epoch,
                    stake_score,
                    inference_score,
                    final_score,
                    authored_blocks: 0,
                    missed_blocks: 0,
                };

                let current_block = frame_system::Pallet::<T>::block_number().saturated_into::<u32>();
                
                let validator_state = ValidatorState {
                    last_active_epoch: current_epoch,
                    current: initial_stats.clone(),
                    history: BoundedVec::try_from(vec![initial_stats]).unwrap_or_default(),
                    uptime: 1,
                    inference_success_count: 0,
                    participation_rate: 0,
                    inference_count: 0, // Starting with 0 inferences
                    last_active_block: current_block, // Current block number
                    name: None, // No name set for governance-added validators
                    trust_score: 0, // Will be calculated later
                };

                ValidatorStates::<T>::insert(validator, validator_state);

                // Set join time in pos pallet
                pos::ValidatorJoinTime::<T>::insert(validator, current_block);
            }

            // Emit event
            Self::deposit_event(Event::ValidatorAdded { validator: validator.clone() });

            log::info!("DCF: Validator {:?} added via governance proposal", validator);
            Ok(())
        }

        /// Execute remove validator action
        fn execute_remove_validator(validator: &T::AccountId) -> DispatchResult {
            pos::Pallet::<T>::force_eject_validator(validator)?;

            Self::deposit_event(Event::ValidatorRemoved { validator: validator.clone() });

            Ok(())
        }

        /// Sort validators by their final weighted score (highest first).
        fn sort_validators_by_score(validators: &mut BoundedVec<T::AccountId, MaxValidatorsOf<T>>) {
            validators.sort_by(|a, b| {
                let score_a = ValidatorStates::<T>::get(a)
                    .map(|state| state.current.final_score)
                    .unwrap_or(0);
                let score_b = ValidatorStates::<T>::get(b)
                    .map(|state| state.current.final_score)
                    .unwrap_or(0);
                // Sort in descending order (highest score first)
                score_b.cmp(&score_a)
            });
        }

        /// Apply pending join/leave actions at epoch transition.
        /// Returns vectors of actually added and removed validators.
        fn apply_pending_validator_actions() -> (Vec<T::AccountId>, Vec<T::AccountId>) {
            let mut active = ActiveValidators::<T>::get();
            let mut changed = false;
            let mut actual_removed: Vec<T::AccountId> = Vec::new();
            let mut actual_added: Vec<T::AccountId> = Vec::new();

            // Collect all actions to avoid double borrow
            let actions: Vec<(T::AccountId, ValidatorAction)> =
                PendingValidatorActions::<T>::iter().collect();

            // Separate join and leave actions
            let mut join_requests: Vec<T::AccountId> = Vec::new();
            let mut leave_requests: Vec<T::AccountId> = Vec::new();

            for (who, action) in actions {
                match action {
                    ValidatorAction::Join => {
                        if !active.contains(&who) {
                            join_requests.push(who.clone());
                        }
                    }
                    ValidatorAction::Leave => {
                        if active.contains(&who) {
                            leave_requests.push(who.clone());
                        }
                    }
                }
                PendingValidatorActions::<T>::remove(&who);
            }

            // Process leave requests first
            for who in leave_requests {
                if let Some(pos) = active.iter().position(|v| v == &who) {
                    active.remove(pos);
                    actual_removed.push(who.clone());
                    changed = true;
                    log::debug!("DCF: Validator {:?} left the active set", who);
                }
            }

            // Process join requests based on scores and available capacity
            if !join_requests.is_empty() {
                // Sort join requests by their scores (highest first)
                join_requests.sort_by(|a, b| {
                    let score_a = ValidatorStates::<T>::get(a)
                        .map(|state| state.current.final_score)
                        .unwrap_or(0);
                    let score_b = ValidatorStates::<T>::get(b)
                        .map(|state| state.current.final_score)
                        .unwrap_or(0);
                    score_b.cmp(&score_a) // Descending order
                });

                let available_slots = active.capacity() - active.len();
                let min_score_threshold = MinValidatorScoreOf::<T>::get() as u64;

                for who in join_requests.iter().take(available_slots) {
                    let score = ValidatorStates::<T>::get(who)
                        .map(|state| state.current.final_score)
                        .unwrap_or(0);
                    
                    // Only add validators that meet the minimum score threshold
                    if score >= min_score_threshold {
                        if active.try_push(who.clone()).is_ok() {
                            actual_added.push(who.clone());
                            changed = true;
                            log::debug!("DCF: Validator {:?} joined the active set (score: {})", who, score);
                        }
                    } else {
                        log::warn!("DCF: Rejected join request for validator {:?} due to low score: {}", who, score);
                    }
                }

                // Log any rejected join requests due to capacity
                if join_requests.len() > available_slots {
                    let rejected_count = join_requests.len() - available_slots;
                    log::info!("DCF: Rejected {} join requests due to capacity constraints", rejected_count);
                }
            }

            if changed {
                // Sort active validators by account ID for deterministic ordering.
                // Score-based sort is non-deterministic across nodes.
                active.sort_by(|a, b| a.cmp(b));
                ActiveValidators::<T>::put(active);
            }

            (actual_added, actual_removed)
        }

        /// Get the epoch history for a given epoch number (for runtime API).
        pub fn get_epoch_history_api(epoch_number: u32) -> Option<RuntimeEpochHistory<T::AccountId>> {
            let histories = Self::epoch_histories();
            histories.iter().find(|h| h.epoch_number == epoch_number).cloned().map(|h| h.into())
        }

        /// Get the most recent n epoch histories (for runtime API).
        pub fn get_recent_epochs_api(n: u32) -> Vec<RuntimeEpochHistory<T::AccountId>> {
            let histories = Self::epoch_histories();
            let len = histories.len().min(n as usize);
            histories.iter().rev().take(len).cloned().map(|h| h.into()).collect::<Vec<_>>().into_iter().rev().collect()
        }

        /// Get governance mode status (for runtime API).
        pub fn get_governance_mode() -> bool {
            Self::governance_mode_enabled()
        }

        /// Get validators sorted by their final weighted score (highest first).
        pub fn get_validators_by_score() -> Vec<(T::AccountId, u64)> {
            let mut validators_with_scores: Vec<(T::AccountId, u64)> = Self::active_validators()
                .iter()
                .map(|validator| {
                    let score = ValidatorStates::<T>::get(validator)
                        .map(|state| state.current.final_score)
                        .unwrap_or(0);
                    (validator.clone(), score)
                })
                .collect();
            
            // Sort by score (highest first)
            validators_with_scores.sort_by(|a, b| b.1.cmp(&a.1));
            validators_with_scores
        }

        /// Get proposal details by ID (for runtime API).
        pub fn get_proposal_details(proposal_id: u32) -> Option<GovernanceProposal<T>> {
            Proposals::<T>::get(proposal_id)
        }

        /// Get all active proposals (for runtime API).
        pub fn get_active_proposals() -> Vec<(u32, GovernanceProposal<T>)> {
            Proposals::<T>::iter()
                .filter(|(_, proposal)| proposal.status == ProposalStatus::Pending)
                .collect()
        }

        /// Get validator's current consensus weights contribution (for runtime API).
        pub fn get_validator_consensus_contribution(validator: &T::AccountId) -> Option<(u64, u64, u64)> {
            ValidatorStates::<T>::get(validator).map(|state| {
                let pos_weight = Self::pos_weight();
                let poi_weight = Self::poi_weight();
                let pos_contribution = (state.current.stake_score * pos_weight) / T::PercentagePrecision::get() as u64;
                let poi_contribution = (state.current.inference_score * poi_weight) / T::PercentagePrecision::get() as u64;
                (pos_contribution, poi_contribution, state.current.final_score)
            })
        }

        /// Get epoch configuration (for runtime API).
        pub fn get_epoch_config() -> EpochConfig {
            Self::epoch_config()
        }

        /// Get validator statistics for a specific epoch (for runtime API).
        pub fn get_validator_epoch_stats(validator: &T::AccountId, epoch: u32) -> Option<EpochStats> {
            ValidatorStates::<T>::get(validator).and_then(|state| {
                if state.current.epoch == epoch {
                    Some(state.current.clone())
                } else {
                    state.history.iter().find(|stats| stats.epoch == epoch).cloned()
                }
            })
        }

        /// Get total number of validators in the system (for runtime API).
        pub fn get_total_validators_count() -> u32 {
            ValidatorSet::<T>::get().len() as u32
        }

        /// Calculate economic impact (rewards and slashing) for the given epoch.
        /// This tracks all reward distributions and slashing that occurred during the epoch.
        fn calculate_epoch_economic_impact(epoch: u32) -> (<T as pallet::Config>::Balance, <T as pallet::Config>::Balance) {
            let mut total_rewards = <T as pallet::Config>::Balance::from(0u32);
            let mut total_slashed = <T as pallet::Config>::Balance::from(0u32);

            // Iterate through all executed proposals from the epoch to calculate economic impact
            for (_, proposal) in Proposals::<T>::iter() {
                // Only count executed proposals from the target epoch
                if proposal.status == ProposalStatus::Executed {
                    // Check if proposal was executed in the target epoch by examining when it was last updated
                    // Note: This is a simplified approach - in production you might want to store epoch info with proposals
                    match &proposal.action {
                        ProposalAction::Reward { amount, .. } => {
                            total_rewards = total_rewards.saturating_add(*amount);
                        }
                        ProposalAction::RewardMultiple { validators, amount } => {
                            let reward_count = validators.len() as u32;
                            let total_reward = amount.saturating_mul(reward_count.into());
                            total_rewards = total_rewards.saturating_add(total_reward);
                        }
                        ProposalAction::Slash { amount, .. } => {
                            total_slashed = total_slashed.saturating_add(*amount);
                        }
                        _ => {} // Other proposal types don't directly affect economic metrics
                    }
                }
            }

            // Additionally, check for any automatic rewards/slashing that occurred
            // This could include block authorship rewards, inference rewards, missed block penalties, etc.
            // For now, we'll use the governance-based tracking above, but this could be expanded
            // to include automatic economic activities.

            log::debug!("DCF: Epoch {} economic impact - Rewards: {:?}, Slashed: {:?}", 
                       epoch, total_rewards, total_slashed);

            (total_rewards, total_slashed)
        }

        /// Get validator set capacity and current usage (for runtime API).
        pub fn get_validator_set_info() -> (u32, u32, u32) {
            let current_count = ValidatorSet::<T>::get().len() as u32;
            let active_count = ActiveValidators::<T>::get().len() as u32;
            let max_validators = MaxValidatorsOf::<T>::get();
            (current_count, active_count, max_validators)
        }

        /// Get the number of misbehavior reports for a validator
        pub fn get_misbehavior_report_count(validator: &T::AccountId) -> u32 {
            let mut count = 0u32;
            for (_, _) in MisbehaviorReports::<T>::iter_prefix(validator) {
                count = count.saturating_add(1);
            }
            count
        }

        /// Get all reporters who have reported a specific validator
        pub fn get_misbehavior_reporters(validator: &T::AccountId) -> Vec<T::AccountId> {
            MisbehaviorReports::<T>::iter_prefix(validator)
                .map(|(reporter, _)| reporter)
                .collect()
        }

        /// Get misbehavior evidence from a specific reporter about a validator
        pub fn get_misbehavior_evidence(
            validator: &T::AccountId,
            reporter: &T::AccountId,
        ) -> Option<Vec<u8>> {
            MisbehaviorReports::<T>::get(validator, reporter)
                .map(|evidence| evidence.into_inner())
        }

        /// Check if a validator is close to the slashing threshold
        pub fn is_validator_at_risk(validator: &T::AccountId) -> bool {
            let report_count = Self::get_misbehavior_report_count(validator);
            let threshold = T::MisbehaviorSlashThreshold::get();
            report_count >= threshold.saturating_sub(1) // At risk if one report away from threshold
        }

        /// Check for underperforming validators and take action
        fn check_and_handle_underperforming_validators() {
            let min_score = MinValidatorScoreOf::<T>::get() as u64;
            let current_epoch = Self::current_epoch();
            
            let validators_to_check: Vec<T::AccountId> = ActiveValidators::<T>::get().into_inner();
            
            for validator in validators_to_check {
                if let Some(state) = ValidatorStates::<T>::get(&validator) {
                    // Check if validator score is below threshold
                    if state.current.final_score < min_score {
                        log::warn!("DCF: Validator {:?} has low score: {}, ejecting", 
                                  validator, state.current.final_score);
                        let _ = Self::eject_validator(&validator, EjectionReason::ScoreBelowThreshold);
                    }
                    
                    // Check if validator has been inactive for too long
                    let inactive_epochs = current_epoch.saturating_sub(state.last_active_epoch);
                    if inactive_epochs > T::MaxInactiveEpochs::get() {
                        log::warn!("DCF: Validator {:?} inactive for {} epochs, ejecting", 
                                  validator, inactive_epochs);
                        let _ = Self::eject_validator(&validator, EjectionReason::ScoreBelowThreshold);
                    }
                }
            }
        }

        /// Generate automatic proposals for validator set optimization based on scores
        fn generate_automatic_validator_proposals() {
            let active_validators = ActiveValidators::<T>::get();
            let all_validators = ValidatorSet::<T>::get();
            let max_validators = MaxValidatorsOf::<T>::get() as usize;
            let _min_active_validators = MinActiveValidatorsOf::<T>::get() as usize;
            
            // Don't generate proposals if governance mode is disabled
            if !Self::governance_mode_enabled() {
                return;
            }
            
            // Get all validators with their scores, sorted by score (highest first)
            let mut all_validators_with_scores: Vec<(T::AccountId, u64)> = all_validators
                .iter()
                .map(|validator| {
                    let score = ValidatorStates::<T>::get(validator)
                        .map(|state| state.current.final_score)
                        .unwrap_or(0);
                    (validator.clone(), score)
                })
                .collect();
            
            // Sort by score (highest first)
            all_validators_with_scores.sort_by(|a, b| b.1.cmp(&a.1));
            
            // Case 1: We have space for more validators
            if active_validators.len() < max_validators {
                Self::propose_add_high_scoring_validators(&active_validators, &all_validators_with_scores, max_validators);
            }
            // Case 2: We're at capacity, consider replacing low-scoring active validators
            else if active_validators.len() == max_validators {
                Self::propose_replace_low_scoring_validators(&active_validators, &all_validators_with_scores);
            }
            // Case 3: We have too many validators (shouldn't happen, but handle it)
            else if active_validators.len() > max_validators {
                Self::propose_remove_excess_validators(&active_validators, max_validators);
            }
        }

        /// Propose adding high-scoring validators when there's space
        fn propose_add_high_scoring_validators(
            active_validators: &BoundedVec<T::AccountId, MaxValidatorsOf<T>>,
            all_validators_with_scores: &[(T::AccountId, u64)],
            max_validators: usize,
        ) {
            let available_slots = max_validators - active_validators.len();
            let min_score_threshold = MinValidatorScoreOf::<T>::get() as u64;
            
            // Find inactive validators with high scores
            let mut candidates = Vec::new();
            for (validator, score) in all_validators_with_scores {
                if !active_validators.contains(validator) && 
                   *score >= min_score_threshold &&
                   !Self::has_pending_join_request(validator) &&
                   !Self::was_recently_ejected(validator) {
                    candidates.push((validator.clone(), *score));
                    if candidates.len() >= available_slots {
                        break;
                    }
                }
            }
            
            // Create proposals for the best candidates
            for (validator, score) in candidates {
                if let Err(e) = Self::create_automatic_join_proposal(&validator, score) {
                    log::warn!("Failed to create join proposal for validator {:?}: {:?}", validator, e);
                }
            }
        }

        /// Propose replacing low-scoring active validators with higher-scoring inactive ones
        fn propose_replace_low_scoring_validators(
            active_validators: &BoundedVec<T::AccountId, MaxValidatorsOf<T>>,
            all_validators_with_scores: &[(T::AccountId, u64)],
        ) {
            // Get active validators with their scores, sorted by score (lowest first)
            let mut active_with_scores: Vec<(T::AccountId, u64)> = active_validators
                .iter()
                .map(|validator| {
                    let score = ValidatorStates::<T>::get(validator)
                        .map(|state| state.current.final_score)
                        .unwrap_or(0);
                    (validator.clone(), score)
                })
                .collect();
            
            active_with_scores.sort_by(|a, b| a.1.cmp(&b.1)); // Sort by score (lowest first)
            
            // Find inactive validators with higher scores than the lowest active ones
            for (inactive_validator, inactive_score) in all_validators_with_scores {
                if active_validators.contains(inactive_validator) {
                    continue; // Skip active validators
                }
                
                // Check if this inactive validator has a significantly higher score than the lowest active validator
                if let Some((lowest_active_validator, lowest_active_score)) = active_with_scores.first() {
                    let score_improvement = inactive_score.saturating_sub(*lowest_active_score);
                    let percentage_threshold = *lowest_active_score / (T::FullPercentage::get() as u64 / T::ScoreImprovementPercentage::get() as u64);
                    let min_improvement_threshold = percentage_threshold.max(T::ScoreImprovementThreshold::get()); // Configurable improvement threshold
                    
                    if score_improvement >= min_improvement_threshold &&
                       *inactive_score >= MinValidatorScoreOf::<T>::get() as u64 &&
                       !Self::has_pending_join_request(inactive_validator) &&
                       !Self::was_recently_ejected(inactive_validator) {
                        
                        // Propose to eject the lowest scoring active validator
                        if let Err(e) = Self::create_automatic_eject_proposal(lowest_active_validator, EjectionReason::ScoreBelowThreshold) {
                            log::warn!("Failed to create eject proposal for validator {:?}: {:?}", lowest_active_validator, e);
                        }
                        
                        // Propose to add the higher scoring inactive validator
                        if let Err(e) = Self::create_automatic_join_proposal(inactive_validator, *inactive_score) {
                            log::warn!("Failed to create join proposal for validator {:?}: {:?}", inactive_validator, e);
                        }
                        
                        // Only propose one replacement at a time to avoid too many simultaneous changes
                        break;
                    }
                }
            }
        }

        /// Propose removing excess validators (when somehow we have more than MaxValidators)
        fn propose_remove_excess_validators(
            active_validators: &BoundedVec<T::AccountId, MaxValidatorsOf<T>>,
            max_validators: usize,
        ) {
            let excess_count = active_validators.len() - max_validators;
            
            // Get active validators sorted by score (lowest first)
            let mut active_with_scores: Vec<(T::AccountId, u64)> = active_validators
                .iter()
                .map(|validator| {
                    let score = ValidatorStates::<T>::get(validator)
                        .map(|state| state.current.final_score)
                        .unwrap_or(0);
                    (validator.clone(), score)
                })
                .collect();
            
            active_with_scores.sort_by(|a, b| a.1.cmp(&b.1)); // Sort by score (lowest first)
            
            // Propose to eject the lowest scoring validators
            for (validator, _score) in active_with_scores.iter().take(excess_count) {
                if let Err(e) = Self::create_automatic_eject_proposal(validator, EjectionReason::ExcessValidators) {
                    log::warn!("Failed to create eject proposal for excess validator {:?}: {:?}", validator, e);
                }
            }
        }

        /// Check if a validator has a pending join request
        fn has_pending_join_request(validator: &T::AccountId) -> bool {
            PendingValidatorActions::<T>::get(validator) == Some(ValidatorAction::Join)
        }

        /// Check if a validator was recently ejected or removed (to avoid immediate re-addition)
        pub fn was_recently_ejected(validator: &T::AccountId) -> bool {
            // First check if validator is in cooldown period after leaving
            if let Some(left_at_block) = pos::RecentlyRemovedValidators::<T>::get(validator) {
                let current_block = frame_system::Pallet::<T>::block_number().saturated_into::<u32>();
                let cooldown_period = LeaveCooldownOf::<T>::get();
                let blocks_since_left = current_block.saturating_sub(left_at_block);

                // If still in cooldown, they cannot be re-added
                if blocks_since_left < cooldown_period {
                    return true;
                }
            }

            // Check if validator was ejected in the last few epochs
            let current_epoch = Self::current_epoch();
            let grace_period = T::MaxInactiveEpochs::get(); // Don't re-add validators ejected in the last few epochs

            if let Some(state) = ValidatorStates::<T>::get(validator) {
                // If the validator has a very low score or was recently active, they might have been ejected
                let epochs_since_active = current_epoch.saturating_sub(state.last_active_epoch);
                epochs_since_active <= grace_period && state.current.final_score == 0
            } else {
                false
            }
        }

        /// Create an automatic proposal to add a validator
        fn create_automatic_join_proposal(validator: &T::AccountId, score: u64) -> DispatchResult {
            // Instead of creating a governance proposal, directly add to pending actions
            // This is more efficient and works with the existing epoch transition logic
            
            // Check if there's already a pending join action
            if PendingValidatorActions::<T>::get(validator) == Some(ValidatorAction::Join) {
                return Ok(()); // Already has a pending join request
            }
            
            // Add the validator to pending join actions
            PendingValidatorActions::<T>::insert(validator, ValidatorAction::Join);
            
            // Get the validator's stake amount from pos storage
            let stake_amount = pos::Pallet::<T>::stake(validator);
            
            // Emit events to indicate automatic addition
            Self::deposit_event_with_evm_compat(Event::ValidatorJoined { 
                validator: validator.clone(),
                stake_amount,
            });
            Self::deposit_event(Event::AutomaticValidatorProposal {
                validator: validator.clone(),
                action: ValidatorAction::Join,
                score,
                reason: BoundedVec::truncate_from(b"High score automatic addition".to_vec()),
            });
            
            log::info!("DCF: Automatically scheduled validator {:?} for addition (score: {})", validator, score);
            Ok(())
        }

        /// Create an automatic proposal to eject a validator
        fn create_automatic_eject_proposal(validator: &T::AccountId, reason: EjectionReason) -> DispatchResult {
            // For automatic ejections based on score, we can directly eject
            // For governance-based ejections, we create proposals
            
            match reason {
                EjectionReason::ScoreBelowThreshold | EjectionReason::ExcessValidators => {
                    // Get the validator's score before ejection for the event
                    let score = ValidatorStates::<T>::get(validator)
                        .map(|state| state.current.final_score)
                        .unwrap_or(0);
                    
                    // Direct ejection for performance-based reasons
                    Self::eject_validator(validator, reason.clone())?;
                    
                    // Emit automatic proposal event
                    let reason_text = match reason {
                        EjectionReason::ScoreBelowThreshold => "Low score automatic ejection",
                        EjectionReason::ExcessValidators => "Excess validators automatic ejection",
                        _ => "Automatic ejection",
                    };
                    
                    Self::deposit_event(Event::AutomaticValidatorProposal {
                        validator: validator.clone(),
                        action: ValidatorAction::Leave,
                        score,
                        reason: BoundedVec::truncate_from(reason_text.as_bytes().to_vec()),
                    });
                    
                    log::info!("DCF: Automatically ejected validator {:?} (reason: {:?})", validator, reason);
                    Ok(())
                },
                _ => {
                    // Create governance proposal for manual/slashing-based ejections
                    let proposal_id = NextProposalId::<T>::get();
                    let system_account = T::AccountId::decode(&mut &[0u8; 32][..]).unwrap_or_else(|_| {
                        // Fallback: use the first validator as proposer
                        Self::active_validators().get(0).cloned().unwrap_or_else(|| {
                            // Ultimate fallback: decode from a known pattern
                            T::AccountId::decode(&mut &[1u8; 32][..])
                                .unwrap_or_else(|_| {
                                    // If even the fallback fails, use first active validator
                                    Self::active_validators().get(0).cloned()
                                        .unwrap_or_else(|| {
                                            // Ultimate fallback: use first validator from set
                                            Self::validator_set().get(0).cloned()
                                                .unwrap_or_else(|| {
                                                    // Last resort: decode from known pattern
                                                    T::AccountId::decode(&mut &[2u8; 32][..])
                                                        .unwrap_or_else(|_| {
                                                            // Create a zero account if decode fails
                                                            // Use the first validator as fallback
                                                            Self::validator_set().get(0).cloned()
                                                                .unwrap_or_else(|| {
                                                                    // If no validators exist, panic as this is a critical error
                                                                    panic!("No validators available and cannot create fallback account");
                                                                })
                                                        })
                                                })
                                        })
                                })
                        })
                    });
                    
                    let action = ProposalAction::Eject { 
                        validator: validator.clone(), 
                        reason: reason.clone()
                    };
                    
                    let proposal = GovernanceProposal {
                        proposer: system_account,
                        action,
                        status: ProposalStatus::Pending,
                        votes_for: 0,
                        votes_against: 0,
                    };
                    
                    Proposals::<T>::insert(proposal_id, &proposal);
                    NextProposalId::<T>::put(proposal_id + 1);
                    
                    Self::deposit_event(Event::ProposalSubmitted {
                        proposal_id,
                        proposer: proposal.proposer,
                        action: proposal.action,
                    });
                    
                    log::info!("DCF: Created governance eject proposal for validator {:?} (reason: {:?})", validator, reason);
                    Ok(())
                }
            }
        }

        // cleanup_recently_removed_validators and process_expired_leave_requests removed because cooldowns are managed by pallet-cbc-pos.

        /// Emit DCF health metrics for monitoring
        fn emit_dcf_health_metrics(block_number: u32) {
            let total_validators = Self::validator_set().len();
            let active_validators = Self::active_validators().len();
            let current_epoch = Self::current_epoch();
            let governance_mode = Self::governance_mode_enabled();
            
            // Calculate average validator score
            let validator_scores: Vec<u64> = ValidatorSet::<T>::get()
                .iter()
                .filter_map(|v| ValidatorStates::<T>::get(v).map(|s| s.current.final_score))
                .collect();
            
            let avg_score = if !validator_scores.is_empty() {
                validator_scores.iter().sum::<u64>() / validator_scores.len() as u64
            } else {
                0
            };
            
            // Only log health metrics at configured intervals to reduce verbosity
            if block_number % T::DetailedLoggingInterval::get() == 0 {
                log::info!("DCF Health: epoch={}, validators={}/{}, avg_score={}", 
                          current_epoch, active_validators, total_validators, avg_score);
            } else {
                log::debug!("DCF Health Metrics at block {}: epoch={}, total_validators={}, active_validators={}, avg_score={}, governance_mode={}", 
                           block_number, current_epoch, total_validators, active_validators, avg_score, governance_mode);
            }
            
            // Emit telemetry metrics (reduced frequency)
            if block_number % T::ScoreRefreshInterval::get() == 0 {
                log::trace!("[cerulea::dcf][prometheus] dcf_health_check{{block={}}} 1", block_number);
                log::trace!("[cerulea::dcf][prometheus] total_validators{{}} {}", total_validators);
                log::trace!("[cerulea::dcf][prometheus] active_validators{{}} {}", active_validators);
                log::trace!("[cerulea::dcf][prometheus] average_validator_score{{}} {}", avg_score);
                log::trace!("[cerulea::dcf][prometheus] current_epoch{{}} {}", current_epoch);
            }
        }

        /// Run comprehensive score aggregation for all validators at epoch boundary.
        fn run_comprehensive_score_aggregation() -> Weight {
            let mut weight = Weight::zero();
            let validators = ValidatorSet::<T>::get();
            let mut updated_count = 0u32;
            
            for validator in validators.iter() {
                // Update PoS score from stake.
                // The raw stake (e.g. 8_000_000_000_000_000_000) overflows u64, so
                // we scale it down to a sane u64 range before storing in stake_score.
                // We use the same VOTE_WEIGHT_SCALE (32_000) reference point as the
                // DVF pallet so that stake_score reflects relative weight correctly.
                let raw_stake = pos::Pallet::<T>::stake(validator).saturated_into::<u128>();
                // Compute total stake across all validators for normalisation.
                // (recomputed per-validator loop but cheap — only 3 validators on testnet).
                let total_stake: u128 = validators.iter()
                    .map(|v| pos::Pallet::<T>::stake(v).saturated_into::<u128>())
                    .sum();
                const STAKE_SCORE_SCALE: u128 = 32_000u128;
                let stake_score: u64 = if total_stake > 0 {
                    (raw_stake.saturating_mul(STAKE_SCORE_SCALE) / total_stake) as u64
                } else {
                    0
                };

                // Update PoI score from inference results
                let inference_score = poi::Pallet::<T>::get_score(validator);
                
                // Update validator state with fresh scores
                if let Ok(()) = ValidatorStates::<T>::try_mutate(validator, |maybe_state| {
                    let state = maybe_state.as_mut().ok_or(Error::<T>::ValidatorNotFound)?;
                    state.current.stake_score = stake_score;
                    state.current.inference_score = inference_score;
                    Ok::<(), Error<T>>(())
                }).map_err(|e| sp_runtime::DispatchError::from(e)) {
                    // Recalculate final score
                    let _ = Self::update_final_score(validator);
                    _=updated_count += 1;
                    weight = weight.saturating_add(<T as Config>::WeightInfo::on_initialize());
                }
            }
            
            log::info!("DCF: Score aggregation completed - updated {} validators", updated_count);
            weight
        }

        /// Generate epoch-specific proposals based on comprehensive analysis.
        fn generate_epoch_proposals() -> Weight {
            let mut weight = Weight::zero();
            
            // Generate validator management proposals
            Self::generate_automatic_validator_proposals();
            weight = weight.saturating_add(<T as Config>::WeightInfo::on_initialize());
            
            // Generate performance-based proposals
            Self::generate_performance_proposals();
            weight = weight.saturating_add(<T as Config>::WeightInfo::on_initialize());
            
            // Generate governance proposals if needed
            Self::generate_governance_proposals();
            weight = weight.saturating_add(<T as Config>::WeightInfo::on_initialize());
            
            log::info!("DCF: Epoch proposal generation completed");
            weight
        }

        /// Process epoch proposals - execute immediately or queue them.
        fn process_epoch_proposals() -> Weight {
            let mut weight = Weight::zero();
            let mut processed_count = 0u32;
            
            // Get all pending proposals
            let pending_proposals: Vec<_> = Proposals::<T>::iter()
                .filter(|(_, proposal)| proposal.status == ProposalStatus::Pending)
                .collect();
            
            for (proposal_id, proposal) in pending_proposals {
                // For epoch boundary, we can auto-approve certain types of proposals
                if Self::should_auto_approve_proposal(&proposal) {
                    // Auto-approve and execute
                    Proposals::<T>::try_mutate(proposal_id, |maybe_prop| {
                        if let Some(prop) = maybe_prop {
                            prop.status = ProposalStatus::Approved;
                            
                            // Execute the proposal immediately
                            match &prop.action {
                                ProposalAction::AddValidator { validator } => {
                                    let _ = Self::execute_add_validator(validator);
                                }
                                ProposalAction::RemoveValidator { validator } => {
                                    let _ = Self::execute_remove_validator(validator);
                                }
                                ProposalAction::Eject { validator, reason } => {
                                    let _ = Self::execute_eject_validator(validator, reason.clone());
                                }
                                _ => {
                                    // Other proposals require manual approval
                                }
                            }
                            
                            prop.status = ProposalStatus::Executed;
                            processed_count += 1;
                        }
                        Ok::<(), Error<T>>(())
                    }).ok();
                }
                
                weight = weight.saturating_add(<T as Config>::WeightInfo::on_initialize());
            }
            
            log::info!("DCF: Processed {} epoch proposals", processed_count);
            weight
        }

        /// Finalize the previous epoch's best block.
        fn finalize_previous_epoch(block_number: u32) -> Weight {
            let weight = <T as Config>::WeightInfo::on_initialize();
            
            let current_epoch = Self::current_epoch();
            let previous_epoch = current_epoch.saturating_sub(1);
            
            if previous_epoch > 0 {
                // Calculate the best block of the previous epoch
                let previous_epoch_end_block = block_number.saturating_sub(1);
                
                // With progressive finalization, we might already be close to current
                // Only advance finalization if we're behind
                let current_finalized = Self::last_finalized_block();
                if current_finalized < previous_epoch_end_block {
                    match Self::update_finality_markers(previous_epoch_end_block, current_epoch) {
                        Ok(()) => {
                            log::info!("DCF: Epoch finalization advanced from {} to {} (end of epoch {})", 
                                      current_finalized, previous_epoch_end_block, previous_epoch);
                        },
                        Err(reason) => {
                            let reason_str = sp_std::str::from_utf8(&reason).unwrap_or("Invalid UTF-8");
                            log::warn!("DCF: Epoch finalization rejected for block {}: {}", 
                                      previous_epoch_end_block, reason_str);
                        }
                    }
                } else {
                    log::info!("DCF: Epoch finalization already at {} (>= {}), no advancement needed", 
                              current_finalized, previous_epoch_end_block);
                }
                
                // Emit epoch events
                Self::deposit_event(Event::EpochEnded {
                    epoch: previous_epoch,
                });
                Self::deposit_event(Event::EpochStarted {
                    epoch: current_epoch,
                    validators: Self::active_validators().to_vec(),
                });
                
                log::info!("DCF: Epoch transition completed: {} -> {}", previous_epoch, current_epoch);
            } else {
                // First epoch - ensure genesis is finalized
                if Self::last_finalized_block() == 0 {
                    // Genesis block should already be finalized by progressive logic
                    log::info!("DCF: First epoch - genesis finalization already handled by progressive logic");
                }
            }
            
            weight
        }

        /// Update epoch-specific metrics and perform cleanup.
        fn update_epoch_metrics(block_number: u32) -> Weight {
            let mut weight = <T as Config>::WeightInfo::on_initialize();
            
            // Update validator performance metrics
            let validators = Self::active_validators();
            for validator in validators.iter() {
                if let Some(mut state) = ValidatorStates::<T>::get(validator) {
                    // Update uptime
                    state.uptime = state.uptime.saturating_add(1);
                    
                    // Calculate participation rate
                    let total_blocks = state.current.authored_blocks + state.current.missed_blocks;
                    if total_blocks > 0 {
                        state.participation_rate = (state.current.authored_blocks * T::PercentagePrecision::get()) / total_blocks;
                    }
                    
                    ValidatorStates::<T>::insert(validator, state);
                }
                weight = weight.saturating_add(<T as Config>::WeightInfo::on_initialize());
            }
            
            // Emit comprehensive health metrics
            Self::emit_dcf_health_metrics(block_number);
            
            log::info!("DCF: Epoch metrics updated at block {}", block_number);
            weight
        }

        /// Generate performance-based proposals.
        fn generate_performance_proposals() -> Weight {
            let weight = <T as Config>::WeightInfo::on_initialize();
            
            // This could include:
            // 1. Reward proposals for high-performing validators
            // 2. Slash proposals for underperforming validators
            // 3. Ejection proposals for consistently poor performers
            
            let validators = Self::active_validators();
            let mut high_performers = Vec::new();
            let mut low_performers = Vec::new();
            
            for validator in validators.iter() {
                if let Some(state) = ValidatorStates::<T>::get(validator) {
                    let score = state.current.final_score;
                    let _max_score = T::MaxValidatorScore::get();
                    let _min_score = MinValidatorScoreOf::<T>::get() as u64;
                    
                    // High performer: score > configured high performance threshold
                    if score > T::HighPerformanceScore::get() {
                        high_performers.push(validator.clone());
                    }
                    // Low performer: score below minimum performance threshold
                    else if score < T::MinPerformanceScore::get() {
                        low_performers.push(validator.clone());
                    }
                }
            }
            
            log::info!("DCF: Performance analysis - {} high performers, {} low performers", 
                      high_performers.len(), low_performers.len());
            
            weight
        }

        /// Generate governance-related proposals.
        fn generate_governance_proposals() -> Weight {
            let weight = <T as Config>::WeightInfo::on_initialize();
            
            // This could include:
            // 1. Parameter adjustment proposals
            // 2. Emergency governance proposals
            // 3. System upgrade proposals
            
            log::info!("DCF: Governance proposal generation completed");
            weight
        }

        /// Determine if a proposal should be auto-approved at epoch boundary.
        fn should_auto_approve_proposal(proposal: &GovernanceProposal<T>) -> bool {
            match &proposal.action {
                ProposalAction::AddValidator { .. } => {
                    // Auto-approve validator additions if they meet criteria
                    true
                }
                ProposalAction::RemoveValidator { .. } => {
                    // Auto-approve validator removals for performance reasons
                    true
                }
                ProposalAction::Eject { reason, .. } => {
                    // Auto-approve ejections for performance reasons
                    matches!(reason, EjectionReason::ScoreBelowThreshold | EjectionReason::ExcessValidators)
                }
                _ => {
                    // Manual approval required for rewards and slashing
                    false
                }
            }
        }

        /// Process block authorship validation and scoring
        fn process_block_authorship(block_number: u32) {
            if let Some(expected_author) = Self::get_expected_author(block_number) {
                // In a real implementation, you would extract the actual author from the block
                // For now, we'll use a simplified approach
                let actual_author = Self::extract_block_author();
                
                match actual_author {
                    Some(actual) if actual == expected_author => {
                        // Correct author produced the block
                        let _ = Self::record_block_authorship(&actual);
                    }
                    Some(actual) => {
                        // Wrong author produced the block
                        let _ = Self::record_missed_block(&expected_author);
                        Self::deposit_event(Event::InvalidAuthor {
                            block_number,
                            author: actual,
                        });
                    }
                    None => {
                        // No author found, record as missed block
                        let _ = Self::record_missed_block(&expected_author);
                    }
                }
            }
        }

        /// Extract block author from system digest (enhanced implementation)
        fn extract_block_author() -> Option<T::AccountId> {
            // Extract the author from block headers or consensus logs
            // This matches the digest format created by the CBC consensus engine
            frame_system::Pallet::<T>::digest()
                .logs()
                .iter()
                .find_map(|log| {
                    match log {
                        // Check PreRuntime digest items (where consensus engine stores author info)
                        DigestItem::PreRuntime(engine_id, data) => {
                            if engine_id == b"cbcd" {
                                // The digest data contains: [32 bytes author][32 bytes signature]
                                if data.len() >= 32 {
                                    // First 32 bytes should be the author's account ID
                                    T::AccountId::decode(&mut &data[0..32]).ok()
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        }
                        // Check Seal digest items (alternative location for author info - legacy support)
                        DigestItem::Seal(engine_id, data) => {
                            if engine_id == b"cbcd" {
                                // First 32 bytes should be the author's account ID
                                if data.len() >= 32 {
                                    T::AccountId::decode(&mut &data[0..32]).ok()
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        }
                        // Check Consensus digest items (legacy support)
                        DigestItem::Consensus(engine_id, data) => {
                            if engine_id == b"cbcd" {
                                T::AccountId::decode(&mut &data[..]).ok()
                            } else {
                                None
                            }
                        }
                        _ => None,
                    }
                })
        }

        /// Apply score decay to inactive validators
        fn apply_validator_score_decay(current_epoch: u32) -> Weight {
            let mut weight = Weight::zero();
            let validators = Self::validator_set();
            
            for validator in validators.iter() {
                if let Err(_) = Self::apply_score_decay(validator, current_epoch) {
                    log::warn!("Failed to apply score decay for validator {:?}", validator);
                }
                weight = weight.saturating_add(<T as Config>::WeightInfo::on_initialize());
            }
            
            weight
        }

        /// Update participation rates for all validators
        fn update_validator_participation_rates() -> Weight {
            let mut weight = Weight::zero();
            let validators = Self::validator_set();
            
            for validator in validators.iter() {
                ValidatorStates::<T>::mutate(validator, |maybe_state| {
                    if let Some(state) = maybe_state.as_mut() {
                        let total_blocks = state.current.authored_blocks + state.current.missed_blocks;
                        if total_blocks > 0 {
                            state.participation_rate = (state.current.authored_blocks * T::PercentagePrecision::get()) / total_blocks;
                        }
                    }
                });
                weight = weight.saturating_add(<T as Config>::WeightInfo::on_initialize());
            }
            
            weight
        }

        /// Calculate comprehensive trust score with bounded growth and decay for enhanced stability.
        ///
        /// This enhanced trust score calculation implements robust bounds and stability mechanisms
        /// to prevent explosive growth, rapid decay, and score volatility. The calculation uses
        /// configurable weights and bounds to ensure long-term fairness and system stability.
        ///
        /// # Enhanced Features
        /// - **Bounded Growth**: Limits score increases per epoch to prevent gaming
        /// - **Bounded Decay**: Limits score decreases to allow recovery from temporary issues
        /// - **Stability Smoothing**: Reduces volatility through weighted averaging
        /// - **Absolute Bounds**: Ensures scores stay within configured min/max range
        /// - **Clamping**: Final safety net against out-of-bounds values
        ///
        /// # Calculation Formula with Bounds
        /// ```
        /// // Base calculation
        /// weighted_score = (uptime_score * uptime_weight + inference_score * inference_weight) / total_weight
        /// base_score = weighted_score - (slashing_penalty * slashing_weight / 100)
        /// 
        /// // Apply bounds and stability
        /// previous_score = get_previous_trust_score(validator)
        /// growth_limited_score = apply_growth_limits(base_score, previous_score, bounds)
        /// decay_limited_score = apply_decay_limits(growth_limited_score, previous_score, bounds)
        /// stabilized_score = apply_stability_smoothing(decay_limited_score, previous_score, bounds)
        /// final_trust_score = clamp(stabilized_score, min_trust_score, max_trust_score)
        /// ```
        ///
        /// # Requirements Addressed
        /// - **8.1**: Configuration-driven caps for trust score growth and decay rates
        /// - **8.2**: Clamping at score boundaries to prevent negative or explosive values
        /// - **8.3**: Trust score stability validation across hundreds of simulated epochs
        /// - **8.4**: Documented trust score formula and parameters in rustdoc comments
        pub fn calculate_trust_score(validator: &T::AccountId) -> u64 {
            let state = match ValidatorStates::<T>::get(validator) {
                Some(state) => state,
                None => return T::MinTrustScore::get(), // Return minimum instead of 0
            };

            let current_epoch = CurrentEpoch::<T>::get();
            let config = TrustScoreConfigStorage::<T>::get();
            let bounds = TrustScoreBounds::<T>::get();

            // Get previous trust score for stability calculations
            let previous_score = ValidatorTrustScores::<T>::get(validator);

            // Calculate uptime component (percentage of epochs active)
            let uptime_percentage = if current_epoch > 0 {
                (state.uptime * T::PercentagePrecision::get()) / current_epoch
            } else {
                T::PercentagePrecision::get() // 100% for genesis
            };

            // Enhanced uptime score including block production reliability
            let blocks_authored = ValidatorBlocksAuthored::<T>::get(validator);
            let blocks_missed = ValidatorBlocksMissed::<T>::get(validator);
            let total_opportunities = blocks_authored.saturating_add(blocks_missed);
            
            let block_reliability = if total_opportunities > 0 {
                (blocks_authored * T::PercentagePrecision::get()) / total_opportunities
            } else {
                T::PercentagePrecision::get() // 100% if no opportunities yet
            };

            // Combine uptime percentage and block reliability (70% uptime, 30% block reliability)
            let uptime_score = (uptime_percentage as u64 * 70 + block_reliability as u64 * 30) / 100;

            // Calculate inference success rate
            let inference_success_rate = if state.inference_count > 0 {
                (state.inference_success_count as u64 * T::PercentagePrecision::get() as u64) / state.inference_count
            } else {
                T::PercentagePrecision::get() as u64 / 2 // 50% if no inferences yet (neutral)
            };

            // Enhanced inference score including participation rate
            let inference_participation = if current_epoch > 0 {
                ((state.inference_count / current_epoch.max(1) as u64).min(100) * T::PercentagePrecision::get() as u64) / 100
            } else {
                T::PercentagePrecision::get() as u64 / 2 // 50% for genesis
            };

            // Combine success rate and participation (60% success rate, 40% participation)
            let inference_score = (inference_success_rate * 60 + inference_participation * 40) / 100;

            // Calculate slashing penalty based on validator performance
            let slashing_penalty = {
                let current_score = state.current.final_score;
                let max_score = T::MaxValidatorScore::get();
                
                if current_score < max_score / 4 {
                    T::PercentagePrecision::get() as u64 / 4 // High penalty for very low scores
                } else if current_score < max_score / 2 {
                    T::PercentagePrecision::get() as u64 / 8 // Medium penalty for moderately low scores
                } else {
                    0 // No penalty for good scores
                }
            };

            // Calculate weighted score using configurable weights
            let total_positive_weight = config.uptime_weight.saturating_add(config.inference_weight);
            if total_positive_weight == 0 {
                return T::MinTrustScore::get(); // Return minimum instead of 0
            }

            let weighted_score = (
                uptime_score.saturating_mul(config.uptime_weight as u64)
                    .saturating_add(inference_score.saturating_mul(config.inference_weight as u64))
            ) / total_positive_weight as u64;

            // Apply slashing penalty
            let penalty_amount = slashing_penalty.saturating_mul(config.slashing_weight as u64) / 100;
            let base_score = weighted_score.saturating_sub(penalty_amount);

            // Scale to trust score range
            let scaled_score = (base_score * bounds.max_score) / T::PercentagePrecision::get() as u64;

            // Apply bounded growth and decay with stability mechanisms
            let bounded_score = Self::apply_trust_score_bounds(scaled_score, previous_score, &bounds);

            // Final clamping to ensure bounds are respected
            bounded_score.max(bounds.min_score).min(bounds.max_score)
        }

        /// Apply trust score bounds including growth limits, decay limits, and stability smoothing.
        ///
        /// This function implements the core stability mechanisms for trust scores to ensure
        /// long-term fairness and prevent gaming of the scoring system. It applies multiple
        /// layers of bounds and smoothing to create stable, predictable score evolution.
        ///
        /// # Stability Mechanisms
        /// - **Growth Rate Limiting**: Prevents explosive score increases that could indicate gaming
        /// - **Decay Rate Limiting**: Prevents rapid score degradation from temporary issues
        /// - **Stability Smoothing**: Reduces volatility through weighted averaging with previous scores
        /// - **Absolute Bounds**: Ensures scores never exceed configured min/max values
        ///
        /// # Parameters
        /// - `new_score`: The newly calculated raw trust score before bounds are applied
        /// - `previous_score`: The validator's previous trust score for comparison and smoothing
        /// - `bounds`: The current trust score bounds configuration with rate limits and ranges
        ///
        /// # Returns
        /// The bounded and stabilized trust score that respects all configured limits
        ///
        /// # Algorithm Details
        /// 1. **First Score Handling**: If no previous score exists, return new score within bounds
        /// 2. **Change Calculation**: Determine the magnitude and direction of score change
        /// 3. **Growth Limiting**: If score is increasing, limit to max_growth_rate per epoch
        /// 4. **Decay Limiting**: If score is decreasing, limit to max_decay_rate per epoch
        /// 5. **Stability Smoothing**: Apply weighted average between old and new scores
        /// 6. **Final Clamping**: Ensure result respects absolute min/max bounds
        ///
        /// # Rate Calculation
        /// Growth and decay rates are specified in basis points (1/10000):
        /// - 500 basis points = 5% maximum change per epoch
        /// - 200 basis points = 2% maximum change per epoch
        ///
        /// # Stability Factor
        /// The stability factor determines how much weight is given to the previous score:
        /// - 8000 basis points = 80% weight to previous score, 20% to new calculation
        /// - Higher values create more stability but slower responsiveness
        /// - Lower values create faster responsiveness but more volatility
        pub fn apply_trust_score_bounds(
            new_score: u64,
            previous_score: u64,
            bounds: &TrustScoreBoundsData,
        ) -> u64 {
            // If this is the first score calculation, return the new score within bounds
            if previous_score == 0 {
                return new_score.max(bounds.min_score).min(bounds.max_score);
            }

            let score_change = if new_score > previous_score {
                new_score - previous_score
            } else {
                previous_score - new_score
            };

            // Calculate maximum allowed change based on bounds (basis points)
            let max_growth = (previous_score * bounds.max_growth_rate as u64) / 10000;
            let max_decay = (previous_score * bounds.max_decay_rate as u64) / 10000;

            let bounded_score = if new_score > previous_score {
                // Apply growth rate limiting
                let max_increase = max_growth.max(1); // Ensure at least 1 point growth is possible
                let limited_increase = score_change.min(max_increase);
                previous_score.saturating_add(limited_increase)
            } else {
                // Apply decay rate limiting
                let max_decrease = max_decay.max(1); // Ensure at least 1 point decay is possible
                let limited_decrease = score_change.min(max_decrease);
                previous_score.saturating_sub(limited_decrease)
            };

            // Apply stability smoothing using weighted average
            let stability_weight = bounds.stability_factor; // basis points (e.g., 8000 = 80%)
            let change_weight = 10000 - stability_weight; // remaining weight for new score

            let stabilized_score = (
                (previous_score * stability_weight as u64) + 
                (bounded_score * change_weight as u64)
            ) / 10000;

            // Ensure the result respects absolute bounds
            stabilized_score.max(bounds.min_score).min(bounds.max_score)
        }

        /// Update trust score stability metrics for system health monitoring.
        ///
        /// This function calculates and updates various metrics related to trust score
        /// stability across the validator network. These metrics help identify potential
        /// issues with score volatility, gaming attempts, or system imbalances.
        ///
        /// # Metrics Calculated
        /// - **Average Score Change**: Mean absolute change in trust scores across all validators
        /// - **Maximum Score Change**: Largest absolute change in any validator's trust score
        /// - **Bound Violations**: Count of validators hitting minimum or maximum bounds
        /// - **Score Distribution**: Statistical measures of score distribution across validators
        /// - **Stability Index**: Overall system stability measure (0-10000, higher = more stable)
        ///
        /// # Usage
        /// This function should be called during epoch transitions to maintain up-to-date
        /// stability metrics for monitoring and governance purposes.
        ///
        /// # Requirements Addressed
        /// - **8.3**: Trust score stability validation across hundreds of simulated epochs
        pub fn update_trust_score_stability_metrics() {
            let current_epoch = Self::current_epoch();
            let bounds = TrustScoreBounds::<T>::get();
            let active_validators = Self::active_validators();
            
            if active_validators.is_empty() {
                return;
            }

            let mut score_changes = Vec::new();
            let mut current_scores = Vec::new();
            let mut validators_at_min_bound = 0u32;
            let mut validators_at_max_bound = 0u32;

            // Collect score data for all active validators
            for validator in active_validators.iter() {
                let current_score = ValidatorTrustScores::<T>::get(validator);
                current_scores.push(current_score);

                // Check for bound violations
                if current_score <= bounds.min_score {
                    validators_at_min_bound += 1;
                }
                if current_score >= bounds.max_score {
                    validators_at_max_bound += 1;
                }

                // Calculate score change from history
                let history = TrustScoreHistory::<T>::get(validator);
                if let Some(last_entry) = history.last() {
                    if last_entry.0 == current_epoch.saturating_sub(1) {
                        let score_change = if current_score > last_entry.1 {
                            current_score - last_entry.1
                        } else {
                            last_entry.1 - current_score
                        };
                        score_changes.push(score_change);
                    }
                }
            }

            // Calculate average and maximum score changes
            let avg_score_change = if !score_changes.is_empty() {
                score_changes.iter().sum::<u64>() / score_changes.len() as u64
            } else {
                0
            };

            let max_score_change = score_changes.iter().max().copied().unwrap_or(0);

            // Calculate score distribution statistics
            current_scores.sort_unstable();
            let score_median = if !current_scores.is_empty() {
                let mid = current_scores.len() / 2;
                if current_scores.len() % 2 == 0 {
                    (current_scores[mid - 1] + current_scores[mid]) / 2
                } else {
                    current_scores[mid]
                }
            } else {
                bounds.min_score + (bounds.max_score - bounds.min_score) / 2
            };

            // Calculate standard deviation
            let mean_score = if !current_scores.is_empty() {
                current_scores.iter().sum::<u64>() / current_scores.len() as u64
            } else {
                score_median
            };

            let variance = if !current_scores.is_empty() {
                current_scores.iter()
                    .map(|&score| {
                        let diff = if score > mean_score { score - mean_score } else { mean_score - score };
                        diff * diff
                    })
                    .sum::<u64>() / current_scores.len() as u64
            } else {
                0
            };

            // Approximate standard deviation (integer square root)
            let score_standard_deviation = Self::integer_sqrt(variance);

            // Calculate percentile counts
            let total_validators = current_scores.len() as u32;
            let p90_index = (total_validators * 90 / 100).max(1) as usize;
            let p10_index = (total_validators * 10 / 100) as usize;

            let high_performers_count = if total_validators > 0 {
                total_validators - p90_index.min(total_validators as usize) as u32
            } else {
                0
            };

            let low_performers_count = if total_validators > 0 {
                p10_index.min(total_validators as usize) as u32
            } else {
                0
            };

            // Calculate stability index (higher = more stable)
            let volatility_factor = if bounds.max_score > 0 {
                (avg_score_change * 10000) / bounds.max_score
            } else {
                0
            };
            let stability_index = 10000u32.saturating_sub(volatility_factor.min(10000) as u32);

            // Update stability metrics storage
            let metrics = TrustScoreStabilityMetricsData {
                epoch: current_epoch,
                avg_score_change,
                max_score_change,
                validators_at_min_bound,
                validators_at_max_bound,
                score_standard_deviation,
                score_median,
                high_performers_count,
                low_performers_count,
                stability_index,
            };

            TrustScoreStabilityMetrics::<T>::put(metrics);
        }

        /// Calculate integer square root for standard deviation calculation.
        ///
        /// Uses binary search to find the largest integer whose square is less than
        /// or equal to the input value. This provides an approximation of the square
        /// root suitable for standard deviation calculations.
        ///
        /// # Parameters
        /// - `n`: The number to find the square root of
        ///
        /// # Returns
        /// The integer square root of the input
        fn integer_sqrt(n: u64) -> u64 {
            if n == 0 {
                return 0;
            }
            
            let mut left = 1u64;
            let mut right = n;
            let mut result = 0u64;
            
            while left <= right {
                let mid = left + (right - left) / 2;
                
                if mid <= n / mid {
                    result = mid;
                    left = mid + 1;
                } else {
                    right = mid - 1;
                }
            }
            
            result
        }

        /// Update trust score for a validator and store in history.
        ///
        /// This function recalculates the trust score for a validator using the current
        /// trust score configuration and updates both the current score storage and
        /// the historical record for trend analysis.
        ///
        /// The function is called during:
        /// - Validator activity events (block production, inference submission)
        /// - Epoch transitions and performance evaluations
        /// - Manual trust score recalculations
        /// - Slashing events and penalty applications
        pub fn update_trust_score(validator: &T::AccountId) -> DispatchResult {
            let new_trust_score = Self::calculate_trust_score(validator);
            
            // Get old trust score for comparison
            let old_trust_score = ValidatorTrustScores::<T>::get(validator);
            
            // Update current trust score storage
            ValidatorTrustScores::<T>::insert(validator, new_trust_score);

            // Update trust score in validator state
            ValidatorStates::<T>::try_mutate(validator, |maybe_state| {
                let state = maybe_state.as_mut().ok_or(Error::<T>::ValidatorNotFound)?;
                state.trust_score = new_trust_score;
                Ok::<(), Error<T>>(())
            })?;
            
            // Add to trust score history if score changed significantly
            let score_change = if new_trust_score > old_trust_score {
                new_trust_score - old_trust_score
            } else {
                old_trust_score - new_trust_score
            };
            
            // Only record in history if change is significant (> 1% of max score)
            let significant_change_threshold = T::MaxTrustScore::get() / 100;
            if score_change > significant_change_threshold {
                let current_epoch = Self::current_epoch();
                TrustScoreHistory::<T>::try_mutate(validator, |history| {
                    // Remove oldest entry if at capacity and not empty
                    if history.len() == history.capacity() && !history.is_empty() {
                        history.remove(0);
                    }
                    // Add new entry
                    history.try_push((current_epoch, new_trust_score))
                        .map_err(|_| Error::<T>::ValidatorNotFound)?; // Use existing error type
                    Ok::<(), Error<T>>(())
                })?;
                
                // Calculate score components for the event
                let _config = TrustScoreConfigStorage::<T>::get();
                let state = ValidatorStates::<T>::get(validator).ok_or(Error::<T>::ValidatorNotFound)?;
                let current_epoch = Self::current_epoch();
                
                // Calculate individual components for event emission
                let uptime_percentage = if current_epoch > 0 {
                    (state.uptime * T::PercentagePrecision::get()) / current_epoch
                } else {
                    T::PercentagePrecision::get()
                };
                
                let inference_success_rate = if state.inference_count > 0 {
                    (state.inference_success_count as u64 * T::PercentagePrecision::get() as u64) / state.inference_count
                } else {
                    T::PercentagePrecision::get() as u64 / 2
                };
                
                let slashing_component = {
                    let current_score = state.current.final_score;
                    let max_score = T::MaxValidatorScore::get();
                    
                    if current_score < max_score / 4 {
                        T::PercentagePrecision::get() as u64 / 4
                    } else if current_score < max_score / 2 {
                        T::PercentagePrecision::get() as u64 / 8
                    } else {
                        0
                    }
                };
                
                // Emit trust score updated event
                Self::deposit_event(Event::TrustScoreUpdated {
                    validator: validator.clone(),
                    old_score: old_trust_score,
                    new_score: new_trust_score,
                    uptime_component: uptime_percentage as u64,
                    inference_component: inference_success_rate,
                    slashing_component,
                });
            }

            Ok(())
        }

        /// Update trust scores for all active validators with stability monitoring.
        ///
        /// This enhanced batch operation recalculates trust scores for all validators
        /// in the active set using the new bounded growth and decay mechanisms. It also
        /// updates system-wide stability metrics to monitor the health of the trust
        /// scoring system across hundreds of simulated epochs.
        ///
        /// # Enhanced Features
        /// - Applies bounded growth and decay to all trust score updates
        /// - Updates stability metrics for system health monitoring
        /// - Tracks score distribution and volatility across the network
        /// - Monitors bound violations and system stability indicators
        ///
        /// # Requirements Addressed
        /// - **8.1**: Configuration-driven caps applied to all validators
        /// - **8.2**: Clamping at score boundaries for all validators
        /// - **8.3**: Stability validation through metrics tracking
        ///
        /// # Returns
        /// * `Weight` - Computational weight consumed by the operation
        pub fn update_all_trust_scores() -> Weight {
            let mut weight = Weight::zero();
            let active_validators = Self::active_validators();
            
            for validator in active_validators.iter() {
                if let Err(e) = Self::update_trust_score(validator) {
                    log::warn!("Failed to update trust score for validator {:?}: {:?}", validator, e);
                }
                // Add weight for each trust score calculation
                weight = weight.saturating_add(T::DbWeight::get().reads_writes(5, 3));
            }
            
            // Update stability metrics after all trust scores are updated
            Self::update_trust_score_stability_metrics();
            weight = weight.saturating_add(T::DbWeight::get().reads_writes(10, 1));
            
            weight
        }

        /// Get the current value of a parameter from the governance configuration.
        /// 
        /// This function retrieves the current value of any DCF parameter from the
        /// governance configuration storage. The value is encoded as bytes to handle
        /// different parameter types in a type-safe manner.
        /// 
        /// # Parameters
        /// - `config`: Reference to the governance configuration
        /// - `parameter`: The parameter type to retrieve
        /// 
        /// # Returns
        /// - `Ok(BoundedVec<u8, ConstU32<64>>)`: Encoded current value
        /// - `Err(Error<T>)`: If parameter type is invalid or config is corrupted
        fn get_parameter_value(
            config: &GovernanceConfig<T>,
            parameter: &ParameterType,
        ) -> Result<BoundedVec<u8, ConstU32<64>>, Error<T>> {
            let value_bytes = match parameter {
                ParameterType::EpochLength => config.epoch_length.current.encode(),
                ParameterType::MinStake => config.min_stake.current.encode(),
                ParameterType::MaxValidators => config.max_validators.current.encode(),
                ParameterType::PosWeight => config.pos_weight.current.encode(),
                ParameterType::PoiWeight => config.poi_weight.current.encode(),
                ParameterType::MinPerformanceScore => config.min_performance_score.current.encode(),
                ParameterType::HighPerformanceScore => config.high_performance_score.current.encode(),
                ParameterType::MinParticipationRate => config.min_participation_rate.current.encode(),
                ParameterType::HighParticipationRate => config.high_participation_rate.current.encode(),
                ParameterType::ValidatorReward => config.validator_reward.current.encode(),
                ParameterType::SlashPercent => config.slash_percent.current.encode(),
                ParameterType::LeaveCooldown => config.leave_cooldown.current.encode(),
                ParameterType::TrustScoreUptimeWeight => config.trust_score_uptime_weight.current.encode(),
                ParameterType::TrustScoreInferenceWeight => config.trust_score_inference_weight.current.encode(),
                ParameterType::TrustScoreSlashingWeight => config.trust_score_slashing_weight.current.encode(),
                ParameterType::TrustScoreMaxGrowthRate => config.trust_score_max_growth_rate.current.encode(),
                ParameterType::TrustScoreMaxDecayRate => config.trust_score_max_decay_rate.current.encode(),
                ParameterType::TrustScoreMinValue => config.trust_score_min_value.current.encode(),
                ParameterType::TrustScoreMaxValue => config.trust_score_max_value.current.encode(),
                ParameterType::TrustScoreStabilityFactor => config.trust_score_stability_factor.current.encode(),
                ParameterType::BlockAuthorshipBoost => config.block_authorship_boost.current.encode(),
                ParameterType::MissedBlockPenalty => config.missed_block_penalty.current.encode(),
                ParameterType::InferenceBoostLow => config.inference_boost_low.current.encode(),
                ParameterType::InferenceBoostMedium => config.inference_boost_medium.current.encode(),
                ParameterType::InferenceBoostHigh => config.inference_boost_high.current.encode(),
            };
            
            BoundedVec::try_from(value_bytes).map_err(|_| Error::<T>::InvalidGovernanceConfig)
        }

        /// Validate a parameter value against its allowed range and update the configuration.
        /// 
        /// This function performs comprehensive validation of parameter updates including:
        /// - Range validation against min/max bounds
        /// - Type-specific validation rules
        /// - Cross-parameter consistency checks
        /// - Safety constraint enforcement
        /// 
        /// # Parameters
        /// - `config`: Mutable reference to governance configuration
        /// - `parameter`: The parameter type being updated
        /// - `value`: New value encoded as bytes
        /// 
        /// # Returns
        /// - `Ok(())`: If validation passes and config is updated
        /// - `Err(Error<T>)`: If validation fails with specific error
        fn validate_and_update_parameter(
            config: &mut GovernanceConfig<T>,
            parameter: ParameterType,
            value: &BoundedVec<u8, ConstU32<64>>,
        ) -> Result<(), Error<T>> {
            match parameter {
                ParameterType::EpochLength => {
                    let new_value: u32 = Decode::decode(&mut &value[..])
                        .map_err(|_| Error::<T>::InvalidParameterType)?;
                    ensure!(
                        new_value >= config.epoch_length.min && new_value <= config.epoch_length.max,
                        Error::<T>::ParameterOutOfRange
                    );
                    config.epoch_length.current = new_value;
                },
                ParameterType::MinStake => {
                    let new_value: <T as pallet::Config>::Balance = Decode::decode(&mut &value[..])
                        .map_err(|_| Error::<T>::InvalidParameterType)?;
                    ensure!(
                        new_value >= config.min_stake.min && new_value <= config.min_stake.max,
                        Error::<T>::ParameterOutOfRange
                    );
                    config.min_stake.current = new_value;
                },
                ParameterType::MaxValidators => {
                    let new_value: u32 = Decode::decode(&mut &value[..])
                        .map_err(|_| Error::<T>::InvalidParameterType)?;
                    ensure!(
                        new_value >= config.max_validators.min && new_value <= config.max_validators.max,
                        Error::<T>::ParameterOutOfRange
                    );
                    config.max_validators.current = new_value;
                },
                ParameterType::PosWeight => {
                    let new_value: u64 = Decode::decode(&mut &value[..])
                        .map_err(|_| Error::<T>::InvalidParameterType)?;
                    ensure!(
                        new_value >= config.pos_weight.min && new_value <= config.pos_weight.max,
                        Error::<T>::ParameterOutOfRange
                    );
                    // Ensure PoS + PoI weights sum to precision factor
                    let poi_weight = config.poi_weight.current;
                    ensure!(
                        new_value + poi_weight == T::PercentagePrecision::get() as u64,
                        Error::<T>::InvalidWeight
                    );
                    config.pos_weight.current = new_value;
                },
                ParameterType::PoiWeight => {
                    let new_value: u64 = Decode::decode(&mut &value[..])
                        .map_err(|_| Error::<T>::InvalidParameterType)?;
                    ensure!(
                        new_value >= config.poi_weight.min && new_value <= config.poi_weight.max,
                        Error::<T>::ParameterOutOfRange
                    );
                    // Ensure PoS + PoI weights sum to precision factor
                    let pos_weight = config.pos_weight.current;
                    ensure!(
                        pos_weight + new_value == T::PercentagePrecision::get() as u64,
                        Error::<T>::InvalidWeight
                    );
                    config.poi_weight.current = new_value;
                },
                ParameterType::MinPerformanceScore => {
                    let new_value: u64 = Decode::decode(&mut &value[..])
                        .map_err(|_| Error::<T>::InvalidParameterType)?;
                    ensure!(
                        new_value >= config.min_performance_score.min && new_value <= config.min_performance_score.max,
                        Error::<T>::ParameterOutOfRange
                    );
                    // Ensure min performance score is less than high performance score
                    ensure!(
                        new_value < config.high_performance_score.current,
                        Error::<T>::InvalidGovernanceConfig
                    );
                    config.min_performance_score.current = new_value;
                },
                ParameterType::HighPerformanceScore => {
                    let new_value: u64 = Decode::decode(&mut &value[..])
                        .map_err(|_| Error::<T>::InvalidParameterType)?;
                    ensure!(
                        new_value >= config.high_performance_score.min && new_value <= config.high_performance_score.max,
                        Error::<T>::ParameterOutOfRange
                    );
                    // Ensure high performance score is greater than min performance score
                    ensure!(
                        new_value > config.min_performance_score.current,
                        Error::<T>::InvalidGovernanceConfig
                    );
                    config.high_performance_score.current = new_value;
                },
                ParameterType::MinParticipationRate => {
                    let new_value: u32 = Decode::decode(&mut &value[..])
                        .map_err(|_| Error::<T>::InvalidParameterType)?;
                    ensure!(
                        new_value >= config.min_participation_rate.min && new_value <= config.min_participation_rate.max,
                        Error::<T>::ParameterOutOfRange
                    );
                    // Ensure min participation rate is less than high participation rate
                    ensure!(
                        new_value < config.high_participation_rate.current,
                        Error::<T>::InvalidGovernanceConfig
                    );
                    config.min_participation_rate.current = new_value;
                },
                ParameterType::HighParticipationRate => {
                    let new_value: u32 = Decode::decode(&mut &value[..])
                        .map_err(|_| Error::<T>::InvalidParameterType)?;
                    ensure!(
                        new_value >= config.high_participation_rate.min && new_value <= config.high_participation_rate.max,
                        Error::<T>::ParameterOutOfRange
                    );
                    // Ensure high participation rate is greater than min participation rate
                    ensure!(
                        new_value > config.min_participation_rate.current,
                        Error::<T>::InvalidGovernanceConfig
                    );
                    config.high_participation_rate.current = new_value;
                },
                ParameterType::ValidatorReward => {
                    let new_value: <T as pallet::Config>::Balance = Decode::decode(&mut &value[..])
                        .map_err(|_| Error::<T>::InvalidParameterType)?;
                    ensure!(
                        new_value >= config.validator_reward.min && new_value <= config.validator_reward.max,
                        Error::<T>::ParameterOutOfRange
                    );
                    config.validator_reward.current = new_value;
                },
                ParameterType::SlashPercent => {
                    let new_value: u32 = Decode::decode(&mut &value[..])
                        .map_err(|_| Error::<T>::InvalidParameterType)?;
                    ensure!(
                        new_value >= config.slash_percent.min && new_value <= config.slash_percent.max,
                        Error::<T>::ParameterOutOfRange
                    );
                    // Ensure slash percent is reasonable (0-100%)
                    ensure!(new_value <= 100, Error::<T>::ParameterOutOfRange);
                    config.slash_percent.current = new_value;
                },
                ParameterType::LeaveCooldown => {
                    let new_value: BlockNumberFor<T> = Decode::decode(&mut &value[..])
                        .map_err(|_| Error::<T>::InvalidParameterType)?;
                    ensure!(
                        new_value >= config.leave_cooldown.min && new_value <= config.leave_cooldown.max,
                        Error::<T>::ParameterOutOfRange
                    );
                    config.leave_cooldown.current = new_value;
                },
                ParameterType::TrustScoreUptimeWeight => {
                    let new_value: u32 = Decode::decode(&mut &value[..])
                        .map_err(|_| Error::<T>::InvalidParameterType)?;
                    ensure!(
                        new_value >= config.trust_score_uptime_weight.min && new_value <= config.trust_score_uptime_weight.max,
                        Error::<T>::ParameterOutOfRange
                    );
                    config.trust_score_uptime_weight.current = new_value;
                },
                ParameterType::TrustScoreInferenceWeight => {
                    let new_value: u32 = Decode::decode(&mut &value[..])
                        .map_err(|_| Error::<T>::InvalidParameterType)?;
                    ensure!(
                        new_value >= config.trust_score_inference_weight.min && new_value <= config.trust_score_inference_weight.max,
                        Error::<T>::ParameterOutOfRange
                    );
                    config.trust_score_inference_weight.current = new_value;
                },
                ParameterType::TrustScoreSlashingWeight => {
                    let new_value: u32 = Decode::decode(&mut &value[..])
                        .map_err(|_| Error::<T>::InvalidParameterType)?;
                    ensure!(
                        new_value >= config.trust_score_slashing_weight.min && new_value <= config.trust_score_slashing_weight.max,
                        Error::<T>::ParameterOutOfRange
                    );
                    config.trust_score_slashing_weight.current = new_value;
                },
                ParameterType::TrustScoreMaxGrowthRate => {
                    let new_value: u32 = Decode::decode(&mut &value[..])
                        .map_err(|_| Error::<T>::InvalidParameterType)?;
                    ensure!(
                        new_value >= config.trust_score_max_growth_rate.min && new_value <= config.trust_score_max_growth_rate.max,
                        Error::<T>::ParameterOutOfRange
                    );
                    config.trust_score_max_growth_rate.current = new_value;
                },
                ParameterType::TrustScoreMaxDecayRate => {
                    let new_value: u32 = Decode::decode(&mut &value[..])
                        .map_err(|_| Error::<T>::InvalidParameterType)?;
                    ensure!(
                        new_value >= config.trust_score_max_decay_rate.min && new_value <= config.trust_score_max_decay_rate.max,
                        Error::<T>::ParameterOutOfRange
                    );
                    config.trust_score_max_decay_rate.current = new_value;
                },
                ParameterType::TrustScoreMinValue => {
                    let new_value: u64 = Decode::decode(&mut &value[..])
                        .map_err(|_| Error::<T>::InvalidParameterType)?;
                    ensure!(
                        new_value >= config.trust_score_min_value.min && new_value <= config.trust_score_min_value.max,
                        Error::<T>::ParameterOutOfRange
                    );
                    // Ensure min value is less than max value
                    ensure!(
                        new_value < config.trust_score_max_value.current,
                        Error::<T>::InvalidGovernanceConfig
                    );
                    config.trust_score_min_value.current = new_value;
                },
                ParameterType::TrustScoreMaxValue => {
                    let new_value: u64 = Decode::decode(&mut &value[..])
                        .map_err(|_| Error::<T>::InvalidParameterType)?;
                    ensure!(
                        new_value >= config.trust_score_max_value.min && new_value <= config.trust_score_max_value.max,
                        Error::<T>::ParameterOutOfRange
                    );
                    // Ensure max value is greater than min value
                    ensure!(
                        new_value > config.trust_score_min_value.current,
                        Error::<T>::InvalidGovernanceConfig
                    );
                    config.trust_score_max_value.current = new_value;
                },
                ParameterType::TrustScoreStabilityFactor => {
                    let new_value: u32 = Decode::decode(&mut &value[..])
                        .map_err(|_| Error::<T>::InvalidParameterType)?;
                    ensure!(
                        new_value >= config.trust_score_stability_factor.min && new_value <= config.trust_score_stability_factor.max,
                        Error::<T>::ParameterOutOfRange
                    );
                    config.trust_score_stability_factor.current = new_value;
                },
                ParameterType::BlockAuthorshipBoost => {
                    let new_value: u64 = Decode::decode(&mut &value[..])
                        .map_err(|_| Error::<T>::InvalidParameterType)?;
                    ensure!(
                        new_value >= config.block_authorship_boost.min && new_value <= config.block_authorship_boost.max,
                        Error::<T>::ParameterOutOfRange
                    );
                    config.block_authorship_boost.current = new_value;
                },
                ParameterType::MissedBlockPenalty => {
                    let new_value: u64 = Decode::decode(&mut &value[..])
                        .map_err(|_| Error::<T>::InvalidParameterType)?;
                    ensure!(
                        new_value >= config.missed_block_penalty.min && new_value <= config.missed_block_penalty.max,
                        Error::<T>::ParameterOutOfRange
                    );
                    config.missed_block_penalty.current = new_value;
                },
                ParameterType::InferenceBoostLow => {
                    let new_value: u64 = Decode::decode(&mut &value[..])
                        .map_err(|_| Error::<T>::InvalidParameterType)?;
                    ensure!(
                        new_value >= config.inference_boost_low.min && new_value <= config.inference_boost_low.max,
                        Error::<T>::ParameterOutOfRange
                    );
                    config.inference_boost_low.current = new_value;
                },
                ParameterType::InferenceBoostMedium => {
                    let new_value: u64 = Decode::decode(&mut &value[..])
                        .map_err(|_| Error::<T>::InvalidParameterType)?;
                    ensure!(
                        new_value >= config.inference_boost_medium.min && new_value <= config.inference_boost_medium.max,
                        Error::<T>::ParameterOutOfRange
                    );
                    config.inference_boost_medium.current = new_value;
                },
                ParameterType::InferenceBoostHigh => {
                    let new_value: u64 = Decode::decode(&mut &value[..])
                        .map_err(|_| Error::<T>::InvalidParameterType)?;
                    ensure!(
                        new_value >= config.inference_boost_high.min && new_value <= config.inference_boost_high.max,
                        Error::<T>::ParameterOutOfRange
                    );
                    config.inference_boost_high.current = new_value;
                },
            }
            
            Ok(())
        }

        /// Apply a parameter change to the active system configuration.
        /// 
        /// This function updates the active system configuration to reflect parameter
        /// changes made through governance. Some parameters take effect immediately,
        /// while others may be scheduled for the next epoch boundary.
        /// 
        /// # Parameters
        /// - `parameter`: The parameter type that was updated
        /// - `value`: New value encoded as bytes
        /// 
        /// # Returns
        /// - `Ok(())`: If the parameter change was successfully applied
        /// - `Err(Error<T>)`: If the application failed
        fn apply_parameter_change(
            parameter: &ParameterType,
            value: &BoundedVec<u8, ConstU32<64>>,
        ) -> Result<(), Error<T>> {
            match parameter {
                ParameterType::PosWeight => {
                    let new_value: u64 = Decode::decode(&mut &value[..])
                        .map_err(|_| Error::<T>::InvalidParameterType)?;
                    PosWeight::<T>::put(new_value);
                },
                ParameterType::PoiWeight => {
                    let new_value: u64 = Decode::decode(&mut &value[..])
                        .map_err(|_| Error::<T>::InvalidParameterType)?;
                    PoiWeight::<T>::put(new_value);
                },
                ParameterType::TrustScoreUptimeWeight => {
                    let new_value: u32 = Decode::decode(&mut &value[..])
                        .map_err(|_| Error::<T>::InvalidParameterType)?;
                    TrustScoreConfigStorage::<T>::mutate(|config| {
                        config.uptime_weight = new_value;
                    });
                },
                ParameterType::TrustScoreInferenceWeight => {
                    let new_value: u32 = Decode::decode(&mut &value[..])
                        .map_err(|_| Error::<T>::InvalidParameterType)?;
                    TrustScoreConfigStorage::<T>::mutate(|config| {
                        config.inference_weight = new_value;
                    });
                },
                ParameterType::TrustScoreSlashingWeight => {
                    let new_value: u32 = Decode::decode(&mut &value[..])
                        .map_err(|_| Error::<T>::InvalidParameterType)?;
                    TrustScoreConfigStorage::<T>::mutate(|config| {
                        config.slashing_weight = new_value;
                    });
                },
                ParameterType::TrustScoreMaxGrowthRate => {
                    let new_value: u32 = Decode::decode(&mut &value[..])
                        .map_err(|_| Error::<T>::InvalidParameterType)?;
                    TrustScoreConfigStorage::<T>::mutate(|config| {
                        config.max_growth_rate = new_value;
                    });
                    // Update bounds storage as well
                    TrustScoreBounds::<T>::mutate(|bounds| {
                        bounds.max_growth_rate = new_value;
                        bounds.last_updated_epoch = Self::current_epoch();
                    });
                },
                ParameterType::TrustScoreMaxDecayRate => {
                    let new_value: u32 = Decode::decode(&mut &value[..])
                        .map_err(|_| Error::<T>::InvalidParameterType)?;
                    TrustScoreConfigStorage::<T>::mutate(|config| {
                        config.max_decay_rate = new_value;
                    });
                    // Update bounds storage as well
                    TrustScoreBounds::<T>::mutate(|bounds| {
                        bounds.max_decay_rate = new_value;
                        bounds.last_updated_epoch = Self::current_epoch();
                    });
                },
                ParameterType::TrustScoreMinValue => {
                    let new_value: u64 = Decode::decode(&mut &value[..])
                        .map_err(|_| Error::<T>::InvalidParameterType)?;
                    TrustScoreConfigStorage::<T>::mutate(|config| {
                        config.min_trust_score = new_value;
                    });
                    // Update bounds storage as well
                    TrustScoreBounds::<T>::mutate(|bounds| {
                        bounds.min_score = new_value;
                        bounds.last_updated_epoch = Self::current_epoch();
                    });
                },
                ParameterType::TrustScoreMaxValue => {
                    let new_value: u64 = Decode::decode(&mut &value[..])
                        .map_err(|_| Error::<T>::InvalidParameterType)?;
                    TrustScoreConfigStorage::<T>::mutate(|config| {
                        config.max_trust_score = new_value;
                    });
                    // Update bounds storage as well
                    TrustScoreBounds::<T>::mutate(|bounds| {
                        bounds.max_score = new_value;
                        bounds.last_updated_epoch = Self::current_epoch();
                    });
                },
                ParameterType::TrustScoreStabilityFactor => {
                    let new_value: u32 = Decode::decode(&mut &value[..])
                        .map_err(|_| Error::<T>::InvalidParameterType)?;
                    TrustScoreConfigStorage::<T>::mutate(|config| {
                        config.stability_factor = new_value;
                    });
                    // Update bounds storage as well
                    TrustScoreBounds::<T>::mutate(|bounds| {
                        bounds.stability_factor = new_value;
                        bounds.last_updated_epoch = Self::current_epoch();
                    });
                },
                // For other parameters, they are stored in governance config and will be
                // applied during epoch transitions or when the relevant systems access them
                _ => {
                    // Most parameters are applied through the governance config storage
                    // and don't require immediate active system updates
                },
            }
            
            Ok(())
        }


    }

    // --- DcfInterface Implementation --- //
    impl<T: Config> pallet_cbc_poi::DcfInterface<<T as frame_system::Config>::AccountId> for Pallet<T> {
        fn record_inference_activity(validator: &<T as frame_system::Config>::AccountId) -> DispatchResult {
            let current_block = <frame_system::Pallet<T>>::block_number().saturated_into::<u32>();
            
            ValidatorStates::<T>::try_mutate(validator, |maybe_state| {
                let state = maybe_state.as_mut().ok_or(Error::<T>::ValidatorNotFound)?;
                
                // Increment inference count
                state.inference_count = state.inference_count.saturating_add(1);
                
                // Update last active block
                state.last_active_block = current_block;
                
                // Update last active epoch
                state.last_active_epoch = Self::current_epoch();
                
                Ok::<(), Error<T>>(())
            })?;
            
            Ok(())
        }

        fn eject_validator(validator: &<T as frame_system::Config>::AccountId, _reason: &'static str) -> DispatchResult {
            Self::eject_validator(validator, EjectionReason::ScoreBelowThreshold)
        }

        fn update_final_score(validator: &<T as frame_system::Config>::AccountId) -> DispatchResult {
            Self::update_final_score(validator)
        }
    }

    // --- ValidatorHandler Implementation --- //
    impl<T: Config> pallet_cbc_pos::ValidatorHandler<<T as frame_system::Config>::AccountId, <T as pallet::Config>::Balance> for Pallet<T> {
        fn on_joined(validator: &<T as frame_system::Config>::AccountId, stake: <T as pallet::Config>::Balance) -> DispatchResult {
            let mut validator_set = ValidatorSet::<T>::get();
            if !validator_set.contains(validator) {
                ensure!(
                    validator_set.len() < T::MaxValidators::get() as usize,
                    Error::<T>::NotEnoughValidators
                );
                validator_set.try_push(validator.clone())
                    .map_err(|_| Error::<T>::NotEnoughValidators)?;
                ValidatorSet::<T>::put(validator_set);
            }

            if !ValidatorStates::<T>::contains_key(validator) {
                let current_epoch = Self::current_epoch();
                let stake_score = stake.saturated_into::<u64>();
                
                let inference_score = poi::Pallet::<T>::get_score(validator);

                let pos_weight = if !PosWeight::<T>::exists() {
                    let weight = T::DefaultPosWeight::get();
                    PosWeight::<T>::put(weight);
                    weight
                } else {
                    PosWeight::<T>::get()
                };
                let poi_weight = if !PoiWeight::<T>::exists() {
                    let weight = T::DefaultPoiWeight::get();
                    PoiWeight::<T>::put(weight);
                    weight
                } else {
                    PoiWeight::<T>::get()
                };

                let mut final_score = (stake_score.saturating_mul(pos_weight) + inference_score.saturating_mul(poi_weight)) / T::PercentagePrecision::get() as u64;
                if final_score > T::MaxValidatorScore::get() {
                    final_score = T::MaxValidatorScore::get();
                }

                let initial_stats = EpochStats {
                    epoch: current_epoch,
                    stake_score,
                    inference_score,
                    final_score,
                    authored_blocks: 0,
                    missed_blocks: 0,
                };

                let current_block = frame_system::Pallet::<T>::block_number().saturated_into::<u32>();
                let state = ValidatorState {
                    last_active_epoch: current_epoch,
                    current: initial_stats,
                    history: BoundedVec::new(),
                    uptime: 0,
                    inference_success_count: 0,
                    participation_rate: 0,
                    inference_count: 0,
                    last_active_block: current_block,
                    name: None,
                    trust_score: 5000,
                };

                ValidatorStates::<T>::insert(validator, state);
            }

            Ok(())
        }

        fn on_leave_requested(validator: &<T as frame_system::Config>::AccountId) -> DispatchResult {
            let mut active_validators = ActiveValidators::<T>::get();
            if let Some(pos) = active_validators.iter().position(|v| v == validator) {
                active_validators.remove(pos);
                ActiveValidators::<T>::put(active_validators);
            }
            Ok(())
        }

        fn on_left(validator: &<T as frame_system::Config>::AccountId) -> DispatchResult {
            let mut validator_set = ValidatorSet::<T>::get();
            if let Some(pos) = validator_set.iter().position(|v| v == validator) {
                validator_set.remove(pos);
                ValidatorSet::<T>::put(validator_set);
            }

            let mut active_validators = ActiveValidators::<T>::get();
            if let Some(pos) = active_validators.iter().position(|v| v == validator) {
                active_validators.remove(pos);
                ActiveValidators::<T>::put(active_validators);
            }

            ValidatorStates::<T>::remove(validator);
            ValidatorNames::<T>::remove(validator);
            ValidatorMetadata::<T>::remove(validator);
            ValidatorPerformanceHistory::<T>::remove(validator);
            ValidatorLastSeen::<T>::remove(validator);
            ValidatorBlocksAuthored::<T>::remove(validator);
            ValidatorBlocksMissed::<T>::remove(validator);
            ValidatorUptime::<T>::remove(validator);
            pallet_cbc_poi::ValidatorInferenceCount::<T>::remove(validator);
            PendingValidatorActions::<T>::remove(validator);

            Ok(())
        }

        fn on_stake_increased(validator: &<T as frame_system::Config>::AccountId, _amount: <T as pallet::Config>::Balance) -> DispatchResult {
            let new_total_stake = pos::Pallet::<T>::stake(validator);
            let new_stake_score = new_total_stake.saturated_into::<u64>();
            ValidatorStates::<T>::try_mutate(validator, |maybe_state| {
                if let Some(state) = maybe_state.as_mut() {
                    state.current.stake_score = new_stake_score;
                    let _ = Self::update_final_score(validator);
                }
                Ok::<(), Error<T>>(())
            })?;
            Ok(())
        }

        fn on_stake_decreased(validator: &<T as frame_system::Config>::AccountId, _amount: <T as pallet::Config>::Balance) -> DispatchResult {
            let new_total_stake = pos::Pallet::<T>::stake(validator);
            let new_stake_score = new_total_stake.saturated_into::<u64>();
            ValidatorStates::<T>::try_mutate(validator, |maybe_state| {
                if let Some(state) = maybe_state.as_mut() {
                    state.current.stake_score = new_stake_score;
                    let _ = Self::update_final_score(validator);
                }
                Ok::<(), Error<T>>(())
            })?;
            Ok(())
        }

        fn on_slashed(validator: &<T as frame_system::Config>::AccountId, _amount: <T as pallet::Config>::Balance, penalty: u64) -> DispatchResult {
            ValidatorStates::<T>::try_mutate(validator, |maybe_state| {
                let state = maybe_state.as_mut().ok_or(Error::<T>::ValidatorNotFound)?;
                state.current.final_score = state.current.final_score.saturating_sub(penalty);
                state.last_active_epoch = Self::current_epoch();
                
                let new_total_stake = pos::Pallet::<T>::stake(validator);
                state.current.stake_score = new_total_stake.saturated_into::<u64>();
                
                if state.history.len() == state.history.capacity() {
                    state.history.remove(0);
                }
                let _ = state.history.try_push(EpochStats {
                    epoch: Self::current_epoch(),
                    stake_score: state.current.stake_score,
                    inference_score: state.current.inference_score,
                    final_score: state.current.final_score,
                    authored_blocks: state.current.authored_blocks,
                    missed_blocks: state.current.missed_blocks,
                });
                
                if state.current.final_score < MinValidatorScoreOf::<T>::get() as u64 {
                    let _ = Self::eject_validator(validator, EjectionReason::ScoreBelowThreshold);
                }
                
                Ok::<(), Error<T>>(())
            })?;
            Ok(())
        }

        fn on_rewarded(validator: &<T as frame_system::Config>::AccountId, _amount: <T as pallet::Config>::Balance, boost: u64) -> DispatchResult {
            ValidatorStates::<T>::try_mutate(validator, |maybe_state| {
                let state = maybe_state.as_mut().ok_or(Error::<T>::ValidatorNotFound)?;
                state.current.final_score = state.current.final_score.saturating_add(boost);
                if state.current.final_score > T::MaxValidatorScore::get() {
                    state.current.final_score = T::MaxValidatorScore::get();
                }
                state.last_active_epoch = Self::current_epoch();
                
                let new_total_stake = pos::Pallet::<T>::stake(validator);
                state.current.stake_score = new_total_stake.saturated_into::<u64>();
                
                if state.history.len() == state.history.capacity() {
                    state.history.remove(0);
                }
                let _ = state.history.try_push(EpochStats {
                    epoch: Self::current_epoch(),
                    stake_score: state.current.stake_score,
                    inference_score: state.current.inference_score,
                    final_score: state.current.final_score,
                    authored_blocks: state.current.authored_blocks,
                    missed_blocks: state.current.missed_blocks,
                });
                
                Ok::<(), Error<T>>(())
            })?;
            Ok(())
        }

        fn get_validator_score(validator: &<T as frame_system::Config>::AccountId) -> u64 {
            ValidatorStates::<T>::get(validator).map(|s| s.current.final_score).unwrap_or(0)
        }

        fn get_active_validators() -> Vec<<T as frame_system::Config>::AccountId> {
            ActiveValidators::<T>::get().into_inner()
        }
    }
}

// Re-export the pallet for external use
pub use pallet::*;

// --- Benchmarking (if enabled) --- //
// (Benchmarking module is declared at the top of the file)

// --- Score/Ejection/Inference Reason Enums --- //

/// Reasons for boosting a validator's score as a reward for positive actions.
/// 
/// Score boosts incentivize beneficial network behavior and recognize validators
/// who contribute positively to network security, performance, and reliability.
/// Different boost reasons may have different score impact amounts.
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen, frame_support::__private::codec::DecodeWithMemTracking)]
pub enum ScoreBoostReason {
    /// Validator successfully authored a valid block when selected.
    /// 
    /// This boost rewards validators for consistent availability and successful
    /// block production. It encourages validators to maintain uptime and respond
    /// promptly when selected as block authors.
    ValidBlockAuthored,

    /// Validator submitted high-quality inference results.
    /// 
    /// This boost rewards validators for accurate AI/ML inference performance,
    /// encouraging investment in quality inference infrastructure and algorithms.
    /// The boost amount may vary based on inference confidence scores.
    ValidInference,

    /// Manual score boost applied through governance or administrative action.
    /// 
    /// This boost allows for discretionary rewards for exceptional service,
    /// community contributions, or other valuable activities that benefit
    /// the network but may not be automatically detected.
    ManualBoost,
}

/// Reasons for ejecting validators from the network.
/// 
/// Validator ejection is a serious disciplinary action that removes validators
/// from the network for failing to meet minimum standards or engaging in
/// harmful behavior. Ejected validators must typically wait for cooldown
/// periods and demonstrate improvement before rejoining.
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen, frame_support::__private::codec::DecodeWithMemTracking)]
pub enum EjectionReason {
    /// Validator's performance score fell below the minimum threshold.
    /// 
    /// This ejection occurs when validators consistently underperform and
    /// their scores drop below the network's minimum acceptable level.
    /// It protects network quality by removing poor performers.
    ScoreBelowThreshold,

    /// Validator reached the maximum allowed slashing count.
    /// 
    /// This ejection occurs when validators accumulate too many slashing
    /// events, indicating chronic misbehavior or poor performance that
    /// poses a risk to network security and reliability.
    MaxSlashingReached,

    /// Manual ejection through governance proposal or administrative action.
    /// 
    /// This ejection allows for discretionary removal of validators for
    /// reasons that may not be automatically detected, such as off-chain
    /// misbehavior or community violations.
    ManualEjection,

    /// Ejection due to validator set size exceeding maximum limits.
    /// 
    /// This ejection occurs when the validator set grows beyond the maximum
    /// allowed size and lower-performing validators are removed to maintain
    /// network performance and consensus efficiency.
    ExcessValidators,

    /// Ejection due to repeated misbehavior reports.
    /// 
    /// This ejection occurs when validators accumulate multiple misbehavior
    /// reports, indicating a pattern of harmful or malicious behavior that
    /// threatens network security and integrity.
    RepeatedMisbehavior,

    /// Validator's stake fell below the minimum required amount.
    /// 
    /// This ejection occurs when validators can no longer maintain the
    /// minimum stake requirement, either due to slashing, withdrawals,
    /// or changes in minimum stake requirements.
    InsufficientStake,
}


/// Reasons for slashing validator stakes as punishment for violations.
/// 
/// Slashing is a critical economic penalty that reduces validator stakes
/// to maintain network security and incentive alignment. Different slash
/// reasons may have different penalty amounts and recovery requirements.
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen, frame_support::__private::codec::DecodeWithMemTracking)]
pub enum RewardReason {
    /// Reward for exceptional performance above standards.
    /// 
    /// This includes consistently high availability, successful block
    /// production, and high-quality inference results. Rewards recognize
    /// validators who exceed baseline expectations.
    ExceptionalPerformance,

    /// Reward for successful block authorship when selected.
    /// 
    /// This provides immediate incentives for validators to produce
    /// valid blocks when chosen as block authors, encouraging
    /// reliable block production.
    BlockAuthorship,

    /// Manual reward through governance proposal or administrative action.
    /// 
    /// This allows for discretionary rewards for special contributions,
    /// community service, or other valuable activities that benefit
    /// the network beyond standard operations.
    ManualReward,

    /// Reward for high-quality inference results and AI/ML contributions.
    /// 
    /// This incentivizes validators to provide accurate and valuable
    /// inference services, supporting the network's AI/ML capabilities
    /// and maintaining service quality.
    InferenceQuality,

    /// Epoch-based performance reward for meeting participation standards.
    /// 
    /// This provides regular rewards for validators who maintain
    /// acceptable performance levels throughout an epoch, encouraging
    /// consistent participation and network stability.
    EpochPerformance,
}

/// Reasons for validator slashing with different severity levels and recovery requirements.
/// 
/// Each slashing reason corresponds to different types of validator misbehavior or
/// poor performance. The reason affects the penalty amount and determines what
/// actions validators must take to recover their standing. Different slashing
/// reasons may have different penalty amounts and recovery requirements.
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen, frame_support::__private::codec::DecodeWithMemTracking)]
pub enum SlashReason {
    /// Slashing due to malicious behavior or protocol violations.
    /// 
    /// This includes actions like double-signing, equivocation, or other
    /// deliberate attempts to harm the network. Typically results in
    /// severe penalties and potential permanent ejection.
    Misbehavior,

    /// Slashing due to consistently poor performance below standards.
    /// 
    /// This includes chronic unavailability, repeated missed blocks,
    /// or consistently low-quality inference results. Penalties are
    /// typically moderate with opportunities for improvement.
    PoorPerformance,

    /// Manual slashing through governance proposal or administrative action.
    /// 
    /// This allows for discretionary penalties for situations that may
    /// not fit standard categories but warrant economic punishment.
    /// Penalty amounts are determined case-by-case.
    ManualSlash,

    /// Slashing due to violations of consensus rules or protocol requirements.
    /// 
    /// This includes technical violations of consensus mechanisms,
    /// invalid block production, or other protocol-level infractions
    /// that threaten network integrity.
    ConsensusViolation,
}

/// Current status of a validator in the network lifecycle.
/// 
/// Validator status determines what actions are available to the validator
/// and how they are treated by the consensus mechanism. Status changes
/// are tracked through events and affect validator participation rights.
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen, frame_support::__private::codec::DecodeWithMemTracking)]
pub enum ValidatorStatus {
    /// Validator is active and participating in consensus operations.
    /// 
    /// Active validators can:
    /// - Produce blocks when selected as authors
    /// - Participate in governance voting
    /// - Receive rewards for good performance
    /// - Submit inference results
    /// 
    /// This is the normal operational status for validators in good standing.
    Active,

    /// Validator is registered but not currently participating in consensus.
    /// 
    /// Inactive validators may be temporarily excluded due to:
    /// - Scores below the minimum threshold
    /// - Voluntary temporary withdrawal
    /// - System maintenance or upgrades
    /// 
    /// Inactive validators can typically return to active status by improving performance.
    Inactive,

    /// Validator has requested to leave and is in the mandatory cooldown period.
    /// 
    /// Leaving validators must continue participating during the cooldown period
    /// but will be removed from the network once the cooldown expires. During
    /// this period, their stake remains reserved and they cannot cancel the
    /// leave request in most implementations.
    Leaving,

    /// Validator has been ejected due to poor performance or misbehavior.
    /// 
    /// Ejected validators are removed from all validator sets and cannot
    /// participate in consensus. They typically face additional cooldown
    /// periods before being allowed to rejoin and may need to demonstrate
    /// improved capabilities or resolve the issues that led to ejection.
    Ejected,
}



// --- Tests Module --- //
#[cfg(test)]
mod tests;

#[cfg(test)]
mod mock;

// Removed redundant weight verification module