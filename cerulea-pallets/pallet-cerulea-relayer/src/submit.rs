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
    AccountLimit, AccountSponsorCaps, AccountSponsorUsage, AccountSponsorWindow, AccountWindow,
    ActionLimit, ActionWindow, CircuitBreakerWindows, CircuitBreakers, Config, Error, Event,
    NextSubmission, Pallet, Quotas, SigningKeys, SponsorReserve, Sponsors, SubmissionRecord,
    Submissions, Throttled, UsedNonces, RELAY_FEATURE,
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
        Self::check_circuit_breaker(deployment, units, now)?;
        Self::spend_account_sponsor_cap(deployment, &authorisation.actor, relayer, units, now)?;
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

    /// Check and advance the deployment-level sponsorship circuit breaker.
    fn check_circuit_breaker(
        deployment: &DeploymentId,
        units: u64,
        now: u64,
    ) -> DispatchResult {
        let Some(cb) = CircuitBreakers::<T>::get(deployment) else {
            return Ok(());
        };
        ensure!(!cb.tripped, Error::<T>::CircuitBreakerTripped);
        let mut win = CircuitBreakerWindows::<T>::get(deployment);
        if win.window_start == 0 || now.saturating_sub(win.window_start) >= cb.window_millis {
            win = crate::CircuitBreakerWindow {
                window_start: now,
                spent_units: 0,
            };
        }
        let next_spent = win.spent_units.saturating_add(units);
        ensure!(
            next_spent <= cb.max_units_per_window,
            Error::<T>::CircuitBreakerTripped
        );
        win.spent_units = next_spent;
        CircuitBreakerWindows::<T>::insert(deployment, win);
        Ok(())
    }

    /// Enforce dynamic account-level sponsorship limits, tightening by `surge_divisor`
    /// when the sponsor's pool utilization reaches 80% or more.
    fn spend_account_sponsor_cap(
        deployment: &DeploymentId,
        actor: &T::AccountId,
        relayer: &T::AccountId,
        units: u64,
        now: u64,
    ) -> DispatchResult {
        let Some(cap) = AccountSponsorCaps::<T>::get(deployment) else {
            return Ok(());
        };
        let high_utilization = Sponsors::<T>::get(deployment, relayer)
            .map(|s| {
                s.budget_units > 0
                    && s.spent_units.saturating_mul(100) >= s.budget_units.saturating_mul(80)
            })
            .unwrap_or(false);
        let effective_cap = if high_utilization {
            (cap.max_units_per_period / (cap.surge_divisor.max(1) as u64)).max(1)
        } else {
            cap.max_units_per_period
        };

        let mut usage = AccountSponsorUsage::<T>::get(deployment, actor);
        if usage.period_start == 0 || now.saturating_sub(usage.period_start) >= cap.period_millis {
            usage = AccountSponsorWindow {
                period_start: now,
                used_units: 0,
            };
        }
        let next_used = usage.used_units.saturating_add(units);
        ensure!(
            next_used <= effective_cap,
            Error::<T>::AccountSponsorLimitExceeded
        );
        usage.used_units = next_used;
        AccountSponsorUsage::<T>::insert(deployment, actor, usage);
        Ok(())
    }

    /// Spend units from the budget of one sponsor while enforcing any protected reserve floor.
    fn spend_sponsor(
        deployment: &DeploymentId,
        sponsor: &T::AccountId,
        units: u64,
    ) -> DispatchResult {
        let reserve = SponsorReserve::<T>::get(deployment, sponsor);
        let mut low_balance_remaining: Option<u64> = None;
        Sponsors::<T>::try_mutate(deployment, sponsor, |slot| -> DispatchResult {
            let held = slot.as_mut().ok_or(Error::<T>::SponsorUnknown)?;
            ensure!(held.active, Error::<T>::SponsorInactive);
            let remaining = held.remaining();
            ensure!(remaining >= units, Error::<T>::SponsorExhausted);
            let after = remaining.saturating_sub(units);
            ensure!(after >= reserve, Error::<T>::SponsorReserveBreached);
            held.spent_units = held.spent_units.saturating_add(units);
            if reserve > 0 && after <= reserve.saturating_mul(2) {
                low_balance_remaining = Some(after);
            }
            Ok(())
        })?;
        if let Some(remaining_units) = low_balance_remaining {
            Self::deposit_event(Event::SponsorLowBalance {
                deployment: *deployment,
                sponsor: sponsor.clone(),
                remaining_units,
                reserve_units: reserve,
            });
        }
        Ok(())
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
