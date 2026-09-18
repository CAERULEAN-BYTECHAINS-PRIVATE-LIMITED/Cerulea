//! Feeds, bridge endpoints and external triggers.
//!
//! A feed carries values from outside the deployment into a collection. A
//! bridge endpoint moves records between this deployment and a named
//! network. An external trigger lets an outside system run one action.
//!
//! Every integration declares the attestation it requires. The attestation
//! names the count of signers and the roles that sign.

use crate::{
    actions::ActionRegistry,
    collections::CollectionRegistry,
    identities::Identities,
    types::{ActionId, CollectionId, CredentialId, IntegrationId, RoleId, TypeId, TypeRegistry},
    Diagnostic, DiagnosticCode, Location, Path, Report,
};
use alloc::{
    collections::BTreeSet,
    format,
    string::{String, ToString},
    vec::Vec,
};
use codec::{Decode, Encode};
use scale_info::TypeInfo;
use serde::{Deserialize, Serialize};

/// The proof an integration presents before the Blockchain accepts its work.
#[derive(
    Clone,
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
pub struct AttestationRequirement {
    /// The count of signatures the Blockchain requires.
    pub signers: u16,
    /// The roles that sign.
    pub roles: Vec<RoleId>,
    /// The credentials a signer holds.
    pub credentials: Vec<CredentialId>,
}

impl AttestationRequirement {
    /// Require a count of signatures from the listed roles.
    pub fn signed_by(signers: u16, roles: &[&str]) -> Self {
        AttestationRequirement {
            signers,
            roles: roles.iter().map(|role| RoleId::new(role)).collect(),
            credentials: Vec::new(),
        }
    }

    /// Check the requirement.
    pub fn validate(&self, identities: &Identities, path: &Path) -> Report {
        let mut report = Report::new();
        if self.signers == 0 {
            report.push(Diagnostic::at(
                DiagnosticCode::ValueOutOfRange,
                path.field("signers").location(),
                "The attestation requires zero signatures.".to_string(),
                "Require at least one signature.".to_string(),
            ));
        }
        if self.roles.is_empty() {
            report.push(Diagnostic::at(
                DiagnosticCode::SectionEmpty,
                path.field("roles").location(),
                "The attestation names zero roles.".to_string(),
                "Name at least one role that signs.".to_string(),
            ));
        }
        if self.signers as usize > self.roles.len() && !self.roles.is_empty() {
            report.push(Diagnostic::at(
                DiagnosticCode::ValueOutOfRange,
                path.field("signers").location(),
                format!(
                    "The attestation requires {} signatures from {} roles.",
                    self.signers,
                    self.roles.len()
                ),
                format!(
                    "Name more roles, or lower the count to {}.",
                    self.roles.len()
                ),
            ));
        }
        for (position, role) in self.roles.iter().enumerate() {
            let here = path.field("roles").index(position);
            report.absorb(role.validate(&here));
            if !identities.has_role(role) {
                report.push(identities.missing_role(role, here.location()));
            }
        }
        for (position, credential) in self.credentials.iter().enumerate() {
            let here = path.field("credentials").index(position);
            report.absorb(credential.validate(&here));
            if !identities
                .credentials
                .iter()
                .any(|held| &held.id == credential)
            {
                report.push(Diagnostic::at(
                    DiagnosticCode::DanglingReference,
                    here.location(),
                    format!(
                        "The attestation names the credential `{credential}`, which the Blueprint lacks."
                    ),
                    format!(
                        "Declare a credential named `{credential}` in the identities section."
                    ),
                ));
            }
        }
        report
    }
}

/// A feed carrying values into a collection.
#[derive(
    Clone,
    Debug,
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
pub struct Feed {
    /// The name of the feed.
    pub id: IntegrationId,
    /// The collection the feed writes.
    pub target: CollectionId,
    /// The record type the feed carries.
    pub record_type: TypeId,
    /// The span in milliseconds between two reads.
    pub refresh_millis: u64,
    /// The attestation the feed presents.
    pub attestation: AttestationRequirement,
    /// One plain sentence stating what the feed carries.
    pub summary: String,
}

/// The direction records move across a bridge endpoint.
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
pub enum BridgeDirection {
    /// Records arrive from the named network.
    #[default]
    Inbound,
    /// Records leave for the named network.
    Outbound,
    /// Records move both ways.
    BothWays,
}

/// An endpoint moving records between this deployment and a named network.
#[derive(
    Clone,
    Debug,
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
pub struct BridgeEndpoint {
    /// The name of the endpoint.
    pub id: IntegrationId,
    /// The name the operator gave the network on the other side.
    pub network: String,
    /// The direction records move.
    pub direction: BridgeDirection,
    /// The collection the endpoint reads and writes.
    pub collection: CollectionId,
    /// The attestation the endpoint presents.
    pub attestation: AttestationRequirement,
    /// One plain sentence stating what the endpoint moves.
    pub summary: String,
}

/// A trigger letting an outside system run one action.
#[derive(
    Clone,
    Debug,
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
pub struct ExternalTrigger {
    /// The name of the trigger.
    pub id: IntegrationId,
    /// The action the trigger runs.
    pub action: ActionId,
    /// The attestation the trigger presents.
    pub attestation: AttestationRequirement,
    /// One plain sentence stating what the trigger starts.
    pub summary: String,
}

/// The integrations section.
#[derive(
    Clone,
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
pub struct Integrations {
    /// The feeds.
    pub feeds: Vec<Feed>,
    /// The bridge endpoints.
    pub bridges: Vec<BridgeEndpoint>,
    /// The external triggers.
    pub triggers: Vec<ExternalTrigger>,
}

impl Integrations {
    /// Report whether an integration exists under the given name.
    pub fn contains(&self, id: &IntegrationId) -> bool {
        self.feeds.iter().any(|feed| &feed.id == id)
            || self.bridges.iter().any(|bridge| &bridge.id == id)
            || self.triggers.iter().any(|trigger| &trigger.id == id)
    }

    /// The integration names, in declaration order.
    pub fn names(&self) -> Vec<&str> {
        self.feeds
            .iter()
            .map(|feed| feed.id.as_str())
            .chain(self.bridges.iter().map(|bridge| bridge.id.as_str()))
            .chain(self.triggers.iter().map(|trigger| trigger.id.as_str()))
            .collect()
    }

    /// Raise a dangling reference diagnostic for an integration the
    /// Blueprint lacks.
    pub fn missing(&self, id: &IntegrationId, location: Location) -> Diagnostic {
        let listed = self.names().join(", ");
        Diagnostic::at(
            DiagnosticCode::DanglingReference,
            location,
            format!("The entry names the integration `{id}`, which the Blueprint lacks."),
            if listed.is_empty() {
                format!("Declare an integration named `{id}` in the integrations section.")
            } else {
                format!("Point the entry at one of these integrations: {listed}.")
            },
        )
    }

    /// Check the whole integrations section.
    pub fn validate(
        &self,
        types: &TypeRegistry,
        collections: &CollectionRegistry,
        identities: &Identities,
        actions: &ActionRegistry,
        path: &Path,
    ) -> Report {
        let mut report = Report::new();
        let mut seen: BTreeSet<&str> = BTreeSet::new();

        for (position, feed) in self.feeds.iter().enumerate() {
            let here = path.field("feeds").index(position);
            report.absorb(feed.id.validate(&here.field("id")));
            if !seen.insert(feed.id.as_str()) {
                report.push(repeated(&feed.id, here.field("id").location()));
            }
            report.merge(sentence(&feed.summary, &feed.id, &here));
            match collections.get(&feed.target) {
                Some(collection) => {
                    if collection.record_type != feed.record_type {
                        report.push(Diagnostic::at(
                            DiagnosticCode::TypeMismatch,
                            here.field("record_type").location(),
                            format!(
                                "The feed `{}` carries `{}` and `{}` holds `{}`.",
                                feed.id, feed.record_type, feed.target, collection.record_type
                            ),
                            format!(
                                "Carry the record type `{}` on the feed.",
                                collection.record_type
                            ),
                        ));
                    }
                }
                None => {
                    report.push(collections.missing(&feed.target, here.field("target").location()))
                }
            }
            if types.record(&feed.record_type).is_none() {
                report.push(Diagnostic::at(
                    DiagnosticCode::DanglingReference,
                    here.field("record_type").location(),
                    format!(
                        "The feed `{}` names the record type `{}`, which the Blueprint lacks.",
                        feed.id, feed.record_type
                    ),
                    format!(
                        "Declare a record type named `{}` in the types section.",
                        feed.record_type
                    ),
                ));
            }
            if feed.refresh_millis == 0 {
                report.push(Diagnostic::at(
                    DiagnosticCode::ValueOutOfRange,
                    here.field("refresh_millis").location(),
                    format!("The feed `{}` reads every zero milliseconds.", feed.id),
                    "Set a span above zero.".to_string(),
                ));
            }
            report.merge(
                feed.attestation
                    .validate(identities, &here.field("attestation")),
            );
        }

        for (position, bridge) in self.bridges.iter().enumerate() {
            let here = path.field("bridges").index(position);
            report.absorb(bridge.id.validate(&here.field("id")));
            if !seen.insert(bridge.id.as_str()) {
                report.push(repeated(&bridge.id, here.field("id").location()));
            }
            report.merge(sentence(&bridge.summary, &bridge.id, &here));
            if bridge.network.is_empty() {
                report.push(Diagnostic::at(
                    DiagnosticCode::MissingValue,
                    here.field("network").location(),
                    format!("The endpoint `{}` names no network.", bridge.id),
                    "Name the network on the other side of the endpoint.".to_string(),
                ));
            }
            if !collections.contains(&bridge.collection) {
                report.push(
                    collections.missing(&bridge.collection, here.field("collection").location()),
                );
            }
            report.merge(
                bridge
                    .attestation
                    .validate(identities, &here.field("attestation")),
            );
        }

        for (position, trigger) in self.triggers.iter().enumerate() {
            let here = path.field("triggers").index(position);
            report.absorb(trigger.id.validate(&here.field("id")));
            if !seen.insert(trigger.id.as_str()) {
                report.push(repeated(&trigger.id, here.field("id").location()));
            }
            report.merge(sentence(&trigger.summary, &trigger.id, &here));
            if !actions.contains(&trigger.action) {
                report.push(actions.missing(&trigger.action, here.field("action").location()));
            }
            report.merge(
                trigger
                    .attestation
                    .validate(identities, &here.field("attestation")),
            );
        }

        report
    }
}

fn repeated(id: &IntegrationId, location: Location) -> Diagnostic {
    Diagnostic::at(
        DiagnosticCode::DuplicateIdentifier,
        location,
        format!("Two integrations share the name `{id}`."),
        format!("Rename one of the two integrations away from `{id}`."),
    )
}

fn sentence(summary: &str, id: &IntegrationId, here: &Path) -> Report {
    let mut report = Report::new();
    if summary.is_empty() {
        report.push(Diagnostic::at(
            DiagnosticCode::MissingValue,
            here.field("summary").location(),
            format!("The integration `{id}` states nothing."),
            "Write one plain sentence stating what the integration carries.".to_string(),
        ));
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        actions::{Action, ActionRegistry},
        collections::{Collection, PurposeDeclaration},
        identities::{CredentialDef, CredentialIssuer, CredentialValidity, RoleDef},
        types::{FieldDef, RecordDef, TypeDef, TypeExpr},
    };
    use alloc::vec;

    fn types() -> Vec<TypeDef> {
        vec![TypeDef::Record(RecordDef {
            id: TypeId::new("rate"),
            fields: vec![
                FieldDef::new("pair", TypeExpr::Text, "The pair quoted."),
                FieldDef::new("value", TypeExpr::Decimal { scale: 6 }, "The quoted value."),
            ],
            summary: "One quoted rate.".to_string(),
        })]
    }

    fn collections() -> Vec<Collection> {
        let mut rate = Collection::new("rate", "rate", "pair", "The quoted rates.");
        rate.purpose = PurposeDeclaration::new("The record carries quoted rates.", &["pricing"]);
        vec![rate]
    }

    fn identities() -> Identities {
        Identities {
            units: Vec::new(),
            roles: vec![
                RoleDef::new(
                    "feed_signer",
                    None,
                    "Feed signer",
                    "The signer attests a feed.",
                ),
                RoleDef::new(
                    "bridge_signer",
                    None,
                    "Bridge signer",
                    "The signer attests a move.",
                ),
            ],
            credentials: vec![CredentialDef {
                id: CredentialId::new("signing_key"),
                issuer: CredentialIssuer::External {
                    authority: "Deployment operator".to_string(),
                },
                validity: CredentialValidity::UntilRevoked,
                attests: "The holder signs feed readings.".to_string(),
            }],
            delegations: Vec::new(),
        }
    }

    fn actions() -> Vec<Action> {
        let mut action = Action::new("refresh_rates", "The action refreshes the rates.");
        action.roles = vec![RoleId::new("feed_signer")];
        vec![action]
    }

    fn sample() -> Integrations {
        Integrations {
            feeds: vec![Feed {
                id: IntegrationId::new("rate_feed"),
                target: CollectionId::new("rate"),
                record_type: TypeId::new("rate"),
                refresh_millis: 60_000,
                attestation: AttestationRequirement {
                    signers: 2,
                    roles: vec![RoleId::new("feed_signer"), RoleId::new("bridge_signer")],
                    credentials: vec![CredentialId::new("signing_key")],
                },
                summary: "The feed carries quoted rates into the register.".to_string(),
            }],
            bridges: vec![BridgeEndpoint {
                id: IntegrationId::new("partner_endpoint"),
                network: "Partner private chain".to_string(),
                direction: BridgeDirection::BothWays,
                collection: CollectionId::new("rate"),
                attestation: AttestationRequirement::signed_by(1, &["bridge_signer"]),
                summary: "The endpoint moves rates to the partner network.".to_string(),
            }],
            triggers: vec![ExternalTrigger {
                id: IntegrationId::new("refresh_trigger"),
                action: ActionId::new("refresh_rates"),
                attestation: AttestationRequirement::signed_by(1, &["feed_signer"]),
                summary: "The trigger refreshes the rates on request.".to_string(),
            }],
        }
    }

    fn run(integrations: &Integrations) -> Report {
        let defs = types();
        let (type_registry, _) = TypeRegistry::build(&defs, &Path::root().field("types"));
        let held = collections();
        let (collection_registry, _) =
            CollectionRegistry::build(&held, &Path::root().field("collections"));
        let declared = actions();
        let action_registry = ActionRegistry::build(&declared);
        integrations.validate(
            &type_registry,
            &collection_registry,
            &identities(),
            &action_registry,
            &Path::root().field("integrations"),
        )
    }

    #[test]
    fn a_complete_integrations_section_validates() {
        let report = run(&sample());
        assert!(report.is_clean(), "{report}");
    }

    #[test]
    fn a_feed_naming_an_absent_collection_reports_its_location() {
        let mut integrations = sample();
        integrations.feeds[0].target = CollectionId::new("absent_collection");
        let report = run(&integrations);
        let found = report
            .find("integrations.feeds[0].target")
            .expect("the diagnostic sits on the target");
        assert_eq!(found.code, DiagnosticCode::DanglingReference);
    }

    #[test]
    fn a_feed_carrying_the_wrong_record_type_is_caught() {
        let mut integrations = sample();
        integrations.feeds[0].record_type = TypeId::new("absent_record");
        let report = run(&integrations);
        let found = report
            .find("integrations.feeds[0].record_type")
            .expect("the diagnostic sits on the record type");
        assert_eq!(found.code, DiagnosticCode::TypeMismatch);
    }

    #[test]
    fn an_attestation_naming_an_absent_role_reports_its_location() {
        let mut integrations = sample();
        integrations.bridges[0].attestation =
            AttestationRequirement::signed_by(1, &["absent_role"]);
        let report = run(&integrations);
        let found = report
            .find("integrations.bridges[0].attestation.roles[0]")
            .expect("the diagnostic sits on the role");
        assert_eq!(found.code, DiagnosticCode::DanglingReference);
    }

    #[test]
    fn an_attestation_asking_for_more_signatures_than_roles_is_caught() {
        let mut integrations = sample();
        integrations.triggers[0].attestation =
            AttestationRequirement::signed_by(3, &["feed_signer"]);
        let report = run(&integrations);
        let found = report
            .find("integrations.triggers[0].attestation.signers")
            .expect("the diagnostic sits on the signer count");
        assert_eq!(found.code, DiagnosticCode::ValueOutOfRange);
        assert!(found.fix.contains("lower the count"));
    }

    #[test]
    fn a_trigger_naming_an_absent_action_reports_its_location() {
        let mut integrations = sample();
        integrations.triggers[0].action = ActionId::new("absent_action");
        let report = run(&integrations);
        let found = report
            .find("integrations.triggers[0].action")
            .expect("the diagnostic sits on the action");
        assert_eq!(found.code, DiagnosticCode::DanglingReference);
        assert!(found.fix.contains("refresh_rates"));
    }

    #[test]
    fn an_endpoint_naming_no_network_is_caught() {
        let mut integrations = sample();
        integrations.bridges[0].network = String::new();
        let report = run(&integrations);
        assert!(report.find("integrations.bridges[0].network").is_some());
    }

    #[test]
    fn two_integrations_sharing_a_name_are_caught() {
        let mut integrations = sample();
        integrations.triggers[0].id = IntegrationId::new("rate_feed");
        let report = run(&integrations);
        let found = report
            .find("integrations.triggers[0].id")
            .expect("the diagnostic sits on the repeat");
        assert_eq!(found.code, DiagnosticCode::DuplicateIdentifier);
    }

    #[test]
    fn an_attestation_naming_an_absent_credential_is_caught() {
        let mut integrations = sample();
        integrations.feeds[0].attestation.credentials =
            vec![CredentialId::new("absent_credential")];
        let report = run(&integrations);
        assert!(report
            .find("integrations.feeds[0].attestation.credentials[0]")
            .is_some());
    }

    #[test]
    fn the_section_lists_the_names_it_holds() {
        let integrations = sample();
        assert!(integrations.contains(&IntegrationId::new("rate_feed")));
        assert!(!integrations.contains(&IntegrationId::new("absent")));
        assert_eq!(integrations.names().len(), 3);
        let diagnostic = integrations.missing(
            &IntegrationId::new("absent"),
            Path::root().field("events").index(0).location(),
        );
        assert!(diagnostic.fix.contains("rate_feed"));
    }

    #[test]
    fn the_section_round_trips_through_scale_and_json() {
        let integrations = sample();
        let bytes = integrations.encode();
        let back = Integrations::decode(&mut &bytes[..]).expect("the encoding decodes");
        assert_eq!(integrations, back);
        let text = serde_json::to_string(&integrations).expect("the section serialises");
        let parsed: Integrations = serde_json::from_str(&text).expect("the text parses");
        assert_eq!(integrations, parsed);
    }
}
