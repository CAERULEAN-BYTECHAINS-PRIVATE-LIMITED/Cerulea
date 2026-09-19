//! The Cerulea oracle module.
//!
//! The oracle module carries external data into a deployment with an
//! attestation behind every value. A feed names the accounts that attest
//! and the count of attestations one value collects before the value
//! counts.
//!
//! Five operations drive a feed. Register feed. Submit value. Dispute.
//! Resolve. Read current.
//!
//! A value collects its attestations in one round. A round reaching the
//! attestation requirement opens a dispute window. A value passing the
//! window without an upheld dispute becomes the current value of the feed.
//! A value whose dispute is upheld never becomes current.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod weights;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub use pallet::*;
pub use weights::WeightInfo;

use alloc::vec::Vec;
use cerulea_blueprint::types::Value;
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use frame_support::{pallet_prelude::*, traits::UnixTime};
use frame_system::pallet_prelude::*;
use pallet_cerulea_audit::{AuditRequest, AuditSink, WorkClass};
use pallet_cerulea_schema::primitives::{BoundedName, DeploymentId};

/// The name the module carries in the audit log.
pub const MODULE_NAME: &str = "oracle";

/// The largest reason sentence one dispute carries, in bytes.
pub const MAX_REASON_BYTES: u32 = 256;

/// A bounded plain sentence.
pub type Sentence = BoundedVec<u8, ConstU32<MAX_REASON_BYTES>>;

/// Where one round of a feed stands.
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
pub enum RoundStatus {
    /// The round collects attestations.
    Collecting,
    /// The round reached its attestation requirement and serves its
    /// dispute window.
    Attested,
    /// A dispute stands open against the round.
    Disputed,
    /// The round passed its dispute window and carries the current value.
    Current,
    /// A dispute against the round was upheld. The value never counts.
    Rejected,
}

/// One registered feed.
#[derive(Clone, Debug, Encode, Decode, DecodeWithMemTracking, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(MaxAttestors))]
pub struct Feed<AccountId, MaxAttestors>
where
    MaxAttestors: Get<u32> + 'static,
{
    /// The accounts the feed accepts an attestation from.
    pub attestors: BoundedVec<AccountId, MaxAttestors>,
    /// The count of attestations one value collects before it counts.
    pub required: u32,
    /// The span a dispute stays open for, in milliseconds.
    pub dispute_window_millis: u64,
    /// The account that resolves a dispute over this feed.
    pub steward: AccountId,
    /// One plain sentence stating what the feed carries.
    pub summary: Sentence,
    /// A retired feed takes no further value.
    pub active: bool,
}

impl<AccountId: PartialEq, MaxAttestors: Get<u32> + 'static> Feed<AccountId, MaxAttestors> {
    /// Report whether one account attests for this feed.
    pub fn attests(&self, who: &AccountId) -> bool {
        self.attestors.iter().any(|held| held == who)
    }
}

/// One round of a feed.
#[derive(Clone, Debug, Encode, Decode, DecodeWithMemTracking, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(MaxAttestors, MaxValueBytes))]
pub struct Round<AccountId, BlockNumber, MaxAttestors, MaxValueBytes>
where
    MaxAttestors: Get<u32> + 'static,
    MaxValueBytes: Get<u32> + 'static,
{
    /// The canonical encoding of the submitted value.
    pub value: BoundedVec<u8, MaxValueBytes>,
    /// The content hash of the value.
    pub value_hash: [u8; 32],
    /// The accounts that attested to the value, in the order they signed.
    pub signers: BoundedVec<AccountId, MaxAttestors>,
    /// The wall clock time the first attestation landed at.
    pub opened_at: u64,
    /// The wall clock time the round reached its requirement.
    pub attested_at: Option<u64>,
    /// The wall clock time the dispute window closes at.
    pub dispute_deadline: Option<u64>,
    /// Where the round stands.
    pub status: RoundStatus,
    /// The account that raised the open dispute.
    pub disputed_by: Option<AccountId>,
    /// One plain sentence stating the dispute.
    pub dispute_reason: Option<Sentence>,
    /// The block the round opened in.
    pub opened_block: BlockNumber,
}

/// The current value of one feed.
#[derive(Clone, Debug, Encode, Decode, DecodeWithMemTracking, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(MaxValueBytes))]
pub struct CurrentValue<MaxValueBytes>
where
    MaxValueBytes: Get<u32> + 'static,
{
    /// The round the value came from.
    pub round: u64,
    /// The canonical encoding of the value.
    pub value: BoundedVec<u8, MaxValueBytes>,
    /// The wall clock time the round reached its requirement.
    pub attested_at: u64,
}

/// The read a dependant module performs over a feed.
///
/// The logic module and the workflow module read an external value
/// through this trait. Neither reads oracle storage directly.
pub trait FeedReader {
    /// Report whether the deployment holds one feed.
    fn feed_exists(deployment: &DeploymentId, feed: &str) -> bool;

    /// The current value of one feed.
    ///
    /// A feed whose newest attested round still serves its dispute window
    /// answers with the value before it.
    fn current(deployment: &DeploymentId, feed: &str) -> Option<Value>;

    /// The current value of one feed with the moment it was attested.
    fn current_at(deployment: &DeploymentId, feed: &str) -> Option<(Value, u64)>;
}

/// One round of a feed, carrying the bounds the runtime declares.
pub type RoundOf<T> = Round<
    <T as frame_system::Config>::AccountId,
    BlockNumberFor<T>,
    <T as Config>::MaxAttestorsPerFeed,
    <T as Config>::MaxValueBytes,
>;

/// The content hash of one submitted value.
pub fn value_hash(bytes: &[u8]) -> [u8; 32] {
    sp_crypto_hashing::blake2_256(&(b"cerulea.oracle.v1", bytes).encode())
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    /// The settings the runtime supplies to the oracle module.
    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// The event type of the runtime.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// The weight figures of the calls.
        type WeightInfo: WeightInfo;

        /// The log the module records its work in.
        type Audit: AuditSink<AccountId = Self::AccountId>;

        /// The clock every window and every stamp reads.
        type Time: UnixTime;

        /// The largest number of attestors one feed names.
        #[pallet::constant]
        type MaxAttestorsPerFeed: Get<u32>;

        /// The largest submitted value, in bytes.
        #[pallet::constant]
        type MaxValueBytes: Get<u32>;

        /// The largest number of feeds one deployment holds.
        #[pallet::constant]
        type MaxFeedsPerDeployment: Get<u32>;

        /// The largest dispute window, in milliseconds.
        #[pallet::constant]
        type MaxDisputeWindow: Get<u64>;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// Every registered feed.
    #[pallet::storage]
    pub type Feeds<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        DeploymentId,
        Blake2_128Concat,
        BoundedName,
        Feed<T::AccountId, T::MaxAttestorsPerFeed>,
        OptionQuery,
    >;

    /// The registered feed names of every deployment.
    #[pallet::storage]
    pub type FeedNames<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        DeploymentId,
        BoundedVec<BoundedName, T::MaxFeedsPerDeployment>,
        ValueQuery,
    >;

    /// Every round of every feed.
    #[pallet::storage]
    pub type Rounds<T: Config> = StorageNMap<
        _,
        (
            NMapKey<Blake2_128Concat, DeploymentId>,
            NMapKey<Blake2_128Concat, BoundedName>,
            NMapKey<Twox64Concat, u64>,
        ),
        Round<T::AccountId, BlockNumberFor<T>, T::MaxAttestorsPerFeed, T::MaxValueBytes>,
        OptionQuery,
    >;

    /// The identifier the next round of every feed takes.
    #[pallet::storage]
    pub type NextRound<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        DeploymentId,
        Blake2_128Concat,
        BoundedName,
        u64,
        ValueQuery,
    >;

    /// The round of every feed that collects attestations now.
    #[pallet::storage]
    pub type OpenRound<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        DeploymentId,
        Blake2_128Concat,
        BoundedName,
        u64,
        OptionQuery,
    >;

    /// The round of every feed that serves its dispute window now.
    #[pallet::storage]
    pub type PendingRound<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        DeploymentId,
        Blake2_128Concat,
        BoundedName,
        u64,
        OptionQuery,
    >;

    /// The current value of every feed.
    #[pallet::storage]
    pub type Current<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        DeploymentId,
        Blake2_128Concat,
        BoundedName,
        CurrentValue<T::MaxValueBytes>,
        OptionQuery,
    >;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A feed entered the registry.
        FeedRegistered {
            /// The deployment.
            deployment: DeploymentId,
            /// The feed.
            feed: BoundedName,
            /// The count of attestations one value collects.
            required: u32,
        },
        /// A feed left the registry.
        FeedRetired {
            /// The deployment.
            deployment: DeploymentId,
            /// The feed.
            feed: BoundedName,
        },
        /// One attestation landed.
        ValueSubmitted {
            /// The deployment.
            deployment: DeploymentId,
            /// The feed.
            feed: BoundedName,
            /// The round.
            round: u64,
            /// The account that attested.
            signer: T::AccountId,
            /// The count of attestations the round holds now.
            signatures: u32,
        },
        /// A round reached its attestation requirement.
        RoundAttested {
            /// The deployment.
            deployment: DeploymentId,
            /// The feed.
            feed: BoundedName,
            /// The round.
            round: u64,
            /// The wall clock time the dispute window closes at.
            dispute_deadline: u64,
        },
        /// A round became the current value of its feed.
        ValueSettled {
            /// The deployment.
            deployment: DeploymentId,
            /// The feed.
            feed: BoundedName,
            /// The round.
            round: u64,
        },
        /// A dispute opened against one round.
        DisputeRaised {
            /// The deployment.
            deployment: DeploymentId,
            /// The feed.
            feed: BoundedName,
            /// The round.
            round: u64,
            /// The account that raised the dispute.
            raiser: T::AccountId,
        },
        /// A dispute closed.
        DisputeResolved {
            /// The deployment.
            deployment: DeploymentId,
            /// The feed.
            feed: BoundedName,
            /// The round.
            round: u64,
            /// The steward upheld the dispute.
            upheld: bool,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// The registry already holds a feed under that name. Retire the
        /// feed before registering another under the same name.
        FeedPresent,
        /// The registry lacks that feed. Register the feed before
        /// submitting a value to it.
        FeedUnknown,
        /// The feed is retired. Register a new feed for further values.
        FeedRetired,
        /// The deployment holds its full count of feeds. Retire a feed, or
        /// raise the limit in the runtime.
        FeedLimitReached,
        /// The feed names more attestors than the runtime admits. Name
        /// fewer attestors, or raise the limit in the runtime.
        TooManyAttestors,
        /// The feed names an attestation requirement of zero. Set a
        /// requirement of one or more.
        RequirementTooLow,
        /// The feed asks for more attestations than it names attestors.
        /// Name more attestors, or lower the requirement.
        RequirementTooHigh,
        /// The window runs past the largest dispute window. Shorten the
        /// window, or raise the limit in the runtime.
        WindowTooLong,
        /// The account attests for no feed of that name. Add the account
        /// to the attestor list of the feed.
        NotAnAttestor,
        /// The account already attested to this round. Submit a value to
        /// the next round.
        AlreadyAttested,
        /// The value runs past the value size limit. Shorten the value, or
        /// raise the limit in the runtime.
        ValueTooLarge,
        /// The value failed to decode. Submit the canonical encoding the
        /// Studio produces.
        ValueUnreadable,
        /// The feed holds no round under that identifier. Read the feed
        /// for the identifier of the round.
        RoundUnknown,
        /// The round takes no dispute now. Dispute a round inside its
        /// dispute window.
        RoundNotDisputable,
        /// The round carries no open dispute. Raise a dispute before
        /// resolving one.
        NoOpenDispute,
        /// The account stewards no feed of that name. Ask the steward of
        /// the feed to resolve the dispute.
        NotTheSteward,
        /// The feed reached its last round identifier. Register a new
        /// feed.
        RegisterFull,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Register one feed.
        ///
        /// The feed names the accounts that attest and the count of
        /// attestations one value collects before the value counts.
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::register_feed())]
        pub fn register_feed(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            feed: BoundedName,
            attestors: BoundedVec<T::AccountId, T::MaxAttestorsPerFeed>,
            required: u32,
            dispute_window_millis: u64,
            summary: Sentence,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(
                Feeds::<T>::get(deployment, &feed).is_none(),
                Error::<T>::FeedPresent
            );
            ensure!(required > 0, Error::<T>::RequirementTooLow);
            ensure!(
                required as usize <= attestors.len(),
                Error::<T>::RequirementTooHigh
            );
            ensure!(
                dispute_window_millis <= T::MaxDisputeWindow::get(),
                Error::<T>::WindowTooLong
            );

            FeedNames::<T>::try_mutate(deployment, |names| -> DispatchResult {
                names
                    .try_push(feed.clone())
                    .map_err(|_| Error::<T>::FeedLimitReached)?;
                Ok(())
            })?;
            Feeds::<T>::insert(
                deployment,
                &feed,
                Feed {
                    attestors,
                    required,
                    dispute_window_millis,
                    steward: who.clone(),
                    summary,
                    active: true,
                },
            );

            Self::record(
                &deployment,
                "feed_registered",
                WorkClass::Structural,
                &who,
                "The steward registered one external data feed.",
                (feed.clone(), required).encode(),
            )?;
            Self::deposit_event(Event::FeedRegistered {
                deployment,
                feed,
                required,
            });
            Ok(())
        }

        /// Retire one feed.
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::retire_feed())]
        pub fn retire_feed(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            feed: BoundedName,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Feeds::<T>::try_mutate(deployment, &feed, |slot| -> DispatchResult {
                let held = slot.as_mut().ok_or(Error::<T>::FeedUnknown)?;
                ensure!(held.steward == who, Error::<T>::NotTheSteward);
                ensure!(held.active, Error::<T>::FeedRetired);
                held.active = false;
                Ok(())
            })?;

            Self::record(
                &deployment,
                "feed_retired",
                WorkClass::Structural,
                &who,
                "The steward retired one external data feed.",
                feed.to_vec(),
            )?;
            Self::deposit_event(Event::FeedRetired { deployment, feed });
            Ok(())
        }

        /// Submit one attested value.
        ///
        /// A value matching the open round joins that round. Any other
        /// value opens a new round.
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::submit_value())]
        pub fn submit_value(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            feed: BoundedName,
            value: BoundedVec<u8, T::MaxValueBytes>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let held = Feeds::<T>::get(deployment, &feed).ok_or(Error::<T>::FeedUnknown)?;
            ensure!(held.active, Error::<T>::FeedRetired);
            ensure!(held.attests(&who), Error::<T>::NotAnAttestor);
            Value::decode(&mut value.as_slice()).map_err(|_| Error::<T>::ValueUnreadable)?;

            Self::promote_due(&deployment, &feed)?;
            let now = Self::now_millis();
            let hash = value_hash(value.as_slice());

            let round = match Self::matching_open_round(&deployment, &feed, hash) {
                Some(found) => found,
                None => Self::open_round(&deployment, &feed, &value, hash, now)?,
            };

            let signatures = Rounds::<T>::try_mutate(
                (deployment, feed.clone(), round),
                |slot| -> Result<u32, DispatchError> {
                    let entry = slot.as_mut().ok_or(Error::<T>::RoundUnknown)?;
                    ensure!(
                        !entry.signers.iter().any(|signer| signer == &who),
                        Error::<T>::AlreadyAttested
                    );
                    entry
                        .signers
                        .try_push(who.clone())
                        .map_err(|_| Error::<T>::TooManyAttestors)?;
                    Ok(entry.signers.len() as u32)
                },
            )?;

            Self::record(
                &deployment,
                "value_submitted",
                WorkClass::Routine,
                &who,
                "An attestor submitted one external value.",
                (feed.clone(), round, hash).encode(),
            )?;
            Self::deposit_event(Event::ValueSubmitted {
                deployment,
                feed: feed.clone(),
                round,
                signer: who,
                signatures,
            });

            if signatures >= held.required {
                Self::attest_round(&deployment, &feed, round, now, held.dispute_window_millis)?;
            }
            Ok(())
        }

        /// Raise one dispute against an attested round.
        ///
        /// A dispute lands while the dispute window stays open.
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::dispute())]
        pub fn dispute(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            feed: BoundedName,
            round: u64,
            reason: Sentence,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let now = Self::now_millis();
            Rounds::<T>::try_mutate(
                (deployment, feed.clone(), round),
                |slot| -> DispatchResult {
                    let entry = slot.as_mut().ok_or(Error::<T>::RoundUnknown)?;
                    ensure!(
                        entry.status == RoundStatus::Attested,
                        Error::<T>::RoundNotDisputable
                    );
                    ensure!(
                        now <= entry.dispute_deadline.unwrap_or(now),
                        Error::<T>::RoundNotDisputable
                    );
                    entry.status = RoundStatus::Disputed;
                    entry.disputed_by = Some(who.clone());
                    entry.dispute_reason = Some(reason.clone());
                    Ok(())
                },
            )?;

            Self::record(
                &deployment,
                "dispute_raised",
                WorkClass::Routine,
                &who,
                "A reader raised one dispute against an attested value.",
                (feed.clone(), round, reason.to_vec()).encode(),
            )?;
            Self::deposit_event(Event::DisputeRaised {
                deployment,
                feed,
                round,
                raiser: who,
            });
            Ok(())
        }

        /// Resolve one open dispute.
        ///
        /// The steward of the feed upholds the dispute or sets it aside. An
        /// upheld dispute keeps the value out of the feed for good.
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::resolve())]
        pub fn resolve(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            feed: BoundedName,
            round: u64,
            uphold: bool,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let held = Feeds::<T>::get(deployment, &feed).ok_or(Error::<T>::FeedUnknown)?;
            ensure!(held.steward == who, Error::<T>::NotTheSteward);
            let now = Self::now_millis();

            Rounds::<T>::try_mutate(
                (deployment, feed.clone(), round),
                |slot| -> DispatchResult {
                    let entry = slot.as_mut().ok_or(Error::<T>::RoundUnknown)?;
                    ensure!(
                        entry.status == RoundStatus::Disputed,
                        Error::<T>::NoOpenDispute
                    );
                    if uphold {
                        entry.status = RoundStatus::Rejected;
                    } else {
                        entry.status = RoundStatus::Attested;
                        entry.dispute_deadline = Some(now);
                    }
                    Ok(())
                },
            )?;
            if uphold && PendingRound::<T>::get(deployment, &feed) == Some(round) {
                PendingRound::<T>::remove(deployment, &feed);
            }

            Self::record(
                &deployment,
                "dispute_resolved",
                WorkClass::Routine,
                &who,
                "The steward of the feed resolved one dispute.",
                (feed.clone(), round, uphold).encode(),
            )?;
            Self::deposit_event(Event::DisputeResolved {
                deployment,
                feed: feed.clone(),
                round,
                upheld: uphold,
            });
            if !uphold {
                Self::promote_due(&deployment, &feed)?;
            }
            Ok(())
        }

        /// Promote the round that passed its dispute window.
        ///
        /// Any account runs this call. The call answers with success while
        /// nothing is due, so a caller runs it freely.
        #[pallet::call_index(5)]
        #[pallet::weight(T::WeightInfo::settle())]
        pub fn settle(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            feed: BoundedName,
        ) -> DispatchResult {
            ensure_signed(origin)?;
            Self::promote_due(&deployment, &feed)
        }
    }
}

impl<T: Config> Pallet<T> {
    /// The wall clock time of the Blockchain, in milliseconds.
    pub fn now_millis() -> u64 {
        T::Time::now().as_millis() as u64
    }

    /// Append one record to the audit log.
    fn record(
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

    /// One stored round.
    pub fn round(deployment: &DeploymentId, feed: &BoundedName, round: u64) -> Option<RoundOf<T>> {
        Rounds::<T>::get((*deployment, feed.clone(), round))
    }

    /// The open round of one feed when its value matches the hash.
    fn matching_open_round(
        deployment: &DeploymentId,
        feed: &BoundedName,
        hash: [u8; 32],
    ) -> Option<u64> {
        let open = OpenRound::<T>::get(deployment, feed)?;
        let held = Self::round(deployment, feed, open)?;
        if held.status == RoundStatus::Collecting && held.value_hash == hash {
            Some(open)
        } else {
            None
        }
    }

    /// Start one round over a submitted value.
    fn open_round(
        deployment: &DeploymentId,
        feed: &BoundedName,
        value: &BoundedVec<u8, T::MaxValueBytes>,
        hash: [u8; 32],
        now: u64,
    ) -> Result<u64, DispatchError> {
        let id = NextRound::<T>::get(deployment, feed);
        let next = id.checked_add(1).ok_or(Error::<T>::RegisterFull)?;
        Rounds::<T>::insert(
            (*deployment, feed.clone(), id),
            Round {
                value: value.clone(),
                value_hash: hash,
                signers: BoundedVec::default(),
                opened_at: now,
                attested_at: None,
                dispute_deadline: None,
                status: RoundStatus::Collecting,
                disputed_by: None,
                dispute_reason: None,
                opened_block: frame_system::Pallet::<T>::block_number(),
            },
        );
        NextRound::<T>::insert(deployment, feed, next);
        OpenRound::<T>::insert(deployment, feed, id);
        Ok(id)
    }

    /// Mark one round as attested and open its dispute window.
    fn attest_round(
        deployment: &DeploymentId,
        feed: &BoundedName,
        round: u64,
        now: u64,
        window: u64,
    ) -> DispatchResult {
        let deadline = now.saturating_add(window);
        Rounds::<T>::try_mutate(
            (*deployment, feed.clone(), round),
            |slot| -> DispatchResult {
                let entry = slot.as_mut().ok_or(Error::<T>::RoundUnknown)?;
                entry.status = RoundStatus::Attested;
                entry.attested_at = Some(now);
                entry.dispute_deadline = Some(deadline);
                Ok(())
            },
        )?;
        OpenRound::<T>::remove(deployment, feed);
        PendingRound::<T>::insert(deployment, feed, round);

        Self::deposit_event(Event::RoundAttested {
            deployment: *deployment,
            feed: feed.clone(),
            round,
            dispute_deadline: deadline,
        });
        if window == 0 {
            Self::promote_due(deployment, feed)?;
        }
        Ok(())
    }

    /// Promote the pending round of one feed once its window closes.
    fn promote_due(deployment: &DeploymentId, feed: &BoundedName) -> DispatchResult {
        let round = match PendingRound::<T>::get(deployment, feed) {
            Some(found) => found,
            None => return Ok(()),
        };
        let held = match Self::round(deployment, feed, round) {
            Some(found) => found,
            None => return Ok(()),
        };
        if held.status != RoundStatus::Attested {
            return Ok(());
        }
        let now = Self::now_millis();
        if now < held.dispute_deadline.unwrap_or(now) {
            return Ok(());
        }

        Rounds::<T>::try_mutate(
            (*deployment, feed.clone(), round),
            |slot| -> DispatchResult {
                let entry = slot.as_mut().ok_or(Error::<T>::RoundUnknown)?;
                entry.status = RoundStatus::Current;
                Ok(())
            },
        )?;
        Current::<T>::insert(
            deployment,
            feed,
            CurrentValue {
                round,
                value: held.value.clone(),
                attested_at: held.attested_at.unwrap_or(now),
            },
        );
        PendingRound::<T>::remove(deployment, feed);
        Self::deposit_event(Event::ValueSettled {
            deployment: *deployment,
            feed: feed.clone(),
            round,
        });
        Ok(())
    }

    /// The current value of one feed, with the moment of its attestation.
    ///
    /// The read promotes nothing. A pending round that already passed its
    /// window counts, so a reader sees the newest settled value without a
    /// further call.
    pub fn current_value(deployment: &DeploymentId, feed: &BoundedName) -> Option<(Value, u64)> {
        let now = Self::now_millis();
        if let Some(round) = PendingRound::<T>::get(deployment, feed) {
            if let Some(held) = Self::round(deployment, feed, round) {
                if held.status == RoundStatus::Attested
                    && now >= held.dispute_deadline.unwrap_or(u64::MAX)
                {
                    let value = Value::decode(&mut held.value.as_slice()).ok()?;
                    return Some((value, held.attested_at.unwrap_or(now)));
                }
            }
        }
        let held = Current::<T>::get(deployment, feed)?;
        let value = Value::decode(&mut held.value.as_slice()).ok()?;
        Some((value, held.attested_at))
    }
}

impl<T: Config> FeedReader for Pallet<T> {
    fn feed_exists(deployment: &DeploymentId, feed: &str) -> bool {
        match pallet_cerulea_schema::primitives::name_of(feed) {
            Ok(name) => Feeds::<T>::contains_key(deployment, name),
            Err(_) => false,
        }
    }

    fn current(deployment: &DeploymentId, feed: &str) -> Option<Value> {
        Self::current_at(deployment, feed).map(|(value, _)| value)
    }

    fn current_at(deployment: &DeploymentId, feed: &str) -> Option<(Value, u64)> {
        let name = pallet_cerulea_schema::primitives::name_of(feed).ok()?;
        Self::current_value(deployment, &name)
    }
}
