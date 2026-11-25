//! Equivocation report propagation.
//!
//! Equivocation means one validator sealing two different headers for one slot
//! at any rank. A slot is one fixed span of time. A node holding both headers
//! builds the evidence and sends it to its peers on this path. A node that saw
//! one header alone learns of the other here.
//!
//! # What the receiving node trusts
//!
//! The receiving node trusts the evidence rather than the peer. The report
//! carries both sealed headers with the signature the author placed on each
//! one. The node runs the whole check itself. A report that fails any part of
//! the check goes no further.
//!
//! # What keeps the path bounded
//!
//! Three limits hold. A hard byte bound runs before any decoding, so an
//! oversized message costs one length comparison. A capped message set
//! remembers the reports already seen and evicts the entry that went longest
//! without a hit. A capped queue holds the checked proofs waiting for the
//! evidence extrinsic and refuses a proof at capacity.

use codec::Decode;
use log::{info, warn};
use parking_lot::RwLock;
use sc_network::PeerId;
use sc_network_gossip::{MessageIntent, ValidationResult, ValidatorContext};
use sp_runtime::traits::Block as BlockT;
use std::sync::Arc;

use crate::engines::authoring::equivocation::{
    accept_report, EquivocationFacts, EquivocationProof, ReportQueue,
};
use crate::engines::commit::gossip::MessageSet;

pub use crate::engines::authoring::equivocation::{
    EQUIVOCATION_PROTOCOL_NAME, MAX_EQUIVOCATION_REPORT_BYTES,
};

/// The validator of the equivocation report path.
///
/// The validator checks every arriving report and places the proofs it accepts
/// in the queue. The node then submits the evidence extrinsic to the framework
/// pallet, which returns the standing of the named validator to zero.
pub struct EquivocationGossipValidator<B: BlockT> {
    queue: Arc<RwLock<ReportQueue>>,
    known_messages: RwLock<MessageSet<B::Hash>>,
}

impl<B: BlockT> EquivocationGossipValidator<B> {
    /// Build a validator over one report queue.
    pub fn new(queue: Arc<RwLock<ReportQueue>>) -> Self {
        EquivocationGossipValidator {
            queue,
            known_messages: RwLock::new(MessageSet::new()),
        }
    }

    /// The queue this validator fills.
    pub fn queue(&self) -> Arc<RwLock<ReportQueue>> {
        self.queue.clone()
    }

    /// Check one arriving message and place the proof it carries.
    ///
    /// The call reports the facts the proof states where every check passes.
    pub fn accept(&self, data: &[u8]) -> Result<EquivocationFacts, String> {
        let (report, facts) = accept_report(data).map_err(|reason| reason.to_string())?;
        self.place(report.proof);
        Ok(facts)
    }

    /// Place one checked proof in the queue.
    fn place(&self, proof: EquivocationProof) -> bool {
        self.queue.write().place(proof)
    }

    /// The proofs waiting for the evidence extrinsic.
    pub fn waiting(&self) -> Vec<EquivocationProof> {
        self.queue.read().held()
    }
}

impl<B: BlockT> sc_network_gossip::Validator<B> for EquivocationGossipValidator<B>
where
    B: BlockT + 'static,
{
    fn validate(
        &self,
        _context: &mut dyn ValidatorContext<B>,
        _sender: &PeerId,
        data: &[u8],
    ) -> ValidationResult<B::Hash> {
        let digest = sp_core::hashing::blake2_256(data);
        let topic = B::Hash::decode(&mut &digest[..]).unwrap_or_default();

        {
            let mut known = self.known_messages.write();
            if known.record(topic) {
                return ValidationResult::ProcessAndDiscard(topic);
            }
        }

        match self.accept(data) {
            Ok(facts) => {
                warn!(
                    "Authoring Engine equivocation: a peer reported two headers sealed for slot {} \
                     at rank {}. The evidence goes to the framework pallet.",
                    facts.slot, facts.rank
                );
                ValidationResult::ProcessAndKeep(topic)
            }
            Err(reason) => {
                info!(
                    "Authoring Engine equivocation: the report was set aside. {}",
                    reason
                );
                ValidationResult::Discard
            }
        }
    }

    fn message_expired<'a>(&'a self) -> Box<dyn FnMut(B::Hash, &[u8]) -> bool + 'a> {
        Box::new(move |_topic, _data| false)
    }

    fn message_allowed<'a>(
        &'a self,
    ) -> Box<dyn FnMut(&PeerId, MessageIntent, &B::Hash, &[u8]) -> bool + 'a> {
        Box::new(move |_peer, _intent, _topic, _data| true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engines::authoring::draw::{draw, DrawInput};
    use crate::engines::authoring::equivocation::sealed_head;
    use crate::engines::authoring::seal::{seal_and_apply, SealClaim};
    use crate::engines::{Rank, Slot, SlotSeed};
    use crate::CERULEA_KEY_TYPE;
    use codec::Encode;
    use sp_core::{ed25519, sr25519, Pair as PairTrait};
    use sp_keystore::testing::MemoryKeystore;
    use sp_keystore::{Keystore, KeystorePtr};
    use sp_runtime::generic::{Digest, Header};
    use sp_runtime::traits::{BlakeTwo256, Header as HeaderT};
    use sp_runtime::OpaqueExtrinsic;

    type TestHeader = Header<u32, BlakeTwo256>;
    type TestBlock = sp_runtime::generic::Block<TestHeader, OpaqueExtrinsic>;

    fn keystore() -> KeystorePtr {
        Arc::new(MemoryKeystore::new())
    }

    fn author_key(keystore: &KeystorePtr, seed: &str) -> ed25519::Public {
        keystore
            .ed25519_generate_new(CERULEA_KEY_TYPE, Some(seed))
            .expect("the keystore generates a key")
    }

    fn head(
        keystore: &KeystorePtr,
        author: ed25519::Public,
        number: u32,
        slot: u64,
    ) -> crate::engines::authoring::equivocation::SealedHead {
        let pair = sr25519::Pair::from_seed(&[3u8; 32]);
        let input = DrawInput::new(Slot(slot), SlotSeed([1u8; 32]));
        let claim = SealClaim::new(
            Slot(slot),
            Rank::FIRST,
            author,
            draw(&pair, &input),
            slot * 1_000,
        );
        let mut header = TestHeader::new(
            number,
            Default::default(),
            Default::default(),
            Default::default(),
            Digest::default(),
        );
        let seal = seal_and_apply(keystore, &mut header, claim).expect("the keystore seals it");
        sealed_head(&header, &seal).expect("the claim encodes to the stated length")
    }

    fn validator() -> EquivocationGossipValidator<TestBlock> {
        EquivocationGossipValidator::new(Arc::new(RwLock::new(ReportQueue::new())))
    }

    #[test]
    fn a_checked_report_reaches_the_queue() {
        let keystore = keystore();
        let author = author_key(&keystore, "//Cerulea//author");
        let proof =
            EquivocationProof::new(head(&keystore, author, 1, 9), head(&keystore, author, 2, 9));
        let validator = validator();
        let facts = validator
            .accept(
                &crate::engines::authoring::equivocation::EquivocationReport::new(proof).encode(),
            )
            .expect("the report checks out");
        assert_eq!(facts.slot, 9);
        assert_eq!(validator.waiting().len(), 1);
    }

    #[test]
    fn an_oversized_report_never_reaches_the_queue() {
        let validator = validator();
        let message = vec![0u8; MAX_EQUIVOCATION_REPORT_BYTES + 1];
        assert!(validator.accept(&message).is_err());
        assert!(validator.waiting().is_empty());
    }

    #[test]
    fn a_report_carrying_one_header_twice_never_reaches_the_queue() {
        let keystore = keystore();
        let author = author_key(&keystore, "//Cerulea//author");
        let sealed = head(&keystore, author, 1, 9);
        let validator = validator();
        let message = crate::engines::authoring::equivocation::EquivocationReport::new(
            EquivocationProof::new(sealed, sealed),
        )
        .encode();
        assert!(validator.accept(&message).is_err());
        assert!(validator.waiting().is_empty());
    }

    #[test]
    fn a_repeat_report_leaves_the_queue_unchanged() {
        let keystore = keystore();
        let author = author_key(&keystore, "//Cerulea//author");
        let proof =
            EquivocationProof::new(head(&keystore, author, 1, 9), head(&keystore, author, 2, 9));
        let message =
            crate::engines::authoring::equivocation::EquivocationReport::new(proof).encode();
        let validator = validator();
        assert!(validator.accept(&message).is_ok());
        assert!(validator.accept(&message).is_ok());
        assert_eq!(validator.waiting().len(), 1);
    }

    #[test]
    fn the_protocol_name_holds_its_stated_value() {
        assert_eq!(EQUIVOCATION_PROTOCOL_NAME, "/cerulea/equivocation/1");
    }
}
