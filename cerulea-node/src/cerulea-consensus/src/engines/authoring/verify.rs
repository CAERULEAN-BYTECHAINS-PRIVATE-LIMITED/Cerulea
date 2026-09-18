//! Seal verification on block import.
//!
//! Import checks three conditions before a block joins the chain.
//!
//! 1. **Membership.** The named author sits in the committee drawn for that
//!    slot.
//! 2. **Signature.** The seal verifies against the author public key.
//! 3. **Timing.** The rank the author claimed holds the slot at the moment the
//!    block carries.
//!
//! A block failing any condition never imports. Together the three conditions
//! close the forgeable authorship defect. A node without the author key cannot
//! produce the signature. A node with a key outside the committee cannot pass
//! the membership check. A node inside the committee cannot author ahead of its
//! rank window.

use std::collections::BTreeMap;

use sp_core::{ed25519, sr25519};
use sp_runtime::traits::Header as HeaderT;

use super::committee::SlotCommittee;
use super::draw::{verify_result, DrawInput};
use super::fallback::SlotSchedule;
use super::seal::{check_seal_signature, extract_seal, BlockSeal, ValidatorKeys};
use crate::engines::{EngineError, Rank, Slot};

/// The registry of validator key pairs.
///
/// The Admission Engine records the draw key and the seal key of each validator
/// when the validator joins the set. Import reads the pair back to confirm that
/// the seal key named in a block belongs to the validator that drew the slot.
pub trait ValidatorRegistry {
    /// The seal key recorded against one draw key.
    fn seal_key_of(&self, draw: &sr25519::Public) -> Option<ed25519::Public>;
}

/// A registry of validator key pairs held in memory.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KeyRegistry {
    pairs: BTreeMap<sr25519::Public, ed25519::Public>,
}

impl KeyRegistry {
    /// Start an empty registry.
    pub fn new() -> Self {
        KeyRegistry {
            pairs: BTreeMap::new(),
        }
    }

    /// Record one validator key pair.
    pub fn record(&mut self, keys: ValidatorKeys) {
        self.pairs.insert(keys.draw, keys.seal);
    }

    /// Build a registry from a list of key pairs.
    pub fn from_pairs(pairs: impl IntoIterator<Item = ValidatorKeys>) -> Self {
        let mut registry = KeyRegistry::new();
        for keys in pairs {
            registry.record(keys);
        }
        registry
    }

    /// The number of validators recorded.
    pub fn len(&self) -> usize {
        self.pairs.len()
    }

    /// Report whether the registry records zero validators.
    pub fn is_empty(&self) -> bool {
        self.pairs.is_empty()
    }
}

impl ValidatorRegistry for KeyRegistry {
    fn seal_key_of(&self, draw: &sr25519::Public) -> Option<ed25519::Public> {
        self.pairs.get(draw).copied()
    }
}

/// Everything import needs to check one block.
#[derive(Clone, Debug)]
pub struct SlotContext {
    /// The slot number and slot seed the block serves.
    pub input: DrawInput,
    /// The committee drawn for that slot.
    pub committee: SlotCommittee,
    /// The schedule that sets the rank windows.
    pub schedule: SlotSchedule,
}

impl SlotContext {
    /// Build the context of one slot.
    pub fn new(input: DrawInput, committee: SlotCommittee, schedule: SlotSchedule) -> Self {
        SlotContext {
            input,
            committee,
            schedule,
        }
    }
}

/// What import learned about a block that passed every check.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VerifiedSeal {
    /// The slot the block serves.
    pub slot: Slot,
    /// The rank the author held.
    pub rank: Rank,
    /// The two keys of the author.
    pub keys: ValidatorKeys,
}

/// Check one block header on import.
///
/// The header carries the seal. The context carries the slot, the committee and
/// the schedule. The moment is the timestamp the block carries, in
/// milliseconds.
pub fn verify_header<H, R>(
    header: &H,
    context: &SlotContext,
    registry: &R,
    moment_ms: u64,
) -> Result<VerifiedSeal, EngineError>
where
    H: HeaderT,
    R: ValidatorRegistry,
{
    let seal = extract_seal(header).ok_or(EngineError::SealAbsent)?;
    verify_seal(header, &seal, context, registry, moment_ms)
}

/// Check one seal against the header it covers.
pub fn verify_seal<H, R>(
    header: &H,
    seal: &BlockSeal,
    context: &SlotContext,
    registry: &R,
    moment_ms: u64,
) -> Result<VerifiedSeal, EngineError>
where
    H: HeaderT,
    R: ValidatorRegistry,
{
    let claim = &seal.claim;
    if claim.slot != context.input.slot {
        return Err(EngineError::SealSlotMismatch {
            claimed: claim.slot.0,
            expected: context.input.slot.0,
        });
    }

    // The author claims a slot draw. The proof travels with the claim.
    verify_result(&context.input, &claim.draw)?;

    // Condition one. The named author sits in the committee for that slot.
    let drawn = claim.draw.validator;
    let held = context
        .committee
        .rank_of(&drawn)
        .ok_or(EngineError::AuthorOutsideCommittee { slot: claim.slot.0 })?;
    if held != claim.rank {
        return Err(EngineError::RankMismatch {
            claimed: claim.rank.0,
            held: held.0,
        });
    }

    // The seal key named in the claim belongs to the validator that drew.
    let recorded = registry
        .seal_key_of(&drawn)
        .ok_or(EngineError::ValidatorKeysUnrecorded)?;
    if recorded != claim.author {
        return Err(EngineError::ValidatorKeysMismatch);
    }

    // Condition two. The seal verifies against that author public key.
    check_seal_signature(header, seal)?;

    // Condition three. The rank claimed matches the timing.
    if !context.schedule.claim_open(claim.rank, moment_ms) {
        return Err(EngineError::RankTimingRejected {
            rank: claim.rank.0,
            moment_ms,
        });
    }

    Ok(VerifiedSeal {
        slot: claim.slot,
        rank: claim.rank,
        keys: claim.keys(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engines::authoring::committee::CommitteeDraw;
    use crate::engines::authoring::draw::{draw, DrawResult};
    use crate::engines::authoring::fallback::{AuthoringPolicy, DEFAULT_RANK_TIMEOUT_MS};
    use crate::engines::authoring::seal::{seal_and_apply, SealClaim};
    use crate::engines::{SlotSeed, TrustScore};
    use crate::CERULEA_KEY_TYPE;
    use codec::Encode;
    use sp_core::Pair as PairTrait;
    use sp_keystore::testing::MemoryKeystore;
    use sp_keystore::{Keystore, KeystorePtr};
    use sp_runtime::generic::{Digest, Header};
    use sp_runtime::traits::BlakeTwo256;
    use std::sync::Arc;

    type TestHeader = Header<u32, BlakeTwo256>;

    const SLOT: u64 = 21;
    const SLOT_OPENS_MS: u64 = 100_000;

    struct Network {
        keystore: KeystorePtr,
        registry: KeyRegistry,
        context: SlotContext,
        seal_keys: BTreeMap<sr25519::Public, ed25519::Public>,
    }

    fn draw_pair(seed: u8) -> sr25519::Pair {
        sr25519::Pair::from_seed(&[seed; 32])
    }

    fn build_network(count: u8) -> Network {
        let memory = MemoryKeystore::new();
        let input = DrawInput::new(Slot(SLOT), SlotSeed([4u8; 32]));
        let mut registry = KeyRegistry::new();
        let mut entries: Vec<(DrawResult, TrustScore)> = Vec::new();
        let mut seal_keys = BTreeMap::new();
        for seed in 1..=count {
            let pair = draw_pair(seed);
            let seal = memory
                .ed25519_generate_new(CERULEA_KEY_TYPE, Some(&format!("//Cerulea//{}", seed)))
                .expect("the keystore generates a key");
            registry.record(ValidatorKeys::new(pair.public(), seal));
            seal_keys.insert(pair.public(), seal);
            entries.push((draw(&pair, &input), TrustScore::from_percent(50)));
        }
        let committee = CommitteeDraw::new()
            .assemble(&input, &entries)
            .expect("the committee assembles");
        let schedule = SlotSchedule::new(Slot(SLOT), SLOT_OPENS_MS, AuthoringPolicy::standard());
        Network {
            keystore: Arc::new(memory),
            registry,
            context: SlotContext::new(input, committee, schedule),
            seal_keys,
        }
    }

    fn header() -> TestHeader {
        TestHeader::new(
            9,
            Default::default(),
            Default::default(),
            Default::default(),
            Digest::default(),
        )
    }

    fn seal_for(network: &Network, rank: Rank) -> (TestHeader, BlockSeal) {
        let member = network
            .context
            .committee
            .at_rank(rank)
            .expect("the committee holds this rank");
        let seal_key = network.seal_keys[member];
        let result = draw(
            &draw_pair(
                (1..=12u8)
                    .find(|seed| draw_pair(*seed).public() == *member)
                    .expect("the member sits in the set"),
            ),
            &network.context.input,
        );
        let claim = SealClaim::new(Slot(SLOT), rank, seal_key, result);
        let mut head = header();
        let seal = seal_and_apply(&network.keystore, &mut head, claim)
            .expect("the keystore seals the header");
        (head, seal)
    }

    #[test]
    fn a_sealed_block_from_rank_one_passes_every_check() {
        let network = build_network(9);
        let (head, _) = seal_for(&network, Rank::FIRST);
        let verified = verify_header(&head, &network.context, &network.registry, SLOT_OPENS_MS)
            .expect("the block passes import");
        assert_eq!(verified.slot, Slot(SLOT));
        assert_eq!(verified.rank, Rank::FIRST);
        assert_eq!(
            verified.keys.seal,
            *network
                .seal_keys
                .get(network.context.committee.at_rank(Rank::FIRST).unwrap())
                .unwrap()
        );
    }

    #[test]
    fn a_block_without_a_seal_is_rejected() {
        let network = build_network(9);
        let error = verify_header(
            &header(),
            &network.context,
            &network.registry,
            SLOT_OPENS_MS,
        )
        .expect_err("an unsealed block fails import");
        assert_eq!(error, EngineError::SealAbsent);
        assert!(error.to_string().contains("Fix:"));
    }

    #[test]
    fn a_forged_seal_is_rejected_on_import() {
        let network = build_network(9);
        let (head, seal) = seal_for(&network, Rank::FIRST);
        // The forger keeps the honest signature and names another author.
        let mut forged = seal.clone();
        let outsider = ed25519::Pair::from_seed(&[77u8; 32]).public();
        forged.claim.author = outsider;
        let error = verify_seal(
            &head,
            &forged,
            &network.context,
            &network.registry,
            SLOT_OPENS_MS,
        )
        .expect_err("the forged seal fails import");
        assert_eq!(error, EngineError::ValidatorKeysMismatch);
    }

    #[test]
    fn a_seal_signed_by_a_key_outside_the_claim_is_rejected() {
        let network = build_network(9);
        let (head, seal) = seal_for(&network, Rank::FIRST);
        let mut tampered = seal.clone();
        tampered.signature = ed25519::Signature::from_raw([3u8; 64]);
        let error = verify_seal(
            &head,
            &tampered,
            &network.context,
            &network.registry,
            SLOT_OPENS_MS,
        )
        .expect_err("a broken signature fails import");
        assert!(matches!(error, EngineError::SealSignatureRejected { .. }));
    }

    #[test]
    fn an_author_outside_the_committee_is_rejected() {
        let network = build_network(9);
        let outsider = draw_pair(60);
        let outsider_seal = ed25519::Pair::from_seed(&[60u8; 32]).public();
        let mut registry = network.registry.clone();
        registry.record(ValidatorKeys::new(outsider.public(), outsider_seal));
        let claim = SealClaim::new(
            Slot(SLOT),
            Rank::FIRST,
            outsider_seal,
            draw(&outsider, &network.context.input),
        );
        let memory = MemoryKeystore::new();
        memory
            .insert(
                CERULEA_KEY_TYPE,
                "//Cerulea//outsider",
                &outsider_seal.0[..],
            )
            .ok();
        let mut head = header();
        head.digest_mut()
            .push(sp_runtime::generic::DigestItem::Seal(
                crate::CERULEA_ENGINE_ID,
                BlockSeal {
                    claim: claim.clone(),
                    signature: ed25519::Signature::from_raw([0u8; 64]),
                }
                .encode(),
            ));
        let error = verify_header(&head, &network.context, &registry, SLOT_OPENS_MS)
            .expect_err("an outsider fails import");
        assert!(matches!(error, EngineError::AuthorOutsideCommittee { .. }));
    }

    #[test]
    fn a_rank_that_differs_from_the_committee_is_rejected() {
        let network = build_network(9);
        let member = network
            .context
            .committee
            .at_rank(Rank::FIRST)
            .expect("rank one holds a member");
        let seed = (1..=12u8)
            .find(|seed| draw_pair(*seed).public() == *member)
            .expect("the member sits in the set");
        let seal_key = network.seal_keys[member];
        // The author holds rank one and claims rank two.
        let claim = SealClaim::new(
            Slot(SLOT),
            Rank(2),
            seal_key,
            draw(&draw_pair(seed), &network.context.input),
        );
        let mut head = header();
        let seal = seal_and_apply(&network.keystore, &mut head, claim)
            .expect("the keystore seals the header");
        let error = verify_seal(
            &head,
            &seal,
            &network.context,
            &network.registry,
            SLOT_OPENS_MS + DEFAULT_RANK_TIMEOUT_MS,
        )
        .expect_err("a claimed rank outside the committee position fails import");
        assert_eq!(
            error,
            EngineError::RankMismatch {
                claimed: 2,
                held: 1
            }
        );
    }

    #[test]
    fn a_rank_authoring_ahead_of_its_window_is_rejected() {
        let network = build_network(9);
        let (head, seal) = seal_for(&network, Rank(2));
        let error = verify_seal(
            &head,
            &seal,
            &network.context,
            &network.registry,
            SLOT_OPENS_MS,
        )
        .expect_err("rank two authoring in the rank one window fails import");
        assert!(matches!(error, EngineError::RankTimingRejected { .. }));
        assert!(error.to_string().contains("Fix:"));
    }

    #[test]
    fn rank_two_passes_once_its_window_opens() {
        let network = build_network(9);
        let (head, seal) = seal_for(&network, Rank(2));
        let verified = verify_seal(
            &head,
            &seal,
            &network.context,
            &network.registry,
            SLOT_OPENS_MS + DEFAULT_RANK_TIMEOUT_MS,
        )
        .expect("rank two passes inside its own window");
        assert_eq!(verified.rank, Rank(2));
    }

    #[test]
    fn a_rank_authoring_after_its_window_is_rejected() {
        let network = build_network(9);
        let (head, seal) = seal_for(&network, Rank::FIRST);
        let error = verify_seal(
            &head,
            &seal,
            &network.context,
            &network.registry,
            SLOT_OPENS_MS + DEFAULT_RANK_TIMEOUT_MS,
        )
        .expect_err("rank one authoring past its window fails import");
        assert!(matches!(error, EngineError::RankTimingRejected { .. }));
    }

    #[test]
    fn a_seal_naming_another_slot_is_rejected() {
        let network = build_network(9);
        let (head, mut seal) = seal_for(&network, Rank::FIRST);
        seal.claim.slot = Slot(SLOT + 1);
        let error = verify_seal(
            &head,
            &seal,
            &network.context,
            &network.registry,
            SLOT_OPENS_MS,
        )
        .expect_err("a seal naming another slot fails import");
        assert!(matches!(error, EngineError::SealSlotMismatch { .. }));
    }

    #[test]
    fn an_unrecorded_validator_is_rejected() {
        let network = build_network(9);
        let (head, seal) = seal_for(&network, Rank::FIRST);
        let empty = KeyRegistry::new();
        assert!(empty.is_empty());
        let error = verify_seal(&head, &seal, &network.context, &empty, SLOT_OPENS_MS)
            .expect_err("an unrecorded validator fails import");
        assert_eq!(error, EngineError::ValidatorKeysUnrecorded);
    }

    #[test]
    fn a_registry_records_and_reads_back_a_pair() {
        let mut registry = KeyRegistry::new();
        let draw_key = draw_pair(1).public();
        let seal_key = ed25519::Pair::from_seed(&[1u8; 32]).public();
        registry.record(ValidatorKeys::new(draw_key, seal_key));
        assert_eq!(registry.len(), 1);
        assert_eq!(registry.seal_key_of(&draw_key), Some(seal_key));
        assert_eq!(registry.seal_key_of(&draw_pair(2).public()), None);
        let built = KeyRegistry::from_pairs([ValidatorKeys::new(draw_key, seal_key)]);
        assert_eq!(built, registry);
    }
}
