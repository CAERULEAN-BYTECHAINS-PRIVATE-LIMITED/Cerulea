#![cfg_attr(not(feature = "std"), no_std)]

// The pallet-side Admission Engine of the Dynamic Consensus Framework
pub mod engines;

/// The validator key records the Authoring Engine reads.
pub mod keys;

pub use pallet::*;

/// The scale the epoch commit weight of one validator reads on.
///
/// The Commit Engine compares aggregated weight against a share of the total.
/// Every validator therefore carries a share of this fixed scale. A validator
/// holding half the Trust Score of an epoch carries half of this number. The
/// sum across an epoch reaches this number, less the remainder integer
/// division leaves behind.
pub const VOTE_WEIGHT_SCALE: u128 = 32_000;

/// The round timeout a deployment starts from, in blocks.
///
/// A round that commits no block inside this many blocks closes, and the
/// validators vote again in the next round. The figure spans two checkpoint
/// intervals in the starting profile, so a healthy round commits long before
/// the timeout runs out. The Amendment Engine governs a change through
/// [`Pallet::set_round_timeout`].
pub const DEFAULT_ROUND_TIMEOUT_BLOCKS: u32 = 20;

/// The penalty for two votes one validator signed in one round.
///
/// The runtime answers this through the Standing Engine, which returns the
/// standing of the validator to zero and removes the seat. The answer is the
/// number of served epochs the reset destroyed.
pub trait VoteEquivocationPenalty<AccountId> {
    /// Apply the penalty to one validator.
    fn penalise(validator: &AccountId) -> u32;
}

/// The highest Trust Score one validator reaches.
///
/// The Standing Engine composes the Trust Score as a share of one, counted in
/// parts per billion. A validator meeting every component reaches this number.
/// The epoch weight freeze reads that share. Genesis refuses a figure above
/// the ceiling.
pub const TRUST_SCORE_CEILING: u128 = 1_000_000_000;

use alloc::vec::Vec;
use frame_support::traits::Get;
use frame_support::{ensure, BoundedVec};
use frame_system::pallet_prelude::BlockNumberFor;
use pallet_cerulea_dcf::EpochStats;
use sp_runtime::SaturatedConversion;

extern crate alloc;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod coverage_tests;

#[frame_support::pallet]
pub mod pallet {
    use alloc::vec::Vec;
    use codec::MaxEncodedLen;
    use codec::{Decode, Encode};
    use frame_support::pallet_prelude::*;
    use frame_support::traits::{Currency, Get, ReservableCurrency};
    use frame_support::BoundedVec;
    use frame_system::pallet_prelude::*;
    use scale_info::TypeInfo;
    use sp_runtime::traits::{IdentifyAccount, SaturatedConversion, Saturating, Verify};
    use sp_runtime::transaction_validity::{
        InvalidTransaction, TransactionSource, TransactionValidity, ValidTransaction,
    };
    use sp_std::prelude::*;

    type BalanceOf<T> = <T as pallet_cerulea_pos::Config>::Balance;

    use pallet_cerulea_dcf::{EjectionReason, EpochStats, ValidatorStatus};

    /// Local Registry ValidatorState structure to keep registry decoupled from DCF Config
    #[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    #[scale_info(skip_type_params(T))]
    pub struct ValidatorState<T: Config> {
        pub last_active_epoch: u32,
        pub current: EpochStats,
        pub history: BoundedVec<EpochStats, T::MaxValidatorHistorySize>,
        pub uptime: u32,
        pub inference_success_count: u32,
        pub participation_rate: u32,
        pub inference_count: u64,
        pub last_active_block: u32,
        pub name: Option<BoundedVec<u8, T::MaxValidatorNameSize>>,
        pub trust_score: u64,
    }

    /// Information about block finality status
    #[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo)]
    pub struct FinalityInfo<BlockNumber, Hash> {
        /// Whether the block is finalized
        pub is_finalized: bool,
        /// The checkpoint block number that provides finality (if finalized)
        pub finalized_by_checkpoint: Option<BlockNumber>,
        /// The hash of the checkpoint that provides finality (if finalized)
        pub finalized_checkpoint_hash: Option<Hash>,
    }

    /// Registry Vote structure sent over the network and recorded in the runtime
    #[derive(
        Encode,
        Decode,
        Clone,
        PartialEq,
        Eq,
        RuntimeDebug,
        TypeInfo,
        MaxEncodedLen,
        frame_support::__private::codec::DecodeWithMemTracking,
    )]
    pub struct DvfVote<Hash, AccountId, Signature> {
        pub epoch_id: u32,
        pub validator_set_id: u32,
        pub round_id: u32,
        pub block_number: u32,
        pub block_hash: Hash,
        pub validator_account: AccountId,
        pub signature: Signature, // Signature of the above fields
    }

    /// The fault record of two votes one validator signed in one round.
    #[derive(
        Encode,
        Decode,
        Clone,
        PartialEq,
        Eq,
        RuntimeDebug,
        TypeInfo,
        MaxEncodedLen,
        frame_support::__private::codec::DecodeWithMemTracking,
    )]
    pub struct VoteEquivocationRecord<Hash> {
        /// The round both votes name.
        pub round: u32,
        /// The height and block of the first vote.
        pub first: (u32, Hash),
        /// The height and block of the second vote.
        pub second: (u32, Hash),
        /// The block at which the chain proved the fault.
        pub proven_at: u32,
        /// The served epochs the standing reset destroyed.
        pub epochs_destroyed: u32,
    }

    /// How one voting round closed.
    ///
    /// A round closes in one of two ways. A block reached the threshold and
    /// committed. Or the round timeout ran out while every candidate sat below
    /// the threshold. The chain keeps the record, so the rule that lets a
    /// validator change its vote in a later round reads recorded weights.
    #[derive(
        Encode,
        Decode,
        Clone,
        PartialEq,
        Eq,
        RuntimeDebug,
        TypeInfo,
        MaxEncodedLen,
        frame_support::__private::codec::DecodeWithMemTracking,
    )]
    pub enum RoundOutcome<Hash> {
        /// A block reached the threshold and committed.
        Committed {
            /// The block that committed.
            block_hash: Hash,
            /// The Trust Score weight behind the commitment.
            weight: u128,
        },
        /// The timeout ran out with every candidate below the threshold.
        TimedOut {
            /// The weight the leading candidate carried when the round closed.
            highest_weight: u128,
            /// The weight the threshold asked for.
            threshold: u128,
        },
    }

    /// Registry Justification structure containing threshold-reaching votes
    #[derive(
        Encode,
        Decode,
        Clone,
        PartialEq,
        Eq,
        RuntimeDebug,
        TypeInfo,
        frame_support::__private::codec::DecodeWithMemTracking,
    )]
    pub struct DvfJustification<Hash, AccountId, Signature> {
        /// Round number for this justification
        pub round_number: u32,
        /// Block hash being justified
        pub block_hash: Hash,
        /// Collection of votes that justify this block
        pub votes: Vec<DvfVote<Hash, AccountId, Signature>>,
    }

    sp_api::decl_runtime_apis! {
        /// Registry Runtime API for querying the Registry gadget network state.
        pub trait DvfApi<BlockNumber, AccountId, Hash>
        where
            BlockNumber: codec::Codec + Clone + Eq + sp_std::fmt::Debug,
            AccountId: codec::Codec + Clone + Eq + sp_std::fmt::Debug,
            Hash: codec::Codec + Clone + Eq + sp_std::fmt::Debug,
        {
            fn get_dvf_finalized_block() -> BlockNumber;
            fn get_current_epoch() -> u32;
            fn get_validator_set() -> sp_std::vec::Vec<AccountId>;
            /// The frozen commit weight of each validator in this epoch.
            ///
            /// Each figure is the share of the epoch Trust Score total the
            /// validator holds, scaled to [`crate::VOTE_WEIGHT_SCALE`].
            fn get_validator_weights() -> sp_std::vec::Vec<(AccountId, u128)>;
            fn get_finality_threshold_perbill() -> sp_runtime::Perbill;
            fn get_finality_info(block_number: BlockNumber) -> FinalityInfo<BlockNumber, Hash>;
            fn get_finality_checkpoint_interval() -> BlockNumber;
            fn get_validator_set_id() -> u32;
            fn get_current_round() -> u32;
            fn get_validator_set_id_changed_at() -> Option<BlockNumber>;
            fn get_vote_retention_rounds() -> u32;
            fn get_vote_tally(block_hash: Hash) -> u128;
            fn submit_dvf_justification(justification: DvfJustification<Hash, AccountId, sp_runtime::MultiSignature>) -> Result<(), sp_runtime::DispatchError>;
        }

        /// The Authoring Engine interface the node reads.
        ///
        /// The node fills its key registry and its slot committee from these calls.
        /// Import then reaches the three checks the Authoring Engine carries.
        /// Membership names the author inside the committee. Signature checks the
        /// seal against the author key. Timing checks the rank against the moment.
        pub trait CeruleaAuthoringApi<AccountId>
        where
            AccountId: codec::Codec + Clone + Eq + sp_std::fmt::Debug,
        {
            /// The draw key and the seal key of every active validator.
            fn active_validator_keys() -> sp_std::vec::Vec<(AccountId, crate::keys::ValidatorKeySet)>;

            /// The commit weight of every active validator.
            ///
            /// The commit weight is the Trust Score the Standing Engine
            /// composes, read in parts per million. The node records the
            /// figure beside the key pair, so the Commit Engine tally opens
            /// over the live set. Stake enters no part of this answer.
            fn validator_commit_weights() -> sp_std::vec::Vec<(AccountId, u32)>;

            /// The five drawn validators for one slot, in rank order.
            ///
            /// Each seat carries the epoch draw proof the validator published.
            fn slot_committee(slot: u64, parent_hash: [u8; 32]) -> sp_std::vec::Vec<crate::keys::SlotCommitteeSeat<AccountId>>;

            /// The epoch the chain runs now.
            fn authoring_epoch() -> u32;

            /// The draw one validator published for one epoch.
            fn published_epoch_draw(validator: AccountId, epoch: u32) -> Option<crate::keys::PublishedDraw>;
        }
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// Set the pallet by specifying the parameters and types on which it depends.
    #[pallet::config]
    pub trait Config: frame_system::Config + pallet_cerulea_pos::Config {
        /// Because this pallet emits events, it depends on the runtime's definition of an event.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// The signers of votes.
        type Signer: Parameter + Member + IdentifyAccount<AccountId = Self::AccountId>;
        /// The signature type.
        type Signature: Parameter + Member + Verify<Signer = Self::Signer> + MaxEncodedLen;

        /// Base weight given to validators per stake unit.
        #[pallet::constant]
        type StakeWeightFactor: Get<u128>;

        /// Base weight multiplier given to validator DCF scores.
        #[pallet::constant]
        type ScoreWeightFactor: Get<u128>;

        /// Maximum allowed score influence on the total voting weight (clamping bound).
        #[pallet::constant]
        type ScoreBoostCap: Get<u128>;

        /// Threshold ratio expressed in Perbill (e.g. 2/3 = 66.666...%)
        #[pallet::constant]
        type FinalityThreshold: Get<sp_runtime::Perbill>;

        /// Interval of blocks defining a finality checkpoint.
        #[pallet::constant]
        type FinalityCheckpointInterval: Get<BlockNumberFor<Self>>;

        /// Maximum rounds to keep past vote records and tallies before pruning.
        #[pallet::constant]
        type VoteRetentionRounds: Get<u32>;

        /// Maximum number of validators in the validator set.
        #[pallet::constant]
        type MaxValidators: Get<u32>;

        #[pallet::constant]
        type MaxInactiveEpochs: Get<u32>;

        #[pallet::constant]
        type UnderperformanceCheckInterval: Get<BlockNumberFor<Self>>;

        #[pallet::constant]
        type MaxValidatorHistorySize: Get<u32>;

        #[pallet::constant]
        type MaxValidatorNameSize: Get<u32>;

        /// The penalty for two votes one validator signed in one round.
        type VotePenalty: crate::VoteEquivocationPenalty<Self::AccountId>;
    }

    /// Frozen weights for active validators during the current epoch.
    #[pallet::storage]
    #[pallet::getter(fn epoch_voting_weight)]
    pub type EpochVotingWeight<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, u128, OptionQuery>;

    /// Total voting weight distributed across all validators in the current epoch.
    #[pallet::storage]
    #[pallet::getter(fn total_voting_weight)]
    pub type TotalVotingWeight<T: Config> = StorageValue<_, u128, ValueQuery>;

    /// Current Registry voting round inside an epoch. Incremented after each successfully finalized block.
    #[pallet::storage]
    #[pallet::getter(fn current_round)]
    pub type CurrentRound<T: Config> = StorageValue<_, u32, ValueQuery>;

    /// Number of the highest finalized block via Registry.
    #[pallet::storage]
    #[pallet::getter(fn finalized_block_number)]
    pub type FinalizedBlockNumber<T: Config> = StorageValue<_, BlockNumberFor<T>, ValueQuery>;

    /// Hash of the highest finalized block via Registry.
    #[pallet::storage]
    #[pallet::getter(fn finalized_block_hash)]
    pub type FinalizedBlockHash<T: Config> = StorageValue<_, T::Hash, OptionQuery>;

    /// Stores the vote cast by a specific validator per round. Map key: (Round, AccountId)
    #[pallet::storage]
    pub type VoteRecords<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        u32, // Round
        Blake2_128Concat,
        T::AccountId, // AccountId
        DvfVote<T::Hash, T::AccountId, T::Signature>,
        OptionQuery,
    >;

    /// The block at which the current round opened.
    ///
    /// The round timeout counts from this block. Genesis opens round zero at
    /// block zero.
    #[pallet::storage]
    #[pallet::getter(fn round_opened_at)]
    pub type RoundOpenedAt<T: Config> = StorageValue<_, BlockNumberFor<T>, ValueQuery>;

    /// The round timeout policy value, in blocks.
    ///
    /// A round that commits no block inside this many blocks closes and the
    /// next round opens.
    #[pallet::storage]
    #[pallet::getter(fn round_timeout)]
    pub type RoundTimeoutBlocks<T: Config> = StorageValue<_, u32, ValueQuery, RoundTimeoutDefault>;

    /// The starting value of the round timeout policy.
    pub struct RoundTimeoutDefault;
    impl Get<u32> for RoundTimeoutDefault {
        fn get() -> u32 {
            crate::DEFAULT_ROUND_TIMEOUT_BLOCKS
        }
    }

    /// The vote equivocation faults the chain proved, by validator.
    ///
    /// The record holds the latest proven fault of each validator.
    #[pallet::storage]
    #[pallet::getter(fn vote_equivocation_of)]
    pub type VoteEquivocations<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, VoteEquivocationRecord<T::Hash>, OptionQuery>;

    /// The rounds in which each validator was already penalised.
    ///
    /// A second report of the same fault leaves the chain unchanged.
    #[pallet::storage]
    pub type PenalisedVoteEquivocations<T: Config> =
        StorageDoubleMap<_, Blake2_128Concat, u32, Blake2_128Concat, T::AccountId, (), OptionQuery>;

    /// How each retained round closed.
    #[pallet::storage]
    #[pallet::getter(fn round_outcome)]
    pub type RoundOutcomes<T: Config> =
        StorageMap<_, Blake2_128Concat, u32, RoundOutcome<T::Hash>, OptionQuery>;

    /// Running sum of weights voting for a specific block hash.
    ///
    /// The tally covers the current round alone. The chain clears every tally
    /// when a round closes, so a vote counts in the round it was cast in.
    #[pallet::storage]
    #[pallet::getter(fn vote_tallies)]
    pub type VoteTallies<T: Config> = StorageMap<_, Blake2_128Concat, T::Hash, u128, ValueQuery>;

    /// The current Validator set ID to avoid out-of-date validators voting
    #[pallet::storage]
    #[pallet::getter(fn validator_set_id)]
    pub type ValidatorSetId<T: Config> = StorageValue<_, u32, ValueQuery>;

    /// The previous validator set for change detection
    #[pallet::storage]
    pub type PreviousValidatorSet<T: Config> =
        StorageValue<_, BoundedVec<T::AccountId, <T as Config>::MaxValidators>, ValueQuery>;

    /// The block number when the validator set ID last changed (for grace period)
    #[pallet::storage]
    pub type ValidatorSetIdChangedAt<T: Config> = StorageValue<_, BlockNumberFor<T>, OptionQuery>;

    // --- Validator Registry Storage Maps ---

    #[pallet::storage]
    #[pallet::getter(fn validator_set)]
    pub type ValidatorSet<T: Config> =
        StorageValue<_, BoundedVec<T::AccountId, <T as Config>::MaxValidators>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn active_validators)]
    pub type ActiveValidators<T: Config> =
        StorageValue<_, BoundedVec<T::AccountId, <T as Config>::MaxValidators>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn pending_validator_actions)]
    pub type PendingValidatorActions<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        pallet_cerulea_dcf::ValidatorAction,
        OptionQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn validator_join_time)]
    pub type ValidatorJoinTime<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, u32, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn validator_leave_requests)]
    pub type ValidatorLeaveRequests<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, u32, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn recently_removed_validators)]
    pub type RecentlyRemovedValidators<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, u32, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn validator_states)]
    pub type ValidatorStates<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, ValidatorState<T>, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn validator_names)]
    pub type ValidatorNames<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, BoundedVec<u8, ConstU32<32>>, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn validator_uptime)]
    pub type ValidatorUptime<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, u32, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn validator_metadata)]
    pub type ValidatorMetadata<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        pallet_cerulea_dcf::ValidatorMetadataInfo,
        OptionQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn validator_performance_history)]
    pub type ValidatorPerformanceHistory<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        BoundedVec<pallet_cerulea_dcf::PerformanceRecord, ConstU32<100>>,
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn validator_last_seen)]
    pub type ValidatorLastSeen<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, u32, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn validator_blocks_authored)]
    pub type ValidatorBlocksAuthored<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, u32, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn validator_blocks_missed)]
    pub type ValidatorBlocksMissed<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, u32, ValueQuery>;

    // --- Validator Key Records ---

    /// The key set each validator uses now.
    ///
    /// The Authoring Engine reads the draw key for the slot draw and the seal
    /// key for the block seal.
    #[pallet::storage]
    #[pallet::getter(fn validator_key_record)]
    pub type ValidatorKeyRecords<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, crate::keys::ValidatorKeySet, OptionQuery>;

    /// The key set each validator takes up at the next epoch boundary.
    #[pallet::storage]
    #[pallet::getter(fn pending_validator_key_record)]
    pub type PendingKeyRecords<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, crate::keys::ValidatorKeySet, OptionQuery>;

    /// The validator account that owns one draw key.
    ///
    /// The reverse record keeps every draw key with one owner. The committee
    /// call reads it to name the validator behind a drawn key.
    #[pallet::storage]
    #[pallet::getter(fn draw_key_owner)]
    pub type DrawKeyOwner<T: Config> =
        StorageMap<_, Blake2_128Concat, [u8; 32], T::AccountId, OptionQuery>;

    /// The draw each validator published for one epoch.
    #[pallet::storage]
    #[pallet::getter(fn epoch_draw)]
    pub type EpochDraws<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        u32,
        Blake2_128Concat,
        T::AccountId,
        crate::keys::PublishedDraw,
        OptionQuery,
    >;

    /// The epoch the active key records last moved forward.
    #[pallet::storage]
    #[pallet::getter(fn key_rotation_epoch)]
    pub type KeyRotationEpoch<T: Config> = StorageValue<_, u32, ValueQuery>;

    /// Genesis settings for the Registry pallet.
    /// The public key the control plane signs validator licences with.
    ///
    /// While the key is present, a validator joins the set only under a
    /// licence attestation that key signed and that still holds.
    #[pallet::storage]
    pub type ValidatorLicenceKey<T: Config> = StorageValue<_, [u8; 32], OptionQuery>;

    /// The lowest licence class a validator of this chain carries.
    #[pallet::storage]
    pub type RequiredLicenceClass<T: Config> =
        StorageValue<_, crate::engines::admission::licence::LicenceClass, ValueQuery>;

    /// The licence attestation each validator holds.
    #[pallet::storage]
    pub type ValidatorLicences<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        crate::engines::admission::licence::LicenceAttestation<T::AccountId, BlockNumberFor<T>>,
        OptionQuery,
    >;

    /// The newest attestation nonce each validator presented.
    ///
    /// A replayed attestation carries an older nonce, so a licence the
    /// control plane withdrew never returns.
    #[pallet::storage]
    pub type LicenceNonces<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, u64, ValueQuery>;

    #[pallet::genesis_config]
    #[derive(frame_support::DefaultNoBound)]
    pub struct GenesisConfig<T: Config> {
        /// The public key the control plane signs validator licences with.
        ///
        /// A chain specification that names a key makes every later
        /// validator join under a licence that key signed.
        pub validator_licence_key: Option<[u8; 32]>,
        /// The lowest licence class a validator of this chain carries, as a
        /// genesis index: 0 private, 1 public-access, 2 government.
        pub required_licence_class: u8,
        /// The validators present at genesis.
        ///
        /// Each entry names the validator, the stake it bonded and the Trust
        /// Score it starts under, as a share of one in parts per billion. The
        /// commit weight reads the Trust Score. Stake stays an admission input.
        pub initial_validator_weights: Vec<(T::AccountId, u128, u128)>,
        /// The draw key and the seal key of each validator present at genesis.
        pub initial_validator_keys: Vec<(T::AccountId, crate::keys::ValidatorKeySet)>,
        /// Whether genesis demands a validator set at block zero.
        ///
        /// A production chain spec sets this true, so a spec naming no
        /// validator stops the chain at genesis. A deployment that admits
        /// every validator through the Admission Engine after genesis leaves
        /// this false and starts from an empty weight table.
        pub strict_validation: bool,
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            if self.strict_validation {
                assert!(
                    !self.initial_validator_weights.is_empty(),
                    "Registry genesis requires at least one validator with non-zero weight. Name the genesis validators in the chain spec, or admit them through the Admission Engine after genesis and leave strict_validation false."
                );
            }

            for (validator, stake, trust) in &self.initial_validator_weights {
                assert!(
                    *stake > 0,
                    "Registry genesis validator {:?} has invalid stake (must be > 0)",
                    validator
                );
                assert!(
                    *trust <= crate::TRUST_SCORE_CEILING,
                    "Registry genesis validator {:?} has invalid score {} (must be 0 to {})",
                    validator,
                    trust,
                    crate::TRUST_SCORE_CEILING
                );
            }

            log::info!(
                target: "runtime::registry",
                "Registry genesis initializing with {} validators",
                self.initial_validator_weights.len()
            );

            ValidatorSetId::<T>::put(0);
            Pallet::<T>::freeze_epoch_weights(0, &self.initial_validator_weights);
            ValidatorSetId::<T>::put(0);

            for (validator, keys) in &self.initial_validator_keys {
                ValidatorKeyRecords::<T>::insert(validator, keys);
                DrawKeyOwner::<T>::insert(keys.draw, validator.clone());
            }

            if let Some(key) = self.validator_licence_key {
                ValidatorLicenceKey::<T>::put(key);
            }
            let class = crate::engines::admission::licence::LicenceClass::from_index(
                self.required_licence_class,
            )
            .expect(
                "Registry genesis names an unknown licence class. Name 0 for private, 1 for public-access or 2 for government.",
            );
            RequiredLicenceClass::<T>::put(class);

            log::info!(
                target: "runtime::registry",
                "Registry genesis initialization complete - EpochVotingWeight populated for epoch 0"
            );
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A round closed at its timeout with no block at the threshold.
        ///
        /// Every validator votes again in the next round.
        RoundTimedOut {
            /// The round that closed.
            round: u32,
            /// The round that opened.
            next_round: u32,
            /// The weight the leading candidate carried.
            highest_weight: u128,
            /// The weight the threshold asked for.
            threshold: u128,
        },
        /// The chain proved that one validator signed two votes in one round.
        ///
        /// The standing of the validator returned to zero and the seat went.
        VoteEquivocationProven {
            /// The validator that signed both votes.
            validator: T::AccountId,
            /// The round both votes name.
            round: u32,
            /// The served epochs the standing reset destroyed.
            epochs_destroyed: u32,
        },
        /// The round timeout policy value changed.
        RoundTimeoutSet {
            /// The new timeout, in blocks.
            blocks: u32,
        },
        /// A block was finalized by the Registry voting protocol.
        BlockFinalized {
            block_number: BlockNumberFor<T>,
            block_hash: T::Hash,
            round: u32,
            weight: u128,
        },
        /// Registry Voting Weights Frozen for a new epoch.
        WeightsFrozen {
            epoch: u32,
            total_weight: u128,
        },
        /// Validator set has changed and ValidatorSetId was incremented.
        ValidatorSetChanged {
            old_id: u32,
            new_id: u32,
        },
        /// Vote validation failed with a specific reason.
        VoteValidationFailed {
            validator: T::AccountId,
            block_number: BlockNumberFor<T>,
            reason: Vec<u8>,
        },

        // Moved registry events
        ValidatorJoined {
            validator: T::AccountId,
            stake_amount: BalanceOf<T>,
        },
        ValidatorLeft {
            validator: T::AccountId,
        },
        ValidatorLeaveRequested {
            validator: T::AccountId,
            cooldown_expires_at: u32,
        },
        ValidatorLeaveCancelled {
            validator: T::AccountId,
        },
        ValidatorAdded {
            validator: T::AccountId,
        },
        ValidatorRemoved {
            validator: T::AccountId,
        },
        ValidatorForcedToLeave {
            validator: T::AccountId,
        },
        ValidatorStatusChanged {
            validator: T::AccountId,
            old_status: pallet_cerulea_dcf::ValidatorStatus,
            new_status: pallet_cerulea_dcf::ValidatorStatus,
            block_number: u32,
        },
        ValidatorEjected {
            validator: T::AccountId,
            reason: pallet_cerulea_dcf::EjectionReason,
        },
        CooldownExpired {
            validator: T::AccountId,
        },
        /// A validator registered a draw key and a seal key. The pair takes
        /// effect at the next epoch boundary.
        ValidatorKeysRegistered {
            validator: T::AccountId,
            keys: crate::keys::ValidatorKeySet,
        },
        /// A pending key pair took effect at an epoch boundary.
        ValidatorKeysRotated {
            validator: T::AccountId,
            epoch: u32,
            keys: crate::keys::ValidatorKeySet,
        },
        /// A validator published its draw for one epoch.
        EpochDrawPublished {
            validator: T::AccountId,
            epoch: u32,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Cannot vote on a block below or equal to the finalized head.
        BlockAlreadyFinalized,
        /// Validator is not in the active set or epoch mismatch.
        InvalidValidator,
        /// Missing or Invalid Signature.
        InvalidSignature,
        /// Validator has already voted in this round.
        DoubleVote,
        /// The validator voted for another block at this height in an earlier
        /// round, and that round carries no record of closing below the
        /// threshold. Vote for the block of the earlier round, or wait for the
        /// round to time out.
        ConflictingVote,
        /// The two votes fail to prove an equivocation. Submit two votes one
        /// validator signed in one round for two different blocks.
        VoteEquivocationUnproven,
        /// The chain already penalised this validator for this round.
        VoteEquivocationAlreadyPenalised,
        /// A round timeout of zero blocks would close every round at once.
        /// Name a timeout of one block or more.
        RoundTimeoutZero,
        /// Vote is for a non-checkpoint block.
        NonCheckpointBlock,
        /// Justification has no votes.
        EmptyJustification,
        /// Justification contains duplicate validators.
        DuplicateValidator,
        /// Justification accumulated weight does not meet threshold.
        ThresholdNotReached,
        /// Votes in justification have mismatched round numbers.
        RoundMismatch,
        /// Votes in justification have mismatched block hashes.
        BlockHashMismatch,
        /// Votes in justification have mismatched validator set IDs.
        ValidatorSetIdMismatch,
        /// Action not allowed in governance mode.
        NotAllowedInGovernanceMode,
        /// There are not enough validators.
        NotEnoughValidators,
        /// The validator was not found.
        ValidatorNotFound,
        /// Account is not a validator.
        NotValidator,
        /// The draw key failed to prove possession. Sign the validator account
        /// with the draw key and submit that signature.
        DrawKeyPossessionRejected,
        /// The seal key failed to prove possession. Sign the validator account
        /// with the seal key and submit that signature.
        SealKeyPossessionRejected,
        /// Another validator already holds this draw key. Generate a fresh draw
        /// key and register that one.
        DrawKeyAlreadyHeld,
        /// The account holds no registered key pair. Register a draw key and a
        /// seal key before publishing a draw.
        ValidatorKeysAbsent,
        /// The published draw failed its proof. Publish a draw made with the
        /// registered draw key for the stated epoch.
        EpochDrawRejected,
        /// The stated epoch already closed. Publish a draw for the epoch that
        /// runs now or for one that follows.
        EpochDrawTooLate,
        /// The validator already published a draw for the stated epoch.
        EpochDrawAlreadyHeld,
        /// The chain holds no key for validator licences. Install the key of
        /// the control plane through the root origin first.
        LicenceKeyMissing,
        /// The validator holds no licence that still holds. Submit the newest
        /// licence attestation the control plane issued, then join.
        LicenceRequired,
        /// The attestation names another account. Submit the attestation
        /// issued for this validator.
        LicenceHolderMismatch,
        /// The licence signature failed against the control plane key.
        /// Request a fresh attestation.
        LicenceSignatureRejected,
        /// The licence lapsed. Renew the licence, then submit the fresh
        /// attestation.
        LicenceLapsed,
        /// The licence begins at a later block. Submit the attestation once
        /// that block passes.
        LicenceNotYetActive,
        /// The attestation repeats an earlier count. Submit the newest
        /// attestation the control plane issued.
        LicenceReplayed,
        /// The licence class sits below the class this chain asks for.
        /// Request a licence of the higher class.
        LicenceClassTooLow,
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_initialize(n: BlockNumberFor<T>) -> Weight {
            let block_number = n.saturated_into::<u32>();
            Self::cleanup_recently_removed_validators(block_number);
            Self::process_expired_leave_requests(block_number);

            if block_number % T::UnderperformanceCheckInterval::get().saturated_into::<u32>() == 0 {
                Self::check_and_handle_underperforming_validators();
            }
            Self::rotate_validator_keys_at_epoch_boundary();
            if Self::close_round_on_timeout(n) {
                return T::DbWeight::get().reads_writes(4, 4);
            }
            T::DbWeight::get().reads(3)
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Primary Extrinsic / Inherent entrypoint to submit a vote.
        #[pallet::call_index(0)]
        #[pallet::weight(Weight::from_parts(10_000, 0) + T::DbWeight::get().reads_writes(1,1))]
        pub fn submit_dvf_vote(
            origin: OriginFor<T>,
            vote: DvfVote<T::Hash, T::AccountId, T::Signature>,
        ) -> DispatchResult {
            ensure_signed(origin)?;

            // Validation Rules
            let mut encoded_payload = Vec::new();
            vote.epoch_id.encode_to(&mut encoded_payload);
            vote.validator_set_id.encode_to(&mut encoded_payload);
            vote.round_id.encode_to(&mut encoded_payload);
            vote.block_number.encode_to(&mut encoded_payload);
            vote.block_hash.encode_to(&mut encoded_payload);
            vote.validator_account.encode_to(&mut encoded_payload);

            ensure!(
                vote.signature
                    .verify(&encoded_payload[..], &vote.validator_account),
                Error::<T>::InvalidSignature
            );

            let voter_weight = EpochVotingWeight::<T>::get(&vote.validator_account)
                .ok_or(Error::<T>::InvalidValidator)?;

            ensure!(
                Self::is_checkpoint_block(vote.block_number.saturated_into()),
                Error::<T>::NonCheckpointBlock
            );

            let current_finalized_number = FinalizedBlockNumber::<T>::get();
            ensure!(
                vote.block_number.saturated_into::<u32>()
                    > current_finalized_number.saturated_into::<u32>(),
                Error::<T>::BlockAlreadyFinalized
            );

            let current_round = CurrentRound::<T>::get();
            ensure!(
                !VoteRecords::<T>::contains_key(current_round, &vote.validator_account),
                Error::<T>::DoubleVote
            );
            ensure!(
                Self::vote_respects_earlier_rounds(
                    current_round,
                    &vote.validator_account,
                    vote.block_number,
                    &vote.block_hash,
                ),
                Error::<T>::ConflictingVote
            );

            VoteRecords::<T>::insert(current_round, &vote.validator_account, vote.clone());

            let new_tally = VoteTallies::<T>::get(vote.block_hash).saturating_add(voter_weight);
            VoteTallies::<T>::insert(vote.block_hash, new_tally);

            let total_weight = TotalVotingWeight::<T>::get();
            let threshold_target = T::FinalityThreshold::get() * total_weight;

            if new_tally >= threshold_target {
                Self::trigger_finalization(
                    vote.block_hash,
                    vote.block_number.saturated_into(),
                    current_round,
                    new_tally,
                );
            }

            Ok(())
        }

        /// Install the key the control plane signs validator licences with.
        ///
        /// The root origin carries the call, which is the path the chain
        /// specification and the Amendment Engine take. The class names the
        /// lowest licence class a validator of this chain carries.
        #[pallet::call_index(11)]
        #[pallet::weight(Weight::from_parts(20_000, 0))]
        pub fn set_validator_licence_key(
            origin: OriginFor<T>,
            key: [u8; 32],
            class: crate::engines::admission::licence::LicenceClass,
        ) -> DispatchResult {
            ensure_root(origin)?;
            ValidatorLicenceKey::<T>::put(key);
            RequiredLicenceClass::<T>::put(class);
            Ok(())
        }

        /// Present one validator licence attestation.
        ///
        /// The Admission Engine checks the signature against the control
        /// plane key, then the window, the nonce and the class. A licence that
        /// passes stays on chain, and the validator joins under it.
        #[pallet::call_index(12)]
        #[pallet::weight(Weight::from_parts(60_000, 0))]
        pub fn submit_validator_licence(
            origin: OriginFor<T>,
            attestation: crate::engines::admission::licence::LicenceAttestation<
                T::AccountId,
                BlockNumberFor<T>,
            >,
            signature: [u8; 64],
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(attestation.holder == who, Error::<T>::LicenceHolderMismatch);
            let key = ValidatorLicenceKey::<T>::get().ok_or(Error::<T>::LicenceKeyMissing)?;
            let public = sp_core::ed25519::Public::from_raw(key);
            let held = sp_core::ed25519::Signature::from_raw(signature);
            let now = frame_system::Pallet::<T>::block_number();
            crate::engines::admission::licence::verify_attestation(
                &attestation,
                now,
                LicenceNonces::<T>::get(&who),
                RequiredLicenceClass::<T>::get(),
                |payload| sp_io::crypto::ed25519_verify(&held, payload, &public),
            )
            .map_err(Self::licence_error)?;
            LicenceNonces::<T>::insert(&who, attestation.nonce);
            ValidatorLicences::<T>::insert(&who, attestation);
            Ok(())
        }

        /// Prove that one validator signed two votes in one round.
        ///
        /// The evidence carries both votes with the signature the validator
        /// placed on each. The chain checks both signatures itself, so the
        /// report travels as an unsigned transaction from any node. A proof
        /// that passes returns the standing of the validator to zero through
        /// the Standing Engine, records the fault and removes the seat.
        #[pallet::call_index(10)]
        #[pallet::weight(Weight::from_parts(60_000, 0) + T::DbWeight::get().reads_writes(4, 4))]
        pub fn report_vote_equivocation(
            origin: OriginFor<T>,
            first: DvfVote<T::Hash, T::AccountId, T::Signature>,
            second: DvfVote<T::Hash, T::AccountId, T::Signature>,
        ) -> DispatchResult {
            ensure_none(origin)?;
            ensure!(
                Self::proves_vote_equivocation(&first, &second),
                Error::<T>::VoteEquivocationUnproven
            );
            let validator = first.validator_account.clone();
            let round = first.round_id;
            ensure!(
                !PenalisedVoteEquivocations::<T>::contains_key(round, &validator),
                Error::<T>::VoteEquivocationAlreadyPenalised
            );
            let epochs_destroyed = <T::VotePenalty as crate::VoteEquivocationPenalty<
                T::AccountId,
            >>::penalise(&validator);
            PenalisedVoteEquivocations::<T>::insert(round, &validator, ());
            VoteEquivocations::<T>::insert(
                &validator,
                VoteEquivocationRecord {
                    round,
                    first: (first.block_number, first.block_hash),
                    second: (second.block_number, second.block_hash),
                    proven_at: frame_system::Pallet::<T>::block_number().saturated_into::<u32>(),
                    epochs_destroyed,
                },
            );
            Self::deposit_event(Event::VoteEquivocationProven {
                validator,
                round,
                epochs_destroyed,
            });
            Ok(())
        }

        /// Set the round timeout policy value, in blocks.
        ///
        /// The Amendment Engine carries a change through root. A round that
        /// commits no block inside this many blocks closes and the next round
        /// opens.
        #[pallet::call_index(9)]
        #[pallet::weight(T::DbWeight::get().writes(1))]
        pub fn set_round_timeout(origin: OriginFor<T>, blocks: u32) -> DispatchResult {
            ensure_root(origin)?;
            ensure!(blocks > 0, Error::<T>::RoundTimeoutZero);
            RoundTimeoutBlocks::<T>::put(blocks);
            Self::deposit_event(Event::RoundTimeoutSet { blocks });
            Ok(())
        }

        /// Submit a Registry justification to finalize a block.
        #[pallet::call_index(1)]
        #[pallet::weight(Weight::from_parts(100_000, 0) + T::DbWeight::get().reads_writes(10, 10))]
        pub fn submit_justification(
            origin: OriginFor<T>,
            justification: DvfJustification<T::Hash, T::AccountId, T::Signature>,
        ) -> DispatchResult {
            ensure_none(origin)?;
            Self::verify_and_finalize_justification(justification)?;
            Ok(())
        }

        /// Request to join the validator set (opt-in, effective next epoch).
        #[pallet::call_index(2)]
        #[pallet::weight(Weight::from_parts(50_000, 0))]
        pub fn join_validator_set(origin: OriginFor<T>) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_licensed(&who)?;

            // Already pending join or already active
            ensure!(
                !Self::active_validators().contains(&who),
                Error::<T>::NotAllowedInGovernanceMode
            );
            ensure!(
                PendingValidatorActions::<T>::get(&who)
                    != Some(pallet_cerulea_dcf::ValidatorAction::Join),
                Error::<T>::NotAllowedInGovernanceMode
            );

            // Check minimum stake and score now, but actual addition is at epoch
            let stake = pallet_cerulea_pos::Pallet::<T>::stake(&who);
            ensure!(stake >= T::MinStake::get(), Error::<T>::NotEnoughValidators);
            let state = ValidatorStates::<T>::get(&who).ok_or(Error::<T>::ValidatorNotFound)?;
            ensure!(
                state.current.final_score >= T::MinValidatorScore::get() as u64,
                Error::<T>::NotValidator
            );

            PendingValidatorActions::<T>::insert(&who, pallet_cerulea_dcf::ValidatorAction::Join);
            Self::deposit_event(Event::ValidatorJoined {
                validator: who,
                stake_amount: T::MinStake::get(),
            });
            Ok(())
        }

        /// Request to leave the validator set (opt-out, effective next epoch).
        #[pallet::call_index(3)]
        #[pallet::weight(Weight::from_parts(50_000, 0))]
        pub fn leave_validator_set(origin: OriginFor<T>) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // Already pending leave or not active
            ensure!(
                Self::active_validators().contains(&who),
                Error::<T>::NotValidator
            );
            ensure!(
                PendingValidatorActions::<T>::get(&who)
                    != Some(pallet_cerulea_dcf::ValidatorAction::Leave),
                Error::<T>::NotAllowedInGovernanceMode
            );

            PendingValidatorActions::<T>::insert(&who, pallet_cerulea_dcf::ValidatorAction::Leave);
            Self::deposit_event(Event::ValidatorLeft { validator: who });
            Ok(())
        }

        #[pallet::call_index(4)]
        #[pallet::weight(Weight::from_parts(50_000, 0))]
        pub fn join_validators(
            origin: OriginFor<T>,
            _name: Option<BoundedVec<u8, ConstU32<32>>>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_licensed(&who)?;

            let min_stake = T::MinStake::get();
            let free_balance = <T::Currency as Currency<T::AccountId>>::free_balance(&who);
            ensure!(
                free_balance >= min_stake,
                pallet_cerulea_pos::Error::<T>::InsufficientStake
            );

            Self::validate_rejoin_eligibility(&who)?;

            let mut validator_set = ValidatorSet::<T>::get();
            ensure!(
                !validator_set.contains(&who),
                pallet_cerulea_pos::Error::<T>::ValidatorAlreadyExists
            );

            <T::Currency as ReservableCurrency<T::AccountId>>::reserve(&who, min_stake)
                .map_err(|_| pallet_cerulea_pos::Error::<T>::InsufficientStake)?;

            pallet_cerulea_pos::Stake::<T>::insert(&who, min_stake);
            ValidatorJoinTime::<T>::insert(
                &who,
                frame_system::Pallet::<T>::block_number().saturated_into::<u32>(),
            );

            ensure!(
                validator_set.len() < <T as Config>::MaxValidators::get() as usize,
                pallet_cerulea_pos::Error::<T>::TooManyValidators
            );

            validator_set
                .try_push(who.clone())
                .map_err(|_| pallet_cerulea_pos::Error::<T>::TooManyValidators)?;
            ValidatorSet::<T>::put(validator_set);

            pallet_cerulea_pos::Validators::<T>::insert(&who, true);

            // Initialize ValidatorStates
            if !ValidatorStates::<T>::contains_key(&who) {
                let current_epoch = pallet_cerulea_pos::Pallet::<T>::current_epoch();
                let initial_stats = EpochStats {
                    epoch: current_epoch,
                    stake_score: min_stake.saturated_into::<u64>(),
                    inference_score: 0,
                    final_score: 50,
                    authored_blocks: 0,
                    missed_blocks: 0,
                };
                let current_block =
                    frame_system::Pallet::<T>::block_number().saturated_into::<u32>();
                let state = ValidatorState {
                    last_active_epoch: current_epoch,
                    current: initial_stats,
                    history: BoundedVec::new(),
                    uptime: 0,
                    inference_success_count: 0,
                    participation_rate: 0,
                    inference_count: 0,
                    last_active_block: current_block,
                    name: None,
                    trust_score: 5000,
                };
                ValidatorStates::<T>::insert(&who, state);
            }

            Self::deposit_event(Event::ValidatorJoined {
                validator: who.clone(),
                stake_amount: min_stake,
            });

            Ok(())
        }

        #[pallet::call_index(5)]
        #[pallet::weight(Weight::from_parts(50_000, 0))]
        pub fn leave_validators(origin: OriginFor<T>) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let validator_set = ValidatorSet::<T>::get();
            ensure!(
                validator_set.contains(&who),
                pallet_cerulea_pos::Error::<T>::ValidatorNotInSet
            );

            Self::validate_leave_request_eligibility(&who)?;

            let current_block = frame_system::Pallet::<T>::block_number().saturated_into::<u32>();

            ValidatorLeaveRequests::<T>::insert(&who, current_block);

            let mut active_validators = ActiveValidators::<T>::get();
            if let Some(pos) = active_validators.iter().position(|v| v == &who) {
                active_validators.remove(pos);
                ActiveValidators::<T>::put(active_validators);
            }

            pallet_cerulea_pos::Validators::<T>::insert(&who, false);

            Self::deposit_event(Event::ValidatorLeaveRequested {
                validator: who.clone(),
                cooldown_expires_at: current_block + T::LeaveCooldown::get(),
            });

            Ok(())
        }

        #[pallet::call_index(6)]
        #[pallet::weight(Weight::from_parts(50_000, 0))]
        pub fn cancel_leave_request(origin: OriginFor<T>) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let leave_request_block = ValidatorLeaveRequests::<T>::get(&who)
                .ok_or(pallet_cerulea_pos::Error::<T>::ValidatorNotInSet)?;

            let current_block = frame_system::Pallet::<T>::block_number().saturated_into::<u32>();
            let cooldown_period = T::LeaveCooldown::get();
            let blocks_since_request = current_block.saturating_sub(leave_request_block);

            if blocks_since_request >= cooldown_period {
                return Err(pallet_cerulea_pos::Error::<T>::LeaveCooldownActive.into());
            }

            ValidatorLeaveRequests::<T>::remove(&who);

            let validator_set = ValidatorSet::<T>::get();
            if validator_set.contains(&who) {
                let mut active_validators = ActiveValidators::<T>::get();
                if !active_validators.contains(&who)
                    && active_validators.len() < <T as Config>::MaxValidators::get() as usize
                {
                    let _ = active_validators.try_push(who.clone());
                    ActiveValidators::<T>::put(active_validators);
                }
                pallet_cerulea_pos::Validators::<T>::insert(&who, true);
            }

            Self::deposit_event(Event::ValidatorLeaveCancelled {
                validator: who.clone(),
            });

            Ok(())
        }

        /// Register the draw key and the seal key of one validator.
        ///
        /// The Authoring Engine draws the slot committee with the draw key. The
        /// Authoring Engine seals the block with the seal key. The Commit
        /// Engine signs every vote with the same seal key.
        ///
        /// The extrinsic is signed by the validator account. Each new key signs
        /// that account in turn, so the chain learns that the account holds
        /// both private keys. The pair waits in a pending record and takes
        /// effect at the next epoch boundary.
        #[pallet::call_index(7)]
        #[pallet::weight(Weight::from_parts(80_000, 0) + T::DbWeight::get().reads_writes(3, 2))]
        pub fn register_validator_keys(
            origin: OriginFor<T>,
            draw_key: [u8; 32],
            seal_key: [u8; 32],
            draw_possession: [u8; 64],
            seal_possession: [u8; 64],
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(
                ValidatorSet::<T>::get().contains(&who),
                Error::<T>::NotValidator
            );

            let payload = crate::keys::possession_payload(&who.encode());
            let draw_public = sp_core::sr25519::Public::from_raw(draw_key);
            let draw_signature = sp_core::sr25519::Signature::from_raw(draw_possession);
            ensure!(
                sp_io::crypto::sr25519_verify(&draw_signature, &payload[..], &draw_public),
                Error::<T>::DrawKeyPossessionRejected
            );

            let seal_public = sp_core::ed25519::Public::from_raw(seal_key);
            let seal_signature = sp_core::ed25519::Signature::from_raw(seal_possession);
            ensure!(
                sp_io::crypto::ed25519_verify(&seal_signature, &payload[..], &seal_public),
                Error::<T>::SealKeyPossessionRejected
            );

            if let Some(owner) = DrawKeyOwner::<T>::get(draw_key) {
                ensure!(owner == who, Error::<T>::DrawKeyAlreadyHeld);
            }

            let set = crate::keys::ValidatorKeySet::new(draw_key, seal_key);
            PendingKeyRecords::<T>::insert(&who, set);
            Self::deposit_event(Event::ValidatorKeysRegistered {
                validator: who,
                keys: set,
            });
            Ok(())
        }

        /// Publish the draw of one validator for one epoch.
        ///
        /// The draw is one verifiable random function evaluation over the epoch
        /// number and the validator draw key. The chain checks the proof once
        /// here and stores the output. The committee of each slot in that epoch
        /// mixes the stored output with the slot seed.
        #[pallet::call_index(8)]
        #[pallet::weight(Weight::from_parts(200_000, 0) + T::DbWeight::get().reads_writes(3, 1))]
        pub fn publish_epoch_draw(
            origin: OriginFor<T>,
            epoch: u32,
            proof: crate::keys::DrawProofBytes,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let current_epoch = pallet_cerulea_pos::Pallet::<T>::current_epoch();
            ensure!(epoch >= current_epoch, Error::<T>::EpochDrawTooLate);
            ensure!(
                !EpochDraws::<T>::contains_key(epoch, &who),
                Error::<T>::EpochDrawAlreadyHeld
            );

            let set =
                Self::key_set_for_epoch(&who, epoch).ok_or(Error::<T>::ValidatorKeysAbsent)?;
            let signature = crate::keys::read_proof(&proof).ok_or(Error::<T>::EpochDrawRejected)?;
            let output =
                cerulea_primitives::verify_epoch_draw(&set.draw_public(), epoch, &signature)
                    .ok_or(Error::<T>::EpochDrawRejected)?;

            EpochDraws::<T>::insert(
                epoch,
                &who,
                crate::keys::PublishedDraw {
                    epoch,
                    output,
                    proof,
                },
            );
            Self::deposit_event(Event::EpochDrawPublished {
                validator: who,
                epoch,
            });
            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        // --- Validator Key Records ---

        /// The key set one validator uses in the stated epoch.
        ///
        /// A pending record serves every epoch beyond the one running now. The
        /// active record serves the epoch running now.
        pub fn key_set_for_epoch(
            validator: &T::AccountId,
            epoch: u32,
        ) -> Option<crate::keys::ValidatorKeySet> {
            let current_epoch = pallet_cerulea_pos::Pallet::<T>::current_epoch();
            if epoch > current_epoch {
                if let Some(pending) = PendingKeyRecords::<T>::get(validator) {
                    return Some(pending);
                }
            }
            ValidatorKeyRecords::<T>::get(validator)
                .or_else(|| PendingKeyRecords::<T>::get(validator))
        }

        /// Move every pending key record into service at an epoch boundary.
        ///
        /// The block initialisation hook calls this. A rotation therefore lands
        /// between two epochs and never splits one slot.
        pub fn rotate_validator_keys_at_epoch_boundary() -> u32 {
            let current_epoch = pallet_cerulea_pos::Pallet::<T>::current_epoch();
            if KeyRotationEpoch::<T>::get() == current_epoch
                && ValidatorKeyRecords::<T>::iter().next().is_some()
            {
                return 0;
            }
            KeyRotationEpoch::<T>::put(current_epoch);
            let pending: Vec<(T::AccountId, crate::keys::ValidatorKeySet)> =
                PendingKeyRecords::<T>::iter().collect();
            let mut moved = 0u32;
            for (validator, set) in pending {
                if let Some(previous) = ValidatorKeyRecords::<T>::get(&validator) {
                    if previous.draw != set.draw {
                        DrawKeyOwner::<T>::remove(previous.draw);
                    }
                }
                ValidatorKeyRecords::<T>::insert(&validator, set);
                DrawKeyOwner::<T>::insert(set.draw, validator.clone());
                PendingKeyRecords::<T>::remove(&validator);
                moved = moved.saturating_add(1);
                Self::deposit_event(Event::ValidatorKeysRotated {
                    validator,
                    epoch: current_epoch,
                    keys: set,
                });
            }
            moved
        }

        /// The key set of every active validator.
        ///
        /// The node fills its key registry from this list. Import then checks
        /// that the seal key named in a block belongs to the validator that
        /// drew the slot.
        pub fn active_validator_keys() -> Vec<(T::AccountId, crate::keys::ValidatorKeySet)> {
            let epoch = pallet_cerulea_pos::Pallet::<T>::current_epoch();
            ActiveValidators::<T>::get()
                .into_iter()
                .filter_map(|validator| {
                    Self::key_set_for_epoch(&validator, epoch).map(|set| (validator, set))
                })
                .collect()
        }

        /// The committee drawn for one slot, in rank order.
        ///
        /// Every candidate carries a published epoch draw. The seat value mixes
        /// that draw with the slot seed and then scales the result by the Trust
        /// Score the caller supplies. The five lowest seat values form the
        /// committee. Every node reaches the same order from the same chain
        /// state.
        pub fn slot_committee(
            slot: u64,
            parent_hash: [u8; 32],
            standing_of: impl Fn(&T::AccountId) -> u32,
        ) -> Vec<crate::keys::SlotCommitteeSeat<T::AccountId>> {
            let epoch = pallet_cerulea_pos::Pallet::<T>::current_epoch();
            let seed = cerulea_primitives::derive_slot_seed(&parent_hash, slot);
            let mut candidates: Vec<cerulea_primitives::SeatInput> = Vec::new();
            let mut records: Vec<crate::keys::SeatRecord<T::AccountId>> = Vec::new();

            for validator in ActiveValidators::<T>::get().into_iter() {
                let set = match Self::key_set_for_epoch(&validator, epoch) {
                    Some(set) => set,
                    None => continue,
                };
                let published = EpochDraws::<T>::get(epoch, &validator);
                let epoch_output = match &published {
                    Some(record) => record.output,
                    None => crate::keys::starting_epoch_output(&set.draw),
                };
                candidates.push(cerulea_primitives::SeatInput {
                    draw_key: set.draw,
                    epoch_output,
                    standing: standing_of(&validator),
                });
                records.push((set.draw, validator, set, published));
            }

            cerulea_primitives::rank_seats(&candidates, &seed)
                .into_iter()
                .filter_map(|seat| {
                    records
                        .iter()
                        .find(|(draw_key, _, _, _)| *draw_key == seat.draw_key)
                        .map(
                            |(_, validator, set, published)| crate::keys::SlotCommitteeSeat {
                                validator: validator.clone(),
                                keys: *set,
                                rank: seat.rank,
                                seat_value: seat.seat_value,
                                draw: *published,
                            },
                        )
                })
                .collect()
        }

        /// Helper function to check if a block number is a checkpoint block
        pub fn is_checkpoint_block(block_number: BlockNumberFor<T>) -> bool {
            let checkpoint_interval = T::FinalityCheckpointInterval::get();
            if checkpoint_interval == BlockNumberFor::<T>::from(0u32) {
                return false;
            }
            block_number % checkpoint_interval == BlockNumberFor::<T>::from(0u32)
        }

        /// Check if a block is finalized by comparing against the last finalized checkpoint
        pub fn is_block_finalized(block_number: BlockNumberFor<T>) -> bool {
            let finalized_checkpoint = FinalizedBlockNumber::<T>::get();
            block_number <= finalized_checkpoint
        }

        /// Get detailed finality information for a block number
        pub fn get_finality_info(
            block_number: BlockNumberFor<T>,
        ) -> FinalityInfo<BlockNumberFor<T>, T::Hash> {
            let finalized_checkpoint = FinalizedBlockNumber::<T>::get();

            if block_number <= finalized_checkpoint {
                let checkpoint_interval = T::FinalityCheckpointInterval::get();
                let providing_checkpoint = if checkpoint_interval > BlockNumberFor::<T>::from(0u32)
                {
                    (block_number / checkpoint_interval) * checkpoint_interval
                } else {
                    finalized_checkpoint
                };

                FinalityInfo {
                    is_finalized: true,
                    finalized_by_checkpoint: Some(providing_checkpoint),
                    finalized_checkpoint_hash: FinalizedBlockHash::<T>::get(),
                }
            } else {
                FinalityInfo {
                    is_finalized: false,
                    finalized_by_checkpoint: None,
                    finalized_checkpoint_hash: None,
                }
            }
        }

        /// Freeze the commit weight of every validator for one epoch.
        ///
        /// Each entry names a validator, the stake it bonded and the Trust
        /// Score the Standing Engine composed for it. The commit weight is the
        /// Trust Score. A validator carries the share of the epoch Trust Score
        /// total it holds, scaled to [`crate::VOTE_WEIGHT_SCALE`]. Stake stays
        /// an admission input, where the design names resource cost, and enters
        /// no weight here.
        ///
        /// A set whose Trust Score total reads zero carries an even split, so a
        /// chain that has yet to record any standing still reaches agreement.
        pub fn freeze_epoch_weights(epoch: u32, validators: &[(T::AccountId, u128, u128)]) {
            let new_validator_accounts: Vec<T::AccountId> =
                validators.iter().map(|(v, _, _)| v.clone()).collect();
            let previous_validator_set = PreviousValidatorSet::<T>::get();

            let validator_set_changed =
                if previous_validator_set.len() != new_validator_accounts.len() {
                    true
                } else {
                    let mut prev_sorted: Vec<T::AccountId> = previous_validator_set.to_vec();
                    prev_sorted.sort();
                    let mut new_sorted = new_validator_accounts.clone();
                    new_sorted.sort();
                    prev_sorted != new_sorted
                };

            if validator_set_changed {
                let old_id = ValidatorSetId::<T>::get();
                let new_id = old_id.saturating_add(1);
                ValidatorSetId::<T>::put(new_id);

                let current_block = frame_system::Pallet::<T>::block_number();
                ValidatorSetIdChangedAt::<T>::put(current_block);

                Self::deposit_event(Event::ValidatorSetChanged { old_id, new_id });
            }

            let new_validator_set: BoundedVec<T::AccountId, <T as Config>::MaxValidators> =
                BoundedVec::try_from(new_validator_accounts.clone()).unwrap_or_else(|_| {
                    let truncated: Vec<T::AccountId> = new_validator_accounts
                        .into_iter()
                        .take(<T as Config>::MaxValidators::get() as usize)
                        .collect();
                    BoundedVec::truncate_from(truncated)
                });
            PreviousValidatorSet::<T>::put(new_validator_set);

            let _ = EpochVotingWeight::<T>::clear(u32::MAX, None);

            let validator_count = validators.len() as u128;
            let total_trust: u128 = validators
                .iter()
                .map(|(_, _, trust)| (*trust).min(crate::TRUST_SCORE_CEILING))
                .sum();
            let mut total_epoch_weight = 0u128;

            for (validator, _stake, trust) in validators {
                let held = (*trust).min(crate::TRUST_SCORE_CEILING);
                let normalized_weight = if total_trust > 0 {
                    held.saturating_mul(crate::VOTE_WEIGHT_SCALE) / total_trust
                } else if validator_count > 0 {
                    crate::VOTE_WEIGHT_SCALE / validator_count
                } else {
                    0
                };

                EpochVotingWeight::<T>::insert(validator, normalized_weight);
                total_epoch_weight = total_epoch_weight.saturating_add(normalized_weight);
            }

            TotalVotingWeight::<T>::put(total_epoch_weight);
            Self::deposit_event(Event::WeightsFrozen {
                epoch,
                total_weight: total_epoch_weight,
            });
        }

        /// Verify and finalize a justification
        pub fn verify_and_finalize_justification(
            justification: DvfJustification<T::Hash, T::AccountId, T::Signature>,
        ) -> DispatchResult {
            log::info!(
                "Registry Pallet: verify_and_finalize_justification called for block {:?}, round {}",
                justification.block_hash,
                justification.round_number
            );

            ensure!(
                !justification.votes.is_empty(),
                Error::<T>::EmptyJustification
            );

            // A justification names the round its votes were cast in. That
            // round is the current round or one the chain already closed. A
            // round the chain has yet to open carries no vote.
            ensure!(
                justification.round_number <= CurrentRound::<T>::get(),
                Error::<T>::RoundMismatch
            );

            let block_number: BlockNumberFor<T> =
                justification.votes[0].block_number.saturated_into();

            ensure!(
                Self::is_checkpoint_block(block_number),
                Error::<T>::NonCheckpointBlock
            );

            let current_finalized_number = FinalizedBlockNumber::<T>::get();
            ensure!(
                block_number > current_finalized_number,
                Error::<T>::BlockAlreadyFinalized
            );

            let mut seen_validators = sp_std::collections::btree_set::BTreeSet::new();
            let mut accumulated_weight = 0u128;
            let mut expected_validator_set_id: Option<u32> = None;

            for vote in &justification.votes {
                ensure!(
                    seen_validators.insert(vote.validator_account.clone()),
                    Error::<T>::DuplicateValidator
                );

                let mut encoded_payload = Vec::new();
                vote.epoch_id.encode_to(&mut encoded_payload);
                vote.validator_set_id.encode_to(&mut encoded_payload);
                vote.round_id.encode_to(&mut encoded_payload);
                vote.block_number.encode_to(&mut encoded_payload);
                vote.block_hash.encode_to(&mut encoded_payload);
                vote.validator_account.encode_to(&mut encoded_payload);

                ensure!(
                    vote.signature
                        .verify(&encoded_payload[..], &vote.validator_account),
                    Error::<T>::InvalidSignature
                );

                if let Some(expected_id) = expected_validator_set_id {
                    ensure!(
                        vote.validator_set_id == expected_id,
                        Error::<T>::ValidatorSetIdMismatch
                    );
                } else {
                    expected_validator_set_id = Some(vote.validator_set_id);
                }

                ensure!(
                    vote.round_id == justification.round_number,
                    Error::<T>::RoundMismatch
                );

                ensure!(
                    vote.block_hash == justification.block_hash,
                    Error::<T>::BlockHashMismatch
                );

                let validator_weight = EpochVotingWeight::<T>::get(&vote.validator_account)
                    .ok_or(Error::<T>::InvalidValidator)?;

                // One vote per validator per round. A commitment record built
                // from a vote that conflicts with a vote the chain already holds
                // for that validator in that round is refused.
                if let Some(held) =
                    VoteRecords::<T>::get(justification.round_number, &vote.validator_account)
                {
                    ensure!(
                        held.block_number == vote.block_number
                            && held.block_hash == vote.block_hash,
                        Error::<T>::ConflictingVote
                    );
                }
                ensure!(
                    Self::vote_respects_earlier_rounds(
                        justification.round_number,
                        &vote.validator_account,
                        vote.block_number,
                        &vote.block_hash,
                    ),
                    Error::<T>::ConflictingVote
                );

                accumulated_weight = accumulated_weight.saturating_add(validator_weight);
            }

            let total_weight = TotalVotingWeight::<T>::get();
            let threshold = T::FinalityThreshold::get() * total_weight;

            ensure!(
                accumulated_weight >= threshold,
                Error::<T>::ThresholdNotReached
            );

            // The chain keeps every vote of the commitment record, so a later
            // record in the same round that names another block for one of
            // these validators is refused.
            for vote in &justification.votes {
                if !VoteRecords::<T>::contains_key(
                    justification.round_number,
                    &vote.validator_account,
                ) {
                    VoteRecords::<T>::insert(
                        justification.round_number,
                        &vote.validator_account,
                        vote.clone(),
                    );
                }
            }

            Self::finalize_block(
                justification.block_hash,
                block_number,
                justification.round_number,
                accumulated_weight,
            );

            log::info!(
                "Registry Pallet: Successfully verified and finalized block #{:?} via justification in round {}",
                block_number,
                justification.round_number
            );

            Ok(())
        }

        fn finalize_block(
            block_hash: T::Hash,
            block_number: BlockNumberFor<T>,
            round: u32,
            accumulated_weight: u128,
        ) {
            FinalizedBlockNumber::<T>::put(block_number);
            FinalizedBlockHash::<T>::put(block_hash);

            // A justification may name a round the chain already closed at its
            // timeout. The round counter never moves backwards, so a validator
            // never meets its own earlier vote in a round it reopens.
            let closed_round = round.max(CurrentRound::<T>::get());
            RoundOutcomes::<T>::insert(
                round,
                RoundOutcome::Committed {
                    block_hash,
                    weight: accumulated_weight,
                },
            );
            Self::open_round(closed_round.saturating_add(1));

            Self::deposit_event(Event::BlockFinalized {
                block_number,
                block_hash,
                round,
                weight: accumulated_weight,
            });
        }

        /// Open one round and clear the tallies of the round before it.
        ///
        /// Vote records stay for the retention window, so the lock rule reads
        /// the vote each validator cast in an earlier round.
        fn open_round(next_round: u32) {
            CurrentRound::<T>::put(next_round);
            RoundOpenedAt::<T>::put(frame_system::Pallet::<T>::block_number());
            let _ = VoteTallies::<T>::clear(u32::MAX, None);
            Self::prune_vote_records(next_round);
        }

        /// The weight one block must reach to commit.
        pub fn threshold_weight() -> u128 {
            T::FinalityThreshold::get() * TotalVotingWeight::<T>::get()
        }

        /// Close the current round when its timeout runs out.
        ///
        /// The round closes when two conditions hold. The timeout counted from
        /// the block the round opened has run out. A checkpoint past the
        /// finalized head has waited at least the timeout for its commitment.
        /// A healthy round therefore never closes this way. A split round
        /// closes, and every validator votes again in the next round on the
        /// best candidate.
        ///
        /// A candidate at the threshold commits in the vote that carried it
        /// there, so a closing round holds every candidate below the threshold.
        /// The call checks that from the recorded tallies before it closes the
        /// round, and commits the leading candidate instead where it holds the
        /// threshold.
        pub fn close_round_on_timeout(now: BlockNumberFor<T>) -> bool {
            let timeout: BlockNumberFor<T> = RoundTimeoutBlocks::<T>::get().max(1).into();
            let opened = RoundOpenedAt::<T>::get();
            if now < opened.saturating_add(timeout) {
                return false;
            }
            let interval = T::FinalityCheckpointInterval::get();
            if interval == BlockNumberFor::<T>::from(0u32) {
                return false;
            }
            let pending_checkpoint = FinalizedBlockNumber::<T>::get().saturating_add(interval);
            if now < pending_checkpoint.saturating_add(timeout) {
                return false;
            }

            let round = CurrentRound::<T>::get();
            let threshold = Self::threshold_weight();
            let highest_weight = VoteTallies::<T>::iter_values().max().unwrap_or(0);
            if threshold > 0 && highest_weight >= threshold {
                // The recorded weights show a committed candidate. Commit it
                // rather than reopen the height.
                if let Some((block_hash, weight)) =
                    VoteTallies::<T>::iter().find(|(_, weight)| *weight >= threshold)
                {
                    let height = VoteRecords::<T>::iter_prefix_values(round)
                        .find(|vote| vote.block_hash == block_hash)
                        .map(|vote| vote.block_number.saturated_into::<BlockNumberFor<T>>());
                    if let Some(height) = height {
                        if height > FinalizedBlockNumber::<T>::get() {
                            Self::finalize_block(block_hash, height, round, weight);
                            return true;
                        }
                    }
                }
            }

            RoundOutcomes::<T>::insert(
                round,
                RoundOutcome::TimedOut {
                    highest_weight,
                    threshold,
                },
            );
            let next_round = round.saturating_add(1);
            Self::open_round(next_round);
            Self::deposit_event(Event::RoundTimedOut {
                round,
                next_round,
                highest_weight,
                threshold,
            });
            true
        }

        /// Report whether two votes prove an equivocation.
        ///
        /// Both votes name one validator and one round and two different
        /// blocks, and both signatures verify against that validator.
        pub fn proves_vote_equivocation(
            first: &DvfVote<T::Hash, T::AccountId, T::Signature>,
            second: &DvfVote<T::Hash, T::AccountId, T::Signature>,
        ) -> bool {
            if first.validator_account != second.validator_account
                || first.round_id != second.round_id
                || (first.block_number == second.block_number
                    && first.block_hash == second.block_hash)
            {
                return false;
            }
            [first, second].iter().all(|vote| {
                let mut payload = Vec::new();
                vote.epoch_id.encode_to(&mut payload);
                vote.validator_set_id.encode_to(&mut payload);
                vote.round_id.encode_to(&mut payload);
                vote.block_number.encode_to(&mut payload);
                vote.block_hash.encode_to(&mut payload);
                vote.validator_account.encode_to(&mut payload);
                vote.signature.verify(&payload[..], &vote.validator_account)
            })
        }

        /// Report whether a vote keeps faith with the earlier rounds.
        ///
        /// A validator votes once per round. A vote in a later round for a
        /// different block at the same height passes only where every earlier
        /// round holding the first vote closed below the threshold. The chain
        /// reads that from the recorded round outcomes. A round with no
        /// recorded outcome refuses the change.
        pub fn vote_respects_earlier_rounds(
            current_round: u32,
            validator: &T::AccountId,
            block_number: u32,
            block_hash: &T::Hash,
        ) -> bool {
            let retention = T::VoteRetentionRounds::get();
            let earliest = current_round.saturating_sub(retention);
            for earlier in earliest..current_round {
                let Some(held) = VoteRecords::<T>::get(earlier, validator) else {
                    continue;
                };
                if held.block_number != block_number || held.block_hash == *block_hash {
                    continue;
                }
                match RoundOutcomes::<T>::get(earlier) {
                    Some(RoundOutcome::TimedOut {
                        highest_weight,
                        threshold,
                    }) if highest_weight < threshold => {}
                    _ => return false,
                }
            }
            true
        }

        fn prune_vote_records(current_round: u32) {
            let retention_rounds = T::VoteRetentionRounds::get();

            if current_round > retention_rounds {
                // Rounds below the cutoff already left the window one by one,
                // so the round that just crossed it is the only one to clear.
                let cutoff_round = current_round.saturating_sub(retention_rounds);
                let leaving = cutoff_round.saturating_sub(1);
                let _ = VoteRecords::<T>::clear_prefix(leaving, u32::MAX, None);
                RoundOutcomes::<T>::remove(leaving);
            }
        }

        fn trigger_finalization(
            block_hash: T::Hash,
            block_number: BlockNumberFor<T>,
            round: u32,
            tally_weight: u128,
        ) {
            Self::finalize_block(block_hash, block_number, round, tally_weight);
        }

        // --- Validator Registry Helpers ---

        fn validate_rejoin_eligibility(who: &T::AccountId) -> DispatchResult {
            if let Some(left_at_block) = RecentlyRemovedValidators::<T>::get(who) {
                let current_block =
                    frame_system::Pallet::<T>::block_number().saturated_into::<u32>();
                let cooldown_period = T::LeaveCooldown::get();
                let blocks_since_left = current_block.saturating_sub(left_at_block);

                if blocks_since_left < cooldown_period {
                    return Err(pallet_cerulea_pos::Error::<T>::CooldownActive.into());
                }

                RecentlyRemovedValidators::<T>::remove(who);
            }

            if ValidatorLeaveRequests::<T>::contains_key(who) {
                return Err(pallet_cerulea_pos::Error::<T>::LeaveCooldownActive.into());
            }

            Ok(())
        }

        fn validate_leave_request_eligibility(who: &T::AccountId) -> DispatchResult {
            ensure!(
                !ValidatorLeaveRequests::<T>::contains_key(who),
                pallet_cerulea_pos::Error::<T>::LeaveCooldownActive
            );
            Ok(())
        }

        fn cleanup_recently_removed_validators(current_block: u32) {
            let cooldown_period = T::LeaveCooldown::get();
            let mut expired_entries = Vec::new();

            for (validator, removed_at_block) in RecentlyRemovedValidators::<T>::iter() {
                let blocks_passed = current_block.saturating_sub(removed_at_block);
                if blocks_passed >= cooldown_period {
                    expired_entries.push(validator);
                }
            }

            for validator in expired_entries {
                RecentlyRemovedValidators::<T>::remove(&validator);
                Self::deposit_event(Event::CooldownExpired { validator });
            }
        }

        fn process_expired_leave_requests(current_block: u32) {
            let cooldown_period = T::LeaveCooldown::get();
            let mut expired_requests = Vec::new();

            for (validator, request_block) in ValidatorLeaveRequests::<T>::iter() {
                let blocks_passed = current_block.saturating_sub(request_block);
                if blocks_passed >= cooldown_period {
                    expired_requests.push(validator);
                }
            }

            for validator in expired_requests {
                let stake_amount = pallet_cerulea_pos::Stake::<T>::get(&validator);
                if stake_amount > BalanceOf::<T>::default() {
                    let _ = <T::Currency as ReservableCurrency<T::AccountId>>::unreserve(
                        &validator,
                        stake_amount,
                    );
                }

                let mut validator_set = ValidatorSet::<T>::get();
                if let Some(pos) = validator_set.iter().position(|v| v == &validator) {
                    validator_set.remove(pos);
                    ValidatorSet::<T>::put(validator_set);
                }

                let mut active_validators = ActiveValidators::<T>::get();
                if let Some(pos) = active_validators.iter().position(|v| v == &validator) {
                    active_validators.remove(pos);
                    ActiveValidators::<T>::put(active_validators);
                }

                pallet_cerulea_pos::Validators::<T>::remove(&validator);
                ValidatorJoinTime::<T>::remove(&validator);
                pallet_cerulea_pos::Stake::<T>::remove(&validator);
                ValidatorLeaveRequests::<T>::remove(&validator);

                RecentlyRemovedValidators::<T>::insert(&validator, current_block);

                Self::deposit_event(Event::ValidatorLeft { validator });
            }
        }

        fn check_and_handle_underperforming_validators() {
            let min_score = T::MinValidatorScore::get() as u64;
            let current_epoch = pallet_cerulea_pos::Pallet::<T>::current_epoch();

            let validators_to_check: Vec<T::AccountId> = ActiveValidators::<T>::get().into_inner();

            for validator in validators_to_check {
                if let Some(state) = ValidatorStates::<T>::get(&validator) {
                    if state.current.final_score < min_score {
                        log::warn!(
                            "Registry: Validator {:?} has low score: {}, ejecting",
                            validator,
                            state.current.final_score
                        );
                        let _ =
                            Self::eject_validator(&validator, EjectionReason::ScoreBelowThreshold);
                    }

                    let inactive_epochs = current_epoch.saturating_sub(state.last_active_epoch);
                    if inactive_epochs > T::MaxInactiveEpochs::get() {
                        log::warn!(
                            "Registry: Validator {:?} inactive for {} epochs, ejecting",
                            validator,
                            inactive_epochs
                        );
                        let _ =
                            Self::eject_validator(&validator, EjectionReason::ScoreBelowThreshold);
                    }
                }
            }
        }

        pub fn eject_validator(validator: &T::AccountId, reason: EjectionReason) -> DispatchResult {
            let mut active_validators = ActiveValidators::<T>::get();
            let pos_found = if let Some(pos) = active_validators.iter().position(|v| v == validator)
            {
                active_validators.remove(pos);
                ActiveValidators::<T>::put(active_validators);
                true
            } else {
                false
            };

            let current_block = frame_system::Pallet::<T>::block_number().saturated_into::<u32>();
            let old_status = if pos_found {
                ValidatorStatus::Active
            } else {
                ValidatorStatus::Inactive
            };
            Self::deposit_event(Event::ValidatorStatusChanged {
                validator: validator.clone(),
                old_status,
                new_status: ValidatorStatus::Ejected,
                block_number: current_block,
            });

            Self::deposit_event(Event::ValidatorEjected {
                validator: validator.clone(),
                reason,
            });

            Ok(())
        }
    }

    #[pallet::validate_unsigned]
    impl<T: Config> ValidateUnsigned for Pallet<T> {
        type Call = Call<T>;

        fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
            if let Call::submit_justification { justification } = call {
                if justification.votes.is_empty() {
                    return InvalidTransaction::Custom(1).into();
                }

                let block_number: BlockNumberFor<T> =
                    justification.votes[0].block_number.saturated_into();
                let current_finalized_number = FinalizedBlockNumber::<T>::get();

                if block_number <= current_finalized_number {
                    return InvalidTransaction::Stale.into();
                }

                let priority = block_number.saturated_into::<u64>();

                let mut tag = b"dvf_justification_".to_vec();
                justification.block_hash.encode_to(&mut tag);

                ValidTransaction::with_tag_prefix("Registry")
                    .priority(priority)
                    .and_provides(tag)
                    .longevity(5)
                    .propagate(true)
                    .build()
            } else if let Call::report_vote_equivocation { first, second } = call {
                if !Self::proves_vote_equivocation(first, second) {
                    return InvalidTransaction::BadProof.into();
                }
                if PenalisedVoteEquivocations::<T>::contains_key(
                    first.round_id,
                    &first.validator_account,
                ) {
                    return InvalidTransaction::Stale.into();
                }
                let mut tag = b"vote_equivocation_".to_vec();
                first.round_id.encode_to(&mut tag);
                first.validator_account.encode_to(&mut tag);
                ValidTransaction::with_tag_prefix("Registry")
                    .priority(u64::MAX / 2)
                    .and_provides(tag)
                    .longevity(64)
                    .propagate(true)
                    .build()
            } else {
                InvalidTransaction::Call.into()
            }
        }
    }
}

impl<T: Config>
    pallet_cerulea_pos::ValidatorHandler<T::AccountId, <T as pallet_cerulea_pos::Config>::Balance>
    for Pallet<T>
{
    fn on_joined(
        validator: &T::AccountId,
        stake: <T as pallet_cerulea_pos::Config>::Balance,
    ) -> sp_runtime::DispatchResult {
        let mut validator_set = ValidatorSet::<T>::get();
        if !validator_set.contains(validator) {
            ensure!(
                validator_set.len() < <T as Config>::MaxValidators::get() as usize,
                pallet_cerulea_pos::Error::<T>::TooManyValidators
            );
            validator_set
                .try_push(validator.clone())
                .map_err(|_| pallet_cerulea_pos::Error::<T>::TooManyValidators)?;
            ValidatorSet::<T>::put(validator_set);
        }

        if !ValidatorStates::<T>::contains_key(validator) {
            let current_epoch = pallet_cerulea_pos::Pallet::<T>::current_epoch();
            let stake_score = stake.saturated_into::<u64>();
            let inference_score = 0;
            let final_score = 50;

            let initial_stats = EpochStats {
                epoch: current_epoch,
                stake_score,
                inference_score,
                final_score,
                authored_blocks: 0,
                missed_blocks: 0,
            };

            let current_block = frame_system::Pallet::<T>::block_number().saturated_into::<u32>();
            let state = ValidatorState {
                last_active_epoch: current_epoch,
                current: initial_stats,
                history: BoundedVec::new(),
                uptime: 0,
                inference_success_count: 0,
                participation_rate: 0,
                inference_count: 0,
                last_active_block: current_block,
                name: None,
                trust_score: 5000,
            };
            ValidatorStates::<T>::insert(validator, state);
        }

        Ok(())
    }

    fn on_leave_requested(validator: &T::AccountId) -> sp_runtime::DispatchResult {
        let current_block = frame_system::Pallet::<T>::block_number().saturated_into::<u32>();
        ValidatorLeaveRequests::<T>::insert(validator, current_block);

        let mut active_validators = ActiveValidators::<T>::get();
        if let Some(pos) = active_validators.iter().position(|v| v == validator) {
            active_validators.remove(pos);
            ActiveValidators::<T>::put(active_validators);
        }
        Ok(())
    }

    fn on_left(validator: &T::AccountId) -> sp_runtime::DispatchResult {
        let mut validator_set = ValidatorSet::<T>::get();
        if let Some(pos) = validator_set.iter().position(|v| v == validator) {
            validator_set.remove(pos);
            ValidatorSet::<T>::put(validator_set);
        }

        let mut active_validators = ActiveValidators::<T>::get();
        if let Some(pos) = active_validators.iter().position(|v| v == validator) {
            active_validators.remove(pos);
            ActiveValidators::<T>::put(active_validators);
        }

        ValidatorJoinTime::<T>::remove(validator);
        ValidatorLeaveRequests::<T>::remove(validator);

        let current_block = frame_system::Pallet::<T>::block_number().saturated_into::<u32>();
        RecentlyRemovedValidators::<T>::insert(validator, current_block);

        Ok(())
    }

    fn on_stake_increased(
        validator: &T::AccountId,
        amount: <T as pallet_cerulea_pos::Config>::Balance,
    ) -> sp_runtime::DispatchResult {
        ValidatorStates::<T>::mutate(validator, |state| {
            if let Some(state) = state {
                state.current.stake_score = state
                    .current
                    .stake_score
                    .saturating_add(amount.saturated_into::<u64>());
            }
        });
        Ok(())
    }

    fn on_stake_decreased(
        validator: &T::AccountId,
        amount: <T as pallet_cerulea_pos::Config>::Balance,
    ) -> sp_runtime::DispatchResult {
        ValidatorStates::<T>::mutate(validator, |state| {
            if let Some(state) = state {
                state.current.stake_score = state
                    .current
                    .stake_score
                    .saturating_sub(amount.saturated_into::<u64>());
            }
        });
        Ok(())
    }

    fn on_slashed(
        validator: &T::AccountId,
        amount: <T as pallet_cerulea_pos::Config>::Balance,
        penalty: u64,
    ) -> sp_runtime::DispatchResult {
        ValidatorStates::<T>::mutate(validator, |state| {
            if let Some(state) = state {
                state.current.stake_score = state
                    .current
                    .stake_score
                    .saturating_sub(amount.saturated_into::<u64>());
                state.trust_score = state.trust_score.saturating_sub(penalty);
            }
        });
        Ok(())
    }

    fn on_rewarded(
        validator: &T::AccountId,
        amount: <T as pallet_cerulea_pos::Config>::Balance,
        boost: u64,
    ) -> sp_runtime::DispatchResult {
        ValidatorStates::<T>::mutate(validator, |state| {
            if let Some(state) = state {
                state.current.stake_score = state
                    .current
                    .stake_score
                    .saturating_add(amount.saturated_into::<u64>());
                state.trust_score = state.trust_score.saturating_add(boost);
            }
        });
        Ok(())
    }

    fn get_validator_score(validator: &T::AccountId) -> u64 {
        ValidatorStates::<T>::get(validator)
            .map(|s| s.current.final_score)
            .unwrap_or(0)
    }

    fn get_active_validators() -> Vec<T::AccountId> {
        ActiveValidators::<T>::get().to_vec()
    }
}

impl<T: Config>
    pallet_cerulea_dcf::traits::ValidatorRegistryProvider<
        T::AccountId,
        <T as pallet_cerulea_pos::Config>::Balance,
        BlockNumberFor<T>,
    > for Pallet<T>
{
    fn get_active_validators() -> Vec<T::AccountId> {
        ActiveValidators::<T>::get().to_vec()
    }

    fn draw_key_owner(draw_key: &[u8; 32]) -> Option<T::AccountId> {
        DrawKeyOwner::<T>::get(draw_key)
    }

    fn validator_seal_key(validator: &T::AccountId) -> Option<[u8; 32]> {
        ValidatorKeyRecords::<T>::get(validator).map(|set| set.seal)
    }

    fn get_validator_profile(
        validator: &T::AccountId,
    ) -> Option<
        pallet_cerulea_dcf::ValidatorProfile<
            T::AccountId,
            <T as pallet_cerulea_pos::Config>::Balance,
            BlockNumberFor<T>,
        >,
    > {
        let state = ValidatorStates::<T>::get(validator)?;
        let stake = pallet_cerulea_pos::Stake::<T>::get(validator);
        let poi_score = state.current.inference_score;
        let status = if ActiveValidators::<T>::get().contains(validator) {
            pallet_cerulea_dcf::ValidatorStatus::Active
        } else {
            pallet_cerulea_dcf::ValidatorStatus::Inactive
        };

        Some(pallet_cerulea_dcf::ValidatorProfile {
            stake,
            poi_score: poi_score as u32,
            final_score: state.current.final_score,
            trust_score: state.trust_score,
            status,
            inference_count: state.inference_count,
            name: state
                .name
                .map(|n| BoundedVec::truncate_from(n.into_inner())),
            last_active_block: state.last_active_block.into(),
            _phantom: sp_std::marker::PhantomData,
        })
    }

    fn get_validator_status(
        validator: &T::AccountId,
    ) -> Option<pallet_cerulea_dcf::ValidatorStatus> {
        if ActiveValidators::<T>::get().contains(validator) {
            Some(pallet_cerulea_dcf::ValidatorStatus::Active)
        } else if ValidatorSet::<T>::get().contains(validator) {
            Some(pallet_cerulea_dcf::ValidatorStatus::Inactive)
        } else if ValidatorLeaveRequests::<T>::contains_key(validator) {
            Some(pallet_cerulea_dcf::ValidatorStatus::Leaving)
        } else if RecentlyRemovedValidators::<T>::contains_key(validator) {
            Some(pallet_cerulea_dcf::ValidatorStatus::Ejected)
        } else {
            None
        }
    }

    fn get_validator_set() -> Vec<T::AccountId> {
        ValidatorSet::<T>::get().to_vec()
    }

    fn is_validator_active(validator: &T::AccountId) -> bool {
        ActiveValidators::<T>::get().contains(validator)
    }

    fn eject_validator(
        validator: &T::AccountId,
        reason: pallet_cerulea_dcf::EjectionReason,
    ) -> sp_runtime::DispatchResult {
        Self::eject_validator(validator, reason)
    }

    fn get_validator_state(
        validator: &T::AccountId,
    ) -> Option<pallet_cerulea_dcf::traits::ValidatorState> {
        let state = ValidatorStates::<T>::get(validator)?;
        Some(pallet_cerulea_dcf::traits::ValidatorState {
            last_active_epoch: state.last_active_epoch,
            current: state.current,
            history: state.history.into_inner(),
            uptime: state.uptime,
            inference_success_count: state.inference_success_count,
            participation_rate: state.participation_rate,
            inference_count: state.inference_count,
            last_active_block: state.last_active_block,
            name: state.name.map(|n| n.into_inner()),
            trust_score: state.trust_score,
        })
    }

    fn update_validator_state(
        validator: &T::AccountId,
        state: pallet_cerulea_dcf::traits::ValidatorState,
    ) {
        let name_bounded = state.name.map(BoundedVec::truncate_from);
        let history_bounded = BoundedVec::truncate_from(state.history);

        let local_state = ValidatorState::<T> {
            last_active_epoch: state.last_active_epoch,
            current: state.current,
            history: history_bounded,
            uptime: state.uptime,
            inference_success_count: state.inference_success_count,
            participation_rate: state.participation_rate,
            inference_count: state.inference_count,
            last_active_block: state.last_active_block,
            name: name_bounded,
            trust_score: state.trust_score,
        };
        ValidatorStates::<T>::insert(validator, local_state);
    }

    fn contains_validator_state(validator: &T::AccountId) -> bool {
        ValidatorStates::<T>::contains_key(validator)
    }

    fn remove_validator_state(validator: &T::AccountId) {
        ValidatorStates::<T>::remove(validator);
    }

    fn get_validator_name(validator: &T::AccountId) -> Option<Vec<u8>> {
        ValidatorNames::<T>::get(validator).map(|n| n.into_inner())
    }

    fn set_validator_name(validator: &T::AccountId, name: Vec<u8>) {
        ValidatorNames::<T>::insert(validator, BoundedVec::truncate_from(name));
    }

    fn get_validator_metadata(
        validator: &T::AccountId,
    ) -> Option<pallet_cerulea_dcf::ValidatorMetadataInfo> {
        ValidatorMetadata::<T>::get(validator)
    }

    fn set_validator_metadata(
        validator: &T::AccountId,
        metadata: pallet_cerulea_dcf::ValidatorMetadataInfo,
    ) {
        ValidatorMetadata::<T>::insert(validator, metadata);
    }

    fn get_validator_detailed_cooldown_status(validator: &T::AccountId) -> Option<(u32, bool)> {
        let current_block = frame_system::Pallet::<T>::block_number().saturated_into::<u32>();
        let cooldown_period = T::LeaveCooldown::get();

        if let Some(leave_request_block) = ValidatorLeaveRequests::<T>::get(validator) {
            let blocks_since_request = current_block.saturating_sub(leave_request_block);
            let blocks_remaining = cooldown_period.saturating_sub(blocks_since_request);
            return Some((blocks_remaining, false));
        }

        if let Some(removed_at_block) = RecentlyRemovedValidators::<T>::get(validator) {
            let blocks_since_removed = current_block.saturating_sub(removed_at_block);
            if blocks_since_removed < cooldown_period {
                let blocks_remaining = cooldown_period.saturating_sub(blocks_since_removed);
                return Some((blocks_remaining, false));
            } else {
                return Some((0, true));
            }
        }

        None
    }

    fn get_validator_leave_request(validator: &T::AccountId) -> Option<u32> {
        ValidatorLeaveRequests::<T>::get(validator)
    }

    fn get_pending_actions() -> Vec<(T::AccountId, pallet_cerulea_dcf::ValidatorAction)> {
        PendingValidatorActions::<T>::iter().collect()
    }

    fn remove_pending_action(validator: &T::AccountId) {
        PendingValidatorActions::<T>::remove(validator);
    }

    fn update_active_validators(active: Vec<T::AccountId>) {
        ActiveValidators::<T>::put(BoundedVec::truncate_from(active));
    }

    fn get_validator_performance_history(
        validator: &T::AccountId,
    ) -> Vec<pallet_cerulea_dcf::PerformanceRecord> {
        ValidatorPerformanceHistory::<T>::get(validator).into_inner()
    }

    fn set_validator_performance_history(
        validator: &T::AccountId,
        history: Vec<pallet_cerulea_dcf::PerformanceRecord>,
    ) {
        ValidatorPerformanceHistory::<T>::insert(validator, BoundedVec::truncate_from(history));
    }

    fn get_validator_last_seen(validator: &T::AccountId) -> u32 {
        ValidatorLastSeen::<T>::get(validator)
    }

    fn set_validator_last_seen(validator: &T::AccountId, val: u32) {
        ValidatorLastSeen::<T>::insert(validator, val);
    }

    fn get_validator_blocks_authored(validator: &T::AccountId) -> u32 {
        ValidatorBlocksAuthored::<T>::get(validator)
    }

    fn set_validator_blocks_authored(validator: &T::AccountId, val: u32) {
        ValidatorBlocksAuthored::<T>::insert(validator, val);
    }

    fn get_validator_blocks_missed(validator: &T::AccountId) -> u32 {
        ValidatorBlocksMissed::<T>::get(validator)
    }

    fn set_validator_blocks_missed(validator: &T::AccountId, val: u32) {
        ValidatorBlocksMissed::<T>::insert(validator, val);
    }

    fn update_validator_set(set: Vec<T::AccountId>) {
        ValidatorSet::<T>::put(BoundedVec::truncate_from(set));
    }

    fn remove_validator_metadata(validator: &T::AccountId) {
        ValidatorMetadata::<T>::remove(validator);
    }

    fn remove_validator_performance_history(validator: &T::AccountId) {
        ValidatorPerformanceHistory::<T>::remove(validator);
    }

    fn remove_validator_last_seen(validator: &T::AccountId) {
        ValidatorLastSeen::<T>::remove(validator);
    }

    fn remove_validator_blocks_authored(validator: &T::AccountId) {
        ValidatorBlocksAuthored::<T>::remove(validator);
    }

    fn remove_validator_blocks_missed(validator: &T::AccountId) {
        ValidatorBlocksMissed::<T>::remove(validator);
    }

    fn add_pending_action(validator: &T::AccountId, action: pallet_cerulea_dcf::ValidatorAction) {
        PendingValidatorActions::<T>::insert(validator, action);
    }

    fn get_validator_join_time(validator: &T::AccountId) -> Option<u32> {
        ValidatorJoinTime::<T>::get(validator)
    }

    fn get_recently_removed_validator(validator: &T::AccountId) -> Option<u32> {
        RecentlyRemovedValidators::<T>::get(validator)
    }

    fn set_validator_join_time(validator: &T::AccountId, val: u32) {
        ValidatorJoinTime::<T>::insert(validator, val);
    }

    fn remove_validator_join_time(validator: &T::AccountId) {
        ValidatorJoinTime::<T>::remove(validator);
    }

    fn set_validator_leave_request(validator: &T::AccountId, val: u32) {
        ValidatorLeaveRequests::<T>::insert(validator, val);
    }

    fn remove_validator_leave_request(validator: &T::AccountId) {
        ValidatorLeaveRequests::<T>::remove(validator);
    }

    fn set_recently_removed_validator(validator: &T::AccountId, val: u32) {
        RecentlyRemovedValidators::<T>::insert(validator, val);
    }

    fn remove_recently_removed_validator(validator: &T::AccountId) {
        RecentlyRemovedValidators::<T>::remove(validator);
    }
}

impl<T: Config> pallet_cerulea_dcf::traits::WeightFreezer<T::AccountId> for Pallet<T> {
    fn freeze_epoch_weights(epoch: u32, validators: &[(T::AccountId, u128, u128)]) {
        Self::freeze_epoch_weights(epoch, validators);
    }
}

impl<T: Config> Pallet<T> {
    /// Check that one account holds a validator licence that still holds.
    ///
    /// The check runs while the chain holds a key for validator licences. A
    /// chain that names no key admits through governance alone.
    ///
    /// The held licence meets the class the chain asks for at the moment of
    /// the join. A class requirement that root raised after the licence
    /// landed therefore applies to every later join.
    pub fn ensure_licensed(who: &T::AccountId) -> frame_support::dispatch::DispatchResult {
        if ValidatorLicenceKey::<T>::get().is_none() {
            return Ok(());
        }
        let held = ValidatorLicences::<T>::get(who).ok_or(Error::<T>::LicenceRequired)?;
        let now = frame_system::Pallet::<T>::block_number();
        ensure!(
            crate::engines::admission::licence::licence_holds(&held, now),
            Error::<T>::LicenceRequired
        );
        ensure!(
            held.class >= RequiredLicenceClass::<T>::get(),
            Error::<T>::LicenceClassTooLow
        );
        Ok(())
    }

    /// The module error for one Admission Engine licence fault.
    fn licence_error(
        fault: crate::engines::admission::licence::LicenceError,
    ) -> sp_runtime::DispatchError {
        match fault {
            crate::engines::admission::licence::LicenceError::SignatureRejected => {
                Error::<T>::LicenceSignatureRejected
            }
            crate::engines::admission::licence::LicenceError::Lapsed => Error::<T>::LicenceLapsed,
            crate::engines::admission::licence::LicenceError::NotYetActive => {
                Error::<T>::LicenceNotYetActive
            }
            crate::engines::admission::licence::LicenceError::NonceReplayed => {
                Error::<T>::LicenceReplayed
            }
            crate::engines::admission::licence::LicenceError::ClassTooLow => {
                Error::<T>::LicenceClassTooLow
            }
        }
        .into()
    }
}
