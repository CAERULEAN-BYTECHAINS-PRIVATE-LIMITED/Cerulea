//! Equivocation detection inside the Authoring Engine.
//!
//! Equivocation means one validator sealing two different headers for one slot
//! at one rank. A slot is one fixed span of time. Each slot draws a committee
//! and names one author at each rank. An author that seals two headers for one
//! slot has signed two futures for one moment. That breaks the single history
//! every participant relies on.
//!
//! # The two paths that reach detection
//!
//! 1. **Import.** Every node keeps the sealed head of each block it imports.
//!    A second head arriving for one slot, one rank and one seal key raises the
//!    proof at once. The node holding both headers builds the evidence without
//!    help.
//! 2. **A propagation report.** A node that saw only one of the two headers
//!    learns of the other through a report from a peer. The report carries the
//!    whole proof, so the receiving node checks the evidence itself and trusts
//!    no peer.
//!
//! Both paths end at the same place. The node holds a checked proof and submits
//! the evidence extrinsic to the framework pallet. The Standing Engine then
//! returns the standing of that validator to zero and records the fault.
//!
//! # What keeps the memory bounded
//!
//! The tracker remembers one sealed head for each slot, rank and seal key it
//! has seen. Two limits hold the memory inside a ceiling. A slot window drops
//! heads that fall far below the highest slot seen. A capacity limit drops the
//! lowest slots once the count reaches the ceiling. The report path carries a
//! hard byte bound checked before any decoding.

use std::collections::BTreeMap;

use codec::{Decode, Encode};
use sp_runtime::traits::Header as HeaderT;

use super::seal::{extract_seal, pre_seal_hash, BlockSeal};
use crate::engines::EngineError;

pub use cerulea_primitives::equivocation::{
    verify_equivocation, EquivocationFacts, EquivocationFault, EquivocationProof, SealClaimBytes,
    SealedHead, SEAL_CLAIM_BYTES,
};

/// The name the propagation layer carries for an equivocation report.
pub const EQUIVOCATION_PROTOCOL_NAME: &str = "/cerulea/equivocation/1";

/// The highest number of bytes one equivocation report carries.
///
/// The proof holds two sealed heads. Each head holds a pre-seal hash of thirty
/// two bytes, a seal claim of two hundred and nine bytes and a signature of
/// sixty four bytes. The bound sits above that figure with room for the
/// envelope. A larger message is refused before any decoding runs.
pub const MAX_EQUIVOCATION_REPORT_BYTES: usize =
    cerulea_primitives::bounds::MessagePath::EquivocationReport.max_bytes();

/// The number of slots the tracker remembers behind the highest slot seen.
///
/// One epoch of slots keeps a proof reachable for the whole epoch the
/// equivocation happened in. Detection and penalty therefore complete inside
/// one epoch.
pub const DEFAULT_TRACKED_SLOTS: u64 = 600;

/// The highest number of sealed heads the tracker remembers.
pub const DEFAULT_TRACKED_HEADS: usize = 4_096;

/// The key one sealed head sits under.
///
/// The key joins the slot, the rank and the author seal key. Two heads under
/// one key name one author at one rank of one slot.
pub type HeadKey = (u64, u8, [u8; 32]);

/// Read one sealed head out of a block header and its seal.
///
/// The call reports a problem where the encoded claim runs to another length.
/// Every claim the Authoring Engine builds encodes to the stated length.
pub fn sealed_head<H: HeaderT>(header: &H, seal: &BlockSeal) -> Result<SealedHead, EngineError> {
    let hash = pre_seal_hash(header);
    let reference = hash.as_ref();
    if reference.len() != 32 {
        return Err(EngineError::SealClaimUnexpectedLength {
            bytes: reference.len(),
            expected: 32,
        });
    }
    let mut pre_seal = [0u8; 32];
    pre_seal.copy_from_slice(reference);

    let encoded = seal.claim.encode();
    if encoded.len() != SEAL_CLAIM_BYTES {
        return Err(EngineError::SealClaimUnexpectedLength {
            bytes: encoded.len(),
            expected: SEAL_CLAIM_BYTES,
        });
    }
    let mut claim = [0u8; SEAL_CLAIM_BYTES];
    claim.copy_from_slice(&encoded);

    Ok(SealedHead::new(pre_seal, claim, seal.signature.0))
}

/// Read one sealed head out of a block header alone.
///
/// The call reports a problem where the header carries no Cerulea seal.
pub fn sealed_head_of<H: HeaderT>(header: &H) -> Result<SealedHead, EngineError> {
    let seal = extract_seal(header).ok_or(EngineError::SealAbsent)?;
    sealed_head(header, &seal)
}

/// The key one sealed head sits under.
pub fn head_key(head: &SealedHead) -> HeadKey {
    (head.slot(), head.rank(), head.seal_key())
}

/// The memory of sealed heads one node keeps.
///
/// The tracker answers one question. Has this author already sealed another
/// header for this slot at this rank. An answer of yes carries the earlier head
/// back, so the node pairs the two into evidence.
#[derive(Clone, Debug)]
pub struct EquivocationTracker {
    heads: BTreeMap<HeadKey, SealedHead>,
    window_slots: u64,
    capacity: usize,
    highest_slot: u64,
    detections: u64,
}

impl Default for EquivocationTracker {
    fn default() -> Self {
        EquivocationTracker::new()
    }
}

impl EquivocationTracker {
    /// Build a tracker holding the standard window and capacity.
    pub fn new() -> Self {
        EquivocationTracker::with_limits(DEFAULT_TRACKED_SLOTS, DEFAULT_TRACKED_HEADS)
    }

    /// Build a tracker holding the stated window and capacity.
    ///
    /// A capacity of zero rises to one, so the tracker always remembers the
    /// last head.
    pub fn with_limits(window_slots: u64, capacity: usize) -> Self {
        EquivocationTracker {
            heads: BTreeMap::new(),
            window_slots,
            capacity: capacity.max(1),
            highest_slot: 0,
            detections: 0,
        }
    }

    /// The number of heads the tracker remembers.
    pub fn len(&self) -> usize {
        self.heads.len()
    }

    /// Report whether the tracker remembers nothing.
    pub fn is_empty(&self) -> bool {
        self.heads.is_empty()
    }

    /// The capacity this tracker holds.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// The slot window this tracker holds.
    pub fn window_slots(&self) -> u64 {
        self.window_slots
    }

    /// The number of proofs this tracker has raised.
    pub fn detections(&self) -> u64 {
        self.detections
    }

    /// The highest slot the tracker has seen.
    pub fn highest_slot(&self) -> u64 {
        self.highest_slot
    }

    /// Record one sealed head and report any equivocation it completes.
    ///
    /// A head arriving for a slot, a rank and a seal key the tracker already
    /// holds under another pre-seal hash completes a proof. The tracker keeps
    /// the earlier head, so a validator sealing a third header raises the proof
    /// again.
    pub fn observe(&mut self, head: SealedHead) -> Option<EquivocationProof> {
        let key = head_key(&head);
        if key.0 > self.highest_slot {
            self.highest_slot = key.0;
        }
        if let Some(held) = self.heads.get(&key) {
            if held.pre_seal_hash == head.pre_seal_hash {
                return None;
            }
            self.detections = self.detections.saturating_add(1);
            return Some(EquivocationProof::new(*held, head));
        }
        self.prune();
        self.heads.insert(key, head);
        None
    }

    /// Record one block header and report any equivocation it completes.
    pub fn observe_header<H: HeaderT>(
        &mut self,
        header: &H,
        seal: &BlockSeal,
    ) -> Result<Option<EquivocationProof>, EngineError> {
        Ok(self.observe(sealed_head(header, seal)?))
    }

    /// The head the tracker holds under one key.
    pub fn held(&self, key: &HeadKey) -> Option<&SealedHead> {
        self.heads.get(key)
    }

    /// Forget every head.
    pub fn clear(&mut self) {
        self.heads.clear();
        self.highest_slot = 0;
    }

    /// Drop the heads that fall outside the window and the capacity.
    fn prune(&mut self) {
        let floor = self.highest_slot.saturating_sub(self.window_slots);
        if floor > 0 {
            self.heads = self.heads.split_off(&(floor, 0, [0u8; 32]));
        }
        while self.heads.len() >= self.capacity {
            let lowest = match self.heads.keys().next().copied() {
                Some(key) => key,
                None => break,
            };
            self.heads.remove(&lowest);
        }
    }
}

/// One equivocation report travelling between peers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct EquivocationReport {
    /// The evidence the reporter collected.
    pub proof: EquivocationProof,
}

impl EquivocationReport {
    /// Wrap one proof as a report.
    pub const fn new(proof: EquivocationProof) -> Self {
        EquivocationReport { proof }
    }

    /// The bytes this report occupies on the wire.
    pub fn encoded_len(&self) -> usize {
        self.encode().len()
    }
}

/// Read one equivocation report from the propagation layer.
///
/// The byte bound runs before the decoding, so an oversized message costs one
/// length comparison. The call reports a problem naming the fix wherever the
/// message fails a check.
pub fn read_report(bytes: &[u8]) -> Result<EquivocationReport, EngineError> {
    if bytes.len() > MAX_EQUIVOCATION_REPORT_BYTES {
        return Err(EngineError::EquivocationReportTooLarge {
            bytes: bytes.len(),
            limit: MAX_EQUIVOCATION_REPORT_BYTES,
        });
    }
    EquivocationReport::decode(&mut &bytes[..]).map_err(|reason| {
        EngineError::EquivocationReportUnreadable {
            detail: format!("{}", reason),
        }
    })
}

/// Check one equivocation proof and read back what it states.
///
/// The check runs the shared evidence code, so the node and the chain reach the
/// same answer over one proof.
pub fn check_proof(proof: &EquivocationProof) -> Result<EquivocationFacts, EngineError> {
    verify_equivocation(proof).map_err(|fault| EngineError::EquivocationProofRejected {
        detail: fault.plain_language().to_string(),
    })
}

/// Read and check one report arriving from a peer.
pub fn accept_report(bytes: &[u8]) -> Result<(EquivocationReport, EquivocationFacts), EngineError> {
    let report = read_report(bytes)?;
    let facts = check_proof(&report.proof)?;
    Ok((report, facts))
}

/// The proofs one node holds while the evidence extrinsic waits to go out.
///
/// The queue holds one proof for each slot, rank and seal key. A repeat report
/// of a proof already queued changes nothing, so a stream of reports raises no
/// memory.
#[derive(Clone, Debug)]
pub struct ReportQueue {
    entries: BTreeMap<HeadKey, EquivocationProof>,
    capacity: usize,
    refused: u64,
}

impl Default for ReportQueue {
    fn default() -> Self {
        ReportQueue::new()
    }
}

/// The highest number of proofs one node queues for submission.
pub const DEFAULT_REPORT_CAPACITY: usize = 256;

impl ReportQueue {
    /// Build a queue holding the standard capacity.
    pub fn new() -> Self {
        ReportQueue::with_capacity(DEFAULT_REPORT_CAPACITY)
    }

    /// Build a queue holding the stated capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        ReportQueue {
            entries: BTreeMap::new(),
            capacity: capacity.max(1),
            refused: 0,
        }
    }

    /// The number of proofs the queue holds.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Report whether the queue holds nothing.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The number of proofs the queue refused at capacity.
    pub fn refused(&self) -> u64 {
        self.refused
    }

    /// Place one checked proof in the queue.
    ///
    /// The call reports whether the queue took the proof. A proof for a slot,
    /// a rank and a seal key already queued leaves the queue unchanged.
    pub fn place(&mut self, proof: EquivocationProof) -> bool {
        let key = head_key(&proof.first);
        if self.entries.contains_key(&key) {
            return false;
        }
        if self.entries.len() >= self.capacity {
            self.refused = self.refused.saturating_add(1);
            return false;
        }
        self.entries.insert(key, proof);
        true
    }

    /// Take every queued proof out for submission.
    pub fn drain(&mut self) -> Vec<EquivocationProof> {
        let taken: Vec<EquivocationProof> = self.entries.values().copied().collect();
        self.entries.clear();
        taken
    }

    /// The proofs the queue holds, leaving the queue untouched.
    pub fn held(&self) -> Vec<EquivocationProof> {
        self.entries.values().copied().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engines::authoring::draw::{draw, DrawInput};
    use crate::engines::authoring::seal::{seal_and_apply, SealClaim};
    use crate::engines::{Rank, Slot, SlotSeed};
    use crate::CERULEA_KEY_TYPE;
    use sp_core::{ed25519, sr25519, Pair as PairTrait};
    use sp_keystore::testing::MemoryKeystore;
    use sp_keystore::{Keystore, KeystorePtr};
    use sp_runtime::generic::{Digest, Header};
    use sp_runtime::traits::BlakeTwo256;
    use std::sync::Arc;

    type TestHeader = Header<u32, BlakeTwo256>;

    fn keystore() -> KeystorePtr {
        Arc::new(MemoryKeystore::new())
    }

    fn author_key(keystore: &KeystorePtr, seed: &str) -> ed25519::Public {
        keystore
            .ed25519_generate_new(CERULEA_KEY_TYPE, Some(seed))
            .expect("the keystore generates a key")
    }

    fn claim(author: ed25519::Public, slot: u64, rank: u8) -> SealClaim {
        let pair = sr25519::Pair::from_seed(&[3u8; 32]);
        let input = DrawInput::new(Slot(slot), SlotSeed([1u8; 32]));
        SealClaim::new(
            Slot(slot),
            Rank(rank),
            author,
            draw(&pair, &input),
            slot * 1_000,
        )
    }

    /// Seal one header of the stated number and read back the sealed head.
    fn head(
        keystore: &KeystorePtr,
        author: ed25519::Public,
        number: u32,
        slot: u64,
        rank: u8,
    ) -> SealedHead {
        let mut header = TestHeader::new(
            number,
            Default::default(),
            Default::default(),
            Default::default(),
            Digest::default(),
        );
        let seal = seal_and_apply(keystore, &mut header, claim(author, slot, rank))
            .expect("the keystore seals the header");
        sealed_head(&header, &seal).expect("the claim encodes to the stated length")
    }

    #[test]
    fn a_seal_claim_encodes_to_the_stated_length() {
        let keystore = keystore();
        let author = author_key(&keystore, "//Cerulea//author");
        assert_eq!(claim(author, 5, 1).encode().len(), SEAL_CLAIM_BYTES);
    }

    #[test]
    fn a_sealed_head_reads_back_its_slot_rank_and_keys() {
        let keystore = keystore();
        let author = author_key(&keystore, "//Cerulea//author");
        let head = head(&keystore, author, 4, 9, 2);
        assert_eq!(head.slot(), 9);
        assert_eq!(head.rank(), 2);
        assert_eq!(head.seal_key(), author.0);
        assert_eq!(
            head.draw_key(),
            sr25519::Pair::from_seed(&[3u8; 32]).public().0
        );
    }

    #[test]
    fn two_headers_at_one_slot_raise_a_checked_proof() {
        let keystore = keystore();
        let author = author_key(&keystore, "//Cerulea//author");
        let mut tracker = EquivocationTracker::new();
        assert!(tracker.observe(head(&keystore, author, 4, 9, 1)).is_none());
        let proof = tracker
            .observe(head(&keystore, author, 5, 9, 1))
            .expect("the second header at one slot raises the proof");
        let facts = check_proof(&proof).expect("the proof stands");
        assert_eq!(facts.slot, 9);
        assert_eq!(facts.rank, 1);
        assert_eq!(facts.seal_key, author.0);
        assert_eq!(tracker.detections(), 1);
    }

    #[test]
    fn one_header_arriving_twice_raises_nothing() {
        let keystore = keystore();
        let author = author_key(&keystore, "//Cerulea//author");
        let sealed = head(&keystore, author, 4, 9, 1);
        let mut tracker = EquivocationTracker::new();
        assert!(tracker.observe(sealed).is_none());
        assert!(tracker.observe(sealed).is_none());
        assert_eq!(tracker.detections(), 0);
        assert_eq!(tracker.len(), 1);
    }

    #[test]
    fn two_headers_at_two_slots_raise_nothing() {
        let keystore = keystore();
        let author = author_key(&keystore, "//Cerulea//author");
        let mut tracker = EquivocationTracker::new();
        assert!(tracker.observe(head(&keystore, author, 4, 9, 1)).is_none());
        assert!(tracker.observe(head(&keystore, author, 5, 10, 1)).is_none());
        assert_eq!(tracker.detections(), 0);
    }

    #[test]
    fn two_authors_at_one_slot_raise_nothing() {
        let keystore = keystore();
        let one = author_key(&keystore, "//Cerulea//one");
        let two = author_key(&keystore, "//Cerulea//two");
        let mut tracker = EquivocationTracker::new();
        assert!(tracker.observe(head(&keystore, one, 4, 9, 1)).is_none());
        assert!(tracker.observe(head(&keystore, two, 5, 9, 1)).is_none());
        assert_eq!(tracker.detections(), 0);
    }

    #[test]
    fn a_third_header_raises_the_proof_again() {
        let keystore = keystore();
        let author = author_key(&keystore, "//Cerulea//author");
        let mut tracker = EquivocationTracker::new();
        tracker.observe(head(&keystore, author, 4, 9, 1));
        assert!(tracker.observe(head(&keystore, author, 5, 9, 1)).is_some());
        assert!(tracker.observe(head(&keystore, author, 6, 9, 1)).is_some());
        assert_eq!(tracker.detections(), 2);
    }

    #[test]
    fn the_tracker_holds_its_memory_inside_the_capacity() {
        let keystore = keystore();
        let author = author_key(&keystore, "//Cerulea//author");
        let mut tracker = EquivocationTracker::with_limits(1_000_000, 8);
        for slot in 0..64u64 {
            tracker.observe(head(&keystore, author, slot as u32 + 1, slot, 1));
        }
        assert!(tracker.len() <= 8, "the tracker holds inside its capacity");
    }

    #[test]
    fn the_tracker_drops_heads_below_the_slot_window() {
        let keystore = keystore();
        let author = author_key(&keystore, "//Cerulea//author");
        let mut tracker = EquivocationTracker::with_limits(4, 1_000);
        tracker.observe(head(&keystore, author, 1, 1, 1));
        tracker.observe(head(&keystore, author, 2, 100, 1));
        assert_eq!(tracker.len(), 1, "the head far below the window is gone");
        assert!(tracker.held(&(1, 1, author.0)).is_none());
        assert!(tracker.held(&(100, 1, author.0)).is_some());
    }

    #[test]
    fn a_detection_inside_the_window_survives_a_full_epoch_of_slots() {
        let keystore = keystore();
        let author = author_key(&keystore, "//Cerulea//author");
        let mut tracker = EquivocationTracker::new();
        tracker.observe(head(&keystore, author, 1, 10, 1));
        for slot in 11..(10 + DEFAULT_TRACKED_SLOTS) {
            let other = author_key(&keystore, &format!("//Cerulea//filler//{}", slot));
            tracker.observe(head(&keystore, other, slot as u32, slot, 1));
        }
        assert!(
            tracker.observe(head(&keystore, author, 2, 10, 1)).is_some(),
            "the proof stays reachable across the epoch"
        );
    }

    #[test]
    fn a_report_round_trips_through_the_wire() {
        let keystore = keystore();
        let author = author_key(&keystore, "//Cerulea//author");
        let proof = EquivocationProof::new(
            head(&keystore, author, 4, 9, 1),
            head(&keystore, author, 5, 9, 1),
        );
        let report = EquivocationReport::new(proof);
        let bytes = report.encode();
        assert!(bytes.len() <= MAX_EQUIVOCATION_REPORT_BYTES);
        let (read, facts) = accept_report(&bytes).expect("the report checks out");
        assert_eq!(read, report);
        assert_eq!(facts.slot, 9);
    }

    #[test]
    fn an_oversized_report_is_refused_before_decoding() {
        let bytes = vec![0u8; MAX_EQUIVOCATION_REPORT_BYTES + 1];
        let error = read_report(&bytes).expect_err("the oversized report is refused");
        assert!(matches!(
            error,
            EngineError::EquivocationReportTooLarge { .. }
        ));
        assert!(error.to_string().contains("Fix:"));
    }

    #[test]
    fn a_report_carrying_one_header_twice_fails_the_check() {
        let keystore = keystore();
        let author = author_key(&keystore, "//Cerulea//author");
        let sealed = head(&keystore, author, 4, 9, 1);
        let report = EquivocationReport::new(EquivocationProof::new(sealed, sealed));
        let error = accept_report(&report.encode()).expect_err("one header proves nothing");
        assert!(matches!(
            error,
            EngineError::EquivocationProofRejected { .. }
        ));
        assert!(error.to_string().contains("Fix:"));
    }

    #[test]
    fn a_short_message_fails_to_read() {
        let error = read_report(&[1u8, 2, 3]).expect_err("three bytes carry no report");
        assert!(matches!(
            error,
            EngineError::EquivocationReportUnreadable { .. }
        ));
    }

    #[test]
    fn the_queue_holds_one_proof_for_each_author_and_slot() {
        let keystore = keystore();
        let author = author_key(&keystore, "//Cerulea//author");
        let proof = EquivocationProof::new(
            head(&keystore, author, 4, 9, 1),
            head(&keystore, author, 5, 9, 1),
        );
        let mut queue = ReportQueue::new();
        assert!(queue.place(proof));
        assert!(!queue.place(proof), "the repeat leaves the queue unchanged");
        assert_eq!(queue.len(), 1);
        assert_eq!(queue.held().len(), 1);
        assert_eq!(queue.drain().len(), 1);
        assert!(queue.is_empty());
    }

    #[test]
    fn the_queue_refuses_a_proof_at_capacity() {
        let keystore = keystore();
        let mut queue = ReportQueue::with_capacity(1);
        let one = author_key(&keystore, "//Cerulea//one");
        let two = author_key(&keystore, "//Cerulea//two");
        assert!(queue.place(EquivocationProof::new(
            head(&keystore, one, 4, 9, 1),
            head(&keystore, one, 5, 9, 1),
        )));
        assert!(!queue.place(EquivocationProof::new(
            head(&keystore, two, 4, 9, 1),
            head(&keystore, two, 5, 9, 1),
        )));
        assert_eq!(queue.refused(), 1);
    }

    #[test]
    fn a_header_without_a_seal_carries_no_head() {
        let header = TestHeader::new(
            1,
            Default::default(),
            Default::default(),
            Default::default(),
            Digest::default(),
        );
        assert_eq!(sealed_head_of(&header), Err(EngineError::SealAbsent));
    }

    #[test]
    fn the_report_bound_sits_above_one_whole_proof() {
        let keystore = keystore();
        let author = author_key(&keystore, "//Cerulea//author");
        let proof = EquivocationProof::new(
            head(&keystore, author, 4, 9, 1),
            head(&keystore, author, 5, 9, 1),
        );
        assert!(EquivocationReport::new(proof).encoded_len() < MAX_EQUIVOCATION_REPORT_BYTES);
    }
}
