//! Dry run, apply, verify and roll back within the window.
//!
//! The four steps run in order. A dry run reports whether the plan applies
//! and whether the plan carries a reverse. An apply changes the registry
//! and rewrites every stored record inside one transaction. A verify
//! confirms the shape the plan produced. A rollback undoes the plan while
//! the window stays open.
//!
//! A plan that breaks an existing integration reaches the apply call only
//! after the electorate approved the change at the structural class
//! threshold, which is 80 percent. A plan that keeps every existing
//! integration working applies without a proposal.

use frame_support::pallet_prelude::*;
use pallet_cerulea_audit::WorkClass;
use pallet_cerulea_governance::ApprovalGate;
use pallet_cerulea_schema::primitives::DeploymentId;

use crate::{Config, Error, Event, Pallet, PlanStatus, Plans, SchemaMigrations, BREAKING_PROPOSAL};
use codec::Encode;

impl<T: Config> Pallet<T> {
    /// Run one plan without changing anything.
    pub(crate) fn run_dry_run(
        deployment: &DeploymentId,
        id: u64,
        actor: &T::AccountId,
    ) -> DispatchResult {
        let held = Self::plan(deployment, id).ok_or(Error::<T>::PlanUnknown)?;
        ensure!(
            matches!(held.status, PlanStatus::Planned | PlanStatus::Checked),
            Error::<T>::PlanClosed
        );
        let body = Self::plan_body(deployment, id)?;
        let outcome =
            T::Schema::dry_run(deployment, &body).map_err(|_| Error::<T>::DryRunFailed)?;

        Plans::<T>::try_mutate(deployment, id, |slot| -> DispatchResult {
            let plan = slot.as_mut().ok_or(Error::<T>::PlanUnknown)?;
            plan.status = PlanStatus::Checked;
            plan.reversible = outcome.reversible;
            Ok(())
        })?;

        Self::record(
            deployment,
            "migration_dry_run",
            WorkClass::Structural,
            actor,
            "The steward ran one migration plan without changing anything.",
            (id, outcome.reversible, outcome.rewrites_records).encode(),
        )?;
        Self::deposit_event(Event::DryRunPassed {
            deployment: *deployment,
            plan: id,
            reversible: outcome.reversible,
        });
        Ok(())
    }

    /// Apply one plan.
    pub(crate) fn run_apply(
        deployment: &DeploymentId,
        id: u64,
        actor: &T::AccountId,
    ) -> DispatchResult {
        let held = Self::plan(deployment, id).ok_or(Error::<T>::PlanUnknown)?;
        ensure!(
            held.status == PlanStatus::Checked,
            Error::<T>::DryRunPending
        );
        if held.breaking {
            ensure!(
                T::Approvals::is_executed(deployment, BREAKING_PROPOSAL, held.plan.as_slice()),
                Error::<T>::ApprovalMissing
            );
        }

        let body = Self::plan_body(deployment, id)?;
        let records = T::Schema::apply(actor, deployment, &body)?;
        let now = Self::now_millis();
        let deadline = now.saturating_add(held.rollback_window_millis);

        Plans::<T>::try_mutate(deployment, id, |slot| -> DispatchResult {
            let plan = slot.as_mut().ok_or(Error::<T>::PlanUnknown)?;
            plan.status = PlanStatus::Applied;
            plan.applied_at = Some(now);
            plan.applied_block = Some(frame_system::Pallet::<T>::block_number());
            plan.rollback_deadline = Some(deadline);
            plan.records_rewritten = records;
            Ok(())
        })?;
        Self::note_applied(
            deployment,
            id,
            held.collection.clone(),
            held.to_version,
            PlanStatus::Applied,
        );

        Self::record(
            deployment,
            "migration_applied",
            WorkClass::Structural,
            actor,
            "The steward applied one migration to a collection.",
            (id, held.to_version, records).encode(),
        )?;
        Self::deposit_event(Event::MigrationApplied {
            deployment: *deployment,
            plan: id,
            to_version: held.to_version,
            records,
        });
        Ok(())
    }

    /// Confirm the shape one applied plan produced.
    ///
    /// The check reads the registry again. The live version of the
    /// collection matches the version the plan produced, and the registry
    /// still holds the collection.
    pub(crate) fn run_verify(
        deployment: &DeploymentId,
        id: u64,
        actor: &T::AccountId,
    ) -> DispatchResult {
        let held = Self::plan(deployment, id).ok_or(Error::<T>::PlanUnknown)?;
        ensure!(held.status == PlanStatus::Applied, Error::<T>::PlanClosed);
        let collection = Self::plan_collection(deployment, id)?;

        ensure!(
            T::Schema::has_collection(deployment, &collection),
            Error::<T>::VerificationFailed
        );
        let live = T::Schema::collection_version(deployment, &collection)
            .ok_or(Error::<T>::VerificationFailed)?;
        ensure!(live == held.to_version, Error::<T>::VerificationFailed);

        Plans::<T>::try_mutate(deployment, id, |slot| -> DispatchResult {
            let plan = slot.as_mut().ok_or(Error::<T>::PlanUnknown)?;
            plan.status = PlanStatus::Verified;
            Ok(())
        })?;
        Self::note_applied(
            deployment,
            id,
            held.collection.clone(),
            held.to_version,
            PlanStatus::Verified,
        );

        Self::record(
            deployment,
            "migration_verified",
            WorkClass::Structural,
            actor,
            "The steward confirmed the shape one migration produced.",
            (id, live).encode(),
        )?;
        Self::deposit_event(Event::MigrationVerified {
            deployment: *deployment,
            plan: id,
        });
        Ok(())
    }

    /// Undo one applied plan inside its rollback window.
    pub(crate) fn run_rollback(
        deployment: &DeploymentId,
        id: u64,
        actor: &T::AccountId,
    ) -> DispatchResult {
        let held = Self::plan(deployment, id).ok_or(Error::<T>::PlanUnknown)?;
        ensure!(
            matches!(held.status, PlanStatus::Applied | PlanStatus::Verified),
            Error::<T>::PlanClosed
        );
        ensure!(held.reversible, Error::<T>::Irreversible);
        let now = Self::now_millis();
        ensure!(
            now <= held.rollback_deadline.unwrap_or(now),
            Error::<T>::WindowClosed
        );

        let body = Self::plan_body(deployment, id)?;
        let records = T::Schema::revert(actor, deployment, &body)?;

        Plans::<T>::try_mutate(deployment, id, |slot| -> DispatchResult {
            let plan = slot.as_mut().ok_or(Error::<T>::PlanUnknown)?;
            plan.status = PlanStatus::RolledBack;
            plan.records_rewritten = records;
            Ok(())
        })?;
        Self::note_applied(
            deployment,
            id,
            held.collection.clone(),
            held.from_version,
            PlanStatus::RolledBack,
        );

        Self::record(
            deployment,
            "migration_rolled_back",
            WorkClass::Structural,
            actor,
            "The steward undid one migration inside its rollback window.",
            (id, held.from_version, records).encode(),
        )?;
        Self::deposit_event(Event::MigrationRolledBack {
            deployment: *deployment,
            plan: id,
            records,
        });
        Ok(())
    }

    /// Report whether the rollback window of one plan stays open.
    pub fn rollback_available(deployment: &DeploymentId, id: u64) -> bool {
        match Self::plan(deployment, id) {
            Some(held) => {
                matches!(held.status, PlanStatus::Applied | PlanStatus::Verified)
                    && held.reversible
                    && Self::now_millis() <= held.rollback_deadline.unwrap_or(0)
            }
            None => false,
        }
    }
}
