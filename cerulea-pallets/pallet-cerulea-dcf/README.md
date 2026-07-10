# pallet-cerulea-dcf

The **Dynamic Consensus Framework (DCF)** pallet manages validators, scoring, and governance for the Cerulea using a hybrid PoS/PoI system.

## Overview

The DCF pallet implements the core consensus mechanism for Cerulea, combining Proof of Stake (PoS) and Proof of Inference (PoI) to create a dynamic validator selection and scoring system. It provides advanced features for validator management, score calculation, and on-chain governance.

## Key Features

- **Dynamic Validator Management**: Validator joining/leaving system with score-based selection
- **Hybrid Scoring System**: Combined PoS and PoI scoring with configurable weight distribution
- **Epoch Management**: Automatic epoch transitions with validator set updates and historical tracking
- **On-Chain Governance**: Proposal submission, voting mechanism, and execution system
- **Security Features**: Validator slashing, score penalties, ejection mechanism, and governance controls

## Configuration

```rust
#[pallet::config]
pub trait Config: frame_system::Config + pos::Config + poi::Config + TypeInfo + fmt::Debug {
    type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
    
    // Validator set configuration
    #[pallet::constant]
    type MaxValidators: Get<u32>;
    
    #[pallet::constant]
    type MinActiveValidators: Get<u32>;
    
    #[pallet::constant]
    type MaxEpochHistory: Get<u32>;
    
    // Scoring weights and thresholds
    #[pallet::constant]
    type DefaultPosWeight: Get<u64>;
    
    #[pallet::constant]
    type DefaultPoiWeight: Get<u64>;
    
    #[pallet::constant]
    type MinValidatorScore: Get<u32>;
    
    #[pallet::constant]
    type MaxValidatorScore: Get<u64>;
    
    // Score decay and activity parameters
    #[pallet::constant]
    type ValidatorScoreDecay: Get<u32>;
    
    #[pallet::constant]
    type MaxInactiveEpochs: Get<u32>;
    
    #[pallet::constant]
    type ScoreDecayInterval: Get<u32>;
    
    #[pallet::constant]
    type ParticipationUpdateInterval: Get<u32>;
    
    #[pallet::constant]
    type UnderperformanceCheckInterval: Get<u32>;
    
    #[pallet::constant]
    type ValidatorProposalInterval: Get<u32>;
    
    // Additional configuration types...
}
```

## Storage

### ValidatorStates
```rust
#[pallet::storage]
pub type ValidatorStates<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, ValidatorState, OptionQuery>;
```

### CurrentEpoch
```rust
#[pallet::storage]
pub type CurrentEpoch<T: Config> = StorageValue<_, u32, ValueQuery>;
```

### EpochHistory
```rust
#[pallet::storage]
pub type EpochHistory<T: Config> = StorageValue<_, BoundedVec<EpochHistory<T>, ConstU32<24>>, ValueQuery>;
```

## Events

```rust
#[pallet::event]
pub enum Event<T: Config> {
    ValidatorScoreUpdated { validator: T::AccountId, score: u64 },
    ValidatorJoined { validator: T::AccountId },
    ValidatorLeft { validator: T::AccountId },
    ProposalSubmitted { proposal_id: u32, proposer: T::AccountId },
    ProposalVoted { proposal_id: u32, voter: T::AccountId, approve: bool },
    ProposalExecuted { proposal_id: u32 },
    ValidatorSlashed { validator: T::AccountId, amount: BalanceOf<T> },
    ValidatorRewarded { validator: T::AccountId, amount: BalanceOf<T> },
    ValidatorEjected { validator: T::AccountId, reason: EjectionReason },
    ScoreBoosted { validator: T::AccountId, amount: u64, reason: ScoreBoostReason },
    ScoreDecayed { validator: T::AccountId, amount: u64 },
}
```

## Errors

```rust
#[pallet::error]
pub enum Error<T> {
    ValidatorNotFound,
    InvalidWeight,
    InvalidEpochConfig,
    NotEnoughValidators,
    ProposalNotFound,
    InvalidVote,
    NotAuthorized,
    GovernanceModeDisabled,
    EpochTransitionFailed,
    ScoreTooLow,
    ValidatorInactive,
    InvalidScoreBoost,
    InvalidEjectionReason,
    InvalidInferenceError,
}
```

## Dispatchable Functions

### Validator Management
```rust
pub fn join_validators(origin: OriginFor<T>, stake: BalanceOf<T>) -> DispatchResult
pub fn leave_validators(origin: OriginFor<T>) -> DispatchResult
pub fn cancel_leave_request(origin: OriginFor<T>) -> DispatchResult
pub fn increase_validator_stake(origin: OriginFor<T>, additional_stake: BalanceOf<T>) -> DispatchResult
pub fn decrease_validator_stake(origin: OriginFor<T>, amount: BalanceOf<T>) -> DispatchResult
```

### Score Management
```rust
pub fn update_validator_stake_score(origin: OriginFor<T>, validator: T::AccountId, score: u32) -> DispatchResult
pub fn update_validator_inference_score(origin: OriginFor<T>, validator: T::AccountId, score: u32) -> DispatchResult
pub fn update_consensus_weights(origin: OriginFor<T>, pos_weight: u64, poi_weight: u64) -> DispatchResult
```

### Governance
```rust
pub fn submit_proposal(origin: OriginFor<T>, action: ProposalAction<T>) -> DispatchResult
pub fn vote_proposal(origin: OriginFor<T>, proposal_id: u32, approve: bool) -> DispatchResult
pub fn execute_proposal(origin: OriginFor<T>, proposal_id: u32) -> DispatchResult
```

### Sudo Operations
```rust
pub fn set_governance_mode(origin: OriginFor<T>, enabled: bool) -> DispatchResult
pub fn sudo_advance_epoch(origin: OriginFor<T>) -> DispatchResult
pub fn slash_validator(origin: OriginFor<T>, validator: T::AccountId, amount: BalanceOf<T>) -> DispatchResult
```

## Runtime API

```rust
sp_api::decl_runtime_apis! {
    pub trait DcfApi<AccountId, Balance, BlockNumber> {
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
        fn get_validator_stake(validator: AccountId) -> u128;
        fn get_leave_request_status(validator: AccountId) -> Option<(u32, u32, bool)>;
        fn get_epoch_length() -> u32;
        fn validate_block_author_strict(block_number: u32, actual_author: AccountId) -> Result<(), u8>;
        fn get_governance_config() -> Vec<u8>;
    }
}
```

## Genesis Configuration

```rust
#[pallet::genesis_config]
pub struct GenesisConfig<T: Config> {
    pub initial_validators: Vec<T::AccountId>,
    pub initial_scores: Vec<(T::AccountId, u64)>,
    pub initial_epoch_config: EpochConfig,
    pub initial_consensus_weights: (u64, u64),
    pub initial_governance_mode: bool,
}
```

## Usage Example

```rust
// Join validator set
assert_ok!(PalletCbcDcf::join_validator_set(Origin::signed(validator_account)));

// Submit governance proposal
assert_ok!(PalletCbcDcf::submit_proposal(
    Origin::signed(proposer_account),
    ProposalAction::Reward {
        validator: validator_account,
        amount: reward_amount
    }
));

// Vote on proposal
assert_ok!(PalletCbcDcf::vote_proposal(
    Origin::signed(voter_account),
    proposal_id,
    true // approve
));

// Get validator score
let score = PalletCbcDcf::get_validator_stake_score(validator_account);
```

## Development

### Building
```bash
cargo build -p pallet-cerulea-dcf
```

### Testing
```bash
cargo test -p pallet-cerulea-dcf
```

### Benchmarking
```bash
cargo bench -p pallet-cerulea-dcf
```

## Integration

The DCF pallet integrates with:

- **Other Pallets**: [PoS](../pallet-cerulea-pos/README.md) (via pos::Config), [PoI](../pallet-cerulea-poi/README.md) (via poi::Config), Balances, Timestamp
- **Runtime APIs**: Validator queries, Score queries, Epoch queries, Governance queries
- **FRAME System**: Event system, Storage system, Weight system

## Security Considerations

- **Validator Management**: Score thresholds for joining, Maximum validator limit, Ejection mechanism
- **Score System**: Score decay prevention, Minimum score requirements, Score normalization
- **Governance**: Proposal quorum requirements, Voting period enforcement, Execution validation
- **Epoch Management**: Validator rotation security, Score snapshot integrity, Historical tracking

## References

- [Architecture Documentation](../../docs/dcf-architecture.md)
- [PoS Pallet](../pallet-cerulea-pos/README.md)
- [PoI Pallet](../pallet-cerulea-poi/README.md)
- [Runtime Overview](../../docs/runtime-overview.md)
