//! Query, subscription reads and export over the log.
//!
//! Cerulea Explorer queries by sequence, by actor and by event. Cerulea
//! Dashboard exports a span of the log. An external subscriber reads the
//! entries its subscription follows.
//!
//! Every read carries a bound. A caller asking for more entries than the
//! runtime admits receives the batch limit instead.

use alloc::{string::String, vec::Vec};
use codec::Encode;
use frame_support::pallet_prelude::*;
use pallet_cerulea_schema::primitives::{name_of, name_text, BoundedName, DeploymentId};

use crate::{
    contract::{LogRecord, WorkClass},
    ActorIndex, AuditEntry, Config, Entries, EventIndex, Pallet, Subscriptions,
};
use frame_system::pallet_prelude::BlockNumberFor;

/// The reader form of one stored entry.
pub type Record<T> = LogRecord<<T as frame_system::Config>::AccountId, BlockNumberFor<T>>;

/// Turn one stored entry into the reader form.
pub fn as_record<T: Config>(
    entry: AuditEntry<T::AccountId, BlockNumberFor<T>, T::MaxPayloadBytes>,
) -> Record<T> {
    LogRecord {
        sequence: entry.sequence,
        event: name_text(&entry.event),
        module: name_text(&entry.module),
        class: entry.class,
        actor: entry.actor,
        at_millis: entry.at_millis,
        block: entry.block,
        reason: String::from_utf8_lossy(entry.reason.as_slice()).into_owned(),
        payload: entry.payload.into_inner(),
        corrects: entry.corrects,
        digest: entry.digest,
    }
}

impl<T: Config> Pallet<T> {
    /// The batch size one read returns at most.
    fn batch(limit: u32) -> u64 {
        u64::from(limit.min(T::MaxReadBatch::get()))
    }

    /// One entry in the reader form.
    pub fn read(deployment: &DeploymentId, sequence: u64) -> Option<Record<T>> {
        Entries::<T>::get(deployment, sequence).map(as_record::<T>)
    }

    /// A span of the log, starting at one sequence number.
    ///
    /// The span runs forward from `from` and stops at the batch limit or at
    /// the end of the log.
    pub fn query(deployment: &DeploymentId, from: u64, limit: u32) -> Vec<Record<T>> {
        let end = crate::NextSequence::<T>::get(deployment);
        let batch = Self::batch(limit);
        let mut found = Vec::new();
        let mut cursor = from;
        while cursor < end && (found.len() as u64) < batch {
            if let Some(entry) = Entries::<T>::get(deployment, cursor) {
                found.push(as_record::<T>(entry));
            }
            cursor = cursor.saturating_add(1);
        }
        found
    }

    /// The entries of one actor, in sequence order.
    pub fn query_by_actor(
        deployment: &DeploymentId,
        actor: &T::AccountId,
        from: u64,
        limit: u32,
    ) -> Vec<Record<T>> {
        let end = crate::NextSequence::<T>::get(deployment);
        let batch = Self::batch(limit);
        let mut found = Vec::new();
        let mut cursor = from;
        while cursor < end && (found.len() as u64) < batch {
            if ActorIndex::<T>::contains_key((*deployment, actor.clone(), cursor)) {
                if let Some(entry) = Entries::<T>::get(deployment, cursor) {
                    found.push(as_record::<T>(entry));
                }
            }
            cursor = cursor.saturating_add(1);
        }
        found
    }

    /// The entries of one event name, in sequence order.
    pub fn query_by_event(
        deployment: &DeploymentId,
        event: &str,
        from: u64,
        limit: u32,
    ) -> Vec<Record<T>> {
        let name: BoundedName = match name_of(event) {
            Ok(held) => held,
            Err(_) => return Vec::new(),
        };
        let end = crate::NextSequence::<T>::get(deployment);
        let batch = Self::batch(limit);
        let mut found = Vec::new();
        let mut cursor = from;
        while cursor < end && (found.len() as u64) < batch {
            if EventIndex::<T>::contains_key((*deployment, name.clone(), cursor)) {
                if let Some(entry) = Entries::<T>::get(deployment, cursor) {
                    found.push(as_record::<T>(entry));
                }
            }
            cursor = cursor.saturating_add(1);
        }
        found
    }

    /// The entries one subscription follows, in sequence order.
    pub fn query_for_subscriber(
        deployment: &DeploymentId,
        subscriber: &T::AccountId,
        from: u64,
        limit: u32,
    ) -> Vec<Record<T>> {
        let subscription = match Subscriptions::<T>::get(deployment, subscriber) {
            Some(held) => held,
            None => return Vec::new(),
        };
        let end = crate::NextSequence::<T>::get(deployment);
        let batch = Self::batch(limit);
        let mut found = Vec::new();
        let mut cursor = from.max(subscription.since);
        while cursor < end && (found.len() as u64) < batch {
            if let Some(entry) = Entries::<T>::get(deployment, cursor) {
                if subscription.follows(&entry.event, entry.class, entry.sequence) {
                    found.push(as_record::<T>(entry));
                }
            }
            cursor = cursor.saturating_add(1);
        }
        found
    }

    /// The accounts whose subscription follows one event and one class.
    pub fn subscribers_for(
        deployment: &DeploymentId,
        event: &str,
        class: WorkClass,
        limit: u32,
    ) -> Vec<T::AccountId> {
        let name: BoundedName = match name_of(event) {
            Ok(held) => held,
            Err(_) => return Vec::new(),
        };
        let sequence = crate::NextSequence::<T>::get(deployment).saturating_sub(1);
        let batch = Self::batch(limit) as usize;
        Subscriptions::<T>::iter_prefix(deployment)
            .filter(|(_, subscription)| subscription.follows(&name, class, sequence))
            .map(|(account, _)| account)
            .take(batch)
            .collect()
    }

    /// The canonical export of one span of the log.
    ///
    /// The bytes carry the canonical encoding of the entries. A customer
    /// running a full export reads the whole log through repeated calls.
    pub fn export(deployment: &DeploymentId, from: u64, limit: u32) -> Vec<u8> {
        Self::query(deployment, from, limit).encode()
    }

    /// Recompute the digest chain over one span of the log.
    ///
    /// The check starts from the digest of the entry before `from`. A span
    /// starting at zero starts from the genesis digest of the deployment.
    /// The result reports whether every stored digest matches the
    /// recomputed digest.
    pub fn verify_chain(deployment: &DeploymentId, from: u64, limit: u32) -> bool {
        let end = crate::NextSequence::<T>::get(deployment);
        if from >= end {
            return true;
        }
        let mut running = if from == 0 {
            Self::genesis_digest(deployment)
        } else {
            match Entries::<T>::get(deployment, from.saturating_sub(1)) {
                Some(previous) => previous.digest,
                None => return false,
            }
        };
        let batch = Self::batch(limit);
        let mut cursor = from;
        let mut seen: u64 = 0;
        while cursor < end && seen < batch {
            let entry = match Entries::<T>::get(deployment, cursor) {
                Some(held) => held,
                None => return false,
            };
            let mut folded = running.to_vec();
            folded.extend_from_slice(&entry.body_bytes());
            let computed = sp_crypto_hashing::blake2_256(&folded);
            if computed != entry.digest {
                return false;
            }
            running = computed;
            cursor = cursor.saturating_add(1);
            seen = seen.saturating_add(1);
        }
        true
    }
}
