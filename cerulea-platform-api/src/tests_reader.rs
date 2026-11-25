//! Reader side tests of the platform runtime interface.
//!
//! A reader outside the Blockchain reaches the platform modules through the
//! calls declared in this crate. Each call encodes its arguments, names the
//! runtime entry point and decodes the reply. The tests below run every
//! call against a local runtime. The local runtime decodes the arguments,
//! answers from an in-memory model of one deployment and encodes the reply.
//! The tests assert the entry point name, the arguments that crossed and the
//! reply that came back.

use super::*;
use alloc::string::String;
use alloc::vec;
use codec::{Decode, Encode};
use sp_api::ApiError;
// The runtime interface macros name the version type through this path.
use sp_api::__private::RuntimeVersion;
use sp_runtime::generic::{Block as GenericBlock, Header};
use sp_runtime::traits::{BlakeTwo256, Block as BlockT, Hash as HashT};
use sp_runtime::OpaqueExtrinsic;
use std::sync::Mutex;

type ReaderBlock = GenericBlock<Header<u32, BlakeTwo256>, OpaqueExtrinsic>;
type At = <ReaderBlock as BlockT>::Hash;

const DEPLOYMENT: DeploymentId = [0x42; 32];
const OTHER: DeploymentId = [0x24; 32];
const REGISTRAR: u64 = 7;
const CLERK: u64 = 8;

fn text(value: &str) -> Vec<u8> {
    value.as_bytes().to_vec()
}

fn at() -> At {
    At::repeat_byte(0x01)
}

/// Decode the arguments of one call or name the call that failed.
fn args<A: Decode>(name: &'static str, params: &[u8]) -> Result<A, ApiError> {
    A::decode(&mut &params[..]).map_err(|error| ApiError::FailedToConvertParameter {
        function: name,
        parameter: "arguments",
        error,
    })
}

/// How the local runtime answers.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// Answer from the model.
    Model,
    /// Answer with bytes no reply shape decodes from.
    Damaged,
    /// Refuse every call with an application error.
    Refusing,
}

/// One deployment held in memory, the way the platform modules hold it.
/// One permission grant the local runtime answers with.
type Grant = (u64, Vec<u8>, Vec<u8>, Vec<u8>);

struct LocalRuntime {
    mode: Mode,
    calls: Mutex<Vec<&'static str>>,
    blueprint: Vec<u8>,
    types: Vec<(Vec<u8>, Vec<u8>)>,
    collections: Vec<CollectionSummary>,
    records: Vec<RecordReply>,
    grants: Vec<Grant>,
    roles: Vec<RoleReply>,
    views: Vec<(Vec<u8>, Vec<Vec<u8>>)>,
    workflows: Vec<WorkflowInstanceReply>,
    proposals: Vec<ProposalReply>,
    votes: Vec<(u64, VoteReply<u64>)>,
    audit: Vec<AuditReply<u64>>,
    licence: LicenceReply,
    feeds: Vec<OracleReply>,
    versions: Vec<VersionReply>,
    windows: Vec<RollbackWindowReply>,
    operators: Vec<u64>,
    custody: Vec<CustodyReply<u64>>,
    manifests: Vec<ActionManifestReply>,
}

fn record_at(key: &str, value: Vec<u8>) -> RecordReply {
    let commitment: [u8; 32] = BlakeTwo256::hash(&value).into();
    RecordReply {
        collection: text("parcel"),
        key: text(key),
        value: Some(value),
        commitment,
        version: 1,
        created_block: 3,
        changed_block: 3,
        privacy: PrivacyLabel::Open,
        history: vec![RecordVersionEntry {
            version: 1,
            commitment,
            block: 3,
            at_millis: 18_000,
        }],
        proof: Some(commitment.to_vec()),
        root: BlakeTwo256::hash(&commitment).into(),
    }
}

fn proposal(id: u64, status: &str, executed_block: Option<u32>) -> ProposalReply {
    ProposalReply {
        id,
        kind: text("schema"),
        class: text("structural"),
        threshold_percent: 80,
        payload_hash: [id as u8; 32],
        status: text(status),
        opened_at: id * 1_000,
        deadline: id * 1_000 + 500,
        approve_weight: 0,
        reject_weight: 0,
        voters: 0,
        roll_weight: 100,
        decided_at: executed_block.map(|_| id * 1_000 + 400),
        executed_block,
    }
}

fn audit_entry(sequence: u64, actor: u64, event: &str) -> AuditReply<u64> {
    AuditReply {
        sequence,
        event: text(event),
        module: text("state"),
        class: text("routine"),
        actor,
        at_millis: sequence * 6_000,
        block: sequence as u32,
        reason: text("The registrar recorded a parcel."),
        payload: sequence.encode(),
        corrects: None,
        digest: [sequence as u8; 32],
    }
}

impl LocalRuntime {
    fn new(mode: Mode) -> Self {
        let parcel_a = record_at("12/1", vec![1, 1]);
        let parcel_b = record_at("12/2", vec![2, 2]);
        let parcel_c = record_at("13/1", vec![3, 3]);
        LocalRuntime {
            mode,
            calls: Mutex::new(Vec::new()),
            blueprint: text("blueprint canonical bytes"),
            types: vec![(text("parcel_record"), vec![0x01, 0x02])],
            collections: vec![CollectionSummary {
                name: text("parcel"),
                record_type: text("parcel_record"),
                privacy: PrivacyLabel::Open,
                records: 3,
                root: parcel_a.root,
                version: 1,
                retired: false,
                summary: text("The parcel collection holds one record per parcel."),
            }],
            records: vec![parcel_a, parcel_b, parcel_c],
            grants: vec![(
                REGISTRAR,
                text("parcel"),
                text("record_transfer"),
                text("registrar"),
            )],
            roles: vec![RoleReply {
                role: text("registrar"),
                collection: text("parcel"),
                read: true,
                insert: true,
                update: true,
                delete: false,
                readable_fields: vec![],
                every_field: true,
                actions: vec![text("record_transfer")],
                statement: text("The registrar reads and records parcels."),
            }],
            views: vec![(text("open_parcels"), vec![vec![1], vec![2]])],
            workflows: (1..=3)
                .map(|instance| WorkflowInstanceReply {
                    workflow: text("transfer"),
                    instance,
                    record: text("12/1"),
                    state: text("filed"),
                    status: text("running"),
                    entered_at: instance * 100,
                    started_at: instance * 100,
                    started_block: instance as u32,
                    history: vec![],
                    scheduled: vec![(text("reminder"), instance * 100 + 60_000)],
                })
                .collect(),
            proposals: vec![
                proposal(1, "executed", Some(4)),
                proposal(2, "rejected", None),
                proposal(3, "open", None),
            ],
            votes: vec![(
                1,
                VoteReply {
                    voter: REGISTRAR,
                    approve: true,
                    weight: 90,
                    at_millis: 1_200,
                },
            )],
            audit: vec![
                audit_entry(0, REGISTRAR, "record.inserted"),
                audit_entry(1, CLERK, "record.read"),
                audit_entry(2, REGISTRAR, "record.updated"),
            ],
            licence: LicenceReply {
                application: text("government"),
                level: 3,
                features: vec![text("workflows")],
                addons: vec![],
                seats: 10,
                seats_used: 2,
                expires_at: 9_000_000,
                state: text("live"),
                suspension_reason: vec![],
                production_permitted: true,
            },
            feeds: vec![OracleReply {
                feed: text("rainfall_mm"),
                round: 12,
                value: 31u32.encode(),
                attested_at: 44_000,
            }],
            versions: vec![
                VersionReply {
                    revision: 1,
                    blueprint: [1u8; 32],
                    recorded_at: 100,
                    recorded_block: 1,
                    summary: text("The first revision."),
                    active: false,
                },
                VersionReply {
                    revision: 2,
                    blueprint: DEPLOYMENT,
                    recorded_at: 900,
                    recorded_block: 9,
                    summary: text("The revision adds the survey field."),
                    active: true,
                },
            ],
            windows: vec![RollbackWindowReply {
                plan: 1,
                collection: text("parcel"),
                from_version: 1,
                to_version: 2,
                status: text("applied"),
                breaking: false,
                reversible: true,
                window_millis: 86_400_000,
                deadline: Some(86_400_900),
                open: true,
            }],
            operators: vec![REGISTRAR],
            custody: (0..4)
                .map(|round| CustodyReply {
                    validator: if round % 2 == 0 { REGISTRAR } else { CLERK },
                    issued_block: round * 5,
                    deadline_block: round * 5 + 3,
                    answered: round != 2,
                    verified: round != 2,
                    settled: true,
                })
                .collect(),
            manifests: vec![ActionManifestReply {
                action: text("record_transfer"),
                class: text("financial"),
                reads: vec![text("parcel")],
                writes: vec![text("parcel")],
                roles: vec![text("registrar")],
                emits: vec![text("transfer_recorded")],
            }],
        }
    }

    fn calls(&self) -> Vec<&'static str> {
        self.calls.lock().expect("the call log opens").clone()
    }

    fn held(deployment: &DeploymentId) -> bool {
        deployment == &DEPLOYMENT
    }

    /// Answer one runtime entry point from the model.
    fn answer(&self, name: &'static str, params: &[u8]) -> Result<Vec<u8>, ApiError> {
        let reply = match name {
            "CeruleaPlatformApi_deployments" => {
                args::<()>(name, params)?;
                vec![DEPLOYMENT].encode()
            }
            "CeruleaPlatformApi_blueprint_address" => {
                let (deployment,): (DeploymentId,) = args(name, params)?;
                Some(deployment).filter(Self::held).encode()
            }
            "CeruleaPlatformApi_blueprint" => {
                let (deployment,): (DeploymentId,) = args(name, params)?;
                Some(self.blueprint.clone())
                    .filter(|_| Self::held(&deployment))
                    .encode()
            }
            "CeruleaPlatformApi_type_names" => {
                let (deployment,): (DeploymentId,) = args(name, params)?;
                self.types
                    .iter()
                    .filter(|_| Self::held(&deployment))
                    .map(|held| held.0.clone())
                    .collect::<Vec<_>>()
                    .encode()
            }
            "CeruleaPlatformApi_type_definition" => {
                let (deployment, wanted): (DeploymentId, Vec<u8>) = args(name, params)?;
                self.types
                    .iter()
                    .find(|held| Self::held(&deployment) && held.0 == wanted)
                    .map(|held| held.1.clone())
                    .encode()
            }
            "CeruleaPlatformApi_collections" => {
                let (deployment,): (DeploymentId,) = args(name, params)?;
                self.collections
                    .iter()
                    .filter(|_| Self::held(&deployment))
                    .cloned()
                    .collect::<Vec<_>>()
                    .encode()
            }
            "CeruleaPlatformApi_record" => {
                let (deployment, collection, key): (DeploymentId, Vec<u8>, Vec<u8>) =
                    args(name, params)?;
                self.records
                    .iter()
                    .find(|held| {
                        Self::held(&deployment) && held.collection == collection && held.key == key
                    })
                    .cloned()
                    .encode()
            }
            "CeruleaPlatformApi_records" => {
                let (deployment, collection, prefix, limit): (DeploymentId, Vec<u8>, Vec<u8>, u32) =
                    args(name, params)?;
                self.records
                    .iter()
                    .filter(|held| {
                        Self::held(&deployment)
                            && held.collection == collection
                            && held.key.starts_with(&prefix)
                    })
                    .take(limit as usize)
                    .cloned()
                    .collect::<Vec<_>>()
                    .encode()
            }
            "CeruleaPlatformApi_collection_root" => {
                let (deployment, collection): (DeploymentId, Vec<u8>) = args(name, params)?;
                self.collections
                    .iter()
                    .find(|held| Self::held(&deployment) && held.name == collection)
                    .map(|held| held.root)
                    .encode()
            }
            "CeruleaPlatformApi_verify_record_proof" => {
                let (proof, root): (Vec<u8>, [u8; 32]) = args(name, params)?;
                let folded: [u8; 32] = BlakeTwo256::hash(&proof).into();
                (folded == root).encode()
            }
            "CeruleaPlatformApi_resolve_permission" => {
                let (deployment, actor, collection, action): (DeploymentId, u64, Vec<u8>, Vec<u8>) =
                    args(name, params)?;
                let grant = self.grants.iter().find(|held| {
                    Self::held(&deployment)
                        && held.0 == actor
                        && held.1 == collection
                        && held.2 == action
                });
                match grant {
                    Some(held) => PermissionReply {
                        admitted: true,
                        role: Some(held.3.clone()),
                        fields: vec![],
                        every_field: true,
                        problem: vec![],
                        fix: vec![],
                    },
                    None => PermissionReply {
                        admitted: false,
                        role: None,
                        fields: vec![],
                        every_field: false,
                        problem: text("The account holds no role reaching this action."),
                        fix: text("Assign a role that runs the action."),
                    },
                }
                .encode()
            }
            "CeruleaPlatformApi_simulate_as_role" => {
                let (deployment, role, collection): (DeploymentId, Vec<u8>, Vec<u8>) =
                    args(name, params)?;
                self.roles
                    .iter()
                    .find(|held| {
                        Self::held(&deployment)
                            && held.role == role
                            && held.collection == collection
                    })
                    .cloned()
                    .unwrap_or(RoleReply {
                        role,
                        collection,
                        read: false,
                        insert: false,
                        update: false,
                        delete: false,
                        readable_fields: vec![],
                        every_field: false,
                        actions: vec![],
                        statement: text("The role reaches nothing in this collection."),
                    })
                    .encode()
            }
            "CeruleaPlatformApi_evaluate_view" => {
                let (deployment, _actor, view): (DeploymentId, u64, Vec<u8>) = args(name, params)?;
                match self
                    .views
                    .iter()
                    .find(|held| Self::held(&deployment) && held.0 == view)
                {
                    Some(held) => ViewReply {
                        view,
                        evaluated: true,
                        rows: held.1.len() as u32,
                        encoded_rows: held.1.clone(),
                        digest: BlakeTwo256::hash(&held.1.encode()).into(),
                        problem: vec![],
                    },
                    None => ViewReply {
                        view,
                        evaluated: false,
                        rows: 0,
                        encoded_rows: vec![],
                        digest: [0u8; 32],
                        problem: text("The Blueprint declares no view by that name."),
                    },
                }
                .encode()
            }
            "CeruleaPlatformApi_workflow_names" => {
                let (deployment,): (DeploymentId,) = args(name, params)?;
                let mut names: Vec<Vec<u8>> = self
                    .workflows
                    .iter()
                    .filter(|_| Self::held(&deployment))
                    .map(|held| held.workflow.clone())
                    .collect();
                names.dedup();
                names.encode()
            }
            "CeruleaPlatformApi_workflow_instance" => {
                let (deployment, workflow, instance): (DeploymentId, Vec<u8>, u64) =
                    args(name, params)?;
                self.workflows
                    .iter()
                    .find(|held| {
                        Self::held(&deployment)
                            && held.workflow == workflow
                            && held.instance == instance
                    })
                    .cloned()
                    .encode()
            }
            "CeruleaPlatformApi_workflow_instances" => {
                let (deployment, workflow, limit): (DeploymentId, Vec<u8>, u32) =
                    args(name, params)?;
                self.workflows
                    .iter()
                    .filter(|held| Self::held(&deployment) && held.workflow == workflow)
                    .take(limit as usize)
                    .cloned()
                    .collect::<Vec<_>>()
                    .encode()
            }
            "CeruleaPlatformApi_proposals" => {
                let (deployment, limit): (DeploymentId, u32) = args(name, params)?;
                let mut held: Vec<ProposalReply> = self
                    .proposals
                    .iter()
                    .filter(|_| Self::held(&deployment))
                    .cloned()
                    .collect();
                held.sort_by(|left, right| right.id.cmp(&left.id));
                held.truncate(limit as usize);
                held.encode()
            }
            "CeruleaPlatformApi_proposal" => {
                let (deployment, id): (DeploymentId, u64) = args(name, params)?;
                self.proposals
                    .iter()
                    .find(|held| Self::held(&deployment) && held.id == id)
                    .cloned()
                    .encode()
            }
            "CeruleaPlatformApi_proposal_votes" => {
                let (deployment, id): (DeploymentId, u64) = args(name, params)?;
                self.votes
                    .iter()
                    .filter(|held| Self::held(&deployment) && held.0 == id)
                    .map(|held| held.1.clone())
                    .collect::<Vec<_>>()
                    .encode()
            }
            "CeruleaPlatformApi_executed_changes" => {
                let (deployment, limit): (DeploymentId, u32) = args(name, params)?;
                self.proposals
                    .iter()
                    .filter(|held| Self::held(&deployment) && held.executed_block.is_some())
                    .take(limit as usize)
                    .cloned()
                    .collect::<Vec<_>>()
                    .encode()
            }
            "CeruleaPlatformApi_audit_length" => {
                let (deployment,): (DeploymentId,) = args(name, params)?;
                (if Self::held(&deployment) {
                    self.audit.len() as u64
                } else {
                    0
                })
                .encode()
            }
            "CeruleaPlatformApi_audit_entries" => {
                let (deployment, from, limit): (DeploymentId, u64, u32) = args(name, params)?;
                self.audit
                    .iter()
                    .filter(|held| Self::held(&deployment) && held.sequence >= from)
                    .take(limit as usize)
                    .cloned()
                    .collect::<Vec<_>>()
                    .encode()
            }
            "CeruleaPlatformApi_audit_by_actor" => {
                let (deployment, actor, limit): (DeploymentId, u64, u32) = args(name, params)?;
                self.audit
                    .iter()
                    .filter(|held| Self::held(&deployment) && held.actor == actor)
                    .take(limit as usize)
                    .cloned()
                    .collect::<Vec<_>>()
                    .encode()
            }
            "CeruleaPlatformApi_licence_state" => {
                let (deployment,): (DeploymentId,) = args(name, params)?;
                Some(self.licence.clone())
                    .filter(|_| Self::held(&deployment))
                    .encode()
            }
            "CeruleaPlatformApi_production_permitted" => {
                args::<()>(name, params)?;
                self.licence.production_permitted.encode()
            }
            "CeruleaPlatformApi_oracle_feeds" => {
                let (deployment,): (DeploymentId,) = args(name, params)?;
                self.feeds
                    .iter()
                    .filter(|_| Self::held(&deployment))
                    .map(|held| held.feed.clone())
                    .collect::<Vec<_>>()
                    .encode()
            }
            "CeruleaPlatformApi_oracle_value" => {
                let (deployment, feed): (DeploymentId, Vec<u8>) = args(name, params)?;
                self.feeds
                    .iter()
                    .find(|held| Self::held(&deployment) && held.feed == feed)
                    .cloned()
                    .encode()
            }
            "CeruleaPlatformApi_versions" => {
                let (deployment,): (DeploymentId,) = args(name, params)?;
                self.versions
                    .iter()
                    .filter(|_| Self::held(&deployment))
                    .cloned()
                    .collect::<Vec<_>>()
                    .encode()
            }
            "CeruleaPlatformApi_rollback_windows" => {
                let (deployment,): (DeploymentId,) = args(name, params)?;
                self.windows
                    .iter()
                    .filter(|_| Self::held(&deployment))
                    .cloned()
                    .collect::<Vec<_>>()
                    .encode()
            }
            "CeruleaPlatformApi_operator_accounts" => {
                let (deployment,): (DeploymentId,) = args(name, params)?;
                self.operators
                    .iter()
                    .filter(|_| Self::held(&deployment))
                    .copied()
                    .collect::<Vec<_>>()
                    .encode()
            }
            "CeruleaPlatformApi_consensus_profile" => {
                args::<()>(name, params)?;
                ConsensusProfileReply {
                    routine_percent: 51,
                    financial_percent: 67,
                    structural_percent: 80,
                    constitutional_percent: 90,
                    committee_size: 5,
                    block_time_millis: 6_000,
                    epoch_length: 600,
                }
                .encode()
            }
            "CeruleaPlatformApi_custody_history" => {
                let (validator, limit): (u64, u32) = args(name, params)?;
                self.custody
                    .iter()
                    .filter(|held| held.validator == validator)
                    .take(limit as usize)
                    .cloned()
                    .collect::<Vec<_>>()
                    .encode()
            }
            "CeruleaPlatformApi_custody_counts" => {
                let (validator,): (u64,) = args(name, params)?;
                let mut counts = CustodyCountsReply::default();
                for held in self
                    .custody
                    .iter()
                    .filter(|held| held.validator == validator)
                {
                    if held.verified {
                        counts.answered += 1;
                    } else if held.answered {
                        counts.faults += 1;
                    } else if held.settled {
                        counts.missed += 1;
                    }
                }
                counts.share = 1_000_000 - 100_000 * counts.missed;
                counts.encode()
            }
            "CeruleaPlatformApi_action_manifest" => {
                let (deployment, action): (DeploymentId, Vec<u8>) = args(name, params)?;
                self.manifests
                    .iter()
                    .find(|held| Self::held(&deployment) && held.action == action)
                    .cloned()
                    .encode()
            }
            "CeruleaPlatformApi_action_names" => {
                let (deployment,): (DeploymentId,) = args(name, params)?;
                self.manifests
                    .iter()
                    .filter(|_| Self::held(&deployment))
                    .map(|held| held.action.clone())
                    .collect::<Vec<_>>()
                    .encode()
            }
            _ => {
                return Err(ApiError::Application(
                    String::from("The local runtime holds no entry point by that name.").into(),
                ))
            }
        };
        Ok(reply)
    }

    fn route(
        &self,
        params: Vec<u8>,
        fn_name: &dyn Fn(RuntimeVersion) -> &'static str,
    ) -> Result<Vec<u8>, ApiError> {
        let name = fn_name(RuntimeVersion::default());
        self.calls.lock().expect("the call log opens").push(name);
        match self.mode {
            Mode::Model => self.answer(name, &params),
            Mode::Damaged => Ok(vec![0xff; 3]),
            Mode::Refusing => Err(ApiError::Application(
                String::from("The runtime refused the read. Retry against a synced block.").into(),
            )),
        }
    }
}

impl sp_api::Core<ReaderBlock> for LocalRuntime {
    fn __runtime_api_internal_call_api_at(
        &self,
        _at: At,
        params: Vec<u8>,
        fn_name: &dyn Fn(RuntimeVersion) -> &'static str,
    ) -> Result<Vec<u8>, ApiError> {
        self.route(params, fn_name)
    }
}

impl CeruleaPlatformApi<ReaderBlock, u64> for LocalRuntime {
    fn __runtime_api_internal_call_api_at(
        &self,
        _at: At,
        params: Vec<u8>,
        fn_name: &dyn Fn(RuntimeVersion) -> &'static str,
    ) -> Result<Vec<u8>, ApiError> {
        self.route(params, fn_name)
    }
}

type Api = dyn CeruleaPlatformApi<ReaderBlock, u64>;

#[test]
fn the_deployment_reads_reach_their_entry_points() {
    let runtime = LocalRuntime::new(Mode::Model);
    let api: &Api = &runtime;
    assert_eq!(api.deployments(at()).expect("reads"), vec![DEPLOYMENT]);
    assert_eq!(
        api.blueprint_address(at(), DEPLOYMENT).expect("reads"),
        Some(DEPLOYMENT)
    );
    assert_eq!(api.blueprint_address(at(), OTHER).expect("reads"), None);
    assert_eq!(
        api.blueprint(at(), DEPLOYMENT).expect("reads"),
        Some(text("blueprint canonical bytes"))
    );
    assert_eq!(api.blueprint(at(), OTHER).expect("reads"), None);
    assert_eq!(
        api.type_names(at(), DEPLOYMENT).expect("reads"),
        vec![text("parcel_record")]
    );
    assert_eq!(
        api.type_definition(at(), DEPLOYMENT, text("parcel_record"))
            .expect("reads"),
        Some(vec![0x01, 0x02])
    );
    assert_eq!(
        api.type_definition(at(), DEPLOYMENT, text("absent"))
            .expect("reads"),
        None
    );
    assert_eq!(
        runtime.calls(),
        vec![
            "CeruleaPlatformApi_deployments",
            "CeruleaPlatformApi_blueprint_address",
            "CeruleaPlatformApi_blueprint_address",
            "CeruleaPlatformApi_blueprint",
            "CeruleaPlatformApi_blueprint",
            "CeruleaPlatformApi_type_names",
            "CeruleaPlatformApi_type_definition",
            "CeruleaPlatformApi_type_definition",
        ]
    );
}

#[test]
fn the_record_reads_carry_the_prefix_the_limit_and_the_proof() {
    let runtime = LocalRuntime::new(Mode::Model);
    let api: &Api = &runtime;

    let collections = api.collections(at(), DEPLOYMENT).expect("reads");
    assert_eq!(collections.len(), 1);
    assert_eq!(collections[0].records, 3);

    let held = api
        .record(at(), DEPLOYMENT, text("parcel"), text("12/2"))
        .expect("reads")
        .expect("the record exists");
    assert_eq!(held.value, Some(vec![2, 2]));
    assert_eq!(
        api.record(at(), DEPLOYMENT, text("parcel"), text("99/9"))
            .expect("reads"),
        None
    );

    let prefixed = api
        .records(at(), DEPLOYMENT, text("parcel"), text("12/"), 10)
        .expect("reads");
    assert_eq!(
        prefixed
            .iter()
            .map(|held| held.key.clone())
            .collect::<Vec<_>>(),
        vec![text("12/1"), text("12/2")]
    );
    let limited = api
        .records(at(), DEPLOYMENT, text("parcel"), vec![], 1)
        .expect("reads");
    assert_eq!(limited.len(), 1);

    let root = api
        .collection_root(at(), DEPLOYMENT, text("parcel"))
        .expect("reads");
    assert_eq!(root, Some(collections[0].root));
    assert_eq!(
        api.collection_root(at(), DEPLOYMENT, text("absent"))
            .expect("reads"),
        None
    );

    let proof = held.proof.clone().expect("an open record carries a proof");
    assert!(api
        .verify_record_proof(at(), proof.clone(), held.root)
        .expect("reads"));
    assert!(!api
        .verify_record_proof(at(), proof, [0u8; 32])
        .expect("reads"));
}

#[test]
fn the_capability_reads_answer_admission_and_refusal() {
    let runtime = LocalRuntime::new(Mode::Model);
    let api: &Api = &runtime;

    let admitted = api
        .resolve_permission(
            at(),
            DEPLOYMENT,
            REGISTRAR,
            text("parcel"),
            text("record_transfer"),
        )
        .expect("reads");
    assert!(admitted.admitted);
    assert_eq!(admitted.role, Some(text("registrar")));

    let refused = api
        .resolve_permission(
            at(),
            DEPLOYMENT,
            CLERK,
            text("parcel"),
            text("record_transfer"),
        )
        .expect("reads");
    assert!(!refused.admitted);
    assert_eq!(refused.fix, text("Assign a role that runs the action."));

    let reach = api
        .simulate_as_role(at(), DEPLOYMENT, text("registrar"), text("parcel"))
        .expect("reads");
    assert!(reach.read && reach.insert && !reach.delete);
    let none = api
        .simulate_as_role(at(), DEPLOYMENT, text("visitor"), text("parcel"))
        .expect("reads");
    assert!(!none.read);
    assert_eq!(
        none.statement,
        text("The role reaches nothing in this collection.")
    );

    let rows = api
        .evaluate_view(at(), DEPLOYMENT, REGISTRAR, text("open_parcels"))
        .expect("reads");
    assert!(rows.evaluated);
    assert_eq!(rows.rows, 2);
    let missing = api
        .evaluate_view(at(), DEPLOYMENT, REGISTRAR, text("absent"))
        .expect("reads");
    assert!(!missing.evaluated);
    assert_eq!(
        missing.problem,
        text("The Blueprint declares no view by that name.")
    );
}

#[test]
fn the_process_reads_carry_the_instance_and_the_limit() {
    let runtime = LocalRuntime::new(Mode::Model);
    let api: &Api = &runtime;
    assert_eq!(
        api.workflow_names(at(), DEPLOYMENT).expect("reads"),
        vec![text("transfer")]
    );
    let second = api
        .workflow_instance(at(), DEPLOYMENT, text("transfer"), 2)
        .expect("reads")
        .expect("the process exists");
    assert_eq!(second.started_block, 2);
    assert_eq!(
        api.workflow_instance(at(), DEPLOYMENT, text("transfer"), 9)
            .expect("reads"),
        None
    );
    assert_eq!(
        api.workflow_instances(at(), DEPLOYMENT, text("transfer"), 2)
            .expect("reads")
            .len(),
        2
    );
}

#[test]
fn the_change_process_reads_answer_newest_first() {
    let runtime = LocalRuntime::new(Mode::Model);
    let api: &Api = &runtime;
    let newest = api.proposals(at(), DEPLOYMENT, 2).expect("reads");
    assert_eq!(
        newest.iter().map(|held| held.id).collect::<Vec<_>>(),
        vec![3, 2]
    );
    assert_eq!(
        api.proposal(at(), DEPLOYMENT, 1)
            .expect("reads")
            .map(|held| held.status),
        Some(text("executed"))
    );
    assert_eq!(api.proposal(at(), DEPLOYMENT, 8).expect("reads"), None);
    let votes = api.proposal_votes(at(), DEPLOYMENT, 1).expect("reads");
    assert_eq!(votes.len(), 1);
    assert_eq!(votes[0].voter, REGISTRAR);
    assert!(api
        .proposal_votes(at(), DEPLOYMENT, 3)
        .expect("reads")
        .is_empty());
    let executed = api.executed_changes(at(), DEPLOYMENT, 5).expect("reads");
    assert_eq!(executed.len(), 1);
    assert_eq!(executed[0].executed_block, Some(4));
}

#[test]
fn the_audit_reads_carry_the_position_and_the_actor() {
    let runtime = LocalRuntime::new(Mode::Model);
    let api: &Api = &runtime;
    assert_eq!(api.audit_length(at(), DEPLOYMENT).expect("reads"), 3);
    assert_eq!(api.audit_length(at(), OTHER).expect("reads"), 0);
    let tail = api.audit_entries(at(), DEPLOYMENT, 1, 10).expect("reads");
    assert_eq!(
        tail.iter().map(|held| held.sequence).collect::<Vec<_>>(),
        vec![1, 2]
    );
    let by_registrar = api
        .audit_by_actor(at(), DEPLOYMENT, REGISTRAR, 10)
        .expect("reads");
    assert_eq!(
        by_registrar
            .iter()
            .map(|held| held.event.clone())
            .collect::<Vec<_>>(),
        vec![text("record.inserted"), text("record.updated")]
    );
    assert_eq!(
        api.audit_by_actor(at(), DEPLOYMENT, CLERK, 0)
            .expect("reads")
            .len(),
        0
    );
}

#[test]
fn the_licence_and_feed_reads_answer_for_the_deployment() {
    let runtime = LocalRuntime::new(Mode::Model);
    let api: &Api = &runtime;
    let licence = api
        .licence_state(at(), DEPLOYMENT)
        .expect("reads")
        .expect("the deployment holds a licence");
    assert_eq!(licence.application, text("government"));
    assert_eq!(api.licence_state(at(), OTHER).expect("reads"), None);
    assert!(api.production_permitted(at()).expect("reads"));
    assert_eq!(
        api.oracle_feeds(at(), DEPLOYMENT).expect("reads"),
        vec![text("rainfall_mm")]
    );
    let value = api
        .oracle_value(at(), DEPLOYMENT, text("rainfall_mm"))
        .expect("reads")
        .expect("the feed holds a value");
    assert_eq!(
        u32::decode(&mut &value.value[..]).expect("a number decodes"),
        31
    );
    assert_eq!(
        api.oracle_value(at(), DEPLOYMENT, text("absent"))
            .expect("reads"),
        None
    );
}

#[test]
fn the_version_and_operator_reads_answer_for_the_deployment() {
    let runtime = LocalRuntime::new(Mode::Model);
    let api: &Api = &runtime;
    let versions = api.versions(at(), DEPLOYMENT).expect("reads");
    assert_eq!(versions.iter().filter(|held| held.active).count(), 1);
    assert_eq!(versions.last().map(|held| held.blueprint), Some(DEPLOYMENT));
    let windows = api.rollback_windows(at(), DEPLOYMENT).expect("reads");
    assert!(windows[0].open);
    assert_eq!(
        api.operator_accounts(at(), DEPLOYMENT).expect("reads"),
        vec![REGISTRAR]
    );
    assert!(api
        .operator_accounts(at(), OTHER)
        .expect("reads")
        .is_empty());
}

#[test]
fn the_consensus_custody_and_manifest_reads_answer() {
    let runtime = LocalRuntime::new(Mode::Model);
    let api: &Api = &runtime;
    let profile = api.consensus_profile(at()).expect("reads");
    assert_eq!(
        (
            profile.routine_percent,
            profile.financial_percent,
            profile.structural_percent,
            profile.constitutional_percent
        ),
        (51, 67, 80, 90)
    );
    let history = api.custody_history(at(), REGISTRAR, 5).expect("reads");
    assert_eq!(history.len(), 2);
    assert!(history.iter().all(|held| held.validator == REGISTRAR));
    assert!(history.iter().any(|held| !held.answered));
    assert_eq!(api.custody_history(at(), CLERK, 1).expect("reads").len(), 1);
    // Rounds 0 and 2 reach the registrar: one answered, one missed.
    assert_eq!(
        api.custody_counts(at(), REGISTRAR).expect("reads"),
        CustodyCountsReply {
            answered: 1,
            missed: 1,
            faults: 0,
            share: 900_000,
        }
    );
    // Rounds 1 and 3 reach the clerk, both answered.
    assert_eq!(
        api.custody_counts(at(), CLERK).expect("reads"),
        CustodyCountsReply {
            answered: 2,
            missed: 0,
            faults: 0,
            share: 1_000_000,
        }
    );
    let manifest = api
        .action_manifest(at(), DEPLOYMENT, text("record_transfer"))
        .expect("reads")
        .expect("the action exists");
    assert_eq!(manifest.writes, vec![text("parcel")]);
    assert_eq!(
        api.action_manifest(at(), DEPLOYMENT, text("absent"))
            .expect("reads"),
        None
    );
    assert_eq!(
        api.action_names(at(), DEPLOYMENT).expect("reads"),
        vec![text("record_transfer")]
    );
}

#[test]
fn a_damaged_reply_names_the_entry_point_it_came_from() {
    let runtime = LocalRuntime::new(Mode::Damaged);
    let api: &Api = &runtime;
    match api.production_permitted(at()) {
        Err(ApiError::FailedToDecodeReturnValue { function, raw, .. }) => {
            assert_eq!(function, "CeruleaPlatformApi_production_permitted");
            assert_eq!(raw, vec![0xff; 3]);
        }
        other => panic!("a damaged reply fails to decode, found {:?}", other.is_ok()),
    }
    match api.consensus_profile(at()) {
        Err(ApiError::FailedToDecodeReturnValue { function, .. }) => {
            assert_eq!(function, "CeruleaPlatformApi_consensus_profile");
        }
        other => panic!("a damaged reply fails to decode, found {:?}", other.is_ok()),
    }
}

#[test]
fn a_refused_read_carries_the_runtime_sentence() {
    let runtime = LocalRuntime::new(Mode::Refusing);
    let api: &Api = &runtime;
    match api.audit_length(at(), DEPLOYMENT) {
        Err(ApiError::Application(error)) => {
            assert_eq!(
                error.to_string(),
                "The runtime refused the read. Retry against a synced block."
            );
        }
        other => panic!(
            "a refused read surfaces the refusal, found {:?}",
            other.is_ok()
        ),
    }
    assert_eq!(runtime.calls(), vec!["CeruleaPlatformApi_audit_length"]);
}

#[test]
fn an_unknown_entry_point_is_refused_by_name() {
    let runtime = LocalRuntime::new(Mode::Model);
    match runtime.answer("CeruleaPlatformApi_absent", &[]) {
        Err(ApiError::Application(error)) => {
            assert_eq!(
                error.to_string(),
                "The local runtime holds no entry point by that name."
            );
        }
        other => panic!(
            "an unknown entry point is refused, found {:?}",
            other.is_ok()
        ),
    }
    match runtime.answer("CeruleaPlatformApi_blueprint", &[1, 2]) {
        Err(ApiError::FailedToConvertParameter { function, .. }) => {
            assert_eq!(function, "CeruleaPlatformApi_blueprint");
        }
        other => panic!("short arguments are refused, found {:?}", other.is_ok()),
    }
}

#[test]
fn the_core_version_read_travels_through_the_same_route() {
    let runtime = LocalRuntime::new(Mode::Damaged);
    let core: &dyn sp_api::Core<ReaderBlock> = &runtime;
    assert!(core.version(at()).is_err());
    assert_eq!(runtime.calls(), vec!["Core_version"]);
}
