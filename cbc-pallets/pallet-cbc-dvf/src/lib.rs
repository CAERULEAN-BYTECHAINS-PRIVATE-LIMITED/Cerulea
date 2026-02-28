#![cfg_attr(not(feature = "std"), no_std)]

/// Edit this file to define custom logic or remove it if it is not needed.
/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// <https://docs.substrate.io/reference/frame-pallets/>
pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[frame_support::pallet]
pub mod pallet {
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;
    use sp_runtime::traits::{SaturatedConversion, Verify, IdentifyAccount};
    use sp_std::prelude::*;
    use codec::{Decode, Encode};
    use scale_info::TypeInfo;
    use codec::MaxEncodedLen;
    
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
    }
}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
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


	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A block was finalized by the DVF voting protocol.
		BlockFinalized { block_number: BlockNumberFor<T>, block_hash: T::Hash, round: u32, weight: u128 },
        /// DVF Voting Weights Frozen for a new epoch.
        WeightsFrozen { epoch: u32, total_weight: u128 },
        /// Vote validation failed with a specific reason.
        VoteValidationFailed { validator: T::AccountId, block_number: BlockNumberFor<T>, reason: Vec<u8> },
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
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
        /// Primary Extrinsic / Inherent entrypoint to submit a vote.
        /// 
        /// Nodes aggregate gossip validations and insert them via here.
        #[pallet::call_index(0)]
        #[pallet::weight(Weight::from_parts(10_000, 0) + T::DbWeight::get().reads_writes(1,1))]
        pub fn submit_dvf_vote(
            origin: OriginFor<T>,
            vote: DvfVote<T::Hash, T::AccountId, T::Signature>,
        ) -> DispatchResult {
            // Validate origin (could be inherent/none or signed)
            // ensure_none(origin) if via inherent, or ensure_signed if real extrinsic.
            ensure_signed(origin)?;

            // Validation Rules
            // 0. Verify Signature
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
            
            // 1. Validator must have weight in this epoch (frozen active set)
            let voter_weight = EpochVotingWeight::<T>::get(&vote.validator_account)
                .ok_or(Error::<T>::InvalidValidator)?;

            // 2. Ensure block number is a checkpoint block
            ensure!(
                Self::is_checkpoint_block(vote.block_number.saturated_into()),
                Error::<T>::NonCheckpointBlock
            );

            // 3. Ensure block number is above `FinalizedBlockNumber`
            let current_finalized_number = FinalizedBlockNumber::<T>::get();
            ensure!(vote.block_number.saturated_into::<u32>() > current_finalized_number.saturated_into::<u32>(), Error::<T>::BlockAlreadyFinalized);

            // 4. Prevent Double Vote in Same Round
            let current_round = CurrentRound::<T>::get();
            ensure!(!VoteRecords::<T>::contains_key(current_round, &vote.validator_account), Error::<T>::DoubleVote);

            // Insert Vote Records and execute Tally Phase
            VoteRecords::<T>::insert(current_round, &vote.validator_account, vote.clone());
            
            let new_tally = VoteTallies::<T>::get(&vote.block_hash).saturating_add(voter_weight);
            VoteTallies::<T>::insert(&vote.block_hash, new_tally);

            // Check Finality Threshold Execution Trigger
            let total_weight = TotalVotingWeight::<T>::get();
            let threshold_target = T::FinalityThreshold::get() * total_weight;

            if new_tally >= threshold_target {
                Self::trigger_finalization(vote.block_hash, vote.block_number.saturated_into(), current_round, new_tally);
            }

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
        /// All blocks up to and including the last finalized checkpoint are considered finalized
        pub fn is_block_finalized(block_number: BlockNumberFor<T>) -> bool {
            let finalized_checkpoint = FinalizedBlockNumber::<T>::get();
            block_number <= finalized_checkpoint
        }

        /// Get detailed finality information for a block number
        /// Returns finality status and the checkpoint that provides the finality guarantee
        pub fn get_finality_info(block_number: BlockNumberFor<T>) -> FinalityInfo<BlockNumberFor<T>, T::Hash> {
            let finalized_checkpoint = FinalizedBlockNumber::<T>::get();
            
            if block_number <= finalized_checkpoint {
                // Block is finalized - find the checkpoint that provides finality
                let checkpoint_interval = T::FinalityCheckpointInterval::get();
                let providing_checkpoint = if checkpoint_interval > BlockNumberFor::<T>::from(0u32) {
                    // Calculate the checkpoint that covers this block
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
            // (Validator, Stake, Final_Score) from DCF/PoS pallets
            let stake_factor = T::StakeWeightFactor::get();
            let score_factor = T::ScoreWeightFactor::get();
            let cap = T::ScoreBoostCap::get();
            
            let mut total_epoch_weight = 0u128;
            let _ = EpochVotingWeight::<T>::clear(u32::MAX, None); // Clear old tracking 

            for (validator, stake, score) in validators {
                let stake_weight = stake.saturating_mul(stake_factor);
                let mut score_weight = score.saturating_mul(score_factor);
                if score_weight > cap {
                    score_weight = cap;
                }
                
                let total_weight = stake_weight.saturating_add(score_weight);
                EpochVotingWeight::<T>::insert(validator, total_weight);
                total_epoch_weight = total_epoch_weight.saturating_add(total_weight);
            }

            TotalVotingWeight::<T>::put(total_epoch_weight);
            Self::deposit_event(Event::WeightsFrozen { epoch, total_weight: total_epoch_weight });
        }

        fn trigger_finalization(block_hash: T::Hash, block_number: BlockNumberFor<T>, round: u32, tally_weight: u128) {
            FinalizedBlockNumber::<T>::put(block_number);
            FinalizedBlockHash::<T>::put(block_hash);

            let next_round = round.saturating_add(1);
            CurrentRound::<T>::put(next_round);

            Self::deposit_event(Event::BlockFinalized {
                block_number,
                block_hash,
                round,
                weight: tally_weight,
            });

            // Prune older VoteRecords based on VoteRetentionRounds
            let retention_rounds = T::VoteRetentionRounds::get();
            if round > retention_rounds {
                let old_round = round.saturating_sub(retention_rounds);
                let _ = VoteRecords::<T>::clear_prefix(old_round, u32::MAX, None);
            }
        }
    }
}

impl<T: Config> pallet_cbc_dcf::traits::WeightFreezer<T::AccountId> for Pallet<T> {
    fn freeze_epoch_weights(epoch: u32, validators: &[(T::AccountId, u128, u128)]) {
        Self::freeze_epoch_weights(epoch, validators);
    }
}
