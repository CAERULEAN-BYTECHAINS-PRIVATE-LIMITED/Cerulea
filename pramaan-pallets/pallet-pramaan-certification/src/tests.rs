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
