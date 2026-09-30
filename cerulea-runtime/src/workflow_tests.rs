//! The workflow adapter and the workflow reads, run against the real runtime.
//!
//! A land registry Blueprint declares one registration workflow and actions
//! that move a parcel between its states. The logic module hands every
//! transition effect to the runtime adapter, and the adapter asks the
//! workflow module to take the step. Each refusal names its fix.

use crate::platform_api as api;
use crate::platform_tests::development_ext;
use crate::{AccountId, Runtime, RuntimeOrigin};
use cerulea_blueprint::{
    actions::{Action, Effect, Param},
    collections::{Collection, KeySelector, PurposeDeclaration},
    expression::Expr,
    identities::RoleDef,
    types::{
        CollectionId, EnumerationDef, FieldDef, FieldName, FieldPath, ParamName, RecordDef, RoleId,
        StateId, TypeDef, TypeExpr, TypeId, Value, VariantName, WorkflowId,
    },
    workflows::{CyclePolicy, Transition, Workflow, WorkflowState},
    Blueprint, BlueprintMeta,
};
use codec::Encode;
use frame_support::{assert_noop, assert_ok, BoundedVec};
use pallet_cerulea_permission::{CapabilityKey, GrantPayload};
use pallet_cerulea_schema::primitives::{name_of, BoundedName, DeploymentId, RecordKey};
use sp_keyring::Ed25519Keyring;
use sp_runtime::DispatchError;

type Logic = pallet_cerulea_logic::Pallet<Runtime>;
type Identity = pallet_cerulea_identity::Pallet<Runtime>;
type Permission = pallet_cerulea_permission::Pallet<Runtime>;
type Workflows = pallet_cerulea_workflow::Pallet<Runtime>;
type State = pallet_cerulea_state::Pallet<Runtime>;

fn registrar() -> AccountId {
    Ed25519Keyring::Alice.to_account_id()
}

fn name(text: &str) -> BoundedName {
    name_of(text).expect("the name fits the bound")
}

fn bounded<M: frame_support::traits::Get<u32>>(bytes: Vec<u8>) -> BoundedVec<u8, M> {
    BoundedVec::try_from(bytes).expect("the payload sits inside the bound")
}

fn stage_type() -> TypeDef {
    TypeDef::Enumeration(EnumerationDef {
        id: TypeId::new("parcel_stage"),
        variants: vec![
            VariantName::new("submitted"),
            VariantName::new("surveyed"),
            VariantName::new("registered"),
        ],
        summary: "The stages a parcel passes through in the register.".into(),
    })
}

fn parcel_type() -> TypeDef {
    TypeDef::Record(RecordDef {
        id: TypeId::new("parcel"),
        fields: vec![
            FieldDef::new(
                "survey_number",
                TypeExpr::Text,
                "The survey number of the parcel.",
            ),
            FieldDef::new(
                "stage",
                TypeExpr::Enumeration(TypeId::new("parcel_stage")),
                "The stage the parcel stands at.",
            ),
        ],
        summary: "One land parcel in the register.".into(),
    })
}

fn collection(id: &str) -> Collection {
    let mut collection = Collection::new(
        id,
        "parcel",
        "survey_number",
        "The register of land parcels.",
    );
    collection.purpose = PurposeDeclaration::new(
        "The register records the stage of each land parcel.",
        &["registration"],
    );
    collection
}

fn machine() -> Workflow {
    Workflow {
        id: WorkflowId::new("registration"),
        collection: CollectionId::new("parcel"),
        state_field: FieldPath::parse("stage"),
        states: vec![
            WorkflowState::passing("submitted", "Submitted", "The holder submitted the parcel."),
            WorkflowState::passing("surveyed", "Surveyed", "The surveyor measured the parcel."),
            WorkflowState::resting("registered", "Registered", "The register holds the parcel."),
        ],
        initial: StateId::new("submitted"),
        transitions: vec![
            Transition::new("survey", "submitted", "surveyed"),
            Transition::new("register", "surveyed", "registered"),
        ],
        timers: Vec::new(),
        deadlines: Vec::new(),
        cycles: CyclePolicy::Forbid,
        summary: "The registration of one land parcel.".into(),
    }
}

/// An action moving the parcel named by its survey number.
fn move_action(id: &str, collection: &str, to: &str) -> Action {
    let mut action = Action::new(id, "The registrar moves one parcel to another stage.");
    action.params = vec![Param::new(
        "survey",
        TypeExpr::Text,
        "The survey number of the parcel.",
    )];
    action.effects = vec![Effect::Transition {
        workflow: WorkflowId::new("registration"),
        collection: CollectionId::new(collection),
        key: KeySelector::Exact(Expr::bound("survey")),
        to: StateId::new(to),
    }];
    action.roles.push(RoleId::new("registrar"));
    action
}

fn registry() -> Blueprint {
    let mut blueprint = Blueprint::new(BlueprintMeta::new(
        "district land registration",
        "The registry moves land parcels through registration.",
        "Kollam District Administration",
    ));
    blueprint.types.push(stage_type());
    blueprint.types.push(parcel_type());
    blueprint.collections.push(collection("parcel"));
    blueprint.collections.push(collection("archive"));
    blueprint.identities.roles.push(RoleDef::new(
        "registrar",
        None,
        "Registrar",
        "The registrar moves parcels through registration.",
    ));
    blueprint.workflows.push(machine());
    blueprint
        .actions
        .push(move_action("survey_parcel", "parcel", "surveyed"));
    blueprint
        .actions
        .push(move_action("register_parcel", "parcel", "registered"));
    blueprint
        .actions
        .push(move_action("survey_archive", "archive", "surveyed"));
    blueprint
}

fn record_key(survey: &str) -> RecordKey {
    let mut value = Value::Text(survey.into());
    value.canonicalise();
    BoundedVec::try_from(value.canonical_bytes()).expect("the key fits")
}

fn parcel(survey: &str, stage: &str) -> Value {
    let mut value = Value::record(
        TypeId::new("parcel"),
        vec![
            (FieldName::new("survey_number"), Value::Text(survey.into())),
            (
                FieldName::new("stage"),
                Value::enumeration(TypeId::new("parcel_stage"), VariantName::new(stage)),
            ),
        ],
    );
    value.canonicalise();
    value
}

/// Install the registry, grant the registrar every capability and land one
/// parcel. The workflow machine installs only when asked.
fn install(with_machine: bool) -> DeploymentId {
    let blueprint = registry();
    assert_ok!(Logic::install_blueprint(
        RuntimeOrigin::signed(registrar()),
        bounded(blueprint.encode())
    ));
    let deployment = blueprint.id().0;
    assert_ok!(Identity::register_account(
        RuntimeOrigin::signed(registrar()),
        registrar(),
        [7u8; 32]
    ));
    assert_ok!(Identity::define_role(
        RuntimeOrigin::signed(registrar()),
        deployment,
        name("registrar"),
        None,
        BoundedVec::default(),
        BoundedVec::truncate_from(b"Registrar".to_vec())
    ));
    assert_ok!(Identity::assign_role(
        RuntimeOrigin::signed(registrar()),
        deployment,
        registrar(),
        name("registrar"),
        None
    ));
    for held in ["parcel", "archive"] {
        let mut capabilities = vec![
            CapabilityKey::Read,
            CapabilityKey::Insert,
            CapabilityKey::Update,
            CapabilityKey::Delete,
        ];
        for action in ["survey_parcel", "register_parcel", "survey_archive"] {
            capabilities.push(CapabilityKey::Invoke(name(action)));
        }
        for capability in capabilities {
            assert_ok!(Permission::grant(
                RuntimeOrigin::signed(registrar()),
                deployment,
                name("registrar"),
                name(held),
                capability,
                bounded(GrantPayload::everything().encode())
            ));
        }
    }
    assert_ok!(State::insert(
        RuntimeOrigin::signed(registrar()),
        deployment,
        bounded(b"parcel".to_vec()),
        bounded(parcel("12/3", "submitted").canonical_bytes())
    ));
    if with_machine {
        assert_ok!(Workflows::install_machine(
            RuntimeOrigin::signed(registrar()),
            deployment,
            bounded(machine().encode())
        ));
    }
    deployment
}

fn start(deployment: DeploymentId) {
    assert_ok!(Workflows::start_instance(
        RuntimeOrigin::signed(registrar()),
        deployment,
        name("registration"),
        record_key("12/3")
    ));
}

fn run(deployment: DeploymentId, action: &str) -> frame_support::dispatch::DispatchResult {
    let arguments: pallet_cerulea_logic::Bindings =
        vec![(ParamName::new("survey"), Value::Text("12/3".into()))];
    Logic::execute_action(
        RuntimeOrigin::signed(registrar()),
        deployment,
        name(action),
        bounded(arguments.encode()),
    )
}

#[test]
fn a_transition_effect_moves_the_process_through_the_workflow_module() {
    development_ext().execute_with(|| {
        let deployment = install(true);
        start(deployment);
        assert_eq!(
            api::workflow_names(deployment),
            vec![b"registration".to_vec()]
        );

        assert_ok!(run(deployment, "survey_parcel"));
        let process = api::workflow_instance(deployment, b"registration".to_vec(), 0)
            .expect("the process exists");
        assert_eq!(process.workflow, b"registration".to_vec());
        assert_eq!(process.instance, 0);
        assert_eq!(process.record, record_key("12/3").to_vec());
        assert_eq!(process.state, b"surveyed".to_vec());
        assert_eq!(process.status, b"running".to_vec());
        assert_eq!(process.history.len(), 1);
        assert_eq!(process.history[0].0, b"survey".to_vec());
        assert_eq!(process.history[0].1, b"submitted".to_vec());
        assert_eq!(process.history[0].2, b"surveyed".to_vec());

        assert_ok!(run(deployment, "register_parcel"));
        let settled = api::workflow_instance(deployment, b"registration".to_vec(), 0)
            .expect("the process exists");
        assert_eq!(settled.state, b"registered".to_vec());
        assert_eq!(settled.status, b"settled".to_vec());

        let listed = api::workflow_instances(deployment, b"registration".to_vec(), 10);
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].instance, 0);
        assert!(api::workflow_instances(deployment, vec![0xff], 10).is_empty());
        assert_eq!(
            api::workflow_instance(deployment, b"registration".to_vec(), 7),
            None
        );
        assert_eq!(api::workflow_instance(deployment, vec![0xff], 0), None);
    });
}

#[test]
fn a_transition_to_an_unreached_state_names_the_fix() {
    development_ext().execute_with(|| {
        let deployment = install(true);
        start(deployment);
        assert_noop!(
            run(deployment, "register_parcel"),
            DispatchError::Other(
                "The workflow declares no transition from the present state to that state. Name a state the present state reaches."
            )
        );
    });
}

#[test]
fn a_transition_without_a_running_process_names_the_fix() {
    development_ext().execute_with(|| {
        let deployment = install(true);
        assert_noop!(
            run(deployment, "survey_parcel"),
            DispatchError::Other(
                "The record runs no process of that workflow. Start the process before moving the record."
            )
        );
    });
}

#[test]
fn a_transition_over_another_collection_names_the_fix() {
    development_ext().execute_with(|| {
        let deployment = install(true);
        start(deployment);
        // The archive holds the same parcel, so the action reaches the
        // adapter with a record the workflow never drives.
        assert_ok!(State::insert(
            RuntimeOrigin::signed(registrar()),
            deployment,
            bounded(b"archive".to_vec()),
            bounded(parcel("12/3", "submitted").canonical_bytes())
        ));
        assert_noop!(
            run(deployment, "survey_archive"),
            DispatchError::Other(
                "The workflow drives another collection. Name the workflow that drives this collection."
            )
        );
    });
}

#[test]
fn a_transition_through_an_uninstalled_machine_declines() {
    development_ext().execute_with(|| {
        let deployment = install(false);
        assert!(api::workflow_names(deployment).is_empty());
        assert_noop!(
            run(deployment, "survey_parcel"),
            pallet_cerulea_workflow::Error::<Runtime>::MachineUnknown
        );
    });
}
