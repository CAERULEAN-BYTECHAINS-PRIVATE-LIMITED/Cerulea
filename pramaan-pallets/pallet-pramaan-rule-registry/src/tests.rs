#[cfg(test)]
mod tests {
	use crate::mock::*;
	use crate::Error;
	use frame_support::{assert_noop, assert_ok, BoundedVec};
	use pramaan_primitives::{CalculationMethod, Divisibility, HsnThreshold, Rule};

	fn sample_ministry() -> pramaan_primitives::MinistryId {
		BoundedVec::try_from(b"MEITY".to_vec()).unwrap()
	}

	fn sample_rule() -> Rule<u64, u64> {
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
			calculation_method: CalculationMethod::ComponentLevel,
			preference_margin_bps: 2_000,
			certification_threshold: 100_000_000,
			exemption_floor: 500_000,
			divisibility: Divisibility::Divisible,
			effective_from: 0,
		}
	}

	#[test]
	fn set_rule_then_get_matches_input_exactly() {
		new_test_ext().execute_with(|| {
			let ministry = sample_ministry();
			let rule = sample_rule();

			assert_ok!(PalletPramaanRuleRegistry::set_rule(
				RuntimeOrigin::root(),
				ministry.clone(),
				rule.clone()
			));

			assert_eq!(PalletPramaanRuleRegistry::rules(&ministry), Some(rule));
			assert_eq!(PalletPramaanRuleRegistry::rule_version(&ministry), 1);
		});
	}

	#[test]
	fn set_rule_from_non_admin_fails() {
		new_test_ext().execute_with(|| {
			let ministry = sample_ministry();
			let rule = sample_rule();

			// Account 2 is signed but is not the designated DPIIT account (1) and is
			// not Root, matching the "Vendor account calling set_rule" test case.
			assert_noop!(
				PalletPramaanRuleRegistry::set_rule(RuntimeOrigin::signed(2), ministry, rule),
				Error::<Test>::NotAuthorised
			);
		});
	}

	#[test]
	fn default_fallback_when_ministry_has_no_explicit_rule() {
		new_test_ext().execute_with(|| {
			let default_rule = sample_rule();
			assert_ok!(PalletPramaanRuleRegistry::set_default_rule(RuntimeOrigin::root(), default_rule.clone()));

			let unconfigured_ministry: pramaan_primitives::MinistryId =
				BoundedVec::try_from(b"NEW-MINISTRY".to_vec()).unwrap();

			assert_eq!(PalletPramaanRuleRegistry::rules(&unconfigured_ministry), None);
			assert_eq!(
				PalletPramaanRuleRegistry::get_effective_rule(&unconfigured_ministry),
				Some(default_rule)
			);
		});
	}

	#[test]
	fn set_rule_rejects_margin_above_100_percent() {
		new_test_ext().execute_with(|| {
			let ministry = sample_ministry();
			let mut rule = sample_rule();
			rule.preference_margin_bps = 10_001;

			assert_noop!(
				PalletPramaanRuleRegistry::set_rule(RuntimeOrigin::root(), ministry, rule),
				Error::<Test>::InvalidRule
			);
		});
	}
}
