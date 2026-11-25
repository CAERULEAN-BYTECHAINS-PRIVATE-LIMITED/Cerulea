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

/// The largest value one direct record commit carries, in bytes.
///
/// The figure matches the largest record the state module stores, so a
/// direct commit covers the full stored bytes of any record.
pub const MAX_COMMITTED_VALUE_BYTES: u32 = 65_536;

/// The interface a record store commits records through.
///
/// The state module calls the hook for every record an insert, an update or
/// a removal commits. The Custody Engine adds the record to the record tree of
/// the block under construction, so every committed record is challengeable.
pub trait CustodyRecordHook {
    /// Note one committed record.
    ///
    /// The collection and the key name the record. An absent value marks a
    /// removal. A failure names the fix and stops the write.
    fn note_committed(
        collection: &[u8],
        key: &[u8],
        value: Option<&[u8]>,
    ) -> frame_support::dispatch::DispatchResult;
}

impl CustodyRecordHook for () {
    fn note_committed(
        _collection: &[u8],
        _key: &[u8],
        _value: Option<&[u8]>,
    ) -> frame_support::dispatch::DispatchResult {
        Ok(())
    }
}

/// The prefix of every custody entry the runtime writes to the local store
/// of the node.
pub const CUSTODY_LOCAL_PREFIX: &[u8] = b"cerulea/custody/entry/";

/// The local store key of one custody entry.
///
/// The key joins the block number, the hash of the parent block and the
/// position of the entry. The parent hash keeps two blocks at one height on
/// two forks apart. A validator answering a challenge reads the entries of
/// the challenged block under these keys, with no network fetch.
pub fn local_entry_key(block: u64, parent: &[u8; 32], index: u32) -> sp_std::vec::Vec<u8> {
    let mut key = CUSTODY_LOCAL_PREFIX.to_vec();
    key.extend_from_slice(&block.to_be_bytes());
    key.extend_from_slice(parent);
    key.extend_from_slice(&index.to_be_bytes());
    key
}

/// The custody entry one committed record contributes to its block.
///
/// The entry joins the collection, the key and the digest of the value. A
/// removal carries the zero digest. The entry stays inside the record bound
/// of an answer whatever the size of the stored value, and the digest binds
/// the entry to the exact bytes the store committed.
pub fn custody_entry(collection: &[u8], key: &[u8], value: Option<&[u8]>) -> sp_std::vec::Vec<u8> {
    let digest = match value {
        Some(bytes) => sp_io::hashing::blake2_256(bytes),
        None => [0u8; 32],
    };
    (collection, key, digest).encode()
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

    /// Encode a value, decode it back and read it as debug text.
    fn round_trip<T: Encode + Decode + PartialEq + core::fmt::Debug>(value: &T) -> usize {
        let bytes = value.encode();
        assert_eq!(T::decode(&mut &bytes[..]).as_ref(), Ok(value));
        assert!(T::decode(&mut &bytes[..bytes.len() - 1]).is_err());
        assert!(!format!("{value:?}").is_empty());
        bytes.len()
    }

    fn record_ref() -> RecordRef<u32> {
        RecordRef {
            block: 7,
            index: 2,
            collection: [3u8; 16],
            key: BoundedVec::truncate_from(b"12/3".to_vec()),
        }
    }

    /// The derived encoding, decoding, debug text and type records of every
    /// custody type run here, so every region of the module is reached.
    #[test]
    fn every_custody_type_encodes_decodes_and_describes_itself() {
        let reference = record_ref();
        assert_eq!(round_trip(&reference), 4 + 4 + 16 + 1 + 4);
        assert!(reference.encode().len() <= RecordRef::<u32>::max_encoded_len());
        assert_eq!(reference.clone(), reference);
        assert_eq!(
            <RecordRef<u32> as TypeInfo>::type_info().path.ident(),
            Some("RecordRef")
        );

        let challenge = Challenge::<u64, u32> {
            target: 9,
            block: 7,
            record: reference.clone(),
            issued: 10,
            deadline: 13,
        };
        assert_eq!(round_trip(&challenge), 8 + 4 + 29 + 4 + 4);
        assert!(challenge.encode().len() <= Challenge::<u64, u32>::max_encoded_len());
        assert_eq!(challenge.clone(), challenge);
        assert_eq!(
            <Challenge<u64, u32> as TypeInfo>::type_info().path.ident(),
            Some("Challenge")
        );

        for (position, state) in [
            ChallengeState::Open,
            ChallengeState::AnsweredInTime,
            ChallengeState::Missed,
            ChallengeState::Incorrect,
        ]
        .iter()
        .enumerate()
        {
            assert_eq!(round_trip(state), 1);
            assert_eq!(state.encode(), sp_std::vec![position as u8]);
            #[allow(clippy::clone_on_copy)]
            let copy = state.clone();
            assert_eq!(copy, *state);
        }
        assert_eq!(ChallengeState::default(), ChallengeState::Open);
        assert_eq!(ChallengeState::max_encoded_len(), 1);
        assert_eq!(
            <ChallengeState as TypeInfo>::type_info().path.ident(),
            Some("ChallengeState")
        );

        let record = CustodyRecord {
            answered: 1,
            missed: 2,
            faults: 3,
            share: 4,
        };
        assert_eq!(round_trip(&record), 16);
        assert_eq!(CustodyRecord::max_encoded_len(), 16);
        assert_eq!(CustodyRecord::default().settled(), 0);
        #[allow(clippy::clone_on_copy)]
        let copy = record.clone();
        assert_eq!(copy, record);
        assert_eq!(
            <CustodyRecord as TypeInfo>::type_info().path.ident(),
            Some("CustodyRecord")
        );
        let full = CustodyRecord {
            answered: u32::MAX,
            missed: u32::MAX,
            faults: 1,
            share: 0,
        };
        assert_eq!(full.settled(), u32::MAX);

        let step = ProofStep {
            sibling: H256::repeat_byte(5),
            sibling_on_left: true,
        };
        assert_eq!(round_trip(&step), 33);
        assert_eq!(ProofStep::max_encoded_len(), 33);
        #[allow(clippy::clone_on_copy)]
        let copy = step.clone();
        assert_eq!(copy, step);
        assert_eq!(
            <ProofStep as TypeInfo>::type_info().path.ident(),
            Some("ProofStep")
        );

        let answer = ChallengeAnswer {
            value: BoundedVec::truncate_from(b"Anitha holds 400 square metres".to_vec()),
            proof: BoundedVec::truncate_from(sp_std::vec![step, step]),
        };
        assert_eq!(round_trip(&answer), 1 + 30 + 1 + 66);
        assert!(answer.encode().len() <= ChallengeAnswer::max_encoded_len());
        assert_eq!(answer.clone(), answer);
        assert_eq!(
            <ChallengeAnswer as TypeInfo>::type_info().path.ident(),
            Some("ChallengeAnswer")
        );
    }

    /// The empty validator set, the empty standing hook and the empty record
    /// hook do nothing and refuse nothing.
    #[test]
    fn the_empty_interfaces_answer_with_nothing() {
        assert!(<() as CustodyValidators<u64>>::custody_candidates().is_empty());
        <() as CustodyStanding<u64>>::report_custody_share(&1, 5);
        <() as CustodyStanding<u64>>::report_custody_fault(&1, 5);
        assert_eq!(
            <() as CustodyRecordHook>::note_committed(b"c", b"k", None),
            Ok(())
        );
    }

    #[test]
    fn a_custody_entry_digests_the_value_and_marks_a_removal() {
        let present = custody_entry(b"parcels", b"12/3", Some(b"deed"));
        let absent = custody_entry(b"parcels", b"12/3", None);
        assert_ne!(present, absent);
        let (collection, key, digest): (sp_std::vec::Vec<u8>, sp_std::vec::Vec<u8>, [u8; 32]) =
            Decode::decode(&mut &absent[..]).expect("the entry decodes");
        assert_eq!(collection, b"parcels");
        assert_eq!(key, b"12/3");
        assert_eq!(digest, [0u8; 32]);
        let (_, _, digest): (sp_std::vec::Vec<u8>, sp_std::vec::Vec<u8>, [u8; 32]) =
            Decode::decode(&mut &present[..]).expect("the entry decodes");
        assert_eq!(digest, sp_io::hashing::blake2_256(b"deed"));
    }
}
