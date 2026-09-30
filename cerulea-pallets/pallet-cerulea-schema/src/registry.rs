//! The interface the schema module exposes to its dependants.
//!
//! The state module, the permission module, the collections module and the
//! logic module read type and collection definitions through the traits
//! here. A dependant never reaches into schema storage directly.

use alloc::vec::Vec;
use cerulea_blueprint::{
    collections::{Collection, CollectionRegistry},
    types::{CollectionId, TypeDef, TypeId, TypeRegistry},
    Blueprint, Path,
};
use sp_runtime::DispatchError;

use crate::primitives::DeploymentId;

/// A read over the registered types and collections of one deployment.
pub trait SchemaRegistry {
    /// Every registered type definition, in name order.
    fn type_defs(deployment: &DeploymentId) -> Vec<TypeDef>;

    /// One registered type definition.
    fn type_def(deployment: &DeploymentId, id: &TypeId) -> Option<TypeDef>;

    /// Every live collection definition, in name order.
    fn collections(deployment: &DeploymentId) -> Vec<Collection>;

    /// One live collection definition.
    fn collection(deployment: &DeploymentId, id: &CollectionId) -> Option<Collection>;

    /// The live version number of one collection.
    fn collection_version(deployment: &DeploymentId, id: &CollectionId) -> Option<u32>;

    /// Report whether a collection is retired.
    ///
    /// A retired collection keeps its records readable and declines every
    /// further write.
    fn is_retired(deployment: &DeploymentId, id: &CollectionId) -> bool;

    /// Report whether the deployment holds at least one collection.
    fn is_installed(deployment: &DeploymentId) -> bool;
}

/// A write over the registry.
///
/// The logic module installs a Blueprint through this trait. Installation
/// registers every type and every collection the Blueprint declares.
pub trait SchemaRegistrar: SchemaRegistry {
    /// Register every type and every collection the Blueprint declares.
    ///
    /// A collection already present keeps its version when the definition
    /// matches. A changed definition raises the version.
    fn register_blueprint(
        deployment: &DeploymentId,
        blueprint: &Blueprint,
    ) -> Result<(), DispatchError>;
}

/// The registered definitions of one deployment, held together.
///
/// The Blueprint registries borrow their slices, so a caller keeps a
/// snapshot alive while it builds a registry over it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SchemaSnapshot {
    /// Every registered type definition.
    pub types: Vec<TypeDef>,
    /// Every live collection definition.
    pub collections: Vec<Collection>,
}

impl SchemaSnapshot {
    /// Load the whole registry of one deployment.
    pub fn load<S: SchemaRegistry>(deployment: &DeploymentId) -> Self {
        SchemaSnapshot {
            types: S::type_defs(deployment),
            collections: S::collections(deployment),
        }
    }

    /// Build a type registry over the snapshot.
    pub fn type_registry(&self) -> TypeRegistry<'_> {
        let (registry, _) = TypeRegistry::build(&self.types, &Path::root().field("types"));
        registry
    }

    /// Build a collection registry over the snapshot.
    pub fn collection_registry(&self) -> CollectionRegistry<'_> {
        let (registry, _) =
            CollectionRegistry::build(&self.collections, &Path::root().field("collections"));
        registry
    }

    /// Find one collection by name.
    pub fn collection(&self, id: &CollectionId) -> Option<&Collection> {
        self.collections.iter().find(|held| &held.id == id)
    }

    /// Report whether the snapshot holds nothing.
    pub fn is_empty(&self) -> bool {
        self.types.is_empty() && self.collections.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::{new_test_ext, RuntimeOrigin, Schema};
    use cerulea_blueprint::{
        collections::PurposeDeclaration,
        types::{FieldDef, RecordDef, TypeExpr},
    };
    use codec::Encode;
    use frame_support::{assert_ok, BoundedVec};

    const DEPLOYMENT: DeploymentId = [5u8; 32];

    fn permit_type() -> TypeDef {
        TypeDef::Record(RecordDef {
            id: TypeId::new("permit"),
            fields: alloc::vec![FieldDef::new(
                "number",
                TypeExpr::Text,
                "The permit number."
            )],
            summary: "One permit.".into(),
        })
    }

    fn permits() -> Collection {
        let mut held = Collection::new("permit", "permit", "number", "The register of permits.");
        held.purpose = PurposeDeclaration::new("The register records permits.", &["licensing"]);
        held
    }

    #[test]
    fn an_empty_deployment_loads_an_empty_snapshot() {
        new_test_ext().execute_with(|| {
            let snapshot = SchemaSnapshot::load::<Schema>(&DEPLOYMENT);
            assert!(snapshot.is_empty());
            assert!(snapshot.type_registry().is_empty());
            assert!(snapshot.collection(&CollectionId::new("permit")).is_none());
        });
    }

    #[test]
    fn a_snapshot_holds_every_registered_type_and_collection() {
        new_test_ext().execute_with(|| {
            let bytes: BoundedVec<u8, _> =
                BoundedVec::try_from(permit_type().encode()).expect("the type fits");
            assert_ok!(Schema::register_type(
                RuntimeOrigin::signed(1),
                DEPLOYMENT,
                bytes
            ));
            let bytes: BoundedVec<u8, _> =
                BoundedVec::try_from(permits().encode()).expect("the collection fits");
            assert_ok!(Schema::register_collection(
                RuntimeOrigin::signed(1),
                DEPLOYMENT,
                bytes
            ));

            let snapshot = SchemaSnapshot::load::<Schema>(&DEPLOYMENT);
            assert!(!snapshot.is_empty());
            assert_eq!(snapshot.types, alloc::vec![permit_type()]);
            assert_eq!(
                snapshot.collection(&CollectionId::new("permit")),
                Some(&permits())
            );
            assert_eq!(snapshot.type_registry().len(), 1);
            assert!(snapshot
                .type_registry()
                .record(&TypeId::new("permit"))
                .is_some());
            assert!(snapshot
                .collection_registry()
                .get(&CollectionId::new("permit"))
                .is_some());
        });
    }
}
