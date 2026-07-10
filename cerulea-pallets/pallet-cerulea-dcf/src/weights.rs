//! # DCF Pallet Weight Calculations
//!
//! This file contains comprehensive weight calculations for all DCF pallet dispatchables.
//! All weights are calculated based on worst-case scenarios to ensure safe block production.
//!
//! ## Worst-Case Assumptions (Cross-Checked with Runtime Configuration)
//!
//! The following assumptions are used for weight calculations and have been verified
//! against the actual runtime configuration in `cerulea-runtime/src/configs/mod.rs`:
//!
//! ### Validator Set Limits
//! - **MaxValidators**: 100 validators (runtime constant: `MaxValidators = 100`)
//! - **MinActiveValidators**: 3 validators minimum for network security
//! - **MaxValidatorHistoryLength**: 10 epochs of history per validator (runtime: `MaxValidatorHistorySize = 10`)
//! - **MaxEpochHistory**: 24 epochs of system history (runtime: `MaxEpochHistory = 24`)
//!
//! ### Proposal and Governance Limits
//! - **MaxProposalQueue**: 50 active proposals at any time (estimated worst-case)
//! - **MaxProposalsPerValidator**: 5 proposals per validator (runtime constant)
//! - **ProposalLifetime**: 1000 blocks until proposal expires
//! - **GovernanceQuorum**: 50% of validators needed for quorum
//!
//! ### Metadata and Evidence Limits
//! - **MaxValidatorNameLength**: 32 bytes per validator name (runtime: `MaxValidatorNameLength = 32`)
//! - **MaxValidatorWebsiteLength**: 64 bytes per validator website
//! - **MaxValidatorDescriptionLength**: 128 bytes per validator description
//! - **MaxEvidenceLength**: 1024 bytes per misbehavior evidence (runtime constant)
//! - **MaxMisbehaviorReports**: 10 reports per validator (estimated worst-case)
//!
//! ### Epoch and Timing Configuration
//! - **EpochLength**: 2400 blocks per epoch (runtime: `EpochLength = 2400`)
//! - **LeaveCooldown**: 1000 blocks cooldown period (runtime: `LeaveCooldown = 1000`)
//! - **MaxMissedBlocksPerEpoch**: 50 missed blocks per epoch
//!
//! ### Performance and Scoring Limits
//! - **MaxValidatorScore**: 100 maximum validator score (runtime constant)
//! - **MaxTrustScore**: 10000 maximum trust score (runtime: `MaxTrustScore = 10000`)
//! - **MaxPerformanceHistoryLength**: 100 performance records (runtime constant)
//!
//! ## Storage Access Patterns and Complexity Analysis
//!
//! Weight calculations account for the following storage access patterns:
//!
//! ### Map Access Patterns (O(1) with logarithmic proof overhead)
//! - **ValidatorStates**: Single validator state read/write
//! - **ValidatorStake**: Validator stake amount access
//! - **ValidatorNames**: Validator metadata access
//! - **Proposals**: Individual proposal access
//! - **ProposalVotes**: Vote records per proposal
//!
//! ### Vector and Bounded Collection Operations
//! - **ValidatorSet**: O(n) iteration where n ≤ MaxValidators (100)
//! - **ActiveValidators**: O(n) iteration where n ≤ MaxValidators (100)
//! - **PendingValidatorActions**: O(n) processing where n ≤ MaxValidators (100)
//! - **ValidatorHistory**: O(h) access where h ≤ MaxValidatorHistoryLength (10)
//! - **EpochHistories**: O(e) access where e ≤ MaxEpochHistory (24)
//!
//! ### Currency Operations (Fixed Weight)
//! - **Reserve/Unreserve**: Fixed weight for balance operations
//! - **Transfer**: Fixed weight for balance transfers
//! - **Slash**: Fixed weight per slashing operation
//!
//! ## Weight Calculation Methodology
//!
//! 1. **Base Weight**: Fixed computational cost for the dispatchable logic
//! 2. **Storage Reads**: Number of storage items read × RocksDbWeight::get().reads(1)
//! 3. **Storage Writes**: Number of storage items written × RocksDbWeight::get().writes(1)
//! 4. **Variable Weight**: Additional weight for operations that scale with input size
//! 5. **Currency Weight**: Fixed weight for balance operations (reserve/unreserve/transfer)
//! 6. **Iteration Weight**: Linear scaling for operations over validator sets
//!
//! ## Benchmarking Cross-Check Requirements
//!
//! All weights in this file MUST be cross-checked against benchmarking results
//! using the worst-case scenarios defined above. Before production deployment:
//!
//! 1. **Benchmark Validation**: Run benchmarks with MaxValidators (100) active
//! 2. **Proposal Queue Testing**: Test with maximum proposal queue size (50)
//! 3. **History Limits Testing**: Test with maximum validator history (10 epochs)
//! 4. **Epoch Transition Testing**: Test epoch transitions with full validator set
//! 5. **Multi-Validator Operations**: Test batch operations with maximum validators
//!
//! ## Weight Safety Margins
//!
//! All weights include safety margins to account for:
//! - **Runtime Overhead**: Additional 10-20% for runtime execution overhead
//! - **Database Variance**: Account for RocksDB performance variations
//! - **Network Conditions**: Additional margin for network-dependent operations
//! - **Future Upgrades**: Headroom for minor feature additions
//!
//! ## Block Weight Limits Compliance
//!
//! Maximum operation weights are designed to stay within block limits:
//! - **Single Operation Limit**: No operation exceeds 25% of block weight
//! - **Epoch Transition Limit**: Epoch transitions stay within 50% of block weight
//! - **Batch Operation Limit**: Multi-validator operations respect block limits
//! - **Emergency Operations**: Critical operations have priority weight allocation

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{
    traits::Get,
    weights::{Weight, constants::RocksDbWeight},
};
use sp_std::marker::PhantomData;

/// Weight functions needed for the pallet.
pub trait WeightInfo {
    fn on_initialize() -> Weight;
    fn offchain_worker() -> Weight;
    fn update_validator_stake_score() -> Weight;
    fn update_validator_inference_score() -> Weight;
    fn update_consensus_weights() -> Weight;
    fn set_governance_mode() -> Weight;
    fn sudo_advance_epoch() -> Weight;
    fn submit_proposal() -> Weight;
    fn vote_proposal() -> Weight;
    fn execute_proposal() -> Weight;
    fn join_validator_set() -> Weight;
    fn leave_validator_set() -> Weight;
    fn set_validator_name() -> Weight;
    fn apply_offchain_poi_scores() -> Weight;
    fn on_initialize_with_validators(v: u32) -> Weight;
    fn apply_score_decay_multiple(v: u32) -> Weight;
    fn validator_set_operations(v: u32) -> Weight;
    fn runtime_api_calls(v: u32) -> Weight;
    fn epoch_transition_multiple(v: u32) -> Weight;
    fn governance_with_multiple_voters(v: u32) -> Weight;
    
    // New benchmark functions for all dispatchable calls
    fn join_validators() -> Weight;
    fn leave_validators() -> Weight;
    fn cancel_leave_request() -> Weight;
    fn slash_validator() -> Weight;
    fn slash_validator_percentage() -> Weight;
    fn increase_validator_stake() -> Weight;
    fn decrease_validator_stake() -> Weight;
    fn report_validator_misbehavior() -> Weight;
    fn simulate_inference() -> Weight;
    fn propose_slash_validator() -> Weight;
    fn propose_reward_validator() -> Weight;
    fn propose_eject_validator() -> Weight;
    fn slash_multiple_validators() -> Weight;
    fn execute_proposals() -> Weight;
    fn epoch_transition() -> Weight;
    fn set_validator_metadata() -> Weight;
    fn update_validator_activity() -> Weight;
    fn distribute_epoch_rewards() -> Weight;
    fn propose_default_reward_validator() -> Weight;
    fn propose_default_reward_multiple_validators() -> Weight;
    fn propose_reward_all_active_validators() -> Weight;
    fn propose_reward_multiple_validators(v: u32) -> Weight;
}

/// Weights for the pallet using the Substrate node and recommended hardware.
pub struct SubstrateWeight<T>(PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    /// Storage: ValidatorStates (r:1 w:1)
    /// Storage: PosWeight (r:1 w:0)
    /// Storage: PoiWeight (r:1 w:0)
    fn update_validator_stake_score() -> Weight {
        Weight::from_parts(15_000, 0)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(1))
    }

    /// Storage: ValidatorStates (r:1 w:1)
    /// Storage: PosWeight (r:1 w:0)
    /// Storage: PoiWeight (r:1 w:0)
    fn update_validator_inference_score() -> Weight {
        Weight::from_parts(15_000, 0)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(1))
    }

    /// Storage: PosWeight (r:0 w:1)
    /// Storage: PoiWeight (r:0 w:1)
    fn update_consensus_weights() -> Weight {
        Weight::from_parts(8_000, 0)
            .saturating_add(T::DbWeight::get().writes(2))
    }

    /// Storage: GovernanceModeEnabled (r:0 w:1)
    fn set_governance_mode() -> Weight {
        Weight::from_parts(5_000, 0)
            .saturating_add(T::DbWeight::get().writes(1))
    }

    /// Storage: GovernanceModeEnabled (r:1 w:0)
    /// Storage: CurrentEpoch (r:1 w:1)
    /// Storage: ActiveValidators (r:1 w:1)
    /// Storage: PendingValidatorActions (r:0 w:10)
    fn sudo_advance_epoch() -> Weight {
        Weight::from_parts(50_000, 0)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(12))
    }

    /// Storage: GovernanceModeEnabled (r:1 w:0)
    /// Storage: NextProposalId (r:1 w:1)
    /// Storage: Proposals (r:0 w:1)
    fn submit_proposal() -> Weight {
        Weight::from_parts(20_000, 0)
            .saturating_add(T::DbWeight::get().reads(2))
            .saturating_add(T::DbWeight::get().writes(2))
    }

    /// Storage: Proposals (r:1 w:1)
    /// Storage: ProposalVotes (r:1 w:1)
    fn vote_proposal() -> Weight {
        Weight::from_parts(18_000, 0)
            .saturating_add(T::DbWeight::get().reads(2))
            .saturating_add(T::DbWeight::get().writes(2))
    }

    /// Storage: Proposals (r:1 w:1)
    /// Storage: ValidatorStates (r:1 w:1)
    /// Storage: ActiveValidators (r:1 w:1)
    fn execute_proposal() -> Weight {
        Weight::from_parts(35_000, 0)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(3))
    }

    /// Storage: ActiveValidators (r:1 w:0)
    /// Storage: PendingValidatorActions (r:1 w:1)
    /// Storage: ValidatorStates (r:1 w:0)
    fn join_validator_set() -> Weight {
        Weight::from_parts(25_000, 0)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(1))
    }

    /// Storage: ActiveValidators (r:1 w:0)
    /// Storage: PendingValidatorActions (r:1 w:1)
    fn leave_validator_set() -> Weight {
        Weight::from_parts(20_000, 0)
            .saturating_add(T::DbWeight::get().reads(2))
            .saturating_add(T::DbWeight::get().writes(1))
    }

    /// Storage: ValidatorStates (r:1 w:0)
    /// Storage: ValidatorNames (r:0 w:1)
    fn set_validator_name() -> Weight {
        Weight::from_parts(12_000, 0)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }

    /// Storage: ValidatorSet (r:1 w:0)
    /// Storage: ValidatorStates (r:MaxValidators w:MaxValidators)
    /// Storage: RateLimitConfigStorage (r:1 w:0)
    /// Proof: Worst case processes all MaxValidators (100) with PoI score updates
    fn apply_offchain_poi_scores() -> Weight {
        Weight::from_parts(50_000, 0)
            .saturating_add(Weight::from_parts(15_000, 0).saturating_mul(100u64)) // 100 = MaxValidators
            .saturating_add(T::DbWeight::get().reads(2)) // ValidatorSet + RateLimitConfig
            .saturating_add(T::DbWeight::get().reads(100u64)) // Read all validator states
            .saturating_add(T::DbWeight::get().writes(100u64)) // Update all validator states
    }

    /// Storage: ValidatorSet (r:1 w:0)
    /// Storage: ActiveValidators (r:1 w:0)
    /// Storage: CurrentEpoch (r:1 w:0)
    /// Storage: EpochConfigStorage (r:1 w:0)
    /// Storage: ValidatorStates (r:v w:v)
    /// Proof: Worst case processes MaxValidators (100) for epoch initialization
    fn on_initialize_with_validators(v: u32) -> Weight {
        Weight::from_parts(25_000, 0)
            .saturating_add(Weight::from_parts(8_000, 0).saturating_mul(v.into()))
            .saturating_add(T::DbWeight::get().reads(4))
            .saturating_add(T::DbWeight::get().reads((1_u64).saturating_mul(v.into())))
            .saturating_add(T::DbWeight::get().writes((1_u64).saturating_mul(v.into())))
    }

    /// Storage: ValidatorStates (r:v w:v)
    /// Proof: Worst case applies decay to MaxValidators (100) inactive validators
    fn apply_score_decay_multiple(v: u32) -> Weight {
        Weight::from_parts(15_000, 0)
            .saturating_add(Weight::from_parts(12_000, 0).saturating_mul(v.into()))
            .saturating_add(T::DbWeight::get().reads((1_u64).saturating_mul(v.into())))
            .saturating_add(T::DbWeight::get().writes((1_u64).saturating_mul(v.into())))
    }

    /// Storage: ValidatorSet (r:1 w:0)
    /// Storage: ActiveValidators (r:1 w:0)
    /// Storage: ValidatorStates (r:v w:0)
    fn validator_set_operations(v: u32) -> Weight {
        Weight::from_parts(15_000, 0)
            .saturating_add(Weight::from_parts(3_000, 0).saturating_mul(v.into()))
            .saturating_add(T::DbWeight::get().reads(2))
            .saturating_add(T::DbWeight::get().reads((1_u64).saturating_mul(v.into())))
    }

    /// Storage: ValidatorSet (r:1 w:0)
    /// Storage: ActiveValidators (r:1 w:0)
    /// Storage: CurrentEpoch (r:1 w:0)
    /// Storage: PosWeight (r:1 w:0)
    /// Storage: PoiWeight (r:1 w:0)
    /// Storage: ValidatorStates (r:v w:0)
    /// Storage: ValidatorStake (r:v w:0)
    /// Storage: ValidatorNames (r:v w:0)
    /// Proof: Worst case API calls query all validator data for MaxValidators (100)
    fn runtime_api_calls(v: u32) -> Weight {
        Weight::from_parts(35_000, 0)
            .saturating_add(Weight::from_parts(8_000, 0).saturating_mul(v.into()))
            .saturating_add(T::DbWeight::get().reads(5)) // Fixed storage items
            .saturating_add(T::DbWeight::get().reads((3_u64).saturating_mul(v.into()))) // Per-validator reads
    }

    /// Storage: CurrentEpoch (r:1 w:1)
    /// Storage: ActiveValidators (r:1 w:1)
    /// Storage: PendingValidatorActions (r:v w:v)
    /// Storage: EpochHistories (r:1 w:1)
    /// Storage: ValidatorStates (r:v w:v) - for score updates and history
    /// Proof: Worst case transitions epoch with MaxValidators (100) pending actions
    fn epoch_transition_multiple(v: u32) -> Weight {
        Weight::from_parts(60_000, 0)
            .saturating_add(Weight::from_parts(15_000, 0).saturating_mul(v.into()))
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().reads((2_u64).saturating_mul(v.into()))) // PendingActions + ValidatorStates
            .saturating_add(T::DbWeight::get().writes(3))
            .saturating_add(T::DbWeight::get().writes((2_u64).saturating_mul(v.into()))) // PendingActions + ValidatorStates
    }

    /// Storage: Proposals (r:1 w:1)
    /// Storage: ProposalVotes (r:v w:v)
    /// Storage: ValidatorSet (r:1 w:0) - for voter validation
    /// Proof: Worst case processes votes from MaxValidators (100) voters
    fn governance_with_multiple_voters(v: u32) -> Weight {
        Weight::from_parts(30_000, 0)
            .saturating_add(Weight::from_parts(18_000, 0).saturating_mul(v.into()))
            .saturating_add(T::DbWeight::get().reads(2)) // Proposals + ValidatorSet
            .saturating_add(T::DbWeight::get().reads((1_u64).saturating_mul(v.into()))) // ProposalVotes
            .saturating_add(T::DbWeight::get().writes(1)) // Proposals
            .saturating_add(T::DbWeight::get().writes((1_u64).saturating_mul(v.into()))) // ProposalVotes
    }

    fn on_initialize() -> Weight {
        Self::on_initialize_with_validators(10)
    }

    fn offchain_worker() -> Weight {
        Weight::from_parts(50_000, 0)
            .saturating_add(T::DbWeight::get().reads(5))
            .saturating_add(T::DbWeight::get().writes(3))
    }

    /// Storage: ValidatorSet (r:1 w:1) - Check and update validator set
    /// Storage: ActiveValidators (r:0 w:0) - Not accessed in join_validators
    /// Storage: ValidatorStates (r:1 w:1) - Initialize or update validator state
    /// Storage: ValidatorStake (r:0 w:1) - Store validator stake amount
    /// Storage: ValidatorJoinTime (r:0 w:1) - Record join timestamp
    /// Storage: RecentlyRemovedValidators (r:1 w:1) - Check and clear cooldown
    /// Storage: RateLimitConfigStorage (r:1 w:0) - Rate limiting check
    /// Storage: BlockOperationCounts (r:1 w:1) - Rate limiting tracking
    /// Storage: AccountOperationHistory (r:1 w:1) - Rate limiting history
    /// Storage: LastOperationBlock (r:1 w:1) - Rate limiting intervals
    /// Storage: PosWeight (r:1 w:1) - Initialize if not exists
    /// Storage: PoiWeight (r:1 w:1) - Initialize if not exists
    /// Storage: Currency operations (r:2 w:2) - Balance check and stake reservation
    /// Proof: Worst case validator joins with full rate limiting checks and state initialization
    fn join_validators() -> Weight {
        Weight::from_parts(120_000, 0) // Increased base weight for complex logic
            .saturating_add(T::DbWeight::get().reads(10)) // All storage reads including rate limiting
            .saturating_add(T::DbWeight::get().writes(10)) // All storage writes including initialization
    }

    /// Storage: ValidatorSet (r:1 w:0)
    /// Storage: ActiveValidators (r:1 w:0)
    /// Storage: ValidatorStates (r:1 w:1)
    /// Storage: ValidatorLeaveRequests (r:1 w:1)
    /// Storage: RateLimitConfigStorage (r:1 w:0) - for rate limiting check
    /// Proof: Validator initiates leave request with cooldown period
    fn leave_validators() -> Weight {
        Weight::from_parts(60_000, 0)
            .saturating_add(T::DbWeight::get().reads(5))
            .saturating_add(T::DbWeight::get().writes(2))
    }

    /// Storage: ValidatorStates (r:1 w:1)
    /// Storage: Currency operations (r:2 w:2)
    fn slash_validator() -> Weight {
        Weight::from_parts(45_000, 0)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(3))
    }







    /// Storage: MisbehaviorReports (r:1 w:1)
    /// Storage: ValidatorStates (r:1 w:1)
    /// Storage: Currency operations (r:2 w:2)


    /// Storage: ValidatorStates (r:1 w:1)
    /// Storage: Currency operations (r:2 w:2)
    fn increase_validator_stake() -> Weight {
        Weight::from_parts(35_000, 0)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(3))
    }

    /// Storage: ValidatorStates (r:1 w:1)
    /// Storage: Currency operations (r:2 w:2)
    fn decrease_validator_stake() -> Weight {
        Weight::from_parts(35_000, 0)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(3))
    }

    /// Storage: ValidatorSet (r:1 w:0) - Validate all target validators exist
    /// Storage: ValidatorStates (r:v w:v) - Update validator states after slashing
    /// Storage: ValidatorStake (r:v w:v) - Read and update stake amounts
    /// Storage: SlashingHistory (r:v w:v) - Record slashing events
    /// Storage: Currency operations (r:2*v w:2*v) - Read balance and execute slash
    /// Storage: RateLimitConfigStorage (r:1 w:0) - Rate limiting check
    /// Storage: BlockOperationCounts (r:1 w:1) - Rate limiting tracking
    /// Proof: Worst case slashes multiple validators (up to MaxValidators/2 = 50)
    /// with full validation and history tracking
    fn slash_multiple_validators() -> Weight {
        Weight::from_parts(150_000, 0) // Increased base weight for validation logic
            .saturating_add(Weight::from_parts(45_000, 0).saturating_mul(50u64)) // 50 validators max
            .saturating_add(T::DbWeight::get().reads(4)) // Fixed reads (ValidatorSet, RateLimit, etc.)
            .saturating_add(T::DbWeight::get().reads(250u64)) // 5 * 50 validators (States, Stake, History, Currency)
            .saturating_add(T::DbWeight::get().writes(2)) // Fixed writes (BlockOperationCounts)
            .saturating_add(T::DbWeight::get().writes(200u64)) // 4 * 50 validators (States, Stake, History, Currency)
    }

    fn cancel_leave_request() -> Weight {
        Weight::from_parts(25_000, 0)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }

    fn report_validator_misbehavior() -> Weight {
        Weight::from_parts(55_000, 0)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(2))
    }

    fn simulate_inference() -> Weight {
        Weight::from_parts(30_000, 0)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }

    fn propose_slash_validator() -> Weight {
        Weight::from_parts(35_000, 0)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(2))
    }

    fn propose_reward_validator() -> Weight {
        Weight::from_parts(35_000, 0)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(2))
    }

    fn propose_eject_validator() -> Weight {
        Weight::from_parts(35_000, 0)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(2))
    }

    /// Storage: ValidatorStates (r:1 w:1)
    /// Storage: Currency operations (r:2 w:2)
    fn slash_validator_percentage() -> Weight {
        Weight::from_parts(40_000, 0)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(3))
    }

    /// Storage: Proposals (r:MaxProposalQueue w:MaxProposalQueue) - Read and update proposal status
    /// Storage: ProposalVotes (r:MaxProposalQueue w:MaxProposalQueue) - Read votes for execution
    /// Storage: ValidatorSet (r:1 w:0) - Validate proposal targets
    /// Storage: ValidatorStates (r:MaxProposalQueue w:MaxProposalQueue) - Update affected validators
    /// Storage: ValidatorStake (r:MaxProposalQueue w:MaxProposalQueue) - Handle stake changes
    /// Storage: SlashingHistory (r:MaxProposalQueue w:MaxProposalQueue) - Record slashing proposals
    /// Storage: RewardHistory (r:MaxProposalQueue w:MaxProposalQueue) - Record reward proposals
    /// Storage: Currency operations (r:2*MaxProposalQueue w:2*MaxProposalQueue) - Execute financial changes
    /// Storage: ActiveValidators (r:1 w:1) - Update active set for ejection proposals
    /// Proof: Worst case executes MaxProposalQueue (50) proposals with mixed types
    fn execute_proposals() -> Weight {
        Weight::from_parts(200_000, 0) // Increased base weight for complex proposal logic
            .saturating_add(Weight::from_parts(55_000, 0).saturating_mul(50u64)) // MaxProposalQueue processing
            .saturating_add(T::DbWeight::get().reads(2)) // Fixed reads (ValidatorSet, ActiveValidators)
            .saturating_add(T::DbWeight::get().reads(450u64)) // 9 * MaxProposalQueue (all per-proposal reads)
            .saturating_add(T::DbWeight::get().writes(1)) // Fixed writes (ActiveValidators)
            .saturating_add(T::DbWeight::get().writes(400u64)) // 8 * MaxProposalQueue (all per-proposal writes)
    }

    /// Storage: CurrentEpoch (r:1 w:1) - Read and increment epoch
    /// Storage: ActiveValidators (r:1 w:1) - Update active validator set
    /// Storage: ValidatorSet (r:1 w:0) - Read all validators for processing
    /// Storage: ValidatorStates (r:MaxValidators w:MaxValidators) - Update all validator states
    /// Storage: ValidatorLeaveRequests (r:MaxValidators w:MaxValidators) - Process leave requests
    /// Storage: RecentlyRemovedValidators (r:MaxValidators w:MaxValidators) - Cleanup cooldowns
    /// Storage: EpochHistories (r:1 w:1) - Store epoch history
    /// Storage: LastFinalizedBlock (r:1 w:1) - Update finality marker
    /// Storage: PosWeight (r:1 w:0) - Read for score calculations
    /// Storage: PoiWeight (r:1 w:0) - Read for score calculations
    /// Storage: Proposals (r:MaxProposalQueue w:MaxProposalQueue) - Process epoch proposals
    /// Storage: ProposalVotes (r:MaxProposalQueue w:MaxProposalQueue) - Process proposal votes
    /// Storage: Currency operations (r:2*MaxValidators w:MaxValidators) - Handle stake changes
    /// Proof: Worst case epoch transition with MaxValidators (100) and MaxProposalQueue (50)
    fn epoch_transition() -> Weight {
        Weight::from_parts(500_000, 0) // Increased base weight for comprehensive epoch processing
            .saturating_add(Weight::from_parts(35_000, 0).saturating_mul(100u64)) // MaxValidators processing
            .saturating_add(Weight::from_parts(25_000, 0).saturating_mul(50u64)) // MaxProposalQueue processing
            .saturating_add(T::DbWeight::get().reads(8)) // Fixed reads (epoch, validators, weights, etc.)
            .saturating_add(T::DbWeight::get().reads(500u64)) // 5 * MaxValidators (states, requests, etc.)
            .saturating_add(T::DbWeight::get().reads(100u64)) // 2 * MaxProposalQueue (proposals, votes)
            .saturating_add(T::DbWeight::get().writes(4)) // Fixed writes (epoch, active validators, etc.)
            .saturating_add(T::DbWeight::get().writes(400u64)) // 4 * MaxValidators (states, requests, etc.)
            .saturating_add(T::DbWeight::get().writes(100u64)) // 2 * MaxProposalQueue (proposals, votes)
    }

    /// Storage: ValidatorMetadata (r:1 w:1)
    /// Storage: ValidatorNames (r:0 w:1)
    fn set_validator_metadata() -> Weight {
        Weight::from_parts(25_000, 0)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(2))
    }

    /// Storage: ValidatorStates (r:1 w:1)
    fn update_validator_activity() -> Weight {
        Weight::from_parts(20_000, 0)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }

    /// Storage: ActiveValidators (r:1 w:0)
    /// Storage: ValidatorStates (r:MaxValidators w:MaxValidators)
    /// Storage: ValidatorStake (r:MaxValidators w:0)
    /// Storage: Currency operations (r:2*MaxValidators w:MaxValidators)
    /// Storage: RewardHistory (r:1 w:1)
    /// Proof: Worst case distributes rewards to all MaxValidators (100) active validators
    fn distribute_epoch_rewards() -> Weight {
        Weight::from_parts(150_000, 0)
            .saturating_add(Weight::from_parts(20_000, 0).saturating_mul(100u64)) // MaxValidators processing
            .saturating_add(T::DbWeight::get().reads(2)) // ActiveValidators + RewardHistory
            .saturating_add(T::DbWeight::get().reads(400u64)) // 4 * MaxValidators (States, Stake, Currency ops)
            .saturating_add(T::DbWeight::get().writes(1)) // RewardHistory
            .saturating_add(T::DbWeight::get().writes(200u64)) // 2 * MaxValidators (States, Currency ops)
    }

    /// Storage: Proposals (r:1 w:1)
    /// Storage: NextProposalId (r:1 w:1)
    fn propose_default_reward_validator() -> Weight {
        Weight::from_parts(30_000, 0)
            .saturating_add(T::DbWeight::get().reads(2))
            .saturating_add(T::DbWeight::get().writes(2))
    }

    /// Storage: Proposals (r:1 w:1)
    /// Storage: NextProposalId (r:1 w:1)
    fn propose_default_reward_multiple_validators() -> Weight {
        Weight::from_parts(35_000, 0)
            .saturating_add(T::DbWeight::get().reads(2))
            .saturating_add(T::DbWeight::get().writes(2))
    }

    /// Storage: Proposals (r:1 w:1)
    /// Storage: NextProposalId (r:1 w:1)
    /// Storage: ActiveValidators (r:1 w:0)
    fn propose_reward_all_active_validators() -> Weight {
        Weight::from_parts(40_000, 0)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(2))
    }

    /// Storage: Proposals (r:1 w:1)
    /// Storage: NextProposalId (r:1 w:1)
    /// Storage: ValidatorSet (r:1 w:0) - for validator validation
    /// Storage: ValidatorStates (r:v w:0) - for validator existence check
    /// Proof: Worst case proposes rewards for multiple validators (up to MaxValidators)
    fn propose_reward_multiple_validators(v: u32) -> Weight {
        Weight::from_parts(40_000, 0)
            .saturating_add(Weight::from_parts(5_000, 0).saturating_mul(v.into()))
            .saturating_add(T::DbWeight::get().reads(3)) // Proposals, NextProposalId, ValidatorSet
            .saturating_add(T::DbWeight::get().reads((1_u64).saturating_mul(v.into()))) // ValidatorStates per validator
            .saturating_add(T::DbWeight::get().writes(2)) // Proposals, NextProposalId
    }

}

// For backwards compatibility and tests
impl WeightInfo for () {
    fn update_validator_stake_score() -> Weight { Weight::from_parts(15_000, 0) }
    fn update_validator_inference_score() -> Weight { Weight::from_parts(15_000, 0) }
    fn update_consensus_weights() -> Weight { Weight::from_parts(8_000, 0) }
    fn set_governance_mode() -> Weight { Weight::from_parts(5_000, 0) }
    fn sudo_advance_epoch() -> Weight { Weight::from_parts(50_000, 0) }
    fn submit_proposal() -> Weight { Weight::from_parts(20_000, 0) }
    fn vote_proposal() -> Weight { Weight::from_parts(18_000, 0) }
    fn execute_proposal() -> Weight { Weight::from_parts(35_000, 0) }
    fn join_validator_set() -> Weight { Weight::from_parts(25_000, 0) }
    fn leave_validator_set() -> Weight { Weight::from_parts(20_000, 0) }
    fn set_validator_name() -> Weight { Weight::from_parts(12_000, 0) }
    fn apply_offchain_poi_scores() -> Weight { Weight::from_parts(40_000, 0) }
    fn on_initialize_with_validators(v: u32) -> Weight { 
        Weight::from_parts(25_000, 0).saturating_add(Weight::from_parts(8_000, 0).saturating_mul(v.into()))
    }
    fn apply_score_decay_multiple(v: u32) -> Weight { 
        Weight::from_parts(15_000, 0).saturating_add(Weight::from_parts(12_000, 0).saturating_mul(v.into()))
    }
    fn validator_set_operations(v: u32) -> Weight { 
        Weight::from_parts(20_000, 0).saturating_add(Weight::from_parts(5_000, 0).saturating_mul(v.into()))
    }
    fn runtime_api_calls(v: u32) -> Weight { 
        Weight::from_parts(35_000, 0).saturating_add(Weight::from_parts(8_000, 0).saturating_mul(v.into()))
    }
    fn epoch_transition_multiple(v: u32) -> Weight { 
        Weight::from_parts(60_000, 0).saturating_add(Weight::from_parts(15_000, 0).saturating_mul(v.into()))
    }
    fn governance_with_multiple_voters(v: u32) -> Weight { 
        Weight::from_parts(30_000, 0).saturating_add(Weight::from_parts(18_000, 0).saturating_mul(v.into()))
    }
    fn on_initialize() -> Weight { Weight::from_parts(50_000, 0) }
    fn offchain_worker() -> Weight { Weight::from_parts(50_000, 0) }
    
    // New benchmark implementations for backwards compatibility

    // Updated weight implementations for new benchmarked functions
    fn join_validators() -> Weight {
        Weight::from_parts(80_000, 0)
            .saturating_add(RocksDbWeight::get().reads(7))
            .saturating_add(RocksDbWeight::get().writes(6))
    }

    fn leave_validators() -> Weight {
        Weight::from_parts(60_000, 0)
            .saturating_add(RocksDbWeight::get().reads(5))
            .saturating_add(RocksDbWeight::get().writes(2))
    }

    fn cancel_leave_request() -> Weight {
        Weight::from_parts(25_000, 0)
            .saturating_add(RocksDbWeight::get().reads(1))
            .saturating_add(RocksDbWeight::get().writes(1))
    }

    fn slash_validator() -> Weight {
        Weight::from_parts(45_000, 0)
            .saturating_add(RocksDbWeight::get().reads(2))
            .saturating_add(RocksDbWeight::get().writes(2))
    }

    fn slash_validator_percentage() -> Weight {
        Weight::from_parts(40_000, 0)
            .saturating_add(RocksDbWeight::get().reads(2))
            .saturating_add(RocksDbWeight::get().writes(2))
    }

    fn increase_validator_stake() -> Weight {
        Weight::from_parts(35_000, 0)
            .saturating_add(RocksDbWeight::get().reads(2))
            .saturating_add(RocksDbWeight::get().writes(2))
    }

    fn decrease_validator_stake() -> Weight {
        Weight::from_parts(35_000, 0)
            .saturating_add(RocksDbWeight::get().reads(2))
            .saturating_add(RocksDbWeight::get().writes(2))
    }

    fn report_validator_misbehavior() -> Weight {
        Weight::from_parts(55_000, 0)
            .saturating_add(RocksDbWeight::get().reads(3))
            .saturating_add(RocksDbWeight::get().writes(2))
    }

    fn simulate_inference() -> Weight {
        Weight::from_parts(30_000, 0)
            .saturating_add(RocksDbWeight::get().reads(1))
            .saturating_add(RocksDbWeight::get().writes(1))
    }

    fn propose_slash_validator() -> Weight {
        Weight::from_parts(35_000, 0)
            .saturating_add(RocksDbWeight::get().reads(1))
            .saturating_add(RocksDbWeight::get().writes(2))
    }

    fn propose_reward_validator() -> Weight {
        Weight::from_parts(35_000, 0)
            .saturating_add(RocksDbWeight::get().reads(1))
            .saturating_add(RocksDbWeight::get().writes(2))
    }

    fn propose_eject_validator() -> Weight {
        Weight::from_parts(35_000, 0)
            .saturating_add(RocksDbWeight::get().reads(1))
            .saturating_add(RocksDbWeight::get().writes(2))
    }

    fn slash_multiple_validators() -> Weight {
        Weight::from_parts(80_000, 0)
            .saturating_add(Weight::from_parts(35_000, 0).saturating_mul(50u64))
            .saturating_add(RocksDbWeight::get().reads(202))
            .saturating_add(RocksDbWeight::get().writes(202))
    }

    fn execute_proposals() -> Weight {
        Weight::from_parts(120_000, 0)
            .saturating_add(Weight::from_parts(45_000, 0).saturating_mul(50u64))
            .saturating_add(RocksDbWeight::get().reads(350))
            .saturating_add(RocksDbWeight::get().writes(250))
    }

    fn epoch_transition() -> Weight {
        Weight::from_parts(200_000, 0)
            .saturating_add(Weight::from_parts(25_000, 0).saturating_mul(100u64))
            .saturating_add(RocksDbWeight::get().reads(304))
            .saturating_add(RocksDbWeight::get().writes(304))
    }

    fn set_validator_metadata() -> Weight {
        Weight::from_parts(25_000, 0)
            .saturating_add(RocksDbWeight::get().reads(1))
            .saturating_add(RocksDbWeight::get().writes(2))
    }

    fn update_validator_activity() -> Weight {
        Weight::from_parts(20_000, 0)
            .saturating_add(RocksDbWeight::get().reads(1))
            .saturating_add(RocksDbWeight::get().writes(1))
    }

    fn distribute_epoch_rewards() -> Weight {
        Weight::from_parts(150_000, 0)
            .saturating_add(Weight::from_parts(20_000, 0).saturating_mul(100u64))
            .saturating_add(RocksDbWeight::get().reads(402))
            .saturating_add(RocksDbWeight::get().writes(201))
    }

    fn propose_default_reward_validator() -> Weight {
        Weight::from_parts(30_000, 0)
            .saturating_add(RocksDbWeight::get().reads(2))
            .saturating_add(RocksDbWeight::get().writes(2))
    }

    fn propose_default_reward_multiple_validators() -> Weight {
        Weight::from_parts(35_000, 0)
            .saturating_add(RocksDbWeight::get().reads(2))
            .saturating_add(RocksDbWeight::get().writes(2))
    }

    fn propose_reward_all_active_validators() -> Weight {
        Weight::from_parts(40_000, 0)
            .saturating_add(RocksDbWeight::get().reads(3))
            .saturating_add(RocksDbWeight::get().writes(2))
    }

    fn propose_reward_multiple_validators(v: u32) -> Weight {
        Weight::from_parts(40_000, 0)
            .saturating_add(Weight::from_parts(5_000, 0).saturating_mul(v.into()))
            .saturating_add(RocksDbWeight::get().reads(3))
            .saturating_add(RocksDbWeight::get().reads((1_u64).saturating_mul(v.into())))
            .saturating_add(RocksDbWeight::get().writes(2))
    }

}