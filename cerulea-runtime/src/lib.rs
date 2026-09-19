// This attribute ensures that the runtime is compiled without the standard library (`no_std`)
// when the `std` feature is not enabled. This is required for Substrate runtimes to run in a
// WebAssembly (Wasm) environment.
#![cfg_attr(not(feature = "std"), no_std)]
// The runtime now composes sixteen platform modules beside the engine
// modules. The macro that builds the runtime needs a deeper expansion
// budget than the default to cover them.
#![recursion_limit = "512"]

// Include the Wasm binary generated during the build process when the `std` feature is enabled.
// This binary is used for native execution of the runtime.
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

// Declare runtime modules (pallets) and other components.
pub mod apis; // Runtime APIs exposed to the outside world.
#[cfg(feature = "runtime-benchmarks")]
mod benchmarks; // Benchmarking logic for runtime performance.
pub mod configs; // Settings settings for the runtime.

extern crate alloc; // Import the `alloc` crate for heap-allocated data structures in `no_std` environments.
use alloc::vec::Vec; // Import the `Vec` type for dynamic arrays.

use sp_runtime::{
    generic, impl_opaque_keys,
    traits::{BlakeTwo256, ConstU32, IdentifyAccount, Verify},
    MultiAddress, MultiSignature,
};

/// The Cerulea commit key type identifier.
/// Every validator signs a commitment with an ed25519 key held under this
/// identifier.
pub mod cerulea_key_crypto {
    use sp_application_crypto::{app_crypto, ed25519, KeyTypeId};
    /// Key type ID for Cerulea the validator registry consensus keys.
    pub const KEY_TYPE: KeyTypeId = KeyTypeId(*b"cerk");
    app_crypto!(ed25519, KEY_TYPE);
}

/// The Cerulea the validator registry public key type used in SessionKeys and for validator identification.
pub type DcfPublic = cerulea_key_crypto::Public;
#[cfg(feature = "std")]
use sp_version::NativeVersion; // Used for native runtime versioning.
use sp_version::RuntimeVersion; // Defines the runtime version.

pub use frame_system::Call as SystemCall; // Expose the `frame_system` pallet's call type.
pub use pallet_balances::Call as BalancesCall; // Expose the `pallet_balances` pallet's call type.
pub use pallet_timestamp::Call as TimestampCall; // Expose the `pallet_timestamp` pallet's call type.
#[cfg(any(feature = "std", test))]
pub use sp_runtime::BuildStorage; // Utility for building storage during tests or native execution.

pub mod genesis_config_presets; // Preset settingss for the genesis block.
pub mod platform_api; // The runtime side of the platform interfaces.

// Test and benchmarking modules
#[cfg(test)]
pub mod mock; // Mock runtime for testing

#[cfg(test)]
pub mod tests; // Runtime integration tests with API unit tests

#[cfg(test)]
pub mod platform_tests; // The platform module wiring and the platform runtime interfaces

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmark_tests; // Benchmarking tests

/// Opaque types hide the shape of the runtime data structures.
/// External tools read these types to reach the runtime without holding the
/// exact implementation.
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
        pub dcf: DcfPublic, // the framework pallet consensus key using ed25519
    }
}

// Define the runtime version. This is critical for ensuring compatibility between the native
// runtime and the Wasm runtime. It also helps tools like Polkadot-JS Apps to interact with the chain.
#[sp_version::runtime_version]
pub const VERSION: RuntimeVersion = RuntimeVersion {
    spec_name: alloc::borrow::Cow::Borrowed("cerulea-runtime"), // Name of the runtime specification.
    impl_name: alloc::borrow::Cow::Borrowed("cerulea-runtime"), // Name of the runtime implementation.
    authoring_version: 1,                                       // Version of the authoring logic.
    spec_version: 100,                // Version of the runtime specification.
    impl_version: 1,                  // Version of the runtime implementation.
    apis: apis::RUNTIME_API_VERSIONS, // Runtime APIs exposed by this runtime.
    transaction_version: 1,           // Version of the transaction format.
    system_version: 1,                // Version of the system logic.
};

mod block_times {
    /// Defines the average expected block time in milliseconds for the framework pallet consensus
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
/// The accounting unit the deposits read. One unit carries 10^12 subunits.
/// Cerulea issues nothing to a user, so the figure serves the deposit
/// arithmetic alone.
pub const CERULEA: Balance = UNIT;

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
    pub type Poi = pallet_cerulea_poi;

    #[runtime::pallet_index(7)]
    pub type Pos = pallet_cerulea_pos;

    #[runtime::pallet_index(8)]
    pub type Dcf = pallet_cerulea_dcf::Pallet<Runtime>;

    // The validator registry pallet. The Admission Engine lives here.
    #[runtime::pallet_index(10)]
    pub type Registry = pallet_cerulea_dvf::Pallet<Runtime>;

    #[runtime::pallet_index(11)]
    pub type Schema = pallet_cerulea_schema::Pallet<Runtime>;

    #[runtime::pallet_index(12)]
    pub type State = pallet_cerulea_state::Pallet<Runtime>;

    #[runtime::pallet_index(13)]
    pub type Collections = pallet_cerulea_collections::Pallet<Runtime>;

    #[runtime::pallet_index(14)]
    pub type Identity = pallet_cerulea_identity::Pallet<Runtime>;

    #[runtime::pallet_index(15)]
    pub type Permission = pallet_cerulea_permission::Pallet<Runtime>;

    #[runtime::pallet_index(16)]
    pub type Logic = pallet_cerulea_logic::Pallet<Runtime>;

    #[runtime::pallet_index(17)]
    pub type Workflow = pallet_cerulea_workflow::Pallet<Runtime>;

    #[runtime::pallet_index(18)]
    pub type Rules = pallet_cerulea_rules::Pallet<Runtime>;

    #[runtime::pallet_index(19)]
    pub type Governance = pallet_cerulea_governance::Pallet<Runtime>;

    #[runtime::pallet_index(20)]
    pub type Evolution = pallet_cerulea_evolution::Pallet<Runtime>;

    #[runtime::pallet_index(21)]
    pub type Bridge = pallet_cerulea_bridge::Pallet<Runtime>;

    #[runtime::pallet_index(22)]
    pub type Oracle = pallet_cerulea_oracle::Pallet<Runtime>;

    #[runtime::pallet_index(24)]
    pub type Tier = pallet_cerulea_tier::Pallet<Runtime>;

    #[runtime::pallet_index(23)]
    pub type Relayer = pallet_cerulea_relayer::Pallet<Runtime>;

    #[runtime::pallet_index(26)]
    pub type Operator = pallet_cerulea_operator::Pallet<Runtime>;

    #[runtime::pallet_index(25)]
    pub type Audit = pallet_cerulea_audit::Pallet<Runtime>;

    #[runtime::pallet_index(27)]
    pub type Custody = pallet_cerulea_custody::Pallet<Runtime>;
}
use sp_runtime::traits::parameter_types;

parameter_types! {
    pub const EnterDuration: BlockNumber = 4 * HOURS;
    pub const EnterDepositAmount: Balance = 2_000_000 * CERULEA;
    pub const ExtendDuration: BlockNumber = 2 * HOURS;
    pub const ExtendDepositAmount: Balance = 1_000_000 * CERULEA;
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

    // Framework parameters
    pub const DcfMaxValidators: u32 = 100;
    // The Trust Score composition carries the weighting. The Standing Engine in
    // the framework pallet holds it. The public-access chain weighs standing at
    // 40 percent, performance at 30 percent, custody at 20 percent and sampling
    // at 10 percent. A private chain weighs role at 35 percent, attestation at
    // 25 percent, performance at 25 percent and custody at 15 percent.
    pub const MinStake: Balance = 1000 * CERULEA; // Minimum stake required
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

pub use pallet_cerulea_poi;
pub use pallet_cerulea_pos;

parameter_types! {
    pub const StakeWeightFactor: u128 = 1;
    pub const ScoreWeightFactor: u128 = 1000;
    pub const ScoreBoostCap: u128 = 100_000;
    pub const FinalityThreshold: sp_runtime::Perbill = sp_runtime::Perbill::from_percent(67);
    pub const FinalityCheckpointInterval: u32 = 10;
    pub const VoteRetentionRounds: u32 = 20;
}

impl pallet_cerulea_dvf::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Signature = Signature;
    type Signer = <Signature as Verify>::Signer;
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
