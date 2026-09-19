//! Deterministic challenge issue.
//!
//! At a set block interval the engine derives a random value from the block
//! hash and the round number. That value selects one validator and one
//! historical record. Every node derives the same value from state every node
//! already holds, so every node agrees on the challenge with no coordination.
//!
//! Selection spans the full history from genesis, so a validator retaining a
//! recent subset fails over time.

use crate::primitives::CUSTODY_DRAW_DOMAIN;
use codec::Encode;
use sp_core::H256;
use sp_std::vec::Vec;

/// The draw one round produces.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Draw {
    /// The raw bytes the draw produced.
    pub bytes: [u8; 32],
}

impl Draw {
    /// Read the first eight bytes as a number.
    pub fn leading_number(&self) -> u64 {
        let mut raw = [0u8; 8];
        raw.copy_from_slice(&self.bytes[..8]);
        u64::from_be_bytes(raw)
    }

    /// Read the second eight bytes as a number.
    pub fn following_number(&self) -> u64 {
        let mut raw = [0u8; 8];
        raw.copy_from_slice(&self.bytes[8..16]);
        u64::from_be_bytes(raw)
    }

    /// Choose one position inside a span of the given length.
    ///
    /// A span of zero length yields no position.
    pub fn choose(&self, value: u64, length: u64) -> Option<u64> {
        if length == 0 {
            None
        } else {
            Some(value % length)
        }
    }
}

/// Derive the draw for one round.
///
/// The draw mixes a domain separator, the block hash and the round number. The
/// separator keeps this draw apart from every other draw the framework takes
/// over the same block hash.
pub fn derive_draw(block_hash: H256, round: u64) -> Draw {
    let mut preimage = Vec::with_capacity(CUSTODY_DRAW_DOMAIN.len() + 32 + 8);
    preimage.extend_from_slice(CUSTODY_DRAW_DOMAIN);
    preimage.extend_from_slice(block_hash.as_bytes());
    preimage.extend_from_slice(&round.encode());
    Draw {
        bytes: sp_io::hashing::blake2_256(&preimage),
    }
}

/// The selection one draw produces.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Selection {
    /// The position of the chosen validator inside the candidate list.
    pub validator_index: u64,
    /// The historical block the challenge reaches into.
    pub block: u64,
    /// The position of the chosen record inside that block.
    pub record_index: u64,
}

/// Select one validator and one historical record.
///
/// The candidate count is the number of validators the engine may challenge.
/// The history length is the number of blocks holding records, counted from
/// genesis. The record count is the number of records the chosen block holds.
///
/// A call with no candidate, or with no history, yields no selection.
pub fn select(draw: Draw, candidate_count: u64, history_length: u64) -> Option<(u64, u64)> {
    let validator_index = draw.choose(draw.leading_number(), candidate_count)?;
    let block = draw.choose(draw.following_number(), history_length)?;
    Some((validator_index, block))
}

/// Select the record inside the chosen block.
///
/// The draw and the chosen block together pick the record, so two blocks
/// holding different record counts each receive a position inside range.
pub fn select_record(draw: Draw, block: u64, record_count: u64) -> Option<u64> {
    if record_count == 0 {
        return None;
    }
    let mut preimage = Vec::with_capacity(CUSTODY_DRAW_DOMAIN.len() + 32 + 8);
    preimage.extend_from_slice(CUSTODY_DRAW_DOMAIN);
    preimage.extend_from_slice(&draw.bytes);
    preimage.extend_from_slice(&block.encode());
    let mixed = sp_io::hashing::blake2_256(&preimage);
    let mut raw = [0u8; 8];
    raw.copy_from_slice(&mixed[..8]);
    Some(u64::from_be_bytes(raw) % record_count)
}

/// Report whether a block issues a challenge.
///
/// The interval is a policy value. An interval of zero halts issue.
pub fn issues_at(block: u64, interval: u64) -> bool {
    interval > 0 && block > 0 && block % interval == 0
}

/// The round number of one block.
///
/// The round counts the challenges issued so far, so the draw changes even
/// where two blocks carry the same hash.
pub fn round_of(block: u64, interval: u64) -> u64 {
    if interval == 0 {
        0
    } else {
        block / interval
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_node_derives_the_same_draw() {
        let hash = H256::repeat_byte(7);
        assert_eq!(derive_draw(hash, 42), derive_draw(hash, 42));
    }

    #[test]
    fn a_different_block_hash_gives_a_different_draw() {
        assert_ne!(
            derive_draw(H256::repeat_byte(1), 1),
            derive_draw(H256::repeat_byte(2), 1)
        );
    }

    #[test]
    fn a_different_round_gives_a_different_draw() {
        let hash = H256::repeat_byte(3);
        assert_ne!(derive_draw(hash, 1), derive_draw(hash, 2));
    }

    #[test]
    fn a_selection_sits_inside_both_spans() {
        for round in 0..200u64 {
            let draw = derive_draw(H256::repeat_byte(9), round);
            let (validator, block) = select(draw, 7, 1_000).expect("selects");
            assert!(validator < 7);
            assert!(block < 1_000);
        }
    }

    #[test]
    fn an_empty_candidate_list_yields_no_selection() {
        let draw = derive_draw(H256::repeat_byte(1), 1);
        assert_eq!(select(draw, 0, 100), None);
    }

    #[test]
    fn an_empty_history_yields_no_selection() {
        let draw = derive_draw(H256::repeat_byte(1), 1);
        assert_eq!(select(draw, 5, 0), None);
    }

    #[test]
    fn selection_spans_the_whole_history() {
        // Over many rounds the draw reaches both ends of the history rather
        // than clustering near the recent blocks.
        let mut lowest = u64::MAX;
        let mut highest = 0u64;
        for round in 0..500u64 {
            let draw = derive_draw(H256::repeat_byte(5), round);
            let (_, block) = select(draw, 4, 1_000).expect("selects");
            lowest = lowest.min(block);
            highest = highest.max(block);
        }
        assert!(lowest < 50, "the draw reaches the early history");
        assert!(highest > 950, "the draw reaches the recent history");
    }

    #[test]
    fn selection_reaches_every_validator() {
        let mut seen = [false; 5];
        for round in 0..500u64 {
            let draw = derive_draw(H256::repeat_byte(11), round);
            let (validator, _) = select(draw, 5, 100).expect("selects");
            seen[validator as usize] = true;
        }
        assert!(seen.iter().all(|held| *held));
    }

    #[test]
    fn a_record_position_sits_inside_the_block() {
        let draw = derive_draw(H256::repeat_byte(4), 3);
        for count in 1..40u64 {
            let index = select_record(draw, 17, count).expect("selects");
            assert!(index < count);
        }
        assert_eq!(select_record(draw, 17, 0), None);
    }

    #[test]
    fn a_block_holding_no_record_yields_no_record_position() {
        let draw = derive_draw(H256::repeat_byte(4), 3);
        assert_eq!(select_record(draw, 1, 0), None);
    }

    #[test]
    fn the_interval_governs_which_blocks_issue() {
        assert!(!issues_at(0, 10));
        assert!(!issues_at(5, 10));
        assert!(issues_at(10, 10));
        assert!(issues_at(20, 10));
        assert!(!issues_at(10, 0));
    }

    #[test]
    fn the_round_counts_the_challenges_issued() {
        assert_eq!(round_of(10, 10), 1);
        assert_eq!(round_of(20, 10), 2);
        assert_eq!(round_of(25, 10), 2);
        assert_eq!(round_of(25, 0), 0);
    }
}
