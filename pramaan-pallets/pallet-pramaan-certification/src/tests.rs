#[cfg(test)]
mod tests {
	use crate::mock::*;
	use crate::{Certificates, AuditorLedger, Error};
	use frame_support::{assert_noop, assert_ok, BoundedVec};

	/// ₹10 crore in paise (`MockRuleSource`'s configured `certification_threshold`).
	const THRESHOLD: u64 = 100_000_000_00;

	fn tender(bytes: &[u8]) -> pramaan_primitives::TenderId {
		BoundedVec::try_from(bytes.to_vec()).unwrap()
	}

	fn cert_id(bytes: &[u8]) -> pramaan_primitives::CertificateId {
		BoundedVec::try_from(bytes.to_vec()).unwrap()
	}

	/// The empanelment register is real state, changed by transaction, and it gates
	/// certification. Before this existed the role check accepted every account, so a
	/// vendor could name themselves as the auditor on a statutory certificate.
	#[test]
	fn empanelment_is_maintained_on_chain_and_gates_certification() {
		new_test_ext().execute_with(|| {
			let firm: frame_support::BoundedVec<u8, pramaan_primitives::IdBound> =
				frame_support::BoundedVec::try_from(b"Narmada & Co.".to_vec()).unwrap();
			let newcomer = 42u64;

			// Not empanelled, so a statutory certificate naming them is refused.
			assert!(!crate::Auditors::<Test>::contains_key(&newcomer));
			assert_noop!(
				PalletPramaanCertification::certify(
					RuntimeOrigin::signed(1),
					cert_id(b"CERT-REG-1"),
					meity_ministry(),
					2,
					tender(b"TENDER-REG-1"),
					THRESHOLD,
					Some(newcomer),
				),
				Error::<Test>::AuditorRoleMissing
			);

			// An unprivileged account cannot empanel anyone.
			assert_noop!(
				PalletPramaanCertification::empanel_auditor(
					RuntimeOrigin::signed(9),
					newcomer,
					firm.clone()
				),
				Error::<Test>::NotAuthorised
			);

			// DPIIT/Root empanels them -- a transaction, not a redeploy.
			assert_ok!(PalletPramaanCertification::empanel_auditor(
				RuntimeOrigin::root(),
				newcomer,
				firm.clone()
			));
			assert_eq!(crate::Auditors::<Test>::get(&newcomer), Some(firm.clone()));

			// The same certificate now succeeds.
			assert_ok!(PalletPramaanCertification::certify(
				RuntimeOrigin::signed(1),
				cert_id(b"CERT-REG-2"),
				meity_ministry(),
				2,
				tender(b"TENDER-REG-2"),
				THRESHOLD,
				Some(newcomer),
			));

			// Removing them closes the door again, but does NOT erase what they signed.
			assert_ok!(PalletPramaanCertification::remove_auditor(
				RuntimeOrigin::root(),
				newcomer
			));
			assert!(!crate::Auditors::<Test>::contains_key(&newcomer));
			assert_eq!(
				PalletPramaanCertification::certificates_for_auditor(&newcomer),
				vec![cert_id(b"CERT-REG-2")],
				"an accountability ledger that forgets on de-empanelment is not one"
			);
			assert_noop!(
				PalletPramaanCertification::certify(
					RuntimeOrigin::signed(1),
					cert_id(b"CERT-REG-3"),
					meity_ministry(),
					2,
					tender(b"TENDER-REG-3"),
					THRESHOLD,
					Some(newcomer),
				),
				Error::<Test>::AuditorRoleMissing
			);
		});
	}

	#[test]
	fn below_threshold_no_auditor_succeeds() {
		new_test_ext().execute_with(|| {
			let id = cert_id(b"CERT-1");
			assert_ok!(PalletPramaanCertification::certify(
				RuntimeOrigin::signed(1),
				id.clone(),
				meity_ministry(),
				2, // vendor
				tender(b"TENDER-1"),
				THRESHOLD - 1, // strictly below threshold
				None,
			));

			let certificate = Certificates::<Test>::get(&id).unwrap();
			assert_eq!(certificate.vendor, 2);
			assert_eq!(certificate.value, THRESHOLD - 1);
			assert_eq!(certificate.auditor, None);
		});
	}

	#[test]
	fn at_or_above_threshold_no_auditor_fails() {
		new_test_ext().execute_with(|| {
			let id = cert_id(b"CERT-2");
			assert_noop!(
				PalletPramaanCertification::certify(
					RuntimeOrigin::signed(1),
					id,
					meity_ministry(),
					2,
					tender(b"TENDER-2"),
					THRESHOLD, // exactly at threshold: "at or above" per the tech spec
					None,
				),
				Error::<Test>::AuditorRequired
			);
		});
	}

	/// "Auditor accountability lookup: One auditor with three prior certificates, one
	/// now found false -> AuditorLedger query surfaces all three for review." This
	/// pallet's own responsibility, proven here, is that the ledger correctly
	/// accumulates and returns all three certificate ids for one auditor; the "found
	/// false" cross-reference into pallet-pramaan-debarment happens at the frontend/API
	/// layer per PoC document Section 4.7.
	#[test]
	fn auditor_ledger_accumulates_all_certificates_for_one_auditor() {
		new_test_ext().execute_with(|| {
			let auditor = 7; // the one registered auditor, per MockAuditorSource

			let ids =
				[cert_id(b"CERT-A"), cert_id(b"CERT-B"), cert_id(b"CERT-C")];

			for (i, id) in ids.iter().enumerate() {
				assert_ok!(PalletPramaanCertification::certify(
					RuntimeOrigin::signed(1),
					id.clone(),
					meity_ministry(),
					10 + i as u64, // distinct vendor per certificate
					tender(b"TENDER-SHARED"),
					THRESHOLD - 1,
					Some(auditor),
				));
			}

			let ledger = AuditorLedger::<Test>::get(&auditor);
			assert_eq!(ledger.into_inner(), ids.to_vec());
			assert_eq!(PalletPramaanCertification::certificates_for_auditor(&auditor), ids.to_vec());
		});
	}

	#[test]
	fn auditor_without_role_is_rejected_above_threshold() {
		new_test_ext().execute_with(|| {
			let id = cert_id(b"CERT-3");
			assert_noop!(
				PalletPramaanCertification::certify(
					RuntimeOrigin::signed(1),
					id,
					meity_ministry(),
					2,
					tender(b"TENDER-3"),
					THRESHOLD, // at threshold, auditor mandatory
					Some(2), // account 2 does not hold CvcOrAuditReviewer per MockAuditorSource
				),
				Error::<Test>::AuditorRoleMissing
			);
		});
	}

	#[test]
	fn duplicate_certificate_id_is_rejected() {
		new_test_ext().execute_with(|| {
			let id = cert_id(b"CERT-DUP");
			assert_ok!(PalletPramaanCertification::certify(
				RuntimeOrigin::signed(1),
				id.clone(),
				meity_ministry(),
				2,
				tender(b"TENDER-4"),
				THRESHOLD - 1,
				None,
			));

			assert_noop!(
				PalletPramaanCertification::certify(
					RuntimeOrigin::signed(1),
					id,
					meity_ministry(),
					3,
					tender(b"TENDER-5"),
					THRESHOLD - 1,
					None,
				),
				Error::<Test>::CertificateIdAlreadyUsed
			);
		});
	}

	#[test]
	fn unconfigured_ministry_fails() {
		new_test_ext().execute_with(|| {
			let unconfigured: pramaan_primitives::MinistryId =
				BoundedVec::try_from(b"NO-SUCH-MINISTRY".to_vec()).unwrap();

			assert_noop!(
				PalletPramaanCertification::certify(
					RuntimeOrigin::signed(1),
					cert_id(b"CERT-4"),
					unconfigured,
					2,
					tender(b"TENDER-6"),
					THRESHOLD - 1,
					None,
				),
				Error::<Test>::NoRuleForMinistry
			);
		});
	}
}
