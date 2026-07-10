# pallet-cerulea-poi

A Substrate pallet implementing Proof-of-Inference (PoI) consensus for the Cerulea, enabling decentralized validation of AI/ML inference results through a challenge-based mechanism.

## Overview

The PoI pallet provides the Proof of Inference functionality for Cerulea, handling inference submission, validation, and challenge mechanisms. It integrates with the DCF pallet to provide the inference component of the hybrid consensus system.

## Key Features

- **Inference Management**: Store and validate inference submissions with confidence scores
- **Challenge System**: Handle disputes between validators through challenge mechanisms
- **Epoch Control**: Manage epoch transitions and validity windows for results
- **PoS Integration**: Seamless integration with Proof of Stake system for scoring

## Configuration

```rust
#[pallet::config]
pub trait Config: frame_system::Config {
    /// The overarching event type
    type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
    
    /// Weight information for extrinsics
    type WeightInfo: WeightInfo;
    
    /// Minimum confidence threshold for inference (0-100)
    type MinInferenceConfidence: Get<u32>;
    
    /// Maximum age of inference in epochs
    type MaxInferenceAge: Get<u32>;
    
    /// Number of epochs to challenge an inference
    type ChallengeWindow: Get<u32>;
    
    /// Reward for correct inference
    type InferenceReward: Get<u128>;
    
    /// Reward for successful challenge
    type ChallengeReward: Get<u128>;
    
    /// Interface to PoS pallet for boosting/slashing scores
    type PosInterface: PosInterface<Self::AccountId>;
    
    /// Interface to DCF pallet for inference tracking
    type DcfInterface: DcfInterface<Self::AccountId>;
}
```

## Storage

### InferenceResults
```rust
#[pallet::storage]
pub type InferenceResults<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, (u32, u32), OptionQuery>;
```

### Challenges
```rust
#[pallet::storage]
pub type Challenges<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, (T::AccountId, u32, u32), OptionQuery>;
```

### CurrentEpoch
```rust
#[pallet::storage]
pub type CurrentEpoch<T: Config> = StorageValue<_, u32, ValueQuery>;
```

## Events

```rust
#[pallet::event]
pub enum Event<T: Config> {
    InferenceSubmitted { who: T::AccountId, result: u32, confidence: u32 },
    InferenceAccepted { validator: T::AccountId, confidence: u32 },
    InferenceRejected { validator: T::AccountId, confidence: u32 },
    InferenceChallenged { challenger: T::AccountId, challenged: T::AccountId, result: u32 },
    ChallengeResolved { challenger: T::AccountId, challenged: T::AccountId, result: u32, success: bool },
    ValidatorSlashed { validator: T::AccountId, reason: Vec<u8> },
}
```

## Errors

```rust
#[pallet::error]
pub enum Error<T> {
    InferenceAlreadySubmitted,
    InferenceNotFound,
    ConfidenceTooLow,
    ChallengeWindowExpired,
    InferenceTooOld,
    InvalidChallenge,
}
```

## Dispatchable Functions

### submit_inference
```rust
pub fn submit_inference(origin: OriginFor<T>, result: u32, confidence: u32) -> DispatchResult
```
Submit an inference result with confidence score (0-100).

### challenge_inference
```rust
pub fn challenge_inference(origin: OriginFor<T>, challenged: T::AccountId, result: u32) -> DispatchResult
```
Challenge another validator's inference result within the challenge window.

## Runtime API

```rust
sp_api::decl_runtime_apis! {
    pub trait PoiApi<AccountId>
    where
        AccountId: codec::Codec + Clone + Eq + sp_std::fmt::Debug,
    {
        /// Get the inference result and epoch for a validator
        fn get_inference_result(validator: AccountId) -> Option<(u32, u32)>;
        
        /// Get the challenge (challenger, result, epoch) for a validator
        fn get_challenge(validator: AccountId) -> Option<(AccountId, u32, u32)>;
        
        /// Get the current inference epoch
        fn get_current_epoch() -> u32;
    }
}
```

## Genesis Configuration

```rust
#[pallet::genesis_config]
pub struct GenesisConfig<T: Config> {
    pub inference_results: Vec<(T::AccountId, u32, u32)>,
    pub current_epoch: u32,
}
```

## Usage Example

```rust
// Submit inference result
assert_ok!(PalletCbcPoi::submit_inference(Origin::signed(validator_account), 42, 85));

// Challenge inference
assert_ok!(PalletCbcPoi::challenge_inference(
    Origin::signed(challenger_account), 
    challenged_account, 
    42
));

// Get inference result
let result = PalletCbcPoi::get_inference_result(validator_account);
```

## Development

### Building
```bash
cargo build -p pallet-cerulea-poi
```

### Testing
```bash
cargo test -p pallet-cerulea-poi
```

### Benchmarking
```bash
cargo bench -p pallet-cerulea-poi
```

## Integration

The PoI pallet integrates with:

- **Other Pallets**: PoS (via PosInterface), DCF (Dynamic Consensus Framework), Balances
- **Runtime APIs**: Inference queries, Challenge queries, Epoch queries
- **FRAME System**: Event system, Storage system, Weight system

## Security Considerations

- **Inference Validation**: Minimum confidence thresholds, epoch-based validity, challenge windows
- **Challenge System**: Time-limited challenges, result validation, automatic resolution
- **Reward System**: Incentives for accurate reporting, penalties for false challenges
- **Integration Security**: Secure PoS interface, proper event handling, state consistency

## References

- [DCF Pallet](../pallet-cerulea-dcf/README.md)
- [PoS Pallet](../pallet-cerulea-pos/README.md)
- [Runtime Overview](../../docs/runtime-overview.md)