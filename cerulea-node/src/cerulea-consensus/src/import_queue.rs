//! Block import for the Dynamic Consensus Framework.
//!
//! Import accepts a block only where the header carries a Cerulea seal and that
//! seal passes every check. The unsigned authorship path is gone. Earlier
//! releases read the author from an unsigned digest item naming a public key,
//! so any node built a block naming the scheduled author and the verifier
//! accepted it. Import now reads the seal alone.
//!
//! # What import checks
//!
//! 1. **The header carries a Cerulea seal.** A header without one never
//!    imports.
//! 2. **Membership.** The chain publishes the committee drawn for the slot.
//!    The named author holds a seat in that committee.
//! 3. **Signature.** The seal verifies against the author public key, and the
//!    chain records that key against the draw key that won the seat.
//! 4. **Timing.** The rank the author claimed holds the slot at the moment the
//!    seal carries.
//! 5. **Membership of the active set.** The framework pallet lists the active
//!    validators, and the seal author sits in that list.
//!
//! Checks two, three and four run inside `engines::authoring::verify`. Import
//! builds the slot context from the chain and hands the work to the engine.
//!
//! # Equivocation detection
//!
//! Import keeps the sealed head of every block it accepts. A second head for
//! one slot, one rank and one author seal key completes an equivocation proof.
//! The queue holds the proof and raises an alert in the log. The node then
//! submits the evidence extrinsic to the framework pallet, which returns the
//! standing of that validator to zero and records the fault. A peer that saw
//! only one of the two headers learns of the other through a report, and
//! [`DcfImportQueue::accept_equivocation_report`] checks that report before the
//! queue takes it.

use crate::engines::authoring::committee::SlotCommittee;
use crate::engines::authoring::draw::{derive_slot_seed, DrawInput};
use crate::engines::authoring::equivocation::{
    accept_report, EquivocationProof, EquivocationTracker, ReportQueue,
};
use crate::engines::authoring::fallback::{AuthoringPolicy, SlotSchedule};
use crate::engines::authoring::seal::{
    extract_seal, is_cerulea_seal, strip_seal, BlockSeal, ValidatorKeys,
};
use crate::engines::authoring::verify::{verify_seal_on_arrival, KeyRegistry};
use crate::engines::{CommitteeMember, Rank, Slot, TrustScore};
use crate::metrics::ConsensusMetrics;
use crate::types::ValidatorMetrics;
use cerulea_runtime::AccountId;
use log::{debug, error, info, warn};
use pallet_cerulea_dcf::DcfApi as RuntimeDcfApi;
use pallet_cerulea_dvf::CeruleaAuthoringApi;
use parking_lot::RwLock;
use sc_client_api::Backend;
use sc_consensus::{BlockCheckParams, BlockImport, BlockImportParams, ImportResult, Verifier};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_consensus::Error;
use sp_core::{ed25519, sr25519};
use sp_runtime::traits::{Block as BlockTrait, Header as HeaderT, NumberFor, SaturatedConversion};
use std::collections::BTreeMap;
use std::sync::Arc;

/// Build the committee of one slot from the seats the chain published.
///
/// Each seat names a validator, the draw key that won the seat and the rank the
/// validator holds. Every node reads the same seats from the same chain state.
pub fn committee_from_seats(
    slot: Slot,
    seats: &[pallet_cerulea_dvf::keys::SlotCommitteeSeat<AccountId>],
) -> SlotCommittee {
    SlotCommittee {
        slot,
        members: seats
            .iter()
            .map(|seat| CommitteeMember {
                validator: sr25519::Public::from_raw(seat.keys.draw),
                rank: Rank(seat.rank),
                weighted_draw: seat.seat_value,
            })
            .collect(),
    }
}

/// Build the key registry from the key sets the chain published.
///
/// The registry records the commit weight beside each key pair. That weight is
/// the Trust Score the Standing Engine composes, which the chain answers in
/// parts per million. A validator the chain names no weight for records a
/// score of zero, so the tally never invents weight the chain did not give.
pub fn registry_from_keys(
    keys: &[(AccountId, pallet_cerulea_dvf::keys::ValidatorKeySet)],
    commit_weights: &[(AccountId, u32)],
) -> KeyRegistry {
    let held: BTreeMap<&AccountId, u32> = commit_weights
        .iter()
        .map(|(validator, parts)| (validator, *parts))
        .collect();
    KeyRegistry::from_weighted_pairs(keys.iter().map(|(validator, set)| {
        let keys = ValidatorKeys::new(
            sr25519::Public::from_raw(set.draw),
            ed25519::Public::from_raw(set.seal),
        );
        let parts = held.get(validator).copied().unwrap_or(0);
        (keys, TrustScore::from_parts_per_million(parts))
    }))
}

/// The wait before import checks a block that claims a moment ahead of the
/// local clock.
///
/// A block inside the arrival allowance needs no wait. A block ahead by more,
/// up to the rank timeout, waits until it falls inside the allowance: two
/// honest clocks and a short network path explain that much, and the peer
/// that relayed the block keeps its connection. A block further ahead comes
/// from a clock that runs fast, gets no wait, and the arrival check refuses
/// it.
pub fn early_wait_ms(claimed_ms: u64, now_ms: u64, rank_timeout_ms: u64) -> Option<u64> {
    use crate::engines::authoring::verify::ARRIVAL_ALLOWANCE_MS;
    let ahead = claimed_ms.checked_sub(now_ms)?;
    if ahead <= ARRIVAL_ALLOWANCE_MS || ahead > rank_timeout_ms {
        return None;
    }
    Some(ahead - ARRIVAL_ALLOWANCE_MS)
}

/// The state a block is checked against: the state of its parent.
///
/// The slot committee, the key records and the active set that decide whether
/// a block was authored correctly are the ones its parent left behind. Two
/// forks can carry different epoch draws and different key records. A block
/// checked against the best state of another fork fails although its author
/// held the slot, and sync then drops the peer that sent it. Checking against
/// the parent gives every node the same answer for one block, whichever fork
/// it follows.
///
/// `parent_known` answers whether the node holds a block, so the call reads
/// the parent state only once that state exists.
pub fn verification_state<H: HeaderT>(
    header: &H,
    parent_known: impl Fn(H::Hash) -> Result<bool, String>,
) -> Result<H::Hash, String> {
    let parent = *header.parent_hash();
    match parent_known(parent) {
        Ok(true) => Ok(parent),
        Ok(false) => Err(format!(
            "Block {} arrived before its parent {:?}. Fix: import the parent first, which sync does on its next request.",
            header.number(),
            parent
        )),
        Err(reason) => Err(format!(
            "The parent of block {} failed to load: {}. Fix: check the node database.",
            header.number(),
            reason
        )),
    }
}

/// What import learned about the author of one block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SealedAuthorship {
    /// The account of the author.
    pub author: AccountId,
    /// The slot the block serves.
    pub slot: Slot,
    /// The rank the author claimed.
    pub rank: u8,
}

/// Read the account of a seal author.
pub fn author_account(seal: &BlockSeal) -> AccountId {
    AccountId::from(seal.claim.author.0)
}

/// The import queue of the Dynamic Consensus Framework.
pub struct DcfImportQueue<B, C, BE>
where
    B: BlockTrait,
    C: ProvideRuntimeApi<B> + HeaderBackend<B> + Send + Sync + 'static,
    C::Api: RuntimeDcfApi<B, AccountId, u128, NumberFor<B>> + CeruleaAuthoringApi<B, AccountId>,
    BE: Backend<B>,
{
    client: Arc<C>,
    metrics: ValidatorMetrics,
    consensus_metrics: Option<ConsensusMetrics>,
    equivocation: Arc<RwLock<EquivocationTracker>>,
    reports: Arc<RwLock<ReportQueue>>,
    _phantom: std::marker::PhantomData<(B, BE)>,
}

impl<B, C, BE> DcfImportQueue<B, C, BE>
where
    B: BlockTrait,
    C: ProvideRuntimeApi<B> + HeaderBackend<B> + Send + Sync + 'static,
    C::Api: RuntimeDcfApi<B, AccountId, u128, NumberFor<B>> + CeruleaAuthoringApi<B, AccountId>,
    BE: Backend<B>,
{
    /// Build an import queue over a client.
    pub fn new(client: Arc<C>) -> Self {
        Self {
            client,
            metrics: ValidatorMetrics::default(),
            consensus_metrics: None,
            equivocation: Arc::new(RwLock::new(EquivocationTracker::new())),
            reports: Arc::new(RwLock::new(ReportQueue::new())),
            _phantom: std::marker::PhantomData,
        }
    }

    /// Build an import queue that reports to the metrics registry.
    pub fn new_with_metrics(client: Arc<C>, consensus_metrics: ConsensusMetrics) -> Self {
        Self {
            client,
            metrics: ValidatorMetrics::default(),
            consensus_metrics: Some(consensus_metrics),
            equivocation: Arc::new(RwLock::new(EquivocationTracker::new())),
            reports: Arc::new(RwLock::new(ReportQueue::new())),
            _phantom: std::marker::PhantomData,
        }
    }

    /// The import counters this queue holds.
    pub fn get_metrics(&self) -> &ValidatorMetrics {
        &self.metrics
    }

    /// Return the counters to zero.
    pub fn reset_metrics(&mut self) {
        self.metrics = ValidatorMetrics::default();
        info!("Commit Engine import: counters reset");
    }

    /// Check the seal of one block header.
    ///
    /// The call reports one sentence naming the fix wherever a check fails.
    pub fn verify_authorship(&self, header: &B::Header) -> Result<SealedAuthorship, String> {
        let block_number = (*header.number()).saturated_into::<u32>();

        // Check one. The header carries a Cerulea seal.
        let seal = extract_seal(header).ok_or_else(|| {
            format!(
                "Block {} carries no Cerulea seal. Fix: produce the block on a node holding the \
                 author key for that slot.",
                block_number
            )
        })?;

        let api = self.client.runtime_api();
        // Every check reads the state the parent of this block left behind.
        let state = verification_state(header, |hash| {
            self.client
                .header(hash)
                .map(|held| held.is_some())
                .map_err(|reason| format!("{:?}", reason))
        })?;
        let parent_hash = *header.parent_hash();
        let mut parent_bytes = [0u8; 32];
        let reference = parent_hash.as_ref();
        if reference.len() != parent_bytes.len() {
            return Err(format!(
                "Block {} carries a parent hash of {} bytes. Fix: run a node whose block hash is \
                 thirty two bytes wide.",
                block_number,
                reference.len()
            ));
        }
        parent_bytes.copy_from_slice(reference);

        // The chain publishes the committee for the slot and the key set of
        // every active validator. Import reads both and hands them to the
        // Authoring Engine.
        let seats = api
            .slot_committee(state, seal.claim.slot.0, parent_bytes)
            .map_err(|reason| {
                format!(
                    "The slot committee failed to load: {:?}. Fix: check that the node runtime \
                     matches the chain specification.",
                    reason
                )
            })?;
        if seats.is_empty() {
            return Err(format!(
                "Block {} arrived while slot {} holds no drawn committee. Fix: have each active \
                 validator publish its draw for the epoch that runs now.",
                block_number, seal.claim.slot.0
            ));
        }
        let key_sets = api.active_validator_keys(state).map_err(|reason| {
            format!(
                "The validator key records failed to load: {:?}. Fix: check that the node runtime \
                 matches the chain specification.",
                reason
            )
        })?;
        // The commit weight of a validator is its Trust Score. Import records
        // that weight beside the key pair.
        let commit_weights = api.validator_commit_weights(state).map_err(|reason| {
            format!(
                "The validator commit weights failed to load: {:?}. Fix: check that the node \
                 runtime matches the chain specification.",
                reason
            )
        })?;

        let seed = derive_slot_seed(reference, seal.claim.slot);
        let input = DrawInput::new(seal.claim.slot, seed);
        let committee = committee_from_seats(seal.claim.slot, &seats);
        let registry = registry_from_keys(&key_sets, &commit_weights);
        let schedule = SlotSchedule::for_slot(seal.claim.slot, AuthoringPolicy::standard());
        let context =
            crate::engines::authoring::verify::SlotContext::new(input, committee, schedule);

        // Checks two, three and four. Membership, signature and rank timing.
        // The rank window reads the moment the seal claims, and the local
        // clock bounds that claim, so a later rank never takes the slot while
        // an earlier rank still holds it.
        let arrival_ms = crate::clock::now_ms();
        let verified = match verify_seal_on_arrival(header, &seal, &context, &registry, arrival_ms)
        {
            Ok(verified) => verified,
            Err(reason) => {
                // A header claiming a rank its author lacks never imports.
                // The author still signed it, so the sealed head joins the
                // equivocation memory and a second header for the slot
                // completes the evidence.
                if matches!(reason, crate::engines::EngineError::RankMismatch { .. }) {
                    self.observe_refused_for_equivocation(header, &seal, &registry);
                }
                return Err(reason.to_string());
            }
        };

        let author = author_account(&seal);

        // Check five. The author holds a seat in the active validator set.
        let active = api.get_active_validators(state).map_err(|reason| {
            format!(
                "The active validator set failed to load: {:?}. Fix: check that the node runtime \
                 matches the chain specification.",
                reason
            )
        })?;
        if active.is_empty() {
            return Err(format!(
                "Block {} arrived while the active validator set holds no members. Fix: admit at \
                 least one validator through the Admission Engine.",
                block_number
            ));
        }
        let seated = seats
            .iter()
            .find(|seat| seat.keys.draw == verified.keys.draw.0)
            .map(|seat| seat.validator.clone())
            .ok_or_else(|| {
                format!(
                    "Block {} names a draw key with no seat record. Fix: register the validator \
                     keys through the Admission Engine before authoring.",
                    block_number
                )
            })?;
        if !active.contains(&seated) {
            if let Some(metrics) = &self.consensus_metrics {
                metrics.record_author_mismatch();
            }
            return Err(format!(
                "Block {} names an author outside the active validator set. Fix: admit the \
                 validator through the Admission Engine before it authors.",
                block_number
            ));
        }

        // The sealed head joins the equivocation memory. A second head for this
        // slot, this rank and this seal key completes the evidence.
        self.observe_for_equivocation(header, &seal);

        Ok(SealedAuthorship {
            author,
            slot: verified.slot,
            rank: verified.rank.0,
        })
    }

    /// Keep the sealed head of one block and raise any equivocation it shows.
    ///
    /// The call runs on every block that passes the seal checks. A header
    /// whose claim runs to an unexpected length leaves the memory untouched.
    pub fn observe_for_equivocation(&self, header: &B::Header, seal: &BlockSeal) {
        let detected = {
            let mut tracker = self.equivocation.write();
            match tracker.observe_header(header, seal) {
                Ok(found) => found,
                Err(reason) => {
                    debug!(
                        "Authoring Engine equivocation: the sealed head was set aside. {}",
                        reason
                    );
                    None
                }
            }
        };
        if let Some(proof) = detected {
            self.raise_equivocation(proof);
        }
    }

    /// Keep the sealed head of one header refused on its claimed rank.
    ///
    /// The tracker checks the recorded seal key and the seal signature first,
    /// so only a header the author signed joins the memory.
    pub fn observe_refused_for_equivocation(
        &self,
        header: &B::Header,
        seal: &BlockSeal,
        registry: &KeyRegistry,
    ) {
        let detected = {
            let mut tracker = self.equivocation.write();
            match tracker.observe_refused(header, seal, registry) {
                Ok(found) => found,
                Err(reason) => {
                    debug!(
                        "Authoring Engine equivocation: the refused head was set aside. {}",
                        reason
                    );
                    None
                }
            }
        };
        if let Some(proof) = detected {
            self.raise_equivocation(proof);
        }
    }

    /// Place one proof in the queue and raise the alert.
    fn raise_equivocation(&self, proof: EquivocationProof) {
        let placed = self.reports.write().place(proof);
        if placed {
            warn!(
                "Authoring Engine equivocation: validator {:?} sealed two headers for slot {} at                  rank {}. The evidence goes to the framework pallet and standing returns to zero.",
                proof.first.seal_key(),
                proof.first.slot(),
                proof.first.rank()
            );
            if let Some(metrics) = &self.consensus_metrics {
                metrics.record_author_mismatch();
            }
        }
    }

    /// Accept one equivocation report arriving from a peer.
    ///
    /// The byte bound runs before any decoding. The evidence check then runs
    /// over the whole proof, so the node trusts no peer. The call reports the
    /// facts the proof states.
    pub fn accept_equivocation_report(
        &self,
        bytes: &[u8],
    ) -> Result<crate::engines::authoring::equivocation::EquivocationFacts, String> {
        let (report, facts) = accept_report(bytes).map_err(|reason| reason.to_string())?;
        self.raise_equivocation(report.proof);
        Ok(facts)
    }

    /// The proofs waiting for the evidence extrinsic.
    pub fn equivocation_reports(&self) -> Vec<EquivocationProof> {
        self.reports.read().held()
    }

    /// Take every waiting proof out for submission.
    pub fn take_equivocation_reports(&self) -> Vec<EquivocationProof> {
        self.reports.write().drain()
    }

    /// The number of equivocations this node has detected.
    pub fn equivocation_count(&self) -> u64 {
        self.equivocation.read().detections()
    }
}

#[async_trait::async_trait]
impl<B, C, BE> BlockImport<B> for DcfImportQueue<B, C, BE>
where
    B: BlockTrait,
    C: ProvideRuntimeApi<B> + HeaderBackend<B> + Send + Sync + 'static,
    C::Api: RuntimeDcfApi<B, AccountId, u128, NumberFor<B>> + CeruleaAuthoringApi<B, AccountId>,
    BE: Backend<B>,
{
    type Error = Error;

    async fn check_block(
        &self,
        block: BlockCheckParams<B>,
    ) -> std::result::Result<ImportResult, Self::Error> {
        let block_number = block.number.saturated_into::<u32>();
        debug!("Commit Engine import: checking block #{}", block_number);

        let api = self.client.runtime_api();
        // The active set the parent of this block left behind decides.
        let active = api
            .get_active_validators(block.parent_hash)
            .map_err(|reason| {
                sp_consensus::Error::ClientImport(format!(
                "The active validator set failed to load: {:?}. Fix: check that the node runtime \
                 matches the chain specification.",
                reason
            ))
            })?;
        if active.is_empty() {
            error!(
                "Commit Engine import: block #{} arrived with no active validators",
                block_number
            );
            return Ok(ImportResult::imported(false));
        }
        Ok(ImportResult::imported(true))
    }

    async fn import_block(
        &self,
        mut block: BlockImportParams<B>,
    ) -> std::result::Result<ImportResult, Self::Error> {
        let block_number = (*block.header.number()).saturated_into::<u32>();
        let block_hash = block.header.hash();

        let authorship = self.verify_authorship(&block.header).map_err(|reason| {
            error!(
                "Commit Engine import: block #{} rejected. {}",
                block_number, reason
            );
            sp_consensus::Error::ClientImport(reason)
        })?;

        block.fork_choice = Some(sc_consensus::ForkChoiceStrategy::LongestChain);

        info!(
            "Commit Engine import: block #{} ({:?}) sealed by {:?} at slot {} rank {}",
            block_number, block_hash, authorship.author, authorship.slot.0, authorship.rank
        );

        Ok(ImportResult::Imported(sc_consensus::ImportedAux {
            header_only: false,
            clear_justification_requests: false,
            needs_justification: false,
            bad_justification: false,
            is_new_best: true,
        }))
    }
}

#[async_trait::async_trait]
impl<B, C, BE> Verifier<B> for DcfImportQueue<B, C, BE>
where
    B: BlockTrait,
    C: ProvideRuntimeApi<B> + HeaderBackend<B> + Send + Sync + 'static,
    C::Api: RuntimeDcfApi<B, AccountId, u128, NumberFor<B>> + CeruleaAuthoringApi<B, AccountId>,
    BE: Backend<B>,
{
    async fn verify(
        &self,
        mut block: BlockImportParams<B>,
    ) -> Result<BlockImportParams<B>, String> {
        let block_number = (*block.header.number()).saturated_into::<u32>();
        // A block a little ahead of the claimed moment waits until the moment
        // arrives, so a small difference between two clocks costs a short
        // wait rather than the peer that relayed the block.
        if let Some(seal) = extract_seal(&block.header) {
            let rank_timeout = AuthoringPolicy::standard().rank_timeout_ms;
            if let Some(wait) =
                early_wait_ms(seal.claim.moment_ms, crate::clock::now_ms(), rank_timeout)
            {
                debug!(
                    "Commit Engine import: block #{} claims a moment {} milliseconds ahead; waiting",
                    block_number, wait
                );
                tokio::time::sleep(std::time::Duration::from_millis(wait)).await;
            }
        }
        let authorship = self.verify_authorship(&block.header)?;
        debug!(
            "Commit Engine import: block #{} passed the seal checks at slot {} rank {}",
            block_number, authorship.slot.0, authorship.rank
        );
        // The seal travels as a post digest, the same way the author imports
        // its own block. Execution reads the header without the seal, so the
        // digest the runtime counts matches the digest the author built. The
        // block keeps the hash it arrived under.
        if block.post_digests.is_empty() {
            let arrived_hash = block.header.hash();
            let seal_item = block
                .header
                .digest()
                .logs()
                .iter()
                .rev()
                .find(|item| is_cerulea_seal(item))
                .cloned();
            if let Some(item) = seal_item {
                block.header = strip_seal(&block.header);
                block.post_digests.push(item);
                if block.post_header().hash() != arrived_hash {
                    return Err(format!(
                        "Block {} carries its Cerulea seal ahead of another digest item. \
                         Fix: produce the block on a node that places the seal last.",
                        block_number
                    ));
                }
                block.post_hash = Some(arrived_hash);
            }
        }
        if block.fork_choice.is_none() {
            block.fork_choice = Some(sc_consensus::ForkChoiceStrategy::LongestChain);
        }
        Ok(block)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engines::authoring::draw::{draw, verify_result};
    use crate::engines::authoring::seal::{check_seal_signature, seal_and_apply, SealClaim};
    use crate::engines::Rank;
    use crate::CERULEA_KEY_TYPE;
    use sp_core::{ed25519, sr25519, Pair as PairTrait};
    use sp_keystore::testing::MemoryKeystore;
    use sp_keystore::{Keystore, KeystorePtr};
    use sp_runtime::generic::{Digest, DigestItem, Header};
    use sp_runtime::traits::BlakeTwo256;
    use std::sync::Arc;

    type TestHeader = Header<u32, BlakeTwo256>;

    fn header(number: u32) -> TestHeader {
        TestHeader::new(
            number,
            Default::default(),
            Default::default(),
            Default::default(),
            Digest::default(),
        )
    }

    fn keystore() -> KeystorePtr {
        Arc::new(MemoryKeystore::new())
    }

    #[test]
    fn a_header_without_a_seal_carries_no_author() {
        let head = header(4);
        assert!(extract_seal(&head).is_none());
    }

    #[test]
    fn an_unsigned_author_digest_carries_no_authorship() {
        // Earlier releases named the author in an unsigned digest item. Import
        // reads the seal alone, so an unsigned item names nobody.
        let mut head = header(4);
        let author = AccountId::from([9u8; 32]);
        head.digest_mut().push(DigestItem::PreRuntime(
            crate::CERULEA_ENGINE_ID,
            codec::Encode::encode(&author),
        ));
        head.digest_mut().push(DigestItem::Consensus(
            crate::CERULEA_ENGINE_ID,
            codec::Encode::encode(&author),
        ));
        assert!(extract_seal(&head).is_none());
    }

    #[test]
    fn a_sealed_header_reads_back_its_author_account() {
        let keystore = keystore();
        let seal_key = keystore
            .ed25519_generate_new(CERULEA_KEY_TYPE, Some("//Cerulea//author"))
            .expect("the keystore generates a key");
        let draw_pair = sr25519::Pair::from_seed(&[5u8; 32]);
        let mut head = header(4);
        let seed = derive_slot_seed(head.parent_hash().as_ref(), Slot(4));
        let input = DrawInput::new(Slot(4), seed);
        let claim = SealClaim::new(Slot(4), Rank::FIRST, seal_key, draw(&draw_pair, &input), 0);
        let seal = seal_and_apply(&keystore, &mut head, claim).expect("the keystore seals it");
        assert_eq!(author_account(&seal), AccountId::from(seal_key.0));
        check_seal_signature(&head, &seal).expect("the seal checks out");
        verify_result(&input, &seal.claim.draw).expect("the draw checks out");
    }

    #[test]
    fn a_seal_from_another_key_fails_the_signature_check() {
        let keystore = keystore();
        let seal_key = keystore
            .ed25519_generate_new(CERULEA_KEY_TYPE, Some("//Cerulea//author"))
            .expect("the keystore generates a key");
        let draw_pair = sr25519::Pair::from_seed(&[5u8; 32]);
        let mut head = header(4);
        let seed = derive_slot_seed(head.parent_hash().as_ref(), Slot(4));
        let input = DrawInput::new(Slot(4), seed);
        let claim = SealClaim::new(Slot(4), Rank::FIRST, seal_key, draw(&draw_pair, &input), 0);
        let seal = seal_and_apply(&keystore, &mut head, claim).expect("the keystore seals it");
        let mut forged = seal.clone();
        forged.claim.author = ed25519::Pair::from_seed(&[1u8; 32]).public();
        assert!(check_seal_signature(&head, &forged).is_err());
    }

    #[test]
    fn the_slot_seed_follows_the_parent_hash_and_the_slot() {
        let one = derive_slot_seed(&[1u8; 32], Slot(4));
        let two = derive_slot_seed(&[2u8; 32], Slot(4));
        let three = derive_slot_seed(&[1u8; 32], Slot(5));
        assert_ne!(one, two);
        assert_ne!(one, three);
        assert_eq!(one, derive_slot_seed(&[1u8; 32], Slot(4)));
    }

    /// A block a little ahead of the local clock waits; a block far ahead
    /// does not.
    #[test]
    fn a_block_slightly_ahead_waits_and_a_block_far_ahead_does_not() {
        let rank = crate::engines::authoring::fallback::DEFAULT_RANK_TIMEOUT_MS;
        // Inside the allowance: no wait.
        assert_eq!(super::early_wait_ms(10_400, 10_000, rank), None);
        // A block 800 milliseconds ahead, as the delay exercise recorded,
        // waits 300 milliseconds and then passes the arrival check.
        assert_eq!(super::early_wait_ms(10_800, 10_000, rank), Some(300));
        // The bound is the rank timeout.
        assert_eq!(super::early_wait_ms(11_500, 10_000, rank), Some(1_000));
        assert_eq!(super::early_wait_ms(11_501, 10_000, rank), None);
        // A clock thirty seconds fast: no wait, the arrival check refuses.
        assert_eq!(super::early_wait_ms(40_000, 10_000, rank), None);
        // A block from the past: no wait.
        assert_eq!(super::early_wait_ms(9_000, 10_000, rank), None);
    }

    /// A block is checked against the state its parent left behind.
    ///
    /// The best block of the node plays no part, so two nodes on two forks
    /// reach the same answer for one block.
    #[test]
    fn a_block_is_checked_against_the_state_of_its_parent() {
        let parent = sp_core::H256::repeat_byte(7);
        let best = sp_core::H256::repeat_byte(9);
        let child = Header::<u64, BlakeTwo256>::new(
            12,
            Default::default(),
            Default::default(),
            parent,
            Digest::default(),
        );
        let known =
            |held: sp_core::H256| -> Result<bool, String> { Ok(held == parent || held == best) };
        assert_eq!(super::verification_state(&child, known), Ok(parent));

        let unknown = |_: sp_core::H256| -> Result<bool, String> { Ok(false) };
        let error =
            super::verification_state(&child, unknown).expect_err("an unknown parent waits");
        assert!(error.contains("before its parent"), "{error}");
        assert!(error.contains("Fix:"), "{error}");

        let broken = |_: sp_core::H256| -> Result<bool, String> { Err("closed".into()) };
        assert!(super::verification_state(&child, broken).is_err());
    }
}
