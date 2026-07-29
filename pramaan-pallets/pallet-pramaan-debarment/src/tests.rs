#[cfg(test)]
mod tests {
	use crate::mock::*;
	use crate::{Error, Event, Reason};
	use frame_support::{assert_noop, assert_ok, BoundedVec};
	use pramaan_primitives::{DebarmentCheck, MinistryId};

	fn mod_ministry() -> MinistryId {
		// "Ministry of Defence" per PoC document Figure 29 / the Chambal Devices
		// example.
		BoundedVec::try_from(b"MOD".to_vec()).unwrap()
	}

	fn meity_ministry() -> MinistryId {
		BoundedVec::try_from(b"MEITY".to_vec()).unwrap()
	}

	fn sample_reason() -> Reason {
		BoundedVec::try_from(b"false declaration under GFR Rule 151(iii)".to_vec()).unwrap()
	}

	#[test]
	fn debar_appends_record_and_emits_event() {
		new_test_ext().execute_with(|| {
			let vendor = 42u64;
			let ministry = mod_ministry();

			assert_ok!(PalletPramaanDebarment::debar(
				RuntimeOrigin::signed(1),
				vendor,
				ministry.clone(),
				0,
				None,
				sample_reason()
			));

			let records = PalletPramaanDebarment::debarments(&vendor);
			assert_eq!(records.len(), 1);
			assert_eq!(records[0].ministry, ministry);

			System::assert_last_event(
				Event::Debarred { vendor, ministry, effective_from: 0, effective_to: None, block_number: 1 }.into(),
			);
		});
	}

	#[test]
	fn debar_allows_multiple_records_for_same_vendor_different_ministries() {
		new_test_ext().execute_with(|| {
			let vendor = 42u64;

			assert_ok!(PalletPramaanDebarment::debar(
				RuntimeOrigin::signed(1),
				vendor,
				mod_ministry(),
				0,
				None,
				sample_reason()
			));
			assert_ok!(PalletPramaanDebarment::debar(
				RuntimeOrigin::signed(1),
				vendor,
				meity_ministry(),
				0,
				None,
				sample_reason()
			));

			assert_eq!(PalletPramaanDebarment::debarments(&vendor).len(), 2);
		});
	}

	/// Tech spec unit-test table: "Debar then check: Debar a vendor, then call
	/// is_debarred -> Returns true for that ministry, false for others."
	///
	/// Read narrowly this could mean enforcement itself is scoped per-ministry, but that
	/// would contradict PoC document Table 8's "one shared ledger, enforced before
	/// bidding" (see `cross_ministry_debarment_blocks_on_unrelated_ministry` below, which
	/// this pallet's actual `is_debarred` implementation follows). This test instead
	/// covers the reading that survives both: debarring vendor A under ministry X makes
	/// `is_debarred(A, X)` true, while an entirely separate, never-debarred vendor/
	/// ministry pair stays false.
	#[test]
	fn debarment_check_returns_true_for_debarred_vendor() {
		new_test_ext().execute_with(|| {
			let debarred_vendor = 42u64;
			let untouched_vendor = 99u64;
			let ministry = mod_ministry();

			assert_ok!(PalletPramaanDebarment::debar(
				RuntimeOrigin::signed(1),
				debarred_vendor,
				ministry.clone(),
				0,
				None,
				sample_reason()
			));

			assert!(PalletPramaanDebarment::is_debarred(&debarred_vendor, &ministry));
			// A vendor nobody has ever debarred, checked under any ministry, is false.
			assert!(!PalletPramaanDebarment::is_debarred(&untouched_vendor, &ministry));
			assert!(!PalletPramaanDebarment::is_debarred(&untouched_vendor, &meity_ministry()));
		});
	}

	/// PoC document Table 8, verbatim: "Cross-ministry debarment: Siloed and reactive ->
	/// One shared ledger, enforced before bidding." Reproduced per PoC document Figure 29
	/// / the Chambal Devices example: "A bid for Chambal Devices, debarred by the
	/// Ministry of Defence, is returned RED and blocked on an unrelated tender."
	///
	/// This is the interpretation `is_debarred` actually implements: debar under one
	/// ministry, then check under a completely different, unrelated ministry, and
	/// confirm the vendor is still blocked.
	#[test]
	fn cross_ministry_debarment_blocks_on_unrelated_ministry() {
		new_test_ext().execute_with(|| {
			let vendor = 42u64; // stands in for "Chambal Devices"

			assert_ok!(PalletPramaanDebarment::debar(
				RuntimeOrigin::signed(1),
				vendor,
				mod_ministry(), // debarred by Ministry of Defence
				0,
				None,
				sample_reason()
			));

			// Checked under MeitY, an unrelated ministry that never debarred this
			// vendor: still blocked, because enforcement is vendor-wide once any
			// ministry's debarment is active.
			assert!(PalletPramaanDebarment::is_debarred(&vendor, &meity_ministry()));
		});
	}

	/// Tech spec: "Debarred vendor blocked pre-bid: Debarred vendor calls classify via
	/// pallet-pramaan-classification -> Fails with VendorDebarred, sourced from this
	/// pallet's check." This suite has no cross-pallet integration test infrastructure,
	/// so it cannot literally invoke pallet-pramaan-classification; instead this proves
	/// the boolean that classification's own tests (and the Part 12.2 pathway
	/// integration-test suite) will rely on via the `DebarmentCheck` trait.
	#[test]
	fn is_debarred_true_is_what_classification_prebid_check_relies_on() {
		new_test_ext().execute_with(|| {
			let vendor = 42u64;
			let ministry = mod_ministry();

			assert!(!PalletPramaanDebarment::is_debarred(&vendor, &ministry));

			assert_ok!(PalletPramaanDebarment::debar(
				RuntimeOrigin::signed(1),
				vendor,
				ministry.clone(),
				0,
				None,
				sample_reason()
			));

			// classification's pre-bid check (pallet-pramaan-classification, not
			// exercised here) rejects a bid whenever this returns true.
			assert!(PalletPramaanDebarment::is_debarred(&vendor, &ministry));
		});
	}

	#[test]
	fn lift_debarment_removes_record_and_is_debarred_returns_false() {
		new_test_ext().execute_with(|| {
			let vendor = 42u64;
			let ministry = mod_ministry();

			assert_ok!(PalletPramaanDebarment::debar(
				RuntimeOrigin::signed(1),
				vendor,
				ministry.clone(),
				0,
				None,
				sample_reason()
			));
			assert!(PalletPramaanDebarment::is_debarred(&vendor, &ministry));

			assert_ok!(PalletPramaanDebarment::lift_debarment(RuntimeOrigin::signed(1), vendor, ministry.clone()));

			assert!(!PalletPramaanDebarment::is_debarred(&vendor, &ministry));
			assert!(PalletPramaanDebarment::debarments(&vendor).is_empty());
			System::assert_last_event(Event::DebarmentLifted { vendor, ministry, block_number: 1 }.into());
		});
	}

	#[test]
	fn lift_debarment_only_removes_matching_ministry_record() {
		new_test_ext().execute_with(|| {
			let vendor = 42u64;

			assert_ok!(PalletPramaanDebarment::debar(
				RuntimeOrigin::signed(1),
				vendor,
				mod_ministry(),
				0,
				None,
				sample_reason()
			));
			assert_ok!(PalletPramaanDebarment::debar(
				RuntimeOrigin::signed(1),
				vendor,
				meity_ministry(),
				0,
				None,
				sample_reason()
			));

			assert_ok!(PalletPramaanDebarment::lift_debarment(RuntimeOrigin::signed(1), vendor, mod_ministry()));

			let records = PalletPramaanDebarment::debarments(&vendor);
			assert_eq!(records.len(), 1);
			assert_eq!(records[0].ministry, meity_ministry());
			// Still debarred overall, since the MeitY record remains active.
			assert!(PalletPramaanDebarment::is_debarred(&vendor, &mod_ministry()));
		});
	}

	#[test]
	fn lift_debarment_with_no_matching_record_fails() {
		new_test_ext().execute_with(|| {
			let vendor = 42u64;

			assert_noop!(
				PalletPramaanDebarment::lift_debarment(RuntimeOrigin::signed(1), vendor, mod_ministry()),
				Error::<Test>::NoSuchDebarment
			);
		});
	}

	#[test]
	fn debar_fails_once_max_records_reached() {
		new_test_ext().execute_with(|| {
			let vendor = 42u64;

			for i in 0..MaxRecords::get() {
				let ministry: MinistryId = BoundedVec::try_from(format!("MIN-{i}").into_bytes()).unwrap();
				assert_ok!(PalletPramaanDebarment::debar(
					RuntimeOrigin::signed(1),
					vendor,
					ministry,
					0,
					None,
					sample_reason()
				));
			}

			let one_too_many: MinistryId = BoundedVec::try_from(b"ONE-TOO-MANY".to_vec()).unwrap();
			assert_noop!(
				PalletPramaanDebarment::debar(RuntimeOrigin::signed(1), vendor, one_too_many, 0, None, sample_reason()),
				Error::<Test>::DebarmentRecordsFull
			);
		});
	}

	#[test]
	fn expired_debarment_effective_to_in_past_is_not_active() {
		new_test_ext().execute_with(|| {
			let vendor = 42u64;
			let ministry = mod_ministry();

			System::set_block_number(100);

			// effective_from = 0, effective_to = 50: expired 50 blocks ago.
			assert_ok!(PalletPramaanDebarment::debar(
				RuntimeOrigin::signed(1),
				vendor,
				ministry.clone(),
				0,
				Some(50),
				sample_reason()
			));

			assert!(!PalletPramaanDebarment::is_debarred(&vendor, &ministry));
		});
	}

	#[test]
	fn debarment_not_yet_effective_is_not_active() {
		new_test_ext().execute_with(|| {
			let vendor = 42u64;
			let ministry = mod_ministry();

			System::set_block_number(10);

			// effective_from = 100: starts in the future.
			assert_ok!(PalletPramaanDebarment::debar(
				RuntimeOrigin::signed(1),
				vendor,
				ministry.clone(),
				100,
				None,
				sample_reason()
			));

			assert!(!PalletPramaanDebarment::is_debarred(&vendor, &ministry));
		});
	}
}
