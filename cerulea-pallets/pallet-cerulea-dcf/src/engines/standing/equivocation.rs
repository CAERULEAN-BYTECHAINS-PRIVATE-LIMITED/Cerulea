//! The equivocation fault record of the Standing Engine.
//!
//! Equivocation means one validator sealing two different headers for one slot
//! at one rank. A slot is one fixed span of time. The Authoring Engine names one
//! author at each rank of each slot. An author that seals two headers for one
//! slot has signed two futures for one moment.
//!
//! The chain accepts evidence rather than suspicion. The evidence carries both
//! sealed headers with the signature the author placed on each one. The shared
//! primitives crate runs the check, so the node and the chain reach the same
//! answer over one proof.
//!
//! A proven equivocation returns standing to zero through the reset path and
//! records the fault here. The record states what the chain proved and when, so
//! an operator reads the history without reading the event stream.

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::pallet_prelude::RuntimeDebug;
use scale_info::TypeInfo;

/// The record of one proven equivocation.
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
pub struct FaultRecord {
    /// The slot both headers served.
    pub slot: u64,
    /// The rank the author claimed in both headers.
    pub rank: u8,
    /// The epoch the chain recorded the fault at.
    pub epoch: u32,
    /// The block the chain recorded the fault at.
    pub block: u32,
    /// The pre-seal hash of the first header.
    pub first_hash: [u8; 32],
    /// The pre-seal hash of the second header.
    pub second_hash: [u8; 32],
    /// The epochs of service the reset destroyed.
    pub epochs_destroyed: u32,
}

/// The key one proven equivocation sits under.
///
/// The key joins the slot, the rank and the author seal key. One key takes one
/// penalty, so a second report of one equivocation changes nothing.
#[derive(
    Encode,
    Decode,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    RuntimeDebug,
    TypeInfo,
    MaxEncodedLen,
    frame_support::__private::codec::DecodeWithMemTracking,
)]
pub struct FaultKey {
    /// The slot both headers served.
    pub slot: u64,
    /// The rank the author claimed.
    pub rank: u8,
    /// The seal key that signed both headers.
    pub seal_key: [u8; 32],
}

impl FaultKey {
    /// Build the key of one proven equivocation.
    pub const fn new(slot: u64, rank: u8, seal_key: [u8; 32]) -> Self {
        FaultKey {
            slot,
            rank,
            seal_key,
        }
    }

    /// The key the given facts sit under.
    pub const fn of(facts: &cerulea_primitives::equivocation::EquivocationFacts) -> Self {
        FaultKey::new(facts.slot, facts.rank, facts.seal_key)
    }
}

/// Build the fault record one set of proven facts produces.
pub fn fault_record(
    facts: &cerulea_primitives::equivocation::EquivocationFacts,
    epoch: u32,
    block: u32,
    epochs_destroyed: u32,
) -> FaultRecord {
    FaultRecord {
        slot: facts.slot,
        rank: facts.rank,
        epoch,
        block,
        first_hash: facts.first_hash,
        second_hash: facts.second_hash,
        epochs_destroyed,
    }
}

/// A plain sentence stating what the chain proved.
pub const EQUIVOCATION_PLAIN_LANGUAGE: &str =
    "The validator sealed two blocks for one slot. Standing returns to zero and the seat goes.";

#[cfg(test)]
mod tests {
    use super::*;
    use cerulea_primitives::equivocation::EquivocationFacts;

    fn facts() -> EquivocationFacts {
        EquivocationFacts {
            slot: 77,
            rank: 2,
            seal_key: [5u8; 32],
            draw_key: [6u8; 32],
            first_hash: [1u8; 32],
            second_hash: [2u8; 32],
        }
    }

    #[test]
    fn one_equivocation_sits_under_one_key() {
        let key = FaultKey::of(&facts());
        assert_eq!(key, FaultKey::new(77, 2, [5u8; 32]));
        assert_eq!(key, FaultKey::of(&facts()));
    }

    #[test]
    fn two_slots_take_two_keys() {
        let mut other = facts();
        other.slot = 78;
        assert_ne!(FaultKey::of(&facts()), FaultKey::of(&other));
    }

    #[test]
    fn two_ranks_take_two_keys() {
        let mut other = facts();
        other.rank = 3;
        assert_ne!(FaultKey::of(&facts()), FaultKey::of(&other));
    }

    #[test]
    fn two_authors_take_two_keys() {
        let mut other = facts();
        other.seal_key = [9u8; 32];
        assert_ne!(FaultKey::of(&facts()), FaultKey::of(&other));
    }

    #[test]
    fn the_record_carries_what_the_chain_proved() {
        let record = fault_record(&facts(), 12, 3_400, 90);
        assert_eq!(record.slot, 77);
        assert_eq!(record.rank, 2);
        assert_eq!(record.epoch, 12);
        assert_eq!(record.block, 3_400);
        assert_eq!(record.first_hash, [1u8; 32]);
        assert_eq!(record.second_hash, [2u8; 32]);
        assert_eq!(record.epochs_destroyed, 90);
    }

    #[test]
    fn the_plain_sentence_states_the_penalty() {
        assert!(EQUIVOCATION_PLAIN_LANGUAGE.contains("Standing returns to zero"));
    }
}
