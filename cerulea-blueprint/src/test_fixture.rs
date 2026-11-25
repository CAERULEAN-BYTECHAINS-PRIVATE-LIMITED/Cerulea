//! A complete land registry Blueprint shared by the Blueprint level tests.
//!
//! Every one of the twelve sections and the composition library holds at
//! least one element, and every nested list holds at least one member. The
//! Blueprint validates clean, so a test breaks one rule and reads the one
//! diagnostic that rule raises.

use crate::{
    actions::{Action, BlockOutput, Effect, Param, TransactionClass},
    collections::{Collection, Index, KeySelector, PrivacyClass, PurposeDeclaration},
    compose::Block,
    economics::{AmountLimit, BalanceSheet, Economics, StandardAmount, UnitOfAccount},
    events::{EventDef, Subscriber},
    expression::{BinaryOp, Builtin, Expr},
    governance::{ChangeProcess, ChangeStage, Governance, QuorumRule, UpgradeAuthority},
    identities::{
        CredentialDef, CredentialIssuer, CredentialValidity, DelegationRule, Identities,
        OrganisationUnit, RoleDef,
    },
    integrations::{
        AttestationRequirement, BridgeDirection, BridgeEndpoint, ExternalTrigger, Feed,
        Integrations,
    },
    permissions::{Capability, CapabilityEntry, Permissions, ROW_BINDING},
    rules::{Invariant, Precondition, RuleInput, Rules},
    types::{
        ActionId, BlockId, CollectionId, CredentialId, Decimal, DelegationId, EnumerationDef,
        EventId, FieldDef, FieldName, FieldPath, IndexName, IntegrationId, ParamName, PurposeTag,
        RecordDef, RoleId, RuleId, StateId, TimerId, TypeDef, TypeExpr, TypeId, UnitId,
        VariantName, WorkflowId,
    },
    views::{Aggregate, Projection, View},
    workflows::{CyclePolicy, Deadline, Timer, TimerAction, Transition, Workflow, WorkflowState},
    Blueprint, BlueprintMeta,
};
use alloc::{string::ToString, vec, vec::Vec};

fn row(field: &str) -> Expr {
    Expr::field(Expr::bound(ROW_BINDING), field)
}

fn types() -> Vec<TypeDef> {
    vec![
        TypeDef::Record(RecordDef {
            id: TypeId::new("parcel"),
            fields: vec![
                FieldDef::new("survey_number", TypeExpr::Text, "The survey number."),
                FieldDef::new("holder", TypeExpr::Identity, "The registered holder."),
                FieldDef::new("area", TypeExpr::Decimal { scale: 2 }, "The area."),
                FieldDef::new(
                    "stage",
                    TypeExpr::Enumeration(TypeId::new("stage")),
                    "The stage.",
                ),
                FieldDef::new("filed_at", TypeExpr::Timestamp, "The filing time."),
            ],
            summary: "One land parcel.".to_string(),
        }),
        TypeDef::Enumeration(EnumerationDef {
            id: TypeId::new("stage"),
            variants: vec![VariantName::new("draft"), VariantName::new("recorded")],
            summary: "The stage of a parcel.".to_string(),
        }),
        TypeDef::Record(RecordDef {
            id: TypeId::new("ledger"),
            fields: vec![
                FieldDef::new("entry", TypeExpr::Text, "The entry reference."),
                FieldDef::new("amount", TypeExpr::Decimal { scale: 2 }, "The amount."),
                FieldDef::new("owner", TypeExpr::Identity, "The owner."),
            ],
            summary: "One ledger line.".to_string(),
        }),
    ]
}

fn collections() -> Vec<Collection> {
    let mut parcel = Collection::new("parcel", "parcel", "survey_number", "The parcels.");
    parcel.purpose = PurposeDeclaration::new("The register records land.", &["registration"]);
    parcel.privacy = PrivacyClass::Encrypted {
        key_holders: vec![RoleId::new("registrar")],
    };
    let mut ledger = Collection::new("ledger", "ledger", "entry", "The fee ledger.");
    ledger.purpose = PurposeDeclaration::new("The ledger records fees.", &["accounting"]);
    ledger.indexes = vec![
        Index {
            name: IndexName::new("by_owner"),
            fields: vec![FieldPath::parse("owner")],
            unique: false,
        },
        Index {
            name: IndexName::new("by_amount"),
            fields: vec![FieldPath::parse("amount")],
            unique: false,
        },
    ];
    vec![parcel, ledger]
}

fn identities() -> Identities {
    let mut registrar = RoleDef::new(
        "registrar",
        Some("district"),
        "Registrar",
        "The registrar records parcels.",
    );
    registrar.requires = vec![CredentialId::new("licence"), CredentialId::new("badge")];
    Identities {
        units: vec![OrganisationUnit::top(
            "district",
            "District office",
            "The office serves one district.",
        )],
        roles: vec![
            registrar,
            RoleDef::new("clerk", None, "Clerk", "The clerk files papers."),
        ],
        credentials: vec![
            CredentialDef {
                id: CredentialId::new("licence"),
                issuer: CredentialIssuer::Unit(UnitId::new("district")),
                validity: CredentialValidity::UntilRevoked,
                attests: "The holder passed the examination.".to_string(),
            },
            CredentialDef {
                id: CredentialId::new("badge"),
                issuer: CredentialIssuer::Unit(UnitId::new("district")),
                validity: CredentialValidity::UntilRevoked,
                attests: "The holder works in the office.".to_string(),
            },
        ],
        delegations: vec![DelegationRule {
            id: DelegationId::new("cover"),
            from: RoleId::new("registrar"),
            to: RoleId::new("clerk"),
            scope: vec![ActionId::new("record_parcel"), ActionId::new("charge_fee")],
            maximum_millis: 3_600_000,
            requires: None,
        }],
    }
}

fn permissions() -> Permissions {
    Permissions {
        entries: vec![
            CapabilityEntry::grant("registrar", "parcel", Capability::Read)
                .where_rows(Expr::binary(
                    BinaryOp::GreaterOrEqual,
                    row("area"),
                    Expr::decimal(0, 2),
                ))
                .for_purposes(&["registration"]),
            CapabilityEntry::grant("registrar", "parcel", Capability::Insert),
            CapabilityEntry::grant(
                "registrar",
                "parcel",
                Capability::Invoke(ActionId::new("record_parcel")),
            ),
            CapabilityEntry::grant("clerk", "ledger", Capability::Read)
                .for_purposes(&["accounting"]),
        ],
    }
}

fn rules() -> Rules {
    Rules {
        invariants: vec![
            Invariant {
                id: RuleId::new("area_positive"),
                collection: CollectionId::new("parcel"),
                holds: Expr::binary(BinaryOp::GreaterOrEqual, row("area"), Expr::decimal(0, 2)),
                problem: "The parcel covers negative land.".to_string(),
                fix: "Enter an area at zero or above.".to_string(),
            },
            Invariant {
                id: RuleId::new("amount_positive"),
                collection: CollectionId::new("ledger"),
                holds: Expr::binary(BinaryOp::GreaterOrEqual, row("amount"), Expr::decimal(0, 2)),
                problem: "The ledger line carries a negative amount.".to_string(),
                fix: "Enter an amount at zero or above.".to_string(),
            },
        ],
        preconditions: vec![
            Precondition {
                id: RuleId::new("area_fits"),
                inputs: vec![RuleInput::new("area", TypeExpr::Decimal { scale: 2 })],
                holds: Expr::binary(BinaryOp::Greater, Expr::bound("area"), Expr::decimal(0, 2)),
                problem: "The parcel covers zero land.".to_string(),
                fix: "Enter an area above zero.".to_string(),
            },
            Precondition {
                id: RuleId::new("amount_fits"),
                inputs: vec![RuleInput::new("amount", TypeExpr::Decimal { scale: 2 })],
                holds: Expr::binary(
                    BinaryOp::Greater,
                    Expr::bound("amount"),
                    Expr::decimal(0, 2),
                ),
                problem: "The fee is zero.".to_string(),
                fix: "Enter a fee above zero.".to_string(),
            },
        ],
    }
}

fn key() -> KeySelector {
    KeySelector::Exact(Expr::bound("survey_number"))
}

fn actions() -> Vec<Action> {
    let mut record = Action::new("record_parcel", "The action records one parcel.");
    record.params = vec![
        Param::new("survey_number", TypeExpr::Text, "The parcel."),
        Param::new("area", TypeExpr::Decimal { scale: 2 }, "The area."),
    ];
    record.preconditions = vec![RuleId::new("area_fits")];
    record.roles = vec![RoleId::new("registrar"), RoleId::new("clerk")];
    record.emits = vec![EventId::new("recorded"), EventId::new("stamped")];
    record.effects = vec![
        Effect::Insert {
            collection: CollectionId::new("parcel"),
            key: key(),
            record: Expr::MakeRecord {
                type_id: TypeId::new("parcel"),
                fields: vec![
                    (
                        FieldName::new("survey_number"),
                        Expr::bound("survey_number"),
                    ),
                    (
                        FieldName::new("holder"),
                        Expr::Call {
                            function: Builtin::Caller,
                            args: Vec::new(),
                        },
                    ),
                    (FieldName::new("area"), Expr::bound("area")),
                    (
                        FieldName::new("stage"),
                        Expr::MakeEnumeration {
                            type_id: TypeId::new("stage"),
                            variant: VariantName::new("draft"),
                        },
                    ),
                    (
                        FieldName::new("filed_at"),
                        Expr::Call {
                            function: Builtin::Now,
                            args: Vec::new(),
                        },
                    ),
                ],
            },
        },
        Effect::Update {
            collection: CollectionId::new("parcel"),
            key: key(),
            field: FieldPath::parse("area"),
            value: Expr::bound("area"),
        },
        Effect::Read {
            collection: CollectionId::new("parcel"),
            key: KeySelector::Prefix(Expr::bound("survey_number")),
            binding: ParamName::new("found"),
            element: TypeExpr::Record(TypeId::new("parcel")),
        },
        Effect::Emit {
            event: EventId::new("recorded"),
            payload: Expr::bound("survey_number"),
        },
        Effect::Transition {
            workflow: WorkflowId::new("parcel_life"),
            collection: CollectionId::new("parcel"),
            key: key(),
            to: StateId::new("recorded"),
        },
        Effect::RunBlock {
            block: BlockId::new("stamp"),
            arguments: vec![
                (ParamName::new("note"), Expr::bound("survey_number")),
                (ParamName::new("fee"), Expr::decimal(100, 2)),
            ],
        },
    ];
    record.compensation = vec![Effect::Delete {
        collection: CollectionId::new("parcel"),
        key: key(),
    }];

    let mut charge = Action::new("charge_fee", "The action charges one fee.");
    charge.params = vec![
        Param::new("entry", TypeExpr::Text, "The entry."),
        Param::new("amount", TypeExpr::Decimal { scale: 2 }, "The amount."),
    ];
    charge.preconditions = vec![RuleId::new("amount_fits")];
    charge.roles = vec![RoleId::new("clerk")];
    charge.class = TransactionClass::Financial;
    charge.effects = vec![Effect::Delete {
        collection: CollectionId::new("ledger"),
        key: KeySelector::Exact(Expr::bound("entry")),
    }];
    vec![record, charge]
}

fn blocks() -> Vec<Block> {
    let mut stamp = Block::new("stamp", "The block writes one ledger line.");
    stamp.inputs = vec![
        Param::new("note", TypeExpr::Text, "The note."),
        Param::new("fee", TypeExpr::Decimal { scale: 2 }, "The fee."),
    ];
    stamp.outputs = vec![BlockOutput::new(
        "stamped",
        TypeExpr::Text,
        Expr::bound("note"),
    )];
    stamp.effects = vec![Effect::Insert {
        collection: CollectionId::new("ledger"),
        key: KeySelector::Exact(Expr::bound("note")),
        record: Expr::MakeRecord {
            type_id: TypeId::new("ledger"),
            fields: vec![
                (FieldName::new("entry"), Expr::bound("note")),
                (FieldName::new("amount"), Expr::bound("fee")),
                (
                    FieldName::new("owner"),
                    Expr::Call {
                        function: Builtin::Caller,
                        args: Vec::new(),
                    },
                ),
            ],
        },
    }];
    let mut empty_note = Block::new("clear", "The block removes one ledger line.");
    empty_note.inputs = vec![Param::new("note", TypeExpr::Text, "The note.")];
    empty_note.effects = vec![Effect::Delete {
        collection: CollectionId::new("ledger"),
        key: KeySelector::Exact(Expr::bound("note")),
    }];
    vec![stamp, empty_note]
}

fn workflows() -> Vec<Workflow> {
    vec![Workflow {
        id: WorkflowId::new("parcel_life"),
        collection: CollectionId::new("parcel"),
        state_field: FieldPath::parse("stage"),
        states: vec![
            WorkflowState::passing("draft", "Draft", "The parcel waits."),
            WorkflowState::resting("recorded", "Recorded", "The parcel rests."),
        ],
        initial: StateId::new("draft"),
        transitions: vec![Transition::new("record", "draft", "recorded")
            .guarded_by(Expr::binary(
                BinaryOp::Greater,
                row("area"),
                Expr::decimal(0, 2),
            ))
            .carried_by("record_parcel")
            .run_by(&["registrar", "clerk"])],
        timers: vec![
            Timer {
                id: TimerId::new("remind"),
                state: StateId::new("draft"),
                after_millis: 60_000,
                on_expiry: TimerAction::Emit(EventId::new("recorded")),
            },
            Timer {
                id: TimerId::new("escalate"),
                state: StateId::new("draft"),
                after_millis: 120_000,
                on_expiry: TimerAction::Run(ActionId::new("record_parcel")),
            },
        ],
        deadlines: vec![
            Deadline {
                id: TimerId::new("lapse"),
                state: StateId::new("draft"),
                due: Expr::Call {
                    function: Builtin::TimestampPlus,
                    args: vec![
                        Expr::field(Expr::bound(ROW_BINDING), "filed_at"),
                        Expr::integer(86_400_000),
                    ],
                },
                on_miss: TimerAction::MoveTo(StateId::new("recorded")),
            },
            Deadline {
                id: TimerId::new("notice"),
                state: StateId::new("draft"),
                due: row("filed_at"),
                on_miss: TimerAction::Emit(EventId::new("stamped")),
            },
        ],
        cycles: CyclePolicy::Forbid,
        summary: "The parcel moves from draft to recorded.".to_string(),
    }]
}

fn events() -> Vec<EventDef> {
    let mut recorded = EventDef::new("recorded", "parcel", "The registry recorded a parcel.");
    recorded.subscribers = vec![
        Subscriber::workflow("parcel_life", "The workflow follows the parcel."),
        Subscriber::view("parcels", "The view lists the parcel."),
        Subscriber::integration("land_feed", "The feed hears of the parcel."),
    ];
    let stamped = EventDef::new("stamped", "ledger", "The registry stamped a fee.");
    vec![recorded, stamped]
}

fn views() -> Vec<View> {
    let mut parcels = View::new("parcels", "parcel", "The view lists the parcels.");
    parcels.projection = vec![Projection::field("survey_number")];
    parcels.filter = Some(Expr::binary(
        BinaryOp::Greater,
        row("area"),
        Expr::decimal(0, 2),
    ));
    parcels.group_by = vec![FieldName::new("survey_number")];
    parcels.aggregates = vec![Aggregate::total("total_area", row("area"))];
    parcels.purposes = vec![PurposeTag("registration".to_string())];
    parcels.roles = vec![RoleId::new("registrar"), RoleId::new("clerk")];
    let mut fees = View::new("fees", "ledger", "The view counts the fees.");
    fees.aggregates = vec![Aggregate::count("fee_count")];
    fees.purposes = vec![PurposeTag("accounting".to_string())];
    vec![parcels, fees]
}

fn integrations() -> Integrations {
    Integrations {
        feeds: vec![Feed {
            id: IntegrationId::new("land_feed"),
            target: CollectionId::new("ledger"),
            record_type: TypeId::new("ledger"),
            refresh_millis: 60_000,
            attestation: AttestationRequirement::signed_by(1, &["registrar", "clerk"]),
            summary: "The feed brings fees from the treasury.".to_string(),
        }],
        bridges: vec![BridgeEndpoint {
            id: IntegrationId::new("state_bridge"),
            network: "State land records".to_string(),
            direction: BridgeDirection::Outbound,
            collection: CollectionId::new("parcel"),
            attestation: AttestationRequirement::signed_by(1, &["registrar", "clerk"]),
            summary: "The bridge sends parcels to the state.".to_string(),
        }],
        triggers: vec![ExternalTrigger {
            id: IntegrationId::new("court_order"),
            action: ActionId::new("record_parcel"),
            attestation: AttestationRequirement::signed_by(1, &["registrar", "clerk"]),
            summary: "A court order records a parcel.".to_string(),
        }],
    }
}

fn governance() -> Governance {
    Governance {
        authority: UpgradeAuthority::new(&["registrar", "clerk"], 67, 1),
        quorum: vec![
            QuorumRule::standard(TransactionClass::Structural),
            QuorumRule::standard(TransactionClass::Routine),
        ],
        process: ChangeProcess {
            stages: vec![ChangeStage::new(
                "review",
                &["registrar", "clerk"],
                86_400_000,
                "The registrar reviews the change.",
            )],
        },
    }
}

fn economics() -> Economics {
    let mut economics = Economics::new(UnitOfAccount::new("INR", "Indian rupee", 2));
    economics.sheets = vec![BalanceSheet {
        collection: CollectionId::new("ledger"),
        amount_field: FieldPath::parse("amount"),
        holder_field: FieldPath::parse("owner"),
        summary: "The sheet records fees.".to_string(),
    }];
    economics.amounts = vec![
        StandardAmount {
            action: ActionId::new("record_parcel"),
            amount: Decimal::new(100, 2),
            summary: "The filing fee.".to_string(),
        },
        StandardAmount {
            action: ActionId::new("charge_fee"),
            amount: Decimal::new(50, 2),
            summary: "The charge.".to_string(),
        },
    ];
    economics.limits = vec![
        AmountLimit {
            role: RoleId::new("registrar"),
            maximum: Decimal::new(100_000, 2),
            summary: "The daily ceiling.".to_string(),
        },
        AmountLimit {
            role: RoleId::new("clerk"),
            maximum: Decimal::new(10_000, 2),
            summary: "The clerk ceiling.".to_string(),
        },
    ];
    economics
}

/// The complete land registry Blueprint.
pub fn land_registry() -> Blueprint {
    let mut blueprint = Blueprint::new(BlueprintMeta::new(
        "land registry",
        "The registry records land parcels and their fees.",
        "Kollam District Administration",
    ));
    blueprint.types = types();
    blueprint.collections = collections();
    blueprint.identities = identities();
    blueprint.permissions = permissions();
    blueprint.rules = rules();
    blueprint.actions = actions();
    blueprint.workflows = workflows();
    blueprint.events = events();
    blueprint.views = views();
    blueprint.integrations = integrations();
    blueprint.governance = governance();
    blueprint.economics = Some(economics());
    blueprint.blocks = blocks();
    blueprint
}
