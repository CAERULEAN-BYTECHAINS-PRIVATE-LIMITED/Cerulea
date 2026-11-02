use crate as pallet_pramaan_certification;
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
				needs_reverification: false,
			})
		} else {
			None
		}
	}
}

// The role check is NOT stubbed here. The mock wires `AuditorSource` to the pallet's own
// on-chain empanelment register, exactly as the runtime does, so these tests exercise the
// real gate rather than a stand-in that could drift from it. Account `7` is empanelled in
// `new_test_ext` below and plays "the one registered auditor" the tests refer to.

impl pallet_pramaan_certification::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
	type Balance = u64;
	type RuleSource = MockRuleSource;
	type AuditorSource = PalletPramaanCertification;
	type MaxCertsPerAuditor = MaxCertsPerAuditor;
	/// Root only in tests. The runtime uses DPIIT-or-Root; what matters for these tests
	/// is that an unprivileged signer cannot alter the empanelment register.
	type AuditorAdminOrigin = frame_system::EnsureRoot<u64>;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut storage = system::GenesisConfig::<Test>::default().build_storage().unwrap();

	// Empanel account 7 at genesis -- the registered auditor these tests assume.
	pallet_pramaan_certification::GenesisConfig::<Test> {
		auditors: vec![(
			7u64,
			frame_support::BoundedVec::try_from(b"Test Auditors LLP".to_vec()).unwrap(),
		)],
	}
	.assimilate_storage(&mut storage)
	.unwrap();

	storage.into()
}
