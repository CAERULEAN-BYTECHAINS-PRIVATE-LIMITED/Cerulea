use crate as pallet_cbc_dvf;
use frame_support::{
    parameter_types,
    traits::ConstU32,
};
use sp_core::{sr25519, Pair, H256};
use sp_runtime::{
    traits::{BlakeTwo256, IdentifyAccount, IdentityLookup, Verify},
    BuildStorage, MultiSignature, Perbill,
};

type Block = frame_system::mocking::MockBlock<Test>;
type Signature = MultiSignature;
type AccountPublic = <Signature as Verify>::Signer;
pub type AccountId = <AccountPublic as IdentifyAccount>::AccountId;

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
    pub enum Test
    {
        System: frame_system,
        Dvf: pallet_cbc_dvf,
    }
);

parameter_types! {
    pub const BlockHashCount: u64 = 250;
}

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
    type AccountId = AccountId;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Block = Block;
    type RuntimeEvent = RuntimeEvent;
    type BlockHashCount = BlockHashCount;
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = ();
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type SS58Prefix = ();
    type OnSetCode = ();
    type MaxConsumers = ConstU32<16>;
    type SingleBlockMigrations = ();
    type MultiBlockMigrator = ();
    type PreInherents = ();
    type PostInherents = ();
    type PostTransactions = ();
    type RuntimeTask = ();
    type ExtensionsWeightInfo = ();
}

parameter_types! {
    pub const StakeWeightFactor: u128 = 1;
    pub const ScoreWeightFactor: u128 = 100;
    pub const ScoreBoostCap: u128 = 100_000;
    pub const FinalityThreshold: Perbill = Perbill::from_percent(67);
    pub const FinalityCheckpointInterval: u32 = 10;
    pub const VoteRetentionRounds: u32 = 20;
}

impl pallet_cbc_dvf::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Signer = AccountPublic;
    type Signature = Signature;
    type StakeWeightFactor = StakeWeightFactor;
    type ScoreWeightFactor = ScoreWeightFactor;
    type ScoreBoostCap = ScoreBoostCap;
    type FinalityThreshold = FinalityThreshold;
    type FinalityCheckpointInterval = FinalityCheckpointInterval;
    type VoteRetentionRounds = VoteRetentionRounds;
    type MaxValidators = ConstU32<100>;
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
    // Create test validator weights synchronized with production pattern
    // Using realistic stakes matching the bug condition exploration test
    let test_validator_weights = vec![
        (account_key("Alice"), 10_000_000, 80),   // Alice: 10M stake, 80 score
        (account_key("Bob"), 8_000_000, 80),      // Bob: 8M stake, 80 score
        (account_key("Charlie"), 6_000_000, 80),  // Charlie: 6M stake, 80 score
    ];

    let mut t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    // Initialize DVF genesis configuration
    crate::GenesisConfig::<Test> {
        initial_validator_weights: test_validator_weights,
    }
    .assimilate_storage(&mut t)
    .unwrap();

    t.into()
}

pub fn account_key(s: &str) -> AccountId {
    let pair = sr25519::Pair::from_string(&format!("//{}", s), None).unwrap();
    AccountPublic::from(pair.public()).into_account()
}
