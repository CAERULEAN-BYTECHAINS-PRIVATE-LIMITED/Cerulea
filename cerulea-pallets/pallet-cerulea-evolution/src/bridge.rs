//! The seam between the evolution module and the schema registry.
//!
//! The schema module holds the type registry, the collection registry and
//! the migration machinery. The evolution module orders the version
//! transition. The trait here keeps the two apart, so a runtime supplies
//! the registry it carries.
//!
//! The crate ships one implementation over the schema module. The
//! implementation drives the registry through its own entry points, so the
//! record rewrite and the definition change stay inside one transaction.

use alloc::vec::Vec;
use cerulea_blueprint::types::CollectionId;
use core::marker::PhantomData;
use frame_support::BoundedVec;
use frame_system::RawOrigin;
use pallet_cerulea_schema::{
    migrate::{MigrationPlan, MigrationState},
    primitives::{name_of, DeploymentId},
    registry::SchemaRegistry,
};
use sp_runtime::DispatchError;

/// What one planning pass found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DryRun {
    /// The plan rewrites every stored record.
    pub rewrites_records: bool,
    /// The plan rebuilds the secondary indexes.
    pub rebuilds_indexes: bool,
    /// The plan carries a reverse, so a rollback restores the earlier
    /// shape.
    pub reversible: bool,
}

/// The registry the evolution module orders a transition against.
pub trait SchemaMigrations {
    /// The account type the runtime carries.
    type AccountId;

    /// The live version number of one collection.
    fn collection_version(deployment: &DeploymentId, collection: &CollectionId) -> Option<u32>;

    /// Report whether the registry holds one collection.
    fn has_collection(deployment: &DeploymentId, collection: &CollectionId) -> bool;

    /// Run the planning pass without changing anything.
    ///
    /// A plan carrying a problem answers with an error naming the module
    /// that declined it. The Studio shows the full report before the plan
    /// reaches the Blockchain.
    fn dry_run(deployment: &DeploymentId, plan: &MigrationPlan) -> Result<DryRun, DispatchError>;

    /// Apply one plan and answer with the number of records it rewrote.
    fn apply(
        steward: &Self::AccountId,
        deployment: &DeploymentId,
        plan: &MigrationPlan,
    ) -> Result<u32, DispatchError>;

    /// Undo one applied plan and answer with the number of records it
    /// rewrote back.
    fn revert(
        steward: &Self::AccountId,
        deployment: &DeploymentId,
        plan: &MigrationPlan,
    ) -> Result<u32, DispatchError>;
}

/// The implementation over the schema module.
pub struct SchemaBridge<T>(PhantomData<T>);

impl<T: pallet_cerulea_schema::Config> SchemaMigrations for SchemaBridge<T> {
    type AccountId = T::AccountId;

    fn collection_version(deployment: &DeploymentId, collection: &CollectionId) -> Option<u32> {
        <pallet_cerulea_schema::Pallet<T> as SchemaRegistry>::collection_version(
            deployment, collection,
        )
    }

    fn has_collection(deployment: &DeploymentId, collection: &CollectionId) -> bool {
        <pallet_cerulea_schema::Pallet<T> as SchemaRegistry>::collection(deployment, collection)
            .is_some()
    }

    fn dry_run(deployment: &DeploymentId, plan: &MigrationPlan) -> Result<DryRun, DispatchError> {
        let preview = pallet_cerulea_schema::Pallet::<T>::preview_migration(deployment, plan)
            .map_err(|_| {
                DispatchError::Other("The migration plan carries a problem the registry named.")
            })?;
        Ok(DryRun {
            rewrites_records: preview.rewrites_records,
            rebuilds_indexes: preview.rebuilds_indexes,
            reversible: preview.reversible,
        })
    }

    fn apply(
        steward: &Self::AccountId,
        deployment: &DeploymentId,
        plan: &MigrationPlan,
    ) -> Result<u32, DispatchError> {
        let name = name_of(plan.collection.as_str())
            .map_err(|_| DispatchError::Other("The collection name runs past 128 bytes."))?;
        let bytes: BoundedVec<u8, T::MaxMigrationBytes> = BoundedVec::try_from(encode(plan))
            .map_err(|_| {
                DispatchError::Other("The migration plan runs past the plan size limit.")
            })?;

        // The registry applies only a proposed entry. A reverted entry, or an
        // entry carrying another plan from the same version, gives way to the
        // plan the evolution module ordered.
        let proposed = pallet_cerulea_schema::Pallet::<T>::migration_entry(
            deployment,
            &plan.collection,
            plan.from_version,
        )
        .map(|held| held.state == MigrationState::Proposed && held.plan == *plan)
        .unwrap_or(false);
        if !proposed {
            pallet_cerulea_schema::Pallet::<T>::propose_migration(
                RawOrigin::Signed(steward.clone()).into(),
                *deployment,
                bytes,
            )?;
        }
        pallet_cerulea_schema::Pallet::<T>::apply_migration(
            RawOrigin::Signed(steward.clone()).into(),
            *deployment,
            name,
            plan.from_version,
        )?;
        Ok(rewritten::<T>(deployment, plan))
    }

    fn revert(
        steward: &Self::AccountId,
        deployment: &DeploymentId,
        plan: &MigrationPlan,
    ) -> Result<u32, DispatchError> {
        let name = name_of(plan.collection.as_str())
            .map_err(|_| DispatchError::Other("The collection name runs past 128 bytes."))?;
        pallet_cerulea_schema::Pallet::<T>::revert_migration(
            RawOrigin::Signed(steward.clone()).into(),
            *deployment,
            name,
            plan.from_version,
        )?;
        Ok(rewritten::<T>(deployment, plan))
    }
}

/// The canonical encoding of one plan.
fn encode(plan: &MigrationPlan) -> Vec<u8> {
    codec::Encode::encode(plan)
}

/// The number of records the registry rewrote for one plan.
fn rewritten<T: pallet_cerulea_schema::Config>(
    deployment: &DeploymentId,
    plan: &MigrationPlan,
) -> u32 {
    pallet_cerulea_schema::Pallet::<T>::migration_entry(
        deployment,
        &plan.collection,
        plan.from_version,
    )
    .map(|held| held.records_rewritten)
    .unwrap_or(0)
}

/// Report whether the registry marks one plan as applied.
pub fn is_applied<T: pallet_cerulea_schema::Config>(
    deployment: &DeploymentId,
    plan: &MigrationPlan,
) -> bool {
    pallet_cerulea_schema::Pallet::<T>::migration_entry(
        deployment,
        &plan.collection,
        plan.from_version,
    )
    .map(|held| held.state == MigrationState::Applied)
    .unwrap_or(false)
}
