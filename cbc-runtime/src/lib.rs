// This attribute ensures that the runtime is compiled without the standard library (`no_std`) 
// when the `std` feature is not enabled. This is required for Substrate runtimes to run in a 
// WebAssembly (Wasm) environment.
#![cfg_attr(not(feature = "std"), no_std)]

// Include the Wasm binary generated during the build process when the `std` feature is enabled.
// This binary is used for native execution of the runtime.
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

// Declare runtime modules (pallets) and other components.
pub mod apis; // Runtime APIs exposed to the outside world.
#[cfg(feature = "runtime-benchmarks")]
mod benchmarks; // Benchmarking logic for runtime performance.
pub mod configs; // Configuration settings for the runtime.

extern crate alloc; // Import the `alloc` crate for heap-allocated data structures in `no_std` environments.
use alloc::vec::Vec; // Import the `Vec` type for dynamic arrays.

use sp_runtime::{
    generic, impl_opaque_keys,
    traits::{BlakeTwo256, IdentifyAccount, Verify},
    MultiAddress, MultiSignature,
};
use sp_application_crypto::ed25519::AppPublic as DcfPublic;
#[cfg(feature = "std")]
use sp_version::NativeVersion; // Used for native runtime versioning.
use sp_version::RuntimeVersion; // Defines the runtime version.

pub use frame_system::Call as SystemCall; // Expose the `frame_system` pallet's call type.
pub use pallet_balances::Call as BalancesCall; // Expose the `pallet_balances` pallet's call type.
pub use pallet_timestamp::Call as TimestampCall; // Expose the `pallet_timestamp` pallet's call type.
#[cfg(any(feature = "std", test))]
pub use sp_runtime::BuildStorage; // Utility for building storage during tests or native execution.

pub mod genesis_config_presets; // Preset configurations for the genesis block.

// Test and benchmarking modules
#[cfg(test)]
pub mod mock; // Mock runtime for testing

#[cfg(test)]
pub mod tests; // Runtime integration tests with API unit tests

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmark_tests; // Benchmarking tests

/// Opaque types are used to abstract away the specifics of runtime data structures.
/// These types are used by the CLI and other tools to interact with the runtime without
/// needing to know the exact implementation details.
pub mod opaque {
    use super::*;
    use sp_runtime::{
        generic,
        traits::{BlakeTwo256, Hash as HashT},
    };

    pub use sp_runtime::OpaqueExtrinsic as UncheckedExtrinsic; // Opaque extrinsic type.

    /// Opaque block header type. This hides the specifics of the header structure.
    pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
    /// Opaque block type. This hides the specifics of the block structure.
    pub type Block = generic::Block<Header, UncheckedExtrinsic>;
    /// Opaque block identifier type. Used to identify blocks.
    pub type BlockId = generic::BlockId<Block>;
    /// Opaque block hash type. Represents the hash of a block.
    pub type Hash = <BlakeTwo256 as HashT>::Output;
}

// Define the session keys used for consensus mechanisms like POS and POI.
impl_opaque_keys! {
    pub struct SessionKeys {
        pub dcf: DcfPublic, // DCF consensus key using ed25519
    }
}

// Define the runtime version. This is critical for ensuring compatibility between the native
// runtime and the Wasm runtime. It also helps tools like Polkadot-JS Apps to interact with the chain.
#[sp_version::runtime_version]
pub const VERSION: RuntimeVersion = RuntimeVersion {
    spec_name: alloc::borrow::Cow::Borrowed("cbc-runtime"), // Name of the runtime specification.
    impl_name: alloc::borrow::Cow::Borrowed("cbc-runtime"), // Name of the runtime implementation.
    authoring_version: 1, // Version of the authoring logic.
    spec_version: 100, // Version of the runtime specification.
    impl_version: 1, // Version of the runtime implementation.
    apis: apis::RUNTIME_API_VERSIONS, // Runtime APIs exposed by this runtime.
    transaction_version: 1, // Version of the transaction format.
    system_version: 1, // Version of the system logic.
};

mod block_times {
    /// Defines the average expected block time in milliseconds for DCF consensus
    pub const MILLI_SECS_PER_BLOCK: u64 = 6000;

    // The slot duration is the minimum time between blocks
    pub const SLOT_DURATION: u64 = MILLI_SECS_PER_BLOCK;
}
pub use block_times::*;

// Constants for time measurement in terms of blocks.
pub const MINUTES: BlockNumber = 60_000 / (MILLI_SECS_PER_BLOCK as BlockNumber);
pub const HOURS: BlockNumber = MINUTES * 60;
pub const DAYS: BlockNumber = HOURS * 24;

// Constants for blockchain parameters.
pub const BLOCK_HASH_COUNT: BlockNumber = 2400; // Number of recent blocks to store in the block hash map.

// Constants for balances.
pub const UNIT: Balance = 1_000_000_000_000; // Base unit for balances.
pub const MILLI_UNIT: Balance = 1_000_000_000; // Milli unit for balances.
pub const MICRO_UNIT: Balance = 1_000_000; // Micro unit for balances.
pub const EXISTENTIAL_DEPOSIT: Balance = MILLI_UNIT; // Minimum balance required to keep an account alive.
pub const CBC: Balance = UNIT; // 1 CBC token equals 1 UNIT (10^12 subunits)

// Define the native runtime version for native execution.
#[cfg(feature = "std")]
pub fn native_version() -> NativeVersion {
    NativeVersion {
        runtime_version: VERSION,
        can_author_with: Default::default(),
    }
}

// Type aliases for commonly used types in the runtime.
pub type Signature = MultiSignature; // Signature type for transactions.
pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId; // Account identifier.
pub type Balance = u128; // Balance type.
pub type Nonce = u32; // Nonce type for transactions.
pub type Hash = sp_core::H256; // Hash type.
pub type BlockNumber = u32; // Block number type.
pub type Address = MultiAddress<AccountId, ()>; // Address type for accounts.
pub type Header = generic::Header<BlockNumber, BlakeTwo256>; // Block header type.
pub type Block = generic::Block<Header, UncheckedExtrinsic>; // Block type.
pub type SignedBlock = generic::SignedBlock<Block>; // Signed block type.
pub type BlockId = generic::BlockId<Block>; // Block identifier type.

// Define the transaction extensions used in the runtime.
pub type TxExtension = (
    frame_system::CheckNonZeroSender<Runtime>,
    frame_system::CheckSpecVersion<Runtime>,
    frame_system::CheckTxVersion<Runtime>,
    frame_system::CheckGenesis<Runtime>,
    frame_system::CheckEra<Runtime>,
    frame_system::CheckNonce<Runtime>,
    frame_system::CheckWeight<Runtime>,
    pallet_transaction_payment::ChargeTransactionPayment<Runtime>,
    frame_metadata_hash_extension::CheckMetadataHash<Runtime>,
    frame_system::WeightReclaim<Runtime>,
);

// Define the unchecked extrinsic type for the runtime.
pub type UncheckedExtrinsic =
    generic::UncheckedExtrinsic<Address, RuntimeCall, Signature, TxExtension>;

// Define the payload being signed in transactions.
pub type SignedPayload = generic::SignedPayload<RuntimeCall, TxExtension>;

#[allow(unused_parens)]
type Migrations = ();

// Define the executive type, which handles dispatching calls to the appropriate pallets.
pub type Executive = frame_executive::Executive<
    Runtime,
    Block,
    frame_system::ChainContext<Runtime>,
    Runtime,
    AllPalletsWithSystem,
    Migrations,
>;

// Define the runtime by composing the FRAME pallets.
#[frame_support::runtime]
mod runtime {
    #[runtime::runtime]
    #[runtime::derive(
        RuntimeCall,
        RuntimeEvent,
        RuntimeError,
        RuntimeOrigin,
        RuntimeFreezeReason,
        RuntimeHoldReason,
        RuntimeSlashReason,
        RuntimeLockId,
        RuntimeTask,
        RuntimeViewFunction
    )]
    pub struct Runtime;

    #[runtime::pallet_index(0)]
    pub type System = frame_system;

    #[runtime::pallet_index(1)]
    pub type Timestamp = pallet_timestamp;

    #[runtime::pallet_index(2)]
    pub type Balances = pallet_balances;

    #[runtime::pallet_index(3)]
    pub type TransactionPayment = pallet_transaction_payment;

    #[runtime::pallet_index(4)]
    pub type Sudo = pallet_sudo;

    #[runtime::pallet_index(6)]
    pub type PalletCbcPoi = pallet_cbc_poi;

    #[runtime::pallet_index(7)]
    pub type PalletCbcPos = pallet_cbc_pos;

    #[runtime::pallet_index(8)]
    pub type Dcf = pallet_cbc_dcf::Pallet<Runtime>;

    /// Simple on-chain Todo list pallet
    #[runtime::pallet_index(9)]
    pub type Todo = pallet_todo;
}
use sp_runtime::traits::parameter_types;

parameter_types! {
	pub const EnterDuration: BlockNumber = 4 * HOURS;
	pub const EnterDepositAmount: Balance = 2_000_000 * CBC;
	pub const ExtendDuration: BlockNumber = 2 * HOURS;
	pub const ExtendDepositAmount: Balance = 1_000_000 * CBC;
	pub const ReleaseDelay: u32 = 2 * DAYS;

    // Validator parameters
    pub const MinValidatorScore: u32 = 50;
    pub const MinActiveValidators: u32 = 3;
    pub const MaxValidators: u32 = 100;
    pub const ValidatorScoreDecay: u32 = 10;
    pub const MaxSlashingCount: u32 = 3;
   

    // Inference parameters
    pub const MinInferenceConfidence: u32 = 80;
    pub const MaxInferenceAge: u32 = 10;
    pub const ChallengeWindow: u32 = 5;
    pub const InferenceReward: u128 = 1000;
    pub const ChallengeReward: u128 = 500;

    // DCF parameters
    pub const DcfMaxValidators: u32 = 100;
    pub const DefaultPosWeight: u64 = 6000; // 60% weight for PoS score (6000/10000)
    pub const DefaultPoiWeight: u64 = 4000; // 40% weight for PoI score (4000/10000)
    pub const MinStake: Balance = 1000 * CBC; // Minimum stake required
    pub const MaxValidatorsPerEpoch: u32 = 50; // Maximum validators per epoch
    pub const MaxValidatorScore: u64 = 100;

    // Block authorship and inference boosting parameters
    pub const BlockAuthorshipBoost: u64 = 10;
    pub const MissedBlockPenalty: u64 = 5;
    pub const InferenceBoostLow: u64 = 2;
    pub const InferenceBoostMedium: u64 = 5;
    pub const InferenceBoostHigh: u64 = 10;
    pub const InferencePenaltyLow: u64 = 1;
    pub const InferencePenaltyMedium: u64 = 3;
    pub const InferencePenaltyHigh: u64 = 7;

    pub const MaxEpochHistory: u32 = 24;
    
    // Performance thresholds
    pub const MinPerformanceScore: u64 = 30;
    pub const HighPerformanceScore: u64 = 80;
    pub const MinParticipationRate: u32 = 50;
    pub const HighParticipationRate: u32 = 90;
    pub const MaxMissedBlocks: u32 = 10;
    pub const MaxMissedBlocksHigh: u32 = 2;
    pub const HealthyValidatorScore: u64 = 50;
    pub const HealthyParticipationRate: u32 = 80;
    pub const HealthyMissedBlocksMax: u32 = 5;
    
    // Score calculation thresholds
    pub const ScoreChangeThreshold: u64 = 1000;
    pub const ScoreChangePercentage: u32 = 10;
    pub const ScoreImprovementThreshold: u64 = 1000;
    pub const ScoreImprovementPercentage: u32 = 10;
    
    // Contribution balance thresholds
    pub const MaxPosContribution: u32 = 90;
    pub const MaxPoiContribution: u32 = 90;
    pub const ImbalanceWarningThreshold: u32 = 85;
    
    // Block processing intervals
    pub const LeaveRequestCheckInterval: u32 = 10;
    pub const MetricsUpdateInterval: u32 = 10;
    pub const ScoreRefreshInterval: u32 = 50;
    pub const DetailedLoggingInterval: u32 = 100;
    pub const ImbalanceCheckInterval: u32 = 500;
    
    // Validator set limits
    pub const TopValidatorsDisplayCount: u32 = 5;
    pub const HealthCheckSampleSize: u32 = 5;
    
    // Percentage constants
    pub const FullPercentage: u32 = 100;
    pub const HighPerformancePercentage: u32 = 80;
    pub const TopPerformerPercentage: u32 = 20;
    
    // Misbehavior reporting
    pub const MaxEvidenceLength: u32 = 1000;
    pub const MisbehaviorSlashThreshold: u32 = 3;
}

pub use pallet_cbc_poi;
pub use pallet_cbc_pos;
pub use pallet_todo;


