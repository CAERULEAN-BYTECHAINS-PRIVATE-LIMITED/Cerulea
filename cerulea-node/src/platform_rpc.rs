//! The node reading surface of the Cerulea platform.
//!
//! Cerulea Explorer, Cerulea Dashboard and the Cerulea APIs chain client
//! read the Blockchain through the methods here. Every method sits in the
//! `cerulea_` namespace and changes nothing.
//!
//! # Where each answer comes from
//!
//! 1. **Blocks and transactions.** The node holds the headers and the
//!    bodies. The Authoring Engine seal inside the header carries the slot
//!    and the rank the author claimed. The Commit Engine envelope inside
//!    the header carries the weight the signers brought.
//! 2. **Records, collections and proofs.** The state module answers
//!    through the platform runtime interface.
//! 3. **Validators, custody and the consensus profile.** The framework
//!    pallet, the validator registry and the custody module answer through
//!    their runtime interfaces.
//!
//! # The deployment argument
//!
//! A chain runs one deployment in the ordinary case. A method taking a
//! deployment therefore accepts nothing and reads the first deployment the
//! chain holds, or accepts the content address of the Blueprint as a hex
//! string.
//!
//! Every JSON shape is documented in `docs/NODE_INTERFACE.md`.

use std::sync::Arc;

use cerulea_platform_api::{
    ActionManifestReply, AuditReply, CeruleaPlatformApi, CollectionSummary, ConsensusProfileReply,
    CustodyReply, DeploymentId, LicenceReply, OracleReply, PermissionReply, ProposalReply,
    RecordReply, RoleReply, RollbackWindowReply, VersionReply, ViewReply, VoteReply,
    WorkflowInstanceReply,
};
use cerulea_runtime::{opaque::Block, AccountId};
use codec::Decode;
use jsonrpsee::core::RpcResult;
use jsonrpsee::proc_macros::rpc;
use jsonrpsee::types::ErrorObjectOwned;
use pallet_cerulea_dcf::DcfApi;
use pallet_cerulea_dvf::{CeruleaAuthoringApi, DvfApi};
use sc_client_api::BlockBackend;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::traits::{Block as BlockT, Header as HeaderT};

use crate::rpc::RpcSecurityConfig;

/// The error code every platform reading method reports a problem under.
pub const PLATFORM_ERROR: i32 = -32010;

/// The error code a refused extension reports under.
pub const EXTENSION_DISABLED: i32 = -32001;

/// One seat of one slot committee.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitteeSeat {
    /// The rank the seat holds, counting from one.
    pub rank: u8,
    /// The validator holding the seat.
    pub validator: AccountId,
    /// The seat value after scaling by the Trust Score.
    pub standing: u64,
}

/// The two keys one validator carries.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidatorKeyPair {
    /// The validator account.
    pub validator: AccountId,
    /// The sr25519 key the slot draw reads, as a hex string.
    pub draw_key: String,
    /// The ed25519 key the block seal reads, as a hex string.
    pub seal_key: String,
}

/// One block with the figures Cerulea Explorer shows.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockDetail {
    /// The height of the block.
    pub number: u32,
    /// The hash of the block, as a hex string.
    pub hash: String,
    /// The hash of the parent block, as a hex string.
    pub parent_hash: String,
    /// The state root of the block, as a hex string.
    pub state_root: String,
    /// The extrinsics root of the block, as a hex string.
    pub extrinsics_root: String,
    /// The account the seal names as the author.
    pub author: Option<String>,
    /// The slot the block serves.
    pub slot: Option<u64>,
    /// The rank the author claimed inside the slot committee.
    pub slot_rank: Option<u8>,
    /// The moment the author sealed the block, in milliseconds.
    pub sealed_at: Option<u64>,
    /// The slot committee in rank order.
    pub committee: Vec<CommitteeSeat>,
    /// One root per conflict domain, in domain order, as hex strings.
    pub domain_roots: Vec<String>,
    /// The root over the domain roots, as a hex string.
    pub combined_domain_root: Option<String>,
    /// The number of transactions the block carries.
    pub transaction_count: u32,
    /// The weight the signers brought to the block.
    pub commit_weight: u128,
    /// The block reached the finality threshold.
    pub finalised: bool,
}

/// One transaction with the figures Cerulea Explorer shows.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionDetail {
    /// The hash of the transaction, as a hex string.
    pub hash: String,
    /// The block the transaction landed in.
    pub block_number: u32,
    /// The hash of that block, as a hex string.
    pub block_hash: String,
    /// The place of the transaction inside the block.
    pub index: u32,
    /// The account the Blockchain records as the actor.
    pub actor: Option<String>,
    /// The module and call the transaction runs.
    pub action: String,
    /// The class of work the transaction carries.
    pub class: String,
    /// The collections the transaction writes. These name the conflict
    /// domain.
    pub domain: Vec<String>,
    /// Where the transaction stands. One of included or finalised.
    pub status: String,
    /// The canonical encoding of the transaction, as a hex string.
    pub encoded: String,
    /// The events the transaction raised, read from the audit log.
    pub events: Vec<String>,
}

/// One validator with the standing breakdown Cerulea Explorer shows.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidatorDetail {
    /// The validator account.
    pub account: AccountId,
    /// The Trust Score the validator carries.
    pub trust_score: u64,
    /// The standing share of the Trust Score.
    pub standing: u64,
    /// The measured performance share of the Trust Score.
    pub performance: u64,
    /// The custody share of the Trust Score.
    pub custody: u32,
    /// The stake the validator holds.
    pub stake: u128,
    /// The blocks the validator authored.
    pub authored_blocks: u32,
    /// The slots the validator missed.
    pub missed_slots: u32,
    /// The measured uptime, in percent.
    pub uptime_percent: u32,
    /// The validator sits in the active set.
    pub active: bool,
    /// The custody challenges the validator answered.
    pub custody_answered: u32,
    /// The custody challenges the validator let pass.
    pub custody_missed: u32,
    /// The custody proofs that failed verification.
    pub custody_faults: u32,
    /// The penalties the validator carries.
    pub penalties: u32,
}

/// Turn bytes into a hex string carrying the leading marker.
fn hex_of(bytes: &[u8]) -> String {
    format!("0x{}", hex::encode(bytes))
}

/// Read a hex string as bytes.
fn bytes_of(text: &str) -> Option<Vec<u8>> {
    hex::decode(text.trim_start_matches("0x")).ok()
}

/// Build one problem with the sentence naming the fix.
fn problem(message: &str) -> ErrorObjectOwned {
    ErrorObjectOwned::owned(PLATFORM_ERROR, message.to_string(), None::<()>)
}

/// The platform reading surface.
#[rpc(server)]
pub trait CeruleaPlatformRpcApi {
    /// Every deployment the chain holds, as hex strings.
    #[method(name = "cerulea_getDeployments")]
    fn get_deployments(&self) -> RpcResult<Vec<String>>;

    /// One block with its slot rank, its committee, its domain roots and
    /// its commit weight.
    #[method(name = "cerulea_getBlockDetail")]
    fn get_block_detail(&self, block_number: u32) -> RpcResult<BlockDetail>;

    /// A run of blocks in the same shape, newest first.
    #[method(name = "cerulea_getRecentBlocks")]
    fn get_recent_blocks(&self, limit: u32, before: Option<u32>) -> RpcResult<Vec<BlockDetail>>;

    /// One transaction with its actor, its class, its domain and its
    /// status.
    #[method(name = "cerulea_getTransaction")]
    fn get_transaction(&self, transaction_hash: String) -> RpcResult<TransactionDetail>;

    /// A run of transactions in the same shape, newest first.
    #[method(name = "cerulea_getRecentTransactions")]
    fn get_recent_transactions(
        &self,
        limit: u32,
        before_block: Option<u32>,
    ) -> RpcResult<Vec<TransactionDetail>>;

    /// Every collection with its privacy class and its record count.
    #[method(name = "cerulea_getCollections")]
    fn get_collections(&self, deployment: Option<String>) -> RpcResult<Vec<CollectionSummary>>;

    /// One record with its commitment, its version history and its proof.
    #[method(name = "cerulea_getRecord")]
    fn get_record(
        &self,
        collection: String,
        key: String,
        deployment: Option<String>,
    ) -> RpcResult<Option<RecordReply>>;

    /// The records of one collection whose key starts with the prefix.
    #[method(name = "cerulea_getRecords")]
    fn get_records(
        &self,
        collection: String,
        prefix: Option<String>,
        limit: u32,
        deployment: Option<String>,
    ) -> RpcResult<Vec<RecordReply>>;

    /// Fold one inclusion proof and compare the result with a root.
    #[method(name = "cerulea_verifyRecordProof")]
    fn verify_record_proof(&self, proof: String, root: String) -> RpcResult<bool>;

    /// The inclusion tree root of one collection.
    #[method(name = "cerulea_getCollectionRoot")]
    fn get_collection_root(
        &self,
        collection: String,
        deployment: Option<String>,
    ) -> RpcResult<Option<String>>;

    /// One validator with its standing breakdown, its uptime, its custody
    /// history and its penalty record.
    #[method(name = "cerulea_getValidatorDetail")]
    fn get_validator_detail(&self, account: AccountId) -> RpcResult<ValidatorDetail>;

    /// Every active validator in the same shape.
    #[method(name = "cerulea_getValidatorDetails")]
    fn get_validator_details(&self) -> RpcResult<Vec<ValidatorDetail>>;

    /// The draw key and the seal key of every active validator.
    #[method(name = "cerulea_getValidatorKeys")]
    fn get_validator_keys(&self) -> RpcResult<Vec<ValidatorKeyPair>>;

    /// The slot committee of one slot, in rank order.
    #[method(name = "cerulea_getSlotCommittee")]
    fn get_slot_committee(&self, slot: u64) -> RpcResult<Vec<CommitteeSeat>>;

    /// The custody challenge history of one validator.
    #[method(name = "cerulea_getCustodyHistory")]
    fn get_custody_history(
        &self,
        account: AccountId,
        limit: Option<u32>,
    ) -> RpcResult<Vec<CustodyReply<AccountId>>>;

    /// The class thresholds and the authoring figures the chain runs under.
    #[method(name = "cerulea_getConsensusProfile")]
    fn get_consensus_profile(&self) -> RpcResult<ConsensusProfileReply>;

    /// The proposals of the change process, newest first.
    #[method(name = "cerulea_listProposals")]
    fn list_proposals(
        &self,
        limit: Option<u32>,
        deployment: Option<String>,
    ) -> RpcResult<Vec<ProposalReply>>;

    /// One proposal.
    #[method(name = "cerulea_getProposal")]
    fn get_proposal(
        &self,
        proposal_id: u64,
        deployment: Option<String>,
    ) -> RpcResult<Option<ProposalReply>>;

    /// The votes one proposal drew.
    #[method(name = "cerulea_getProposalVotes")]
    fn get_proposal_votes(
        &self,
        proposal_id: u64,
        deployment: Option<String>,
    ) -> RpcResult<Vec<VoteReply<AccountId>>>;

    /// The proposals a module already carried out.
    #[method(name = "cerulea_getExecutedChanges")]
    fn get_executed_changes(
        &self,
        limit: Option<u32>,
        deployment: Option<String>,
    ) -> RpcResult<Vec<ProposalReply>>;

    /// The audit log, from one position forward.
    #[method(name = "cerulea_getAuditLog")]
    fn get_audit_log(
        &self,
        from: u64,
        limit: u32,
        deployment: Option<String>,
    ) -> RpcResult<Vec<AuditReply<AccountId>>>;

    /// The number of entries the audit log holds.
    #[method(name = "cerulea_getAuditLength")]
    fn get_audit_length(&self, deployment: Option<String>) -> RpcResult<u64>;

    /// The audit entries one account raised.
    #[method(name = "cerulea_getAuditByActor")]
    fn get_audit_by_actor(
        &self,
        actor: AccountId,
        limit: u32,
        deployment: Option<String>,
    ) -> RpcResult<Vec<AuditReply<AccountId>>>;

    /// Resolve the capability matrix for one request.
    #[method(name = "cerulea_resolvePermission")]
    fn resolve_permission(
        &self,
        actor: AccountId,
        collection: String,
        action: String,
        deployment: Option<String>,
    ) -> RpcResult<PermissionReply>;

    /// What one role reaches in one collection.
    #[method(name = "cerulea_simulateAsRole")]
    fn simulate_as_role(
        &self,
        role: String,
        collection: String,
        deployment: Option<String>,
    ) -> RpcResult<RoleReply>;

    /// Evaluate one view as one caller.
    #[method(name = "cerulea_evaluateView")]
    fn evaluate_view(
        &self,
        view: String,
        actor: AccountId,
        deployment: Option<String>,
    ) -> RpcResult<ViewReply>;

    /// The workflows one deployment declares.
    #[method(name = "cerulea_getWorkflows")]
    fn get_workflows(&self, deployment: Option<String>) -> RpcResult<Vec<String>>;

    /// Where one process stands.
    #[method(name = "cerulea_getWorkflowInstance")]
    fn get_workflow_instance(
        &self,
        workflow: String,
        instance: u64,
        deployment: Option<String>,
    ) -> RpcResult<Option<WorkflowInstanceReply>>;

    /// Every process of one workflow.
    #[method(name = "cerulea_getWorkflowInstances")]
    fn get_workflow_instances(
        &self,
        workflow: String,
        limit: u32,
        deployment: Option<String>,
    ) -> RpcResult<Vec<WorkflowInstanceReply>>;

    /// Where the deployment stands against its licence.
    #[method(name = "cerulea_getLicenceState")]
    fn get_licence_state(&self, deployment: Option<String>) -> RpcResult<Option<LicenceReply>>;

    /// Report whether block production runs.
    #[method(name = "cerulea_getProductionPermitted")]
    fn get_production_permitted(&self) -> RpcResult<bool>;

    /// The external feeds one deployment declares.
    #[method(name = "cerulea_getOracleFeeds")]
    fn get_oracle_feeds(&self, deployment: Option<String>) -> RpcResult<Vec<String>>;

    /// The current value of one feed.
    #[method(name = "cerulea_getOracleValue")]
    fn get_oracle_value(
        &self,
        feed: String,
        deployment: Option<String>,
    ) -> RpcResult<Option<OracleReply>>;

    /// The recorded versions of one deployment.
    #[method(name = "cerulea_getVersions")]
    fn get_versions(&self, deployment: Option<String>) -> RpcResult<Vec<VersionReply>>;

    /// The migration plans with their rollback windows.
    #[method(name = "cerulea_getRollbackWindows")]
    fn get_rollback_windows(
        &self,
        deployment: Option<String>,
    ) -> RpcResult<Vec<RollbackWindowReply>>;

    /// The registered type names of one deployment.
    #[method(name = "cerulea_getSchemaTypes")]
    fn get_schema_types(&self, deployment: Option<String>) -> RpcResult<Vec<String>>;

    /// The canonical encoding of one registered type definition.
    #[method(name = "cerulea_getSchemaType")]
    fn get_schema_type(
        &self,
        name: String,
        deployment: Option<String>,
    ) -> RpcResult<Option<String>>;

    /// The actions one deployment declares.
    #[method(name = "cerulea_getActions")]
    fn get_actions(&self, deployment: Option<String>) -> RpcResult<Vec<String>>;

    /// What one action reads, writes and needs.
    #[method(name = "cerulea_getActionManifest")]
    fn get_action_manifest(
        &self,
        action: String,
        deployment: Option<String>,
    ) -> RpcResult<Option<ActionManifestReply>>;

    /// The accounts holding the Cerulea operator role.
    #[method(name = "cerulea_getOperators")]
    fn get_operators(&self, deployment: Option<String>) -> RpcResult<Vec<AccountId>>;
}

/// The platform reading surface over one client.
pub struct CeruleaPlatformRpc<C> {
    client: Arc<C>,
    security: RpcSecurityConfig,
}

impl<C> CeruleaPlatformRpc<C> {
    /// Build the surface over one client.
    pub fn new(client: Arc<C>, security: RpcSecurityConfig) -> Self {
        CeruleaPlatformRpc { client, security }
    }

    /// Refuse every call while the Cerulea extensions stay closed.
    fn extensions_open(&self) -> RpcResult<()> {
        if self.security.enable_cerulea_extensions {
            return Ok(());
        }
        Err(ErrorObjectOwned::owned(
            EXTENSION_DISABLED,
            "The Cerulea reading methods are closed on this node. Start the node with the Cerulea extensions flag to open them."
                .to_string(),
            None::<()>,
        ))
    }
}

impl<C> CeruleaPlatformRpc<C>
where
    C: ProvideRuntimeApi<Block>
        + HeaderBackend<Block>
        + BlockBackend<Block>
        + Send
        + Sync
        + 'static,
    C::Api: CeruleaPlatformApi<Block, AccountId>,
    C::Api: pallet_cerulea_dcf::DcfApi<Block, AccountId, u128, u32>,
    C::Api: pallet_cerulea_dvf::DvfApi<Block, u32, AccountId, <Block as BlockT>::Hash>,
    C::Api: pallet_cerulea_dvf::CeruleaAuthoringApi<Block, AccountId>,
{
    /// The hash of the best block.
    fn best(&self) -> <Block as BlockT>::Hash {
        self.client.info().best_hash
    }

    /// The deployment a call names, or the first deployment the chain
    /// holds.
    fn deployment_of(&self, named: Option<String>) -> RpcResult<DeploymentId> {
        if let Some(text) = named {
            let bytes = bytes_of(&text).ok_or_else(|| {
                problem("The deployment identifier carries characters outside hexadecimal. Supply the content address of the Blueprint as a hex string.")
            })?;
            let held: DeploymentId = bytes.as_slice().try_into().map_err(|_| {
                problem("The deployment identifier carries another length. Supply the thirty two byte content address of the Blueprint.")
            })?;
            return Ok(held);
        }
        let api = self.client.runtime_api();
        let held = api
            .deployments(self.best())
            .map_err(|_| problem("The deployment list failed to load. Check that the node runtime matches the chain specification."))?;
        held.into_iter().next().ok_or_else(|| {
            problem("The chain holds no installed Blueprint. Install a Blueprint before reading a deployment.")
        })
    }

    /// The header of one height.
    fn header_at(&self, number: u32) -> RpcResult<<Block as BlockT>::Header> {
        let hash = self
            .client
            .hash(number)
            .map_err(|_| problem("The block index failed to load. Check that the node database is present."))?
            .ok_or_else(|| {
                problem("The chain holds no block at that height. Name a height at or below the head of the chain.")
            })?;
        self.client
            .header(hash)
            .map_err(|_| {
                problem("The block header failed to load. Check that the node database is present.")
            })?
            .ok_or_else(|| {
                problem(
                    "The chain holds no header for that block. Name a block the node still holds.",
                )
            })
    }

    /// Build the detail of one block.
    fn detail_of(&self, number: u32) -> RpcResult<BlockDetail> {
        use cerulea_consensus::engines::authoring::seal::extract_seal;
        use cerulea_consensus::engines::commit::aggregate::AggregateSignature;
        use cerulea_consensus::engines::execution::roots::StateRoots;

        let header = self.header_at(number)?;
        let hash = header.hash();
        let seal = extract_seal(&header);
        let api = self.client.runtime_api();

        let (slot, slot_rank, sealed_at, author) = match seal.as_ref() {
            Some(held) => (
                Some(held.claim.slot.0),
                Some(held.claim.rank.0),
                Some(held.claim.moment_ms),
                Some(hex_of(held.claim.author.as_ref())),
            ),
            None => (None, None, None, None),
        };

        let committee = match slot {
            Some(held) => {
                let mut parent = [0u8; 32];
                let reference = header.parent_hash();
                let bytes = reference.as_ref();
                if bytes.len() == parent.len() {
                    parent.copy_from_slice(bytes);
                }
                api.slot_committee(self.best(), held, parent)
                    .map(|seats| {
                        seats
                            .into_iter()
                            .map(|seat| CommitteeSeat {
                                rank: seat.rank,
                                validator: seat.validator,
                                standing: u64::try_from(seat.seat_value).unwrap_or(u64::MAX),
                            })
                            .collect()
                    })
                    .unwrap_or_default()
            }
            None => Vec::new(),
        };

        let roots = StateRoots::from_digest(header.digest());
        let (domain_roots, combined) = match roots {
            Some(held) => (
                held.domains.iter().map(|root| hex_of(&root.0)).collect(),
                Some(hex_of(&held.combined.0)),
            ),
            None => (Vec::new(), None),
        };

        let body = self
            .client
            .block_body(hash)
            .map_err(|_| {
                problem("The block body failed to load. Check that the node database is present.")
            })?
            .unwrap_or_default();

        let commit_weight = AggregateSignature::from_digest(header.digest())
            .map(|_| api.get_vote_tally(self.best(), hash).unwrap_or_default())
            .unwrap_or_else(|| api.get_vote_tally(self.best(), hash).unwrap_or_default());

        let finalised = api
            .get_finality_info(self.best(), number)
            .map(|held| held.is_finalized)
            .unwrap_or(false);

        Ok(BlockDetail {
            number,
            hash: hex_of(hash.as_ref()),
            parent_hash: hex_of(header.parent_hash().as_ref()),
            state_root: hex_of(header.state_root().as_ref()),
            extrinsics_root: hex_of(header.extrinsics_root().as_ref()),
            author,
            slot,
            slot_rank,
            sealed_at,
            committee,
            domain_roots,
            combined_domain_root: combined,
            transaction_count: u32::try_from(body.len()).unwrap_or(u32::MAX),
            commit_weight,
            finalised,
        })
    }

    /// Build the detail of every transaction in one block.
    fn transactions_in(&self, number: u32) -> RpcResult<Vec<TransactionDetail>> {
        use frame_support::traits::GetCallMetadata;
        use sp_runtime::traits::Hash as HashT;

        let header = self.header_at(number)?;
        let hash = header.hash();
        let body = self
            .client
            .block_body(hash)
            .map_err(|_| {
                problem("The block body failed to load. Check that the node database is present.")
            })?
            .unwrap_or_default();
        let api = self.client.runtime_api();
        let deployment = self.deployment_of(None).ok();
        let finalised = api
            .get_finality_info(self.best(), number)
            .map(|held| held.is_finalized)
            .unwrap_or(false);

        let mut found = Vec::new();
        for (index, extrinsic) in body.iter().enumerate() {
            let encoded = codec::Encode::encode(extrinsic);
            let transaction_hash = <<Block as BlockT>::Header as HeaderT>::Hashing::hash(&encoded);
            let decoded = cerulea_runtime::UncheckedExtrinsic::decode(&mut encoded.as_slice());
            let (actor, action, class, domain, events) = match decoded {
                Ok(held) => {
                    let metadata = held.function.get_call_metadata();
                    let action = format!("{}.{}", metadata.pallet_name, metadata.function_name);
                    let actor = match &held.preamble {
                        sp_runtime::generic::Preamble::Signed(address, _, _) => {
                            Some(format!("{:?}", address))
                        }
                        _ => None,
                    };
                    let (class, domain, events) =
                        self.classify(&held.function, deployment, &action);
                    (actor, action, class, domain, events)
                }
                Err(_) => (
                    None,
                    "unreadable".to_string(),
                    "routine".to_string(),
                    Vec::new(),
                    Vec::new(),
                ),
            };
            found.push(TransactionDetail {
                hash: hex_of(transaction_hash.as_ref()),
                block_number: number,
                block_hash: hex_of(hash.as_ref()),
                index: u32::try_from(index).unwrap_or(u32::MAX),
                actor,
                action,
                class,
                domain,
                status: if finalised {
                    "finalised".to_string()
                } else {
                    "included".to_string()
                },
                encoded: hex_of(&encoded),
                events,
            });
        }
        Ok(found)
    }

    /// The class, the conflict domain and the events one call carries.
    ///
    /// An action of the Blueprint carries its own class and its own
    /// manifest, so the interpreter call reads the declaration. Every other
    /// call takes the class its module carries.
    fn classify(
        &self,
        call: &cerulea_runtime::RuntimeCall,
        deployment: Option<DeploymentId>,
        action: &str,
    ) -> (String, Vec<String>, Vec<String>) {
        if let cerulea_runtime::RuntimeCall::Logic(pallet_cerulea_logic::Call::execute_action {
            deployment: named,
            action: name,
            ..
        }) = call
        {
            let api = self.client.runtime_api();
            if let Ok(Some(held)) = api.action_manifest(self.best(), *named, name.to_vec()) {
                return (
                    String::from_utf8_lossy(&held.class).into_owned(),
                    held.writes
                        .iter()
                        .map(|held| String::from_utf8_lossy(held).into_owned())
                        .collect(),
                    held.emits
                        .iter()
                        .map(|held| String::from_utf8_lossy(held).into_owned())
                        .collect(),
                );
            }
        }
        let _ = deployment;
        let class = match action.split('.').next().unwrap_or_default() {
            "Governance" | "Evolution" | "Operator" | "Tier" => "constitutional",
            "Schema" | "Permission" | "Identity" | "Collections" => "structural",
            "Balances" => "financial",
            _ => "routine",
        };
        (class.to_string(), Vec::new(), Vec::new())
    }

    /// Build the detail of one validator.
    fn validator_detail(&self, account: AccountId) -> RpcResult<ValidatorDetail> {
        let api = self.client.runtime_api();
        let best = self.best();
        let breakdown = api
            .get_validator_score_breakdown(best, account.clone())
            .ok()
            .flatten();
        let uptime = api
            .get_validator_uptime(best, account.clone())
            .ok()
            .flatten();
        let (authored_blocks, missed_slots) = api
            .get_validator_participation(best, account.clone())
            .unwrap_or((0, 0));
        // The counters come from the custody record, which counts every
        // settled challenge. The history log keeps the newest challenges
        // alone, so it serves the list and leaves the counting here.
        let counts = api
            .custody_counts(best, account.clone())
            .unwrap_or_default();
        let answered = counts.answered;
        let missed = counts.missed;
        let faults = counts.faults;
        let custody_share = counts.share;
        let penalties = api
            .get_slashing_history(best, account.clone())
            .map(|held| u32::try_from(held.len()).unwrap_or(u32::MAX))
            .unwrap_or(0);

        Ok(ValidatorDetail {
            account: account.clone(),
            trust_score: breakdown.as_ref().map(|held| held.final_score).unwrap_or(0),
            standing: breakdown.as_ref().map(|held| held.trust_score).unwrap_or(0),
            performance: breakdown.as_ref().map(|held| held.pos_score).unwrap_or(0),
            custody: custody_share,
            stake: api.get_validator_stake(best, account.clone()).unwrap_or(0),
            authored_blocks,
            missed_slots,
            uptime_percent: uptime
                .as_ref()
                .map(|held| held.uptime_percentage)
                .unwrap_or(0),
            active: api
                .is_validator_active(best, account.clone())
                .unwrap_or(false),
            custody_answered: answered,
            custody_missed: missed,
            custody_faults: faults,
            penalties,
        })
    }
}

impl<C> CeruleaPlatformRpcApiServer for CeruleaPlatformRpc<C>
where
    C: ProvideRuntimeApi<Block>
        + HeaderBackend<Block>
        + BlockBackend<Block>
        + Send
        + Sync
        + 'static,
    C::Api: CeruleaPlatformApi<Block, AccountId>,
    C::Api: pallet_cerulea_dcf::DcfApi<Block, AccountId, u128, u32>,
    C::Api: pallet_cerulea_dvf::DvfApi<Block, u32, AccountId, <Block as BlockT>::Hash>,
    C::Api: pallet_cerulea_dvf::CeruleaAuthoringApi<Block, AccountId>,
{
    fn get_deployments(&self) -> RpcResult<Vec<String>> {
        self.extensions_open()?;
        let api = self.client.runtime_api();
        let held = api
            .deployments(self.best())
            .map_err(|_| problem("The deployment list failed to load. Check that the node runtime matches the chain specification."))?;
        Ok(held.iter().map(|id| hex_of(id)).collect())
    }

    fn get_block_detail(&self, block_number: u32) -> RpcResult<BlockDetail> {
        self.extensions_open()?;
        self.detail_of(block_number)
    }

    fn get_recent_blocks(&self, limit: u32, before: Option<u32>) -> RpcResult<Vec<BlockDetail>> {
        self.extensions_open()?;
        let head = self.client.info().best_number;
        let top = before.unwrap_or(head).min(head);
        let count = limit.clamp(1, 128);
        let mut found = Vec::new();
        let mut number = top;
        while found.len() < count as usize {
            found.push(self.detail_of(number)?);
            if number == 0 {
                break;
            }
            number -= 1;
        }
        Ok(found)
    }

    fn get_transaction(&self, transaction_hash: String) -> RpcResult<TransactionDetail> {
        self.extensions_open()?;
        let wanted = transaction_hash.trim().to_lowercase();
        let head = self.client.info().best_number;
        let mut number = head;
        loop {
            for held in self.transactions_in(number)? {
                if held.hash.to_lowercase() == wanted {
                    return Ok(held);
                }
            }
            if number == 0 {
                break;
            }
            number -= 1;
        }
        Err(problem(
            "The chain holds no transaction under that hash. Supply the hash of a transaction the chain already carried.",
        ))
    }

    fn get_recent_transactions(
        &self,
        limit: u32,
        before_block: Option<u32>,
    ) -> RpcResult<Vec<TransactionDetail>> {
        self.extensions_open()?;
        let head = self.client.info().best_number;
        let top = before_block.unwrap_or(head).min(head);
        let count = limit.clamp(1, 256) as usize;
        let mut found = Vec::new();
        let mut number = top;
        loop {
            let mut held = self.transactions_in(number)?;
            held.reverse();
            for one in held {
                if found.len() >= count {
                    return Ok(found);
                }
                found.push(one);
            }
            if number == 0 || found.len() >= count {
                break;
            }
            number -= 1;
        }
        Ok(found)
    }

    fn get_collections(&self, deployment: Option<String>) -> RpcResult<Vec<CollectionSummary>> {
        self.extensions_open()?;
        let held = self.deployment_of(deployment)?;
        self.client
            .runtime_api()
            .collections(self.best(), held)
            .map_err(|_| problem("The collection list failed to load. Check that the node runtime matches the chain specification."))
    }

    fn get_record(
        &self,
        collection: String,
        key: String,
        deployment: Option<String>,
    ) -> RpcResult<Option<RecordReply>> {
        self.extensions_open()?;
        let held = self.deployment_of(deployment)?;
        let key_bytes = bytes_of(&key).unwrap_or_else(|| key.clone().into_bytes());
        self.client
            .runtime_api()
            .record(self.best(), held, collection.into_bytes(), key_bytes)
            .map_err(|_| problem("The record failed to load. Check that the node runtime matches the chain specification."))
    }

    fn get_records(
        &self,
        collection: String,
        prefix: Option<String>,
        limit: u32,
        deployment: Option<String>,
    ) -> RpcResult<Vec<RecordReply>> {
        self.extensions_open()?;
        let held = self.deployment_of(deployment)?;
        let prefix_bytes = prefix
            .map(|text| bytes_of(&text).unwrap_or_else(|| text.into_bytes()))
            .unwrap_or_default();
        self.client
            .runtime_api()
            .records(
                self.best(),
                held,
                collection.into_bytes(),
                prefix_bytes,
                limit,
            )
            .map_err(|_| problem("The record list failed to load. Check that the node runtime matches the chain specification."))
    }

    fn verify_record_proof(&self, proof: String, root: String) -> RpcResult<bool> {
        self.extensions_open()?;
        let proof_bytes = bytes_of(&proof).ok_or_else(|| {
            problem("The proof carries characters outside hexadecimal. Supply the canonical encoding of the proof as a hex string.")
        })?;
        let root_bytes = bytes_of(&root).ok_or_else(|| {
            problem("The root carries characters outside hexadecimal. Supply the thirty two byte root as a hex string.")
        })?;
        let held: [u8; 32] = root_bytes.as_slice().try_into().map_err(|_| {
            problem("The root carries another length. Supply the thirty two byte collection root.")
        })?;
        self.client
            .runtime_api()
            .verify_record_proof(self.best(), proof_bytes, held)
            .map_err(|_| problem("The proof check failed to run. Check that the node runtime matches the chain specification."))
    }

    fn get_collection_root(
        &self,
        collection: String,
        deployment: Option<String>,
    ) -> RpcResult<Option<String>> {
        self.extensions_open()?;
        let held = self.deployment_of(deployment)?;
        let root = self
            .client
            .runtime_api()
            .collection_root(self.best(), held, collection.into_bytes())
            .map_err(|_| problem("The collection root failed to load. Check that the node runtime matches the chain specification."))?;
        Ok(root.map(|found| hex_of(&found)))
    }

    fn get_validator_detail(&self, account: AccountId) -> RpcResult<ValidatorDetail> {
        self.extensions_open()?;
        self.validator_detail(account)
    }

    fn get_validator_details(&self) -> RpcResult<Vec<ValidatorDetail>> {
        self.extensions_open()?;
        let api = self.client.runtime_api();
        let validators = api
            .get_active_validators(self.best())
            .map_err(|_| problem("The validator set failed to load. Check that the node runtime matches the chain specification."))?;
        validators
            .into_iter()
            .map(|held| self.validator_detail(held))
            .collect()
    }

    fn get_validator_keys(&self) -> RpcResult<Vec<ValidatorKeyPair>> {
        self.extensions_open()?;
        let api = self.client.runtime_api();
        let held = api
            .active_validator_keys(self.best())
            .map_err(|_| problem("The validator key records failed to load. Check that the node runtime matches the chain specification."))?;
        Ok(held
            .into_iter()
            .map(|(validator, keys)| ValidatorKeyPair {
                validator,
                draw_key: hex_of(&keys.draw),
                seal_key: hex_of(&keys.seal),
            })
            .collect())
    }

    fn get_slot_committee(&self, slot: u64) -> RpcResult<Vec<CommitteeSeat>> {
        self.extensions_open()?;
        let api = self.client.runtime_api();
        let best = self.best();
        let mut parent = [0u8; 32];
        let bytes = best.as_ref();
        if bytes.len() == parent.len() {
            parent.copy_from_slice(bytes);
        }
        let seats = api
            .slot_committee(best, slot, parent)
            .map_err(|_| problem("The slot committee failed to load. Check that the node runtime matches the chain specification."))?;
        Ok(seats
            .into_iter()
            .map(|seat| CommitteeSeat {
                rank: seat.rank,
                validator: seat.validator,
                standing: u64::try_from(seat.seat_value).unwrap_or(u64::MAX),
            })
            .collect())
    }

    fn get_custody_history(
        &self,
        account: AccountId,
        limit: Option<u32>,
    ) -> RpcResult<Vec<CustodyReply<AccountId>>> {
        self.extensions_open()?;
        self.client
            .runtime_api()
            .custody_history(self.best(), account, limit.unwrap_or(64))
            .map_err(|_| problem("The custody history failed to load. Check that the node runtime matches the chain specification."))
    }

    fn get_consensus_profile(&self) -> RpcResult<ConsensusProfileReply> {
        self.extensions_open()?;
        self.client
            .runtime_api()
            .consensus_profile(self.best())
            .map_err(|_| problem("The consensus profile failed to load. Check that the node runtime matches the chain specification."))
    }

    fn list_proposals(
        &self,
        limit: Option<u32>,
        deployment: Option<String>,
    ) -> RpcResult<Vec<ProposalReply>> {
        self.extensions_open()?;
        let held = self.deployment_of(deployment)?;
        self.client
            .runtime_api()
            .proposals(self.best(), held, limit.unwrap_or(64))
            .map_err(|_| problem("The proposal list failed to load. Check that the node runtime matches the chain specification."))
    }

    fn get_proposal(
        &self,
        proposal_id: u64,
        deployment: Option<String>,
    ) -> RpcResult<Option<ProposalReply>> {
        self.extensions_open()?;
        let held = self.deployment_of(deployment)?;
        self.client
            .runtime_api()
            .proposal(self.best(), held, proposal_id)
            .map_err(|_| problem("The proposal failed to load. Check that the node runtime matches the chain specification."))
    }

    fn get_proposal_votes(
        &self,
        proposal_id: u64,
        deployment: Option<String>,
    ) -> RpcResult<Vec<VoteReply<AccountId>>> {
        self.extensions_open()?;
        let held = self.deployment_of(deployment)?;
        self.client
            .runtime_api()
            .proposal_votes(self.best(), held, proposal_id)
            .map_err(|_| problem("The vote list failed to load. Check that the node runtime matches the chain specification."))
    }

    fn get_executed_changes(
        &self,
        limit: Option<u32>,
        deployment: Option<String>,
    ) -> RpcResult<Vec<ProposalReply>> {
        self.extensions_open()?;
        let held = self.deployment_of(deployment)?;
        self.client
            .runtime_api()
            .executed_changes(self.best(), held, limit.unwrap_or(64))
            .map_err(|_| problem("The executed change list failed to load. Check that the node runtime matches the chain specification."))
    }

    fn get_audit_log(
        &self,
        from: u64,
        limit: u32,
        deployment: Option<String>,
    ) -> RpcResult<Vec<AuditReply<AccountId>>> {
        self.extensions_open()?;
        let held = self.deployment_of(deployment)?;
        self.client
            .runtime_api()
            .audit_entries(self.best(), held, from, limit)
            .map_err(|_| problem("The audit log failed to load. Check that the node runtime matches the chain specification."))
    }

    fn get_audit_length(&self, deployment: Option<String>) -> RpcResult<u64> {
        self.extensions_open()?;
        let held = self.deployment_of(deployment)?;
        self.client
            .runtime_api()
            .audit_length(self.best(), held)
            .map_err(|_| problem("The audit log length failed to load. Check that the node runtime matches the chain specification."))
    }

    fn get_audit_by_actor(
        &self,
        actor: AccountId,
        limit: u32,
        deployment: Option<String>,
    ) -> RpcResult<Vec<AuditReply<AccountId>>> {
        self.extensions_open()?;
        let held = self.deployment_of(deployment)?;
        self.client
            .runtime_api()
            .audit_by_actor(self.best(), held, actor, limit)
            .map_err(|_| problem("The audit log failed to load. Check that the node runtime matches the chain specification."))
    }

    fn resolve_permission(
        &self,
        actor: AccountId,
        collection: String,
        action: String,
        deployment: Option<String>,
    ) -> RpcResult<PermissionReply> {
        self.extensions_open()?;
        let held = self.deployment_of(deployment)?;
        self.client
            .runtime_api()
            .resolve_permission(
                self.best(),
                held,
                actor,
                collection.into_bytes(),
                action.into_bytes(),
            )
            .map_err(|_| problem("The capability matrix failed to load. Check that the node runtime matches the chain specification."))
    }

    fn simulate_as_role(
        &self,
        role: String,
        collection: String,
        deployment: Option<String>,
    ) -> RpcResult<RoleReply> {
        self.extensions_open()?;
        let held = self.deployment_of(deployment)?;
        self.client
            .runtime_api()
            .simulate_as_role(self.best(), held, role.into_bytes(), collection.into_bytes())
            .map_err(|_| problem("The role preview failed to run. Check that the node runtime matches the chain specification."))
    }

    fn evaluate_view(
        &self,
        view: String,
        actor: AccountId,
        deployment: Option<String>,
    ) -> RpcResult<ViewReply> {
        self.extensions_open()?;
        let held = self.deployment_of(deployment)?;
        self.client
            .runtime_api()
            .evaluate_view(self.best(), held, actor, view.into_bytes())
            .map_err(|_| problem("The view failed to evaluate. Check that the node runtime matches the chain specification."))
    }

    fn get_workflows(&self, deployment: Option<String>) -> RpcResult<Vec<String>> {
        self.extensions_open()?;
        let held = self.deployment_of(deployment)?;
        let names = self
            .client
            .runtime_api()
            .workflow_names(self.best(), held)
            .map_err(|_| problem("The workflow list failed to load. Check that the node runtime matches the chain specification."))?;
        Ok(names
            .iter()
            .map(|name| String::from_utf8_lossy(name).into_owned())
            .collect())
    }

    fn get_workflow_instance(
        &self,
        workflow: String,
        instance: u64,
        deployment: Option<String>,
    ) -> RpcResult<Option<WorkflowInstanceReply>> {
        self.extensions_open()?;
        let held = self.deployment_of(deployment)?;
        self.client
            .runtime_api()
            .workflow_instance(self.best(), held, workflow.into_bytes(), instance)
            .map_err(|_| problem("The process failed to load. Check that the node runtime matches the chain specification."))
    }

    fn get_workflow_instances(
        &self,
        workflow: String,
        limit: u32,
        deployment: Option<String>,
    ) -> RpcResult<Vec<WorkflowInstanceReply>> {
        self.extensions_open()?;
        let held = self.deployment_of(deployment)?;
        self.client
            .runtime_api()
            .workflow_instances(self.best(), held, workflow.into_bytes(), limit)
            .map_err(|_| problem("The process list failed to load. Check that the node runtime matches the chain specification."))
    }

    fn get_licence_state(&self, deployment: Option<String>) -> RpcResult<Option<LicenceReply>> {
        self.extensions_open()?;
        let held = self.deployment_of(deployment)?;
        self.client
            .runtime_api()
            .licence_state(self.best(), held)
            .map_err(|_| problem("The licence state failed to load. Check that the node runtime matches the chain specification."))
    }

    fn get_production_permitted(&self) -> RpcResult<bool> {
        self.extensions_open()?;
        self.client
            .runtime_api()
            .production_permitted(self.best())
            .map_err(|_| problem("The production gate failed to load. Check that the node runtime matches the chain specification."))
    }

    fn get_oracle_feeds(&self, deployment: Option<String>) -> RpcResult<Vec<String>> {
        self.extensions_open()?;
        let held = self.deployment_of(deployment)?;
        let names = self
            .client
            .runtime_api()
            .oracle_feeds(self.best(), held)
            .map_err(|_| problem("The feed list failed to load. Check that the node runtime matches the chain specification."))?;
        Ok(names
            .iter()
            .map(|name| String::from_utf8_lossy(name).into_owned())
            .collect())
    }

    fn get_oracle_value(
        &self,
        feed: String,
        deployment: Option<String>,
    ) -> RpcResult<Option<OracleReply>> {
        self.extensions_open()?;
        let held = self.deployment_of(deployment)?;
        self.client
            .runtime_api()
            .oracle_value(self.best(), held, feed.into_bytes())
            .map_err(|_| problem("The feed value failed to load. Check that the node runtime matches the chain specification."))
    }

    fn get_versions(&self, deployment: Option<String>) -> RpcResult<Vec<VersionReply>> {
        self.extensions_open()?;
        let held = self.deployment_of(deployment)?;
        self.client
            .runtime_api()
            .versions(self.best(), held)
            .map_err(|_| problem("The version registry failed to load. Check that the node runtime matches the chain specification."))
    }

    fn get_rollback_windows(
        &self,
        deployment: Option<String>,
    ) -> RpcResult<Vec<RollbackWindowReply>> {
        self.extensions_open()?;
        let held = self.deployment_of(deployment)?;
        self.client
            .runtime_api()
            .rollback_windows(self.best(), held)
            .map_err(|_| problem("The rollback windows failed to load. Check that the node runtime matches the chain specification."))
    }

    fn get_schema_types(&self, deployment: Option<String>) -> RpcResult<Vec<String>> {
        self.extensions_open()?;
        let held = self.deployment_of(deployment)?;
        let names = self
            .client
            .runtime_api()
            .type_names(self.best(), held)
            .map_err(|_| problem("The type registry failed to load. Check that the node runtime matches the chain specification."))?;
        Ok(names
            .iter()
            .map(|name| String::from_utf8_lossy(name).into_owned())
            .collect())
    }

    fn get_schema_type(
        &self,
        name: String,
        deployment: Option<String>,
    ) -> RpcResult<Option<String>> {
        self.extensions_open()?;
        let held = self.deployment_of(deployment)?;
        let definition = self
            .client
            .runtime_api()
            .type_definition(self.best(), held, name.into_bytes())
            .map_err(|_| problem("The type definition failed to load. Check that the node runtime matches the chain specification."))?;
        Ok(definition.map(|found| hex_of(&found)))
    }

    fn get_actions(&self, deployment: Option<String>) -> RpcResult<Vec<String>> {
        self.extensions_open()?;
        let held = self.deployment_of(deployment)?;
        let names = self
            .client
            .runtime_api()
            .action_names(self.best(), held)
            .map_err(|_| problem("The action list failed to load. Check that the node runtime matches the chain specification."))?;
        Ok(names
            .iter()
            .map(|name| String::from_utf8_lossy(name).into_owned())
            .collect())
    }

    fn get_action_manifest(
        &self,
        action: String,
        deployment: Option<String>,
    ) -> RpcResult<Option<ActionManifestReply>> {
        self.extensions_open()?;
        let held = self.deployment_of(deployment)?;
        self.client
            .runtime_api()
            .action_manifest(self.best(), held, action.into_bytes())
            .map_err(|_| problem("The action manifest failed to load. Check that the node runtime matches the chain specification."))
    }

    fn get_operators(&self, deployment: Option<String>) -> RpcResult<Vec<AccountId>> {
        self.extensions_open()?;
        let held = self.deployment_of(deployment)?;
        self.client
            .runtime_api()
            .operator_accounts(self.best(), held)
            .map_err(|_| problem("The operator list failed to load. Check that the node runtime matches the chain specification."))
    }
}
