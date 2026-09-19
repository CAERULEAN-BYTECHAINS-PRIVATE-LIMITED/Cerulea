//! Outbound message production.
//!
//! A record marked for a bridge endpoint leaves the deployment as an
//! outbound message. The module builds the envelope, reads the record and
//! its inclusion proof from the state module, and stores the body beside
//! the envelope. The off-chain driver reads the event, carries the message
//! to the far network and reports the reference the far network answered
//! with.
//!
//! # What the far network receives
//!
//! The body names the collection and the key, carries the stored bytes,
//! carries the collection root and carries the proof that the record sits
//! under that root. A reader on the far network folds the proof and
//! compares the result with the root. The far network therefore checks the
//! Cerulea state rather than trusting the relayer.
//!
//! # The commitment
//!
//! The envelope reduces to one digest. The driver writes that digest into
//! the far network. A reply arriving later carries the same digest, so the
//! two sides agree on the message without carrying the whole envelope
//! across twice.

use crate::endpoint::EndpointKey;
use crate::envelope::{MessageEnvelope, MessageId, NetworkAddress, Payload, Travel};
use crate::{
    Config, Error, Event, MessageStatus, Outbound, OutboundBodies, OutboundBody, OutboundNonce,
    OutboundRecord, Pallet, Seen, SeenRecord,
};
use codec::Encode;
use frame_support::pallet_prelude::*;
use frame_system::pallet_prelude::BlockNumberFor;
use pallet_cerulea_audit::contract::WorkClass;
use pallet_cerulea_schema::primitives::{BoundedName, DeploymentId, RecordKey};
use pallet_cerulea_state::RecordStore;

/// Produce one outbound message covering one record.
pub fn produce<T: Config>(
    who: &T::AccountId,
    deployment: DeploymentId,
    name: BoundedName,
    record_key: RecordKey,
    party: NetworkAddress,
) -> DispatchResult {
    let key = EndpointKey::new(deployment, name.clone());
    let record = crate::Endpoints::<T>::get(&key).ok_or(Error::<T>::EndpointUnknown)?;
    ensure!(record.live, Error::<T>::EndpointRetired);
    ensure!(
        record.takes(Travel::Outbound),
        Error::<T>::DirectionDeclined
    );

    let collection = cerulea_blueprint::types::CollectionId::new(
        core::str::from_utf8(record.collection.as_slice()).unwrap_or_default(),
    );
    let value = T::Records::read_bytes(&deployment, &collection, &record_key)
        .ok_or(Error::<T>::RecordAbsent)?;
    let root = T::Records::root(&deployment, &collection).ok_or(Error::<T>::RootAbsent)?;
    let proof =
        T::Records::proof(&deployment, &collection, &record_key).ok_or(Error::<T>::RecordAbsent)?;

    let body = OutboundBody {
        collection: record.collection.clone(),
        key: record_key,
        value,
        root,
        proof,
    };
    let encoded = body.encode();
    let payload: Payload =
        BoundedVec::try_from(encoded.clone()).map_err(|_| Error::<T>::PayloadTooLarge)?;

    let nonce = OutboundNonce::<T>::get(&key);
    let now = Pallet::<T>::now_millis();
    let expiry_millis = now.saturating_add(record.lifetime_millis);
    let envelope = MessageEnvelope {
        deployment,
        endpoint: name.clone(),
        source: name.clone(),
        destination: record.network.clone(),
        party,
        nonce,
        payload,
        class: record.class,
        expiry_millis,
    };
    let id = envelope.id();
    ensure!(
        Seen::<T>::get(&key, id).is_none(),
        Error::<T>::MessageReplayed
    );
    let commitment = envelope.commitment();

    Seen::<T>::insert(
        &key,
        id,
        SeenRecord::<BlockNumberFor<T>> {
            at: frame_system::Pallet::<T>::block_number(),
            height: 0,
            travel: Travel::Outbound,
        },
    );
    OutboundNonce::<T>::insert(&key, nonce.saturating_add(1));
    Outbound::<T>::insert(
        &key,
        id,
        OutboundRecord::<BlockNumberFor<T>> {
            envelope,
            commitment,
            status: MessageStatus::Pending,
            created_at: frame_system::Pallet::<T>::block_number(),
            reference: [0u8; 32],
        },
    );
    OutboundBodies::<T>::insert(
        &key,
        id,
        BoundedVec::try_from(encoded).map_err(|_| Error::<T>::PayloadTooLarge)?,
    );
    Pallet::<T>::schedule(&key, id, Travel::Outbound, expiry_millis)?;

    Pallet::<T>::log(
        &deployment,
        "message.ready",
        record.class,
        who,
        "The deployment produced a bridge message with its inclusion proof.",
        (id, commitment, nonce).encode(),
    )?;
    Pallet::<T>::deposit_event(Event::MessageReady {
        deployment,
        endpoint: name,
        id,
        commitment,
        expiry_millis,
    });
    Ok(())
}

/// Record the delivery of one outbound message.
pub fn confirm<T: Config>(
    who: &T::AccountId,
    deployment: DeploymentId,
    name: BoundedName,
    id: MessageId,
    reference: [u8; 32],
) -> DispatchResult {
    let key = EndpointKey::new(deployment, name.clone());
    ensure!(
        crate::Endpoints::<T>::contains_key(&key),
        Error::<T>::EndpointUnknown
    );
    Outbound::<T>::try_mutate(&key, id, |slot| -> DispatchResult {
        let held = slot.as_mut().ok_or(Error::<T>::MessageUnknown)?;
        ensure!(held.status.is_open(), Error::<T>::MessageSettled);
        held.status = MessageStatus::Delivered;
        held.reference = reference;
        Ok(())
    })?;

    Pallet::<T>::log(
        &deployment,
        "message.delivered",
        WorkClass::Routine,
        who,
        "The far network took delivery of a bridge message.",
        (id, reference).encode(),
    )?;
    Pallet::<T>::deposit_event(Event::MessageDelivered {
        deployment,
        endpoint: name,
        id,
        reference,
    });
    Ok(())
}
