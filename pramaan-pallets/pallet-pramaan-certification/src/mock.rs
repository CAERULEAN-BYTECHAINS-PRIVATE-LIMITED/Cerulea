use crate as pallet_pramaan_certification;
use crate::AuditorRoleSource;
use frame_support::BoundedVec;
use frame_system as system;
use pramaan_primitives::{CalculationMethod, Divisibility, HsnThreshold, MinistryId, Rule, RuleLookup};
use sp_core::H256;
use sp_runtime::{
	traits::{BlakeTwo256, IdentityLookup},
	BuildStorage,
};

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
	pub enum Test {
		System: frame_system,
		PalletPramaanCertification: pallet_pramaan_certification,
	}
);

frame_support::parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const SS58Prefix: u8 = 42;
	pub const MaxCertsPerAuditor: u32 = 5;
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

/// Ministry id with a configured rule in `MockRuleSource`, standing in for MEITY as
/// used throughout the sibling pallet-pramaan-rule-registry's own tests.
pub fn meity_ministry() -> MinistryId {
	BoundedVec::try_from(b"MEITY".to_vec()).unwrap()
}

/// Test-only stand-in for `pallet-pramaan-rule-registry`'s `RuleLookup`: a fixed rule
/// with `certification_threshold` set to the DPIIT default of ₹10 crore, in the
/// paise-denominated Balance-unit convention documented at the top of `lib.rs`
/// (100_000_000_00 = 10,00,00,000 rupees × 100 paise). Only `meity_ministry()` has a
/// configured rule; every other ministry id resolves to `None`, for the
/// `NoRuleForMinistry` test.
pub struct MockRuleSource;
impl RuleLookup<u64, u64> for MockRuleSource {
	fn rule(ministry: &MinistryId) -> Option<Rule<u64, u64>> {
		if ministry.clone().into_inner() == b"MEITY".to_vec() {
			let hsn_code: pramaan_primitives::HsnCode = BoundedVec::try_from(b"8471".to_vec()).unwrap();
			Some(Rule {
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
				// ₹10 crore in paise, the DPIIT default (PoC document Table 10).
				certification_threshold: 100_000_000_00,
				exemption_floor: 500_000,
				divisibility: Divisibility::Divisible,
				effective_from: 0,
			})
		} else {
			None
		}
	}
}

/// Test-only stand-in for "holds the CvcOrAuditReviewer role": only account `7` is a
/// registered auditor, representing "the one registered auditor" in these tests. Every
/// other account, including self-certifying vendors, is not.
pub struct MockAuditorSource;
impl AuditorRoleSource<u64> for MockAuditorSource {
	fn is_registered_auditor(who: &u64) -> bool {
		*who == 7
	}
}

impl pallet_pramaan_certification::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
	type Balance = u64;
	type RuleSource = MockRuleSource;
	type AuditorSource = MockAuditorSource;
	type MaxCertsPerAuditor = MaxCertsPerAuditor;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let storage = system::GenesisConfig::<Test>::default().build_storage().unwrap();
	storage.into()
}
