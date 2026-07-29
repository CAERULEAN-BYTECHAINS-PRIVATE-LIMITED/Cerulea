#[cfg(test)]
mod tests {
	use crate::mock::*;
	use crate::{Error, Event};
	use frame_support::{assert_noop, assert_ok, BoundedVec};

	/// PoC document Table 6 names the vendor in the canonical worked example
	/// "Sabarmati Systems". This test suite uses a plain test `AccountId` in its place
	/// (as every other pallet in this build does for vendor/auditor/ministry accounts),
	/// but keeps the name in the constant and comments for traceability back to the
	/// submission document.
	const SABARMATI_SYSTEMS: u64 = 42;
	const PROCURING_ENTITY: u64 = 1;

	fn product(id: &str) -> pramaan_primitives::ProductId {
		BoundedVec::try_from(id.as_bytes().to_vec()).unwrap()
	}

	fn tender(id: &str) -> pramaan_primitives::TenderId {
		BoundedVec::try_from(id.as_bytes().to_vec()).unwrap()
	}

	fn flagged_event_count() -> usize {
		System::events()
			.iter()
			.filter(|record| {
				matches!(record.event, RuntimeEvent::PalletPramaanConsistency(Event::InconsistencyFlagged { .. }))
			})
			.count()
	}

	/// PoC document Section 4.7, verbatim: "The Declaration Consistency Engine
	/// cross-references every declaration against the vendor's history, identifying
	/// the vendor who claims 86 percent local content on one tender and 30 percent for
	/// the same product on another." PoC document Table 6, verbatim: "The vendor view
	/// for Sabarmati Systems surfaces its earlier 86 percent declaration against the
	/// later 30 percent one for the same product."
	#[test]
	fn sabarmati_systems_86_vs_30_flags_inconsistency() {
		new_test_ext().execute_with(|| {
			let product = product("PRODUCT-STEEL-PIPE");
			let tender_a = tender("TENDER-A");
			let tender_b = tender("TENDER-B");

			// Earlier declaration: 86% (8600 bps) local content on tender A.
			assert_ok!(PalletPramaanConsistency::declare(
				RuntimeOrigin::signed(PROCURING_ENTITY),
				SABARMATI_SYSTEMS,
				product.clone(),
				tender_a.clone(),
				8_600,
			));
			// No prior history yet, so only DeclarationRecorded fires.
			System::assert_last_event(
				Event::DeclarationRecorded {
					vendor: SABARMATI_SYSTEMS,
					product: product.clone(),
					tender: tender_a.clone(),
					local_content_bps: 8_600,
					block_number: 1,
				}
				.into(),
			);
			assert_eq!(flagged_event_count(), 0);

			// Later declaration: 30% (3000 bps) for the SAME product on a DIFFERENT
			// tender. 8600 vs 3000 is a 5600 bps gap, clearly exceeding the mock's
			// 1000 bps tolerance.
			assert_ok!(PalletPramaanConsistency::declare(
				RuntimeOrigin::signed(PROCURING_ENTITY),
				SABARMATI_SYSTEMS,
				product.clone(),
				tender_b.clone(),
				3_000,
			));

			// InconsistencyFlagged carries prior = tender A's 8600, new = tender B's
			// 3000.
			assert!(System::events().iter().any(|record| record.event
				== RuntimeEvent::PalletPramaanConsistency(Event::InconsistencyFlagged {
					vendor: SABARMATI_SYSTEMS,
					product: product.clone(),
					prior_tender: tender_a.clone(),
					prior_value: 8_600,
					new_tender: tender_b.clone(),
					new_value: 3_000,
					block_number: 1,
				})));

			// DeclarationRecorded still fires too, as the last event (recorded after
			// the flag(s) are raised).
			System::assert_last_event(
				Event::DeclarationRecorded {
					vendor: SABARMATI_SYSTEMS,
					product: product.clone(),
					tender: tender_b.clone(),
					local_content_bps: 3_000,
					block_number: 1,
				}
				.into(),
			);
			assert_eq!(flagged_event_count(), 1);

			// Both declarations remain in the vendor's history for the Table 6
			// "vendor view" lookup.
			let history = PalletPramaanConsistency::history_for(&SABARMATI_SYSTEMS, &product);
			assert_eq!(history.len(), 2);
			assert_eq!(history[0].local_content_bps, 8_600);
			assert_eq!(history[1].local_content_bps, 3_000);
		});
	}

	/// Same vendor, same product, values within tolerance across two tenders ->
	/// no flag fires. 8600 then 8700 is a 100 bps (1 percentage point) difference,
	/// clearly within the mock's 1000 bps tolerance.
	#[test]
	fn consistent_declarations_within_tolerance_do_not_flag() {
		new_test_ext().execute_with(|| {
			let product = product("PRODUCT-CONSISTENT");
			let tender_a = tender("TENDER-A");
			let tender_b = tender("TENDER-B");

			assert_ok!(PalletPramaanConsistency::declare(
				RuntimeOrigin::signed(PROCURING_ENTITY),
				SABARMATI_SYSTEMS,
				product.clone(),
				tender_a,
				8_600,
			));
			assert_ok!(PalletPramaanConsistency::declare(
				RuntimeOrigin::signed(PROCURING_ENTITY),
				SABARMATI_SYSTEMS,
				product.clone(),
				tender_b.clone(),
				8_700,
			));

			// Only DeclarationRecorded events fired across both calls; no
			// InconsistencyFlagged.
			assert_eq!(flagged_event_count(), 0);
			System::assert_last_event(
				Event::DeclarationRecorded {
					vendor: SABARMATI_SYSTEMS,
					product,
					tender: tender_b,
					local_content_bps: 8_700,
					block_number: 1,
				}
				.into(),
			);
		});
	}

	/// Filling `Declarations[(vendor, product)]` to `MaxHistory` (8 in the mock), then
	/// attempting one more `declare` call, fails with `HistoryFull` — and, per
	/// `assert_noop!`, leaves storage (including the Events map) completely
	/// unchanged, confirming the pallet rejects before emitting anything.
	#[test]
	fn history_full_rejects_once_max_history_reached() {
		new_test_ext().execute_with(|| {
			let product = product("PRODUCT-FULL");

			for i in 0..MaxHistory::get() {
				assert_ok!(PalletPramaanConsistency::declare(
					RuntimeOrigin::signed(PROCURING_ENTITY),
					SABARMATI_SYSTEMS,
					product.clone(),
					tender(&format!("TENDER-{i}")),
					8_600,
				));
			}
			assert_eq!(
				PalletPramaanConsistency::history_for(&SABARMATI_SYSTEMS, &product).len() as u32,
				MaxHistory::get()
			);

			assert_noop!(
				PalletPramaanConsistency::declare(
					RuntimeOrigin::signed(PROCURING_ENTITY),
					SABARMATI_SYSTEMS,
					product,
					tender("TENDER-OVERFLOW"),
					8_600,
				),
				Error::<Test>::HistoryFull
			);
		});
	}

	/// Three prior wildly-differing declarations for one product, followed by a
	/// fourth `declare` call: this pallet's "once per differing prior entry"
	/// interpretation (documented on the module and on the `declare` extrinsic) means
	/// all three prior entries individually exceed tolerance against the fourth
	/// value, so THREE separate `InconsistencyFlagged` events fire on that fourth
	/// call (not one aggregate flag).
	#[test]
	fn three_prior_wild_declarations_emit_three_flags_on_fourth() {
		new_test_ext().execute_with(|| {
			let product = product("PRODUCT-WILD");

			// Three prior declarations, each far from the others and far from the
			// fourth value below: 9000, 100, 5000 bps. Note these three are also far
			// from EACH OTHER, so setting them up here (the 2nd vs. the 1st, the 3rd
			// vs. the 1st and 2nd) itself fires InconsistencyFlagged events along the
			// way — that's expected and not what this test is about, so events are
			// reset afterwards to isolate what the fourth call alone produces.
			for (i, bps) in [9_000u16, 100, 5_000].into_iter().enumerate() {
				assert_ok!(PalletPramaanConsistency::declare(
					RuntimeOrigin::signed(PROCURING_ENTITY),
					SABARMATI_SYSTEMS,
					product.clone(),
					tender(&format!("TENDER-{i}")),
					bps,
				));
			}
			System::reset_events();

			// Fourth declaration: 2000 bps. abs_diff against 9000 (7000), 100 (1900),
			// and 5000 (3000) all exceed the mock's 1000 bps tolerance -> three
			// distinct InconsistencyFlagged events, one per prior entry.
			assert_ok!(PalletPramaanConsistency::declare(
				RuntimeOrigin::signed(PROCURING_ENTITY),
				SABARMATI_SYSTEMS,
				product.clone(),
				tender("TENDER-NEW"),
				2_000,
			));

			assert_eq!(flagged_event_count(), 3);
			assert_eq!(PalletPramaanConsistency::history_for(&SABARMATI_SYSTEMS, &product).len(), 4);
		});
	}
}
