//! Shared types and traits for the six pramaan-pallets/* crates.
//!
//! CBC-PRAMAAN Technical Implementation Specification, Part 4. Every field, enum
//! variant, and formula constant here is transcribed from the PoC submission document
//! (CBC-PRAMAAN_PoC_Template.pdf) rather than re-derived, per the build spec's own
//! instruction not to approximate a second time what the submission already states.

#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::traits::Get;
use frame_support::BoundedVec;
use scale_info::TypeInfo;
use sp_runtime::RuntimeDebug;
use sp_std::prelude::*;

/// Bound on the byte length of a short identifier (ministry id, tender id, product id,
/// certificate id, HSN code). These are free-text-ish identifiers rather than a fixed
/// enum of ministries, deliberately: PoC document Section 4.6 / 9.3 states that adding a
/// new nodal ministry must be a configuration task, not an engineering one, so the
/// ministry identifier space must not be closed by an enum baked into the runtime.
pub type IdBound = ConstU32Id;

pub struct ConstU32Id;
impl Get<u32> for ConstU32Id {
	fn get() -> u32 {
		64
	}
}

pub type MinistryId = BoundedVec<u8, IdBound>;
pub type TenderId = BoundedVec<u8, IdBound>;
pub type ProductId = BoundedVec<u8, IdBound>;
pub type CertificateId = BoundedVec<u8, IdBound>;
pub type HsnCode = BoundedVec<u8, IdBound>;

/// Basis points: 10_000 = 100%. Used for every percentage-shaped value in the nine rule
/// parameters (thresholds, preference margin) so the runtime never carries a float.
pub type BasisPoints = u16;
pub const BPS_DENOMINATOR: u32 = 10_000;

/// Upper bound on how many per-HSN threshold entries one ministry's rule can carry.
pub struct MaxHsn;
impl Get<u32> for MaxHsn {
	fn get() -> u32 {
		64
	}
}

/// Rule parameter 3 (Calculation method), PoC document Figure 4 / Table 10. Four
/// methods: Standard is the sale-price-minus-imported-content formula (PoC Annexure B,
/// Table 13); ComponentLevel and WeightedModule validate each component/module against
/// its own condition before aggregating (tech spec Part 4.2.1 / PathwayId::P2);
/// Custom routes to a recorded human decision where no automatable formula exists
/// (tech spec Part 4.2.1 / PathwayId::P3, e.g. software per PoC Table 2).
#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen, frame_support::__private::codec::DecodeWithMemTracking)]
pub enum CalculationMethod {
	Standard,
	ComponentLevel,
	WeightedModule,
	Custom,
}

/// Rule parameter 9 (Divisibility), PoC document Table 10: "Determined by the procuring
/// entity for each tender, within what the order permits." Drives PathwayId::P8 vs P9.
#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen, frame_support::__private::codec::DecodeWithMemTracking)]
pub enum Divisibility {
	Divisible,
	NonDivisible,
}

/// One HSN code's Class-I / Class-II boundary, per PoC document Section 7.1: "The
/// classification boundaries are inclusive at the lower end: a supplier at exactly 50
/// percent is Class-I, and a supplier at exactly 20 percent is Class-II rather than
/// Non-local." DPIIT default is class_one_bps = 5000, class_two_bps = 2000 (Table 10).
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen, frame_support::__private::codec::DecodeWithMemTracking)]
pub struct HsnThreshold {
	pub hsn_code: HsnCode,
	pub class_one_bps: BasisPoints,
	pub class_two_bps: BasisPoints,
}

/// The nine rule parameters (tech spec Part 4.1 / Table 4), one struct per ministry
/// (or the DPIIT default). Every field maps one-to-one onto PoC document Figure 4's
/// nine named parameters: Class-I/Class-II threshold together form `hsn_thresholds`
/// (thresholds are set per HSN code, PoC Section 4.1.2); the remaining seven map
/// directly. `effective_from` is the engineering field behind PoC Section 12.2.3's
/// "every change carries ... an effective date" and behind RuleVersion's audit history.
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen, frame_support::__private::codec::DecodeWithMemTracking)]
pub struct Rule<Balance, BlockNumber>
where
	Balance: Encode + Decode + Clone + PartialEq + Eq + MaxEncodedLen + TypeInfo + 'static,
	BlockNumber: Encode + Decode + Clone + PartialEq + Eq + MaxEncodedLen + TypeInfo + 'static,
{
	pub hsn_thresholds: BoundedVec<HsnThreshold, MaxHsn>,
	pub para_3a_applicable: bool,
	pub pli_linked: bool,
	pub calculation_method: CalculationMethod,
	pub preference_margin_bps: BasisPoints,
	pub certification_threshold: Balance,
	pub exemption_floor: Balance,
	pub divisibility: Divisibility,
	pub effective_from: BlockNumber,
}

/// DPIIT default rule set, PoC document Table 10, transcribed exactly: Class-I 50%,
/// Class-II 20%, Standard method, 20% preference margin, ₹10 crore certification
/// threshold, Para 3A and PLI both off by default, divisibility per-tender (encoded as
/// Divisible here since the registry's DefaultRule is a fallback, not itself a tender
/// decision — the procuring entity's actual choice is recorded per-tender elsewhere).
/// `certification_threshold`/`exemption_floor` are left to the caller to supply in the
/// runtime's Balance denomination (paise vs rupee-units differ by deployment).
pub fn dpiit_default_thresholds() -> (BasisPoints, BasisPoints) {
	(5_000, 2_000)
}
pub const DPIIT_DEFAULT_PREFERENCE_MARGIN_BPS: BasisPoints = 2_000;

/// The twelve decision pathways (tech spec Part 4.2 / PoC document Section 4.2,
/// Figure 7). Grouped exactly as tech spec Table 3 groups them. Doc comments on each
/// variant are transcribed verbatim from PoC document Sections 4.2.1 through 4.2.4 so
/// the code's inline documentation matches the submission word for word.
#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen, frame_support::__private::codec::DecodeWithMemTracking)]
pub enum PathwayId {
	/// "P1 applies the standard formula against the applicable threshold." Handled by
	/// pallet-pramaan-classification, CalculationMethod::Standard.
	P1,
	/// "P2 handles component-level and weighted-module methods, validating each
	/// component against its own condition before aggregating." Handled by
	/// pallet-pramaan-classification, CalculationMethod::ComponentLevel /
	/// WeightedModule.
	P2,
	/// "P3 covers categories where no automatable formula exists and routes them to a
	/// recorded human decision rather than computing one." Handled by
	/// pallet-pramaan-classification, CalculationMethod::Custom.
	P3,
	/// "P4 applies the PLI deeming rule, which treats a manufacturer as Class-II only
	/// where the incentive has been received and only for the period the PLI ministry
	/// notified." Handled by pallet-pramaan-classification, pli_linked = true.
	P4,
	/// "P5 enforces the sub Rupees 200 crore domestic restriction." Handled by
	/// pallet-pramaan-preference.
	P5,
	/// "P6 admits Non-local suppliers only where a global tender enquiry has been
	/// approved under GFR Rule 161(iv)." Handled by pallet-pramaan-preference.
	P6,
	/// "P7 enforces Para 3A, restricting sourcing to Class-I suppliers for items a
	/// nodal ministry has notified as having sufficient local capacity, in system
	/// integration, EPC, turnkey, and service tenders." Handled by
	/// pallet-pramaan-preference, para_3a_applicable = true.
	P7,
	/// "P8 and P9 split on divisibility ... a Class-I bid priced at or below L1
	/// multiplied by 1.20 is offered the chance to match L1's price." Divisible award.
	/// Handled by pallet-pramaan-preference / pallet-pramaan-certification.
	P8,
	/// As P8, for Divisibility::NonDivisible: "the lowest Class-I within the band is
	/// offered a price match for the full contract."
	P9,
	/// "P10 overlays the MSE preference where both preferences are concurrently
	/// active ... the Department of Expenditure's memorandum of 18.05.2023 governs how
	/// the two apply together." 75%/25% MSE-within-15%-band split.
	P10,
	/// "P11 carries the certification obligation, which the 19.07.2024 amendment
	/// places at execution rather than at bidding." Handled by
	/// pallet-pramaan-certification.
	P11,
	/// "P12 handles the consequence of a false declaration: a downgrade in class
	/// triggers a penalty of up to 10 percent of contract value ... and debarment of up
	/// to two years follows under GFR Rule 151(iii)." Handled by
	/// pallet-pramaan-debarment. Cross-tender consistency (pallet-pramaan-consistency)
	/// is explicitly not a thirteenth pathway: "It runs across all twelve, because a
	/// declaration is checked against the vendor's history regardless of which route
	/// it takes."
	P12,
}

impl PathwayId {
	/// All twelve variants, for the Part 12.2 integration-test suite to enumerate.
	pub fn all() -> [PathwayId; 12] {
		[
			PathwayId::P1,
			PathwayId::P2,
			PathwayId::P3,
			PathwayId::P4,
			PathwayId::P5,
			PathwayId::P6,
			PathwayId::P7,
			PathwayId::P8,
			PathwayId::P9,
			PathwayId::P10,
			PathwayId::P11,
			PathwayId::P12,
		]
	}
}

/// The six roles (PoC document Section 8: "each of the six roles (vendor, procuring
/// entity, nodal ministry administrator, cost or chartered accountant, CVC or audit
/// reviewer, and DPIIT) sees only what its role permits"), used by every pallet's
/// origin checks and by the frontend's auth layer (tech spec Part 9.1).
#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen, frame_support::__private::codec::DecodeWithMemTracking)]
pub enum Role {
	Vendor,
	ProcuringEntity,
	NodalMinistryAdmin,
	CostOrCharteredAccountant,
	CvcOrAuditReviewer,
	Dpiit,
}

/// Implemented by pallet-pramaan-debarment, consumed by every other pallet so
/// debarment is enforced once, centrally (tech spec Part 4.3 / Part 5.5). Reproduces
/// PoC document Table 8's "Cross-ministry debarment: Siloed and reactive -> One shared
/// ledger, enforced before bidding."
pub trait DebarmentCheck<AccountId> {
	fn is_debarred(vendor: &AccountId, ministry: &MinistryId) -> bool;
}

impl<AccountId> DebarmentCheck<AccountId> for () {
	fn is_debarred(_vendor: &AccountId, _ministry: &MinistryId) -> bool {
		false
	}
}

/// Implemented by pallet-pramaan-rule-registry, consumed by classification,
/// preference, and certification (tech spec Part 4.3 / Part 5.1). `Balance` and
/// `BlockNumber` are left generic so every consuming pallet supplies its own runtime
/// types rather than this crate picking one.
pub trait RuleLookup<Balance, BlockNumber>
where
	Balance: Encode + Decode + Clone + PartialEq + Eq + MaxEncodedLen + TypeInfo + 'static,
	BlockNumber: Encode + Decode + Clone + PartialEq + Eq + MaxEncodedLen + TypeInfo + 'static,
{
	fn rule(ministry: &MinistryId) -> Option<Rule<Balance, BlockNumber>>;
}
