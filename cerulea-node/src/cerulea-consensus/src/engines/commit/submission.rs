//! Bounded, repriced vote submission.
//!
//! A commit submission carries the aggregate signature of one block. The
//! submission passes this gate before the transaction pool admits it.
//!
//! Earlier releases accepted a submission of any size and declared one fixed
//! weight for it. A peer with no account reached that path through a network
//! message, so one message could carry thousands of signatures at the price of
//! one. The gate closes that surface in two ways.
//!
//! 1. **A maximum count.** A submission carrying more signatures than the
//!    limit is refused before the pool sees it.
//! 2. **A weight that scales.** The declared weight rises with the signature
//!    count, so a large submission costs what it costs to check.
//!
//! The gate also bounds the encoded size, so a submission padded with spare
//! bytes never reaches the pool.

use codec::{Decode, Encode};

use super::aggregate::AggregateSignature;
use crate::engines::bounds::{admit, MessagePath};
use crate::engines::{EngineError, Slot};

/// The highest number of signatures one submission carries.
///
/// The figure covers the largest validator set the throughput target names,
/// with room above it.
pub const MAX_VOTES_PER_SUBMISSION: u32 = 256;

/// The highest number of bytes one submission carries.
pub const MAX_SUBMISSION_BYTES: usize =
    cerulea_primitives::bounds::MessagePath::CommitSubmission.max_bytes();

/// The weight units one submission costs before the signature count.
pub const BASE_WEIGHT_UNITS: u64 = 10_000;

/// The weight units each signature adds.
///
/// The figure reflects the cost of checking one signature.
pub const WEIGHT_UNITS_PER_VOTE: u64 = 25_000;

/// The submission policy of one deployment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SubmissionPolicy {
    /// The highest number of signatures one submission carries.
    pub max_votes: u32,
    /// The highest number of bytes one submission carries.
    pub max_bytes: usize,
    /// The weight units one submission costs before the signature count.
    pub base_weight: u64,
    /// The weight units each signature adds.
    pub weight_per_vote: u64,
}

impl Default for SubmissionPolicy {
    fn default() -> Self {
        SubmissionPolicy::standard()
    }
}

impl SubmissionPolicy {
    /// The policy a deployment starts from.
    pub const fn standard() -> Self {
        SubmissionPolicy {
            max_votes: MAX_VOTES_PER_SUBMISSION,
            max_bytes: MAX_SUBMISSION_BYTES,
            base_weight: BASE_WEIGHT_UNITS,
            weight_per_vote: WEIGHT_UNITS_PER_VOTE,
        }
    }

    /// Build a policy holding the four stated values.
    pub const fn new(
        max_votes: u32,
        max_bytes: usize,
        base_weight: u64,
        weight_per_vote: u64,
    ) -> Self {
        SubmissionPolicy {
            max_votes,
            max_bytes,
            base_weight,
            weight_per_vote,
        }
    }

    /// The weight a submission of the given signature count declares.
    pub const fn declared_weight(&self, votes: u32) -> DeclaredWeight {
        DeclaredWeight(
            self.base_weight
                .saturating_add(self.weight_per_vote.saturating_mul(votes as u64)),
        )
    }

    /// The highest weight any admitted submission declares.
    pub const fn ceiling_weight(&self) -> DeclaredWeight {
        self.declared_weight(self.max_votes)
    }
}

/// The weight one submission declares.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
pub struct DeclaredWeight(pub u64);

/// One commit submission.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct CommitSubmission {
    /// The slot the block serves.
    pub slot: Slot,
    /// The hash of the block the signers accepted.
    pub block_hash: [u8; 32],
    /// The aggregate the author published.
    pub aggregate: AggregateSignature,
}

impl CommitSubmission {
    /// Wrap one aggregate as a submission.
    pub fn new(aggregate: AggregateSignature) -> Self {
        CommitSubmission {
            slot: aggregate.slot,
            block_hash: aggregate.block_hash,
            aggregate,
        }
    }

    /// The number of signatures the submission carries.
    pub fn vote_count(&self) -> usize {
        self.aggregate.signatures.len()
    }

    /// The bytes the submission occupies on the wire.
    pub fn encoded_len(&self) -> usize {
        self.encode().len()
    }
}

/// What the gate decided about one submission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Admission {
    /// The number of signatures the submission carries.
    pub votes: u32,
    /// The weight the submission declares.
    pub weight: DeclaredWeight,
    /// The bytes the submission occupies.
    pub bytes: usize,
}

/// The gate every submission passes before pool admission.
#[derive(Clone, Copy, Debug, Default)]
pub struct SubmissionGate {
    policy: SubmissionPolicy,
}

impl SubmissionGate {
    /// Build a gate under the standard policy.
    pub fn new() -> Self {
        SubmissionGate {
            policy: SubmissionPolicy::standard(),
        }
    }

    /// Build a gate under the given policy.
    pub fn with_policy(policy: SubmissionPolicy) -> Self {
        SubmissionGate { policy }
    }

    /// The policy this gate applies.
    pub fn policy(&self) -> SubmissionPolicy {
        self.policy
    }

    /// Check one submission before the transaction pool admits it.
    ///
    /// The call reports the declared weight where the submission passes. Every
    /// refusal names the fix.
    pub fn admit(&self, submission: &CommitSubmission) -> Result<Admission, EngineError> {
        let bytes = submission.encoded_len();
        if bytes > self.policy.max_bytes {
            return Err(EngineError::SubmissionTooLarge {
                bytes,
                limit: self.policy.max_bytes,
            });
        }
        let named = submission.aggregate.signers.len();
        if named > self.policy.max_votes as usize {
            return Err(EngineError::SubmissionCountAboveLimit {
                count: named,
                limit: self.policy.max_votes as usize,
            });
        }
        let carried = submission.vote_count();
        if carried > self.policy.max_votes as usize {
            return Err(EngineError::SubmissionCountAboveLimit {
                count: carried,
                limit: self.policy.max_votes as usize,
            });
        }
        if !submission.aggregate.signers.well_formed() {
            return Err(EngineError::AggregateBitmapMalformed);
        }
        let named_signers = submission.aggregate.signers.count();
        if named_signers != carried {
            return Err(EngineError::AggregateCountMismatch {
                named: named_signers,
                carried,
            });
        }
        if submission.slot != submission.aggregate.slot {
            return Err(EngineError::VoteSlotMismatch {
                claimed: submission.slot.0,
                expected: submission.aggregate.slot.0,
            });
        }
        if submission.block_hash != submission.aggregate.block_hash {
            return Err(EngineError::VoteBlockMismatch {
                slot: submission.slot.0,
            });
        }
        Ok(Admission {
            votes: carried as u32,
            weight: self.policy.declared_weight(carried as u32),
            bytes,
        })
    }
}

/// Read one commit submission from the network.
///
/// The size bound of the path runs before the decoding, so an oversized
/// message costs one length comparison. The gate then runs over the decoded
/// value before the pool admits it.
pub fn read_submission(bytes: &[u8]) -> Result<CommitSubmission, EngineError> {
    admit(MessagePath::CommitSubmission, bytes.len())?;
    CommitSubmission::decode(&mut &bytes[..]).map_err(|reason| EngineError::MessageUnreadable {
        path: MessagePath::CommitSubmission.label(),
        detail: format!("{}", reason),
    })
}

/// Read one commit submission and run the pool gate over it.
pub fn read_and_admit(bytes: &[u8]) -> Result<(CommitSubmission, Admission), EngineError> {
    let submission = read_submission(bytes)?;
    let admitted = SubmissionGate::new().admit(&submission)?;
    Ok((submission, admitted))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engines::commit::aggregate::SignerBitmap;
    use crate::engines::commit::collect::{cast_vote, SignatureCollector};
    use crate::engines::TrustScore;
    use crate::CERULEA_KEY_TYPE;
    use sp_core::ed25519;
    use sp_keystore::testing::MemoryKeystore;
    use sp_keystore::{Keystore, KeystorePtr};
    use std::sync::Arc;

    const SLOT: Slot = Slot(200);
    const BLOCK: [u8; 32] = [41u8; 32];

    fn submission_of(signers: usize, members: usize) -> CommitSubmission {
        let mut keystores: Vec<KeystorePtr> = Vec::new();
        let mut keys: Vec<ed25519::Public> = Vec::new();
        for index in 0..members {
            let memory = MemoryKeystore::new();
            let key = memory
                .ed25519_generate_new(
                    CERULEA_KEY_TYPE,
                    Some(&format!("//Cerulea//submit//{}", index)),
                )
                .expect("the keystore generates a key");
            keystores.push(Arc::new(memory) as KeystorePtr);
            keys.push(key);
        }
        let set: Vec<(ed25519::Public, TrustScore)> = keys
            .iter()
            .map(|key| (*key, TrustScore::from_percent(100)))
            .collect();
        let mut collector =
            SignatureCollector::open(SLOT, BLOCK, &set, 0, 10_000).expect("the collector opens");
        let ordered = collector.members().to_vec();
        for member in ordered.iter().take(signers) {
            let position = keys
                .iter()
                .position(|key| key == member)
                .expect("the member sits in the set");
            let vote =
                cast_vote(&keystores[position], member, SLOT, BLOCK).expect("the validator signs");
            collector.accept(vote, 1).expect("the vote lands");
        }
        CommitSubmission::new(
            AggregateSignature::assemble(&collector).expect("the envelope assembles"),
        )
    }

    #[test]
    fn the_standard_policy_holds_its_stated_values() {
        let policy = SubmissionPolicy::standard();
        assert_eq!(policy.max_votes, MAX_VOTES_PER_SUBMISSION);
        assert_eq!(policy.max_bytes, MAX_SUBMISSION_BYTES);
        assert_eq!(policy.base_weight, BASE_WEIGHT_UNITS);
        assert_eq!(policy.weight_per_vote, WEIGHT_UNITS_PER_VOTE);
        assert_eq!(SubmissionPolicy::default(), policy);
    }

    #[test]
    fn a_submission_inside_the_limit_is_admitted() {
        let submission = submission_of(5, 8);
        let admission = SubmissionGate::new()
            .admit(&submission)
            .expect("the submission passes");
        assert_eq!(admission.votes, 5);
        assert_eq!(admission.bytes, submission.encoded_len());
        assert_eq!(
            admission.weight,
            DeclaredWeight(BASE_WEIGHT_UNITS + 5 * WEIGHT_UNITS_PER_VOTE)
        );
    }

    #[test]
    fn the_declared_weight_scales_with_the_signature_count() {
        let policy = SubmissionPolicy::standard();
        let one = policy.declared_weight(1);
        let ten = policy.declared_weight(10);
        let hundred = policy.declared_weight(100);
        assert!(one < ten && ten < hundred);
        assert_eq!(
            ten.0 - one.0,
            9 * WEIGHT_UNITS_PER_VOTE,
            "each signature adds the same weight"
        );
        assert_eq!(
            policy.ceiling_weight(),
            policy.declared_weight(MAX_VOTES_PER_SUBMISSION)
        );
    }

    #[test]
    fn a_submission_above_the_maximum_count_is_refused_before_pool_admission() {
        let gate = SubmissionGate::with_policy(SubmissionPolicy::new(4, 64 * 1024, 10, 10));
        let submission = submission_of(6, 6);
        let error = gate
            .admit(&submission)
            .expect_err("the count above the limit fails");
        assert_eq!(
            error,
            EngineError::SubmissionCountAboveLimit { count: 6, limit: 4 }
        );
        assert!(error.to_string().contains("Fix:"));
    }

    #[test]
    fn a_bitmap_sized_above_the_maximum_count_is_refused() {
        let gate = SubmissionGate::with_policy(SubmissionPolicy::new(4, 64 * 1024, 10, 10));
        let mut submission = submission_of(2, 4);
        submission.aggregate.signers = SignerBitmap::new(9);
        for position in [0usize, 1] {
            submission
                .aggregate
                .signers
                .set(position)
                .expect("the position sits inside");
        }
        let error = gate
            .admit(&submission)
            .expect_err("a bitmap above the limit fails");
        assert_eq!(
            error,
            EngineError::SubmissionCountAboveLimit { count: 9, limit: 4 }
        );
    }

    #[test]
    fn a_submission_above_the_byte_limit_is_refused() {
        let gate = SubmissionGate::with_policy(SubmissionPolicy::new(256, 32, 10, 10));
        let submission = submission_of(3, 4);
        let error = gate
            .admit(&submission)
            .expect_err("the size above the limit fails");
        assert!(matches!(error, EngineError::SubmissionTooLarge { .. }));
        assert!(error.to_string().contains("Fix:"));
    }

    #[test]
    fn a_submission_naming_more_signers_than_it_carries_is_refused() {
        let mut submission = submission_of(3, 8);
        submission
            .aggregate
            .signers
            .set(7)
            .expect("the position sits inside");
        let error = SubmissionGate::new()
            .admit(&submission)
            .expect_err("a count mismatch fails");
        assert_eq!(
            error,
            EngineError::AggregateCountMismatch {
                named: 4,
                carried: 3
            }
        );
    }

    #[test]
    fn a_submission_with_a_broken_bitmap_is_refused() {
        let mut submission = submission_of(1, 5);
        submission.aggregate.signers = SignerBitmap::new(5);
        submission
            .aggregate
            .signers
            .set(0)
            .expect("the position sits inside");
        // A spare bit past the five member set breaks the shape check.
        let mut bytes = submission.aggregate.signers.encode();
        let last = bytes.len() - 1;
        bytes[last] |= 1u8 << 6;
        submission.aggregate.signers =
            SignerBitmap::decode(&mut &bytes[..]).expect("the encoding decodes");
        let error = SubmissionGate::new()
            .admit(&submission)
            .expect_err("a broken bitmap fails");
        assert_eq!(error, EngineError::AggregateBitmapMalformed);
    }

    #[test]
    fn a_submission_naming_another_slot_is_refused() {
        let mut submission = submission_of(2, 4);
        submission.slot = Slot(SLOT.0 + 1);
        let error = SubmissionGate::new()
            .admit(&submission)
            .expect_err("another slot fails");
        assert!(matches!(error, EngineError::VoteSlotMismatch { .. }));
    }

    #[test]
    fn a_submission_naming_another_block_is_refused() {
        let mut submission = submission_of(2, 4);
        submission.block_hash = [99u8; 32];
        let error = SubmissionGate::new()
            .admit(&submission)
            .expect_err("another block fails");
        assert!(matches!(error, EngineError::VoteBlockMismatch { .. }));
    }

    #[test]
    fn the_weight_of_the_largest_admitted_submission_stays_bounded() {
        let gate = SubmissionGate::new();
        let ceiling = gate.policy().ceiling_weight();
        assert_eq!(
            ceiling.0,
            BASE_WEIGHT_UNITS + WEIGHT_UNITS_PER_VOTE * MAX_VOTES_PER_SUBMISSION as u64
        );
        // Every admitted submission declares at most the ceiling.
        for count in [0u32, 1, 10, MAX_VOTES_PER_SUBMISSION] {
            assert!(gate.policy().declared_weight(count) <= ceiling);
        }
    }

    #[test]
    fn a_submission_round_trips_through_scale() {
        let submission = submission_of(3, 5);
        let bytes = submission.encode();
        assert_eq!(
            CommitSubmission::decode(&mut &bytes[..]).expect("the encoding decodes"),
            submission
        );
    }
}
