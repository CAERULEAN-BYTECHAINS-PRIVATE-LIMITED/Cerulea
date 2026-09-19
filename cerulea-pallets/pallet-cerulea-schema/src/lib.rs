//! The Cerulea schema module.
//!
//! A runtime module fixes its storage layout when it compiles. A user who
//! adds an entity at three in the afternoon needs storage that exists one
//! second later. Two modules answer that. The schema module holds the
//! registered types and collections of a deployment. The state module
//! holds the records.
//!
//! The schema module carries four registries.
//!
//! 1. Type definitions keyed by the content hash of the definition.
//! 2. Collection definitions keyed by the collection name.
//! 3. The version history of every collection.
//! 4. The migration plans between versions.
//!
//! Five operations drive the registries. Register a type. Register a
//! collection. Propose a migration. Apply a migration. Retire a
//! collection.
//!
//! A deployment carries one steward account. The first account that
//! registers anything for a deployment becomes the steward. Every later
//! change to that deployment comes from the steward.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod access;
pub mod migrate;
pub mod primitives;
pub mod registry;
pub mod weights;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub use access::{
    AccessAction, AccessDenial, AccessGrant, AccessRequest, DeniedByDefault, PermissionGate,
};
pub use pallet::*;
pub use registry::{SchemaRegistrar, SchemaRegistry, SchemaSnapshot};
pub use weights::WeightInfo;

use alloc::{vec, vec::Vec};
use cerulea_blueprint::{
    collections::Collection,
    expression::{Env, Meter},
    types::{CollectionId, Identity, RecordDef, Timestamp, TypeDef, TypeId, TypeRegistry, Value},
    Path, Report,
};
use codec::{Decode, Encode};
use frame_support::pallet_prelude::*;
use frame_system::pallet_prelude::*;
use sp_runtime::Saturating;

use crate::{
    migrate::{MigrationPlan, MigrationState},
    primitives::{name_of, name_text, BoundedName, DeploymentId, TypeHash},
};

/// The record rewrite a migration asks of the record store.
///
/// The schema module holds definitions. The state module holds records.
/// An applied migration changes both. The schema module calls this hook so
/// the record rewrite runs inside the same transaction as the definition
/// change.
pub trait MigrationHook {
    /// Rewrite every record of the collection into the new shape.
    ///
    /// The result counts the records the call rewrote.
    fn migrate_records(
        deployment: &DeploymentId,
        plan: &MigrationPlan,
        produced_type: &RecordDef,
        produced_collection: &Collection,
    ) -> Result<u32, DispatchError>;

    /// Rewrite every record of the collection back into the earlier shape.
    ///
    /// The result counts the records the call rewrote.
    fn revert_records(
        deployment: &DeploymentId,
        inverse: &MigrationPlan,
        restored_type: &RecordDef,
        restored_collection: &Collection,
    ) -> Result<u32, DispatchError>;
}

/// The hook of a deployment carrying no record store.
///
/// A chain that runs the schema module alone registers definitions and
/// holds no records. The counts are therefore zero.
impl MigrationHook for () {
    fn migrate_records(
        _deployment: &DeploymentId,
        _plan: &MigrationPlan,
        _produced_type: &RecordDef,
        _produced_collection: &Collection,
    ) -> Result<u32, DispatchError> {
        Ok(0)
    }

    fn revert_records(
        _deployment: &DeploymentId,
        _inverse: &MigrationPlan,
        _restored_type: &RecordDef,
        _restored_collection: &Collection,
    ) -> Result<u32, DispatchError> {
        Ok(0)
    }
}

/// One registered collection with its version and its status.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
#[scale_info(skip_type_params(BlockNumber))]
pub struct CollectionEntry<BlockNumber> {
    /// The collection definition.
    pub definition: Collection,
    /// The live version number. Registration starts at one.
    pub version: u32,
    /// A retired collection keeps its records readable and declines every
    /// further write.
    pub retired: bool,
    /// The block the collection first registered at.
    pub registered_at: BlockNumber,
    /// The block the definition last changed at.
    pub changed_at: BlockNumber,
}

/// One stored migration plan with its status.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
#[scale_info(skip_type_params(AccountId, BlockNumber))]
pub struct MigrationEntry<AccountId, BlockNumber> {
    /// The plan.
    pub plan: MigrationPlan,
    /// The status of the plan.
    pub state: MigrationState,
    /// The account that proposed the plan.
    pub proposer: AccountId,
    /// The block the proposal landed at.
    pub proposed_at: BlockNumber,
    /// The block the apply call ran at.
    pub applied_at: Option<BlockNumber>,
    /// The number of records the apply call rewrote.
    pub records_rewritten: u32,
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    /// The settings the runtime supplies to the schema module.
    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// The event type of the runtime.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// The weight figures of the calls.
        type WeightInfo: WeightInfo;

        /// The record store the applied migrations rewrite.
        type Records: MigrationHook;

        /// The largest number of type definitions one deployment holds.
        #[pallet::constant]
        type MaxTypesPerDeployment: Get<u32>;

        /// The largest number of collections one deployment holds.
        #[pallet::constant]
        type MaxCollectionsPerDeployment: Get<u32>;

        /// The largest encoded definition the module accepts, in bytes.
        #[pallet::constant]
        type MaxDefinitionBytes: Get<u32>;

        /// The largest encoded migration plan the module accepts, in bytes.
        #[pallet::constant]
        type MaxMigrationBytes: Get<u32>;

        /// The largest number of stored versions one collection keeps.
        #[pallet::constant]
        type MaxVersionsPerCollection: Get<u32>;

        /// The expression step budget a migration conversion runs inside.
        #[pallet::constant]
        type MigrationStepLimit: Get<u64>;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// The account that owns the registry of one deployment.
    #[pallet::storage]
    pub type Stewards<T: Config> =
        StorageMap<_, Blake2_128Concat, DeploymentId, T::AccountId, OptionQuery>;

    /// Type definitions keyed by the content hash of the definition.
    #[pallet::storage]
    #[pallet::unbounded]
    pub type TypeDefs<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        DeploymentId,
        Blake2_128Concat,
        TypeHash,
        TypeDef,
        OptionQuery,
    >;

    /// The content hash a type name resolves to.
    #[pallet::storage]
    pub type TypeHashes<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        DeploymentId,
        Blake2_128Concat,
        BoundedName,
        TypeHash,
        OptionQuery,
    >;

    /// The registered type names of one deployment, in name order.
    #[pallet::storage]
    pub type TypeNames<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        DeploymentId,
        BoundedVec<BoundedName, T::MaxTypesPerDeployment>,
        ValueQuery,
    >;

    /// Collection definitions keyed by the collection name.
    #[pallet::storage]
    #[pallet::unbounded]
    pub type CollectionDefs<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        DeploymentId,
        Blake2_128Concat,
        BoundedName,
        CollectionEntry<BlockNumberFor<T>>,
        OptionQuery,
    >;

    /// The registered collection names of one deployment, in name order.
    #[pallet::storage]
    pub type CollectionNames<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        DeploymentId,
        BoundedVec<BoundedName, T::MaxCollectionsPerDeployment>,
        ValueQuery,
    >;

    /// The version history of every collection.
    #[pallet::storage]
    #[pallet::unbounded]
    pub type CollectionHistory<T: Config> = StorageNMap<
        _,
        (
            NMapKey<Blake2_128Concat, DeploymentId>,
            NMapKey<Blake2_128Concat, BoundedName>,
            NMapKey<Twox64Concat, u32>,
        ),
        Collection,
        OptionQuery,
    >;

    /// The migration plans between versions, keyed by the version each
    /// plan starts from.
    #[pallet::storage]
    #[pallet::unbounded]
    pub type Migrations<T: Config> = StorageNMap<
        _,
        (
            NMapKey<Blake2_128Concat, DeploymentId>,
            NMapKey<Blake2_128Concat, BoundedName>,
            NMapKey<Twox64Concat, u32>,
        ),
        MigrationEntry<T::AccountId, BlockNumberFor<T>>,
        OptionQuery,
    >;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A deployment gained its steward account.
        StewardSet {
            /// The deployment.
            deployment: DeploymentId,
            /// The steward.
            steward: T::AccountId,
        },
        /// A type definition entered the registry.
        TypeRegistered {
            /// The deployment.
            deployment: DeploymentId,
            /// The type name.
            name: BoundedName,
            /// The content hash of the definition.
            hash: TypeHash,
        },
        /// A collection definition entered the registry.
        CollectionRegistered {
            /// The deployment.
            deployment: DeploymentId,
            /// The collection name.
            name: BoundedName,
            /// The version the registration produced.
            version: u32,
        },
        /// A migration plan entered the registry.
        MigrationProposed {
            /// The deployment.
            deployment: DeploymentId,
            /// The collection the plan changes.
            name: BoundedName,
            /// The version the plan starts from.
            from_version: u32,
            /// The version the plan produces.
            to_version: u32,
        },
        /// A migration plan changed the registry and the records.
        MigrationApplied {
            /// The deployment.
            deployment: DeploymentId,
            /// The collection the plan changed.
            name: BoundedName,
            /// The version the plan started from.
            from_version: u32,
            /// The version the collection now carries.
            to_version: u32,
            /// The number of records the apply call rewrote.
            records: u32,
        },
        /// A migration plan was undone inside its window.
        MigrationReverted {
            /// The deployment.
            deployment: DeploymentId,
            /// The collection the revert restored.
            name: BoundedName,
            /// The version the collection now carries.
            version: u32,
            /// The number of records the revert rewrote.
            records: u32,
        },
        /// A collection stopped accepting writes.
        CollectionRetired {
            /// The deployment.
            deployment: DeploymentId,
            /// The collection name.
            name: BoundedName,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// The name runs past 128 bytes. Shorten the name.
        NameTooLong,
        /// The payload failed to decode. Submit the canonical encoding the
        /// Studio produces.
        DefinitionUnreadable,
        /// The definition failed validation. Run the check call to read
        /// every problem with its location and its fix.
        DefinitionInvalid,
        /// The deployment already holds a different definition under that
        /// name. Propose a migration instead.
        DefinitionConflict,
        /// The deployment holds its full count of type definitions. Raise
        /// the limit in the runtime, or retire an unused type.
        TypeLimitReached,
        /// The deployment holds its full count of collections. Raise the
        /// limit in the runtime, or retire an unused collection.
        CollectionLimitReached,
        /// The registry lacks that collection. Register the collection
        /// before naming it.
        CollectionUnknown,
        /// The collection is retired. Register a new collection for new
        /// records.
        CollectionRetired,
        /// The registry lacks the record type the collection names.
        /// Register the record type first.
        RecordTypeUnknown,
        /// The registry lacks a migration plan starting from that version.
        /// Propose the plan before applying it.
        MigrationUnknown,
        /// The plan already ran. Propose a further plan for a further
        /// change.
        MigrationAlreadyApplied,
        /// The plan has yet to run. Apply the plan before reverting it.
        MigrationNotApplied,
        /// The plan holds a step that carries no reverse. Restore the
        /// collection from an export instead.
        MigrationNotReversible,
        /// The rollback window closed. Propose a forward plan restoring
        /// the earlier shape.
        RevertWindowClosed,
        /// The collection keeps its full count of stored versions. Raise
        /// the limit in the runtime.
        VersionLimitReached,
        /// The account holds no stewardship over the deployment. Submit
        /// the call from the steward account.
        NotSteward,
        /// The record rewrite ran past its step budget. Simplify the
        /// conversion, or raise the budget in the runtime.
        MigrationBudgetSpent,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Register one type definition.
        ///
        /// The payload carries the canonical encoding of a type
        /// definition. The registry keys the definition by its content
        /// hash and resolves the type name to that hash.
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::register_type())]
        pub fn register_type(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            definition: BoundedVec<u8, T::MaxDefinitionBytes>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::claim_steward(&who, &deployment)?;

            let parsed = TypeDef::decode(&mut definition.as_slice())
                .map_err(|_| Error::<T>::DefinitionUnreadable)?;
            let report = Self::check_type(&deployment, &parsed);
            ensure!(report.is_clean(), Error::<T>::DefinitionInvalid);

            let name = name_of(parsed.id().as_str()).map_err(|_| Error::<T>::NameTooLong)?;
            let hash = Self::type_hash(&parsed);

            if let Some(present) = TypeHashes::<T>::get(deployment, &name) {
                ensure!(present == hash, Error::<T>::DefinitionConflict);
            } else {
                TypeNames::<T>::try_mutate(deployment, |names| match names.binary_search(&name) {
                    Ok(_) => Ok(()),
                    Err(slot) => names
                        .try_insert(slot, name.clone())
                        .map_err(|_| Error::<T>::TypeLimitReached),
                })?;
            }

            TypeDefs::<T>::insert(deployment, hash, parsed);
            TypeHashes::<T>::insert(deployment, &name, hash);
            Self::deposit_event(Event::TypeRegistered {
                deployment,
                name,
                hash,
            });
            Ok(())
        }

        /// Register one collection definition.
        ///
        /// A first registration produces version one. A registration that
        /// repeats the stored definition keeps the version. A changed
        /// definition needs a migration plan.
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::register_collection())]
        pub fn register_collection(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            definition: BoundedVec<u8, T::MaxDefinitionBytes>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::claim_steward(&who, &deployment)?;

            let parsed = Collection::decode(&mut definition.as_slice())
                .map_err(|_| Error::<T>::DefinitionUnreadable)?;
            let report = Self::check_collection(&deployment, &parsed);
            ensure!(report.is_clean(), Error::<T>::DefinitionInvalid);

            let name = name_of(parsed.id.as_str()).map_err(|_| Error::<T>::NameTooLong)?;
            let now = frame_system::Pallet::<T>::block_number();

            match CollectionDefs::<T>::get(deployment, &name) {
                Some(entry) => {
                    ensure!(!entry.retired, Error::<T>::CollectionRetired);
                    ensure!(entry.definition == parsed, Error::<T>::DefinitionConflict);
                    Self::deposit_event(Event::CollectionRegistered {
                        deployment,
                        name,
                        version: entry.version,
                    });
                }
                None => {
                    CollectionNames::<T>::try_mutate(deployment, |names| {
                        match names.binary_search(&name) {
                            Ok(_) => Ok(()),
                            Err(slot) => names
                                .try_insert(slot, name.clone())
                                .map_err(|_| Error::<T>::CollectionLimitReached),
                        }
                    })?;
                    CollectionDefs::<T>::insert(
                        deployment,
                        &name,
                        CollectionEntry {
                            definition: parsed.clone(),
                            version: 1,
                            retired: false,
                            registered_at: now,
                            changed_at: now,
                        },
                    );
                    CollectionHistory::<T>::insert((deployment, name.clone(), 1u32), parsed);
                    Self::deposit_event(Event::CollectionRegistered {
                        deployment,
                        name,
                        version: 1,
                    });
                }
            }
            Ok(())
        }

        /// Store one migration plan.
        ///
        /// The plan names the collection, the version it starts from and
        /// the version it produces. Storing a plan changes nothing. The
        /// apply call changes the registry and the records.
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::propose_migration())]
        pub fn propose_migration(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            plan: BoundedVec<u8, T::MaxMigrationBytes>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_steward(&who, &deployment)?;

            let parsed = MigrationPlan::decode(&mut plan.as_slice())
                .map_err(|_| Error::<T>::DefinitionUnreadable)?;
            let name = name_of(parsed.collection.as_str()).map_err(|_| Error::<T>::NameTooLong)?;
            let entry =
                CollectionDefs::<T>::get(deployment, &name).ok_or(Error::<T>::CollectionUnknown)?;
            ensure!(!entry.retired, Error::<T>::CollectionRetired);

            let report = Self::check_migration(&deployment, &parsed, &entry);
            ensure!(report.is_clean(), Error::<T>::DefinitionInvalid);

            let now = frame_system::Pallet::<T>::block_number();
            Migrations::<T>::insert(
                (deployment, name.clone(), parsed.from_version),
                MigrationEntry {
                    plan: parsed.clone(),
                    state: MigrationState::Proposed,
                    proposer: who,
                    proposed_at: now,
                    applied_at: None,
                    records_rewritten: 0,
                },
            );
            Self::deposit_event(Event::MigrationProposed {
                deployment,
                name,
                from_version: parsed.from_version,
                to_version: parsed.to_version,
            });
            Ok(())
        }

        /// Apply one stored migration plan.
        ///
        /// The call rewrites the record type, rewrites the collection
        /// definition, raises the version and asks the record store to
        /// rewrite every stored record. A failure at any step leaves the
        /// registry and the records untouched.
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::apply_migration())]
        pub fn apply_migration(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            collection: BoundedName,
            from_version: u32,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_steward(&who, &deployment)?;
            Self::run_migration(deployment, collection, from_version)
        }

        /// Undo one applied migration plan.
        ///
        /// A revert restores the earlier definition and asks the record
        /// store to rewrite every record back. A plan holding a step that
        /// loses data carries no reverse and declines the call.
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::revert_migration())]
        pub fn revert_migration(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            collection: BoundedName,
            from_version: u32,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_steward(&who, &deployment)?;
            Self::run_revert(deployment, collection, from_version)
        }

        /// Retire one collection.
        ///
        /// A retired collection keeps every record readable and exportable
        /// and declines every further write.
        #[pallet::call_index(5)]
        #[pallet::weight(T::WeightInfo::retire_collection())]
        pub fn retire_collection(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            collection: BoundedName,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_steward(&who, &deployment)?;
            CollectionDefs::<T>::try_mutate(deployment, &collection, |slot| -> DispatchResult {
                let entry = slot.as_mut().ok_or(Error::<T>::CollectionUnknown)?;
                ensure!(!entry.retired, Error::<T>::CollectionRetired);
                entry.retired = true;
                entry.changed_at = frame_system::Pallet::<T>::block_number();
                Ok(())
            })?;
            Self::deposit_event(Event::CollectionRetired {
                deployment,
                name: collection,
            });
            Ok(())
        }
    }
}

impl<T: Config> Pallet<T> {
    /// The content hash of one type definition.
    pub fn type_hash(definition: &TypeDef) -> TypeHash {
        sp_crypto_hashing::blake2_256(&definition.encode())
    }

    /// Take stewardship of a deployment that holds none, or check the
    /// steward already in place.
    fn claim_steward(who: &T::AccountId, deployment: &DeploymentId) -> DispatchResult {
        match Stewards::<T>::get(deployment) {
            Some(holder) => {
                ensure!(&holder == who, Error::<T>::NotSteward);
            }
            None => {
                Stewards::<T>::insert(deployment, who.clone());
                Self::deposit_event(Event::StewardSet {
                    deployment: *deployment,
                    steward: who.clone(),
                });
            }
        }
        Ok(())
    }

    /// Check that the caller stewards a deployment already registered.
    fn ensure_steward(who: &T::AccountId, deployment: &DeploymentId) -> DispatchResult {
        let holder = Stewards::<T>::get(deployment).ok_or(Error::<T>::NotSteward)?;
        ensure!(&holder == who, Error::<T>::NotSteward);
        Ok(())
    }

    /// Check one type definition against the deployment registry.
    ///
    /// A clean report means the definition registers. Every diagnostic
    /// carries the exact location and one sentence naming the fix.
    pub fn check_type(deployment: &DeploymentId, definition: &TypeDef) -> Report {
        let mut held = Self::type_defs(deployment);
        held.retain(|present| present.id() != definition.id());
        held.push(definition.clone());
        held.sort_by(|left, right| left.id().cmp(right.id()));
        cerulea_blueprint::types::validate_types(&held, &Path::root().field("types"))
    }

    /// Check one collection definition against the deployment registry.
    pub fn check_collection(deployment: &DeploymentId, definition: &Collection) -> Report {
        let defs = Self::type_defs(deployment);
        let (types, _) = TypeRegistry::build(&defs, &Path::root().field("types"));
        let held = vec![definition.clone()];
        cerulea_blueprint::collections::validate_collections(
            &held,
            &types,
            &Path::root().field("collections"),
        )
    }

    /// Check one migration plan against the stored collection.
    pub fn check_migration(
        deployment: &DeploymentId,
        plan: &MigrationPlan,
        entry: &CollectionEntry<BlockNumberFor<T>>,
    ) -> Report {
        let defs = Self::type_defs(deployment);
        let (types, _) = TypeRegistry::build(&defs, &Path::root().field("types"));
        migrate::check_plan(
            plan,
            &entry.definition,
            entry.version,
            &types,
            &Path::root().field("migration"),
        )
    }

    /// Run the planning pass of one migration plan and produce the
    /// definitions the plan yields.
    ///
    /// The Studio shows a user those definitions before the user commits
    /// the plan. A plan carrying a problem yields the report instead, with
    /// every diagnostic naming its location and its fix.
    pub fn preview_migration(
        deployment: &DeploymentId,
        plan: &MigrationPlan,
    ) -> Result<migrate::MigrationPreview, Report> {
        let entry = match Self::collection_entry(deployment, &plan.collection) {
            Some(found) => found,
            None => {
                let mut missing = Report::new();
                missing.push(cerulea_blueprint::Diagnostic::at(
                    cerulea_blueprint::DiagnosticCode::DanglingReference,
                    Path::root()
                        .field("migration")
                        .field("collection")
                        .location(),
                    alloc::format!("The registry lacks the collection `{}`.", plan.collection),
                    alloc::string::String::from(
                        "Register the collection before planning a migration.",
                    ),
                ));
                return Err(missing);
            }
        };
        let defs = Self::type_defs(deployment);
        let (types, _) = TypeRegistry::build(&defs, &Path::root().field("types"));
        migrate::preview(
            plan,
            &entry.definition,
            entry.version,
            &types,
            &Path::root().field("migration"),
        )
    }

    /// The registered type names of one deployment, in name order.
    pub fn type_names(deployment: &DeploymentId) -> Vec<BoundedName> {
        TypeNames::<T>::get(deployment).into_inner()
    }

    /// The registered collection names of one deployment, in name order.
    pub fn collection_names(deployment: &DeploymentId) -> Vec<BoundedName> {
        CollectionNames::<T>::get(deployment).into_inner()
    }

    /// The registered type definitions of one deployment, in name order.
    pub fn type_defs(deployment: &DeploymentId) -> Vec<TypeDef> {
        TypeNames::<T>::get(deployment)
            .into_iter()
            .filter_map(|name| TypeHashes::<T>::get(deployment, &name))
            .filter_map(|hash| TypeDefs::<T>::get(deployment, hash))
            .collect()
    }

    /// One stored collection entry.
    pub fn collection_entry(
        deployment: &DeploymentId,
        id: &CollectionId,
    ) -> Option<CollectionEntry<BlockNumberFor<T>>> {
        let name = name_of(id.as_str()).ok()?;
        CollectionDefs::<T>::get(deployment, &name)
    }

    /// One stored version of one collection.
    pub fn collection_at(
        deployment: &DeploymentId,
        id: &CollectionId,
        version: u32,
    ) -> Option<Collection> {
        let name = name_of(id.as_str()).ok()?;
        CollectionHistory::<T>::get((*deployment, name, version))
    }

    /// One stored migration entry.
    pub fn migration_entry(
        deployment: &DeploymentId,
        id: &CollectionId,
        from_version: u32,
    ) -> Option<MigrationEntry<T::AccountId, BlockNumberFor<T>>> {
        let name = name_of(id.as_str()).ok()?;
        Migrations::<T>::get((*deployment, name, from_version))
    }

    fn run_migration(
        deployment: DeploymentId,
        collection: BoundedName,
        from_version: u32,
    ) -> DispatchResult {
        let mut migration = Migrations::<T>::get((deployment, collection.clone(), from_version))
            .ok_or(Error::<T>::MigrationUnknown)?;
        ensure!(
            migration.state == MigrationState::Proposed,
            Error::<T>::MigrationAlreadyApplied
        );
        let entry = CollectionDefs::<T>::get(deployment, &collection)
            .ok_or(Error::<T>::CollectionUnknown)?;
        ensure!(!entry.retired, Error::<T>::CollectionRetired);

        let defs = Self::type_defs(&deployment);
        let (types, _) = TypeRegistry::build(&defs, &Path::root().field("types"));
        let report = migrate::check_plan(
            &migration.plan,
            &entry.definition,
            entry.version,
            &types,
            &Path::root().field("migration"),
        );
        ensure!(report.is_clean(), Error::<T>::DefinitionInvalid);

        let present = types
            .record(&entry.definition.record_type)
            .ok_or(Error::<T>::RecordTypeUnknown)?
            .clone();
        let produced_type =
            migrate::plan_record_type(&migration.plan, &present, &Path::root().field("migration"))
                .map_err(|_| Error::<T>::DefinitionInvalid)?;
        let produced_collection = migrate::plan_collection(
            &migration.plan,
            &entry.definition,
            &Path::root().field("migration"),
        )
        .map_err(|_| Error::<T>::DefinitionInvalid)?;

        let records = T::Records::migrate_records(
            &deployment,
            &migration.plan,
            &produced_type,
            &produced_collection,
        )?;

        Self::store_record_type(&deployment, produced_type)?;
        Self::store_collection_version(
            &deployment,
            &collection,
            &entry,
            produced_collection,
            migration.plan.to_version,
        )?;

        let now = frame_system::Pallet::<T>::block_number();
        migration.state = MigrationState::Applied;
        migration.applied_at = Some(now);
        migration.records_rewritten = records;
        let to_version = migration.plan.to_version;
        Migrations::<T>::insert((deployment, collection.clone(), from_version), migration);

        Self::deposit_event(Event::MigrationApplied {
            deployment,
            name: collection,
            from_version,
            to_version,
            records,
        });
        Ok(())
    }

    fn run_revert(
        deployment: DeploymentId,
        collection: BoundedName,
        from_version: u32,
    ) -> DispatchResult {
        let mut migration = Migrations::<T>::get((deployment, collection.clone(), from_version))
            .ok_or(Error::<T>::MigrationUnknown)?;
        ensure!(
            migration.state == MigrationState::Applied,
            Error::<T>::MigrationNotApplied
        );
        let applied_at = migration
            .applied_at
            .ok_or(Error::<T>::MigrationNotApplied)?;
        let now = frame_system::Pallet::<T>::block_number();
        let elapsed = now.saturating_sub(applied_at);
        let elapsed: u32 = elapsed.try_into().unwrap_or(u32::MAX);
        ensure!(
            migrate::revert_available(0, elapsed, migration.plan.rollback_window),
            Error::<T>::RevertWindowClosed
        );

        let entry = CollectionDefs::<T>::get(deployment, &collection)
            .ok_or(Error::<T>::CollectionUnknown)?;
        let restored_collection =
            CollectionHistory::<T>::get((deployment, collection.clone(), from_version))
                .ok_or(Error::<T>::MigrationNotApplied)?;

        let inverse = migrate::inverse_plan(
            &migration.plan,
            &restored_collection,
            &Path::root().field("migration"),
        )
        .map_err(|_| Error::<T>::MigrationNotReversible)?;

        let defs = Self::type_defs(&deployment);
        let (types, _) = TypeRegistry::build(&defs, &Path::root().field("types"));
        let present = types
            .record(&entry.definition.record_type)
            .ok_or(Error::<T>::RecordTypeUnknown)?
            .clone();
        let restored_type =
            migrate::plan_record_type(&inverse, &present, &Path::root().field("migration"))
                .map_err(|_| Error::<T>::MigrationNotReversible)?;

        let records = T::Records::revert_records(
            &deployment,
            &inverse,
            &restored_type,
            &restored_collection,
        )?;

        Self::store_record_type(&deployment, restored_type)?;
        CollectionDefs::<T>::insert(
            deployment,
            &collection,
            CollectionEntry {
                definition: restored_collection,
                version: from_version,
                retired: entry.retired,
                registered_at: entry.registered_at,
                changed_at: now,
            },
        );

        migration.state = MigrationState::Reverted;
        migration.records_rewritten = records;
        Migrations::<T>::insert((deployment, collection.clone(), from_version), migration);

        Self::deposit_event(Event::MigrationReverted {
            deployment,
            name: collection,
            version: from_version,
            records,
        });
        Ok(())
    }

    fn store_record_type(deployment: &DeploymentId, produced: RecordDef) -> DispatchResult {
        let name = name_of(produced.id.as_str()).map_err(|_| Error::<T>::NameTooLong)?;
        let definition = TypeDef::Record(produced);
        let hash = Self::type_hash(&definition);
        if let Some(previous) = TypeHashes::<T>::get(deployment, &name) {
            if previous != hash {
                TypeDefs::<T>::remove(deployment, previous);
            }
        } else {
            TypeNames::<T>::try_mutate(deployment, |names| match names.binary_search(&name) {
                Ok(_) => Ok(()),
                Err(slot) => names
                    .try_insert(slot, name.clone())
                    .map_err(|_| Error::<T>::TypeLimitReached),
            })?;
        }
        TypeDefs::<T>::insert(deployment, hash, definition);
        TypeHashes::<T>::insert(deployment, &name, hash);
        Ok(())
    }

    fn store_collection_version(
        deployment: &DeploymentId,
        name: &BoundedName,
        entry: &CollectionEntry<BlockNumberFor<T>>,
        produced: Collection,
        version: u32,
    ) -> DispatchResult {
        ensure!(
            version <= T::MaxVersionsPerCollection::get(),
            Error::<T>::VersionLimitReached
        );
        CollectionHistory::<T>::insert((*deployment, name.clone(), version), produced.clone());
        CollectionDefs::<T>::insert(
            deployment,
            name,
            CollectionEntry {
                definition: produced,
                version,
                retired: entry.retired,
                registered_at: entry.registered_at,
                changed_at: frame_system::Pallet::<T>::block_number(),
            },
        );
        Ok(())
    }

    /// An evaluation setting for a migration conversion.
    ///
    /// A conversion reads the old record alone. The caller identity and
    /// the clock stay fixed, so two nodes convert alike.
    pub fn migration_env() -> (Env<'static>, Meter) {
        let env = Env::new(Identity([0u8; 32]), Timestamp::from_millis(0));
        let meter = Meter::new(T::MigrationStepLimit::get());
        (env, meter)
    }

    /// Rewrite one record value through a plan.
    ///
    /// The state module calls this from its migration hook.
    pub fn migrate_value(plan: &MigrationPlan, record: &Value) -> Result<Value, DispatchError> {
        let (env, mut meter) = Self::migration_env();
        migrate::migrate_record(
            plan,
            record,
            &env,
            &mut meter,
            &Path::root().field("record"),
        )
        .map_err(|_| Error::<T>::MigrationBudgetSpent.into())
    }
}

impl<T: Config> SchemaRegistry for Pallet<T> {
    fn type_defs(deployment: &DeploymentId) -> Vec<TypeDef> {
        Pallet::<T>::type_defs(deployment)
    }

    fn type_def(deployment: &DeploymentId, id: &TypeId) -> Option<TypeDef> {
        let name = name_of(id.as_str()).ok()?;
        let hash = TypeHashes::<T>::get(deployment, &name)?;
        TypeDefs::<T>::get(deployment, hash)
    }

    fn collections(deployment: &DeploymentId) -> Vec<Collection> {
        CollectionNames::<T>::get(deployment)
            .into_iter()
            .filter_map(|name| CollectionDefs::<T>::get(deployment, &name))
            .map(|entry| entry.definition)
            .collect()
    }

    fn collection(deployment: &DeploymentId, id: &CollectionId) -> Option<Collection> {
        Pallet::<T>::collection_entry(deployment, id).map(|entry| entry.definition)
    }

    fn collection_version(deployment: &DeploymentId, id: &CollectionId) -> Option<u32> {
        Pallet::<T>::collection_entry(deployment, id).map(|entry| entry.version)
    }

    fn is_retired(deployment: &DeploymentId, id: &CollectionId) -> bool {
        Pallet::<T>::collection_entry(deployment, id)
            .map(|entry| entry.retired)
            .unwrap_or(false)
    }

    fn is_installed(deployment: &DeploymentId) -> bool {
        !CollectionNames::<T>::get(deployment).is_empty()
    }
}

impl<T: Config> SchemaRegistrar for Pallet<T> {
    fn register_blueprint(
        deployment: &DeploymentId,
        blueprint: &cerulea_blueprint::Blueprint,
    ) -> Result<(), DispatchError> {
        for definition in &blueprint.types {
            let name = name_of(definition.id().as_str()).map_err(|_| Error::<T>::NameTooLong)?;
            let hash = Self::type_hash(definition);
            if TypeHashes::<T>::get(deployment, &name).is_none() {
                TypeNames::<T>::try_mutate(deployment, |names| match names.binary_search(&name) {
                    Ok(_) => Ok(()),
                    Err(slot) => names
                        .try_insert(slot, name.clone())
                        .map_err(|_| Error::<T>::TypeLimitReached),
                })?;
            }
            TypeDefs::<T>::insert(deployment, hash, definition.clone());
            TypeHashes::<T>::insert(deployment, &name, hash);
        }

        let now = frame_system::Pallet::<T>::block_number();
        for definition in &blueprint.collections {
            let name = name_of(definition.id.as_str()).map_err(|_| Error::<T>::NameTooLong)?;
            match CollectionDefs::<T>::get(deployment, &name) {
                Some(entry) => {
                    if &entry.definition == definition {
                        continue;
                    }
                    let version = entry.version.saturating_add(1);
                    ensure!(
                        version <= T::MaxVersionsPerCollection::get(),
                        Error::<T>::VersionLimitReached
                    );
                    CollectionHistory::<T>::insert(
                        (*deployment, name.clone(), version),
                        definition.clone(),
                    );
                    CollectionDefs::<T>::insert(
                        deployment,
                        &name,
                        CollectionEntry {
                            definition: definition.clone(),
                            version,
                            retired: entry.retired,
                            registered_at: entry.registered_at,
                            changed_at: now,
                        },
                    );
                }
                None => {
                    CollectionNames::<T>::try_mutate(deployment, |names| {
                        match names.binary_search(&name) {
                            Ok(_) => Ok(()),
                            Err(slot) => names
                                .try_insert(slot, name.clone())
                                .map_err(|_| Error::<T>::CollectionLimitReached),
                        }
                    })?;
                    CollectionHistory::<T>::insert(
                        (*deployment, name.clone(), 1u32),
                        definition.clone(),
                    );
                    CollectionDefs::<T>::insert(
                        deployment,
                        &name,
                        CollectionEntry {
                            definition: definition.clone(),
                            version: 1,
                            retired: false,
                            registered_at: now,
                            changed_at: now,
                        },
                    );
                }
            }
        }
        Ok(())
    }
}

/// Read a bounded name as text.
pub fn text_of(name: &BoundedName) -> alloc::string::String {
    name_text(name)
}
