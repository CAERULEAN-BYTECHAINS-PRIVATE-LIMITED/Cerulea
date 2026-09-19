//! The access contract every platform module shares.
//!
//! The state module resolves permission before every read and before every
//! write. The permission module answers. Neither module depends on the
//! other as a crate. The contract lives here, at the root of the platform
//! module graph, so both modules read one definition.
//!
//! A resolution answers with a grant or with a denial. A grant names the
//! role that carried the caller and the fields the caller reaches. A
//! denial carries one sentence stating the problem and one sentence naming
//! the fix, both ready for a person to read.

use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use cerulea_blueprint::{
    permissions::FieldQualification,
    types::{ActionId, CollectionId, FieldPath, PurposeTag, RoleId, Timestamp, Value},
};
use core::marker::PhantomData;

use crate::primitives::DeploymentId;

/// What a caller asks to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AccessAction {
    /// Read records.
    Read,
    /// Add one record.
    Insert,
    /// Change one record.
    Update,
    /// Remove one record.
    Delete,
    /// Run one action.
    Invoke(ActionId),
}

impl AccessAction {
    /// A short label for the interface layer.
    pub fn label(&self) -> &'static str {
        match self {
            AccessAction::Read => "read",
            AccessAction::Insert => "insert",
            AccessAction::Update => "update",
            AccessAction::Delete => "delete",
            AccessAction::Invoke(_) => "invoke",
        }
    }

    /// Report whether the request changes stored records.
    pub fn writes(&self) -> bool {
        matches!(
            self,
            AccessAction::Insert | AccessAction::Update | AccessAction::Delete
        )
    }
}

/// One question put to the permission module.
#[derive(Clone, Debug)]
pub struct AccessRequest<'a, AccountId> {
    /// The deployment the request belongs to.
    pub deployment: DeploymentId,
    /// The account the chain records as the actor.
    pub actor: &'a AccountId,
    /// The collection the request reaches.
    pub collection: CollectionId,
    /// What the caller asks to do.
    pub action: AccessAction,
    /// The field the caller reaches, where the request names one.
    pub field: Option<FieldPath>,
    /// The record the request reads or writes, where one is present. A row
    /// predicate reads this record.
    pub record: Option<&'a Value>,
    /// The purpose the caller declares, where the request names one.
    pub purpose: Option<PurposeTag>,
    /// The clock the row predicate reads.
    pub now: Timestamp,
}

impl<'a, AccountId> AccessRequest<'a, AccountId> {
    /// Build a request naming a collection and an action.
    pub fn new(
        deployment: DeploymentId,
        actor: &'a AccountId,
        collection: CollectionId,
        action: AccessAction,
        now: Timestamp,
    ) -> Self {
        AccessRequest {
            deployment,
            actor,
            collection,
            action,
            field: None,
            record: None,
            purpose: None,
            now,
        }
    }

    /// Name the record the request reads or writes.
    pub fn about(mut self, record: &'a Value) -> Self {
        self.record = Some(record);
        self
    }

    /// Name the field the request reaches.
    pub fn on_field(mut self, field: FieldPath) -> Self {
        self.field = Some(field);
        self
    }

    /// Name the purpose the caller declares.
    pub fn for_purpose(mut self, purpose: PurposeTag) -> Self {
        self.purpose = Some(purpose);
        self
    }
}

/// The answer to an admitted request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccessGrant {
    /// The role that carried the caller, where a role carried the caller.
    pub role: Option<RoleId>,
    /// The fields the caller reaches.
    pub fields: FieldQualification,
}

impl AccessGrant {
    /// A grant reaching every field.
    pub fn every_field(role: Option<RoleId>) -> Self {
        AccessGrant {
            role,
            fields: FieldQualification::EveryField,
        }
    }

    /// Report whether the grant reaches one field.
    pub fn reaches(&self, field: &FieldPath) -> bool {
        self.fields.reaches(field)
    }

    /// The fields the grant names, where the grant lists fields.
    pub fn listed_fields(&self) -> &[FieldPath] {
        self.fields.listed()
    }
}

/// The answer to a refused request.
///
/// Both sentences reach a person unchanged. The interface layer shows the
/// problem and the fix side by side.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccessDenial {
    /// One sentence stating the problem.
    pub problem: String,
    /// One sentence naming the fix.
    pub fix: String,
}

impl AccessDenial {
    /// Build a denial.
    pub fn new(problem: &str, fix: &str) -> Self {
        AccessDenial {
            problem: problem.to_string(),
            fix: fix.to_string(),
        }
    }

    /// Build a denial from owned sentences.
    pub fn owned(problem: String, fix: String) -> Self {
        AccessDenial { problem, fix }
    }

    /// The denial as one line, ready for a log entry.
    pub fn line(&self) -> String {
        let mut held = self.problem.clone();
        held.push(' ');
        held.push_str(&self.fix);
        held
    }

    /// The denial as bytes, ready for an event field.
    pub fn bytes(&self) -> Vec<u8> {
        self.line().into_bytes()
    }
}

/// The resolver the state module asks before every read and every write.
///
/// The permission module implements this trait. A runtime that leaves the
/// permission module out supplies [`DeniedByDefault`], which refuses every
/// request and names the fix.
pub trait PermissionGate {
    /// The account type the runtime carries.
    type AccountId;

    /// Answer one request.
    fn resolve(request: &AccessRequest<'_, Self::AccountId>) -> Result<AccessGrant, AccessDenial>;
}

/// The resolver of a runtime that carries no permission module.
///
/// Every request is refused. The fix names the permission module.
pub struct DeniedByDefault<AccountId>(PhantomData<AccountId>);

impl<AccountId> PermissionGate for DeniedByDefault<AccountId> {
    type AccountId = AccountId;

    fn resolve(request: &AccessRequest<'_, Self::AccountId>) -> Result<AccessGrant, AccessDenial> {
        Err(AccessDenial::owned(
            alloc::format!(
                "The deployment carries no capability matrix, so the {} of `{}` is refused.",
                request.action.label(),
                request.collection
            ),
            "Add the permission module to the runtime and grant the role its capabilities."
                .to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cerulea_blueprint::types::Value;

    #[test]
    fn a_grant_reaching_every_field_admits_any_field() {
        let grant = AccessGrant::every_field(Some(RoleId::new("registrar")));
        assert!(grant.reaches(&FieldPath::parse("holder")));
        assert!(grant.listed_fields().is_empty());
    }

    #[test]
    fn a_denial_reads_as_one_line() {
        let denial = AccessDenial::new(
            "The role reaches no record in the register.",
            "Grant the role read access to the register.",
        );
        assert_eq!(
            denial.line(),
            "The role reaches no record in the register. Grant the role read access to the register."
        );
        assert_eq!(denial.bytes(), denial.line().into_bytes());
    }

    #[test]
    fn the_default_resolver_refuses_and_names_the_fix() {
        let actor = 5u64;
        let request = AccessRequest::new(
            [1u8; 32],
            &actor,
            CollectionId::new("parcel"),
            AccessAction::Insert,
            Timestamp::from_millis(0),
        );
        let denial = DeniedByDefault::<u64>::resolve(&request)
            .expect_err("a runtime without the permission module refuses");
        assert!(denial.fix.contains("permission module"));
    }

    #[test]
    fn a_request_carries_the_record_it_asks_about() {
        let actor = 5u64;
        let record = Value::Bool(true);
        let request = AccessRequest::new(
            [1u8; 32],
            &actor,
            CollectionId::new("parcel"),
            AccessAction::Read,
            Timestamp::from_millis(7),
        )
        .about(&record)
        .on_field(FieldPath::parse("holder"))
        .for_purpose(PurposeTag::new("registration"));
        assert_eq!(request.record, Some(&record));
        assert_eq!(request.field, Some(FieldPath::parse("holder")));
        assert_eq!(request.purpose, Some(PurposeTag::new("registration")));
        assert!(!request.action.writes());
    }
}
