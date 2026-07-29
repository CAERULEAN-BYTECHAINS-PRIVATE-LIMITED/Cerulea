#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::v2::*;
use frame_support::BoundedVec;
use frame_system::RawOrigin;
use pramaan_primitives::{CalculationMethod, Divisibility, HsnThreshold};

fn sample_rule<T: Config>() -> RuleOf<T> {
	let hsn_code: pramaan_primitives::HsnCode = BoundedVec::try_from(b"8471".to_vec()).unwrap();
	Rule {
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
		certification_threshold: T::Balance::default(),
		exemption_floor: T::Balance::default(),
		divisibility: Divisibility::Divisible,
		effective_from: BlockNumberFor::<T>::default(),
	}
}

#[benchmarks]
mod benchmarks {
	use super::*;

	#[benchmark]
	fn set_rule() {
		let ministry: MinistryId = BoundedVec::try_from(b"DPIIT".to_vec()).unwrap();
		let rule = sample_rule::<T>();

		#[extrinsic_call]
		set_rule(RawOrigin::Root, ministry.clone(), rule);

		assert!(Rules::<T>::contains_key(&ministry));
	}

	#[benchmark]
	fn set_default_rule() {
		let rule = sample_rule::<T>();

		#[extrinsic_call]
		set_default_rule(RawOrigin::Root, rule);

		assert!(DefaultRule::<T>::exists());
	}
}
