//! An in-process network of Cerulea validator nodes.
//!
//! The harness runs the real engines of the Dynamic Consensus Framework. The
//! Authoring Engine draws the slot committee, seals the block and checks the
//! seal on import. The Commit Engine collects signatures and decides when a
//! domain commits. The Execution Engine groups transactions into domains, runs
//! them and produces the state roots. Nothing here stands in for an engine. The
//! harness supplies the parts a running network supplies. A keystore for each
//! validator. A message path between nodes. A clock.
//!
//! # What the harness carries
//!
//! 1. **One node per validator.** Each node holds its own keystore with an
//!    sr25519 draw key and an ed25519 seal key. Each node holds its own chain
//!    of imported headers and its own record of committed heights.
//! 2. **One committee per slot.** Every node draws the committee from the same
//!    published state through the shared primitives crate. The draw reaches the
//!    same order on every node for every slot.
//! 3. **A partition switch.** A node taken out of the network sends nothing and
//!    receives nothing. The remaining nodes carry on.
//! 4. **A deployment profile.** The profile carries the rank timeout, the
//!    committee size and the share of weight each transaction class needs.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use cerulea_consensus::engines::authoring::committee::SlotCommittee;
use cerulea_consensus::engines::authoring::draw::{
    derive_slot_seed, draw_from_keystore, epoch_draw_from_keystore, DrawInput, DrawResult,
};
use cerulea_consensus::engines::authoring::fallback::{
    AuthoringPolicy, FallbackTracker, SlotSchedule,
};
use cerulea_consensus::engines::authoring::gate::{AuthorGate, GateDecision};
use cerulea_consensus::engines::authoring::seal::{
    extract_seal, seal_and_apply, BlockSeal, SealClaim, ValidatorKeys,
};
use cerulea_consensus::engines::authoring::verify::{verify_header, KeyRegistry, SlotContext};
use cerulea_consensus::engines::commit::collect::{cast_vote, SignatureCollector};
use cerulea_consensus::engines::commit::commit::{DomainCommitter, ThresholdTable};
use cerulea_consensus::engines::execution::partition::DomainPartitioner;
use cerulea_consensus::engines::execution::roots::StateRoots;
use cerulea_consensus::engines::execution::runner::DomainRunner;
use cerulea_consensus::engines::manifest::{AccessManifest, CollectionAccess, PendingTransaction};
use cerulea_consensus::engines::manifest_guard::MemoryStateStore;
use cerulea_consensus::engines::peers::{DiversityPolicy, PeerAddress, PeerRecord, PeerSet};
use cerulea_consensus::engines::{
    CollectionId, CommitEngine, CommitteeMember, EngineError, KeyRange, Rank, Slot,
    TransactionClass, TrustScore,
};
use cerulea_consensus::CERULEA_KEY_TYPE;
use cerulea_primitives::{rank_seats, SeatInput};
use sp_core::{ed25519, sr25519};
use sp_keystore::testing::MemoryKeystore;
use sp_keystore::{Keystore, KeystorePtr};
use sp_runtime::generic::{Digest, Header};
use sp_runtime::traits::{BlakeTwo256, Header as HeaderT};

/// The header type the harness carries.
pub type TestHeader = Header<u32, BlakeTwo256>;

/// The share of weight a routine domain needs inside the harness profile.
///
/// The profile sets the routine share at two thirds of the weight. Two signer
/// sets each carrying two thirds of the weight share at least one member, so
/// two blocks at one height never both commit. The share also sets the outage
/// the network survives. A network of nine keeps committing while six remain.
pub const HARNESS_ROUTINE_PERCENT: u8 = 66;

/// The window the signature collector holds open, in milliseconds.
pub const COLLECTION_WINDOW_MS: u64 = 1_000;

/// One validator node inside the harness.
pub struct ValidatorNode {
    /// The plain name a report shows.
    pub name: String,
    /// The keystore holding both private keys.
    pub keystore: KeystorePtr,
    /// The sr25519 key the slot draw uses.
    pub draw_key: sr25519::Public,
    /// The ed25519 key the block seal uses.
    pub seal_key: ed25519::Public,
    /// The Trust Score the network measures for this validator.
    pub standing: TrustScore,
    /// The headers this node imported, in order.
    pub chain: Vec<TestHeader>,
    /// The block hash this node committed at each height.
    pub committed: BTreeMap<u32, [u8; 32]>,
    /// Whether this node reaches the rest of the network.
    pub reachable: bool,
    /// The peers this node holds.
    ///
    /// An eclipse attack isolates a node behind attacker-controlled peers. The
    /// peer set counts the distinct address ranges the node reaches, and a node
    /// failing the requirement halts authoring.
    pub peers: PeerSet,
}

impl ValidatorNode {
    /// Build one node with a fresh keystore.
    pub fn new(name: &str, seed: u8, standing: TrustScore) -> Self {
        let keystore = MemoryKeystore::new();
        let phrase = format!("//Cerulea//harness//{}", seed);
        let draw_key = keystore
            .sr25519_generate_new(CERULEA_KEY_TYPE, Some(&phrase))
            .expect("the keystore generates a draw key");
        let seal_key = keystore
            .ed25519_generate_new(CERULEA_KEY_TYPE, Some(&phrase))
            .expect("the keystore generates a seal key");
        ValidatorNode {
            name: name.to_string(),
            keystore: Arc::new(keystore),
            draw_key,
            seal_key,
            standing,
            chain: Vec::new(),
            committed: BTreeMap::new(),
            reachable: true,
            peers: healthy_peers(seed),
        }
    }

    /// Place this node behind peers of one address range.
    ///
    /// Every peer sits in one range, so the peer set fails the diversity
    /// requirement and the node halts authoring.
    pub fn eclipse(&mut self) {
        let mut set = PeerSet::new(DiversityPolicy::public_chain());
        for index in 0..8u8 {
            set.admit(PeerRecord::new(
                &format!("eclipse-{}", index),
                PeerAddress::V4([203, 0, 113, index]),
                None,
            ))
            .expect("a public chain admits every peer");
        }
        self.peers = set;
    }

    /// Return this node to a peer set spanning several address ranges.
    pub fn restore_peers(&mut self, seed: u8) {
        self.peers = healthy_peers(seed);
    }

    /// Report whether this node may author a block.
    pub fn authoring_allowed(&self) -> bool {
        self.peers.authoring_allowed()
    }

    /// The two keys this validator carries.
    pub fn keys(&self) -> ValidatorKeys {
        ValidatorKeys::new(self.draw_key, self.seal_key)
    }

    /// The height this node reached.
    pub fn height(&self) -> u32 {
        self.chain.len() as u32
    }

    /// The hash of the head, or the genesis hash where the chain is empty.
    pub fn head_hash(&self) -> [u8; 32] {
        match self.chain.last() {
            Some(header) => header.hash().into(),
            None => [0u8; 32],
        }
    }
}

/// A peer set spanning four address ranges.
///
/// The set meets the diversity requirement, so a node holding it authors.
pub fn healthy_peers(seed: u8) -> PeerSet {
    let mut set = PeerSet::new(DiversityPolicy::public_chain());
    for index in 0..4u8 {
        set.admit(PeerRecord::new(
            &format!("peer-{}-{}", seed, index),
            PeerAddress::V4([10, 0, index, seed]),
            None,
        ))
        .expect("a public chain admits every peer");
    }
    set
}

/// What one slot produced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SlotOutcome {
    /// A block was sealed, accepted by every reachable node and committed.
    Committed {
        /// The height the block took.
        height: u32,
        /// The hash of the committed block.
        hash: [u8; 32],
        /// The rank that authored.
        rank: u8,
        /// The share of weight that signed, in percent.
        signed_percent: u8,
    },
    /// A block was sealed and accepted, and the signing weight stayed below the
    /// share the class needs. The block waits for the absent validators.
    Withheld {
        /// The height the block reached.
        height: u32,
        /// The share of weight that signed, in percent.
        signed_percent: u8,
    },
    /// No reachable node held a rank that could author the slot.
    NoAuthor,
}

impl SlotOutcome {
    /// Report whether the slot committed a block.
    pub fn committed(&self) -> bool {
        matches!(self, SlotOutcome::Committed { .. })
    }
}

/// The in-process network.
pub struct TestNetwork {
    /// The nodes taking part.
    pub nodes: Vec<ValidatorNode>,
    /// The authoring profile the deployment carries.
    pub policy: AuthoringPolicy,
    /// The share of weight each transaction class needs.
    pub thresholds: ThresholdTable,
    /// The number of slots one epoch carries.
    pub slots_per_epoch: u64,
    /// Every block hash accepted at each height.
    pub accepted: BTreeMap<u32, BTreeSet<[u8; 32]>>,
    /// The epoch the network runs.
    pub epoch: u32,
}

impl TestNetwork {
    /// Build a network of the stated size.
    ///
    /// Every validator starts at the same Trust Score, so the committee draw
    /// rests on the verifiable random value alone.
    pub fn with_validators(count: usize) -> Self {
        Self::with_standing(count, TrustScore::from_percent(50))
    }

    /// Build a network where every validator carries the stated Trust Score.
    pub fn with_standing(count: usize, standing: TrustScore) -> Self {
        assert!(count > 0, "a network holds at least one validator");
        let nodes = (0..count)
            .map(|index| {
                ValidatorNode::new(
                    &format!("validator-{}", index + 1),
                    index as u8 + 1,
                    standing,
                )
            })
            .collect();
        TestNetwork {
            nodes,
            policy: AuthoringPolicy::standard(),
            thresholds: ThresholdTable::new(HARNESS_ROUTINE_PERCENT, 67, 80, 90),
            slots_per_epoch: 6,
            accepted: BTreeMap::new(),
            epoch: 0,
        }
    }

    /// The key registry every node reads.
    pub fn registry(&self) -> KeyRegistry {
        KeyRegistry::from_pairs(self.nodes.iter().map(|node| node.keys()))
    }

    /// The epoch draw output of one validator.
    ///
    /// Each validator evaluates the verifiable random function over the epoch
    /// number and its own draw key. The harness reads that output through the
    /// keystore, exactly as a node reads it before publishing the draw.
    pub fn epoch_output(&self, node: &ValidatorNode) -> [u8; 32] {
        let signature = epoch_draw_from_keystore(&node.keystore, &node.draw_key, self.epoch)
            .expect("the keystore holds the draw key");
        cerulea_primitives::verify_epoch_draw(&node.draw_key, self.epoch, &signature)
            .expect("the published draw checks against the draw key")
    }

    /// The committee drawn for one slot.
    ///
    /// The draw runs through the shared primitives crate, which is the code the
    /// runtime runs. Every node reaches the same order.
    pub fn committee(&self, slot: Slot, parent_hash: [u8; 32]) -> SlotCommittee {
        let seed = derive_slot_seed(&parent_hash, slot).0;
        let candidates: Vec<SeatInput> = self
            .nodes
            .iter()
            .map(|node| SeatInput {
                draw_key: node.draw_key.0,
                epoch_output: self.epoch_output(node),
                standing: node.standing.parts_per_million(),
            })
            .collect();
        SlotCommittee {
            slot,
            members: rank_seats(&candidates, &seed)
                .into_iter()
                .map(|seat| CommitteeMember {
                    validator: sr25519::Public::from_raw(seat.draw_key),
                    rank: Rank(seat.rank),
                    weighted_draw: seat.seat_value,
                })
                .collect(),
        }
    }

    /// The slot context every node builds for one slot.
    pub fn context(&self, slot: Slot, parent_hash: [u8; 32]) -> SlotContext {
        let seed = derive_slot_seed(&parent_hash, slot).0;
        SlotContext::new(
            DrawInput::new(slot, cerulea_consensus::engines::SlotSeed(seed)),
            self.committee(slot, parent_hash),
            SlotSchedule::for_slot(slot, self.policy),
        )
    }

    /// The parent hash every reachable node agrees on.
    pub fn parent_hash(&self) -> [u8; 32] {
        self.nodes
            .iter()
            .find(|node| node.reachable)
            .map(|node| node.head_hash())
            .unwrap_or([0u8; 32])
    }

    /// The height the reachable nodes reached.
    pub fn height(&self) -> u32 {
        self.nodes
            .iter()
            .filter(|node| node.reachable)
            .map(|node| node.height())
            .max()
            .unwrap_or(0)
    }

    /// The number of nodes reaching the network.
    pub fn reachable_count(&self) -> usize {
        self.nodes.iter().filter(|node| node.reachable).count()
    }

    /// Take one node out of the network.
    pub fn partition(&mut self, index: usize) {
        self.nodes[index].reachable = false;
    }

    /// Return one node to the network and bring its chain up to date.
    ///
    /// A returning node fetches the chain the reachable nodes hold. That fetch
    /// is the recovery path a real node walks after a partition heals.
    pub fn restore(&mut self, index: usize) {
        let chain: Vec<TestHeader> = self
            .nodes
            .iter()
            .filter(|node| node.reachable)
            .max_by_key(|node| node.height())
            .map(|node| node.chain.clone())
            .unwrap_or_default();
        let committed: BTreeMap<u32, [u8; 32]> = self
            .nodes
            .iter()
            .filter(|node| node.reachable)
            .max_by_key(|node| node.committed.len())
            .map(|node| node.committed.clone())
            .unwrap_or_default();
        let node = &mut self.nodes[index];
        node.reachable = true;
        node.chain = chain;
        node.committed = committed;
    }

    /// The total Trust Score weight of the whole validator set.
    pub fn total_weight(&self) -> u128 {
        self.nodes.iter().map(|node| node.standing.weight()).sum()
    }

    /// Run one slot.
    ///
    /// The rank holding the slot at each moment authors. A rank belonging to an
    /// unreachable node lets the slot pass to the next rank, which is the rank
    /// timeout the Authoring Engine carries. Every reachable node then checks
    /// the seal. The author collects one signature per reachable node and the
    /// Commit Engine decides whether the domain commits.
    pub fn run_slot(&mut self, slot: Slot) -> SlotOutcome {
        let parent_hash = self.parent_hash();
        let context = self.context(slot, parent_hash);
        let registry = self.registry();
        let schedule = SlotSchedule::for_slot(slot, self.policy);

        for rank_index in 0..self.policy.committee_size {
            let rank = Rank::from_index(rank_index);
            let window = match schedule.window(rank) {
                Some(window) => window,
                None => continue,
            };
            let moment_ms = window.opens_ms;
            let drawn = match context.committee.at_rank(rank) {
                Some(validator) => *validator,
                None => continue,
            };
            let author_index = match self
                .nodes
                .iter()
                .position(|node| node.draw_key == drawn && node.reachable)
            {
                Some(index) => index,
                None => continue,
            };

            let ticket = {
                let gate = AuthorGate::new(self.nodes[author_index].keystore.clone());
                let tracker = FallbackTracker::new(schedule);
                match gate
                    .decide_with_peers(
                        &context,
                        &registry,
                        &tracker,
                        moment_ms,
                        &self.nodes[author_index].peers,
                    )
                    .expect("the keystore answers the gate")
                {
                    GateDecision::Author(ticket) => ticket,
                    GateDecision::StandDown(_) => continue,
                }
            };

            let height = self.height() + 1;
            let mut header = TestHeader::new(
                height,
                Default::default(),
                Default::default(),
                sp_core::H256::from(parent_hash),
                Digest::default(),
            );
            seal_and_apply(
                &self.nodes[author_index].keystore,
                &mut header,
                ticket.seal_claim(),
            )
            .expect("the keystore seals the header");

            let accepted = self.offer(&header, moment_ms, &context, &registry);
            assert!(accepted, "every reachable node accepts an honest block");

            let hash: [u8; 32] = header.hash().into();
            let (signed_weight, signed_percent) = self.collect_votes(slot, hash, moment_ms);
            let committed = self.commit_domain(slot, hash, signed_weight);

            self.accepted.entry(height).or_default().insert(hash);
            for node in self.nodes.iter_mut().filter(|node| node.reachable) {
                node.chain.push(header.clone());
                if committed {
                    node.committed.insert(height, hash);
                }
            }

            return if committed {
                SlotOutcome::Committed {
                    height,
                    hash,
                    rank: rank.0,
                    signed_percent,
                }
            } else {
                SlotOutcome::Withheld {
                    height,
                    signed_percent,
                }
            };
        }

        SlotOutcome::NoAuthor
    }

    /// Offer one sealed header to every reachable node.
    ///
    /// Each node runs the whole Authoring Engine check. Membership, signature
    /// and rank timing.
    pub fn offer(
        &self,
        header: &TestHeader,
        moment_ms: u64,
        context: &SlotContext,
        registry: &KeyRegistry,
    ) -> bool {
        self.nodes
            .iter()
            .filter(|node| node.reachable)
            .all(|_| verify_header(header, context, registry, moment_ms).is_ok())
    }

    /// Check one header the way an importing node checks it.
    pub fn check(
        &self,
        header: &TestHeader,
        slot: Slot,
        parent_hash: [u8; 32],
        moment_ms: u64,
    ) -> Result<(), EngineError> {
        let context = self.context(slot, parent_hash);
        let registry = self.registry();
        verify_header(header, &context, &registry, moment_ms).map(|_| ())
    }

    /// Collect one signature from every reachable node.
    fn collect_votes(&self, slot: Slot, hash: [u8; 32], opens_ms: u64) -> (u128, u8) {
        let set: Vec<(ed25519::Public, TrustScore)> = self
            .nodes
            .iter()
            .map(|node| (node.seal_key, node.standing))
            .collect();
        let mut collector =
            SignatureCollector::open(slot, hash, &set, opens_ms, COLLECTION_WINDOW_MS)
                .expect("the validator set holds members");
        for node in self.nodes.iter().filter(|node| node.reachable) {
            let vote = cast_vote(&node.keystore, &node.seal_key, slot, hash)
                .expect("the keystore signs the vote");
            collector
                .accept(vote, opens_ms)
                .expect("the vote joins the collection");
        }
        assert_eq!(
            collector.signer_count(),
            self.reachable_count(),
            "the collector holds one signature per reachable node"
        );
        let signed = collector.signed_weight();
        let total = self.total_weight();
        let percent = if total == 0 {
            0
        } else {
            ((signed.saturating_mul(100)) / total) as u8
        };
        (signed, percent)
    }

    /// Ask the Commit Engine whether one routine domain commits.
    fn commit_domain(&self, slot: Slot, hash: [u8; 32], _signed_weight: u128) -> bool {
        let mut committer = DomainCommitter::open(slot, hash, self.total_weight(), self.thresholds)
            .expect("the validator set carries weight");
        for node in self.nodes.iter().filter(|node| node.reachable) {
            committer
                .add_signer(node.seal_key, node.standing)
                .expect("the signer joins the tally");
        }
        committer.settle(0, TransactionClass::Routine).committed
    }

    /// Move the network to the next epoch.
    ///
    /// Each validator publishes a fresh draw for the new epoch, so the
    /// committee of every slot in the new epoch rests on a fresh value.
    pub fn advance_epoch(&mut self) {
        self.epoch = self.epoch.saturating_add(1);
    }

    /// The epoch one slot belongs to.
    pub fn epoch_of(&self, slot: Slot) -> u32 {
        (slot.0 / self.slots_per_epoch) as u32
    }

    /// The draw input of one slot.
    pub fn draw_input(&self, slot: Slot, parent_hash: [u8; 32]) -> DrawInput {
        DrawInput::new(slot, derive_slot_seed(&parent_hash, slot))
    }

    /// Produce the slot draw of one validator through its own keystore.
    pub fn keystore_draw(&self, index: usize, input: &DrawInput) -> DrawResult {
        draw_from_keystore(
            &self.nodes[index].keystore,
            &self.nodes[index].draw_key,
            input,
        )
        .expect("the keystore holds the draw key")
    }

    /// Build one header carrying a seal that another node produced.
    ///
    /// The integration tests use this to mount a forged authorship attempt.
    pub fn sealed_header(
        &self,
        signer: usize,
        claim: SealClaim,
        height: u32,
        parent_hash: [u8; 32],
    ) -> TestHeader {
        let mut header = TestHeader::new(
            height,
            Default::default(),
            Default::default(),
            sp_core::H256::from(parent_hash),
            Digest::default(),
        );
        seal_and_apply(&self.nodes[signer].keystore, &mut header, claim)
            .expect("the keystore seals the header");
        header
    }

    /// Build one header whose seal names another validator as the author.
    ///
    /// The signer places its own signature over a claim naming somebody else.
    /// A node that holds no author key cannot produce the author signature, so
    /// this is the strongest forgery a node can mount. Import rejects it.
    pub fn forged_header(
        &self,
        signer: usize,
        claim: SealClaim,
        height: u32,
        parent_hash: [u8; 32],
    ) -> TestHeader {
        use cerulea_consensus::engines::authoring::seal::{pre_seal_hash, seal_payload};
        use sp_keystore::Keystore;

        let header = TestHeader::new(
            height,
            Default::default(),
            Default::default(),
            sp_core::H256::from(parent_hash),
            Digest::default(),
        );
        let hash = pre_seal_hash(&header);
        let payload = seal_payload(hash.as_ref(), &claim);
        let signature = self.nodes[signer]
            .keystore
            .ed25519_sign(CERULEA_KEY_TYPE, &self.nodes[signer].seal_key, &payload)
            .expect("the keystore answers")
            .expect("the keystore holds the signer key");
        let seal = BlockSeal { claim, signature };
        let mut sealed = header;
        sealed.digest_mut().push(seal.digest_item());
        sealed
    }

    /// Report whether two conflicting blocks ever committed at one height.
    pub fn conflicting_commit(&self) -> Option<u32> {
        let mut seen: BTreeMap<u32, [u8; 32]> = BTreeMap::new();
        for node in self.nodes.iter() {
            for (height, hash) in node.committed.iter() {
                match seen.get(height) {
                    Some(held) if held != hash => return Some(*height),
                    _ => {
                        seen.insert(*height, *hash);
                    }
                }
            }
        }
        None
    }

    /// The seal one header carries.
    pub fn seal_of(header: &TestHeader) -> Option<BlockSeal> {
        extract_seal(header)
    }
}

/// One transaction the determinism test runs.
#[derive(Clone, Debug)]
pub struct HarnessTransaction {
    /// The collection the transaction writes.
    pub collection: CollectionId,
    /// The key inside that collection.
    pub key: Vec<u8>,
    /// The value the transaction writes.
    pub value: Vec<u8>,
    /// The class the transaction carries.
    pub class: TransactionClass,
}

impl HarnessTransaction {
    /// Build one transaction over one named collection.
    pub fn new(collection: &str, key: &str, value: &str, class: TransactionClass) -> Self {
        HarnessTransaction {
            collection: CollectionId::from_name(collection),
            key: key.as_bytes().to_vec(),
            value: value.as_bytes().to_vec(),
            class,
        }
    }

    /// The Access Manifest this transaction declares.
    pub fn manifest(&self) -> AccessManifest {
        let access = CollectionAccess::new(self.collection, KeyRange::Exact(self.key.clone()));
        AccessManifest::build(self.class, Vec::new(), vec![access])
            .expect("the manifest holds one write")
    }

    /// The pending entry the partitioner reads.
    pub fn pending(&self, sequence: u64) -> PendingTransaction {
        let hash = sp_core::hashing::blake2_256(
            &[
                self.collection.0.as_slice(),
                self.key.as_slice(),
                self.value.as_slice(),
            ]
            .concat(),
        );
        PendingTransaction::new(hash, sequence, self.manifest(), self.value.clone())
    }
}

/// Run one block of transactions through the Execution Engine.
///
/// The partitioner groups the transactions into conflict domains. The runner
/// executes the domains across the available cores. The roots record one state
/// root per domain beside the combined root.
pub fn execute_block(transactions: &[HarnessTransaction], workers: usize) -> StateRoots {
    let pending: Vec<PendingTransaction> = transactions
        .iter()
        .enumerate()
        .map(|(position, transaction)| transaction.pending(position as u64))
        .collect();
    let domains = DomainPartitioner::new().partition_domains(&pending);
    let runner = DomainRunner::with_workers(workers);
    let mut state = MemoryStateStore::new();
    let report = runner.run(
        &pending,
        &domains,
        &mut state,
        &|transaction: &PendingTransaction,
          guard: &mut cerulea_consensus::engines::manifest_guard::ManifestGuard<
            '_,
            cerulea_consensus::engines::execution::runner::DomainStore<'_>,
        >| {
            let access = transaction
                .manifest
                .writes
                .first()
                .expect("the manifest declares one write");
            let key = match &access.keys {
                KeyRange::Exact(key) => key.to_vec(),
                _ => Vec::new(),
            };
            guard.write(&access.collection, &key, transaction.body.clone())
        },
    );
    StateRoots::from_report(&report)
}
