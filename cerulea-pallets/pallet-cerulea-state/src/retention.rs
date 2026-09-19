//! Retention expiry and record version history.
//!
//! Every collection declares a retention rule. The rule names a span a
//! record stays for and the action expiry runs. The block hook here checks
//! one watched collection each block and runs the declared action over the
//! records whose span ran out.
//!
//! # The four actions
//!
//! 1. **Review.** The record stays in place. The Blockchain records the
//!    review moment and raises an event, so an operator sees the record
//!    reached its review date.
//! 2. **Erase.** The Blockchain clears the payload and keeps the
//!    commitment. The inclusion tree keeps its leaf, so a verifier still
//!    proves the record was present and still reads its digest.
//! 3. **Anonymise.** The Blockchain clears the listed fields and keeps the
//!    rest. A cleared field takes the blank value of its own shape, so the
//!    record keeps the type the schema registered.
//! 4. **Move.** The Blockchain writes the record into the named collection
//!    and removes it from the collection it sat in.
//!
//! # Boundedness
//!
//! One block checks one collection and at most the declared count of
//! records inside it. The watch list is bounded and the cursor walks it in
//! order, so every watched collection comes round in turn.
//!
//! # Version history
//!
//! Every write appends one entry to the history of the record. An entry
//! carries the version number, the commitment, the block and the account
//! the Blockchain recorded as the author. The history is bounded, so the
//! oldest entry leaves once the bound is reached.

use alloc::vec::Vec;
use cerulea_blueprint::{
    collections::{Collection, RetentionAction, RetentionPeriod},
    types::{Decimal, FieldPath, Identity, Timestamp, Value},
};
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use frame_support::{pallet_prelude::*, weights::Weight, BoundedVec};
use frame_system::pallet_prelude::*;
use pallet_cerulea_schema::{
    primitives::{name_of, CollectionKey, DeploymentId, RecordKey},
    registry::SchemaRegistry,
};
use scale_info::TypeInfo;

use crate::{
    index, proof, Config, Error, Event, History, KeyOrder, Meta, Pallet, RecordCount, RecordMeta,
    Records, RetentionCursor, RetentionSettled, RetentionWatch, WrittenAt,
};

/// One entry of the version history of a record.
#[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo, MaxEncodedLen,
)]
#[scale_info(skip_type_params(BlockNumber, AccountId))]
pub struct RecordVersion<BlockNumber, AccountId> {
    /// The version number the write produced. A first insert gives one.
    pub version: u32,
    /// The hash of the bytes the write stored.
    pub commitment: [u8; 32],
    /// The block the write landed in.
    pub block: BlockNumber,
    /// The account the Blockchain records as the author of the write.
    pub author: AccountId,
    /// The wall clock time of the write, in milliseconds.
    pub at_millis: u64,
}

/// The blank value of one shape.
///
/// A cleared field keeps the shape the schema registered and loses the
/// content the record carried. A value that names one of several
/// alternatives keeps its name, since the schema admits only the names the
/// type declares.
pub fn blank_like(held: &Value) -> Value {
    match held {
        Value::Bool(_) => Value::Bool(false),
        Value::Integer(_) => Value::Integer(0),
        Value::Decimal(number) => Value::Decimal(Decimal::zero(number.scale)),
        Value::Text(_) => Value::Text(alloc::string::String::new()),
        Value::Bytes(_) => Value::Bytes(Vec::new()),
        Value::Timestamp(_) => Value::Timestamp(Timestamp::from_millis(0)),
        Value::Identity(_) => Value::Identity(Identity([0u8; 32])),
        Value::Hash(_) => Value::Hash(cerulea_blueprint::types::Hash32([0u8; 32])),
        Value::List(list) => Value::list(list.element.clone(), Vec::new()),
        Value::Map(map) => Value::map(map.key.clone(), map.value.clone(), Vec::new()),
        Value::Optional(optional) => Value::none(optional.element.clone()),
        Value::Record(record) => {
            let mut produced = record.clone();
            for (_, field) in produced.fields.iter_mut() {
                *field = blank_like(field);
            }
            Value::Record(produced)
        }
        other => other.clone(),
    }
}

/// Write one field of a record value.
///
/// The path walks nested records. The result reports whether the path
/// reached a field.
pub fn set_value_at(record: &mut Value, path: &FieldPath, new: Value) -> bool {
    let mut steps = path.0.iter();
    let head = match steps.next() {
        Some(held) => held,
        None => {
            *record = new;
            return true;
        }
    };
    let rest = FieldPath(steps.cloned().collect());
    match record {
        Value::Record(held) => {
            for (name, slot) in held.fields.iter_mut() {
                if name == head {
                    if rest.0.is_empty() {
                        *slot = new;
                        return true;
                    }
                    return set_value_at(slot, &rest, new);
                }
            }
            false
        }
        Value::Optional(held) => match held.item.as_mut() {
            Some(inner) => {
                let mut whole = FieldPath(alloc::vec![head.clone()]);
                whole.0.extend(rest.0.iter().cloned());
                set_value_at(inner, &whole, new)
            }
            None => false,
        },
        _ => false,
    }
}

/// Clear one field of a record value.
///
/// The result reports whether the path reached a field.
pub fn clear_field(record: &mut Value, path: &FieldPath) -> bool {
    match crate::value_at(record, path) {
        Some(held) => set_value_at(record, path, blank_like(&held)),
        None => false,
    }
}

impl<T: Config> Pallet<T> {
    /// Append one entry to the version history of a record.
    pub(crate) fn note_version(
        slot: &CollectionKey,
        key: &RecordKey,
        meta: &RecordMeta<BlockNumberFor<T>, T::AccountId>,
    ) {
        let entry = RecordVersion {
            version: meta.version,
            commitment: meta.value_hash,
            block: meta.changed_at,
            author: meta.author.clone(),
            at_millis: Self::now_millis(),
        };
        History::<T>::mutate(slot, key, |held| {
            if held.len() as u32 >= T::MaxVersionHistory::get() {
                held.remove(0);
            }
            let _ = held.try_push(entry);
        });
    }

    /// The version history of one record, oldest entry first.
    pub fn version_history(
        deployment: &DeploymentId,
        collection: &cerulea_blueprint::types::CollectionId,
        key: &RecordKey,
    ) -> Vec<RecordVersion<BlockNumberFor<T>, T::AccountId>> {
        match Self::storage_key(deployment, collection) {
            Ok(slot) => History::<T>::get(&slot, key).into_inner(),
            Err(_) => Vec::new(),
        }
    }

    /// Put one collection on the retention watch list.
    ///
    /// A collection whose records stay for the life of the deployment
    /// reaches no sweep, so the list holds the collections with a finite
    /// span alone.
    pub(crate) fn note_retention(slot: &CollectionKey, definition: &Collection) {
        let finite = matches!(
            definition.retention.period,
            RetentionPeriod::Milliseconds(span) if span > 0
        );
        if !finite {
            return;
        }
        RetentionWatch::<T>::mutate(|held| {
            if held.contains(slot) {
                return;
            }
            let _ = held.try_push(slot.clone());
        });
    }

    /// The collections the retention sweep watches.
    pub fn retention_watch() -> Vec<CollectionKey> {
        RetentionWatch::<T>::get().into_inner()
    }

    /// Check one watched collection and run the declared expiry action.
    ///
    /// The cursor moves one place each block, so every watched collection
    /// comes round in turn. One block checks at most the declared count of
    /// records, so one block never runs without bound.
    pub(crate) fn sweep_retention() -> Weight {
        let mut spent = T::DbWeight::get().reads(2);
        let watch = RetentionWatch::<T>::get();
        if watch.is_empty() {
            return spent;
        }
        let length = watch.len();
        let cursor = (RetentionCursor::<T>::get() as usize) % length;
        RetentionCursor::<T>::put(((cursor + 1) % length) as u32);
        let slot = watch[cursor].clone();

        let collection = match Self::collection_id(slot.name.as_slice()) {
            Ok(held) => held,
            Err(_) => return spent,
        };
        let definition = match T::Schema::collection(&slot.deployment, &collection) {
            Some(held) => held,
            None => return spent,
        };
        let span = match definition.retention.period {
            RetentionPeriod::Milliseconds(held) if held > 0 => held,
            _ => return spent,
        };

        let now = Self::now_millis();
        let budget = T::MaxRetentionScan::get();
        let mut checked = 0u32;
        let mut due: Vec<RecordKey> = Vec::new();
        for key in KeyOrder::<T>::iter_key_prefix(&slot) {
            if checked >= budget {
                break;
            }
            checked = checked.saturating_add(1);
            if RetentionSettled::<T>::contains_key(&slot, &key) {
                continue;
            }
            let written = match WrittenAt::<T>::get(&slot, &key) {
                Some(held) => held,
                None => continue,
            };
            if now < written.saturating_add(span) {
                continue;
            }
            due.push(key);
        }
        spent = spent.saturating_add(T::DbWeight::get().reads(checked as u64 * 2));

        for key in due.iter() {
            spent = spent.saturating_add(T::DbWeight::get().reads_writes(4, 6));
            let _ = Self::run_expiry(&slot, &collection, &definition, key, now);
        }
        spent
    }

    /// Run the declared expiry action over one record.
    fn run_expiry(
        slot: &CollectionKey,
        collection: &cerulea_blueprint::types::CollectionId,
        definition: &Collection,
        key: &RecordKey,
        now: u64,
    ) -> DispatchResult {
        match &definition.retention.on_expiry {
            RetentionAction::Review => {
                RetentionSettled::<T>::insert(slot, key, now);
                Self::deposit_event(Event::RetentionReviewDue {
                    deployment: slot.deployment,
                    collection: Self::event_name(collection),
                    key: key.clone(),
                });
                Ok(())
            }
            RetentionAction::Erase => Self::erase_payload(slot, collection, definition, key, now),
            RetentionAction::Anonymise(fields) => {
                Self::anonymise_record(slot, collection, definition, key, fields, now)
            }
            RetentionAction::MoveTo(target) => {
                Self::move_record(slot, collection, definition, key, target, now)
            }
        }
    }

    /// Clear the payload of one record and keep its commitment.
    fn erase_payload(
        slot: &CollectionKey,
        collection: &cerulea_blueprint::types::CollectionId,
        definition: &Collection,
        key: &RecordKey,
        now: u64,
    ) -> DispatchResult {
        let held = match Records::<T>::get(slot, key) {
            Some(found) => found,
            None => {
                RetentionSettled::<T>::insert(slot, key, now);
                return Ok(());
            }
        };
        let previous = Value::decode(&mut held.as_slice()).ok();
        if let Some(record) = previous.as_ref() {
            index::maintain::<T>(slot, definition, key, Some(record), None)?;
        }
        Records::<T>::remove(slot, key);
        RetentionSettled::<T>::insert(slot, key, now);
        Self::deposit_event(Event::RetentionErased {
            deployment: slot.deployment,
            collection: Self::event_name(collection),
            key: key.clone(),
        });
        Ok(())
    }

    /// Clear the listed fields of one record and keep the rest.
    fn anonymise_record(
        slot: &CollectionKey,
        collection: &cerulea_blueprint::types::CollectionId,
        definition: &Collection,
        key: &RecordKey,
        fields: &[FieldPath],
        now: u64,
    ) -> DispatchResult {
        let held = match Records::<T>::get(slot, key) {
            Some(found) => found,
            None => {
                RetentionSettled::<T>::insert(slot, key, now);
                return Ok(());
            }
        };
        let previous =
            Value::decode(&mut held.as_slice()).map_err(|_| Error::<T>::RecordInvalid)?;
        let mut produced = previous.clone();
        for path in fields.iter() {
            clear_field(&mut produced, path);
        }
        produced.canonicalise();
        let bytes = produced.canonical_bytes();
        let bounded: BoundedVec<u8, T::MaxRecordSize> =
            BoundedVec::try_from(bytes).map_err(|_| Error::<T>::RecordTooLarge)?;

        index::maintain::<T>(slot, definition, key, Some(&previous), Some(&produced))?;
        let digest = proof::record_digest(key.as_slice(), bounded.as_slice());
        Self::set_leaf(slot, key, Some(digest))?;
        Records::<T>::insert(slot, key, bounded);
        if let Some(mut meta) = Meta::<T>::get(slot, key) {
            meta.changed_at = frame_system::Pallet::<T>::block_number();
            meta.version = meta.version.saturating_add(1);
            meta.value_hash = digest;
            Meta::<T>::insert(slot, key, &meta);
            Self::note_version(slot, key, &meta);
        }
        RetentionSettled::<T>::insert(slot, key, now);
        Self::deposit_event(Event::RetentionAnonymised {
            deployment: slot.deployment,
            collection: Self::event_name(collection),
            key: key.clone(),
        });
        Ok(())
    }

    /// Move one record into the collection the retention rule names.
    fn move_record(
        slot: &CollectionKey,
        collection: &cerulea_blueprint::types::CollectionId,
        definition: &Collection,
        key: &RecordKey,
        target: &cerulea_blueprint::types::CollectionId,
        now: u64,
    ) -> DispatchResult {
        let held = match Records::<T>::get(slot, key) {
            Some(found) => found,
            None => {
                RetentionSettled::<T>::insert(slot, key, now);
                return Ok(());
            }
        };
        let target_definition = match T::Schema::collection(&slot.deployment, target) {
            Some(found) => found,
            None => return Err(Error::<T>::CollectionUnknown.into()),
        };
        let target_name = name_of(target.as_str()).map_err(|_| Error::<T>::NameTooLong)?;
        let target_slot = CollectionKey::new(slot.deployment, target_name);
        if Records::<T>::contains_key(&target_slot, key) {
            RetentionSettled::<T>::insert(slot, key, now);
            return Ok(());
        }

        let record = Value::decode(&mut held.as_slice()).ok();
        let count = RecordCount::<T>::get(&target_slot);
        ensure!(
            count < T::MaxRecordsPerCollection::get(),
            Error::<T>::CollectionFull
        );

        // Place the record in the target collection.
        if let Some(found) = record.as_ref() {
            index::maintain::<T>(&target_slot, &target_definition, key, None, Some(found))?;
        }
        let digest = proof::record_digest(key.as_slice(), held.as_slice());
        Self::set_leaf(&target_slot, key, Some(digest))?;
        let block = frame_system::Pallet::<T>::block_number();
        let author = match Meta::<T>::get(slot, key) {
            Some(found) => found.author,
            None => return Err(Error::<T>::RecordAbsent.into()),
        };
        let meta = RecordMeta {
            created_at: block,
            changed_at: block,
            version: 1,
            author,
            value_hash: digest,
        };
        Records::<T>::insert(&target_slot, key, held.clone());
        Meta::<T>::insert(&target_slot, key, &meta);
        KeyOrder::<T>::insert(&target_slot, key, ());
        RecordCount::<T>::insert(&target_slot, count.saturating_add(1));
        WrittenAt::<T>::insert(&target_slot, key, now);
        Self::note_version(&target_slot, key, &meta);
        Self::note_retention(&target_slot, &target_definition);

        // Remove the record from the collection it sat in.
        if let Some(found) = record.as_ref() {
            index::maintain::<T>(slot, definition, key, Some(found), None)?;
        }
        Self::set_leaf(slot, key, None)?;
        Records::<T>::remove(slot, key);
        Meta::<T>::remove(slot, key);
        KeyOrder::<T>::remove(slot, key);
        History::<T>::remove(slot, key);
        WrittenAt::<T>::remove(slot, key);
        RecordCount::<T>::mutate(slot, |count| *count = count.saturating_sub(1));
        RetentionSettled::<T>::remove(slot, key);

        Self::deposit_event(Event::RetentionMoved {
            deployment: slot.deployment,
            collection: Self::event_name(collection),
            key: key.clone(),
            target: Self::event_name(target),
        });
        Ok(())
    }
}
