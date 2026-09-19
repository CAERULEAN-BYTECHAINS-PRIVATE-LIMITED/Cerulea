//! The Cerulea tier module.
//!
//! The tier module gates entitlement on the Blockchain as well as in
//! Cerulea APIs. The module holds the deployment tier, the enabled feature
//! set, the seat count, the add-on registry and the licence attestation
//! with its expiry.
//!
//! Five operations drive the gate. Verify licence. Check entitlement.
//! Record usage. Suspend. Resume.
//!
//! The control plane signs a licence attestation with an ed25519 key. The
//! deployment holds the public key of the control plane and checks every
//! attestation against it. A deployment therefore verifies its own licence
//! without reaching outside the Blockchain.
//!
//! A lapsed licence stops block production. Every existing record stays
//! readable and exportable throughout. Customer data is never withheld.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod suspend;
pub mod weights;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub use pallet::*;
pub use suspend::ProductionGate;
pub use weights::WeightInfo;

use alloc::vec::Vec;
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use frame_support::{pallet_prelude::*, traits::UnixTime};
use frame_system::pallet_prelude::*;
use pallet_cerulea_audit::{AuditRequest, AuditSink, WorkClass};
use pallet_cerulea_schema::primitives::{name_of, BoundedName, DeploymentId};

/// The name the module carries in the audit log.
pub const MODULE_NAME: &str = "tier";

/// The domain separator mixed into every licence attestation.
///
/// The separator keeps a licence signature apart from every other
/// signature the control plane produces.
pub const LICENCE_DOMAIN: &[u8] = b"cerulea.licence.v1";

/// The largest reason sentence the module stores, in bytes.
pub const MAX_REASON_BYTES: u32 = 256;

/// A bounded plain sentence.
pub type Sentence = BoundedVec<u8, ConstU32<MAX_REASON_BYTES>>;

/// Which Cerulea application the deployment runs.
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
pub enum Application {
    /// Independent builders and small teams.
    Dapps,
    /// Companies with compliance duties.
    Enterprise,
    /// Public bodies and agencies.
    Government,
}

impl Application {
    /// The count of tier levels the application carries.
    ///
    /// The Dapps application carries three levels. The Enterprise
    /// application carries three levels. The Government application
    /// carries one level.
    pub fn levels(&self) -> u8 {
        match self {
            Application::Dapps => 3,
            Application::Enterprise => 3,
            Application::Government => 1,
        }
    }

    /// A word naming the application for the interface layer.
    pub fn label(&self) -> &'static str {
        match self {
            Application::Dapps => "dapps",
            Application::Enterprise => "enterprise",
            Application::Government => "government",
        }
    }
}

/// The tier one deployment runs under.
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
pub struct Tier {
    /// The application the deployment runs.
    pub application: Application,
    /// The level inside that application, counting from one.
    pub level: u8,
}

impl Tier {
    /// Report whether the level sits inside the application.
    pub fn is_valid(&self) -> bool {
        self.level >= 1 && self.level <= self.application.levels()
    }
}

/// The licence attestation the control plane signs.
///
/// The fields below form the message the signature covers. The canonical
/// encoding of the message is what a verifier hashes.
#[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo, MaxEncodedLen,
)]
#[scale_info(skip_type_params(MaxFeatures, MaxAddons))]
pub struct Licence<MaxFeatures, MaxAddons>
where
    MaxFeatures: Get<u32> + 'static,
    MaxAddons: Get<u32> + 'static,
{
    /// The deployment the licence covers.
    pub deployment: DeploymentId,
    /// The tier the deployment runs under.
    pub tier: Tier,
    /// The features the licence enables.
    pub features: BoundedVec<BoundedName, MaxFeatures>,
    /// The seats the licence covers.
    pub seats: u32,
    /// The add-ons the licence enables.
    pub addons: BoundedVec<BoundedName, MaxAddons>,
    /// The wall clock time the licence was issued at.
    pub issued_at: u64,
    /// The wall clock time the licence lapses at.
    pub expires_at: u64,
    /// The number the control plane gives this licence.
    ///
    /// A later licence for one deployment carries a higher number, so a
    /// replayed earlier licence is refused.
    pub sequence: u64,
}

impl<MaxFeatures, MaxAddons> Licence<MaxFeatures, MaxAddons>
where
    MaxFeatures: Get<u32> + 'static,
    MaxAddons: Get<u32> + 'static,
{
    /// The canonical message the signature covers.
    pub fn message(&self) -> Vec<u8> {
        (LICENCE_DOMAIN, self).encode()
    }

    /// Report whether the licence holds at one moment.
    pub fn holds_at(&self, now: u64) -> bool {
        now < self.expires_at
    }

    /// Report whether the licence enables one feature.
    pub fn enables(&self, feature: &BoundedName) -> bool {
        self.features.iter().any(|held| held == feature)
    }

    /// Report whether the licence enables one add-on.
    pub fn carries_addon(&self, addon: &BoundedName) -> bool {
        self.addons.iter().any(|held| held == addon)
    }
}

/// Where one deployment stands against its licence.
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
pub enum LicenceState {
    /// The licence holds and block production runs.
    Live,
    /// The licence lapsed and block production stopped.
    Lapsed,
    /// The operator suspended the deployment.
    Suspended,
}

/// The entitlement read a dependant module performs.
///
/// The relayer module, the operator module and Cerulea APIs all reach the
/// gate through this trait.
pub trait EntitlementGate {
    /// The account type the runtime carries.
    type AccountId;

    /// Report whether the deployment holds a licence that covers now.
    fn licence_holds(deployment: &DeploymentId) -> bool;

    /// Report whether the licence enables one feature.
    fn entitled(deployment: &DeploymentId, feature: &str) -> bool;

    /// The seat count the licence covers.
    fn seats(deployment: &DeploymentId) -> u32;

    /// The tier the deployment runs under.
    fn tier(deployment: &DeploymentId) -> Option<Tier>;

    /// Record one unit of usage against one feature.
    fn record_usage(
        deployment: &DeploymentId,
        feature: &str,
        units: u64,
        actor: &Self::AccountId,
    ) -> DispatchResult;
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    /// The licence one deployment holds, with the bounds the runtime
    /// declares.
    pub type LicenceOf<T> = Licence<<T as Config>::MaxFeatures, <T as Config>::MaxAddons>;

    /// The settings the runtime supplies to the tier module.
    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// The event type of the runtime.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// The weight figures of the calls.
        type WeightInfo: WeightInfo;

        /// The log the module records its work in.
        type Audit: AuditSink<AccountId = Self::AccountId>;

        /// The clock every expiry reads.
        type Time: UnixTime;

        /// The largest number of features one licence enables.
        #[pallet::constant]
        type MaxFeatures: Get<u32>;

        /// The largest number of add-ons one licence enables.
        #[pallet::constant]
        type MaxAddons: Get<u32>;

        /// The largest number of deployments the block hook watches.
        #[pallet::constant]
        type MaxWatchedDeployments: Get<u32>;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// The public key of the control plane, per deployment.
    #[pallet::storage]
    pub type ControlPlaneKey<T: Config> =
        StorageMap<_, Blake2_128Concat, DeploymentId, [u8; 32], OptionQuery>;

    /// The licence every deployment holds.
    #[pallet::storage]
    pub type Licences<T: Config> =
        StorageMap<_, Blake2_128Concat, DeploymentId, LicenceOf<T>, OptionQuery>;

    /// The signature over the licence every deployment holds.
    #[pallet::storage]
    pub type Signatures<T: Config> =
        StorageMap<_, Blake2_128Concat, DeploymentId, [u8; 64], OptionQuery>;

    /// Where every deployment stands against its licence.
    #[pallet::storage]
    pub type States<T: Config> =
        StorageMap<_, Blake2_128Concat, DeploymentId, LicenceState, OptionQuery>;

    /// The reason block production stopped, per deployment.
    #[pallet::storage]
    pub type SuspensionReason<T: Config> =
        StorageMap<_, Blake2_128Concat, DeploymentId, Sentence, OptionQuery>;

    /// The seats every deployment uses now.
    #[pallet::storage]
    pub type SeatsUsed<T: Config> = StorageMap<_, Blake2_128Concat, DeploymentId, u32, ValueQuery>;

    /// The usage every deployment recorded against each feature.
    #[pallet::storage]
    pub type Usage<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        DeploymentId,
        Blake2_128Concat,
        BoundedName,
        u64,
        ValueQuery,
    >;

    /// The deployments the block hook watches for a lapse.
    #[pallet::storage]
    pub type Watched<T: Config> =
        StorageValue<_, BoundedVec<DeploymentId, T::MaxWatchedDeployments>, ValueQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A deployment took the public key of its control plane.
        ControlPlaneKeySet {
            /// The deployment.
            deployment: DeploymentId,
        },
        /// A licence attestation landed and verified.
        LicenceInstalled {
            /// The deployment.
            deployment: DeploymentId,
            /// The tier the licence carries.
            tier: Tier,
            /// The wall clock time the licence lapses at.
            expires_at: u64,
        },
        /// A verification pass confirmed the licence.
        LicenceVerified {
            /// The deployment.
            deployment: DeploymentId,
            /// The wall clock time the licence lapses at.
            expires_at: u64,
        },
        /// A licence lapsed and block production stopped.
        LicenceLapsed {
            /// The deployment.
            deployment: DeploymentId,
        },
        /// Block production stopped.
        ProductionSuspended {
            /// The deployment.
            deployment: DeploymentId,
        },
        /// Block production started again.
        ProductionResumed {
            /// The deployment.
            deployment: DeploymentId,
        },
        /// A usage record landed.
        UsageRecorded {
            /// The deployment.
            deployment: DeploymentId,
            /// The feature.
            feature: BoundedName,
            /// The units the deployment used in total.
            units: u64,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// The name runs past 128 bytes. Shorten the feature name.
        NameTooLong,
        /// The deployment holds no control plane key. Install the key
        /// before installing a licence.
        ControlPlaneKeyMissing,
        /// The deployment already holds a control plane key. Install a new
        /// licence rather than a second key.
        ControlPlaneKeyPresent,
        /// The attestation failed to decode. Submit the canonical encoding
        /// the control plane produces.
        AttestationUnreadable,
        /// The signature over the attestation failed the check. Ask the
        /// control plane to issue the licence again.
        SignatureInvalid,
        /// The attestation names another deployment. Install the licence
        /// issued for this deployment.
        DeploymentMismatch,
        /// The attestation carries a sequence number at or below the
        /// stored licence. Install the newest licence the control plane
        /// issued.
        SequenceReplayed,
        /// The tier names a level the application lacks. Ask the control
        /// plane to issue the licence at a level the application carries.
        TierInvalid,
        /// The deployment holds no licence. Install the licence the control
        /// plane issued.
        LicenceMissing,
        /// The licence lapsed. Renew the subscription and install the new
        /// licence.
        LicenceLapsed,
        /// The licence enables no feature of that name. Add the feature to
        /// the subscription and install the new licence.
        FeatureUnlicensed,
        /// The deployment uses every seat the licence covers. Add seats to
        /// the subscription and install the new licence.
        SeatsExhausted,
        /// The module watches its full count of deployments. Raise the
        /// limit in the runtime.
        WatchListFull,
        /// Block production already runs. Suspend the deployment before
        /// resuming it.
        NotSuspended,
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        /// Check every watched deployment against the expiry of its
        /// licence.
        ///
        /// A deployment whose licence lapsed stops producing blocks. Every
        /// existing record stays readable and exportable throughout.
        fn on_initialize(_now: BlockNumberFor<T>) -> Weight {
            Self::sweep_expiries()
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Install the public key of the control plane.
        ///
        /// Every licence attestation of this deployment is checked against
        /// this key.
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::set_control_plane_key())]
        pub fn set_control_plane_key(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            key: [u8; 32],
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(
                ControlPlaneKey::<T>::get(deployment).is_none(),
                Error::<T>::ControlPlaneKeyPresent
            );
            ControlPlaneKey::<T>::insert(deployment, key);
            Watched::<T>::try_mutate(|held| -> DispatchResult {
                if held.contains(&deployment) {
                    return Ok(());
                }
                held.try_push(deployment)
                    .map_err(|_| Error::<T>::WatchListFull)?;
                Ok(())
            })?;

            Self::record(
                &deployment,
                "control_plane_key_set",
                WorkClass::Constitutional,
                &who,
                "The deployment took the public key of its control plane.",
                key.to_vec(),
            )?;
            Self::deposit_event(Event::ControlPlaneKeySet { deployment });
            Ok(())
        }

        /// Install one licence attestation.
        ///
        /// The call checks the signature against the control plane key,
        /// checks the deployment the attestation names and checks the
        /// sequence number against the stored licence.
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::install_licence())]
        pub fn install_licence(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            attestation: BoundedVec<u8, ConstU32<8192>>,
            signature: [u8; 64],
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let licence = LicenceOf::<T>::decode(&mut attestation.as_slice())
                .map_err(|_| Error::<T>::AttestationUnreadable)?;
            ensure!(
                licence.deployment == deployment,
                Error::<T>::DeploymentMismatch
            );
            ensure!(licence.tier.is_valid(), Error::<T>::TierInvalid);
            if let Some(held) = Licences::<T>::get(deployment) {
                ensure!(
                    licence.sequence > held.sequence,
                    Error::<T>::SequenceReplayed
                );
            }
            Self::check_signature(&deployment, &licence, &signature)?;

            let tier = licence.tier;
            let expires_at = licence.expires_at;
            Licences::<T>::insert(deployment, &licence);
            Signatures::<T>::insert(deployment, signature);
            States::<T>::insert(deployment, LicenceState::Live);
            SuspensionReason::<T>::remove(deployment);

            Self::record(
                &deployment,
                "licence_installed",
                WorkClass::Constitutional,
                &who,
                "The deployment installed the licence the control plane issued.",
                (tier, expires_at, licence.sequence).encode(),
            )?;
            Self::deposit_event(Event::LicenceInstalled {
                deployment,
                tier,
                expires_at,
            });
            Ok(())
        }

        /// Verify the stored licence again.
        ///
        /// The call checks the signature and the expiry. A lapsed licence
        /// stops block production.
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::verify_licence())]
        pub fn verify_licence(origin: OriginFor<T>, deployment: DeploymentId) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let licence = Licences::<T>::get(deployment).ok_or(Error::<T>::LicenceMissing)?;
            let signature = Signatures::<T>::get(deployment).ok_or(Error::<T>::LicenceMissing)?;
            Self::check_signature(&deployment, &licence, &signature)?;

            let now = Self::now_millis();
            if licence.holds_at(now) {
                if States::<T>::get(deployment) == Some(LicenceState::Lapsed) {
                    States::<T>::insert(deployment, LicenceState::Live);
                }
                Self::record(
                    &deployment,
                    "licence_verified",
                    WorkClass::Routine,
                    &who,
                    "The deployment checked its licence and the licence holds.",
                    licence.expires_at.encode(),
                )?;
                Self::deposit_event(Event::LicenceVerified {
                    deployment,
                    expires_at: licence.expires_at,
                });
                return Ok(());
            }

            Self::mark_lapsed(&deployment, &who)?;
            Err(Error::<T>::LicenceLapsed.into())
        }

        /// Record one unit of usage against one feature.
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::record_usage())]
        pub fn record_usage(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            feature: BoundedName,
            units: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::note_usage(&deployment, &feature, units, &who)
        }

        /// Take one seat of the licence.
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::take_seat())]
        pub fn take_seat(origin: OriginFor<T>, deployment: DeploymentId) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let licence = Licences::<T>::get(deployment).ok_or(Error::<T>::LicenceMissing)?;
            ensure!(
                licence.holds_at(Self::now_millis()),
                Error::<T>::LicenceLapsed
            );
            SeatsUsed::<T>::try_mutate(deployment, |held| -> DispatchResult {
                ensure!(*held < licence.seats, Error::<T>::SeatsExhausted);
                *held = held.saturating_add(1);
                Ok(())
            })?;
            Self::record(
                &deployment,
                "seat_taken",
                WorkClass::Routine,
                &who,
                "One account took a seat of the licence.",
                SeatsUsed::<T>::get(deployment).encode(),
            )?;
            Ok(())
        }

        /// Stop block production for one deployment.
        #[pallet::call_index(5)]
        #[pallet::weight(T::WeightInfo::suspend())]
        pub fn suspend(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            reason: Sentence,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::stop_production(&deployment, &who, &reason, LicenceState::Suspended)
        }

        /// Start block production again.
        #[pallet::call_index(6)]
        #[pallet::weight(T::WeightInfo::resume())]
        pub fn resume(origin: OriginFor<T>, deployment: DeploymentId) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::start_production(&deployment, &who)
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

    /// Check one signature against the control plane key of a deployment.
    pub(crate) fn check_signature(
        deployment: &DeploymentId,
        licence: &LicenceOf<T>,
        signature: &[u8; 64],
    ) -> DispatchResult {
        let key =
            ControlPlaneKey::<T>::get(deployment).ok_or(Error::<T>::ControlPlaneKeyMissing)?;
        let public = sp_core::ed25519::Public::from_raw(key);
        let held = sp_core::ed25519::Signature::from_raw(*signature);
        ensure!(
            sp_io::crypto::ed25519_verify(&held, &licence.message(), &public),
            Error::<T>::SignatureInvalid
        );
        Ok(())
    }

    /// The licence one deployment holds.
    pub fn licence(deployment: &DeploymentId) -> Option<LicenceOf<T>> {
        Licences::<T>::get(deployment)
    }

    /// The usage one deployment recorded against one feature.
    pub fn usage_of(deployment: &DeploymentId, feature: &str) -> u64 {
        match name_of(feature) {
            Ok(name) => Usage::<T>::get(deployment, name),
            Err(_) => 0,
        }
    }

    /// Record one unit of usage against one feature.
    pub(crate) fn note_usage(
        deployment: &DeploymentId,
        feature: &BoundedName,
        units: u64,
        actor: &T::AccountId,
    ) -> DispatchResult {
        let licence = Licences::<T>::get(deployment).ok_or(Error::<T>::LicenceMissing)?;
        ensure!(
            licence.holds_at(Self::now_millis()),
            Error::<T>::LicenceLapsed
        );
        ensure!(licence.enables(feature), Error::<T>::FeatureUnlicensed);

        let total = Usage::<T>::mutate(deployment, feature, |held| {
            *held = held.saturating_add(units);
            *held
        });
        Self::record(
            deployment,
            "usage_recorded",
            WorkClass::Routine,
            actor,
            "The deployment recorded usage against one licensed feature.",
            (feature.clone(), total).encode(),
        )?;
        Self::deposit_event(Event::UsageRecorded {
            deployment: *deployment,
            feature: feature.clone(),
            units: total,
        });
        Ok(())
    }
}

impl<T: Config> EntitlementGate for Pallet<T> {
    type AccountId = T::AccountId;

    fn licence_holds(deployment: &DeploymentId) -> bool {
        match Licences::<T>::get(deployment) {
            Some(held) => held.holds_at(Self::now_millis()),
            None => false,
        }
    }

    fn entitled(deployment: &DeploymentId, feature: &str) -> bool {
        let name = match name_of(feature) {
            Ok(held) => held,
            Err(_) => return false,
        };
        match Licences::<T>::get(deployment) {
            Some(held) => held.holds_at(Self::now_millis()) && held.enables(&name),
            None => false,
        }
    }

    fn seats(deployment: &DeploymentId) -> u32 {
        Licences::<T>::get(deployment)
            .map(|held| held.seats)
            .unwrap_or(0)
    }

    fn tier(deployment: &DeploymentId) -> Option<Tier> {
        Licences::<T>::get(deployment).map(|held| held.tier)
    }

    fn record_usage(
        deployment: &DeploymentId,
        feature: &str,
        units: u64,
        actor: &Self::AccountId,
    ) -> DispatchResult {
        let name = name_of(feature).map_err(|_| Error::<T>::NameTooLong)?;
        Self::note_usage(deployment, &name, units, actor)
    }
}
