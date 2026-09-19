//! Expiry and cancellation.
//!
//! Every message carries a moment it stops being acceptable. The module
//! places the message in the bucket of that moment. A bucket covers one
//! minute. The sweep at the start of each block walks the buckets that
//! passed and cancels what it finds.
//!
//! # The bound
//!
//! The sweep handles a fixed count of messages per block and walks a fixed
//! count of buckets per block. A bucket holding more messages than the
//! block budget keeps the rest and the next block continues from there.
//! The work of one block therefore stays inside a known figure whatever
//! the size of the queue.
//!
//! # What cancellation means
//!
//! An outbound message the far network never took moves to cancelled. An
//! inbound message left pending moves to cancelled. A cancelled message
//! keeps its entry in the replay registry, so a late proof of the same
//! message finds the entry and stops. Each cancellation writes one record
//! into the audit log.

use crate::endpoint::EndpointKey;
use crate::envelope::{MessageId, Travel};
use crate::{
    Config, Error, Event, ExpirySchedule, Inbound, MessageStatus, Outbound, Pallet, SweepCursor,
    WeightInfo,
};
use codec::Encode;
use frame_support::pallet_prelude::*;
use pallet_cerulea_audit::contract::WorkClass;
use pallet_cerulea_schema::primitives::{BoundedName, DeploymentId};

/// Run the sweep of one block.
///
/// The answer is the weight the sweep spent.
pub fn run<T: Config>() -> Weight {
    let now = Pallet::<T>::now_millis();
    let current = Pallet::<T>::bucket_of(now);
    let mut cursor = SweepCursor::<T>::get();
    if cursor == 0 || cursor > current {
        return Weight::zero();
    }

    let mut budget = T::MaxSweepPerBlock::get();
    let mut buckets = T::MaxBucketsPerBlock::get();
    let mut spent = Weight::zero();

    while budget > 0 && buckets > 0 && cursor <= current {
        let mut held = ExpirySchedule::<T>::get(cursor);
        while budget > 0 {
            match held.pop() {
                Some((key, id, travel)) => {
                    if cancel_one::<T>(&key, id, travel).is_ok() {
                        spent = spent.saturating_add(T::WeightInfo::sweep_one());
                    }
                    budget = budget.saturating_sub(1);
                }
                None => break,
            }
        }
        if held.is_empty() {
            ExpirySchedule::<T>::remove(cursor);
            cursor = cursor.saturating_add(1);
            buckets = buckets.saturating_sub(1);
        } else {
            ExpirySchedule::<T>::insert(cursor, held);
            break;
        }
    }

    SweepCursor::<T>::put(cursor);
    spent.saturating_add(T::DbWeight::get().reads_writes(2, 2))
}

/// Cancel one message the sweep found.
fn cancel_one<T: Config>(key: &EndpointKey, id: MessageId, travel: Travel) -> DispatchResult {
    let moved = match travel {
        Travel::Outbound => Outbound::<T>::try_mutate(key, id, |slot| -> Result<bool, ()> {
            match slot.as_mut() {
                Some(held) if held.status.is_open() => {
                    held.status = MessageStatus::Cancelled;
                    Ok(true)
                }
                _ => Ok(false),
            }
        })
        .unwrap_or(false),
        Travel::Inbound => Inbound::<T>::try_mutate(key, id, |slot| -> Result<bool, ()> {
            match slot.as_mut() {
                Some(held) if held.status.is_open() => {
                    held.status = MessageStatus::Cancelled;
                    Ok(true)
                }
                _ => Ok(false),
            }
        })
        .unwrap_or(false),
    };
    if !moved {
        return Ok(());
    }

    let actor = Pallet::<T>::platform_account();
    Pallet::<T>::log(
        &key.deployment,
        "message.cancelled",
        WorkClass::Routine,
        &actor,
        "The message passed its expiry and stopped.",
        (id, travel).encode(),
    )?;
    Pallet::<T>::deposit_event(Event::MessageCancelled {
        deployment: key.deployment,
        endpoint: key.name.clone(),
        id,
        travel,
    });
    Ok(())
}

/// Cancel one message that passed its expiry, on request.
///
/// A person cancels a message the sweep has yet to reach. The call
/// declines a message still inside its lifetime, so a cancellation never
/// removes a message a sender may still prove.
pub fn cancel_expired<T: Config>(
    who: &T::AccountId,
    deployment: DeploymentId,
    name: BoundedName,
    id: MessageId,
    travel: Travel,
) -> DispatchResult {
    let key = EndpointKey::new(deployment, name.clone());
    ensure!(
        crate::Endpoints::<T>::contains_key(&key),
        Error::<T>::EndpointUnknown
    );
    let now = Pallet::<T>::now_millis();
    let expiry = match travel {
        Travel::Outbound => Outbound::<T>::get(&key, id)
            .map(|held| (held.envelope.expiry_millis, held.status))
            .ok_or(Error::<T>::MessageUnknown)?,
        Travel::Inbound => Inbound::<T>::get(&key, id)
            .map(|held| (held.envelope.expiry_millis, held.status))
            .ok_or(Error::<T>::MessageUnknown)?,
    };
    ensure!(expiry.1.is_open(), Error::<T>::MessageSettled);
    ensure!(now >= expiry.0, Error::<T>::MessageExpired);

    match travel {
        Travel::Outbound => Outbound::<T>::try_mutate(&key, id, |slot| -> DispatchResult {
            let held = slot.as_mut().ok_or(Error::<T>::MessageUnknown)?;
            held.status = MessageStatus::Cancelled;
            Ok(())
        })?,
        Travel::Inbound => Inbound::<T>::try_mutate(&key, id, |slot| -> DispatchResult {
            let held = slot.as_mut().ok_or(Error::<T>::MessageUnknown)?;
            held.status = MessageStatus::Cancelled;
            Ok(())
        })?,
    }

    Pallet::<T>::log(
        &deployment,
        "message.cancelled",
        WorkClass::Routine,
        who,
        "The message passed its expiry and stopped.",
        (id, travel).encode(),
    )?;
    Pallet::<T>::deposit_event(Event::MessageCancelled {
        deployment,
        endpoint: name,
        id,
        travel,
    });
    Ok(())
}
