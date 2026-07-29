use crate as pallet_pramaan_classification;
use frame_support::{traits::ConstU32, BoundedVec};
use frame_system as system;
use pramaan_primitives::{CalculationMethod, Divisibility, HsnThreshold, MinistryId, Rule};
use sp_core::H256;
use sp_runtime::{
	traits::{BlakeTwo256, IdentityLookup},
	BuildStorage,
};

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
	pub enum Test {
		System: frame_system,
		PalletPramaanClassification: pallet_pramaan_classification,
	}
);

frame_support::parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const SS58Prefix: u8 = 42;
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

/// The one AccountId that `MockDebarmentSource` treats as debarred. Every other account
/// passes, so the "debarred vendor" test can use this id and every other test can use a
/// different one.
pub const DEBARRED_VENDOR: u64 = 99;

/// Test-only stand-in for pallet-pramaan-debarment: account `DEBARRED_VENDOR` is
/// debarred everywhere, every other account passes. Mirrors what the real runtime wires
/// as the actual debarment pallet.
pub struct MockDebarmentSource;
impl pramaan_primitives::DebarmentCheck<u64> for MockDebarmentSource {
	fn is_debarred(vendor: &u64, _ministry: &MinistryId) -> bool {
		*vendor == DEBARRED_VENDOR
	}
}

/// Test-only stand-in for pallet-pramaan-rule-registry. Returns a fixed DPIIT-default-
/// shaped rule (class_one_bps = 5000, class_two_bps = 2000, `CalculationMethod::Standard`)
/// for every ministry, with the ministry name switching in the handful of variations the
/// pathway-specific unit tests need: "CUSTOM" -> `CalculationMethod::Custom`
/// (PathwayId::P3), "PLI" -> `pli_linked = true` (PathwayId::P4), "COMPONENT" ->
/// `CalculationMethod::ComponentLevel` (PathwayId::P2), "NORULE" -> `None` (no rule
/// configured for this ministry, per the `NoRuleForMinistry` error path). Every other
/// ministry name (e.g. "MEITY") gets the fixed default shape.
pub struct MockRuleSource;
impl pramaan_primitives::RuleLookup<u64, u64> for MockRuleSource {
	fn rule(ministry: &MinistryId) -> Option<Rule<u64, u64>> {
		let raw = ministry.to_vec();
		if raw == b"NORULE".to_vec() {
			return None;
		}

		let hsn_code: pramaan_primitives::HsnCode = BoundedVec::try_from(b"8471".to_vec()).unwrap();
		let mut rule = Rule {
			hsn_thresholds: BoundedVec::try_from(vec![HsnThreshold {
				hsn_code,
				class_one_bps: 5_000,
				class_two_bps: 2_000,
			}])
			.unwrap(),
			para_3a_applicable: false,
			pli_linked: false,
			calculation_method: CalculationMethod::Standard,
			preference_margin_bps: 2_000,
			certification_threshold: 100_000_000u64,
			exemption_floor: 500_000u64,
			divisibility: Divisibility::Divisible,
			effective_from: 0u64,
			needs_reverification: false,
		};

		if raw == b"CUSTOM".to_vec() {
			rule.calculation_method = CalculationMethod::Custom;
		} else if raw == b"PLI".to_vec() {
			rule.pli_linked = true;
		} else if raw == b"COMPONENT".to_vec() {
			rule.calculation_method = CalculationMethod::ComponentLevel;
		}

		Some(rule)
	}
}

impl pallet_pramaan_classification::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
	type Balance = u64;
	type RuleSource = MockRuleSource;
	type DebarmentSource = MockDebarmentSource;
	type MaxComponents = ConstU32<16>;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let storage = system::GenesisConfig::<Test>::default().build_storage().unwrap();
	storage.into()
}
