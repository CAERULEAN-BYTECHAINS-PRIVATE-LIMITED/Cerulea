use crate as pallet_pramaan_consistency;
use frame_system as system;
use sp_core::H256;
use sp_runtime::{
	traits::{BlakeTwo256, IdentityLookup},
	BuildStorage,
};

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
	pub enum Test {
		System: frame_system,
		PalletPramaanConsistency: pallet_pramaan_consistency,
	}
);

frame_support::parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const SS58Prefix: u8 = 42;
	pub const MaxHistory: u32 = 8;
	// 1000 bps = 10 percentage points, a reasonable default tolerance clearly exceeded
	// by the PoC document's 86%-vs-30% (5600 bps) Sabarmati Systems example.
	pub const ToleranceBps: u16 = 1000;
}

impl system::Config for Test {
	type BaseCallFilter = frame_support::traits::Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type RuntimeOrigin = RuntimeOrigin;
	type RuntimeCall = RuntimeCall;
	type Nonce = u64;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = u64;
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

// This pallet has no dependency on RuleLookup, DebarmentCheck, or any other external
// trait — cross-tender consistency reads only its own Declarations map — so no external
// trait mocks are needed here.
impl pallet_pramaan_consistency::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
	type MaxHistory = MaxHistory;
	type ToleranceBps = ToleranceBps;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut ext: sp_io::TestExternalities =
		system::GenesisConfig::<Test>::default().build_storage().unwrap().into();

	// Advance to block 1 so that events are recorded (frame_system does not deposit
	// events at block 0), matching pallet-todo's mock.rs convention — needed here
	// because, unlike pallet-pramaan-rule-registry's tests, this pallet's required
	// tests assert on emitted events (DeclarationRecorded / InconsistencyFlagged).
	ext.execute_with(|| {
		System::set_block_number(1);
	});

	ext
}
