//! The Cerulea state module.
//!
//! The state module is the universal typed store. Every record of every
//! deployment lives here, in one double map keyed by collection identifier
//! and record key. The value is the canonical binary encoding of the
//! record.
//!
//! Six operations drive the store. Insert. Update. Delete. Read. Range
//! read. Proof of inclusion.
//!
//! Every write path runs four steps in order.
//!
//! 1. Resolve permission through the permission module.
//! 2. Validate the record against the registered schema.
//! 3. Write the record and its metadata.
//! 4. Maintain every declared index.
//!
//! A failure at any step aborts the whole transaction. The steps run
//! inside one dispatch, so the Blockchain commits all four or none.
//!
//! The record key is the canonical encoding of the primary key value the
//! collection declares. A record therefore carries its own key, and two
//! nodes derive the same key from the same record.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod index;
pub mod weights;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub use pallet::*;
pub use weights::WeightInfo;

use alloc::{vec, vec::Vec};
use cerulea_blueprint::{
    collections::{Collection, KeyRange},
    types::{CollectionId, FieldName, FieldPath, TypeExpr, TypeRegistry, Value},
    Path,
};
use codec::{Decode, Encode};
use frame_support::{pallet_prelude::*, traits::UnixTime};
use frame_system::pallet_prelude::*;
use pallet_cerulea_schema::{
    access::{AccessAction, AccessDenial, AccessGrant, AccessRequest, PermissionGate},
    primitives::{name_of, CollectionKey, DeploymentId, RecordKey},
    registry::SchemaRegistry,
};

/// The metadata the store keeps beside every record.
#[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo, MaxEncodedLen,
)]
#[scale_info(skip_type_params(BlockNumber, AccountId))]
pub struct RecordMeta<BlockNumber, AccountId> {
    /// The block the record first landed at.
    pub created_at: BlockNumber,
    /// The block the record last changed at.
    pub changed_at: BlockNumber,
    /// The number of writes the record has taken. A first insert gives one.
    pub version: u32,
    /// The account the chain records as the author of the last write.
    pub author: AccountId,
    /// The hash of the stored bytes.
    pub value_hash: [u8; 32],
}

/// The read a dependant module performs over the store.
///
/// The collections module and the logic module reach records through this
/// trait. Neither reads state storage directly.
pub trait RecordStore {
    /// The account type the runtime carries.
    type AccountId;

    /// Add one record. The key comes from the declared primary key.
    fn insert(
        deployment: &DeploymentId,
        collection: &CollectionId,
        actor: &Self::AccountId,
        record: &Value,
    ) -> Result<RecordKey, DispatchError>;

    /// Change one record.
    fn update(
        deployment: &DeploymentId,
        collection: &CollectionId,
        actor: &Self::AccountId,
        key: &RecordKey,
        record: &Value,
    ) -> Result<(), DispatchError>;

    /// Remove one record.
    fn delete(
        deployment: &DeploymentId,
        collection: &CollectionId,
        actor: &Self::AccountId,
        key: &RecordKey,
    ) -> Result<(), DispatchError>;

    /// Read one record as a value.
    ///
    /// A sealed record answers with nothing, since the bytes carry
    /// ciphertext rather than a value.
    fn read(deployment: &DeploymentId, collection: &CollectionId, key: &RecordKey)
        -> Option<Value>;

    /// Read the stored bytes of one record.
    fn read_bytes(
        deployment: &DeploymentId,
        collection: &CollectionId,
        key: &RecordKey,
    ) -> Option<Vec<u8>>;

    /// Replace the stored bytes of one record, keeping the key.
    ///
    /// The collections module seals and opens payloads through this call.
    /// The bytes stay opaque to the store.
    fn write_bytes(
        deployment: &DeploymentId,
        collection: &CollectionId,
        actor: &Self::AccountId,
        key: &RecordKey,
        bytes: Vec<u8>,
    ) -> DispatchResult;

    /// The record keys of one collection, in canonical key order.
    fn keys(deployment: &DeploymentId, collection: &CollectionId, limit: u32) -> Vec<RecordKey>;

    /// The records a key range reaches, in canonical key order.
    fn range(
        deployment: &DeploymentId,
        collection: &CollectionId,
        keys: &KeyRange,
        limit: u32,
    ) -> Vec<(RecordKey, Value)>;

    /// The number of records one collection holds.
    fn count(deployment: &DeploymentId, collection: &CollectionId) -> u32;

    /// The record keys one index entry reaches, in canonical key order.
    fn by_index(
        deployment: &DeploymentId,
        collection: &CollectionId,
        index: &cerulea_blueprint::types::IndexName,
        probe: &[Value],
    ) -> Vec<RecordKey>;

    /// The metadata of one record.
    fn meta(
        deployment: &DeploymentId,
        collection: &CollectionId,
        key: &RecordKey,
    ) -> Option<RecordMeta<u32, Self::AccountId>>;
}

/// Read one field of a record value.
///
/// The path walks nested records. A path reaching past a value that holds
/// no fields answers with nothing.
pub fn value_at(record: &Value, path: &FieldPath) -> Option<Value> {
    let mut current = record.clone();
    for step in path.0.iter() {
        current = match current {
            Value::Record(held) => held.field(step)?.clone(),
            Value::Optional(held) => match held.item {
                Some(inner) => match *inner {
                    Value::Record(nested) => nested.field(step)?.clone(),
                    _ => return None,
                },
                None => return None,
            },
            _ => return None,
        };
    }
    Some(current)
}

/// The canonical key bytes of one record under one collection.
pub fn key_bytes_of(collection: &Collection, record: &Value) -> Option<Vec<u8>> {
    let mut held = value_at(record, &collection.primary_key)?;
    held.canonicalise();
    Some(held.canonical_bytes())
}

/// The name of the first field of a path, ready for a diagnostic.
pub fn head_field(path: &FieldPath) -> Option<FieldName> {
    path.0.first().cloned()
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    /// The settings the runtime supplies to the state module.
    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// The event type of the runtime.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// The weight figures of the calls.
        type WeightInfo: WeightInfo;

        /// The registry the store validates against.
        type Schema: SchemaRegistry;

        /// The resolver the store asks before every read and every write.
        type Permissions: PermissionGate<AccountId = Self::AccountId>;

        /// The clock a row predicate reads.
        type Time: UnixTime;

        /// The largest stored record, in bytes.
        #[pallet::constant]
        type MaxRecordSize: Get<u32>;

        /// The largest number of records one range read returns.
        #[pallet::constant]
        type MaxRangeRead: Get<u32>;

        /// The largest number of records one collection holds.
        #[pallet::constant]
        type MaxRecordsPerCollection: Get<u32>;

        /// The largest number of record keys one index entry holds.
        #[pallet::constant]
        type MaxIndexEntries: Get<u32>;

        /// The largest number of declared indexes one collection carries.
        #[pallet::constant]
        type MaxIndexesPerCollection: Get<u32>;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// Every record of every deployment.
    ///
    /// The first key names the collection. The second key is the canonical
    /// encoding of the primary key value. The value is the canonical
    /// encoding of the record.
    #[pallet::storage]
    pub type Records<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        CollectionKey,
        Blake2_128Concat,
        RecordKey,
        BoundedVec<u8, T::MaxRecordSize>,
        OptionQuery,
    >;

    /// The record keys of every collection, held in canonical key order.
    ///
    /// The second key carries the record key unchanged, so a walk over the
    /// map returns the keys in canonical order. That order is what a range
    /// read and a proof both need.
    #[pallet::storage]
    pub type KeyOrder<T: Config> =
        StorageDoubleMap<_, Blake2_128Concat, CollectionKey, Identity, RecordKey, (), OptionQuery>;

    /// The metadata of every record.
    #[pallet::storage]
    pub type Meta<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        CollectionKey,
        Blake2_128Concat,
        RecordKey,
        RecordMeta<BlockNumberFor<T>, T::AccountId>,
        OptionQuery,
    >;

    /// The number of records every collection holds.
    #[pallet::storage]
    pub type RecordCount<T: Config> =
        StorageMap<_, Blake2_128Concat, CollectionKey, u32, ValueQuery>;

    /// Every declared secondary index, materialised as its own map.
    ///
    /// The three keys name the collection, the index and the indexed
    /// value. The value lists the record keys that entry reaches.
    #[pallet::storage]
    pub type Indexes<T: Config> = StorageNMap<
        _,
        (
            NMapKey<Blake2_128Concat, CollectionKey>,
            NMapKey<Blake2_128Concat, BoundedVec<u8, ConstU32<128>>>,
            NMapKey<Blake2_128Concat, pallet_cerulea_schema::primitives::IndexValue>,
        ),
        BoundedVec<RecordKey, T::MaxIndexEntries>,
        OptionQuery,
    >;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A record entered the store.
        RecordInserted {
            /// The deployment.
            deployment: DeploymentId,
            /// The collection.
            collection: BoundedVec<u8, ConstU32<128>>,
            /// The record key.
            key: RecordKey,
            /// The account the chain records as the actor.
            actor: T::AccountId,
        },
        /// A record changed.
        RecordUpdated {
            /// The deployment.
            deployment: DeploymentId,
            /// The collection.
            collection: BoundedVec<u8, ConstU32<128>>,
            /// The record key.
            key: RecordKey,
            /// The version the record now carries.
            version: u32,
            /// The account the chain records as the actor.
            actor: T::AccountId,
        },
        /// A record left the store.
        RecordDeleted {
            /// The deployment.
            deployment: DeploymentId,
            /// The collection.
            collection: BoundedVec<u8, ConstU32<128>>,
            /// The record key.
            key: RecordKey,
            /// The account the chain records as the actor.
            actor: T::AccountId,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// The name runs past 128 bytes. Shorten the collection name.
        NameTooLong,
        /// The registry lacks that collection. Register the collection
        /// before writing records to it.
        CollectionUnknown,
        /// The collection is retired. Register a new collection for new
        /// records.
        CollectionRetired,
        /// The registry lacks the record type the collection names.
        /// Register the record type first.
        RecordTypeUnknown,
        /// The payload failed to decode. Submit the canonical encoding the
        /// Studio produces.
        RecordUnreadable,
        /// The record carries a shape other than the registered one. Match
        /// the record to the registered type and submit again.
        RecordInvalid,
        /// The record carries no value at the declared primary key. Give
        /// the record a primary key value.
        PrimaryKeyMissing,
        /// The primary key value encodes past 256 bytes. Choose a shorter
        /// primary key for the collection.
        PrimaryKeyTooLong,
        /// The record key already holds a record. Update the record rather
        /// than inserting it again.
        RecordPresent,
        /// The store holds no record under that key. Insert the record
        /// before changing it.
        RecordAbsent,
        /// The record encodes past the record size limit. Split the record
        /// across collections, or raise the limit in the runtime.
        RecordTooLarge,
        /// The collection holds its full count of records. Raise the limit
        /// in the runtime.
        CollectionFull,
        /// The submitted key differs from the key the record carries.
        /// Submit the key the primary key field holds.
        KeyMismatch,
        /// The permission module refused the request. Call the explain
        /// entry point to read the problem and the fix in plain language.
        PermissionDenied,
        /// A unique index already holds another record under that value.
        /// Give the record a value the index lacks.
        IndexConflict,
        /// The index entry holds its full count of record keys. Narrow the
        /// index, or raise the limit in the runtime.
        IndexFull,
        /// The collection declares more indexes than the runtime admits.
        /// Remove an index, or raise the limit in the runtime.
        IndexLimitReached,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Add one record.
        ///
        /// The key comes from the primary key field the collection
        /// declares. A key already in the store declines the call.
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::insert())]
        pub fn insert(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            collection: BoundedVec<u8, ConstU32<128>>,
            record: BoundedVec<u8, T::MaxRecordSize>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let id = Self::collection_id(&collection)?;
            let value = Self::decode_record(record.as_slice())?;
            Self::write(&deployment, &id, &who, None, &value, WriteMode::Insert).map(|_| ())
        }

        /// Change one record.
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::update())]
        pub fn update(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            collection: BoundedVec<u8, ConstU32<128>>,
            key: RecordKey,
            record: BoundedVec<u8, T::MaxRecordSize>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let id = Self::collection_id(&collection)?;
            let value = Self::decode_record(record.as_slice())?;
            Self::write(&deployment, &id, &who, Some(key), &value, WriteMode::Update).map(|_| ())
        }

        /// Remove one record.
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::delete())]
        pub fn delete(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            collection: BoundedVec<u8, ConstU32<128>>,
            key: RecordKey,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let id = Self::collection_id(&collection)?;
            Self::remove(&deployment, &id, &who, &key)
        }
    }
}

/// Which write the store performs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriteMode {
    /// The key holds no record yet.
    Insert,
    /// The key holds a record already.
    Update,
    /// The key holds a record already and the caller supplies raw bytes.
    Replace,
}

impl WriteMode {
    fn access(&self) -> AccessAction {
        match self {
            WriteMode::Insert => AccessAction::Insert,
            WriteMode::Update | WriteMode::Replace => AccessAction::Update,
        }
    }
}

impl<T: Config> Pallet<T> {
    /// The clock the row predicates read, in milliseconds.
    pub fn now_millis() -> u64 {
        T::Time::now().as_millis() as u64
    }

    fn timestamp() -> cerulea_blueprint::types::Timestamp {
        cerulea_blueprint::types::Timestamp::from_millis(Self::now_millis())
    }

    fn collection_id(name: &[u8]) -> Result<CollectionId, DispatchError> {
        let text = alloc::string::String::from_utf8(name.to_vec())
            .map_err(|_| Error::<T>::CollectionUnknown)?;
        Ok(CollectionId::new(&text))
    }

    fn storage_key(
        deployment: &DeploymentId,
        collection: &CollectionId,
    ) -> Result<CollectionKey, DispatchError> {
        let name = name_of(collection.as_str()).map_err(|_| Error::<T>::NameTooLong)?;
        Ok(CollectionKey::new(*deployment, name))
    }

    fn event_name(collection: &CollectionId) -> BoundedVec<u8, ConstU32<128>> {
        BoundedVec::try_from(collection.as_str().as_bytes().to_vec()).unwrap_or_default()
    }

    fn decode_record(bytes: &[u8]) -> Result<Value, DispatchError> {
        let mut value = Value::decode(&mut &bytes[..]).map_err(|_| Error::<T>::RecordUnreadable)?;
        value.canonicalise();
        Ok(value)
    }

    /// The live definition of one collection.
    pub fn definition(
        deployment: &DeploymentId,
        collection: &CollectionId,
    ) -> Result<Collection, DispatchError> {
        let found =
            T::Schema::collection(deployment, collection).ok_or(Error::<T>::CollectionUnknown)?;
        Ok(found)
    }

    /// Resolve permission for one request and return the plain sentences
    /// when the request is refused.
    ///
    /// The interface layer calls this entry point to show a person why a
    /// write was refused.
    pub fn explain(request: &AccessRequest<'_, T::AccountId>) -> Result<AccessGrant, AccessDenial> {
        T::Permissions::resolve(request)
    }

    fn resolve(
        deployment: &DeploymentId,
        collection: &CollectionId,
        actor: &T::AccountId,
        action: AccessAction,
        record: Option<&Value>,
    ) -> Result<AccessGrant, DispatchError> {
        let mut request = AccessRequest::new(
            *deployment,
            actor,
            collection.clone(),
            action,
            Self::timestamp(),
        );
        request.record = record;
        T::Permissions::resolve(&request).map_err(|_| Error::<T>::PermissionDenied.into())
    }

    /// Run one write through the four steps of the write path.
    ///
    /// The steps run in order. Resolve permission. Validate against the
    /// registered schema. Write the record. Maintain every declared index.
    pub fn write(
        deployment: &DeploymentId,
        collection: &CollectionId,
        actor: &T::AccountId,
        key: Option<RecordKey>,
        record: &Value,
        mode: WriteMode,
    ) -> Result<RecordKey, DispatchError> {
        // Step one. Resolve permission.
        Self::resolve(deployment, collection, actor, mode.access(), Some(record))?;

        // Step two. Validate against the registered schema.
        let definition = Self::definition(deployment, collection)?;
        ensure!(
            !T::Schema::is_retired(deployment, collection),
            Error::<T>::CollectionRetired
        );
        Self::check_shape(deployment, &definition, record)?;

        let derived = key_bytes_of(&definition, record).ok_or(Error::<T>::PrimaryKeyMissing)?;
        let derived: RecordKey =
            BoundedVec::try_from(derived).map_err(|_| Error::<T>::PrimaryKeyTooLong)?;
        if let Some(supplied) = key.as_ref() {
            ensure!(supplied == &derived, Error::<T>::KeyMismatch);
        }

        // Step three. Write the record and its metadata.
        let bytes = record.canonical_bytes();
        let bounded: BoundedVec<u8, T::MaxRecordSize> =
            BoundedVec::try_from(bytes).map_err(|_| Error::<T>::RecordTooLarge)?;
        Self::store(
            deployment,
            collection,
            actor,
            &definition,
            &derived,
            bounded,
            mode,
        )?;
        Ok(derived)
    }

    fn store(
        deployment: &DeploymentId,
        collection: &CollectionId,
        actor: &T::AccountId,
        definition: &Collection,
        key: &RecordKey,
        bytes: BoundedVec<u8, T::MaxRecordSize>,
        mode: WriteMode,
    ) -> DispatchResult {
        let slot = Self::storage_key(deployment, collection)?;
        let present = Records::<T>::get(&slot, key);
        match mode {
            WriteMode::Insert => ensure!(present.is_none(), Error::<T>::RecordPresent),
            WriteMode::Update | WriteMode::Replace => {
                ensure!(present.is_some(), Error::<T>::RecordAbsent)
            }
        }

        // Step four. Maintain every declared index. A unique index that
        // already holds another record under the new value aborts here,
        // and the whole transaction aborts with it.
        let previous = present
            .as_ref()
            .and_then(|held| Value::decode(&mut held.as_slice()).ok());
        let next = Value::decode(&mut bytes.as_slice()).ok();
        index::maintain::<T>(&slot, definition, key, previous.as_ref(), next.as_ref())?;

        let now = frame_system::Pallet::<T>::block_number();
        let value_hash = sp_crypto_hashing::blake2_256(bytes.as_slice());
        let meta = match Meta::<T>::get(&slot, key) {
            Some(held) => RecordMeta {
                created_at: held.created_at,
                changed_at: now,
                version: held.version.saturating_add(1),
                author: actor.clone(),
                value_hash,
            },
            None => {
                let count = RecordCount::<T>::get(&slot);
                ensure!(
                    count < T::MaxRecordsPerCollection::get(),
                    Error::<T>::CollectionFull
                );
                RecordCount::<T>::insert(&slot, count.saturating_add(1));
                KeyOrder::<T>::insert(&slot, key, ());
                RecordMeta {
                    created_at: now,
                    changed_at: now,
                    version: 1,
                    author: actor.clone(),
                    value_hash,
                }
            }
        };

        let version = meta.version;
        Records::<T>::insert(&slot, key, bytes);
        Meta::<T>::insert(&slot, key, meta);

        if version == 1 {
            Self::deposit_event(Event::RecordInserted {
                deployment: *deployment,
                collection: Self::event_name(collection),
                key: key.clone(),
                actor: actor.clone(),
            });
        } else {
            Self::deposit_event(Event::RecordUpdated {
                deployment: *deployment,
                collection: Self::event_name(collection),
                key: key.clone(),
                version,
                actor: actor.clone(),
            });
        }
        Ok(())
    }

    /// Remove one record.
    pub fn remove(
        deployment: &DeploymentId,
        collection: &CollectionId,
        actor: &T::AccountId,
        key: &RecordKey,
    ) -> DispatchResult {
        let slot = Self::storage_key(deployment, collection)?;
        let held = Records::<T>::get(&slot, key).ok_or(Error::<T>::RecordAbsent)?;
        let record = Value::decode(&mut held.as_slice()).ok();
        Self::resolve(
            deployment,
            collection,
            actor,
            AccessAction::Delete,
            record.as_ref(),
        )?;
        ensure!(
            !T::Schema::is_retired(deployment, collection),
            Error::<T>::CollectionRetired
        );

        let definition = Self::definition(deployment, collection)?;
        if let Some(held) = record.as_ref() {
            index::maintain::<T>(&slot, &definition, key, Some(held), None)?;
        }
        Records::<T>::remove(&slot, key);
        Meta::<T>::remove(&slot, key);
        KeyOrder::<T>::remove(&slot, key);
        RecordCount::<T>::mutate(&slot, |count| *count = count.saturating_sub(1));

        Self::deposit_event(Event::RecordDeleted {
            deployment: *deployment,
            collection: Self::event_name(collection),
            key: key.clone(),
            actor: actor.clone(),
        });
        Ok(())
    }

    /// Check that a record carries the registered shape.
    ///
    /// The check decodes the record against the record type the collection
    /// names. A record carrying another shape aborts the write.
    pub fn check_shape(
        deployment: &DeploymentId,
        definition: &Collection,
        record: &Value,
    ) -> DispatchResult {
        let defs = T::Schema::type_defs(deployment);
        let (types, _) = TypeRegistry::build(&defs, &Path::root().field("types"));
        ensure!(
            types.record(&definition.record_type).is_some(),
            Error::<T>::RecordTypeUnknown
        );
        record
            .check(
                &TypeExpr::Record(definition.record_type.clone()),
                &types,
                &Path::root().field("record"),
            )
            .map_err(|_| Error::<T>::RecordInvalid)?;
        Ok(())
    }

    /// Read one record as a value.
    pub fn record(
        deployment: &DeploymentId,
        collection: &CollectionId,
        key: &RecordKey,
    ) -> Option<Value> {
        let bytes = Self::record_bytes(deployment, collection, key)?;
        Value::decode(&mut bytes.as_slice()).ok()
    }

    /// Read the stored bytes of one record.
    pub fn record_bytes(
        deployment: &DeploymentId,
        collection: &CollectionId,
        key: &RecordKey,
    ) -> Option<Vec<u8>> {
        let slot = Self::storage_key(deployment, collection).ok()?;
        Records::<T>::get(&slot, key).map(|held| held.into_inner())
    }

    /// The record keys of one collection, in canonical key order.
    pub fn record_keys(
        deployment: &DeploymentId,
        collection: &CollectionId,
        limit: u32,
    ) -> Vec<RecordKey> {
        let slot = match Self::storage_key(deployment, collection) {
            Ok(held) => held,
            Err(_) => return Vec::new(),
        };
        let ceiling = limit.min(T::MaxRangeRead::get()) as usize;
        let mut found: Vec<RecordKey> = KeyOrder::<T>::iter_key_prefix(&slot).collect();
        found.sort();
        found.truncate(ceiling);
        found
    }

    /// The records a key range reaches, in canonical key order.
    pub fn range_read(
        deployment: &DeploymentId,
        collection: &CollectionId,
        keys: &KeyRange,
        limit: u32,
    ) -> Vec<(RecordKey, Value)> {
        Self::range_bytes(deployment, collection, keys, limit)
            .into_iter()
            .filter_map(|(key, bytes)| {
                Value::decode(&mut bytes.as_slice())
                    .ok()
                    .map(|value| (key, value))
            })
            .collect()
    }

    /// The stored bytes a key range reaches, in canonical key order.
    pub fn range_bytes(
        deployment: &DeploymentId,
        collection: &CollectionId,
        keys: &KeyRange,
        limit: u32,
    ) -> Vec<(RecordKey, Vec<u8>)> {
        let slot = match Self::storage_key(deployment, collection) {
            Ok(held) => held,
            Err(_) => return Vec::new(),
        };
        let ceiling = limit.min(T::MaxRangeRead::get()) as usize;
        let mut found: Vec<RecordKey> = KeyOrder::<T>::iter_key_prefix(&slot)
            .filter(|key| Self::covers(keys, key.as_slice()))
            .collect();
        found.sort();
        found.truncate(ceiling);
        found
            .into_iter()
            .filter_map(|key| Records::<T>::get(&slot, &key).map(|bytes| (key, bytes.into_inner())))
            .collect()
    }

    fn covers(range: &KeyRange, key: &[u8]) -> bool {
        match range {
            KeyRange::Full => true,
            KeyRange::Exact(held) => held.as_slice() == key,
            KeyRange::Prefix(held) => key.starts_with(held.as_slice()),
        }
    }

    /// The record keys one index entry reaches, in canonical key order.
    ///
    /// The probe carries one value per field the index names, in the
    /// declared order. A probe of another shape reaches nothing.
    pub fn index_lookup(
        deployment: &DeploymentId,
        collection: &CollectionId,
        name: &cerulea_blueprint::types::IndexName,
        probe: &[Value],
    ) -> Vec<RecordKey> {
        let slot = match Self::storage_key(deployment, collection) {
            Ok(held) => held,
            Err(_) => return Vec::new(),
        };
        let definition = match T::Schema::collection(deployment, collection) {
            Some(held) => held,
            None => return Vec::new(),
        };
        let declared = match index::declared(&definition, name) {
            Some(held) => held,
            None => return Vec::new(),
        };
        let value = match index::index_probe(declared, probe) {
            Some(held) => held,
            None => return Vec::new(),
        };
        index::lookup::<T>(&slot, name, &value)
    }

    /// The number of records one collection holds.
    pub fn record_count(deployment: &DeploymentId, collection: &CollectionId) -> u32 {
        match Self::storage_key(deployment, collection) {
            Ok(slot) => RecordCount::<T>::get(&slot),
            Err(_) => 0,
        }
    }

    /// The metadata of one record.
    pub fn record_meta(
        deployment: &DeploymentId,
        collection: &CollectionId,
        key: &RecordKey,
    ) -> Option<RecordMeta<BlockNumberFor<T>, T::AccountId>> {
        let slot = Self::storage_key(deployment, collection).ok()?;
        Meta::<T>::get(&slot, key)
    }
}

impl<T: Config> RecordStore for Pallet<T> {
    type AccountId = T::AccountId;

    fn insert(
        deployment: &DeploymentId,
        collection: &CollectionId,
        actor: &Self::AccountId,
        record: &Value,
    ) -> Result<RecordKey, DispatchError> {
        Pallet::<T>::write(
            deployment,
            collection,
            actor,
            None,
            record,
            WriteMode::Insert,
        )
    }

    fn update(
        deployment: &DeploymentId,
        collection: &CollectionId,
        actor: &Self::AccountId,
        key: &RecordKey,
        record: &Value,
    ) -> Result<(), DispatchError> {
        Pallet::<T>::write(
            deployment,
            collection,
            actor,
            Some(key.clone()),
            record,
            WriteMode::Update,
        )
        .map(|_| ())
    }

    fn delete(
        deployment: &DeploymentId,
        collection: &CollectionId,
        actor: &Self::AccountId,
        key: &RecordKey,
    ) -> Result<(), DispatchError> {
        Pallet::<T>::remove(deployment, collection, actor, key)
    }

    fn read(
        deployment: &DeploymentId,
        collection: &CollectionId,
        key: &RecordKey,
    ) -> Option<Value> {
        Pallet::<T>::record(deployment, collection, key)
    }

    fn read_bytes(
        deployment: &DeploymentId,
        collection: &CollectionId,
        key: &RecordKey,
    ) -> Option<Vec<u8>> {
        Pallet::<T>::record_bytes(deployment, collection, key)
    }

    fn write_bytes(
        deployment: &DeploymentId,
        collection: &CollectionId,
        actor: &Self::AccountId,
        key: &RecordKey,
        bytes: Vec<u8>,
    ) -> DispatchResult {
        let definition = Pallet::<T>::definition(deployment, collection)?;
        let bounded: BoundedVec<u8, T::MaxRecordSize> =
            BoundedVec::try_from(bytes).map_err(|_| Error::<T>::RecordTooLarge)?;
        Pallet::<T>::store(
            deployment,
            collection,
            actor,
            &definition,
            key,
            bounded,
            WriteMode::Replace,
        )
    }

    fn keys(deployment: &DeploymentId, collection: &CollectionId, limit: u32) -> Vec<RecordKey> {
        Pallet::<T>::record_keys(deployment, collection, limit)
    }

    fn range(
        deployment: &DeploymentId,
        collection: &CollectionId,
        keys: &KeyRange,
        limit: u32,
    ) -> Vec<(RecordKey, Value)> {
        Pallet::<T>::range_read(deployment, collection, keys, limit)
    }

    fn count(deployment: &DeploymentId, collection: &CollectionId) -> u32 {
        Pallet::<T>::record_count(deployment, collection)
    }

    fn by_index(
        deployment: &DeploymentId,
        collection: &CollectionId,
        index: &cerulea_blueprint::types::IndexName,
        probe: &[Value],
    ) -> Vec<RecordKey> {
        Pallet::<T>::index_lookup(deployment, collection, index, probe)
    }

    fn meta(
        deployment: &DeploymentId,
        collection: &CollectionId,
        key: &RecordKey,
    ) -> Option<RecordMeta<u32, Self::AccountId>> {
        let held = Pallet::<T>::record_meta(deployment, collection, key)?;
        Some(RecordMeta {
            created_at: held.created_at.try_into().unwrap_or(u32::MAX),
            changed_at: held.changed_at.try_into().unwrap_or(u32::MAX),
            version: held.version,
            author: held.author,
            value_hash: held.value_hash,
        })
    }
}

impl<T: Config> pallet_cerulea_schema::MigrationHook for Pallet<T> {
    fn migrate_records(
        deployment: &DeploymentId,
        plan: &pallet_cerulea_schema::migrate::MigrationPlan,
        produced_type: &cerulea_blueprint::types::RecordDef,
        produced_collection: &Collection,
    ) -> Result<u32, DispatchError> {
        Pallet::<T>::rewrite(deployment, plan, produced_type, produced_collection)
    }

    fn revert_records(
        deployment: &DeploymentId,
        inverse: &pallet_cerulea_schema::migrate::MigrationPlan,
        restored_type: &cerulea_blueprint::types::RecordDef,
        restored_collection: &Collection,
    ) -> Result<u32, DispatchError> {
        Pallet::<T>::rewrite(deployment, inverse, restored_type, restored_collection)
    }
}

impl<T: Config> Pallet<T> {
    /// Rewrite every record of one collection through a migration plan.
    ///
    /// The schema module calls this inside its apply transaction. A record
    /// whose primary key value moves takes its new key, and the old key
    /// leaves the store. A failure at any record aborts the whole call, so
    /// the definition change and the record change land together.
    pub fn rewrite(
        deployment: &DeploymentId,
        plan: &pallet_cerulea_schema::migrate::MigrationPlan,
        produced_type: &cerulea_blueprint::types::RecordDef,
        produced_collection: &Collection,
    ) -> Result<u32, DispatchError> {
        let slot = Self::storage_key(deployment, &plan.collection)?;
        let defs = pallet_cerulea_schema::migrate::with_replaced_record(
            &T::Schema::type_defs(deployment),
            produced_type.clone(),
        );
        let (types, _) = TypeRegistry::build(&defs, &Path::root().field("types"));
        let target = TypeExpr::Record(produced_type.id.clone());

        let env = cerulea_blueprint::expression::Env::new(
            cerulea_blueprint::types::Identity([0u8; 32]),
            Self::timestamp(),
        );
        let previous_definition = Self::definition(deployment, &plan.collection)?;
        let mut keys: Vec<RecordKey> = KeyOrder::<T>::iter_key_prefix(&slot).collect();
        keys.sort();

        let mut rewritten = 0u32;
        for key in keys {
            let held = match Records::<T>::get(&slot, &key) {
                Some(found) => found,
                None => continue,
            };
            let record = match Value::decode(&mut held.as_slice()) {
                Ok(found) => found,
                Err(_) => continue,
            };
            let mut meter = cerulea_blueprint::expression::Meter::new(
                cerulea_blueprint::expression::DEFAULT_STEP_LIMIT,
            );
            let produced = pallet_cerulea_schema::migrate::migrate_record(
                plan,
                &record,
                &env,
                &mut meter,
                &Path::root().field("record"),
            )
            .map_err(|_| Error::<T>::RecordInvalid)?;
            produced
                .check(&target, &types, &Path::root().field("record"))
                .map_err(|_| Error::<T>::RecordInvalid)?;

            let moved = key_bytes_of(produced_collection, &produced)
                .ok_or(Error::<T>::PrimaryKeyMissing)?;
            let moved: RecordKey =
                BoundedVec::try_from(moved).map_err(|_| Error::<T>::PrimaryKeyTooLong)?;
            let bytes: BoundedVec<u8, T::MaxRecordSize> =
                BoundedVec::try_from(produced.canonical_bytes())
                    .map_err(|_| Error::<T>::RecordTooLarge)?;

            // The declared indexes follow the record into its new shape.
            index::maintain::<T>(&slot, &previous_definition, &key, Some(&record), None)?;
            index::maintain::<T>(&slot, produced_collection, &moved, None, Some(&produced))?;

            let meta = Meta::<T>::get(&slot, &key);
            if moved != key {
                Records::<T>::remove(&slot, &key);
                Meta::<T>::remove(&slot, &key);
                KeyOrder::<T>::remove(&slot, &key);
                KeyOrder::<T>::insert(&slot, &moved, ());
            }
            let now = frame_system::Pallet::<T>::block_number();
            let value_hash = sp_crypto_hashing::blake2_256(bytes.as_slice());
            let updated = match meta {
                Some(held) => RecordMeta {
                    created_at: held.created_at,
                    changed_at: now,
                    version: held.version.saturating_add(1),
                    author: held.author,
                    value_hash,
                },
                None => continue,
            };
            Records::<T>::insert(&slot, &moved, bytes);
            Meta::<T>::insert(&slot, &moved, updated);
            rewritten = rewritten.saturating_add(1);
        }
        Ok(rewritten)
    }
}
