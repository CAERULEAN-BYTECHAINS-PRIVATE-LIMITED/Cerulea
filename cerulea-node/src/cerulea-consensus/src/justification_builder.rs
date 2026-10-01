//! Justification Builder
//!
//! This module implements the JustificationBuilder component that constructs and verifies
//! Commit Engine justifications containing threshold-reaching votes.

use codec::Encode;
use log::{debug, info};
use sc_client_api::HeaderBackend;
use sp_api::ProvideRuntimeApi;
use sp_runtime::traits::{Block as BlockT, NumberFor};
use std::collections::HashSet;
use std::sync::Arc;

use crate::dvf_gossip::{DvfJustification, DvfVoteMessage, DvfVotePool};
use crate::engines::commit::submission::{
    SubmissionPolicy, MAX_SUBMISSION_BYTES, MAX_VOTES_PER_SUBMISSION,
};
use crate::metrics::DvfMetrics;
use pallet_cerulea_dcf::DcfApi as RuntimeDcfApi;
use pallet_cerulea_dvf::DvfApi;

/// Maximum encoded size for a justification (1 MB)
const MAX_JUSTIFICATION_SIZE: usize = 1_048_576;

/// The highest number of votes one submitted justification carries.
///
/// The bound holds before any signature check runs, so an oversized message
/// from an unknown peer costs one length read.
const MAX_SUBMITTED_VOTES: usize = MAX_VOTES_PER_SUBMISSION as usize;

/// Justification Builder
///
/// Constructs and verifies Commit Engine justifications containing threshold-reaching votes.
pub struct JustificationBuilder<Block, Client, AccountId>
where
    Block: BlockT,
{
    client: Arc<Client>,
    vote_pool: Arc<DvfVotePool<Block::Hash, AccountId>>,
    metrics: Option<Arc<DvfMetrics>>,
}

impl<Block, Client, AccountId> JustificationBuilder<Block, Client, AccountId>
where
    Block: BlockT,
    Client: ProvideRuntimeApi<Block> + HeaderBackend<Block> + Send + Sync + 'static,
    Client::Api: RuntimeDcfApi<Block, AccountId, u128, NumberFor<Block>>
        + DvfApi<Block, NumberFor<Block>, AccountId, Block::Hash>,
    AccountId: Clone
        + Encode
        + codec::Codec
        + PartialEq
        + Eq
        + std::fmt::Debug
        + Send
        + Sync
        + std::hash::Hash
        + 'static,
{
    /// Creates a new justification builder
    ///
    /// # Arguments
    /// * `client` - The blockchain client for querying runtime state
    /// * `vote_pool` - The vote pool containing accumulated votes
    pub fn new(client: Arc<Client>, vote_pool: Arc<DvfVotePool<Block::Hash, AccountId>>) -> Self {
        info!("Commit Engine Justification Builder: Initializing");
        Self {
            client,
            vote_pool,
            metrics: None,
        }
    }

    /// Sets the metrics for this builder
    pub fn with_metrics(mut self, metrics: Arc<DvfMetrics>) -> Self {
        self.metrics = Some(metrics);
        self
    }
}

impl<Block, Client, AccountId> JustificationBuilder<Block, Client, AccountId>
where
    Block: BlockT,
    Client: ProvideRuntimeApi<Block> + HeaderBackend<Block> + Send + Sync + 'static,
    Client::Api: RuntimeDcfApi<Block, AccountId, u128, NumberFor<Block>>
        + DvfApi<Block, NumberFor<Block>, AccountId, Block::Hash>,
    AccountId: Clone
        + Encode
        + codec::Codec
        + PartialEq
        + Eq
        + std::fmt::Debug
        + Send
        + Sync
        + std::hash::Hash
        + 'static,
{
    /// Selects minimum votes needed to reach threshold
    ///
    /// This method retrieves all votes for the target block from the vote pool,
    /// queries validator weights from the runtime, sorts votes by validator weight
    /// descending, and selects the minimum votes needed to reach the threshold.
    ///
    /// # Arguments
    /// * `round` - The round number
    /// * `block_hash` - The block hash to select votes for
    ///
    /// # Returns
    /// A vector of selected votes sorted by weight (descending)
    fn select_votes(
        &self,
        round: u32,
        block_hash: &Block::Hash,
    ) -> Result<Vec<DvfVoteMessage<Block::Hash, AccountId>>, String> {
        debug!(
            "Commit Engine Justification Builder: Selecting votes for block {:?} in round {}",
            block_hash, round
        );

        // 1. Retrieve all votes for target block from vote pool
        let votes = self.vote_pool.get_votes(round, block_hash);

        if votes.is_empty() {
            return Err("No votes found for block".to_string());
        }

        debug!(
            "Commit Engine Justification Builder: Found {} votes for block",
            votes.len()
        );

        // 2. Query validator weights from runtime
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;

        let validator_weights = api
            .get_validator_weights(best_hash)
            .map_err(|e| format!("Failed to get validator weights: {:?}", e))?;

        // Create a map for quick weight lookup
        let weight_map: std::collections::HashMap<_, _> = validator_weights.into_iter().collect();

        // 3. Create weighted votes (vote, weight) pairs
        let mut weighted_votes: Vec<(DvfVoteMessage<Block::Hash, AccountId>, u128)> = votes
            .into_iter()
            .filter_map(|vote| {
                weight_map
                    .get(&vote.validator_account_id)
                    .map(|&weight| (vote, weight))
            })
            .collect();

        if weighted_votes.is_empty() {
            return Err("No valid votes with weights found".to_string());
        }

        // 4. Sort votes by validator weight descending
        weighted_votes.sort_by(|a, b| b.1.cmp(&a.1));

        debug!(
            "Commit Engine Justification Builder: Sorted {} weighted votes",
            weighted_votes.len()
        );

        // 5. Query finality threshold
        let finality_threshold_perbill = api
            .get_finality_threshold_perbill(best_hash)
            .map_err(|e| format!("Failed to get finality threshold: {:?}", e))?;

        let total_weight: u128 = weight_map.values().sum();
        let threshold = finality_threshold_perbill * total_weight;

        debug!(
            "Commit Engine Justification Builder: Total weight: {}, Threshold: {}",
            total_weight, threshold
        );

        // 6. Select minimum votes needed to reach threshold
        let mut selected_votes = Vec::new();
        let mut accumulated_weight = 0u128;

        for (vote, weight) in weighted_votes {
            selected_votes.push(vote);
            accumulated_weight = accumulated_weight.saturating_add(weight);

            if accumulated_weight >= threshold {
                break;
            }
        }

        info!(
            "Commit Engine Justification Builder: Selected {} votes with accumulated weight {} (threshold: {})",
            selected_votes.len(),
            accumulated_weight,
            threshold
        );

        Ok(selected_votes)
    }
}

impl<Block, Client, AccountId> JustificationBuilder<Block, Client, AccountId>
where
    Block: BlockT,
    Client: ProvideRuntimeApi<Block> + HeaderBackend<Block> + Send + Sync + 'static,
    Client::Api: RuntimeDcfApi<Block, AccountId, u128, NumberFor<Block>>
        + DvfApi<Block, NumberFor<Block>, AccountId, Block::Hash>,
    AccountId: Clone
        + Encode
        + codec::Codec
        + PartialEq
        + Eq
        + std::fmt::Debug
        + Send
        + Sync
        + std::hash::Hash
        + 'static,
{
    /// Constructs a justification for a block
    ///
    /// This method constructs a DvfJustification with the round number, block hash,
    /// and selected votes. It verifies no duplicate validators and ensures accumulated
    /// weight meets the threshold.
    ///
    /// # Arguments
    /// * `round` - The round number
    /// * `block_hash` - The block hash to construct justification for
    ///
    /// # Returns
    /// A verified DvfJustification
    pub fn construct_justification(
        &self,
        round: u32,
        block_hash: Block::Hash,
    ) -> Result<DvfJustification<Block::Hash, AccountId>, String> {
        info!(
            "Commit Engine Justification Builder: Constructing justification for block {:?} in round {}",
            block_hash, round
        );

        // 1. Select votes using the vote selection algorithm
        let selected_votes = self.select_votes(round, &block_hash)?;

        // 2. Verify no duplicate validators
        let mut seen_validators = HashSet::new();
        for vote in &selected_votes {
            if !seen_validators.insert(vote.validator_account_id.clone()) {
                return Err(format!(
                    "Duplicate validator {:?} in selected votes",
                    vote.validator_account_id
                ));
            }
        }

        debug!(
            "Commit Engine Justification Builder: Verified no duplicate validators ({} unique)",
            seen_validators.len()
        );

        // 3. Construct justification
        let justification = DvfJustification {
            round_number: round,
            block_hash,
            votes: selected_votes,
        };

        // 4. Verify accumulated weight meets threshold
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;

        let validator_weights = api
            .get_validator_weights(best_hash)
            .map_err(|e| format!("Failed to get validator weights: {:?}", e))?;

        let weight_map: std::collections::HashMap<_, _> = validator_weights.into_iter().collect();

        let mut accumulated_weight = 0u128;
        for vote in &justification.votes {
            if let Some(&weight) = weight_map.get(&vote.validator_account_id) {
                accumulated_weight = accumulated_weight.saturating_add(weight);
            }
        }

        let finality_threshold_perbill = api
            .get_finality_threshold_perbill(best_hash)
            .map_err(|e| format!("Failed to get finality threshold: {:?}", e))?;

        let total_weight: u128 = weight_map.values().sum();
        let threshold = finality_threshold_perbill * total_weight;

        if accumulated_weight < threshold {
            return Err(format!(
                "Accumulated weight {} does not meet threshold {}",
                accumulated_weight, threshold
            ));
        }

        info!(
            "Commit Engine Justification Builder: Constructed justification with {} votes, accumulated weight {} (threshold: {})",
            justification.votes.len(),
            accumulated_weight,
            threshold
        );

        // Log justification construction at info level
        info!(
            "Commit Engine Justification Builder: Justification constructed for block {:?}, round {}, {} votes, weight {}",
            block_hash, round, justification.votes.len(), accumulated_weight
        );

        // 5. Enforce size limits
        self.enforce_size_limits(&justification)?;

        Ok(justification)
    }

    /// Enforces size limits on a justification
    ///
    /// This method checks that the encoded size doesn't exceed 1 MB and that
    /// the number of votes doesn't exceed the number of active validators.
    ///
    /// # Arguments
    /// * `justification` - The justification to check
    ///
    /// # Returns
    /// Ok if size limits are satisfied, Err otherwise
    fn enforce_size_limits(
        &self,
        justification: &DvfJustification<Block::Hash, AccountId>,
    ) -> Result<(), String> {
        // 1. Check encoded size doesn't exceed 1 MB
        let encoded = justification.encode();
        let encoded_size = encoded.len();

        if encoded_size > MAX_JUSTIFICATION_SIZE {
            return Err(format!(
                "Justification size {} exceeds maximum {} bytes",
                encoded_size, MAX_JUSTIFICATION_SIZE
            ));
        }

        debug!(
            "Commit Engine Justification Builder: Justification encoded size: {} bytes",
            encoded_size
        );

        // 2. Limit votes to number of active validators
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;

        let active_validators = api
            .get_validator_set(best_hash)
            .map_err(|e| format!("Failed to get validator set: {:?}", e))?;

        if justification.votes.len() > active_validators.len() {
            return Err(format!(
                "Justification has {} votes but only {} active validators",
                justification.votes.len(),
                active_validators.len()
            ));
        }

        debug!(
            "Commit Engine Justification Builder: Size limits satisfied ({} votes, {} bytes)",
            justification.votes.len(),
            encoded_size
        );

        Ok(())
    }
}

impl<Block, Client, AccountId> JustificationBuilder<Block, Client, AccountId>
where
    Block: BlockT,
    Client: ProvideRuntimeApi<Block> + HeaderBackend<Block> + Send + Sync + 'static,
    Client::Api: RuntimeDcfApi<Block, AccountId, u128, NumberFor<Block>>
        + DvfApi<Block, NumberFor<Block>, AccountId, Block::Hash>,
    AccountId: Clone
        + Encode
        + codec::Codec
        + PartialEq
        + Eq
        + std::fmt::Debug
        + Send
        + Sync
        + std::hash::Hash
        + 'static,
{
    /// Verifies a justification is valid
    ///
    /// This method verifies all vote signatures, checks that all votes are from active
    /// validators, verifies accumulated weight meets threshold, and ensures all votes
    /// have matching validator_set_id. Logs verification at info level.
    ///
    /// # Arguments
    /// * `justification` - The justification to verify
    ///
    /// # Returns
    /// Ok if justification is valid, Err with reason otherwise
    pub fn verify_justification(
        &self,
        justification: &DvfJustification<Block::Hash, AccountId>,
    ) -> Result<(), String> {
        info!(
            "Commit Engine Justification Builder: Verifying justification for block {:?} in round {}",
            justification.block_hash, justification.round_number
        );

        // 1. Bound the submission before any signature check runs.
        if justification.votes.is_empty() {
            return Err(
                "The justification carries no votes. Fix: gather votes before submitting."
                    .to_string(),
            );
        }
        if justification.votes.len() > MAX_SUBMITTED_VOTES {
            return Err(format!(
                "The justification carries {} votes and the limit is {}. \
                 Fix: send one justification built from the validator set of that round.",
                justification.votes.len(),
                MAX_SUBMITTED_VOTES
            ));
        }
        let submission_bytes = justification.encode().len();
        if submission_bytes > MAX_SUBMISSION_BYTES {
            return Err(format!(
                "The justification carries {} bytes and the limit is {}. \
                 Fix: send one justification for one block rather than a joined message.",
                submission_bytes, MAX_SUBMISSION_BYTES
            ));
        }
        let declared =
            SubmissionPolicy::standard().declared_weight(justification.votes.len() as u32);
        debug!(
            "Commit Engine Justification Builder: the submission declares {} weight units for {} votes",
            declared.0,
            justification.votes.len()
        );

        // 2. Query runtime state
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;

        let active_validators = api
            .get_validator_set(best_hash)
            .map_err(|e| format!("Failed to get validator set: {:?}", e))?;

        let validator_weights = api
            .get_validator_weights(best_hash)
            .map_err(|e| format!("Failed to get validator weights: {:?}", e))?;

        let weight_map: std::collections::HashMap<_, _> = validator_weights.into_iter().collect();

        // 3. Verify all votes
        let mut seen_validators = HashSet::new();
        let mut accumulated_weight = 0u128;
        let mut expected_validator_set_id: Option<u32> = None;

        for vote in &justification.votes {
            // Check for duplicate validators
            if !seen_validators.insert(vote.validator_account_id.clone()) {
                return Err(format!(
                    "Duplicate validator {:?} in justification",
                    vote.validator_account_id
                ));
            }

            // Verify signature
            let mut encoded_payload = Vec::new();
            vote.epoch_id.encode_to(&mut encoded_payload);
            vote.validator_set_id.encode_to(&mut encoded_payload);
            vote.round_number.encode_to(&mut encoded_payload);
            vote.block_number.encode_to(&mut encoded_payload);
            vote.block_hash.encode_to(&mut encoded_payload);
            vote.validator_account_id.encode_to(&mut encoded_payload);

            if !sp_io::crypto::ed25519_verify(
                &vote.signature,
                &encoded_payload,
                &vote.validator_public_key,
            ) {
                return Err(format!(
                    "Invalid signature for validator {:?}",
                    vote.validator_account_id
                ));
            }

            // Check validator set ID consistency
            if let Some(expected_id) = expected_validator_set_id {
                if vote.validator_set_id != expected_id {
                    return Err(format!(
                        "Validator set ID mismatch: expected {}, got {}",
                        expected_id, vote.validator_set_id
                    ));
                }
            } else {
                expected_validator_set_id = Some(vote.validator_set_id);
            }

            // Check round consistency
            if vote.round_number != justification.round_number {
                return Err(format!(
                    "Round mismatch: expected {}, got {}",
                    justification.round_number, vote.round_number
                ));
            }

            // Check block hash consistency
            if vote.block_hash != justification.block_hash {
                return Err(format!(
                    "Block hash mismatch in vote from validator {:?}",
                    vote.validator_account_id
                ));
            }

            // Verify validator is active
            if !active_validators.contains(&vote.validator_account_id) {
                return Err(format!(
                    "Validator {:?} is not in active set",
                    vote.validator_account_id
                ));
            }

            // Accumulate weight
            if let Some(&weight) = weight_map.get(&vote.validator_account_id) {
                accumulated_weight = accumulated_weight.saturating_add(weight);
            } else {
                return Err(format!(
                    "Validator {:?} has no weight",
                    vote.validator_account_id
                ));
            }
        }

        // 4. Verify accumulated weight meets threshold
        let finality_threshold_perbill = api
            .get_finality_threshold_perbill(best_hash)
            .map_err(|e| format!("Failed to get finality threshold: {:?}", e))?;

        let total_weight: u128 = weight_map.values().sum();
        let threshold = finality_threshold_perbill * total_weight;

        if accumulated_weight < threshold {
            return Err(format!(
                "Accumulated weight {} does not meet threshold {}",
                accumulated_weight, threshold
            ));
        }

        info!(
            "Commit Engine Justification Builder: Justification verified successfully - {} votes, accumulated weight {} (threshold: {})",
            justification.votes.len(),
            accumulated_weight,
            threshold
        );

        // Log justification verification at info level
        info!(
            "Commit Engine Justification Builder: Justification verified for block {:?}, round {}, {} votes, weight {}",
            justification.block_hash, justification.round_number, justification.votes.len(), accumulated_weight
        );

        Ok(())
    }
}

#[cfg(test)]
mod coverage_tests {
    //! Construction and verification of Commit Engine justifications.
    //!
    //! Three validators hold five, three and two tenths of the weight. The
    //! threshold asks for sixty seven percent, a weight of 670.

    use super::*;
    use crate::test_support::{AccountId, Block, TestChain, Voter};
    use sp_core::{Pair as _, H256};
    use sp_runtime::Perbill;

    type Pool = DvfVotePool<H256, AccountId>;
    type Builder = JustificationBuilder<Block, TestChain, AccountId>;

    struct Scene {
        chain: Arc<TestChain>,
        pool: Arc<Pool>,
        builder: Builder,
        voters: [Voter; 3],
        target: H256,
    }

    fn scene() -> Scene {
        let chain = TestChain::with_blocks(12);
        let voters = [
            Voter::from_seed(1),
            Voter::from_seed(2),
            Voter::from_seed(3),
        ];
        chain.answers.set(
            "DvfApi_get_finality_threshold_perbill",
            (),
            Perbill::from_percent(67),
        );
        chain.answers.set(
            "DvfApi_get_validator_weights",
            (),
            vec![
                (voters[0].account.clone(), 500u128),
                (voters[1].account.clone(), 300u128),
                (voters[2].account.clone(), 200u128),
            ],
        );
        chain.answers.set(
            "DvfApi_get_validator_set",
            (),
            voters
                .iter()
                .map(|voter| voter.account.clone())
                .collect::<Vec<_>>(),
        );
        let pool = Pool::new();
        let builder = Builder::new(chain.clone(), pool.clone());
        let target = chain.hash_at(10);
        Scene {
            chain,
            pool,
            builder,
            voters,
            target,
        }
    }

    fn signed(scene: &Scene, index: usize) -> DvfVoteMessage<H256, AccountId> {
        scene.voters[index].vote(1, 4, 5, 10, scene.target)
    }

    fn justification(
        scene: &Scene,
        votes: Vec<DvfVoteMessage<H256, AccountId>>,
    ) -> DvfJustification<H256, AccountId> {
        DvfJustification {
            round_number: 5,
            block_hash: scene.target,
            votes,
        }
    }

    #[test]
    fn construction_takes_the_heaviest_votes_until_the_threshold() {
        let scene = scene();
        for index in [2, 0, 1] {
            scene.pool.insert_vote(signed(&scene, index));
        }

        let built = scene
            .builder
            .construct_justification(5, scene.target)
            .expect("the votes reach the threshold");

        assert_eq!(built.round_number, 5);
        assert_eq!(built.block_hash, scene.target);
        let signers: Vec<AccountId> = built
            .votes
            .iter()
            .map(|vote| vote.validator_account_id.clone())
            .collect();
        // Five tenths alone falls short. Five and three tenths cross 670.
        assert_eq!(
            signers,
            vec![
                scene.voters[0].account.clone(),
                scene.voters[1].account.clone()
            ]
        );
        assert_eq!(scene.builder.verify_justification(&built), Ok(()));
    }

    #[test]
    fn construction_without_votes_names_the_gap() {
        let scene = scene();
        assert_eq!(
            scene.builder.construct_justification(5, scene.target),
            Err("No votes found for block".to_string())
        );
    }

    #[test]
    fn construction_from_validators_without_weight_names_the_gap() {
        let scene = scene();
        scene
            .pool
            .insert_vote(Voter::from_seed(9).vote(1, 4, 5, 10, scene.target));
        assert_eq!(
            scene.builder.construct_justification(5, scene.target),
            Err("No valid votes with weights found".to_string())
        );
    }

    #[test]
    fn construction_below_the_threshold_states_both_weights() {
        let scene = scene();
        scene.pool.insert_vote(signed(&scene, 0));
        assert_eq!(
            scene.builder.construct_justification(5, scene.target),
            Err("Accumulated weight 500 does not meet threshold 670".to_string())
        );
    }

    #[test]
    fn construction_holds_the_votes_inside_the_active_set_size() {
        let scene = scene();
        scene.pool.insert_vote(signed(&scene, 0));
        scene.pool.insert_vote(signed(&scene, 1));
        scene.chain.answers.set(
            "DvfApi_get_validator_set",
            (),
            vec![scene.voters[0].account.clone()],
        );
        assert_eq!(
            scene.builder.construct_justification(5, scene.target),
            Err("Justification has 2 votes but only 1 active validators".to_string())
        );
    }

    #[test]
    fn construction_names_each_missing_runtime_answer() {
        let cases = [
            (
                "DvfApi_get_validator_weights",
                "Failed to get validator weights: ",
            ),
            (
                "DvfApi_get_finality_threshold_perbill",
                "Failed to get finality threshold: ",
            ),
            ("DvfApi_get_validator_set", "Failed to get validator set: "),
        ];
        for (call, opening) in cases {
            let scene = scene();
            scene.pool.insert_vote(signed(&scene, 0));
            scene.pool.insert_vote(signed(&scene, 1));
            scene.chain.answers.forget(call);
            let reason = scene
                .builder
                .construct_justification(5, scene.target)
                .expect_err("the construction stops at the missing answer");
            assert!(reason.starts_with(opening), "{call}: {reason}");
        }
    }

    #[test]
    fn a_builder_with_metrics_keeps_them() {
        let scene = scene();
        let registry = prometheus::Registry::new();
        let metrics = Arc::new(DvfMetrics::new(&registry).expect("the metrics register"));
        let builder = Builder::new(scene.chain.clone(), scene.pool.clone()).with_metrics(metrics);
        assert!(builder.metrics.is_some());
        assert!(scene.builder.metrics.is_none());
    }

    #[test]
    fn verification_refuses_an_empty_justification() {
        let scene = scene();
        assert_eq!(
            scene
                .builder
                .verify_justification(&justification(&scene, Vec::new())),
            Err(
                "The justification carries no votes. Fix: gather votes before submitting."
                    .to_string()
            )
        );
    }

    #[test]
    fn verification_refuses_more_votes_than_one_submission_carries() {
        let scene = scene();
        let vote = signed(&scene, 0);
        let votes = vec![vote; MAX_SUBMITTED_VOTES + 1];
        assert_eq!(
            scene.builder.verify_justification(&justification(&scene, votes)),
            Err(format!(
                "The justification carries {} votes and the limit is {}. Fix: send one justification built from the validator set of that round.",
                MAX_SUBMITTED_VOTES + 1,
                MAX_SUBMITTED_VOTES
            ))
        );
    }

    #[test]
    fn verification_refuses_one_validator_counted_twice() {
        let scene = scene();
        let votes = vec![signed(&scene, 0), signed(&scene, 0)];
        assert_eq!(
            scene
                .builder
                .verify_justification(&justification(&scene, votes)),
            Err(format!(
                "Duplicate validator {:?} in justification",
                scene.voters[0].account
            ))
        );
    }

    #[test]
    fn verification_refuses_a_forged_signature() {
        let scene = scene();
        let mut forged = signed(&scene, 1);
        forged.signature = scene.voters[0].pair.sign(b"another payload");
        assert_eq!(
            scene
                .builder
                .verify_justification(&justification(&scene, vec![forged])),
            Err(format!(
                "Invalid signature for validator {:?}",
                scene.voters[1].account
            ))
        );
    }

    #[test]
    fn verification_refuses_votes_from_two_validator_sets() {
        let scene = scene();
        let votes = vec![
            signed(&scene, 0),
            scene.voters[1].vote(1, 5, 5, 10, scene.target),
        ];
        assert_eq!(
            scene
                .builder
                .verify_justification(&justification(&scene, votes)),
            Err("Validator set ID mismatch: expected 4, got 5".to_string())
        );
    }

    #[test]
    fn verification_refuses_a_vote_from_another_round() {
        let scene = scene();
        let votes = vec![scene.voters[0].vote(1, 4, 6, 10, scene.target)];
        assert_eq!(
            scene
                .builder
                .verify_justification(&justification(&scene, votes)),
            Err("Round mismatch: expected 5, got 6".to_string())
        );
    }

    #[test]
    fn verification_refuses_a_vote_for_another_block() {
        let scene = scene();
        let votes = vec![scene.voters[0].vote(1, 4, 5, 11, scene.chain.hash_at(11))];
        assert_eq!(
            scene
                .builder
                .verify_justification(&justification(&scene, votes)),
            Err(format!(
                "Block hash mismatch in vote from validator {:?}",
                scene.voters[0].account
            ))
        );
    }

    #[test]
    fn verification_refuses_a_validator_outside_the_active_set() {
        let scene = scene();
        let outsider = Voter::from_seed(9);
        let votes = vec![outsider.vote(1, 4, 5, 10, scene.target)];
        assert_eq!(
            scene
                .builder
                .verify_justification(&justification(&scene, votes)),
            Err(format!(
                "Validator {:?} is not in active set",
                outsider.account
            ))
        );
    }

    #[test]
    fn verification_refuses_an_active_validator_without_weight() {
        let scene = scene();
        let unweighted = Voter::from_seed(9);
        let mut active: Vec<AccountId> = scene
            .voters
            .iter()
            .map(|voter| voter.account.clone())
            .collect();
        active.push(unweighted.account.clone());
        scene
            .chain
            .answers
            .set("DvfApi_get_validator_set", (), active);
        let votes = vec![unweighted.vote(1, 4, 5, 10, scene.target)];
        assert_eq!(
            scene
                .builder
                .verify_justification(&justification(&scene, votes)),
            Err(format!("Validator {:?} has no weight", unweighted.account))
        );
    }

    #[test]
    fn verification_refuses_weight_below_the_threshold() {
        let scene = scene();
        let votes = vec![signed(&scene, 1), signed(&scene, 2)];
        assert_eq!(
            scene
                .builder
                .verify_justification(&justification(&scene, votes)),
            Err("Accumulated weight 500 does not meet threshold 670".to_string())
        );
    }

    #[test]
    fn verification_names_each_missing_runtime_answer() {
        let cases = [
            ("DvfApi_get_validator_set", "Failed to get validator set: "),
            (
                "DvfApi_get_validator_weights",
                "Failed to get validator weights: ",
            ),
            (
                "DvfApi_get_finality_threshold_perbill",
                "Failed to get finality threshold: ",
            ),
        ];
        for (call, opening) in cases {
            let scene = scene();
            let votes = vec![signed(&scene, 0), signed(&scene, 1)];
            scene.chain.answers.forget(call);
            let reason = scene
                .builder
                .verify_justification(&justification(&scene, votes))
                .expect_err("the check stops at the missing answer");
            assert!(reason.starts_with(opening), "{call}: {reason}");
        }
    }
}
