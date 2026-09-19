//! Checkpoint anchoring against the long-range attack.
//!
//! A long-range attack rewrites history from a point far below the head. The
//! attacker builds a second chain from an old block and offers it to a node
//! that holds little history of its own. A node joining the network from
//! genesis is the target that matters. Block rules alone leave that node with
//! two chains and no way to tell which one the network lived through.
//!
//! A checkpoint answers that question. A checkpoint is one committed block the
//! chain names as settled. The chain records every checkpoint. Each node keeps
//! its own copy in a local store on disk. Two rules then follow.
//!
//! 1. **No reorganisation below the anchor.** A node refuses any chain whose
//!    common ancestor with the chain it holds sits below the highest anchored
//!    checkpoint.
//! 2. **Sync runs through the anchors.** A node syncing from genesis accepts a
//!    chain only where the block at each anchored height carries the hash that
//!    anchor names.
//!
//! The same anchored set reaches an external network. The Bitcoin anchoring
//! path of the bridge module writes the checkpoint commitment into a data
//! carrying output, so the Cerulea history gains a witness outside Cerulea.
//!
//! # The local store
//!
//! The store holds the anchored set in index order. The file sits beside the
//! chain database under the node base path. A node reads the file at start and
//! writes it whenever a fresh checkpoint lands. A missing file starts an empty
//! store, which a node then fills from the chain.

use std::fs;
use std::path::{Path, PathBuf};

use codec::{Decode, Encode};

use crate::engines::EngineError;

pub use cerulea_primitives::checkpoint::{
    accepts_block, accepts_reorganisation, checkpoint_due, checkpoint_index, follows,
    highest_anchored_block, Checkpoint, CheckpointFault, CHECKPOINT_DOMAIN,
    DEFAULT_CHECKPOINT_INTERVAL,
};

/// The name of the checkpoint file inside the node base path.
pub const CHECKPOINT_FILE_NAME: &str = "cerulea-checkpoints.scale";

/// The highest number of checkpoints one store holds in memory.
///
/// The store keeps the newest entries and drops the oldest once the count
/// reaches this ceiling. The highest anchored checkpoint settles every one
/// below it, so the dropped entries carry no further weight. Memory therefore
/// stays inside a stated ceiling however long the chain runs.
pub const MAX_HELD_CHECKPOINTS: usize = 4_096;

/// The version byte the checkpoint file carries.
pub const CHECKPOINT_FILE_VERSION: u8 = 1;

/// The anchored checkpoint set one node holds.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CheckpointStore {
    anchors: Vec<Checkpoint>,
    path: Option<PathBuf>,
}

impl CheckpointStore {
    /// Start an empty store with no file behind it.
    pub fn new() -> Self {
        CheckpointStore {
            anchors: Vec::new(),
            path: None,
        }
    }

    /// Start a store from a list of anchors.
    ///
    /// The call reports a problem where the list carries a gap, a repeat or a
    /// block number out of order.
    pub fn from_anchors(anchors: Vec<Checkpoint>) -> Result<Self, EngineError> {
        let mut store = CheckpointStore::new();
        for anchor in anchors {
            store.anchor(anchor)?;
        }
        Ok(store)
    }

    /// Read the store held in the node base path.
    ///
    /// A missing file starts an empty store. A file that fails to read reports
    /// a problem naming the fix.
    pub fn open(base_path: &Path) -> Result<Self, EngineError> {
        let path = base_path.join(CHECKPOINT_FILE_NAME);
        let mut store = CheckpointStore {
            anchors: Vec::new(),
            path: Some(path.clone()),
        };
        if !path.exists() {
            return Ok(store);
        }
        let raw = fs::read(&path).map_err(|reason| EngineError::CheckpointStoreUnreadable {
            detail: format!("{}", reason),
        })?;
        store.anchors = decode_file(&raw)?;
        Ok(store)
    }

    /// The path of the file behind this store.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// The anchors this store holds, in index order.
    pub fn anchors(&self) -> &[Checkpoint] {
        &self.anchors
    }

    /// The number of anchors this store holds.
    pub fn len(&self) -> usize {
        self.anchors.len()
    }

    /// Report whether this store holds no anchor.
    pub fn is_empty(&self) -> bool {
        self.anchors.is_empty()
    }

    /// The highest anchored checkpoint.
    pub fn latest(&self) -> Option<&Checkpoint> {
        self.anchors.last()
    }

    /// The highest block this store settles.
    pub fn highest_block(&self) -> u32 {
        highest_anchored_block(&self.anchors)
    }

    /// Add one checkpoint to the anchored set.
    ///
    /// The index rises by one and the block number rises, so the set carries no
    /// gap and no repeat. Adding the checkpoint the store already holds at that
    /// index leaves the store unchanged.
    pub fn anchor(&mut self, checkpoint: Checkpoint) -> Result<bool, EngineError> {
        if let Some(held) = self
            .anchors
            .iter()
            .find(|held| held.index == checkpoint.index)
        {
            if held == &checkpoint {
                return Ok(false);
            }
            return Err(EngineError::CheckpointRejected {
                detail: CheckpointFault::HashMismatch.plain_language().to_string(),
            });
        }
        follows(self.anchors.last(), &checkpoint).map_err(|fault| {
            EngineError::CheckpointRejected {
                detail: fault.plain_language().to_string(),
            }
        })?;
        self.anchors.push(checkpoint);
        while self.anchors.len() > MAX_HELD_CHECKPOINTS {
            self.anchors.remove(0);
        }
        Ok(true)
    }

    /// Check one block against the anchored set.
    ///
    /// A block at an anchored height carries the hash that anchor names. A
    /// block at any other height passes.
    pub fn accepts_block(&self, block: u32, hash: &[u8; 32]) -> Result<(), EngineError> {
        accepts_block(&self.anchors, block, hash).map_err(|fault| EngineError::CheckpointRejected {
            detail: fault.plain_language().to_string(),
        })
    }

    /// Check whether a chain may replace the chain this node holds.
    ///
    /// The common ancestor is the highest block the two chains share. A common
    /// ancestor below the highest anchored checkpoint means the offered chain
    /// would rewrite settled history. The node refuses.
    pub fn accepts_reorganisation(&self, common_ancestor: u32) -> Result<(), EngineError> {
        accepts_reorganisation(&self.anchors, common_ancestor).map_err(|fault| {
            EngineError::CheckpointRejected {
                detail: fault.plain_language().to_string(),
            }
        })
    }

    /// Write the anchored set to the file behind this store.
    ///
    /// A store with no file behind it writes nothing and reports success.
    pub fn save(&self) -> Result<(), EngineError> {
        let path = match &self.path {
            Some(path) => path,
            None => return Ok(()),
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|reason| {
                EngineError::CheckpointStoreUnreadable {
                    detail: format!("{}", reason),
                }
            })?;
        }
        fs::write(path, encode_file(&self.anchors)).map_err(|reason| {
            EngineError::CheckpointStoreUnreadable {
                detail: format!("{}", reason),
            }
        })
    }

    /// Add one checkpoint and write the file behind this store.
    pub fn anchor_and_save(&mut self, checkpoint: Checkpoint) -> Result<bool, EngineError> {
        let added = self.anchor(checkpoint)?;
        if added {
            self.save()?;
        }
        Ok(added)
    }
}

/// The bytes one checkpoint file carries.
pub fn encode_file(anchors: &[Checkpoint]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(4 + anchors.len() * 72);
    bytes.push(CHECKPOINT_FILE_VERSION);
    anchors.to_vec().encode_to(&mut bytes);
    bytes
}

/// Read the anchors one checkpoint file carries.
pub fn decode_file(raw: &[u8]) -> Result<Vec<Checkpoint>, EngineError> {
    let version = raw
        .first()
        .copied()
        .ok_or_else(|| EngineError::CheckpointStoreUnreadable {
            detail: "the file holds no bytes".to_string(),
        })?;
    if version != CHECKPOINT_FILE_VERSION {
        return Err(EngineError::CheckpointStoreUnreadable {
            detail: format!(
                "the file carries version {} and this release reads version {}",
                version, CHECKPOINT_FILE_VERSION
            ),
        });
    }
    Vec::<Checkpoint>::decode(&mut &raw[1..]).map_err(|reason| {
        EngineError::CheckpointStoreUnreadable {
            detail: format!("{}", reason),
        }
    })
}

/// The gate a node syncing from genesis runs every block through.
///
/// The gate holds the anchored set the node was given. A block arriving at an
/// anchored height carries the hash that anchor names. Every other block
/// passes. The sync completes only where every anchor was met.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SyncGate {
    anchors: Vec<Checkpoint>,
    met: Vec<u32>,
    highest_seen: u32,
}

impl SyncGate {
    /// Build a gate over the anchored set a node was given.
    pub fn with_anchors(anchors: Vec<Checkpoint>) -> Self {
        SyncGate {
            anchors,
            met: Vec::new(),
            highest_seen: 0,
        }
    }

    /// The anchors this gate holds.
    pub fn anchors(&self) -> &[Checkpoint] {
        &self.anchors
    }

    /// The number of anchors this gate has met.
    pub fn met(&self) -> usize {
        self.met.len()
    }

    /// The highest block this gate has seen.
    pub fn highest_seen(&self) -> u32 {
        self.highest_seen
    }

    /// Offer one block to the gate.
    ///
    /// The call reports a problem where the block sits at an anchored height
    /// and carries another hash. The node then drops that chain and syncs from
    /// another peer.
    pub fn offer(&mut self, block: u32, hash: &[u8; 32]) -> Result<(), EngineError> {
        accepts_block(&self.anchors, block, hash).map_err(|fault| {
            EngineError::CheckpointRejected {
                detail: fault.plain_language().to_string(),
            }
        })?;
        if let Some(anchor) = self.anchors.iter().find(|anchor| anchor.block == block) {
            if !self.met.contains(&anchor.index) {
                self.met.push(anchor.index);
            }
        }
        if block > self.highest_seen {
            self.highest_seen = block;
        }
        Ok(())
    }

    /// Report whether the chain offered so far met every anchor.
    ///
    /// A chain that reached the highest anchored block and met every anchor on
    /// the way is the chain the network lived through.
    pub fn complete(&self) -> bool {
        self.met.len() == self.anchors.len() && self.highest_seen >= self.highest_block()
    }

    /// The highest block this gate anchors.
    pub fn highest_block(&self) -> u32 {
        highest_anchored_block(&self.anchors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checkpoint(index: u32, block: u32, byte: u8) -> Checkpoint {
        Checkpoint::new(index, block, [byte; 32], [byte.wrapping_add(1); 32])
    }

    fn three_anchors() -> Vec<Checkpoint> {
        vec![
            checkpoint(0, 900, 1),
            checkpoint(1, 1_800, 2),
            checkpoint(2, 2_700, 3),
        ]
    }

    #[test]
    fn a_store_holds_the_anchors_in_index_order() {
        let store = CheckpointStore::from_anchors(three_anchors()).expect("the set is in order");
        assert_eq!(store.len(), 3);
        assert_eq!(store.latest(), Some(&checkpoint(2, 2_700, 3)));
        assert_eq!(store.highest_block(), 2_700);
        assert!(!store.is_empty());
    }

    #[test]
    fn a_store_refuses_a_gap_in_the_anchored_set() {
        let error =
            CheckpointStore::from_anchors(vec![checkpoint(0, 900, 1), checkpoint(2, 1_800, 2)])
                .expect_err("the set carries a gap");
        assert!(matches!(error, EngineError::CheckpointRejected { .. }));
        assert!(error.to_string().contains("Fix:"));
    }

    #[test]
    fn a_store_refuses_a_second_checkpoint_at_one_index() {
        let mut store =
            CheckpointStore::from_anchors(three_anchors()).expect("the set is in order");
        assert!(matches!(
            store.anchor(checkpoint(1, 1_800, 9)),
            Err(EngineError::CheckpointRejected { .. })
        ));
    }

    #[test]
    fn anchoring_the_same_checkpoint_twice_leaves_the_store_unchanged() {
        let mut store =
            CheckpointStore::from_anchors(three_anchors()).expect("the set is in order");
        assert_eq!(store.anchor(checkpoint(1, 1_800, 2)), Ok(false));
        assert_eq!(store.len(), 3);
    }

    #[test]
    fn a_block_at_an_anchored_height_carries_the_anchored_hash() {
        let store = CheckpointStore::from_anchors(three_anchors()).expect("the set is in order");
        assert_eq!(store.accepts_block(1_800, &[2u8; 32]), Ok(()));
        assert!(store.accepts_block(1_800, &[9u8; 32]).is_err());
        assert_eq!(store.accepts_block(1_801, &[9u8; 32]), Ok(()));
    }

    /// The node refuses to reorganise below the latest anchored checkpoint.
    #[test]
    fn a_reorganisation_below_the_latest_anchor_is_refused() {
        let store = CheckpointStore::from_anchors(three_anchors()).expect("the set is in order");
        assert_eq!(store.accepts_reorganisation(2_700), Ok(()));
        assert_eq!(store.accepts_reorganisation(3_000), Ok(()));
        let error = store
            .accepts_reorganisation(2_699)
            .expect_err("the chain would rewrite settled history");
        assert!(error.to_string().contains("Fix:"));
        assert!(store.accepts_reorganisation(0).is_err());
    }

    #[test]
    fn a_store_round_trips_through_its_file() {
        let directory = tempfile::tempdir().expect("the test makes a directory");
        let mut store =
            CheckpointStore::open(directory.path()).expect("an absent file starts empty");
        assert!(store.is_empty());
        for anchor in three_anchors() {
            assert_eq!(store.anchor_and_save(anchor), Ok(true));
        }
        let read = CheckpointStore::open(directory.path()).expect("the file reads back");
        assert_eq!(read.anchors(), store.anchors());
        assert_eq!(read.highest_block(), 2_700);
        assert_eq!(
            read.path().map(|path| path.file_name().unwrap().to_owned()),
            Some(CHECKPOINT_FILE_NAME.into())
        );
    }

    #[test]
    fn a_file_of_another_version_names_the_fix() {
        let error = decode_file(&[9u8, 0, 0, 0]).expect_err("the version differs");
        assert!(matches!(
            error,
            EngineError::CheckpointStoreUnreadable { .. }
        ));
    }

    #[test]
    fn an_empty_file_fails_to_read() {
        assert!(decode_file(&[]).is_err());
    }

    /// A fresh node syncing from genesis accepts only a chain through the
    /// anchored checkpoints it was given.
    #[test]
    fn a_fresh_node_accepts_the_chain_through_its_anchors() {
        let mut gate = SyncGate::with_anchors(three_anchors());
        for block in 1..=2_700u32 {
            let hash = match block {
                900 => [1u8; 32],
                1_800 => [2u8; 32],
                2_700 => [3u8; 32],
                _ => [0u8; 32],
            };
            gate.offer(block, &hash).expect("the honest chain passes");
        }
        assert_eq!(gate.met(), 3);
        assert!(gate.complete());
    }

    #[test]
    fn a_fresh_node_refuses_a_chain_that_misses_an_anchor() {
        let mut gate = SyncGate::with_anchors(three_anchors());
        gate.offer(900, &[1u8; 32])
            .expect("the first anchor passes");
        let error = gate
            .offer(1_800, &[9u8; 32])
            .expect_err("the second anchor names another block");
        assert!(matches!(error, EngineError::CheckpointRejected { .. }));
        assert!(error.to_string().contains("Fix:"));
        assert!(!gate.complete());
    }

    #[test]
    fn a_fresh_node_refuses_a_chain_that_stops_below_the_highest_anchor() {
        let mut gate = SyncGate::with_anchors(three_anchors());
        gate.offer(900, &[1u8; 32])
            .expect("the first anchor passes");
        gate.offer(1_800, &[2u8; 32])
            .expect("the second anchor passes");
        assert_eq!(gate.met(), 2);
        assert!(!gate.complete(), "the chain missed the highest anchor");
        assert_eq!(gate.highest_seen(), 1_800);
        assert_eq!(gate.highest_block(), 2_700);
    }

    #[test]
    fn a_node_given_no_anchors_completes_at_once() {
        let gate = SyncGate::with_anchors(Vec::new());
        assert!(gate.complete());
        assert!(gate.anchors().is_empty());
    }

    #[test]
    fn a_checkpoint_falls_due_on_the_stated_interval() {
        assert!(checkpoint_due(
            DEFAULT_CHECKPOINT_INTERVAL,
            DEFAULT_CHECKPOINT_INTERVAL
        ));
        assert_eq!(
            checkpoint_index(DEFAULT_CHECKPOINT_INTERVAL * 3, DEFAULT_CHECKPOINT_INTERVAL),
            3
        );
    }

    #[test]
    fn the_store_holds_its_memory_inside_the_ceiling() {
        let mut store = CheckpointStore::new();
        for index in 0..(MAX_HELD_CHECKPOINTS as u32 + 16) {
            store
                .anchor(checkpoint(index, (index + 1) * 900, (index % 251) as u8))
                .expect("each checkpoint follows the one before it");
        }
        assert_eq!(store.len(), MAX_HELD_CHECKPOINTS);
    }
}
