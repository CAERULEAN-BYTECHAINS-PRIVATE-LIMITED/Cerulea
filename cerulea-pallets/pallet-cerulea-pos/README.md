# pallet-cerulea-pos

The CBC PoS (Proof of Stake) pallet implements the PoS consensus mechanism for the CBC blockchain, managing validator registration, scoring, staking, and slashing.

## Overview

The PoS pallet provides the Proof of Stake functionality for Cerulea, handling validator management, stake-based scoring, and slashing mechanisms. It integrates with the DCF pallet to provide the stake component of the hybrid consensus system.

## Key Features

- **Validator Management**: Registration with maximum limits, active/inactive status tracking, and minimum stake requirements
- **Score System**: Score submission with thresholds, dynamic adjustments, and per-epoch decay
- **Stake Management**: Bonding and unbonding with minimum enforcement and stake-based selection
- **Slashing Mechanism**: Configurable slashing counts, automatic removal, and event tracking

## Configuration

```rust
#[pallet::config]
pub trait Config: frame_system::Config {
    type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
    type WeightInfo: WeightInfo;
    
    /// Minimum score required for a validator to be considered active
    type MinValidatorScore: Get<u32>;
    
    /// Minimum number of active validators required
    type MinActiveValidators: Get<u32>;
    
    /// Maximum number of validators allowed
    type MaxValidators: Get<u32>;
    
    /// Score decay per epoch
    type ValidatorScoreDecay: Get<u32>;
    
    /// Maximum slashing count before removal
    type MaxSlashingCount: Get<u32>;
    
    /// Minimum stake amount required for validators
    type MinStake: Get<BalanceOf<Self>>;
    
    /// The balance type
    type Balance: Parameter + Member + AtLeast32BitUnsigned + Default + Copy + MaxEncodedLen;
}
```

## Storage

### Validators
```rust
#[pallet::storage]
pub type Validators<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, bool>;
```

### ValidatorScores
```rust
#[pallet::storage]
pub type ValidatorScores<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, u32>;
```

### CurrentEpoch
```rust
#[pallet::storage]
pub type CurrentEpoch<T: Config> = StorageValue<_, u32, ValueQuery>;
```

### SlashingCount
```rust
#[pallet::storage]
pub type SlashingCount<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, u32>;
```

### Stake
```rust
#[pallet::storage]
pub type Stake<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, BalanceOf<T>, ValueQuery>;
```

## Events

```rust
#[pallet::event]
pub enum Event<T: Config> {
    ValidatorRegistered { validator: T::AccountId },
    ScoreSubmitted { validator: T::AccountId, score: u32 },
    ValidatorSlashed { validator: T::AccountId, slashing_count: u32 },
    ValidatorRemoved { validator: T::AccountId, reason: Vec<u8> },
    StakeBonded { validator: T::AccountId, amount: BalanceOf<T> },
    StakeUnbonded { validator: T::AccountId, amount: BalanceOf<T> },
}
```

## Errors

```rust
#[pallet::error]
pub enum Error<T> {
    ValidatorAlreadyRegistered,
    ValidatorNotRegistered,
    InvalidScore,
    TooManyValidators,
    ScoreTooLow,
    MaxSlashingCountReached,
    InsufficientStake,
    InvalidStakeAmount,
}
```

## Dispatchable Functions

### register_validator
```rust
pub fn register_validator(origin: OriginFor<T>) -> DispatchResult
```
Registers a new validator with minimum stake requirements and enforces maximum validator limit.

### submit_score
```rust
pub fn submit_score(origin: OriginFor<T>, validator: T::AccountId, score: u32) -> DispatchResult
```
Submits a score for a validator with minimum threshold validation.

### slash_validator
```rust
pub fn slash_validator(origin: OriginFor<T>, validator: T::AccountId) -> DispatchResult
```
Slashes a validator and removes them if maximum slashes are reached.

### bond_stake
```rust
pub fn bond_stake(origin: OriginFor<T>, amount: BalanceOf<T>) -> DispatchResult
```
Bonds stake to become/remain a validator with minimum stake enforcement.

### unbond_stake
```rust
pub fn unbond_stake(origin: OriginFor<T>, amount: BalanceOf<T>) -> DispatchResult
```
Unbonds stake from a validator while maintaining minimum requirements.

### boost_score
```rust
pub fn boost_score(origin: OriginFor<T>, validator: T::AccountId, weight: u32) -> DispatchResult
```
Increases validator's score for performance rewards.

### slash_score
```rust
pub fn slash_score(origin: OriginFor<T>, validator: T::AccountId, weight: u32) -> DispatchResult
```
Decreases validator's score for penalties.

## Runtime API

```rust
sp_api::decl_runtime_apis! {
    pub trait PosApi<AccountId, Balance> 
    where
        AccountId: codec::Codec + Clone + Eq + sp_std::fmt::Debug,
        Balance: codec::Codec + Clone + Eq + sp_runtime::traits::AtLeast32BitUnsigned,
    {
        fn get_validator_stake(validator: AccountId) -> Balance;
        fn get_validator_score(validator: AccountId) -> u32;
        fn get_active_validators() -> Vec<AccountId>;
        fn get_slashing_count(validator: AccountId) -> u32;
    }
}
```

## Genesis Configuration

```rust
#[pallet::genesis_config]
pub struct GenesisConfig<T: Config> {
    pub validators: Vec<T::AccountId>,
    pub validator_scores: Vec<u32>,
    pub current_epoch: u32,
    pub slashing_count: Vec<(T::AccountId, u32)>,
}
```

## Usage Example

```rust
// Register a validator
assert_ok!(PalletCbcPos::register_validator(Origin::signed(validator_account)));

// Bond stake
assert_ok!(PalletCbcPos::bond_stake(Origin::signed(validator_account), 1000));

// Submit score
assert_ok!(PalletCbcPos::submit_score(Origin::signed(validator_account), validator_account, 100));

// Get validator stake
let stake = PalletCbcPos::get_validator_stake(validator_account);
```

## Development

### Building
```bash
cargo build -p pallet-cerulea-pos
```

### Testing
```bash
cargo test -p pallet-cerulea-pos
```

### Benchmarking
```bash
cargo bench -p pallet-cerulea-pos
```

## Integration

The PoS pallet integrates with:

- **Other Pallets**: PoI (Proof of Inference), DCF (Dynamic Consensus Framework), Balances
- **Runtime APIs**: Validator queries, Score queries, Stake queries, Slashing queries
- **FRAME System**: Event system, Storage system, Weight system

## Security Considerations

- **Validator Registration**: Limited by maximum validators, requires minimum stake, enforces minimum score
- **Score Management**: Score decay per epoch, minimum score requirement, score boosting and slashing
- **Slashing**: Maximum slashing count, automatic removal after max slashing, stake slashing
- **Stake Management**: Minimum stake requirement, bonding/unbonding validation, stake slashing on misbehavior

## References

- [DCF Pallet](../pallet-cerulea-dcf/README.md)
- [PoI Pallet](../pallet-cerulea-poi/README.md)
- [Runtime Overview](../../docs/runtime-overview.md)



