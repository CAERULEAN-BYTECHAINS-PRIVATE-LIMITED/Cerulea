//! The Cerulea relayer module.
//!
//! The relayer module keeps the end-user experience free of gas, wallets
//! and fees. The user authorises an action. The relayer submits and pays.
//! The Blockchain records the original authoriser as the actor.
//!
//! Four operations drive the module. Submit on behalf. Check quota.
//! Throttle. Report usage.
//!
//! The module holds sponsor accounts per deployment, a quota per tenant
//! and rate limits per account and per action. Metering continues
//! internally for quota enforcement while the user encounters no fee.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod submit;
pub mod weights;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub use pallet::*;
pub use submit::Authorisation;
pub use weights::WeightInfo;

use alloc::vec::Vec;
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use frame_support::{pallet_prelude::*, traits::UnixTime};
use frame_system::pallet_prelude::*;
use pallet_cerulea_audit::{AuditRequest, AuditSink, WorkClass};
use pallet_cerulea_identity::IdentityRegistry;
use pallet_cerulea_schema::primitives::{name_of, name_text, BoundedName, DeploymentId};
use pallet_cerulea_tier::EntitlementGate;

/// The name the module carries in the audit log.
pub const MODULE_NAME: &str = "relayer";

/// The feature name a deployment holds to relay a submission.
pub const RELAY_FEATURE: &str = "relayed_submission";

/// One sponsor account of one deployment.
///
/// The sponsor carries the cost of the submissions it relays. The user
/// encounters no fee at any point.
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
pub struct Sponsor {
    /// The units the sponsor carries in total.
    pub budget_units: u64,
    /// The units the sponsor already spent.
    pub spent_units: u64,
    /// A sponsor left inactive relays nothing.
    pub active: bool,
}

impl Sponsor {
    /// The units the sponsor has left.
    pub fn remaining(&self) -> u64 {
        self.budget_units.saturating_sub(self.spent_units)
    }
}

/// The quota one tenant carries in one period.
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
pub struct TenantQuota {
    /// The span of one period, in milliseconds.
    pub period_millis: u64,
    /// The units the tenant spends in one period.
    pub units_per_period: u64,
    /// The units the tenant spent in the present period.
    pub used_units: u64,
    /// The wall clock time the present period started at.
    pub period_start: u64,
}

impl TenantQuota {
    /// The units the tenant has left in the present period.
    pub fn remaining(&self) -> u64 {
        self.units_per_period.saturating_sub(self.used_units)
    }
}

/// One rate limit.
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
pub struct RateLimit {
    /// The span of one window, in milliseconds.
    pub window_millis: u64,
    /// The count of submissions one window admits.
    pub max_count: u32,
}

/// The submissions counted inside one window.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    Encode,
    Decode,
    DecodeWithMemTracking,
    TypeInfo,
    MaxEncodedLen,
)]
pub struct RateWindow {
    /// The wall clock time the present window started at.
    pub window_start: u64,
    /// The count of submissions inside the present window.
    pub count: u32,
}

/// One relayed submission.
#[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo, MaxEncodedLen,
)]
pub struct SubmissionRecord<AccountId, BlockNumber> {
    /// The account the Blockchain records as the actor.
    ///
    /// The actor is the account that authorised the action. The relayer
    /// account never stands in for the actor.
    pub actor: AccountId,
    /// The account that submitted and paid.
    pub relayer: AccountId,
    /// The action the authorisation names.
    pub action: BoundedName,
    /// The content hash of the payload the authorisation covers.
    pub payload_hash: [u8; 32],
    /// The number the authorisation carries.
    pub nonce: u64,
    /// The units the submission metered.
    pub units: u64,
    /// The wall clock time the submission landed at.
    pub at_millis: u64,
    /// The block the submission landed in.
    pub block: BlockNumber,
}

/// The read a dependant module performs over the relayed submissions.
pub trait RelayedActor {
    /// The account type the runtime carries.
    type AccountId;

    /// The account the Blockchain records as the actor of one submission.
    fn actor_of(deployment: &DeploymentId, submission: u64) -> Option<Self::AccountId>;

    /// The units the tenant has left in the present period.
    fn quota_remaining(deployment: &DeploymentId) -> u64;

    /// Report whether one account submits through the relayer now.
    fn admits(deployment: &DeploymentId, actor: &Self::AccountId, action: &str) -> bool;
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    /// The settings the runtime supplies to the relayer module.
    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// The event type of the runtime.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// The weight figures of the calls.
        type WeightInfo: WeightInfo;

        /// The registry the module reads accounts from.
        type Identities: IdentityRegistry<AccountId = Self::AccountId>;

        /// The gate that answers whether the deployment holds the feature.
        type Entitlements: EntitlementGate<AccountId = Self::AccountId>;

        /// The log the module records its work in.
        type Audit: AuditSink<AccountId = Self::AccountId>;

        /// The clock every window and every quota period reads.
        type Time: UnixTime;

        /// The units one relayed submission meters.
        #[pallet::constant]
        type UnitsPerSubmission: Get<u64>;

        /// The largest span an authorisation stays valid for, in
        /// milliseconds.
        #[pallet::constant]
        type MaxAuthorisationSpan: Get<u64>;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// Every sponsor account of every deployment.
    #[pallet::storage]
    pub type Sponsors<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        DeploymentId,
        Blake2_128Concat,
        T::AccountId,
        Sponsor,
        OptionQuery,
    >;

    /// The quota every tenant carries.
    #[pallet::storage]
    pub type Quotas<T: Config> =
        StorageMap<_, Blake2_128Concat, DeploymentId, TenantQuota, OptionQuery>;

    /// The rate limit every account carries.
    #[pallet::storage]
    pub type AccountLimit<T: Config> =
        StorageMap<_, Blake2_128Concat, DeploymentId, RateLimit, OptionQuery>;

    /// The rate limit every action carries.
    #[pallet::storage]
    pub type ActionLimit<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        DeploymentId,
        Blake2_128Concat,
        BoundedName,
        RateLimit,
        OptionQuery,
    >;

    /// The submissions every account made inside its present window.
    #[pallet::storage]
    pub type AccountWindow<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        DeploymentId,
        Blake2_128Concat,
        T::AccountId,
        RateWindow,
        ValueQuery,
    >;

    /// The submissions every account made against every action inside its
    /// present window.
    #[pallet::storage]
    pub type ActionWindow<T: Config> = StorageNMap<
        _,
        (
            NMapKey<Blake2_128Concat, DeploymentId>,
            NMapKey<Blake2_128Concat, T::AccountId>,
            NMapKey<Blake2_128Concat, BoundedName>,
        ),
        RateWindow,
        ValueQuery,
    >;

    /// The public key every account signs its authorisations with.
    #[pallet::storage]
    pub type SigningKeys<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        DeploymentId,
        Blake2_128Concat,
        T::AccountId,
        [u8; 32],
        OptionQuery,
    >;

    /// The numbers every account already used on an authorisation.
    #[pallet::storage]
    pub type UsedNonces<T: Config> = StorageNMap<
        _,
        (
            NMapKey<Blake2_128Concat, DeploymentId>,
            NMapKey<Blake2_128Concat, T::AccountId>,
            NMapKey<Twox64Concat, u64>,
        ),
        (),
        OptionQuery,
    >;

    /// Every relayed submission.
    #[pallet::storage]
    pub type Submissions<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        DeploymentId,
        Twox64Concat,
        u64,
        SubmissionRecord<T::AccountId, BlockNumberFor<T>>,
        OptionQuery,
    >;

    /// The identifier the next submission takes.
    #[pallet::storage]
    pub type NextSubmission<T: Config> =
        StorageMap<_, Blake2_128Concat, DeploymentId, u64, ValueQuery>;

    /// The accounts held back until a moment.
    #[pallet::storage]
    pub type Throttled<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        DeploymentId,
        Blake2_128Concat,
        T::AccountId,
        u64,
        OptionQuery,
    >;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A sponsor account entered the registry.
        SponsorRegistered {
            /// The deployment.
            deployment: DeploymentId,
            /// The sponsor.
            sponsor: T::AccountId,
            /// The units the sponsor carries.
            budget_units: u64,
        },
        /// The quota of a tenant changed.
        QuotaSet {
            /// The deployment.
            deployment: DeploymentId,
            /// The units the tenant spends in one period.
            units_per_period: u64,
        },
        /// A rate limit changed.
        RateLimitSet {
            /// The deployment.
            deployment: DeploymentId,
            /// The action the limit covers. An empty name covers every
            /// action of one account.
            action: BoundedName,
            /// The count of submissions one window admits.
            max_count: u32,
        },
        /// An account registered the key it signs authorisations with.
        SigningKeyRegistered {
            /// The deployment.
            deployment: DeploymentId,
            /// The account.
            account: T::AccountId,
        },
        /// A relayed submission landed.
        ///
        /// The actor is the account that authorised the action.
        SubmittedOnBehalf {
            /// The deployment.
            deployment: DeploymentId,
            /// The submission.
            submission: u64,
            /// The account the Blockchain records as the actor.
            actor: T::AccountId,
            /// The account that submitted and paid.
            relayer: T::AccountId,
            /// The action the authorisation names.
            action: BoundedName,
        },
        /// An account entered the throttle list.
        AccountThrottled {
            /// The deployment.
            deployment: DeploymentId,
            /// The account.
            account: T::AccountId,
            /// The wall clock time the throttle lifts at.
            until: u64,
        },
        /// An account left the throttle list.
        ThrottleLifted {
            /// The deployment.
            deployment: DeploymentId,
            /// The account.
            account: T::AccountId,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// The name runs past 128 bytes. Shorten the action name.
        NameTooLong,
        /// The deployment holds no sponsor for that account. Register the
        /// sponsor before relaying a submission.
        SponsorUnknown,
        /// The sponsor is inactive. Activate the sponsor before relaying a
        /// submission.
        SponsorInactive,
        /// The sponsor spent its whole budget. Raise the budget of the
        /// sponsor.
        SponsorExhausted,
        /// The tenant spent its whole quota for the period. Wait for the
        /// next period, or raise the quota of the tenant.
        QuotaExhausted,
        /// The deployment holds no quota. Set the quota of the tenant
        /// before relaying a submission.
        QuotaMissing,
        /// The account reached its rate limit for the window. Wait for the
        /// next window.
        AccountRateLimited,
        /// The account reached the rate limit of that action for the
        /// window. Wait for the next window.
        ActionRateLimited,
        /// The account is held back until a stated moment. Wait for the
        /// throttle to lift.
        AccountThrottled,
        /// The account registered no signing key. Register the key the
        /// account signs authorisations with.
        SigningKeyMissing,
        /// The signature over the authorisation failed the check. Sign the
        /// authorisation with the registered key.
        SignatureInvalid,
        /// The authorisation names another deployment. Submit the
        /// authorisation to the deployment it names.
        DeploymentMismatch,
        /// The authorisation passed its moment. Ask the user to authorise
        /// the action again.
        AuthorisationExpired,
        /// The authorisation stays valid past the largest span. Shorten the
        /// span the authorisation names.
        AuthorisationTooLong,
        /// The authorisation number already ran. Ask the user to authorise
        /// the action with a fresh number.
        NonceUsed,
        /// The account registered with no identity. Register the account in
        /// the identity module first.
        ActorUnregistered,
        /// The deployment holds no licence for relayed submission. Add the
        /// feature to the subscription and install the new licence.
        FeatureUnlicensed,
        /// The deployment reached its last submission identifier. Start a
        /// new deployment.
        RegisterFull,
    }

        /// The call differs from the call the authorisation names. Sign the
        /// content hash of the exact call the relayer submits.
        CallMismatch,
    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Register one sponsor account.
        ///
        /// The sponsor carries the cost of the submissions it relays.
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::register_sponsor())]
        pub fn register_sponsor(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            sponsor: T::AccountId,
            budget_units: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let spent = Sponsors::<T>::get(deployment, &sponsor)
                .map(|held| held.spent_units)
                .unwrap_or(0);
            Sponsors::<T>::insert(
                deployment,
                &sponsor,
                Sponsor {
                    budget_units,
                    spent_units: spent,
                    active: true,
                },
            );

            Self::record(
                &deployment,
                "sponsor_registered",
                WorkClass::Structural,
                &who,
                "The steward registered one sponsor account for relayed submission.",
                (sponsor.clone(), budget_units).encode(),
            )?;
            Self::deposit_event(Event::SponsorRegistered {
                deployment,
                sponsor,
                budget_units,
            });
            Ok(())
        }

        /// Set the quota one tenant carries.
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::set_quota())]
        pub fn set_quota(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            period_millis: u64,
            units_per_period: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let now = Self::now_millis();
            let start = Quotas::<T>::get(deployment)
                .map(|held| held.period_start)
                .unwrap_or(now);
            let used = Quotas::<T>::get(deployment)
                .map(|held| held.used_units)
                .unwrap_or(0);
            Quotas::<T>::insert(
                deployment,
                TenantQuota {
                    period_millis,
                    units_per_period,
                    used_units: used,
                    period_start: start,
                },
            );

            Self::record(
                &deployment,
                "quota_set",
                WorkClass::Structural,
                &who,
                "The steward set the relayed submission quota of the tenant.",
                (period_millis, units_per_period).encode(),
            )?;
            Self::deposit_event(Event::QuotaSet {
                deployment,
                units_per_period,
            });
            Ok(())
        }

        /// Set the rate limit one account carries.
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::set_account_limit())]
        pub fn set_account_limit(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            window_millis: u64,
            max_count: u32,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            AccountLimit::<T>::insert(
                deployment,
                RateLimit {
                    window_millis,
                    max_count,
                },
            );
            Self::record(
                &deployment,
                "account_limit_set",
                WorkClass::Structural,
                &who,
                "The steward set the rate limit one account carries.",
                (window_millis, max_count).encode(),
            )?;
            Self::deposit_event(Event::RateLimitSet {
                deployment,
                action: BoundedName::default(),
                max_count,
            });
            Ok(())
        }

        /// Set the rate limit one action carries.
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::set_action_limit())]
        pub fn set_action_limit(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            action: BoundedName,
            window_millis: u64,
            max_count: u32,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ActionLimit::<T>::insert(
                deployment,
                &action,
                RateLimit {
                    window_millis,
                    max_count,
                },
            );
            Self::record(
                &deployment,
                "action_limit_set",
                WorkClass::Structural,
                &who,
                "The steward set the rate limit one action carries.",
                (action.clone(), window_millis, max_count).encode(),
            )?;
            Self::deposit_event(Event::RateLimitSet {
                deployment,
                action,
                max_count,
            });
            Ok(())
        }

        /// Register the key one account signs its authorisations with.
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::register_signing_key())]
        pub fn register_signing_key(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            key: [u8; 32],
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(
                T::Identities::is_registered(&who),
                Error::<T>::ActorUnregistered
            );
            SigningKeys::<T>::insert(deployment, &who, key);
            Self::record(
                &deployment,
                "signing_key_registered",
                WorkClass::Structural,
                &who,
                "The account registered the key it signs authorisations with.",
                key.to_vec(),
            )?;
            Self::deposit_event(Event::SigningKeyRegistered {
                deployment,
                account: who,
            });
            Ok(())
        }

        /// Submit one action on behalf of the account that authorised it.
        ///
        /// The Blockchain records the authoriser as the actor. The relayer
        /// carries the cost. The user encounters no fee.
        #[pallet::call_index(5)]
        #[pallet::weight(
            T::WeightInfo::submit_on_behalf()
                .saturating_add(
                    frame_support::dispatch::GetDispatchInfo::get_dispatch_info(&**call)
                        .total_weight()
                )
        )]
        ///
        /// The call carries the inner call the account authorised. The
        /// authorisation names the content hash of that call. Once the
        /// signature holds, the module dispatches the inner call with the
        /// authoriser as the origin, so permission resolves against the
        /// authoriser and the history names the authoriser.
        pub fn submit_on_behalf(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            authorisation: Authorisation<T::AccountId>,
            signature: [u8; 64],
        ) -> DispatchResult {
            let relayer = ensure_signed(origin)?;
            call: alloc::boxed::Box<<T as frame_system::Config>::RuntimeCall>,
            Self::relay(&deployment, &authorisation, &signature, &relayer)?;
            // The authorisation binds the exact inner call.
            ensure!(
                authorisation.payload_hash == sp_io::hashing::blake2_256(&call.encode()),
                Error::<T>::CallMismatch
            );
            let inner: <T as frame_system::Config>::RuntimeOrigin =
                frame_system::RawOrigin::Signed(authorisation.actor.clone()).into();
            sp_runtime::traits::Dispatchable::dispatch(*call, inner)
                .map(|_| ())
                .map_err(|failure| failure.error)
        }

        /// Hold one account back until a stated moment.
        #[pallet::call_index(6)]
        #[pallet::weight(T::WeightInfo::throttle())]
        pub fn throttle(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            account: T::AccountId,
            until: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Throttled::<T>::insert(deployment, &account, until);
            Self::record(
                &deployment,
                "account_throttled",
                WorkClass::Routine,
                &who,
                "The steward held one account back from relayed submission.",
                (account.clone(), until).encode(),
            )?;
            Self::deposit_event(Event::AccountThrottled {
                deployment,
                account,
                until,
            });
            Ok(())
        }

        /// Let one account submit again.
        #[pallet::call_index(7)]
        #[pallet::weight(T::WeightInfo::lift_throttle())]
        pub fn lift_throttle(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            account: T::AccountId,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Throttled::<T>::remove(deployment, &account);
            Self::record(
                &deployment,
                "throttle_lifted",
                WorkClass::Routine,
                &who,
                "The steward let one account submit through the relayer again.",
                account.encode(),
            )?;
            Self::deposit_event(Event::ThrottleLifted {
                deployment,
                account,
            });
            Ok(())
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

    /// The quota of one tenant, rolled forward to the present period.
    pub fn quota_state(deployment: &DeploymentId) -> Option<TenantQuota> {
        let mut held = Quotas::<T>::get(deployment)?;
        let now = Self::now_millis();
        if held.period_millis > 0 && now >= held.period_start.saturating_add(held.period_millis) {
            let elapsed = now.saturating_sub(held.period_start);
            let periods = elapsed / held.period_millis;
            held.period_start = held
                .period_start
                .saturating_add(periods.saturating_mul(held.period_millis));
            held.used_units = 0;
        }
        Some(held)
    }

    /// The units the tenant has left in the present period.
    pub fn quota_remaining(deployment: &DeploymentId) -> u64 {
        Self::quota_state(deployment)
            .map(|held| held.remaining())
            .unwrap_or(0)
    }

    /// The units one sponsor has left.
    pub fn sponsor_remaining(deployment: &DeploymentId, sponsor: &T::AccountId) -> u64 {
        Sponsors::<T>::get(deployment, sponsor)
            .map(|held| held.remaining())
            .unwrap_or(0)
    }

    /// One stored submission.
    pub fn submission(
        deployment: &DeploymentId,
        id: u64,
    ) -> Option<SubmissionRecord<T::AccountId, BlockNumberFor<T>>> {
        Submissions::<T>::get(deployment, id)
    }

    /// The count of submissions one account made in its present window.
    pub fn account_window(deployment: &DeploymentId, account: &T::AccountId) -> RateWindow {
        Self::rolled(
            AccountWindow::<T>::get(deployment, account),
            AccountLimit::<T>::get(deployment),
        )
    }

    /// The count of submissions one account made against one action in its
    /// present window.
    pub fn action_window(
        deployment: &DeploymentId,
        account: &T::AccountId,
        action: &BoundedName,
    ) -> RateWindow {
        Self::rolled(
            ActionWindow::<T>::get((*deployment, account.clone(), action.clone())),
            ActionLimit::<T>::get(deployment, action),
        )
    }

    /// Roll one window forward when its span already passed.
    pub(crate) fn rolled(window: RateWindow, limit: Option<RateLimit>) -> RateWindow {
        let held = match limit {
            Some(found) => found,
            None => return window,
        };
        if held.window_millis == 0 {
            return window;
        }
        let now = Self::now_millis();
        if now >= window.window_start.saturating_add(held.window_millis) {
            return RateWindow {
                window_start: now,
                count: 0,
            };
        }
        window
    }

    /// Report whether one account submits through the relayer now.
    pub fn admits_now(deployment: &DeploymentId, actor: &T::AccountId, action: &str) -> bool {
        let name = match name_of(action) {
            Ok(held) => held,
            Err(_) => return false,
        };
        if let Some(until) = Throttled::<T>::get(deployment, actor) {
            if Self::now_millis() < until {
                return false;
            }
        }
        if Self::quota_remaining(deployment) < T::UnitsPerSubmission::get() {
            return false;
        }
        if let Some(limit) = AccountLimit::<T>::get(deployment) {
            if Self::account_window(deployment, actor).count >= limit.max_count {
                return false;
            }
        }
        if let Some(limit) = ActionLimit::<T>::get(deployment, &name) {
            if Self::action_window(deployment, actor, &name).count >= limit.max_count {
                return false;
            }
        }
        true
    }

    /// The usage report of one deployment.
    ///
    /// The report carries the units spent in the present period, the units
    /// left and the count of submissions the deployment relayed.
    pub fn usage_report(deployment: &DeploymentId) -> (u64, u64, u64) {
        let quota = Self::quota_state(deployment);
        (
            quota.map(|held| held.used_units).unwrap_or(0),
            quota.map(|held| held.remaining()).unwrap_or(0),
            NextSubmission::<T>::get(deployment),
        )
    }
}

impl<T: Config> RelayedActor for Pallet<T> {
    type AccountId = T::AccountId;

    fn actor_of(deployment: &DeploymentId, submission: u64) -> Option<Self::AccountId> {
        Submissions::<T>::get(deployment, submission).map(|held| held.actor)
    }

    fn quota_remaining(deployment: &DeploymentId) -> u64 {
        Self::quota_remaining(deployment)
    }

    fn admits(deployment: &DeploymentId, actor: &Self::AccountId, action: &str) -> bool {
        Self::admits_now(deployment, actor, action)
    }
}

/// The action name one submission record carries, as text.
pub fn action_text(record: &BoundedName) -> alloc::string::String {
    name_text(record)
}
