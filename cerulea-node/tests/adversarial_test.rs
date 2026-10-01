//! Adversarial protocol review of the Cerulea node.
//!
//! Each test mounts one attack against the real engine code and asserts the
//! refusal. The Authoring Engine checks seals, ranks and timing. The Commit
//! Engine bounds vote submission. The propagation layer caps its memory. The
//! Custody Engine checks inclusion proofs. The bridge module binds a proof to
//! one endpoint. The relayer module binds a signature to one statement. The
//! operator module keeps customer state out of reach. The tier module and the
//! Admission Engine check licence signatures and expiry. The Execution Engine
//! holds every call to its Access Manifest and its metered bounds. The Commit
//! Engine counts signer weight from members alone.
//!
//! A few inputs pass one check by design and meet a later check instead. The
//! tests assert that observed behaviour and name it as a documented gap. Four
//! attacks passed an engine when the review began. Each fix sits in the engine
//! and the test here guards it. The list lives in
//! `chain/docs/ADVERSARIAL_REVIEW.md`.
//!
//! | Attack | Tests |
//! |---|---|
//! | 1 Forged seal | `forged_*` |
//! | 2 Replayed seal | `replayed_*` |
//! | 3 Early rank claim | `early_rank_*` |
//! | 4 Vote flooding | `vote_flood_*` |
//! | 5 Gossip exhaustion | `gossip_exhaustion_*` |
//! | 6 Swapped custody proof | `custody_*` |
//! | 7 Cross-rank equivocation | `equivocation_*` |
//! | 8 Bridge proof for another endpoint | `bridge_*` |
//! | 9 Tampered relayer authorisation | `relayer_*` |
//! | 10 Operator reaching customer state | `operator_*` |
//! | 11 Expired or foreign licence | `licence_*` |
//! | 12 Manifest omitting a written collection | `manifest_*` |
//! | 13 Transaction past its metered bound | `transaction_past_*`, `metered_bound_*` |
//! | 14 Signer bitmap claiming a non-member | `domain_commit_*` |

mod support;

use cerulea_consensus::dvf_gossip::admit_vote_message;
use cerulea_consensus::engines::authoring::draw::draw;
use cerulea_consensus::engines::authoring::equivocation::{
    accept_report, check_proof, read_report, sealed_head, verify_equivocation, EquivocationProof,
    EquivocationReport, EquivocationTracker, ReportQueue, SealedHead, DEFAULT_REPORT_CAPACITY,
    DEFAULT_TRACKED_HEADS, DEFAULT_TRACKED_SLOTS, SEAL_CLAIM_BYTES,
};
use cerulea_consensus::engines::authoring::fallback::{FallbackTracker, SlotSchedule};
use cerulea_consensus::engines::authoring::gate::{AuthorGate, GateDecision, StandDown};
use cerulea_consensus::engines::authoring::seal::{
    pre_seal_hash, seal_payload, BlockSeal, SealClaim,
};
use cerulea_consensus::engines::authoring::verify::{
    verify_seal, verify_seal_on_arrival, ARRIVAL_ALLOWANCE_MS,
};
use cerulea_consensus::engines::bounds::{admit, MessagePath};
use cerulea_consensus::engines::commit::aggregate::{
    AggregateSignature, SignerBitmap, SCHEME_ENVELOPE_V1,
};
use cerulea_consensus::engines::commit::collect::{
    cast_vote, CommitVote, SignatureCollector, VoteAdmission,
};
use cerulea_consensus::engines::commit::commit::{DomainCommitter, ThresholdTable};
use cerulea_consensus::engines::commit::gossip::{MessageSet, DEFAULT_MESSAGE_CAPACITY};
use cerulea_consensus::engines::commit::submission::{
    read_and_admit, read_submission, CommitSubmission, SubmissionGate, SubmissionPolicy,
    MAX_SUBMISSION_BYTES, MAX_VOTES_PER_SUBMISSION,
};
use cerulea_consensus::engines::execution::meter::ResourceLimits;
use cerulea_consensus::engines::execution::partition::DomainPartitioner;
use cerulea_consensus::engines::execution::runner::{
    DomainRunner, DomainStore, ExecutionReport, TransactionOutcome,
};
use cerulea_consensus::engines::manifest::PendingTransaction;
use cerulea_consensus::engines::manifest_derive::{
    derive_manifest, AccessSpec, ActionDescription, KeyExpression, ParameterValue,
    TransactionParameters,
};
use cerulea_consensus::engines::manifest_guard::{ManifestGuard, MemoryStateStore, StateAccess};
use cerulea_consensus::engines::{
    CollectionId, EngineError, KeyRange, Rank, Slot, TransactionClass, TrustScore,
};
use cerulea_consensus::{CERULEA_ENGINE_ID, CERULEA_KEY_TYPE};
use cerulea_node::test_harness::{TestHeader, TestNetwork};

use pallet_cerulea_audit::contract::WorkClass;
use pallet_cerulea_bridge::endpoint::{
    AvalancheParameters, EndpointState, ListedHeaders, ValidatorEntry, VerifierParameters,
};
use pallet_cerulea_bridge::envelope::{BridgeFault, MessageEnvelope, MessageFormat, Travel};
use pallet_cerulea_bridge::networks::avalanche::{message_hash, AvalancheProof};
use pallet_cerulea_bridge::verify::{verify as verify_bridge_proof, IndexedSignature, Proof};
use pallet_cerulea_bridge::EndpointDeclaration;
use pallet_cerulea_custody::primitives::{ChallengeAnswer, ChallengeState, ProofStep};
use pallet_cerulea_custody::respond::{assemble_answer, local_record_root};
use pallet_cerulea_custody::verify::{verify_against_root, VerifyError};
use pallet_cerulea_dvf::engines::admission::licence::{
    attestation_payload, licence_holds, verify_attestation, LicenceAttestation, LicenceClass,
    LicenceError,
};
use pallet_cerulea_operator::{OperatorRequest, Outcome};
use pallet_cerulea_relayer::Authorisation;
use pallet_cerulea_schema::primitives::{BoundedName, DeploymentId};
use pallet_cerulea_tier::{EntitlementGate, LicenceControl, LicenceState, ProductionGate};

use cerulea_runtime::{AccountId, Runtime, RuntimeOrigin};
use codec::{Decode, Encode};
use frame_support::traits::{OnFinalize, OnInitialize};
use frame_support::{assert_noop, assert_ok, BoundedVec};
use frame_system::pallet_prelude::BlockNumberFor;
use sp_core::{ed25519, sr25519, Pair, H256};
use sp_keyring::Ed25519Keyring;
use sp_keystore::testing::MemoryKeystore;
use sp_keystore::{Keystore, KeystorePtr};
use sp_runtime::generic::{Digest, DigestItem};
use sp_runtime::traits::Header as HeaderT;
use std::sync::Arc;
use support::*;

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// The slot the authoring attacks run in.
const SLOT: Slot = Slot(3);

/// The validators the authoring attacks run over.
const VALIDATORS: usize = 5;

/// Find the node holding one rank of one slot.
fn node_at_rank(network: &TestNetwork, slot: Slot, parent: [u8; 32], rank: Rank) -> usize {
    let context = network.context(slot, parent);
    let drawn = *context
        .committee
        .at_rank(rank)
        .expect("The committee fills this rank.");
    network
        .nodes
        .iter()
        .position(|node| node.draw_key == drawn)
        .expect("The drawn key belongs to a node.")
}

/// The moment one rank window opens.
fn window_opens(network: &TestNetwork, slot: Slot, rank: Rank) -> u64 {
    SlotSchedule::for_slot(slot, network.policy)
        .window(rank)
        .expect("The rank holds a window.")
        .opens_ms
}

/// The claim one node builds for itself at one rank and one moment.
fn own_claim(
    network: &TestNetwork,
    index: usize,
    slot: Slot,
    parent: [u8; 32],
    rank: Rank,
    moment_ms: u64,
) -> SealClaim {
    let input = network.draw_input(slot, parent);
    let result = network.keystore_draw(index, &input);
    SealClaim::new(slot, rank, network.nodes[index].seal_key, result, moment_ms)
}

/// A bare header at one height over one parent.
fn bare_header(height: u32, parent: [u8; 32], state_root: H256) -> TestHeader {
    <TestHeader as HeaderT>::new(
        height,
        Default::default(),
        state_root,
        H256::from(parent),
        Digest::default(),
    )
}

/// Place one seal on a header built from its parts.
fn with_seal(mut header: TestHeader, seal: BlockSeal) -> TestHeader {
    header.digest_mut().push(seal.digest_item());
    header
}

/// Seal one header with a raw key pair held outside every keystore.
fn sealed_with_pair(pair: &ed25519::Pair, claim: SealClaim, header: TestHeader) -> TestHeader {
    let hash = pre_seal_hash(&header);
    let payload = seal_payload(hash.as_ref(), &claim);
    let signature = pair.sign(&payload);
    with_seal(header, BlockSeal { claim, signature })
}

/// Read the sealed head of one header.
fn head_of(header: &TestHeader) -> SealedHead {
    let seal = TestNetwork::seal_of(header).expect("The header carries a seal.");
    sealed_head(header, &seal).expect("The claim encodes to the stated length.")
}

/// The seal one header carries.
fn seal_of(header: &TestHeader) -> BlockSeal {
    TestNetwork::seal_of(header).expect("The header carries a seal.")
}

/// A keystore holding one fresh seal key outside the validator set.
fn stranger_keystore(seed: &str) -> (KeystorePtr, ed25519::Public) {
    let memory = MemoryKeystore::new();
    let key = memory
        .ed25519_generate_new(CERULEA_KEY_TYPE, Some(seed))
        .expect("The keystore generates a key.");
    (Arc::new(memory), key)
}

/// A third account, holding no role and no appointment.
fn third_party() -> AccountId {
    Ed25519Keyring::Charlie.to_account_id()
}

/// A fourth account, holding no role and no appointment.
fn fourth_party() -> AccountId {
    Ed25519Keyring::Dave.to_account_id()
}

/// Turn one sentence into the bounded reason an extrinsic carries.
fn sentence(text: &str) -> BoundedVec<u8, frame_support::traits::ConstU32<256>> {
    BoundedVec::truncate_from(text.as_bytes().to_vec())
}

/// The block number type of the runtime.
fn block(number: u32) -> BlockNumberFor<Runtime> {
    number
}

// ---------------------------------------------------------------------------
// Attack 1. A forged seal
// ---------------------------------------------------------------------------

/// A node signs a claim naming the scheduled author.
///
/// The forger copies the public draw of the scheduled author and names the
/// scheduled seal key. Membership, rank and key checks pass. The signature
/// check refuses the block. A header altered after an honest seal meets the
/// same refusal.
#[test]
fn forged_seal_signed_by_another_key_fails_import() {
    let network = TestNetwork::with_validators(VALIDATORS);
    let parent = network.parent_hash();
    let scheduled = node_at_rank(&network, SLOT, parent, Rank::FIRST);
    let forger = (scheduled + 1) % VALIDATORS;
    let moment = window_opens(&network, SLOT, Rank::FIRST);

    // The honest header of the scheduled author imports.
    let honest_claim = own_claim(&network, scheduled, SLOT, parent, Rank::FIRST, moment);
    let honest = network.sealed_header(scheduled, honest_claim.clone(), 1, parent);
    network
        .check(&honest, SLOT, parent, moment)
        .expect("The honest header imports.");

    // The forger signs the same claim with its own key.
    let forged = network.forged_header(forger, honest_claim, 1, parent);
    assert_eq!(
        network.check(&forged, SLOT, parent, moment),
        Err(EngineError::SealSignatureRejected { slot: SLOT.0 }),
        "The forged signature fails against the named author key."
    );

    // An honest seal moved onto an altered header fails the same check.
    let altered = with_seal(
        bare_header(1, parent, H256::repeat_byte(0x5a)),
        seal_of(&honest),
    );
    assert_eq!(
        network.check(&altered, SLOT, parent, moment),
        Err(EngineError::SealSignatureRejected { slot: SLOT.0 }),
        "The seal covers the whole header."
    );
}

/// A forger names a seal key that belongs to another validator.
///
/// A committee member claiming its own rank with the seal key of another
/// validator fails the key registry. A key pair outside the set fails the
/// committee check.
#[test]
fn forged_seal_naming_a_foreign_key_or_an_outside_draw_fails_import() {
    let network = TestNetwork::with_validators(VALIDATORS);
    let parent = network.parent_hash();
    let scheduled = node_at_rank(&network, SLOT, parent, Rank::FIRST);
    let second = node_at_rank(&network, SLOT, parent, Rank(2));
    let second_moment = window_opens(&network, SLOT, Rank(2));

    // The rank two member names the seal key of the rank one member.
    let mut claim = own_claim(&network, second, SLOT, parent, Rank(2), second_moment);
    claim.author = network.nodes[scheduled].seal_key;
    let header = network.forged_header(second, claim, 1, parent);
    assert_eq!(
        network.check(&header, SLOT, parent, second_moment),
        Err(EngineError::ValidatorKeysMismatch),
        "The seal key belongs to another draw key."
    );

    // A key pair outside the validator set draws and seals for itself.
    let outside_draw = sr25519::Pair::from_seed(&[0xd7; 32]);
    let outside_seal = ed25519::Pair::from_seed(&[0xe7; 32]);
    let input = network.draw_input(SLOT, parent);
    let moment = window_opens(&network, SLOT, Rank::FIRST);
    let claim = SealClaim::new(
        SLOT,
        Rank::FIRST,
        outside_seal.public(),
        draw(&outside_draw, &input),
        moment,
    );
    let header = sealed_with_pair(&outside_seal, claim, bare_header(1, parent, H256::zero()));
    assert_eq!(
        network.check(&header, SLOT, parent, moment),
        Err(EngineError::AuthorOutsideCommittee { slot: SLOT.0 }),
        "A draw key outside the set holds no seat."
    );
}

// ---------------------------------------------------------------------------
// Attack 2. A replayed seal at a later slot
// ---------------------------------------------------------------------------

/// A committed header returns as the block of a later slot.
///
/// The whole header fails on the slot. A seal rewritten to name the later slot
/// fails on the draw. The draw proof covers the slot and the parent.
#[test]
fn replayed_seal_at_a_later_slot_fails_import() {
    let mut network = TestNetwork::with_validators(VALIDATORS);
    assert!(network.run_slot(Slot(1)).committed(), "Slot one commits.");
    let committed = network.nodes[0].chain[0].clone();
    let seal = seal_of(&committed);
    let later = Slot(2);
    let later_parent = network.parent_hash();
    let later_moment = window_opens(&network, later, Rank::FIRST);

    // The committed header offered as the block of slot two.
    assert_eq!(
        network.check(&committed, later, later_parent, later_moment),
        Err(EngineError::SealSlotMismatch {
            claimed: 1,
            expected: later.0
        }),
        "The seal names the slot it served."
    );

    // The claim rewritten to slot two keeps the old draw and signature.
    let mut moved = seal.clone();
    moved.claim.slot = later;
    moved.claim.moment_ms = later_moment;
    let header = with_seal(bare_header(2, later_parent, H256::zero()), moved);
    assert_eq!(
        network.check(&header, later, later_parent, later_moment),
        Err(EngineError::DrawProofRejected { slot: later.0 }),
        "The draw proof binds the claim to its own slot."
    );
}

/// An honest seal lifted onto a new header for its own slot.
///
/// The lifted seal fails the signature check. The unchanged header passes a
/// second check for its own slot. Documented gap: the seal check holds no
/// memory, so a repeat of one block passes it. The block store keys on the
/// header hash and keeps one copy.
#[test]
fn replayed_seal_on_another_header_fails_and_the_same_header_repeats() {
    let mut network = TestNetwork::with_validators(VALIDATORS);
    let parent = network.parent_hash();
    assert!(network.run_slot(Slot(1)).committed(), "Slot one commits.");
    let committed = network.nodes[0].chain[0].clone();
    let seal = seal_of(&committed);
    let moment = seal.claim.moment_ms;

    let lifted = with_seal(
        bare_header(1, parent, H256::repeat_byte(0x21)),
        seal.clone(),
    );
    assert_eq!(
        network.check(&lifted, Slot(1), parent, moment),
        Err(EngineError::SealSignatureRejected { slot: 1 }),
        "The seal covers the header it came from."
    );

    // Documented gap. The same header checks again for its own slot.
    assert_eq!(
        network.check(&committed, Slot(1), parent, moment),
        Ok(()),
        "The seal check reads the header alone and holds no memory."
    );
}

// ---------------------------------------------------------------------------
// Attack 3. A committee member claiming a lower rank early
// ---------------------------------------------------------------------------

/// The rank two member claims rank one inside the rank one window.
///
/// The committee holds that member at rank two. Import refuses the claimed
/// rank.
#[test]
fn early_rank_downgrade_by_a_standby_member_fails_import() {
    let network = TestNetwork::with_validators(VALIDATORS);
    let parent = network.parent_hash();
    let standby = node_at_rank(&network, SLOT, parent, Rank(2));
    let first_moment = window_opens(&network, SLOT, Rank::FIRST);

    let claim = own_claim(&network, standby, SLOT, parent, Rank::FIRST, first_moment);
    let header = network.sealed_header(standby, claim, 1, parent);
    assert_eq!(
        network.check(&header, SLOT, parent, first_moment),
        Err(EngineError::RankMismatch {
            claimed: 1,
            held: 2
        }),
        "The committee gives that member rank two."
    );
}

/// The rank two member claims its own rank inside the rank one window.
///
/// Import refuses the moment. The same claim inside the rank two window
/// imports. The local gate of the member stands down during the rank one
/// window. Import reads the moment from the signed claim, the same way the
/// import queue does.
#[test]
fn early_rank_claim_inside_the_rank_one_window_fails_import() {
    let network = TestNetwork::with_validators(VALIDATORS);
    let parent = network.parent_hash();
    let standby = node_at_rank(&network, SLOT, parent, Rank(2));
    let schedule = SlotSchedule::for_slot(SLOT, network.policy);
    let first = schedule
        .window(Rank::FIRST)
        .expect("Rank one holds a window.");
    let second = schedule.window(Rank(2)).expect("Rank two holds a window.");

    for early in [first.opens_ms, second.opens_ms - 1] {
        let claim = own_claim(&network, standby, SLOT, parent, Rank(2), early);
        let header = network.sealed_header(standby, claim.clone(), 1, parent);
        assert_eq!(
            network.check(&header, SLOT, parent, claim.moment_ms),
            Err(EngineError::RankTimingRejected {
                rank: 2,
                moment_ms: early
            }),
            "Rank two holds no window at {early}."
        );
    }

    let claim = own_claim(&network, standby, SLOT, parent, Rank(2), second.opens_ms);
    let header = network.sealed_header(standby, claim.clone(), 1, parent);
    assert_eq!(
        network.check(&header, SLOT, parent, claim.moment_ms),
        Ok(()),
        "The same member imports inside its own window."
    );

    let context = network.context(SLOT, parent);
    let registry = network.registry();
    let gate = AuthorGate::new(network.nodes[standby].keystore.clone());
    let tracker = FallbackTracker::new(schedule);
    let early = gate
        .decide(&context, &registry, &tracker, first.opens_ms)
        .expect("The keystore answers the gate.");
    assert_eq!(
        early,
        GateDecision::StandDown(StandDown::KeyAbsent { rank: 1 }),
        "The gate of the standby member stands down in the rank one window."
    );
    let own = gate
        .decide(&context, &registry, &tracker, second.opens_ms)
        .expect("The keystore answers the gate.");
    assert_eq!(
        own.ticket().map(|ticket| ticket.rank),
        Some(Rank(2)),
        "The gate opens in the rank two window."
    );
}

// ---------------------------------------------------------------------------
// Attack 4. Vote flooding at the bound
// ---------------------------------------------------------------------------

/// One aggregate carrying the stated number of signatures and bitmap seats.
fn aggregate_of(seats: usize, signatures: usize) -> AggregateSignature {
    let mut signers = SignerBitmap::new(seats);
    for position in 0..signatures.min(seats) {
        signers.set(position).expect("The position sits inside.");
    }
    AggregateSignature {
        scheme: SCHEME_ENVELOPE_V1,
        slot: Slot(4),
        block_hash: [1u8; 32],
        signers,
        signatures: vec![ed25519::Signature::from_raw([7u8; 64]); signatures],
    }
}

/// A submission at the vote bound passes at the ceiling price.
///
/// One vote past the bound fails. A bitmap naming seats past the bound fails.
/// A submission padded past the byte bound fails before decoding.
#[test]
fn vote_flood_at_the_bound_is_priced_and_one_past_it_is_refused() {
    let bound = MAX_VOTES_PER_SUBMISSION as usize;
    let gate = SubmissionGate::new();

    let at_bound = CommitSubmission::new(aggregate_of(bound, bound));
    let admitted = gate
        .admit(&at_bound)
        .expect("A submission at the bound passes.");
    assert_eq!(admitted.votes, MAX_VOTES_PER_SUBMISSION);
    assert_eq!(
        admitted.weight,
        SubmissionPolicy::standard().ceiling_weight(),
        "The largest submission pays the ceiling price."
    );
    let (_, read_back) =
        read_and_admit(&at_bound.encode()).expect("The wire path admits it as well.");
    assert_eq!(read_back, admitted);

    let one_past = CommitSubmission::new(aggregate_of(bound + 1, bound + 1));
    assert_eq!(
        gate.admit(&one_past),
        Err(EngineError::SubmissionCountAboveLimit {
            count: bound + 1,
            limit: bound
        }),
        "One vote past the bound fails."
    );

    let extra_signature = CommitSubmission::new(aggregate_of(bound, bound + 1));
    assert_eq!(
        gate.admit(&extra_signature),
        Err(EngineError::SubmissionCountAboveLimit {
            count: bound + 1,
            limit: bound
        }),
        "A signature past the bound fails whatever the bitmap says."
    );

    let wide_bitmap = CommitSubmission::new(aggregate_of(4 * bound, 1));
    assert_eq!(
        gate.admit(&wide_bitmap),
        Err(EngineError::SubmissionCountAboveLimit {
            count: 4 * bound,
            limit: bound
        }),
        "A bitmap naming seats past the bound fails."
    );

    let padded = CommitSubmission::new(aggregate_of(1_100, 1_100));
    let bytes = padded.encode();
    assert!(bytes.len() > MAX_SUBMISSION_BYTES);
    assert_eq!(
        gate.admit(&padded),
        Err(EngineError::SubmissionTooLarge {
            bytes: bytes.len(),
            limit: MAX_SUBMISSION_BYTES
        }),
        "The gate refuses on the size first."
    );
    assert!(
        matches!(
            read_submission(&bytes),
            Err(EngineError::MessageAboveBound { .. })
        ),
        "The wire path refuses the padded bytes before decoding."
    );
}

/// A flood of repeated, stranger, forged and late votes moves no weight.
///
/// The collector keeps one vote per member. A forged signature inside an
/// assembled aggregate fails the aggregate check.
#[test]
fn vote_flood_of_repeats_and_strangers_moves_no_weight() {
    let network = TestNetwork::with_validators(VALIDATORS);
    let slot = Slot(9);
    let hash = [0x44u8; 32];
    let opens = 10_000u64;
    let window = 800u64;
    let set: Vec<(ed25519::Public, TrustScore)> = network
        .nodes
        .iter()
        .map(|node| (node.seal_key, node.standing))
        .collect();
    let mut collector =
        SignatureCollector::open(slot, hash, &set, opens, window).expect("The set holds members.");

    let first = cast_vote(
        &network.nodes[0].keystore,
        &network.nodes[0].seal_key,
        slot,
        hash,
    )
    .expect("The keystore signs.");
    assert_eq!(
        collector.accept(first.clone(), opens),
        Ok(VoteAdmission::Taken)
    );
    let weight = collector.signed_weight();

    let position = collector
        .members()
        .iter()
        .position(|member| *member == network.nodes[0].seal_key)
        .expect("The voter sits in the set.");
    for _ in 0..10_000 {
        assert_eq!(
            collector.accept(first.clone(), opens + 1),
            Err(EngineError::VoteRepeated {
                slot: slot.0,
                position
            }),
            "A repeated vote meets a named refusal."
        );
    }

    let (stranger_store, stranger) = stranger_keystore("//Cerulea//adversary//voter");
    for _ in 0..1_000 {
        let vote = cast_vote(&stranger_store, &stranger, slot, hash).expect("The keystore signs.");
        assert_eq!(
            collector.accept(vote, opens + 2),
            Err(EngineError::VoterOutsideSet)
        );
    }

    let stranger_vote =
        cast_vote(&stranger_store, &stranger, slot, hash).expect("The keystore signs.");
    let forged = CommitVote {
        slot,
        block_hash: hash,
        validator: network.nodes[1].seal_key,
        signature: stranger_vote.signature,
    };
    assert_eq!(
        collector.accept(forged, opens + 3),
        Err(EngineError::VoteSignatureRejected)
    );

    let other_block = cast_vote(
        &network.nodes[2].keystore,
        &network.nodes[2].seal_key,
        slot,
        [0x45u8; 32],
    )
    .expect("The keystore signs.");
    assert_eq!(
        collector.accept(other_block, opens + 4),
        Err(EngineError::VoteBlockMismatch { slot: slot.0 })
    );

    let late = cast_vote(
        &network.nodes[3].keystore,
        &network.nodes[3].seal_key,
        slot,
        hash,
    )
    .expect("The keystore signs.");
    assert_eq!(
        collector.accept(late, opens + window),
        Err(EngineError::CollectionWindowClosed {
            slot: slot.0,
            moment_ms: opens + window
        })
    );

    assert_eq!(collector.signer_count(), 1, "One member signed.");
    assert_eq!(
        collector.signed_weight(),
        weight,
        "The flood moved no weight."
    );

    // A forged signature inside an assembled aggregate fails the check.
    let mut full =
        SignatureCollector::open(slot, hash, &set, opens, window).expect("The set holds members.");
    for node in network.nodes.iter() {
        let vote = cast_vote(&node.keystore, &node.seal_key, slot, hash).expect("Signed.");
        full.accept(vote, opens).expect("The vote joins.");
    }
    let mut aggregate = AggregateSignature::assemble(&full).expect("The aggregate assembles.");
    let members = full.members().to_vec();
    assert_eq!(aggregate.verify(&members), Ok(()));
    aggregate.signatures[2] = aggregate.signatures[0];
    assert_eq!(
        aggregate.verify(&members),
        Err(EngineError::AggregateSignatureRejected { position: 2 })
    );
}

// ---------------------------------------------------------------------------
// Attack 5. Gossip set exhaustion
// ---------------------------------------------------------------------------

/// One distinct thirty two byte message identifier.
fn message_key(index: u64) -> [u8; 32] {
    let mut key = [0u8; 32];
    key[..8].copy_from_slice(&index.to_le_bytes());
    key[31] = 0xc3;
    key
}

/// A flood of distinct messages holds the seen set at its capacity.
///
/// The set the node runs evicts the entry that went longest without a hit.
/// A message in steady use survives the flood. An oversized message fails on
/// the byte bound first.
#[test]
fn gossip_exhaustion_holds_the_seen_set_at_capacity() {
    let mut set: MessageSet<[u8; 32]> = MessageSet::new();
    assert_eq!(set.capacity(), DEFAULT_MESSAGE_CAPACITY);
    let hot = message_key(u64::MAX);
    assert!(!set.record(hot));

    let flood = 4 * DEFAULT_MESSAGE_CAPACITY as u64;
    for index in 0..flood {
        assert!(
            !set.record(message_key(index)),
            "Each flooded message is new."
        );
        if index % 64 == 0 {
            assert!(set.record(hot), "The node keeps seeing the message in use.");
        }
        assert!(set.len() <= DEFAULT_MESSAGE_CAPACITY);
    }

    assert_eq!(set.len(), DEFAULT_MESSAGE_CAPACITY);
    assert_eq!(
        set.evictions(),
        flood + 1 - DEFAULT_MESSAGE_CAPACITY as u64,
        "Every message past the capacity evicted one entry."
    );
    assert!(set.holds(&hot), "The message in steady use survives.");
    assert!(set.holds(&message_key(flood - 1)));
    assert!(
        !set.holds(&message_key(0)),
        "The oldest flooded message left."
    );

    let bound = MessagePath::GossipMessage.max_bytes();
    assert!(admit(MessagePath::GossipMessage, bound).is_ok());
    assert!(matches!(
        admit(MessagePath::GossipMessage, bound + 1),
        Err(EngineError::MessageAboveBound { .. })
    ));
}

/// One synthetic sealed head naming one slot and one seal key.
fn synthetic_head(slot: u64, seal_key: u64, pre_seal: u8) -> SealedHead {
    let mut claim = [0u8; SEAL_CLAIM_BYTES];
    claim[..8].copy_from_slice(&slot.to_le_bytes());
    claim[8] = 1;
    claim[9..17].copy_from_slice(&seal_key.to_le_bytes());
    SealedHead::new([pre_seal; 32], claim, [0u8; 64])
}

/// A flood of heads and reports holds the equivocation memory at capacity.
///
/// The tracker and the report queue each carry a ceiling. A repeat report
/// changes nothing.
#[test]
fn gossip_exhaustion_holds_the_equivocation_memory_at_capacity() {
    let mut tracker = EquivocationTracker::new();
    assert_eq!(tracker.capacity(), DEFAULT_TRACKED_HEADS);
    assert_eq!(tracker.window_slots(), DEFAULT_TRACKED_SLOTS);
    let flood = 3 * DEFAULT_TRACKED_HEADS as u64;
    for index in 0..flood {
        assert!(tracker.observe(synthetic_head(1, index, 1)).is_none());
        assert!(tracker.len() <= DEFAULT_TRACKED_HEADS);
    }
    assert_eq!(tracker.len(), DEFAULT_TRACKED_HEADS);
    assert_eq!(tracker.detections(), 0, "Distinct keys raise no evidence.");

    let mut queue = ReportQueue::new();
    let reports = 2 * DEFAULT_REPORT_CAPACITY as u64;
    for index in 0..reports {
        let proof = EquivocationProof::new(
            synthetic_head(index + 1, index, 1),
            synthetic_head(index + 1, index, 2),
        );
        queue.place(proof);
        assert!(queue.len() <= DEFAULT_REPORT_CAPACITY);
    }
    assert_eq!(queue.len(), DEFAULT_REPORT_CAPACITY);
    assert_eq!(queue.refused(), reports - DEFAULT_REPORT_CAPACITY as u64);

    let repeat = EquivocationProof::new(synthetic_head(1, 0, 1), synthetic_head(1, 0, 2));
    assert!(!queue.place(repeat), "A repeat report changes nothing.");
    assert_eq!(queue.len(), DEFAULT_REPORT_CAPACITY);
}

// ---------------------------------------------------------------------------
// Attack 6. A custody answer carrying a proof from a different record
// ---------------------------------------------------------------------------

/// The records one historical block committed, as a validator holds them.
fn custody_records(count: u8) -> Vec<Vec<u8>> {
    (0..count).map(|byte| vec![byte ^ 0x5c; 16]).collect()
}

/// A record paired with the proof of its neighbour fails verification.
///
/// A record moved to the position of another record fails. A proof longer
/// than the tree fails on its length.
#[test]
fn custody_answer_with_a_proof_from_another_record_fails_verification() {
    let records = custody_records(8);
    let root = local_record_root(&records);
    let asked = assemble_answer(&records, 2).expect("The answer assembles.");
    let neighbour = assemble_answer(&records, 5).expect("The answer assembles.");

    assert_eq!(
        verify_against_root(root, 2, asked.value.as_slice(), asked.proof.as_slice()),
        Ok(())
    );
    assert_eq!(
        verify_against_root(root, 2, asked.value.as_slice(), neighbour.proof.as_slice()),
        Err(VerifyError::RootMismatch),
        "The proof of record five folds record two to another root."
    );
    assert_eq!(
        verify_against_root(
            root,
            2,
            neighbour.value.as_slice(),
            neighbour.proof.as_slice()
        ),
        Err(VerifyError::RootMismatch),
        "The leaf hash carries its position."
    );

    let long: Vec<ProofStep> = (0..=pallet_cerulea_custody::primitives::MAX_PROOF_STEPS)
        .map(|_| ProofStep {
            sibling: H256::repeat_byte(3),
            sibling_on_left: false,
        })
        .collect();
    assert_eq!(
        verify_against_root(root, 2, asked.value.as_slice(), &long),
        Err(VerifyError::ProofTooLong)
    );
}

/// The Custody Engine settles a swapped proof as incorrect.
///
/// The verdict stays on chain. The challenge closes, the custody share falls
/// by the fault loss and the fault count rises by one. The honest answer that
/// follows finds no open challenge, so a validator gains no second attempt.
#[test]
fn custody_engine_refuses_a_swapped_proof_on_chain() {
    development_ext().execute_with(|| {
        type Custody = pallet_cerulea_custody::Pallet<Runtime>;
        let records = custody_records(8);
        for record in records.iter() {
            assert_ok!(Custody::commit_record(
                RuntimeOrigin::root(),
                [0u8; 16],
                BoundedVec::truncate_from(vec![7u8; 4]),
                BoundedVec::truncate_from(record.clone()),
            ));
        }
        // The chain commits one entry per record. The entry joins the
        // collection, the key and the digest of the value.
        let records: Vec<Vec<u8>> = records
            .iter()
            .map(|record| {
                pallet_cerulea_custody::custody_entry(&[0u8; 16], &[7u8; 4], Some(record))
            })
            .collect();
        <Custody as OnFinalize<BlockNumberFor<Runtime>>>::on_finalize(block(1));
        assert_eq!(
            Custody::record_root_at(block(1)),
            Some(local_record_root(&records)),
            "The chain holds the root the validator computes."
        );

        frame_system::Pallet::<Runtime>::set_block_number(block(10));
        <Custody as OnInitialize<BlockNumberFor<Runtime>>>::on_initialize(block(10));
        let (id, challenge) = pallet_cerulea_custody::PublishedChallenge::<Runtime>::get()
            .expect("A challenge issued.");
        let index = challenge.record.index;
        let honest = assemble_answer(&records, index).expect("The answer assembles.");
        let other = assemble_answer(&records, (index + 1) % records.len() as u32)
            .expect("The answer assembles.");
        let swapped = ChallengeAnswer {
            value: honest.value.clone(),
            proof: other.proof.clone(),
        };

        let share_before = Custody::custody_share(&challenge.target);
        assert_ok!(Custody::answer_challenge(
            RuntimeOrigin::signed(challenge.target.clone()),
            id,
            swapped
        ));
        assert_eq!(
            pallet_cerulea_custody::ChallengeStates::<Runtime>::get(id),
            Some(ChallengeState::Incorrect),
            "The swapped proof settles as incorrect."
        );
        assert_eq!(
            Custody::custody_faults(&challenge.target),
            1,
            "The incorrect answer records one custody fault."
        );
        assert_eq!(
            Custody::custody_share(&challenge.target),
            share_before.saturating_sub(pallet_cerulea_custody::score::FAULT_LOSS),
            "The fault lowers the custody share sharply."
        );
        assert!(pallet_cerulea_custody::OpenChallenges::<Runtime>::get(id).is_none());

        assert_noop!(
            Custody::answer_challenge(RuntimeOrigin::signed(challenge.target.clone()), id, honest),
            pallet_cerulea_custody::Error::<Runtime>::ChallengeNotFound
        );
    });
}

// ---------------------------------------------------------------------------
// Attack 7. Equivocation across two ranks
// ---------------------------------------------------------------------------

/// One validator seals a rank one header and a rank two header for one slot.
///
/// Import refuses the header at the rank the committee withholds and takes
/// the other. The author signed both, so the refused head joins the
/// equivocation memory and the pair completes the evidence. The shared
/// evidence check accepts the pair, so the report reaches the framework
/// module that resets the standing of the author. A misranked header that the
/// author never signed joins nothing, so nobody frames a validator.
#[test]
fn equivocation_across_two_ranks_is_refused_at_import_and_raises_evidence() {
    let network = TestNetwork::with_validators(VALIDATORS);
    let parent = network.parent_hash();
    let member = node_at_rank(&network, SLOT, parent, Rank(2));
    let first_moment = window_opens(&network, SLOT, Rank::FIRST);
    let second_moment = window_opens(&network, SLOT, Rank(2));
    let registry = network.registry();

    let off_rank = network.sealed_header(
        member,
        own_claim(&network, member, SLOT, parent, Rank::FIRST, first_moment),
        1,
        parent,
    );
    let own_rank = network.sealed_header(
        member,
        own_claim(&network, member, SLOT, parent, Rank(2), second_moment),
        2,
        parent,
    );

    // Import refuses the off-rank header and takes the other.
    assert_eq!(
        network.check(&off_rank, SLOT, parent, first_moment),
        Err(EngineError::RankMismatch {
            claimed: 1,
            held: 2
        })
    );
    assert_eq!(
        network.check(&own_rank, SLOT, parent, second_moment),
        Ok(())
    );

    // The import path keeps both heads under one key and raises the proof.
    let mut tracker = EquivocationTracker::new();
    assert_eq!(
        tracker.observe_refused(&off_rank, &seal_of(&off_rank), &registry),
        Ok(None),
        "The refused head joins the memory."
    );
    let proof = tracker
        .observe(head_of(&own_rank))
        .expect("The second header for the slot completes the evidence.");
    assert_eq!(tracker.detections(), 1);
    assert_eq!(tracker.len(), 1, "The two heads sit under one key.");

    // The evidence stands on its own and travels on the report path.
    let facts = check_proof(&proof).expect("The evidence stands.");
    assert_eq!(facts.slot, SLOT.0);
    assert_eq!(facts.seal_key, network.nodes[member].seal_key.0);
    assert_eq!(facts.draw_key, network.nodes[member].draw_key.0);
    assert!(verify_equivocation(&proof).is_ok());
    let report = EquivocationReport::new(proof).encode();
    let (_, reported) = accept_report(&report).expect("A peer accepts the report.");
    assert_eq!(reported.seal_key, facts.seal_key);
    let mut queue = ReportQueue::new();
    assert!(queue.place(proof), "The report waits for submission.");

    // A stranger signs an off-rank header naming the author keys.
    let (stranger_store, stranger_key) = stranger_keystore("//Cerulea//adversary//framer");
    let mut framed_claim = own_claim(&network, member, SLOT, parent, Rank::FIRST, first_moment);
    framed_claim.author = network.nodes[member].seal_key;
    let framed = with_seal(
        bare_header(3, parent, H256::repeat_byte(0x33)),
        BlockSeal {
            claim: framed_claim.clone(),
            signature: {
                let hash = pre_seal_hash(&bare_header(3, parent, H256::repeat_byte(0x33)));
                stranger_store
                    .ed25519_sign(
                        CERULEA_KEY_TYPE,
                        &stranger_key,
                        &seal_payload(hash.as_ref(), &framed_claim),
                    )
                    .expect("The keystore answers.")
                    .expect("The keystore holds the key.")
            },
        },
    );
    let mut fresh = EquivocationTracker::new();
    assert_eq!(
        fresh.observe_refused(&framed, &seal_of(&framed), &registry),
        Err(EngineError::SealSignatureRejected { slot: SLOT.0 }),
        "A header the author never signed joins nothing."
    );
    assert!(fresh.is_empty());
}

/// One validator seals two headers at its own rank for one slot.
///
/// Both import on their own. The tracker raises the proof and the evidence
/// names the author.
#[test]
fn equivocation_at_one_rank_is_detected_with_evidence() {
    let network = TestNetwork::with_validators(VALIDATORS);
    let parent = network.parent_hash();
    let member = node_at_rank(&network, SLOT, parent, Rank(2));
    let moment = window_opens(&network, SLOT, Rank(2));
    let claim = own_claim(&network, member, SLOT, parent, Rank(2), moment);
    let first = network.sealed_header(member, claim.clone(), 1, parent);
    let second = network.sealed_header(member, claim, 2, parent);

    assert_eq!(network.check(&first, SLOT, parent, moment), Ok(()));
    assert_eq!(network.check(&second, SLOT, parent, moment), Ok(()));

    let mut tracker = EquivocationTracker::new();
    assert!(tracker.observe(head_of(&first)).is_none());
    let proof = tracker
        .observe(head_of(&second))
        .expect("The second head raises the proof.");
    assert_eq!(tracker.detections(), 1);

    let facts = check_proof(&proof).expect("The evidence stands on its own.");
    assert_eq!(facts.slot, SLOT.0);
    assert_eq!(facts.rank, 2);
    assert_eq!(facts.seal_key, network.nodes[member].seal_key.0);
    assert_eq!(facts.draw_key, network.nodes[member].draw_key.0);
}

// ---------------------------------------------------------------------------
// Attack 8. An inbound bridge message with a proof for another endpoint
// ---------------------------------------------------------------------------

/// The four far network validators of the bridge attack.
fn far_validators() -> Vec<ed25519::Pair> {
    (1u8..=4)
        .map(|seed| ed25519::Pair::from_seed(&[seed ^ 0xa0; 32]))
        .collect()
}

/// The endpoint parameters every bridge endpoint of the attack carries.
fn avalanche_params() -> VerifierParameters {
    let validators: Vec<ValidatorEntry> = far_validators()
        .iter()
        .map(|pair| ValidatorEntry {
            key: pair.public().0,
            power: 10,
        })
        .collect();
    VerifierParameters::Avalanche(AvalancheParameters {
        chain_id: [0x3c; 32],
        validators: BoundedVec::truncate_from(validators),
        threshold_percent: 67,
        sender: BoundedVec::truncate_from(vec![0x42; 20]),
    })
}

/// One inbound envelope naming one endpoint.
fn inbound_envelope(deployment: DeploymentId, endpoint: &str) -> MessageEnvelope {
    MessageEnvelope {
        deployment,
        endpoint: name(endpoint),
        source: name("far_network"),
        destination: name("cerulea"),
        party: BoundedVec::truncate_from(vec![0x42; 20]),
        nonce: 1,
        payload: BoundedVec::truncate_from(b"the adversarial payload".to_vec()),
        class: WorkClass::Routine,
        expiry_millis: 1_700_000_600_000,
    }
}

/// The proof every far validator signs for one envelope.
fn avalanche_proof(envelope: &MessageEnvelope) -> Proof {
    let hash = message_hash(7, &[0x3c; 32], &[0x42; 20], &envelope.commitment());
    let signatures: Vec<IndexedSignature> = far_validators()
        .iter()
        .enumerate()
        .map(|(position, pair)| IndexedSignature {
            signer: position as u32,
            signature: pair.sign(&hash).0,
        })
        .collect();
    Proof::Avalanche(AvalancheProof {
        network_id: 7,
        source_chain: [0x3c; 32],
        height: 88,
        signatures: BoundedVec::truncate_from(signatures),
    })
}

/// A proof built for endpoint B fails the verifier for endpoint A.
///
/// The message identifier covers the endpoint name. The commitment covers the
/// identifier. The far validators signed the commitment of endpoint B.
#[test]
fn bridge_proof_for_another_endpoint_fails_the_verifier() {
    let deployment = [0x81u8; 32];
    let params = avalanche_params();
    let headers = ListedHeaders::default();
    let state = EndpointState::new(&params, MessageFormat::CanonicalValue, &headers, 0);

    let for_b = inbound_envelope(deployment, "endpoint_b");
    let proof = avalanche_proof(&for_b);
    let accepted = verify_bridge_proof(&for_b, &proof, &state).expect("The proof holds for B.");
    assert_eq!(accepted.id, for_b.id());

    let for_a = inbound_envelope(deployment, "endpoint_a");
    assert_ne!(for_a.id(), for_b.id());
    assert_ne!(for_a.commitment(), for_b.commitment());
    assert_eq!(
        verify_bridge_proof(&for_a, &proof, &state).map(|held| held.id),
        Err(BridgeFault::SignatureInvalid),
        "The signatures cover the commitment of endpoint B."
    );
}

/// The bridge module declines a proof carried to another endpoint.
///
/// Two endpoints share one verifier set. The replay registry of endpoint A
/// stays empty after the refusal.
#[test]
fn bridge_module_declines_a_proof_carried_to_another_endpoint() {
    development_ext().execute_with(|| {
        type Bridge = pallet_cerulea_bridge::Pallet<Runtime>;
        let deployment = [0x82u8; 32];
        for endpoint in ["endpoint_a", "endpoint_b"] {
            let declaration = EndpointDeclaration::<AccountId> {
                network: name("far_network"),
                direction: Travel::Inbound,
                both_ways: false,
                collection: name("bridged_record"),
                action: name("record_bridged"),
                format: MessageFormat::CanonicalValue,
                class: WorkClass::Routine,
                lifetime_millis: 60_000,
                params: avalanche_params(),
                executor: actor(),
            };
            assert_ok!(Bridge::register_endpoint(
                RuntimeOrigin::root(),
                deployment,
                name(endpoint),
                declaration
            ));
        }

        let for_b = inbound_envelope(deployment, "endpoint_b");
        let proof = avalanche_proof(&for_b);
        let for_a = inbound_envelope(deployment, "endpoint_a");
        assert_noop!(
            Bridge::deliver_message(RuntimeOrigin::signed(outsider()), for_a.clone(), proof),
            pallet_cerulea_bridge::Error::<Runtime>::ProofDeclined
        );
        assert!(!Bridge::has_seen(
            &deployment,
            &name("endpoint_a"),
            &for_a.id()
        ));
        assert!(!Bridge::has_seen(
            &deployment,
            &name("endpoint_b"),
            &for_b.id()
        ));
    });
}

// ---------------------------------------------------------------------------
// Attack 9. A relayer submission with a tampered authorisation
// ---------------------------------------------------------------------------

/// The key a user signs authorisations with.
fn user_signing_pair() -> ed25519::Pair {
    ed25519::Pair::from_seed(&[0x9e; 32])
}

/// Prepare one deployment for relayed submission.
///
/// The user registers, the deployment holds a licence carrying the relay
/// feature, the user registers a signing key, the relayer holds a budget and
/// the tenant holds a quota.
fn relayer_deployment(deployment: DeploymentId) {
    type Relayer = pallet_cerulea_relayer::Pallet<Runtime>;
    claim_deployment(deployment);
    let _ = pallet_cerulea_identity::Pallet::<Runtime>::register_account(
        RuntimeOrigin::signed(actor()),
        actor(),
        [7u8; 32],
    );
    install_licence(deployment);
    assert_ok!(Relayer::register_signing_key(
        RuntimeOrigin::signed(actor()),
        deployment,
        user_signing_pair().public().0
    ));
    for relayer in [outsider(), third_party()] {
        assert_ok!(Relayer::register_sponsor(
            RuntimeOrigin::signed(actor()),
            deployment,
            relayer,
            1_000
        ));
    }
    assert_ok!(Relayer::set_quota(
        RuntimeOrigin::signed(actor()),
        deployment,
        3_600_000,
        1_000
    ));
}

/// One authorisation the user signs.
/// The inner call the user authorises the relayer to submit.
fn relayed_call() -> cerulea_runtime::RuntimeCall {
    cerulea_runtime::RuntimeCall::System(frame_system::Call::remark {
        remark: b"record parcel".to_vec(),
    })
}

fn user_authorisation(deployment: DeploymentId, nonce: u64) -> Authorisation<AccountId> {
    Authorisation {
        deployment,
        actor: actor(),
        action: name("record_parcel"),
        payload_hash: sp_io::hashing::blake2_256(&relayed_call().encode()),
        valid_until: 1_700_000_060_000,
        nonce,
    }
}

/// A relayer alters a signed authorisation before it submits.
///
/// Each altered field breaks the signature. A signature from a foreign key
/// fails the same check. The honest authorisation lands and names the user
/// as the actor.
#[test]
fn relayer_tampered_authorisation_is_refused() {
    development_ext().execute_with(|| {
        type Relayer = pallet_cerulea_relayer::Pallet<Runtime>;
        let deployment = [0x91u8; 32];
        relayer_deployment(deployment);
        let signed = user_authorisation(deployment, 1);
        let signature = user_signing_pair().sign(&signed.message()).0;

        let mut tampered = Vec::new();
        let mut held = signed.clone();
        held.action = name("transfer_parcel");
        tampered.push(held);
        let mut held = signed.clone();
        held.payload_hash = [0x5f; 32];
        tampered.push(held);
        let mut held = signed.clone();
        held.valid_until += 1_000;
        tampered.push(held);
        let mut held = signed.clone();
        held.nonce = 2;
        tampered.push(held);
        for altered in tampered {
            assert_noop!(
                Relayer::submit_on_behalf(
                    RuntimeOrigin::signed(outsider()),
                    deployment,
                    altered,
                    signature,
                    Box::new(relayed_call()),
                ),
                pallet_cerulea_relayer::Error::<Runtime>::SignatureInvalid
            );
        }

        let foreign = ed25519::Pair::from_seed(&[0x9f; 32])
            .sign(&signed.message())
            .0;
        assert_noop!(
            Relayer::submit_on_behalf(
                RuntimeOrigin::signed(outsider()),
                deployment,
                signed.clone(),
                foreign,
                Box::new(relayed_call()),
            ),
            pallet_cerulea_relayer::Error::<Runtime>::SignatureInvalid
        );

        assert_ok!(Relayer::submit_on_behalf(
            RuntimeOrigin::signed(outsider()),
            deployment,
            signed,
            signature,
            Box::new(relayed_call()),
        ));
        let record = pallet_cerulea_relayer::Submissions::<Runtime>::get(deployment, 0)
            .expect("The honest submission landed.");
        assert_eq!(record.actor, actor(), "The user stands as the actor.");
        assert_eq!(record.relayer, outsider());
    });
}

/// A relayer replays an authorisation that already landed.
///
/// The nonce runs once. A second relayer meets the same refusal.
#[test]
fn relayer_replayed_nonce_is_refused() {
    development_ext().execute_with(|| {
        type Relayer = pallet_cerulea_relayer::Pallet<Runtime>;
        let deployment = [0x92u8; 32];
        relayer_deployment(deployment);
        let signed = user_authorisation(deployment, 7);
        let signature = user_signing_pair().sign(&signed.message()).0;

        assert_ok!(Relayer::submit_on_behalf(
            RuntimeOrigin::signed(outsider()),
            deployment,
            signed.clone(),
            signature,
            Box::new(relayed_call()),
        ));
        for relayer in [outsider(), third_party()] {
            assert_noop!(
                Relayer::submit_on_behalf(
                    RuntimeOrigin::signed(relayer),
                    deployment,
                    signed.clone(),
                    signature,
                    Box::new(relayed_call()),
                ),
                pallet_cerulea_relayer::Error::<Runtime>::NonceUsed
            );
        }
        assert_eq!(
            pallet_cerulea_relayer::NextSubmission::<Runtime>::get(deployment),
            1,
            "One submission landed."
        );
    });
}

// ---------------------------------------------------------------------------
// Attack 10. An operator extrinsic touching customer state
// ---------------------------------------------------------------------------

/// The fragment the operator attack installs.
const OPERATOR_FRAGMENT: &str = "188-land-title-registry.json";

/// The collection the operator attack reaches for.
const OPERATOR_COLLECTION: &str = "land_title_registry_parcel";

/// The action that writes the customer record.
const OPERATOR_ACTION: &str = "land_title_registry_record_parcel";

/// An appointed operator and a self-appointed stranger reach for customer
/// state.
///
/// Every request lands with a refusal outcome and the customer record stands
/// unchanged. The call answers with success by design, so the record of the
/// refusal stays. An account outside the role meets `NotAnOperator`.
/// The chain appoints an operator through the root origin. A stranger appoints
/// nobody, and an appointed operator appoints beside it.
#[test]
fn operator_request_touching_customer_state_is_refused_and_recorded() {
    let library = fragment_library();
    assert!(
        !library.is_empty(),
        "The content repository sits beside the chain. Clone it before the run."
    );
    let blueprint = resolve(OPERATOR_FRAGMENT, &library).expect("The fragment resolves.");

    development_ext().execute_with(|| {
        type Operator = pallet_cerulea_operator::Pallet<Runtime>;
        let deployment = install(&blueprint).expect("The Blueprint installs.");
        seed_identity_and_permissions(deployment, &blueprint);

        let action = blueprint
            .actions
            .iter()
            .find(|held| held.id.as_str() == OPERATOR_ACTION)
            .expect("The fragment declares the action.");
        let arguments =
            sample_arguments(action, &blueprint.types).expect("Every parameter takes a value.");
        run_action(deployment, action, &arguments).expect("The registrar records one parcel.");

        let collection = cerulea_blueprint::types::CollectionId::new(OPERATOR_COLLECTION);
        let mut key_value = arguments
            .iter()
            .find(|(held, _)| held == &cerulea_blueprint::types::ParamName::new("survey_number"))
            .map(|(_, value)| value.clone())
            .expect("The call carries a survey number.");
        key_value.canonicalise();
        let record_key: pallet_cerulea_schema::primitives::RecordKey =
            BoundedVec::try_from(key_value.canonical_bytes()).expect("The key fits.");
        let read_record = || {
            pallet_cerulea_state::Pallet::<Runtime>::record(&deployment, &collection, &record_key)
        };
        let read_root =
            || pallet_cerulea_state::Pallet::<Runtime>::collection_root(&deployment, &collection);
        let record_before = read_record().expect("The parcel record stands.");
        let root_before = read_root();

        // The chain appoints the operator. A stranger appoints nobody,
        // itself included, and an appointed operator appoints beside it.
        let appointed = outsider();
        assert_ok!(Operator::appoint_operator(
            RuntimeOrigin::root(),
            deployment,
            appointed.clone()
        ));
        let stranger = fourth_party();
        assert_noop!(
            Operator::appoint_operator(
                RuntimeOrigin::signed(stranger.clone()),
                deployment,
                stranger.clone()
            ),
            pallet_cerulea_operator::Error::<Runtime>::NotAnOperator
        );
        assert_noop!(
            Operator::appoint_operator(
                RuntimeOrigin::signed(actor()),
                deployment,
                stranger.clone()
            ),
            pallet_cerulea_operator::Error::<Runtime>::NotAnOperator
        );
        assert_ok!(Operator::appoint_operator(
            RuntimeOrigin::signed(appointed.clone()),
            deployment,
            stranger.clone()
        ));

        let customer: BoundedName = name(OPERATOR_COLLECTION);
        let reaching = [
            OperatorRequest::WriteRecord {
                collection: customer.clone(),
            },
            OperatorRequest::ReadRecords {
                collection: customer.clone(),
            },
            OperatorRequest::GrantPermission {
                role: name(TEST_ROLE),
            },
            OperatorRequest::ChangeBlueprint {
                blueprint: [5u8; 32],
            },
        ];
        for who in [appointed.clone(), stranger] {
            for request in reaching.iter() {
                let id = Operator::request_count(&deployment);
                assert_ok!(Operator::request(
                    RuntimeOrigin::signed(who.clone()),
                    deployment,
                    request.clone(),
                    sentence("The operator reached past the platform scope.")
                ));
                let held = Operator::request_record(&deployment, id).expect("The record stays.");
                assert_eq!(held.actor, who);
                assert_eq!(
                    held.outcome,
                    Outcome::PermissionDenied,
                    "The permission module refused {:?}.",
                    request
                );
                assert!(!held.reason_text().is_empty());
            }
        }

        assert_eq!(
            read_record(),
            Some(record_before),
            "The record stands unchanged."
        );
        assert_eq!(
            read_root(),
            root_before,
            "The collection root stands unchanged."
        );

        assert_noop!(
            Operator::request(
                RuntimeOrigin::signed(third_party()),
                deployment,
                OperatorRequest::WriteRecord {
                    collection: customer
                },
                sentence("The account holds no appointment.")
            ),
            pallet_cerulea_operator::Error::<Runtime>::NotAnOperator
        );
    });
}

/// A retired operator reaches nothing.
///
/// The appointment record stays with the active flag cleared. Every request
/// meets `NotAnOperator`.
#[test]
fn operator_retired_from_the_role_is_refused() {
    development_ext().execute_with(|| {
        type Operator = pallet_cerulea_operator::Pallet<Runtime>;
        let deployment = [0xb1u8; 32];
        assert_ok!(Operator::appoint_operator(
            RuntimeOrigin::root(),
            deployment,
            outsider()
        ));
        assert_noop!(
            Operator::retire_operator(RuntimeOrigin::signed(actor()), deployment, outsider()),
            pallet_cerulea_operator::Error::<Runtime>::NotAnOperator
        );
        assert_ok!(Operator::retire_operator(
            RuntimeOrigin::root(),
            deployment,
            outsider()
        ));
        assert_noop!(
            Operator::request(
                RuntimeOrigin::signed(outsider()),
                deployment,
                OperatorRequest::WriteRecord {
                    collection: name(OPERATOR_COLLECTION)
                },
                sentence("The retired operator reached for customer data.")
            ),
            pallet_cerulea_operator::Error::<Runtime>::NotAnOperator
        );
        assert_eq!(Operator::request_count(&deployment), 0);
    });
}

// ---------------------------------------------------------------------------
// Attack 11. A licence attestation with an expired or foreign key
// ---------------------------------------------------------------------------

/// The key the control plane signs licences with.
fn control_plane_pair() -> ed25519::Pair {
    ed25519::Pair::from_seed(&[0x11; 32])
}

/// A key that belongs to someone other than the control plane.
fn foreign_pair() -> ed25519::Pair {
    ed25519::Pair::from_seed(&[0x22; 32])
}

/// One licence for one deployment expiring at the stated moment.
fn tier_licence(
    deployment: DeploymentId,
    expires_at: u64,
    sequence: u64,
) -> pallet_cerulea_tier::LicenceOf<Runtime> {
    pallet_cerulea_tier::Licence {
        deployment,
        tier: pallet_cerulea_tier::Tier {
            application: pallet_cerulea_tier::Application::Enterprise,
            level: 1,
        },
        features: BoundedVec::truncate_from(vec![name(pallet_cerulea_relayer::RELAY_FEATURE)]),
        seats: 4,
        addons: BoundedVec::truncate_from(Vec::new()),
        issued_at: 1_600_000_000_000,
        expires_at,
        sequence,
    }
}

/// Record the test actor as the account that installed one deployment.
///
/// A Blueprint install records the installer. The tests below reach the
/// tier and relayer modules alone, so they record the installer directly.
fn claim_deployment(deployment: DeploymentId) {
    pallet_cerulea_logic::InstalledBy::<Runtime>::insert(deployment, actor());
}

/// Install the control plane key of one deployment.
fn tier_deployment(deployment: DeploymentId) {
    claim_deployment(deployment);
    assert_ok!(
        pallet_cerulea_tier::Pallet::<Runtime>::set_control_plane_key(
            RuntimeOrigin::signed(actor()),
            deployment,
            control_plane_pair().public().0
        )
    );
}

/// A licence signed by a foreign key fails the tier module.
///
/// The foreign key holder cannot replace the control plane key either. The
/// licence the control plane signed installs.
#[test]
fn licence_signed_by_a_foreign_key_is_refused_by_the_tier_module() {
    development_ext().execute_with(|| {
        type Tier = pallet_cerulea_tier::Pallet<Runtime>;
        let deployment = [0xa1u8; 32];
        tier_deployment(deployment);
        let licence = tier_licence(deployment, 3_000_000_000_000, 1);

        let foreign = foreign_pair().sign(&licence.message()).0;
        assert_noop!(
            Tier::install_licence(
                RuntimeOrigin::signed(outsider()),
                deployment,
                BoundedVec::truncate_from(licence.encode()),
                foreign
            ),
            pallet_cerulea_tier::Error::<Runtime>::SignatureInvalid
        );
        assert!(pallet_cerulea_tier::Licences::<Runtime>::get(deployment).is_none());

        assert_noop!(
            Tier::set_control_plane_key(
                RuntimeOrigin::signed(outsider()),
                deployment,
                foreign_pair().public().0
            ),
            pallet_cerulea_tier::Error::<Runtime>::ControlPlaneKeyPresent
        );

        let genuine = control_plane_pair().sign(&licence.message()).0;
        assert_ok!(Tier::install_licence(
            RuntimeOrigin::signed(actor()),
            deployment,
            BoundedVec::truncate_from(licence.encode()),
            genuine
        ));
        assert!(<Tier as LicenceControl>::holds(&deployment));
    });
}

/// An expired licence signed by the control plane fails the tier module.
///
/// The signature holds and the expiry has passed. The tier module refuses the
/// attestation on the expiry, so the deployment keeps no licence and no live
/// state. A deployment that lapsed stays lapsed. The same licence carrying a
/// later expiry installs.
#[test]
fn licence_expired_before_installation_is_refused_by_the_tier_module() {
    development_ext().execute_with(|| {
        type Tier = pallet_cerulea_tier::Pallet<Runtime>;
        let deployment = [0xa2u8; 32];
        tier_deployment(deployment);
        let now = Tier::now_millis();

        for expires_at in [now - 1, now] {
            let licence = tier_licence(deployment, expires_at, 1);
            let signature = control_plane_pair().sign(&licence.message()).0;
            assert_noop!(
                Tier::install_licence(
                    RuntimeOrigin::signed(actor()),
                    deployment,
                    BoundedVec::truncate_from(licence.encode()),
                    signature
                ),
                pallet_cerulea_tier::Error::<Runtime>::LicenceLapsed
            );
        }
        assert!(pallet_cerulea_tier::Licences::<Runtime>::get(deployment).is_none());
        assert!(pallet_cerulea_tier::States::<Runtime>::get(deployment).is_none());
        assert!(!<Tier as EntitlementGate>::entitled(
            &deployment,
            pallet_cerulea_relayer::RELAY_FEATURE
        ));

        // A live licence lapses and the deployment stops. An expired
        // attestation carrying a later sequence leaves the stop in place.
        let live = tier_licence(deployment, now + 60_000, 1);
        assert_ok!(Tier::install_licence(
            RuntimeOrigin::signed(actor()),
            deployment,
            BoundedVec::truncate_from(live.encode()),
            control_plane_pair().sign(&live.message()).0
        ));
        assert!(<Tier as ProductionGate>::production_permitted(&deployment));
        pallet_timestamp::Pallet::<Runtime>::set_timestamp(now + 120_000);
        frame_system::Pallet::<Runtime>::set_block_number(block(2));
        <Tier as OnInitialize<BlockNumberFor<Runtime>>>::on_initialize(block(2));
        assert_eq!(
            pallet_cerulea_tier::States::<Runtime>::get(deployment),
            Some(LicenceState::Lapsed),
            "The block hook marks the lapse."
        );
        let stale = tier_licence(deployment, now + 90_000, 2);
        assert_noop!(
            Tier::install_licence(
                RuntimeOrigin::signed(actor()),
                deployment,
                BoundedVec::truncate_from(stale.encode()),
                control_plane_pair().sign(&stale.message()).0
            ),
            pallet_cerulea_tier::Error::<Runtime>::LicenceLapsed
        );
        assert_eq!(
            pallet_cerulea_tier::States::<Runtime>::get(deployment),
            Some(LicenceState::Lapsed),
            "The stale attestation leaves the stop in place."
        );
        assert!(!<Tier as ProductionGate>::production_permitted(&deployment));
    });
}

/// An expired or foreign attestation fails the Admission Engine.
///
/// The signature check runs first. An expired attestation under a foreign key
/// fails on the key. An expired attestation under the control plane key fails
/// on the window.
#[test]
fn licence_expired_or_foreign_is_refused_by_the_admission_engine() {
    let attestation = LicenceAttestation::<[u8; 32], u32> {
        holder: [0x77; 32],
        class: LicenceClass::PublicAccess,
        issued_at: 10,
        expires_at: 100,
        nonce: 3,
    };
    let control = control_plane_pair();
    let payload = attestation_payload(&attestation);
    let genuine = control.sign(&payload);
    let foreign = foreign_pair().sign(&payload);
    let checks_with = |signature: ed25519::Signature| {
        let key = control.public();
        move |bytes: &[u8]| ed25519::Pair::verify(&signature, bytes, &key)
    };

    assert_eq!(
        verify_attestation(
            &attestation,
            50,
            2,
            LicenceClass::PublicAccess,
            checks_with(genuine)
        ),
        Ok(()),
        "The genuine attestation holds inside its window."
    );

    for now in [100u32, 101, 5_000] {
        assert_eq!(
            verify_attestation(
                &attestation,
                now,
                2,
                LicenceClass::PublicAccess,
                checks_with(genuine)
            ),
            Err(LicenceError::Lapsed),
            "The attestation lapsed at block 100."
        );
        assert!(!licence_holds(&attestation, now));
    }

    for now in [50u32, 5_000] {
        assert_eq!(
            verify_attestation(
                &attestation,
                now,
                2,
                LicenceClass::PublicAccess,
                checks_with(foreign)
            ),
            Err(LicenceError::SignatureRejected),
            "A foreign key fails before the window check."
        );
    }

    let mut widened = attestation.clone();
    widened.expires_at = 10_000;
    assert_eq!(
        verify_attestation(
            &widened,
            5_000,
            2,
            LicenceClass::PublicAccess,
            checks_with(genuine)
        ),
        Err(LicenceError::SignatureRejected),
        "An expiry moved after signing breaks the signature."
    );
}

// ---------------------------------------------------------------------------
// Attack 3, continued. A later rank publishing ahead of its window
// ---------------------------------------------------------------------------

/// The rank three member claims its own window and publishes during rank one.
///
/// The claim names a moment inside the rank three window, so the window check
/// over the claimed moment passes on its own. The block arrives while rank one
/// still holds the slot. The Authoring Engine compares the claimed moment with
/// the local clock on arrival and refuses the block. The same block arriving
/// once the rank three window opens imports. A block fetched long after its
/// slot imports, since its claimed moment sits in the past.
#[test]
fn early_rank_three_block_published_in_the_rank_one_window_fails_import() {
    let network = TestNetwork::with_validators(VALIDATORS);
    let parent = network.parent_hash();
    let third = node_at_rank(&network, SLOT, parent, Rank(3));
    let first_opens = window_opens(&network, SLOT, Rank::FIRST);
    let third_opens = window_opens(&network, SLOT, Rank(3));

    let claim = own_claim(&network, third, SLOT, parent, Rank(3), third_opens);
    let header = network.sealed_header(third, claim, 1, parent);
    let seal = seal_of(&header);
    let context = network.context(SLOT, parent);
    let registry = network.registry();

    assert_eq!(
        verify_seal(&header, &seal, &context, &registry, seal.claim.moment_ms)
            .map(|verified| verified.rank),
        Ok(Rank(3)),
        "The claimed moment alone reads as an honest rank three block."
    );

    let too_early = third_opens - ARRIVAL_ALLOWANCE_MS - 1;
    for arrival in [first_opens, too_early] {
        assert_eq!(
            verify_seal_on_arrival(&header, &seal, &context, &registry, arrival)
                .map(|verified| verified.rank),
            Err(EngineError::SealMomentAhead {
                rank: 3,
                claimed_ms: third_opens,
                arrival_ms: arrival,
            }),
            "The block arrived at {arrival} ahead of the window it claims."
        );
    }

    for arrival in [
        third_opens - ARRIVAL_ALLOWANCE_MS,
        third_opens,
        third_opens + 3_600_000,
    ] {
        assert_eq!(
            verify_seal_on_arrival(&header, &seal, &context, &registry, arrival)
                .map(|verified| verified.rank),
            Ok(Rank(3)),
            "The block arriving at {arrival} sits inside or after its window."
        );
    }

    // The allowance stays below one rank timeout, so a later rank never
    // reaches into the window of an earlier rank.
    assert!(ARRIVAL_ALLOWANCE_MS < network.policy.rank_timeout_ms);

    // The import queue runs the arrival check on every block it imports.
    let import_queue = include_str!("../src/cerulea-consensus/src/import_queue.rs");
    assert!(
        import_queue
            .contains("verify_seal_on_arrival(header, &seal, &context, &registry, arrival_ms)"),
        "Block import checks the claimed moment against the local clock."
    );
}

// ---------------------------------------------------------------------------
// Attack 12. A manifest omitting a collection the action writes
// ---------------------------------------------------------------------------

/// The parcel collection the record parcel action declares.
fn parcel_collection() -> CollectionId {
    CollectionId::from_name("parcel")
}

/// The ledger collection the record parcel action writes without declaring.
fn ledger_collection() -> CollectionId {
    CollectionId::from_name("ledger")
}

/// The record parcel action as the Execution Engine reads it.
///
/// The declaration names the parcel collection. The declaration omits the
/// ledger collection that a hostile body then writes.
fn record_parcel_action() -> ActionDescription {
    ActionDescription::new(
        "record_parcel",
        TransactionClass::Routine,
        Vec::new(),
        vec![AccessSpec::new(
            "parcel",
            KeyExpression::parameter("parcel_id"),
        )],
    )
}

/// One call of the record parcel action carrying one body.
///
/// The Access Manifest comes from the action declaration and the parameters,
/// the way admission derives it.
fn record_parcel(parcel_id: &str, arrival: u64, body: &[u8]) -> PendingTransaction {
    let parameters =
        TransactionParameters::new().bind("parcel_id", ParameterValue::Text(parcel_id.to_string()));
    let manifest = derive_manifest(&record_parcel_action(), &parameters)
        .expect("The declaration derives a manifest.");
    let hash = sp_core::hashing::blake2_256(&[parcel_id.as_bytes(), body].concat());
    PendingTransaction::new(hash, arrival, manifest, body.to_vec())
}

/// The parcel key one call writes, read from its manifest.
fn declared_key(transaction: &PendingTransaction) -> Vec<u8> {
    match &transaction
        .manifest
        .writes
        .first()
        .expect("The manifest declares one write.")
        .keys
    {
        KeyRange::Exact(key) => key.to_vec(),
        _ => Vec::new(),
    }
}

/// The ledger key a hostile body reaches for.
fn ledger_key(transaction: &PendingTransaction) -> Vec<u8> {
    [b"entry-".as_slice(), declared_key(transaction).as_slice()].concat()
}

/// The body the Execution Engine runs for each call.
///
/// A declared body writes its parcel. An undeclared body also writes the
/// ledger and passes the refusal up. A body that sets the refusal aside
/// reports success anyway. A spinning body spends computation until the
/// meter stops it. A hoarding body writes past the storage bound. A body
/// that sets the meter aside reports success anyway.
fn parcel_body(
    transaction: &PendingTransaction,
    guard: &mut ManifestGuard<'_, DomainStore<'_>>,
) -> Result<(), EngineError> {
    let parcel = parcel_collection();
    let key = declared_key(transaction);
    match transaction.body.as_slice() {
        b"declared" => guard.write(&parcel, &key, b"surveyed".to_vec()),
        b"undeclared" => {
            guard.write(&parcel, &key, b"surveyed".to_vec())?;
            guard.write(
                &ledger_collection(),
                &ledger_key(transaction),
                b"entry".to_vec(),
            )
        }
        b"undeclared, set aside" => {
            guard.write(&parcel, &key, b"surveyed".to_vec())?;
            let _ = guard.write(
                &ledger_collection(),
                &ledger_key(transaction),
                b"entry".to_vec(),
            );
            Ok(())
        }
        b"spin" => {
            guard.write(&parcel, &key, b"surveyed".to_vec())?;
            loop {
                guard.charge_work(10_000)?;
            }
        }
        b"hoard" => guard.write(&parcel, &key, vec![0x5a; 8_192]),
        b"spin, set aside" => {
            guard.write(&parcel, &key, b"surveyed".to_vec())?;
            let _ = guard.charge_work(1_000_000);
            Ok(())
        }
        _ => Ok(()),
    }
}

/// Run one block of parcel calls through the Execution Engine.
fn run_parcel_block(
    pending: &[PendingTransaction],
    limits: ResourceLimits,
) -> (ExecutionReport, MemoryStateStore) {
    let domains = DomainPartitioner::new().partition_domains(pending);
    let mut state = MemoryStateStore::new();
    let report = DomainRunner::with_workers(2).run_with_limits(
        pending,
        &domains,
        &mut state,
        &parcel_body,
        limits,
    );
    (report, state)
}

/// The outcome of the call at one position of the block.
fn outcome_at(report: &ExecutionReport, member: usize) -> TransactionOutcome {
    report
        .domains
        .iter()
        .flat_map(|domain| domain.outcomes.iter())
        .find(|outcome| outcome.member == member)
        .cloned()
        .expect("Every call carries an outcome.")
}

/// An action writing a collection its manifest omits aborts alone.
///
/// The derived manifest names the parcel collection and omits the ledger. The
/// body writes its parcel, then reaches for the ledger. The Execution Engine
/// refuses the ledger write, rolls back the parcel write of that call and
/// carries on with the rest of the block.
#[test]
fn manifest_omitting_a_written_collection_aborts_the_call() {
    let pending = vec![
        record_parcel("parcel-1", 0, b"declared"),
        record_parcel("parcel-2", 1, b"undeclared"),
    ];
    let manifest = &pending[1].manifest;
    assert!(manifest.permits_write(&parcel_collection(), b"parcel-2"));
    assert!(
        !manifest.permits_write(&ledger_collection(), b"entry-parcel-2"),
        "The manifest omits the ledger collection."
    );

    let (report, state) = run_parcel_block(&pending, ResourceLimits::standard());
    assert!(outcome_at(&report, 0).applied(), "The declared call lands.");
    match outcome_at(&report, 1).abort {
        Some(EngineError::WriteOutsideManifest { key, .. }) => {
            assert_eq!(key, "entry-parcel-2", "The refusal names the ledger key.")
        }
        other => panic!("The undeclared write aborts on the manifest. It read {other:?}."),
    }
    assert_eq!(report.aborted_count(), 1);
    assert!(state.read(&parcel_collection(), b"parcel-1").is_some());
    assert!(
        state.read(&parcel_collection(), b"parcel-2").is_none(),
        "The parcel write of the aborted call rolls back."
    );
    assert!(state
        .read(&ledger_collection(), b"entry-parcel-2")
        .is_none());
}

/// A body that sets the manifest refusal aside still aborts.
///
/// A body running as WebAssembly contract code reads the refusal as a value
/// and may carry on. The guard holds the verdict, so the call aborts with the
/// manifest fault and its parcel write rolls back.
#[test]
fn manifest_refusal_set_aside_by_the_body_still_aborts_the_call() {
    let pending = vec![
        record_parcel("parcel-1", 0, b"declared"),
        record_parcel("parcel-2", 1, b"undeclared, set aside"),
    ];
    let (report, state) = run_parcel_block(&pending, ResourceLimits::standard());
    assert!(outcome_at(&report, 0).applied());
    assert!(
        matches!(
            outcome_at(&report, 1).abort,
            Some(EngineError::WriteOutsideManifest { .. })
        ),
        "The guard verdict aborts the call whatever the body reports."
    );
    assert!(state.read(&parcel_collection(), b"parcel-1").is_some());
    assert!(
        state.read(&parcel_collection(), b"parcel-2").is_none(),
        "The parcel write of the aborted call rolls back."
    );
    assert!(state
        .read(&ledger_collection(), b"entry-parcel-2")
        .is_none());
}

// ---------------------------------------------------------------------------
// Attack 13. A transaction past its metered bound
// ---------------------------------------------------------------------------

/// The resource bounds the metering attacks run under.
fn tight_limits() -> ResourceLimits {
    ResourceLimits::new(50_000, 1024 * 1024, 4_096)
}

/// A call spending past its computation bound or its storage bound aborts.
///
/// The spinning call charges computation until the meter crosses the bound.
/// The hoarding call writes more bytes than the storage bound admits. Each
/// aborts with the fault naming the resource. The declared call beside them
/// lands.
#[test]
fn transaction_past_its_metered_bound_aborts_the_call() {
    let pending = vec![
        record_parcel("parcel-1", 0, b"declared"),
        record_parcel("parcel-3", 1, b"spin"),
        record_parcel("parcel-4", 2, b"hoard"),
    ];
    let (report, state) = run_parcel_block(&pending, tight_limits());
    assert!(outcome_at(&report, 0).applied(), "The declared call lands.");
    match outcome_at(&report, 1).abort {
        Some(EngineError::ComputationLimitCrossed { used, limit }) => {
            assert_eq!(limit, 50_000);
            assert!(used > limit, "The meter stopped the call past its bound.");
        }
        other => panic!("The spinning call aborts on computation. It read {other:?}."),
    }
    match outcome_at(&report, 2).abort {
        Some(EngineError::StorageLimitCrossed { used, limit }) => {
            assert_eq!(limit, 4_096);
            assert!(used > limit, "The meter stopped the call past its bound.");
        }
        other => panic!("The hoarding call aborts on storage. It read {other:?}."),
    }
    assert_eq!(report.aborted_count(), 2);
    assert!(state.read(&parcel_collection(), b"parcel-1").is_some());
    assert!(state.read(&parcel_collection(), b"parcel-3").is_none());
    assert!(state.read(&parcel_collection(), b"parcel-4").is_none());
}

/// A body that sets the meter refusal aside still aborts.
///
/// The body charges past the computation bound, reads the refusal as a value
/// and reports success. The guard holds the verdict, so the call aborts with
/// the computation fault and its parcel write rolls back.
#[test]
fn metered_bound_set_aside_by_the_body_still_aborts_the_call() {
    let pending = vec![
        record_parcel("parcel-1", 0, b"declared"),
        record_parcel("parcel-5", 1, b"spin, set aside"),
    ];
    let (report, state) = run_parcel_block(&pending, tight_limits());
    assert!(outcome_at(&report, 0).applied());
    assert!(
        matches!(
            outcome_at(&report, 1).abort,
            Some(EngineError::ComputationLimitCrossed { limit: 50_000, .. })
        ),
        "The guard verdict aborts the call whatever the body reports."
    );
    assert!(
        state.read(&parcel_collection(), b"parcel-5").is_none(),
        "The parcel write of the aborted call rolls back."
    );
}

// ---------------------------------------------------------------------------
// Attack 14. A domain commit whose signer bitmap claims a non-member
// ---------------------------------------------------------------------------

/// The slot the commit attack runs in.
const COMMIT_SLOT: Slot = Slot(40);

/// The block the commit attack signs.
const COMMIT_BLOCK: [u8; 32] = [0x44; 32];

/// One validator set with its keystores in the order the collector holds.
struct CommitSet {
    keystores: Vec<KeystorePtr>,
    members: Vec<ed25519::Public>,
    weights: Vec<TrustScore>,
}

/// Build a validator set of the stated size, each at half standing.
fn commit_set(count: usize) -> CommitSet {
    let mut stores = Vec::new();
    let mut keys = Vec::new();
    for index in 0..count {
        let (store, key) = stranger_keystore(&format!("//Cerulea//adversarial//commit//{index}"));
        stores.push(store);
        keys.push(key);
    }
    let set: Vec<(ed25519::Public, TrustScore)> = keys
        .iter()
        .map(|key| (*key, TrustScore::from_percent(50)))
        .collect();
    let collector = SignatureCollector::open(COMMIT_SLOT, COMMIT_BLOCK, &set, 0, 1_000)
        .expect("The collector opens over the set.");
    let members = collector.members().to_vec();
    let keystores = members
        .iter()
        .map(|member| {
            let position = keys
                .iter()
                .position(|key| key == member)
                .expect("The member sits in the set.");
            stores[position].clone()
        })
        .collect();
    CommitSet {
        keystores,
        members,
        weights: vec![TrustScore::from_percent(50); count],
    }
}

/// The signatures of the members at the stated positions.
fn member_signatures(set: &CommitSet, positions: &[usize]) -> Vec<ed25519::Signature> {
    positions
        .iter()
        .map(|position| {
            cast_vote(
                &set.keystores[*position],
                &set.members[*position],
                COMMIT_SLOT,
                COMMIT_BLOCK,
            )
            .expect("The member signs.")
            .signature
        })
        .collect()
}

/// One envelope over the stated bitmap and signatures.
fn commit_envelope(
    signers: SignerBitmap,
    signatures: Vec<ed25519::Signature>,
) -> AggregateSignature {
    AggregateSignature {
        scheme: SCHEME_ENVELOPE_V1,
        slot: COMMIT_SLOT,
        block_hash: COMMIT_BLOCK,
        signers,
        signatures,
    }
}

/// A fresh tally over the set.
fn commit_tally(set: &CommitSet) -> DomainCommitter {
    DomainCommitter::over_set(
        COMMIT_SLOT,
        COMMIT_BLOCK,
        &set.weights,
        ThresholdTable::standard(),
    )
    .expect("The set carries weight.")
}

/// An aggregate whose bitmap claims a seat outside the validator set.
///
/// Two of four members sign, which holds half the weight and stays below the
/// routine share. A stranger signs the same vote and the author widens the
/// bitmap to a fifth seat to carry the extra signature. The Commit Engine
/// refuses the envelope before it counts any weight. A bitmap carrying a bit
/// past its covered positions meets the same refusal. A stranger signature
/// placed on the seat of a member that stayed silent fails its signature
/// check. The honest aggregate of the same two members counts and stays short
/// of the share.
#[test]
fn domain_commit_bitmap_claiming_a_non_member_is_refused() {
    let set = commit_set(4);
    let (stranger_store, stranger) = stranger_keystore("//Cerulea//adversarial//phantom");
    let phantom = cast_vote(&stranger_store, &stranger, COMMIT_SLOT, COMMIT_BLOCK)
        .expect("The stranger signs.")
        .signature;
    let with_phantom = || {
        let mut signatures = member_signatures(&set, &[0, 1]);
        signatures.push(phantom);
        signatures
    };

    // A fifth seat past the set.
    let mut widened = SignerBitmap::new(5);
    for position in [0usize, 1, 4] {
        widened
            .set(position)
            .expect("The widened bitmap covers five seats.");
    }
    let mut tally = commit_tally(&set);
    assert_eq!(
        tally.absorb_aggregate(
            &commit_envelope(widened, with_phantom()),
            &set.members,
            &set.weights
        ),
        Err(EngineError::AggregateBitmapMalformed),
        "The bitmap claims a seat the validator set lacks."
    );
    assert_eq!(tally.signed_weight(), 0, "The refusal counts no weight.");
    assert!(!tally.settle(0, TransactionClass::Routine).committed);

    // A bit past the covered positions inside the last byte.
    let padded = SignerBitmap::decode(&mut &(4u32, vec![0b0001_0011u8]).encode()[..])
        .expect("The bitmap decodes.");
    assert!(!padded.well_formed());
    let mut tally = commit_tally(&set);
    assert_eq!(
        tally.absorb_aggregate(
            &commit_envelope(padded, with_phantom()),
            &set.members,
            &set.weights
        ),
        Err(EngineError::AggregateBitmapMalformed),
        "The bitmap carries a bit past the covered positions."
    );
    assert_eq!(tally.signed_weight(), 0);

    // A stranger signature on the seat of a silent member.
    let mut borrowed = SignerBitmap::new(4);
    for position in [0usize, 1, 2] {
        borrowed
            .set(position)
            .expect("The bitmap covers four seats.");
    }
    let mut tally = commit_tally(&set);
    assert_eq!(
        tally.absorb_aggregate(
            &commit_envelope(borrowed, with_phantom()),
            &set.members,
            &set.weights
        ),
        Err(EngineError::AggregateSignatureRejected { position: 2 }),
        "The silent member never signed."
    );
    assert_eq!(tally.signed_weight(), 0);

    // The honest aggregate of the two members counts and stays short.
    let mut honest = SignerBitmap::new(4);
    for position in [0usize, 1] {
        honest.set(position).expect("The bitmap covers four seats.");
    }
    let mut tally = commit_tally(&set);
    assert_eq!(
        tally.absorb_aggregate(
            &commit_envelope(honest, member_signatures(&set, &[0, 1])),
            &set.members,
            &set.weights
        ),
        Ok(2)
    );
    assert_eq!(tally.signed_percent(), 50);
    assert!(
        !tally.settle(0, TransactionClass::Routine).committed,
        "Half the weight stays below the routine share."
    );
}

// ---------------------------------------------------------------------------
// Review observation P-01. Every network path runs the shared size check
// ---------------------------------------------------------------------------

/// A block seal padded past its bound reads as no seal.
///
/// The seal claim and the signature decode from the front of the item, so
/// padding once slipped through to the decoder. The shared check now refuses
/// the item on its length and import reports the missing seal.
#[test]
fn network_path_block_seal_refuses_a_padded_seal() {
    let mut network = TestNetwork::with_validators(VALIDATORS);
    let parent = network.parent_hash();
    assert!(network.run_slot(Slot(1)).committed(), "Slot one commits.");
    let honest = network.nodes[0].chain[0].clone();
    let seal = seal_of(&honest);
    let moment = seal.claim.moment_ms;
    assert_eq!(network.check(&honest, Slot(1), parent, moment), Ok(()));

    let mut padded_bytes = seal.encode();
    padded_bytes.resize(MessagePath::BlockSeal.max_bytes() + 1, 0);
    let mut padded = honest.clone();
    padded.digest_mut().logs.retain(
        |item| !matches!(item, DigestItem::Seal(engine, _) if *engine == CERULEA_ENGINE_ID),
    );
    padded
        .digest_mut()
        .push(DigestItem::Seal(CERULEA_ENGINE_ID, padded_bytes));
    assert!(TestNetwork::seal_of(&padded).is_none());
    assert_eq!(
        network.check(&padded, Slot(1), parent, moment),
        Err(EngineError::SealAbsent),
        "The padded seal fails on its length."
    );
}

/// Each path the node decodes refuses one byte past its bound.
///
/// The commit vote and the propagation message share one entry point. The
/// commit submission and the equivocation report each read through the shared
/// check. The extrinsic body meets the block length the runtime sets from the
/// same table.
#[test]
fn network_path_node_entries_refuse_one_byte_past_the_bound() {
    let vote = vec![0u8; MessagePath::CommitVote.max_bytes()];
    assert!(
        admit_vote_message(&vote).is_ok(),
        "A vote at its bound passes."
    );
    let vote = vec![0u8; MessagePath::CommitVote.max_bytes() + 1];
    assert!(matches!(
        admit_vote_message(&vote),
        Err(EngineError::MessageAboveBound { .. })
    ));
    let propagation = vec![0u8; MessagePath::GossipMessage.max_bytes() + 1];
    assert!(matches!(
        admit_vote_message(&propagation),
        Err(EngineError::MessageAboveBound { .. })
    ));

    let submission = vec![0u8; MessagePath::CommitSubmission.max_bytes() + 1];
    assert!(matches!(
        read_submission(&submission),
        Err(EngineError::MessageAboveBound { .. })
    ));

    let report = vec![0u8; MessagePath::EquivocationReport.max_bytes() + 1];
    assert!(matches!(
        read_report(&report),
        Err(EngineError::EquivocationReportTooLarge { .. })
    ));

    development_ext().execute_with(|| {
        let info = frame_support::dispatch::DispatchInfo::default();
        let bound = MessagePath::ExtrinsicBody.max_bytes();
        assert!(frame_system::CheckWeight::<Runtime>::do_validate(&info, bound).is_ok());
        assert!(
            frame_system::CheckWeight::<Runtime>::do_validate(&info, bound + 1).is_err(),
            "The transaction pool refuses a body past the bound."
        );
    });
}

/// The runtime paths run the shared check before any other work.
///
/// The custody answer, the bridge proof and the Blueprint artefact each carry
/// field bounds that sit inside the path bound. The shared check still runs
/// first on each path, so a later change to a field bound cannot open the
/// path. The widest custody answer fits and reaches the challenge lookup.
#[test]
fn network_path_runtime_entries_run_the_shared_check() {
    let widest = ChallengeAnswer {
        value: BoundedVec::truncate_from(vec![
            7u8;
            pallet_cerulea_custody::primitives::MAX_RECORD_BYTES
                as usize
        ]),
        proof: BoundedVec::truncate_from(vec![
            ProofStep {
                sibling: H256::repeat_byte(3),
                sibling_on_left: true,
            };
            pallet_cerulea_custody::primitives::MAX_PROOF_STEPS
                as usize
        ]),
    };
    assert!(widest.encoded_size() <= MessagePath::CustodyAnswer.max_bytes());
    development_ext().execute_with(|| {
        assert_noop!(
            pallet_cerulea_custody::Pallet::<Runtime>::answer_challenge(
                RuntimeOrigin::signed(actor()),
                u64::MAX,
                widest
            ),
            pallet_cerulea_custody::Error::<Runtime>::ChallengeNotFound
        );
    });

    let blueprint_bound =
        <<Runtime as pallet_cerulea_logic::Config>::MaxBlueprintBytes as frame_support::traits::Get<u32>>::get();
    assert!(blueprint_bound as usize <= MessagePath::BlueprintArtefact.max_bytes());

    let custody = include_str!("../../cerulea-pallets/pallet-cerulea-custody/src/lib.rs");
    assert!(custody.contains("MessagePath::CustodyAnswer"));
    let bridge = include_str!("../../cerulea-pallets/pallet-cerulea-bridge/src/inbound.rs");
    assert!(bridge.contains("MessagePath::BridgeProof"));
    let logic = include_str!("../../cerulea-pallets/pallet-cerulea-logic/src/lib.rs");
    assert!(logic.contains("MessagePath::BlueprintArtefact"));
    let service = include_str!("../src/service.rs");
    assert!(service.contains("MessagePath::RpcBody"));
}
