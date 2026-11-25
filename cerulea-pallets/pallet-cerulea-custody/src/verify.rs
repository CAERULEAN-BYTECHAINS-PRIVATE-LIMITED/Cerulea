//! Proof verification against the record root of a historical block.
//!
//! Every block carries a record root. The root is the top of a binary hash tree
//! built over the records the block committed. A challenged validator sends the
//! record and the chain of sibling hashes that joins the record to that root.
//!
//! Verification hashes the record, walks the chain upward and compares the
//! result against the root the chain already holds for that block. A record
//! that never belonged to the block produces a different top, so the comparison
//! fails.

use crate::primitives::{ProofStep, CUSTODY_LEAF_DOMAIN, CUSTODY_NODE_DOMAIN};
use codec::{Decode, Encode};
use frame_support::pallet_prelude::RuntimeDebug;
use scale_info::TypeInfo;
use sp_core::H256;
use sp_std::vec::Vec;

/// A reason verification fails.
#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, RuntimeDebug, TypeInfo)]
pub enum VerifyError {
    /// The chain holds no record root for that block.
    RootAbsent,
    /// The walk reached a different top than the stored root.
    RootMismatch,
    /// The proof carries more steps than the tree holds levels.
    ProofTooLong,
}

impl VerifyError {
    /// The sentence a person reads, naming the fix.
    pub const fn plain_language(&self) -> &'static str {
        match self {
            VerifyError::RootAbsent => {
                "The chain holds no record root for that block. Name a block that committed records."
            }
            VerifyError::RootMismatch => {
                "The proof reached a different root. Send the record and the proof the named block committed."
            }
            VerifyError::ProofTooLong => {
                "The proof carries more steps than the tree holds levels. Send the shortest chain that reaches the root."
            }
        }
    }
}

/// The hash of one record leaf.
///
/// The leaf mixes a domain separator, the position of the record inside the
/// block and the record bytes. The separator keeps a leaf hash apart from a
/// node hash, so no rearranged proof passes.
pub fn leaf_hash(index: u32, value: &[u8]) -> H256 {
    let mut preimage = Vec::with_capacity(CUSTODY_LEAF_DOMAIN.len() + 4 + value.len());
    preimage.extend_from_slice(CUSTODY_LEAF_DOMAIN);
    preimage.extend_from_slice(&index.to_be_bytes());
    preimage.extend_from_slice(value);
    H256::from(sp_io::hashing::blake2_256(&preimage))
}

/// The hash of one inner node.
pub fn node_hash(left: &H256, right: &H256) -> H256 {
    let mut preimage = Vec::with_capacity(CUSTODY_NODE_DOMAIN.len() + 64);
    preimage.extend_from_slice(CUSTODY_NODE_DOMAIN);
    preimage.extend_from_slice(left.as_bytes());
    preimage.extend_from_slice(right.as_bytes());
    H256::from(sp_io::hashing::blake2_256(&preimage))
}

/// The record root over a block of leaves.
///
/// A level holding an odd count carries its last node upward unchanged. A block
/// holding no record carries the zero root.
pub fn record_root(leaves: &[H256]) -> H256 {
    if leaves.is_empty() {
        return H256::zero();
    }
    let mut level: Vec<H256> = leaves.to_vec();
    while level.len() > 1 {
        let mut next = Vec::with_capacity(level.len().div_ceil(2));
        for chunk in level.chunks(2) {
            if chunk.len() == 2 {
                next.push(node_hash(&chunk[0], &chunk[1]));
            } else {
                next.push(chunk[0]);
            }
        }
        level = next;
    }
    level[0]
}

/// The chain of sibling hashes joining one leaf to the root.
///
/// The chain runs from the leaf upward. A validator builds the same chain from
/// its own copy of the block.
pub fn build_proof(leaves: &[H256], index: usize) -> Vec<ProofStep> {
    let mut steps = Vec::new();
    if index >= leaves.len() {
        return steps;
    }
    let mut level: Vec<H256> = leaves.to_vec();
    let mut position = index;
    while level.len() > 1 {
        let sibling_index = if position % 2 == 0 {
            position + 1
        } else {
            position - 1
        };
        if sibling_index < level.len() {
            steps.push(ProofStep {
                sibling: level[sibling_index],
                sibling_on_left: position % 2 == 1,
            });
        }
        let mut next = Vec::with_capacity(level.len().div_ceil(2));
        for chunk in level.chunks(2) {
            if chunk.len() == 2 {
                next.push(node_hash(&chunk[0], &chunk[1]));
            } else {
                next.push(chunk[0]);
            }
        }
        level = next;
        position /= 2;
    }
    steps
}

/// Walk a proof upward from one leaf and report the top it reaches.
pub fn walk_proof(leaf: H256, proof: &[ProofStep]) -> H256 {
    let mut current = leaf;
    for step in proof.iter() {
        current = if step.sibling_on_left {
            node_hash(&step.sibling, &current)
        } else {
            node_hash(&current, &step.sibling)
        };
    }
    current
}

/// Verify one answer against the record root of a historical block.
///
/// The call hashes the record, walks the proof upward and compares the result
/// against the stored root.
pub fn verify_against_root(
    root: H256,
    index: u32,
    value: &[u8],
    proof: &[ProofStep],
) -> Result<(), VerifyError> {
    if proof.len() > crate::primitives::MAX_PROOF_STEPS as usize {
        return Err(VerifyError::ProofTooLong);
    }
    let reached = walk_proof(leaf_hash(index, value), proof);
    if reached == root {
        Ok(())
    } else {
        Err(VerifyError::RootMismatch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaves(count: u32) -> Vec<H256> {
        (0..count).map(|i| leaf_hash(i, &[i as u8; 8])).collect()
    }

    #[test]
    fn a_block_holding_no_record_carries_the_zero_root() {
        assert_eq!(record_root(&[]), H256::zero());
    }

    #[test]
    fn a_block_holding_one_record_carries_that_leaf_as_the_root() {
        let set = leaves(1);
        assert_eq!(record_root(&set), set[0]);
    }

    #[test]
    fn every_record_in_a_block_proves_its_membership() {
        for count in 1..=17u32 {
            let set = leaves(count);
            let root = record_root(&set);
            for index in 0..count {
                let proof = build_proof(&set, index as usize);
                assert_eq!(
                    verify_against_root(root, index, &[index as u8; 8], &proof),
                    Ok(()),
                    "record {} of {} proves membership",
                    index,
                    count
                );
            }
        }
    }

    #[test]
    fn a_record_the_block_never_held_fails_verification() {
        let set = leaves(8);
        let root = record_root(&set);
        let proof = build_proof(&set, 3);
        assert_eq!(
            verify_against_root(root, 3, b"a record the block never held", &proof),
            Err(VerifyError::RootMismatch)
        );
    }

    #[test]
    fn a_proof_from_another_position_fails_verification() {
        let set = leaves(8);
        let root = record_root(&set);
        let proof = build_proof(&set, 5);
        assert_eq!(
            verify_against_root(root, 3, &[3u8; 8], &proof),
            Err(VerifyError::RootMismatch)
        );
    }

    #[test]
    fn a_proof_against_another_block_root_fails_verification() {
        let first = leaves(8);
        let second = leaves(9);
        let proof = build_proof(&first, 2);
        assert_eq!(
            verify_against_root(record_root(&second), 2, &[2u8; 8], &proof),
            Err(VerifyError::RootMismatch)
        );
    }

    #[test]
    fn a_leaf_hash_differs_from_a_node_hash_over_the_same_bytes() {
        let left = H256::repeat_byte(1);
        let right = H256::repeat_byte(2);
        let node = node_hash(&left, &right);
        let mut bytes = Vec::new();
        bytes.extend_from_slice(left.as_bytes());
        bytes.extend_from_slice(right.as_bytes());
        assert_ne!(node, leaf_hash(0, &bytes));
    }

    #[test]
    fn the_position_enters_the_leaf_hash() {
        assert_ne!(leaf_hash(0, b"same bytes"), leaf_hash(1, b"same bytes"));
    }

    #[test]
    fn a_proof_longer_than_the_bound_is_refused() {
        let step = ProofStep {
            sibling: H256::zero(),
            sibling_on_left: false,
        };
        let proof = sp_std::vec![step; (crate::primitives::MAX_PROOF_STEPS + 1) as usize];
        assert_eq!(
            verify_against_root(H256::zero(), 0, b"record", &proof),
            Err(VerifyError::ProofTooLong)
        );
    }

    #[test]
    fn every_verification_failure_names_a_fix() {
        for failure in [
            VerifyError::RootAbsent,
            VerifyError::RootMismatch,
            VerifyError::ProofTooLong,
        ] {
            assert!(failure.plain_language().len() > 20);
        }
    }
}
