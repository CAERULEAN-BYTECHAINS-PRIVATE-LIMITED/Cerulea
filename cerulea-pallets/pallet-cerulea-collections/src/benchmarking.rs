//! The benchmark set of the collections module.
//!
//! Each benchmark registers a real collection, lands a real record and
//! then runs one call. The figures in `weights.rs` come from this set.

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use alloc::{string::ToString, vec};
use cerulea_blueprint::{
    collections::{Collection, PurposeDeclaration},
    types::{FieldDef, FieldName, RecordDef, TypeDef, TypeExpr, TypeId, Value},
};
use frame_benchmarking::v2::*;
use frame_system::RawOrigin;

const DEPLOYMENT: DeploymentId = [6u8; 32];
const CONTENT_KEY: [u8; 32] = [11u8; 32];
const ROLE_KEY: [u8; 32] = [33u8; 32];

fn parcel_type() -> TypeDef {
    TypeDef::Record(RecordDef {
        id: TypeId::new("parcel"),
        fields: vec![
            FieldDef::new(
                "survey_number",
                TypeExpr::Text,
                "The survey number of the parcel.",
            ),
            FieldDef::new("holder", TypeExpr::Text, "The name of the present holder."),
        ],
        summary: "One land parcel in the register.".to_string(),
    })
}

fn parcel_collection() -> Collection {
    let mut collection = Collection::new(
        "parcel",
        "parcel",
        "survey_number",
        "The register of land parcels.",
    );
    collection.purpose = PurposeDeclaration::new(
        "The register records who holds each land parcel.",
        &["registration"],
    );
    collection
}

fn parcel(survey: &str) -> Value {
    let mut value = Value::record(
        TypeId::new("parcel"),
        vec![
            (
                FieldName::new("survey_number"),
                Value::Text(survey.to_string()),
            ),
            (FieldName::new("holder"), Value::Text("Anitha".to_string())),
        ],
    );
    value.canonicalise();
    value
}

#[benchmarks(where T: pallet_cerulea_schema::Config)]
mod benchmarks {
    use super::*;

    fn collection_name() -> BoundedName {
        name_of("parcel").expect("the name fits")
    }

    fn role_name() -> BoundedName {
        name_of("registrar").expect("the name fits")
    }

    fn install<T: Config + pallet_cerulea_schema::Config>(steward: &T::AccountId) {
        pallet_cerulea_schema::Pallet::<T>::register_type(
            RawOrigin::Signed(steward.clone()).into(),
            DEPLOYMENT,
            BoundedVec::try_from(parcel_type().encode()).expect("the type fits"),
        )
        .expect("the type registers");
        pallet_cerulea_schema::Pallet::<T>::register_collection(
            RawOrigin::Signed(steward.clone()).into(),
            DEPLOYMENT,
            BoundedVec::try_from(parcel_collection().encode()).expect("the collection fits"),
        )
        .expect("the collection registers");
        T::Records::insert(
            &DEPLOYMENT,
            &CollectionId::new("parcel"),
            steward,
            &parcel("12/3"),
        )
        .expect("the record lands");
    }

    fn holders<T: Config>() -> BoundedVec<BoundedName, T::MaxKeyHolders> {
        BoundedVec::try_from(vec![role_name()]).expect("the holder list fits")
    }

    fn seal<T: Config + pallet_cerulea_schema::Config>(steward: &T::AccountId) {
        Pallet::<T>::set_class(
            RawOrigin::Signed(steward.clone()).into(),
            DEPLOYMENT,
            collection_name(),
            PrivacyLevel::Encrypted,
            holders::<T>(),
            None,
            Some(CONTENT_KEY),
        )
        .expect("the class change runs");
    }

    #[benchmark]
    fn set_class() {
        let steward: T::AccountId = whitelisted_caller();
        install::<T>(&steward);

        #[extrinsic_call]
        set_class(
            RawOrigin::Signed(steward),
            DEPLOYMENT,
            collection_name(),
            PrivacyLevel::Encrypted,
            holders::<T>(),
            None,
            Some(CONTENT_KEY),
        );

        assert_eq!(
            Pallet::<T>::level(&DEPLOYMENT, &CollectionId::new("parcel")),
            PrivacyLevel::Encrypted
        );
    }

    #[benchmark]
    fn advance_class_change() {
        let steward: T::AccountId = whitelisted_caller();
        install::<T>(&steward);
        for survey in ["44/9", "07/1", "88/2", "55/5"] {
            T::Records::insert(
                &DEPLOYMENT,
                &CollectionId::new("parcel"),
                &steward,
                &parcel(survey),
            )
            .expect("the record lands");
        }
        Pallet::<T>::set_class(
            RawOrigin::Signed(steward.clone()).into(),
            DEPLOYMENT,
            collection_name(),
            PrivacyLevel::Encrypted,
            holders::<T>(),
            None,
            Some(CONTENT_KEY),
        )
        .expect("the class change starts");

        #[extrinsic_call]
        advance_class_change(
            RawOrigin::Signed(steward),
            DEPLOYMENT,
            collection_name(),
            None,
            Some(CONTENT_KEY),
        );

        assert!(
            Changes::<T>::contains_key(CollectionKey::new(DEPLOYMENT, collection_name()))
                || Pallet::<T>::level(&DEPLOYMENT, &CollectionId::new("parcel"))
                    == PrivacyLevel::Encrypted
        );
    }

    #[benchmark]
    fn distribute_key() {
        let steward: T::AccountId = whitelisted_caller();
        install::<T>(&steward);
        seal::<T>(&steward);

        #[extrinsic_call]
        distribute_key(
            RawOrigin::Signed(steward),
            DEPLOYMENT,
            collection_name(),
            role_name(),
            ROLE_KEY,
            CONTENT_KEY,
        );

        assert!(
            Pallet::<T>::wrapped_key(&DEPLOYMENT, &CollectionId::new("parcel"), &role_name())
                .is_some()
        );
    }

    #[benchmark]
    fn revoke_access() {
        let steward: T::AccountId = whitelisted_caller();
        install::<T>(&steward);
        seal::<T>(&steward);
        Pallet::<T>::distribute_key(
            RawOrigin::Signed(steward.clone()).into(),
            DEPLOYMENT,
            collection_name(),
            role_name(),
            ROLE_KEY,
            CONTENT_KEY,
        )
        .expect("the key reaches the role");

        #[extrinsic_call]
        revoke_access(
            RawOrigin::Signed(steward),
            DEPLOYMENT,
            collection_name(),
            role_name(),
        );

        assert!(
            Pallet::<T>::wrapped_key(&DEPLOYMENT, &CollectionId::new("parcel"), &role_name())
                .is_none()
        );
    }

    #[benchmark]
    fn verify_commitment() {
        let steward: T::AccountId = whitelisted_caller();
        install::<T>(&steward);
        let payload = parcel("12/3").canonical_bytes();
        let key = T::Records::keys(&DEPLOYMENT, &CollectionId::new("parcel"), 1)
            .first()
            .cloned()
            .expect("the store holds a key");
        Pallet::<T>::set_class(
            RawOrigin::Signed(steward.clone()).into(),
            DEPLOYMENT,
            collection_name(),
            PrivacyLevel::HashOnly,
            BoundedVec::default(),
            None,
            None,
        )
        .expect("the class change runs");

        #[extrinsic_call]
        verify_commitment(
            RawOrigin::Signed(steward),
            DEPLOYMENT,
            collection_name(),
            key.clone(),
            BoundedVec::try_from(payload).expect("the payload fits"),
        );

        assert!(
            Commitments::<T>::get(CollectionKey::new(DEPLOYMENT, collection_name()), &key)
                .is_some()
        );
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
