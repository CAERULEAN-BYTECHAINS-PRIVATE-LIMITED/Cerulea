#[cfg(test)]
mod tests {
	use crate::mock::*;
	use crate::{weighted_local_content_bps, BidItem, BidLineItem, ClassResultLike, Error};
	use frame_support::{assert_noop, assert_ok, BoundedVec};
	use pramaan_primitives::{CalculationMethod, Divisibility, HsnThreshold, PathwayId, Rule};

	fn sample_ministry() -> pramaan_primitives::MinistryId {
		BoundedVec::try_from(b"MEITY".to_vec()).unwrap()
	}

	fn sample_tender() -> pramaan_primitives::TenderId {
		BoundedVec::try_from(b"TENDER-001".to_vec()).unwrap()
	}

	fn make_rule(divisibility: Divisibility, para_3a_applicable: bool, preference_margin_bps: u16) -> Rule<u64, u64> {
		let hsn_code: pramaan_primitives::HsnCode = BoundedVec::try_from(b"8471".to_vec()).unwrap();
		Rule {
			hsn_thresholds: BoundedVec::try_from(vec![HsnThreshold {
				hsn_code,
				class_one_bps: 5_000,
				class_two_bps: 2_000,
			}])
			.unwrap(),
			para_3a_applicable,
			pli_linked: false,
			calculation_method: CalculationMethod::Standard,
			preference_margin_bps,
			certification_threshold: 100_000_000,
			exemption_floor: 500_000,
			divisibility,
			effective_from: 0,
		}
	}

	fn bid(vendor: u64, class: ClassResultLike, price: u64, is_mse: bool, is_gte: bool) -> BidItem<u64, u64> {
		BidItem { vendor, class, price, is_mse, is_gte }
	}

	// --- 1. Single-item bid: one Class-I bid, only bid in the tender, wins 100%. ---
	#[test]
	fn single_bid_class_one_wins_full_award() {
		new_test_ext().execute_with(|| {
			MockRuleSource::set_rule(make_rule(Divisibility::Divisible, false, 2_000));
			let tender = sample_tender();
			let ministry = sample_ministry();
			let vendor = 10u64;
			let bids = BoundedVec::try_from(vec![bid(vendor, ClassResultLike::ClassOne, 1_000_000, false, false)]).unwrap();

			assert_ok!(PalletPramaanPreference::calculate_preference(
				RuntimeOrigin::signed(1),
				tender.clone(),
				ministry,
				bids,
				1_000_000,
				false,
			));

			let outcome = PalletPramaanPreference::preference_results(vendor, &tender).unwrap();
			assert!(outcome.qualifies);
			assert_eq!(outcome.awarded_percent_bps, 10_000);
			assert_eq!(outcome.matched_price, None);
		});
	}

	// --- 2. Multi-item weighted average: sum(local_value_added) / sum(sale_price), in
	// basis points, verified against a hand-computed expectation from the same formula.
	#[test]
	fn multi_item_weighted_average_matches_hand_computation() {
		let items = vec![
			BidLineItem { sale_price: 100_000u64, local_value_added: 60_000u64 },
			BidLineItem { sale_price: 200_000u64, local_value_added: 90_000u64 },
			BidLineItem { sale_price: 50_000u64, local_value_added: 10_000u64 },
		];
		// sum(local_value_added) = 160_000, sum(sale_price) = 350_000
		// bps = 160_000 * 10_000 / 350_000 = 4571 (integer division)
		let expected_bps: u16 = ((160_000u64 * 10_000u64) / 350_000u64) as u16;
		assert_eq!(expected_bps, 4_571);
		assert_eq!(weighted_local_content_bps(&items), Some(expected_bps));
	}

	#[test]
	fn weighted_average_of_empty_items_is_none() {
		let items: Vec<BidLineItem<u64>> = Vec::new();
		assert_eq!(weighted_local_content_bps(&items), None);
	}

	// --- 3. Price at exactly the 20% band boundary: L1 x 1.20 qualifies (inclusive). ---
	#[test]
	fn price_at_exactly_the_band_boundary_qualifies() {
		new_test_ext().execute_with(|| {
			MockRuleSource::set_rule(make_rule(Divisibility::Divisible, false, 2_000)); // 20% margin
			let tender = sample_tender();
			let ministry = sample_ministry();
			let l1_vendor = 10u64;
			let candidate_vendor = 20u64;
			let l1_price = 1_000_000u64;
			let candidate_price = 1_200_000u64; // exactly L1 * 1.20

			// L1 is Class-II (not Class-I, not Non-local) so it isn't itself excluded by
			// P6/P7 and doesn't trivially collapse the case before the band check runs.
			let bids = BoundedVec::try_from(vec![
				bid(l1_vendor, ClassResultLike::ClassTwo, l1_price, false, false),
				bid(candidate_vendor, ClassResultLike::ClassOne, candidate_price, false, false),
			])
			.unwrap();

			assert_ok!(PalletPramaanPreference::calculate_preference(
				RuntimeOrigin::signed(1),
				tender.clone(),
				ministry,
				bids,
				l1_price,
				false,
			));

			let outcome = PalletPramaanPreference::preference_results(candidate_vendor, &tender).unwrap();
			assert!(outcome.qualifies, "a Class-I bid priced at exactly L1 x 1.20 must qualify (inclusive boundary)");
		});
	}

	// --- 4. P8 divisible award: 50% to L1, 50% to the lowest Class-I within band. ---
	#[test]
	fn p8_divisible_award_splits_fifty_fifty() {
		new_test_ext().execute_with(|| {
			MockRuleSource::set_rule(make_rule(Divisibility::Divisible, false, 2_000));
			let tender = sample_tender();
			let ministry = sample_ministry();
			let l1_vendor = 10u64;
			let winner_vendor = 20u64;
			let l1_price = 1_000_000u64;
			let winner_price = 1_100_000u64; // within the 20% band

			let bids = BoundedVec::try_from(vec![
				bid(l1_vendor, ClassResultLike::ClassTwo, l1_price, false, false),
				bid(winner_vendor, ClassResultLike::ClassOne, winner_price, false, false),
			])
			.unwrap();

			assert_ok!(PalletPramaanPreference::calculate_preference(
				RuntimeOrigin::signed(1),
				tender.clone(),
				ministry,
				bids,
				l1_price,
				false,
			));

			let l1_outcome = PalletPramaanPreference::preference_results(l1_vendor, &tender).unwrap();
			let winner_outcome = PalletPramaanPreference::preference_results(winner_vendor, &tender).unwrap();

			assert_eq!(l1_outcome.awarded_percent_bps, 5_000);
			assert_eq!(l1_outcome.matched_price, None);
			assert_eq!(l1_outcome.decision_path, PathwayId::P8);

			assert_eq!(winner_outcome.awarded_percent_bps, 5_000);
			assert_eq!(winner_outcome.matched_price, Some(l1_price));
			assert_eq!(winner_outcome.decision_path, PathwayId::P8);
		});
	}

	// --- 5. P9 non-divisible award: full contract to the lowest Class-I within band. ---
	#[test]
	fn p9_non_divisible_award_goes_fully_to_matched_class_one() {
		new_test_ext().execute_with(|| {
			MockRuleSource::set_rule(make_rule(Divisibility::NonDivisible, false, 2_000));
			let tender = sample_tender();
			let ministry = sample_ministry();
			let l1_vendor = 10u64;
			let winner_vendor = 20u64;
			let l1_price = 1_000_000u64;
			let winner_price = 1_150_000u64; // within the 20% band

			let bids = BoundedVec::try_from(vec![
				bid(l1_vendor, ClassResultLike::ClassTwo, l1_price, false, false),
				bid(winner_vendor, ClassResultLike::ClassOne, winner_price, false, false),
			])
			.unwrap();

			assert_ok!(PalletPramaanPreference::calculate_preference(
				RuntimeOrigin::signed(1),
				tender.clone(),
				ministry,
				bids,
				l1_price,
				false,
			));

			let l1_outcome = PalletPramaanPreference::preference_results(l1_vendor, &tender).unwrap();
			let winner_outcome = PalletPramaanPreference::preference_results(winner_vendor, &tender).unwrap();

			assert_eq!(l1_outcome.awarded_percent_bps, 0);
			assert_eq!(l1_outcome.decision_path, PathwayId::P9);

			assert_eq!(winner_outcome.awarded_percent_bps, 10_000);
			assert_eq!(winner_outcome.matched_price, Some(l1_price));
			assert_eq!(winner_outcome.decision_path, PathwayId::P9);
		});
	}

	// --- 6. P10 MSE+MII overlay: 75% to non-MSE Class-I L1, 25% to an MSE within the
	// hardcoded 15% band -- using a rule preference_margin_bps of 10% to prove the 15%
	// figure is NOT derived from the rule's own margin (a bid at L1 x 1.15 would fail a
	// 10%-margin band check but must still qualify under the literal 1500 bps MSE band).
	#[test]
	fn p10_mse_within_fifteen_percent_band_gets_25_percent() {
		new_test_ext().execute_with(|| {
			MockRuleSource::set_rule(make_rule(Divisibility::Divisible, false, 1_000)); // 10% general margin
			let tender = sample_tender();
			let ministry = sample_ministry();
			let l1_vendor = 10u64;
			let mse_vendor = 20u64;
			let l1_price = 1_000_000u64;
			let mse_price = 1_150_000u64; // exactly L1 * 1.15: inside the 15% MSE band,
			                              // outside the rule's own 10% margin.

			let bids = BoundedVec::try_from(vec![
				bid(l1_vendor, ClassResultLike::ClassOne, l1_price, false, false),
				bid(mse_vendor, ClassResultLike::ClassTwo, mse_price, true, false),
			])
			.unwrap();

			assert_ok!(PalletPramaanPreference::calculate_preference(
				RuntimeOrigin::signed(1),
				tender.clone(),
				ministry,
				bids,
				l1_price,
				false,
			));

			let l1_outcome = PalletPramaanPreference::preference_results(l1_vendor, &tender).unwrap();
			let mse_outcome = PalletPramaanPreference::preference_results(mse_vendor, &tender).unwrap();

			assert_eq!(l1_outcome.awarded_percent_bps, 7_500);
			assert_eq!(l1_outcome.decision_path, PathwayId::P10);

			assert_eq!(mse_outcome.awarded_percent_bps, 2_500);
			assert_eq!(mse_outcome.matched_price, Some(l1_price));
			assert_eq!(mse_outcome.decision_path, PathwayId::P10);
		});
	}

	// --- 7a. P5: a non-GTE domestic tender priced above Rs 200 crore is rejected.
	// Balance convention is paise (matching pallet-pramaan-certification's fixed
	// convention): Rs 200 crore = 200_000_000_000 paise.
	#[test]
	fn p5_rejects_domestic_tender_above_200_crore() {
		new_test_ext().execute_with(|| {
			MockRuleSource::set_rule(make_rule(Divisibility::Divisible, false, 2_000));
			let tender = sample_tender();
			let ministry = sample_ministry();
			let bids = BoundedVec::try_from(vec![bid(10u64, ClassResultLike::ClassOne, 1_000, false, false)]).unwrap();
			let over_limit_value = 200_000_000_001u64; // > Rs 200 crore in paise

			assert_noop!(
				PalletPramaanPreference::calculate_preference(
					RuntimeOrigin::signed(1),
					tender,
					ministry,
					bids,
					over_limit_value,
					false,
				),
				Error::<Test>::TenderValueExceedsDomesticLimit
			);
		});
	}

	// --- 7b. P6: a Non-local bid on a non-GTE tender is not eligible to be ranked. ---
	#[test]
	fn p6_rejects_non_local_bid_without_gte_approval() {
		new_test_ext().execute_with(|| {
			MockRuleSource::set_rule(make_rule(Divisibility::Divisible, false, 2_000));
			let tender = sample_tender();
			let ministry = sample_ministry();
			let bids = BoundedVec::try_from(vec![bid(10u64, ClassResultLike::NonLocal, 1_000, false, false)]).unwrap();

			assert_noop!(
				PalletPramaanPreference::calculate_preference(
					RuntimeOrigin::signed(1),
					tender,
					ministry,
					bids,
					1_000,
					false,
				),
				Error::<Test>::NonLocalNotPermittedOnDomesticTender
			);
		});
	}

	// --- 7c. P7: Para 3A active, no Class-I bid present -> nothing eligible to rank. ---
	#[test]
	fn p7_rejects_when_para_3a_applies_and_no_class_one_bid_present() {
		new_test_ext().execute_with(|| {
			MockRuleSource::set_rule(make_rule(Divisibility::Divisible, true, 2_000)); // Para 3A on
			let tender = sample_tender();
			let ministry = sample_ministry();
			let bids = BoundedVec::try_from(vec![bid(10u64, ClassResultLike::ClassTwo, 1_000, false, false)]).unwrap();

			assert_noop!(
				PalletPramaanPreference::calculate_preference(
					RuntimeOrigin::signed(1),
					tender,
					ministry,
					bids,
					1_000,
					false,
				),
				Error::<Test>::Para3ARequiresClassOne
			);
		});
	}

	#[test]
	fn empty_bid_list_is_rejected() {
		new_test_ext().execute_with(|| {
			MockRuleSource::set_rule(make_rule(Divisibility::Divisible, false, 2_000));
			let tender = sample_tender();
			let ministry = sample_ministry();
			let bids: BoundedVec<BidItem<u64, u64>, MaxBids> = BoundedVec::try_from(vec![]).unwrap();

			assert_noop!(
				PalletPramaanPreference::calculate_preference(
					RuntimeOrigin::signed(1),
					tender,
					ministry,
					bids,
					1_000,
					false,
				),
				Error::<Test>::EmptyBidList
			);
		});
	}

	#[test]
	fn missing_rule_for_ministry_is_rejected() {
		new_test_ext().execute_with(|| {
			// No MockRuleSource::set_rule call: RuleSource returns None for every ministry.
			let tender = sample_tender();
			let ministry = sample_ministry();
			let bids = BoundedVec::try_from(vec![bid(10u64, ClassResultLike::ClassOne, 1_000, false, false)]).unwrap();

			assert_noop!(
				PalletPramaanPreference::calculate_preference(
					RuntimeOrigin::signed(1),
					tender,
					ministry,
					bids,
					1_000,
					false,
				),
				Error::<Test>::NoRuleForMinistry
			);
		});
	}
}
