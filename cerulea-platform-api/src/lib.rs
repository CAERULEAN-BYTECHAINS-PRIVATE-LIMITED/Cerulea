//! The platform runtime interfaces.
//!
//! The sixteen platform modules hold the schema registry, the records, the
//! capability matrix, the interpreter, the processes, the change process,
//! the version registry, the external feeds, the licence and the event
//! log. Cerulea Explorer, Cerulea Dashboard and the Cerulea APIs chain
//! client read all of it through the calls declared here.
//!
//! # Why the replies carry plain shapes
//!
//! A reader outside the Blockchain holds no runtime types. Each reply here
//! therefore carries plain bytes, plain numbers and plain names. The
//! runtime turns its own storage types into these shapes, so a reader
//! decodes one small crate rather than sixteen modules.
//!
//! # Names
//!
//! Every name in a reply is the raw bytes of an identifier the Blueprint
//! declared. A reader reads the bytes as text.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::vec::Vec;
use codec::{Codec, Decode, Encode};
use scale_info::TypeInfo;
use serde::{Deserialize, Serialize};

/// The identifier of one deployment.
///
/// A deployment is one installed Blueprint on one chain. The identifier is
/// the content address of the Blueprint.
pub type DeploymentId = [u8; 32];

/// The visibility class of one collection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, TypeInfo, Serialize, Deserialize)]
pub enum PrivacyLabel {
    /// Readable by anyone the capability matrix permits.
    Open,
    /// The Blockchain holds a commitment while the payload lives elsewhere.
    HashOnly,
    /// The Blockchain holds the payload sealed under a role key.
    Encrypted,
    /// The Blockchain holds a commitment and an external store holds the
    /// payload.
    External,
}

/// One collection of one deployment.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo, Serialize, Deserialize)]
pub struct CollectionSummary {
    /// The collection name.
    pub name: Vec<u8>,
    /// The record type the collection holds.
    pub record_type: Vec<u8>,
    /// The visibility class.
    pub privacy: PrivacyLabel,
    /// The number of records the collection holds.
    pub records: u32,
    /// The inclusion tree root of the collection.
    pub root: [u8; 32],
    /// The version the collection definition carries.
    pub version: u32,
    /// A retired collection takes no further record.
    pub retired: bool,
    /// One plain sentence stating what the collection holds.
    pub summary: Vec<u8>,
}

/// One version of one record.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo, Serialize, Deserialize)]
pub struct RecordVersionEntry {
    /// The version number the write produced.
    pub version: u32,
    /// The hash of the bytes the write stored.
    pub commitment: [u8; 32],
    /// The block the write landed in.
    pub block: u32,
    /// The wall clock time of the write, in milliseconds.
    pub at_millis: u64,
}

/// One record with its commitment, its history and its proof.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo, Serialize, Deserialize)]
pub struct RecordReply {
    /// The collection the record sits in.
    pub collection: Vec<u8>,
    /// The record key.
    pub key: Vec<u8>,
    /// The stored bytes, where the visibility class permits a reader to
    /// see them. A sealed record answers with nothing.
    pub value: Option<Vec<u8>>,
    /// The hash of the stored bytes.
    pub commitment: [u8; 32],
    /// The version the record carries now.
    pub version: u32,
    /// The block the record first landed in.
    pub created_block: u32,
    /// The block the record last changed in.
    pub changed_block: u32,
    /// The visibility class of the collection.
    pub privacy: PrivacyLabel,
    /// The version history, oldest entry first.
    pub history: Vec<RecordVersionEntry>,
    /// The canonical encoding of the inclusion proof.
    pub proof: Option<Vec<u8>>,
    /// The inclusion tree root the proof folds to.
    pub root: [u8; 32],
}

/// What one role reaches in one collection.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo, Serialize, Deserialize)]
pub struct RoleReply {
    /// The role.
    pub role: Vec<u8>,
    /// The collection.
    pub collection: Vec<u8>,
    /// The role reads records.
    pub read: bool,
    /// The role adds records.
    pub insert: bool,
    /// The role changes records.
    pub update: bool,
    /// The role removes records.
    pub delete: bool,
    /// The fields the role reads. An empty list beside `every_field` means
    /// the role reads every field.
    pub readable_fields: Vec<Vec<u8>>,
    /// The role reads every field of the record.
    pub every_field: bool,
    /// The actions the role runs.
    pub actions: Vec<Vec<u8>>,
    /// One plain sentence stating what the role reaches.
    pub statement: Vec<u8>,
}

/// What the capability matrix answered for one request.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo, Serialize, Deserialize)]
pub struct PermissionReply {
    /// The resolver admitted the request.
    pub admitted: bool,
    /// The role the grant came through.
    pub role: Option<Vec<u8>>,
    /// The fields the grant reaches.
    pub fields: Vec<Vec<u8>>,
    /// The grant reaches every field.
    pub every_field: bool,
    /// One plain sentence stating the problem, where the resolver refused.
    pub problem: Vec<u8>,
    /// One plain sentence naming the fix, where the resolver refused.
    pub fix: Vec<u8>,
}

/// The rows one view produced.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo, Serialize, Deserialize)]
pub struct ViewReply {
    /// The view name.
    pub view: Vec<u8>,
    /// The evaluation produced rows.
    pub evaluated: bool,
    /// The number of rows the view produced.
    pub rows: u32,
    /// The canonical encoding of each row, in the order the view produced
    /// them.
    pub encoded_rows: Vec<Vec<u8>>,
    /// The digest over every row.
    pub digest: [u8; 32],
    /// One plain sentence stating the problem, where the evaluation
    /// refused.
    pub problem: Vec<u8>,
}

/// Where one process stands.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo, Serialize, Deserialize)]
pub struct WorkflowInstanceReply {
    /// The workflow the process runs.
    pub workflow: Vec<u8>,
    /// The identifier of the process.
    pub instance: u64,
    /// The record the process drives.
    pub record: Vec<u8>,
    /// The state the record holds now.
    pub state: Vec<u8>,
    /// Where the process stands. One of running, settled, compensating or
    /// compensated.
    pub status: Vec<u8>,
    /// The wall clock time the record entered its present state.
    pub entered_at: u64,
    /// The wall clock time the process started.
    pub started_at: u64,
    /// The block the process started in.
    pub started_block: u32,
    /// Every completed step, naming the transition, the state it left and
    /// the state it reached.
    pub history: Vec<(Vec<u8>, Vec<u8>, Vec<u8>, u64)>,
    /// The timers and deadlines scheduled for the present state, with the
    /// wall clock time each falls due at.
    pub scheduled: Vec<(Vec<u8>, u64)>,
}

/// One proposal of the change process.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo, Serialize, Deserialize)]
pub struct ProposalReply {
    /// The identifier of the proposal.
    pub id: u64,
    /// What the proposal changes. One of schema, permission, policy,
    /// validator set or upgrade.
    pub kind: Vec<u8>,
    /// The class of work the change carries. One of routine, financial,
    /// structural or constitutional.
    pub class: Vec<u8>,
    /// The share of the electorate the class needs, in percent.
    pub threshold_percent: u8,
    /// The content hash of the change.
    pub payload_hash: [u8; 32],
    /// Where the proposal stands. One of open, approved, rejected,
    /// executed or cancelled.
    pub status: Vec<u8>,
    /// The wall clock time the proposal opened at.
    pub opened_at: u64,
    /// The wall clock time the vote closes at.
    pub deadline: u64,
    /// The weight that agrees.
    pub approve_weight: u64,
    /// The weight that declines.
    pub reject_weight: u64,
    /// The count of voters that took part.
    pub voters: u32,
    /// The weight of the whole electorate at the moment of the decision.
    pub roll_weight: u64,
    /// The wall clock time the decision landed at.
    pub decided_at: Option<u64>,
    /// The block the execution landed in.
    pub executed_block: Option<u32>,
}

/// One vote with its weight.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo, Serialize, Deserialize)]
pub struct VoteReply<AccountId> {
    /// The account that voted.
    pub voter: AccountId,
    /// The voter agreed.
    pub approve: bool,
    /// The weight the voter carried.
    pub weight: u64,
    /// The wall clock time the vote landed at.
    pub at_millis: u64,
}

/// One entry of the audit log.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo, Serialize, Deserialize)]
pub struct AuditReply<AccountId> {
    /// The position of the entry in the log of the deployment.
    pub sequence: u64,
    /// The event name.
    pub event: Vec<u8>,
    /// The module that recorded the work.
    pub module: Vec<u8>,
    /// The class of the work.
    pub class: Vec<u8>,
    /// The account the Blockchain records as the actor.
    pub actor: AccountId,
    /// The wall clock time of the entry, in milliseconds.
    pub at_millis: u64,
    /// The block the entry landed in.
    pub block: u32,
    /// One plain sentence stating why the work ran.
    pub reason: Vec<u8>,
    /// The canonical encoding of the detail the event carries.
    pub payload: Vec<u8>,
    /// The entry this record corrects.
    pub corrects: Option<u64>,
    /// The running digest of the log up to and including this entry.
    pub digest: [u8; 32],
}

/// Where one deployment stands against its licence.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo, Serialize, Deserialize)]
pub struct LicenceReply {
    /// The application the deployment runs. One of dapps, enterprise or
    /// government.
    pub application: Vec<u8>,
    /// The level inside that application, counting from one.
    pub level: u8,
    /// The features the licence enables.
    pub features: Vec<Vec<u8>>,
    /// The add-ons the licence enables.
    pub addons: Vec<Vec<u8>>,
    /// The seats the licence covers.
    pub seats: u32,
    /// The seats the deployment uses now.
    pub seats_used: u32,
    /// The wall clock time the licence lapses at.
    pub expires_at: u64,
    /// Where the deployment stands. One of live, lapsed or suspended.
    pub state: Vec<u8>,
    /// One plain sentence stating why block production stopped.
    pub suspension_reason: Vec<u8>,
    /// Block production runs.
    pub production_permitted: bool,
}

/// The current value of one external feed.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo, Serialize, Deserialize)]
pub struct OracleReply {
    /// The feed name.
    pub feed: Vec<u8>,
    /// The round the value came from.
    pub round: u64,
    /// The canonical encoding of the value.
    pub value: Vec<u8>,
    /// The wall clock time the round reached its requirement.
    pub attested_at: u64,
}

/// One recorded version of one deployment.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo, Serialize, Deserialize)]
pub struct VersionReply {
    /// The revision the Blueprint carries.
    pub revision: u32,
    /// The content address of the Blueprint this version installed.
    pub blueprint: [u8; 32],
    /// The wall clock time the version landed at.
    pub recorded_at: u64,
    /// The block the version landed in.
    pub recorded_block: u32,
    /// One plain sentence stating what the version changes.
    pub summary: Vec<u8>,
    /// The version the deployment runs now.
    pub active: bool,
}

/// One migration plan with its rollback window.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo, Serialize, Deserialize)]
pub struct RollbackWindowReply {
    /// The identifier of the plan.
    pub plan: u64,
    /// The collection the plan changes.
    pub collection: Vec<u8>,
    /// The version the plan starts from.
    pub from_version: u32,
    /// The version the plan produces.
    pub to_version: u32,
    /// Where the plan stands. One of planned, checked, applied, verified or
    /// rolled back.
    pub status: Vec<u8>,
    /// The plan breaks an existing integration.
    pub breaking: bool,
    /// The plan carries a reverse.
    pub reversible: bool,
    /// The span a rollback stays available for, in milliseconds.
    pub window_millis: u64,
    /// The wall clock time the rollback window closes at.
    pub deadline: Option<u64>,
    /// A rollback runs now.
    pub open: bool,
}

/// The class thresholds the Assurance Engine applies.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo, Serialize, Deserialize)]
pub struct ConsensusProfileReply {
    /// The share of weight a routine transaction needs, in percent.
    pub routine_percent: u8,
    /// The share of weight a financial transaction needs, in percent.
    pub financial_percent: u8,
    /// The share of weight a structural transaction needs, in percent.
    pub structural_percent: u8,
    /// The share of weight a constitutional transaction needs, in percent.
    pub constitutional_percent: u8,
    /// The number of seats one slot committee holds.
    pub committee_size: u32,
    /// The span between blocks, in milliseconds.
    pub block_time_millis: u64,
    /// The number of blocks one epoch holds.
    pub epoch_length: u32,
}

/// What one action reads, writes and needs.
///
/// The Access Manifest of a transaction comes from this declaration. The
/// Execution Engine partitions a block by the collections each transaction
/// writes.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo, Serialize, Deserialize)]
pub struct ActionManifestReply {
    /// The action name.
    pub action: Vec<u8>,
    /// The class of work the action carries.
    pub class: Vec<u8>,
    /// The collections the action reads.
    pub reads: Vec<Vec<u8>>,
    /// The collections the action writes. These name the conflict domain.
    pub writes: Vec<Vec<u8>>,
    /// The roles the action admits.
    pub roles: Vec<Vec<u8>>,
    /// The events the action raises.
    pub emits: Vec<Vec<u8>>,
}

/// One custody challenge and the answer it drew.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo, Serialize, Deserialize)]
pub struct CustodyReply<AccountId> {
    /// The validator the challenge reached.
    pub validator: AccountId,
    /// The block the challenge issued in.
    pub issued_block: u32,
    /// The block the answer falls due in.
    pub deadline_block: u32,
    /// The validator answered.
    pub answered: bool,
    /// The answer passed the check.
    pub verified: bool,
}

sp_api::decl_runtime_apis! {
    /// The reads the platform modules publish.
    ///
    /// Every call takes the deployment the read covers. A chain runs one
    /// deployment in the ordinary case, and `deployments` names every one
    /// the chain holds.
    pub trait CeruleaPlatformApi<AccountId> where AccountId: Codec {
        /// Every deployment the chain holds.
        fn deployments() -> Vec<DeploymentId>;

        /// The content address of the Blueprint one deployment runs.
        fn blueprint_address(deployment: DeploymentId) -> Option<[u8; 32]>;

        /// The canonical encoding of the Blueprint one deployment runs.
        fn blueprint(deployment: DeploymentId) -> Option<Vec<u8>>;

        /// The registered type names of one deployment.
        fn type_names(deployment: DeploymentId) -> Vec<Vec<u8>>;

        /// The canonical encoding of one registered type definition.
        fn type_definition(deployment: DeploymentId, name: Vec<u8>) -> Option<Vec<u8>>;

        /// Every collection of one deployment with its class and its count.
        fn collections(deployment: DeploymentId) -> Vec<CollectionSummary>;

        /// One record with its commitment, its history and its proof.
        fn record(
            deployment: DeploymentId,
            collection: Vec<u8>,
            key: Vec<u8>,
        ) -> Option<RecordReply>;

        /// The records of one collection whose key starts with the prefix.
        fn records(
            deployment: DeploymentId,
            collection: Vec<u8>,
            prefix: Vec<u8>,
            limit: u32,
        ) -> Vec<RecordReply>;

        /// The inclusion tree root of one collection.
        fn collection_root(deployment: DeploymentId, collection: Vec<u8>) -> Option<[u8; 32]>;

        /// Fold one inclusion proof and compare the result with a root.
        fn verify_record_proof(proof: Vec<u8>, root: [u8; 32]) -> bool;

        /// Resolve the capability matrix for one request.
        fn resolve_permission(
            deployment: DeploymentId,
            actor: AccountId,
            collection: Vec<u8>,
            action: Vec<u8>,
        ) -> PermissionReply;

        /// What one role reaches in one collection.
        fn simulate_as_role(
            deployment: DeploymentId,
            role: Vec<u8>,
            collection: Vec<u8>,
        ) -> RoleReply;

        /// Evaluate one view as one caller.
        fn evaluate_view(
            deployment: DeploymentId,
            actor: AccountId,
            view: Vec<u8>,
        ) -> ViewReply;

        /// The workflows one deployment declares.
        fn workflow_names(deployment: DeploymentId) -> Vec<Vec<u8>>;

        /// Where one process stands.
        fn workflow_instance(
            deployment: DeploymentId,
            workflow: Vec<u8>,
            instance: u64,
        ) -> Option<WorkflowInstanceReply>;

        /// Every process of one workflow.
        fn workflow_instances(
            deployment: DeploymentId,
            workflow: Vec<u8>,
            limit: u32,
        ) -> Vec<WorkflowInstanceReply>;

        /// The proposals of one deployment, newest first.
        fn proposals(deployment: DeploymentId, limit: u32) -> Vec<ProposalReply>;

        /// One proposal.
        fn proposal(deployment: DeploymentId, id: u64) -> Option<ProposalReply>;

        /// The votes one proposal drew.
        fn proposal_votes(deployment: DeploymentId, id: u64) -> Vec<VoteReply<AccountId>>;

        /// The proposals a module already carried out.
        fn executed_changes(deployment: DeploymentId, limit: u32) -> Vec<ProposalReply>;

        /// The number of entries the audit log of one deployment holds.
        fn audit_length(deployment: DeploymentId) -> u64;

        /// The entries of the audit log, from one position forward.
        fn audit_entries(
            deployment: DeploymentId,
            from: u64,
            limit: u32,
        ) -> Vec<AuditReply<AccountId>>;

        /// The entries one account raised.
        fn audit_by_actor(
            deployment: DeploymentId,
            actor: AccountId,
            limit: u32,
        ) -> Vec<AuditReply<AccountId>>;

        /// Where one deployment stands against its licence.
        fn licence_state(deployment: DeploymentId) -> Option<LicenceReply>;

        /// Report whether block production runs on this chain.
        ///
        /// A lapsed licence or a suspended deployment stops block
        /// production. Every existing record stays readable and exportable
        /// throughout.
        fn production_permitted() -> bool;

        /// The feeds one deployment declares.
        fn oracle_feeds(deployment: DeploymentId) -> Vec<Vec<u8>>;

        /// The current value of one feed.
        fn oracle_value(deployment: DeploymentId, feed: Vec<u8>) -> Option<OracleReply>;

        /// The recorded versions of one deployment.
        fn versions(deployment: DeploymentId) -> Vec<VersionReply>;

        /// The migration plans of one deployment with their rollback
        /// windows.
        fn rollback_windows(deployment: DeploymentId) -> Vec<RollbackWindowReply>;

        /// The accounts holding the Cerulea operator role.
        fn operator_accounts(deployment: DeploymentId) -> Vec<AccountId>;

        /// The class thresholds and the authoring figures the chain runs
        /// under.
        fn consensus_profile() -> ConsensusProfileReply;

        /// The custody challenge history of one validator.
        fn custody_history(validator: AccountId, limit: u32) -> Vec<CustodyReply<AccountId>>;

        /// What one action reads, writes and needs.
        fn action_manifest(
            deployment: DeploymentId,
            action: Vec<u8>,
        ) -> Option<ActionManifestReply>;

        /// The actions one deployment declares.
        fn action_names(deployment: DeploymentId) -> Vec<Vec<u8>>;
    }
}
