//! The Cerulea audit module.
//!
//! The audit module holds the typed event log of every deployment. Cerulea
//! Explorer, Cerulea Dashboard and every external subscriber read the log.
//! Every other platform module writes into the log through the
//! [`AuditSink`] trait.
//!
//! Four operations drive the log. Emit. Query. Subscribe. Export.
//!
//! The log is append only. An entry never changes after it lands. A
//! correction appends a further entry naming the entry it corrects, so the
//! history stays whole and the reader sees both the original statement and
//! the correction.
//!
//! Every entry carries a running digest. The digest of an entry folds the
//! digest of the entry before it together with the canonical encoding of
//! the entry. A reader recomputes the chain and detects any removal or any
//! change.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod contract;
pub mod export;
pub mod weights;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub use contract::{AuditRequest, AuditSink, LogRecord, WorkClass};
pub use pallet::*;
pub use weights::WeightInfo;

use alloc::vec::Vec;
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use frame_support::{pallet_prelude::*, traits::UnixTime};
use frame_system::pallet_prelude::*;
use pallet_cerulea_schema::primitives::{name_of, BoundedName, DeploymentId};

use crate::contract::{Sentence, MAX_SUMMARY_BYTES};

/// The domain separator mixed into the first digest of every log.
///
/// The separator keeps audit digests apart from every other digest the
/// Blockchain computes.
pub const AUDIT_DOMAIN: &[u8] = b"cerulea.audit.v1";

/// One registered event definition.
#[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo, MaxEncodedLen,
)]
#[scale_info(skip_type_params(BlockNumber))]
pub struct EventDefinition<BlockNumber> {
    /// The class of work the event records.
    pub class: WorkClass,
    /// One plain sentence stating what the event records.
    pub summary: BoundedVec<u8, ConstU32<MAX_SUMMARY_BYTES>>,
    /// The module that registered the definition.
    pub module: BoundedName,
    /// The block the definition landed at.
    pub registered_at: BlockNumber,
}

/// One entry of the log.
#[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo, MaxEncodedLen,
)]
#[scale_info(skip_type_params(MaxPayload))]
pub struct AuditEntry<AccountId, BlockNumber, MaxPayload: Get<u32> + 'static> {
    /// The position of the entry in the log of the deployment.
    pub sequence: u64,
    /// The event name.
    pub event: BoundedName,
    /// The module that recorded the work.
    pub module: BoundedName,
    /// The class of the work.
    pub class: WorkClass,
    /// The account the Blockchain records as the actor.
    pub actor: AccountId,
    /// The wall clock time of the entry, in milliseconds.
    pub at_millis: u64,
    /// The block the entry landed in.
    pub block: BlockNumber,
    /// One plain sentence stating why the work ran.
    pub reason: Sentence,
    /// The canonical encoding of the detail the event carries.
    pub payload: BoundedVec<u8, MaxPayload>,
    /// The entry this record corrects.
    pub corrects: Option<u64>,
    /// The running digest of the log up to and including this entry.
    pub digest: [u8; 32],
}

impl<AccountId, BlockNumber, MaxPayload> AuditEntry<AccountId, BlockNumber, MaxPayload>
where
    AccountId: Encode,
    BlockNumber: Encode,
    MaxPayload: Get<u32> + 'static,
{
    /// The canonical encoding of the body of the entry.
    ///
    /// The digest folds this encoding. The digest field itself stays out of
    /// the encoding, so a reader recomputes the chain from the bodies.
    pub fn body_bytes(&self) -> Vec<u8> {
        (
            self.sequence,
            self.event.as_slice(),
            self.module.as_slice(),
            self.class,
            &self.actor,
            self.at_millis,
            &self.block,
            self.reason.as_slice(),
            self.payload.as_slice(),
            self.corrects,
        )
            .encode()
    }
}

/// The interest one subscriber registers.
#[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo, MaxEncodedLen,
)]
#[scale_info(skip_type_params(MaxEvents))]
pub struct Subscription<MaxEvents: Get<u32> + 'static> {
    /// The events the subscriber follows. An empty list follows every
    /// event.
    pub events: BoundedVec<BoundedName, MaxEvents>,
    /// The classes the subscriber follows. An empty list follows every
    /// class.
    pub classes: BoundedVec<WorkClass, ConstU32<4>>,
    /// The sequence number the subscription starts from.
    pub since: u64,
}

impl<MaxEvents: Get<u32> + 'static> Subscription<MaxEvents> {
    /// Report whether one entry reaches this subscriber.
    pub fn follows(&self, event: &BoundedName, class: WorkClass, sequence: u64) -> bool {
        if sequence < self.since {
            return false;
        }
        let event_matches = self.events.is_empty() || self.events.iter().any(|held| held == event);
        let class_matches =
            self.classes.is_empty() || self.classes.iter().any(|held| *held == class);
        event_matches && class_matches
    }
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    /// The settings the runtime supplies to the audit module.
    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// The event type of the runtime.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// The weight figures of the calls.
        type WeightInfo: WeightInfo;

        /// The clock the log stamps every entry with.
        type Time: UnixTime;

        /// The largest payload one entry carries, in bytes.
        #[pallet::constant]
        type MaxPayloadBytes: Get<u32>;

        /// The largest number of event definitions one deployment holds.
        #[pallet::constant]
        type MaxEventDefinitions: Get<u32>;

        /// The largest number of events one subscription names.
        #[pallet::constant]
        type MaxSubscribedEvents: Get<u32>;

        /// The largest number of entries one query or one export returns.
        #[pallet::constant]
        type MaxReadBatch: Get<u32>;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// The registered event definitions of every deployment.
    #[pallet::storage]
    pub type Definitions<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        DeploymentId,
        Blake2_128Concat,
        BoundedName,
        EventDefinition<BlockNumberFor<T>>,
        OptionQuery,
    >;

    /// The registered event names of every deployment, in registration
    /// order.
    #[pallet::storage]
    pub type DefinitionNames<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        DeploymentId,
        BoundedVec<BoundedName, T::MaxEventDefinitions>,
        ValueQuery,
    >;

    /// Every entry of every log, keyed by deployment and sequence number.
    #[pallet::storage]
    pub type Entries<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        DeploymentId,
        Twox64Concat,
        u64,
        AuditEntry<T::AccountId, BlockNumberFor<T>, T::MaxPayloadBytes>,
        OptionQuery,
    >;

    /// The sequence number the next entry of every log takes.
    #[pallet::storage]
    pub type NextSequence<T: Config> =
        StorageMap<_, Blake2_128Concat, DeploymentId, u64, ValueQuery>;

    /// The running digest of every log.
    #[pallet::storage]
    pub type LogDigest<T: Config> =
        StorageMap<_, Blake2_128Concat, DeploymentId, [u8; 32], OptionQuery>;

    /// The entries of every actor, for a query by actor.
    #[pallet::storage]
    pub type ActorIndex<T: Config> = StorageNMap<
        _,
        (
            NMapKey<Blake2_128Concat, DeploymentId>,
            NMapKey<Blake2_128Concat, T::AccountId>,
            NMapKey<Twox64Concat, u64>,
        ),
        (),
        OptionQuery,
    >;

    /// The entries of every event name, for a query by event.
    #[pallet::storage]
    pub type EventIndex<T: Config> = StorageNMap<
        _,
        (
            NMapKey<Blake2_128Concat, DeploymentId>,
            NMapKey<Blake2_128Concat, BoundedName>,
            NMapKey<Twox64Concat, u64>,
        ),
        (),
        OptionQuery,
    >;

    /// The registered subscribers of every deployment.
    #[pallet::storage]
    pub type Subscriptions<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        DeploymentId,
        Blake2_128Concat,
        T::AccountId,
        Subscription<T::MaxSubscribedEvents>,
        OptionQuery,
    >;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// An event definition entered the registry.
        EventDefined {
            /// The deployment.
            deployment: DeploymentId,
            /// The event name.
            event: BoundedName,
            /// The class of work the event records.
            class: WorkClass,
        },
        /// An entry landed in the log.
        ///
        /// A subscriber reads this runtime event and fetches the entry.
        EntryRecorded {
            /// The deployment.
            deployment: DeploymentId,
            /// The position of the entry in the log.
            sequence: u64,
            /// The event name.
            event: BoundedName,
            /// The module that recorded the work.
            module: BoundedName,
            /// The class of the work.
            class: WorkClass,
            /// The account the Blockchain records as the actor.
            actor: T::AccountId,
        },
        /// A correcting entry landed in the log.
        EntryCorrected {
            /// The deployment.
            deployment: DeploymentId,
            /// The entry the correction names.
            original: u64,
            /// The position of the correcting entry.
            correction: u64,
        },
        /// A subscriber registered its interest.
        SubscriptionOpened {
            /// The deployment.
            deployment: DeploymentId,
            /// The subscriber.
            subscriber: T::AccountId,
        },
        /// A subscriber withdrew its interest.
        SubscriptionClosed {
            /// The deployment.
            deployment: DeploymentId,
            /// The subscriber.
            subscriber: T::AccountId,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// The name runs past 128 bytes. Shorten the event name.
        NameTooLong,
        /// The registry lacks that event. Define the event before emitting
        /// it.
        EventUnknown,
        /// The registry already holds that event under another class. Emit
        /// the event under its registered class.
        ClassMismatch,
        /// The deployment holds its full count of event definitions. Raise
        /// the definition limit in the runtime.
        DefinitionLimitReached,
        /// The reason runs past 256 bytes. Shorten the sentence.
        ReasonTooLong,
        /// The summary runs past 256 bytes. Shorten the sentence.
        SummaryTooLong,
        /// The payload runs past the payload limit. Shorten the payload, or
        /// raise the limit in the runtime.
        PayloadTooLarge,
        /// The log holds no entry under that sequence number. Query the log
        /// for the sequence number of the entry.
        EntryAbsent,
        /// A correction names an entry that is itself a correction. Correct
        /// the original entry.
        CorrectionChained,
        /// The deployment holds no subscription for that account. Open a
        /// subscription before closing one.
        SubscriptionAbsent,
        /// The log reached its last sequence number. Start a new deployment
        /// log.
        LogFull,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Register one event definition.
        ///
        /// A definition names the class of work the event records and one
        /// plain sentence stating what the event records.
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::define_event())]
        pub fn define_event(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            event: BoundedName,
            class: WorkClass,
            summary: BoundedVec<u8, ConstU32<MAX_SUMMARY_BYTES>>,
        ) -> DispatchResult {
            ensure_signed(origin)?;
            let module = name_of("audit").map_err(|_| Error::<T>::NameTooLong)?;
            Self::register_definition(&deployment, &event, class, summary, &module)
        }

        /// Append one entry to the log.
        ///
        /// The signed account is the actor the log records.
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::emit())]
        pub fn emit(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            event: BoundedName,
            reason: Sentence,
            payload: BoundedVec<u8, T::MaxPayloadBytes>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let definition =
                Definitions::<T>::get(deployment, &event).ok_or(Error::<T>::EventUnknown)?;
            let module = name_of("audit").map_err(|_| Error::<T>::NameTooLong)?;
            Self::append(
                &deployment,
                &event,
                &module,
                definition.class,
                &who,
                &reason,
                payload,
                None,
            )
            .map(|_| ())
        }

        /// Append a correcting entry.
        ///
        /// The log keeps the original entry unchanged. The correction names
        /// the entry it corrects and carries the new statement.
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::correct())]
        pub fn correct(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            original: u64,
            reason: Sentence,
            payload: BoundedVec<u8, T::MaxPayloadBytes>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let entry = Entries::<T>::get(deployment, original).ok_or(Error::<T>::EntryAbsent)?;
            ensure!(entry.corrects.is_none(), Error::<T>::CorrectionChained);
            let sequence = Self::append(
                &deployment,
                &entry.event,
                &entry.module,
                entry.class,
                &who,
                &reason,
                payload,
                Some(original),
            )?;
            Self::deposit_event(Event::EntryCorrected {
                deployment,
                original,
                correction: sequence,
            });
            Ok(())
        }

        /// Register the interest of one subscriber.
        ///
        /// An empty event list follows every event. An empty class list
        /// follows every class.
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::subscribe())]
        pub fn subscribe(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            events: BoundedVec<BoundedName, T::MaxSubscribedEvents>,
            classes: BoundedVec<WorkClass, ConstU32<4>>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Subscriptions::<T>::insert(
                deployment,
                &who,
                Subscription {
                    events,
                    classes,
                    since: NextSequence::<T>::get(deployment),
                },
            );
            Self::deposit_event(Event::SubscriptionOpened {
                deployment,
                subscriber: who,
            });
            Ok(())
        }

        /// Withdraw the interest of one subscriber.
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::unsubscribe())]
        pub fn unsubscribe(origin: OriginFor<T>, deployment: DeploymentId) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(
                Subscriptions::<T>::contains_key(deployment, &who),
                Error::<T>::SubscriptionAbsent
            );
            Subscriptions::<T>::remove(deployment, &who);
            Self::deposit_event(Event::SubscriptionClosed {
                deployment,
                subscriber: who,
            });
            Ok(())
        }
    }
}

impl<T: Config> Pallet<T> {
    /// Register one event definition, or check an existing one.
    ///
    /// A definition already present under the same class passes. A
    /// definition present under another class declines the call.
    pub fn register_definition(
        deployment: &DeploymentId,
        event: &BoundedName,
        class: WorkClass,
        summary: BoundedVec<u8, ConstU32<MAX_SUMMARY_BYTES>>,
        module: &BoundedName,
    ) -> DispatchResult {
        if let Some(held) = Definitions::<T>::get(deployment, event) {
            ensure!(held.class == class, Error::<T>::ClassMismatch);
            return Ok(());
        }
        DefinitionNames::<T>::try_mutate(deployment, |names| -> DispatchResult {
            names
                .try_push(event.clone())
                .map_err(|_| Error::<T>::DefinitionLimitReached)?;
            Ok(())
        })?;
        Definitions::<T>::insert(
            deployment,
            event,
            EventDefinition {
                class,
                summary,
                module: module.clone(),
                registered_at: frame_system::Pallet::<T>::block_number(),
            },
        );
        Self::deposit_event(Event::EventDefined {
            deployment: *deployment,
            event: event.clone(),
            class,
        });
        Ok(())
    }

    /// Append one entry and answer with its sequence number.
    #[allow(clippy::too_many_arguments)]
    fn append(
        deployment: &DeploymentId,
        event: &BoundedName,
        module: &BoundedName,
        class: WorkClass,
        actor: &T::AccountId,
        reason: &Sentence,
        payload: BoundedVec<u8, T::MaxPayloadBytes>,
        corrects: Option<u64>,
    ) -> Result<u64, DispatchError> {
        let sequence = NextSequence::<T>::get(deployment);
        let next = sequence.checked_add(1).ok_or(Error::<T>::LogFull)?;
        let previous = LogDigest::<T>::get(deployment)
            .unwrap_or_else(|| sp_crypto_hashing::blake2_256(&(AUDIT_DOMAIN, deployment).encode()));

        let mut entry = AuditEntry {
            sequence,
            event: event.clone(),
            module: module.clone(),
            class,
            actor: actor.clone(),
            at_millis: T::Time::now().as_millis() as u64,
            block: frame_system::Pallet::<T>::block_number(),
            reason: reason.clone(),
            payload,
            corrects,
            digest: [0u8; 32],
        };
        let mut folded = previous.to_vec();
        folded.extend_from_slice(&entry.body_bytes());
        entry.digest = sp_crypto_hashing::blake2_256(&folded);

        LogDigest::<T>::insert(deployment, entry.digest);
        ActorIndex::<T>::insert((*deployment, actor.clone(), sequence), ());
        EventIndex::<T>::insert((*deployment, event.clone(), sequence), ());
        Entries::<T>::insert(deployment, sequence, &entry);
        NextSequence::<T>::insert(deployment, next);

        Self::deposit_event(Event::EntryRecorded {
            deployment: *deployment,
            sequence,
            event: event.clone(),
            module: module.clone(),
            class,
            actor: actor.clone(),
        });
        Ok(sequence)
    }

    /// The number of entries one log holds.
    pub fn length(deployment: &DeploymentId) -> u64 {
        NextSequence::<T>::get(deployment)
    }

    /// One stored entry.
    pub fn entry(
        deployment: &DeploymentId,
        sequence: u64,
    ) -> Option<AuditEntry<T::AccountId, BlockNumberFor<T>, T::MaxPayloadBytes>> {
        Entries::<T>::get(deployment, sequence)
    }

    /// The first digest of one log.
    ///
    /// The chain starts from the domain separator folded with the
    /// deployment identifier, so two deployments never share a chain.
    pub fn genesis_digest(deployment: &DeploymentId) -> [u8; 32] {
        sp_crypto_hashing::blake2_256(&(AUDIT_DOMAIN, deployment).encode())
    }
}

impl<T: Config> AuditSink for Pallet<T> {
    type AccountId = T::AccountId;

    fn record(request: AuditRequest<'_, Self::AccountId>) -> Result<u64, DispatchError> {
        let event = name_of(request.event).map_err(|_| Error::<T>::NameTooLong)?;
        let module = name_of(request.module).map_err(|_| Error::<T>::NameTooLong)?;
        let reason: Sentence = BoundedVec::try_from(request.reason.as_bytes().to_vec())
            .map_err(|_| Error::<T>::ReasonTooLong)?;
        let payload: BoundedVec<u8, T::MaxPayloadBytes> =
            BoundedVec::try_from(request.payload).map_err(|_| Error::<T>::PayloadTooLarge)?;

        if Definitions::<T>::get(request.deployment, &event).is_none() {
            let summary: BoundedVec<u8, ConstU32<MAX_SUMMARY_BYTES>> =
                BoundedVec::try_from(request.reason.as_bytes().to_vec())
                    .map_err(|_| Error::<T>::SummaryTooLong)?;
            Self::register_definition(
                &request.deployment,
                &event,
                request.class,
                summary,
                &module,
            )?;
        }

        Self::append(
            &request.deployment,
            &event,
            &module,
            request.class,
            request.actor,
            &reason,
            payload,
            request.corrects,
        )
    }
}
