//! The runtime side of the platform interfaces.
//!
//! The calls declared in `cerulea-platform-api` land here. Each one reads
//! one platform module and turns the stored shape into the plain shape a
//! reader decodes.
//!
//! Every call changes nothing. A reader therefore runs any call against
//! any block the node still holds.

extern crate alloc;

use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use cerulea_blueprint::types::{CollectionId, Value};
use cerulea_platform_api::{
    AuditReply, CollectionSummary, ConsensusProfileReply, CustodyCountsReply, CustodyReply,
    DeploymentId, LicenceReply, OracleReply, PermissionReply, PrivacyLabel, ProposalReply,
    RecordReply, RecordVersionEntry, RoleReply, RollbackWindowReply, VersionReply, ViewReply,
    VoteReply, WorkflowInstanceReply,
};
use codec::{Decode, Encode};
use frame_support::traits::Get;
use pallet_cerulea_audit::WorkClass;
use pallet_cerulea_collections::PrivacyLevel;
use pallet_cerulea_schema::{
    access::{AccessAction, AccessRequest},
    primitives::{name_of, BoundedName, RecordKey},
};
use sp_runtime::traits::SaturatedConversion;

use crate::{AccountId, Runtime};

/// The largest number of rows, entries or records one call answers with.
///
/// A caller naming a larger limit takes this ceiling instead, so one call
/// never walks the whole store.
pub const READ_CEILING: u32 = 512;

/// Hold one limit inside the ceiling.
fn bounded_limit(limit: u32) -> u32 {
    limit.clamp(1, READ_CEILING)
}

/// Read a bounded name as plain bytes.
fn name_bytes(name: &BoundedName) -> Vec<u8> {
    name.to_vec()
}

/// Turn plain bytes into a collection identifier.
fn collection_of(bytes: &[u8]) -> Option<CollectionId> {
    let text = String::from_utf8(bytes.to_vec()).ok()?;
    Some(CollectionId::new(&text))
}

/// Turn plain bytes into a bounded name.
fn bounded_of(bytes: &[u8]) -> Option<BoundedName> {
    let text = String::from_utf8(bytes.to_vec()).ok()?;
    name_of(&text).ok()
}

/// The label of one privacy class.
fn privacy_label(level: PrivacyLevel) -> PrivacyLabel {
    match level {
        PrivacyLevel::Open => PrivacyLabel::Open,
        PrivacyLevel::HashOnly => PrivacyLabel::HashOnly,
        PrivacyLevel::Encrypted => PrivacyLabel::Encrypted,
        PrivacyLevel::External => PrivacyLabel::External,
    }
}

/// The label of one class of work.
fn class_label(class: WorkClass) -> Vec<u8> {
    match class {
        WorkClass::Routine => b"routine".to_vec(),
        WorkClass::Financial => b"financial".to_vec(),
        WorkClass::Structural => b"structural".to_vec(),
        WorkClass::Constitutional => b"constitutional".to_vec(),
    }
}

/// The field list of one qualification, with the flag naming the whole
/// record.
fn field_list(fields: &cerulea_blueprint::permissions::FieldQualification) -> (Vec<Vec<u8>>, bool) {
    use cerulea_blueprint::permissions::FieldQualification;
    match fields {
        FieldQualification::EveryField => (Vec::new(), true),
        FieldQualification::Only(listed) | FieldQualification::Apart(listed) => (
            listed
                .iter()
                .map(|path| path.to_string().into_bytes())
                .collect(),
            false,
        ),
    }
}

/// Every deployment the chain holds.
pub fn deployments() -> Vec<DeploymentId> {
    pallet_cerulea_logic::Installed::<Runtime>::iter_keys().collect()
}

/// The content address of the Blueprint one deployment runs.
pub fn blueprint_address(deployment: DeploymentId) -> Option<[u8; 32]> {
    pallet_cerulea_logic::Installed::<Runtime>::get(deployment).map(|held| held.id().0)
}

/// The canonical encoding of the Blueprint one deployment runs.
pub fn blueprint(deployment: DeploymentId) -> Option<Vec<u8>> {
    pallet_cerulea_logic::Installed::<Runtime>::get(deployment).map(|held| held.encode())
}

/// The registered type names of one deployment.
pub fn type_names(deployment: DeploymentId) -> Vec<Vec<u8>> {
    pallet_cerulea_schema::Pallet::<Runtime>::type_names(&deployment)
        .iter()
        .map(name_bytes)
        .collect()
}

/// The canonical encoding of one registered type definition.
pub fn type_definition(deployment: DeploymentId, name: Vec<u8>) -> Option<Vec<u8>> {
    let text = String::from_utf8(name).ok()?;
    let wanted = cerulea_blueprint::types::TypeId::new(&text);
    pallet_cerulea_schema::Pallet::<Runtime>::type_defs(&deployment)
        .into_iter()
        .find(|held| *held.id() == wanted)
        .map(|held| held.encode())
}

/// Every collection of one deployment with its class and its count.
pub fn collections(deployment: DeploymentId) -> Vec<CollectionSummary> {
    pallet_cerulea_schema::Pallet::<Runtime>::collection_names(&deployment)
        .iter()
        .filter_map(|name| {
            let text = String::from_utf8(name.to_vec()).ok()?;
            let id = CollectionId::new(&text);
            let entry =
                pallet_cerulea_schema::Pallet::<Runtime>::collection_entry(&deployment, &id)?;
            let level = pallet_cerulea_collections::Pallet::<Runtime>::level(&deployment, &id);
            Some(CollectionSummary {
                name: name_bytes(name),
                record_type: entry.definition.record_type.as_str().as_bytes().to_vec(),
                privacy: privacy_label(level),
                records: pallet_cerulea_state::Pallet::<Runtime>::record_count(&deployment, &id),
                root: pallet_cerulea_state::Pallet::<Runtime>::collection_root(&deployment, &id)
                    .unwrap_or_else(pallet_cerulea_state::proof::empty_root),
                version: entry.version,
                retired: entry.retired,
                summary: entry.definition.summary.clone().into_bytes(),
            })
        })
        .collect()
}

/// One record with its commitment, its history and its proof.
pub fn record(deployment: DeploymentId, collection: Vec<u8>, key: Vec<u8>) -> Option<RecordReply> {
    let id = collection_of(&collection)?;
    let record_key: RecordKey = RecordKey::try_from(key).ok()?;
    record_at(deployment, &id, &collection, &record_key)
}

/// The records of one collection whose key starts with the prefix.
pub fn records(
    deployment: DeploymentId,
    collection: Vec<u8>,
    prefix: Vec<u8>,
    limit: u32,
) -> Vec<RecordReply> {
    let id = match collection_of(&collection) {
        Some(held) => held,
        None => return Vec::new(),
    };
    let ceiling = bounded_limit(limit);
    pallet_cerulea_state::Pallet::<Runtime>::record_keys(&deployment, &id, READ_CEILING)
        .into_iter()
        .filter(|held| held.as_slice().starts_with(prefix.as_slice()))
        .take(ceiling as usize)
        .filter_map(|held| record_at(deployment, &id, &collection, &held))
        .collect()
}

/// Build one record reply.
fn record_at(
    deployment: DeploymentId,
    id: &CollectionId,
    collection: &[u8],
    key: &RecordKey,
) -> Option<RecordReply> {
    let meta = pallet_cerulea_state::Pallet::<Runtime>::record_meta(&deployment, id, key)?;
    let level = pallet_cerulea_collections::Pallet::<Runtime>::level(&deployment, id);
    let bytes = pallet_cerulea_state::Pallet::<Runtime>::record_bytes(&deployment, id, key);
    // A sealed record shows its commitment and never its content.
    let value = match level {
        PrivacyLevel::Open => bytes,
        _ => None,
    };
    let proof = pallet_cerulea_state::Pallet::<Runtime>::inclusion_proof(&deployment, id, key)
        .map(|held| held.encode());
    let history = pallet_cerulea_state::Pallet::<Runtime>::version_history(&deployment, id, key)
        .into_iter()
        .map(|held| RecordVersionEntry {
            version: held.version,
            commitment: held.commitment,
            block: held.block.saturated_into::<u32>(),
            at_millis: held.at_millis,
        })
        .collect();
    Some(RecordReply {
        collection: collection.to_vec(),
        key: key.to_vec(),
        value,
        commitment: meta.value_hash,
        version: meta.version,
        created_block: meta.created_at.saturated_into::<u32>(),
        changed_block: meta.changed_at.saturated_into::<u32>(),
        privacy: privacy_label(level),
        history,
        proof,
        root: pallet_cerulea_state::Pallet::<Runtime>::collection_root(&deployment, id)
            .unwrap_or_else(pallet_cerulea_state::proof::empty_root),
    })
}

/// The inclusion tree root of one collection.
pub fn collection_root(deployment: DeploymentId, collection: Vec<u8>) -> Option<[u8; 32]> {
    let id = collection_of(&collection)?;
    // The store answers a root for a collection the schema registered and
    // nothing for a name the Blueprint never declared.
    <pallet_cerulea_state::Pallet<Runtime> as pallet_cerulea_state::RecordStore>::root(
        &deployment,
        &id,
    )
}

/// Fold one inclusion proof and compare the result with a root.
pub fn verify_record_proof(proof: Vec<u8>, root: [u8; 32]) -> bool {
    match pallet_cerulea_state::proof::InclusionProof::decode(&mut proof.as_slice()) {
        Ok(held) => held.verifies(&root),
        Err(_) => false,
    }
}

/// Resolve the capability matrix for one request.
pub fn resolve_permission(
    deployment: DeploymentId,
    actor: AccountId,
    collection: Vec<u8>,
    action: Vec<u8>,
) -> PermissionReply {
    let refusal = |problem: &str, fix: &str| PermissionReply {
        admitted: false,
        role: None,
        fields: Vec::new(),
        every_field: false,
        problem: problem.as_bytes().to_vec(),
        fix: fix.as_bytes().to_vec(),
    };
    let id = match collection_of(&collection) {
        Some(held) => held,
        None => {
            return refusal(
                "The request names a collection whose name runs past the limit.",
                "Shorten the collection name in the Blueprint.",
            )
        }
    };
    let action_text = match String::from_utf8(action) {
        Ok(held) => held,
        Err(_) => {
            return refusal(
                "The request names an action the Blueprint never declared.",
                "Name an action the Blueprint declares.",
            )
        }
    };
    let wanted = match action_text.as_str() {
        "read" => AccessAction::Read,
        "insert" => AccessAction::Insert,
        "update" => AccessAction::Update,
        "delete" => AccessAction::Delete,
        other => AccessAction::Invoke(cerulea_blueprint::types::ActionId::new(other)),
    };
    let now = cerulea_blueprint::types::Timestamp::from_millis(pallet_cerulea_state::Pallet::<
        Runtime,
    >::now_millis());
    let request = AccessRequest::new(deployment, &actor, id, wanted, now);
    match pallet_cerulea_permission::Pallet::<Runtime>::resolve_request(&request) {
        Ok(grant) => {
            let (fields, every_field) = field_list(&grant.fields);
            PermissionReply {
                admitted: true,
                role: grant
                    .role
                    .as_ref()
                    .map(|held| held.as_str().as_bytes().to_vec()),
                fields,
                every_field,
                problem: Vec::new(),
                fix: Vec::new(),
            }
        }
        Err(denial) => PermissionReply {
            admitted: false,
            role: None,
            fields: Vec::new(),
            every_field: false,
            problem: denial.problem.clone().into_bytes(),
            fix: denial.fix.clone().into_bytes(),
        },
    }
}

/// What one role reaches in one collection.
pub fn simulate_as_role(deployment: DeploymentId, role: Vec<u8>, collection: Vec<u8>) -> RoleReply {
    let empty = |role: Vec<u8>, collection: Vec<u8>| RoleReply {
        role: role.clone(),
        collection: collection.clone(),
        read: false,
        insert: false,
        update: false,
        delete: false,
        readable_fields: Vec::new(),
        every_field: false,
        actions: Vec::new(),
        statement: b"The role reaches nothing in that collection.".to_vec(),
    };
    let role_name = match bounded_of(&role) {
        Some(held) => held,
        None => return empty(role, collection),
    };
    let collection_name = match bounded_of(&collection) {
        Some(held) => held,
        None => return empty(role, collection),
    };
    let held = pallet_cerulea_permission::Pallet::<Runtime>::role_simulation(
        &deployment,
        &role_name,
        &collection_name,
    );
    let statement = held.statement().into_bytes();
    let (readable_fields, every_field) = field_list(&held.readable_fields);
    RoleReply {
        role,
        collection,
        read: held.read,
        insert: held.insert,
        update: held.update,
        delete: held.delete,
        readable_fields,
        every_field,
        actions: held.actions.iter().map(name_bytes).collect(),
        statement,
    }
}

/// Evaluate one view as one caller.
pub fn evaluate_view(deployment: DeploymentId, actor: AccountId, view: Vec<u8>) -> ViewReply {
    let name = match String::from_utf8(view.clone()) {
        Ok(held) => held,
        Err(_) => {
            return ViewReply {
                view,
                evaluated: false,
                rows: 0,
                encoded_rows: Vec::new(),
                digest: [0u8; 32],
                problem: b"The request names a view the Blueprint never declared. Name a view the Blueprint declares."
                    .to_vec(),
            }
        }
    };
    match pallet_cerulea_logic::Pallet::<Runtime>::run_view(&deployment, &actor, &name) {
        Ok(rows) => {
            let digest = pallet_cerulea_logic::Pallet::<Runtime>::digest_of_rows(&rows);
            let count = u32::try_from(rows.len()).unwrap_or(u32::MAX);
            ViewReply {
                view,
                evaluated: true,
                rows: count,
                encoded_rows: rows.iter().map(|row| row.encode()).collect(),
                digest,
                problem: Vec::new(),
            }
        }
        Err(_) => ViewReply {
            view,
            evaluated: false,
            rows: 0,
            encoded_rows: Vec::new(),
            digest: [0u8; 32],
            problem:
                b"The view failed to evaluate. Check the roles the view names and the collections it reads."
                    .to_vec(),
        },
    }
}

/// The workflows one deployment declares.
pub fn workflow_names(deployment: DeploymentId) -> Vec<Vec<u8>> {
    pallet_cerulea_workflow::MachineNames::<Runtime>::get(deployment)
        .iter()
        .map(name_bytes)
        .collect()
}

/// The label of one process status.
fn instance_status(status: pallet_cerulea_workflow::InstanceStatus) -> Vec<u8> {
    use pallet_cerulea_workflow::InstanceStatus;
    match status {
        InstanceStatus::Running => b"running".to_vec(),
        InstanceStatus::Settled => b"settled".to_vec(),
        InstanceStatus::Compensating => b"compensating".to_vec(),
        InstanceStatus::Compensated => b"compensated".to_vec(),
    }
}

/// Build one process reply.
fn instance_reply(
    workflow: &BoundedName,
    id: u64,
    instance: &pallet_cerulea_workflow::Instance<
        AccountId,
        crate::BlockNumber,
        <Runtime as pallet_cerulea_workflow::Config>::MaxStepsPerInstance,
        <Runtime as pallet_cerulea_workflow::Config>::MaxTimersPerState,
    >,
) -> WorkflowInstanceReply {
    WorkflowInstanceReply {
        workflow: name_bytes(workflow),
        instance: id,
        record: instance.record.to_vec(),
        state: instance.state.to_vec(),
        status: instance_status(instance.status),
        entered_at: instance.entered_at,
        started_at: instance.started_at,
        started_block: instance.started_block.saturated_into::<u32>(),
        history: instance
            .history
            .iter()
            .map(|step| {
                (
                    step.transition.to_vec(),
                    step.from.to_vec(),
                    step.to.to_vec(),
                    step.at_millis,
                )
            })
            .collect(),
        scheduled: instance
            .scheduled
            .iter()
            .map(|(name, due)| (name.to_vec(), *due))
            .collect(),
    }
}

/// Where one process stands.
pub fn workflow_instance(
    deployment: DeploymentId,
    workflow: Vec<u8>,
    instance: u64,
) -> Option<WorkflowInstanceReply> {
    let name = bounded_of(&workflow)?;
    let held = pallet_cerulea_workflow::Pallet::<Runtime>::instance(&deployment, &name, instance)?;
    Some(instance_reply(&name, instance, &held))
}

/// Every process of one workflow.
pub fn workflow_instances(
    deployment: DeploymentId,
    workflow: Vec<u8>,
    limit: u32,
) -> Vec<WorkflowInstanceReply> {
    let name = match bounded_of(&workflow) {
        Some(held) => held,
        None => return Vec::new(),
    };
    let ceiling = bounded_limit(limit) as usize;
    let mut found: Vec<WorkflowInstanceReply> =
        pallet_cerulea_workflow::Instances::<Runtime>::iter_prefix((deployment, name.clone()))
            .take(READ_CEILING as usize)
            .map(|(id, held)| instance_reply(&name, id, &held))
            .collect();
    found.sort_by_key(|held| held.instance);
    found.truncate(ceiling);
    found
}

/// The label of one proposal kind.
fn proposal_kind(kind: pallet_cerulea_governance::ProposalKind) -> Vec<u8> {
    use pallet_cerulea_governance::ProposalKind;
    match kind {
        ProposalKind::SchemaChange => b"schema".to_vec(),
        ProposalKind::PermissionChange => b"permission".to_vec(),
        ProposalKind::PolicyAmendment => b"policy".to_vec(),
        ProposalKind::ValidatorSetChange => b"validator_set".to_vec(),
        ProposalKind::UpgradeApproval => b"upgrade".to_vec(),
    }
}

/// The label of one proposal status.
fn proposal_status(status: pallet_cerulea_governance::ProposalStatus) -> Vec<u8> {
    use pallet_cerulea_governance::ProposalStatus;
    match status {
        ProposalStatus::Open => b"open".to_vec(),
        ProposalStatus::Approved => b"approved".to_vec(),
        ProposalStatus::Rejected => b"rejected".to_vec(),
        ProposalStatus::Executed => b"executed".to_vec(),
        ProposalStatus::Cancelled => b"cancelled".to_vec(),
    }
}

/// Build one proposal reply.
fn proposal_reply(
    id: u64,
    held: &pallet_cerulea_governance::Proposal<
        AccountId,
        crate::BlockNumber,
        <Runtime as pallet_cerulea_governance::Config>::MaxPayloadBytes,
    >,
) -> ProposalReply {
    ProposalReply {
        id,
        kind: proposal_kind(held.kind),
        class: class_label(held.class),
        threshold_percent: held.class.threshold_percent(),
        payload_hash: held.payload_hash,
        status: proposal_status(held.status),
        opened_at: held.opened_at,
        deadline: held.deadline,
        approve_weight: held.approve_weight,
        reject_weight: held.reject_weight,
        voters: held.voters,
        roll_weight: held.roll_weight,
        decided_at: held.decided_at,
        executed_block: held
            .executed_block
            .map(|block| block.saturated_into::<u32>()),
    }
}

/// The proposals of one deployment, newest first.
pub fn proposals(deployment: DeploymentId, limit: u32) -> Vec<ProposalReply> {
    let ceiling = bounded_limit(limit) as usize;
    let mut found: Vec<ProposalReply> =
        pallet_cerulea_governance::Proposals::<Runtime>::iter_prefix(deployment)
            .take(READ_CEILING as usize)
            .map(|(id, held)| proposal_reply(id, &held))
            .collect();
    found.sort_by(|left, right| right.id.cmp(&left.id));
    found.truncate(ceiling);
    found
}

/// One proposal.
pub fn proposal(deployment: DeploymentId, id: u64) -> Option<ProposalReply> {
    pallet_cerulea_governance::Proposals::<Runtime>::get(deployment, id)
        .map(|held| proposal_reply(id, &held))
}

/// The votes one proposal drew.
pub fn proposal_votes(deployment: DeploymentId, id: u64) -> Vec<VoteReply<AccountId>> {
    pallet_cerulea_governance::Votes::<Runtime>::iter_prefix((deployment, id))
        .take(READ_CEILING as usize)
        .map(|(voter, held)| VoteReply {
            voter,
            approve: held.approve,
            weight: held.weight,
            at_millis: held.at_millis,
        })
        .collect()
}

/// The proposals a module already carried out.
pub fn executed_changes(deployment: DeploymentId, limit: u32) -> Vec<ProposalReply> {
    let ceiling = bounded_limit(limit) as usize;
    let mut found: Vec<ProposalReply> =
        pallet_cerulea_governance::Proposals::<Runtime>::iter_prefix(deployment)
            .take(READ_CEILING as usize)
            .filter(|(_, held)| held.status == pallet_cerulea_governance::ProposalStatus::Executed)
            .map(|(id, held)| proposal_reply(id, &held))
            .collect();
    found.sort_by(|left, right| right.id.cmp(&left.id));
    found.truncate(ceiling);
    found
}

/// The number of entries the audit log of one deployment holds.
pub fn audit_length(deployment: DeploymentId) -> u64 {
    pallet_cerulea_audit::Pallet::<Runtime>::length(&deployment)
}

/// Build one audit reply.
fn audit_reply(
    held: pallet_cerulea_audit::AuditEntry<
        AccountId,
        crate::BlockNumber,
        <Runtime as pallet_cerulea_audit::Config>::MaxPayloadBytes,
    >,
) -> AuditReply<AccountId> {
    AuditReply {
        sequence: held.sequence,
        event: held.event.to_vec(),
        module: held.module.to_vec(),
        class: class_label(held.class),
        actor: held.actor,
        at_millis: held.at_millis,
        block: held.block.saturated_into::<u32>(),
        reason: held.reason.to_vec(),
        payload: held.payload.to_vec(),
        corrects: held.corrects,
        digest: held.digest,
    }
}

/// The entries of the audit log, from one position forward.
pub fn audit_entries(
    deployment: DeploymentId,
    from: u64,
    limit: u32,
) -> Vec<AuditReply<AccountId>> {
    let ceiling = bounded_limit(limit) as u64;
    let length = pallet_cerulea_audit::Pallet::<Runtime>::length(&deployment);
    let mut found = Vec::new();
    let mut sequence = from;
    while sequence < length && (found.len() as u64) < ceiling {
        if let Some(held) = pallet_cerulea_audit::Pallet::<Runtime>::entry(&deployment, sequence) {
            found.push(audit_reply(held));
        }
        sequence = sequence.saturating_add(1);
    }
    found
}

/// The entries one account raised.
pub fn audit_by_actor(
    deployment: DeploymentId,
    actor: AccountId,
    limit: u32,
) -> Vec<AuditReply<AccountId>> {
    let ceiling = bounded_limit(limit) as usize;
    let mut sequences: Vec<u64> =
        pallet_cerulea_audit::ActorIndex::<Runtime>::iter_key_prefix((deployment, actor))
            .take(READ_CEILING as usize)
            .collect();
    sequences.sort();
    sequences.truncate(ceiling);
    sequences
        .into_iter()
        .filter_map(|sequence| {
            pallet_cerulea_audit::Pallet::<Runtime>::entry(&deployment, sequence)
        })
        .map(audit_reply)
        .collect()
}

/// Where one deployment stands against its licence.
pub fn licence_state(deployment: DeploymentId) -> Option<LicenceReply> {
    use pallet_cerulea_tier::{Application, LicenceState, ProductionGate};
    let licence = pallet_cerulea_tier::Licences::<Runtime>::get(deployment)?;
    let state = pallet_cerulea_tier::States::<Runtime>::get(deployment);
    let application = match licence.tier.application {
        Application::Dapps => b"dapps".to_vec(),
        Application::Enterprise => b"enterprise".to_vec(),
        Application::Government => b"government".to_vec(),
    };
    let state_label = match state {
        Some(LicenceState::Live) => b"live".to_vec(),
        Some(LicenceState::Lapsed) => b"lapsed".to_vec(),
        Some(LicenceState::Suspended) => b"suspended".to_vec(),
        None => b"live".to_vec(),
    };
    let reason = pallet_cerulea_tier::SuspensionReason::<Runtime>::get(deployment)
        .map(|held| held.to_vec())
        .unwrap_or_default();
    Some(LicenceReply {
        application,
        level: licence.tier.level,
        features: licence.features.iter().map(name_bytes).collect(),
        addons: licence.addons.iter().map(name_bytes).collect(),
        seats: licence.seats,
        seats_used: pallet_cerulea_tier::SeatsUsed::<Runtime>::get(deployment),
        expires_at: licence.expires_at,
        state: state_label,
        suspension_reason: reason,
        production_permitted:
            <pallet_cerulea_tier::Pallet<Runtime> as ProductionGate>::production_permitted(
                &deployment,
            ),
    })
}

/// Report whether block production runs on this chain.
///
/// A deployment whose licence lapsed stops producing blocks. Every
/// existing record stays readable and exportable throughout. The
/// Authoring Engine reads this answer before it opens the author gate.
pub fn production_permitted() -> bool {
    use pallet_cerulea_tier::ProductionGate;
    pallet_cerulea_tier::Watched::<Runtime>::get()
        .iter()
        .all(|deployment| {
            <pallet_cerulea_tier::Pallet<Runtime> as ProductionGate>::production_permitted(
                deployment,
            )
        })
}

/// The feeds one deployment declares.
pub fn oracle_feeds(deployment: DeploymentId) -> Vec<Vec<u8>> {
    pallet_cerulea_oracle::FeedNames::<Runtime>::get(deployment)
        .iter()
        .map(name_bytes)
        .collect()
}

/// The current value of one feed.
pub fn oracle_value(deployment: DeploymentId, feed: Vec<u8>) -> Option<OracleReply> {
    let name = bounded_of(&feed)?;
    let (value, attested_at) =
        pallet_cerulea_oracle::Pallet::<Runtime>::current_value(&deployment, &name)?;
    let round = pallet_cerulea_oracle::Current::<Runtime>::get(deployment, &name)
        .map(|held| held.round)
        .unwrap_or_default();
    Some(OracleReply {
        feed,
        round,
        value: Value::encode(&value),
        attested_at,
    })
}

/// The recorded versions of one deployment.
pub fn versions(deployment: DeploymentId) -> Vec<VersionReply> {
    let active = pallet_cerulea_evolution::ActiveVersion::<Runtime>::get(deployment);
    let mut found: Vec<VersionReply> =
        pallet_cerulea_evolution::Versions::<Runtime>::iter_prefix(deployment)
            .take(READ_CEILING as usize)
            .map(|(revision, held)| VersionReply {
                revision,
                blueprint: held.blueprint,
                recorded_at: held.recorded_at,
                recorded_block: held.recorded_block.saturated_into::<u32>(),
                summary: held.summary.to_vec(),
                active: revision == active,
            })
            .collect();
    found.sort_by_key(|held| held.revision);
    found
}

/// The label of one plan status.
fn plan_status(status: pallet_cerulea_evolution::PlanStatus) -> Vec<u8> {
    use pallet_cerulea_evolution::PlanStatus;
    match status {
        PlanStatus::Planned => b"planned".to_vec(),
        PlanStatus::Checked => b"checked".to_vec(),
        PlanStatus::Applied => b"applied".to_vec(),
        PlanStatus::Verified => b"verified".to_vec(),
        PlanStatus::RolledBack => b"rolled_back".to_vec(),
    }
}

/// The migration plans of one deployment with their rollback windows.
pub fn rollback_windows(deployment: DeploymentId) -> Vec<RollbackWindowReply> {
    let now = pallet_cerulea_state::Pallet::<Runtime>::now_millis();
    let mut found: Vec<RollbackWindowReply> =
        pallet_cerulea_evolution::Plans::<Runtime>::iter_prefix(deployment)
            .take(READ_CEILING as usize)
            .map(|(plan, held)| {
                let open = held.reversible
                    && held.status == pallet_cerulea_evolution::PlanStatus::Applied
                    && held.rollback_deadline.map(|end| now < end).unwrap_or(false);
                RollbackWindowReply {
                    plan,
                    collection: held.collection.to_vec(),
                    from_version: held.from_version,
                    to_version: held.to_version,
                    status: plan_status(held.status),
                    breaking: held.breaking,
                    reversible: held.reversible,
                    window_millis: held.rollback_window_millis,
                    deadline: held.rollback_deadline,
                    open,
                }
            })
            .collect();
    found.sort_by_key(|held| held.plan);
    found
}

/// The accounts holding the Cerulea operator role.
pub fn operator_accounts(deployment: DeploymentId) -> Vec<AccountId> {
    pallet_cerulea_operator::Operators::<Runtime>::iter_prefix(deployment)
        .take(READ_CEILING as usize)
        .filter(|(_, record)| record.active)
        .map(|(account, _)| account)
        .collect()
}

/// The class thresholds and the authoring figures the chain runs under.
pub fn consensus_profile() -> ConsensusProfileReply {
    ConsensusProfileReply {
        routine_percent: WorkClass::Routine.threshold_percent(),
        financial_percent: WorkClass::Financial.threshold_percent(),
        structural_percent: WorkClass::Structural.threshold_percent(),
        constitutional_percent: WorkClass::Constitutional.threshold_percent(),
        committee_size: cerulea_primitives::COMMITTEE_SIZE as u32,
        block_time_millis: crate::MILLI_SECS_PER_BLOCK,
        epoch_length: <Runtime as pallet_cerulea_dcf::Config>::EpochLength::get(),
    }
}

/// The label of one transaction class.
fn transaction_class_label(class: cerulea_blueprint::actions::TransactionClass) -> Vec<u8> {
    use cerulea_blueprint::actions::TransactionClass;
    match class {
        TransactionClass::Routine => b"routine".to_vec(),
        TransactionClass::Financial => b"financial".to_vec(),
        TransactionClass::Structural => b"structural".to_vec(),
        TransactionClass::Constitutional => b"constitutional".to_vec(),
    }
}

/// What one action reads, writes and needs.
///
/// The Access Manifest of a transaction comes from this declaration. The
/// collections the action writes name its conflict domain.
pub fn action_manifest(
    deployment: DeploymentId,
    action: Vec<u8>,
) -> Option<cerulea_platform_api::ActionManifestReply> {
    use cerulea_blueprint::actions::Effect;
    let name = String::from_utf8(action.clone()).ok()?;
    let blueprint = pallet_cerulea_logic::Installed::<Runtime>::get(deployment)?;
    let held = blueprint
        .actions
        .iter()
        .find(|candidate| candidate.id.as_str() == name)?;

    let mut reads: Vec<Vec<u8>> = Vec::new();
    let mut writes: Vec<Vec<u8>> = Vec::new();
    let note = |list: &mut Vec<Vec<u8>>, collection: &CollectionId| {
        let bytes = collection.as_str().as_bytes().to_vec();
        if !list.contains(&bytes) {
            list.push(bytes);
        }
    };
    for effect in held.effects.iter() {
        match effect {
            Effect::Insert { collection, .. }
            | Effect::Update { collection, .. }
            | Effect::Delete { collection, .. } => note(&mut writes, collection),
            Effect::Read { collection, .. } => note(&mut reads, collection),
            Effect::Transition { collection, .. } => note(&mut writes, collection),
            _ => {}
        }
    }

    Some(cerulea_platform_api::ActionManifestReply {
        action,
        class: transaction_class_label(held.class),
        reads,
        writes,
        roles: held
            .roles
            .iter()
            .map(|role| role.as_str().as_bytes().to_vec())
            .collect(),
        emits: held
            .emits
            .iter()
            .map(|event| event.as_str().as_bytes().to_vec())
            .collect(),
    })
}

/// The actions one deployment declares.
pub fn action_names(deployment: DeploymentId) -> Vec<Vec<u8>> {
    match pallet_cerulea_logic::Installed::<Runtime>::get(deployment) {
        Some(held) => held
            .actions
            .iter()
            .map(|action| action.id.as_str().as_bytes().to_vec())
            .collect(),
        None => Vec::new(),
    }
}

/// The custody challenge history of one validator.
///
/// The history joins the open challenges with the settled log of the custody
/// module, so a challenge stays readable after it settles. The reply holds
/// the newest `limit` challenges, oldest first.
pub fn custody_history(validator: AccountId, limit: u32) -> Vec<CustodyReply<AccountId>> {
    use pallet_cerulea_custody::primitives::ChallengeState;
    let ceiling = bounded_limit(limit) as usize;
    let reply = |challenge: &pallet_cerulea_custody::ChallengeOf<Runtime>,
                 state: ChallengeState| {
        CustodyReply {
            validator: challenge.target.clone(),
            issued_block: challenge.issued.saturated_into::<u32>(),
            deadline_block: challenge.deadline.saturated_into::<u32>(),
            answered: state == ChallengeState::AnsweredInTime || state == ChallengeState::Incorrect,
            verified: state == ChallengeState::AnsweredInTime,
            settled: state != ChallengeState::Open,
        }
    };
    let mut found: Vec<(u64, CustodyReply<AccountId>)> =
        pallet_cerulea_custody::SettledChallenges::<Runtime>::get(&validator)
            .iter()
            .map(|(id, challenge, state)| (*id, reply(challenge, *state)))
            .collect();
    for id in pallet_cerulea_custody::OpenChallengeIds::<Runtime>::get().iter() {
        if let Some(challenge) = pallet_cerulea_custody::OpenChallenges::<Runtime>::get(id) {
            if challenge.target == validator {
                found.push((*id, reply(&challenge, ChallengeState::Open)));
            }
        }
    }
    // Challenge identifiers rise with issue order, so they order the history.
    found.sort_by_key(|(id, _)| *id);
    let skip = found.len().saturating_sub(ceiling);
    found.into_iter().skip(skip).map(|(_, held)| held).collect()
}

/// The custody counters of one validator, read from its custody record.
pub fn custody_counts(validator: AccountId) -> CustodyCountsReply {
    let record = pallet_cerulea_custody::CustodyRecords::<Runtime>::get(&validator);
    CustodyCountsReply {
        answered: record.answered,
        missed: record.missed,
        faults: record.faults,
        share: pallet_cerulea_custody::Pallet::<Runtime>::custody_share(&validator),
    }
}
