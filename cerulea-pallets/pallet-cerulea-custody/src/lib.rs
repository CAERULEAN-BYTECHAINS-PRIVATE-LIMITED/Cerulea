//! # The Custody Engine
//!
//! Cerulea verifies continuously that every validator holds the complete
//! history and serves that history on demand. The mechanism runs automatically
//! for as long as the network produces blocks. No person is involved at any
//! point.
//!
//! Five steps carry the work.
//!
//! 1. **Issue.** At a set block interval the engine derives a value from the
//!    block hash and the round number. The value selects one validator and one
//!    historical record. Every node derives the same value, so every node
//!    agrees on the challenge with no coordination.
//! 2. **Publish.** The challenge appears in the block header as a digest item.
//!    Every node sees the challenge at the same moment.
//! 3. **Answer.** The challenged validator reads the record from its local
//!    store, builds the inclusion proof and submits both. An inclusion proof is
//!    a short chain of hashes showing the record belongs to a committed block.
//! 4. **Verify.** The chain hashes the record, walks the proof upward and
//!    compares against the record root it already holds for that block.
//! 5. **Score.** A correct proof inside the deadline raises standing. A missed
//!    deadline lowers standing. An incorrect proof lowers standing sharply and
//!    records a fault.
//!
//! The deadline defaults to three blocks. Three blocks close faster than a
//! network fetch and a proof rebuild complete. Selection spans the full history
//! from genesis, so retaining a subset fails over time.

#![cfg_attr(not(feature = "std"), no_std)]

pub mod issue;
pub mod primitives;
pub mod publish;
pub mod respond;
pub mod score;
pub mod verify;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_reject;

pub use pallet::*;
pub use primitives::{
    custody_entry, Challenge, ChallengeAnswer, ChallengeState, CollectionId, CustodyRecord,
    CustodyRecordHook, CustodyStanding, CustodyValidators, ProofStep, RecordRef, CUSTODY_DIGEST_ID,
    DEFAULT_DEADLINE_BLOCKS,
};

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use crate::primitives::{
        Challenge, ChallengeAnswer, ChallengeState, CollectionId, CustodyRecord, CustodyStanding,
        CustodyValidators, RecordRef, MAX_KEY_BYTES,
    };
    use crate::verify::{leaf_hash, record_root, verify_against_root};
    use frame_support::pallet_prelude::*;
    use frame_support::traits::ConstU32;
    use frame_system::pallet_prelude::*;
    use sp_core::H256;
    use sp_runtime::traits::{SaturatedConversion, Saturating};
    use sp_std::vec::Vec;

    /// The challenge type this pallet holds.
    pub type ChallengeOf<T> = Challenge<<T as frame_system::Config>::AccountId, BlockNumberFor<T>>;

    /// The record reference type this pallet holds.
    pub type RecordRefOf<T> = RecordRef<BlockNumberFor<T>>;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// The overarching event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// The validator set the engine draws challenges against.
        type Validators: CustodyValidators<Self::AccountId>;

        /// The Standing Engine the custody share reaches.
        type Standing: CustodyStanding<Self::AccountId>;

        /// The blocks between one challenge and the next.
        ///
        /// A value of zero halts issue. The Amendment Engine governs the value.
        #[pallet::constant]
        type ChallengeInterval: Get<u32>;

        /// The blocks a challenged validator holds to answer.
        ///
        /// Three blocks close faster than a network fetch and a proof rebuild
        /// complete.
        #[pallet::constant]
        type DeadlineBlocks: Get<u32>;

        /// The greatest number of records one block commits.
        #[pallet::constant]
        type MaxRecordsPerBlock: Get<u32>;

        /// The greatest number of challenges the engine holds open at once.
        #[pallet::constant]
        type MaxOpenChallenges: Get<u32>;
    }

    /// The record root of each block that committed records.
    ///
    /// Verification compares every proof against the root held here.
    #[pallet::storage]
    #[pallet::getter(fn record_root_at)]
    pub type RecordRoots<T: Config> =
        StorageMap<_, Blake2_128Concat, BlockNumberFor<T>, H256, OptionQuery>;

    /// The number of records each block committed.
    #[pallet::storage]
    #[pallet::getter(fn record_count_at)]
    pub type RecordCounts<T: Config> =
        StorageMap<_, Blake2_128Concat, BlockNumberFor<T>, u32, ValueQuery>;

    /// The leaves of the block under construction.
    ///
    /// The block finalisation hook folds these into a record root and clears
    /// the list.
    #[pallet::storage]
    pub type PendingLeaves<T: Config> =
        StorageValue<_, BoundedVec<H256, T::MaxRecordsPerBlock>, ValueQuery>;

    /// The highest block that committed records.
    #[pallet::storage]
    #[pallet::getter(fn history_head)]
    pub type HistoryHead<T: Config> = StorageValue<_, BlockNumberFor<T>, ValueQuery>;

    /// The identifier the next challenge receives.
    #[pallet::storage]
    #[pallet::getter(fn next_challenge_id)]
    pub type NextChallengeId<T: Config> = StorageValue<_, u64, ValueQuery>;

    /// The challenges waiting for an answer.
    #[pallet::storage]
    #[pallet::getter(fn open_challenge)]
    pub type OpenChallenges<T: Config> =
        StorageMap<_, Blake2_128Concat, u64, ChallengeOf<T>, OptionQuery>;

    /// The identifiers of the challenges waiting for an answer.
    #[pallet::storage]
    pub type OpenChallengeIds<T: Config> =
        StorageValue<_, BoundedVec<u64, T::MaxOpenChallenges>, ValueQuery>;

    /// The state each settled challenge reached.
    #[pallet::storage]
    #[pallet::getter(fn challenge_state)]
    pub type ChallengeStates<T: Config> =
        StorageMap<_, Blake2_128Concat, u64, ChallengeState, OptionQuery>;

    /// The custody record of each validator.
    #[pallet::storage]
    #[pallet::getter(fn custody_record)]
    pub type CustodyRecords<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, CustodyRecord, ValueQuery>;

    /// The challenge the current block published, where one issued.
    #[pallet::storage]
    #[pallet::getter(fn published_challenge)]
    pub type PublishedChallenge<T: Config> = StorageValue<_, (u64, ChallengeOf<T>), OptionQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// The engine issued a challenge and published it in the header.
        ChallengeIssued {
            /// The identifier of the challenge.
            id: u64,
            /// The validator the challenge names.
            target: T::AccountId,
            /// The historical block the challenge reaches into.
            block: BlockNumberFor<T>,
            /// The last block at which an answer counts as timely.
            deadline: BlockNumberFor<T>,
        },
        /// A validator served the record and the proof verified.
        ChallengeAnswered {
            /// The identifier of the challenge.
            id: u64,
            /// The validator that answered.
            target: T::AccountId,
            /// The custody share after the movement, in parts per million.
            share: u32,
        },
        /// A deadline passed with no answer.
        ChallengeMissed {
            /// The identifier of the challenge.
            id: u64,
            /// The validator that let the challenge pass.
            target: T::AccountId,
            /// The custody share after the movement, in parts per million.
            share: u32,
        },
        /// A proof failed verification and the chain recorded a fault.
        ChallengeFaulted {
            /// The identifier of the challenge.
            id: u64,
            /// The validator whose proof failed.
            target: T::AccountId,
            /// The custody share after the movement, in parts per million.
            share: u32,
            /// The fault count after the movement.
            faults: u32,
        },
        /// A block committed records and the chain holds their root.
        RecordRootSealed {
            /// The block that committed the records.
            block: BlockNumberFor<T>,
            /// The number of records the block committed.
            count: u32,
            /// The root over those records.
            root: H256,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// No challenge carries that identifier.
        ChallengeNotFound,
        /// The challenge names another validator.
        WrongValidator,
        /// The deadline passed, so the answer arrives late.
        DeadlinePassed,
        /// The chain holds no record root for that block.
        RecordRootAbsent,
        /// The proof reached a different root.
        ProofRejected,
        /// The block holds more records than the bound allows.
        BlockRecordsFull,
        /// The engine holds as many open challenges as the bound allows.
        OpenChallengesFull,
        /// The answer runs longer than the custody answer path admits. Send
        /// the record and the shortest proof that reaches the root.
        AnswerAboveBound,
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_initialize(now: BlockNumberFor<T>) -> Weight {
            let mut weight = T::DbWeight::get().reads_writes(4, 2);

            PublishedChallenge::<T>::kill();

            // Close every challenge whose deadline passed with no answer.
            weight = weight.saturating_add(Self::close_lapsed_challenges(now));

            // Issue the challenge this block carries, where the interval calls
            // for one.
            if Self::issue_challenge(now).is_some() {
                weight = weight.saturating_add(T::DbWeight::get().reads_writes(4, 4));
            }
            weight
        }

        fn on_finalize(now: BlockNumberFor<T>) {
            Self::seal_record_root(now);
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Answer an open challenge with the record and its inclusion proof.
        ///
        /// The chain hashes the record, walks the proof upward and compares
        /// against the record root it holds for that block.
        #[pallet::call_index(0)]
        #[pallet::weight(Weight::from_parts(60_000, 0))]
        pub fn answer_challenge(
            origin: OriginFor<T>,
            id: u64,
            answer: ChallengeAnswer,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            // The custody answer path carries a hard size bound. The shared
            // check runs before any other work.
            cerulea_primitives::bounds::admit(
                cerulea_primitives::bounds::MessagePath::CustodyAnswer,
                codec::Encode::encoded_size(&answer),
            )
            .map_err(|_| Error::<T>::AnswerAboveBound)?;
            let challenge = OpenChallenges::<T>::get(id).ok_or(Error::<T>::ChallengeNotFound)?;
            ensure!(challenge.target == who, Error::<T>::WrongValidator);

            let now = frame_system::Pallet::<T>::block_number();
            ensure!(now <= challenge.deadline, Error::<T>::DeadlinePassed);

            let root = RecordRoots::<T>::get(challenge.record.block)
                .ok_or(Error::<T>::RecordRootAbsent)?;

            let verified =
                verify_against_root(root, challenge.record.index, &answer.value, &answer.proof)
                    .is_ok();

            let state = if verified {
                ChallengeState::AnsweredInTime
            } else {
                ChallengeState::Incorrect
            };
            // The verdict closes the challenge either way. An incorrect proof
            // lowers standing sharply and records a fault. The call succeeds
            // so the verdict stays on chain. A failing call would roll the
            // fault back and leave the challenge open for a second attempt.
            Self::settle_challenge(id, &challenge, state);
            Ok(())
        }

        /// Commit one record into the block under construction.
        ///
        /// The state module feeds every record it commits through the record
        /// hook. This call adds a record outside the state module, for a
        /// governance correction and for the engine tests. The call asks for
        /// the root origin, so no account fills the record tree of a block and
        /// crowds out the records the state module commits.
        #[pallet::call_index(1)]
        #[pallet::weight(Weight::from_parts(20_000, 0))]
        pub fn commit_record(
            origin: OriginFor<T>,
            collection: CollectionId,
            key: BoundedVec<u8, ConstU32<MAX_KEY_BYTES>>,
            value: BoundedVec<u8, ConstU32<{ crate::primitives::MAX_COMMITTED_VALUE_BYTES }>>,
        ) -> DispatchResult {
            ensure_root(origin)?;
            // The entry joins the collection, the key and the digest of the
            // full value, the same entry the state module commits.
            Self::note_record(&crate::primitives::custody_entry(
                &collection,
                &key,
                Some(&value),
            ))
        }
    }

    impl<T: Config> Pallet<T> {
        /// Add one record to the block under construction.
        ///
        /// The block finalisation hook folds the records into a root.
        pub fn note_record(value: &[u8]) -> DispatchResult {
            PendingLeaves::<T>::try_mutate(|leaves| {
                let index = leaves.len() as u32;
                leaves
                    .try_push(leaf_hash(index, value))
                    .map_err(|_| Error::<T>::BlockRecordsFull)?;
                Ok(())
            })
        }

        /// Fold the records of one block into a root and store it.
        pub fn seal_record_root(now: BlockNumberFor<T>) {
            let leaves = PendingLeaves::<T>::take();
            if leaves.is_empty() {
                return;
            }
            let count = leaves.len() as u32;
            let root = record_root(&leaves);
            RecordRoots::<T>::insert(now, root);
            RecordCounts::<T>::insert(now, count);
            HistoryHead::<T>::put(now);
            Self::deposit_event(Event::RecordRootSealed {
                block: now,
                count,
                root,
            });
        }

        /// Issue the challenge one block carries, where the interval calls for
        /// one.
        ///
        /// The draw reads the parent block hash and the round number, so every
        /// node reaches the same challenge.
        pub fn issue_challenge(now: BlockNumberFor<T>) -> Option<u64> {
            let block_number = now.saturated_into::<u64>();
            let interval = T::ChallengeInterval::get() as u64;
            if !crate::issue::issues_at(block_number, interval) {
                return None;
            }

            let candidates = T::Validators::custody_candidates();
            if candidates.is_empty() {
                return None;
            }

            let head = HistoryHead::<T>::get().saturated_into::<u64>();
            if head == 0 {
                return None;
            }

            let parent = frame_system::Pallet::<T>::parent_hash();
            let parent = H256::from_slice(parent.as_ref());
            let round = crate::issue::round_of(block_number, interval);
            let draw = crate::issue::derive_draw(parent, round);

            // The history runs from block one through the head.
            let (validator_index, offset) =
                crate::issue::select(draw, candidates.len() as u64, head)?;
            let mut target_block = offset.saturating_add(1);

            // Walk forward to the first block holding records. Every node walks
            // the same way from the same state.
            let mut steps = 0u64;
            let mut record_count = Self::record_count_of(target_block);
            while record_count == 0 && steps < head {
                target_block = if target_block >= head {
                    1
                } else {
                    target_block.saturating_add(1)
                };
                record_count = Self::record_count_of(target_block);
                steps = steps.saturating_add(1);
            }
            if record_count == 0 {
                return None;
            }

            let record_index =
                crate::issue::select_record(draw, target_block, record_count as u64)?;

            let target = candidates[validator_index as usize].clone();
            let deadline = now.saturating_add(T::DeadlineBlocks::get().into());
            let block: BlockNumberFor<T> = target_block.saturated_into();

            let challenge = Challenge {
                target: target.clone(),
                block,
                record: RecordRef {
                    block,
                    index: record_index as u32,
                    collection: [0u8; 16],
                    key: BoundedVec::default(),
                },
                issued: now,
                deadline,
            };

            let id = NextChallengeId::<T>::get();
            NextChallengeId::<T>::put(id.saturating_add(1));

            if OpenChallengeIds::<T>::mutate(|ids| ids.try_push(id)).is_err() {
                return None;
            }
            OpenChallenges::<T>::insert(id, challenge.clone());
            ChallengeStates::<T>::insert(id, ChallengeState::Open);
            PublishedChallenge::<T>::put((id, challenge.clone()));

            frame_system::Pallet::<T>::deposit_log(crate::publish::digest_item(&challenge));

            Self::deposit_event(Event::ChallengeIssued {
                id,
                target,
                block,
                deadline,
            });
            Some(id)
        }

        /// The number of records one historical block committed.
        pub fn record_count_of(block: u64) -> u32 {
            let block: BlockNumberFor<T> = block.saturated_into();
            RecordCounts::<T>::get(block)
        }

        /// Close every challenge whose deadline passed with no answer.
        pub fn close_lapsed_challenges(now: BlockNumberFor<T>) -> Weight {
            let ids = OpenChallengeIds::<T>::get();
            let mut weight = Weight::zero();
            let mut lapsed: Vec<u64> = Vec::new();
            for id in ids.iter() {
                if let Some(challenge) = OpenChallenges::<T>::get(id) {
                    if now > challenge.deadline {
                        lapsed.push(*id);
                    }
                }
            }
            for id in lapsed {
                if let Some(challenge) = OpenChallenges::<T>::get(id) {
                    Self::settle_challenge(id, &challenge, ChallengeState::Missed);
                    weight = weight.saturating_add(T::DbWeight::get().reads_writes(2, 3));
                }
            }
            weight
        }

        /// Settle one challenge, move the custody share and report the outcome.
        pub fn settle_challenge(id: u64, challenge: &ChallengeOf<T>, state: ChallengeState) {
            let mut record = CustodyRecords::<T>::get(&challenge.target);
            let movement = crate::score::settle(&mut record, state);
            CustodyRecords::<T>::insert(&challenge.target, record);

            OpenChallenges::<T>::remove(id);
            OpenChallengeIds::<T>::mutate(|ids| {
                ids.retain(|held| held != &id);
            });
            ChallengeStates::<T>::insert(id, state);

            T::Standing::report_custody_share(&challenge.target, movement.current_share);

            match state {
                ChallengeState::AnsweredInTime => {
                    Self::deposit_event(Event::ChallengeAnswered {
                        id,
                        target: challenge.target.clone(),
                        share: movement.current_share,
                    });
                }
                ChallengeState::Missed => {
                    Self::deposit_event(Event::ChallengeMissed {
                        id,
                        target: challenge.target.clone(),
                        share: movement.current_share,
                    });
                }
                ChallengeState::Incorrect => {
                    T::Standing::report_custody_fault(&challenge.target, movement.faults);
                    Self::deposit_event(Event::ChallengeFaulted {
                        id,
                        target: challenge.target.clone(),
                        share: movement.current_share,
                        faults: movement.faults,
                    });
                }
                ChallengeState::Open => {}
            }
        }

        /// The custody share of one validator, in parts per million.
        pub fn custody_share(validator: &T::AccountId) -> u32 {
            let record = CustodyRecords::<T>::get(validator);
            if record.settled() == 0 {
                crate::score::STARTING_SHARE
            } else {
                record.share
            }
        }

        /// The proven custody faults of one validator.
        pub fn custody_faults(validator: &T::AccountId) -> u32 {
            CustodyRecords::<T>::get(validator).faults
        }
    }
}

impl<T: Config> CustodyRecordHook for Pallet<T> {
    fn note_committed(
        collection: &[u8],
        key: &[u8],
        value: Option<&[u8]>,
    ) -> frame_support::dispatch::DispatchResult {
        Pallet::<T>::note_record(&custody_entry(collection, key, value))
    }
}
