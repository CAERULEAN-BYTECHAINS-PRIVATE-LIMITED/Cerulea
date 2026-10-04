//! The platform reading surface, called against the real client.
//!
//! Cerulea Explorer, Cerulea Dashboard and the Cerulea APIs chain client read
//! the Blockchain through these methods. Every test calls a method through
//! the JSON-RPC module and reads what the fixture chain recorded: the land
//! registry Blueprint, its parcel record, its proposal, its audit log and the
//! sealed blocks themselves.

use std::sync::Arc;

use cerulea_platform_api::{
    ActionManifestReply, AuditReply, CollectionSummary, ConsensusProfileReply, CustodyReply,
    LicenceReply, OracleReply, PermissionReply, ProposalReply, RecordReply, RoleReply,
    RollbackWindowReply, VersionReply, ViewReply, VoteReply, WorkflowInstanceReply,
};
use cerulea_runtime::AccountId;
use jsonrpsee::rpc_params;
use jsonrpsee::types::ErrorObjectOwned;
use jsonrpsee::{MethodsError, RpcModule};
use sc_client_api::BlockBackend;
use serde::de::DeserializeOwned;
use sp_blockchain::HeaderBackend;
use sp_core::Pair;

use crate::chain_fixture_tests::{
    alice, bob, draw_pair, fixture, seal_key, FaultyStore, StoreFault, ViewAt, MISSED_BLOCKS,
    PARCEL, RECORDED_SURVEY, REGISTRY_ACTION,
};
use crate::platform_rpc::{
    BlockDetail, CeruleaPlatformRpc, CeruleaPlatformRpcApiServer, CommitteeSeat, TransactionDetail,
    ValidatorDetail, ValidatorKeyPair, EXTENSION_DISABLED, PLATFORM_ERROR,
};
use crate::rpc::RpcSecurityConfig;

fn open() -> RpcSecurityConfig {
    RpcSecurityConfig {
        enable_cerulea_extensions: true,
        ..RpcSecurityConfig::default()
    }
}

/// The platform surface over one reader.
fn surface<C>(client: Arc<C>, security: RpcSecurityConfig) -> RpcModule<()>
where
    CeruleaPlatformRpc<C>: CeruleaPlatformRpcApiServer,
{
    CeruleaPlatformRpc::new(client, security)
        .into_rpc()
        .remove_context()
}

/// The platform surface over the fixture client.
fn served() -> RpcModule<()> {
    surface(fixture().client.clone(), open())
}

/// The store key of the recorded parcel.
///
/// The store keys a record by the canonical encoding of its primary key
/// value, and the interface takes that encoding as hex.
fn survey_key() -> Vec<u8> {
    cerulea_blueprint::types::Value::Text(RECORDED_SURVEY.to_string()).canonical_bytes()
}

fn hex_of(bytes: &[u8]) -> String {
    format!("0x{}", hex::encode(bytes))
}

/// The land registry deployment as the surface names it.
fn registry() -> String {
    hex_of(&fixture().deployment)
}

/// The deployment every Cerulea chain starts with.
fn starting() -> String {
    hex_of(&cerulea_runtime::genesis_config_presets::starting_deployment())
}

async fn call<T: DeserializeOwned + Clone>(
    module: &RpcModule<()>,
    method: &str,
    params: jsonrpsee::core::params::ArrayParams,
) -> Result<T, ErrorObjectOwned> {
    match module.call::<_, T>(method, params).await {
        Ok(value) => Ok(value),
        Err(MethodsError::JsonRpc(error)) => Err(error),
        Err(other) => panic!("The method {method} failed outside the interface: {other:?}"),
    }
}

/// Call one method and check the plain sentence of its refusal.
async fn refused_with(
    module: &RpcModule<()>,
    method: &str,
    params: jsonrpsee::core::params::ArrayParams,
    sentence: &str,
) {
    let error = call::<serde_json::Value>(module, method, params)
        .await
        .expect_err("the method refuses the call");
    assert_eq!(error.code(), PLATFORM_ERROR, "{method}");
    assert_eq!(error.message(), sentence, "{method}");
}

#[tokio::test]
async fn a_closed_surface_names_the_flag_that_opens_it() {
    let module = surface(fixture().client.clone(), RpcSecurityConfig::default());
    let error = call::<Vec<String>>(&module, "cerulea_getDeployments", rpc_params![])
        .await
        .unwrap_err();
    assert_eq!(error.code(), EXTENSION_DISABLED);
    assert_eq!(
        error.message(),
        "The Cerulea reading methods are closed on this node. Start the node with the Cerulea extensions flag to open them."
    );
}

#[tokio::test]
async fn the_deployment_list_names_the_starting_and_the_installed_blueprint() {
    let deployments: Vec<String> = call(&served(), "cerulea_getDeployments", rpc_params![])
        .await
        .unwrap();
    assert_eq!(deployments.len(), 2);
    assert!(deployments.contains(&registry()));
    assert!(deployments.contains(&starting()));
}

#[tokio::test]
async fn a_sealed_block_answers_with_its_seal_its_committee_and_its_body() {
    let chain = fixture();
    let built = &chain.blocks[1];
    let detail: BlockDetail = call(&served(), "cerulea_getBlockDetail", rpc_params![2u32])
        .await
        .unwrap();
    assert_eq!(detail.number, 2);
    assert_eq!(detail.hash, hex_of(built.hash.as_bytes()));
    assert_eq!(detail.parent_hash, hex_of(chain.blocks[0].hash.as_bytes()));
    assert_eq!(detail.author, Some(hex_of(&seal_key().0)));
    assert_eq!(detail.slot, Some(built.seal.claim.slot.0));
    assert_eq!(detail.slot_rank, Some(built.seal.claim.rank.0));
    assert_eq!(detail.sealed_at, Some(built.seal.claim.moment_ms));
    assert_eq!(detail.committee.len(), 1);
    assert_eq!(detail.committee[0].validator, alice());
    assert_eq!(detail.committee[0].rank, 1);
    let body = chain
        .client
        .block_body(built.hash)
        .unwrap()
        .expect("the client holds the body");
    assert_eq!(detail.transaction_count as usize, body.len());
    // One inherent, four calls from Alice and two from Bob.
    assert_eq!(detail.transaction_count, 7);
    assert_eq!(
        detail.domain_roots.is_empty(),
        detail.combined_domain_root.is_none()
    );
    assert!(!detail.finalised || detail.number <= 2);
}

#[tokio::test]
async fn the_genesis_block_answers_without_a_seal() {
    let detail: BlockDetail = call(&served(), "cerulea_getBlockDetail", rpc_params![0u32])
        .await
        .unwrap();
    assert_eq!(detail.number, 0);
    assert_eq!(detail.author, None);
    assert_eq!(detail.slot, None);
    assert_eq!(detail.slot_rank, None);
    assert_eq!(detail.sealed_at, None);
    assert!(detail.committee.is_empty());
    assert_eq!(detail.transaction_count, 0);
    assert_eq!(
        detail.hash,
        hex_of(fixture().client.info().genesis_hash.as_bytes())
    );
}

#[tokio::test]
async fn a_height_above_the_head_names_the_highest_block_to_ask_for() {
    refused_with(
        &served(),
        "cerulea_getBlockDetail",
        rpc_params![500u32],
        "The chain holds no block at that height. Name a height at or below the head of the chain.",
    )
    .await;
}

#[tokio::test]
async fn the_recent_blocks_run_newest_first_and_stop_at_genesis() {
    let tip = fixture().client.info().best_number;
    let recent: Vec<BlockDetail> = call(
        &served(),
        "cerulea_getRecentBlocks",
        rpc_params![3u32, None::<u32>],
    )
    .await
    .unwrap();
    let numbers: Vec<u32> = recent.iter().map(|block| block.number).collect();
    assert_eq!(numbers, vec![tip, tip - 1, tip - 2]);

    let clamped: Vec<BlockDetail> = call(
        &served(),
        "cerulea_getRecentBlocks",
        rpc_params![0u32, Some(5u32)],
    )
    .await
    .unwrap();
    assert_eq!(clamped.len(), 1);
    assert_eq!(clamped[0].number, 5);

    let down: Vec<BlockDetail> = call(
        &served(),
        "cerulea_getRecentBlocks",
        rpc_params![10u32, Some(1u32)],
    )
    .await
    .unwrap();
    let numbers: Vec<u32> = down.iter().map(|block| block.number).collect();
    assert_eq!(numbers, vec![1, 0]);

    let beyond: Vec<BlockDetail> = call(
        &served(),
        "cerulea_getRecentBlocks",
        rpc_params![1u32, Some(900u32)],
    )
    .await
    .unwrap();
    assert_eq!(beyond[0].number, tip);
}

#[tokio::test]
async fn a_balance_transfer_answers_as_a_financial_transaction() {
    let chain = fixture();
    let found: TransactionDetail = call(
        &served(),
        "cerulea_getTransaction",
        rpc_params![chain.transfer_hash.to_uppercase().replace("0X", "0x")],
    )
    .await
    .unwrap();
    assert_eq!(found.hash, chain.transfer_hash);
    assert_eq!(found.block_number, 2);
    assert_eq!(found.block_hash, hex_of(chain.blocks[1].hash.as_bytes()));
    assert_eq!(found.action, "Balances.transfer_keep_alive");
    assert_eq!(found.class, "financial");
    assert!(found.domain.is_empty());
    assert_eq!(found.status, "included");
    let actor = found.actor.expect("a signed transfer names its actor");
    // The actor reads as the signer address of the transfer.
    let signer = alice();
    let account: &[u8] = signer.as_ref();
    assert!(actor.contains(&hex::encode(account)), "{actor}");
}

#[tokio::test]
async fn a_blueprint_action_answers_with_the_class_and_the_domain_it_declares() {
    let chain = fixture();
    let found: TransactionDetail = call(
        &served(),
        "cerulea_getTransaction",
        rpc_params![chain.action_hash.clone()],
    )
    .await
    .unwrap();
    assert_eq!(found.action, "Logic.execute_action");
    assert_eq!(found.class, "routine");
    assert_eq!(found.domain, vec![PARCEL.to_string()]);
    assert_eq!(found.block_number, 2);
}

#[tokio::test]
async fn an_unknown_transaction_hash_names_what_to_supply() {
    refused_with(
        &served(),
        "cerulea_getTransaction",
        rpc_params!["0xdeadbeef"],
        "The chain holds no transaction under that hash. Supply the hash of a transaction the chain already carried.",
    )
    .await;
}

#[tokio::test]
async fn the_recent_transactions_carry_every_module_class() {
    let tip = fixture().client.info().best_number;
    let newest: Vec<TransactionDetail> = call(
        &served(),
        "cerulea_getRecentTransactions",
        rpc_params![3u32, None::<u32>],
    )
    .await
    .unwrap();
    assert_eq!(newest.len(), 3);
    for (offset, one) in newest.iter().enumerate() {
        // Every block after block two carries the timestamp inherent alone.
        assert_eq!(one.block_number, tip - offset as u32);
        assert_eq!(one.action, "Timestamp.set");
        assert_eq!(one.class, "routine");
        assert_eq!(one.actor, None);
    }

    let early: Vec<TransactionDetail> = call(
        &served(),
        "cerulea_getRecentTransactions",
        rpc_params![500u32, Some(2u32)],
    )
    .await
    .unwrap();
    // Block two carries seven, block one carries twelve and genesis none.
    assert_eq!(early.len(), 19);
    let classes: std::collections::BTreeSet<&str> =
        early.iter().map(|one| one.class.as_str()).collect();
    for class in ["routine", "structural", "financial", "constitutional"] {
        assert!(classes.contains(class), "{class}");
    }
    assert!(early
        .iter()
        .any(|one| one.action == "Logic.install_blueprint"));
    assert!(early
        .iter()
        .any(|one| one.action == "Governance.propose" && one.class == "constitutional"));
    assert!(early
        .iter()
        .any(|one| one.action == "Identity.assign_role" && one.class == "structural"));

    let limited: Vec<TransactionDetail> = call(
        &served(),
        "cerulea_getRecentTransactions",
        rpc_params![2u32, Some(2u32)],
    )
    .await
    .unwrap();
    assert_eq!(limited.len(), 2);
    assert!(limited.iter().all(|one| one.block_number == 2));
}

#[tokio::test]
async fn the_registry_collection_answers_its_record_and_its_proof() {
    let module = served();
    let collections: Vec<CollectionSummary> =
        call(&module, "cerulea_getCollections", rpc_params![registry()])
            .await
            .unwrap();
    assert_eq!(collections.len(), 1);
    assert_eq!(collections[0].name, PARCEL.as_bytes().to_vec());
    assert_eq!(collections[0].records, 1);

    let record: Option<RecordReply> = call(
        &module,
        "cerulea_getRecord",
        rpc_params![PARCEL, hex_of(&survey_key()), registry()],
    )
    .await
    .unwrap();
    let record = record.expect("the action wrote the parcel record");
    assert_eq!(record.collection, PARCEL.as_bytes().to_vec());
    assert_eq!(record.key, survey_key());
    assert_eq!(record.root, collections[0].root);

    let missing: Option<RecordReply> = call(
        &module,
        "cerulea_getRecord",
        rpc_params![PARCEL, "0x9999", registry()],
    )
    .await
    .unwrap();
    assert!(missing.is_none());

    let listed: Vec<RecordReply> = call(
        &module,
        "cerulea_getRecords",
        rpc_params![PARCEL, Some(hex_of(&survey_key()[..4])), 10u32, registry()],
    )
    .await
    .unwrap();
    assert_eq!(listed.len(), 1);
    let none: Vec<RecordReply> = call(
        &module,
        "cerulea_getRecords",
        rpc_params![PARCEL, Some("0x99"), 10u32, registry()],
    )
    .await
    .unwrap();
    assert!(none.is_empty());
    let every: Vec<RecordReply> = call(
        &module,
        "cerulea_getRecords",
        rpc_params![PARCEL, None::<String>, 10u32, registry()],
    )
    .await
    .unwrap();
    assert_eq!(every.len(), 1);

    let root: Option<String> = call(
        &module,
        "cerulea_getCollectionRoot",
        rpc_params![PARCEL, registry()],
    )
    .await
    .unwrap();
    assert_eq!(root, Some(hex_of(&collections[0].root)));
    let absent: Option<String> = call(
        &module,
        "cerulea_getCollectionRoot",
        rpc_params!["deed", registry()],
    )
    .await
    .unwrap();
    assert_eq!(absent, None);

    let proof = hex_of(&record.proof.expect("an open record carries its proof"));
    let holds: bool = call(
        &module,
        "cerulea_verifyRecordProof",
        rpc_params![proof.clone(), hex_of(&record.root)],
    )
    .await
    .unwrap();
    assert!(holds);
    let fails: bool = call(
        &module,
        "cerulea_verifyRecordProof",
        rpc_params![proof.clone(), hex_of(&[1u8; 32])],
    )
    .await
    .unwrap();
    assert!(!fails);
}

#[tokio::test]
async fn every_malformed_argument_names_its_fix() {
    let module = served();
    refused_with(
        &module,
        "cerulea_verifyRecordProof",
        rpc_params!["0xzz", hex_of(&[0u8; 32])],
        "The proof carries characters outside hexadecimal. Supply the canonical encoding of the proof as a hex string.",
    )
    .await;
    refused_with(
        &module,
        "cerulea_verifyRecordProof",
        rpc_params!["0x00", "0xzz"],
        "The root carries characters outside hexadecimal. Supply the thirty two byte root as a hex string.",
    )
    .await;
    refused_with(
        &module,
        "cerulea_verifyRecordProof",
        rpc_params!["0x00", "0x1234"],
        "The root carries another length. Supply the thirty two byte collection root.",
    )
    .await;
    refused_with(
        &module,
        "cerulea_getCollections",
        rpc_params!["0xnothex"],
        "The deployment identifier carries characters outside hexadecimal. Supply the content address of the Blueprint as a hex string.",
    )
    .await;
    refused_with(
        &module,
        "cerulea_getCollections",
        rpc_params!["0x1234"],
        "The deployment identifier carries another length. Supply the thirty two byte content address of the Blueprint.",
    )
    .await;
}

#[tokio::test]
async fn a_call_naming_no_deployment_reads_the_first_one_the_chain_holds() {
    let module = served();
    let deployments: Vec<String> = call(&module, "cerulea_getDeployments", rpc_params![])
        .await
        .unwrap();
    let first: Vec<String> = call(
        &module,
        "cerulea_getSchemaTypes",
        rpc_params![None::<String>],
    )
    .await
    .unwrap();
    let named: Vec<String> = call(
        &module,
        "cerulea_getSchemaTypes",
        rpc_params![deployments[0].clone()],
    )
    .await
    .unwrap();
    assert_eq!(first, named);
}

#[tokio::test]
async fn the_validator_surface_reads_the_standing_the_keys_and_the_committee() {
    let chain = fixture();
    let module = served();
    let detail: ValidatorDetail = call(&module, "cerulea_getValidatorDetail", rpc_params![alice()])
        .await
        .unwrap();
    assert_eq!(detail.account, alice());
    assert!(detail.active);
    assert_eq!(detail.authored_blocks, 2);
    assert_eq!(detail.missed_slots, MISSED_BLOCKS);
    assert!(detail.stake > 0);
    // A validator with no settled challenge carries the starting custody
    // share, parts per million, and no answered count.
    assert_eq!(
        detail.custody,
        pallet_cerulea_custody::score::STARTING_SHARE
    );
    assert_eq!(detail.custody_answered, 0);

    let outsider: ValidatorDetail = call(&module, "cerulea_getValidatorDetail", rpc_params![bob()])
        .await
        .unwrap();
    assert!(!outsider.active);
    assert_eq!(outsider.authored_blocks, 0);
    assert_eq!(outsider.trust_score, 0);

    let every: Vec<ValidatorDetail> = call(&module, "cerulea_getValidatorDetails", rpc_params![])
        .await
        .unwrap();
    assert_eq!(every.len(), 1);
    assert_eq!(every[0].account, alice());

    let keys: Vec<ValidatorKeyPair> = call(&module, "cerulea_getValidatorKeys", rpc_params![])
        .await
        .unwrap();
    assert_eq!(keys.len(), 1);
    assert_eq!(keys[0].validator, alice());
    assert_eq!(keys[0].draw_key, hex_of(&draw_pair().public().0));
    assert_eq!(keys[0].seal_key, hex_of(&seal_key().0));

    let slot = chain.blocks[3].seal.claim.slot.0;
    let seats: Vec<CommitteeSeat> = call(&module, "cerulea_getSlotCommittee", rpc_params![slot])
        .await
        .unwrap();
    assert_eq!(seats.len(), 1);
    assert_eq!(seats[0].validator, alice());
    assert_eq!(seats[0].rank, 1);

    let custody: Vec<CustodyReply<AccountId>> = call(
        &module,
        "cerulea_getCustodyHistory",
        rpc_params![alice(), None::<u32>],
    )
    .await
    .unwrap();
    assert!(custody.iter().all(|entry| entry.validator == alice()));

    let profile: ConsensusProfileReply =
        call(&module, "cerulea_getConsensusProfile", rpc_params![])
            .await
            .unwrap();
    assert_eq!(profile.routine_percent, 51);
    assert_eq!(profile.financial_percent, 67);
    assert_eq!(profile.structural_percent, 80);
    assert_eq!(profile.constitutional_percent, 90);
}

#[tokio::test]
async fn the_change_process_answers_the_open_proposal_and_its_vote() {
    let module = served();
    let proposals: Vec<ProposalReply> = call(
        &module,
        "cerulea_listProposals",
        rpc_params![None::<u32>, registry()],
    )
    .await
    .unwrap();
    assert_eq!(proposals.len(), 1);
    assert_eq!(proposals[0].id, 0);
    assert_eq!(proposals[0].kind, b"policy".to_vec());
    assert_eq!(proposals[0].status, b"open".to_vec());

    let one: Option<ProposalReply> = call(
        &module,
        "cerulea_getProposal",
        rpc_params![0u64, registry()],
    )
    .await
    .unwrap();
    assert_eq!(one.expect("the proposal exists").approve_weight, 10);
    let none: Option<ProposalReply> = call(
        &module,
        "cerulea_getProposal",
        rpc_params![7u64, registry()],
    )
    .await
    .unwrap();
    assert!(none.is_none());

    let votes: Vec<VoteReply<AccountId>> = call(
        &module,
        "cerulea_getProposalVotes",
        rpc_params![0u64, registry()],
    )
    .await
    .unwrap();
    assert_eq!(votes.len(), 1);
    assert_eq!(votes[0].voter, alice());
    assert!(votes[0].approve);
    assert_eq!(votes[0].weight, 10);

    let executed: Vec<ProposalReply> = call(
        &module,
        "cerulea_getExecutedChanges",
        rpc_params![Some(5u32), registry()],
    )
    .await
    .unwrap();
    assert!(executed.is_empty());
}

#[tokio::test]
async fn the_audit_log_names_the_registrar_behind_every_entry() {
    let module = served();
    let length: u64 = call(&module, "cerulea_getAuditLength", rpc_params![registry()])
        .await
        .unwrap();
    assert!(length >= 3);
    let entries: Vec<AuditReply<AccountId>> = call(
        &module,
        "cerulea_getAuditLog",
        rpc_params![0u64, 500u32, registry()],
    )
    .await
    .unwrap();
    assert_eq!(entries.len() as u64, length);
    for (index, entry) in entries.iter().enumerate() {
        assert_eq!(entry.sequence, index as u64);
    }
    let mine: Vec<AuditReply<AccountId>> = call(
        &module,
        "cerulea_getAuditByActor",
        rpc_params![alice(), 500u32, registry()],
    )
    .await
    .unwrap();
    assert!(!mine.is_empty());
    assert!(mine.iter().all(|entry| entry.actor == alice()));
    let none: Vec<AuditReply<AccountId>> = call(
        &module,
        "cerulea_getAuditByActor",
        rpc_params![bob(), 500u32, registry()],
    )
    .await
    .unwrap();
    assert!(none.is_empty());
}

#[tokio::test]
async fn the_capability_matrix_admits_the_registrar_and_refuses_bob() {
    let module = served();
    let admitted: PermissionReply = call(
        &module,
        "cerulea_resolvePermission",
        rpc_params![alice(), PARCEL, "read", registry()],
    )
    .await
    .unwrap();
    assert!(admitted.admitted);
    assert_eq!(admitted.role, Some(b"registrar".to_vec()));

    let refused: PermissionReply = call(
        &module,
        "cerulea_resolvePermission",
        rpc_params![bob(), PARCEL, "read", registry()],
    )
    .await
    .unwrap();
    assert!(!refused.admitted);
    assert!(!refused.fix.is_empty());

    let role: RoleReply = call(
        &module,
        "cerulea_simulateAsRole",
        rpc_params!["registrar", PARCEL, registry()],
    )
    .await
    .unwrap();
    assert!(role.read && role.insert && role.update && role.delete);
    assert_eq!(role.actions, vec![REGISTRY_ACTION.as_bytes().to_vec()]);

    let view: ViewReply = call(
        &module,
        "cerulea_evaluateView",
        rpc_params!["holders", alice(), registry()],
    )
    .await
    .unwrap();
    assert!(view.evaluated, "{}", String::from_utf8_lossy(&view.problem));
    assert_eq!(view.rows, 1);
}

#[tokio::test]
async fn the_blueprint_reads_answer_the_declared_types_and_actions() {
    let module = served();
    let types: Vec<String> = call(&module, "cerulea_getSchemaTypes", rpc_params![registry()])
        .await
        .unwrap();
    assert_eq!(types, vec!["parcel".to_string()]);
    let parcel: Option<String> = call(
        &module,
        "cerulea_getSchemaType",
        rpc_params!["parcel", registry()],
    )
    .await
    .unwrap();
    assert!(parcel.expect("the type is registered").starts_with("0x"));
    let deed: Option<String> = call(
        &module,
        "cerulea_getSchemaType",
        rpc_params!["deed", registry()],
    )
    .await
    .unwrap();
    assert!(deed.is_none());

    let actions: Vec<String> = call(&module, "cerulea_getActions", rpc_params![registry()])
        .await
        .unwrap();
    assert_eq!(actions, vec![REGISTRY_ACTION.to_string()]);
    let manifest: Option<ActionManifestReply> = call(
        &module,
        "cerulea_getActionManifest",
        rpc_params![REGISTRY_ACTION, registry()],
    )
    .await
    .unwrap();
    let manifest = manifest.expect("the action carries a manifest");
    assert_eq!(manifest.writes, vec![PARCEL.as_bytes().to_vec()]);
    assert_eq!(manifest.class, b"routine".to_vec());

    let workflows: Vec<String> = call(&module, "cerulea_getWorkflows", rpc_params![registry()])
        .await
        .unwrap();
    assert!(workflows.is_empty());
    let instance: Option<WorkflowInstanceReply> = call(
        &module,
        "cerulea_getWorkflowInstance",
        rpc_params!["approval", 0u64, registry()],
    )
    .await
    .unwrap();
    assert!(instance.is_none());
    let instances: Vec<WorkflowInstanceReply> = call(
        &module,
        "cerulea_getWorkflowInstances",
        rpc_params!["approval", 5u32, registry()],
    )
    .await
    .unwrap();
    assert!(instances.is_empty());

    let feeds: Vec<String> = call(&module, "cerulea_getOracleFeeds", rpc_params![registry()])
        .await
        .unwrap();
    assert!(feeds.is_empty());
    let value: Option<OracleReply> = call(
        &module,
        "cerulea_getOracleValue",
        rpc_params!["rainfall", registry()],
    )
    .await
    .unwrap();
    assert!(value.is_none());

    let versions: Vec<VersionReply> = call(&module, "cerulea_getVersions", rpc_params![registry()])
        .await
        .unwrap();
    assert!(versions
        .iter()
        .all(|version| version.revision >= 1 || !version.active));
    let windows: Vec<RollbackWindowReply> = call(
        &module,
        "cerulea_getRollbackWindows",
        rpc_params![registry()],
    )
    .await
    .unwrap();
    assert!(windows.is_empty());
}

#[tokio::test]
async fn the_starting_deployment_answers_its_licence_and_its_operator() {
    let module = served();
    let licence: Option<LicenceReply> =
        call(&module, "cerulea_getLicenceState", rpc_params![starting()])
            .await
            .unwrap();
    let licence = licence.expect("the starting deployment carries a licence");
    assert!(licence.production_permitted);
    let permitted: bool = call(&module, "cerulea_getProductionPermitted", rpc_params![])
        .await
        .unwrap();
    assert!(permitted);
    let operators: Vec<AccountId> = call(&module, "cerulea_getOperators", rpc_params![starting()])
        .await
        .unwrap();
    assert_eq!(
        operators,
        vec![cerulea_runtime::genesis_config_presets::cerulea_operator()]
    );
}

#[tokio::test]
async fn every_failed_runtime_read_names_the_runtime_check() {
    let module = surface(ViewAt::unknown(), open());
    let deployment = registry();
    let check = "Check that the node runtime matches the chain specification.";
    for (method, params, lead) in [
        (
            "cerulea_getDeployments",
            rpc_params![],
            "The deployment list failed to load.",
        ),
        (
            "cerulea_getCollections",
            rpc_params![None::<String>],
            "The deployment list failed to load.",
        ),
        (
            "cerulea_getCollections",
            rpc_params![deployment.clone()],
            "The collection list failed to load.",
        ),
        (
            "cerulea_getRecord",
            rpc_params![PARCEL, "k", deployment.clone()],
            "The record failed to load.",
        ),
        (
            "cerulea_getRecords",
            rpc_params![PARCEL, None::<String>, 5u32, deployment.clone()],
            "The record list failed to load.",
        ),
        (
            "cerulea_verifyRecordProof",
            rpc_params!["0x00", hex_of(&[0u8; 32])],
            "The proof check failed to run.",
        ),
        (
            "cerulea_getCollectionRoot",
            rpc_params![PARCEL, deployment.clone()],
            "The collection root failed to load.",
        ),
        (
            "cerulea_getValidatorDetails",
            rpc_params![],
            "The validator set failed to load.",
        ),
        (
            "cerulea_getValidatorKeys",
            rpc_params![],
            "The validator key records failed to load.",
        ),
        (
            "cerulea_getSlotCommittee",
            rpc_params![3u64],
            "The slot committee failed to load.",
        ),
        (
            "cerulea_getCustodyHistory",
            rpc_params![alice(), Some(3u32)],
            "The custody history failed to load.",
        ),
        (
            "cerulea_getConsensusProfile",
            rpc_params![],
            "The consensus profile failed to load.",
        ),
        (
            "cerulea_listProposals",
            rpc_params![None::<u32>, deployment.clone()],
            "The proposal list failed to load.",
        ),
        (
            "cerulea_getProposal",
            rpc_params![0u64, deployment.clone()],
            "The proposal failed to load.",
        ),
        (
            "cerulea_getProposalVotes",
            rpc_params![0u64, deployment.clone()],
            "The vote list failed to load.",
        ),
        (
            "cerulea_getExecutedChanges",
            rpc_params![None::<u32>, deployment.clone()],
            "The executed change list failed to load.",
        ),
        (
            "cerulea_getAuditLog",
            rpc_params![0u64, 5u32, deployment.clone()],
            "The audit log failed to load.",
        ),
        (
            "cerulea_getAuditLength",
            rpc_params![deployment.clone()],
            "The audit log length failed to load.",
        ),
        (
            "cerulea_getAuditByActor",
            rpc_params![alice(), 5u32, deployment.clone()],
            "The audit log failed to load.",
        ),
        (
            "cerulea_resolvePermission",
            rpc_params![alice(), PARCEL, "read", deployment.clone()],
            "The capability matrix failed to load.",
        ),
        (
            "cerulea_simulateAsRole",
            rpc_params!["registrar", PARCEL, deployment.clone()],
            "The role preview failed to run.",
        ),
        (
            "cerulea_evaluateView",
            rpc_params!["holders", alice(), deployment.clone()],
            "The view failed to evaluate.",
        ),
        (
            "cerulea_getWorkflows",
            rpc_params![deployment.clone()],
            "The workflow list failed to load.",
        ),
        (
            "cerulea_getWorkflowInstance",
            rpc_params!["w", 0u64, deployment.clone()],
            "The process failed to load.",
        ),
        (
            "cerulea_getWorkflowInstances",
            rpc_params!["w", 5u32, deployment.clone()],
            "The process list failed to load.",
        ),
        (
            "cerulea_getLicenceState",
            rpc_params![deployment.clone()],
            "The licence state failed to load.",
        ),
        (
            "cerulea_getProductionPermitted",
            rpc_params![],
            "The production gate failed to load.",
        ),
        (
            "cerulea_getOracleFeeds",
            rpc_params![deployment.clone()],
            "The feed list failed to load.",
        ),
        (
            "cerulea_getOracleValue",
            rpc_params!["f", deployment.clone()],
            "The feed value failed to load.",
        ),
        (
            "cerulea_getVersions",
            rpc_params![deployment.clone()],
            "The version registry failed to load.",
        ),
        (
            "cerulea_getRollbackWindows",
            rpc_params![deployment.clone()],
            "The rollback windows failed to load.",
        ),
        (
            "cerulea_getSchemaTypes",
            rpc_params![deployment.clone()],
            "The type registry failed to load.",
        ),
        (
            "cerulea_getSchemaType",
            rpc_params!["parcel", deployment.clone()],
            "The type definition failed to load.",
        ),
        (
            "cerulea_getActions",
            rpc_params![deployment.clone()],
            "The action list failed to load.",
        ),
        (
            "cerulea_getActionManifest",
            rpc_params![REGISTRY_ACTION, deployment.clone()],
            "The action manifest failed to load.",
        ),
        (
            "cerulea_getOperators",
            rpc_params![deployment.clone()],
            "The operator list failed to load.",
        ),
    ] {
        refused_with(&module, method, params, &format!("{lead} {check}")).await;
    }

    // A validator read gathers what the chain answers and reads zero for the
    // rest, so the detail still answers.
    let detail: ValidatorDetail = call(&module, "cerulea_getValidatorDetail", rpc_params![alice()])
        .await
        .unwrap();
    assert_eq!(detail.trust_score, 0);
    assert_eq!(detail.stake, 0);
    assert!(!detail.active);

    // The block reads take the committee, the finality and the weight as
    // absent, and still answer from the block store.
    let block: BlockDetail = call(&module, "cerulea_getBlockDetail", rpc_params![2u32])
        .await
        .unwrap();
    assert!(block.committee.is_empty());
    assert!(!block.finalised);
    assert_eq!(block.commit_weight, 0);
    let transactions: Vec<TransactionDetail> = call(
        &module,
        "cerulea_getRecentTransactions",
        rpc_params![3u32, Some(2u32)],
    )
    .await
    .unwrap();
    assert_eq!(transactions.len(), 3);
    assert!(transactions.iter().all(|one| one.status == "included"));
}

#[tokio::test]
async fn every_block_store_fault_names_the_database_check() {
    for (fault, sentence) in [
        (
            StoreFault::HashIndex,
            "The block index failed to load. Check that the node database is present.",
        ),
        (
            StoreFault::HeaderRead,
            "The block header failed to load. Check that the node database is present.",
        ),
        (
            StoreFault::HeaderMissing,
            "The chain holds no header for that block. Name a block the node still holds.",
        ),
        (
            StoreFault::BodyRead,
            "The block body failed to load. Check that the node database is present.",
        ),
    ] {
        let module = surface(FaultyStore::with(fault), open());
        refused_with(
            &module,
            "cerulea_getBlockDetail",
            rpc_params![1u32],
            sentence,
        )
        .await;
        refused_with(
            &module,
            "cerulea_getRecentTransactions",
            rpc_params![1u32, Some(1u32)],
            sentence,
        )
        .await;
        refused_with(
            &module,
            "cerulea_getTransaction",
            rpc_params!["0x00"],
            sentence,
        )
        .await;
    }
}
