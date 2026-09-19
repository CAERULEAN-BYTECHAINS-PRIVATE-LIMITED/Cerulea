//! The benchmark set of the state module.
//!
//! Each benchmark registers a real type and a real collection, then runs
//! one write through the full write path. The figures in `weights.rs` come
//! from this set.

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use alloc::{string::ToString, vec};
use cerulea_blueprint::{
    collections::PurposeDeclaration,
    types::{FieldDef, FieldName, RecordDef, TypeDef, TypeId},
};
use frame_benchmarking::v2::*;
use frame_system::RawOrigin;

const DEPLOYMENT: DeploymentId = [7u8; 32];

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

fn parcel(survey: &str, holder: &str) -> Value {
    let mut value = Value::record(
        TypeId::new("parcel"),
        vec![
            (
                FieldName::new("survey_number"),
                Value::Text(survey.to_string()),
            ),
            (FieldName::new("holder"), Value::Text(holder.to_string())),
        ],
    );
    value.canonicalise();
    value
}

fn name() -> BoundedVec<u8, ConstU32<128>> {
    BoundedVec::try_from(b"parcel".to_vec()).expect("the name fits")
}

fn body<T: Config>(record: &Value) -> BoundedVec<u8, T::MaxRecordSize> {
    BoundedVec::try_from(record.canonical_bytes()).expect("the record sits inside the bound")
}

fn key<T: Config>(record: &Value) -> RecordKey {
    let bytes = key_bytes_of(&parcel_collection(), record).expect("the record carries its key");
    BoundedVec::try_from(bytes).expect("the key sits inside the bound")
}

#[benchmarks(where T: pallet_cerulea_schema::Config)]
mod benchmarks {
    use super::*;

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
    }

    #[benchmark]
    fn insert() {
        let steward: T::AccountId = whitelisted_caller();
        install::<T>(&steward);
        let record = parcel("12/3", "Anitha");

        #[extrinsic_call]
        insert(
            RawOrigin::Signed(steward),
            DEPLOYMENT,
            name(),
            body::<T>(&record),
        );

        assert_eq!(
            Pallet::<T>::record_count(&DEPLOYMENT, &CollectionId::new("parcel")),
            1
        );
    }

    #[benchmark]
    fn update() {
        let steward: T::AccountId = whitelisted_caller();
        install::<T>(&steward);
        let record = parcel("12/3", "Anitha");
        Pallet::<T>::insert(
            RawOrigin::Signed(steward.clone()).into(),
            DEPLOYMENT,
            name(),
            body::<T>(&record),
        )
        .expect("the record lands");
        let changed = parcel("12/3", "Benny");

        #[extrinsic_call]
        update(
            RawOrigin::Signed(steward),
            DEPLOYMENT,
            name(),
            key::<T>(&record),
            body::<T>(&changed),
        );

        let meta = Pallet::<T>::record_meta(
            &DEPLOYMENT,
            &CollectionId::new("parcel"),
            &key::<T>(&record),
        )
        .expect("the store holds the metadata");
        assert_eq!(meta.version, 2);
    }

    #[benchmark]
    fn delete() {
        let steward: T::AccountId = whitelisted_caller();
        install::<T>(&steward);
        let record = parcel("12/3", "Anitha");
        Pallet::<T>::insert(
            RawOrigin::Signed(steward.clone()).into(),
            DEPLOYMENT,
            name(),
            body::<T>(&record),
        )
        .expect("the record lands");

        #[extrinsic_call]
        delete(
            RawOrigin::Signed(steward),
            DEPLOYMENT,
            name(),
            key::<T>(&record),
        );

        assert_eq!(
            Pallet::<T>::record_count(&DEPLOYMENT, &CollectionId::new("parcel")),
            0
        );
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
