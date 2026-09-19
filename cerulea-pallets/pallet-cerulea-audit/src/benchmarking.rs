//! The benchmark set of the audit module.
//!
//! Each benchmark registers a real event definition and appends a real
//! entry. The figures in `weights.rs` come from this set.

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use alloc::vec;
use frame_benchmarking::v2::*;
use frame_system::RawOrigin;

const DEPLOYMENT: DeploymentId = [7u8; 32];

fn event_name() -> BoundedName {
    name_of("parcel_registered").expect("the name fits")
}

fn summary() -> BoundedVec<u8, ConstU32<MAX_SUMMARY_BYTES>> {
    BoundedVec::try_from(b"The register recorded one land parcel.".to_vec())
        .expect("the summary fits")
}

fn reason() -> Sentence {
    BoundedVec::try_from(b"The registrar recorded the parcel.".to_vec()).expect("the reason fits")
}

fn payload<T: Config>() -> BoundedVec<u8, T::MaxPayloadBytes> {
    BoundedVec::try_from(vec![1u8, 2, 3, 4]).expect("the payload fits")
}

#[benchmarks]
mod benchmarks {
    use super::*;

    fn define<T: Config>(steward: &T::AccountId) {
        Pallet::<T>::define_event(
            RawOrigin::Signed(steward.clone()).into(),
            DEPLOYMENT,
            event_name(),
            WorkClass::Routine,
            summary(),
        )
        .expect("the definition lands");
    }

    #[benchmark]
    fn define_event() {
        let steward: T::AccountId = whitelisted_caller();

        #[extrinsic_call]
        define_event(
            RawOrigin::Signed(steward),
            DEPLOYMENT,
            event_name(),
            WorkClass::Routine,
            summary(),
        );

        assert!(Definitions::<T>::contains_key(DEPLOYMENT, event_name()));
    }

    #[benchmark]
    fn emit() {
        let steward: T::AccountId = whitelisted_caller();
        define::<T>(&steward);

        #[extrinsic_call]
        emit(
            RawOrigin::Signed(steward),
            DEPLOYMENT,
            event_name(),
            reason(),
            payload::<T>(),
        );

        assert_eq!(Pallet::<T>::length(&DEPLOYMENT), 1);
    }

    #[benchmark]
    fn correct() {
        let steward: T::AccountId = whitelisted_caller();
        define::<T>(&steward);
        Pallet::<T>::emit(
            RawOrigin::Signed(steward.clone()).into(),
            DEPLOYMENT,
            event_name(),
            reason(),
            payload::<T>(),
        )
        .expect("the entry lands");

        #[extrinsic_call]
        correct(
            RawOrigin::Signed(steward),
            DEPLOYMENT,
            0,
            reason(),
            payload::<T>(),
        );

        assert_eq!(Pallet::<T>::length(&DEPLOYMENT), 2);
    }

    #[benchmark]
    fn subscribe() {
        let reader: T::AccountId = whitelisted_caller();

        #[extrinsic_call]
        subscribe(
            RawOrigin::Signed(reader.clone()),
            DEPLOYMENT,
            BoundedVec::default(),
            BoundedVec::default(),
        );

        assert!(Subscriptions::<T>::contains_key(DEPLOYMENT, &reader));
    }

    #[benchmark]
    fn unsubscribe() {
        let reader: T::AccountId = whitelisted_caller();
        Pallet::<T>::subscribe(
            RawOrigin::Signed(reader.clone()).into(),
            DEPLOYMENT,
            BoundedVec::default(),
            BoundedVec::default(),
        )
        .expect("the subscription lands");

        #[extrinsic_call]
        unsubscribe(RawOrigin::Signed(reader.clone()), DEPLOYMENT);

        assert!(!Subscriptions::<T>::contains_key(DEPLOYMENT, &reader));
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
