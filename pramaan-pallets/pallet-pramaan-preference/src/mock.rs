use crate as pallet_pramaan_preference;
use frame_system as system;
use pramaan_primitives::{MinistryId, Rule, RuleLookup};
use sp_core::H256;
use sp_runtime::{
	traits::{BlakeTwo256, IdentityLookup},
	BuildStorage,
};
use std::cell::RefCell;

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
	pub enum Test {
		System: frame_system,
		PalletPramaanPreference: pallet_pramaan_preference,
	}
);

frame_support::parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const SS58Prefix: u8 = 42;
	pub const MaxBids: u32 = 32;
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

thread_local! {
	/// The single rule every test configures before calling `calculate_preference`.
	/// `None` means "no rule for any ministry", exercising `NoRuleForMinistry`.
	static MOCK_RULE: RefCell<Option<Rule<u64, u64>>> = RefCell::new(None);
}

/// Test-only stand-in for `Config::RuleSource`: returns whatever rule the current test
/// installed via `MockRuleSource::set_rule`, for every `ministry` id (the mock does not
/// distinguish ministries -- tests that need per-ministry behaviour are out of scope for
/// this pallet's own unit tests, since ministry-keyed lookup is rule-registry's concern).
pub struct MockRuleSource;

impl MockRuleSource {
	pub fn set_rule(rule: Rule<u64, u64>) {
		MOCK_RULE.with(|r| *r.borrow_mut() = Some(rule));
	}

	pub fn clear() {
		MOCK_RULE.with(|r| *r.borrow_mut() = None);
	}
}

impl RuleLookup<u64, u64> for MockRuleSource {
	fn rule(_ministry: &MinistryId) -> Option<Rule<u64, u64>> {
		MOCK_RULE.with(|r| r.borrow().clone())
	}
}

impl pallet_pramaan_preference::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
	type Balance = u64;
	type RuleSource = MockRuleSource;
	type MaxBids = MaxBids;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let storage = system::GenesisConfig::<Test>::default().build_storage().unwrap();
	let mut ext: sp_io::TestExternalities = storage.into();
	ext.execute_with(|| MockRuleSource::clear());
	ext
}
