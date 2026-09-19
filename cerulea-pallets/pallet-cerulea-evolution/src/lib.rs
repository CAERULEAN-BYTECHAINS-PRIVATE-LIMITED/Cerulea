//! The Cerulea evolution module.
//!
//! The evolution module orders the version transition of a deployment.
//! The module holds the version registry, the migration definitions, the
//! rollback windows and the applied migration log.
//!
//! Five operations drive a transition. Plan migration. Dry run. Apply.
//! Verify. Roll back within the window.
//!
//! A plan states any break before approval. A plan that removes a field,
//! renames a field or retypes a field breaks an existing integration. Such
//! a plan reaches the Blockchain only after the electorate approved it at
//! the structural class threshold, which is 80 percent.
//!
//! A plan that adds a field, adds an index or changes a declaration keeps
//! every existing integration working. Such a plan applies without a
//! proposal.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod apply;
pub mod bridge;
pub mod weights;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub use bridge::{DryRun, SchemaBridge, SchemaMigrations};
pub use pallet::*;
pub use weights::WeightInfo;

use alloc::vec::Vec;
use cerulea_blueprint::types::CollectionId;
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use frame_support::{pallet_prelude::*, traits::UnixTime};
use frame_system::pallet_prelude::*;
use pallet_cerulea_audit::{AuditRequest, AuditSink, WorkClass};
use pallet_cerulea_governance::{ApprovalGate, ProposalKind};
use pallet_cerulea_schema::{
    migrate::{MigrationPlan, MigrationStep},
    primitives::{name_of, name_text, BoundedName, DeploymentId},
};

/// The name the module carries in the audit log.
pub const MODULE_NAME: &str = "evolution";

/// Where one migration plan stands.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    DecodeWithMemTracking,
    TypeInfo,
    MaxEncodedLen,
)]
pub enum PlanStatus {
    /// The plan is stored and waits for a dry run.
    Planned,
    /// The dry run passed and the plan waits for an apply call.
    Checked,
    /// The plan changed the registry and the records carry the new shape.
    Applied,
    /// The verify call confirmed the shape the plan produced.
    Verified,
    /// The plan was undone inside its rollback window.
    RolledBack,
}

/// One recorded version of one deployment.
#[derive(Clone, Debug, Encode, Decode, DecodeWithMemTracking, TypeInfo, MaxEncodedLen)]
pub struct VersionRecord<AccountId, BlockNumber> {
    /// The content address of the Blueprint this version installed.
    pub blueprint: [u8; 32],
    /// The revision the Blueprint carries.
    pub revision: u32,
    /// The wall clock time the version landed at.
    pub recorded_at: u64,
    /// The block the version landed in.
    pub recorded_block: BlockNumber,
    /// The account that recorded the version.
    pub recorded_by: AccountId,
    /// One plain sentence stating what the version changes.
    pub summary: BoundedVec<u8, ConstU32<256>>,
}

/// One migration plan with its status and its window.
#[derive(Clone, Debug, Encode, Decode, DecodeWithMemTracking, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(MaxBytes))]
pub struct PlanRecord<AccountId, BlockNumber, MaxBytes>
where
    MaxBytes: Get<u32> + 'static,
{
    /// The collection the plan changes.
    pub collection: BoundedName,
    /// The version the plan starts from.
    pub from_version: u32,
    /// The version the plan produces.
    pub to_version: u32,
    /// The canonical encoding of the plan.
    pub plan: BoundedVec<u8, MaxBytes>,
    /// The content hash of the plan.
    pub plan_hash: [u8; 32],
    /// The plan breaks an existing integration.
    pub breaking: bool,
    /// The plan carries a reverse.
    pub reversible: bool,
    /// The span a rollback stays available for, in milliseconds.
    pub rollback_window_millis: u64,
    /// Where the plan stands.
    pub status: PlanStatus,
    /// The account that planned the migration.
    pub planner: AccountId,
    /// The wall clock time the plan landed at.
    pub planned_at: u64,
    /// The wall clock time the apply call ran at.
    pub applied_at: Option<u64>,
    /// The block the apply call ran in.
    pub applied_block: Option<BlockNumber>,
    /// The wall clock time the rollback window closes at.
    pub rollback_deadline: Option<u64>,
    /// The number of records the apply call rewrote.
    pub records_rewritten: u32,
}

/// One entry of the applied migration log.
#[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo, MaxEncodedLen,
)]
pub struct AppliedEntry<BlockNumber> {
    /// The plan the entry records.
    pub plan: u64,
    /// The collection the plan changed.
    pub collection: BoundedName,
    /// The version the plan produced.
    pub to_version: u32,
    /// Where the plan stands now.
    pub status: PlanStatus,
    /// The wall clock time the entry landed at.
    pub at_millis: u64,
    /// The block the entry landed in.
    pub block: BlockNumber,
}

/// The read a dependant module performs over the platform versions.
///
/// The operator module upgrades and downgrades the platform version
/// through this trait.
pub trait PlatformVersions {
    /// The account type the runtime carries.
    type AccountId;

    /// The platform version one deployment runs.
    fn platform_version(deployment: &DeploymentId) -> u32;

    /// Move the deployment to another platform version.
    ///
    /// The call records the actor and the reason in the audit log.
    fn set_platform_version(
        deployment: &DeploymentId,
        version: u32,
        actor: &Self::AccountId,
        reason: &str,
    ) -> DispatchResult;
}

/// Report whether one step breaks an existing integration.
///
/// A step that removes a field, renames a field or retypes a field changes
/// the shape an existing reader expects. Every other step leaves an
/// existing reader working.
pub fn step_breaks(step: &MigrationStep) -> bool {
    matches!(
        step,
        MigrationStep::RemoveField { .. }
            | MigrationStep::RenameField { .. }
            | MigrationStep::RetypeField { .. }
    )
}

/// Report whether one plan breaks an existing integration.
pub fn plan_breaks(plan: &MigrationPlan) -> bool {
    plan.steps.iter().any(step_breaks)
}

/// The content hash of one plan.
pub fn plan_hash(plan: &[u8]) -> [u8; 32] {
    sp_crypto_hashing::blake2_256(&(b"cerulea.evolution.v1", plan).encode())
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    /// The settings the runtime supplies to the evolution module.
    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// The event type of the runtime.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// The weight figures of the calls.
        type WeightInfo: WeightInfo;

        /// The registry the module orders a transition against.
        type Schema: SchemaMigrations<AccountId = Self::AccountId>;

        /// The gate that answers whether the electorate approved a change.
        type Approvals: ApprovalGate;

        /// The log the module records its work in.
        type Audit: AuditSink<AccountId = Self::AccountId>;

        /// The clock every window reads.
        type Time: UnixTime;

        /// The largest migration plan, in bytes.
        #[pallet::constant]
        type MaxPlanBytes: Get<u32>;

        /// The largest number of versions one deployment records.
        #[pallet::constant]
        type MaxVersionsPerDeployment: Get<u32>;

        /// The largest rollback window, in milliseconds.
        #[pallet::constant]
        type MaxRollbackWindow: Get<u64>;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// Every recorded version of every deployment.
    #[pallet::storage]
    pub type Versions<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        DeploymentId,
        Twox64Concat,
        u32,
        VersionRecord<T::AccountId, BlockNumberFor<T>>,
        OptionQuery,
    >;

    /// The version every deployment runs now.
    #[pallet::storage]
    pub type ActiveVersion<T: Config> =
        StorageMap<_, Blake2_128Concat, DeploymentId, u32, ValueQuery>;

    /// The platform version every deployment runs now.
    #[pallet::storage]
    pub type PlatformVersion<T: Config> =
        StorageMap<_, Blake2_128Concat, DeploymentId, u32, ValueQuery>;

    /// Every migration plan.
    #[pallet::storage]
    pub type Plans<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        DeploymentId,
        Twox64Concat,
        u64,
        PlanRecord<T::AccountId, BlockNumberFor<T>, T::MaxPlanBytes>,
        OptionQuery,
    >;

    /// The identifier the next plan takes.
    #[pallet::storage]
    pub type NextPlan<T: Config> = StorageMap<_, Blake2_128Concat, DeploymentId, u64, ValueQuery>;

    /// The applied migration log, in the order the entries landed.
    #[pallet::storage]
    pub type AppliedLog<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        DeploymentId,
        Twox64Concat,
        u64,
        AppliedEntry<BlockNumberFor<T>>,
        OptionQuery,
    >;

    /// The number of entries the applied migration log holds.
    #[pallet::storage]
    pub type AppliedCount<T: Config> =
        StorageMap<_, Blake2_128Concat, DeploymentId, u64, ValueQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A version entered the registry.
        VersionRecorded {
            /// The deployment.
            deployment: DeploymentId,
            /// The version.
            version: u32,
            /// The content address of the Blueprint.
            blueprint: [u8; 32],
        },
        /// The platform version of a deployment changed.
        PlatformVersionSet {
            /// The deployment.
            deployment: DeploymentId,
            /// The version the deployment runs now.
            version: u32,
        },
        /// A migration plan entered the registry.
        MigrationPlanned {
            /// The deployment.
            deployment: DeploymentId,
            /// The plan.
            plan: u64,
            /// The collection the plan changes.
            collection: BoundedName,
            /// The plan breaks an existing integration.
            breaking: bool,
        },
        /// A dry run passed.
        DryRunPassed {
            /// The deployment.
            deployment: DeploymentId,
            /// The plan.
            plan: u64,
            /// The plan carries a reverse.
            reversible: bool,
        },
        /// A plan changed the registry and the records.
        MigrationApplied {
            /// The deployment.
            deployment: DeploymentId,
            /// The plan.
            plan: u64,
            /// The version the plan produced.
            to_version: u32,
            /// The number of records the call rewrote.
            records: u32,
        },
        /// A verify call confirmed the shape the plan produced.
        MigrationVerified {
            /// The deployment.
            deployment: DeploymentId,
            /// The plan.
            plan: u64,
        },
        /// A plan was undone inside its rollback window.
        MigrationRolledBack {
            /// The deployment.
            deployment: DeploymentId,
            /// The plan.
            plan: u64,
            /// The number of records the call rewrote back.
            records: u32,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// The name runs past 128 bytes. Shorten the collection name.
        NameTooLong,
        /// The plan failed to decode. Submit the canonical encoding the
        /// Studio produces.
        PlanUnreadable,
        /// The registry lacks the collection the plan names. Register the
        /// collection before planning a migration.
        CollectionUnknown,
        /// The plan starts from a version the collection left. Plan the
        /// migration from the live version of the collection.
        VersionMismatch,
        /// The registry holds no plan under that identifier. Plan the
        /// migration before running it.
        PlanUnknown,
        /// The plan already ran. Plan a new migration for a further
        /// change.
        PlanClosed,
        /// The plan waits for its dry run. Run the dry run before the
        /// apply call.
        DryRunPending,
        /// The dry run found a problem in the plan. Correct the plan in
        /// the Studio and plan the migration again.
        DryRunFailed,
        /// The plan breaks an existing integration and carries no
        /// approval. Open a schema change proposal and reach the
        /// structural threshold of 80 percent first.
        ApprovalMissing,
        /// The rollback window closed. Plan a further migration to change
        /// the shape again.
        WindowClosed,
        /// The plan carries no reverse. Plan a further migration to change
        /// the shape again.
        Irreversible,
        /// The requested window runs past the largest window. Shorten the
        /// window, or raise the limit in the runtime.
        WindowTooLong,
        /// The registry reports a shape other than the one the plan
        /// produced. Roll the migration back inside its window.
        VerificationFailed,
        /// The registry already holds that version. Record the next
        /// version number.
        VersionPresent,
        /// The registry holds no such version. Record the version before
        /// activating it.
        VersionUnknown,
        /// The deployment reached its last plan identifier. Start a new
        /// deployment.
        RegisterFull,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Record one version of the deployment.
        ///
        /// A version names the content address of the Blueprint it
        /// installed. The newest recorded version becomes the active
        /// version.
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::record_version())]
        pub fn record_version(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            version: u32,
            blueprint: [u8; 32],
            revision: u32,
            summary: BoundedVec<u8, ConstU32<256>>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(
                Versions::<T>::get(deployment, version).is_none(),
                Error::<T>::VersionPresent
            );
            let now = Self::now_millis();
            Versions::<T>::insert(
                deployment,
                version,
                VersionRecord {
                    blueprint,
                    revision,
                    recorded_at: now,
                    recorded_block: frame_system::Pallet::<T>::block_number(),
                    recorded_by: who.clone(),
                    summary,
                },
            );
            if version > ActiveVersion::<T>::get(deployment) {
                ActiveVersion::<T>::insert(deployment, version);
            }

            Self::record(
                &deployment,
                "version_recorded",
                WorkClass::Structural,
                &who,
                "The steward recorded one version of the deployment.",
                (version, blueprint).encode(),
            )?;
            Self::deposit_event(Event::VersionRecorded {
                deployment,
                version,
                blueprint,
            });
            Ok(())
        }

        /// Plan one migration.
        ///
        /// The call stores the plan and states whether the plan breaks an
        /// existing integration. Storing a plan changes nothing.
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::plan_migration())]
        pub fn plan_migration(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            plan: BoundedVec<u8, T::MaxPlanBytes>,
            rollback_window_millis: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(
                rollback_window_millis <= T::MaxRollbackWindow::get(),
                Error::<T>::WindowTooLong
            );
            let parsed = MigrationPlan::decode(&mut plan.as_slice())
                .map_err(|_| Error::<T>::PlanUnreadable)?;
            let collection =
                name_of(parsed.collection.as_str()).map_err(|_| Error::<T>::NameTooLong)?;
            ensure!(
                T::Schema::has_collection(&deployment, &parsed.collection),
                Error::<T>::CollectionUnknown
            );
            let live = T::Schema::collection_version(&deployment, &parsed.collection)
                .ok_or(Error::<T>::CollectionUnknown)?;
            ensure!(live == parsed.from_version, Error::<T>::VersionMismatch);

            let id = NextPlan::<T>::get(deployment);
            let next = id.checked_add(1).ok_or(Error::<T>::RegisterFull)?;
            let breaking = plan_breaks(&parsed);
            let hash = plan_hash(plan.as_slice());
            let now = Self::now_millis();

            Plans::<T>::insert(
                deployment,
                id,
                PlanRecord {
                    collection: collection.clone(),
                    from_version: parsed.from_version,
                    to_version: parsed.to_version,
                    plan,
                    plan_hash: hash,
                    breaking,
                    reversible: false,
                    rollback_window_millis,
                    status: PlanStatus::Planned,
                    planner: who.clone(),
                    planned_at: now,
                    applied_at: None,
                    applied_block: None,
                    rollback_deadline: None,
                    records_rewritten: 0,
                },
            );
            NextPlan::<T>::insert(deployment, next);

            Self::record(
                &deployment,
                "migration_planned",
                WorkClass::Structural,
                &who,
                "The steward planned one migration of a collection.",
                (id, collection.clone(), breaking).encode(),
            )?;
            Self::deposit_event(Event::MigrationPlanned {
                deployment,
                plan: id,
                collection,
                breaking,
            });
            Ok(())
        }

        /// Run one plan without changing anything.
        ///
        /// The call reports whether the plan applies and whether the plan
        /// carries a reverse.
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::dry_run())]
        pub fn dry_run(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            plan: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::run_dry_run(&deployment, plan, &who)
        }

        /// Apply one plan.
        ///
        /// The call changes the registry and rewrites every stored record.
        /// A plan that breaks an existing integration runs only after the
        /// electorate approved it at the structural class threshold.
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::apply())]
        pub fn apply(origin: OriginFor<T>, deployment: DeploymentId, plan: u64) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::run_apply(&deployment, plan, &who)
        }

        /// Confirm the shape one applied plan produced.
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::verify())]
        pub fn verify(origin: OriginFor<T>, deployment: DeploymentId, plan: u64) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::run_verify(&deployment, plan, &who)
        }

        /// Undo one applied plan inside its rollback window.
        #[pallet::call_index(5)]
        #[pallet::weight(T::WeightInfo::roll_back())]
        pub fn roll_back(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            plan: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::run_rollback(&deployment, plan, &who)
        }

        /// Move the deployment to another platform version.
        ///
        /// The Cerulea operator role reaches this call through the
        /// platform version trait.
        #[pallet::call_index(6)]
        #[pallet::weight(T::WeightInfo::set_platform_version())]
        pub fn set_platform_version(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            version: u32,
            reason: BoundedVec<u8, ConstU32<256>>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let sentence = alloc::string::String::from_utf8_lossy(reason.as_slice()).into_owned();
            <Self as PlatformVersions>::set_platform_version(&deployment, version, &who, &sentence)
        }
    }
}

impl<T: Config> Pallet<T> {
    /// The wall clock time of the Blockchain, in milliseconds.
    pub fn now_millis() -> u64 {
        T::Time::now().as_millis() as u64
    }

    /// Append one record to the audit log.
    pub(crate) fn record(
        deployment: &DeploymentId,
        event: &str,
        class: WorkClass,
        actor: &T::AccountId,
        reason: &str,
        payload: Vec<u8>,
    ) -> Result<u64, DispatchError> {
        T::Audit::record(
            AuditRequest::new(*deployment, MODULE_NAME, event, actor, reason)
                .classed(class)
                .carrying(payload),
        )
    }

    /// One stored plan.
    pub fn plan(
        deployment: &DeploymentId,
        id: u64,
    ) -> Option<PlanRecord<T::AccountId, BlockNumberFor<T>, T::MaxPlanBytes>> {
        Plans::<T>::get(deployment, id)
    }

    /// The decoded form of one stored plan.
    pub fn plan_body(deployment: &DeploymentId, id: u64) -> Result<MigrationPlan, DispatchError> {
        let held = Self::plan(deployment, id).ok_or(Error::<T>::PlanUnknown)?;
        MigrationPlan::decode(&mut held.plan.as_slice())
            .map_err(|_| Error::<T>::PlanUnreadable.into())
    }

    /// The collection one plan changes.
    pub fn plan_collection(
        deployment: &DeploymentId,
        id: u64,
    ) -> Result<CollectionId, DispatchError> {
        let held = Self::plan(deployment, id).ok_or(Error::<T>::PlanUnknown)?;
        Ok(CollectionId::new(&name_text(&held.collection)))
    }

    /// The applied migration log of one deployment, in order.
    pub fn applied_log(deployment: &DeploymentId) -> Vec<AppliedEntry<BlockNumberFor<T>>> {
        let count = AppliedCount::<T>::get(deployment);
        let mut found = Vec::new();
        for position in 0..count {
            if let Some(entry) = AppliedLog::<T>::get(deployment, position) {
                found.push(entry);
            }
        }
        found
    }

    /// Append one entry to the applied migration log.
    pub(crate) fn note_applied(
        deployment: &DeploymentId,
        plan: u64,
        collection: BoundedName,
        to_version: u32,
        status: PlanStatus,
    ) {
        let position = AppliedCount::<T>::get(deployment);
        AppliedLog::<T>::insert(
            deployment,
            position,
            AppliedEntry {
                plan,
                collection,
                to_version,
                status,
                at_millis: Self::now_millis(),
                block: frame_system::Pallet::<T>::block_number(),
            },
        );
        AppliedCount::<T>::insert(deployment, position.saturating_add(1));
    }
}

impl<T: Config> PlatformVersions for Pallet<T> {
    type AccountId = T::AccountId;

    fn platform_version(deployment: &DeploymentId) -> u32 {
        PlatformVersion::<T>::get(deployment)
    }

    fn set_platform_version(
        deployment: &DeploymentId,
        version: u32,
        actor: &Self::AccountId,
        reason: &str,
    ) -> DispatchResult {
        PlatformVersion::<T>::insert(deployment, version);
        Self::record(
            deployment,
            "platform_version_set",
            WorkClass::Structural,
            actor,
            reason,
            version.encode(),
        )?;
        Self::deposit_event(Event::PlatformVersionSet {
            deployment: *deployment,
            version,
        });
        Ok(())
    }
}

/// The proposal kind a breaking migration reaches approval through.
pub const BREAKING_PROPOSAL: ProposalKind = ProposalKind::SchemaChange;
