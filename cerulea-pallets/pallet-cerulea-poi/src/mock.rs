// Import the pallet as a local crate alias for easier reference in tests.
use crate as pallet_cbc_poi;
use frame_support::{
    parameter_types,
    traits::{ConstU128, ConstU32},
};
use frame_system as system;
use sp_core::H256;
use sp_runtime::{
    traits::{BlakeTwo256, IdentityLookup},
    BuildStorage,
};

// Define the mock block type for the test runtime.
type Block = frame_system::mocking::MockBlock<Test>;

// Configure a mock runtime to test the pallet using construct_runtime macro.
// This creates a minimal runtime with just the system and the PoI pallet.
frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        PalletCbcPoi: pallet_cbc_poi,
    }
);

// Parameter types for system configuration.
parameter_types! {
    pub const BlockHashCount: u64 = 250; // Number of recent block hashes to keep.
    pub const SS58Prefix: u8 = 42;       // Default Substrate address prefix.
}

// Implement the system pallet configuration trait for the mock runtime.
impl system::Config for Test {
    type BaseCallFilter = frame_support::traits::Everything;
    type BlockWeights = ();
    type BlockLength = ();
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    type Nonce = u64;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = u64; // Use u64 for account IDs in tests.
    type Lookup = IdentityLookup<Self::AccountId>;
    type Block = Block;
    type RuntimeEvent = RuntimeEvent;
    type BlockHashCount = BlockHashCount;
    type DbWeight = ();
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = ();
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type SS58Prefix = SS58Prefix;
    type OnSetCode = ();
    type MaxConsumers = frame_support::traits::ConstU32<16>;
    type RuntimeTask = ();
    type ExtensionsWeightInfo = ();
    type SingleBlockMigrations = ();
    type MultiBlockMigrator = ();
    type PreInherents = ();
    type PostInherents = ();
    type PostTransactions = ();
}

// Implement the PoI pallet configuration trait for the mock runtime.
pub struct DummyPosInterface;
impl pallet_cbc_poi::PosInterface<u64> for DummyPosInterface {
    fn boost_score(_validator: &u64, _weight: u32) -> frame_support::dispatch::DispatchResult { Ok(()) }
    fn slash_score(_validator: &u64, _weight: u32) -> frame_support::dispatch::DispatchResult { Ok(()) }
}

impl pallet_cbc_poi::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type MinInferenceConfidence = ConstU32<50>; // Minimum confidence for inference.
    type MaxInferenceAge = ConstU32<10>;       // Max epochs an inference is valid.
    type ChallengeWindow = ConstU32<5>;        // Epochs allowed for challenge.
    type InferenceReward = ConstU128<1000>;    // Reward for correct inference.
    type ChallengeReward = ConstU128<500>;     // Reward for successful challenge.
    type DcfInterface = MockDcfInterface;
    type PosInterface = DummyPosInterface;
}

// Mock DcfInterface implementation
pub struct MockDcfInterface;
impl pallet_cbc_poi::DcfInterface<u64> for MockDcfInterface {
    fn record_inference_activity(_validator: &u64) -> frame_support::dispatch::DispatchResult {
        Ok(())
    }
}

// Helper function to build genesis storage for tests.
// Returns a TestExternalities instance for executing tests in an isolated environment.
pub fn new_test_ext() -> sp_io::TestExternalities {
    system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap()
        .into()
}