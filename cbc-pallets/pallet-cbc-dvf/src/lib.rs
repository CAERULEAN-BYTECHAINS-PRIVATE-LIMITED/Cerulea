#![cfg_attr(not(feature = "std"), no_std)]

/// Edit this file to define custom logic or remove it if it is not needed.
/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// <https://docs.substrate.io/reference/frame-pallets/>
pub use pallet::*;

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
        
        /// Maximum number of validators in the validator set.
        #[pallet::constant]
        type MaxValidators: Get<u32>;
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
    pub type PreviousValidatorSet<T: Config> = StorageValue<_, BoundedVec<T::AccountId, T::MaxValidators>, ValueQuery>;

    /// The block number when the validator set ID last changed (for grace period)
    #[pallet::storage]
    pub type ValidatorSetIdChangedAt<T: Config> = StorageValue<_, BlockNumberFor<T>, OptionQuery>;

    /// Genesis configuration for the DVF pallet.
    /// 
    /// This configuration initializes validator voting weights at chain genesis,
    /// ensuring that finalization can succeed from the very first checkpoint block.
    /// The weights are calculated using the same freeze_epoch_weights() logic that
    /// is used at epoch boundaries, maintaining consistency throughout the chain lifetime.
    /// 
    /// # Example
    /// 
    /// ```ignore
    /// GenesisConfig {
    ///     initial_validator_weights: vec![
    ///         (alice_account, 10_000_000, 80),  // (AccountId, Stake, Score)
    ///         (bob_account, 8_000_000, 80),
    ///         (charlie_account, 6_000_000, 80),
    ///     ],
    /// }
    /// ```
    #[pallet::genesis_config]
    #[derive(frame_support::DefaultNoBound)]
    pub struct GenesisConfig<T: Config> {
        /// Initial validator weights: (AccountId, Stake, Score)
        /// 
        /// Each tuple contains:
        /// - AccountId: The validator's account identifier
        /// - Stake: The validator's staked amount (in smallest unit)
        /// - Score: The validator's DCF score (0-100 range)
        /// 
        /// These values should be synchronized with the DCF pallet's genesis validators
        /// to ensure consistency between validator set membership and voting weights.
        pub initial_validator_weights: Vec<(T::AccountId, u128, u128)>,
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            // Validate genesis configuration
            assert!(
                !self.initial_validator_weights.is_empty(),
                "DVF genesis requires at least one validator with non-zero weight"
            );

            // Validate stakes and scores
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

            // Log genesis initialization for audit trail
            log::info!(
                target: "runtime::dvf",
                "DVF genesis initializing with {} validators",
                self.initial_validator_weights.len()
            );

            // Initialize ValidatorSetId to 0 at genesis
            ValidatorSetId::<T>::put(0);

            // Reuse existing freeze_epoch_weights logic for consistency
            // This ensures genesis weight calculation matches epoch transition logic
            Pallet::<T>::freeze_epoch_weights(0, &self.initial_validator_weights);

            // Reset ValidatorSetId back to 0 after freeze_epoch_weights
            // (freeze_epoch_weights increments it to 1 when it detects the initial validator set)
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

        /// Submit a DVF justification to finalize a block.
        /// 
        /// This extrinsic is called by the block import pipeline when a justification
        /// is received. It verifies the justification and updates finality state atomically.
        #[pallet::call_index(1)]
        #[pallet::weight(Weight::from_parts(100_000, 0) + T::DbWeight::get().reads_writes(10, 10))]
        pub fn submit_justification(
            origin: OriginFor<T>,
            justification: DvfJustification<T::Hash, T::AccountId, T::Signature>,
        ) -> DispatchResult {
            // Unsigned extrinsics always have a None origin when executed
            ensure_none(origin)?;

            // Verify and process the justification
            Self::verify_and_finalize_justification(justification)?;

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
            
            // Detect validator set changes
            let new_validator_accounts: Vec<T::AccountId> = validators.iter().map(|(v, _, _)| v.clone()).collect();
            let previous_validator_set = PreviousValidatorSet::<T>::get();
            
            // Check if validator set composition changed
            let validator_set_changed = if previous_validator_set.len() != new_validator_accounts.len() {
                true
            } else {
                // Check if all validators are the same (order doesn't matter)
                let mut prev_sorted: Vec<T::AccountId> = previous_validator_set.to_vec();
                prev_sorted.sort();
                let mut new_sorted = new_validator_accounts.clone();
                new_sorted.sort();
                prev_sorted != new_sorted
            };
            
            // Increment ValidatorSetId if validator set changed
            if validator_set_changed {
                let old_id = ValidatorSetId::<T>::get();
                let new_id = old_id.saturating_add(1);
                ValidatorSetId::<T>::put(new_id);
                
                // Store the current block number for grace period tracking
                let current_block = frame_system::Pallet::<T>::block_number();
                ValidatorSetIdChangedAt::<T>::put(current_block);
                
                Self::deposit_event(Event::ValidatorSetChanged { old_id, new_id });
            }
            
            // Store the new validator set for next comparison
            // Convert to BoundedVec, truncating if necessary
            let new_validator_set: BoundedVec<T::AccountId, T::MaxValidators> = 
                BoundedVec::try_from(new_validator_accounts.clone())
                    .unwrap_or_else(|_| {
                        // If conversion fails (too many validators), truncate to max
                        let truncated: Vec<T::AccountId> = new_validator_accounts
                            .into_iter()
                            .take(T::MaxValidators::get() as usize)
                            .collect();
                        BoundedVec::truncate_from(truncated)
                    });
            PreviousValidatorSet::<T>::put(new_validator_set);
            
            let _ = EpochVotingWeight::<T>::clear(u32::MAX, None); // Clear old weights

            // Normalize weights to VOTE_WEIGHT_SCALE per validator to prevent u64 overflow.
            // weight = (stake * VOTE_WEIGHT_SCALE) / total_stake using u128 arithmetic.
            // If total_stake == 0, assign VOTE_WEIGHT_SCALE / validator_count to each.
            const VOTE_WEIGHT_SCALE: u128 = 32_000u128;
            let validator_count = validators.len() as u128;

            let total_stake: u128 = validators.iter().map(|(_, stake, _)| *stake).sum();

            let mut total_epoch_weight = 0u128;

            for (validator, stake, _score) in validators {
                let normalized_weight = if total_stake > 0 {
                    // Use u128 intermediate arithmetic to avoid overflow
                    stake.saturating_mul(VOTE_WEIGHT_SCALE) / total_stake
                } else {
                    // Equal weights when total stake is zero
                    if validator_count > 0 { VOTE_WEIGHT_SCALE / validator_count } else { 0 }
                };

                EpochVotingWeight::<T>::insert(validator, normalized_weight);
                total_epoch_weight = total_epoch_weight.saturating_add(normalized_weight);
            }

            TotalVotingWeight::<T>::put(total_epoch_weight);
            Self::deposit_event(Event::WeightsFrozen { epoch, total_weight: total_epoch_weight });
        }

        /// Verify and finalize a justification
        /// 
        /// This method verifies all aspects of a justification and updates finality state atomically.
        /// It performs the following checks:
        /// - Justification is not empty
        /// - Block number is a checkpoint
        /// - Block is not already finalized
        /// - All votes have valid signatures
        /// - All votes are from active validators
        /// - All votes have matching validator_set_id
        /// - All votes have matching round_number
        /// - All votes have matching block_hash
        /// - No duplicate validators
        /// - Accumulated weight meets threshold
        /// 
        /// If all checks pass, it updates FinalizedBlockNumber, FinalizedBlockHash,
        /// increments CurrentRound, emits BlockFinalized event, and prunes old vote records.
        pub fn verify_and_finalize_justification(
            justification: DvfJustification<T::Hash, T::AccountId, T::Signature>,
        ) -> DispatchResult {
            log::info!(
                "DVF Pallet: verify_and_finalize_justification called for block {:?}, round {}",
                justification.block_hash,
                justification.round_number
            );

            // 1. Check justification is not empty
            ensure!(!justification.votes.is_empty(), Error::<T>::EmptyJustification);

            // 2. Extract block number from first vote (all should match)
            let block_number: BlockNumberFor<T> = justification.votes[0].block_number.saturated_into();

            // 3. Verify block number is a checkpoint
            ensure!(
                Self::is_checkpoint_block(block_number),
                Error::<T>::NonCheckpointBlock
            );

            // 4. Ensure block is not already finalized
            let current_finalized_number = FinalizedBlockNumber::<T>::get();
            ensure!(
                block_number > current_finalized_number,
                Error::<T>::BlockAlreadyFinalized
            );

            // 5. Verify all votes and accumulate weight
            let mut seen_validators = sp_std::collections::btree_set::BTreeSet::new();
            let mut accumulated_weight = 0u128;
            let mut expected_validator_set_id: Option<u32> = None;

            for vote in &justification.votes {
                // Check for duplicate validators
                ensure!(
                    seen_validators.insert(vote.validator_account.clone()),
                    Error::<T>::DuplicateValidator
                );

                // Verify signature
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

                // Check validator set ID consistency
                if let Some(expected_id) = expected_validator_set_id {
                    ensure!(
                        vote.validator_set_id == expected_id,
                        Error::<T>::ValidatorSetIdMismatch
                    );
                } else {
                    expected_validator_set_id = Some(vote.validator_set_id);
                }

                // Check round consistency
                ensure!(
                    vote.round_id == justification.round_number,
                    Error::<T>::RoundMismatch
                );

                // Check block hash consistency
                ensure!(
                    vote.block_hash == justification.block_hash,
                    Error::<T>::BlockHashMismatch
                );

                // Verify validator is active and get weight
                let validator_weight = EpochVotingWeight::<T>::get(&vote.validator_account)
                    .ok_or(Error::<T>::InvalidValidator)?;

                accumulated_weight = accumulated_weight.saturating_add(validator_weight);
            }

            // 6. Verify accumulated weight meets threshold
            let total_weight = TotalVotingWeight::<T>::get();
            let threshold = T::FinalityThreshold::get() * total_weight;

            ensure!(
                accumulated_weight >= threshold,
                Error::<T>::ThresholdNotReached
            );

            // 7. Update finality state atomically
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

        /// Finalize a block and update all related state
        /// 
        /// This method performs atomic finality state updates:
        /// - Updates FinalizedBlockNumber to justified block number
        /// - Updates FinalizedBlockHash to justified block hash
        /// - Increments CurrentRound by one
        /// - Ensures monotonic increase of finalized block number
        /// - Emits BlockFinalized event
        /// - Prunes VoteRecords older than VoteRetentionRounds
        /// - Clears VoteTallies for finalized blocks
        fn finalize_block(
            block_hash: T::Hash,
            block_number: BlockNumberFor<T>,
            round: u32,
            accumulated_weight: u128,
        ) {
            // Update FinalizedBlockNumber (monotonic increase ensured by caller)
            FinalizedBlockNumber::<T>::put(block_number);

            // Update FinalizedBlockHash
            FinalizedBlockHash::<T>::put(block_hash);

            // Increment CurrentRound
            let next_round = round.saturating_add(1);
            CurrentRound::<T>::put(next_round);

            // Emit BlockFinalized event
            Self::deposit_event(Event::BlockFinalized {
                block_number,
                block_hash,
                round,
                weight: accumulated_weight,
            });

            // Prune old vote records
            Self::prune_vote_records(round);

            // Clear vote tallies for finalized block
            VoteTallies::<T>::remove(&block_hash);
        }

        /// Prune vote records older than VoteRetentionRounds
        /// 
        /// This method removes VoteRecords for rounds older than
        /// (current_round - VoteRetentionRounds) to prevent unbounded storage growth.
        fn prune_vote_records(current_round: u32) {
            let retention_rounds = T::VoteRetentionRounds::get();
            
            if current_round > retention_rounds {
                let cutoff_round = current_round.saturating_sub(retention_rounds);
                
                // Remove all vote records for rounds older than cutoff
                for old_round in 0..cutoff_round {
                    let _ = VoteRecords::<T>::clear_prefix(old_round, u32::MAX, None);
                }
            }
        }

        fn trigger_finalization(block_hash: T::Hash, block_number: BlockNumberFor<T>, round: u32, tally_weight: u128) {
            Self::finalize_block(block_hash, block_number, round, tally_weight);
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

                // The priority is based on the block number being finalized
                let priority = block_number.saturated_into::<u64>();

                // Provide a unique tag for this block hash so only one justification per block is permitted
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
