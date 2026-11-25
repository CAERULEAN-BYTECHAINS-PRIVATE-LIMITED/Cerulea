//! The benchmark set of the schema module.
//!
//! Each benchmark builds a real deployment, runs one call and checks the
//! registry afterwards. The figures in `weights.rs` come from this set.

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use alloc::{string::ToString, vec};
use cerulea_blueprint::{
    collections::{Collection, PurposeDeclaration},
    types::{FieldDef, FieldName, RecordDef, TypeDef, TypeExpr, TypeId, Value},
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

fn add_area_plan() -> MigrationPlan {
    MigrationPlan::new("parcel", 1, "The register gains the parcel area.")
        .with(migrate::MigrationStep::AddField {
            field: FieldName::new("area"),
            ty: TypeExpr::Integer,
            default: Value::Integer(0),
            summary: "The area of the parcel in square metres.".to_string(),
        })
        .within(0)
}

fn payload<T: Config>(bytes: alloc::vec::Vec<u8>) -> BoundedVec<u8, T::MaxDefinitionBytes> {
    BoundedVec::try_from(bytes).expect("the payload sits inside the bound")
}

fn plan_payload<T: Config>(bytes: alloc::vec::Vec<u8>) -> BoundedVec<u8, T::MaxMigrationBytes> {
    BoundedVec::try_from(bytes).expect("the plan sits inside the bound")
}

fn install<T: Config>(steward: &T::AccountId) {
    Pallet::<T>::register_type(
        RawOrigin::Signed(steward.clone()).into(),
        DEPLOYMENT,
        payload::<T>(parcel_type().encode()),
    )
    .expect("the type registers");
    Pallet::<T>::register_collection(
        RawOrigin::Signed(steward.clone()).into(),
        DEPLOYMENT,
        payload::<T>(parcel_collection().encode()),
    )
    .expect("the collection registers");
}

#[benchmarks]
mod benchmarks {
    use super::*;

    #[benchmark]
    fn register_type() {
        let steward: T::AccountId = whitelisted_caller();

        #[extrinsic_call]
        register_type(
            RawOrigin::Signed(steward),
            DEPLOYMENT,
            payload::<T>(parcel_type().encode()),
        );

        assert!(TypeHashes::<T>::get(DEPLOYMENT, name_of("parcel").unwrap()).is_some());
    }

    #[benchmark]
    fn register_collection() {
        let steward: T::AccountId = whitelisted_caller();
        Pallet::<T>::register_type(
            RawOrigin::Signed(steward.clone()).into(),
            DEPLOYMENT,
            payload::<T>(parcel_type().encode()),
        )
        .expect("the type registers");

        #[extrinsic_call]
        register_collection(
            RawOrigin::Signed(steward),
            DEPLOYMENT,
            payload::<T>(parcel_collection().encode()),
        );

        assert!(CollectionDefs::<T>::get(DEPLOYMENT, name_of("parcel").unwrap()).is_some());
    }

    #[benchmark]
    fn propose_migration() {
        let steward: T::AccountId = whitelisted_caller();
        install::<T>(&steward);

        #[extrinsic_call]
        propose_migration(
            RawOrigin::Signed(steward),
            DEPLOYMENT,
            plan_payload::<T>(add_area_plan().encode()),
        );

        assert!(Migrations::<T>::get((DEPLOYMENT, name_of("parcel").unwrap(), 1u32)).is_some());
    }

    #[benchmark]
    fn apply_migration() {
        let steward: T::AccountId = whitelisted_caller();
        install::<T>(&steward);
        Pallet::<T>::propose_migration(
            RawOrigin::Signed(steward.clone()).into(),
            DEPLOYMENT,
            plan_payload::<T>(add_area_plan().encode()),
        )
        .expect("the plan stores");

        #[extrinsic_call]
        apply_migration(
            RawOrigin::Signed(steward),
            DEPLOYMENT,
            name_of("parcel").unwrap(),
            1u32,
        );

        let entry = CollectionDefs::<T>::get(DEPLOYMENT, name_of("parcel").unwrap())
            .expect("the registry holds the entry");
        assert_eq!(entry.version, 2);
    }

    #[benchmark]
    fn revert_migration() {
        let steward: T::AccountId = whitelisted_caller();
        install::<T>(&steward);
        Pallet::<T>::propose_migration(
            RawOrigin::Signed(steward.clone()).into(),
            DEPLOYMENT,
            plan_payload::<T>(add_area_plan().encode()),
        )
        .expect("the plan stores");
        Pallet::<T>::apply_migration(
            RawOrigin::Signed(steward.clone()).into(),
            DEPLOYMENT,
            name_of("parcel").unwrap(),
            1u32,
        )
        .expect("the plan applies");

        #[extrinsic_call]
        revert_migration(
            RawOrigin::Signed(steward),
            DEPLOYMENT,
            name_of("parcel").unwrap(),
            1u32,
        );

        let entry = CollectionDefs::<T>::get(DEPLOYMENT, name_of("parcel").unwrap())
            .expect("the registry holds the entry");
        assert_eq!(entry.version, 1);
    }

    #[benchmark]
    fn retire_collection() {
        let steward: T::AccountId = whitelisted_caller();
        install::<T>(&steward);

        #[extrinsic_call]
        retire_collection(
            RawOrigin::Signed(steward),
            DEPLOYMENT,
            name_of("parcel").unwrap(),
        );

        let entry = CollectionDefs::<T>::get(DEPLOYMENT, name_of("parcel").unwrap())
            .expect("the registry holds the entry");
        assert!(entry.retired);
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
