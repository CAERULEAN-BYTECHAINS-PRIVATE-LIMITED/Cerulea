#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::v2::*;
use frame_support::BoundedVec;
use frame_system::RawOrigin;

#[benchmarks]
mod benchmarks {
	use super::*;

	/// Worst case for `certify()`: `auditor` is `Some`, so both the
	/// `AuditorSource::is_registered_auditor` role check and the `AuditorLedger`
	/// `try_push` (the heavier of the extrinsic's two possible storage writes) execute,
	/// on top of the `Certificates` write every call performs. Using `auditor = Some`
	/// exercises this path regardless of where `value` falls relative to
	/// `Rules[ministry].certification_threshold`, since a voluntarily-supplied auditor
	/// is role-checked and ledgered the same way a mandatory one is (see `certify`'s
	/// doc comment). This still depends on the benchmarking runtime's
	/// `Config::RuleSource` resolving `ministry` to a configured rule (else the call
	/// short-circuits on `NoRuleForMinistry`) and `Config::AuditorSource` recognising
	/// `auditor` as a registered CvcOrAuditReviewer (else it short-circuits on
	/// `AuditorRoleMissing`) — wiring left to the runtime's benchmark configuration,
	/// the same externally-supplied-`RuleLookup` dependency
	/// pallet-pramaan-classification/preference's benchmarks would also carry.
	#[benchmark]
	fn certify() {
		let certificate_id: CertificateId = BoundedVec::try_from(b"CERT-0001".to_vec()).unwrap();
		let ministry: MinistryId = BoundedVec::try_from(b"MEITY".to_vec()).unwrap();
		let tender: TenderId = BoundedVec::try_from(b"TENDER-0001".to_vec()).unwrap();
		let vendor: T::AccountId = whitelisted_caller();
		let auditor: T::AccountId = account("auditor", 0, 0);
		let value = T::Balance::default();

		#[extrinsic_call]
		certify(
			RawOrigin::Signed(vendor.clone()),
			certificate_id.clone(),
			ministry,
			vendor,
			tender,
			value,
			Some(auditor),
		);

		assert!(Certificates::<T>::contains_key(&certificate_id));
	}
}
