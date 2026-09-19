//! The node interface, checked against the document it publishes.
//!
//! The node publishes one reading surface in the `cerulea_` namespace.
//! Cerulea Explorer, Cerulea Dashboard and the Cerulea APIs chain client
//! read the Blockchain through it. Three gates hold here.
//!
//! 1. **Every method carries a document entry.** The source of the surface
//!    names each method. The document at `docs/NODE_INTERFACE.md` names
//!    each one beside its parameters and its reply.
//! 2. **Every reply carries the JSON shape the document states.** Each
//!    reply type serialises to JSON and the test reads the member names
//!    back.
//! 3. **Every reading path answers.** The runtime side of the surface runs
//!    against a chain built from the development genesis preset.

use std::collections::BTreeSet;

/// The source of the node reading surface.
const SURFACE: &str = include_str!("../src/platform_rpc.rs");

/// The document the surface publishes.
const DOCUMENT: &str = include_str!("../../docs/NODE_INTERFACE.md");

/// Every method name the surface declares.
fn published_methods() -> Vec<String> {
    let mut found = Vec::new();
    for line in SURFACE.lines() {
        let trimmed = line.trim();
        let marker = "#[method(name = \"";
        if let Some(rest) = trimmed.strip_prefix(marker) {
            if let Some(end) = rest.find('"') {
                found.push(rest[..end].to_string());
            }
        }
    }
    found
}

/// Build the externalities of a development chain.
fn development_ext() -> sp_io::TestExternalities {
    use sp_runtime::BuildStorage;
    let json = cerulea_runtime::genesis_config_presets::development_config_genesis();
    let patch = serde_json::to_vec(&json).expect("the development preset serialises");
    let storage = frame_system::GenesisConfig::<cerulea_runtime::Runtime>::default()
        .build_storage()
        .expect("the system module builds its storage");
    let mut ext = sp_io::TestExternalities::new(storage);
    ext.execute_with(|| {
        frame_support::genesis_builder_helper::build_state::<cerulea_runtime::RuntimeGenesisConfig>(
            patch,
        )
        .expect("the development preset builds");
        frame_system::Pallet::<cerulea_runtime::Runtime>::set_block_number(1);
    });
    ext.commit_all().expect("the genesis state commits");
    ext
}

#[test]
fn the_surface_publishes_at_least_thirty_reading_methods() {
    let methods = published_methods();
    assert!(
        methods.len() >= 30,
        "The reading surface publishes {} methods. Cerulea Explorer, Cerulea Dashboard and the Cerulea APIs chain client need at least thirty.",
        methods.len()
    );
}

#[test]
fn every_published_method_sits_in_the_cerulea_namespace() {
    for name in published_methods() {
        assert!(
            name.starts_with("cerulea_"),
            "The method `{name}` sits outside the Cerulea namespace. Rename the method so it starts with `cerulea_`."
        );
    }
}

#[test]
fn every_published_method_carries_a_document_entry() {
    let mut missing: Vec<String> = Vec::new();
    for name in published_methods() {
        if !DOCUMENT.contains(&name) {
            missing.push(name);
        }
    }
    assert!(
        missing.is_empty(),
        "The node interface document names no entry for {}. Add each one to docs/NODE_INTERFACE.md.",
        missing.join(", ")
    );
}

#[test]
fn every_method_the_explorer_named_is_published() {
    // The names the Explorer range and the Cerulea APIs range recorded in
    // the question register.
    let wanted = [
        "cerulea_getBlockDetail",
        "cerulea_getRecentBlocks",
        "cerulea_getTransaction",
        "cerulea_getRecentTransactions",
        "cerulea_getCollections",
        "cerulea_getRecord",
        "cerulea_getRecords",
        "cerulea_getValidatorDetail",
        "cerulea_getValidatorDetails",
        "cerulea_listProposals",
        "cerulea_getProposal",
        "cerulea_getProposalVotes",
        "cerulea_getExecutedChanges",
        "cerulea_verifyRecordProof",
        "cerulea_getSlotCommittee",
        "cerulea_getValidatorKeys",
        "cerulea_getCustodyHistory",
        "cerulea_getConsensusProfile",
    ];
    let published: BTreeSet<String> = published_methods().into_iter().collect();
    let mut missing: Vec<&str> = Vec::new();
    for name in wanted {
        if !published.contains(name) {
            missing.push(name);
        }
    }
    assert!(
        missing.is_empty(),
        "The reading surface publishes no {}. Add each one to cerulea-node/src/platform_rpc.rs.",
        missing.join(", ")
    );
}

#[test]
fn the_block_detail_carries_the_members_the_document_states() {
    let held = cerulea_node::platform_rpc::BlockDetail {
        number: 1,
        hash: "0x00".to_string(),
        parent_hash: "0x00".to_string(),
        state_root: "0x00".to_string(),
        extrinsics_root: "0x00".to_string(),
        author: None,
        slot: Some(7),
        slot_rank: Some(1),
        sealed_at: Some(1000),
        committee: Vec::new(),
        domain_roots: Vec::new(),
        combined_domain_root: None,
        transaction_count: 0,
        commit_weight: 0,
        finalised: false,
    };
    let json = serde_json::to_value(&held).expect("the block detail serialises");
    for member in [
        "number",
        "hash",
        "parentHash",
        "stateRoot",
        "extrinsicsRoot",
        "author",
        "slot",
        "slotRank",
        "sealedAt",
        "committee",
        "domainRoots",
        "combinedDomainRoot",
        "transactionCount",
        "commitWeight",
        "finalised",
    ] {
        assert!(
            json.get(member).is_some(),
            "The block reply carries no `{member}`. Add the member to BlockDetail in cerulea-node/src/platform_rpc.rs."
        );
        assert!(
            DOCUMENT.contains(member),
            "The node interface document names no `{member}`. Add the member to docs/NODE_INTERFACE.md."
        );
    }
}

#[test]
fn the_transaction_detail_carries_the_members_the_document_states() {
    let held = cerulea_node::platform_rpc::TransactionDetail {
        hash: "0x00".to_string(),
        block_number: 1,
        block_hash: "0x00".to_string(),
        index: 0,
        actor: None,
        action: "Logic.execute_action".to_string(),
        class: "routine".to_string(),
        domain: Vec::new(),
        status: "included".to_string(),
        encoded: "0x00".to_string(),
        events: Vec::new(),
    };
    let json = serde_json::to_value(&held).expect("the transaction detail serialises");
    for member in [
        "hash",
        "blockNumber",
        "blockHash",
        "index",
        "actor",
        "action",
        "class",
        "domain",
        "status",
        "encoded",
        "events",
    ] {
        assert!(
            json.get(member).is_some(),
            "The transaction reply carries no `{member}`. Add the member to TransactionDetail in cerulea-node/src/platform_rpc.rs."
        );
        assert!(
            DOCUMENT.contains(member),
            "The node interface document names no `{member}`. Add the member to docs/NODE_INTERFACE.md."
        );
    }
}

#[test]
fn the_validator_detail_carries_the_standing_breakdown() {
    let json = serde_json::to_value(cerulea_node::platform_rpc::ValidatorDetail {
        account: sp_keyring::Ed25519Keyring::Alice.to_account_id(),
        trust_score: 0,
        standing: 0,
        performance: 0,
        custody: 0,
        stake: 0,
        authored_blocks: 0,
        missed_slots: 0,
        uptime_percent: 0,
        active: true,
        custody_answered: 0,
        custody_missed: 0,
        custody_faults: 0,
        penalties: 0,
    })
    .expect("the validator detail serialises");
    for member in [
        "account",
        "trustScore",
        "standing",
        "performance",
        "custody",
        "stake",
        "authoredBlocks",
        "missedSlots",
        "uptimePercent",
        "active",
        "custodyAnswered",
        "custodyMissed",
        "custodyFaults",
        "penalties",
    ] {
        assert!(
            json.get(member).is_some(),
            "The validator reply carries no `{member}`. Add the member to ValidatorDetail in cerulea-node/src/platform_rpc.rs."
        );
        assert!(
            DOCUMENT.contains(member),
            "The node interface document names no `{member}`. Add the member to docs/NODE_INTERFACE.md."
        );
    }
}

#[test]
fn the_reading_paths_answer_on_a_development_chain() {
    use cerulea_runtime::platform_api;
    development_ext().execute_with(|| {
        let deployments = platform_api::deployments();
        assert_eq!(
            deployments.len(),
            1,
            "The development chain installs one Blueprint. The reading path reports {}.",
            deployments.len()
        );
        let deployment = deployments[0];

        assert!(platform_api::blueprint_address(deployment).is_some());
        assert!(platform_api::blueprint(deployment).is_some());
        // The starting Blueprint declares the header alone, so the lists
        // below answer with nothing and the calls still answer.
        assert!(platform_api::collections(deployment).is_empty());
        assert!(platform_api::type_names(deployment).is_empty());
        assert!(platform_api::workflow_names(deployment).is_empty());
        assert!(platform_api::action_names(deployment).is_empty());
        assert!(platform_api::oracle_feeds(deployment).is_empty());
        assert!(platform_api::versions(deployment).is_empty());
        assert!(platform_api::rollback_windows(deployment).is_empty());
        assert!(platform_api::proposals(deployment, 16).is_empty());
        assert!(platform_api::executed_changes(deployment, 16).is_empty());
        assert_eq!(platform_api::audit_length(deployment), 0);

        let licence = platform_api::licence_state(deployment)
            .expect("the development chain installs a licence");
        assert_eq!(licence.state, b"live".to_vec());
        assert!(licence.production_permitted);
        assert!(platform_api::production_permitted());

        let operators = platform_api::operator_accounts(deployment);
        assert_eq!(
            operators.len(),
            1,
            "The development chain appoints one Cerulea operator. The reading path reports {}.",
            operators.len()
        );

        let profile = platform_api::consensus_profile();
        assert_eq!(profile.routine_percent, 51);
        assert_eq!(profile.financial_percent, 67);
        assert_eq!(profile.structural_percent, 80);
        assert_eq!(profile.constitutional_percent, 90);
        assert_eq!(profile.committee_size, 5);

        // A collection the chain never registered answers with nothing
        // rather than a problem.
        assert!(platform_api::record(deployment, b"absent".to_vec(), b"key".to_vec()).is_none());
        assert!(platform_api::records(deployment, b"absent".to_vec(), Vec::new(), 8).is_empty());
        // A collection holding no record answers with the empty root, so
        // a verifier folds a proof against a root either way.
        assert_eq!(
            platform_api::collection_root(deployment, b"absent".to_vec()),
            Some(pallet_cerulea_state::proof::empty_root())
        );
        assert!(!platform_api::verify_record_proof(Vec::new(), [0u8; 32]));

        // A role the Blueprint never declared reaches nothing, and the
        // reply carries the plain sentence.
        let role =
            platform_api::simulate_as_role(deployment, b"absent".to_vec(), b"absent".to_vec());
        assert!(!role.read && !role.insert && !role.update && !role.delete);
        assert!(!role.statement.is_empty());

        // A refused request names the problem and the fix.
        let refusal = platform_api::resolve_permission(
            deployment,
            sp_keyring::Ed25519Keyring::Bob.to_account_id(),
            b"absent".to_vec(),
            b"read".to_vec(),
        );
        assert!(!refusal.admitted);
        assert!(!refusal.problem.is_empty());
        assert!(!refusal.fix.is_empty());

        // A view the Blueprint never declared answers with the plain
        // sentence rather than a panic.
        let view = platform_api::evaluate_view(
            deployment,
            sp_keyring::Ed25519Keyring::Alice.to_account_id(),
            b"absent".to_vec(),
        );
        assert!(!view.evaluated);
        assert!(!view.problem.is_empty());
    });
}
