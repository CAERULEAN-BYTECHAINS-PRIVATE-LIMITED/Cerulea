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
			// Balance convention across this whole build is paise (smallest currency
			// unit) — see pallet-pramaan-certification's module doc comment, which is
			// where this was made explicit after pallet-pramaan-preference and this
			// fixture were both found to have assumed plain rupee units instead.
			// Rs 10 crore = 100_000_000_00 paise; Rs 5 lakh = 500_000_00 paise.
			certification_threshold: 100_000_000_00,
			exemption_floor: 500_000_00,
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

	/// A rule notified today but commencing later must NOT judge bids taken before it
	/// commences. `effective_from` is a notified commencement date, not a label: the API
	/// tells the caller "takes effect from block N", so the registry has to honour it.
	#[test]
	fn a_rule_with_a_future_effective_from_is_not_yet_in_force() {
		new_test_ext().execute_with(|| {
			let ministry = sample_ministry();

			// A default exists, so there is something to fall back to.
			let mut default_rule = sample_rule();
			default_rule.preference_margin_bps = 2_000;
			assert_ok!(PalletPramaanRuleRegistry::set_default_rule(
				RuntimeOrigin::root(),
				default_rule.clone()
			));

			// The ministry notifies a change commencing far in the future.
			let mut future_rule = sample_rule();
			future_rule.preference_margin_bps = 500;
			future_rule.effective_from = 100_000;
			assert_ok!(PalletPramaanRuleRegistry::set_rule(
				RuntimeOrigin::root(),
				ministry.clone(),
				future_rule.clone()
			));

			// It is stored and versioned -- the notification is on the record ...
			assert_eq!(PalletPramaanRuleRegistry::rules(&ministry), Some(future_rule.clone()));
			assert_eq!(PalletPramaanRuleRegistry::rule_version(&ministry), 1);

			// ... but it is not what a bid is judged against yet.
			assert_eq!(
				PalletPramaanRuleRegistry::get_effective_rule(&ministry),
				Some(default_rule),
				"a rule commencing at block 100000 must not apply at block 1"
			);

			// Once the chain reaches the commencement block, it takes over.
			System::set_block_number(100_000);
			assert_eq!(PalletPramaanRuleRegistry::get_effective_rule(&ministry), Some(future_rule));
		});
	}

	/// The same test applied to the default set: a default scheduled for the future must
	/// not apply early either, and with nothing else in force the answer is None rather
	/// than a rule that has not commenced.
	#[test]
	fn a_default_rule_with_a_future_effective_from_is_not_yet_in_force() {
		new_test_ext().execute_with(|| {
			let unconfigured: pramaan_primitives::MinistryId =
				BoundedVec::try_from(b"NEW-MINISTRY".to_vec()).unwrap();

			let mut future_default = sample_rule();
			future_default.effective_from = 50_000;
			assert_ok!(PalletPramaanRuleRegistry::set_default_rule(
				RuntimeOrigin::root(),
				future_default.clone()
			));

			assert_eq!(PalletPramaanRuleRegistry::get_effective_rule(&unconfigured), None);

			System::set_block_number(50_000);
			assert_eq!(
				PalletPramaanRuleRegistry::get_effective_rule(&unconfigured),
				Some(future_default)
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
