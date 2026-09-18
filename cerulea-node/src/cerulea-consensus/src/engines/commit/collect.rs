//! Signature collection during the slot.
//!
//! Agreement completes in a single round. The author of the slot gathers one
//! signature from each validator that accepts the block. Each signature covers
//! the slot number and the block hash under a Cerulea separator, so a signature
//! carries no meaning for another block or another slot.
//!
//! Message volume grows in proportion to the validator count. Every validator
//! sends one message to the author. The author sends one aggregate to the
//! network. That shape is what supports sub-second commitment across two
//! hundred validators.
//!
//! The collector holds the validator set in a settled order. That order is the
//! order of the signer bitmap the block carries.

use std::collections::BTreeMap;

use codec::{Decode, Encode};
use sp_core::ed25519;
use sp_keystore::KeystorePtr;

use crate::engines::{EngineError, Slot, TrustScore};
use crate::CERULEA_KEY_TYPE;

/// The separator mixed into every commit vote payload.
pub const VOTE_DOMAIN: &[u8] = b"cerulea.commit.vote.v1";

/// One vote from one validator over one block.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct CommitVote {
    /// The slot the block serves.
    pub slot: Slot,
    /// The hash of the block the validator accepts.
    pub block_hash: [u8; 32],
    /// The validator that signed.
    pub validator: ed25519::Public,
    /// The signature over the vote payload.
    pub signature: ed25519::Signature,
}

/// The bytes a validator signs when it accepts a block.
pub fn vote_payload(slot: Slot, block_hash: &[u8; 32]) -> Vec<u8> {
    let mut payload = Vec::with_capacity(VOTE_DOMAIN.len() + 40);
    payload.extend_from_slice(VOTE_DOMAIN);
    payload.extend_from_slice(&slot.0.to_be_bytes());
    payload.extend_from_slice(block_hash);
    payload
}

/// Cast one vote with the validator key held in the node keystore.
pub fn cast_vote(
    keystore: &KeystorePtr,
    validator: &ed25519::Public,
    slot: Slot,
    block_hash: [u8; 32],
) -> Result<CommitVote, EngineError> {
    let payload = vote_payload(slot, &block_hash);
    let signature = keystore
        .ed25519_sign(CERULEA_KEY_TYPE, validator, &payload)
        .map_err(|reason| EngineError::KeystoreRefused {
            detail: format!("{:?}", reason),
        })?
        .ok_or(EngineError::SealKeyAbsent)?;
    Ok(CommitVote {
        slot,
        block_hash,
        validator: *validator,
        signature,
    })
}

/// Check one vote signature.
pub fn check_vote(vote: &CommitVote) -> Result<(), EngineError> {
    let payload = vote_payload(vote.slot, &vote.block_hash);
    if sp_io::crypto::ed25519_verify(&vote.signature, &payload, &vote.validator) {
        Ok(())
    } else {
        Err(EngineError::VoteSignatureRejected)
    }
}

/// The result of offering one vote to the collector.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VoteAdmission {
    /// The collector took the vote.
    Taken,
    /// The collector already holds a vote from that validator.
    AlreadyHeld,
}

/// The signature collector the author runs during the slot.
///
/// The collector holds the validator set in one settled order. Each accepted
/// vote lands at the position of its validator, so the signer bitmap reads the
/// same on every node.
#[derive(Clone, Debug)]
pub struct SignatureCollector {
    slot: Slot,
    block_hash: [u8; 32],
    members: Vec<ed25519::Public>,
    weights: Vec<TrustScore>,
    received: BTreeMap<usize, CommitVote>,
    opens_ms: u64,
    window_ms: u64,
}

impl SignatureCollector {
    /// Open a collector for one block.
    ///
    /// The member list settles the bitmap order. The list sorts on the
    /// validator key, so every node builds the same order.
    pub fn open(
        slot: Slot,
        block_hash: [u8; 32],
        set: &[(ed25519::Public, TrustScore)],
        opens_ms: u64,
        window_ms: u64,
    ) -> Result<Self, EngineError> {
        if set.is_empty() {
            return Err(EngineError::EmptyValidatorSet);
        }
        let mut ordered = set.to_vec();
        ordered.sort_by(|left, right| left.0 .0.cmp(&right.0 .0));
        ordered.dedup_by(|left, right| left.0 == right.0);
        let members = ordered.iter().map(|entry| entry.0).collect();
        let weights = ordered.iter().map(|entry| entry.1).collect();
        Ok(SignatureCollector {
            slot,
            block_hash,
            members,
            weights,
            received: BTreeMap::new(),
            opens_ms,
            window_ms: window_ms.max(1),
        })
    }

    /// The slot this collector serves.
    pub fn slot(&self) -> Slot {
        self.slot
    }

    /// The block this collector serves.
    pub fn block_hash(&self) -> [u8; 32] {
        self.block_hash
    }

    /// The validator set in bitmap order.
    pub fn members(&self) -> &[ed25519::Public] {
        &self.members
    }

    /// The position one validator holds in bitmap order.
    pub fn position_of(&self, validator: &ed25519::Public) -> Option<usize> {
        self.members
            .binary_search_by(|held| held.0.cmp(&validator.0))
            .ok()
    }

    /// The moment the collection window closes, in milliseconds.
    pub fn closes_ms(&self) -> u64 {
        self.opens_ms.saturating_add(self.window_ms)
    }

    /// Report whether the collection window still stands open.
    pub fn open_at(&self, moment_ms: u64) -> bool {
        moment_ms >= self.opens_ms && moment_ms < self.closes_ms()
    }

    /// Offer one vote to the collector.
    ///
    /// The vote passes four checks. The window stands open. The vote names this
    /// slot and this block. The validator sits in the set. The signature
    /// verifies.
    pub fn accept(
        &mut self,
        vote: CommitVote,
        moment_ms: u64,
    ) -> Result<VoteAdmission, EngineError> {
        if !self.open_at(moment_ms) {
            return Err(EngineError::CollectionWindowClosed {
                slot: self.slot.0,
                moment_ms,
            });
        }
        if vote.slot != self.slot {
            return Err(EngineError::VoteSlotMismatch {
                claimed: vote.slot.0,
                expected: self.slot.0,
            });
        }
        if vote.block_hash != self.block_hash {
            return Err(EngineError::VoteBlockMismatch { slot: self.slot.0 });
        }
        let position = self
            .position_of(&vote.validator)
            .ok_or(EngineError::VoterOutsideSet)?;
        if self.received.contains_key(&position) {
            return Ok(VoteAdmission::AlreadyHeld);
        }
        check_vote(&vote)?;
        self.received.insert(position, vote);
        Ok(VoteAdmission::Taken)
    }

    /// The number of validators that signed.
    pub fn signer_count(&self) -> usize {
        self.received.len()
    }

    /// The number of validators in the set.
    pub fn member_count(&self) -> usize {
        self.members.len()
    }

    /// The Trust Score weight the signers carry.
    pub fn signed_weight(&self) -> u128 {
        self.received
            .keys()
            .map(|position| self.weights[*position].weight())
            .sum()
    }

    /// The Trust Score weight the whole set carries.
    pub fn total_weight(&self) -> u128 {
        self.weights.iter().map(|weight| weight.weight()).sum()
    }

    /// The signer positions, in bitmap order.
    pub fn signer_positions(&self) -> Vec<usize> {
        self.received.keys().copied().collect()
    }

    /// The votes the collector holds, in bitmap order.
    pub fn votes(&self) -> Vec<&CommitVote> {
        self.received.values().collect()
    }

    /// The weight of one validator in the set.
    pub fn weight_at(&self, position: usize) -> Option<TrustScore> {
        self.weights.get(position).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sp_core::Pair as PairTrait;
    use sp_keystore::testing::MemoryKeystore;
    use sp_keystore::Keystore;
    use std::sync::Arc;

    const SLOT: Slot = Slot(64);
    const BLOCK: [u8; 32] = [11u8; 32];
    const OPENS: u64 = 5_000;
    const WINDOW: u64 = 800;

    struct Validators {
        keystores: Vec<KeystorePtr>,
        keys: Vec<ed25519::Public>,
    }

    fn build_validators(count: usize) -> Validators {
        let mut keystores = Vec::new();
        let mut keys = Vec::new();
        for index in 0..count {
            let memory = MemoryKeystore::new();
            let key = memory
                .ed25519_generate_new(
                    CERULEA_KEY_TYPE,
                    Some(&format!("//Cerulea//vote//{}", index)),
                )
                .expect("the keystore generates a key");
            keystores.push(Arc::new(memory) as KeystorePtr);
            keys.push(key);
        }
        Validators { keystores, keys }
    }

    fn set(validators: &Validators, percent: u8) -> Vec<(ed25519::Public, TrustScore)> {
        validators
            .keys
            .iter()
            .map(|key| (*key, TrustScore::from_percent(percent)))
            .collect()
    }

    fn collector(validators: &Validators) -> SignatureCollector {
        SignatureCollector::open(SLOT, BLOCK, &set(validators, 50), OPENS, WINDOW)
            .expect("the collector opens")
    }

    #[test]
    fn the_author_gathers_one_signature_from_each_validator() {
        let validators = build_validators(7);
        let mut collector = collector(&validators);
        for index in 0..7 {
            let vote = cast_vote(
                &validators.keystores[index],
                &validators.keys[index],
                SLOT,
                BLOCK,
            )
            .expect("the validator signs");
            assert_eq!(
                collector.accept(vote, OPENS + 10).expect("the vote lands"),
                VoteAdmission::Taken
            );
        }
        assert_eq!(collector.signer_count(), 7);
        assert_eq!(collector.member_count(), 7);
        assert_eq!(collector.signed_weight(), collector.total_weight());
        assert_eq!(collector.votes().len(), 7);
    }

    #[test]
    fn a_second_vote_from_one_validator_leaves_the_tally_unchanged() {
        let validators = build_validators(4);
        let mut collector = collector(&validators);
        let vote = cast_vote(&validators.keystores[0], &validators.keys[0], SLOT, BLOCK)
            .expect("the validator signs");
        assert_eq!(
            collector
                .accept(vote.clone(), OPENS + 1)
                .expect("the vote lands"),
            VoteAdmission::Taken
        );
        assert_eq!(
            collector.accept(vote, OPENS + 2).expect("the vote lands"),
            VoteAdmission::AlreadyHeld
        );
        assert_eq!(collector.signer_count(), 1);
    }

    #[test]
    fn a_vote_with_a_broken_signature_is_rejected() {
        let validators = build_validators(4);
        let mut collector = collector(&validators);
        let mut vote = cast_vote(&validators.keystores[0], &validators.keys[0], SLOT, BLOCK)
            .expect("the validator signs");
        vote.signature = ed25519::Signature::from_raw([2u8; 64]);
        let error = collector
            .accept(vote, OPENS + 1)
            .expect_err("a broken signature fails");
        assert_eq!(error, EngineError::VoteSignatureRejected);
        assert!(error.to_string().contains("Fix:"));
        assert_eq!(collector.signer_count(), 0);
    }

    #[test]
    fn a_vote_from_a_validator_outside_the_set_is_rejected() {
        let validators = build_validators(5);
        // The collector holds the first four. The fifth stays outside the set.
        let held = Validators {
            keystores: validators.keystores[..4].to_vec(),
            keys: validators.keys[..4].to_vec(),
        };
        let mut collector = collector(&held);
        let vote = cast_vote(&validators.keystores[4], &validators.keys[4], SLOT, BLOCK)
            .expect("the validator signs");
        let error = collector
            .accept(vote, OPENS + 1)
            .expect_err("an outsider fails");
        assert_eq!(error, EngineError::VoterOutsideSet);
    }

    #[test]
    fn a_vote_naming_another_block_is_rejected() {
        let validators = build_validators(4);
        let mut collector = collector(&validators);
        let vote = cast_vote(
            &validators.keystores[0],
            &validators.keys[0],
            SLOT,
            [99u8; 32],
        )
        .expect("the validator signs");
        let error = collector
            .accept(vote, OPENS + 1)
            .expect_err("another block fails");
        assert!(matches!(error, EngineError::VoteBlockMismatch { .. }));
    }

    #[test]
    fn a_vote_naming_another_slot_is_rejected() {
        let validators = build_validators(4);
        let mut collector = collector(&validators);
        let vote = cast_vote(
            &validators.keystores[0],
            &validators.keys[0],
            Slot(SLOT.0 + 1),
            BLOCK,
        )
        .expect("the validator signs");
        let error = collector
            .accept(vote, OPENS + 1)
            .expect_err("another slot fails");
        assert!(matches!(error, EngineError::VoteSlotMismatch { .. }));
    }

    #[test]
    fn a_vote_after_the_window_closes_is_rejected() {
        let validators = build_validators(4);
        let mut collector = collector(&validators);
        assert!(collector.open_at(OPENS));
        assert!(!collector.open_at(collector.closes_ms()));
        let vote = cast_vote(&validators.keystores[0], &validators.keys[0], SLOT, BLOCK)
            .expect("the validator signs");
        let error = collector
            .accept(vote, collector.closes_ms())
            .expect_err("a late vote fails");
        assert!(matches!(error, EngineError::CollectionWindowClosed { .. }));
        assert!(error.to_string().contains("Fix:"));
    }

    #[test]
    fn the_signer_weight_follows_the_standing_of_each_signer() {
        let validators = build_validators(3);
        let mut set: Vec<(ed25519::Public, TrustScore)> = Vec::new();
        set.push((validators.keys[0], TrustScore::from_percent(10)));
        set.push((validators.keys[1], TrustScore::from_percent(30)));
        set.push((validators.keys[2], TrustScore::from_percent(60)));
        let mut collector = SignatureCollector::open(SLOT, BLOCK, &set, OPENS, WINDOW)
            .expect("the collector opens");
        assert_eq!(collector.total_weight(), TrustScore::FULL as u128);
        assert_eq!(collector.signed_weight(), 0);
        let vote = cast_vote(&validators.keystores[2], &validators.keys[2], SLOT, BLOCK)
            .expect("the validator signs");
        collector.accept(vote, OPENS + 1).expect("the vote lands");
        assert_eq!(collector.signed_weight(), 600_000);
    }

    #[test]
    fn the_member_order_settles_on_the_validator_key() {
        let validators = build_validators(6);
        let collector = collector(&validators);
        let members = collector.members().to_vec();
        let mut sorted = members.clone();
        sorted.sort_by(|left, right| left.0.cmp(&right.0));
        assert_eq!(members, sorted);
        for (position, member) in members.iter().enumerate() {
            assert_eq!(collector.position_of(member), Some(position));
            assert!(collector.weight_at(position).is_some());
        }
        assert!(collector.weight_at(members.len()).is_none());
    }

    #[test]
    fn one_validator_takes_one_position() {
        let validators = build_validators(2);
        let mut repeated = set(&validators, 50);
        repeated.push((validators.keys[0], TrustScore::from_percent(50)));
        let collector = SignatureCollector::open(SLOT, BLOCK, &repeated, OPENS, WINDOW)
            .expect("the collector opens");
        assert_eq!(collector.member_count(), 2);
    }

    #[test]
    fn an_empty_validator_set_names_the_fix() {
        let error = SignatureCollector::open(SLOT, BLOCK, &[], OPENS, WINDOW)
            .expect_err("an empty set reports a problem");
        assert_eq!(error, EngineError::EmptyValidatorSet);
    }

    #[test]
    fn a_vote_verifies_on_its_own() {
        let validators = build_validators(1);
        let vote = cast_vote(&validators.keystores[0], &validators.keys[0], SLOT, BLOCK)
            .expect("the validator signs");
        check_vote(&vote).expect("the vote checks out");
        let bytes = vote.encode();
        assert_eq!(
            CommitVote::decode(&mut &bytes[..]).expect("the encoding decodes"),
            vote
        );
    }

    #[test]
    fn a_keystore_without_the_vote_key_names_the_fix() {
        let keystore: KeystorePtr = Arc::new(MemoryKeystore::new());
        let absent = ed25519::Pair::from_seed(&[7u8; 32]).public();
        let error =
            cast_vote(&keystore, &absent, SLOT, BLOCK).expect_err("the keystore holds no key");
        assert_eq!(error, EngineError::SealKeyAbsent);
    }

    #[test]
    fn the_payload_binds_the_slot_and_the_block() {
        assert_ne!(vote_payload(SLOT, &BLOCK), vote_payload(Slot(1), &BLOCK));
        assert_ne!(vote_payload(SLOT, &BLOCK), vote_payload(SLOT, &[1u8; 32]));
        assert!(vote_payload(SLOT, &BLOCK).starts_with(VOTE_DOMAIN));
    }
}
