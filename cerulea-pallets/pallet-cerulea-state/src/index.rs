//! Secondary index maintenance on the write path.
//!
//! A collection declares its secondary indexes in the Blueprint. Each
//! index names one or more fields. The state module materialises every
//! declared index as its own map and maintains it on the same write path
//! that writes the record. An index is therefore correct after every
//! insert, every update and every delete.
//!
//! The index value is the concatenation of the canonical encodings of the
//! named field values, each carried behind a four byte length. The length
//! keeps two different field splits apart, so `["ab", "c"]` and
//! `["a", "bc"]` land on different entries. A value running past the index
//! value bound folds to its hash, which keeps the entry usable for an
//! equality lookup.
//!
//! A unique index holds one record key per value. A second record reaching
//! the same value aborts the write, and the whole transaction aborts with
//! it.

use alloc::vec::Vec;
use cerulea_blueprint::{
    collections::{Collection, Index},
    types::{IndexName, Value},
};
use frame_support::{ensure, pallet_prelude::DispatchResult, traits::Get, BoundedVec};
use pallet_cerulea_schema::primitives::{
    index_value_of, name_of, CollectionKey, IndexValue, RecordKey,
};

use crate::{value_at, Config, Error, Indexes};

/// The bytes one index derives from one record.
///
/// A record lacking any named field answers with nothing, so the write
/// path leaves that index alone for that record.
pub fn index_bytes(index: &Index, record: &Value) -> Option<Vec<u8>> {
    let mut held: Vec<u8> = Vec::new();
    for path in index.fields.iter() {
        let mut value = value_at(record, path)?;
        value.canonicalise();
        let part = value.canonical_bytes();
        let length = u32::try_from(part.len()).unwrap_or(u32::MAX);
        held.extend_from_slice(&length.to_be_bytes());
        held.extend_from_slice(&part);
    }
    Some(held)
}

/// The index entry one index derives from one record.
pub fn index_entry(index: &Index, record: &Value) -> Option<IndexValue> {
    index_bytes(index, record).map(index_value_of)
}

/// The index entry one probe reaches.
///
/// The interface layer builds a probe from the values a person typed. The
/// probe carries one value per field the index names, in the declared
/// order.
pub fn index_probe(index: &Index, values: &[Value]) -> Option<IndexValue> {
    if values.len() != index.fields.len() {
        return None;
    }
    let mut held: Vec<u8> = Vec::new();
    for value in values.iter() {
        let mut held_value = value.clone();
        held_value.canonicalise();
        let part = held_value.canonical_bytes();
        let length = u32::try_from(part.len()).unwrap_or(u32::MAX);
        held.extend_from_slice(&length.to_be_bytes());
        held.extend_from_slice(&part);
    }
    Some(index_value_of(held))
}

/// Find one declared index by name.
pub fn declared<'a>(definition: &'a Collection, name: &IndexName) -> Option<&'a Index> {
    definition.indexes.iter().find(|held| &held.name == name)
}

/// Maintain every declared index for one record write.
///
/// The call takes the record the store held and the record the store now
/// holds. A missing record on either side means the write added or removed
/// the record. A record that fails to decode, such as a sealed payload,
/// carries no index entries.
pub fn maintain<T: Config>(
    slot: &CollectionKey,
    definition: &Collection,
    key: &RecordKey,
    previous: Option<&Value>,
    next: Option<&Value>,
) -> DispatchResult {
    let declared_count = u32::try_from(definition.indexes.len()).unwrap_or(u32::MAX);
    ensure!(
        declared_count <= T::MaxIndexesPerCollection::get(),
        Error::<T>::IndexLimitReached
    );

    for index in definition.indexes.iter() {
        let name = name_of(index.name.as_str()).map_err(|_| Error::<T>::NameTooLong)?;
        let before = previous.and_then(|record| index_entry(index, record));
        let after = next.and_then(|record| index_entry(index, record));

        if before.is_some() && before == after {
            continue;
        }
        if let Some(value) = before {
            drop_entry::<T>(slot, &name, &value, key);
        }
        if let Some(value) = after {
            add_entry::<T>(slot, &name, &value, key, index.unique)?;
        }
    }
    Ok(())
}

fn add_entry<T: Config>(
    slot: &CollectionKey,
    name: &BoundedVec<u8, frame_support::traits::ConstU32<128>>,
    value: &IndexValue,
    key: &RecordKey,
    unique: bool,
) -> DispatchResult {
    Indexes::<T>::try_mutate(
        (slot.clone(), name.clone(), value.clone()),
        |held| -> DispatchResult {
            let list = held.get_or_insert_with(Default::default);
            if list.contains(key) {
                return Ok(());
            }
            if unique {
                ensure!(list.is_empty(), Error::<T>::IndexConflict);
            }
            list.try_push(key.clone())
                .map_err(|_| Error::<T>::IndexFull)?;
            Ok(())
        },
    )
}

fn drop_entry<T: Config>(
    slot: &CollectionKey,
    name: &BoundedVec<u8, frame_support::traits::ConstU32<128>>,
    value: &IndexValue,
    key: &RecordKey,
) {
    Indexes::<T>::mutate_exists((slot.clone(), name.clone(), value.clone()), |held| {
        if let Some(list) = held {
            list.retain(|present| present != key);
            if list.is_empty() {
                *held = None;
            }
        }
    });
}

/// The record keys one index entry reaches, in canonical key order.
pub fn lookup<T: Config>(
    slot: &CollectionKey,
    name: &IndexName,
    value: &IndexValue,
) -> Vec<RecordKey> {
    let bounded = match name_of(name.as_str()) {
        Ok(held) => held,
        Err(_) => return Vec::new(),
    };
    let mut found: Vec<RecordKey> = Indexes::<T>::get((slot.clone(), bounded, value.clone()))
        .map(|held| held.into_inner())
        .unwrap_or_default();
    found.sort();
    found
}

/// Clear every entry of every declared index for one record.
///
/// The collections module calls this when a record leaves the plain form,
/// since a sealed payload carries no readable field values.
pub fn clear<T: Config>(
    slot: &CollectionKey,
    definition: &Collection,
    key: &RecordKey,
    record: &Value,
) {
    for index in definition.indexes.iter() {
        let name = match name_of(index.name.as_str()) {
            Ok(held) => held,
            Err(_) => continue,
        };
        if let Some(value) = index_entry(index, record) {
            drop_entry::<T>(slot, &name, &value, key);
        }
    }
}
