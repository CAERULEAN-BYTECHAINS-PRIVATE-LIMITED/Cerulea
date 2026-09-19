//! The Cerulea permission module.
//!
//! The permission module holds the capability matrix. One entry maps a
//! role, a collection and a capability to the fields the role reaches and
//! the rows the role reaches. Every read and every write passes through
//! resolution. A refused operation aborts, and the refusal carries one
//! sentence stating the problem and one sentence naming the fix.
//!
//! Resolution runs five passes in order.
//!
//! 1. **The roles.** The identity module answers which roles the actor
//!    holds at this moment. An assignment past its expiry stays out, and a
//!    role whose credential requirement fails stays out.
//! 2. **The entries.** The matrix answers which entries those roles carry
//!    for the collection and the capability.
//! 3. **The purpose.** A read declaring a purpose the collection leaves
//!    out is refused, and so is a read declaring a purpose the entry
//!    leaves out.
//! 4. **The fields.** An entry names every field, a list of fields, or
//!    every field apart from a list. A request naming a field outside the
//!    reach of every entry is refused.
//! 5. **The rows.** An entry carries an optional predicate over the
//!    record. The predicate reads the record under the name `record`, and
//!    a record the predicate declines is refused.
//!
//! A delegation carries an invoke capability from a role holder to
//! another account for a declared span. Resolution reads the delegation
//! chain when the direct roles carry nothing.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod resolve;
pub mod simulate;
pub mod weights;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub use pallet::*;
pub use resolve::{admits_row, project};
pub use simulate::RecordPreview;
pub use weights::WeightInfo;

use alloc::{format, string::String, string::ToString, vec::Vec};
use cerulea_blueprint::{
    expression::{Env, Expr, Meter},
    permissions::{FieldQualification, ROW_BINDING},
    types::{CollectionId, FieldPath, Identity, ParamName, PurposeTag, Timestamp, Value},
    Path,
};
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use frame_support::pallet_prelude::*;
use frame_system::pallet_prelude::*;
use pallet_cerulea_identity::IdentityRegistry;
use pallet_cerulea_schema::{
    access::{AccessAction, AccessDenial, AccessGrant, AccessRequest, PermissionGate},
    primitives::{name_of, name_text, BoundedName, DeploymentId},
    registry::SchemaRegistry,
};

/// The capability one matrix entry carries.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    DecodeWithMemTracking,
    TypeInfo,
    MaxEncodedLen,
)]
pub enum CapabilityKey {
    /// Read records of the collection.
    Read,
    /// Add records to the collection.
    Insert,
    /// Change records of the collection.
    Update,
    /// Remove records of the collection.
    Delete,
    /// Run one named action.
    Invoke(BoundedName),
}

impl CapabilityKey {
    /// A short label for the interface layer.
    pub fn label(&self) -> &'static str {
        match self {
            CapabilityKey::Read => "read",
            CapabilityKey::Insert => "insert",
            CapabilityKey::Update => "update",
            CapabilityKey::Delete => "delete",
            CapabilityKey::Invoke(_) => "invoke",
        }
    }

    /// Report whether the capability changes stored records.
    pub fn writes(&self) -> bool {
        matches!(
            self,
            CapabilityKey::Insert | CapabilityKey::Update | CapabilityKey::Delete
        )
    }

    /// The capability one access request asks for.
    ///
    /// A name running past the identifier bound answers with nothing.
    pub fn of(action: &AccessAction) -> Option<CapabilityKey> {
        Some(match action {
            AccessAction::Read => CapabilityKey::Read,
            AccessAction::Insert => CapabilityKey::Insert,
            AccessAction::Update => CapabilityKey::Update,
            AccessAction::Delete => CapabilityKey::Delete,
            AccessAction::Invoke(id) => CapabilityKey::Invoke(name_of(id.as_str()).ok()?),
        })
    }
}

/// One entry of the capability matrix.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
#[scale_info(skip_type_params(AccountId))]
pub struct GrantEntry<AccountId> {
    /// The fields the role reaches.
    pub fields: FieldQualification,
    /// The predicate the rows pass. An entry reaching every row carries
    /// none.
    pub rows: Option<Expr>,
    /// The purposes the entry serves. An entry serving every declared
    /// purpose of the collection carries none.
    pub purposes: Vec<PurposeTag>,
    /// The account that granted the entry.
    pub granter: AccountId,
    /// The moment the entry landed, in milliseconds.
    pub granted_at: u64,
}

/// The part of one entry a caller submits.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
pub struct GrantPayload {
    /// The fields the role reaches.
    pub fields: FieldQualification,
    /// The predicate the rows pass.
    pub rows: Option<Expr>,
    /// The purposes the entry serves.
    pub purposes: Vec<PurposeTag>,
}

impl Default for GrantPayload {
    fn default() -> Self {
        GrantPayload {
            fields: FieldQualification::EveryField,
            rows: None,
            purposes: Vec::new(),
        }
    }
}

impl GrantPayload {
    /// A payload reaching every field and every row.
    pub fn everything() -> Self {
        GrantPayload::default()
    }

    /// A payload reaching the named fields alone.
    pub fn only_fields(fields: Vec<FieldPath>) -> Self {
        GrantPayload {
            fields: FieldQualification::Only(fields),
            rows: None,
            purposes: Vec::new(),
        }
    }

    /// A payload reaching every field apart from the named ones.
    pub fn apart_from(fields: Vec<FieldPath>) -> Self {
        GrantPayload {
            fields: FieldQualification::Apart(fields),
            rows: None,
            purposes: Vec::new(),
        }
    }

    /// Add a row predicate.
    pub fn where_rows(mut self, predicate: Expr) -> Self {
        self.rows = Some(predicate);
        self
    }

    /// Name the purposes the entry serves.
    pub fn for_purposes(mut self, purposes: &[&str]) -> Self {
        self.purposes = purposes.iter().map(|held| PurposeTag::new(held)).collect();
        self
    }
}

/// What one role reaches in one collection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoleSimulation {
    /// The role.
    pub role: BoundedName,
    /// The collection.
    pub collection: BoundedName,
    /// The role reads records.
    pub read: bool,
    /// The role adds records.
    pub insert: bool,
    /// The role changes records.
    pub update: bool,
    /// The role removes records.
    pub delete: bool,
    /// The fields the role reads.
    pub readable_fields: FieldQualification,
    /// The predicate the rows pass on a read.
    pub row_predicate: Option<Expr>,
    /// The actions the role runs.
    pub actions: Vec<BoundedName>,
    /// The purposes the read entries serve.
    pub purposes: Vec<PurposeTag>,
}

impl RoleSimulation {
    /// Report whether the role reaches nothing in the collection.
    pub fn is_empty(&self) -> bool {
        !self.read && !self.insert && !self.update && !self.delete && self.actions.is_empty()
    }

    /// One plain sentence stating what the role reaches.
    pub fn statement(&self) -> String {
        let collection = name_text(&self.collection);
        let role = name_text(&self.role);
        if self.is_empty() {
            return format!("The role `{role}` reaches nothing in `{collection}`.");
        }
        let mut held = format!("The role `{role}` reaches `{collection}` for");
        let mut first = true;
        for (present, label) in [
            (self.read, "read"),
            (self.insert, "insert"),
            (self.update, "update"),
            (self.delete, "delete"),
        ] {
            if present {
                if first {
                    held.push(' ');
                    first = false;
                } else {
                    held.push_str(", ");
                }
                held.push_str(label);
            }
        }
        if !self.actions.is_empty() {
            if first {
                held.push(' ');
            } else {
                held.push_str(", ");
            }
            held.push_str("and ");
            held.push_str(&alloc::string::ToString::to_string(&self.actions.len()));
            held.push_str(" actions");
        }
        held.push('.');
        held
    }
}

/// The read a dependant module performs over the capability matrix.
pub trait PermissionMatrix {
    /// The account type the runtime carries.
    type AccountId;

    /// What one role reaches in one collection.
    fn simulate(
        deployment: &DeploymentId,
        role: &BoundedName,
        collection: &BoundedName,
    ) -> RoleSimulation;

    /// Report whether one account reaches one capability of one
    /// collection at this moment.
    fn allows(
        deployment: &DeploymentId,
        who: &Self::AccountId,
        collection: &CollectionId,
        capability: &CapabilityKey,
    ) -> bool;

    /// The fields one account reads in one collection at this moment.
    fn readable_fields(
        deployment: &DeploymentId,
        who: &Self::AccountId,
        collection: &CollectionId,
    ) -> FieldQualification;

    /// One record as one account reads it, with the fields outside its
    /// reach removed.
    ///
    /// A record the row predicate declines answers with nothing.
    fn visible_record(
        deployment: &DeploymentId,
        who: &Self::AccountId,
        collection: &CollectionId,
        record: &Value,
    ) -> Option<Value>;

    /// The records one account reads, each with the fields outside its
    /// reach removed.
    fn visible_records(
        deployment: &DeploymentId,
        who: &Self::AccountId,
        collection: &CollectionId,
        records: &[Value],
    ) -> Vec<Value>;
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    /// The settings the runtime supplies to the permission module.
    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// The event type of the runtime.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// The weight figures of the calls.
        type WeightInfo: WeightInfo;

        /// The registry that holds the collection definitions.
        type Schema: SchemaRegistry;

        /// The registry that answers which roles an actor holds.
        type Identity: IdentityRegistry<AccountId = Self::AccountId>;

        /// The largest encoded entry the module accepts, in bytes.
        #[pallet::constant]
        type MaxEntryBytes: Get<u32>;

        /// The largest number of entries one role carries for one
        /// collection.
        #[pallet::constant]
        type MaxEntriesPerPair: Get<u32>;

        /// The expression step budget a row predicate runs inside.
        #[pallet::constant]
        type RowStepLimit: Get<u64>;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// The capability matrix.
    ///
    /// The four keys name the deployment, the role, the collection and
    /// the capability.
    #[pallet::storage]
    #[pallet::unbounded]
    pub type Matrix<T: Config> = StorageNMap<
        _,
        (
            NMapKey<Blake2_128Concat, DeploymentId>,
            NMapKey<Blake2_128Concat, BoundedName>,
            NMapKey<Blake2_128Concat, BoundedName>,
            NMapKey<Blake2_128Concat, CapabilityKey>,
        ),
        GrantEntry<T::AccountId>,
        OptionQuery,
    >;

    /// The number of entries every role carries for every collection.
    #[pallet::storage]
    pub type EntryCount<T: Config> = StorageNMap<
        _,
        (
            NMapKey<Blake2_128Concat, DeploymentId>,
            NMapKey<Blake2_128Concat, BoundedName>,
            NMapKey<Blake2_128Concat, BoundedName>,
        ),
        u32,
        ValueQuery,
    >;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A role gained a capability over a collection.
        CapabilityGranted {
            /// The deployment.
            deployment: DeploymentId,
            /// The role.
            role: BoundedName,
            /// The collection.
            collection: BoundedName,
            /// The capability.
            capability: CapabilityKey,
        },
        /// A role lost a capability over a collection.
        CapabilityRevoked {
            /// The deployment.
            deployment: DeploymentId,
            /// The role.
            role: BoundedName,
            /// The collection.
            collection: BoundedName,
            /// The capability.
            capability: CapabilityKey,
        },
        /// A preview reported what one role sees of one record.
        RecordPreviewed {
            /// The deployment.
            deployment: DeploymentId,
            /// The role.
            role: BoundedName,
            /// The collection.
            collection: BoundedName,
            /// The role reaches the record.
            visible: bool,
            /// The plain sentence the interface layer shows.
            statement: BoundedVec<u8, ConstU32<512>>,
        },
        /// A preview reported what one role reaches.
        RolePreviewed {
            /// The deployment.
            deployment: DeploymentId,
            /// The role.
            role: BoundedName,
            /// The collection.
            collection: BoundedName,
            /// The plain sentence the interface layer shows.
            statement: BoundedVec<u8, ConstU32<512>>,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// The name runs past 128 bytes. Shorten the role name, the
        /// collection name or the action name.
        NameTooLong,
        /// The registry lacks that collection. Register the collection
        /// before granting a capability over it.
        CollectionUnknown,
        /// The payload failed to decode. Submit the canonical encoding
        /// the Studio produces.
        EntryUnreadable,
        /// The matrix already holds that entry. Revoke the entry before
        /// granting it again.
        EntryPresent,
        /// The matrix holds no such entry. Grant the capability before
        /// revoking it.
        EntryAbsent,
        /// The role carries its full count of entries for that
        /// collection. Revoke an entry, or raise the limit in the runtime.
        EntryLimitReached,
        /// The entry names a field the record type lacks. Name a field the
        /// record type carries.
        FieldUnknown,
        /// The sample record failed to decode. Submit the canonical
        /// encoding the Studio produces.
        RecordUnreadable,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Give one role one capability over one collection.
        ///
        /// The payload carries the fields the role reaches, the row
        /// predicate and the purposes the entry serves.
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::grant())]
        pub fn grant(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            role: BoundedName,
            collection: BoundedName,
            capability: CapabilityKey,
            payload: BoundedVec<u8, T::MaxEntryBytes>,
        ) -> DispatchResult {
            let granter = ensure_signed(origin)?;
            let id = Self::collection_id(&collection)?;
            ensure!(
                T::Schema::collection(&deployment, &id).is_some(),
                Error::<T>::CollectionUnknown
            );
            let parsed = GrantPayload::decode(&mut payload.as_slice())
                .map_err(|_| Error::<T>::EntryUnreadable)?;
            let slot = (
                deployment,
                role.clone(),
                collection.clone(),
                capability.clone(),
            );
            ensure!(
                Matrix::<T>::get(slot.clone()).is_none(),
                Error::<T>::EntryPresent
            );
            let count = EntryCount::<T>::get((deployment, role.clone(), collection.clone()));
            ensure!(
                count < T::MaxEntriesPerPair::get(),
                Error::<T>::EntryLimitReached
            );

            Matrix::<T>::insert(
                slot,
                GrantEntry {
                    fields: parsed.fields,
                    rows: parsed.rows,
                    purposes: parsed.purposes,
                    granter,
                    granted_at: T::Identity::now_millis(),
                },
            );
            EntryCount::<T>::insert(
                (deployment, role.clone(), collection.clone()),
                count.saturating_add(1),
            );
            Self::deposit_event(Event::CapabilityGranted {
                deployment,
                role,
                collection,
                capability,
            });
            Ok(())
        }

        /// Take one capability away from one role.
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::revoke())]
        pub fn revoke(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            role: BoundedName,
            collection: BoundedName,
            capability: CapabilityKey,
        ) -> DispatchResult {
            let _ = ensure_signed(origin)?;
            let slot = (
                deployment,
                role.clone(),
                collection.clone(),
                capability.clone(),
            );
            ensure!(
                Matrix::<T>::get(slot.clone()).is_some(),
                Error::<T>::EntryAbsent
            );
            Matrix::<T>::remove(slot);
            EntryCount::<T>::mutate(
                (deployment, role.clone(), collection.clone()),
                |count| *count = count.saturating_sub(1),
            );
            Self::deposit_event(Event::CapabilityRevoked {
                deployment,
                role,
                collection,
                capability,
            });
            Ok(())
        }

        /// Report what one role sees of one sample record.
        ///
        /// The call resolves the named role directly against the matrix,
        /// so the Studio previews a role before anyone holds it. The call
        /// changes nothing.
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::preview_record_as_role())]
        pub fn preview_record_as_role(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            role: BoundedName,
            collection: BoundedName,
            capability: CapabilityKey,
            record: BoundedVec<u8, T::MaxEntryBytes>,
        ) -> DispatchResult {
            let _ = ensure_signed(origin)?;
            let parsed = Value::decode(&mut record.as_slice())
                .map_err(|_| Error::<T>::RecordUnreadable)?;
            let found = Self::record_preview(&deployment, &role, &collection, &capability, &parsed);
            let statement: BoundedVec<u8, ConstU32<512>> =
                BoundedVec::truncate_from(found.statement().into_bytes());
            Self::deposit_event(Event::RecordPreviewed {
                deployment,
                role,
                collection,
                visible: found.visible,
                statement,
            });
            Ok(())
        }

        /// Report what one role reaches in one collection.
        ///
        /// The Studio shows the outcome beside the matrix a person edits.
        /// The call changes nothing.
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::preview_as_role())]
        pub fn preview_as_role(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            role: BoundedName,
            collection: BoundedName,
        ) -> DispatchResult {
            let _ = ensure_signed(origin)?;
            let found = Self::role_simulation(&deployment, &role, &collection);
            let statement: BoundedVec<u8, ConstU32<512>> =
                BoundedVec::truncate_from(found.statement().into_bytes());
            Self::deposit_event(Event::RolePreviewed {
                deployment,
                role,
                collection,
                statement,
            });
            Ok(())
        }
    }
}

impl<T: Config> Pallet<T> {
    fn collection_id(name: &BoundedName) -> Result<CollectionId, DispatchError> {
        let text = alloc::string::String::from_utf8(name.to_vec())
            .map_err(|_| Error::<T>::CollectionUnknown)?;
        Ok(CollectionId::new(&text))
    }

    /// The entry one role carries for one collection and one capability.
    pub fn entry(
        deployment: &DeploymentId,
        role: &BoundedName,
        collection: &BoundedName,
        capability: &CapabilityKey,
    ) -> Option<GrantEntry<T::AccountId>> {
        Matrix::<T>::get((
            *deployment,
            role.clone(),
            collection.clone(),
            capability.clone(),
        ))
    }

    /// Every capability one role carries for one collection.
    pub fn capabilities(
        deployment: &DeploymentId,
        role: &BoundedName,
        collection: &BoundedName,
    ) -> Vec<CapabilityKey> {
        let mut found: Vec<CapabilityKey> =
            Matrix::<T>::iter_key_prefix((*deployment, role.clone(), collection.clone())).collect();
        found.sort();
        found
    }

    /// What one role reaches in one collection.
    pub fn role_simulation(
        deployment: &DeploymentId,
        role: &BoundedName,
        collection: &BoundedName,
    ) -> RoleSimulation {
        let mut found = RoleSimulation {
            role: role.clone(),
            collection: collection.clone(),
            read: false,
            insert: false,
            update: false,
            delete: false,
            readable_fields: FieldQualification::Only(Vec::new()),
            row_predicate: None,
            actions: Vec::new(),
            purposes: Vec::new(),
        };
        for capability in Self::capabilities(deployment, role, collection) {
            let held = match Self::entry(deployment, role, collection, &capability) {
                Some(entry) => entry,
                None => continue,
            };
            match capability {
                CapabilityKey::Read => {
                    found.read = true;
                    found.readable_fields = held.fields.clone();
                    found.row_predicate = held.rows.clone();
                    found.purposes = held.purposes.clone();
                }
                CapabilityKey::Insert => found.insert = true,
                CapabilityKey::Update => found.update = true,
                CapabilityKey::Delete => found.delete = true,
                CapabilityKey::Invoke(action) => found.actions.push(action),
            }
        }
        found.actions.sort();
        found
    }

    /// What one role sees of one sample record.
    ///
    /// The call resolves the named role directly against the matrix. No
    /// account holds the role, so the Studio previews a draft entry.
    pub fn record_preview(
        deployment: &DeploymentId,
        role: &BoundedName,
        collection: &BoundedName,
        capability: &CapabilityKey,
        record: &Value,
    ) -> RecordPreview {
        let entry = match Self::entry(deployment, role, collection, capability) {
            Some(held) => held,
            None => {
                return RecordPreview::refused(AccessDenial::owned(
                    format!(
                        "The role `{}` reaches no {} of `{}`.",
                        name_text(role),
                        capability.label(),
                        name_text(collection)
                    ),
                    format!(
                        "Grant the role the {} capability over `{}`.",
                        capability.label(),
                        name_text(collection)
                    ),
                ))
            }
        };

        let env = Env::new(Identity([0u8; 32]), Self::now());
        let binding = ParamName::new(ROW_BINDING);
        let mut meter = Meter::new(T::RowStepLimit::get());
        if !resolve::admits_row(entry.rows.as_ref(), record, &env, &mut meter, &binding) {
            return RecordPreview::refused(AccessDenial::owned(
                format!(
                    "The row predicate of the role `{}` declines this record.",
                    name_text(role)
                ),
                "Widen the row predicate, or preview a record the predicate admits."
                    .to_string(),
            ));
        }
        RecordPreview::seen(resolve::project(record, &entry.fields))
    }

    /// Resolve one request and answer with the grant or the refusal.
    pub fn resolve_request(
        request: &AccessRequest<'_, T::AccountId>,
    ) -> Result<AccessGrant, AccessDenial> {
        let capability = CapabilityKey::of(&request.action).ok_or_else(|| {
            AccessDenial::new(
                "The request names an action whose name runs past the limit.",
                "Shorten the action name in the Blueprint.",
            )
        })?;
        let collection = name_of(request.collection.as_str()).map_err(|_| {
            AccessDenial::new(
                "The request names a collection whose name runs past the limit.",
                "Shorten the collection name in the Blueprint.",
            )
        })?;
        let now = T::Identity::now_millis();

        // Pass one. The roles the actor holds at this moment.
        let mut roles = T::Identity::roles_of(&request.deployment, request.actor, now);
        if let AccessAction::Invoke(action) = &request.action {
            if let Ok(named) = name_of(action.as_str()) {
                let outcome =
                    T::Identity::delegation_for(&request.deployment, request.actor, &named, now);
                if let pallet_cerulea_identity::DelegationOutcome::Allowed { role, .. } = outcome {
                    if !roles.contains(&role) {
                        roles.push(role);
                    }
                }
            }
        }
        if roles.is_empty() {
            return Err(AccessDenial::owned(
                format!(
                    "The account holds no role, so the {} of `{}` is refused.",
                    request.action.label(),
                    request.collection
                ),
                "Ask the steward to assign a role to the account.".to_string(),
            ));
        }

        // Pass two. The entries those roles carry.
        let mut admitting: Vec<(BoundedName, GrantEntry<T::AccountId>)> = Vec::new();
        for role in roles.iter() {
            if let Some(held) = Self::entry(&request.deployment, role, &collection, &capability) {
                admitting.push((role.clone(), held));
            }
        }
        if admitting.is_empty() {
            return Err(AccessDenial::owned(
                format!(
                    "No role the account holds reaches the {} of `{}`.",
                    request.action.label(),
                    request.collection
                ),
                format!(
                    "Grant a role of the account the {} capability over `{}`.",
                    request.action.label(),
                    request.collection
                ),
            ));
        }

        // Pass three. The declared purpose of the collection.
        if let Some(purpose) = request.purpose.as_ref() {
            match T::Schema::collection(&request.deployment, &request.collection) {
                Some(definition) => {
                    if !definition.purpose.admits(purpose) {
                        return Err(AccessDenial::owned(
                            format!(
                                "The collection `{}` declares no purpose `{}`.",
                                request.collection, purpose
                            ),
                            format!(
                                "Declare the purpose `{purpose}` on the collection, or read under a declared purpose."
                            ),
                        ));
                    }
                }
                None => {
                    return Err(AccessDenial::owned(
                        format!("The registry lacks the collection `{}`.", request.collection),
                        "Register the collection before reading it.".to_string(),
                    ))
                }
            }
            admitting.retain(|(_, entry)| {
                entry.purposes.is_empty() || entry.purposes.contains(purpose)
            });
            if admitting.is_empty() {
                return Err(AccessDenial::owned(
                    format!(
                        "No entry of the account serves the purpose `{purpose}` on `{}`.",
                        request.collection
                    ),
                    format!("Grant an entry serving the purpose `{purpose}`."),
                ));
            }
        }

        // Pass four. The fields the entries reach.
        if let Some(field) = request.field.as_ref() {
            admitting.retain(|(_, entry)| entry.fields.reaches(field));
            if admitting.is_empty() {
                return Err(AccessDenial::owned(
                    format!(
                        "No role the account holds reaches the field `{field}` of `{}`.",
                        request.collection
                    ),
                    format!("Add `{field}` to the field list of an entry of the account."),
                ));
            }
        }

        // Pass five. The rows the entries reach.
        if let Some(record) = request.record {
            let identity = T::Identity::identity_of(request.actor).unwrap_or([0u8; 32]);
            let env = Env::new(Identity(identity), request.now);
            let binding = ParamName::new(ROW_BINDING);
            let bound = env.with(&binding, record.clone());
            let mut passed: Vec<(BoundedName, GrantEntry<T::AccountId>)> = Vec::new();
            for (role, entry) in admitting.into_iter() {
                let held = match entry.rows.as_ref() {
                    None => true,
                    Some(predicate) => {
                        let mut meter = Meter::new(T::RowStepLimit::get());
                        matches!(
                            predicate.evaluate(&bound, &mut meter, &Path::root().field("rows")),
                            Ok(Value::Bool(true))
                        )
                    }
                };
                if held {
                    passed.push((role, entry));
                }
            }
            if passed.is_empty() {
                return Err(AccessDenial::owned(
                    format!(
                        "The row predicate of every entry declines this record of `{}`.",
                        request.collection
                    ),
                    "Widen the row predicate, or act on a record the predicate admits."
                        .to_string(),
                ));
            }
            admitting = passed;
        }

        let role = admitting.first().map(|(held, _)| held.clone());
        let fields = Self::merge_fields(admitting.iter().map(|(_, entry)| &entry.fields));
        Ok(AccessGrant {
            role: role.map(|held| cerulea_blueprint::types::RoleId::new(&name_text(&held))),
            fields,
        })
    }

    /// Merge the field qualifications of several admitting entries.
    ///
    /// An entry reaching every field widens the result to every field. A
    /// list of held out fields narrows to the fields every entry holds
    /// out. A list of named fields widens to the union of the lists.
    pub fn merge_fields<'a, I>(entries: I) -> FieldQualification
    where
        I: Iterator<Item = &'a FieldQualification>,
    {
        let mut every = false;
        let mut only: Vec<FieldPath> = Vec::new();
        let mut apart: Option<Vec<FieldPath>> = None;
        for held in entries {
            match held {
                FieldQualification::EveryField => every = true,
                FieldQualification::Only(list) => {
                    for field in list.iter() {
                        if !only.contains(field) {
                            only.push(field.clone());
                        }
                    }
                }
                FieldQualification::Apart(list) => {
                    apart = Some(match apart {
                        None => list.clone(),
                        Some(present) => present
                            .into_iter()
                            .filter(|field| list.contains(field))
                            .collect(),
                    });
                }
            }
        }
        if every {
            return FieldQualification::EveryField;
        }
        match apart {
            Some(list) => FieldQualification::Apart(list),
            None => FieldQualification::Only(only),
        }
    }

    /// One record as one account reads it.
    ///
    /// The call resolves the read against the capability matrix, checks
    /// the row predicate and then removes the fields outside the reach of
    /// the account.
    pub fn read_record(
        deployment: &DeploymentId,
        who: &T::AccountId,
        collection: &CollectionId,
        record: &Value,
    ) -> Option<Value> {
        let request = AccessRequest::new(
            *deployment,
            who,
            collection.clone(),
            AccessAction::Read,
            Self::now(),
        )
        .about(record);
        let grant = Self::resolve_request(&request).ok()?;
        Some(resolve::project(record, &grant.fields))
    }

    /// The records one account reads, each with the fields outside its
    /// reach removed.
    pub fn read_records(
        deployment: &DeploymentId,
        who: &T::AccountId,
        collection: &CollectionId,
        records: &[Value],
    ) -> Vec<Value> {
        records
            .iter()
            .filter_map(|record| Self::read_record(deployment, who, collection, record))
            .collect()
    }

    /// The moment the module reads, in milliseconds.
    pub fn now() -> Timestamp {
        Timestamp::from_millis(T::Identity::now_millis())
    }
}

impl<T: Config> PermissionGate for Pallet<T> {
    type AccountId = T::AccountId;

    fn resolve(
        request: &AccessRequest<'_, Self::AccountId>,
    ) -> Result<AccessGrant, AccessDenial> {
        Pallet::<T>::resolve_request(request)
    }
}

impl<T: Config> PermissionMatrix for Pallet<T> {
    type AccountId = T::AccountId;

    fn simulate(
        deployment: &DeploymentId,
        role: &BoundedName,
        collection: &BoundedName,
    ) -> RoleSimulation {
        Pallet::<T>::role_simulation(deployment, role, collection)
    }

    fn allows(
        deployment: &DeploymentId,
        who: &Self::AccountId,
        collection: &CollectionId,
        capability: &CapabilityKey,
    ) -> bool {
        let name = match name_of(collection.as_str()) {
            Ok(held) => held,
            Err(_) => return false,
        };
        let now = T::Identity::now_millis();
        T::Identity::roles_of(deployment, who, now)
            .iter()
            .any(|role| Pallet::<T>::entry(deployment, role, &name, capability).is_some())
    }

    fn visible_record(
        deployment: &DeploymentId,
        who: &Self::AccountId,
        collection: &CollectionId,
        record: &Value,
    ) -> Option<Value> {
        Pallet::<T>::read_record(deployment, who, collection, record)
    }

    fn visible_records(
        deployment: &DeploymentId,
        who: &Self::AccountId,
        collection: &CollectionId,
        records: &[Value],
    ) -> Vec<Value> {
        Pallet::<T>::read_records(deployment, who, collection, records)
    }

    fn readable_fields(
        deployment: &DeploymentId,
        who: &Self::AccountId,
        collection: &CollectionId,
    ) -> FieldQualification {
        let name = match name_of(collection.as_str()) {
            Ok(held) => held,
            Err(_) => return FieldQualification::Only(Vec::new()),
        };
        let now = T::Identity::now_millis();
        let entries: Vec<FieldQualification> = T::Identity::roles_of(deployment, who, now)
            .iter()
            .filter_map(|role| {
                Pallet::<T>::entry(deployment, role, &name, &CapabilityKey::Read)
                    .map(|held| held.fields)
            })
            .collect();
        Pallet::<T>::merge_fields(entries.iter())
    }
}
