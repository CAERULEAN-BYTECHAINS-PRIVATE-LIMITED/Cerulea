//! Submission on behalf with the original actor recorded.
//!
//! The user authorises an action by signing a short statement with the key
//! the account registered. The relayer submits that statement and carries
//! the cost. The Blockchain records the authoriser as the actor, so the
//! relayer never stands in for the person who acted.
//!
//! Metering continues internally for quota enforcement while the user
//! encounters no fee. Every submission spends units from the sponsor
//! budget and from the tenant quota, and counts against the rate limit of
//! the account and the rate limit of the action.

use alloc::vec::Vec;
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use frame_support::pallet_prelude::*;
use pallet_cerulea_audit::WorkClass;
use pallet_cerulea_schema::primitives::{BoundedName, DeploymentId};
use pallet_cerulea_tier::EntitlementGate;

use crate::{
    AccountLimit, AccountWindow, ActionLimit, ActionWindow, Config, Error, Event, NextSubmission,
    Pallet, Quotas, SigningKeys, Sponsors, SubmissionRecord, Submissions, Throttled, UsedNonces,
    RELAY_FEATURE,
};

/// The domain separator mixed into every authorisation.
///
/// The separator keeps an authorisation signature apart from every other
/// signature the account produces.
pub const AUTHORISATION_DOMAIN: &[u8] = b"cerulea.relayer.v1";

/// The statement one user signs to authorise an action.
#[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo, MaxEncodedLen,
)]
pub struct Authorisation<AccountId> {
    /// The deployment the action runs in.
    pub deployment: DeploymentId,
    /// The account that authorises the action.
    pub actor: AccountId,
    /// The action the user authorises.
    pub action: BoundedName,
    /// The content hash of the payload the action carries.
    pub payload_hash: [u8; 32],
    /// The wall clock time the authorisation stays valid until.
    pub valid_until: u64,
    /// The number this authorisation carries. A number runs once.
    pub nonce: u64,
}

impl<AccountId: Encode> Authorisation<AccountId> {
    /// The canonical message the signature covers.
    pub fn message(&self) -> Vec<u8> {
        (AUTHORISATION_DOMAIN, self).encode()
    }
}

impl<T: Config> Pallet<T> {
    /// Relay one authorised action.
    pub(crate) fn relay(
        deployment: &DeploymentId,
        authorisation: &Authorisation<T::AccountId>,
        signature: &[u8; 64],
        relayer: &T::AccountId,
    ) -> DispatchResult {
        let now = Self::now_millis();
        ensure!(
            &authorisation.deployment == deployment,
            Error::<T>::DeploymentMismatch
        );
        ensure!(
            now <= authorisation.valid_until,
            Error::<T>::AuthorisationExpired
        );
        ensure!(
            authorisation.valid_until.saturating_sub(now) <= T::MaxAuthorisationSpan::get(),
            Error::<T>::AuthorisationTooLong
        );
        ensure!(
            T::Identities::is_registered(&authorisation.actor),
            Error::<T>::ActorUnregistered
        );
        ensure!(
            T::Entitlements::entitled(deployment, RELAY_FEATURE),
            Error::<T>::FeatureUnlicensed
        );

        // The signature proves the authoriser asked for this action.
        let key = SigningKeys::<T>::get(deployment, &authorisation.actor)
            .ok_or(Error::<T>::SigningKeyMissing)?;
        let public = sp_core::ed25519::Public::from_raw(key);
        let held = sp_core::ed25519::Signature::from_raw(*signature);
        ensure!(
            sp_io::crypto::ed25519_verify(&held, &authorisation.message(), &public),
            Error::<T>::SignatureInvalid
        );

        // One number runs once, so a relayer replays nothing.
        ensure!(
            UsedNonces::<T>::get((
                *deployment,
                authorisation.actor.clone(),
                authorisation.nonce
            ))
            .is_none(),
            Error::<T>::NonceUsed
        );

        if let Some(until) = Throttled::<T>::get(deployment, &authorisation.actor) {
            ensure!(now >= until, Error::<T>::AccountThrottled);
        }

        let units = T::UnitsPerSubmission::get();
        Self::spend_sponsor(deployment, relayer, units)?;
        Self::spend_quota(deployment, units)?;
        Self::count_account(deployment, &authorisation.actor)?;
        Self::count_action(deployment, &authorisation.actor, &authorisation.action)?;

        let id = NextSubmission::<T>::get(deployment);
        let next = id.checked_add(1).ok_or(Error::<T>::RegisterFull)?;
        Submissions::<T>::insert(
            deployment,
            id,
            SubmissionRecord {
                actor: authorisation.actor.clone(),
                relayer: relayer.clone(),
                action: authorisation.action.clone(),
                payload_hash: authorisation.payload_hash,
                nonce: authorisation.nonce,
                units,
                at_millis: now,
                block: frame_system::Pallet::<T>::block_number(),
            },
        );
        NextSubmission::<T>::insert(deployment, next);
        UsedNonces::<T>::insert(
            (
                *deployment,
                authorisation.actor.clone(),
                authorisation.nonce,
            ),
            (),
        );

        // The metering continues inside the deployment. The user
        // encounters no fee at any point.
        T::Entitlements::record_usage(deployment, RELAY_FEATURE, units, &authorisation.actor)?;

        // The audit record names the authoriser as the actor.
        Self::record(
            deployment,
            "submitted_on_behalf",
            WorkClass::Routine,
            &authorisation.actor,
            "The relayer submitted one action the account authorised.",
            (
                id,
                authorisation.action.clone(),
                relayer.clone(),
                authorisation.payload_hash,
            )
                .encode(),
        )?;
        Self::deposit_event(Event::SubmittedOnBehalf {
            deployment: *deployment,
            submission: id,
            actor: authorisation.actor.clone(),
            relayer: relayer.clone(),
            action: authorisation.action.clone(),
        });
        Ok(())
    }

    /// Spend units from the budget of one sponsor.
    fn spend_sponsor(
        deployment: &DeploymentId,
        sponsor: &T::AccountId,
        units: u64,
    ) -> DispatchResult {
        Sponsors::<T>::try_mutate(deployment, sponsor, |slot| -> DispatchResult {
            let held = slot.as_mut().ok_or(Error::<T>::SponsorUnknown)?;
            ensure!(held.active, Error::<T>::SponsorInactive);
            ensure!(held.remaining() >= units, Error::<T>::SponsorExhausted);
            held.spent_units = held.spent_units.saturating_add(units);
            Ok(())
        })
    }

    /// Spend units from the quota of the tenant.
    fn spend_quota(deployment: &DeploymentId, units: u64) -> DispatchResult {
        let mut held = Self::quota_state(deployment).ok_or(Error::<T>::QuotaMissing)?;
        ensure!(held.remaining() >= units, Error::<T>::QuotaExhausted);
        held.used_units = held.used_units.saturating_add(units);
        Quotas::<T>::insert(deployment, held);
        Ok(())
    }

    /// Count one submission against the rate limit of one account.
    fn count_account(deployment: &DeploymentId, actor: &T::AccountId) -> DispatchResult {
        let limit = match AccountLimit::<T>::get(deployment) {
            Some(found) => found,
            None => return Ok(()),
        };
        let mut window = Self::account_window(deployment, actor);
        ensure!(
            window.count < limit.max_count,
            Error::<T>::AccountRateLimited
        );
        window.count = window.count.saturating_add(1);
        if window.window_start == 0 {
            window.window_start = Self::now_millis();
        }
        AccountWindow::<T>::insert(deployment, actor, window);
        Ok(())
    }

    /// Count one submission against the rate limit of one action.
    fn count_action(
        deployment: &DeploymentId,
        actor: &T::AccountId,
        action: &BoundedName,
    ) -> DispatchResult {
        let limit = match ActionLimit::<T>::get(deployment, action) {
            Some(found) => found,
            None => return Ok(()),
        };
        let mut window = Self::action_window(deployment, actor, action);
        ensure!(
            window.count < limit.max_count,
            Error::<T>::ActionRateLimited
        );
        window.count = window.count.saturating_add(1);
        if window.window_start == 0 {
            window.window_start = Self::now_millis();
        }
        ActionWindow::<T>::insert((*deployment, actor.clone(), action.clone()), window);
        Ok(())
    }
}

use pallet_cerulea_identity::IdentityRegistry;
