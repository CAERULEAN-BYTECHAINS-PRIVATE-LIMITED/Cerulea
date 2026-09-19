//! The shared types of the Custody Engine.
//!
//! The Custody Engine verifies continuously that every validator holds the
//! complete history and serves that history on demand. The mechanism runs for
//! as long as the network produces blocks. No person is involved at any point.
//!
//! Five steps carry the work. Issue derives a challenge from the block hash and
//! the round number. Publish places the challenge in the block header. Answer
//! carries the record and its inclusion proof. Verify walks the proof upward
//! and compares against the record root the chain already holds. Score moves
//! the standing of the challenged validator.

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::pallet_prelude::RuntimeDebug;
use frame_support::BoundedVec;
use scale_info::TypeInfo;
use sp_core::H256;

/// The identifier of the custody digest item in a block header.
///
/// The node-side engines read this identifier to find the challenge a block
/// carries. The value spells the Cerulea Custody Engine in four bytes.
pub const CUSTODY_DIGEST_ID: [u8; 4] = *b"cecu";

/// The domain separator mixed into every custody challenge draw.
pub const CUSTODY_DRAW_DOMAIN: &[u8] = b"cerulea.custody.draw.v1";

/// The domain separator mixed into every custody record leaf.
pub const CUSTODY_LEAF_DOMAIN: &[u8] = b"cerulea.custody.leaf.v1";

/// The domain separator mixed into every custody tree node.
pub const CUSTODY_NODE_DOMAIN: &[u8] = b"cerulea.custody.node.v1";

/// The blocks a challenged validator holds to answer, by default.
///
/// Three blocks close faster than a network fetch and a proof rebuild complete,
/// so a validator holding a subset of the history fails the challenge.
pub const DEFAULT_DEADLINE_BLOCKS: u32 = 3;

/// The identifier of one collection.
///
/// A collection is one named store of records inside a Blueprint.
pub type CollectionId = [u8; 16];

/// The greatest number of bytes a record key carries.
pub const MAX_KEY_BYTES: u32 = 128;

/// A reference to one record inside the committed history.
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
pub struct RecordRef<BlockNumber> {
    /// The block that committed the record.
    pub block: BlockNumber,
    /// The position of the record inside that block.
    pub index: u32,
    /// The collection holding the record.
    pub collection: CollectionId,
    /// The key of the record inside the collection.
    pub key: BoundedVec<u8, frame_support::traits::ConstU32<MAX_KEY_BYTES>>,
}

/// One challenge the Custody Engine issued.
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
pub struct Challenge<ValidatorId, BlockNumber> {
    /// The validator the challenge names.
    pub target: ValidatorId,
    /// The historical block the challenge reaches into.
    pub block: BlockNumber,
    /// The record the validator serves.
    pub record: RecordRef<BlockNumber>,
    /// The block at which the challenge issued.
    pub issued: BlockNumber,
    /// The last block at which an answer counts as timely.
    pub deadline: BlockNumber,
}

/// The state one challenge reached.
#[derive(
    Encode,
    Decode,
    Clone,
    Copy,
    PartialEq,
    Eq,
    RuntimeDebug,
    TypeInfo,
    MaxEncodedLen,
    Default,
    frame_support::__private::codec::DecodeWithMemTracking,
)]
pub enum ChallengeState {
    /// The challenge waits for an answer.
    #[default]
    Open,
    /// A correct proof arrived inside the deadline.
    AnsweredInTime,
    /// The deadline passed with no answer.
    Missed,
    /// A proof arrived and failed verification.
    Incorrect,
}

impl ChallengeState {
    /// A plain sentence stating what happened.
    pub const fn plain_language(&self) -> &'static str {
        match self {
            ChallengeState::Open => "The challenge waits for an answer.",
            ChallengeState::AnsweredInTime => {
                "The validator served the record inside the deadline. Standing rises."
            }
            ChallengeState::Missed => {
                "The deadline passed with no answer. Standing falls."
            }
            ChallengeState::Incorrect => {
                "The proof failed verification. Standing falls sharply and the chain records a fault."
            }
        }
    }
}

/// The custody record one validator carries.
#[derive(
    Encode,
    Decode,
    Clone,
    Copy,
    PartialEq,
    Eq,
    RuntimeDebug,
    TypeInfo,
    MaxEncodedLen,
    Default,
    frame_support::__private::codec::DecodeWithMemTracking,
)]
pub struct CustodyRecord {
    /// Challenges answered correctly inside the deadline.
    pub answered: u32,
    /// Challenges the validator let pass.
    pub missed: u32,
    /// Proofs that failed verification.
    pub faults: u32,
    /// The custody share of the Trust Score, in parts per million.
    pub share: u32,
}

impl CustodyRecord {
    /// The highest custody share, in parts per million.
    pub const FULL_SHARE: u32 = 1_000_000;

    /// The total challenges this validator received and settled.
    pub const fn settled(&self) -> u32 {
        self.answered
            .saturating_add(self.missed)
            .saturating_add(self.faults)
    }
}

/// The inclusion proof one answer carries.
///
/// An inclusion proof is a short chain of hashes showing the record belongs to
/// a committed block. Each entry carries one sibling hash and the side that
/// sibling sits on.
#[derive(
    Encode,
    Decode,
    Clone,
    Copy,
    PartialEq,
    Eq,
    RuntimeDebug,
    TypeInfo,
    MaxEncodedLen,
    frame_support::__private::codec::DecodeWithMemTracking,
)]
pub struct ProofStep {
    /// The sibling hash at this level.
    pub sibling: H256,
    /// The sibling sits on the left where this holds.
    pub sibling_on_left: bool,
}

/// The greatest number of steps a proof carries.
///
/// A tree of two to the power of thirty two leaves needs thirty two steps, so
/// this bound covers every block Cerulea produces.
pub const MAX_PROOF_STEPS: u32 = 32;

/// The greatest number of bytes a served record carries.
pub const MAX_RECORD_BYTES: u32 = 1024;

/// The answer one validator submits.
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
pub struct ChallengeAnswer {
    /// The record the validator read from its local store.
    pub value: BoundedVec<u8, frame_support::traits::ConstU32<MAX_RECORD_BYTES>>,
    /// The chain of sibling hashes from the record up to the block root.
    pub proof: BoundedVec<ProofStep, frame_support::traits::ConstU32<MAX_PROOF_STEPS>>,
}

/// The interface the Custody Engine reads the validator set through.
pub trait CustodyValidators<ValidatorId> {
    /// The validators the engine may challenge, in a stable order.
    ///
    /// Every node derives the same order, so every node derives the same
    /// challenge.
    fn custody_candidates() -> sp_std::vec::Vec<ValidatorId>;
}

impl<ValidatorId> CustodyValidators<ValidatorId> for () {
    fn custody_candidates() -> sp_std::vec::Vec<ValidatorId> {
        sp_std::vec::Vec::new()
    }
}

/// The interface the Custody Engine moves standing through.
///
/// The Standing Engine holds the Trust Score. The Custody Engine reports the
/// custody share and the faults it proves.
pub trait CustodyStanding<ValidatorId> {
    /// Report the custody share of one validator, in parts per million.
    fn report_custody_share(validator: &ValidatorId, share: u32);

    /// Report a proven custody fault.
    fn report_custody_fault(validator: &ValidatorId, faults: u32);
}

impl<ValidatorId> CustodyStanding<ValidatorId> for () {
    fn report_custody_share(_validator: &ValidatorId, _share: u32) {}
    fn report_custody_fault(_validator: &ValidatorId, _faults: u32) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_digest_identifier_holds_four_bytes() {
        assert_eq!(CUSTODY_DIGEST_ID, [b'c', b'e', b'c', b'u']);
    }

    #[test]
    fn the_default_deadline_is_three_blocks() {
        assert_eq!(DEFAULT_DEADLINE_BLOCKS, 3);
    }

    #[test]
    fn a_settled_record_counts_every_outcome() {
        let record = CustodyRecord {
            answered: 4,
            missed: 2,
            faults: 1,
            share: 0,
        };
        assert_eq!(record.settled(), 7);
    }

    #[test]
    fn every_challenge_state_carries_a_plain_sentence() {
        for state in [
            ChallengeState::Open,
            ChallengeState::AnsweredInTime,
            ChallengeState::Missed,
            ChallengeState::Incorrect,
        ] {
            assert!(state.plain_language().len() > 20);
        }
    }

    #[test]
    fn the_three_domain_separators_differ() {
        assert_ne!(CUSTODY_DRAW_DOMAIN, CUSTODY_LEAF_DOMAIN);
        assert_ne!(CUSTODY_LEAF_DOMAIN, CUSTODY_NODE_DOMAIN);
        assert_ne!(CUSTODY_DRAW_DOMAIN, CUSTODY_NODE_DOMAIN);
    }
}
