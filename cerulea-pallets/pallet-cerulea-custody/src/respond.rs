//! The validator response path.
//!
//! The challenged validator reads the record from its local store, builds the
//! inclusion proof and submits both. This module holds the shape of an answer
//! and the helper that assembles one from a local copy of a block.
//!
//! The helper runs off chain as well as on chain, so a validator prepares the
//! answer with the same code the chain verifies against.

use crate::primitives::{Challenge, ChallengeAnswer, ProofStep, MAX_PROOF_STEPS, MAX_RECORD_BYTES};
use crate::verify::{build_proof, leaf_hash, record_root};
use codec::{Decode, Encode};
use frame_support::pallet_prelude::RuntimeDebug;
use frame_support::BoundedVec;
use scale_info::TypeInfo;
use sp_core::H256;
use sp_std::vec::Vec;

/// A reason an answer fails to assemble or to admit.
#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, RuntimeDebug, TypeInfo)]
pub enum RespondError {
    /// The local copy of the block holds no record at that position.
    RecordAbsent,
    /// The record carries more bytes than an answer holds.
    RecordTooLong,
    /// The proof carries more steps than an answer holds.
    ProofTooLong,
    /// The account submitting the answer is not the challenged validator.
    WrongValidator,
}

impl RespondError {
    /// The sentence a person reads, naming the fix.
    pub const fn plain_language(&self) -> &'static str {
        match self {
            RespondError::RecordAbsent => {
                "The local copy holds no record at that position. Fetch the full history, then answer again."
            }
            RespondError::RecordTooLong => {
                "The record carries more bytes than an answer holds. Split the record across entries."
            }
            RespondError::ProofTooLong => {
                "The proof carries more steps than an answer holds. Build the proof against the named block alone."
            }
            RespondError::WrongValidator => {
                "The challenge names another validator. Answer the challenge that names this account."
            }
        }
    }
}

/// Assemble an answer from a local copy of one block.
///
/// The caller passes the records the block committed, in the order the block
/// committed them. The helper hashes each record, builds the tree, takes the
/// chain of siblings for the chosen position and returns the answer.
pub fn assemble_answer(
    block_records: &[Vec<u8>],
    index: u32,
) -> Result<ChallengeAnswer, RespondError> {
    let position = index as usize;
    let value = block_records
        .get(position)
        .ok_or(RespondError::RecordAbsent)?;

    let leaves: Vec<H256> = block_records
        .iter()
        .enumerate()
        .map(|(at, record)| leaf_hash(at as u32, record))
        .collect();
    let proof = build_proof(&leaves, position);

    let value: BoundedVec<u8, frame_support::traits::ConstU32<MAX_RECORD_BYTES>> =
        BoundedVec::try_from(value.clone()).map_err(|_| RespondError::RecordTooLong)?;
    let proof: BoundedVec<ProofStep, frame_support::traits::ConstU32<MAX_PROOF_STEPS>> =
        BoundedVec::try_from(proof).map_err(|_| RespondError::ProofTooLong)?;

    Ok(ChallengeAnswer { value, proof })
}

/// The record root over a local copy of one block.
///
/// A validator computes the root to check its own copy before answering.
pub fn local_record_root(block_records: &[Vec<u8>]) -> H256 {
    let leaves: Vec<H256> = block_records
        .iter()
        .enumerate()
        .map(|(at, record)| leaf_hash(at as u32, record))
        .collect();
    record_root(&leaves)
}

/// Report whether an account may answer one challenge.
pub fn may_answer<ValidatorId, BlockNumber>(
    challenge: &Challenge<ValidatorId, BlockNumber>,
    account: &ValidatorId,
) -> Result<(), RespondError>
where
    ValidatorId: PartialEq,
{
    if &challenge.target == account {
        Ok(())
    } else {
        Err(RespondError::WrongValidator)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives::RecordRef;
    use crate::verify::verify_against_root;

    fn block_records(count: usize) -> Vec<Vec<u8>> {
        (0..count).map(|i| sp_std::vec![i as u8; 16]).collect()
    }

    fn challenge(target: u64) -> Challenge<u64, u64> {
        Challenge {
            target,
            block: 5,
            record: RecordRef {
                block: 5,
                index: 1,
                collection: [0u8; 16],
                key: BoundedVec::default(),
            },
            issued: 10,
            deadline: 13,
        }
    }

    #[test]
    fn an_assembled_answer_verifies_against_the_block_root() {
        let records = block_records(9);
        let root = local_record_root(&records);
        for index in 0..records.len() as u32 {
            let answer = assemble_answer(&records, index).expect("assembles");
            assert_eq!(
                verify_against_root(root, index, &answer.value, &answer.proof),
                Ok(())
            );
        }
    }

    #[test]
    fn an_absent_record_names_the_fix() {
        let records = block_records(3);
        assert_eq!(
            assemble_answer(&records, 9),
            Err(RespondError::RecordAbsent)
        );
        assert!(RespondError::RecordAbsent.plain_language().len() > 20);
    }

    #[test]
    fn a_record_above_the_byte_bound_is_refused() {
        let records = sp_std::vec![sp_std::vec![1u8; (MAX_RECORD_BYTES + 1) as usize]];
        assert_eq!(
            assemble_answer(&records, 0),
            Err(RespondError::RecordTooLong)
        );
    }

    #[test]
    fn only_the_named_validator_answers() {
        let held = challenge(7);
        assert_eq!(may_answer(&held, &7u64), Ok(()));
        assert_eq!(may_answer(&held, &8u64), Err(RespondError::WrongValidator));
    }

    #[test]
    fn a_validator_holding_a_different_block_builds_a_failing_answer() {
        let held = block_records(8);
        let other = block_records(9);
        let root = local_record_root(&held);
        let answer = assemble_answer(&other, 2).expect("assembles");
        assert!(verify_against_root(root, 2, &answer.value, &answer.proof).is_err());
    }

    #[test]
    fn every_response_failure_names_a_fix() {
        for failure in [
            RespondError::RecordAbsent,
            RespondError::RecordTooLong,
            RespondError::ProofTooLong,
            RespondError::WrongValidator,
        ] {
            assert!(failure.plain_language().len() > 20);
        }
    }
}
