//! The import queue and the proposer factory, run against the real client.
//!
//! The import queue of the Dynamic Consensus Framework reads the slot
//! committee, the validator key records and the active set from the chain.
//! The proposer factory builds blocks through the block builder interface
//! of the real runtime. Every test here drives both against the fixture
//! chain, whose blocks carry seals from the development validator keys.

use std::time::Duration;

use cerulea_consensus::engines::authoring::seal::{seal_header, strip_seal, BlockSeal};
use cerulea_consensus::import_queue::DcfImportQueue;
use cerulea_consensus::{ConsensusError, ProposerFactory, CERULEA_ENGINE_ID};
use cerulea_runtime::opaque::Block;
use codec::Encode;
use sc_consensus::{BlockCheckParams, BlockImport, BlockImportParams, ImportResult, Verifier};
use sp_core::H256;
use sp_keyring::Ed25519Keyring;
use sp_runtime::traits::{Block as BlockT, Header as HeaderT};
use sp_runtime::DigestItem;

use crate::chain_fixture_tests::{alice, fixture, seal_key, Backend, Pool, ViewAt};
use crate::service::FullClient;

type Queue = DcfImportQueue<Block, FullClient, Backend>;

fn queue() -> Queue {
    DcfImportQueue::new(fixture().client.clone())
}

/// A copy of one fixture header carrying a second seal for the same slot.
///
/// The copy names another state root, so the two headers differ while the
/// seal claims the same slot, the same rank and the same seal key.
fn rival_of(number: usize) -> (<Block as BlockT>::Header, BlockSeal) {
    let chain = fixture();
    let built = &chain.blocks[number];
    let mut rival = strip_seal(&built.header);
    rival.set_state_root(H256::repeat_byte(0x5a));
    let seal = seal_header(&chain.keystore, &rival, built.seal.claim.clone())
        .expect("the keystore seals the rival header");
    let mut sealed = rival;
    sealed.digest_mut().push(seal.digest_item());
    (sealed, seal)
}

#[test]
fn every_fixture_block_passes_the_seal_checks() {
    let chain = fixture();
    let queue = queue();
    for built in chain.blocks.iter() {
        let authorship = queue
            .verify_authorship(&built.header)
            .unwrap_or_else(|reason| panic!("block {} fails: {reason}", built.number));
        assert_eq!(authorship.author, alice());
        assert_eq!(authorship.slot, built.seal.claim.slot);
        assert_eq!(authorship.rank, 1);
    }
    // Honest blocks at distinct slots raise no equivocation.
    assert_eq!(queue.equivocation_count(), 0);
    assert!(queue.equivocation_reports().is_empty());
}

#[test]
fn a_queue_reporting_to_the_metrics_registry_passes_the_same_block() {
    let chain = fixture();
    let metrics = chain
        .metrics
        .clone()
        .expect("the fixture registry carries the consensus metrics");
    let queue: Queue = DcfImportQueue::new_with_metrics(chain.client.clone(), metrics);
    let authorship = queue
        .verify_authorship(&chain.blocks[4].header)
        .expect("the sealed block passes");
    assert_eq!(authorship.author, alice());
}

#[test]
fn a_header_without_a_seal_names_the_author_key_as_the_fix() {
    let chain = fixture();
    let unsealed = strip_seal(&chain.blocks[2].header);
    let reason = queue()
        .verify_authorship(&unsealed)
        .expect_err("an unsealed header fails");
    assert_eq!(
        reason,
        "Block 3 carries no Cerulea seal. Fix: produce the block on a node holding the author key for that slot."
    );
}

#[test]
fn a_block_whose_parent_the_node_lacks_names_the_import_order() {
    let chain = fixture();
    let mut orphan = chain.blocks[0].header.clone();
    orphan.set_parent_hash(H256::repeat_byte(0xab));
    let reason = queue()
        .verify_authorship(&orphan)
        .expect_err("the parent state is absent");
    assert!(
        reason.starts_with("Block 1 arrived before its parent"),
        "{reason}"
    );
    assert!(reason.ends_with("Fix: import the parent first, which sync does on its next request."));
}

#[test]
fn a_seal_from_a_key_outside_the_committee_fails_the_seal_checks() {
    let chain = fixture();
    let built = &chain.blocks[5];
    let mut claim = built.seal.claim.clone();
    claim.author = Ed25519Keyring::Bob.public();
    let keystore = sp_keystore::testing::MemoryKeystore::new();
    sp_keystore::Keystore::ed25519_generate_new(
        &keystore,
        cerulea_consensus::CERULEA_KEY_TYPE,
        Some("//Bob"),
    )
    .unwrap();
    let keystore: sp_keystore::KeystorePtr = std::sync::Arc::new(keystore);
    let mut forged = strip_seal(&built.header);
    let seal = seal_header(&keystore, &forged, claim).unwrap();
    forged.digest_mut().push(seal.digest_item());
    let reason = queue()
        .verify_authorship(&forged)
        .expect_err("a seal key the chain never recorded fails");
    assert!(!reason.contains("carries no Cerulea seal"), "{reason}");
}

#[test]
fn two_headers_sealed_for_one_slot_complete_an_equivocation_proof() {
    let chain = fixture();
    let queue = queue();
    let built = &chain.blocks[2];
    queue.observe_for_equivocation(&built.header, &built.seal);
    assert_eq!(queue.equivocation_count(), 0);

    let (rival, rival_seal) = rival_of(2);
    queue.observe_for_equivocation(&rival, &rival_seal);
    assert_eq!(queue.equivocation_count(), 1);
    let held = queue.equivocation_reports();
    assert_eq!(held.len(), 1);

    let taken = queue.take_equivocation_reports();
    assert_eq!(taken.len(), 1);
    assert!(queue.equivocation_reports().is_empty());
    assert_eq!(queue.equivocation_count(), 1);
}

#[test]
fn the_seal_checks_record_an_equivocation_on_the_second_header() {
    let chain = fixture();
    let queue = queue();
    queue
        .verify_authorship(&chain.blocks[3].header)
        .expect("the first header passes");
    let (rival, _) = rival_of(3);
    let authorship = queue
        .verify_authorship(&rival)
        .expect("the rival header carries a valid seal");
    assert_eq!(authorship.slot, chain.blocks[3].seal.claim.slot);
    assert_eq!(queue.equivocation_count(), 1);
    assert_eq!(queue.equivocation_reports().len(), 1);
}

#[test]
fn a_malformed_equivocation_report_is_refused() {
    let queue = queue();
    assert!(queue.accept_equivocation_report(&[0u8; 3]).is_err());
    assert!(queue.accept_equivocation_report(&[]).is_err());
    assert_eq!(queue.equivocation_count(), 0);
}

#[test]
fn the_import_counters_start_at_zero_and_reset() {
    let mut queue = queue();
    let before = format!("{:?}", queue.get_metrics());
    queue.reset_metrics();
    assert_eq!(format!("{:?}", queue.get_metrics()), before);
}

#[tokio::test]
async fn the_verifier_and_the_block_check_admit_a_sealed_block() {
    let chain = fixture();
    let queue = queue();
    let built = &chain.blocks[6];

    let params = BlockImportParams::new(
        sp_consensus::BlockOrigin::NetworkBroadcast,
        built.header.clone(),
    );
    let verified = Verifier::verify(&queue, params)
        .await
        .expect("the sealed block passes the verifier");
    assert!(matches!(
        verified.fork_choice,
        Some(sc_consensus::ForkChoiceStrategy::LongestChain)
    ));

    let unsealed = BlockImportParams::new(
        sp_consensus::BlockOrigin::NetworkBroadcast,
        strip_seal(&built.header),
    );
    let reason = match Verifier::verify(&queue, unsealed).await {
        Ok(_) => panic!("the verifier refuses an unsealed block"),
        Err(held) => held,
    };
    assert!(reason.contains("carries no Cerulea seal"));

    let checked = BlockImport::check_block(
        &queue,
        BlockCheckParams {
            hash: built.hash,
            number: built.number,
            parent_hash: *built.header.parent_hash(),
            allow_missing_state: false,
            allow_missing_parent: false,
            import_existing: false,
        },
    )
    .await
    .expect("the block check runs");
    assert!(matches!(checked, ImportResult::Imported(aux) if aux.is_new_best));

    let imported = BlockImport::import_block(
        &queue,
        BlockImportParams::new(
            sp_consensus::BlockOrigin::NetworkBroadcast,
            built.header.clone(),
        ),
    )
    .await
    .expect("the sealed block passes import");
    assert!(matches!(imported, ImportResult::Imported(aux) if aux.is_new_best));

    let refused = BlockImport::import_block(
        &queue,
        BlockImportParams::new(
            sp_consensus::BlockOrigin::NetworkBroadcast,
            strip_seal(&built.header),
        ),
    )
    .await;
    assert!(refused.is_err());

    let stale: DcfImportQueue<Block, ViewAt, Backend> = DcfImportQueue::new(ViewAt::unknown());
    let failed = BlockImport::check_block(
        &stale,
        BlockCheckParams {
            hash: built.hash,
            number: built.number,
            parent_hash: H256::repeat_byte(0xab),
            allow_missing_state: false,
            allow_missing_parent: false,
            import_existing: false,
        },
    )
    .await;
    assert!(failed.is_err());
}

fn factory(min_block_time: Duration) -> ProposerFactory<Block, FullClient, Pool> {
    let chain = fixture();
    ProposerFactory::new(chain.client.clone(), chain.pool.clone(), min_block_time, 64)
}

#[tokio::test]
async fn the_proposer_reads_the_scheduled_author_from_the_runtime() {
    let chain = fixture();
    let parent = chain.blocks[0].hash;
    let mut factory = factory(Duration::ZERO);
    let (block, author) = factory
        .create_block_with_transactions(parent, 77)
        .await
        .expect("the proposer builds on block one");
    assert_eq!(author, seal_key());
    assert_eq!(*block.header().number(), 2);
    assert_eq!(*block.header().parent_hash(), parent);
    assert_ne!(*block.header().state_root(), H256::zero());
    // The block carries the timestamp inherent at least.
    assert!(!block.extrinsics().is_empty());
}

#[tokio::test]
async fn the_proposer_places_the_author_digest_on_the_header() {
    let chain = fixture();
    let mut factory = factory(Duration::ZERO);
    let item = DigestItem::PreRuntime(CERULEA_ENGINE_ID, seal_key().encode());
    let (block, author) = factory
        .create_block_with_transactions_and_digest(chain.blocks[1].hash, 78, Some(item.clone()))
        .await
        .expect("the proposer builds on block two");
    assert_eq!(author, seal_key());
    assert!(block.header().digest().logs().contains(&item));
}

#[tokio::test]
async fn the_emergency_block_carries_the_inherents_alone() {
    let chain = fixture();
    let mut factory = factory(Duration::ZERO);
    let (block, author) = factory
        .create_emergency_block(chain.blocks[2].hash, 79, seal_key())
        .await
        .expect("the proposer builds the emergency block");
    assert_eq!(author, seal_key());
    assert_eq!(*block.header().number(), 4);
    assert_eq!(block.extrinsics().len(), 1);
    assert!(block.header().digest().logs().is_empty());
}

#[tokio::test]
async fn the_proposer_holds_the_minimum_block_time() {
    let chain = fixture();
    let mut factory = factory(Duration::from_secs(3_600));
    factory
        .create_complete_block(chain.blocks[3].hash, 80, None, Some(seal_key()))
        .await
        .expect("the first block builds");
    let refused = factory
        .create_complete_block(chain.blocks[3].hash, 81, None, Some(seal_key()))
        .await
        .expect_err("the second block waits for the minimum block time");
    assert!(
        matches!(&refused, ConsensusError::Proposer(reason) if reason.starts_with("Not enough time since last block")),
        "{refused:?}"
    );
    factory.set_min_block_time(Duration::ZERO);
    factory
        .create_complete_block(chain.blocks[3].hash, 82, None, Some(seal_key()))
        .await
        .expect("the block builds once the minimum falls");
}

#[tokio::test]
async fn the_proposer_refuses_a_parent_the_chain_never_held() {
    let mut factory = factory(Duration::ZERO);
    let unknown = H256::repeat_byte(0x33);
    let refused = factory
        .create_complete_block(unknown, 1, None, Some(seal_key()))
        .await
        .expect_err("an unknown parent fails");
    assert!(
        matches!(&refused, ConsensusError::Proposer(reason) if reason == "Parent header not found")
    );
    let refused = factory
        .create_emergency_block(unknown, 1, seal_key())
        .await
        .expect_err("an unknown parent fails the emergency path");
    assert!(
        matches!(&refused, ConsensusError::Proposer(reason) if reason == "Parent header not found")
    );
}

#[tokio::test]
async fn the_proposer_reports_a_failed_author_read() {
    let chain = fixture();
    let mut factory: ProposerFactory<Block, ViewAt, Pool> =
        ProposerFactory::new(ViewAt::unknown(), chain.pool.clone(), Duration::ZERO, 8);
    let refused = factory
        .create_block_with_transactions(chain.blocks[0].hash, 5)
        .await
        .expect_err("the author read fails at an unknown block");
    assert!(
        matches!(&refused, ConsensusError::AuthorSelection(reason) if reason.starts_with("Runtime API error: ")),
        "{refused:?}"
    );
    let refused = factory
        .create_block(chain.blocks[0].hash, 5)
        .expect_err("the older path reads the same author");
    assert!(
        matches!(&refused, ConsensusError::AuthorSelection(reason) if reason.starts_with("Runtime API error: "))
    );
}

#[tokio::test]
async fn the_older_header_paths_carry_placeholder_roots() {
    let chain = fixture();
    let mut factory = factory(Duration::ZERO);
    let (header, author) = factory
        .create_block(chain.blocks[0].hash, 6)
        .expect("the older path answers");
    assert_eq!(author, seal_key());
    assert_eq!(*header.number(), 0);
    assert_eq!(*header.parent_hash(), chain.blocks[0].hash);
    assert_eq!(*header.state_root(), H256::zero());

    let placeholder = factory.create_proposer().expect("the header answers");
    assert_eq!(*placeholder.number(), 0);
    assert_eq!(*placeholder.parent_hash(), H256::zero());
    assert_eq!(*placeholder.extrinsics_root(), H256::zero());
}
