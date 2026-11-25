//! Suspension that preserves read and export.
//!
//! A lapsed licence stops block production. The authoring engine consults
//! [`ProductionGate`] before it selects a slot committee, so a suspended
//! deployment authors no further block.
//!
//! Reading stays open throughout. Export stays open throughout. Customer
//! data is never withheld, so a customer runs a full export of data,
//! history and Blueprint while the deployment is suspended.

use frame_support::{pallet_prelude::*, weights::Weight};
use pallet_cerulea_audit::WorkClass;
use pallet_cerulea_schema::primitives::DeploymentId;

use crate::{
    Config, Error, Event, LicenceState, Licences, Pallet, Sentence, States, SuspensionReason,
    Watched, WeightInfo,
};
use codec::Encode;
use sp_runtime::DispatchError;

/// The gate the authoring engine consults before it authors a block.
///
/// The gate answers three questions. Block production. Reading. Export.
/// The first stops on a lapse. The other two stay open.
pub trait ProductionGate {
    /// Report whether the deployment authors a further block.
    fn production_permitted(deployment: &DeploymentId) -> bool;

    /// Report whether a reader reaches the records of the deployment.
    ///
    /// The answer holds through every suspension.
    fn read_permitted(deployment: &DeploymentId) -> bool;

    /// Report whether a customer runs a full export.
    ///
    /// The answer holds through every suspension.
    fn export_permitted(deployment: &DeploymentId) -> bool;

    /// One plain sentence stating why block production stopped.
    fn suspension_reason(deployment: &DeploymentId) -> Option<alloc::string::String>;
}

impl<T: Config> ProductionGate for Pallet<T> {
    fn production_permitted(deployment: &DeploymentId) -> bool {
        match States::<T>::get(deployment) {
            Some(LicenceState::Live) => Licences::<T>::get(deployment)
                .map(|held| held.holds_at(Self::now_millis()))
                .unwrap_or(false),
            Some(_) => false,
            // A deployment that installed no licence authors blocks while
            // the control plane has yet to reach it. The block hook marks
            // a lapse once a licence lands.
            None => true,
        }
    }

    fn read_permitted(_deployment: &DeploymentId) -> bool {
        true
    }

    fn export_permitted(_deployment: &DeploymentId) -> bool {
        true
    }

    fn suspension_reason(deployment: &DeploymentId) -> Option<alloc::string::String> {
        SuspensionReason::<T>::get(deployment)
            .map(|held| alloc::string::String::from_utf8_lossy(held.as_slice()).into_owned())
    }
}

impl<T: Config> Pallet<T> {
    /// Stop block production for one deployment.
    pub(crate) fn stop_production(
        deployment: &DeploymentId,
        actor: &T::AccountId,
        reason: &Sentence,
        state: LicenceState,
    ) -> DispatchResult {
        States::<T>::insert(deployment, state);
        SuspensionReason::<T>::insert(deployment, reason.clone());

        Self::record(
            deployment,
            "production_suspended",
            WorkClass::Constitutional,
            actor,
            "Block production stopped while every record stays readable and exportable.",
            reason.to_vec(),
        )?;
        Self::deposit_event(Event::ProductionSuspended {
            deployment: *deployment,
        });
        Ok(())
    }

    /// Start block production again.
    pub(crate) fn start_production(
        deployment: &DeploymentId,
        actor: &T::AccountId,
    ) -> DispatchResult {
        ensure!(
            matches!(
                States::<T>::get(deployment),
                Some(LicenceState::Suspended) | Some(LicenceState::Lapsed)
            ),
            Error::<T>::NotSuspended
        );
        let licence = Licences::<T>::get(deployment).ok_or(Error::<T>::LicenceMissing)?;
        ensure!(
            licence.holds_at(Self::now_millis()),
            Error::<T>::LicenceLapsed
        );

        States::<T>::insert(deployment, LicenceState::Live);
        SuspensionReason::<T>::remove(deployment);
        Self::record(
            deployment,
            "production_resumed",
            WorkClass::Constitutional,
            actor,
            "Block production started again under a licence that holds.",
            licence.expires_at.encode(),
        )?;
        Self::deposit_event(Event::ProductionResumed {
            deployment: *deployment,
        });
        Ok(())
    }

    /// Mark one deployment as lapsed and stop block production.
    pub(crate) fn mark_lapsed(deployment: &DeploymentId, actor: &T::AccountId) -> DispatchResult {
        let reason: Sentence = BoundedVec::truncate_from(
            b"The licence lapsed. Renew the subscription and install the new licence.".to_vec(),
        );
        Self::stop_production(deployment, actor, &reason, LicenceState::Lapsed)?;
        Self::deposit_event(Event::LicenceLapsed {
            deployment: *deployment,
        });
        Ok(())
    }

    /// Check every watched deployment against the expiry of its licence.
    ///
    /// The sweep runs over a bounded list, so one block never runs without
    /// bound.
    pub(crate) fn sweep_expiries() -> Weight {
        let watched = Watched::<T>::get();
        if watched.is_empty() {
            return Weight::zero();
        }
        let now = Self::now_millis();
        let mut checked = 0u64;
        for deployment in watched.into_iter() {
            checked = checked.saturating_add(1);
            if States::<T>::get(deployment) != Some(LicenceState::Live) {
                continue;
            }
            let lapsed = Licences::<T>::get(deployment)
                .map(|held| !held.holds_at(now))
                .unwrap_or(false);
            if lapsed {
                States::<T>::insert(deployment, LicenceState::Lapsed);
                SuspensionReason::<T>::insert(
                    deployment,
                    Sentence::truncate_from(
                        b"The licence lapsed. Renew the subscription and install the new licence."
                            .to_vec(),
                    ),
                );
                Self::deposit_event(Event::LicenceLapsed { deployment });
                Self::deposit_event(Event::ProductionSuspended { deployment });
            }
        }
        T::WeightInfo::sweep_one_deployment().saturating_mul(checked)
    }
}

/// The licence control the Cerulea operator role reaches.
///
/// The role verifies a licence and stops block production on a lapse. The
/// role reaches nothing else of the deployment through this trait.
pub trait LicenceControl {
    /// The account type the runtime carries.
    type AccountId;

    /// Report whether the licence of one deployment holds now.
    fn holds(deployment: &DeploymentId) -> bool;

    /// Check the signature and the expiry of the stored licence.
    ///
    /// The result reports whether the licence holds.
    fn verify(deployment: &DeploymentId, actor: &Self::AccountId) -> Result<bool, DispatchError>;

    /// Stop block production when the licence lapsed.
    ///
    /// The result reports whether the call stopped block production. A
    /// licence that holds leaves the deployment producing blocks.
    fn suspend_on_lapse(
        deployment: &DeploymentId,
        actor: &Self::AccountId,
    ) -> Result<bool, DispatchError>;
}

impl<T: Config> LicenceControl for Pallet<T> {
    type AccountId = T::AccountId;

    fn holds(deployment: &DeploymentId) -> bool {
        Licences::<T>::get(deployment)
            .map(|held| held.holds_at(Self::now_millis()))
            .unwrap_or(false)
    }

    fn verify(deployment: &DeploymentId, actor: &Self::AccountId) -> Result<bool, DispatchError> {
        let licence = Licences::<T>::get(deployment).ok_or(Error::<T>::LicenceMissing)?;
        let signature =
            crate::Signatures::<T>::get(deployment).ok_or(Error::<T>::LicenceMissing)?;
        Self::check_signature(deployment, &licence, &signature)?;
        let held = licence.holds_at(Self::now_millis());
        Self::record(
            deployment,
            "licence_verified",
            WorkClass::Routine,
            actor,
            "The Cerulea operator role checked the licence of the deployment.",
            (licence.expires_at, held).encode(),
        )?;
        if held {
            Self::deposit_event(Event::LicenceVerified {
                deployment: *deployment,
                expires_at: licence.expires_at,
            });
        }
        Ok(held)
    }

    fn suspend_on_lapse(
        deployment: &DeploymentId,
        actor: &Self::AccountId,
    ) -> Result<bool, DispatchError> {
        if <Self as LicenceControl>::holds(deployment) {
            return Ok(false);
        }
        Self::mark_lapsed(deployment, actor)?;
        Ok(true)
    }
}

extern crate alloc;
