#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

use sp_runtime::SaturatedConversion;
use frame_support::traits::Get;
use frame_support::{ensure, BoundedVec};
use alloc::vec::Vec;
use frame_system::pallet_prelude::BlockNumberFor;
use pallet_cbc_dcf::EpochStats;

extern crate alloc;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[frame_support::pallet]
pub mod pallet {
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;
    use sp_runtime::traits::{SaturatedConversion, Verify, IdentifyAccount};
    use sp_runtime::transaction_validity::{
        InvalidTransaction, TransactionSource, TransactionValidity, ValidTransaction,
    };
    use sp_std::prelude::*;
    use codec::{Decode, Encode};
    use alloc::vec::Vec;
    use scale_info::TypeInfo;
    use codec::MaxEncodedLen;
    use frame_support::BoundedVec;
    use frame_support::traits::{Currency, ReservableCurrency, Get};

    type BalanceOf<T> = <T as pallet_cbc_pos::Config>::Balance;

    use pallet_cbc_dcf::{ValidatorStatus, EpochStats, EjectionReason};

    /// Local DVF ValidatorState structure to keep registry decoupled from DCF Config
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
    
    /// DVF Vote structure sent over the network and recorded in the runtime
    #[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen, frame_support::__private::codec::DecodeWithMemTracking)]
    pub struct DvfVote<Hash, AccountId, Signature> {
        pub epoch_id: u32,
        pub validator_set_id: u32,
        pub round_id: u32,
        pub block_number: u32,
        pub block_hash: Hash,
        pub validator_account: AccountId,
        pub signature: Signature, // Signature of the above fields
    }

    /// DVF Justification structure containing threshold-reaching votes
    #[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, frame_support::__private::codec::DecodeWithMemTracking)]
    pub struct DvfJustification<Hash, AccountId, Signature> {
        /// Round number for this justification
        pub round_number: u32,
        /// Block hash being justified
        pub block_hash: Hash,
        /// Collection of votes that justify this block
        pub votes: Vec<DvfVote<Hash, AccountId, Signature>>,
    }

sp_api::decl_runtime_apis! {
    /// DVF Runtime API for querying the DVF gadget network state.
    pub trait DvfApi<BlockNumber, AccountId, Hash>
    where
        BlockNumber: codec::Codec + Clone + Eq + sp_std::fmt::Debug,
        AccountId: codec::Codec + Clone + Eq + sp_std::fmt::Debug,
        Hash: codec::Codec + Clone + Eq + sp_std::fmt::Debug,
    {
        fn get_dvf_finalized_block() -> BlockNumber;
        fn get_current_epoch() -> u32;
        fn get_validator_set() -> sp_std::vec::Vec<AccountId>;
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
}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_cbc_pos::Config {
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
	}

    /// Frozen weights for active validators during the current epoch.
    #[pallet::storage]
    #[pallet::getter(fn epoch_voting_weight)]
    pub type EpochVotingWeight<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        u128,
        OptionQuery,
    >;

    /// Total voting weight distributed across all validators in the current epoch.
    #[pallet::storage]
    #[pallet::getter(fn total_voting_weight)]
    pub type TotalVotingWeight<T: Config> = StorageValue<_, u128, ValueQuery>;

    /// Current DVF voting round inside an epoch. Incremented after each successfully finalized block.
    #[pallet::storage]
    #[pallet::getter(fn current_round)]
    pub type CurrentRound<T: Config> = StorageValue<_, u32, ValueQuery>;

    /// Number of the highest finalized block via DVF.
    #[pallet::storage]
    #[pallet::getter(fn finalized_block_number)]
    pub type FinalizedBlockNumber<T: Config> = StorageValue<_, BlockNumberFor<T>, ValueQuery>;

    /// Hash of the highest finalized block via DVF.
    #[pallet::storage]
    #[pallet::getter(fn finalized_block_hash)]
    pub type FinalizedBlockHash<T: Config> = StorageValue<_, T::Hash, OptionQuery>;

    /// Stores the vote cast by a specific validator per round. Map key: (Round, AccountId)
    #[pallet::storage]
    pub type VoteRecords<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat, u32,  // Round
        Blake2_128Concat, T::AccountId, // AccountId
        DvfVote<T::Hash, T::AccountId, T::Signature>,
        OptionQuery,
    >;

    /// Running sum of weights voting for a specific block hash.
    #[pallet::storage]
    #[pallet::getter(fn vote_tallies)]
    pub type VoteTallies<T: Config> = StorageMap<
        _,
        Blake2_128Concat, T::Hash,
        u128,
        ValueQuery,
    >;
    
    /// The current Validator set ID to avoid out-of-date validators voting
    #[pallet::storage]
    #[pallet::getter(fn validator_set_id)]
    pub type ValidatorSetId<T: Config> = StorageValue<_, u32, ValueQuery>;

    /// The previous validator set for change detection
    #[pallet::storage]
    pub type PreviousValidatorSet<T: Config> = StorageValue<_, BoundedVec<T::AccountId, <T as Config>::MaxValidators>, ValueQuery>;

    /// The block number when the validator set ID last changed (for grace period)
    #[pallet::storage]
    pub type ValidatorSetIdChangedAt<T: Config> = StorageValue<_, BlockNumberFor<T>, OptionQuery>;

    // --- Validator Registry Storage Maps ---

    #[pallet::storage]
    #[pallet::getter(fn validator_set)]
    pub type ValidatorSet<T: Config> = StorageValue<
        _,
        BoundedVec<T::AccountId, <T as Config>::MaxValidators>,
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn active_validators)]
    pub type ActiveValidators<T: Config> = StorageValue<
        _,
        BoundedVec<T::AccountId, <T as Config>::MaxValidators>,
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn pending_validator_actions)]
    pub type PendingValidatorActions<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        pallet_cbc_dcf::ValidatorAction,
        OptionQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn validator_join_time)]
    pub type ValidatorJoinTime<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        u32,
        OptionQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn validator_leave_requests)]
    pub type ValidatorLeaveRequests<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        u32,
        OptionQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn recently_removed_validators)]
    pub type RecentlyRemovedValidators<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        u32,
        OptionQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn validator_states)]
    pub type ValidatorStates<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        ValidatorState<T>,
        OptionQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn validator_names)]
    pub type ValidatorNames<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        BoundedVec<u8, ConstU32<32>>,
        OptionQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn validator_uptime)]
    pub type ValidatorUptime<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        u32,
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn validator_metadata)]
    pub type ValidatorMetadata<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        pallet_cbc_dcf::ValidatorMetadataInfo,
        OptionQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn validator_performance_history)]
    pub type ValidatorPerformanceHistory<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        BoundedVec<pallet_cbc_dcf::PerformanceRecord, ConstU32<100>>,
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn validator_last_seen)]
    pub type ValidatorLastSeen<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        u32,
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn validator_blocks_authored)]
    pub type ValidatorBlocksAuthored<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        u32,
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn validator_blocks_missed)]
    pub type ValidatorBlocksMissed<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        u32,
        ValueQuery,
    >;

    /// Genesis configuration for the DVF pallet.
    #[pallet::genesis_config]
    #[derive(frame_support::DefaultNoBound)]
    pub struct GenesisConfig<T: Config> {
        pub initial_validator_weights: Vec<(T::AccountId, u128, u128)>,
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            assert!(
                !self.initial_validator_weights.is_empty(),
                "DVF genesis requires at least one validator with non-zero weight"
            );

            for (validator, stake, score) in &self.initial_validator_weights {
                assert!(
                    *stake > 0,
                    "DVF genesis validator {:?} has invalid stake (must be > 0)",
                    validator
                );
                assert!(
                    *score <= 100,
                    "DVF genesis validator {:?} has invalid score {} (must be 0-100)",
                    validator,
                    score
                );
            }

            log::info!(
                target: "runtime::dvf",
                "DVF genesis initializing with {} validators",
                self.initial_validator_weights.len()
            );

            ValidatorSetId::<T>::put(0);
            Pallet::<T>::freeze_epoch_weights(0, &self.initial_validator_weights);
            ValidatorSetId::<T>::put(0);

            log::info!(
                target: "runtime::dvf",
                "DVF genesis initialization complete - EpochVotingWeight populated for epoch 0"
            );
        }
    }

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A block was finalized by the DVF voting protocol.
		BlockFinalized { block_number: BlockNumberFor<T>, block_hash: T::Hash, round: u32, weight: u128 },
        /// DVF Voting Weights Frozen for a new epoch.
        WeightsFrozen { epoch: u32, total_weight: u128 },
        /// Validator set has changed and ValidatorSetId was incremented.
        ValidatorSetChanged { old_id: u32, new_id: u32 },
        /// Vote validation failed with a specific reason.
        VoteValidationFailed { validator: T::AccountId, block_number: BlockNumberFor<T>, reason: Vec<u8> },
        
        // Moved registry events
		ValidatorJoined { validator: T::AccountId, stake_amount: BalanceOf<T> },
		ValidatorLeft { validator: T::AccountId },
		ValidatorLeaveRequested { validator: T::AccountId, cooldown_expires_at: u32 },
		ValidatorLeaveCancelled { validator: T::AccountId },
		ValidatorAdded { validator: T::AccountId },
		ValidatorRemoved { validator: T::AccountId },
		ValidatorForcedToLeave { validator: T::AccountId },
		ValidatorStatusChanged {
			validator: T::AccountId,
			old_status: pallet_cbc_dcf::ValidatorStatus,
			new_status: pallet_cbc_dcf::ValidatorStatus,
			block_number: u32,
		},
		ValidatorEjected { validator: T::AccountId, reason: pallet_cbc_dcf::EjectionReason },
        CooldownExpired { validator: T::AccountId },
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
            Weight::zero()
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
                vote.signature.verify(&encoded_payload[..], &vote.validator_account),
                Error::<T>::InvalidSignature
            );
            
            let voter_weight = EpochVotingWeight::<T>::get(&vote.validator_account)
                .ok_or(Error::<T>::InvalidValidator)?;

            ensure!(
                Self::is_checkpoint_block(vote.block_number.saturated_into()),
                Error::<T>::NonCheckpointBlock
            );

            let current_finalized_number = FinalizedBlockNumber::<T>::get();
            ensure!(vote.block_number.saturated_into::<u32>() > current_finalized_number.saturated_into::<u32>(), Error::<T>::BlockAlreadyFinalized);

            let current_round = CurrentRound::<T>::get();
            ensure!(!VoteRecords::<T>::contains_key(current_round, &vote.validator_account), Error::<T>::DoubleVote);

            VoteRecords::<T>::insert(current_round, &vote.validator_account, vote.clone());
            
            let new_tally = VoteTallies::<T>::get(&vote.block_hash).saturating_add(voter_weight);
            VoteTallies::<T>::insert(&vote.block_hash, new_tally);

            let total_weight = TotalVotingWeight::<T>::get();
            let threshold_target = T::FinalityThreshold::get() * total_weight;

            if new_tally >= threshold_target {
                Self::trigger_finalization(vote.block_hash, vote.block_number.saturated_into(), current_round, new_tally);
            }

            Ok(())
        }

        /// Submit a DVF justification to finalize a block.
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

            // Already pending join or already active
            ensure!(
                !Self::active_validators().contains(&who),
                Error::<T>::NotAllowedInGovernanceMode 
            );
            ensure!(
                PendingValidatorActions::<T>::get(&who) != Some(pallet_cbc_dcf::ValidatorAction::Join),
                Error::<T>::NotAllowedInGovernanceMode
            );

            // Check minimum stake and score now, but actual addition is at epoch
            let stake = pallet_cbc_pos::Pallet::<T>::stake(&who);
            ensure!(
                stake >= T::MinStake::get(),
                Error::<T>::NotEnoughValidators
            );
            let state = ValidatorStates::<T>::get(&who).ok_or(Error::<T>::ValidatorNotFound)?;
            ensure!(
                state.current.final_score >= T::MinValidatorScore::get() as u64,
                Error::<T>::NotValidator
            );

            PendingValidatorActions::<T>::insert(&who, pallet_cbc_dcf::ValidatorAction::Join);
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
                PendingValidatorActions::<T>::get(&who) != Some(pallet_cbc_dcf::ValidatorAction::Leave),
                Error::<T>::NotAllowedInGovernanceMode
            );

            PendingValidatorActions::<T>::insert(&who, pallet_cbc_dcf::ValidatorAction::Leave);
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

            let min_stake = T::MinStake::get();
            let free_balance = <T::Currency as Currency<T::AccountId>>::free_balance(&who);
            ensure!(
                free_balance >= min_stake,
                pallet_cbc_pos::Error::<T>::InsufficientStake
            );

            Self::validate_rejoin_eligibility(&who)?;

            let mut validator_set = ValidatorSet::<T>::get();
            ensure!(
                !validator_set.contains(&who),
                pallet_cbc_pos::Error::<T>::ValidatorAlreadyExists
            );

            <T::Currency as ReservableCurrency<T::AccountId>>::reserve(&who, min_stake)
                .map_err(|_| pallet_cbc_pos::Error::<T>::InsufficientStake)?;

            pallet_cbc_pos::Stake::<T>::insert(&who, min_stake);
            ValidatorJoinTime::<T>::insert(&who, frame_system::Pallet::<T>::block_number().saturated_into::<u32>());

            ensure!(
                validator_set.len() < <T as Config>::MaxValidators::get() as usize,
                pallet_cbc_pos::Error::<T>::TooManyValidators
            );

            validator_set.try_push(who.clone())
                .map_err(|_| pallet_cbc_pos::Error::<T>::TooManyValidators)?;
            ValidatorSet::<T>::put(validator_set);

            pallet_cbc_pos::Validators::<T>::insert(&who, true);

            // Initialize ValidatorStates
            if !ValidatorStates::<T>::contains_key(&who) {
                let current_epoch = pallet_cbc_pos::Pallet::<T>::current_epoch();
                let initial_stats = EpochStats {
                    epoch: current_epoch,
                    stake_score: min_stake.saturated_into::<u64>(),
                    inference_score: 0,
                    final_score: 50,
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
                pallet_cbc_pos::Error::<T>::ValidatorNotInSet
            );

            Self::validate_leave_request_eligibility(&who)?;

            let current_block = frame_system::Pallet::<T>::block_number().saturated_into::<u32>();

            ValidatorLeaveRequests::<T>::insert(&who, current_block);

            let mut active_validators = ActiveValidators::<T>::get();
            if let Some(pos) = active_validators.iter().position(|v| v == &who) {
                active_validators.remove(pos);
                ActiveValidators::<T>::put(active_validators);
            }

            pallet_cbc_pos::Validators::<T>::insert(&who, false);

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
                .ok_or(pallet_cbc_pos::Error::<T>::ValidatorNotInSet)?;

            let current_block = frame_system::Pallet::<T>::block_number().saturated_into::<u32>();
            let cooldown_period = T::LeaveCooldown::get();
            let blocks_since_request = current_block.saturating_sub(leave_request_block);

            if blocks_since_request >= cooldown_period {
                return Err(pallet_cbc_pos::Error::<T>::LeaveCooldownActive.into());
            }

            ValidatorLeaveRequests::<T>::remove(&who);

            let validator_set = ValidatorSet::<T>::get();
            if validator_set.contains(&who) {
                let mut active_validators = ActiveValidators::<T>::get();
                if !active_validators.contains(&who) {
                    if active_validators.len() < <T as Config>::MaxValidators::get() as usize {
                        let _ = active_validators.try_push(who.clone());
                        ActiveValidators::<T>::put(active_validators);
                    }
                }
                pallet_cbc_pos::Validators::<T>::insert(&who, true);
            }

            Self::deposit_event(Event::ValidatorLeaveCancelled {
                validator: who.clone(),
            });

            Ok(())
        }
	}

    impl<T: Config> Pallet<T> {
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
        pub fn get_finality_info(block_number: BlockNumberFor<T>) -> FinalityInfo<BlockNumberFor<T>, T::Hash> {
            let finalized_checkpoint = FinalizedBlockNumber::<T>::get();
            
            if block_number <= finalized_checkpoint {
                let checkpoint_interval = T::FinalityCheckpointInterval::get();
                let providing_checkpoint = if checkpoint_interval > BlockNumberFor::<T>::from(0u32) {
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

        pub fn freeze_epoch_weights(epoch: u32, validators: &[(T::AccountId, u128, u128)]) {
            let new_validator_accounts: Vec<T::AccountId> = validators.iter().map(|(v, _, _)| v.clone()).collect();
            let previous_validator_set = PreviousValidatorSet::<T>::get();
            
            let validator_set_changed = if previous_validator_set.len() != new_validator_accounts.len() {
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
                BoundedVec::try_from(new_validator_accounts.clone())
                    .unwrap_or_else(|_| {
                        let truncated: Vec<T::AccountId> = new_validator_accounts
                            .into_iter()
                            .take(<T as Config>::MaxValidators::get() as usize)
                            .collect();
                        BoundedVec::truncate_from(truncated)
                    });
            PreviousValidatorSet::<T>::put(new_validator_set);
            
            let _ = EpochVotingWeight::<T>::clear(u32::MAX, None);

            const VOTE_WEIGHT_SCALE: u128 = 32_000u128;
            let validator_count = validators.len() as u128;
            let total_stake: u128 = validators.iter().map(|(_, stake, _)| *stake).sum();
            let mut total_epoch_weight = 0u128;

            for (validator, stake, _score) in validators {
                let normalized_weight = if total_stake > 0 {
                    stake.saturating_mul(VOTE_WEIGHT_SCALE) / total_stake
                } else {
                    if validator_count > 0 { VOTE_WEIGHT_SCALE / validator_count } else { 0 }
                };

                EpochVotingWeight::<T>::insert(validator, normalized_weight);
                total_epoch_weight = total_epoch_weight.saturating_add(normalized_weight);
            }

            TotalVotingWeight::<T>::put(total_epoch_weight);
            Self::deposit_event(Event::WeightsFrozen { epoch, total_weight: total_epoch_weight });
        }

        /// Verify and finalize a justification
        pub fn verify_and_finalize_justification(
            justification: DvfJustification<T::Hash, T::AccountId, T::Signature>,
        ) -> DispatchResult {
            log::info!(
                "DVF Pallet: verify_and_finalize_justification called for block {:?}, round {}",
                justification.block_hash,
                justification.round_number
            );

            ensure!(!justification.votes.is_empty(), Error::<T>::EmptyJustification);

            let block_number: BlockNumberFor<T> = justification.votes[0].block_number.saturated_into();

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
                    vote.signature.verify(&encoded_payload[..], &vote.validator_account),
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

                accumulated_weight = accumulated_weight.saturating_add(validator_weight);
            }

            let total_weight = TotalVotingWeight::<T>::get();
            let threshold = T::FinalityThreshold::get() * total_weight;

            ensure!(
                accumulated_weight >= threshold,
                Error::<T>::ThresholdNotReached
            );

            Self::finalize_block(
                justification.block_hash,
                block_number,
                justification.round_number,
                accumulated_weight,
            );

            log::info!(
                "DVF Pallet: Successfully verified and finalized block #{:?} via justification in round {}",
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

            let next_round = round.saturating_add(1);
            CurrentRound::<T>::put(next_round);

            Self::deposit_event(Event::BlockFinalized {
                block_number,
                block_hash,
                round,
                weight: accumulated_weight,
            });

            Self::prune_vote_records(round);
            VoteTallies::<T>::remove(&block_hash);
        }

        fn prune_vote_records(current_round: u32) {
            let retention_rounds = T::VoteRetentionRounds::get();
            
            if current_round > retention_rounds {
                let cutoff_round = current_round.saturating_sub(retention_rounds);
                
                for old_round in 0..cutoff_round {
                    let _ = VoteRecords::<T>::clear_prefix(old_round, u32::MAX, None);
                }
            }
        }

        fn trigger_finalization(block_hash: T::Hash, block_number: BlockNumberFor<T>, round: u32, tally_weight: u128) {
            Self::finalize_block(block_hash, block_number, round, tally_weight);
        }

        // --- Validator Registry Helpers ---

        fn validate_rejoin_eligibility(who: &T::AccountId) -> DispatchResult {
            if let Some(left_at_block) = RecentlyRemovedValidators::<T>::get(who) {
                let current_block = frame_system::Pallet::<T>::block_number().saturated_into::<u32>();
                let cooldown_period = T::LeaveCooldown::get();
                let blocks_since_left = current_block.saturating_sub(left_at_block);

                if blocks_since_left < cooldown_period {
                    return Err(pallet_cbc_pos::Error::<T>::CooldownActive.into());
                }

                RecentlyRemovedValidators::<T>::remove(who);
            }

            if ValidatorLeaveRequests::<T>::contains_key(who) {
                return Err(pallet_cbc_pos::Error::<T>::LeaveCooldownActive.into());
            }

            Ok(())
        }

        fn validate_leave_request_eligibility(who: &T::AccountId) -> DispatchResult {
            ensure!(
                !ValidatorLeaveRequests::<T>::contains_key(who),
                pallet_cbc_pos::Error::<T>::LeaveCooldownActive
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
                let stake_amount = pallet_cbc_pos::Stake::<T>::get(&validator);
                if stake_amount > BalanceOf::<T>::default() {
                    let _ = <T::Currency as ReservableCurrency<T::AccountId>>::unreserve(&validator, stake_amount);
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

                pallet_cbc_pos::Validators::<T>::remove(&validator);
                ValidatorJoinTime::<T>::remove(&validator);
                pallet_cbc_pos::Stake::<T>::remove(&validator);
                ValidatorLeaveRequests::<T>::remove(&validator);

                RecentlyRemovedValidators::<T>::insert(&validator, current_block);

                Self::deposit_event(Event::ValidatorLeft { validator });
            }
        }

        fn check_and_handle_underperforming_validators() {
            let min_score = T::MinValidatorScore::get() as u64;
            let current_epoch = pallet_cbc_pos::Pallet::<T>::current_epoch();
            
            let validators_to_check: Vec<T::AccountId> = ActiveValidators::<T>::get().into_inner();
            
            for validator in validators_to_check {
                if let Some(state) = ValidatorStates::<T>::get(&validator) {
                    if state.current.final_score < min_score {
                        log::warn!("DVF: Validator {:?} has low score: {}, ejecting", 
                                  validator, state.current.final_score);
                        let _ = Self::eject_validator(&validator, EjectionReason::ScoreBelowThreshold);
                    }
                    
                    let inactive_epochs = current_epoch.saturating_sub(state.last_active_epoch);
                    if inactive_epochs > T::MaxInactiveEpochs::get() {
                        log::warn!("DVF: Validator {:?} inactive for {} epochs, ejecting", 
                                  validator, inactive_epochs);
                        let _ = Self::eject_validator(&validator, EjectionReason::ScoreBelowThreshold);
                    }
                }
            }
        }

        pub fn eject_validator(validator: &T::AccountId, reason: EjectionReason) -> DispatchResult {
            let mut active_validators = ActiveValidators::<T>::get();
            let pos_found = if let Some(pos) = active_validators.iter().position(|v| v == validator) {
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

        fn validate_unsigned(
            _source: TransactionSource,
            call: &Self::Call,
        ) -> TransactionValidity {
            if let Call::submit_justification { justification } = call {
                if justification.votes.is_empty() {
                    return InvalidTransaction::Custom(1).into();
                }

                let block_number: BlockNumberFor<T> = justification.votes[0].block_number.saturated_into();
                let current_finalized_number = FinalizedBlockNumber::<T>::get();

                if block_number <= current_finalized_number {
                    return InvalidTransaction::Stale.into();
                }

                let priority = block_number.saturated_into::<u64>();

                let mut tag = b"dvf_justification_".to_vec();
                justification.block_hash.encode_to(&mut tag);

                ValidTransaction::with_tag_prefix("Dvf")
                    .priority(priority)
                    .and_provides(tag)
                    .longevity(5)
                    .propagate(true)
                    .build()
            } else {
                InvalidTransaction::Call.into()
            }
        }
    }
}

impl<T: Config> pallet_cbc_pos::ValidatorHandler<T::AccountId, <T as pallet_cbc_pos::Config>::Balance> for Pallet<T> {
    fn on_joined(validator: &T::AccountId, stake: <T as pallet_cbc_pos::Config>::Balance) -> sp_runtime::DispatchResult {
        let mut validator_set = ValidatorSet::<T>::get();
        if !validator_set.contains(validator) {
            ensure!(
                validator_set.len() < <T as Config>::MaxValidators::get() as usize,
                pallet_cbc_pos::Error::<T>::TooManyValidators
            );
            validator_set.try_push(validator.clone())
                .map_err(|_| pallet_cbc_pos::Error::<T>::TooManyValidators)?;
            ValidatorSet::<T>::put(validator_set);
        }

        if !ValidatorStates::<T>::contains_key(validator) {
            let current_epoch = pallet_cbc_pos::Pallet::<T>::current_epoch();
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

    fn on_stake_increased(validator: &T::AccountId, amount: <T as pallet_cbc_pos::Config>::Balance) -> sp_runtime::DispatchResult {
        ValidatorStates::<T>::mutate(validator, |state| {
            if let Some(state) = state {
                state.current.stake_score = state.current.stake_score.saturating_add(amount.saturated_into::<u64>());
            }
        });
        Ok(())
    }

    fn on_stake_decreased(validator: &T::AccountId, amount: <T as pallet_cbc_pos::Config>::Balance) -> sp_runtime::DispatchResult {
        ValidatorStates::<T>::mutate(validator, |state| {
            if let Some(state) = state {
                state.current.stake_score = state.current.stake_score.saturating_sub(amount.saturated_into::<u64>());
            }
        });
        Ok(())
    }

    fn on_slashed(validator: &T::AccountId, amount: <T as pallet_cbc_pos::Config>::Balance, penalty: u64) -> sp_runtime::DispatchResult {
        ValidatorStates::<T>::mutate(validator, |state| {
            if let Some(state) = state {
                state.current.stake_score = state.current.stake_score.saturating_sub(amount.saturated_into::<u64>());
                state.trust_score = state.trust_score.saturating_sub(penalty);
            }
        });
        Ok(())
    }

    fn on_rewarded(validator: &T::AccountId, amount: <T as pallet_cbc_pos::Config>::Balance, boost: u64) -> sp_runtime::DispatchResult {
        ValidatorStates::<T>::mutate(validator, |state| {
            if let Some(state) = state {
                state.current.stake_score = state.current.stake_score.saturating_add(amount.saturated_into::<u64>());
                state.trust_score = state.trust_score.saturating_add(boost);
            }
        });
        Ok(())
    }

    fn get_validator_score(validator: &T::AccountId) -> u64 {
        ValidatorStates::<T>::get(validator).map(|s| s.current.final_score).unwrap_or(0)
    }

    fn get_active_validators() -> Vec<T::AccountId> {
        ActiveValidators::<T>::get().to_vec()
    }
}

impl<T: Config> pallet_cbc_dcf::traits::ValidatorRegistryProvider<T::AccountId, <T as pallet_cbc_pos::Config>::Balance, BlockNumberFor<T>> for Pallet<T> {
    fn get_active_validators() -> Vec<T::AccountId> {
        ActiveValidators::<T>::get().to_vec()
    }

    fn get_validator_profile(validator: &T::AccountId) -> Option<pallet_cbc_dcf::ValidatorProfile<T::AccountId, <T as pallet_cbc_pos::Config>::Balance, BlockNumberFor<T>>> {
        let state = ValidatorStates::<T>::get(validator)?;
        let stake = pallet_cbc_pos::Stake::<T>::get(validator);
        let poi_score = state.current.inference_score;
        let status = if ActiveValidators::<T>::get().contains(validator) {
            pallet_cbc_dcf::ValidatorStatus::Active
        } else {
            pallet_cbc_dcf::ValidatorStatus::Inactive
        };

        Some(pallet_cbc_dcf::ValidatorProfile {
            stake,
            poi_score: poi_score as u32,
            final_score: state.current.final_score,
            trust_score: state.trust_score,
            status,
            inference_count: state.inference_count,
            name: state.name.map(|n| BoundedVec::truncate_from(n.into_inner())),
            last_active_block: state.last_active_block.into(),
            _phantom: sp_std::marker::PhantomData,
        })
    }

    fn get_validator_status(validator: &T::AccountId) -> Option<pallet_cbc_dcf::ValidatorStatus> {
        if ActiveValidators::<T>::get().contains(validator) {
            Some(pallet_cbc_dcf::ValidatorStatus::Active)
        } else if ValidatorSet::<T>::get().contains(validator) {
            Some(pallet_cbc_dcf::ValidatorStatus::Inactive)
        } else if ValidatorLeaveRequests::<T>::contains_key(validator) {
            Some(pallet_cbc_dcf::ValidatorStatus::Leaving)
        } else if RecentlyRemovedValidators::<T>::contains_key(validator) {
            Some(pallet_cbc_dcf::ValidatorStatus::Ejected)
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

    fn eject_validator(validator: &T::AccountId, reason: pallet_cbc_dcf::EjectionReason) -> sp_runtime::DispatchResult {
        Self::eject_validator(validator, reason)
    }

    fn get_validator_state(validator: &T::AccountId) -> Option<pallet_cbc_dcf::traits::ValidatorState> {
        let state = ValidatorStates::<T>::get(validator)?;
        Some(pallet_cbc_dcf::traits::ValidatorState {
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

    fn update_validator_state(validator: &T::AccountId, state: pallet_cbc_dcf::traits::ValidatorState) {
        let name_bounded = state.name.map(|n| BoundedVec::truncate_from(n));
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

    fn get_validator_metadata(validator: &T::AccountId) -> Option<pallet_cbc_dcf::ValidatorMetadataInfo> {
        ValidatorMetadata::<T>::get(validator)
    }

    fn set_validator_metadata(validator: &T::AccountId, metadata: pallet_cbc_dcf::ValidatorMetadataInfo) {
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

    fn get_pending_actions() -> Vec<(T::AccountId, pallet_cbc_dcf::ValidatorAction)> {
        PendingValidatorActions::<T>::iter().collect()
    }

    fn remove_pending_action(validator: &T::AccountId) {
        PendingValidatorActions::<T>::remove(validator);
    }

    fn update_active_validators(active: Vec<T::AccountId>) {
        ActiveValidators::<T>::put(BoundedVec::truncate_from(active));
    }

    fn get_validator_performance_history(validator: &T::AccountId) -> Vec<pallet_cbc_dcf::PerformanceRecord> {
        ValidatorPerformanceHistory::<T>::get(validator).into_inner()
    }

    fn set_validator_performance_history(validator: &T::AccountId, history: Vec<pallet_cbc_dcf::PerformanceRecord>) {
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

    fn add_pending_action(validator: &T::AccountId, action: pallet_cbc_dcf::ValidatorAction) {
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

impl<T: Config> pallet_cbc_dcf::traits::WeightFreezer<T::AccountId> for Pallet<T> {
    fn freeze_epoch_weights(epoch: u32, validators: &[(T::AccountId, u128, u128)]) {
        Self::freeze_epoch_weights(epoch, validators);
    }
}

impl<T: Config> pallet_cbc_dcf::traits::DvfFinalizedBlockProvider for Pallet<T> {
    fn dvf_finalized_block() -> u32 {
        use sp_runtime::traits::SaturatedConversion;
        FinalizedBlockNumber::<T>::get().saturated_into::<u32>()
    }
}
