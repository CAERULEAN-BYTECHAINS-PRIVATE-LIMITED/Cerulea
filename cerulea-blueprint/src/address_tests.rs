//! Behavioural tests for content addressing over a complete Blueprint.
//!
//! The land registry fixture fills every section. The tests reverse every
//! list the canonical form sorts and assert the identifier holds still,
//! then change one value and assert the identifier moves.

use crate::{
    actions::Effect,
    address::{canonical_bytes, canonical_form, content_address, BlueprintId},
    expression::Expr,
    test_fixture::land_registry,
    types::{FieldName, TypeId, Value},
    Blueprint,
};
use alloc::{format, string::ToString, vec};

fn reverse_everything(blueprint: &mut Blueprint) {
    blueprint.types.reverse();
    blueprint.collections.reverse();
    for collection in blueprint.collections.iter_mut() {
        collection.indexes.reverse();
    }
    let identities = &mut blueprint.identities;
    identities.units.reverse();
    identities.roles.reverse();
    for role in identities.roles.iter_mut() {
        role.requires.reverse();
    }
    identities.credentials.reverse();
    identities.delegations.reverse();
    for rule in identities.delegations.iter_mut() {
        rule.scope.reverse();
    }
    blueprint.permissions.entries.reverse();
    for entry in blueprint.permissions.entries.iter_mut() {
        entry.purposes.reverse();
    }
    blueprint.rules.invariants.reverse();
    blueprint.rules.preconditions.reverse();
    blueprint.actions.reverse();
    for action in blueprint.actions.iter_mut() {
        action.emits.reverse();
        action.roles.reverse();
        action.preconditions.reverse();
        for effect in action.effects.iter_mut() {
            if let Effect::RunBlock { arguments, .. } = effect {
                arguments.reverse();
            }
        }
    }
    blueprint.workflows.reverse();
    for workflow in blueprint.workflows.iter_mut() {
        workflow.states.reverse();
        workflow.transitions.reverse();
        for transition in workflow.transitions.iter_mut() {
            transition.roles.reverse();
        }
        workflow.timers.reverse();
        workflow.deadlines.reverse();
    }
    blueprint.events.reverse();
    for event in blueprint.events.iter_mut() {
        event.subscribers.reverse();
    }
    blueprint.views.reverse();
    for view in blueprint.views.iter_mut() {
        view.purposes.reverse();
        view.roles.reverse();
    }
    let integrations = &mut blueprint.integrations;
    integrations.feeds.reverse();
    integrations.bridges.reverse();
    integrations.triggers.reverse();
    for feed in integrations.feeds.iter_mut() {
        feed.attestation.roles.reverse();
    }
    for bridge in integrations.bridges.iter_mut() {
        bridge.attestation.roles.reverse();
    }
    for trigger in integrations.triggers.iter_mut() {
        trigger.attestation.roles.reverse();
    }
    blueprint.governance.authority.roles.reverse();
    blueprint.governance.quorum.reverse();
    for stage in blueprint.governance.process.stages.iter_mut() {
        stage.roles.reverse();
    }
    blueprint.blocks.reverse();
    if let Some(economics) = blueprint.economics.as_mut() {
        economics.sheets.reverse();
        economics.amounts.reverse();
        economics.limits.reverse();
    }
}

#[test]
fn the_declaration_order_of_every_section_leaves_the_identifier_alone() {
    let original = land_registry();
    let mut shuffled = land_registry();
    reverse_everything(&mut shuffled);
    assert_ne!(original, shuffled);
    assert_eq!(original.id(), shuffled.id());
    assert_eq!(original.canonical_bytes(), shuffled.canonical_bytes());
    assert_eq!(canonical_form(&original), canonical_form(&shuffled));
    assert_eq!(shuffled.canonical(), original.canonical());
    let canonical = original.canonical();
    let mut again = canonical.clone();
    crate::address::canonicalise(&mut again);
    assert_eq!(again, canonical);
}

#[test]
fn a_literal_inside_any_expression_is_sorted_before_hashing() {
    let unsorted = Expr::Literal(Value::Record(crate::types::RecordValue {
        type_id: TypeId::new("ledger"),
        fields: vec![
            (FieldName::new("owner"), Value::Integer(1)),
            (FieldName::new("amount"), Value::Integer(2)),
        ],
    }));
    let sorted = Expr::Literal(Value::record(
        TypeId::new("ledger"),
        vec![
            (FieldName::new("owner"), Value::Integer(1)),
            (FieldName::new("amount"), Value::Integer(2)),
        ],
    ));
    assert_ne!(unsorted, sorted);
    let place = |blueprint: &mut Blueprint, literal: &Expr| {
        blueprint.permissions.entries[0].rows =
            crate::permissions::RowQualification::Where(literal.clone());
        blueprint.rules.invariants[0].holds = literal.clone();
        blueprint.rules.preconditions[0].holds = literal.clone();
        blueprint.workflows[0].transitions[0].guard = Some(literal.clone());
        blueprint.workflows[0].deadlines[0].due = literal.clone();
        blueprint.views[0].projection[0].value = literal.clone();
        blueprint.views[0].filter = Some(literal.clone());
        blueprint.views[0].aggregates[0].value = Some(literal.clone());
        blueprint.blocks[0].outputs[0].value = literal.clone();
        for effect in blueprint.actions[0].effects.iter_mut() {
            match effect {
                Effect::Insert { record, .. } => *record = literal.clone(),
                Effect::Update { value, .. } => *value = literal.clone(),
                Effect::Emit { payload, .. } => *payload = literal.clone(),
                Effect::RunBlock { arguments, .. } => arguments[0].1 = literal.clone(),
                _ => {}
            }
        }
    };
    let mut first = land_registry();
    place(&mut first, &unsorted);
    let mut second = land_registry();
    place(&mut second, &sorted);
    assert_ne!(first, second);
    assert_eq!(first.id(), second.id());
}

#[test]
fn one_changed_value_in_any_section_moves_the_identifier() {
    let base = land_registry().id();
    let mut changed = land_registry();
    changed.blocks[0].summary.push('.');
    assert_ne!(changed.id(), base);
    let mut economics = land_registry();
    if let Some(section) = economics.economics.as_mut() {
        section.limits[0].maximum.units += 1;
    }
    assert_ne!(economics.id(), base);
    let mut governance = land_registry();
    governance.governance.authority.threshold_percent += 1;
    assert_ne!(governance.id(), base);
}

#[test]
fn the_identifier_reads_as_bytes_hexadecimal_and_a_short_heading() {
    let blueprint = land_registry();
    let id = content_address(&blueprint);
    assert_eq!(id, blueprint.id());
    let bytes = canonical_bytes(&blueprint);
    assert_eq!(id, BlueprintId(sp_crypto_hashing::blake2_256(&bytes)));
    assert_eq!(id.as_bytes().len(), 32);
    let hex = id.to_hex();
    assert_eq!(hex.len(), 64);
    assert!(hex.chars().all(|c| c.is_ascii_hexdigit()));
    assert_eq!(id.short(), hex[..8].to_string());
    assert_eq!(format!("{id}"), hex);
    assert_eq!(BlueprintId([0u8; 32]).short(), "00000000");
}
