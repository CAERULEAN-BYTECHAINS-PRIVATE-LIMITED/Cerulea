#[cfg(test)]
mod tests {
	use crate::mock::*;
	use crate::{ClassResult, ComponentDeclaration, Error};
	use frame_support::{assert_noop, assert_ok, BoundedVec};

	fn ministry(name: &[u8]) -> pramaan_primitives::MinistryId {
		BoundedVec::try_from(name.to_vec()).unwrap()
	}

	fn tender(name: &[u8]) -> pramaan_primitives::TenderId {
		BoundedVec::try_from(name.to_vec()).unwrap()
	}

	/// Tech spec unit-test table: "Exactly 50% local content" against the DPIIT default
	/// rule (class_one_bps = 5000) -> Class = ClassOne. Inclusive lower bound, PoC
	/// document Section 7.1.
	#[test]
	fn exactly_50_percent_local_content_is_class_one() {
		new_test_ext().execute_with(|| {
			let m = ministry(b"MEITY");
			let t = tender(b"TENDER-1");

			assert_ok!(PalletPramaanClassification::classify(
				RuntimeOrigin::signed(1),
				1,
				t.clone(),
				m,
				5_000,
				false
			));

			assert_eq!(PalletPramaanClassification::classifications((1, t)), Some(ClassResult::ClassOne));
		});
	}

	/// Tech spec unit-test table: "Exactly 20% local content" against the DPIIT default
	/// rule (class_two_bps = 2000) -> Class = ClassTwo, not NonLocal. Inclusive lower
	/// bound, PoC document Section 7.1.
	#[test]
	fn exactly_20_percent_local_content_is_class_two_not_non_local() {
		new_test_ext().execute_with(|| {
			let m = ministry(b"MEITY");
			let t = tender(b"TENDER-2");

			assert_ok!(PalletPramaanClassification::classify(
				RuntimeOrigin::signed(1),
				1,
				t.clone(),
				m,
				2_000,
				false
			));

			let class = PalletPramaanClassification::classifications((1, t)).unwrap();
			assert_eq!(class, ClassResult::ClassTwo);
			assert_ne!(class, ClassResult::NonLocal);
		});
	}

	/// Tech spec unit-test table: "19.99% local content" -> Class = NonLocal.
	#[test]
	fn below_20_percent_local_content_is_non_local() {
		new_test_ext().execute_with(|| {
			let m = ministry(b"MEITY");
			let t = tender(b"TENDER-3");

			assert_ok!(PalletPramaanClassification::classify(
				RuntimeOrigin::signed(1),
				1,
				t.clone(),
				m,
				1_999,
				false
			));

			assert_eq!(PalletPramaanClassification::classifications((1, t)), Some(ClassResult::NonLocal));
		});
	}

	/// Tech spec unit-test table: "Debarred vendor" -> call fails with `VendorDebarred`.
	#[test]
	fn debarred_vendor_is_rejected() {
		new_test_ext().execute_with(|| {
			let m = ministry(b"MEITY");
			let t = tender(b"TENDER-4");

			assert_noop!(
				PalletPramaanClassification::classify(
					RuntimeOrigin::signed(1),
					DEBARRED_VENDOR,
					t,
					m,
					9_000,
					false
				),
				Error::<Test>::VendorDebarred
			);
		});
	}

	/// PathwayId::P2: weighted-average computation, mirroring the MeitY HSN 8471
	/// 5-component pattern (PCB/Motherboard 40%, Power Supply 15%, Enclosure 10%,
	/// Assembly 20%, Software 15%).
	/// (6000*4000 + 3000*1500 + 8000*1000 + 9000*2000 + 2000*1500) / 10000 = 5750 bps,
	/// which is >= the 5000 bps class_one_bps threshold, so ClassOne.
	#[test]
	fn component_level_weighted_average_is_computed_correctly() {
		new_test_ext().execute_with(|| {
			let m = ministry(b"COMPONENT");
			let t = tender(b"TENDER-5");

			let components = BoundedVec::try_from(vec![
				ComponentDeclaration {
					name: BoundedVec::try_from(b"PCB_MOTHERBOARD".to_vec()).unwrap(),
					declared_bps: 6_000,
					weight_bps: 4_000,
				},
				ComponentDeclaration {
					name: BoundedVec::try_from(b"POWER_SUPPLY".to_vec()).unwrap(),
					declared_bps: 3_000,
					weight_bps: 1_500,
				},
				ComponentDeclaration {
					name: BoundedVec::try_from(b"ENCLOSURE".to_vec()).unwrap(),
					declared_bps: 8_000,
					weight_bps: 1_000,
				},
				ComponentDeclaration {
					name: BoundedVec::try_from(b"ASSEMBLY".to_vec()).unwrap(),
					declared_bps: 9_000,
					weight_bps: 2_000,
				},
				ComponentDeclaration {
					name: BoundedVec::try_from(b"SOFTWARE".to_vec()).unwrap(),
					declared_bps: 2_000,
					weight_bps: 1_500,
				},
			])
			.unwrap();

			assert_eq!(PalletPramaanClassification::weighted_average_bps(&components), 5_750);

			assert_ok!(PalletPramaanClassification::classify_component_level(
				RuntimeOrigin::signed(1),
				1,
				t.clone(),
				m,
				components
			));

			assert_eq!(PalletPramaanClassification::classifications((1, t)), Some(ClassResult::ClassOne));
		});
	}

	#[test]
	fn component_level_rejects_weights_not_summing_to_100_percent() {
		new_test_ext().execute_with(|| {
			let m = ministry(b"COMPONENT");
			let t = tender(b"TENDER-6");

			let components = BoundedVec::try_from(vec![
				ComponentDeclaration {
					name: BoundedVec::try_from(b"PCB_MOTHERBOARD".to_vec()).unwrap(),
					declared_bps: 6_000,
					weight_bps: 4_000,
				},
				ComponentDeclaration {
					name: BoundedVec::try_from(b"POWER_SUPPLY".to_vec()).unwrap(),
					declared_bps: 3_000,
					weight_bps: 1_500,
				},
			])
			.unwrap();

			assert_noop!(
				PalletPramaanClassification::classify_component_level(RuntimeOrigin::signed(1), 1, t, m, components),
				Error::<Test>::WeightsDoNotSumToOneHundredPercent
			);
		});
	}

	/// PathwayId::P3: `CalculationMethod::Custom` must write `ManualReviewRequired`
	/// rather than compute an automatic class.
	#[test]
	fn custom_method_routes_to_manual_review_not_an_automatic_class() {
		new_test_ext().execute_with(|| {
			let m = ministry(b"CUSTOM");
			let t = tender(b"TENDER-7");

			assert_ok!(PalletPramaanClassification::classify(
				RuntimeOrigin::signed(1),
				1,
				t.clone(),
				m,
				9_000,
				false
			));

			assert_eq!(
				PalletPramaanClassification::classifications((1, t)),
				Some(ClassResult::ManualReviewRequired)
			);
		});
	}

	/// PathwayId::P4: `is_pli_manufacturer = true` with `rule.pli_linked = true` forces
	/// ClassTwo even when the declared percentage would otherwise be ClassOne (9000 bps)
	/// or NonLocal (100 bps).
	#[test]
	fn pli_deeming_forces_class_two_regardless_of_declared_percentage() {
		new_test_ext().execute_with(|| {
			let m = ministry(b"PLI");

			let t_would_be_class_one = tender(b"TENDER-8A");
			assert_ok!(PalletPramaanClassification::classify(
				RuntimeOrigin::signed(1),
				1,
				t_would_be_class_one.clone(),
				m.clone(),
				9_000,
				true
			));
			assert_eq!(
				PalletPramaanClassification::classifications((1, t_would_be_class_one)),
				Some(ClassResult::ClassTwo)
			);

			let t_would_be_non_local = tender(b"TENDER-8B");
			assert_ok!(PalletPramaanClassification::classify(
				RuntimeOrigin::signed(1),
				1,
				t_would_be_non_local.clone(),
				m,
				100,
				true
			));
			assert_eq!(
				PalletPramaanClassification::classifications((1, t_would_be_non_local)),
				Some(ClassResult::ClassTwo)
			);
		});
	}

	#[test]
	fn no_rule_for_ministry_is_rejected() {
		new_test_ext().execute_with(|| {
			let m = ministry(b"NORULE");
			let t = tender(b"TENDER-9");

			assert_noop!(
				PalletPramaanClassification::classify(RuntimeOrigin::signed(1), 1, t, m, 5_000, false),
				Error::<Test>::NoRuleForMinistry
			);
		});
	}
}
