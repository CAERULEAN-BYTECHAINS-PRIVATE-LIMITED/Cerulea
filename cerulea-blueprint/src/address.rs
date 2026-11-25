//! Content addressing and version stamping.
//!
//! The identifier of a Blueprint is the hash of its canonical encoding. Any
//! change produces a new identifier, so tampering is detectable.
//!
//! The canonical encoding is the SCALE encoding of the canonical form. The
//! canonical form sorts every set that carries no order of its own and
//! keeps every sequence that does. An effect sequence stays in order. A
//! list of collections sorts by name.
//!
//! The hash is blake2 with a 32 byte result, taken over the domain
//! separator, the address version and the canonical bytes.

use crate::{
    expression::Expr, Blueprint, SchemaVersion, VersionStamp, BLUEPRINT_DOMAIN,
    BLUEPRINT_SCHEMA_VERSION,
};
use alloc::{string::String, vec::Vec};
use codec::{Decode, Encode};
use scale_info::TypeInfo;
use serde::{Deserialize, Serialize};

/// The version of the addressing scheme.
///
/// A change to the canonical form raises this number.
pub const ADDRESS_VERSION: u8 = 1;

/// The identifier of a Blueprint.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Encode,
    Decode,
    TypeInfo,
    Serialize,
    Deserialize,
)]
pub struct BlueprintId(pub [u8; 32]);

impl BlueprintId {
    /// Read the identifier as bytes.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Write the identifier as lower case hexadecimal text.
    pub fn to_hex(&self) -> String {
        crate::types::Hash32(self.0).to_hex()
    }

    /// The first eight hexadecimal digits, for a heading in the interface
    /// layer.
    pub fn short(&self) -> String {
        self.to_hex().chars().take(8).collect()
    }
}

impl core::fmt::Display for BlueprintId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.to_hex())
    }
}

/// The canonical byte string of a Blueprint.
///
/// The string holds the domain separator, the address version and the SCALE
/// encoding of the canonical form.
pub fn canonical_bytes(blueprint: &Blueprint) -> Vec<u8> {
    let canonical = canonical_form(blueprint);
    let mut bytes = Vec::with_capacity(BLUEPRINT_DOMAIN.len() + 1);
    bytes.extend_from_slice(BLUEPRINT_DOMAIN);
    bytes.push(ADDRESS_VERSION);
    canonical.encode_to(&mut bytes);
    bytes
}

/// The identifier of a Blueprint.
pub fn content_address(blueprint: &Blueprint) -> BlueprintId {
    BlueprintId(sp_crypto_hashing::blake2_256(&canonical_bytes(blueprint)))
}

/// The canonical form of a Blueprint.
///
/// Two Blueprints carrying the same content produce the same canonical form
/// whatever order the author entered them in.
pub fn canonical_form(blueprint: &Blueprint) -> Blueprint {
    let mut copy = blueprint.clone();
    canonicalise(&mut copy);
    copy
}

/// Sort a Blueprint into canonical order in place.
pub fn canonicalise(blueprint: &mut Blueprint) {
    blueprint
        .types
        .sort_by(|left, right| left.id().cmp(right.id()));

    blueprint
        .collections
        .sort_by(|left, right| left.id.cmp(&right.id));
    for collection in blueprint.collections.iter_mut() {
        collection
            .indexes
            .sort_by(|left, right| left.name.cmp(&right.name));
    }

    blueprint
        .identities
        .units
        .sort_by(|left, right| left.id.cmp(&right.id));
    blueprint
        .identities
        .roles
        .sort_by(|left, right| left.id.cmp(&right.id));
    for role in blueprint.identities.roles.iter_mut() {
        role.requires.sort();
    }
    blueprint
        .identities
        .credentials
        .sort_by(|left, right| left.id.cmp(&right.id));
    blueprint
        .identities
        .delegations
        .sort_by(|left, right| left.id.cmp(&right.id));
    for rule in blueprint.identities.delegations.iter_mut() {
        rule.scope.sort();
    }

    for entry in blueprint.permissions.entries.iter_mut() {
        entry.purposes.sort();
        if let Some(predicate) = row_predicate_mut(entry) {
            predicate.canonicalise();
        }
    }
    blueprint.permissions.entries.sort();

    blueprint
        .rules
        .invariants
        .sort_by(|left, right| left.id.cmp(&right.id));
    for rule in blueprint.rules.invariants.iter_mut() {
        rule.holds.canonicalise();
    }
    blueprint
        .rules
        .preconditions
        .sort_by(|left, right| left.id.cmp(&right.id));
    for rule in blueprint.rules.preconditions.iter_mut() {
        rule.holds.canonicalise();
    }

    blueprint
        .actions
        .sort_by(|left, right| left.id.cmp(&right.id));
    for action in blueprint.actions.iter_mut() {
        action.emits.sort();
        action.roles.sort();
        action.preconditions.sort();
        for effect in action
            .effects
            .iter_mut()
            .chain(action.compensation.iter_mut())
        {
            canonicalise_effect(effect);
        }
    }

    blueprint
        .workflows
        .sort_by(|left, right| left.id.cmp(&right.id));
    for workflow in blueprint.workflows.iter_mut() {
        workflow
            .states
            .sort_by(|left, right| left.id.cmp(&right.id));
        workflow
            .transitions
            .sort_by(|left, right| left.id.cmp(&right.id));
        for transition in workflow.transitions.iter_mut() {
            transition.roles.sort();
            if let Some(guard) = transition.guard.as_mut() {
                guard.canonicalise();
            }
        }
        workflow
            .timers
            .sort_by(|left, right| left.id.cmp(&right.id));
        workflow
            .deadlines
            .sort_by(|left, right| left.id.cmp(&right.id));
        for deadline in workflow.deadlines.iter_mut() {
            deadline.due.canonicalise();
        }
    }

    blueprint
        .events
        .sort_by(|left, right| left.id.cmp(&right.id));
    for event in blueprint.events.iter_mut() {
        event.subscribers.sort();
    }

    blueprint
        .views
        .sort_by(|left, right| left.id.cmp(&right.id));
    for view in blueprint.views.iter_mut() {
        view.purposes.sort();
        view.roles.sort();
        for field in view.projection.iter_mut() {
            field.value.canonicalise();
        }
        if let Some(filter) = view.filter.as_mut() {
            filter.canonicalise();
        }
        for aggregate in view.aggregates.iter_mut() {
            if let Some(value) = aggregate.value.as_mut() {
                value.canonicalise();
            }
        }
    }

    blueprint
        .integrations
        .feeds
        .sort_by(|left, right| left.id.cmp(&right.id));
    blueprint
        .integrations
        .bridges
        .sort_by(|left, right| left.id.cmp(&right.id));
    blueprint
        .integrations
        .triggers
        .sort_by(|left, right| left.id.cmp(&right.id));
    for feed in blueprint.integrations.feeds.iter_mut() {
        feed.attestation.roles.sort();
        feed.attestation.credentials.sort();
    }
    for bridge in blueprint.integrations.bridges.iter_mut() {
        bridge.attestation.roles.sort();
        bridge.attestation.credentials.sort();
    }
    for trigger in blueprint.integrations.triggers.iter_mut() {
        trigger.attestation.roles.sort();
        trigger.attestation.credentials.sort();
    }

    blueprint.governance.authority.roles.sort();
    blueprint
        .governance
        .quorum
        .sort_by(|left, right| left.class.cmp(&right.class));
    for stage in blueprint.governance.process.stages.iter_mut() {
        stage.roles.sort();
    }

    blueprint
        .blocks
        .sort_by(|left, right| left.id.cmp(&right.id));
    for block in blueprint.blocks.iter_mut() {
        for effect in block.effects.iter_mut() {
            canonicalise_effect(effect);
        }
        for output in block.outputs.iter_mut() {
            output.value.canonicalise();
        }
    }

    if let Some(economics) = blueprint.economics.as_mut() {
        economics
            .sheets
            .sort_by(|left, right| left.collection.cmp(&right.collection));
        economics
            .amounts
            .sort_by(|left, right| left.action.cmp(&right.action));
        economics
            .limits
            .sort_by(|left, right| left.role.cmp(&right.role));
    }
}

fn row_predicate_mut(entry: &mut crate::permissions::CapabilityEntry) -> Option<&mut Expr> {
    match &mut entry.rows {
        crate::permissions::RowQualification::EveryRow => None,
        crate::permissions::RowQualification::Where(expr) => Some(expr),
    }
}

fn canonicalise_effect(effect: &mut crate::actions::Effect) {
    use crate::{actions::Effect, collections::KeySelector};
    let canonicalise_key = |key: &mut KeySelector| match key {
        KeySelector::Exact(expr) | KeySelector::Prefix(expr) => expr.canonicalise(),
        KeySelector::Full => {}
    };
    match effect {
        Effect::Insert { key, record, .. } => {
            canonicalise_key(key);
            record.canonicalise();
        }
        Effect::Update { key, value, .. } => {
            canonicalise_key(key);
            value.canonicalise();
        }
        Effect::Delete { key, .. } => canonicalise_key(key),
        Effect::Read { key, .. } => canonicalise_key(key),
        Effect::Transition { key, .. } => canonicalise_key(key),
        Effect::Emit { payload, .. } => payload.canonicalise(),
        Effect::RunBlock { arguments, .. } => {
            for (_, value) in arguments.iter_mut() {
                value.canonicalise();
            }
            arguments.sort_by(|left, right| left.0.cmp(&right.0));
        }
    }
}

impl Blueprint {
    /// The canonical form of this Blueprint.
    pub fn canonical(&self) -> Blueprint {
        canonical_form(self)
    }

    /// The canonical byte string of this Blueprint.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        canonical_bytes(self)
    }

    /// The identifier of this Blueprint.
    pub fn id(&self) -> BlueprintId {
        content_address(self)
    }

    /// The version stamp this Blueprint carries.
    pub fn version(&self) -> VersionStamp {
        self.meta.version
    }

    /// Raise the revision by one and stamp the current schema version.
    ///
    /// The Studio calls this on every publication.
    pub fn stamp_next_revision(&mut self) {
        self.meta.version.schema = BLUEPRINT_SCHEMA_VERSION;
        self.meta.version.revision = self.meta.version.revision.saturating_add(1);
    }

    /// Report whether this crate reads the artefact.
    pub fn readable_here(&self) -> bool {
        self.meta
            .version
            .schema
            .readable_by(&BLUEPRINT_SCHEMA_VERSION)
    }

    /// The schema version the artefact was written for.
    pub fn schema_version(&self) -> SchemaVersion {
        self.meta.version.schema
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        collections::{Collection, PurposeDeclaration},
        types::{FieldDef, RecordDef, TypeDef, TypeExpr, TypeId},
        BlueprintMeta,
    };
    use alloc::{string::ToString, vec};

    fn parcel() -> TypeDef {
        TypeDef::Record(RecordDef {
            id: TypeId::new("parcel"),
            fields: vec![
                FieldDef::new("survey_number", TypeExpr::Text, "The survey number."),
                FieldDef::new("holder", TypeExpr::Identity, "The registered holder."),
            ],
            summary: "One land parcel.".to_string(),
        })
    }

    fn transfer() -> TypeDef {
        TypeDef::Record(RecordDef {
            id: TypeId::new("transfer"),
            fields: vec![FieldDef::new("reference", TypeExpr::Text, "The reference.")],
            summary: "One transfer.".to_string(),
        })
    }

    fn collection(id: &str, record: &str, key: &str) -> Collection {
        let mut held = Collection::new(id, record, key, "The register.");
        held.purpose = PurposeDeclaration::new("The register records land.", &["registration"]);
        held
    }

    fn sample() -> Blueprint {
        let mut blueprint = Blueprint::new(BlueprintMeta::new(
            "district land registry",
            "The registry records land parcels and their holders.",
            "Kollam District Administration",
        ));
        blueprint.types = vec![parcel(), transfer()];
        blueprint.collections = vec![
            collection("parcel", "parcel", "survey_number"),
            collection("transfer", "transfer", "reference"),
        ];
        blueprint
    }

    #[test]
    fn two_identical_blueprints_produce_the_same_identifier() {
        let first = sample();
        let second = sample();
        assert_eq!(first.id(), second.id());
        assert_eq!(first.canonical_bytes(), second.canonical_bytes());
    }

    #[test]
    fn a_one_byte_change_produces_a_different_identifier() {
        let first = sample();
        let mut second = sample();
        second.meta.summary.push('.');
        assert_ne!(first.id(), second.id());

        let mut third = sample();
        third.meta.version.revision += 1;
        assert_ne!(first.id(), third.id());

        let mut fourth = sample();
        fourth.collections[0].primary_key = crate::types::FieldPath::parse("holder");
        assert_ne!(first.id(), fourth.id());
    }

    #[test]
    fn the_declaration_order_leaves_the_identifier_alone() {
        let first = sample();
        let mut second = sample();
        second.types.reverse();
        second.collections.reverse();
        assert_eq!(first.id(), second.id());
    }

    #[test]
    fn the_identifier_survives_an_encoding_round_trip() {
        let blueprint = sample();
        let bytes = blueprint.encode();
        let back = Blueprint::decode(&mut &bytes[..]).expect("the encoding decodes");
        assert_eq!(blueprint.id(), back.id());
    }

    #[test]
    fn a_literal_out_of_canonical_order_leaves_the_identifier_alone() {
        use crate::{
            actions::{Action, Effect, Param},
            collections::KeySelector,
            types::{FieldName, RecordValue, RoleId, Value},
        };
        let build = |fields: Vec<(FieldName, Value)>| {
            let mut action = Action::new("record_parcel", "The action records one parcel.");
            action.roles = vec![RoleId::new("registrar")];
            action.params = vec![Param::new("key", TypeExpr::Text, "The key.")];
            action.effects = vec![Effect::Insert {
                collection: crate::types::CollectionId::new("parcel"),
                key: KeySelector::Exact(Expr::bound("key")),
                record: Expr::Literal(Value::Record(RecordValue {
                    type_id: TypeId::new("parcel"),
                    fields,
                })),
            }];
            let mut blueprint = sample();
            blueprint.actions = vec![action];
            blueprint
        };
        let ordered = build(vec![
            (
                FieldName::new("holder"),
                Value::Identity(crate::types::Identity([1u8; 32])),
            ),
            (
                FieldName::new("survey_number"),
                Value::Text("KL-1".to_string()),
            ),
        ]);
        let shuffled = build(vec![
            (
                FieldName::new("survey_number"),
                Value::Text("KL-1".to_string()),
            ),
            (
                FieldName::new("holder"),
                Value::Identity(crate::types::Identity([1u8; 32])),
            ),
        ]);
        assert_ne!(ordered.actions, shuffled.actions);
        assert_eq!(ordered.id(), shuffled.id());
    }

    #[test]
    fn the_identifier_prints_as_hexadecimal() {
        let blueprint = sample();
        let text = blueprint.id().to_hex();
        assert_eq!(text.len(), 64);
        assert_eq!(blueprint.id().short().len(), 8);
        assert_eq!(alloc::format!("{}", blueprint.id()), text);
    }

    #[test]
    fn the_domain_separator_sits_at_the_front_of_the_canonical_bytes() {
        let blueprint = sample();
        let bytes = blueprint.canonical_bytes();
        assert!(bytes.starts_with(BLUEPRINT_DOMAIN));
        assert_eq!(bytes[BLUEPRINT_DOMAIN.len()], ADDRESS_VERSION);
    }

    #[test]
    fn the_stamp_raises_the_revision() {
        let mut blueprint = sample();
        assert_eq!(blueprint.version().revision, 1);
        blueprint.stamp_next_revision();
        assert_eq!(blueprint.version().revision, 2);
        assert_eq!(blueprint.schema_version(), BLUEPRINT_SCHEMA_VERSION);
        assert!(blueprint.readable_here());
    }

    #[test]
    fn an_artefact_from_a_later_major_version_stays_outside_this_reader() {
        let mut blueprint = sample();
        blueprint.meta.version.schema = SchemaVersion {
            major: BLUEPRINT_SCHEMA_VERSION.major + 1,
            minor: 0,
            patch: 0,
        };
        assert!(!blueprint.readable_here());
    }

    #[test]
    fn the_canonical_form_leaves_the_original_alone() {
        let blueprint = sample();
        let before = blueprint.types.clone();
        let canonical = blueprint.canonical();
        assert_eq!(blueprint.types, before);
        assert_eq!(canonical.types.len(), 2);
    }
}
