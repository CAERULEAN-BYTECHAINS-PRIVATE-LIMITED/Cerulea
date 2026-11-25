//! Checkpoint anchoring, shared between the node and the pallet set.
//!
//! A long-range attack rewrites history from a point far below the head. The
//! attacker builds a second chain from an old block and offers it to a node
//! that holds little history of its own. A fresh node joining the network is
//! the target that matters. Nothing inside the block rules tells that node
//! which of the two chains the network lived through.
//!
//! A checkpoint answers that question. A checkpoint is one committed block the
//! chain names as settled. The chain records the checkpoint. Every node keeps a
//! copy in its own store. A node refuses any chain that disagrees with an
//! anchored checkpoint, and refuses to reorganise below the highest anchored
//! checkpoint it holds.
//!
//! # What a checkpoint carries
//!
//! 1. **The index.** Checkpoints run from zero upward with no gaps.
//! 2. **The block.** The block number the checkpoint settles.
//! 3. **The block hash.** The hash of that block.
//! 4. **The state root.** The combined state root of that block.
//!
//! The four values fold into one thirty two byte commitment under a Cerulea
//! separator. That commitment is the value the Bitcoin anchoring path of the
//! bridge module writes into a data carrying output, so an external network
//! witnesses the Cerulea history.

use alloc::vec::Vec;
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_core::hashing::blake2_256;

/// The separator mixed into every checkpoint commitment.
pub const CHECKPOINT_DOMAIN: &[u8] = b"cerulea.checkpoint.v1";

/// The blocks between two checkpoints in the starting profile.
///
/// The figure sets how far a fresh node trusts block rules alone before an
/// anchor settles the question. A deployment profile raises or lowers the
/// figure through the Amendment Engine.
pub const DEFAULT_CHECKPOINT_INTERVAL: u32 = 900;

/// One anchored checkpoint.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    DecodeWithMemTracking,
    TypeInfo,
    MaxEncodedLen,
)]
pub struct Checkpoint {
    /// The position of this checkpoint in the anchored set.
    pub index: u32,
    /// The block this checkpoint settles.
    pub block: u32,
    /// The hash of that block.
    pub hash: [u8; 32],
    /// The combined state root of that block.
    pub state_root: [u8; 32],
}

impl Checkpoint {
    /// Build one checkpoint from its four values.
    pub const fn new(index: u32, block: u32, hash: [u8; 32], state_root: [u8; 32]) -> Self {
        Checkpoint {
            index,
            block,
            hash,
            state_root,
        }
    }

    /// The thirty two byte commitment this checkpoint folds into.
    ///
    /// The commitment joins the Cerulea separator, the index, the block number,
    /// the block hash and the state root. Every node derives the same value, so
    /// an external witness of the commitment witnesses the same history on
    /// every node.
    pub fn commitment(&self) -> [u8; 32] {
        let mut preimage = Vec::with_capacity(CHECKPOINT_DOMAIN.len() + 72);
        preimage.extend_from_slice(CHECKPOINT_DOMAIN);
        preimage.extend_from_slice(&self.index.to_be_bytes());
        preimage.extend_from_slice(&self.block.to_be_bytes());
        preimage.extend_from_slice(&self.hash);
        preimage.extend_from_slice(&self.state_root);
        blake2_256(&preimage)
    }
}

/// Report whether one block number falls on a checkpoint boundary.
///
/// An interval of zero raises no checkpoint, so a deployment that anchors
/// nothing keeps producing blocks.
pub const fn checkpoint_due(block: u32, interval: u32) -> bool {
    interval > 0 && block > 0 && block % interval == 0
}

/// The index a checkpoint at one block number takes.
pub const fn checkpoint_index(block: u32, interval: u32) -> u32 {
    if interval == 0 {
        0
    } else {
        block / interval
    }
}

/// A reason one chain fails the anchored checkpoint check.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    DecodeWithMemTracking,
    TypeInfo,
    MaxEncodedLen,
)]
pub enum CheckpointFault {
    /// The chain names another block at an anchored height.
    HashMismatch,
    /// The chain would reorganise below the highest anchored checkpoint.
    BelowAnchor,
    /// The checkpoint carries an index out of order with the set.
    OutOfOrder,
    /// The checkpoint names a block below the checkpoint before it.
    BlockOutOfOrder,
}

impl CheckpointFault {
    /// A plain sentence naming the problem and the fix.
    pub const fn plain_language(&self) -> &'static str {
        match self {
            CheckpointFault::HashMismatch => {
                "The chain offered names another block at an anchored checkpoint height. \
                 Fix: sync from a peer whose history matches the anchored checkpoints."
            }
            CheckpointFault::BelowAnchor => {
                "The chain offered would replace blocks below the highest anchored checkpoint. \
                 Fix: sync from a peer that builds above the anchored checkpoints."
            }
            CheckpointFault::OutOfOrder => {
                "The checkpoint carries an index out of order with the anchored set. \
                 Fix: anchor checkpoints in index order starting from zero."
            }
            CheckpointFault::BlockOutOfOrder => {
                "The checkpoint names a block at or below the checkpoint before it. \
                 Fix: anchor each checkpoint above the block the one before it settles."
            }
        }
    }
}

/// Check that one checkpoint follows another in the anchored set.
///
/// The index rises by one. The block number rises. A set built this way carries
/// no gap and no repeat, so the highest checkpoint settles every one below it.
pub fn follows(previous: Option<&Checkpoint>, next: &Checkpoint) -> Result<(), CheckpointFault> {
    match previous {
        None => {
            if next.index == 0 {
                Ok(())
            } else {
                Err(CheckpointFault::OutOfOrder)
            }
        }
        Some(held) => {
            if next.index != held.index.saturating_add(1) {
                return Err(CheckpointFault::OutOfOrder);
            }
            if next.block <= held.block {
                return Err(CheckpointFault::BlockOutOfOrder);
            }
            Ok(())
        }
    }
}

/// Check one block against an anchored set.
///
/// A block sitting at an anchored height carries the hash that checkpoint
/// names. A block at any other height passes this check.
pub fn accepts_block(
    anchors: &[Checkpoint],
    block: u32,
    hash: &[u8; 32],
) -> Result<(), CheckpointFault> {
    match anchors.iter().find(|anchor| anchor.block == block) {
        Some(anchor) if &anchor.hash != hash => Err(CheckpointFault::HashMismatch),
        _ => Ok(()),
    }
}

/// The highest block an anchored set settles.
pub fn highest_anchored_block(anchors: &[Checkpoint]) -> u32 {
    anchors.iter().map(|anchor| anchor.block).max().unwrap_or(0)
}

/// Check whether a chain may take the place of the chain a node holds.
///
/// A reorganisation replaces the blocks above a common ancestor. The node
/// refuses any reorganisation whose common ancestor sits below the highest
/// anchored checkpoint. That refusal is what closes the long-range attack.
pub fn accepts_reorganisation(
    anchors: &[Checkpoint],
    common_ancestor: u32,
) -> Result<(), CheckpointFault> {
    if common_ancestor < highest_anchored_block(anchors) {
        Err(CheckpointFault::BelowAnchor)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checkpoint(index: u32, block: u32, byte: u8) -> Checkpoint {
        Checkpoint::new(index, block, [byte; 32], [byte.wrapping_add(1); 32])
    }

    #[test]
    fn one_checkpoint_folds_into_one_commitment() {
        let one = checkpoint(0, 900, 1);
        assert_eq!(one.commitment(), one.commitment());
        assert_ne!(one.commitment(), checkpoint(1, 900, 1).commitment());
        assert_ne!(one.commitment(), checkpoint(0, 1_800, 1).commitment());
        assert_ne!(one.commitment(), checkpoint(0, 900, 2).commitment());
    }

    #[test]
    fn a_changed_state_root_changes_the_commitment() {
        let mut one = checkpoint(0, 900, 1);
        let before = one.commitment();
        one.state_root = [9u8; 32];
        assert_ne!(one.commitment(), before);
    }

    #[test]
    fn a_checkpoint_falls_due_on_the_interval() {
        assert!(checkpoint_due(900, 900));
        assert!(checkpoint_due(1_800, 900));
        assert!(!checkpoint_due(899, 900));
        assert!(!checkpoint_due(0, 900));
        assert!(!checkpoint_due(900, 0));
    }

    #[test]
    fn the_index_follows_the_interval() {
        assert_eq!(checkpoint_index(900, 900), 1);
        assert_eq!(checkpoint_index(1_800, 900), 2);
        assert_eq!(checkpoint_index(900, 0), 0);
    }

    #[test]
    fn the_anchored_set_starts_at_index_zero() {
        assert_eq!(follows(None, &checkpoint(0, 900, 1)), Ok(()));
        assert_eq!(
            follows(None, &checkpoint(1, 900, 1)),
            Err(CheckpointFault::OutOfOrder)
        );
    }

    #[test]
    fn the_anchored_set_carries_no_gap() {
        let held = checkpoint(0, 900, 1);
        assert_eq!(follows(Some(&held), &checkpoint(1, 1_800, 2)), Ok(()));
        assert_eq!(
            follows(Some(&held), &checkpoint(2, 1_800, 2)),
            Err(CheckpointFault::OutOfOrder)
        );
    }

    #[test]
    fn the_anchored_set_rises_in_block_number() {
        let held = checkpoint(0, 900, 1);
        assert_eq!(
            follows(Some(&held), &checkpoint(1, 900, 2)),
            Err(CheckpointFault::BlockOutOfOrder)
        );
        assert_eq!(
            follows(Some(&held), &checkpoint(1, 800, 2)),
            Err(CheckpointFault::BlockOutOfOrder)
        );
    }

    #[test]
    fn a_block_at_an_anchored_height_carries_the_anchored_hash() {
        let anchors = [checkpoint(0, 900, 1), checkpoint(1, 1_800, 2)];
        assert_eq!(accepts_block(&anchors, 900, &[1u8; 32]), Ok(()));
        assert_eq!(
            accepts_block(&anchors, 900, &[7u8; 32]),
            Err(CheckpointFault::HashMismatch)
        );
        assert_eq!(accepts_block(&anchors, 901, &[7u8; 32]), Ok(()));
    }

    #[test]
    fn a_reorganisation_below_the_highest_anchor_is_refused() {
        let anchors = [checkpoint(0, 900, 1), checkpoint(1, 1_800, 2)];
        assert_eq!(highest_anchored_block(&anchors), 1_800);
        assert_eq!(accepts_reorganisation(&anchors, 1_800), Ok(()));
        assert_eq!(accepts_reorganisation(&anchors, 2_000), Ok(()));
        assert_eq!(
            accepts_reorganisation(&anchors, 1_799),
            Err(CheckpointFault::BelowAnchor)
        );
        assert_eq!(
            accepts_reorganisation(&anchors, 0),
            Err(CheckpointFault::BelowAnchor)
        );
    }

    #[test]
    fn a_node_holding_no_anchor_accepts_any_reorganisation() {
        assert_eq!(highest_anchored_block(&[]), 0);
        assert_eq!(accepts_reorganisation(&[], 0), Ok(()));
    }

    #[test]
    fn every_fault_names_a_fix() {
        for fault in [
            CheckpointFault::HashMismatch,
            CheckpointFault::BelowAnchor,
            CheckpointFault::OutOfOrder,
            CheckpointFault::BlockOutOfOrder,
        ] {
            assert!(fault.plain_language().contains("Fix:"));
        }
    }

    #[test]
    fn a_checkpoint_round_trips_through_the_encoding() {
        let one = checkpoint(3, 2_700, 4);
        let bytes = one.encode();
        assert_eq!(Checkpoint::decode(&mut &bytes[..]).expect("decodes"), one);
    }
}
